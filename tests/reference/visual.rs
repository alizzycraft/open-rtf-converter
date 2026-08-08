use super::*;

#[test]
fn reference_policy_fixtures_match_current_passive_converter_output() {
    for fixture in manifest_reference_fixtures() {
        let input = std::fs::read(&fixture.input).unwrap_or_else(|error| {
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
        for expected in &fixture.must_preserve_text {
            assert!(
                rendered_text.contains(expected),
                "{} rendered PDF text did not contain {:?}; text was {:?}",
                fixture.input,
                expected,
                rendered_text
            );
        }
        for forbidden in &fixture.must_not_leak {
            let forbidden = forbidden.as_bytes();
            assert!(
                !output
                    .pdf
                    .windows(forbidden.len())
                    .any(|window| window == forbidden),
                "{} leaked forbidden source/control bytes {:?}",
                fixture.input,
                String::from_utf8_lossy(forbidden)
            );
        }
        for expected in &fixture.must_contain_pdf {
            let expected = expected.as_bytes();
            assert!(
                output
                    .pdf
                    .windows(expected.len())
                    .any(|window| window == expected),
                "{} rendered PDF did not contain expected passive marker {:?}",
                fixture.input,
                String::from_utf8_lossy(expected)
            );
        }
        for expected in &fixture.must_emit_diagnostics {
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
