use super::*;

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
    let parsed = parse_rtf_bytes_strict(&input);
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
            parse_options: RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
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
fn old_style_list_marker_text_reset_clears_stale_styled_runs_without_leakage() {
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
        "blue0;}{",
        "\\",
        "pn",
        "\\",
        "pndec{",
        "\\",
        "pntxta",
        "\\",
        "b",
        "\\",
        "cf1 STALE",
        "\\",
        "tab}{",
        "\\",
        "pntxtb 7}{",
        "\\",
        "pntxta .",
        "\\",
        "tab}}Item",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);

    assert!(text.contains("7.\tItem"));
    for forbidden in ["STALE", "pntxtb", "pntxta", "pndec"] {
        assert!(
            !text.contains(forbidden),
            "forbidden stale marker content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("reset-old-style-list-marker.rtf");
    let output_path = dir.path().join("reset-old-style-list-marker.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            parse_options: RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"STALE".as_slice(),
        b"pntxtb",
        b"pntxta",
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
fn hidden_pending_list_marker_state_does_not_leak_to_next_paragraph() {
    let input = br"{\rtf1{\pn\pndec\pnstart9}\v\par\v0 Visible\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible"));
    assert!(!text.contains("9.\t"));
    assert!(!text.contains("pndec"));

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
    assert!(!rendered_text.contains("9."));

    for forbidden in [
        b"pndec".as_slice(),
        b"pnstart",
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
            "hidden pending marker state leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn structural_old_style_list_markers_do_not_cross_pdf_boundary() {
    let input = br"{\rtf1{\fonttbl{\f0 Arial;{\pntext HiddenFontMarker\tab}}}{\stylesheet{\s1 VisibleStyle;{\pn\pndec\pnstart9{\pntxtb HiddenStyleMarker}{\pntxta .\tab}}}}{\*\listtable{\list{\listlevel\levelnfc0{\leveltext\'02\'00.;}{\levelnumbers;}{\pntext HiddenListMarker\tab}}\listid7}}Visible body\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible body"));
    for forbidden in [
        "HiddenFontMarker",
        "HiddenStyleMarker",
        "HiddenListMarker",
        "9.",
        "pntext",
        "pntxtb",
        "pntxta",
    ] {
        assert!(
            !text.contains(forbidden),
            "structural old-style list marker leaked to text: {forbidden}"
        );
    }

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

    assert!(
        rendered_text.contains("Visible body"),
        "rendered text: {rendered_text:?}"
    );
    for forbidden in [
        b"HiddenFontMarker".as_slice(),
        b"HiddenStyleMarker",
        b"HiddenListMarker",
        b"pntext",
        b"pntxtb",
        b"pntxta",
        b"/Annots",
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
            "structural old-style list marker leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
    audit_passive_pdf_bytes(&output.pdf).unwrap();
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
    let searchable_pdf = pdf_bytes_excluding_embedded_font_programs(&output.pdf);
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
            !searchable_pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden explicit list marker control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_passive_field_inside_explicit_list_marker_stays_marker_text() {
    let input = br#"{\rtf1{\*\listtext{\field{\*\fldinst QUOTE "Q"}}\tab}Body\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected list paragraph"),
    };
    let text = paragraph
        .runs
        .iter()
        .map(|run| run.text.as_str())
        .collect::<String>();

    assert_eq!(text.replace(PASSIVE_ADVANCE_MARKER, ""), "Q\tBody");
    assert_eq!(text.matches(PASSIVE_ADVANCE_MARKER).count(), 2);
    assert!(!text.contains("QUOTE"));
    assert!(!text.contains("fldinst"));
    assert!(
        parsed.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("rendering passive field QUOTE without executing field instruction")),
        "expected passive QUOTE diagnostic: {:?}",
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
        rendered_text.contains("QBody"),
        "decoded PDF text did not contain passive field marker text: {rendered_text:?}"
    );

    for forbidden in [
        b"listtext".as_slice(),
        b"fldinst".as_slice(),
        b"QUOTE",
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
            "forbidden resultless marker field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn passive_advance_field_inside_explicit_list_marker_stays_marker_text() {
    let input =
        br"{\rtf1{\*\listtext{\field{\*\fldinst ADVANCE \\r 240 \\d 120}}1.\tab}Body\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected list paragraph"),
    };

    assert_eq!(paragraph.runs[0].text, PASSIVE_ADVANCE_MARKER);
    assert_eq!(paragraph.runs[0].style.character_spacing_twips, 240);
    assert_eq!(paragraph.runs[0].style.baseline_shift_half_points, -12);
    assert_eq!(paragraph.runs[1].text, "1.\tBody");
    assert_eq!(paragraph.runs[1].style.character_spacing_twips, 0);
    assert_eq!(paragraph.runs[1].style.baseline_shift_half_points, 0);
    let text = collect_text(&parsed.document);
    for forbidden in ["ADVANCE", "fldinst", "\\r", "\\d"] {
        assert!(
            !text.contains(forbidden),
            "ADVANCE field instruction leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("1.Body"));
    assert!(!rendered_text.contains(PASSIVE_ADVANCE_MARKER));

    for forbidden in [
        b"listtext".as_slice(),
        b"ADVANCE",
        b"fldinst".as_slice(),
        PASSIVE_ADVANCE_MARKER.as_bytes(),
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
            "forbidden marker ADVANCE field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_active_field_placeholder_inside_explicit_list_marker_stays_marker_text() {
    let input =
        br#"{\rtf1{\*\listtext{\field{\*\fldinst HYPERLINK "https://example.com"}}\tab}Body\par}"#
            .to_vec();
    let parsed = parse_rtf_bytes_with_options(
        &input,
        &RtfParseOptions {
            active_content_policy: ActiveContentPolicy::Placeholder,
            ..RtfParseOptions::default()
        },
    )
    .unwrap();
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected list paragraph"),
    };
    let text = paragraph
        .runs
        .iter()
        .map(|run| run.text.as_str())
        .collect::<String>();

    assert_eq!(text, "[Field removed: no passive result]\tBody");
    assert!(!text.contains("HYPERLINK"));
    assert!(!text.contains("https://example.com"));
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .all(|block| matches!(block, Block::Paragraph(_))),
        "marker placeholder should not become a standalone block: {:?}",
        parsed.document.blocks
    );

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("[Field removed: no passive result]Body"));
    assert!(!rendered_text.contains("HYPERLINK"));
    assert!(!rendered_text.contains("https://example.com"));

    for forbidden in [
        b"listtext".as_slice(),
        b"HYPERLINK",
        b"https://example.com",
        b"fldinst".as_slice(),
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
            "forbidden marker placeholder field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn picture_placeholder_inside_explicit_list_marker_stays_marker_text() {
    let input = br"{\rtf1{\*\listtext{\pict\pngblip 00}\tab}Body\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected list paragraph"),
    };
    let text = paragraph
        .runs
        .iter()
        .map(|run| run.text.as_str())
        .collect::<String>();

    assert_eq!(text, "[Image skipped: unsupported PNG]\tBody");
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .all(|block| matches!(block, Block::Paragraph(_))),
        "marker picture placeholder should not become a standalone block: {:?}",
        parsed.document.blocks
    );
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unsupported or malformed")),
        "expected malformed PNG diagnostic: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("[Image skipped: unsupported PNG]Body"));

    for forbidden in [
        b"listtext".as_slice(),
        b"pngblip",
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
            "forbidden marker picture content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn shape_text_inside_explicit_list_marker_stays_marker_text() {
    let input =
        br"{\rtf1{\*\listtext{\shp{\shpinst{\shptxt Shape marker\par}}}\tab}Body\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected list paragraph"),
    };
    let text = paragraph
        .runs
        .iter()
        .map(|run| run.text.as_str())
        .collect::<String>();

    assert_eq!(text, "Shape marker\tBody");
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .all(|block| matches!(block, Block::Paragraph(_))),
        "marker shape text should not become standalone model content: {:?}",
        parsed.document.blocks
    );
    assert!(
        parsed.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("safe passive shape text/result")),
        "expected passive shape text diagnostic: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Shape markerBody"));

    for forbidden in [
        b"listtext".as_slice(),
        b"shp",
        b"shpinst",
        b"shptxt",
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
            "forbidden marker shape content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn explicit_list_marker_only_paragraph_does_not_leak_to_next_paragraph() {
    let input = br"{\rtf1{\colortbl;\red255\green0\blue0;}{\*\listtext\b\cf1 1.\tab}\par Next\par}"
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let paragraphs: Vec<_> = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .collect();

    assert_eq!(paragraphs.len(), 2);
    assert_eq!(paragraphs[0].runs[0].text, "1.\t");
    assert!(paragraphs[0].runs[0].style.bold);
    assert_eq!(paragraphs[0].runs[0].style.color_index, 1);
    assert_eq!(paragraphs[1].runs[0].text, "Next");
    assert!(!paragraphs[1].runs[0].style.bold);
    assert_eq!(paragraphs[1].runs[0].style.color_index, 0);

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
    let searchable_pdf = pdf_bytes_excluding_embedded_font_programs(&output.pdf);
    assert!(rendered_text.contains("1."));
    assert!(rendered_text.contains("Next"));

    for forbidden in [
        b"listtext".as_slice(),
        b"cf1",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !searchable_pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden explicit marker-only paragraph content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn named_control_explicit_list_marker_renders_as_marker_without_control_leakage() {
    let input = br"{\rtf1{\*\pntext\bullet\tab}Named bullet marker\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains("\u{2022}\tNamed bullet marker"),
        "named control list marker should precede item text: {text:?}"
    );
    for forbidden in ["pntext"] {
        assert!(
            !text.contains(forbidden),
            "forbidden named list marker control leaked to text: {forbidden}"
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
        rendered_text.contains("Named bullet marker"),
        "decoded PDF text did not contain named list item text: {rendered_text:?}"
    );
    for forbidden in [
        b"pntext".as_slice(),
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
            "forbidden named list marker content leaked to PDF: {:?}",
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
    let searchable_pdf = pdf_bytes_excluding_embedded_font_programs(&output.pdf);
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
            !searchable_pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden mixed explicit list marker control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn old_style_list_format_controls_synthesize_passive_markers_without_leakage() {
    let input = br"{\rtf1{\pn\pnucrm\pnrnot0\pnstart4}Fourth item\par{\pn\pnlcltr\pnrnot0\pnstart28}Lower alpha\par{\pn\pnord\pnrnot0\pnstart13}Ordinal\par{\pn\pnbul\pnrnot0}Bullet\par}".to_vec();
    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);

    assert!(text.contains("IV.\tFourth item"));
    assert!(text.contains("ab.\tLower alpha"));
    assert!(text.contains("13th.\tOrdinal"));
    assert!(text.contains("\u{2022}\tBullet"));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "old-style list metadata should not be unsupported: {:?}",
        parsed.diagnostics
    );
    for forbidden in ["pnucrm", "pnlcltr", "pnord", "pnbul", "pnrnot", "pnstart"] {
        assert!(
            !text.contains(forbidden),
            "forbidden old-style list control leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            parse_options: RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
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
        b"pnrnot",
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
    let parsed = parse_rtf_bytes_strict(&input);
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
            parse_options: RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
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
    let parsed = parse_rtf_bytes_strict(&input);
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
            parse_options: RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
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
    let parsed = parse_rtf_bytes_strict(&input);
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
            parse_options: RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
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
fn old_style_list_marker_underline_variants_render_passively_without_control_leakage() {
    let input = br"{\rtf1{\pn\pndec\pnuldb}Double old marker\par{\pn\pndec\pnuld}Dotted old marker\par{\pn\pndec\pnuldash}Dashed old marker\par{\pn\pndec\pnulwave}Wave old marker\par{\pn\pndec\pnulw}Words old marker\par{\pn\pndec\pnuldb\pnulnone}Plain old marker\par}".to_vec();
    let parsed = parse_rtf_bytes_strict(&input);

    for (index, (text, underline)) in [
        ("Double old marker", UnderlineStyle::Double),
        ("Dotted old marker", UnderlineStyle::Dotted),
        ("Dashed old marker", UnderlineStyle::Dashed),
        ("Wave old marker", UnderlineStyle::Wave),
        ("Words old marker", UnderlineStyle::Words),
    ]
    .into_iter()
    .enumerate()
    {
        let paragraph = match &parsed.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected old-style list paragraph"),
        };
        assert_eq!(paragraph.runs[0].text, "1.\t");
        assert_eq!(paragraph.runs[0].style.underline, underline);
        assert_eq!(paragraph.runs[1].text, text);
        assert_eq!(paragraph.runs[1].style.underline, UnderlineStyle::None);
    }

    let plain = match &parsed.document.blocks[5] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected old-style list paragraph"),
    };
    assert_eq!(plain.runs[0].text, "1.\tPlain old marker");
    assert_eq!(plain.runs[0].style.underline, UnderlineStyle::None);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            parse_options: RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
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
    for expected in [
        "1.Double old marker",
        "1.Dotted old marker",
        "1.Dashed old marker",
        "1.Wave old marker",
        "1.Words old marker",
        "1.Plain old marker",
    ] {
        assert!(
            rendered_text.contains(expected),
            "decoded PDF text did not contain old-style underline marker text {expected:?}: {rendered_text:?}"
        );
    }

    for forbidden in [
        b"pnuldb".as_slice(),
        b"pnuld",
        b"pnuldash",
        b"pnulwave",
        b"pnulw",
        b"pnulnone",
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
            "forbidden old-style list underline control leaked to PDF: {:?}",
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
    let input = br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\levelfollow1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}{\list{\listlevel\levelnfc0\levelstartat1\levelfollow2{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid6}}{\*\listoverridetable{\listoverride\listid5\ls5}{\listoverride\listid6\ls6}}\pard\ls5\ilvl0 Space\par\pard\ls6\ilvl0 Nothing\par}".to_vec();
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
    let input = br"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Courier New;}}{\colortbl;\red255\green0\blue0;\red255\green255\blue0;\red0\green0\blue255;}{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\f1\afs28\ab\ai\aul\ulc3\astrike\acaps\acf1\chshdng5000\chcbpat2{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Styled item\par}".to_vec();
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
        b"afs28",
        b"aul",
        b"ulc",
        b"astrike",
        b"acaps",
        b"acf1",
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
    let input = br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\strikedl{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Double strike item\par}".to_vec();
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
    let input = br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\uldb{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}{\list{\listlevel\levelnfc0\levelstartat1\ulth{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid6}{\list{\listlevel\levelnfc0\levelstartat1\uld{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid7}{\list{\listlevel\levelnfc0\levelstartat1\uldash{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid8}{\list{\listlevel\levelnfc0\levelstartat1\ulwave{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid9}{\list{\listlevel\levelnfc0\levelstartat1\ulw{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid10}}{\*\listoverridetable{\listoverride\listid5\ls1}{\listoverride\listid6\ls2}{\listoverride\listid7\ls3}{\listoverride\listid8\ls4}{\listoverride\listid9\ls5}{\listoverride\listid10\ls6}}\pard\ls1\ilvl0 Double marker\par\pard\ls2\ilvl0 Thick marker\par\pard\ls3\ilvl0 Dotted marker\par\pard\ls4\ilvl0 Dashed marker\par\pard\ls5\ilvl0 Wave marker\par\pard\ls6\ilvl0 Words marker\par}".to_vec();
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
    let input = br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\outl{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}{\list{\listlevel\levelnfc0\levelstartat1\shad{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid6}{\list{\listlevel\levelnfc0\levelstartat1\embo{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid7}{\list{\listlevel\levelnfc0\levelstartat1\impr{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid8}{\list{\listlevel\levelnfc0\levelstartat1\scaps{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid9}{\list{\listlevel\levelnfc0\levelstartat1\ol{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid10}}{\*\listoverridetable{\listoverride\listid5\ls1}{\listoverride\listid6\ls2}{\listoverride\listid7\ls3}{\listoverride\listid8\ls4}{\listoverride\listid9\ls5}{\listoverride\listid10\ls6}}\pard\ls1\ilvl0 Outline marker\par\pard\ls2\ilvl0 Shadow marker\par\pard\ls3\ilvl0 Emboss marker\par\pard\ls4\ilvl0 Engrave marker\par\pard\ls5\ilvl0 Small caps marker\par\pard\ls6\ilvl0 Overline marker\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();

    for (index, text) in [
        "Outline marker",
        "Shadow marker",
        "Emboss marker",
        "Engrave marker",
        "Small caps marker",
        "Overline marker",
    ]
    .into_iter()
    .enumerate()
    {
        let paragraph = match &parsed.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(
            paragraph
                .runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            format!("1.\t{text}")
        );
        let body = paragraph.runs.last().expect("list body run");
        assert!(!body.style.outline);
        assert!(!body.style.shadow);
        assert!(!body.style.overline);
        assert_eq!(body.style.relief, TextRelief::None);
        assert!(!body.style.small_caps);
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
    assert!(first.runs[0].style.outline);
    assert!(second.runs[0].style.shadow);
    assert_eq!(third.runs[0].style.relief, TextRelief::Emboss);
    assert_eq!(fourth.runs[0].style.relief, TextRelief::Engrave);
    assert!(fifth.runs[0].style.small_caps);
    assert!(!sixth.runs[0].style.overline);

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
        "1.Overline marker",
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
fn list_level_marker_emphasis_marks_render_passively_without_control_leakage() {
    let input = br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\accdot{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}{\list{\listlevel\levelnfc0\levelstartat1\acccomma{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid6}{\list{\listlevel\levelnfc0\levelstartat1\accdot\accnone{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid7}}{\*\listoverridetable{\listoverride\listid5\ls1}{\listoverride\listid6\ls2}{\listoverride\listid7\ls3}}\pard\ls1\ilvl0 Dot marker\par\pard\ls2\ilvl0 Comma marker\par\pard\ls3\ilvl0\b Plain marker\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();

    for (index, expected) in [
        CharacterEmphasisMark::Dot,
        CharacterEmphasisMark::Comma,
        CharacterEmphasisMark::None,
    ]
    .into_iter()
    .enumerate()
    {
        let paragraph = match &parsed.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        assert_eq!(paragraph.runs[0].text, "1.\t");
        assert_eq!(paragraph.runs[0].style.emphasis_mark, expected);
        assert_eq!(
            paragraph.runs[1].style.emphasis_mark,
            CharacterEmphasisMark::None
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
    for expected in ["1.Dot marker", "1.Comma marker", "1.Plain marker"] {
        assert!(
            rendered_text.contains(expected),
            "decoded PDF text missing {expected:?}: {rendered_text:?}"
        );
    }

    for forbidden in [
        b"accdot".as_slice(),
        b"acccomma",
        b"accnone",
        b"levelnfc",
        b"leveltext",
        b"levelnumbers",
        b"listtable",
        b"listoverridetable",
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
            "forbidden list-level marker emphasis control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
    audit_passive_pdf_bytes(&output.pdf).unwrap();
}

#[test]
fn list_level_marker_script_and_spacing_render_passively_without_control_leakage() {
    let input = br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\super{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}{\list{\listlevel\levelnfc0\levelstartat1\sub{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid6}{\list{\listlevel\levelnfc0\levelstartat1\up8{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid7}{\list{\listlevel\levelnfc0\levelstartat1\dn6{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid8}{\list{\listlevel\levelnfc0\levelstartat1\expndtw80{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid9}{\list{\listlevel\levelnfc0\levelstartat1\kerning2{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid10}{\list{\listlevel\levelnfc0\levelstartat1\charscalex150{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid11}{\list{\listlevel\levelnfc0\levelstartat1\aup{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid12}{\list{\listlevel\levelnfc0\levelstartat1\adn{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid13}{\list{\listlevel\levelnfc0\levelstartat1\aexpnd4{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid14}}{\*\listoverridetable{\listoverride\listid5\ls1}{\listoverride\listid6\ls2}{\listoverride\listid7\ls3}{\listoverride\listid8\ls4}{\listoverride\listid9\ls5}{\listoverride\listid10\ls6}{\listoverride\listid11\ls7}{\listoverride\listid12\ls8}{\listoverride\listid13\ls9}{\listoverride\listid14\ls10}}\pard\ls1\ilvl0 Raised marker\par\pard\ls2\ilvl0 Lowered marker\par\pard\ls3\ilvl0 Manual up marker\par\pard\ls4\ilvl0 Manual down marker\par\pard\ls5\ilvl0 Spaced marker\par\pard\ls6\ilvl0 Kerned marker\par\pard\ls7\ilvl0 Scaled marker\par\pard\ls8\ilvl0 Associated up marker\par\pard\ls9\ilvl0 Associated down marker\par\pard\ls10\ilvl0 Associated spaced marker\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();

    for (index, text) in [
        "Raised marker",
        "Lowered marker",
        "Manual up marker",
        "Manual down marker",
        "Spaced marker",
        "Kerned marker",
        "Scaled marker",
        "Associated up marker",
        "Associated down marker",
        "Associated spaced marker",
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
    let eighth = match &parsed.document.blocks[7] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    let ninth = match &parsed.document.blocks[8] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    let tenth = match &parsed.document.blocks[9] {
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
    assert_eq!(eighth.runs[0].style.baseline_shift_half_points, 6);
    assert_eq!(ninth.runs[0].style.baseline_shift_half_points, -6);
    assert_eq!(tenth.runs[0].style.character_spacing_twips, 20);

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
        "1.Associated up marker",
        "1.Associated down marker",
        "1.Associated spaced marker",
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
        b"aexpnd",
        b"kerning",
        b"charscalex",
        b"aup",
        b"adn",
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
    let input = br"{\rtf1{\colortbl;\red255\green0\blue0;}{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\chbrdr\brdrdash\brdrw80\brdrcf1\brsp120{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Bordered marker\par}".to_vec();
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
fn list_level_marker_relief_and_compound_borders_render_passively_without_control_leakage() {
    let input = br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\chbrdr\brdrinset\brdrw60{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}{\list{\listlevel\levelnfc0\levelstartat1\chbrdr\brdroutset\brdrw60{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid6}{\list{\listlevel\levelnfc0\levelstartat1\chbrdr\brdrtnthsg\brdrw60{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid7}{\list{\listlevel\levelnfc0\levelstartat1\chbrdr\brdrthtnmg\brdrw60{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid8}{\list{\listlevel\levelnfc0\levelstartat1\chbrdr\brdrtnthtnlg\brdrw60{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid9}}{\*\listoverridetable{\listoverride\listid5\ls1}{\listoverride\listid6\ls2}{\listoverride\listid7\ls3}{\listoverride\listid8\ls4}{\listoverride\listid9\ls5}}\pard\ls1\ilvl0 Engraved marker\par\pard\ls2\ilvl0 Embossed marker\par\pard\ls3\ilvl0 Thin thick marker\par\pard\ls4\ilvl0 Thick thin marker\par\pard\ls5\ilvl0 Thin thick thin marker\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let paragraphs = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();
    let text = collect_text(&parsed.document);

    assert_eq!(paragraphs.len(), 5);
    assert_eq!(
        paragraphs[0].runs[0].style.border.style,
        BorderStyle::Engrave
    );
    assert_eq!(
        paragraphs[1].runs[0].style.border.style,
        BorderStyle::Emboss
    );
    assert_eq!(
        paragraphs[2].runs[0].style.border.style,
        BorderStyle::ThinThick
    );
    assert_eq!(
        paragraphs[3].runs[0].style.border.style,
        BorderStyle::ThickThin
    );
    assert_eq!(
        paragraphs[4].runs[0].style.border.style,
        BorderStyle::ThinThickThin
    );
    for paragraph in &paragraphs {
        assert!(paragraph.runs[0].style.border.visible);
        assert_eq!(paragraph.runs[0].style.border.width_twips, 60);
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("approximated as passive")),
        "list marker border styles should normalize without stale approximation diagnostics: {:?}",
        parsed.diagnostics
    );
    for forbidden in [
        "chbrdr",
        "brdrinset",
        "brdroutset",
        "brdrtnthsg",
        "brdrthtnmg",
        "brdrtnthtnlg",
        "leveltext",
        "levelnumbers",
        "listtable",
        "listoverride",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden list-level marker border control leaked to text: {forbidden}"
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
    audit_passive_pdf_bytes(&output.pdf).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    for expected in [
        "1.Engraved marker",
        "1.Embossed marker",
        "1.Thin thick marker",
        "1.Thick thin marker",
        "1.Thin thick thin marker",
    ] {
        assert!(
            rendered_text.contains(expected),
            "decoded PDF text did not contain passive marker text {expected:?}: {rendered_text:?}"
        );
    }
    for forbidden in [
        b"chbrdr".as_slice(),
        b"brdrinset",
        b"brdroutset",
        b"brdrtnthsg",
        b"brdrthtnmg",
        b"brdrtnthtnlg",
        b"leveltext",
        b"levelnumbers",
        b"listtable",
        b"listoverride",
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
            "forbidden list-level marker border content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn list_level_marker_plain_reset_renders_passively_without_control_leakage() {
    let input = br"{\rtf1{\colortbl;\red255\green0\blue0;}{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\b\ul\chbrdr\brdrs\brdrw80\plain\i\cf1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Reset item\par}".to_vec();
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
    let input = br"{\rtf1{\colortbl;\red255\green0\blue0;}{\stylesheet{\cs5\b\ul\cf1 Marker emphasis;}}{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\i\cs5{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Styled marker\par}".to_vec();
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
fn cardinal_and_ordinal_text_list_markers_render_as_passive_pdf_text() {
    let input = br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc6\levelstartat21{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid21}{\list{\listlevel\levelnfc7\levelstartat22{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid22}{\list{\listlevel\levelnfc6\levelstartat1000{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid23}}{\*\listoverridetable{\listoverride\listid21\ls21}{\listoverride\listid22\ls22}{\listoverride\listid23\ls23}}\pard\ls21\ilvl0 Cardinal text\par\pard\ls22\ilvl0 Ordinal text\par\pard\ls23\ilvl0 Fallback text\par{\pn\pncard\pnstart21}Old cardinal\par{\pn\pnordt\pnstart22}Old ordinal\par}".to_vec();
    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);

    for expected in [
        "twenty-one.\tCardinal text",
        "twenty-second.\tOrdinal text",
        "1000.\tFallback text",
        "twenty-one.\tOld cardinal",
        "twenty-second.\tOld ordinal",
    ] {
        assert!(
            text.contains(expected),
            "normalized text missing {expected:?}: {text:?}"
        );
    }
    for forbidden in ["levelnfc", "leveltext", "levelnumbers", "pncard", "pnordt"] {
        assert!(
            !text.contains(forbidden),
            "list text marker control leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            parse_options: RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
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
    for expected in [
        "twenty-one.Cardinal text",
        "twenty-second.Ordinal text",
        "1000.Fallback text",
        "twenty-one.Old cardinal",
        "twenty-second.Old ordinal",
    ] {
        assert!(
            rendered_text.contains(expected),
            "decoded PDF text missing {expected:?}: {rendered_text:?}"
        );
    }

    for forbidden in [
        b"levelnfc".as_slice(),
        b"leveltext",
        b"levelnumbers",
        b"listtable",
        b"listoverridetable",
        b"pncard",
        b"pnordt",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden text-list marker content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
    audit_passive_pdf_bytes(&output.pdf).unwrap();
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
    let searchable_pdf = pdf_bytes_excluding_embedded_font_programs(&output.pdf);
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
            !searchable_pdf
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
    let visible_text = text.replace(PASSIVE_ADVANCE_MARKER, "");

    assert!(visible_text.contains("Item 1). Sub a). Named 2.. Named next 3.. Reset 7.. Next 8.."));
    assert_eq!(text.matches(PASSIVE_ADVANCE_MARKER).count(), 6);
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

    let strict = parse_rtf_bytes_with_options(
        &input,
        &RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        },
    )
    .unwrap();
    assert!(
        collect_text(&strict.document)
            .contains("Item 1. Sub 1. Named 1. Named next 2. Reset 7. Next 8.")
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
        rendered_text.contains("Item 1). Sub a). Named 2.. Named next 3.. Reset 7.. Next 8.."),
        "decoded PDF text did not contain passive LISTNUM values: {rendered_text:?}"
    );
    for forbidden in [
        b"LISTNUM".as_slice(),
        b"LegalDefault",
        b"fldinst",
        PASSIVE_ADVANCE_MARKER.as_bytes(),
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
fn list_level_template_length_boundary_prevents_trailing_metadata_leakage() {
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
        "'00.;HIDDEN-LIST-PAYLOAD{",
        "\\",
        "object",
        "\\",
        "objdata 414243};}{",
        "\\",
        "levelnumbers",
        "\\",
        "'01;}",
        "\\",
        "levelnumbers HIDDEN-NUMBERS-PAYLOAD}",
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
        "ilvl0 Safe item",
        "\\",
        "par",
        "\\",
        "pard Typed HIDDEN-LIST-PAYLOAD outside",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains("1.\tSafe item"),
        "decoded text did not contain bounded synthesized list marker: {text:?}"
    );
    for forbidden in [
        "HIDDEN-NUMBERS-PAYLOAD",
        "[Embedded object removed]",
        "objdata",
        "414243",
        "leveltext",
        "levelnumbers",
        "listtable",
        "listoverridetable",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden trailing list template metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        text.contains("Typed HIDDEN-LIST-PAYLOAD outside"),
        "literal visible text matching the hidden payload should remain renderable"
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
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
        rendered_text.contains("1.Safe item"),
        "decoded PDF text did not contain bounded synthesized list marker: {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("Typed HIDDEN-LIST-PAYLOAD outside"),
        "visible literal payload text should still render: {rendered_text:?}"
    );
    for forbidden in [
        b"HIDDEN-NUMBERS-PAYLOAD".as_slice(),
        b"[Embedded object removed]",
        b"objdata",
        b"414243",
        b"leveltext",
        b"levelnumbers",
        b"listtable",
        b"listoverridetable",
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
            "forbidden trailing list template metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_styleref_number_switch_renders_list_number_without_payload_leakage() {
    let input = br#"{\rtf1{\stylesheet{\s1 TitleStyle;}}{\*\listtable{\list{\listlevel\levelnfc0{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\s1\ls1\ilvl0 Numbered title\par\pard Ref {\field{\*\fldinst STYLEREF TitleStyle \\n}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("0.\tNumbered title"));
    assert!(text.contains("Ref 0"));
    assert!(!text.contains("Ref Numbered title"));
    for forbidden in [
        "STYLEREF",
        "TitleStyle",
        "listtable",
        "leveltext",
        "levelnumbers",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "STYLEREF number switch leaked unsafe text: {forbidden}"
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

    assert!(rendered_text.contains("Numbered title"));
    assert!(
        rendered_text.contains("Ref 0"),
        "decoded PDF text did not contain passive STYLEREF list number: {rendered_text:?}"
    );
    assert!(!rendered_text.contains("Ref Numbered title"));
    for forbidden in [
        b"STYLEREF".as_slice(),
        b"TitleStyle",
        b"listtable",
        b"leveltext",
        b"levelnumbers",
        b"fldinst",
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
            "forbidden STYLEREF number content leaked to PDF: {:?}",
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
    assert!(
        pdf_text_font_names(&content)
            .iter()
            .any(|name| name.starts_with(b"TF"))
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
fn malformed_wmf_point_list_records_count_as_partial_without_payload_leakage() {
    for (function, label) in [
        (0x0324, "WMF-POLYGON-MALFORMED"),
        (0x0325, "WMF-POLYLINE-MALFORMED"),
        (0x0538, "WMF-POLYPOLYGON-MALFORMED"),
        (0x1005, "WMF-POLYBEZIER-MALFORMED"),
        (0x1004, "WMF-POLYBEZIERTO-MALFORMED"),
    ] {
        let records = [
            wmf_function_record(function),
            wmf_bounds_record(0x041b, 20, 10, 80, 50),
        ];
        let wmf = minimal_wmf_with_records(160, 80, &records);
        let wmf_hex = bytes_to_hex(&wmf);
        let input = format!(
            "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {wmf_hex}}} after\\par}}"
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
            .expect("malformed WMF point-list partial vector preview");

        assert!(text.contains("before"));
        assert!(text.contains("after"));
        assert_eq!(image.format, ImageFormat::WmfVector, "{label}");
        assert!(image.bytes.is_empty(), "{label}");
        assert!(
            image
                .vector_commands
                .iter()
                .any(|command| matches!(command, StaticImageVectorCommand::Rectangle { .. })),
            "safe drawing after {label} should stay visible"
        );
        for forbidden in ["wmetafile", "JavaScript", "EmbeddedFile", "Launch"] {
            assert!(
                !text.contains(forbidden),
                "malformed WMF point-list payload/control leaked to normalized text: {forbidden}"
            );
        }

        let output = convert_rtf_to_pdf(
            &input,
            &ConvertOptions {
                diagnostics: true,
                ..ConvertOptions::browser_safe_defaults()
            },
        )
        .unwrap();
        assert!(
            output.diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("1 unsupported record(s) skipped")),
            "{label} should produce skipped-record diagnostics: {:?}",
            output.diagnostics
        );
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
                .any(|operation| operation.operator == "re"),
            "safe drawing after {label} should render passively"
        );
        for forbidden in [
            b"wmetafile".as_slice(),
            wmf_hex.as_bytes(),
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
                "malformed WMF point-list payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn malformed_emf_point_list_records_count_as_partial_without_payload_leakage() {
    for (record_type, label) in [
        (2, "EMF-POLYBEZIER-MALFORMED"),
        (3, "EMF-POLYGON-MALFORMED"),
        (4, "EMF-POLYLINE-MALFORMED"),
        (5, "EMF-POLYBEZIERTO-MALFORMED"),
        (6, "EMF-POLYLINETO-MALFORMED"),
        (7, "EMF-POLYPOLYLINE-MALFORMED"),
        (8, "EMF-POLYPOLYGON-MALFORMED"),
        (56, "EMF-POLYDRAW-MALFORMED"),
        (85, "EMF-POLYBEZIER16-MALFORMED"),
        (86, "EMF-POLYGON16-MALFORMED"),
        (87, "EMF-POLYLINE16-MALFORMED"),
        (88, "EMF-POLYBEZIERTO16-MALFORMED"),
        (89, "EMF-POLYLINETO16-MALFORMED"),
        (90, "EMF-POLYPOLYLINE16-MALFORMED"),
        (91, "EMF-POLYPOLYGON16-MALFORMED"),
        (92, "EMF-POLYDRAW16-MALFORMED"),
    ] {
        let mut point_record = emf_unknown_record(record_type);
        write_test_le_u32(&mut point_record, 4, 8);
        let records = [point_record, emf_rect_record(43, 20, 10, 80, 50)];
        let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
        let emf_hex = bytes_to_hex(&emf);
        let input =
            format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
            .expect("malformed EMF point-list partial vector preview");

        assert!(text.contains("before"));
        assert!(text.contains("after"));
        assert_eq!(image.format, ImageFormat::WmfVector, "{label}");
        assert!(image.bytes.is_empty(), "{label}");
        assert!(
            image
                .vector_commands
                .iter()
                .any(|command| matches!(command, StaticImageVectorCommand::Rectangle { .. })),
            "safe drawing after {label} should stay visible"
        );
        for forbidden in ["emfblip", "JavaScript", "EmbeddedFile", "Launch"] {
            assert!(
                !text.contains(forbidden),
                "malformed EMF point-list payload/control leaked to normalized text: {forbidden}"
            );
        }

        let output = convert_rtf_to_pdf(
            &input,
            &ConvertOptions {
                diagnostics: true,
                ..ConvertOptions::browser_safe_defaults()
            },
        )
        .unwrap();
        assert!(
            output.diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("1 unsupported record(s) skipped")),
            "{label} should produce skipped-record diagnostics: {:?}",
            output.diagnostics
        );
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
                .any(|operation| operation.operator == "re"),
            "safe drawing after {label} should render passively"
        );
        for forbidden in [
            b"emfblip".as_slice(),
            emf_hex.as_bytes(),
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
                "malformed EMF point-list payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn footnote_page_restart_uses_authored_page_breaks_for_passive_numbering() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ftnrstpg",
        "\\",
        "ftnstart4",
        "\\",
        "ftnnruc First page",
        "\\",
        "chftn{",
        "\\",
        "footnote ",
        "\\",
        "chftn First page footnote",
        "\\",
        "par}",
        "\\",
        "page Second page",
        "\\",
        "chftn{",
        "\\",
        "footnote ",
        "\\",
        "chftn Second page footnote",
        "\\",
        "par}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains(&format!(
            "First page{FOOTNOTE_REFERENCE_MARKER}0{FOOTNOTE_REFERENCE_MARKER_END}"
        )),
        "first page note reference should be a safe layout-resolved marker: {text:?}"
    );
    assert!(
        text.contains(&format!(
            "Second page{FOOTNOTE_REFERENCE_MARKER}1{FOOTNOTE_REFERENCE_MARKER_END}"
        )),
        "second authored page note reference should be a safe layout-resolved marker: {text:?}"
    );
    assert_eq!(parsed.document.footnote_block_indices.len(), 2);
    assert!(
        parsed.document.footnote_block_indices[0] < parsed.document.footnote_block_indices[1],
        "footnote reference block order should be monotonic: {:?}",
        parsed.document.footnote_block_indices
    );
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("footnote page restart")),
        "footnote page restart should be layout-resolved, not reported as approximated: {:?}",
        parsed.diagnostics
    );
    for forbidden in ["ftnrstpg", "ftnstart", "ftnnruc", "chftn"] {
        assert!(
            !text.contains(forbidden),
            "page-restart footnote control leaked to text: {forbidden}"
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
    let page_texts = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| {
            parsed_pdf
                .get_and_decode_page_content(*page_id)
                .map(|content| decoded_pdf_text(&content))
                .unwrap()
        })
        .collect::<Vec<_>>();

    assert_eq!(page_texts.len(), 2);
    assert!(
        page_texts[0].contains("First pageIV"),
        "page texts: {page_texts:?}"
    );
    assert!(
        page_texts[0].contains("IV. First page footnote"),
        "first page footnote label should start at IV: {page_texts:?}"
    );
    assert!(
        page_texts[1].contains("Second pageIV"),
        "page texts: {page_texts:?}"
    );
    assert!(
        page_texts[1].contains("IV. Second page footnote"),
        "second authored page footnote label should restart at IV: {page_texts:?}"
    );
    assert!(
        !page_texts[1].contains("First page footnote"),
        "first page footnote should not duplicate on second page: {page_texts:?}"
    );
    for forbidden in [
        b"ftnrstpg".as_slice(),
        b"ftnstart",
        b"ftnnruc",
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
            "forbidden footnote page-restart content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn section_note_numbering_overrides_render_without_control_leakage() {
    let input = br"{\rtf1\fet2\sftnstart4\sftnnruc\sftnrstcont\saftnstart2\saftnnalc\saftnrstcont Body\chftn{\footnote \chftn Footnote text\par} End\chftn{\footnote\ftnalt \chftn Endnote text\par}\par}";
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
            parsed_pdf
                .get_and_decode_page_content(*page_id)
                .map(|content| decoded_pdf_text(&content))
                .unwrap()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered_text.contains("BodyIV Endb"), "{rendered_text:?}");
    assert!(
        rendered_text.contains("IVFootnote text"),
        "{rendered_text:?}"
    );
    assert!(rendered_text.contains("bEndnote text"), "{rendered_text:?}");
    for forbidden in [
        b"sftnstart".as_slice(),
        b"sftnnruc",
        b"sftnrstcont",
        b"saftnstart",
        b"saftnnalc",
        b"saftnrstcont",
        b"chftn",
        b"footnote",
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
            "section note control leaked to PDF: {:?}",
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

    assert!(
        text.contains(&format!("Body{} Endb", footnote_reference_marker(0))),
        "text: {text:?}"
    );
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
fn section_note_restart_controls_render_passive_section_local_numbering() {
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
        "aftnnalc",
        "\\",
        "aenddoc",
        "\\",
        "ftnrestart",
        "\\",
        "aftnrestart First section",
        "\\",
        "chftn{",
        "\\",
        "footnote ",
        "\\",
        "chftn First section footnote",
        "\\",
        "par} First end",
        "\\",
        "chftn{",
        "\\",
        "endnote ",
        "\\",
        "chftn First section endnote",
        "\\",
        "par}",
        "\\",
        "par",
        "\\",
        "sect Second section",
        "\\",
        "chftn{",
        "\\",
        "footnote ",
        "\\",
        "chftn Second section footnote",
        "\\",
        "par} Second end",
        "\\",
        "chftn{",
        "\\",
        "endnote ",
        "\\",
        "chftn Second section endnote",
        "\\",
        "par}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains(&format!(
            "First section{} First endb",
            footnote_reference_marker(0)
        )),
        "text: {text:?}"
    );
    assert!(
        text.contains(&format!(
            "Second section{} Second endb",
            footnote_reference_marker(1)
        )),
        "text: {text:?}"
    );
    assert_eq!(parsed.document.footnote_section_indices, vec![1, 2]);
    assert_eq!(parsed.document.endnote_section_indices, vec![1, 2]);
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("restart behavior approximated")),
        "section restart should be normalized as safe metadata: {:?}",
        parsed.diagnostics
    );
    for forbidden in ["ftnrestart", "aftnrestart", "chftn"] {
        assert!(
            !text.contains(forbidden),
            "section note restart control leaked into text: {forbidden}"
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
        rendered_text.contains("First sectionIV First endb"),
        "rendered text: {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("Second sectionIV Second endb"),
        "rendered text: {rendered_text:?}"
    );
    assert!(rendered_text.contains("IV. First section footnote"));
    assert!(rendered_text.contains("IV. Second section footnote"));
    assert!(rendered_text.contains("b. First section endnote"));
    assert!(rendered_text.contains("b. Second section endnote"));
    assert!(
        output
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("restart behavior approximated")),
        "section restart should not emit approximation diagnostics: {:?}",
        output.diagnostics
    );
    for forbidden in [
        b"ftnrestart".as_slice(),
        b"aftnrestart",
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
            "section note restart content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}
