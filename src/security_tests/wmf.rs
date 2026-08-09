use super::*;

#[test]
fn inline_wmf_establishes_word_baselines_without_payload_leakage() {
    let output = convert_rtf_to_pdf(
        include_bytes!("../../fixtures/wmf-vector-passive.rtf"),
        &ConvertOptions::browser_safe_defaults(),
    )
    .unwrap();
    audit_passive_pdf_bytes(&output.pdf).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let after = pdf_first_text_position_for_text(&content, "After").expect("text after WMF");

    assert!(
        (after.1 - 310.18).abs() < 0.03,
        "inline WMF line should use Word's image-established baseline: {after:?}"
    );
    for forbidden in [
        b"wmetafile".as_slice(),
        b"0100090000032a",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
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
fn wmf_mm_text_setmapmode_is_passive_state_without_payload_leakage() {
    let mut mode = wmf_u16_record(0x0103, 1);
    mode.extend_from_slice(b"WMF-SETMAPMODE-MM-TEXT /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(2), 0);
    let mode_len = (mode.len() / 2) as u32;
    write_test_le_u32(&mut mode, 0, mode_len);
    let records = [mode, wmf_bounds_record(0x041b, 20, 10, 80, 50)];
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
        .expect("passive WMF MM_TEXT map mode vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "WMF-SETMAPMODE-MM-TEXT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF SETMAPMODE payload/control leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after WMF SETMAPMODE MM_TEXT should render a passive PDF rectangle"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-SETMAPMODE-MM-TEXT",
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
            "WMF SETMAPMODE payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn strict_spec_anisotropic_wmf_setmapmode_is_passive_state_without_payload_leakage() {
    let mut mode = wmf_u16_record(0x0103, 8);
    mode.extend_from_slice(b"WMF-SETMAPMODE-ANISOTROPIC /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(2), 0);
    let mode_len = (mode.len() / 2) as u32;
    write_test_le_u32(&mut mode, 0, mode_len);
    let records = [mode, wmf_bounds_record(0x041b, 20, 10, 80, 50)];
    let wmf = minimal_wmf_with_records(160, 80, &records);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes_with_options(
        &input,
        &RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        },
    )
    .unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("passive WMF anisotropic map mode vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 20.0,
            top: 10.0,
            right: 80.0,
            bottom: 50.0,
            ..
        }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "WMF-SETMAPMODE-ANISOTROPIC",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF anisotropic SETMAPMODE payload/control leaked to normalized text: {forbidden}"
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

    assert!(output.diagnostics.is_empty());
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-SETMAPMODE-ANISOTROPIC",
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
            "WMF anisotropic SETMAPMODE payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_wmf_setmapmode_becomes_passive_placeholder() {
    let records = [wmf_function_record(0x0103)];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
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
        .expect("malformed WMF SETMAPMODE placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn wmf_copypen_setrop2_is_passive_state_without_payload_leakage() {
    let mut mode = wmf_u16_record(0x0104, 13);
    mode.extend_from_slice(b"WMF-SETROP2-COPYPEN /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(2), 0);
    let mode_len = (mode.len() / 2) as u32;
    write_test_le_u32(&mut mode, 0, mode_len);
    let records = [mode, wmf_bounds_record(0x041b, 20, 10, 80, 50)];
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
        .expect("passive WMF R2_COPYPEN mode vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "WMF-SETROP2-COPYPEN",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF SETROP2 payload/control leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after WMF SETROP2 R2_COPYPEN should render a passive PDF rectangle"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-SETROP2-COPYPEN",
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
            "WMF SETROP2 payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn notcopypen_wmf_setrop2_inverts_passive_pen_without_payload_leakage() {
    let mut mode = wmf_u16_record(0x0104, 4);
    mode.extend_from_slice(b"WMF-SETROP2-NOTCOPYPEN /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(2), 0);
    let mode_len = (mode.len() / 2) as u32;
    write_test_le_u32(&mut mode, 0, mode_len);
    let records = [
        mode,
        wmf_create_pen_record(
            0,
            1,
            Color {
                red: 20,
                green: 40,
                blue: 60,
            },
        ),
        wmf_select_object_record(0),
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
        .expect("passive WMF R2_NOTCOPYPEN vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            stroke_color: Some(color),
            ..
        } if color.red == 235 && color.green == 215 && color.blue == 195
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "WMF-SETROP2-NOTCOPYPEN",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF SETROP2 R2_NOTCOPYPEN payload/control leaked to normalized text: {forbidden}"
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

    assert!(output.diagnostics.is_empty());
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-SETROP2-NOTCOPYPEN",
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
            "WMF SETROP2 R2_NOTCOPYPEN payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn backdrop_dependent_wmf_setrop2_stays_partial_without_payload_leakage() {
    let mut mode = wmf_u16_record(0x0104, 6);
    mode.extend_from_slice(b"WMF-SETROP2-BACKDROP /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(2), 0);
    let mode_len = (mode.len() / 2) as u32;
    write_test_le_u32(&mut mode, 0, mode_len);
    let records = [mode, wmf_bounds_record(0x041b, 20, 10, 80, 50)];
    let wmf = minimal_wmf_with_records(160, 80, &records);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
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
            .contains("1 unsupported record(s) skipped")
    }));
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-SETROP2-BACKDROP",
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
            "unsupported WMF SETROP2 payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_wmf_setrop2_becomes_passive_placeholder() {
    let records = [wmf_function_record(0x0104)];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
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
        .expect("malformed WMF SETROP2 placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn wmf_default_setviewportorg_is_passive_state_without_payload_leakage() {
    let mut mode = wmf_yx_record(0x020d, 0, 0);
    mode.extend_from_slice(b"WMF-SETVIEWPORTORG-DEFAULT /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(2), 0);
    let mode_len = (mode.len() / 2) as u32;
    write_test_le_u32(&mut mode, 0, mode_len);
    let records = [mode, wmf_bounds_record(0x041b, 20, 10, 80, 50)];
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
        .expect("passive WMF default viewport origin vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "WMF-SETVIEWPORTORG-DEFAULT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF SETVIEWPORTORG payload/control leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after default WMF SETVIEWPORTORG should render a passive PDF rectangle"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-SETVIEWPORTORG-DEFAULT",
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
            "WMF SETVIEWPORTORG payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn nonzero_wmf_setviewportorg_maps_passive_coordinates_without_payload_leakage() {
    let mut mode = wmf_yx_record(0x020d, 3, 5);
    mode.extend_from_slice(b"WMF-SETVIEWPORTORG-NONZERO /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(2), 0);
    let mode_len = (mode.len() / 2) as u32;
    write_test_le_u32(&mut mode, 0, mode_len);
    let records = [mode, wmf_bounds_record(0x041b, 20, 10, 80, 50)];
    let wmf = minimal_wmf_with_records(160, 80, &records);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("passive WMF nonzero viewport origin vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 25.0,
            top: 13.0,
            right: 85.0,
            bottom: 53.0,
            ..
        }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "WMF-SETVIEWPORTORG-NONZERO",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF SETVIEWPORTORG payload/control leaked to normalized text: {forbidden}"
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
    assert!(output.diagnostics.is_empty());
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after nonzero WMF SETVIEWPORTORG should render a passive PDF rectangle"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-SETVIEWPORTORG-NONZERO",
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
            "WMF SETVIEWPORTORG payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_wmf_setviewportorg_becomes_passive_placeholder() {
    let records = [wmf_function_record(0x020d)];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
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
        .expect("malformed WMF SETVIEWPORTORG placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn wmf_zero_offsetviewportorg_is_passive_state_without_payload_leakage() {
    let mut mode = wmf_yx_record(0x0211, 0, 0);
    mode.extend_from_slice(b"WMF-OFFSETVIEWPORTORG-ZERO /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(2), 0);
    let mode_len = (mode.len() / 2) as u32;
    write_test_le_u32(&mut mode, 0, mode_len);
    let records = [mode, wmf_bounds_record(0x041b, 20, 10, 80, 50)];
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
        .expect("passive WMF zero OFFSETVIEWPORTORG vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "WMF-OFFSETVIEWPORTORG-ZERO",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF OFFSETVIEWPORTORG payload/control leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after zero WMF OFFSETVIEWPORTORG should render a passive PDF rectangle"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-OFFSETVIEWPORTORG-ZERO",
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
            "WMF OFFSETVIEWPORTORG payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn nonzero_wmf_offsetviewportorg_maps_passive_coordinates_without_payload_leakage() {
    let mut mode = wmf_yx_record(0x0211, 3, 5);
    mode.extend_from_slice(b"WMF-OFFSETVIEWPORTORG-NONZERO /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(2), 0);
    let mode_len = (mode.len() / 2) as u32;
    write_test_le_u32(&mut mode, 0, mode_len);
    let records = [mode, wmf_bounds_record(0x041b, 20, 10, 80, 50)];
    let wmf = minimal_wmf_with_records(160, 80, &records);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("passive WMF nonzero viewport offset vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 25.0,
            top: 13.0,
            right: 85.0,
            bottom: 53.0,
            ..
        }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "WMF-OFFSETVIEWPORTORG-NONZERO",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF OFFSETVIEWPORTORG payload/control leaked to normalized text: {forbidden}"
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
    assert!(output.diagnostics.is_empty());
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after nonzero WMF OFFSETVIEWPORTORG should render a passive PDF rectangle"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-OFFSETVIEWPORTORG-NONZERO",
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
            "WMF OFFSETVIEWPORTORG payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_wmf_offsetviewportorg_becomes_passive_placeholder() {
    let records = [wmf_function_record(0x0211)];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
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
        .expect("malformed WMF OFFSETVIEWPORTORG placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn wmf_matching_setviewportext_is_passive_state_without_payload_leakage() {
    let mut mode = wmf_yx_record(0x020e, 80, 160);
    mode.extend_from_slice(b"WMF-SETVIEWPORTEXT-MATCH /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(2), 0);
    let mode_len = (mode.len() / 2) as u32;
    write_test_le_u32(&mut mode, 0, mode_len);
    let records = [mode, wmf_bounds_record(0x041b, 20, 10, 80, 50)];
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
        .expect("passive WMF matching viewport extent vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "WMF-SETVIEWPORTEXT-MATCH",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF SETVIEWPORTEXT payload/control leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after matching WMF SETVIEWPORTEXT should render a passive PDF rectangle"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-SETVIEWPORTEXT-MATCH",
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
            "WMF SETVIEWPORTEXT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn nonmatching_wmf_setviewportext_scales_passive_coordinates_without_payload_leakage() {
    let mut mode = wmf_yx_record(0x020e, 40, 160);
    mode.extend_from_slice(b"WMF-SETVIEWPORTEXT-UNSUPPORTED /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(2), 0);
    let mode_len = (mode.len() / 2) as u32;
    write_test_le_u32(&mut mode, 0, mode_len);
    let records = [mode, wmf_bounds_record(0x041b, 20, 10, 80, 50)];
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
        .expect("passive WMF scaled viewport extent vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 20.0,
            top: 5.0,
            right: 80.0,
            bottom: 25.0,
            ..
        }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "WMF-SETVIEWPORTEXT-UNSUPPORTED",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF SETVIEWPORTEXT payload/control leaked to normalized text: {forbidden}"
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

    assert!(output.diagnostics.is_empty());
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-SETVIEWPORTEXT-UNSUPPORTED",
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
            "unsupported WMF SETVIEWPORTEXT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_wmf_setviewportext_becomes_passive_placeholder() {
    let records = [wmf_function_record(0x020e)];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
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
        .expect("malformed WMF SETVIEWPORTEXT placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn strict_spec_wmf_offsetwindoworg_maps_passive_coordinates_without_payload_leakage() {
    let mut offset = wmf_yx_record(0x020f, 5, 10);
    offset.extend_from_slice(b"WMF-OFFSETWINDOWORG /JavaScript /EmbeddedFile /Launch");
    offset.resize(offset.len().next_multiple_of(2), 0);
    let offset_len = (offset.len() / 2) as u32;
    write_test_le_u32(&mut offset, 0, offset_len);
    let records = [offset, wmf_bounds_record(0x041b, 30, 15, 90, 55)];
    let wmf = minimal_wmf_with_records(160, 80, &records);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("passive WMF offset window origin vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 20.0,
            top: 10.0,
            right: 80.0,
            bottom: 50.0,
            ..
        }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "WMF-OFFSETWINDOWORG",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF OFFSETWINDOWORG payload/control leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after WMF OFFSETWINDOWORG should render a passive PDF rectangle"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-OFFSETWINDOWORG",
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
            "WMF OFFSETWINDOWORG payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_wmf_offsetwindoworg_becomes_passive_placeholder() {
    let records = [wmf_function_record(0x020f)];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
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
        .expect("malformed WMF OFFSETWINDOWORG placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn malformed_wmf_coordinate_state_counts_as_partial_without_payload_leakage() {
    for (function, label) in [
        (0x020b, "WMF-SETWINDOWORG-MALFORMED"),
        (0x020c, "WMF-SETWINDOWEXT-MALFORMED"),
        (0x020d, "WMF-SETVIEWPORTORG-MALFORMED"),
        (0x020e, "WMF-SETVIEWPORTEXT-MALFORMED"),
        (0x020f, "WMF-OFFSETWINDOWORG-MALFORMED"),
        (0x0211, "WMF-OFFSETVIEWPORTORG-MALFORMED"),
        (0x0410, "WMF-SCALEWINDOWEXT-MALFORMED"),
        (0x0412, "WMF-SCALEVIEWPORTEXT-MALFORMED"),
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
            .expect("malformed WMF coordinate state partial vector preview");

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
                "malformed WMF coordinate state payload/control leaked to normalized text: {forbidden}"
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
                "malformed WMF coordinate state payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn malformed_wmf_line_position_records_count_as_partial_without_payload_leakage() {
    for (function, label) in [
        (0x0214, "WMF-MOVETO-MALFORMED"),
        (0x0213, "WMF-LINETO-MALFORMED"),
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
            .expect("malformed WMF line-position partial vector preview");

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
                "malformed WMF line-position payload/control leaked to normalized text: {forbidden}"
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
                "malformed WMF line-position payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn malformed_wmf_clip_state_records_count_as_partial_without_payload_leakage() {
    for (function, label) in [
        (0x012e, "WMF-SETTEXTALIGN-MALFORMED"),
        (0x0220, "WMF-OFFSETCLIPRGN-MALFORMED"),
        (0x0415, "WMF-EXCLUDECLIPRECT-MALFORMED"),
        (0x0416, "WMF-INTERSECTCLIPRECT-MALFORMED"),
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
            .expect("malformed WMF clip/state partial vector preview");

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
                "malformed WMF clip/state payload/control leaked to normalized text: {forbidden}"
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
                "malformed WMF clip/state payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn malformed_wmf_restore_state_records_count_as_partial_without_payload_leakage() {
    let mut out_of_range_restore = wmf_u16_record(0x0127, 0xfffe);
    out_of_range_restore.extend_from_slice(b"WMF-RESTOREDC-OUT-OF-RANGE /JavaScript");
    out_of_range_restore.resize(out_of_range_restore.len().next_multiple_of(2), 0);
    let out_of_range_restore_len = (out_of_range_restore.len() / 2) as u32;
    write_test_le_u32(&mut out_of_range_restore, 0, out_of_range_restore_len);

    for (records, label) in [
        (
            vec![
                wmf_function_record(0x0127),
                wmf_bounds_record(0x041b, 20, 10, 80, 50),
            ],
            "WMF-RESTOREDC-MALFORMED",
        ),
        (
            vec![
                wmf_function_record(0x001e),
                wmf_function_record(0x0127),
                wmf_bounds_record(0x041b, 20, 10, 80, 50),
            ],
            "WMF-SAVEDC-RESTOREDC-MALFORMED",
        ),
        (
            vec![
                wmf_function_record(0x001e),
                out_of_range_restore,
                wmf_bounds_record(0x041b, 20, 10, 80, 50),
            ],
            "WMF-SAVEDC-RESTOREDC-OUT-OF-RANGE",
        ),
    ] {
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
            .expect("malformed WMF restore-state partial vector preview");

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
                "malformed WMF restore-state payload/control leaked to normalized text: {forbidden}"
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
                "malformed WMF restore-state payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn malformed_wmf_arc_records_count_as_partial_without_payload_leakage() {
    for (function, label) in [
        (0x0817, "WMF-ARC-MALFORMED"),
        (0x081a, "WMF-PIE-MALFORMED"),
        (0x0830, "WMF-CHORD-MALFORMED"),
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
            .expect("malformed WMF arc partial vector preview");

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
                "malformed WMF arc payload/control leaked to normalized text: {forbidden}"
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
                "malformed WMF arc payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn malformed_wmf_shape_bounds_count_as_partial_without_payload_leakage() {
    for (function, label) in [
        (0x041b, "WMF-RECTANGLE-MALFORMED"),
        (0x0418, "WMF-ELLIPSE-MALFORMED"),
        (0x061c, "WMF-ROUNDRECT-MALFORMED"),
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
            .expect("malformed WMF shape-bounds partial vector preview");

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
                "malformed WMF shape payload/control leaked to normalized text: {forbidden}"
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
                "malformed WMF shape payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn wmf_identity_scalewindowext_is_passive_state_without_payload_leakage() {
    let mut mode = wmf_scale_record(0x0410, 1, 1, 1, 1);
    mode.extend_from_slice(b"WMF-SCALEWINDOWEXT-IDENTITY /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(2), 0);
    let mode_len = (mode.len() / 2) as u32;
    write_test_le_u32(&mut mode, 0, mode_len);
    let records = [mode, wmf_bounds_record(0x041b, 20, 10, 80, 50)];
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
        .expect("passive WMF identity SCALEWINDOWEXT vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "WMF-SCALEWINDOWEXT-IDENTITY",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF SCALEWINDOWEXT payload/control leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after identity WMF SCALEWINDOWEXT should render a passive PDF rectangle"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-SCALEWINDOWEXT-IDENTITY",
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
            "WMF SCALEWINDOWEXT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn nonidentity_wmf_scalewindowext_scales_passive_coordinates_without_payload_leakage() {
    let mut mode = wmf_scale_record(0x0410, 1, 2, 1, 1);
    mode.extend_from_slice(b"WMF-SCALEWINDOWEXT-SCALED /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(2), 0);
    let mode_len = (mode.len() / 2) as u32;
    write_test_le_u32(&mut mode, 0, mode_len);
    let records = [mode, wmf_bounds_record(0x041b, 20, 10, 80, 50)];
    let wmf = minimal_wmf_with_records(160, 80, &records);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("passive WMF nonidentity SCALEWINDOWEXT vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.width_px, 160);
    assert_eq!(image.height_px, 80);
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 20.0,
            top: 5.0,
            right: 80.0,
            bottom: 25.0,
            ..
        }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "WMF-SCALEWINDOWEXT-SCALED",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF SCALEWINDOWEXT payload/control leaked to normalized text: {forbidden}"
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

    assert!(output.diagnostics.is_empty());
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-SCALEWINDOWEXT-SCALED",
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
            "WMF SCALEWINDOWEXT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn zero_denom_wmf_scalewindowext_becomes_passive_placeholder() {
    let records = [wmf_scale_record(0x0410, 0, 1, 1, 1)];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
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
        .expect("zero-denominator WMF SCALEWINDOWEXT placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn malformed_wmf_scalewindowext_becomes_passive_placeholder() {
    let records = [wmf_function_record(0x0410)];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
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
        .expect("malformed WMF SCALEWINDOWEXT placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn wmf_identity_scaleviewportext_is_passive_state_without_payload_leakage() {
    let mut mode = wmf_scale_record(0x0412, 1, 1, 1, 1);
    mode.extend_from_slice(b"WMF-SCALEVIEWPORTEXT-IDENTITY /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(2), 0);
    let mode_len = (mode.len() / 2) as u32;
    write_test_le_u32(&mut mode, 0, mode_len);
    let records = [mode, wmf_bounds_record(0x041b, 20, 10, 80, 50)];
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
        .expect("passive WMF identity SCALEVIEWPORTEXT vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "WMF-SCALEVIEWPORTEXT-IDENTITY",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF SCALEVIEWPORTEXT payload/control leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after identity WMF SCALEVIEWPORTEXT should render a passive PDF rectangle"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-SCALEVIEWPORTEXT-IDENTITY",
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
            "WMF SCALEVIEWPORTEXT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn nonidentity_wmf_scaleviewportext_scales_passive_coordinates_without_payload_leakage() {
    let mut mode = wmf_scale_record(0x0412, 2, 1, 2, 1);
    mode.extend_from_slice(b"WMF-SCALEVIEWPORTEXT-UNSUPPORTED /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(2), 0);
    let mode_len = (mode.len() / 2) as u32;
    write_test_le_u32(&mut mode, 0, mode_len);
    let records = [mode, wmf_bounds_record(0x041b, 20, 10, 80, 50)];
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
        .expect("passive WMF scaled SCALEVIEWPORTEXT vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 10.0,
            top: 5.0,
            right: 40.0,
            bottom: 25.0,
            ..
        }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "WMF-SCALEVIEWPORTEXT-UNSUPPORTED",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF SCALEVIEWPORTEXT payload/control leaked to normalized text: {forbidden}"
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

    assert!(output.diagnostics.is_empty());
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-SCALEVIEWPORTEXT-UNSUPPORTED",
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
            "unsupported WMF SCALEVIEWPORTEXT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn zero_denom_wmf_scaleviewportext_becomes_passive_placeholder() {
    let records = [wmf_scale_record(0x0412, 1, 1, 0, 1)];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
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
        .expect("zero-denominator WMF SCALEVIEWPORTEXT placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn malformed_wmf_scaleviewportext_becomes_passive_placeholder() {
    let records = [wmf_function_record(0x0412)];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
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
        .expect("malformed WMF SCALEVIEWPORTEXT placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn wmf_palette_records_are_passive_noops_without_payload_leakage() {
    let mut realize = wmf_function_record(0x0035);
    realize.extend_from_slice(b"WMF-REALIZEPALETTE /JavaScript /EmbeddedFile");
    realize.resize(realize.len().next_multiple_of(2), 0);
    let realize_len = (realize.len() / 2) as u32;
    write_test_le_u32(&mut realize, 0, realize_len);

    let mut select = wmf_u16_record(0x0234, 0);
    select.extend_from_slice(b"WMF-SELECTPALETTE /Launch /RichMedia");
    select.resize(select.len().next_multiple_of(2), 0);
    let select_len = (select.len() / 2) as u32;
    write_test_le_u32(&mut select, 0, select_len);

    let records = [realize, select, wmf_bounds_record(0x041b, 20, 10, 80, 50)];
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
        .expect("passive WMF palette no-op vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "WMF-REALIZEPALETTE",
        "WMF-SELECTPALETTE",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
        "RichMedia",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF palette payload/control leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after WMF palette no-ops should render a passive PDF rectangle"
    );
    assert!(output.diagnostics.is_empty());
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-REALIZEPALETTE",
        b"WMF-SELECTPALETTE",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF palette payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_wmf_selectpalette_becomes_passive_placeholder() {
    let records = [wmf_function_record(0x0234)];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
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
        .expect("malformed WMF SELECTPALETTE placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn malformed_wmf_createpalette_becomes_passive_placeholder() {
    let mut palette = wmf_create_palette_record(&[]);
    write_test_le_u16(&mut palette, 8, 2);
    let records = [palette];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
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
        .expect("malformed WMF CREATEPALETTE placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn wmf_palette_mutation_records_are_passive_noops_without_payload_leakage() {
    let mut set_entries = wmf_palette_entries_record(0x0037, 1, &[(12, 34, 56), (78, 90, 123)]);
    set_entries.extend_from_slice(b"WMF-SETPALENTRIES /JavaScript /EmbeddedFile");
    set_entries.resize(set_entries.len().next_multiple_of(2), 0);
    let set_entries_len = (set_entries.len() / 2) as u32;
    write_test_le_u32(&mut set_entries, 0, set_entries_len);

    let mut resize = wmf_u16_record(0x0139, 16);
    resize.extend_from_slice(b"WMF-RESIZEPALETTE /Launch /RichMedia");
    resize.resize(resize.len().next_multiple_of(2), 0);
    let resize_len = (resize.len() / 2) as u32;
    write_test_le_u32(&mut resize, 0, resize_len);

    let mut animate = wmf_palette_entries_record(0x0436, 0, &[(220, 10, 80)]);
    animate.extend_from_slice(b"WMF-ANIMATEPALETTE /OpenAction /JavaScript");
    animate.resize(animate.len().next_multiple_of(2), 0);
    let animate_len = (animate.len() / 2) as u32;
    write_test_le_u32(&mut animate, 0, animate_len);

    let records = [
        set_entries,
        resize,
        animate,
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
        .expect("passive WMF palette mutation vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "WMF-SETPALENTRIES",
        "WMF-RESIZEPALETTE",
        "WMF-ANIMATEPALETTE",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
        "RichMedia",
        "OpenAction",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF palette mutation payload/control leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after WMF palette mutations should render a passive PDF rectangle"
    );
    assert!(output.diagnostics.is_empty());
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-SETPALENTRIES",
        b"WMF-RESIZEPALETTE",
        b"WMF-ANIMATEPALETTE",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF palette mutation payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_wmf_setpalentries_becomes_passive_placeholder() {
    let mut entries = wmf_palette_entries_record(0x0037, 0, &[(255, 0, 0)]);
    write_test_le_u16(&mut entries, 8, 2);
    let records = [entries];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
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
        .expect("malformed WMF SETPALENTRIES placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn wmf_setmapperflags_is_passive_noop_without_payload_leakage() {
    let mut mapper = wmf_u32_record(0x0231, 1);
    mapper.extend_from_slice(b"WMF-SETMAPPERFLAGS /JavaScript /EmbeddedFile /Launch");
    mapper.resize(mapper.len().next_multiple_of(2), 0);
    let mapper_len = (mapper.len() / 2) as u32;
    write_test_le_u32(&mut mapper, 0, mapper_len);
    let records = [
        mapper,
        wmf_exttextout_record(30, 20, "Mapper", 0, None),
        wmf_bounds_record(0x041b, 20, 40, 90, 70),
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
        .expect("passive WMF SETMAPPERFLAGS vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "WMF-SETMAPPERFLAGS",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF SETMAPPERFLAGS payload/control leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("Mapper"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after WMF SETMAPPERFLAGS should render a passive PDF rectangle"
    );
    assert!(output.diagnostics.is_empty());
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-SETMAPPERFLAGS",
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
            "WMF SETMAPPERFLAGS payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_wmf_setmapperflags_becomes_passive_placeholder() {
    let records = [wmf_function_record(0x0231)];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
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
        .expect("malformed WMF SETMAPPERFLAGS placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn unsupported_wmf_pen_and_brush_styles_are_inert_without_payload_leakage() {
    let mut pen = vec![0; 16];
    write_test_le_u32(&mut pen, 0, 8);
    write_test_le_u16(&mut pen, 4, 0x02fa);
    write_test_le_u16(&mut pen, 6, 9);
    write_test_le_i16(&mut pen, 8, 7);
    pen[12] = 220;
    pen[13] = 80;
    pen[14] = 40;
    pen.extend_from_slice(b"WMF-CREATEPEN-UNSUPPORTED /JavaScript /EmbeddedFile /Launch");
    pen.resize(pen.len().next_multiple_of(2), 0);
    let pen_len = (pen.len() / 2) as u32;
    write_test_le_u32(&mut pen, 0, pen_len);

    let mut brush = wmf_create_brush_record(Color {
        red: 220,
        green: 180,
        blue: 120,
    });
    write_test_le_u16(&mut brush, 6, 3);
    brush.extend_from_slice(b"WMF-CREATEBRUSH-UNSUPPORTED /RichMedia /OpenAction");
    brush.resize(brush.len().next_multiple_of(2), 0);
    let brush_len = (brush.len() / 2) as u32;
    write_test_le_u32(&mut brush, 0, brush_len);

    let records = [
        pen,
        brush,
        wmf_select_object_record(0),
        wmf_select_object_record(1),
        wmf_bounds_record(0x041b, 20, 10, 80, 50),
        wmf_yx_record(0x0214, 60, 20),
        wmf_yx_record(0x0213, 70, 80),
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
        .expect("unsupported WMF object-style vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            stroke_color: Some(Color {
                red: 0,
                green: 0,
                blue: 0
            }),
            stroke_width,
            stroke_style: BorderStyle::Single,
            fill_color: None,
            fill_pattern: ShadingPattern::None,
            ..
        } if (stroke_width - 4.172_978).abs() < f32::EPSILON
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Line {
            stroke_color: Some(Color {
                red: 0,
                green: 0,
                blue: 0
            }),
            stroke_width,
            stroke_style: BorderStyle::Single,
            ..
        } if (stroke_width - 4.172_978).abs() < f32::EPSILON
    ));
    for forbidden in [
        "wmetafile",
        "WMF-CREATEPEN-UNSUPPORTED",
        "WMF-CREATEBRUSH-UNSUPPORTED",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
        "RichMedia",
        "OpenAction",
    ] {
        assert!(
            !text.contains(forbidden),
            "unsupported WMF object-style payload/control leaked to normalized text: {forbidden}"
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
            .contains("2 unsupported record(s) skipped")
    }));
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
        "drawing after unsupported WMF objects should render a passive PDF rectangle"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-CREATEPEN-UNSUPPORTED",
        b"WMF-CREATEBRUSH-UNSUPPORTED",
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
            "unsupported WMF object-style payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_wmf_pattern_brushes_become_passive_placeholders() {
    for function in [0x01f9, 0x0142] {
        let records = [wmf_function_record(function)];
        let input = format!(
            "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
            bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
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
            .expect("malformed WMF pattern-brush placeholder");

        assert_eq!(image.format, ImageFormat::Placeholder);
        assert!(image.bytes.is_empty());
        assert!(image.vector_commands.is_empty());
    }
}

#[test]
fn invalid_wmf_modes_stay_partial_without_payload_leakage() {
    for (function, marker) in [
        (0x0102, "WMF-SETBKMODE-INVALID"),
        (0x0106, "WMF-SETPOLYFILLMODE-INVALID"),
    ] {
        let mut mode = wmf_u16_record(function, 99);
        mode.extend_from_slice(format!("{marker} /JavaScript /EmbeddedFile /Launch").as_bytes());
        mode.resize(mode.len().next_multiple_of(2), 0);
        let mode_len = (mode.len() / 2) as u32;
        write_test_le_u32(&mut mode, 0, mode_len);
        let records = [mode, wmf_bounds_record(0x041b, 20, 10, 80, 50)];
        let wmf = minimal_wmf_with_records(160, 80, &records);
        let wmf_hex = bytes_to_hex(&wmf);
        let input = format!(
            "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {wmf_hex}}} after\\par}}"
        )
        .into_bytes();
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
                .contains("1 unsupported record(s) skipped")
        }));
        for forbidden in [
            b"/Subtype /Image".as_slice(),
            b"wmetafile",
            wmf_hex.as_bytes(),
            marker.as_bytes(),
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
                "invalid WMF mode payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn malformed_wmf_modes_become_passive_placeholders() {
    for (function, name) in [(0x0102, "SETBKMODE"), (0x0106, "SETPOLYFILLMODE")] {
        let records = [wmf_function_record(function)];
        let input = format!(
            "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
            bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
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
            .expect("malformed WMF mode placeholder");

        assert!(text.contains("before"));
        assert!(text.contains("after"));
        assert_eq!(image.format, ImageFormat::Placeholder, "{name}");
        assert!(image.bytes.is_empty());
        assert!(image.vector_commands.is_empty());
    }
}

#[test]
fn malformed_wmf_scalar_modes_count_as_partial_without_payload_leakage() {
    for (function, label) in [
        (0x0103, "WMF-SETMAPMODE-MALFORMED"),
        (0x0104, "WMF-SETROP2-MALFORMED"),
        (0x0107, "WMF-SETSTRETCHBLTMODE-MALFORMED"),
        (0x0102, "WMF-SETBKMODE-MALFORMED"),
        (0x0106, "WMF-SETPOLYFILLMODE-MALFORMED"),
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
            .expect("malformed WMF scalar mode partial vector preview");

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
                "malformed WMF scalar mode payload/control leaked to normalized text: {forbidden}"
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
                "malformed WMF scalar mode payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn wmf_setrelabs_is_ignored_without_payload_leakage() {
    let mut mode = wmf_function_record(0x0105);
    mode.extend_from_slice(b"WMF-SETRELABS /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(2), 0);
    let mode_len = (mode.len() / 2) as u32;
    write_test_le_u32(&mut mode, 0, mode_len);
    let records = [mode, wmf_bounds_record(0x041b, 20, 10, 80, 50)];
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
        .expect("passive WMF SETRELABS vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "WMF-SETRELABS",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF SETRELABS payload/control leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after ignored WMF SETRELABS should render a passive PDF rectangle"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-SETRELABS",
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
            "WMF SETRELABS payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_setstretchbltmode_is_passive_state_without_payload_leakage() {
    let mut mode = wmf_u16_record(0x0107, 4);
    mode.extend_from_slice(b"WMF-SETSTRETCHBLTMODE /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(2), 0);
    let mode_len = (mode.len() / 2) as u32;
    write_test_le_u32(&mut mode, 0, mode_len);
    let records = [mode, wmf_bounds_record(0x041b, 20, 10, 80, 50)];
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
        .expect("passive WMF stretch mode vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "WMF-SETSTRETCHBLTMODE",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF SETSTRETCHBLTMODE payload/control leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after WMF SETSTRETCHBLTMODE should render a passive PDF rectangle"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-SETSTRETCHBLTMODE",
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
            "WMF SETSTRETCHBLTMODE payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn invalid_wmf_setstretchbltmode_stays_partial_without_payload_leakage() {
    let mut mode = wmf_u16_record(0x0107, 99);
    mode.extend_from_slice(b"WMF-SETSTRETCHBLTMODE-INVALID /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(2), 0);
    let mode_len = (mode.len() / 2) as u32;
    write_test_le_u32(&mut mode, 0, mode_len);
    let records = [mode, wmf_bounds_record(0x041b, 20, 10, 80, 50)];
    let wmf = minimal_wmf_with_records(160, 80, &records);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
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
            .contains("1 unsupported record(s) skipped")
    }));
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-SETSTRETCHBLTMODE-INVALID",
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
            "invalid WMF SETSTRETCHBLTMODE payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_wmf_setstretchbltmode_becomes_passive_placeholder() {
    let records = [wmf_function_record(0x0107)];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
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
        .expect("malformed WMF SETSTRETCHBLTMODE placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
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
                fill_pattern: ShadingPattern::WordWmfBitmapHorizontal,
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
    assert_eq!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "l")
            .count(),
        1,
        "Word's clipped legacy hatch should render as one passive PDF band"
    );
    assert!(
        content.operations.iter().any(|operation| {
            operation.operator == "w"
                && operation
                    .operands
                    .first()
                    .and_then(|operand| operand.as_f32().ok())
                    .is_some_and(|width| (width - 5.1428).abs() < 0.01)
        }),
        "Word's scaled legacy hatch should retain its recovered 5.14-point band"
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
fn word_and_strict_wmf_arc_family_render_passively_without_payload_leakage() {
    let wmf_hex = concat!(
        "0100090000033d0000000100070000000000",
        "050000000c026400c800",
        "07000000fc020000dcdcdc000000",
        "040000002d010000",
        "0b000000170846003c0028006400460064000a001400",
        "0b000000300846003c0028006400460064000a001400",
        "0b0000001a0846003c0028006400460064000a001400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let word_parsed = parse_rtf_bytes(&input).unwrap();
    let word_image = word_parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("Word-compatible WMF arc vector preview image");
    assert_eq!(word_image.vector_commands.len(), 3);
    assert!(matches!(
        &word_image.vector_commands[0],
        StaticImageVectorCommand::Polyline { points, .. }
            if points.len() > 40
                && points.first().is_some_and(|(x, y)| *x > 88.0 && *y > 40.0)
                && points.last().is_some_and(|(x, y)| *x > 58.0 && *x < 60.0 && *y > 63.0)
    ));

    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF arc vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 3);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::Polyline { points, .. }
            if points.first() == Some(&(100.0, 40.0))
                && points.last() == Some(&(60.0, 70.0))
                && points.len() <= 128
    ));
    assert!(matches!(
        &image.vector_commands[1],
        StaticImageVectorCommand::Polygon {
            points,
            fill_color: Some(Color { red: 220, green: 220, blue: 220 }),
            ..
        } if points.first() == Some(&(100.0, 40.0))
            && points.last() == Some(&(60.0, 70.0))
            && points.len() <= 128
    ));
    assert!(matches!(
        &image.vector_commands[2],
        StaticImageVectorCommand::Polygon {
            points,
            fill_color: Some(Color { red: 220, green: 220, blue: 220 }),
            ..
        } if points.first() == Some(&(60.0, 40.0))
            && points.get(1) == Some(&(100.0, 40.0))
            && points.last() == Some(&(60.0, 70.0))
            && points.len() <= 128
    ));
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in [
        "wmetafile",
        "010009",
        "1708",
        "3008",
        "1a08",
        "dcdcdc",
        "JavaScript",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF arc internals leaked to text: {forbidden}"
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
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "l")
            .count()
            >= 4,
        "WMF arc family should render as passive PDF path segments"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "f*" | "B" | "B*")),
        "WMF chord/pie records should render as passive filled paths"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"010009",
        b"1708",
        b"3008",
        b"1a08",
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
            "WMF arc preview leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn word_ignores_wmf_polybezier_while_strict_spec_renders_without_payload_leakage() {
    let wmf_hex = concat!(
        "0100090000032900000001000c0000000000",
        "050000000c026400c800",
        "08000000fa020000020002003c281400",
        "040000002d010000",
        "0c000000051004000a000a001e0005004600320050001400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let word_parsed = parse_rtf_bytes(&input).unwrap();
    let word_image = word_parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("blank Word-compatible WMF PolyBezier image");
    assert!(word_image.vector_commands.is_empty());

    let parsed = parse_rtf_bytes_with_options(
        &input,
        &RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        },
    )
    .unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF polybezier vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::Bezier {
            points,
            stroke_color: Some(Color { red: 60, green: 40, blue: 20 }),
            stroke_width: 2.086_489,
            ..
        } if points == &vec![(10.0, 10.0), (30.0, 5.0), (70.0, 50.0), (80.0, 20.0)]
    ));
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in [
        "wmetafile",
        "010009",
        "1005",
        "3c2814",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF polybezier internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            parse_options: RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
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
            .any(|operation| operation.operator == "c"),
        "WMF PolyBezier should render as a passive PDF cubic path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "S"),
        "WMF PolyBezier should stroke the passive cubic path"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"010009",
        b"1005",
        b"3c2814",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF polybezier preview leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn strict_spec_wmf_polybezierto_records_update_current_point_without_payload_leakage() {
    let wmf_hex = concat!(
        "0100090000033100000001000a0000000000",
        "050000000c026400c800",
        "08000000fa020000020002003c281400",
        "040000002d010000",
        "0500000014020a000a00",
        "0a000000041003001e0005004600320050001400",
        "0500000013023c005a00",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let word_parsed = parse_rtf_bytes(&input).unwrap();
    let word_image = word_parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("Word-compatible WMF polybezierto vector preview image");
    assert!(matches!(
        word_image.vector_commands.as_slice(),
        [StaticImageVectorCommand::Line {
            x1,
            y1,
            x2,
            y2,
            stroke_width,
            ..
        }] if (*x1 - 9.747_124).abs() < 0.001
            && (*y1 - 10.975_914).abs() < 0.001
            && (*x2 - 88.837_28).abs() < 0.001
            && (*y2 - 64.649_986).abs() < 0.001
            && (*stroke_width - 4.172_978).abs() < f32::EPSILON
    ));

    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF polybezierto vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::Bezier {
            points,
            stroke_color: Some(Color { red: 60, green: 40, blue: 20 }),
            stroke_width: 2.086_489,
            ..
        } if points == &vec![(10.0, 10.0), (30.0, 5.0), (70.0, 50.0), (80.0, 20.0)]
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Line {
            x1: 80.0,
            y1: 20.0,
            x2: 90.0,
            y2: 60.0,
            ..
        }
    ));
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in [
        "wmetafile",
        "010009",
        "1004",
        "3c2814",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF polybezierto internals leaked to text: {forbidden}"
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
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "c"),
        "WMF PolyBezierTo should render as a passive PDF cubic path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "l"),
        "WMF PolyBezierTo should update current point before following LineTo"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"010009",
        b"1004",
        b"3c2814",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF polybezierto preview leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_incomplete_bezier_records_are_noops_without_payload_leakage() {
    let mut polybezier = wmf_point_list_record(0x1005, &[(10, 10), (30, 5), (70, 50)]);
    polybezier.extend_from_slice(b"PARTIAL-WMF-POLYBEZIER /JavaScript");
    polybezier.resize(polybezier.len().next_multiple_of(2), 0);
    let polybezier_len_words = (polybezier.len() / 2) as u32;
    write_test_le_u32(&mut polybezier, 0, polybezier_len_words);

    let mut polybezierto = wmf_point_list_record(0x1004, &[(30, 5), (70, 50)]);
    polybezierto.extend_from_slice(b"PARTIAL-WMF-POLYBEZIERTO /EmbeddedFile");
    polybezierto.resize(polybezierto.len().next_multiple_of(2), 0);
    let polybezierto_len_words = (polybezierto.len() / 2) as u32;
    write_test_le_u32(&mut polybezierto, 0, polybezierto_len_words);

    let records = [
        polybezier,
        wmf_yx_record(0x0214, 20, 20),
        polybezierto,
        wmf_bounds_record(0x041b, 0, 0, 80, 40),
    ];
    let wmf = minimal_wmf_with_records(160, 80, &records);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal2160\\pichgoal1080 {wmf_hex}}} after\\par}}"
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
        .expect("passive WMF incomplete Bezier no-op image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in [
        "wmetafile",
        "PARTIAL-WMF-POLYBEZIER",
        "PARTIAL-WMF-POLYBEZIERTO",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "incomplete WMF Bezier payload/control leaked to text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "painting after incomplete WMF Bezier no-ops should still render"
    );
    for forbidden in [
        b"wmetafile".as_slice(),
        wmf_hex.as_bytes(),
        b"PARTIAL-WMF-POLYBEZIER",
        b"PARTIAL-WMF-POLYBEZIERTO",
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
            "incomplete WMF Bezier payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_incomplete_bezier_only_renders_blank_vector_without_payload_leakage() {
    let mut polybezier = wmf_point_list_record(0x1005, &[(10, 10), (30, 5), (70, 50)]);
    polybezier.extend_from_slice(b"BLANK-WMF-POLYBEZIER /JavaScript /EmbeddedFile /Launch");
    polybezier.resize(polybezier.len().next_multiple_of(2), 0);
    let polybezier_len_words = (polybezier.len() / 2) as u32;
    write_test_le_u32(&mut polybezier, 0, polybezier_len_words);

    let records = [polybezier];
    let wmf = minimal_wmf_with_records(160, 80, &records);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal2160\\pichgoal1080 {wmf_hex}}} after\\par}}"
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
        .expect("blank passive WMF vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in [
        "wmetafile",
        "BLANK-WMF-POLYBEZIER",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
        "Image skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "blank WMF vector payload/control leaked to text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(!rendered_text.contains("Image skipped"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "W"),
        "blank WMF vector should preserve the passive image box clip"
    );
    for forbidden in [
        b"wmetafile".as_slice(),
        wmf_hex.as_bytes(),
        b"BLANK-WMF-POLYBEZIER",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
        b"Image skipped",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "blank WMF vector payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn zero_count_wmf_poly_records_are_noops_without_payload_leakage() {
    let mut polygon = wmf_point_list_record(0x0324, &[]);
    polygon.extend_from_slice(b"ZERO-WMF-POLYGON /JavaScript /EmbeddedFile");
    polygon.resize(polygon.len().next_multiple_of(2), 0);
    let polygon_len_words = (polygon.len() / 2) as u32;
    write_test_le_u32(&mut polygon, 0, polygon_len_words);

    let mut polyline = wmf_point_list_record(0x0325, &[]);
    polyline.extend_from_slice(b"ZERO-WMF-POLYLINE /Launch /OpenAction");
    polyline.resize(polyline.len().next_multiple_of(2), 0);
    let polyline_len_words = (polyline.len() / 2) as u32;
    write_test_le_u32(&mut polyline, 0, polyline_len_words);

    let mut polybezier = wmf_point_list_record(0x1005, &[]);
    polybezier.extend_from_slice(b"ZERO-WMF-POLYBEZIER /RichMedia");
    polybezier.resize(polybezier.len().next_multiple_of(2), 0);
    let polybezier_len_words = (polybezier.len() / 2) as u32;
    write_test_le_u32(&mut polybezier, 0, polybezier_len_words);

    let mut polybezierto = wmf_point_list_record(0x1004, &[]);
    polybezierto.extend_from_slice(b"ZERO-WMF-POLYBEZIERTO /AcroForm");
    polybezierto.resize(polybezierto.len().next_multiple_of(2), 0);
    let polybezierto_len_words = (polybezierto.len() / 2) as u32;
    write_test_le_u32(&mut polybezierto, 0, polybezierto_len_words);

    let records = [
        polygon,
        polyline,
        polybezier,
        polybezierto,
        wmf_bounds_record(0x041b, 0, 0, 80, 40),
    ];
    let wmf = minimal_wmf_with_records(160, 80, &records);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal2160\\pichgoal1080 {wmf_hex}}} after\\par}}"
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
        .expect("passive zero-count WMF vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in [
        "wmetafile",
        "ZERO-WMF-POLYGON",
        "ZERO-WMF-POLYLINE",
        "ZERO-WMF-POLYBEZIER",
        "ZERO-WMF-POLYBEZIERTO",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
        "OpenAction",
        "RichMedia",
        "AcroForm",
    ] {
        assert!(
            !text.contains(forbidden),
            "zero-count WMF payload/control leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "painting after zero-count WMF poly no-ops should still render"
    );
    for forbidden in [
        b"wmetafile".as_slice(),
        wmf_hex.as_bytes(),
        b"ZERO-WMF-POLYGON",
        b"ZERO-WMF-POLYLINE",
        b"ZERO-WMF-POLYBEZIER",
        b"ZERO-WMF-POLYBEZIERTO",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
        b"/AcroForm",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "zero-count WMF payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn strict_spec_wmf_polypolygon_records_render_compound_path_without_payload_leakage() {
    let wmf_hex = concat!(
        "010009000003360000000100160000000000",
        "050000000c026400c800",
        "07000000fc020000dcdcdc000000",
        "040000002d010000",
        "0400000006010200",
        "160000003805020004000400",
        "0a000a008c000a008c0046000a004600",
        "32001900640019006400370032003700",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF polypolygon vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::Path {
            start: (10.0, 10.0),
            segments,
            fill_rule: StaticImageVectorFillRule::Winding,
            fill_color: Some(Color { red: 220, green: 220, blue: 220 }),
            ..
        } if segments.iter().any(|segment| matches!(
            segment,
            StaticImageVectorPathSegment::MoveTo(50.0, 25.0)
        ))
    ));
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in [
        "wmetafile",
        "010009",
        "0538",
        "dcdcdc",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF polypolygon internals leaked to text: {forbidden}"
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
            .filter(|operation| operation.operator == "m")
            .count()
            >= 2,
        "WMF PolyPolygon should render nested contours as one passive compound path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "B")),
        "WMF PolyPolygon should use winding fill operations for passive compound paths"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"010009",
        b"0538",
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
            "WMF polypolygon preview leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_cliprect_records_render_passively_without_payload_leakage() {
    let wmf_hex = concat!(
        "0100090000032a0000000100070000000000",
        "050000000c026400c800",
        "07000000fc020000dcdcdc000000",
        "040000002d010000",
        "0700000016043c00640014001400",
        "070000001b045000b4000a001400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF intersect-clipped vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::ClipRect {
            left: 20.0,
            top: 20.0,
            right: 100.0,
            bottom: 60.0
        }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in ["wmetafile", "010009", "1604", "dcdcdc", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "WMF clip rect internals leaked to text: {forbidden}"
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
        "WMF IntersectClipRect should render a passive PDF clipping path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "n"),
        "WMF IntersectClipRect should terminate the clipping path before painting"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"010009",
        b"1604",
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
            "WMF clip rect preview leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_empty_intersectcliprect_renders_empty_clip_without_payload_leakage() {
    let mut clip = wmf_bounds_record(0x0416, 20, 20, 20, 60);
    clip.extend_from_slice(b"EMPTY-WMF-INTERSECTCLIPRECT /JavaScript /EmbeddedFile");
    clip.resize(clip.len().next_multiple_of(2), 0);
    let clip_len_words = (clip.len() / 2) as u32;
    write_test_le_u32(&mut clip, 0, clip_len_words);

    let records = [clip, wmf_bounds_record(0x041b, 0, 0, 160, 80)];
    let wmf = minimal_wmf_with_records(160, 80, &records);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal2160\\pichgoal1080 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("passive WMF empty clip vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    assert_eq!(
        image.vector_commands[0],
        StaticImageVectorCommand::ClipRect {
            left: 20.0,
            top: 20.0,
            right: 20.0,
            bottom: 60.0,
        }
    );
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in [
        "wmetafile",
        "EMPTY-WMF-INTERSECTCLIPRECT",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "empty WMF IntersectClipRect payload/control leaked to text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "W"),
        "empty WMF IntersectClipRect should render a passive PDF clipping path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "empty WMF IntersectClipRect fixture should still emit passive rectangle paths"
    );
    for forbidden in [
        b"wmetafile".as_slice(),
        wmf_hex.as_bytes(),
        b"EMPTY-WMF-INTERSECTCLIPRECT",
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
            "empty WMF IntersectClipRect payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_offsetcliprgn_offsets_unpainted_clip_without_payload_leakage() {
    let wmf_hex = concat!(
        "0100090000032f0000000100070000000000",
        "050000000c026400c800",
        "07000000fc020000dcdcdc000000",
        "040000002d010000",
        "0700000016043c00640014001400",
        "05000000200205000a00",
        "070000001b045000b4000a001400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF offset-clipped vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert_eq!(
        image.vector_commands[0],
        StaticImageVectorCommand::ClipRect {
            left: 30.0,
            top: 25.0,
            right: 110.0,
            bottom: 65.0,
        }
    );
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in ["wmetafile", "010009", "2002", "dcdcdc", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "WMF offset clip rect internals leaked to text: {forbidden}"
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
        "WMF OffsetClipRgn should preserve passive PDF clipping"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "B")),
        "painting after WMF OffsetClipRgn should still render passively"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"010009",
        b"2002",
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
            "WMF offset clip preview leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_zero_offsetcliprgn_is_noop_without_payload_leakage() {
    let mut offset = wmf_yx_record(0x0220, 0, 0);
    offset.extend_from_slice(b"ZERO-OFFSET-WMF-OFFSETCLIPRGN /JavaScript /EmbeddedFile /Launch");
    offset.resize(offset.len().next_multiple_of(2), 0);
    let offset_len_words = (offset.len() / 2) as u32;
    write_test_le_u32(&mut offset, 0, offset_len_words);
    let records = [
        wmf_bounds_record(0x041b, 0, 0, 80, 40),
        offset,
        wmf_bounds_record(0x041b, 90, 10, 130, 50),
    ];
    let wmf = minimal_wmf_with_records(200, 100, &records);
    let wmf_hex = bytes_to_hex(&wmf);
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
        .expect("passive WMF zero-offset clip vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in [
        "wmetafile",
        "OFFSETCLIPRGN",
        "ZERO-OFFSET-WMF-OFFSETCLIPRGN",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "zero-offset WMF OffsetClipRgn payload/control leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "re")
            .count()
            >= 2,
        "painting before and after zero-offset WMF OffsetClipRgn should render"
    );
    for forbidden in [
        b"wmetafile".as_slice(),
        wmf_hex.as_bytes(),
        b"ZERO-OFFSET-WMF-OFFSETCLIPRGN",
        b"OFFSETCLIPRGN",
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
            "zero-offset WMF OffsetClipRgn payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_offsetcliprgn_offsets_saved_painted_clip_without_payload_leakage() {
    let wmf_hex = concat!(
        "010009000003390000000100070000000000",
        "050000000c026400c800",
        "07000000fc020000dcdcdc000000",
        "040000002d010000",
        "030000001e00",
        "0700000016043c00640014001400",
        "070000001b042800280000000000",
        "05000000200205000a00",
        "070000001b045000b4000a001400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF scoped offset-clipped vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(
        image
            .vector_commands
            .iter()
            .filter(|command| matches!(command, StaticImageVectorCommand::SaveState))
            .count(),
        2
    );
    assert_eq!(
        image
            .vector_commands
            .iter()
            .filter(|command| matches!(command, StaticImageVectorCommand::RestoreState))
            .count(),
        2
    );
    assert!(
        image.vector_commands.iter().any(|command| matches!(
            command,
            StaticImageVectorCommand::ClipRect {
                left: 30.0,
                top: 25.0,
                right: 110.0,
                bottom: 65.0,
            }
        )),
        "saved scoped WMF OffsetClipRgn should replay an offset passive clip"
    );
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in ["wmetafile", "001e", "2002", "dcdcdc", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "WMF scoped offset clip internals leaked to text: {forbidden}"
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
        "WMF scoped OffsetClipRgn should preserve passive PDF clipping"
    );
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "q")
            .count()
            >= 2,
        "WMF scoped OffsetClipRgn should use passive PDF graphics-state scopes"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "B")),
        "painting after scoped WMF OffsetClipRgn should still render passively"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"001e",
        b"2002",
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
            "WMF scoped offset clip preview leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_offsetcliprgn_offsets_unscoped_painted_clip_without_payload_leakage() {
    let wmf_hex = concat!(
        "010009000003360000000100070000000000",
        "050000000c026400c800",
        "07000000fc020000dcdcdc000000",
        "040000002d010000",
        "0700000016043c00640014001400",
        "070000001b042800280000000000",
        "05000000200205000a00",
        "070000001b045000b4000a001400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF unscoped offset-clipped vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(
        image
            .vector_commands
            .iter()
            .filter(|command| matches!(command, StaticImageVectorCommand::SaveState))
            .count(),
        2
    );
    assert_eq!(
        image
            .vector_commands
            .iter()
            .filter(|command| matches!(command, StaticImageVectorCommand::RestoreState))
            .count(),
        1
    );
    assert!(
        image.vector_commands.iter().any(|command| matches!(
            command,
            StaticImageVectorCommand::ClipRect {
                left: 30.0,
                top: 25.0,
                right: 110.0,
                bottom: 65.0,
            }
        )),
        "unscoped WMF OffsetClipRgn should replay an offset passive clip"
    );
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in ["wmetafile", "010009", "2002", "dcdcdc", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "WMF unscoped offset clip internals leaked to text: {forbidden}"
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
            .filter(|operation| operation.operator == "q")
            .count()
            >= 2,
        "WMF unscoped OffsetClipRgn should use passive PDF graphics-state scopes"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "Q"),
        "WMF unscoped OffsetClipRgn should restore the prior passive PDF clipping scope"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "W"),
        "WMF unscoped OffsetClipRgn should preserve passive PDF clipping"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "B")),
        "painting after unscoped WMF OffsetClipRgn should still render passively"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"010009",
        b"2002",
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
            "WMF unscoped offset clip preview leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_offsetcliprgn_offsets_mutated_painted_clip_without_payload_leakage() {
    let mut offset = wmf_yx_record(0x0220, 5, 10);
    offset.extend_from_slice(b"MUTATED-WMF-OFFSETCLIPRGN /JavaScript /EmbeddedFile /Launch");
    if offset.len() % 2 != 0 {
        offset.push(0);
    }
    let offset_words = (offset.len() / 2) as u32;
    write_test_le_u32(&mut offset, 0, offset_words);

    let records = [
        wmf_create_brush_record(Color {
            red: 220,
            green: 220,
            blue: 220,
        }),
        wmf_select_object_record(0),
        wmf_bounds_record(0x0416, 20, 20, 100, 60),
        wmf_bounds_record(0x041b, 0, 0, 30, 30),
        wmf_bounds_record(0x0416, 40, 30, 120, 70),
        offset,
        wmf_bounds_record(0x041b, 0, 0, 160, 80),
    ];
    let wmf = minimal_wmf_with_records(200, 100, &records);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF mutated offset-clipped vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(
        image.vector_commands.iter().any(|command| matches!(
            command,
            StaticImageVectorCommand::ClipRect {
                left: 50.0,
                top: 35.0,
                right: 130.0,
                bottom: 75.0,
            }
        )),
        "mutated WMF OffsetClipRgn should replay the offset current combined clip"
    );
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in [
        "wmetafile",
        "OFFSETCLIPRGN",
        "MUTATED-WMF-OFFSETCLIPRGN",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "mutated WMF OffsetClipRgn internals leaked to text: {forbidden}"
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
            .filter(|operation| operation.operator == "q")
            .count()
            >= 2,
        "mutated WMF OffsetClipRgn should use passive PDF graphics-state scopes"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "W"),
        "mutated WMF OffsetClipRgn should preserve passive PDF clipping"
    );
    for forbidden in [
        b"wmetafile".as_slice(),
        wmf_hex.as_bytes(),
        b"MUTATED-WMF-OFFSETCLIPRGN",
        b"OFFSETCLIPRGN",
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
            "mutated WMF OffsetClipRgn payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_excludecliprect_records_render_passively_without_payload_leakage() {
    let wmf_hex = concat!(
        "0100090000032a0000000100070000000000",
        "050000000c026400c800",
        "07000000fc020000dcdcdc000000",
        "040000002d010000",
        "0700000015043c00640014001400",
        "070000001b045000b4000a001400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF exclude-clipped vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::ClipPath {
            fill_rule: StaticImageVectorFillRule::Alternate,
            segments,
            ..
        } if segments.iter().any(|segment| matches!(
            segment,
            StaticImageVectorPathSegment::MoveTo(20.0, 20.0)
        ))
    ));
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in ["wmetafile", "010009", "1504", "dcdcdc", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "WMF exclude clip rect internals leaked to text: {forbidden}"
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
            .any(|operation| operation.operator == "W*"),
        "WMF ExcludeClipRect should render a passive PDF even-odd clipping path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "n"),
        "WMF ExcludeClipRect should terminate the clipping path before painting"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"010009",
        b"1504",
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
            "WMF exclude clip rect preview leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_empty_excludecliprect_is_noop_without_payload_leakage() {
    let mut clip = wmf_bounds_record(0x0415, 20, 20, 20, 60);
    clip.extend_from_slice(b"EMPTY-WMF-EXCLUDECLIPRECT /JavaScript /EmbeddedFile");
    clip.resize(clip.len().next_multiple_of(2), 0);
    let clip_len_words = (clip.len() / 2) as u32;
    write_test_le_u32(&mut clip, 0, clip_len_words);

    let records = [clip, wmf_bounds_record(0x041b, 0, 0, 160, 80)];
    let wmf = minimal_wmf_with_records(160, 80, &records);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal2160\\pichgoal1080 {wmf_hex}}} after\\par}}"
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
        .expect("passive WMF empty exclude no-op image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in [
        "wmetafile",
        "EMPTY-WMF-EXCLUDECLIPRECT",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "empty WMF ExcludeClipRect payload/control leaked to text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "painting after empty WMF ExcludeClipRect no-op should still render"
    );
    for forbidden in [
        b"wmetafile".as_slice(),
        wmf_hex.as_bytes(),
        b"EMPTY-WMF-EXCLUDECLIPRECT",
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
            "empty WMF ExcludeClipRect payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_excludecliprect_after_unpainted_clip_rect_renders_passively_without_payload_leakage() {
    let records = [
        wmf_bounds_record(0x0416, 10, 10, 120, 70),
        wmf_bounds_record(0x0415, 40, 20, 80, 50),
        wmf_bounds_record(0x041b, 0, 0, 160, 80),
    ];
    let wmf = minimal_wmf_with_records(200, 100, &records);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex} 2f4a617661536372697074 2f456d62656464656446696c65}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF exclude-clipped vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::ClipPath {
            start: (10.0, 10.0),
            fill_rule: StaticImageVectorFillRule::Alternate,
            segments,
            ..
        } if segments.iter().any(|segment| matches!(
            segment,
            StaticImageVectorPathSegment::MoveTo(40.0, 20.0)
        ))
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in ["wmetafile", "JavaScript", "EmbeddedFile", "1504", "1604"] {
        assert!(
            !text.contains(forbidden),
            "WMF active ExcludeClipRect internals leaked to text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "W*"),
        "WMF active ExcludeClipRect should render a passive even-odd clipping path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after WMF active ExcludeClipRect should still render"
    );
    for forbidden in [
        b"wmetafile".as_slice(),
        wmf_hex.as_bytes(),
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
            "WMF active ExcludeClipRect preview leaked forbidden PDF content: {:?}",
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
    let parsed = parse_rtf_bytes_strict(&input);
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
                && (*stroke_width - 12.518_934).abs() < 0.01
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
                && (*stroke_width - 4.172_978).abs() < 0.01
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
fn strict_spec_wmf_polypolygon_renders_compound_passive_path_without_payload_leakage() {
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
    let parsed = parse_rtf_bytes_strict(&input);
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
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::Path {
            start: (20.0, 20.0),
            segments,
            fill_rule: StaticImageVectorFillRule::Alternate,
            fill_color: Some(Color { red: 0, green: 255, blue: 255 }),
            ..
        } if segments.iter().any(|segment| matches!(
            segment,
            StaticImageVectorPathSegment::MoveTo(40.0, 70.0)
        ))
    ));
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
            .filter(|operation| operation.operator == "m")
            .count()
            >= 2,
        "WMF POLYPOLYGON should render each contour as a passive compound subpath"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "B" | "B*")),
        "WMF POLYPOLYGON should fill and stroke the passive compound path"
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
fn word_wmf_patblt_patcopy_renders_recovered_brush_rectangle_without_payload_leakage() {
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
            } if (*left - 49.205_02).abs() < 0.01
                && (*top - 31.972_137).abs() < 0.01
                && (*right - 88.886_7).abs() < 0.01
                && (*bottom - 53.740_94).abs() < 0.01
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
fn strict_spec_wmf_patcopy_stretchblt_renders_passive_brush_rectangle() {
    let wmf_hex = concat!(
        "010009000003400000000200170000000000",
        "050000000c026400c800",
        "07000000fc020000285078000000",
        "040000002d010000",
        "0d000000230b2100f00000000000000000002800500019000f00",
        "17000000230b2000cc00000000000000000019002d001e0023005241572d535243434f50592d5041594c4f414400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF STRETCHBLT vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 15.0,
            top: 25.0,
            right: 95.0,
            bottom: 65.0,
            stroke_color: None,
            fill_color: Some(Color {
                red: 40,
                green: 80,
                blue: 120
            }),
            ..
        }
    ));
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in [
        "wmetafile",
        "0b23",
        "RAW-SRCCOPY-PAYLOAD",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF STRETCHBLT internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            parse_options: RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
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
        "WMF PATCOPY STRETCHBLT should render as passive PDF rectangle path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "f*" | "B" | "B*")),
        "WMF PATCOPY STRETCHBLT should render as a passive PDF fill"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"0b23",
        b"RAW-SRCCOPY-PAYLOAD",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF STRETCHBLT leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn strict_spec_wmf_patcopy_bitblt_renders_passive_brush_rectangle() {
    let wmf_hex = concat!(
        "0100090000033c0000000200150000000000",
        "050000000c026400c800",
        "07000000fc0200001e6eb4000000",
        "040000002d010000",
        "0b00000022092100f000000000003c00460014000a00",
        "1500000022092000cc00000000000019002d001e0023005241572d535243434f50592d424954424c5400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF BITBLT vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 10.0,
            top: 20.0,
            right: 80.0,
            bottom: 80.0,
            stroke_color: None,
            fill_color: Some(Color {
                red: 30,
                green: 110,
                blue: 180
            }),
            ..
        }
    ));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("1 unsupported record(s) skipped")
    }));
    for forbidden in [
        "wmetafile",
        "0922",
        "RAW-SRCCOPY-BITBLT",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF BITBLT internals leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            parse_options: RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
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
        "WMF PATCOPY BITBLT should render as passive PDF rectangle path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "f*" | "B" | "B*")),
        "WMF PATCOPY BITBLT should render as a passive PDF fill"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"0922",
        b"RAW-SRCCOPY-BITBLT",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF BITBLT leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_blackness_and_whiteness_transfers_render_passively_without_payload_leakage() {
    let wmf_hex = concat!(
        "010009000003900000000400190000000000",
        "050000000c026400c800",
        "07000000fc020000ff0000000000",
        "040000002d010000",
        "160000002209420000000000000019002d001e0023005241572d424c41434b4e4553532d5041594c4f414400",
        "18000000230b6200ff0000000000000000002800500019000f005241572d57484954454e4553532d5041594c4f414400",
        "190000004009420000000000000019002d001e00230000005241572d4449422d424c41434b4e4553532d5041594c4f414400",
        "18000000430f6200ff000000000000000000000019002d001e0023005241572d535452455443484449422d5748495445",
        "1500000022092000cc00000000000019002d001e0023005241572d535243434f50592d424954424c5400",
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
        .expect("WMF solid raster transfer vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 4);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 35.0,
            top: 30.0,
            right: 80.0,
            bottom: 55.0,
            stroke_color: None,
            fill_color: Some(Color {
                red: 0,
                green: 0,
                blue: 0
            }),
            ..
        }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Rectangle {
            left: 15.0,
            top: 25.0,
            right: 95.0,
            bottom: 65.0,
            stroke_color: None,
            fill_color: Some(Color {
                red: 255,
                green: 255,
                blue: 255
            }),
            ..
        }
    ));
    assert!(matches!(
        image.vector_commands[2],
        StaticImageVectorCommand::Rectangle {
            left: 35.0,
            top: 30.0,
            right: 80.0,
            bottom: 55.0,
            stroke_color: None,
            fill_color: Some(Color {
                red: 0,
                green: 0,
                blue: 0
            }),
            ..
        }
    ));
    assert!(matches!(
        image.vector_commands[3],
        StaticImageVectorCommand::Rectangle {
            left: 35.0,
            top: 30.0,
            right: 80.0,
            bottom: 55.0,
            stroke_color: None,
            fill_color: Some(Color {
                red: 255,
                green: 255,
                blue: 255
            }),
            ..
        }
    ));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("1 unsupported record(s) skipped")
    }));
    for forbidden in [
        "wmetafile",
        "RAW-BLACKNESS-PAYLOAD",
        "RAW-WHITENESS-PAYLOAD",
        "RAW-DIB-BLACKNESS-PAYLOAD",
        "RAW-STRETCHDIB-WHITE",
        "RAW-SRCCOPY-BITBLT",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF solid raster transfer internals leaked to text: {forbidden}"
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
            .filter(|operation| operation.operator == "re")
            .count()
            >= 4,
        "WMF BLACKNESS/WHITENESS transfers should render passive PDF rectangles"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "f*" | "B" | "B*")),
        "WMF BLACKNESS/WHITENESS transfers should render passive PDF fills"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"RAW-BLACKNESS-PAYLOAD",
        b"RAW-WHITENESS-PAYLOAD",
        b"RAW-DIB-BLACKNESS-PAYLOAD",
        b"RAW-STRETCHDIB-WHITE",
        b"RAW-SRCCOPY-BITBLT",
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
            "WMF solid raster transfer leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_setpixel_renders_word_sized_passive_stroke_without_payload_leakage() {
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
            StaticImageVectorCommand::SquareCappedLine {
                x1,
                y1,
                x2,
                y2,
                stroke_color: Some(color),
                stroke_width,
                ..
            } if (*x1 - 71.2882).abs() < 0.01
                && (*y1 - 41.581).abs() < 0.01
                && (*x2 - 72.8765).abs() < 0.01
                && (*y2 - 41.581).abs() < 0.01
                && (*stroke_width - 4.172978).abs() < 0.0001
                && color.red == 255
                && color.green == 0
                && color.blue == 0
        )
    }));
    let strict = parse_rtf_bytes_strict(&input);
    let strict_image = strict
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("strict-spec WMF SETPIXEL vector preview image");
    assert!(matches!(
        strict_image.vector_commands.as_slice(),
        [StaticImageVectorCommand::Line { x1, y1, x2, y2, .. }]
            if (*x1 - 64.0).abs() < 0.01
                && (*y1 - 32.0).abs() < 0.01
                && (*x2 - 65.5883).abs() < 0.01
                && (*y2 - 32.0).abs() < 0.01
    ));
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
            .any(|operation| operation.operator == "l"),
        "WMF SETPIXEL should render as a passive PDF line segment"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "S"),
        "WMF SETPIXEL should render as a passive PDF stroke"
    );
    assert!(
        content.operations.iter().any(|operation| {
            operation.operator == "J" && operation.operands.as_slice() == [PdfObject::Integer(2)]
        }),
        "Word-compatible WMF SETPIXEL should use a projecting-square PDF line cap"
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
fn malformed_wmf_setpixel_becomes_passive_placeholder() {
    let records = [wmf_u32_record(0x041f, 0x0000_00ff)];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
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
        .expect("malformed WMF SETPIXEL placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
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
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "q"),
        "WMF SaveDC should emit a passive PDF graphics-state save"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "Q"),
        "WMF RestoreDC should emit a passive PDF graphics-state restore"
    );
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
fn wmf_saved_dc_restores_passive_clip_scope_without_record_leakage() {
    let wmf_hex = concat!(
        "010009000003380000000100070000000000",
        "050000000c026400c800",
        "07000000fc020000dcdcdc000000",
        "040000002d010000",
        "030000001e00",
        "0700000016043c00640014001400",
        "070000001b045000b4000a001400",
        "040000002701ffff",
        "070000001b045000b40078001400",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("WMF SaveDC/RestoreDC clipped vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 5);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::SaveState
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::ClipRect { .. }
    ));
    assert!(matches!(
        image.vector_commands[3],
        StaticImageVectorCommand::RestoreState
    ));
    assert!(matches!(
        image.vector_commands[4],
        StaticImageVectorCommand::Rectangle {
            left: 20.0,
            top: 80.0,
            right: 180.0,
            bottom: 100.0,
            ..
        }
    ));
    for forbidden in ["wmetafile", "001e", "0127", "1604", "dcdcdc", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "WMF SaveDC clip internals leaked to text: {forbidden}"
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
        "WMF clipped SaveDC scope should emit passive clipping"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "q"),
        "WMF SaveDC clip scope should save PDF graphics state"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "Q"),
        "WMF RestoreDC clip scope should restore PDF graphics state"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"001e",
        b"0127",
        b"1604",
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
            "WMF SaveDC clip leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_saved_dc_state_limit_becomes_placeholder_without_record_leakage() {
    let mut wmf_hex = String::from("0100090000036f0000000000030000000000");
    for _ in 0..33 {
        wmf_hex.push_str("030000001e00");
    }
    wmf_hex.push_str("030000000000");
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
        .expect("over-limit WMF SaveDC placeholder image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    assert!(image.vector_commands.is_empty());
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("unsupported picture format replaced with a passive geometry placeholder")
    }));
    assert!(!parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("WMF picture rendered as bounded passive vector preview")
    }));
    for forbidden in ["wmetafile", "001e", "6f000000", "010009", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "over-limit WMF SaveDC internals leaked to text: {forbidden}"
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
        rendered_text.contains("Image skipped"),
        "over-limit WMF should render a passive placeholder label"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"001e",
        b"6f000000",
        b"010009",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "over-limit WMF SaveDC leaked forbidden PDF content: {:?}",
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
fn wmf_settextjustification_renders_passive_word_spacing_without_record_leakage() {
    let mut justification = wmf_i16_pair_record(0x020a, 2, 40);
    justification.extend_from_slice(b"WMF-SETTEXTJUSTIFICATION /JavaScript /EmbeddedFile /Launch");
    justification.resize(justification.len().next_multiple_of(2), 0);
    let justification_len = (justification.len() / 2) as u32;
    write_test_le_u32(&mut justification, 0, justification_len);
    let records = [
        justification,
        wmf_exttextout_record(40, 20, "A B C", 0, None),
    ];
    let wmf = minimal_wmf_with_records(200, 100, &records);
    let wmf_hex = bytes_to_hex(&wmf);
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
        .expect("WMF SETTEXTJUSTIFICATION vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("A B C"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::Text {
                text,
                word_extra,
                character_extra: 0.0,
                ..
            } if text == "A B C" && (*word_extra - 20.0).abs() < 0.01
        )
    }));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "WMF-SETTEXTJUSTIFICATION",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF SETTEXTJUSTIFICATION internals leaked to text: {forbidden}"
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
        decoded_pdf_text(&content).contains("A B C"),
        "WMF SETTEXTJUSTIFICATION should still render passive PDF text"
    );
    assert!(
        content.operations.iter().any(|operation| {
            operation.operator == "Tw"
                && operation
                    .operands
                    .first()
                    .and_then(pdf_operand_number)
                    .is_some_and(|value| value > 1.0)
        }),
        "WMF SETTEXTJUSTIFICATION should emit nonzero passive PDF word spacing"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-SETTEXTJUSTIFICATION",
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
            "WMF SETTEXTJUSTIFICATION leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_wmf_textout_counts_as_partial_without_payload_leakage() {
    let mut malformed_text = wmf_textout_record(40, 20, b"Hidden");
    write_test_le_u16(&mut malformed_text, 6, 513);
    malformed_text.extend_from_slice(b"WMF-TEXTOUT-MALFORMED /JavaScript /EmbeddedFile /Launch");
    malformed_text.resize(malformed_text.len().next_multiple_of(2), 0);
    let text_len = (malformed_text.len() / 2) as u32;
    write_test_le_u32(&mut malformed_text, 0, text_len);

    let records = [malformed_text, wmf_bounds_record(0x041b, 20, 10, 80, 50)];
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
        .expect("malformed WMF TEXTOUT partial vector preview");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("Hidden"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(
        image
            .vector_commands
            .iter()
            .any(|command| matches!(command, StaticImageVectorCommand::Rectangle { .. }))
    );
    assert!(image.vector_commands.iter().all(|command| !matches!(
        command,
        StaticImageVectorCommand::Text { text, .. } if text.contains("Hidden")
    )));
    for forbidden in [
        "wmetafile",
        "WMF-TEXTOUT-MALFORMED",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "malformed WMF TEXTOUT payload/control leaked to normalized text: {forbidden}"
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
            .contains("1 unsupported record(s) skipped")
    }));
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(!rendered_text.contains("Hidden"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "safe drawing after malformed WMF TEXTOUT should still render passively"
    );
    for forbidden in [
        b"wmetafile".as_slice(),
        wmf_hex.as_bytes(),
        b"WMF-TEXTOUT-MALFORMED",
        b"Hidden",
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
            "malformed WMF TEXTOUT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_wmf_text_spacing_state_counts_as_partial_without_payload_leakage() {
    for (function, label) in [
        (0x0108, "WMF-SETTEXTCHAREXTRA-MALFORMED"),
        (0x020a, "WMF-SETTEXTJUSTIFICATION-MALFORMED"),
    ] {
        let malformed_state = wmf_function_record(function);
        let records = [malformed_state, wmf_bounds_record(0x041b, 20, 10, 80, 50)];
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
            .expect("malformed WMF text spacing state partial vector preview");

        assert!(text.contains("before"));
        assert!(text.contains("after"));
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
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
                "malformed WMF text spacing payload/control leaked to normalized text: {forbidden}"
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
                "malformed WMF text spacing payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn malformed_wmf_settextjustification_becomes_passive_placeholder() {
    let records = [wmf_function_record(0x020a)];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
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
        .expect("malformed WMF SETTEXTJUSTIFICATION placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn malformed_wmf_settextcolor_becomes_passive_placeholder() {
    let records = [wmf_function_record(0x0209)];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
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
        .expect("malformed WMF SETTEXTCOLOR placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn malformed_wmf_setbkcolor_becomes_passive_placeholder() {
    let records = [wmf_function_record(0x0201)];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
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
        .expect("malformed WMF SETBKCOLOR placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn malformed_wmf_color_state_counts_as_partial_without_payload_leakage() {
    for (function, label) in [
        (0x0209, "WMF-SETTEXTCOLOR-MALFORMED"),
        (0x0201, "WMF-SETBKCOLOR-MALFORMED"),
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
            .expect("malformed WMF color state partial vector preview");

        assert!(text.contains("before"));
        assert!(text.contains("after"));
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
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
                "malformed WMF color payload/control leaked to normalized text: {forbidden}"
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
                "malformed WMF color payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
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
fn wmf_exttextout_dx_records_render_positioned_passive_text_without_payload_leakage() {
    let text_record = wmf_exttextout_dx_record(40, 20, "ABC", 0, None, &[10, 20, 30]);
    let wmf = minimal_wmf_with_records(160, 80, &[text_record]);
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
        .expect("WMF EXTTEXTOUT Dx vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("ABC"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    let text_commands: Vec<_> = image
        .vector_commands
        .iter()
        .filter_map(|command| match command {
            StaticImageVectorCommand::Text { x, y, text, .. } => Some((*x, *y, text.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(
        text_commands,
        vec![(40.0, 20.0, "A"), (50.0, 20.0, "B"), (70.0, 20.0, "C")]
    );
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in ["wmetafile", "0a32", "JavaScript", "EmbeddedFile", "Launch"] {
        assert!(
            !text.contains(forbidden),
            "WMF EXTTEXTOUT Dx internals leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(rendered_text.contains("ABC"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "Tj")
            .count()
            >= 3,
        "WMF EXTTEXTOUT Dx should render through positioned passive text operations"
    );
    for forbidden in [
        b"wmetafile".as_slice(),
        wmf_hex.as_bytes(),
        b"0a32",
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
            "WMF EXTTEXTOUT Dx payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_exttextout_pdy_records_render_positioned_passive_text_without_payload_leakage() {
    let text_record =
        wmf_exttextout_pdy_record(40, 20, "ABC", 0x2000, None, &[(10, 5), (20, -3), (30, 0)]);
    let wmf = minimal_wmf_with_records(160, 80, &[text_record]);
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
        .expect("WMF EXTTEXTOUT PDY vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("ABC"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.palette.is_empty());
    let text_commands: Vec<_> = image
        .vector_commands
        .iter()
        .filter_map(|command| match command {
            StaticImageVectorCommand::Text { x, y, text, .. } => Some((*x, *y, text.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(
        text_commands,
        vec![(40.0, 20.0, "A"), (50.0, 25.0, "B"), (70.0, 22.0, "C")]
    );
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in ["wmetafile", "0a32", "JavaScript", "EmbeddedFile", "Launch"] {
        assert!(
            !text.contains(forbidden),
            "WMF EXTTEXTOUT PDY internals leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(rendered_text.contains("ABC"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "Tj")
            .count()
            >= 3,
        "WMF EXTTEXTOUT PDY should render through positioned passive text operations"
    );
    for forbidden in [
        b"wmetafile".as_slice(),
        wmf_hex.as_bytes(),
        b"0a32",
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
            "WMF EXTTEXTOUT PDY payload leaked to PDF: {:?}",
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
fn unsupported_wmf_exttextout_options_stay_partial_without_payload_leakage() {
    let mut text_record = wmf_exttextout_record(40, 20, "Hidden", 0x4000, None);
    text_record.extend_from_slice(b"WMF-EXTTEXTOUT-UNSUPPORTED /JavaScript /EmbeddedFile /Launch");
    text_record.resize(text_record.len().next_multiple_of(2), 0);
    let text_words = (text_record.len() / 2) as u32;
    write_test_le_u32(&mut text_record, 0, text_words);

    let wmf = minimal_wmf_with_records(160, 80, &[text_record]);
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
        .expect("unsupported WMF EXTTEXTOUT option placeholder");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
    for forbidden in [
        "wmetafile",
        "WMF-EXTTEXTOUT-UNSUPPORTED",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "unsupported WMF EXTTEXTOUT payload/control leaked to normalized text: {forbidden}"
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
            .contains("unsupported picture format replaced with a passive geometry placeholder")
    }));
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(!rendered_text.contains("Hidden"));
    for forbidden in [
        b"wmetafile".as_slice(),
        wmf_hex.as_bytes(),
        b"WMF-EXTTEXTOUT-UNSUPPORTED",
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
            "unsupported WMF EXTTEXTOUT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn empty_wmf_exttextout_opaque_bounds_render_passive_background_without_payload_leakage() {
    let mut empty_text = wmf_exttextout_record(40, 20, "", 0x0002, Some((30, 10, 90, 30)));
    empty_text.extend_from_slice(b"EMPTY-WMF-EXTTEXTOUT /JavaScript /EmbeddedFile /Launch");
    empty_text.resize(empty_text.len().next_multiple_of(2), 0);
    let empty_text_words = (empty_text.len() / 2) as u32;
    write_test_le_u32(&mut empty_text, 0, empty_text_words);

    let records = [empty_text, wmf_bounds_record(0x041b, 0, 0, 80, 40)];
    let wmf = minimal_wmf_with_records(160, 80, &records);
    let wmf_hex = bytes_to_hex(&wmf);
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
        .expect("passive empty WMF opaque text vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 30.0,
            top: 10.0,
            right: 90.0,
            bottom: 30.0,
            stroke_color: None,
            fill_color: Some(Color {
                red: 255,
                green: 255,
                blue: 255
            }),
            ..
        }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "EMPTY-WMF-EXTTEXTOUT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "empty WMF opaque text payload/control leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(!rendered_text.contains("EMPTY-WMF-EXTTEXTOUT"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "re")
            .count()
            >= 2,
        "empty WMF opaque text bounds and later rectangle should render passive PDF rectangles"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "f*")),
        "empty WMF opaque text bounds should render a passive PDF fill"
    );
    for forbidden in [
        b"wmetafile".as_slice(),
        wmf_hex.as_bytes(),
        b"EMPTY-WMF-EXTTEXTOUT",
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
            "empty WMF opaque text payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn sanitized_empty_wmf_exttextout_opaque_bounds_render_passive_background_without_payload_leakage()
{
    let mut empty_text = wmf_exttextout_record(40, 20, "\0\0", 0x0002, Some((30, 10, 90, 30)));
    empty_text
        .extend_from_slice(b"SANITIZED-EMPTY-WMF-EXTTEXTOUT /JavaScript /EmbeddedFile /Launch");
    empty_text.resize(empty_text.len().next_multiple_of(2), 0);
    let empty_text_words = (empty_text.len() / 2) as u32;
    write_test_le_u32(&mut empty_text, 0, empty_text_words);

    let records = [empty_text, wmf_bounds_record(0x041b, 0, 0, 80, 40)];
    let wmf = minimal_wmf_with_records(160, 80, &records);
    let wmf_hex = bytes_to_hex(&wmf);
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
        .expect("passive sanitized-empty WMF opaque text vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 30.0,
            top: 10.0,
            right: 90.0,
            bottom: 30.0,
            stroke_color: None,
            fill_color: Some(Color {
                red: 255,
                green: 255,
                blue: 255
            }),
            ..
        }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "SANITIZED-EMPTY-WMF-EXTTEXTOUT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "sanitized-empty WMF opaque text payload/control leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(!rendered_text.contains("SANITIZED-EMPTY-WMF-EXTTEXTOUT"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "re")
            .count()
            >= 2,
        "sanitized-empty WMF opaque text bounds and later rectangle should render passive PDF rectangles"
    );
    for forbidden in [
        b"wmetafile".as_slice(),
        wmf_hex.as_bytes(),
        b"SANITIZED-EMPTY-WMF-EXTTEXTOUT",
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
            "sanitized-empty WMF opaque text payload leaked to PDF: {:?}",
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
        (155.0..161.0).contains(&x),
        "center alignment should shift text left of its inline image-space anchor, got {x}"
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
fn invalid_wmf_settextalign_stays_partial_without_payload_leakage() {
    let mut mode = wmf_u16_record(0x012e, 0x0100);
    mode.extend_from_slice(b"WMF-SETTEXTALIGN-INVALID /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(2), 0);
    let mode_len = (mode.len() / 2) as u32;
    write_test_le_u32(&mut mode, 0, mode_len);
    let records = [
        mode,
        wmf_exttextout_record(40, 20, "Hi", 0, None),
        wmf_bounds_record(0x041b, 20, 10, 80, 50),
    ];
    let wmf = minimal_wmf_with_records(160, 80, &records);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
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
            .contains("1 unsupported record(s) skipped")
    }));
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Hi"));
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-SETTEXTALIGN-INVALID",
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
            "invalid WMF SETTEXTALIGN payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_wmf_settextalign_keeps_later_safe_text() {
    let records = [
        wmf_function_record(0x012e),
        wmf_exttextout_record(40, 20, "Hi", 0, None),
    ];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
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
        .expect("malformed WMF SETTEXTALIGN partial vector preview");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(
        image.vector_commands.iter().any(
            |command| matches!(command, StaticImageVectorCommand::Text { text, .. } if text == "Hi")
        ),
        "safe text after malformed WMF SETTEXTALIGN should stay visible"
    );
    assert!(!text.contains("Hi"));
}

#[test]
fn strict_spec_wmf_roundrect_renders_passive_rounded_rectangle_without_payload_leakage() {
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
    let parsed = parse_rtf_bytes_strict(&input);
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
