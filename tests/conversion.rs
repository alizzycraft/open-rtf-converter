use std::fs;
#[cfg(not(feature = "cli"))]
use std::path::Path;

use lopdf::Document as PdfDocument;
#[cfg(not(feature = "cli"))]
use open_rtf_converter::ConvertReport;
#[cfg(feature = "cli")]
use open_rtf_converter::convert_rtf_file_to_pdf;
use open_rtf_converter::{
    ConvertError, ConvertOptions, ConvertOptionsError, FontAsset, FontAssetStyle, FontProvider,
    FontProviderLimits, LimitValidationError, ParseError, RtfLimits, RtfParseOptions,
    convert_rtf_to_pdf,
};
use tempfile::tempdir;

#[cfg(not(feature = "cli"))]
fn convert_rtf_file_to_pdf(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &ConvertOptions,
) -> Result<ConvertReport, ConvertError> {
    let input = fs::read(input).expect("test fixture should be readable");
    let converted = convert_rtf_to_pdf(&input, options)?;
    fs::write(output, &converted.pdf).expect("test output should be writable");
    Ok(ConvertReport {
        diagnostics: converted.diagnostics,
        pages: converted.pages,
    })
}

// Tests are grouped by domain; shared target support stays above.
#[path = "conversion/fixtures.rs"]
mod fixtures;
#[path = "conversion/fonts.rs"]
mod fonts;
#[path = "conversion/limits.rs"]
mod limits;
