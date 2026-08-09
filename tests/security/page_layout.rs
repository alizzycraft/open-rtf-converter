use super::*;

#[test]
fn word_compatible_modern_shape_retains_its_empty_anchor_paragraph() {
    let input = fs::read("fixtures/shape-rotation-passive.rtf").unwrap();
    let parsed = parse_rtf_bytes_with_options(
        &input,
        &RtfParseOptions {
            compatibility_mode: CompatibilityMode::WordCompatiblePassive,
            ..RtfParseOptions::default()
        },
    )
    .unwrap();
    assert!(
        matches!(parsed.document.blocks.as_slice(), [
        Block::Paragraph(_),
        Block::Shape(_),
        Block::Paragraph(anchor),
        Block::Paragraph(_),
    ] if anchor.runs.is_empty()),
        "blocks: {:?}",
        parsed.document.blocks
    );
}

#[test]
fn header_footer_paragraph_growth_is_bounded() {
    let options = RtfParseOptions {
        limits: RtfLimits {
            max_header_footer_paragraphs: 1,
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
                "header First",
                "\\",
                "par Second",
                "\\",
                "par}Body",
                "\\",
                "par}"
            ]),
            &options
        ),
        Err(ParseError::ResourceLimitExceeded { resource, .. })
            if resource == "header/footer paragraphs"
    ));
}

#[test]
fn section_growth_is_bounded() {
    let options = RtfParseOptions {
        limits: RtfLimits {
            max_sections: 1,
            ..RtfLimits::default()
        },
        ..RtfParseOptions::default()
    };

    assert!(matches!(
        parse_rtf_bytes_with_options(
            &rtf(&[
                "{",
                "\\",
                "rtf1 First",
                "\\",
                "sect Second",
                "\\",
                "par}"
            ]),
            &options
        ),
        Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "sections"
    ));
}

#[test]
fn retained_section_setting_blocks_are_bounded_before_layout() {
    let options = RtfParseOptions {
        limits: RtfLimits {
            max_document_blocks: 2,
            ..RtfLimits::default()
        },
        ..RtfParseOptions::default()
    };

    assert!(matches!(
        parse_rtf_bytes_with_options(
            &rtf(&[
                "{",
                "\\",
                "rtf1 First",
                "\\",
                "par",
                "\\",
                "paperw12240 Second",
                "\\",
                "par",
                "\\",
                "paperh15840 Third",
                "\\",
                "par}"
            ]),
            &options
        ),
        Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "document blocks"
    ));
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
fn old_style_marker_only_paragraph_flushes_without_leaking_to_next_paragraph() {
    let input =
        br"{\rtf1{\colortbl;\red255\green0\blue0;}{\pn\pnucrm\pnstart4\pnb\pncf1}\par Next\par}"
            .to_vec();
    let parsed = parse_rtf_bytes_strict(&input);
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
    assert_eq!(paragraphs[0].runs[0].text, "IV.\t");
    assert!(paragraphs[0].runs[0].style.bold);
    assert_eq!(paragraphs[0].runs[0].style.color_index, 1);
    assert_eq!(paragraphs[1].runs[0].text, "Next");
    assert!(!paragraphs[1].runs[0].style.bold);
    assert_eq!(paragraphs[1].runs[0].style.color_index, 0);

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
    assert!(rendered_text.contains("IV."));
    assert!(rendered_text.contains("Next"));

    for forbidden in [
        b"pnucrm".as_slice(),
        b"pnstart",
        b"pncf",
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
            "forbidden old-style marker-only paragraph content leaked to PDF: {:?}",
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

    assert_eq!(parsed_pdf.get_pages().len(), 3);
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
fn page_number_start_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "pgnstart7{",
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
    assert!(parsed.document.page.restart_page_numbering);
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
        "pgnstart4{",
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
        "pgnstarts3",
        "\\",
        "pgnrestart Second",
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
    let section_numbering: Vec<_> = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::SectionSettings(settings) => {
                Some((settings.page_number_start, settings.restart_page_numbering))
            }
            _ => None,
        })
        .collect();

    assert_eq!(section_numbering, vec![(None, true), (None, false)]);
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
        "pgnstarts2",
        "\\",
        "pgnrestart Second",
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

    assert_eq!(parsed.document.page.text_line_grid_twips, Some(360));
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
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("page number position rendered as bounded passive header/footer coordinates")
    }));
    for expected in [
        "section line grid applied as bounded passive paragraph line pitch",
        "section default text grid cleared bounded passive paragraph line pitch",
        "section text grid interpreted through bounded passive paragraph layout",
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
            .contains("section text grid approximated")
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
    let pdf_text = decoded_pdf_text(&content);
    assert!(pdf_text.contains("Visible section grid"));
    assert!(pdf_text.contains("Page 1"));
    let page_number_position =
        pdf_first_text_position_for_text(&content, "Page").expect("page number position");
    assert!(
        (page_number_position.0 - 72.0).abs() < 0.01,
        "expected header page number x at 72pt, got {page_number_position:?}"
    );
    assert!(
        (page_number_position.1 - 744.82).abs() < 0.03,
        "expected source-corrected default-Times header baseline near 744.82pt, got {page_number_position:?}"
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
        PASSIVE_ADVANCE_MARKER.as_bytes(),
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
    assert!(paragraph.runs.iter().all(|run| !run.style.bold));

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
fn arial_source_baseline_recovery_scales_without_changing_passive_output() {
    let position = |input: &[u8], needle: &str| {
        let output = convert_rtf_to_pdf(input, &ConvertOptions::browser_safe_defaults()).unwrap();
        audit_passive_pdf_bytes(&output.pdf).unwrap();
        let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
        let page_id = *parsed_pdf.get_pages().values().next().expect("page");
        let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
        pdf_first_text_position_for_text(&content, needle)
            .unwrap_or_else(|| panic!("missing PDF text position for {needle:?}"))
    };

    let twelve_point = position(
        include_bytes!("../../fixtures/nonvisible-storage-fields-passive.rtf"),
        "Visible",
    );
    let twenty_four_point = position(
        include_bytes!("../../fixtures/paragraph-keep-pagination-passive.rtf"),
        "Filler",
    );

    assert!(
        (twelve_point.1 - 348.70).abs() < 0.03,
        "12-point Arial source baseline should match Word: {twelve_point:?}"
    );
    assert!(
        (twenty_four_point.1 - 139.40).abs() < 0.03,
        "24-point Arial source baseline should scale linearly: {twenty_four_point:?}"
    );
}

#[test]
fn times_source_baseline_recovery_covers_explicit_and_legacy_defaults() {
    let position = |input: &[u8], needle: &str| {
        let output = convert_rtf_to_pdf(input, &ConvertOptions::browser_safe_defaults()).unwrap();
        audit_passive_pdf_bytes(&output.pdf).unwrap();
        let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
        let page_id = *parsed_pdf.get_pages().values().next().expect("page");
        let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
        pdf_first_text_position_for_text(&content, needle)
            .unwrap_or_else(|| panic!("missing PDF text position for {needle:?}"))
    };

    let legacy_default = position(include_bytes!("../../fixtures/table-ish.rtf"), "Name");
    let explicit_times = position(include_bytes!("../../docs/sample.rtf"), "It");

    assert!(
        (legacy_default.1 - 708.82).abs() < 0.03,
        "legacy default Times baseline should match Word: {legacy_default:?}"
    );
    assert!(
        (explicit_times.1 - 706.95).abs() < 0.03,
        "explicit Times New Roman baseline should scale with source size: {explicit_times:?}"
    );
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
            .contains("paragraph hyphenation rendered as bounded passive soft hyphenation")
    }));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("paragraph hyphenation approximated")
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

    assert_eq!(rendered_text, "Antidisestablishmentarianism");
    assert!(
        !rendered_text.contains('-'),
        "dictionary-free emergency wrapping must not invent punctuation: {rendered_text:?}"
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
        "linestarts5",
        "\\",
        "linecont First",
        "\\",
        "par",
        "\\",
        "sect",
        "\\",
        "sectd",
        "\\",
        "linestarts5",
        "\\",
        "linecont Second",
        "\\",
        "par}",
    ]);
    let page_restart = rtf(&[
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
        "linestarts5",
        "\\",
        "lineppage First",
        "\\",
        "par",
        "\\",
        "page Second",
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
        "linestarts5",
        "\\",
        "linecont First",
        "\\",
        "par",
        "\\",
        "sect",
        "\\",
        "sectd",
        "\\",
        "linestarts7",
        "\\",
        "linerestart Second",
        "\\",
        "par}",
    ]);

    for (input, expected_first, expected_second) in [
        (&continuous, "5First", "6Second"),
        (&page_restart, "5First", "5Second"),
        (&section_restart, "5First", "7Second"),
    ] {
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
            rendered_text.contains(expected_first),
            "first section should render passive line number text: {rendered_text:?}"
        );
        assert!(
            rendered_text.contains(expected_second),
            "second section line-number restart mode should render {expected_second:?}: {rendered_text:?}"
        );
        for forbidden in [
            b"linecont".as_slice(),
            b"linerestart",
            b"lineppage",
            b"linemod",
            b"linex",
            b"linestarts",
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
        vec![open_rtf_converter::model::TabLeader::None]
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
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        !decoded_pdf_text(&content).contains('='),
        "Word-compatible equal-sign tab leader should remain nonvisual"
    );
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
        "tb720",
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
    let searchable_pdf = pdf_bytes_excluding_embedded_font_programs(&pdf);
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
            !searchable_pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden bar-tab content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
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
fn soft_line_page_and_column_breaks_are_stripped_without_forcing_pdf_breaks() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before ",
        "\\",
        "softline soft line ",
        "\\",
        "softcol soft column ",
        "\\",
        "softpage soft page",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before soft line soft column soft page"));
    assert!(!text.contains("softline"));
    assert!(!text.contains("softcol"));
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
    assert!(decoded_pdf_text(&content).contains("Before soft line soft column soft page"));
    for forbidden in [
        b"softline".as_slice(),
        b"softcol",
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
        "titlepg\\facingp{",
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
    assert_eq!(paragraph.style.first_line_indent_twips, 0);
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
fn paragraph_keep_controls_paginate_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw6120",
        "\\",
        "paperh3600",
        "\\",
        "margt360",
        "\\",
        "margb360",
        "\\",
        "fs48 Filler one",
        "\\",
        "par Filler two",
        "\\",
        "par Filler three",
        "\\",
        "par ",
        "\\",
        "keep Kept line one",
        "\\",
        "line Kept line two",
        "\\",
        "line Kept line three",
        "\\",
        "par ",
        "\\",
        "keepn Keep next heading",
        "\\",
        "par Follower paragraph",
        "\\",
        "par ",
        "\\",
        "keep0 Plain tail",
        "\\",
        "par} ",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Filler one"));
    assert!(text.contains("Kept line one\nKept line two\nKept line three"));
    assert!(text.contains("Keep next heading"));
    assert!(text.contains("Follower paragraph"));
    assert!(!text.contains("keep"));
    let paragraphs = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(paragraphs[3].style.keep_together);
    assert!(paragraphs[4].style.keep_with_next);
    assert!(!paragraphs[6].style.keep_together);
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "paragraph keep controls should not be unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    audit_passive_pdf_bytes(&output.pdf).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let pages = parsed_pdf.get_pages();
    assert!(
        pages.len() >= 2,
        "kept paragraph should force passive pagination"
    );
    let page_texts = pages
        .values()
        .map(|page_id| {
            let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
            decoded_pdf_text(&content)
        })
        .collect::<Vec<_>>();
    assert!(
        !page_texts[0].contains("Kept line one"),
        "keep-together paragraph should move as a unit: {page_texts:?}"
    );
    assert!(
        page_texts
            .iter()
            .skip(1)
            .any(|page| page.contains("Kept line one") && page.contains("Kept line three")),
        "kept lines should remain together on a later page: {page_texts:?}"
    );
    let keep_next_page = page_texts
        .iter()
        .position(|page| page.contains("Keep next heading"))
        .expect("keep-next heading should render");
    assert!(
        page_texts[keep_next_page].contains("Follower paragraph")
            || page_texts
                .iter()
                .skip(keep_next_page + 1)
                .any(|page| page.contains("Follower paragraph")),
        "keep-next follower should not be lost: {page_texts:?}"
    );
    for forbidden in [
        b"keep".as_slice(),
        b"keepn",
        b"/OpenAction",
        b"/AcroForm",
        b"/Annots",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden paragraph keep content leaked to PDF: {:?}",
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
        "margmirsxn",
        "\\",
        "facingp Odd text",
        "\\",
        "page Even text",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(!parsed.document.page.mirror_margins);
    assert_eq!(parsed.document.page.gutter_twips, 720);
    assert!(text.contains("Odd text"));
    assert!(text.contains("Even text"));
    assert!(!text.contains("facingp"));
    assert!(!text.contains("margmirsxn"));

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
        b"margmirsxn",
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
        "gutterprl",
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
    assert!(!text.contains("gutterprl"));

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
        b"gutterprl",
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
fn word_compatible_overline_controls_are_ignored_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 ",
        "\\",
        "ol over",
        "\\",
        "ol0 plain ",
        "\\",
        "ol again",
        "\\",
        "olnone done",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("over"));
    assert!(text.contains("plain"));
    assert!(text.contains("again"));
    assert!(text.contains("done"));

    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    assert!(paragraph.runs.iter().all(|run| !run.style.overline));
    for forbidden in ["ol", "olnone"] {
        assert!(
            !text.contains(forbidden),
            "forbidden overline control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("overline.rtf");
    let output_path = dir.path().join("overline.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
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
    assert!(rendered_text.contains("over"));
    assert!(rendered_text.contains("plain"));
    assert!(rendered_text.contains("again"));
    assert!(rendered_text.contains("done"));
    assert_eq!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "S")
            .count(),
        0,
        "Word-compatible direct overline controls should not emit strokes"
    );
    for forbidden in [
        b"olnone".as_slice(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden overline content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
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
            .filter(|operation| operation.operator == "re")
            .count()
            >= 4
    );
    assert!(content.operations.iter().any(|operation| {
        operation.operator == "rg"
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
        "brdroutset outset paragraph",
        "\\",
        "par ",
        "\\",
        "pard",
        "\\",
        "brdrl",
        "\\",
        "brdrengrave engrave paragraph",
        "\\",
        "par ",
        "\\",
        "pard",
        "\\",
        "brdrt",
        "\\",
        "brdremboss emboss paragraph",
        "\\",
        "par ",
        "\\",
        "pard",
        "\\",
        "brdrb",
        "\\",
        "brdrtnthsg thin thick paragraph",
        "\\",
        "par ",
        "\\",
        "pard",
        "\\",
        "brdrr",
        "\\",
        "brdrthtnmg thick thin paragraph",
        "\\",
        "par ",
        "\\",
        "pard",
        "\\",
        "brdrt",
        "\\",
        "brdrtnthtnlg thin thick thin paragraph",
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
    assert!(text.contains("outset paragraph"));
    assert!(text.contains("engrave paragraph"));
    assert!(text.contains("emboss paragraph"));
    assert!(text.contains("thin thick paragraph"));
    assert!(text.contains("thick thin paragraph"));
    assert!(text.contains("thin thick thin paragraph"));
    assert!(text.contains("shadow paragraph"));
    assert!(text.contains("cell border"));
    let paragraph_styles = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(&paragraph.style),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        paragraph_styles
            .iter()
            .any(|style| style.borders.right.style == BorderStyle::Engrave)
    );
    assert!(
        paragraph_styles
            .iter()
            .any(|style| style.borders.bottom.style == BorderStyle::Emboss)
    );
    assert!(
        paragraph_styles
            .iter()
            .any(|style| style.borders.left.style == BorderStyle::Engrave)
    );
    assert!(
        paragraph_styles
            .iter()
            .any(|style| style.borders.top.style == BorderStyle::Emboss)
    );
    assert!(
        paragraph_styles
            .iter()
            .any(|style| style.borders.bottom.style == BorderStyle::ThinThick)
    );
    assert!(
        paragraph_styles
            .iter()
            .any(|style| style.borders.right.style == BorderStyle::ThickThin)
    );
    assert!(
        paragraph_styles
            .iter()
            .any(|style| style.borders.top.style == BorderStyle::ThinThickThin)
    );
    assert!(
        paragraph_styles
            .iter()
            .any(|style| style.borders.bottom.style == BorderStyle::Emboss)
    );
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "extended Word border variants should not be unsupported: {:?}",
        parsed.diagnostics
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("Word border effect \\brdrsh rendered as bounded passive relief border")
    }));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("approximated as passive")),
        "extended border styles should normalize without border-style approximation: {:?}",
        parsed.diagnostics
    );
    let layout = LayoutEngine::try_layout(&parsed.document)
        .expect("parsed documents satisfy model invariants");
    let line_styles = layout
        .pages
        .iter()
        .flat_map(|page| page.items.iter())
        .filter_map(|item| match item {
            LayoutItem::Line { style, .. } => Some(*style),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(line_styles.contains(&LineStyle::Engrave));
    assert!(line_styles.contains(&LineStyle::Emboss));
    assert!(line_styles.contains(&LineStyle::ThinThick));
    assert!(line_styles.contains(&LineStyle::ThickThin));
    assert!(line_styles.contains(&LineStyle::ThinThickThin));
    for forbidden in [
        "brdrhair",
        "brdrdashdot",
        "brdrwavy",
        "brdrtriple",
        "brdrinset",
        "brdroutset",
        "brdrengrave",
        "brdremboss",
        "brdrtnthsg",
        "brdrthtnmg",
        "brdrtnthtnlg",
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
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "S")
            .count()
            >= 8,
        "triple border should emit additional passive stroke operations"
    );
    for forbidden in [
        b"brdrhair".as_slice(),
        b"brdrdashdot",
        b"brdrwavy",
        b"brdrtriple",
        b"brdrinset",
        b"brdroutset",
        b"brdrengrave",
        b"brdremboss",
        b"brdrtnthsg",
        b"brdrthtnmg",
        b"brdrtnthtnlg",
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
fn page_border_art_renders_passive_vector_template_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 ",
        "\\",
        "pgbrdrt",
        "\\",
        "brdrart42",
        "\\",
        "brdrw80 Body",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(parsed.document.page.page_borders.top.visible);
    assert_eq!(
        parsed.document.page.page_borders.top.style,
        BorderStyle::BorderArtScissors
    );
    assert_eq!(parsed.document.page.page_borders.top.width_twips, 80);
    assert!(text.contains("Body"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("page border art 42 rendered as bounded passive vector template")
    }));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "page border art should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );
    for forbidden in ["pgbrdrt", "brdrart", "brdrw80"] {
        assert!(
            !text.contains(forbidden),
            "forbidden page border art control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("page-border-art.rtf");
    let output_path = dir.path().join("page-border-art.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
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
        b"brdrart",
        b"brdrw80",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden page border art content leaked to PDF: {:?}",
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
fn framed_drop_cap_uses_only_bounded_passive_flow_geometry() {
    let input = include_bytes!("../../fixtures/framed-drop-cap-passive.rtf");
    let parsed = parse_rtf_bytes(input).unwrap();
    let paragraphs = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(paragraphs[0].style.drop_cap_lines, 3);
    assert_eq!(paragraphs[0].style.frame_width_twips, Some(720));
    assert_eq!(paragraphs[0].style.frame_height_twips, Some(720));
    assert_eq!(paragraphs[2].style.drop_cap_lines, 2);
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| { !diagnostic.message.contains("unsupported RTF control") })
    );

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("framed-drop-cap.rtf");
    let output_path = dir.path().join("framed-drop-cap.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(&input_path, &output_path, &ConvertOptions::default()).unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    for visible in ["D", "First framed", "I", "Second framed", "Plain paragraph"] {
        assert!(rendered_text.contains(visible), "missing {visible:?}");
    }
    for forbidden in ["dropcapli", "dropcapt", "absw", "absh"] {
        assert!(!rendered_text.contains(forbidden));
    }
    for forbidden in [
        b"/JavaScript".as_slice(),
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
        b"/Annots",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn framed_drop_cap_text_distance_remains_bounded_passive_geometry() {
    let input = include_bytes!("../../fixtures/framed-drop-cap-wrap-distance-passive.rtf");
    let parsed = parse_rtf_bytes(input).unwrap();
    let paragraphs = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(paragraphs[0].style.frame_text_distance_twips, 0);
    assert_eq!(paragraphs[2].style.frame_text_distance_twips, 180);
    assert_eq!(paragraphs[4].style.frame_text_distance_twips, 240);
    assert_eq!(paragraphs[6].style.frame_text_distance_twips, 0);

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("framed-drop-cap-distance.rtf");
    let output_path = dir.path().join("framed-drop-cap-distance.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(&input_path, &output_path, &ConvertOptions::default()).unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    for visible in ["No gap", "All gap", "X gap", "Y gap", "sigma tau."] {
        assert!(rendered_text.contains(visible), "missing {visible:?}");
    }
    for forbidden in ["dropcapli", "dropcapt", "dxfrtext", "dfrmtxtx", "dfrmtxty"] {
        assert!(!rendered_text.contains(forbidden));
    }
    for forbidden in [
        b"/JavaScript".as_slice(),
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
        b"/Annots",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn ordinary_frame_text_distance_remains_bounded_passive_geometry() {
    let input = include_bytes!("../../fixtures/paragraph-frame-text-distance-passive.rtf");
    let parsed = parse_rtf_bytes(input).unwrap();
    let frames = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) if paragraph.style.frame_width_twips.is_some() => Some((
                paragraph.style.frame_text_distance_twips,
                paragraph.style.frame_vertical_text_distance_twips,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        frames,
        vec![(0, 0), (180, 180), (240, 0), (0, 360), (240, 360), (0, 0)]
    );

    let layout = LayoutEngine::try_layout(&parsed.document)
        .expect("parsed documents satisfy model invariants");
    assert_eq!(layout.pages.len(), 6);
    let expected_before_x = [36.0, 117.0, 36.0, 108.0, 120.0, 36.0];
    for (page_index, expected_x) in expected_before_x.into_iter().enumerate() {
        let fragment = layout.pages[page_index]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text.starts_with("Before") => Some(fragment),
                _ => None,
            })
            .expect("before-frame fragment");
        assert!((fragment.x - expected_x).abs() < 0.01);
    }

    let output = convert_rtf_to_pdf(input, &ConvertOptions::default()).unwrap();
    audit_passive_pdf_bytes(&output.pdf).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let rendered_text = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| {
            let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
            decoded_pdf_text(&content)
        })
        .collect::<Vec<_>>()
        .join(" ");
    for visible in [
        "No distance frame",
        "All distance frame",
        "Horizontal distance frame",
        "Vertical distance frame",
        "Combined distance frame",
        "Negative distance frame",
    ] {
        assert!(rendered_text.contains(visible), "missing {visible:?}");
    }
    for forbidden in [
        b"dxfrtext".as_slice(),
        b"dfrmtxtx",
        b"dfrmtxty",
        b"absw",
        b"absh",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
        b"/Annots",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn framed_drop_cap_positioning_remains_bounded_passive_geometry() {
    let input = include_bytes!("../../fixtures/framed-drop-cap-positioning-passive.rtf");
    let parsed = parse_rtf_bytes(input).unwrap();
    let paragraphs = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        paragraphs[2].style.frame_horizontal_anchor,
        StaticShapeHorizontalAnchor::Margin
    );
    assert_eq!(paragraphs[2].style.frame_horizontal_offset_twips, 720);
    assert_eq!(
        paragraphs[6].style.frame_horizontal_alignment,
        Some(TableRowAlignment::Center)
    );
    assert_eq!(paragraphs[8].style.frame_vertical_offset_twips, 360);
    assert_eq!(
        paragraphs[10].style.frame_horizontal_alignment,
        Some(TableRowAlignment::Right)
    );

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("framed-drop-cap-positioning.rtf");
    let output_path = dir.path().join("framed-drop-cap-positioning.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(&input_path, &output_path, &ConvertOptions::default()).unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 1);
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    for visible in [
        "Normal alpha",
        "Margin offset alpha",
        "Page offset alpha",
        "Centered alpha",
        "Vertical alpha",
        "Right alpha",
    ] {
        assert!(rendered_text.contains(visible), "missing {visible:?}");
    }
    for forbidden in [
        "dropcapli",
        "dropcapt",
        "phmrg",
        "phpg",
        "posx",
        "posxc",
        "posxr",
        "pvpara",
        "posy",
    ] {
        assert!(!rendered_text.contains(forbidden));
    }
    for forbidden in [
        b"/JavaScript".as_slice(),
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
        b"/Annots",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn framed_drop_cap_vertical_alignments_remain_passive_metadata() {
    let input = include_bytes!("../../fixtures/framed-drop-cap-vertical-alignment-passive.rtf");
    let parsed = parse_rtf_bytes(input).unwrap();
    let paragraphs = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(paragraphs.len(), 8);
    for index in [0, 2, 4, 6] {
        assert_eq!(
            paragraphs[index].style.frame_vertical_anchor,
            StaticShapeVerticalAnchor::Paragraph
        );
        assert_eq!(paragraphs[index].style.frame_vertical_offset_twips, 0);
    }
    assert_eq!(paragraphs[0].style.frame_vertical_alignment, None);
    assert_eq!(
        paragraphs[2].style.frame_vertical_alignment,
        Some(ParagraphFrameVerticalAlignment::Top)
    );
    assert_eq!(
        paragraphs[4].style.frame_vertical_alignment,
        Some(ParagraphFrameVerticalAlignment::Center)
    );
    assert_eq!(
        paragraphs[6].style.frame_vertical_alignment,
        Some(ParagraphFrameVerticalAlignment::Bottom)
    );

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("framed-drop-cap-vertical-alignment.rtf");
    let output_path = dir.path().join("framed-drop-cap-vertical-alignment.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(&input_path, &output_path, &ConvertOptions::default()).unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 1);
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    for visible in ["Zero alpha", "Tops alpha", "Cent alpha", "Bott alpha"] {
        assert!(rendered_text.contains(visible), "missing {visible:?}");
    }
    for forbidden in ["dropcapli", "dropcapt", "pvpara", "posyt", "posyc", "posyb"] {
        assert!(!rendered_text.contains(forbidden));
    }
    for forbidden in [
        b"/JavaScript".as_slice(),
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
        b"/Annots",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn ordinary_frame_vertical_alignments_remain_bounded_passive_geometry() {
    let input = include_bytes!("../../fixtures/paragraph-frame-vertical-alignment-passive.rtf");
    let parsed = parse_rtf_bytes(input).unwrap();
    let frames = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) if paragraph.style.frame_width_twips.is_some() => Some((
                paragraph.style.frame_vertical_anchor,
                paragraph.style.frame_vertical_alignment,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(frames.len(), 7);
    assert_eq!(
        frames.iter().map(|frame| frame.1).collect::<Vec<_>>(),
        vec![
            Some(ParagraphFrameVerticalAlignment::Top),
            Some(ParagraphFrameVerticalAlignment::Center),
            Some(ParagraphFrameVerticalAlignment::Bottom),
            Some(ParagraphFrameVerticalAlignment::Top),
            Some(ParagraphFrameVerticalAlignment::Center),
            Some(ParagraphFrameVerticalAlignment::Bottom),
            Some(ParagraphFrameVerticalAlignment::Center),
        ]
    );

    let layout = LayoutEngine::try_layout(&parsed.document)
        .expect("parsed documents satisfy model invariants");
    assert_eq!(layout.pages.len(), 7);
    let frame_baseline = |page_index: usize| {
        layout.pages[page_index]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text.contains("frame.") => {
                    Some(fragment.baseline_y)
                }
                _ => None,
            })
            .expect("frame text baseline")
    };
    assert!(frame_baseline(0) > frame_baseline(1));
    assert!(frame_baseline(1) > frame_baseline(2));
    assert!(frame_baseline(3) > frame_baseline(4));
    assert!(frame_baseline(4) > frame_baseline(5));

    let output = convert_rtf_to_pdf(input, &ConvertOptions::default()).unwrap();
    audit_passive_pdf_bytes(&output.pdf).unwrap();
    for forbidden in [
        b"posyt".as_slice(),
        b"posyc",
        b"posyb",
        b"pvpg",
        b"pvmrg",
        b"pvpara",
        b"absw",
        b"absh",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
        b"/Annots",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn ordinary_frame_vertical_precedence_and_edges_remain_bounded_passive() {
    let input = include_bytes!("../../fixtures/paragraph-frame-vertical-precedence-passive.rtf");
    let parsed = parse_rtf_bytes(input).unwrap();
    let alignments = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) if paragraph.style.frame_width_twips.is_some() => {
                Some(paragraph.style.frame_vertical_alignment)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        alignments,
        vec![
            None,
            Some(ParagraphFrameVerticalAlignment::Center),
            None,
            Some(ParagraphFrameVerticalAlignment::Bottom),
            Some(ParagraphFrameVerticalAlignment::Inside),
            Some(ParagraphFrameVerticalAlignment::Inside),
            Some(ParagraphFrameVerticalAlignment::Outside),
            Some(ParagraphFrameVerticalAlignment::Outside),
            Some(ParagraphFrameVerticalAlignment::Inline),
            Some(ParagraphFrameVerticalAlignment::Inline),
        ]
    );

    let layout = LayoutEngine::try_layout(&parsed.document)
        .expect("parsed documents satisfy model invariants");
    assert_eq!(layout.pages.len(), 10);
    let frame_baseline = |page_index: usize| {
        layout.pages[page_index]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text.contains("frame.") => {
                    Some(fragment.baseline_y)
                }
                _ => None,
            })
            .expect("frame baseline")
    };
    assert!(frame_baseline(4) > frame_baseline(5));
    assert!((frame_baseline(5) - frame_baseline(6)).abs() < 0.01);
    assert!(frame_baseline(7) > frame_baseline(6));
    assert!(frame_baseline(8) > frame_baseline(9));

    let output = convert_rtf_to_pdf(input, &ConvertOptions::default()).unwrap();
    audit_passive_pdf_bytes(&output.pdf).unwrap();
    for forbidden in [
        b"posyin".as_slice(),
        b"posyout",
        b"posyil",
        b"posyt",
        b"posyc",
        b"posyb",
        b"posy1440",
        b"pvpg",
        b"pvmrg",
        b"absw",
        b"absh",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
        b"/Annots",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn absolute_drop_cap_frame_reflow_remains_bounded_passive_geometry() {
    let input = include_bytes!("../../fixtures/framed-drop-cap-absolute-overlap-passive.rtf");
    let parsed = parse_rtf_bytes(input).unwrap();
    let paragraphs = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(paragraphs.len(), 7);
    assert_eq!(
        paragraphs[1].style.frame_horizontal_anchor,
        StaticShapeHorizontalAnchor::Margin
    );
    assert_eq!(
        paragraphs[1].style.frame_vertical_anchor,
        StaticShapeVerticalAnchor::Margin
    );
    assert_eq!(
        paragraphs[5].style.frame_horizontal_anchor,
        StaticShapeHorizontalAnchor::Page
    );
    assert_eq!(
        paragraphs[5].style.frame_vertical_anchor,
        StaticShapeVerticalAnchor::Page
    );

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("framed-drop-cap-absolute-overlap.rtf");
    let output_path = dir.path().join("framed-drop-cap-absolute-overlap.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(&input_path, &output_path, &ConvertOptions::default()).unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 2);
    let rendered_text = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| {
            let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
            decoded_pdf_text(&content)
        })
        .collect::<String>();
    for visible in [
        "Margin prior",
        "Margin after",
        "Page first prior",
        "Page prior",
        "Page after",
    ] {
        assert!(rendered_text.contains(visible), "missing {visible:?}");
    }
    for forbidden in [
        "dropcapli",
        "dropcapt",
        "phmrg",
        "phpg",
        "pvmrg",
        "pvpg",
        "posx",
        "posy",
    ] {
        assert!(!rendered_text.contains(forbidden));
    }
    for forbidden in [
        b"/JavaScript".as_slice(),
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
        b"/Annots",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn naturally_paginated_absolute_frame_remains_page_scoped_and_passive() {
    let input = include_bytes!("../../fixtures/framed-drop-cap-natural-pagination-passive.rtf");
    let dir = tempdir().unwrap();
    let input_path = dir.path().join("framed-drop-cap-natural-pagination.rtf");
    let output_path = dir.path().join("framed-drop-cap-natural-pagination.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(&input_path, &output_path, &ConvertOptions::default()).unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 2);
    let page_text = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| {
            let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
            decoded_pdf_text(&content)
        })
        .collect::<Vec<_>>();
    assert!(page_text[0].contains("Before A01"));
    assert!(page_text[0].contains("A73 A74 A75"));
    assert!(!page_text[0].contains("A76"));
    assert!(page_text[1].contains("A76 A77 A78 A79 A80."));
    assert!(page_text[1].contains("After natural pagination."));
    for forbidden in ["dropcapli", "dropcapt", "phpg", "pvpg", "posx", "posy"] {
        assert!(!page_text.iter().any(|text| text.contains(forbidden)));
    }
    for forbidden in [
        b"/JavaScript".as_slice(),
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
        b"/Annots",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn facing_absolute_frame_alignment_remains_passive_page_geometry() {
    let input = include_bytes!("../../fixtures/framed-drop-cap-facing-alignment-passive.rtf");
    let parsed = parse_rtf_bytes(input).unwrap();
    assert!(parsed.document.page.facing_pages);
    assert!(!parsed.document.page.mirror_margins);
    let frames = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) if paragraph.style.drop_cap_lines > 1 => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(frames.len(), 4);
    assert_eq!(
        frames[0].style.frame_horizontal_alignment,
        Some(TableRowAlignment::Inside)
    );
    assert_eq!(
        frames[1].style.frame_horizontal_alignment,
        Some(TableRowAlignment::Outside)
    );

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("framed-drop-cap-facing-alignment.rtf");
    let output_path = dir.path().join("framed-drop-cap-facing-alignment.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(&input_path, &output_path, &ConvertOptions::default()).unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 2);
    let rendered_text = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| {
            let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
            decoded_pdf_text(&content)
        })
        .collect::<String>();
    for visible in ["Odd inside", "Odd outside", "Even inside", "Even outside"] {
        assert!(rendered_text.contains(visible), "missing {visible:?}");
    }
    for forbidden in [
        "dropcapli",
        "dropcapt",
        "facingp",
        "phmrg",
        "pvpg",
        "posxi",
        "posxo",
        "posy",
    ] {
        assert!(!rendered_text.contains(forbidden));
    }
    for forbidden in [
        b"/JavaScript".as_slice(),
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
        b"/Annots",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn signed_absolute_frame_offsets_remain_bounded_passive_geometry() {
    let input = include_bytes!("../../fixtures/framed-drop-cap-negative-positioning-passive.rtf");
    let parsed = parse_rtf_bytes(input).unwrap();
    let frames = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) if paragraph.style.drop_cap_lines > 1 => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(frames.len(), 8);
    assert_eq!(frames[0].style.frame_horizontal_offset_twips, 360);
    assert_eq!(frames[5].style.frame_vertical_offset_twips, 360);
    assert_eq!(frames[6].style.frame_horizontal_offset_twips, -360);
    assert_eq!(frames[7].style.frame_vertical_offset_twips, -360);

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("framed-drop-cap-signed-positioning.rtf");
    let output_path = dir.path().join("framed-drop-cap-signed-positioning.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(&input_path, &output_path, &ConvertOptions::default()).unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 4);
    let rendered_text = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| {
            let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
            decoded_pdf_text(&content)
        })
        .collect::<String>();
    for visible in [
        "Signed positive x",
        "Last signed x",
        "Last positive x",
        "Signed positive y",
        "Page signed positive",
        "Signed negative x",
        "Signed negative y",
    ] {
        assert!(rendered_text.contains(visible), "missing {visible:?}");
    }
    for forbidden in [
        "dropcapli",
        "dropcapt",
        "phmrg",
        "phpg",
        "pvmrg",
        "pvpg",
        "posx",
        "posy",
        "posnegx",
        "posnegy",
    ] {
        assert!(!rendered_text.contains(forbidden));
    }
    for forbidden in [
        b"/JavaScript".as_slice(),
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
        b"/Annots",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn signed_frame_dimensions_remain_bounded_passive_geometry() {
    let input = include_bytes!("../../fixtures/framed-drop-cap-signed-dimensions-passive.rtf");
    let parsed = parse_rtf_bytes(input).unwrap();
    let frames = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) if paragraph.style.drop_cap_lines > 1 => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(frames.len(), 8);
    assert_eq!(frames[1].style.frame_width_twips, Some(-720));
    assert_eq!(frames[2].style.frame_height_twips, Some(-720));
    assert_eq!(frames[4].style.frame_height_twips, None);
    assert_eq!(frames[5].style.frame_height_twips, Some(1_440));
    assert_eq!(frames[6].style.frame_width_twips, Some(720));
    assert_eq!(frames[7].style.frame_width_twips, Some(-720));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("framed-drop-cap-signed-dimensions.rtf");
    let output_path = dir.path().join("framed-drop-cap-signed-dimensions.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(&input_path, &output_path, &ConvertOptions::default()).unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 8);
    let page_text = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| {
            let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
            decoded_pdf_text(&content)
        })
        .collect::<Vec<_>>();
    for visible in [
        "Positive width positive height",
        "Negative width positive height",
        "Positive width negative height",
        "Negative width negative height",
        "Negative width only",
        "Negative width tall height",
        "Last positive width",
        "Last negative width",
    ] {
        assert!(page_text.iter().any(|text| text.contains(visible)));
    }
    assert!(!page_text[4].contains('O'));
    for forbidden in ["dropcapli", "dropcapt", "absw", "absh"] {
        assert!(!page_text.iter().any(|text| text.contains(forbidden)));
    }
    for forbidden in [
        b"/JavaScript".as_slice(),
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
        b"/Annots",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn paragraph_frame_collision_controls_remain_passive_geometry() {
    let input = include_bytes!("../../fixtures/framed-drop-cap-collision-controls-passive.rtf");
    let parsed = parse_rtf_bytes(input).unwrap();
    for control in ["absnoovrlp", "abslock"] {
        assert!(parsed.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(&format!(
                "paragraph frame control \\{control} retained through bounded passive placement"
            ))
        }));
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("framed-drop-cap-collision-controls.rtf");
    let output_path = dir.path().join("framed-drop-cap-collision-controls.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(&input_path, &output_path, &ConvertOptions::default()).unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 3);
    let rendered_text = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| {
            let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
            decoded_pdf_text(&content)
        })
        .collect::<String>();
    for visible in ["Default overlap", "No overlap", "Locked overlap"] {
        assert!(rendered_text.contains(visible));
    }
    for forbidden in [
        "dropcapli",
        "dropcapt",
        "absw",
        "absh",
        "absnoovrlp",
        "abslock",
    ] {
        assert!(!rendered_text.contains(forbidden));
    }
    for forbidden in [
        b"/JavaScript".as_slice(),
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
        b"/Annots",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn unequal_paragraph_frame_intersections_remain_passive_geometry() {
    let input = include_bytes!("../../fixtures/framed-drop-cap-overlap-geometry-passive.rtf");
    let parsed = parse_rtf_bytes(input).unwrap();
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "paragraph frame control \\absnoovrlp retained through bounded passive placement",
        )
    }));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("framed-drop-cap-overlap-geometry.rtf");
    let output_path = dir.path().join("framed-drop-cap-overlap-geometry.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(&input_path, &output_path, &ConvertOptions::default()).unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 5);
    let rendered_text = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| {
            let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
            decoded_pdf_text(&content)
        })
        .collect::<String>();
    for visible in [
        "Unequal same origin",
        "Horizontal intersection",
        "Vertical intersection",
        "Separated frames",
        "No-overlap unequal",
    ] {
        assert!(rendered_text.contains(visible));
    }
    for forbidden in [
        "dropcapli",
        "dropcapt",
        "absw",
        "absh",
        "absnoovrlp",
        "phpg",
        "pvpg",
        "posx",
        "posy",
    ] {
        assert!(!rendered_text.contains(forbidden));
    }
    for forbidden in [
        b"/JavaScript".as_slice(),
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
        b"/Annots",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn ordinary_paragraph_frames_remain_passive_grouped_geometry() {
    let input = include_bytes!("../../fixtures/paragraph-frame-passive.rtf");
    let parsed = parse_rtf_bytes(input).unwrap();
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("paragraph frame dimensions interpreted through bounded passive flow layout")
    }));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("paragraph-frame-passive.rtf");
    let output_path = dir.path().join("paragraph-frame-passive.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(&input_path, &output_path, &ConvertOptions::default()).unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 3);
    let rendered_text = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| {
            let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
            decoded_pdf_text(&content)
        })
        .collect::<String>();
    for visible in [
        "Before single frame",
        "Framed one",
        "After single frame",
        "Before grouped frame",
        "Group first",
        "Group second",
        "After grouped frame",
        "Before derived frame",
        "Derived frame",
        "After derived frame",
    ] {
        assert!(rendered_text.contains(visible));
    }
    for forbidden in ["absw", "absh", "phpg", "phmrg", "pvpg", "posx", "posy"] {
        assert!(!rendered_text.contains(forbidden));
    }
    for forbidden in [
        b"/JavaScript".as_slice(),
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
        b"/Annots",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn ordinary_frame_exact_height_and_in_flow_recovery_remain_passive() {
    let input = include_bytes!("../../fixtures/paragraph-frame-height-flow-passive.rtf");
    let parsed = parse_rtf_bytes(input).unwrap();
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("paragraph frame dimensions interpreted through bounded passive flow layout")
    }));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("paragraph-frame-height-flow-passive.rtf");
    let output_path = dir.path().join("paragraph-frame-height-flow-passive.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(&input_path, &output_path, &ConvertOptions::default()).unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 6);
    let page_text = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| {
            let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
            decoded_pdf_text(&content)
        })
        .collect::<Vec<_>>();
    for visible in [
        "Before positive short frame",
        "After positive short frame",
        "Before negative short frame",
        "After negative short frame",
        "Explicit paragraph frame",
        "Default anchor frame",
        "Width only default frame",
        "Before negative width ordinary frame",
        "After negative width ordinary frame",
    ] {
        assert!(page_text.iter().any(|text| text.contains(visible)));
    }
    assert!(page_text[5].contains('N'));
    assert!(!page_text[5].contains("Negative width ordinary alpha"));
    for forbidden in [
        "absw", "absh", "phpg", "phcol", "pvpg", "pvpara", "posx", "posy",
    ] {
        assert!(!page_text.iter().any(|text| text.contains(forbidden)));
    }
    for forbidden in [
        b"/JavaScript".as_slice(),
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
        b"/Annots",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn paragraph_frame_text_direction_remains_passive_rotated_text() {
    let input = include_bytes!("../../fixtures/paragraph-frame-text-direction-passive.rtf");
    let parsed = parse_rtf_bytes(input).unwrap();
    for control in [
        "frmtxlrtb",
        "frmtxtbrl",
        "frmtxbtlr",
        "frmtxlrtbv",
        "frmtxtbrlv",
    ] {
        assert!(parsed.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(&format!(
                "paragraph frame text direction \\{control} interpreted through bounded passive rotation"
            ))
        }));
    }

    let layout = LayoutEngine::try_layout(&parsed.document)
        .expect("parsed documents satisfy model invariants");
    let rotations = layout
        .pages
        .iter()
        .map(|page| {
            page.items
                .iter()
                .filter_map(|item| match item {
                    LayoutItem::Text(fragment) => Some(fragment.rotation),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert!(rotations[2].contains(&open_rtf_converter::layout::TextRotation::Clockwise90));
    assert!(rotations[3].contains(&open_rtf_converter::layout::TextRotation::CounterClockwise90));
    assert!(rotations[5].contains(&open_rtf_converter::layout::TextRotation::Clockwise90));

    let dir = tempdir().unwrap();
    let input_path = dir
        .path()
        .join("paragraph-frame-text-direction-passive.rtf");
    let output_path = dir
        .path()
        .join("paragraph-frame-text-direction-passive.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(&input_path, &output_path, &ConvertOptions::default()).unwrap();
    let pdf = fs::read(&output_path).unwrap();
    audit_passive_pdf_bytes(&pdf).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 6);
    let rendered_text = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| {
            let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
            decoded_pdf_text(&content)
        })
        .collect::<String>();
    for visible in [
        "Default Alpha Beta",
        "Horizontal Alpha Beta",
        "Top Bottom Alpha Beta",
        "Bottom Top Alpha Beta",
        "Vertical LR Alpha Beta",
        "Vertical RL Alpha Beta",
    ] {
        assert!(rendered_text.contains(visible));
    }
    for forbidden in [
        "frmtxlrtb",
        "frmtxtbrl",
        "frmtxbtlr",
        "frmtxlrtbv",
        "frmtxtbrlv",
        "absw",
        "absh",
    ] {
        assert!(!rendered_text.contains(forbidden));
    }
    for forbidden in [
        b"/JavaScript".as_slice(),
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
        b"/Annots",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
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

    assert!(!parsed.document.page.page_border_from_page_edge);
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
fn office_shape_full_fill_and_line_opacity_are_consumed_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillColor}{\sv 13434879}}{\sp{\sn fillOpacity}{\sv 65536}}{\sp{\sn lineColor}{\sv 255}}{\sp{\sn lineOpacity}{\sv 65536}}}}After\par}"#.to_vec();
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
        .expect("passive shape with full fill/line opacity");

    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert!(shape.fill_color.is_some());
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unsupported/active drawing properties")),
        "full fillOpacity/lineOpacity should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in ["fillOpacity", "lineOpacity", "shapeType"] {
        assert!(
            !text.contains(forbidden),
            "full opacity metadata leaked to normalized text: {forbidden}"
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
    audit_passive_pdf_bytes(&output.pdf).unwrap();
    for forbidden in [
        b"fillOpacity".as_slice(),
        b"lineOpacity",
        b"shapeType",
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
            "full opacity metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn partial_office_shape_fill_and_line_opacity_render_passively_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillColor}{\sv 13434879}}{\sp{\sn fillOpacity}{\sv 32768}}{\sp{\sn lineColor}{\sv 255}}{\sp{\sn lineOpacity}{\sv 16384}}{\sp{\sn pFragments}{\sv hidden-partial-shape-opacity-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with partial fill/line opacity");

    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert!(shape.fill_color.is_some());
    assert_eq!(shape.fill_opacity_percent, 50);
    assert_eq!(shape.stroke_opacity_percent, 25);
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("stripping unsupported/active drawing properties")
    }));
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "fillOpacity",
        "lineOpacity",
        "32768",
        "16384",
        "pFragments",
        "hidden-partial-shape-opacity-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "partial fill/line opacity metadata leaked to normalized text: {forbidden}"
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
    audit_passive_pdf_bytes(&output.pdf).unwrap();
    let layout = LayoutEngine::try_layout(&parsed.document)
        .expect("parsed documents satisfy model invariants");
    assert!(
        layout
            .pages
            .iter()
            .flat_map(|page| page.items.iter())
            .any(|item| matches!(
                item,
                LayoutItem::Opacity {
                    fill_percent: 50,
                    stroke_percent: 25,
                    ..
                }
            )),
        "partial Office shape opacity should wrap passive geometry"
    );
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "gs"),
        "partial Office shape opacity should emit passive PDF graphics state"
    );
    for forbidden in [
        b"fillOpacity".as_slice(),
        b"lineOpacity",
        b"32768",
        b"16384",
        b"pFragments",
        b"hidden-partial-shape-opacity-payload",
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
            "partial fill/line opacity metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn oversized_office_shape_line_color_is_stripped_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn lineColor}{\sv 4294967295}}{\sp{\sn pFragments}{\sv hidden-invalid-line-color-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with invalid line color");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert_eq!(
        shape.stroke_color,
        open_rtf_converter::model::Color::default()
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("stripping unsupported/active drawing properties")
    }));
    for forbidden in [
        "shapeType",
        "lineColor",
        "pFragments",
        "hidden-invalid-line-color-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "invalid line color metadata leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "S"),
        "shape outline should still render with default passive stroke color"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"lineColor",
        b"pFragments",
        b"hidden-invalid-line-color-payload",
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
            "invalid line color metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_line_fore_color_is_inert_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn lineForeColor}{\sv 255}}{\sp{\sn pFragments}{\sv hidden-line-fore-color-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with lineForeColor");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert_eq!(
        shape.stroke_color,
        open_rtf_converter::model::Color {
            red: 0,
            green: 0,
            blue: 0,
        }
    );
    for forbidden in [
        "shapeType",
        "lineForeColor",
        "pFragments",
        "hidden-line-fore-color-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "lineForeColor metadata leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "S"),
        "Word's default black outline should remain visible"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"lineForeColor",
        b"pFragments",
        b"hidden-line-fore-color-payload",
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
            "lineForeColor metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn invalid_office_shape_line_fore_color_is_stripped_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn lineForeColor}{\sv 4294967295}}{\sp{\sn pFragments}{\sv hidden-invalid-line-fore-color-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with invalid lineForeColor");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert_eq!(
        shape.stroke_color,
        open_rtf_converter::model::Color::default()
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("stripping unsupported/active drawing properties")
    }));
    for forbidden in [
        "shapeType",
        "lineForeColor",
        "pFragments",
        "hidden-invalid-line-fore-color-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "invalid lineForeColor metadata leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "S"),
        "shape outline should still render with default passive stroke color"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"lineForeColor",
        b"pFragments",
        b"hidden-invalid-line-fore-color-payload",
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
            "invalid lineForeColor metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_line_back_color_is_inert_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn lineBackColor}{\sv 16711680}}{\sp{\sn pFragments}{\sv hidden-line-back-color-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with lineBackColor");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert_eq!(
        shape.stroke_color,
        open_rtf_converter::model::Color {
            red: 0,
            green: 0,
            blue: 0,
        }
    );
    for forbidden in [
        "shapeType",
        "lineBackColor",
        "pFragments",
        "hidden-line-back-color-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "lineBackColor metadata leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "S"),
        "Word's default black outline should remain visible"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"lineBackColor",
        b"pFragments",
        b"hidden-line-back-color-payload",
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
            "lineBackColor metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_line_color_aliases_leave_word_default_outline() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn lineBackColor}{\sv 16711680}}{\sp{\sn lineForeColor}{\sv 255}}{\sp{\sn pFragments}{\sv hidden-line-color-precedence-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with line color precedence");

    assert_eq!(
        shape.stroke_color,
        open_rtf_converter::model::Color {
            red: 0,
            green: 0,
            blue: 0,
        },
        "Word ignores both line-color aliases"
    );
    for forbidden in [
        "lineBackColor",
        "lineForeColor",
        "pFragments",
        "hidden-line-color-precedence-payload",
    ] {
        assert!(
            !text.contains(forbidden),
            "line color precedence metadata leaked to normalized text: {forbidden}"
        );
    }
}

#[test]
fn invalid_office_shape_line_back_color_is_stripped_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn lineBackColor}{\sv -1}}{\sp{\sn pFragments}{\sv hidden-invalid-line-back-color-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with invalid lineBackColor");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert_eq!(
        shape.stroke_color,
        open_rtf_converter::model::Color::default()
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("stripping unsupported/active drawing properties")
    }));
    for forbidden in [
        "shapeType",
        "lineBackColor",
        "pFragments",
        "hidden-invalid-line-back-color-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "invalid lineBackColor metadata leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "S"),
        "shape outline should still render with default passive stroke color"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"lineBackColor",
        b"pFragments",
        b"hidden-invalid-line-back-color-payload",
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
            "invalid lineBackColor metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn rotated_office_lines_lower_to_passive_polyline_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 20}}{\sp{\sn lineColor}{\sv 255}}{\sp{\sn lineStartArrowhead}{\sv open}}{\sp{\sn lineEndArrowhead}{\sv triangle}}{\sp{\sn rotation}{\sv 2700000}}{\sp{\sn pFragments}{\sv hidden-rotated-line-payload}}}}After\par}"#.to_vec();
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
        .expect("rotated passive line shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polyline);
    assert_eq!(shape.points.len(), 2);
    assert_ne!(shape.points[0].x_twips, shape.points[1].x_twips);
    assert_ne!(shape.points[0].y_twips, shape.points[1].y_twips);
    assert_eq!(shape.start_arrowhead, StaticShapeArrowhead::Open);
    assert_eq!(shape.end_arrowhead, StaticShapeArrowhead::Triangle);
    for forbidden in [
        "rotation",
        "lineStartArrowhead",
        "lineEndArrowhead",
        "pFragments",
        "hidden-rotated-line-payload",
    ] {
        assert!(
            !text.contains(forbidden),
            "rotated line metadata leaked to normalized text: {forbidden}"
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
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("shape rotation rendered as bounded passive static geometry")
    }));
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "l")
            .count()
            >= 3,
        "rotated line and arrowheads should render through passive line segments"
    );
    for forbidden in [
        b"rotation".as_slice(),
        b"lineStartArrowhead",
        b"lineEndArrowhead",
        b"pFragments",
        b"hidden-rotated-line-payload",
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
            "rotated line metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_line_callout_shapes_render_passively_without_payload_leakage() {
    let mut input = String::from("{\\rtf1 Before\\par");
    for (shape_type, payload) in [
        (109, "office-line-callout-1-payload"),
        (110, "office-line-callout-2-payload"),
        (111, "office-line-callout-3-payload"),
        (112, "office-line-callout-4-payload"),
        (121, "office-line-callout-1-accent-bar-payload"),
        (122, "office-line-callout-2-accent-bar-payload"),
        (123, "office-line-callout-3-accent-bar-payload"),
        (124, "office-line-callout-4-accent-bar-payload"),
    ] {
        input.push_str(&format!(
            "{{\\shp{{\\*\\shpinst\\shpleft720\\shptop720\\shpright3600\\shpbottom1800{{\\sp{{\\sn shapeType}}{{\\sv {shape_type}}}}}{{\\sp{{\\sn fillColor}}{{\\sv 65280}}}}{{\\sp{{\\sn lineColor}}{{\\sv 16711680}}}}{{\\sp{{\\sn pFragments}}{{\\sv {payload}}}}}}}}}"
        ));
    }
    input.push_str("After\\par}");
    let input = input.into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let shapes = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shapes.len(), 8);
    for shape in &shapes {
        assert_eq!(shape.kind, StaticShapeKind::Polygon);
        assert_eq!(shape.points.len(), 7);
        assert!(
            shape.points.iter().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "line callout shape points must stay inside passive frame: {shape:?}"
        );
    }
    assert_eq!(shapes[0].points[4].y_twips, shapes[0].height_twips);
    for shape in shapes.iter().take(4) {
        assert!(
            shape.overlay_paths.is_empty(),
            "plain bordered line callouts should not gain accent-bar overlays"
        );
    }
    for shape in shapes.iter().skip(4) {
        assert_eq!(
            shape.overlay_paths.len(),
            1,
            "accent-bar line callouts should preserve the passive accent stroke"
        );
        assert_eq!(shape.overlay_paths[0].len(), 2);
        assert!(
            shape.overlay_paths.iter().flatten().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "line callout accent bar must stay inside passive frame"
        );
    }
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "office-line-callout-1-payload",
        "office-line-callout-2-payload",
        "office-line-callout-3-payload",
        "office-line-callout-4-payload",
        "office-line-callout-1-accent-bar-payload",
        "office-line-callout-2-accent-bar-payload",
        "office-line-callout-3-accent-bar-payload",
        "office-line-callout-4-accent-bar-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "line callout metadata leaked to normalized text: {forbidden}"
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
    let mut rendered_text = String::new();
    let mut passive_shape_paints = 0usize;
    let mut passive_overlay_strokes = 0usize;
    for page_id in parsed_pdf.get_pages().values() {
        let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
        rendered_text.push_str(&decoded_pdf_text(&content));
        passive_shape_paints += content
            .operations
            .iter()
            .filter(|operation| operation.operator == "B")
            .count();
        passive_overlay_strokes += content
            .operations
            .iter()
            .filter(|operation| operation.operator == "S")
            .count();
    }

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        passive_shape_paints >= 8,
        "bordered line callouts should render passive fill/stroke paths"
    );
    assert!(
        passive_overlay_strokes >= 4,
        "accent-bar line callouts should render passive overlay strokes"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"office-line-callout-1-payload",
        b"office-line-callout-2-payload",
        b"office-line-callout-3-payload",
        b"office-line-callout-4-payload",
        b"office-line-callout-1-accent-bar-payload",
        b"office-line-callout-2-accent-bar-payload",
        b"office-line-callout-3-accent-bar-payload",
        b"office-line-callout-4-accent-bar-payload",
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
            "line callout metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_no_border_line_callouts_render_as_passive_polylines_without_payload_leakage() {
    let mut input = String::from("{\\rtf1 Before\\par");
    for (shape_type, payload) in [
        (113, "office-line-callout-1-no-border-payload"),
        (114, "office-line-callout-2-no-border-payload"),
        (115, "office-line-callout-3-no-border-payload"),
        (116, "office-line-callout-4-no-border-payload"),
        (117, "office-line-callout-1-accent-bar-no-border-payload"),
        (118, "office-line-callout-2-accent-bar-no-border-payload"),
        (119, "office-line-callout-3-accent-bar-no-border-payload"),
        (120, "office-line-callout-4-accent-bar-no-border-payload"),
    ] {
        input.push_str(&format!(
            "{{\\shp{{\\*\\shpinst\\shpleft720\\shptop720\\shpright3600\\shpbottom1800{{\\sp{{\\sn shapeType}}{{\\sv {shape_type}}}}}{{\\sp{{\\sn fillColor}}{{\\sv 65280}}}}{{\\sp{{\\sn lineColor}}{{\\sv 16711680}}}}{{\\sp{{\\sn pFragments}}{{\\sv {payload}}}}}}}}}"
        ));
    }
    input.push_str("After\\par}");
    let input = input.into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let shapes = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shapes.len(), 8);
    let expected_point_counts = [2, 2, 3, 4, 2, 2, 3, 4];
    for (shape, expected_point_count) in shapes.iter().zip(expected_point_counts) {
        assert_eq!(shape.kind, StaticShapeKind::Polyline);
        assert_eq!(shape.points.len(), expected_point_count);
        assert!(
            shape.points.iter().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "no-border line callout points must stay inside passive frame: {shape:?}"
        );
    }
    for shape in shapes.iter().take(4) {
        assert!(
            shape.overlay_paths.is_empty(),
            "plain no-border line callouts should not gain accent-bar overlays"
        );
    }
    for shape in shapes.iter().skip(4) {
        assert_eq!(
            shape.overlay_paths.len(),
            1,
            "no-border accent-bar line callouts should preserve the passive accent stroke"
        );
        assert_eq!(shape.overlay_paths[0].len(), 2);
        assert!(
            shape.overlay_paths.iter().flatten().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "no-border line callout accent bar must stay inside passive frame"
        );
    }
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "office-line-callout-1-no-border-payload",
        "office-line-callout-2-no-border-payload",
        "office-line-callout-3-no-border-payload",
        "office-line-callout-4-no-border-payload",
        "office-line-callout-1-accent-bar-no-border-payload",
        "office-line-callout-2-accent-bar-no-border-payload",
        "office-line-callout-3-accent-bar-no-border-payload",
        "office-line-callout-4-accent-bar-no-border-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "no-border line callout metadata leaked to normalized text: {forbidden}"
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
    let mut rendered_text = String::new();
    let mut passive_strokes = 0usize;
    for page_id in parsed_pdf.get_pages().values() {
        let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
        rendered_text.push_str(&decoded_pdf_text(&content));
        passive_strokes += content
            .operations
            .iter()
            .filter(|operation| operation.operator == "S")
            .count();
    }

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        passive_strokes >= 12,
        "no-border line callouts and accent bars should render passive stroked polylines"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"office-line-callout-1-no-border-payload",
        b"office-line-callout-2-no-border-payload",
        b"office-line-callout-3-no-border-payload",
        b"office-line-callout-4-no-border-payload",
        b"office-line-callout-1-accent-bar-no-border-payload",
        b"office-line-callout-2-accent-bar-no-border-payload",
        b"office-line-callout-3-accent-bar-no-border-payload",
        b"office-line-callout-4-accent-bar-no-border-payload",
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
            "no-border line callout metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_round_snip_frame_and_tear_shapes_render_passively_without_payload_leakage() {
    let mut input = String::from("{\\rtf1 Before\\par");
    for (shape_type, payload) in [
        (151, "office-round-one-rectangle-payload"),
        (152, "office-round-two-same-rectangle-payload"),
        (153, "office-round-two-diagonal-rectangle-payload"),
        (154, "office-snip-round-rectangle-payload"),
        (155, "office-snip-one-rectangle-payload"),
        (156, "office-snip-two-same-rectangle-payload"),
        (157, "office-snip-two-diagonal-rectangle-payload"),
        (158, "office-frame-payload"),
        (159, "office-half-frame-payload"),
        (160, "office-tear-payload"),
    ] {
        input.push_str(&format!(
            "{{\\shp{{\\*\\shpinst\\shpleft720\\shptop720\\shpright3600\\shpbottom1800{{\\sp{{\\sn shapeType}}{{\\sv {shape_type}}}}}{{\\sp{{\\sn fillColor}}{{\\sv 65280}}}}{{\\sp{{\\sn lineColor}}{{\\sv 16711680}}}}{{\\sp{{\\sn pFragments}}{{\\sv {payload}}}}}}}}}"
        ));
    }
    input.push_str("After\\par}");
    let input = input.into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let shapes = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shapes.len(), 10);
    let expected_point_counts = [10, 16, 16, 17, 5, 6, 6, 4, 6, 55];
    for (shape, expected_point_count) in shapes.iter().zip(expected_point_counts) {
        assert_eq!(shape.kind, StaticShapeKind::Polygon);
        assert_eq!(shape.points.len(), expected_point_count);
        assert!(
            shape.points.iter().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "round/snip/frame/tear points must stay inside passive frame: {shape:?}"
        );
    }
    assert!(
        shapes[0]
            .points
            .iter()
            .filter(|point| {
                point.x_twips <= shapes[0].width_twips / 6
                    && point.y_twips <= shapes[0].height_twips / 6
            })
            .count()
            >= 5,
        "round-one rectangle should preserve a bounded passive rounded-corner arc"
    );
    for (index, shape) in [(1usize, shapes[1]), (2, shapes[2]), (3, shapes[3])] {
        assert!(
            shape
                .points
                .iter()
                .filter(|point| {
                    point.x_twips <= shape.width_twips / 6
                        && point.y_twips <= shape.height_twips / 6
                })
                .count()
                >= 5,
            "rounded/snip rectangle {index} should preserve a bounded passive top-left arc"
        );
        assert!(
            shape
                .points
                .iter()
                .filter(|point| {
                    point.x_twips >= shape.width_twips - (shape.width_twips / 6)
                        && (point.y_twips <= shape.height_twips / 6
                            || point.y_twips >= shape.height_twips - (shape.height_twips / 6))
                })
                .count()
                >= 5,
            "rounded/snip rectangle {index} should preserve a bounded passive opposite-corner arc"
        );
    }
    assert_eq!(shapes[7].points[0].x_twips, 0);
    assert_eq!(
        shapes[7].fill_rule,
        StaticImageVectorFillRule::Alternate,
        "frame should use even-odd fill so the passive PDF keeps the center open"
    );
    assert_eq!(shapes[7].point_paths.len(), 1);
    assert_eq!(shapes[7].point_paths[0].len(), 4);
    assert!(
        shapes[7].point_paths.iter().flatten().all(|point| {
            point.x_twips >= 0
                && point.x_twips <= shapes[7].width_twips
                && point.y_twips >= 0
                && point.y_twips <= shapes[7].height_twips
        }),
        "frame inner path must stay inside passive frame"
    );
    assert_eq!(shapes[8].points[2].x_twips, shapes[8].width_twips);
    assert!(shapes[9].points.iter().any(|point| point.y_twips == 0));
    assert!(
        shapes[9]
            .points
            .iter()
            .filter(|point| point.y_twips <= shapes[9].height_twips / 8)
            .count()
            >= 6,
        "tear should preserve a bounded passive top taper"
    );
    assert!(
        shapes[9]
            .points
            .iter()
            .filter(|point| point.y_twips >= (shapes[9].height_twips * 7) / 8)
            .count()
            >= 8,
        "tear should preserve a bounded passive lower lobe"
    );
    assert!(
        shapes[9]
            .points
            .iter()
            .filter(|point| {
                point.x_twips <= shapes[9].width_twips / 8
                    && point.y_twips > shapes[9].height_twips / 4
                    && point.y_twips < (shapes[9].height_twips * 3) / 4
            })
            .count()
            >= 6,
        "tear should preserve a bounded passive left-side sweep"
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "office-round-one-rectangle-payload",
        "office-round-two-same-rectangle-payload",
        "office-round-two-diagonal-rectangle-payload",
        "office-snip-round-rectangle-payload",
        "office-snip-one-rectangle-payload",
        "office-snip-two-same-rectangle-payload",
        "office-snip-two-diagonal-rectangle-payload",
        "office-frame-payload",
        "office-half-frame-payload",
        "office-tear-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "round/snip/frame/tear metadata leaked to normalized text: {forbidden}"
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
    let mut rendered_text = String::new();
    let mut passive_shape_paints = 0usize;
    let mut passive_even_odd_shape_paints = 0usize;
    let mut passive_path_moves = 0usize;
    for page_id in parsed_pdf.get_pages().values() {
        let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
        rendered_text.push_str(&decoded_pdf_text(&content));
        passive_shape_paints += content
            .operations
            .iter()
            .filter(|operation| operation.operator == "B" || operation.operator == "B*")
            .count();
        passive_even_odd_shape_paints += content
            .operations
            .iter()
            .filter(|operation| operation.operator == "B*")
            .count();
        passive_path_moves += content
            .operations
            .iter()
            .filter(|operation| operation.operator == "m")
            .count();
    }

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        passive_shape_paints >= 10,
        "round/snip/frame/tear shapes should render passive fill/stroke paths"
    );
    assert!(
        passive_even_odd_shape_paints >= 1,
        "frame should render as a passive even-odd compound path"
    );
    assert!(
        passive_path_moves >= 11,
        "compound frame should emit separate bounded outer/inner passive paths"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"office-round-one-rectangle-payload",
        b"office-round-two-same-rectangle-payload",
        b"office-round-two-diagonal-rectangle-payload",
        b"office-snip-round-rectangle-payload",
        b"office-snip-one-rectangle-payload",
        b"office-snip-two-same-rectangle-payload",
        b"office-snip-two-diagonal-rectangle-payload",
        b"office-frame-payload",
        b"office-half-frame-payload",
        b"office-tear-payload",
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
            "round/snip/frame/tear metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn repeated_office_shape_line_flag_restores_passive_outline_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fLine}{\sv 0}}{\sp{\sn fLine}{\sv 1}}{\sp{\sn pFragments}{\sv hidden-repeated-line-flag-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape from repeated line flags");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert_eq!(
        shape.stroke_width_twips, 15,
        "last enabled fLine flag should restore the bounded default outline"
    );
    for forbidden in [
        "shapeType",
        "fLine",
        "pFragments",
        "hidden-repeated-line-flag-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "repeated line flag shape metadata leaked to normalized text: {forbidden}"
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
    let stroke_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "S")
        .count();

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        stroke_count >= 4,
        "enabled fLine should render a passive rectangle outline"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fLine",
        b"pFragments",
        b"hidden-repeated-line-flag-payload",
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
            "repeated line flag shape metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn disabled_office_shape_line_remains_hidden_after_width_metadata_without_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fLine}{\sv 0}}{\sp{\sn lineWidth}{\sv 12700}}{\sp{\sn pFragments}{\sv hidden-disabled-line-width-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with disabled line and width metadata");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert_eq!(
        shape.stroke_width_twips, 0,
        "lineWidth metadata must not resurrect a disabled passive outline"
    );
    for forbidden in [
        "shapeType",
        "fLine",
        "lineWidth",
        "pFragments",
        "hidden-disabled-line-width-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "disabled line width metadata leaked to normalized text: {forbidden}"
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
    let stroke_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "S")
        .count();

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert_eq!(
        stroke_count, 0,
        "disabled line should not emit passive stroke operators"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fLine",
        b"lineWidth",
        b"pFragments",
        b"hidden-disabled-line-width-payload",
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
            "disabled line width metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn z_ordered_shape_text_underline_renders_passively_without_control_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright4320\shpbottom1800\shpz23{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fLine}{\sv 0}}{\sp{\sn pFragments}{\sv hidden-underlined-z-text-payload}}}{\shptxt\ul Layered underline\par}}After\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Layered underline"));
    for forbidden in ["shpz", "shapeType", "fLine", "pFragments"] {
        assert!(
            !text.contains(forbidden),
            "z-ordered underlined shape control leaked to normalized text: {forbidden}"
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

    assert!(
        rendered_text.contains("Layered underline"),
        "z-ordered underlined shape text should render as passive PDF text: {rendered_text:?}"
    );
    assert!(
        stroke_count >= 1,
        "z-ordered underlined shape text should emit a passive underline stroke"
    );
    for forbidden in [
        b"shpz".as_slice(),
        b"shapeType",
        b"fLine",
        b"pFragments",
        b"hidden-underlined-z-text-payload",
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
            "z-ordered underlined shape text leaked controls or active PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn z_ordered_styled_shape_text_renders_passive_shading_and_border_without_control_leakage() {
    let input = br#"{\rtf1{\colortbl;\red240\green240\blue0;\red255\green0\blue0;}Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright4320\shpbottom1800\shpz23{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fLine}{\sv 0}}{\sp{\sn pFragments}{\sv hidden-styled-z-text-payload}}}{\shptxt\cbpat1\brdrb\brdrs\brdrw40\brdrcf2 Layered styled\par}}After\par}"#.to_vec();
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

    assert!(
        rendered_text.contains("Layered styled"),
        "z-ordered styled shape text should render as passive PDF text: {rendered_text:?}"
    );
    assert!(
        fill_count >= 1,
        "z-ordered styled shape text should emit passive shading fill"
    );
    assert!(
        stroke_count >= 1,
        "z-ordered styled shape text should emit passive paragraph border stroke"
    );
    for forbidden in [
        b"shpz".as_slice(),
        b"shapeType",
        b"fLine",
        b"pFragments",
        b"hidden-styled-z-text-payload",
        b"shptxt",
        b"cbpat",
        b"brdrb",
        b"brdrw",
        b"brdrcf",
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
            "z-ordered styled shape text leaked controls or active PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn square_shape_wrap_metadata_is_consumed_by_passive_line_exclusion() {
    let image_hex = bytes_to_hex(&minimal_jpeg_with_dimensions(1, 1));
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
        "shpleft0",
        "\\",
        "shptop0",
        "\\",
        "shpright1440",
        "\\",
        "shpbottom720",
        "\\",
        "shpbxcolumn",
        "\\",
        "shpbypara",
        "\\",
        "shpwr2",
        "\\",
        "shpwrk0",
        "\\",
        "shpfblwtxt0",
        "\\",
        "shpz0{",
        "\\",
        "shppict{",
        "\\",
        "pict",
        "\\",
        "jpegblip ",
        image_hex.as_str(),
        "}}}} Wrapped text beside picture",
        "\\",
        "par}",
    ]);
    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();

    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "shape picture result layout metadata normalized into bounded passive image placement",
        )
    }));
    assert!(
        output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("shape text wrapping approximated")
        }),
        "square wrapped shape picture should be handled by passive line exclusion: {:?}",
        output.diagnostics
    );
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("Wrapped text beside picture"));
    for forbidden in [
        b"shpwr".as_slice(),
        b"shpwrk",
        b"shppict",
        b"objdata",
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
            "forbidden square-wrap shape content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn top_bottom_shape_wrap_keeps_following_text_below_passive_frame() {
    let image_hex = bytes_to_hex(&minimal_jpeg_with_dimensions(1, 1));
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
        "shpleft0",
        "\\",
        "shptop0",
        "\\",
        "shpright1440",
        "\\",
        "shpbottom720",
        "\\",
        "shpbxcolumn",
        "\\",
        "shpbypara",
        "\\",
        "shpwr1",
        "\\",
        "shpwrk0",
        "\\",
        "shpfblwtxt0",
        "\\",
        "shpz0{",
        "\\",
        "shppict{",
        "\\",
        "pict",
        "\\",
        "jpegblip ",
        image_hex.as_str(),
        "}}}} Wrapped text below picture",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("shape picture image");
    let placement = image.placement.expect("shape picture placement");
    assert!(
        !placement.text_wrap,
        "top/bottom wrap should keep the passive frame in block flow"
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
        output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("shape text wrapping approximated")
        }),
        "top/bottom wrapped shape should be handled by passive block flow: {:?}",
        output.diagnostics
    );
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("Wrapped text below picture"));
    for forbidden in [
        b"shpwr".as_slice(),
        b"shpwrk",
        b"shppict",
        b"objdata",
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
            "forbidden top-bottom shape content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn column_paragraph_shape_anchoring_is_consumed_as_passive_layout_metadata() {
    let image_hex = bytes_to_hex(&minimal_jpeg_with_dimensions(1, 1));
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
        "shptop360",
        "\\",
        "shpright2160",
        "\\",
        "shpbottom1080",
        "\\",
        "shpbxcolumn",
        "\\",
        "shpbypara{",
        "\\",
        "sp{",
        "\\",
        "sn shapeType}{",
        "\\",
        "sv 1}}}{",
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
    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();

    assert!(
        output
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("bounded passive shape frame") })
    );
    assert!(
        output.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("floating shape anchoring approximated")),
        "column/paragraph anchoring should be handled by the passive frame layout: {:?}",
        output.diagnostics
    );
    assert!(
        output
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "column/paragraph shape anchors should be classified: {:?}",
        output.diagnostics
    );

    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "Do"),
        "anchored shape picture should render as passive image bytes"
    );
    for forbidden in [
        b"shpbxcolumn".as_slice(),
        b"shpbypara",
        b"shpinst",
        b"shppict",
        b"shapeType",
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
            "column/paragraph shape anchor metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn page_and_margin_shape_anchoring_is_consumed_without_property_leakage() {
    let static_input = rtf(&[
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
        "shpright1440",
        "\\",
        "shpbottom1440",
        "\\",
        "shpbxpage",
        "\\",
        "shpbymargin{",
        "\\",
        "sp{",
        "\\",
        "sn shapeType}{",
        "\\",
        "sv 1}}}}After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&static_input).unwrap();
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("page/margin anchored shape");

    assert_eq!(
        shape.horizontal_anchor,
        open_rtf_converter::model::StaticShapeHorizontalAnchor::Page
    );
    assert_eq!(
        shape.vertical_anchor,
        open_rtf_converter::model::StaticShapeVerticalAnchor::Margin
    );

    let image_hex = bytes_to_hex(&minimal_jpeg_with_dimensions(1, 1));
    let picture_input = rtf(&[
        "{",
        "\\",
        "rtf1{",
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
        "shpright1440",
        "\\",
        "shpbottom1440",
        "\\",
        "shpbxmargin",
        "\\",
        "shpbypage{",
        "\\",
        "shppict{",
        "\\",
        "pict",
        "\\",
        "jpegblip ",
        image_hex.as_str(),
        "}}}}}",
    ]);
    let picture = parse_rtf_bytes(&picture_input).unwrap();
    let placement = picture
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => image.placement,
            _ => None,
        })
        .expect("page/margin anchored shape picture");

    assert_eq!(
        placement.horizontal_anchor,
        open_rtf_converter::model::StaticShapeHorizontalAnchor::Margin
    );
    assert_eq!(
        placement.vertical_anchor,
        open_rtf_converter::model::StaticShapeVerticalAnchor::Page
    );

    let output = convert_rtf_to_pdf(
        &static_input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    assert!(
        output.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("floating shape anchoring approximated")),
        "page/margin anchors should be normalized, not reported as ignored: {:?}",
        output.diagnostics
    );
    for forbidden in [
        "shpbxpage",
        "shpbymargin",
        "shpbxmargin",
        "shpbypage",
        "shapeType",
        "shppict",
    ] {
        for text in [
            collect_text(&parsed.document),
            collect_text(&picture.document),
        ] {
            assert!(
                !text.contains(forbidden),
                "shape anchor control leaked to text: {forbidden}"
            );
        }
    }
    for forbidden in [
        b"shpbxpage".as_slice(),
        b"shpbymargin",
        b"shapeType",
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
            "shape anchor control leaked to PDF: {:?}",
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
fn disabled_office_shape_line_remains_hidden_after_line_fore_color_metadata_without_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fLine}{\sv 0}}{\sp{\sn lineForeColor}{\sv 255}}{\sp{\sn pFragments}{\sv hidden-disabled-line-fore-color-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with disabled line and lineForeColor metadata");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert_eq!(
        shape.stroke_width_twips, 0,
        "lineForeColor metadata must not resurrect a disabled passive outline"
    );
    assert_eq!(
        shape.stroke_color,
        open_rtf_converter::model::Color {
            red: 0,
            green: 0,
            blue: 0,
        },
        "Word ignores lineForeColor while final fLine state keeps the outline invisible"
    );
    for forbidden in [
        "shapeType",
        "fLine",
        "lineForeColor",
        "pFragments",
        "hidden-disabled-line-fore-color-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "disabled lineForeColor metadata leaked to normalized text: {forbidden}"
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

    assert!(
        !content
            .operations
            .iter()
            .any(|operation| operation.operator == "S"),
        "disabled outline should not emit passive stroke"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fLine",
        b"lineForeColor",
        b"pFragments",
        b"hidden-disabled-line-fore-color-payload",
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
            "disabled lineForeColor metadata leaked to PDF: {:?}",
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
        open_rtf_converter::model::BorderStyle::Single
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
        !content
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
fn office_shape_round_line_join_renders_passively_without_property_leakage() {
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
        "dppolycount3",
        "\\",
        "dpptx360",
        "\\",
        "dppty480",
        "\\",
        "dpptx1440",
        "\\",
        "dppty480",
        "\\",
        "dpptx1440",
        "\\",
        "dppty1200",
        "{",
        "\\",
        "sp{",
        "\\",
        "sn lineWidth}{",
        "\\",
        "sv 76200}}{",
        "\\",
        "sp{",
        "\\",
        "sn lineJoinStyle}{",
        "\\",
        "sv round}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-line-join-payload}}}After",
        "\\",
        "par}",
    ]);
    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("polyline shape with round join");
    let text = collect_text(&parsed.document);

    assert_eq!(shape.kind, StaticShapeKind::Polyline);
    assert_eq!(shape.stroke_join, StaticShapeLineJoin::Round);
    assert!(
        output.diagnostics.iter().any(|warning| warning
            .message
            .contains("unsupported/active drawing properties")),
        "unrelated active Office drawing properties should still be stripped: {:?}",
        output.diagnostics
    );
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "lineJoinStyle",
        "round",
        "pFragments",
        "hostile-line-join-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden lineJoinStyle drawing content leaked to text: {forbidden}"
        );
    }

    let layout = LayoutEngine::try_layout(&parsed.document)
        .expect("parsed documents satisfy model invariants");
    assert!(
        layout
            .pages
            .iter()
            .flat_map(|page| page.items.iter())
            .any(|item| matches!(
                item,
                LayoutItem::JoinedPolyline {
                    points,
                    join: LineJoin::Round,
                    ..
                } if points.len() == 3
            )),
        "round joined polyline should stay one passive joined path"
    );
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content.operations.iter().any(|operation| {
            operation.operator == "j"
                && operation.operands.len() == 1
                && pdf_operand_number(&operation.operands[0])
                    .is_some_and(|value| (value - 1.0).abs() < 0.01)
        }),
        "round line join should emit passive PDF line-join operator"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "S"),
        "joined polyline should still render as a passive stroked path"
    );
    for forbidden in [
        b"lineJoinStyle".as_slice(),
        b"pFragments",
        b"hostile-line-join-payload",
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
            "forbidden lineJoinStyle drawing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn invalid_office_shape_line_join_is_stripped_without_payload_leakage() {
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
        "dppolycount3",
        "\\",
        "dpptx360",
        "\\",
        "dppty480",
        "\\",
        "dpptx1440",
        "\\",
        "dppty480",
        "\\",
        "dpptx1440",
        "\\",
        "dppty1200",
        "{",
        "\\",
        "sp{",
        "\\",
        "sn lineJoinStyle}{",
        "\\",
        "sv activeJoinPayload}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-invalid-line-join-payload}}}After",
        "\\",
        "par}",
    ]);
    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("polyline shape with invalid join");
    let text = collect_text(&parsed.document);

    assert_eq!(shape.stroke_join, StaticShapeLineJoin::Miter);
    assert!(
        output.diagnostics.iter().any(|warning| warning
            .message
            .contains("stripping unsupported/active drawing properties")),
        "invalid line join should be diagnosed as stripped: {:?}",
        output.diagnostics
    );
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "lineJoinStyle",
        "activeJoinPayload",
        "pFragments",
        "hostile-invalid-line-join-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden invalid lineJoinStyle drawing content leaked to text: {forbidden}"
        );
    }
    let layout = LayoutEngine::try_layout(&parsed.document)
        .expect("parsed documents satisfy model invariants");
    assert!(
        !layout
            .pages
            .iter()
            .flat_map(|page| page.items.iter())
            .any(|item| matches!(item, LayoutItem::JoinedPolyline { .. })),
        "invalid line join should not create joined passive geometry"
    );
    for forbidden in [
        b"lineJoinStyle".as_slice(),
        b"activeJoinPayload",
        b"pFragments",
        b"hostile-invalid-line-join-payload",
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
            "forbidden invalid lineJoinStyle drawing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_numeric_line_dash_styles_render_passively_without_property_leakage() {
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
        "sv 2}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-dot-style-payload}}}{",
        "\\",
        "do",
        "\\",
        "dpline",
        "\\",
        "dpx360",
        "\\",
        "dpy1320",
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
        "sv 5}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-dash-style-payload}}}After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let shapes: Vec<_> = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .collect();
    let text = collect_text(&parsed.document);

    assert_eq!(shapes.len(), 2);
    assert_eq!(
        shapes[0].stroke_style,
        open_rtf_converter::model::BorderStyle::Single
    );
    assert_eq!(
        shapes[1].stroke_style,
        open_rtf_converter::model::BorderStyle::Single
    );
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "lineDashing",
        "pFragments",
        "hostile-dot-style-payload",
        "hostile-dash-style-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden numeric lineDashing drawing content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("office-shape-numeric-line-dashing.rtf");
    let output_path = dir.path().join("office-shape-numeric-line-dashing.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
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
        !content
            .operations
            .iter()
            .any(|operation| operation.operator == "d")
    );
    for forbidden in [
        b"lineDashing".as_slice(),
        b"pFragments",
        b"hostile-dot-style-payload",
        b"hostile-dash-style-payload",
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
            "forbidden numeric lineDashing drawing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_line_style_renders_passively_without_property_leakage() {
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
        "sn lineStyle}{",
        "\\",
        "sv 2}}{",
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
        .expect("line style shape");
    let text = collect_text(&parsed.document);

    assert_eq!(
        shape.stroke_style,
        open_rtf_converter::model::BorderStyle::Double
    );
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "lineStyle",
        "pFragments",
        "hostile-line-style-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden lineStyle drawing content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("office-shape-line-style.rtf");
    let output_path = dir.path().join("office-shape-line-style.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
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
            .any(|operation| operation.operator == "S"),
        "double Office line style should render as passive double stroke"
    );
    for forbidden in [
        b"lineStyle".as_slice(),
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
            "forbidden lineStyle drawing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_named_double_line_style_alias_renders_passively() {
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
        "sn lineStyle}{",
        "\\",
        "sv msoLineDouble}}}After",
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
        .expect("named double line style shape");
    let text = collect_text(&parsed.document);

    assert_eq!(
        shape.stroke_style,
        open_rtf_converter::model::BorderStyle::Double
    );
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unsupported/active drawing properties")),
        "named double lineStyle should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );
    for forbidden in ["lineStyle", "msoLineDouble"] {
        assert!(
            !text.contains(forbidden),
            "named double lineStyle metadata leaked to normalized text: {forbidden}"
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
    audit_passive_pdf_bytes(&output.pdf).unwrap();
    for forbidden in [
        b"lineStyle".as_slice(),
        b"msoLineDouble",
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
            "named double lineStyle metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_named_weighted_line_style_aliases_render_passively() {
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
        "sn lineStyle}{",
        "\\",
        "sv msoLineThinThick}}}{",
        "\\",
        "do",
        "\\",
        "dpline",
        "\\",
        "dpx360",
        "\\",
        "dpy960",
        "\\",
        "dpxsize1440",
        "\\",
        "dpysize720",
        "{",
        "\\",
        "sp{",
        "\\",
        "sn lineStyle}{",
        "\\",
        "sv msoLineThickThin}}}After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let shapes = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .collect::<Vec<_>>();
    let text = collect_text(&parsed.document);

    assert_eq!(shapes.len(), 2);
    assert_eq!(
        shapes[0].stroke_style,
        open_rtf_converter::model::BorderStyle::ThinThick
    );
    assert_eq!(
        shapes[1].stroke_style,
        open_rtf_converter::model::BorderStyle::ThickThin
    );
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("unsupported/active drawing properties")
    }));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("stripping unsupported/active drawing properties")
    }));
    for forbidden in ["lineStyle", "msoLineThinThick", "msoLineThickThin"] {
        assert!(
            !text.contains(forbidden),
            "named weighted lineStyle metadata leaked to normalized text: {forbidden}"
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
    audit_passive_pdf_bytes(&output.pdf).unwrap();
    let layout = LayoutEngine::try_layout(&parsed.document)
        .expect("parsed documents satisfy model invariants");
    let line_styles = layout
        .pages
        .iter()
        .flat_map(|page| page.items.iter())
        .filter_map(|item| match item {
            LayoutItem::Line { style, .. } => Some(*style),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(line_styles.contains(&LineStyle::ThinThick));
    assert!(line_styles.contains(&LineStyle::ThickThin));
    for forbidden in [
        b"lineStyle".as_slice(),
        b"msoLineThinThick",
        b"msoLineThickThin",
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
            "named weighted lineStyle metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_weighted_line_style_renders_passively_without_property_leakage() {
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
        "sn lineStyle}{",
        "\\",
        "sv 5}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-triple-line-style-payload}}}After",
        "\\",
        "par}",
    ]);
    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("weighted line style shape");
    let text = collect_text(&parsed.document);

    assert_eq!(
        shape.stroke_style,
        open_rtf_converter::model::BorderStyle::ThinThickThin
    );
    assert!(
        output.diagnostics.iter().any(|warning| warning
            .message
            .contains("unsupported/active drawing properties")),
        "active Office drawing properties should still be stripped: {:?}",
        output.diagnostics
    );
    assert!(
        output
            .diagnostics
            .iter()
            .all(|warning| !warning.message.contains("lineStyle approximation")),
        "supported weighted Office line style should not be diagnosed as an approximation: {:?}",
        output.diagnostics
    );
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "lineStyle",
        "pFragments",
        "hostile-triple-line-style-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden weighted lineStyle drawing content leaked to text: {forbidden}"
        );
    }
    for forbidden in [
        b"lineStyle".as_slice(),
        b"pFragments",
        b"hostile-triple-line-style-payload",
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
            "forbidden weighted lineStyle drawing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
    let layout = LayoutEngine::try_layout(&parsed.document)
        .expect("parsed documents satisfy model invariants");
    assert!(
        layout
            .pages
            .iter()
            .flat_map(|page| page.items.iter())
            .any(|item| matches!(
                item,
                LayoutItem::Line {
                    style: LineStyle::ThinThickThin,
                    ..
                }
            )),
        "weighted Office line style should normalize to a passive thin-thick-thin line"
    );
}

#[test]
fn office_shape_round_line_cap_renders_passively_without_property_leakage() {
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
        "sn lineEndCap}{",
        "\\",
        "sv round}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-round-cap-payload}}}After",
        "\\",
        "par}",
    ]);
    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("round cap line shape");
    let text = collect_text(&parsed.document);

    assert_eq!(shape.stroke_cap, StaticShapeLineCap::Flat);
    assert!(
        output.diagnostics.iter().any(|warning| warning
            .message
            .contains("unsupported/active drawing properties")),
        "active Office drawing properties should still be stripped: {:?}",
        output.diagnostics
    );
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "lineEndCap",
        "round",
        "pFragments",
        "hostile-round-cap-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden line cap drawing content leaked to text: {forbidden}"
        );
    }
    let layout = LayoutEngine::try_layout(&parsed.document)
        .expect("parsed documents satisfy model invariants");
    assert!(
        layout
            .pages
            .iter()
            .flat_map(|page| page.items.iter())
            .any(|item| matches!(item, LayoutItem::Line { .. })),
        "Word-compatible legacy line cap should remain a passive flat line"
    );
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        !content
            .operations
            .iter()
            .any(|operation| operation.operator == "J")
    );
    for forbidden in [
        b"lineEndCap".as_slice(),
        b"pFragments",
        b"hostile-round-cap-payload",
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
            "forbidden line cap drawing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn invalid_office_shape_line_cap_is_stripped_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn lineEndCap}{\sv active-cap-payload}}}}After\par}"#.to_vec();
    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("invalid cap line shape");
    let text = collect_text(&parsed.document);

    assert_eq!(shape.stroke_cap, StaticShapeLineCap::Flat);
    assert!(
        output.diagnostics.iter().any(|warning| warning
            .message
            .contains("unsupported/active drawing properties")),
        "invalid Office line cap should be stripped and diagnosed: {:?}",
        output.diagnostics
    );
    for forbidden in ["lineEndCap", "active-cap-payload", "[Shape skipped"] {
        assert!(
            !text.contains(forbidden),
            "forbidden invalid line cap content leaked to text: {forbidden}"
        );
    }
    for forbidden in [
        b"lineEndCap".as_slice(),
        b"active-cap-payload",
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
            "forbidden invalid line cap content leaked to PDF: {:?}",
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
        "dppolycount3",
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
fn old_drawing_arc_renders_as_bounded_passive_polyline_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before",
        "\\",
        "par{",
        "\\",
        "do",
        "\\",
        "dparc",
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
        "sv hostile-arc-payload}}}After",
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
        .expect("arc shape");
    let text = collect_text(&parsed.document);

    assert_eq!(
        shape.kind,
        open_rtf_converter::model::StaticShapeKind::Polyline
    );
    assert!(
        shape.points.len() > 8,
        "legacy arc should normalize to sampled passive points"
    );
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("stripping unsupported/active drawing properties")
    }));
    for forbidden in [
        "dparc",
        "dpxsize",
        "dpysize",
        "pFragments",
        "hostile-arc-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden arc drawing content leaked to text: {forbidden}"
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
    audit_passive_pdf_bytes(&output.pdf).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let line_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "l")
        .count();

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        line_count > 8,
        "legacy arc should render as bounded passive line segments"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "S")
    );
    for forbidden in [
        b"dparc".as_slice(),
        b"dpxsize",
        b"dpysize",
        b"pFragments",
        b"hostile-arc-payload",
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
            "forbidden arc drawing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}
