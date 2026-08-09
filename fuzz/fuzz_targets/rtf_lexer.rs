#![no_main]

use libfuzzer_sys::fuzz_target;
use open_rtf_converter::{RtfLimits, fuzzing::exercise_lexer};

fuzz_target!(|data: &[u8]| {
    exercise_lexer(data, RtfLimits::browser_defaults());
});
