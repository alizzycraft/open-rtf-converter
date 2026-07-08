use std::fs;

use lopdf::Document as PdfDocument;
use open_rtf_converter::{
    ConvertError, ConvertOptions, FontAsset, FontAssetStyle, FontProvider, FontProviderLimits,
    RtfLimits, RtfParseOptions, convert_rtf_file_to_pdf, convert_rtf_to_pdf,
};
use tempfile::tempdir;

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
fn browser_safe_defaults_use_stricter_pdf_output_limit() {
    assert_eq!(RtfLimits::default().max_pdf_output_bytes, 100 * 1024 * 1024);
    assert_eq!(
        ConvertOptions::browser_safe_defaults()
            .parse_options
            .limits
            .max_pdf_output_bytes,
        20 * 1024 * 1024
    );
    assert_eq!(
        ConvertOptions::browser_safe_defaults()
            .font_provider
            .limits
            .max_asset_bytes,
        2 * 1024 * 1024
    );
}

#[test]
fn conversion_rejects_pdf_output_over_configured_limit() {
    let input = br"{\rtf1\ansi Oversized PDF guard\par}";
    let error = convert_rtf_to_pdf(
        input,
        &ConvertOptions {
            diagnostics: true,
            parse_options: RtfParseOptions {
                limits: RtfLimits {
                    max_pdf_output_bytes: 1,
                    ..RtfLimits::default()
                },
                ..RtfParseOptions::default()
            },
            ..ConvertOptions::default()
        },
    )
    .expect_err("rendered PDF should exceed one byte");

    assert!(matches!(
        error,
        ConvertError::OutputTooLarge { size, limit } if limit == 1 && size > limit
    ));
}

#[test]
fn conversion_rejects_font_assets_over_configured_limits() {
    let input = br"{\rtf1\ansi Font asset limit\par}";
    let error = convert_rtf_to_pdf(
        input,
        &ConvertOptions {
            font_provider: FontProvider {
                assets: vec![FontAsset {
                    family_names: vec!["Bounded Font".to_string()],
                    style: FontAssetStyle::default(),
                    bytes: vec![0; 4],
                }],
                limits: FontProviderLimits {
                    max_asset_bytes: 3,
                    ..FontProviderLimits::default()
                },
            },
            ..ConvertOptions::default()
        },
    )
    .unwrap_err();

    assert!(matches!(
        error,
        ConvertError::FontProvider(open_rtf_converter::FontProviderError::AssetTooLarge {
            size: 4,
            limit: 3,
            ..
        })
    ));
}

#[test]
fn valid_memory_font_assets_report_coverage_and_metrics_without_system_fonts() {
    let provider = FontProvider {
        assets: vec![FontAsset {
            family_names: vec!["Tuffy".to_string()],
            style: FontAssetStyle::default(),
            bytes: include_bytes!("../fixtures/fonts/Tuffy.ttf").to_vec(),
        }],
        limits: FontProviderLimits {
            max_asset_bytes: 256 * 1024,
            max_total_bytes: 256 * 1024,
            ..FontProviderLimits::default()
        },
    };

    provider.validate().unwrap();
    assert_eq!(
        provider.coverage_for_char("tuffy", 'A'),
        open_rtf_converter::FontCoverage::Covered
    );
    assert_eq!(
        provider.coverage_for_char("Tuffy", '\u{10ffff}'),
        open_rtf_converter::FontCoverage::MissingGlyph
    );
    assert_eq!(
        provider.coverage_for_char("Missing", 'A'),
        open_rtf_converter::FontCoverage::NoAsset
    );

    let metrics = provider.glyph_metrics_for_char("Tuffy", 'A').unwrap();
    assert!(metrics.units_per_em > 0);
    assert!(metrics.advance_units > 0);
    assert!(metrics.ascender_units > 0);
    assert!(metrics.descender_units < 0);
    assert!(metrics.advance_points(12.0) > 0.0);
    assert!(
        provider
            .glyph_metrics_for_char("Tuffy", '\u{10ffff}')
            .is_none()
    );
}

#[test]
fn conversion_audits_pdf_syntax_without_rejecting_visible_active_words() {
    let input = br"{\rtf1\ansi Visible /JavaScript /Launch /URI /Annots /Widget text\par}";
    let output = convert_rtf_to_pdf(input, &ConvertOptions::browser_safe_defaults()).unwrap();

    assert_eq!(output.pages, 1);
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
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
