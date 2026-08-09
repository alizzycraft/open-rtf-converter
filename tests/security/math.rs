use super::*;

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
    assert!(text.contains("Before E=mc2 After"));
    let math_style = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Paragraph(paragraph) => paragraph
                .runs
                .iter()
                .find(|run| run.text.contains('2'))
                .map(|run| &run.style),
            _ => None,
        })
        .expect("math run");
    assert!(math_style.passive_math.enabled);
    assert_eq!(math_style.baseline_shift_half_points, 0);
    assert_eq!(math_style.font_size_scale_percent, 100);
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
            .contains("Office math rendered as bounded passive math text")
    }));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("Office math layout approximated")
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
    assert!(rendered_text.contains("Before E=mc2 After"));
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
    assert!(text.contains("Before x+1y After"));
    let numerator_style = run_style_for_text(&parsed.document, "x+1").expect("fraction numerator");
    assert!(numerator_style.baseline_shift_half_points > 0);
    assert!(numerator_style.font_size_scale_percent < 100);
    assert_eq!(
        numerator_style.passive_math.fraction_part,
        PassiveMathFractionPart::Numerator
    );
    let denominator_style =
        run_style_for_text(&parsed.document, "y").expect("fraction denominator");
    assert!(denominator_style.baseline_shift_half_points < 0);
    assert!(denominator_style.font_size_scale_percent < 100);
    assert_eq!(
        denominator_style.passive_math.fraction_part,
        PassiveMathFractionPart::Denominator
    );
    assert_eq!(
        numerator_style.passive_math.fraction_id,
        denominator_style.passive_math.fraction_id
    );
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
    assert!(rendered_text.contains("Before x+1"));
    assert!(rendered_text.contains("y After"));
    let layout = LayoutEngine::try_layout(&parsed.document)
        .expect("parsed documents satisfy model invariants");
    let page = &layout.pages[0];
    let numerator_fragment = page.items.iter().find_map(|item| match item {
        LayoutItem::Text(fragment) if fragment.text == "x" => Some(fragment),
        _ => None,
    });
    let denominator_fragment = page.items.iter().find_map(|item| match item {
        LayoutItem::Text(fragment) if fragment.text == "y" => Some(fragment),
        _ => None,
    });
    let fraction_rule = page.items.iter().find_map(|item| match item {
        LayoutItem::Line {
            x1,
            y1,
            x2,
            y2,
            width,
            ..
        } if (*y1 - *y2).abs() < 0.01 && (*width - 0.84).abs() < 0.01 => Some((*x1, *y1, *x2)),
        _ => None,
    });
    let numerator_fragment = numerator_fragment.expect("stacked numerator fragment");
    let denominator_fragment = denominator_fragment.expect("stacked denominator fragment");
    let (rule_x1, rule_y, rule_x2) = fraction_rule.expect("passive fraction rule");
    assert!(numerator_fragment.baseline_y > denominator_fragment.baseline_y);
    assert!((numerator_fragment.x - rule_x1).abs() < 0.1);
    assert!(denominator_fragment.x > numerator_fragment.x);
    assert!(rule_x2 > denominator_fragment.x);
    assert!(
        rule_y < numerator_fragment.baseline_y && rule_y > denominator_fragment.baseline_y,
        "fraction rule should sit between numerator and denominator"
    );
    assert!(
        pdf_text_font_names(&content)
            .iter()
            .any(|name| name.starts_with(b"TF")),
        "Office math fraction should use a passive supplied font"
    );
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
fn office_math_fraction_types_are_bounded_passive_metadata() {
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
        "mfPr{",
        "launch.exe objdata 414243 ",
        "\\",
        "u65?",
        "\\",
        "'42",
        "\\",
        "mtype noBar}{",
        "\\",
        "*",
        "\\",
        "unknown{",
        "\\",
        "object",
        "\\",
        "objdata 414243}}}{",
        "\\",
        "mnum{",
        "\\",
        "mtext A}}{",
        "\\",
        "mden{",
        "\\",
        "mtext B}}}}} and {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mf{",
        "\\",
        "mfPr{",
        "https://example.com/payload objdata 444546 ",
        "\\",
        "u67?",
        "\\",
        "'44",
        "\\",
        "mtype calc.exe}}{",
        "\\",
        "mnum{",
        "\\",
        "mtext C}}{",
        "\\",
        "mden{",
        "\\",
        "mtext D}}}}} and {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mf{",
        "\\",
        "mfPr{",
        "calc.exe objdata 454647 ",
        "\\",
        "u72?",
        "\\",
        "'49",
        "\\",
        "mtype lin}}{",
        "\\",
        "mnum{",
        "\\",
        "mtext E}}{",
        "\\",
        "mden{",
        "\\",
        "mtext F}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before AB and CD and E\u{2044}F After"),
        "unexpected fraction type text: {text:?}"
    );
    let no_bar_denominator_style =
        run_style_for_text(&parsed.document, "B").expect("no-bar denominator");
    assert!(no_bar_denominator_style.baseline_shift_half_points < 0);
    assert!(no_bar_denominator_style.font_size_scale_percent < 100);
    for forbidden in [
        "mmath",
        "moMath",
        "mfPr",
        "mtype",
        "mtext",
        "noBar",
        "calc.exe",
        "launch.exe",
        "example.com/payload",
        "objdata",
        "414243",
        "444546",
        "454647",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math fraction type metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math fraction type controls should not be reported as unknown or unsupported: {:?}",
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
    for visible in ["Before AB", "C", "D", "E", "F After"] {
        assert!(
            rendered_text.contains(visible),
            "fraction type visible text missing from PDF text: {visible}; got {rendered_text:?}"
        );
    }
    assert!(
        pdf_text_font_names(&content)
            .iter()
            .any(|name| name.starts_with(b"TF")),
        "fraction types should use a passive supplied font"
    );
    let e_position = pdf_first_text_position_for_text(&content, "E").expect("linear numerator");
    let f_position = pdf_first_text_position_for_text(&content, "F").expect("linear denominator");
    assert!(
        (e_position.1 - f_position.1).abs() < 0.5,
        "linear Office math fraction should render numerator and denominator on one baseline: E={e_position:?}, F={f_position:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mfPr",
        b"mtype",
        b"mtext",
        b"noBar",
        b"calc.exe",
        b"launch.exe",
        b"example.com/payload",
        b"objdata",
        b"414243",
        b"444546",
        b"454647",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math fraction type metadata leaked to PDF: {:?}",
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
    let radicand_style = run_style_for_text(&parsed.document, "x+1").expect("radicand run");
    assert!(radicand_style.overline);
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
        font_names.iter().any(|name| name.starts_with(b"TF")),
        "square-root marker should use a passive supplied font; font selections were {font_names:?}"
    );
    assert!(
        content.operations.windows(3).any(|operations| {
            operations[0].operator == "m"
                && operations[1].operator == "l"
                && operations[2].operator == "S"
        }),
        "Office math radicand should render with a passive overbar stroke"
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
fn office_math_radical_degrees_render_or_hide_passively() {
    let visible_input = br"{\rtf1 Before {\mmath{\moMath{\mrad{\mradPr calc.exe objdata 414243 \u65?\'42{\mdegHide0}}{\mdeg{\mtext 3}}{\me{\mtext x}}}}} After\par}".to_vec();
    let visible_parsed = parse_rtf_bytes(&visible_input).unwrap();
    let visible_text = collect_text(&visible_parsed.document);
    assert!(
        visible_text.contains("Before \u{221a}3x After"),
        "unexpected visible radical degree text: {visible_text:?}"
    );
    let visible_degree_style =
        run_style_for_text(&visible_parsed.document, "3").expect("visible radical degree");
    assert!(visible_degree_style.baseline_shift_half_points > 0);
    assert!(visible_degree_style.font_size_scale_percent < 100);
    let visible_radicand_style =
        run_style_for_text(&visible_parsed.document, "x").expect("visible radicand");
    assert!(visible_radicand_style.overline);
    for forbidden in ["calc.exe", "objdata", "414243", "AB"] {
        assert!(
            !visible_text.contains(forbidden),
            "Office math visible radical property payload leaked to text: {forbidden}"
        );
    }

    let hidden_input = br"{\rtf1 Before {\mmath{\moMath{\mrad{\mradPr launch.exe https://example.com/payload objdata 444546 \u67?\'44{\mdegHide1}}{\mdeg{\mtext HIDDEN-DEGREE-PAYLOAD}}{\me{\mtext x}}}}} After\par}".to_vec();
    let hidden_parsed = parse_rtf_bytes(&hidden_input).unwrap();
    let hidden_text = collect_text(&hidden_parsed.document);
    assert!(
        hidden_text.contains("Before \u{221a}x After"),
        "unexpected hidden radical degree text: {hidden_text:?}"
    );
    for forbidden in [
        "mmath",
        "moMath",
        "mrad",
        "mradPr",
        "mdegHide",
        "mdeg",
        "mtext",
        "launch.exe",
        "example.com/payload",
        "objdata",
        "444546",
        "HIDDEN-DEGREE-PAYLOAD",
    ] {
        assert!(
            !hidden_text.contains(forbidden),
            "Office math radical degree content leaked to text: {forbidden}"
        );
    }
    assert!(
        hidden_parsed
            .diagnostics
            .iter()
            .all(
                |diagnostic| !diagnostic.message.contains("unknown RTF destination")
                    && !diagnostic.message.contains("unsupported RTF control")
            ),
        "Office math radical degree controls should not be reported as unknown or unsupported: {:?}",
        hidden_parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &hidden_input,
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
    assert!(
        pdf_text_font_names(&content)
            .iter()
            .any(|name| name.starts_with(b"TF")),
        "hidden-degree radical should use a passive supplied font"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mrad",
        b"mradPr",
        b"mdegHide",
        b"mdeg",
        b"mtext",
        b"launch.exe",
        b"example.com/payload",
        b"objdata",
        b"444546",
        b"HIDDEN-DEGREE-PAYLOAD",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math radical degree content leaked to PDF: {:?}",
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
    assert!(text.contains("Before x After"));
    let overbar_style = run_style_for_text(&parsed.document, "x").expect("overbar run");
    assert!(overbar_style.overline);
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
    assert!(rendered_text.contains("Before x After"));
    let helvetica_bytes = pdf_text_bytes_for_font(&content, b"F1");
    assert!(
        !helvetica_bytes.contains(&0xaf),
        "overbar should render as passive geometry, not as a macron glyph; got {helvetica_bytes:?}"
    );
    assert!(
        content.operations.windows(3).any(|operations| {
            operations[0].operator == "m"
                && operations[1].operator == "l"
                && operations[2].operator == "S"
        }),
        "Office math overbar should render as a passive stroke"
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
fn office_math_bar_positions_render_passively_without_payload_leakage() {
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
        "calc.exe objdata 414243 ",
        "\\",
        "u65?",
        "\\",
        "'42",
        "\\",
        "mpos bot}{",
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
        "mtext UnderBar}}}}} and {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mbar{",
        "\\",
        "mbarPr{",
        "launch.exe https://example.com/payload objdata 444546 ",
        "\\",
        "u67?",
        "\\",
        "'44",
        "\\",
        "mpos top}}{",
        "\\",
        "me{",
        "\\",
        "mtext OverBar}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before UnderBar and OverBar After"),
        "unexpected bar-position math text: {text:?}"
    );
    let under_style = run_style_for_text(&parsed.document, "UnderBar").expect("underbar run");
    assert!(!under_style.overline);
    assert_eq!(under_style.underline, UnderlineStyle::Single);
    let over_style = run_style_for_text(&parsed.document, "OverBar").expect("overbar run");
    assert!(over_style.overline);
    assert_eq!(over_style.underline, UnderlineStyle::None);
    for forbidden in [
        "mmath",
        "moMath",
        "mbar",
        "mbarPr",
        "mpos",
        "bot",
        "top",
        "mtext",
        "calc.exe",
        "launch.exe",
        "example.com/payload",
        "objdata",
        "414243",
        "444546",
        "AB",
        "CD",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math bar-position metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math bar-position controls should not be reported as unknown or unsupported: {:?}",
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
    assert!(rendered_text.contains("Before UnderBar and OverBar After"));
    let passive_strokes = content
        .operations
        .windows(3)
        .filter(|operations| {
            operations[0].operator == "m"
                && operations[1].operator == "l"
                && operations[2].operator == "S"
        })
        .count();
    assert!(
        passive_strokes >= 2,
        "Office math top/bottom bars should render as passive strokes, got {passive_strokes}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mbar",
        b"mbarPr",
        b"mpos",
        b"calc.exe",
        b"launch.exe",
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
            "Office math bar-position metadata or payload leaked to PDF: {:?}",
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
        text.contains("Before \u{2211}i=1ni After"),
        "unexpected n-ary math text: {text:?}"
    );
    let lower_limit_style =
        run_style_for_text(&parsed.document, "i=1").expect("lower n-ary limit run");
    assert_eq!(lower_limit_style.baseline_shift_half_points, 0);
    assert_eq!(lower_limit_style.font_size_scale_percent, 100);
    assert!(lower_limit_style.passive_math.nary_id.is_some());
    assert!(lower_limit_style.passive_math.nary_base_id.is_some());
    let upper_limit_style = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .flat_map(|paragraph| &paragraph.runs)
        .find(|run| run.style.passive_math.nary_id.is_some() && run.text.contains('n'))
        .map(|run| &run.style)
        .expect("upper n-ary limit run");
    assert_eq!(upper_limit_style.baseline_shift_half_points, 0);
    assert_eq!(upper_limit_style.font_size_scale_percent, 100);
    assert!(upper_limit_style.passive_math.nary_id.is_some());
    let strict = parse_rtf_bytes_with_options(
        &input,
        &RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        },
    )
    .unwrap();
    let strict_lower =
        run_style_for_text(&strict.document, "i=1").expect("strict lower n-ary limit run");
    let strict_upper =
        run_style_for_text(&strict.document, "n").expect("strict upper n-ary limit run");
    assert!(strict_lower.baseline_shift_half_points < 0);
    assert!(strict_lower.font_size_scale_percent < 100);
    assert!(strict_upper.baseline_shift_half_points > 0);
    assert!(strict_upper.font_size_scale_percent < 100);
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
    assert!(rendered_text.contains("i=1"));
    assert!(rendered_text.contains("ni After"));
    assert!(
        pdf_text_font_names(&content)
            .iter()
            .any(|name| name.starts_with(b"TF")),
        "summation operator should use a passive supplied font"
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
fn office_math_visible_nary_reference_uses_word_source_advances_and_wrap_lead_in() {
    let input = include_bytes!("../../fixtures/office-math-nary-visible-passive.rtf");
    let mut options = ConvertOptions::browser_safe_defaults();
    options.parse_options.compatibility_mode = CompatibilityMode::WordCompatiblePassive;
    let output = convert_rtf_to_pdf(input, &options).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    let between = pdf_first_text_position_for_text(&content, "between").unwrap_or_else(|| {
        panic!(
            "text after summation group: {:?}",
            decoded_pdf_text(&content)
        )
    });
    let product_base = pdf_first_text_position_for_text(&content, "k").expect("product base");
    let after =
        pdf_first_text_position_for_text(&content, "after").expect("text after product group");

    assert!(
        (between.0 - 159.98).abs() < 0.15,
        "the first n-ary group should retain Word's lead-in and source advances: {between:?}"
    );
    assert!(
        (product_base.0 - 47.75).abs() < 0.10,
        "wrapped n-ary object must discard its line-leading object inset: {product_base:?}"
    );
    assert!(
        (after.0 - 96.25).abs() < 0.10,
        "Cambria Math source advances should bound the product group: {after:?}"
    );
    assert!(
        ((product_base.1 - after.1) - 0.48).abs() < 0.02,
        "linearized n-ary glyphs should use Word's raised Cambria Math baseline"
    );
    assert!(
        ((between.1 - after.1) - 14.04).abs() < 0.03,
        "visible n-ary lines should use Word's Cambria Math line pitch"
    );
}

#[test]
fn office_math_nary_limit_locations_are_bounded_passive_metadata() {
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
        "mlimLoc undOvr}{",
        "\\",
        "*",
        "\\",
        "unknown{",
        "\\",
        "object",
        "\\",
        "objdata 414243}}}{",
        "\\",
        "msub{",
        "\\",
        "mtext lowA}}{",
        "\\",
        "msup{",
        "\\",
        "mtext upA}}{",
        "\\",
        "me{",
        "\\",
        "mtext bodyA}}}}} and {",
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
        "mlimLoc calc.exe}}{",
        "\\",
        "msub{",
        "\\",
        "mtext lowB}}{",
        "\\",
        "msup{",
        "\\",
        "mtext upB}}{",
        "\\",
        "me{",
        "\\",
        "mtext bodyB}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes_with_options(
        &input,
        &RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        },
    )
    .unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before \u{2211}lowAupAbodyA and \u{2211}lowBupBbodyB After"),
        "unexpected n-ary limit-location text: {text:?}"
    );
    let under_over_lower =
        run_style_for_text(&parsed.document, "lowA").expect("under-over lower limit");
    let default_lower = run_style_for_text(&parsed.document, "lowB").expect("default lower limit");
    assert!(
        under_over_lower.baseline_shift_half_points < default_lower.baseline_shift_half_points,
        "under-over lower limit should use a stronger passive downward shift"
    );
    let under_over_upper =
        run_style_for_text(&parsed.document, "upA").expect("under-over upper limit");
    let default_upper = run_style_for_text(&parsed.document, "upB").expect("default upper limit");
    assert!(
        under_over_upper.baseline_shift_half_points > default_upper.baseline_shift_half_points,
        "under-over upper limit should use a stronger passive upward shift"
    );
    for forbidden in [
        "mmath", "moMath", "mnary", "mnaryPr", "mchr", "mlimLoc", "undOvr", "calc.exe", "objdata",
        "414243", "mtext",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math n-ary limit-location metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math n-ary limit-location controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );

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
    for visible in ["Before ", "lowA", "upAbodyA and ", "lowB", "upBbodyB After"] {
        assert!(
            rendered_text.contains(visible),
            "n-ary limit-location visible text missing from PDF text: {visible}; got {rendered_text:?}"
        );
    }
    let under_over_base = pdf_first_text_position_for_text(&content, "bodyA").expect("base bodyA");
    let under_over_lower_pos =
        pdf_first_text_position_for_text(&content, "lowA").expect("under-over lower position");
    let default_base = pdf_first_text_position_for_text(&content, "bodyB").expect("base bodyB");
    let default_lower_pos =
        pdf_first_text_position_for_text(&content, "lowB").expect("default lower position");
    assert!(
        (under_over_base.1 - under_over_lower_pos.1) > (default_base.1 - default_lower_pos.1),
        "under-over lower limit should render farther below its base than default sub/sup mode"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mnary",
        b"mnaryPr",
        b"mchr",
        b"mlimLoc",
        b"undOvr",
        b"calc.exe",
        b"objdata",
        b"414243",
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
            "Office math n-ary limit-location metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_style_metadata_is_bounded_and_passive() {
    let input = br"{\rtf1 Before {\mmath{\moMath{\mtext{\msty bi}Styled}}} and {\mmath{\moMath{\mtext{{\mctrlPr calc.exe objdata 444546 \u67?\'44{\*\unknown{\object\objdata 454647}}{\msty i}}Wrapped}}}} and {\mmath{\moMath{\mtext{\msty calc.exe{\*\unknown{\object\objdata 414243}}}Plain}}} After\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before Styled and Wrapped and Plain After"),
        "unexpected Office math style text: {text:?}"
    );
    let styled = run_style_for_text(&parsed.document, "Styled").expect("styled math run");
    assert!(styled.bold);
    assert!(styled.italic);
    let wrapped = run_style_for_text(&parsed.document, "Wrapped").expect("wrapped math run");
    assert!(!wrapped.bold);
    assert!(wrapped.italic);
    let plain = run_style_for_text(&parsed.document, "Plain").expect("plain math run");
    assert!(!plain.bold);
    assert!(!plain.italic);
    for forbidden in [
        "mmath", "moMath", "mtext", "mctrlPr", "msty", "bi", "calc.exe", "objdata", "414243",
        "444546", "454647",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math style metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math style metadata controls should not be reported as unknown or unsupported: {:?}",
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
        rendered_text.contains("Before Styled and Wrapped and Plain After"),
        "Office math style visible text missing from PDF text: {rendered_text:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mtext",
        b"mctrlPr",
        b"msty",
        b"calc.exe",
        b"objdata",
        b"414243",
        b"444546",
        b"454647",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math style metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_overline_accents_render_as_passive_strokes_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "macc{",
        "\\",
        "maccPr{",
        "\\",
        "mchr ",
        "\\",
        "u773?}}{",
        "\\",
        "me{",
        "\\",
        "mtext x}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before x After"),
        "unexpected accent math text: {text:?}"
    );
    let overlined_style = run_style_for_text(&parsed.document, "x").expect("accent base run");
    assert!(overlined_style.overline);
    for forbidden in ["mmath", "moMath", "macc", "maccPr", "mchr", "mtext"] {
        assert!(
            !text.contains(forbidden),
            "Office math accent control leaked to text: {forbidden}"
        );
    }
    assert!(
        !text.contains('?'),
        "Office math accent fallback text leaked to text: {text:?}"
    );
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "Office math accent controls should not be reported as unsupported: {:?}",
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
    assert!(rendered_text.contains("Before x After"));
    assert!(
        content.operations.windows(3).any(|operations| {
            operations[0].operator == "m"
                && operations[1].operator == "l"
                && operations[2].operator == "S"
        }),
        "Office math overline accent should render as a passive stroke"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"macc",
        b"maccPr",
        b"mchr",
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
            "Office math accent control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_text_accents_render_passively_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "macc{",
        "\\",
        "maccPr{",
        "launch.exe objdata 414243 ",
        "\\",
        "u65?",
        "\\",
        "'42",
        "https://example.com/payload objdata 444546 ",
        "\\",
        "u67?",
        "\\",
        "'44",
        "\\",
        "mchr ",
        "\\",
        "u770?}}{",
        "\\",
        "me{",
        "\\",
        "mtext x}}}}} and {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "macc{",
        "\\",
        "maccPr{",
        "calc.exe objdata 454647 ",
        "\\",
        "u72?",
        "\\",
        "'49",
        "\\",
        "mchr ",
        "\\",
        "u732?}}{",
        "\\",
        "me{",
        "\\",
        "mtext y}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before x^ and y~ After"),
        "unexpected text-accent math text: {text:?}"
    );
    for forbidden in [
        "mmath",
        "moMath",
        "macc",
        "maccPr",
        "mchr",
        "mtext",
        "launch.exe",
        "example.com/payload",
        "objdata",
        "414243",
        "444546",
        "454647",
        "calc.exe",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math text-accent metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math text-accent controls should not be reported as unknown or unsupported: {:?}",
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
        rendered_text.contains("Before x^ and y~ After"),
        "Office math text accents missing from PDF text: {rendered_text:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"macc",
        b"maccPr",
        b"mchr",
        b"mtext",
        b"launch.exe",
        b"example.com/payload",
        b"objdata",
        b"414243",
        b"444546",
        b"454647",
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
            "Office math text-accent metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_border_boxes_render_passive_character_borders_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mborderBox{",
        "\\",
        "mborderBoxPr{calc.exe objdata 414243 ",
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
        "objdata 444546}}}}{",
        "\\",
        "me{",
        "\\",
        "mtext x+1}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before x+1 After"),
        "unexpected border-box math text: {text:?}"
    );
    let boxed_style = run_style_for_text(&parsed.document, "x+1").expect("border-box run");
    assert!(boxed_style.border.visible);
    assert_eq!(boxed_style.border.style, BorderStyle::Single);
    assert!(boxed_style.border.width_twips <= RtfLimits::default().max_table_border_width_twips);
    for forbidden in [
        "mmath",
        "moMath",
        "mborderBox",
        "mborderBoxPr",
        "mtext",
        "calc.exe",
        "objdata",
        "414243",
        "444546",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math border-box control leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math border-box controls should not be reported as unknown or unsupported: {:?}",
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
    assert!(rendered_text.contains("Before x+1 After"));
    let passive_rectangles = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "re")
        .count();
    assert!(
        passive_rectangles >= 4,
        "Office math border box should render as passive filled rectangles, got {passive_rectangles}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mborderBox",
        b"mborderBoxPr",
        b"mtext",
        b"calc.exe",
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
            "Office math border-box control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_manual_breaks_render_passive_line_breaks_without_control_leakage() {
    let input = br"{\rtf1 Before {\mmath{\moMath{\mbox{\mboxPr{\mbrk0 calc.exe objdata 414243 \u65?\'42 }{\*\unknown{\object\objdata 444546}}}{\me{\mtext First}}}{\mtext Second}}} After\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before \nFirstSecond After"),
        "unexpected Office math manual-break text: {text:?}"
    );
    for forbidden in [
        "mmath", "moMath", "mbox", "mboxPr", "mbrk", "mtext", "calc.exe", "objdata", "414243",
        "444546",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math manual-break control leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math manual-break controls should not be reported as unknown or unsupported: {:?}",
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
    assert!(rendered_text.contains("FirstSecond After"));
    let before_pos = pdf_first_text_position_for_text(&content, "Before").expect("before position");
    let first_pos = pdf_first_text_position_for_text(&content, "First").expect("first position");
    assert!(
        first_pos.1 < before_pos.1,
        "Office math manual break should render following text below the prior line: before={before_pos:?}, first={first_pos:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mbox",
        b"mboxPr",
        b"mbrk",
        b"mtext",
        b"calc.exe",
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
            "Office math manual-break content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_box_operator_emulation_adds_passive_spacing_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 A{",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mbox{",
        "\\",
        "mboxPr{",
        "\\",
        "mopEmu1 launch.exe objdata 444546 ",
        "\\",
        "u67?'44}{",
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
        "mtext +}}}}}B ",
        "C{",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mbox{",
        "\\",
        "mboxPr{",
        "\\",
        "mopEmu0 https://example.com/payload objdata 454647 ",
        "\\",
        "u72?'49}}{",
        "\\",
        "me{",
        "\\",
        "mtext -}}}}}D",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("A + B C-D"),
        "unexpected Office math box operator-emulation text: {text:?}"
    );
    for forbidden in [
        "mmath",
        "moMath",
        "mbox",
        "mboxPr",
        "mopEmu",
        "mtext",
        "launch.exe",
        "example.com/payload",
        "objdata",
        "414243",
        "444546",
        "454647",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math box operator-emulation metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math box operator-emulation controls should not be reported as unknown or unsupported: {:?}",
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
        rendered_text.contains("A + B C-D"),
        "Office math operator-emulation spacing missing from PDF text: {rendered_text:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mbox",
        b"mboxPr",
        b"mopEmu",
        b"mtext",
        b"launch.exe",
        b"example.com/payload",
        b"objdata",
        b"414243",
        b"444546",
        b"454647",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math box operator-emulation metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_group_characters_render_passive_lines_without_fallback_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mgroupChr{",
        "\\",
        "mgroupChrPr{",
        "\\",
        "mchr ",
        "\\",
        "u9182?}}{",
        "\\",
        "me{",
        "\\",
        "mtext x+1}}}}} and {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mgroupChr{",
        "\\",
        "mgroupChrPr{",
        "\\",
        "mchr ",
        "\\",
        "u9183?}}{",
        "\\",
        "me{",
        "\\",
        "mtext y}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before x+1 and y After"),
        "unexpected group-character math text: {text:?}"
    );
    let over_style = run_style_for_text(&parsed.document, "x+1").expect("over group run");
    assert!(over_style.overline);
    let under_style = run_style_for_text(&parsed.document, "y").expect("under group run");
    assert_eq!(under_style.underline, UnderlineStyle::Single);
    let layout = LayoutEngine::try_layout(&parsed.document)
        .expect("parsed documents satisfy model invariants");
    let passive_over_group_lines = layout.pages[0]
        .items
        .iter()
        .filter_map(|item| match item {
            LayoutItem::Line { y1, y2, width, .. } if (*y1 - *y2).abs() < 0.01 => Some(*width),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        passive_over_group_lines.len(),
        1,
        "operator spacing must not split the passive over-group stroke"
    );
    assert!((passive_over_group_lines[0] - 0.84).abs() < 0.01);
    for forbidden in [
        "mmath",
        "moMath",
        "mgroupChr",
        "mgroupChrPr",
        "mchr",
        "mtext",
        "?",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math group-character content leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math group-character controls should not be reported as unknown or unsupported: {:?}",
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
    assert!(rendered_text.contains("Before x+1 and y After"));
    let passive_strokes = content
        .operations
        .windows(3)
        .filter(|operations| {
            operations[0].operator == "m"
                && operations[1].operator == "l"
                && operations[2].operator == "S"
        })
        .count();
    assert!(
        passive_strokes >= 2,
        "Office math group characters should render as passive line strokes, got {passive_strokes}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mgroupChr",
        b"mgroupChrPr",
        b"mchr",
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
            "Office math group-character control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_group_character_positions_render_passively_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mgroupChr{",
        "\\",
        "mgroupChrPr{",
        "launch.exe objdata 414243 ",
        "\\",
        "u65?",
        "\\",
        "'42",
        "\\",
        "mchr ",
        "\\",
        "u9182?}{",
        "\\",
        "mpos bot}{",
        "https://example.com/payload objdata 444546 ",
        "\\",
        "u67?",
        "\\",
        "'44",
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
        "mtext x+1}}}}} Unsafe {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mgroupChr{",
        "\\",
        "mgroupChrPr{",
        "calc.exe objdata 454647 ",
        "\\",
        "u72?",
        "\\",
        "'49",
        "\\",
        "mchr ",
        "\\",
        "u9183?}{",
        "\\",
        "mpos calc.exe}}{",
        "\\",
        "me{",
        "\\",
        "mtext y}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before x+1 Unsafe y After"),
        "unexpected group-character position text: {text:?}"
    );
    let bottom_style = run_style_for_text(&parsed.document, "x+1").expect("bottom group run");
    assert!(!bottom_style.overline);
    assert_eq!(bottom_style.underline, UnderlineStyle::Single);
    let invalid_position_style =
        run_style_for_text(&parsed.document, "y").expect("invalid-position group run");
    assert_eq!(invalid_position_style.underline, UnderlineStyle::Single);
    for forbidden in [
        "mmath",
        "moMath",
        "mgroupChr",
        "mgroupChrPr",
        "mchr",
        "mpos",
        "bot",
        "calc.exe",
        "launch.exe",
        "example.com/payload",
        "mtext",
        "objdata",
        "414243",
        "444546",
        "454647",
        "?",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math group-character position metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math group-character position controls should not be reported as unknown or unsupported: {:?}",
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
    assert!(rendered_text.contains("Before x+1 Unsafe y After"));
    let passive_strokes = content
        .operations
        .windows(3)
        .filter(|operations| {
            operations[0].operator == "m"
                && operations[1].operator == "l"
                && operations[2].operator == "S"
        })
        .count();
    assert!(
        passive_strokes >= 2,
        "Office math group-character positions should render as passive line strokes, got {passive_strokes}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mgroupChr",
        b"mgroupChrPr",
        b"mchr",
        b"mpos",
        b"bot",
        b"calc.exe",
        b"launch.exe",
        b"example.com/payload",
        b"mtext",
        b"objdata",
        b"414243",
        b"444546",
        b"454647",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math group-character position metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_functions_render_passive_argument_spacing_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mfunc{",
        "\\",
        "mfuncPr{",
        "calc.exe http://example.invalid/payload objdata 444546 ",
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
        "objdata 414243}}",
        "\\",
        "mctrlPr}}{",
        "\\",
        "mfName{",
        "\\",
        "mtext sin}{",
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
        "mtext x}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before sin x After"),
        "unexpected function math text: {text:?}"
    );
    for forbidden in [
        "mmath",
        "moMath",
        "mfunc",
        "mfuncPr",
        "mfName",
        "mctrlPr",
        "mtext",
        "calc.exe",
        "example.invalid",
        "objdata",
        "414243",
        "444546",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math function content leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math function controls should not be reported as unknown or unsupported: {:?}",
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
    assert!(rendered_text.contains("Before sin x After"));
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mfunc",
        b"mfuncPr",
        b"mfName",
        b"mctrlPr",
        b"mtext",
        b"calc.exe",
        b"example.invalid",
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
            "Office math function content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_nary_hidden_limits_are_stripped_passively() {
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
        "msubHide1}{",
        "\\",
        "msupHide1}}{",
        "\\",
        "msub{",
        "\\",
        "mtext HIDDEN-SUB-LIMIT}}{",
        "\\",
        "msup{",
        "\\",
        "mtext HIDDEN-SUP-LIMIT}}{",
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
        text.contains("Before \u{2211}i After"),
        "unexpected hidden n-ary limit text: {text:?}"
    );
    assert!(
        run_style_for_text(&parsed.document, "i")
            .and_then(|style| style.passive_math.nary_base_id)
            .is_some(),
        "n-ary base should retain bounded spacing identity"
    );
    for forbidden in [
        "mmath",
        "moMath",
        "mnary",
        "mnaryPr",
        "msubHide",
        "msupHide",
        "msub",
        "msup",
        "mtext",
        "HIDDEN-SUB-LIMIT",
        "HIDDEN-SUP-LIMIT",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math hidden n-ary limit content leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math hidden n-ary limit controls should not be reported as unknown or unsupported: {:?}",
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
    assert!(rendered_text.contains("i After"));
    assert!(
        pdf_text_font_names(&content)
            .iter()
            .any(|name| name.starts_with(b"TF")),
        "hidden-limit n-ary operator should use a passive supplied font"
    );
    let layout = LayoutEngine::try_layout(&parsed.document)
        .expect("parsed documents satisfy model invariants");
    let operator_position = layout.pages[0]
        .items
        .iter()
        .find_map(|item| match item {
            LayoutItem::Text(fragment) if fragment.text == "\u{2211}" => Some(fragment.x),
            _ => None,
        })
        .expect("n-ary operator position");
    let base_position = layout.pages[0]
        .items
        .iter()
        .find_map(|item| match item {
            LayoutItem::Text(fragment) if fragment.text == "i" => Some(fragment.x),
            _ => None,
        })
        .expect("n-ary base position");
    assert!(
        (9.0..=12.0).contains(&(base_position - operator_position)),
        "n-ary base should retain Word's measured operator-side gap: operator={operator_position:?}, base={base_position:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mnary",
        b"mnaryPr",
        b"msubHide",
        b"msupHide",
        b"mtext",
        b"HIDDEN-SUB-LIMIT",
        b"HIDDEN-SUP-LIMIT",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math hidden n-ary limit content leaked to PDF: {:?}",
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
        text.contains("Before \u{221a}xi2 After"),
        "unexpected property-heavy math text: {text:?}"
    );
    let property_overbar_run = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Paragraph(paragraph) => paragraph.runs.iter().find(|run| run.text.contains('x')),
            _ => None,
        })
        .expect("property overbar run");
    assert!(property_overbar_run.style.overline);
    let property_subscript_style =
        run_style_for_text(&parsed.document, "i").expect("property subscript run");
    assert!(property_subscript_style.baseline_shift_half_points < 0);
    assert!(property_subscript_style.font_size_scale_percent < 100);
    let property_superscript_style =
        run_style_for_text(&parsed.document, "2").expect("property superscript run");
    assert!(property_superscript_style.baseline_shift_half_points > 0);
    assert!(property_superscript_style.font_size_scale_percent < 100);
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
    assert!(rendered_text.contains("xi2 After"));
    assert!(
        pdf_text_font_names(&content)
            .iter()
            .any(|name| name.starts_with(b"TF")),
        "property-heavy radical should use a passive supplied font"
    );
    let helvetica_bytes = pdf_text_bytes_for_font(&content, b"F1");
    assert!(
        !helvetica_bytes.contains(&0xaf),
        "property-heavy overbar should render as passive geometry, not as a macron glyph; got {helvetica_bytes:?}"
    );
    assert!(
        content.operations.windows(3).any(|operations| {
            operations[0].operator == "m"
                && operations[1].operator == "l"
                && operations[2].operator == "S"
        }),
        "property-heavy overbar should render as a passive stroke"
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
fn office_math_delimiters_render_passive_text_without_control_leakage() {
    let input = br"{\rtf1 Before {\mmath{\moMath{\md{\mdPr{\mbegChr (}{\mendChr )}}{\me{\mtext x+1}}}}} After\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before (x+1) After"),
        "unexpected delimiter math text: {text:?}"
    );
    for forbidden in ["mmath", "moMath", "mdPr", "mbegChr", "mendChr", "mtext"] {
        assert!(
            !text.contains(forbidden),
            "Office math delimiter control leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math delimiter controls should not be reported as unknown or unsupported: {:?}",
        parsed.diagnostics
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("Office math rendered as bounded passive math text")
    }));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("Office math layout approximated")
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
    assert!(rendered_text.contains("Before (x+1) After"));
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mdPr",
        b"mbegChr",
        b"mendChr",
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
            "Office math delimiter control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_delimiter_separators_render_between_arguments_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "md{",
        "\\",
        "mdPr{",
        "launch.exe objdata 414243 ",
        "\\",
        "u65?",
        "\\",
        "'42",
        "\\",
        "mbegChr [}{",
        "\\",
        "msepChr ;}{",
        "\\",
        "mendChr ]}{",
        "https://example.com/payload objdata 444546 ",
        "\\",
        "u67?",
        "\\",
        "'44",
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
        "mtext A}}{",
        "\\",
        "me{",
        "\\",
        "mtext B}}}}} After",
        "\\",
        "par Unsafe {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "md{",
        "\\",
        "mdPr{",
        "calc.exe objdata 454647 ",
        "\\",
        "u72?",
        "\\",
        "'49",
        "\\",
        "mbegChr (}{",
        "\\",
        "msepChr calc.exe}{",
        "\\",
        "mendChr )}}{",
        "\\",
        "me{",
        "\\",
        "mtext C}}{",
        "\\",
        "me{",
        "\\",
        "mtext D}}}}} After",
        "\\",
        "par Invalid {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "md{",
        "\\",
        "mdPr{",
        "\\",
        "mbegChr calc.exe}{",
        "\\",
        "msepChr |}{",
        "\\",
        "mendChr launch.exe}}{",
        "\\",
        "me{",
        "\\",
        "mtext E}}{",
        "\\",
        "me{",
        "\\",
        "mtext F}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before [A;B] After"),
        "unexpected delimiter separator text: {text:?}"
    );
    assert!(
        text.contains("Unsafe (C\tD) After"),
        "multi-character delimiter separator should fall back to a passive tab: {text:?}"
    );
    assert!(
        text.contains("Invalid E|F After"),
        "multi-character delimiter begin/end values should be stripped while a safe separator remains: {text:?}"
    );
    for forbidden in [
        "mmath",
        "moMath",
        "mdPr",
        "mbegChr",
        "msepChr",
        "mendChr",
        "mtext",
        "launch.exe",
        "example.com/payload",
        "objdata",
        "414243",
        "444546",
        "454647",
        "calc.exe",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math delimiter separator metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math delimiter separator controls should not be reported as unknown or unsupported: {:?}",
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
    assert!(rendered_text.contains("Before [A;B] After"));
    assert!(
        rendered_text.contains("Unsafe (C\tD) After")
            || rendered_text.contains("Unsafe (CD) After"),
        "multi-character delimiter separator should not leak payload text in PDF: {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("Invalid E|F After"),
        "multi-character delimiter begin/end values should not leak payload text in PDF: {rendered_text:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mdPr",
        b"mbegChr",
        b"msepChr",
        b"mendChr",
        b"mtext",
        b"launch.exe",
        b"example.com/payload",
        b"objdata",
        b"414243",
        b"444546",
        b"454647",
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
            "Office math delimiter separator metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_equation_arrays_render_passive_rows() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "meqArr{",
        "\\",
        "meqArrPr calc.exe objdata 414243}{",
        "\\",
        "me{",
        "\\",
        "mtext X=1}}{",
        "\\",
        "me{",
        "\\",
        "mtext Y=2}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before X=1\nY=2 After"),
        "unexpected equation-array math text: {text:?}"
    );
    let first_row_style = run_style_for_text(&parsed.document, "X=1").expect("first array row");
    let second_row_style = run_style_for_text(&parsed.document, "\nY=2").expect("second array row");
    assert_eq!(
        first_row_style.passive_math.equation_array_id,
        second_row_style.passive_math.equation_array_id
    );
    assert!(first_row_style.passive_math.equation_array_id.is_some());
    assert_eq!(first_row_style.passive_math.equation_array_row, 0);
    assert_eq!(second_row_style.passive_math.equation_array_row, 1);
    for forbidden in [
        "mmath", "moMath", "meqArr", "meqArrPr", "me", "mtext", "calc.exe", "objdata", "414243",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math equation-array control leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math equation-array controls should not be reported as unknown or unsupported: {:?}",
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
    for visible in ["Before ", "X=1", "Y=2", " After"] {
        assert!(
            rendered_text.contains(visible),
            "equation-array visible text missing from PDF text: {visible}; got {rendered_text:?}"
        );
    }
    let x_position = pdf_first_text_position_for_text(&content, "X").expect("X=1 position");
    let y_position = pdf_first_text_position_for_text(&content, "Y").expect("Y=2 position");
    assert!(
        y_position.1 < x_position.1,
        "Office math equation-array rows should render below prior rows: X=1={x_position:?}, Y=2={y_position:?}"
    );
    assert!(
        (y_position.0 - x_position.0).abs() <= 0.1,
        "Office math equation-array rows should share a common horizontal origin: X=1={x_position:?}, Y=2={y_position:?}"
    );
    assert!(
        ((x_position.1 - y_position.1) - 14.0).abs() <= 0.1,
        "Office math equation-array rows should retain Word's measured 14-point baseline pitch: X=1={x_position:?}, Y=2={y_position:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"meqArr",
        b"meqArrPr",
        b"mtext",
        b"calc.exe",
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
            "Office math equation-array control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_phantoms_are_stripped_from_passive_output() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mphant{",
        "\\",
        "mphantPr calc.exe objdata 414243 ",
        "\\",
        "u65?",
        "\\",
        "'42}{",
        "\\",
        "me{",
        "\\",
        "mtext HIDDEN-PHANTOM-PAYLOAD}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let visible_text = text.replace(PASSIVE_ADVANCE_MARKER, "");
    assert!(
        visible_text.contains("Before  After"),
        "visible text around Office math phantom should remain: {text:?}"
    );
    let phantom_advance = run_style_for_text(&parsed.document, PASSIVE_ADVANCE_MARKER)
        .expect("hidden phantom width advance");
    assert!(
        (3_300..=3_500).contains(&phantom_advance.character_spacing_twips),
        "hidden phantom should retain a bounded width estimate without retaining its text: {phantom_advance:?}"
    );
    for forbidden in [
        "mmath",
        "moMath",
        "mphant",
        "mphantPr",
        "mtext",
        "calc.exe",
        "objdata",
        "414243",
        "AB",
        "HIDDEN-PHANTOM-PAYLOAD",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math phantom content leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math phantom controls should not be reported as unknown or unsupported: {:?}",
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
        rendered_text.contains("Before  After"),
        "visible text around Office math phantom missing from PDF text: {rendered_text:?}"
    );
    let layout = LayoutEngine::try_layout(&parsed.document)
        .expect("parsed documents satisfy model invariants");
    let before_x = layout.pages[0]
        .items
        .iter()
        .find_map(|item| match item {
            LayoutItem::Text(fragment) if fragment.text.starts_with("Before") => Some(fragment.x),
            _ => None,
        })
        .expect("text before hidden phantom");
    let after_x = layout.pages[0]
        .items
        .iter()
        .find_map(|item| match item {
            LayoutItem::Text(fragment) if fragment.text.contains("After") => Some(fragment.x),
            _ => None,
        })
        .expect("text after hidden phantom");
    assert!(
        (190.0..=260.0).contains(&(after_x - before_x)),
        "hidden phantom should reserve bounded horizontal flow without rendering its text: before={before_x}, after={after_x}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mphant",
        b"mphantPr",
        b"mtext",
        b"calc.exe",
        b"objdata",
        b"414243",
        b"HIDDEN-PHANTOM-PAYLOAD",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math phantom content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_math_phantom_zero_width_suppresses_hidden_advance() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mphant{",
        "\\",
        "mphantPr{",
        "\\",
        "mshow0}{",
        "\\",
        "mzeroWid1}}{",
        "\\",
        "me{",
        "\\",
        "mtext HIDDEN-PHANTOM-PAYLOAD}}}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before  After"),
        "zero-width hidden phantom should retain surrounding text: {text:?}"
    );
    assert!(!text.contains(PASSIVE_ADVANCE_MARKER));
    assert!(!text.contains("HIDDEN-PHANTOM-PAYLOAD"));
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "zero-width phantom controls should be recognized: {:?}",
        parsed.diagnostics
    );

    let layout = LayoutEngine::try_layout(&parsed.document)
        .expect("parsed documents satisfy model invariants");
    let before_x = layout.pages[0]
        .items
        .iter()
        .find_map(|item| match item {
            LayoutItem::Text(fragment) if fragment.text.starts_with("Before") => Some(fragment.x),
            _ => None,
        })
        .expect("text before zero-width phantom");
    let after_x = layout.pages[0]
        .items
        .iter()
        .find_map(|item| match item {
            LayoutItem::Text(fragment) if fragment.text.contains("After") => Some(fragment.x),
            _ => None,
        })
        .expect("text after zero-width phantom");
    assert!(
        after_x - before_x < 80.0,
        "zero-width phantom should not reserve hidden horizontal flow: before={before_x}, after={after_x}"
    );

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::default()).unwrap();
    assert!(
        !output
            .pdf
            .windows("HIDDEN-PHANTOM-PAYLOAD".len())
            .any(|window| window == b"HIDDEN-PHANTOM-PAYLOAD")
    );
    assert!(
        !output
            .pdf
            .windows(PASSIVE_ADVANCE_MARKER.len())
            .any(|window| window == PASSIVE_ADVANCE_MARKER.as_bytes())
    );
}

#[test]
fn office_math_phantom_mshow_renders_visible_content_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mphant{",
        "\\",
        "mphantPr{",
        "calc.exe objdata 414243 ",
        "\\",
        "u65?",
        "\\",
        "'42",
        "\\",
        "mshow1}{",
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
        "mtext VisiblePhantom}}}}} After",
        "\\",
        "par {",
        "\\",
        "mmath{",
        "\\",
        "moMath{",
        "\\",
        "mphant{",
        "\\",
        "mphantPr{",
        "launch.exe https://example.com/payload objdata 444546 ",
        "\\",
        "u67?",
        "\\",
        "'44",
        "\\",
        "mshow0}}{",
        "\\",
        "me{",
        "\\",
        "mtext HIDDEN-PHANTOM-PAYLOAD}}}}}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(
        text.contains("Before VisiblePhantom After"),
        "visible Office math phantom content should render when mshow is enabled: {text:?}"
    );
    for forbidden in [
        "mmath",
        "moMath",
        "mphant",
        "mphantPr",
        "mshow",
        "mtext",
        "calc.exe",
        "launch.exe",
        "example.com/payload",
        "objdata",
        "414243",
        "444546",
        "AB",
        "CD",
        "HIDDEN-PHANTOM-PAYLOAD",
    ] {
        assert!(
            !text.contains(forbidden),
            "Office math phantom metadata or hidden payload leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unknown RTF destination")
            && !diagnostic.message.contains("unsupported RTF control")),
        "Office math phantom mshow controls should not be reported as unknown or unsupported: {:?}",
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
        rendered_text.contains("Before VisiblePhantom After"),
        "visible Office math phantom content missing from PDF text: {rendered_text:?}"
    );
    for forbidden in [
        b"mmath".as_slice(),
        b"moMath",
        b"mphant",
        b"mphantPr",
        b"mshow",
        b"mtext",
        b"calc.exe",
        b"launch.exe",
        b"example.com/payload",
        b"objdata",
        b"414243",
        b"444546",
        b"HIDDEN-PHANTOM-PAYLOAD",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Office math phantom metadata or payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_chord_math_tab_and_gear_shapes_render_passively_without_payload_leakage() {
    let mut input = String::from("{\\rtf1 Before\\par");
    for (shape_type, payload) in [
        (161, "office-chord-payload"),
        (162, "office-corner-payload"),
        (163, "office-math-plus-payload"),
        (164, "office-math-minus-payload"),
        (165, "office-math-multiply-payload"),
        (166, "office-math-divide-payload"),
        (167, "office-math-equal-payload"),
        (168, "office-math-not-equal-payload"),
        (169, "office-corner-tabs-payload"),
        (170, "office-square-tabs-payload"),
        (171, "office-plaque-tabs-payload"),
        (172, "office-gear-6-payload"),
        (173, "office-gear-9-payload"),
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
    assert_eq!(shapes.len(), 13);
    let expected_point_counts = [45, 6, 12, 4, 12, 4, 8, 20, 6, 6, 8, 24, 36];
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
            "chord/math/tab/gear points must stay inside passive frame: {shape:?}"
        );
    }
    assert!(shapes[0].points.iter().any(|point| point.y_twips == 0));
    assert!(
        shapes[0]
            .points
            .iter()
            .filter(|point| {
                point.y_twips <= shapes[0].height_twips / 3
                    && point.x_twips >= (shapes[0].width_twips * 4) / 5
            })
            .count()
            >= 7,
        "chord should preserve a denser bounded passive upper-right shoulder curve"
    );
    assert!(
        shapes[0]
            .points
            .iter()
            .filter(|point| {
                point.y_twips <= shapes[0].height_twips / 3
                    && point.x_twips <= shapes[0].width_twips / 5
            })
            .count()
            >= 7,
        "chord should preserve a denser bounded passive upper-left shoulder curve"
    );
    assert_eq!(shapes[1].points[0].x_twips, 0);
    assert_eq!(
        shapes[5].point_paths.len(),
        2,
        "division sign should keep rounded passive dots as separate subpaths without duplicating the bar"
    );
    assert_eq!(
        shapes[5]
            .point_paths
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>(),
        vec![16, 16],
        "division sign should preserve rounded passive dot paths"
    );
    assert!(
        shapes[5].point_paths[0]
            .iter()
            .any(|point| point.y_twips <= shapes[5].height_twips / 8)
            && shapes[5].point_paths[1]
                .iter()
                .any(|point| point.y_twips >= (shapes[5].height_twips * 7) / 8),
        "division sign should preserve passive rounded dot extrema"
    );
    assert_eq!(
        shapes[6].point_paths.len(),
        2,
        "equals sign should keep separate passive bar paths"
    );
    assert_eq!(
        shapes[7].point_paths.len(),
        3,
        "not-equals sign should keep separate passive bar/slash paths"
    );
    for shape in [&shapes[5], &shapes[6], &shapes[7]] {
        assert!(
            shape.point_paths.iter().flatten().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "compound math shape paths must stay inside passive frame: {shape:?}"
        );
    }
    assert_eq!(
        shapes[11].fill_rule,
        StaticImageVectorFillRule::Alternate,
        "gear 6 should use even-odd fill for its passive center hole"
    );
    assert_eq!(
        shapes[12].fill_rule,
        StaticImageVectorFillRule::Alternate,
        "gear 9 should use even-odd fill for its passive center hole"
    );
    assert_eq!(shapes[11].point_paths.len(), 1);
    assert_eq!(shapes[11].point_paths[0].len(), 24);
    assert_eq!(shapes[12].point_paths.len(), 1);
    assert_eq!(shapes[12].point_paths[0].len(), 36);
    assert!(
        shapes[11]
            .points
            .iter()
            .filter(|point| {
                point.x_twips <= shapes[11].width_twips / 8
                    || point.x_twips >= shapes[11].width_twips - (shapes[11].width_twips / 8)
                    || point.y_twips <= shapes[11].height_twips / 8
                    || point.y_twips >= shapes[11].height_twips - (shapes[11].height_twips / 8)
            })
            .count()
            >= 6,
        "gear 6 should preserve bounded passive tooth tips around the outer edge"
    );
    assert!(
        shapes[12]
            .points
            .iter()
            .filter(|point| {
                point.x_twips <= shapes[12].width_twips / 8
                    || point.x_twips >= shapes[12].width_twips - (shapes[12].width_twips / 8)
                    || point.y_twips <= shapes[12].height_twips / 8
                    || point.y_twips >= shapes[12].height_twips - (shapes[12].height_twips / 8)
            })
            .count()
            >= 9,
        "gear 9 should preserve bounded passive tooth tips around the outer edge"
    );
    for shape in [&shapes[11], &shapes[12]] {
        assert!(
            shape.point_paths.iter().flatten().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "gear hole paths must stay inside passive frame: {shape:?}"
        );
    }
    assert_eq!(
        shapes[8].point_paths.len(),
        1,
        "corner tabs should keep the disconnected tab as a separate passive subpath"
    );
    assert_eq!(shapes[8].point_paths[0].len(), 6);
    assert!(
        shapes[8].point_paths.iter().flatten().all(|point| {
            point.x_twips >= 0
                && point.x_twips <= shapes[8].width_twips
                && point.y_twips >= 0
                && point.y_twips <= shapes[8].height_twips
        }),
        "corner tab subpath must stay inside passive frame"
    );
    assert_eq!(
        shapes[9].point_paths.len(),
        3,
        "square tabs should keep disconnected tab runs as separate passive subpaths"
    );
    assert!(
        shapes[9].point_paths.iter().all(|path| path.len() == 6),
        "square tab subpaths should preserve bounded six-point tab runs"
    );
    assert!(
        shapes[9].point_paths.iter().flatten().all(|point| {
            point.x_twips >= 0
                && point.x_twips <= shapes[9].width_twips
                && point.y_twips >= 0
                && point.y_twips <= shapes[9].height_twips
        }),
        "square tab subpaths must stay inside passive frame"
    );
    assert_eq!(
        shapes[10].point_paths.len(),
        3,
        "plaque tabs should keep side tab runs as separate passive subpaths"
    );
    assert_eq!(
        shapes[10]
            .point_paths
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>(),
        vec![6, 10, 8]
    );
    assert!(
        shapes[10].point_paths.iter().flatten().all(|point| {
            point.x_twips >= 0
                && point.x_twips <= shapes[10].width_twips
                && point.y_twips >= 0
                && point.y_twips <= shapes[10].height_twips
        }),
        "plaque tab subpaths must stay inside passive frame"
    );
    assert!(shapes[11].points.iter().any(|point| point.x_twips == 0));
    assert!(
        shapes[12]
            .points
            .iter()
            .any(|point| point.y_twips == shapes[12].height_twips)
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "office-chord-payload",
        "office-corner-payload",
        "office-math-plus-payload",
        "office-math-minus-payload",
        "office-math-multiply-payload",
        "office-math-divide-payload",
        "office-math-equal-payload",
        "office-math-not-equal-payload",
        "office-corner-tabs-payload",
        "office-square-tabs-payload",
        "office-plaque-tabs-payload",
        "office-gear-6-payload",
        "office-gear-9-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "chord/math/tab/gear metadata leaked to normalized text: {forbidden}"
        );
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
        passive_shape_paints >= 13,
        "chord/math/tab/gear shapes should render passive fill/stroke paths"
    );
    assert!(
        passive_even_odd_shape_paints >= 2,
        "gear shapes should render as passive even-odd compound paths"
    );
    assert!(
        passive_path_moves >= 29,
        "compound math, tab, and gear shapes should emit separate passive PDF subpaths"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"office-chord-payload",
        b"office-corner-payload",
        b"office-math-plus-payload",
        b"office-math-minus-payload",
        b"office-math-multiply-payload",
        b"office-math-divide-payload",
        b"office-math-equal-payload",
        b"office-math-not-equal-payload",
        b"office-corner-tabs-payload",
        b"office-square-tabs-payload",
        b"office-plaque-tabs-payload",
        b"office-gear-6-payload",
        b"office-gear-9-payload",
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
            "chord/math/tab/gear metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}
