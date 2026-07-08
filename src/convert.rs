#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use thiserror::Error;

use crate::config::RtfParseOptions;
use crate::diagnostics::Diagnostic;
use crate::layout::{LayoutEngine, PdfFontFamily, passive_pdf_font_family_for_font};
use crate::model::FontDef;
use crate::pdf::{PassivePdfError, audit_passive_pdf_bytes, render_pdf};
use crate::rtf::{ParseError, parse_rtf_bytes_with_options};

#[derive(Debug, Clone)]
pub struct ConvertOptions {
    pub diagnostics: bool,
    pub parse_options: RtfParseOptions,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            diagnostics: false,
            parse_options: RtfParseOptions::default(),
        }
    }
}

impl ConvertOptions {
    pub fn browser_safe_defaults() -> Self {
        Self {
            diagnostics: false,
            parse_options: RtfParseOptions::browser_safe_defaults(),
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
    Parse(#[from] ParseError),
}

pub fn convert_rtf_to_pdf(
    input: &[u8],
    options: &ConvertOptions,
) -> Result<ConversionOutput, ConvertError> {
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
    }
    let layout = LayoutEngine::layout(&parsed.document);
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
