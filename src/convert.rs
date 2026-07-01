use std::path::Path;

use fontdb::Database;
use thiserror::Error;

use crate::diagnostics::Diagnostic;
use crate::layout::LayoutEngine;
use crate::pdf::render_pdf;
use crate::rtf::{ParseError, parse_rtf};

#[derive(Debug, Clone, Default)]
pub struct ConvertOptions {
    pub diagnostics: bool,
}

#[derive(Debug)]
pub struct ConvertReport {
    pub diagnostics: Vec<Diagnostic>,
    pub pages: usize,
}

#[derive(Debug, Error)]
pub enum ConvertError {
    #[error("failed to read input: {0}")]
    ReadInput(#[source] std::io::Error),
    #[error("failed to write output: {0}")]
    WriteOutput(#[source] std::io::Error),
    #[error(transparent)]
    Parse(#[from] ParseError),
}

pub fn convert_rtf_to_pdf(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &ConvertOptions,
) -> Result<ConvertReport, ConvertError> {
    let input = std::fs::read_to_string(input).map_err(ConvertError::ReadInput)?;
    let parsed = parse_rtf(&input)?;

    let mut font_db = Database::new();
    font_db.load_system_fonts();
    drop(font_db);

    let layout = LayoutEngine::layout(&parsed.document);
    let page_count = layout.pages.len();
    let pdf = render_pdf(&layout);
    std::fs::write(output, pdf).map_err(ConvertError::WriteOutput)?;

    Ok(ConvertReport {
        diagnostics: if options.diagnostics {
            parsed.diagnostics
        } else {
            Vec::new()
        },
        pages: page_count,
    })
}
