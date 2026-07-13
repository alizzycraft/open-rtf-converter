#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use thiserror::Error;

use crate::config::RtfParseOptions;
use crate::diagnostics::Diagnostic;
use crate::fonts::{FontCoverage, FontProvider, FontProviderError};
use crate::layout::{LayoutEngine, PdfFontFamily, passive_pdf_font_family_for_font};
use crate::model::{Block, Document, FontDef, Paragraph, Run, StaticShape, Table};
use crate::pdf::{PassivePdfError, audit_passive_pdf_bytes, render_pdf_with_font_provider};
use crate::rtf::{ParseError, parse_rtf_bytes_with_options};

#[derive(Debug, Clone)]
pub struct ConvertOptions {
    pub diagnostics: bool,
    pub parse_options: RtfParseOptions,
    pub font_provider: FontProvider,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            diagnostics: false,
            parse_options: RtfParseOptions::default(),
            font_provider: FontProvider::default(),
        }
    }
}

impl ConvertOptions {
    pub fn browser_safe_defaults() -> Self {
        Self {
            diagnostics: false,
            parse_options: RtfParseOptions::browser_safe_defaults(),
            font_provider: FontProvider::browser_safe_defaults(),
        }
    }
}

#[derive(Debug)]
pub struct ConvertReport {
    pub diagnostics: Vec<Diagnostic>,
    pub pages: usize,
}

#[derive(Debug)]
pub struct ConversionOutput {
    pub pdf: Vec<u8>,
    pub diagnostics: Vec<Diagnostic>,
    pub pages: usize,
}

#[derive(Debug, Error)]
pub enum ConvertError {
    #[cfg(not(target_arch = "wasm32"))]
    #[error("failed to read input: {0}")]
    ReadInput(#[source] std::io::Error),
    #[cfg(not(target_arch = "wasm32"))]
    #[error("failed to write output: {0}")]
    WriteOutput(#[source] std::io::Error),
    #[error(transparent)]
    PdfSafety(#[from] PassivePdfError),
    #[error("rendered PDF output exceeded limit: {size} bytes > {limit} bytes")]
    OutputTooLarge { size: usize, limit: usize },
    #[error("rendered PDF page count exceeded limit: {pages} pages > {limit} pages")]
    TooManyPages { pages: usize, limit: usize },
    #[error(transparent)]
    FontProvider(#[from] FontProviderError),
    #[error(transparent)]
    Parse(#[from] ParseError),
}

pub fn convert_rtf_to_pdf(
    input: &[u8],
    options: &ConvertOptions,
) -> Result<ConversionOutput, ConvertError> {
    options.font_provider.validate()?;
    let parsed = parse_rtf_bytes_with_options(input, &options.parse_options)?;
    let mut diagnostics = if options.diagnostics {
        parsed.diagnostics.clone()
    } else {
        Vec::new()
    };
    if options.diagnostics {
        diagnostics.extend(passive_font_substitution_diagnostics(
            &parsed.document,
            &options.font_provider,
        ));
        diagnostics.extend(unsupported_passive_glyph_diagnostics(
            &parsed.document,
            &options.font_provider,
        ));
    }
    let layout =
        LayoutEngine::layout_with_font_provider(&parsed.document, Some(&options.font_provider));
    let page_count = layout.pages.len();
    let page_limit = options.parse_options.limits.max_pdf_pages;
    if page_count > page_limit {
        return Err(ConvertError::TooManyPages {
            pages: page_count,
            limit: page_limit,
        });
    }
    let pdf = render_pdf_with_font_provider(&layout, Some(&options.font_provider));
    let output_limit = options.parse_options.limits.max_pdf_output_bytes;
    if pdf.len() > output_limit {
        return Err(ConvertError::OutputTooLarge {
            size: pdf.len(),
            limit: output_limit,
        });
    }
    audit_passive_pdf_bytes(&pdf)?;

    Ok(ConversionOutput {
        pdf,
        diagnostics,
        pages: page_count,
    })
}

fn passive_font_substitution_diagnostics(
    document: &Document,
    font_provider: &FontProvider,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen = Vec::<(String, PdfFontFamily)>::new();
    let used_font_indexes = collect_visible_font_indexes(document);
    for font in &document.fonts {
        if !used_font_indexes.contains(&font.index) {
            continue;
        }
        let family = passive_pdf_font_family_for_font(font);
        if font_provider_has_asset_for_font(font_provider, font) {
            continue;
        }
        let key = (font.name.to_ascii_lowercase(), family);
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        let message = if font_name_matches_exact_pdf_base_font(&font.name, family) {
            continue;
        } else if font_name_matches_pdf_family(&font.name, family) {
            format!(
                "font '{}' approximated with passive PDF base font {}; provide a passive font asset for closer Word-compatible output",
                font.name,
                passive_pdf_font_family_label(family)
            )
        } else {
            format!(
                "font '{}' substituted with passive PDF base font {}",
                font.name,
                passive_pdf_font_family_label(family)
            )
        };
        diagnostics.push(Diagnostic::warning(message, None));
    }
    diagnostics
}

fn collect_visible_font_indexes(document: &Document) -> Vec<i32> {
    let mut indexes = Vec::new();
    collect_visible_font_indexes_from_blocks(&document.blocks, &mut indexes);
    for paragraphs in [
        &document.header,
        &document.first_page_header,
        &document.even_page_header,
        &document.footer,
        &document.first_page_footer,
        &document.even_page_footer,
        &document.footnotes,
        &document.endnotes,
    ] {
        collect_visible_font_indexes_from_paragraphs(paragraphs, &mut indexes);
    }
    for shapes in [
        &document.header_shapes,
        &document.first_page_header_shapes,
        &document.even_page_header_shapes,
        &document.footer_shapes,
        &document.first_page_footer_shapes,
        &document.even_page_footer_shapes,
        &document.background_shapes,
    ] {
        collect_visible_font_indexes_from_shapes(shapes, &mut indexes);
    }
    indexes
}

fn push_visible_font_index(indexes: &mut Vec<i32>, index: i32) {
    if !indexes.contains(&index) {
        indexes.push(index);
    }
}

fn collect_visible_font_indexes_from_blocks(blocks: &[Block], indexes: &mut Vec<i32>) {
    for block in blocks {
        match block {
            Block::Paragraph(paragraph) => {
                collect_visible_font_indexes_from_paragraph(paragraph, indexes)
            }
            Block::Table(table) => collect_visible_font_indexes_from_table(table, indexes),
            Block::Shape(shape) => collect_visible_font_indexes_from_shape(shape, indexes),
            Block::Image(_)
            | Block::Placeholder(_)
            | Block::PageBreak
            | Block::ColumnBreak
            | Block::ContinuousSectionBreak
            | Block::SectionBreak
            | Block::EvenPageSectionBreak
            | Block::OddPageSectionBreak
            | Block::SectionSettings(_) => {}
        }
    }
}

fn collect_visible_font_indexes_from_table(table: &Table, indexes: &mut Vec<i32>) {
    for row in &table.rows {
        for cell in &row.cells {
            collect_visible_font_indexes_from_paragraphs(&cell.paragraphs, indexes);
        }
    }
}

fn collect_visible_font_indexes_from_shapes(shapes: &[StaticShape], indexes: &mut Vec<i32>) {
    for shape in shapes {
        collect_visible_font_indexes_from_shape(shape, indexes);
    }
}

fn collect_visible_font_indexes_from_shape(shape: &StaticShape, indexes: &mut Vec<i32>) {
    collect_visible_font_indexes_from_paragraphs(&shape.text, indexes);
}

fn collect_visible_font_indexes_from_paragraphs(paragraphs: &[Paragraph], indexes: &mut Vec<i32>) {
    for paragraph in paragraphs {
        collect_visible_font_indexes_from_paragraph(paragraph, indexes);
    }
}

fn collect_visible_font_indexes_from_paragraph(paragraph: &Paragraph, indexes: &mut Vec<i32>) {
    for run in &paragraph.runs {
        if !run.text.is_empty() {
            push_visible_font_index(indexes, run.style.font_index);
        }
    }
}

fn unsupported_passive_glyph_diagnostics(
    document: &Document,
    font_provider: &FontProvider,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen = Vec::<(i32, &'static str)>::new();

    collect_unsupported_glyph_diagnostics_from_blocks(
        &document.blocks,
        document,
        font_provider,
        &mut seen,
        &mut diagnostics,
    );
    for paragraphs in [
        &document.header,
        &document.first_page_header,
        &document.even_page_header,
        &document.footer,
        &document.first_page_footer,
        &document.even_page_footer,
        &document.footnotes,
        &document.endnotes,
    ] {
        collect_unsupported_glyph_diagnostics_from_paragraphs(
            paragraphs,
            document,
            font_provider,
            &mut seen,
            &mut diagnostics,
        );
    }
    for shapes in [
        &document.header_shapes,
        &document.first_page_header_shapes,
        &document.even_page_header_shapes,
        &document.footer_shapes,
        &document.first_page_footer_shapes,
        &document.even_page_footer_shapes,
        &document.background_shapes,
    ] {
        collect_unsupported_glyph_diagnostics_from_shapes(
            shapes,
            document,
            font_provider,
            &mut seen,
            &mut diagnostics,
        );
    }

    diagnostics
}

fn collect_unsupported_glyph_diagnostics_from_blocks(
    blocks: &[Block],
    document: &Document,
    font_provider: &FontProvider,
    seen: &mut Vec<(i32, &'static str)>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for block in blocks {
        match block {
            Block::Paragraph(paragraph) => collect_unsupported_glyph_diagnostics_from_paragraph(
                paragraph,
                document,
                font_provider,
                seen,
                diagnostics,
            ),
            Block::Table(table) => collect_unsupported_glyph_diagnostics_from_table(
                table,
                document,
                font_provider,
                seen,
                diagnostics,
            ),
            Block::Shape(shape) => collect_unsupported_glyph_diagnostics_from_shape(
                shape,
                document,
                font_provider,
                seen,
                diagnostics,
            ),
            Block::Image(_)
            | Block::Placeholder(_)
            | Block::PageBreak
            | Block::ColumnBreak
            | Block::ContinuousSectionBreak
            | Block::SectionBreak
            | Block::EvenPageSectionBreak
            | Block::OddPageSectionBreak
            | Block::SectionSettings(_) => {}
        }
    }
}

fn collect_unsupported_glyph_diagnostics_from_table(
    table: &Table,
    document: &Document,
    font_provider: &FontProvider,
    seen: &mut Vec<(i32, &'static str)>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for row in &table.rows {
        for cell in &row.cells {
            collect_unsupported_glyph_diagnostics_from_paragraphs(
                &cell.paragraphs,
                document,
                font_provider,
                seen,
                diagnostics,
            );
        }
    }
}

fn collect_unsupported_glyph_diagnostics_from_shapes(
    shapes: &[StaticShape],
    document: &Document,
    font_provider: &FontProvider,
    seen: &mut Vec<(i32, &'static str)>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for shape in shapes {
        collect_unsupported_glyph_diagnostics_from_shape(
            shape,
            document,
            font_provider,
            seen,
            diagnostics,
        );
    }
}

fn collect_unsupported_glyph_diagnostics_from_shape(
    shape: &StaticShape,
    document: &Document,
    font_provider: &FontProvider,
    seen: &mut Vec<(i32, &'static str)>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_unsupported_glyph_diagnostics_from_paragraphs(
        &shape.text,
        document,
        font_provider,
        seen,
        diagnostics,
    );
}

fn collect_unsupported_glyph_diagnostics_from_paragraphs(
    paragraphs: &[Paragraph],
    document: &Document,
    font_provider: &FontProvider,
    seen: &mut Vec<(i32, &'static str)>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for paragraph in paragraphs {
        collect_unsupported_glyph_diagnostics_from_paragraph(
            paragraph,
            document,
            font_provider,
            seen,
            diagnostics,
        );
    }
}

fn collect_unsupported_glyph_diagnostics_from_paragraph(
    paragraph: &Paragraph,
    document: &Document,
    font_provider: &FontProvider,
    seen: &mut Vec<(i32, &'static str)>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for run in &paragraph.runs {
        collect_unsupported_glyph_diagnostic_from_run(
            run,
            document,
            font_provider,
            seen,
            diagnostics,
        );
    }
}

fn collect_unsupported_glyph_diagnostic_from_run(
    run: &Run,
    document: &Document,
    font_provider: &FontProvider,
    seen: &mut Vec<(i32, &'static str)>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((script, sample_char)) = unsupported_passive_glyph_script(&run.text) else {
        return;
    };
    let key = (run.style.font_index, script);
    if seen.contains(&key) {
        return;
    }
    seen.push(key);
    let Some(font) = document
        .fonts
        .iter()
        .find(|font| font.index == run.style.font_index)
    else {
        diagnostics.push(Diagnostic::warning(
            format!(
                "{} characters for font 'unknown' need passive font asset support; current PDF base-font fallback may render replacement glyphs",
                script
            ),
            None,
        ));
        return;
    };
    let font_name = font.name.as_str();
    let message = match font_provider_coverage_for_font_char(font_provider, font, sample_char) {
        FontCoverage::NoAsset => format!(
            "{} characters for font '{}' need passive font asset support; current PDF base-font fallback may render replacement glyphs",
            script, font_name
        ),
        FontCoverage::Covered => format!(
            "{} characters for font '{}' have a caller-provided passive font asset; covered glyphs can render through embedded passive Type0 fonts",
            script, font_name
        ),
        FontCoverage::MissingGlyph => format!(
            "{} characters for font '{}' are not covered by the caller-provided passive font asset; current PDF base-font fallback may render replacement glyphs",
            script, font_name
        ),
    };
    diagnostics.push(Diagnostic::warning(message, None));
}

fn font_provider_has_asset_for_font(font_provider: &FontProvider, font: &FontDef) -> bool {
    font_provider.has_asset_for_family(&font.name)
        || font
            .alternate_name
            .as_deref()
            .is_some_and(|alternate| font_provider.has_asset_for_family(alternate))
}

fn font_provider_coverage_for_font_char(
    font_provider: &FontProvider,
    font: &FontDef,
    ch: char,
) -> FontCoverage {
    let primary = font_provider.coverage_for_char(&font.name, ch);
    let alternate = font
        .alternate_name
        .as_deref()
        .map(|alternate| font_provider.coverage_for_char(alternate, ch));

    if primary == FontCoverage::Covered || alternate == Some(FontCoverage::Covered) {
        FontCoverage::Covered
    } else if primary == FontCoverage::MissingGlyph || alternate == Some(FontCoverage::MissingGlyph)
    {
        FontCoverage::MissingGlyph
    } else {
        FontCoverage::NoAsset
    }
}

fn unsupported_passive_glyph_script(text: &str) -> Option<(&'static str, char)> {
    text.chars()
        .find(|ch| is_cyrillic_char(*ch))
        .map(|ch| ("Cyrillic", ch))
}

fn is_cyrillic_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{0400}'..='\u{04ff}' | '\u{0500}'..='\u{052f}' | '\u{2de0}'..='\u{2dff}' | '\u{a640}'..='\u{a69f}'
    )
}

fn font_name_matches_pdf_family(name: &str, family: PdfFontFamily) -> bool {
    let normalized = direct_base14_alias_name(name);
    match family {
        PdfFontFamily::Helvetica => {
            matches!(
                normalized.as_str(),
                "helvetica" | "arial" | "ms sans serif" | "microsoft sans serif"
            ) || normalized.starts_with("helvetica ")
        }
        PdfFontFamily::Courier => {
            matches!(normalized.as_str(), "courier" | "courier new")
                || normalized.starts_with("courier ")
        }
        PdfFontFamily::Times => matches!(
            normalized.as_str(),
            "times-roman" | "times roman" | "times new roman" | "ms serif"
        ),
        PdfFontFamily::Symbol => matches!(normalized.as_str(), "symbol" | "symbol mt" | "symbolmt"),
        PdfFontFamily::ZapfDingbats => {
            matches!(
                normalized.as_str(),
                "zapfdingbats"
                    | "zapf dingbats"
                    | "wingdings"
                    | "wingdings 2"
                    | "wingdings2"
                    | "wingdings 3"
                    | "wingdings3"
                    | "webdings"
            )
        }
    }
}

fn font_name_matches_exact_pdf_base_font(name: &str, family: PdfFontFamily) -> bool {
    let normalized = direct_base14_alias_name(name);
    match family {
        PdfFontFamily::Helvetica => normalized == "helvetica",
        PdfFontFamily::Courier => normalized == "courier",
        PdfFontFamily::Times => matches!(normalized.as_str(), "times-roman" | "times roman"),
        PdfFontFamily::Symbol => normalized == "symbol",
        PdfFontFamily::ZapfDingbats => {
            matches!(normalized.as_str(), "zapfdingbats" | "zapf dingbats")
        }
    }
}

fn direct_base14_alias_name(name: &str) -> String {
    let normalized = name
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    strip_word_charset_suffix(&normalized).to_string()
}

fn strip_word_charset_suffix(name: &str) -> &str {
    for suffix in [" ce", " cyr", " greek", " tur", " baltic"] {
        if let Some(stripped) = name.strip_suffix(suffix) {
            return stripped;
        }
    }
    name
}

fn passive_pdf_font_family_label(family: PdfFontFamily) -> &'static str {
    match family {
        PdfFontFamily::Helvetica => "Helvetica",
        PdfFontFamily::Courier => "Courier",
        PdfFontFamily::Times => "Times-Roman",
        PdfFontFamily::Symbol => "Symbol",
        PdfFontFamily::ZapfDingbats => "ZapfDingbats",
    }
}

pub fn convert_rtf_bytes_to_pdf(
    input: &[u8],
    options: &ConvertOptions,
) -> Result<ConversionOutput, ConvertError> {
    convert_rtf_to_pdf(input, options)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn convert_rtf_file_to_pdf(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &ConvertOptions,
) -> Result<ConvertReport, ConvertError> {
    let input = std::fs::read(input).map_err(ConvertError::ReadInput)?;
    let converted = convert_rtf_bytes_to_pdf(&input, options)?;
    std::fs::write(output, converted.pdf).map_err(ConvertError::WriteOutput)?;

    Ok(ConvertReport {
        diagnostics: converted.diagnostics,
        pages: converted.pages,
    })
}
