#![warn(missing_docs)]

//! Convert RTF bytes into passive PDF bytes without invoking an office suite,
//! browser print engine, system font service, filesystem API, or network API in
//! the conversion core.
//!
//! The supported entry point is [`convert_rtf_to_pdf`] (or its equivalent
//! [`convert_rtf_bytes_to_pdf`] name). Call [`inspect_rtf`] when a caller needs
//! only the document's bounded, deterministic font-family projection.
//! [`ConvertOptions::browser_safe_defaults`] applies the stricter resource limits
//! intended for browser/WASM workers. Both normal and browser-safe conversion
//! remove, placeholder, or reject active document behavior according to the
//! configured policy, limit the generated layout/PDF, and audit the completed PDF
//! for forbidden active features.
//!
//! Parser, normalized-model, layout, and raw-renderer modules are crate-private.
//! The repository's fuzz crate uses only payload-opaque hooks behind the
//! non-default `fuzzing` feature; those hooks are unstable and hidden from
//! generated documentation.
//!
//! # Example
//!
//! ```
//! use open_rtf_converter::{ConvertOptions, audit_passive_pdf_bytes, convert_rtf_to_pdf};
//!
//! let input = br"{\rtf1\ansi Passive in-memory conversion\par}";
//! let options = ConvertOptions::browser_safe_defaults();
//! let output = convert_rtf_to_pdf(input, &options)?;
//!
//! assert_eq!(output.pages, 1);
//! assert!(output.pdf.starts_with(b"%PDF-"));
//! audit_passive_pdf_bytes(&output.pdf)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod config;
mod convert;
mod diagnostics;
mod fonts;
mod inspect;
mod layout;
mod model;
mod pdf;
mod rtf;
#[cfg(target_arch = "wasm32")]
mod wasm;

/// Unstable, non-default entry points used only by the repository's fuzz crate.
#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub mod fuzzing {
    use crate::rtf::{Lexer, parse_rtf_bytes_with_options};
    use crate::{RtfLimits, RtfParseOptions};

    /// Exercises bounded tokenization without exposing lexer tokens or errors.
    pub fn exercise_lexer(input: &[u8], limits: RtfLimits) {
        let _ = Lexer::new(input, limits).tokenize();
    }

    /// Exercises bounded parsing without exposing the normalized model.
    pub fn exercise_parser(input: &[u8], options: &RtfParseOptions) {
        let _ = parse_rtf_bytes_with_options(input, options);
    }
}

pub use config::{
    ActiveContentPolicy, CompatibilityMode, LimitValidationError, PdfLinkPolicy, RtfLimits,
    RtfParseOptions,
};
#[cfg(all(feature = "cli", not(target_arch = "wasm32")))]
pub use convert::convert_rtf_file_to_pdf;
pub use convert::{
    ConversionOutput, ConvertError, ConvertOptions, ConvertOptionsError, ConvertReport, Converter,
    convert_rtf_bytes_to_pdf, convert_rtf_to_pdf,
};
pub use diagnostics::{Diagnostic, Severity};
pub use fonts::{
    FontAsset, FontAssetStyle, FontCoverage, FontGlyphMetrics, FontProvider, FontProviderError,
    FontProviderLimits,
};
pub use inspect::{InspectError, RtfInspection, inspect_rtf};
pub use pdf::{PassivePdfError, PassivePdfIssue, PdfRenderError, audit_passive_pdf_bytes};
pub use rtf::ParseError;

#[cfg(test)]
extern crate self as open_rtf_converter;
#[cfg(test)]
mod model_invariant_tests;
#[cfg(test)]
mod security_tests;
