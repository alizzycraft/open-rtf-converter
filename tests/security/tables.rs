use super::*;

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
fn table_cell_definition_growth_is_bounded() {
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
                "cellx1000",
                "\\",
                "cellx2000 A",
                "\\",
                "cell",
                "\\",
                "row}"
            ]),
            &options
        ),
        Err(ParseError::ResourceLimitExceeded { resource, .. })
            if resource == "table cell definitions"
    ));
}

#[test]
fn table_row_growth_is_bounded() {
    let options = RtfParseOptions {
        limits: RtfLimits {
            max_table_rows: 1,
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
                "cell",
                "\\",
                "row",
                "\\",
                "trowd",
                "\\",
                "cellx1000 B",
                "\\",
                "cell",
                "\\",
                "row}"
            ]),
            &options
        ),
        Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "table rows"
    ));
}

#[test]
fn table_nesting_growth_is_bounded() {
    let options = RtfParseOptions {
        limits: RtfLimits {
            max_table_nesting_level: 1,
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
                "cellx6000 Outer {",
                "\\",
                "trowd",
                "\\",
                "itap2",
                "\\",
                "cellx1000 Inner",
                "\\",
                "nestcell",
                "\\",
                "nestrow} after",
                "\\",
                "cell",
                "\\",
                "row}"
            ]),
            &options
        ),
        Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "table nesting"
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
    fs::write(&input_path, &input).unwrap();
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
fn preferred_cell_widths_without_cell_boundaries_are_omitted_without_control_leakage() {
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
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::Table(_)))
    );
    assert!(!text.contains("Narrow"));
    assert!(!text.contains("Wide percent"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("table row without authored cell boundaries omitted to match Word recovery")
    }));
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

    assert!(!rendered_text.contains("Narrow"));
    assert!(!rendered_text.contains("Wide percent"));
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
fn preferred_row_widths_without_cell_boundaries_are_omitted_without_control_leakage() {
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
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::Table(_)))
    );
    assert!(!text.contains("Exact left"));
    assert!(!text.contains("Exact right"));
    assert!(!text.contains("Percent left"));
    assert!(!text.contains("Percent right"));
    assert_eq!(
        parsed
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.message.contains(
                "table row without authored cell boundaries omitted to match Word recovery"
            ))
            .count(),
        2
    );
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

    assert!(!rendered_text.contains("Exact left"));
    assert!(!rendered_text.contains("Exact right"));
    assert!(!rendered_text.contains("Percent left"));
    assert!(!rendered_text.contains("Percent right"));
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
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("table autofit growth disabled through bounded passive authored widths")
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
        "trrh720",
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
        "tposxr",
        "\\",
        "tposxo",
        "\\",
        "tpvmrg",
        "\\",
        "tpvpara",
        "\\",
        "tposy360",
        "\\",
        "tposyb",
        "\\",
        "tposyin",
        "\\",
        "tposyout",
        "\\",
        "clftsWidth3",
        "\\",
        "clwWidth1800",
        "\\",
        "cellx1800 Positioned left",
        "\\",
        "cell",
        "\\",
        "clFitText",
        "\\",
        "clftsWidth3",
        "\\",
        "clwWidth1800",
        "\\",
        "cellx3600 Positioned right",
        "\\",
        "cell",
        "\\",
        "row",
        "\\",
        "trowd",
        "\\",
        "trrh360",
        "\\",
        "phmrg",
        "\\",
        "posx720",
        "\\",
        "pvmrg",
        "\\",
        "posy120",
        "\\",
        "posyin",
        "\\",
        "posyout",
        "\\",
        "cellx1800 Alias anchored",
        "\\",
        "cell",
        "\\",
        "cellx3600 Alias right",
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
    assert_eq!(table.rows[0].left_offset_twips, 1080);
    assert_eq!(table.rows[0].vertical_offset_twips, -360);
    assert_eq!(table.rows[0].wrap_margins.left_twips, 180);
    assert_eq!(table.rows[0].wrap_margins.right_twips, 180);
    assert_eq!(table.rows[0].wrap_margins.top_twips, 120);
    assert_eq!(table.rows[0].wrap_margins.bottom_twips, 120);
    assert_eq!(table.rows[0].alignment, TableRowAlignment::Outside);
    assert!(table.rows[0].no_overlap);
    assert_eq!(table.rows[1].left_offset_twips, 720);
    assert_eq!(table.rows[1].vertical_offset_twips, 120);
    assert!(!table.rows[0].cells[0].fit_text);
    assert!(table.rows[0].cells[1].fit_text);
    for forbidden in [
        "tabsnoovrlp",
        "tdfrmtxtLeft",
        "tdfrmtxtRight",
        "tdfrmtxtTop",
        "tdfrmtxtBottom",
        "tphmrg",
        "phmrg",
        "tposx",
        "tposxr",
        "tposxo",
        "tpvmrg",
        "tpvpara",
        "pvmrg",
        "tposy",
        "tposyb",
        "tposyin",
        "tposyout",
        "posyin",
        "posyout",
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
    assert!(!parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("floating table positioning approximated by passive table flow")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("floating table no-overlap \\tabsnoovrlp captured")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "floating table horizontal position interpreted as bounded passive row offset",
        )
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("floating table horizontal anchor \\tphmrg")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("floating table horizontal alignment \\tposxr")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("floating table horizontal alignment \\tposxo")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("floating table vertical position interpreted as bounded passive row offset")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("floating table vertical alignment \\tposyb")
    }));
    for control_name in ["tposyin", "tposyout", "posyin", "posyout"] {
        assert!(parsed.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(&format!(
                "floating table vertical flow control \\{control_name}"
            ))
        }));
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "floating table vertical center/bottom alignment interpreted as bounded passive row offset",
        )
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("floating table vertical anchor \\tpvpara")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("floating table vertical anchor \\tpvmrg")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("floating table wrap distance interpreted as bounded passive row margin")
    }));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("table cell fit-text approximated")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("table cell fit-text rendered as bounded passive horizontal scaling")
    }));

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Positioned left"));
    assert!(rendered_text.contains("Positioned right"));
    assert!(rendered_text.contains("Alias anchored"));
    assert!(rendered_text.contains("Alias right"));
    assert!(rendered_text.contains("After table"));
    for forbidden in [
        b"tabsnoovrlp".as_slice(),
        b"tdfrmtxtLeft",
        b"tdfrmtxtRight",
        b"tdfrmtxtTop",
        b"tdfrmtxtBottom",
        b"tphmrg",
        b"phmrg",
        b"tposx",
        b"tposxr",
        b"tposxo",
        b"tpvmrg",
        b"tpvpara",
        b"pvmrg",
        b"tposy",
        b"tposyb",
        b"tposyin",
        b"tposyout",
        b"posyin",
        b"posyout",
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
fn table_cell_fit_text_renders_bounded_passive_scaling_without_payload_leakage() {
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
        "clFitText",
        "\\",
        "clftsWidth3",
        "\\",
        "clwWidth720",
        "\\",
        "cellx720 Fit text stays on one visual row without wrapping",
        "\\",
        "cell",
        "\\",
        "row",
        "\\",
        "par}",
    ]);

    let parsed = parse_rtf_bytes(&input).unwrap();
    let table = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("table");
    assert!(table.rows[0].cells[0].fit_text);
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("table cell fit-text rendered as bounded passive horizontal scaling")
    }));
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("table cell fit-text approximated")),
        "fit-text should be reported as rendered, not approximated: {:?}",
        parsed.diagnostics
    );

    let layout = LayoutEngine::try_layout(&parsed.document)
        .expect("parsed documents satisfy model invariants");
    let fragments = layout.pages[0]
        .items
        .iter()
        .filter_map(|item| match item {
            LayoutItem::Text(fragment)
                if fragment.text.contains("Fit")
                    || fragment.text.contains("visual")
                    || fragment.text.contains("wrapping") =>
            {
                Some(fragment)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let first_baseline = fragments
        .first()
        .map(|fragment| fragment.baseline_y)
        .expect("fit-text fragments");
    assert!(
        fragments.iter().all(|fragment| {
            fragment.style.character_scaling_percent < 100
                && (fragment.baseline_y - first_baseline).abs() < 0.01
        }),
        "fit-text should render as one passively scaled row: {fragments:?}"
    );

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Fit text stays"));
    for forbidden in [
        b"clFitText".as_slice(),
        b"clftsWidth",
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
            "fit-text control leaked to PDF: {:?}",
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
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("table cell spacing rendered as bounded passive border gaps")
    }));

    let layout = LayoutEngine::try_layout(&parsed.document)
        .expect("parsed documents satisfy model invariants");
    let text_x = |needle: &str| {
        layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text.contains(needle) => Some(fragment.x),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing layout text {needle}"))
    };
    assert!(
        text_x("right") - text_x("left") > 80.0,
        "cell spacing should keep passive cell content visibly separated"
    );

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
fn table_row_keep_following_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "trowd",
        "\\",
        "trkeepfollow",
        "\\",
        "cellx1440 Kept follow",
        "\\",
        "cell",
        "\\",
        "row",
        "\\",
        "trowd",
        "\\",
        "trkeepfollow0",
        "\\",
        "cellx1440 Normal follow",
        "\\",
        "cell",
        "\\",
        "row} explicit-payload",
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

    assert!(text.contains("Kept follow"));
    assert!(text.contains("Normal follow"));
    assert!(text.contains("explicit-payload"));
    assert!(!table.rows[0].keep_together);
    assert!(table.rows[0].keep_with_next);
    assert!(!table.rows[1].keep_together);
    assert!(!table.rows[1].keep_with_next);
    assert!(!text.contains("trkeepfollow"));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("table row keep-with-following approximated")
    }));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "trkeepfollow should be consumed as bounded passive table metadata: {:?}",
        parsed.diagnostics
    );

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("table-row-keep-following.rtf");
    let output_path = dir.path().join("table-row-keep-following.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
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

    assert!(rendered_text.contains("Kept follow"));
    assert!(rendered_text.contains("Normal follow"));
    assert!(rendered_text.contains("explicit-payload"));
    for forbidden in [
        b"trkeepfollow".as_slice(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden table row keep-following content leaked to PDF: {:?}",
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
    input.push_str("\\trowd\\trhdr\\cellx9000 Split header\\cell\\row");
    input.push_str("\\trowd\\trrh720\\cellx9000 ");
    for idx in 0..120 {
        input.push_str(&format!("Split row line {idx:03}\\line "));
    }
    input.push_str("\\cell\\row}");
    let input = input.into_bytes();

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Safe object fallback"));
    assert!(text.contains("Split header"));
    assert!(text.contains("Split row line 000"));
    assert!(text.contains("Split row line 119"));
    for forbidden in [
        "objdata",
        "objemb",
        "JavaScript",
        "EmbeddedFile",
        "trhdr",
        "trrh",
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
    for page_text in page_texts.iter().skip(1) {
        assert!(
            page_text.contains("Split header"),
            "continued split-row page missing repeated header: {page_text:?}"
        );
    }

    for forbidden in [
        b"objdata".as_slice(),
        b"objemb",
        b"414243",
        b"JavaScript",
        b"EmbeddedFile",
        b"trhdr",
        b"trrh",
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
fn nested_table_content_normalizes_passively_without_control_leakage() {
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
        "rtlrow",
        "\\",
        "ltrrow",
        "\\",
        "trpaddfl3",
        "\\",
        "trpaddl120",
        "\\",
        "trspdfl3",
        "\\",
        "trspdl60",
        "\\",
        "clpadfl3",
        "\\",
        "clpadl80",
        "\\",
        "clspdfl3",
        "\\",
        "clspdl30",
        "\\",
        "tphmrg",
        "\\",
        "tposx120",
        "\\",
        "tpvpg",
        "\\",
        "tposyin",
        "\\",
        "tdfrmtxtLeft120",
        "\\",
        "tabsnoovrlp",
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
    let nested = table.rows[0].cells[0]
        .nested_table
        .as_ref()
        .expect("nested table should be retained in the safe model");
    let nested_text = nested
        .rows
        .iter()
        .flat_map(|row| row.cells.iter())
        .flat_map(|cell| cell.paragraphs.iter())
        .flat_map(|paragraph| paragraph.runs.iter())
        .map(|run| run.text.as_str())
        .collect::<String>();
    assert!(text.contains("Outer before"));
    assert!(nested_text.contains("Inner A"));
    assert!(nested_text.contains("Inner B"));
    assert!(text.contains("Outer after"));
    for forbidden in ["itap", "nesttableprops", "nestcell", "nestrow"] {
        assert!(
            !text.contains(forbidden),
            "forbidden nested table control leaked to text: {forbidden}"
        );
    }
    for forbidden in [
        "rtlrow",
        "ltrrow",
        "trpaddfl",
        "trpaddl",
        "trspdfl",
        "trspdl",
        "clpadfl",
        "clpadl",
        "clspdfl",
        "clspdl",
        "tphmrg",
        "tposx",
        "tpvpg",
        "tposyin",
        "tdfrmtxtLeft",
        "tabsnoovrlp",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden nested table layout control leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "nested table layout controls should be consumed without unsupported-control noise: {:?}",
        parsed.diagnostics
    );

    let layout = LayoutEngine::try_layout(&parsed.document)
        .expect("parsed documents satisfy model invariants");
    let line_count = layout
        .pages
        .iter()
        .flat_map(|page| page.items.iter())
        .filter(|item| matches!(item, LayoutItem::Line { .. }))
        .count();
    assert_eq!(
        line_count, 0,
        "unbordered nested cells must not synthesize passive grid lines"
    );

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
        b"rtlrow",
        b"ltrrow",
        b"trpaddfl",
        b"trpaddl",
        b"trspdfl",
        b"trspdl",
        b"clpadfl",
        b"clpadl",
        b"clspdfl",
        b"clspdl",
        b"tphmrg",
        b"tposx",
        b"tpvpg",
        b"tposyin",
        b"tdfrmtxtLeft",
        b"tabsnoovrlp",
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
        "cltxtbrl",
        "\\",
        "cellx2000 Alias",
        "\\",
        "cell",
        "\\",
        "cltxtbrlv",
        "\\",
        "cellx4000 ABC",
        "\\",
        "cell",
        "\\",
        "cltxbtlr",
        "\\",
        "cellx6000 XY",
        "\\",
        "cell",
        "\\",
        "cltxlrtb",
        "\\",
        "cellx8000 Flat",
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

    assert!(text.contains("Alias"));
    assert_eq!(
        table.rows[0].cells[0].text_direction,
        TableCellTextDirection::TopToBottomRightToLeft
    );
    assert!(text.contains("ABC"));
    assert_eq!(
        table.rows[0].cells[1].text_direction,
        TableCellTextDirection::TopToBottomRightToLeft
    );
    assert!(text.contains("XY"));
    assert_eq!(
        table.rows[0].cells[2].text_direction,
        TableCellTextDirection::BottomToTopLeftToRight
    );
    assert!(text.contains("Flat"));
    for forbidden in ["cltxtbrl", "cltxtbrlv", "cltxbtlr", "cltxlrtb"] {
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

    for expected in ["A", "l", "i", "a", "s", "B", "C", "Y", "X"] {
        assert!(
            rendered_text.contains(expected),
            "decoded PDF text did not contain stacked cell text {expected:?}; got {rendered_text:?}"
        );
    }
    assert!(rendered_text.contains("Flat"));
    for forbidden in [
        b"cltxtbrlv".as_slice(),
        b"cltxtbrl",
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
fn hex_color_table_separators_render_passively_without_control_leakage() {
    let input =
        br"{\rtf1{\colortbl;\red255\green0\blue0\'3b\red0\green255\blue0\'3b}{\cf1 Red} {\highlight2 Marked}\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Red"));
    assert!(text.contains("Marked"));
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    let red = paragraph
        .runs
        .iter()
        .find(|run| run.text.trim() == "Red")
        .expect("red run");
    let marked = paragraph
        .runs
        .iter()
        .find(|run| run.text.trim() == "Marked")
        .expect("marked run");
    assert_eq!(red.style.color_index, 1);
    assert_eq!(marked.style.highlight_index, Some(2));
    for forbidden in [
        "colortbl",
        "red255",
        "green255",
        "\\'3b",
        "cf1",
        "highlight2",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden hex color-table content leaked to text: {forbidden}"
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
        rendered_text.contains("Red") && rendered_text.contains("Marked"),
        "decoded PDF text did not contain hex color-table text: {rendered_text:?}"
    );
    assert!(content.operations.iter().any(|operation| {
        operation.operator == "rg"
            && operation.operands.len() == 3
            && pdf_operand_number(&operation.operands[0]).is_some_and(|value| value > 0.9)
            && pdf_operand_number(&operation.operands[1]).is_some_and(|value| value < 0.01)
            && pdf_operand_number(&operation.operands[2]).is_some_and(|value| value < 0.01)
    }));
    for forbidden in [
        b"colortbl".as_slice(),
        b"red255",
        b"green255",
        b"\\'3b",
        b"cf1",
        b"highlight2",
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
            "forbidden hex color-table content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn unicode_color_table_separators_render_passively_without_control_leakage() {
    let input =
        br"{\rtf1{\colortbl;\red255\green0\blue0\u59?\red0\green255\blue0\u59?}{\cf1 Red} {\highlight2 Marked}\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Red"));
    assert!(text.contains("Marked"));
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    let red = paragraph
        .runs
        .iter()
        .find(|run| run.text.trim() == "Red")
        .expect("red run");
    let marked = paragraph
        .runs
        .iter()
        .find(|run| run.text.trim() == "Marked")
        .expect("marked run");
    assert_eq!(red.style.color_index, 1);
    assert_eq!(marked.style.highlight_index, Some(2));
    for forbidden in [
        "colortbl",
        "red255",
        "green255",
        "u59",
        "?",
        "cf1",
        "highlight2",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden Unicode color-table content leaked to text: {forbidden}"
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
        rendered_text.contains("Red") && rendered_text.contains("Marked"),
        "decoded PDF text did not contain Unicode color-table text: {rendered_text:?}"
    );
    assert!(content.operations.iter().any(|operation| {
        operation.operator == "rg"
            && operation.operands.len() == 3
            && pdf_operand_number(&operation.operands[0]).is_some_and(|value| value > 0.9)
            && pdf_operand_number(&operation.operands[1]).is_some_and(|value| value < 0.01)
            && pdf_operand_number(&operation.operands[2]).is_some_and(|value| value < 0.01)
    }));
    for forbidden in [
        b"colortbl".as_slice(),
        b"red255",
        b"green255",
        b"u59",
        b"cf1",
        b"highlight2",
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
            "forbidden Unicode color-table content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn table_row_borders_are_ignored_without_control_leakage_in_word_mode() {
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
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("row-level table borders ignored to match Word print behavior")
    }));
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
fn table_row_inner_borders_are_ignored_without_control_leakage_in_word_mode() {
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
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("row-level table borders ignored to match Word print behavior")
    }));
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
    assert_eq!(stroke_count, 0, "Word ignores row-level table borders");
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
    assert_eq!(
        stroke_count, 0,
        "borderless merged table must not gain synthetic grid strokes"
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
        "rtlrow",
        "\\",
        "ltrrow",
        "\\",
        "cellx1440 Reset left",
        "\\",
        "cell",
        "\\",
        "cellx4320 Reset right",
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
        "Reset left"
    );
    assert_eq!(
        table.rows[1].cells[1].paragraphs[0].runs[0].text,
        "Reset right"
    );
    assert_eq!(
        table.rows[2].cells[0].paragraphs[0].runs[0].text,
        "Normal left"
    );
    assert_eq!(
        table.rows[2].cells[1].paragraphs[0].runs[0].text,
        "Normal right"
    );
    assert!(text.find("Left wide cell") < text.find("Right cell"));
    assert!(text.find("Reset left") < text.find("Reset right"));
    assert!(!text.contains("taprtl"));
    assert!(!text.contains("rtlrow"));
    assert!(!text.contains("ltrrow"));

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
    assert!(
        rendered_text.find("Reset left") < rendered_text.find("Reset right"),
        "LTR row reset should preserve normal visual order: {rendered_text:?}"
    );
    for forbidden in [
        b"taprtl".as_slice(),
        b"rtlrow",
        b"ltrrow",
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
fn list_table_metadata_controls_do_not_leak_or_override_visible_markers() {
    let input = br#"{\rtf1{\*\listtable{\list\listtemplateid67698689\listsimple{\listlevel\levelnfc23\leveljc0\levelstartat1{\leveltext\'01\u8226 ?;}{\levelnumbers{\object\objdata 4142432f4a617661536372697074};}\f1\fbias0\fi-360\li360\jclisttab\tx360}{\listname Hidden list name{\field{\*\fldinst HYPERLINK "https://example.com/list-name"}{\fldrslt Hidden link}}{\object\objdata 4142432f4a617661536372697074};}\listid7}}{\*\listoverridetable{\listoverride\listid7\listoverridecount0\ls2}}\pard\ls2\ilvl0 Bullet item\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("\u{2022}\tBullet item"));
    assert!(!text.contains("Hidden list name"));
    assert!(!text.contains("Hidden link"));
    assert!(!text.contains("https://example.com/list-name"));
    for forbidden in [
        "listtemplateid",
        "listsimple",
        "leveljc",
        "levelnumbers",
        "listname",
        "listoverridecount",
        "fldinst",
        "objdata",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden list table metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "list table metadata should not be unsupported: {:?}",
        parsed.diagnostics
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("active content removed: field instruction in metadata")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("active content removed: object payload in metadata")
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
        rendered_text.contains("Bullet item"),
        "decoded PDF text did not contain list item text: {rendered_text:?}"
    );
    assert!(
        !rendered_text.contains("Hidden list name"),
        "list name metadata leaked to rendered PDF text: {rendered_text:?}"
    );
    assert!(
        !rendered_text.contains("Hidden link"),
        "list name field result leaked to rendered PDF text: {rendered_text:?}"
    );
    assert!(
        !rendered_text.contains("https://example.com/list-name"),
        "list name field instruction leaked to rendered PDF text: {rendered_text:?}"
    );
    assert!(
        output
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "list table metadata should not be unsupported: {:?}",
        output.diagnostics
    );
    for forbidden in [
        b"listtemplateid".as_slice(),
        b"listsimple",
        b"leveljc",
        b"levelnumbers",
        b"listname",
        b"Hidden list name",
        b"Hidden link",
        b"https://example.com/list-name",
        b"listoverridecount",
        b"fldinst",
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
            "list table metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
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
fn intbl_zero_consumes_table_state_without_pdf_control_leakage() {
    let input = br"{\rtf1\trowd\cellx1800 A\cell\row\pard\intbl0 Outside table\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("A"));
    assert!(text.contains("Outside table"));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "intbl0 should be consumed without unsupported-control noise: {:?}",
        parsed.diagnostics
    );
    assert!(
        matches!(&parsed.document.blocks[1], Block::Paragraph(paragraph) if paragraph.runs[0].text == "Outside table"),
        "intbl0 should keep following text as body flow: {:?}",
        parsed.document.blocks
    );
    for forbidden in ["intbl0", "cellx", "row"] {
        assert!(
            !text.contains(forbidden),
            "forbidden intbl0 table control leaked to text: {forbidden}"
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

    assert!(rendered_text.contains("A"));
    assert!(rendered_text.contains("Outside table"));
    for forbidden in [
        b"intbl0".as_slice(),
        b"intbl",
        b"cellx",
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
            "forbidden intbl0 table content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
    audit_passive_pdf_bytes(&output.pdf).unwrap();
}

#[test]
fn document_stat_fields_ignore_hidden_merged_table_continuations() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "trowd",
        "\\",
        "clmgf",
        "\\",
        "cellx1000 Visible",
        "\\",
        "cell",
        "\\",
        "clmrg",
        "\\",
        "cellx2000 Hidden horizontal",
        "\\",
        "cell",
        "\\",
        "cellx3000 Plain",
        "\\",
        "cell",
        "\\",
        "row",
        "\\",
        "trowd",
        "\\",
        "clvmgf",
        "\\",
        "cellx1000 Top",
        "\\",
        "cell",
        "\\",
        "cellx2000 A",
        "\\",
        "cell",
        "\\",
        "row",
        "\\",
        "trowd",
        "\\",
        "clvmrg",
        "\\",
        "cellx1000 Hidden vertical",
        "\\",
        "cell",
        "\\",
        "cellx2000 B",
        "\\",
        "cell",
        "\\",
        "row Words:{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst NUMWORDS}}",
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
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.contains("Words:6"),
        "visible merged table stats should count only rendered cells; got {rendered_text:?}"
    );
    assert!(
        !rendered_text.contains("Words:10"),
        "hidden merge continuation text changed passive NUMWORDS: {rendered_text:?}"
    );
    for forbidden in [
        b"Hidden horizontal".as_slice(),
        b"Hidden vertical",
        b"NUMWORDS",
        b"fldinst",
        b"clmgf",
        b"clmrg",
        b"clvmgf",
        b"clvmrg",
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
            "merged table stat field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn narrow_font_aliases_render_as_passive_scaled_bundled_font_without_source_payload_leakage() {
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
        font_names.iter().any(|name| name.starts_with(b"TF")),
        "narrow sans text should use a passive supplied font resource; got {font_names:?}"
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
fn bar_tab_stops_render_in_headers_and_tables_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "header",
        "\\",
        "tb720",
        "\\",
        "tx1440 Head",
        "\\",
        "tab Right",
        "\\",
        "par}",
        "\\",
        "trowd",
        "\\",
        "tb360",
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
    let searchable_pdf = pdf_bytes_excluding_embedded_font_programs(&output.pdf);
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
            !searchable_pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden header/table bar-tab content leaked to PDF: {:?}",
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
fn table_border_spacing_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "trowd",
        "\\",
        "trbrdrt",
        "\\",
        "brdrs",
        "\\",
        "brsp60",
        "\\",
        "clbrdrl",
        "\\",
        "brdrs",
        "\\",
        "brsp240",
        "\\",
        "clbrdrr",
        "\\",
        "brdrs",
        "\\",
        "brsp120",
        "\\",
        "cellx1440 Spaced cell",
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
    let cell = &table.rows[0].cells[0];

    assert!(!cell.borders.top.visible);
    assert_eq!(cell.borders.top.spacing_twips, 0);
    assert_eq!(cell.borders.left.spacing_twips, 240);
    assert_eq!(cell.borders.right.spacing_twips, 120);
    assert!(text.contains("Spaced cell"));
    for forbidden in ["trbrdrt", "clbrdrl", "clbrdrr", "brsp"] {
        assert!(
            !text.contains(forbidden),
            "forbidden table border spacing control leaked to text: {forbidden}"
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

    assert!(rendered_text.contains("Spaced cell"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "S"),
        "table border spacing should still render passive stroked borders"
    );
    for forbidden in [
        b"trbrdrt".as_slice(),
        b"clbrdrl",
        b"clbrdrr",
        b"brsp",
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
            "forbidden table border spacing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
    audit_passive_pdf_bytes(&output.pdf).unwrap();
}

#[test]
fn skipped_destinations_do_not_seed_structural_tables_or_later_field_results() {
    let input = br#"{\rtf1{\*\unknown{\fonttbl{\f77 HiddenFont;}}{\colortbl;\red1\green2\blue3;}{\stylesheet{\s9 HiddenStyle;}}{\*\listtable{\list\listtemplateid1{\listlevel\levelnfc23{\leveltext\'01?;}{\levelnumbers;}}\listid99}}}{\s9 Styled visible text\par}Ref {\field{\*\fldinst STYLEREF HiddenStyle}}\par Fallback {\f77 font request} {\cf1 color request}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Styled visible text"));
    assert!(text.contains("Fallback font request color request"));
    assert!(text.contains("Ref [Field removed: no passive result]"));
    assert!(
        parsed
            .document
            .fonts
            .iter()
            .all(|font| font.name != "HiddenFont"),
        "skipped font table mutated normalized fonts: {:?}",
        parsed.document.fonts
    );
    assert!(
        parsed
            .document
            .colors
            .iter()
            .all(|color| { !(color.red == 1 && color.green == 2 && color.blue == 3) }),
        "skipped color table mutated normalized colors: {:?}",
        parsed.document.colors
    );
    for forbidden in [
        "HiddenFont",
        "HiddenStyle",
        "listtemplateid",
        "leveltext",
        "listid99",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "skipped structural destination leaked to text: {forbidden}"
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

    assert!(rendered_text.contains("Styled visible text"));
    assert!(rendered_text.contains("Ref [Field removed: no passive result]"));
    assert!(rendered_text.contains("Fallback font request color request"));
    for forbidden in [
        b"HiddenFont".as_slice(),
        b"HiddenStyle",
        b"listtemplateid",
        b"leveltext",
        b"listid99",
        b"fldinst",
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
            "skipped structural destination leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_circular_arrow_shape_renders_passively_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright3600\shpbottom1800{\sp{\sn shapeType}{\sv 60}}{\sp{\sn fillColor}{\sv 65280}}{\sp{\sn lineColor}{\sv 16711680}}{\sp{\sn pFragments}{\sv hidden-circular-arrow-payload}}}}After\par}"#.to_vec();
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
        .expect("passive circular arrow shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 68);
    assert!(
        shape.points.iter().all(|point| {
            point.x_twips >= 0
                && point.x_twips <= shape.width_twips
                && point.y_twips >= 0
                && point.y_twips <= shape.height_twips
        }),
        "circular arrow points must stay inside passive frame: {shape:?}"
    );
    assert!(
        shape.points.iter().any(|point| point.x_twips == 0),
        "circular arrow should include a bounded arrow head at the left edge"
    );
    assert!(
        shape
            .points
            .iter()
            .any(|point| point.x_twips == shape.width_twips),
        "circular arrow should include a bounded outer arc at the right edge"
    );
    assert_eq!(
        shape.overlay_paths.len(),
        1,
        "circular arrow should preserve the passive inner arc"
    );
    assert!(
        shape
            .points
            .iter()
            .filter(|point| {
                point.x_twips <= (shape.width_twips * 3) / 4
                    && point.y_twips >= shape.height_twips / 8
                    && point.y_twips <= (shape.height_twips * 7) / 8
            })
            .count()
            >= 40,
        "circular arrow should preserve a bounded passive circular body"
    );
    assert!(
        shape.overlay_paths[0].len() >= 28,
        "circular arrow inner arc should be a bounded passive polyline"
    );
    assert!(
        shape.overlay_paths.iter().flatten().all(|point| {
            point.x_twips >= 0
                && point.x_twips <= shape.width_twips
                && point.y_twips >= 0
                && point.y_twips <= shape.height_twips
        }),
        "circular arrow inner arc must stay inside passive frame"
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "hidden-circular-arrow-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "circular arrow metadata leaked to normalized text: {forbidden}"
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
    let mut passive_strokes = 0usize;
    for page_id in parsed_pdf.get_pages().values() {
        let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
        rendered_text.push_str(&decoded_pdf_text(&content));
        passive_shape_paints += content
            .operations
            .iter()
            .filter(|operation| operation.operator == "B")
            .count();
        passive_strokes += content
            .operations
            .iter()
            .filter(|operation| operation.operator == "S")
            .count();
    }

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        passive_shape_paints >= 1,
        "circular arrow should render as a passive fill/stroke path"
    );
    assert!(
        passive_strokes >= 1,
        "circular arrow inner arc should render as a passive stroke"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-circular-arrow-payload",
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
            "circular arrow metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_funnel_arrow_cloud_chart_and_inverse_line_shapes_render_passively_without_payload_leakage()
 {
    let mut input = String::from("{\\rtf1 Before\\par");
    for (shape_type, payload) in [
        (174, "office-funnel-payload"),
        (175, "office-pie-wedge-payload"),
        (176, "office-left-circular-arrow-payload"),
        (177, "office-left-right-circular-arrow-payload"),
        (178, "office-swoosh-arrow-payload"),
        (179, "office-cloud-payload"),
        (180, "office-chart-x-payload"),
        (181, "office-chart-star-payload"),
        (182, "office-chart-plus-payload"),
        (183, "office-line-inverse-payload"),
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
    let expected = [
        (StaticShapeKind::Polygon, 6),
        (StaticShapeKind::Polygon, 34),
        (StaticShapeKind::Polygon, 68),
        (StaticShapeKind::Polygon, 77),
        (StaticShapeKind::Polygon, 42),
        (StaticShapeKind::Polygon, 70),
        (StaticShapeKind::Polygon, 4),
        (StaticShapeKind::Polygon, 4),
        (StaticShapeKind::Polygon, 4),
        (StaticShapeKind::Polyline, 2),
    ];
    for (shape, (expected_kind, expected_point_count)) in shapes.iter().zip(expected) {
        assert_eq!(shape.kind, expected_kind);
        assert_eq!(shape.points.len(), expected_point_count);
        assert!(
            shape.points.iter().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "funnel/arrow/cloud/chart/line points must stay inside passive frame: {shape:?}"
        );
    }
    assert_eq!(shapes[0].points[0].x_twips, 0);
    assert!(
        shapes[1]
            .points
            .iter()
            .filter(|point| {
                point.y_twips <= shapes[1].height_twips / 2
                    || point.x_twips >= shapes[1].width_twips / 2
            })
            .count()
            >= 24,
        "pie wedge should preserve a bounded passive quarter-circle arc"
    );
    assert!(
        shapes[1]
            .points
            .iter()
            .any(|point| point.x_twips == shapes[1].width_twips
                && point.y_twips == shapes[1].height_twips),
        "pie wedge should preserve the passive arc endpoint at the right edge"
    );
    assert!(shapes[3].points.iter().any(|point| point.x_twips == 0));
    assert_eq!(
        shapes[2].overlay_paths.len(),
        1,
        "left circular arrow should preserve the passive inner arc"
    );
    assert_eq!(
        shapes[3].overlay_paths.len(),
        2,
        "left-right circular arrow should preserve both passive inner arcs"
    );
    assert!(
        shapes[2]
            .points
            .iter()
            .filter(|point| {
                point.x_twips >= shapes[2].width_twips / 4
                    && point.x_twips <= shapes[2].width_twips
                    && point.y_twips >= shapes[2].height_twips / 8
                    && point.y_twips <= (shapes[2].height_twips * 7) / 8
            })
            .count()
            >= 40,
        "left circular arrow should preserve a bounded passive circular body"
    );
    assert!(
        shapes[2].overlay_paths.iter().all(|path| path.len() >= 34),
        "left circular arrow inner arc should keep dense bounded passive samples"
    );
    assert!(
        shapes[3]
            .points
            .iter()
            .filter(|point| {
                point.y_twips <= shapes[3].height_twips / 2
                    && point.x_twips >= shapes[3].width_twips / 5
            })
            .count()
            >= 24,
        "left-right circular arrow should preserve a bounded passive upper circular band"
    );
    assert!(
        shapes[3]
            .points
            .iter()
            .filter(|point| {
                point.y_twips >= shapes[3].height_twips / 2
                    && point.x_twips <= (shapes[3].width_twips * 4) / 5
            })
            .count()
            >= 24,
        "left-right circular arrow should preserve a bounded passive lower circular band"
    );
    assert!(
        shapes[3].overlay_paths.iter().all(|path| path.len() >= 18),
        "left-right circular arrow inner arcs should keep dense bounded passive samples"
    );
    for shape in [&shapes[2], &shapes[3]] {
        assert!(
            shape.overlay_paths.iter().all(|path| path.len() >= 10),
            "circular arrow inner arcs should be bounded passive polylines"
        );
        assert!(
            shape.overlay_paths.iter().flatten().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "circular arrow inner arcs must stay inside passive frame: {shape:?}"
        );
    }
    assert!(
        shapes[4]
            .points
            .iter()
            .filter(|point| {
                point.y_twips <= (shapes[4].height_twips * 2) / 3
                    && point.x_twips <= shapes[4].width_twips
            })
            .count()
            >= 21,
        "swoosh arrow should preserve a bounded passive upper sweep"
    );
    assert!(
        shapes[4]
            .points
            .iter()
            .filter(|point| {
                point.y_twips >= shapes[4].height_twips / 2
                    && point.x_twips <= shapes[4].width_twips
            })
            .count()
            >= 15,
        "swoosh arrow should preserve a bounded passive lower return edge"
    );
    assert!(
        shapes[5]
            .points
            .iter()
            .any(|point| point.x_twips == shapes[5].width_twips)
    );
    assert!(
        shapes[5]
            .points
            .iter()
            .filter(|point| {
                point.y_twips <= shapes[5].height_twips / 5
                    && point.x_twips >= shapes[5].width_twips / 4
            })
            .count()
            >= 15,
        "cloud should preserve bounded passive top lobes"
    );
    assert!(
        shapes[5]
            .points
            .iter()
            .filter(|point| {
                point.x_twips >= (shapes[5].width_twips * 4) / 5
                    && point.y_twips >= shapes[5].height_twips / 4
                    && point.y_twips <= (shapes[5].height_twips * 3) / 4
            })
            .count()
            >= 9,
        "cloud should preserve bounded passive right lobes"
    );
    assert!(
        shapes[5]
            .points
            .iter()
            .filter(|point| {
                point.y_twips >= (shapes[5].height_twips * 3) / 4
                    && point.x_twips >= shapes[5].width_twips / 4
                    && point.x_twips <= (shapes[5].width_twips * 4) / 5
            })
            .count()
            >= 11,
        "cloud should preserve bounded passive lower lobes"
    );
    for shape in [&shapes[6], &shapes[7], &shapes[8]] {
        assert_eq!(
            shape.point_paths.len(),
            1,
            "chart symbol should keep its glyph as a separate passive subpath"
        );
        assert!(
            shape.point_paths.iter().flatten().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "chart symbol subpaths must stay inside passive frame: {shape:?}"
        );
    }
    assert_eq!(shapes[9].points[0].x_twips, shapes[9].width_twips);
    assert_eq!(shapes[9].points[1].y_twips, shapes[9].height_twips);
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "office-funnel-payload",
        "office-pie-wedge-payload",
        "office-left-circular-arrow-payload",
        "office-left-right-circular-arrow-payload",
        "office-swoosh-arrow-payload",
        "office-cloud-payload",
        "office-chart-x-payload",
        "office-chart-star-payload",
        "office-chart-plus-payload",
        "office-line-inverse-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "funnel/arrow/cloud/chart/line metadata leaked to normalized text: {forbidden}"
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
    let mut passive_strokes = 0usize;
    let mut passive_path_moves = 0usize;
    for page_id in parsed_pdf.get_pages().values() {
        let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
        rendered_text.push_str(&decoded_pdf_text(&content));
        for operation in &content.operations {
            if operation.operator == "B" {
                passive_shape_paints += 1;
            }
            if operation.operator == "S" {
                passive_strokes += 1;
            }
            if operation.operator == "m" {
                passive_path_moves += 1;
            }
        }
    }

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        passive_shape_paints >= 9,
        "funnel/arrow/cloud/chart shapes should render passive fill/stroke paths"
    );
    assert!(
        passive_strokes >= 1,
        "inverse line should render as a passive stroked path"
    );
    assert!(
        passive_strokes >= 4,
        "circular arrow inner arcs and inverse line should render as passive strokes"
    );
    assert!(
        passive_path_moves >= 12,
        "chart symbols should emit separate passive PDF subpaths"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"office-funnel-payload",
        b"office-pie-wedge-payload",
        b"office-left-circular-arrow-payload",
        b"office-left-right-circular-arrow-payload",
        b"office-swoosh-arrow-payload",
        b"office-cloud-payload",
        b"office-chart-x-payload",
        b"office-chart-star-payload",
        b"office-chart-plus-payload",
        b"office-line-inverse-payload",
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
            "funnel/arrow/cloud/chart/line metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_right_arrow_callouts_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright3600\shpbottom1800{\sp{\sn shapeType}{\sv 53}}{\sp{\sn fillColor}{\sv 16744448}}{\sp{\sn lineColor}{\sv 255}}{\sp{\sn pFragments}{\sv hidden-right-arrow-callout-payload}}}}After\par}"#.to_vec();
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
        .expect("passive right arrow callout shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 7);
    assert!(
        shape
            .points
            .iter()
            .any(|point| point.x_twips == shape.width_twips
                && point.y_twips == shape.height_twips / 2),
        "right arrow callout should keep a passive point at the arrow tip"
    );
    assert!(
        shape
            .points
            .iter()
            .any(|point| point.x_twips == 0 && point.y_twips == 0)
            && shape
                .points
                .iter()
                .any(|point| point.x_twips == 0 && point.y_twips == shape.height_twips),
        "right arrow callout should preserve the full-height callout body"
    );
    assert_eq!(
        shape.overlay_paths.len(),
        1,
        "right arrow callout should preserve the passive body/head separator"
    );
    assert_eq!(shape.overlay_paths[0].len(), 2);
    assert!(
        shape.overlay_paths.iter().flatten().all(|point| {
            point.x_twips >= 0
                && point.x_twips <= shape.width_twips
                && point.y_twips >= 0
                && point.y_twips <= shape.height_twips
        }),
        "right arrow callout separator must stay inside passive frame"
    );
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 0,
            green: 128,
            blue: 255,
        })
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "hidden-right-arrow-callout-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "right arrow callout metadata leaked to normalized text: {forbidden}"
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
            .any(|operation| operation.operator == "B"),
        "right arrow callout fill/stroke should render passively"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "S"),
        "right arrow callout separator should render as a passive stroke"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-right-arrow-callout-payload",
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
            "right arrow callout metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_cardinal_arrow_callouts_render_as_passive_polygons_without_payload_leakage() {
    for (shape_type, label, expected_tip, expected_body_corner) in [
        (54, "left", (0, 540), (2880, 0)),
        (55, "up", (1440, 0), (2880, 1080)),
        (56, "down", (1440, 1080), (2880, 0)),
    ] {
        let payload = format!("hidden-{label}-arrow-callout-payload");
        let shape_type = shape_type.to_string();
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
            "shpright3600",
            "\\",
            "shpbottom1800{",
            "\\",
            "sp{",
            "\\",
            "sn shapeType}{",
            "\\",
            "sv ",
            shape_type.as_str(),
            "}}{",
            "\\",
            "sp{",
            "\\",
            "sn fillColor}{",
            "\\",
            "sv 10040064}}{",
            "\\",
            "sp{",
            "\\",
            "sn lineColor}{",
            "\\",
            "sv 16711680}}{",
            "\\",
            "sp{",
            "\\",
            "sn pFragments}{",
            "\\",
            "sv ",
            payload.as_str(),
            "}}}}After",
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
            .unwrap_or_else(|| panic!("passive {label} arrow callout shape"));

        assert!(text.contains("Before"));
        assert!(text.contains("After"));
        assert_eq!(shape.kind, StaticShapeKind::Polygon);
        assert_eq!(shape.points.len(), 7);
        assert!(
            shape
                .points
                .iter()
                .any(|point| point.x_twips == expected_tip.0 && point.y_twips == expected_tip.1),
            "{label} arrow callout should keep passive arrow-tip point"
        );
        assert!(
            shape
                .points
                .iter()
                .any(|point| point.x_twips == expected_body_corner.0
                    && point.y_twips == expected_body_corner.1),
            "{label} arrow callout should preserve a passive body corner"
        );
        assert_eq!(
            shape.fill_color,
            Some(open_rtf_converter::model::Color {
                red: 0,
                green: 51,
                blue: 153,
            })
        );
        for forbidden in [
            "shapeType",
            "fillColor",
            "lineColor",
            "pFragments",
            payload.as_str(),
            "[Shape skipped",
        ] {
            assert!(
                !text.contains(forbidden),
                "{label} arrow callout metadata leaked to normalized text: {forbidden}"
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
                .any(|operation| operation.operator == "B"),
            "{label} arrow callout fill/stroke should render passively"
        );
        for forbidden in [
            b"shapeType".as_slice(),
            b"fillColor",
            b"lineColor",
            b"pFragments",
            payload.as_bytes(),
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
                "{label} arrow callout metadata leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn office_multi_arrow_callouts_render_as_passive_polygons_without_payload_leakage() {
    for (shape_type, label, expected_points, expected_tips) in [
        (
            57,
            "left-right",
            10,
            [
                (0, 540),
                (2880, 540),
                (i32::MIN, i32::MIN),
                (i32::MIN, i32::MIN),
            ],
        ),
        (
            58,
            "up-down",
            10,
            [
                (1440, 0),
                (1440, 1080),
                (i32::MIN, i32::MIN),
                (i32::MIN, i32::MIN),
            ],
        ),
        (
            59,
            "quad",
            12,
            [(1440, 0), (2880, 540), (1440, 1080), (0, 540)],
        ),
    ] {
        let payload = format!("hidden-{label}-arrow-callout-payload");
        let shape_type = shape_type.to_string();
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
            "shpright3600",
            "\\",
            "shpbottom1800{",
            "\\",
            "sp{",
            "\\",
            "sn shapeType}{",
            "\\",
            "sv ",
            shape_type.as_str(),
            "}}{",
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
            "sn pFragments}{",
            "\\",
            "sv ",
            payload.as_str(),
            "}}}}After",
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
            .unwrap_or_else(|| panic!("passive {label} arrow callout shape"));

        assert!(text.contains("Before"));
        assert!(text.contains("After"));
        assert_eq!(shape.kind, StaticShapeKind::Polygon);
        assert_eq!(shape.points.len(), expected_points);
        for expected_tip in expected_tips
            .into_iter()
            .filter(|point| point.0 != i32::MIN)
        {
            assert!(
                shape
                    .points
                    .iter()
                    .any(|point| point.x_twips == expected_tip.0
                        && point.y_twips == expected_tip.1),
                "{label} arrow callout should keep passive arrow-tip point {expected_tip:?}"
            );
        }
        assert_eq!(
            shape.fill_color,
            Some(open_rtf_converter::model::Color {
                red: 0,
                green: 255,
                blue: 0,
            })
        );
        for forbidden in [
            "shapeType",
            "fillColor",
            "lineColor",
            "pFragments",
            payload.as_str(),
            "[Shape skipped",
        ] {
            assert!(
                !text.contains(forbidden),
                "{label} arrow callout metadata leaked to normalized text: {forbidden}"
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
                .any(|operation| operation.operator == "B"),
            "{label} arrow callout fill/stroke should render passively"
        );
        for forbidden in [
            b"shapeType".as_slice(),
            b"fillColor",
            b"lineColor",
            b"pFragments",
            payload.as_bytes(),
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
                "{label} arrow callout metadata leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}
