#![no_main]

use libfuzzer_sys::fuzz_target;
use open_rtf_converter::{RtfParseOptions, fuzzing::exercise_parser};

fuzz_target!(|data: &[u8]| {
    let options = RtfParseOptions::browser_safe_defaults();
    exercise_parser(data, &options);
});
