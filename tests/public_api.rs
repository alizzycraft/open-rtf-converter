use std::error::Error;

use open_rtf_converter::{
    ActiveContentPolicy, CompatibilityMode, ConvertError, ConvertOptions, ConvertOptionsError,
    LimitValidationError, ParseError, PassivePdfError, PdfLinkPolicy, RtfLimits, RtfParseOptions,
    audit_passive_pdf_bytes, convert_rtf_bytes_to_pdf, convert_rtf_to_pdf,
};

fn assert_public_error<E: Error>() {}

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
