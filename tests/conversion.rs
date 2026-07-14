use std::fs;

use lopdf::Document as PdfDocument;
use open_rtf_converter::rtf::ParseError;
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
fn browser_safe_conversion_rejects_excessive_font_table() {
    let limit = ConvertOptions::browser_safe_defaults()
        .parse_options
        .limits
        .max_fonts;
    let mut input = String::from(r"{\rtf1{\fonttbl");
    for idx in 0..=limit {
        input.push_str(&format!(r"{{\f{idx} BrowserFont{idx};}}"));
    }
    input.push_str(r"}Visible\par}");

    let error = convert_rtf_to_pdf(input.as_bytes(), &ConvertOptions::browser_safe_defaults())
        .expect_err("browser-safe conversion should reject excessive font tables");

    assert!(matches!(
        error,
        ConvertError::Parse(ParseError::ResourceLimitExceeded { resource, .. })
            if resource == "fonts"
    ));
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
fn caller_provided_font_asset_embeds_passive_type0_font_without_system_fonts() {
    let input = br"{\rtf1\ansi{\fonttbl{\f0 Tuffy;}}\f0 AB\par}";
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
    let output = convert_rtf_to_pdf(
        input,
        &ConvertOptions {
            font_provider: provider,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();

    assert_eq!(output.pages, 1);
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
    for expected in [
        b"/Subtype /Type0".as_slice(),
        b"/CIDFontType2".as_slice(),
        b"/Encoding /Identity-H".as_slice(),
        b"/FontFile2".as_slice(),
        b"/ToUnicode".as_slice(),
        b"/TF1".as_slice(),
    ] {
        assert!(
            output
                .pdf
                .windows(expected.len())
                .any(|window| window == expected),
            "expected supplied passive font marker {:?}",
            String::from_utf8_lossy(expected)
        );
    }
    for forbidden in [
        b"/JavaScript".as_slice(),
        b"/Launch".as_slice(),
        b"/EmbeddedFile".as_slice(),
        b"/Filespec".as_slice(),
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden active PDF marker {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn caller_provided_font_assets_prefer_matching_run_style() {
    let input = br"{\rtf1\ansi{\fonttbl{\f0 Tuffy;}}\f0 Regular {\b Bold}\par}";
    let provider = FontProvider {
        assets: vec![
            FontAsset {
                family_names: vec!["Tuffy".to_string()],
                style: FontAssetStyle::default(),
                bytes: include_bytes!("../fixtures/fonts/Tuffy.ttf").to_vec(),
            },
            FontAsset {
                family_names: vec!["Tuffy".to_string()],
                style: FontAssetStyle {
                    bold: true,
                    italic: false,
                },
                bytes: include_bytes!("../fixtures/fonts/Tuffy.ttf").to_vec(),
            },
        ],
        limits: FontProviderLimits {
            max_asset_bytes: 256 * 1024,
            max_total_bytes: 512 * 1024,
            ..FontProviderLimits::default()
        },
    };
    let output = convert_rtf_to_pdf(
        input,
        &ConvertOptions {
            font_provider: provider,
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();

    assert_eq!(output.pages, 1);
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
    for expected in [
        b"/TF1".as_slice(),
        b"/TF2".as_slice(),
        b"ORTF01+Tuffy",
        b"ORTF02+Tuffy",
    ] {
        assert!(
            output
                .pdf
                .windows(expected.len())
                .any(|window| window == expected),
            "expected style-matched supplied font marker {:?}",
            String::from_utf8_lossy(expected)
        );
    }
    assert_eq!(
        output
            .pdf
            .windows(b"/FontFile2".len())
            .filter(|window| *window == b"/FontFile2")
            .count(),
        2,
        "regular and bold caller-provided assets should both be embedded"
    );
    assert!(
        output
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("font 'Tuffy' substituted")),
        "caller-provided styled assets should avoid base-font substitution diagnostics: {:?}",
        output.diagnostics
    );
    for forbidden in [
        b"/JavaScript".as_slice(),
        b"/Launch".as_slice(),
        b"/EmbeddedFile".as_slice(),
        b"/Filespec".as_slice(),
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden active PDF marker {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn caller_base_font_asset_matches_word_charset_suffixed_font_names() {
    let input = br"{\rtf1\ansi{\fonttbl{\f38\fcharset204 Times New Roman Cyr;}}\f38 Alias AB\par}";
    let provider = FontProvider {
        assets: vec![FontAsset {
            family_names: vec!["Times New Roman".to_string()],
            style: FontAssetStyle::default(),
            bytes: include_bytes!("../fixtures/fonts/Tuffy.ttf").to_vec(),
        }],
        limits: FontProviderLimits {
            max_asset_bytes: 256 * 1024,
            max_total_bytes: 256 * 1024,
            ..FontProviderLimits::default()
        },
    };
    let output = convert_rtf_to_pdf(
        input,
        &ConvertOptions {
            diagnostics: true,
            font_provider: provider,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();

    assert_eq!(output.pages, 1);
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
    for expected in [
        b"/Subtype /Type0".as_slice(),
        b"/CIDFontType2".as_slice(),
        b"/Encoding /Identity-H".as_slice(),
        b"/FontFile2".as_slice(),
        b"/TF1".as_slice(),
    ] {
        assert!(
            output
                .pdf
                .windows(expected.len())
                .any(|window| window == expected),
            "expected supplied passive font marker {:?}",
            String::from_utf8_lossy(expected)
        );
    }
    assert!(
        output.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("Times New Roman Cyr")
            || !diagnostic.message.contains("substituted")),
        "caller base font asset should suppress substitution diagnostic for charset alias: {:?}",
        output.diagnostics
    );
}

#[test]
fn unused_font_table_entries_do_not_emit_passive_font_diagnostics() {
    let input =
        br"{\rtf1\ansi{\fonttbl{\f0 Arial;}{\f1 Calibri;}{\f2 Cambria;}}\f0 Visible text\par}";
    let output = convert_rtf_to_pdf(
        input,
        &ConvertOptions {
            diagnostics: true,
            font_provider: FontProvider::browser_safe_with_bundled_fallback(),
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();

    assert_eq!(output.pages, 1);
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
    assert!(
        output
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("font 'Arial' approximated")),
        "browser-safe bundled font should suppress Arial base-font approximation: {:?}",
        output.diagnostics
    );
    for unused in ["Calibri", "Cambria"] {
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains(unused)),
            "unused font table entry {unused} should not produce a font diagnostic: {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn caller_font_asset_aliases_embed_passive_font_for_multiple_word_names() {
    let input = br"{\rtf1\ansi{\fonttbl{\f0 Arial Narrow;}{\f1 Book Antiqua;}}\f0 Narrow AB\par\f1 Serif CD\par}";
    let provider = FontProvider {
        assets: vec![FontAsset {
            family_names: vec![
                "Arial Narrow".to_string(),
                "Book Antiqua".to_string(),
                "Times New Roman".to_string(),
            ],
            style: FontAssetStyle::default(),
            bytes: include_bytes!("../fixtures/fonts/Tuffy.ttf").to_vec(),
        }],
        limits: FontProviderLimits {
            max_asset_bytes: 256 * 1024,
            max_total_bytes: 256 * 1024,
            ..FontProviderLimits::default()
        },
    };
    let output = convert_rtf_to_pdf(
        input,
        &ConvertOptions {
            diagnostics: true,
            font_provider: provider,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();

    assert_eq!(output.pages, 1);
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
    for expected in [
        b"/Subtype /Type0".as_slice(),
        b"/CIDFontType2".as_slice(),
        b"/Encoding /Identity-H".as_slice(),
        b"/FontFile2".as_slice(),
        b"/TF1".as_slice(),
    ] {
        assert!(
            output
                .pdf
                .windows(expected.len())
                .any(|window| window == expected),
            "expected supplied passive font marker {:?}",
            String::from_utf8_lossy(expected)
        );
    }
    for forbidden in [
        b"/JavaScript".as_slice(),
        b"/OpenAction".as_slice(),
        b"/AA".as_slice(),
        b"/AcroForm".as_slice(),
        b"/Widget".as_slice(),
        b"/Launch".as_slice(),
        b"/EmbeddedFile".as_slice(),
        b"/Filespec".as_slice(),
        b"/RichMedia".as_slice(),
        b"/XFA".as_slice(),
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden active PDF marker {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
    for family in ["Arial Narrow", "Book Antiqua"] {
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains(family)
                    || !diagnostic.message.contains("substituted")),
            "caller alias should suppress substitution diagnostic for {family}: {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn caller_wildcard_font_asset_embeds_unmatched_word_font_without_system_fonts() {
    let input = br"{\rtf1\ansi{\fonttbl{\f7\froman Unknown Word Serif;}}\f7 Wildcard AB\par}";
    let provider = FontProvider {
        assets: vec![FontAsset {
            family_names: vec!["*".to_string()],
            style: FontAssetStyle::default(),
            bytes: include_bytes!("../fixtures/fonts/Tuffy.ttf").to_vec(),
        }],
        limits: FontProviderLimits {
            max_asset_bytes: 256 * 1024,
            max_total_bytes: 256 * 1024,
            ..FontProviderLimits::default()
        },
    };
    let output = convert_rtf_to_pdf(
        input,
        &ConvertOptions {
            diagnostics: true,
            font_provider: provider,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();

    assert_eq!(output.pages, 1);
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
    for expected in [
        b"/Subtype /Type0".as_slice(),
        b"/CIDFontType2".as_slice(),
        b"/Encoding /Identity-H".as_slice(),
        b"/FontFile2".as_slice(),
        b"/TF1".as_slice(),
    ] {
        assert!(
            output
                .pdf
                .windows(expected.len())
                .any(|window| window == expected),
            "expected wildcard passive font marker {:?}",
            String::from_utf8_lossy(expected)
        );
    }
    assert!(
        output.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("Unknown Word Serif")
            || !diagnostic.message.contains("substituted")),
        "wildcard caller font should suppress base-font substitution diagnostics: {:?}",
        output.diagnostics
    );
    for forbidden in [
        b"Unknown Word Serif".as_slice(),
        b"/JavaScript",
        b"/OpenAction",
        b"/AA",
        b"/AcroForm",
        b"/Widget",
        b"/Launch",
        b"/EmbeddedFile",
        b"/Filespec",
        b"/RichMedia",
        b"/XFA",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden active PDF or source font marker {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn browser_safe_defaults_embed_bundled_passive_fallback_font() {
    let input = br"{\rtf1\ansi{\fonttbl{\f7\froman Unknown Word Serif;}}\f7 Bundled AB\par}";
    let output = convert_rtf_to_pdf(
        input,
        &ConvertOptions {
            diagnostics: true,
            font_provider: FontProvider::browser_safe_with_bundled_fallback(),
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();

    assert_eq!(output.pages, 1);
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
    for expected in [
        b"/Subtype /Type0".as_slice(),
        b"/CIDFontType2".as_slice(),
        b"/Encoding /Identity-H".as_slice(),
        b"/FontFile2".as_slice(),
    ] {
        assert!(
            output
                .pdf
                .windows(expected.len())
                .any(|window| window == expected),
            "expected bundled passive font marker {:?}",
            String::from_utf8_lossy(expected)
        );
    }
    assert!(
        output.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("Unknown Word Serif")
            || !diagnostic.message.contains("substituted")),
        "browser-safe bundled font should suppress base-font substitution diagnostics: {:?}",
        output.diagnostics
    );
    for forbidden in [
        b"Unknown Word Serif".as_slice(),
        b"/JavaScript",
        b"/OpenAction",
        b"/AA",
        b"/AcroForm",
        b"/Widget",
        b"/Launch",
        b"/EmbeddedFile",
        b"/Filespec",
        b"/RichMedia",
        b"/XFA",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden active PDF or source font marker {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn caller_font_asset_matches_rtf_alternate_font_name_without_system_fonts() {
    let input = br"{\rtf1\ansi{\fonttbl{\f0 Mystery Sans{\*\falt Tuffy;};}}\f0 Alternate AB\par}";
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
    let output = convert_rtf_to_pdf(
        input,
        &ConvertOptions {
            diagnostics: true,
            font_provider: provider,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();

    assert_eq!(output.pages, 1);
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
    for expected in [
        b"/Subtype /Type0".as_slice(),
        b"/CIDFontType2".as_slice(),
        b"/Encoding /Identity-H".as_slice(),
        b"/FontFile2".as_slice(),
        b"/TF1".as_slice(),
    ] {
        assert!(
            output
                .pdf
                .windows(expected.len())
                .any(|window| window == expected),
            "expected alternate-name supplied passive font marker {:?}",
            String::from_utf8_lossy(expected)
        );
    }
    for forbidden in [
        b"fonttbl".as_slice(),
        b"falt",
        b"Mystery Sans",
        b"/JavaScript",
        b"/OpenAction",
        b"/AA",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "alternate font metadata or active marker leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
    assert!(
        output
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("Mystery Sans")
                || !diagnostic.message.contains("substituted")),
        "alternate caller font should suppress substitution diagnostics: {:?}",
        output.diagnostics
    );
}

#[test]
fn rtf_embedded_font_payload_does_not_become_pdf_font_file() {
    let input =
        br"{\rtf1\ansi{\fonttbl{\f0 Arial{\fontemb{\fontfile HOSTILE-FONT-PAYLOAD}};}}Visible\par}";
    let output = convert_rtf_to_pdf(input, &ConvertOptions::browser_safe_defaults()).unwrap();

    assert_eq!(output.pages, 1);
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
    for forbidden in [
        b"/FontFile".as_slice(),
        b"/FontFile2".as_slice(),
        b"/FontFile3".as_slice(),
        b"HOSTILE-FONT-PAYLOAD".as_slice(),
        b"fontemb".as_slice(),
        b"fontfile".as_slice(),
        b"/EmbeddedFile".as_slice(),
        b"/Launch".as_slice(),
        b"/JavaScript".as_slice(),
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "RTF embedded font payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
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
