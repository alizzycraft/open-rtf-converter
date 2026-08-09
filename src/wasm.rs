use wasm_bindgen::prelude::*;

use crate::{ConvertOptions, convert_rtf_to_pdf};

/// Converts RTF bytes to passive PDF bytes using browser-safe defaults.
///
/// This narrow JavaScript boundary keeps parser, model, layout, and renderer
/// internals private while exposing the same validated and audited conversion
/// path as the supported Rust facade.
#[wasm_bindgen]
pub fn convert_rtf_to_pdf_browser(input: &[u8]) -> Result<Vec<u8>, JsValue> {
    convert_rtf_to_pdf(input, &ConvertOptions::browser_safe_defaults())
        .map(|output| output.pdf)
        .map_err(|error| JsValue::from_str(&error.to_string()))
}
