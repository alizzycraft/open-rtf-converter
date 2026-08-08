use super::*;

#[test]
fn malformed_hex_escapes_are_typed_errors() {
    let parsed = parse_rtf_bytes(&rtf(&[
        "{", "\\", "rtf1 ", "\\", "'41", "\\", "'42", "\\", "'43}",
    ]))
    .unwrap();
    assert!(collect_text(&parsed.document).contains("ABC"));

    for input in [
        rtf(&["{", "\\", "rtf1 ", "\\", "'}"]),
        rtf(&["{", "\\", "rtf1 ", "\\", "'GZ}"]),
        rtf(&["{", "\\", "rtf1 ", "\\", "'1}"]),
    ] {
        assert!(matches!(
            parse_rtf_bytes(&input),
            Err(ParseError::Lex(LexError::MalformedHexEscape(_)))
        ));
    }
}

#[test]
fn cyrillic_charset_text_uses_bundled_serif_asset() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "ansicpg1252{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Times New Roman;}{",
        "\\",
        "f38",
        "\\",
        "fcharset204 Times New Roman Cyr;}}",
        "\\",
        "f38 ",
        "\\",
        "'cf",
        "\\",
        "'f0",
        "\\",
        "'e8",
        "\\",
        "'e2",
        "\\",
        "'e5",
        "\\",
        "'f2",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("\u{041f}\u{0440}\u{0438}\u{0432}\u{0435}\u{0442}"));
    assert!(!text.contains("fcharset204"));
    assert!(!text.contains("Times New Roman Cyr"));

    let mut options = ConvertOptions::browser_safe_defaults();
    options.diagnostics = true;
    let output = convert_rtf_to_pdf(&input, &options).unwrap();
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
    assert!(
        output.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("Cyrillic characters for font 'Times New Roman Cyr'")),
        "covered Cyrillic glyphs should render through embedded passive Type0 fonts without a degradation warning: {:?}",
        output.diagnostics
    );

    for forbidden in [
        b"Times New Roman Cyr".as_slice(),
        b"fcharset204".as_slice(),
        b"ansicpg1251".as_slice(),
        b"/JavaScript".as_slice(),
        b"/EmbeddedFile".as_slice(),
        b"/Launch".as_slice(),
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden Cyrillic/source content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn central_european_charset_text_uses_bundled_serif_asset() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "ansicpg1252{",
        "\\",
        "fonttbl{",
        "\\",
        "f37",
        "\\",
        "fcharset238 Times New Roman CE;}}",
        "\\",
        "f37 ",
        "\\",
        "'e1rv",
        "\\",
        "'edzt",
        "\\",
        "'fbr",
        "\\",
        "'f5",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("\u{00e1}rv\u{00ed}zt\u{0171}r\u{0151}"));
    assert!(!text.contains("fcharset238"));
    assert!(!text.contains("Times New Roman CE"));

    let mut options = ConvertOptions::browser_safe_defaults();
    options.diagnostics = true;
    let output = convert_rtf_to_pdf(&input, &options).unwrap();
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
    assert!(
        output.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("Latin Extended characters for font 'Times New Roman CE'")),
        "covered Central European glyphs should render through embedded passive Type0 fonts without a degradation warning: {:?}",
        output.diagnostics
    );
    assert!(
        !output
            .pdf
            .windows(b"OpenRtfConverter-ExtendedLatin".len())
            .any(|window| window == b"OpenRtfConverter-ExtendedLatin"),
        "bundled serif asset should avoid the base-font Extended Latin CMap"
    );

    for forbidden in [
        b"Times New Roman CE".as_slice(),
        b"fcharset238".as_slice(),
        b"ansicpg1250".as_slice(),
        b"/JavaScript".as_slice(),
        b"/EmbeddedFile".as_slice(),
        b"/Launch".as_slice(),
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden Central European/source content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn bundled_fonts_cover_latin_extended_text_without_control_leakage() {
    let input = r"{\rtf1\ansi{\fonttbl{\f0\froman Unknown Word Serif;}}\f0 Unsupported \u384?\par}"
        .as_bytes()
        .to_vec();
    let mut options = ConvertOptions::browser_safe_defaults();
    options.diagnostics = true;
    let output = convert_rtf_to_pdf(&input, &options).unwrap();

    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
    assert!(output.diagnostics.iter().all(|diagnostic| {
        !diagnostic.message.contains("Latin Extended characters")
            || !diagnostic.message.contains("passive font asset support")
    }));
    for forbidden in [
        b"fonttbl".as_slice(),
        b"Unknown Word Serif",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden unsupported Latin Extended/source content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn bundled_fonts_cover_greek_text_without_control_leakage() {
    let input =
        r"{\rtf1\ansi{\fonttbl{\f0\fswiss Unknown Word Sans;}}\f0 Unsupported \u945?\u7936?\par}"
            .as_bytes()
            .to_vec();
    let mut options = ConvertOptions::browser_safe_defaults();
    options.diagnostics = true;
    let output = convert_rtf_to_pdf(&input, &options).unwrap();

    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
    assert!(output.diagnostics.iter().all(|diagnostic| {
        !diagnostic.message.contains("Greek characters")
            || !diagnostic.message.contains("passive font asset support")
    }));
    for forbidden in [
        b"fonttbl".as_slice(),
        b"Unknown Word Sans",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden unsupported Greek/source content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn invalid_caller_font_assets_are_rejected_before_pdf_rendering() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi{",
        "\\",
        "fonttbl{",
        "\\",
        "f38",
        "\\",
        "fcharset204 Times New Roman Cyr;}}",
        "\\",
        "f38 ",
        "\\",
        "'cf",
        "\\",
        "'f0",
        "\\",
        "'e8",
        "\\",
        "'e2",
        "\\",
        "'e5",
        "\\",
        "'f2",
        "\\",
        "par}",
    ]);
    let font_bytes = b"not-a-real-font-but-hostile-private-data".to_vec();
    let mut options = ConvertOptions::browser_safe_defaults();
    options.diagnostics = true;
    options.font_provider = FontProvider {
        assets: vec![FontAsset {
            family_names: vec!["Times New Roman Cyr".to_string()],
            style: FontAssetStyle::default(),
            bytes: font_bytes.clone(),
        }],
        ..FontProvider::browser_safe_defaults()
    };

    let error = convert_rtf_to_pdf(&input, &options).unwrap_err();
    assert!(matches!(
        error,
        open_rtf_converter::ConvertError::FontProvider(
            open_rtf_converter::FontProviderError::InvalidAsset { .. }
        )
    ));
}

#[test]
fn escaped_control_symbols_in_hidden_text_do_not_cross_pdf_boundary() {
    let input = br"{\rtf1 Visible {\v \~ \- \_ \\ \{ \}}{\v0 shown}\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible"));
    assert!(text.contains("shown"));
    for forbidden in ["\u{00a0}", "\u{00ad}", "\u{2011}", "\\", "{", "}"] {
        assert!(
            !text.contains(forbidden),
            "hidden escaped symbol leaked to normalized text: {forbidden:?}"
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

    assert!(rendered_text.contains("Visible"));
    assert!(rendered_text.contains("shown"));
    for forbidden in ["\u{00a0}", "\u{00ad}", "\u{2011}", "\\", "{", "}"] {
        assert!(
            !rendered_text.contains(forbidden),
            "hidden escaped symbol leaked to decoded PDF text: {forbidden:?}"
        );
    }
    for forbidden in [
        b"/JavaScript".as_slice(),
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden hidden escaped symbol content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn unicode_stylesheet_name_terminator_ignores_trailing_text_without_pdf_leakage() {
    let input = br#"{\rtf1{\stylesheet{\s1 Code\u65? Style\u59?Ignored;}}\s1 Visible title\par\pard Ref {\field{\*\fldinst STYLEREF "Code\u65? Style" \\* Upper}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible title"));
    assert!(text.contains("Ref VISIBLE TITLE"));
    for forbidden in [
        "STYLEREF",
        "CodeA Style",
        "Ignored",
        "u65",
        "u59",
        "?",
        "fldinst",
        "stylesheet",
    ] {
        assert!(
            !text.contains(forbidden),
            "Unicode stylesheet terminator leaked unsafe text: {forbidden}"
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
        rendered_text.contains("Ref VISIBLE TITLE"),
        "decoded PDF text did not contain Unicode-terminated passive STYLEREF value: {rendered_text:?}"
    );
    for forbidden in [
        b"STYLEREF".as_slice(),
        b"CodeA Style",
        b"Ignored",
        b"u65",
        b"u59",
        b"fldinst",
        b"stylesheet",
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
            "forbidden Unicode stylesheet terminator content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn quoted_symbol_payload_does_not_act_as_font_switch() {
    let input =
        br#"{\rtf1 Before {\field{\*\fldinst SYMBOL 65 "literal \\f Symbol"}} After\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains("Before A After"),
        "quoted fake SYMBOL switch should not affect passive output, got {text:?}"
    );
    assert!(!text.contains("literal"));
    assert!(!text.contains("SYMBOL"));
    assert!(!text.contains("fldinst"));

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
    assert!(!rendered_text.contains("literal"));
    for forbidden in [
        b"SYMBOL".as_slice(),
        b"literal",
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
            "forbidden quoted-symbol content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn empty_stored_symbol_result_falls_back_to_passive_symbol_rendering() {
    let input =
        br#"{\rtf1{\fonttbl{\f0 Arial;}{\f14 Wingdings;}}\f0 Before ({\field{\*\fldinst SYMBOL 74 \\f "Wingdings" \\s 12}{\fldrslt\f14\fs24}}) After\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains("Before (\u{263a}) After"),
        "empty stored SYMBOL result should render a passive Unicode dingbat result, got {text:?}"
    );
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        other => panic!("expected paragraph with passive SYMBOL field, got {other:?}"),
    };
    let symbol_run = paragraph
        .runs
        .iter()
        .find(|run| run.text == "\u{263a}")
        .expect("passive SYMBOL field run");
    assert_eq!(symbol_run.style.font_size_half_points, 24);
    for forbidden in ["fldinst", "fldrslt", "SYMBOL", "Wingdings"] {
        assert!(
            !text.contains(forbidden),
            "forbidden empty-result symbol metadata leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("rendering passive field SYMBOL without executing field instruction")
    }));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("empty-symbol-result.rtf");
    let output_path = dir.path().join("empty-symbol-result.pdf");
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
        !pdf.windows(b"/BaseFont /ZapfDingbats".len())
            .any(|window| window == b"/BaseFont /ZapfDingbats"),
        "empty-result Wingdings smiley should not require a viewer ZapfDingbats font"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "c"),
        "empty-result Wingdings smiley should draw a passive vector overlay for viewer-stable output"
    );

    for forbidden in [
        b"Wingdings".as_slice(),
        b"fldinst",
        b"fldrslt",
        b"SYMBOL",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/AcroForm",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden empty-result symbol content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn unquoted_symbol_font_switch_renders_passive_dingbat_without_metadata_leakage() {
    let input =
        br"{\rtf1 Before {\field{\*\fldinst SYMBOL 74 \\f Wingdings \\s 12}} After\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains("Before \u{263a} After"),
        "unquoted Wingdings SYMBOL field should render a passive dingbat, got {text:?}"
    );
    for forbidden in ["fldinst", "SYMBOL", "Wingdings"] {
        assert!(
            !text.contains(forbidden),
            "forbidden unquoted-symbol metadata leaked to text: {forbidden}"
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
    assert!(
        !output
            .pdf
            .windows(b"/BaseFont /ZapfDingbats".len())
            .any(|window| window == b"/BaseFont /ZapfDingbats"),
        "unquoted Wingdings smiley should not require a viewer ZapfDingbats font"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "c"),
        "unquoted Wingdings smiley should draw a passive vector overlay for viewer-stable output"
    );
    for forbidden in [
        b"Wingdings".as_slice(),
        b"fldinst",
        b"SYMBOL",
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
            "forbidden unquoted-symbol content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn extreme_font_sizes_are_bounded_before_pdf_rendering() {
    let input = rtf(&["{", "\\", "rtf1 tiny ", "\\", "fs999999 huge", "\\", "par}"]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("tiny"));
    assert!(text.contains("huge"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("font-size.rtf");
    let output_path = dir.path().join("font-size.pdf");
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
fn symbol_font_charset_renders_passive_unicode_without_rtf_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Arial;}{",
        "\\",
        "f1",
        "\\",
        "fcharset2 Symbol;}}",
        "\\",
        "f1 ab ",
        "\\",
        "'b7",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("\u{03b1}\u{03b2} \u{2022}"));
    assert!(!text.contains("fcharset"));
    assert!(!text.contains("fonttbl"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("symbol-font.rtf");
    let output_path = dir.path().join("symbol-font.pdf");
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
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        pdf_text_font_names(&content)
            .iter()
            .any(|name| name.starts_with(b"TF")),
        "Symbol text should use a passive supplied font resource"
    );
    for forbidden in [
        b"fcharset".as_slice(),
        b"fonttbl",
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
fn winansi_hex_ellipsis_renders_through_normal_text_font() {
    let input = br"{\rtf1\ansi\ansicpg1252{\fonttbl{\f0\froman Times New Roman;}{\f1\fcharset2 Symbol;}}\f0 Normal \'85\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Normal \u{2026}"));
    for forbidden in ["ansicpg", "fonttbl", "Symbol", "Times New Roman"] {
        assert!(
            !text.contains(forbidden),
            "forbidden font/codepage metadata leaked to text: {forbidden}"
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
    assert!(decoded_pdf_text(&content).contains("Normal "));
    for forbidden in [
        b"ansicpg".as_slice(),
        b"fonttbl",
        b"Times New Roman",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden WinAnsi ellipsis metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wingdings_checkbox_glyphs_render_passively_without_font_payload_leakage() {
    let input = br#"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Wingdings;}}\f1 \'a3 \'fe \'fc \'fb \'fd \u61693?\par {\field{\*\fldinst SYMBOL 254 \\f "Wingdings"}} {\field{\*\fldinst SYMBOL 253 \\f "Wingdings"}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("\u{25c9} \u{2611} \u{2713} \u{2717} \u{2612} \u{2612}"));
    assert!(text.contains("\u{2611}"));
    assert!(text.contains("\u{2612}"));
    for forbidden in ["fonttbl", "Wingdings", "fldinst", "SYMBOL"] {
        assert!(
            !text.contains(forbidden),
            "forbidden Wingdings metadata leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("wingdings-checkboxes.rtf");
    let output_path = dir.path().join("wingdings-checkboxes.pdf");
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
    assert_passive_checkbox_vectors_without_zapf(&pdf, &content, "Wingdings checkbox glyphs");
    for forbidden in [
        b"Wingdings".as_slice(),
        b"fonttbl",
        b"fldinst",
        b"SYMBOL",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden Wingdings content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn associated_font_checkbox_glyphs_render_passively_without_font_payload_leakage() {
    let input =
        br#"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Wingdings;}}\loch\af1\afs72 \'a3 \'fe \'fc \'fb\par}"#
            .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("\u{25c9} \u{2611} \u{2713} \u{2717}"));
    let paragraph = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .expect("paragraph");
    assert_eq!(paragraph.runs[0].style.font_index, 1);
    assert_eq!(paragraph.runs[0].style.font_size_half_points, 72);
    for forbidden in ["fonttbl", "Wingdings", "loch", "af1", "afs72"] {
        assert!(
            !text.contains(forbidden),
            "forbidden associated font metadata leaked to text: {forbidden}"
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
            operation.operator == "re"
                && operation
                    .operands
                    .get(2)
                    .and_then(pdf_operand_number)
                    .is_some_and(|size| size > 18.0)
        }),
        "associated font size should render large passive checkbox vector boxes"
    );
    assert_passive_checkbox_vectors_without_zapf(
        &output.pdf,
        &content,
        "associated-font checkbox glyphs",
    );
    for forbidden in [
        b"Wingdings".as_slice(),
        b"fonttbl",
        b"loch",
        b"af1",
        b"afs72",
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
            "forbidden associated font content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wingdings2_checkbox_glyphs_render_passively_without_font_payload_leakage() {
    let input = br#"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Wingdings 2;}}\f1 O P Q R S T\par {\field{\*\fldinst SYMBOL 82 \\f "Wingdings 2"}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("\u{2717} \u{2713} \u{2612} \u{2611} \u{1f5f5} \u{1f5f7}"));
    assert!(text.contains("\u{2611}"));
    for forbidden in ["fonttbl", "Wingdings 2", "fldinst", "SYMBOL"] {
        assert!(
            !text.contains(forbidden),
            "forbidden Wingdings 2 metadata leaked to text: {forbidden}"
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
    assert_passive_checkbox_vectors_without_zapf(&output.pdf, &content, "Wingdings 2 checkboxes");
    for forbidden in [
        b"Wingdings 2".as_slice(),
        b"fonttbl",
        b"fldinst",
        b"SYMBOL",
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
            "forbidden Wingdings 2 content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn wingdings3_basic_arrows_render_passively_without_font_payload_leakage() {
    let input = br#"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Wingdings 3;}}\f1 f g h i\par {\field{\*\fldinst SYMBOL 102 \\f "Wingdings 3"}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("\u{2190} \u{2192} \u{2191} \u{2193}"));
    assert!(text.contains("\u{2190}"));
    for forbidden in ["fonttbl", "Wingdings 3", "fldinst", "SYMBOL"] {
        assert!(
            !text.contains(forbidden),
            "forbidden Wingdings 3 metadata leaked to text: {forbidden}"
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
        b"Wingdings 3".as_slice(),
        b"fonttbl",
        b"fldinst",
        b"SYMBOL",
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
            "forbidden Wingdings 3 content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn webdings_checkbox_glyphs_render_passively_without_font_payload_leakage() {
    let input = br#"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Webdings;}}\f1 \'3f \'61 \'63\par {\field{\*\fldinst SYMBOL 63 \\f "Webdings"}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("\u{1f4e5} \u{2714} \u{25a1}"));
    assert!(text.contains("\u{1f4e5}"));
    for forbidden in ["fonttbl", "Webdings", "fldinst", "SYMBOL"] {
        assert!(
            !text.contains(forbidden),
            "forbidden Webdings metadata leaked to text: {forbidden}"
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
    assert_passive_checkbox_vectors_without_zapf(&output.pdf, &content, "Webdings checkboxes");
    for forbidden in [
        b"Webdings".as_slice(),
        b"fonttbl",
        b"fldinst",
        b"SYMBOL",
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
            "forbidden Webdings content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn unicode_checkbox_glyphs_use_passive_checkbox_font_without_embedding_source_font() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Arial;}{",
        "\\",
        "f1 Segoe UI Symbol;}}",
        "\\",
        "f1 ",
        "\\",
        "u9744?",
        "\\",
        "u9745?",
        "\\",
        "u9746?",
        "\\",
        "u10003?",
        "\\",
        "u10007?",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("\u{2610}\u{2611}\u{2612}\u{2713}\u{2717}"));
    for forbidden in ["fonttbl", "Segoe UI Symbol"] {
        assert!(
            !text.contains(forbidden),
            "forbidden Unicode checkbox font metadata leaked to text: {forbidden}"
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
    assert_passive_checkbox_vectors_without_zapf(&output.pdf, &content, "Unicode checkboxes");
    for forbidden in [
        b"Segoe UI Symbol".as_slice(),
        b"fonttbl",
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
            "forbidden Unicode checkbox content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn segoe_ui_symbol_latin_text_stays_readable_while_checkboxes_use_passive_font() {
    let input =
        br"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Segoe UI Symbol;}}\f1 Label \u9745?\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Label"));
    assert!(text.contains("\u{2611}"));
    for forbidden in ["fonttbl", "Segoe UI Symbol", "u9745"] {
        assert!(
            !text.contains(forbidden),
            "forbidden Segoe UI Symbol metadata leaked to text: {forbidden}"
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
    assert!(decoded_pdf_text(&content).contains("Label "));
    assert_passive_checkbox_vectors_without_zapf(&output.pdf, &content, "Segoe UI Symbol checkbox");
    for forbidden in [
        b"Segoe UI Symbol".as_slice(),
        b"fonttbl",
        b"u9745",
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
            "forbidden Segoe UI Symbol content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn unicode_symbol_spans_use_passive_symbol_font_without_source_font_payload() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Arial;}}Alpha ",
        "\\",
        "u945?+",
        "\\",
        "u946? ",
        "\\",
        "u8804? ",
        "\\",
        "u937? done",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Alpha \u{03b1}+\u{03b2} \u{2264} \u{03a9} done"));
    for forbidden in ["fonttbl", "u945", "u946", "u8804", "u937"] {
        assert!(
            !text.contains(forbidden),
            "forbidden Unicode symbol source leaked to text: {forbidden}"
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
        "text should use passive supplied fonts; font selections were {font_names:?}"
    );
    assert!(decoded_pdf_text(&content).contains("Alpha "));
    for forbidden in [
        b"fonttbl".as_slice(),
        b"u945",
        b"u946",
        b"u8804",
        b"u937",
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
            "forbidden Unicode symbol content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn extended_unicode_math_symbols_use_passive_symbol_encoding() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Extended ",
        "\\",
        "u177?",
        "\\",
        "u215?",
        "\\",
        "u8734?",
        "\\",
        "u8706?",
        "\\",
        "u8711?",
        "\\",
        "u8712?",
        "\\",
        "u8745?",
        "\\",
        "u8746?",
        "\\",
        "u8834?",
        "\\",
        "u8835?",
        "\\",
        "u8853?",
        "\\",
        "u8855?",
        "\\",
        "u8596?",
        "\\",
        "u8658?",
        "\\",
        "u172?",
        "\\",
        "u8743?",
        "\\",
        "u8744? done",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains(
        "Extended \u{00b1}\u{00d7}\u{221e}\u{2202}\u{2207}\u{2208}\u{2229}\u{222a}\u{2282}\u{2283}\u{2295}\u{2297}\u{2194}\u{21d2}\u{00ac}\u{2227}\u{2228} done"
    ));

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
    assert!(decoded_pdf_text(&content).contains("Extended "));
    for forbidden in [
        b"u8734".as_slice(),
        b"u8706",
        b"u8711",
        b"u8853",
        b"u8855",
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
            "forbidden extended symbol content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn legacy_symbol_font_glyphs_use_passive_symbol_encoding_without_embedding_font_data() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Arial;}{",
        "\\",
        "f1",
        "\\",
        "fcharset2 Symbol;}}Symbol ",
        "\\",
        "f1 ",
        "\\",
        "'5c ",
        "\\",
        "'5e ",
        "\\",
        "'bd ",
        "\\",
        "'be ",
        "\\",
        "'d2 ",
        "\\",
        "'d3 ",
        "\\",
        "'d4 ",
        "\\",
        "'e6 ",
        "\\",
        "'e7 ",
        "\\",
        "'e8 ",
        "\\",
        "'e9 ",
        "\\",
        "'ea ",
        "\\",
        "'eb ",
        "\\",
        "'ec ",
        "\\",
        "'ed ",
        "\\",
        "'ee ",
        "\\",
        "'ef ",
        "\\",
        "'f0 ",
        "\\",
        "'f4 ",
        "\\",
        "'f5 ",
        "\\",
        "'f6 ",
        "\\",
        "'f7 ",
        "\\",
        "'f8 ",
        "\\",
        "'f9 ",
        "\\",
        "'fa ",
        "\\",
        "'fb ",
        "\\",
        "'fc ",
        "\\",
        "'fd ",
        "\\",
        "'fe ",
        "\\",
        "'ff",
        "\\",
        "f0 done",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    for expected in [
        '\u{2234}', '\u{22a5}', '\u{23d0}', '\u{23af}', '\u{00ae}', '\u{00a9}', '\u{2122}',
        '\u{239b}', '\u{239c}', '\u{239d}', '\u{23a1}', '\u{23a2}', '\u{23a3}', '\u{23a7}',
        '\u{23a8}', '\u{23a9}', '\u{23aa}', '\u{20ac}', '\u{23ae}', '\u{239e}', '\u{239f}',
        '\u{23a0}', '\u{23a4}', '\u{23a5}', '\u{23a6}', '\u{23ab}', '\u{23ac}', '\u{23ad}',
    ] {
        assert!(
            text.contains(expected),
            "legacy Symbol glyph {expected:?} was not normalized into safe text: {text:?}"
        );
    }
    for forbidden in ["fonttbl", "fcharset2", "'5c", "'ff"] {
        assert!(
            !text.contains(forbidden),
            "forbidden legacy Symbol source leaked to text: {forbidden}"
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
        pdf_text_font_names(&content)
            .iter()
            .any(|name| name.starts_with(b"TF")),
        "legacy Symbol glyphs should use a passive supplied font"
    );
    assert!(rendered_text.contains("Symbol "));
    assert!(rendered_text.contains("done"));
    for forbidden in [
        b"fonttbl".as_slice(),
        b"fcharset2",
        b"fontemb",
        b"fontfile",
        b"/EmbeddedFile",
        b"/JavaScript",
        b"/Launch",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden legacy Symbol content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn font_family_hints_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0",
        "\\",
        "froman Mystery Serif;}{",
        "\\",
        "f1",
        "\\",
        "fmodern Mystery Mono;}}",
        "\\",
        "f0 Roman ",
        "\\",
        "f1 Modern",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Roman"));
    assert!(text.contains("Modern"));
    assert!(!text.contains("froman"));
    assert!(!text.contains("fmodern"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("font-family-hints.rtf");
    let output_path = dir.path().join("font-family-hints.pdf");
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
        b"fonttbl".as_slice(),
        b"froman",
        b"fmodern",
        b"Mystery Serif",
        b"Mystery Mono",
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
fn theme_font_hints_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0",
        "\\",
        "flomajor Mystery Heading;}{",
        "\\",
        "f1",
        "\\",
        "fhiminor Mystery Body;}}",
        "\\",
        "f0 Major ",
        "\\",
        "f1 Minor",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let heading_font = parsed
        .document
        .fonts
        .iter()
        .find(|font| font.name == "Mystery Heading")
        .expect("theme heading font");
    let body_font = parsed
        .document
        .fonts
        .iter()
        .find(|font| font.name == "Mystery Body")
        .expect("theme body font");

    assert_eq!(heading_font.family, FontFamilyHint::Roman);
    assert_eq!(body_font.family, FontFamilyHint::Swiss);
    assert!(text.contains("Major Minor"));
    for forbidden in ["fonttbl", "flomajor", "fhiminor", "Mystery Heading"] {
        assert!(
            !text.contains(forbidden),
            "theme font metadata leaked to text: {forbidden}"
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
        "major theme font should use a passive supplied font resource; font selections were {font_names:?}"
    );
    assert!(
        font_names.iter().any(|name| name.starts_with(b"TF")),
        "minor theme font should use a passive supplied font resource; font selections were {font_names:?}"
    );
    for forbidden in [
        b"fonttbl".as_slice(),
        b"flomajor",
        b"fhiminor",
        b"Mystery Heading",
        b"Mystery Body",
        b"fontemb",
        b"fontfile",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden theme font content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_font_names_substitute_to_passive_base14_without_font_payload() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Calibri;}{",
        "\\",
        "f1 Cambria;}{",
        "\\",
        "f2 Aptos Mono;}{",
        "\\",
        "f3 MS Sans Serif;}{",
        "\\",
        "f4 MS Serif;}{",
        "\\",
        "f5 Wingdings;}{",
        "\\",
        "f6 Times New Roman;}{",
        "\\",
        "f7",
        "\\",
        "fcharset238 Times New Roman CE;}{",
        "\\",
        "f8 Arial;}{",
        "\\",
        "f9 Courier New;}}",
        "\\",
        "f0 Sans ",
        "\\",
        "f1 Serif ",
        "\\",
        "f2 Mono ",
        "\\",
        "f3 LegacySans ",
        "\\",
        "f4 LegacySerif ",
        "\\",
        "f5 Wing",
        "\\",
        "f6 DirectTimes ",
        "\\",
        "f7 CharsetTimes ",
        "\\",
        "f8 DirectSans ",
        "\\",
        "f9 DirectCourier",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Sans"));
    assert!(text.contains("Serif"));
    assert!(text.contains("Mono"));
    assert!(text.contains("LegacySans"));
    assert!(text.contains("LegacySerif"));
    assert!(text.contains("Wing"));
    assert!(text.contains("DirectTimes"));
    assert!(text.contains("CharsetTimes"));
    assert!(text.contains("DirectSans"));
    assert!(text.contains("DirectCourier"));
    assert!(!text.contains("fonttbl"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("office-font-substitution.rtf");
    let output_path = dir.path().join("office-font-substitution.pdf");
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
            .all(|diagnostic| { !diagnostic.message.contains("bounded passive PDF base font") })
    );
    for unexpected in [
        "font 'MS Sans Serif' approximated",
        "font 'MS Serif' approximated",
        "font 'Wingdings' approximated",
        "font 'Times New Roman' approximated",
        "font 'Times New Roman CE' approximated",
        "font 'Arial' approximated",
        "font 'Courier New' approximated",
        "font 'Calibri' substituted",
        "font 'Cambria' substituted",
        "font 'Aptos Mono' substituted",
        "font 'MS Sans Serif' substituted",
        "font 'MS Serif' substituted",
        "font 'Wingdings' substituted",
        "font 'Times New Roman' substituted",
        "font 'Times New Roman CE' substituted",
        "font 'Arial' substituted",
        "font 'Courier New' substituted",
    ] {
        assert!(
            report
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains(unexpected)),
            "known passive font alias should not emit substitution diagnostic {unexpected:?}; diagnostics were {:?}",
            report.diagnostics
        );
    }
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let font_names = pdf_text_font_names(&content);

    assert!(
        font_names.iter().any(|name| name.starts_with(b"TF")),
        "Office fonts should use passive supplied resources; font selections were {font_names:?}"
    );
    for forbidden in [
        b"fonttbl".as_slice(),
        b"Calibri",
        b"Cambria",
        b"Aptos Mono",
        b"MS Sans Serif",
        b"MS Serif",
        b"Wingdings",
        b"Times New Roman",
        b"Arial",
        b"Courier New",
        b"fontemb",
        b"fontfile",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden font substitution content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn font_pitch_hints_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0",
        "\\",
        "fnil",
        "\\",
        "fprq1 Mystery Fixed;}{",
        "\\",
        "f1",
        "\\",
        "fnil",
        "\\",
        "fprq2 Mystery Variable;}}",
        "\\",
        "f0 Fixed ",
        "\\",
        "f1 Variable",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let fixed_font = parsed
        .document
        .fonts
        .iter()
        .find(|font| font.name == "Mystery Fixed")
        .expect("fixed-pitch font");
    let variable_font = parsed
        .document
        .fonts
        .iter()
        .find(|font| font.name == "Mystery Variable")
        .expect("variable-pitch font");

    assert_eq!(fixed_font.pitch, FontPitch::Fixed);
    assert_eq!(variable_font.pitch, FontPitch::Variable);
    assert!(text.contains("Fixed"));
    assert!(text.contains("Variable"));
    assert!(!text.contains("fprq"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("font-pitch-hints.rtf");
    let output_path = dir.path().join("font-pitch-hints.pdf");
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

    assert!(decoded_pdf_text(&content).contains("Fixed"));
    assert!(
        pdf_text_font_names(&content)
            .iter()
            .any(|name| name.starts_with(b"TF"))
    );
    for forbidden in [
        b"fprq".as_slice(),
        b"Mystery Fixed",
        b"Mystery Variable",
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
fn font_alternate_name_guides_passive_fallback_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0",
        "\\",
        "fnil Mystery Sans{",
        "\\",
        "*",
        "\\",
        "falt Courier New;};}}",
        "\\",
        "f0 Fallback text",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let font = parsed
        .document
        .fonts
        .iter()
        .find(|font| font.name == "Mystery Sans")
        .expect("font");

    assert_eq!(font.alternate_name.as_deref(), Some("Courier New"));
    assert!(text.contains("Fallback text"));
    for forbidden in ["fonttbl", "falt", "Courier New"] {
        assert!(
            !text.contains(forbidden),
            "font alternate metadata leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("font-alternate.rtf");
    let output_path = dir.path().join("font-alternate.pdf");
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
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(decoded_pdf_text(&content).contains("Fallback text"));
    for forbidden in [
        b"fonttbl".as_slice(),
        b"falt",
        b"Mystery Sans",
        b"Courier New",
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
fn font_face_metadata_is_stripped_before_passive_pdf_rendering() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0",
        "\\",
        "fnil",
        "\\",
        "fprq2",
        "\\",
        "fttruetype",
        "\\",
        "fbias0{",
        "\\",
        "panose 02020603050405020304 HOSTILE-PANOSE {",
        "\\",
        "object",
        "\\",
        "objdata ",
        payload_hex(),
        "}}Metadata Sans;}}",
        "\\",
        "f0 Visible font metadata body",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let font = parsed
        .document
        .fonts
        .iter()
        .find(|font| font.index == 0)
        .expect("font");

    assert_eq!(font.name, "Metadata Sans");
    assert!(text.contains("Visible font metadata body"));
    for forbidden in [
        "panose",
        "fttruetype",
        "fbias",
        "HOSTILE-PANOSE",
        "objdata",
        payload_hex(),
    ] {
        assert!(
            !text.contains(forbidden),
            "font metadata leaked to normalized text: {forbidden}"
        );
        assert!(
            !font.name.contains(forbidden),
            "font metadata polluted font name: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "font metadata should not be unsupported: {:?}",
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

    assert!(rendered_text.contains("Visible font metadata body"));
    for forbidden in [
        b"panose".as_slice(),
        b"fttruetype",
        b"fbias",
        b"HOSTILE-PANOSE",
        b"objdata",
        payload_hex().as_bytes(),
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
            "font metadata or active PDF content leaked: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn embedded_font_destinations_are_stripped_before_pdf_rendering() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0",
        "\\",
        "fswiss Arial{",
        "\\",
        "fontemb{",
        "\\",
        "fontfile HOSTILE-FONT-PAYLOAD {",
        "\\",
        "object",
        "\\",
        "objdata ",
        payload_hex(),
        "}}};}}Visible body",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible body"));
    assert!(
        parsed
            .document
            .fonts
            .iter()
            .any(|font| font.name == "Arial"),
        "expected visible font name to survive embedded-font stripping"
    );
    for forbidden in [
        "HOSTILE-FONT-PAYLOAD",
        "fontemb",
        "fontfile",
        "object",
        "objdata",
        payload_hex(),
    ] {
        assert!(
            !text.contains(forbidden),
            "embedded font payload leaked into text: {forbidden}"
        );
        assert!(
            parsed
                .document
                .fonts
                .iter()
                .all(|font| !font.name.contains(forbidden)),
            "embedded font payload leaked into font name: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("embedded-font.rtf");
    let output_path = dir.path().join("embedded-font.pdf");
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

    assert!(rendered_text.contains("Visible body"));
    for forbidden in [
        b"HOSTILE-FONT-PAYLOAD".as_slice(),
        b"fontemb",
        b"fontfile",
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
            "embedded font payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn embedded_font_destinations_obey_reject_policy() {
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
                "fonttbl{",
                "\\",
                "f0 Arial{",
                "\\",
                "fontemb{",
                "\\",
                "fontfile HOSTILE-FONT-PAYLOAD}};}}Visible",
                "\\",
                "par}"
            ]),
            &reject_options,
        ),
        Err(ParseError::ActiveContentRejected { feature, .. }) if feature == "embedded font payload"
    ));
}

#[test]
fn default_font_control_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "deff1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0",
        "\\",
        "fswiss Arial;}{",
        "\\",
        "f1",
        "\\",
        "froman Times New Roman;}}",
        "\\",
        "plain Default ",
        "\\",
        "f0 Sans ",
        "\\",
        "plain Back",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let paragraph = match &parsed.document.blocks[0] {
        open_rtf_converter::model::Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    let font_for = |text: &str| {
        paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == text)
            .map(|run| run.style.font_index)
            .unwrap_or_else(|| panic!("missing run {text}"))
    };
    let text = collect_text(&parsed.document);

    assert_eq!(font_for("Default"), 1);
    assert_eq!(font_for("Sans"), 0);
    assert_eq!(font_for("Back"), 1);
    assert!(text.contains("Default"));
    assert!(text.contains("Sans"));
    assert!(text.contains("Back"));
    assert!(!text.contains("deff"));
    assert!(!text.contains("fonttbl"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("default-font.rtf");
    let output_path = dir.path().join("default-font.pdf");
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

    assert!(content.operations.iter().any(|operation| {
        operation.operator == "Tf" && format!("{:?}", operation.operands).contains("/TF")
    }));
    for forbidden in [
        b"deff".as_slice(),
        b"fonttbl",
        b"Times New Roman",
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
fn legacy_code_page_hex_escapes_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansicpg437 cafe ",
        "\\",
        "'82 box ",
        "\\",
        "'b3",
        "\\",
        "par",
        "\\",
        "mac quote ",
        "\\",
        "'d2Hello",
        "\\",
        "'d3 bullet ",
        "\\",
        "'a5",
        "\\",
        "par",
        "\\",
        "pca nordic ",
        "\\",
        "'9b",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("cafe \u{00e9} box \u{2502}"));
    assert!(text.contains("quote \u{201c}Hello\u{201d} bullet \u{2022}"));
    assert!(text.contains("nordic \u{00f8}"));
    assert!(!text.contains("ansicpg"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("legacy-code-pages.rtf");
    let output_path = dir.path().join("legacy-code-pages.pdf");
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
        b"ansicpg".as_slice(),
        b"mac",
        b"pca",
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
fn unicode_alternate_destinations_render_passively_without_fallback_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Header {",
        "\\",
        "upr{fallback-header}{",
        "\\",
        "*",
        "\\",
        "ud{",
        "\\",
        "u937?}}} body {",
        "\\",
        "upr{fallback-body}{",
        "\\",
        "*",
        "\\",
        "ud{",
        "\\",
        "u8212-}}}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Header \u{03a9} body \u{2014}"));
    for forbidden in ["fallback-header", "fallback-body", "upr", "ud"] {
        assert!(
            !text.contains(forbidden),
            "forbidden unicode alternate content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("unicode-alternate.rtf");
    let output_path = dir.path().join("unicode-alternate.pdf");
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
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"fallback-header".as_slice(),
        b"fallback-body",
        b"upr",
        b"\\ud",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !searchable_pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn oversized_unicode_fallback_skip_is_bounded_before_pdf_rendering() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "uc999 Before ",
        "\\",
        "u65? after",
        "\\",
        "par}",
    ]);
    let options = RtfParseOptions {
        limits: RtfLimits {
            max_unicode_fallback_skip: 1,
            ..RtfLimits::default()
        },
        ..RtfParseOptions::default()
    };
    let parsed = parse_rtf_bytes_with_options(&input, &options).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before A after"));
    assert!(!text.contains("uc999"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("Unicode fallback skip clamped from 999 to 1")
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

    assert!(
        rendered_text.contains("Before A after"),
        "bounded Unicode fallback skip should preserve following visible text: {rendered_text:?}"
    );
    for forbidden in [
        b"uc999".as_slice(),
        b"u65",
        b"999",
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
            "forbidden Unicode fallback content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn unicode_surrogate_pairs_are_normalized_before_pdf_rendering() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "uc1 Face ",
        "\\",
        "u-10179?",
        "\\",
        "u-8704? done",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Face \u{1f600} done"));
    assert!(!text.contains("u-10179"));
    assert!(!text.contains("u-8704"));
    assert!(!text.contains("??"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("unicode-surrogate-pair.rtf");
    let output_path = dir.path().join("unicode-surrogate-pair.pdf");
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
        rendered_text.contains("Face ? done"),
        "current Base14 PDF fallback text was {rendered_text:?}"
    );
    for forbidden in [
        b"u-10179".as_slice(),
        b"u-8704",
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
fn wmf_exttextout_uses_selected_font_charset_and_stays_passive() {
    let wmf_hex = concat!(
        "0100090000033200000001000c0000000000",
        "050000000c026400c800",
        "0500000009020000ff00",
        "0c000000fb02f4ff0000000000000000000000ee00000000",
        "040000002d010000",
        "0c000000320a140028000100040030006000100020008c00",
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
        .expect("WMF EXTTEXTOUT vector preview image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains('\u{015a}'));
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
                && text == "\u{015a}"
                && color.red == 0
                && color.green == 0
                && color.blue == 255
        )
    }));
    for forbidden in ["wmetafile", "0a32", "fb02", "JavaScript"] {
        assert!(
            !text.contains(forbidden),
            "WMF EXTTEXTOUT internals leaked to text: {forbidden}"
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
        output.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("Latin Extended characters in passive WMF vector text")),
        "visible WMF Latin Extended vector text should produce a passive glyph diagnostic: {:?}",
        output.diagnostics
    );
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        pdf_text_font_names(&content)
            .iter()
            .any(|font| font.as_slice() == b"F1"),
        "WMF EXTTEXTOUT should use a passive built-in font resource"
    );
    assert!(
        pdf_text_bytes_for_font(&content, b"F1").contains(&0xda),
        "WMF EXTTEXTOUT should encode Central European text as passive WinAnsi bytes"
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
            "WMF EXTTEXTOUT leaked forbidden PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_extcreatefontindirectw_sets_passive_text_height_without_payload_leakage() {
    let mut font = emf_extcreatefontindirectw_record(7, -32, 0, 0, 0);
    font.extend_from_slice(b"EMF-EXTCREATEFONTINDIRECTW-FACE /JavaScript /EmbeddedFile /Launch");
    font.resize(font.len().next_multiple_of(4), 0);
    let font_len = font.len() as u32;
    write_test_le_u32(&mut font, 4, font_len);

    let records = [
        font,
        emf_select_object_record(7),
        emf_exttextoutw_record(40, 20, "Tall", 0, None, false),
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
        .expect("passive EMF font-height text vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("Tall"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(
        matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Text {
                height,
                ref text,
                ..
            } if text == "Tall" && (height - 32.0).abs() < 0.01
        ),
        "selected EMF font height should drive passive text height: {:?}; diagnostics: {:?}",
        image.vector_commands[0],
        parsed.diagnostics
    );
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "EMF-EXTCREATEFONTINDIRECTW-FACE",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF font payload/control leaked to normalized text: {forbidden}"
        );
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
    assert!(rendered_text.contains("Tall"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "Tf"),
        "selected EMF font should render through passive PDF text state"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-EXTCREATEFONTINDIRECTW-FACE",
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
            "EMF font payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn rotated_emf_extcreatefontindirectw_stays_partial_without_payload_leakage() {
    let mut font = emf_extcreatefontindirectw_record(7, -32, 900, 900, 0);
    font.extend_from_slice(b"EMF-ROTATED-EXTCREATEFONTINDIRECTW /JavaScript /EmbeddedFile /Launch");
    font.resize(font.len().next_multiple_of(4), 0);
    let font_len = font.len() as u32;
    write_test_le_u32(&mut font, 4, font_len);
    let records = [
        font,
        emf_select_object_record(7),
        emf_exttextoutw_record(40, 20, "Rotated", 0, None, false),
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
    assert!(rendered_text.contains("Rotated"));
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-ROTATED-EXTCREATEFONTINDIRECTW",
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
            "rotated EMF font payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emf_selected_font_charset_decodes_ansi_text_passively_without_payload_leakage() {
    let mut font = emf_extcreatefontindirectw_record(7, -16, 0, 0, 238);
    font.extend_from_slice(b"EMF-ANSI-CHARSET-FONT /JavaScript /EmbeddedFile /Launch");
    font.resize(font.len().next_multiple_of(4), 0);
    let font_len = font.len() as u32;
    write_test_le_u32(&mut font, 4, font_len);
    let records = [
        font,
        emf_select_object_record(7),
        emf_exttextouta_record(30, 20, b"\x8c", 0, None, false),
        emf_polytextouta_record(&[(50, 20, b"\x8c".as_slice(), 0, None)]),
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
        .expect("passive EMF ANSI charset text vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains('\u{015a}'));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 2);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Text {
            x: 30.0,
            ref text,
            ..
        } if text == "\u{015a}"
    ));
    assert!(matches!(
        image.vector_commands[1],
        StaticImageVectorCommand::Text {
            x: 50.0,
            ref text,
            ..
        } if text == "\u{015a}"
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "EMF-ANSI-CHARSET-FONT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF ANSI charset payload/control leaked to normalized text: {forbidden}"
        );
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
            .contains("Latin Extended characters in passive WMF vector text")),
        "visible EMF Latin Extended vector text should produce a passive glyph diagnostic: {:?}",
        output.diagnostics
    );
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        pdf_text_font_names(&content)
            .iter()
            .any(|font| font.as_slice() == b"F1"),
        "EMF ANSI charset text should use a passive built-in font resource"
    );
    assert!(
        pdf_text_bytes_for_font(&content, b"F1").contains(&0xda),
        "EMF ANSI charset text should encode Central European text as passive WinAnsi bytes"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-ANSI-CHARSET-FONT",
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
            "EMF ANSI charset payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn font_code_page_hex_escapes_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "ansicpg1252{",
        "\\",
        "fonttbl{",
        "\\",
        "f0",
        "\\",
        "cpg437 Terminal;}{",
        "\\",
        "f1",
        "\\",
        "cpg10000 Mac Face;}}",
        "\\",
        "f0 cafe ",
        "\\",
        "'82 ",
        "\\",
        "f1 quote ",
        "\\",
        "'d2Hello",
        "\\",
        "'d3",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("cafe \u{201a}"));
    assert!(text.contains("quote \u{00d2}Hello\u{00d3}"));
    let strict = parse_rtf_bytes_with_options(
        &input,
        &RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        },
    )
    .unwrap();
    let strict_text = collect_text(&strict.document);
    assert!(strict_text.contains("cafe \u{00e9}"));
    assert!(strict_text.contains("quote \u{201c}Hello\u{201d}"));
    for forbidden in ["cpg", "Terminal", "Mac Face"] {
        assert!(
            !text.contains(forbidden),
            "font code-page metadata leaked into text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("font-code-pages.rtf");
    let output_path = dir.path().join("font-code-pages.pdf");
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
        b"cpg".as_slice(),
        b"Terminal",
        b"Mac Face",
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
fn font_charset_hex_escapes_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "ansicpg1252{",
        "\\",
        "fonttbl{",
        "\\",
        "f0",
        "\\",
        "fcharset77 Mac Face;}{",
        "\\",
        "f1",
        "\\",
        "fcharset255 Oem Face;}}",
        "\\",
        "f0 quote ",
        "\\",
        "'d2Hello",
        "\\",
        "'d3 ",
        "\\",
        "f1 line ",
        "\\",
        "'b3",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("quote \u{201c}Hello\u{201d}"));
    assert!(text.contains("line \u{2502}"));
    for forbidden in ["fcharset", "Mac Face", "Oem Face"] {
        assert!(
            !text.contains(forbidden),
            "font charset metadata leaked into text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("font-charset-code-pages.rtf");
    let output_path = dir.path().join("font-charset-code-pages.pdf");
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
        b"fcharset".as_slice(),
        b"Mac Face",
        b"Oem Face",
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
fn central_european_font_charset_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "ansicpg1252{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Times New Roman;}{",
        "\\",
        "f37",
        "\\",
        "fcharset238 Times New Roman CE;}}",
        "\\",
        "f37 ",
        "\\",
        "'f6t ",
        "\\",
        "'e1rv",
        "\\",
        "'edzt",
        "\\",
        "'fb",
        "r",
        "\\",
        "'f5 ",
        "\\",
        "'fctvef",
        "\\",
        "'fa",
        "r",
        "\\",
        "'f3g",
        "\\",
        "'e9p",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains(
        "\u{00f6}t \u{00e1}rv\u{00ed}zt\u{0171}r\u{0151} \u{00fc}tvef\u{00fa}r\u{00f3}g\u{00e9}p"
    ));
    for forbidden in ["fcharset", "Times New Roman CE"] {
        assert!(
            !text.contains(forbidden),
            "Central European font metadata leaked into text: {forbidden}"
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
    assert!(decoded_pdf_text(&content).contains("t "));
    for forbidden in [
        b"fcharset".as_slice(),
        b"Times New Roman CE",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn greek_font_charset_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "ansicpg1252{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Times New Roman;}{",
        "\\",
        "f40",
        "\\",
        "fcharset161 Times New Roman Greek;}}",
        "\\",
        "f40 Greek ",
        "\\",
        "'c1",
        "\\",
        "'e1 ",
        "\\",
        "'d0",
        "\\",
        "'f0 ",
        "\\",
        "'d9",
        "\\",
        "'f9",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Greek \u{0391}\u{03b1} \u{03a0}\u{03c0} \u{03a9}\u{03c9}"));
    for forbidden in ["fcharset", "Times New Roman Greek"] {
        assert!(
            !text.contains(forbidden),
            "Greek font metadata leaked into text: {forbidden}"
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
    assert!(decoded_pdf_text(&content).contains("Greek "));
    for forbidden in [
        b"fcharset".as_slice(),
        b"Times New Roman Greek",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn turkish_font_charset_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "ansicpg1252{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Times New Roman;}{",
        "\\",
        "f41",
        "\\",
        "fcharset162 Times New Roman Tur;}}",
        "\\",
        "f41 Turkish ",
        "\\",
        "'d0",
        "\\",
        "'dd",
        "\\",
        "'de ",
        "\\",
        "'f0",
        "\\",
        "'fd",
        "\\",
        "'fe",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Turkish \u{011e}\u{0130}\u{015e} \u{011f}\u{0131}\u{015f}"));
    for forbidden in ["fcharset", "Times New Roman Tur"] {
        assert!(
            !text.contains(forbidden),
            "Turkish font metadata leaked into text: {forbidden}"
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
    assert!(decoded_pdf_text(&content).contains("Turkish "));
    for forbidden in [
        b"fcharset".as_slice(),
        b"Times New Roman Tur",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn baltic_font_charset_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "ansicpg1252{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Times New Roman;}{",
        "\\",
        "f42",
        "\\",
        "fcharset186 Times New Roman Baltic;}}",
        "\\",
        "f42 Baltic ",
        "\\",
        "'c0",
        "\\",
        "'e0 ",
        "\\",
        "'c8",
        "\\",
        "'e8 ",
        "\\",
        "'d8",
        "\\",
        "'f8 ",
        "\\",
        "'da",
        "\\",
        "'fa ",
        "\\",
        "'dd",
        "\\",
        "'fd",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains(
        "Baltic \u{0104}\u{0105} \u{010c}\u{010d} \u{0172}\u{0173} \u{015a}\u{015b} \u{017b}\u{017c}"
    ));
    for forbidden in ["fcharset", "Times New Roman Baltic"] {
        assert!(
            !text.contains(forbidden),
            "Baltic font metadata leaked into text: {forbidden}"
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
    assert!(decoded_pdf_text(&content).contains("Baltic "));
    for forbidden in [
        b"fcharset".as_slice(),
        b"Times New Roman Baltic",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn ansi_font_charset_hex_escapes_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "ansicpg437{",
        "\\",
        "fonttbl{",
        "\\",
        "f0",
        "\\",
        "fcharset0 Ansi Face;}}",
        "\\",
        "f0 quote ",
        "\\",
        "'93Hello",
        "\\",
        "'94 dash ",
        "\\",
        "'97",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("quote \u{201c}Hello\u{201d} dash \u{2014}"));
    for forbidden in ["fcharset", "Ansi Face"] {
        assert!(
            !text.contains(forbidden),
            "ANSI font charset metadata leaked into text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("ansi-font-charset.rtf");
    let output_path = dir.path().join("ansi-font-charset.pdf");
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
        b"fcharset".as_slice(),
        b"Ansi Face",
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
fn office_symbol_shapes_render_as_passive_polygons_without_payload_leakage() {
    let mut input = String::from("{\\rtf1 Before\\par");
    for (shape_type, payload) in [
        (17, "hidden-smiley-payload"),
        (18, "hidden-donut-payload"),
        (19, "hidden-no-symbol-payload"),
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
    assert_eq!(shapes.len(), 3);
    for (shape, expected_points) in shapes.iter().zip([24, 48, 48]) {
        assert_eq!(shape.kind, StaticShapeKind::Polygon);
        assert_eq!(shape.points.len(), expected_points);
        assert!(
            shape.points.iter().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "symbol shape points must stay inside the passive shape frame"
        );
    }
    assert_eq!(
        shapes[0].overlay_paths.len(),
        3,
        "smiley face should preserve passive eye and mouth strokes"
    );
    assert_eq!(shapes[0].overlay_paths[0].len(), 2);
    assert_eq!(shapes[0].overlay_paths[1].len(), 2);
    assert!(
        shapes[0].overlay_paths[2].len() >= 11,
        "smiley mouth should be a bounded passive polyline"
    );
    assert!(
        shapes[0].overlay_paths.iter().flatten().all(|point| {
            point.x_twips >= 0
                && point.x_twips <= shapes[0].width_twips
                && point.y_twips >= 0
                && point.y_twips <= shapes[0].height_twips
        }),
        "smiley face detail strokes must stay inside the passive shape frame"
    );
    assert_eq!(
        shapes[1].fill_rule,
        StaticImageVectorFillRule::Alternate,
        "donut should use even-odd fill so the passive PDF keeps the center open"
    );
    assert_eq!(shapes[1].point_paths.len(), 1);
    assert_eq!(shapes[1].point_paths[0].len(), 48);
    assert!(
        shapes[1].point_paths.iter().flatten().all(|point| {
            point.x_twips >= 0
                && point.x_twips <= shapes[1].width_twips
                && point.y_twips >= 0
                && point.y_twips <= shapes[1].height_twips
        }),
        "donut inner path must stay inside the passive shape frame"
    );
    assert_eq!(
        shapes[2].fill_rule,
        StaticImageVectorFillRule::Alternate,
        "no-symbol should use even-odd fill for its passive ring"
    );
    assert_eq!(shapes[2].point_paths.len(), 1);
    assert_eq!(shapes[2].point_paths[0].len(), 48);
    assert_eq!(shapes[2].overlay_paths.len(), 1);
    assert_eq!(shapes[2].overlay_paths[0].len(), 4);
    assert!(
        shapes[2]
            .point_paths
            .iter()
            .chain(shapes[2].overlay_paths.iter())
            .flatten()
            .all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shapes[2].width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shapes[2].height_twips
            }),
        "no-symbol ring and slash paths must stay inside the passive shape frame"
    );
    assert_eq!(shapes[0].fill_rule, StaticImageVectorFillRule::Winding);
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "hidden-smiley-payload",
        "hidden-donut-payload",
        "hidden-no-symbol-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "symbol shape metadata leaked to normalized text: {forbidden}"
        );
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
    let mut passive_even_odd_shape_paints = 0;
    let mut passive_path_moves = 0;
    let mut passive_overlay_strokes = 0;
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
        passive_overlay_strokes += content
            .operations
            .iter()
            .filter(|operation| operation.operator == "S")
            .count();
    }

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        passive_shape_paints >= 3,
        "symbol shapes should render passive fill/stroke paths"
    );
    assert!(
        passive_even_odd_shape_paints >= 2,
        "donut and no-symbol rings should render as passive even-odd compound paths"
    );
    assert!(
        passive_path_moves >= 6,
        "compound symbols should emit separate bounded ring and slash passive paths"
    );
    assert!(
        passive_overlay_strokes >= 4,
        "smiley face details and no-symbol slash should render as passive strokes"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-smiley-payload",
        b"hidden-donut-payload",
        b"hidden-no-symbol-payload",
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
            "symbol shape metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}
