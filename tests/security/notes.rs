use super::*;

#[test]
fn compact_word_footnote_band_reservation_matches_source_without_payload_leakage() {
    let output = convert_rtf_to_pdf(
        include_bytes!("../../fixtures/note-restart-numbering-passive.rtf"),
        &ConvertOptions::browser_safe_defaults(),
    )
    .unwrap();
    audit_passive_pdf_bytes(&output.pdf).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let first = pdf_first_text_position_for_text(&content, "First").expect("first footnote");
    let second = pdf_first_text_position_for_text(&content, "Second").expect("second footnote");

    assert!(
        (first.1 - 79.80).abs() < 0.03,
        "first compact footnote should match Word's bottom band: {first:?}"
    );
    assert!(
        (second.1 - 52.20).abs() < 0.03,
        "second compact footnote should match Word's bottom band: {second:?}"
    );
    for forbidden in [
        b"ftnrestart".as_slice(),
        b"aftnrestart",
        b"chftn",
        b"objdata",
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
    assert!(text.contains(&format!("Body{}", footnote_reference_marker(0))));
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
            .contains("line numbering rendered as bounded passive margin text")
    }));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| { !diagnostic.message.contains("line numbering approximated") })
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
fn footnote_static_shape_stays_note_text_without_body_escape() {
    let input = br#"{\rtf1 Body\chftn{\footnote \chftn before {\shp{\*\shpinst\shpleft720\shptop720\shpright2160\shpbottom1440{\sp{\sn shapeType}{\sv 1}}{\sp{\sn pFragments}{\sv hostile-note-shape-payload}}}} after\par}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains(&format!("Body{}", footnote_reference_marker(0))));
    assert!(text.contains("before [Shape skipped: note shape] after"));
    assert!(!parsed.document.footnotes.is_empty());
    assert!(
        parsed
            .document
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::Shape(_))),
        "footnote-owned static shape should not escape into body blocks: {:?}",
        parsed.document.blocks
    );
    for forbidden in ["shapeType", "pFragments", "hostile-note-shape-payload"] {
        assert!(
            !text.contains(forbidden),
            "forbidden footnote shape content leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Body1"));
    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("[Shape skipped: note shape]"));
    assert!(rendered_text.contains("after"));
    for forbidden in [
        b"chftn".as_slice(),
        b"footnote",
        b"shapeType",
        b"pFragments",
        b"hostile-note-shape-payload",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden footnote shape content leaked to PDF: {:?}",
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

    assert!(text.contains(&format!("Body{}", footnote_reference_marker(0))));
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
fn section_footnotes_render_at_passive_section_boundary_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 First section",
        "\\",
        "chftn{",
        "\\",
        "footnote ",
        "\\",
        "chftn First section footnote {",
        "\\",
        "object",
        "\\",
        "objdata ",
        payload_hex(),
        "}",
        "\\",
        "par}",
        "\\",
        "par",
        "\\",
        "sect Second section",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains(&format!("First section{}", footnote_reference_marker(0))));
    assert!(text.contains("Second section"));
    assert!(text.contains("First section footnote"));
    assert_eq!(parsed.document.footnote_section_indices, vec![1]);
    for forbidden in ["chftn", "objdata"] {
        assert!(
            !text.contains(forbidden),
            "section footnote control leaked to text: {forbidden}"
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
        page_texts[0].contains("First section1"),
        "page texts: {page_texts:?}"
    );
    assert!(
        page_texts[0].contains("1. First section footnote"),
        "first section footnote should render before the next section: {page_texts:?}"
    );
    assert!(
        page_texts[1].contains("Second section"),
        "page texts: {page_texts:?}"
    );
    assert!(
        !page_texts[1].contains("First section footnote"),
        "first section footnote should not drift to the next section page: {page_texts:?}"
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
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden section footnote content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn explicit_page_break_flushes_prior_footnotes_without_later_note_duplication() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 First page",
        "\\",
        "chftn{",
        "\\",
        "footnote ",
        "\\",
        "chftn First page footnote {",
        "\\",
        "object",
        "\\",
        "objdata ",
        payload_hex(),
        "}",
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

    assert!(text.contains(&format!("First page{}", footnote_reference_marker(0))));
    assert!(text.contains(&format!("Second page{}", footnote_reference_marker(1))));
    assert_eq!(parsed.document.footnote_block_indices.len(), 2);
    assert!(
        parsed.document.footnote_block_indices[0] < parsed.document.footnote_block_indices[1],
        "footnote reference block order should be monotonic: {:?}",
        parsed.document.footnote_block_indices
    );
    for forbidden in ["chftn", "objdata"] {
        assert!(
            !text.contains(forbidden),
            "page-break footnote control leaked to text: {forbidden}"
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
        page_texts[0].contains("First page1"),
        "page texts: {page_texts:?}"
    );
    assert!(
        page_texts[0].contains("1. First page footnote"),
        "first page footnote should be flushed before explicit page break: {page_texts:?}"
    );
    assert!(
        !page_texts[0].contains("Second page footnote"),
        "later footnote should not render on the earlier page: {page_texts:?}"
    );
    assert!(
        page_texts[1].contains("Second page2"),
        "page texts: {page_texts:?}"
    );
    assert!(
        page_texts[1].contains("2. Second page footnote"),
        "second page footnote should render after its reference page: {page_texts:?}"
    );
    assert!(
        !page_texts[1].contains("First page footnote"),
        "first page footnote should not duplicate on the later page: {page_texts:?}"
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
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden page-break footnote content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn automatic_page_flow_flushes_prior_footnotes_without_tail_drift() {
    let mut input = "{\\rtf1\\paperw6120\\paperh7200\\margt360\\margb360\\fs48 First".to_string();
    input.push_str("\\chftn{\\footnote \\chftn First automatic footnote {\\object\\objdata ");
    input.push_str(payload_hex());
    input.push_str("}\\par} page automatic\\par ");
    for idx in 0..60 {
        input.push_str("Filler paragraph ");
        input.push_str(&idx.to_string());
        input.push_str("\\par ");
    }
    input.push('}');
    let input = input.into_bytes();

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains(&format!(
        "First{} page automatic",
        footnote_reference_marker(0)
    )));
    assert!(text.contains("First automatic footnote"));
    assert_eq!(
        parsed.document.footnote_block_indices,
        vec![0],
        "automatic-flow fixture should reference the first body paragraph block"
    );
    for forbidden in ["chftn", "objdata"] {
        assert!(
            !text.contains(forbidden),
            "automatic-flow footnote control leaked to text: {forbidden}"
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

    assert!(
        page_texts.len() >= 2,
        "fixture should paginate through automatic flow: {page_texts:?}"
    );
    assert!(
        page_texts[0].contains("First1"),
        "page texts: {page_texts:?}"
    );
    assert!(
        page_texts[0].contains("1. First automatic footnote"),
        "first-page footnote should render before automatic flow advances: {page_texts:?}"
    );
    assert!(
        page_texts
            .iter()
            .skip(1)
            .all(|text| !text.contains("First automatic footnote")),
        "first-page footnote should not drift to a later page: {page_texts:?}"
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
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden automatic-flow footnote content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn footnote_page_restart_body_references_use_automatic_layout_pages() {
    let mut input =
        "{\\rtf1\\paperw6120\\paperh7200\\margt360\\margb360\\fs48\\ftnrstpg\\ftnstart4\\ftnnruc First"
            .to_string();
    input.push_str("\\chftn{\\footnote \\chftn First automatic page footnote\\par}\\par ");
    for idx in 0..26 {
        input.push_str("Filler paragraph ");
        input.push_str(&idx.to_string());
        input.push_str("\\par ");
    }
    input.push_str("Second");
    input.push_str("\\chftn{\\footnote \\chftn Second automatic page footnote {\\object\\objdata ");
    input.push_str(payload_hex());
    input.push_str("}\\par}\\par}");
    let input = input.into_bytes();

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains(&format!("First{}", footnote_reference_marker(0))),
        "first automatic-page reference should be layout-resolved: {text:?}"
    );
    assert!(
        text.contains(&format!("Second{}", footnote_reference_marker(1))),
        "second automatic-page reference should be layout-resolved: {text:?}"
    );
    for forbidden in ["ftnrstpg", "ftnstart", "ftnnruc", "chftn", "objdata"] {
        assert!(
            !text.contains(forbidden),
            "automatic page-restart footnote control leaked to text: {forbidden}"
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

    let second_page_index = page_texts
        .iter()
        .position(|text| text.contains("SecondIV"))
        .expect("second footnote reference should render as page-local IV");
    assert!(
        second_page_index > 0,
        "second reference should be on an automatic later page: {page_texts:?}"
    );
    assert!(
        page_texts[0].contains("FirstIV"),
        "first page reference should start at IV: {page_texts:?}"
    );
    assert!(
        page_texts[0].contains("IV. First automatic page footnote"),
        "first page footnote label should start at IV: {page_texts:?}"
    );
    assert!(
        page_texts[second_page_index].contains("IV. Second automatic page footnote"),
        "second automatic page footnote label should restart on its reference page: {page_texts:?}"
    );
    for forbidden in [
        payload_hex().as_bytes(),
        b"ftnrstpg".as_slice(),
        b"ftnstart",
        b"ftnnruc",
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
            "forbidden automatic page-restart content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn footnote_page_restart_uses_line_owned_references_inside_spanning_paragraph() {
    let mut input =
        "{\\rtf1\\paperw6120\\paperh7200\\margt360\\margb360\\fs48\\ftnrstpg\\ftnstart4\\ftnnruc First"
            .to_string();
    input.push_str("\\chftn{\\footnote \\chftn First line-owned footnote\\par} ");
    for _ in 0..120 {
        input.push_str("middle text ");
    }
    input.push_str("Second");
    input.push_str("\\chftn{\\footnote \\chftn Second line-owned footnote {\\object\\objdata ");
    input.push_str(payload_hex());
    input.push_str("}\\par} ");
    for _ in 0..40 {
        input.push_str("tail text ");
    }
    input.push_str("\\par}");
    let input = input.into_bytes();

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains(&format!(
            "First{FOOTNOTE_REFERENCE_MARKER}0{FOOTNOTE_REFERENCE_MARKER_END}"
        )),
        "first spanning-paragraph reference should be layout-resolved: {text:?}"
    );
    assert!(
        text.contains(&format!(
            "Second{FOOTNOTE_REFERENCE_MARKER}1{FOOTNOTE_REFERENCE_MARKER_END}"
        )),
        "second spanning-paragraph reference should be layout-resolved: {text:?}"
    );
    for forbidden in ["ftnrstpg", "ftnstart", "ftnnruc", "chftn", "objdata"] {
        assert!(
            !text.contains(forbidden),
            "line-owned footnote control leaked to text: {forbidden}"
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

    assert!(
        page_texts.len() >= 2,
        "spanning paragraph fixture should create automatic pages: {page_texts:?}"
    );
    assert!(
        page_texts[0].contains("FirstIV"),
        "first reference should render on page 1: {page_texts:?}"
    );
    assert!(
        page_texts[0].contains("IV. First line-owned footnote"),
        "first line-owned footnote should render before page 1 advances: {page_texts:?}"
    );
    assert!(
        !page_texts[0].contains("Second line-owned footnote"),
        "second line-owned footnote should not be pulled back to page 1: {page_texts:?}"
    );
    let second_page_index = page_texts
        .iter()
        .position(|text| text.contains("SecondIV"))
        .expect("second line-owned reference should render as page-local IV");
    assert!(
        second_page_index > 0,
        "second reference should render on a later automatic page: {page_texts:?}"
    );
    assert!(
        page_texts[second_page_index].contains("IV. Second line-owned footnote")
            || page_texts
                .iter()
                .skip(second_page_index)
                .any(|text| text.contains("IV. Second line-owned footnote")),
        "second line-owned footnote should restart at IV on or after its reference page: {page_texts:?}"
    );
    for forbidden in [
        payload_hex().as_bytes(),
        b"ftnrstpg".as_slice(),
        b"ftnstart",
        b"ftnnruc",
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
            "forbidden line-owned footnote content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn continuous_footnotes_use_line_owned_references_inside_spanning_paragraph() {
    let mut input = "{\\rtf1\\paperw6120\\paperh7200\\margt360\\margb360\\fs48 First".to_string();
    input.push_str("\\chftn{\\footnote \\chftn First continuous footnote\\par} ");
    for _ in 0..120 {
        input.push_str("middle text ");
    }
    input.push_str("Second");
    input.push_str("\\chftn{\\footnote \\chftn Second continuous footnote {\\object\\objdata ");
    input.push_str(payload_hex());
    input.push_str("}\\par} ");
    for _ in 0..40 {
        input.push_str("tail text ");
    }
    input.push_str("\\par}");
    let input = input.into_bytes();

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains(&format!("First{}", footnote_reference_marker(0))),
        "first continuous reference should be layout-resolved: {text:?}"
    );
    assert!(
        text.contains(&format!("Second{}", footnote_reference_marker(1))),
        "second continuous reference should be layout-resolved: {text:?}"
    );
    for forbidden in ["chftn", "objdata"] {
        assert!(
            !text.contains(forbidden),
            "continuous footnote control leaked to text: {forbidden}"
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

    assert!(
        page_texts.len() >= 2,
        "continuous spanning paragraph fixture should create automatic pages: {page_texts:?}"
    );
    assert!(
        page_texts[0].contains("First1"),
        "first continuous reference should render on page 1: {page_texts:?}"
    );
    assert!(
        page_texts[0].contains("1. First continuous footnote"),
        "first continuous footnote should render before page 1 advances: {page_texts:?}"
    );
    assert!(
        !page_texts[0].contains("Second continuous footnote"),
        "second continuous footnote should not be pulled back to page 1: {page_texts:?}"
    );
    let second_page_index = page_texts
        .iter()
        .position(|text| text.contains("Second2"))
        .expect("second continuous reference should render with continuous label 2");
    assert!(
        second_page_index > 0,
        "second continuous reference should render on a later automatic page: {page_texts:?}"
    );
    assert!(
        page_texts[second_page_index].contains("2. Second continuous footnote")
            || page_texts
                .iter()
                .skip(second_page_index)
                .any(|text| text.contains("2. Second continuous footnote")),
        "second continuous footnote should render on or after its reference page: {page_texts:?}"
    );
    for forbidden in [
        payload_hex().as_bytes(),
        b"chftn".as_slice(),
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
            "forbidden continuous footnote content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn section_restarting_footnotes_use_line_owned_references_inside_spanning_paragraph() {
    let mut input =
        "{\\rtf1\\paperw6120\\paperh7200\\margt360\\margb360\\fs48\\ftnrestart\\ftnstart1 First"
            .to_string();
    input.push_str("\\chftn{\\footnote \\chftn First section-owned footnote\\par} ");
    for _ in 0..120 {
        input.push_str("middle text ");
    }
    input.push_str("Second");
    input.push_str("\\chftn{\\footnote \\chftn Second section-owned footnote {\\object\\objdata ");
    input.push_str(payload_hex());
    input.push_str("}\\par} ");
    for _ in 0..40 {
        input.push_str("tail text ");
    }
    input.push_str("\\par}");
    let input = input.into_bytes();

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains(&format!("First{}", footnote_reference_marker(0))),
        "first section-restarting reference should be layout-resolved: {text:?}"
    );
    assert!(
        text.contains(&format!("Second{}", footnote_reference_marker(1))),
        "second section-restarting reference should be layout-resolved: {text:?}"
    );
    for forbidden in ["ftnrestart", "ftnstart", "chftn", "objdata"] {
        assert!(
            !text.contains(forbidden),
            "section-restarting footnote control leaked to text: {forbidden}"
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

    assert!(
        page_texts.len() >= 2,
        "section-restarting spanning paragraph fixture should create automatic pages: {page_texts:?}"
    );
    assert!(
        page_texts[0].contains("First1"),
        "first section-restarting reference should render on page 1: {page_texts:?}"
    );
    assert!(
        page_texts[0].contains("1. First section-owned footnote"),
        "first section-restarting footnote should render before page 1 advances: {page_texts:?}"
    );
    assert!(
        !page_texts[0].contains("Second section-owned footnote"),
        "second section-restarting footnote should not be pulled back to page 1: {page_texts:?}"
    );
    let second_page_index = page_texts
        .iter()
        .position(|text| text.contains("Second2"))
        .expect("second section-restarting reference should render with section-local label 2");
    assert!(
        second_page_index > 0,
        "second section-restarting reference should render on a later automatic page: {page_texts:?}"
    );
    assert!(
        page_texts[second_page_index].contains("2. Second section-owned footnote")
            || page_texts
                .iter()
                .skip(second_page_index)
                .any(|text| text.contains("2. Second section-owned footnote")),
        "second section-restarting footnote should render on or after its reference page: {page_texts:?}"
    );
    for forbidden in [
        payload_hex().as_bytes(),
        b"ftnrestart".as_slice(),
        b"ftnstart",
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
            "forbidden section-restarting footnote content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn later_large_footnote_does_not_reserve_space_before_its_reference_line() {
    let mut input = "{\\rtf1\\paperw6120\\paperh4320\\margt360\\margb360\\fs48 First".to_string();
    input.push_str("\\chftn{\\footnote \\chftn First small footnote\\par}");
    for label in [
        "Line one",
        "Line two",
        "Line three",
        "Line four",
        "Line five",
        "Line six",
        "Line seven",
    ] {
        input.push_str("\\line ");
        input.push_str(label);
    }
    input.push_str("\\line Late");
    input.push_str("\\chftn{\\footnote \\chftn Late large footnote {\\object\\objdata ");
    input.push_str(payload_hex());
    input.push_str("} ");
    for _ in 0..80 {
        input.push_str("large note text ");
    }
    input.push_str("\\par} tail\\par}");
    let input = input.into_bytes();

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains(&format!("First{}", footnote_reference_marker(0))),
        "first reference should be a safe layout marker: {text:?}"
    );
    assert!(
        text.contains(&format!("Late{}", footnote_reference_marker(1))),
        "late reference should be a safe layout marker: {text:?}"
    );
    for forbidden in ["chftn", "objdata"] {
        assert!(
            !text.contains(forbidden),
            "large-footnote fixture control leaked to text: {forbidden}"
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
    let page_contents = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| parsed_pdf.get_and_decode_page_content(*page_id).unwrap())
        .collect::<Vec<_>>();
    let page_texts = page_contents
        .iter()
        .map(decoded_pdf_text)
        .collect::<Vec<_>>();
    let page_stroke_counts = page_contents
        .iter()
        .map(|content| {
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "S")
                .count()
        })
        .collect::<Vec<_>>();

    assert!(
        page_texts.len() >= 2,
        "large later footnote fixture should paginate: {page_texts:?}"
    );
    assert!(
        page_texts[0].contains("First1"),
        "first reference should render on page 1: {page_texts:?}"
    );
    assert!(
        page_texts[0].contains("Line four"),
        "later large footnote should not pre-reserve page 1 before its reference: {page_texts:?}"
    );
    assert!(
        page_texts[0].contains("1. First small footnote"),
        "first small footnote should still render on the first page: {page_texts:?}"
    );
    assert!(
        !page_texts[0].contains("Late large footnote"),
        "late large footnote should not render before its reference page: {page_texts:?}"
    );
    let late_page_index = page_texts
        .iter()
        .position(|text| text.contains("Late2"))
        .expect("late reference should render with continuous label 2");
    assert!(
        late_page_index > 0,
        "late reference should render after page 1: {page_texts:?}"
    );
    assert!(
        page_texts
            .iter()
            .skip(late_page_index)
            .any(|text| text.contains("2. Late large footnote")),
        "late large footnote should render on or after its reference page: {page_texts:?}"
    );
    let continuation_note_pages = page_texts
        .iter()
        .enumerate()
        .skip(late_page_index + 1)
        .filter_map(|(idx, text)| text.contains("large note text").then_some(idx))
        .collect::<Vec<_>>();
    assert!(
        !continuation_note_pages.is_empty(),
        "fixture should force the large footnote onto continuation pages: {page_texts:?}"
    );
    for page_idx in continuation_note_pages {
        assert!(
            page_stroke_counts[page_idx] >= 1,
            "continued footnote page should include a passive continuation separator: page {page_idx}, strokes {:?}, texts {page_texts:?}",
            page_stroke_counts
        );
    }
    for forbidden in [
        payload_hex().as_bytes(),
        b"chftn".as_slice(),
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
            "forbidden large-footnote content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn continued_endnotes_use_safe_authored_continuation_separator() {
    let mut input = "{\\rtf1\\paperw6120\\paperh4320\\margt360\\margb360\\fs48\\aenddoc{\\aftnsep Safe endnote separator\\par}{\\aftnsepc Safe endnote continuation {\\object\\objdata ".to_string();
    input.push_str(payload_hex());
    input.push_str("}\\par}Body\\chftn{\\endnote \\chftn Long endnote ");
    for _ in 0..120 {
        input.push_str("continued endnote text ");
    }
    input.push_str("\\par}\\par}");
    let input = input.into_bytes();

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Body1"), "text: {text:?}");
    assert!(text.contains("Long endnote"));
    assert!(text.contains("continued endnote text"));
    for forbidden in [
        "Safe endnote separator",
        "Safe endnote continuation",
        "Embedded object removed",
        "aftnsep",
        "aftnsepc",
        "objdata",
        payload_hex(),
    ] {
        assert!(
            !text.contains(forbidden),
            "endnote separator definition leaked to model text: {forbidden}"
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

    assert!(
        page_texts.len() >= 3,
        "long endnote fixture should span continuation pages: {page_texts:?}"
    );
    assert!(
        page_texts
            .iter()
            .any(|text| text.contains("Safe endnote separator")),
        "initial authored endnote separator should render: {page_texts:?}"
    );
    let continuation_pages = page_texts
        .iter()
        .enumerate()
        .skip(1)
        .filter_map(|(idx, text)| text.contains("Safe endnote continuation").then_some(idx))
        .collect::<Vec<_>>();
    assert!(
        !continuation_pages.is_empty(),
        "continued endnote pages should render the authored continuation separator: {page_texts:?}"
    );
    for page_idx in continuation_pages {
        assert!(
            page_texts[page_idx].contains("continued endnote text"),
            "continuation separator should appear on a carried-over endnote page: {page_texts:?}"
        );
    }
    for forbidden in [
        payload_hex().as_bytes(),
        b"aftnsep".as_slice(),
        b"aftnsepc",
        b"objdata",
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
            "forbidden continued-endnote content leaked to PDF: {:?}",
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

    assert_eq!(page_texts.len(), 2);
    assert!(page_texts[0].contains("First page"));
    assert!(page_texts[1].contains("Last page1"));
    assert!(page_texts[1].contains("1. Final endnote"));
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
    assert!(text.contains("Bodyi"));
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

    assert_eq!(page_texts.len(), 1);
    assert!(page_texts[0].contains("Bodyi"));
    assert!(page_texts[0].contains("iLegacy endnote"));
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
fn note_separator_definitions_render_safely_without_body_or_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "ftnsep Safe footnote separator {",
        "\\",
        "object",
        "\\",
        "objdata ",
        payload_hex(),
        "}",
        "\\",
        "par}{",
        "\\",
        "ftnsepc Safe footnote continuation}{",
        "\\",
        "aftnsep Safe endnote separator}{",
        "\\",
        "aftnsepc Safe endnote continuation}Body",
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

    assert!(text.contains(&format!("Body{} End1", footnote_reference_marker(0))));
    assert!(text.contains("Footnote text"));
    assert!(text.contains("Endnote text"));
    for forbidden in [
        "Safe footnote separator",
        "Safe footnote continuation",
        "Safe endnote separator",
        "Safe endnote continuation",
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
    assert!(rendered_text.contains("Safe footnote separator"));
    assert!(rendered_text.contains("Safe endnote separator"));
    assert!(rendered_text.contains("Footnote text"));
    assert!(rendered_text.contains("Endnote text"));
    for forbidden in [
        b"ftnsep".as_slice(),
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

    assert!(
        text.contains(&format!(
            "Body{} Middle{} Endb",
            footnote_reference_marker(0),
            footnote_reference_marker(1)
        )),
        "text: {text:?}"
    );
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
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("footnote page restart")),
        "footnote page restart should be layout-resolved, not reported as approximated: {:?}",
        parsed.diagnostics
    );
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("endnote page restart behavior approximated")),
        "endnote page restart should be layout-resolved, not reported as approximated: {:?}",
        parsed.diagnostics
    );
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
fn endnote_page_restart_uses_layout_resolved_endnote_pages() {
    let mut input =
        "{\\rtf1\\paperw6120\\paperh3600\\margt360\\margb360\\fs48\\aenddoc\\aftnrstpg\\aftnstart2\\aftnnalc First".to_string();
    input.push_str("\\chftn{\\endnote \\chftn First long endnote ");
    for idx in 0..42 {
        input.push_str("line ");
        input.push_str(&idx.to_string());
        input.push_str(" ");
    }
    input.push_str("{\\object\\objdata ");
    input.push_str(payload_hex());
    input.push_str("}\\par} Second");
    input.push_str("\\chftn{\\endnote \\chftn Second endnote text\\par} End\\par}");
    let input = input.into_bytes();

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains(&format!(
            "First{} Second{} End",
            endnote_reference_marker(0),
            endnote_reference_marker(1)
        )),
        "page-restarting endnote references should stay internal until layout: {text:?}"
    );
    assert_eq!(parsed.document.endnotes.len(), 2);
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("endnote page restart behavior approximated")),
        "endnote page restart should not be reported as approximated: {:?}",
        parsed.diagnostics
    );
    for forbidden in ["aftnrstpg", "aftnstart", "aftnnalc", "chftn", "objdata"] {
        assert!(
            !text.contains(forbidden),
            "endnote page-restart control leaked into text: {forbidden}"
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
    let rendered_text = page_texts.join("\n");

    assert!(
        rendered_text.contains("Firstb Secondb End"),
        "body endnote references should resolve to page-local labels: {page_texts:?}"
    );
    assert!(
        rendered_text.contains("b. First long endnote"),
        "first endnote should restart at configured lower-alpha start: {page_texts:?}"
    );
    assert!(
        rendered_text.contains("b. Second endnote text"),
        "second endnote should restart on its later rendered endnote page: {page_texts:?}"
    );
    let first_note_page = page_texts
        .iter()
        .position(|page| page.contains("b. First long endnote"))
        .expect("first endnote page");
    let second_note_page = page_texts
        .iter()
        .position(|page| page.contains("b. Second endnote text"))
        .expect("second endnote page");
    assert!(
        second_note_page > first_note_page,
        "fixture should prove page-local endnote restart across different pages: {page_texts:?}"
    );
    for forbidden in [
        ENDNOTE_REFERENCE_MARKER.as_bytes(),
        ENDNOTE_REFERENCE_MARKER_END.as_bytes(),
        payload_hex().as_bytes(),
        b"aftnrstpg".as_slice(),
        b"aftnstart",
        b"aftnnalc",
        b"chftn",
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
            "endnote page-restart content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}
