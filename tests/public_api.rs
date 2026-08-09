use std::error::Error;

use open_rtf_converter::{
    ActiveContentPolicy, CompatibilityMode, ConvertError, ConvertOptions, ConvertOptionsError,
    Converter, InspectError, LimitValidationError, ParseError, PassivePdfError, PdfLinkPolicy,
    RtfInspection, RtfLimits, RtfParseOptions, audit_passive_pdf_bytes, convert_rtf_bytes_to_pdf,
    convert_rtf_to_pdf, inspect_rtf,
};

fn assert_public_error<E: Error>() {}
fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn reusable_converter_preserves_output_and_is_send_sync() {
    assert_send_sync::<Converter>();
    let input = br"{\rtf1\ansi Reusable converter\par}";
    let options = ConvertOptions::browser_safe_defaults();
    let expected = convert_rtf_to_pdf(input, &options).unwrap();
    let converter = Converter::new(options).unwrap();

    let first = converter.convert(input).unwrap();
    let second = converter.convert(input).unwrap();
    assert_eq!(first.pdf, expected.pdf);
    assert_eq!(second.pdf, expected.pdf);
    assert_eq!(first.pages, expected.pages);
    assert_eq!(second.diagnostics, expected.diagnostics);
}

#[test]
fn reusable_converter_rejects_invalid_configuration_at_construction() {
    let mut options = ConvertOptions::browser_safe_defaults();
    options.font_provider.limits.max_asset_bytes = 1;

    assert!(matches!(
        Converter::new(options),
        Err(ConvertError::FontProvider(_))
    ));
}

#[test]
fn supported_root_facade_converts_and_audits_passive_pdf() {
    let input = br"{\rtf1\ansi Supported facade\par}";
    let options = ConvertOptions::browser_safe_defaults();
    options.validate().unwrap();

    let output = convert_rtf_to_pdf(input, &options).unwrap();
    let alias_output = convert_rtf_bytes_to_pdf(input, &options).unwrap();

    assert_eq!(output.pdf, alias_output.pdf);
    assert_eq!(output.pages, 1);
    assert!(output.pdf.starts_with(b"%PDF-"));
    audit_passive_pdf_bytes(&output.pdf).unwrap();
}

#[test]
fn supported_root_facade_exposes_configuration_and_error_types() {
    assert_public_error::<ConvertError>();
    assert_public_error::<ConvertOptionsError>();
    assert_public_error::<InspectError>();
    assert_public_error::<LimitValidationError>();
    assert_public_error::<ParseError>();
    assert_public_error::<PassivePdfError>();

    let options = RtfParseOptions {
        compatibility_mode: CompatibilityMode::WordCompatiblePassive,
        active_content_policy: ActiveContentPolicy::Placeholder,
        pdf_link_policy: PdfLinkPolicy::RenderVisibleTextOnly,
        limits: RtfLimits::browser_defaults(),
    };
    assert_eq!(options.validate(), Ok(()));
}

#[test]
fn supported_root_facade_inspects_requested_font_families() {
    let inspection = inspect_rtf(
        br"{\rtf1{\fonttbl{\f0 Primary{\*\falt Alternate;};}}Body\par}",
        &RtfParseOptions::browser_safe_defaults(),
    )
    .unwrap();

    assert_eq!(
        inspection,
        RtfInspection {
            requested_font_families: vec!["Primary".to_string(), "Alternate".to_string()],
        }
    );
}
