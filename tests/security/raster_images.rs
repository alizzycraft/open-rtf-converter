use super::*;

#[test]
fn word_shape_picture_wmf_uses_exact_preview_coordinates() {
    let input = fs::read("docs/sample.rtf").unwrap();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) if image.format == ImageFormat::WmfVector => Some(image),
            _ => None,
        })
        .expect("sample shape picture WMF preview");

    assert!(image.vector_commands.iter().any(|command| matches!(
        command,
        StaticImageVectorCommand::Rectangle {
            left,
            top,
            right,
            bottom,
            fill_color: Some(Color { red: 128, green: 128, blue: 128 }),
            stroke_color: None,
            ..
        } if (*left - 23.0).abs() < 0.01
            && (*top - 25.0).abs() < 0.01
            && (*right - 356.0).abs() < 0.01
            && (*bottom - 238.0).abs() < 0.01
    )));
    assert!(image.vector_commands.iter().any(|command| matches!(
        command,
        StaticImageVectorCommand::Rectangle {
            left,
            top,
            right,
            bottom,
            fill_color: Some(Color { red: 255, green: 255, blue: 0 }),
            stroke_color: Some(Color { red: 0, green: 0, blue: 255 }),
            ..
        } if (*left - 0.0).abs() < 0.01
            && (*top - 1.0).abs() < 0.01
            && (*right - 330.0).abs() < 0.01
            && (*bottom - 212.0).abs() < 0.01
    )));
    assert!(image.vector_commands.iter().any(|command| matches!(
        command,
        StaticImageVectorCommand::Ellipse {
            left,
            top,
            right,
            bottom,
            fill_color: Some(Color { red: 0, green: 0, blue: 255 }),
            stroke_color: Some(Color { red: 255, green: 0, blue: 255 }),
            ..
        } if (*left - 239.0).abs() < 0.01
            && (*top - 121.0).abs() < 0.01
            && (*right - 630.0).abs() < 0.01
            && (*bottom - 302.0).abs() < 0.01
    )));
}

#[test]
fn inline_wmf_picture_stays_in_its_surrounding_safe_paragraph() {
    let input = fs::read("fixtures/wmf-text-passive.rtf").unwrap();
    let parsed = parse_rtf_bytes(&input).unwrap();
    assert_eq!(parsed.document.inline_images.len(), 1);
    assert_eq!(
        parsed
            .document
            .blocks
            .iter()
            .filter(|block| matches!(block, Block::Image(_)))
            .count(),
        1
    );
    assert_eq!(parsed.document.inline_image_block_indices.len(), 1);
    let command = parsed.document.inline_images[0]
        .vector_commands
        .iter()
        .find_map(|command| match command {
            StaticImageVectorCommand::Text {
                height,
                bold,
                vertical_scale_percent,
                text,
                background_color,
                character_extra,
                ..
            } => Some((
                height,
                bold,
                vertical_scale_percent,
                text,
                background_color,
                character_extra,
            )),
            _ => None,
        })
        .expect("fontless WMF text command");
    assert!((command.0 - (80.0 / 5.52)).abs() < 0.01);
    assert!(*command.1, "Word maps the stock SYSTEM_FONT to bold Arial");
    assert_eq!(*command.2, 81);
    assert_eq!(command.3, "Hi");
    assert_eq!(
        *command.4,
        Some(Color {
            red: 255,
            green: 255,
            blue: 255
        })
    );
    assert_eq!(*command.5, 160.0);
    let paragraph = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .expect("inline picture paragraph");
    assert!(paragraph.runs.iter().any(|run| {
        run.text
            .chars()
            .any(|ch| inline_image_marker_index(&ch.to_string()) == Some(0))
    }));

    let layout = LayoutEngine::layout(&parsed.document);
    let page = &layout.pages[0];
    let image = page
        .items
        .iter()
        .find_map(|item| match item {
            LayoutItem::Image(image) => Some(image),
            _ => None,
        })
        .expect("inline image fragment");
    assert!((image.x - 129.36).abs() < 0.2, "x={}", image.x);
    assert!((image.width - 72.0).abs() < 0.01);
    let after = page
        .items
        .iter()
        .find_map(|item| match item {
            LayoutItem::Text(fragment) if fragment.text.starts_with("After") => Some(fragment),
            _ => None,
        })
        .expect("following inline text");
    assert!((after.x - 201.36).abs() < 0.2, "x={}", after.x);

    let converted = convert_rtf_to_pdf(&input, &ConvertOptions::default()).unwrap();
    assert!(
        converted
            .pdf
            .windows(b"/BaseFont /Helvetica-Bold".len())
            .any(|window| window == b"/BaseFont /Helvetica-Bold"),
        "bold WMF text must register its requested passive PDF font resource"
    );
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
fn binary_picture_payload_that_looks_like_rtf_stays_opaque_and_passive() {
    let payload = br#"}{\object\objdata 4142432f4a617661536372697074}{\field{\*\fldinst HYPERLINK "https://example.com/payload"}{\fldrslt BAD}}"#;
    let mut input = br"{\rtf1 before {\pict\pngblip\picw100\pich100\bin".to_vec();
    input.extend_from_slice(payload.len().to_string().as_bytes());
    input.push(b' ');
    input.extend_from_slice(payload);
    input.extend_from_slice(br"} after\par}");

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(text.contains("[Image skipped: unsupported PNG]"));
    for forbidden in [
        "object",
        "objdata",
        "414243",
        "JavaScript",
        "HYPERLINK",
        "https://example.com/payload",
        "BAD",
    ] {
        assert!(
            !text.contains(forbidden),
            "RTF-looking binary picture payload leaked to text: {forbidden}"
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
        b"objdata".as_slice(),
        b"414243",
        b"JavaScript",
        b"HYPERLINK",
        b"https://example.com/payload",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "RTF-looking binary picture payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn shape_picture_identity_metadata_does_not_mark_passive_shape_unsupported() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn pib}{\sv passive-picture-binary-id}}{\sp{\sn pictureId}{\sv 65536}}{\sp{\sn pictureActive}{\sv 0}}{\sp{\sn pictureGray}{\sv 0}}{\sp{\sn pictureBiLevel}{\sv 0}}{\sp{\sn fLine}{\sv 0}}}}After\par}"#.to_vec();
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
        .expect("passive shape with picture identity metadata");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert_eq!(shape.stroke_width_twips, 0);
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("shape picture activation metadata stripped")
    }));
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unsupported/active drawing properties")),
        "picture identity metadata should not mark passive shape unsupported: {:?}",
        parsed.diagnostics
    );
    for forbidden in [
        "pictureId",
        "pib",
        "pictureActive",
        "pictureGray",
        "pictureBiLevel",
        "shapeType",
        "fLine",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "shape picture identity metadata leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    for forbidden in [
        b"pictureId".as_slice(),
        b"pib",
        b"pictureActive",
        b"pictureGray",
        b"pictureBiLevel",
        b"shapeType",
        b"fLine",
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
            "shape picture identity metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn invalid_shape_picture_color_mode_metadata_is_stripped_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn pictureGray}{\sv maybe}}{\sp{\sn pictureBiLevel}{\sv also-maybe}}{\sp{\sn pFragments}{\sv hidden-invalid-shape-picture-mode-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with invalid picture color-mode metadata");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("stripping unsupported/active drawing properties")
    }));
    for forbidden in [
        "pictureGray",
        "pictureBiLevel",
        "maybe",
        "also-maybe",
        "pFragments",
        "hidden-invalid-shape-picture-mode-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "invalid shape picture color-mode metadata leaked to normalized text: {forbidden}"
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
        b"pictureGray".as_slice(),
        b"pictureBiLevel",
        b"maybe",
        b"also-maybe",
        b"pFragments",
        b"hidden-invalid-shape-picture-mode-payload",
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
            "invalid shape picture color-mode metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn shape_picture_tone_metadata_updates_safe_image_without_property_leakage() {
    let image_hex = bytes_to_hex(&minimal_24bit_dib_with_rgb_pixels(
        2,
        1,
        &[[128, 64, 192], [32, 96, 160]],
    ));
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
        "shpbottom1440{",
        "\\",
        "sp{",
        "\\",
        "sn pictureBrightness}{",
        "\\",
        "sv 32768}}{",
        "\\",
        "sp{",
        "\\",
        "sn pictureContrast}{",
        "\\",
        "sv 65536}}}{",
        "\\",
        "*",
        "\\",
        "shppict{",
        "\\",
        "pict",
        "\\",
        "dibitmap0",
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
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("shape picture image");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(image.format, ImageFormat::Rgb8);
    assert_eq!(image.bytes, vec![255; 6]);
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unsupported/active drawing properties")),
        "shape tone metadata should be consumed as passive metadata: {:?}",
        parsed.diagnostics
    );
    for forbidden in [
        "pictureBrightness",
        "pictureContrast",
        "32768",
        "65536",
        "dibitmap",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "shape picture tone metadata leaked to normalized text: {forbidden}"
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
        b"pictureBrightness".as_slice(),
        b"pictureContrast",
        b"32768",
        b"65536",
        b"dibitmap",
        image_hex.as_bytes(),
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
            "shape picture tone metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn invalid_picture_metadata_is_diagnosed_without_property_leakage() {
    let image_hex = bytes_to_hex(&minimal_rgb_png(&[[255, 0, 0]]));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "pict",
        "\\",
        "pngblip{",
        "\\",
        "picprop{",
        "\\",
        "sp{",
        "\\",
        "sn pictureBrightness}{",
        "\\",
        "sv active-brightness-payload}}{",
        "\\",
        "sp{",
        "\\",
        "sn pictureContrast}{",
        "\\",
        "sv active-contrast-payload}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hidden-invalid-picture-metadata-payload}}}",
        "\\",
        "picwgoal720",
        "\\",
        "pichgoal720 ",
        image_hex.as_str(),
        "} After",
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
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("safe image survives invalid picture metadata");

    assert_eq!(image.format, ImageFormat::Png);
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("stripping unsupported/active picture metadata")
    }));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "invalid picture metadata should be recognized and stripped, not treated as unknown RTF: {:?}",
        parsed.diagnostics
    );
    for forbidden in [
        "pictureBrightness",
        "pictureContrast",
        "active-brightness-payload",
        "active-contrast-payload",
        "pFragments",
        "hidden-invalid-picture-metadata-payload",
        "pngblip",
    ] {
        assert!(
            !text.contains(forbidden),
            "invalid picture metadata leaked to normalized text: {forbidden}"
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
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("stripping unsupported/active picture metadata")
    }));
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(decoded_pdf_text(&content).contains("Before"));
    assert!(decoded_pdf_text(&content).contains("After"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "Do"),
        "safe picture bytes should still render as a passive image"
    );
    for forbidden in [
        b"pictureBrightness".as_slice(),
        b"pictureContrast",
        b"active-brightness-payload",
        b"active-contrast-payload",
        b"pFragments",
        b"hidden-invalid-picture-metadata-payload",
        b"pngblip",
        image_hex.as_bytes(),
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
            "invalid picture metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn structural_metadata_shape_pictures_do_not_emit_pdf_placeholders_or_images() {
    let input = br"{\rtf1{\info{\title Hidden title {\*\shppict{\pict\pngblip 00}}{\*\nonshppict{\pict\wmetafile8 2f4a617661536372697074}}}}Visible body\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert_eq!(text, "Visible body");
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::Image(_)))
    );
    for forbidden in [
        "Hidden title",
        "Image skipped",
        "unsupported picture",
        "shppict",
        "nonshppict",
        "pict",
        "pngblip",
        "wmetafile",
        "JavaScript",
    ] {
        assert!(
            !text.contains(forbidden),
            "metadata shape-picture fallback leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("Visible body"));
    for forbidden in [
        b"Hidden title".as_slice(),
        b"Image skipped",
        b"unsupported picture",
        b"shppict",
        b"nonshppict",
        b"pict",
        b"pngblip",
        b"wmetafile",
        b"JavaScript",
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/EmbeddedFile",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "metadata shape-picture fallback leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
    audit_passive_pdf_bytes(&output.pdf).unwrap();
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
    let visible_text = text.replace(PASSIVE_ADVANCE_MARKER, "");

    assert!(
        visible_text.contains("Values 0042 1,234,567 $5.00 -008."),
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
    let searchable_pdf = pdf_bytes_excluding_embedded_font_programs(&output.pdf);
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
            !searchable_pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden numeric-picture content leaked to PDF: {:?}",
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
fn paragraph_start_images_do_not_create_an_extra_blank_flow_line() {
    let position = |input: &[u8], needle: &str| {
        let parsed = parse_rtf_bytes(input).unwrap();
        assert_eq!(parsed.document.inline_images.len(), 1);
        assert_eq!(parsed.document.inline_image_block_indices.len(), 1);

        let output = convert_rtf_to_pdf(input, &ConvertOptions::browser_safe_defaults()).unwrap();
        audit_passive_pdf_bytes(&output.pdf).unwrap();
        let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
        let page_id = *parsed_pdf.get_pages().values().next().expect("page");
        let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
        pdf_first_text_position_for_text(&content, needle)
            .unwrap_or_else(|| panic!("missing PDF text position for {needle:?}"))
    };

    let alpha = position(include_bytes!("../../fixtures/png-alpha.rtf"), "After");
    let scaled = position(
        include_bytes!("../../fixtures/picture-scaling-passive.rtf"),
        "After",
    );

    assert!(
        (alpha.1 - 659.02).abs() < 0.03,
        "alpha-image paragraph should match Word flow: {alpha:?}"
    );
    assert!(
        (scaled.1 - 635.02).abs() < 0.03,
        "scaled-image paragraph should match Word flow: {scaled:?}"
    );
}

#[test]
fn picture_crop_uses_authored_goal_dimensions_without_control_leakage() {
    let image_hex = bytes_to_hex(&minimal_jpeg_with_dimensions(2, 1));
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
        "pich40",
        "\\",
        "picwgoal1440",
        "\\",
        "pichgoal720",
        "\\",
        "piccropl720 ",
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
    assert_eq!(image.natural_width_px_hint, Some(80));
    assert_eq!(image.natural_height_px_hint, Some(40));
    assert_eq!(image.crop.left_twips, 720);
    for forbidden in [
        "jpegblip", "picw80", "pich40", "picwgoal", "pichgoal", "piccropl",
    ] {
        assert!(
            !text.contains(forbidden),
            "picture crop control leaked to text: {forbidden}"
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
    let image_transform = pdf_image_transform_before_first_do(&content).expect("image transform");
    assert_eq!(image_transform.operands.len(), 6);
    assert!(
        pdf_operand_number(&image_transform.operands[0])
            .is_some_and(|value| (value - 72.0).abs() < 0.01),
        "picture crop should preserve the authored full goal width; got {:?}",
        image_transform.operands
    );
    assert!(
        pdf_operand_number(&image_transform.operands[3])
            .is_some_and(|value| (value - 36.0).abs() < 0.01),
        "picture crop should preserve displayed height; got {:?}",
        image_transform.operands
    );
    for forbidden in [
        b"jpegblip".as_slice(),
        b"picw80",
        b"pich40",
        b"picwgoal",
        b"pichgoal",
        b"piccropl",
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
            "forbidden picture crop content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
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
fn one_sided_picture_goal_dimensions_preserve_aspect_ratio_without_control_leakage() {
    let image_hex = bytes_to_hex(&minimal_jpeg_with_dimensions(2, 1));
    let width_only = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "jpegblip",
        "\\",
        "picwgoal1440 ",
        image_hex.as_str(),
        "} after",
        "\\",
        "par}",
    ]);
    let height_only = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "jpegblip",
        "\\",
        "pichgoal720 ",
        image_hex.as_str(),
        "} after",
        "\\",
        "par}",
    ]);

    for (input, forbidden_goal) in [
        (&width_only, b"picwgoal".as_slice()),
        (&height_only, b"pichgoal".as_slice()),
    ] {
        let parsed = parse_rtf_bytes(input).unwrap();
        let text = collect_text(&parsed.document);
        assert!(text.contains("before"));
        assert!(text.contains("after"));
        assert!(!text.contains("jpegblip"));
        assert!(!text.contains("picwgoal"));
        assert!(!text.contains("pichgoal"));

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
        let image_transform = content
            .operations
            .iter()
            .find(|operation| operation.operator == "cm")
            .expect("image transform");

        assert_eq!(image_transform.operands.len(), 6);
        assert!(
            pdf_operand_number(&image_transform.operands[0])
                .is_some_and(|value| (value - 72.0).abs() < 0.01),
            "one-sided picture goal should preserve natural width ratio; got {:?}",
            image_transform.operands
        );
        assert!(
            pdf_operand_number(&image_transform.operands[3])
                .is_some_and(|value| (value - 36.0).abs() < 0.01),
            "one-sided picture goal should preserve natural height ratio; got {:?}",
            image_transform.operands
        );
        for forbidden in [
            forbidden_goal,
            b"jpegblip",
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
                "forbidden one-sided picture sizing content leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
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
fn picture_units_per_inch_metadata_affects_natural_size_without_payload_leakage() {
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
        "picw192",
        "\\",
        "pich96",
        "\\",
        "blipupi192 ",
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

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.width_px, 1);
    assert_eq!(image.height_px, 1);
    assert_eq!(image.natural_width_px_hint, Some(96));
    assert_eq!(image.natural_height_px_hint, Some(48));
    assert_eq!(image.display_width_twips, None);
    assert_eq!(image.display_height_twips, None);
    for forbidden in ["picw", "pich", "blipupi", "jpegblip"] {
        assert!(
            !text.contains(forbidden),
            "picture units-per-inch metadata leaked to text: {forbidden}"
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
            .is_some_and(|value| (value - 72.0).abs() < 0.01),
        "horizontal image matrix should use blip units-per-inch natural size; got {:?}",
        image_transform.operands
    );
    assert!(
        pdf_operand_number(&image_transform.operands[3])
            .is_some_and(|value| (value - 36.0).abs() < 0.01),
        "vertical image matrix should use blip units-per-inch natural size; got {:?}",
        image_transform.operands
    );
    for forbidden in [
        b"picw".as_slice(),
        b"pich",
        b"blipupi",
        b"jpegblip",
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
            "picture units-per-inch metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn picture_units_per_inch_metadata_is_bounded_before_layout() {
    let image_hex = bytes_to_hex(&minimal_jpeg_with_dimensions(1, 1));

    let invalid = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "pict",
        "\\",
        "jpegblip",
        "\\",
        "picw80",
        "\\",
        "pich40",
        "\\",
        "blipupi0 ",
        image_hex.as_str(),
        "}}",
    ]);
    let parsed_invalid = parse_rtf_bytes(&invalid).unwrap();
    let invalid_image = parsed_invalid
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            open_rtf_converter::model::Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("invalid blipupi image");
    assert_eq!(invalid_image.natural_width_px_hint, Some(80));
    assert_eq!(invalid_image.natural_height_px_hint, Some(40));
    assert!(
        parsed_invalid
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic
                .message
                .contains("picture units-per-inch metadata ignored")),
        "invalid blipupi should be diagnosed: {:?}",
        parsed_invalid.diagnostics
    );

    let oversized_units = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "jpegblip",
        "\\",
        "picw9600",
        "\\",
        "pich4800",
        "\\",
        "blipupi999999 ",
        image_hex.as_str(),
        "} after",
        "\\",
        "par}",
    ]);
    let parsed_oversized_units = parse_rtf_bytes(&oversized_units).unwrap();
    let oversized_units_text = collect_text(&parsed_oversized_units.document);
    let oversized_units_image = parsed_oversized_units
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            open_rtf_converter::model::Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("oversized blipupi image");
    assert!(oversized_units_text.contains("before"));
    assert!(oversized_units_text.contains("after"));
    assert_eq!(oversized_units_image.natural_width_px_hint, Some(96));
    assert_eq!(oversized_units_image.natural_height_px_hint, Some(48));
    assert!(
        parsed_oversized_units
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic
                .message
                .contains("picture units-per-inch metadata clamped from 999999 to 9600")),
        "oversized blipupi should be diagnosed: {:?}",
        parsed_oversized_units.diagnostics
    );
    for forbidden in ["picw", "pich", "blipupi", "jpegblip"] {
        assert!(
            !oversized_units_text.contains(forbidden),
            "oversized picture units-per-inch metadata leaked to text: {forbidden}"
        );
    }

    let bounded = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "jpegblip",
        "\\",
        "picw10",
        "\\",
        "pich5",
        "\\",
        "blipupi1 ",
        image_hex.as_str(),
        "} after",
        "\\",
        "par}",
    ]);
    let options = RtfParseOptions {
        limits: RtfLimits {
            max_image_dimension_hint_px: 10,
            ..RtfLimits::default()
        },
        ..RtfParseOptions::default()
    };
    let parsed_bounded = parse_rtf_bytes_with_options(&bounded, &options).unwrap();
    let bounded_text = collect_text(&parsed_bounded.document);
    let bounded_image = parsed_bounded
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            open_rtf_converter::model::Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("bounded blipupi image");
    assert!(bounded_text.contains("before"));
    assert!(bounded_text.contains("after"));
    assert_eq!(bounded_image.natural_width_px_hint, Some(10));
    assert_eq!(bounded_image.natural_height_px_hint, Some(10));
    assert!(
        parsed_bounded
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic
                .message
                .contains("from units-per-inch metadata clamped to 10 px")),
        "inflated blipupi natural size should be clamped: {:?}",
        parsed_bounded.diagnostics
    );
    for forbidden in ["picw", "pich", "blipupi", "jpegblip"] {
        assert!(
            !bounded_text.contains(forbidden),
            "bounded picture units-per-inch metadata leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &bounded,
        &ConvertOptions {
            diagnostics: true,
            parse_options: options,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 1);
    for forbidden in [
        b"picw".as_slice(),
        b"pich",
        b"blipupi",
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
            "bounded picture units-per-inch metadata leaked to PDF: {:?}",
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
fn indexed_png_grayscale_metadata_updates_palette_without_payload_leakage() {
    let image_hex = bytes_to_hex(&minimal_indexed_png_with_dimensions(1, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "pngblip{",
        "\\",
        "picprop{",
        "\\",
        "sp{",
        "\\",
        "sn pictureGray}{",
        "\\",
        "sv 1}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv indexed-hidden-fragment}}}",
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
    assert!(!text.contains("pictureGray"));
    assert!(!text.contains("pFragments"));
    assert!(!text.contains("indexed-hidden-fragment"));
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("picture grayscale property approximated")),
        "indexed PNG grayscale should be applied to the safe palette: {:?}",
        parsed.diagnostics
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("indexed PNG picture grayscale property rendered as bounded passive palette")
    }));
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
    assert_eq!(image.palette, vec![54, 54, 54, 182, 182, 182]);

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
    for forbidden in [
        b"pngblip".as_slice(),
        b"picprop",
        b"pictureGray",
        b"pFragments",
        b"indexed-hidden-fragment",
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
            "indexed PNG grayscale metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn indexed_png_bilevel_metadata_updates_palette_without_payload_leakage() {
    let image_hex = bytes_to_hex(&minimal_indexed_png_with_dimensions(1, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "pngblip{",
        "\\",
        "picprop{",
        "\\",
        "sp{",
        "\\",
        "sn pictureBiLevel}{",
        "\\",
        "sv 1}}{",
        "\\",
        "sp{",
        "\\",
        "sn pictureGray}{",
        "\\",
        "sv 1}}}",
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
    assert!(!text.contains("pictureBiLevel"));
    assert!(!text.contains("pictureGray"));
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("picture bilevel property approximated")
                && !diagnostic
                    .message
                    .contains("picture grayscale property approximated")
        }),
        "indexed PNG bilevel should be applied to the safe palette: {:?}",
        parsed.diagnostics
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("indexed PNG picture bilevel property rendered as bounded passive palette")
    }));
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
    assert_eq!(image.palette, vec![0, 0, 0, 255, 255, 255]);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
    for forbidden in [
        b"pngblip".as_slice(),
        b"picprop",
        b"pictureBiLevel",
        b"pictureGray",
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
            "indexed PNG bilevel metadata leaked to PDF: {:?}",
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
fn jpeg_picture_trailing_bytes_are_stripped_before_pdf_without_payload_leakage() {
    let image = minimal_jpeg_with_dimensions(2, 2);
    let mut payload = image.clone();
    payload.extend_from_slice(b"TRAILING-JPEG-AFTER-EOI /JavaScript /EmbeddedFile");
    let payload_hex = bytes_to_hex(&payload);
    let input =
        format!("{{\\rtf1 before {{\\pict\\jpegblip {payload_hex}}} after\\par}}").into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image_block = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("JPEG picture image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image_block.format, ImageFormat::Jpeg);
    assert_eq!(image_block.width_px, 2);
    assert_eq!(image_block.height_px, 2);
    assert_eq!(image_block.bytes, image);
    for forbidden in [
        "jpegblip",
        "TRAILING-JPEG-AFTER-EOI",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "trailing JPEG payload/control leaked to normalized text: {forbidden}"
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
        output
            .pdf
            .windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image"),
        "trimmed JPEG should still render as a passive image XObject"
    );
    for forbidden in [
        b"jpegblip".as_slice(),
        payload_hex.as_bytes(),
        b"TRAILING-JPEG-AFTER-EOI",
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
            "trailing JPEG payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_jpeg_without_scan_is_suppressed_without_payload_leakage() {
    let mut payload = minimal_jpeg_frame_header_with_components(2, 2, 3);
    payload.extend_from_slice(b"TRAILING-MALFORMED-JPEG /JavaScript /EmbeddedFile");
    let payload_hex = bytes_to_hex(&payload);
    let input =
        format!("{{\\rtf1 before{{\\pict\\jpegblip {payload_hex}}}after\\par}}").into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(parsed.document.inline_images.is_empty());
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::Image(_) | Block::Placeholder(_)))
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("malformed JPEG picture suppressed for Word-compatible passive rendering")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
    for forbidden in [
        b"jpegblip".as_slice(),
        b"TRAILING-MALFORMED-JPEG",
        b"/DCTDecode",
        b"/Subtype /Image",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "malformed JPEG payload leaked to PDF: {:?}",
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
                    .contains("malformed JPEG picture data replaced")
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
fn inert_picture_hit_test_metadata_is_consumed_without_payload_leakage() {
    let image_hex = bytes_to_hex(&minimal_jpeg_with_dimensions(1, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "jpegblip{",
        "\\",
        "picprop{",
        "\\",
        "sp{",
        "\\",
        "sn fHitTestFill}{",
        "\\",
        "sv 1}}{",
        "\\",
        "sp{",
        "\\",
        "sn fillShape}{",
        "\\",
        "sv 1}}{",
        "\\",
        "sp{",
        "\\",
        "sn fillUseRect}{",
        "\\",
        "sv 0}}{",
        "\\",
        "sp{",
        "\\",
        "sn fNoFillHitTest}{",
        "\\",
        "sv 0}}}",
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
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("passive image");

    assert_eq!(image.format, open_rtf_converter::model::ImageFormat::Jpeg);
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "inert picture metadata should not be unsupported: {:?}",
        parsed.diagnostics
    );
    for forbidden in ["fHitTestFill", "fillShape", "fillUseRect", "fNoFillHitTest"] {
        assert!(
            !text.contains(forbidden),
            "inert picture metadata leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 1);
    for forbidden in [
        b"fHitTestFill".as_slice(),
        b"fillShape",
        b"fillUseRect",
        b"fNoFillHitTest",
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
            "inert picture metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn visible_picture_color_mode_metadata_warns_without_payload_leakage() {
    let image = jpeg_with_large_ignored_app_segments(minimal_jpeg_with_dimensions(1, 1));
    let image_hex = bytes_to_hex(&image);
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "jpegblip{",
        "\\",
        "picprop{",
        "\\",
        "sp{",
        "\\",
        "sn pictureGray}{",
        "\\",
        "sv 1}}{",
        "\\",
        "sp{",
        "\\",
        "sn pictureBiLevel}{",
        "\\",
        "sv 1}}{",
        "\\",
        "sp{",
        "\\",
        "sn pictureBrightness}{",
        "\\",
        "sv 32768}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-visible-picture-metadata {",
        "\\",
        "object",
        "\\",
        "objdata 414243}}}}",
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
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("picture grayscale property approximated")),
        "JPEG grayscale should be represented by a passive PDF color-mode model: {:?}",
        parsed.diagnostics
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "JPEG picture bilevel property rendered as passive PDF decode with luminosity blend",
        )
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "JPEG picture brightness/contrast property preserved as bounded passive original image",
        )
    }));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("JPEG picture brightness/contrast property approximated")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("active content removed: object payload in metadata")
    }));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "picture color-mode metadata should not be unsupported: {:?}",
        parsed.diagnostics
    );
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
        open_rtf_converter::model::ImageFormat::JpegPassiveBilevel
    );
    for forbidden in [
        "pictureGray",
        "pictureBiLevel",
        "pFragments",
        "hostile-visible-picture-metadata",
        "objdata",
    ] {
        assert!(
            !text.contains(forbidden),
            "visible picture metadata leaked to text: {forbidden}"
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
            .any(|operation| operation.operator == "Do"),
        "valid JPEG should still render as a passive image"
    );
    assert!(
        content.operations.iter().any(|operation| {
            operation.operator == "gs"
                && operation
                    .operands
                    .first()
                    .and_then(|operand| operand.as_name().ok())
                    .is_some_and(|name| name == b"GSImageLuminosity")
        }),
        "JPEG color-mode metadata should apply passive luminosity graphics state"
    );
    assert!(
        output
            .pdf
            .windows(b"/ExtGState".len())
            .any(|window| window == b"/ExtGState")
    );
    assert!(
        output
            .pdf
            .windows(b"/BM /Luminosity".len())
            .any(|window| window == b"/BM /Luminosity")
    );
    assert!(
        output
            .pdf
            .windows(b"/Decode [1 0 1 0 1 0]".len())
            .any(|window| window == b"/Decode [1 0 1 0 1 0]")
    );
    for forbidden in [
        b"pictureGray".as_slice(),
        b"pictureBiLevel",
        b"pictureBrightness",
        b"pFragments",
        b"hostile-visible-picture-metadata",
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
            "visible picture metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn decodable_jpeg_bilevel_metadata_rewrites_safe_pixels_without_decode_fallback() {
    let image_hex = bytes_to_hex(&valid_rgb_jpeg_1x1());
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "jpegblip{",
        "\\",
        "picprop{",
        "\\",
        "sp{",
        "\\",
        "sn pictureBiLevel}{",
        "\\",
        "sv 1}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hidden-decodable-jpeg-bilevel-payload}}}",
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
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("image block");

    assert_eq!(image.format, ImageFormat::Rgb8);
    assert_eq!(image.width_px, 1);
    assert_eq!(image.height_px, 1);
    assert_eq!(image.bytes, vec![0, 0, 0]);
    assert_eq!(image.tone_adjustment, None);
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    for forbidden in [
        "pictureBiLevel",
        "pFragments",
        "hidden-decodable-jpeg-bilevel-payload",
    ] {
        assert!(
            !text.contains(forbidden),
            "JPEG bilevel metadata leaked to normalized text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "JPEG picture bilevel property rendered as bounded passive decoded image pixels",
        )
    }));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("JPEG picture bilevel property rendered as passive PDF decode")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
    assert!(
        !output
            .pdf
            .windows(b"/DCTDecode".len())
            .any(|window| window == b"/DCTDecode"),
        "decoded JPEG bilevel output should not preserve compressed JPEG bytes"
    );
    assert!(
        !output
            .pdf
            .windows(b"/BM /Luminosity".len())
            .any(|window| window == b"/BM /Luminosity"),
        "exact JPEG bilevel output should not require passive luminosity blend fallback"
    );
    assert!(
        !output
            .pdf
            .windows(b"/Decode [1 0 1 0 1 0]".len())
            .any(|window| window == b"/Decode [1 0 1 0 1 0]"),
        "exact JPEG bilevel output should not use approximate PDF decode inversion"
    );
    for forbidden in [
        b"pictureBiLevel".as_slice(),
        b"pFragments",
        b"hidden-decodable-jpeg-bilevel-payload",
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
            "JPEG bilevel metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn jpeg_brightness_metadata_renders_passive_decode_without_payload_leakage() {
    let image = jpeg_with_large_ignored_app_segments(minimal_jpeg_with_dimensions(1, 1));
    let image_hex = bytes_to_hex(&image);
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "jpegblip{",
        "\\",
        "picprop{",
        "\\",
        "sp{",
        "\\",
        "sn pictureBrightness}{",
        "\\",
        "sv 32768}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hidden-tone-payload {",
        "\\",
        "object",
        "\\",
        "objdata 414243}}}}",
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
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("image block");

    assert_eq!(image.format, ImageFormat::Jpeg);
    assert_eq!(
        image.tone_adjustment,
        Some(ImageToneAdjustment {
            decode_low: 1.0,
            decode_high: 1.0,
        })
    );
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    for forbidden in [
        "pictureBrightness",
        "pFragments",
        "hidden-tone-payload",
        "objdata",
    ] {
        assert!(
            !text.contains(forbidden),
            "picture tone metadata leaked to normalized text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("picture brightness/contrast property approximated")),
        "JPEG brightness should normalize to passive PDF decode metadata: {:?}",
        parsed.diagnostics
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("JPEG picture brightness/contrast property rendered as passive PDF decode")
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
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
    assert!(
        output
            .pdf
            .windows(b"/Decode [1 1 1 1 1 1]".len())
            .any(|window| window == b"/Decode [1 1 1 1 1 1]"),
        "JPEG tone metadata should render as a passive PDF decode array"
    );
    for forbidden in [
        b"pictureBrightness".as_slice(),
        b"pFragments",
        b"hidden-tone-payload",
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
            "picture tone metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn grayscale_jpeg_brightness_metadata_renders_passive_decode_without_payload_leakage() {
    let image = jpeg_with_large_ignored_app_segments(minimal_grayscale_jpeg_with_dimensions(1, 1));
    let image_hex = bytes_to_hex(&image);
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "jpegblip{",
        "\\",
        "picprop{",
        "\\",
        "sp{",
        "\\",
        "sn pictureBrightness}{",
        "\\",
        "sv -32768}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hidden-gray-tone-payload}}}",
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
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("image block");

    assert_eq!(image.format, ImageFormat::JpegGrayscale);
    assert_eq!(
        image.tone_adjustment,
        Some(ImageToneAdjustment {
            decode_low: 0.0,
            decode_high: 0.0,
        })
    );
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    for forbidden in [
        "pictureBrightness",
        "pFragments",
        "hidden-gray-tone-payload",
    ] {
        assert!(
            !text.contains(forbidden),
            "grayscale JPEG tone metadata leaked to normalized text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("picture brightness/contrast property approximated")),
        "grayscale JPEG brightness should normalize to passive PDF decode metadata: {:?}",
        parsed.diagnostics
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("JPEG picture brightness/contrast property rendered as passive PDF decode")
    }));

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
            .windows(b"/Decode [0 0]".len())
            .any(|window| window == b"/Decode [0 0]"),
        "grayscale JPEG tone metadata should render as a one-channel passive PDF decode array"
    );
    for forbidden in [
        b"pictureBrightness".as_slice(),
        b"pFragments",
        b"hidden-gray-tone-payload",
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
            "grayscale JPEG tone metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn cmyk_jpeg_brightness_metadata_renders_passive_decode_without_payload_leakage() {
    let image = jpeg_with_large_ignored_app_segments(minimal_cmyk_jpeg_with_dimensions(1, 1));
    let image_hex = bytes_to_hex(&image);
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "jpegblip{",
        "\\",
        "picprop{",
        "\\",
        "sp{",
        "\\",
        "sn pictureBrightness}{",
        "\\",
        "sv 32768}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hidden-cmyk-tone-payload}}}",
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
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("image block");

    assert_eq!(image.format, ImageFormat::JpegCmyk);
    assert_eq!(
        image.tone_adjustment,
        Some(ImageToneAdjustment {
            decode_low: 1.0,
            decode_high: 1.0,
        })
    );
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    for forbidden in [
        "pictureBrightness",
        "pFragments",
        "hidden-cmyk-tone-payload",
    ] {
        assert!(
            !text.contains(forbidden),
            "CMYK JPEG tone metadata leaked to normalized text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("picture brightness/contrast property approximated")),
        "CMYK JPEG brightness should normalize to passive PDF decode metadata: {:?}",
        parsed.diagnostics
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("JPEG picture brightness/contrast property rendered as passive PDF decode")
    }));

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
            .windows(b"/ColorSpace /DeviceCMYK".len())
            .any(|window| window == b"/ColorSpace /DeviceCMYK")
    );
    assert!(
        output
            .pdf
            .windows(b"/Decode [1 1 1 1 1 1 1 1]".len())
            .any(|window| window == b"/Decode [1 1 1 1 1 1 1 1]"),
        "CMYK JPEG tone metadata should render as a four-channel passive PDF decode array"
    );
    for forbidden in [
        b"pictureBrightness".as_slice(),
        b"pFragments",
        b"hidden-cmyk-tone-payload",
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
            "CMYK JPEG tone metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn cmyk_jpeg_grayscale_and_brightness_render_passive_decode_without_payload_leakage() {
    let image = jpeg_with_large_ignored_app_segments(minimal_cmyk_jpeg_with_dimensions(1, 1));
    let image_hex = bytes_to_hex(&image);
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "jpegblip{",
        "\\",
        "picprop{",
        "\\",
        "sp{",
        "\\",
        "sn pictureGray}{",
        "\\",
        "sv 1}}{",
        "\\",
        "sp{",
        "\\",
        "sn pictureBrightness}{",
        "\\",
        "sv 32768}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hidden-cmyk-gray-tone-payload}}}",
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
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("image block");

    assert_eq!(image.format, ImageFormat::JpegCmykPassiveGrayscale);
    assert_eq!(
        image.tone_adjustment,
        Some(ImageToneAdjustment {
            decode_low: 1.0,
            decode_high: 1.0,
        })
    );
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    for forbidden in [
        "pictureGray",
        "pictureBrightness",
        "pFragments",
        "hidden-cmyk-gray-tone-payload",
    ] {
        assert!(
            !text.contains(forbidden),
            "CMYK JPEG grayscale/tone metadata leaked to normalized text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("JPEG picture brightness/contrast property rendered as passive PDF decode")
    }));

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
            .windows(b"/ColorSpace /DeviceCMYK".len())
            .any(|window| window == b"/ColorSpace /DeviceCMYK")
    );
    assert!(
        output
            .pdf
            .windows(b"/BM /Luminosity".len())
            .any(|window| window == b"/BM /Luminosity")
    );
    assert!(
        output
            .pdf
            .windows(b"/Decode [1 1 1 1 1 1 1 1]".len())
            .any(|window| window == b"/Decode [1 1 1 1 1 1 1 1]"),
        "CMYK JPEG grayscale/tone metadata should keep the four-channel passive PDF decode array"
    );
    for forbidden in [
        b"pictureGray".as_slice(),
        b"pictureBrightness",
        b"pFragments",
        b"hidden-cmyk-gray-tone-payload",
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
            "CMYK JPEG grayscale/tone metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn cmyk_jpeg_bilevel_metadata_renders_passive_decode_without_payload_leakage() {
    let image = jpeg_with_large_ignored_app_segments(minimal_cmyk_jpeg_with_dimensions(1, 1));
    let image_hex = bytes_to_hex(&image);
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "jpegblip{",
        "\\",
        "picprop{",
        "\\",
        "sp{",
        "\\",
        "sn pictureBiLevel}{",
        "\\",
        "sv 1}}{",
        "\\",
        "sp{",
        "\\",
        "sn pictureBrightness}{",
        "\\",
        "sv 32768}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hidden-cmyk-bilevel-payload {",
        "\\",
        "object",
        "\\",
        "objdata 414243}}}}",
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
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("image block");

    assert_eq!(image.format, ImageFormat::JpegCmykPassiveBilevel);
    assert_eq!(image.tone_adjustment, None);
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "JPEG picture bilevel property rendered as passive PDF decode with luminosity blend",
        )
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "JPEG picture brightness/contrast property preserved as bounded passive original image",
        )
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("active content removed: object payload in metadata")
    }));
    for forbidden in [
        "pictureBiLevel",
        "pictureBrightness",
        "pFragments",
        "hidden-cmyk-bilevel-payload",
        "objdata",
    ] {
        assert!(
            !text.contains(forbidden),
            "CMYK JPEG bilevel metadata leaked to normalized text: {forbidden}"
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
            .any(|operation| operation.operator == "Do"),
        "CMYK JPEG bilevel metadata should still render as a passive image"
    );
    assert!(
        content.operations.iter().any(|operation| {
            operation.operator == "gs"
                && operation
                    .operands
                    .first()
                    .and_then(|operand| operand.as_name().ok())
                    .is_some_and(|name| name == b"GSImageLuminosity")
        }),
        "CMYK JPEG bilevel metadata should apply passive luminosity graphics state"
    );
    assert!(
        output
            .pdf
            .windows(b"/ColorSpace /DeviceCMYK".len())
            .any(|window| window == b"/ColorSpace /DeviceCMYK")
    );
    assert!(
        output
            .pdf
            .windows(b"/BM /Luminosity".len())
            .any(|window| window == b"/BM /Luminosity")
    );
    assert!(
        output
            .pdf
            .windows(b"/Decode [1 0 1 0 1 0 1 0]".len())
            .any(|window| window == b"/Decode [1 0 1 0 1 0 1 0]")
    );
    for forbidden in [
        b"pictureBiLevel".as_slice(),
        b"pictureBrightness",
        b"pFragments",
        b"hidden-cmyk-bilevel-payload",
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
            "CMYK JPEG bilevel metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn rgb_jpeg_grayscale_and_brightness_render_passive_decode_without_payload_leakage() {
    let image = jpeg_with_large_ignored_app_segments(minimal_jpeg_with_dimensions(1, 1));
    let image_hex = bytes_to_hex(&image);
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "jpegblip{",
        "\\",
        "picprop{",
        "\\",
        "sp{",
        "\\",
        "sn pictureGray}{",
        "\\",
        "sv 1}}{",
        "\\",
        "sp{",
        "\\",
        "sn pictureBrightness}{",
        "\\",
        "sv 32768}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hidden-rgb-gray-tone-payload}}}",
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
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("image block");

    assert_eq!(image.format, ImageFormat::JpegPassiveGrayscale);
    assert_eq!(
        image.tone_adjustment,
        Some(ImageToneAdjustment {
            decode_low: 1.0,
            decode_high: 1.0,
        })
    );
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    for forbidden in [
        "pictureGray",
        "pictureBrightness",
        "pFragments",
        "hidden-rgb-gray-tone-payload",
    ] {
        assert!(
            !text.contains(forbidden),
            "RGB JPEG grayscale/tone metadata leaked to normalized text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("picture grayscale property approximated")
                && !diagnostic
                    .message
                    .contains("picture brightness/contrast property approximated")
        }),
        "RGB JPEG grayscale and brightness should normalize to passive PDF metadata: {:?}",
        parsed.diagnostics
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("JPEG picture brightness/contrast property rendered as passive PDF decode")
    }));

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
            .windows(b"/Decode [1 1 1 1 1 1]".len())
            .any(|window| window == b"/Decode [1 1 1 1 1 1]")
    );
    assert!(
        output
            .pdf
            .windows(b"/BM /Luminosity".len())
            .any(|window| window == b"/BM /Luminosity")
    );
    for forbidden in [
        b"pictureGray".as_slice(),
        b"pictureBrightness",
        b"pFragments",
        b"hidden-rgb-gray-tone-payload",
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
            "RGB JPEG grayscale/tone metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn dib_picture_grayscale_metadata_renders_passively_without_payload_leakage() {
    let image_hex = bytes_to_hex(&minimal_24bit_dib_with_dimensions(2, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "dibitmap0{",
        "\\",
        "picprop{",
        "\\",
        "sp{",
        "\\",
        "sn pictureGray}{",
        "\\",
        "sv 1}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hidden-fragment}}}",
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
    assert!(!text.contains("pictureGray"));
    assert!(!text.contains("pFragments"));
    assert!(!text.contains("hidden-fragment"));
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("picture grayscale property approximated")),
        "RGB DIB grayscale should be applied before the safe image model: {:?}",
        parsed.diagnostics
    );
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
    assert_eq!(image.bytes, vec![54, 54, 54, 182, 182, 182]);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
    for forbidden in [
        b"dibitmap".as_slice(),
        b"picprop",
        b"pictureGray",
        b"pFragments",
        b"hidden-fragment",
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
            "DIB grayscale metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn word_compatible_parameterless_dib_is_suppressed_inline_without_payload_leakage() {
    let image_hex = bytes_to_hex(&minimal_24bit_dib_with_dimensions(2, 1));
    let input = format!(
        "{{\\rtf1 before{{\\pict\\dibitmap\\picwgoal720\\pichgoal720 {image_hex}}}after\\par}}"
    );
    let parsed = parse_rtf_bytes(input.as_bytes()).unwrap();
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::Image(_)))
    );
    assert!(collect_text(&parsed.document).contains("before\u{200b}after"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("parameterless DIB picture suppressed")
    }));

    let output = convert_rtf_to_pdf(input.as_bytes(), &ConvertOptions::default()).unwrap();
    assert!(
        !output
            .pdf
            .windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image")
    );
    for forbidden in [b"dibitmap".as_slice(), image_hex.as_bytes()] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn dib_picture_bilevel_metadata_renders_passively_without_payload_leakage() {
    let image_hex = bytes_to_hex(&minimal_24bit_dib_with_dimensions(2, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "dibitmap0{",
        "\\",
        "picprop{",
        "\\",
        "sp{",
        "\\",
        "sn pictureBiLevel}{",
        "\\",
        "sv 1}}{",
        "\\",
        "sp{",
        "\\",
        "sn pictureGray}{",
        "\\",
        "sv 1}}}",
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
    assert!(!text.contains("pictureBiLevel"));
    assert!(!text.contains("pictureGray"));
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("picture bilevel property approximated")
                && !diagnostic
                    .message
                    .contains("picture grayscale property approximated")
        }),
        "RGB DIB bilevel should be applied before the safe image model: {:?}",
        parsed.diagnostics
    );
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
    assert_eq!(image.bytes, vec![0, 0, 0, 255, 255, 255]);

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
    for forbidden in [
        b"dibitmap".as_slice(),
        b"picprop",
        b"pictureBiLevel",
        b"pictureGray",
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
            "DIB bilevel metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn compressed_dib_picture_payloads_render_passively_without_raw_dib_leakage() {
    let jpeg = minimal_jpeg_with_dimensions(2, 1);
    let png = minimal_grayscale_png_with_dimensions(2, 1);
    let mut jpeg_dib = minimal_compressed_dib_with_payload(2, 1, 4, &jpeg);
    let mut png_dib = minimal_compressed_dib_with_payload(2, 1, 5, &png);
    jpeg_dib.extend_from_slice(b"TRAILING-DIB-JPEG-PAYLOAD /JavaScript");
    png_dib.extend_from_slice(b"TRAILING-DIB-PNG-PAYLOAD /EmbeddedFile");
    let jpeg_hex = bytes_to_hex(&jpeg_dib);
    let png_hex = bytes_to_hex(&png_dib);

    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "dibitmap0",
        "\\",
        "picwgoal720",
        "\\",
        "pichgoal720 ",
        jpeg_hex.as_str(),
        "}{",
        "\\",
        "pict",
        "\\",
        "dibitmap0",
        "\\",
        "picwgoal720",
        "\\",
        "pichgoal720 ",
        png_hex.as_str(),
        "} after",
        "\\",
        "par}",
    ]);

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    let images: Vec<_> = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .collect();
    assert_eq!(images.len(), 2);
    assert_eq!(images[0].format, ImageFormat::Jpeg);
    assert_eq!(images[0].width_px, 2);
    assert_eq!(images[0].height_px, 1);
    assert_eq!(images[0].bytes, jpeg);
    assert_eq!(images[1].format, ImageFormat::PngGrayscale);
    assert_eq!(images[1].width_px, 2);
    assert_eq!(images[1].height_px, 1);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let image_paints = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "Do")
        .count();
    assert!(image_paints >= 2, "both compressed DIBs should render");
    for forbidden in [
        b"dibitmap".as_slice(),
        b"TRAILING-DIB-JPEG-PAYLOAD",
        b"TRAILING-DIB-PNG-PAYLOAD",
        b"89504e470d0a1a0a",
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
            "compressed DIB payload leaked to PDF: {:?}",
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
    assert!(!text.contains("[Image skipped: malformed JPEG]"));
    assert!(text.contains("after"));
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::Image(_) | Block::Placeholder(_)))
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("malformed JPEG picture suppressed for Word-compatible passive rendering")
    }));
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
    assert!(!rendered_text.contains("[Image skipped: malformed JPEG]"));
    assert!(rendered_text.contains("after"));
    for forbidden in [
        payload.as_slice(),
        b"objdata".as_slice(),
        b"414243",
        b"/DCTDecode",
        b"/Subtype /Image",
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
        "dibitmap0",
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
fn sixteen_bit_dib_picture_renders_passively_without_payload_leakage() {
    let image_hex = bytes_to_hex(&minimal_16bit_dib_with_dimensions(2, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "dibitmap0",
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
        .expect("16-bit DIB image block");
    assert_eq!(image.format, open_rtf_converter::model::ImageFormat::Rgb8);
    assert_eq!(image.width_px, 2);
    assert_eq!(image.height_px, 1);
    assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0]);
    assert!(!text.contains("dibitmap"));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
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
    for forbidden in [
        b"dibitmap".as_slice(),
        image_hex.as_bytes(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden 16-bit DIB content leaked to PDF: {:?}",
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
        "dibitmap0",
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
fn rle8_dib_picture_renders_passively_without_payload_leakage() {
    let image_hex = bytes_to_hex(&minimal_rle8_dib_with_dimensions(3, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "dibitmap0",
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
        .expect("RLE8 DIB image block");
    assert_eq!(image.format, open_rtf_converter::model::ImageFormat::Rgb8);
    assert_eq!(image.width_px, 3);
    assert_eq!(image.height_px, 1);
    assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0, 255, 0, 0]);
    assert!(!text.contains("dibitmap"));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
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
    for forbidden in [
        b"dibitmap".as_slice(),
        image_hex.as_bytes(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden RLE8 DIB content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn rle4_dib_picture_renders_passively_without_payload_leakage() {
    let image_hex = bytes_to_hex(&minimal_rle4_dib_with_dimensions(3, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "dibitmap0",
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
        .expect("RLE4 DIB image block");
    assert_eq!(image.format, open_rtf_converter::model::ImageFormat::Rgb8);
    assert_eq!(image.width_px, 3);
    assert_eq!(image.height_px, 1);
    assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0, 255, 0, 0]);
    assert!(!text.contains("dibitmap"));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
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
    for forbidden in [
        b"dibitmap".as_slice(),
        image_hex.as_bytes(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden RLE4 DIB content leaked to PDF: {:?}",
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
        "dibitmap0",
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
        "dibitmap0",
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
fn shape_picture_result_uses_bounded_shape_frame_without_payload_leakage() {
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
        "shpinst",
        "\\",
        "shpleft720",
        "\\",
        "shptop360",
        "\\",
        "shpright2160",
        "\\",
        "shpbottom1080{",
        "\\",
        "shpwr2",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hidden-frame-payload}}}{",
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
        "nonshppict FALLBACK {",
        "\\",
        "object",
        "\\",
        "objdata 414243}}} After",
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
    let text = collect_text(&parsed.document);

    assert_eq!(placement.left_twips, 720);
    assert_eq!(placement.top_twips, 360);
    assert_eq!(placement.width_twips, 1440);
    assert_eq!(placement.height_twips, 720);
    assert!(placement.text_wrap);
    assert_eq!(placement.wrap_margin_left_twips, 180);
    assert_eq!(placement.wrap_margin_right_twips, 180);
    assert_eq!(placement.wrap_margin_top_twips, 0);
    assert_eq!(placement.wrap_margin_bottom_twips, 0);
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "hidden-frame-payload",
        "pFragments",
        "nonshppict",
        "FALLBACK",
        "objdata",
        "414243",
        "shppict",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden shape picture frame content leaked to text: {forbidden}"
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
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("bounded passive shape frame") })
    );
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(decoded_pdf_text(&content).contains("Before"));
    assert!(decoded_pdf_text(&content).contains("After"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "Do"),
        "shape picture frame should still render passive image bytes"
    );
    for forbidden in [
        b"hidden-frame-payload".as_slice(),
        b"pFragments",
        b"nonshppict",
        b"FALLBACK",
        b"objdata",
        b"414243",
        b"shppict",
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
            "forbidden shape picture frame content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn shape_picture_grayscale_metadata_updates_safe_image_without_payload_leakage() {
    let image_hex = bytes_to_hex(&minimal_rgb_png(&[[255, 0, 0]]));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before{",
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
        "shpbottom1080{",
        "\\",
        "sp{",
        "\\",
        "sn pictureGray}{",
        "\\",
        "sv 1}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hidden-shape-grayscale-picture-payload}}}{",
        "\\",
        "*",
        "\\",
        "shppict{",
        "\\",
        "pict",
        "\\",
        "pngblip",
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
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("shape grayscale picture image");

    assert_eq!(image.format, ImageFormat::Png);
    assert!(image.placement.is_some());
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("picture grayscale property approximated")),
        "shape picture grayscale should be applied to stored scanlines: {:?}",
        parsed.diagnostics
    );
    let scanlines = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 4)
        .expect("shape grayscale PNG scanline");
    assert_eq!(scanlines, vec![0, 54, 54, 54]);
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "pictureGray",
        "pFragments",
        "hidden-shape-grayscale-picture-payload",
        "shppict",
        "pngblip",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "shape grayscale picture metadata leaked to normalized text: {forbidden}"
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
    assert_eq!(parsed_pdf.get_pages().len(), 1);
    assert!(
        output
            .pdf
            .windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image"),
        "shape grayscale picture should still render a passive image XObject"
    );
    for forbidden in [
        b"pictureGray".as_slice(),
        b"pFragments",
        b"hidden-shape-grayscale-picture-payload",
        b"shppict",
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
            "shape grayscale picture metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn shape_picture_bilevel_metadata_updates_safe_image_without_payload_leakage() {
    let image_hex = bytes_to_hex(&minimal_rgb_png(&[[255, 0, 0]]));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before{",
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
        "shpbottom1080{",
        "\\",
        "sp{",
        "\\",
        "sn pictureBiLevel}{",
        "\\",
        "sv 1}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hidden-shape-bilevel-picture-payload}}}{",
        "\\",
        "*",
        "\\",
        "shppict{",
        "\\",
        "pict",
        "\\",
        "pngblip",
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
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("shape bilevel picture image");

    assert_eq!(image.format, ImageFormat::Png);
    assert!(image.placement.is_some());
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("picture bilevel property approximated")
                && !diagnostic
                    .message
                    .contains("picture grayscale property approximated")
        }),
        "shape picture bilevel should be applied to stored scanlines: {:?}",
        parsed.diagnostics
    );
    let scanlines = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 4)
        .expect("shape bilevel PNG scanline");
    assert_eq!(scanlines, vec![0, 0, 0, 0]);
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "pictureBiLevel",
        "pFragments",
        "hidden-shape-bilevel-picture-payload",
        "shppict",
        "pngblip",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "shape bilevel picture metadata leaked to normalized text: {forbidden}"
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
    assert_eq!(parsed_pdf.get_pages().len(), 1);
    assert!(
        output
            .pdf
            .windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image"),
        "shape bilevel picture should still render a passive image XObject"
    );
    for forbidden in [
        b"pictureBiLevel".as_slice(),
        b"pFragments",
        b"hidden-shape-bilevel-picture-payload",
        b"shppict",
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
            "shape bilevel picture metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn shape_picture_result_uses_passive_author_wrap_distances_without_payload_leakage() {
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
        "shpwr2{",
        "\\",
        "sp{",
        "\\",
        "sn dxWrapDistLeft}{",
        "\\",
        "sv 152400}}{",
        "\\",
        "sp{",
        "\\",
        "sn dxWrapDistRight}{",
        "\\",
        "sv 457200}}{",
        "\\",
        "sp{",
        "\\",
        "sn dyWrapDistTop}{",
        "\\",
        "sv 304800}}{",
        "\\",
        "sp{",
        "\\",
        "sn dyWrapDistBottom}{",
        "\\",
        "sv 609600}}}{",
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
        "}}} After",
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
    let text = collect_text(&parsed.document);

    assert!(placement.text_wrap);
    assert_eq!(placement.wrap_margin_left_twips, 240);
    assert_eq!(placement.wrap_margin_right_twips, 720);
    assert_eq!(placement.wrap_margin_top_twips, 480);
    assert_eq!(placement.wrap_margin_bottom_twips, 960);
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "dxWrapDistLeft",
        "dxWrapDistRight",
        "dyWrapDistTop",
        "dyWrapDistBottom",
        "152400",
        "457200",
        "304800",
        "609600",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden shape wrap-distance metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unsupported/active drawing properties")),
        "shape wrap distances should be consumed as passive metadata: {:?}",
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
    for forbidden in [
        b"dxWrapDistLeft".as_slice(),
        b"dxWrapDistRight",
        b"dyWrapDistTop",
        b"dyWrapDistBottom",
        b"152400",
        b"457200",
        b"304800",
        b"609600",
        b"shppict",
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
            "forbidden shape wrap-distance metadata leaked to PDF: {:?}",
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
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("unsupported picture format replaced with a passive geometry placeholder")
    }));

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
fn word_compatible_windows_bitmap_is_suppressed_without_payload_leakage() {
    let input = br"{\rtf1 before {\pict\wbitmap0\wbmplanes1\wbmbitspixel24\wbmwidthbytes6\picw2\pich2\picwgoal720\pichgoal720 ff0000ffffff0000ff00ff00} after\par}";
    let parsed = parse_rtf_bytes(input).unwrap();
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::Image(_)))
    );
    assert!(collect_text(&parsed.document).contains("before"));
    assert!(collect_text(&parsed.document).contains("after"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("legacy Windows bitmap picture suppressed")
    }));

    let output = convert_rtf_to_pdf(input, &ConvertOptions::default()).unwrap();
    assert!(
        !output
            .pdf
            .windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image")
    );
    for forbidden in [b"wbitmap".as_slice(), b"ff0000ffffff0000ff00ff00"] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn word_compatible_block_windows_bitmap_retains_empty_paragraph_flow() {
    let input = br"{\rtf1 before\par {\pict\wbitmap0\wbmplanes1\wbmbitspixel24\wbmwidthbytes6\picw2\pich2 ff0000ffffff0000ff00ff00}\par after\par}";
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
    assert_eq!(paragraphs.len(), 3);
    assert_eq!(paragraphs[1].runs.len(), 1);
    assert_eq!(paragraphs[1].runs[0].text, "\u{200b}");
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::Image(_)))
    );
}

#[test]
fn strict_spec_windows_bitmap_renders_as_bounded_passive_rgb_without_payload_leakage() {
    let bitmap_hex = "ff0000ffffff0000ff00ff00";
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "wbitmap0",
        "\\",
        "wbmplanes1",
        "\\",
        "wbmbitspixel24",
        "\\",
        "wbmwidthbytes6",
        "\\",
        "picw2",
        "\\",
        "pich2",
        "\\",
        "picwgoal720",
        "\\",
        "pichgoal720 ",
        bitmap_hex,
        "} after",
        "\\",
        "par}",
    ]);
    let strict_options = RtfParseOptions {
        compatibility_mode: CompatibilityMode::StrictSpec,
        ..RtfParseOptions::default()
    };
    let parsed = parse_rtf_bytes_with_options(&input, &strict_options).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("Windows bitmap image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::Rgb8);
    assert_eq!(image.width_px, 2);
    assert_eq!(image.height_px, 2);
    assert_eq!(
        image.bytes,
        vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
        "Windows bitmap BGR rows should normalize to top-down passive RGB"
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("Windows bitmap picture rendered as bounded passive RGB image")
    }));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported picture format")),
        "parseable Windows bitmap should not fall back to placeholder: {:?}",
        parsed.diagnostics
    );
    for forbidden in [
        "wbitmap",
        "wbmplanes",
        "wbmbitspixel",
        "wbmwidthbytes",
        bitmap_hex,
    ] {
        assert!(
            !text.contains(forbidden),
            "Windows bitmap metadata leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            parse_options: strict_options,
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
        output
            .pdf
            .windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image"),
        "Windows bitmap should render as a passive PDF image XObject"
    );
    for forbidden in [
        b"wbitmap".as_slice(),
        b"wbmplanes",
        b"wbmbitspixel",
        b"wbmwidthbytes",
        bitmap_hex.as_bytes(),
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
            "Windows bitmap metadata or active marker leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_picture_hex_data_becomes_geometry_placeholder_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "pngblip",
        "\\",
        "picw100",
        "\\",
        "pich50",
        "\\",
        "picwgoal720",
        "\\",
        "pichgoal360 4142XX434448494444454e5041594c4f4144} after",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("HIDDENPAYLOAD"));
    assert!(
        parsed.document.blocks.iter().any(|block| matches!(
            block,
            Block::Image(image)
                if image.format == ImageFormat::Placeholder
                    && image.bytes.is_empty()
                    && image.natural_width_px_hint == Some(100)
                    && image.natural_height_px_hint == Some(50)
                    && image.display_width_twips == Some(720)
                    && image.display_height_twips == Some(360)
        )),
        "malformed picture hex should preserve bounded passive geometry"
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("non-hex picture data replaced with bounded passive geometry placeholder")
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(content.operations.iter().any(|operation| {
        operation.operator == "re" || operation.operator == "S" || operation.operator == "Tj"
    }));
    for forbidden in [
        b"pngblip".as_slice(),
        b"4142XX",
        b"HIDDENPAYLOAD",
        b"/Subtype /Image",
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
            "malformed picture payload leaked to PDF: {:?}",
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
fn wmf_viewportext_scales_text_and_raster_coordinates_without_payload_leakage() {
    let mut viewport = wmf_yx_record(0x020e, 40, 160);
    viewport.extend_from_slice(b"WMF-VIEWPORTEXT-TEXT-RASTER /JavaScript /EmbeddedFile /Launch");
    viewport.resize(viewport.len().next_multiple_of(2), 0);
    let viewport_len = (viewport.len() / 2) as u32;
    write_test_le_u32(&mut viewport, 0, viewport_len);
    let mut raster = wmf_patblt_record(20, 10, 60, 40, 0x0000_0042);
    raster.extend_from_slice(b"WMF-VIEWPORTEXT-RASTER-PAYLOAD /JavaScript");
    raster.resize(raster.len().next_multiple_of(2), 0);
    let raster_len = (raster.len() / 2) as u32;
    write_test_le_u32(&mut raster, 0, raster_len);
    let records = [
        viewport,
        wmf_exttextout_record(40, 20, "Scaled", 0, None),
        raster,
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
        .expect("passive WMF scaled text/raster viewport image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::Text {
                x: 40.0,
                y: 10.0,
                text,
                ..
            } if text == "Scaled"
        )
    }));
    assert!(image.vector_commands.iter().any(|command| {
        matches!(
            command,
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 5.0,
                right: 80.0,
                bottom: 25.0,
                fill_color: Some(color),
                ..
            } if color.red == 0 && color.green == 0 && color.blue == 0
        )
    }));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "WMF-VIEWPORTEXT-TEXT-RASTER",
        "WMF-VIEWPORTEXT-RASTER-PAYLOAD",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF viewport text/raster payload leaked to normalized text: {forbidden}"
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
        b"WMF-VIEWPORTEXT-TEXT-RASTER",
        b"WMF-VIEWPORTEXT-RASTER-PAYLOAD",
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
            "WMF viewport text/raster payload leaked to PDF: {:?}",
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
fn word_wmf_patcopy_dibbitblt_renders_recovered_brush_rectangle_without_payload_leakage() {
    let wmf_hex = concat!(
        "0100090000033f0000000200170000000000",
        "050000000c026400c800",
        "07000000fc020000785028000000",
        "040000002d010000",
        "0c00000040092100f000000000003c00460014000a000000",
        "1700000040092000cc0000000019002d001e00230000005241572d444942424954424c542d5041594c4f41440000",
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
        .expect("WMF DIBBITBLT vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left,
            top,
            right,
            bottom,
            stroke_color: None,
            fill_color: Some(Color {
                red: 120,
                green: 80,
                blue: 40
            }),
            ..
        } if (*left - 0.0).abs() < 0.01
            && (*top - 10.884_12).abs() < 0.01
            && (*right - 19.841_17).abs() < 0.01
            && (*bottom - 85.714_2).abs() < 0.01
    ));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("1 unsupported record(s) skipped")
    }));
    for forbidden in [
        "wmetafile",
        "0940",
        "RAW-DIBBITBLT-PAYLOAD",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF DIBBITBLT internals leaked to text: {forbidden}"
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
        "WMF PATCOPY DIBBITBLT should render as passive PDF rectangle path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "f*" | "B" | "B*")),
        "WMF PATCOPY DIBBITBLT should render as a passive PDF fill"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"0940",
        b"RAW-DIBBITBLT-PAYLOAD",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF DIBBITBLT leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn strict_spec_wmf_patcopy_dibstretchblt_renders_passive_brush_rectangle() {
    let wmf_hex = concat!(
        "010009000003420000000200190000000000",
        "050000000c026400c800",
        "07000000fc0200002846a0000000",
        "040000002d010000",
        "0d000000410b2100f00000000000000000002800500019000f00",
        "19000000410b2000cc00000000000000000019002d001e0023005241572d444942535452455443482d5041594c4f41440000",
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
        .expect("WMF DIBSTRETCHBLT vector preview image");

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
                green: 70,
                blue: 160
            }),
            ..
        }
    ));
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in [
        "wmetafile",
        "0b41",
        "RAW-DIBSTRETCH-PAYLOAD",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF DIBSTRETCHBLT internals leaked to text: {forbidden}"
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
        "WMF PATCOPY DIBSTRETCHBLT should render as passive PDF rectangle path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "f*" | "B" | "B*")),
        "WMF PATCOPY DIBSTRETCHBLT should render as a passive PDF fill"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"0b41",
        b"RAW-DIBSTRETCH-PAYLOAD",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF DIBSTRETCHBLT leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn strict_spec_wmf_patcopy_stretchdib_renders_passive_brush_rectangle() {
    let wmf_hex = concat!(
        "010009000003430000000200190000000000",
        "050000000c026400c800",
        "07000000fc020000a04628000000",
        "040000002d010000",
        "0e000000430f2100f000000000000000000000002800500019000f00",
        "19000000430f2000cc000000000000000000000019002d001e0023005241572d535452455443484449422d5041594c4f4144",
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
        .expect("WMF STRETCHDIB vector preview image");

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
                red: 160,
                green: 70,
                blue: 40
            }),
            ..
        }
    ));
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in [
        "wmetafile",
        "0f43",
        "RAW-STRETCHDIB-PAYLOAD",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF STRETCHDIB internals leaked to text: {forbidden}"
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
        "WMF PATCOPY STRETCHDIB should render as passive PDF rectangle path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "f*" | "B" | "B*")),
        "WMF PATCOPY STRETCHDIB should render as a passive PDF fill"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        b"0f43",
        b"RAW-STRETCHDIB-PAYLOAD",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF STRETCHDIB leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_srccopy_stretchdib_renders_passive_image_without_payload_leakage() {
    let dib = minimal_24bit_dib_with_dimensions(2, 1);
    let record = wmf_stretchdib_dib_record(15, 25, 80, 40, 0x00cc_0020, &dib);
    let wmf = minimal_wmf_with_records(200, 100, &[record]);
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
        .expect("WMF SRCCOPY STRETCHDIB vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    let StaticImageVectorCommand::RasterImage {
        left,
        top,
        right,
        bottom,
        image: raster,
    } = &image.vector_commands[0]
    else {
        panic!("expected passive raster command");
    };
    assert!((*left - 15.079_498).abs() < 0.000_1);
    assert!((*top - 26.530_811).abs() < 0.000_1);
    assert!((*right - 93.652_44).abs() < 0.000_1);
    assert!((*bottom - 69.388_46).abs() < 0.000_1);
    assert_eq!(raster.format, ImageFormat::Rgb8);
    assert_eq!((raster.width_px, raster.height_px), (2, 1));
    assert_eq!(raster.bytes, vec![255, 0, 0, 0, 255, 0]);
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in ["wmetafile", "0f43", "cc0020", "JavaScript", "EmbeddedFile"] {
        assert!(
            !text.contains(forbidden),
            "WMF SRCCOPY STRETCHDIB internals leaked to text: {forbidden}"
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
        output
            .pdf
            .windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image"),
        "WMF SRCCOPY STRETCHDIB should render a passive image XObject"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "Do"),
        "WMF SRCCOPY STRETCHDIB should invoke a passive PDF image XObject"
    );
    for forbidden in [
        b"wmetafile".as_slice(),
        b"0f43",
        b"cc0020",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF SRCCOPY STRETCHDIB leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn strict_spec_wmf_setdib_to_dev_renders_passive_image_without_payload_leakage() {
    let mut dib = minimal_24bit_dib_with_dimensions(2, 1);
    dib.extend_from_slice(b"TRAILING-WMF-SETDIBTODEV /JavaScript");
    let mut partial_dib = minimal_24bit_dib_with_rgb_pixels(
        2,
        3,
        &[
            [10, 20, 30],
            [40, 50, 60],
            [70, 80, 90],
            [100, 110, 120],
            [130, 140, 150],
            [160, 170, 180],
        ],
    );
    partial_dib.extend_from_slice(b"TRAILING-WMF-PARTIAL-SETDIBTODEV /JavaScript");
    let png_rows = [
        [10, 20, 30],
        [40, 50, 60],
        [70, 80, 90],
        [100, 110, 120],
        [130, 140, 150],
        [160, 170, 180],
    ];
    let mut top_down_compressed_png_dib =
        minimal_compressed_dib_with_payload(2, 3, 5, &minimal_rgb_png_with_rows(2, 3, &png_rows));
    write_test_le_i32(&mut top_down_compressed_png_dib, 8, -3);
    top_down_compressed_png_dib
        .extend_from_slice(b"TRAILING-WMF-TOPDOWN-COMPRESSED-SETDIBTODEV /Launch");
    let jpeg = minimal_jpeg_with_dimensions(2, 1);
    let mut jpeg_payload = jpeg.clone();
    jpeg_payload.extend_from_slice(b"TRAILING-WMF-SETDIBTODEV-JPEG /EmbeddedFile");
    let mut jpeg_dib = minimal_compressed_dib_with_payload(2, 1, 4, &jpeg_payload);
    jpeg_dib.extend_from_slice(b"TRAILING-WMF-SETDIBTODEV-JPEG-DIB /Launch");
    let records = [
        wmf_setdib_to_dev_record(16, 24, 2, 1, 0, 1, &dib),
        wmf_setdib_to_dev_record(30, 34, 2, 3, 1, 1, &partial_dib),
        wmf_setdib_to_dev_record(46, 44, 2, 3, 0, 1, &top_down_compressed_png_dib),
        wmf_setdib_to_dev_record(62, 54, 2, 1, 0, 1, &jpeg_dib),
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
        .expect("WMF SETDIBTODEV vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 4);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::RasterImage {
            left: 16.0,
            top: 24.0,
            right: 18.0,
            bottom: 25.0,
            image,
        } if image.format == ImageFormat::Rgb8
            && image.width_px == 2
            && image.height_px == 1
            && image.bytes == vec![255, 0, 0, 0, 255, 0]
    ));
    assert!(matches!(
        &image.vector_commands[1],
        StaticImageVectorCommand::RasterImage {
            left: 30.0,
            top: 34.0,
            right: 32.0,
            bottom: 35.0,
            image,
        } if image.format == ImageFormat::Rgb8
            && image.width_px == 2
            && image.height_px == 1
            && image.bytes == vec![70, 80, 90, 100, 110, 120]
    ));
    match &image.vector_commands[2] {
        StaticImageVectorCommand::RasterImage {
            left: 46.0,
            top: 44.0,
            right: 48.0,
            bottom: 45.0,
            image,
        } => {
            assert_eq!(image.format, ImageFormat::Png);
            assert_eq!(image.width_px, 2);
            assert_eq!(image.height_px, 1);
            assert_eq!(
                miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 7).unwrap(),
                vec![0, 10, 20, 30, 40, 50, 60]
            );
            assert!(image.alpha_mask.is_none());
        }
        _ => panic!("expected top-down compressed WMF SETDIBTODEV PNG raster image"),
    }
    assert!(matches!(
        &image.vector_commands[3],
        StaticImageVectorCommand::RasterImage {
            left: 62.0,
            top: 54.0,
            right: 64.0,
            bottom: 55.0,
            image,
        } if image.format == ImageFormat::Jpeg
            && image.width_px == 2
            && image.height_px == 1
            && image.bytes == jpeg
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "0d33",
        "TRAILING-WMF-SETDIBTODEV",
        "TRAILING-WMF-PARTIAL-SETDIBTODEV",
        "TRAILING-WMF-TOPDOWN-COMPRESSED-SETDIBTODEV",
        "TRAILING-WMF-SETDIBTODEV-JPEG",
        "TRAILING-WMF-SETDIBTODEV-JPEG-DIB",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF SETDIBTODEV internals leaked to text: {forbidden}"
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
        output
            .pdf
            .windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image"),
        "WMF SETDIBTODEV should render a passive image XObject"
    );
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "Do")
            .count()
            >= 4,
        "WMF SETDIBTODEV should invoke a passive PDF image XObject"
    );
    for forbidden in [
        b"wmetafile".as_slice(),
        b"0d33",
        b"TRAILING-WMF-SETDIBTODEV",
        b"TRAILING-WMF-PARTIAL-SETDIBTODEV",
        b"TRAILING-WMF-TOPDOWN-COMPRESSED-SETDIBTODEV",
        b"TRAILING-WMF-SETDIBTODEV-JPEG",
        b"TRAILING-WMF-SETDIBTODEV-JPEG-DIB",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF SETDIBTODEV leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_zero_extent_and_zero_scan_rasters_are_noops_without_payload_leakage() {
    let mut hidden_dib = b"INVISIBLE-WMF-RASTER-PAYLOAD /JavaScript /EmbeddedFile /Launch".to_vec();
    hidden_dib.resize(96, 0x41);
    let records = [
        wmf_create_brush_record(Color {
            red: 30,
            green: 90,
            blue: 150,
        }),
        wmf_select_object_record(0),
        wmf_bounds_record(0x041b, 0, 0, 80, 40),
        wmf_patblt_record(18, 24, 0, 28, 0x00f0_0021),
        wmf_bitblt_record(70, 24, 48, 0, 0x00f0_0021, &hidden_dib),
        wmf_stretchblt_record(18, 54, 0, 28, 0x00f0_0021, &hidden_dib),
        wmf_dibbitblt_record(70, 54, 48, 0, 0x00f0_0021, &hidden_dib),
        wmf_dibstretchblt_record(18, 84, 0, 28, 0x00f0_0021, &hidden_dib),
        wmf_stretchdib_dib_record(70, 84, 48, 0, 0x00f0_0021, &hidden_dib),
        wmf_setdib_to_dev_record(18, 114, 48, 28, 0, 0, &hidden_dib),
    ];
    let wmf = minimal_wmf_with_records(160, 140, &records);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich140\\picwgoal2160\\pichgoal1008 {wmf_hex}}} after\\par}}"
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
        .expect("passive WMF zero-size raster vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in [
        "wmetafile",
        "INVISIBLE-WMF-RASTER-PAYLOAD",
        "061d",
        "0922",
        "0b23",
        "0940",
        "0b41",
        "0f43",
        "0d33",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "zero-size WMF raster payload/control leaked to normalized text: {forbidden}"
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
    assert_no_wmf_preview_warning(&output.diagnostics);
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "re")
            .count()
            >= 1,
        "preexisting paint before zero-size WMF raster records should still render"
    );
    for forbidden in [
        b"wmetafile".as_slice(),
        wmf_hex.as_bytes(),
        b"INVISIBLE-WMF-RASTER-PAYLOAD",
        b"061d",
        b"0b23",
        b"0940",
        b"0b41",
        b"0f43",
        b"0d33",
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
            "zero-size WMF raster payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_zero_source_extent_stretch_rasters_are_noops_without_payload_leakage() {
    let mut hidden_dib =
        b"ZERO-SOURCE-WMF-RASTER-PAYLOAD /JavaScript /EmbeddedFile /Launch".to_vec();
    hidden_dib.resize(96, 0x41);
    let records = [
        wmf_create_brush_record(Color {
            red: 30,
            green: 90,
            blue: 150,
        }),
        wmf_select_object_record(0),
        wmf_bounds_record(0x041b, 0, 0, 80, 40),
        wmf_stretchblt_record(18, 54, 48, 28, 0x00cc_0020, &hidden_dib),
        wmf_dibstretchblt_record_with_source(70, 54, 48, 28, 0, 0, 0, 28, &hidden_dib),
        wmf_stretchdib_dib_record_with_source(18, 84, 48, 28, 0, 0, 48, 0, &hidden_dib),
    ];
    let wmf = minimal_wmf_with_records(160, 120, &records);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw200\\pich120\\picwgoal2160\\pichgoal864 {wmf_hex}}} after\\par}}"
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
        .expect("passive WMF zero-source raster vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert_no_wmf_preview_warning(&parsed.diagnostics);
    for forbidden in [
        "wmetafile",
        "ZERO-SOURCE-WMF-RASTER-PAYLOAD",
        "0b23",
        "0b41",
        "0f43",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "zero-source WMF raster payload/control leaked to normalized text: {forbidden}"
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
    assert_no_wmf_preview_warning(&output.diagnostics);
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "re")
            .count()
            >= 1,
        "preexisting paint before zero-source WMF raster records should still render"
    );
    for forbidden in [
        b"wmetafile".as_slice(),
        wmf_hex.as_bytes(),
        b"ZERO-SOURCE-WMF-RASTER-PAYLOAD",
        b"0b23",
        b"0b41",
        b"0f43",
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
            "zero-source WMF raster payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_srccopy_dib_blt_records_render_passive_images_without_payload_leakage() {
    let dib = minimal_24bit_dib_with_dimensions(2, 1);
    let records = [
        wmf_dibbitblt_record(15, 25, 2, 1, 0x00cc_0020, &dib),
        wmf_dibstretchblt_record(35, 45, 80, 40, 0x00cc_0020, &dib),
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
        .expect("WMF SRCCOPY DIB blit vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    let StaticImageVectorCommand::RasterImage {
        left,
        top,
        right,
        bottom,
        image: raster,
    } = &image.vector_commands[0]
    else {
        panic!("expected passive DIBBITBLT raster command");
    };
    assert!((*left - 15.079_498).abs() < 0.000_1);
    assert!((*top - 26.530_811).abs() < 0.000_1);
    assert!((*right - 16.666_801).abs() < 0.000_1);
    assert!((*bottom - 27.891_36).abs() < 0.000_1);
    assert_eq!(raster.format, ImageFormat::Rgb8);
    assert_eq!((raster.width_px, raster.height_px), (2, 1));
    assert_eq!(raster.bytes, vec![255, 0, 0, 0, 255, 0]);
    assert!(matches!(
        &image.vector_commands[1],
        StaticImageVectorCommand::RasterImage {
            left: 35.0,
            top: 45.0,
            right: 115.0,
            bottom: 85.0,
            image,
        } if image.format == ImageFormat::Rgb8
            && image.width_px == 2
            && image.height_px == 1
            && image.bytes == vec![255, 0, 0, 0, 255, 0]
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "0940",
        "0b41",
        "cc0020",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF SRCCOPY DIB blit internals leaked to text: {forbidden}"
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
        output
            .pdf
            .windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image"),
        "WMF SRCCOPY DIB blits should render passive image XObjects"
    );
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "Do")
            .count()
            >= 2,
        "WMF SRCCOPY DIB blits should invoke passive PDF image XObjects"
    );
    for forbidden in [
        b"wmetafile".as_slice(),
        b"0940",
        b"0b41",
        b"cc0020",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF SRCCOPY DIB blits leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_srccopy_cropped_embedded_dibs_render_passive_images_without_payload_leakage() {
    let dib = minimal_24bit_dib_with_rgb_pixels(
        3,
        2,
        &[
            [10, 20, 30],
            [40, 50, 60],
            [70, 80, 90],
            [100, 110, 120],
            [130, 140, 150],
            [160, 170, 180],
        ],
    );
    let records = [
        wmf_stretchdib_dib_record_with_source(15, 25, 80, 40, 1, 1, 2, 1, &dib),
        wmf_dibbitblt_record_with_source(25, 35, 2, 1, 1, 1, &dib),
        wmf_dibstretchblt_record_with_source(35, 45, 60, 30, 1, 1, 2, 1, &dib),
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
        .expect("cropped WMF SRCCOPY embedded-DIB vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 3);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::RasterImage { image, .. }
            if image.format == ImageFormat::Rgb8
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == vec![40, 50, 60, 70, 80, 90]
    ));
    for command in &image.vector_commands[1..] {
        assert!(matches!(
            command,
            StaticImageVectorCommand::RasterImage {
                image,
                ..
            } if image.format == ImageFormat::Rgb8
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == vec![130, 140, 150, 160, 170, 180]
        ));
    }
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "wmetafile",
        "0f43",
        "0940",
        "0b41",
        "cc0020",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "cropped WMF SRCCOPY internals leaked to text: {forbidden}"
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
        output
            .pdf
            .windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image"),
        "cropped WMF SRCCOPY records should render passive image XObjects"
    );
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "Do")
            .count()
            >= 3,
        "cropped WMF SRCCOPY records should invoke passive PDF image XObjects"
    );
    for forbidden in [
        b"wmetafile".as_slice(),
        wmf_hex.as_bytes(),
        b"0f43",
        b"0940",
        b"0b41",
        b"cc0020",
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
            "cropped WMF SRCCOPY payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wmf_srccopy_cropped_jpeg_embedded_dibs_render_clipped_without_payload_leakage() {
    let jpeg = minimal_jpeg_with_dimensions(3, 2);
    let mut jpeg_payload = jpeg.clone();
    jpeg_payload.extend_from_slice(b"TRAILING-WMF-STRETCHDIB-JPEG-CROP /JavaScript");
    let mut dib = minimal_compressed_dib_with_payload(3, 2, 4, &jpeg_payload);
    dib.extend_from_slice(b"TRAILING-WMF-STRETCHDIB-JPEG-CROP-DIB /EmbeddedFile");
    let records = [
        wmf_stretchdib_dib_record_with_source(15, 25, 80, 40, 1, 0, 1, 2, &dib),
        wmf_dibstretchblt_record_with_source(35, 45, 60, 30, 1, 0, 1, 2, &dib),
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
        .expect("JPEG-cropped WMF SRCCOPY embedded-DIB vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 8);
    assert_eq!(parsed.diagnostics.len(), 0);
    let StaticImageVectorCommand::ClipRect {
        left,
        top,
        right,
        bottom,
    } = image.vector_commands[1]
    else {
        panic!("expected passive JPEG clip rectangle");
    };
    assert!((left - 15.079_498).abs() < 0.000_1);
    assert!((top - 26.530_811).abs() < 0.000_1);
    assert!((right - 93.652_44).abs() < 0.000_1);
    assert!((bottom - 69.388_46).abs() < 0.000_1);
    let StaticImageVectorCommand::RasterImage {
        left,
        top,
        right,
        bottom,
        image: raster,
    } = &image.vector_commands[2]
    else {
        panic!("expected clipped passive JPEG raster");
    };
    assert!((*left - -63.493_446).abs() < 0.000_1);
    assert!((*top - 26.530_811).abs() < 0.000_1);
    assert!((*right - 172.225_39).abs() < 0.000_1);
    assert!((*bottom - 69.388_46).abs() < 0.000_1);
    assert_eq!(raster.format, ImageFormat::Jpeg);
    assert_eq!((raster.width_px, raster.height_px), (3, 2));
    assert_eq!(raster.bytes, jpeg);
    assert!(raster.alpha_mask.is_none());
    assert!(matches!(
        image.vector_commands[5],
        StaticImageVectorCommand::ClipRect {
            left: 35.0,
            top: 45.0,
            right: 95.0,
            bottom: 75.0,
        }
    ));
    for forbidden in [
        "wmetafile",
        "0f43",
        "0b41",
        "cc0020",
        "TRAILING-WMF-STRETCHDIB-JPEG-CROP",
        "TRAILING-WMF-STRETCHDIB-JPEG-CROP-DIB",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "JPEG-cropped WMF SRCCOPY payload/control leaked to text: {forbidden}"
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
        output
            .pdf
            .windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image"),
        "JPEG-cropped WMF SRCCOPY records should render passive image XObjects"
    );
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "Do")
            .count()
            >= 2,
        "JPEG-cropped WMF SRCCOPY records should invoke passive PDF image XObjects"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "W"),
        "JPEG-cropped WMF SRCCOPY records should clip the full JPEG draw"
    );
    for forbidden in [
        b"wmetafile".as_slice(),
        wmf_hex.as_bytes(),
        b"0f43",
        b"0b41",
        b"cc0020",
        b"TRAILING-WMF-STRETCHDIB-JPEG-CROP",
        b"TRAILING-WMF-STRETCHDIB-JPEG-CROP-DIB",
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
            "JPEG-cropped WMF SRCCOPY payload leaked to PDF: {:?}",
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
        let searchable_pdf = pdf_bytes_excluding_embedded_font_programs(&output.pdf);

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
                !searchable_pdf
                    .windows(forbidden.len())
                    .any(|window| window == forbidden),
                "unsupported {control} payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn malformed_emf_raster_op_records_count_as_partial_without_payload_leakage() {
    let mut malformed_bitblt = vec![0; 40];
    write_test_le_u32(&mut malformed_bitblt, 0, 76);
    write_test_le_u32(&mut malformed_bitblt, 4, 40);
    write_test_le_i32(&mut malformed_bitblt, 8, 10);
    write_test_le_i32(&mut malformed_bitblt, 12, 10);
    write_test_le_i32(&mut malformed_bitblt, 16, 50);
    write_test_le_i32(&mut malformed_bitblt, 20, 50);
    write_test_le_i32(&mut malformed_bitblt, 24, 10);
    write_test_le_i32(&mut malformed_bitblt, 28, 10);
    write_test_le_i32(&mut malformed_bitblt, 32, 40);
    write_test_le_i32(&mut malformed_bitblt, 36, 40);
    let records = [malformed_bitblt, emf_rect_record(43, 20, 10, 80, 50)];
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
        .expect("malformed EMF raster-op partial vector preview");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(
        image
            .vector_commands
            .iter()
            .any(|command| matches!(command, StaticImageVectorCommand::Rectangle { .. })),
        "safe drawing after malformed EMF raster-op record should stay visible"
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
        "malformed EMF raster-op record should produce skipped-record diagnostics: {:?}",
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
        "safe drawing after malformed EMF raster-op record should render passively"
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
            "malformed EMF raster-op payload/control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_bitmap_orientation_setlayout_is_passive_noop_without_payload_leakage() {
    let mut mode = emf_u32_record(115, 8);
    mode.extend_from_slice(b"EMF-SETLAYOUT-BITMAP-ORIENTATION /JavaScript /EmbeddedFile /Launch");
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
        .expect("passive EMF bitmap-orientation layout vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "EMF-SETLAYOUT-BITMAP-ORIENTATION",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF bitmap-orientation SETLAYOUT payload/control leaked to normalized text: {forbidden}"
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
        "drawing after EMF bitmap-orientation SETLAYOUT should render a passive PDF rectangle"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETLAYOUT-BITMAP-ORIENTATION",
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
            "EMF bitmap-orientation SETLAYOUT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_srccopy_bitblt_and_stretchblt_dibs_render_as_passive_images_without_payload_leakage() {
    let rgb_dib = minimal_24bit_dib_with_dimensions(2, 1);
    let jpeg = minimal_jpeg_with_dimensions(2, 1);
    let mut jpeg_dib = minimal_compressed_dib_with_payload(2, 1, 4, &jpeg);
    jpeg_dib.extend_from_slice(b"TRAILING-EMF-STRETCHBLT-JPEG /JavaScript");
    let mut png_dib = minimal_compressed_dib_with_payload(
        3,
        1,
        5,
        &minimal_rgb_png(&[[255, 0, 0], [0, 255, 0], [0, 0, 255]]),
    );
    png_dib.extend_from_slice(b"TRAILING-EMF-CROPPED-STRETCHBLT-PNG /EmbeddedFile");
    let mut cropped_png_record = emf_stretchblt_dib_record(92, 36, 36, 18, 0x00cc_0020, &png_dib);
    write_test_le_i32(&mut cropped_png_record, 44, 1);
    write_test_le_i32(&mut cropped_png_record, 100, 2);
    let records = [
        emf_bitblt_dib_record(10, 20, 2, 1, 0x00cc_0020, &rgb_dib),
        emf_stretchblt_dib_record(35, 30, 45, 25, 0x00cc_0020, &jpeg_dib),
        cropped_png_record,
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
        .expect("passive EMF BITBLT/STRETCHBLT vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 3);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::RasterImage {
            left: 10.0,
            top: 20.0,
            right: 12.0,
            bottom: 21.0,
            image,
        } if image.format == ImageFormat::Rgb8
            && image.width_px == 2
            && image.height_px == 1
            && image.bytes == vec![255, 0, 0, 0, 255, 0]
    ));
    assert!(matches!(
        &image.vector_commands[1],
        StaticImageVectorCommand::RasterImage {
            left: 35.0,
            top: 30.0,
            right: 80.0,
            bottom: 55.0,
            image,
        } if image.format == ImageFormat::Jpeg
            && image.width_px == 2
            && image.height_px == 1
            && image.bytes == jpeg
    ));
    match &image.vector_commands[2] {
        StaticImageVectorCommand::RasterImage {
            left: 92.0,
            top: 36.0,
            right: 128.0,
            bottom: 54.0,
            image,
        } => {
            assert_eq!(image.format, ImageFormat::Png);
            assert_eq!(image.width_px, 2);
            assert_eq!(image.height_px, 1);
            assert_eq!(
                miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 7).unwrap(),
                vec![0, 0, 255, 0, 0, 0, 255]
            );
            assert!(image.alpha_mask.is_none());
        }
        _ => panic!("expected cropped compressed PNG raster image"),
    }
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "TRAILING-EMF-STRETCHBLT-JPEG",
        "TRAILING-EMF-CROPPED-STRETCHBLT-PNG",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF BITBLT/STRETCHBLT DIB payload/control leaked to normalized text: {forbidden}"
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
    let image_paints = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "Do")
        .count();

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(image_paints >= 3, "all EMF blit DIBs should render");
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"TRAILING-EMF-STRETCHBLT-JPEG",
        b"TRAILING-EMF-CROPPED-STRETCHBLT-PNG",
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
            "EMF BITBLT/STRETCHBLT DIB payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_setdibitstodevice_dib_renders_as_passive_image_without_payload_leakage() {
    let mut dib = minimal_24bit_dib_with_dimensions(2, 1);
    dib.extend_from_slice(b"TRAILING-EMF-SETDIBITS /JavaScript");
    let mut partial_dib = minimal_24bit_dib_with_rgb_pixels(
        2,
        3,
        &[
            [10, 20, 30],
            [40, 50, 60],
            [70, 80, 90],
            [100, 110, 120],
            [130, 140, 150],
            [160, 170, 180],
        ],
    );
    partial_dib.extend_from_slice(b"TRAILING-EMF-PARTIAL-SETDIBITS /JavaScript");
    let png_rows = [
        [10, 20, 30],
        [40, 50, 60],
        [70, 80, 90],
        [100, 110, 120],
        [130, 140, 150],
        [160, 170, 180],
    ];
    let mut compressed_png_dib =
        minimal_compressed_dib_with_payload(2, 3, 5, &minimal_rgb_png_with_rows(2, 3, &png_rows));
    compressed_png_dib
        .extend_from_slice(b"TRAILING-EMF-COMPRESSED-PARTIAL-SETDIBITS /EmbeddedFile");
    let mut top_down_compressed_png_dib =
        minimal_compressed_dib_with_payload(2, 3, 5, &minimal_rgb_png_with_rows(2, 3, &png_rows));
    write_test_le_i32(&mut top_down_compressed_png_dib, 8, -3);
    top_down_compressed_png_dib
        .extend_from_slice(b"TRAILING-EMF-TOPDOWN-COMPRESSED-PARTIAL-SETDIBITS /Launch");
    let jpeg = minimal_jpeg_with_dimensions(2, 1);
    let mut jpeg_payload = jpeg.clone();
    jpeg_payload.extend_from_slice(b"TRAILING-EMF-SETDIBITS-JPEG /EmbeddedFile");
    let mut jpeg_dib = minimal_compressed_dib_with_payload(2, 1, 4, &jpeg_payload);
    jpeg_dib.extend_from_slice(b"TRAILING-EMF-SETDIBITS-JPEG-DIB /Launch");
    let partial_jpeg = minimal_jpeg_with_dimensions(3, 2);
    let mut partial_jpeg_payload = partial_jpeg.clone();
    partial_jpeg_payload.extend_from_slice(b"TRAILING-EMF-PARTIAL-SETDIBITS-JPEG /JavaScript");
    let mut partial_jpeg_dib = minimal_compressed_dib_with_payload(3, 2, 4, &partial_jpeg_payload);
    partial_jpeg_dib.extend_from_slice(b"TRAILING-EMF-PARTIAL-SETDIBITS-JPEG-DIB /Launch");
    let records = [
        emf_setdibitstodevice_dib_record(18, 22, 2, 1, 0, 1, &dib),
        emf_setdibitstodevice_dib_record(42, 34, 2, 3, 1, 1, &partial_dib),
        emf_setdibitstodevice_dib_record(68, 42, 2, 3, 1, 1, &compressed_png_dib),
        emf_setdibitstodevice_dib_record(94, 50, 2, 3, 0, 1, &top_down_compressed_png_dib),
        emf_setdibitstodevice_dib_record(120, 58, 2, 1, 0, 1, &jpeg_dib),
        emf_setdibitstodevice_dib_record(132, 62, 3, 2, 0, 1, &partial_jpeg_dib),
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
        .expect("passive EMF SETDIBITSTODEVICE vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 9);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::RasterImage {
            left: 18.0,
            top: 22.0,
            right: 20.0,
            bottom: 23.0,
            image,
        } if image.format == ImageFormat::Rgb8
            && image.width_px == 2
            && image.height_px == 1
            && image.bytes == vec![255, 0, 0, 0, 255, 0]
    ));
    assert!(matches!(
        &image.vector_commands[1],
        StaticImageVectorCommand::RasterImage {
            left: 42.0,
            top: 34.0,
            right: 44.0,
            bottom: 35.0,
            image,
        } if image.format == ImageFormat::Rgb8
            && image.width_px == 2
            && image.height_px == 1
            && image.bytes == vec![70, 80, 90, 100, 110, 120]
    ));
    match &image.vector_commands[2] {
        StaticImageVectorCommand::RasterImage {
            left: 68.0,
            top: 42.0,
            right: 70.0,
            bottom: 43.0,
            image,
        } => {
            assert_eq!(image.format, ImageFormat::Png);
            assert_eq!(image.width_px, 2);
            assert_eq!(image.height_px, 1);
            assert_eq!(
                miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 7).unwrap(),
                vec![0, 70, 80, 90, 100, 110, 120]
            );
            assert!(image.alpha_mask.is_none());
        }
        _ => panic!("expected compressed partial SETDIB PNG raster image"),
    }
    match &image.vector_commands[3] {
        StaticImageVectorCommand::RasterImage {
            left: 94.0,
            top: 50.0,
            right: 96.0,
            bottom: 51.0,
            image,
        } => {
            assert_eq!(image.format, ImageFormat::Png);
            assert_eq!(image.width_px, 2);
            assert_eq!(image.height_px, 1);
            assert_eq!(
                miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 7).unwrap(),
                vec![0, 10, 20, 30, 40, 50, 60]
            );
            assert!(image.alpha_mask.is_none());
        }
        _ => panic!("expected top-down compressed partial SETDIB PNG raster image"),
    }
    assert!(matches!(
        &image.vector_commands[4],
        StaticImageVectorCommand::RasterImage {
            left: 120.0,
            top: 58.0,
            right: 122.0,
            bottom: 59.0,
            image,
        } if image.format == ImageFormat::Jpeg
            && image.width_px == 2
            && image.height_px == 1
            && image.bytes == jpeg
    ));
    assert!(matches!(
        image.vector_commands[5],
        StaticImageVectorCommand::SaveState
    ));
    assert_eq!(
        image.vector_commands[6],
        StaticImageVectorCommand::ClipRect {
            left: 132.0,
            top: 62.0,
            right: 135.0,
            bottom: 63.0,
        }
    );
    assert!(matches!(
        &image.vector_commands[7],
        StaticImageVectorCommand::RasterImage {
            left: 132.0,
            top: 61.0,
            right: 135.0,
            bottom: 63.0,
            image,
        } if image.format == ImageFormat::Jpeg
            && image.width_px == 3
            && image.height_px == 2
            && image.bytes == partial_jpeg
            && image.alpha_mask.is_none()
    ));
    assert!(matches!(
        image.vector_commands[8],
        StaticImageVectorCommand::RestoreState
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "TRAILING-EMF-SETDIBITS",
        "TRAILING-EMF-PARTIAL-SETDIBITS",
        "TRAILING-EMF-COMPRESSED-PARTIAL-SETDIBITS",
        "TRAILING-EMF-TOPDOWN-COMPRESSED-PARTIAL-SETDIBITS",
        "TRAILING-EMF-SETDIBITS-JPEG",
        "TRAILING-EMF-SETDIBITS-JPEG-DIB",
        "TRAILING-EMF-PARTIAL-SETDIBITS-JPEG",
        "TRAILING-EMF-PARTIAL-SETDIBITS-JPEG-DIB",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF SETDIBITSTODEVICE payload/control leaked to normalized text: {forbidden}"
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
            >= 6,
        "EMF SETDIBITSTODEVICE DIB should render as a passive image XObject"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "W"),
        "partial JPEG EMF SETDIBITSTODEVICE should clip the full JPEG draw"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"TRAILING-EMF-SETDIBITS",
        b"TRAILING-EMF-PARTIAL-SETDIBITS",
        b"TRAILING-EMF-COMPRESSED-PARTIAL-SETDIBITS",
        b"TRAILING-EMF-TOPDOWN-COMPRESSED-PARTIAL-SETDIBITS",
        b"TRAILING-EMF-SETDIBITS-JPEG",
        b"TRAILING-EMF-SETDIBITS-JPEG-DIB",
        b"TRAILING-EMF-PARTIAL-SETDIBITS-JPEG",
        b"TRAILING-EMF-PARTIAL-SETDIBITS-JPEG-DIB",
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
            "EMF SETDIBITSTODEVICE DIB payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_constant_alpha_alphablend_dib_renders_with_passive_alpha_mask_without_payload_leakage() {
    let mut dib = minimal_24bit_dib_with_dimensions(2, 1);
    dib.extend_from_slice(b"TRAILING-EMF-ALPHABLEND /JavaScript");
    let jpeg = minimal_jpeg_with_dimensions(2, 1);
    let mut jpeg_payload = jpeg.clone();
    jpeg_payload.extend_from_slice(b"TRAILING-EMF-ALPHABLEND-JPEG /EmbeddedFile");
    let mut jpeg_dib = minimal_compressed_dib_with_payload(2, 1, 4, &jpeg_payload);
    jpeg_dib.extend_from_slice(b"TRAILING-EMF-ALPHABLEND-JPEG-DIB /Launch");
    let records = [
        emf_alphablend_dib_record(18, 24, 48, 28, 0xff, 0, &dib),
        emf_alphablend_dib_record(70, 30, 48, 28, 0x80, 0, &dib),
        emf_alphablend_dib_record(
            100,
            36,
            48,
            28,
            0xff,
            1,
            &minimal_32bit_dib_with_rgba_pixels(2, 1, &[[255, 0, 0, 64], [0, 255, 0, 192]]),
        ),
        emf_alphablend_dib_record(
            118,
            42,
            48,
            28,
            0xff,
            1,
            &minimal_32bit_bitfields_dib_with_rgba_pixels(
                2,
                1,
                &[[255, 0, 0, 32], [0, 255, 0, 224]],
            ),
        ),
        emf_alphablend_dib_record(
            130,
            46,
            48,
            28,
            0xff,
            1,
            &minimal_32bit_alpha_bitfields_dib_with_rgba_pixels(
                2,
                1,
                &[[255, 0, 0, 48], [0, 255, 0, 208]],
            ),
        ),
        emf_alphablend_dib_record(
            136,
            48,
            48,
            28,
            0xff,
            1,
            &minimal_16bit_alpha_bitfields_dib_with_rgba_pixels(
                2,
                1,
                &[[255, 0, 0, 0], [0, 255, 0, 255]],
            ),
        ),
        emf_alphablend_dib_record(
            140,
            49,
            48,
            28,
            0xff,
            1,
            &minimal_compressed_dib_with_payload(
                2,
                1,
                5,
                &minimal_rgba_png(&[[255, 0, 0, 16], [0, 255, 0, 240]]),
            ),
        ),
        emf_alphablend_dib_record(10, 50, 48, 28, 0x60, 0, &jpeg_dib),
        emf_alphablend_dib_record(142, 50, 48, 28, 0xff, 1, &dib),
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
        .expect("passive EMF ALPHABLEND vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 8);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::RasterImage {
            left: 18.0,
            top: 24.0,
            right: 66.0,
            bottom: 52.0,
            image,
        } if image.format == ImageFormat::Rgb8
            && image.width_px == 2
            && image.height_px == 1
            && image.bytes == vec![255, 0, 0, 0, 255, 0]
            && image.alpha_mask.is_none()
    ));
    let alpha_mask = match &image.vector_commands[1] {
        StaticImageVectorCommand::RasterImage { image, .. } => image
            .alpha_mask
            .as_ref()
            .expect("semi-transparent EMF ALPHABLEND should render as a passive alpha mask"),
        _ => panic!("expected constant-alpha raster image"),
    };
    assert_eq!(
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&alpha_mask.bytes, 3).unwrap(),
        vec![0, 0x80, 0x80]
    );
    let source_alpha_mask = match &image.vector_commands[2] {
        StaticImageVectorCommand::RasterImage { image, .. } => image
            .alpha_mask
            .as_ref()
            .expect("per-pixel EMF ALPHABLEND should render as a passive alpha mask"),
        _ => panic!("expected per-pixel alpha raster image"),
    };
    assert_eq!(
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&source_alpha_mask.bytes, 3)
            .unwrap(),
        vec![0, 64, 192]
    );
    let bitfield_alpha_mask = match &image.vector_commands[3] {
        StaticImageVectorCommand::RasterImage { image, .. } => image
            .alpha_mask
            .as_ref()
            .expect("bitfield EMF ALPHABLEND should render as a passive alpha mask"),
        _ => panic!("expected bitfield alpha raster image"),
    };
    assert_eq!(
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&bitfield_alpha_mask.bytes, 3)
            .unwrap(),
        vec![0, 32, 224]
    );
    let alpha_bitfield_alpha_mask = match &image.vector_commands[4] {
        StaticImageVectorCommand::RasterImage { image, .. } => image
            .alpha_mask
            .as_ref()
            .expect("alpha-bitfield EMF ALPHABLEND should render as a passive alpha mask"),
        _ => panic!("expected alpha-bitfield alpha raster image"),
    };
    assert_eq!(
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(
            &alpha_bitfield_alpha_mask.bytes,
            3,
        )
        .unwrap(),
        vec![0, 48, 208]
    );
    let alpha_bitfield_16bit_alpha_mask = match &image.vector_commands[5] {
        StaticImageVectorCommand::RasterImage { image, .. } => image
            .alpha_mask
            .as_ref()
            .expect("16-bit alpha-bitfield EMF ALPHABLEND should render as a passive alpha mask"),
        _ => panic!("expected 16-bit alpha-bitfield alpha raster image"),
    };
    assert_eq!(
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(
            &alpha_bitfield_16bit_alpha_mask.bytes,
            3,
        )
        .unwrap(),
        vec![0, 0, 255]
    );
    let compressed_png_alpha_mask = match &image.vector_commands[6] {
        StaticImageVectorCommand::RasterImage { image, .. } => image
            .alpha_mask
            .as_ref()
            .expect("compressed PNG EMF ALPHABLEND should render as a passive alpha mask"),
        _ => panic!("expected compressed PNG alpha raster image"),
    };
    assert_eq!(
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(
            &compressed_png_alpha_mask.bytes,
            3,
        )
        .unwrap(),
        vec![0, 16, 240]
    );
    assert!(matches!(
        &image.vector_commands[7],
            StaticImageVectorCommand::RasterImage {
            left: 10.0,
            top: 50.0,
            right: 58.0,
            bottom: 78.0,
            image,
        } if image.format == ImageFormat::Jpeg
            && image.width_px == 2
            && image.height_px == 1
            && image.bytes == jpeg
    ));
    let jpeg_alpha_mask = match &image.vector_commands[7] {
        StaticImageVectorCommand::RasterImage { image, .. } => image
            .alpha_mask
            .as_ref()
            .expect("constant-alpha JPEG EMF ALPHABLEND should render as a passive alpha mask"),
        _ => panic!("expected constant-alpha JPEG raster image"),
    };
    assert_eq!(
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&jpeg_alpha_mask.bytes, 3).unwrap(),
        vec![0, 0x60, 0x60]
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("1 unsupported record(s) skipped")
    }));
    for forbidden in [
        "emfblip",
        "ALPHABLEND",
        "TRAILING-EMF-ALPHABLEND",
        "TRAILING-EMF-ALPHABLEND-JPEG",
        "TRAILING-EMF-ALPHABLEND-JPEG-DIB",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF ALPHABLEND payload/control leaked to normalized text: {forbidden}"
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
            >= 5,
        "EMF ALPHABLEND DIBs should render as passive image XObjects"
    );
    assert!(
        output
            .pdf
            .windows(b"/SMask".len())
            .any(|window| window == b"/SMask"),
        "constant-alpha EMF ALPHABLEND should render transparency as a passive PDF soft mask"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"ALPHABLEND",
        b"TRAILING-EMF-ALPHABLEND",
        b"TRAILING-EMF-ALPHABLEND-JPEG",
        b"TRAILING-EMF-ALPHABLEND-JPEG-DIB",
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
            "EMF ALPHABLEND DIB payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_invisible_alpha_and_transparent_bitmap_transfers_are_noops_without_payload_leakage() {
    let mut hidden_dib =
        b"INVISIBLE-EMF-BITMAP-TRANSFER-DIB /JavaScript /EmbeddedFile /Launch".to_vec();
    hidden_dib.resize(96, 0x41);
    let records = [
        emf_rect_record(43, 0, 0, 80, 40),
        emf_alphablend_dib_record(18, 24, 0, 28, 0xff, 0, &hidden_dib),
        emf_transparentblt_dib_record(
            70,
            24,
            48,
            0,
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
            &hidden_dib,
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
        .expect("passive EMF invisible bitmap transfer vector image");

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
        "INVISIBLE-EMF-BITMAP-TRANSFER-DIB",
        "ALPHABLEND",
        "TRANSPARENTBLT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "invisible EMF bitmap-transfer payload/control leaked to normalized text: {forbidden}"
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
        "preexisting paint before invisible bitmap transfers should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"INVISIBLE-EMF-BITMAP-TRANSFER-DIB",
        b"ALPHABLEND",
        b"TRANSPARENTBLT",
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
            "invisible EMF bitmap-transfer payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_zero_source_alpha_and_transparent_bitmap_transfers_are_noops_without_payload_leakage() {
    let mut hidden_dib =
        b"ZERO-SOURCE-EMF-BITMAP-TRANSFER-DIB /JavaScript /EmbeddedFile /Launch".to_vec();
    hidden_dib.resize(96, 0x41);
    let mut alphablend = emf_alphablend_dib_record(18, 24, 48, 28, 0xff, 0, &hidden_dib);
    write_test_le_i32(&mut alphablend, 100, 0);
    let mut transparentblt = emf_transparentblt_dib_record(
        70,
        24,
        48,
        28,
        Color {
            red: 255,
            green: 0,
            blue: 0,
        },
        &hidden_dib,
    );
    write_test_le_i32(&mut transparentblt, 104, 0);
    let records = [
        emf_rect_record(43, 0, 0, 80, 40),
        alphablend,
        transparentblt,
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
        .expect("passive EMF zero-source bitmap transfer vector image");

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
        "ZERO-SOURCE-EMF-BITMAP-TRANSFER-DIB",
        "ALPHABLEND",
        "TRANSPARENTBLT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "zero-source EMF bitmap-transfer payload/control leaked to normalized text: {forbidden}"
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
        !output
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("unsupported EMF record") })
    );
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "re")
            .count()
            >= 1,
        "preexisting paint before zero-source bitmap transfers should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"ZERO-SOURCE-EMF-BITMAP-TRANSFER-DIB",
        b"ALPHABLEND",
        b"TRANSPARENTBLT",
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
            "zero-source EMF bitmap-transfer payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_zero_extent_and_zero_scan_rasters_are_noops_without_payload_leakage() {
    let mut hidden_dib = b"INVISIBLE-EMF-RASTER-PAYLOAD /JavaScript /EmbeddedFile /Launch".to_vec();
    hidden_dib.resize(96, 0x41);
    let records = [
        emf_rect_record(43, 0, 0, 80, 40),
        emf_bitblt_record(18, 24, 0, 28, 0x00f0_0021, &hidden_dib),
        emf_stretchblt_record(70, 24, 48, 0, 0x00f0_0021, &hidden_dib),
        emf_stretchdibits_record(18, 54, 0, 28, 0x00f0_0021, &hidden_dib),
        emf_setdibitstodevice_dib_record(70, 54, 48, 28, 0, 0, &hidden_dib),
    ];
    let emf = minimal_emf_with_records(160, 100, 2540, 1588, &records);
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
        .expect("passive EMF zero-size raster vector image");

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
        "INVISIBLE-EMF-RASTER-PAYLOAD",
        "BITBLT",
        "STRETCHBLT",
        "STRETCHDIBITS",
        "SETDIBITS",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "zero-size EMF raster payload/control leaked to normalized text: {forbidden}"
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
        !output
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("unsupported EMF record") })
    );
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "re")
            .count()
            >= 1,
        "preexisting paint before zero-size raster records should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"INVISIBLE-EMF-RASTER-PAYLOAD",
        b"BITBLT",
        b"STRETCHBLT",
        b"STRETCHDIBITS",
        b"SETDIBITS",
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
            "zero-size EMF raster payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_zero_source_extent_stretch_rasters_are_noops_without_payload_leakage() {
    let mut hidden_dib =
        b"ZERO-SOURCE-EMF-RASTER-PAYLOAD /JavaScript /EmbeddedFile /Launch".to_vec();
    hidden_dib.resize(96, 0x41);
    let mut stretchblt = emf_stretchblt_dib_record(70, 24, 48, 28, 0x00cc_0020, &hidden_dib);
    write_test_le_i32(&mut stretchblt, 100, 0);
    let mut stretchdibits = emf_stretchdibits_dib_record(18, 54, 48, 28, 0x00cc_0020, &hidden_dib);
    write_test_le_i32(&mut stretchdibits, 44, 0);
    let records = [emf_rect_record(43, 0, 0, 80, 40), stretchblt, stretchdibits];
    let emf = minimal_emf_with_records(160, 100, 2540, 1588, &records);
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
        .expect("passive EMF zero-source raster vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "ZERO-SOURCE-EMF-RASTER-PAYLOAD",
        "STRETCHBLT",
        "STRETCHDIBITS",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "zero-source EMF raster payload/control leaked to normalized text: {forbidden}"
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
        !output
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("unsupported EMF record") })
    );
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "re")
            .count()
            >= 1,
        "preexisting paint before zero-source EMF raster records should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"ZERO-SOURCE-EMF-RASTER-PAYLOAD",
        b"STRETCHBLT",
        b"STRETCHDIBITS",
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
            "zero-source EMF raster payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_palette_usage_bitmap_transfers_stay_partial_without_payload_leakage() {
    let mut dib = minimal_24bit_dib_with_dimensions(2, 1);
    dib.extend_from_slice(b"PALETTE-USAGE-EMF-RASTER-DIB /JavaScript /EmbeddedFile /Launch");
    let mut bitblt = emf_bitblt_dib_record(18, 24, 48, 28, 0x00cc_0020, &dib);
    write_test_le_u32(&mut bitblt, 80, 1);
    let mut stretchblt = emf_stretchblt_dib_record(70, 24, 48, 28, 0x00cc_0020, &dib);
    write_test_le_u32(&mut stretchblt, 80, 1);
    let mut stretchdibits = emf_stretchdibits_dib_record(18, 54, 48, 28, 0x00cc_0020, &dib);
    write_test_le_u32(&mut stretchdibits, 72, 1);
    let records = [
        emf_rect_record(43, 0, 0, 80, 40),
        bitblt,
        stretchblt,
        stretchdibits,
    ];
    let emf = minimal_emf_with_records(160, 100, 2540, 1588, &records);
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
        .expect("partial passive EMF palette-usage raster vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("unsupported record(s) skipped") })
    );
    for forbidden in [
        "emfblip",
        "PALETTE-USAGE-EMF-RASTER-DIB",
        "BITBLT",
        "STRETCHBLT",
        "STRETCHDIBITS",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "palette-usage EMF raster payload/control leaked to normalized text: {forbidden}"
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
        output
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("unsupported record(s) skipped") })
    );
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "re")
            .count()
            >= 1,
        "preexisting paint before palette-usage EMF raster records should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"PALETTE-USAGE-EMF-RASTER-DIB",
        b"BITBLT",
        b"STRETCHBLT",
        b"STRETCHDIBITS",
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
            "palette-usage EMF raster payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_transparentblt_jpeg_keyed_color_gets_passive_alpha_mask_without_decoded_payload_leakage() {
    let jpeg = valid_rgb_jpeg_1x1();
    let transparent = valid_rgb_jpeg_1x1_color();
    let mut jpeg_payload = jpeg.clone();
    jpeg_payload.extend_from_slice(b"TRAILING-EMF-TRANSPARENTBLT-DECODED-JPEG /JavaScript");
    let mut jpeg_dib = minimal_compressed_dib_with_payload(1, 1, 4, &jpeg_payload);
    jpeg_dib.extend_from_slice(b"TRAILING-EMF-TRANSPARENTBLT-DECODED-JPEG-DIB /EmbeddedFile");
    let records = [emf_transparentblt_dib_record(
        24,
        18,
        40,
        30,
        transparent,
        &jpeg_dib,
    )];
    let emf = minimal_emf_with_records(120, 80, 2540, 1270, &records);
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
        .expect("passive EMF TRANSPARENTBLT JPEG vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    let alpha_mask = match &image.vector_commands[0] {
        StaticImageVectorCommand::RasterImage {
            left: 24.0,
            top: 18.0,
            right: 64.0,
            bottom: 48.0,
            image,
        } if image.format == ImageFormat::Jpeg
            && image.width_px == 1
            && image.height_px == 1
            && image.bytes == jpeg =>
        {
            image
                .alpha_mask
                .as_ref()
                .expect("decoded JPEG transparent color key alpha mask")
        }
        other => panic!("expected keyed JPEG transparent raster image, got {other:?}"),
    };
    assert_eq!(
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&alpha_mask.bytes, 2).unwrap(),
        vec![0, 0]
    );
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "TRANSPARENTBLT",
        "TRAILING-EMF-TRANSPARENTBLT-DECODED-JPEG",
        "TRAILING-EMF-TRANSPARENTBLT-DECODED-JPEG-DIB",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF TRANSPARENTBLT decoded JPEG payload/control leaked to normalized text: {forbidden}"
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
        output
            .pdf
            .windows(b"/DCTDecode".len())
            .any(|window| window == b"/DCTDecode"),
        "transparent JPEG source should remain a passive JPEG image stream"
    );
    assert!(
        output
            .pdf
            .windows(b"/SMask".len())
            .any(|window| window == b"/SMask"),
        "decoded-keyed JPEG TRANSPARENTBLT should render transparency as a passive PDF soft mask"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"TRANSPARENTBLT",
        b"TRAILING-EMF-TRANSPARENTBLT-DECODED-JPEG",
        b"TRAILING-EMF-TRANSPARENTBLT-DECODED-JPEG-DIB",
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
            "EMF TRANSPARENTBLT decoded JPEG DIB payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_transparentblt_jpeg_source_crop_renders_clipped_without_payload_leakage() {
    let jpeg = minimal_jpeg_with_dimensions(3, 2);
    let mut jpeg_payload = jpeg.clone();
    jpeg_payload.extend_from_slice(b"TRAILING-EMF-TRANSPARENTBLT-JPEG-CROP /JavaScript");
    let mut dib = minimal_compressed_dib_with_payload(3, 2, 4, &jpeg_payload);
    dib.extend_from_slice(b"TRAILING-EMF-TRANSPARENTBLT-JPEG-CROP-DIB /EmbeddedFile");
    let mut record = emf_transparentblt_dib_record(
        20,
        15,
        60,
        30,
        Color {
            red: 255,
            green: 0,
            blue: 255,
        },
        &dib,
    );
    write_test_le_i32(&mut record, 44, 1);
    write_test_le_i32(&mut record, 48, 0);
    write_test_le_i32(&mut record, 100, 1);
    write_test_le_i32(&mut record, 104, 2);
    let records = [record];
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
        .expect("passive JPEG-cropped EMF TRANSPARENTBLT vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 4);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::SaveState
    ));
    assert_eq!(
        image.vector_commands[1],
        StaticImageVectorCommand::ClipRect {
            left: 20.0,
            top: 15.0,
            right: 80.0,
            bottom: 45.0,
        }
    );
    assert!(matches!(
        &image.vector_commands[2],
        StaticImageVectorCommand::RasterImage {
            left: -40.0,
            top: 15.0,
            right: 140.0,
            bottom: 45.0,
            image,
        } if image.format == ImageFormat::Jpeg
            && image.width_px == 3
            && image.height_px == 2
            && image.bytes == jpeg
            && image.alpha_mask.is_none()
    ));
    assert!(matches!(
        image.vector_commands[3],
        StaticImageVectorCommand::RestoreState
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "TRANSPARENTBLT",
        "TRAILING-EMF-TRANSPARENTBLT-JPEG-CROP",
        "TRAILING-EMF-TRANSPARENTBLT-JPEG-CROP-DIB",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "JPEG-cropped EMF TRANSPARENTBLT payload/control leaked to normalized text: {forbidden}"
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
        output
            .pdf
            .windows(b"/DCTDecode".len())
            .any(|window| window == b"/DCTDecode"),
        "JPEG-cropped EMF TRANSPARENTBLT should keep the passive JPEG stream"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "Do"),
        "JPEG-cropped EMF TRANSPARENTBLT should invoke a passive PDF image XObject"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "W"),
        "JPEG-cropped EMF TRANSPARENTBLT should clip the full JPEG draw"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"TRANSPARENTBLT",
        b"TRAILING-EMF-TRANSPARENTBLT-JPEG-CROP",
        b"TRAILING-EMF-TRANSPARENTBLT-JPEG-CROP-DIB",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "JPEG-cropped EMF TRANSPARENTBLT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_patcopy_stretchdibits_records_render_passively_without_payload_leakage() {
    let payload = b"STRETCHDIBITS-SOURCE-PAYLOAD";
    let records = [
        emf_create_brush_record(
            3,
            0,
            Color {
                red: 180,
                green: 90,
                blue: 30,
            },
            0,
        ),
        emf_select_object_record(3),
        emf_stretchdibits_record(20, 15, 60, 30, 0x00f0_0021, payload),
        emf_stretchdibits_record(
            35,
            25,
            40,
            20,
            0x00cc_0020,
            b"UNSUPPORTED-STRETCHDIBITS-PAYLOAD",
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
        .expect("passive EMF STRETCHDIBITS vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 20.0,
            top: 15.0,
            right: 80.0,
            bottom: 45.0,
            stroke_color: None,
            fill_color: Some(Color {
                red: 180,
                green: 90,
                blue: 30
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
        "STRETCHDIBITS-SOURCE-PAYLOAD",
        "UNSUPPORTED-STRETCHDIBITS-PAYLOAD",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF STRETCHDIBITS payload/control leaked to normalized text: {forbidden}"
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
        "EMF PATCOPY STRETCHDIBITS should render as passive PDF rectangle path"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "f*" | "B" | "B*")),
        "EMF PATCOPY STRETCHDIBITS should render as a passive PDF fill"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        payload.as_slice(),
        b"UNSUPPORTED-STRETCHDIBITS-PAYLOAD",
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
            "EMF STRETCHDIBITS payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_srccopy_stretchdibits_dib_renders_as_passive_image_without_payload_leakage() {
    let dib = minimal_24bit_dib_with_dimensions(2, 1);
    let records = [emf_stretchdibits_dib_record(
        20,
        15,
        60,
        30,
        0x00cc_0020,
        &dib,
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
        .expect("passive EMF SRCCOPY STRETCHDIBITS vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::RasterImage {
            left: 20.0,
            top: 15.0,
            right: 80.0,
            bottom: 45.0,
            image,
        } if image.format == ImageFormat::Rgb8
            && image.width_px == 2
            && image.height_px == 1
            && image.bytes == vec![255, 0, 0, 0, 255, 0]
    ));
    assert!(!text.contains("emfblip"));
    assert!(!text.contains("STRETCHDIBITS"));

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
        output
            .pdf
            .windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image"),
        "EMF SRCCOPY STRETCHDIBITS should render a passive image XObject"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "Do"),
        "EMF SRCCOPY STRETCHDIBITS should invoke a passive PDF image XObject"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"STRETCHDIBITS",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "EMF SRCCOPY STRETCHDIBITS payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn blank_destination_srcand_dib_rasters_render_as_passive_images_without_payload_leakage() {
    let mut emf_dib = minimal_24bit_dib_with_dimensions(2, 1);
    emf_dib.extend_from_slice(b"TRAILING-EMF-SRCAND-DIB /JavaScript");
    let mut wmf_dib = minimal_24bit_dib_with_dimensions(2, 1);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-SRCAND-DIB /EmbeddedFile");
    let emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[emf_stretchdibits_dib_record(
            20,
            15,
            60,
            30,
            0x0088_00c6,
            &emf_dib,
        )],
    );
    let wmf = minimal_wmf_with_records(
        200,
        100,
        &[wmf_stretchdib_dib_record(
            35,
            25,
            80,
            40,
            0x0088_00c6,
            &wmf_dib,
        )],
    );
    let emf_hex = bytes_to_hex(&emf);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} middle {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
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
    for image in &images {
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage { image, .. }
                if image.format == ImageFormat::Rgb8
                    && image.width_px == 2
                    && image.height_px == 1
                    && image.bytes == vec![255, 0, 0, 0, 255, 0]
        ));
    }
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-SRCAND-DIB",
        "TRAILING-WMF-SRCAND-DIB",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "blank SRCAND DIB payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("middle"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "Do")
            .count()
            >= 2,
        "blank SRCAND DIBs should render passive image XObjects"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-SRCAND-DIB",
        b"TRAILING-WMF-SRCAND-DIB",
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
            "blank SRCAND DIB payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn blank_destination_srcinvert_dib_rasters_render_as_passive_images_without_payload_leakage() {
    let mut emf_dib = minimal_24bit_dib_with_dimensions(2, 1);
    emf_dib.extend_from_slice(b"TRAILING-EMF-SRCINVERT-DIB /JavaScript");
    let mut wmf_dib = minimal_24bit_dib_with_dimensions(2, 1);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-SRCINVERT-DIB /EmbeddedFile");
    let emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[emf_stretchdibits_dib_record(
            20,
            15,
            60,
            30,
            0x0066_0046,
            &emf_dib,
        )],
    );
    let wmf = minimal_wmf_with_records(
        200,
        100,
        &[wmf_stretchdib_dib_record(
            35,
            25,
            80,
            40,
            0x0066_0046,
            &wmf_dib,
        )],
    );
    let emf_hex = bytes_to_hex(&emf);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} middle {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
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
    for image in &images {
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage { image, .. }
                if image.format == ImageFormat::Rgb8
                    && image.width_px == 2
                    && image.height_px == 1
                    && image.bytes == vec![0, 255, 255, 255, 0, 255]
        ));
    }
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-SRCINVERT-DIB",
        "TRAILING-WMF-SRCINVERT-DIB",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "blank SRCINVERT DIB payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("middle"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "Do")
            .count()
            >= 2,
        "blank SRCINVERT DIBs should render passive image XObjects"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-SRCINVERT-DIB",
        b"TRAILING-WMF-SRCINVERT-DIB",
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
            "blank SRCINVERT DIB payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn notsrccopy_dib_rasters_render_as_passive_inverted_images_without_payload_leakage() {
    let mut emf_dib = minimal_24bit_dib_with_dimensions(2, 1);
    emf_dib.extend_from_slice(b"TRAILING-EMF-NOTSRCCOPY-DIB /JavaScript");
    let mut wmf_dib = minimal_24bit_dib_with_dimensions(2, 1);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-NOTSRCCOPY-DIB /EmbeddedFile");
    let emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[
            emf_rect_record(43, 0, 0, 30, 30),
            emf_stretchdibits_dib_record(20, 15, 60, 30, 0x0033_0008, &emf_dib),
        ],
    );
    let wmf = minimal_wmf_with_records(
        200,
        100,
        &[
            wmf_set_window_ext_record(200, 100),
            wmf_stretchdib_dib_record(35, 25, 80, 40, 0x0033_0008, &wmf_dib),
        ],
    );
    let emf_hex = bytes_to_hex(&emf);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} middle {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
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
    assert_eq!(images[0].vector_commands.len(), 2);
    assert_eq!(images[1].vector_commands.len(), 1);
    for command in [&images[0].vector_commands[1], &images[1].vector_commands[0]] {
        assert!(matches!(
            command,
            StaticImageVectorCommand::RasterImage { image, .. }
                if image.format == ImageFormat::Rgb8
                    && image.width_px == 2
                    && image.height_px == 1
                    && image.bytes == vec![0, 255, 255, 255, 0, 255]
        ));
    }
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-NOTSRCCOPY-DIB",
        "TRAILING-WMF-NOTSRCCOPY-DIB",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "NOTSRCCOPY DIB payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("middle"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "Do")
            .count()
            >= 2,
        "NOTSRCCOPY DIBs should render passive image XObjects"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-NOTSRCCOPY-DIB",
        b"TRAILING-WMF-NOTSRCCOPY-DIB",
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
            "NOTSRCCOPY DIB payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn notsrccopy_compressed_png_rasters_render_as_passive_inverted_images_without_payload_leakage() {
    let mut emf_png = minimal_rgb_png(&[[10, 20, 30], [200, 210, 220]]);
    emf_png.extend_from_slice(b"TRAILING-EMF-NOTSRCCOPY-PNG /JavaScript");
    let mut emf_dib = minimal_compressed_dib_with_payload(2, 1, 5, &emf_png);
    emf_dib.extend_from_slice(b"TRAILING-EMF-NOTSRCCOPY-PNG-DIB /Launch");
    let mut wmf_png = minimal_indexed_png_with_dimensions(2, 1);
    wmf_png.extend_from_slice(b"TRAILING-WMF-NOTSRCCOPY-PNG /EmbeddedFile");
    let mut wmf_dib = minimal_compressed_dib_with_payload(2, 1, 5, &wmf_png);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-NOTSRCCOPY-PNG-DIB /OpenAction");
    let emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[emf_stretchdibits_dib_record(
            20,
            15,
            60,
            30,
            0x0033_0008,
            &emf_dib,
        )],
    );
    let wmf = minimal_wmf_with_records(
        200,
        100,
        &[
            wmf_set_window_ext_record(200, 100),
            wmf_stretchdib_dib_record(35, 25, 80, 40, 0x0033_0008, &wmf_dib),
        ],
    );
    let emf_hex = bytes_to_hex(&emf);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} middle {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
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
    assert_eq!(images[0].vector_commands.len(), 1);
    assert_eq!(images[1].vector_commands.len(), 1);
    let StaticImageVectorCommand::RasterImage {
        image: emf_image, ..
    } = &images[0].vector_commands[0]
    else {
        panic!("expected EMF passive raster command");
    };
    assert_eq!(emf_image.format, ImageFormat::Png);
    let emf_scanlines =
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&emf_image.bytes, 7)
            .expect("inverted EMF PNG scanlines");
    assert_eq!(emf_scanlines, vec![0, 245, 235, 225, 55, 45, 35]);
    let StaticImageVectorCommand::RasterImage {
        image: wmf_image, ..
    } = &images[1].vector_commands[0]
    else {
        panic!("expected WMF passive raster command");
    };
    assert_eq!(wmf_image.format, ImageFormat::PngIndexed);
    assert_eq!(wmf_image.palette, vec![0, 255, 255, 255, 0, 255]);
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-NOTSRCCOPY-PNG",
        "TRAILING-EMF-NOTSRCCOPY-PNG-DIB",
        "TRAILING-WMF-NOTSRCCOPY-PNG",
        "TRAILING-WMF-NOTSRCCOPY-PNG-DIB",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
        "OpenAction",
    ] {
        assert!(
            !text.contains(forbidden),
            "compressed PNG NOTSRCCOPY payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("middle"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "Do")
            .count()
            >= 2,
        "compressed PNG NOTSRCCOPY DIBs should render passive image XObjects"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-NOTSRCCOPY-PNG",
        b"TRAILING-EMF-NOTSRCCOPY-PNG-DIB",
        b"TRAILING-WMF-NOTSRCCOPY-PNG",
        b"TRAILING-WMF-NOTSRCCOPY-PNG-DIB",
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
            "compressed PNG NOTSRCCOPY payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn notsrccopy_compressed_jpeg_rasters_use_passive_decode_without_payload_leakage() {
    let jpeg = minimal_jpeg_with_dimensions(2, 1);
    let mut emf_jpeg = jpeg.clone();
    emf_jpeg.extend_from_slice(b"TRAILING-EMF-NOTSRCCOPY-JPEG /JavaScript");
    let mut emf_dib = minimal_compressed_dib_with_payload(2, 1, 4, &emf_jpeg);
    emf_dib.extend_from_slice(b"TRAILING-EMF-NOTSRCCOPY-JPEG-DIB /Launch");
    let mut wmf_jpeg = jpeg.clone();
    wmf_jpeg.extend_from_slice(b"TRAILING-WMF-NOTSRCCOPY-JPEG /EmbeddedFile");
    let mut wmf_dib = minimal_compressed_dib_with_payload(2, 1, 4, &wmf_jpeg);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-NOTSRCCOPY-JPEG-DIB /OpenAction");
    let emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[emf_stretchdibits_dib_record(
            20,
            15,
            60,
            30,
            0x0033_0008,
            &emf_dib,
        )],
    );
    let wmf = minimal_wmf_with_records(
        200,
        100,
        &[
            wmf_set_window_ext_record(200, 100),
            wmf_stretchdib_dib_record(35, 25, 80, 40, 0x0033_0008, &wmf_dib),
        ],
    );
    let emf_hex = bytes_to_hex(&emf);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} middle {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
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
    assert_eq!(images[0].vector_commands.len(), 1);
    assert_eq!(images[1].vector_commands.len(), 1);
    for image in &images {
        let StaticImageVectorCommand::RasterImage { image, .. } = &image.vector_commands[0] else {
            panic!("expected passive inverted JPEG raster command");
        };
        assert_eq!(image.format, ImageFormat::JpegInverted);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.bytes, jpeg);
        assert!(image.palette.is_empty());
        assert!(image.alpha_mask.is_none());
    }
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-NOTSRCCOPY-JPEG",
        "TRAILING-EMF-NOTSRCCOPY-JPEG-DIB",
        "TRAILING-WMF-NOTSRCCOPY-JPEG",
        "TRAILING-WMF-NOTSRCCOPY-JPEG-DIB",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
        "OpenAction",
    ] {
        assert!(
            !text.contains(forbidden),
            "compressed JPEG NOTSRCCOPY payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("middle"));
    assert!(rendered_text.contains("after"));
    assert!([b"/Decode [1 0 1 0 1 0]".as_slice()].iter().all(|needle| {
        output
            .pdf
            .windows(needle.len())
            .any(|window| window == *needle)
    }));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "Do")
            .count()
            >= 2,
        "compressed JPEG NOTSRCCOPY DIBs should render passive image XObjects"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-NOTSRCCOPY-JPEG",
        b"TRAILING-EMF-NOTSRCCOPY-JPEG-DIB",
        b"TRAILING-WMF-NOTSRCCOPY-JPEG",
        b"TRAILING-WMF-NOTSRCCOPY-JPEG-DIB",
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
            "compressed JPEG NOTSRCCOPY payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn blank_srcinvert_compressed_png_rasters_render_without_payload_leakage() {
    let mut emf_png = minimal_rgb_png(&[[10, 20, 30], [200, 210, 220]]);
    emf_png.extend_from_slice(b"TRAILING-EMF-SRCINVERT-PNG /JavaScript");
    let mut emf_dib = minimal_compressed_dib_with_payload(2, 1, 5, &emf_png);
    emf_dib.extend_from_slice(b"TRAILING-EMF-SRCINVERT-PNG-DIB /Launch");
    let mut wmf_png = minimal_indexed_png_with_dimensions(2, 1);
    wmf_png.extend_from_slice(b"TRAILING-WMF-SRCINVERT-PNG /EmbeddedFile");
    let mut wmf_dib = minimal_compressed_dib_with_payload(2, 1, 5, &wmf_png);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-SRCINVERT-PNG-DIB /OpenAction");
    let emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[emf_stretchdibits_dib_record(
            20,
            15,
            60,
            30,
            0x0066_0046,
            &emf_dib,
        )],
    );
    let wmf = minimal_wmf_with_records(
        200,
        100,
        &[
            wmf_set_window_ext_record(200, 100),
            wmf_stretchdib_dib_record(35, 25, 80, 40, 0x0066_0046, &wmf_dib),
        ],
    );
    let emf_hex = bytes_to_hex(&emf);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} middle {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
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
    assert_eq!(images[0].vector_commands.len(), 1);
    assert_eq!(images[1].vector_commands.len(), 1);
    let StaticImageVectorCommand::RasterImage {
        image: emf_image, ..
    } = &images[0].vector_commands[0]
    else {
        panic!("expected EMF passive raster command");
    };
    assert_eq!(emf_image.format, ImageFormat::Png);
    let emf_scanlines =
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&emf_image.bytes, 7)
            .expect("inverted EMF PNG scanlines");
    assert_eq!(emf_scanlines, vec![0, 245, 235, 225, 55, 45, 35]);
    let StaticImageVectorCommand::RasterImage {
        image: wmf_image, ..
    } = &images[1].vector_commands[0]
    else {
        panic!("expected WMF passive raster command");
    };
    assert_eq!(wmf_image.format, ImageFormat::PngIndexed);
    assert_eq!(wmf_image.palette, vec![0, 255, 255, 255, 0, 255]);
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-SRCINVERT-PNG",
        "TRAILING-EMF-SRCINVERT-PNG-DIB",
        "TRAILING-WMF-SRCINVERT-PNG",
        "TRAILING-WMF-SRCINVERT-PNG-DIB",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
        "OpenAction",
    ] {
        assert!(
            !text.contains(forbidden),
            "compressed PNG SRCINVERT payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("middle"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "Do")
            .count()
            >= 2,
        "compressed PNG SRCINVERT DIBs should render passive image XObjects"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-SRCINVERT-PNG",
        b"TRAILING-EMF-SRCINVERT-PNG-DIB",
        b"TRAILING-WMF-SRCINVERT-PNG",
        b"TRAILING-WMF-SRCINVERT-PNG-DIB",
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
            "compressed PNG SRCINVERT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn compressed_dibs_inside_metafile_raster_records_render_without_raw_payload_leakage() {
    let jpeg = minimal_jpeg_with_dimensions(2, 1);
    let png = minimal_grayscale_png_with_dimensions(2, 1);
    let mut emf_jpeg_payload = jpeg.clone();
    emf_jpeg_payload.extend_from_slice(b"TRAILING-EMF-BI-JPEG /JavaScript");
    let mut emf_jpeg_dib = minimal_compressed_dib_with_payload(2, 1, 4, &emf_jpeg_payload);
    emf_jpeg_dib.extend_from_slice(b"TRAILING-EMF-BI-JPEG-DIB /Launch");
    let mut wmf_jpeg_payload = jpeg.clone();
    wmf_jpeg_payload.extend_from_slice(b"TRAILING-WMF-BI-JPEG /EmbeddedFile");
    let mut wmf_jpeg_dib = minimal_compressed_dib_with_payload(2, 1, 4, &wmf_jpeg_payload);
    wmf_jpeg_dib.extend_from_slice(b"TRAILING-WMF-BI-JPEG-DIB /Launch");
    let mut png_dib = minimal_compressed_dib_with_payload(2, 1, 5, &png);
    png_dib.extend_from_slice(b"TRAILING-WMF-BI-PNG /EmbeddedFile");

    let emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[emf_stretchdibits_dib_record(
            20,
            15,
            60,
            30,
            0x00cc_0020,
            &emf_jpeg_dib,
        )],
    );
    let wmf = minimal_wmf_with_records(
        200,
        100,
        &[
            wmf_stretchdib_dib_record(35, 25, 80, 40, 0x00cc_0020, &png_dib),
            wmf_dibstretchblt_record(90, 45, 70, 35, 0x00cc_0020, &wmf_jpeg_dib),
        ],
    );
    let emf_hex = bytes_to_hex(&emf);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} middle {{\\pict\\wmetafile8 {wmf_hex}}} after\\par}}"
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
    assert!(
        images
            .iter()
            .all(|image| image.format == ImageFormat::WmfVector)
    );
    assert!(images.iter().all(|image| image.bytes.is_empty()));
    assert_eq!(images[0].vector_commands.len(), 1);
    assert_eq!(images[1].vector_commands.len(), 2);
    assert!(matches!(
        &images[0].vector_commands[0],
        StaticImageVectorCommand::RasterImage { image, .. }
            if image.format == ImageFormat::Jpeg
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == jpeg
    ));
    assert!(matches!(
        &images[1].vector_commands[0],
        StaticImageVectorCommand::RasterImage { image, .. }
            if image.format == ImageFormat::PngGrayscale
                && image.width_px == 2
                && image.height_px == 1
                && image.alpha_mask.is_none()
    ));
    assert!(matches!(
        &images[1].vector_commands[1],
        StaticImageVectorCommand::RasterImage { image, .. }
            if image.format == ImageFormat::Jpeg
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == jpeg
    ));
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-BI-JPEG",
        "TRAILING-EMF-BI-JPEG-DIB",
        "TRAILING-WMF-BI-JPEG",
        "TRAILING-WMF-BI-JPEG-DIB",
        "TRAILING-WMF-BI-PNG",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "compressed metafile DIB payload/control leaked to text: {forbidden}"
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
    let image_paints = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "Do")
        .count();
    assert!(
        image_paints >= 3,
        "both metafile compressed DIBs should render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        b"TRAILING-EMF-BI-JPEG",
        b"TRAILING-EMF-BI-JPEG-DIB",
        b"TRAILING-WMF-BI-JPEG",
        b"TRAILING-WMF-BI-JPEG-DIB",
        b"TRAILING-WMF-BI-PNG",
        b"89504e470d0a1a0a",
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
            "compressed metafile DIB payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_srccopy_stretchdibits_crop_renders_as_passive_image_without_payload_leakage() {
    let dib = minimal_24bit_dib_with_rgb_pixels(
        3,
        2,
        &[
            [10, 20, 30],
            [40, 50, 60],
            [70, 80, 90],
            [100, 110, 120],
            [130, 140, 150],
            [160, 170, 180],
        ],
    );
    let mut record = emf_stretchdibits_dib_record(20, 15, 60, 30, 0x00cc_0020, &dib);
    write_test_le_i32(&mut record, 32, 1);
    write_test_le_i32(&mut record, 36, 1);
    write_test_le_i32(&mut record, 40, 2);
    write_test_le_i32(&mut record, 44, 1);
    let records = [record];
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
        .expect("passive cropped EMF SRCCOPY STRETCHDIBITS vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        &image.vector_commands[0],
        StaticImageVectorCommand::RasterImage {
            left: 20.0,
            top: 15.0,
            right: 80.0,
            bottom: 45.0,
            image,
        } if image.format == ImageFormat::Rgb8
            && image.width_px == 2
            && image.height_px == 1
            && image.bytes == vec![130, 140, 150, 160, 170, 180]
    ));
    assert!(!text.contains("emfblip"));
    assert!(!text.contains("STRETCHDIBITS"));

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
        output
            .pdf
            .windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image"),
        "cropped EMF SRCCOPY STRETCHDIBITS should render a passive image XObject"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "Do"),
        "cropped EMF SRCCOPY STRETCHDIBITS should invoke a passive PDF image XObject"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"STRETCHDIBITS",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "cropped EMF SRCCOPY STRETCHDIBITS payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_srccopy_stretchdibits_jpeg_crop_renders_clipped_without_payload_leakage() {
    let jpeg = minimal_jpeg_with_dimensions(3, 2);
    let mut jpeg_payload = jpeg.clone();
    jpeg_payload.extend_from_slice(b"TRAILING-EMF-STRETCHDIBITS-JPEG-CROP /JavaScript");
    let mut dib = minimal_compressed_dib_with_payload(3, 2, 4, &jpeg_payload);
    dib.extend_from_slice(b"TRAILING-EMF-STRETCHDIBITS-JPEG-CROP-DIB /EmbeddedFile");
    let mut record = emf_stretchdibits_dib_record(20, 15, 60, 30, 0x00cc_0020, &dib);
    write_test_le_i32(&mut record, 32, 1);
    write_test_le_i32(&mut record, 36, 0);
    write_test_le_i32(&mut record, 40, 1);
    write_test_le_i32(&mut record, 44, 2);
    let records = [record];
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
        .expect("passive JPEG-cropped EMF SRCCOPY STRETCHDIBITS vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 4);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::SaveState
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::ClipRect {
            left: 20.0,
            top: 15.0,
            right: 80.0,
            bottom: 45.0,
        }
    ));
    assert!(matches!(
        &image.vector_commands[2],
        StaticImageVectorCommand::RasterImage {
            left: -40.0,
            top: 15.0,
            right: 140.0,
            bottom: 45.0,
            image,
        } if image.format == ImageFormat::Jpeg
            && image.width_px == 3
            && image.height_px == 2
            && image.bytes == jpeg
            && image.alpha_mask.is_none()
    ));
    assert!(matches!(
        image.vector_commands[3],
        StaticImageVectorCommand::RestoreState
    ));
    for forbidden in [
        "emfblip",
        "STRETCHDIBITS",
        "TRAILING-EMF-STRETCHDIBITS-JPEG-CROP",
        "TRAILING-EMF-STRETCHDIBITS-JPEG-CROP-DIB",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "JPEG-cropped EMF STRETCHDIBITS payload/control leaked to normalized text: {forbidden}"
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
        output
            .pdf
            .windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image"),
        "JPEG-cropped EMF SRCCOPY STRETCHDIBITS should render a passive image XObject"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "Do"),
        "JPEG-cropped EMF SRCCOPY STRETCHDIBITS should invoke a passive PDF image XObject"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "W"),
        "JPEG-cropped EMF SRCCOPY STRETCHDIBITS should clip the full JPEG draw"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"STRETCHDIBITS",
        b"TRAILING-EMF-STRETCHDIBITS-JPEG-CROP",
        b"TRAILING-EMF-STRETCHDIBITS-JPEG-CROP-DIB",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "JPEG-cropped EMF STRETCHDIBITS payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_srccopy_stretchdibits_grayscale_jpeg_crop_renders_clipped_without_payload_leakage() {
    let jpeg = minimal_grayscale_jpeg_with_dimensions(3, 2);
    let mut jpeg_payload = jpeg.clone();
    jpeg_payload.extend_from_slice(b"TRAILING-EMF-STRETCHDIBITS-GRAY-JPEG-CROP /JavaScript");
    let mut dib = minimal_compressed_dib_with_payload(3, 2, 4, &jpeg_payload);
    dib.extend_from_slice(b"TRAILING-EMF-STRETCHDIBITS-GRAY-JPEG-CROP-DIB /EmbeddedFile");
    let mut record = emf_stretchdibits_dib_record(20, 15, 60, 30, 0x00cc_0020, &dib);
    write_test_le_i32(&mut record, 32, 1);
    write_test_le_i32(&mut record, 36, 0);
    write_test_le_i32(&mut record, 40, 1);
    write_test_le_i32(&mut record, 44, 2);
    let records = [record];
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
        .expect("passive grayscale-JPEG-cropped EMF SRCCOPY STRETCHDIBITS vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 4);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::SaveState
    ));
    assert_eq!(
        image.vector_commands[1],
        StaticImageVectorCommand::ClipRect {
            left: 20.0,
            top: 15.0,
            right: 80.0,
            bottom: 45.0,
        }
    );
    assert!(matches!(
        &image.vector_commands[2],
        StaticImageVectorCommand::RasterImage {
            left: -40.0,
            top: 15.0,
            right: 140.0,
            bottom: 45.0,
            image,
        } if image.format == ImageFormat::JpegGrayscale
            && image.width_px == 3
            && image.height_px == 2
            && image.bytes == jpeg
            && image.alpha_mask.is_none()
    ));
    assert!(matches!(
        image.vector_commands[3],
        StaticImageVectorCommand::RestoreState
    ));
    for forbidden in [
        "emfblip",
        "STRETCHDIBITS",
        "TRAILING-EMF-STRETCHDIBITS-GRAY-JPEG-CROP",
        "TRAILING-EMF-STRETCHDIBITS-GRAY-JPEG-CROP-DIB",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "grayscale JPEG-cropped EMF STRETCHDIBITS payload/control leaked to normalized text: {forbidden}"
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
        output
            .pdf
            .windows(b"/DCTDecode".len())
            .any(|window| window == b"/DCTDecode"),
        "grayscale JPEG-cropped EMF SRCCOPY STRETCHDIBITS should keep the passive JPEG stream"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "Do"),
        "grayscale JPEG-cropped EMF SRCCOPY STRETCHDIBITS should invoke a passive PDF image XObject"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "W"),
        "grayscale JPEG-cropped EMF SRCCOPY STRETCHDIBITS should clip the full JPEG draw"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"STRETCHDIBITS",
        b"TRAILING-EMF-STRETCHDIBITS-GRAY-JPEG-CROP",
        b"TRAILING-EMF-STRETCHDIBITS-GRAY-JPEG-CROP-DIB",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "grayscale JPEG-cropped EMF STRETCHDIBITS payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_blackness_and_whiteness_raster_transfers_render_passively_without_payload_leakage() {
    let patinvert_payload = b"PATINVERT-SOURCE-PAYLOAD";
    let notpatcopy_payload = b"NOTPATCOPY-SOURCE-PAYLOAD";
    let blackness_payload = b"BLACKNESS-SOURCE-PAYLOAD";
    let whiteness_payload = b"WHITENESS-STRETCH-PAYLOAD";
    let dib_payload = b"BLACKNESS-DIB-PAYLOAD";
    let dstcopy_payload = b"DSTCOPY-NOOP-SOURCE-PAYLOAD";
    let backdrop_dstinvert_payload = b"BACKDROP-DSTINVERT-PAYLOAD";
    let backdrop_patinvert_payload = b"BACKDROP-PATINVERT-PAYLOAD";
    let unsupported_payload = b"UNSUPPORTED-SRCCOPY-PAYLOAD";
    let records = [
        emf_create_brush_record(
            3,
            0,
            Color {
                red: 20,
                green: 80,
                blue: 140,
            },
            0,
        ),
        emf_select_object_record(3),
        emf_bitblt_record(4, 8, 24, 12, 0x005a_0049, patinvert_payload),
        emf_bitblt_record(10, 20, 30, 15, 0x0000_0042, blackness_payload),
        emf_stretchblt_record(50, 25, 35, 20, 0x00ff_0062, whiteness_payload),
        emf_bitblt_record(62, 10, 24, 12, 0x000f_0001, notpatcopy_payload),
        emf_stretchdibits_record(90, 30, 40, 25, 0x0000_0042, dib_payload),
        emf_bitblt_record(96, 8, 18, 10, 0x00aa_0029, dstcopy_payload),
        emf_bitblt_record(110, 12, 20, 10, 0x0055_0009, backdrop_dstinvert_payload),
        emf_bitblt_record(112, 24, 20, 10, 0x005a_0049, backdrop_patinvert_payload),
        emf_bitblt_record(15, 35, 20, 10, 0x00cc_0020, unsupported_payload),
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
        .expect("passive EMF solid raster transfer vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 5);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 4.0,
            top: 8.0,
            right: 28.0,
            bottom: 20.0,
            stroke_color: None,
            fill_color: Some(Color {
                red: 235,
                green: 175,
                blue: 115
            }),
            ..
        }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Rectangle {
            left: 10.0,
            top: 20.0,
            right: 40.0,
            bottom: 35.0,
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
        image.vector_commands[2],
        StaticImageVectorCommand::Rectangle {
            left: 50.0,
            top: 25.0,
            right: 85.0,
            bottom: 45.0,
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
        image.vector_commands[3],
        StaticImageVectorCommand::Rectangle {
            left: 62.0,
            top: 10.0,
            right: 86.0,
            bottom: 22.0,
            stroke_color: None,
            fill_color: Some(Color {
                red: 235,
                green: 175,
                blue: 115
            }),
            ..
        }
    ));
    assert!(matches!(
        image.vector_commands[4],
        StaticImageVectorCommand::Rectangle {
            left: 90.0,
            top: 30.0,
            right: 130.0,
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
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("unsupported record(s) skipped") })
    );
    for forbidden in [
        "emfblip",
        "PATINVERT-SOURCE-PAYLOAD",
        "NOTPATCOPY-SOURCE-PAYLOAD",
        "BLACKNESS-SOURCE-PAYLOAD",
        "WHITENESS-STRETCH-PAYLOAD",
        "BLACKNESS-DIB-PAYLOAD",
        "DSTCOPY-NOOP-SOURCE-PAYLOAD",
        "BACKDROP-DSTINVERT-PAYLOAD",
        "BACKDROP-PATINVERT-PAYLOAD",
        "UNSUPPORTED-SRCCOPY-PAYLOAD",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF solid raster transfer payload/control leaked to normalized text: {forbidden}"
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
            >= 5,
        "EMF solid and blank invert transfers should render passive PDF rectangles"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "f*" | "B" | "B*")),
        "EMF BLACKNESS/WHITENESS transfers should render passive PDF fills"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        patinvert_payload.as_slice(),
        notpatcopy_payload.as_slice(),
        blackness_payload.as_slice(),
        whiteness_payload.as_slice(),
        dib_payload.as_slice(),
        dstcopy_payload.as_slice(),
        backdrop_dstinvert_payload.as_slice(),
        backdrop_patinvert_payload.as_slice(),
        unsupported_payload.as_slice(),
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
            "EMF solid raster transfer payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_miter_state_before_blank_raster_stays_unpainted_without_payload_leakage() {
    let mut miter = emf_f32_record(58, 2.0);
    miter.extend_from_slice(b"EMF-MITER-BEFORE-BLANK-RASTER /JavaScript");
    miter.resize(miter.len().next_multiple_of(4), 0);
    let miter_len = miter.len() as u32;
    write_test_le_u32(&mut miter, 4, miter_len);

    let payload = b"EMF-BLANK-DSTINVERT-AFTER-MITER /EmbeddedFile /Launch";
    let records = [
        miter,
        emf_bitblt_record(10, 20, 30, 15, 0x0055_0009, payload),
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
        .expect("passive EMF miter state before blank raster vector image");

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
        StaticImageVectorCommand::Rectangle {
            left: 10.0,
            top: 20.0,
            right: 40.0,
            bottom: 35.0,
            stroke_color: None,
            fill_color: Some(Color {
                red: 0,
                green: 0,
                blue: 0
            }),
            ..
        }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "EMF-MITER-BEFORE-BLANK-RASTER",
        "EMF-BLANK-DSTINVERT-AFTER-MITER",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF miter/blank-raster payload/control leaked to normalized text: {forbidden}"
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
            .any(|operation| operation.operator == "M"),
        "miter state before blank raster should still render passive PDF miter state"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "blank-destination DSTINVERT after miter state should render passive black rectangle"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-MITER-BEFORE-BLANK-RASTER",
        payload.as_slice(),
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
            "EMF miter/blank-raster payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn strict_spec_same_bounds_backdrop_raster_transfers_reduce_to_passive_solid_rectangles_without_payload_leakage()
 {
    let emf_payload = b"EMF-SAME-BOUNDS-DSTINVERT /JavaScript";
    let wmf_payload = b"WMF-SAME-BOUNDS-PATINVERT /EmbeddedFile";
    let fill_color = Color {
        red: 100,
        green: 30,
        blue: 200,
    };
    let xor_color = Color {
        red: 20,
        green: 80,
        blue: 140,
    };
    let emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[
            emf_create_brush_record(3, 0, fill_color, 0),
            emf_select_object_record(3),
            emf_bitblt_record(10, 20, 80, 40, 0x00f0_0021, b"EMF-PATCOPY-FILL"),
            emf_bitblt_record(10, 20, 80, 40, 0x0055_0009, emf_payload),
        ],
    );
    let wmf = minimal_wmf_with_records(
        200,
        100,
        &[
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(fill_color),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(15, 25, 80, 40, 0x00f0_0021, b"WMF-PATCOPY-FILL"),
            wmf_create_brush_record(xor_color),
            wmf_select_object_record(1),
            wmf_dibbitblt_record(15, 25, 80, 40, 0x005a_0049, wmf_payload),
        ],
    );
    let emf_hex = bytes_to_hex(&emf);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} middle {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes_strict(&input);
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
    assert_eq!(images[0].vector_commands.len(), 1);
    assert_eq!(images[1].vector_commands.len(), 1);
    assert!(matches!(
        images[0].vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            fill_color: Some(Color {
                red: 155,
                green: 225,
                blue: 55
            }),
            ..
        }
    ));
    assert!(matches!(
        images[1].vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            fill_color: Some(Color {
                red: 112,
                green: 78,
                blue: 68
            }),
            ..
        }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "EMF-SAME-BOUNDS-DSTINVERT",
        "WMF-SAME-BOUNDS-PATINVERT",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "same-bounds backdrop raster payload/control leaked to normalized text: {forbidden}"
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
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("middle"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "re")
            .count()
            >= 2,
        "same-bounds backdrop raster reductions should render passive PDF rectangles"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        emf_payload.as_slice(),
        wmf_payload.as_slice(),
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
            "same-bounds backdrop raster payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn black_brush_patinvert_raster_transfers_are_noops_without_payload_leakage() {
    let emf_payload = b"EMF-BLACK-PATINVERT-NOOP-PAYLOAD /JavaScript";
    let wmf_payload = b"WMF-BLACK-PATINVERT-NOOP-PAYLOAD /EmbeddedFile";
    let emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[
            emf_create_brush_record(3, 0, Color::default(), 0),
            emf_select_object_record(3),
            emf_rect_record(43, 0, 0, 30, 30),
            emf_bitblt_record(10, 20, 30, 15, 0x005a_0049, emf_payload),
        ],
    );
    let wmf = minimal_wmf_with_records(
        200,
        100,
        &[
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color::default()),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(15, 25, 80, 40, 0x00f0_0021, b"WMF-PATCOPY-FILL"),
            wmf_dibbitblt_record(20, 30, 80, 40, 0x005a_0049, wmf_payload),
        ],
    );
    let emf_hex = bytes_to_hex(&emf);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} middle {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
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
    assert!(images.iter().all(|image| {
        image.format == ImageFormat::WmfVector
            && image.bytes.is_empty()
            && image.vector_commands.len() == 1
    }));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "EMF-BLACK-PATINVERT-NOOP-PAYLOAD",
        "WMF-BLACK-PATINVERT-NOOP-PAYLOAD",
        "WMF-PATCOPY-FILL",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "black PATINVERT noop payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("middle"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "re")
            .count()
            >= 2,
        "only the visible paint before black PATINVERT noops should render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        emf_payload.as_slice(),
        wmf_payload.as_slice(),
        b"WMF-PATCOPY-FILL",
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
            "black PATINVERT noop payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn white_brush_patpaint_raster_transfers_render_without_payload_leakage() {
    let emf_payload = b"EMF-WHITE-PATPAINT-PAYLOAD /JavaScript";
    let wmf_payload = b"WMF-WHITE-PATPAINT-PAYLOAD /EmbeddedFile";
    let white = Color {
        red: u8::MAX,
        green: u8::MAX,
        blue: u8::MAX,
    };
    let emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[
            emf_create_brush_record(3, 0, white, 0),
            emf_select_object_record(3),
            emf_rect_record(43, 0, 0, 30, 30),
            emf_bitblt_record(10, 20, 30, 15, 0x00fb_0a09, emf_payload),
        ],
    );
    let wmf = minimal_wmf_with_records(
        200,
        100,
        &[
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(white),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(15, 25, 80, 40, 0x00f0_0021, b"WMF-PATCOPY-FILL"),
            wmf_dibbitblt_record(20, 30, 80, 40, 0x00fb_0a09, wmf_payload),
        ],
    );
    let emf_hex = bytes_to_hex(&emf);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} middle {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
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
    assert!(images.iter().all(|image| {
        image.format == ImageFormat::WmfVector
            && image.bytes.is_empty()
            && image.vector_commands.len() == 2
    }));
    for image in &images {
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle {
                stroke_color: None,
                fill_color: Some(Color {
                    red: 255,
                    green: 255,
                    blue: 255
                }),
                ..
            }
        ));
    }
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "EMF-WHITE-PATPAINT-PAYLOAD",
        "WMF-WHITE-PATPAINT-PAYLOAD",
        "WMF-PATCOPY-FILL",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "white PATPAINT payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("middle"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "re")
            .count()
            >= 4,
        "white PATPAINT transfers should render passive rectangles"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        emf_payload.as_slice(),
        wmf_payload.as_slice(),
        b"WMF-PATCOPY-FILL",
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
            "white PATPAINT source payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn strict_spec_source_backed_patinvert_ignores_dib_source_without_payload_leakage() {
    let mut emf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[1, 2, 3], [250, 251, 252]]);
    emf_dib.extend_from_slice(b"TRAILING-EMF-PATINVERT-DIB-SOURCE /JavaScript");
    let mut wmf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[3, 2, 1], [252, 251, 250]]);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-PATINVERT-DIB-SOURCE /EmbeddedFile");
    let fill_color = Color {
        red: 100,
        green: 30,
        blue: 200,
    };
    let pattern_color = Color {
        red: 20,
        green: 80,
        blue: 140,
    };
    let emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[
            emf_create_brush_record(3, 0, fill_color, 0),
            emf_select_object_record(3),
            emf_bitblt_record(20, 15, 60, 30, 0x00f0_0021, b"EMF-PATCOPY-FILL"),
            emf_create_brush_record(4, 0, pattern_color, 0),
            emf_select_object_record(4),
            emf_stretchdibits_dib_record(20, 15, 60, 30, 0x005a_0049, &emf_dib),
        ],
    );
    let wmf = minimal_wmf_with_records(
        200,
        100,
        &[
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(fill_color),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(15, 25, 80, 40, 0x00f0_0021, b"WMF-PATCOPY-FILL"),
            wmf_create_brush_record(pattern_color),
            wmf_select_object_record(1),
            wmf_stretchdib_dib_record(15, 25, 80, 40, 0x005a_0049, &wmf_dib),
        ],
    );
    let emf_hex = bytes_to_hex(&emf);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} middle {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes_strict(&input);
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
    assert_eq!(images[0].vector_commands.len(), 1);
    assert_eq!(images[1].vector_commands.len(), 1);
    for image in &images {
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                fill_color: Some(Color {
                    red: 112,
                    green: 78,
                    blue: 68
                }),
                ..
            }
        ));
    }
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-PATINVERT-DIB-SOURCE",
        "TRAILING-WMF-PATINVERT-DIB-SOURCE",
        "EMF-PATCOPY-FILL",
        "WMF-PATCOPY-FILL",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "source-backed PATINVERT payload/control leaked to normalized text: {forbidden}"
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
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("middle"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "re")
            .count()
            >= 2,
        "source-backed PATINVERT reductions should render passive PDF rectangles"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-PATINVERT-DIB-SOURCE",
        b"TRAILING-WMF-PATINVERT-DIB-SOURCE",
        b"EMF-PATCOPY-FILL",
        b"WMF-PATCOPY-FILL",
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
            "source-backed PATINVERT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn solid_source_raster_reductions_render_without_payload_leakage() {
    let mut emf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[0, 0, 0], [0, 0, 0]]);
    emf_dib.extend_from_slice(b"TRAILING-EMF-SRCAND-BLACK-DIB /JavaScript");
    let mut wmf_png = minimal_grayscale_png(&[255, 255]);
    wmf_png.extend_from_slice(b"TRAILING-WMF-SRCPAINT-WHITE-PNG /EmbeddedFile");
    let mut wmf_dib = minimal_compressed_dib_with_payload(2, 1, 5, &wmf_png);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-SRCPAINT-WHITE-DIB /OpenAction");
    let emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[
            emf_rect_record(43, 0, 0, 30, 30),
            emf_stretchdibits_dib_record(20, 15, 60, 30, 0x0088_00c6, &emf_dib),
        ],
    );
    let wmf = minimal_wmf_with_records(
        200,
        100,
        &[
            wmf_set_window_ext_record(200, 100),
            wmf_dibbitblt_record(15, 25, 80, 40, 0x00ff_0062, b"WMF-WHITENESS-PAINT"),
            wmf_stretchdib_dib_record(35, 25, 80, 40, 0x00ee_0086, &wmf_dib),
        ],
    );
    let emf_hex = bytes_to_hex(&emf);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} middle {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
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
    assert_eq!(images[0].vector_commands.len(), 2);
    assert_eq!(images[1].vector_commands.len(), 2);
    assert!(matches!(
        images[0].vector_commands[1],
        StaticImageVectorCommand::Rectangle {
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
        images[1].vector_commands[1],
        StaticImageVectorCommand::Rectangle {
            stroke_color: None,
            fill_color: Some(Color {
                red: 255,
                green: 255,
                blue: 255
            }),
            ..
        }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-SRCAND-BLACK-DIB",
        "TRAILING-WMF-SRCPAINT-WHITE-PNG",
        "TRAILING-WMF-SRCPAINT-WHITE-DIB",
        "WMF-WHITENESS-PAINT",
        "JavaScript",
        "EmbeddedFile",
        "OpenAction",
    ] {
        assert!(
            !text.contains(forbidden),
            "solid source raster payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("middle"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "re")
            .count()
            >= 4,
        "solid source raster reductions should render passive rectangles"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-SRCAND-BLACK-DIB",
        b"TRAILING-WMF-SRCPAINT-WHITE-PNG",
        b"TRAILING-WMF-SRCPAINT-WHITE-DIB",
        b"WMF-WHITENESS-PAINT",
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
            "solid source raster payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn source_raster_noop_reductions_render_without_payload_leakage() {
    let mut emf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[0, 0, 0], [0, 0, 0]]);
    emf_dib.extend_from_slice(b"TRAILING-EMF-SRCINVERT-BLACK-DIB /JavaScript");
    let mut emf_srcerase_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[0, 0, 0], [0, 0, 0]]);
    emf_srcerase_dib.extend_from_slice(b"TRAILING-EMF-SRCERASE-BLACK-DIB /OpenAction");
    let mut wmf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[255, 255, 255], [255, 255, 255]]);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-SRCAND-WHITE-DIB /EmbeddedFile");
    let mut wmf_srcerase_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[0, 0, 0], [0, 0, 0]]);
    wmf_srcerase_dib.extend_from_slice(b"TRAILING-WMF-SRCERASE-BLACK-DIB /RichMedia");
    let emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[
            emf_rect_record(43, 0, 0, 30, 30),
            emf_stretchdibits_dib_record(20, 15, 60, 30, 0x0066_0046, &emf_dib),
            emf_stretchdibits_dib_record(70, 15, 40, 20, 0x0044_0328, &emf_srcerase_dib),
        ],
    );
    let wmf = minimal_wmf_with_records(
        200,
        100,
        &[
            wmf_set_window_ext_record(200, 100),
            wmf_dibbitblt_record(15, 25, 80, 40, 0x0000_0042, b"WMF-BLACKNESS-PAINT"),
            wmf_stretchdib_dib_record(35, 25, 80, 40, 0x0088_00c6, &wmf_dib),
            wmf_stretchdib_dib_record(75, 35, 60, 30, 0x0044_0328, &wmf_srcerase_dib),
        ],
    );
    let emf_hex = bytes_to_hex(&emf);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} middle {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
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
    assert_eq!(images[0].vector_commands.len(), 1);
    assert_eq!(images[1].vector_commands.len(), 1);
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-SRCINVERT-BLACK-DIB",
        "TRAILING-EMF-SRCERASE-BLACK-DIB",
        "TRAILING-WMF-SRCAND-WHITE-DIB",
        "TRAILING-WMF-SRCERASE-BLACK-DIB",
        "WMF-BLACKNESS-PAINT",
        "JavaScript",
        "EmbeddedFile",
        "OpenAction",
        "RichMedia",
    ] {
        assert!(
            !text.contains(forbidden),
            "source raster noop payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("middle"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "re")
            .count()
            >= 2,
        "only the visible paint before source raster noops should render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-SRCINVERT-BLACK-DIB",
        b"TRAILING-EMF-SRCERASE-BLACK-DIB",
        b"TRAILING-WMF-SRCAND-WHITE-DIB",
        b"TRAILING-WMF-SRCERASE-BLACK-DIB",
        b"WMF-BLACKNESS-PAINT",
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
            "source raster noop payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn patpaint_source_raster_reductions_render_without_payload_leakage() {
    let mut emf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[0, 0, 0], [0, 0, 0]]);
    emf_dib.extend_from_slice(b"TRAILING-EMF-PATPAINT-BLACK-DIB /JavaScript");
    let mut wmf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[255, 255, 255], [255, 255, 255]]);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-PATPAINT-WHITE-DIB /EmbeddedFile");
    let emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 20,
                    green: 80,
                    blue: 140,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_rect_record(43, 0, 0, 30, 30),
            emf_stretchdibits_dib_record(20, 15, 60, 30, 0x00fb_0a09, &emf_dib),
        ],
    );
    let wmf = minimal_wmf_with_records(
        200,
        100,
        &[
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color::default()),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(15, 25, 80, 40, 0x00ff_0062, b"WMF-WHITENESS-PAINT"),
            wmf_stretchdib_dib_record(35, 25, 80, 40, 0x00fb_0a09, &wmf_dib),
        ],
    );
    let emf_hex = bytes_to_hex(&emf);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} middle {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
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
    assert_eq!(images[0].vector_commands.len(), 2);
    assert_eq!(images[1].vector_commands.len(), 1);
    assert!(matches!(
        images[0].vector_commands[1],
        StaticImageVectorCommand::Rectangle {
            stroke_color: None,
            fill_color: Some(Color {
                red: 255,
                green: 255,
                blue: 255
            }),
            ..
        }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-PATPAINT-BLACK-DIB",
        "TRAILING-WMF-PATPAINT-WHITE-DIB",
        "WMF-WHITENESS-PAINT",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "PATPAINT source raster payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("middle"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "re")
            .count()
            >= 3,
        "PATPAINT reductions should render only passive rectangles"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-PATPAINT-BLACK-DIB",
        b"TRAILING-WMF-PATPAINT-WHITE-DIB",
        b"WMF-WHITENESS-PAINT",
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
            "PATPAINT source raster payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn black_brush_mergecopy_raster_transfers_render_passively_without_payload_leakage() {
    let emf_payload = b"EMF-MERGECOPY-SOURCE-PAYLOAD /JavaScript";
    let wmf_payload = b"WMF-MERGECOPY-SOURCE-PAYLOAD /EmbeddedFile";
    let emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[
            emf_create_brush_record(3, 0, Color::default(), 0),
            emf_select_object_record(3),
            emf_bitblt_record(10, 20, 30, 15, 0x00c0_00ca, emf_payload),
        ],
    );
    let wmf = minimal_wmf_with_records(
        200,
        100,
        &[
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color::default()),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(15, 25, 80, 40, 0x00c0_00ca, wmf_payload),
        ],
    );
    let emf_hex = bytes_to_hex(&emf);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} middle {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
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
    assert!(images.iter().all(|image| {
        image.format == ImageFormat::WmfVector
            && image.bytes.is_empty()
            && image.vector_commands.len() == 1
    }));
    for image in &images {
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                stroke_color: None,
                fill_color: Some(Color {
                    red: 0,
                    green: 0,
                    blue: 0
                }),
                ..
            }
        ));
    }
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "EMF-MERGECOPY-SOURCE-PAYLOAD",
        "WMF-MERGECOPY-SOURCE-PAYLOAD",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "MERGECOPY payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("middle"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "re")
            .count()
            >= 2,
        "black-brush MERGECOPY transfers should render passive PDF rectangles"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        emf_payload.as_slice(),
        wmf_payload.as_slice(),
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
            "MERGECOPY payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn nonblack_mergecopy_embedded_rasters_render_without_payload_leakage() {
    let mut emf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[255, 63, 170], [15, 240, 85]]);
    emf_dib.extend_from_slice(b"TRAILING-EMF-MERGECOPY-DIB /JavaScript");
    let mut wmf_png = minimal_indexed_png_with_dimensions(2, 1);
    wmf_png.extend_from_slice(b"TRAILING-WMF-MERGECOPY-PNG /EmbeddedFile");
    let mut wmf_dib = minimal_compressed_dib_with_payload(2, 1, 5, &wmf_png);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-MERGECOPY-PNG-DIB /OpenAction");
    let brush_color = Color {
        red: 240,
        green: 15,
        blue: 170,
    };
    let emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[
            emf_create_brush_record(3, 0, brush_color, 0),
            emf_select_object_record(3),
            emf_rect_record(43, 0, 0, 30, 30),
            emf_stretchdibits_dib_record(20, 15, 60, 30, 0x00c0_00ca, &emf_dib),
        ],
    );
    let wmf = minimal_wmf_with_records(
        200,
        100,
        &[
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(brush_color),
            wmf_select_object_record(0),
            wmf_stretchdib_dib_record(35, 25, 80, 40, 0x00c0_00ca, &wmf_dib),
        ],
    );
    let emf_hex = bytes_to_hex(&emf);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} middle {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
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
    assert_eq!(images[0].vector_commands.len(), 2);
    assert_eq!(images[1].vector_commands.len(), 1);
    let StaticImageVectorCommand::RasterImage {
        image: emf_image, ..
    } = &images[0].vector_commands[1]
    else {
        panic!("expected EMF passive raster command");
    };
    assert_eq!(emf_image.format, ImageFormat::Rgb8);
    assert_eq!(emf_image.bytes, vec![240, 15, 170, 0, 0, 0]);
    let StaticImageVectorCommand::RasterImage {
        image: wmf_image, ..
    } = &images[1].vector_commands[0]
    else {
        panic!("expected WMF passive raster command");
    };
    assert_eq!(wmf_image.format, ImageFormat::PngIndexed);
    assert_eq!(wmf_image.palette, vec![240, 0, 0, 0, 15, 0]);
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-MERGECOPY-DIB",
        "TRAILING-WMF-MERGECOPY-PNG",
        "TRAILING-WMF-MERGECOPY-PNG-DIB",
        "JavaScript",
        "EmbeddedFile",
        "OpenAction",
    ] {
        assert!(
            !text.contains(forbidden),
            "MERGECOPY source payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("middle"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "Do")
            .count()
            >= 2,
        "nonblack MERGECOPY embedded rasters should render passive image XObjects"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-MERGECOPY-DIB",
        b"TRAILING-WMF-MERGECOPY-PNG",
        b"TRAILING-WMF-MERGECOPY-PNG-DIB",
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
            "MERGECOPY source payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn white_brush_mergecopy_jpeg_sources_render_without_payload_leakage() {
    let jpeg = minimal_jpeg_with_dimensions(2, 1);
    let mut emf_jpeg_payload = jpeg.clone();
    emf_jpeg_payload.extend_from_slice(b"TRAILING-EMF-MERGECOPY-JPEG /JavaScript");
    let mut emf_dib = minimal_compressed_dib_with_payload(2, 1, 4, &emf_jpeg_payload);
    emf_dib.extend_from_slice(b"TRAILING-EMF-MERGECOPY-JPEG-DIB /Launch");
    let mut wmf_jpeg_payload = jpeg.clone();
    wmf_jpeg_payload.extend_from_slice(b"TRAILING-WMF-MERGECOPY-JPEG /EmbeddedFile");
    let mut wmf_dib = minimal_compressed_dib_with_payload(2, 1, 4, &wmf_jpeg_payload);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-MERGECOPY-JPEG-DIB /OpenAction");
    let white = Color {
        red: u8::MAX,
        green: u8::MAX,
        blue: u8::MAX,
    };
    let emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[
            emf_create_brush_record(3, 0, white, 0),
            emf_select_object_record(3),
            emf_stretchdibits_dib_record(20, 15, 60, 30, 0x00c0_00ca, &emf_dib),
        ],
    );
    let wmf = minimal_wmf_with_records(
        200,
        100,
        &[
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(white),
            wmf_select_object_record(0),
            wmf_stretchdib_dib_record(35, 25, 80, 40, 0x00c0_00ca, &wmf_dib),
        ],
    );
    let emf_hex = bytes_to_hex(&emf);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} middle {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
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
    assert_eq!(images[0].vector_commands.len(), 1);
    assert_eq!(images[1].vector_commands.len(), 1);
    for image in &images {
        let StaticImageVectorCommand::RasterImage { image, .. } = &image.vector_commands[0] else {
            panic!("expected passive JPEG raster command");
        };
        assert_eq!(image.format, ImageFormat::Jpeg);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.bytes, jpeg);
        assert!(image.palette.is_empty());
        assert!(image.alpha_mask.is_none());
    }
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-MERGECOPY-JPEG",
        "TRAILING-EMF-MERGECOPY-JPEG-DIB",
        "TRAILING-WMF-MERGECOPY-JPEG",
        "TRAILING-WMF-MERGECOPY-JPEG-DIB",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
        "OpenAction",
    ] {
        assert!(
            !text.contains(forbidden),
            "white MERGECOPY JPEG payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("middle"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "Do")
            .count()
            >= 2,
        "white MERGECOPY JPEG sources should render passive image XObjects"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-MERGECOPY-JPEG",
        b"TRAILING-EMF-MERGECOPY-JPEG-DIB",
        b"TRAILING-WMF-MERGECOPY-JPEG",
        b"TRAILING-WMF-MERGECOPY-JPEG-DIB",
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
            "white MERGECOPY JPEG source payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn black_brush_mergecopy_jpeg_sources_render_solid_without_payload_leakage() {
    let jpeg = minimal_jpeg_with_dimensions(2, 1);
    let mut emf_jpeg_payload = jpeg.clone();
    emf_jpeg_payload.extend_from_slice(b"TRAILING-EMF-BLACK-MERGECOPY-JPEG /JavaScript");
    let mut emf_dib = minimal_compressed_dib_with_payload(2, 1, 4, &emf_jpeg_payload);
    emf_dib.extend_from_slice(b"TRAILING-EMF-BLACK-MERGECOPY-JPEG-DIB /Launch");
    let mut wmf_jpeg_payload = jpeg;
    wmf_jpeg_payload.extend_from_slice(b"TRAILING-WMF-BLACK-MERGECOPY-JPEG /EmbeddedFile");
    let mut wmf_dib = minimal_compressed_dib_with_payload(2, 1, 4, &wmf_jpeg_payload);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-BLACK-MERGECOPY-JPEG-DIB /OpenAction");
    let emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[
            emf_create_brush_record(3, 0, Color::default(), 0),
            emf_select_object_record(3),
            emf_rect_record(43, 0, 0, 30, 30),
            emf_stretchdibits_dib_record(20, 15, 60, 30, 0x00c0_00ca, &emf_dib),
        ],
    );
    let wmf = minimal_wmf_with_records(
        200,
        100,
        &[
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color::default()),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(15, 25, 80, 40, 0x00ff_0062, b"WMF-WHITENESS-PAINT"),
            wmf_stretchdib_dib_record(35, 25, 80, 40, 0x00c0_00ca, &wmf_dib),
        ],
    );
    let emf_hex = bytes_to_hex(&emf);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} middle {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
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
    assert_eq!(images[0].vector_commands.len(), 2);
    assert_eq!(images[1].vector_commands.len(), 2);
    for command in [&images[0].vector_commands[1], &images[1].vector_commands[1]] {
        assert!(matches!(
            command,
            StaticImageVectorCommand::Rectangle {
                stroke_color: None,
                fill_color: Some(Color {
                    red: 0,
                    green: 0,
                    blue: 0
                }),
                ..
            }
        ));
    }
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-BLACK-MERGECOPY-JPEG",
        "TRAILING-EMF-BLACK-MERGECOPY-JPEG-DIB",
        "TRAILING-WMF-BLACK-MERGECOPY-JPEG",
        "TRAILING-WMF-BLACK-MERGECOPY-JPEG-DIB",
        "WMF-WHITENESS-PAINT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
        "OpenAction",
    ] {
        assert!(
            !text.contains(forbidden),
            "black MERGECOPY JPEG payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("middle"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "re")
            .count()
            >= 4,
        "black MERGECOPY JPEG sources should render passive rectangles"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-BLACK-MERGECOPY-JPEG",
        b"TRAILING-EMF-BLACK-MERGECOPY-JPEG-DIB",
        b"TRAILING-WMF-BLACK-MERGECOPY-JPEG",
        b"TRAILING-WMF-BLACK-MERGECOPY-JPEG-DIB",
        b"WMF-WHITENESS-PAINT",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
        b"/Subtype /Image",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "black MERGECOPY JPEG source payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn blank_destination_source_raster_ops_render_passively_without_payload_leakage() {
    let emf_srcerase_payload = b"EMF-SRCERASE-BLANK-SOURCE-PAYLOAD /JavaScript";
    let emf_patpaint_payload = b"EMF-PATPAINT-BLANK-SOURCE-PAYLOAD /EmbeddedFile";
    let wmf_srcerase_payload = b"WMF-SRCERASE-BLANK-SOURCE-PAYLOAD /Launch";
    let wmf_patpaint_payload = b"WMF-PATPAINT-BLANK-SOURCE-PAYLOAD /OpenAction";
    let srcerase_emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[emf_bitblt_record(
            10,
            20,
            30,
            15,
            0x0044_0328,
            emf_srcerase_payload,
        )],
    );
    let patpaint_emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[emf_bitblt_record(
            50,
            25,
            35,
            20,
            0x00fb_0a09,
            emf_patpaint_payload,
        )],
    );
    let srcerase_wmf = minimal_wmf_with_records(
        200,
        100,
        &[
            wmf_set_window_ext_record(200, 100),
            wmf_dibbitblt_record(15, 25, 80, 40, 0x0044_0328, wmf_srcerase_payload),
        ],
    );
    let patpaint_wmf = minimal_wmf_with_records(
        200,
        100,
        &[
            wmf_set_window_ext_record(200, 100),
            wmf_dibbitblt_record(45, 20, 70, 35, 0x00fb_0a09, wmf_patpaint_payload),
        ],
    );
    let srcerase_hex = bytes_to_hex(&srcerase_emf);
    let patpaint_hex = bytes_to_hex(&patpaint_emf);
    let srcerase_wmf_hex = bytes_to_hex(&srcerase_wmf);
    let patpaint_wmf_hex = bytes_to_hex(&patpaint_wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {srcerase_hex}}} middle {{\\pict\\emfblip {patpaint_hex}}} wmf {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {srcerase_wmf_hex}}} more {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {patpaint_wmf_hex}}} after\\par}}"
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
    assert!(text.contains("wmf"));
    assert!(text.contains("more"));
    assert!(text.contains("after"));
    assert_eq!(images.len(), 4);
    assert!(matches!(
        images[0].vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 10.0,
            top: 20.0,
            right: 40.0,
            bottom: 35.0,
            fill_color: Some(Color {
                red: 0,
                green: 0,
                blue: 0
            }),
            ..
        }
    ));
    assert!(matches!(
        images[1].vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 50.0,
            top: 25.0,
            right: 85.0,
            bottom: 45.0,
            fill_color: Some(Color {
                red: 255,
                green: 255,
                blue: 255
            }),
            ..
        }
    ));
    assert!(matches!(
        images[2].vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 15.0,
            top: 25.0,
            right: 95.0,
            bottom: 65.0,
            fill_color: Some(Color {
                red: 0,
                green: 0,
                blue: 0
            }),
            ..
        }
    ));
    assert!(matches!(
        images[3].vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 45.0,
            top: 20.0,
            right: 115.0,
            bottom: 55.0,
            fill_color: Some(Color {
                red: 255,
                green: 255,
                blue: 255
            }),
            ..
        }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "EMF-SRCERASE-BLANK-SOURCE-PAYLOAD",
        "EMF-PATPAINT-BLANK-SOURCE-PAYLOAD",
        "WMF-SRCERASE-BLANK-SOURCE-PAYLOAD",
        "WMF-PATPAINT-BLANK-SOURCE-PAYLOAD",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
        "OpenAction",
    ] {
        assert!(
            !text.contains(forbidden),
            "blank-destination raster payload/control leaked to normalized text: {forbidden}"
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
    assert!(rendered_text.contains("middle"));
    assert!(rendered_text.contains("wmf"));
    assert!(rendered_text.contains("more"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "re")
            .count()
            >= 4,
        "blank-destination raster ops should render passive PDF rectangles"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        srcerase_hex.as_bytes(),
        patpaint_hex.as_bytes(),
        srcerase_wmf_hex.as_bytes(),
        patpaint_wmf_hex.as_bytes(),
        emf_srcerase_payload.as_slice(),
        emf_patpaint_payload.as_slice(),
        wmf_srcerase_payload.as_slice(),
        wmf_patpaint_payload.as_slice(),
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
            "blank-destination raster payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn footnote_picture_placeholder_stays_note_text_without_body_escape() {
    let input =
        br"{\rtf1 Body\chftn{\footnote \chftn before {\pict\pngblip 00} after\par}\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains(&format!("Body{}", footnote_reference_marker(0))));
    assert!(text.contains("before [Image skipped: unsupported PNG] after"));
    assert!(!parsed.document.footnotes.is_empty());
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::Image(_) | Block::Placeholder(_))),
        "footnote-owned picture placeholder should not escape into body blocks: {:?}",
        parsed.document.blocks
    );

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Body1"));
    assert!(rendered_text.contains("before [Image skipped: unsupported PNG] after"));
    for forbidden in [
        b"chftn".as_slice(),
        b"footnote",
        b"pict",
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
            "forbidden footnote picture content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn note_separator_picture_placeholder_is_stripped_without_body_escape() {
    let input =
        br"{\rtf1{\ftnsep Safe separator {\pict\pngblip 00}\par}Body\chftn{\footnote \chftn Note text\par}\par}"
            .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains(&format!("Body{}", footnote_reference_marker(0))));
    assert!(text.contains("Note text"));
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::Image(_) | Block::Placeholder(_))),
        "separator picture placeholder should not escape into body blocks: {:?}",
        parsed.document.blocks
    );
    assert!(
        parsed.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("note separator picture placeholder stripped")),
        "expected separator picture placeholder strip diagnostic: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Body1"));
    assert!(rendered_text.contains("Safe separator"));
    assert!(rendered_text.contains("Note text"));
    assert!(!rendered_text.contains("[Image skipped"));
    for forbidden in [
        b"ftnsep".as_slice(),
        b"pict",
        b"pngblip",
        b"[Image skipped",
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
            "forbidden separator picture content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn note_separator_vector_picture_is_stripped_without_body_image_escape() {
    let input =
        br"{\rtf1{\aftnsep Safe endnote separator {\pict\wmetafile8\picw200\pich100\picwgoal2160\pichgoal720 01020304}\par}Body\chftn{\endnote \chftn Note text\par}\par}"
            .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Body1"));
    assert!(text.contains("Note text"));
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::Image(_))),
        "separator vector picture should not escape into body images: {:?}",
        parsed.document.blocks
    );
    assert!(
        parsed.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("note separator picture stripped")),
        "expected separator static picture strip diagnostic: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Body1"));
    assert!(rendered_text.contains("Safe endnote separator"));
    assert!(rendered_text.contains("Note text"));
    for forbidden in [
        b"aftnsep".as_slice(),
        b"pict",
        b"wmetafile",
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
            "forbidden separator vector picture content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn ignorable_destination_picture_fallback_is_stripped_without_body_escape() {
    let input = br"{\rtf1 Visible {\*\unknown{\pict\pngblip 00}} after\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible  after"));
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::Image(_) | Block::Placeholder(_))),
        "ignorable picture fallback should not escape into body blocks: {:?}",
        parsed.document.blocks
    );
    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible  after"));
    for forbidden in [
        b"unknown".as_slice(),
        b"pict",
        b"pngblip",
        b"[Image skipped",
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
            "ignorable picture fallback leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn hidden_office_shape_picture_is_stripped_without_image_or_payload_leakage() {
    let png_hex = bytes_to_hex(&minimal_rgba_png(&[[255, 0, 0, 255]]));
    let input = rtf(&[
        r"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fHidden}{\sv 1}}{\sp{\sn pFragments}{\sv hidden-shape-picture-payload}}}{\*\shppict{\pict\pngblip\picwgoal720\pichgoal720 ",
        &png_hex,
        r"}}}After\par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert!(
        !parsed
            .document
            .blocks
            .iter()
            .any(|block| matches!(block, Block::Shape(_) | Block::Image(_))),
        "hidden shape picture should not enter the normalized visible model"
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("hidden shape picture stripped before safe model normalization")
    }));
    for forbidden in [
        "shapeType",
        "fHidden",
        "pFragments",
        "hidden-shape-picture-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "hidden shape picture metadata leaked to normalized text: {forbidden}"
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
        !content
            .operations
            .iter()
            .any(|operation| operation.operator == "Do"),
        "hidden shape picture should not render a passive image XObject"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fHidden",
        b"pFragments",
        b"hidden-shape-picture-payload",
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
            "hidden shape picture metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn shape_picture_wrap_side_metadata_is_consumed_by_passive_line_exclusion() {
    let image_hex = bytes_to_hex(&minimal_jpeg_with_dimensions(1, 1));
    for (control_value, expected_side) in [
        (1, StaticImageWrapSide::Left),
        (2, StaticImageWrapSide::Right),
        (3, StaticImageWrapSide::Largest),
    ] {
        let control_value = control_value.to_string();
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
            "shpwrk",
            control_value.as_str(),
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
        assert_eq!(placement.wrap_side, expected_side);

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
            "supported wrap side should be consumed without approximation warning: {:?}",
            output.diagnostics
        );
        for forbidden in [
            b"shpwrk".as_slice(),
            b"shpwr",
            b"shppict",
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
                "forbidden wrap-side content leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn background_picture_is_stripped_without_body_flow_or_payload_leakage() {
    let input = br#"{\rtf1{\background{\pict\pngblip 00}}Visible body\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible body"));
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::Image(_) | Block::Placeholder(_))),
        "background picture should not escape into body blocks: {:?}",
        parsed.document.blocks
    );
    assert!(
        parsed.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("background picture placeholder stripped")),
        "expected background picture strip diagnostic: {:?}",
        parsed.diagnostics
    );
    for forbidden in ["pict", "pngblip", "[Image skipped"] {
        assert!(
            !text.contains(forbidden),
            "forbidden background picture content leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible body"));
    for forbidden in [
        b"background".as_slice(),
        b"pict",
        b"pngblip",
        b"[Image skipped",
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
            "forbidden background picture content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn background_vector_picture_is_stripped_without_body_image_escape() {
    let input =
        br#"{\rtf1{\background{\pict\wmetafile8\picw200\pich100\picwgoal2160\pichgoal720 01020304}}Visible body\par}"#
            .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible body"));
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::Image(_))),
        "background vector picture should not escape into body images: {:?}",
        parsed.document.blocks
    );
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("background picture stripped")),
        "expected background static image strip diagnostic: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible body"));
    for forbidden in [
        b"background".as_slice(),
        b"pict",
        b"wmetafile",
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
            "forbidden background vector picture content leaked to PDF: {:?}",
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
