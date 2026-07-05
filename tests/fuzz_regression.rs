use std::panic;

use open_rtf_converter::pdf::audit_passive_pdf_bytes;
use open_rtf_converter::rtf::{Lexer, parse_rtf_bytes_with_options};
use open_rtf_converter::{ConvertOptions, RtfLimits, RtfParseOptions, convert_rtf_to_pdf};

#[test]
fn tokenizer_and_parser_mutation_corpus_return_typed_results_without_panics() {
    let parse_options = RtfParseOptions::browser_safe_defaults();

    for (case_idx, case) in mutation_corpus().into_iter().enumerate() {
        let lex_result =
            panic::catch_unwind(|| Lexer::new(&case, RtfLimits::browser_defaults()).tokenize());
        assert!(
            lex_result.is_ok(),
            "lexer panicked for mutation corpus case {case_idx}: {:?}",
            String::from_utf8_lossy(&case)
        );

        let parse_result =
            panic::catch_unwind(|| parse_rtf_bytes_with_options(&case, &parse_options));
        assert!(
            parse_result.is_ok(),
            "parser panicked for mutation corpus case {case_idx}: {:?}",
            String::from_utf8_lossy(&case)
        );
    }
}

#[test]
fn converter_mutation_corpus_rejects_or_emits_passive_pdf_without_payload_leakage() {
    let options = ConvertOptions {
        diagnostics: true,
        parse_options: RtfParseOptions::browser_safe_defaults(),
    };

    for (case_idx, case) in mutation_corpus().into_iter().enumerate() {
        let convert_result = panic::catch_unwind(|| convert_rtf_to_pdf(&case, &options));
        assert!(
            convert_result.is_ok(),
            "converter panicked for mutation corpus case {case_idx}: {:?}",
            String::from_utf8_lossy(&case)
        );

        let Ok(output) = convert_result.expect("panic checked") else {
            continue;
        };

        audit_passive_pdf_bytes(&output.pdf).expect("converter output must remain passive");
        for forbidden in [
            b"objdata".as_slice(),
            b"objocx",
            b"414243",
            b"pFragments",
            b"calc.exe",
            b"launch.exe",
            b"https://example.com/payload",
            b"/JavaScript",
            b"/EmbeddedFile",
            b"/Launch",
            b"/OpenAction",
            b"/RichMedia",
        ] {
            assert!(
                !output
                    .pdf
                    .windows(forbidden.len())
                    .any(|window| window == forbidden),
                "forbidden payload leaked to PDF for mutation corpus case {case_idx}: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

fn mutation_corpus() -> Vec<Vec<u8>> {
    let mut cases = Vec::new();
    for seed in seed_corpus() {
        for mutated in mutations(&seed) {
            cases.push(mutated);
        }
    }
    cases
}

fn mutations(seed: &[u8]) -> Vec<Vec<u8>> {
    let mut cases = vec![
        seed.to_vec(),
        with_prefix(seed, b"{"),
        with_suffix(seed, b"}"),
        with_suffix(seed, b"\\bin5 abc"),
        with_suffix(seed, b"{\\*\\unknown{\\object\\objdata 414243}}"),
    ];

    if !seed.is_empty() {
        let mid = seed.len() / 2;
        let mut split_control = Vec::with_capacity(seed.len() + 18);
        split_control.extend_from_slice(&seed[..mid]);
        split_control.extend_from_slice(b"{\\*\\unknown ");
        split_control.extend_from_slice(&seed[mid..]);
        split_control.extend_from_slice(b"}");
        cases.push(split_control);
    }

    cases
}

fn with_prefix(seed: &[u8], prefix: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(prefix.len() + seed.len());
    output.extend_from_slice(prefix);
    output.extend_from_slice(seed);
    output
}

fn with_suffix(seed: &[u8], suffix: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(seed.len() + suffix.len());
    output.extend_from_slice(seed);
    output.extend_from_slice(suffix);
    output
}

fn seed_corpus() -> Vec<Vec<u8>> {
    vec![
        rtf(&["{", "\\", "rtf1 hello}"]),
        rtf(&["{", "\\", "rtf1{", "\\", "b bold}}"]),
        rtf(&["{", "\\", "rtf1 ", "\\", "u8217?}"]),
        rtf(&["{", "\\", "rtf1", "\\", "bin3 abc}"]),
        rtf(&["{", "\\", "rtf1", "\\", "'41", "\\", "'42", "\\", "'43}"]),
        rtf(&["{", "\\", "rtf1", "\\", "'GZ}"]),
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "*",
            "\\",
            "unknown ignored} visible}",
        ]),
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "fonttbl{",
            "\\",
            "f0 Arial;}}Hello}",
        ]),
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "colortbl;",
            "\\",
            "red255",
            "\\",
            "green0",
            "\\",
            "blue0;}Hello}",
        ]),
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "object",
            "\\",
            "objocx",
            "\\",
            "objdata 414243}}",
        ]),
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "object{",
            "\\",
            "objdata 414243}{",
            "\\",
            "result visible fallback}}}",
        ]),
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "field{",
            "\\",
            "*",
            "\\",
            "fldinst HYPERLINK \"https://example.com/payload\"}{",
            "\\",
            "fldrslt visible link}}}",
        ]),
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "field{",
            "\\",
            "*",
            "\\",
            "fldinst QUOTE \"mixed Case\" \\\\* Lower \\\\* FirstCap}}}",
        ]),
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "field{",
            "\\",
            "*",
            "\\",
            "fldinst SEQ Figure \\\\r 4 \\\\* ROMAN}} {",
            "\\",
            "field{",
            "\\",
            "*",
            "\\",
            "fldinst = 27 \\\\* alphabetic}}}",
        ]),
        rtf(&[
            "{",
            "\\",
            "rtf1 visible{",
            "\\",
            "annotation hidden {",
            "\\",
            "object",
            "\\",
            "objdata 414243}}}",
        ]),
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "*",
            "\\",
            "unknown{",
            "\\",
            "object",
            "\\",
            "objdata 414243}}}",
        ]),
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "shp{",
            "\\",
            "*",
            "\\",
            "shpinst{",
            "\\",
            "sp{",
            "\\",
            "sn pFragments}{",
            "\\",
            "sv calc.exe}}}} visible}",
        ]),
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "field{",
            "\\",
            "*",
            "\\",
            "fldinst MACROBUTTON launch.exe Click}{",
            "\\",
            "fldrslt Safe caption}}}",
        ]),
        b"{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{".to_vec(),
        rtf(&["{", "\\", "rtf1", "\\", "bin999999999999999999999 abc}"]),
        rtf(&["{", "\\", "rtf1", "\\", "bin-1 abc}"]),
        rtf(&[
            "{",
            "\\",
            "rtf1",
            "\\",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa text}",
        ]),
    ]
}

fn rtf(parts: &[&str]) -> Vec<u8> {
    let mut output = Vec::new();
    for part in parts {
        output.extend_from_slice(part.as_bytes());
    }
    output
}
