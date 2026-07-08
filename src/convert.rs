#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use thiserror::Error;

use crate::config::RtfParseOptions;
use crate::diagnostics::Diagnostic;
use crate::fonts::{FontCoverage, FontProvider, FontProviderError};
use crate::layout::{LayoutEngine, PdfFontFamily, passive_pdf_font_family_for_font};
use crate::model::{Block, Document, FontDef, Paragraph, Run, StaticShape, Table};
use crate::pdf::{PassivePdfError, audit_passive_pdf_bytes, render_pdf};
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
            &parsed.document.fonts,
        ));
        diagnostics.extend(unsupported_passive_glyph_diagnostics(
            &parsed.document,
            &options.font_provider,
        ));
    }
    let layout =
        LayoutEngine::layout_with_font_provider(&parsed.document, Some(&options.font_provider));
    let page_count = layout.pages.len();
    let pdf = render_pdf(&layout);
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

fn passive_font_substitution_diagnostics(fonts: &[FontDef]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen = Vec::<(String, PdfFontFamily)>::new();
    for font in fonts {
        let family = passive_pdf_font_family_for_font(font);
        if font_name_matches_pdf_family(&font.name, family) {
            continue;
        }
        let key = (font.name.to_ascii_lowercase(), family);
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        diagnostics.push(Diagnostic::warning(
            format!(
                "font '{}' substituted with passive PDF base font {}",
                font.name,
                passive_pdf_font_family_label(family)
            ),
            None,
        ));
    }
    diagnostics
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
    let font_name = document
        .fonts
        .iter()
        .find(|font| font.index == run.style.font_index)
        .map(|font| font.name.as_str())
        .unwrap_or("unknown");
    let message = match font_provider.coverage_for_char(font_name, sample_char) {
        FontCoverage::NoAsset => format!(
            "{} characters for font '{}' need passive font asset support; current PDF base-font fallback may render replacement glyphs",
            script, font_name
        ),
        FontCoverage::Covered => format!(
            "{} characters for font '{}' have a parsed caller-provided passive font asset, but embedded font rendering is not implemented yet; current PDF base-font fallback may render replacement glyphs",
            script, font_name
        ),
        FontCoverage::MissingGlyph => format!(
            "{} characters for font '{}' are not covered by the caller-provided passive font asset; current PDF base-font fallback may render replacement glyphs",
            script, font_name
        ),
    };
    diagnostics.push(Diagnostic::warning(message, None));
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
    let normalized = name.to_ascii_lowercase();
    match family {
        PdfFontFamily::Helvetica => {
            normalized == "helvetica" || normalized.starts_with("helvetica ")
        }
        PdfFontFamily::Courier => normalized == "courier" || normalized.starts_with("courier "),
        PdfFontFamily::Times => normalized == "times-roman" || normalized == "times roman",
        PdfFontFamily::Symbol => normalized == "symbol",
        PdfFontFamily::ZapfDingbats => {
            normalized == "zapfdingbats" || normalized == "zapf dingbats"
        }
    }
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
