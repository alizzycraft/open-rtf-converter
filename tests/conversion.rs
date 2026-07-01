use std::fs;

use lopdf::Document as PdfDocument;
use open_rtf_converter::{ConvertOptions, convert_rtf_to_pdf};
use tempfile::tempdir;

#[test]
fn converts_simple_fixture_to_valid_two_page_pdf() {
    let dir = tempdir().unwrap();
    let output = dir.path().join("simple.pdf");

    let report = convert_rtf_to_pdf(
        "fixtures/simple.rtf",
        &output,
        &ConvertOptions { diagnostics: true },
    )
    .unwrap();

    assert_eq!(report.pages, 2);
    let metadata = fs::metadata(&output).unwrap();
    assert!(metadata.len() > 500);

    let pdf = PdfDocument::load(&output).unwrap();
    assert_eq!(pdf.get_pages().len(), 2);
}

#[test]
fn weird_fixture_warns_but_still_converts() {
    let dir = tempdir().unwrap();
    let output = dir.path().join("weird.pdf");

    let report = convert_rtf_to_pdf(
        "fixtures/weird.rtf",
        &output,
        &ConvertOptions { diagnostics: true },
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

    convert_rtf_to_pdf(
        "fixtures/table-ish.rtf",
        &output,
        &ConvertOptions { diagnostics: true },
    )
    .unwrap();

    assert!(PdfDocument::load(&output).is_ok());
}
