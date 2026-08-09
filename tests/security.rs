use open_rtf_converter::{
    ConvertError, ConvertOptions, RtfLimits, audit_passive_pdf_bytes, convert_rtf_to_pdf,
};

#[test]
fn public_conversion_boundary_emits_only_passive_pdf_content() {
    let input = br#"{\rtf1\ansi Visible{\object\objclass Package{\*\objdata 414243}}{\field{\*\fldinst HYPERLINK "https://example.com/payload"}{\fldrslt link}}}"#;
    let output = convert_rtf_to_pdf(input, &ConvertOptions::browser_safe_defaults()).unwrap();

    audit_passive_pdf_bytes(&output.pdf).unwrap();
    for forbidden in [b"414243".as_slice(), b"example.com", b"/EmbeddedFile"] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn public_conversion_boundary_enforces_pdf_page_limits() {
    let input = br"{\rtf1\ansi first\page second}";
    let mut options = ConvertOptions::default();
    options.parse_options.limits = RtfLimits {
        max_pdf_pages: 1,
        ..RtfLimits::default()
    };

    assert!(matches!(
        convert_rtf_to_pdf(input, &options),
        Err(ConvertError::TooManyPages { pages: 2, limit: 1 })
    ));
}
