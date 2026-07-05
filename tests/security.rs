use std::fs;

use lopdf::Document as PdfDocument;
use open_rtf_converter::model::{
    Alignment, BOOKMARK_PAGE_ANCHOR_MARKER, BOOKMARK_PAGE_MARKER_END, BOOKMARK_PAGE_REF_MARKER,
    Block, DOCUMENT_CHARS_MARKER, DOCUMENT_CHARS_WITH_SPACES_MARKER, DOCUMENT_WORDS_MARKER,
    EndnotePlacement, FontFamilyHint, FontPitch, PAGE_NUMBER_MARKER, PageVerticalAlignment,
    SECTION_NUMBER_MARKER, SECTION_PAGES_MARKER, ShadingPattern, TOTAL_PAGES_MARKER, TabAlignment,
    UnderlineStyle,
};
use open_rtf_converter::rtf::{
    LexError, ParseError, parse_rtf_bytes, parse_rtf_bytes_with_options,
};
use open_rtf_converter::{
    ActiveContentPolicy, ConvertOptions, PdfLinkPolicy, RtfLimits, RtfParseOptions,
    convert_rtf_file_to_pdf, convert_rtf_to_pdf,
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
        "cellx1440 Exact row text",
        "\\",
        "cell",
        "\\",
        "row}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Exact row text"));
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
    assert!(PdfDocument::load_mem(&pdf).is_ok());
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
fn old_style_list_marker_formatting_renders_passively_without_control_leakage() {
    let input =
        br"{\rtf1{\pn\pndec\pnb\pni\pnul\pnstrike\pncaps\pnfs28}Formatted item\par}".to_vec();
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
    assert_eq!(paragraph.runs[0].style.font_size_half_points, 28);
    assert_eq!(paragraph.runs[1].text, "Formatted item");
    assert!(!paragraph.runs[1].style.bold);
    assert!(!paragraph.runs[1].style.italic);
    assert_eq!(paragraph.runs[1].style.underline, UnderlineStyle::None);
    assert!(!paragraph.runs[1].style.strike);

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
fn active_object_data_is_placeholdered_or_rejected_and_never_normalized() {
    let parsed = parse_rtf_bytes(&object_with_payload(false)).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("[Embedded object removed]"));
    assert!(!text.contains(payload_hex()));
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("active content removed") })
    );

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
fn hyperlink_stored_results_render_as_inert_pdf_text_under_passive_link_policies() {
    for policy in [
        PdfLinkPolicy::RenderVisibleTextOnly,
        PdfLinkPolicy::DisableAll,
    ] {
        let output = convert_rtf_to_pdf(
            &field_with_link_result(),
            &ConvertOptions {
                diagnostics: true,
                parse_options: RtfParseOptions {
                    pdf_link_policy: policy,
                    ..RtfParseOptions::default()
                },
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
            .contains("field DATE has no stored result and was not evaluated dynamically")
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
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
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
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("form-field shading approximated by passive highlight rectangles")
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
    assert!(
        rendered_text.contains("q"),
        "checked checkbox should render a passive ZapfDingbats box glyph; got {rendered_text:?}"
    );
    assert!(rendered_text.contains(" After"));
    assert!(
        stroke_count >= 1,
        "checked checkbox should add a passive vector check overlay"
    );
    assert!(
        pdf.windows(b"/BaseFont /ZapfDingbats".len())
            .any(|window| window == b"/BaseFont /ZapfDingbats")
    );
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
    assert!(
        rendered_text.contains("q"),
        "unchecked checkbox should render a passive ZapfDingbats box glyph; got {rendered_text:?}"
    );
    assert!(rendered_text.contains(" After"));
    assert!(
        pdf.windows(b"/BaseFont /ZapfDingbats".len())
            .any(|window| window == b"/BaseFont /ZapfDingbats")
    );
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
    let zapf_bytes = pdf_text_bytes_for_font(&content, b"F14");
    assert!(
        zapf_bytes
            .windows(b"q q 3 7".len())
            .any(|window| window == b"q q 3 7"),
        "Wingdings checkbox glyphs should encode through passive ZapfDingbats bytes, got {zapf_bytes:?}"
    );
    assert!(
        pdf.windows(b"/BaseFont /ZapfDingbats".len())
            .any(|window| window == b"/BaseFont /ZapfDingbats")
    );
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
        "u10003?",
        "\\",
        "u10007?",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("\u{2610}\u{2611}\u{2713}\u{2717}"));
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
    let zapf_bytes = pdf_text_bytes_for_font(&content, b"F14");
    assert!(
        zapf_bytes
            .windows(b"qq37".len())
            .any(|window| window == b"qq37"),
        "Unicode checkbox glyphs should encode through passive ZapfDingbats bytes, got {zapf_bytes:?}"
    );
    assert!(
        output
            .pdf
            .windows(b"/BaseFont /ZapfDingbats".len())
            .any(|window| window == b"/BaseFont /ZapfDingbats")
    );
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
    let zapf_bytes = pdf_text_bytes_for_font(&content, b"F14");

    assert!(
        String::from_utf8_lossy(&helvetica_bytes).contains("Label "),
        "Segoe UI Symbol Latin text should use passive Helvetica bytes, got {helvetica_bytes:?}"
    );
    assert!(
        zapf_bytes.contains(&b'q'),
        "Segoe UI Symbol checkbox should encode through passive ZapfDingbats bytes, got {zapf_bytes:?}"
    );
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
        "f2 Aptos Mono;}}",
        "\\",
        "f0 Sans ",
        "\\",
        "f1 Serif ",
        "\\",
        "f2 Mono",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Sans"));
    assert!(text.contains("Serif"));
    assert!(text.contains("Mono"));
    assert!(!text.contains("fonttbl"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("office-font-substitution.rtf");
    let output_path = dir.path().join("office-font-substitution.pdf");
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
    assert!(text.contains("Before E=mc^2 After"));
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
    assert!(rendered_text.contains("Before E=mc^2 After"));
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
    assert!(text.contains("Before x+1/y After"));
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
    assert!(rendered_text.contains("Before x+1/y After"));
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
    assert!(text.contains("Before x_i After"));
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
    assert!(rendered_text.contains("Before x_i After"));
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
    assert!(text.contains("Before \u{00af}x After"));
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
    assert!(rendered_text.contains("Before "));
    assert!(rendered_text.contains("x After"));
    let helvetica_bytes = pdf_text_bytes_for_font(&content, b"F1");
    assert!(
        helvetica_bytes.contains(&0xaf),
        "macron marker should encode through passive WinAnsi byte 0xaf; got {helvetica_bytes:?}"
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
        text.contains("Before \u{2211}_i=1^ni After"),
        "unexpected n-ary math text: {text:?}"
    );
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
    assert!(rendered_text.contains("_i=1"));
    assert!(rendered_text.contains("^ni After"));
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
        text.contains("Before \u{00af}\u{221a}x_i^2 After"),
        "unexpected property-heavy math text: {text:?}"
    );
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
    assert!(rendered_text.contains("x_i^2 After"));
    let symbol_bytes = pdf_text_bytes_for_font(&content, b"F13");
    assert!(
        symbol_bytes.contains(&0xd6),
        "property-heavy radical should encode through passive Symbol byte 0xd6; got {symbol_bytes:?}"
    );
    let helvetica_bytes = pdf_text_bytes_for_font(&content, b"F1");
    assert!(
        helvetica_bytes.contains(&0xaf),
        "property-heavy overbar should encode through passive WinAnsi byte 0xaf; got {helvetica_bytes:?}"
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
            .contains("capitalized word hyphenation disabled")
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
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("note placement control approximated")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("form-field shading approximated by passive highlight rectangles")
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
    assert!(text.contains("[Image skipped: unsupported format]"));
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
fn wmf_emf_picture_formats_are_passive_placeholders_without_payload_leakage() {
    let payload_hex = "4142432f4a6176615363726970742f456d62656464656446696c65";

    for (control, forbidden_control) in [
        ("wmetafile8", b"wmetafile".as_slice()),
        ("emfblip", b"emfblip".as_slice()),
        ("pmmetafile1", b"pmmetafile".as_slice()),
        ("macpict", b"macpict".as_slice()),
    ] {
        let input = format!("{{\\rtf1 before {{\\pict\\{control} {payload_hex}}} after\\par}}")
            .into_bytes();
        let parsed = parse_rtf_bytes(&input).unwrap();
        let text = collect_text(&parsed.document);

        assert!(text.contains("before"));
        assert!(text.contains("[Image skipped: unsupported format]"));
        assert!(text.contains("after"));
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
        assert!(rendered_text.contains("[Image skipped: unsupported format]"));
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
    assert!(text.contains("cell border"));
    for forbidden in ["brdrhair", "brdrdashdot", "brdrwavy", "brdrdashdd"] {
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

    let parsed =
        parse_rtf_bytes(br"{\rtf1{\*\unknown{\object\objdata 414243}} visible\par}").unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("visible"));
    assert!(!text.contains("414243"));
    assert!(!text.contains("[Embedded object removed]"));
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
            open_rtf_converter::model::Block::Image(_)
            | open_rtf_converter::model::Block::Shape(_) => {}
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
