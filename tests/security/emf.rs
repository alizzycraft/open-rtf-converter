use super::*;

#[test]
fn emf_header_geometry_renders_passive_placeholder_without_payload_leakage() {
    let emf = minimal_emf_with_bounds_and_frame(160, 80, 2540, 1270);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF placeholder image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert_eq!(image.width_px, 160);
    assert_eq!(image.height_px, 80);
    assert_eq!(image.natural_width_px_hint, Some(160));
    assert_eq!(image.natural_height_px_hint, Some(80));
    assert_eq!(image.display_width_twips, Some(1440));
    assert_eq!(image.display_height_twips, Some(720));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("EMF picture records stripped; header dimensions used")
    }));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF payload/control leaked to normalized text: {forbidden}"
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
            .any(|operation| operation.operator == "S"),
        "EMF placeholder should render a passive stroked frame"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_simple_vector_records_render_passively_without_payload_leakage() {
    let records = [
        emf_rect_record(43, 10, 10, 70, 40),
        emf_rect_record(42, 80, 20, 150, 70),
        emf_unknown_record(999),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.width_px, 160);
    assert_eq!(image.height_px, 80);
    assert_eq!(image.display_width_twips, Some(1440));
    assert_eq!(image.display_height_twips, Some(720));
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 10.0,
            top: 10.0,
            right: 70.0,
            bottom: 40.0,
            stroke_color: Some(_),
            stroke_width: 1.0,
            stroke_style: BorderStyle::Single,
            fill_pattern: ShadingPattern::None,
            fill_color: None,
        }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Ellipse {
            left: 80.0,
            top: 20.0,
            right: 150.0,
            bottom: 70.0,
            stroke_color: Some(_),
            stroke_width: 1.0,
            stroke_style: BorderStyle::Single,
            fill_pattern: ShadingPattern::None,
            fill_color: None,
        }
    ));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("EMF picture rendered as bounded passive vector preview with 1 unsupported record(s) skipped")
    }));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF payload/control leaked to normalized text: {forbidden}"
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
            .any(|operation| matches!(operation.operator.as_str(), "S" | "s")),
        "EMF vector preview should render passive stroked paths"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_roundrect_records_render_passively_without_payload_leakage() {
    let records = [emf_roundrect_record(10, 10, 70, 40, 90, 80)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF roundrect vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::RoundedRectangle {
            left: 10.0,
            top: 10.0,
            right: 70.0,
            bottom: 40.0,
            corner_width: 60.0,
            corner_height: 30.0,
            stroke_color: Some(_),
            stroke_width: 1.0,
            stroke_style: BorderStyle::Single,
            fill_pattern: ShadingPattern::None,
            fill_color: None,
        }
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF roundrect payload/control leaked to normalized text: {forbidden}"
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
            .any(|operation| operation.operator == "c"),
        "EMF roundrect vector preview should render passive Bezier curve operations"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "S" | "s")),
        "EMF roundrect vector preview should render passive stroked paths"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF roundrect payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_pen_and_brush_records_render_passively_without_payload_leakage() {
    let records = [
        emf_create_pen_record(
            2,
            1,
            3,
            Color {
                red: 20,
                green: 40,
                blue: 60,
            },
        ),
        emf_select_object_record(2),
        emf_create_brush_record(
            3,
            0,
            Color {
                red: 220,
                green: 180,
                blue: 120,
            },
            0,
        ),
        emf_select_object_record(3),
        emf_rect_record(43, 10, 10, 70, 40),
        emf_point_record(27, 10, 50),
        emf_point_record(54, 70, 70),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive styled EMF vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            stroke_color: Some(Color {
                red: 20,
                green: 40,
                blue: 60
            }),
            stroke_width: 3.0,
            stroke_style: BorderStyle::Dashed,
            fill_pattern: ShadingPattern::None,
            fill_color: Some(Color {
                red: 220,
                green: 180,
                blue: 120
            }),
            ..
        }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Line {
            stroke_color: Some(Color {
                red: 20,
                green: 40,
                blue: 60
            }),
            stroke_width: 3.0,
            stroke_style: BorderStyle::Dashed,
            ..
        }
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF styled object payload/control leaked to normalized text: {forbidden}"
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
            .any(|operation| operation.operator == "RG"),
        "EMF pen color should render as passive PDF stroke color"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "rg"),
        "EMF brush color should render as passive PDF fill color"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "S" | "s" | "B" | "B*")),
        "styled EMF vector preview should render passive painted paths"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF styled object payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn unsupported_emf_brush_style_is_inert_without_payload_leakage() {
    let mut brush = emf_create_brush_record(
        3,
        3,
        Color {
            red: 220,
            green: 180,
            blue: 120,
        },
        0,
    );
    brush.extend_from_slice(b"EMF-CREATEBRUSH-UNSUPPORTED /JavaScript /EmbeddedFile /Launch");
    brush.resize(brush.len().next_multiple_of(4), 0);
    let brush_len = brush.len() as u32;
    write_test_le_u32(&mut brush, 4, brush_len);
    let records = [
        brush,
        emf_select_object_record(3),
        emf_rect_record(43, 10, 10, 70, 40),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("unsupported EMF brush-style vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            fill_color: None,
            fill_pattern: ShadingPattern::None,
            ..
        }
    ));
    for forbidden in [
        "emfblip",
        "EMF-CREATEBRUSH-UNSUPPORTED",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "unsupported EMF brush payload/control leaked to normalized text: {forbidden}"
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
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after unsupported EMF brush should render a passive PDF rectangle"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-CREATEBRUSH-UNSUPPORTED",
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
            "unsupported EMF brush payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_palette_records_are_passive_noops_without_payload_leakage() {
    let mut palette = emf_create_palette_record(4, &[(255, 0, 0), (0, 255, 0)]);
    palette.extend_from_slice(b"EMF-CREATEPALETTE /JavaScript /EmbeddedFile /OpenAction");
    palette.resize(palette.len().next_multiple_of(4), 0);
    let palette_len = palette.len() as u32;
    write_test_le_u32(&mut palette, 4, palette_len);

    let mut select = emf_u32_record(48, 4);
    select.extend_from_slice(b"EMF-SELECTPALETTE /Launch /RichMedia");
    select.resize(select.len().next_multiple_of(4), 0);
    let select_len = select.len() as u32;
    write_test_le_u32(&mut select, 4, select_len);

    let mut realize = emf_unknown_record(52);
    realize.extend_from_slice(b"EMF-REALIZEPALETTE /JavaScript /EmbeddedFile");
    realize.resize(realize.len().next_multiple_of(4), 0);
    let realize_len = realize.len() as u32;
    write_test_le_u32(&mut realize, 4, realize_len);

    let records = [
        palette,
        select,
        realize,
        emf_rect_record(43, 10, 10, 70, 40),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF palette no-op vector image");

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
        "emfblip",
        "EMF-CREATEPALETTE",
        "EMF-SELECTPALETTE",
        "EMF-REALIZEPALETTE",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
        "RichMedia",
        "OpenAction",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF palette payload/control leaked to normalized text: {forbidden}"
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
        "drawing after EMF palette no-ops should render a passive PDF rectangle"
    );
    assert!(output.diagnostics.is_empty());
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-CREATEPALETTE",
        b"EMF-SELECTPALETTE",
        b"EMF-REALIZEPALETTE",
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
            "EMF palette payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_emf_createpalette_becomes_passive_placeholder() {
    let mut palette = emf_create_palette_record(4, &[(255, 0, 0)]);
    write_test_le_u16(&mut palette, 14, 2);
    let records = [palette];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {}}} after\\par}}",
        bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
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
        .expect("malformed EMF CREATEPALETTE placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn emf_palette_mutation_records_are_passive_noops_without_payload_leakage() {
    let mut set_entries = emf_set_palette_entries_record(4, 1, &[(12, 34, 56), (78, 90, 123)]);
    set_entries.extend_from_slice(b"EMF-SETPALETTEENTRIES /JavaScript /EmbeddedFile");
    set_entries.resize(set_entries.len().next_multiple_of(4), 0);
    let set_entries_len = set_entries.len() as u32;
    write_test_le_u32(&mut set_entries, 4, set_entries_len);

    let mut resize = emf_i32_pair_record(51, 4, 16);
    resize.extend_from_slice(b"EMF-RESIZEPALETTE /Launch /RichMedia");
    resize.resize(resize.len().next_multiple_of(4), 0);
    let resize_len = resize.len() as u32;
    write_test_le_u32(&mut resize, 4, resize_len);

    let records = [set_entries, resize, emf_rect_record(43, 10, 10, 70, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF palette mutation vector image");

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
        "emfblip",
        "EMF-SETPALETTEENTRIES",
        "EMF-RESIZEPALETTE",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
        "RichMedia",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF palette mutation payload/control leaked to normalized text: {forbidden}"
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
        "drawing after EMF palette mutations should render a passive PDF rectangle"
    );
    assert!(output.diagnostics.is_empty());
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETPALETTEENTRIES",
        b"EMF-RESIZEPALETTE",
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
            "EMF palette mutation payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_emf_setpaletteentries_becomes_passive_placeholder() {
    let mut entries = emf_set_palette_entries_record(4, 0, &[(255, 0, 0)]);
    write_test_le_u32(&mut entries, 16, 2);
    let records = [entries];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {}}} after\\par}}",
        bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
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
        .expect("malformed EMF SETPALETTEENTRIES placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn unsupported_emf_pen_styles_are_inert_without_payload_leakage() {
    for (mut pen, marker) in [
        (
            emf_create_pen_record(
                3,
                9,
                7,
                Color {
                    red: 220,
                    green: 80,
                    blue: 40,
                },
            ),
            "EMF-CREATEPEN-UNSUPPORTED",
        ),
        (
            emf_extcreatepen_record(
                3,
                9,
                7,
                0,
                Color {
                    red: 220,
                    green: 80,
                    blue: 40,
                },
                &[],
            ),
            "EMF-EXTCREATEPEN-UNSUPPORTED",
        ),
    ] {
        pen.extend_from_slice(format!("{marker} /JavaScript /EmbeddedFile /Launch").as_bytes());
        pen.resize(pen.len().next_multiple_of(4), 0);
        let pen_len = pen.len() as u32;
        write_test_le_u32(&mut pen, 4, pen_len);
        let records = [
            pen,
            emf_select_object_record(3),
            emf_point_record(27, 10, 20),
            emf_point_record(54, 70, 60),
        ];
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
            .expect("unsupported EMF pen-style vector image");

        assert!(text.contains("before"));
        assert!(text.contains("after"));
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Line {
                stroke_color: Some(Color {
                    red: 0,
                    green: 0,
                    blue: 0
                }),
                stroke_width: 1.0,
                stroke_style: BorderStyle::Single,
                ..
            }
        ));
        for forbidden in ["emfblip", marker, "JavaScript", "EmbeddedFile", "Launch"] {
            assert!(
                !text.contains(forbidden),
                "unsupported EMF pen payload/control leaked to normalized text: {forbidden}"
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
        assert!(
            content
                .operations
                .iter()
                .any(|operation| matches!(operation.operator.as_str(), "m" | "l" | "S")),
            "drawing after unsupported EMF pen should render passive PDF path operations"
        );
        for forbidden in [
            b"emfblip".as_slice(),
            emf_hex.as_bytes(),
            marker.as_bytes(),
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
                "unsupported EMF pen payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn unknown_high_bit_emf_pen_styles_are_inert_without_payload_leakage() {
    for (mut pen, select_handle, marker) in [
        (
            emf_create_pen_record(
                3,
                0x0040_0001,
                7,
                Color {
                    red: 220,
                    green: 80,
                    blue: 40,
                },
            ),
            3,
            "EMF-CREATEPEN-HIGHBITS",
        ),
        (
            emf_extcreatepen_record(
                4,
                0x0080_0002,
                9,
                0,
                Color {
                    red: 30,
                    green: 160,
                    blue: 210,
                },
                &[],
            ),
            4,
            "EMF-EXTCREATEPEN-HIGHBITS",
        ),
    ] {
        pen.extend_from_slice(format!("{marker} /JavaScript /EmbeddedFile /Launch").as_bytes());
        pen.resize(pen.len().next_multiple_of(4), 0);
        let pen_len = pen.len() as u32;
        write_test_le_u32(&mut pen, 4, pen_len);
        let records = [
            pen,
            emf_select_object_record(select_handle),
            emf_point_record(27, 10, 20),
            emf_point_record(54, 70, 60),
        ];
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
            .expect("unknown high-bit EMF pen-style vector image");

        assert!(text.contains("before"));
        assert!(text.contains("after"));
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Line {
                stroke_color: Some(Color {
                    red: 0,
                    green: 0,
                    blue: 0
                }),
                stroke_width: 1.0,
                stroke_style: BorderStyle::Single,
                ..
            }
        ));
        for forbidden in ["emfblip", marker, "JavaScript", "EmbeddedFile", "Launch"] {
            assert!(
                !text.contains(forbidden),
                "unknown high-bit EMF pen payload/control leaked to normalized text: {forbidden}"
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
        assert!(
            content
                .operations
                .iter()
                .any(|operation| matches!(operation.operator.as_str(), "m" | "l" | "S")),
            "drawing after unknown high-bit EMF pen should render passive PDF path operations"
        );
        for forbidden in [
            b"emfblip".as_slice(),
            emf_hex.as_bytes(),
            marker.as_bytes(),
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
                "unknown high-bit EMF pen payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn emf_extcreatepen_records_render_passively_without_payload_leakage() {
    let payload = b"EXTCREATEPEN-DIB-PAYLOAD";
    let records = [
        emf_extcreatepen_record(
            4,
            1,
            5,
            0,
            Color {
                red: 90,
                green: 30,
                blue: 210,
            },
            payload,
        ),
        emf_select_object_record(4),
        emf_point_record(27, 10, 20),
        emf_point_record(54, 80, 60),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF EXTCREATEPEN vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("EXTCREATEPEN-DIB-PAYLOAD"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Line {
            stroke_color: Some(Color {
                red: 90,
                green: 30,
                blue: 210
            }),
            stroke_width: 5.0,
            stroke_style: BorderStyle::Dashed,
            ..
        }
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF EXTCREATEPEN payload/control leaked to normalized text: {forbidden}"
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
            .any(|operation| operation.operator == "RG"),
        "EMF EXTCREATEPEN stroke color should render as passive PDF stroke color"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "w"),
        "EMF EXTCREATEPEN stroke width should render as passive PDF line width"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        payload.as_slice(),
        b" EMF",
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
            "EMF EXTCREATEPEN payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_extcreatepen_irrelevant_invalid_payload_does_not_leak() {
    let mut ext_pen = emf_extcreatepen_record(
        4,
        0,
        5,
        0,
        Color {
            red: 90,
            green: 30,
            blue: 210,
        },
        &[],
    );
    write_test_le_u32(&mut ext_pen, 12, 999);
    write_test_le_u32(&mut ext_pen, 16, 4);
    ext_pen.extend_from_slice(b"IRRELEVANT-EXTCREATEPEN-PAYLOAD /JavaScript /EmbeddedFile");
    ext_pen.resize(ext_pen.len().next_multiple_of(4), 0);
    let ext_pen_len = ext_pen.len() as u32;
    write_test_le_u32(&mut ext_pen, 4, ext_pen_len);

    let records = [
        ext_pen,
        emf_select_object_record(4),
        emf_point_record(27, 10, 20),
        emf_point_record(54, 80, 60),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF EXTCREATEPEN invalid irrelevant payload image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Line {
            stroke_color: Some(Color {
                red: 90,
                green: 30,
                blue: 210
            }),
            stroke_width: 5.0,
            stroke_style: BorderStyle::Single,
            ..
        }
    ));
    for forbidden in [
        "emfblip",
        "IRRELEVANT-EXTCREATEPEN-PAYLOAD",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "irrelevant EXTCREATEPEN payload/control leaked to normalized text: {forbidden}"
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
            .any(|operation| operation.operator == "l"),
        "EXTCREATEPEN line should render passive PDF line operations"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"IRRELEVANT-EXTCREATEPEN-PAYLOAD",
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
            "irrelevant EXTCREATEPEN payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_stroked_path_records_render_passively_without_payload_leakage() {
    let records = [
        emf_create_pen_record(
            2,
            1,
            4,
            Color {
                red: 120,
                green: 40,
                blue: 10,
            },
        ),
        emf_select_object_record(2),
        emf_unknown_record(59),
        emf_point_record(27, 10, 10),
        emf_point_record(54, 50, 10),
        emf_poly_record(6, &[(50, 40), (10, 40)]),
        emf_unknown_record(61),
        emf_point_record(27, 70, 20),
        emf_poly_record(5, &[(80, 5), (100, 35), (110, 20)]),
        emf_unknown_record(60),
        emf_unknown_record(64),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF stroked path vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Path {
            stroke_color: Some(Color {
                red: 120,
                green: 40,
                blue: 10
            }),
            stroke_width: 4.0,
            stroke_style: BorderStyle::Dashed,
            ..
        }
    ));
    assert!(
        matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::Path { segments, .. }
                if segments.iter().any(|segment| matches!(
                    segment,
                    StaticImageVectorPathSegment::CubicTo { .. }
                ))
        ),
        "stroked EMF path Bezier segment should be preserved passively"
    );
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF stroked path payload/control leaked to normalized text: {forbidden}"
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
            .any(|operation| operation.operator == "l"),
        "EMF stroked paths should render passive PDF line operations"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "c"),
        "EMF stroked paths should render passive PDF cubic operations"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "S"),
        "EMF stroked paths should render passive PDF stroke operations"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF stroked path payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_unpainted_path_renders_blank_vector_without_payload_leakage() {
    let mut line = emf_point_record(54, 50, 10);
    line.extend_from_slice(b"BLANK-EMF-UNPAINTED-PATH /JavaScript /EmbeddedFile /Launch");
    line.resize(line.len().next_multiple_of(4), 0);
    let line_len = line.len() as u32;
    write_test_le_u32(&mut line, 4, line_len);

    let records = [emf_unknown_record(59), emf_point_record(27, 10, 10), line];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("blank passive EMF vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "BLANK-EMF-UNPAINTED-PATH",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
        "Image skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "blank EMF vector payload/control leaked to normalized text: {forbidden}"
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
        "blank EMF vector should preserve the passive image box clip"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"BLANK-EMF-UNPAINTED-PATH",
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
            "blank EMF vector payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_fillpath_records_render_passively_without_payload_leakage() {
    let records = [
        emf_create_pen_record(
            2,
            0,
            3,
            Color {
                red: 180,
                green: 30,
                blue: 40,
            },
        ),
        emf_select_object_record(2),
        emf_create_brush_record(
            3,
            0,
            Color {
                red: 230,
                green: 210,
                blue: 80,
            },
            0,
        ),
        emf_select_object_record(3),
        emf_unknown_record(59),
        emf_point_record(27, 20, 20),
        emf_poly_record(6, &[(70, 20), (45, 60)]),
        emf_unknown_record(61),
        emf_unknown_record(60),
        emf_unknown_record(63),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF fill path vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Polygon {
            stroke_color: Some(Color {
                red: 180,
                green: 30,
                blue: 40
            }),
            stroke_width: 3.0,
            fill_color: Some(Color {
                red: 230,
                green: 210,
                blue: 80
            }),
            ..
        }
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF fill path payload/control leaked to normalized text: {forbidden}"
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
            .any(|operation| operation.operator == "rg"),
        "EMF fill paths should render passive PDF fill color"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "RG"),
        "EMF stroke-and-fill paths should render passive PDF stroke color"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "B" | "B*")),
        "EMF stroke-and-fill paths should render passive PDF fill/stroke operations"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF fill path payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_filled_bezier_paths_render_passively_without_payload_leakage() {
    let records = [
        emf_create_brush_record(
            3,
            0,
            Color {
                red: 80,
                green: 140,
                blue: 210,
            },
            0,
        ),
        emf_select_object_record(3),
        emf_unknown_record(59),
        emf_point_record(27, 20, 20),
        emf_poly_record(5, &[(30, 5), (60, 55), (80, 20)]),
        emf_unknown_record(60),
        emf_unknown_record(62),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF filled Bezier path image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Path {
            fill_color: Some(Color {
                red: 80,
                green: 140,
                blue: 210
            }),
            ..
        }
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF filled Bezier payload/control leaked to normalized text: {forbidden}"
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
            .any(|operation| operation.operator == "c"),
        "EMF filled Bezier path should render passive PDF cubic operations"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "f*")),
        "EMF filled Bezier path should render passive PDF fill operations"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF filled Bezier path payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn word_suppresses_emf_without_device_metrics_while_strict_spec_renders_fallback() {
    let records = [
        emf_create_brush_record(
            3,
            2,
            Color {
                red: 20,
                green: 90,
                blue: 180,
            },
            5,
        ),
        emf_select_object_record(3),
        emf_unknown_record(59),
        emf_point_record(27, 20, 20),
        emf_poly_record(5, &[(30, 5), (60, 55), (80, 20)]),
        emf_unknown_record(60),
        emf_unknown_record(62),
    ];
    let mut emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    for offset in [72, 76, 80, 84] {
        write_test_le_i32(&mut emf, offset, 0);
    }
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
    let word_parsed = parse_rtf_bytes(&input).unwrap();
    assert!(
        !word_parsed
            .document
            .blocks
            .iter()
            .any(|block| matches!(block, Block::Image(_)))
    );
    assert!(word_parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("EMF picture with missing reference-device metrics suppressed")
    }));

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
        .expect("passive EMF hatched Bezier path image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Path {
            fill_pattern: ShadingPattern::DiagonalCross,
            fill_color: Some(Color {
                red: 20,
                green: 90,
                blue: 180
            }),
            ..
        }
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF hatched Bezier payload/control leaked to normalized text: {forbidden}"
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
            .any(|operation| operation.operator == "c"),
        "EMF hatched Bezier path should preserve passive cubic clipping path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "W" | "W*")),
        "EMF hatched Bezier path should clip hatch lines to the passive path"
    );
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "l")
            .count()
            >= 4,
        "EMF hatched Bezier path should render bounded passive hatch lines"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF hatched Bezier path payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_multi_subpath_fillpaths_render_passively_without_payload_leakage() {
    let records = [
        emf_create_brush_record(
            3,
            0,
            Color {
                red: 30,
                green: 170,
                blue: 210,
            },
            0,
        ),
        emf_select_object_record(3),
        emf_unknown_record(59),
        emf_point_record(27, 10, 10),
        emf_poly_record(6, &[(50, 10), (30, 40)]),
        emf_unknown_record(61),
        emf_point_record(27, 80, 20),
        emf_poly_record(6, &[(120, 20), (100, 50)]),
        emf_unknown_record(61),
        emf_unknown_record(60),
        emf_unknown_record(62),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF multi-subpath fill image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Path {
            fill_color: Some(Color {
                red: 30,
                green: 170,
                blue: 210
            }),
            ..
        }
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF multi-subpath payload/control leaked to normalized text: {forbidden}"
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
            .filter(|operation| operation.operator == "m")
            .count()
            >= 2,
        "multi-subpath EMF fill should render separate passive PDF move operations"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "f*")),
        "multi-subpath EMF fill should render passive PDF fill operations"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF multi-subpath payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_intersectcliprect_records_render_passively_without_payload_leakage() {
    let records = [
        emf_rect_record(30, 20, 20, 100, 60),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF clipped vector image");

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
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF clip rect payload/control leaked to normalized text: {forbidden}"
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
        "EMF INTERSECTCLIPRECT should render a passive PDF clipping path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "n"),
        "EMF INTERSECTCLIPRECT should terminate the passive clipping path before painting"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "S" | "f" | "B")),
        "clipped EMF content should still render passive paint operations"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF clip rect payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_empty_intersectcliprect_renders_empty_clip_without_payload_leakage() {
    let mut clip = emf_rect_record(30, 20, 20, 20, 60);
    clip.extend_from_slice(b"EMPTY-EMF-INTERSECTCLIPRECT /JavaScript /EmbeddedFile");
    clip.resize(clip.len().next_multiple_of(4), 0);
    let clip_len = clip.len() as u32;
    write_test_le_u32(&mut clip, 4, clip_len);

    let records = [clip, emf_rect_record(43, 0, 0, 160, 80)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF empty clip vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::ClipRect {
            left: 20.0,
            top: 20.0,
            right: 20.0,
            bottom: 60.0
        }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in [
        "emfblip",
        "EMPTY-EMF-INTERSECTCLIPRECT",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "empty EMF INTERSECTCLIPRECT payload/control leaked to normalized text: {forbidden}"
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
        "empty EMF INTERSECTCLIPRECT should render a passive PDF clipping path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "empty EMF INTERSECTCLIPRECT fixture should still emit passive rectangle paths"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMPTY-EMF-INTERSECTCLIPRECT",
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
            "empty EMF INTERSECTCLIPRECT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_offsetcliprgn_offsets_unpainted_clip_without_payload_leakage() {
    let records = [
        emf_rect_record(30, 20, 20, 100, 60),
        emf_point_record(26, 10, 5),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF offset-clipped vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(
        image.vector_commands[0],
        StaticImageVectorCommand::ClipRect {
            left: 30.0,
            top: 25.0,
            right: 110.0,
            bottom: 65.0,
        }
    );
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF offset clip payload/control leaked to normalized text: {forbidden}"
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
        "EMF OffsetClipRgn should preserve passive PDF clipping"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "S" | "f" | "B")),
        "painting after EMF OffsetClipRgn should still render passively"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF offset clip payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_zero_offsetcliprgn_is_noop_without_payload_leakage() {
    let mut offset = emf_point_record(26, 0, 0);
    offset.extend_from_slice(b"ZERO-OFFSET-EMF-OFFSETCLIPRGN /JavaScript /EmbeddedFile /Launch");
    offset.resize(offset.len().next_multiple_of(4), 0);
    let offset_len = offset.len() as u32;
    write_test_le_u32(&mut offset, 4, offset_len);
    let records = [
        emf_rect_record(43, 0, 0, 80, 40),
        offset,
        emf_rect_record(43, 90, 10, 130, 50),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF zero-offset clip vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "OFFSETCLIPRGN",
        "ZERO-OFFSET-EMF-OFFSETCLIPRGN",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "zero-offset EMF OffsetClipRgn payload/control leaked to normalized text: {forbidden}"
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
        "painting before and after zero-offset EMF OffsetClipRgn should render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"ZERO-OFFSET-EMF-OFFSETCLIPRGN",
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
            "zero-offset EMF OffsetClipRgn payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_offsetcliprgn_offsets_saved_painted_clip_without_payload_leakage() {
    let records = [
        emf_unknown_record(33),
        emf_rect_record(30, 20, 20, 100, 60),
        emf_rect_record(43, 0, 0, 30, 30),
        emf_point_record(26, 10, 5),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF scoped offset-clipped vector image");

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
        "saved scoped EMF OffsetClipRgn should replay an offset passive clip"
    );
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF scoped offset clip payload/control leaked to normalized text: {forbidden}"
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
        "EMF scoped OffsetClipRgn should preserve passive PDF clipping"
    );
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "q")
            .count()
            >= 2,
        "EMF scoped OffsetClipRgn should use passive PDF graphics-state scopes"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "S" | "f" | "B")),
        "painting after scoped EMF OffsetClipRgn should still render passively"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF scoped offset clip payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_offsetcliprgn_offsets_unscoped_painted_clip_without_payload_leakage() {
    let records = [
        emf_rect_record(30, 20, 20, 100, 60),
        emf_f32_record(58, 2.0),
        emf_rect_record(43, 0, 0, 30, 30),
        emf_point_record(26, 10, 5),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF unscoped offset-clipped vector image");

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
            StaticImageVectorCommand::SetMiterLimit { limit: 2.0 }
        )),
        "unscoped EMF OffsetClipRgn should retain passive graphics state before paint"
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
        "unscoped EMF OffsetClipRgn should replay an offset passive clip"
    );
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF unscoped offset clip payload/control leaked to normalized text: {forbidden}"
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
            .filter(|operation| operation.operator == "q")
            .count()
            >= 2,
        "EMF unscoped OffsetClipRgn should emit passive save-state operators"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "Q"),
        "EMF unscoped OffsetClipRgn should emit passive restore-state operators"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "M"),
        "EMF unscoped OffsetClipRgn should retain passive graphics-state operators"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "W"),
        "EMF unscoped OffsetClipRgn should preserve passive PDF clipping"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "S" | "f" | "B")),
        "painting after unscoped EMF OffsetClipRgn should still render passively"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF unscoped offset clip payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_offsetcliprgn_offsets_mutated_painted_clip_without_payload_leakage() {
    let mut offset = emf_point_record(26, 10, 5);
    offset.extend_from_slice(b"MUTATED-EMF-OFFSETCLIPRGN /JavaScript /EmbeddedFile /Launch");
    offset.resize(offset.len().next_multiple_of(4), 0);
    let offset_len = offset.len() as u32;
    write_test_le_u32(&mut offset, 4, offset_len);

    let records = [
        emf_rect_record(30, 20, 20, 100, 60),
        emf_rect_record(43, 0, 0, 30, 30),
        emf_rect_record(30, 40, 30, 120, 70),
        offset,
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF mutated offset-clipped vector image");

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
        "mutated EMF OffsetClipRgn should replay the offset current combined clip"
    );
    for forbidden in [
        "emfblip",
        "OFFSETCLIPRGN",
        "MUTATED-EMF-OFFSETCLIPRGN",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "mutated EMF OffsetClipRgn payload/control leaked to normalized text: {forbidden}"
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
            .filter(|operation| operation.operator == "q")
            .count()
            >= 2,
        "mutated EMF OffsetClipRgn should use passive PDF graphics-state scopes"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "W"),
        "mutated EMF OffsetClipRgn should preserve passive PDF clipping"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"MUTATED-EMF-OFFSETCLIPRGN",
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
            "mutated EMF OffsetClipRgn payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_excludecliprect_records_render_passively_without_payload_leakage() {
    let records = [
        emf_rect_record(29, 20, 20, 100, 60),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF exclude-clipped vector image");

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
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF exclude clip rect payload/control leaked to normalized text: {forbidden}"
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
        "EMF EXCLUDECLIPRECT should render a passive PDF even-odd clipping path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "n"),
        "EMF EXCLUDECLIPRECT should terminate the passive clipping path before painting"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "S" | "f" | "B")),
        "exclude-clipped EMF content should still render passive paint operations"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF exclude clip rect payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_excludecliprect_after_unpainted_clip_rect_renders_passively_without_payload_leakage() {
    let records = [
        emf_rect_record(30, 10, 10, 120, 70),
        emf_rect_record(29, 40, 20, 80, 50),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex} 2f4a617661536372697074 2f456d62656464656446696c65}} after\\par}}"
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
        .expect("passive EMF active exclude-clipped vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert_eq!(parsed.diagnostics.len(), 0);
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
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF active exclude clip rect payload/control leaked to normalized text: {forbidden}"
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
        "EMF active EXCLUDECLIPRECT should render a passive even-odd clipping path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after EMF active EXCLUDECLIPRECT should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF active exclude clip rect payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_selectclippath_records_render_passively_without_payload_leakage() {
    let records = [
        emf_unknown_record(59),
        emf_point_record(27, 20, 20),
        emf_poly_record(6, &[(80, 20), (50, 60)]),
        emf_unknown_record(61),
        emf_unknown_record(60),
        emf_u32_record(67, 1),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF clip path vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::ClipPath { .. }
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF clip path payload/control leaked to normalized text: {forbidden}"
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
        "EMF SELECTCLIPPATH should render a passive PDF even-odd clipping path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "n"),
        "EMF SELECTCLIPPATH should terminate the passive clipping path before painting"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "S" | "f" | "B")),
        "clipped EMF path content should still render passive paint operations"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF clip path payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_selectclippath_copy_records_render_first_clip_without_payload_leakage() {
    let records = [
        emf_unknown_record(59),
        emf_point_record(27, 20, 20),
        emf_poly_record(6, &[(80, 20), (50, 60)]),
        emf_unknown_record(61),
        emf_unknown_record(60),
        emf_u32_record(67, 5),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF copy clip path vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::ClipPath { .. }
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF copy clip path payload/control leaked to normalized text: {forbidden}"
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
        "EMF SELECTCLIPPATH RGN_COPY should render the first clip as a passive path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "n"),
        "EMF SELECTCLIPPATH RGN_COPY should terminate the passive clipping path"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF copy clip path payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_selectclippath_copy_replaces_unpainted_clip_without_payload_leakage() {
    let records = [
        emf_rect_record(30, 10, 10, 90, 70),
        emf_unknown_record(59),
        emf_point_record(27, 20, 20),
        emf_poly_record(6, &[(80, 20), (50, 60)]),
        emf_unknown_record(60),
        emf_u32_record(67, 5),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF replaced clip path vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::ClipPath {
            start: (20.0, 20.0),
            ..
        }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF replaced clip path payload/control leaked to normalized text: {forbidden}"
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
        "EMF SELECTCLIPPATH RGN_COPY replacement should render a passive PDF clip path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "S" | "f" | "B")),
        "painting after replaced EMF clip should still render passively"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF replaced clip path payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_selectclippath_copy_replaces_unscoped_painted_clip_without_payload_leakage() {
    let records = [
        emf_rect_record(30, 10, 10, 90, 70),
        emf_rect_record(43, 0, 0, 30, 30),
        emf_unknown_record(59),
        emf_point_record(27, 20, 20),
        emf_poly_record(6, &[(80, 20), (50, 60)]),
        emf_unknown_record(60),
        emf_u32_record(67, 5),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF unscoped clip replacement vector image");

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
        image
            .vector_commands
            .iter()
            .any(|command| matches!(command, StaticImageVectorCommand::ClipPath { .. })),
        "unscoped EMF RGN_COPY should replay a passive replacement clip"
    );
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF unscoped clip replacement payload/control leaked to normalized text: {forbidden}"
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
            .filter(|operation| operation.operator == "q")
            .count()
            >= 2,
        "EMF unscoped clip replacement should emit passive save-state operators"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "Q"),
        "EMF unscoped clip replacement should emit passive restore-state operators"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "W*"),
        "EMF unscoped SELECTCLIPPATH RGN_COPY should render a passive replacement clip"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF unscoped clip replacement payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_selectclippath_copy_replaces_mutated_painted_clip_without_payload_leakage() {
    let mut copy = emf_u32_record(67, 5);
    copy.extend_from_slice(b"MUTATED-EMF-SELECTCLIPPATH-COPY /JavaScript /EmbeddedFile /Launch");
    copy.resize(copy.len().next_multiple_of(4), 0);
    let copy_len = copy.len() as u32;
    write_test_le_u32(&mut copy, 4, copy_len);

    let records = [
        emf_rect_record(30, 10, 10, 90, 70),
        emf_rect_record(43, 0, 0, 30, 30),
        emf_rect_record(30, 20, 20, 100, 60),
        emf_unknown_record(59),
        emf_point_record(27, 20, 20),
        emf_poly_record(6, &[(80, 20), (50, 60)]),
        emf_unknown_record(60),
        copy,
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF mutated clip replacement vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(
        image
            .vector_commands
            .iter()
            .any(|command| matches!(command, StaticImageVectorCommand::ClipPath { .. })),
        "mutated EMF RGN_COPY should replay a passive replacement clip"
    );
    for forbidden in [
        "emfblip",
        "SELECTCLIPPATH",
        "MUTATED-EMF-SELECTCLIPPATH-COPY",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "mutated EMF clip replacement payload/control leaked to normalized text: {forbidden}"
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
            .filter(|operation| operation.operator == "q")
            .count()
            >= 2,
        "mutated EMF clip replacement should emit passive save-state operators"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "W*"),
        "mutated EMF SELECTCLIPPATH RGN_COPY should render a passive replacement clip"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"MUTATED-EMF-SELECTCLIPPATH-COPY",
        b"SELECTCLIPPATH",
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
            "mutated EMF clip replacement payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_selectclippath_copy_replaces_saved_painted_clip_without_payload_leakage() {
    let records = [
        emf_unknown_record(33),
        emf_rect_record(30, 10, 10, 90, 70),
        emf_rect_record(43, 0, 0, 30, 30),
        emf_unknown_record(59),
        emf_point_record(27, 20, 20),
        emf_poly_record(6, &[(80, 20), (50, 60)]),
        emf_unknown_record(60),
        emf_u32_record(67, 5),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF scoped clip replacement vector image");

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
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF scoped clip replacement payload/control leaked to normalized text: {forbidden}"
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
            .filter(|operation| operation.operator == "q")
            .count()
            >= 2,
        "EMF scoped clip replacement should emit passive save-state operators"
    );
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "Q")
            .count()
            >= 2,
        "EMF scoped clip replacement should emit passive restore-state operators"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "W*"),
        "EMF scoped SELECTCLIPPATH RGN_COPY should render a passive replacement clip"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF scoped clip replacement payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_empty_ended_path_paint_records_are_noops_without_payload_leakage() {
    let mut stroke = emf_unknown_record(64);
    stroke.extend_from_slice(b"EMPTY-EMF-STROKEPATH /JavaScript");
    stroke.resize(stroke.len().next_multiple_of(4), 0);
    let stroke_len = stroke.len() as u32;
    write_test_le_u32(&mut stroke, 4, stroke_len);

    let mut fill = emf_unknown_record(62);
    fill.extend_from_slice(b"EMPTY-EMF-FILLPATH /EmbeddedFile");
    fill.resize(fill.len().next_multiple_of(4), 0);
    let fill_len = fill.len() as u32;
    write_test_le_u32(&mut fill, 4, fill_len);

    let mut stroke_and_fill = emf_unknown_record(63);
    stroke_and_fill.extend_from_slice(b"EMPTY-EMF-STROKEANDFILLPATH /Launch");
    stroke_and_fill.resize(stroke_and_fill.len().next_multiple_of(4), 0);
    let stroke_and_fill_len = stroke_and_fill.len() as u32;
    write_test_le_u32(&mut stroke_and_fill, 4, stroke_and_fill_len);

    let records = [
        emf_unknown_record(59),
        emf_point_record(27, 20, 20),
        emf_point_record(54, 20, 20),
        emf_unknown_record(60),
        stroke,
        emf_unknown_record(59),
        emf_point_record(27, 30, 30),
        emf_point_record(54, 30, 30),
        emf_unknown_record(60),
        fill,
        emf_unknown_record(59),
        emf_point_record(27, 40, 40),
        emf_point_record(54, 40, 40),
        emf_unknown_record(60),
        stroke_and_fill,
        emf_rect_record(43, 0, 0, 80, 40),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF empty path vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "STROKEPATH",
        "FILLPATH",
        "STROKEANDFILLPATH",
        "EMPTY-EMF-STROKEPATH",
        "EMPTY-EMF-FILLPATH",
        "EMPTY-EMF-STROKEANDFILLPATH",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "empty EMF path payload/control leaked to normalized text: {forbidden}"
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
        "painting after empty EMF path no-ops should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMPTY-EMF-STROKEPATH",
        b"EMPTY-EMF-FILLPATH",
        b"EMPTY-EMF-STROKEANDFILLPATH",
        b"STROKEPATH",
        b"FILLPATH",
        b"STROKEANDFILLPATH",
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
            "empty EMF path payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn unmatched_emf_path_records_count_as_partial_without_payload_leakage() {
    for (record_type, label) in [
        (60, "EMF-ENDPATH-UNMATCHED"),
        (61, "EMF-CLOSEFIGURE-UNMATCHED"),
        (62, "EMF-FILLPATH-UNMATCHED"),
        (64, "EMF-STROKEPATH-UNMATCHED"),
        (63, "EMF-STROKEANDFILLPATH-UNMATCHED"),
    ] {
        let mut path_record = emf_unknown_record(record_type);
        path_record.extend_from_slice(label.as_bytes());
        path_record.extend_from_slice(b" /JavaScript /EmbeddedFile /Launch");
        path_record.resize(path_record.len().next_multiple_of(4), 0);
        let path_len = path_record.len() as u32;
        write_test_le_u32(&mut path_record, 4, path_len);

        let records = [path_record, emf_rect_record(43, 20, 10, 80, 50)];
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
            .expect("unmatched EMF path partial vector preview");

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
        for forbidden in ["emfblip", label, "JavaScript", "EmbeddedFile", "Launch"] {
            assert!(
                !text.contains(forbidden),
                "unmatched EMF path payload/control leaked to normalized text: {forbidden}"
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
            label.as_bytes(),
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
                "unmatched EMF path payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn duplicate_emf_beginpath_counts_as_partial_without_payload_leakage() {
    let mut duplicate_begin = emf_unknown_record(59);
    duplicate_begin.extend_from_slice(b"DUPLICATE-EMF-BEGINPATH /JavaScript /EmbeddedFile");
    duplicate_begin.resize(duplicate_begin.len().next_multiple_of(4), 0);
    let duplicate_begin_len = duplicate_begin.len() as u32;
    write_test_le_u32(&mut duplicate_begin, 4, duplicate_begin_len);

    let records = [
        emf_unknown_record(59),
        emf_point_record(27, 10, 10),
        duplicate_begin,
        emf_rect_record(43, 20, 10, 80, 50),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("duplicate EMF BEGINPATH partial vector preview");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(
        image
            .vector_commands
            .iter()
            .any(|command| matches!(command, StaticImageVectorCommand::Rectangle { .. })),
        "safe drawing after duplicate EMF BEGINPATH should stay visible"
    );
    for forbidden in [
        "emfblip",
        "BEGINPATH",
        "DUPLICATE-EMF-BEGINPATH",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "duplicate EMF BEGINPATH payload/control leaked to normalized text: {forbidden}"
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
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "safe drawing after duplicate EMF BEGINPATH should render passively"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"DUPLICATE-EMF-BEGINPATH",
        b"BEGINPATH",
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
            "duplicate EMF BEGINPATH payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_abortpath_discards_path_without_payload_leakage() {
    let mut abort = emf_unknown_record(68);
    abort.extend_from_slice(b"ABORT-EMF-PATH /JavaScript /EmbeddedFile");
    abort.resize(abort.len().next_multiple_of(4), 0);
    let abort_len = abort.len() as u32;
    write_test_le_u32(&mut abort, 4, abort_len);

    let records = [
        emf_unknown_record(59),
        emf_point_record(27, 10, 10),
        emf_point_record(54, 50, 10),
        abort,
        emf_rect_record(43, 0, 0, 80, 40),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF ABORTPATH vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in [
        "emfblip",
        "ABORTPATH",
        "ABORT-EMF-PATH",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF ABORTPATH payload/control leaked to normalized text: {forbidden}"
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
        "painting after EMF ABORTPATH should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"ABORT-EMF-PATH",
        b"ABORTPATH",
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
            "EMF ABORTPATH payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_dangling_unpainted_path_at_eof_discards_state_without_payload_leakage() {
    let mut begin = emf_unknown_record(59);
    begin.extend_from_slice(b"DANGLING-EMF-BEGINPATH /JavaScript /EmbeddedFile");
    begin.resize(begin.len().next_multiple_of(4), 0);
    let begin_len = begin.len() as u32;
    write_test_le_u32(&mut begin, 4, begin_len);

    let records = [
        emf_rect_record(43, 0, 0, 80, 40),
        begin,
        emf_point_record(27, 10, 10),
        emf_point_record(54, 50, 10),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF dangling path vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in [
        "emfblip",
        "BEGINPATH",
        "DANGLING-EMF-BEGINPATH",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "dangling EMF path payload/control leaked to normalized text: {forbidden}"
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
        "painting before dangling EMF path should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"DANGLING-EMF-BEGINPATH",
        b"BEGINPATH",
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
            "dangling EMF path payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_exttextoutw_records_render_passively_without_payload_leakage() {
    let records = [
        emf_u32_record(24, 0x0033_2211),
        emf_u32_record(22, 0x0000_0006),
        emf_exttextoutw_record(40, 20, "Hi", 0, None, false),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF text vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("Hi"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Text {
            x: 40.0,
            y: 20.0,
            ref text,
            color: Some(Color {
                red: 17,
                green: 34,
                blue: 51
            }),
            horizontal_align: StaticImageTextHorizontalAlign::Center,
            ..
        } if text == "Hi"
    ));
    for forbidden in ["emfblip", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF text payload/control leaked to normalized text: {forbidden}"
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
        rendered_text.contains("Hi"),
        "EMF EXTTEXTOUTW should render through passive PDF text operations"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "rg"),
        "EMF text color should render as passive PDF fill color"
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
            "EMF text payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_exttextoutw_dx_records_render_positioned_passive_text_without_payload_leakage() {
    let mut text_record = emf_exttextoutw_dx_record(40, 20, "ABC", &[10, 20, 30]);
    text_record.extend_from_slice(b"EMF-EXTTEXTOUT-DX /JavaScript /EmbeddedFile /Launch");
    text_record.resize(text_record.len().next_multiple_of(4), 0);
    let text_record_len = text_record.len() as u32;
    write_test_le_u32(&mut text_record, 4, text_record_len);
    let records = [emf_u32_record(24, 0x0033_2211), text_record];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF EXTTEXTOUTW Dx vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("ABC"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 3);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::Text {
            x: 40.0,
            text,
            color: Some(Color { red: 17, green: 34, blue: 51 }),
            ..
        } if text == "A"
    ));
    assert!(matches!(
        &image.vector_commands[1],
        StaticImageVectorCommand::Text { x: 50.0, text, .. } if text == "B"
    ));
    assert!(matches!(
        &image.vector_commands[2],
        StaticImageVectorCommand::Text { x: 70.0, text, .. } if text == "C"
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "EMF-EXTTEXTOUT-DX",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF EXTTEXTOUTW Dx payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("A"));
    assert!(rendered_text.contains("B"));
    assert!(rendered_text.contains("C"));
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-EXTTEXTOUT-DX",
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
            "EMF EXTTEXTOUTW Dx payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_exttextoutw_pdy_records_render_positioned_passive_text_without_payload_leakage() {
    let mut text_record = emf_exttextoutw_pdy_record(40, 20, "ABC", &[(10, 5), (20, -3), (30, 0)]);
    text_record.extend_from_slice(b"EMF-EXTTEXTOUT-PDY /JavaScript /EmbeddedFile /Launch");
    text_record.resize(text_record.len().next_multiple_of(4), 0);
    let text_record_len = text_record.len() as u32;
    write_test_le_u32(&mut text_record, 4, text_record_len);
    let records = [emf_u32_record(24, 0x0033_2211), text_record];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF EXTTEXTOUTW PDY vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("ABC"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 3);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::Text { x: 40.0, y: 20.0, text, .. } if text == "A"
    ));
    assert!(matches!(
        &image.vector_commands[1],
        StaticImageVectorCommand::Text { x: 50.0, y: 25.0, text, .. } if text == "B"
    ));
    assert!(matches!(
        &image.vector_commands[2],
        StaticImageVectorCommand::Text { x: 70.0, y: 22.0, text, .. } if text == "C"
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "EMF-EXTTEXTOUT-PDY",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF EXTTEXTOUTW PDY payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("A"));
    assert!(rendered_text.contains("B"));
    assert!(rendered_text.contains("C"));
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-EXTTEXTOUT-PDY",
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
            "EMF EXTTEXTOUTW PDY payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_emf_exttextoutw_dx_becomes_passive_placeholder() {
    let mut text_record = emf_exttextoutw_dx_record(40, 20, "ABC", &[10, 20, 30]);
    let malformed_dx_offset = text_record.len() as u32;
    write_test_le_u32(&mut text_record, 72, malformed_dx_offset);
    let records = [text_record];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {}}} after\\par}}",
        bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
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
        .expect("malformed EMF EXTTEXTOUTW Dx placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn invalid_emf_settextalign_stays_partial_without_payload_leakage() {
    let mut mode = emf_u32_record(22, 0x0000_0004);
    mode.extend_from_slice(b"EMF-SETTEXTALIGN-INVALID /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let size = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, size);
    let records = [
        emf_u32_record(24, 0x0033_2211),
        mode,
        emf_exttextoutw_record(40, 20, "Hi", 0, None, false),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip\\picw160\\pich80\\picwgoal1600\\pichgoal800 {emf_hex}}} after\\par}}"
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
        b"emfblip",
        emf_hex.as_bytes(),
        b"EMF-SETTEXTALIGN-INVALID",
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
            "invalid EMF SETTEXTALIGN payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_emf_passive_state_records_count_as_partial_without_payload_leakage() {
    for (record_type, label) in [
        (22, "EMF-SETTEXTALIGN-MALFORMED"),
        (23, "EMF-SETCOLORADJUSTMENT-MALFORMED"),
        (57, "EMF-SETARCDIRECTION-MALFORMED"),
    ] {
        let records = [
            emf_unknown_record(record_type),
            emf_rect_record(43, 20, 10, 80, 50),
        ];
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
            .expect("malformed EMF passive state partial vector preview");

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
                "malformed EMF passive state payload/control leaked to normalized text: {forbidden}"
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
                "malformed EMF passive state payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn unsupported_emf_exttextout_options_stay_partial_without_payload_leakage() {
    let mut text_record = emf_exttextoutw_record(40, 20, "Hidden", 0x4000, None, false);
    text_record.extend_from_slice(b"EMF-EXTTEXTOUT-UNSUPPORTED /JavaScript /EmbeddedFile /Launch");
    text_record.resize(text_record.len().next_multiple_of(4), 0);
    let text_len = text_record.len() as u32;
    write_test_le_u32(&mut text_record, 4, text_len);
    let records = [text_record];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("unsupported EMF EXTTEXTOUT options placeholder");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("Hidden"));
    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
    for forbidden in [
        "emfblip",
        "EMF-EXTTEXTOUT-UNSUPPORTED",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "unsupported EMF EXTTEXTOUT option payload/control leaked to normalized text: {forbidden}"
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
            .contains("EMF picture records stripped; header dimensions used")
    }));
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(!rendered_text.contains("Hidden"));
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-EXTTEXTOUT-UNSUPPORTED",
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
            "unsupported EMF EXTTEXTOUT option payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn unsupported_emf_polytextout_options_stay_partial_without_payload_leakage() {
    let mut text_record = emf_polytextoutw_record(&[(40, 20, "Hidden", 0x4000, None)]);
    text_record.extend_from_slice(b"EMF-POLYTEXTOUT-UNSUPPORTED /JavaScript /EmbeddedFile /Launch");
    text_record.resize(text_record.len().next_multiple_of(4), 0);
    let text_len = text_record.len() as u32;
    write_test_le_u32(&mut text_record, 4, text_len);
    let records = [text_record];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("unsupported EMF POLYTEXTOUT options placeholder");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("Hidden"));
    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
    for forbidden in [
        "emfblip",
        "EMF-POLYTEXTOUT-UNSUPPORTED",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "unsupported EMF POLYTEXTOUT option payload/control leaked to normalized text: {forbidden}"
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
            .contains("EMF picture records stripped; header dimensions used")
    }));
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(!rendered_text.contains("Hidden"));
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-POLYTEXTOUT-UNSUPPORTED",
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
            "unsupported EMF POLYTEXTOUT option payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_emf_settextalign_keeps_later_safe_text() {
    let records = [
        emf_unknown_record(22),
        emf_exttextoutw_record(40, 20, "Hi", 0, None, false),
    ];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
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
        .expect("malformed EMF SETTEXTALIGN partial vector preview");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(
        image.vector_commands.iter().any(
            |command| matches!(command, StaticImageVectorCommand::Text { text, .. } if text == "Hi")
        ),
        "safe text after malformed EMF SETTEXTALIGN should stay visible"
    );
    assert!(!text.contains("Hi"));
}

#[test]
fn malformed_emf_color_state_becomes_passive_placeholder() {
    for (record_type, name) in [(24, "SETTEXTCOLOR"), (25, "SETBKCOLOR")] {
        let mut color = emf_unknown_record(record_type);
        write_test_le_u32(&mut color, 4, 8);
        let records = [color];
        let input = format!(
            "{{\\rtf1 before {{\\pict\\emfblip\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
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
            .expect("malformed EMF color-state placeholder");

        assert!(text.contains("before"), "{name}");
        assert!(text.contains("after"), "{name}");
        assert_eq!(image.format, ImageFormat::Placeholder, "{name}");
        assert!(image.bytes.is_empty(), "{name}");
        assert!(image.vector_commands.is_empty(), "{name}");
    }
}

#[test]
fn malformed_emf_color_state_counts_as_partial_without_payload_leakage() {
    for (record_type, label) in [
        (24, "EMF-SETTEXTCOLOR-MALFORMED"),
        (25, "EMF-SETBKCOLOR-MALFORMED"),
    ] {
        let mut color = emf_unknown_record(record_type);
        write_test_le_u32(&mut color, 4, 8);
        let records = [color, emf_rect_record(43, 20, 10, 80, 50)];
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
            .expect("malformed EMF color state partial vector preview");

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
        for forbidden in ["emfblip", "JavaScript", "EmbeddedFile", "Launch"] {
            assert!(
                !text.contains(forbidden),
                "malformed EMF color payload/control leaked to normalized text: {forbidden}"
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
                "malformed EMF color payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn emf_polytextoutw_records_render_passively_without_payload_leakage() {
    let records = [
        emf_u32_record(24, 0x0033_2211),
        emf_u32_record(22, 0x0000_0006),
        emf_polytextoutw_record(&[(40, 20, "One", 0, None), (70, 35, "Two", 0, None)]),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF POLYTEXTOUTW vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("One"));
    assert!(!text.contains("Two"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::Text {
            x: 40.0,
            y: 20.0,
            text,
            color: Some(Color { red: 17, green: 34, blue: 51 }),
            horizontal_align: StaticImageTextHorizontalAlign::Center,
            ..
        } if text == "One"
    ));
    assert!(matches!(
        &image.vector_commands[1],
        StaticImageVectorCommand::Text {
            x: 70.0,
            y: 35.0,
            text,
            color: Some(Color { red: 17, green: 34, blue: 51 }),
            horizontal_align: StaticImageTextHorizontalAlign::Center,
            ..
        } if text == "Two"
    ));
    for forbidden in [
        "emfblip",
        "POLYTEXTOUT",
        "offString",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF POLYTEXTOUTW payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("One"));
    assert!(rendered_text.contains("Two"));
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
            "EMF POLYTEXTOUTW payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_polytextoutw_dx_records_render_positioned_passive_text_without_payload_leakage() {
    let records = [
        emf_u32_record(24, 0x0033_2211),
        emf_polytextoutw_dx_record(40, 20, "ABC", &[10, 20, 30]),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF POLYTEXTOUTW Dx vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("ABC"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 3);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::Text { x: 40.0, text, .. } if text == "A"
    ));
    assert!(matches!(
        &image.vector_commands[1],
        StaticImageVectorCommand::Text { x: 50.0, text, .. } if text == "B"
    ));
    assert!(matches!(
        &image.vector_commands[2],
        StaticImageVectorCommand::Text { x: 70.0, text, .. } if text == "C"
    ));
    for forbidden in [
        "emfblip",
        "POLYTEXTOUT",
        "offDx",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF POLYTEXTOUTW Dx payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("A"));
    assert!(rendered_text.contains("B"));
    assert!(rendered_text.contains("C"));
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
            "EMF POLYTEXTOUTW Dx payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_polytextoutw_pdy_records_render_positioned_passive_text_without_payload_leakage() {
    let records = [
        emf_u32_record(24, 0x0033_2211),
        emf_polytextoutw_pdy_record(40, 20, "ABC", &[(10, 5), (20, -3), (30, 0)]),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF POLYTEXTOUTW PDY vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("ABC"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 3);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::Text { x: 40.0, y: 20.0, text, .. } if text == "A"
    ));
    assert!(matches!(
        &image.vector_commands[1],
        StaticImageVectorCommand::Text { x: 50.0, y: 25.0, text, .. } if text == "B"
    ));
    assert!(matches!(
        &image.vector_commands[2],
        StaticImageVectorCommand::Text { x: 70.0, y: 22.0, text, .. } if text == "C"
    ));
    for forbidden in [
        "emfblip",
        "POLYTEXTOUT",
        "offDx",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF POLYTEXTOUTW PDY payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("A"));
    assert!(rendered_text.contains("B"));
    assert!(rendered_text.contains("C"));
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
            "EMF POLYTEXTOUTW PDY payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_polytextouta_dx_records_render_positioned_passive_text_without_payload_leakage() {
    let records = [
        emf_u32_record(24, 0x0066_4422),
        emf_polytextouta_dx_record(30, 18, b"ABC", &[8, 12, 16]),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF POLYTEXTOUTA Dx vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("ABC"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 3);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::Text { x: 30.0, text, .. } if text == "A"
    ));
    assert!(matches!(
        &image.vector_commands[1],
        StaticImageVectorCommand::Text { x: 38.0, text, .. } if text == "B"
    ));
    assert!(matches!(
        &image.vector_commands[2],
        StaticImageVectorCommand::Text { x: 50.0, text, .. } if text == "C"
    ));
    for forbidden in [
        "emfblip",
        "POLYTEXTOUT",
        "offDx",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF POLYTEXTOUTA Dx payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("A"));
    assert!(rendered_text.contains("B"));
    assert!(rendered_text.contains("C"));
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
            "EMF POLYTEXTOUTA Dx payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_exttextoutw_opaque_bounds_render_passive_background_without_payload_leakage() {
    let records = [
        emf_u32_record(25, 0x0078_b4dc),
        emf_exttextoutw_record(40, 20, "Hi", 0x0002, Some((30, 10, 90, 35)), false),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF opaque text vector image");

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
            bottom: 35.0,
            stroke_color: None,
            fill_color: Some(Color {
                red: 220,
                green: 180,
                blue: 120
            }),
            ..
        }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Text { ref text, .. } if text == "Hi"
    ));
    for forbidden in ["emfblip", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF opaque text payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("Hi"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "EMF opaque text bounds should render a passive PDF rectangle"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "f*")),
        "EMF opaque text bounds should render a passive PDF fill"
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
            "EMF opaque text payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn empty_emf_exttextout_opaque_bounds_render_passive_background_without_payload_leakage() {
    let mut empty_text = emf_exttextoutw_record(40, 20, "", 0x0002, Some((30, 10, 90, 35)), false);
    empty_text.extend_from_slice(b"EMPTY-EMF-EXTTEXTOUT /JavaScript /EmbeddedFile /Launch");
    empty_text.resize(empty_text.len().next_multiple_of(4), 0);
    let empty_text_len = empty_text.len() as u32;
    write_test_le_u32(&mut empty_text, 4, empty_text_len);

    let records = [
        emf_u32_record(25, 0x0078_b4dc),
        empty_text,
        emf_rect_record(43, 0, 0, 80, 40),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive empty EMF opaque text vector image");

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
            bottom: 35.0,
            stroke_color: None,
            fill_color: Some(Color {
                red: 220,
                green: 180,
                blue: 120
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
        "emfblip",
        "EMPTY-EMF-EXTTEXTOUT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "empty EMF opaque text payload/control leaked to normalized text: {forbidden}"
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
    assert!(!rendered_text.contains("EMPTY-EMF-EXTTEXTOUT"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "re")
            .count()
            >= 2,
        "empty EMF opaque text bounds and later rectangle should render passive PDF rectangles"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "f*")),
        "empty EMF opaque text bounds should render a passive PDF fill"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMPTY-EMF-EXTTEXTOUT",
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
            "empty EMF opaque text payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn sanitized_empty_emf_exttextout_opaque_bounds_render_passive_background_without_payload_leakage()
{
    let mut empty_text =
        emf_exttextoutw_record(40, 20, "\0\0", 0x0002, Some((30, 10, 90, 35)), false);
    empty_text
        .extend_from_slice(b"SANITIZED-EMPTY-EMF-EXTTEXTOUT /JavaScript /EmbeddedFile /Launch");
    empty_text.resize(empty_text.len().next_multiple_of(4), 0);
    let empty_text_len = empty_text.len() as u32;
    write_test_le_u32(&mut empty_text, 4, empty_text_len);

    let records = [
        emf_u32_record(25, 0x0078_b4dc),
        empty_text,
        emf_rect_record(43, 0, 0, 80, 40),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive sanitized-empty EMF opaque text vector image");

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
            bottom: 35.0,
            stroke_color: None,
            fill_color: Some(Color {
                red: 220,
                green: 180,
                blue: 120
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
        "emfblip",
        "SANITIZED-EMPTY-EMF-EXTTEXTOUT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "sanitized-empty EMF opaque text payload/control leaked to normalized text: {forbidden}"
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
    assert!(!rendered_text.contains("SANITIZED-EMPTY-EMF-EXTTEXTOUT"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "re")
            .count()
            >= 2,
        "sanitized-empty EMF opaque text bounds and later rectangle should render passive PDF rectangles"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"SANITIZED-EMPTY-EMF-EXTTEXTOUT",
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
            "sanitized-empty EMF opaque text payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_settextcharextra_renders_passive_spacing_without_payload_leakage() {
    let mut extra = emf_i32_record(106, 12);
    extra.extend_from_slice(b"EMF-SETTEXTCHAREXTRA /JavaScript /EmbeddedFile /Launch");
    extra.resize(extra.len().next_multiple_of(4), 0);
    let extra_len = extra.len() as u32;
    write_test_le_u32(&mut extra, 4, extra_len);
    let records = [
        emf_size_record(9, 320, 160),
        emf_size_record(11, 160, 80),
        extra,
        emf_exttextoutw_record(40, 20, "Hi", 0, None, false),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF text character extra vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Text {
            ref text,
            character_extra,
            word_extra: 0.0,
            ..
        } if text == "Hi" && (character_extra - 6.0).abs() < 0.01
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "EMF-SETTEXTCHAREXTRA",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF text character extra payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("Hi"));
    assert!(
        content.operations.iter().any(|operation| {
            operation.operator == "Tc"
                && operation
                    .operands
                    .first()
                    .and_then(pdf_operand_number)
                    .is_some_and(|value| value > 1.0)
        }),
        "EMF SETTEXTCHAREXTRA should emit nonzero passive PDF character spacing"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETTEXTCHAREXTRA",
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
            "EMF text character extra payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_emf_settextcharextra_becomes_passive_placeholder() {
    let mut record = vec![0; 8];
    write_test_le_u32(&mut record, 0, 106);
    write_test_le_u32(&mut record, 4, 8);
    let records = [record];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {}}} after\\par}}",
        bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
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
        .expect("malformed EMF SETTEXTCHAREXTRA placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn malformed_emf_region_clip_records_count_as_partial_without_payload_leakage() {
    for (record_type, label) in [
        (26, "EMF-OFFSETCLIPRGN-MALFORMED"),
        (29, "EMF-EXCLUDECLIPRECT-MALFORMED"),
        (30, "EMF-INTERSECTCLIPRECT-MALFORMED"),
        (71, "EMF-FILLRGN-MALFORMED"),
        (72, "EMF-FRAMERGN-MALFORMED"),
        (73, "EMF-INVERTRGN-MALFORMED"),
        (74, "EMF-PAINTRGN-MALFORMED"),
        (75, "EMF-EXTSELECTCLIPRGN-MALFORMED"),
    ] {
        let mut region_record = emf_unknown_record(record_type);
        write_test_le_u32(&mut region_record, 4, 8);
        let records = [region_record, emf_rect_record(43, 20, 10, 80, 50)];
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
            .expect("malformed EMF region/clip partial vector preview");

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
                "malformed EMF region/clip payload/control leaked to normalized text: {forbidden}"
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
                "malformed EMF region/clip payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn malformed_emf_clip_path_state_records_count_as_partial_without_payload_leakage() {
    for (record_type, label) in [
        (67, "EMF-SELECTCLIPPATH-MALFORMED"),
        (98, "EMF-SETICMMODE-MALFORMED"),
    ] {
        let mut state_record = emf_unknown_record(record_type);
        write_test_le_u32(&mut state_record, 4, 8);
        let records = [state_record, emf_rect_record(43, 20, 10, 80, 50)];
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
            .expect("malformed EMF clip-path/state partial vector preview");

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
                "malformed EMF clip-path/state payload/control leaked to normalized text: {forbidden}"
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
                "malformed EMF clip-path/state payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn invalid_emf_selectclippath_mode_counts_as_partial_without_payload_leakage() {
    let mode = emf_u32_record(67, 99);
    let records = [mode, emf_rect_record(43, 20, 10, 80, 50)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("invalid EMF SELECTCLIPPATH mode partial vector preview");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(
        image
            .vector_commands
            .iter()
            .any(|command| matches!(command, StaticImageVectorCommand::Rectangle { .. })),
        "safe drawing after invalid EMF SELECTCLIPPATH mode should stay visible"
    );

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
        "invalid EMF SELECTCLIPPATH mode should produce skipped-record diagnostics: {:?}",
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
        "safe drawing after invalid EMF SELECTCLIPPATH mode should render passively"
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
            "invalid EMF SELECTCLIPPATH mode payload/control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_emf_restore_state_records_count_as_partial_without_payload_leakage() {
    for (records, label) in [
        (
            vec![emf_unknown_record(34), emf_rect_record(43, 20, 10, 80, 50)],
            "EMF-RESTOREDC-MALFORMED",
        ),
        (
            vec![
                emf_unknown_record(33),
                emf_unknown_record(34),
                emf_rect_record(43, 20, 10, 80, 50),
            ],
            "EMF-SAVEDC-RESTOREDC-MALFORMED",
        ),
    ] {
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
            .expect("malformed EMF restore-state partial vector preview");

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
                "malformed EMF restore-state payload/control leaked to normalized text: {forbidden}"
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
                "malformed EMF restore-state payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn malformed_emf_text_spacing_state_counts_as_partial_without_payload_leakage() {
    for (record_type, label) in [
        (106, "EMF-SETTEXTCHAREXTRA-MALFORMED"),
        (120, "EMF-SETTEXTJUSTIFICATION-MALFORMED"),
    ] {
        let mut malformed_state = vec![0; 8];
        write_test_le_u32(&mut malformed_state, 0, record_type);
        write_test_le_u32(&mut malformed_state, 4, 8);
        let records = [malformed_state, emf_rect_record(43, 20, 10, 80, 50)];
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
            .expect("malformed EMF text spacing state partial vector preview");

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
        for forbidden in ["emfblip", "JavaScript", "EmbeddedFile", "Launch"] {
            assert!(
                !text.contains(forbidden),
                "malformed EMF text spacing payload/control leaked to normalized text: {forbidden}"
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
                "malformed EMF text spacing payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn emf_settextjustification_renders_passive_word_spacing_without_payload_leakage() {
    let records = [
        emf_size_record(9, 320, 160),
        emf_size_record(11, 160, 80),
        emf_i32_pair_record(120, 40, 2),
        emf_exttextoutw_record(40, 20, "A B C", 0, None, false),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF text justification vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Text {
            ref text,
            word_extra,
            character_extra: 0.0,
            ..
        } if text == "A B C" && (word_extra - 10.0).abs() < 0.01
    ));
    for forbidden in ["emfblip", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF text justification payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("A B C"));
    assert!(
        content.operations.iter().any(|operation| {
            operation.operator == "Tw"
                && operation
                    .operands
                    .first()
                    .and_then(|operand| operand.as_f32().ok())
                    .is_some_and(|value| value.abs() > 0.01)
        }),
        "EMF SETTEXTJUSTIFICATION should emit nonzero passive PDF word spacing"
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
            "EMF text justification payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn invalid_emf_settextjustification_stays_partial_without_payload_leakage() {
    let mut justification = emf_i32_pair_record(120, 40, -2);
    justification
        .extend_from_slice(b"EMF-SETTEXTJUSTIFICATION-INVALID /JavaScript /EmbeddedFile /Launch");
    justification.resize(justification.len().next_multiple_of(4), 0);
    let justification_len = justification.len() as u32;
    write_test_le_u32(&mut justification, 4, justification_len);
    let records = [
        justification,
        emf_exttextoutw_record(40, 20, "A B C", 0, None, false),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
    assert!(rendered_text.contains("A B C"));
    assert!(
        !content.operations.iter().any(|operation| {
            operation.operator == "Tw"
                && operation
                    .operands
                    .first()
                    .and_then(|operand| operand.as_f32().ok())
                    .is_some_and(|value| value.abs() > 0.01)
        }),
        "invalid EMF SETTEXTJUSTIFICATION should not mutate passive PDF word spacing"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETTEXTJUSTIFICATION-INVALID",
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
            "invalid EMF SETTEXTJUSTIFICATION payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn huge_emf_settextjustification_is_clamped_passively() {
    let records = [
        emf_i32_pair_record(120, i32::MAX, 1),
        emf_exttextoutw_record(40, 20, "A B C", 0, None, false),
    ];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
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
        .expect("clamped EMF text justification vector image");

    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Text {
            word_extra,
            ..
        } if (word_extra - 160.0).abs() < 0.01
    ));
}

#[test]
fn malformed_emf_settextjustification_becomes_passive_placeholder() {
    let mut justification = emf_unknown_record(120);
    write_test_le_u32(&mut justification, 4, 8);
    let records = [justification];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip\\picw160\\pich80\\picwgoal1600\\pichgoal800 {}}} after\\par}}",
        bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
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
        .expect("malformed EMF text justification placeholder");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn emf_comment_records_are_ignored_without_payload_leakage() {
    let records = [
        emf_comment_record(b"EMF-COMMENT /JavaScript /EmbeddedFile /Launch"),
        emf_rect_record(43, 0, 0, 80, 40),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF comment vector image");

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
        "emfblip",
        "EMF-COMMENT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF comment payload/control leaked to normalized text: {forbidden}"
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
        "drawing after EMF comment should render a passive PDF rectangle"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-COMMENT",
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
            "EMF comment payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_comment_only_renders_blank_vector_without_payload_leakage() {
    let records = [emf_comment_record(
        b"BLANK-EMF-COMMENT /JavaScript /EmbeddedFile /Launch",
    )];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("blank passive EMF comment vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "BLANK-EMF-COMMENT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "blank EMF comment payload/control leaked to normalized text: {forbidden}"
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
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"BLANK-EMF-COMMENT",
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
            "blank EMF comment payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_setstretchbltmode_is_passive_state_without_payload_leakage() {
    let mut mode = emf_u32_record(21, 4);
    mode.extend_from_slice(b"EMF-SETSTRETCHBLTMODE /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF stretch mode vector image");

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
        "emfblip",
        "EMF-SETSTRETCHBLTMODE",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF SETSTRETCHBLTMODE payload/control leaked to normalized text: {forbidden}"
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
        "drawing after EMF SETSTRETCHBLTMODE should render a passive PDF rectangle"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETSTRETCHBLTMODE",
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
            "EMF SETSTRETCHBLTMODE payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn invalid_emf_modes_stay_partial_without_payload_leakage() {
    for (record_type, value, marker) in [
        (18, 99, "EMF-SETBKMODE-INVALID"),
        (19, 99, "EMF-SETPOLYFILLMODE-INVALID"),
        (21, 99, "EMF-SETSTRETCHBLTMODE-INVALID"),
    ] {
        let mut mode = emf_u32_record(record_type, value);
        mode.extend_from_slice(format!("{marker} /JavaScript /EmbeddedFile /Launch").as_bytes());
        mode.resize(mode.len().next_multiple_of(4), 0);
        let mode_len = mode.len() as u32;
        write_test_le_u32(&mut mode, 4, mode_len);
        let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
        let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
        let emf_hex = bytes_to_hex(&emf);
        let input =
            format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "re"),
            "drawing after invalid EMF mode should render a passive PDF rectangle"
        );
        for forbidden in [
            b"emfblip".as_slice(),
            emf_hex.as_bytes(),
            marker.as_bytes(),
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
                "invalid EMF mode payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn emf_mm_text_setmapmode_is_passive_state_without_payload_leakage() {
    let mut mode = emf_u32_record(17, 1);
    mode.extend_from_slice(b"EMF-SETMAPMODE-MM-TEXT /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF map mode vector image");

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
        "emfblip",
        "EMF-SETMAPMODE-MM-TEXT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF SETMAPMODE payload/control leaked to normalized text: {forbidden}"
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
        "drawing after EMF SETMAPMODE MM_TEXT should render a passive PDF rectangle"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETMAPMODE-MM-TEXT",
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
            "EMF SETMAPMODE payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn anisotropic_emf_setmapmode_is_passive_state_without_payload_leakage() {
    let mut mode = emf_u32_record(17, 8);
    mode.extend_from_slice(b"EMF-SETMAPMODE-ANISOTROPIC /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF anisotropic map mode vector image");

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
        "emfblip",
        "EMF-SETMAPMODE-ANISOTROPIC",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF anisotropic SETMAPMODE payload/control leaked to normalized text: {forbidden}"
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
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETMAPMODE-ANISOTROPIC",
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
            "EMF anisotropic SETMAPMODE payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_default_setrop2_is_passive_state_without_payload_leakage() {
    let mut mode = emf_u32_record(20, 13);
    mode.extend_from_slice(b"EMF-SETROP2-COPYPEN /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF ROP2 vector image");

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
        "emfblip",
        "EMF-SETROP2-COPYPEN",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF SETROP2 payload/control leaked to normalized text: {forbidden}"
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
        "drawing after EMF SETROP2 R2_COPYPEN should render a passive PDF rectangle"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETROP2-COPYPEN",
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
            "EMF SETROP2 payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn notcopypen_emf_setrop2_inverts_passive_pen_without_payload_leakage() {
    let mut mode = emf_u32_record(20, 4);
    mode.extend_from_slice(b"EMF-SETROP2-NOTCOPYPEN /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [
        mode,
        emf_create_pen_record(
            2,
            0,
            1,
            Color {
                red: 20,
                green: 40,
                blue: 60,
            },
        ),
        emf_select_object_record(2),
        emf_rect_record(43, 0, 0, 80, 40),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF R2_NOTCOPYPEN vector image");

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
        "emfblip",
        "EMF-SETROP2-NOTCOPYPEN",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF SETROP2 R2_NOTCOPYPEN payload/control leaked to normalized text: {forbidden}"
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
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETROP2-NOTCOPYPEN",
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
            "EMF SETROP2 R2_NOTCOPYPEN payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn non_default_emf_setrop2_stays_partial_without_payload_leakage() {
    let mut mode = emf_u32_record(20, 6);
    mode.extend_from_slice(b"EMF-SETROP2-UNSUPPORTED /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETROP2-UNSUPPORTED",
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
            "unsupported EMF SETROP2 payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_emf_setrop2_becomes_passive_placeholder() {
    let mut mode = emf_unknown_record(20);
    write_test_le_u32(&mut mode, 4, 8);
    let records = [mode];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {}}} after\\par}}",
        bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
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
        .expect("malformed EMF ROP2 placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn emf_default_setlayout_is_passive_state_without_payload_leakage() {
    let mut mode = emf_u32_record(115, 0);
    mode.extend_from_slice(b"EMF-SETLAYOUT-LTR /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF layout vector image");

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
        "emfblip",
        "EMF-SETLAYOUT-LTR",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF SETLAYOUT payload/control leaked to normalized text: {forbidden}"
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
        "drawing after EMF SETLAYOUT default layout should render a passive PDF rectangle"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETLAYOUT-LTR",
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
            "EMF SETLAYOUT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn non_default_emf_setlayout_stays_partial_without_payload_leakage() {
    let mut mode = emf_u32_record(115, 1);
    mode.extend_from_slice(b"EMF-SETLAYOUT-UNSUPPORTED /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETLAYOUT-UNSUPPORTED",
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
            "unsupported EMF SETLAYOUT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_emf_setlayout_becomes_passive_placeholder() {
    let mut mode = emf_unknown_record(115);
    write_test_le_u32(&mut mode, 4, 8);
    let records = [mode];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {}}} after\\par}}",
        bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
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
        .expect("malformed EMF layout placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn emf_default_setbrushorgex_is_passive_state_without_payload_leakage() {
    let mut mode = emf_i32_pair_record(13, 0, 0);
    mode.extend_from_slice(b"EMF-SETBRUSHORGEX-DEFAULT /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF brush origin vector image");

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
        "emfblip",
        "EMF-SETBRUSHORGEX-DEFAULT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF SETBRUSHORGEX payload/control leaked to normalized text: {forbidden}"
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
        "drawing after default EMF SETBRUSHORGEX should render a passive PDF rectangle"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETBRUSHORGEX-DEFAULT",
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
            "EMF SETBRUSHORGEX payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn non_default_emf_setbrushorgex_is_passive_noop_without_payload_leakage() {
    let mut mode = emf_i32_pair_record(13, 3, 4);
    mode.extend_from_slice(b"EMF-SETBRUSHORGEX-NONDEFAULT /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF non-default brush origin vector image");

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
        "emfblip",
        "EMF-SETBRUSHORGEX-NONDEFAULT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "non-default EMF SETBRUSHORGEX payload/control leaked to normalized text: {forbidden}"
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
        "drawing after non-default EMF SETBRUSHORGEX should render a passive PDF rectangle"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETBRUSHORGEX-NONDEFAULT",
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
            "non-default EMF SETBRUSHORGEX payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_emf_setbrushorgex_becomes_passive_placeholder() {
    let mut mode = emf_unknown_record(13);
    write_test_le_u32(&mut mode, 4, 8);
    let records = [mode];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {}}} after\\par}}",
        bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
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
        .expect("malformed EMF brush origin placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn emf_default_setmapperflags_is_passive_state_without_payload_leakage() {
    let mut mode = emf_u32_record(16, 0);
    mode.extend_from_slice(b"EMF-SETMAPPERFLAGS-DEFAULT /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF mapper flags vector image");

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
        "emfblip",
        "EMF-SETMAPPERFLAGS-DEFAULT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF SETMAPPERFLAGS payload/control leaked to normalized text: {forbidden}"
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
        "drawing after default EMF SETMAPPERFLAGS should render a passive PDF rectangle"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETMAPPERFLAGS-DEFAULT",
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
            "EMF SETMAPPERFLAGS payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn non_default_emf_setmapperflags_is_passive_noop_without_payload_leakage() {
    let mut mode = emf_u32_record(16, 1);
    mode.extend_from_slice(b"EMF-SETMAPPERFLAGS-NONDEFAULT /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF non-default mapper flags vector image");

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
        "emfblip",
        "EMF-SETMAPPERFLAGS-NONDEFAULT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "non-default EMF SETMAPPERFLAGS payload/control leaked to normalized text: {forbidden}"
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
        "drawing after non-default EMF SETMAPPERFLAGS should render a passive PDF rectangle"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETMAPPERFLAGS-NONDEFAULT",
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
            "non-default EMF SETMAPPERFLAGS payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_emf_setmapperflags_becomes_passive_placeholder() {
    let mut mode = emf_unknown_record(16);
    write_test_le_u32(&mut mode, 4, 8);
    let records = [mode];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {}}} after\\par}}",
        bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
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
        .expect("malformed EMF mapper flags placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn emf_default_setmiterlimit_is_passive_state_without_payload_leakage() {
    let mut mode = emf_f32_record(58, 10.0);
    mode.extend_from_slice(b"EMF-SETMITERLIMIT-DEFAULT /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF miter limit vector image");

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
        "emfblip",
        "EMF-SETMITERLIMIT-DEFAULT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF SETMITERLIMIT payload/control leaked to normalized text: {forbidden}"
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
        "drawing after default EMF SETMITERLIMIT should render a passive PDF rectangle"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETMITERLIMIT-DEFAULT",
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
            "EMF SETMITERLIMIT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn non_default_emf_setmiterlimit_renders_passive_pdf_state_without_payload_leakage() {
    let mut mode = emf_f32_record(58, 2.0);
    mode.extend_from_slice(b"EMF-SETMITERLIMIT-NONDEFAULT /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF non-default miter limit vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::SetMiterLimit { limit: 2.0 }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "EMF-SETMITERLIMIT-NONDEFAULT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "non-default EMF SETMITERLIMIT payload/control leaked to normalized text: {forbidden}"
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
        content.operations.iter().any(|operation| {
            operation.operator == "M"
                && operation
                    .operands
                    .first()
                    .and_then(pdf_operand_number)
                    .is_some_and(|value| (value - 2.0).abs() < 0.01)
        }),
        "non-default EMF SETMITERLIMIT should render a passive PDF miter-limit operator"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after non-default EMF SETMITERLIMIT should render a passive PDF rectangle"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETMITERLIMIT-NONDEFAULT",
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
            "non-default EMF SETMITERLIMIT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_emf_setmiterlimit_becomes_passive_placeholder() {
    let mut mode = emf_unknown_record(58);
    write_test_le_u32(&mut mode, 4, 8);
    let records = [mode];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {}}} after\\par}}",
        bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
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
        .expect("malformed EMF miter limit placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn emf_icm_off_seticmmode_is_passive_state_without_payload_leakage() {
    let mut mode = emf_u32_record(98, 1);
    mode.extend_from_slice(b"EMF-SETICMMODE-OFF /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF ICM mode vector image");

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
        "emfblip",
        "EMF-SETICMMODE-OFF",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF SETICMMODE payload/control leaked to normalized text: {forbidden}"
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
        "drawing after EMF SETICMMODE ICM_OFF should render a passive PDF rectangle"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETICMMODE-OFF",
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
            "EMF SETICMMODE payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn non_transforming_emf_seticmmode_values_are_passive_state_without_payload_leakage() {
    let mut query = emf_u32_record(98, 3);
    query.extend_from_slice(b"EMF-SETICMMODE-QUERY /JavaScript /EmbeddedFile");
    query.resize(query.len().next_multiple_of(4), 0);
    let query_len = query.len() as u32;
    write_test_le_u32(&mut query, 4, query_len);

    let mut done = emf_u32_record(98, 4);
    done.extend_from_slice(b"EMF-SETICMMODE-DONE /Launch /RichMedia");
    done.resize(done.len().next_multiple_of(4), 0);
    let done_len = done.len() as u32;
    write_test_le_u32(&mut done, 4, done_len);

    let records = [query, done, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF non-transforming ICM modes vector image");

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
        "emfblip",
        "EMF-SETICMMODE-QUERY",
        "EMF-SETICMMODE-DONE",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
        "RichMedia",
    ] {
        assert!(
            !text.contains(forbidden),
            "non-transforming EMF SETICMMODE payload/control leaked to normalized text: {forbidden}"
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
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after non-transforming EMF SETICMMODE should render a passive PDF rectangle"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETICMMODE-QUERY",
        b"EMF-SETICMMODE-DONE",
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
            "non-transforming EMF SETICMMODE payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn non_default_emf_seticmmode_stays_partial_without_payload_leakage() {
    let mut mode = emf_u32_record(98, 2);
    mode.extend_from_slice(b"EMF-SETICMMODE-UNSUPPORTED /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETICMMODE-UNSUPPORTED",
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
            "unsupported EMF SETICMMODE payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_emf_seticmmode_becomes_passive_placeholder() {
    let mut mode = emf_unknown_record(98);
    write_test_le_u32(&mut mode, 4, 8);
    let records = [mode];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {}}} after\\par}}",
        bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
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
        .expect("malformed EMF ICM mode placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn emf_neutral_setcoloradjustment_is_passive_state_without_payload_leakage() {
    let mut mode = emf_color_adjustment_record(0, 10_000, 0, 0);
    mode.extend_from_slice(b"EMF-SETCOLORADJUSTMENT-NEUTRAL /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF color adjustment vector image");

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
        "emfblip",
        "EMF-SETCOLORADJUSTMENT-NEUTRAL",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF SETCOLORADJUSTMENT payload/control leaked to normalized text: {forbidden}"
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
        "drawing after neutral EMF SETCOLORADJUSTMENT should render a passive PDF rectangle"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETCOLORADJUSTMENT-NEUTRAL",
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
            "EMF SETCOLORADJUSTMENT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn non_neutral_emf_setcoloradjustment_stays_partial_without_payload_leakage() {
    let mut mode = emf_color_adjustment_record(0, 8_000, 10, 0);
    mode.extend_from_slice(b"EMF-SETCOLORADJUSTMENT-UNSUPPORTED /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETCOLORADJUSTMENT-UNSUPPORTED",
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
            "unsupported EMF SETCOLORADJUSTMENT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_emf_setcoloradjustment_becomes_passive_placeholder() {
    let mut mode = emf_unknown_record(23);
    write_test_le_u32(&mut mode, 4, 8);
    let records = [mode];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {}}} after\\par}}",
        bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
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
        .expect("malformed EMF color adjustment placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn malformed_emf_setmapmode_becomes_passive_placeholder() {
    let mut mode = emf_unknown_record(17);
    write_test_le_u32(&mut mode, 4, 8);
    let records = [mode];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {}}} after\\par}}",
        bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
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
        .expect("malformed EMF map mode placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn malformed_emf_modes_become_passive_placeholders() {
    for (record_type, name) in [(18, "SETBKMODE"), (19, "SETPOLYFILLMODE")] {
        let mut mode = emf_unknown_record(record_type);
        write_test_le_u32(&mut mode, 4, 8);
        let records = [mode];
        let input = format!(
            "{{\\rtf1 before {{\\pict\\emfblip {}}} after\\par}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
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
            .expect("malformed EMF mode placeholder");

        assert!(text.contains("before"), "{name}");
        assert!(text.contains("after"), "{name}");
        assert_eq!(image.format, ImageFormat::Placeholder, "{name}");
        assert!(image.bytes.is_empty(), "{name}");
        assert!(image.vector_commands.is_empty(), "{name}");
    }
}

#[test]
fn malformed_emf_setstretchbltmode_becomes_passive_placeholder() {
    let mut mode = emf_unknown_record(21);
    write_test_le_u32(&mut mode, 4, 8);
    let records = [mode];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {}}} after\\par}}",
        bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
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
        .expect("malformed EMF stretch mode placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn malformed_emf_scalar_modes_count_as_partial_without_payload_leakage() {
    for (record_type, label) in [
        (17, "EMF-SETMAPMODE-MALFORMED"),
        (18, "EMF-SETBKMODE-MALFORMED"),
        (19, "EMF-SETPOLYFILLMODE-MALFORMED"),
        (20, "EMF-SETROP2-MALFORMED"),
        (21, "EMF-SETSTRETCHBLTMODE-MALFORMED"),
    ] {
        let mut mode = emf_unknown_record(record_type);
        write_test_le_u32(&mut mode, 4, 8);
        let records = [mode, emf_rect_record(43, 20, 10, 80, 50)];
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
            .expect("malformed EMF scalar mode partial vector preview");

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
                "malformed EMF scalar mode payload/control leaked to normalized text: {forbidden}"
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
                "malformed EMF scalar mode payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn malformed_emf_comment_becomes_passive_placeholder() {
    let mut comment = emf_comment_record(b"SHORT-EMF-COMMENT");
    write_test_le_u32(&mut comment, 8, 1024);
    let records = [comment];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {}}} after\\par}}",
        bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
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
        .expect("malformed EMF comment placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn emf_polyline_and_polygon_records_render_passively_without_payload_leakage() {
    let records = [
        emf_poly_record(4, &[(0, 0), (40, 20), (200, 100)]),
        emf_poly_record(3, &[(20, 10), (60, 70), (100, 10)]),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Polyline { ref points, .. }
            if points == &vec![(0.0, 0.0), (40.0, 20.0), (160.0, 80.0)]
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Polygon {
            ref points,
            fill_rule: StaticImageVectorFillRule::Alternate,
            fill_pattern: ShadingPattern::None,
            ..
        } if points == &vec![(20.0, 10.0), (60.0, 70.0), (100.0, 10.0)]
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF poly payload/control leaked to normalized text: {forbidden}"
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
            .any(|operation| matches!(operation.operator.as_str(), "S" | "s")),
        "EMF poly vector preview should render passive stroked paths"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF poly payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_polypolyline_and_polypolygon_records_render_passively_without_payload_leakage() {
    let records = [
        emf_poly_poly_record(7, &[vec![(0, 0), (40, 20)], vec![(50, 10), (200, 100)]]),
        emf_poly_poly_record(
            8,
            &[
                vec![(20, 10), (60, 70), (100, 10)],
                vec![(100, 20), (140, 70), (200, 20)],
            ],
        ),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF multi-poly vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 3);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Polyline { ref points, .. } if points.len() == 2
    ));
    assert!(matches!(
        &image.vector_commands[2],
        StaticImageVectorCommand::Path { segments, .. }
            if segments.iter().any(|segment| matches!(
                segment,
                StaticImageVectorPathSegment::MoveTo(100.0, 20.0)
            ))
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF multi-poly payload/control leaked to normalized text: {forbidden}"
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
            .any(|operation| matches!(operation.operator.as_str(), "S" | "s" | "B" | "B*")),
        "EMF multi-poly vector preview should render passive stroked or filled paths"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF multi-poly payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_poly16_records_render_passively_without_payload_leakage() {
    let records = [
        emf_poly16_record(87, &[(0, 0), (40, 20), (200, 100)]),
        emf_poly16_record(86, &[(20, 10), (60, 70), (100, 10)]),
        emf_poly_poly16_record(90, &[vec![(0, 0), (40, 20)], vec![(50, 10), (200, 100)]]),
        emf_poly_poly16_record(
            91,
            &[
                vec![(20, 10), (60, 70), (100, 10)],
                vec![(100, 20), (140, 70), (200, 20)],
            ],
        ),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF 16-bit poly vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 5);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Polyline { ref points, .. }
            if points == &vec![(0.0, 0.0), (40.0, 20.0), (160.0, 80.0)]
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Polygon { ref points, .. } if points.len() == 3
    ));
    assert!(matches!(
        &image.vector_commands[4],
        StaticImageVectorCommand::Path { segments, .. }
            if segments.iter().any(|segment| matches!(
                segment,
                StaticImageVectorPathSegment::MoveTo(100.0, 20.0)
            ))
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF 16-bit poly payload/control leaked to normalized text: {forbidden}"
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
            .any(|operation| matches!(operation.operator.as_str(), "S" | "s" | "B" | "B*")),
        "EMF 16-bit poly vector preview should render passive stroked or filled paths"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF 16-bit poly payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_setpolyfillmode_records_render_passively_without_payload_leakage() {
    let records = [
        emf_u32_record(19, 2),
        emf_create_brush_record(
            3,
            0,
            Color {
                red: 220,
                green: 180,
                blue: 120,
            },
            0,
        ),
        emf_select_object_record(3),
        emf_poly_record(3, &[(20, 10), (60, 70), (100, 10)]),
        emf_poly16_record(86, &[(100, 20), (140, 70), (200, 20)]),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF polyfill vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(image.vector_commands.iter().all(|command| matches!(
        command,
        StaticImageVectorCommand::Polygon {
            fill_rule: StaticImageVectorFillRule::Winding,
            fill_color: Some(Color {
                red: 220,
                green: 180,
                blue: 120
            }),
            ..
        }
    )));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF polyfill payload/control leaked to normalized text: {forbidden}"
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
            .any(|operation| matches!(operation.operator.as_str(), "f" | "B")),
        "EMF winding polyfill should render passive nonzero fill operations"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF polyfill payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_exttextouta_records_render_passively_without_payload_leakage() {
    let records = [
        emf_u32_record(24, 0x0066_4422),
        emf_u32_record(22, 0x0000_0002),
        emf_exttextouta_record(30, 18, b"Ansi\x97Text", 0, None, false),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF ANSI text vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("Ansi"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::Text {
            x: 30.0,
            y: 18.0,
            text,
            color: Some(Color {
                red: 34,
                green: 68,
                blue: 102
            }),
            horizontal_align: StaticImageTextHorizontalAlign::Right,
            ..
        } if text == "Ansi\u{2014}Text"
    ));
    for forbidden in ["emfblip", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF ANSI text payload/control leaked to normalized text: {forbidden}"
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
        rendered_text.contains("Ansi"),
        "EMF EXTTEXTOUTA should render through passive PDF text operations"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "rg"),
        "EMF ANSI text color should render as passive PDF fill color"
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
            "EMF ANSI text payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn strict_spec_emf_setpixelv_records_render_passively_without_payload_leakage() {
    let records = [
        emf_setpixelv_record(
            10,
            20,
            Color {
                red: 20,
                green: 40,
                blue: 60,
            },
        ),
        emf_setpixelv_record(
            200,
            100,
            Color {
                red: 220,
                green: 180,
                blue: 120,
            },
        ),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF SETPIXELV vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 10.0,
            top: 20.0,
            right: 11.0,
            bottom: 21.0,
            stroke_color: None,
            fill_color: Some(Color {
                red: 20,
                green: 40,
                blue: 60
            }),
            ..
        }
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF SETPIXELV payload/control leaked to normalized text: {forbidden}"
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
        "EMF SETPIXELV should render as passive PDF rectangle paths"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "f*" | "B" | "B*")),
        "EMF SETPIXELV should render as passive PDF fills"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF SETPIXELV payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn word_compatible_emf_without_device_metrics_is_omitted_without_payload_leakage() {
    let mut emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[emf_setpixelv_record(
            10,
            20,
            Color {
                red: 20,
                green: 40,
                blue: 60,
            },
        )],
    );
    for offset in [72, 76, 80, 84] {
        write_test_le_i32(&mut emf, offset, 0);
    }
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {}}} after\\par}}",
        bytes_to_hex(&emf)
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();

    assert!(collect_text(&parsed.document).contains("before"));
    assert!(collect_text(&parsed.document).contains("after"));
    assert!(
        !parsed
            .document
            .blocks
            .iter()
            .any(|block| matches!(block, Block::Image(_)))
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("EMF picture with missing reference-device metrics suppressed")
    }));

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let pdf = String::from_utf8_lossy(&output.pdf);
    for forbidden in ["emfblip", "20454d46", "/Subtype /Image", "/EmbeddedFile"] {
        assert!(
            !pdf.contains(forbidden),
            "malformed EMF payload leaked: {forbidden}"
        );
    }
}

#[test]
fn malformed_emf_setpixelv_becomes_passive_placeholder() {
    let mut record = vec![0; 16];
    write_test_le_u32(&mut record, 0, 15);
    write_test_le_u32(&mut record, 4, 16);
    write_test_le_i32(&mut record, 8, 10);
    write_test_le_i32(&mut record, 12, 20);
    let records = [record];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {}}} after\\par}}",
        bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
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
        .expect("malformed EMF SETPIXELV placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn emf_fillrgn_records_render_passively_without_payload_leakage() {
    let records = [
        emf_create_brush_record(
            3,
            0,
            Color {
                red: 40,
                green: 120,
                blue: 200,
            },
            0,
        ),
        emf_fillrgn_record(3, &[(20, 10, 90, 50), (100, 20, 140, 70)]),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF FILLRGN vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 20.0,
            top: 10.0,
            right: 90.0,
            bottom: 50.0,
            stroke_color: None,
            fill_color: Some(Color {
                red: 40,
                green: 120,
                blue: 200
            }),
            ..
        }
    ));
    for forbidden in [
        "emfblip",
        "FILLRGN",
        "RGNDATA",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF FILLRGN payload/control leaked to normalized text: {forbidden}"
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
        "EMF FILLRGN should render passive PDF rectangle paths"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "B")),
        "EMF FILLRGN should render passive PDF fills"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF FILLRGN payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_framergn_records_render_passively_without_payload_leakage() {
    let records = [
        emf_create_brush_record(
            3,
            0,
            Color {
                red: 80,
                green: 20,
                blue: 210,
            },
            0,
        ),
        emf_framergn_record(3, 6, 4, &[(20, 10, 90, 50)]),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF FRAMERGN vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 4);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 20.0,
            top: 10.0,
            right: 90.0,
            bottom: 14.0,
            stroke_color: None,
            fill_color: Some(Color {
                red: 80,
                green: 20,
                blue: 210
            }),
            ..
        }
    ));
    for forbidden in [
        "emfblip",
        "FRAMERGN",
        "RGNDATA",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF FRAMERGN payload/control leaked to normalized text: {forbidden}"
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
            >= 4,
        "EMF FRAMERGN should render passive PDF border rectangles"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "B")),
        "EMF FRAMERGN should render passive PDF fills"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF FRAMERGN payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_zero_frame_framergn_is_noop_without_payload_leakage() {
    let mut frame = emf_framergn_record(0xFFFF_FFFE, 0, 4, &[(20, 10, 90, 50)]);
    frame.extend_from_slice(b"ZERO-FRAME-EMF-FRAMERGN /JavaScript /EmbeddedFile /Launch");
    frame.resize(frame.len().next_multiple_of(4), 0);
    let frame_len = frame.len() as u32;
    write_test_le_u32(&mut frame, 4, frame_len);
    write_test_le_u32(&mut frame, 24, 31);
    let records = [emf_rect_record(43, 0, 0, 80, 40), frame];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF zero-frame FRAMERGN vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in [
        "emfblip",
        "FRAMERGN",
        "RGNDATA",
        "ZERO-FRAME-EMF-FRAMERGN",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "zero-frame EMF FRAMERGN payload/control leaked to normalized text: {forbidden}"
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
            >= 1,
        "preexisting paint before zero-frame FRAMERGN should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"ZERO-FRAME-EMF-FRAMERGN",
        b"FRAMERGN",
        b"RGNDATA",
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
            "zero-frame EMF FRAMERGN payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_empty_paint_regions_are_noops_without_payload_leakage() {
    let mut fill = emf_fillrgn_record(0xFFFF_FFFE, &[]);
    fill.extend_from_slice(b"EMPTY-EMF-FILLRGN /JavaScript");
    fill.resize(fill.len().next_multiple_of(4), 0);
    let fill_len = fill.len() as u32;
    write_test_le_u32(&mut fill, 4, fill_len);

    let mut frame = emf_framergn_record(0xFFFF_FFFE, 6, 4, &[]);
    frame.extend_from_slice(b"EMPTY-EMF-FRAMERGN /EmbeddedFile");
    frame.resize(frame.len().next_multiple_of(4), 0);
    let frame_len = frame.len() as u32;
    write_test_le_u32(&mut frame, 4, frame_len);

    let mut paint = emf_paintrgn_record(&[]);
    paint.extend_from_slice(b"EMPTY-EMF-PAINTRGN /Launch");
    paint.resize(paint.len().next_multiple_of(4), 0);
    let paint_len = paint.len() as u32;
    write_test_le_u32(&mut paint, 4, paint_len);

    let mut invert = emf_invertrgn_record(&[]);
    invert.extend_from_slice(b"EMPTY-EMF-INVERTRGN /OpenAction");
    invert.resize(invert.len().next_multiple_of(4), 0);
    let invert_len = invert.len() as u32;
    write_test_le_u32(&mut invert, 4, invert_len);

    let records = [
        emf_rect_record(43, 0, 0, 80, 40),
        fill,
        frame,
        paint,
        invert,
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF empty paint-region vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in [
        "emfblip",
        "FILLRGN",
        "FRAMERGN",
        "PAINTRGN",
        "INVERTRGN",
        "RGNDATA",
        "EMPTY-EMF-FILLRGN",
        "EMPTY-EMF-FRAMERGN",
        "EMPTY-EMF-PAINTRGN",
        "EMPTY-EMF-INVERTRGN",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
        "OpenAction",
    ] {
        assert!(
            !text.contains(forbidden),
            "empty EMF paint-region payload/control leaked to normalized text: {forbidden}"
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
            >= 1,
        "preexisting paint before empty EMF paint-region no-ops should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMPTY-EMF-FILLRGN",
        b"EMPTY-EMF-FRAMERGN",
        b"EMPTY-EMF-PAINTRGN",
        b"EMPTY-EMF-INVERTRGN",
        b"FILLRGN",
        b"FRAMERGN",
        b"PAINTRGN",
        b"INVERTRGN",
        b"RGNDATA",
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
            "empty EMF paint-region payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn invalid_emf_extselectcliprgn_mode_counts_as_partial_without_payload_leakage() {
    let mut clip = emf_extselectcliprgn_record(99, &[(20, 10, 80, 50)]);
    clip.extend_from_slice(b"INVALID-EMF-EXTSELECTCLIPRGN /JavaScript /EmbeddedFile /Launch");
    clip.resize(clip.len().next_multiple_of(4), 0);
    let clip_len = clip.len() as u32;
    write_test_le_u32(&mut clip, 4, clip_len);
    let records = [clip, emf_rect_record(43, 20, 10, 80, 50)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("invalid EMF EXTSELECTCLIPRGN mode partial vector preview");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(
        image
            .vector_commands
            .iter()
            .any(|command| matches!(command, StaticImageVectorCommand::Rectangle { .. })),
        "safe drawing after invalid EMF EXTSELECTCLIPRGN mode should stay visible"
    );
    for forbidden in [
        "emfblip",
        "EXTSELECTCLIPRGN",
        "INVALID-EMF-EXTSELECTCLIPRGN",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "invalid EMF EXTSELECTCLIPRGN mode payload/control leaked to normalized text: {forbidden}"
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
        "invalid EMF EXTSELECTCLIPRGN mode should produce skipped-record diagnostics: {:?}",
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
        "safe drawing after invalid EMF EXTSELECTCLIPRGN mode should render passively"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EXTSELECTCLIPRGN",
        b"INVALID-EMF-EXTSELECTCLIPRGN",
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
            "invalid EMF EXTSELECTCLIPRGN mode payload/control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_extselectcliprgn_records_render_passively_without_payload_leakage() {
    let records = [
        emf_extselectcliprgn_record(1, &[(20, 10, 90, 50), (100, 20, 140, 70)]),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF EXTSELECTCLIPRGN vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::ClipPath {
            start: (20.0, 10.0),
            ref segments,
            fill_rule: StaticImageVectorFillRule::Winding,
            ..
        } if segments.len() == 9
    ));
    for forbidden in [
        "emfblip",
        "EXTSELECTCLIPRGN",
        "RGNDATA",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF EXTSELECTCLIPRGN payload/control leaked to normalized text: {forbidden}"
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
        "EMF EXTSELECTCLIPRGN should render a passive PDF clipping path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "EMF EXTSELECTCLIPRGN clipped drawing should still render passive rectangles"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF EXTSELECTCLIPRGN payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_initial_rgn_or_clips_are_passive_noops_without_payload_leakage() {
    let records = [
        emf_extselectcliprgn_record(2, &[(20, 10, 90, 50), (100, 20, 140, 70)]),
        emf_unknown_record(59),
        emf_point_record(27, 20, 20),
        emf_poly_record(6, &[(80, 20), (50, 60)]),
        emf_unknown_record(60),
        emf_u32_record(67, 2),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex} 2f4a617661536372697074 2f456d62656464656446696c65}} after\\par}}"
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
        .expect("passive EMF RGN_OR vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in [
        "emfblip",
        "JavaScript",
        "EmbeddedFile",
        "EXTSELECTCLIPRGN",
        "RGNDATA",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF RGN_OR payload/control leaked to normalized text: {forbidden}"
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
        "unclipped EMF drawing after initial RGN_OR should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
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
            "EMF RGN_OR payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_empty_extselectcliprgn_combine_noops_without_payload_leakage() {
    let mut or = emf_extselectcliprgn_record(2, &[]);
    or.extend_from_slice(b"EMPTY-EMF-EXTSELECTCLIPRGN-OR /JavaScript");
    or.resize(or.len().next_multiple_of(4), 0);
    let or_len = or.len() as u32;
    write_test_le_u32(&mut or, 4, or_len);

    let mut diff = emf_extselectcliprgn_record(4, &[]);
    diff.extend_from_slice(b"EMPTY-EMF-EXTSELECTCLIPRGN-DIFF /EmbeddedFile");
    diff.resize(diff.len().next_multiple_of(4), 0);
    let diff_len = diff.len() as u32;
    write_test_le_u32(&mut diff, 4, diff_len);

    let mut xor = emf_extselectcliprgn_record(3, &[]);
    xor.extend_from_slice(b"EMPTY-EMF-EXTSELECTCLIPRGN-XOR /Launch");
    xor.resize(xor.len().next_multiple_of(4), 0);
    let xor_len = xor.len() as u32;
    write_test_le_u32(&mut xor, 4, xor_len);

    let records = [
        emf_rect_record(30, 10, 10, 120, 70),
        or,
        diff,
        xor,
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF empty EXTSELECTCLIPRGN vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert_eq!(
        image.vector_commands[0],
        StaticImageVectorCommand::ClipRect {
            left: 10.0,
            top: 10.0,
            right: 120.0,
            bottom: 70.0,
        }
    );
    for forbidden in [
        "emfblip",
        "EXTSELECTCLIPRGN",
        "RGNDATA",
        "EMPTY-EMF-EXTSELECTCLIPRGN-OR",
        "EMPTY-EMF-EXTSELECTCLIPRGN-DIFF",
        "EMPTY-EMF-EXTSELECTCLIPRGN-XOR",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "empty EMF EXTSELECTCLIPRGN payload/control leaked to normalized text: {forbidden}"
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
        "preexisting clip before empty EMF EXTSELECTCLIPRGN no-ops should still render"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after empty EMF EXTSELECTCLIPRGN no-ops should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMPTY-EMF-EXTSELECTCLIPRGN-OR",
        b"EMPTY-EMF-EXTSELECTCLIPRGN-DIFF",
        b"EMPTY-EMF-EXTSELECTCLIPRGN-XOR",
        b"EXTSELECTCLIPRGN",
        b"RGNDATA",
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
            "empty EMF EXTSELECTCLIPRGN payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_empty_extselectcliprgn_and_copy_clip_without_payload_leakage() {
    let mut and = emf_extselectcliprgn_record(1, &[]);
    and.extend_from_slice(b"EMPTY-EMF-EXTSELECTCLIPRGN-AND /JavaScript");
    and.resize(and.len().next_multiple_of(4), 0);
    let and_len = and.len() as u32;
    write_test_le_u32(&mut and, 4, and_len);

    let mut copy = emf_extselectcliprgn_record(5, &[]);
    copy.extend_from_slice(b"EMPTY-EMF-EXTSELECTCLIPRGN-COPY /EmbeddedFile");
    copy.resize(copy.len().next_multiple_of(4), 0);
    let copy_len = copy.len() as u32;
    write_test_le_u32(&mut copy, 4, copy_len);

    let records = [and, copy, emf_rect_record(43, 0, 0, 160, 80)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF empty AND/COPY EXTSELECTCLIPRGN vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert_eq!(
        image.vector_commands[0],
        StaticImageVectorCommand::ClipRect {
            left: 0.0,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
        }
    );
    for forbidden in [
        "emfblip",
        "EXTSELECTCLIPRGN",
        "RGNDATA",
        "EMPTY-EMF-EXTSELECTCLIPRGN-AND",
        "EMPTY-EMF-EXTSELECTCLIPRGN-COPY",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "empty EMF EXTSELECTCLIPRGN AND/COPY payload leaked to normalized text: {forbidden}"
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
        "empty EMF EXTSELECTCLIPRGN AND/COPY should emit a passive clip"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMPTY-EMF-EXTSELECTCLIPRGN-AND",
        b"EMPTY-EMF-EXTSELECTCLIPRGN-COPY",
        b"EXTSELECTCLIPRGN",
        b"RGNDATA",
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
            "empty EMF EXTSELECTCLIPRGN AND/COPY payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_contained_rgn_or_clips_are_passive_noops_without_payload_leakage() {
    let records = [
        emf_rect_record(30, 10, 10, 120, 70),
        emf_extselectcliprgn_record(2, &[(20, 20, 80, 50), (90, 25, 110, 60)]),
        emf_unknown_record(59),
        emf_point_record(27, 20, 20),
        emf_poly_record(6, &[(80, 20), (50, 60)]),
        emf_unknown_record(60),
        emf_u32_record(67, 2),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex} 2f4a617661536372697074 2f456d62656464656446696c65}} after\\par}}"
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
        .expect("passive contained EMF RGN_OR vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::ClipRect {
            left: 10.0,
            top: 10.0,
            right: 120.0,
            bottom: 70.0,
        }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in [
        "emfblip",
        "JavaScript",
        "EmbeddedFile",
        "EXTSELECTCLIPRGN",
        "SELECTCLIPPATH",
        "RGNDATA",
    ] {
        assert!(
            !text.contains(forbidden),
            "contained EMF RGN_OR payload/control leaked to normalized text: {forbidden}"
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
        "contained EMF RGN_OR should retain the prior passive PDF clipping path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "contained EMF RGN_OR drawing should still render passive rectangles"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
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
            "contained EMF RGN_OR payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_containing_rgn_or_clip_replaces_unpainted_clip_without_payload_leakage() {
    let records = [
        emf_rect_record(30, 40, 20, 100, 60),
        emf_extselectcliprgn_record(2, &[(10, 10, 130, 70)]),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex} 2f4a617661536372697074 2f456d62656464656446696c65}} after\\par}}"
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
        .expect("passive containing EMF RGN_OR vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::ClipRect {
            left: 10.0,
            top: 10.0,
            right: 130.0,
            bottom: 70.0,
        }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in [
        "emfblip",
        "JavaScript",
        "EmbeddedFile",
        "EXTSELECTCLIPRGN",
        "RGNDATA",
    ] {
        assert!(
            !text.contains(forbidden),
            "containing EMF RGN_OR payload/control leaked to normalized text: {forbidden}"
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
        "containing EMF RGN_OR should render the replacement passive PDF clip"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after containing EMF RGN_OR should still render passive rectangles"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
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
            "containing EMF RGN_OR payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_selectclippath_or_containing_rect_replaces_unpainted_clip_without_payload_leakage() {
    let records = [
        emf_rect_record(30, 40, 20, 100, 60),
        emf_unknown_record(59),
        emf_point_record(27, 10, 10),
        emf_poly_record(6, &[(130, 10), (130, 70), (10, 70), (10, 10)]),
        emf_unknown_record(60),
        emf_u32_record(67, 2),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex} 2f4a617661536372697074 2f456d62656464656446696c65}} after\\par}}"
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
        .expect("passive containing EMF SELECTCLIPPATH RGN_OR vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::ClipRect {
            left: 10.0,
            top: 10.0,
            right: 130.0,
            bottom: 70.0,
        }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in [
        "emfblip",
        "JavaScript",
        "EmbeddedFile",
        "SELECTCLIPPATH",
        "RGNDATA",
    ] {
        assert!(
            !text.contains(forbidden),
            "containing EMF SELECTCLIPPATH payload/control leaked to normalized text: {forbidden}"
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
        "containing EMF SELECTCLIPPATH RGN_OR should render the replacement passive PDF clip"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after containing EMF SELECTCLIPPATH RGN_OR should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
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
            "containing EMF SELECTCLIPPATH RGN_OR payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_initial_rgn_diff_clip_region_renders_passively_without_payload_leakage() {
    let records = [
        emf_extselectcliprgn_record(4, &[(20, 10, 90, 50), (100, 20, 140, 70)]),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex} 2f4a617661536372697074 2f456d62656464656446696c65}} after\\par}}"
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
        .expect("passive EMF RGN_DIFF vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::ClipPath {
            start: (0.0, 0.0),
            ref segments,
            fill_rule: StaticImageVectorFillRule::Alternate,
            ..
        } if segments.len() == 14
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in [
        "emfblip",
        "JavaScript",
        "EmbeddedFile",
        "EXTSELECTCLIPRGN",
        "RGNDATA",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF RGN_DIFF payload/control leaked to normalized text: {forbidden}"
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
        "initial RGN_DIFF should render a passive even-odd clipping path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after initial RGN_DIFF should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
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
            "EMF RGN_DIFF payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_rgn_diff_from_unpainted_clip_rect_renders_passively_without_payload_leakage() {
    let records = [
        emf_rect_record(30, 10, 10, 120, 70),
        emf_extselectcliprgn_record(4, &[(40, 20, 80, 50)]),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex} 2f4a617661536372697074 2f456d62656464656446696c65}} after\\par}}"
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
        .expect("passive EMF clipped RGN_DIFF vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::ClipPath {
            start: (10.0, 10.0),
            ref segments,
            fill_rule: StaticImageVectorFillRule::Alternate,
            ..
        } if segments.len() == 9
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in [
        "emfblip",
        "JavaScript",
        "EmbeddedFile",
        "EXTSELECTCLIPRGN",
        "RGNDATA",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF non-initial RGN_DIFF payload/control leaked to normalized text: {forbidden}"
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
        "non-initial RGN_DIFF should render a passive even-odd clipping path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after non-initial RGN_DIFF should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
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
            "EMF non-initial RGN_DIFF payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_rgn_xor_clip_regions_render_safe_subset_without_payload_leakage() {
    let initial_records = [
        emf_extselectcliprgn_record(3, &[(20, 10, 90, 50), (100, 20, 140, 70)]),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let active_records = [
        emf_rect_record(30, 10, 10, 120, 70),
        emf_extselectcliprgn_record(3, &[(40, 20, 80, 50)]),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let initial_emf = minimal_emf_with_records(160, 80, 2540, 1270, &initial_records);
    let active_emf = minimal_emf_with_records(160, 80, 2540, 1270, &active_records);
    let initial_emf_hex = bytes_to_hex(&initial_emf);
    let active_emf_hex = bytes_to_hex(&active_emf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {initial_emf_hex} 2f4a617661536372697074}} middle {{\\pict\\emfblip {active_emf_hex} 2f456d62656464656446696c65}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let images: Vec<_> = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .collect();

    assert!(text.contains("before"));
    assert!(text.contains("middle"));
    assert!(text.contains("after"));
    assert_eq!(images.len(), 2);
    assert_eq!(parsed.diagnostics.len(), 0);
    for image in &images {
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(image.vector_commands.len(), 2);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipPath {
                fill_rule: StaticImageVectorFillRule::Alternate,
                ..
            }
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }
    for forbidden in [
        "emfblip",
        "JavaScript",
        "EmbeddedFile",
        "EXTSELECTCLIPRGN",
        "RGNDATA",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF RGN_XOR payload/control leaked to normalized text: {forbidden}"
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
            .filter(|operation| operation.operator == "W*")
            .count()
            >= 2,
        "RGN_XOR safe subset should render passive even-odd clipping paths"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after RGN_XOR should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        initial_emf_hex.as_bytes(),
        active_emf_hex.as_bytes(),
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
            "EMF RGN_XOR payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_initial_selectclippath_diff_renders_passively_without_payload_leakage() {
    let records = [
        emf_unknown_record(59),
        emf_point_record(27, 20, 20),
        emf_poly_record(6, &[(80, 20), (50, 60)]),
        emf_unknown_record(60),
        emf_u32_record(67, 4),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex} 2f4a617661536372697074 2f456d62656464656446696c65}} after\\par}}"
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
        .expect("passive EMF SELECTCLIPPATH RGN_DIFF vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert!(
        image.vector_commands.iter().any(|command| matches!(
            command,
            StaticImageVectorCommand::ClipPath {
                start: (0.0, 0.0),
                fill_rule: StaticImageVectorFillRule::Alternate,
                ..
            }
        )),
        "expected a passive even-odd exclude clip path"
    );
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in ["emfblip", "JavaScript", "EmbeddedFile", "SELECTCLIPPATH"] {
        assert!(
            !text.contains(forbidden),
            "EMF SELECTCLIPPATH RGN_DIFF payload/control leaked to normalized text: {forbidden}"
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
        "initial SELECTCLIPPATH RGN_DIFF should render a passive even-odd clipping path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after SELECTCLIPPATH RGN_DIFF should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
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
            "EMF SELECTCLIPPATH RGN_DIFF payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_initial_selectclippath_xor_renders_passively_without_payload_leakage() {
    let records = [
        emf_unknown_record(59),
        emf_point_record(27, 20, 20),
        emf_poly_record(6, &[(80, 20), (50, 60)]),
        emf_unknown_record(60),
        emf_u32_record(67, 3),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex} 2f4a617661536372697074 2f456d62656464656446696c65}} after\\par}}"
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
        .expect("passive EMF SELECTCLIPPATH RGN_XOR vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert!(
        image.vector_commands.iter().any(|command| matches!(
            command,
            StaticImageVectorCommand::ClipPath {
                start: (0.0, 0.0),
                fill_rule: StaticImageVectorFillRule::Alternate,
                ..
            }
        )),
        "expected a passive even-odd exclude clip path"
    );
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in ["emfblip", "JavaScript", "EmbeddedFile", "SELECTCLIPPATH"] {
        assert!(
            !text.contains(forbidden),
            "EMF SELECTCLIPPATH RGN_XOR payload/control leaked to normalized text: {forbidden}"
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
        "initial SELECTCLIPPATH RGN_XOR should render a passive even-odd clipping path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after SELECTCLIPPATH RGN_XOR should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
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
            "EMF SELECTCLIPPATH RGN_XOR payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_selectclippath_diff_from_unpainted_clip_rect_renders_passively_without_payload_leakage() {
    let records = [
        emf_rect_record(30, 10, 10, 120, 70),
        emf_unknown_record(59),
        emf_point_record(27, 40, 20),
        emf_poly_record(6, &[(80, 20), (50, 60)]),
        emf_unknown_record(60),
        emf_u32_record(67, 4),
        emf_rect_record(43, 0, 0, 160, 80),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex} 2f4a617661536372697074 2f456d62656464656446696c65}} after\\par}}"
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
        .expect("passive EMF SELECTCLIPPATH RGN_DIFF clipped vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert!(
        image.vector_commands.iter().any(|command| matches!(
            command,
            StaticImageVectorCommand::ClipPath {
                start: (10.0, 10.0),
                fill_rule: StaticImageVectorFillRule::Alternate,
                ..
            }
        )),
        "expected a passive even-odd exclude clip path bounded to the prior clip rect"
    );
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in ["emfblip", "JavaScript", "EmbeddedFile", "SELECTCLIPPATH"] {
        assert!(
            !text.contains(forbidden),
            "EMF active SELECTCLIPPATH RGN_DIFF payload/control leaked to normalized text: {forbidden}"
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
        "active SELECTCLIPPATH RGN_DIFF should render a passive even-odd clipping path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after active SELECTCLIPPATH RGN_DIFF should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
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
            "EMF active SELECTCLIPPATH RGN_DIFF payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_paintrgn_records_render_passively_without_payload_leakage() {
    let records = [
        emf_create_brush_record(
            3,
            0,
            Color {
                red: 220,
                green: 90,
                blue: 30,
            },
            0,
        ),
        emf_select_object_record(3),
        emf_paintrgn_record(&[(15, 12, 70, 40), (80, 20, 130, 65)]),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF PAINTRGN vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 15.0,
            top: 12.0,
            right: 70.0,
            bottom: 40.0,
            stroke_color: None,
            fill_color: Some(Color {
                red: 220,
                green: 90,
                blue: 30
            }),
            ..
        }
    ));
    for forbidden in [
        "emfblip",
        "PAINTRGN",
        "RGNDATA",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF PAINTRGN payload/control leaked to normalized text: {forbidden}"
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
        "EMF PAINTRGN should render passive PDF rectangle paths"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "B")),
        "EMF PAINTRGN should render passive PDF fills"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF PAINTRGN payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_initial_invertrgn_records_render_passively_without_payload_leakage() {
    let records = [emf_invertrgn_record(&[(15, 12, 70, 40), (80, 20, 130, 65)])];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF INVERTRGN vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 15.0,
            top: 12.0,
            right: 70.0,
            bottom: 40.0,
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
            left: 80.0,
            top: 20.0,
            right: 130.0,
            bottom: 65.0,
            stroke_color: None,
            fill_color: Some(Color {
                red: 0,
                green: 0,
                blue: 0
            }),
            ..
        }
    ));
    for forbidden in [
        "emfblip",
        "INVERTRGN",
        "RGNDATA",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF INVERTRGN payload/control leaked to normalized text: {forbidden}"
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
        "initial EMF INVERTRGN should render passive PDF rectangle paths"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "B")),
        "initial EMF INVERTRGN should render passive PDF fills"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF INVERTRGN payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_clipped_invertrgn_records_render_passively_without_payload_leakage() {
    let records = [
        emf_rect_record(30, 20, 10, 100, 60),
        emf_invertrgn_record(&[(15, 12, 70, 40), (80, 20, 130, 65)]),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive clipped EMF INVERTRGN vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 3);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::ClipRect {
            left: 20.0,
            top: 10.0,
            right: 100.0,
            bottom: 60.0,
        }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Rectangle {
            fill_color: Some(Color {
                red: 0,
                green: 0,
                blue: 0
            }),
            ..
        }
    ));
    for forbidden in [
        "emfblip",
        "INVERTRGN",
        "RGNDATA",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "clipped EMF INVERTRGN payload/control leaked to normalized text: {forbidden}"
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
        "clipped EMF INVERTRGN should render a passive PDF clipping path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "clipped EMF INVERTRGN should render passive PDF rectangle paths"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "B")),
        "clipped EMF INVERTRGN should render passive PDF fills"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "clipped EMF INVERTRGN payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_same_bounds_invertrgn_after_solid_paint_reduces_without_payload_leakage() {
    let payload = b"SAME-BOUNDS-EMF-INVERTRGN /JavaScript";
    let records = [
        emf_create_brush_record(
            3,
            0,
            Color {
                red: 100,
                green: 30,
                blue: 200,
            },
            0,
        ),
        emf_select_object_record(3),
        emf_bitblt_record(20, 15, 60, 30, 0x00f0_0021, b"EMF-INVERTRGN-FILL"),
        {
            let mut record = emf_invertrgn_record(&[(20, 15, 80, 45)]);
            record.extend_from_slice(payload);
            record.resize(record.len().next_multiple_of(4), 0);
            let record_len = record.len() as u32;
            write_test_le_u32(&mut record, 4, record_len);
            record
        },
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("partial passive EMF INVERTRGN vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            fill_color: Some(Color {
                red: 155,
                green: 225,
                blue: 55
            }),
            ..
        }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "INVERTRGN",
        "RGNDATA",
        "SAME-BOUNDS-EMF-INVERTRGN",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "same-bounds EMF INVERTRGN payload/control leaked to normalized text: {forbidden}"
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
        "same-bounds EMF INVERTRGN should render passive rectangle path"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        payload.as_slice(),
        b" EMF",
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
            "same-bounds EMF INVERTRGN payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_multi_rect_same_bounds_invertrgn_reduces_without_payload_leakage() {
    let payload = b"MULTI-SAME-BOUNDS-EMF-INVERTRGN /OpenAction";
    let records = [
        emf_create_brush_record(
            3,
            0,
            Color {
                red: 40,
                green: 120,
                blue: 210,
            },
            0,
        ),
        emf_select_object_record(3),
        emf_fillrgn_record(3, &[(20, 15, 70, 35), (90, 20, 140, 50)]),
        {
            let mut record = emf_invertrgn_record(&[(20, 15, 70, 35), (90, 20, 140, 50)]);
            record.extend_from_slice(payload);
            record.resize(record.len().next_multiple_of(4), 0);
            let record_len = record.len() as u32;
            write_test_le_u32(&mut record, 4, record_len);
            record
        },
    ];
    let emf = minimal_emf_with_records(180, 90, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("partial passive multi-rect EMF INVERTRGN vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(image.vector_commands.iter().all(|command| matches!(
        command,
        StaticImageVectorCommand::Rectangle {
            stroke_color: None,
            fill_color: Some(Color {
                red: 215,
                green: 135,
                blue: 45
            }),
            ..
        }
    )));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "INVERTRGN",
        "RGNDATA",
        "MULTI-SAME-BOUNDS-EMF-INVERTRGN",
        "OpenAction",
    ] {
        assert!(
            !text.contains(forbidden),
            "multi-rect same-bounds EMF INVERTRGN payload/control leaked to normalized text: {forbidden}"
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
        "multi-rect same-bounds EMF INVERTRGN should render passive rectangle paths"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        payload.as_slice(),
        b" EMF",
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
            "multi-rect same-bounds EMF INVERTRGN payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_backdrop_dependent_invertrgn_is_skipped_without_payload_leakage() {
    let records = [
        emf_rect_record(43, 0, 0, 30, 30),
        emf_invertrgn_record(&[(15, 12, 70, 40)]),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("partial passive EMF INVERTRGN vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("1 unsupported record(s) skipped")
    }));
    for forbidden in [
        "emfblip",
        "INVERTRGN",
        "RGNDATA",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "backdrop-dependent EMF INVERTRGN payload/control leaked to normalized text: {forbidden}"
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
        "safe painted EMF rectangle should still render after skipping INVERTRGN"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "backdrop-dependent EMF INVERTRGN payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_patcopy_bitblt_records_render_passively_without_payload_leakage() {
    let payload = b"BITBLT-SOURCE-PAYLOAD";
    let records = [
        emf_create_brush_record(
            3,
            0,
            Color {
                red: 120,
                green: 80,
                blue: 40,
            },
            0,
        ),
        emf_select_object_record(3),
        emf_bitblt_record(10, 20, 80, 40, 0x00f0_0021, payload),
        emf_bitblt_record(30, 25, 40, 20, 0x00cc_0020, b"UNSUPPORTED-SRCCOPY-PAYLOAD"),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF BITBLT vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("BITBLT-SOURCE-PAYLOAD"));
    assert!(!text.contains("UNSUPPORTED-SRCCOPY-PAYLOAD"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 10.0,
            top: 20.0,
            right: 90.0,
            bottom: 60.0,
            stroke_color: None,
            fill_color: Some(Color {
                red: 120,
                green: 80,
                blue: 40
            }),
            ..
        }
    ));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("1 unsupported record(s) skipped")
    }));
    for forbidden in ["emfblip", "BITBLT", "SRCCOPY", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF BITBLT payload/control leaked to normalized text: {forbidden}"
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
        "EMF PATCOPY BITBLT should render as a passive PDF rectangle path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "f*" | "B" | "B*")),
        "EMF PATCOPY BITBLT should render as a passive PDF fill"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        payload.as_slice(),
        b"UNSUPPORTED-SRCCOPY-PAYLOAD",
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
            "EMF BITBLT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_patcopy_stretchblt_records_render_passively_without_payload_leakage() {
    let payload = b"STRETCHBLT-SOURCE-PAYLOAD";
    let records = [
        emf_create_brush_record(
            3,
            0,
            Color {
                red: 30,
                green: 150,
                blue: 200,
            },
            0,
        ),
        emf_select_object_record(3),
        emf_stretchblt_record(15, 25, 70, 35, 0x00f0_0021, payload),
        emf_stretchblt_record(35, 30, 45, 25, 0x00cc_0020, b"UNSUPPORTED-STRETCH-SRCCOPY"),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF STRETCHBLT vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("STRETCHBLT-SOURCE-PAYLOAD"));
    assert!(!text.contains("UNSUPPORTED-STRETCH-SRCCOPY"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 15.0,
            top: 25.0,
            right: 85.0,
            bottom: 60.0,
            stroke_color: None,
            fill_color: Some(Color {
                red: 30,
                green: 150,
                blue: 200
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
        "emfblip",
        "STRETCHBLT",
        "SRCCOPY",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF STRETCHBLT payload/control leaked to normalized text: {forbidden}"
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
        "EMF PATCOPY STRETCHBLT should render as a passive PDF rectangle path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "f*" | "B" | "B*")),
        "EMF PATCOPY STRETCHBLT should render as a passive PDF fill"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        payload.as_slice(),
        b"UNSUPPORTED-STRETCH-SRCCOPY",
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
            "EMF STRETCHBLT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_zero_alpha_alphablend_is_noop_without_payload_leakage() {
    let mut hidden_dib = b"ZERO-ALPHA-ALPHABLEND-DIB /JavaScript /EmbeddedFile /Launch".to_vec();
    hidden_dib.resize(96, 0x41);
    let records = [
        emf_rect_record(43, 0, 0, 80, 40),
        emf_alphablend_dib_record(18, 24, 48, 28, 0, 0, &hidden_dib),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF ALPHABLEND no-op vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in [
        "emfblip",
        "ZERO-ALPHA-ALPHABLEND-DIB",
        "ALPHABLEND",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "zero-alpha ALPHABLEND payload/control leaked to normalized text: {forbidden}"
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
            >= 1,
        "preexisting paint before zero-alpha ALPHABLEND should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"ZERO-ALPHA-ALPHABLEND-DIB",
        b"ALPHABLEND",
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
            "zero-alpha ALPHABLEND payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_transparentblt_keyed_color_renders_with_passive_alpha_mask_without_payload_leakage() {
    let mut dib = minimal_24bit_dib_with_dimensions(2, 1);
    dib.extend_from_slice(b"TRAILING-EMF-TRANSPARENTBLT /JavaScript");
    let jpeg = minimal_jpeg_with_dimensions(2, 1);
    let mut jpeg_payload = jpeg.clone();
    jpeg_payload.extend_from_slice(b"TRAILING-EMF-TRANSPARENTBLT-JPEG /EmbeddedFile");
    let mut jpeg_dib = minimal_compressed_dib_with_payload(2, 1, 4, &jpeg_payload);
    jpeg_dib.extend_from_slice(b"TRAILING-EMF-TRANSPARENTBLT-JPEG-DIB /Launch");
    let records = [
        emf_transparentblt_dib_record(
            18,
            26,
            48,
            28,
            Color {
                red: 12,
                green: 34,
                blue: 56,
            },
            &dib,
        ),
        emf_transparentblt_dib_record(
            92,
            32,
            48,
            28,
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
            &jpeg_dib,
        ),
        emf_transparentblt_dib_record(
            70,
            30,
            48,
            28,
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
            &dib,
        ),
        emf_transparentblt_dib_record(
            100,
            34,
            48,
            28,
            Color {
                red: 12,
                green: 34,
                blue: 56,
            },
            &minimal_compressed_dib_with_payload(
                2,
                1,
                5,
                &minimal_rgb_png(&[[255, 0, 0], [0, 255, 0]]),
            ),
        ),
        emf_transparentblt_dib_record(
            124,
            38,
            48,
            28,
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
            &minimal_compressed_dib_with_payload(
                2,
                1,
                5,
                &minimal_rgb_png(&[[255, 0, 0], [0, 255, 0]]),
            ),
        ),
        emf_transparentblt_dib_record(
            148,
            42,
            48,
            28,
            Color {
                red: 0,
                green: 255,
                blue: 0,
            },
            &minimal_compressed_dib_with_payload(
                2,
                1,
                5,
                &minimal_rgba_png(&[[255, 0, 0, 128], [0, 255, 0, 255]]),
            ),
        ),
        emf_transparentblt_dib_record(
            152,
            46,
            48,
            28,
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
            &minimal_compressed_dib_with_payload(
                2,
                1,
                5,
                &minimal_indexed_png_with_samples(&[0, 1]),
            ),
        ),
        emf_transparentblt_dib_record(
            156,
            50,
            48,
            28,
            Color {
                red: 128,
                green: 128,
                blue: 128,
            },
            &minimal_compressed_dib_with_payload(2, 1, 5, &minimal_grayscale_png(&[128, 255])),
        ),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF TRANSPARENTBLT vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 8);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::RasterImage {
            left: 18.0,
            top: 26.0,
            right: 66.0,
            bottom: 54.0,
            image,
        } if image.format == ImageFormat::Rgb8
            && image.width_px == 2
            && image.height_px == 1
            && image.bytes == vec![255, 0, 0, 0, 255, 0]
            && image.alpha_mask.is_none()
    ));
    assert!(matches!(
        &image.vector_commands[1],
        StaticImageVectorCommand::RasterImage {
            left: 92.0,
            top: 32.0,
            right: 140.0,
            bottom: 60.0,
            image,
        } if image.format == ImageFormat::Jpeg
            && image.width_px == 2
            && image.height_px == 1
            && image.bytes == jpeg
            && image.alpha_mask.is_none()
    ));
    let alpha_mask = match &image.vector_commands[2] {
        StaticImageVectorCommand::RasterImage { image, .. } => image
            .alpha_mask
            .as_ref()
            .expect("transparent color key alpha mask"),
        _ => panic!("expected keyed transparent raster image"),
    };
    assert_eq!(
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&alpha_mask.bytes, 3).unwrap(),
        vec![0, 0, 255]
    );
    assert!(matches!(
        &image.vector_commands[3],
        StaticImageVectorCommand::RasterImage {
            left: 100.0,
            top: 34.0,
            right: 148.0,
            bottom: 62.0,
            image,
        } if image.format == ImageFormat::Png
            && image.width_px == 2
            && image.height_px == 1
            && image.alpha_mask.is_none()
    ));
    let compressed_png_alpha_mask = match &image.vector_commands[4] {
        StaticImageVectorCommand::RasterImage { image, .. } => image
            .alpha_mask
            .as_ref()
            .expect("compressed PNG transparent color key alpha mask"),
        _ => panic!("expected compressed PNG keyed transparent raster image"),
    };
    assert_eq!(
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(
            &compressed_png_alpha_mask.bytes,
            3,
        )
        .unwrap(),
        vec![0, 0, 255]
    );
    let rgba_png_alpha_mask = match &image.vector_commands[5] {
        StaticImageVectorCommand::RasterImage { image, .. } => image
            .alpha_mask
            .as_ref()
            .expect("RGBA PNG transparent color key alpha mask"),
        _ => panic!("expected RGBA PNG keyed transparent raster image"),
    };
    assert_eq!(
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&rgba_png_alpha_mask.bytes, 3)
            .unwrap(),
        vec![0, 128, 0]
    );
    let indexed_png_alpha_mask = match &image.vector_commands[6] {
        StaticImageVectorCommand::RasterImage { image, .. } => image
            .alpha_mask
            .as_ref()
            .expect("indexed PNG transparent color key alpha mask"),
        _ => panic!("expected indexed PNG keyed transparent raster image"),
    };
    assert_eq!(
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&indexed_png_alpha_mask.bytes, 3,)
            .unwrap(),
        vec![0, 0, 255]
    );
    let grayscale_png_alpha_mask = match &image.vector_commands[7] {
        StaticImageVectorCommand::RasterImage { image, .. } => image
            .alpha_mask
            .as_ref()
            .expect("grayscale PNG transparent color key alpha mask"),
        _ => panic!("expected grayscale PNG keyed transparent raster image"),
    };
    assert_eq!(
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(
            &grayscale_png_alpha_mask.bytes,
            3,
        )
        .unwrap(),
        vec![0, 0, 255]
    );
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "TRANSPARENTBLT",
        "TRAILING-EMF-TRANSPARENTBLT",
        "TRAILING-EMF-TRANSPARENTBLT-JPEG",
        "TRAILING-EMF-TRANSPARENTBLT-JPEG-DIB",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF TRANSPARENTBLT payload/control leaked to normalized text: {forbidden}"
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
            .filter(|operation| operation.operator == "Do")
            .count()
            >= 8,
        "keyed EMF TRANSPARENTBLT DIBs should render as passive image XObjects"
    );
    assert!(
        output
            .pdf
            .windows(b"/SMask".len())
            .any(|window| window == b"/SMask"),
        "keyed EMF TRANSPARENTBLT should render transparency as a passive PDF soft mask"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"TRANSPARENTBLT",
        b"TRAILING-EMF-TRANSPARENTBLT",
        b"TRAILING-EMF-TRANSPARENTBLT-JPEG",
        b"TRAILING-EMF-TRANSPARENTBLT-JPEG-DIB",
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
            "EMF TRANSPARENTBLT DIB payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_move_to_and_line_to_records_render_passively_without_payload_leakage() {
    let records = [
        emf_point_record(27, 10, 10),
        emf_point_record(54, 70, 40),
        emf_point_record(54, 70, 40),
        emf_point_record(54, 200, 100),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Line {
            x1: 10.0,
            y1: 10.0,
            x2: 70.0,
            y2: 40.0,
            stroke_color: Some(_),
            stroke_width: 1.0,
            stroke_style: BorderStyle::Single,
        }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Line {
            x1: 70.0,
            y1: 40.0,
            x2: 160.0,
            y2: 80.0,
            stroke_color: Some(_),
            stroke_width: 1.0,
            stroke_style: BorderStyle::Single,
        }
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF line payload/control leaked to normalized text: {forbidden}"
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
            .any(|operation| operation.operator == "m"),
        "EMF line vector preview should render passive PDF move operations"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "l"),
        "EMF line vector preview should render passive PDF line operations"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "S" | "s")),
        "EMF line vector preview should render passive stroked paths"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF line payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_polylineto_records_render_passively_without_payload_leakage() {
    let records = [
        emf_point_record(27, 10, 10),
        emf_poly_record(6, &[(20, 20), (200, 100)]),
        emf_point_record(54, 20, 10),
        emf_point_record(27, 30, 30),
        emf_poly16_record(89, &[(40, 40), (200, 120)]),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF POLYLINETO vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 3);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Polyline { ref points, .. }
            if points == &vec![(10.0, 10.0), (20.0, 20.0), (160.0, 80.0)]
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Line {
            x1: 160.0,
            y1: 80.0,
            x2: 20.0,
            y2: 10.0,
            ..
        }
    ));
    assert!(matches!(
        image.vector_commands[2],
        StaticImageVectorCommand::Polyline { ref points, .. }
            if points == &vec![(30.0, 30.0), (40.0, 40.0), (160.0, 80.0)]
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF POLYLINETO payload/control leaked to normalized text: {forbidden}"
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
            .filter(|operation| operation.operator == "l")
            .count()
            >= 3,
        "EMF POLYLINETO should render passive PDF line operations"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "S" | "s")),
        "EMF POLYLINETO should render passive stroked paths"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF POLYLINETO payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_polybezier_records_render_passively_without_payload_leakage() {
    let records = [
        emf_poly_record(2, &[(10, 10), (30, 5), (50, 35), (70, 30)]),
        emf_point_record(27, 80, 20),
        emf_poly_record(5, &[(90, 10), (110, 50), (130, 40)]),
        emf_poly16_record(85, &[(20, 50), (40, 30), (60, 70), (80, 60)]),
        emf_point_record(27, 90, 50),
        emf_poly16_record(88, &[(100, 40), (120, 70), (200, 100)]),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF Bezier vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 4);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Bezier { ref points, .. }
            if points == &vec![(10.0, 10.0), (30.0, 5.0), (50.0, 35.0), (70.0, 30.0)]
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Bezier { ref points, .. }
            if points == &vec![(80.0, 20.0), (90.0, 10.0), (110.0, 50.0), (130.0, 40.0)]
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF Bezier payload/control leaked to normalized text: {forbidden}"
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
            .filter(|operation| operation.operator == "c")
            .count()
            >= 4,
        "EMF Bezier paths should render passive PDF cubic curve operations"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "S" | "s")),
        "EMF Bezier paths should render passive stroked paths"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF Bezier payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_incomplete_bezier_records_are_noops_without_payload_leakage() {
    let mut polybezier = emf_poly_record(2, &[(10, 10), (30, 5), (50, 35), (70, 30), (80, 40)]);
    polybezier.extend_from_slice(b"PARTIAL-EMF-POLYBEZIER /JavaScript");
    polybezier.resize(polybezier.len().next_multiple_of(4), 0);
    let polybezier_len = polybezier.len() as u32;
    write_test_le_u32(&mut polybezier, 4, polybezier_len);

    let mut polybezierto = emf_poly_record(5, &[(30, 5), (50, 35)]);
    polybezierto.extend_from_slice(b"PARTIAL-EMF-POLYBEZIERTO /EmbeddedFile");
    polybezierto.resize(polybezierto.len().next_multiple_of(4), 0);
    let polybezierto_len = polybezierto.len() as u32;
    write_test_le_u32(&mut polybezierto, 4, polybezierto_len);

    let mut polybezier16 =
        emf_poly16_record(85, &[(20, 50), (40, 30), (60, 70), (80, 60), (90, 65)]);
    polybezier16.extend_from_slice(b"PARTIAL-EMF-POLYBEZIER16 /Launch");
    polybezier16.resize(polybezier16.len().next_multiple_of(4), 0);
    let polybezier16_len = polybezier16.len() as u32;
    write_test_le_u32(&mut polybezier16, 4, polybezier16_len);

    let mut polybezierto16 = emf_poly16_record(88, &[(40, 30), (60, 70)]);
    polybezierto16.extend_from_slice(b"PARTIAL-EMF-POLYBEZIERTO16 /RichMedia");
    polybezierto16.resize(polybezierto16.len().next_multiple_of(4), 0);
    let polybezierto16_len = polybezierto16.len() as u32;
    write_test_le_u32(&mut polybezierto16, 4, polybezierto16_len);

    let records = [
        polybezier,
        emf_point_record(27, 10, 10),
        polybezierto,
        polybezier16,
        emf_point_record(27, 20, 20),
        polybezierto16,
        emf_rect_record(43, 0, 0, 80, 40),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF incomplete Bezier no-op image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in [
        "emfblip",
        "PARTIAL-EMF-POLYBEZIER",
        "PARTIAL-EMF-POLYBEZIERTO",
        "PARTIAL-EMF-POLYBEZIER16",
        "PARTIAL-EMF-POLYBEZIERTO16",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
        "RichMedia",
    ] {
        assert!(
            !text.contains(forbidden),
            "incomplete EMF Bezier payload/control leaked to normalized text: {forbidden}"
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
        "painting after incomplete EMF Bezier no-ops should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"PARTIAL-EMF-POLYBEZIER",
        b"PARTIAL-EMF-POLYBEZIERTO",
        b"PARTIAL-EMF-POLYBEZIER16",
        b"PARTIAL-EMF-POLYBEZIERTO16",
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
            "incomplete EMF Bezier payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn zero_count_emf_poly_records_are_noops_without_payload_leakage() {
    let mut polygon = emf_poly_record(3, &[]);
    polygon.extend_from_slice(b"ZERO-EMF-POLYGON /JavaScript /EmbeddedFile");
    polygon.resize(polygon.len().next_multiple_of(4), 0);
    let polygon_len = polygon.len() as u32;
    write_test_le_u32(&mut polygon, 4, polygon_len);

    let mut polyline = emf_poly_record(4, &[]);
    polyline.extend_from_slice(b"ZERO-EMF-POLYLINE /Launch");
    polyline.resize(polyline.len().next_multiple_of(4), 0);
    let polyline_len = polyline.len() as u32;
    write_test_le_u32(&mut polyline, 4, polyline_len);

    let mut polylineto = emf_poly_record(6, &[]);
    polylineto.extend_from_slice(b"ZERO-EMF-POLYLINETO /AcroForm");
    polylineto.resize(polylineto.len().next_multiple_of(4), 0);
    let polylineto_len = polylineto.len() as u32;
    write_test_le_u32(&mut polylineto, 4, polylineto_len);

    let mut polybezier = emf_poly_record(2, &[]);
    polybezier.extend_from_slice(b"ZERO-EMF-POLYBEZIER /OpenAction");
    polybezier.resize(polybezier.len().next_multiple_of(4), 0);
    let polybezier_len = polybezier.len() as u32;
    write_test_le_u32(&mut polybezier, 4, polybezier_len);

    let mut polyline16 = emf_poly16_record(87, &[]);
    polyline16.extend_from_slice(b"ZERO-EMF-POLYLINE16 /RichMedia");
    polyline16.resize(polyline16.len().next_multiple_of(4), 0);
    let polyline16_len = polyline16.len() as u32;
    write_test_le_u32(&mut polyline16, 4, polyline16_len);

    let mut polylineto16 = emf_poly16_record(89, &[]);
    polylineto16.extend_from_slice(b"ZERO-EMF-POLYLINETO16 /AA");
    polylineto16.resize(polylineto16.len().next_multiple_of(4), 0);
    let polylineto16_len = polylineto16.len() as u32;
    write_test_le_u32(&mut polylineto16, 4, polylineto16_len);

    let mut polydraw = emf_polydraw_record(56, &[], &[]);
    polydraw.extend_from_slice(b"ZERO-EMF-POLYDRAW /Subtype /Image");
    polydraw.resize(polydraw.len().next_multiple_of(4), 0);
    let polydraw_len = polydraw.len() as u32;
    write_test_le_u32(&mut polydraw, 4, polydraw_len);

    let records = [
        polygon,
        polyline,
        polylineto,
        polybezier,
        polyline16,
        polylineto16,
        polydraw,
        emf_rect_record(43, 0, 0, 80, 40),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive zero-count EMF vector image");

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
        "emfblip",
        "ZERO-EMF-POLYGON",
        "ZERO-EMF-POLYLINE",
        "ZERO-EMF-POLYLINETO",
        "ZERO-EMF-POLYBEZIER",
        "ZERO-EMF-POLYLINE16",
        "ZERO-EMF-POLYLINETO16",
        "ZERO-EMF-POLYDRAW",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
        "OpenAction",
        "RichMedia",
        "AcroForm",
        "AA",
        "Subtype",
        "Image",
    ] {
        assert!(
            !text.contains(forbidden),
            "zero-count EMF payload/control leaked to normalized text: {forbidden}"
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
        "painting after zero-count EMF poly no-ops should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"ZERO-EMF-POLYGON",
        b"ZERO-EMF-POLYLINE",
        b"ZERO-EMF-POLYLINETO",
        b"ZERO-EMF-POLYBEZIER",
        b"ZERO-EMF-POLYLINE16",
        b"ZERO-EMF-POLYLINETO16",
        b"ZERO-EMF-POLYDRAW",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
        b"/AcroForm",
        b"/AA",
        b"/Subtype /Image",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "zero-count EMF payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_coordinate_mapping_records_render_passively_without_payload_leakage() {
    let records = [
        emf_size_record(9, 320, 160),
        emf_point_record(10, 10, 20),
        emf_size_record(11, 160, 80),
        emf_point_record(12, 5, 3),
        emf_rect_record(43, 10, 20, 170, 100),
        emf_point_record(27, 10, 20),
        emf_point_record(54, 330, 180),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF coordinate-mapped vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 5.0,
            top: 3.0,
            right: 85.0,
            bottom: 43.0,
            ..
        }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Line {
            x1: 5.0,
            y1: 3.0,
            x2: 160.0,
            y2: 80.0,
            ..
        }
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF coordinate payload/control leaked to normalized text: {forbidden}"
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
        "EMF coordinate-mapped rectangle should render passive PDF rectangle paths"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "l"),
        "EMF coordinate-mapped line should render passive PDF line operations"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF coordinate payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_scaled_extent_records_render_passively_without_payload_leakage() {
    let records = [
        emf_scale_record(31, 2, 1, 2, 1),
        emf_rect_record(43, 10, 10, 50, 30),
        emf_scale_record(32, 2, 1, 2, 1),
        emf_rect_record(43, 10, 10, 50, 30),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF scaled-extent vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 20.0,
            top: 20.0,
            right: 100.0,
            bottom: 60.0,
            ..
        }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Rectangle {
            left: 10.0,
            top: 10.0,
            right: 50.0,
            bottom: 30.0,
            ..
        }
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF scaled-extent payload/control leaked to normalized text: {forbidden}"
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
    let rect_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "re")
        .count();

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        rect_count >= 2,
        "scaled EMF rectangles should render passive PDF rectangle paths"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF scaled-extent payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_empty_excludecliprect_is_noop_without_payload_leakage() {
    let mut clip = emf_rect_record(29, 20, 20, 20, 60);
    clip.extend_from_slice(b"EMPTY-EMF-EXCLUDECLIPRECT /JavaScript /EmbeddedFile");
    clip.resize(clip.len().next_multiple_of(4), 0);
    let clip_len = clip.len() as u32;
    write_test_le_u32(&mut clip, 4, clip_len);

    let records = [clip, emf_rect_record(43, 0, 0, 160, 80)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF empty exclude no-op image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in [
        "emfblip",
        "EMPTY-EMF-EXCLUDECLIPRECT",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "empty EMF EXCLUDECLIPRECT payload/control leaked to normalized text: {forbidden}"
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
        "painting after empty EMF EXCLUDECLIPRECT no-op should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMPTY-EMF-EXCLUDECLIPRECT",
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
            "empty EMF EXCLUDECLIPRECT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_failed_extent_state_records_count_as_partial_without_payload_leakage() {
    let mut set_window = emf_size_record(9, 0, 160);
    set_window.extend_from_slice(b"ZERO-EXTENT-EMF-SETWINDOWEXTEX /JavaScript");
    set_window.resize(set_window.len().next_multiple_of(4), 0);
    let set_window_len = set_window.len() as u32;
    write_test_le_u32(&mut set_window, 4, set_window_len);

    let mut set_viewport = emf_size_record(11, 160, 0);
    set_viewport.extend_from_slice(b"ZERO-EXTENT-EMF-SETVIEWPORTEXTEX /EmbeddedFile");
    set_viewport.resize(set_viewport.len().next_multiple_of(4), 0);
    let set_viewport_len = set_viewport.len() as u32;
    write_test_le_u32(&mut set_viewport, 4, set_viewport_len);

    let mut scale_viewport = emf_scale_record(31, 0, 1, 1, 1);
    scale_viewport.extend_from_slice(b"ZERO-FACTOR-EMF-SCALEVIEWPORTEXTEX /Launch");
    scale_viewport.resize(scale_viewport.len().next_multiple_of(4), 0);
    let scale_viewport_len = scale_viewport.len() as u32;
    write_test_le_u32(&mut scale_viewport, 4, scale_viewport_len);

    let mut scale_window = emf_scale_record(32, 1, 1, 0, 1);
    scale_window.extend_from_slice(b"ZERO-FACTOR-EMF-SCALEWINDOWEXTEX /OpenAction");
    scale_window.resize(scale_window.len().next_multiple_of(4), 0);
    let scale_window_len = scale_window.len() as u32;
    write_test_le_u32(&mut scale_window, 4, scale_window_len);

    let records = [
        set_window,
        set_viewport,
        scale_viewport,
        scale_window,
        emf_rect_record(43, 10, 20, 170, 100),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF failed extent-state no-op image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unsupported record(s) skipped"))
    );
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in [
        "emfblip",
        "ZERO-EXTENT-EMF-SETWINDOWEXTEX",
        "ZERO-EXTENT-EMF-SETVIEWPORTEXTEX",
        "ZERO-FACTOR-EMF-SCALEVIEWPORTEXTEX",
        "ZERO-FACTOR-EMF-SCALEWINDOWEXTEX",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
        "OpenAction",
    ] {
        assert!(
            !text.contains(forbidden),
            "failed EMF extent-state payload/control leaked to normalized text: {forbidden}"
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
        "painting after failed EMF extent-state no-ops should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"ZERO-EXTENT-EMF-SETWINDOWEXTEX",
        b"ZERO-EXTENT-EMF-SETVIEWPORTEXTEX",
        b"ZERO-FACTOR-EMF-SCALEVIEWPORTEXTEX",
        b"ZERO-FACTOR-EMF-SCALEWINDOWEXTEX",
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
            "failed EMF extent-state payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_emf_coordinate_state_counts_as_partial_without_payload_leakage() {
    for (record_type, label) in [
        (9, "EMF-SETWINDOWEXTEX-MALFORMED"),
        (10, "EMF-SETWINDOWORGEX-MALFORMED"),
        (11, "EMF-SETVIEWPORTEXTEX-MALFORMED"),
        (12, "EMF-SETVIEWPORTORGEX-MALFORMED"),
        (13, "EMF-SETBRUSHORGEX-MALFORMED"),
        (16, "EMF-SETMAPPERFLAGS-MALFORMED"),
        (31, "EMF-SCALEVIEWPORTEXTEX-MALFORMED"),
        (32, "EMF-SCALEWINDOWEXTEX-MALFORMED"),
    ] {
        let mut state_record = emf_unknown_record(record_type);
        write_test_le_u32(&mut state_record, 4, 8);
        let records = [state_record, emf_rect_record(43, 20, 10, 80, 50)];
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
            .expect("malformed EMF coordinate state partial vector preview");

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
                "malformed EMF coordinate state payload/control leaked to normalized text: {forbidden}"
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
                "malformed EMF coordinate state payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn malformed_emf_line_position_records_count_as_partial_without_payload_leakage() {
    for (record_type, label) in [(27, "EMF-MOVETOEX-MALFORMED"), (54, "EMF-LINETO-MALFORMED")] {
        let mut state_record = emf_unknown_record(record_type);
        write_test_le_u32(&mut state_record, 4, 8);
        let records = [state_record, emf_rect_record(43, 20, 10, 80, 50)];
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
            .expect("malformed EMF line-position partial vector preview");

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
                "malformed EMF line-position payload/control leaked to normalized text: {forbidden}"
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
                "malformed EMF line-position payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn malformed_emf_arc_records_count_as_partial_without_payload_leakage() {
    for (record_type, label) in [
        (41, "EMF-ANGLEARC-MALFORMED"),
        (45, "EMF-ARC-MALFORMED"),
        (46, "EMF-CHORD-MALFORMED"),
        (47, "EMF-PIE-MALFORMED"),
        (55, "EMF-ARCTO-MALFORMED"),
    ] {
        let mut arc_record = emf_unknown_record(record_type);
        write_test_le_u32(&mut arc_record, 4, 8);
        let records = [arc_record, emf_rect_record(43, 20, 10, 80, 50)];
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
            .expect("malformed EMF arc partial vector preview");

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
                "malformed EMF arc payload/control leaked to normalized text: {forbidden}"
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
                "malformed EMF arc payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn malformed_emf_shape_bounds_count_as_partial_without_payload_leakage() {
    for (record_type, label) in [
        (43, "EMF-RECTANGLE-MALFORMED"),
        (42, "EMF-ELLIPSE-MALFORMED"),
        (44, "EMF-ROUNDRECT-MALFORMED"),
    ] {
        let mut shape_record = emf_unknown_record(record_type);
        write_test_le_u32(&mut shape_record, 4, 8);
        let records = [shape_record, emf_rect_record(43, 20, 10, 80, 50)];
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
            .expect("malformed EMF shape-bounds partial vector preview");

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
                "malformed EMF shape payload/control leaked to normalized text: {forbidden}"
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
                "malformed EMF shape payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn emf_polypolygon_records_render_single_passive_fill_path_without_payload_leakage() {
    let records = [
        emf_u32_record(19, 2),
        emf_create_brush_record(
            3,
            0,
            Color {
                red: 40,
                green: 180,
                blue: 90,
            },
            0,
        ),
        emf_select_object_record(3),
        emf_poly_poly_record(
            8,
            &[
                vec![(10, 10), (140, 10), (140, 70), (10, 70)],
                vec![(50, 25), (100, 25), (100, 55), (50, 55)],
            ],
        ),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF POLYPOLYGON vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::Path {
            fill_rule: StaticImageVectorFillRule::Winding,
            fill_color: Some(Color {
                red: 40,
                green: 180,
                blue: 90
            }),
            segments,
            ..
        } if segments.iter().any(|segment| matches!(
            segment,
            StaticImageVectorPathSegment::MoveTo(50.0, 25.0)
        ))
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF POLYPOLYGON payload/control leaked to normalized text: {forbidden}"
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
            .filter(|operation| operation.operator == "m")
            .count()
            >= 2,
        "EMF POLYPOLYGON should render separate passive PDF subpath moves"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "B")),
        "EMF POLYPOLYGON winding fill should render a passive nonzero fill operation"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF POLYPOLYGON payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn invalid_emf_setarcdirection_stays_partial_without_payload_leakage() {
    let mut mode = emf_u32_record(57, 99);
    mode.extend_from_slice(b"EMF-SETARCDIRECTION-INVALID /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after invalid EMF SETARCDIRECTION should render a passive PDF rectangle"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETARCDIRECTION-INVALID",
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
            "invalid EMF SETARCDIRECTION payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_emf_setarcdirection_keeps_later_safe_drawing() {
    let mut mode = emf_unknown_record(57);
    write_test_le_u32(&mut mode, 4, 8);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {}}} after\\par}}",
        bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
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
        .expect("malformed EMF SETARCDIRECTION partial vector preview");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(
        image
            .vector_commands
            .iter()
            .any(|command| matches!(command, StaticImageVectorCommand::Rectangle { .. })),
        "safe drawing after malformed EMF SETARCDIRECTION should stay visible"
    );
}

#[test]
fn emf_arc_chord_and_pie_records_render_passively_without_payload_leakage() {
    let records = [
        emf_create_brush_record(
            3,
            0,
            Color {
                red: 40,
                green: 120,
                blue: 200,
            },
            0,
        ),
        emf_select_object_record(3),
        emf_arc_record(45, 20, 10, 100, 70, 100, 40, 60, 70),
        emf_arc_record(46, 20, 10, 100, 70, 100, 40, 60, 70),
        emf_arc_record(47, 20, 10, 100, 70, 100, 40, 60, 70),
        emf_u32_record(57, 2),
        emf_arc_record(55, 20, 10, 100, 70, 100, 40, 60, 70),
        emf_point_record(54, 120, 70),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF arc-family vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Polyline { .. }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Polygon { .. }
    ));
    assert!(matches!(
        image.vector_commands[2],
        StaticImageVectorCommand::Polygon { .. }
    ));
    assert!(matches!(
        image.vector_commands[3],
        StaticImageVectorCommand::Polyline { .. }
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF arc-family payload/control leaked to normalized text: {forbidden}"
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
            .any(|operation| operation.operator == "l"),
        "EMF arc-family paths should render passive PDF line segments"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "f*" | "B" | "B*")),
        "EMF chord/pie paths should render passive PDF fill operations"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF arc-family payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_degenerate_arc_bounds_are_noops_without_payload_leakage() {
    let mut arc = emf_arc_record(45, 20, 10, 20, 70, 100, 40, 60, 70);
    arc.extend_from_slice(b"DEGENERATE-EMF-ARC /JavaScript");
    arc.resize(arc.len().next_multiple_of(4), 0);
    let arc_len = arc.len() as u32;
    write_test_le_u32(&mut arc, 4, arc_len);

    let mut chord = emf_arc_record(46, 20, 10, 100, 10, 100, 40, 60, 70);
    chord.extend_from_slice(b"DEGENERATE-EMF-CHORD /EmbeddedFile");
    chord.resize(chord.len().next_multiple_of(4), 0);
    let chord_len = chord.len() as u32;
    write_test_le_u32(&mut chord, 4, chord_len);

    let mut pie = emf_arc_record(47, 20, 10, 20, 70, 100, 40, 60, 70);
    pie.extend_from_slice(b"DEGENERATE-EMF-PIE /Launch");
    pie.resize(pie.len().next_multiple_of(4), 0);
    let pie_len = pie.len() as u32;
    write_test_le_u32(&mut pie, 4, pie_len);

    let mut arcto = emf_arc_record(55, 20, 10, 100, 10, 100, 40, 60, 70);
    arcto.extend_from_slice(b"DEGENERATE-EMF-ARCTO /OpenAction");
    arcto.resize(arcto.len().next_multiple_of(4), 0);
    let arcto_len = arcto.len() as u32;
    write_test_le_u32(&mut arcto, 4, arcto_len);

    let records = [arc, chord, pie, arcto, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF degenerate arc no-op image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in [
        "emfblip",
        "DEGENERATE-EMF-ARC",
        "DEGENERATE-EMF-CHORD",
        "DEGENERATE-EMF-PIE",
        "DEGENERATE-EMF-ARCTO",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
        "OpenAction",
    ] {
        assert!(
            !text.contains(forbidden),
            "degenerate EMF arc payload/control leaked to normalized text: {forbidden}"
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
        "painting after degenerate EMF arc no-ops should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"DEGENERATE-EMF-ARC",
        b"DEGENERATE-EMF-CHORD",
        b"DEGENERATE-EMF-PIE",
        b"DEGENERATE-EMF-ARCTO",
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
            "degenerate EMF arc payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_arcto_fillpath_records_render_passively_without_payload_leakage() {
    let records = [
        emf_create_brush_record(
            3,
            0,
            Color {
                red: 210,
                green: 120,
                blue: 40,
            },
            0,
        ),
        emf_select_object_record(3),
        emf_unknown_record(59),
        emf_point_record(27, 60, 70),
        emf_arc_record(55, 20, 10, 100, 70, 100, 40, 60, 70),
        emf_unknown_record(61),
        emf_unknown_record(60),
        emf_unknown_record(62),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF ArcTo fill path image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Polygon {
            fill_color: Some(Color {
                red: 210,
                green: 120,
                blue: 40
            }),
            ..
        }
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF ArcTo fill path payload/control leaked to normalized text: {forbidden}"
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
            .filter(|operation| operation.operator == "l")
            .count()
            >= 4,
        "EMF ArcTo fill path should render sampled passive PDF line segments"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "f*")),
        "EMF ArcTo fill path should render passive PDF fill operations"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF ArcTo fill path payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_polydraw_records_render_passively_without_payload_leakage() {
    let records = [
        emf_polydraw_record(
            56,
            &[
                (10, 10),
                (50, 10),
                (50, 40),
                (60, 10),
                (70, 5),
                (80, 40),
                (90, 30),
            ],
            &[0x06, 0x02, 0x03, 0x06, 0x04, 0x04, 0x05],
        ),
        emf_point_record(54, 100, 10),
        emf_polydraw16_record(92, &[(20, 50), (60, 50), (60, 70)], &[0x06, 0x02, 0x02]),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF POLYDRAW vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Polyline { .. }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Bezier { .. }
    ));
    assert!(matches!(
        image.vector_commands[4],
        StaticImageVectorCommand::Polyline { .. }
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF POLYDRAW payload/control leaked to normalized text: {forbidden}"
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
            .any(|operation| operation.operator == "l"),
        "EMF POLYDRAW lines should render passive PDF line operations"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "c"),
        "EMF POLYDRAW Bezier segments should render passive PDF cubic operations"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF POLYDRAW payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_incomplete_polydraw_bezier_runs_are_noops_without_payload_leakage() {
    let mut polydraw =
        emf_polydraw_record(56, &[(10, 10), (30, 20), (40, 30)], &[0x06, 0x04, 0x04]);
    polydraw.extend_from_slice(b"PARTIAL-EMF-POLYDRAW /JavaScript /EmbeddedFile");
    polydraw.resize(polydraw.len().next_multiple_of(4), 0);
    let polydraw_len = polydraw.len() as u32;
    write_test_le_u32(&mut polydraw, 4, polydraw_len);

    let mut polydraw16 = emf_polydraw16_record(92, &[(20, 20), (60, 20)], &[0x06, 0x04]);
    polydraw16.extend_from_slice(b"PARTIAL-EMF-POLYDRAW16 /Launch /RichMedia");
    polydraw16.resize(polydraw16.len().next_multiple_of(4), 0);
    let polydraw16_len = polydraw16.len() as u32;
    write_test_le_u32(&mut polydraw16, 4, polydraw16_len);

    let records = [polydraw, polydraw16, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF incomplete POLYDRAW no-op image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in [
        "emfblip",
        "PARTIAL-EMF-POLYDRAW",
        "PARTIAL-EMF-POLYDRAW16",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
        "RichMedia",
    ] {
        assert!(
            !text.contains(forbidden),
            "incomplete EMF POLYDRAW payload/control leaked to normalized text: {forbidden}"
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
        "painting after incomplete EMF POLYDRAW no-ops should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"PARTIAL-EMF-POLYDRAW",
        b"PARTIAL-EMF-POLYDRAW16",
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
            "incomplete EMF POLYDRAW payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_anglearc_records_render_passively_without_payload_leakage() {
    let records = [
        emf_point_record(27, 10, 40),
        emf_anglearc_record(60, 40, 40, 0.0, 90.0),
        emf_point_record(54, 60, 70),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF ANGLEARC vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::Polyline { points, .. }
            if points.first() == Some(&(10.0, 40.0))
                && points.get(1) == Some(&(100.0, 40.0))
                && points.last() == Some(&(60.0, 0.0))
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF ANGLEARC payload/control leaked to normalized text: {forbidden}"
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
            .any(|operation| operation.operator == "l"),
        "EMF ANGLEARC should render passive PDF line operations"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF ANGLEARC payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_zero_sweep_anglearc_renders_line_without_payload_leakage() {
    let mut anglearc = emf_anglearc_record(60, 40, 40, 0.0, 0.0);
    anglearc.extend_from_slice(b"ZERO-SWEEP-EMF-ANGLEARC /JavaScript /EmbeddedFile");
    anglearc.resize(anglearc.len().next_multiple_of(4), 0);
    let anglearc_len = anglearc.len() as u32;
    write_test_le_u32(&mut anglearc, 4, anglearc_len);

    let records = [
        emf_point_record(27, 10, 40),
        anglearc,
        emf_point_record(54, 60, 70),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF zero-sweep ANGLEARC image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::Polyline { points, .. }
            if points == &vec![(10.0, 40.0), (100.0, 40.0)]
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Line {
            x1: 100.0,
            y1: 40.0,
            x2: 60.0,
            y2: 70.0,
            ..
        }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "ZERO-SWEEP-EMF-ANGLEARC",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "zero-sweep EMF ANGLEARC payload/control leaked to normalized text: {forbidden}"
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
            .filter(|operation| operation.operator == "l")
            .count()
            >= 2,
        "zero-sweep EMF ANGLEARC should render passive PDF line operations"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"ZERO-SWEEP-EMF-ANGLEARC",
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
            "zero-sweep EMF ANGLEARC payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_zero_radius_anglearc_renders_line_without_payload_leakage() {
    let mut anglearc = emf_anglearc_record(60, 40, 0, 0.0, 90.0);
    anglearc.extend_from_slice(b"ZERO-RADIUS-EMF-ANGLEARC /JavaScript /EmbeddedFile");
    anglearc.resize(anglearc.len().next_multiple_of(4), 0);
    let anglearc_len = anglearc.len() as u32;
    write_test_le_u32(&mut anglearc, 4, anglearc_len);

    let records = [
        emf_point_record(27, 10, 40),
        anglearc,
        emf_point_record(54, 60, 70),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF zero-radius ANGLEARC image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::Polyline { points, .. }
            if points == &vec![(10.0, 40.0), (60.0, 40.0)]
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Line {
            x1: 60.0,
            y1: 40.0,
            x2: 60.0,
            y2: 70.0,
            ..
        }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "ZERO-RADIUS-EMF-ANGLEARC",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "zero-radius EMF ANGLEARC payload/control leaked to normalized text: {forbidden}"
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
            .filter(|operation| operation.operator == "l")
            .count()
            >= 2,
        "zero-radius EMF ANGLEARC should render passive PDF line operations"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"ZERO-RADIUS-EMF-ANGLEARC",
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
            "zero-radius EMF ANGLEARC payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_savedc_and_restoredc_records_render_passively_without_payload_leakage() {
    let records = [
        emf_create_pen_record(
            2,
            0,
            2,
            Color {
                red: 200,
                green: 20,
                blue: 20,
            },
        ),
        emf_select_object_record(2),
        emf_point_record(12, 5, 0),
        emf_unknown_record(33),
        emf_create_pen_record(
            3,
            1,
            4,
            Color {
                red: 20,
                green: 40,
                blue: 200,
            },
        ),
        emf_select_object_record(3),
        emf_point_record(12, 20, 0),
        emf_point_record(27, 0, 10),
        emf_point_record(54, 40, 10),
        emf_i32_record(34, -1),
        emf_point_record(27, 0, 20),
        emf_point_record(54, 40, 20),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF saved-state vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::SaveState
    ));
    assert!(matches!(
        image.vector_commands[2],
        StaticImageVectorCommand::RestoreState
    ));
    let line_commands: Vec<_> = image
        .vector_commands
        .iter()
        .filter(|command| matches!(command, StaticImageVectorCommand::Line { .. }))
        .collect();
    assert_eq!(line_commands.len(), 2);
    assert!(matches!(
        line_commands[0],
        StaticImageVectorCommand::Line {
            x1: 20.0,
            y1: 10.0,
            x2: 60.0,
            y2: 10.0,
            stroke_color: Some(Color {
                red: 20,
                green: 40,
                blue: 200
            }),
            stroke_width: 4.0,
            stroke_style: BorderStyle::Dashed,
        }
    ));
    assert!(matches!(
        line_commands[1],
        StaticImageVectorCommand::Line {
            x1: 5.0,
            y1: 20.0,
            x2: 45.0,
            y2: 20.0,
            stroke_color: Some(Color {
                red: 200,
                green: 20,
                blue: 20
            }),
            stroke_width: 2.0,
            stroke_style: BorderStyle::Single,
        }
    ));
    for forbidden in ["emfblip", " EMF", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "EMF saved-state payload/control leaked to normalized text: {forbidden}"
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
            .any(|operation| operation.operator == "l"),
        "EMF saved-state lines should render passive PDF line operations"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "RG" | "rg")),
        "EMF saved-state colors should render passive PDF color operations"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF saved-state payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_restoredc_restores_passive_clip_scope_without_payload_leakage() {
    let records = [
        emf_unknown_record(33),
        emf_rect_record(30, 20, 20, 60, 60),
        emf_rect_record(43, 0, 0, 160, 80),
        emf_i32_record(34, -1),
        emf_rect_record(43, 80, 0, 150, 70),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("passive EMF restored clip image");

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
            left: 80.0,
            top: 0.0,
            right: 150.0,
            bottom: 70.0,
            ..
        }
    ));

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
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "q")
            .count()
            >= 2,
        "EMF saved clip state should render passive PDF save-state operations"
    );
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "Q")
            .count()
            >= 2,
        "EMF restored clip state should render passive PDF restore-state operations"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "W"),
        "EMF saved clip scope should render a passive PDF clipping path"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b" EMF",
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
            "EMF restored clip payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_restoredc_without_saved_state_is_noop_without_payload_leakage() {
    let mut restore = emf_i32_record(34, -1);
    restore.extend_from_slice(b"UNSAVED-EMF-RESTOREDC /JavaScript /EmbeddedFile");
    restore.resize(restore.len().next_multiple_of(4), 0);
    let restore_len = restore.len() as u32;
    write_test_le_u32(&mut restore, 4, restore_len);

    let records = [restore, emf_rect_record(43, 10, 20, 170, 100)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
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
        .expect("passive EMF unsaved RESTOREDC no-op image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert_eq!(parsed.diagnostics.len(), 0);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    for forbidden in [
        "emfblip",
        "UNSAVED-EMF-RESTOREDC",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "unsaved EMF RESTOREDC payload/control leaked to normalized text: {forbidden}"
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
        "painting after unsaved EMF RESTOREDC no-op should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"UNSAVED-EMF-RESTOREDC",
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
            "unsaved EMF RESTOREDC payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}
