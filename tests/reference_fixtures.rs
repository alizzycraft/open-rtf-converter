use std::path::Path;

use lopdf::Document as PdfDocument;
use lopdf::content::Content;
use open_rtf_converter::pdf::audit_passive_pdf_bytes;
use open_rtf_converter::{ConvertOptions, convert_rtf_to_pdf};

const MANIFEST: &str = include_str!("../fixtures/reference/expected-policy.json");

#[test]
fn word_reference_policy_manifest_covers_existing_visual_fixtures() {
    assert!(MANIFEST.contains("\"schema\": 1"));
    assert!(
        MANIFEST.contains("development references only"),
        "manifest should document that Word is not a production dependency"
    );

    for fixture in [
        "fixtures/simple.rtf",
        "fixtures/table-ish.rtf",
        "fixtures/weird.rtf",
        "fixtures/object-result.rtf",
        "fixtures/png-alpha.rtf",
        "fixtures/png-trns.rtf",
        "docs/sample.rtf",
    ] {
        assert!(
            Path::new(fixture).is_file(),
            "manifest references missing fixture {fixture}"
        );
        assert!(
            MANIFEST.contains(&format!("\"input\": \"{fixture}\"")),
            "manifest must classify {fixture}"
        );
    }

    for category in [
        "must_match_closely",
        "acceptable_approximation",
        "intentional_security_difference",
    ] {
        assert!(
            MANIFEST.contains(&format!("\"category\": \"{category}\"")),
            "manifest must include category {category}"
        );
    }

    assert!(
        MANIFEST.contains("\"word_reference_status\": \"pending_word_export\""),
        "current fixtures should explicitly mark missing Word references instead of implying coverage"
    );
    assert!(
        MANIFEST.contains("\"word_reference_pdf\": null"),
        "missing Word reference PDFs should be explicit"
    );
    assert!(
        MANIFEST.contains("\"intentional_security_differences\""),
        "security-sensitive fixtures must document intentional Word differences"
    );
    assert!(
        MANIFEST.contains("\"known_gaps\""),
        "visual fixtures must track missing comparison evidence"
    );
    assert_eq!(
        MANIFEST.matches("\"expected_diagnostics\"").count(),
        reference_fixtures().len(),
        "each manifest fixture should explicitly document expected diagnostics"
    );
    assert_eq!(
        MANIFEST.matches("\"forbidden_pdf_markers\"").count(),
        reference_fixtures().len(),
        "each manifest fixture should explicitly document source/control/PDF markers that must not reach PDF bytes"
    );
    assert_eq!(
        MANIFEST.matches("\"expected_pdf_markers\"").count(),
        reference_fixtures().len(),
        "each manifest fixture should explicitly document passive PDF markers required by the executable fixture gate"
    );
}

#[test]
fn reference_policy_fixtures_match_current_passive_converter_output() {
    for fixture in reference_fixtures() {
        let input = std::fs::read(fixture.input).unwrap_or_else(|error| {
            panic!(
                "failed to read reference fixture {}: {error}",
                fixture.input
            )
        });
        let output = convert_rtf_to_pdf(
            &input,
            &ConvertOptions {
                diagnostics: true,
                ..ConvertOptions::default()
            },
        )
        .unwrap_or_else(|error| panic!("failed to convert {}: {error}", fixture.input));

        assert_eq!(
            output.pages, fixture.expected_pages,
            "{} should render the expected page count",
            fixture.input
        );
        audit_passive_pdf_bytes(&output.pdf).unwrap_or_else(|error| {
            panic!("{} emitted active PDF content: {error}", fixture.input)
        });

        let pdf = PdfDocument::load_mem(&output.pdf)
            .unwrap_or_else(|error| panic!("{} emitted invalid PDF: {error}", fixture.input));
        assert_eq!(
            pdf.get_pages().len(),
            fixture.expected_pages,
            "{} PDF page tree should match report",
            fixture.input
        );
        let rendered_text = decoded_pdf_text(&pdf);
        for expected in fixture.must_preserve_text {
            assert!(
                rendered_text.contains(expected),
                "{} rendered PDF text did not contain {:?}; text was {:?}",
                fixture.input,
                expected,
                rendered_text
            );
        }
        for forbidden in fixture.must_not_leak {
            assert!(
                !output
                    .pdf
                    .windows(forbidden.len())
                    .any(|window| window == *forbidden),
                "{} leaked forbidden source/control bytes {:?}",
                fixture.input,
                String::from_utf8_lossy(forbidden)
            );
        }
        for expected in fixture.must_contain_pdf {
            assert!(
                output
                    .pdf
                    .windows(expected.len())
                    .any(|window| window == *expected),
                "{} rendered PDF did not contain expected passive marker {:?}",
                fixture.input,
                String::from_utf8_lossy(expected)
            );
        }
        for expected in fixture.must_emit_diagnostics {
            assert!(
                output
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "{} did not emit expected diagnostic {:?}; diagnostics were {:?}",
                fixture.input,
                expected,
                output.diagnostics
            );
        }
    }
}

struct ReferenceFixture {
    input: &'static str,
    expected_pages: usize,
    must_preserve_text: &'static [&'static str],
    must_not_leak: &'static [&'static [u8]],
    must_contain_pdf: &'static [&'static [u8]],
    must_emit_diagnostics: &'static [&'static str],
}

fn reference_fixtures() -> &'static [ReferenceFixture] {
    &[
        ReferenceFixture {
            input: "fixtures/simple.rtf",
            expected_pages: 2,
            must_preserve_text: &[
                "Hello from open-rtf-converter",
                "Centered paragraph with",
                "Second page text",
            ],
            must_not_leak: &[b"fonttbl", b"colortbl", b"/JavaScript", b"/EmbeddedFile"],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/table-ish.rtf",
            expected_pages: 1,
            must_preserve_text: &["Name", "Value", "Alpha", "Beta", "After table text"],
            must_not_leak: &[b"trowd", b"cellx", b"/JavaScript", b"/EmbeddedFile"],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/weird.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "visible text should survive",
                "Escaped braces: {sample}",
                "hex: ABC",
            ],
            must_not_leak: &[
                b"unknownDestination",
                b"madeup123",
                b"/JavaScript",
                b"/EmbeddedFile",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/object-result.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "Before object result.",
                "visible fallback",
                "After object result.",
            ],
            must_not_leak: &[
                b"objemb",
                b"objdata",
                b"414243",
                b"JavaScript",
                b"EmbeddedFile",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/png-alpha.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before alpha image.", "After alpha image."],
            must_not_leak: &[
                b"pngblip",
                b"IHDR",
                b"IDAT",
                b"IEND",
                b"/JavaScript",
                b"/EmbeddedFile",
            ],
            must_contain_pdf: &[b"/SMask"],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/png-trns.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "Before indexed transparency image.",
                "After indexed transparency image.",
            ],
            must_not_leak: &[
                b"pngblip",
                b"IHDR",
                b"PLTE",
                b"tRNS",
                b"IDAT",
                b"IEND",
                b"/JavaScript",
                b"/EmbeddedFile",
            ],
            must_contain_pdf: &[b"/SMask"],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "docs/sample.rtf",
            expected_pages: 2,
            must_preserve_text: &[
                "It is an example test rtf-file to RTF2XML bean for testing",
                "Simple table",
                "Here are some special characters",
                "At last you can see an image",
            ],
            must_not_leak: &[
                b"objdata",
                b"Word.Picture.8",
                b"METAFILEPICT",
                b"shppict",
                b"shprslt",
                b"wmetafile8",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "rendering shape picture result with bounded passive shape frame",
                "ignoring duplicate embedded object alternate after passive shape result",
                "active content removed: object metadata in skipped destination",
                "ignoring duplicate shape result fallback after passive primary shape result",
                "Latin Extended characters for font 'Times New Roman CE'",
            ],
        },
    ]
}

fn decoded_pdf_text(pdf: &PdfDocument) -> String {
    let mut output = String::new();
    for page_id in pdf.get_pages().values() {
        let content = pdf
            .get_and_decode_page_content(*page_id)
            .expect("page content should decode");
        output.push_str(&content_text(&content));
        output.push('\n');
    }
    output
}

fn content_text(content: &Content) -> String {
    let mut text = String::new();
    for operation in &content.operations {
        match operation.operator.as_ref() {
            "Tj" | "'" | "\"" => {
                for operand in &operation.operands {
                    if let Ok(bytes) = operand.as_str() {
                        text.push_str(&decode_pdf_text_bytes(bytes));
                    }
                }
            }
            "TJ" => {
                for operand in &operation.operands {
                    if let Ok(items) = operand.as_array() {
                        for item in items {
                            if let Ok(bytes) = item.as_str() {
                                text.push_str(&decode_pdf_text_bytes(bytes));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    text
}

fn decode_pdf_text_bytes(bytes: &[u8]) -> String {
    if bytes_look_like_utf16be_cids(bytes) {
        let utf16 = bytes
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        if let Ok(decoded) = String::from_utf16(&utf16) {
            return decoded;
        }
    }
    String::from_utf8_lossy(bytes).into_owned()
}

fn bytes_look_like_utf16be_cids(bytes: &[u8]) -> bool {
    if bytes.len() < 2 || bytes.len() % 2 != 0 {
        return false;
    }
    let chunks = bytes.len() / 2;
    let zero_high_bytes = bytes.chunks_exact(2).filter(|chunk| chunk[0] == 0).count();
    zero_high_bytes * 2 >= chunks
}
