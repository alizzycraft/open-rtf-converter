use std::collections::{HashMap, HashSet};

#[cfg(all(feature = "cli", not(target_arch = "wasm32")))]
use std::path::Path;

use thiserror::Error;

use crate::config::{LimitValidationError, RtfParseOptions};
use crate::diagnostics::Diagnostic;
use crate::fonts::{FontCoverage, FontProvider, FontProviderError};
use crate::layout::{
    LayoutDocument, LayoutEngine, LayoutItem, PdfFontFamily, TextFragment,
    passive_pdf_font_family_for_font,
};
use crate::model::{Document, FontDef, ImageFormat, StaticImageVectorCommand};
use crate::pdf::{
    PassivePdfError, audit_passive_pdf_bytes, estimate_passive_pdf_object_count,
    passive_base14_covers_latin_extended_char, render_pdf_with_font_provider,
};
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
            font_provider: FontProvider::browser_safe_defaults(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConvertOptionsError {
    #[error(transparent)]
    Limits(#[from] LimitValidationError),
    #[error(transparent)]
    Fonts(#[from] FontProviderError),
}

impl ConvertOptions {
    pub fn validate(&self) -> Result<(), ConvertOptionsError> {
        self.parse_options.validate()?;
        self.font_provider.validate()?;
        Ok(())
    }

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
    #[cfg(all(feature = "cli", not(target_arch = "wasm32")))]
    #[error("failed to read input: {0}")]
    ReadInput(#[source] std::io::Error),
    #[cfg(all(feature = "cli", not(target_arch = "wasm32")))]
    #[error("failed to write output: {0}")]
    WriteOutput(#[source] std::io::Error),
    #[error(transparent)]
    PdfSafety(#[from] PassivePdfError),
    #[error("rendered PDF output exceeded limit: {size} bytes > {limit} bytes")]
    OutputTooLarge { size: usize, limit: usize },
    #[error("rendered PDF layout item count exceeded limit: {items} items > {limit} items")]
    TooManyLayoutItems { items: usize, limit: usize },
    #[error("rendered PDF page count exceeded limit: {pages} pages > {limit} pages")]
    TooManyPages { pages: usize, limit: usize },
    #[error(
        "rendered PDF indirect object count exceeded limit: {objects} objects > {limit} objects"
    )]
    TooManyPdfObjects { objects: usize, limit: usize },
    #[error(transparent)]
    FontProvider(#[from] FontProviderError),
    #[error(transparent)]
    Parse(#[from] ParseError),
}

impl From<ConvertOptionsError> for ConvertError {
    fn from(error: ConvertOptionsError) -> Self {
        match error {
            ConvertOptionsError::Limits(error) => Self::Parse(ParseError::InvalidLimits(error)),
            ConvertOptionsError::Fonts(error) => Self::FontProvider(error),
        }
    }
}

pub fn convert_rtf_to_pdf(
    input: &[u8],
    options: &ConvertOptions,
) -> Result<ConversionOutput, ConvertError> {
    options.validate()?;
    let parsed = parse_rtf_bytes_with_options(input, &options.parse_options)?;
    let mut diagnostics = if options.diagnostics {
        parsed.diagnostics.clone()
    } else {
        Vec::new()
    };
    let layout =
        LayoutEngine::layout_with_font_provider(&parsed.document, Some(&options.font_provider));
    if options.diagnostics {
        diagnostics.extend(passive_font_substitution_diagnostics(
            &parsed.document,
            &layout,
            &options.font_provider,
        ));
        diagnostics.extend(unsupported_passive_glyph_diagnostics(
            &parsed.document,
            &layout,
            &options.font_provider,
        ));
    }
    let page_count = layout.pages.len();
    let layout_item_count = count_layout_items(&layout);
    let item_limit = options.parse_options.limits.max_pdf_layout_items;
    if layout_item_count > item_limit {
        return Err(ConvertError::TooManyLayoutItems {
            items: layout_item_count,
            limit: item_limit,
        });
    }
    let page_limit = options.parse_options.limits.max_pdf_pages;
    if page_count > page_limit {
        return Err(ConvertError::TooManyPages {
            pages: page_count,
            limit: page_limit,
        });
    }
    let object_count = estimate_passive_pdf_object_count(&layout, Some(&options.font_provider));
    let object_limit = options.parse_options.limits.max_pdf_indirect_objects;
    if object_count > object_limit {
        return Err(ConvertError::TooManyPdfObjects {
            objects: object_count,
            limit: object_limit,
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

fn count_layout_items(layout: &LayoutDocument) -> usize {
    layout.pages.iter().fold(0usize, |count, page| {
        page.items.iter().fold(count, |count, item| {
            count.saturating_add(count_layout_item(item))
        })
    })
}

fn count_layout_item(item: &LayoutItem) -> usize {
    let mut count = 0usize;
    let mut item = item;
    loop {
        count = count.saturating_add(1);
        match item {
            LayoutItem::Drawing(fragment) => {
                item = &fragment.item;
            }
            LayoutItem::Opacity { item: nested, .. }
            | LayoutItem::ClipRect { item: nested, .. } => {
                item = nested;
            }
            LayoutItem::Text(_)
            | LayoutItem::Highlight { .. }
            | LayoutItem::Underline { .. }
            | LayoutItem::Line { .. }
            | LayoutItem::CappedLine { .. }
            | LayoutItem::JoinedPolyline { .. }
            | LayoutItem::Ellipse { .. }
            | LayoutItem::RoundedRectangle { .. }
            | LayoutItem::Polygon { .. }
            | LayoutItem::Image(_) => return count,
        }
    }
}

fn passive_font_substitution_diagnostics(
    document: &Document,
    layout: &LayoutDocument,
    font_provider: &FontProvider,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen = HashSet::<(String, PdfFontFamily)>::new();
    let used_font_indexes = collect_visible_font_indexes(layout);
    for font in &document.fonts {
        if !used_font_indexes.contains(&font.index) {
            continue;
        }
        let family = passive_pdf_font_family_for_font(font);
        if font_provider_has_asset_for_font(font_provider, font) {
            continue;
        }
        let key = (font.name.to_ascii_lowercase(), family);
        if !seen.insert(key) {
            continue;
        }
        if font_name_matches_exact_pdf_base_font(&font.name, family) {
            continue;
        }
        let message = format!(
            "font '{}' rendered through bounded passive PDF base font {}; provide a passive font asset for closer Word-compatible metrics",
            font.name,
            passive_pdf_font_family_label(family)
        );
        diagnostics.push(Diagnostic::warning(message, None));
    }
    diagnostics
}

fn collect_visible_font_indexes(layout: &LayoutDocument) -> HashSet<i32> {
    let mut indexes = HashSet::new();
    layout.for_each_text_fragment(&mut |fragment| {
        if !fragment.text.is_empty() {
            indexes.insert(fragment.style.font_index);
        }
    });
    indexes
}

fn unsupported_passive_glyph_diagnostics(
    document: &Document,
    layout: &LayoutDocument,
    font_provider: &FontProvider,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen = HashSet::<(i32, &'static str)>::new();
    let fonts_by_index = document
        .fonts
        .iter()
        .map(|font| (font.index, font))
        .collect::<HashMap<_, _>>();

    layout.for_each_text_fragment(&mut |fragment| {
        collect_unsupported_glyph_diagnostic_from_fragment(
            fragment,
            &fonts_by_index,
            font_provider,
            &mut seen,
            &mut diagnostics,
        );
    });
    collect_unsupported_glyph_diagnostics_from_vector_images(layout, &mut seen, &mut diagnostics);

    diagnostics
}

fn collect_unsupported_glyph_diagnostic_from_fragment(
    fragment: &TextFragment,
    fonts_by_index: &HashMap<i32, &FontDef>,
    font_provider: &FontProvider,
    seen: &mut HashSet<(i32, &'static str)>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((script, sample_char)) = unsupported_passive_glyph_script(&fragment.text) else {
        return;
    };
    let key = (fragment.style.font_index, script);
    if !seen.insert(key) {
        return;
    }
    let Some(font) = fonts_by_index.get(&fragment.style.font_index).copied() else {
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
        FontCoverage::NoAsset
            if script == "Latin Extended"
                && fragment_latin_extended_is_covered_by_passive_base14(fragment) =>
        {
            format!(
                "{} characters for font '{}' render through bounded passive PDF base-font encoding; provide a passive font asset for closer Word-compatible font metrics",
                script, font_name
            )
        }
        FontCoverage::NoAsset => format!(
            "{} characters for font '{}' need passive font asset support; current PDF base-font fallback may render replacement glyphs",
            script, font_name
        ),
        FontCoverage::Covered => return,
        FontCoverage::MissingGlyph => format!(
            "{} characters for font '{}' are not covered by the caller-provided passive font asset; current PDF base-font fallback may render replacement glyphs",
            script, font_name
        ),
    };
    diagnostics.push(Diagnostic::warning(message, None));
}

fn fragment_latin_extended_is_covered_by_passive_base14(fragment: &TextFragment) -> bool {
    if matches!(
        fragment.font_family,
        PdfFontFamily::Symbol | PdfFontFamily::ZapfDingbats
    ) {
        return false;
    }
    let mut saw_latin_extended = false;
    for ch in fragment
        .text
        .chars()
        .filter(|ch| is_latin_extended_char(*ch))
    {
        saw_latin_extended = true;
        if !passive_base14_covers_latin_extended_char(ch) {
            return false;
        }
    }
    saw_latin_extended
}

fn collect_unsupported_glyph_diagnostics_from_vector_images(
    layout: &LayoutDocument,
    seen: &mut HashSet<(i32, &'static str)>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    layout.for_each_image_fragment(&mut |fragment| {
        if fragment.image.format != ImageFormat::WmfVector {
            return;
        }
        for command in &fragment.image.vector_commands {
            let StaticImageVectorCommand::Text { text, .. } = command else {
                continue;
            };
            let Some((script, _sample_char)) = unsupported_passive_glyph_script(text) else {
                continue;
            };
            let key = (i32::MIN, script);
            if !seen.insert(key) {
                continue;
            }
            diagnostics.push(Diagnostic::warning(
                format!(
                    "{} characters in passive WMF vector text need passive vector font support; current PDF base-font fallback may render replacement or mismatched glyphs",
                    script
                ),
                None,
            ));
        }
    });
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
    text.chars().find_map(|ch| {
        if is_cyrillic_char(ch) {
            Some(("Cyrillic", ch))
        } else if is_greek_char(ch) {
            Some(("Greek", ch))
        } else if is_latin_extended_char(ch) {
            Some(("Latin Extended", ch))
        } else {
            None
        }
    })
}

fn is_cyrillic_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{0400}'..='\u{04ff}' | '\u{0500}'..='\u{052f}' | '\u{2de0}'..='\u{2dff}' | '\u{a640}'..='\u{a69f}'
    )
}

fn is_latin_extended_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{0100}'..='\u{024f}' | '\u{1e00}'..='\u{1eff}'
    )
}

fn is_greek_char(ch: char) -> bool {
    matches!(ch, '\u{0370}'..='\u{03ff}' | '\u{1f00}'..='\u{1fff}')
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

#[cfg(all(feature = "cli", not(target_arch = "wasm32")))]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::DrawingFragment;
    use crate::model::{Block, Document, Paragraph, Run};

    #[test]
    fn default_option_sets_validate() {
        assert_eq!(ConvertOptions::default().validate(), Ok(()));
        assert_eq!(ConvertOptions::browser_safe_defaults().validate(), Ok(()));
    }

    #[test]
    fn option_validation_composes_limit_errors() {
        let mut options = ConvertOptions::default();
        options.parse_options.limits.max_font_size_half_points = 0;

        assert_eq!(
            options.validate(),
            Err(ConvertOptionsError::Limits(
                LimitValidationError::NonPositive {
                    field: "max_font_size_half_points",
                    value: 0,
                }
            ))
        );
    }

    #[test]
    fn option_validation_composes_font_errors() {
        let mut options = ConvertOptions::default();
        options.font_provider.assets[0].bytes.clear();

        assert_eq!(
            options.validate(),
            Err(ConvertOptionsError::Fonts(FontProviderError::EmptyAsset {
                asset_index: 0
            }))
        );
    }

    #[test]
    fn layout_item_limit_count_recurses_through_drawing_wrappers() {
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Nested drawing text".to_string(),
                style: Default::default(),
            }],
        })];
        let mut layout = LayoutEngine::layout(&document);
        let top_level_count = layout.pages[0].items.len();
        let item = layout.pages[0].items[0].clone();
        layout.pages[0].items[0] = LayoutItem::Drawing(DrawingFragment {
            z_order: 3,
            below_text: false,
            item: Box::new(LayoutItem::Drawing(DrawingFragment {
                z_order: 2,
                below_text: false,
                item: Box::new(item),
            })),
        });

        assert_eq!(layout.pages[0].items.len(), top_level_count);
        assert_eq!(count_layout_items(&layout), top_level_count + 2);
    }

    #[test]
    fn layout_item_limit_count_handles_deep_drawing_wrapper_chain_iteratively() {
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Deep drawing text".to_string(),
                style: Default::default(),
            }],
        })];
        let mut layout = LayoutEngine::layout(&document);
        let top_level_count = layout.pages[0].items.len();
        let wrapper_count = 512usize;
        let mut item = layout.pages[0].items[0].clone();
        for z_order in 0..wrapper_count {
            item = LayoutItem::Drawing(DrawingFragment {
                z_order: z_order as i32,
                below_text: false,
                item: Box::new(item),
            });
        }
        layout.pages[0].items[0] = item;

        assert_eq!(layout.pages[0].items.len(), top_level_count);
        assert_eq!(count_layout_items(&layout), top_level_count + wrapper_count);
    }
}
