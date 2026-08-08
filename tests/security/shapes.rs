use super::*;

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
fn metadata_static_shapes_do_not_emit_body_shapes_or_payloads() {
    let input = br#"{\rtf1{\info{\title Hidden title {\shp{\*\shpinst\shpleft720\shptop720\shpright2160\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn pFragments}{\sv hidden-metadata-shape-payload}}}}}}Visible body\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert_eq!(text, "Visible body");
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::Shape(_))),
        "metadata static shape should not escape into body shapes: {:?}",
        parsed.document.blocks
    );
    for forbidden in [
        "Hidden title",
        "shapeType",
        "pFragments",
        "hidden-metadata-shape-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "metadata static shape leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible body"));
    for forbidden in [
        b"Hidden title".as_slice(),
        b"shapeType",
        b"pFragments",
        b"hidden-metadata-shape-payload",
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
            "metadata static shape leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn solid_srccopy_and_notsrccopy_sources_reduce_to_rectangles_without_payload_leakage() {
    let mut emf_src_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[12, 34, 56], [12, 34, 56]]);
    emf_src_dib.extend_from_slice(b"TRAILING-EMF-SOLID-SRCCOPY-DIB /JavaScript");
    let mut emf_not_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[12, 34, 56], [12, 34, 56]]);
    emf_not_dib.extend_from_slice(b"TRAILING-EMF-SOLID-NOTSRCCOPY-DIB /OpenAction");
    let mut wmf_src_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[12, 34, 56], [12, 34, 56]]);
    wmf_src_dib.extend_from_slice(b"TRAILING-WMF-SOLID-SRCCOPY-DIB /EmbeddedFile");
    let mut wmf_not_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[12, 34, 56], [12, 34, 56]]);
    wmf_not_dib.extend_from_slice(b"TRAILING-WMF-SOLID-NOTSRCCOPY-DIB /RichMedia");
    let emf = minimal_emf_with_records(
        180,
        120,
        2858,
        1905,
        &[
            emf_stretchdibits_dib_record(20, 15, 60, 30, 0x00cc_0020, &emf_src_dib),
            emf_stretchdibits_dib_record(90, 15, 60, 30, 0x0033_0008, &emf_not_dib),
        ],
    );
    let wmf = minimal_wmf_with_records(
        220,
        120,
        &[
            wmf_set_window_ext_record(220, 120),
            wmf_stretchdib_dib_record(15, 25, 80, 40, 0x00cc_0020, &wmf_src_dib),
            wmf_stretchdib_dib_record(110, 25, 80, 40, 0x0033_0008, &wmf_not_dib),
        ],
    );
    let emf_hex = bytes_to_hex(&emf);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} middle {{\\pict\\wmetafile8\\picw220\\pich120\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
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
    let expected = [
        Color {
            red: 12,
            green: 34,
            blue: 56,
        },
        Color {
            red: 243,
            green: 221,
            blue: 199,
        },
    ];
    for image in &images {
        for (command, fill_color) in image.vector_commands.iter().zip(expected) {
            assert!(matches!(
                command,
                StaticImageVectorCommand::Rectangle {
                    fill_color: Some(actual_fill_color),
                    ..
                } if *actual_fill_color == fill_color
            ));
        }
    }
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-SOLID-SRCCOPY-DIB",
        "TRAILING-EMF-SOLID-NOTSRCCOPY-DIB",
        "TRAILING-WMF-SOLID-SRCCOPY-DIB",
        "TRAILING-WMF-SOLID-NOTSRCCOPY-DIB",
        "JavaScript",
        "OpenAction",
        "EmbeddedFile",
        "RichMedia",
    ] {
        assert!(
            !text.contains(forbidden),
            "solid SRCCOPY/NOTSRCCOPY payload/control leaked to normalized text: {forbidden}"
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
        "solid SRCCOPY/NOTSRCCOPY reductions should render passive PDF rectangles"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-SOLID-SRCCOPY-DIB",
        b"TRAILING-EMF-SOLID-NOTSRCCOPY-DIB",
        b"TRAILING-WMF-SOLID-SRCCOPY-DIB",
        b"TRAILING-WMF-SOLID-NOTSRCCOPY-DIB",
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
            "solid SRCCOPY/NOTSRCCOPY source payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn strict_spec_same_bounds_srcinvert_white_source_reduces_to_passive_solid_rectangles_without_payload_leakage()
 {
    let mut emf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[255, 255, 255], [255, 255, 255]]);
    emf_dib.extend_from_slice(b"TRAILING-EMF-SAME-BOUNDS-SRCINVERT-WHITE-DIB /JavaScript");
    let mut wmf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[255, 255, 255], [255, 255, 255]]);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-SAME-BOUNDS-SRCINVERT-WHITE-DIB /EmbeddedFile");
    let fill_color = Color {
        red: 100,
        green: 30,
        blue: 200,
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
            emf_stretchdibits_dib_record(20, 15, 60, 30, 0x0066_0046, &emf_dib),
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
            wmf_stretchdib_dib_record(15, 25, 80, 40, 0x0066_0046, &wmf_dib),
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
                    red: 155,
                    green: 225,
                    blue: 55
                }),
                ..
            }
        ));
    }
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-SAME-BOUNDS-SRCINVERT-WHITE-DIB",
        "TRAILING-WMF-SAME-BOUNDS-SRCINVERT-WHITE-DIB",
        "EMF-PATCOPY-FILL",
        "WMF-PATCOPY-FILL",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "same-bounds white SRCINVERT payload/control leaked to normalized text: {forbidden}"
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
        "same-bounds white SRCINVERT reductions should render passive PDF rectangles"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-SAME-BOUNDS-SRCINVERT-WHITE-DIB",
        b"TRAILING-WMF-SAME-BOUNDS-SRCINVERT-WHITE-DIB",
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
            "same-bounds white SRCINVERT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn strict_spec_same_bounds_srcand_solid_source_reduces_to_passive_solid_rectangles_without_payload_leakage()
 {
    let mut emf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]);
    emf_dib.extend_from_slice(b"TRAILING-EMF-SAME-BOUNDS-SRCAND-SOLID-DIB /JavaScript");
    let mut wmf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-SAME-BOUNDS-SRCAND-SOLID-DIB /EmbeddedFile");
    let fill_color = Color {
        red: 100,
        green: 30,
        blue: 200,
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
            emf_stretchdibits_dib_record(20, 15, 60, 30, 0x0088_00c6, &emf_dib),
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
            wmf_stretchdib_dib_record(15, 25, 80, 40, 0x0088_00c6, &wmf_dib),
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
                    red: 32,
                    green: 10,
                    blue: 136
                }),
                ..
            }
        ));
    }
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-SAME-BOUNDS-SRCAND-SOLID-DIB",
        "TRAILING-WMF-SAME-BOUNDS-SRCAND-SOLID-DIB",
        "EMF-PATCOPY-FILL",
        "WMF-PATCOPY-FILL",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "same-bounds solid SRCAND payload/control leaked to normalized text: {forbidden}"
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
        "same-bounds solid SRCAND reductions should render passive PDF rectangles"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-SAME-BOUNDS-SRCAND-SOLID-DIB",
        b"TRAILING-WMF-SAME-BOUNDS-SRCAND-SOLID-DIB",
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
            "same-bounds solid SRCAND payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn strict_spec_same_bounds_srcerase_solid_source_reduces_to_passive_solid_rectangles_without_payload_leakage()
 {
    let mut emf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]);
    emf_dib.extend_from_slice(b"TRAILING-EMF-SAME-BOUNDS-SRCERASE-SOLID-DIB /JavaScript");
    let mut wmf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-SAME-BOUNDS-SRCERASE-SOLID-DIB /EmbeddedFile");
    let fill_color = Color {
        red: 100,
        green: 30,
        blue: 200,
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
            emf_stretchdibits_dib_record(20, 15, 60, 30, 0x0044_0328, &emf_dib),
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
            wmf_stretchdib_dib_record(15, 25, 80, 40, 0x0044_0328, &wmf_dib),
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
                    red: 138,
                    green: 160,
                    blue: 34
                }),
                ..
            }
        ));
    }
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-SAME-BOUNDS-SRCERASE-SOLID-DIB",
        "TRAILING-WMF-SAME-BOUNDS-SRCERASE-SOLID-DIB",
        "EMF-PATCOPY-FILL",
        "WMF-PATCOPY-FILL",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "same-bounds solid SRCERASE payload/control leaked to normalized text: {forbidden}"
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
        "same-bounds solid SRCERASE reductions should render passive PDF rectangles"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-SAME-BOUNDS-SRCERASE-SOLID-DIB",
        b"TRAILING-WMF-SAME-BOUNDS-SRCERASE-SOLID-DIB",
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
            "same-bounds solid SRCERASE payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn strict_spec_same_bounds_notsrcerase_solid_source_reduces_to_passive_solid_rectangles_without_payload_leakage()
 {
    let mut emf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]);
    emf_dib.extend_from_slice(b"TRAILING-EMF-SAME-BOUNDS-NOTSRCERASE-SOLID-DIB /JavaScript");
    let mut wmf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-SAME-BOUNDS-NOTSRCERASE-SOLID-DIB /EmbeddedFile");
    let fill_color = Color {
        red: 100,
        green: 30,
        blue: 200,
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
            emf_stretchdibits_dib_record(20, 15, 60, 30, 0x0011_00a6, &emf_dib),
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
            wmf_stretchdib_dib_record(15, 25, 80, 40, 0x0011_00a6, &wmf_dib),
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
                    red: 17,
                    green: 65,
                    blue: 21
                }),
                ..
            }
        ));
    }
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-SAME-BOUNDS-NOTSRCERASE-SOLID-DIB",
        "TRAILING-WMF-SAME-BOUNDS-NOTSRCERASE-SOLID-DIB",
        "EMF-PATCOPY-FILL",
        "WMF-PATCOPY-FILL",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "same-bounds solid NOTSRCERASE payload/control leaked to normalized text: {forbidden}"
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
        "same-bounds solid NOTSRCERASE reductions should render passive PDF rectangles"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-SAME-BOUNDS-NOTSRCERASE-SOLID-DIB",
        b"TRAILING-WMF-SAME-BOUNDS-NOTSRCERASE-SOLID-DIB",
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
            "same-bounds solid NOTSRCERASE payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn strict_spec_same_bounds_patpaint_solid_source_reduces_to_passive_solid_rectangles_without_payload_leakage()
 {
    let mut emf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]);
    emf_dib.extend_from_slice(b"TRAILING-EMF-SAME-BOUNDS-PATPAINT-SOLID-DIB /JavaScript");
    let mut wmf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-SAME-BOUNDS-PATPAINT-SOLID-DIB /EmbeddedFile");
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
            emf_create_brush_record(3, 0, pattern_color, 0),
            emf_create_brush_record(4, 0, fill_color, 0),
            emf_select_object_record(4),
            emf_bitblt_record(20, 15, 60, 30, 0x00f0_0021, b"EMF-PATCOPY-FILL"),
            emf_select_object_record(3),
            emf_stretchdibits_dib_record(20, 15, 60, 30, 0x00fb_0a09, &emf_dib),
        ],
    );
    let wmf = minimal_wmf_with_records(
        200,
        100,
        &[
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(pattern_color),
            wmf_create_brush_record(fill_color),
            wmf_select_object_record(1),
            wmf_dibbitblt_record(15, 25, 80, 40, 0x00f0_0021, b"WMF-PATCOPY-FILL"),
            wmf_select_object_record(0),
            wmf_stretchdib_dib_record(15, 25, 80, 40, 0x00fb_0a09, &wmf_dib),
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
                    red: 117,
                    green: 95,
                    blue: 221
                }),
                ..
            }
        ));
    }
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-SAME-BOUNDS-PATPAINT-SOLID-DIB",
        "TRAILING-WMF-SAME-BOUNDS-PATPAINT-SOLID-DIB",
        "EMF-PATCOPY-FILL",
        "WMF-PATCOPY-FILL",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "same-bounds solid PATPAINT payload/control leaked to normalized text: {forbidden}"
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
        "same-bounds solid PATPAINT reductions should render passive PDF rectangles"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-SAME-BOUNDS-PATPAINT-SOLID-DIB",
        b"TRAILING-WMF-SAME-BOUNDS-PATPAINT-SOLID-DIB",
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
            "same-bounds solid PATPAINT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn strict_spec_same_bounds_srcinvert_solid_source_reduces_to_passive_solid_rectangles_without_payload_leakage()
 {
    let mut emf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]);
    emf_dib.extend_from_slice(b"TRAILING-EMF-SAME-BOUNDS-SRCINVERT-SOLID-DIB /JavaScript");
    let mut wmf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-SAME-BOUNDS-SRCINVERT-SOLID-DIB /EmbeddedFile");
    let fill_color = Color {
        red: 100,
        green: 30,
        blue: 200,
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
            emf_stretchdibits_dib_record(20, 15, 60, 30, 0x0066_0046, &emf_dib),
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
            wmf_stretchdib_dib_record(15, 25, 80, 40, 0x0066_0046, &wmf_dib),
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
                    red: 206,
                    green: 180,
                    blue: 98
                }),
                ..
            }
        ));
    }
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-SAME-BOUNDS-SRCINVERT-SOLID-DIB",
        "TRAILING-WMF-SAME-BOUNDS-SRCINVERT-SOLID-DIB",
        "EMF-PATCOPY-FILL",
        "WMF-PATCOPY-FILL",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "same-bounds solid SRCINVERT payload/control leaked to normalized text: {forbidden}"
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
        "same-bounds solid SRCINVERT reductions should render passive PDF rectangles"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-SAME-BOUNDS-SRCINVERT-SOLID-DIB",
        b"TRAILING-WMF-SAME-BOUNDS-SRCINVERT-SOLID-DIB",
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
            "same-bounds solid SRCINVERT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn mergecopy_solid_source_reduces_to_passive_solid_rectangles_without_payload_leakage() {
    let mut emf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 85, 204], [170, 85, 204]]);
    emf_dib.extend_from_slice(b"TRAILING-EMF-SOLID-MERGECOPY-DIB /JavaScript");
    let mut wmf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 85, 204], [170, 85, 204]]);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-SOLID-MERGECOPY-DIB /EmbeddedFile");
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
            wmf_stretchdib_dib_record(15, 25, 80, 40, 0x00c0_00ca, &wmf_dib),
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
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                fill_color: Some(Color {
                    red: 160,
                    green: 5,
                    blue: 136
                }),
                ..
            }
        ));
    }
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-SOLID-MERGECOPY-DIB",
        "TRAILING-WMF-SOLID-MERGECOPY-DIB",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "solid MERGECOPY payload/control leaked to normalized text: {forbidden}"
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
        "solid MERGECOPY reductions should render passive PDF rectangles"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-SOLID-MERGECOPY-DIB",
        b"TRAILING-WMF-SOLID-MERGECOPY-DIB",
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
            "solid MERGECOPY source payload leaked to PDF: {:?}",
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
        "nowrap Alias Alpha Beta",
        "\\",
        "par",
        "\\",
        "nowrap0 Alias wrapped",
        "\\",
        "par",
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
    assert!(text.contains("Alias Alpha Beta"));
    assert!(text.contains("Alias wrapped"));
    assert!(text.contains("Cell Alpha Beta"));
    for forbidden in ["nowrap", "nowwrap", "clNoWrap"] {
        assert!(
            !text.contains(forbidden),
            "forbidden no-wrap control leaked to text: {forbidden}"
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
    assert!(!paragraphs[0].style.no_wrap);
    assert!(!paragraphs[1].style.no_wrap);
    assert!(paragraphs[2].style.no_wrap);
    assert!(paragraphs[3].style.no_wrap);
    assert_eq!(paragraphs[2].style.no_wrap_emergency_right_inset_twips, 144);
    assert_eq!(paragraphs[3].style.no_wrap_emergency_right_inset_twips, 144);
    let table = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("bounded no-wrap table");
    assert_eq!(
        table.rows[0].cells[1].paragraphs[0].style.left_indent_twips,
        108
    );
    assert_eq!(
        table.rows[0].cells[1].paragraphs[0]
            .style
            .first_line_indent_twips,
        -108
    );
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "no-wrap controls should not be unsupported: {:?}",
        parsed.diagnostics
    );

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
        b"nowrap".as_slice(),
        b"nowwrap",
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
fn ignorable_destination_static_shape_is_stripped_without_body_escape() {
    let input = br#"{\rtf1 Visible {\*\unknown{\shp{\*\shpinst\shpleft720\shptop720\shpright2160\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn pFragments}{\sv hidden-ignorable-shape-payload}}}}} after\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible  after"));
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::Shape(_))),
        "ignorable static shape should not escape into body shapes: {:?}",
        parsed.document.blocks
    );
    for forbidden in [
        "unknown",
        "shapeType",
        "pFragments",
        "hidden-ignorable-shape-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "ignorable static shape leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible  after"));
    for forbidden in [
        b"unknown".as_slice(),
        b"shapeType",
        b"pFragments",
        b"hidden-ignorable-shape-payload",
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
            "ignorable static shape leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn bounded_shape_text_renders_inside_passive_shape_without_body_flow_or_payload_leakage() {
    let input = br#"{\rtf1{\shp{\shpinst\shpleft720\shptop720\shpright4320\shpbottom1800{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillColor}{\sv 13434879}}{\sp{\sn dxTextLeft}{\sv 127000}}{\sp{\sn dxTextRight}{\sv 63500}}{\sp{\sn dyTextTop}{\sv 190500}}{\sp{\sn dyTextBottom}{\sv 63500}}{\sp{\sn anchorText}{\sv 4}}{\sp{\sn pFragments}{\sv hostile-shape-text-payload}}}{\shptxt Box text {\field{\*\fldinst HYPERLINK "https://example.com/shape-text"}{\fldrslt safe link}}\par}}After\par}"#.to_vec();
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
    assert_eq!(shape.text_margin_left_twips, 200);
    assert_eq!(shape.text_margin_right_twips, 100);
    assert_eq!(shape.text_margin_top_twips, 300);
    assert_eq!(shape.text_margin_bottom_twips, 100);
    assert_eq!(
        shape.text_vertical_anchor,
        StaticShapeTextVerticalAnchor::Middle
    );
    assert!(shape.text_horizontal_anchor_centered);
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
        "dxTextLeft",
        "dxTextRight",
        "dyTextTop",
        "dyTextBottom",
        "anchorText",
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
        b"dxTextLeft",
        b"dxTextRight",
        b"dyTextTop",
        b"dyTextBottom",
        b"anchorText",
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
fn office_shape_shadow_renders_as_passive_bounded_geometry_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillColor}{\sv 13434879}}{\sp{\sn fShadow}{\sv 1}}{\sp{\sn shadowColor}{\sv 8421504}}{\sp{\sn pFragments}{\sv hidden-shadow-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with shadow");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert!(shape.shadow_enabled);
    assert_eq!(
        shape.shadow_color,
        Color {
            red: 128,
            green: 128,
            blue: 128,
        }
    );
    for forbidden in ["fShadow", "shadowColor", "hidden-shadow-payload"] {
        assert!(
            !text.contains(forbidden),
            "shape shadow property leaked to normalized text: {forbidden}"
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
    let fill_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "f")
        .count();

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        fill_count >= 2,
        "shadowed filled shape should emit passive shadow fill plus shape fill"
    );
    for forbidden in [
        b"fShadow".as_slice(),
        b"shadowColor",
        b"hidden-shadow-payload",
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
            "shape shadow property leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_shadow_offsets_render_as_bounded_passive_geometry_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillColor}{\sv 13434879}}{\sp{\sn fShadow}{\sv 1}}{\sp{\sn shadowColor}{\sv 8421504}}{\sp{\sn shadowOffsetX}{\sv 127000}}{\sp{\sn shadowOffsetY}{\sv -63500}}{\sp{\sn pFragments}{\sv hidden-shadow-offset-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with explicit shadow offsets");

    assert!(shape.shadow_enabled);
    assert_eq!(shape.shadow_offset_x_twips, 200);
    assert_eq!(shape.shadow_offset_y_twips, -100);
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "shadowOffsetX",
        "shadowOffsetY",
        "pFragments",
        "hidden-shadow-offset-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "shape shadow offset property leaked to normalized text: {forbidden}"
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
    let fill_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "f")
        .count();

    assert!(
        fill_count >= 2,
        "shadowed filled shape should emit passive shadow fill plus shape fill"
    );
    for forbidden in [
        b"shadowOffsetX".as_slice(),
        b"shadowOffsetY",
        b"pFragments",
        b"hidden-shadow-offset-payload",
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
            "shape shadow offset property leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_full_shadow_opacity_is_consumed_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillColor}{\sv 13434879}}{\sp{\sn fShadow}{\sv 1}}{\sp{\sn shadowColor}{\sv 8421504}}{\sp{\sn shadowOpacity}{\sv 65536}}}}After\par}"#.to_vec();
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
        .expect("passive shape with full shadow opacity");

    assert!(shape.shadow_enabled);
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unsupported/active drawing properties")),
        "full shadowOpacity should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in ["shadowOpacity", "shapeType", "shadowColor"] {
        assert!(
            !text.contains(forbidden),
            "full shadow opacity metadata leaked to normalized text: {forbidden}"
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
        b"shadowOpacity".as_slice(),
        b"shapeType",
        b"shadowColor",
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
            "full shadow opacity metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn partial_office_shape_shadow_opacity_renders_passively_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillColor}{\sv 13434879}}{\sp{\sn fShadow}{\sv 1}}{\sp{\sn shadowColor}{\sv 8421504}}{\sp{\sn shadowOpacity}{\sv 32768}}{\sp{\sn pFragments}{\sv hidden-partial-shadow-opacity-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with partial shadow opacity");

    assert!(shape.shadow_enabled);
    assert_eq!(shape.shadow_opacity_percent, 50);
    assert!(
        parsed.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("stripping unsupported/active drawing properties")),
        "unrelated active Office drawing properties should still be stripped: {:?}",
        parsed.diagnostics
    );
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "shadowOpacity",
        "32768",
        "pFragments",
        "hidden-partial-shadow-opacity-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "partial shadow opacity metadata leaked to normalized text: {forbidden}"
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
    let layout = LayoutEngine::layout(&parsed.document);
    assert!(
        layout
            .pages
            .iter()
            .flat_map(|page| page.items.iter())
            .any(|item| matches!(
                item,
                LayoutItem::Opacity {
                    fill_percent: 50,
                    stroke_percent: 50,
                    ..
                }
            )),
        "partial shadow opacity should wrap generated shadow geometry"
    );
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "gs"),
        "partial shadow opacity should emit passive PDF graphics state"
    );
    for forbidden in [
        b"shadowOpacity".as_slice(),
        b"32768",
        b"pFragments",
        b"hidden-partial-shadow-opacity-payload",
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
            "partial shadow opacity metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn invalid_office_shape_shadow_opacity_is_stripped_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillColor}{\sv 13434879}}{\sp{\sn fShadow}{\sv 1}}{\sp{\sn shadowColor}{\sv 8421504}}{\sp{\sn shadowOpacity}{\sv hostile-shadow-opacity-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with invalid shadow opacity");

    assert!(shape.shadow_enabled);
    assert_eq!(shape.shadow_opacity_percent, 100);
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("stripping unsupported/active drawing properties")
    }));
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "shadowOpacity",
        "hostile-shadow-opacity-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "invalid shadow opacity metadata leaked to normalized text: {forbidden}"
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
    let layout = LayoutEngine::layout(&parsed.document);
    assert!(
        !layout
            .pages
            .iter()
            .flat_map(|page| page.items.iter())
            .any(|item| matches!(item, LayoutItem::Opacity { .. })),
        "invalid shadow opacity should not create an opacity wrapper"
    );
    for forbidden in [
        b"shadowOpacity".as_slice(),
        b"hostile-shadow-opacity-payload",
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
            "invalid shadow opacity metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn invalid_office_shape_fill_color_is_stripped_without_becoming_visible_black() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillColor}{\sv -1}}{\sp{\sn pFragments}{\sv hidden-invalid-fill-color-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with invalid fill color");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert_eq!(
        shape.fill_color,
        Some(Color {
            red: 255,
            green: 255,
            blue: 255
        }),
        "invalid Office fillColor should recover to Word's safe white default"
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("stripping unsupported/active drawing properties")
    }));
    for forbidden in [
        "shapeType",
        "fillColor",
        "pFragments",
        "hidden-invalid-fill-color-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "invalid fill color metadata leaked to normalized text: {forbidden}"
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
    let fill_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "f")
        .count();

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert_eq!(
        fill_count, 1,
        "invalid Office fillColor should retain only Word's safe white default fill"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"pFragments",
        b"hidden-invalid-fill-color-payload",
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
            "invalid fill color metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_fill_fore_color_is_inert_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillForeColor}{\sv 65535}}{\sp{\sn pFragments}{\sv hidden-fill-fore-color-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with fillForeColor");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 255,
            green: 255,
            blue: 255,
        })
    );
    for forbidden in [
        "shapeType",
        "fillForeColor",
        "pFragments",
        "hidden-fill-fore-color-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "fillForeColor metadata leaked to normalized text: {forbidden}"
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
            .any(|operation| operation.operator == "f" || operation.operator == "B"),
        "Word's default white fill should remain visible"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillForeColor",
        b"pFragments",
        b"hidden-fill-fore-color-payload",
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
            "fillForeColor metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn invalid_office_shape_fill_fore_color_is_stripped_without_becoming_visible_black() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillForeColor}{\sv -1}}{\sp{\sn pFragments}{\sv hidden-invalid-fill-fore-color-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with invalid fillForeColor");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert_eq!(
        shape.fill_color,
        Some(Color {
            red: 255,
            green: 255,
            blue: 255
        }),
        "invalid Office fillForeColor should recover to Word's safe white default"
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("stripping unsupported/active drawing properties")
    }));
    for forbidden in [
        "shapeType",
        "fillForeColor",
        "pFragments",
        "hidden-invalid-fill-fore-color-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "invalid fillForeColor metadata leaked to normalized text: {forbidden}"
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
    let fill_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "f")
        .count();

    assert_eq!(
        fill_count, 1,
        "invalid Office fillForeColor should retain only Word's safe white default fill"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillForeColor",
        b"pFragments",
        b"hidden-invalid-fill-fore-color-payload",
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
            "invalid fillForeColor metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_fill_back_color_is_secondary_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillBackColor}{\sv 16711680}}{\sp{\sn pFragments}{\sv hidden-fill-back-color-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with fillBackColor");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 255,
            green: 255,
            blue: 255,
        })
    );
    for forbidden in [
        "shapeType",
        "fillBackColor",
        "pFragments",
        "hidden-fill-back-color-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "fillBackColor metadata leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "f" || operation.operator == "B"),
        "Word's default white fill should remain visible"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillBackColor",
        b"pFragments",
        b"hidden-fill-back-color-payload",
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
            "fillBackColor metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_fill_color_aliases_leave_word_default_fill() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillBackColor}{\sv 16711680}}{\sp{\sn fillForeColor}{\sv 65535}}{\sp{\sn pFragments}{\sv hidden-fill-precedence-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with fill color precedence");

    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 255,
            green: 255,
            blue: 255,
        })
    );
    for forbidden in [
        "fillBackColor",
        "fillForeColor",
        "hidden-fill-precedence-payload",
    ] {
        assert!(
            !text.contains(forbidden),
            "fill color precedence metadata leaked to normalized text: {forbidden}"
        );
    }
}

#[test]
fn strict_spec_retains_bounded_office_shape_color_aliases() {
    let input = br#"{\rtf1 {\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillType}{\sv 4}}{\sp{\sn fillBackColor}{\sv 16711680}}{\sp{\sn fillForeColor}{\sv 65535}}{\sp{\sn lineBackColor}{\sv 16711680}}{\sp{\sn lineForeColor}{\sv 255}}}}}"#.to_vec();
    let parsed = parse_rtf_bytes_strict(&input);
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("strict-spec shape with bounded color aliases");

    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 255,
            green: 255,
            blue: 0,
        })
    );
    assert_eq!(
        shape.fill_gradient_color,
        Some(open_rtf_converter::model::Color {
            red: 0,
            green: 0,
            blue: 255,
        })
    );
    assert_eq!(
        shape.stroke_color,
        open_rtf_converter::model::Color {
            red: 255,
            green: 0,
            blue: 0,
        }
    );
}

#[test]
fn invalid_office_shape_fill_back_color_is_stripped_without_becoming_visible_black() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillBackColor}{\sv -1}}{\sp{\sn pFragments}{\sv hidden-invalid-fill-back-color-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with invalid fillBackColor");

    assert_eq!(
        shape.fill_color,
        Some(Color {
            red: 255,
            green: 255,
            blue: 255
        }),
        "invalid Office fillBackColor should recover to Word's safe white default"
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("stripping unsupported/active drawing properties")
    }));
    for forbidden in [
        "fillBackColor",
        "pFragments",
        "hidden-invalid-fill-back-color-payload",
    ] {
        assert!(
            !text.contains(forbidden),
            "invalid fillBackColor metadata leaked to normalized text: {forbidden}"
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
    let fill_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "f")
        .count();

    assert_eq!(
        fill_count, 1,
        "invalid Office fillBackColor should retain only Word's safe white default fill"
    );
    for forbidden in [
        b"fillBackColor".as_slice(),
        b"pFragments",
        b"hidden-invalid-fill-back-color-payload",
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
            "invalid fillBackColor metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_solid_fill_type_is_consumed_without_unsupported_property_warning() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillType}{\sv solid}}{\sp{\sn fillColor}{\sv 65280}}}}After\par}"#.to_vec();
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
        .expect("passive shape with solid fillType");

    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 0,
            green: 255,
            blue: 0,
        })
    );
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unsupported/active drawing properties")),
        "solid fillType should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );
    for forbidden in ["fillType", "solid"] {
        assert!(
            !text.contains(forbidden),
            "solid fillType metadata leaked to normalized text: {forbidden}"
        );
    }
}

#[test]
fn office_shape_pattern_fill_type_renders_passively_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillType}{\sv 1}}{\sp{\sn fillColor}{\sv 65280}}{\sp{\sn pFragments}{\sv hidden-pattern-fill-type-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with pattern fillType");

    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert_eq!(shape.fill_pattern, ShadingPattern::Cross);
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("stripping unsupported/active drawing properties")
    }));
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "fillType",
        "msoFillPattern",
        "pFragments",
        "hidden-pattern-fill-type-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "pattern fillType metadata leaked to normalized text: {forbidden}"
        );
    }

    let layout = LayoutEngine::layout(&parsed.document);
    assert!(
        layout
            .pages
            .iter()
            .flat_map(|page| page.items.iter())
            .filter(|item| matches!(item, LayoutItem::Line { .. }))
            .count()
            > 4,
        "rectangular pattern fill should render passive hatch lines in addition to borders"
    );

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

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "S")
            .count()
            > 4,
        "rectangular pattern fill should emit passive stroked hatch lines"
    );
    for forbidden in [
        b"fillType".as_slice(),
        b"msoFillPattern",
        b"pFragments",
        b"hidden-pattern-fill-type-payload",
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
            "pattern fillType metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn non_rectangular_office_shape_pattern_fills_are_clipped_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 4}}{\sp{\sn fillType}{\sv 1}}{\sp{\sn fillColor}{\sv 16711680}}{\sp{\sn pFragments}{\sv hidden-pattern-diamond-payload}}}}{\shp{\*\shpinst\shpleft720\shptop1800\shpright2880\shpbottom2520{\sp{\sn shapeType}{\sv 9}}{\sp{\sn fillType}{\sv 1}}{\sp{\sn fillColor}{\sv 65280}}{\sp{\sn pFragments}{\sv hidden-pattern-ellipse-payload}}}}After\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let shapes: Vec<_> = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .collect();

    assert_eq!(shapes.len(), 2);
    assert_eq!(shapes[0].kind, StaticShapeKind::Polygon);
    assert_eq!(shapes[0].fill_pattern, ShadingPattern::Cross);
    assert_eq!(shapes[1].kind, StaticShapeKind::Ellipse);
    assert_eq!(shapes[1].fill_pattern, ShadingPattern::Cross);
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("stripping unsupported/active drawing properties")
    }));
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "fillType",
        "msoFillPattern",
        "pFragments",
        "hidden-pattern-diamond-payload",
        "hidden-pattern-ellipse-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "non-rectangular pattern fill metadata leaked to normalized text: {forbidden}"
        );
    }

    let layout = LayoutEngine::layout(&parsed.document);
    assert!(
        layout
            .pages
            .iter()
            .flat_map(|page| page.items.iter())
            .any(|item| matches!(
                item,
                LayoutItem::Polygon {
                    fill_pattern: ShadingPattern::Cross,
                    ..
                }
            )),
        "pattern-filled diamond should preserve hatch state through layout"
    );
    assert!(
        layout
            .pages
            .iter()
            .flat_map(|page| page.items.iter())
            .any(|item| matches!(
                item,
                LayoutItem::Ellipse {
                    fill_pattern: ShadingPattern::Cross,
                    ..
                }
            )),
        "pattern-filled ellipse should preserve hatch state through layout"
    );

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

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "W" || operation.operator == "W*")
            .count()
            >= 2,
        "non-rectangular pattern fills should clip hatch lines to their passive shape paths"
    );
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "S")
            .count()
            > 8,
        "non-rectangular pattern fills should emit passive stroked hatch lines"
    );
    for forbidden in [
        b"fillType".as_slice(),
        b"msoFillPattern",
        b"pFragments",
        b"hidden-pattern-diamond-payload",
        b"hidden-pattern-ellipse-payload",
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
            "non-rectangular pattern fill metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_gradient_fill_type_renders_passively_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillType}{\sv 4}}{\sp{\sn fillColor}{\sv 65280}}{\sp{\sn pFragments}{\sv hidden-gradient-fill-type-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with gradient fill");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.fill_pattern, ShadingPattern::VerticalGradient);
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("stripping unsupported/active drawing properties")
    }));
    for forbidden in [
        "fillType",
        "msofillShade",
        "hidden-gradient-fill-type-payload",
    ] {
        assert!(
            !text.contains(forbidden),
            "gradient fillType metadata leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    for forbidden in [
        b"fillType".as_slice(),
        b"msofillShade",
        b"hidden-gradient-fill-type-payload",
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
            "gradient fillType metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_gradient_fill_uses_back_color_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillType}{\sv 4}}{\sp{\sn fillColor}{\sv 255}}{\sp{\sn fillBackColor}{\sv 16711680}}}}After\par}"#.to_vec();
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
        .expect("passive shape with two-color gradient fill");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.fill_pattern, ShadingPattern::VerticalGradient);
    assert_eq!(
        shape.fill_color,
        Some(Color {
            red: 255,
            green: 0,
            blue: 0
        })
    );
    assert_eq!(
        shape.fill_gradient_color,
        Some(Color {
            red: 0,
            green: 0,
            blue: 255
        })
    );
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unsupported/active drawing properties")),
        "two-color gradient fill should be consumed as passive metadata: {:?}",
        parsed.diagnostics
    );
    for forbidden in ["fillType", "msofillShade", "fillBackColor", "16711680"] {
        assert!(
            !text.contains(forbidden),
            "two-color gradient fill metadata leaked to normalized text: {forbidden}"
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
        b"fillType".as_slice(),
        b"msofillShade",
        b"fillBackColor",
        b"16711680",
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
            "two-color gradient fill metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn non_rectangular_office_shape_gradient_fills_are_clipped_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 4}}{\sp{\sn fillType}{\sv 4}}{\sp{\sn fillColor}{\sv 255}}{\sp{\sn fillBackColor}{\sv 16711680}}{\sp{\sn pFragments}{\sv hidden-gradient-diamond-payload}}}}{\shp{\*\shpinst\shpleft720\shptop1800\shpright2880\shpbottom2520{\sp{\sn shapeType}{\sv 9}}{\sp{\sn fillType}{\sv 4}}{\sp{\sn fillColor}{\sv 65280}}{\sp{\sn fillBackColor}{\sv 16776960}}{\sp{\sn pFragments}{\sv hidden-gradient-ellipse-payload}}}}After\par}"#.to_vec();
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
    assert_eq!(shapes.len(), 2);
    assert_eq!(shapes[0].kind, StaticShapeKind::Polygon);
    assert_eq!(shapes[0].fill_pattern, ShadingPattern::VerticalGradient);
    assert_eq!(
        shapes[0].fill_gradient_color,
        Some(Color {
            red: 0,
            green: 0,
            blue: 255
        })
    );
    assert_eq!(shapes[1].kind, StaticShapeKind::Ellipse);
    assert_eq!(shapes[1].fill_pattern, ShadingPattern::VerticalGradient);
    assert_eq!(
        shapes[1].fill_gradient_color,
        Some(Color {
            red: 0,
            green: 255,
            blue: 255
        })
    );

    let layout = LayoutEngine::layout(&parsed.document);
    assert!(
        layout
            .pages
            .iter()
            .flat_map(|page| page.items.iter())
            .any(|item| matches!(
                item,
                LayoutItem::Polygon {
                    fill_pattern: ShadingPattern::VerticalGradient,
                    fill_gradient_color: Some(_),
                    ..
                }
            )),
        "gradient-filled diamond should preserve gradient endpoint through layout"
    );
    assert!(
        layout
            .pages
            .iter()
            .flat_map(|page| page.items.iter())
            .any(|item| matches!(
                item,
                LayoutItem::Ellipse {
                    fill_pattern: ShadingPattern::VerticalGradient,
                    fill_gradient_color: Some(_),
                    ..
                }
            )),
        "gradient-filled ellipse should preserve gradient endpoint through layout"
    );
    for forbidden in [
        "fillType",
        "msofillShade",
        "fillBackColor",
        "hidden-gradient-diamond-payload",
        "hidden-gradient-ellipse-payload",
    ] {
        assert!(
            !text.contains(forbidden),
            "non-rectangular gradient metadata leaked to normalized text: {forbidden}"
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
    let passive_fill_ops = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "f")
        .count();
    assert!(
        passive_fill_ops >= 16,
        "non-rectangular gradients should render bounded passive fill bands"
    );
    for forbidden in [
        b"fillType".as_slice(),
        b"msofillShade",
        b"fillBackColor",
        b"hidden-gradient-diamond-payload",
        b"hidden-gradient-ellipse-payload",
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
            "non-rectangular gradient metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_shade_fill_variants_are_bounded_passive_gradients_without_property_leakage() {
    for fill_type in ["5", "6", "7"] {
        let input = format!(
            r#"{{\rtf1 Before\par{{\shp{{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{{\sp{{\sn shapeType}}{{\sv 1}}}}{{\sp{{\sn fillType}}{{\sv {fill_type}}}}}{{\sp{{\sn fillColor}}{{\sv 65280}}}}}}}}After\par}}"#
        )
        .into_bytes();
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
            .expect("passive shape with shade fill variant");

        assert!(text.contains("Before"));
        assert!(text.contains("After"));
        assert_eq!(shape.fill_pattern, ShadingPattern::VerticalGradient);
        assert!(
            parsed.diagnostics.iter().all(|diagnostic| !diagnostic
                .message
                .contains("unsupported/active drawing properties")),
            "shade fill variants should be consumed as passive metadata: {:?}",
            parsed.diagnostics
        );
        for forbidden in ["fillType"] {
            assert!(
                !text.contains(forbidden),
                "shade fill variant metadata leaked to normalized text: {forbidden}"
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
            b"fillType".as_slice(),
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
                "shade fill variant metadata leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn office_shape_background_fill_type_uses_passive_solid_fill_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillType}{\sv msoFillBackground}}{\sp{\sn fillColor}{\sv 65280}}}}After\par}"#.to_vec();
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
        .expect("passive shape with background fillType");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert_eq!(shape.fill_pattern, ShadingPattern::None);
    assert_eq!(
        shape.fill_color,
        Some(Color {
            red: 0,
            green: 255,
            blue: 0
        })
    );
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unsupported/active drawing properties")),
        "background fillType should be consumed as passive metadata: {:?}",
        parsed.diagnostics
    );
    for forbidden in ["fillType", "msoFillBackground"] {
        assert!(
            !text.contains(forbidden),
            "background fillType metadata leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "f" || operation.operator == "B"),
        "background fillType should render as a bounded passive filled shape"
    );
    for forbidden in [
        b"fillType".as_slice(),
        b"msoFillBackground",
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
            "background fillType metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn hidden_office_shape_text_is_stripped_without_placeholder_or_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fHidden}{\sv 1}}{\sp{\sn pFragments}{\sv hidden-shape-text-payload}}}{\shptxt Invisible box text\par}}After\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert!(!text.contains("Invisible box text"));
    assert!(
        !parsed
            .document
            .blocks
            .iter()
            .any(|block| matches!(block, Block::Shape(_))),
        "hidden shape should not enter the normalized visible shape model"
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("hidden shape stripped before safe model normalization")
    }));
    for forbidden in [
        "shapeType",
        "fHidden",
        "pFragments",
        "hidden-shape-text-payload",
        "Invisible box text",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "hidden shape text metadata leaked to normalized text: {forbidden}"
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
    assert!(!rendered_text.contains("Invisible box text"));
    for forbidden in [
        b"shapeType".as_slice(),
        b"fHidden",
        b"pFragments",
        b"hidden-shape-text-payload",
        b"Invisible box text",
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
            "hidden shape text metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn invalid_office_shape_opacity_is_stripped_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillColor}{\sv 13434879}}{\sp{\sn fillOpacity}{\sv -1}}{\sp{\sn lineOpacity}{\sv hostile-opacity-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with invalid opacity");

    assert_eq!(shape.fill_opacity_percent, 100);
    assert_eq!(shape.stroke_opacity_percent, 100);
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
        "hostile-opacity-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "invalid opacity metadata leaked to normalized text: {forbidden}"
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
    let layout = LayoutEngine::layout(&parsed.document);
    assert!(
        !layout
            .pages
            .iter()
            .flat_map(|page| page.items.iter())
            .any(|item| matches!(item, LayoutItem::Opacity { .. })),
        "invalid Office shape opacity should not wrap passive geometry"
    );
    for forbidden in [
        b"fillOpacity".as_slice(),
        b"lineOpacity",
        b"hostile-opacity-payload",
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
            "invalid opacity metadata leaked to PDF: {:?}",
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
fn rotated_office_rectangles_lower_to_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillColor}{\sv 65535}}{\sp{\sn rotation}{\sv 2700000}}{\sp{\sn pFragments}{\sv hidden-rotation-payload}}}}After\par}"#.to_vec();
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
        .expect("rotated passive shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 4);
    assert_eq!(shape.width_twips, 2100);
    assert_eq!(shape.height_twips, 1964);
    assert_eq!(
        shape.points[0],
        open_rtf_converter::model::StaticShapePoint {
            x_twips: 474,
            y_twips: 0,
        }
    );
    assert_eq!(
        shape.points[1],
        open_rtf_converter::model::StaticShapePoint {
            x_twips: 2100,
            y_twips: 1422,
        }
    );
    assert!(
        shape.points.iter().any(|point| point.y_twips == 0)
            && shape
                .points
                .iter()
                .any(|point| point.y_twips == shape.height_twips)
            && shape.points.iter().any(|point| point.x_twips > 0)
            && shape
                .points
                .iter()
                .any(|point| point.x_twips < shape.width_twips),
        "rotated rectangle should normalize to non-axis-aligned passive polygon points: {:?}",
        shape.points
    );
    for forbidden in ["rotation", "pFragments", "hidden-rotation-payload"] {
        assert!(
            !text.contains(forbidden),
            "rotated shape metadata leaked to normalized text: {forbidden}"
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
        "rotated rectangle should render through passive polygon line segments"
    );
    for forbidden in [
        b"rotation".as_slice(),
        b"pFragments",
        b"hidden-rotation-payload",
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
            "rotated shape metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn rotated_office_round_rectangles_lower_to_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 5}}{\sp{\sn fillColor}{\sv 65280}}{\sp{\sn rotation}{\sv 2700000}}{\sp{\sn pFragments}{\sv hidden-rotated-roundrect-payload}}}}After\par}"#.to_vec();
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
        .expect("rotated passive rounded rectangle shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert!(
        shape.points.len() >= 36,
        "rotated rounded rectangle should preserve a dense sampled passive rounded outline"
    );
    assert!(
        shape.points.iter().all(|point| {
            point.x_twips >= 0
                && point.x_twips <= shape.width_twips
                && point.y_twips >= 0
                && point.y_twips <= shape.height_twips
        }),
        "rotated rounded rectangle points must stay inside normalized passive frame: {shape:?}"
    );
    assert!(
        shape
            .points
            .iter()
            .filter(|point| {
                point.x_twips <= shape.width_twips / 5
                    || point.x_twips >= (shape.width_twips * 4) / 5
            })
            .count()
            >= 12,
        "rotated rounded rectangle should keep bounded passive rounded side samples"
    );
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
        "rotation",
        "pFragments",
        "hidden-rotated-roundrect-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "rotated rounded rectangle metadata leaked to normalized text: {forbidden}"
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
            .contains("stripping unsupported/active drawing properties")),
        "hostile rounded-rectangle payload metadata should still be stripped: {:?}",
        output.diagnostics
    );
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("shape rotation rendered as bounded passive static geometry")
    }));
    assert!(output.diagnostics.iter().all(|diagnostic| !diagnostic.message.contains(
        "rotated rounded rectangle rendered as bounded passive polygon with rounded corners flattened"
    )));
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
        "rotated rounded rectangle fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"rotation",
        b"pFragments",
        b"hidden-rotated-roundrect-payload",
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
            "rotated rounded rectangle metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_type_five_renders_as_passive_rounded_rectangle_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 5}}{\sp{\sn fillColor}{\sv 13434879}}{\sp{\sn pFragments}{\sv hidden-shapetype-five-payload}}}}After\par}"#.to_vec();
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
        .expect("passive rounded rectangle shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::RoundedRectangle);
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 255,
            green: 255,
            blue: 204,
        })
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "pFragments",
        "hidden-shapetype-five-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "shapeType 5 metadata leaked to normalized text: {forbidden}"
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
            .filter(|operation| operation.operator == "c")
            .count()
            >= 4,
        "shapeType 5 should render rounded rectangle curves passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"pFragments",
        b"hidden-shapetype-five-payload",
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
            "shapeType 5 metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn rotated_office_diamonds_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 4}}{\sp{\sn fillColor}{\sv 16711680}}{\sp{\sn lineColor}{\sv 255}}{\sp{\sn rotation}{\sv 2700000}}{\sp{\sn pFragments}{\sv hidden-rotated-diamond-payload}}}}After\par}"#.to_vec();
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
        .expect("rotated passive diamond shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 4);
    assert!(
        shape
            .points
            .iter()
            .any(|point| point.x_twips != 0 && point.y_twips != 0),
        "rotation should preserve non-axis-aligned passive diamond coordinates"
    );
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 0,
            green: 0,
            blue: 255,
        })
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "rotation",
        "pFragments",
        "hidden-rotated-diamond-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "rotated diamond metadata leaked to normalized text: {forbidden}"
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
        "rotated diamond fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"rotation",
        b"pFragments",
        b"hidden-rotated-diamond-payload",
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
            "rotated diamond metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn rotated_office_triangles_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 7}}{\sp{\sn fillColor}{\sv 65535}}{\sp{\sn lineColor}{\sv 16711680}}{\sp{\sn rotation}{\sv 900000}}{\sp{\sn pFragments}{\sv hidden-rotated-triangle-payload}}}}After\par}"#.to_vec();
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
        .expect("rotated passive triangle shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 3);
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 255,
            green: 255,
            blue: 0,
        })
    );
    assert!(
        shape
            .points
            .iter()
            .any(|point| point.x_twips != 0 && point.y_twips != 0),
        "rotation should preserve non-axis-aligned passive triangle coordinates"
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "rotation",
        "pFragments",
        "hidden-rotated-triangle-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "rotated triangle metadata leaked to normalized text: {forbidden}"
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
        "rotated triangle fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"rotation",
        b"pFragments",
        b"hidden-rotated-triangle-payload",
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
            "rotated triangle metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_right_triangles_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 8}}{\sp{\sn fillColor}{\sv 33023}}{\sp{\sn lineColor}{\sv 255}}{\sp{\sn pFragments}{\sv hidden-right-triangle-payload}}}}After\par}"#.to_vec();
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
        .expect("passive right triangle shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 3);
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 255,
            green: 128,
            blue: 0,
        })
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "hidden-right-triangle-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "right triangle metadata leaked to normalized text: {forbidden}"
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
        "right triangle fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-right-triangle-payload",
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
            "right triangle metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn rotated_office_trapezoids_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 3}}{\sp{\sn fillColor}{\sv 65535}}{\sp{\sn lineColor}{\sv 16711680}}{\sp{\sn rotation}{\sv 1800000}}{\sp{\sn pFragments}{\sv hidden-rotated-trapezoid-payload}}}}After\par}"#.to_vec();
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
        .expect("rotated passive trapezoid shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 4);
    assert!(
        shape
            .points
            .iter()
            .any(|point| point.x_twips != 0 && point.y_twips != 0),
        "rotation should preserve non-axis-aligned passive trapezoid coordinates"
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "rotation",
        "pFragments",
        "hidden-rotated-trapezoid-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "rotated trapezoid metadata leaked to normalized text: {forbidden}"
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
        "rotated trapezoid fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"rotation",
        b"pFragments",
        b"hidden-rotated-trapezoid-payload",
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
            "rotated trapezoid metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn rotated_office_parallelograms_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 2}}{\sp{\sn fillColor}{\sv 13395456}}{\sp{\sn lineColor}{\sv 255}}{\sp{\sn rotation}{\sv 1200000}}{\sp{\sn pFragments}{\sv hidden-rotated-parallelogram-payload}}}}After\par}"#.to_vec();
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
        .expect("rotated passive parallelogram shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 4);
    assert!(
        shape
            .points
            .iter()
            .any(|point| point.x_twips != 0 && point.y_twips != 0),
        "rotation should preserve non-axis-aligned passive parallelogram coordinates"
    );
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 0,
            green: 102,
            blue: 204,
        })
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "rotation",
        "pFragments",
        "hidden-rotated-parallelogram-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "rotated parallelogram metadata leaked to normalized text: {forbidden}"
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
        "rotated parallelogram fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"rotation",
        b"pFragments",
        b"hidden-rotated-parallelogram-payload",
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
            "rotated parallelogram metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn rotated_office_octagons_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 6}}{\sp{\sn fillColor}{\sv 10079232}}{\sp{\sn lineColor}{\sv 255}}{\sp{\sn rotation}{\sv 1800000}}{\sp{\sn pFragments}{\sv hidden-rotated-octagon-payload}}}}After\par}"#.to_vec();
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
        .expect("rotated passive octagon shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 8);
    assert!(
        shape
            .points
            .iter()
            .any(|point| point.x_twips != 0 && point.y_twips != 0),
        "rotation should preserve non-axis-aligned passive octagon coordinates"
    );
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 0,
            green: 204,
            blue: 153,
        })
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "rotation",
        "pFragments",
        "hidden-rotated-octagon-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "rotated octagon metadata leaked to normalized text: {forbidden}"
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
        "rotated octagon fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"rotation",
        b"pFragments",
        b"hidden-rotated-octagon-payload",
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
            "rotated octagon metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn rotated_office_hexagons_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 10}}{\sp{\sn fillColor}{\sv 3368448}}{\sp{\sn lineColor}{\sv 255}}{\sp{\sn rotation}{\sv 1500000}}{\sp{\sn pFragments}{\sv hidden-rotated-hexagon-payload}}}}After\par}"#.to_vec();
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
        .expect("rotated passive hexagon shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 6);
    assert!(
        shape
            .points
            .iter()
            .any(|point| point.x_twips != 0 && point.y_twips != 0),
        "rotation should preserve non-axis-aligned passive hexagon coordinates"
    );
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 0,
            green: 102,
            blue: 51,
        })
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "rotation",
        "pFragments",
        "hidden-rotated-hexagon-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "rotated hexagon metadata leaked to normalized text: {forbidden}"
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
        "rotated hexagon fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"rotation",
        b"pFragments",
        b"hidden-rotated-hexagon-payload",
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
            "rotated hexagon metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn rotated_office_crosses_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 11}}{\sp{\sn fillColor}{\sv 16744448}}{\sp{\sn lineColor}{\sv 255}}{\sp{\sn rotation}{\sv 900000}}{\sp{\sn pFragments}{\sv hidden-rotated-cross-payload}}}}After\par}"#.to_vec();
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
        .expect("rotated passive cross shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 12);
    assert!(
        shape
            .points
            .iter()
            .any(|point| point.x_twips != 0 && point.y_twips != 0),
        "rotation should preserve non-axis-aligned passive cross coordinates"
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
        "rotation",
        "pFragments",
        "hidden-rotated-cross-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "rotated cross metadata leaked to normalized text: {forbidden}"
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
        "rotated cross fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"rotation",
        b"pFragments",
        b"hidden-rotated-cross-payload",
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
            "rotated cross metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_pentagons_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 12}}{\sp{\sn fillColor}{\sv 10040064}}{\sp{\sn lineColor}{\sv 16711680}}{\sp{\sn pFragments}{\sv hidden-pentagon-payload}}}}After\par}"#.to_vec();
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
        .expect("passive pentagon shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 5);
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
        "hidden-pentagon-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "pentagon metadata leaked to normalized text: {forbidden}"
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
        "pentagon fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-pentagon-payload",
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
            "pentagon metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_pentagon_and_chevron_shape_types_render_as_passive_polygons_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 51}}{\sp{\sn fillColor}{\sv 10040064}}{\sp{\sn lineColor}{\sv 16711680}}{\sp{\sn pFragments}{\sv hidden-office-pentagon-payload}}}}{\shp{\*\shpinst\shpleft720\shptop1800\shpright2880\shpbottom2520{\sp{\sn shapeType}{\sv 52}}{\sp{\sn fillColor}{\sv 65280}}{\sp{\sn lineColor}{\sv 255}}{\sp{\sn pFragments}{\sv hidden-office-chevron-payload}}}}After\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let shapes: Vec<_> = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .collect();

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shapes.len(), 2);
    assert_eq!(shapes[0].kind, StaticShapeKind::Polygon);
    assert_eq!(shapes[0].points.len(), 5);
    assert_eq!(shapes[1].kind, StaticShapeKind::Polygon);
    assert_eq!(shapes[1].points.len(), 6);
    assert_eq!(shapes[1].points[2].x_twips, shapes[1].width_twips);
    assert_eq!(shapes[1].points[2].y_twips, shapes[1].height_twips / 2);
    assert_eq!(shapes[1].points[5].x_twips, shapes[1].width_twips / 3);
    assert_eq!(shapes[1].points[5].y_twips, shapes[1].height_twips / 2);
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "hidden-office-pentagon-payload",
        "hidden-office-chevron-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office polygon metadata leaked to normalized text: {forbidden}"
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
        "Office pentagon and chevron should render passive fill/stroke paths"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-office-pentagon-payload",
        b"hidden-office-chevron-payload",
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
            "Office polygon metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_3d_and_folded_shapes_render_as_passive_polygons_without_payload_leakage() {
    let mut input = String::from("{\\rtf1 Before\\par");
    for (shape_type, payload) in [
        (13, "hidden-can-payload"),
        (14, "hidden-cube-payload"),
        (15, "hidden-bevel-payload"),
        (16, "hidden-folded-corner-payload"),
    ] {
        input.push_str(&format!(
            "{{\\shp{{\\*\\shpinst\\shpleft720\\shptop720\\shpright3600\\shpbottom1800{{\\sp{{\\sn shapeType}}{{\\sv {shape_type}}}}}{{\\sp{{\\sn fillColor}}{{\\sv 65535}}}}{{\\sp{{\\sn lineColor}}{{\\sv 16711680}}}}{{\\sp{{\\sn pFragments}}{{\\sv {payload}}}}}}}}}"
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
    assert_eq!(shapes.len(), 4);
    for (shape, expected_points) in shapes.iter().zip([18, 6, 8, 5]) {
        assert_eq!(shape.kind, StaticShapeKind::Polygon);
        assert_eq!(shape.points.len(), expected_points);
        assert!(
            shape.points.iter().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "3D/folded shape points must stay inside the passive shape frame"
        );
    }
    assert_eq!(
        shapes[0].overlay_paths.len(),
        1,
        "can should preserve passive top-face strokes"
    );
    assert_eq!(
        shapes[1].overlay_paths.len(),
        3,
        "cube should preserve passive internal face strokes"
    );
    assert_eq!(
        shapes[2].overlay_paths.len(),
        4,
        "bevel should preserve passive internal face strokes"
    );
    assert_eq!(
        shapes[3].overlay_paths.len(),
        2,
        "folded corner should preserve passive crease strokes"
    );
    for shape in [&shapes[0], &shapes[1], &shapes[2], &shapes[3]] {
        assert!(
            shape
                .overlay_paths
                .iter()
                .all(|path| (2..=RtfLimits::default().max_shape_points).contains(&path.len())),
            "3D/folded overlays should be bounded passive paths"
        );
        assert!(
            shape.overlay_paths.iter().flatten().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "3D/folded overlay points must stay inside the passive shape frame"
        );
    }
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "hidden-can-payload",
        "hidden-cube-payload",
        "hidden-bevel-payload",
        "hidden-folded-corner-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "3D/folded shape metadata leaked to normalized text: {forbidden}"
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
    let mut passive_shape_paints = 0;
    let mut passive_overlay_strokes = 0;
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
        passive_shape_paints >= 4,
        "3D/folded shapes should render passive fill/stroke paths"
    );
    assert!(
        passive_overlay_strokes >= 1,
        "3D/folded face details should render as passive strokes"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-can-payload",
        b"hidden-cube-payload",
        b"hidden-bevel-payload",
        b"hidden-folded-corner-payload",
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
            "3D/folded shape metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_plaque_bracket_and_brace_shapes_render_as_passive_polygons_without_payload_leakage() {
    let mut input = String::from("{\\rtf1 Before\\par");
    for (shape_type, payload) in [
        (28, "hidden-plaque-payload"),
        (29, "hidden-left-bracket-payload"),
        (30, "hidden-right-bracket-payload"),
        (31, "hidden-left-brace-payload"),
        (32, "hidden-right-brace-payload"),
    ] {
        input.push_str(&format!(
            "{{\\shp{{\\*\\shpinst\\shpleft720\\shptop720\\shpright3600\\shpbottom1800{{\\sp{{\\sn shapeType}}{{\\sv {shape_type}}}}}{{\\sp{{\\sn fillColor}}{{\\sv 65535}}}}{{\\sp{{\\sn lineColor}}{{\\sv 16711680}}}}{{\\sp{{\\sn pFragments}}{{\\sv {payload}}}}}}}}}"
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
    assert_eq!(shapes.len(), 5);
    for (shape, expected_points) in shapes.iter().zip([24, 16, 16, 46, 46]) {
        assert_eq!(shape.kind, StaticShapeKind::Polygon);
        assert_eq!(shape.points.len(), expected_points);
        assert!(
            shape.points.iter().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "plaque/bracket/brace points must stay inside the passive shape frame"
        );
    }
    assert!(
        shapes[0]
            .points
            .iter()
            .filter(|point| {
                point.x_twips >= shapes[0].width_twips - (shapes[0].width_twips / 6)
                    && point.y_twips <= shapes[0].height_twips / 6
            })
            .count()
            >= 5,
        "plaque should preserve a bounded passive top-right scallop"
    );
    assert!(
        shapes[0]
            .points
            .iter()
            .filter(|point| {
                point.x_twips <= shapes[0].width_twips / 6
                    && point.y_twips >= shapes[0].height_twips - (shapes[0].height_twips / 6)
            })
            .count()
            >= 5,
        "plaque should preserve a bounded passive bottom-left scallop"
    );
    assert_eq!(shapes[1].points[0].x_twips, shapes[1].width_twips);
    assert_eq!(shapes[2].points[0].x_twips, 0);
    assert!(
        shapes[1]
            .points
            .iter()
            .filter(|point| {
                point.x_twips <= (shapes[1].width_twips * 2) / 3
                    && point.y_twips >= shapes[1].height_twips / 5
                    && point.y_twips <= (shapes[1].height_twips * 4) / 5
            })
            .count()
            >= 8,
        "left bracket should preserve a denser bounded passive inner shoulder"
    );
    assert!(
        shapes[2]
            .points
            .iter()
            .filter(|point| {
                point.x_twips >= shapes[2].width_twips / 3
                    && point.y_twips >= shapes[2].height_twips / 5
                    && point.y_twips <= (shapes[2].height_twips * 4) / 5
            })
            .count()
            >= 8,
        "right bracket should preserve a denser bounded passive inner shoulder"
    );
    assert!(
        shapes[3]
            .points
            .iter()
            .filter(|point| {
                point.x_twips <= shapes[3].width_twips / 6
                    && point.y_twips >= (shapes[3].height_twips * 2) / 5
                    && point.y_twips <= (shapes[3].height_twips * 3) / 5
            })
            .count()
            >= 5,
        "left brace should preserve a bounded passive waist curve"
    );
    assert!(
        shapes[4]
            .points
            .iter()
            .filter(|point| {
                point.x_twips >= shapes[4].width_twips - (shapes[4].width_twips / 6)
                    && point.y_twips >= (shapes[4].height_twips * 2) / 5
                    && point.y_twips <= (shapes[4].height_twips * 3) / 5
            })
            .count()
            >= 5,
        "right brace should preserve a bounded passive waist curve"
    );
    assert!(
        [&shapes[3], &shapes[4]].iter().all(|shape| {
            shape
                .points
                .iter()
                .filter(|point| {
                    point.y_twips <= shape.height_twips / 4
                        || point.y_twips >= shape.height_twips - (shape.height_twips / 4)
                })
                .count()
                >= 12
        }),
        "brace shoulders should preserve bounded passive curve samples"
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "hidden-plaque-payload",
        "hidden-left-bracket-payload",
        "hidden-right-bracket-payload",
        "hidden-left-brace-payload",
        "hidden-right-brace-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "plaque/bracket/brace metadata leaked to normalized text: {forbidden}"
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
    let mut passive_shape_paints = 0;
    for page_id in parsed_pdf.get_pages().values() {
        let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
        rendered_text.push_str(&decoded_pdf_text(&content));
        passive_shape_paints += content
            .operations
            .iter()
            .filter(|operation| operation.operator == "B")
            .count();
    }

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        passive_shape_paints >= 5,
        "plaque/bracket/brace shapes should render passive fill/stroke paths"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-plaque-payload",
        b"hidden-left-bracket-payload",
        b"hidden-right-bracket-payload",
        b"hidden-left-brace-payload",
        b"hidden-right-brace-payload",
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
            "plaque/bracket/brace metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_heart_and_lightning_shapes_render_as_passive_polygons_without_payload_leakage() {
    let mut input = String::from("{\\rtf1 Before\\par");
    for (shape_type, payload) in [
        (21, "hidden-heart-payload"),
        (22, "hidden-lightning-payload"),
    ] {
        input.push_str(&format!(
            "{{\\shp{{\\*\\shpinst\\shpleft720\\shptop720\\shpright3600\\shpbottom1800{{\\sp{{\\sn shapeType}}{{\\sv {shape_type}}}}}{{\\sp{{\\sn fillColor}}{{\\sv 33023}}}}{{\\sp{{\\sn lineColor}}{{\\sv 16711680}}}}{{\\sp{{\\sn pFragments}}{{\\sv {payload}}}}}}}}}"
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
    assert_eq!(shapes.len(), 2);
    assert_eq!(shapes[0].kind, StaticShapeKind::Polygon);
    assert_eq!(shapes[0].points.len(), 48);
    assert_eq!(shapes[1].kind, StaticShapeKind::Polygon);
    assert_eq!(shapes[1].points.len(), 6);
    for shape in &shapes {
        assert!(
            shape.points.iter().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "heart/lightning points must stay inside the passive shape frame"
        );
    }
    assert!(
        shapes[0]
            .points
            .iter()
            .filter(|point| {
                point.y_twips <= shapes[0].height_twips / 10
                    && point.x_twips >= shapes[0].width_twips / 6
                    && point.x_twips <= shapes[0].width_twips / 3
            })
            .count()
            >= 4,
        "heart should preserve bounded passive left-lobe curve samples"
    );
    assert!(
        shapes[0]
            .points
            .iter()
            .filter(|point| {
                point.y_twips <= shapes[0].height_twips / 10
                    && point.x_twips >= (shapes[0].width_twips * 2) / 3
                    && point.x_twips <= (shapes[0].width_twips * 5) / 6
            })
            .count()
            >= 4,
        "heart should preserve bounded passive right-lobe curve samples"
    );
    assert!(
        shapes[0]
            .points
            .iter()
            .filter(|point| {
                point.x_twips >= (shapes[0].width_twips * 9) / 20
                    && point.x_twips <= (shapes[0].width_twips * 11) / 20
                    && point.y_twips <= shapes[0].height_twips / 4
            })
            .count()
            >= 5,
        "heart should preserve a bounded passive central cleft"
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "hidden-heart-payload",
        "hidden-lightning-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "heart/lightning metadata leaked to normalized text: {forbidden}"
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
    let mut passive_shape_paints = 0;
    for page_id in parsed_pdf.get_pages().values() {
        let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
        rendered_text.push_str(&decoded_pdf_text(&content));
        passive_shape_paints += content
            .operations
            .iter()
            .filter(|operation| operation.operator == "B")
            .count();
    }

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        passive_shape_paints >= 2,
        "heart/lightning shapes should render passive fill/stroke paths"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-heart-payload",
        b"hidden-lightning-payload",
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
            "heart/lightning metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_sun_and_moon_shapes_render_as_passive_polygons_without_payload_leakage() {
    let mut input = String::from("{\\rtf1 Before\\par");
    for (shape_type, payload) in [(23, "hidden-sun-payload"), (24, "hidden-moon-payload")] {
        input.push_str(&format!(
            "{{\\shp{{\\*\\shpinst\\shpleft720\\shptop720\\shpright3600\\shpbottom1800{{\\sp{{\\sn shapeType}}{{\\sv {shape_type}}}}}{{\\sp{{\\sn fillColor}}{{\\sv 65535}}}}{{\\sp{{\\sn lineColor}}{{\\sv 16711680}}}}{{\\sp{{\\sn pFragments}}{{\\sv {payload}}}}}}}}}"
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
    assert_eq!(shapes.len(), 2);
    assert_eq!(shapes[0].kind, StaticShapeKind::Polygon);
    assert_eq!(shapes[0].points.len(), 32);
    assert_eq!(shapes[1].kind, StaticShapeKind::Polygon);
    assert_eq!(shapes[1].points.len(), 49);
    for shape in &shapes {
        assert!(
            shape.points.iter().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "sun/moon points must stay inside the passive shape frame"
        );
    }
    assert_eq!(
        shapes[0].overlay_paths.len(),
        1,
        "sun should preserve a passive inner disk boundary"
    );
    assert!(
        shapes[0].overlay_paths[0].len() >= 49,
        "sun inner disk boundary should be a bounded passive polyline"
    );
    assert!(
        shapes[0].overlay_paths.iter().flatten().all(|point| {
            point.x_twips >= 0
                && point.x_twips <= shapes[0].width_twips
                && point.y_twips >= 0
                && point.y_twips <= shapes[0].height_twips
        }),
        "sun inner disk boundary must stay inside the passive shape frame"
    );
    assert!(
        shapes[1]
            .points
            .iter()
            .filter(|point| {
                point.x_twips >= shapes[1].width_twips / 3
                    && point.x_twips <= shapes[1].width_twips / 2
                    && point.y_twips >= (shapes[1].height_twips * 2) / 5
                    && point.y_twips <= (shapes[1].height_twips * 3) / 5
            })
            .count()
            >= 9,
        "moon should preserve bounded passive inner crescent samples"
    );
    assert!(
        shapes[1]
            .points
            .iter()
            .filter(|point| point.y_twips >= (shapes[1].height_twips * 19) / 20)
            .count()
            >= 7,
        "moon should preserve bounded passive lower outer curve samples"
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "hidden-sun-payload",
        "hidden-moon-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "sun/moon metadata leaked to normalized text: {forbidden}"
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
    let mut passive_shape_paints = 0;
    let mut passive_overlay_strokes = 0;
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
        passive_shape_paints >= 2,
        "sun/moon shapes should render passive fill/stroke paths"
    );
    assert!(
        passive_overlay_strokes >= 1,
        "sun inner disk boundary should render as a passive stroke"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-sun-payload",
        b"hidden-moon-payload",
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
            "sun/moon metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_flowchart_storage_and_io_shapes_render_passively_without_payload_leakage() {
    let mut input = String::from("{\\rtf1 Before\\par");
    for (shape_type, payload) in [
        (75, "flowchart-punched-card-payload"),
        (76, "flowchart-punched-tape-payload"),
        (77, "flowchart-summing-junction-payload"),
        (78, "flowchart-or-payload"),
        (79, "flowchart-collate-payload"),
        (80, "flowchart-sort-payload"),
        (83, "flowchart-stored-data-payload"),
        (84, "flowchart-delay-payload"),
        (85, "flowchart-sequential-access-storage-payload"),
        (86, "flowchart-magnetic-disk-payload"),
        (87, "flowchart-direct-access-storage-payload"),
        (88, "flowchart-display-payload"),
    ] {
        input.push_str(&format!(
            "{{\\shp{{\\*\\shpinst\\shpleft720\\shptop720\\shpright2880\\shpbottom1440{{\\sp{{\\sn shapeType}}{{\\sv {shape_type}}}}}{{\\sp{{\\sn fillColor}}{{\\sv 65280}}}}{{\\sp{{\\sn lineColor}}{{\\sv 16711680}}}}{{\\sp{{\\sn pFragments}}{{\\sv {payload}}}}}}}}}"
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
    assert_eq!(shapes.len(), 12);
    let expected_point_counts = [5, 34, 24, 24, 6, 4, 18, 25, 32, 18, 6, 40];
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
            "flowchart storage/io shape points must stay inside passive frame: {shape:?}"
        );
    }
    assert_eq!(shapes[0].points[0].x_twips, shapes[0].width_twips / 5);
    assert!(
        shapes[1]
            .points
            .iter()
            .filter(|point| point.y_twips < shapes[1].height_twips / 8)
            .count()
            >= 8,
        "flowchart punched tape should preserve a bounded passive top wave"
    );
    assert!(
        shapes[1]
            .points
            .iter()
            .filter(|point| point.y_twips > shapes[1].height_twips - (shapes[1].height_twips / 8))
            .count()
            >= 8,
        "flowchart punched tape should preserve a bounded passive bottom wave"
    );
    assert_eq!(
        shapes[5].overlay_paths.len(),
        1,
        "flowchart sort should preserve the passive internal divider"
    );
    assert_eq!(shapes[5].overlay_paths[0].len(), 2);
    assert!(
        shapes[5].overlay_paths.iter().flatten().all(|point| {
            point.x_twips >= 0
                && point.x_twips <= shapes[5].width_twips
                && point.y_twips >= 0
                && point.y_twips <= shapes[5].height_twips
        }),
        "flowchart sort divider must stay inside passive frame"
    );
    for shape in [&shapes[2], &shapes[3]] {
        assert_eq!(
            shape.overlay_paths.len(),
            2,
            "flowchart summing/or shapes should preserve Word-visible passive math marks"
        );
        assert!(
            shape.overlay_paths.iter().all(|path| path.len() == 2),
            "flowchart summing/or marks should be passive line segments"
        );
        assert!(
            shape.overlay_paths.iter().flatten().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "flowchart summing/or marks must stay inside passive frame"
        );
    }
    for shape in [&shapes[6], &shapes[9]] {
        assert_eq!(
            shape.overlay_paths.len(),
            1,
            "can-style storage shapes should preserve the Word-visible top-face curve"
        );
        assert!(
            shape.overlay_paths.iter().all(|path| path.len() >= 17),
            "can-style storage face overlay should be a sampled passive curve"
        );
        assert!(
            shape.overlay_paths.iter().flatten().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "can-style storage face overlays must stay inside passive frame"
        );
        assert!(
            shape
                .points
                .iter()
                .filter(|point| point.y_twips <= shape.height_twips / 8)
                .count()
                >= 6,
            "can-style storage should preserve a bounded passive top ellipse"
        );
        assert!(
            shape
                .points
                .iter()
                .filter(|point| point.y_twips >= shape.height_twips - (shape.height_twips / 8))
                .count()
                >= 6,
            "can-style storage should preserve a bounded passive bottom ellipse"
        );
    }
    assert!(
        shapes[7]
            .points
            .iter()
            .any(|point| point.x_twips == shapes[7].width_twips),
        "flowchart delay curve should reach the right edge of the passive frame"
    );
    assert!(
        shapes[7]
            .points
            .iter()
            .filter(|point| point.x_twips > shapes[7].width_twips / 2)
            .count()
            >= 16,
        "flowchart delay should preserve a bounded passive rounded side"
    );
    assert!(
        shapes[7]
            .points
            .iter()
            .any(|point| point.x_twips > (shapes[7].width_twips * 9) / 10
                && point.y_twips < shapes[7].height_twips / 3)
            && shapes[7].points.iter().any(|point| {
                point.x_twips > (shapes[7].width_twips * 9) / 10
                    && point.y_twips > (shapes[7].height_twips * 2) / 3
            }),
        "flowchart delay should preserve bounded passive upper and lower curve shoulders"
    );
    assert!(
        shapes[11]
            .points
            .iter()
            .any(|point| point.x_twips == shapes[11].width_twips),
        "flowchart display curve should reach the right edge of the passive frame"
    );
    assert!(
        shapes[11]
            .points
            .iter()
            .filter(|point| point.x_twips > (shapes[11].width_twips * 9) / 10)
            .count()
            >= 8,
        "flowchart display should preserve a bounded passive rounded right nose"
    );
    assert!(
        shapes[11]
            .points
            .iter()
            .filter(|point| {
                point.x_twips <= shapes[11].width_twips / 8
                    && point.y_twips > shapes[11].height_twips / 8
                    && point.y_twips < shapes[11].height_twips
            })
            .count()
            >= 8,
        "flowchart display should preserve a bounded passive concave left edge"
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "flowchart-punched-card-payload",
        "flowchart-punched-tape-payload",
        "flowchart-summing-junction-payload",
        "flowchart-or-payload",
        "flowchart-collate-payload",
        "flowchart-sort-payload",
        "flowchart-stored-data-payload",
        "flowchart-delay-payload",
        "flowchart-sequential-access-storage-payload",
        "flowchart-magnetic-disk-payload",
        "flowchart-direct-access-storage-payload",
        "flowchart-display-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "flowchart storage/io metadata leaked to normalized text: {forbidden}"
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
        passive_shape_paints >= 12,
        "flowchart storage/io shapes should render passive fill/stroke paths"
    );
    assert!(
        passive_overlay_strokes >= 5,
        "flowchart summing/or marks, sort divider, and can-style storage face overlays should render as passive strokes"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"flowchart-punched-card-payload",
        b"flowchart-punched-tape-payload",
        b"flowchart-summing-junction-payload",
        b"flowchart-or-payload",
        b"flowchart-collate-payload",
        b"flowchart-sort-payload",
        b"flowchart-stored-data-payload",
        b"flowchart-delay-payload",
        b"flowchart-sequential-access-storage-payload",
        b"flowchart-magnetic-disk-payload",
        b"flowchart-direct-access-storage-payload",
        b"flowchart-display-payload",
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
            "flowchart storage/io metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_ribbon_scroll_and_wave_shapes_render_passively_without_payload_leakage() {
    let mut input = String::from("{\\rtf1 Before\\par");
    for (shape_type, payload) in [
        (97, "office-up-ribbon-payload"),
        (98, "office-down-ribbon-payload"),
        (99, "office-curved-up-ribbon-payload"),
        (100, "office-curved-down-ribbon-payload"),
        (101, "office-vertical-scroll-payload"),
        (102, "office-horizontal-scroll-payload"),
        (103, "office-wave-payload"),
        (104, "office-double-wave-payload"),
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
    let expected_point_counts = [9, 9, 28, 30, 27, 26, 38, 58];
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
            "ribbon/scroll/wave shape points must stay inside passive frame: {shape:?}"
        );
    }
    assert_eq!(shapes[0].points[1].y_twips, 0);
    assert_eq!(shapes[1].points[5].y_twips, shapes[1].height_twips);
    for shape in shapes.iter().take(4) {
        assert_eq!(
            shape.overlay_paths.len(),
            2,
            "ribbon shapes should preserve passive fold-line strokes"
        );
        assert!(
            shape.overlay_paths.iter().all(|path| path.len() == 2),
            "ribbon fold lines should be passive line segments"
        );
        assert!(
            shape.overlay_paths.iter().flatten().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "ribbon fold lines must stay inside passive frame: {shape:?}"
        );
    }
    assert!(
        shapes[2]
            .points
            .iter()
            .filter(|point| point.y_twips <= shapes[2].height_twips / 8)
            .count()
            >= 9,
        "curved up ribbon should preserve denser bounded passive top arc samples"
    );
    assert!(
        shapes[3]
            .points
            .iter()
            .filter(|point| point.y_twips >= (shapes[3].height_twips * 7) / 8)
            .count()
            >= 9,
        "curved down ribbon should preserve denser bounded passive bottom arc samples"
    );
    assert_eq!(shapes[4].points[1].x_twips, shapes[4].width_twips);
    assert!(
        shapes[4]
            .points
            .iter()
            .filter(|point| {
                point.x_twips >= (shapes[4].width_twips * 17) / 20
                    && point.y_twips <= shapes[4].height_twips / 3
            })
            .count()
            >= 7,
        "vertical scroll should preserve a denser bounded passive upper curl"
    );
    assert!(
        shapes[5]
            .points
            .iter()
            .filter(|point| {
                point.x_twips >= (shapes[5].width_twips * 3) / 4
                    && point.y_twips <= shapes[5].height_twips / 6
            })
            .count()
            >= 7,
        "horizontal scroll should preserve a denser bounded passive right curl"
    );
    for shape in [&shapes[4], &shapes[5]] {
        assert_eq!(
            shape.overlay_paths.len(),
            2,
            "scroll shapes should preserve passive curled-roll seam strokes"
        );
        assert!(
            shape.overlay_paths.iter().all(|path| path.len() >= 9),
            "scroll curled-roll seams should be detailed passive bounded polylines"
        );
        assert!(
            shape.overlay_paths.iter().flatten().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "scroll curled-roll seams must stay inside passive frame: {shape:?}"
        );
    }
    assert!(
        shapes[6]
            .points
            .iter()
            .filter(|point| point.y_twips <= shapes[6].height_twips / 4)
            .count()
            >= 7,
        "wave should preserve a bounded passive upper crest"
    );
    assert!(
        shapes[6]
            .points
            .iter()
            .filter(|point| point.y_twips >= (shapes[6].height_twips * 3) / 4)
            .count()
            >= 7,
        "wave should preserve a bounded passive lower trough"
    );
    assert!(
        shapes[7]
            .points
            .iter()
            .filter(|point| point.y_twips <= shapes[7].height_twips / 4)
            .count()
            >= 12,
        "double wave should preserve bounded passive upper crests"
    );
    assert!(
        shapes[7]
            .points
            .iter()
            .filter(|point| point.y_twips >= (shapes[7].height_twips * 3) / 4)
            .count()
            >= 12,
        "double wave should preserve bounded passive lower troughs"
    );
    assert!(
        shapes[7]
            .points
            .iter()
            .any(|point| point.x_twips == shapes[7].width_twips)
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "office-up-ribbon-payload",
        "office-down-ribbon-payload",
        "office-curved-up-ribbon-payload",
        "office-curved-down-ribbon-payload",
        "office-vertical-scroll-payload",
        "office-horizontal-scroll-payload",
        "office-wave-payload",
        "office-double-wave-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "ribbon/scroll/wave metadata leaked to normalized text: {forbidden}"
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
        "ribbon/scroll/wave shapes should render passive fill/stroke paths"
    );
    assert!(
        passive_overlay_strokes >= 12,
        "ribbon folds and scroll curl seams should render as passive strokes"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"office-up-ribbon-payload",
        b"office-down-ribbon-payload",
        b"office-curved-up-ribbon-payload",
        b"office-curved-down-ribbon-payload",
        b"office-vertical-scroll-payload",
        b"office-horizontal-scroll-payload",
        b"office-wave-payload",
        b"office-double-wave-payload",
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
            "ribbon/scroll/wave metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_callout_shapes_render_passively_without_payload_leakage() {
    let mut input = String::from("{\\rtf1 Before\\par");
    for (shape_type, payload) in [
        (105, "office-rectangular-callout-payload"),
        (106, "office-rounded-rectangular-callout-payload"),
        (107, "office-oval-callout-payload"),
        (108, "office-cloud-callout-payload"),
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
    assert_eq!(shapes.len(), 4);
    let expected_point_counts = [7, 27, 60, 62];
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
            "callout shape points must stay inside passive frame: {shape:?}"
        );
    }
    assert_eq!(shapes[0].points[4].y_twips, shapes[0].height_twips);
    assert!(
        shapes[1]
            .points
            .iter()
            .any(|point| point.y_twips == shapes[1].height_twips),
        "rounded rectangular callout pointer should still reach the bottom of the passive frame"
    );
    assert!(
        shapes[1]
            .points
            .iter()
            .filter(|point| {
                point.x_twips <= shapes[1].width_twips / 8
                    && point.y_twips <= shapes[1].height_twips / 8
            })
            .count()
            >= 4,
        "rounded rectangular callout should preserve a bounded passive top-left corner arc"
    );
    assert!(
        shapes[1]
            .points
            .iter()
            .filter(|point| {
                point.x_twips >= shapes[1].width_twips - (shapes[1].width_twips / 8)
                    && point.y_twips <= shapes[1].height_twips / 8
            })
            .count()
            >= 4,
        "rounded rectangular callout should preserve a bounded passive top-right corner arc"
    );
    assert!(
        shapes[2]
            .points
            .iter()
            .any(|point| point.y_twips == shapes[2].height_twips),
        "oval callout pointer should still reach the bottom of the passive frame"
    );
    assert!(
        shapes[2]
            .points
            .iter()
            .filter(|point| {
                point.y_twips >= (shapes[2].height_twips * 9) / 10
                    && point.x_twips >= (shapes[2].width_twips * 2) / 5
                    && point.x_twips <= shapes[2].width_twips / 2
            })
            .count()
            >= 1,
        "oval callout should preserve a bounded passive pointer/lower-ellipse transition"
    );
    assert!(
        shapes[2]
            .points
            .iter()
            .filter(|point| {
                point.y_twips <= shapes[2].height_twips / 5
                    && point.x_twips >= shapes[2].width_twips / 4
                    && point.x_twips <= (shapes[2].width_twips * 7) / 8
            })
            .count()
            >= 13,
        "oval callout should preserve bounded passive upper ellipse samples"
    );
    assert!(
        shapes[2]
            .points
            .iter()
            .filter(|point| {
                point.y_twips >= (shapes[2].height_twips * 7) / 10
                    && point.y_twips < (shapes[2].height_twips * 9) / 10
            })
            .count()
            >= 10,
        "oval callout should preserve bounded passive lower ellipse samples"
    );
    assert!(
        shapes[3]
            .points
            .iter()
            .any(|point| point.y_twips == shapes[3].height_twips),
        "cloud callout pointer should still reach the bottom of the passive frame"
    );
    assert!(
        shapes[3]
            .points
            .iter()
            .filter(|point| {
                point.y_twips <= shapes[3].height_twips / 5
                    && point.x_twips >= shapes[3].width_twips / 4
            })
            .count()
            >= 12,
        "cloud callout should preserve bounded passive top lobe samples"
    );
    assert!(
        shapes[3]
            .points
            .iter()
            .filter(|point| {
                point.x_twips >= (shapes[3].width_twips * 9) / 10
                    && point.y_twips >= shapes[3].height_twips / 3
                    && point.y_twips <= shapes[3].height_twips / 2
            })
            .count()
            >= 3,
        "cloud callout should preserve bounded passive right lobe transition samples"
    );
    assert!(
        shapes[3]
            .points
            .iter()
            .filter(|point| {
                point.y_twips >= (shapes[3].height_twips * 7) / 10
                    && point.y_twips < (shapes[3].height_twips * 9) / 10
            })
            .count()
            >= 9,
        "cloud callout should preserve bounded passive lower lobe samples"
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "office-rectangular-callout-payload",
        "office-rounded-rectangular-callout-payload",
        "office-oval-callout-payload",
        "office-cloud-callout-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "callout metadata leaked to normalized text: {forbidden}"
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
    for page_id in parsed_pdf.get_pages().values() {
        let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
        rendered_text.push_str(&decoded_pdf_text(&content));
        passive_shape_paints += content
            .operations
            .iter()
            .filter(|operation| operation.operator == "B")
            .count();
    }

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        passive_shape_paints >= 4,
        "callout shapes should render passive fill/stroke paths"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"office-rectangular-callout-payload",
        b"office-rounded-rectangular-callout-payload",
        b"office-oval-callout-payload",
        b"office-cloud-callout-payload",
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
            "callout metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_action_button_shapes_render_passive_visuals_without_action_leakage() {
    let mut input = String::from("{\\rtf1 Before\\par");
    for (shape_type, payload) in [
        (125, "office-action-button-custom-payload"),
        (126, "office-action-button-home-payload"),
        (127, "office-action-button-help-payload"),
        (128, "office-action-button-information-payload"),
        (129, "office-action-button-back-previous-payload"),
        (130, "office-action-button-forward-next-payload"),
        (131, "office-action-button-beginning-payload"),
        (132, "office-action-button-end-payload"),
        (133, "office-action-button-return-payload"),
        (134, "office-action-button-document-payload"),
        (135, "office-action-button-sound-payload"),
        (136, "office-action-button-movie-payload"),
    ] {
        input.push_str(&format!(
            "{{\\shp{{\\*\\shpinst\\shpleft720\\shptop720\\shpright3600\\shpbottom1800{{\\sp{{\\sn shapeType}}{{\\sv {shape_type}}}}}{{\\sp{{\\sn fillColor}}{{\\sv 65280}}}}{{\\sp{{\\sn lineColor}}{{\\sv 16711680}}}}{{\\sp{{\\sn hlinkClick}}{{\\sv https://example.invalid/action-{shape_type}}}}}{{\\sp{{\\sn pFragments}}{{\\sv {payload}}}}}}}}}"
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
    assert_eq!(shapes.len(), 12);
    let expected_point_counts = [8, 11, 32, 8, 7, 7, 8, 8, 9, 5, 6, 9];
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
            "action button points must stay inside passive frame: {shape:?}"
        );
    }
    assert_eq!(shapes[1].points[1].y_twips, 0);
    assert!(
        shapes[2]
            .points
            .iter()
            .filter(|point| {
                point.y_twips <= shapes[2].height_twips / 3
                    && point.x_twips >= shapes[2].width_twips / 2
            })
            .count()
            >= 8,
        "action-button help mark should preserve a bounded passive upper curve"
    );
    assert!(
        shapes[2]
            .points
            .iter()
            .filter(|point| {
                point.y_twips >= shapes[2].height_twips / 2
                    && point.y_twips <= (shapes[2].height_twips * 3) / 4
            })
            .count()
            >= 5,
        "action-button help mark should preserve a bounded passive lower hook"
    );
    assert_eq!(shapes[2].point_paths.len(), 1);
    assert_eq!(shapes[2].point_paths[0].len(), 4);
    assert!(
        shapes[2]
            .point_paths
            .iter()
            .flatten()
            .all(|point| point.x_twips >= 0
                && point.x_twips <= shapes[2].width_twips
                && point.y_twips >= 0
                && point.y_twips <= shapes[2].height_twips),
        "action-button help dot path must stay inside passive frame"
    );
    assert_eq!(shapes[3].point_paths.len(), 1);
    assert_eq!(shapes[3].point_paths[0].len(), 4);
    assert!(
        shapes[3]
            .point_paths
            .iter()
            .flatten()
            .all(|point| point.x_twips >= 0
                && point.x_twips <= shapes[3].width_twips
                && point.y_twips >= 0
                && point.y_twips <= shapes[3].height_twips),
        "action-button information dot path must stay inside passive frame"
    );
    assert!(shapes[4].points.iter().any(|point| point.x_twips == 0));
    assert_eq!(shapes[4].point_paths.len(), 1);
    assert_eq!(shapes[4].point_paths[0].len(), 3);
    assert!(
        shapes[4]
            .point_paths
            .iter()
            .flatten()
            .all(|point| point.x_twips >= 0
                && point.x_twips <= shapes[4].width_twips
                && point.y_twips >= 0
                && point.y_twips <= shapes[4].height_twips),
        "action-button back/previous secondary arrow must stay inside passive frame"
    );
    assert!(
        shapes[5]
            .points
            .iter()
            .any(|point| point.x_twips == shapes[5].width_twips)
    );
    assert_eq!(shapes[5].point_paths.len(), 1);
    assert_eq!(shapes[5].point_paths[0].len(), 3);
    assert!(
        shapes[5]
            .point_paths
            .iter()
            .flatten()
            .all(|point| point.x_twips >= 0
                && point.x_twips <= shapes[5].width_twips
                && point.y_twips >= 0
                && point.y_twips <= shapes[5].height_twips),
        "action-button forward/next secondary arrow must stay inside passive frame"
    );
    assert_eq!(shapes[6].point_paths.len(), 1);
    assert_eq!(shapes[6].point_paths[0].len(), 3);
    assert!(
        shapes[6]
            .point_paths
            .iter()
            .flatten()
            .all(|point| point.x_twips >= 0
                && point.x_twips <= shapes[6].width_twips
                && point.y_twips >= 0
                && point.y_twips <= shapes[6].height_twips),
        "action-button beginning extra path must stay inside passive frame"
    );
    assert_eq!(shapes[7].point_paths.len(), 1);
    assert_eq!(shapes[7].point_paths[0].len(), 3);
    assert!(
        shapes[7]
            .point_paths
            .iter()
            .flatten()
            .all(|point| point.x_twips >= 0
                && point.x_twips <= shapes[7].width_twips
                && point.y_twips >= 0
                && point.y_twips <= shapes[7].height_twips),
        "action-button end extra path must stay inside passive frame"
    );
    assert_eq!(shapes[8].point_paths.len(), 1);
    assert_eq!(shapes[8].point_paths[0].len(), 3);
    assert!(
        shapes[8]
            .point_paths
            .iter()
            .flatten()
            .all(|point| point.x_twips >= 0
                && point.x_twips <= shapes[8].width_twips
                && point.y_twips >= 0
                && point.y_twips <= shapes[8].height_twips),
        "action-button return inner corner must stay inside passive frame"
    );
    for (index, shape) in shapes.iter().enumerate() {
        let expected_overlay_count = if index == 9 { 6 } else { 4 };
        assert_eq!(
            shape.overlay_paths.len(),
            expected_overlay_count,
            "action button {index} should include bounded passive bevel frame overlays"
        );
        assert!(
            shape.overlay_paths.iter().all(|path| path.len() == 2),
            "action button {index} frame details should use bounded passive line segments"
        );
        assert!(
            shape.overlay_paths.iter().flatten().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "action button {index} frame overlays must stay inside passive frame"
        );
    }
    assert_eq!(shapes[9].points[3].y_twips, shapes[9].height_twips);
    assert!(
        shapes[9]
            .overlay_paths
            .iter()
            .skip(4)
            .all(|path| path.len() == 2),
        "action-button document fold should append bounded passive line segments"
    );
    assert_eq!(shapes[10].point_paths.len(), 2);
    assert_eq!(shapes[10].point_paths[0].len(), 14);
    assert_eq!(shapes[10].point_paths[1].len(), 17);
    assert!(
        shapes[10]
            .point_paths
            .iter()
            .flatten()
            .all(|point| point.x_twips >= 0
                && point.x_twips <= shapes[10].width_twips
                && point.y_twips >= 0
                && point.y_twips <= shapes[10].height_twips),
        "action-button sound wave paths must stay inside passive frame"
    );
    assert_eq!(shapes[11].point_paths.len(), 4);
    assert!(shapes[11].point_paths.iter().all(|path| path.len() == 4));
    assert!(
        shapes[11]
            .point_paths
            .iter()
            .flatten()
            .all(|point| point.x_twips >= 0
                && point.x_twips <= shapes[11].width_twips
                && point.y_twips >= 0
                && point.y_twips <= shapes[11].height_twips),
        "action-button movie filmstrip detail paths must stay inside passive frame"
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "hlinkClick",
        "https://example.invalid",
        "pFragments",
        "office-action-button-custom-payload",
        "office-action-button-home-payload",
        "office-action-button-help-payload",
        "office-action-button-information-payload",
        "office-action-button-back-previous-payload",
        "office-action-button-forward-next-payload",
        "office-action-button-beginning-payload",
        "office-action-button-end-payload",
        "office-action-button-return-payload",
        "office-action-button-document-payload",
        "office-action-button-sound-payload",
        "office-action-button-movie-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "action button metadata leaked to normalized text: {forbidden}"
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
    let mut passive_path_moves = 0usize;
    let mut passive_overlay_strokes = 0usize;
    for page_id in parsed_pdf.get_pages().values() {
        let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
        rendered_text.push_str(&decoded_pdf_text(&content));
        passive_shape_paints += content
            .operations
            .iter()
            .filter(|operation| operation.operator == "B")
            .count();
        passive_path_moves += content
            .operations
            .iter()
            .filter(|operation| operation.operator == "m")
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
        passive_shape_paints >= 12,
        "action buttons should render passive fill/stroke paths"
    );
    assert!(
        passive_path_moves >= 25,
        "action button icon details should be included as passive subpaths"
    );
    assert!(
        passive_overlay_strokes >= 1,
        "action-button document fold should render as a passive overlay stroke"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"hlinkClick",
        b"https://example.invalid",
        b"pFragments",
        b"office-action-button-custom-payload",
        b"office-action-button-home-payload",
        b"office-action-button-help-payload",
        b"office-action-button-information-payload",
        b"office-action-button-back-previous-payload",
        b"office-action-button-forward-next-payload",
        b"office-action-button-beginning-payload",
        b"office-action-button-end-payload",
        b"office-action-button-return-payload",
        b"office-action-button-document-payload",
        b"office-action-button-sound-payload",
        b"office-action-button-movie-payload",
        b"[Shape skipped",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
        b"/Annots",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "action button metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_post_action_shape_group_renders_passively_without_payload_leakage() {
    let mut input = String::from("{\\rtf1 Before\\par");
    for (shape_type, payload) in [
        (137, "office-balloon-payload"),
        (139, "flowchart-offline-storage-payload"),
        (140, "office-left-right-ribbon-payload"),
        (141, "office-diagonal-stripe-payload"),
        (142, "office-pie-payload"),
        (143, "office-non-isosceles-trapezoid-payload"),
        (144, "office-decagon-payload"),
        (145, "office-heptagon-payload"),
        (146, "office-dodecagon-payload"),
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
    assert_eq!(shapes.len(), 9);
    let expected_point_counts = [57, 5, 30, 4, 33, 4, 10, 7, 12];
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
            "post-action shape points must stay inside passive frame: {shape:?}"
        );
    }
    assert!(
        shapes[0]
            .points
            .iter()
            .filter(|point| {
                point.y_twips >= (shapes[0].height_twips * 4) / 5
                    && point.x_twips >= (shapes[0].width_twips * 2) / 5
                    && point.x_twips <= (shapes[0].width_twips * 3) / 5
            })
            .count()
            >= 5,
        "balloon should preserve a bounded passive speech-tail join"
    );
    assert!(
        shapes[0]
            .points
            .iter()
            .filter(|point| {
                point.y_twips >= (shapes[0].height_twips * 3) / 4
                    && point.y_twips < (shapes[0].height_twips * 9) / 10
            })
            .count()
            >= 10,
        "balloon should preserve bounded passive lower-oval samples"
    );
    assert_eq!(shapes[1].points[0].x_twips, shapes[1].width_twips / 4);
    assert_eq!(shapes[1].points[3].x_twips, shapes[1].width_twips / 4);
    assert!(
        shapes[1]
            .points
            .iter()
            .any(|point| { point.x_twips == 0 && point.y_twips == shapes[1].height_twips / 2 }),
        "flowchart offline storage should preserve the passive left midpoint"
    );
    assert_eq!(
        shapes[2].overlay_paths.len(),
        2,
        "left-right ribbon should preserve passive fold-line strokes"
    );
    assert!(
        shapes[2].overlay_paths.iter().all(|path| path.len() == 2),
        "left-right ribbon folds should be passive line segments"
    );
    assert!(
        shapes[2].overlay_paths.iter().flatten().all(|point| {
            point.x_twips >= 0
                && point.x_twips <= shapes[2].width_twips
                && point.y_twips >= 0
                && point.y_twips <= shapes[2].height_twips
        }),
        "left-right ribbon fold lines must stay inside passive frame"
    );
    assert!(
        shapes[2]
            .points
            .iter()
            .filter(|point| point.y_twips <= shapes[2].height_twips / 6)
            .count()
            >= 10,
        "left-right ribbon should preserve bounded passive top ribbon curves"
    );
    assert!(
        shapes[2]
            .points
            .iter()
            .filter(|point| {
                point.y_twips >= (shapes[2].height_twips * 3) / 4
                    && point.x_twips >= shapes[2].width_twips / 6
                    && point.x_twips <= (shapes[2].width_twips * 5) / 6
            })
            .count()
            >= 8,
        "left-right ribbon should preserve bounded passive lower ribbon curves"
    );
    assert_eq!(
        shapes[3].overlay_paths.len(),
        1,
        "diagonal stripe should preserve a passive rectangular frame"
    );
    assert_eq!(
        shapes[3].overlay_paths[0].len(),
        4,
        "diagonal stripe frame should be a bounded rectangle overlay"
    );
    assert!(
        shapes[3].overlay_paths[0]
            .iter()
            .any(|point| point.x_twips == 0 && point.y_twips == 0)
            && shapes[3].overlay_paths[0].iter().any(|point| {
                point.x_twips == shapes[3].width_twips && point.y_twips == shapes[3].height_twips
            }),
        "diagonal stripe frame should preserve opposite passive frame corners"
    );
    assert!(shapes[6].points.iter().any(|point| point.y_twips == 0));
    assert!(shapes[7].points.iter().any(|point| point.x_twips == 0));
    assert!(
        shapes[8]
            .points
            .iter()
            .any(|point| point.x_twips == shapes[8].width_twips)
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "office-balloon-payload",
        "flowchart-offline-storage-payload",
        "office-left-right-ribbon-payload",
        "office-diagonal-stripe-payload",
        "office-pie-payload",
        "office-non-isosceles-trapezoid-payload",
        "office-decagon-payload",
        "office-heptagon-payload",
        "office-dodecagon-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "post-action shape metadata leaked to normalized text: {forbidden}"
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
        passive_shape_paints >= 9,
        "post-action shape group should render passive fill/stroke paths"
    );
    assert!(
        passive_overlay_strokes >= 3,
        "left-right ribbon folds and diagonal stripe frame should render as passive strokes"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"office-balloon-payload",
        b"flowchart-offline-storage-payload",
        b"office-left-right-ribbon-payload",
        b"office-diagonal-stripe-payload",
        b"office-pie-payload",
        b"office-non-isosceles-trapezoid-payload",
        b"office-decagon-payload",
        b"office-heptagon-payload",
        b"office-dodecagon-payload",
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
            "post-action shape metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn unsupported_office_shape_type_138_does_not_cross_pdf_boundary() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright3600\shpbottom1800{\sp{\sn shapeType}{\sv 138}}{\sp{\sn fillColor}{\sv 65280}}{\sp{\sn pFragments}{\sv unsupported-shape-138-payload}}}}After\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert!(!text.contains("unsupported-shape-138-payload"));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    for forbidden in [
        b"shapeType".as_slice(),
        b"unsupported-shape-138-payload",
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
            "unsupported shapeType 138 metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_right_arrows_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright3600\shpbottom1800{\sp{\sn shapeType}{\sv 33}}{\sp{\sn fillColor}{\sv 65280}}{\sp{\sn lineColor}{\sv 16711680}}{\sp{\sn pFragments}{\sv hidden-right-arrow-payload}}}}After\par}"#.to_vec();
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
        .expect("passive right arrow shape");

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
        "right arrow should keep a passive point at the arrow tip"
    );
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
        "hidden-right-arrow-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "right arrow metadata leaked to normalized text: {forbidden}"
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
        "right arrow fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-right-arrow-payload",
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
            "right arrow metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_cardinal_arrows_render_as_passive_polygons_without_payload_leakage() {
    for (shape_type, label, expected_tip) in [
        (34, "left", (0, 540)),
        (35, "up", (1440, 0)),
        (36, "down", (1440, 1080)),
    ] {
        let payload = format!("hidden-{label}-arrow-payload");
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
            "sv 16744448}}{",
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
            .unwrap_or_else(|| panic!("passive {label} arrow shape"));

        assert!(text.contains("Before"));
        assert!(text.contains("After"));
        assert_eq!(shape.kind, StaticShapeKind::Polygon);
        assert_eq!(shape.points.len(), 7);
        assert!(
            shape
                .points
                .iter()
                .any(|point| point.x_twips == expected_tip.0 && point.y_twips == expected_tip.1),
            "{label} arrow should keep its passive arrow-tip point"
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
            payload.as_str(),
            "[Shape skipped",
        ] {
            assert!(
                !text.contains(forbidden),
                "{label} arrow metadata leaked to normalized text: {forbidden}"
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
            "{label} arrow fill/stroke should render passively"
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
                "{label} arrow metadata leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn office_bidirectional_arrows_render_as_passive_polygons_without_payload_leakage() {
    for (shape_type, label, expected_tips) in [
        (37, "left-right", [(0, 540), (2880, 540)]),
        (38, "up-down", [(1440, 0), (1440, 1080)]),
    ] {
        let payload = format!("hidden-{label}-arrow-payload");
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
            .unwrap_or_else(|| panic!("passive {label} arrow shape"));

        assert!(text.contains("Before"));
        assert!(text.contains("After"));
        assert_eq!(shape.kind, StaticShapeKind::Polygon);
        assert_eq!(shape.points.len(), 10);
        for expected_tip in expected_tips {
            assert!(
                shape
                    .points
                    .iter()
                    .any(|point| point.x_twips == expected_tip.0
                        && point.y_twips == expected_tip.1),
                "{label} arrow should keep passive arrow-tip point {expected_tip:?}"
            );
        }
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
                "{label} arrow metadata leaked to normalized text: {forbidden}"
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
            "{label} arrow fill/stroke should render passively"
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
                "{label} arrow metadata leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn office_quad_arrows_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright3600\shpbottom1800{\sp{\sn shapeType}{\sv 39}}{\sp{\sn fillColor}{\sv 65280}}{\sp{\sn lineColor}{\sv 255}}{\sp{\sn pFragments}{\sv hidden-quad-arrow-payload}}}}After\par}"#.to_vec();
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
        .expect("passive quad arrow shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 11);
    for expected_tip in [(1440, 0), (2880, 540), (1440, 1080), (0, 540)] {
        assert!(
            shape
                .points
                .iter()
                .any(|point| point.x_twips == expected_tip.0 && point.y_twips == expected_tip.1),
            "quad arrow should keep passive arrow-tip point {expected_tip:?}"
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
        "hidden-quad-arrow-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "quad arrow metadata leaked to normalized text: {forbidden}"
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
        "quad arrow fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-quad-arrow-payload",
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
            "quad arrow metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_left_right_up_arrows_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright3600\shpbottom1800{\sp{\sn shapeType}{\sv 40}}{\sp{\sn fillColor}{\sv 16744448}}{\sp{\sn lineColor}{\sv 255}}{\sp{\sn pFragments}{\sv hidden-left-right-up-arrow-payload}}}}After\par}"#.to_vec();
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
        .expect("passive left-right-up arrow shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 11);
    for expected_tip in [(1440, 0), (2880, 540), (0, 540)] {
        assert!(
            shape
                .points
                .iter()
                .any(|point| point.x_twips == expected_tip.0 && point.y_twips == expected_tip.1),
            "left-right-up arrow should keep passive arrow-tip point {expected_tip:?}"
        );
    }
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
        "hidden-left-right-up-arrow-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "left-right-up arrow metadata leaked to normalized text: {forbidden}"
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
        "left-right-up arrow fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-left-right-up-arrow-payload",
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
            "left-right-up arrow metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_bent_and_uturn_arrows_render_as_passive_polygons_without_payload_leakage() {
    let mut input = String::from("{\\rtf1 Before\\par");
    for (shape_type, payload) in [
        (41, "hidden-bent-arrow-payload"),
        (42, "hidden-uturn-arrow-payload"),
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
    assert_eq!(shapes.len(), 2);
    for (shape, expected_points) in shapes.iter().zip([11, 23]) {
        assert_eq!(shape.kind, StaticShapeKind::Polygon);
        assert_eq!(shape.points.len(), expected_points);
        assert!(
            shape.points.iter().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "bent/uturn arrow points must stay inside the passive shape frame"
        );
    }
    assert!(
        shapes[0]
            .points
            .iter()
            .any(|point| point.x_twips == shapes[0].width_twips),
        "bent arrow should preserve the right arrow tip"
    );
    assert!(
        shapes[1].points.iter().any(|point| point.x_twips == 0),
        "uturn arrow should preserve the left turn edge"
    );
    assert!(
        shapes[1]
            .points
            .iter()
            .filter(|point| {
                point.x_twips <= shapes[1].width_twips / 5
                    && point.y_twips <= shapes[1].height_twips / 2
            })
            .count()
            >= 6,
        "uturn arrow should preserve a bounded passive rounded outer bend"
    );
    assert!(
        shapes[1]
            .points
            .iter()
            .filter(|point| {
                point.x_twips >= shapes[1].width_twips / 3
                    && point.x_twips <= (shapes[1].width_twips * 4) / 5
                    && point.y_twips >= shapes[1].height_twips / 3
                    && point.y_twips <= (shapes[1].height_twips * 2) / 3
            })
            .count()
            >= 7,
        "uturn arrow should preserve the passive curved arrow shoulder"
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "hidden-bent-arrow-payload",
        "hidden-uturn-arrow-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "bent/uturn arrow metadata leaked to normalized text: {forbidden}"
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
    let mut passive_shape_paints = 0;
    for page_id in parsed_pdf.get_pages().values() {
        let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
        rendered_text.push_str(&decoded_pdf_text(&content));
        passive_shape_paints += content
            .operations
            .iter()
            .filter(|operation| operation.operator == "B")
            .count();
    }

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        passive_shape_paints >= 2,
        "bent/uturn arrows should render passive fill/stroke paths"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-bent-arrow-payload",
        b"hidden-uturn-arrow-payload",
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
            "bent/uturn arrow metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_curved_and_striped_arrows_render_as_passive_polygons_without_payload_leakage() {
    let mut input = String::from("{\\rtf1 Before\\par");
    for (shape_type, payload) in [
        (45, "hidden-curved-right-arrow-payload"),
        (46, "hidden-curved-left-arrow-payload"),
        (47, "hidden-curved-up-arrow-payload"),
        (48, "hidden-curved-down-arrow-payload"),
        (49, "hidden-striped-right-arrow-payload"),
    ] {
        input.push_str(&format!(
            "{{\\shp{{\\*\\shpinst\\shpleft720\\shptop720\\shpright3600\\shpbottom1800{{\\sp{{\\sn shapeType}}{{\\sv {shape_type}}}}}{{\\sp{{\\sn fillColor}}{{\\sv 10040064}}}}{{\\sp{{\\sn lineColor}}{{\\sv 16711680}}}}{{\\sp{{\\sn pFragments}}{{\\sv {payload}}}}}}}}}"
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
    assert_eq!(shapes.len(), 5);
    for (shape, expected_points) in shapes.iter().zip([42, 42, 42, 42, 7]) {
        assert_eq!(shape.kind, StaticShapeKind::Polygon);
        assert_eq!(shape.points.len(), expected_points);
        assert!(
            shape.points.iter().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "curved/striped arrow points must stay inside the passive shape frame"
        );
    }
    assert!(
        shapes[0]
            .points
            .iter()
            .any(|point| point.x_twips == shapes[0].width_twips),
        "curved right arrow should preserve the right arrow tip"
    );
    assert!(
        shapes[1].points.iter().any(|point| point.x_twips == 0),
        "curved left arrow should preserve the left arrow tip"
    );
    assert!(
        shapes[2].points.iter().any(|point| point.y_twips == 0),
        "curved up arrow should preserve the top arrow tip"
    );
    assert!(
        shapes[3]
            .points
            .iter()
            .any(|point| point.y_twips == shapes[3].height_twips),
        "curved down arrow should preserve the bottom arrow tip"
    );
    for (index, shape) in shapes.iter().take(4).enumerate() {
        assert!(
            shape
                .points
                .iter()
                .filter(|point| {
                    point.x_twips >= shape.width_twips / 4
                        && point.x_twips <= (shape.width_twips * 7) / 8
                        && point.y_twips >= shape.height_twips / 8
                        && point.y_twips <= (shape.height_twips * 7) / 8
                })
                .count()
                >= 20,
            "curved arrow {index} should preserve denser bounded passive bend samples"
        );
    }
    assert_eq!(
        shapes[4].overlay_paths.len(),
        2,
        "striped right arrow should preserve passive stripe strokes"
    );
    assert!(
        shapes[4].overlay_paths.iter().all(|path| path.len() == 2),
        "striped right arrow stripes should be passive line segments"
    );
    assert!(
        shapes[4].overlay_paths.iter().flatten().all(|point| {
            point.x_twips >= 0
                && point.x_twips <= shapes[4].width_twips
                && point.y_twips >= 0
                && point.y_twips <= shapes[4].height_twips
        }),
        "striped right arrow stripe strokes must stay inside passive frame"
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "hidden-curved-right-arrow-payload",
        "hidden-curved-left-arrow-payload",
        "hidden-curved-up-arrow-payload",
        "hidden-curved-down-arrow-payload",
        "hidden-striped-right-arrow-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "curved/striped arrow metadata leaked to normalized text: {forbidden}"
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
    let mut passive_shape_paints = 0;
    let mut passive_overlay_strokes = 0;
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
        passive_shape_paints >= 5,
        "curved/striped arrows should render passive fill/stroke paths"
    );
    assert!(
        passive_overlay_strokes >= 2,
        "striped right arrow stripes should render as passive strokes"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-curved-right-arrow-payload",
        b"hidden-curved-left-arrow-payload",
        b"hidden-curved-up-arrow-payload",
        b"hidden-curved-down-arrow-payload",
        b"hidden-striped-right-arrow-payload",
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
            "curved/striped arrow metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_left_up_arrows_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright3600\shpbottom1800{\sp{\sn shapeType}{\sv 43}}{\sp{\sn fillColor}{\sv 10040064}}{\sp{\sn lineColor}{\sv 255}}{\sp{\sn pFragments}{\sv hidden-left-up-arrow-payload}}}}After\par}"#.to_vec();
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
        .expect("passive left-up arrow shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 9);
    for expected_tip in [(1440, 0), (0, 540)] {
        assert!(
            shape
                .points
                .iter()
                .any(|point| point.x_twips == expected_tip.0 && point.y_twips == expected_tip.1),
            "left-up arrow should keep passive arrow-tip point {expected_tip:?}"
        );
    }
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
        "hidden-left-up-arrow-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "left-up arrow metadata leaked to normalized text: {forbidden}"
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
        "left-up arrow fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-left-up-arrow-payload",
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
            "left-up arrow metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_bent_up_arrows_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright3600\shpbottom1800{\sp{\sn shapeType}{\sv 44}}{\sp{\sn fillColor}{\sv 16744448}}{\sp{\sn lineColor}{\sv 255}}{\sp{\sn pFragments}{\sv hidden-bent-up-arrow-payload}}}}After\par}"#.to_vec();
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
        .expect("passive bent-up arrow shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 9);
    assert!(
        shape
            .points
            .iter()
            .any(|point| point.x_twips == shape.width_twips / 2 && point.y_twips == 0),
        "bent-up arrow should keep a passive top arrow-tip point"
    );
    assert!(
        shape
            .points
            .iter()
            .any(|point| point.x_twips == 0 && point.y_twips == shape.height_twips),
        "bent-up arrow should keep a passive lower-left bend corner"
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
        "hidden-bent-up-arrow-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "bent-up arrow metadata leaked to normalized text: {forbidden}"
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
        "bent-up arrow fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-bent-up-arrow-payload",
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
            "bent-up arrow metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_explosion_shapes_render_as_passive_polygons_without_payload_leakage() {
    let mut input = String::from("{\\rtf1 Before\\par");
    for (shape_type, payload) in [
        (89, "hidden-explosion-one-payload"),
        (90, "hidden-explosion-two-payload"),
    ] {
        input.push_str(&format!(
            "{{\\shp{{\\*\\shpinst\\shpleft720\\shptop720\\shpright3600\\shpbottom1800{{\\sp{{\\sn shapeType}}{{\\sv {shape_type}}}}}{{\\sp{{\\sn fillColor}}{{\\sv 65535}}}}{{\\sp{{\\sn lineColor}}{{\\sv 16711680}}}}{{\\sp{{\\sn pFragments}}{{\\sv {payload}}}}}}}}}"
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
    assert_eq!(shapes.len(), 2);
    assert_eq!(shapes[0].kind, StaticShapeKind::Polygon);
    assert_eq!(shapes[0].points.len(), 16);
    assert_eq!(shapes[1].kind, StaticShapeKind::Polygon);
    assert_eq!(shapes[1].points.len(), 18);
    for shape in &shapes {
        assert!(
            shape.points.iter().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "explosion points must stay inside the passive shape frame"
        );
    }
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "hidden-explosion-one-payload",
        "hidden-explosion-two-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "explosion metadata leaked to normalized text: {forbidden}"
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
    let mut passive_shape_paints = 0;
    for page_id in parsed_pdf.get_pages().values() {
        let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
        rendered_text.push_str(&decoded_pdf_text(&content));
        passive_shape_paints += content
            .operations
            .iter()
            .filter(|operation| operation.operator == "B")
            .count();
    }

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        passive_shape_paints >= 2,
        "explosion shapes should render passive fill/stroke paths"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-explosion-one-payload",
        b"hidden-explosion-two-payload",
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
            "explosion metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_four_point_stars_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright3600\shpbottom1800{\sp{\sn shapeType}{\sv 91}}{\sp{\sn fillColor}{\sv 16744448}}{\sp{\sn lineColor}{\sv 255}}{\sp{\sn pFragments}{\sv hidden-four-point-star-payload}}}}After\par}"#.to_vec();
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
        .expect("passive four-point star shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 8);
    for expected_tip in [
        (shape.width_twips / 2, 0),
        (shape.width_twips, shape.height_twips / 2),
        (shape.width_twips / 2, shape.height_twips),
        (0, shape.height_twips / 2),
    ] {
        assert!(
            shape
                .points
                .iter()
                .any(|point| point.x_twips == expected_tip.0 && point.y_twips == expected_tip.1),
            "four-point star should keep passive cardinal outer point {expected_tip:?}"
        );
    }
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
        "hidden-four-point-star-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "four-point star metadata leaked to normalized text: {forbidden}"
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
        "four-point star fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-four-point-star-payload",
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
            "four-point star metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_five_point_stars_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright3600\shpbottom1800{\sp{\sn shapeType}{\sv 92}}{\sp{\sn fillColor}{\sv 65535}}{\sp{\sn lineColor}{\sv 16711680}}{\sp{\sn pFragments}{\sv hidden-five-point-star-payload}}}}After\par}"#.to_vec();
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
        .expect("passive five-point star shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 10);
    for expected_tip in [
        (shape.width_twips / 2, 0),
        (shape.width_twips, (shape.height_twips * 38) / 100),
        ((shape.width_twips * 79) / 100, shape.height_twips),
        ((shape.width_twips * 21) / 100, shape.height_twips),
        (0, (shape.height_twips * 38) / 100),
    ] {
        assert!(
            shape
                .points
                .iter()
                .any(|point| point.x_twips == expected_tip.0 && point.y_twips == expected_tip.1),
            "five-point star should keep passive outer point {expected_tip:?}"
        );
    }
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 255,
            green: 255,
            blue: 0,
        })
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "hidden-five-point-star-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "five-point star metadata leaked to normalized text: {forbidden}"
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
        "five-point star fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-five-point-star-payload",
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
            "five-point star metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_six_point_stars_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright3600\shpbottom1800{\sp{\sn shapeType}{\sv 147}}{\sp{\sn fillColor}{\sv 10040064}}{\sp{\sn lineColor}{\sv 255}}{\sp{\sn pFragments}{\sv hidden-six-point-star-payload}}}}After\par}"#.to_vec();
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
        .expect("passive six-point star shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 12);
    for expected_tip in [
        (shape.width_twips / 2, 0),
        (shape.width_twips / 2, shape.height_twips),
    ] {
        assert!(
            shape
                .points
                .iter()
                .any(|point| point.x_twips == expected_tip.0 && point.y_twips == expected_tip.1),
            "six-point star should keep passive vertical outer point {expected_tip:?}"
        );
    }
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
        "hidden-six-point-star-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "six-point star metadata leaked to normalized text: {forbidden}"
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
        "six-point star fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-six-point-star-payload",
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
            "six-point star metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_seven_point_stars_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright3600\shpbottom1800{\sp{\sn shapeType}{\sv 148}}{\sp{\sn fillColor}{\sv 33023}}{\sp{\sn lineColor}{\sv 16711680}}{\sp{\sn pFragments}{\sv hidden-seven-point-star-payload}}}}After\par}"#.to_vec();
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
        .expect("passive seven-point star shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 14);
    assert!(
        shape
            .points
            .iter()
            .any(|point| point.x_twips == shape.width_twips / 2 && point.y_twips == 0),
        "seven-point star should keep a passive top outer point"
    );
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 255,
            green: 128,
            blue: 0,
        })
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "hidden-seven-point-star-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "seven-point star metadata leaked to normalized text: {forbidden}"
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
        "seven-point star fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-seven-point-star-payload",
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
            "seven-point star metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_eight_point_stars_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright3600\shpbottom1800{\sp{\sn shapeType}{\sv 93}}{\sp{\sn fillColor}{\sv 33023}}{\sp{\sn lineColor}{\sv 16711680}}{\sp{\sn pFragments}{\sv hidden-eight-point-star-payload}}}}After\par}"#.to_vec();
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
        .expect("passive eight-point star shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 16);
    for expected_tip in [
        (shape.width_twips / 2, 0),
        (shape.width_twips, 0),
        (shape.width_twips, shape.height_twips / 2),
        (shape.width_twips, shape.height_twips),
        (shape.width_twips / 2, shape.height_twips),
        (0, shape.height_twips),
        (0, shape.height_twips / 2),
        (0, 0),
    ] {
        assert!(
            shape
                .points
                .iter()
                .any(|point| point.x_twips == expected_tip.0 && point.y_twips == expected_tip.1),
            "eight-point star should keep passive outer point {expected_tip:?}"
        );
    }
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 255,
            green: 128,
            blue: 0,
        })
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "hidden-eight-point-star-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "eight-point star metadata leaked to normalized text: {forbidden}"
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
        "eight-point star fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-eight-point-star-payload",
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
            "eight-point star metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_ten_point_stars_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright3600\shpbottom1800{\sp{\sn shapeType}{\sv 149}}{\sp{\sn fillColor}{\sv 65535}}{\sp{\sn lineColor}{\sv 16711680}}{\sp{\sn pFragments}{\sv hidden-ten-point-star-payload}}}}After\par}"#.to_vec();
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
        .expect("passive ten-point star shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 20);
    assert!(
        shape
            .points
            .iter()
            .any(|point| point.x_twips == shape.width_twips / 2 && point.y_twips == 0),
        "ten-point star should keep a passive top outer point"
    );
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 255,
            green: 255,
            blue: 0,
        })
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "hidden-ten-point-star-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "ten-point star metadata leaked to normalized text: {forbidden}"
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
        "ten-point star fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-ten-point-star-payload",
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
            "ten-point star metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_twelve_point_stars_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright3600\shpbottom1800{\sp{\sn shapeType}{\sv 150}}{\sp{\sn fillColor}{\sv 33023}}{\sp{\sn lineColor}{\sv 16711680}}{\sp{\sn pFragments}{\sv hidden-twelve-point-star-payload}}}}After\par}"#.to_vec();
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
        .expect("passive twelve-point star shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 24);
    assert!(
        shape
            .points
            .iter()
            .any(|point| point.x_twips == shape.width_twips / 2 && point.y_twips == 0),
        "twelve-point star should keep a passive top outer point"
    );
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 255,
            green: 128,
            blue: 0,
        })
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "hidden-twelve-point-star-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "twelve-point star metadata leaked to normalized text: {forbidden}"
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
        "twelve-point star fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-twelve-point-star-payload",
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
            "twelve-point star metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_sixteen_point_stars_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright3600\shpbottom1800{\sp{\sn shapeType}{\sv 94}}{\sp{\sn fillColor}{\sv 10040064}}{\sp{\sn lineColor}{\sv 255}}{\sp{\sn pFragments}{\sv hidden-sixteen-point-star-payload}}}}After\par}"#.to_vec();
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
        .expect("passive sixteen-point star shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 32);
    for expected_tip in [
        (shape.width_twips / 2, 0),
        (shape.width_twips, shape.height_twips / 2),
        (shape.width_twips / 2, shape.height_twips),
        (0, shape.height_twips / 2),
    ] {
        assert!(
            shape
                .points
                .iter()
                .any(|point| point.x_twips == expected_tip.0 && point.y_twips == expected_tip.1),
            "sixteen-point star should keep passive cardinal outer point {expected_tip:?}"
        );
    }
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
        "hidden-sixteen-point-star-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "sixteen-point star metadata leaked to normalized text: {forbidden}"
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
        "sixteen-point star fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-sixteen-point-star-payload",
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
            "sixteen-point star metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_twenty_four_point_stars_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright3600\shpbottom1800{\sp{\sn shapeType}{\sv 95}}{\sp{\sn fillColor}{\sv 16744448}}{\sp{\sn lineColor}{\sv 255}}{\sp{\sn pFragments}{\sv hidden-twenty-four-point-star-payload}}}}After\par}"#.to_vec();
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
        .expect("passive twenty-four-point star shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 48);
    for expected_tip in [
        (shape.width_twips / 2, 0),
        (shape.width_twips, shape.height_twips / 2),
        (shape.width_twips / 2, shape.height_twips),
        (0, shape.height_twips / 2),
    ] {
        assert!(
            shape
                .points
                .iter()
                .any(|point| point.x_twips == expected_tip.0 && point.y_twips == expected_tip.1),
            "twenty-four-point star should keep passive cardinal outer point {expected_tip:?}"
        );
    }
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
        "hidden-twenty-four-point-star-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "twenty-four-point star metadata leaked to normalized text: {forbidden}"
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
        "twenty-four-point star fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-twenty-four-point-star-payload",
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
            "twenty-four-point star metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_thirty_two_point_stars_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright3600\shpbottom1800{\sp{\sn shapeType}{\sv 96}}{\sp{\sn fillColor}{\sv 65280}}{\sp{\sn lineColor}{\sv 16711680}}{\sp{\sn pFragments}{\sv hidden-thirty-two-point-star-payload}}}}After\par}"#.to_vec();
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
        .expect("passive thirty-two-point star shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 64);
    for expected_tip in [
        (shape.width_twips / 2, 0),
        (shape.width_twips, shape.height_twips / 2),
        (shape.width_twips / 2, shape.height_twips),
        (0, shape.height_twips / 2),
    ] {
        assert!(
            shape
                .points
                .iter()
                .any(|point| point.x_twips == expected_tip.0 && point.y_twips == expected_tip.1),
            "thirty-two-point star should keep passive cardinal outer point {expected_tip:?}"
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
        "hidden-thirty-two-point-star-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "thirty-two-point star metadata leaked to normalized text: {forbidden}"
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
        "thirty-two-point star fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-thirty-two-point-star-payload",
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
            "thirty-two-point star metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_notched_right_arrows_render_as_passive_polygon_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright3600\shpbottom1800{\sp{\sn shapeType}{\sv 50}}{\sp{\sn fillColor}{\sv 65280}}{\sp{\sn lineColor}{\sv 16711680}}{\sp{\sn pFragments}{\sv hidden-notched-right-arrow-payload}}}}After\par}"#.to_vec();
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
        .expect("passive notched right arrow shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(shape.points.len(), 8);
    assert!(
        shape
            .points
            .iter()
            .any(|point| point.x_twips == shape.width_twips
                && point.y_twips == shape.height_twips / 2),
        "notched right arrow should keep a passive point at the arrow tip"
    );
    assert!(
        shape
            .points
            .iter()
            .any(|point| point.x_twips == shape.width_twips / 5
                && point.y_twips == shape.height_twips / 2),
        "notched right arrow should keep a passive notch point"
    );
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
        "hidden-notched-right-arrow-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "notched right arrow metadata leaked to normalized text: {forbidden}"
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
        "notched right arrow fill/stroke should render passively"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-notched-right-arrow-payload",
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
            "notched right arrow metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn repeated_office_shape_type_uses_last_passive_preset_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 4}}{\sp{\sn shapeType}{\sv 12}}{\sp{\sn fillColor}{\sv 10040064}}{\sp{\sn pFragments}{\sv hidden-repeated-shapetype-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape from repeated shapeType properties");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Polygon);
    assert_eq!(
        shape.points.len(),
        5,
        "last recognized shapeType should determine the passive polygon preset"
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
        "pFragments",
        "hidden-repeated-shapetype-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "repeated shapeType metadata leaked to normalized text: {forbidden}"
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
        "last passive polygon preset should render filled/stroked output"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"pFragments",
        b"hidden-repeated-shapetype-payload",
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
            "repeated shapeType metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
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
        "dobxcolumn",
        "\\",
        "doby720",
        "\\",
        "dobypara",
        "\\",
        "dodhgt1{",
        "\\",
        "dpxsize3150",
        "\\",
        "dpysize1575",
        "\\",
        "dptxbx{",
        "\\",
        "dptxbxtext Legacy box text {",
        "\\",
        "object",
        "\\",
        "objemb{",
        "\\",
        "*",
        "\\",
        "objclass Word.Picture.8}{",
        "\\",
        "objdata 4142432f4a6176615363726970742f456d62656464656446696c65}}}",
        "\\",
        "par}}{{",
        "\\",
        "dpptx111 hostile-coordinate-payload}}After",
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
        .expect("legacy drawing text box should remain a passive shape");

    assert!(text.contains("Before"));
    assert!(text.contains("Legacy box text"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert_eq!(shape.z_order, 1);
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::Image(_) | Block::Placeholder(_))),
        "active object inside legacy text box should not create body image/placeholder blocks: {:?}",
        parsed.document.blocks
    );
    for forbidden in [
        "dobx",
        "dobxcolumn",
        "doby",
        "dobypara",
        "dodhgt",
        "dpxsize",
        "dpysize",
        "dptxbx",
        "dptxbxtext",
        "objdata",
        "Word.Picture.8",
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
    let report = convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
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
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
    );
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
        b"dobxcolumn",
        b"doby",
        b"dobypara",
        b"dodhgt",
        b"dpxsize",
        b"dpysize",
        b"dptxbx",
        b"dptxbxtext",
        b"objdata",
        b"Word.Picture.8",
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
        "shpbxcolumn",
        "\\",
        "shpbypage",
        "\\",
        "shpbypara",
        "\\",
        "shpwr3",
        "\\",
        "shpwrk0",
        "\\",
        "shpfblwtxt1{",
        "\\",
        "shpfhdr0",
        "\\",
        "shpz0",
        "\\",
        "shplid1026",
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
    assert_eq!(
        shape.horizontal_anchor,
        open_rtf_converter::model::StaticShapeHorizontalAnchor::Column
    );
    assert_eq!(
        shape.vertical_anchor,
        open_rtf_converter::model::StaticShapeVerticalAnchor::Paragraph
    );
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
        "shpbxcolumn",
        "shpbypara",
        "shpfblwtxt",
        "shpfhdr",
        "shplid",
        "shpwr",
        "shpwrk",
        "shpz",
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
        report.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("shape text wrapping approximated")
        }),
        "no-wrap shape should not be reported as a wrapping approximation: {:?}",
        report.diagnostics
    );
    assert!(
        report.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("floating shape anchoring approximated")),
        "supported shape anchors should not be reported as ignored: {:?}",
        report.diagnostics
    );
    assert!(
        report
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("stripping shape properties"))
    );
    assert!(
        report
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
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
        b"shpbxcolumn",
        b"shpbypara",
        b"shpfblwtxt",
        b"shpfhdr",
        b"shplid",
        b"shpwr",
        b"shpwrk",
        b"shpz",
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
fn office_shape_filled_flag_renders_default_passive_fill_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fFilled}{\sv 1}}{\sp{\sn pFragments}{\sv hidden-filled-flag-payload}}}}After\par}"#.to_vec();
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
        .expect("filled passive shape");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 255,
            green: 255,
            blue: 255,
        })
    );
    for forbidden in [
        "shapeType",
        "fFilled",
        "pFragments",
        "hidden-filled-flag-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "filled flag shape metadata leaked to normalized text: {forbidden}"
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
            .any(|operation| operation.operator == "f"),
        "fFilled=1 should render a passive default fill"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fFilled",
        b"pFragments",
        b"hidden-filled-flag-payload",
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
            "filled flag shape metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_inert_fill_metadata_is_consumed_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fHitTestFill}{\sv 1}}{\sp{\sn fNoFillHitTest}{\sv 0}}{\sp{\sn fillShape}{\sv 1}}{\sp{\sn fillUseRect}{\sv 0}}}}After\par}"#.to_vec();
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
        .expect("passive shape with inert fill metadata");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unsupported/active drawing properties")),
        "valid inert fill metadata should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );
    for forbidden in [
        "shapeType",
        "fHitTestFill",
        "fNoFillHitTest",
        "fillShape",
        "fillUseRect",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "inert fill metadata leaked to normalized text: {forbidden}"
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
        b"shapeType".as_slice(),
        b"fHitTestFill",
        b"fNoFillHitTest",
        b"fillShape",
        b"fillUseRect",
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
            "inert fill metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn invalid_office_shape_inert_fill_metadata_is_stripped_without_payload_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fHitTestFill}{\sv not-a-number}}{\sp{\sn pFragments}{\sv hidden-invalid-inert-fill-metadata-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with invalid inert fill metadata");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("stripping unsupported/active drawing properties")
    }));
    for forbidden in [
        "shapeType",
        "fHitTestFill",
        "not-a-number",
        "pFragments",
        "hidden-invalid-inert-fill-metadata-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "invalid inert fill metadata leaked to normalized text: {forbidden}"
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
        b"shapeType".as_slice(),
        b"fHitTestFill",
        b"not-a-number",
        b"pFragments",
        b"hidden-invalid-inert-fill-metadata-payload",
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
            "invalid inert fill metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn disabled_office_shape_fill_remains_hidden_after_color_metadata_without_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fFilled}{\sv 0}}{\sp{\sn fillColor}{\sv 65535}}{\sp{\sn pFragments}{\sv hidden-disabled-fill-color-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with disabled fill and color metadata");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert_eq!(
        shape.fill_color, None,
        "fillColor metadata must not resurrect a disabled passive fill"
    );
    for forbidden in [
        "shapeType",
        "fFilled",
        "fillColor",
        "pFragments",
        "hidden-disabled-fill-color-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "disabled fill color metadata leaked to normalized text: {forbidden}"
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
    let fill_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "f")
        .count();

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert_eq!(
        fill_count, 0,
        "disabled fill should not emit passive fill operators"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fFilled",
        b"fillColor",
        b"pFragments",
        b"hidden-disabled-fill-color-payload",
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
            "disabled fill color metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn disabled_office_shape_fill_remains_hidden_after_fill_fore_color_metadata_without_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fFilled}{\sv 0}}{\sp{\sn fillForeColor}{\sv 65535}}{\sp{\sn pFragments}{\sv hidden-disabled-fill-fore-color-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with disabled fill");

    assert_eq!(
        shape.fill_color, None,
        "fillForeColor metadata must not resurrect a disabled passive fill"
    );
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "shapeType",
        "fFilled",
        "fillForeColor",
        "pFragments",
        "hidden-disabled-fill-fore-color-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "disabled fillForeColor metadata leaked to normalized text: {forbidden}"
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
            .any(|operation| operation.operator == "f"),
        "disabled fill should not emit passive fill"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fFilled",
        b"fillForeColor",
        b"pFragments",
        b"hidden-disabled-fill-fore-color-payload",
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
            "disabled fillForeColor metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn repeated_office_shape_fill_flag_restores_authored_color_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn fillColor}{\sv 65535}}{\sp{\sn fFilled}{\sv 0}}{\sp{\sn fFilled}{\sv 1}}{\sp{\sn pFragments}{\sv hidden-repeated-fill-flag-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape from repeated fill flags");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 255,
            green: 255,
            blue: 0,
        }),
        "later enabled fFilled flag should restore the authored passive fill color"
    );
    for forbidden in [
        "shapeType",
        "fFilled",
        "fillColor",
        "pFragments",
        "hidden-repeated-fill-flag-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "repeated fill flag metadata leaked to normalized text: {forbidden}"
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
            .any(|operation| operation.operator == "f"),
        "enabled fill should emit passive fill operators"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fFilled",
        b"fillColor",
        b"pFragments",
        b"hidden-repeated-fill-flag-payload",
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
            "repeated fill flag metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn neutral_shape_layout_metadata_is_consumed_without_false_approximation_warning() {
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
        "shpwr0",
        "\\",
        "shpwrk0",
        "\\",
        "shpfblwtxt0",
        "\\",
        "shpz0",
        "\\",
        "shpfhdr0",
        "\\",
        "shplid1026{",
        "\\",
        "sp{",
        "\\",
        "sn shapeType}{",
        "\\",
        "sv 1}}}}After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .any(|block| matches!(block, Block::Shape(_))),
        "neutral shape metadata should still leave a passive static shape"
    );
    for forbidden in [
        "shpwr",
        "shpwrk",
        "shpfblwtxt",
        "shpz",
        "shpfhdr",
        "shplid",
        "shapeType",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "neutral shape metadata leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("neutral-shape-layout-metadata.rtf");
    let output_path = dir.path().join("neutral-shape-layout-metadata.pdf");
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
    assert!(
        report
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "neutral shape metadata should be classified: {:?}",
        report.diagnostics
    );
    assert!(
        report.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("shape text wrapping approximated")
                && !diagnostic.message.contains("shape z-order approximated")
                && !diagnostic.message.contains("shape metadata interpreted")
        }),
        "neutral layout metadata should not produce visible approximation warnings: {:?}",
        report.diagnostics
    );

    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    for forbidden in [
        b"shpwr".as_slice(),
        b"shpwrk",
        b"shpfblwtxt",
        b"shpz",
        b"shpfhdr",
        b"shplid",
        b"shapeType",
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
            "neutral shape metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn shape_z_order_controls_are_bounded_and_do_not_leak() {
    let modern = rtf(&[
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
        "shpz99999{",
        "\\",
        "sp{",
        "\\",
        "sn shapeType}{",
        "\\",
        "sv 1}}}}After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&modern).unwrap();
    let modern_shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("modern z-ordered shape");

    assert_eq!(modern_shape.z_order, 65_535);
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("shape z-order clamped")),
        "oversized z-order should be reported as clamped: {:?}",
        parsed.diagnostics
    );
    assert!(
        parsed.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("shape z-order rendered through bounded passive drawing order")
        }),
        "nonzero modern z-order should be reported as applied passive ordering: {:?}",
        parsed.diagnostics
    );

    let legacy = parse_rtf_bytes(&rtf(&[
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
        "dpxsize720",
        "\\",
        "dpysize720",
        "\\",
        "dodhgt17}After",
        "\\",
        "par}",
    ]))
    .unwrap();
    let legacy_shape = legacy
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("legacy z-ordered shape");

    assert_eq!(legacy_shape.z_order, 17);
    assert!(
        legacy.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("shape z-order rendered through bounded passive drawing order")
        }),
        "legacy z-order should be reported as applied passive ordering: {:?}",
        legacy.diagnostics
    );
    let image_hex = bytes_to_hex(&minimal_jpeg_with_dimensions(1, 1));
    let shape_picture = parse_rtf_bytes(&rtf(&[
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
        "shpleft0",
        "\\",
        "shptop0",
        "\\",
        "shpright720",
        "\\",
        "shpbottom720",
        "\\",
        "shpz23{",
        "\\",
        "shppict{",
        "\\",
        "pict",
        "\\",
        "jpegblip ",
        image_hex.as_str(),
        "}}}}}",
    ]))
    .unwrap();
    let picture_z_order = shape_picture
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => image.placement.map(|placement| placement.z_order),
            _ => None,
        })
        .expect("shape picture placement z-order");

    assert_eq!(picture_z_order, 23);
    assert!(
        shape_picture.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("shape z-order rendered through bounded passive drawing order")
        }),
        "shape picture z-order should be reported as applied passive ordering: {:?}",
        shape_picture.diagnostics
    );
    for text in [
        collect_text(&parsed.document),
        collect_text(&legacy.document),
        collect_text(&shape_picture.document),
    ] {
        for forbidden in ["shpz", "dodhgt", "shapeType", "[Shape skipped"] {
            assert!(
                !text.contains(forbidden),
                "shape z-order control leaked to text: {forbidden}"
            );
        }
    }

    let output = convert_rtf_to_pdf(
        &modern,
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
            .all(|diagnostic| !diagnostic.message.contains("shape z-order approximated")),
        "nonzero normalized z-order should not be reported as ignored: {:?}",
        output.diagnostics
    );
    assert!(
        output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("shape z-order rendered through bounded passive drawing order")
        }),
        "PDF conversion should report z-order as bounded passive ordering: {:?}",
        output.diagnostics
    );
    for forbidden in [
        b"shpz".as_slice(),
        b"dodhgt",
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
            "shape z-order control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn z_ordered_shape_text_renders_passively_without_control_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright4320\shpbottom1800\shpz23{\sp{\sn shapeType}{\sv 1}}{\sp{\sn pFragments}{\sv hidden-z-text-payload}}}{\shptxt Layered Box\par}}After\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before"));
    assert!(text.contains("Layered Box"));
    assert!(text.contains("After"));
    for forbidden in ["shpz", "shapeType", "pFragments", "hidden-z-text-payload"] {
        assert!(
            !text.contains(forbidden),
            "z-ordered shape control leaked to normalized text: {forbidden}"
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
        rendered_text.contains("Layered Box"),
        "z-ordered shape text should render through passive PDF text operations: {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("Before") && rendered_text.contains("After"),
        "body text around z-ordered shape should still render: {rendered_text:?}"
    );
    for forbidden in [
        b"shpz".as_slice(),
        b"shapeType",
        b"pFragments",
        b"hidden-z-text-payload",
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
            "z-ordered shape text leaked controls or active PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn shape_below_text_controls_are_normalized_without_leakage() {
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
        "shpfblwtxt1{",
        "\\",
        "sp{",
        "\\",
        "sn shapeType}{",
        "\\",
        "sv 1}}}}After",
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
        .expect("below-text shape");

    assert!(shape.below_text);
    assert!(
        parsed.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("shape below-text flag rendered through bounded passive drawing order")
        }),
        "below-text shape should report passive drawing order: {:?}",
        parsed.diagnostics
    );

    let image_hex = bytes_to_hex(&minimal_jpeg_with_dimensions(1, 1));
    let shape_picture = parse_rtf_bytes(&rtf(&[
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
        "shpleft0",
        "\\",
        "shptop0",
        "\\",
        "shpright720",
        "\\",
        "shpbottom720",
        "\\",
        "shpfblwtxt1{",
        "\\",
        "shppict{",
        "\\",
        "pict",
        "\\",
        "jpegblip ",
        image_hex.as_str(),
        "}}}}}",
    ]))
    .unwrap();
    let placement_below_text = shape_picture
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => image.placement.map(|placement| placement.below_text),
            _ => None,
        })
        .expect("shape picture below-text placement");

    assert!(placement_below_text);
    assert!(
        shape_picture.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("shape below-text flag rendered through bounded passive drawing order")
        }),
        "below-text shape picture should report passive drawing order: {:?}",
        shape_picture.diagnostics
    );
    for text in [
        collect_text(&parsed.document),
        collect_text(&shape_picture.document),
    ] {
        for forbidden in ["shpfblwtxt", "shapeType", "shppict", "[Shape skipped"] {
            assert!(
                !text.contains(forbidden),
                "shape below-text control leaked to text: {forbidden}"
            );
        }
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
        output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("shape text wrapping approximated")
        }),
        "below-text shape ordering should not be reported as an ignored wrap approximation: {:?}",
        output.diagnostics
    );
    assert!(
        output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("shape below-text flag rendered through bounded passive drawing order")
        }),
        "PDF conversion should report below-text as bounded passive ordering: {:?}",
        output.diagnostics
    );
    for forbidden in [
        b"shpfblwtxt".as_slice(),
        b"shapeType",
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
            "shape below-text control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn unsupported_shape_wrap_modes_warn_as_bounded_passive_layout_without_payload_leakage() {
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
        "shpwr4",
        "\\",
        "shpwrk9{",
        "\\",
        "sp{",
        "\\",
        "sn shapeType}{",
        "\\",
        "sv 1}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hidden-wrap-payload}}}}After",
        "\\",
        "par}",
    ]);

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .any(|block| matches!(block, Block::Shape(_))),
        "unsupported wrap metadata should still leave a safe passive shape"
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "shape contour wrapping interpreted through bounded passive bounding-box exclusion",
        )
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("shape text wrapping interpreted through bounded passive shape layout")
    }));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("shape contour wrapping approximated")
            && !diagnostic
                .message
                .contains("shape text wrapping approximated")
    }));
    for forbidden in [
        "shpwr",
        "shpwrk",
        "shapeType",
        "pFragments",
        "hidden-wrap-payload",
    ] {
        assert!(
            !text.contains(forbidden),
            "unsupported shape wrap metadata leaked to text: {forbidden}"
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
    for forbidden in [
        b"shpwr".as_slice(),
        b"shpwrk",
        b"shapeType",
        b"pFragments",
        b"hidden-wrap-payload",
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
            "unsupported shape wrap metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn square_wrapped_static_shape_excludes_following_text_without_payload_leakage() {
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
        "shpright2160",
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
        "shpfblwtxt0{",
        "\\",
        "sp{",
        "\\",
        "sn shapeType}{",
        "\\",
        "sv 1}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hidden-static-wrap-payload}}}} Wrapped static shape text",
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
        .expect("static wrapped shape");
    assert!(shape.text_wrap);
    assert_eq!(shape.wrap_side, StaticImageWrapSide::Both);

    let layout = LayoutEngine::layout(&parsed.document);
    let page = &layout.pages[0];
    let shape_right = page
        .items
        .iter()
        .filter_map(|item| match item {
            LayoutItem::Line { x1, x2, .. } => Some(x1.max(*x2)),
            _ => None,
        })
        .fold(0.0_f32, f32::max);
    let wrapped_text = page
        .items
        .iter()
        .find_map(|item| match item {
            LayoutItem::Text(text) if text.text.contains("Wrapped") => Some(text),
            _ => None,
        })
        .expect("wrapped text");
    assert!(
        wrapped_text.x > shape_right,
        "following text should be shifted beside the passive static shape: text {}, shape right {}",
        wrapped_text.x,
        shape_right
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    audit_passive_pdf_bytes(&output.pdf).unwrap();
    assert!(output.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("shape text wrapping approximated")
    }));
    for forbidden in [
        b"shpwr".as_slice(),
        b"shpwrk",
        b"shapeType",
        b"pFragments",
        b"hidden-static-wrap-payload",
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
            "static shape wrap metadata leaked to PDF: {:?}",
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
fn old_drawing_background_fill_renders_when_foreground_is_absent() {
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
        "dpfillbgcr40",
        "\\",
        "dpfillbgcg50",
        "\\",
        "dpfillbgcb60",
        "{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-background-fill-payload}}}After",
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
        .expect("background-fill shape");
    let color = shape.fill_color.expect("background fill color");
    let text = collect_text(&parsed.document);

    assert_eq!(color.red, 40);
    assert_eq!(color.green, 50);
    assert_eq!(color.blue, 60);
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "dpfillbg",
        "pFragments",
        "hostile-background-fill-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden background fill drawing content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("old-drawing-background-fill-shape.rtf");
    let output_path = dir.path().join("old-drawing-background-fill-shape.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
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
        b"dpfillbg".as_slice(),
        b"pFragments",
        b"hostile-background-fill-payload",
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
            "forbidden background fill drawing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn old_drawing_foreground_fill_takes_precedence_over_background_fill() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "do",
        "\\",
        "dprect",
        "\\",
        "dpxsize1440",
        "\\",
        "dpysize720",
        "\\",
        "dpfillfgcr10",
        "\\",
        "dpfillfgcg20",
        "\\",
        "dpfillfgcb30",
        "\\",
        "dpfillbgcr200",
        "\\",
        "dpfillbgcg210",
        "\\",
        "dpfillbgcb220}Visible",
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
        .expect("foreground-fill shape");
    let color = shape.fill_color.expect("foreground fill color");

    assert_eq!(color.red, 10);
    assert_eq!(color.green, 20);
    assert_eq!(color.blue, 30);
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
    let parsed = parse_rtf_bytes_strict(&input);
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
            parse_options: RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
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
fn office_shape_diamond_arrowhead_renders_passively_without_property_leakage() {
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
        "sn lineEndArrowhead}{",
        "\\",
        "sv diamond}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-diamond-arrow-payload}}}After",
        "\\",
        "par}",
    ]);
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
    let parsed = parse_rtf_bytes_strict(&input);
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("diamond arrowhead line shape");
    let text = collect_text(&parsed.document);

    assert_eq!(shape.end_arrowhead, StaticShapeArrowhead::Diamond);
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
        "lineEndArrowhead",
        "diamond",
        "pFragments",
        "hostile-diamond-arrow-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden diamond arrowhead content leaked to text: {forbidden}"
        );
    }
    let layout = LayoutEngine::layout(&parsed.document);
    assert!(
        layout
            .pages
            .iter()
            .flat_map(|page| page.items.iter())
            .any(|item| matches!(
                item,
                LayoutItem::Polygon {
                    points,
                    fill_color: Some(_),
                    ..
                } if points.len() == 4
            )),
        "diamond arrowhead should render as passive filled four-point geometry"
    );
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "B"),
        "diamond arrowhead should emit passive fill-and-stroke geometry"
    );
    for forbidden in [
        b"lineEndArrowhead".as_slice(),
        b"pFragments",
        b"hostile-diamond-arrow-payload",
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
            "forbidden diamond arrowhead content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_oval_arrowhead_renders_passively_without_property_leakage() {
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
        "sn lineEndArrowhead}{",
        "\\",
        "sv oval}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-oval-arrow-payload}}}After",
        "\\",
        "par}",
    ]);
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
    let parsed = parse_rtf_bytes_strict(&input);
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("oval arrowhead line shape");
    let text = collect_text(&parsed.document);

    assert_eq!(shape.end_arrowhead, StaticShapeArrowhead::Oval);
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
        "lineEndArrowhead",
        "oval",
        "pFragments",
        "hostile-oval-arrow-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden oval arrowhead content leaked to text: {forbidden}"
        );
    }
    let layout = LayoutEngine::layout(&parsed.document);
    assert!(
        layout
            .pages
            .iter()
            .flat_map(|page| page.items.iter())
            .any(|item| matches!(
                item,
                LayoutItem::Polygon {
                    points,
                    fill_color: Some(_),
                    ..
                } if points.len() == 24
            )),
        "oval arrowhead should render as dense passive filled polygonal oval geometry"
    );
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "B"),
        "oval arrowhead should emit passive fill-and-stroke geometry"
    );
    for forbidden in [
        b"lineEndArrowhead".as_slice(),
        b"pFragments",
        b"hostile-oval-arrow-payload",
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
            "forbidden oval arrowhead content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_stealth_arrowhead_renders_passively_without_property_leakage() {
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
        "sn lineEndArrowhead}{",
        "\\",
        "sv msoArrowheadStealth}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-stealth-arrow-payload}}}After",
        "\\",
        "par}",
    ]);
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
    let parsed = parse_rtf_bytes_strict(&input);
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("stealth arrowhead line shape");
    let text = collect_text(&parsed.document);

    assert_eq!(shape.end_arrowhead, StaticShapeArrowhead::Stealth);
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
        "lineEndArrowhead",
        "msoArrowheadStealth",
        "pFragments",
        "hostile-stealth-arrow-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden stealth arrowhead content leaked to text: {forbidden}"
        );
    }
    let layout = LayoutEngine::layout(&parsed.document);
    assert!(
        layout
            .pages
            .iter()
            .flat_map(|page| page.items.iter())
            .any(|item| matches!(
                item,
                LayoutItem::Polygon {
                    points,
                    fill_color: Some(_),
                    ..
                } if points.len() == 6
            )),
        "stealth arrowhead should render as passive filled swept geometry"
    );
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "B"),
        "stealth arrowhead should emit passive fill-and-stroke geometry"
    );
    for forbidden in [
        b"lineEndArrowhead".as_slice(),
        b"pFragments",
        b"hostile-stealth-arrow-payload",
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
            "forbidden stealth arrowhead content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_block_arrowhead_renders_passively_without_property_leakage() {
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
        "sn lineEndArrowhead}{",
        "\\",
        "sv msoArrowheadBlock}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-block-arrow-payload}}}After",
        "\\",
        "par}",
    ]);
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
    let parsed = parse_rtf_bytes_strict(&input);
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("block arrowhead line shape");
    let text = collect_text(&parsed.document);

    assert_eq!(shape.end_arrowhead, StaticShapeArrowhead::Block);
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
        "lineEndArrowhead",
        "msoArrowheadBlock",
        "pFragments",
        "hostile-block-arrow-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden block arrowhead content leaked to text: {forbidden}"
        );
    }
    let layout = LayoutEngine::layout(&parsed.document);
    assert!(
        layout
            .pages
            .iter()
            .flat_map(|page| page.items.iter())
            .any(|item| matches!(
                item,
                LayoutItem::Polygon {
                    points,
                    fill_color: Some(_),
                    ..
                } if points.len() == 5
            )),
        "block arrowhead should render as passive filled broad wedge geometry"
    );
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "B"),
        "block arrowhead should emit passive fill-and-stroke geometry"
    );
    for forbidden in [
        b"lineEndArrowhead".as_slice(),
        b"pFragments",
        b"hostile-block-arrow-payload",
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
            "forbidden block arrowhead content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_arrowhead_size_renders_passively_without_property_leakage() {
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
        "sn lineEndArrowhead}{",
        "\\",
        "sv triangle}}{",
        "\\",
        "sp{",
        "\\",
        "sn lineEndArrowWidth}{",
        "\\",
        "sv wide}}{",
        "\\",
        "sp{",
        "\\",
        "sn lineEndArrowLength}{",
        "\\",
        "sv long}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-arrow-size-payload}}}After",
        "\\",
        "par}",
    ]);
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
    let parsed = parse_rtf_bytes_strict(&input);
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("sized arrowhead line shape");
    let text = collect_text(&parsed.document);

    assert_eq!(shape.end_arrowhead_width_percent, 135);
    assert_eq!(shape.end_arrowhead_length_percent, 135);
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
        "lineEndArrowhead",
        "lineEndArrowWidth",
        "lineEndArrowLength",
        "triangle",
        "wide",
        "long",
        "pFragments",
        "hostile-arrow-size-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden arrowhead size content leaked to text: {forbidden}"
        );
    }
    let layout = LayoutEngine::layout(&parsed.document);
    assert!(
        layout
            .pages
            .iter()
            .flat_map(|page| page.items.iter())
            .any(|item| matches!(
                item,
                LayoutItem::Polygon {
                    fill_color: Some(_),
                    ..
                }
            )),
        "sized triangle arrowhead should render as passive filled geometry"
    );
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "B"),
        "sized triangle arrowhead should emit passive fill-and-stroke geometry"
    );
    for forbidden in [
        b"lineEndArrowWidth".as_slice(),
        b"lineEndArrowLength",
        b"pFragments",
        b"hostile-arrow-size-payload",
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
            "forbidden arrowhead size content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn invalid_office_shape_arrowhead_size_is_stripped_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 20}}{\sp{\sn lineEndArrowhead}{\sv triangle}}{\sp{\sn lineEndArrowWidth}{\sv active-arrow-size-payload}}}}After\par}"#.to_vec();
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
        .expect("invalid arrowhead size line shape");
    let text = collect_text(&parsed.document);

    assert_eq!(shape.end_arrowhead_width_percent, 100);
    assert!(
        output.diagnostics.iter().any(|warning| warning
            .message
            .contains("unsupported/active drawing properties")),
        "invalid Office arrowhead size should be stripped and diagnosed: {:?}",
        output.diagnostics
    );
    for forbidden in [
        "lineEndArrowWidth",
        "active-arrow-size-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden invalid arrowhead size content leaked to text: {forbidden}"
        );
    }
    for forbidden in [
        b"lineEndArrowWidth".as_slice(),
        b"active-arrow-size-payload",
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
            "forbidden invalid arrowhead size content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_named_arrowhead_aliases_render_passively() {
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
        "sv msoArrowheadOpen}}{",
        "\\",
        "sp{",
        "\\",
        "sn lineEndArrowhead}{",
        "\\",
        "sv msoArrowheadTriangle}}}After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes_strict(&input);
    let shape = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .expect("named arrowhead line shape");
    let text = collect_text(&parsed.document);

    assert_eq!(shape.start_arrowhead, StaticShapeArrowhead::Open);
    assert_eq!(shape.end_arrowhead, StaticShapeArrowhead::Triangle);
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unsupported/active drawing properties")),
        "named arrowhead aliases should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );
    for forbidden in [
        "lineStartArrowhead",
        "lineEndArrowhead",
        "msoArrowheadOpen",
        "msoArrowheadTriangle",
    ] {
        assert!(
            !text.contains(forbidden),
            "named arrowhead metadata leaked to normalized text: {forbidden}"
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
    audit_passive_pdf_bytes(&output.pdf).unwrap();
    for forbidden in [
        b"lineStartArrowhead".as_slice(),
        b"lineEndArrowhead",
        b"msoArrowheadOpen",
        b"msoArrowheadTriangle",
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
            "named arrowhead metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_shape_numeric_arrowhead_styles_render_passively_without_property_leakage() {
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
        "sv 1}}{",
        "\\",
        "sp{",
        "\\",
        "sn lineEndArrowhead}{",
        "\\",
        "sv 3}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-numeric-open-arrow-payload}}}{",
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
        "sn lineStartArrowhead}{",
        "\\",
        "sv 2}}{",
        "\\",
        "sp{",
        "\\",
        "sn lineEndArrowhead}{",
        "\\",
        "sv 1}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-numeric-triangle-arrow-payload}}}After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes_strict(&input);
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
    assert_eq!(shapes[0].start_arrowhead, StaticShapeArrowhead::None);
    assert_eq!(shapes[0].end_arrowhead, StaticShapeArrowhead::Open);
    assert_eq!(shapes[1].start_arrowhead, StaticShapeArrowhead::Triangle);
    assert_eq!(shapes[1].end_arrowhead, StaticShapeArrowhead::None);
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "lineStartArrowhead",
        "lineEndArrowhead",
        "pFragments",
        "hostile-numeric-open-arrow-payload",
        "hostile-numeric-triangle-arrow-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden numeric arrowhead drawing content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("office-shape-numeric-arrowheads.rtf");
    let output_path = dir.path().join("office-shape-numeric-arrowheads.pdf");
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
            >= 4
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
        b"pFragments",
        b"hostile-numeric-open-arrow-payload",
        b"hostile-numeric-triangle-arrow-payload",
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
            "forbidden numeric arrowhead drawing content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn unsupported_office_shape_arrowheads_are_stripped_without_payload_leakage() {
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
        "sv weirdArrow}}{",
        "\\",
        "sp{",
        "\\",
        "sn lineEndArrowhead}{",
        "\\",
        "sv filled}}{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-unsupported-arrow-payload}}}After",
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
        .expect("line shape with unsupported arrowheads");
    let text = collect_text(&parsed.document);

    assert_eq!(shape.start_arrowhead, StaticShapeArrowhead::None);
    assert_eq!(shape.end_arrowhead, StaticShapeArrowhead::None);
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    for forbidden in [
        "lineStartArrowhead",
        "lineEndArrowhead",
        "weirdArrow",
        "filled",
        "pFragments",
        "hostile-unsupported-arrow-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden unsupported arrowhead drawing content leaked to text: {forbidden}"
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
        "line should still render without unsupported arrowheads"
    );
    assert!(
        !content
            .operations
            .iter()
            .any(|operation| operation.operator == "B"),
        "unsupported filled arrowheads must not be approximated as triangles"
    );
    for forbidden in [
        b"lineStartArrowhead".as_slice(),
        b"lineEndArrowhead",
        b"pFragments",
        b"hostile-unsupported-arrow-payload",
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
            "forbidden unsupported arrowhead drawing content leaked to PDF: {:?}",
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
        "dppolycount3",
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
fn old_drawing_fill_patterns_render_passively_without_control_leakage() {
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
        "dpfillfgcr10",
        "\\",
        "dpfillfgcg20",
        "\\",
        "dpfillfgcb30",
        "\\",
        "dpfillpat14{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-dark-horizontal-fill-payload}}}{",
        "\\",
        "do",
        "\\",
        "dprect",
        "\\",
        "dpx360",
        "\\",
        "dpy1440",
        "\\",
        "dpxsize1440",
        "\\",
        "dpysize720",
        "\\",
        "dplinew30",
        "\\",
        "dpfillfgcr200",
        "\\",
        "dpfillfgcg210",
        "\\",
        "dpfillfgcb220",
        "\\",
        "dpfillpat24{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-cross-fill-payload}}}After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let shapes: Vec<_> = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .collect();

    assert_eq!(shapes.len(), 2);
    assert_eq!(shapes[0].fill_pattern, ShadingPattern::DarkHorizontal);
    assert_eq!(shapes[1].fill_pattern, ShadingPattern::Cross);
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "old drawing fill patterns should be consumed without unsupported-control noise: {:?}",
        parsed.diagnostics
    );
    for forbidden in [
        "dprect",
        "dpfillpat",
        "dpfillfg",
        "pFragments",
        "hostile-dark-horizontal-fill-payload",
        "hostile-cross-fill-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "old drawing fill pattern metadata leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "S")
            .count()
            >= 6,
        "old drawing hatch fills should emit bounded passive hatch strokes"
    );
    for forbidden in [
        b"dprect".as_slice(),
        b"dpfillpat",
        b"dpfillfg",
        b"pFragments",
        b"hostile-dark-horizontal-fill-payload",
        b"hostile-cross-fill-payload",
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
            "old drawing fill pattern metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn old_drawing_without_explicit_kind_defaults_to_passive_rectangle_without_property_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before",
        "\\",
        "par{",
        "\\",
        "do",
        "\\",
        "dpx360",
        "\\",
        "dpy480",
        "\\",
        "dpxsize1440",
        "\\",
        "dpysize720",
        "\\",
        "dpfillfgcr255",
        "\\",
        "dpfillfgcg255",
        "\\",
        "dpfillfgcb255",
        "\\",
        "dplinehollow}After",
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
        .expect("passive default rectangle old drawing");

    assert_eq!(
        shape.kind,
        open_rtf_converter::model::StaticShapeKind::Rectangle
    );
    assert_eq!(shape.width_twips, 1440);
    assert_eq!(shape.height_twips, 720);
    assert_eq!(
        shape.fill_color,
        Some(open_rtf_converter::model::Color {
            red: 255,
            green: 255,
            blue: 255,
        })
    );
    assert_eq!(shape.stroke_width_twips, 0);
    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unsupported/active drawing properties")),
        "default old drawing rectangle should not strip safe drawing metadata: {:?}",
        parsed.diagnostics
    );
    for forbidden in [
        "dpxsize",
        "dpysize",
        "dpfillfg",
        "dplinehollow",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "old drawing default rectangle metadata leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "default old drawing rectangle should render as passive PDF rectangle geometry"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "f"),
        "hollow-line default rectangle should still render its passive fill"
    );
    assert!(
        !content
            .operations
            .iter()
            .any(|operation| operation.operator == "S" || operation.operator == "B"),
        "hollow-line default rectangle should not render an outline stroke"
    );
    for forbidden in [
        b"dpxsize".as_slice(),
        b"dpysize",
        b"dpfillfg",
        b"dplinehollow",
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
            "old drawing default rectangle metadata leaked to PDF: {:?}",
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
