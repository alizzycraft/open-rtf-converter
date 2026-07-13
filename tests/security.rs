use std::fs;

use lopdf::Document as PdfDocument;
use open_rtf_converter::model::{
    Alignment, BOOKMARK_PAGE_ANCHOR_MARKER, BOOKMARK_PAGE_MARKER_END, BOOKMARK_PAGE_REF_MARKER,
    Block, BorderStyle, CharacterEmphasisMark, CharacterStyle, DOCUMENT_CHARS_MARKER,
    DOCUMENT_CHARS_WITH_SPACES_MARKER, DOCUMENT_WORDS_MARKER, EndnotePlacement, FontFamilyHint,
    FontPitch, ImageFormat, PAGE_NUMBER_MARKER, PageVerticalAlignment, SECTION_NUMBER_MARKER,
    SECTION_PAGES_MARKER, ShadingPattern, StaticImageTextHorizontalAlign,
    StaticImageTextVerticalAlign, StaticImageVectorCommand, StaticImageVectorFillRule,
    TOTAL_PAGES_MARKER, TabAlignment, TextRelief, UnderlineStyle,
};
use open_rtf_converter::pdf::audit_passive_pdf_bytes;
use open_rtf_converter::rtf::{
    LexError, ParseError, parse_rtf_bytes, parse_rtf_bytes_with_options,
};
use open_rtf_converter::{
    ActiveContentPolicy, ConvertOptions, Diagnostic, FontAsset, FontAssetStyle, FontProvider,
    PdfLinkPolicy, RtfLimits, RtfParseOptions, convert_rtf_file_to_pdf, convert_rtf_to_pdf,
};
use tempfile::tempdir;

#[test]
fn bin_lengths_are_bounded_and_binary_does_not_become_text() {
    let parsed = parse_rtf_bytes(&rtf(&[
        "{",
        "\\",
        "rtf1 before ",
        "\\",
        "bin3 a",
        "\\",
        "} after}",
    ]))
    .unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("a\\}"));

    assert!(matches!(
        parse_rtf_bytes(&rtf(&["{", "\\", "rtf1 ", "\\", "bin-1 abc}"])),
        Err(ParseError::Lex(LexError::InvalidBinaryLength(_)))
    ));
    assert!(matches!(
        parse_rtf_bytes(&rtf(&["{", "\\", "rtf1 ", "\\", "bin9999999999999 abc}"])),
        Err(ParseError::Lex(
            LexError::NumericParameterTooLong(_) | LexError::NumericParameterOverflow(_)
        ))
    ));
    assert!(matches!(
        parse_rtf_bytes(&rtf(&["{", "\\", "rtf1 ", "\\", "bin5 abc}"])),
        Err(ParseError::Lex(LexError::ShortBinaryBlob(_)))
    ));
}

#[test]
fn binary_payloads_obey_reject_policy_outside_passive_pictures() {
    let strip_input = rtf(&["{", "\\", "rtf1 before ", "\\", "bin3 abc after}"]);
    let stripped = parse_rtf_bytes(&strip_input).unwrap();
    let text = collect_text(&stripped.document);
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("abc"));

    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };
    assert!(matches!(
        parse_rtf_bytes_with_options(&strip_input, &reject_options),
        Err(ParseError::ActiveContentRejected { feature, .. }) if feature == "binary RTF payload"
    ));
    assert!(matches!(
        parse_rtf_bytes_with_options(
            br"{\rtf1{\*\unknown\bin5 abcde} visible\par}",
            &reject_options
        ),
        Err(ParseError::ActiveContentRejected { feature, .. }) if feature == "binary RTF payload"
    ));
    assert!(matches!(
        parse_rtf_bytes_with_options(
            br"{\rtf1{\unknown\bin5 abcde} visible\par}",
            &reject_options
        ),
        Err(ParseError::ActiveContentRejected { feature, .. }) if feature == "binary RTF payload"
    ));
}

#[test]
fn malformed_hex_escapes_are_typed_errors() {
    let parsed = parse_rtf_bytes(&rtf(&[
        "{", "\\", "rtf1 ", "\\", "'41", "\\", "'42", "\\", "'43}",
    ]))
    .unwrap();
    assert!(collect_text(&parsed.document).contains("ABC"));

    for input in [
        rtf(&["{", "\\", "rtf1 ", "\\", "'}"]),
        rtf(&["{", "\\", "rtf1 ", "\\", "'GZ}"]),
        rtf(&["{", "\\", "rtf1 ", "\\", "'1}"]),
    ] {
        assert!(matches!(
            parse_rtf_bytes(&input),
            Err(ParseError::Lex(LexError::MalformedHexEscape(_)))
        ));
    }
}

#[test]
fn hex_escapes_inside_object_data_do_not_become_text() {
    let parsed = parse_rtf_bytes(&rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "object{",
        "\\",
        "objdata ",
        "\\",
        "'41",
        "\\",
        "'42",
        "\\",
        "'43}} visible}",
    ]))
    .unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("visible"));
    assert!(!text.contains("ABC"));
}

#[test]
fn cyrillic_charset_text_is_decoded_but_unrenderable_font_gap_is_reported() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "ansicpg1252{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Times New Roman;}{",
        "\\",
        "f38",
        "\\",
        "fcharset204 Times New Roman Cyr;}}",
        "\\",
        "f38 ",
        "\\",
        "'cf",
        "\\",
        "'f0",
        "\\",
        "'e8",
        "\\",
        "'e2",
        "\\",
        "'e5",
        "\\",
        "'f2",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("\u{041f}\u{0440}\u{0438}\u{0432}\u{0435}\u{0442}"));
    assert!(!text.contains("fcharset204"));
    assert!(!text.contains("Times New Roman Cyr"));

    let mut options = ConvertOptions::browser_safe_defaults();
    options.diagnostics = true;
    let output = convert_rtf_to_pdf(&input, &options).unwrap();
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("Cyrillic characters")
            && diagnostic.message.contains("passive font asset support")
            && diagnostic.message.contains("Times New Roman Cyr")
    }));

    for forbidden in [
        b"Times New Roman Cyr".as_slice(),
        b"fcharset204".as_slice(),
        b"ansicpg1251".as_slice(),
        b"/JavaScript".as_slice(),
        b"/EmbeddedFile".as_slice(),
        b"/Launch".as_slice(),
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden Cyrillic/source content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn invalid_caller_font_assets_are_rejected_before_pdf_rendering() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi{",
        "\\",
        "fonttbl{",
        "\\",
        "f38",
        "\\",
        "fcharset204 Times New Roman Cyr;}}",
        "\\",
        "f38 ",
        "\\",
        "'cf",
        "\\",
        "'f0",
        "\\",
        "'e8",
        "\\",
        "'e2",
        "\\",
        "'e5",
        "\\",
        "'f2",
        "\\",
        "par}",
    ]);
    let font_bytes = b"not-a-real-font-but-hostile-private-data".to_vec();
    let mut options = ConvertOptions::browser_safe_defaults();
    options.diagnostics = true;
    options.font_provider = FontProvider {
        assets: vec![FontAsset {
            family_names: vec!["Times New Roman Cyr".to_string()],
            style: FontAssetStyle::default(),
            bytes: font_bytes.clone(),
        }],
        ..FontProvider::browser_safe_defaults()
    };

    let error = convert_rtf_to_pdf(&input, &options).unwrap_err();
    assert!(matches!(
        error,
        open_rtf_converter::ConvertError::FontProvider(
            open_rtf_converter::FontProviderError::InvalidAsset { .. }
        )
    ));
}

#[test]
fn oversized_controls_parameters_and_deep_groups_are_rejected() {
    let options = RtfParseOptions {
        limits: RtfLimits {
            max_group_depth: 3,
            max_control_word_len: 4,
            max_parameter_digits: 2,
            ..RtfLimits::default()
        },
        ..RtfParseOptions::default()
    };

    assert!(matches!(
        parse_rtf_bytes_with_options(&rtf(&["{", "\\", "rtf1 ", "\\", "abcde text}"]), &options),
        Err(ParseError::Lex(LexError::ControlWordTooLong(_)))
    ));
    assert!(matches!(
        parse_rtf_bytes_with_options(&rtf(&["{", "\\", "rtf1 ", "\\", "fs240 text}"]), &options),
        Err(ParseError::Lex(LexError::NumericParameterTooLong(_)))
    ));
    assert!(matches!(
        parse_rtf_bytes_with_options(b"{{{{x}}}}", &options),
        Err(ParseError::Lex(LexError::GroupDepthExceeded(_)))
    ));
}

#[test]
fn unmatched_and_unclosed_groups_are_rejected_by_tokenizer_boundary() {
    assert!(matches!(
        parse_rtf_bytes(b"}{\\rtf1 visible}"),
        Err(ParseError::Lex(LexError::UnmatchedGroupEnd(0)))
    ));
    assert!(matches!(
        parse_rtf_bytes(b"{\\rtf1 visible"),
        Err(ParseError::Lex(LexError::UnclosedGroup(_)))
    ));
}

#[test]
fn table_cell_growth_is_bounded() {
    let options = RtfParseOptions {
        limits: RtfLimits {
            max_table_cells: 1,
            ..RtfLimits::default()
        },
        ..RtfParseOptions::default()
    };

    assert!(matches!(
        parse_rtf_bytes_with_options(
            &rtf(&[
                "{",
                "\\",
                "rtf1",
                "\\",
                "trowd",
                "\\",
                "cellx1000 A",
                "\\",
                "cell B",
                "\\",
                "cell",
                "\\",
                "row}"
            ]),
            &options
        ),
        Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "table cells"
    ));
}

#[test]
fn table_row_padding_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "trowd",
        "\\",
        "trpaddl360",
        "\\",
        "trpaddr120",
        "\\",
        "trpaddt60",
        "\\",
        "trpaddb180",
        "\\",
        "cellx1440 A",
        "\\",
        "cell",
        "\\",
        "cellx2880 B",
        "\\",
        "cell",
        "\\",
        "row}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("A"));
    assert!(text.contains("B"));
    for forbidden in ["trpaddl", "trpaddr", "trpaddt", "trpaddb"] {
        assert!(
            !text.contains(forbidden),
            "forbidden table row padding control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("table-row-padding.rtf");
    let output_path = dir.path().join("table-row-padding.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"trpaddl".as_slice(),
        b"trpaddr",
        b"trpaddt",
        b"trpaddb",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden table row padding content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn preferred_cell_widths_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw7200",
        "\\",
        "margl720",
        "\\",
        "margr720",
        "\\",
        "trowd",
        "\\",
        "clftsWidth3",
        "\\",
        "clwWidth1440 Narrow",
        "\\",
        "cell",
        "\\",
        "clftsWidth2",
        "\\",
        "clwWidth2500 Wide percent",
        "\\",
        "cell",
        "\\",
        "row}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let table = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("table");

    assert_eq!(table.column_widths_twips, vec![1440, 2880]);
    assert!(text.contains("Narrow"));
    assert!(text.contains("Wide percent"));
    for forbidden in ["clftsWidth", "clwWidth"] {
        assert!(
            !text.contains(forbidden),
            "forbidden preferred-width control leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Narrow"));
    assert!(rendered_text.contains("Wide percent"));
    for forbidden in [
        b"clftsWidth".as_slice(),
        b"clwWidth",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden preferred-width content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn preferred_row_widths_fill_missing_table_geometry_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw7200",
        "\\",
        "margl720",
        "\\",
        "margr720",
        "\\",
        "trowd",
        "\\",
        "trftsWidth3",
        "\\",
        "trwWidth2880 Exact left",
        "\\",
        "cell Exact right",
        "\\",
        "cell",
        "\\",
        "row",
        "\\",
        "trowd",
        "\\",
        "trftsWidth2",
        "\\",
        "trwWidth2500 Percent left",
        "\\",
        "cell Percent right",
        "\\",
        "cell",
        "\\",
        "row}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let table = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("table");

    assert_eq!(table.column_widths_twips, vec![1440, 1440]);
    assert!(text.contains("Exact left"));
    assert!(text.contains("Exact right"));
    assert!(text.contains("Percent left"));
    assert!(text.contains("Percent right"));
    for forbidden in ["trftsWidth", "trwWidth"] {
        assert!(
            !text.contains(forbidden),
            "forbidden preferred row-width control leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Exact left"));
    assert!(rendered_text.contains("Exact right"));
    assert!(rendered_text.contains("Percent left"));
    assert!(rendered_text.contains("Percent right"));
    for forbidden in [
        b"trftsWidth".as_slice(),
        b"trwWidth",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden preferred row-width content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn nogrowautofit_preserves_authored_table_widths_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw7200",
        "\\",
        "margl720",
        "\\",
        "margr720",
        "\\",
        "nogrowautofit",
        "\\",
        "trowd",
        "\\",
        "cellx4000 Wide left",
        "\\",
        "cell",
        "\\",
        "cellx8000 Wide right",
        "\\",
        "cell",
        "\\",
        "row}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let table = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("table");

    assert!(table.preserve_authored_widths);
    assert_eq!(table.column_widths_twips, vec![4_000, 4_000]);
    assert!(text.contains("Wide left"));
    assert!(text.contains("Wide right"));
    assert!(!text.contains("nogrowautofit"));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("table autofit growth compatibility approximated")
    }));

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Wide left"));
    assert!(rendered_text.contains("Wide right"));
    for forbidden in [
        b"nogrowautofit".as_slice(),
        b"cellx",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden nogrowautofit content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn trautofit_row_control_normalizes_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw7200",
        "\\",
        "margl720",
        "\\",
        "margr720",
        "\\",
        "trowd",
        "\\",
        "trautofit0",
        "\\",
        "cellx4000 Fixed left",
        "\\",
        "cell",
        "\\",
        "cellx8000 Fixed right",
        "\\",
        "cell",
        "\\",
        "row",
        "}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let table = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("table");

    assert!(table.preserve_authored_widths);
    assert_eq!(table.column_widths_twips, vec![4_000, 4_000]);
    assert!(text.contains("Fixed left"));
    assert!(text.contains("Fixed right"));
    assert!(!text.contains("trautofit"));
    assert!(
        parsed.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("table row autofit interpreted through bounded passive table width layout")),
        "missing trautofit diagnostic: {:?}",
        parsed.diagnostics
    );
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "trautofit should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Fixed left"));
    assert!(rendered_text.contains("Fixed right"));
    for forbidden in [
        b"trautofit".as_slice(),
        b"cellx",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden trautofit content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn floating_table_positioning_controls_warn_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw7200",
        "\\",
        "margl720",
        "\\",
        "margr720",
        "\\",
        "trowd",
        "\\",
        "trleft360",
        "\\",
        "trgaph108",
        "\\",
        "trftsWidth3",
        "\\",
        "trwWidth3600",
        "\\",
        "tabsnoovrlp",
        "\\",
        "tdfrmtxtLeft180",
        "\\",
        "tdfrmtxtRight180",
        "\\",
        "tdfrmtxtTop120",
        "\\",
        "tdfrmtxtBottom120",
        "\\",
        "tphmrg",
        "\\",
        "tposx720",
        "\\",
        "tpvmrg",
        "\\",
        "tposy360",
        "\\",
        "clftsWidth3",
        "\\",
        "clwWidth1800 Positioned left",
        "\\",
        "cell",
        "\\",
        "clFitText",
        "\\",
        "clftsWidth3",
        "\\",
        "clwWidth1800 Positioned right",
        "\\",
        "cell",
        "\\",
        "row",
        "\\",
        "pard After table",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let table = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("table");

    assert!(text.contains("Positioned left"));
    assert!(text.contains("Positioned right"));
    assert!(text.contains("After table"));
    assert!(!table.rows[0].cells[0].fit_text);
    assert!(table.rows[0].cells[1].fit_text);
    for forbidden in [
        "tabsnoovrlp",
        "tdfrmtxtLeft",
        "tdfrmtxtRight",
        "tdfrmtxtTop",
        "tdfrmtxtBottom",
        "tphmrg",
        "tposx",
        "tpvmrg",
        "tposy",
        "clFitText",
    ] {
        assert!(
            !text.contains(forbidden),
            "floating table control leaked to normalized text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "floating table controls should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("floating table positioning approximated by passive table flow")
    }));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("table cell fit-text approximated")
    }));

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Positioned left"));
    assert!(rendered_text.contains("Positioned right"));
    assert!(rendered_text.contains("After table"));
    for forbidden in [
        b"tabsnoovrlp".as_slice(),
        b"tdfrmtxtLeft",
        b"tdfrmtxtRight",
        b"tdfrmtxtTop",
        b"tdfrmtxtBottom",
        b"tphmrg",
        b"tposx",
        b"tpvmrg",
        b"tposy",
        b"clFitText",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden floating table content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn table_padding_unit_and_spacing_controls_warn_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw7200",
        "\\",
        "margl720",
        "\\",
        "margr720",
        "\\",
        "trowd",
        "\\",
        "trgaph108",
        "\\",
        "trpaddfl3",
        "\\",
        "trpaddl240",
        "\\",
        "trpaddfr3",
        "\\",
        "trpaddr240",
        "\\",
        "trpaddft3",
        "\\",
        "trpaddt120",
        "\\",
        "trpaddfb3",
        "\\",
        "trpaddb120",
        "\\",
        "trspdl120",
        "\\",
        "trspdfl3",
        "\\",
        "trspdr120",
        "\\",
        "trspdfr3",
        "\\",
        "trspdt60",
        "\\",
        "trspdft3",
        "\\",
        "trspdb60",
        "\\",
        "trspdfb3",
        "\\",
        "clpadfl3",
        "\\",
        "clpadl180",
        "\\",
        "clpadfr3",
        "\\",
        "clpadr180",
        "\\",
        "clpadft3",
        "\\",
        "clpadt90",
        "\\",
        "clpadfb3",
        "\\",
        "clpadb90",
        "\\",
        "clspdl60",
        "\\",
        "clspdfl3",
        "\\",
        "clspdr60",
        "\\",
        "clspdfr3",
        "\\",
        "clspdt30",
        "\\",
        "clspdft3",
        "\\",
        "clspdb30",
        "\\",
        "clspdfb3",
        "\\",
        "cellx1800 Unit left",
        "\\",
        "cell",
        "\\",
        "clpadfl3",
        "\\",
        "clpadl180",
        "\\",
        "cellx3600 Unit right",
        "\\",
        "cell",
        "\\",
        "row}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let table = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("table");

    assert!(text.contains("Unit left"));
    assert!(text.contains("Unit right"));
    assert_eq!(table.rows[0].cells[0].spacing.left_twips, Some(60));
    assert_eq!(table.rows[0].cells[0].spacing.right_twips, Some(60));
    assert_eq!(table.rows[0].cells[0].spacing.top_twips, Some(30));
    assert_eq!(table.rows[0].cells[0].spacing.bottom_twips, Some(30));
    assert_eq!(table.rows[0].cells[1].spacing.left_twips, Some(120));
    assert_eq!(table.rows[0].cells[1].spacing.right_twips, Some(120));
    for forbidden in [
        "trpaddfl", "trpaddfr", "trpaddft", "trpaddfb", "trspdl", "trspdfl", "trspdr", "trspdfr",
        "trspdt", "trspdft", "trspdb", "trspdfb", "clpadfl", "clpadfr", "clpadft", "clpadfb",
        "clspdl", "clspdfl", "clspdr", "clspdfr", "clspdt", "clspdft", "clspdb", "clspdfb",
    ] {
        assert!(
            !text.contains(forbidden),
            "table padding/spacing control leaked to normalized text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "table padding/spacing controls should not be unsupported: {:?}",
        parsed.diagnostics
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("table padding and spacing units interpreted through bounded twip layout")
    }));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("table cell spacing approximated")
    }));

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Unit left"));
    assert!(rendered_text.contains("Unit right"));
    for forbidden in [
        b"trpaddfl".as_slice(),
        b"trpaddfr",
        b"trpaddft",
        b"trpaddfb",
        b"trspdl",
        b"trspdfl",
        b"trspdr",
        b"trspdfr",
        b"trspdt",
        b"trspdft",
        b"trspdb",
        b"trspdfb",
        b"clpadfl",
        b"clpadfr",
        b"clpadft",
        b"clpadfb",
        b"clspdl",
        b"clspdfl",
        b"clspdr",
        b"clspdfr",
        b"clspdt",
        b"clspdft",
        b"clspdb",
        b"clspdfb",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden table padding/spacing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn shading_patterns_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "colortbl;",
        "\\",
        "red255",
        "\\",
        "green0",
        "\\",
        "blue0;}",
        "\\",
        "cbpat1",
        "\\",
        "shading2500",
        "\\",
        "bghoriz Paragraph",
        "\\",
        "par",
        "\\",
        "trowd",
        "\\",
        "trcbpat1",
        "\\",
        "trshdng5000",
        "\\",
        "trbgvert",
        "\\",
        "cellx1440 Row",
        "\\",
        "cell",
        "\\",
        "clcbpat1",
        "\\",
        "clshdng7500",
        "\\",
        "clbghoriz",
        "\\",
        "cellx2880 Cell",
        "\\",
        "cell",
        "\\",
        "row}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Paragraph"));
    assert!(text.contains("Row"));
    assert!(text.contains("Cell"));
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    assert_eq!(paragraph.style.shading_pattern, ShadingPattern::Horizontal);
    let table = match &parsed.document.blocks[1] {
        Block::Table(table) => table,
        _ => panic!("expected table"),
    };
    assert_eq!(
        table.rows[0].cells[0].shading_pattern,
        ShadingPattern::Vertical
    );
    assert_eq!(
        table.rows[0].cells[1].shading_pattern,
        ShadingPattern::Horizontal
    );
    for forbidden in ["bghoriz", "trbgvert", "clbghoriz"] {
        assert!(
            !text.contains(forbidden),
            "shading pattern control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("shading-patterns.rtf");
    let output_path = dir.path().join("shading-patterns.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"bghoriz".as_slice(),
        b"trbgvert",
        b"clbghoriz",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden shading pattern content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn extended_shading_patterns_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "colortbl;",
        "\\",
        "red255",
        "\\",
        "green0",
        "\\",
        "blue0;}",
        "\\",
        "cbpat1",
        "\\",
        "bgdkdcross Dark diagonal",
        "\\",
        "par",
        "\\",
        "trowd",
        "\\",
        "trcbpat1",
        "\\",
        "trbgfdiag",
        "\\",
        "cellx1440 Row",
        "\\",
        "cell",
        "\\",
        "clcbpat1",
        "\\",
        "clbgcross",
        "\\",
        "cellx2880 Cell",
        "\\",
        "cell",
        "\\",
        "row}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Dark diagonal"));
    assert!(text.contains("Row"));
    assert!(text.contains("Cell"));
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    assert_eq!(
        paragraph.style.shading_pattern,
        ShadingPattern::DarkDiagonalCross
    );
    let table = match &parsed.document.blocks[1] {
        Block::Table(table) => table,
        _ => panic!("expected table"),
    };
    assert_eq!(
        table.rows[0].cells[0].shading_pattern,
        ShadingPattern::ForwardDiagonal
    );
    assert_eq!(
        table.rows[0].cells[1].shading_pattern,
        ShadingPattern::Cross
    );
    for forbidden in ["bgdkdcross", "trbgfdiag", "clbgcross"] {
        assert!(
            !text.contains(forbidden),
            "extended shading control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("extended-shading-patterns.rtf");
    let output_path = dir.path().join("extended-shading-patterns.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"bgdkdcross".as_slice(),
        b"trbgfdiag",
        b"clbgcross",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden extended shading content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn table_row_keep_together_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "trowd",
        "\\",
        "trkeep",
        "\\",
        "cellx1440 Kept row",
        "\\",
        "cell",
        "\\",
        "row",
        "\\",
        "trowd",
        "\\",
        "trkeep0",
        "\\",
        "cellx1440 Normal row",
        "\\",
        "cell",
        "\\",
        "row}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let table = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("table");

    assert!(text.contains("Kept row"));
    assert!(text.contains("Normal row"));
    assert!(table.rows[0].keep_together);
    assert!(!table.rows[1].keep_together);
    assert!(!text.contains("trkeep"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("table-row-keep-together.rtf");
    let output_path = dir.path().join("table-row-keep-together.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Kept row"));
    assert!(rendered_text.contains("Normal row"));
    for forbidden in [
        b"trkeep".as_slice(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden table row keep-together content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn table_header_rows_repeat_passively_without_control_leakage() {
    let mut input =
        String::from("{\\rtf1\\trowd\\trhdr\\trrh720\\clcbpat1\\cellx3000 Header row\\cell\\row");
    input.push_str("\\trowd\\trhdr0\\trrh720\\cellx3000 First body\\cell\\row");
    for idx in 0..28 {
        input.push_str(&format!(
            "\\trowd\\trrh720\\cellx3000 Body row {idx}\\cell\\row"
        ));
    }
    input.push('}');
    let input = input.into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let table = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("table");

    assert!(text.contains("Header row"));
    assert!(text.contains("First body"));
    assert!(table.rows[0].repeat_header);
    assert!(!table.rows[1].repeat_header);
    for forbidden in ["trhdr", "trhdr0", "trrh", "cellx", "trowd"] {
        assert!(
            !text.contains(forbidden),
            "forbidden table header control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("table-header-repeat.rtf");
    let output_path = dir.path().join("table-header-repeat.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let pages = parsed_pdf.get_pages();

    assert!(pages.len() > 1, "test input should flow across pages");
    for page_id in pages.values().skip(1) {
        let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
        let rendered_text = decoded_pdf_text(&content);
        assert!(
            rendered_text.contains("Header row"),
            "repeated header missing from continued page text: {rendered_text:?}"
        );
    }
    for forbidden in [
        b"trhdr".as_slice(),
        b"trhdr0",
        b"trrh",
        b"cellx",
        b"trowd",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden table header content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn tall_auto_height_table_rows_split_passively_without_control_leakage() {
    let mut input = String::from("{\\rtf1\\ansi");
    input.push_str(
        "{\\object\\objemb\\objdata 4142432f4a6176615363726970742f456d62656464656446696c65{\\result Safe object fallback\\par}}",
    );
    input.push_str("\\trowd\\cellx9000 ");
    for idx in 0..120 {
        input.push_str(&format!("Split row line {idx:03}\\line "));
    }
    input.push_str("\\cell\\row}");
    let input = input.into_bytes();

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Safe object fallback"));
    assert!(text.contains("Split row line 000"));
    assert!(text.contains("Split row line 119"));
    for forbidden in [
        "objdata",
        "objemb",
        "JavaScript",
        "EmbeddedFile",
        "cellx",
        "trowd",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden tall-row source content leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    audit_passive_pdf_bytes(&output.pdf).unwrap();

    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let pages = parsed_pdf.get_pages();
    assert!(pages.len() > 1, "tall row should split across PDF pages");

    let page_texts = pages
        .values()
        .map(|page_id| {
            let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
            decoded_pdf_text(&content)
        })
        .collect::<Vec<_>>();
    assert!(
        page_texts[0].contains("Split row line 000"),
        "first split-row fragment missing first line: {:?}",
        page_texts[0]
    );
    assert!(
        page_texts
            .iter()
            .skip(1)
            .any(|text| text.contains("Split row line 119")),
        "continued split-row fragments missing final line: {page_texts:?}"
    );

    for forbidden in [
        b"objdata".as_slice(),
        b"objemb",
        b"414243",
        b"JavaScript",
        b"EmbeddedFile",
        b"cellx",
        b"trowd",
        b"/OpenAction",
        b"/AcroForm",
        b"/Annots",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden tall-row source content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn nested_table_content_flattens_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "trowd",
        "\\",
        "cellx6000 Outer before {",
        "\\",
        "trowd",
        "\\",
        "itap2",
        "\\",
        "nesttableprops",
        "\\",
        "cellx1000 Inner A",
        "\\",
        "nestcell",
        "\\",
        "cellx2000 Inner B",
        "\\",
        "nestrow} Outer after",
        "\\",
        "cell",
        "\\",
        "row}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let table = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("table");

    assert_eq!(table.rows.len(), 1);
    assert_eq!(table.rows[0].cells.len(), 1);
    assert!(text.contains("Outer before"));
    assert!(text.contains("Inner A"));
    assert!(text.contains("Inner B"));
    assert!(text.contains("Outer after"));
    for forbidden in ["itap", "nesttableprops", "nestcell", "nestrow"] {
        assert!(
            !text.contains(forbidden),
            "forbidden nested table control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("nested-table-passive.rtf");
    let output_path = dir.path().join("nested-table-passive.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Outer before"));
    assert!(rendered_text.contains("Inner A"));
    assert!(rendered_text.contains("Inner B"));
    assert!(rendered_text.contains("Outer after"));
    for forbidden in [
        b"itap".as_slice(),
        b"nesttableprops",
        b"nestcell",
        b"nestrow",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden nested table content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn exact_table_row_height_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "colortbl;",
        "\\",
        "red240",
        "\\",
        "green240",
        "\\",
        "blue240;}",
        "\\",
        "trowd",
        "\\",
        "trrh-360",
        "\\",
        "trcbpat1",
        "\\",
        "cellx1440 Visible",
        "\\",
        "line Overflow",
        "\\",
        "cell",
        "\\",
        "row}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible"));
    assert!(text.contains("Overflow"));
    let table = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            open_rtf_converter::model::Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("table");
    assert_eq!(table.rows[0].height_twips, Some(-360));
    assert!(!text.contains("trrh"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("table-row-exact-height.rtf");
    let output_path = dir.path().join("table-row-exact-height.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Visible"));
    assert!(!rendered_text.contains("Overflow"));
    for forbidden in [
        b"trrh".as_slice(),
        b"trcbpat",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden table row height content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn table_cell_text_direction_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "trowd",
        "\\",
        "cltxtbrlv",
        "\\",
        "cellx2000 ABC",
        "\\",
        "cell",
        "\\",
        "cltxbtlr",
        "\\",
        "cellx4000 XY",
        "\\",
        "cell",
        "\\",
        "cltxlrtb",
        "\\",
        "cellx6000 Flat",
        "\\",
        "cell",
        "\\",
        "row}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("A\nB\nC"));
    assert!(text.contains("Y\nX"));
    assert!(text.contains("Flat"));
    for forbidden in ["cltxtbrlv", "cltxbtlr", "cltxlrtb"] {
        assert!(
            !text.contains(forbidden),
            "forbidden table cell text-direction control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("table-cell-text-direction.rtf");
    let output_path = dir.path().join("table-cell-text-direction.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    for expected in ["A", "B", "C", "Y", "X"] {
        assert!(
            rendered_text.contains(expected),
            "decoded PDF text did not contain stacked cell text {expected:?}; got {rendered_text:?}"
        );
    }
    assert!(rendered_text.contains("Flat"));
    for forbidden in [
        b"cltxtbrlv".as_slice(),
        b"cltxbtlr",
        b"cltxlrtb",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden table cell text-direction content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn table_row_shading_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "colortbl;",
        "\\",
        "red220",
        "\\",
        "green230",
        "\\",
        "blue240;}",
        "\\",
        "trowd",
        "\\",
        "trcfpat1",
        "\\",
        "cellx1440 A",
        "\\",
        "cell",
        "\\",
        "cellx2880 B",
        "\\",
        "cell",
        "\\",
        "row}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("A"));
    assert!(text.contains("B"));
    for forbidden in ["trcbpat", "trcfpat"] {
        assert!(
            !text.contains(forbidden),
            "forbidden table row shading control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("table-row-shading.rtf");
    let output_path = dir.path().join("table-row-shading.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"trcbpat".as_slice(),
        b"trcfpat",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden table row shading content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn shading_intensity_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "colortbl;",
        "\\",
        "red255",
        "\\",
        "green0",
        "\\",
        "blue0;}",
        "\\",
        "cbpat1",
        "\\",
        "shading2500 paragraph",
        "\\",
        "par",
        "\\",
        "trowd",
        "\\",
        "trcbpat1",
        "\\",
        "trshdng5000",
        "\\",
        "cellx1440 row",
        "\\",
        "cell",
        "\\",
        "clcbpat1",
        "\\",
        "clshdng7500",
        "\\",
        "cellx2880 cell",
        "\\",
        "cell",
        "\\",
        "row}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("paragraph"));
    assert!(text.contains("row"));
    assert!(text.contains("cell"));
    for forbidden in [
        "shading", "trshdng", "clshdng", "cbpat", "trcbpat", "clcbpat",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden shading intensity control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("shading-intensity.rtf");
    let output_path = dir.path().join("shading-intensity.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"shading".as_slice(),
        b"trshdng",
        b"clshdng",
        b"cbpat",
        b"trcbpat",
        b"clcbpat",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden shading intensity content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn table_row_borders_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "colortbl;",
        "\\",
        "red255",
        "\\",
        "green0",
        "\\",
        "blue0;}",
        "\\",
        "trowd",
        "\\",
        "trbrdrt",
        "\\",
        "brdrdb",
        "\\",
        "brdrw80",
        "\\",
        "brdrcf1",
        "\\",
        "trbrdrl",
        "\\",
        "brdrdash",
        "\\",
        "brdrw40",
        "\\",
        "cellx1440 A",
        "\\",
        "cell",
        "\\",
        "cellx2880 B",
        "\\",
        "cell",
        "\\",
        "row}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("A"));
    assert!(text.contains("B"));
    for forbidden in [
        "trbrdrt", "trbrdrl", "brdrdb", "brdrdash", "brdrw", "brdrcf",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden table row border control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("table-row-borders.rtf");
    let output_path = dir.path().join("table-row-borders.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"trbrdrt".as_slice(),
        b"trbrdrl",
        b"brdrdb",
        b"brdrdash",
        b"brdrw",
        b"brdrcf",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden table row border content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn table_row_inner_borders_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "trowd",
        "\\",
        "trbrdrh",
        "\\",
        "brdrdb",
        "\\",
        "brdrw80",
        "\\",
        "trbrdrv",
        "\\",
        "brdrdot",
        "\\",
        "brdrw60",
        "\\",
        "cellx1440 A",
        "\\",
        "cell",
        "\\",
        "cellx2880 B",
        "\\",
        "cell",
        "\\",
        "cellx4320 C",
        "\\",
        "cell",
        "\\",
        "row}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    for expected in ["A", "B", "C"] {
        assert!(text.contains(expected));
    }
    for forbidden in ["trbrdrh", "trbrdrv", "brdrdb", "brdrdot", "brdrw"] {
        assert!(
            !text.contains(forbidden),
            "forbidden table row inner border control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("table-row-inner-borders.rtf");
    let output_path = dir.path().join("table-row-inner-borders.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = parsed_pdf.get_pages().into_values().next().unwrap();
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let stroke_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "S")
        .count();

    for expected in ["A", "B", "C"] {
        assert!(
            rendered_text.contains(expected),
            "decoded PDF text did not contain table cell text {expected:?}: {rendered_text:?}"
        );
    }
    assert!(
        stroke_count >= 3,
        "expected passive table inner border strokes, saw {stroke_count}"
    );
    for forbidden in [
        b"trbrdrh".as_slice(),
        b"trbrdrv",
        b"brdrdb",
        b"brdrdot",
        b"brdrw",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden table row inner border content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn table_cell_diagonal_borders_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "colortbl;",
        "\\",
        "red255",
        "\\",
        "green0",
        "\\",
        "blue0;}",
        "\\",
        "trowd",
        "\\",
        "cldgll",
        "\\",
        "brdrdash",
        "\\",
        "brdrw40",
        "\\",
        "brdrcf1",
        "\\",
        "cldglu",
        "\\",
        "brdrs",
        "\\",
        "brdrw30",
        "\\",
        "cellx1440 Diagonal",
        "\\",
        "cell",
        "\\",
        "row}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let table = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("table");
    let borders = table.rows[0].cells[0].borders;

    assert!(text.contains("Diagonal"));
    assert!(borders.diagonal_down.visible);
    assert_eq!(borders.diagonal_down.width_twips, 40);
    assert!(borders.diagonal_up.visible);
    assert_eq!(borders.diagonal_up.width_twips, 30);
    for forbidden in ["cldgll", "cldglu", "brdrdash", "brdrw", "brdrcf"] {
        assert!(
            !text.contains(forbidden),
            "forbidden diagonal border control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("table-cell-diagonal-borders.rtf");
    let output_path = dir.path().join("table-cell-diagonal-borders.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let stroke_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "S")
        .count();

    assert!(rendered_text.contains("Diagonal"));
    assert!(stroke_count >= 2);
    for forbidden in [
        b"cldgll".as_slice(),
        b"cldglu",
        b"brdrdash",
        b"brdrw",
        b"brdrcf",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden diagonal border content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn merged_table_cells_render_passively_without_continuation_or_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "trowd",
        "\\",
        "clmgf",
        "\\",
        "cellx1440 Merged",
        "\\",
        "cell",
        "\\",
        "clmrg",
        "\\",
        "cellx2880 Hidden horizontal continuation",
        "\\",
        "cell",
        "\\",
        "cellx4320 Plain",
        "\\",
        "cell",
        "\\",
        "row",
        "\\",
        "trowd",
        "\\",
        "clvmgf",
        "\\",
        "cellx1440 Top",
        "\\",
        "cell",
        "\\",
        "cellx2880 A",
        "\\",
        "cell",
        "\\",
        "row",
        "\\",
        "trowd",
        "\\",
        "clvmrg",
        "\\",
        "cellx1440 Hidden vertical continuation",
        "\\",
        "cell",
        "\\",
        "cellx2880 B",
        "\\",
        "cell",
        "\\",
        "row}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let table = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("table");

    assert!(text.contains("Merged"));
    assert_eq!(
        table.rows[0].cells[0].horizontal_merge,
        open_rtf_converter::model::TableCellHorizontalMerge::First
    );
    assert_eq!(
        table.rows[0].cells[1].horizontal_merge,
        open_rtf_converter::model::TableCellHorizontalMerge::Continuation
    );
    assert_eq!(
        table.rows[1].cells[0].vertical_merge,
        open_rtf_converter::model::TableCellVerticalMerge::First
    );
    assert_eq!(
        table.rows[2].cells[0].vertical_merge,
        open_rtf_converter::model::TableCellVerticalMerge::Continuation
    );
    for forbidden in ["clmgf", "clmrg", "clvmgf", "clvmrg"] {
        assert!(
            !text.contains(forbidden),
            "merged-cell control leaked to normalized text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("merged-table-cells.rtf");
    let output_path = dir.path().join("merged-table-cells.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let stroke_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "S")
        .count();

    for expected in ["Merged", "Plain", "Top", "A", "B"] {
        assert!(
            rendered_text.contains(expected),
            "merged table visible text {expected:?} missing from PDF text {rendered_text:?}"
        );
    }
    for forbidden in [
        "Hidden horizontal continuation",
        "Hidden vertical continuation",
        "clmgf",
        "clmrg",
        "clvmgf",
        "clvmrg",
    ] {
        assert!(
            !rendered_text.contains(forbidden),
            "merged-cell hidden/control text leaked to rendered PDF text: {forbidden}"
        );
    }
    assert!(
        stroke_count >= 8,
        "merged table should still render passive grid strokes"
    );
    for forbidden in [
        b"Hidden horizontal continuation".as_slice(),
        b"Hidden vertical continuation",
        b"clmgf",
        b"clmrg",
        b"clvmgf",
        b"clvmrg",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/AcroForm",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden merged-cell content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn rtl_table_rows_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "trowd",
        "\\",
        "taprtl",
        "\\",
        "cellx1440 Right cell",
        "\\",
        "cell",
        "\\",
        "cellx4320 Left wide cell",
        "\\",
        "cell",
        "\\",
        "row",
        "\\",
        "trowd",
        "\\",
        "rtlrow0",
        "\\",
        "cellx1440 Normal left",
        "\\",
        "cell",
        "\\",
        "cellx4320 Normal right",
        "\\",
        "cell",
        "\\",
        "row}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let table = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("table");

    assert_eq!(table.column_widths_twips, vec![2880, 1440]);
    assert_eq!(
        table.rows[0].cells[0].paragraphs[0].runs[0].text,
        "Left wide cell"
    );
    assert_eq!(
        table.rows[0].cells[1].paragraphs[0].runs[0].text,
        "Right cell"
    );
    assert_eq!(
        table.rows[1].cells[0].paragraphs[0].runs[0].text,
        "Normal left"
    );
    assert_eq!(
        table.rows[1].cells[1].paragraphs[0].runs[0].text,
        "Normal right"
    );
    assert!(text.find("Left wide cell") < text.find("Right cell"));
    assert!(!text.contains("taprtl"));
    assert!(!text.contains("rtlrow"));

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.find("Left wide cell") < rendered_text.find("Right cell"),
        "RTL row should render in visual order: {rendered_text:?}"
    );
    for expected in ["Normal left", "Normal right"] {
        assert!(
            rendered_text.contains(expected),
            "visible RTL table text {expected:?} missing from PDF text {rendered_text:?}"
        );
    }
    for forbidden in [
        b"taprtl".as_slice(),
        b"rtlrow",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden RTL table content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn list_marker_growth_is_bounded() {
    let options = RtfParseOptions {
        limits: RtfLimits {
            max_text_run_len: 4,
            ..RtfLimits::default()
        },
        ..RtfParseOptions::default()
    };

    assert!(matches!(
        parse_rtf_bytes_with_options(
            &rtf(&[
                "{",
                "\\",
                "rtf1{",
                "\\",
                "*",
                "\\",
                "listtext 1234",
                "\\",
                "tab}body",
                "\\",
                "par}"
            ]),
            &options
        ),
        Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "list marker text"
    ));
}

#[test]
fn old_style_list_marker_text_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "pn",
        "\\",
        "pndec",
        "\\",
        "pnstart3{",
        "\\",
        "pntxtb 3}{",
        "\\",
        "pntxta .",
        "\\",
        "tab}}Third item",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("3.\tThird item"));
    for forbidden in ["pntxtb", "pntxta", "pnstart", "pndec"] {
        assert!(
            !text.contains(forbidden),
            "forbidden old-style list control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("old-style-list.rtf");
    let output_path = dir.path().join("old-style-list.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"pntxtb".as_slice(),
        b"pntxta",
        b"pnstart",
        b"pndec",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn formatted_explicit_listtext_marker_renders_passively_without_control_leakage() {
    let input =
        br"{\rtf1{\colortbl;\red255\green0\blue0;}{\*\listtext\b\cf1 1.\tab}Styled explicit\par}"
            .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected list paragraph"),
    };

    assert_eq!(paragraph.runs[0].text, "1.\t");
    assert!(paragraph.runs[0].style.bold);
    assert_eq!(paragraph.runs[0].style.color_index, 1);
    assert_eq!(paragraph.runs[1].text, "Styled explicit");
    assert!(!paragraph.runs[1].style.bold);
    assert_eq!(paragraph.runs[1].style.color_index, 0);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("1.Styled explicit"),
        "decoded PDF text did not contain explicit styled marker text: {rendered_text:?}"
    );

    for forbidden in [
        b"listtext".as_slice(),
        b"pntext",
        b"cf1",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden explicit list marker control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn mixed_formatted_explicit_listtext_marker_renders_passively_without_control_leakage() {
    let input = br"{\rtf1{\colortbl;\red255\green0\blue0;}{\*\listtext\b 1\b0\cf1 .\cf0\tab}Styled explicit\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected list paragraph"),
    };

    assert_eq!(paragraph.runs[0].text, "1");
    assert!(paragraph.runs[0].style.bold);
    assert_eq!(paragraph.runs[0].style.color_index, 0);
    assert_eq!(paragraph.runs[1].text, ".");
    assert!(!paragraph.runs[1].style.bold);
    assert_eq!(paragraph.runs[1].style.color_index, 1);
    assert_eq!(paragraph.runs[2].text, "\tStyled explicit");
    assert!(!paragraph.runs[2].style.bold);
    assert_eq!(paragraph.runs[2].style.color_index, 0);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("1.Styled explicit"),
        "decoded PDF text did not contain mixed explicit marker text: {rendered_text:?}"
    );

    for forbidden in [
        b"listtext".as_slice(),
        b"pntext",
        b"cf1",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden mixed explicit list marker control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn ignored_pn_metadata_after_explicit_markers_does_not_leak_or_override_visible_marker() {
    let input = br"{\rtf1{\fonttbl{\f0 Arial;}{\f1\fcharset2 Symbol;}}{\*\pnseclvl1\pnucrm\pnstart1{\pntxta .}}Title text\par{\pntext\pard\plain\f1 \'b7\tab}{\*\pn\pnlvlblt\pnf1{\pntxtb \'b7}}\pard\fi-360\li360\tx360 Bullet item\par{\pntext I.\tab}{\*\pn\pnucrm{\pntxta .}}\pard\fi-720\li720\tx720 Roman item\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Title text"));
    assert!(text.contains("\u{2022}\tBullet item"));
    assert!(text.contains("I.\tRoman item"));
    assert!(!text.contains(".Title"));
    assert!(!text.contains("\u{2022}\u{2022}"));
    assert!(!text.contains("I.\t.Roman"));
    for forbidden in [
        "pntext", "pntxtb", "pntxta", "pnlvlblt", "pnseclvl", "pnucrm", "fonttbl",
    ] {
        assert!(
            !text.contains(forbidden),
            "ignored list metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "ignored list metadata should not be unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let helvetica_bytes = pdf_text_bytes_for_font(&content, b"F1");
    let symbol_bytes = pdf_text_bytes_for_font(&content, b"F13");
    assert!(
        helvetica_bytes.contains(&0x95),
        "explicit Symbol bullet should encode through passive WinAnsi bullet byte, got {helvetica_bytes:?}"
    );
    assert!(
        !symbol_bytes.contains(&0xb7),
        "ignored Symbol list metadata should not render through PDF Symbol bytes, got {symbol_bytes:?}"
    );
    assert!(
        rendered_text.contains("Title text"),
        "decoded PDF text did not contain title text: {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("Bullet item"),
        "decoded PDF text did not contain bullet item text: {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("I.Roman item"),
        "decoded PDF text did not contain explicit roman marker: {rendered_text:?}"
    );
    assert!(
        !rendered_text.contains("I..Roman"),
        "ignored pn suffix metadata should not become visible punctuation: {rendered_text:?}"
    );
    assert!(
        !rendered_text.contains(".Title"),
        "ignored pn section-level metadata should not prefix body text: {rendered_text:?}"
    );

    for forbidden in [
        b"pntext".as_slice(),
        b"pntxtb",
        b"pntxta",
        b"pnlvlblt",
        b"pnseclvl",
        b"pnucrm",
        b"fonttbl",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/AcroForm",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "ignored list metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn old_style_list_format_controls_synthesize_passive_markers_without_leakage() {
    let input = br"{\rtf1{\pn\pnucrm\pnstart4}Fourth item\par{\pn\pnlcltr\pnstart28}Lower alpha\par{\pn\pnord\pnstart13}Ordinal\par{\pn\pnbul}Bullet\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("IV.\tFourth item"));
    assert!(text.contains("ab.\tLower alpha"));
    assert!(text.contains("13th.\tOrdinal"));
    assert!(text.contains("\u{2022}\tBullet"));
    for forbidden in ["pnucrm", "pnlcltr", "pnord", "pnbul", "pnstart"] {
        assert!(
            !text.contains(forbidden),
            "forbidden old-style list control leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("IV.Fourth item")
            && rendered_text.contains("ab.Lower alpha")
            && rendered_text.contains("13th.Ordinal")
            && rendered_text.contains("Bullet"),
        "decoded PDF text did not contain synthesized old-style list markers: {rendered_text:?}"
    );
    for forbidden in [
        b"pnucrm".as_slice(),
        b"pnlcltr",
        b"pnord",
        b"pnbul",
        b"pnstart",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden old-style list content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn old_style_list_indent_controls_render_passively_without_control_leakage() {
    let input = br"{\rtf1{\pn\pndec\pnstart1\pnindent720\pnhang}Indented item\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected old-style list paragraph"),
    };

    assert_eq!(paragraph.runs[0].text, "1.\tIndented item");
    assert_eq!(paragraph.style.left_indent_twips, 720);
    assert_eq!(paragraph.style.first_line_indent_twips, -360);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("1.Indented item"),
        "decoded PDF text did not contain indented old-style list marker: {rendered_text:?}"
    );

    for forbidden in [
        b"pnindent".as_slice(),
        b"pnhang",
        b"pnstart",
        b"pndec",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden old-style list indent content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn old_style_list_spacing_renders_passively_without_control_leakage() {
    let input = br"{\rtf1{\pn\pndec\pnstart1\pnindent720\pnhang\pnsp360}Spaced item\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected old-style list paragraph"),
    };

    assert_eq!(paragraph.runs[0].text, "1.\tSpaced item");
    assert_eq!(paragraph.style.left_indent_twips, 720);
    assert_eq!(paragraph.style.first_line_indent_twips, -360);
    assert_eq!(paragraph.style.tab_stops_twips, vec![360]);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("1.Spaced item"),
        "decoded PDF text did not contain spaced old-style list marker: {rendered_text:?}"
    );

    for forbidden in [
        b"pnsp".as_slice(),
        b"pnindent",
        b"pnhang",
        b"pnstart",
        b"pndec",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden old-style list spacing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn old_style_list_marker_formatting_renders_passively_without_control_leakage() {
    let input =
        br"{\rtf1{\colortbl;\red255\green0\blue0;}{\pn\pndec\pnb\pni\pnul\pnstrike\pncaps\pncf1\pnfs28}Formatted item\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected old-style list paragraph"),
    };

    assert_eq!(paragraph.runs[0].text, "1.\t");
    assert!(paragraph.runs[0].style.bold);
    assert!(paragraph.runs[0].style.italic);
    assert_eq!(paragraph.runs[0].style.underline, UnderlineStyle::Single);
    assert!(paragraph.runs[0].style.strike);
    assert!(paragraph.runs[0].style.all_caps);
    assert_eq!(paragraph.runs[0].style.color_index, 1);
    assert_eq!(paragraph.runs[0].style.font_size_half_points, 28);
    assert_eq!(paragraph.runs[1].text, "Formatted item");
    assert!(!paragraph.runs[1].style.bold);
    assert!(!paragraph.runs[1].style.italic);
    assert_eq!(paragraph.runs[1].style.underline, UnderlineStyle::None);
    assert!(!paragraph.runs[1].style.strike);
    assert_eq!(paragraph.runs[1].style.color_index, 0);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("1.Formatted item"),
        "decoded PDF text did not contain formatted old-style list marker: {rendered_text:?}"
    );

    for forbidden in [
        b"pnb".as_slice(),
        b"pni",
        b"pnul",
        b"pnstrike",
        b"pncaps",
        b"pncf",
        b"pnfs",
        b"pndec",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden old-style list marker formatting leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn roman_and_alpha_list_markers_render_as_passive_pdf_text() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "*",
        "\\",
        "listtable{",
        "\\",
        "list{",
        "\\",
        "listlevel",
        "\\",
        "levelnfc1",
        "\\",
        "levelstartat4{",
        "\\",
        "leveltext",
        "\\",
        "'02",
        "\\",
        "'00.;}{",
        "\\",
        "levelnumbers",
        "\\",
        "'01;}}",
        "\\",
        "listid5}}{",
        "\\",
        "*",
        "\\",
        "listoverridetable{",
        "\\",
        "listoverride",
        "\\",
        "listid5",
        "\\",
        "ls1}}",
        "\\",
        "pard",
        "\\",
        "ls1",
        "\\",
        "ilvl0 Item",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("IV.\tItem"));
    assert!(!text.contains("levelnfc"));
    assert!(!text.contains("listtable"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("roman-list.rtf");
    let output_path = dir.path().join("roman-list.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"levelnfc".as_slice(),
        b"listtable",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn list_level_indent_and_spacing_render_passively_without_control_leakage() {
    let input = br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\levelindent720\levelspace1080{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Indented\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };

    assert_eq!(paragraph.runs[0].text, "1.\tIndented");
    assert_eq!(paragraph.style.left_indent_twips, 720);
    assert_eq!(paragraph.style.tab_stops_twips, vec![1080]);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("1.Indented"),
        "decoded PDF text did not contain indented list-table marker: {rendered_text:?}"
    );

    for forbidden in [
        b"levelindent".as_slice(),
        b"levelspace",
        b"levelnfc",
        b"leveltext",
        b"levelnumbers",
        b"listtable",
        b"listoverridetable",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden list-level geometry content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn list_level_follow_controls_render_passively_without_control_leakage() {
    let input = br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelfollow1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}{\list{\listlevel\levelnfc0\levelfollow2{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid6}}{\*\listoverridetable{\listoverride\listid5\ls5}{\listoverride\listid6\ls6}}\pard\ls5\ilvl0 Space\par\pard\ls6\ilvl0 Nothing\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let first = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    let second = match &parsed.document.blocks[1] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };

    assert_eq!(first.runs[0].text, "1. Space");
    assert_eq!(second.runs[0].text, "1.Nothing");

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("1. Space") && rendered_text.contains("1.Nothing"),
        "decoded PDF text did not contain followed list-table markers: {rendered_text:?}"
    );

    for forbidden in [
        b"levelfollow".as_slice(),
        b"levelnfc",
        b"leveltext",
        b"levelnumbers",
        b"listtable",
        b"listoverridetable",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden list-level follow content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn list_level_marker_formatting_renders_passively_without_control_leakage() {
    let input = br"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Courier New;}}{\colortbl;\red255\green0\blue0;\red255\green255\blue0;\red0\green0\blue255;}{\*\listtable{\list{\listlevel\levelnfc0\f1\fs28\b\i\ul\ulc3\strike\caps\cf1\chshdng5000\chcbpat2{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Styled item\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };

    assert_eq!(paragraph.runs[0].text, "1.\t");
    assert!(paragraph.runs[0].style.bold);
    assert!(paragraph.runs[0].style.italic);
    assert_eq!(paragraph.runs[0].style.underline, UnderlineStyle::Single);
    assert!(paragraph.runs[0].style.strike);
    assert!(paragraph.runs[0].style.all_caps);
    assert_eq!(paragraph.runs[0].style.font_index, 1);
    assert_eq!(paragraph.runs[0].style.font_size_half_points, 28);
    assert_eq!(paragraph.runs[0].style.color_index, 1);
    assert_eq!(paragraph.runs[0].style.highlight_index, Some(2));
    assert_eq!(
        paragraph.runs[0].style.highlight_shading_basis_points,
        5_000
    );
    assert_eq!(paragraph.runs[0].style.underline_color_index, Some(3));
    assert_eq!(paragraph.runs[1].text, "Styled item");
    assert!(!paragraph.runs[1].style.bold);
    assert!(!paragraph.runs[1].style.italic);
    assert_eq!(paragraph.runs[1].style.color_index, 0);
    assert_eq!(paragraph.runs[1].style.highlight_index, None);
    assert_eq!(paragraph.runs[1].style.underline_color_index, None);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("1.Styled item"),
        "decoded PDF text did not contain styled list-table marker: {rendered_text:?}"
    );

    for forbidden in [
        b"levelnfc".as_slice(),
        b"leveltext",
        b"levelnumbers",
        b"ulc",
        b"chcbpat",
        b"chshdng",
        b"listtable",
        b"listoverridetable",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden list-level marker formatting leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn list_level_marker_double_strike_renders_passively_without_control_leakage() {
    let input = br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\strikedl{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Double strike item\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };

    assert_eq!(paragraph.runs[0].text, "1.\t");
    assert!(paragraph.runs[0].style.strike);
    assert!(paragraph.runs[0].style.double_strike);
    assert_eq!(paragraph.runs[1].text, "Double strike item");
    assert!(!paragraph.runs[1].style.strike);
    assert!(!paragraph.runs[1].style.double_strike);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("1.Double strike item"),
        "decoded PDF text did not contain double-struck list-table marker: {rendered_text:?}"
    );

    for forbidden in [
        b"strikedl".as_slice(),
        b"levelnfc",
        b"leveltext",
        b"levelnumbers",
        b"listtable",
        b"listoverridetable",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden list-level marker double-strike leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn list_level_marker_underline_variants_render_passively_without_control_leakage() {
    let input = br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\uldb{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}{\list{\listlevel\levelnfc0\ulth{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid6}{\list{\listlevel\levelnfc0\uld{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid7}{\list{\listlevel\levelnfc0\uldash{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid8}{\list{\listlevel\levelnfc0\ulwave{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid9}{\list{\listlevel\levelnfc0\ulw{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid10}}{\*\listoverridetable{\listoverride\listid5\ls1}{\listoverride\listid6\ls2}{\listoverride\listid7\ls3}{\listoverride\listid8\ls4}{\listoverride\listid9\ls5}{\listoverride\listid10\ls6}}\pard\ls1\ilvl0 Double marker\par\pard\ls2\ilvl0 Thick marker\par\pard\ls3\ilvl0 Dotted marker\par\pard\ls4\ilvl0 Dashed marker\par\pard\ls5\ilvl0 Wave marker\par\pard\ls6\ilvl0 Words marker\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();

    for (index, (text, underline)) in [
        ("Double marker", UnderlineStyle::Double),
        ("Thick marker", UnderlineStyle::Thick),
        ("Dotted marker", UnderlineStyle::Dotted),
        ("Dashed marker", UnderlineStyle::Dashed),
        ("Wave marker", UnderlineStyle::Wave),
        ("Words marker", UnderlineStyle::Words),
    ]
    .into_iter()
    .enumerate()
    {
        let paragraph = match &parsed.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "1.\t");
        assert_eq!(paragraph.runs[0].style.underline, underline);
        assert_eq!(paragraph.runs[1].text, text);
        assert_eq!(paragraph.runs[1].style.underline, UnderlineStyle::None);
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    for expected in [
        "1.Double marker",
        "1.Thick marker",
        "1.Dotted marker",
        "1.Dashed marker",
        "1.Wave marker",
        "1.Words marker",
    ] {
        assert!(
            rendered_text.contains(expected),
            "decoded PDF text did not contain underlined marker text {expected:?}: {rendered_text:?}"
        );
    }

    for forbidden in [
        b"uldb".as_slice(),
        b"ulth",
        b"uldash",
        b"ulwave",
        b"levelnfc",
        b"leveltext",
        b"levelnumbers",
        b"listtable",
        b"listoverridetable",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden list-level marker underline control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn list_level_marker_text_effects_render_passively_without_control_leakage() {
    let input = br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\outl{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}{\list{\listlevel\levelnfc0\shad{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid6}{\list{\listlevel\levelnfc0\embo{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid7}{\list{\listlevel\levelnfc0\impr{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid8}{\list{\listlevel\levelnfc0\scaps{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid9}}{\*\listoverridetable{\listoverride\listid5\ls1}{\listoverride\listid6\ls2}{\listoverride\listid7\ls3}{\listoverride\listid8\ls4}{\listoverride\listid9\ls5}}\pard\ls1\ilvl0 Outline marker\par\pard\ls2\ilvl0 Shadow marker\par\pard\ls3\ilvl0 Emboss marker\par\pard\ls4\ilvl0 Engrave marker\par\pard\ls5\ilvl0 Small caps marker\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();

    for (index, text) in [
        "Outline marker",
        "Shadow marker",
        "Emboss marker",
        "Engrave marker",
        "Small caps marker",
    ]
    .into_iter()
    .enumerate()
    {
        let paragraph = match &parsed.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "1.\t");
        assert_eq!(paragraph.runs[1].text, text);
        assert!(!paragraph.runs[1].style.outline);
        assert!(!paragraph.runs[1].style.shadow);
        assert_eq!(paragraph.runs[1].style.relief, TextRelief::None);
        assert!(!paragraph.runs[1].style.small_caps);
    }

    let first = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    let second = match &parsed.document.blocks[1] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    let third = match &parsed.document.blocks[2] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    let fourth = match &parsed.document.blocks[3] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    let fifth = match &parsed.document.blocks[4] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    assert!(first.runs[0].style.outline);
    assert!(second.runs[0].style.shadow);
    assert_eq!(third.runs[0].style.relief, TextRelief::Emboss);
    assert_eq!(fourth.runs[0].style.relief, TextRelief::Engrave);
    assert!(fifth.runs[0].style.small_caps);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    for expected in [
        "1.Outline marker",
        "1.Shadow marker",
        "1.Emboss marker",
        "1.Engrave marker",
        "1.Small caps marker",
    ] {
        assert!(
            rendered_text.contains(expected),
            "decoded PDF text did not contain passive marker effect text {expected:?}: {rendered_text:?}"
        );
    }

    for forbidden in [
        b"outl".as_slice(),
        b"shad",
        b"embo",
        b"impr",
        b"scaps",
        b"levelnfc",
        b"leveltext",
        b"levelnumbers",
        b"listtable",
        b"listoverridetable",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden list-level marker text effect leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn list_level_marker_script_and_spacing_render_passively_without_control_leakage() {
    let input = br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\super{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}{\list{\listlevel\levelnfc0\sub{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid6}{\list{\listlevel\levelnfc0\up8{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid7}{\list{\listlevel\levelnfc0\dn6{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid8}{\list{\listlevel\levelnfc0\expndtw80{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid9}{\list{\listlevel\levelnfc0\kerning2{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid10}{\list{\listlevel\levelnfc0\charscalex150{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid11}}{\*\listoverridetable{\listoverride\listid5\ls1}{\listoverride\listid6\ls2}{\listoverride\listid7\ls3}{\listoverride\listid8\ls4}{\listoverride\listid9\ls5}{\listoverride\listid10\ls6}{\listoverride\listid11\ls7}}\pard\ls1\ilvl0 Raised marker\par\pard\ls2\ilvl0 Lowered marker\par\pard\ls3\ilvl0 Manual up marker\par\pard\ls4\ilvl0 Manual down marker\par\pard\ls5\ilvl0 Spaced marker\par\pard\ls6\ilvl0 Kerned marker\par\pard\ls7\ilvl0 Scaled marker\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();

    for (index, text) in [
        "Raised marker",
        "Lowered marker",
        "Manual up marker",
        "Manual down marker",
        "Spaced marker",
        "Kerned marker",
        "Scaled marker",
    ]
    .into_iter()
    .enumerate()
    {
        let paragraph = match &parsed.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "1.\t");
        assert_eq!(paragraph.runs[1].text, text);
        assert_eq!(paragraph.runs[1].style.baseline_shift_half_points, 0);
        assert_eq!(paragraph.runs[1].style.font_size_scale_percent, 100);
        assert_eq!(paragraph.runs[1].style.character_spacing_twips, 0);
        assert_eq!(paragraph.runs[1].style.character_kerning_half_points, 0);
        assert_eq!(paragraph.runs[1].style.character_scaling_percent, 100);
    }

    let first = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    let second = match &parsed.document.blocks[1] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    let third = match &parsed.document.blocks[2] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    let fourth = match &parsed.document.blocks[3] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    let fifth = match &parsed.document.blocks[4] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    let sixth = match &parsed.document.blocks[5] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    let seventh = match &parsed.document.blocks[6] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    assert!(first.runs[0].style.baseline_shift_half_points > 0);
    assert!(second.runs[0].style.baseline_shift_half_points < 0);
    assert_eq!(third.runs[0].style.baseline_shift_half_points, 8);
    assert_eq!(fourth.runs[0].style.baseline_shift_half_points, -6);
    assert_eq!(fifth.runs[0].style.character_spacing_twips, 80);
    assert_eq!(sixth.runs[0].style.character_kerning_half_points, 2);
    assert_eq!(seventh.runs[0].style.character_scaling_percent, 150);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    for expected in [
        "1.Raised marker",
        "1.Lowered marker",
        "1.Manual up marker",
        "1.Manual down marker",
        "1.Spaced marker",
        "1.Kerned marker",
        "1.Scaled marker",
    ] {
        assert!(
            rendered_text.contains(expected),
            "decoded PDF text did not contain passive marker script/spacing text {expected:?}: {rendered_text:?}"
        );
    }

    for forbidden in [
        b"super".as_slice(),
        b"sub",
        b"expndtw",
        b"kerning",
        b"charscalex",
        b"levelnfc",
        b"leveltext",
        b"levelnumbers",
        b"listtable",
        b"listoverridetable",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden list-level marker script/spacing control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn list_level_marker_character_border_renders_passively_without_control_leakage() {
    let input = br"{\rtf1{\colortbl;\red255\green0\blue0;}{\*\listtable{\list{\listlevel\levelnfc0\chbrdr\brdrdash\brdrw80\brdrcf1\brsp120{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Bordered marker\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };

    assert_eq!(paragraph.runs[0].text, "1.\t");
    assert!(paragraph.runs[0].style.border.visible);
    assert_eq!(paragraph.runs[0].style.border.style, BorderStyle::Dashed);
    assert_eq!(paragraph.runs[0].style.border.width_twips, 80);
    assert_eq!(paragraph.runs[0].style.border.color_index, Some(1));
    assert_eq!(paragraph.runs[0].style.border.spacing_twips, 120);
    assert_eq!(paragraph.runs[1].text, "Bordered marker");
    assert!(!paragraph.runs[1].style.border.visible);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("1.Bordered marker"),
        "decoded PDF text did not contain bordered marker text: {rendered_text:?}"
    );

    for forbidden in [
        b"chbrdr".as_slice(),
        b"brdrdash",
        b"brdrw",
        b"brdrcf",
        b"brsp",
        b"levelnfc",
        b"leveltext",
        b"levelnumbers",
        b"listtable",
        b"listoverridetable",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden list-level marker border control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn list_level_marker_plain_reset_renders_passively_without_control_leakage() {
    let input = br"{\rtf1{\colortbl;\red255\green0\blue0;}{\*\listtable{\list{\listlevel\levelnfc0\b\ul\chbrdr\brdrs\brdrw80\plain\i\cf1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Reset item\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };

    assert_eq!(paragraph.runs[0].text, "1.\t");
    assert!(!paragraph.runs[0].style.bold);
    assert!(paragraph.runs[0].style.italic);
    assert_eq!(paragraph.runs[0].style.underline, UnderlineStyle::None);
    assert_eq!(paragraph.runs[0].style.color_index, 1);
    assert!(!paragraph.runs[0].style.border.visible);
    assert_eq!(paragraph.runs[1].text, "Reset item");
    assert!(!paragraph.runs[1].style.bold);
    assert!(!paragraph.runs[1].style.italic);
    assert_eq!(paragraph.runs[1].style.color_index, 0);
    assert!(!paragraph.runs[1].style.border.visible);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("1.Reset item"),
        "decoded PDF text did not contain plain-reset marker text: {rendered_text:?}"
    );

    for forbidden in [
        b"plain".as_slice(),
        b"levelnfc",
        b"leveltext",
        b"levelnumbers",
        b"listtable",
        b"listoverridetable",
        b"chbrdr",
        b"brdrs",
        b"brdrw",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden list-level marker plain-reset control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn list_level_marker_character_style_renders_passively_without_control_leakage() {
    let input = br"{\rtf1{\colortbl;\red255\green0\blue0;}{\stylesheet{\cs5\b\ul\cf1 Marker emphasis;}}{\*\listtable{\list{\listlevel\levelnfc0\i\cs5{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Styled marker\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };

    assert_eq!(paragraph.runs[0].text, "1.\t");
    assert!(paragraph.runs[0].style.bold);
    assert!(paragraph.runs[0].style.italic);
    assert_eq!(paragraph.runs[0].style.underline, UnderlineStyle::Single);
    assert_eq!(paragraph.runs[0].style.color_index, 1);
    assert_eq!(paragraph.runs[1].text, "Styled marker");
    assert!(!paragraph.runs[1].style.bold);
    assert!(!paragraph.runs[1].style.italic);
    assert_eq!(paragraph.runs[1].style.underline, UnderlineStyle::None);
    assert_eq!(paragraph.runs[1].style.color_index, 0);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("1.Styled marker"),
        "decoded PDF text did not contain styled marker text: {rendered_text:?}"
    );

    for forbidden in [
        b"stylesheet".as_slice(),
        b"cs5",
        b"Marker emphasis",
        b"levelnfc",
        b"leveltext",
        b"levelnumbers",
        b"listtable",
        b"listoverridetable",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden list-level marker character style control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn list_level_legal_numbering_renders_passively_without_control_leakage() {
    let input = br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc1\levelstartat4{\leveltext\'02\'00.;}{\levelnumbers\'01;}}{\listlevel\levelnfc0\levellegal1\levelstartat1{\leveltext\'04\'00.\'01.;}{\levelnumbers\'01\'03;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Parent\par\pard\ls1\ilvl1 Child\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let first = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    let second = match &parsed.document.blocks[1] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };

    assert_eq!(first.runs[0].text, "IV.\tParent");
    assert_eq!(second.runs[0].text, "4.1.\tChild");

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("IV.Parent") && rendered_text.contains("4.1.Child"),
        "decoded PDF text did not contain legal-numbered list markers: {rendered_text:?}"
    );

    for forbidden in [
        b"levellegal".as_slice(),
        b"levelnfc",
        b"leveltext",
        b"levelnumbers",
        b"listtable",
        b"listoverridetable",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden legal-list content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn list_level_no_restart_renders_passively_without_control_leakage() {
    let input = br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}{\listlevel\levelnfc0\levelnorestart1\levelstartat1{\leveltext\'04\'00.\'01.;}{\levelnumbers\'01\'03;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Top\par\pard\ls1\ilvl1 Child\par\pard\ls1\ilvl0 Next top\par\pard\ls1\ilvl1 Continued child\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("1.\tTop"));
    assert!(text.contains("1.1.\tChild"));
    assert!(text.contains("2.\tNext top"));
    assert!(text.contains("2.2.\tContinued child"));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("1.Top")
            && rendered_text.contains("1.1.Child")
            && rendered_text.contains("2.Next top")
            && rendered_text.contains("2.2.Continued child"),
        "decoded PDF text did not contain no-restart list markers: {rendered_text:?}"
    );

    for forbidden in [
        b"levelnorestart".as_slice(),
        b"levelnfc",
        b"leveltext",
        b"levelnumbers",
        b"listtable",
        b"listoverridetable",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden no-restart list content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn ordinal_list_markers_render_as_passive_pdf_text() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "*",
        "\\",
        "listtable{",
        "\\",
        "list{",
        "\\",
        "listlevel",
        "\\",
        "levelnfc5",
        "\\",
        "levelstartat11{",
        "\\",
        "leveltext",
        "\\",
        "'02",
        "\\",
        "'00.;}{",
        "\\",
        "levelnumbers",
        "\\",
        "'01;}}",
        "\\",
        "listid15}}{",
        "\\",
        "*",
        "\\",
        "listoverridetable{",
        "\\",
        "listoverride",
        "\\",
        "listid15",
        "\\",
        "ls15}}",
        "\\",
        "pard",
        "\\",
        "ls15",
        "\\",
        "ilvl0 Eleventh",
        "\\",
        "par",
        "\\",
        "pard",
        "\\",
        "ls15",
        "\\",
        "ilvl0 Twelfth",
        "\\",
        "par",
        "\\",
        "pard",
        "\\",
        "ls15",
        "\\",
        "ilvl0 Thirteenth",
        "\\",
        "par} ",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("11th.\tEleventh"));
    assert!(text.contains("12th.\tTwelfth"));
    assert!(text.contains("13th.\tThirteenth"));
    assert!(!text.contains("levelnfc"));
    assert!(!text.contains("listtable"));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("11th.Eleventh")
            && rendered_text.contains("12th.Twelfth")
            && rendered_text.contains("13th.Thirteenth"),
        "decoded PDF text did not contain ordinal list markers: {rendered_text:?}"
    );
    for forbidden in [
        b"levelnfc".as_slice(),
        b"leveltext",
        b"levelnumbers",
        b"listtable",
        b"listoverridetable",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden ordinal-list content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn zero_padded_list_markers_render_as_passive_pdf_text() {
    let input = br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc22\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid16}{\list{\listlevel\levelnfc63\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid18}}{\*\listoverridetable{\listoverride\listid16\ls16}{\listoverride\listid18\ls18}}\pard\ls16\ilvl0 Two digits\par\pard\ls18\ilvl0 Four digits\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("01.\tTwo digits"));
    assert!(text.contains("0001.\tFour digits"));
    assert!(!text.contains("levelnfc"));
    assert!(!text.contains("listtable"));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("01.Two digits") && rendered_text.contains("0001.Four digits"),
        "decoded PDF text did not contain zero-padded list markers: {rendered_text:?}"
    );
    for forbidden in [
        b"levelnfc".as_slice(),
        b"leveltext",
        b"levelnumbers",
        b"listtable",
        b"listoverridetable",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden zero-padded list content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn list_override_start_values_render_as_passive_pdf_text() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "*",
        "\\",
        "listtable{",
        "\\",
        "list{",
        "\\",
        "listlevel",
        "\\",
        "levelnfc0",
        "\\",
        "levelstartat1{",
        "\\",
        "leveltext",
        "\\",
        "'02",
        "\\",
        "'00.;}{",
        "\\",
        "levelnumbers",
        "\\",
        "'01;}}",
        "\\",
        "listid5}}{",
        "\\",
        "*",
        "\\",
        "listoverridetable{",
        "\\",
        "listoverride",
        "\\",
        "listid5{",
        "\\",
        "lfolevel",
        "\\",
        "listoverridestartat",
        "\\",
        "levelstartat3}",
        "\\",
        "ls1}}",
        "\\",
        "pard",
        "\\",
        "ls1",
        "\\",
        "ilvl0 First",
        "\\",
        "par",
        "\\",
        "pard",
        "\\",
        "ls1",
        "\\",
        "ilvl0 Second",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("3.\tFirst"));
    assert!(text.contains("4.\tSecond"));
    assert!(!text.contains("listoverridestartat"));
    assert!(!text.contains("lfolevel"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("override-start-list.rtf");
    let output_path = dir.path().join("override-start-list.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"listoverridestartat".as_slice(),
        b"lfolevel",
        b"listoverridetable",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn list_override_format_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "*",
        "\\",
        "listtable{",
        "\\",
        "list{",
        "\\",
        "listlevel",
        "\\",
        "levelnfc0",
        "\\",
        "levelstartat1{",
        "\\",
        "leveltext",
        "\\",
        "'02",
        "\\",
        "'00.;}{",
        "\\",
        "levelnumbers",
        "\\",
        "'01;}}",
        "\\",
        "listid5}}{",
        "\\",
        "*",
        "\\",
        "listoverridetable{",
        "\\",
        "listoverride",
        "\\",
        "listid5{",
        "\\",
        "lfolevel",
        "\\",
        "listoverrideformat{",
        "\\",
        "listlevel",
        "\\",
        "levelnfc4",
        "\\",
        "levelstartat3{",
        "\\",
        "leveltext",
        "\\",
        "'02",
        "\\",
        "'00);}{",
        "\\",
        "levelnumbers",
        "\\",
        "'01;}}}",
        "\\",
        "ls1}}",
        "\\",
        "pard",
        "\\",
        "ls1",
        "\\",
        "ilvl0 First",
        "\\",
        "par",
        "\\",
        "pard",
        "\\",
        "ls1",
        "\\",
        "ilvl0 Second",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("c)\tFirst"));
    assert!(text.contains("d)\tSecond"));
    for forbidden in [
        "listoverrideformat",
        "lfolevel",
        "levelnfc",
        "listoverridetable",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden list override format control leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("c)First"),
        "decoded PDF text did not contain override formatted first marker: {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("d)Second"),
        "decoded PDF text did not contain override formatted second marker: {rendered_text:?}"
    );
    for forbidden in [
        b"listoverrideformat".as_slice(),
        b"lfolevel",
        b"levelnfc",
        b"listoverridetable",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden list override format content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn styled_list_override_format_renders_passively_without_control_leakage() {
    let input = br"{\rtf1{\colortbl;\red255\green0\blue0;}{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5{\lfolevel\listoverrideformat{\listlevel\levelnfc4\b\cf1\levelstartat3{\leveltext\'02\'00);}{\levelnumbers\'01;}}}\ls1}}\pard\ls1\ilvl0 Styled override\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };

    assert_eq!(paragraph.runs[0].text, "c)\t");
    assert!(paragraph.runs[0].style.bold);
    assert_eq!(paragraph.runs[0].style.color_index, 1);
    assert_eq!(paragraph.runs[1].text, "Styled override");
    assert!(!paragraph.runs[1].style.bold);
    assert_eq!(paragraph.runs[1].style.color_index, 0);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("c)Styled override"),
        "decoded PDF text did not contain styled override marker text: {rendered_text:?}"
    );

    for forbidden in [
        b"listoverrideformat".as_slice(),
        b"lfolevel",
        b"levelnfc",
        b"listoverridetable",
        b"cf1",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden styled list override format content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn multilevel_list_markers_render_as_passive_pdf_text() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "*",
        "\\",
        "listtable{",
        "\\",
        "list{",
        "\\",
        "listlevel",
        "\\",
        "levelnfc0",
        "\\",
        "levelstartat1{",
        "\\",
        "leveltext",
        "\\",
        "'02",
        "\\",
        "'00.;}{",
        "\\",
        "levelnumbers",
        "\\",
        "'01;}}{",
        "\\",
        "listlevel",
        "\\",
        "levelnfc0",
        "\\",
        "levelstartat1{",
        "\\",
        "leveltext",
        "\\",
        "'04",
        "\\",
        "'00.",
        "\\",
        "'01.;}{",
        "\\",
        "levelnumbers",
        "\\",
        "'01",
        "\\",
        "'03;}}",
        "\\",
        "listid5}}{",
        "\\",
        "*",
        "\\",
        "listoverridetable{",
        "\\",
        "listoverride",
        "\\",
        "listid5",
        "\\",
        "ls1}}",
        "\\",
        "pard",
        "\\",
        "ls1",
        "\\",
        "ilvl0 Top",
        "\\",
        "par",
        "\\",
        "pard",
        "\\",
        "ls1",
        "\\",
        "ilvl1 Child",
        "\\",
        "par",
        "\\",
        "pard",
        "\\",
        "ls1",
        "\\",
        "ilvl1 Child two",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("1.\tTop"));
    assert!(text.contains("1.1.\tChild"));
    assert!(text.contains("1.2.\tChild two"));
    assert!(!text.contains("levelnumbers"));
    assert!(!text.contains("listtable"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("multilevel-list.rtf");
    let output_path = dir.path().join("multilevel-list.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"levelnumbers".as_slice(),
        b"leveltext",
        b"listtable",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn carried_table_row_definitions_render_passively_without_control_leakage() {
    let input = br"{\rtf1\trowd\trgaph108\trbrdrb\brdrs\brdrw20\clbrdrl\brdrs\brdrw30\clbrdrt\brdrs\brdrw30\clbrdrb\brdrs\brdrw30\clbrdrr\brdrs\brdrw30\cellx1200\clbrdrr\brdrs\brdrw30\cellx2400 A\cell B\cell\row\pard\intbl C\cell D\cell\row After\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("A"));
    assert!(text.contains("B"));
    assert!(text.contains("C"));
    assert!(text.contains("D"));
    assert!(text.contains("After"));
    for forbidden in ["trowd", "trgaph", "cellx", "intbl", "brdrw"] {
        assert!(
            !text.contains(forbidden),
            "forbidden carried-table control leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("A"));
    assert!(rendered_text.contains("B"));
    assert!(rendered_text.contains("C"));
    assert!(rendered_text.contains("D"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "S"),
        "carried table borders should render as passive stroked lines"
    );
    for forbidden in [
        b"trowd".as_slice(),
        b"trgaph",
        b"cellx",
        b"intbl",
        b"brdrw",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden carried-table content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn active_object_data_is_placeholdered_or_rejected_and_never_normalized() {
    let parsed = parse_rtf_bytes(&object_with_payload(false)).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("[Embedded object removed]"));
    assert!(!text.contains(payload_hex()));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("active content removed: OLE object before safe model normalization")
    }));

    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };
    assert!(matches!(
        parse_rtf_bytes_with_options(&object_with_payload(false), &reject_options),
        Err(ParseError::ActiveContentRejected { .. })
    ));
}

#[test]
fn embedded_object_result_is_rendered_without_objdata() {
    let parsed = parse_rtf_bytes(&object_with_payload(true)).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("visible fallback"));
    assert!(!text.contains("[Embedded object removed]"));
    assert!(!text.contains(payload_hex()));
}

#[test]
fn dimensioned_embedded_object_without_result_renders_passive_geometry_placeholder() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "object",
        "\\",
        "objw2160",
        "\\",
        "objh720",
        "\\",
        "objemb",
        "\\",
        "objdata 4142432f4a6176615363726970742f456d62656464656446696c65} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert!(!text.contains("[Embedded object removed]"));
    assert!(
        parsed.document.blocks.iter().any(|block| matches!(
            block,
            Block::Image(image)
                if image.format == ImageFormat::Placeholder
                    && image.bytes.is_empty()
                    && image.display_width_twips == Some(2160)
                    && image.display_height_twips == Some(720)
        )),
        "dimensioned active object should become a passive geometry placeholder"
    );
    for forbidden in ["objdata", "objemb", "414243", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "object internals leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("Image skipped"));
    assert!(rendered_text.contains("After"));
    for forbidden in [
        b"objdata".as_slice(),
        b"objemb",
        b"414243",
        b"JavaScript",
        b"EmbeddedFile",
        b"/Subtype /Image",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "dimensioned object leaked active PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn dimensioned_object_empty_picture_result_renders_passive_geometry_placeholder() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "object",
        "\\",
        "objw2160",
        "\\",
        "objh720",
        "\\",
        "objemb{",
        "\\",
        "objdata 4142432f4a6176615363726970742f456d62656464656446696c65}{",
        "\\",
        "result{",
        "\\",
        "pict",
        "\\",
        "wmetafile8}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let images = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert!(!text.contains("[Embedded object removed]"));
    assert!(!text.contains("[Image skipped: empty picture]"));
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].format, ImageFormat::Placeholder);
    assert!(images[0].bytes.is_empty());
    assert_eq!(images[0].display_width_twips, Some(2160));
    assert_eq!(images[0].display_height_twips, Some(720));
    for forbidden in [
        "objdata",
        "objemb",
        "wmetafile",
        "414243",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "object result internals leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("Image skipped"));
    assert!(rendered_text.contains("After"));
    assert!(!rendered_text.contains("[Image skipped: empty picture]"));
    for forbidden in [
        b"objdata".as_slice(),
        b"objemb",
        b"wmetafile",
        b"414243",
        b"JavaScript",
        b"EmbeddedFile",
        b"/Subtype /Image",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "empty object picture result leaked active PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn shape_result_fallback_is_ignored_after_primary_passive_visual_result() {
    let input = concat!(
        r"{\rtf1 Before {\shp{\*\shpinst",
        r"{\pict\picwgoal2160\pichgoal720\wmetafile8 01020304}",
        r"{\shptxt{\object\objw2160\objh720\objemb",
        r"{\objdata 4142432f4a617661536372697074}",
        r"{\result{\pict\wmetafile8}}}}",
        r"{\shprslt{\pict\picwgoal2160\pichgoal720\wmetafile8 05060708}}",
        r"}} After\par}",
    )
    .as_bytes()
    .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let images = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert!(!text.contains("[Image skipped: empty picture]"));
    assert_eq!(
        images.len(),
        1,
        "shape fallback result should not duplicate an already-rendered primary visual"
    );
    assert_eq!(images[0].format, ImageFormat::Placeholder);
    assert_eq!(images[0].display_width_twips, Some(2160));
    assert_eq!(images[0].display_height_twips, Some(720));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("rendering safe passive shape text/result")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("ignoring duplicate embedded object alternate after passive shape result")
    }));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("object payload in skipped destination")
    }));
    for forbidden in ["objdata", "shprslt", "wmetafile", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "shape alternate internals leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before"));
    assert_eq!(
        rendered_text.matches("Image skipped").count(),
        1,
        "PDF should contain one passive image placeholder label"
    );
    assert!(rendered_text.contains("After"));
    assert!(output.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("rendering safe passive shape text/result")
    }));
    assert!(output.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("object payload in skipped destination")
    }));
    for forbidden in [
        b"objdata".as_slice(),
        b"shprslt",
        b"wmetafile",
        b"01020304",
        b"05060708",
        b"/Subtype /Image",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "duplicate shape fallback leaked active PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn duplicate_shape_fallback_object_still_rejects_active_payload_in_reject_mode() {
    let input = concat!(
        r"{\rtf1 Before {\shp{\*\shpinst",
        r"{\pict\picwgoal2160\pichgoal720\wmetafile8 01020304}",
        r"{\shprslt{\object\objw2160\objh720\objemb",
        r"{\objdata 4142432f4a617661536372697074}",
        r"{\result{\pict\wmetafile8}}}}",
        r"}} After\par}",
    )
    .as_bytes()
    .to_vec();
    let options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };

    assert!(matches!(
        parse_rtf_bytes_with_options(&input, &options),
        Err(ParseError::ActiveContentRejected { .. })
    ));
}

#[test]
fn embedded_object_picture_result_renders_as_sanitized_static_image() {
    let image_hex = bytes_to_hex(&minimal_jpeg_with_dimensions(1, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "object{",
        "\\",
        "objdata ",
        payload_hex(),
        "}{",
        "\\",
        "result{",
        "\\",
        "pict",
        "\\",
        "jpegblip",
        "\\",
        "picwgoal720",
        "\\",
        "pichgoal720 ",
        image_hex.as_str(),
        "}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let images = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].width_px, 1);
    assert_eq!(images[0].height_px, 1);
    assert_eq!(images[0].display_width_twips, Some(720));
    assert_eq!(images[0].display_height_twips, Some(720));
    for forbidden in [
        "[Embedded object removed]",
        "objdata",
        "jpegblip",
        "picwgoal",
        "pichgoal",
        payload_hex(),
    ] {
        assert!(
            !text.contains(forbidden),
            "object picture internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
    assert!(
        output
            .pdf
            .windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image")
    );
    for forbidden in [
        b"objdata".as_slice(),
        payload_hex().as_bytes(),
        b"jpegblip",
        b"picwgoal",
        b"pichgoal",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "object picture result leaked active PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn header_object_results_and_placeholders_render_passively_without_body_flow_or_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "header Logo {",
        "\\",
        "object",
        "\\",
        "objdata 414243{",
        "\\",
        "result Object fallback",
        "\\",
        "par}} {",
        "\\",
        "object",
        "\\",
        "objdata 444546}",
        "\\",
        "par} Body",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let body_text = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(
                paragraph
                    .runs
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<String>(),
            ),
            Block::Placeholder(text) => Some(text.clone()),
            _ => None,
        })
        .collect::<String>();

    assert!(text.contains("Logo"));
    assert!(text.contains("Object fallback"));
    assert!(text.contains("[Embedded object removed]"));
    assert_eq!(body_text.trim(), "Body");
    for forbidden in ["objdata", "414243", "444546"] {
        assert!(
            !text.contains(forbidden),
            "object payload leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Logo"));
    assert!(rendered_text.contains("Object fallback"));
    assert!(rendered_text.contains("[Embedded object removed]"));
    assert!(rendered_text.contains("Body"));
    for forbidden in [
        b"objdata".as_slice(),
        b"414243",
        b"444546",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "object payload or active PDF content leaked: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn linked_object_result_renders_without_fetching_or_pdf_actions() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "object",
        "\\",
        "objlink https://example.com/live-object ",
        "\\",
        "objupdate{",
        "\\",
        "result linked fallback}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Before linked fallback After"));
    for forbidden in ["https://example.com", "objlink", "objupdate"] {
        assert!(
            !text.contains(forbidden),
            "linked object internals leaked into text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before linked fallback After"));
    for forbidden in [
        b"https://example.com".as_slice(),
        b"objlink",
        b"objupdate",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/Launch",
        b"/JavaScript",
        b"/EmbeddedFile",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "linked object leaked active PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }

    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };
    assert!(matches!(
        parse_rtf_bytes_with_options(&input, &reject_options),
        Err(ParseError::ActiveContentRejected { feature, .. }) if feature == "OLE object"
    ));
}

#[test]
fn embedded_package_destinations_do_not_reach_text_or_pdf_and_obey_reject_policy() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "package calc.exe /JavaScript /EmbeddedFile}{",
        "\\",
        "packager HiddenPackager}{",
        "\\",
        "embeddedpackage HiddenEmbeddedPackage} After",
        "\\",
        "par}",
    ]);

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "calc.exe",
        "JavaScript",
        "EmbeddedFile",
        "HiddenPackager",
        "HiddenEmbeddedPackage",
        "package",
        "packager",
        "embeddedpackage",
    ] {
        assert!(
            !text.contains(forbidden),
            "embedded package payload leaked into text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    for forbidden in [
        b"calc.exe".as_slice(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"HiddenPackager",
        b"HiddenEmbeddedPackage",
        b"package",
        b"packager",
        b"embeddedpackage",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "embedded package payload leaked into PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }

    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };
    for package_input in [
        br"{\rtf1{\package calc.exe} visible\par}".as_slice(),
        br"{\rtf1{\packager calc.exe} visible\par}",
        br"{\rtf1{\embeddedpackage calc.exe} visible\par}",
    ] {
        assert!(matches!(
            parse_rtf_bytes_with_options(package_input, &reject_options),
            Err(ParseError::ActiveContentRejected { feature, .. })
                if feature == "embedded package payload"
        ));
    }
    assert!(matches!(
        parse_rtf_bytes_with_options(
            br"{\rtf1{\info{\title Safe {\package calc.exe}}} visible\par}",
            &reject_options
        ),
        Err(ParseError::ActiveContentRejected { feature, .. })
            if feature == "embedded package payload in metadata"
    ));
    assert!(matches!(
        parse_rtf_bytes_with_options(
            br"{\rtf1{\*\unknown{\package calc.exe}} visible\par}",
            &reject_options
        ),
        Err(ParseError::ActiveContentRejected { feature, .. })
            if feature == "embedded package payload in skipped destination"
    ));
}

#[test]
fn external_file_reference_destinations_do_not_reach_text_or_pdf_and_obey_reject_policy() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "filetbl{",
        "\\",
        "file{",
        "\\",
        "filepath C:\\\\secret\\\\}{",
        "\\",
        "filename payload.docm /JavaScript /EmbeddedFile}}} After",
        "\\",
        "par}",
    ]);

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "secret",
        "payload.docm",
        "JavaScript",
        "EmbeddedFile",
        "filetbl",
        "filepath",
        "filename",
    ] {
        assert!(
            !text.contains(forbidden),
            "external file reference leaked into text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    for forbidden in [
        b"secret".as_slice(),
        b"payload.docm",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"filetbl",
        b"filepath",
        b"filename",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "external file reference leaked into PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }

    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };
    for file_input in [
        br"{\rtf1{\filetbl{\file{\filepath C:\\secret\\}{\filename payload.docm}}} visible\par}"
            .as_slice(),
        br"{\rtf1{\file{\filename payload.docm}} visible\par}",
        br"{\rtf1{\filepath C:\\secret\\} visible\par}",
        br"{\rtf1{\filename payload.docm} visible\par}",
    ] {
        assert!(matches!(
            parse_rtf_bytes_with_options(file_input, &reject_options),
            Err(ParseError::ActiveContentRejected { feature, .. })
                if feature == "external file reference"
        ));
    }
    assert!(matches!(
        parse_rtf_bytes_with_options(
            br"{\rtf1{\info{\title Safe {\file{\filename payload.docm}}}} visible\par}",
            &reject_options
        ),
        Err(ParseError::ActiveContentRejected { feature, .. })
            if feature == "external file reference in metadata"
    ));
    assert!(matches!(
        parse_rtf_bytes_with_options(
            br"{\rtf1{\*\unknown{\filename payload.docm}} visible\par}",
            &reject_options
        ),
        Err(ParseError::ActiveContentRejected { feature, .. })
            if feature == "external file reference in skipped destination"
    ));
}

#[test]
fn object_metadata_destinations_do_not_reach_text_or_pdf() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before{",
        "\\",
        "objclass HiddenClass {",
        "\\",
        "object",
        "\\",
        "objdata ",
        payload_hex(),
        "}} middle{",
        "\\",
        "objname HiddenName}{",
        "\\",
        "objalias HiddenAlias}{",
        "\\",
        "objtopic HiddenTopic} after",
        "\\",
        "par}",
    ]);
    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };
    let reject_result = parse_rtf_bytes_with_options(&input, &reject_options);
    assert!(
        matches!(
            reject_result,
            Err(ParseError::ActiveContentRejected { ref feature, .. })
                if feature == "object metadata"
        ),
        "unexpected object metadata reject result: {reject_result:?}"
    );

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before middle after"));
    for forbidden in [
        "HiddenClass",
        "HiddenName",
        "HiddenAlias",
        "HiddenTopic",
        "Embedded object removed",
        "objclass",
        "objname",
        "objalias",
        "objtopic",
        "objdata",
        payload_hex(),
    ] {
        assert!(
            !text.contains(forbidden),
            "object metadata leaked into text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("object-metadata.rtf");
    let output_path = dir.path().join("object-metadata.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before middle after"));
    for forbidden in [
        b"HiddenClass".as_slice(),
        b"HiddenName",
        b"HiddenAlias",
        b"HiddenTopic",
        b"objclass",
        b"objname",
        b"objalias",
        b"objtopic",
        b"objdata",
        payload_hex().as_bytes(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "object metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn fields_render_result_without_executing_instruction() {
    let parsed = parse_rtf_bytes(&field_with_link_result()).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("visible link"));
    assert!(!text.contains("HYPERLINK"));
    assert!(!text.contains("https://example.com"));
}

#[test]
fn header_field_results_and_placeholders_render_passively_without_body_flow_or_instruction_leakage()
{
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "header Logo {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst HYPERLINK \"https://example.com/click\"}{",
        "\\",
        "fldrslt Stored link}} {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst INCLUDEPICTURE \"https://example.com/pixel.png\"}}",
        "\\",
        "par} Body",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let body_text = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(
                paragraph
                    .runs
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<String>(),
            ),
            Block::Placeholder(text) => Some(text.clone()),
            _ => None,
        })
        .collect::<String>();

    assert!(text.contains("Logo"));
    assert!(text.contains("Stored link"));
    assert!(text.contains("[Field removed: no passive result]"));
    assert_eq!(body_text.trim(), "Body");
    for forbidden in [
        "HYPERLINK",
        "INCLUDEPICTURE",
        "https://example.com",
        "fldinst",
        "fldrslt",
    ] {
        assert!(
            !text.contains(forbidden),
            "field instruction leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Logo"));
    assert!(rendered_text.contains("Stored link"));
    assert!(rendered_text.contains("[Field removed: no passive result]"));
    assert!(rendered_text.contains("Body"));
    for forbidden in [
        b"HYPERLINK".as_slice(),
        b"INCLUDEPICTURE",
        b"https://example.com",
        b"fldinst",
        b"fldrslt",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "field instruction or active PDF content leaked: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn hyperlink_stored_results_render_as_inert_pdf_text_under_all_current_link_policies() {
    for policy in [
        PdfLinkPolicy::RenderVisibleTextOnly,
        PdfLinkPolicy::DisableAll,
        PdfLinkPolicy::AllowSanitizedHttpLinks,
    ] {
        let output = convert_rtf_to_pdf(
            &field_with_link_result(),
            &ConvertOptions {
                diagnostics: true,
                parse_options: RtfParseOptions {
                    pdf_link_policy: policy,
                    ..RtfParseOptions::default()
                },
                ..ConvertOptions::default()
            },
        )
        .unwrap();
        let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
        let page_id = *parsed_pdf.get_pages().values().next().expect("page");
        let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
        let rendered_text = decoded_pdf_text(&content);

        assert!(rendered_text.contains("visible link"));
        for forbidden in [
            b"HYPERLINK".as_slice(),
            b"https://example.com",
            b"/Action",
            b"/Annots",
            b"/JavaScript",
            b"/Launch",
            b"/OpenAction",
            b"/URI",
        ] {
            assert!(
                !output
                    .pdf
                    .windows(forbidden.len())
                    .any(|window| window == forbidden),
                "hyperlink field leaked active PDF content under {policy:?}: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn docproperty_fields_render_metadata_without_leaking_nested_active_content() {
    let input = br#"{\rtf1{\info{\title Safe title {\title Nested overwrite}{\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt Hidden link}} tail}{\author Alice}{\doccomm Hidden comment}}Doc {\field{\*\fldinst DOCPROPERTY Title}} / {\field{\*\fldinst DOCPROPERTY Author}}\par}"#.to_vec();
    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Doc Safe title"));
    assert!(rendered_text.contains("tail / Alice"));
    for forbidden in [
        b"DOCPROPERTY".as_slice(),
        b"HYPERLINK",
        b"https://example.com",
        b"Nested overwrite",
        b"Hidden link",
        b"Hidden comment",
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "document property field leaked active PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn shortcut_document_property_fields_render_metadata_without_instruction_leakage() {
    let input = br#"{\rtf1{\info{\title Safe title {\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt Hidden link}} tail}{\author Alice}{\operator Bob}{\doccomm Comment text}}Doc {\field{\*\fldinst TITLE}} / {\field{\*\fldinst AUTHOR \\* Upper}} / {\field{\*\fldinst LASTSAVEDBY}} / {\field{\*\fldinst COMMENTS}}\par}"#.to_vec();
    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Doc Safe title"));
    assert!(rendered_text.contains("tail / ALICE / Bob / Comment text"));
    for forbidden in [
        b"TITLE".as_slice(),
        b"AUTHOR",
        b"LASTSAVEDBY",
        b"COMMENTS",
        b"HYPERLINK",
        b"https://example.com",
        b"Hidden link",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "shortcut document property field leaked active PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn info_fields_render_metadata_without_instruction_or_active_payload_leakage() {
    let input = br#"{\rtf1{\info{\title Safe title {\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt Hidden link}} tail}{\author Alice}{\doccomm Comment text}}Doc {\field{\*\fldinst INFO Title}} / {\field{\*\fldinst INFO Author \\* Upper}} / {\field{\*\fldinst INFO Comments}} / {\field{\*\fldinst INFO Filename}}\par}"#.to_vec();
    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Doc Safe title"));
    assert!(rendered_text.contains("tail / ALICE / Comment text"));
    assert!(rendered_text.contains("[Field removed: no passive result]"));
    for forbidden in [
        b"INFO".as_slice(),
        b"Filename",
        b"HYPERLINK",
        b"https://example.com",
        b"Hidden link",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "INFO field leaked active PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn docproperty_fields_render_custom_properties_without_leaking_linked_or_active_content() {
    let input = br#"{\rtf1{\*\userprops{\propname Client Name}{\proptype30}{\staticval Contoso {\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt Hidden link}} tail}{\linkval Hidden linked value}}Client {\field{\*\fldinst DOCPROPERTY "Client Name"}}\par}"#.to_vec();
    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Client Contoso"));
    assert!(rendered_text.contains("tail"));
    for forbidden in [
        b"DOCPROPERTY".as_slice(),
        b"Client Name",
        b"HYPERLINK",
        b"https://example.com",
        b"Hidden link",
        b"Hidden linked value",
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "custom document property field leaked active PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn linked_custom_property_values_are_never_rendered_or_preserved() {
    let input = br#"{\rtf1{\*\userprops{\propname Client Name}{\proptype30}{\staticval Contoso}{\linkval C:\\secret\\linked-source.docm /JavaScript /EmbeddedFile}}Client {\field{\*\fldinst DOCPROPERTY "Client Name"}}\par}"#.to_vec();
    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Client Contoso"));
    for forbidden in [
        b"linked-source.docm".as_slice(),
        b"secret",
        b"linkval",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "linked custom property value leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }

    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };
    for linked_property_input in [
        br"{\rtf1{\linkval C:\\secret\\linked-source.docm} visible\par}".as_slice(),
        br"{\rtf1{\*\userprops{\propname Client}{\linkval C:\\secret\\linked-source.docm}} visible\par}",
        br"{\rtf1{\*\unknown{\linkval C:\\secret\\linked-source.docm}} visible\par}",
    ] {
        assert!(matches!(
            parse_rtf_bytes_with_options(linked_property_input, &reject_options),
            Err(ParseError::ActiveContentRejected { feature, .. })
                if feature == "linked custom property value"
        ));
    }
}

#[test]
fn docvariable_fields_render_metadata_without_leaking_nested_active_content() {
    let input = br#"{\rtf1{\*\docvar {Client Name}{Contoso {\field{\*\fldinst HYPERLINK "https://example.com/docvar"}{\fldrslt Hidden link}} tail}}Client {\field{\*\fldinst DOCVARIABLE "Client Name"}} missing {\field{\*\fldinst DOCVARIABLE Missing}}\par}"#.to_vec();
    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Client Contoso"));
    assert!(rendered_text.contains("tail"));
    assert!(rendered_text.contains("missing [Field removed: no passive result]"));
    for forbidden in [
        b"DOCVARIABLE".as_slice(),
        b"docvar",
        b"Client Name",
        b"HYPERLINK",
        b"https://example.com",
        b"Hidden link",
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "document variable field leaked active PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn date_fields_use_stored_results_and_never_update_or_leak_instructions() {
    let stored = parse_rtf_bytes(&rtf(&[
        "{",
        "\\",
        "rtf1 Created {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst DATE \\\\@ \"current-date-sentinel\"}{",
        "\\",
        "fldrslt Stored visible date}}",
        "\\",
        "par}",
    ]))
    .unwrap();
    let stored_text = collect_text(&stored.document);
    assert!(stored_text.contains("Created Stored visible date"));
    assert!(!stored_text.contains("DATE"));
    assert!(!stored_text.contains("current-date-sentinel"));
    assert!(!stored_text.contains("fldinst"));
    assert!(!stored_text.contains("[Field removed"));

    let resultless = parse_rtf_bytes(&rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst DATE \\\\@ \"current-date-sentinel\"}} After",
        "\\",
        "par}",
    ]))
    .unwrap();
    let resultless_text = collect_text(&resultless.document);
    assert!(resultless_text.contains("Before"));
    assert!(resultless_text.contains("[Field removed: no passive result]"));
    assert!(resultless_text.contains("After"));
    assert!(!resultless_text.contains("DATE"));
    assert!(!resultless_text.contains("current-date-sentinel"));
    assert!(resultless.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("dynamic field DATE removed without reading converter clock")
    }));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("date-field.rtf");
    let output_path = dir.path().join("date-field.pdf");
    fs::write(
        &input_path,
        rtf(&[
            "{",
            "\\",
            "rtf1 Before {",
            "\\",
            "field{",
            "\\",
            "*",
            "\\",
            "fldinst DATE \\\\@ \"current-date-sentinel\"}} After",
            "\\",
            "par}",
        ]),
    )
    .unwrap();
    convert_rtf_file_to_pdf(&input_path, &output_path, &ConvertOptions::default()).unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"fldinst".as_slice(),
        b"current-date-sentinel",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden date-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn metadata_timestamp_fields_render_passively_without_instruction_or_metadata_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "info{",
        "\\",
        "creatim",
        "\\",
        "yr2024",
        "\\",
        "mo7",
        "\\",
        "dy5",
        "\\",
        "hr14",
        "\\",
        "min30",
        "\\",
        "sec9}{",
        "\\",
        "revtim",
        "\\",
        "yr2025",
        "\\",
        "mo1",
        "\\",
        "dy2",
        "\\",
        "hr9",
        "\\",
        "min4",
        "\\",
        "sec5}}Created {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst CREATEDATE \\\\@ \"MMMM d, yyyy\"}} saved {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SAVEDATE \\\\@ \"yyyy-MM-dd HH:mm:ss\"}} dynamic {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst DATE \\\\@ \"current-date-sentinel\"}}",
        "\\",
        "par}",
    ]);
    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Created July 5, 2024"));
    assert!(rendered_text.contains("saved 2025-01-02 09:04:05"));
    assert!(rendered_text.contains("[Field removed: no passive result]"));
    for forbidden in [
        b"fldinst".as_slice(),
        b"CREATEDATE",
        b"SAVEDATE",
        b"current-date-sentinel",
        b"creatim",
        b"revtim",
        b"yr2024",
        b"MMMM",
        b"yyyy",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden metadata timestamp content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_page_count_fields_render_without_executing_field_instruction() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Page {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst PAGE}} of {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst NUMPAGES}}",
        "\\",
        "page second page",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains(PAGE_NUMBER_MARKER));
    assert!(text.contains(TOTAL_PAGES_MARKER));
    assert!(!text.contains("fldinst"));
    assert!(!text.contains("PAGE"));
    assert!(!text.contains("NUMPAGES"));
    assert!(!text.contains("[Field removed"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("page-count-field.rtf");
    let output_path = dir.path().join("page-count-field.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"fldinst".as_slice(),
        b"NUMPAGES",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn resultless_document_stat_fields_render_passive_counts_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 One two. Three Words:{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst NUMWORDS}} Chars:{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst NUMCHARS}} WS:{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst NUMCHARSWS}}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains(DOCUMENT_WORDS_MARKER));
    assert!(text.contains(DOCUMENT_CHARS_MARKER));
    assert!(text.contains(DOCUMENT_CHARS_WITH_SPACES_MARKER));
    for forbidden in [
        "NUMWORDS",
        "NUMCHARS",
        "NUMCHARSWS",
        "fldinst",
        "[Field removed",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden document stat field content leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("One two. Three Words:6 Chars:27 WS:32"),
        "decoded PDF text did not contain passive document stat counts: {rendered_text:?}"
    );
    for forbidden in [
        b"NUMWORDS".as_slice(),
        b"NUMCHARS",
        b"NUMCHARSWS",
        b"fldinst",
        DOCUMENT_WORDS_MARKER.as_bytes(),
        DOCUMENT_CHARS_MARKER.as_bytes(),
        DOCUMENT_CHARS_WITH_SPACES_MARKER.as_bytes(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden document stat field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn section_numbers_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Section ",
        "\\",
        "sectnum",
        "\\",
        "par",
        "\\",
        "sbknone",
        "\\",
        "sect Section ",
        "\\",
        "sectnum",
        "\\",
        "par",
        "\\",
        "sbkpage",
        "\\",
        "sect Section {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SECTION}}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains(SECTION_NUMBER_MARKER));
    assert!(!text.contains("sectnum"));
    assert!(!text.contains("fldinst"));
    assert!(!text.contains("SECTION"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("section-number.rtf");
    let output_path = dir.path().join("section-number.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();

    assert_eq!(parsed_pdf.get_pages().len(), 2);
    for forbidden in [
        b"sectnum".as_slice(),
        b"fldinst",
        b"SECTION",
        SECTION_NUMBER_MARKER.as_bytes(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn section_page_count_fields_render_passively_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 S1 page {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SECTIONPAGES}}",
        "\\",
        "page S1 second page",
        "\\",
        "sect S2 page {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SECTIONPAGES}}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains(SECTION_PAGES_MARKER));
    assert!(!text.contains("fldinst"));
    assert!(!text.contains("SECTIONPAGES"));
    assert!(!text.contains("[Field removed"));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 3);
    let rendered_text = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| {
            let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
            decoded_pdf_text(&content)
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered_text.contains("S1 page 2"),
        "first section page count should resolve to 2 in visible PDF text: {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("S2 page 1"),
        "second section page count should resolve to 1 in visible PDF text: {rendered_text:?}"
    );
    for forbidden in [
        b"fldinst".as_slice(),
        b"SECTIONPAGES",
        SECTION_PAGES_MARKER.as_bytes(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden section-page field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_seq_fields_render_deterministic_passive_counters_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Figure {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SEQ Figure}} and Figure {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SEQ Figure}} Table {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SEQ Table}}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Figure 1 and Figure 2 Table 1"));
    assert!(!text.contains("SEQ"));
    assert!(!text.contains("fldinst"));
    assert!(!text.contains("[Field removed"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field SEQ without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Figure 1 and Figure 2 Table 1"),
        "decoded PDF text did not contain passive SEQ counters: {rendered_text:?}"
    );
    for forbidden in [
        b"SEQ".as_slice(),
        b"fldinst",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden sequence-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_seq_switches_render_passive_word_visible_counters_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Figure {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SEQ Figure \\\\r 4}} then {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SEQ Figure}} repeat {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SEQ Figure \\\\c}} hidden {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SEQ Figure \\\\h}} next {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SEQ Figure}}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Figure 4 then 5 repeat 5 hidden  next 7"));
    assert!(!text.contains("\\r"));
    assert!(!text.contains("\\c"));
    assert!(!text.contains("\\h"));
    assert!(!text.contains("SEQ"));
    assert!(!text.contains("fldinst"));
    assert!(!text.contains("[Field removed"));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Figure 4 then 5 repeat 5 hidden  next 7"),
        "decoded PDF text did not contain passive SEQ switch results: {rendered_text:?}"
    );
    for forbidden in [
        b"SEQ".as_slice(),
        b"fldinst",
        b"\\r",
        b"\\c",
        b"\\h",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden sequence-switch content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_mergefields_render_passive_placeholders_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Dear {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst MERGEFIELD FirstName \\\\* MERGEFORMAT}} {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst MERGEFIELD \"Last Name\" \\\\b \"before\"}}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Dear \u{00ab}FirstName\u{00bb} \u{00ab}Last Name\u{00bb}"));
    assert!(!text.contains("MERGEFIELD"));
    assert!(!text.contains("MERGEFORMAT"));
    assert!(!text.contains("fldinst"));
    assert!(!text.contains("before"));
    assert!(!text.contains("[Field removed"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field MERGEFIELD without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Dear")
            && rendered_text.contains("FirstName")
            && rendered_text.contains("Last Name"),
        "decoded PDF text did not contain passive MERGEFIELD placeholders: {rendered_text:?}"
    );
    for forbidden in [
        b"MERGEFIELD".as_slice(),
        b"MERGEFORMAT",
        b"fldinst",
        b"before",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden merge-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_autonum_fields_render_bounded_passive_numbers_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Clause {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst AUTONUM}}. Legal {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst AUTONUMLGL \\\\s 1}}. Outline {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst AUTONUMOUT}}.",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Clause 1. Legal 2. Outline 3."));
    assert!(!text.contains("AUTONUM"));
    assert!(!text.contains("AUTONUMLGL"));
    assert!(!text.contains("AUTONUMOUT"));
    assert!(!text.contains("fldinst"));
    assert!(!text.contains("\\s"));
    assert!(!text.contains("[Field removed"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field AUTONUM without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Clause 1. Legal 2. Outline 3."),
        "decoded PDF text did not contain passive AUTONUM values: {rendered_text:?}"
    );
    for forbidden in [
        b"AUTONUM".as_slice(),
        b"AUTONUMLGL",
        b"AUTONUMOUT",
        b"fldinst",
        b"\\s",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden autonum-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_listnum_fields_render_bounded_passive_numbers_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Item {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst LISTNUM}}. Sub {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst LISTNUM \\\\l 2}}. Named {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst LISTNUM LegalDefault}}. Named next {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst LISTNUM LegalDefault}}. Reset {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst LISTNUM LegalDefault \\\\s 7}}. Next {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst LISTNUM LegalDefault}}.",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Item 1. Sub 1. Named 1. Named next 2. Reset 7. Next 8."));
    for forbidden in [
        "LISTNUM",
        "LegalDefault",
        "fldinst",
        "\\l",
        "\\s",
        "[Field removed",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden LISTNUM content leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field LISTNUM without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Item 1. Sub 1. Named 1. Named next 2. Reset 7. Next 8."),
        "decoded PDF text did not contain passive LISTNUM values: {rendered_text:?}"
    );
    for forbidden in [
        b"LISTNUM".as_slice(),
        b"LegalDefault",
        b"fldinst",
        b"\\l",
        b"\\s",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden listnum-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_ref_fields_render_closed_bookmark_text_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "*",
        "\\",
        "bkmkstart SafeBookmark}Visible referenced text{",
        "\\",
        "*",
        "\\",
        "bkmkend SafeBookmark} copy {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst REF SafeBookmark \\\\h}}.",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let visible_text = strip_bookmark_page_markers(&text);

    assert!(visible_text.contains("Before Visible referenced text copy Visible referenced text."));
    for forbidden in [
        "REF",
        "SafeBookmark",
        "bkmkstart",
        "bkmkend",
        "fldinst",
        "\\h",
        "[Field removed",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden REF/bookmark content leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field REF without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Before Visible referenced text copy Visible referenced text."),
        "decoded PDF text did not contain passive REF value: {rendered_text:?}"
    );
    for forbidden in [
        b"REF".as_slice(),
        b"SafeBookmark",
        b"bkmkstart",
        b"bkmkend",
        b"fldinst",
        b"\\h",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden REF/bookmark content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_set_fields_feed_refs_without_instruction_or_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Output {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SET HiddenKey \"Contoso\"}}{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst REF HiddenKey}} reject {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SET BadKey \"Hidden value\" HYPERLINK \"https://example.com/set\"}}{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst REF BadKey}}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Output Contoso reject [Field removed: no passive result]"));
    for forbidden in [
        "SET",
        "HiddenKey",
        "BadKey",
        "Hidden value",
        "HYPERLINK",
        "https://example.com/set",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "SET field leaked unsafe text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Output Contoso reject [Field removed: no passive result]"),
        "decoded PDF text did not contain passive SET/REF value: {rendered_text:?}"
    );
    for forbidden in [
        b"SET".as_slice(),
        b"REF",
        b"HiddenKey",
        b"BadKey",
        b"Hidden value",
        b"HYPERLINK",
        b"https://example.com/set",
        b"fldinst",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden SET-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_styleref_fields_render_safe_prior_style_text_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "stylesheet{",
        "\\",
        "s1 TitleStyle;}}{",
        "\\",
        "s1 Visible title {",
        "\\",
        "*",
        "\\",
        "unknown{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst HYPERLINK \"https://example.com/styleref\"}{",
        "\\",
        "fldrslt Hidden link}}}",
        "\\",
        "par}{",
        "\\",
        "pard Ref {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst STYLEREF TitleStyle \\\\* Upper}}",
        "\\",
        "par}}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible title"));
    assert!(text.contains("Ref VISIBLE TITLE"));
    for forbidden in [
        "STYLEREF",
        "TitleStyle",
        "HYPERLINK",
        "https://example.com",
        "Hidden link",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "STYLEREF leaked unsafe text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Visible title"));
    assert!(
        rendered_text.contains("Ref VISIBLE TITLE"),
        "decoded PDF text did not contain passive STYLEREF value: {rendered_text:?}"
    );
    for forbidden in [
        b"STYLEREF".as_slice(),
        b"TitleStyle",
        b"HYPERLINK",
        b"https://example.com",
        b"Hidden link",
        b"fldinst",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden STYLEREF content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_noteref_fields_render_bookmarked_note_reference_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Note {",
        "\\",
        "*",
        "\\",
        "bkmkstart NoteRef}",
        "\\",
        "chftn{",
        "\\",
        "*",
        "\\",
        "bkmkend NoteRef}{",
        "\\",
        "footnote Footnote text {",
        "\\",
        "*",
        "\\",
        "unknown{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst HYPERLINK \"https://example.com/hidden\"}}",
        "\\",
        "par} again {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst NOTEREF NoteRef \\\\* ROMAN}}.",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst NOTEREF Missing}}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let visible_text = strip_bookmark_page_markers(&text);

    assert!(visible_text.contains("Note 1"), "text was {text:?}");
    assert!(visible_text.contains("again I."), "text was {text:?}");
    assert_eq!(
        text.matches("[Field removed: no passive result]").count(),
        1
    );
    for forbidden in [
        "NOTEREF",
        "NoteRef",
        "bkmkstart",
        "bkmkend",
        "fldinst",
        "HYPERLINK",
        "https://example.com",
        "unknown",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden NOTEREF/bookmark content leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field NOTEREF without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let rendered_text = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| {
            let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
            decoded_pdf_text(&content)
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered_text.contains("Note 1"),
        "decoded PDF text did not contain bookmarked note reference: {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("again I."),
        "decoded PDF text did not contain passive NOTEREF value: {rendered_text:?}"
    );
    for forbidden in [
        b"NOTEREF".as_slice(),
        b"NoteRef",
        b"bkmkstart",
        b"bkmkend",
        b"fldinst",
        b"HYPERLINK",
        b"https://example.com",
        b"Hidden link",
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden NOTEREF/bookmark content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_pageref_fields_resolve_bookmark_page_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 See target on page {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst PAGEREF TargetBookmark \\\\h}}.",
        "\\",
        "page {",
        "\\",
        "*",
        "\\",
        "bkmkstart TargetBookmark}Target text{",
        "\\",
        "*",
        "\\",
        "bkmkend TargetBookmark}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains(BOOKMARK_PAGE_REF_MARKER));
    assert!(text.contains(BOOKMARK_PAGE_ANCHOR_MARKER));
    for forbidden in [
        "PAGEREF",
        "TargetBookmark",
        "bkmkstart",
        "bkmkend",
        "fldinst",
        "\\h",
        "[Field removed",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden PAGEREF/bookmark content leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field PAGEREF without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 2);
    let rendered_text = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| {
            let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
            decoded_pdf_text(&content)
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered_text.contains("See target on page 2."),
        "decoded PDF text did not contain passive PAGEREF value: {rendered_text:?}"
    );
    assert!(rendered_text.contains("Target text"));
    for forbidden in [
        b"PAGEREF".as_slice(),
        b"TargetBookmark",
        b"bkmkstart",
        b"bkmkend",
        b"fldinst",
        b"\\h",
        BOOKMARK_PAGE_REF_MARKER.as_bytes(),
        BOOKMARK_PAGE_ANCHOR_MARKER.as_bytes(),
        BOOKMARK_PAGE_MARKER_END.as_bytes(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden PAGEREF/bookmark content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_formula_fields_render_bounded_passive_arithmetic_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Total {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = (6 + 4) * 3 \\\\# \"0\"}} and delta {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = -8 + 3 * 2}}.",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Total 30 and delta -2."));
    for forbidden in [
        "fldinst",
        "\\#",
        "\"0\"",
        "(6 + 4)",
        "3 * 2",
        "[Field removed",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden formula-field content leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field FORMULA without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Total 30 and delta -2."),
        "decoded PDF text did not contain passive formula values: {rendered_text:?}"
    );
    for forbidden in [
        b"fldinst".as_slice(),
        b"\\#",
        b"\"0\"",
        b"(6 + 4)",
        b"3 * 2",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden formula-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_eq_fraction_fields_render_passively_without_instruction_leakage() {
    let input = br"{\rtf1 Equation {\field{\*\fldinst EQ \f(1,2)}} and escaped {\field{\*\fldinst EQ \\f(alpha,beta)}}\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains("Equation 1\u{2044}2 and escaped alpha\u{2044}beta"),
        "normalized EQ text was {text:?}"
    );
    for forbidden in [
        "EQ",
        "fldinst",
        "\\f",
        "(1,2)",
        "(alpha,beta)",
        "[Field removed",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden EQ field content leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field EQ without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Equation 1"));
    assert!(rendered_text.contains("2 and escaped alpha"));
    assert!(rendered_text.contains("beta"));
    let numerator_position =
        pdf_first_text_position_for_text(&content, "1").expect("EQ numerator position");
    let denominator_position =
        pdf_first_text_position_for_text(&content, "2").expect("EQ denominator position");
    assert!(
        numerator_position.1 > denominator_position.1,
        "EQ fraction numerator should render above denominator: numerator={numerator_position:?}, denominator={denominator_position:?}"
    );
    let alpha_position =
        pdf_first_text_position_for_text(&content, "alpha").expect("escaped EQ numerator position");
    let beta_position = pdf_first_text_position_for_text(&content, "beta")
        .expect("escaped EQ denominator position");
    assert!(
        alpha_position.1 > beta_position.1,
        "escaped EQ fraction numerator should render above denominator: numerator={alpha_position:?}, denominator={beta_position:?}"
    );
    let symbol_bytes = pdf_text_bytes_for_font(&content, b"F13");
    assert!(
        symbol_bytes.iter().filter(|byte| **byte == 0xa4).count() >= 2,
        "EQ fractions should encode semantic fraction slashes through passive Symbol byte 0xa4; got {symbol_bytes:?}"
    );
    for forbidden in [
        b"EQ".as_slice(),
        b"fldinst",
        b"\\f",
        b"(1,2)",
        b"(alpha,beta)",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden EQ field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_eq_root_fields_render_passively_without_instruction_leakage() {
    let input =
        br"{\rtf1 Roots {\field{\*\fldinst EQ \\r(x+1)}} cube {\field{\*\fldinst EQ \\r(3,y)}}\par}"
            .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains("Roots \u{221a}x+1 cube 3\u{221a}y"),
        "normalized EQ root text was {text:?}"
    );
    for forbidden in ["EQ", "fldinst", "\\r", "(x+1)", "(3,y)", "[Field removed"] {
        assert!(
            !text.contains(forbidden),
            "forbidden EQ root field content leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field EQ without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Roots "));
    assert!(rendered_text.contains("x+1 cube 3"));
    assert!(rendered_text.contains("y"));
    let degree_position =
        pdf_first_text_position_for_text(&content, "3").expect("EQ root degree position");
    let radicand_position =
        pdf_first_text_position_for_text(&content, "y").expect("EQ root radicand position");
    assert!(
        degree_position.1 > radicand_position.1,
        "EQ indexed root degree should render above radicand: degree={degree_position:?}, radicand={radicand_position:?}"
    );
    let symbol_bytes = pdf_text_bytes_for_font(&content, b"F13");
    assert!(
        symbol_bytes.iter().filter(|byte| **byte == 0xd6).count() >= 2,
        "EQ roots should encode radical markers through passive Symbol byte 0xd6; got {symbol_bytes:?}"
    );
    assert!(
        content.operations.windows(3).any(|operations| {
            operations[0].operator == "m"
                && operations[1].operator == "l"
                && operations[2].operator == "S"
        }),
        "EQ root radicands should render with passive overbar strokes"
    );
    for forbidden in [
        b"EQ".as_slice(),
        b"fldinst",
        b"\\r",
        b"(3,y)",
        b"\\r(x+1)",
        b"\\r(3,y)",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden EQ root field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_if_fields_render_passive_branches_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Status {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst IF 5 > 3 \"Greater\" \"Lower\"}} and {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst IF \"Alpha\" = \"Beta\" \"Match\" \"Different\"}}.",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Status Greater and Different."));
    for forbidden in [
        "IF",
        "Alpha",
        "Beta",
        "fldinst",
        "Greater\"",
        "Lower\"",
        "[Field removed",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden IF field content leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field IF without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Status Greater and Different."),
        "decoded PDF text did not contain passive IF values: {rendered_text:?}"
    );
    for forbidden in [
        b"IF".as_slice(),
        b"Alpha",
        b"Beta",
        b"fldinst",
        b"Greater\"",
        b"Lower\"",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden IF field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_compare_fields_render_bounded_passive_values_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Compare {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst COMPARE 5 > 3}} and {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst COMPARE \"Alpha\" = \"Beta\"}} and {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst COMPARE 3 <= 3 \\\\* ROMAN}} and malformed {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst COMPARE 1 = 1 HIDDEN-TRAIL}}.",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains("Compare 1 and 0 and I and malformed [Field removed: no passive result]."),
        "text was {text:?}"
    );
    for forbidden in [
        "COMPARE",
        "Alpha",
        "Beta",
        "HIDDEN-TRAIL",
        "fldinst",
        "ROMAN",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden COMPARE field content leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field COMPARE without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text
            .contains("Compare 1 and 0 and I and malformed [Field removed: no passive result]."),
        "decoded PDF text did not contain passive COMPARE values: {rendered_text:?}"
    );
    for forbidden in [
        b"COMPARE".as_slice(),
        b"Alpha",
        b"Beta",
        b"HIDDEN-TRAIL",
        b"fldinst",
        b"ROMAN",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden COMPARE field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_field_case_switches_render_passively_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Values {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst QUOTE \"mixed Case\" \\\\* Upper}} and {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst QUOTE \"MIXED Case\" \\\\* Lower \\\\* FirstCap}} and {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst IF 1 = 1 \"checked status\" \"other\" \\\\* Caps}}.",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains("Values MIXED CASE and Mixed case and Checked Status."),
        "normalized field switch text was {text:?}"
    );
    for forbidden in [
        "QUOTE",
        "IF",
        "fldinst",
        "Upper",
        "Lower",
        "FirstCap",
        "Caps",
        "checked status\"",
        "[Field removed",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden field case-switch content leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field QUOTE without executing field instruction")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field IF without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Values MIXED CASE and Mixed case and Checked Status."),
        "decoded PDF text did not contain passive field case-switch values: {rendered_text:?}"
    );
    for forbidden in [
        b"QUOTE".as_slice(),
        b"IF",
        b"fldinst",
        b"Upper",
        b"Lower",
        b"FirstCap",
        b"checked status\"",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden field case-switch content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_field_number_switches_render_passively_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Values {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SEQ Figure \\\\r 4 \\\\* ROMAN}} {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SEQ Figure \\\\* roman}} {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = 27 \\\\* alphabetic}} {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = 27 \\\\* ALPHABETIC}} {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = 255 \\\\* Hex}} {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst IF 1 = 1 \"7\" \"0\" \\\\* Ordinal}}.",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains("Values IV v aa AA FF 7th."),
        "normalized field number-switch text was {text:?}"
    );
    for forbidden in [
        "SEQ",
        "IF",
        "fldinst",
        "\\r",
        "Figure",
        "ROMAN",
        "roman",
        "alphabetic",
        "ALPHABETIC",
        "Ordinal",
        "Hex",
        "[Field removed",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden field number-switch content leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field SEQ without executing field instruction")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field FORMULA without executing field instruction")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field IF without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Values IV v aa AA FF 7th."),
        "decoded PDF text did not contain passive field number-switch values: {rendered_text:?}"
    );
    for forbidden in [
        b"SEQ".as_slice(),
        b"IF",
        b"fldinst",
        b"\\r",
        b"Figure",
        b"ROMAN",
        b"roman",
        b"alphabetic",
        b"ALPHABETIC",
        b"Ordinal",
        b"Hex",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden field number-switch content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_numeric_picture_switches_render_passively_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Values {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = 42 \\\\# \"0000\"}} {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = 1234567 \\\\# \"#,##0\"}} {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst IF 1 = 1 \"5\" \"0\" \\\\# \"$0.00\"}} {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = -8 \\\\# \"000\"}}.",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains("Values 0042 1,234,567 $5.00 -008."),
        "normalized numeric-picture text was {text:?}"
    );
    for forbidden in [
        "fldinst",
        "\\#",
        "#,##0",
        "$0.00",
        "\"0000\"",
        "[Field removed",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden numeric-picture content leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Values 0042 1,234,567 $5.00 -008."),
        "decoded PDF text did not contain passive numeric-picture values: {rendered_text:?}"
    );
    for forbidden in [
        b"fldinst".as_slice(),
        b"\\#",
        b"#,##0",
        b"$0.00",
        b"\"0000\"",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden numeric-picture content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_quote_fields_render_without_executing_field_instruction() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst QUOTE \"Visible quoted text\"}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before Visible quoted text After"));
    assert!(!text.contains("QUOTE"));
    assert!(!text.contains("fldinst"));
    assert!(!text.contains("[Field removed"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field QUOTE without executing field instruction")
    }));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("quote-field.rtf");
    let output_path = dir.path().join("quote-field.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(
        decoded_pdf_text(&content).contains("Before Visible quoted text After"),
        "decoded PDF text did not contain passive QUOTE result"
    );
    for forbidden in [
        b"QUOTE".as_slice(),
        b"fldinst",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden quote-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_macrobutton_fields_render_without_executing_macro() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst MACROBUTTON LaunchPayload Visible button text}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before Visible button text After"));
    assert!(!text.contains("MACROBUTTON"));
    assert!(!text.contains("LaunchPayload"));
    assert!(!text.contains("fldinst"));
    assert!(!text.contains("[Field removed"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field MACROBUTTON without executing field instruction")
    }));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("macrobutton-field.rtf");
    let output_path = dir.path().join("macrobutton-field.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(decoded_pdf_text(&content).contains("Before Visible button text After"));
    for forbidden in [
        b"MACROBUTTON".as_slice(),
        b"LaunchPayload",
        b"fldinst",
        b"/AA",
        b"/AcroForm",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden macrobutton-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_symbol_fields_render_without_executing_field_instruction() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SYMBOL 65}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before A After"));
    assert!(!text.contains("SYMBOL"));
    assert!(!text.contains("fldinst"));
    assert!(!text.contains("[Field removed"));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("rendering passive field SYMBOL without executing field instruction")
    }));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("symbol-field.rtf");
    let output_path = dir.path().join("symbol-field.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(decoded_pdf_text(&content).contains("Before A After"));
    for forbidden in [
        b"SYMBOL".as_slice(),
        b"fldinst",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden symbol-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn empty_stored_symbol_result_falls_back_to_passive_symbol_rendering() {
    let input =
        br#"{\rtf1{\fonttbl{\f0 Arial;}{\f14 Wingdings;}}\f0 Before ({\field{\*\fldinst SYMBOL 74 \\f "Wingdings" \\s 12}{\fldrslt\f14\fs24}}) After\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains("Before (\u{263a}) After"),
        "empty stored SYMBOL result should render a passive Unicode dingbat result, got {text:?}"
    );
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        other => panic!("expected paragraph with passive SYMBOL field, got {other:?}"),
    };
    let symbol_run = paragraph
        .runs
        .iter()
        .find(|run| run.text == "\u{263a}")
        .expect("passive SYMBOL field run");
    assert_eq!(symbol_run.style.font_size_half_points, 24);
    for forbidden in ["fldinst", "fldrslt", "SYMBOL", "Wingdings"] {
        assert!(
            !text.contains(forbidden),
            "forbidden empty-result symbol metadata leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("rendering passive field SYMBOL without executing field instruction")
    }));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("empty-symbol-result.rtf");
    let output_path = dir.path().join("empty-symbol-result.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        !pdf.windows(b"/BaseFont /ZapfDingbats".len())
            .any(|window| window == b"/BaseFont /ZapfDingbats"),
        "empty-result Wingdings smiley should not require a viewer ZapfDingbats font"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "c"),
        "empty-result Wingdings smiley should draw a passive vector overlay for viewer-stable output"
    );

    for forbidden in [
        b"Wingdings".as_slice(),
        b"fldinst",
        b"fldrslt",
        b"SYMBOL",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/AcroForm",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden empty-result symbol content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn unquoted_symbol_font_switch_renders_passive_dingbat_without_metadata_leakage() {
    let input =
        br"{\rtf1 Before {\field{\*\fldinst SYMBOL 74 \\f Wingdings \\s 12}} After\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains("Before \u{263a} After"),
        "unquoted Wingdings SYMBOL field should render a passive dingbat, got {text:?}"
    );
    for forbidden in ["fldinst", "SYMBOL", "Wingdings"] {
        assert!(
            !text.contains(forbidden),
            "forbidden unquoted-symbol metadata leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        !output
            .pdf
            .windows(b"/BaseFont /ZapfDingbats".len())
            .any(|window| window == b"/BaseFont /ZapfDingbats"),
        "unquoted Wingdings smiley should not require a viewer ZapfDingbats font"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "c"),
        "unquoted Wingdings smiley should draw a passive vector overlay for viewer-stable output"
    );
    for forbidden in [
        b"Wingdings".as_slice(),
        b"fldinst",
        b"SYMBOL",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/AcroForm",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden unquoted-symbol content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn page_number_start_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "pgnstarts7{",
        "\\",
        "header Page ",
        "\\",
        "chpgn",
        "\\",
        "par}First",
        "\\",
        "page Second",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert_eq!(parsed.document.page.page_number_start, Some(7));
    assert!(text.contains(PAGE_NUMBER_MARKER));
    assert!(!text.contains("pgnstarts"));
    assert!(!text.contains("chpgn"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("page-number-start.rtf");
    let output_path = dir.path().join("page-number-start.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();

    assert_eq!(parsed_pdf.get_pages().len(), 2);
    for forbidden in [
        b"pgnstarts".as_slice(),
        b"pgnstart",
        b"chpgn",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn page_number_format_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "pgnucrm",
        "\\",
        "pgnstarts4{",
        "\\",
        "header Page ",
        "\\",
        "chpgn",
        "\\",
        "par}First",
        "\\",
        "page Second",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(parsed.document.page.page_number_format.is_some());
    assert!(text.contains(PAGE_NUMBER_MARKER));
    assert!(!text.contains("pgnucrm"));
    assert!(!text.contains("pgnstarts"));
    assert!(!text.contains("chpgn"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("page-number-format.rtf");
    let output_path = dir.path().join("page-number-format.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();

    assert_eq!(parsed_pdf.get_pages().len(), 2);
    for forbidden in [
        b"pgnucrm".as_slice(),
        b"pgnstarts",
        b"chpgn",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn section_page_number_restart_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "header Page ",
        "\\",
        "chpgn",
        "\\",
        "par}First",
        "\\",
        "sect",
        "\\",
        "sectd",
        "\\",
        "pgnstarts3 Second",
        "\\",
        "page Third",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();

    assert!(collect_text(&parsed.document).contains(PAGE_NUMBER_MARKER));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("section-page-number-start.rtf");
    let output_path = dir.path().join("section-page-number-start.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();

    assert_eq!(parsed_pdf.get_pages().len(), 3);
    for forbidden in [
        b"pgnstarts".as_slice(),
        b"sectd",
        b"chpgn",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn section_page_number_restart_and_continue_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "header Page ",
        "\\",
        "chpgn",
        "\\",
        "par}First",
        "\\",
        "page Second",
        "\\",
        "sect",
        "\\",
        "sectd",
        "\\",
        "pgnrestart Restarted",
        "\\",
        "sect",
        "\\",
        "sectd",
        "\\",
        "pgncont Continued",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let section_starts: Vec<_> = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::SectionSettings(settings) => Some(settings.page_number_start),
            _ => None,
        })
        .collect();

    assert_eq!(section_starts, vec![Some(1), None]);
    assert!(collect_text(&parsed.document).contains(PAGE_NUMBER_MARKER));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("section-page-number-restart-continue.rtf");
    let output_path = dir.path().join("section-page-number-restart-continue.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();

    assert_eq!(parsed_pdf.get_pages().len(), 4);
    for forbidden in [
        b"pgnrestart".as_slice(),
        b"pgncont",
        b"sectd",
        b"chpgn",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn section_page_number_format_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "header Page ",
        "\\",
        "chpgn",
        "\\",
        "par}First",
        "\\",
        "sect",
        "\\",
        "sectd",
        "\\",
        "pgnlcltr",
        "\\",
        "pgnstarts2 Second",
        "\\",
        "page Third",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();

    assert!(collect_text(&parsed.document).contains(PAGE_NUMBER_MARKER));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("section-page-number-format.rtf");
    let output_path = dir.path().join("section-page-number-format.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();

    assert_eq!(parsed_pdf.get_pages().len(), 3);
    for forbidden in [
        b"pgnlcltr".as_slice(),
        b"pgnstarts",
        b"sectd",
        b"chpgn",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn page_number_position_and_section_grid_controls_warn_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "sectd",
        "\\",
        "pgnx360",
        "\\",
        "pgny1440",
        "\\",
        "sectlinegrid360",
        "\\",
        "sectdefaultcl",
        "\\",
        "sectexpand720",
        "\\",
        "sectspecifycl",
        "\\",
        "sectspecifyl",
        "\\",
        "sectunlocked{",
        "\\",
        "header Page ",
        "\\",
        "chpgn",
        "\\",
        "par}Visible section grid",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert_eq!(parsed.document.page.text_line_grid_twips, None);
    assert_eq!(parsed.document.page.page_number_x_twips, Some(360));
    assert_eq!(parsed.document.page.page_number_y_twips, Some(1_440));
    assert!(text.contains("Visible section grid"));
    assert!(text.contains(PAGE_NUMBER_MARKER));
    for forbidden in [
        "pgnx",
        "pgny",
        "sectlinegrid",
        "sectdefaultcl",
        "sectexpand",
        "sectspecifycl",
        "sectspecifyl",
        "sectunlocked",
        "chpgn",
    ] {
        assert!(
            !text.contains(forbidden),
            "section compatibility control leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "section compatibility controls should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("page number position approximated")
    }));
    for expected in [
        "section line grid applied as bounded passive paragraph line pitch",
        "section text grid approximated by passive paragraph layout",
    ] {
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "missing diagnostic: {expected}; diagnostics were {:?}",
            parsed.diagnostics
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let pdf_text = decoded_pdf_text(&content);
    assert!(pdf_text.contains("Visible section grid"));
    assert!(pdf_text.contains("Page 1"));
    let page_number_position =
        pdf_first_text_position_for_text(&content, "Page").expect("page number position");
    assert!(
        (page_number_position.0 - 18.0).abs() < 0.01,
        "expected page number x at 18pt, got {page_number_position:?}"
    );
    assert!(
        (page_number_position.1 - 708.75).abs() < 0.01,
        "expected page number baseline near 708.75pt, got {page_number_position:?}"
    );
    for forbidden in [
        b"pgnx".as_slice(),
        b"pgny",
        b"sectlinegrid",
        b"sectdefaultcl",
        b"sectexpand",
        b"sectspecifycl",
        b"sectspecifyl",
        b"sectunlocked",
        b"chpgn",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "section compatibility content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn no_snap_line_grid_control_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "sectd",
        "\\",
        "sectlinegrid720",
        "\\",
        "nosnaplinegrid Loose one",
        "\\",
        "line Loose two",
        "\\",
        "par",
        "\\",
        "nosnaplinegrid0 Grid one",
        "\\",
        "line Grid two",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let paragraph = |idx| match &parsed.document.blocks[idx] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };

    assert!(text.contains("Loose one\nLoose two"));
    assert!(text.contains("Grid one\nGrid two"));
    assert!(!text.contains("nosnaplinegrid"));
    assert!(!paragraph(0).style.snap_to_line_grid);
    assert!(paragraph(1).style.snap_to_line_grid);
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "nosnaplinegrid should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Loose one"));
    assert!(rendered_text.contains("Loose two"));
    assert!(rendered_text.contains("Grid one"));
    assert!(rendered_text.contains("Grid two"));
    for forbidden in [
        b"nosnaplinegrid".as_slice(),
        b"sectlinegrid",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "no-snap line-grid content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_external_fields_are_placeholdered_without_fetching() {
    let parsed = parse_rtf_bytes(&rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst INCLUDEPICTURE \"https://example.com/pixel.png\"}} After}",
    ]))
    .unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before"));
    assert!(text.contains("[Field removed: no passive result]"));
    assert!(text.contains("After"));
    assert!(!text.contains("INCLUDEPICTURE"));
    assert!(!text.contains("https://example.com"));

    let strip_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Strip,
        ..RtfParseOptions::default()
    };
    let stripped = parse_rtf_bytes_with_options(
        &rtf(&[
            "{",
            "\\",
            "rtf1 Before {",
            "\\",
            "field{",
            "\\",
            "*",
            "\\",
            "fldinst INCLUDEPICTURE \"https://example.com/pixel.png\"}} After}",
        ]),
        &strip_options,
    )
    .unwrap();
    let stripped_text = collect_text(&stripped.document);
    assert!(!stripped_text.contains("[Field removed"));
    assert!(!stripped_text.contains("https://example.com"));
}

#[test]
fn resultless_active_external_fields_do_not_fetch_or_leak_to_pdf() {
    let input = br#"{\rtf1 Visible before
{\field{\*\fldinst INCLUDEPICTURE "https://example.com/pixel.png"}}
{\field{\*\fldinst INCLUDETEXT "https://example.com/doc.rtf"}}
{\field{\*\fldinst HYPERLINK "https://example.com/click"}}
{\field{\*\fldinst LINK Word.Document.8 "https://example.com/doc.doc"}}
{\field{\*\fldinst DDEAUTO Excel "Sheet1" "R1C1"}}
{\field{\*\fldinst DDE Excel "Sheet1" "R2C2"}}
{\field{\*\fldinst IMPORT "https://example.com/data"}}
{\field{\*\fldinst DATABASE \d "https://example.com/db" \s "SELECT * FROM Hidden"}}
visible after\par}"#
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("visible after"));
    assert_eq!(
        text.matches("[Field removed: no passive result]").count(),
        8
    );
    for forbidden in [
        "INCLUDEPICTURE",
        "INCLUDETEXT",
        "HYPERLINK",
        "LINK",
        "DDEAUTO",
        "DDE",
        "IMPORT",
        "DATABASE",
        "example.com",
        "SELECT *",
        "Sheet1",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden active external field leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("active-external-fields.rtf");
    let output_path = dir.path().join("active-external-fields.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("visible after"));
    assert!(rendered_text.contains("[Field removed: no passive result]"));
    for forbidden in [
        b"INCLUDEPICTURE".as_slice(),
        b"INCLUDETEXT",
        b"HYPERLINK",
        b"LINK",
        b"DDEAUTO",
        b"DDE",
        b"IMPORT",
        b"DATABASE",
        b"example.com",
        b"SELECT *",
        b"Sheet1",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden active external field leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_generated_reference_fields_do_not_build_or_leak_to_pdf() {
    let input = br#"{\rtf1 Visible before
{\field{\*\fldinst TOC \o "1-3" \h \z \u}}
index {\field{\*\fldinst INDEX \c "2" \e "Hidden separator"}}
citation {\field{\*\fldinst CITATION HiddenSource \l 1033}}
bib {\field{\*\fldinst BIBLIOGRAPHY}}
toa {\field{\*\fldinst TOA \c 1}}
visible after\par}"#
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("visible after"));
    assert!(text.contains("index [Field removed: no passive result]"));
    assert!(text.contains("citation [Field removed: no passive result]"));
    assert!(text.contains("bib [Field removed: no passive result]"));
    assert!(text.contains("toa [Field removed: no passive result]"));
    assert_eq!(
        text.matches("[Field removed: no passive result]").count(),
        5
    );
    for forbidden in [
        "TOC",
        "INDEX",
        "CITATION",
        "BIBLIOGRAPHY",
        "TOA",
        "HiddenSource",
        "Hidden separator",
        "fldinst",
        "\\o",
        "\\h",
        "\\z",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden generated field leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("visible after"));
    assert!(rendered_text.contains("[Field removed: no passive result]"));
    for forbidden in [
        b"TOC".as_slice(),
        b"INDEX",
        b"CITATION",
        b"BIBLIOGRAPHY",
        b"TOA",
        b"HiddenSource",
        b"Hidden separator",
        b"fldinst",
        b"\\o",
        b"\\h",
        b"\\z",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden generated field leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_template_barcode_equation_embed_and_gotobutton_fields_stay_passive() {
    let input = br#"{\rtf1 Visible before
{\field{\*\fldinst AUTOTEXT HiddenBlock}}
list {\field{\*\fldinst AUTOTEXTLIST "Hidden menu" \s HiddenStyle}}
barcode {\field{\*\fldinst BARCODE "Hidden address" QR \h 720}}
display {\field{\*\fldinst DISPLAYBARCODE "Hidden code" QR}}
mergebarcode {\field{\*\fldinst MERGEBARCODE "Hidden merge value" QR \h 720}}
eq {\field{\*\fldinst EQ \f(1,2)}}
embed {\field{\*\fldinst EMBED Word.Document.8}}
go {\field{\*\fldinst GOTOBUTTON HiddenBookmark "Visible jump"}}
visible after\par}"#
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("visible after"));
    assert!(text.contains("eq 1\u{2044}2"));
    assert!(text.contains("go Visible jump"));
    assert_eq!(
        text.matches("[Field removed: no passive result]").count(),
        6
    );
    for forbidden in [
        "AUTOTEXT",
        "AUTOTEXTLIST",
        "BARCODE",
        "DISPLAYBARCODE",
        "MERGEBARCODE",
        "EMBED",
        "EQ",
        "GOTOBUTTON",
        "HiddenBlock",
        "Hidden menu",
        "HiddenStyle",
        "Hidden address",
        "Hidden code",
        "Hidden merge value",
        "HiddenBookmark",
        "Word.Document.8",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden template/generated/embedded field leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("visible after"));
    assert!(rendered_text.contains("eq 1"));
    assert!(rendered_text.contains("2"));
    let symbol_bytes = pdf_text_bytes_for_font(&content, b"F13");
    assert!(
        symbol_bytes.contains(&0xa4),
        "EQ field fraction should encode semantic slash through passive Symbol byte 0xa4; got {symbol_bytes:?}"
    );
    assert!(rendered_text.contains("go Visible jump"));
    assert!(rendered_text.contains("[Field removed: no passive result]"));
    for forbidden in [
        b"AUTOTEXT".as_slice(),
        b"AUTOTEXTLIST",
        b"BARCODE",
        b"DISPLAYBARCODE",
        b"MERGEBARCODE",
        b"EMBED",
        b"EQ",
        b"GOTOBUTTON",
        b"HiddenBlock",
        b"Hidden menu",
        b"HiddenStyle",
        b"Hidden address",
        b"Hidden code",
        b"Hidden merge value",
        b"HiddenBookmark",
        b"Word.Document.8",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden template/generated/embedded field leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn shape_fields_render_stored_result_without_synthesizing_canvas_or_leaking_to_pdf() {
    let input = br#"{\rtf1 Visible before
stored {\field{\*\fldinst SHAPE \* MERGEFORMAT}{\fldrslt Visible drawing fallback}}
resultless {\field{\*\fldinst SHAPE \* MERGEFORMAT}}
visible after\par}"#
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("stored Visible drawing fallback"));
    assert!(text.contains("resultless [Field removed: no passive result]"));
    assert!(text.contains("visible after"));
    for forbidden in ["SHAPE", "MERGEFORMAT", "fldinst", "fldrslt"] {
        assert!(
            !text.contains(forbidden),
            "forbidden SHAPE field content leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("stored Visible drawing fallback"));
    assert!(rendered_text.contains("resultless [Field removed: no passive result]"));
    assert!(rendered_text.contains("visible after"));
    for forbidden in [
        b"SHAPE".as_slice(),
        b"MERGEFORMAT",
        b"fldinst",
        b"fldrslt",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden SHAPE field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_active_control_fields_do_not_create_controls_or_leak_to_pdf() {
    let input = br#"{\rtf1 Visible before
control {\field{\*\fldinst CONTROL Forms.CommandButton.1 "Hidden caption"}}
checkbox {\field{\*\fldinst HTMLCHECKBOX "secret-name" checked}}
input {\field{\*\fldinst HTMLINPUT "password" value="secret"}}
select {\field{\*\fldinst HTMLSELECT "Secret option"}}
visible after\par}"#
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("visible after"));
    assert_eq!(
        text.matches("[Field removed: no passive result]").count(),
        4
    );
    for forbidden in [
        "CONTROL",
        "HTMLCHECKBOX",
        "HTMLINPUT",
        "HTMLSELECT",
        "Forms.CommandButton",
        "Hidden caption",
        "secret-name",
        "password",
        "value=",
        "Secret option",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden active control field leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("visible after"));
    assert!(rendered_text.contains("[Field removed: no passive result]"));
    for forbidden in [
        b"CONTROL".as_slice(),
        b"HTMLCHECKBOX",
        b"HTMLINPUT",
        b"HTMLSELECT",
        b"Forms.CommandButton",
        b"Hidden caption",
        b"secret-name",
        b"password",
        b"value=",
        b"Secret option",
        b"fldinst",
        b"/AcroForm",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden active control field leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_environment_fields_do_not_expose_host_state_to_pdf() {
    let input = br#"{\rtf1 Visible before
{\field{\*\fldinst FILENAME \p}}
{\field{\*\fldinst FILESIZE}}
{\field{\*\fldinst TEMPLATE}}
{\field{\*\fldinst USERNAME}}
{\field{\*\fldinst USERINITIALS}}
{\field{\*\fldinst USERADDRESS}}
visible after\par}"#
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("visible after"));
    assert_eq!(
        text.matches("[Field removed: no passive result]").count(),
        6
    );
    for forbidden in [
        "FILENAME",
        "FILESIZE",
        "TEMPLATE",
        "USERNAME",
        "USERINITIALS",
        "USERADDRESS",
        "Users\\",
        "open-rtf-converter",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden environment field leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("visible after"));
    assert!(rendered_text.contains("[Field removed: no passive result]"));
    for forbidden in [
        b"FILENAME".as_slice(),
        b"FILESIZE",
        b"TEMPLATE",
        b"USERNAME",
        b"USERINITIALS",
        b"USERADDRESS",
        b"Users\\",
        b"open-rtf-converter",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden environment field leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_layout_and_clock_fields_do_not_leak_to_pdf() {
    let input = br#"{\rtf1 Visible before
{\field{\*\fldinst ADVANCE \r 240 \d 120}}
after advance
date {\field{\*\fldinst DATE \\@ "yyyy-MM-dd host-sentinel"}}
time {\field{\*\fldinst TIME \\@ "HH:mm host-sentinel"}}
visible after\par}"#
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("after advance"));
    assert!(text.contains("date [Field removed: no passive result]"));
    assert!(text.contains("time [Field removed: no passive result]"));
    assert!(text.contains("visible after"));
    for forbidden in [
        "ADVANCE",
        "DATE",
        "TIME",
        "host-sentinel",
        "yyyy-MM-dd",
        "HH:mm",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden layout/clock field leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("after advance"));
    assert!(rendered_text.contains("date [Field removed: no passive result]"));
    assert!(rendered_text.contains("time [Field removed: no passive result]"));
    assert!(rendered_text.contains("visible after"));
    for forbidden in [
        b"ADVANCE".as_slice(),
        b"DATE",
        b"TIME",
        b"host-sentinel",
        b"yyyy-MM-dd",
        b"HH:mm",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden layout/clock field leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_prompt_fields_do_not_request_input_or_leak_to_pdf() {
    let input = br#"{\rtf1 Visible before
{\field{\*\fldinst ASK Client "Hidden prompt" \d "Hidden default"}}
ask-ref {\field{\*\fldinst REF Client}}
fill {\field{\*\fldinst FILLIN "Secret prompt" \d "Secret default"}}
visible after\par}"#
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("ask-ref [Field removed: no passive result]"));
    assert!(text.contains("fill [Field removed: no passive result]"));
    assert!(text.contains("visible after"));
    for forbidden in [
        "ASK",
        "FILLIN",
        "Client",
        "Hidden prompt",
        "Hidden default",
        "Secret prompt",
        "Secret default",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden prompt field leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("ask-ref [Field removed: no passive result]"));
    assert!(rendered_text.contains("fill [Field removed: no passive result]"));
    assert!(rendered_text.contains("visible after"));
    for forbidden in [
        b"ASK".as_slice(),
        b"FILLIN",
        b"Client",
        b"Hidden prompt",
        b"Hidden default",
        b"Secret prompt",
        b"Secret default",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden prompt field leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_mail_merge_flow_fields_do_not_execute_or_leak_to_pdf() {
    let input = br#"{\rtf1 Visible before
{\field{\*\fldinst NEXT}}
nextif {\field{\*\fldinst NEXTIF "City" = "Paris"}}
skip {\field{\*\fldinst SKIPIF "Status" = "Hidden"}}
record {\field{\*\fldinst MERGEREC}}
seq {\field{\*\fldinst MERGESEQ}}
name {\field{\*\fldinst MERGEFIELD Client}}
visible after\par}"#
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("nextif"));
    assert!(text.contains("skip"));
    assert!(text.contains("record [Field removed: no passive result]"));
    assert!(text.contains("seq [Field removed: no passive result]"));
    assert!(text.contains("name \u{00ab}Client\u{00bb}"));
    assert!(text.contains("visible after"));
    for forbidden in [
        "NEXT",
        "NEXTIF",
        "SKIPIF",
        "MERGEREC",
        "MERGESEQ",
        "MERGEFIELD",
        "City",
        "Paris",
        "Status",
        "Hidden",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden mail-merge field leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("record [Field removed: no passive result]"));
    assert!(rendered_text.contains("seq [Field removed: no passive result]"));
    assert!(rendered_text.contains("name"));
    assert!(rendered_text.contains("Client"));
    assert!(rendered_text.contains("visible after"));
    for forbidden in [
        b"NEXT".as_slice(),
        b"NEXTIF",
        b"SKIPIF",
        b"MERGEREC",
        b"MERGESEQ",
        b"MERGEFIELD",
        b"City",
        b"Paris",
        b"Status",
        b"Hidden",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden mail-merge field leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_mail_merge_address_fields_do_not_execute_or_leak_to_pdf() {
    let input = br#"{\rtf1 Visible before
address {\field{\*\fldinst ADDRESSBLOCK \f "<<_COMPANY_>>\r<<_ADDRESS1_>>" \l 1033}}
greeting {\field{\*\fldinst GREETINGLINE \f "<<_TITLE0_>> <<_LAST0_>>" \e "Dear Sir or Madam,"}}
visible after\par}"#
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("address [Field removed: no passive result]"));
    assert!(text.contains("greeting [Field removed: no passive result]"));
    assert!(text.contains("visible after"));
    for forbidden in [
        "ADDRESSBLOCK",
        "GREETINGLINE",
        "_COMPANY_",
        "_ADDRESS1_",
        "_TITLE0_",
        "_LAST0_",
        "Dear Sir",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden mail-merge address field leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("address [Field removed: no passive result]"));
    assert!(rendered_text.contains("greeting [Field removed: no passive result]"));
    assert!(rendered_text.contains("visible after"));
    for forbidden in [
        b"ADDRESSBLOCK".as_slice(),
        b"GREETINGLINE",
        b"_COMPANY_",
        b"_ADDRESS1_",
        b"_TITLE0_",
        b"_LAST0_",
        b"Dear Sir",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden mail-merge address field leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_private_and_print_fields_are_stripped_without_pdf_leakage() {
    let input = br#"{\rtf1 Visible before
{\field{\*\fldinst ADDIN "Hidden addin payload"}}
addin {\field{\*\fldinst PRIVATE "414243 hidden private"}}
private {\field{\*\fldinst PRINT "hidden printer command"}}
print {\field{\*\fldinst RD "C:\\secret\\source.rtf"}}
visible after\par}"#
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("addin"));
    assert!(text.contains("private"));
    assert!(text.contains("print"));
    assert!(text.contains("visible after"));
    assert!(!text.contains("[Field removed"));
    for forbidden in [
        "ADDIN",
        "PRIVATE",
        "PRINT",
        "RD",
        "Hidden addin payload",
        "414243",
        "hidden private",
        "hidden printer command",
        "secret",
        "source.rtf",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden hidden active/storage field leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("addin"));
    assert!(rendered_text.contains("private"));
    assert!(rendered_text.contains("print"));
    assert!(rendered_text.contains("visible after"));
    assert!(!rendered_text.contains("[Field removed"));
    for forbidden in [
        b"ADDIN".as_slice(),
        b"PRIVATE",
        b"PRINT",
        b"RD",
        b"Hidden addin payload",
        b"414243",
        b"hidden private",
        b"hidden printer command",
        b"secret",
        b"source.rtf",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden hidden active/storage field leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn edit_time_field_uses_passive_metadata_without_pdf_leakage() {
    let input = br#"{\rtf1{\info{\edmins42}{\title Hidden title {\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt Hidden link}}}}Edit {\field{\*\fldinst EDITTIME \\# "0000"}} dynamic {\field{\*\fldinst TIME}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Edit 0042"));
    assert!(text.contains("dynamic [Field removed: no passive result]"));
    for forbidden in [
        "EDITTIME",
        "edmins",
        "TIME",
        "HYPERLINK",
        "https://example.com",
        "Hidden title",
        "Hidden link",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden edit-time metadata leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("edit-time-field.rtf");
    let output_path = dir.path().join("edit-time-field.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Edit 0042"));
    assert!(rendered_text.contains("dynamic [Field removed: no passive result]"));
    for forbidden in [
        b"EDITTIME".as_slice(),
        b"edmins",
        b"HYPERLINK",
        b"https://example.com",
        b"Hidden title",
        b"Hidden link",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden edit-time metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn revision_number_field_uses_passive_metadata_without_pdf_leakage() {
    let input = br#"{\rtf1{\info{\version12}{\subject Hidden subject {\field{\*\fldinst HYPERLINK "https://example.com/revision"}{\fldrslt Hidden link}}}}Revision {\field{\*\fldinst REVNUM \\# "0000"}} dynamic {\field{\*\fldinst DATE}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Revision 0012"));
    assert!(text.contains("dynamic [Field removed: no passive result]"));
    for forbidden in [
        "REVNUM",
        "version",
        "DATE",
        "HYPERLINK",
        "https://example.com",
        "Hidden subject",
        "Hidden link",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden revision metadata leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("revision-number-field.rtf");
    let output_path = dir.path().join("revision-number-field.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Revision 0012"));
    assert!(rendered_text.contains("dynamic [Field removed: no passive result]"));
    for forbidden in [
        b"REVNUM".as_slice(),
        b"version",
        b"HYPERLINK",
        b"https://example.com",
        b"Hidden subject",
        b"Hidden link",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden revision metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_index_entry_fields_are_stripped_without_pdf_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Visible before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst XE \"https://example.com/hidden-index\"}} visible after {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst TC \"Hidden TOC entry\"}}{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst TA \"Hidden authority entry\"}}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("visible after"));
    assert!(!text.contains("[Field removed"));
    for forbidden in [
        "https://example.com",
        "Hidden TOC",
        "Hidden authority",
        "fldinst",
        "XE",
        "TC",
        "TA",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden non-visible field content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("index-entry-fields.rtf");
    let output_path = dir.path().join("index-entry-fields.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("visible after"));
    for forbidden in [
        b"https://example.com".as_slice(),
        b"Hidden TOC",
        b"Hidden authority",
        b"fldinst",
        b"XE",
        b"TC",
        b"TA",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden non-visible field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn form_field_metadata_does_not_create_pdf_forms_or_leak_payloads() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst FORMTEXT}{",
        "\\",
        "formfield{",
        "\\",
        "fftype0}{",
        "\\",
        "ffname HiddenName}{",
        "\\",
        "ffdeftext HiddenDefault}{",
        "\\",
        "ffentrymcr launch.exe}{",
        "\\",
        "ffexitmcr https://example.com/macro}{",
        "\\",
        "datafield 414243}}{",
        "\\",
        "fldrslt Visible value}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before Visible value After"));
    for forbidden in [
        "FORMTEXT",
        "HiddenName",
        "HiddenDefault",
        "launch.exe",
        "https://example.com",
        "414243",
        "ffentrymcr",
        "ffexitmcr",
        "datafield",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden form-field metadata leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("form-field-metadata.rtf");
    let output_path = dir.path().join("form-field-metadata.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(decoded_pdf_text(&content).contains("Before Visible value After"));
    for forbidden in [
        b"FORMTEXT".as_slice(),
        b"HiddenName",
        b"HiddenDefault",
        b"launch.exe",
        b"https://example.com",
        b"414243",
        b"ffentrymcr",
        b"ffexitmcr",
        b"datafield",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden form-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn form_field_macros_obey_reject_policy_without_breaking_passive_defaults() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst FORMTEXT}{",
        "\\",
        "formfield{",
        "\\",
        "fftype0}{",
        "\\",
        "ffdeftext Passive default}{",
        "\\",
        "ffentrymcr launch.exe}{",
        "\\",
        "ffexitmcr https://example.com/macro}}} After",
        "\\",
        "par}",
    ]);

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Before Passive default After"));
    assert!(!text.contains("launch.exe"));
    assert!(!text.contains("https://example.com/macro"));

    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };
    assert!(matches!(
        parse_rtf_bytes_with_options(&input, &reject_options),
        Err(ParseError::ActiveContentRejected { feature, .. }) if feature == "field instruction"
    ));
    assert!(matches!(
        parse_rtf_bytes_with_options(
            br"{\rtf1 Visible{\formfield{\ffentrymcr launch.exe}} body\par}",
            &reject_options,
        ),
        Err(ParseError::ActiveContentRejected { feature, .. }) if feature == "form field macro"
    ));
}

#[test]
fn resultless_form_text_renders_default_without_metadata_or_pdf_form() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst FORMTEXT}{",
        "\\",
        "formfield{",
        "\\",
        "fftype0}{",
        "\\",
        "ffname HiddenName}{",
        "\\",
        "ffdeftext Default value}{",
        "\\",
        "ffentrymcr launch.exe}{",
        "\\",
        "ffexitmcr https://example.com/macro}{",
        "\\",
        "datafield 414243}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before Default value After"));
    for forbidden in [
        "FORMTEXT",
        "HiddenName",
        "launch.exe",
        "https://example.com",
        "414243",
        "ffentrymcr",
        "ffexitmcr",
        "datafield",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden form-text metadata leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("form-text-default.rtf");
    let output_path = dir.path().join("form-text-default.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(decoded_pdf_text(&content).contains("Before Default value After"));
    for forbidden in [
        b"FORMTEXT".as_slice(),
        b"HiddenName",
        b"launch.exe",
        b"https://example.com",
        b"414243",
        b"ffentrymcr",
        b"ffexitmcr",
        b"datafield",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden form-text content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn formshade_renders_passive_form_field_background_without_pdf_form() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "formshade Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst FORMTEXT}{",
        "\\",
        "formfield{",
        "\\",
        "fftype0}{",
        "\\",
        "ffname HiddenName}{",
        "\\",
        "ffdeftext Default value}{",
        "\\",
        "ffentrymcr launch.exe}{",
        "\\",
        "datafield 414243}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        other => panic!("expected paragraph, got {other:?}"),
    };
    let form_run = paragraph
        .runs
        .iter()
        .find(|run| run.text == "Default value")
        .expect("form default run");

    assert!(text.contains("Before Default value After"));
    assert!(form_run.style.form_field_shading);
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("form-field shading approximated")
    }));
    for forbidden in [
        "FORMTEXT",
        "HiddenName",
        "launch.exe",
        "414243",
        "formshade",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden formshade metadata leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before Default value After"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "form shading should render as a passive rectangle"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "f"),
        "form shading should render as a passive fill"
    );
    for forbidden in [
        b"FORMTEXT".as_slice(),
        b"HiddenName",
        b"launch.exe",
        b"414243",
        b"formshade",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden formshade content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn formshade_applies_to_stored_form_field_result_without_pdf_form() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "formshade Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst FORMTEXT}{",
        "\\",
        "formfield{",
        "\\",
        "fftype0}{",
        "\\",
        "ffname HiddenName}{",
        "\\",
        "ffdeftext HiddenDefault}{",
        "\\",
        "ffentrymcr launch.exe}{",
        "\\",
        "datafield 414243}}{",
        "\\",
        "fldrslt Visible value}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        other => panic!("expected paragraph, got {other:?}"),
    };
    let form_run = paragraph
        .runs
        .iter()
        .find(|run| run.text == "Visible value")
        .expect("stored form result run");

    assert!(text.contains("Before Visible value After"));
    assert!(
        form_run.style.form_field_shading,
        "stored form-field result should carry passive shading style"
    );
    for forbidden in [
        "FORMTEXT",
        "HiddenName",
        "HiddenDefault",
        "launch.exe",
        "414243",
        "formshade",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden stored formshade metadata leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(decoded_pdf_text(&content).contains("Before Visible value After"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "stored form shading should render as a passive rectangle"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "f"),
        "stored form shading should render as a passive fill"
    );
    for forbidden in [
        b"FORMTEXT".as_slice(),
        b"HiddenName",
        b"HiddenDefault",
        b"launch.exe",
        b"414243",
        b"formshade",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden stored formshade content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_form_checkbox_renders_passively_without_metadata_or_pdf_form() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst FORMCHECKBOX}{",
        "\\",
        "formfield{",
        "\\",
        "fftype1}{",
        "\\",
        "ffname HiddenName}{",
        "\\",
        "ffdefres0}{",
        "\\",
        "ffres1}{",
        "\\",
        "ffentrymcr launch.exe}{",
        "\\",
        "datafield 414243}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before \u{2611} After"));
    assert!(
        parsed
            .document
            .fonts
            .iter()
            .any(|font| font.name == "ZapfDingbats")
    );
    for forbidden in [
        "FORMCHECKBOX",
        "HiddenName",
        "launch.exe",
        "414243",
        "ffentrymcr",
        "datafield",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden checkbox form-field metadata leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("form-checkbox.rtf");
    let output_path = dir.path().join("form-checkbox.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let stroke_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "S")
        .count();

    assert!(rendered_text.contains("Before "));
    assert!(rendered_text.contains(" After"));
    assert!(
        stroke_count >= 1,
        "checked checkbox should draw vector strokes"
    );
    assert_passive_checkbox_vectors_without_zapf(&pdf, &content, "checked form checkbox");
    for forbidden in [
        b"FORMCHECKBOX".as_slice(),
        b"HiddenName",
        b"launch.exe",
        b"414243",
        b"ffentrymcr",
        b"datafield",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden checkbox form-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn unchecked_form_checkbox_renders_passive_font_glyph_without_pdf_form() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst FORMCHECKBOX}{",
        "\\",
        "formfield{",
        "\\",
        "fftype1}{",
        "\\",
        "ffname HiddenUnchecked}{",
        "\\",
        "ffres0}{",
        "\\",
        "ffentrymcr launch.exe}{",
        "\\",
        "datafield 414243}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before \u{2610} After"));
    assert!(
        parsed
            .document
            .fonts
            .iter()
            .any(|font| font.name == "ZapfDingbats")
    );
    for forbidden in [
        "FORMCHECKBOX",
        "HiddenUnchecked",
        "launch.exe",
        "414243",
        "ffentrymcr",
        "datafield",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden unchecked checkbox metadata leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("unchecked-form-checkbox.rtf");
    let output_path = dir.path().join("unchecked-form-checkbox.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before "));
    assert!(rendered_text.contains(" After"));
    assert_passive_checkbox_vectors_without_zapf(&pdf, &content, "unchecked form checkbox");
    for forbidden in [
        b"FORMCHECKBOX".as_slice(),
        b"HiddenUnchecked",
        b"launch.exe",
        b"414243",
        b"ffentrymcr",
        b"datafield",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden unchecked checkbox content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_form_dropdown_renders_selected_entry_without_metadata_or_pdf_form() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst FORMDROPDOWN}{",
        "\\",
        "formfield{",
        "\\",
        "fftype2}{",
        "\\",
        "ffname HiddenName}{",
        "\\",
        "ffdefres0}{",
        "\\",
        "ffres1}{",
        "\\",
        "*",
        "\\",
        "ffl First choice}{",
        "\\",
        "*",
        "\\",
        "ffl Second choice}{",
        "\\",
        "ffentrymcr launch.exe}{",
        "\\",
        "datafield 414243}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before Second choice After"));
    for forbidden in [
        "FORMDROPDOWN",
        "HiddenName",
        "First choice",
        "launch.exe",
        "414243",
        "ffentrymcr",
        "datafield",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden dropdown form-field metadata leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("form-dropdown.rtf");
    let output_path = dir.path().join("form-dropdown.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(decoded_pdf_text(&content).contains("Before Second choice After"));
    for forbidden in [
        b"FORMDROPDOWN".as_slice(),
        b"HiddenName",
        b"First choice",
        b"launch.exe",
        b"414243",
        b"ffentrymcr",
        b"datafield",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden dropdown form-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn document_protection_metadata_does_not_reach_text_or_pdf() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "formprot",
        "\\",
        "revprot",
        "\\",
        "annotprot{",
        "\\",
        "passwordhash DEADBEEFCAFE0123456789ABCDEF0123}Visible protected body",
        "\\",
        "par",
        "\\",
        "passwordhash AABBCCDDEEFF Inline body",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible protected body"));
    assert!(text.contains("Inline body"));
    for forbidden in [
        "passwordhash",
        "DEADBEEF",
        "AABBCC",
        "formprot",
        "revprot",
        "annotprot",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden document protection metadata leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("document-protection.rtf");
    let output_path = dir.path().join("document-protection.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible protected body"));
    assert!(rendered_text.contains("Inline body"));
    for forbidden in [
        b"passwordhash".as_slice(),
        b"DEADBEEF",
        b"AABBCC",
        b"formprot",
        b"revprot",
        b"annotprot",
        b"/AcroForm",
        b"/Encrypt",
        b"/Perms",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden document protection content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn document_protection_password_hash_obeys_reject_policy() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "passwordhash AABBCCDDEEFF",
        "\\",
        "par Visible protected body",
        "\\",
        "par}",
    ]);
    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };

    assert!(matches!(
        parse_rtf_bytes_with_options(&input, &reject_options),
        Err(ParseError::ActiveContentRejected { feature, .. })
            if feature == "document protection password hash"
    ));
}

#[test]
fn dynamic_date_time_controls_do_not_evaluate_or_leak_to_pdf() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before ",
        "\\",
        "chdate ",
        "\\",
        "chtime ",
        "\\",
        "chdpa ",
        "\\",
        "chdpl After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before"));
    assert!(text.contains("[Dynamic date/time removed]"));
    assert!(text.contains("After"));
    assert!(!text.contains("chdate"));
    assert!(!text.contains("chtime"));
    assert!(!text.contains("chdpa"));
    assert!(!text.contains("chdpl"));

    let strip_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Strip,
        ..RtfParseOptions::default()
    };
    let stripped = parse_rtf_bytes_with_options(&input, &strip_options).unwrap();
    let stripped_text = collect_text(&stripped.document);
    assert!(stripped_text.contains("Before"));
    assert!(stripped_text.contains("After"));
    assert!(!stripped_text.contains("[Dynamic date/time removed]"));

    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };
    assert!(matches!(
        parse_rtf_bytes_with_options(&input, &reject_options),
        Err(ParseError::ActiveContentRejected { feature, .. })
            if feature == "dynamic date/time control"
    ));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("dynamic-date-time.rtf");
    let output_path = dir.path().join("dynamic-date-time.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("[Dynamic date/time removed]"));
    for forbidden in [
        b"chdate".as_slice(),
        b"chtime",
        b"chdpa",
        b"chdpl",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden dynamic date/time content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn metadata_and_external_templates_do_not_reach_text_or_pdf() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "info{",
        "\\",
        "title Hidden title}{",
        "\\",
        "author Hidden author}{",
        "\\",
        "doccomm Hidden comment}}{",
        "\\",
        "template https://example.com/template.dotm}Visible body",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible body"));
    assert!(!text.contains("Hidden title"));
    assert!(!text.contains("Hidden author"));
    assert!(!text.contains("Hidden comment"));
    assert!(!text.contains("template.dotm"));
    assert!(!text.contains("https://example.com"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("metadata-template.rtf");
    let output_path = dir.path().join("metadata-template.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"Hidden title".as_slice(),
        b"Hidden author",
        b"Hidden comment",
        b"template.dotm",
        b"https://example.com",
        b"/URI",
        b"/JavaScript",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden metadata/template content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }

    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };
    assert!(matches!(
        parse_rtf_bytes_with_options(
            &rtf(&[
                "{",
                "\\",
                "rtf1{",
                "\\",
                "template https://example.com/template.dotm}}"
            ]),
            &reject_options,
        ),
        Err(ParseError::ActiveContentRejected { feature, .. }) if feature == "external template"
    ));
}

#[test]
fn macro_and_script_destinations_do_not_reach_text_or_pdf() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Visible before {",
        "\\",
        "macros launch.exe}{",
        "\\",
        "script <script>414243</script>}{",
        "\\",
        "*",
        "\\",
        "vbaproject Hidden VBA payload}{",
        "\\",
        "info{",
        "\\",
        "title Hidden {",
        "\\",
        "activex Forms.CommandButton.1}}} visible after",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("visible after"));
    for forbidden in [
        "launch.exe",
        "script",
        "414243",
        "Hidden VBA payload",
        "Forms.CommandButton",
        "macros",
        "vbaproject",
        "activex",
    ] {
        assert!(
            !text.contains(forbidden),
            "macro/script payload leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "active content removed: macro/script payload before safe model normalization",
        )
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("visible after"));
    for forbidden in [
        b"launch.exe".as_slice(),
        b"<script>",
        b"414243",
        b"Hidden VBA payload",
        b"Forms.CommandButton",
        b"macros",
        b"vbaproject",
        b"activex",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "macro/script payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn macro_and_script_destinations_obey_reject_policy() {
    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };

    for (input, expected_feature) in [
        (
            br"{\rtf1{\macros launch.exe}Visible body\par}".as_slice(),
            "macro/script payload",
        ),
        (
            br"{\rtf1{\info{\title Hidden {\script launch.exe}}}Visible body\par}".as_slice(),
            "macro/script payload in metadata",
        ),
        (
            br"{\rtf1{\*\vbaproject Hidden VBA payload}Visible body\par}".as_slice(),
            "macro/script payload in skipped destination",
        ),
    ] {
        let result = parse_rtf_bytes_with_options(input, &reject_options);
        assert!(
            matches!(
                result,
                Err(ParseError::ActiveContentRejected { ref feature, .. })
                    if feature == expected_feature
            ),
            "expected {expected_feature:?}, got {result:?}"
        );
    }
}

#[test]
fn active_controls_nested_in_metadata_obey_reject_policy() {
    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };

    for (input, expected_feature) in [
        (
            br"{\rtf1{\info{\title Hidden {\object\objdata 414243}}}Body\par}".as_slice(),
            "object payload in metadata",
        ),
        (
            br#"{\rtf1{\info{\title Hidden {\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt Hidden link}}}}Body\par}"#.as_slice(),
            "field instruction in metadata",
        ),
        (
            br"{\rtf1{\info{\title Hidden {\fontemb{\fontfile HOSTILE-FONT-PAYLOAD}}}}Body\par}".as_slice(),
            "embedded font payload in metadata",
        ),
        (
            br"{\rtf1{\info{\title Hidden {\template https://example.com/t.dotm}}}Body\par}".as_slice(),
            "external template in metadata",
        ),
        (
            br"{\rtf1{\info{\title Hidden {\mmdatasource https://example.com/data.csv}}}Body\par}".as_slice(),
            "mail merge data source in metadata",
        ),
        (
            br"{\rtf1{\info{\title Hidden {\annotation comment payload}}}Body\par}".as_slice(),
            "annotation metadata in metadata",
        ),
    ] {
        let result = parse_rtf_bytes_with_options(input, &reject_options);
        assert!(
            matches!(
                result,
                Err(ParseError::ActiveContentRejected { ref feature, .. })
                    if feature == expected_feature
            ),
            "expected {expected_feature:?}, got {result:?}"
        );
    }

    let parsed = parse_rtf_bytes(
        br#"{\rtf1{\info{\title Hidden {\object\objdata 414243}{\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt Hidden link}}{\fontemb{\fontfile HOSTILE-FONT-PAYLOAD}}{\template https://example.com/t.dotm}{\mmdatasource https://example.com/data.csv}{\annotation comment payload}}}Body\par}"#,
    )
    .unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Body"));
    for forbidden in [
        "Hidden",
        "414243",
        "HYPERLINK",
        "example.com",
        "Hidden link",
        "HOSTILE-FONT-PAYLOAD",
        "data.csv",
        "comment payload",
        "[Embedded object removed]",
        "[Field removed",
    ] {
        assert!(
            !text.contains(forbidden),
            "metadata active payload leaked to text: {forbidden}"
        );
    }
    for expected in [
        "active content removed: object payload in metadata before safe model normalization",
        "active content removed: field instruction in metadata before safe model normalization",
        "active content removed: embedded font payload in metadata before safe model normalization",
        "active content removed: external template in metadata before safe model normalization",
        "active content removed: mail merge data source in metadata before safe model normalization",
        "active content removed: annotation metadata in metadata before safe model normalization",
    ] {
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "missing metadata active-content diagnostic {expected:?}: {:?}",
            parsed.diagnostics
        );
    }
}

#[test]
fn encapsulated_html_metadata_does_not_warn_or_reach_text_or_pdf() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "fromhtml1",
        "\\",
        "htmlrtf1",
        "\\",
        "b",
        "\\",
        "htmlrtf0 Visible body{",
        "\\",
        "*",
        "\\",
        "htmltag <p onclick=\"launch.exe\">Hidden HTML</p>}{",
        "\\",
        "htmltag <script>414243</script>}{",
        "\\",
        "htmlbase https://example.com/base/}{",
        "\\",
        "*",
        "\\",
        "htmltag {",
        "\\",
        "object",
        "\\",
        "objdata 414243}} after",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible body after"));
    for forbidden in [
        "Hidden HTML",
        "script",
        "onclick",
        "launch.exe",
        "htmltag",
        "htmlbase",
        "htmlrtf",
        "fromhtml",
        "https://example.com",
        "objdata",
        "414243",
    ] {
        assert!(
            !text.contains(forbidden),
            "encapsulated HTML metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "encapsulated HTML wrapper controls should be classified, got {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible body after"));
    for forbidden in [
        b"Hidden HTML".as_slice(),
        b"script",
        b"onclick",
        b"launch.exe",
        b"htmltag",
        b"htmlbase",
        b"htmlrtf",
        b"fromhtml",
        b"https://example.com",
        b"objdata",
        b"414243",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "encapsulated HTML metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn encapsulated_html_metadata_obeys_reject_policy() {
    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };

    for input in [
        br"{\rtf1{\*\htmltag <script>launch.exe</script>}Visible body\par}".as_slice(),
        br"{\rtf1{\htmlbase https://example.com/base/}Visible body\par}".as_slice(),
        br#"{\rtf1{\info{\htmltag <p onclick="launch.exe">Hidden</p>}}Visible body\par}"#
            .as_slice(),
        br"{\rtf1{\*\unknown{\htmltag <script>414243</script>}}Visible body\par}".as_slice(),
    ] {
        assert!(matches!(
            parse_rtf_bytes_with_options(input, &reject_options),
            Err(ParseError::ActiveContentRejected { feature, .. })
                if feature == "encapsulated HTML metadata"
        ));
    }
}

#[test]
fn review_bookmark_and_annotation_payloads_do_not_reach_text_or_pdf() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Visible before {",
        "\\",
        "annotation Hidden comment {",
        "\\",
        "object",
        "\\",
        "objdata ",
        payload_hex(),
        "}{",
        "\\",
        "result Hidden result}{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst HYPERLINK \"https://example.com/comment\"}{",
        "\\",
        "fldrslt Hidden link}}}{",
        "\\",
        "bkmkstart SecretBookmark}{",
        "\\",
        "*",
        "\\",
        "bkmkend SecretBookmark}{",
        "\\",
        "deleted Deleted text} visible after",
        " {",
        "\\",
        "revised",
        "\\",
        "revauth1",
        "\\",
        "revdttm123456789",
        "\\",
        "insrsid42 Inserted text}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before  visible after Inserted text"));
    for forbidden in [
        "Hidden comment",
        "Hidden result",
        "Hidden link",
        "SecretBookmark",
        "Deleted text",
        "revised",
        "revauth",
        "revdttm",
        "insrsid",
        "https://example.com/comment",
        payload_hex(),
        "[Embedded object removed]",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden review/bookmark content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("review-bookmark.rtf");
    let output_path = dir.path().join("review-bookmark.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"Hidden comment".as_slice(),
        b"Hidden result",
        b"Hidden link",
        b"SecretBookmark",
        b"Deleted text",
        b"revised",
        b"revauth",
        b"revdttm",
        b"insrsid",
        b"https://example.com/comment",
        payload_hex().as_bytes(),
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden review/bookmark content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn character_animation_controls_render_static_text_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "animtext5 Animated text",
        "\\",
        "animtext0 Static text",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Animated textStatic text"));
    assert!(!text.contains("animtext"));
    assert!(
        parsed.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("character animation stripped for passive static PDF output")),
        "missing character animation diagnostic: {:?}",
        parsed.diagnostics
    );
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "animtext should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    assert!(
        output.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("character animation stripped for passive static PDF output")),
        "conversion diagnostics should include character animation warning: {:?}",
        output.diagnostics
    );
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Animated textStatic text"));
    for forbidden in [
        b"animtext".as_slice(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden character animation content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn annotation_metadata_obeys_reject_policy() {
    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };

    assert!(matches!(
        parse_rtf_bytes_with_options(
            br"{\rtf1 Visible{\annotation Hidden comment {\object\objdata 414243}} body\par}",
            &reject_options,
        ),
        Err(ParseError::ActiveContentRejected { feature, .. }) if feature == "annotation metadata"
    ));
}

#[test]
fn mail_merge_metadata_does_not_reach_text_or_pdf() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Visible before {",
        "\\",
        "mailmerge{",
        "\\",
        "mmconnectstr Provider=SQLOLEDB;Password=secret;Data Source=https://example.com/db}{",
        "\\",
        "mmdatasource C:",
        "\\",
        "remote",
        "\\",
        "contacts.mdb}{",
        "\\",
        "mmquery SELECT * FROM Contacts WHERE Url='https://example.com/query'}{",
        "\\",
        "mmodsoudl https://example.com/source.udl}{",
        "\\",
        "object",
        "\\",
        "objdata ",
        payload_hex(),
        "}} visible after",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before  visible after"));
    for forbidden in [
        "Provider=SQLOLEDB",
        "Password=secret",
        "Data Source",
        "contacts.mdb",
        "SELECT *",
        "example.com",
        payload_hex(),
        "[Embedded object removed]",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden mail merge content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("mail-merge-metadata.rtf");
    let output_path = dir.path().join("mail-merge-metadata.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"Provider=SQLOLEDB".as_slice(),
        b"Password=secret",
        b"Data Source",
        b"contacts.mdb",
        b"SELECT *",
        b"example.com",
        payload_hex().as_bytes(),
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden mail merge content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn mail_merge_metadata_obeys_reject_policy() {
    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };

    assert!(matches!(
        parse_rtf_bytes_with_options(
            br"{\rtf1 Visible{\mailmerge{\mmconnectstr Provider=SQLOLEDB;Password=secret}} body\par}",
            &reject_options,
        ),
        Err(ParseError::ActiveContentRejected { feature, .. }) if feature == "mail merge data source"
    ));
}

#[test]
fn hidden_text_and_hidden_active_payloads_do_not_reach_text_or_pdf() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Visible {",
        "\\",
        "v Hidden text {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst INCLUDEPICTURE \"https://example.com/hidden.png\"}}{",
        "\\",
        "object",
        "\\",
        "objdata ",
        payload_hex(),
        "}}{",
        "\\",
        "v0 Shown",
        "\\",
        "par}",
        "}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible"));
    assert!(text.contains("Shown"));
    for forbidden in [
        "Hidden text",
        "https://example.com/hidden.png",
        payload_hex(),
        "[Field removed",
        "[Embedded object removed]",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden hidden content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("hidden-text.rtf");
    let output_path = dir.path().join("hidden-text.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"Hidden text".as_slice(),
        b"https://example.com/hidden.png",
        payload_hex().as_bytes(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden hidden content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn extreme_font_sizes_are_bounded_before_pdf_rendering() {
    let input = rtf(&["{", "\\", "rtf1 tiny ", "\\", "fs999999 huge", "\\", "par}"]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("tiny"));
    assert!(text.contains("huge"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("font-size.rtf");
    let output_path = dir.path().join("font-size.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [b"/JavaScript".as_slice(), b"/EmbeddedFile", b"/Launch"] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn symbol_font_charset_renders_passive_unicode_without_rtf_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Arial;}{",
        "\\",
        "f1",
        "\\",
        "fcharset2 Symbol;}}",
        "\\",
        "f1 ab ",
        "\\",
        "'b7",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("\u{03b1}\u{03b2} \u{2022}"));
    assert!(!text.contains("fcharset"));
    assert!(!text.contains("fonttbl"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("symbol-font.rtf");
    let output_path = dir.path().join("symbol-font.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    assert!(
        pdf.windows(b"/BaseFont /Symbol".len())
            .any(|window| window == b"/BaseFont /Symbol")
    );
    for forbidden in [
        b"fcharset".as_slice(),
        b"fonttbl",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn winansi_hex_ellipsis_renders_through_normal_text_font() {
    let input = br"{\rtf1\ansi\ansicpg1252{\fonttbl{\f0\froman Times New Roman;}{\f1\fcharset2 Symbol;}}\f0 Normal \'85\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Normal \u{2026}"));
    for forbidden in ["ansicpg", "fonttbl", "Symbol", "Times New Roman"] {
        assert!(
            !text.contains(forbidden),
            "forbidden font/codepage metadata leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let times_bytes = pdf_text_bytes_for_font(&content, b"F9");
    let symbol_bytes = pdf_text_bytes_for_font(&content, b"F13");

    assert!(
        times_bytes.contains(&0x85),
        "WinAnsi ellipsis should encode through passive Times byte 0x85, got {times_bytes:?}"
    );
    assert!(
        !symbol_bytes.contains(&0xbc),
        "normal WinAnsi ellipsis should not be rendered as Symbol byte 0xbc, got {symbol_bytes:?}"
    );
    for forbidden in [
        b"ansicpg".as_slice(),
        b"fonttbl",
        b"Times New Roman",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden WinAnsi ellipsis metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn symbol_pntext_bullets_render_as_passive_winansi_without_font_payload_leakage() {
    let input = br"{\rtf1{\fonttbl{\f0 Arial;}{\f1\fcharset2 Symbol;}}{\pntext\pard\plain\f1 \'b7\tab}\pard\fi-360\li360 Item\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("\u{2022}\tItem"));
    for forbidden in ["fonttbl", "fcharset", "Symbol", "pntext"] {
        assert!(
            !text.contains(forbidden),
            "forbidden Symbol bullet metadata leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let helvetica_bytes = pdf_text_bytes_for_font(&content, b"F1");
    let symbol_bytes = pdf_text_bytes_for_font(&content, b"F13");
    assert!(
        helvetica_bytes.contains(&0x95),
        "Symbol pntext bullet should encode through passive WinAnsi bullet byte, got {helvetica_bytes:?}"
    );
    assert!(
        !symbol_bytes.contains(&0xb7),
        "Symbol pntext bullet should not require PDF Symbol display bytes, got {symbol_bytes:?}"
    );
    for forbidden in [
        b"fonttbl".as_slice(),
        b"fcharset",
        b"pntext",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden Symbol bullet content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wingdings_checkbox_glyphs_render_passively_without_font_payload_leakage() {
    let input = br#"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Wingdings;}}\f1 \'a3 \'fe \'fc \'fb\par {\field{\*\fldinst SYMBOL 254 \\f "Wingdings"}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("\u{2610} \u{2611} \u{2713} \u{2717}"));
    assert!(text.contains("\u{2611}"));
    for forbidden in ["fonttbl", "Wingdings", "fldinst", "SYMBOL"] {
        assert!(
            !text.contains(forbidden),
            "forbidden Wingdings metadata leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("wingdings-checkboxes.rtf");
    let output_path = dir.path().join("wingdings-checkboxes.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert_passive_checkbox_vectors_without_zapf(&pdf, &content, "Wingdings checkbox glyphs");
    for forbidden in [
        b"Wingdings".as_slice(),
        b"fonttbl",
        b"fldinst",
        b"SYMBOL",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden Wingdings content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn associated_font_checkbox_glyphs_render_passively_without_font_payload_leakage() {
    let input =
        br#"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Wingdings;}}\loch\af1\afs72 \'a3 \'fe \'fc \'fb\par}"#
            .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("\u{2610} \u{2611} \u{2713} \u{2717}"));
    let paragraph = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .expect("paragraph");
    assert_eq!(paragraph.runs[0].style.font_index, 1);
    assert_eq!(paragraph.runs[0].style.font_size_half_points, 72);
    for forbidden in ["fonttbl", "Wingdings", "loch", "af1", "afs72"] {
        assert!(
            !text.contains(forbidden),
            "forbidden associated font metadata leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content.operations.iter().any(|operation| {
            operation.operator == "re"
                && operation
                    .operands
                    .get(2)
                    .and_then(pdf_operand_number)
                    .is_some_and(|size| size > 18.0)
        }),
        "associated font size should render large passive checkbox vector boxes"
    );
    assert_passive_checkbox_vectors_without_zapf(
        &output.pdf,
        &content,
        "associated-font checkbox glyphs",
    );
    for forbidden in [
        b"Wingdings".as_slice(),
        b"fonttbl",
        b"loch",
        b"af1",
        b"afs72",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden associated font content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn associated_character_properties_render_passively_without_control_leakage() {
    let input = br"{\rtf1{\colortbl;\red255\green0\blue0;}\loch\ab\ai\acf1\aul\aexpnd4\aup6 Associated styled{\object\objdata 414243}\par Plain\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Associated styled"));
    assert!(text.contains("Plain"));
    let paragraph = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .expect("paragraph");
    let styled = paragraph
        .runs
        .iter()
        .find(|run| run.text.contains("Associated styled"))
        .expect("associated styled run");
    assert!(styled.style.bold);
    assert!(styled.style.italic);
    assert_eq!(styled.style.color_index, 1);
    assert_eq!(styled.style.underline, UnderlineStyle::Single);
    assert_eq!(styled.style.character_spacing_twips, 20);
    assert_eq!(styled.style.baseline_shift_half_points, 6);
    for forbidden in ["acf1", "aul", "aexpnd4", "aup6", "objdata", "414243"] {
        assert!(
            !text.contains(forbidden),
            "forbidden associated character content leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let bold_italic_bytes = pdf_text_bytes_for_font(&content, b"F4");
    assert!(
        String::from_utf8_lossy(&bold_italic_bytes).contains("Associated styled"),
        "associated bold/italic text should use passive Helvetica-BoldOblique bytes, got {bold_italic_bytes:?}"
    );
    for forbidden in [
        b"acf1".as_slice(),
        b"aul",
        b"aexpnd4",
        b"aup6",
        b"objdata",
        b"414243",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden associated character content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emphasis_mark_controls_render_passive_overlays_without_control_leakage() {
    let input = br"{\rtf1\accdot Dot text \acccomma Comma text \accnone Plain text{\object\objdata 414243}\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Dot text"));
    assert!(text.contains("Comma text"));
    assert!(text.contains("Plain text"));
    let paragraph = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .expect("paragraph");
    let style_for = |needle: &str| {
        paragraph
            .runs
            .iter()
            .find(|run| run.text.contains(needle))
            .unwrap_or_else(|| panic!("missing run containing {needle}"))
            .style
            .emphasis_mark
    };
    assert_eq!(style_for("Dot text"), CharacterEmphasisMark::Dot);
    assert_eq!(style_for("Comma text"), CharacterEmphasisMark::Comma);
    assert_eq!(style_for("Plain text"), CharacterEmphasisMark::None);
    for forbidden in ["accdot", "acccomma", "accnone", "objdata", "414243"] {
        assert!(
            !text.contains(forbidden),
            "forbidden emphasis mark content leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Dot text"));
    assert!(rendered_text.contains("Comma text"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "dot emphasis should render as passive filled vector marks"
    );
    assert!(
        content.operations.windows(3).any(|operations| {
            operations[0].operator == "m"
                && operations[1].operator == "l"
                && operations[2].operator == "S"
        }),
        "comma emphasis should render as passive stroked vector marks"
    );
    for forbidden in [
        b"accdot".as_slice(),
        b"acccomma",
        b"accnone",
        b"objdata",
        b"414243",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden emphasis mark content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wingdings2_checkbox_glyphs_render_passively_without_font_payload_leakage() {
    let input = br#"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Wingdings 2;}}\f1 O P Q R S T\par {\field{\*\fldinst SYMBOL 82 \\f "Wingdings 2"}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("\u{2717} \u{2713} \u{2612} \u{2611} \u{2612} \u{2612}"));
    assert!(text.contains("\u{2611}"));
    for forbidden in ["fonttbl", "Wingdings 2", "fldinst", "SYMBOL"] {
        assert!(
            !text.contains(forbidden),
            "forbidden Wingdings 2 metadata leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert_passive_checkbox_vectors_without_zapf(&output.pdf, &content, "Wingdings 2 checkboxes");
    for forbidden in [
        b"Wingdings 2".as_slice(),
        b"fonttbl",
        b"fldinst",
        b"SYMBOL",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden Wingdings 2 content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wingdings3_basic_arrows_render_passively_without_font_payload_leakage() {
    let input = br#"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Wingdings 3;}}\f1 f g h i\par {\field{\*\fldinst SYMBOL 102 \\f "Wingdings 3"}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("\u{2190} \u{2192} \u{2191} \u{2193}"));
    assert!(text.contains("\u{2190}"));
    for forbidden in ["fonttbl", "Wingdings 3", "fldinst", "SYMBOL"] {
        assert!(
            !text.contains(forbidden),
            "forbidden Wingdings 3 metadata leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let symbol_bytes = pdf_text_bytes_for_font(&content, b"F13");
    let expected_arrows: &[u8] = &[0xac, 0xae, 0xad, 0xaf];
    assert!(
        symbol_bytes
            .windows(expected_arrows.len())
            .any(|window| window == expected_arrows)
            && symbol_bytes.contains(&0xac),
        "Wingdings 3 arrows should encode through passive Symbol bytes, got {symbol_bytes:?}"
    );
    assert!(
        output
            .pdf
            .windows(b"/BaseFont /Symbol".len())
            .any(|window| window == b"/BaseFont /Symbol")
    );
    for forbidden in [
        b"Wingdings 3".as_slice(),
        b"fonttbl",
        b"fldinst",
        b"SYMBOL",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden Wingdings 3 content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn webdings_checkbox_glyphs_render_passively_without_font_payload_leakage() {
    let input = br#"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Webdings;}}\f1 \'3f \'61 \'63\par {\field{\*\fldinst SYMBOL 63 \\f "Webdings"}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("\u{2612} \u{2714} \u{25a1}"));
    assert!(text.contains("\u{2612}"));
    for forbidden in ["fonttbl", "Webdings", "fldinst", "SYMBOL"] {
        assert!(
            !text.contains(forbidden),
            "forbidden Webdings metadata leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert_passive_checkbox_vectors_without_zapf(&output.pdf, &content, "Webdings checkboxes");
    for forbidden in [
        b"Webdings".as_slice(),
        b"fonttbl",
        b"fldinst",
        b"SYMBOL",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden Webdings content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn unicode_checkbox_glyphs_use_passive_checkbox_font_without_embedding_source_font() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Arial;}{",
        "\\",
        "f1 Segoe UI Symbol;}}",
        "\\",
        "f1 ",
        "\\",
        "u9744?",
        "\\",
        "u9745?",
        "\\",
        "u9746?",
        "\\",
        "u10003?",
        "\\",
        "u10007?",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("\u{2610}\u{2611}\u{2612}\u{2713}\u{2717}"));
    for forbidden in ["fonttbl", "Segoe UI Symbol"] {
        assert!(
            !text.contains(forbidden),
            "forbidden Unicode checkbox font metadata leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert_passive_checkbox_vectors_without_zapf(&output.pdf, &content, "Unicode checkboxes");
    for forbidden in [
        b"Segoe UI Symbol".as_slice(),
        b"fonttbl",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden Unicode checkbox content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn segoe_ui_symbol_latin_text_stays_readable_while_checkboxes_use_passive_font() {
    let input =
        br"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Segoe UI Symbol;}}\f1 Label \u9745?\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Label"));
    assert!(text.contains("\u{2611}"));
    for forbidden in ["fonttbl", "Segoe UI Symbol", "u9745"] {
        assert!(
            !text.contains(forbidden),
            "forbidden Segoe UI Symbol metadata leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let helvetica_bytes = pdf_text_bytes_for_font(&content, b"F1");
    let symbol_bytes = pdf_text_bytes_for_font(&content, b"F13");

    assert!(
        String::from_utf8_lossy(&helvetica_bytes).contains("Label "),
        "Segoe UI Symbol Latin text should use passive Helvetica bytes, got {helvetica_bytes:?}"
    );
    assert_passive_checkbox_vectors_without_zapf(&output.pdf, &content, "Segoe UI Symbol checkbox");
    assert!(
        symbol_bytes.is_empty(),
        "Segoe UI Symbol Latin text should not be emitted through PDF Symbol, got {symbol_bytes:?}"
    );
    for forbidden in [
        b"Segoe UI Symbol".as_slice(),
        b"fonttbl",
        b"u9745",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden Segoe UI Symbol content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn unicode_symbol_spans_use_passive_symbol_font_without_source_font_payload() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Arial;}}Alpha ",
        "\\",
        "u945?+",
        "\\",
        "u946? ",
        "\\",
        "u8804? ",
        "\\",
        "u937? done",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Alpha \u{03b1}+\u{03b2} \u{2264} \u{03a9} done"));
    for forbidden in ["fonttbl", "u945", "u946", "u8804", "u937"] {
        assert!(
            !text.contains(forbidden),
            "forbidden Unicode symbol source leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let font_names = pdf_text_font_names(&content);
    assert!(
        font_names.iter().any(|name| name.as_slice() == b"F1"),
        "normal text should stay on passive Helvetica substitution; font selections were {font_names:?}"
    );
    assert!(
        font_names.iter().any(|name| name.as_slice() == b"F13"),
        "Unicode Greek/math symbols should use passive Symbol substitution; font selections were {font_names:?}"
    );
    assert!(
        output
            .pdf
            .windows(b"/BaseFont /Symbol".len())
            .any(|window| window == b"/BaseFont /Symbol")
    );
    for forbidden in [
        b"fonttbl".as_slice(),
        b"u945",
        b"u946",
        b"u8804",
        b"u937",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden Unicode symbol content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn extended_unicode_math_symbols_use_passive_symbol_encoding() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Extended ",
        "\\",
        "u177?",
        "\\",
        "u215?",
        "\\",
        "u8734?",
        "\\",
        "u8706?",
        "\\",
        "u8711?",
        "\\",
        "u8712?",
        "\\",
        "u8745?",
        "\\",
        "u8746?",
        "\\",
        "u8834?",
        "\\",
        "u8835?",
        "\\",
        "u8853?",
        "\\",
        "u8855?",
        "\\",
        "u8596?",
        "\\",
        "u8658?",
        "\\",
        "u172?",
        "\\",
        "u8743?",
        "\\",
        "u8744? done",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains(
        "Extended \u{00b1}\u{00d7}\u{221e}\u{2202}\u{2207}\u{2208}\u{2229}\u{222a}\u{2282}\u{2283}\u{2295}\u{2297}\u{2194}\u{21d2}\u{00ac}\u{2227}\u{2228} done"
    ));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let symbol_bytes = pdf_text_bytes_for_font(&content, b"F13");
    for expected in [
        0xb1, 0xb4, 0xa5, 0xb6, 0xd1, 0xce, 0xc7, 0xc8, 0xcc, 0xc9, 0xc5, 0xc4, 0xab, 0xde, 0xd8,
        0xd9, 0xda,
    ] {
        assert!(
            symbol_bytes.contains(&expected),
            "extended Unicode math symbol was not encoded through passive Symbol byte {expected:#04x}; got {symbol_bytes:?}"
        );
    }
    assert!(
        !symbol_bytes.contains(&b'?'),
        "extended Unicode math symbols should not degrade to '?' in Symbol text operands"
    );
    for forbidden in [
        b"u8734".as_slice(),
        b"u8706",
        b"u8711",
        b"u8853",
        b"u8855",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden extended symbol content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn legacy_symbol_font_glyphs_use_passive_symbol_encoding_without_embedding_font_data() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Arial;}{",
        "\\",
        "f1",
        "\\",
        "fcharset2 Symbol;}}Symbol ",
        "\\",
        "f1 ",
        "\\",
        "'5c ",
        "\\",
        "'5e ",
        "\\",
        "'bd ",
        "\\",
        "'be ",
        "\\",
        "'d2 ",
        "\\",
        "'d3 ",
        "\\",
        "'d4 ",
        "\\",
        "'e6 ",
        "\\",
        "'e7 ",
        "\\",
        "'e8 ",
        "\\",
        "'e9 ",
        "\\",
        "'ea ",
        "\\",
        "'eb ",
        "\\",
        "'ec ",
        "\\",
        "'ed ",
        "\\",
        "'ee ",
        "\\",
        "'ef ",
        "\\",
        "'f0 ",
        "\\",
        "'f4 ",
        "\\",
        "'f5 ",
        "\\",
        "'f6 ",
        "\\",
        "'f7 ",
        "\\",
        "'f8 ",
        "\\",
        "'f9 ",
        "\\",
        "'fa ",
        "\\",
        "'fb ",
        "\\",
        "'fc ",
        "\\",
        "'fd ",
        "\\",
        "'fe ",
        "\\",
        "'ff",
        "\\",
        "f0 done",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    for expected in [
        '\u{2234}', '\u{22a5}', '\u{23d0}', '\u{23af}', '\u{00ae}', '\u{00a9}', '\u{2122}',
        '\u{239b}', '\u{239c}', '\u{239d}', '\u{23a1}', '\u{23a2}', '\u{23a3}', '\u{23a7}',
        '\u{23a8}', '\u{23a9}', '\u{23aa}', '\u{20ac}', '\u{23ae}', '\u{239e}', '\u{239f}',
        '\u{23a0}', '\u{23a4}', '\u{23a5}', '\u{23a6}', '\u{23ab}', '\u{23ac}', '\u{23ad}',
    ] {
        assert!(
            text.contains(expected),
            "legacy Symbol glyph {expected:?} was not normalized into safe text: {text:?}"
        );
    }
    for forbidden in ["fonttbl", "fcharset2", "'5c", "'ff"] {
        assert!(
            !text.contains(forbidden),
            "forbidden legacy Symbol source leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let symbol_bytes = pdf_text_bytes_for_font(&content, b"F13");
    for expected in [
        0x5c, 0x5e, 0xbd, 0xbe, 0xd2, 0xd3, 0xd4, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xeb, 0xec, 0xed,
        0xee, 0xef, 0xf0, 0xf3, 0xf4, 0xf5, 0xf7, 0xf8, 0xf9, 0xfa, 0xfb, 0xfc, 0xfd, 0xfe, 0xff,
    ] {
        assert!(
            symbol_bytes.contains(&expected),
            "legacy Symbol glyph was not encoded through passive Symbol byte {expected:#04x}; got {symbol_bytes:?}"
        );
    }
    assert!(
        !symbol_bytes.contains(&b'?'),
        "legacy Symbol glyphs should not degrade to '?' in Symbol text operands"
    );
    for forbidden in [
        b"fonttbl".as_slice(),
        b"fcharset2",
        b"fontemb",
        b"fontfile",
        b"/EmbeddedFile",
        b"/JavaScript",
        b"/Launch",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden legacy Symbol content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn font_family_hints_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0",
        "\\",
        "froman Mystery Serif;}{",
        "\\",
        "f1",
        "\\",
        "fmodern Mystery Mono;}}",
        "\\",
        "f0 Roman ",
        "\\",
        "f1 Modern",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Roman"));
    assert!(text.contains("Modern"));
    assert!(!text.contains("froman"));
    assert!(!text.contains("fmodern"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("font-family-hints.rtf");
    let output_path = dir.path().join("font-family-hints.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"fonttbl".as_slice(),
        b"froman",
        b"fmodern",
        b"Mystery Serif",
        b"Mystery Mono",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn narrow_font_aliases_render_as_passive_scaled_base14_without_font_payload_leakage() {
    let input =
        br"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Arial Narrow;}}\f1 Narrow visible text\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Narrow visible text"));
    assert_eq!(
        parsed
            .document
            .fonts
            .iter()
            .find(|font| font.index == 1)
            .map(|font| font.name.as_str()),
        Some("Arial Narrow")
    );
    for forbidden in ["fonttbl", "Arial Narrow"] {
        assert!(
            !text.contains(forbidden),
            "font metadata leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let font_names = pdf_text_font_names(&content);

    assert!(
        font_names.iter().any(|name| name.as_slice() == b"F1"),
        "narrow sans text should use passive Helvetica resource; got {font_names:?}"
    );
    assert!(content.operations.iter().any(|operation| {
        operation.operator == "Tz"
            && operation
                .operands
                .first()
                .and_then(pdf_operand_number)
                .is_some_and(|value| (value - 82.0).abs() < 0.01)
    }));
    for forbidden in [
        b"fonttbl".as_slice(),
        b"Arial Narrow",
        b"ArialNarrow",
        b"HelveticaNarrow",
        b"/FontFile",
        b"/FontFile2",
        b"/FontFile3",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden narrow font content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn theme_font_hints_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0",
        "\\",
        "flomajor Mystery Heading;}{",
        "\\",
        "f1",
        "\\",
        "fhiminor Mystery Body;}}",
        "\\",
        "f0 Major ",
        "\\",
        "f1 Minor",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let heading_font = parsed
        .document
        .fonts
        .iter()
        .find(|font| font.name == "Mystery Heading")
        .expect("theme heading font");
    let body_font = parsed
        .document
        .fonts
        .iter()
        .find(|font| font.name == "Mystery Body")
        .expect("theme body font");

    assert_eq!(heading_font.family, FontFamilyHint::Roman);
    assert_eq!(body_font.family, FontFamilyHint::Swiss);
    assert!(text.contains("Major Minor"));
    for forbidden in ["fonttbl", "flomajor", "fhiminor", "Mystery Heading"] {
        assert!(
            !text.contains(forbidden),
            "theme font metadata leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let font_names = pdf_text_font_names(&content);

    assert!(
        font_names.iter().any(|name| name.as_slice() == b"F9"),
        "major theme font should use passive Times substitution; font selections were {font_names:?}"
    );
    assert!(
        font_names.iter().any(|name| name.as_slice() == b"F1"),
        "minor theme font should use passive Helvetica substitution; font selections were {font_names:?}"
    );
    for forbidden in [
        b"fonttbl".as_slice(),
        b"flomajor",
        b"fhiminor",
        b"Mystery Heading",
        b"Mystery Body",
        b"fontemb",
        b"fontfile",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden theme font content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_font_names_substitute_to_passive_base14_without_font_payload() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Calibri;}{",
        "\\",
        "f1 Cambria;}{",
        "\\",
        "f2 Aptos Mono;}{",
        "\\",
        "f3 MS Sans Serif;}{",
        "\\",
        "f4 MS Serif;}{",
        "\\",
        "f5 Wingdings;}{",
        "\\",
        "f6 Times New Roman;}{",
        "\\",
        "f7",
        "\\",
        "fcharset238 Times New Roman CE;}{",
        "\\",
        "f8 Arial;}{",
        "\\",
        "f9 Courier New;}}",
        "\\",
        "f0 Sans ",
        "\\",
        "f1 Serif ",
        "\\",
        "f2 Mono ",
        "\\",
        "f3 LegacySans ",
        "\\",
        "f4 LegacySerif ",
        "\\",
        "f5 Wing",
        "\\",
        "f6 DirectTimes ",
        "\\",
        "f7 CharsetTimes ",
        "\\",
        "f8 DirectSans ",
        "\\",
        "f9 DirectCourier",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Sans"));
    assert!(text.contains("Serif"));
    assert!(text.contains("Mono"));
    assert!(text.contains("LegacySans"));
    assert!(text.contains("LegacySerif"));
    assert!(text.contains("Wing"));
    assert!(text.contains("DirectTimes"));
    assert!(text.contains("CharsetTimes"));
    assert!(text.contains("DirectSans"));
    assert!(text.contains("DirectCourier"));
    assert!(!text.contains("fonttbl"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("office-font-substitution.rtf");
    let output_path = dir.path().join("office-font-substitution.pdf");
    fs::write(&input_path, input).unwrap();
    let report = convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    for expected in [
        "font 'Calibri' substituted with passive PDF base font Helvetica",
        "font 'Cambria' substituted with passive PDF base font Times-Roman",
        "font 'Aptos Mono' substituted with passive PDF base font Courier",
    ] {
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "missing font substitution diagnostic {expected:?}; diagnostics were {:?}",
            report.diagnostics
        );
    }
    for expected in [
        "font 'MS Sans Serif' approximated with passive PDF base font Helvetica",
        "font 'MS Serif' approximated with passive PDF base font Times-Roman",
        "font 'Wingdings' approximated with passive PDF base font ZapfDingbats",
        "font 'Times New Roman' approximated with passive PDF base font Times-Roman",
        "font 'Times New Roman CE' approximated with passive PDF base font Times-Roman",
        "font 'Arial' approximated with passive PDF base font Helvetica",
        "font 'Courier New' approximated with passive PDF base font Courier",
    ] {
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "missing font approximation diagnostic {expected:?}; diagnostics were {:?}",
            report.diagnostics
        );
    }
    for unexpected in [
        "font 'MS Sans Serif' substituted",
        "font 'MS Serif' substituted",
        "font 'Wingdings' substituted",
        "font 'Times New Roman' substituted",
        "font 'Times New Roman CE' substituted",
        "font 'Arial' substituted",
        "font 'Courier New' substituted",
    ] {
        assert!(
            report
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains(unexpected)),
            "known passive font alias should not emit substitution diagnostic {unexpected:?}; diagnostics were {:?}",
            report.diagnostics
        );
    }
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let font_names = pdf_text_font_names(&content);

    assert!(
        font_names.iter().any(|name| name.as_slice() == b"F1"),
        "Calibri should use passive Helvetica substitution; font selections were {font_names:?}"
    );
    assert!(
        font_names.iter().any(|name| name.as_slice() == b"F9"),
        "Cambria should use passive Times substitution; font selections were {font_names:?}"
    );
    assert!(
        font_names.iter().any(|name| name.as_slice() == b"F5"),
        "Aptos Mono should use passive Courier substitution; font selections were {font_names:?}"
    );
    for forbidden in [
        b"fonttbl".as_slice(),
        b"Calibri",
        b"Cambria",
        b"Aptos Mono",
        b"MS Sans Serif",
        b"MS Serif",
        b"Wingdings",
        b"Times New Roman",
        b"Arial",
        b"Courier New",
        b"fontemb",
        b"fontfile",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden font substitution content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn font_pitch_hints_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0",
        "\\",
        "fnil",
        "\\",
        "fprq1 Mystery Fixed;}{",
        "\\",
        "f1",
        "\\",
        "fnil",
        "\\",
        "fprq2 Mystery Variable;}}",
        "\\",
        "f0 Fixed ",
        "\\",
        "f1 Variable",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let fixed_font = parsed
        .document
        .fonts
        .iter()
        .find(|font| font.name == "Mystery Fixed")
        .expect("fixed-pitch font");
    let variable_font = parsed
        .document
        .fonts
        .iter()
        .find(|font| font.name == "Mystery Variable")
        .expect("variable-pitch font");

    assert_eq!(fixed_font.pitch, FontPitch::Fixed);
    assert_eq!(variable_font.pitch, FontPitch::Variable);
    assert!(text.contains("Fixed"));
    assert!(text.contains("Variable"));
    assert!(!text.contains("fprq"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("font-pitch-hints.rtf");
    let output_path = dir.path().join("font-pitch-hints.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(decoded_pdf_text(&content).contains("Fixed"));
    assert!(content.operations.iter().any(|operation| {
        operation.operator == "Tf" && format!("{:?}", operation.operands).contains("/F5")
    }));
    for forbidden in [
        b"fprq".as_slice(),
        b"Mystery Fixed",
        b"Mystery Variable",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn font_alternate_name_guides_passive_fallback_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0",
        "\\",
        "fnil Mystery Sans{",
        "\\",
        "*",
        "\\",
        "falt Courier New;};}}",
        "\\",
        "f0 Fallback text",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let font = parsed
        .document
        .fonts
        .iter()
        .find(|font| font.name == "Mystery Sans")
        .expect("font");

    assert_eq!(font.alternate_name.as_deref(), Some("Courier New"));
    assert!(text.contains("Fallback text"));
    for forbidden in ["fonttbl", "falt", "Courier New"] {
        assert!(
            !text.contains(forbidden),
            "font alternate metadata leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("font-alternate.rtf");
    let output_path = dir.path().join("font-alternate.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    assert!(
        pdf.windows(b"/BaseFont /Courier".len())
            .any(|window| window == b"/BaseFont /Courier")
    );
    for forbidden in [
        b"fonttbl".as_slice(),
        b"falt",
        b"Mystery Sans",
        b"Courier New",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn embedded_font_destinations_are_stripped_before_pdf_rendering() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0",
        "\\",
        "fswiss Arial{",
        "\\",
        "fontemb{",
        "\\",
        "fontfile HOSTILE-FONT-PAYLOAD {",
        "\\",
        "object",
        "\\",
        "objdata ",
        payload_hex(),
        "}}};}}Visible body",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible body"));
    assert!(
        parsed
            .document
            .fonts
            .iter()
            .any(|font| font.name == "Arial"),
        "expected visible font name to survive embedded-font stripping"
    );
    for forbidden in [
        "HOSTILE-FONT-PAYLOAD",
        "fontemb",
        "fontfile",
        "object",
        "objdata",
        payload_hex(),
    ] {
        assert!(
            !text.contains(forbidden),
            "embedded font payload leaked into text: {forbidden}"
        );
        assert!(
            parsed
                .document
                .fonts
                .iter()
                .all(|font| !font.name.contains(forbidden)),
            "embedded font payload leaked into font name: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("embedded-font.rtf");
    let output_path = dir.path().join("embedded-font.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible body"));
    for forbidden in [
        b"HOSTILE-FONT-PAYLOAD".as_slice(),
        b"fontemb",
        b"fontfile",
        b"objdata",
        payload_hex().as_bytes(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "embedded font payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn embedded_font_destinations_obey_reject_policy() {
    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };

    assert!(matches!(
        parse_rtf_bytes_with_options(
            &rtf(&[
                "{",
                "\\",
                "rtf1{",
                "\\",
                "fonttbl{",
                "\\",
                "f0 Arial{",
                "\\",
                "fontemb{",
                "\\",
                "fontfile HOSTILE-FONT-PAYLOAD}};}}Visible",
                "\\",
                "par}"
            ]),
            &reject_options,
        ),
        Err(ParseError::ActiveContentRejected { feature, .. }) if feature == "embedded font payload"
    ));
}

#[test]
fn default_font_control_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "deff1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0",
        "\\",
        "fswiss Arial;}{",
        "\\",
        "f1",
        "\\",
        "froman Times New Roman;}}",
        "\\",
        "plain Default ",
        "\\",
        "f0 Sans ",
        "\\",
        "plain Back",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let paragraph = match &parsed.document.blocks[0] {
        open_rtf_converter::model::Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    let font_for = |text: &str| {
        paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == text)
            .map(|run| run.style.font_index)
            .unwrap_or_else(|| panic!("missing run {text}"))
    };
    let text = collect_text(&parsed.document);

    assert_eq!(font_for("Default"), 1);
    assert_eq!(font_for("Sans"), 0);
    assert_eq!(font_for("Back"), 1);
    assert!(text.contains("Default"));
    assert!(text.contains("Sans"));
    assert!(text.contains("Back"));
    assert!(!text.contains("deff"));
    assert!(!text.contains("fonttbl"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("default-font.rtf");
    let output_path = dir.path().join("default-font.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(content.operations.iter().any(|operation| {
        operation.operator == "Tf" && format!("{:?}", operation.operands).contains("/F9")
    }));
    for forbidden in [
        b"deff".as_slice(),
        b"fonttbl",
        b"Times New Roman",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn stylesheet_inheritance_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "stylesheet{",
        "\\",
        "s2",
        "\\",
        "sbasedon1",
        "\\",
        "i Child;}{",
        "\\",
        "s1",
        "\\",
        "qc",
        "\\",
        "b Base;}}",
        "\\",
        "s2 Styled",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Styled"));
    assert!(!text.contains("Child"));
    assert!(!text.contains("sbasedon"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("stylesheet-inheritance.rtf");
    let output_path = dir.path().join("stylesheet-inheritance.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"stylesheet".as_slice(),
        b"sbasedon",
        b"Child",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn missing_word_normal_style_zero_renders_without_control_leakage() {
    let input = br"{\rtf1{\stylesheet{\s1\b Heading;}}\s1 Bold\par\s0 Normal\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Bold"));
    assert!(text.contains("Normal"));
    for forbidden in ["stylesheet", "Heading", "s0", "s1"] {
        assert!(
            !text.contains(forbidden),
            "forbidden style metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unknown RTF style index 0")),
        "missing Normal style 0 should not warn: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.contains("Bold"),
        "decoded PDF text missing styled text: {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("Normal"),
        "decoded PDF text missing Normal style text: {rendered_text:?}"
    );
    for forbidden in [
        b"stylesheet".as_slice(),
        b"Heading",
        b"s0",
        b"s1",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden missing-style content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn stylesheet_next_style_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "stylesheet{",
        "\\",
        "s1",
        "\\",
        "snext2",
        "\\",
        "b Heading;}{",
        "\\",
        "s2",
        "\\",
        "qc",
        "\\",
        "i Body;}}",
        "\\",
        "s1 Heading text",
        "\\",
        "par Body text",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Heading text"));
    assert!(text.contains("Body text"));
    assert!(!text.contains("snext"));
    assert!(!text.contains("Body;"));

    let first = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    let second = match &parsed.document.blocks[1] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    assert!(first.runs[0].style.bold);
    assert!(!first.runs[0].style.italic);
    assert_eq!(second.style.alignment, Alignment::Center);
    assert!(!second.runs[0].style.bold);
    assert!(second.runs[0].style.italic);

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("stylesheet-next-style.rtf");
    let output_path = dir.path().join("stylesheet-next-style.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Heading text"));
    assert!(rendered_text.contains("Body text"));
    for forbidden in [
        b"stylesheet".as_slice(),
        b"snext",
        b"Heading;",
        b"Body;",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn stylesheet_character_style_renders_passively_without_paragraph_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "stylesheet{",
        "\\",
        "cs5",
        "\\",
        "qc",
        "\\",
        "li1440",
        "\\",
        "b Emphasis;}}",
        "\\",
        "qr Right ",
        "\\",
        "cs5 Bold only",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Right Bold only"));
    assert!(!text.contains("Emphasis"));
    assert!(!text.contains("li1440"));

    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    assert_eq!(paragraph.style.alignment, Alignment::Right);
    assert_eq!(paragraph.style.left_indent_twips, 0);
    assert!(!paragraph.runs[0].style.bold);
    assert!(paragraph.runs[1].style.bold);

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("stylesheet-character-style.rtf");
    let output_path = dir.path().join("stylesheet-character-style.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Right Bold only"));
    for forbidden in [
        b"stylesheet".as_slice(),
        b"cs5",
        b"li1440",
        b"Emphasis",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn stylesheet_character_style_preserves_direct_formatting_without_metadata_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Arial;}{",
        "\\",
        "f1 Courier New;}}{",
        "\\",
        "stylesheet{",
        "\\",
        "cs5",
        "\\",
        "b Emphasis;}}",
        "\\",
        "f1",
        "\\",
        "i Direct ",
        "\\",
        "cs5 Direct and styled",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Direct Direct and styled"));
    assert!(!text.contains("Emphasis"));
    assert!(!text.contains("cs5"));

    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    assert_eq!(paragraph.runs[0].style.font_index, 1);
    assert!(paragraph.runs[0].style.italic);
    assert!(!paragraph.runs[0].style.bold);
    assert_eq!(paragraph.runs[1].style.font_index, 1);
    assert!(paragraph.runs[1].style.italic);
    assert!(paragraph.runs[1].style.bold);

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("stylesheet-character-style-additive.rtf");
    let output_path = dir.path().join("stylesheet-character-style-additive.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Direct Direct and styled"));
    for forbidden in [
        b"stylesheet".as_slice(),
        b"cs5",
        b"Emphasis",
        b"fonttbl",
        b"Courier New",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn word_stylesheet_metadata_controls_do_not_warn_or_leak_to_pdf() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Calibri;}}{",
        "\\",
        "stylesheet{",
        "\\",
        "s0",
        "\\",
        "ql",
        "\\",
        "widctlpar",
        "\\",
        "snext0",
        "\\",
        "ssemihidden",
        "\\",
        "sqformat",
        "\\",
        "spriority0 Normal;}{",
        "\\",
        "s1",
        "\\",
        "sbasedon0",
        "\\",
        "slink15",
        "\\",
        "slocked",
        "\\",
        "sautoupd",
        "\\",
        "styrsid123",
        "\\",
        "b",
        "\\",
        "qc Heading 1;}{",
        "\\",
        "cs15",
        "\\",
        "sadditive",
        "\\",
        "ssemihidden",
        "\\",
        "i Linked Char;}{",
        "\\",
        "*",
        "\\",
        "latentstyles",
        "\\",
        "lsdstimax376",
        "\\",
        "lsdlockeddef0",
        "\\",
        "lsdsemihiddendef1{",
        "\\",
        "lsdlockedexcept Normal;}}}",
        "\\",
        "pard",
        "\\",
        "s1 Heading",
        "\\",
        "par ",
        "\\",
        "cs15 linked text",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Heading"));
    assert!(text.contains("linked text"));
    assert!(!text.contains("Heading 1"));
    assert!(!text.contains("Linked Char"));
    for forbidden in [
        "ssemihidden",
        "sqformat",
        "spriority",
        "slink",
        "slocked",
        "sautoupd",
        "styrsid",
        "latentstyles",
        "lsdstimax",
    ] {
        assert!(
            !text.contains(forbidden),
            "stylesheet metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "Word stylesheet metadata should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );
    let heading = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected heading paragraph"),
    };
    assert_eq!(heading.style.alignment, Alignment::Center);
    assert!(heading.runs[0].style.bold);
    let linked = match &parsed.document.blocks[1] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected linked character-style paragraph"),
    };
    assert!(linked.runs[0].style.italic);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Heading"));
    assert!(rendered_text.contains("linked text"));
    for forbidden in [
        b"stylesheet".as_slice(),
        b"ssemihidden",
        b"sqformat",
        b"spriority",
        b"slink",
        b"slocked",
        b"sautoupd",
        b"styrsid",
        b"latentstyles",
        b"lsdstimax",
        b"Heading 1",
        b"Linked Char",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "stylesheet metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn latent_styles_metadata_does_not_warn_or_leak_payloads() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "stylesheet{",
        "\\",
        "s0 Normal;}}{",
        "\\",
        "latentstyles",
        "\\",
        "lsdstimax376",
        "\\",
        "lsdlockeddef0",
        "\\",
        "lsdpriority99",
        "\\",
        "lsdqformat1",
        "\\",
        "lsdunhideused1{",
        "\\",
        "lsdsemihiddendef1 Hidden latent style metadata}{",
        "\\",
        "object",
        "\\",
        "objdata 414243}}Visible",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible"));
    for forbidden in [
        "latentstyles",
        "lsdstimax",
        "lsdlockeddef",
        "lsdpriority",
        "lsdqformat",
        "lsdunhideused",
        "lsdsemihiddendef",
        "Hidden latent style metadata",
        "objdata",
        "414243",
    ] {
        assert!(
            !text.contains(forbidden),
            "latent styles metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| {
            !diagnostic.message.contains("unsupported RTF control")
                && !diagnostic.message.contains("unknown RTF destination")
        }),
        "latent styles metadata should be classified, got {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible"));
    for forbidden in [
        b"latentstyles".as_slice(),
        b"lsdstimax",
        b"lsdlockeddef",
        b"lsdpriority",
        b"lsdqformat",
        b"lsdunhideused",
        b"lsdsemihiddendef",
        b"Hidden latent style metadata",
        b"objdata",
        b"414243",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "latent styles metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_text_renders_passively_without_math_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mr{",
        "\\",
        "mtext E=mc}{",
        "\\",
        "msup{",
        "\\",
        "me{",
        "\\",
        "mtext 2}}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Before E=mc2 After"));
    let superscript_style = run_style_for_text(&parsed.document, "2").expect("superscript run");
    assert!(superscript_style.baseline_shift_half_points > 0);
    assert!(superscript_style.font_size_scale_percent < 100);
    for forbidden in ["mmath", "moMath", "mtext", "msup", "mctrlPr"] {
        assert!(
            !text.contains(forbidden),
            "Office math control leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "Office math controls should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("Office math layout approximated as passive text")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before E=mc2 After"));
    let base_position = pdf_first_text_position_for_text(&content, "E=mc").expect("base text");
    let script_position = pdf_first_text_position_for_text(&content, "2").expect("script text");
    assert!(
        script_position.1 > base_position.1,
        "Office math superscript should render above the base text: base={base_position:?}, script={script_position:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mtext",
        b"msup",
        b"mctrlPr",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_fractions_render_readable_passive_text() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mf{",
        "\\",
        "mnum{",
        "\\",
        "mtext x+1}}{",
        "\\",
        "mden{",
        "\\",
        "mtext y}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Before x+1\u{2044}y After"));
    let numerator_style = run_style_for_text(&parsed.document, "x+1").expect("fraction numerator");
    assert!(numerator_style.baseline_shift_half_points > 0);
    assert!(numerator_style.font_size_scale_percent < 100);
    let denominator_style =
        run_style_for_text(&parsed.document, "y").expect("fraction denominator");
    assert!(denominator_style.baseline_shift_half_points < 0);
    assert!(denominator_style.font_size_scale_percent < 100);
    for forbidden in ["mmath", "moMath", "mf", "mnum", "mden", "mtext"] {
        assert!(
            !text.contains(forbidden),
            "Office math fraction control leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before x+1"));
    assert!(rendered_text.contains("y After"));
    let numerator_position =
        pdf_first_text_position_for_text(&content, "x+1").expect("fraction numerator position");
    let denominator_position =
        pdf_first_text_position_for_text(&content, "y").expect("fraction denominator position");
    assert!(
        numerator_position.1 > denominator_position.1,
        "Office math fraction numerator should render above denominator: numerator={numerator_position:?}, denominator={denominator_position:?}"
    );
    let symbol_bytes = pdf_text_bytes_for_font(&content, b"F13");
    assert!(
        symbol_bytes.contains(&0xa4),
        "Office math fraction slash should encode through passive Symbol byte 0xa4; got {symbol_bytes:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mnum",
        b"mden",
        b"mtext",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math fraction control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_fraction_types_are_bounded_passive_metadata() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mf{",
        "\\",
        "mfPr{",
        "launch.exe objdata 414243 ",
        "\\",
        "u65?",
        "\\",
        "'42",
        "\\",
        "mtype noBar}{",
        "\\",
        "*",
        "\\",
        "unknown{",
        "\\",
        "object",
        "\\",
        "objdata 414243}}}{",
        "\\",
        "mnum{",
        "\\",
        "mtext A}}{",
        "\\",
        "mden{",
        "\\",
        "mtext B}}}}} and {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mf{",
        "\\",
        "mfPr{",
        "https://example.com/payload objdata 444546 ",
        "\\",
        "u67?",
        "\\",
        "'44",
        "\\",
        "mtype calc.exe}}{",
        "\\",
        "mnum{",
        "\\",
        "mtext C}}{",
        "\\",
        "mden{",
        "\\",
        "mtext D}}}}} and {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mf{",
        "\\",
        "mfPr{",
        "calc.exe objdata 454647 ",
        "\\",
        "u72?",
        "\\",
        "'49",
        "\\",
        "mtype lin}}{",
        "\\",
        "mnum{",
        "\\",
        "mtext E}}{",
        "\\",
        "mden{",
        "\\",
        "mtext F}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before AB and C\u{2044}D and E\u{2044}F After"),
        "unexpected fraction type text: {text:?}"
    );
    let no_bar_denominator_style =
        run_style_for_text(&parsed.document, "B").expect("no-bar denominator");
    assert!(no_bar_denominator_style.baseline_shift_half_points < 0);
    assert!(no_bar_denominator_style.font_size_scale_percent < 100);
    for forbidden in [
        "mmath",
        "moMath",
        "mfPr",
        "mtype",
        "mtext",
        "noBar",
        "calc.exe",
        "launch.exe",
        "example.com/payload",
        "objdata",
        "414243",
        "444546",
        "454647",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math fraction type metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math fraction type controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    for visible in ["Before AB", "C", "D", "E", "F After"] {
        assert!(
            rendered_text.contains(visible),
            "fraction type visible text missing from PDF text: {visible}; got {rendered_text:?}"
        );
    }
    let symbol_bytes = pdf_text_bytes_for_font(&content, b"F13");
    assert!(
        symbol_bytes.contains(&0xa4),
        "bar/linear fraction slash should encode through passive Symbol byte 0xa4; got {symbol_bytes:?}"
    );
    let e_position = pdf_first_text_position_for_text(&content, "E").expect("linear numerator");
    let f_position = pdf_first_text_position_for_text(&content, "F").expect("linear denominator");
    assert!(
        (e_position.1 - f_position.1).abs() < 0.5,
        "linear Office math fraction should render numerator and denominator on one baseline: E={e_position:?}, F={f_position:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mfPr",
        b"mtype",
        b"mtext",
        b"noBar",
        b"calc.exe",
        b"launch.exe",
        b"example.com/payload",
        b"objdata",
        b"414243",
        b"444546",
        b"454647",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math fraction type metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_subscripts_render_readable_passive_text() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "msSub{",
        "\\",
        "me{",
        "\\",
        "mtext x}}{",
        "\\",
        "msub{",
        "\\",
        "mtext i}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Before xi After"));
    let subscript_style = run_style_for_text(&parsed.document, "i").expect("subscript run");
    assert!(subscript_style.baseline_shift_half_points < 0);
    assert!(subscript_style.font_size_scale_percent < 100);
    for forbidden in ["mmath", "moMath", "msSub", "msub", "mtext"] {
        assert!(
            !text.contains(forbidden),
            "Office math subscript control leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before xi After"));
    let base_position = pdf_first_text_position_for_text(&content, "x").expect("base text");
    let script_position = pdf_first_text_position_for_text(&content, "i").expect("script text");
    assert!(
        script_position.1 < base_position.1,
        "Office math subscript should render below the base text: base={base_position:?}, script={script_position:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"msSub",
        b"msub",
        b"mtext",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math subscript control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_radicals_render_readable_passive_text() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mrad{",
        "\\",
        "me{",
        "\\",
        "mtext x+1}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Before \u{221a}x+1 After"));
    let radicand_style = run_style_for_text(&parsed.document, "x+1").expect("radicand run");
    assert!(radicand_style.overline);
    for forbidden in ["mmath", "moMath", "mrad", "me", "mtext"] {
        assert!(
            !text.contains(forbidden),
            "Office math radical control leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before "));
    assert!(rendered_text.contains("x+1 After"));
    let font_names = pdf_text_font_names(&content);
    assert!(
        font_names.iter().any(|name| name.as_slice() == b"F13"),
        "square-root marker should use passive Symbol substitution; font selections were {font_names:?}"
    );
    let symbol_bytes = pdf_text_bytes_for_font(&content, b"F13");
    assert!(
        symbol_bytes.contains(&0xd6),
        "square-root marker should encode as Symbol radical byte 0xd6; got {symbol_bytes:?}"
    );
    assert!(
        content.operations.windows(3).any(|operations| {
            operations[0].operator == "m"
                && operations[1].operator == "l"
                && operations[2].operator == "S"
        }),
        "Office math radicand should render with a passive overbar stroke"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mrad",
        b"mtext",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math radical control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_radical_degrees_render_or_hide_passively() {
    let visible_input = br"{\rtf1 Before {\mmath{\moMath{\mrad{\mradPr calc.exe objdata 414243 \u65?\'42{\mdegHide0}}{\mdeg{\mtext 3}}{\me{\mtext x}}}}} After\par}".to_vec();
    let visible_parsed = parse_rtf_bytes(&visible_input).unwrap();
    let visible_text = collect_text(&visible_parsed.document);
    assert!(
        visible_text.contains("Before \u{221a}3x After"),
        "unexpected visible radical degree text: {visible_text:?}"
    );
    let visible_degree_style =
        run_style_for_text(&visible_parsed.document, "3").expect("visible radical degree");
    assert!(visible_degree_style.baseline_shift_half_points > 0);
    assert!(visible_degree_style.font_size_scale_percent < 100);
    let visible_radicand_style =
        run_style_for_text(&visible_parsed.document, "x").expect("visible radicand");
    assert!(visible_radicand_style.overline);
    for forbidden in ["calc.exe", "objdata", "414243", "AB"] {
        assert!(
            !visible_text.contains(forbidden),
            "Office math visible radical property payload leaked to text: {forbidden}"
        );
    }

    let hidden_input = br"{\rtf1 Before {\mmath{\moMath{\mrad{\mradPr launch.exe https://example.com/payload objdata 444546 \u67?\'44{\mdegHide1}}{\mdeg{\mtext HIDDEN-DEGREE-PAYLOAD}}{\me{\mtext x}}}}} After\par}".to_vec();
    let hidden_parsed = parse_rtf_bytes(&hidden_input).unwrap();
    let hidden_text = collect_text(&hidden_parsed.document);
    assert!(
        hidden_text.contains("Before \u{221a}x After"),
        "unexpected hidden radical degree text: {hidden_text:?}"
    );
    for forbidden in [
        "mmath",
        "moMath",
        "mrad",
        "mradPr",
        "mdegHide",
        "mdeg",
        "mtext",
        "launch.exe",
        "example.com/payload",
        "objdata",
        "444546",
        "HIDDEN-DEGREE-PAYLOAD",
    ] {
        assert!(
            !hidden_text.contains(forbidden),
            "Office math radical degree content leaked to text: {forbidden}"
        );
    }
    assert!(
        hidden_parsed
            .diagnostics
            .iter()
            .all(
                |diagnostic| !diagnostic.message.contains("unknown RTF destination")
                    && !diagnostic.message.contains("unsupported RTF control")
            ),
        "Office math radical degree controls should not be reported as unknown or unsupported: {:?}",
        hidden_parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &hidden_input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before "));
    assert!(rendered_text.contains("x After"));
    let symbol_bytes = pdf_text_bytes_for_font(&content, b"F13");
    assert!(
        symbol_bytes.contains(&0xd6),
        "hidden-degree radical should still encode the radical marker through passive Symbol byte 0xd6; got {symbol_bytes:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mrad",
        b"mradPr",
        b"mdegHide",
        b"mdeg",
        b"mtext",
        b"launch.exe",
        b"example.com/payload",
        b"objdata",
        b"444546",
        b"HIDDEN-DEGREE-PAYLOAD",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math radical degree content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_overbars_render_readable_passive_text() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mbar{",
        "\\",
        "me{",
        "\\",
        "mtext x}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Before x After"));
    let overbar_style = run_style_for_text(&parsed.document, "x").expect("overbar run");
    assert!(overbar_style.overline);
    for forbidden in ["mmath", "moMath", "mbar", "me", "mtext"] {
        assert!(
            !text.contains(forbidden),
            "Office math overbar control leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before x After"));
    let helvetica_bytes = pdf_text_bytes_for_font(&content, b"F1");
    assert!(
        !helvetica_bytes.contains(&0xaf),
        "overbar should render as passive geometry, not as a macron glyph; got {helvetica_bytes:?}"
    );
    assert!(
        content.operations.windows(3).any(|operations| {
            operations[0].operator == "m"
                && operations[1].operator == "l"
                && operations[2].operator == "S"
        }),
        "Office math overbar should render as a passive stroke"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mbar",
        b"mtext",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math overbar control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_bar_positions_render_passively_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mbar{",
        "\\",
        "mbarPr{",
        "calc.exe objdata 414243 ",
        "\\",
        "u65?",
        "\\",
        "'42",
        "\\",
        "mpos bot}{",
        "\\",
        "*",
        "\\",
        "unknown{",
        "\\",
        "object",
        "\\",
        "objdata 414243}}}{",
        "\\",
        "me{",
        "\\",
        "mtext UnderBar}}}}} and {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mbar{",
        "\\",
        "mbarPr{",
        "launch.exe https://example.com/payload objdata 444546 ",
        "\\",
        "u67?",
        "\\",
        "'44",
        "\\",
        "mpos top}}{",
        "\\",
        "me{",
        "\\",
        "mtext OverBar}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before UnderBar and OverBar After"),
        "unexpected bar-position math text: {text:?}"
    );
    let under_style = run_style_for_text(&parsed.document, "UnderBar").expect("underbar run");
    assert!(!under_style.overline);
    assert_eq!(under_style.underline, UnderlineStyle::Single);
    let over_style = run_style_for_text(&parsed.document, "OverBar").expect("overbar run");
    assert!(over_style.overline);
    assert_eq!(over_style.underline, UnderlineStyle::None);
    for forbidden in [
        "mmath",
        "moMath",
        "mbar",
        "mbarPr",
        "mpos",
        "bot",
        "top",
        "mtext",
        "calc.exe",
        "launch.exe",
        "example.com/payload",
        "objdata",
        "414243",
        "444546",
        "AB",
        "CD",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math bar-position metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math bar-position controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before UnderBar and OverBar After"));
    let passive_strokes = content
        .operations
        .windows(3)
        .filter(|operations| {
            operations[0].operator == "m"
                && operations[1].operator == "l"
                && operations[2].operator == "S"
        })
        .count();
    assert!(
        passive_strokes >= 2,
        "Office math top/bottom bars should render as passive strokes, got {passive_strokes}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mbar",
        b"mbarPr",
        b"mpos",
        b"calc.exe",
        b"launch.exe",
        b"example.com/payload",
        b"objdata",
        b"414243",
        b"444546",
        b"AB",
        b"CD",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math bar-position metadata or payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_nary_operators_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mnary{",
        "\\",
        "mnaryPr{",
        "\\",
        "mchr ",
        "\\",
        "u8721?}{",
        "\\",
        "msubHide0}{",
        "\\",
        "msupHide0}}{",
        "\\",
        "msub{",
        "\\",
        "mtext i=1}}{",
        "\\",
        "msup{",
        "\\",
        "mtext n}}{",
        "\\",
        "me{",
        "\\",
        "mtext i}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before \u{2211}i=1ni After"),
        "unexpected n-ary math text: {text:?}"
    );
    let lower_limit_style =
        run_style_for_text(&parsed.document, "i=1").expect("lower n-ary limit run");
    assert!(lower_limit_style.baseline_shift_half_points < 0);
    assert!(lower_limit_style.font_size_scale_percent < 100);
    let upper_limit_style =
        run_style_for_text(&parsed.document, "n").expect("upper n-ary limit run");
    assert!(upper_limit_style.baseline_shift_half_points > 0);
    assert!(upper_limit_style.font_size_scale_percent < 100);
    for forbidden in [
        "mmath", "moMath", "mnary", "mnaryPr", "mchr", "msubHide", "msupHide", "mtext",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math n-ary control leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "Office math n-ary controls should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before "));
    assert!(rendered_text.contains("i=1"));
    assert!(rendered_text.contains("ni After"));
    let symbol_bytes = pdf_text_bytes_for_font(&content, b"F13");
    assert!(
        symbol_bytes.contains(&0xe5),
        "summation operator should encode through passive Symbol byte 0xe5; got {symbol_bytes:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mnary",
        b"mnaryPr",
        b"mchr",
        b"msubHide",
        b"msupHide",
        b"mtext",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math n-ary control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_nary_limit_locations_are_bounded_passive_metadata() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mnary{",
        "\\",
        "mnaryPr{",
        "\\",
        "mchr ",
        "\\",
        "u8721?}{",
        "\\",
        "mlimLoc undOvr}{",
        "\\",
        "*",
        "\\",
        "unknown{",
        "\\",
        "object",
        "\\",
        "objdata 414243}}}{",
        "\\",
        "msub{",
        "\\",
        "mtext lowA}}{",
        "\\",
        "msup{",
        "\\",
        "mtext upA}}{",
        "\\",
        "me{",
        "\\",
        "mtext bodyA}}}}} and {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mnary{",
        "\\",
        "mnaryPr{",
        "\\",
        "mchr ",
        "\\",
        "u8721?}{",
        "\\",
        "mlimLoc calc.exe}}{",
        "\\",
        "msub{",
        "\\",
        "mtext lowB}}{",
        "\\",
        "msup{",
        "\\",
        "mtext upB}}{",
        "\\",
        "me{",
        "\\",
        "mtext bodyB}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before \u{2211}lowAupAbodyA and \u{2211}lowBupBbodyB After"),
        "unexpected n-ary limit-location text: {text:?}"
    );
    let under_over_lower =
        run_style_for_text(&parsed.document, "lowA").expect("under-over lower limit");
    let default_lower = run_style_for_text(&parsed.document, "lowB").expect("default lower limit");
    assert!(
        under_over_lower.baseline_shift_half_points < default_lower.baseline_shift_half_points,
        "under-over lower limit should use a stronger passive downward shift"
    );
    let under_over_upper =
        run_style_for_text(&parsed.document, "upA").expect("under-over upper limit");
    let default_upper = run_style_for_text(&parsed.document, "upB").expect("default upper limit");
    assert!(
        under_over_upper.baseline_shift_half_points > default_upper.baseline_shift_half_points,
        "under-over upper limit should use a stronger passive upward shift"
    );
    for forbidden in [
        "mmath", "moMath", "mnary", "mnaryPr", "mchr", "mlimLoc", "undOvr", "calc.exe", "objdata",
        "414243", "mtext",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math n-ary limit-location metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math n-ary limit-location controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    for visible in ["Before ", "lowA", "upAbodyA and ", "lowB", "upBbodyB After"] {
        assert!(
            rendered_text.contains(visible),
            "n-ary limit-location visible text missing from PDF text: {visible}; got {rendered_text:?}"
        );
    }
    let under_over_base = pdf_first_text_position_for_text(&content, "bodyA").expect("base bodyA");
    let under_over_lower_pos =
        pdf_first_text_position_for_text(&content, "lowA").expect("under-over lower position");
    let default_base = pdf_first_text_position_for_text(&content, "bodyB").expect("base bodyB");
    let default_lower_pos =
        pdf_first_text_position_for_text(&content, "lowB").expect("default lower position");
    assert!(
        (under_over_base.1 - under_over_lower_pos.1) > (default_base.1 - default_lower_pos.1),
        "under-over lower limit should render farther below its base than default sub/sup mode"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mnary",
        b"mnaryPr",
        b"mchr",
        b"mlimLoc",
        b"undOvr",
        b"calc.exe",
        b"objdata",
        b"414243",
        b"mtext",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math n-ary limit-location metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_style_metadata_is_bounded_and_passive() {
    let input = br"{\rtf1 Before {\mmath{\moMath{\mtext{\msty bi}Styled}}} and {\mmath{\moMath{\mtext{{\mctrlPr calc.exe objdata 444546 \u67?\'44{\*\unknown{\object\objdata 454647}}{\msty i}}Wrapped}}}} and {\mmath{\moMath{\mtext{\msty calc.exe{\*\unknown{\object\objdata 414243}}}Plain}}} After\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before Styled and Wrapped and Plain After"),
        "unexpected Office math style text: {text:?}"
    );
    let styled = run_style_for_text(&parsed.document, "Styled").expect("styled math run");
    assert!(styled.bold);
    assert!(styled.italic);
    let wrapped = run_style_for_text(&parsed.document, "Wrapped").expect("wrapped math run");
    assert!(!wrapped.bold);
    assert!(wrapped.italic);
    let plain = run_style_for_text(&parsed.document, " and Plain After").expect("plain math run");
    assert!(!plain.bold);
    assert!(!plain.italic);
    for forbidden in [
        "mmath", "moMath", "mtext", "mctrlPr", "msty", "bi", "calc.exe", "objdata", "414243",
        "444546", "454647",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math style metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math style metadata controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Before Styled and Wrapped and Plain After"),
        "Office math style visible text missing from PDF text: {rendered_text:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mtext",
        b"mctrlPr",
        b"msty",
        b"calc.exe",
        b"objdata",
        b"414243",
        b"444546",
        b"454647",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math style metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_overline_accents_render_as_passive_strokes_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "macc{",
        "\\",
        "maccPr{",
        "\\",
        "mchr ",
        "\\",
        "u773?}}{",
        "\\",
        "me{",
        "\\",
        "mtext x}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before x After"),
        "unexpected accent math text: {text:?}"
    );
    let overlined_style = run_style_for_text(&parsed.document, "x").expect("accent base run");
    assert!(overlined_style.overline);
    for forbidden in ["mmath", "moMath", "macc", "maccPr", "mchr", "mtext"] {
        assert!(
            !text.contains(forbidden),
            "Office math accent control leaked to text: {forbidden}"
        );
    }
    assert!(
        !text.contains('?'),
        "Office math accent fallback text leaked to text: {text:?}"
    );
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "Office math accent controls should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before x After"));
    assert!(
        content.operations.windows(3).any(|operations| {
            operations[0].operator == "m"
                && operations[1].operator == "l"
                && operations[2].operator == "S"
        }),
        "Office math overline accent should render as a passive stroke"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"macc",
        b"maccPr",
        b"mchr",
        b"mtext",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math accent control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_text_accents_render_passively_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "macc{",
        "\\",
        "maccPr{",
        "launch.exe objdata 414243 ",
        "\\",
        "u65?",
        "\\",
        "'42",
        "https://example.com/payload objdata 444546 ",
        "\\",
        "u67?",
        "\\",
        "'44",
        "\\",
        "mchr ",
        "\\",
        "u770?}}{",
        "\\",
        "me{",
        "\\",
        "mtext x}}}}} and {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "macc{",
        "\\",
        "maccPr{",
        "calc.exe objdata 454647 ",
        "\\",
        "u72?",
        "\\",
        "'49",
        "\\",
        "mchr ",
        "\\",
        "u732?}}{",
        "\\",
        "me{",
        "\\",
        "mtext y}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before x^ and y~ After"),
        "unexpected text-accent math text: {text:?}"
    );
    for forbidden in [
        "mmath",
        "moMath",
        "macc",
        "maccPr",
        "mchr",
        "mtext",
        "launch.exe",
        "example.com/payload",
        "objdata",
        "414243",
        "444546",
        "454647",
        "calc.exe",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math text-accent metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math text-accent controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Before x^ and y~ After"),
        "Office math text accents missing from PDF text: {rendered_text:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"macc",
        b"maccPr",
        b"mchr",
        b"mtext",
        b"launch.exe",
        b"example.com/payload",
        b"objdata",
        b"414243",
        b"444546",
        b"454647",
        b"calc.exe",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math text-accent metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_border_boxes_render_passive_character_borders_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mborderBox{",
        "\\",
        "mborderBoxPr{calc.exe objdata 414243 ",
        "\\",
        "u65?",
        "\\",
        "'42{",
        "\\",
        "*",
        "\\",
        "unknown{",
        "\\",
        "object",
        "\\",
        "objdata 444546}}}}{",
        "\\",
        "me{",
        "\\",
        "mtext x+1}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before x+1 After"),
        "unexpected border-box math text: {text:?}"
    );
    let boxed_style = run_style_for_text(&parsed.document, "x+1").expect("border-box run");
    assert!(boxed_style.border.visible);
    assert_eq!(boxed_style.border.style, BorderStyle::Single);
    assert!(boxed_style.border.width_twips <= RtfLimits::default().max_table_border_width_twips);
    for forbidden in [
        "mmath",
        "moMath",
        "mborderBox",
        "mborderBoxPr",
        "mtext",
        "calc.exe",
        "objdata",
        "414243",
        "444546",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math border-box control leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math border-box controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before x+1 After"));
    let passive_strokes = content
        .operations
        .windows(3)
        .filter(|operations| {
            operations[0].operator == "m"
                && operations[1].operator == "l"
                && operations[2].operator == "S"
        })
        .count();
    assert!(
        passive_strokes >= 4,
        "Office math border box should render as passive line strokes, got {passive_strokes}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mborderBox",
        b"mborderBoxPr",
        b"mtext",
        b"calc.exe",
        b"objdata",
        b"414243",
        b"444546",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math border-box control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_manual_breaks_render_passive_line_breaks_without_control_leakage() {
    let input = br"{\rtf1 Before {\mmath{\moMath{\mbox{\mboxPr{\mbrk0 calc.exe objdata 414243 \u65?\'42 }{\*\unknown{\object\objdata 444546}}}{\me{\mtext First}}}{\mtext Second}}} After\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before \nFirstSecond After"),
        "unexpected Office math manual-break text: {text:?}"
    );
    for forbidden in [
        "mmath", "moMath", "mbox", "mboxPr", "mbrk", "mtext", "calc.exe", "objdata", "414243",
        "444546",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math manual-break control leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math manual-break controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before "));
    assert!(rendered_text.contains("FirstSecond After"));
    let before_pos = pdf_first_text_position_for_text(&content, "Before").expect("before position");
    let first_pos = pdf_first_text_position_for_text(&content, "First").expect("first position");
    assert!(
        first_pos.1 < before_pos.1,
        "Office math manual break should render following text below the prior line: before={before_pos:?}, first={first_pos:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mbox",
        b"mboxPr",
        b"mbrk",
        b"mtext",
        b"calc.exe",
        b"objdata",
        b"414243",
        b"444546",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math manual-break content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_box_operator_emulation_adds_passive_spacing_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 A{",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mbox{",
        "\\",
        "mboxPr{",
        "\\",
        "mopEmu1 launch.exe objdata 444546 ",
        "\\",
        "u67?'44}{",
        "\\",
        "*",
        "\\",
        "unknown{",
        "\\",
        "object",
        "\\",
        "objdata 414243}}}{",
        "\\",
        "me{",
        "\\",
        "mtext +}}}}}B ",
        "C{",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mbox{",
        "\\",
        "mboxPr{",
        "\\",
        "mopEmu0 https://example.com/payload objdata 454647 ",
        "\\",
        "u72?'49}}{",
        "\\",
        "me{",
        "\\",
        "mtext -}}}}}D",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("A + B C-D"),
        "unexpected Office math box operator-emulation text: {text:?}"
    );
    for forbidden in [
        "mmath",
        "moMath",
        "mbox",
        "mboxPr",
        "mopEmu",
        "mtext",
        "launch.exe",
        "example.com/payload",
        "objdata",
        "414243",
        "444546",
        "454647",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math box operator-emulation metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math box operator-emulation controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("A + B C-D"),
        "Office math operator-emulation spacing missing from PDF text: {rendered_text:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mbox",
        b"mboxPr",
        b"mopEmu",
        b"mtext",
        b"launch.exe",
        b"example.com/payload",
        b"objdata",
        b"414243",
        b"444546",
        b"454647",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math box operator-emulation metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_group_characters_render_passive_lines_without_fallback_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mgroupChr{",
        "\\",
        "mgroupChrPr{",
        "\\",
        "mchr ",
        "\\",
        "u9182?}}{",
        "\\",
        "me{",
        "\\",
        "mtext x+1}}}}} and {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mgroupChr{",
        "\\",
        "mgroupChrPr{",
        "\\",
        "mchr ",
        "\\",
        "u9183?}}{",
        "\\",
        "me{",
        "\\",
        "mtext y}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before x+1 and y After"),
        "unexpected group-character math text: {text:?}"
    );
    let over_style = run_style_for_text(&parsed.document, "x+1").expect("over group run");
    assert!(over_style.overline);
    let under_style = run_style_for_text(&parsed.document, "y").expect("under group run");
    assert_eq!(under_style.underline, UnderlineStyle::Single);
    for forbidden in [
        "mmath",
        "moMath",
        "mgroupChr",
        "mgroupChrPr",
        "mchr",
        "mtext",
        "?",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math group-character content leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math group-character controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before x+1 and y After"));
    let passive_strokes = content
        .operations
        .windows(3)
        .filter(|operations| {
            operations[0].operator == "m"
                && operations[1].operator == "l"
                && operations[2].operator == "S"
        })
        .count();
    assert!(
        passive_strokes >= 2,
        "Office math group characters should render as passive line strokes, got {passive_strokes}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mgroupChr",
        b"mgroupChrPr",
        b"mchr",
        b"mtext",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math group-character control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_group_character_positions_render_passively_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mgroupChr{",
        "\\",
        "mgroupChrPr{",
        "launch.exe objdata 414243 ",
        "\\",
        "u65?",
        "\\",
        "'42",
        "\\",
        "mchr ",
        "\\",
        "u9182?}{",
        "\\",
        "mpos bot}{",
        "https://example.com/payload objdata 444546 ",
        "\\",
        "u67?",
        "\\",
        "'44",
        "\\",
        "*",
        "\\",
        "unknown{",
        "\\",
        "object",
        "\\",
        "objdata 414243}}}{",
        "\\",
        "me{",
        "\\",
        "mtext x+1}}}}} Unsafe {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mgroupChr{",
        "\\",
        "mgroupChrPr{",
        "calc.exe objdata 454647 ",
        "\\",
        "u72?",
        "\\",
        "'49",
        "\\",
        "mchr ",
        "\\",
        "u9183?}{",
        "\\",
        "mpos calc.exe}}{",
        "\\",
        "me{",
        "\\",
        "mtext y}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before x+1 Unsafe y After"),
        "unexpected group-character position text: {text:?}"
    );
    let bottom_style = run_style_for_text(&parsed.document, "x+1").expect("bottom group run");
    assert!(!bottom_style.overline);
    assert_eq!(bottom_style.underline, UnderlineStyle::Single);
    let invalid_position_style =
        run_style_for_text(&parsed.document, "y").expect("invalid-position group run");
    assert_eq!(invalid_position_style.underline, UnderlineStyle::Single);
    for forbidden in [
        "mmath",
        "moMath",
        "mgroupChr",
        "mgroupChrPr",
        "mchr",
        "mpos",
        "bot",
        "calc.exe",
        "launch.exe",
        "example.com/payload",
        "mtext",
        "objdata",
        "414243",
        "444546",
        "454647",
        "?",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math group-character position metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math group-character position controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before x+1 Unsafe y After"));
    let passive_strokes = content
        .operations
        .windows(3)
        .filter(|operations| {
            operations[0].operator == "m"
                && operations[1].operator == "l"
                && operations[2].operator == "S"
        })
        .count();
    assert!(
        passive_strokes >= 2,
        "Office math group-character positions should render as passive line strokes, got {passive_strokes}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mgroupChr",
        b"mgroupChrPr",
        b"mchr",
        b"mpos",
        b"bot",
        b"calc.exe",
        b"launch.exe",
        b"example.com/payload",
        b"mtext",
        b"objdata",
        b"414243",
        b"444546",
        b"454647",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math group-character position metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_functions_render_passive_argument_spacing_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mfunc{",
        "\\",
        "mfuncPr{",
        "calc.exe http://example.invalid/payload objdata 444546 ",
        "\\",
        "u65?",
        "\\",
        "'42{",
        "\\",
        "*",
        "\\",
        "unknown{",
        "\\",
        "object",
        "\\",
        "objdata 414243}}",
        "\\",
        "mctrlPr}}{",
        "\\",
        "mfName{",
        "\\",
        "mtext sin}{",
        "\\",
        "*",
        "\\",
        "unknown{",
        "\\",
        "object",
        "\\",
        "objdata 414243}}}{",
        "\\",
        "me{",
        "\\",
        "mtext x}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before sin x After"),
        "unexpected function math text: {text:?}"
    );
    for forbidden in [
        "mmath",
        "moMath",
        "mfunc",
        "mfuncPr",
        "mfName",
        "mctrlPr",
        "mtext",
        "calc.exe",
        "example.invalid",
        "objdata",
        "414243",
        "444546",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math function content leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math function controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before sin x After"));
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mfunc",
        b"mfuncPr",
        b"mfName",
        b"mctrlPr",
        b"mtext",
        b"calc.exe",
        b"example.invalid",
        b"objdata",
        b"414243",
        b"444546",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math function content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_matrix_separators_render_between_cells_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mmatrix{",
        "\\",
        "mmatrixPr{",
        "\\",
        "msepChr ;}{",
        "\\",
        "*",
        "\\",
        "unknown{",
        "\\",
        "object",
        "\\",
        "objdata 414243}}}{",
        "\\",
        "mr{",
        "\\",
        "marg{",
        "\\",
        "mtext A}}{",
        "\\",
        "marg{",
        "\\",
        "mtext B}}}}}} After",
        "\\",
        "par ",
        "Unsafe {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mmatrix{",
        "\\",
        "mmatrixPr{",
        "\\",
        "msepChr calc.exe}}{",
        "\\",
        "mr{",
        "\\",
        "marg{",
        "\\",
        "mtext C}}{",
        "\\",
        "marg{",
        "\\",
        "mtext D}}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before A;B After"),
        "unexpected matrix separator text: {text:?}"
    );
    assert!(
        !text.contains("Before ;A"),
        "matrix separator leaked before the first cell: {text:?}"
    );
    assert!(
        text.contains("Unsafe C\tD After"),
        "multi-character matrix separator should fall back to a passive tab: {text:?}"
    );
    for forbidden in [
        "mmath",
        "moMath",
        "mmatrix",
        "mmatrixPr",
        "msepChr",
        "mtext",
        "objdata",
        "414243",
        "calc.exe",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math matrix separator content leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math matrix separator controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before A;B After"));
    assert!(
        !rendered_text.contains("Before ;A"),
        "matrix separator leaked before the first PDF cell: {rendered_text:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mmatrix",
        b"mmatrixPr",
        b"msepChr",
        b"mtext",
        b"objdata",
        b"414243",
        b"calc.exe",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math matrix separator content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_nary_hidden_limits_are_stripped_passively() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mnary{",
        "\\",
        "mnaryPr{",
        "\\",
        "mchr ",
        "\\",
        "u8721?}{",
        "\\",
        "msubHide1}{",
        "\\",
        "msupHide1}}{",
        "\\",
        "msub{",
        "\\",
        "mtext HIDDEN-SUB-LIMIT}}{",
        "\\",
        "msup{",
        "\\",
        "mtext HIDDEN-SUP-LIMIT}}{",
        "\\",
        "me{",
        "\\",
        "mtext i}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before \u{2211}i After"),
        "unexpected hidden n-ary limit text: {text:?}"
    );
    for forbidden in [
        "mmath",
        "moMath",
        "mnary",
        "mnaryPr",
        "msubHide",
        "msupHide",
        "msub",
        "msup",
        "mtext",
        "HIDDEN-SUB-LIMIT",
        "HIDDEN-SUP-LIMIT",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math hidden n-ary limit content leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math hidden n-ary limit controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before "));
    assert!(rendered_text.contains("i After"));
    let symbol_bytes = pdf_text_bytes_for_font(&content, b"F13");
    assert!(
        symbol_bytes.contains(&0xe5),
        "hidden-limit n-ary operator should encode through passive Symbol byte 0xe5; got {symbol_bytes:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mnary",
        b"mnaryPr",
        b"msubHide",
        b"msupHide",
        b"mtext",
        b"HIDDEN-SUB-LIMIT",
        b"HIDDEN-SUP-LIMIT",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math hidden n-ary limit content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_property_controls_are_passive_structure() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mbar{",
        "\\",
        "mbarPr{",
        "\\",
        "mctrlPr}}{",
        "\\",
        "mrad{",
        "\\",
        "mradPr{",
        "\\",
        "mdegHide1}}{",
        "\\",
        "msSubSup{",
        "\\",
        "msSubSupPr{",
        "\\",
        "mctrlPr}}{",
        "\\",
        "me{",
        "\\",
        "mtext x}}{",
        "\\",
        "msub{",
        "\\",
        "mtext i}}{",
        "\\",
        "msup{",
        "\\",
        "mtext 2}}}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before \u{221a}xi2 After"),
        "unexpected property-heavy math text: {text:?}"
    );
    let property_overbar_run = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Paragraph(paragraph) => paragraph.runs.iter().find(|run| run.text.contains('x')),
            _ => None,
        })
        .expect("property overbar run");
    assert!(property_overbar_run.style.overline);
    let property_subscript_style =
        run_style_for_text(&parsed.document, "i").expect("property subscript run");
    assert!(property_subscript_style.baseline_shift_half_points < 0);
    assert!(property_subscript_style.font_size_scale_percent < 100);
    let property_superscript_style =
        run_style_for_text(&parsed.document, "2").expect("property superscript run");
    assert!(property_superscript_style.baseline_shift_half_points > 0);
    assert!(property_superscript_style.font_size_scale_percent < 100);
    for forbidden in [
        "mmath",
        "moMath",
        "mbarPr",
        "mradPr",
        "msSubSupPr",
        "mdegHide",
        "mctrlPr",
        "mtext",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math property control leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "Office math property controls should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before "));
    assert!(rendered_text.contains("xi2 After"));
    let symbol_bytes = pdf_text_bytes_for_font(&content, b"F13");
    assert!(
        symbol_bytes.contains(&0xd6),
        "property-heavy radical should encode through passive Symbol byte 0xd6; got {symbol_bytes:?}"
    );
    let helvetica_bytes = pdf_text_bytes_for_font(&content, b"F1");
    assert!(
        !helvetica_bytes.contains(&0xaf),
        "property-heavy overbar should render as passive geometry, not as a macron glyph; got {helvetica_bytes:?}"
    );
    assert!(
        content.operations.windows(3).any(|operations| {
            operations[0].operator == "m"
                && operations[1].operator == "l"
                && operations[2].operator == "S"
        }),
        "property-heavy overbar should render as a passive stroke"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mbarPr",
        b"mradPr",
        b"msSubSupPr",
        b"mdegHide",
        b"mctrlPr",
        b"mtext",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math property control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_script_property_groups_strip_passive_metadata_payloads() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "msSubSup{",
        "\\",
        "msSubSupPr calc.exe objdata 444546 ",
        "\\",
        "u65?",
        "\\",
        "'42{",
        "\\",
        "*",
        "\\",
        "unknown{",
        "\\",
        "object",
        "\\",
        "objdata 414243}}}{",
        "\\",
        "me{",
        "\\",
        "mtext x}}{",
        "\\",
        "msub{",
        "\\",
        "msSubPr launch.exe objdata 454647 ",
        "\\",
        "u66?",
        "\\",
        "'43}{",
        "\\",
        "mtext i}}{",
        "\\",
        "msup{",
        "\\",
        "msSupPr https://example.com/payload objdata 464748 ",
        "\\",
        "u67?",
        "\\",
        "'44}{",
        "\\",
        "mtext 2}}}}} Pre {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "msPre{",
        "\\",
        "msPrePr HIDDEN-PRE-PROPERTY objdata 474849 ",
        "\\",
        "u68?",
        "\\",
        "'45}{",
        "\\",
        "msub{",
        "\\",
        "mtext a}}{",
        "\\",
        "msup{",
        "\\",
        "mtext b}}{",
        "\\",
        "me{",
        "\\",
        "mtext X}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before xi2 Pre abX After"),
        "unexpected script-property math text: {text:?}"
    );
    let subscript_style = run_style_for_text(&parsed.document, "i").expect("subscript run");
    assert!(subscript_style.baseline_shift_half_points < 0);
    assert!(subscript_style.font_size_scale_percent < 100);
    let superscript_style = run_style_for_text(&parsed.document, "2").expect("superscript run");
    assert!(superscript_style.baseline_shift_half_points > 0);
    assert!(superscript_style.font_size_scale_percent < 100);
    for forbidden in [
        "mmath",
        "moMath",
        "msSubSup",
        "msSubSupPr",
        "msSubPr",
        "msSupPr",
        "msPre",
        "msPrePr",
        "msub",
        "msup",
        "mtext",
        "calc.exe",
        "launch.exe",
        "https://example.com/payload",
        "HIDDEN-PRE-PROPERTY",
        "objdata",
        "414243",
        "444546",
        "454647",
        "464748",
        "474849",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math script property payload leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math script property controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Before xi2 Pre abX After"),
        "Office math script-property text missing from PDF: {rendered_text:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"msSubSup",
        b"msSubSupPr",
        b"msSubPr",
        b"msSupPr",
        b"msPre",
        b"msPrePr",
        b"msub",
        b"msup",
        b"mtext",
        b"calc.exe",
        b"launch.exe",
        b"https://example.com/payload",
        b"HIDDEN-PRE-PROPERTY",
        b"objdata",
        b"414243",
        b"444546",
        b"454647",
        b"464748",
        b"474849",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math script property payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_delimiters_render_passive_text_without_control_leakage() {
    let input = br"{\rtf1 Before {\mmath{\moMath{\md{\mdPr{\mbegChr (}{\mendChr )}}{\me{\mtext x+1}}}}} After\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before (x+1) After"),
        "unexpected delimiter math text: {text:?}"
    );
    for forbidden in ["mmath", "moMath", "mdPr", "mbegChr", "mendChr", "mtext"] {
        assert!(
            !text.contains(forbidden),
            "Office math delimiter control leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math delimiter controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("Office math layout approximated as passive text")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before (x+1) After"));
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mdPr",
        b"mbegChr",
        b"mendChr",
        b"mtext",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math delimiter control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_delimiter_separators_render_between_arguments_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "md{",
        "\\",
        "mdPr{",
        "launch.exe objdata 414243 ",
        "\\",
        "u65?",
        "\\",
        "'42",
        "\\",
        "mbegChr [}{",
        "\\",
        "msepChr ;}{",
        "\\",
        "mendChr ]}{",
        "https://example.com/payload objdata 444546 ",
        "\\",
        "u67?",
        "\\",
        "'44",
        "\\",
        "*",
        "\\",
        "unknown{",
        "\\",
        "object",
        "\\",
        "objdata 414243}}}{",
        "\\",
        "me{",
        "\\",
        "mtext A}}{",
        "\\",
        "me{",
        "\\",
        "mtext B}}}}} After",
        "\\",
        "par Unsafe {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "md{",
        "\\",
        "mdPr{",
        "calc.exe objdata 454647 ",
        "\\",
        "u72?",
        "\\",
        "'49",
        "\\",
        "mbegChr (}{",
        "\\",
        "msepChr calc.exe}{",
        "\\",
        "mendChr )}}{",
        "\\",
        "me{",
        "\\",
        "mtext C}}{",
        "\\",
        "me{",
        "\\",
        "mtext D}}}}} After",
        "\\",
        "par Invalid {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "md{",
        "\\",
        "mdPr{",
        "\\",
        "mbegChr calc.exe}{",
        "\\",
        "msepChr |}{",
        "\\",
        "mendChr launch.exe}}{",
        "\\",
        "me{",
        "\\",
        "mtext E}}{",
        "\\",
        "me{",
        "\\",
        "mtext F}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before [A;B] After"),
        "unexpected delimiter separator text: {text:?}"
    );
    assert!(
        text.contains("Unsafe (C\tD) After"),
        "multi-character delimiter separator should fall back to a passive tab: {text:?}"
    );
    assert!(
        text.contains("Invalid E|F After"),
        "multi-character delimiter begin/end values should be stripped while a safe separator remains: {text:?}"
    );
    for forbidden in [
        "mmath",
        "moMath",
        "mdPr",
        "mbegChr",
        "msepChr",
        "mendChr",
        "mtext",
        "launch.exe",
        "example.com/payload",
        "objdata",
        "414243",
        "444546",
        "454647",
        "calc.exe",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math delimiter separator metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math delimiter separator controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before [A;B] After"));
    assert!(
        rendered_text.contains("Unsafe (C\tD) After")
            || rendered_text.contains("Unsafe (CD) After"),
        "multi-character delimiter separator should not leak payload text in PDF: {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("Invalid E|F After"),
        "multi-character delimiter begin/end values should not leak payload text in PDF: {rendered_text:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mdPr",
        b"mbegChr",
        b"msepChr",
        b"mendChr",
        b"mtext",
        b"launch.exe",
        b"example.com/payload",
        b"objdata",
        b"414243",
        b"444546",
        b"454647",
        b"calc.exe",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math delimiter separator metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_matrices_render_passive_rows_and_cells() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mmatrix{",
        "\\",
        "mr{",
        "\\",
        "marg{",
        "\\",
        "mtext A1}}{",
        "\\",
        "marg{",
        "\\",
        "mtext B2}}}{",
        "\\",
        "mr{",
        "\\",
        "marg{",
        "\\",
        "mtext C3}}{",
        "\\",
        "marg{",
        "\\",
        "mtext D4}}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before A1\tB2\nC3\tD4 After"),
        "unexpected matrix math text: {text:?}"
    );
    for forbidden in ["mmath", "moMath", "mmatrix", "mr", "marg", "mtext"] {
        assert!(
            !text.contains(forbidden),
            "Office math matrix control leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math matrix controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    for visible in ["Before A1", "B2", "C3", "D4 After"] {
        assert!(
            rendered_text.contains(visible),
            "matrix visible text missing from PDF text: {visible}; got {rendered_text:?}"
        );
    }
    let a1_position = pdf_first_text_position_for_text(&content, "A1").expect("A1 position");
    let b2_position = pdf_first_text_position_for_text(&content, "B2").expect("B2 position");
    let c3_position = pdf_first_text_position_for_text(&content, "C3").expect("C3 position");
    assert!(
        b2_position.0 > a1_position.0,
        "Office math matrix cells should advance horizontally: A1={a1_position:?}, B2={b2_position:?}"
    );
    assert!(
        c3_position.1 < a1_position.1,
        "Office math matrix rows should render below prior rows: A1={a1_position:?}, C3={c3_position:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mmatrix",
        b"marg",
        b"mtext",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math matrix control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_equation_arrays_render_passive_rows() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "meqArr{",
        "\\",
        "meqArrPr calc.exe objdata 414243}{",
        "\\",
        "me{",
        "\\",
        "mtext X=1}}{",
        "\\",
        "me{",
        "\\",
        "mtext Y=2}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before X=1\nY=2 After"),
        "unexpected equation-array math text: {text:?}"
    );
    for forbidden in [
        "mmath", "moMath", "meqArr", "meqArrPr", "me", "mtext", "calc.exe", "objdata", "414243",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math equation-array control leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math equation-array controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    for visible in ["Before X=1", "Y=2 After"] {
        assert!(
            rendered_text.contains(visible),
            "equation-array visible text missing from PDF text: {visible}; got {rendered_text:?}"
        );
    }
    let x_position = pdf_first_text_position_for_text(&content, "X=1").expect("X=1 position");
    let y_position = pdf_first_text_position_for_text(&content, "Y=2").expect("Y=2 position");
    assert!(
        y_position.1 < x_position.1,
        "Office math equation-array rows should render below prior rows: X=1={x_position:?}, Y=2={y_position:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"meqArr",
        b"meqArrPr",
        b"mtext",
        b"calc.exe",
        b"objdata",
        b"414243",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math equation-array control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_limits_render_passive_lower_and_upper_scripts() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mlimLow{",
        "\\",
        "mlimLowPr calc.exe objdata 414243}{",
        "\\",
        "me{",
        "\\",
        "mtext lim}}{",
        "\\",
        "mlim{",
        "\\",
        "mtext x=0}}}{",
        "\\",
        "mlimUpp{",
        "\\",
        "mlimUppPr https://example.com/payload objdata 444546}{",
        "\\",
        "me{",
        "\\",
        "mtext max}}{",
        "\\",
        "mlim{",
        "\\",
        "mtext n}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before limx=0maxn After"),
        "unexpected limit math text: {text:?}"
    );
    let lower_limit_style = run_style_for_text(&parsed.document, "x=0").expect("lower limit run");
    assert!(lower_limit_style.baseline_shift_half_points < 0);
    assert!(lower_limit_style.font_size_scale_percent < 100);
    let upper_limit_style = run_style_for_text(&parsed.document, "n").expect("upper limit run");
    assert!(upper_limit_style.baseline_shift_half_points > 0);
    assert!(upper_limit_style.font_size_scale_percent < 100);
    for forbidden in [
        "mmath",
        "moMath",
        "mlimLow",
        "mlimLowPr",
        "mlimUpp",
        "mlimUppPr",
        "mlim",
        "mtext",
        "calc.exe",
        "example.com/payload",
        "objdata",
        "414243",
        "444546",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math limit control leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math limit controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before limx=0maxn After"));
    let lower_base = pdf_first_text_position_for_text(&content, "lim").expect("lim base");
    let lower_limit = pdf_first_text_position_for_text(&content, "x=0").expect("lower limit");
    let upper_base = pdf_first_text_position_for_text(&content, "max").expect("max base");
    let upper_limit = pdf_first_text_position_for_text(&content, "n").expect("upper limit");
    assert!(
        lower_limit.1 < lower_base.1,
        "Office math lower limit should render below the base text: base={lower_base:?}, limit={lower_limit:?}"
    );
    assert!(
        upper_limit.1 > upper_base.1,
        "Office math upper limit should render above the base text: base={upper_base:?}, limit={upper_limit:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mlimLow",
        b"mlimLowPr",
        b"mlimUpp",
        b"mlimUppPr",
        b"mlim",
        b"mtext",
        b"calc.exe",
        b"example.com/payload",
        b"objdata",
        b"414243",
        b"444546",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math limit control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_phantoms_are_stripped_from_passive_output() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mphant{",
        "\\",
        "mphantPr calc.exe objdata 414243 ",
        "\\",
        "u65?",
        "\\",
        "'42}{",
        "\\",
        "me{",
        "\\",
        "mtext HIDDEN-PHANTOM-PAYLOAD}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before  After"),
        "visible text around Office math phantom should remain: {text:?}"
    );
    for forbidden in [
        "mmath",
        "moMath",
        "mphant",
        "mphantPr",
        "mtext",
        "calc.exe",
        "objdata",
        "414243",
        "AB",
        "HIDDEN-PHANTOM-PAYLOAD",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math phantom content leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math phantom controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Before  After"),
        "visible text around Office math phantom missing from PDF text: {rendered_text:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mphant",
        b"mphantPr",
        b"mtext",
        b"calc.exe",
        b"objdata",
        b"414243",
        b"AB",
        b"HIDDEN-PHANTOM-PAYLOAD",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math phantom content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_phantom_mshow_renders_visible_content_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mphant{",
        "\\",
        "mphantPr{",
        "calc.exe objdata 414243 ",
        "\\",
        "u65?",
        "\\",
        "'42",
        "\\",
        "mshow1}{",
        "\\",
        "*",
        "\\",
        "unknown{",
        "\\",
        "object",
        "\\",
        "objdata 414243}}}{",
        "\\",
        "me{",
        "\\",
        "mtext VisiblePhantom}}}}} After",
        "\\",
        "par {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mphant{",
        "\\",
        "mphantPr{",
        "launch.exe https://example.com/payload objdata 444546 ",
        "\\",
        "u67?",
        "\\",
        "'44",
        "\\",
        "mshow0}}{",
        "\\",
        "me{",
        "\\",
        "mtext HIDDEN-PHANTOM-PAYLOAD}}}}}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before VisiblePhantom After"),
        "visible Office math phantom content should render when mshow is enabled: {text:?}"
    );
    for forbidden in [
        "mmath",
        "moMath",
        "mphant",
        "mphantPr",
        "mshow",
        "mtext",
        "calc.exe",
        "launch.exe",
        "example.com/payload",
        "objdata",
        "414243",
        "444546",
        "AB",
        "CD",
        "HIDDEN-PHANTOM-PAYLOAD",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math phantom metadata or hidden payload leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math phantom mshow controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Before VisiblePhantom After"),
        "visible Office math phantom content missing from PDF text: {rendered_text:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mphant",
        b"mphantPr",
        b"mshow",
        b"mtext",
        b"calc.exe",
        b"launch.exe",
        b"example.com/payload",
        b"objdata",
        b"414243",
        b"444546",
        b"AB",
        b"CD",
        b"HIDDEN-PHANTOM-PAYLOAD",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math phantom metadata or payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn legacy_code_page_hex_escapes_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansicpg437 cafe ",
        "\\",
        "'82 box ",
        "\\",
        "'b3",
        "\\",
        "par",
        "\\",
        "mac quote ",
        "\\",
        "'d2Hello",
        "\\",
        "'d3 bullet ",
        "\\",
        "'a5",
        "\\",
        "par",
        "\\",
        "pca nordic ",
        "\\",
        "'9b",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("cafe \u{00e9} box \u{2502}"));
    assert!(text.contains("quote \u{201c}Hello\u{201d} bullet \u{2022}"));
    assert!(text.contains("nordic \u{00f8}"));
    assert!(!text.contains("ansicpg"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("legacy-code-pages.rtf");
    let output_path = dir.path().join("legacy-code-pages.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"ansicpg".as_slice(),
        b"mac",
        b"pca",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn zero_width_formatting_controls_render_without_visible_glyph_or_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 A",
        "\\",
        "zwbo B",
        "\\",
        "zwnbo C",
        "\\",
        "zwnj D",
        "\\",
        "zwj E",
        "\\",
        "ltrmark L",
        "\\",
        "rtlmark R",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("A\u{200b}B\u{feff}C\u{200c}D\u{200d}E\u{200e}L\u{200f}R"));
    for forbidden in ["zwbo", "zwnbo", "zwnj", "zwj", "ltrmark", "rtlmark"] {
        assert!(
            !text.contains(forbidden),
            "forbidden zero-width control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("zero-width-formatting.rtf");
    let output_path = dir.path().join("zero-width-formatting.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"zwbo".as_slice(),
        b"zwnbo",
        b"zwnj",
        b"zwj",
        b"ltrmark",
        b"rtlmark",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden zero-width formatting content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn common_word_metadata_controls_do_not_warn_or_leak_to_pdf() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0",
        "\\",
        "fswiss Calibri;}{",
        "\\",
        "f1",
        "\\",
        "froman Times New Roman;}}",
        "\\",
        "pard",
        "\\",
        "adeflang1025",
        "\\",
        "nobrkwrptbl",
        "\\",
        "sprstsp",
        "\\",
        "doctype0",
        "\\",
        "oldas",
        "\\",
        "donotembedsysfont",
        "\\",
        "donotembedlingdata",
        "\\",
        "trackmoves0",
        "\\",
        "trackformatting1",
        "\\",
        "validatexml0",
        "\\",
        "showxmlerrors0",
        "\\",
        "saveinvalidxml0",
        "\\",
        "usenormstyforlist",
        "\\",
        "lang1033",
        "\\",
        "loch",
        "\\",
        "af0",
        "\\",
        "hich",
        "\\",
        "af0",
        "\\",
        "dbch",
        "\\",
        "af1",
        "\\",
        "ltrch",
        "\\",
        "kerning2",
        "\\",
        "hyphpar0",
        "\\",
        "adjustright Word metadata text",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Word metadata text"));
    for forbidden in [
        "lang1033",
        "loch",
        "hich",
        "dbch",
        "af0",
        "af1",
        "ltrch",
        "kerning2",
        "hyphpar0",
        "adjustright",
        "adeflang",
        "nobrkwrptbl",
        "sprstsp",
        "doctype",
        "oldas",
        "donotembedsysfont",
        "donotembedlingdata",
        "trackmoves",
        "trackformatting",
        "validatexml",
        "showxmlerrors",
        "saveinvalidxml",
        "usenormstyforlist",
    ] {
        assert!(
            !text.contains(forbidden),
            "Word metadata control leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "common Word metadata should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("character kerning approximated by passive pair spacing")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("paragraph hyphenation disabled")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(decoded_pdf_text(&content).contains("Word metadata text"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "TJ"),
        "kerning should render as passive positioned text"
    );
    for forbidden in [
        b"lang1033".as_slice(),
        b"adjustright",
        b"adeflang",
        b"nobrkwrptbl",
        b"sprstsp",
        b"doctype",
        b"oldas",
        b"donotembedsysfont",
        b"donotembedlingdata",
        b"trackmoves",
        b"trackformatting",
        b"validatexml",
        b"showxmlerrors",
        b"saveinvalidxml",
        b"usenormstyforlist",
        b"kerning",
        b"hyphpar",
        b"loch",
        b"hich",
        b"dbch",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Word metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn word_session_and_layout_flags_are_classified_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "deff0",
        "\\",
        "deflang1033",
        "\\",
        "deflangfe1033",
        "\\",
        "nouicompat",
        "\\",
        "rsidroot123456{",
        "\\",
        "*",
        "\\",
        "rsidtbl ",
        "\\",
        "rsid123456{",
        "\\",
        "object",
        "\\",
        "objdata 414243}}",
        "\\",
        "rsid123456",
        "\\",
        "insrsid123456",
        "\\",
        "delrsid789012",
        "\\",
        "vertdoc",
        "\\",
        "wraptrsp",
        "\\",
        "sprsspbf",
        "\\",
        "sprsbsp Visible body",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Visible body"));
    for forbidden in [
        "rsid", "rsidtbl", "insrsid", "delrsid", "vertdoc", "wraptrsp", "sprsspbf", "sprsbsp",
        "objdata", "414243",
    ] {
        assert!(
            !text.contains(forbidden),
            "Word session/layout control leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unsupported RTF control")
            && !diagnostic.message.contains("unknown RTF destination")),
        "Word session/layout flags should not be unknown or unsupported: {:?}",
        parsed.diagnostics
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("vertical document layout approximated")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("Word typography compatibility option approximated")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(decoded_pdf_text(&content).contains("Visible body"));
    for forbidden in [
        b"rsid".as_slice(),
        b"rsidtbl",
        b"insrsid",
        b"delrsid",
        b"vertdoc",
        b"wraptrsp",
        b"sprsspbf",
        b"sprsbsp",
        b"objdata",
        b"414243",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Word session/layout payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn custom_xml_markup_preserves_visible_text_without_metadata_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "deff0{",
        "\\",
        "*",
        "\\",
        "xmlnstbl {",
        "\\",
        "xmlns1 http://schemas.example/payload}}{",
        "\\",
        "*",
        "\\",
        "datastore 414243 {",
        "\\",
        "object",
        "\\",
        "objdata 414243}}{",
        "\\",
        "*",
        "\\",
        "themedata 504b0304}{",
        "\\",
        "*",
        "\\",
        "colorschememapping 414243}",
        "Before {",
        "\\",
        "xmlopen{",
        "\\",
        "xmlattrname secret}{",
        "\\",
        "xmlattrvalue 414243}tagged ",
        "\\",
        "xmlclose} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before tagged  After") || text.contains("Before tagged After"),
        "custom XML visible text was not preserved: {text:?}"
    );
    for forbidden in [
        "xmlopen",
        "xmlclose",
        "xmlattrname",
        "xmlattrvalue",
        "xmlnstbl",
        "datastore",
        "themedata",
        "colorschememapping",
        "secret",
        "414243",
        "objdata",
    ] {
        assert!(
            !text.contains(forbidden),
            "custom XML metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unsupported RTF control")
            && !diagnostic.message.contains("unknown RTF destination")),
        "custom XML controls should not be unknown or unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before tagged"));
    assert!(rendered_text.contains("After"));
    for forbidden in [
        b"xmlopen".as_slice(),
        b"xmlclose",
        b"xmlattrname",
        b"xmlattrvalue",
        b"xmlnstbl",
        b"datastore",
        b"themedata",
        b"colorschememapping",
        b"secret",
        b"414243",
        b"objdata",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "custom XML metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn opaque_metadata_payloads_obey_reject_policy() {
    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };

    for (input, expected_feature) in [
        (
            br"{\rtf1{\*\datastore 414243}Visible body\par}".as_slice(),
            "custom XML data store",
        ),
        (
            br"{\rtf1{\*\themedata 504b0304}Visible body\par}".as_slice(),
            "Office theme data",
        ),
        (
            br"{\rtf1{\info{\hlinkbase https://example.com/base/}}Visible body\par}".as_slice(),
            "hyperlink base",
        ),
        (
            br"{\rtf1{\*\colorschememapping 414243}Visible body\par}".as_slice(),
            "Office color scheme mapping",
        ),
        (
            br"{\rtf1{\*\xmlnstbl {\xmlns1 http://schemas.example/payload}}Visible body\par}"
                .as_slice(),
            "custom XML namespace table",
        ),
        (
            br"{\rtf1{\xmlattrvalue 414243}Visible body\par}".as_slice(),
            "custom XML attribute metadata",
        ),
        (
            br"{\rtf1 Before {\xmlopen tagged\par} After\par}".as_slice(),
            "custom XML markup",
        ),
        (
            br"{\rtf1{\datafield 414243}Visible body\par}".as_slice(),
            "form field data payload",
        ),
    ] {
        assert!(matches!(
            parse_rtf_bytes_with_options(input, &reject_options),
            Err(ParseError::ActiveContentRejected { feature, .. })
                if feature == expected_feature
        ));
    }
}

#[test]
fn word_layout_compatibility_controls_are_classified_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "deff0",
        "\\",
        "themelang1033",
        "\\",
        "themelangfe2052",
        "\\",
        "themelangcs1025",
        "\\",
        "dghspace180",
        "\\",
        "dgvspace180",
        "\\",
        "dghorigin1701",
        "\\",
        "dgvorigin1984",
        "\\",
        "dghshow1",
        "\\",
        "dgvshow1",
        "\\",
        "viewbksp1",
        "\\",
        "viewnobound1",
        "\\",
        "formdisp",
        "\\",
        "rempersonalinfo",
        "\\",
        "jexpand",
        "\\",
        "jcompress",
        "\\",
        "jclisttab",
        "\\",
        "asianbrkrule",
        "\\",
        "nogrowautofit",
        "\\",
        "lytexcttp",
        "\\",
        "lytprtmet",
        "\\",
        "noextrasprl",
        "\\",
        "notcvasp",
        "\\",
        "notvatxbx",
        "\\",
        "expshrtn",
        "\\",
        "useltbaln",
        "\\",
        "htmautsp",
        " Visible compatibility text",
        "\\",
        "par}",
    ]);

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Visible compatibility text"));
    for forbidden in [
        "themelang",
        "dghspace",
        "dgvspace",
        "dghorigin",
        "dgvorigin",
        "dghshow",
        "dgvshow",
        "viewbksp",
        "viewnobound",
        "formdisp",
        "rempersonalinfo",
        "jexpand",
        "jcompress",
        "jclisttab",
        "asianbrkrule",
        "nogrowautofit",
        "lytexcttp",
        "lytprtmet",
        "noextrasprl",
        "notcvasp",
        "notvatxbx",
        "expshrtn",
        "useltbaln",
        "htmautsp",
    ] {
        assert!(
            !text.contains(forbidden),
            "Word compatibility control leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "Word compatibility controls should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );
    for expected in [
        "Japanese text justification approximated by passive line layout",
        "Asian line-breaking rule approximated by passive Unicode line layout",
        "Word typography compatibility option approximated by passive layout",
    ] {
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "missing diagnostic: {expected}; diagnostics were {:?}",
            parsed.diagnostics
        );
    }
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("table autofit growth compatibility approximated")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(decoded_pdf_text(&content).contains("Visible compatibility text"));
    for forbidden in [
        b"themelang".as_slice(),
        b"dghspace",
        b"dgvspace",
        b"dghorigin",
        b"dgvorigin",
        b"dghshow",
        b"dgvshow",
        b"viewbksp",
        b"viewnobound",
        b"formdisp",
        b"rempersonalinfo",
        b"jexpand",
        b"jcompress",
        b"jclisttab",
        b"asianbrkrule",
        b"nogrowautofit",
        b"lytexcttp",
        b"lytprtmet",
        b"noextrasprl",
        b"notcvasp",
        b"notvatxbx",
        b"expshrtn",
        b"useltbaln",
        b"htmautsp",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Word compatibility content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn word_list_tab_metadata_renders_without_justification_warning_or_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "pard",
        "\\",
        "fi-360",
        "\\",
        "li360",
        "\\",
        "jclisttab",
        "\\",
        "tx360 List item",
        "\\",
        "par}",
    ]);

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("List item"));
    assert!(!text.contains("jclisttab"));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("Japanese text justification")),
        "list tab metadata should not be reported as Japanese justification: {:?}",
        parsed.diagnostics
    );
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "list tab metadata should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("List item"),
        "expected visible list text in PDF, got {rendered_text:?}"
    );
    for forbidden in [
        b"jclisttab".as_slice(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "list tab metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn paragraph_hyphenation_renders_passive_soft_breaks_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw3000",
        "\\",
        "margl720",
        "\\",
        "margr720",
        "\\",
        "hyphpar Antidisestablishmentarianism",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        other => panic!("expected paragraph, got {other:?}"),
    };

    assert!(paragraph.style.auto_hyphenation);
    assert!(text.contains("Antidisestablishmentarianism"));
    assert!(!text.contains("hyphpar"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("paragraph hyphenation approximated by bounded passive soft hyphenation")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.contains('-'),
        "passive automatic hyphenation should materialize a line-end hyphen: {rendered_text:?}"
    );
    assert_eq!(
        rendered_text.replace('-', ""),
        "Antidisestablishmentarianism"
    );
    for forbidden in [
        b"hyphpar".as_slice(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden hyphenation content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn document_hyphenation_default_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw3000",
        "\\",
        "margl720",
        "\\",
        "margr720",
        "\\",
        "hyphauto",
        "\\",
        "pard Antidisestablishmentarianism",
        "\\",
        "par",
        "\\",
        "pard Short",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let first = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        other => panic!("expected first paragraph, got {other:?}"),
    };
    let second = match &parsed.document.blocks[1] {
        Block::Paragraph(paragraph) => paragraph,
        other => panic!("expected second paragraph, got {other:?}"),
    };

    assert!(first.style.auto_hyphenation);
    assert!(second.style.auto_hyphenation);
    assert!(text.contains("AntidisestablishmentarianismShort"));
    assert!(!text.contains("hyphauto"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("document hyphenation approximated by bounded passive soft hyphenation")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.contains('-'),
        "document automatic hyphenation should materialize a passive line-end hyphen: {rendered_text:?}"
    );
    assert!(rendered_text.contains("Short"));
    for forbidden in [
        b"hyphauto".as_slice(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden document hyphenation content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn capital_word_hyphenation_control_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw3000",
        "\\",
        "margl720",
        "\\",
        "margr720",
        "\\",
        "hyphauto",
        "\\",
        "hyphcaps0",
        "\\",
        "pard ANTIDISESTABLISHMENTARIANISM",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        other => panic!("expected paragraph, got {other:?}"),
    };

    assert!(paragraph.style.auto_hyphenation);
    assert!(!paragraph.style.hyphenate_caps);
    assert!(text.contains("ANTIDISESTABLISHMENTARIANISM"));
    assert!(!text.contains("hyphcaps"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("document hyphenation approximated")
    }));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| { !diagnostic.message.contains("capitalized word hyphenation") })
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert_eq!(
        rendered_text, "ANTIDISESTABLISHMENTARIANISM",
        "all-caps automatic hyphenation should be suppressed when hyphcaps0 is active: {rendered_text:?}"
    );
    for forbidden in [
        b"hyphcaps".as_slice(),
        b"hyphauto",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden capital-word hyphenation content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn consecutive_hyphenation_limit_renders_passively_without_control_leakage() {
    let word = "AntidisestablishmentarianismAntidisestablishmentarianism";
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw3000",
        "\\",
        "margl720",
        "\\",
        "margr720",
        "\\",
        "hyphauto",
        "\\",
        "hyphconsec1",
        "\\",
        "pard ",
        word,
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        other => panic!("expected paragraph, got {other:?}"),
    };

    assert!(paragraph.style.auto_hyphenation);
    assert_eq!(paragraph.style.max_consecutive_hyphenated_lines, Some(1));
    assert!(text.contains(word));
    assert!(!text.contains("hyphconsec"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("consecutive automatic hyphenation limit applied")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.matches('-').count() <= 1,
        "consecutive automatic hyphenation should be bounded to one line-end hyphen: {rendered_text:?}"
    );
    assert_eq!(rendered_text.replace('-', ""), word);
    for forbidden in [
        b"hyphconsec".as_slice(),
        b"hyphauto",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden consecutive hyphenation content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn hyphenation_zone_renders_passively_without_control_leakage() {
    let word = "Antidisestablishment";
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw3000",
        "\\",
        "margl720",
        "\\",
        "margr720",
        "\\",
        "hyphauto",
        "\\",
        "hyphhotz0",
        "\\",
        "pard ",
        word,
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        other => panic!("expected paragraph, got {other:?}"),
    };

    assert!(paragraph.style.auto_hyphenation);
    assert_eq!(paragraph.style.hyphenation_zone_twips, 0);
    assert!(text.contains(word));
    assert!(!text.contains("hyphhotz"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("hyphenation zone applied to bounded passive hyphenation")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.contains('-'),
        "tight hyphenation zone should permit passive line-end hyphenation: {rendered_text:?}"
    );
    assert_eq!(rendered_text.replace('-', ""), word);
    for forbidden in [
        b"hyphhotz".as_slice(),
        b"hyphauto",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden hyphenation-zone content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn nonbreaking_hyphen_renders_passively_without_control_leakage() {
    let input = rtf(&["{", "\\", "rtf1 Before A", "\\", "_B after", "\\", "par}"]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before A\u{2011}B after"));
    assert!(!text.contains("\\_"));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before A-B after"));
    for forbidden in [
        b"\\_".as_slice(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden nonbreaking hyphen content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn word_preamble_note_and_line_number_controls_are_passive_approximations() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "ansicpg1252",
        "\\",
        "deflang1033",
        "\\",
        "deflangfe1033{",
        "\\",
        "*",
        "\\",
        "generator Microsoft Word 16.0;}",
        "\\",
        "viewkind4",
        "\\",
        "viewscale100",
        "\\",
        "viewzk2",
        "\\",
        "nouicompat",
        "\\",
        "horzdoc",
        "\\",
        "fet0",
        "\\",
        "ftntj",
        "\\",
        "ftnbj",
        "\\",
        "aenddoc",
        "\\",
        "endnhere",
        "\\",
        "sftnbj",
        "\\",
        "formshade",
        "\\",
        "linemod1",
        "\\",
        "linex360",
        "\\",
        "lineppage Body",
        "\\",
        "chftn{",
        "\\",
        "footnote Footnote text",
        "\\",
        "par}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Body1"));
    assert!(text.contains("Footnote text"));
    for forbidden in [
        "deflang",
        "viewscale",
        "nouicompat",
        "horzdoc",
        "ftntj",
        "ftnbj",
        "aenddoc",
        "endnhere",
        "sftnbj",
        "formshade",
        "linemod",
        "linex",
        "lineppage",
    ] {
        assert!(
            !text.contains(forbidden),
            "Word preamble/layout control leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "Word preamble controls should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("endnotes placed at passive section boundary")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("footnotes placed at passive page bottom")
    }));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("form-field shading approximated")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("line numbering approximated by passive margin text")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("1Body1"),
        "line numbering should render as passive margin text before body content: {rendered_text:?}"
    );
    assert!(rendered_text.contains("Footnote text"));
    for forbidden in [
        b"deflang".as_slice(),
        b"viewscale",
        b"nouicompat",
        b"horzdoc",
        b"ftntj",
        b"ftnbj",
        b"aenddoc",
        b"endnhere",
        b"sftnbj",
        b"formshade",
        b"linemod",
        b"linex",
        b"lineppage",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/AcroForm",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Word preamble/layout control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn zero_line_number_distance_does_not_enable_margin_numbers_or_leak_controls() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "sectd",
        "\\",
        "linex0 Body",
        "\\",
        "par}",
    ]);

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Body"));
    assert_eq!(parsed.document.page.line_numbering.distance_twips, 0);
    assert!(!parsed.document.page.line_numbering.enabled);
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("line numbering approximated")),
        "zero line-number distance should not enable numbering: {:?}",
        parsed.diagnostics
    );
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "zero line-number distance should not be unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Body"),
        "expected body text in PDF, got {rendered_text:?}"
    );
    assert!(
        !rendered_text.contains("1Body"),
        "zero line-number distance should not render margin number before body: {rendered_text:?}"
    );
    for forbidden in [
        b"linex".as_slice(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/AcroForm",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "zero line-number distance control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn no_line_number_paragraph_control_suppresses_passive_margin_number_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "sectd",
        "\\",
        "linex360",
        "\\",
        "linemod1 First",
        "\\",
        "par",
        "\\",
        "noline Suppressed",
        "\\",
        "par",
        "\\",
        "noline0 After",
        "\\",
        "par}",
    ]);

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("First"));
    assert!(text.contains("Suppressed"));
    assert!(text.contains("After"));
    assert!(!text.contains("noline"));
    let paragraph = |idx| match &parsed.document.blocks[idx] {
        open_rtf_converter::model::Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    assert!(!paragraph(0).style.suppress_line_numbers);
    assert!(paragraph(1).style.suppress_line_numbers);
    assert!(!paragraph(2).style.suppress_line_numbers);
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "line-number suppression should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("1First"),
        "first line should render passive margin number: {rendered_text:?}"
    );
    assert!(
        !rendered_text.contains("2Suppressed"),
        "suppressed paragraph should not render passive margin number: {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("3After"),
        "later numbered paragraph should preserve physical line sequence: {rendered_text:?}"
    );
    for forbidden in [
        b"noline".as_slice(),
        b"linemod",
        b"linex",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/AcroForm",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "line-number suppression control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn line_number_restart_modes_render_passively_without_control_leakage() {
    let continuous = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "sectd",
        "\\",
        "linex360",
        "\\",
        "linemod1",
        "\\",
        "linecont First",
        "\\",
        "par",
        "\\",
        "sect",
        "\\",
        "sectd",
        "\\",
        "linecont Second",
        "\\",
        "par}",
    ]);
    let section_restart = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "sectd",
        "\\",
        "linex360",
        "\\",
        "linemod1",
        "\\",
        "linecont First",
        "\\",
        "par",
        "\\",
        "sect",
        "\\",
        "sectd",
        "\\",
        "linerestart Second",
        "\\",
        "par}",
    ]);

    for (input, expected_second) in [(&continuous, "2Second"), (&section_restart, "1Second")] {
        let output = convert_rtf_to_pdf(
            input,
            &ConvertOptions {
                diagnostics: true,
                ..ConvertOptions::default()
            },
        )
        .unwrap();
        let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
        let rendered_text = parsed_pdf
            .get_pages()
            .values()
            .map(|page_id| {
                let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
                decoded_pdf_text(&content)
            })
            .collect::<String>();

        assert!(
            rendered_text.contains("1First"),
            "first section should render passive line number text: {rendered_text:?}"
        );
        assert!(
            rendered_text.contains(expected_second),
            "second section line-number restart mode should render {expected_second:?}: {rendered_text:?}"
        );
        for forbidden in [
            b"linecont".as_slice(),
            b"linerestart",
            b"linemod",
            b"linex",
            b"sectd",
            b"/JavaScript",
            b"/EmbeddedFile",
            b"/Launch",
            b"/AcroForm",
        ] {
            assert!(
                !output
                    .pdf
                    .windows(forbidden.len())
                    .any(|window| window == forbidden),
                "line numbering control leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn unicode_alternate_destinations_render_passively_without_fallback_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Header {",
        "\\",
        "upr{fallback-header}{",
        "\\",
        "*",
        "\\",
        "ud{",
        "\\",
        "u937?}}} body {",
        "\\",
        "upr{fallback-body}{",
        "\\",
        "*",
        "\\",
        "ud{",
        "\\",
        "u8212-}}}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Header \u{03a9} body \u{2014}"));
    for forbidden in ["fallback-header", "fallback-body", "upr", "ud"] {
        assert!(
            !text.contains(forbidden),
            "forbidden unicode alternate content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("unicode-alternate.rtf");
    let output_path = dir.path().join("unicode-alternate.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"fallback-header".as_slice(),
        b"fallback-body",
        b"upr",
        b"\\ud",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn oversized_unicode_fallback_skip_is_bounded_before_pdf_rendering() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "uc999 Before ",
        "\\",
        "u65? after",
        "\\",
        "par}",
    ]);
    let options = RtfParseOptions {
        limits: RtfLimits {
            max_unicode_fallback_skip: 1,
            ..RtfLimits::default()
        },
        ..RtfParseOptions::default()
    };
    let parsed = parse_rtf_bytes_with_options(&input, &options).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before A after"));
    assert!(!text.contains("uc999"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("Unicode fallback skip clamped from 999 to 1")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            parse_options: options,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.contains("Before A after"),
        "bounded Unicode fallback skip should preserve following visible text: {rendered_text:?}"
    );
    for forbidden in [
        b"uc999".as_slice(),
        b"u65",
        b"999",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden Unicode fallback content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn unicode_surrogate_pairs_are_normalized_before_pdf_rendering() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "uc1 Face ",
        "\\",
        "u-10179?",
        "\\",
        "u-8704? done",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Face \u{1f600} done"));
    assert!(!text.contains("u-10179"));
    assert!(!text.contains("u-8704"));
    assert!(!text.contains("??"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("unicode-surrogate-pair.rtf");
    let output_path = dir.path().join("unicode-surrogate-pair.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.contains("Face ? done"),
        "current Base14 PDF fallback text was {rendered_text:?}"
    );
    for forbidden in [
        b"u-10179".as_slice(),
        b"u-8704",
        b"??",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn tab_leaders_render_as_passive_pdf_text_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "tldot",
        "\\",
        "tx1440 Left",
        "\\",
        "tab Right",
        "\\",
        "par",
        "\\",
        "pard",
        "\\",
        "tlmdot",
        "\\",
        "tx1440 Middle",
        "\\",
        "tab Right",
        "\\",
        "par",
        "\\",
        "pard",
        "\\",
        "tleq",
        "\\",
        "tx1440 Equal",
        "\\",
        "tab Right",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Left\tRight"));
    assert!(text.contains("Middle\tRight"));
    assert!(text.contains("Equal\tRight"));
    for forbidden in ["tldot", "tlmdot", "tleq"] {
        assert!(
            !text.contains(forbidden),
            "forbidden tab leader control leaked to text: {forbidden}"
        );
    }
    assert!(!text.contains("tx1440"));
    let paragraphs = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            open_rtf_converter::model::Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        paragraphs[0].style.tab_stop_leaders,
        vec![open_rtf_converter::model::TabLeader::Dots]
    );
    assert_eq!(
        paragraphs[1].style.tab_stop_leaders,
        vec![open_rtf_converter::model::TabLeader::MiddleDots]
    );
    assert_eq!(
        paragraphs[2].style.tab_stop_leaders,
        vec![open_rtf_converter::model::TabLeader::Equals]
    );

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("tab-leader.rtf");
    let output_path = dir.path().join("tab-leader.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    assert!(pdf.windows(b"...".len()).any(|window| window == b"..."));
    assert!(pdf.windows(b"===".len()).any(|window| window == b"==="));
    for forbidden in [
        b"tldot".as_slice(),
        b"tlmdot",
        b"tleq",
        b"tx1440",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn default_tab_width_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "deftab360 Left",
        "\\",
        "tab Right",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert_eq!(parsed.document.default_tab_width_twips, 360);
    assert!(text.contains("Left\tRight"));
    assert!(!text.contains("deftab"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("default-tab-width.rtf");
    let output_path = dir.path().join("default-tab-width.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"deftab".as_slice(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn tab_alignment_controls_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "tqr",
        "\\",
        "tldot",
        "\\",
        "tx1440 Left",
        "\\",
        "tab 9",
        "\\",
        "par",
        "\\",
        "tqdec",
        "\\",
        "tx2160 Value",
        "\\",
        "tab 12.3",
        "\\",
        "par} ",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Left\t9"));
    assert!(text.contains("Value\t12.3"));
    assert!(!text.contains("tqr"));
    assert!(!text.contains("tqdec"));
    assert!(!text.contains("tldot"));
    assert!(!text.contains("tx1440"));
    assert!(!text.contains("tx2160"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("tab-alignment.rtf");
    let output_path = dir.path().join("tab-alignment.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    assert!(pdf.windows(b"...".len()).any(|window| window == b"..."));
    for forbidden in [
        b"tqr".as_slice(),
        b"tqdec",
        b"tldot",
        b"tx1440",
        b"tx2160",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn bar_tab_stops_render_passive_lines_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "tb",
        "\\",
        "tx720",
        "\\",
        "tx1440 Left",
        "\\",
        "tab Right",
        "\\",
        "par} ",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let paragraph = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .expect("paragraph");

    assert!(text.contains("Left\tRight"));
    assert_eq!(paragraph.style.tab_stops_twips, vec![720, 1440]);
    assert_eq!(
        paragraph.style.tab_stop_alignments,
        vec![TabAlignment::Bar, TabAlignment::Left]
    );
    for forbidden in ["tb", "tx720", "tx1440"] {
        assert!(
            !text.contains(forbidden),
            "forbidden bar-tab control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("bar-tab.rtf");
    let output_path = dir.path().join("bar-tab.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let has_bar_line = content.operations.windows(3).any(|operations| {
        operations[0].operator == "m"
            && operations[1].operator == "l"
            && operations[2].operator == "S"
            && operations[0].operands.first().and_then(pdf_operand_number)
                == operations[1].operands.first().and_then(pdf_operand_number)
    });

    assert!(rendered_text.contains("Left"));
    assert!(rendered_text.contains("Right"));
    assert!(
        has_bar_line,
        "bar tab stop should render a passive vertical stroke"
    );
    for forbidden in [
        b"tb".as_slice(),
        b"tx720",
        b"tx1440",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden bar-tab content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn bar_tab_stops_render_in_headers_and_tables_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "header",
        "\\",
        "tb",
        "\\",
        "tx720",
        "\\",
        "tx1440 Head",
        "\\",
        "tab Right",
        "\\",
        "par}",
        "\\",
        "trowd",
        "\\",
        "tb",
        "\\",
        "tx360",
        "\\",
        "tx720 Cell",
        "\\",
        "tab Value",
        "\\",
        "cellx1440",
        "\\",
        "cell",
        "\\",
        "row Body",
        "\\",
        "par} ",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let header = parsed.document.header.first().expect("header paragraph");
    let table = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("table");
    let cell_paragraph = &table.rows[0].cells[0].paragraphs[0];

    assert!(text.contains("Head\tRight"));
    assert!(text.contains("Cell\tValue"));
    assert!(text.contains("Body"));
    assert_eq!(
        header.style.tab_stop_alignments,
        vec![TabAlignment::Bar, TabAlignment::Left]
    );
    assert_eq!(
        cell_paragraph.style.tab_stop_alignments,
        vec![TabAlignment::Bar, TabAlignment::Left]
    );
    for forbidden in ["tb", "tx360", "tx720", "tx1440", "header", "trowd", "cellx"] {
        assert!(
            !text.contains(forbidden),
            "forbidden header/table bar-tab control leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let vertical_strokes = content
        .operations
        .windows(3)
        .filter(|operations| {
            operations[0].operator == "m"
                && operations[1].operator == "l"
                && operations[2].operator == "S"
                && operations[0].operands.first().and_then(pdf_operand_number)
                    == operations[1].operands.first().and_then(pdf_operand_number)
        })
        .count();

    for expected in ["Head", "Right", "Cell", "Value", "Body"] {
        assert!(
            rendered_text.contains(expected),
            "visible text {expected:?} missing from PDF text {rendered_text:?}"
        );
    }
    assert!(
        vertical_strokes >= 2,
        "header and table bar tabs should render passive vertical strokes"
    );
    for forbidden in [
        b"tb".as_slice(),
        b"tx360",
        b"tx720",
        b"tx1440",
        b"header",
        b"trowd",
        b"cellx",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden header/table bar-tab content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn picture_scaling_controls_render_passively_without_control_leakage() {
    let image_hex = bytes_to_hex(&minimal_jpeg_with_dimensions(2, 2));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "pict",
        "\\",
        "jpegblip",
        "\\",
        "picscalex50",
        "\\",
        "picscaley200 ",
        "\\",
        "piccropl120",
        "\\",
        "piccropt240",
        "\\",
        "piccropr360",
        "\\",
        "piccropb480 ",
        &image_hex,
        "}}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    match &parsed.document.blocks[0] {
        open_rtf_converter::model::Block::Image(image) => {
            assert_eq!(image.scale_x_percent, Some(50));
            assert_eq!(image.scale_y_percent, Some(200));
            assert_eq!(image.crop.left_twips, 120);
            assert_eq!(image.crop.top_twips, 240);
            assert_eq!(image.crop.right_twips, 360);
            assert_eq!(image.crop.bottom_twips, 480);
        }
        _ => panic!("expected image block"),
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("scaled-picture.rtf");
    let output_path = dir.path().join("scaled-picture.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    assert!(
        pdf.windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image")
    );
    for forbidden in [
        b"picscalex".as_slice(),
        b"picscaley",
        b"piccropl",
        b"piccropt",
        b"piccropr",
        b"piccropb",
        b"jpegblip",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn picture_goal_dimensions_and_scaling_combine_in_passive_pdf_transform() {
    let image_hex = bytes_to_hex(&minimal_jpeg_with_dimensions(2, 2));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "jpegblip",
        "\\",
        "picwgoal720",
        "\\",
        "pichgoal720",
        "\\",
        "picscalex50",
        "\\",
        "picscaley200 ",
        &image_hex,
        "} after",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            open_rtf_converter::model::Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("image block");
    assert_eq!(image.display_width_twips, Some(720));
    assert_eq!(image.display_height_twips, Some(720));
    assert_eq!(image.scale_x_percent, Some(50));
    assert_eq!(image.scale_y_percent, Some(200));
    for forbidden in ["picwgoal", "pichgoal", "picscalex", "picscaley", "jpegblip"] {
        assert!(
            !text.contains(forbidden),
            "picture sizing control leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    let image_transform = content
        .operations
        .iter()
        .find(|operation| operation.operator == "cm")
        .expect("image transform");
    assert_eq!(image_transform.operands.len(), 6);
    assert!(
        pdf_operand_number(&image_transform.operands[0])
            .is_some_and(|value| (value - 18.0).abs() < 0.01),
        "horizontal image matrix should combine picwgoal and picscalex; got {:?}",
        image_transform.operands
    );
    assert!(
        pdf_operand_number(&image_transform.operands[3])
            .is_some_and(|value| (value - 72.0).abs() < 0.01),
        "vertical image matrix should combine pichgoal and picscaley; got {:?}",
        image_transform.operands
    );
    for forbidden in [
        b"picwgoal".as_slice(),
        b"pichgoal",
        b"picscalex",
        b"picscaley",
        b"jpegblip",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden picture sizing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn picture_natural_size_hints_shape_passive_pdf_without_raw_control_leakage() {
    let image_hex = bytes_to_hex(&minimal_jpeg_with_dimensions(1, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "jpegblip",
        "\\",
        "picw80",
        "\\",
        "pich40 ",
        &image_hex,
        "} after",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            open_rtf_converter::model::Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("image block");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.width_px, 1);
    assert_eq!(image.height_px, 1);
    assert_eq!(image.natural_width_px_hint, Some(80));
    assert_eq!(image.natural_height_px_hint, Some(40));
    assert!(!text.contains("picw"));
    assert!(!text.contains("pich"));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let image_transform = content
        .operations
        .iter()
        .find(|operation| operation.operator == "cm")
        .expect("image transform");

    assert_eq!(image_transform.operands.len(), 6);
    assert!(
        pdf_operand_number(&image_transform.operands[0])
            .is_some_and(|value| (value - 60.0).abs() < 0.01),
        "horizontal image matrix should use picw natural size; got {:?}",
        image_transform.operands
    );
    assert!(
        pdf_operand_number(&image_transform.operands[3])
            .is_some_and(|value| (value - 30.0).abs() < 0.01),
        "vertical image matrix should use pich natural size; got {:?}",
        image_transform.operands
    );
    for forbidden in [
        b"picw".as_slice(),
        b"pich",
        b"jpegblip",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden picture natural-size content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn oversized_picture_goal_dimensions_are_bounded_before_pdf_rendering() {
    let image_hex = bytes_to_hex(&minimal_jpeg_with_dimensions(2, 2));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "jpegblip",
        "\\",
        "picwgoal999999",
        "\\",
        "pichgoal999999 ",
        &image_hex,
        "} after",
        "\\",
        "par}",
    ]);
    let options = RtfParseOptions {
        limits: RtfLimits {
            max_image_display_twips: 720,
            ..RtfLimits::default()
        },
        ..RtfParseOptions::default()
    };
    let parsed = parse_rtf_bytes_with_options(&input, &options).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            open_rtf_converter::model::Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("image block");

    assert_eq!(image.display_width_twips, Some(720));
    assert_eq!(image.display_height_twips, Some(720));
    assert!(!text.contains("picwgoal"));
    assert!(!text.contains("pichgoal"));
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("picture display width clamped") })
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("picture display height clamped")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            parse_options: options,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let image_transform = content
        .operations
        .iter()
        .find(|operation| operation.operator == "cm")
        .expect("image transform");

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert_eq!(image_transform.operands.len(), 6);
    assert!(
        pdf_operand_number(&image_transform.operands[0])
            .is_some_and(|value| (value - 36.0).abs() < 0.01),
        "horizontal image matrix should use clamped picture display width; got {:?}",
        image_transform.operands
    );
    assert!(
        pdf_operand_number(&image_transform.operands[3])
            .is_some_and(|value| (value - 36.0).abs() < 0.01),
        "vertical image matrix should use clamped picture display height; got {:?}",
        image_transform.operands
    );
    for forbidden in [
        b"picwgoal".as_slice(),
        b"pichgoal",
        b"999999",
        b"jpegblip",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden bounded picture sizing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn grayscale_png_picture_renders_passively_without_decoder_dependency_or_control_leakage() {
    let image_hex = bytes_to_hex(&minimal_grayscale_png_with_dimensions(1, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "pngblip",
        "\\",
        "picwgoal720",
        "\\",
        "pichgoal720 ",
        image_hex.as_str(),
        "} after",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            open_rtf_converter::model::Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("image block");
    assert_eq!(
        image.format,
        open_rtf_converter::model::ImageFormat::PngGrayscale
    );
    assert_eq!(image.width_px, 1);
    assert_eq!(image.height_px, 1);
    for forbidden in ["pngblip", "picwgoal", "pichgoal"] {
        assert!(
            !text.contains(forbidden),
            "grayscale PNG control leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 1);
    assert!(
        output
            .pdf
            .windows(b"/ColorSpace /DeviceGray".len())
            .any(|window| window == b"/ColorSpace /DeviceGray")
    );
    assert!(
        output
            .pdf
            .windows(b"/Colors 1".len())
            .any(|window| window == b"/Colors 1")
    );
    for forbidden in [
        b"pngblip".as_slice(),
        b"picwgoal",
        b"pichgoal",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden grayscale PNG content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn indexed_png_picture_renders_passively_with_bounded_palette_without_control_leakage() {
    let image_hex = bytes_to_hex(&minimal_indexed_png_with_dimensions(1, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "pngblip",
        "\\",
        "picwgoal720",
        "\\",
        "pichgoal720 ",
        image_hex.as_str(),
        "} after",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            open_rtf_converter::model::Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("image block");
    assert_eq!(
        image.format,
        open_rtf_converter::model::ImageFormat::PngIndexed
    );
    assert_eq!(image.width_px, 1);
    assert_eq!(image.height_px, 1);
    assert_eq!(image.palette, vec![255, 0, 0, 0, 255, 0]);
    for forbidden in ["pngblip", "picwgoal", "pichgoal"] {
        assert!(
            !text.contains(forbidden),
            "indexed PNG control leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 1);
    assert!(
        output
            .pdf
            .windows(b"/Indexed /DeviceRGB 1".len())
            .any(|window| window == b"/Indexed /DeviceRGB 1")
    );
    assert!(
        output
            .pdf
            .windows(b"/Colors 1".len())
            .any(|window| window == b"/Colors 1")
    );
    for forbidden in [
        b"pngblip".as_slice(),
        b"picwgoal",
        b"pichgoal",
        b"PLTE",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden indexed PNG content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn binary_jpeg_picture_renders_passively_without_control_leakage() {
    let image = minimal_jpeg_with_dimensions(2, 2);
    let mut input = br"{\rtf1 before {\pict\jpegblip\picwgoal720\pichgoal720\bin".to_vec();
    input.extend_from_slice(image.len().to_string().as_bytes());
    input.push(b' ');
    input.extend_from_slice(&image);
    input.extend_from_slice(br"} after\par}");

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("jpegblip"));
    assert!(!text.contains("bin"));
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .any(|block| matches!(block, open_rtf_converter::model::Block::Image(image) if image.format == open_rtf_converter::model::ImageFormat::Jpeg && image.width_px == 2 && image.height_px == 2))
    );

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("binary-jpeg-picture.rtf");
    let output_path = dir.path().join("binary-jpeg-picture.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        pdf.windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image")
    );
    for forbidden in [
        b"jpegblip".as_slice(),
        b"picwgoal",
        b"pichgoal",
        b"\\bin",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/AcroForm",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden binary picture content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn grayscale_jpeg_picture_renders_passively_without_control_leakage() {
    let image_hex = bytes_to_hex(&minimal_grayscale_jpeg_with_dimensions(1, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "jpegblip",
        "\\",
        "picwgoal720",
        "\\",
        "pichgoal720 ",
        image_hex.as_str(),
        "} after",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            open_rtf_converter::model::Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("image block");
    assert_eq!(
        image.format,
        open_rtf_converter::model::ImageFormat::JpegGrayscale
    );
    assert_eq!(image.width_px, 1);
    assert_eq!(image.height_px, 1);
    for forbidden in ["jpegblip", "picwgoal", "pichgoal"] {
        assert!(
            !text.contains(forbidden),
            "grayscale JPEG control leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 1);
    assert!(
        output
            .pdf
            .windows(b"/ColorSpace /DeviceGray".len())
            .any(|window| window == b"/ColorSpace /DeviceGray")
    );
    assert!(
        output
            .pdf
            .windows(b"/DCTDecode".len())
            .any(|window| window == b"/DCTDecode")
    );
    for forbidden in [
        b"jpegblip".as_slice(),
        b"picwgoal",
        b"pichgoal",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden grayscale JPEG content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn picture_metadata_controls_do_not_corrupt_image_or_leak_payloads() {
    let image_hex = bytes_to_hex(&minimal_jpeg_with_dimensions(1, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "jpegblip",
        "\\",
        "bliptag12345",
        "\\",
        "blipupi96",
        "\\",
        "blipuid 0123456789abcdef0123456789abcdef{",
        "\\",
        "picprop{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-picture-metadata-payload}}}{",
        "\\",
        "*",
        "\\",
        "picprop{",
        "\\",
        "object",
        "\\",
        "objdata 414243}}",
        "\\",
        "picwgoal720",
        "\\",
        "pichgoal720 ",
        image_hex.as_str(),
        "} after",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            open_rtf_converter::model::Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("image block");

    assert_eq!(image.format, open_rtf_converter::model::ImageFormat::Jpeg);
    assert_eq!(image.width_px, 1);
    assert_eq!(image.height_px, 1);
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| {
            !diagnostic.message.contains("unknown RTF")
                && !diagnostic.message.contains("unsupported RTF control")
                && !diagnostic
                    .message
                    .contains("JPEG picture data was malformed")
        }),
        "picture metadata should not corrupt image decoding or produce unknown-control noise: {:?}",
        parsed.diagnostics
    );
    for forbidden in [
        "bliptag",
        "blipupi",
        "blipuid",
        "0123456789abcdef",
        "picprop",
        "pFragments",
        "hostile-picture-metadata-payload",
        "objdata",
    ] {
        assert!(
            !text.contains(forbidden),
            "picture metadata leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "Do"),
        "valid JPEG should still render as a passive image"
    );
    for forbidden in [
        b"bliptag".as_slice(),
        b"blipupi",
        b"blipuid",
        b"0123456789abcdef",
        b"picprop",
        b"pFragments",
        b"hostile-picture-metadata-payload",
        b"objdata",
        b"414243",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "picture metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn hostile_binary_picture_payload_is_not_retokenized_or_copied_to_pdf() {
    let payload = br"{\object\objdata 414243 /JavaScript /EmbeddedFile}";
    let mut input = br"{\rtf1 before {\pict\jpegblip\bin".to_vec();
    input.extend_from_slice(payload.len().to_string().as_bytes());
    input.push(b' ');
    input.extend_from_slice(payload);
    input.extend_from_slice(br"} after\par}");

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("before"));
    assert!(text.contains("[Image skipped: malformed JPEG]"));
    assert!(text.contains("after"));
    for forbidden in ["object", "objdata", "414243", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "hostile binary picture payload leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("hostile-binary-picture.rtf");
    let output_path = dir.path().join("hostile-binary-picture.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("[Image skipped: malformed JPEG]"));
    assert!(rendered_text.contains("after"));
    for forbidden in [
        payload.as_slice(),
        b"objdata".as_slice(),
        b"414243",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/AcroForm",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "hostile binary picture payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn object_data_picture_payload_is_stripped_while_safe_result_renders() {
    let image = minimal_jpeg_with_dimensions(2, 2);
    let mut input = br"{\rtf1 before {\object\objdata {\pict\jpegblip\bin".to_vec();
    input.extend_from_slice(image.len().to_string().as_bytes());
    input.push(b' ');
    input.extend_from_slice(&image);
    input.extend_from_slice(br"}{\result visible fallback}} after\par}");

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains("before visible fallback after"),
        "normalized text was {text:?}; diagnostics were {:?}",
        parsed.diagnostics
    );
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::Image(_))),
        "object payload picture crossed into normalized document blocks: {:?}",
        parsed.document.blocks
    );
    for forbidden in ["pict", "jpegblip", "objdata", "[Image skipped"] {
        assert!(
            !text.contains(forbidden),
            "object payload leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("visible fallback"));
    assert!(rendered_text.contains("after"));
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"jpegblip",
        b"objdata",
        b"[Image skipped",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/AcroForm",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "object payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn object_data_structural_payload_does_not_seed_visible_result() {
    let input = br"{\rtf1 before {\object\objdata {\*\listtext Hidden marker\tab}{\result visible fallback}} after\par}";
    let parsed = parse_rtf_bytes(input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains("before visible fallback after"),
        "normalized text was {text:?}; diagnostics were {:?}",
        parsed.diagnostics
    );
    for forbidden in ["Hidden marker", "listtext", "objdata"] {
        assert!(
            !text.contains(forbidden),
            "object structural payload leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("visible fallback"));
    assert!(rendered_text.contains("after"));
    for forbidden in [
        b"Hidden marker".as_slice(),
        b"listtext",
        b"objdata",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/AcroForm",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "object structural payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn uncompressed_dib_picture_renders_passively_without_payload_leakage() {
    let image_hex = bytes_to_hex(&minimal_24bit_dib_with_dimensions(2, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "dibitmap",
        "\\",
        "picwgoal720",
        "\\",
        "pichgoal720 ",
        image_hex.as_str(),
        "} after",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    match parsed
        .document
        .blocks
        .iter()
        .find(|block| matches!(block, open_rtf_converter::model::Block::Image(_)))
        .expect("image block")
    {
        open_rtf_converter::model::Block::Image(image) => {
            assert_eq!(image.format, open_rtf_converter::model::ImageFormat::Rgb8);
            assert_eq!(image.width_px, 2);
            assert_eq!(image.height_px, 1);
            assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0]);
        }
        _ => unreachable!(),
    }
    assert!(!text.contains("dibitmap"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("uncompressed-dib-picture.rtf");
    let output_path = dir.path().join("uncompressed-dib-picture.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    assert!(
        pdf.windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image")
    );
    for forbidden in [
        b"dibitmap".as_slice(),
        b"424d",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden DIB content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn paletted_dib_picture_renders_passively_without_payload_leakage() {
    let image_hex = bytes_to_hex(&minimal_8bit_dib_with_dimensions(2, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "dibitmap",
        "\\",
        "picwgoal720",
        "\\",
        "pichgoal720 ",
        image_hex.as_str(),
        "} after",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            open_rtf_converter::model::Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("image block");
    assert_eq!(image.format, open_rtf_converter::model::ImageFormat::Rgb8);
    assert_eq!(image.width_px, 2);
    assert_eq!(image.height_px, 1);
    assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0]);
    assert!(!text.contains("dibitmap"));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 1);
    assert!(
        output
            .pdf
            .windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image")
    );
    assert!(
        output
            .pdf
            .windows(b"/ColorSpace /DeviceRGB".len())
            .any(|window| window == b"/ColorSpace /DeviceRGB")
    );
    for forbidden in [
        b"dibitmap".as_slice(),
        b"picwgoal",
        b"pichgoal",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden paletted DIB content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn low_bit_depth_dib_picture_renders_passively_without_payload_leakage() {
    let image_hex = bytes_to_hex(&minimal_4bit_dib_with_dimensions(2, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "dibitmap",
        "\\",
        "picwgoal720",
        "\\",
        "pichgoal720 ",
        image_hex.as_str(),
        "} after",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            open_rtf_converter::model::Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("image block");
    assert_eq!(image.format, open_rtf_converter::model::ImageFormat::Rgb8);
    assert_eq!(image.width_px, 2);
    assert_eq!(image.height_px, 1);
    assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0]);
    assert!(!text.contains("dibitmap"));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 1);
    assert!(
        output
            .pdf
            .windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image")
    );
    assert!(
        output
            .pdf
            .windows(b"/ColorSpace /DeviceRGB".len())
            .any(|window| window == b"/ColorSpace /DeviceRGB")
    );
    for forbidden in [
        b"dibitmap".as_slice(),
        b"picwgoal",
        b"pichgoal",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden low-bit-depth DIB content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn bitmap_core_dib_picture_renders_passively_without_payload_leakage() {
    let image_hex = bytes_to_hex(&minimal_4bit_bitmap_core_dib_with_dimensions(2, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "dibitmap",
        "\\",
        "picwgoal720",
        "\\",
        "pichgoal720 ",
        image_hex.as_str(),
        "} after",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            open_rtf_converter::model::Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("bitmap core DIB image block");
    assert_eq!(image.format, open_rtf_converter::model::ImageFormat::Rgb8);
    assert_eq!(image.width_px, 2);
    assert_eq!(image.height_px, 1);
    assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0]);
    assert!(!text.contains("dibitmap"));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 1);
    assert!(
        output
            .pdf
            .windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image")
    );
    assert!(
        output
            .pdf
            .windows(b"/ColorSpace /DeviceRGB".len())
            .any(|window| window == b"/ColorSpace /DeviceRGB")
    );
    for forbidden in [
        b"dibitmap".as_slice(),
        b"picwgoal",
        b"pichgoal",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden bitmap core DIB content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn shape_picture_wrapper_renders_modern_static_image_without_fallback_payload() {
    let image_hex = bytes_to_hex(&minimal_jpeg_with_dimensions(1, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "*",
        "\\",
        "shppict{",
        "\\",
        "pict",
        "\\",
        "jpegblip",
        "\\",
        "picwgoal720 ",
        image_hex.as_str(),
        "}}{",
        "\\",
        "nonshppict FALLBACK-PAYLOAD {",
        "\\",
        "object",
        "\\",
        "objdata 414243}{",
        "\\",
        "pict",
        "\\",
        "jpegblip",
        "\\",
        "picwgoal1440 ",
        image_hex.as_str(),
        "}} Visible",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let image_count = parsed
        .document
        .blocks
        .iter()
        .filter(|block| matches!(block, open_rtf_converter::model::Block::Image(_)))
        .count();
    let text = collect_text(&parsed.document);

    assert_eq!(image_count, 1);
    assert!(text.contains("Visible"));
    for forbidden in [
        "FALLBACK-PAYLOAD",
        "objdata",
        "414243",
        "nonshppict",
        "shppict",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden shape-picture wrapper content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("shape-picture-wrapper.rtf");
    let output_path = dir.path().join("shape-picture-wrapper.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(decoded_pdf_text(&content).contains("Visible"));
    assert!(
        pdf.windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image")
    );
    for forbidden in [
        b"FALLBACK-PAYLOAD".as_slice(),
        b"objdata",
        b"414243",
        b"nonshppict",
        b"shppict",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden shape-picture wrapper content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn nested_shape_picture_renders_without_shape_placeholder_or_property_leakage() {
    let image_hex = bytes_to_hex(&minimal_jpeg_with_dimensions(1, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before{",
        "\\",
        "shp{",
        "\\",
        "*",
        "\\",
        "shpinst{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hidden-shape-payload}}}{",
        "\\",
        "*",
        "\\",
        "shppict{",
        "\\",
        "pict",
        "\\",
        "jpegblip ",
        image_hex.as_str(),
        "}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let image_count = parsed
        .document
        .blocks
        .iter()
        .filter(|block| matches!(block, open_rtf_converter::model::Block::Image(_)))
        .count();
    let text = collect_text(&parsed.document);

    assert_eq!(image_count, 1);
    assert!(text.contains("Before After"));
    for forbidden in [
        "[Shape skipped",
        "hidden-shape-payload",
        "pFragments",
        "shppict",
        "shpinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden nested shape-picture content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("nested-shape-picture.rtf");
    let output_path = dir.path().join("nested-shape-picture.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(decoded_pdf_text(&content).contains("Before After"));
    assert!(
        pdf.windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image")
    );
    for forbidden in [
        b"[Shape skipped".as_slice(),
        b"hidden-shape-payload",
        b"pFragments",
        b"shppict",
        b"shpinst",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden nested shape-picture content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn header_picture_renders_passively_without_body_flow_or_payload_leakage() {
    let image_hex = bytes_to_hex(&minimal_jpeg_with_dimensions(1, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "header Logo {",
        "\\",
        "pict",
        "\\",
        "jpegblip",
        "\\",
        "picwgoal720",
        "\\",
        "pichgoal720 ",
        image_hex.as_str(),
        "}",
        "\\",
        "par} Body",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let body_image_count = parsed
        .document
        .blocks
        .iter()
        .filter(|block| matches!(block, open_rtf_converter::model::Block::Image(_)))
        .count();

    assert!(text.contains("Logo"));
    assert!(text.contains("Body"));
    assert_eq!(parsed.document.header_images.len(), 1);
    assert_eq!(body_image_count, 0);
    for forbidden in ["jpegblip", "picwgoal", "pichgoal"] {
        assert!(
            !text.contains(forbidden),
            "forbidden header picture control leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Logo"));
    assert!(rendered_text.contains("Body"));
    assert!(
        output
            .pdf
            .windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image"),
        "header picture should render as a passive PDF image"
    );
    for forbidden in [
        b"jpegblip".as_slice(),
        b"picwgoal",
        b"pichgoal",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden header picture content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn unsupported_picture_formats_are_placeholdered_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "pmmetafile1 4142434445464d4554415041594c4f4144} after",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(
        parsed.document.blocks.iter().any(|block| matches!(
            block,
            Block::Image(image)
                if image.format == ImageFormat::Placeholder
                    && image.bytes.is_empty()
                    && image.palette.is_empty()
        )),
        "unsupported picture should become a passive image placeholder"
    );
    assert!(!text.contains("ABC"));
    assert!(!text.contains("METAPAYLOAD"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("unsupported-picture.rtf");
    let output_path = dir.path().join("unsupported-picture.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"pmmetafile",
        b"41424344",
        b"ABCDEF",
        b"METAPAYLOAD",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn simple_wmf_picture_renders_passive_vector_preview_without_payload_leakage() {
    let wmf_hex = concat!(
        "0100090000032a0000000100070000000000",
        "050000000c026400c800",
        "07000000fc020000dcdcdc000000",
        "040000002d010000",
        "070000001b045000b4000a001400",
        "0700000018045a00be0014006400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF vector preview image");
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    assert!(
        image
            .vector_commands
            .iter()
            .any(|command| { matches!(command, StaticImageVectorCommand::Rectangle { .. }) })
    );
    assert!(
        image
            .vector_commands
            .iter()
            .any(|command| { matches!(command, StaticImageVectorCommand::Ellipse { .. }) })
    );
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in [
        "wmetafile",
        "010009",
        "dcdcdc",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF parser internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "WMF rectangle should render as passive PDF path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "c"),
        "WMF ellipse should render as passive PDF Bezier path"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"010009",
        b"dcdcdc",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF vector preview leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_unknown_record_reports_partial_preview_without_payload_leakage() {
    let wmf_hex = concat!(
        "0100090000032d0000000100070000000000",
        "050000000c026400c800",
        "07000000fc020000dcdcdc000000",
        "040000002d010000",
        "070000001b045000b4000a001400",
        "0700000018045a00be0014006400",
        "030000009999",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("partial WMF vector preview image");

    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    assert!(
        image
            .vector_commands
            .iter()
            .any(|command| { matches!(command, StaticImageVectorCommand::Rectangle { .. }) })
    );
    assert!(
        image
            .vector_commands
            .iter()
            .any(|command| { matches!(command, StaticImageVectorCommand::Ellipse { .. }) })
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("1 unsupported record(s) skipped")
    }));
    for forbidden in ["wmetafile", "010009", "9999", "dcdcdc"] {
        assert!(
            !text.contains(forbidden),
            "partial WMF internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "supported WMF records should still render as passive PDF paths"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"010009",
        b"030000009999",
        b"dcdcdc",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "partial WMF vector preview leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_mfcomment_escape_is_ignored_as_non_visual_metadata_without_payload_leakage() {
    let wmf_hex = concat!(
        "0100090000033e0000000100140000000000",
        "1400000026060f001e00ffffffff040014000000576f72640e004d6963726f736f667420576f7264",
        "050000000c026400c800",
        "07000000fc020000dcdcdc000000",
        "040000002d010000",
        "070000001b045000b4000a001400",
        "0700000018045a00be0014006400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF MFCOMMENT vector preview image");

    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    assert!(
        image
            .vector_commands
            .iter()
            .any(|command| { matches!(command, StaticImageVectorCommand::Rectangle { .. }) })
    );
    assert!(
        image
            .vector_commands
            .iter()
            .any(|command| { matches!(command, StaticImageVectorCommand::Ellipse { .. }) })
    );
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in ["wmetafile", "0626", "576f7264", "Word", "Microsoft Word"] {
        assert!(
            !text.contains(forbidden),
            "WMF MFCOMMENT metadata leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "WMF MFCOMMENT should not prevent supported records from rendering"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"0626",
        b"576f7264",
        b"Word",
        b"Microsoft Word",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF MFCOMMENT leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_hatched_brush_renders_passive_clipped_lines_without_payload_leakage() {
    let wmf_hex = concat!(
        "010009000003230000000100070000000000",
        "050000000c026400c800",
        "07000000fc020200ff0000000000",
        "040000002d010000",
        "070000001b045000b4000a001400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF vector preview image");
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::Rectangle {
                fill_pattern: ShadingPattern::Horizontal,
                fill_color: Some(color),
                ..
            } if color.red == 255 && color.green == 0 && color.blue == 0
        )
    }));
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in [
        "wmetafile",
        "010009",
        "02fc",
        "0200ff0000000000",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF parser internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "W"),
        "WMF hatch fill should be clipped to the passive rectangle"
    );
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "l")
            .count()
            >= 2,
        "WMF hatch fill should render as passive PDF line paths"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"010009",
        b"02fc",
        b"0200ff0000000000",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF hatch preview leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_non_rectangular_hatched_brushes_render_with_passive_clipping() {
    let wmf_hex = concat!(
        "0100090000032d00000001000a0000000000",
        "050000000c026400c800",
        "07000000fc0202000000ff000500",
        "040000002d010000",
        "0a0000002403030014002800b400280064005a00",
        "0700000018045a00be0014006400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("non-rectangular hatch WMF vector preview image");

    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::Polygon {
                fill_pattern: ShadingPattern::DiagonalCross,
                fill_color: Some(color),
                ..
            } if color.red == 0 && color.green == 0 && color.blue == 255
        )
    }));
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::Ellipse {
                fill_pattern: ShadingPattern::DiagonalCross,
                fill_color: Some(color),
                ..
            } if color.red == 0 && color.green == 0 && color.blue == 255
        )
    }));
    for forbidden in [
        "wmetafile",
        "010009",
        "02fc",
        "02000000ff000500",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "non-rectangular hatch WMF internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "W" || operation.operator == "W*")
            .count()
            >= 2,
        "polygon and ellipse hatch fills should both be clipped before passive line drawing"
    );
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "l")
            .count()
            >= 4,
        "non-rectangular hatch fills should render passive line paths"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "c"),
        "ellipse hatch clipping should preserve the passive Bezier ellipse path"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"010009",
        b"02fc",
        b"02000000ff000500",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "non-rectangular hatch preview leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn placeable_wmf_picture_renders_passive_vector_preview_without_payload_leakage() {
    let wmf_hex = concat!(
        "d7cdc69a000000000000c8006400a005000000001d52",
        "0100090000032a0000000100070000000000",
        "050000000c026400c800",
        "07000000fc020000dcdcdc000000",
        "040000002d010000",
        "070000001b045000b4000a001400",
        "0700000018045a00be0014006400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("placeable WMF vector preview image");
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert_eq!(image.width_px, 200);
    assert_eq!(image.height_px, 100);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    assert!(
        image
            .vector_commands
            .iter()
            .any(|command| { matches!(command, StaticImageVectorCommand::Rectangle { .. }) })
    );
    assert!(
        image
            .vector_commands
            .iter()
            .any(|command| { matches!(command, StaticImageVectorCommand::Ellipse { .. }) })
    );
    for forbidden in [
        "wmetafile",
        "d7cdc69a",
        "9ac6cdd7",
        "010009",
        "dcdcdc",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "placeable WMF internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "placeable WMF rectangle should render as passive PDF path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "c"),
        "placeable WMF ellipse should render as passive PDF Bezier path"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"d7cdc69a",
        b"9ac6cdd7",
        b"010009",
        b"dcdcdc",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "placeable WMF vector preview leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn invalid_placeable_wmf_checksum_becomes_placeholder_without_payload_leakage() {
    let wmf_hex = concat!(
        "d7cdc69a000000000000c8006400a005000000000000",
        "0100090000032a0000000100070000000000",
        "050000000c026400c800",
        "07000000fc020000dcdcdc000000",
        "040000002d010000",
        "070000001b045000b4000a001400",
        "0700000018045a00be0014006400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("invalid placeable WMF placeholder image");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    assert!(image.vector_commands.is_empty());
    assert!(!parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("WMF picture rendered as bounded passive vector preview")
    }));
    for forbidden in ["d7cdc69a", "9ac6cdd7", "010009", "dcdcdc"] {
        assert!(
            !text.contains(forbidden),
            "invalid placeable WMF internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    for forbidden in [
        b"d7cdc69a".as_slice(),
        b"9ac6cdd7",
        b"010009",
        b"dcdcdc",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "invalid placeable WMF leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_window_origin_offsets_are_normalized_before_passive_vector_rendering() {
    let wmf_hex = concat!(
        "010009000003280000000100070000000000",
        "050000000b0214000a00",
        "050000000c026400c800",
        "07000000fc020000dcdcdc000000",
        "040000002d010000",
        "070000001b045a00b4001e001400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("origin-offset WMF vector preview image");

    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::Rectangle {
                left,
                top,
                right,
                bottom,
                ..
            } if (*left - 10.0).abs() < 0.01
                && (*top - 10.0).abs() < 0.01
                && (*right - 170.0).abs() < 0.01
                && (*bottom - 70.0).abs() < 0.01
        )
    }));
    assert_no_wmf_preview_warning(&parsed.diagnostics);
}

#[test]
fn wmf_line_polyline_and_polygon_render_as_passive_paths_without_payload_leakage() {
    let wmf_hex = concat!(
        "0100090000033c00000001000c0000000000",
        "050000000c026400c800",
        "07000000fc020000dcdcdc000000",
        "040000002d010000",
        "05000000140214001400",
        "0500000013021400b400",
        "0a0000002403030014002800b400280064005a00",
        "0c00000025030400140046003c005a008c004600b4005a00",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("line/polyline/polygon WMF vector preview image");

    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    assert!(
        image
            .vector_commands
            .iter()
            .any(|command| { matches!(command, StaticImageVectorCommand::Line { .. }) })
    );
    assert!(image.vector_commands.iter().any(|command| {
        matches!(command, StaticImageVectorCommand::Polyline { points, .. } if points.len() == 4)
    }));
    assert!(image.vector_commands.iter().any(|command| {
        matches!(command, StaticImageVectorCommand::Polygon { points, .. } if points.len() == 3)
    }));
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in [
        "wmetafile",
        "010009",
        "0324",
        "0325",
        "dcdcdc",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF line/polyline/polygon internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "m"),
        "WMF line/polyline/polygon should emit passive PDF move path operations"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "l"),
        "WMF line/polyline/polygon should emit passive PDF line path operations"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"010009",
        b"0324",
        b"0325",
        b"dcdcdc",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF line/polyline/polygon leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_pen_width_renders_passive_stroke_width_without_record_leakage() {
    let wmf_hex = concat!(
        "010009000003270000000100080000000000",
        "050000000c026400c800",
        "08000000fa0200000c000000ff000000",
        "040000002d010000",
        "05000000140214001400",
        "0500000013021400b400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF pen-width vector preview image");

    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::Line {
                stroke_color: Some(color),
                stroke_width,
                ..
            } if color.red == 255
                && color.green == 0
                && color.blue == 0
                && (*stroke_width - 12.0).abs() < 0.01
        )
    }));
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in ["wmetafile", "02fa", "0c000000", "ff000000", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "WMF pen-width internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content.operations.iter().any(|operation| {
            operation.operator == "w"
                && operation
                    .operands
                    .first()
                    .and_then(pdf_operand_number)
                    .is_some_and(|value| value > 1.0)
        }),
        "WMF pen width should emit a bounded passive PDF stroke width"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"02fa",
        b"0c000000",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF pen-width leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_pen_dash_style_renders_passive_dash_pattern_without_record_leakage() {
    let wmf_hex = concat!(
        "010009000003270000000100080000000000",
        "050000000c026400c800",
        "08000000fa02010004000000ff000000",
        "040000002d010000",
        "05000000140214001400",
        "0500000013021400b400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF dashed-pen vector preview image");

    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::Line {
                stroke_color: Some(color),
                stroke_width,
                stroke_style: BorderStyle::Dashed,
                ..
            } if color.red == 255
                && color.green == 0
                && color.blue == 0
                && (*stroke_width - 4.0).abs() < 0.01
        )
    }));
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in [
        "wmetafile",
        "02fa",
        "010004000000",
        "ff000000",
        "JavaScript",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF dashed-pen internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "d"),
        "WMF dashed pen should emit a passive PDF dash pattern"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"02fa",
        b"010004000000",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF dashed-pen leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_polyfill_alternate_renders_even_odd_fill_without_record_leakage() {
    let wmf_hex = concat!(
        "0100090000032c00000001000c0000000000",
        "050000000c026400c800",
        "07000000fc02000000ffff000000",
        "040000002d010000",
        "0400000006010100",
        "0c0000002403040014001400b400500014005000b4001400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF polyfill-mode vector preview image");

    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::Polygon {
                points,
                fill_rule: StaticImageVectorFillRule::Alternate,
                fill_color: Some(_),
                ..
            } if points.len() == 4
        )
    }));
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in ["wmetafile", "0106", "0324", "00ffff", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "WMF polyfill-mode internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "B*"),
        "WMF ALTERNATE polyfill mode should emit passive even-odd fill/stroke"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"0106",
        b"0324",
        b"00ffff",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF polyfill-mode leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_polypolygon_renders_multiple_passive_polygons_without_payload_leakage() {
    let wmf_hex = concat!(
        "010009000003300000000100140000000000",
        "050000000c026400c800",
        "07000000fc02000000ffff000000",
        "040000002d010000",
        "14000000380502000300040014001400b400140064003c00",
        "2800460050005a007800460050003200",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF POLYPOLYGON vector preview image");

    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    let polygons = image
        .vector_commands
        .iter()
        .filter_map(|command| match command {
            StaticImageVectorCommand::Polygon {
                points, fill_color, ..
            } => Some((points, fill_color)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(polygons.len(), 2);
    assert_eq!(polygons[0].0.len(), 3);
    assert_eq!(polygons[1].0.len(), 4);
    assert!(
        polygons
            .iter()
            .all(|(_, fill_color)| matches!(fill_color, Some(color) if color.red == 0 && color.green == 255 && color.blue == 255))
    );
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in ["wmetafile", "0538", "00ffff", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "WMF POLYPOLYGON internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "h")
            .count()
            >= 2,
        "WMF POLYPOLYGON should close each passive polygon path"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"0538",
        b"00ffff",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF POLYPOLYGON leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_deleted_object_handles_are_reused_for_passive_style_selection() {
    let wmf_hex = concat!(
        "0100090000032e00000001000c0000000000",
        "050000000c026400c800",
        "07000000fc020000dcdcdc000000",
        "04000000f0010000",
        "07000000fc0200000000ff000000",
        "040000002d010000",
        "070000001b045000b4000a001400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF reused object handle vector preview image");

    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::Rectangle {
                fill_color: Some(color),
                ..
            } if color.red == 0 && color.green == 0 && color.blue == 255
        )
    }));
    assert_no_wmf_preview_warning(&parsed.diagnostics);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "WMF reused brush rectangle should render as passive PDF path"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"010009",
        b"dcdcdc",
        b"0000ff",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF reused object handle leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_patblt_patcopy_renders_passive_brush_rectangle_without_payload_leakage() {
    let wmf_hex = concat!(
        "010009000003250000000100090000000000",
        "050000000c026400c800",
        "07000000fc020000ff0000000000",
        "040000002d010000",
        "090000001d062100f000140028001e003200",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF PATBLT vector preview image");

    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::Rectangle {
                left,
                top,
                right,
                bottom,
                stroke_color: None,
                fill_color: Some(color),
                ..
            } if (*left - 40.0).abs() < 0.01
                && (*top - 20.0).abs() < 0.01
                && (*right - 90.0).abs() < 0.01
                && (*bottom - 50.0).abs() < 0.01
                && color.red == 255
                && color.green == 0
                && color.blue == 0
        )
    }));
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in ["wmetafile", "061d", "00f00021", "ff0000", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "WMF PATBLT internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "WMF PATBLT should render as passive PDF rectangle path"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"061d",
        b"00f00021",
        b"ff0000",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF PATBLT leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_setpixel_renders_passive_filled_pixel_without_payload_leakage() {
    let wmf_hex = concat!(
        "010009000003180000000000070000000000",
        "050000000c026400c800",
        "070000001f04ff00000020004000",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF SETPIXEL vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::Rectangle {
                left,
                top,
                right,
                bottom,
                stroke_color: None,
                fill_color: Some(color),
                ..
            } if (*left - 64.0).abs() < 0.01
                && (*top - 32.0).abs() < 0.01
                && (*right - 65.0).abs() < 0.01
                && (*bottom - 33.0).abs() < 0.01
                && color.red == 255
                && color.green == 0
                && color.blue == 0
        )
    }));
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in ["wmetafile", "041f", "ff0000", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "WMF SETPIXEL internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "WMF SETPIXEL should render as a passive PDF rectangle path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "f"),
        "WMF SETPIXEL should render as a passive PDF fill"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"041f",
        b"ff0000",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF SETPIXEL leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_textout_renders_passive_text_without_payload_leakage() {
    let wmf_hex = concat!(
        "0100090000032d00000001000c0000000000",
        "050000000c026400c800",
        "050000000902ff000000",
        "0c000000fb02f4ff00000000000000000000000000000000",
        "040000002d010000",
        "0700000021050200486914002800",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF TEXTOUT vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("Hi"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::Text {
                x,
                y,
                height,
                text,
                color: Some(color),
                ..
            } if (*x - 40.0).abs() < 0.01
                && (*y - 20.0).abs() < 0.01
                && (*height - 12.0).abs() < 0.01
                && text == "Hi"
                && color.red == 255
                && color.green == 0
                && color.blue == 0
        )
    }));
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in ["wmetafile", "0521", "fb02", "ff0000", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "WMF TEXTOUT internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        decoded_pdf_text(&content).contains("Hi"),
        "WMF TEXTOUT should render through passive PDF text operations"
    );
    assert!(
        pdf_text_font_names(&content)
            .iter()
            .any(|font| font.as_slice() == b"F1"),
        "WMF TEXTOUT should use a passive built-in font resource"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"0521",
        b"fb02",
        b"ff0000",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF TEXTOUT leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_saved_dc_restores_passive_text_state_without_record_leakage() {
    let wmf_hex = concat!(
        "0100090000034000000001000c0000000000",
        "050000000c026400c800",
        "050000000902ff000000",
        "030000001e00",
        "0500000009020000ff00",
        "0c000000fb02f4ff00000000000000000000000000000000",
        "040000002d010000",
        "0700000021050200496e14002800",
        "040000002701ffff",
        "0700000021050200486928002800",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF SaveDC/RestoreDC vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("In"));
    assert!(!text.contains("Hi"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::Text {
                text,
                color: Some(color),
                ..
            } if text == "In" && color.red == 0 && color.green == 0 && color.blue == 255
        )
    }));
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::Text {
                text,
                color: Some(color),
                ..
            } if text == "Hi" && color.red == 255 && color.green == 0 && color.blue == 0
        )
    }));
    for forbidden in ["wmetafile", "001e", "0127", "0209", "fb02", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "WMF SaveDC/RestoreDC internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("In"));
    assert!(rendered_text.contains("Hi"));
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"001e",
        b"0127",
        b"0209",
        b"fb02",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF SaveDC/RestoreDC leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_text_character_extra_renders_passive_spacing_without_record_leakage() {
    let wmf_hex = concat!(
        "0100090000033100000001000c0000000000",
        "050000000c026400c800",
        "050000000902ff000000",
        "0400000008010a00",
        "0c000000fb02f4ff00000000000000000000000000000000",
        "040000002d010000",
        "0700000021050200486914002800",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF SETTEXTCHAREXTRA vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("Hi"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::Text {
                text,
                character_extra,
                ..
            } if text == "Hi" && (*character_extra - 10.0).abs() < 0.01
        )
    }));
    for forbidden in ["wmetafile", "0108", "0521", "fb02", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "WMF SETTEXTCHAREXTRA internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        decoded_pdf_text(&content).contains("Hi"),
        "WMF SETTEXTCHAREXTRA should still render passive PDF text"
    );
    assert!(
        content.operations.iter().any(|operation| {
            operation.operator == "Tc"
                && operation
                    .operands
                    .first()
                    .and_then(pdf_operand_number)
                    .is_some_and(|value| value > 1.0)
        }),
        "WMF SETTEXTCHAREXTRA should emit nonzero passive PDF character spacing"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"0108",
        b"0521",
        b"fb02",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF SETTEXTCHAREXTRA leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_textout_opaque_background_mode_renders_passive_fill_without_payload_leakage() {
    let wmf_hex = concat!(
        "0100090000033100000001000c0000000000",
        "050000000c026400c800",
        "05000000010200ff0000",
        "0400000002010200",
        "0c000000fb02f4ff00000000000000000000000000000000",
        "040000002d010000",
        "0700000021050200486914002800",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF opaque TEXTOUT vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("Hi"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::Text {
                x,
                y,
                height,
                text,
                background_color: Some(background_color),
                ..
            } if (*x - 40.0).abs() < 0.01
                && (*y - 20.0).abs() < 0.01
                && (*height - 12.0).abs() < 0.01
                && text == "Hi"
                && background_color.red == 0
                && background_color.green == 255
                && background_color.blue == 0
        )
    }));
    for forbidden in ["wmetafile", "0102", "0201", "0521", "fb02", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "WMF opaque TEXTOUT internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "opaque TEXTOUT should render passive PDF rectangle path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "f"),
        "opaque TEXTOUT should fill its passive background rectangle"
    );
    assert!(
        decoded_pdf_text(&content).contains("Hi"),
        "opaque TEXTOUT should render passive PDF text"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"0102",
        b"0201",
        b"0521",
        b"fb02",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF opaque TEXTOUT leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_exttextout_uses_selected_font_charset_and_stays_passive() {
    let wmf_hex = concat!(
        "0100090000033200000001000c0000000000",
        "050000000c026400c800",
        "0500000009020000ff00",
        "0c000000fb02f4ff0000000000000000000000ee00000000",
        "040000002d010000",
        "0c000000320a140028000100040030006000100020008c00",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF EXTTEXTOUT vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains('\u{015a}'));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::Text {
                x,
                y,
                height,
                text,
                color: Some(color),
                ..
            } if (*x - 40.0).abs() < 0.01
                && (*y - 20.0).abs() < 0.01
                && (*height - 12.0).abs() < 0.01
                && text == "\u{015a}"
                && color.red == 0
                && color.green == 0
                && color.blue == 255
        )
    }));
    for forbidden in ["wmetafile", "0a32", "fb02", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "WMF EXTTEXTOUT internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        pdf_text_font_names(&content)
            .iter()
            .any(|font| font.as_slice() == b"F1"),
        "WMF EXTTEXTOUT should use a passive built-in font resource"
    );
    assert!(
        pdf_text_bytes_for_font(&content, b"F1").contains(&0xda),
        "WMF EXTTEXTOUT should encode Central European text as passive WinAnsi bytes"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"0a32",
        b"fb02",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF EXTTEXTOUT leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_exttextout_clips_passive_text_without_flag_leakage() {
    let wmf_hex = concat!(
        "010009000003360000000100100000000000",
        "050000000c026400c800",
        "0500000009020000ff00",
        "0c000000fb02f4ff00000000000000000000000000000000",
        "040000002d010000",
        "10000000320a140028000a0004001e002d000a001e0048656c6c6f576f726c64",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF clipped EXTTEXTOUT vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("HelloWorld"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::Text {
                x,
                y,
                text,
                clip_bounds: Some(bounds),
                ..
            } if (*x - 40.0).abs() < 0.01
                && (*y - 20.0).abs() < 0.01
                && text == "HelloWorld"
                && (bounds.left - 30.0).abs() < 0.01
                && (bounds.top - 10.0).abs() < 0.01
                && (bounds.right - 45.0).abs() < 0.01
                && (bounds.bottom - 30.0).abs() < 0.01
        )
    }));
    for forbidden in ["wmetafile", "0a32", "fb02", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "WMF clipped EXTTEXTOUT internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "W"),
        "clipped EXTTEXTOUT should emit a passive PDF clipping path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "n"),
        "clipped EXTTEXTOUT should terminate the clipping path before text"
    );
    assert!(
        decoded_pdf_text(&content).contains("HelloWorld"),
        "clipped EXTTEXTOUT should still render text through passive text operations"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"0a32",
        b"fb02",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF clipped EXTTEXTOUT leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_exttextout_opaque_background_mode_renders_passive_text_fill_without_record_leakage() {
    let wmf_hex = concat!(
        "0100090000032200000001000c0000000000",
        "050000000c026400c800",
        "05000000010200ff0000",
        "0400000002010200",
        "08000000320a14002800020000004869",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF opaque-background-mode EXTTEXTOUT vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("Hi"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::Text {
                x,
                y,
                text,
                background_color: Some(background_color),
                ..
            } if (*x - 40.0).abs() < 0.01
                && (*y - 20.0).abs() < 0.01
                && text == "Hi"
                && background_color.red == 0
                && background_color.green == 255
                && background_color.blue == 0
        )
    }));
    assert!(
        !image
            .vector_commands
            .iter()
            .any(|command| matches!(command, StaticImageVectorCommand::Rectangle { .. })),
        "background-mode EXTTEXTOUT without ETO_OPAQUE should stay attached to the text command"
    );
    for forbidden in ["wmetafile", "0102", "0201", "0a32", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "WMF opaque-background-mode EXTTEXTOUT internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "opaque-background-mode EXTTEXTOUT should emit passive text background rectangle"
    );
    assert!(
        decoded_pdf_text(&content).contains("Hi"),
        "opaque-background-mode EXTTEXTOUT should render passive PDF text"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"0102",
        b"0201",
        b"0a32",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF opaque-background-mode EXTTEXTOUT leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_exttextout_opaque_background_renders_as_passive_fill() {
    let wmf_hex = concat!(
        "0100090000032200000000000c0000000000",
        "050000000c026400c800",
        "05000000010200ff0000",
        "0c000000320a14002800020002001e005a000a001e004869",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF opaque EXTTEXTOUT vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("Hi"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    let rectangle_index = image
        .vector_commands
        .iter()
        .position(|command| {
            matches!(
                command,
                StaticImageVectorCommand::Rectangle {
                    left,
                    top,
                    right,
                    bottom,
                    stroke_color: None,
                    fill_color: Some(color),
                    ..
                } if (*left - 30.0).abs() < 0.01
                    && (*top - 10.0).abs() < 0.01
                    && (*right - 90.0).abs() < 0.01
                    && (*bottom - 30.0).abs() < 0.01
                    && color.red == 0
                    && color.green == 255
                    && color.blue == 0
            )
        })
        .expect("opaque EXTTEXTOUT should emit passive background rectangle");
    let text_index = image
        .vector_commands
        .iter()
        .position(|command| {
            matches!(
                command,
                StaticImageVectorCommand::Text {
                    x,
                    y,
                    text,
                    ..
                } if (*x - 40.0).abs() < 0.01 && (*y - 20.0).abs() < 0.01 && text == "Hi"
            )
        })
        .expect("opaque EXTTEXTOUT should emit passive text");
    assert!(
        rectangle_index < text_index,
        "opaque background should paint before text"
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "opaque EXTTEXTOUT should render passive PDF rectangle path"
    );
    assert!(
        decoded_pdf_text(&content).contains("Hi"),
        "opaque EXTTEXTOUT should render passive PDF text"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"0a32",
        b"0102",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "opaque EXTTEXTOUT leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_settextalign_positions_passive_text_without_flag_leakage() {
    let wmf_hex = concat!(
        "0100090000032c00000001000c0000000000",
        "050000000c026400c800",
        "0c000000fb02f4ff00000000000000000000000000000000",
        "040000002d010000",
        "040000002e011e00",
        "0700000021050200486914006400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF SETTEXTALIGN vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("Hi"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::Text {
                x,
                y,
                height,
                text,
                horizontal_align,
                vertical_align,
                ..
            } if (*x - 100.0).abs() < 0.01
                && (*y - 20.0).abs() < 0.01
                && (*height - 12.0).abs() < 0.01
                && text == "Hi"
                && *horizontal_align == StaticImageTextHorizontalAlign::Center
                && *vertical_align == StaticImageTextVerticalAlign::Baseline
        )
    }));
    for forbidden in ["wmetafile", "012e", "1e00", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "WMF SETTEXTALIGN internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let (x, y) =
        pdf_first_text_position_for_text(&content, "Hi").expect("SETTEXTALIGN text position");
    assert!(
        (120.0..126.0).contains(&x),
        "center alignment should shift text left of the page-space anchor, got {x}"
    );
    assert!(
        y > 300.0,
        "baseline alignment should keep baseline near the anchor, got {y}"
    );
    assert!(
        decoded_pdf_text(&content).contains("Hi"),
        "WMF SETTEXTALIGN text should still render passively"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"012e",
        b"1e00",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF SETTEXTALIGN leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_roundrect_renders_passive_rounded_rectangle_without_payload_leakage() {
    let wmf_hex = concat!(
        "010009000003250000000100070000000000",
        "050000000c026400c800",
        "07000000fc02000000ff00000000",
        "040000002d010000",
        "090000001c06140028005000b4000a001400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF ROUNDRECT vector preview image");

    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::RoundedRectangle {
                left,
                top,
                right,
                bottom,
                corner_width,
                corner_height,
                fill_color: Some(color),
                ..
            } if (*left - 20.0).abs() < 0.01
                && (*top - 10.0).abs() < 0.01
                && (*right - 180.0).abs() < 0.01
                && (*bottom - 80.0).abs() < 0.01
                && (*corner_width - 40.0).abs() < 0.01
                && (*corner_height - 20.0).abs() < 0.01
                && color.red == 0
                && color.green == 255
                && color.blue == 0
        )
    }));
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in ["wmetafile", "061c", "00ff00", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "WMF ROUNDRECT internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "c")
            .count()
            >= 4,
        "WMF ROUNDRECT should render passive rounded-corner Bezier curves"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"061c",
        b"00ff00",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF ROUNDRECT leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn excessive_wmf_polyline_points_become_placeholder_without_payload_leakage() {
    let wmf_hex = concat!(
        "010009000003150000000100050000000000",
        "050000000c026400c800",
        "0400000025038100",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("excessive-point WMF placeholder image");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    assert!(image.vector_commands.is_empty());
    assert!(!parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("WMF picture rendered as bounded passive vector preview")
    }));
    for forbidden in ["wmetafile", "010009", "0325", "8100"] {
        assert!(
            !text.contains(forbidden),
            "excessive-point WMF internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    for forbidden in [
        b"wmetafile".as_slice(),
        b"010009",
        b"0325",
        b"8100",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "excessive-point WMF leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn shape_pib_wmf_picture_renders_passive_vector_preview_without_payload_leakage() {
    let wmf_hex = concat!(
        "0100090000032a0000000100070000000000",
        "050000000c026400c800",
        "07000000fc020000dcdcdc000000",
        "040000002d010000",
        "070000001b045000b4000a001400",
        "0700000018045a00be0014006400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\shp{{\\shpinst{{\\sp{{\\sn pib}}{{\\sv {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}}}}}}}}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("shape PIB WMF vector preview image");
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    assert!(
        image
            .vector_commands
            .iter()
            .any(|command| { matches!(command, StaticImageVectorCommand::Rectangle { .. }) })
    );
    assert!(
        image
            .vector_commands
            .iter()
            .any(|command| { matches!(command, StaticImageVectorCommand::Ellipse { .. }) })
    );
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    assert!(
        !parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("empty picture")),
        "shape PIB WMF bytes must not be stolen by shape-property capture"
    );
    for forbidden in [
        "pib",
        "wmetafile",
        "010009",
        "dcdcdc",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "shape PIB WMF internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "shape PIB WMF rectangle should render as passive PDF path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "c"),
        "shape PIB WMF ellipse should render as passive PDF Bezier path"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"pib",
        b"wmetafile",
        b"010009",
        b"dcdcdc",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "shape PIB WMF vector preview leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_and_other_metafile_picture_formats_are_passive_placeholders_without_payload_leakage() {
    let payload_hex = "4142432f4a6176615363726970742f456d62656464656446696c65";

    for (control, forbidden_control) in [
        ("emfblip", b"emfblip".as_slice()),
        ("pmmetafile1", b"pmmetafile".as_slice()),
        ("macpict", b"macpict".as_slice()),
    ] {
        let input = format!(
            "{{\\rtf1 before {{\\pict\\{control}\\picw100\\pich50\\picwgoal2160\\pichgoal720 {payload_hex}}} after\\par}}"
        )
        .into_bytes();
        let parsed = parse_rtf_bytes(&input).unwrap();
        let text = collect_text(&parsed.document);

        assert!(text.contains("before"));
        assert!(text.contains("after"));
        assert!(
            parsed.document.blocks.iter().any(|block| matches!(
                block,
                Block::Image(image)
                    if image.format == ImageFormat::Placeholder
                        && image.bytes.is_empty()
                        && image.display_width_twips == Some(2160)
                        && image.display_height_twips == Some(720)
            )),
            "unsupported {control} should become a passive image placeholder"
        );
        for forbidden in ["ABC", "JavaScript", "EmbeddedFile"] {
            assert!(
                !text.contains(forbidden),
                "unsupported {control} payload leaked to text: {forbidden}"
            );
        }

        let output = convert_rtf_to_pdf(
            &input,
            &ConvertOptions {
                diagnostics: true,
                ..ConvertOptions::default()
            },
        )
        .unwrap();
        let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
        let page_id = *parsed_pdf.get_pages().values().next().expect("page");
        let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
        let rendered_text = decoded_pdf_text(&content);

        assert!(rendered_text.contains("before"));
        assert!(rendered_text.contains("Image skipped"));
        assert!(rendered_text.contains("after"));
        for forbidden in [
            forbidden_control,
            payload_hex.as_bytes(),
            b"ABC",
            b"/JavaScript",
            b"/EmbeddedFile",
            b"/Subtype /Image",
            b"/Launch",
            b"/OpenAction",
            b"/RichMedia",
        ] {
            assert!(
                !output
                    .pdf
                    .windows(forbidden.len())
                    .any(|window| window == forbidden),
                "unsupported {control} payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn extreme_character_spacing_is_bounded_before_pdf_rendering() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 normal ",
        "\\",
        "expndtw999999 spaced",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("normal"));
    assert!(text.contains("spaced"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("character-spacing.rtf");
    let output_path = dir.path().join("character-spacing.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [b"/JavaScript".as_slice(), b"/EmbeddedFile", b"/Launch"] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn extreme_paragraph_spacing_is_bounded_before_pdf_rendering() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "sb999999",
        "\\",
        "sa999999 spaced",
        "\\",
        "par next",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("spaced"));
    assert!(text.contains("next"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("paragraph-spacing.rtf");
    let output_path = dir.path().join("paragraph-spacing.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"sb999999".as_slice(),
        b"sa999999",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn paragraph_auto_spacing_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "sb0",
        "\\",
        "sbauto",
        "\\",
        "sa0",
        "\\",
        "saauto Auto spaced",
        "\\",
        "par",
        "\\",
        "sbauto0",
        "\\",
        "saauto0 Manual",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Auto spaced"));
    assert!(text.contains("Manual"));
    assert!(!text.contains("sbauto"));
    assert!(!text.contains("saauto"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("paragraph-auto-spacing.rtf");
    let output_path = dir.path().join("paragraph-auto-spacing.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"sbauto".as_slice(),
        b"saauto",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden auto-spacing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn contextual_paragraph_spacing_renders_passively_without_control_leakage() {
    let input =
        br"{\rtf1\sb240\sa360\contextualspace First\par\sb240\sa360\contextualspace Second\par}"
            .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let paragraphs = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(text.contains("First"));
    assert!(text.contains("Second"));
    assert_eq!(paragraphs.len(), 2);
    assert!(paragraphs[0].style.contextual_spacing);
    assert!(paragraphs[1].style.contextual_spacing);
    assert!(!text.contains("contextualspace"));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("First"));
    assert!(rendered_text.contains("Second"));
    for forbidden in [
        b"contextualspace".as_slice(),
        b"sb240",
        b"sa360",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden contextual spacing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn extreme_paragraph_indents_are_bounded_before_pdf_rendering() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "li999999",
        "\\",
        "ri-999999",
        "\\",
        "fi999999 indented",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("indented"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("paragraph-indents.rtf");
    let output_path = dir.path().join("paragraph-indents.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"li999999".as_slice(),
        b"ri-999999",
        b"fi999999",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn soft_line_and_page_breaks_are_stripped_without_forcing_pdf_breaks() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before ",
        "\\",
        "softline soft line ",
        "\\",
        "softpage soft page",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before soft line soft page"));
    assert!(!text.contains("softline"));
    assert!(!text.contains("softpage"));
    assert!(!parsed.document.blocks.iter().any(|block| matches!(
        block,
        open_rtf_converter::model::Block::PageBreak
            | open_rtf_converter::model::Block::ColumnBreak
            | open_rtf_converter::model::Block::SectionBreak
    )));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("soft-breaks.rtf");
    let output_path = dir.path().join("soft-breaks.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 1);
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(decoded_pdf_text(&content).contains("Before soft line soft page"));
    for forbidden in [
        b"softline".as_slice(),
        b"softpage",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden soft break content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn column_section_breaks_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "cols2 First",
        "\\",
        "par",
        "\\",
        "sbkcol",
        "\\",
        "sect Second",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("First"));
    assert!(text.contains("Second"));
    assert!(matches!(
        parsed.document.blocks[1],
        open_rtf_converter::model::Block::ColumnBreak
    ));
    assert!(!parsed.document.blocks.iter().any(|block| matches!(
        block,
        open_rtf_converter::model::Block::SectionBreak
            | open_rtf_converter::model::Block::PageBreak
    )));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("column-section-break.rtf");
    let output_path = dir.path().join("column-section-break.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"sbkcol".as_slice(),
        b"\\sect",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn odd_section_breaks_render_blank_page_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 First",
        "\\",
        "par",
        "\\",
        "sbkodd",
        "\\",
        "sect Second",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("First"));
    assert!(text.contains("Second"));
    assert!(matches!(
        parsed.document.blocks[1],
        open_rtf_converter::model::Block::OddPageSectionBreak
    ));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("odd-section-break.rtf");
    let output_path = dir.path().join("odd-section-break.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();

    assert_eq!(parsed_pdf.get_pages().len(), 3);
    for forbidden in [
        b"sbkodd".as_slice(),
        b"\\sect",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn header_footer_variants_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "titlepg{",
        "\\",
        "headerf First header",
        "\\",
        "par}{",
        "\\",
        "headerl Even header",
        "\\",
        "par}{",
        "\\",
        "headerr Odd header",
        "\\",
        "par}{",
        "\\",
        "footerf First footer",
        "\\",
        "par}{",
        "\\",
        "footerl Even footer",
        "\\",
        "par}{",
        "\\",
        "footerr Odd footer",
        "\\",
        "par}Page one",
        "\\",
        "page Page two",
        "\\",
        "page Page three",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(parsed.document.page.title_page);
    for expected in [
        "First header",
        "Even header",
        "Odd header",
        "First footer",
        "Even footer",
        "Odd footer",
        "Page one",
        "Page two",
        "Page three",
    ] {
        assert!(text.contains(expected), "missing visible text: {expected}");
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("header-footer-variants.rtf");
    let output_path = dir.path().join("header-footer-variants.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();

    assert_eq!(parsed_pdf.get_pages().len(), 3);
    for forbidden in [
        b"headerf".as_slice(),
        b"headerl",
        b"headerr",
        b"footerf",
        b"footerl",
        b"footerr",
        b"titlepg",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn explicit_section_columns_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "cols2",
        "\\",
        "colsx720",
        "\\",
        "linebetcol",
        "\\",
        "colno1",
        "\\",
        "colw1440",
        "\\",
        "colsr360",
        "\\",
        "colno2",
        "\\",
        "colw2880 Left",
        "\\",
        "par",
        "\\",
        "column Right",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Left"));
    assert!(text.contains("Right"));
    assert_eq!(parsed.document.page.column_widths_twips, vec![1440, 2880]);
    assert_eq!(parsed.document.page.column_gaps_twips, vec![360]);
    for forbidden in ["colsx", "linebetcol", "colno", "colw", "colsr"] {
        assert!(
            !text.contains(forbidden),
            "forbidden explicit column control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("explicit-section-columns.rtf");
    let output_path = dir.path().join("explicit-section-columns.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let stroke_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "S")
        .count();

    assert!(rendered_text.contains("Left"));
    assert!(rendered_text.contains("Right"));
    assert!(
        stroke_count >= 1,
        "linebetcol should render a passive column separator stroke"
    );
    for forbidden in [
        b"colsx".as_slice(),
        b"linebetcol",
        b"colno",
        b"colw",
        b"colsr",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden explicit column content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn later_section_title_page_header_footer_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "header Odd header",
        "\\",
        "par}{",
        "\\",
        "headerf First header",
        "\\",
        "par}{",
        "\\",
        "footer Odd footer",
        "\\",
        "par}{",
        "\\",
        "footerf First footer",
        "\\",
        "par}First section",
        "\\",
        "sect",
        "\\",
        "sectd",
        "\\",
        "titlepg Second section",
        "\\",
        "page Second section page two",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let section_title_pages: Vec<_> = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::SectionSettings(settings) => Some(settings.title_page),
            _ => None,
        })
        .collect();
    let text = collect_text(&parsed.document);

    assert_eq!(section_title_pages, vec![true]);
    for expected in [
        "Odd header",
        "First header",
        "Odd footer",
        "First footer",
        "First section",
        "Second section",
    ] {
        assert!(text.contains(expected), "missing visible text: {expected}");
    }
    assert!(!text.contains("titlepg"));
    assert!(!text.contains("sectd"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("later-section-title-page.rtf");
    let output_path = dir.path().join("later-section-title-page.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();

    assert_eq!(parsed_pdf.get_pages().len(), 3);
    for forbidden in [
        b"titlepg".as_slice(),
        b"sectd",
        b"headerf",
        b"footerf",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn later_section_header_footer_content_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "header Document header",
        "\\",
        "par}{",
        "\\",
        "footer Document footer",
        "\\",
        "par}First section",
        "\\",
        "sect",
        "\\",
        "sectd{",
        "\\",
        "header Section header",
        "\\",
        "par}{",
        "\\",
        "footer Section footer",
        "\\",
        "par}Second section",
        "\\",
        "page Second section page two",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let section_headers: Vec<_> = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::SectionSettings(settings) => {
                Some((settings.header.len(), settings.footer.len()))
            }
            _ => None,
        })
        .collect();

    assert_eq!(section_headers, vec![(1, 1)]);
    for expected in [
        "Document header",
        "Document footer",
        "Section header",
        "Section footer",
        "First section",
        "Second section",
    ] {
        assert!(text.contains(expected), "missing visible text: {expected}");
    }
    for forbidden in ["sectd"] {
        assert!(
            !text.contains(forbidden),
            "forbidden control text leaked: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("later-section-header-footer.rtf");
    let output_path = dir.path().join("later-section-header-footer.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();

    assert_eq!(parsed_pdf.get_pages().len(), 3);
    for forbidden in [
        b"sectd".as_slice(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn header_footer_distances_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "headery360",
        "\\",
        "footery1080{",
        "\\",
        "header Header",
        "\\",
        "par}{",
        "\\",
        "footer Footer",
        "\\",
        "par}Body",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert_eq!(parsed.document.page.header_distance_twips, 360);
    assert_eq!(parsed.document.page.footer_distance_twips, 1_080);
    for expected in ["Header", "Footer", "Body"] {
        assert!(text.contains(expected), "missing visible text: {expected}");
    }
    for forbidden in ["headery", "footery"] {
        assert!(
            !text.contains(forbidden),
            "forbidden header/footer distance control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("header-footer-distances.rtf");
    let output_path = dir.path().join("header-footer-distances.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"headery".as_slice(),
        b"footery",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn extreme_character_scaling_is_bounded_before_pdf_rendering() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 normal ",
        "\\",
        "charscalex999999 scaled",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("normal"));
    assert!(text.contains("scaled"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("character-scaling.rtf");
    let output_path = dir.path().join("character-scaling.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [b"/JavaScript".as_slice(), b"/EmbeddedFile", b"/Launch"] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn word_grid_and_outline_metadata_do_not_leak_to_pdf() {
    let input = br"{\rtf1{\stylesheet{\s1\outlinelevel2\cgrid Heading;}}\pard\outlinelevel1\cgrid Visible metadata\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Visible metadata"));
    for forbidden in ["outlinelevel", "cgrid", "stylesheet"] {
        assert!(
            !text.contains(forbidden),
            "forbidden Word metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.contains("Visible metadata"),
        "decoded PDF text did not contain visible metadata text: {rendered_text:?}"
    );
    for forbidden in [
        b"outlinelevel".as_slice(),
        b"cgrid",
        b"stylesheet",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden Word metadata content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn justified_word_spacing_stays_passive_pdf_text_state() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw5000",
        "\\",
        "margl720",
        "\\",
        "margr720",
        "\\",
        "qj one two three four five six seven eight nine ten",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("one"));
    assert!(text.contains("ten"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("justified.rtf");
    let output_path = dir.path().join("justified.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [b"/JavaScript".as_slice(), b"/EmbeddedFile", b"/Launch"] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn distributed_alignment_controls_render_as_passive_justified_text() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw5000",
        "\\",
        "margl720",
        "\\",
        "margr720",
        "\\",
        "qd distributed one two three four five six seven eight",
        "\\",
        "par",
        "\\",
        "qk thai distributed one two three four five six seven eight",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("distributed one"));
    assert!(text.contains("thai distributed"));
    for forbidden in ["qd", "qk"] {
        assert!(
            !text.contains(forbidden),
            "forbidden distributed-alignment control leaked to text: {forbidden}"
        );
    }
    let paragraphs = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            open_rtf_converter::model::Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(paragraphs[0].style.alignment, Alignment::Justified);
    assert_eq!(paragraphs[1].style.alignment, Alignment::Justified);

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("distributed-alignment.rtf");
    let output_path = dir.path().join("distributed-alignment.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"\\qd".as_slice(),
        b"\\qk",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden distributed-alignment content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn paragraph_direction_controls_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw5000",
        "\\",
        "margl720",
        "\\",
        "margr720",
        "\\",
        "rtlpar Right placed",
        "\\",
        "par",
        "\\",
        "ltrpar Left placed",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Right placed"));
    assert!(text.contains("Left placed"));
    for forbidden in ["rtlpar", "ltrpar"] {
        assert!(
            !text.contains(forbidden),
            "forbidden paragraph direction control leaked to text: {forbidden}"
        );
    }
    let paragraphs = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            open_rtf_converter::model::Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(paragraphs[0].style.alignment, Alignment::Right);
    assert_eq!(paragraphs[1].style.alignment, Alignment::Left);

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("paragraph-direction.rtf");
    let output_path = dir.path().join("paragraph-direction.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"rtlpar".as_slice(),
        b"ltrpar",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden paragraph direction content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn character_direction_controls_are_explicit_passive_approximations_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Normal {",
        "\\",
        "rtlch RTL text} {",
        "\\",
        "ltrch LTR text}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Normal RTL text LTR text"));
    for forbidden in ["rtlch", "ltrch"] {
        assert!(
            !text.contains(forbidden),
            "forbidden character direction control leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("right-to-left character direction approximated")
    }));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "character direction controls should not be unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("right-to-left character direction approximated")
    }));

    for forbidden in [
        b"rtlch".as_slice(),
        b"ltrch",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden character direction content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn word_paragraph_indent_aliases_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw6000",
        "\\",
        "margl720",
        "\\",
        "margr720",
        "\\",
        "lin720",
        "\\",
        "rin360",
        "\\",
        "fin-240 Alias indented paragraph",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Alias indented paragraph"));
    for forbidden in ["lin720", "rin360", "fin-240"] {
        assert!(
            !text.contains(forbidden),
            "forbidden indent alias control leaked to text: {forbidden}"
        );
    }
    let open_rtf_converter::model::Block::Paragraph(paragraph) = &parsed.document.blocks[0] else {
        panic!("expected paragraph");
    };
    assert_eq!(paragraph.style.left_indent_twips, 720);
    assert_eq!(paragraph.style.right_indent_twips, 360);
    assert_eq!(paragraph.style.first_line_indent_twips, -240);
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "indent aliases should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("paragraph-indent-aliases.rtf");
    let output_path = dir.path().join("paragraph-indent-aliases.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"lin720".as_slice(),
        b"rin360",
        b"fin-240",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden indent alias content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn widow_control_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "widowctrl",
        "\\",
        "pard Document default",
        "\\",
        "line Carries widow control",
        "\\",
        "par",
        "\\",
        "widctlpar Alpha",
        "\\",
        "line Beta",
        "\\",
        "line Gamma",
        "\\",
        "par",
        "\\",
        "nowidctlpar Plain",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Document default\nCarries widow control"));
    assert!(text.contains("Alpha\nBeta\nGamma"));
    assert!(text.contains("Plain"));
    assert!(!text.contains("widowctrl"));
    assert!(!text.contains("widctlpar"));
    assert!(!text.contains("nowidctlpar"));
    let open_rtf_converter::model::Block::Paragraph(first) = &parsed.document.blocks[0] else {
        panic!("expected first paragraph");
    };
    let open_rtf_converter::model::Block::Paragraph(third) = &parsed.document.blocks[2] else {
        panic!("expected third paragraph");
    };
    assert!(first.style.widow_control);
    assert!(!third.style.widow_control);

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("widow-control.rtf");
    let output_path = dir.path().join("widow-control.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"widowctrl".as_slice(),
        b"widctlpar",
        b"nowidctlpar",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden widow-control content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn no_wrap_controls_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "nowwrap Alpha Beta",
        "\\",
        "par",
        "\\",
        "nowwrap0 Wrapped",
        "\\",
        "par",
        "\\",
        "trowd",
        "\\",
        "clNoWrap",
        "\\",
        "cellx1440 Cell Alpha Beta",
        "\\",
        "cell",
        "\\",
        "cellx2880 Cell wrapped",
        "\\",
        "cell",
        "\\",
        "row}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Alpha Beta"));
    assert!(text.contains("Cell Alpha Beta"));
    for forbidden in ["nowwrap", "clNoWrap"] {
        assert!(
            !text.contains(forbidden),
            "forbidden no-wrap control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("no-wrap.rtf");
    let output_path = dir.path().join("no-wrap.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"nowwrap".as_slice(),
        b"clNoWrap",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden no-wrap content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn page_vertical_alignment_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperh5000",
        "\\",
        "margt720",
        "\\",
        "margb720",
        "\\",
        "vertalc Centered body",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert_eq!(
        parsed.document.page.vertical_alignment,
        PageVerticalAlignment::Center
    );
    assert!(text.contains("Centered body"));
    assert!(!text.contains("vertalc"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("page-vertical-alignment.rtf");
    let output_path = dir.path().join("page-vertical-alignment.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 1);

    for forbidden in [
        b"vertalc".as_slice(),
        b"vertalb",
        b"vertalt",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn page_break_before_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 First page text",
        "\\",
        "par",
        "\\",
        "pagebb Second page text",
        "\\",
        "par",
        "\\",
        "pagebb0 Still second page",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("First page text"));
    assert!(text.contains("Second page text"));
    assert!(text.contains("Still second page"));
    assert!(!text.contains("pagebb"));

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    audit_passive_pdf_bytes(&output.pdf).unwrap();

    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let pages = parsed_pdf.get_pages();
    assert_eq!(
        pages.len(),
        2,
        "page-break-before paragraph should start a second PDF page"
    );
    let page_texts = pages
        .values()
        .map(|page_id| {
            let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
            decoded_pdf_text(&content)
        })
        .collect::<Vec<_>>();
    assert!(page_texts[0].contains("First page text"));
    assert!(!page_texts[0].contains("Second page text"));
    assert!(page_texts[1].contains("Second page text"));
    assert!(page_texts[1].contains("Still second page"));

    for forbidden in [
        b"pagebb".as_slice(),
        b"/OpenAction",
        b"/AcroForm",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden page-break-before content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn page_gutter_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw5760",
        "\\",
        "margl720",
        "\\",
        "margr720",
        "\\",
        "gutter720 Bound text",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert_eq!(parsed.document.page.gutter_twips, 720);
    assert!(text.contains("Bound text"));
    assert!(!text.contains("gutter"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("page-gutter.rtf");
    let output_path = dir.path().join("page-gutter.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(decoded_pdf_text(&content).contains("Bound text"));
    for forbidden in [
        b"gutter".as_slice(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn mirrored_page_gutter_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw5760",
        "\\",
        "margl720",
        "\\",
        "margr360",
        "\\",
        "gutter720",
        "\\",
        "facingp Odd text",
        "\\",
        "page Even text",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(parsed.document.page.mirror_margins);
    assert_eq!(parsed.document.page.gutter_twips, 720);
    assert!(text.contains("Odd text"));
    assert!(text.contains("Even text"));
    assert!(!text.contains("facingp"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("mirrored-page-gutter.rtf");
    let output_path = dir.path().join("mirrored-page-gutter.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 2);

    for forbidden in [
        b"facingp".as_slice(),
        b"gutter",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn right_to_left_gutter_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw5760",
        "\\",
        "margl720",
        "\\",
        "margr360",
        "\\",
        "gutter720",
        "\\",
        "rtlgutter Bound text",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(parsed.document.page.gutter_on_right);
    assert_eq!(parsed.document.page.gutter_twips, 720);
    assert!(text.contains("Bound text"));
    assert!(!text.contains("rtlgutter"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("rtl-gutter.rtf");
    let output_path = dir.path().join("rtl-gutter.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 1);

    for forbidden in [
        b"rtlgutter".as_slice(),
        b"gutter",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn outline_text_stays_passive_pdf_text_state() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 plain ",
        "\\",
        "outl outlined ",
        "\\",
        "outl0 normal",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("plain"));
    assert!(text.contains("outlined"));
    assert!(text.contains("normal"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("outline.rtf");
    let output_path = dir.path().join("outline.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [b"/JavaScript".as_slice(), b"/EmbeddedFile", b"/Launch"] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn shadow_text_stays_passive_pdf_text_state() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 plain ",
        "\\",
        "shad shadow ",
        "\\",
        "shad0 normal",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("plain"));
    assert!(text.contains("shadow"));
    assert!(text.contains("normal"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("shadow.rtf");
    let output_path = dir.path().join("shadow.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [b"/JavaScript".as_slice(), b"/EmbeddedFile", b"/Launch"] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn relief_text_stays_passive_pdf_text_state() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 plain ",
        "\\",
        "embo emboss ",
        "\\",
        "impr engrave ",
        "\\",
        "impr0 normal",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("plain"));
    assert!(text.contains("emboss"));
    assert!(text.contains("engrave"));
    assert!(text.contains("normal"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("relief.rtf");
    let output_path = dir.path().join("relief.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [b"/JavaScript".as_slice(), b"/EmbeddedFile", b"/Launch"] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn small_caps_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 ",
        "\\",
        "scaps Mix",
        "\\",
        "scaps0 normal",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Mix"));
    assert!(text.contains("normal"));
    assert!(!text.contains("scaps"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("small-caps.rtf");
    let output_path = dir.path().join("small-caps.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(decoded_pdf_text(&content).contains("MIX"));
    assert!(content.operations.iter().any(|operation| {
        operation.operator == "Tf" && format!("{:?}", operation.operands).contains("8.5")
    }));
    for forbidden in [
        b"scaps".as_slice(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn superscript_and_subscript_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 base ",
        "\\",
        "super raised ",
        "\\",
        "nosupersub base2 ",
        "\\",
        "sub lowered ",
        "\\",
        "sub0 base3 ",
        "\\",
        "up8 manualup ",
        "\\",
        "dn6 manualdown",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    for expected in [
        "base",
        "raised",
        "base2",
        "lowered",
        "base3",
        "manualup",
        "manualdown",
    ] {
        assert!(text.contains(expected));
    }
    for forbidden in ["super", "sub", "nosupersub", "up8", "dn6"] {
        assert!(
            !text.contains(forbidden),
            "forbidden script-positioning control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("script-positioning.rtf");
    let output_path = dir.path().join("script-positioning.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(decoded_pdf_text(&content).contains("raised"));
    assert!(decoded_pdf_text(&content).contains("lowered"));
    for forbidden in [
        b"super".as_slice(),
        b"nosupersub",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn word_only_underline_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 ",
        "\\",
        "ulw Two words",
        "\\",
        "ulnone plain",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Two words"));
    assert!(text.contains("plain"));
    assert!(!text.contains("ulw"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("word-underline.rtf");
    let output_path = dir.path().join("word-underline.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(decoded_pdf_text(&content).contains("Two words"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "S")
            .count()
            >= 2
    );
    for forbidden in [
        b"ulw".as_slice(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn underline_color_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "colortbl;",
        "\\",
        "red0",
        "\\",
        "green0",
        "\\",
        "blue0;",
        "\\",
        "red255",
        "\\",
        "green0",
        "\\",
        "blue0;} ",
        "\\",
        "ul",
        "\\",
        "ulc2 colored",
        "\\",
        "ulc0 auto",
        "\\",
        "ulnone plain",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("colored"));
    assert!(text.contains("auto"));
    assert!(text.contains("plain"));

    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    let colored = paragraph
        .runs
        .iter()
        .find(|run| run.text.trim() == "colored")
        .expect("colored underline run");
    let auto = paragraph
        .runs
        .iter()
        .find(|run| run.text.trim() == "auto")
        .expect("auto underline run");
    assert_eq!(colored.style.underline, UnderlineStyle::Single);
    assert_eq!(colored.style.underline_color_index, Some(2));
    assert_eq!(auto.style.underline, UnderlineStyle::Single);
    assert_eq!(auto.style.underline_color_index, Some(0));
    for forbidden in ["ulc", "ulnone"] {
        assert!(
            !text.contains(forbidden),
            "forbidden underline color control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("underline-color.rtf");
    let output_path = dir.path().join("underline-color.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(decoded_pdf_text(&content).contains("colored"));
    assert!(content.operations.iter().any(|operation| {
        operation.operator == "RG"
            && operation.operands.len() == 3
            && pdf_operand_number(&operation.operands[0]).is_some_and(|value| value > 0.9)
            && pdf_operand_number(&operation.operands[1]).is_some_and(|value| value < 0.01)
            && pdf_operand_number(&operation.operands[2]).is_some_and(|value| value < 0.01)
    }));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "S")
            .count()
            >= 2
    );
    for forbidden in [
        b"ulc".as_slice(),
        b"ulnone",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden underline color content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn extreme_character_shading_is_bounded_before_pdf_rendering() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "colortbl;",
        "\\",
        "red255",
        "\\",
        "green0",
        "\\",
        "blue0;} ",
        "\\",
        "chshdng999999",
        "\\",
        "chcbpat1 shaded",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("shaded"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("character-shading.rtf");
    let output_path = dir.path().join("character-shading.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [b"/JavaScript".as_slice(), b"/EmbeddedFile", b"/Launch"] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn character_borders_are_bounded_passive_pdf_lines() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "colortbl;",
        "\\",
        "red255",
        "\\",
        "green0",
        "\\",
        "blue0;} ",
        "\\",
        "chbrdr",
        "\\",
        "brdrs",
        "\\",
        "brdrw9999",
        "\\",
        "brdrcf1 bordered",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("bordered"));
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    let bordered = paragraph
        .runs
        .iter()
        .find(|run| run.text.trim() == "bordered")
        .expect("bordered run");
    assert!(bordered.style.border.visible);
    assert_eq!(bordered.style.border.color_index, Some(1));
    assert!(bordered.style.border.width_twips <= RtfLimits::default().max_table_border_width_twips);
    for forbidden in ["chbrdr", "brdrs", "brdrw", "brdrcf"] {
        assert!(
            !text.contains(forbidden),
            "forbidden character border control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("character-border.rtf");
    let output_path = dir.path().join("character-border.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(decoded_pdf_text(&content).contains("bordered"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "S")
            .count()
            >= 4
    );
    assert!(content.operations.iter().any(|operation| {
        operation.operator == "RG"
            && operation.operands.len() == 3
            && pdf_operand_number(&operation.operands[0]).is_some_and(|value| value > 0.9)
            && pdf_operand_number(&operation.operands[1]).is_some_and(|value| value < 0.01)
            && pdf_operand_number(&operation.operands[2]).is_some_and(|value| value < 0.01)
    }));
    for forbidden in [
        b"chbrdr".as_slice(),
        b"brdrs",
        b"brdrw",
        b"brdrcf",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden character border content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn header_and_table_paragraph_shading_borders_render_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "colortbl;",
        "\\",
        "red220",
        "\\",
        "green230",
        "\\",
        "blue240;}",
        "{",
        "\\",
        "header",
        "\\",
        "cbpat1",
        "\\",
        "brdrb",
        "\\",
        "brdrs",
        "\\",
        "brdrw40 Header",
        "\\",
        "par}",
        "\\",
        "trowd",
        "\\",
        "cellx1440",
        "\\",
        "intbl",
        "\\",
        "brdrb",
        "\\",
        "brdrs",
        "\\",
        "brdrw40 Cell",
        "\\",
        "cell",
        "\\",
        "row Body",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Header"));
    assert!(text.contains("Cell"));
    assert!(text.contains("Body"));

    let header = parsed.document.header.first().expect("header paragraph");
    assert_eq!(header.style.shading_color_index, Some(1));
    assert!(header.style.borders.bottom.visible);
    assert_eq!(header.style.borders.bottom.width_twips, 40);
    let table = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("table");
    let cell_paragraph = &table.rows[0].cells[0].paragraphs[0];
    assert!(cell_paragraph.style.borders.bottom.visible);
    assert_eq!(cell_paragraph.style.borders.bottom.width_twips, 40);
    for forbidden in ["cbpat", "brdrb", "brdrs", "brdrw", "trowd", "cellx"] {
        assert!(
            !text.contains(forbidden),
            "forbidden header/table paragraph decoration control leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Header"));
    assert!(rendered_text.contains("Cell"));
    assert!(rendered_text.contains("Body"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "f" || operation.operator == "f*"),
        "header shading should render passive fill"
    );
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "S")
            .count()
            >= 2,
        "header and table paragraph borders should render passive strokes"
    );
    for forbidden in [
        b"cbpat".as_slice(),
        b"brdrb",
        b"brdrs",
        b"brdrw",
        b"trowd",
        b"cellx",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden header/table paragraph decoration content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn styled_borders_stay_passive_pdf_strokes() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 ",
        "\\",
        "brdrb",
        "\\",
        "brdrdb double paragraph",
        "\\",
        "par ",
        "\\",
        "pard",
        "\\",
        "chbrdr",
        "\\",
        "brdrdash dashed character",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("double paragraph"));
    assert!(text.contains("dashed character"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("styled-borders.rtf");
    let output_path = dir.path().join("styled-borders.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [b"/JavaScript".as_slice(), b"/EmbeddedFile", b"/Launch"] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn border_spacing_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 ",
        "\\",
        "box",
        "\\",
        "brdrs",
        "\\",
        "brsp240 spaced paragraph",
        "\\",
        "par ",
        "\\",
        "pard",
        "\\",
        "chbrdr",
        "\\",
        "brdrs",
        "\\",
        "brsp120 spaced character",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("spaced paragraph"));
    assert!(text.contains("spaced character"));
    for forbidden in ["brsp", "chbrdr"] {
        assert!(
            !text.contains(forbidden),
            "forbidden border spacing control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("border-spacing.rtf");
    let output_path = dir.path().join("border-spacing.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"brsp".as_slice(),
        b"chbrdr",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden border spacing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn extended_word_borders_stay_passive_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 ",
        "\\",
        "box",
        "\\",
        "brdrhair hairline paragraph",
        "\\",
        "par ",
        "\\",
        "pard",
        "\\",
        "brdrb",
        "\\",
        "brdrdashdot dashed paragraph",
        "\\",
        "par ",
        "\\",
        "pard",
        "\\",
        "brdrt",
        "\\",
        "brdrwavy wavy paragraph",
        "\\",
        "par ",
        "\\",
        "pard",
        "\\",
        "brdrl",
        "\\",
        "brdrtriple triple paragraph",
        "\\",
        "par ",
        "\\",
        "pard",
        "\\",
        "brdrr",
        "\\",
        "brdrinset inset paragraph",
        "\\",
        "par ",
        "\\",
        "pard",
        "\\",
        "brdrb",
        "\\",
        "brdrs",
        "\\",
        "brdrsh shadow paragraph",
        "\\",
        "par ",
        "\\",
        "trowd",
        "\\",
        "clbrdrl",
        "\\",
        "brdrdashdd",
        "\\",
        "cellx1440 cell border",
        "\\",
        "cell",
        "\\",
        "row}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("hairline paragraph"));
    assert!(text.contains("dashed paragraph"));
    assert!(text.contains("wavy paragraph"));
    assert!(text.contains("triple paragraph"));
    assert!(text.contains("inset paragraph"));
    assert!(text.contains("shadow paragraph"));
    assert!(text.contains("cell border"));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "extended Word border variants should not be unsupported: {:?}",
        parsed.diagnostics
    );
    for expected in [
        "Word border style \\brdrtriple approximated as passive double border",
        "Word border style \\brdrinset approximated as passive single border",
        "Word border effect \\brdrsh flattened for passive static PDF output",
    ] {
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "missing diagnostic: {expected}; diagnostics were {:?}",
            parsed.diagnostics
        );
    }
    for forbidden in [
        "brdrhair",
        "brdrdashdot",
        "brdrwavy",
        "brdrtriple",
        "brdrinset",
        "brdrsh",
        "brdrdashdd",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden extended border control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("extended-word-borders.rtf");
    let output_path = dir.path().join("extended-word-borders.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"brdrhair".as_slice(),
        b"brdrdashdot",
        b"brdrwavy",
        b"brdrtriple",
        b"brdrinset",
        b"brdrsh",
        b"brdrdashdd",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden extended border content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn paragraph_bar_border_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "colortbl;",
        "\\",
        "red255",
        "\\",
        "green0",
        "\\",
        "blue0;}",
        "\\",
        "brdrbar",
        "\\",
        "brdrs",
        "\\",
        "brdrw80",
        "\\",
        "brdrcf1 barred paragraph",
        "\\",
        "par ",
        "\\",
        "pard plain paragraph",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("barred paragraph"));
    assert!(text.contains("plain paragraph"));
    for forbidden in ["brdrbar", "brdrw", "brdrcf"] {
        assert!(
            !text.contains(forbidden),
            "forbidden paragraph bar border control leaked to text: {forbidden}"
        );
    }

    let paragraphs = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            open_rtf_converter::model::Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();
    let barred = paragraphs
        .iter()
        .find(|paragraph| {
            paragraph
                .runs
                .iter()
                .any(|run| run.text.contains("barred paragraph"))
        })
        .expect("barred paragraph");
    let plain = paragraphs
        .iter()
        .find(|paragraph| {
            paragraph
                .runs
                .iter()
                .any(|run| run.text.contains("plain paragraph"))
        })
        .expect("plain paragraph");
    assert!(barred.style.borders.left.visible);
    assert_eq!(barred.style.borders.left.width_twips, 80);
    assert_eq!(barred.style.borders.left.color_index, Some(1));
    assert!(!plain.style.borders.left.visible);

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("paragraph-bar-border.rtf");
    let output_path = dir.path().join("paragraph-bar-border.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"brdrbar".as_slice(),
        b"brdrw",
        b"brdrcf",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden paragraph bar border content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn paragraph_between_borders_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "colortbl;",
        "\\",
        "red255",
        "\\",
        "green0",
        "\\",
        "blue0;}",
        "\\",
        "brdrbtw",
        "\\",
        "brdrs",
        "\\",
        "brdrw60",
        "\\",
        "brdrcf1 first paragraph",
        "\\",
        "par ",
        "\\",
        "brdrbtw",
        "\\",
        "brdrs",
        "\\",
        "brdrw60",
        "\\",
        "brdrcf1 second paragraph",
        "\\",
        "par} ",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("first paragraph"));
    assert!(text.contains("second paragraph"));
    for forbidden in ["brdrbtw", "brdrw", "brdrcf"] {
        assert!(
            !text.contains(forbidden),
            "forbidden paragraph between border control leaked to text: {forbidden}"
        );
    }

    let paragraphs = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(paragraphs.len() >= 2);
    for paragraph in paragraphs.iter().take(2) {
        assert!(paragraph.style.borders.between.visible);
        assert_eq!(paragraph.style.borders.between.width_twips, 60);
        assert_eq!(paragraph.style.borders.between.color_index, Some(1));
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("paragraph-between-border.rtf");
    let output_path = dir.path().join("paragraph-between-border.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("first paragraph"));
    assert!(rendered_text.contains("second paragraph"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "S")
            .count()
            >= 1
    );
    for forbidden in [
        b"brdrbtw".as_slice(),
        b"brdrs",
        b"brdrw",
        b"brdrcf",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden paragraph between border content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn font_code_page_hex_escapes_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "ansicpg1252{",
        "\\",
        "fonttbl{",
        "\\",
        "f0",
        "\\",
        "cpg437 Terminal;}{",
        "\\",
        "f1",
        "\\",
        "cpg10000 Mac Face;}}",
        "\\",
        "f0 cafe ",
        "\\",
        "'82 ",
        "\\",
        "f1 quote ",
        "\\",
        "'d2Hello",
        "\\",
        "'d3",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("cafe \u{00e9}"));
    assert!(text.contains("quote \u{201c}Hello\u{201d}"));
    for forbidden in ["cpg", "Terminal", "Mac Face"] {
        assert!(
            !text.contains(forbidden),
            "font code-page metadata leaked into text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("font-code-pages.rtf");
    let output_path = dir.path().join("font-code-pages.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"cpg".as_slice(),
        b"Terminal",
        b"Mac Face",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn font_charset_hex_escapes_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "ansicpg1252{",
        "\\",
        "fonttbl{",
        "\\",
        "f0",
        "\\",
        "fcharset77 Mac Face;}{",
        "\\",
        "f1",
        "\\",
        "fcharset255 Oem Face;}}",
        "\\",
        "f0 quote ",
        "\\",
        "'d2Hello",
        "\\",
        "'d3 ",
        "\\",
        "f1 line ",
        "\\",
        "'b3",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("quote \u{201c}Hello\u{201d}"));
    assert!(text.contains("line \u{2502}"));
    for forbidden in ["fcharset", "Mac Face", "Oem Face"] {
        assert!(
            !text.contains(forbidden),
            "font charset metadata leaked into text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("font-charset-code-pages.rtf");
    let output_path = dir.path().join("font-charset-code-pages.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"fcharset".as_slice(),
        b"Mac Face",
        b"Oem Face",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn central_european_font_charset_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "ansicpg1252{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Times New Roman;}{",
        "\\",
        "f37",
        "\\",
        "fcharset238 Times New Roman CE;}}",
        "\\",
        "f37 ",
        "\\",
        "'f6t ",
        "\\",
        "'e1rv",
        "\\",
        "'edzt",
        "\\",
        "'fb",
        "r",
        "\\",
        "'f5 ",
        "\\",
        "'fctvef",
        "\\",
        "'fa",
        "r",
        "\\",
        "'f3g",
        "\\",
        "'e9p",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains(
        "\u{00f6}t \u{00e1}rv\u{00ed}zt\u{0171}r\u{0151} \u{00fc}tvef\u{00fa}r\u{00f3}g\u{00e9}p"
    ));
    for forbidden in ["fcharset", "Times New Roman CE"] {
        assert!(
            !text.contains(forbidden),
            "Central European font metadata leaked into text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let times_bytes = pdf_text_bytes_for_font(&content, b"F9");

    assert!(
        times_bytes
            .windows([0x90, b'r', 0x8d].len())
            .any(|window| window == [0x90, b'r', 0x8d]),
        "Hungarian double-acute glyphs should use passive extended Latin bytes, got {times_bytes:?}"
    );
    for forbidden in [
        b"fcharset".as_slice(),
        b"Times New Roman CE",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn greek_font_charset_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "ansicpg1252{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Times New Roman;}{",
        "\\",
        "f40",
        "\\",
        "fcharset161 Times New Roman Greek;}}",
        "\\",
        "f40 Greek ",
        "\\",
        "'c1",
        "\\",
        "'e1 ",
        "\\",
        "'d0",
        "\\",
        "'f0 ",
        "\\",
        "'d9",
        "\\",
        "'f9",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Greek \u{0391}\u{03b1} \u{03a0}\u{03c0} \u{03a9}\u{03c9}"));
    for forbidden in ["fcharset", "Times New Roman Greek"] {
        assert!(
            !text.contains(forbidden),
            "Greek font metadata leaked into text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let symbol_bytes = pdf_text_bytes_for_font(&content, b"F13");

    assert!(
        symbol_bytes
            .windows(b"AaPpWw".len())
            .any(|window| window == b"AaPpWw"),
        "Greek text should render through passive Symbol bytes, got {symbol_bytes:?}"
    );
    for forbidden in [
        b"fcharset".as_slice(),
        b"Times New Roman Greek",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn turkish_font_charset_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "ansicpg1252{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Times New Roman;}{",
        "\\",
        "f41",
        "\\",
        "fcharset162 Times New Roman Tur;}}",
        "\\",
        "f41 Turkish ",
        "\\",
        "'d0",
        "\\",
        "'dd",
        "\\",
        "'de ",
        "\\",
        "'f0",
        "\\",
        "'fd",
        "\\",
        "'fe",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Turkish \u{011e}\u{0130}\u{015e} \u{011f}\u{0131}\u{015f}"));
    for forbidden in ["fcharset", "Times New Roman Tur"] {
        assert!(
            !text.contains(forbidden),
            "Turkish font metadata leaked into text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let times_bytes = pdf_text_bytes_for_font(&content, b"F9");

    assert!(
        times_bytes
            .windows([0xd0, 0xdd, 0xde, b' ', 0xf0, 0xfd, 0xfe].len())
            .any(|window| window == [0xd0, 0xdd, 0xde, b' ', 0xf0, 0xfd, 0xfe]),
        "Turkish glyphs should use passive extended Latin bytes, got {times_bytes:?}"
    );
    for forbidden in [
        b"fcharset".as_slice(),
        b"Times New Roman Tur",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn baltic_font_charset_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "ansicpg1252{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Times New Roman;}{",
        "\\",
        "f42",
        "\\",
        "fcharset186 Times New Roman Baltic;}}",
        "\\",
        "f42 Baltic ",
        "\\",
        "'c0",
        "\\",
        "'e0 ",
        "\\",
        "'c8",
        "\\",
        "'e8 ",
        "\\",
        "'d8",
        "\\",
        "'f8 ",
        "\\",
        "'da",
        "\\",
        "'fa ",
        "\\",
        "'dd",
        "\\",
        "'fd",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains(
        "Baltic \u{0104}\u{0105} \u{010c}\u{010d} \u{0172}\u{0173} \u{015a}\u{015b} \u{017b}\u{017c}"
    ));
    for forbidden in ["fcharset", "Times New Roman Baltic"] {
        assert!(
            !text.contains(forbidden),
            "Baltic font metadata leaked into text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let times_bytes = pdf_text_bytes_for_font(&content, b"F9");

    assert!(
        times_bytes
            .windows(
                [
                    0xc0, 0xe0, b' ', 0xc8, 0xe8, b' ', 0xd8, 0xf8, b' ', 0xda, 0xfa, b' ', 0x8c,
                    0x9c,
                ]
                .len()
            )
            .any(|window| window
                == [
                    0xc0, 0xe0, b' ', 0xc8, 0xe8, b' ', 0xd8, 0xf8, b' ', 0xda, 0xfa, b' ', 0x8c,
                    0x9c,
                ]),
        "Baltic glyphs should use passive extended Latin bytes, got {times_bytes:?}"
    );
    for forbidden in [
        b"fcharset".as_slice(),
        b"Times New Roman Baltic",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn ansi_font_charset_hex_escapes_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "ansicpg437{",
        "\\",
        "fonttbl{",
        "\\",
        "f0",
        "\\",
        "fcharset0 Ansi Face;}}",
        "\\",
        "f0 quote ",
        "\\",
        "'93Hello",
        "\\",
        "'94 dash ",
        "\\",
        "'97",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("quote \u{201c}Hello\u{201d} dash \u{2014}"));
    for forbidden in ["fcharset", "Ansi Face"] {
        assert!(
            !text.contains(forbidden),
            "ANSI font charset metadata leaked into text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("ansi-font-charset.rtf");
    let output_path = dir.path().join("ansi-font-charset.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"fcharset".as_slice(),
        b"Ansi Face",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn page_borders_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 ",
        "\\",
        "pgbrdrt",
        "\\",
        "brdrdb",
        "\\",
        "brdrw9999",
        "\\",
        "brsp9999",
        "\\",
        "pgbrdrl",
        "\\",
        "brdrdash Body",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(parsed.document.page.page_borders.top.visible);
    assert!(parsed.document.page.page_borders.left.visible);
    assert!(text.contains("Body"));
    for forbidden in [
        "pgbrdrt",
        "pgbrdrl",
        "brdrdb",
        "brdrw9999",
        "brsp9999",
        "brdrcf",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden page border control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("page-borders.rtf");
    let output_path = dir.path().join("page-borders.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"pgbrdrt".as_slice(),
        b"pgbrdrl",
        b"brdrdb",
        b"brdrw9999",
        b"brsp9999",
        b"brdrcf",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden page border control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn page_border_header_footer_scope_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 ",
        "\\",
        "headery360",
        "\\",
        "footery360",
        "\\",
        "pgbrdrt",
        "\\",
        "brdrs",
        "\\",
        "pgbrdrhead",
        "\\",
        "pgbrdrb",
        "\\",
        "brdrs",
        "\\",
        "pgbrdrfoot Body",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(parsed.document.page.page_borders.top.visible);
    assert!(parsed.document.page.page_borders.bottom.visible);
    assert!(parsed.document.page.page_border_includes_header);
    assert!(parsed.document.page.page_border_includes_footer);
    assert!(text.contains("Body"));
    for forbidden in ["pgbrdrhead", "pgbrdrfoot", "pgbrdrt", "pgbrdrb"] {
        assert!(
            !text.contains(forbidden),
            "forbidden page border scope control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("page-border-header-footer.rtf");
    let output_path = dir.path().join("page-border-header-footer.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"pgbrdrhead".as_slice(),
        b"pgbrdrfoot",
        b"pgbrdrt",
        b"pgbrdrb",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden page border scope control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn drop_cap_controls_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "dropcapli3",
        "\\",
        "dropcapt1 Dropped paragraph",
        "\\",
        "par",
        "\\",
        "pard Plain paragraph",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let paragraphs = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(text.contains("Dropped paragraph"));
    assert!(text.contains("Plain paragraph"));
    assert_eq!(paragraphs[0].style.drop_cap_lines, 3);
    assert_eq!(paragraphs[1].style.drop_cap_lines, 0);
    for forbidden in ["dropcapli", "dropcapt"] {
        assert!(
            !text.contains(forbidden),
            "forbidden drop-cap control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("drop-cap.rtf");
    let output_path = dir.path().join("drop-cap.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Dropped paragraph"));
    assert!(rendered_text.contains("Plain paragraph"));
    for forbidden in [
        b"dropcapli".as_slice(),
        b"dropcapt",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden drop-cap content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn page_border_reference_mode_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 ",
        "\\",
        "pgbrdropt",
        "\\",
        "pgbrdrt",
        "\\",
        "brdrs",
        "\\",
        "pgbrdrl",
        "\\",
        "brdrs Body",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(parsed.document.page.page_border_from_page_edge);
    assert!(parsed.document.page.page_borders.top.visible);
    assert!(parsed.document.page.page_borders.left.visible);
    assert!(text.contains("Body"));
    for forbidden in ["pgbrdropt", "pgbrdrt", "pgbrdrl"] {
        assert!(
            !text.contains(forbidden),
            "forbidden page border reference control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("page-border-reference-mode.rtf");
    let output_path = dir.path().join("page-border-reference-mode.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"pgbrdropt".as_slice(),
        b"pgbrdrt",
        b"pgbrdrl",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden page border reference control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn footnotes_strip_active_content_without_losing_safe_text() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Body",
        "\\",
        "chftn{",
        "\\",
        "footnote safe footnote {",
        "\\",
        "object",
        "\\",
        "objdata ",
        payload_hex(),
        "} text",
        "\\",
        "par}}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Body1"));
    assert!(text.contains("safe footnote"));
    assert!(text.contains("text"));
    assert!(text.contains("[Embedded object removed]"));
    assert!(!text.contains(payload_hex()));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("footnote.rtf");
    let output_path = dir.path().join("footnote.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let stroke_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "S")
        .count();

    assert!(rendered_text.contains("Body1"));
    assert!(rendered_text.contains("safe footnote"));
    assert!(rendered_text.contains("text"));
    assert!(
        stroke_count >= 1,
        "footnote separator should render as passive line art"
    );
    for forbidden in [
        payload_hex().as_bytes(),
        b"chftn",
        b"objdata",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/AcroForm",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden footnote content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn bottom_footnote_placement_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ftnbj Body",
        "\\",
        "chftn{",
        "\\",
        "footnote Bottom footnote {",
        "\\",
        "object",
        "\\",
        "objdata ",
        payload_hex(),
        "} text",
        "\\",
        "par}}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Body1"));
    assert!(text.contains("Bottom footnote"));
    assert!(text.contains("text"));
    for forbidden in ["ftnbj", "chftn", "objdata"] {
        assert!(
            !text.contains(forbidden),
            "forbidden bottom-footnote control leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("footnotes placed at passive page bottom")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Body1"));
    assert!(rendered_text.contains("1. Bottom footnote"));
    assert!(rendered_text.contains("text"));
    for forbidden in [
        payload_hex().as_bytes(),
        b"ftnbj",
        b"chftn",
        b"objdata",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/AcroForm",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden bottom-footnote content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn endnotes_strip_active_content_without_losing_safe_text_or_pdf_passivity() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Body",
        "\\",
        "chftn{",
        "\\",
        "endnote safe endnote {",
        "\\",
        "object",
        "\\",
        "objdata ",
        payload_hex(),
        "} text",
        "\\",
        "par}}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Body1"));
    assert!(text.contains("safe endnote"));
    assert!(text.contains("text"));
    assert!(text.contains("[Embedded object removed]"));
    assert!(!text.contains(payload_hex()));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("endnote.rtf");
    let output_path = dir.path().join("endnote.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let stroke_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "S")
        .count();

    assert!(rendered_text.contains("Body1"));
    assert!(rendered_text.contains("safe endnote"));
    assert!(rendered_text.contains("text"));
    assert!(
        stroke_count >= 1,
        "endnote separator should render as passive line art"
    );
    for forbidden in [
        payload_hex().as_bytes(),
        b"chftn",
        b"objdata",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/AcroForm",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden endnote content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn endnotes_at_end_of_document_render_on_passive_final_page_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "aenddoc First page",
        "\\",
        "page Last page",
        "\\",
        "chftn{",
        "\\",
        "endnote Final endnote",
        "\\",
        "par}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert_eq!(
        parsed.document.endnote_placement,
        EndnotePlacement::EndOfDocument
    );
    assert!(text.contains("First page"));
    assert!(text.contains("Last page1"));
    assert!(text.contains("Final endnote"));
    for forbidden in ["aenddoc", "chftn"] {
        assert!(
            !text.contains(forbidden),
            "endnote placement control leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let pages = parsed_pdf.get_pages();
    let page_texts = pages
        .values()
        .map(|page_id| {
            parsed_pdf
                .get_and_decode_page_content(*page_id)
                .map(|content| decoded_pdf_text(&content))
                .unwrap()
        })
        .collect::<Vec<_>>();

    assert_eq!(page_texts.len(), 3);
    assert!(page_texts[0].contains("First page"));
    assert!(page_texts[1].contains("Last page1"));
    assert!(!page_texts[1].contains("Final endnote"));
    assert!(page_texts[2].contains("1. Final endnote"));
    for forbidden in [
        b"aenddoc".as_slice(),
        b"chftn",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/AcroForm",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden endnote placement content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn endnotes_at_end_of_section_render_before_next_section_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "endnhere First section",
        "\\",
        "chftn{",
        "\\",
        "endnote Section note",
        "\\",
        "par}",
        "\\",
        "sect Second section",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert_eq!(
        parsed.document.endnote_placement,
        EndnotePlacement::EndOfSection
    );
    assert_eq!(parsed.document.endnotes.len(), 1);
    assert_eq!(parsed.document.endnote_section_indices, vec![1]);
    assert!(text.contains("First section1"));
    assert!(text.contains("Second section"));
    assert!(text.contains("Section note"));
    for forbidden in ["endnhere", "chftn"] {
        assert!(
            !text.contains(forbidden),
            "endnote section control leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let pages = parsed_pdf.get_pages();
    let page_texts = pages
        .values()
        .map(|page_id| {
            parsed_pdf
                .get_and_decode_page_content(*page_id)
                .map(|content| decoded_pdf_text(&content))
                .unwrap()
        })
        .collect::<Vec<_>>();

    assert_eq!(page_texts.len(), 2);
    assert!(page_texts[0].contains("First section1"));
    assert!(page_texts[0].contains("1. Section note"));
    assert!(page_texts[1].contains("Second section"));
    assert!(!page_texts[1].contains("Section note"));
    for forbidden in [
        b"endnhere".as_slice(),
        b"chftn",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/AcroForm",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden endnote section content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn mixed_endnote_placements_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "aenddoc First section",
        "\\",
        "chftn{",
        "\\",
        "endnote Final note",
        "\\",
        "par}",
        "\\",
        "sect",
        "\\",
        "sectd",
        "\\",
        "endnhere Second section",
        "\\",
        "chftn{",
        "\\",
        "endnote Section note text",
        "\\",
        "par}",
        "\\",
        "sect Third section",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert_eq!(parsed.document.endnotes.len(), 2);
    assert_eq!(parsed.document.endnote_section_indices, vec![1, 2]);
    assert_eq!(
        parsed.document.endnote_placements,
        vec![
            EndnotePlacement::EndOfDocument,
            EndnotePlacement::EndOfSection
        ]
    );
    assert!(text.contains("First section1"), "text: {text:?}");
    assert!(text.contains("Second section2"), "text: {text:?}");
    assert!(text.contains("Third section"), "text: {text:?}");
    assert!(text.contains("Final note"));
    assert!(text.contains("Section note"));
    for forbidden in ["aenddoc", "endnhere", "chftn"] {
        assert!(
            !text.contains(forbidden),
            "mixed endnote placement control leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let pages = parsed_pdf.get_pages();
    let page_texts = pages
        .values()
        .map(|page_id| {
            parsed_pdf
                .get_and_decode_page_content(*page_id)
                .map(|content| decoded_pdf_text(&content))
                .unwrap()
        })
        .collect::<Vec<_>>();

    assert!(
        page_texts
            .iter()
            .any(|text| text.contains("Second section2") && text.contains("2. Section note")),
        "section endnote should render at its passive section boundary: {page_texts:?}"
    );
    assert!(
        page_texts
            .last()
            .is_some_and(|text| text.contains("1. Final note") && !text.contains("Section note")),
        "final-page endnote should remain on the passive final page: {page_texts:?}"
    );
    for forbidden in [
        b"aenddoc".as_slice(),
        b"endnhere",
        b"chftn",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/AcroForm",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden mixed endnote placement content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn fet1_legacy_footnote_groups_render_as_passive_endnotes_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "fet1",
        "\\",
        "aenddoc Body",
        "\\",
        "chftn{",
        "\\",
        "footnote ",
        "\\",
        "chftn Legacy endnote",
        "\\",
        "par}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(parsed.document.footnotes.is_empty());
    assert_eq!(parsed.document.endnotes.len(), 1);
    assert_eq!(parsed.document.endnote_section_indices, vec![1]);
    assert!(text.contains("Body1"));
    assert!(text.contains("Legacy endnote"));
    for forbidden in ["fet1", "aenddoc", "chftn", "footnote"] {
        assert!(
            !text.contains(forbidden),
            "forbidden note-type control leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("passive endnote-only interpretation")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let pages = parsed_pdf.get_pages();
    let page_texts = pages
        .values()
        .map(|page_id| {
            parsed_pdf
                .get_and_decode_page_content(*page_id)
                .map(|content| decoded_pdf_text(&content))
                .unwrap()
        })
        .collect::<Vec<_>>();

    assert_eq!(page_texts.len(), 2);
    assert!(page_texts[0].contains("Body1"));
    assert!(!page_texts[0].contains("Legacy endnote"));
    assert!(page_texts[1].contains("1. Legacy endnote"));
    for forbidden in [
        b"fet1".as_slice(),
        b"aenddoc",
        b"chftn",
        b"footnote",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/AcroForm",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden note-type content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn note_separator_definitions_do_not_reach_text_or_pdf() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "ftnsep Hidden footnote separator {",
        "\\",
        "object",
        "\\",
        "objdata ",
        payload_hex(),
        "}",
        "\\",
        "par}{",
        "\\",
        "ftnsepc Hidden footnote continuation}{",
        "\\",
        "aftnsep Hidden endnote separator}{",
        "\\",
        "aftnsepc Hidden endnote continuation}Body",
        "\\",
        "chftn{",
        "\\",
        "footnote Footnote text",
        "\\",
        "par} End",
        "\\",
        "chftn{",
        "\\",
        "endnote Endnote text",
        "\\",
        "par}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Body1 End1"));
    assert!(text.contains("Footnote text"));
    assert!(text.contains("Endnote text"));
    for forbidden in [
        "Hidden footnote separator",
        "Hidden footnote continuation",
        "Hidden endnote separator",
        "Hidden endnote continuation",
        "Embedded object removed",
        "ftnsep",
        "ftnsepc",
        "aftnsep",
        "aftnsepc",
        "objdata",
        payload_hex(),
    ] {
        assert!(
            !text.contains(forbidden),
            "note separator definition leaked into text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("note-separators.rtf");
    let output_path = dir.path().join("note-separators.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Body1 End1"));
    assert!(rendered_text.contains("Footnote text"));
    assert!(rendered_text.contains("Endnote text"));
    for forbidden in [
        b"Hidden footnote separator".as_slice(),
        b"Hidden footnote continuation",
        b"Hidden endnote separator",
        b"Hidden endnote continuation",
        b"ftnsep",
        b"ftnsepc",
        b"aftnsep",
        b"aftnsepc",
        b"objdata",
        payload_hex().as_bytes(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "note separator definition leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn note_numbering_controls_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ftnstart4",
        "\\",
        "ftnnruc",
        "\\",
        "aftnstart2",
        "\\",
        "aftnnalc Body",
        "\\",
        "chftn{",
        "\\",
        "footnote ",
        "\\",
        "chftn Footnote text",
        "\\",
        "par} End",
        "\\",
        "chftn{",
        "\\",
        "endnote ",
        "\\",
        "chftn Endnote text",
        "\\",
        "par}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("BodyIV Endb"), "text: {text:?}");
    assert!(text.contains("Footnote text"));
    assert!(text.contains("Endnote text"));
    for forbidden in ["ftnstart", "ftnnruc", "aftnstart", "aftnnalc", "chftn"] {
        assert!(
            !text.contains(forbidden),
            "note numbering control leaked into text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("note-numbering.rtf");
    let output_path = dir.path().join("note-numbering.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.contains("BodyIV Endb"),
        "rendered text: {rendered_text:?}"
    );
    assert!(rendered_text.contains("IV. Footnote text"));
    assert!(rendered_text.contains("b. Endnote text"));
    for forbidden in [
        b"ftnstart".as_slice(),
        b"ftnnruc",
        b"aftnstart",
        b"aftnnalc",
        b"chftn",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "note numbering control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn note_restart_controls_warn_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ftnrestart",
        "\\",
        "ftnrstpg",
        "\\",
        "ftnrstcont",
        "\\",
        "ftnstart4",
        "\\",
        "ftnnruc",
        "\\",
        "aenddoc",
        "\\",
        "aftnrestart",
        "\\",
        "aftnrstpg",
        "\\",
        "aftnrstcont",
        "\\",
        "aftnstart2",
        "\\",
        "aftnnalc Body",
        "\\",
        "chftn{",
        "\\",
        "footnote ",
        "\\",
        "chftn First footnote",
        "\\",
        "par} Middle",
        "\\",
        "chftn{",
        "\\",
        "footnote ",
        "\\",
        "chftn Second footnote",
        "\\",
        "par} End",
        "\\",
        "chftn{",
        "\\",
        "endnote ",
        "\\",
        "chftn Endnote text",
        "\\",
        "par}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("BodyIV MiddleV Endb"), "text: {text:?}");
    assert!(text.contains("First footnote"));
    assert!(text.contains("Second footnote"));
    assert!(text.contains("Endnote text"));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "note restart controls should not be unsupported: {:?}",
        parsed.diagnostics
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("footnote restart behavior approximated by passive sequential numbering")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("endnote restart behavior approximated by passive sequential numbering")
    }));
    for forbidden in [
        "ftnrestart",
        "ftnrstpg",
        "ftnrstcont",
        "aftnrestart",
        "aftnrstpg",
        "aftnrstcont",
        "ftnstart",
        "aftnstart",
        "chftn",
    ] {
        assert!(
            !text.contains(forbidden),
            "note restart control leaked into text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let rendered_text = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| {
            parsed_pdf
                .get_and_decode_page_content(*page_id)
                .map(|content| decoded_pdf_text(&content))
                .unwrap()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        rendered_text.contains("BodyIV MiddleV Endb"),
        "rendered text: {rendered_text:?}"
    );
    assert!(rendered_text.contains("IV. First footnote"));
    assert!(rendered_text.contains("V. Second footnote"));
    assert!(
        rendered_text.contains("b. Endnote text"),
        "rendered text: {rendered_text:?}"
    );
    for forbidden in [
        b"ftnrestart".as_slice(),
        b"ftnrstpg",
        b"ftnrstcont",
        b"aftnrestart",
        b"aftnrstpg",
        b"aftnrstcont",
        b"ftnstart",
        b"aftnstart",
        b"chftn",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "note restart content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn unknown_ignorable_destinations_are_skipped_even_when_they_contain_objects() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "*",
        "\\",
        "unknown{",
        "\\",
        "object",
        "\\",
        "objdata ",
        payload_hex(),
        "}} visible}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("visible"));
    assert!(!text.contains(payload_hex()));
    assert!(!text.contains("[Embedded object removed]"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("unknown-ignorable-object.rtf");
    let output_path = dir.path().join("unknown-ignorable-object.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("visible"));
    for forbidden in [
        payload_hex().as_bytes(),
        b"unknown",
        b"object",
        b"objdata",
        b"Embedded object removed",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "unknown ignorable destination leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn unknown_non_ignorable_destinations_are_skipped_without_pdf_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 visible {",
        "\\",
        "unknown Hidden text {",
        "\\",
        "object",
        "\\",
        "objdata ",
        payload_hex(),
        " ",
        "\\",
        "bin5 ABCDE",
        " /JavaScript /EmbeddedFile}} after",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("visible  after"));
    for forbidden in [
        "Hidden text",
        payload_hex(),
        "unknown",
        "objdata",
        "ABCDE",
        "JavaScript",
        "EmbeddedFile",
        "[Embedded object removed]",
    ] {
        assert!(
            !text.contains(forbidden),
            "unknown destination leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("visible  after"));
    for forbidden in [
        b"Hidden text".as_slice(),
        payload_hex().as_bytes(),
        b"unknown",
        b"objdata",
        b"ABCDE",
        b"Embedded object removed",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "unknown destination leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn active_controls_inside_skipped_destinations_obey_reject_policy() {
    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };

    let skipped_object_result = parse_rtf_bytes_with_options(
        br"{\rtf1{\*\unknown{\object\objdata 414243}} visible\par}",
        &reject_options,
    );
    assert!(
        matches!(
            skipped_object_result,
            Err(ParseError::ActiveContentRejected { ref feature, .. })
                if feature == "object payload in skipped destination"
        ),
        "unexpected skipped object result: {skipped_object_result:?}"
    );
    assert!(matches!(
        parse_rtf_bytes_with_options(
            br"{\rtf1{\unknown{\objdata 414243}} visible\par}",
            &reject_options
        ),
        Err(ParseError::ActiveContentRejected { feature, .. })
            if feature == "object payload in skipped destination"
    ));
    assert!(matches!(
        parse_rtf_bytes_with_options(
            br#"{\rtf1{\*\unknown{\field{\*\fldinst INCLUDEPICTURE "https://example.com/a.png"}}} visible\par}"#,
            &reject_options
        ),
        Err(ParseError::ActiveContentRejected { feature, .. })
            if feature == "field instruction in skipped destination"
    ));
    assert!(matches!(
        parse_rtf_bytes_with_options(
            br"{\rtf1{\*\unknown{\fontemb{\fontfile HOSTILE-FONT-PAYLOAD}}} visible\par}",
            &reject_options
        ),
        Err(ParseError::ActiveContentRejected { feature, .. })
            if feature == "embedded font payload in skipped destination"
    ));
    assert!(matches!(
        parse_rtf_bytes_with_options(
            br"{\rtf1{\*\unknown{\template https://example.com/template.dotm}} visible\par}",
            &reject_options
        ),
        Err(ParseError::ActiveContentRejected { feature, .. })
            if feature == "external template in skipped destination"
    ));
    assert!(matches!(
        parse_rtf_bytes_with_options(
            br"{\rtf1{\*\template https://example.com/direct-template.dotm} visible\par}",
            &reject_options
        ),
        Err(ParseError::ActiveContentRejected { feature, .. })
            if feature == "external template in skipped destination"
    ));
    assert!(matches!(
        parse_rtf_bytes_with_options(
            br"{\rtf1{\*\unknown{\objclass HiddenClass}{\objname HiddenName}} visible\par}",
            &reject_options
        ),
        Err(ParseError::ActiveContentRejected { feature, .. })
            if feature == "object metadata in skipped destination"
    ));
    assert!(matches!(
        parse_rtf_bytes_with_options(
            br"{\rtf1{\*\unknown{\mmdatasource https://example.com/data.csv}} visible\par}",
            &reject_options
        ),
        Err(ParseError::ActiveContentRejected { feature, .. })
            if feature == "mail merge data source in skipped destination"
    ));
    assert!(matches!(
        parse_rtf_bytes_with_options(
            br"{\rtf1{\*\objdata 414243} visible\par}",
            &reject_options
        ),
        Err(ParseError::ActiveContentRejected { feature, .. })
            if feature == "object payload in skipped destination"
    ));
    assert!(matches!(
        parse_rtf_bytes_with_options(
            br"{\rtf1{\*\unknown{\annotation hidden comment}} visible\par}",
            &reject_options
        ),
        Err(ParseError::ActiveContentRejected { feature, .. })
            if feature == "annotation metadata in skipped destination"
    ));

    let parsed =
        parse_rtf_bytes(
            br"{\rtf1{\*\unknown{\object\objdata 414243}{\fontemb{\fontfile HOSTILE-FONT-PAYLOAD}}{\template https://example.com/template.dotm}{\objclass HiddenClass}{\mmdatasource https://example.com/data.csv}{\annotation hidden comment}}{\*\template https://example.com/direct-template.dotm}{\*\objdata 444546} visible\par}"
        ).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("visible"));
    assert!(!text.contains("414243"));
    assert!(!text.contains("444546"));
    assert!(!text.contains("template.dotm"));
    assert!(!text.contains("direct-template.dotm"));
    assert!(!text.contains("HiddenClass"));
    assert!(!text.contains("data.csv"));
    assert!(!text.contains("hidden comment"));
    assert!(!text.contains("HOSTILE-FONT-PAYLOAD"));
    assert!(!text.contains("[Embedded object removed]"));
    for expected in [
        "active content removed: object payload in skipped destination before safe model normalization",
        "active content removed: embedded font payload in skipped destination before safe model normalization",
        "active content removed: external template in skipped destination before safe model normalization",
        "active content removed: object metadata in skipped destination before safe model normalization",
        "active content removed: mail merge data source in skipped destination before safe model normalization",
        "active content removed: annotation metadata in skipped destination before safe model normalization",
    ] {
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "missing skipped active-content diagnostic {expected:?}: {:?}",
            parsed.diagnostics
        );
    }
}

#[test]
fn inline_ignorable_marker_does_not_suppress_following_visible_content() {
    let input = br"{\rtf1 Before \*After {\*\unknown Hidden {\object\objdata 414243}} visible\par}";
    let parsed = parse_rtf_bytes(input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert!(text.contains("visible"));
    assert!(!text.contains("Hidden"));
    assert!(!text.contains("414243"));
    let output = convert_rtf_to_pdf(
        input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 1);
    for forbidden in [
        b"Hidden".as_slice(),
        b"414243",
        b"objdata",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "inline ignorable marker allowed hidden payload into PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn ignorable_marker_before_formatting_control_does_not_hide_visible_text() {
    let input = br"{\rtf1 Before {\*\b Bold {\*\i italic}} After {\*\unknown Hidden {\object\objdata 414243}} visible\par}";
    let parsed = parse_rtf_bytes(input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before"));
    assert!(text.contains("Bold"));
    assert!(text.contains("italic"));
    assert!(text.contains("After"));
    assert!(text.contains("visible"));
    assert!(!text.contains("Hidden"));
    assert!(!text.contains("414243"));
    assert!(
        parsed.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "ignorable destination marker before a non-destination control was ignored",
            )
        }),
        "missing non-destination ignorable marker diagnostic: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("Bold"));
    assert!(rendered_text.contains("italic"));
    assert!(rendered_text.contains("After"));
    assert!(rendered_text.contains("visible"));
    for forbidden in [
        b"Hidden".as_slice(),
        b"414243",
        b"objdata",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "non-destination ignorable marker leaked hidden payload to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn generated_pdf_contains_no_active_features_or_raw_object_payloads() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("object.rtf");
    let output = dir.path().join("object.pdf");
    fs::write(
        &input,
        rtf(&[
            "{",
            "\\",
            "rtf1 before {",
            "\\",
            "object",
            "\\",
            "objdata ",
            payload_hex(),
            "} after}",
        ]),
    )
    .unwrap();

    convert_rtf_file_to_pdf(
        &input,
        &output,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();

    let pdf = fs::read(&output).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        payload_hex().as_bytes(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
        b"/OpenAction",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn generated_pdf_strips_object_payloads_inside_headers() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("header-object.rtf");
    let output = dir.path().join("header-object.pdf");
    fs::write(
        &input,
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "header before {",
            "\\",
            "object",
            "\\",
            "objdata ",
            payload_hex(),
            "} after",
            "\\",
            "par} body}",
        ]),
    )
    .unwrap();

    convert_rtf_file_to_pdf(
        &input,
        &output,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();

    let pdf = fs::read(&output).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        payload_hex().as_bytes(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn modern_shape_text_renders_passively_without_property_or_field_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before",
        "\\",
        "par{",
        "\\",
        "shp{",
        "\\",
        "shpinst{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-shape-property}}{",
        "\\",
        "shptxt Modern box text {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst HYPERLINK \"https://example.com/box\"}{",
        "\\",
        "fldrslt safe link}}",
        "\\",
        "par}}}After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before"));
    assert!(text.contains("Modern box text safe link"));
    assert!(text.contains("After"));
    for forbidden in [
        "shpinst",
        "pFragments",
        "hostile-shape-property",
        "shptxt",
        "HYPERLINK",
        "https://example.com/box",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden modern shape-text content leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("Modern box text safe link"));
    assert!(rendered_text.contains("After"));
    for forbidden in [
        b"shpinst".as_slice(),
        b"pFragments",
        b"hostile-shape-property",
        b"shptxt",
        b"HYPERLINK",
        b"https://example.com/box",
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden modern shape-text content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn bounded_shape_text_renders_inside_passive_shape_without_body_flow_or_payload_leakage() {
    let input = br#"{\rtf1{\shp{\shpinst\shpleft720\shptop720\shpright4320\shpbottom1800{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillColor}{\sv 13434879}}{\sp{\sn pFragments}{\sv hostile-shape-text-payload}}}{\shptxt Box text {\field{\*\fldinst HYPERLINK "https://example.com/shape-text"}{\fldrslt safe link}}\par}}After\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("shape block");

    assert_eq!(shape.text.len(), 1);
    assert!(text.contains("Box text safe link"));
    assert!(text.contains("After"));
    assert!(
        !parsed.document.blocks.iter().any(|block| {
            matches!(block, Block::Paragraph(paragraph) if paragraph.runs.iter().any(|run| run.text.contains("Box text")))
        }),
        "shape text should stay attached to the shape instead of entering body flow"
    );
    for forbidden in [
        "shpinst",
        "shapeType",
        "fillColor",
        "pFragments",
        "hostile-shape-text-payload",
        "HYPERLINK",
        "https://example.com/shape-text",
        "fldinst",
        "fldrslt",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden bounded shape-text content leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering safe passive shape text/result")
    }));
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("stripping unsupported/active drawing properties")
    }));
    assert!(
        output
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("stripping shape properties"))
    );
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let fill_index = content
        .operations
        .iter()
        .position(|operation| operation.operator == "f")
        .expect("shape fill should render");
    let text_index = content
        .operations
        .iter()
        .position(|operation| operation.operator == "Tj" || operation.operator == "TJ")
        .expect("shape text should render");

    assert!(rendered_text.contains("Box text safe link"));
    assert!(rendered_text.contains("After"));
    assert!(
        fill_index < text_index,
        "shape text should paint after shape fill"
    );
    for forbidden in [
        b"shpinst".as_slice(),
        b"shapeType",
        b"fillColor",
        b"pFragments",
        b"hostile-shape-text-payload",
        b"HYPERLINK",
        b"https://example.com/shape-text",
        b"fldinst",
        b"fldrslt",
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden bounded shape-text content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn styled_shape_text_renders_passively_without_style_control_leakage() {
    let input = br#"{\rtf1{\colortbl;\red240\green240\blue0;\red255\green0\blue0;}{\shp{\shpinst\shpleft720\shptop720\shpright4320\shpbottom1800{\sp{\sn shapeType}{\sv 1}}}{\shptxt\cbpat1\brdrb\brdrs\brdrw40\brdrcf2 Styled box text\par}}After\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("shape block");

    assert_eq!(shape.text.len(), 1);
    assert_eq!(shape.text[0].style.shading_color_index, Some(1));
    assert!(shape.text[0].style.borders.bottom.visible);
    assert!(text.contains("Styled box text"));
    assert!(text.contains("After"));
    for forbidden in [
        "shptxt",
        "cbpat",
        "brdrb",
        "brdrs",
        "brdrw",
        "brdrcf",
        "shpinst",
        "shapeType",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden styled shape-text content leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering safe passive shape text/result")
    }));
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Styled box text"));
    assert!(rendered_text.contains("After"));
    for forbidden in [
        b"shptxt".as_slice(),
        b"cbpat",
        b"brdrb",
        b"brdrs",
        b"brdrw",
        b"brdrcf",
        b"shpinst",
        b"shapeType",
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden styled shape-text content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn duplicate_object_alternate_after_shape_result_is_ignored_without_generic_ole_warning() {
    let input = br#"{\rtf1 Before\par{\shp{\shpinst\shpleft720\shptop720\shpright4320\shpbottom1800{\sp{\sn shapeType}{\sv 1}}}{\shptxt Box text\par}{\object\objdata 4142432f4a6176615363726970742f456d62656464656446696c65}}After\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before"));
    assert!(text.contains("Box text"));
    assert!(text.contains("After"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("ignoring duplicate embedded object alternate")
    }));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("active content removed: OLE object")
    }));
    for forbidden in ["object", "objdata", "414243", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "duplicate shape object alternate leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("Box text"));
    assert!(rendered_text.contains("After"));
    for forbidden in [
        b"object".as_slice(),
        b"objdata",
        b"414243",
        b"JavaScript",
        b"EmbeddedFile",
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "duplicate object alternate leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }

    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };
    assert!(matches!(
        parse_rtf_bytes_with_options(&input, &reject_options),
        Err(ParseError::ActiveContentRejected { feature, .. }) if feature == "OLE object"
    ));
}

#[test]
fn ignored_destinations_consume_bounded_skip_budget() {
    let options = RtfParseOptions {
        limits: RtfLimits {
            max_destination_bytes: 4,
            ..RtfLimits::default()
        },
        ..RtfParseOptions::default()
    };

    assert!(matches!(
        parse_rtf_bytes_with_options(br"{\rtf1{\*\unknown abcde} visible\par}", &options),
        Err(ParseError::DestinationTooLarge(_))
    ));
    assert!(matches!(
        parse_rtf_bytes_with_options(br"{\rtf1{\*\unknown\bin5 abcde} visible\par}", &options),
        Err(ParseError::DestinationTooLarge(_))
    ));
}

#[test]
fn old_drawing_text_box_renders_passively_without_property_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before",
        "\\",
        "par{",
        "\\",
        "do",
        "\\",
        "dobx720",
        "\\",
        "doby720",
        "\\",
        "dodhgt1{",
        "\\",
        "dptxbx Legacy box text",
        "\\",
        "par}{",
        "\\",
        "dpptx111 hostile-coordinate-payload}}After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before"));
    assert!(text.contains("Legacy box text"));
    assert!(text.contains("After"));
    for forbidden in [
        "dobx",
        "doby",
        "dodhgt",
        "dptxbx",
        "dpptx",
        "hostile-coordinate-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden drawing text-box content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("old-drawing-text-box.rtf");
    let output_path = dir.path().join("old-drawing-text-box.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("Legacy box text"));
    assert!(rendered_text.contains("After"));
    for forbidden in [
        b"dobx".as_slice(),
        b"doby",
        b"dodhgt",
        b"dptxbx",
        b"dpptx",
        b"hostile-coordinate-payload",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden drawing text-box content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn old_drawing_static_shapes_render_passively_without_property_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before",
        "\\",
        "par{",
        "\\",
        "do",
        "\\",
        "dprect",
        "\\",
        "dobx120",
        "\\",
        "doby240",
        "\\",
        "dpx360",
        "\\",
        "dpy480",
        "\\",
        "dpxsize1440",
        "\\",
        "dpysize720",
        "\\",
        "dplinew30",
        "\\",
        "dplinecor255",
        "\\",
        "dplinecog128",
        "\\",
        "dplinecob0",
        "\\",
        "dpfillfgcr10",
        "\\",
        "dpfillfgcg20",
        "\\",
        "dpfillfgcb30",
        "{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-shape-payload}}}After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let shape_count = parsed
        .document
        .blocks
        .iter()
        .filter(|block| matches!(block, Block::Shape(_)))
        .count();
    let text = collect_text(&parsed.document);

    assert_eq!(shape_count, 1);
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("static shape");
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 10,
            green: 20,
            blue: 30,
        })
    );
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "dprect",
        "dobx",
        "dpxsize",
        "dpfillfg",
        "pFragments",
        "hostile-shape-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden static drawing content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("old-drawing-static-shape.rtf");
    let output_path = dir.path().join("old-drawing-static-shape.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let stroke_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "S")
        .count();
    let fill_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "f")
        .count();

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(stroke_count >= 4);
    assert!(fill_count >= 1);
    for forbidden in [
        b"dprect".as_slice(),
        b"dobx",
        b"dpxsize",
        b"dpfillfg",
        b"pFragments",
        b"hostile-shape-payload",
        b"[Shape skipped",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden static drawing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn modern_static_shape_properties_render_passively_without_property_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before",
        "\\",
        "par{",
        "\\",
        "shp{",
        "\\",
        "*",
        "\\",
        "shpinst",
        "\\",
        "shpleft720",
        "\\",
        "shptop720",
        "\\",
        "shpright2160",
        "\\",
        "shpbottom1440",
        "\\",
        "shpbxpage",
        "\\",
        "shpbypage",
        "\\",
        "shpwr3",
        "\\",
        "shpfblwtxt1{",
        "\\",
        "sp{",
        "\\",
        "sn shapeType}{",
        "\\",
        "sv 1}}{",
        "\\",
        "sp{",
        "\\",
        "sn fillColor}{",
        "\\",
        "sv 65280}}{",
        "\\",
        "sp{",
        "\\",
        "sn lineColor}{",
        "\\",
        "sv 255}}{",
        "\\",
        "sp{",
        "\\",
        "sn lineWidth}{",
        "\\",
        "sv 12700}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-shape-payload}}}}After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("modern static shape");

    assert_eq!(
        shape.kind,
        open_rtf_converter::model::StaticShapeKind::Rectangle
    );
    assert_eq!(shape.left_twips, 720);
    assert_eq!(shape.top_twips, 720);
    assert_eq!(shape.width_twips, 1440);
    assert_eq!(shape.height_twips, 720);
    assert_eq!(shape.stroke_width_twips, 20);
    assert_eq!(
        shape.stroke_color,
        open_rtf_converter::model::Color {
            red: 255,
            green: 0,
            blue: 0,
        }
    );
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 0,
            green: 255,
            blue: 0,
        })
    );
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "shpinst",
        "shapeType",
        "fillColor",
        "lineColor",
        "lineWidth",
        "pFragments",
        "hostile-shape-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden modern shape content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("modern-static-shape.rtf");
    let output_path = dir.path().join("modern-static-shape.pdf");
    fs::write(&input_path, input).unwrap();
    let report = convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("stripping unsupported/active drawing properties")
    }));
    assert!(
        report
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("stripping shape properties"))
    );
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let stroke_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "S")
        .count();
    let fill_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "f")
        .count();

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(stroke_count >= 4);
    assert!(fill_count >= 1);
    for forbidden in [
        b"shpinst".as_slice(),
        b"shapeType",
        b"fillColor",
        b"lineColor",
        b"lineWidth",
        b"pFragments",
        b"hostile-shape-payload",
        b"[Shape skipped",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden modern shape content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn background_static_shape_renders_passively_without_body_flow_or_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "deff0{",
        "\\",
        "background Hidden background text {",
        "\\",
        "shp{",
        "\\",
        "*",
        "\\",
        "shpinst",
        "\\",
        "shpleft0",
        "\\",
        "shptop0",
        "\\",
        "shpright3000",
        "\\",
        "shpbottom1200{",
        "\\",
        "sp{",
        "\\",
        "sn shapeType}{",
        "\\",
        "sv 1}}{",
        "\\",
        "sp{",
        "\\",
        "sn fillColor}{",
        "\\",
        "sv 16776960}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-background-payload}}}}}",
        "Body text",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert_eq!(parsed.document.background_shapes.len(), 1);
    assert!(
        !parsed
            .document
            .blocks
            .iter()
            .any(|block| matches!(block, Block::Shape(_))),
        "background shape should not enter body flow"
    );
    assert!(text.contains("Body text"));
    for forbidden in [
        "background",
        "Hidden background text",
        "shpinst",
        "shapeType",
        "fillColor",
        "pFragments",
        "hostile-background-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "background content leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "background controls should not be unknown or unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let fill_index = content
        .operations
        .iter()
        .position(|operation| operation.operator == "f")
        .expect("background fill should render as passive vector fill");
    let text_index = content
        .operations
        .iter()
        .position(|operation| operation.operator == "Tj" || operation.operator == "TJ")
        .expect("body text should render");

    assert!(rendered_text.contains("Body text"));
    assert!(
        fill_index < text_index,
        "background fill should paint before body text"
    );
    for forbidden in [
        b"background".as_slice(),
        b"Hidden background text",
        b"shpinst",
        b"shapeType",
        b"fillColor",
        b"pFragments",
        b"hostile-background-payload",
        b"[Shape skipped",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "background payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn header_static_shape_renders_passively_without_body_flow_or_property_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "header Logo {",
        "\\",
        "do",
        "\\",
        "dprect",
        "\\",
        "dpxsize1440",
        "\\",
        "dpysize720",
        "\\",
        "dplinew30",
        "\\",
        "dpfillfgcr10",
        "\\",
        "dpfillfgcg20",
        "\\",
        "dpfillfgcb30",
        "{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-shape-payload}}}",
        "\\",
        "par} Body",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let body_shape_count = parsed
        .document
        .blocks
        .iter()
        .filter(|block| matches!(block, Block::Shape(_)))
        .count();

    assert!(text.contains("Logo"));
    assert!(text.contains("Body"));
    assert_eq!(parsed.document.header_shapes.len(), 1);
    assert_eq!(body_shape_count, 0);
    for forbidden in [
        "dprect",
        "dpxsize",
        "dpfillfg",
        "pFragments",
        "hostile-shape-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden header static shape content leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let stroke_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "S")
        .count();
    let fill_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "f")
        .count();

    assert!(rendered_text.contains("Logo"));
    assert!(rendered_text.contains("Body"));
    assert!(stroke_count >= 4);
    assert!(fill_count >= 1);
    for forbidden in [
        b"dprect".as_slice(),
        b"dpxsize",
        b"dpfillfg",
        b"pFragments",
        b"hostile-shape-payload",
        b"[Shape skipped",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden header static shape content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn header_shape_text_and_picture_render_passively_without_body_flow_or_payload_leakage() {
    let image_hex = bytes_to_hex(&minimal_jpeg_with_dimensions(1, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "header Logo {",
        "\\",
        "shp{",
        "\\",
        "*",
        "\\",
        "shpinst{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-shape-payload}}}{",
        "\\",
        "*",
        "\\",
        "shppict{",
        "\\",
        "pict",
        "\\",
        "jpegblip",
        "\\",
        "picwgoal720",
        "\\",
        "pichgoal720 ",
        image_hex.as_str(),
        "}}{",
        "\\",
        "shptxt Box text",
        "\\",
        "par}}",
        "\\",
        "par} Body",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let body_image_count = parsed
        .document
        .blocks
        .iter()
        .filter(|block| matches!(block, Block::Image(_)))
        .count();
    let body_text = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(
                paragraph
                    .runs
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<String>(),
            ),
            _ => None,
        })
        .collect::<String>();

    assert!(text.contains("Logo"));
    assert!(text.contains("Box text"));
    assert!(text.contains("Body"));
    assert_eq!(parsed.document.header_images.len(), 1);
    assert_eq!(body_image_count, 0);
    assert_eq!(body_text.trim(), "Body");
    for forbidden in [
        "pFragments",
        "hostile-shape-payload",
        "shpinst",
        "shppict",
        "jpegblip",
        "picwgoal",
        "shptxt",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden header shape fallback content leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Logo"));
    assert!(rendered_text.contains("Box text"));
    assert!(rendered_text.contains("Body"));
    assert!(
        output
            .pdf
            .windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image")
    );
    for forbidden in [
        b"pFragments".as_slice(),
        b"hostile-shape-payload",
        b"shpinst",
        b"shppict",
        b"jpegblip",
        b"picwgoal",
        b"shptxt",
        b"[Shape skipped",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden header shape fallback content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn old_drawing_zero_width_outline_renders_fill_only_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before",
        "\\",
        "par{",
        "\\",
        "do",
        "\\",
        "dprect",
        "\\",
        "dpxsize1440",
        "\\",
        "dpysize720",
        "\\",
        "dplinew0",
        "\\",
        "dplinecor255",
        "\\",
        "dplinecog0",
        "\\",
        "dplinecob0",
        "\\",
        "dpfillfgcr10",
        "\\",
        "dpfillfgcg20",
        "\\",
        "dpfillfgcb30",
        "{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-fill-only-payload}}}After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("fill-only shape");
    let text = collect_text(&parsed.document);

    assert_eq!(shape.stroke_width_twips, 0);
    assert!(shape.fill_color.is_some());
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "dplinew",
        "dpfillfg",
        "pFragments",
        "hostile-fill-only-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden fill-only drawing content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("old-drawing-fill-only-shape.rtf");
    let output_path = dir.path().join("old-drawing-fill-only-shape.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let fill_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "f")
        .count();
    let stroke_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "S")
        .count();

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(fill_count >= 1);
    assert_eq!(stroke_count, 0);
    for forbidden in [
        b"dplinew".as_slice(),
        b"dpfillfg",
        b"pFragments",
        b"hostile-fill-only-payload",
        b"[Shape skipped",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden fill-only drawing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn old_drawing_line_styles_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before",
        "\\",
        "par{",
        "\\",
        "do",
        "\\",
        "dpline",
        "\\",
        "dplinedash",
        "\\",
        "dpx360",
        "\\",
        "dpy480",
        "\\",
        "dpxsize1440",
        "\\",
        "dpysize720",
        "\\",
        "dplinew30",
        "{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-line-style-payload}}}After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("styled line shape");
    let text = collect_text(&parsed.document);

    assert_eq!(
        shape.stroke_style,
        open_rtf_converter::model::BorderStyle::Dashed
    );
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "dpline",
        "dplinedash",
        "pFragments",
        "hostile-line-style-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden line-style drawing content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("old-drawing-line-style.rtf");
    let output_path = dir.path().join("old-drawing-line-style.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "d")
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "S")
    );
    for forbidden in [
        b"dpline".as_slice(),
        b"dplinedash",
        b"pFragments",
        b"hostile-line-style-payload",
        b"[Shape skipped",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden line-style drawing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn flipped_old_drawing_line_renders_passively_without_property_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before",
        "\\",
        "par{",
        "\\",
        "do",
        "\\",
        "dpline",
        "\\",
        "dpx360",
        "\\",
        "dpy480",
        "\\",
        "dpxsize1440",
        "\\",
        "dpysize720",
        "{",
        "\\",
        "sp{",
        "\\",
        "sn fFlipH}{",
        "\\",
        "sv 1}}{",
        "\\",
        "sp{",
        "\\",
        "sn fFlipV}{",
        "\\",
        "sv 1}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-flip-payload}}}After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("flipped line shape");
    let text = collect_text(&parsed.document);

    assert_eq!(shape.kind, open_rtf_converter::model::StaticShapeKind::Line);
    assert!(shape.flip_horizontal);
    assert!(shape.flip_vertical);
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "fFlipH",
        "fFlipV",
        "pFragments",
        "hostile-flip-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden flipped drawing content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("old-drawing-flipped-line.rtf");
    let output_path = dir.path().join("old-drawing-flipped-line.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "m")
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "l")
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "S")
    );
    for forbidden in [
        b"fFlipH".as_slice(),
        b"fFlipV",
        b"pFragments",
        b"hostile-flip-payload",
        b"[Shape skipped",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden flipped drawing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_line_dashing_renders_passively_without_property_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before",
        "\\",
        "par{",
        "\\",
        "do",
        "\\",
        "dpline",
        "\\",
        "dpx360",
        "\\",
        "dpy480",
        "\\",
        "dpxsize1440",
        "\\",
        "dpysize720",
        "{",
        "\\",
        "sp{",
        "\\",
        "sn lineDashing}{",
        "\\",
        "sv dashDot}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-dashing-payload}}}After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("dashed line shape");
    let text = collect_text(&parsed.document);

    assert_eq!(
        shape.stroke_style,
        open_rtf_converter::model::BorderStyle::Dashed
    );
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "lineDashing",
        "dashDot",
        "pFragments",
        "hostile-dashing-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden lineDashing drawing content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("office-shape-line-dashing.rtf");
    let output_path = dir.path().join("office-shape-line-dashing.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "d")
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "S")
    );
    for forbidden in [
        b"lineDashing".as_slice(),
        b"dashDot",
        b"pFragments",
        b"hostile-dashing-payload",
        b"[Shape skipped",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden lineDashing drawing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_arrowheads_render_passively_without_property_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before",
        "\\",
        "par{",
        "\\",
        "do",
        "\\",
        "dpline",
        "\\",
        "dpx360",
        "\\",
        "dpy480",
        "\\",
        "dpxsize1440",
        "\\",
        "dpysize720",
        "{",
        "\\",
        "sp{",
        "\\",
        "sn lineStartArrowhead}{",
        "\\",
        "sv open}}{",
        "\\",
        "sp{",
        "\\",
        "sn lineEndArrowhead}{",
        "\\",
        "sv triangle}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-arrow-payload}}}After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("arrowhead line shape");
    let text = collect_text(&parsed.document);

    assert_eq!(
        shape.start_arrowhead,
        open_rtf_converter::model::StaticShapeArrowhead::Open
    );
    assert_eq!(
        shape.end_arrowhead,
        open_rtf_converter::model::StaticShapeArrowhead::Triangle
    );
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "lineStartArrowhead",
        "lineEndArrowhead",
        "open",
        "triangle",
        "pFragments",
        "hostile-arrow-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden arrowhead drawing content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("office-shape-arrowheads.rtf");
    let output_path = dir.path().join("office-shape-arrowheads.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "S")
            .count()
            >= 3
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "B")
    );
    for forbidden in [
        b"lineStartArrowhead".as_slice(),
        b"lineEndArrowhead",
        b"triangle",
        b"pFragments",
        b"hostile-arrow-payload",
        b"[Shape skipped",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden arrowhead drawing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn old_drawing_polyline_renders_passively_without_coordinate_or_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before",
        "\\",
        "par{",
        "\\",
        "do",
        "\\",
        "dppolyline",
        "\\",
        "dplinedot",
        "\\",
        "dplinew30",
        "\\",
        "dpptx360",
        "\\",
        "dppty480",
        "\\",
        "dpptx1080",
        "\\",
        "dppty1200",
        "\\",
        "dpptx1800",
        "\\",
        "dppty480",
        "{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-polyline-payload}}}After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("polyline shape");
    let text = collect_text(&parsed.document);

    assert_eq!(
        shape.kind,
        open_rtf_converter::model::StaticShapeKind::Polyline
    );
    assert_eq!(shape.points.len(), 3);
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "dppolyline",
        "dpptx",
        "dppty",
        "dplinedot",
        "pFragments",
        "hostile-polyline-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden polyline drawing content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("old-drawing-polyline.rtf");
    let output_path = dir.path().join("old-drawing-polyline.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let stroke_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "S")
        .count();

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(stroke_count >= 2);
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "d")
    );
    for forbidden in [
        b"dppolyline".as_slice(),
        b"dpptx",
        b"dppty",
        b"dplinedot",
        b"pFragments",
        b"hostile-polyline-payload",
        b"[Shape skipped",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden polyline drawing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn old_drawing_polygon_renders_passively_without_coordinate_or_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before",
        "\\",
        "par{",
        "\\",
        "do",
        "\\",
        "dppolygon",
        "\\",
        "dplinedot",
        "\\",
        "dplinew30",
        "\\",
        "dpfillfgcr10",
        "\\",
        "dpfillfgcg20",
        "\\",
        "dpfillfgcb30",
        "\\",
        "dpptx360",
        "\\",
        "dppty480",
        "\\",
        "dpptx1080",
        "\\",
        "dppty1200",
        "\\",
        "dpptx1800",
        "\\",
        "dppty480",
        "{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-polygon-payload}}}After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("polygon shape");
    let text = collect_text(&parsed.document);

    assert_eq!(
        shape.kind,
        open_rtf_converter::model::StaticShapeKind::Polygon
    );
    assert_eq!(shape.points.len(), 3);
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 10,
            green: 20,
            blue: 30,
        })
    );
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "dppolygon",
        "dpptx",
        "dppty",
        "dplinedot",
        "dpfillfg",
        "pFragments",
        "hostile-polygon-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden polygon drawing content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("old-drawing-polygon.rtf");
    let output_path = dir.path().join("old-drawing-polygon.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "B")
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "d")
    );
    for forbidden in [
        b"dppolygon".as_slice(),
        b"dpptx",
        b"dppty",
        b"dplinedot",
        b"dpfillfg",
        b"pFragments",
        b"hostile-polygon-payload",
        b"[Shape skipped",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden polygon drawing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn old_drawing_static_ellipse_renders_passively_without_property_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before",
        "\\",
        "par{",
        "\\",
        "do",
        "\\",
        "dpellipse",
        "\\",
        "dobx120",
        "\\",
        "doby240",
        "\\",
        "dpx360",
        "\\",
        "dpy480",
        "\\",
        "dpxsize1440",
        "\\",
        "dpysize720",
        "\\",
        "dplinew30",
        "\\",
        "dplinecor255",
        "\\",
        "dplinecog128",
        "\\",
        "dplinecob0",
        "\\",
        "dpfillfgcr10",
        "\\",
        "dpfillfgcg20",
        "\\",
        "dpfillfgcb30",
        "{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-ellipse-payload}}}After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("static ellipse");
    let text = collect_text(&parsed.document);

    assert_eq!(
        shape.kind,
        open_rtf_converter::model::StaticShapeKind::Ellipse
    );
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 10,
            green: 20,
            blue: 30,
        })
    );
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "dpellipse",
        "dobx",
        "dpxsize",
        "dpfillfg",
        "pFragments",
        "hostile-ellipse-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden ellipse drawing content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("old-drawing-static-ellipse.rtf");
    let output_path = dir.path().join("old-drawing-static-ellipse.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let curve_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "c")
        .count();

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(curve_count >= 4);
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "B")
    );
    for forbidden in [
        b"dpellipse".as_slice(),
        b"dobx",
        b"dpxsize",
        b"dpfillfg",
        b"pFragments",
        b"hostile-ellipse-payload",
        b"[Shape skipped",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden ellipse drawing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn old_drawing_static_rounded_rectangle_renders_passively_without_property_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before",
        "\\",
        "par{",
        "\\",
        "do",
        "\\",
        "dprect",
        "\\",
        "dproundr",
        "\\",
        "dobx120",
        "\\",
        "doby240",
        "\\",
        "dpx360",
        "\\",
        "dpy480",
        "\\",
        "dpxsize1440",
        "\\",
        "dpysize720",
        "\\",
        "dplinew30",
        "\\",
        "dplinecor255",
        "\\",
        "dplinecog128",
        "\\",
        "dplinecob0",
        "\\",
        "dpfillfgcr10",
        "\\",
        "dpfillfgcg20",
        "\\",
        "dpfillfgcb30",
        "{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-roundrect-payload}}}After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("static rounded rectangle");
    let text = collect_text(&parsed.document);

    assert_eq!(
        shape.kind,
        open_rtf_converter::model::StaticShapeKind::RoundedRectangle
    );
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 10,
            green: 20,
            blue: 30,
        })
    );
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "dprect",
        "dproundr",
        "dobx",
        "dpxsize",
        "dpfillfg",
        "pFragments",
        "hostile-roundrect-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden rounded-rectangle drawing content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("old-drawing-static-rounded-rectangle.rtf");
    let output_path = dir.path().join("old-drawing-static-rounded-rectangle.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let curve_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "c")
        .count();

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(curve_count >= 4);
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "B")
    );
    for forbidden in [
        b"dprect".as_slice(),
        b"dproundr",
        b"dobx",
        b"dpxsize",
        b"dpfillfg",
        b"pFragments",
        b"hostile-roundrect-payload",
        b"[Shape skipped",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden rounded-rectangle drawing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn hostile_seed_corpus_returns_typed_results_without_panics() {
    let seeds = vec![
        rtf(&["{", "\\", "rtf1 hello}"]),
        rtf(&["{", "\\", "rtf1{", "\\", "b bold}}"]),
        rtf(&["{", "\\", "rtf1 ", "\\", "u8217?}"]),
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "fonttbl{",
            "\\",
            "f0 Arial;}}Hello}",
        ]),
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "colortbl;",
            "\\",
            "red255",
            "\\",
            "green0",
            "\\",
            "blue0;}Hello}",
        ]),
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "*",
            "\\",
            "unknown ignored}visible}",
        ]),
        object_with_payload(false),
        object_with_payload(true),
        field_with_link_result(),
        rtf(&[
            "{",
            "\\",
            "rtf1 visible{",
            "\\",
            "annotation hidden {",
            "\\",
            "object",
            "\\",
            "objdata ",
            payload_hex(),
            "}}}",
        ]),
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "bkmkstart SecretBookmark}visible{",
            "\\",
            "deleted Deleted text}}",
        ]),
        rtf(&[
            "{",
            "\\",
            "rtf1 visible {",
            "\\",
            "v hidden {",
            "\\",
            "object",
            "\\",
            "objdata ",
            payload_hex(),
            "}}}",
        ]),
        rtf(&["{", "\\", "rtf1", "\\", "bin3 abc}"]),
        rtf(&["{", "\\", "rtf1", "\\", "'41", "\\", "'42", "\\", "'43}"]),
        b"{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{".to_vec(),
        rtf(&["{", "\\", "rtf1", "\\", "bin999999999999999999999 abc}"]),
        rtf(&["{", "\\", "rtf1", "\\", "bin-1 abc}"]),
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "*",
            "\\",
            "unknown{",
            "\\",
            "object",
            "\\",
            "objdata ",
            payload_hex(),
            "}}}",
        ]),
        rtf(&[
            "{",
            "\\",
            "rtf1",
            "\\",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa text}",
        ]),
        rtf(&["{", "\\", "rtf1", "\\", "'GZ}"]),
    ];

    for seed in seeds {
        let _ = parse_rtf_bytes(&seed);
    }
}

fn object_with_payload(with_result: bool) -> Vec<u8> {
    if with_result {
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "object{",
            "\\",
            "objdata ",
            payload_hex(),
            "}{",
            "\\",
            "result visible fallback}}}",
        ])
    } else {
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "object",
            "\\",
            "objocx",
            "\\",
            "objdata ",
            payload_hex(),
            "}}",
        ])
    }
}

fn field_with_link_result() -> Vec<u8> {
    rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst HYPER",
        "LINK \"https://example.com\"}{",
        "\\",
        "fldrslt visible link}}}",
    ])
}

fn payload_hex() -> &'static str {
    "414243"
}

fn rtf(parts: &[&str]) -> Vec<u8> {
    let mut output = Vec::new();
    for part in parts {
        output.extend_from_slice(part.as_bytes());
    }
    output
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn minimal_jpeg_with_dimensions(width: u16, height: u16) -> Vec<u8> {
    minimal_jpeg_with_components(width, height, 3)
}

fn minimal_grayscale_jpeg_with_dimensions(width: u16, height: u16) -> Vec<u8> {
    minimal_jpeg_with_components(width, height, 1)
}

fn minimal_jpeg_with_components(width: u16, height: u16, components: u8) -> Vec<u8> {
    let [height_hi, height_lo] = height.to_be_bytes();
    let [width_hi, width_lo] = width.to_be_bytes();
    let segment_len = 8 + u16::from(components) * 3;
    let [segment_hi, segment_lo] = segment_len.to_be_bytes();
    let mut jpeg = vec![
        0xff, 0xd8, 0xff, 0xc0, segment_hi, segment_lo, 0x08, height_hi, height_lo, width_hi,
        width_lo, components,
    ];
    for component in 1..=components {
        jpeg.extend_from_slice(&[component, 0x11, 0x00]);
    }
    jpeg.extend_from_slice(&[0xff, 0xd9]);
    jpeg
}

fn minimal_grayscale_png_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 0, 0, 0, 0]);
    push_png_chunk(&mut png, b"IHDR", &ihdr);

    let idat = [
        0x78, 0x01, 0x01, 0x02, 0x00, 0xfd, 0xff, 0x00, 0x80, 0x00, 0x81, 0x00, 0x81,
    ];
    push_png_chunk(&mut png, b"IDAT", &idat);
    push_png_chunk(&mut png, b"IEND", &[]);
    png
}

fn minimal_indexed_png_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 3, 0, 0, 0]);
    push_png_chunk(&mut png, b"IHDR", &ihdr);
    push_png_chunk(&mut png, b"PLTE", &[255, 0, 0, 0, 255, 0]);

    let idat = [
        0x78, 0x01, 0x01, 0x02, 0x00, 0xfd, 0xff, 0x00, 0x01, 0x00, 0x02, 0x00, 0x02,
    ];
    push_png_chunk(&mut png, b"IDAT", &idat);
    push_png_chunk(&mut png, b"IEND", &[]);
    png
}

fn push_png_chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(kind);
    png.extend_from_slice(data);
    png.extend_from_slice(&0u32.to_be_bytes());
}

fn minimal_24bit_dib_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    let row_stride = ((width as usize * 3) + 3) / 4 * 4;
    let pixel_bytes = row_stride * height as usize;
    let mut dib = Vec::with_capacity(40 + pixel_bytes);
    dib.extend_from_slice(&40u32.to_le_bytes());
    dib.extend_from_slice(&(width as i32).to_le_bytes());
    dib.extend_from_slice(&(height as i32).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&24u16.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());

    for _ in 0..height {
        let mut row = Vec::with_capacity(row_stride);
        for x in 0..width {
            if x % 2 == 0 {
                row.extend_from_slice(&[0, 0, 255]);
            } else {
                row.extend_from_slice(&[0, 255, 0]);
            }
        }
        row.resize(row_stride, 0);
        dib.extend_from_slice(&row);
    }
    dib
}

fn minimal_8bit_dib_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    minimal_indexed_dib_with_dimensions(width, height, 8)
}

fn minimal_4bit_dib_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    minimal_indexed_dib_with_dimensions(width, height, 4)
}

fn minimal_4bit_bitmap_core_dib_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    let row_stride = ((width as usize * 4).div_ceil(32)) * 4;
    let pixel_bytes = row_stride * height as usize;
    let palette_entries = 16u32;
    let mut dib = Vec::with_capacity(12 + (palette_entries as usize * 3) + pixel_bytes);
    dib.extend_from_slice(&12u32.to_le_bytes());
    dib.extend_from_slice(&(width as u16).to_le_bytes());
    dib.extend_from_slice(&(height as u16).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&4u16.to_le_bytes());
    dib.extend_from_slice(&[0, 0, 255]);
    dib.extend_from_slice(&[0, 255, 0]);
    for _ in 2..palette_entries {
        dib.extend_from_slice(&[0, 0, 0]);
    }

    for _ in 0..height {
        let mut row = indexed_dib_test_row(width, 4);
        row.resize(row_stride, 0);
        dib.extend_from_slice(&row);
    }
    dib
}

fn minimal_indexed_dib_with_dimensions(width: u32, height: u32, bits_per_pixel: u16) -> Vec<u8> {
    let row_stride = ((width as usize * usize::from(bits_per_pixel)).div_ceil(32)) * 4;
    let pixel_bytes = row_stride * height as usize;
    let palette_entries = 2u32;
    let mut dib = Vec::with_capacity(40 + (palette_entries as usize * 4) + pixel_bytes);
    dib.extend_from_slice(&40u32.to_le_bytes());
    dib.extend_from_slice(&(width as i32).to_le_bytes());
    dib.extend_from_slice(&(height as i32).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&bits_per_pixel.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&palette_entries.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&[0, 0, 255, 0]);
    dib.extend_from_slice(&[0, 255, 0, 0]);

    for _ in 0..height {
        let mut row = indexed_dib_test_row(width, bits_per_pixel);
        row.resize(row_stride, 0);
        dib.extend_from_slice(&row);
    }
    dib
}

fn indexed_dib_test_row(width: u32, bits_per_pixel: u16) -> Vec<u8> {
    match bits_per_pixel {
        4 => {
            let mut row = Vec::new();
            let mut high = None;
            for x in 0..width {
                let index = (x % 2) as u8;
                if let Some(high_index) = high.take() {
                    row.push((high_index << 4) | index);
                } else {
                    high = Some(index);
                }
            }
            if let Some(high_index) = high {
                row.push(high_index << 4);
            }
            row
        }
        8 => (0..width).map(|x| (x % 2) as u8).collect(),
        _ => Vec::new(),
    }
}

fn collect_text(document: &open_rtf_converter::model::Document) -> String {
    let mut text = String::new();
    for paragraph in &document.header {
        append_paragraph_text(&mut text, paragraph);
    }
    for paragraph in &document.first_page_header {
        append_paragraph_text(&mut text, paragraph);
    }
    for paragraph in &document.even_page_header {
        append_paragraph_text(&mut text, paragraph);
    }
    for paragraph in &document.footer {
        append_paragraph_text(&mut text, paragraph);
    }
    for paragraph in &document.first_page_footer {
        append_paragraph_text(&mut text, paragraph);
    }
    for paragraph in &document.even_page_footer {
        append_paragraph_text(&mut text, paragraph);
    }
    for paragraph in &document.footnotes {
        append_paragraph_text(&mut text, paragraph);
    }
    for paragraph in &document.endnotes {
        append_paragraph_text(&mut text, paragraph);
    }
    for block in &document.blocks {
        match block {
            open_rtf_converter::model::Block::Paragraph(paragraph) => {
                append_paragraph_text(&mut text, paragraph);
            }
            open_rtf_converter::model::Block::Table(table) => {
                for row in &table.rows {
                    for cell in &row.cells {
                        for paragraph in &cell.paragraphs {
                            append_paragraph_text(&mut text, paragraph);
                        }
                    }
                }
            }
            open_rtf_converter::model::Block::Placeholder(value) => text.push_str(value),
            open_rtf_converter::model::Block::Shape(shape) => {
                for paragraph in &shape.text {
                    append_paragraph_text(&mut text, paragraph);
                }
            }
            open_rtf_converter::model::Block::Image(_) => {}
            open_rtf_converter::model::Block::SectionSettings(settings) => {
                for paragraph in &settings.header {
                    append_paragraph_text(&mut text, paragraph);
                }
                for paragraph in &settings.first_page_header {
                    append_paragraph_text(&mut text, paragraph);
                }
                for paragraph in &settings.even_page_header {
                    append_paragraph_text(&mut text, paragraph);
                }
                for paragraph in &settings.footer {
                    append_paragraph_text(&mut text, paragraph);
                }
                for paragraph in &settings.first_page_footer {
                    append_paragraph_text(&mut text, paragraph);
                }
                for paragraph in &settings.even_page_footer {
                    append_paragraph_text(&mut text, paragraph);
                }
            }
            open_rtf_converter::model::Block::PageBreak
            | open_rtf_converter::model::Block::ColumnBreak
            | open_rtf_converter::model::Block::ContinuousSectionBreak
            | open_rtf_converter::model::Block::SectionBreak
            | open_rtf_converter::model::Block::EvenPageSectionBreak
            | open_rtf_converter::model::Block::OddPageSectionBreak => {}
        }
    }
    text
}

fn assert_passive_checkbox_vectors_without_zapf(
    pdf: &[u8],
    content: &lopdf::content::Content,
    context: &str,
) {
    assert!(
        !pdf.windows(b"/BaseFont /ZapfDingbats".len())
            .any(|window| window == b"/BaseFont /ZapfDingbats"),
        "{context} should not require a viewer ZapfDingbats font"
    );
    assert!(
        pdf_text_bytes_for_font(content, b"F14").is_empty(),
        "{context} should not emit checkbox glyphs as ZapfDingbats text bytes"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "{context} should render checkbox boxes as passive vector rectangles"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "S"),
        "{context} should render checkbox marks as passive stroked vector paths"
    );
}

fn run_style_for_text<'a>(
    document: &'a open_rtf_converter::model::Document,
    exact_text: &str,
) -> Option<&'a CharacterStyle> {
    for paragraph in &document.header {
        if let Some(style) = paragraph_run_style_for_text(paragraph, exact_text) {
            return Some(style);
        }
    }
    for paragraph in &document.first_page_header {
        if let Some(style) = paragraph_run_style_for_text(paragraph, exact_text) {
            return Some(style);
        }
    }
    for paragraph in &document.even_page_header {
        if let Some(style) = paragraph_run_style_for_text(paragraph, exact_text) {
            return Some(style);
        }
    }
    for paragraph in &document.footer {
        if let Some(style) = paragraph_run_style_for_text(paragraph, exact_text) {
            return Some(style);
        }
    }
    for paragraph in &document.first_page_footer {
        if let Some(style) = paragraph_run_style_for_text(paragraph, exact_text) {
            return Some(style);
        }
    }
    for paragraph in &document.even_page_footer {
        if let Some(style) = paragraph_run_style_for_text(paragraph, exact_text) {
            return Some(style);
        }
    }
    for paragraph in &document.footnotes {
        if let Some(style) = paragraph_run_style_for_text(paragraph, exact_text) {
            return Some(style);
        }
    }
    for paragraph in &document.endnotes {
        if let Some(style) = paragraph_run_style_for_text(paragraph, exact_text) {
            return Some(style);
        }
    }
    for block in &document.blocks {
        if let Some(style) = block_run_style_for_text(block, exact_text) {
            return Some(style);
        }
    }
    None
}

fn block_run_style_for_text<'a>(block: &'a Block, exact_text: &str) -> Option<&'a CharacterStyle> {
    match block {
        Block::Paragraph(paragraph) => paragraph_run_style_for_text(paragraph, exact_text),
        Block::Table(table) => {
            for row in &table.rows {
                for cell in &row.cells {
                    for paragraph in &cell.paragraphs {
                        if let Some(style) = paragraph_run_style_for_text(paragraph, exact_text) {
                            return Some(style);
                        }
                    }
                }
            }
            None
        }
        Block::Shape(shape) => {
            for paragraph in &shape.text {
                if let Some(style) = paragraph_run_style_for_text(paragraph, exact_text) {
                    return Some(style);
                }
            }
            None
        }
        Block::SectionSettings(settings) => {
            for paragraph in settings
                .header
                .iter()
                .chain(&settings.first_page_header)
                .chain(&settings.even_page_header)
                .chain(&settings.footer)
                .chain(&settings.first_page_footer)
                .chain(&settings.even_page_footer)
            {
                if let Some(style) = paragraph_run_style_for_text(paragraph, exact_text) {
                    return Some(style);
                }
            }
            None
        }
        Block::Placeholder(_)
        | Block::Image(_)
        | Block::PageBreak
        | Block::ColumnBreak
        | Block::ContinuousSectionBreak
        | Block::SectionBreak
        | Block::EvenPageSectionBreak
        | Block::OddPageSectionBreak => None,
    }
}

fn paragraph_run_style_for_text<'a>(
    paragraph: &'a open_rtf_converter::model::Paragraph,
    exact_text: &str,
) -> Option<&'a CharacterStyle> {
    paragraph
        .runs
        .iter()
        .find(|run| run.text == exact_text)
        .map(|run| &run.style)
}

fn append_paragraph_text(text: &mut String, paragraph: &open_rtf_converter::model::Paragraph) {
    for run in &paragraph.runs {
        text.push_str(&run.text);
    }
}

fn strip_bookmark_page_markers(text: &str) -> String {
    let mut stripped = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(marker_start) = rest
        .find(BOOKMARK_PAGE_ANCHOR_MARKER)
        .into_iter()
        .chain(rest.find(BOOKMARK_PAGE_REF_MARKER))
        .min()
    {
        stripped.push_str(&rest[..marker_start]);
        let marker_tail = &rest[marker_start..];
        let Some(marker_end) = marker_tail.find(BOOKMARK_PAGE_MARKER_END) else {
            rest = &rest[marker_start + BOOKMARK_PAGE_ANCHOR_MARKER.len()..];
            continue;
        };
        rest = &marker_tail[marker_end + BOOKMARK_PAGE_MARKER_END.len()..];
    }
    stripped.push_str(rest);
    stripped
}

fn decoded_pdf_text(content: &lopdf::content::Content) -> String {
    let mut text = String::new();
    for operation in &content.operations {
        if operation.operator == "Tj" {
            for operand in &operation.operands {
                if let Ok(bytes) = operand.as_str() {
                    text.push_str(&String::from_utf8_lossy(bytes));
                }
            }
        } else if operation.operator == "TJ" {
            for operand in &operation.operands {
                if let Ok(items) = operand.as_array() {
                    for item in items {
                        if let Ok(bytes) = item.as_str() {
                            text.push_str(&String::from_utf8_lossy(bytes));
                        }
                    }
                }
            }
        }
    }
    text
}

fn pdf_first_text_position_for_text(
    content: &lopdf::content::Content,
    needle: &str,
) -> Option<(f32, f32)> {
    let mut position = None;
    for operation in &content.operations {
        if operation.operator == "Td" {
            let x = operation.operands.first().and_then(pdf_operand_number)?;
            let y = operation.operands.get(1).and_then(pdf_operand_number)?;
            position = Some((x, y));
        } else if operation.operator == "Tj" {
            if operation.operands.iter().any(|operand| {
                operand
                    .as_str()
                    .is_ok_and(|bytes| String::from_utf8_lossy(bytes).contains(needle))
            }) {
                return position;
            }
        } else if operation.operator == "TJ"
            && operation.operands.iter().any(|operand| {
                operand.as_array().is_ok_and(|items| {
                    items.iter().any(|item| {
                        item.as_str()
                            .is_ok_and(|bytes| String::from_utf8_lossy(bytes).contains(needle))
                    })
                })
            })
        {
            return position;
        }
    }
    None
}

fn pdf_text_font_names(content: &lopdf::content::Content) -> Vec<Vec<u8>> {
    content
        .operations
        .iter()
        .filter(|operation| operation.operator == "Tf")
        .filter_map(|operation| operation.operands.first())
        .filter_map(|operand| operand.as_name().ok().map(|name| name.to_vec()))
        .collect()
}

fn pdf_text_bytes_for_font(content: &lopdf::content::Content, font_name: &[u8]) -> Vec<u8> {
    let mut current_font: Option<Vec<u8>> = None;
    let mut output = Vec::new();
    for operation in &content.operations {
        if operation.operator == "Tf" {
            current_font = operation
                .operands
                .first()
                .and_then(|operand| operand.as_name().ok().map(|name| name.to_vec()));
        } else if operation.operator == "Tj" && current_font.as_deref() == Some(font_name) {
            for operand in &operation.operands {
                if let Ok(bytes) = operand.as_str() {
                    output.extend_from_slice(bytes);
                }
            }
        } else if operation.operator == "TJ" && current_font.as_deref() == Some(font_name) {
            for operand in &operation.operands {
                if let Ok(items) = operand.as_array() {
                    for item in items {
                        if let Ok(bytes) = item.as_str() {
                            output.extend_from_slice(bytes);
                        }
                    }
                }
            }
        }
    }
    output
}

fn pdf_operand_number(object: &lopdf::Object) -> Option<f32> {
    match object {
        lopdf::Object::Integer(value) => Some(*value as f32),
        lopdf::Object::Real(value) => Some(*value),
        _ => None,
    }
}

fn has_wmf_preview_warning(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("WMF picture rendered as bounded passive vector preview")
    })
}

fn assert_no_wmf_preview_warning(diagnostics: &[Diagnostic]) {
    assert!(
        !has_wmf_preview_warning(diagnostics),
        "fully handled WMF should not be reported as a partial passive preview: {diagnostics:?}"
    );
}
