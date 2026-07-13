#![no_main]

use libfuzzer_sys::fuzz_target;
use open_rtf_converter::RtfParseOptions;
use open_rtf_converter::rtf::parse_rtf_bytes_with_options;

fuzz_target!(|data: &[u8]| {
    let options = RtfParseOptions::browser_safe_defaults();
    let _ = parse_rtf_bytes_with_options(data, &options);
});
