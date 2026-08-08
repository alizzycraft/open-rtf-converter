use super::*;

#[test]
fn validates_limits_through_the_public_api() {
    let mut limits = RtfLimits::default();
    limits.max_font_size_half_points = 0;

    assert_eq!(
        limits.validate(),
        Err(LimitValidationError::NonPositive {
            field: "max_font_size_half_points",
            value: 0,
        })
    );
}

#[test]
fn validates_conversion_options_through_the_public_api() {
    let mut options = ConvertOptions::default();
    options.parse_options.limits.max_shape_dimension_twips = 0;

    assert_eq!(
        options.validate(),
        Err(ConvertOptionsError::Limits(
            LimitValidationError::NonPositive {
                field: "max_shape_dimension_twips",
                value: 0,
            }
        ))
    );
}

#[test]
fn conversion_validates_limits_before_lexing_input() {
    let mut options = ConvertOptions::default();
    options.parse_options.limits.max_shape_dimension_twips = 0;

    let error = convert_rtf_to_pdf(b"}", &options).unwrap_err();
    assert!(matches!(
        error,
        ConvertError::Parse(ParseError::InvalidLimits(
            LimitValidationError::NonPositive {
                field: "max_shape_dimension_twips",
                value: 0,
            }
        ))
    ));
}

#[test]
fn browser_safe_defaults_use_stricter_pdf_output_limit() {
    assert_eq!(RtfLimits::default().max_pdf_output_bytes, 100 * 1024 * 1024);
    assert_eq!(RtfLimits::default().max_document_blocks, 1_000_000);
    assert_eq!(RtfLimits::default().max_pdf_layout_items, 1_000_000);
    assert_eq!(RtfLimits::default().max_pdf_indirect_objects, 100_000);
    assert_eq!(RtfLimits::default().max_pdf_pages, 10_000);
    assert_eq!(
        ConvertOptions::browser_safe_defaults()
            .parse_options
            .limits
            .max_pdf_output_bytes,
        20 * 1024 * 1024
    );
    assert_eq!(
        ConvertOptions::browser_safe_defaults()
            .parse_options
            .limits
            .max_document_blocks,
        200_000
    );
    assert_eq!(
        ConvertOptions::browser_safe_defaults()
            .parse_options
            .limits
            .max_pdf_layout_items,
        200_000
    );
    assert_eq!(
        ConvertOptions::browser_safe_defaults()
            .parse_options
            .limits
            .max_pdf_indirect_objects,
        25_000
    );
    assert_eq!(
        ConvertOptions::browser_safe_defaults()
            .parse_options
            .limits
            .max_pdf_pages,
        2_000
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
fn browser_safe_defaults_use_stricter_resource_table_limits() {
    let default_limits = RtfLimits::default();
    let browser_limits = ConvertOptions::browser_safe_defaults().parse_options.limits;

    assert_eq!(default_limits.max_fonts, 2_000);
    assert_eq!(default_limits.max_colors, 10_000);
    assert_eq!(default_limits.max_styles, 10_000);
    assert_eq!(browser_limits.max_fonts, 256);
    assert_eq!(browser_limits.max_colors, 2_048);
    assert_eq!(browser_limits.max_styles, 2_048);
    assert_eq!(default_limits.max_table_cells, 100_000);
    assert_eq!(browser_limits.max_table_cells, 50_000);
    assert_eq!(default_limits.max_table_rows, 50_000);
    assert_eq!(browser_limits.max_table_rows, 20_000);
    assert_eq!(default_limits.max_table_nesting_level, 16);
    assert_eq!(browser_limits.max_table_nesting_level, 8);
    assert_eq!(default_limits.max_header_footer_paragraphs, 20_000);
    assert_eq!(browser_limits.max_header_footer_paragraphs, 4_000);
    assert_eq!(default_limits.max_sections, 10_000);
    assert_eq!(browser_limits.max_sections, 2_000);
    assert_eq!(default_limits.max_notes, 10_000);
    assert_eq!(browser_limits.max_notes, 2_048);
    assert_eq!(default_limits.max_bookmarks, 10_000);
    assert_eq!(browser_limits.max_bookmarks, 2_048);
    assert_eq!(default_limits.max_field_counters, 10_000);
    assert_eq!(browser_limits.max_field_counters, 2_048);
    assert_eq!(default_limits.max_document_properties, 10_000);
    assert_eq!(browser_limits.max_document_properties, 2_048);
    assert_eq!(default_limits.max_lists, 10_000);
    assert_eq!(browser_limits.max_lists, 2_048);
    assert_eq!(default_limits.max_style_references, 10_000);
    assert_eq!(browser_limits.max_style_references, 2_048);
    assert_eq!(default_limits.max_field_instruction_chars, 64 * 1024);
    assert_eq!(browser_limits.max_field_instruction_chars, 16 * 1024);
}

#[test]
fn browser_safe_conversion_rejects_excessive_color_table() {
    let limit = ConvertOptions::browser_safe_defaults()
        .parse_options
        .limits
        .max_colors;
    let mut input = String::from(r"{\rtf1{\colortbl;");
    for idx in 0..=limit {
        let red = idx % 256;
        let green = (idx / 2) % 256;
        let blue = (idx / 3) % 256;
        input.push_str(&format!(r"\red{red}\green{green}\blue{blue};"));
    }
    input.push_str(r"}Visible\par}");

    let error = convert_rtf_to_pdf(input.as_bytes(), &ConvertOptions::browser_safe_defaults())
        .expect_err("browser-safe conversion should reject excessive color tables");

    assert!(matches!(
        error,
        ConvertError::Parse(ParseError::ResourceLimitExceeded { resource, .. })
            if resource == "colors"
    ));
}

#[test]
fn browser_safe_conversion_rejects_excessive_stylesheet() {
    let limit = ConvertOptions::browser_safe_defaults()
        .parse_options
        .limits
        .max_styles;
    let mut input = String::from(r"{\rtf1{\stylesheet");
    for idx in 0..=limit {
        input.push_str(&format!(r"{{\s{idx} Browser Style {idx};}}"));
    }
    input.push_str(r"}Visible\par}");

    let error = convert_rtf_to_pdf(input.as_bytes(), &ConvertOptions::browser_safe_defaults())
        .expect_err("browser-safe conversion should reject excessive stylesheets");

    assert!(matches!(
        error,
        ConvertError::Parse(ParseError::ResourceLimitExceeded { resource, .. })
            if resource == "styles"
    ));
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
fn conversion_rejects_layout_item_count_over_configured_limit_before_pdf_rendering() {
    let input = br"{\rtf1\ansi First\par Second\par Third\par}";
    let error = convert_rtf_to_pdf(
        input,
        &ConvertOptions {
            parse_options: RtfParseOptions {
                limits: RtfLimits {
                    max_pdf_layout_items: 2,
                    ..RtfLimits::default()
                },
                ..RtfParseOptions::default()
            },
            ..ConvertOptions::default()
        },
    )
    .expect_err("three laid-out text fragments should exceed a two-item PDF layout limit");

    assert!(matches!(
        error,
        ConvertError::TooManyLayoutItems { items, limit } if items == 3 && limit == 2
    ));
}

#[test]
fn conversion_rejects_page_count_over_configured_limit_before_pdf_rendering() {
    let input = br"{\rtf1\ansi First\page Second\page Third\par}";
    let error = convert_rtf_to_pdf(
        input,
        &ConvertOptions {
            parse_options: RtfParseOptions {
                limits: RtfLimits {
                    max_pdf_pages: 2,
                    ..RtfLimits::default()
                },
                ..RtfParseOptions::default()
            },
            ..ConvertOptions::default()
        },
    )
    .expect_err("three-page layout should exceed a two-page PDF limit");

    assert!(matches!(
        error,
        ConvertError::TooManyPages { pages, limit } if pages == 3 && limit == 2
    ));
}

#[test]
fn conversion_rejects_pdf_object_count_over_configured_limit_before_pdf_rendering() {
    let input = br"{\rtf1\ansi Object guard\par}";
    let error = convert_rtf_to_pdf(
        input,
        &ConvertOptions {
            parse_options: RtfParseOptions {
                limits: RtfLimits {
                    max_pdf_indirect_objects: 1,
                    ..RtfLimits::default()
                },
                ..RtfParseOptions::default()
            },
            ..ConvertOptions::default()
        },
    )
    .expect_err("even a minimal PDF should exceed a one-object PDF limit");

    assert!(matches!(
        error,
        ConvertError::TooManyPdfObjects { objects, limit } if limit == 1 && objects > limit
    ));
}

#[test]
fn conversion_audits_pdf_syntax_without_rejecting_visible_active_words() {
    let input = br"{\rtf1\ansi Visible /JavaScript /Launch /URI /Annots /Widget text\par}";
    let output = convert_rtf_to_pdf(input, &ConvertOptions::browser_safe_defaults()).unwrap();

    assert_eq!(output.pages, 1);
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
}
