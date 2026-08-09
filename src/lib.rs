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
//! Lower parser, normalized-model, layout, and raw-renderer modules remain
//! temporarily accessible for repository migration and fuzzing, but they are
//! hidden from generated documentation and are not the supported conversion
//! security boundary.
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

#[doc(hidden)]
pub mod config;
#[doc(hidden)]
pub mod convert;
#[doc(hidden)]
pub mod diagnostics;
#[doc(hidden)]
pub mod fonts;
mod inspect;
#[doc(hidden)]
pub mod layout;
#[doc(hidden)]
pub mod model;
#[doc(hidden)]
pub mod pdf;
#[doc(hidden)]
pub mod rtf;

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
pub use pdf::{PassivePdfError, PassivePdfIssue, audit_passive_pdf_bytes};
pub use rtf::ParseError;
