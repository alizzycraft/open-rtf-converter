#![no_main]

use libfuzzer_sys::fuzz_target;
use open_rtf_converter::pdf::audit_passive_pdf_bytes;
use open_rtf_converter::{ConvertOptions, RtfParseOptions, convert_rtf_to_pdf};

fuzz_target!(|data: &[u8]| {
    let options = ConvertOptions {
        parse_options: RtfParseOptions::browser_safe_defaults(),
        ..ConvertOptions::default()
    };

    if let Ok(output) = convert_rtf_to_pdf(data, &options) {
        audit_passive_pdf_bytes(&output.pdf).expect("converted PDF must remain passive");
    }
});
