use super::*;

#[test]
fn converts_simple_fixture_to_valid_two_page_pdf() {
    let dir = tempdir().unwrap();
    let output = dir.path().join("simple.pdf");

    let report = convert_rtf_file_to_pdf(
        "fixtures/simple.rtf",
        &output,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();

    assert_eq!(report.pages, 2);
    let metadata = fs::metadata(&output).unwrap();
    assert!(metadata.len() > 500);

    let pdf = PdfDocument::load(&output).unwrap();
    assert_eq!(pdf.get_pages().len(), 2);
}

#[test]
fn converts_rtf_bytes_to_pdf_without_filesystem_core_api() {
    let input = br"{\rtf1\ansi In-memory conversion\par}";
    let output = convert_rtf_to_pdf(input, &ConvertOptions::browser_safe_defaults()).unwrap();

    assert_eq!(output.pages, 1);
    assert!(output.pdf.len() > 500);
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
}

#[test]
fn word_compatible_extreme_line_spacing_stays_on_one_page() {
    let input = fs::read("fixtures/line-spacing-passive.rtf").unwrap();
    let output = convert_rtf_to_pdf(&input, &ConvertOptions::default()).unwrap();

    assert_eq!(output.pages, 1);
}

#[test]
fn weird_fixture_warns_but_still_converts() {
    let dir = tempdir().unwrap();
    let output = dir.path().join("weird.pdf");

    let report = convert_rtf_file_to_pdf(
        "fixtures/weird.rtf",
        &output,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();

    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("madeup"))
    );
    assert!(PdfDocument::load(&output).is_ok());
}

#[test]
fn table_like_fixture_degrades_to_valid_text_pdf() {
    let dir = tempdir().unwrap();
    let output = dir.path().join("table-ish.pdf");

    convert_rtf_file_to_pdf(
        "fixtures/table-ish.rtf",
        &output,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();

    assert!(PdfDocument::load(&output).is_ok());
}
