#![no_main]

use libfuzzer_sys::fuzz_target;
use open_rtf_converter::RtfLimits;
use open_rtf_converter::rtf::Lexer;

fuzz_target!(|data: &[u8]| {
    let _ = Lexer::new(data, RtfLimits::browser_defaults()).tokenize();
});
