use super::*;

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
fn raw_text_after_stray_ignorable_marker_does_not_hide_later_active_destination() {
    let input = br"{\rtf1{\* raw visible \object\objdata 414243}After\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("raw visible"));
    assert!(text.contains("[Embedded object removed]"));
    assert!(text.contains("After"));
    for forbidden in ["objdata", "414243"] {
        assert!(
            !text.contains(forbidden),
            "object payload leaked to normalized text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("ignorable destination marker without a following destination control")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("active content removed: OLE object before safe model normalization")
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

    assert!(rendered_text.contains("raw visible"));
    assert!(rendered_text.contains("[Embedded object removed]"));
    assert!(rendered_text.contains("After"));
    for forbidden in [
        b"objdata".as_slice(),
        b"414243",
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
            "forbidden stale-ignorable object content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
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
fn unsafe_only_object_result_falls_back_to_passive_placeholder_without_payload_leakage() {
    let input = br#"{\rtf1 Before {\object\objw2160\objh720\objemb{\objdata 4142432f4a6176615363726970742f456d62656464656446696c65}{\result{\object\objdata 4445462f4a6176615363726970742f456d62656464656446696c65}{\field{\*\fldinst HYPERLINK "https://example.com/result"}}}} After\par}"#.to_vec();
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
    assert_eq!(images.len(), 1, "text={text:?}, images={images:?}");
    assert_eq!(images[0].format, ImageFormat::Placeholder);
    assert!(images[0].bytes.is_empty());
    assert_eq!(images[0].width_px, 144);
    assert_eq!(images[0].height_px, 48);
    assert_eq!(images[0].natural_width_px_hint, Some(144));
    assert_eq!(images[0].natural_height_px_hint, Some(48));
    assert_eq!(images[0].display_width_twips, Some(2160));
    assert_eq!(images[0].display_height_twips, Some(720));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("resultless embedded object rendered as bounded passive placeholder")
    }));
    for forbidden in [
        "objdata",
        "444546",
        "414243",
        "JavaScript",
        "EmbeddedFile",
        "HYPERLINK",
        "https://example.com/result",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "unsafe-only object result leaked to normalized text: {forbidden}"
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
        b"444546",
        b"414243",
        b"JavaScript",
        b"EmbeddedFile",
        b"HYPERLINK",
        b"https://example.com/result",
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
            "unsafe-only object result leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_object_without_dimensions_reports_passive_text_placeholder() {
    let input =
        br"{\rtf1 Before {\object\objemb{\objdata 4142432f4a617661536372697074}} After\par}"
            .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before [Embedded object removed] After"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("resultless embedded object rendered as passive text placeholder")
    }));
    for forbidden in ["objdata", "414243", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "resultless object payload leaked to normalized text: {forbidden}"
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

    assert!(rendered_text.contains("[Embedded object removed]"));
    for forbidden in [
        b"objdata".as_slice(),
        b"414243",
        b"JavaScript",
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
            "resultless object payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn mixed_object_result_keeps_visible_fallback_and_strips_nested_active_payload() {
    let input = br#"{\rtf1 Before {\object\objemb{\objdata 4142432f4a617661536372697074}{\result visible fallback {\object\objdata 4445462f456d62656464656446696c65}{\field{\*\fldinst HYPERLINK "https://example.com/result"}} tail}} After\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before"));
    assert!(
        text.contains("visible fallback  tail"),
        "unexpected normalized text: {text:?}"
    );
    assert!(text.contains("After"));
    assert!(!text.contains("[Embedded object removed]"));
    for forbidden in [
        "objdata",
        "444546",
        "414243",
        "JavaScript",
        "EmbeddedFile",
        "HYPERLINK",
        "https://example.com/result",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "mixed object result leaked active content to text: {forbidden}"
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
    assert!(rendered_text.contains("visible fallback  tail"));
    assert!(rendered_text.contains("After"));
    assert!(!rendered_text.contains("[Embedded object removed]"));
    for forbidden in [
        b"objdata".as_slice(),
        b"444546",
        b"414243",
        b"JavaScript",
        b"EmbeddedFile",
        b"HYPERLINK",
        b"https://example.com/result",
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
            "mixed object result leaked active content to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
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
fn active_shape_picture_metadata_is_stripped_without_property_leakage() {
    let input = br#"{\rtf1 Before\par{\shp{\*\shpinst\shpleft720\shptop720\shpright2880\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn pictureActive}{\sv 1}}{\sp{\sn pFragments}{\sv hidden-active-shape-picture-payload}}}}After\par}"#.to_vec();
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
        .expect("passive shape with active picture metadata");

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shape.kind, StaticShapeKind::Rectangle);
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("shape picture activation metadata stripped")
    }));
    for forbidden in [
        "pictureActive",
        "pFragments",
        "hidden-active-shape-picture-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "active shape picture metadata leaked to normalized text: {forbidden}"
        );
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
        b"pictureActive".as_slice(),
        b"pFragments",
        b"hidden-active-shape-picture-payload",
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
            "active shape picture metadata leaked to PDF: {:?}",
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
fn symbol_font_switch_rejects_active_marker_text_before_pdf() {
    let input =
        br#"{\rtf1 Before {\field{\*\fldinst SYMBOL 65 \\f "/JavaScript"}} After\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains("Before A After"),
        "hostile SYMBOL font switch should be ignored while preserving the passive value, got {text:?}"
    );
    for forbidden in ["fldinst", "SYMBOL", "/JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "forbidden hostile-symbol font switch content leaked to text: {forbidden}"
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

    assert!(rendered_text.contains("Before A After"));
    for forbidden in [
        b"SYMBOL".as_slice(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden hostile-symbol font switch content leaked to PDF: {:?}",
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
fn office_math_fraction_reference_uses_word_inline_object_lead_in() {
    let input = include_bytes!("../../fixtures/office-math-passive.rtf");
    let mut options = ConvertOptions::browser_safe_defaults();
    options.parse_options.compatibility_mode = CompatibilityMode::WordCompatiblePassive;
    let output = convert_rtf_to_pdf(input, &options).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    let numerator = pdf_first_text_position_for_text(&content, "x").expect("fraction numerator");
    let following =
        pdf_first_text_position_for_text(&content, "after").expect("text after fraction");
    assert!(
        (numerator.0 - 174.44).abs() < 0.15,
        "stacked fraction should retain Word's inline object lead-in: {numerator:?}"
    );
    assert!(
        (following.0 - 193.51).abs() < 0.15,
        "text after stacked fraction should resume at the Word-compatible edge: {following:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mnum",
        b"mden",
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
            "Office Math or object payload leaked to PDF: {:?}",
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
    assert_eq!(lower_limit_style.baseline_shift_half_points, -15);
    assert_eq!(lower_limit_style.font_size_scale_percent, 71);
    assert_eq!(
        lower_limit_style.passive_math.limit_part,
        PassiveMathLimitPart::Lower
    );
    let lower_base_style = run_style_for_text(&parsed.document, "lim").expect("lower base run");
    assert_eq!(
        lower_base_style.passive_math.limit_part,
        PassiveMathLimitPart::Base
    );
    assert_eq!(
        lower_base_style.passive_math.limit_id,
        lower_limit_style.passive_math.limit_id
    );
    let upper_limit_style = run_style_for_text(&parsed.document, "n").expect("upper limit run");
    assert_eq!(upper_limit_style.baseline_shift_half_points, 17);
    assert_eq!(upper_limit_style.font_size_scale_percent, 71);
    assert_eq!(
        upper_limit_style.passive_math.limit_part,
        PassiveMathLimitPart::Upper
    );
    let upper_base_style = run_style_for_text(&parsed.document, "max").expect("upper base run");
    assert_eq!(
        upper_base_style.passive_math.limit_part,
        PassiveMathLimitPart::Base
    );
    assert_eq!(
        upper_base_style.passive_math.limit_id,
        upper_limit_style.passive_math.limit_id
    );
    assert_ne!(
        lower_limit_style.passive_math.limit_id,
        upper_limit_style.passive_math.limit_id
    );
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
    let lower_limit = pdf_first_text_position_for_text(&content, "x").expect("lower limit");
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
    assert!(
        (lower_limit.0 - lower_base.0).abs() <= 2.0,
        "Office math lower limit should be horizontally centered under the base text: base={lower_base:?}, limit={lower_limit:?}"
    );
    assert!(
        (upper_limit.0 - upper_base.0 - 8.2).abs() <= 1.0,
        "Office math upper limit should be horizontally centered over the base text: base={upper_base:?}, limit={upper_limit:?}"
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
fn wmf_createpalette_is_inert_object_without_payload_leakage() {
    let mut palette = wmf_create_palette_record(&[(255, 0, 0), (0, 255, 0)]);
    palette.extend_from_slice(b"WMF-CREATEPALETTE /JavaScript /EmbeddedFile /OpenAction");
    palette.resize(palette.len().next_multiple_of(2), 0);
    let palette_len = (palette.len() / 2) as u32;
    write_test_le_u32(&mut palette, 0, palette_len);

    let records = [
        palette,
        wmf_u16_record(0x0234, 0),
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
        .expect("passive WMF CREATEPALETTE vector image");

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
        "WMF-CREATEPALETTE",
        "JavaScript",
        "EmbeddedFile",
        "OpenAction",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF CREATEPALETTE payload/control leaked to normalized text: {forbidden}"
        );
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
        "drawing after inert WMF CREATEPALETTE should render a passive PDF rectangle"
    );
    assert!(output.diagnostics.is_empty());
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-CREATEPALETTE",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/OpenAction",
        b"/Launch",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF CREATEPALETTE payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_wmf_object_state_records_count_as_partial_without_payload_leakage() {
    for (function, label) in [
        (0x00f7, "WMF-CREATEPALETTE-MALFORMED"),
        (0x012d, "WMF-SELECTOBJECT-MALFORMED"),
        (0x0139, "WMF-RESIZEPALETTE-MALFORMED"),
        (0x01f0, "WMF-DELETEOBJECT-MALFORMED"),
        (0x01f9, "WMF-CREATEPATTERNBRUSH-MALFORMED"),
        (0x0231, "WMF-SETMAPPERFLAGS-MALFORMED"),
        (0x0234, "WMF-SELECTPALETTE-MALFORMED"),
        (0x02fa, "WMF-CREATEPENINDIRECT-MALFORMED"),
        (0x02fb, "WMF-CREATEFONTINDIRECT-MALFORMED"),
        (0x02fc, "WMF-CREATEBRUSHINDIRECT-MALFORMED"),
        (0x0037, "WMF-SETPALENTRIES-MALFORMED"),
        (0x0436, "WMF-ANIMATEPALETTE-MALFORMED"),
        (0x0142, "WMF-DIBCREATEPATTERNBRUSH-MALFORMED"),
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
            .expect("malformed WMF object/state partial vector preview");

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
                "malformed WMF object/state payload/control leaked to normalized text: {forbidden}"
            );
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
                "malformed WMF object/state payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn wmf_pattern_brush_objects_are_inert_without_payload_leakage() {
    let mut pattern = wmf_u16_record(0x01f9, 0);
    pattern.extend_from_slice(b"WMF-CREATEPATTERNBRUSH /JavaScript /EmbeddedFile");
    pattern.resize(pattern.len().next_multiple_of(2), 0);
    let pattern_len = (pattern.len() / 2) as u32;
    write_test_le_u32(&mut pattern, 0, pattern_len);

    let mut dib_pattern = wmf_i16_pair_record(0x0142, 0, 0);
    dib_pattern.extend_from_slice(b"WMF-DIBCREATEPATTERNBRUSH /Launch /RichMedia");
    dib_pattern.resize(dib_pattern.len().next_multiple_of(2), 0);
    let dib_pattern_len = (dib_pattern.len() / 2) as u32;
    write_test_le_u32(&mut dib_pattern, 0, dib_pattern_len);

    let records = [
        pattern,
        dib_pattern,
        wmf_u16_record(0x012d, 0),
        wmf_u16_record(0x012d, 1),
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
        .expect("passive WMF pattern-brush vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert_eq!(parsed.diagnostics.len(), 1);
    for forbidden in [
        "wmetafile",
        "WMF-CREATEPATTERNBRUSH",
        "WMF-DIBCREATEPATTERNBRUSH",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
        "RichMedia",
    ] {
        assert!(
            !text.contains(forbidden),
            "WMF pattern-brush payload/control leaked to normalized text: {forbidden}"
        );
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
        "drawing after inert WMF pattern brushes should render a passive PDF rectangle"
    );
    assert_eq!(output.diagnostics.len(), 1);
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-CREATEPATTERNBRUSH",
        b"WMF-DIBCREATEPATTERNBRUSH",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF pattern-brush payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn missing_wmf_selectobject_is_inert_without_payload_leakage() {
    let mut select = wmf_select_object_record(42);
    select.extend_from_slice(b"WMF-MISSING-SELECTOBJECT /JavaScript /EmbeddedFile /Launch");
    select.resize(select.len().next_multiple_of(2), 0);
    let select_len = (select.len() / 2) as u32;
    write_test_le_u32(&mut select, 0, select_len);

    let records = [
        select,
        wmf_bounds_record(0x041b, 20, 10, 80, 50),
        wmf_yx_record(0x0214, 60, 20),
        wmf_yx_record(0x0213, 70, 80),
    ];
    let wmf = minimal_wmf_with_records(160, 80, &records);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal160\\pichgoal80 {wmf_hex}}} after\\par}}"
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
        .expect("missing WMF SELECTOBJECT vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Line { .. }
    ));
    assert_eq!(parsed.diagnostics.len(), 1);
    for forbidden in [
        "wmetafile",
        "WMF-MISSING-SELECTOBJECT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "missing WMF SELECTOBJECT payload/control leaked to normalized text: {forbidden}"
        );
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
        "drawing after missing WMF SELECTOBJECT should render a passive PDF rectangle"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-MISSING-SELECTOBJECT",
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
            "missing WMF SELECTOBJECT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn missing_wmf_deleteobject_is_inert_without_payload_leakage() {
    let mut delete = wmf_u16_record(0x01f0, 42);
    delete.extend_from_slice(b"WMF-MISSING-DELETEOBJECT /JavaScript /EmbeddedFile /Launch");
    delete.resize(delete.len().next_multiple_of(2), 0);
    let delete_len = (delete.len() / 2) as u32;
    write_test_le_u32(&mut delete, 0, delete_len);

    let records = [
        delete,
        wmf_bounds_record(0x041b, 20, 10, 80, 50),
        wmf_yx_record(0x0214, 60, 20),
        wmf_yx_record(0x0213, 70, 80),
    ];
    let wmf = minimal_wmf_with_records(160, 80, &records);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\wmetafile8\\picw160\\pich80\\picwgoal160\\pichgoal80 {wmf_hex}}} after\\par}}"
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
        .expect("missing WMF DELETEOBJECT vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Line { .. }
    ));
    assert_eq!(parsed.diagnostics.len(), 1);
    for forbidden in [
        "wmetafile",
        "WMF-MISSING-DELETEOBJECT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "missing WMF DELETEOBJECT payload/control leaked to normalized text: {forbidden}"
        );
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
        "drawing after missing WMF DELETEOBJECT should render a passive PDF rectangle"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"wmetafile",
        wmf_hex.as_bytes(),
        b"WMF-MISSING-DELETEOBJECT",
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
            "missing WMF DELETEOBJECT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_wmf_pen_and_brush_objects_keep_later_safe_drawing() {
    for (function, name) in [
        (0x02fa, "CREATEPENINDIRECT"),
        (0x02fc, "CREATEBRUSHINDIRECT"),
    ] {
        let record = wmf_function_record(function);
        let records = [record, wmf_bounds_record(0x041b, 20, 10, 80, 50)];
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
            .expect("malformed WMF pen/brush object partial vector preview");

        assert!(text.contains("before"));
        assert!(text.contains("after"));
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert!(
            image
                .vector_commands
                .iter()
                .any(|command| matches!(command, StaticImageVectorCommand::Rectangle { .. })),
            "safe drawing after malformed WMF {name} should stay visible"
        );
        assert!(!text.contains(name));
    }
}

#[test]
fn malformed_wmf_font_object_keeps_later_safe_text() {
    let records = [
        wmf_function_record(0x02fb),
        wmf_exttextout_record(30, 20, "Font", 0, None),
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
        .expect("malformed WMF font object partial vector preview");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert!(
        image
            .vector_commands
            .iter()
            .any(|command| matches!(command, StaticImageVectorCommand::Text { text, .. } if text == "Font")),
        "safe text after malformed WMF font object should stay visible"
    );
    assert!(!text.contains("Font"));
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
fn missing_emf_deleteobject_is_inert_without_payload_leakage() {
    let mut delete = emf_u32_record(40, 42);
    delete.extend_from_slice(b"EMF-MISSING-DELETEOBJECT /JavaScript /EmbeddedFile /Launch");
    delete.resize(delete.len().next_multiple_of(4), 0);
    let delete_len = delete.len() as u32;
    write_test_le_u32(&mut delete, 4, delete_len);

    let records = [
        delete,
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
        .expect("missing EMF DELETEOBJECT vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Line { .. }
    ));
    assert_eq!(parsed.diagnostics.len(), 1);
    for forbidden in [
        "emfblip",
        " EMF",
        "EMF-MISSING-DELETEOBJECT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "missing EMF DELETEOBJECT payload/control leaked to normalized text: {forbidden}"
        );
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
        "drawing after missing EMF DELETEOBJECT should render a passive PDF rectangle"
    );
    for forbidden in [
        b"/Subtype /Image".as_slice(),
        b"emfblip",
        emf_hex.as_bytes(),
        b"EMF-MISSING-DELETEOBJECT",
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
            "missing EMF DELETEOBJECT payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_null_object_path_paint_records_are_noops_without_payload_leakage() {
    let mut stroke = emf_unknown_record(64);
    stroke.extend_from_slice(b"NULL-OBJECT-EMF-STROKEPATH /JavaScript");
    stroke.resize(stroke.len().next_multiple_of(4), 0);
    let stroke_len = stroke.len() as u32;
    write_test_le_u32(&mut stroke, 4, stroke_len);

    let mut fill = emf_unknown_record(62);
    fill.extend_from_slice(b"NULL-OBJECT-EMF-FILLPATH /EmbeddedFile");
    fill.resize(fill.len().next_multiple_of(4), 0);
    let fill_len = fill.len() as u32;
    write_test_le_u32(&mut fill, 4, fill_len);

    let mut stroke_and_fill = emf_unknown_record(63);
    stroke_and_fill.extend_from_slice(b"NULL-OBJECT-EMF-STROKEANDFILLPATH /Launch");
    stroke_and_fill.resize(stroke_and_fill.len().next_multiple_of(4), 0);
    let stroke_and_fill_len = stroke_and_fill.len() as u32;
    write_test_le_u32(&mut stroke_and_fill, 4, stroke_and_fill_len);

    let records = [
        emf_select_object_record(0x8000_0008),
        emf_unknown_record(59),
        emf_point_record(27, 10, 10),
        emf_point_record(54, 80, 10),
        emf_unknown_record(60),
        stroke,
        emf_select_object_record(0x8000_0005),
        emf_unknown_record(59),
        emf_point_record(27, 20, 20),
        emf_poly_record(6, &[(80, 20), (80, 50), (20, 50)]),
        emf_unknown_record(61),
        emf_unknown_record(60),
        fill,
        emf_select_object_record(0x8000_0008),
        emf_select_object_record(0x8000_0005),
        emf_unknown_record(59),
        emf_point_record(27, 30, 30),
        emf_poly_record(6, &[(90, 30), (90, 60), (30, 60)]),
        emf_unknown_record(61),
        emf_unknown_record(60),
        stroke_and_fill,
        emf_select_object_record(0x8000_0007),
        emf_select_object_record(0x8000_0004),
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
        .expect("passive EMF null-object path vector image");

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
        "NULL-OBJECT-EMF-STROKEPATH",
        "NULL-OBJECT-EMF-FILLPATH",
        "NULL-OBJECT-EMF-STROKEANDFILLPATH",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "null-object EMF path payload/control leaked to normalized text: {forbidden}"
        );
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
        "painting after null-object EMF path no-ops should still render"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"NULL-OBJECT-EMF-STROKEPATH",
        b"NULL-OBJECT-EMF-FILLPATH",
        b"NULL-OBJECT-EMF-STROKEANDFILLPATH",
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
            "null-object EMF path payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_gray_and_dc_stock_objects_render_passively_without_payload_leakage() {
    let mut light_gray = emf_select_object_record(0x8000_0001);
    light_gray.extend_from_slice(b"EMF-LTGRAY-BRUSH /JavaScript");
    light_gray.resize(light_gray.len().next_multiple_of(4), 0);
    let light_gray_len = light_gray.len() as u32;
    write_test_le_u32(&mut light_gray, 4, light_gray_len);

    let mut dc_pen = emf_select_object_record(0x8000_0013);
    dc_pen.extend_from_slice(b"EMF-DC-PEN /EmbeddedFile");
    dc_pen.resize(dc_pen.len().next_multiple_of(4), 0);
    let dc_pen_len = dc_pen.len() as u32;
    write_test_le_u32(&mut dc_pen, 4, dc_pen_len);

    let mut dc_brush = emf_select_object_record(0x8000_0012);
    dc_brush.extend_from_slice(b"EMF-DC-BRUSH /Launch");
    dc_brush.resize(dc_brush.len().next_multiple_of(4), 0);
    let dc_brush_len = dc_brush.len() as u32;
    write_test_le_u32(&mut dc_brush, 4, dc_brush_len);

    let records = [
        light_gray,
        emf_rect_record(43, 0, 0, 80, 40),
        emf_select_object_record(0x8000_0003),
        emf_rect_record(43, 90, 0, 140, 40),
        dc_pen,
        dc_brush,
        emf_rect_record(43, 0, 50, 80, 75),
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
        .expect("passive EMF stock object vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 3);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            fill_color: Some(Color {
                red: 192,
                green: 192,
                blue: 192
            }),
            ..
        }
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Rectangle {
            fill_color: Some(Color {
                red: 64,
                green: 64,
                blue: 64
            }),
            ..
        }
    ));
    assert!(matches!(
        image.vector_commands[2],
        StaticImageVectorCommand::Rectangle {
            stroke_color: Some(Color {
                red: 0,
                green: 0,
                blue: 0
            }),
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
        "EMF-LTGRAY-BRUSH",
        "EMF-DC-PEN",
        "EMF-DC-BRUSH",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF stock object payload/control leaked to normalized text: {forbidden}"
        );
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
            >= 3,
        "EMF stock objects should render passive PDF rectangles"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "f" | "B" | "b")),
        "EMF stock brushes should render passive PDF fills"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "S" | "s" | "B" | "b")),
        "EMF DC pen should render passive PDF strokes"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-LTGRAY-BRUSH",
        b"EMF-DC-PEN",
        b"EMF-DC-BRUSH",
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
            "EMF stock object payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_emf_object_state_records_count_as_partial_without_payload_leakage() {
    for (record_type, label) in [
        (37, "EMF-SELECTOBJECT-MALFORMED"),
        (38, "EMF-CREATEPEN-MALFORMED"),
        (39, "EMF-CREATEBRUSHINDIRECT-MALFORMED"),
        (40, "EMF-DELETEOBJECT-MALFORMED"),
        (48, "EMF-SELECTPALETTE-MALFORMED"),
        (49, "EMF-CREATEPALETTE-MALFORMED"),
        (50, "EMF-SETPALETTEENTRIES-MALFORMED"),
        (51, "EMF-RESIZEPALETTE-MALFORMED"),
        (70, "EMF-COMMENT-MALFORMED"),
        (82, "EMF-EXTCREATEFONTINDIRECTW-MALFORMED"),
        (95, "EMF-EXTCREATEPEN-MALFORMED"),
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
            .expect("malformed EMF object/state partial vector preview");

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
                "malformed EMF object/state payload/control leaked to normalized text: {forbidden}"
            );
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
                "malformed EMF object/state payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
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

    assert!(text.contains(&format!("Body{}", footnote_reference_marker(0))));
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
