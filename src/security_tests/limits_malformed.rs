use super::*;

#[test]
fn fuzz_corpus_seeds_exercise_bounded_parser_and_passive_pdf_paths() {
    let parser_options = RtfParseOptions::browser_safe_defaults();
    for seed in [
        include_bytes!("../../fuzz/corpus/rtf_lexer/simple.rtf").as_slice(),
        include_bytes!("../../fuzz/corpus/rtf_lexer/malformed-boundaries.rtf").as_slice(),
        include_bytes!("../../fuzz/corpus/rtf_lexer/oversized-control.rtf").as_slice(),
        include_bytes!("../../fuzz/corpus/rtf_parser/bin-boundary.rtf").as_slice(),
        include_bytes!("../../fuzz/corpus/rtf_parser/object-result.rtf").as_slice(),
        include_bytes!("../../fuzz/corpus/rtf_parser/unknown-object-data.rtf").as_slice(),
        include_bytes!("../../fuzz/corpus/rtf_parser/malformed-hex-and-bin.rtf").as_slice(),
        include_bytes!("../../fuzz/corpus/rtf_parser/deep-groups.rtf").as_slice(),
    ] {
        let _ = parse_rtf_bytes_with_options(seed, &parser_options);
    }

    let convert_options = ConvertOptions::browser_safe_defaults();
    for (name, seed) in [
        (
            "passive field object",
            include_bytes!("../../fuzz/corpus/rtf_to_pdf_passive/field-object.rtf").as_slice(),
        ),
        (
            "passive external and object fields",
            include_bytes!("../../fuzz/corpus/rtf_to_pdf_passive/external-and-object-fields.rtf")
                .as_slice(),
        ),
        (
            "passive metadata object smuggling",
            include_bytes!("../../fuzz/corpus/rtf_to_pdf_passive/metadata-object-smuggling.rtf")
                .as_slice(),
        ),
        (
            "passive list-name metadata smuggling",
            include_bytes!("../../fuzz/corpus/rtf_to_pdf_passive/listname-metadata-smuggling.rtf")
                .as_slice(),
        ),
    ] {
        let output = convert_rtf_to_pdf(seed, &convert_options)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        audit_passive_pdf_bytes(&output.pdf).unwrap_or_else(|error| panic!("{name}: {error}"));
    }
}

#[test]
fn bin_lengths_are_bounded_and_binary_does_not_become_text() {
    let parsed = parse_rtf_bytes(&rtf(&[
        "{",
        "\\",
        "rtf1 before ",
        "\\",
        "bin3 a",
        "\\",
        "} after}",
    ]))
    .unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert!(!text.contains("a\\}"));

    assert!(matches!(
        parse_rtf_bytes(&rtf(&["{", "\\", "rtf1 ", "\\", "bin-1 abc}"])),
        Err(ParseError::Lex(LexError::InvalidBinaryLength(_)))
    ));
    assert!(matches!(
        parse_rtf_bytes(&rtf(&["{", "\\", "rtf1 ", "\\", "bin9999999999999 abc}"])),
        Err(ParseError::Lex(
            LexError::NumericParameterTooLong(_) | LexError::NumericParameterOverflow(_)
        ))
    ));
    assert!(matches!(
        parse_rtf_bytes(&rtf(&["{", "\\", "rtf1 ", "\\", "bin5 abc}"])),
        Err(ParseError::Lex(LexError::ShortBinaryBlob(_)))
    ));
}

#[test]
fn oversized_controls_parameters_and_deep_groups_are_rejected() {
    let options = RtfParseOptions {
        limits: RtfLimits {
            max_group_depth: 3,
            max_control_word_len: 4,
            max_parameter_digits: 2,
            ..RtfLimits::default()
        },
        ..RtfParseOptions::default()
    };

    assert!(matches!(
        parse_rtf_bytes_with_options(&rtf(&["{", "\\", "rtf1 ", "\\", "abcde text}"]), &options),
        Err(ParseError::Lex(LexError::ControlWordTooLong(_)))
    ));
    assert!(matches!(
        parse_rtf_bytes_with_options(&rtf(&["{", "\\", "rtf1 ", "\\", "fs240 text}"]), &options),
        Err(ParseError::Lex(LexError::NumericParameterTooLong(_)))
    ));
    assert!(matches!(
        parse_rtf_bytes_with_options(b"{{{{x}}}}", &options),
        Err(ParseError::Lex(LexError::GroupDepthExceeded(_)))
    ));
}

#[test]
fn unmatched_and_unclosed_groups_are_rejected_by_tokenizer_boundary() {
    assert!(matches!(
        parse_rtf_bytes(b"}{\\rtf1 visible}"),
        Err(ParseError::Lex(LexError::UnmatchedGroupEnd(0)))
    ));
    assert!(matches!(
        parse_rtf_bytes(b"{\\rtf1 visible"),
        Err(ParseError::Lex(LexError::UnclosedGroup(_)))
    ));
}

#[test]
fn document_block_growth_is_bounded_before_layout() {
    let options = RtfParseOptions {
        limits: RtfLimits {
            max_document_blocks: 1,
            ..RtfLimits::default()
        },
        ..RtfParseOptions::default()
    };

    assert!(matches!(
        parse_rtf_bytes_with_options(
            &rtf(&["{", "\\", "rtf1 First", "\\", "par", "\\", "page Second", "\\", "par}"]),
            &options
        ),
        Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "document blocks"
    ));
}

#[test]
fn consecutive_hyphenation_limit_renders_passively_without_control_leakage() {
    let word = "AntidisestablishmentarianismAntidisestablishmentarianism";
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
        "hyphauto",
        "\\",
        "hyphconsec1",
        "\\",
        "pard ",
        word,
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
    assert_eq!(paragraph.style.max_consecutive_hyphenated_lines, Some(1));
    assert!(text.contains(word));
    assert!(!text.contains("hyphconsec"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("consecutive automatic hyphenation limit applied")
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

    assert_eq!(rendered_text, word);
    assert!(!rendered_text.contains('-'));
    for forbidden in [
        b"hyphconsec".as_slice(),
        b"hyphauto",
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
            "forbidden consecutive hyphenation content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn extreme_character_scaling_is_bounded_before_pdf_rendering() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 normal ",
        "\\",
        "charscalex999999 scaled",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("normal"));
    assert!(text.contains("scaled"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("character-scaling.rtf");
    let output_path = dir.path().join("character-scaling.pdf");
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
fn character_direction_controls_render_bounded_passive_metadata_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Normal {",
        "\\",
        "rtlch ",
        "\\",
        "u1513?",
        "\\",
        "u1500?",
        "\\",
        "u1493?",
        "\\",
        "u1501?} {",
        "\\",
        "ltrch LTR text}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Normal שלום LTR text"));
    for forbidden in ["rtlch", "ltrch"] {
        assert!(
            !text.contains(forbidden),
            "forbidden character direction control leaked to text: {forbidden}"
        );
    }
    let paragraph = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            open_rtf_converter::model::Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .expect("paragraph");
    let rtl_run = paragraph
        .runs
        .iter()
        .find(|run| run.text.contains("שלום"))
        .expect("RTL run");
    let ltr_run = paragraph
        .runs
        .iter()
        .find(|run| run.text.contains("LTR text"))
        .expect("LTR run");
    assert!(rtl_run.style.right_to_left);
    assert!(!ltr_run.style.right_to_left);
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "right-to-left character direction rendered by bounded passive visual text order",
        )
    }));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "character direction controls should not be unsupported: {:?}",
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
    assert!(PdfDocument::load_mem(&output.pdf).is_ok());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "right-to-left character direction rendered by bounded passive visual text order",
        )
    }));

    for forbidden in [
        b"rtlch".as_slice(),
        b"ltrch",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden character direction content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn derived_drop_cap_dimensions_remain_bounded_passive_geometry() {
    let input = include_bytes!("../../fixtures/framed-drop-cap-derived-dimensions-passive.rtf");
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
    assert_eq!(paragraphs[0].style.frame_width_twips, Some(720));
    assert_eq!(paragraphs[0].style.frame_height_twips, None);
    assert_eq!(paragraphs[2].style.frame_width_twips, None);
    assert_eq!(paragraphs[2].style.frame_height_twips, Some(720));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("derived-drop-cap.rtf");
    let output_path = dir.path().join("derived-drop-cap.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(&input_path, &output_path, &ConvertOptions::default()).unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    for visible in [
        "W",
        "Width-only frame",
        "H",
        "Height-only frame",
        "Plain paragraph",
    ] {
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
fn ignored_destinations_consume_bounded_skip_budget() {
    let options = RtfParseOptions {
        limits: RtfLimits {
            max_destination_bytes: 4,
            ..RtfLimits::default()
        },
        ..RtfParseOptions::default()
    };

    assert!(matches!(
        parse_rtf_bytes_with_options(br"{\rtf1{\*\unknown abcde} visible\par}", &options),
        Err(ParseError::DestinationTooLarge(_))
    ));
    assert!(matches!(
        parse_rtf_bytes_with_options(br"{\rtf1{\*\unknown\bin5 abcde} visible\par}", &options),
        Err(ParseError::DestinationTooLarge(_))
    ));
}

#[test]
fn hostile_seed_corpus_returns_typed_results_without_panics() {
    let seeds = vec![
        rtf(&["{", "\\", "rtf1 hello}"]),
        rtf(&["{", "\\", "rtf1{", "\\", "b bold}}"]),
        rtf(&["{", "\\", "rtf1 ", "\\", "u8217?}"]),
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "fonttbl{",
            "\\",
            "f0 Arial;}}Hello}",
        ]),
        rtf(&[
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
            "blue0;}Hello}",
        ]),
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "*",
            "\\",
            "unknown ignored}visible}",
        ]),
        object_with_payload(false),
        object_with_payload(true),
        field_with_link_result(),
        rtf(&[
            "{",
            "\\",
            "rtf1 visible{",
            "\\",
            "annotation hidden {",
            "\\",
            "object",
            "\\",
            "objdata ",
            payload_hex(),
            "}}}",
        ]),
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "bkmkstart SecretBookmark}visible{",
            "\\",
            "deleted Deleted text}}",
        ]),
        rtf(&[
            "{",
            "\\",
            "rtf1 visible {",
            "\\",
            "v hidden {",
            "\\",
            "object",
            "\\",
            "objdata ",
            payload_hex(),
            "}}}",
        ]),
        rtf(&["{", "\\", "rtf1", "\\", "bin3 abc}"]),
        rtf(&["{", "\\", "rtf1", "\\", "'41", "\\", "'42", "\\", "'43}"]),
        b"{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{".to_vec(),
        rtf(&["{", "\\", "rtf1", "\\", "bin999999999999999999999 abc}"]),
        rtf(&["{", "\\", "rtf1", "\\", "bin-1 abc}"]),
        rtf(&[
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
            "}}}",
        ]),
        rtf(&[
            "{",
            "\\",
            "rtf1",
            "\\",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa text}",
        ]),
        rtf(&["{", "\\", "rtf1", "\\", "'GZ}"]),
    ];

    for seed in seeds {
        let _ = parse_rtf_bytes(&seed);
    }
}
