    #[test]
    fn resultless_fields_become_passive_placeholders() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\field{\*\fldinst INCLUDEPICTURE "https://example.com/a.png"}} After\par}"#,
        )
        .unwrap();
        let text = output
            .document
            .blocks
            .iter()
            .map(|block| match block {
                Block::Paragraph(paragraph) => paragraph
                    .runs
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<String>(),
                Block::Placeholder(text) => text.clone(),
                _ => String::new(),
            })
            .collect::<String>();

        assert!(text.contains("Before"));
        assert!(text.contains("[Field removed: no passive result]"));
        assert!(text.contains("After"));
        assert!(!text.contains("INCLUDEPICTURE"));
        assert!(!text.contains("https://example.com"));
    }

    #[test]
    fn resultless_noteref_fields_render_bookmarked_note_reference() {
        let output = parse_rtf(
            r#"{\rtf1 Note {\*\bkmkstart NoteRef}\chftn{\*\bkmkend NoteRef}{\footnote Footnote text\par} again {\field{\*\fldinst NOTEREF NoteRef \\* ROMAN}} missing {\field{\*\fldinst NOTEREF Missing}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains(&format!(
                "Note {} again I missing [Field removed: no passive result]",
                footnote_reference_marker(FootnoteId::new(0))
            )),
            "text was {text:?}"
        );
        assert_eq!(output.document.footnotes[0].runs[0].text, "Footnote text");
        for forbidden in [
            "NOTEREF",
            "NoteRef",
            "Missing}",
            "bkmkstart",
            "bkmkend",
            "fldinst",
            PENDING_NOTE_REFERENCE_MARKER,
        ] {
            assert!(
                !text.contains(forbidden),
                "NOTEREF field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_set_fields_feed_passive_ref_values() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\field{\*\fldinst SET Client "Contoso value"}} copy {\field{\*\fldinst REF Client \\* Upper}} missing {\field{\*\fldinst REF Missing}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Before  copy CONTOSO VALUE missing [Field removed: no passive result]"),
            "text was {text:?}"
        );
        for forbidden in ["SET", "REF Client", "fldinst"] {
            assert!(
                !text.contains(forbidden),
                "SET/REF field leaked unsafe text: {forbidden}"
            );
        }
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("rendering passive field SET without executing field instruction")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("rendering passive field REF without executing field instruction")
        }));
    }

    #[test]
    fn resultless_ref_fields_resolve_unicode_bookmark_names() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\*\bkmkstart Caf\u233?Bookmark}Marked text{\*\bkmkend Caf\u233?Bookmark} ref {\field{\*\fldinst REF "Caf\u233?Bookmark"}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Before Marked text ref Marked text"),
            "text was {text:?}"
        );
        for forbidden in [
            "Caf\u{00e9}Bookmark",
            "bkmkstart",
            "bkmkend",
            "fldinst",
            "REF",
            "?",
        ] {
            assert!(
                !text.contains(forbidden),
                "Unicode bookmark REF leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn structural_metadata_bookmarks_do_not_feed_ref_fields() {
        let output = parse_rtf(
            r#"{\rtf1{\fonttbl{\f0 Arial;}{\*\bkmkstart HiddenMeta}Hidden font payload{\*\bkmkend HiddenMeta}}\f0 Body {\*\bkmkstart VisibleMark}Marked text{\*\bkmkend VisibleMark} visible {\field{\*\fldinst REF VisibleMark}} hidden {\field{\*\fldinst REF HiddenMeta}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains(
                "Body Marked text visible Marked text hidden [Field removed: no passive result]"
            ),
            "text was {text:?}"
        );
        for forbidden in [
            "Hidden font payload",
            "HiddenMeta",
            "bkmkstart",
            "bkmkend",
            "fldinst",
            "REF VisibleMark",
        ] {
            assert!(
                !text.contains(forbidden),
                "structural metadata bookmark leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_set_fields_strip_complex_values_without_side_effects() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\field{\*\fldinst SET Unsafe "Hidden value" HYPERLINK "https://example.com/payload"}} after {\field{\*\fldinst REF Unsafe}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before  after [Field removed: no passive result]"));
        for forbidden in [
            "Hidden value",
            "HYPERLINK",
            "https://example.com",
            "SET",
            "REF Unsafe",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "complex SET field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_set_fields_obey_value_count_bounds() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_bookmarks: 0,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        let result = parse_rtf_bytes_with_options(
            br#"{\rtf1{\field{\*\fldinst SET Client "Contoso"}}\par}"#,
            &options,
        );
        assert!(
            matches!(
                result,
                Err(ParseError::ResourceLimitExceeded { ref resource, .. })
                    if resource == "field bookmark values"
            ),
            "unexpected result: {result:?}"
        );
    }

    #[test]
    fn bookmark_limit_bounds_captured_and_field_reference_targets() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_bookmarks: 1,
                max_styles: 0,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        let result = parse_rtf_bytes_with_options(
            br"{\rtf1{\*\bkmkstart One}Text{\*\bkmkend One}{\field{\*\fldinst PAGEREF Two}}\par}",
            &options,
        );

        assert!(
            matches!(
                result,
                Err(ParseError::ResourceLimitExceeded { ref resource, .. })
                    if resource == "bookmarks"
            ),
            "unexpected result: {result:?}"
        );
    }

    #[test]
    fn field_bookmark_values_share_bookmark_limit_with_page_refs() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_bookmarks: 1,
                max_styles: 0,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        let result = parse_rtf_bytes_with_options(
            br#"{\rtf1{\field{\*\fldinst SET One "Alpha"}}{\field{\*\fldinst PAGEREF Two}}\par}"#,
            &options,
        );

        assert!(
            matches!(
                result,
                Err(ParseError::ResourceLimitExceeded { ref resource, .. })
                    if resource == "bookmarks"
            ),
            "unexpected result: {result:?}"
        );
    }

    #[test]
    fn field_counter_limit_is_independent_from_styles() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_field_counters: 1,
                max_styles: 0,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        let parsed = parse_rtf_bytes_with_options(
            br"{\rtf1{\field{\*\fldinst SEQ Figure}}{\field{\*\fldinst SEQ Figure}}\par}",
            &options,
        )
        .unwrap();
        let text = document_text(&parsed.document);

        assert!(
            text.contains("12"),
            "field counters should render despite max_styles=0: {text:?}"
        );
    }

    #[test]
    fn field_counter_limit_bounds_combined_sequence_and_listnum_state() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_field_counters: 1,
                max_styles: 0,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        let result = parse_rtf_bytes_with_options(
            br"{\rtf1{\field{\*\fldinst SEQ Figure}}{\field{\*\fldinst LISTNUM LegalDefault}}\par}",
            &options,
        );

        assert!(
            matches!(
                result,
                Err(ParseError::ResourceLimitExceeded { ref resource, .. })
                    if resource == "field list numbers"
            ),
            "unexpected result: {result:?}"
        );
    }

    #[test]
    fn word_compatible_legacy_number_fields_apply_bounded_builtin_templates() {
        let output = parse_rtf(
            r#"{\rtf1 Legal {\field{\*\fldinst AUTONUMLGL \\e}}. sep {\field{\*\fldinst AUTONUMLGL \\s "-"}}. level six {\field{\*\fldinst LISTNUM NumberDefault \\l 6}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);
        let visible_text = text.replace(PASSIVE_ADVANCE_MARKER, "");

        assert!(
            visible_text.contains("Legal 1. sep 2-. level six (i)"),
            "text was {text:?}"
        );
        assert_eq!(text.matches(PASSIVE_ADVANCE_MARKER).count(), 3);
        for forbidden in [
            "AUTONUMLGL",
            "LISTNUM",
            "NumberDefault",
            "\\e",
            "\\s",
            "\\l",
        ] {
            assert!(
                !text.contains(forbidden),
                "field syntax leaked: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_styleref_fields_render_latest_safe_styled_paragraph_text() {
        let output = parse_rtf(
            r#"{\rtf1{\stylesheet{\s1 Heading One;}{\s2 Other Style;}}\s1 Visible heading\par\s1 Latest heading\par\pard Body {\field{\*\fldinst STYLEREF "Heading One" \\* Upper}} again {\field{\*\fldinst STYLEREF 1}} missing {\field{\*\fldinst STYLEREF Missing}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Visible headingLatest headingBody LATEST HEADING again Latest heading missing [Field removed: no passive result]"),
            "text was {text:?}"
        );
        for forbidden in ["STYLEREF", "Heading One", "Other Style", "fldinst"] {
            assert!(
                !text.contains(forbidden),
                "STYLEREF leaked style or instruction text: {forbidden}"
            );
        }
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("rendering passive field STYLEREF without executing field instruction")
        }));
    }

    #[test]
    fn resultless_styleref_fields_resolve_unicode_style_names() {
        let output = parse_rtf(
            r#"{\rtf1{\stylesheet{\s1 Caf\u233? Style;}}\s1 Styled heading\par\pard Ref {\field{\*\fldinst STYLEREF "Caf\u233? Style"}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Styled headingRef Styled heading"),
            "text was {text:?}"
        );
        for forbidden in [
            "Caf\u{00e9} Style",
            "STYLEREF",
            "fldinst",
            "stylesheet",
            "?",
        ] {
            assert!(
                !text.contains(forbidden),
                "Unicode STYLEREF leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_styleref_fields_resolve_hex_style_names() {
        let output = parse_rtf(
            r#"{\rtf1{\stylesheet{\s1 Cod\'65 Style;}}\s1 Styled heading\par\pard Ref {\field{\*\fldinst STYLEREF "Cod\'65 Style"}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Styled headingRef Styled heading"),
            "hex-escaped STYLEREF style names should resolve to prior styled text: {text:?}"
        );
        for forbidden in ["Code Style", "STYLEREF", "fldinst", "stylesheet", "\\'65"] {
            assert!(
                !text.contains(forbidden),
                "hex STYLEREF leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn header_field_stored_result_stays_in_safe_repeating_metadata() {
        let output = parse_rtf(
            r#"{\rtf1{\header Prefix {\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt Stored link}}\par}Body\par}"#,
        )
        .unwrap();
        let header_text = output.document.header[0]
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();
        let body_text = document_text(&output.document);

        assert_eq!(header_text, "Prefix Stored link");
        assert_eq!(body_text, "Body");
        for forbidden in ["HYPERLINK", "https://example.com", "fldinst", "fldrslt"] {
            assert!(
                !header_text.contains(forbidden),
                "field instruction leaked to header result text: {forbidden}"
            );
            assert!(
                !body_text.contains(forbidden),
                "field instruction leaked to body text: {forbidden}"
            );
        }
    }

    #[test]
    fn header_resultless_external_field_placeholder_stays_in_safe_repeating_metadata() {
        let output = parse_rtf(
            r#"{\rtf1{\header Prefix {\field{\*\fldinst INCLUDEPICTURE "https://example.com/a.png"}}\par}Body\par}"#,
        )
        .unwrap();
        let header_text = output.document.header[0]
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();
        let body_text = document_text(&output.document);

        assert_eq!(header_text, "Prefix [Field removed: no passive result]");
        assert_eq!(body_text, "Body");
        assert!(!body_text.contains("[Field removed"));
        for forbidden in ["INCLUDEPICTURE", "https://example.com", "fldinst"] {
            assert!(
                !header_text.contains(forbidden),
                "field instruction leaked to header placeholder text: {forbidden}"
            );
            assert!(
                !body_text.contains(forbidden),
                "field instruction leaked to body text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_external_fields_are_classified_without_fetching() {
        let input = r#"{\rtf1 Before
{\field{\*\fldinst INCLUDEPICTURE "https://example.com/a.png"}}
{\field{\*\fldinst INCLUDETEXT "https://example.com/doc.rtf"}}
{\field{\*\fldinst LINK Word.Document.8 "https://example.com/doc.doc"}}
{\field{\*\fldinst DDEAUTO Excel "Sheet1" "R1C1"}}
{\field{\*\fldinst DATABASE \d "https://example.com/db" \s "SELECT * FROM Hidden"}}
After\par}"#;
        let output = parse_rtf(input).unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before"));
        assert!(text.contains("After"));
        assert_eq!(
            text.matches("[Field removed: no passive result]").count(),
            5
        );
        assert_eq!(output.document.blocks.len(), 1);
        assert!(matches!(output.document.blocks[0], Block::Paragraph(_)));
        for forbidden in [
            "INCLUDEPICTURE",
            "INCLUDETEXT",
            "LINK",
            "DDEAUTO",
            "DATABASE",
            "example.com",
            "SELECT *",
        ] {
            assert!(
                !text.contains(forbidden),
                "external field leaked to text: {forbidden}"
            );
        }
        for name in [
            "INCLUDEPICTURE",
            "INCLUDETEXT",
            "LINK",
            "DDEAUTO",
            "DATABASE",
        ] {
            assert!(
                output.diagnostics.iter().any(|diagnostic| {
                    diagnostic.message.contains(&format!(
                        "external field {name} rendered as passive placeholder without fetching external resource"
                    ))
                }),
                "missing diagnostic for {name}"
            );
        }
    }

    #[test]
    fn resultless_environment_fields_are_placeholdered_without_host_state() {
        let input = r#"{\rtf1 Before
{\field{\*\fldinst FILENAME \p}}
{\field{\*\fldinst FILESIZE}}
{\field{\*\fldinst TEMPLATE}}
{\field{\*\fldinst USERNAME}}
{\field{\*\fldinst USERINITIALS}}
{\field{\*\fldinst USERADDRESS}}
After\par}"#;
        let output = parse_rtf(input).unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before"));
        assert!(text.contains("After"));
        assert_eq!(
            text.matches("[Field removed: no passive result]").count(),
            5
        );
        for forbidden in [
            "FILENAME",
            "FILESIZE",
            "TEMPLATE",
            "USERNAME",
            "USERINITIALS",
            "USERADDRESS",
            "Users\\",
            "open-rtf-converter",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "environment field leaked host or instruction text: {forbidden}"
            );
        }
        for name in [
            "FILENAME",
            "TEMPLATE",
            "USERNAME",
            "USERINITIALS",
            "USERADDRESS",
        ] {
            assert!(
                output.diagnostics.iter().any(|diagnostic| {
                    diagnostic.message.contains(&format!(
                        "environment field {name} rendered as passive placeholder without exposing converter host state"
                    ))
                }),
                "missing diagnostic for {name}"
            );
        }
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("resultless FILESIZE suppressed for Word-compatible passive rendering")
        }));
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("environment field FILENAME removed")
        }));

        let strip_options = RtfParseOptions {
            active_content_policy: ActiveContentPolicy::Strip,
            ..RtfParseOptions::default()
        };
        let stripped = parse_rtf_bytes_with_options(input.as_bytes(), &strip_options).unwrap();
        let stripped_text = document_text(&stripped.document);
        assert!(stripped_text.contains("Before"));
        assert!(stripped_text.contains("After"));
        assert!(!stripped_text.contains("[Field removed"));
        assert!(stripped.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("environment field FILENAME stripped")
        }));
    }

    #[test]
    fn resultless_layout_advance_fields_strip_without_visible_placeholder() {
        let input = r#"{\rtf1 Before {\field{\*\fldinst ADVANCE \\r 240 \\d 120}} after\par}"#;
        let output = parse_rtf(input).unwrap();
        let text = document_text(&output.document);
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let advance = paragraph
            .runs
            .iter()
            .find(|run| run.text == PASSIVE_ADVANCE_MARKER)
            .expect("passive advance marker");

        assert!(text.contains("Before"), "text was {text:?}");
        assert!(text.contains("after"), "text was {text:?}");
        assert_eq!(advance.style.character_spacing_twips, 240);
        assert_eq!(advance.style.baseline_shift_half_points, -12);
        assert!(!text.contains("[Field removed"));
        for forbidden in ["ADVANCE", "\\r", "\\d", "fldinst"] {
            assert!(
                !text.contains(forbidden),
                "ADVANCE field leaked instruction text: {forbidden}"
            );
        }
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "layout field ADVANCE interpreted as bounded passive horizontal cursor advance",
            )
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "layout field ADVANCE interpreted as bounded passive vertical cursor advance",
            )
        }));
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("ADVANCE stripped without applying cursor positioning")
        }));
    }

    #[test]
    fn resultless_layout_advance_field_offsets_are_bounded() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_character_spacing_twips: 120,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1 Before {\field{\*\fldinst ADVANCE \\r 9999 \\l 20 \\u 9999 \\d 20}} after\par}",
            &options,
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let advance = paragraph
            .runs
            .iter()
            .find(|run| run.text == PASSIVE_ADVANCE_MARKER)
            .expect("passive advance marker");

        assert_eq!(advance.style.character_spacing_twips, 120);
        assert_eq!(advance.style.baseline_shift_half_points, 12);
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("ADVANCE stripped without applying cursor positioning")
        }));
    }

    #[test]
    fn resultless_dynamic_clock_fields_do_not_read_converter_clock() {
        let input = r#"{\rtf1 Before {\field{\*\fldinst DATE \\@ "yyyy-MM-dd host-sentinel"}} time {\field{\*\fldinst TIME \\@ "HH:mm host-sentinel"}} After\par}"#;
        let output = parse_rtf(input).unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before  time  After"), "text was {text:?}");
        assert!(!text.contains("[Field removed: no passive result]"));
        for forbidden in [
            "DATE",
            "TIME",
            "host-sentinel",
            "yyyy-MM-dd",
            "HH:mm",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "dynamic clock field leaked instruction text: {forbidden}"
            );
        }
        for name in ["DATE", "TIME"] {
            assert!(output.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(&format!(
                    "resultless field {name} omitted for Word-compatible passive rendering without dynamic evaluation"
                ))
            }));
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| { !diagnostic.message.contains("dynamic field DATE removed") })
        );

        let strip_options = RtfParseOptions {
            active_content_policy: ActiveContentPolicy::Strip,
            ..RtfParseOptions::default()
        };
        let stripped = parse_rtf_bytes_with_options(input.as_bytes(), &strip_options).unwrap();
        let stripped_text = document_text(&stripped.document);
        assert!(
            stripped_text.contains("Before  time  After"),
            "stripped text was {stripped_text:?}"
        );
        assert!(!stripped_text.contains("[Field removed"));
        assert!(
            stripped
                .diagnostics
                .iter()
                .any(|diagnostic| {
                    diagnostic.message.contains(
                        "resultless field DATE omitted for Word-compatible passive rendering without dynamic evaluation",
                    )
                })
        );
    }

    #[test]
    fn resultless_prompt_fields_do_not_request_input_or_leak_prompt_text() {
        let input = r#"{\rtf1 Before {\field{\*\fldinst ASK Client "Hidden prompt" \d "Hidden default"}} ask-ref {\field{\*\fldinst REF Client}} fill {\field{\*\fldinst FILLIN "Secret prompt" \d "Secret default"}} After\par}"#;
        let output = parse_rtf(input).unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains(
                "Before  ask-ref [Field removed: no passive result] fill Secret default After"
            ),
            "text was {text:?}"
        );
        for forbidden in [
            "ASK",
            "FILLIN",
            "Client",
            "Hidden prompt",
            "Hidden default",
            "Secret prompt",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "prompt field leaked prompt/input text: {forbidden}"
            );
        }
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("non-visible field ASK stripped")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "prompt field FILLIN rendered from passive default without requesting user input",
            )
        }));
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("prompt field FILLIN removed without requesting user input")
        }));

        let strip_options = RtfParseOptions {
            active_content_policy: ActiveContentPolicy::Strip,
            ..RtfParseOptions::default()
        };
        let stripped = parse_rtf_bytes_with_options(input.as_bytes(), &strip_options).unwrap();
        let stripped_text = document_text(&stripped.document);
        assert!(stripped_text.contains("Before  ask-ref  fill  After"));
        assert!(!stripped_text.contains("[Field removed"));
        assert!(stripped.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("prompt field FILLIN stripped without requesting user input")
        }));
    }

    #[test]
    fn resultless_mail_merge_flow_fields_do_not_execute_merge_data_source() {
        let input = r#"{\rtf1 Before {\field{\*\fldinst NEXT}} nextif {\field{\*\fldinst NEXTIF "City" = "Paris"}} skip {\field{\*\fldinst SKIPIF "Status" = "Hidden"}} record {\field{\*\fldinst MERGEREC}} seq {\field{\*\fldinst MERGESEQ}} name {\field{\*\fldinst MERGEFIELD Client}}\par}"#;
        let output = parse_rtf(input).unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains(
                "Before  nextif  skip  record [Field removed: no passive result] seq [Field removed: no passive result] name \u{00ab}Client\u{00bb}"
            ),
            "text was {text:?}"
        );
        for forbidden in [
            "NEXT",
            "NEXTIF",
            "SKIPIF",
            "MERGEREC",
            "MERGESEQ",
            "MERGEFIELD",
            "City",
            "Paris",
            "Status",
            "Hidden",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "mail-merge field leaked instruction or data-source text: {forbidden}"
            );
        }
        for name in ["NEXT", "NEXTIF", "SKIPIF"] {
            assert!(
                output.diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .message
                        .contains(&format!("non-visible field {name} stripped"))
                }),
                "missing diagnostic for {name}"
            );
        }
        for name in ["MERGEREC", "MERGESEQ"] {
            assert!(
                output.diagnostics.iter().any(|diagnostic| {
                    diagnostic.message.contains(&format!(
                        "mail-merge field {name} rendered as passive placeholder without executing merge data source"
                    ))
                }),
                "missing diagnostic for {name}"
            );
        }
    }

    #[test]
    fn resultless_mail_merge_address_fields_do_not_execute_data_source() {
        let input = r#"{\rtf1 Before address {\field{\*\fldinst ADDRESSBLOCK \f "<<_COMPANY_>>\r<<_ADDRESS1_>>" \l 1033}} greeting {\field{\*\fldinst GREETINGLINE \f "<<_TITLE0_>> <<_LAST0_>>" \e "Dear Sir or Madam,"}} unsafe {\field{\*\fldinst GREETINGLINE \e "/JavaScript"}} After\par}"#;
        let output = parse_rtf(input).unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains(
                "Before address [Field removed: no passive result] greeting Dear Sir or Madam, unsafe [Field removed: no passive result] After"
            ),
            "text was {text:?}"
        );
        for forbidden in [
            "ADDRESSBLOCK",
            "GREETINGLINE",
            "_COMPANY_",
            "_ADDRESS1_",
            "_TITLE0_",
            "_LAST0_",
            "/JavaScript",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "mail-merge address field leaked data-source text: {forbidden}"
            );
        }
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "mail-merge field GREETINGLINE rendered from passive fallback without executing merge data source",
            )
        }));
        for name in ["ADDRESSBLOCK"] {
            assert!(
                output.diagnostics.iter().any(|diagnostic| {
                    diagnostic.message.contains(&format!(
                        "mail-merge field {name} rendered as passive placeholder without executing merge data source"
                    ))
                }),
                "missing diagnostic for {name}"
            );
        }
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("mail-merge field ADDRESSBLOCK removed")
        }));

        let strip_options = RtfParseOptions {
            active_content_policy: ActiveContentPolicy::Strip,
            ..RtfParseOptions::default()
        };
        let stripped = parse_rtf_bytes_with_options(input.as_bytes(), &strip_options).unwrap();
        let stripped_text = document_text(&stripped.document);
        assert!(
            stripped_text.contains("Before address  greeting  unsafe  After"),
            "stripped text was {stripped_text:?}"
        );
        assert!(!stripped_text.contains("[Field removed"));
        assert!(stripped.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("mail-merge field ADDRESSBLOCK stripped")
        }));
    }

    #[test]
    fn resultless_generated_reference_fields_placeholder_without_building_generated_content() {
        let input = r#"{\rtf1 Before {\field{\*\fldinst TOC \o "1-3" \h \z}} index {\field{\*\fldinst INDEX \c "2"}} cites {\field{\*\fldinst CITATION HiddenSource \l 1033}} bib {\field{\*\fldinst BIBLIOGRAPHY}} auth {\field{\*\fldinst TOA \c 1}} After\par}"#;
        let output = parse_rtf(input).unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before"));
        assert!(text.contains("index"));
        assert!(text.contains("cites"));
        assert!(text.contains("bib"));
        assert!(text.contains("auth"));
        assert!(text.contains("After"));
        assert_eq!(
            text.matches("[Field removed: no passive result]").count(),
            5
        );
        assert_eq!(output.document.blocks.len(), 1);
        assert!(matches!(output.document.blocks[0], Block::Paragraph(_)));
        for forbidden in [
            "TOC",
            "INDEX",
            "CITATION",
            "BIBLIOGRAPHY",
            "TOA",
            "HiddenSource",
            "fldinst",
            "\\o",
            "\\h",
            "\\z",
        ] {
            assert!(
                !text.contains(forbidden),
                "generated field leaked instruction or source text: {forbidden}"
            );
        }
        for name in ["TOC", "INDEX", "CITATION", "BIBLIOGRAPHY", "TOA"] {
            assert!(
                output.diagnostics.iter().any(|diagnostic| {
                    diagnostic.message.contains(&format!(
                        "generated field {name} rendered as passive placeholder without synthesizing generated field output"
                    ))
                }),
                "missing diagnostic for {name}"
            );
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| { !diagnostic.message.contains("generated field TOC removed") })
        );

        let strip_options = RtfParseOptions {
            active_content_policy: ActiveContentPolicy::Strip,
            ..RtfParseOptions::default()
        };
        let stripped = parse_rtf_bytes_with_options(input.as_bytes(), &strip_options).unwrap();
        let stripped_text = document_text(&stripped.document);
        assert!(stripped_text.contains("Before"));
        assert!(stripped_text.contains("After"));
        assert!(!stripped_text.contains("[Field removed"));
        assert!(
            stripped
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("generated field TOC stripped") })
        );
    }

    #[test]
    fn resultless_template_barcode_equation_and_embed_fields_do_not_execute_or_leak() {
        let input = r#"{\rtf1 Before {\field{\*\fldinst AUTOTEXT HiddenBlock}} list {\field{\*\fldinst AUTOTEXTLIST "Hidden menu" \s HiddenStyle}} barcode {\field{\*\fldinst BARCODE "Hidden address" QR \h 720}} display {\field{\*\fldinst DISPLAYBARCODE "Hidden code" QR}} mergebarcode {\field{\*\fldinst MERGEBARCODE "Hidden merge value" QR \h 720}} eq {\field{\*\fldinst EQ \f(1,2)}} embed {\field{\*\fldinst EMBED Word.Document.8}} go {\field{\*\fldinst GOTOBUTTON HiddenBookmark "Visible jump"}} After\par}"#;
        let output = parse_rtf(input).unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before"));
        assert!(text.contains("list"));
        assert!(text.contains("barcode"));
        assert!(text.contains("display"));
        assert!(text.contains("mergebarcode"));
        assert!(text.contains("eq (1,2)"));
        assert!(text.contains("embed"));
        assert!(text.contains("go \"Visible jump\""));
        assert!(text.contains("After"));
        assert_eq!(
            text.matches("[Field removed: no passive result]").count(),
            6
        );
        for forbidden in [
            "AUTOTEXT",
            "AUTOTEXTLIST",
            "BARCODE",
            "DISPLAYBARCODE",
            "MERGEBARCODE",
            "EMBED",
            "EQ",
            "GOTOBUTTON",
            "HiddenBlock",
            "Hidden menu",
            "HiddenStyle",
            "Hidden address",
            "Hidden code",
            "Hidden merge value",
            "HiddenBookmark",
            "Word.Document.8",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "template/generated/embedded field leaked instruction text: {forbidden}"
            );
        }
        for name in ["AUTOTEXT", "AUTOTEXTLIST"] {
            assert!(
                output.diagnostics.iter().any(|diagnostic| {
                    diagnostic.message.contains(&format!(
                        "building-block field {name} rendered as passive placeholder without reading template storage"
                    ))
                }),
                "missing diagnostic for {name}"
            );
        }
        for name in ["BARCODE", "DISPLAYBARCODE", "MERGEBARCODE"] {
            assert!(
                output.diagnostics.iter().any(|diagnostic| {
                    diagnostic.message.contains(&format!(
                        "generated field {name} rendered as passive placeholder without synthesizing generated field output"
                    ))
                }),
                "missing diagnostic for {name}"
            );
        }
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("rendering passive field EQ without executing field instruction")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "embedded object field EMBED rendered as passive placeholder without activating embedded object",
            )
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("rendering passive field GOTOBUTTON without executing field instruction")
        }));

        let strip_options = RtfParseOptions {
            active_content_policy: ActiveContentPolicy::Strip,
            ..RtfParseOptions::default()
        };
        let stripped = parse_rtf_bytes_with_options(input.as_bytes(), &strip_options).unwrap();
        let stripped_text = document_text(&stripped.document);
        assert!(stripped_text.contains("Before"));
        assert!(stripped_text.contains("go \"Visible jump\""));
        assert!(stripped_text.contains("After"));
        assert!(!stripped_text.contains("[Field removed"));
        assert!(stripped.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("building-block field AUTOTEXT stripped")
        }));
        assert!(stripped.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("generated field BARCODE stripped")
        }));
        assert!(stripped.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("embedded object field EMBED stripped")
        }));
    }

    #[test]
    fn resultless_eq_fields_reject_active_pdf_marker_components() {
        let output = parse_rtf(
            r#"{\rtf1 frac {\field{\*\fldinst EQ \f(/JavaScript,2)}} root {\field{\*\fldinst EQ \r(3,/EmbeddedFile)}} ok {\field{\*\fldinst EQ \f(1,2)}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains(
                "frac [Field removed: no passive result] root [Field removed: no passive result] ok (1,2)"
            ),
            "text was {text:?}"
        );
        for forbidden in [
            "EQ",
            "\\f",
            "\\r",
            "/JavaScript",
            "/EmbeddedFile",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "active marker EQ component leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn shape_fields_use_stored_result_or_placeholder_without_synthesizing_canvas() {
        let input = r#"{\rtf1 Stored {\field{\*\fldinst SHAPE \\* MERGEFORMAT}{\fldrslt Visible drawing fallback}} resultless {\field{\*\fldinst SHAPE \\* MERGEFORMAT}} After\par}"#;
        let output = parse_rtf(input).unwrap();
        let text = document_text(&output.document);
        let visible_text = visible_document_text(&output.document);

        assert!(
            visible_text.contains("Stored Visible drawing fallback resultless [Field removed: no passive result] After"),
            "text was {text:?}"
        );
        for forbidden in ["SHAPE", "MERGEFORMAT", "fldinst", "fldrslt"] {
            assert!(
                !text.contains(forbidden),
                "shape field leaked instruction text: {forbidden}"
            );
        }
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "drawing field SHAPE rendered as passive placeholder without synthesizing drawing canvas output",
            )
        }));

        let strip_options = RtfParseOptions {
            active_content_policy: ActiveContentPolicy::Strip,
            ..RtfParseOptions::default()
        };
        let stripped = parse_rtf_bytes_with_options(input.as_bytes(), &strip_options).unwrap();
        let stripped_text = document_text(&stripped.document);
        let stripped_visible_text = visible_document_text(&stripped.document);
        assert!(
            stripped_visible_text.contains("Stored Visible drawing fallback resultless  After"),
            "stripped text was {stripped_text:?}"
        );
        assert!(!stripped_text.contains("[Field removed"));
        assert!(stripped.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("drawing field SHAPE stripped without synthesizing drawing canvas output")
        }));
    }

    #[test]
    fn word_literal_and_drawing_fields_apply_bounded_layout_only_side_bearings() {
        let input = r#"{\rtf1 Quote {\field{\*\fldinst QUOTE "Visible"}} if {\field{\*\fldinst IF 1 = 1 "Yes" "No"}} compare {\field{\*\fldinst COMPARE 2 > 1}} macro {\field{\*\fldinst MACROBUTTON SafeMacro Button}} stored {\field{\*\fldinst SHAPE}{\fldrslt Drawing}} empty {\field{\*\fldinst SHAPE}}\par}"#;
        let output = parse_rtf(input).unwrap();
        let text = document_text(&output.document);
        let advances = output
            .document
            .blocks
            .iter()
            .flat_map(|block| match block {
                Block::Paragraph(paragraph) => paragraph.runs.iter().collect::<Vec<_>>(),
                _ => Vec::new(),
            })
            .filter(|run| run.text == PASSIVE_ADVANCE_MARKER)
            .collect::<Vec<_>>();

        assert_eq!(text.matches(PASSIVE_ADVANCE_MARKER).count(), 12);
        assert_eq!(advances.len(), 12);
        assert!(advances.iter().all(|run| {
            run.style.character_spacing_twips == WORD_LITERAL_DRAWING_FIELD_SIDE_BEARING_TWIPS
                && run.style.baseline_shift_half_points == 0
        }));
        assert!(visible_document_text(&output.document).contains(
            "Quote Visible if Yes compare 1 macro Button stored Drawing empty [Field removed: no passive result]"
        ));

        let strict = parse_rtf_strict(input).unwrap();
        assert_eq!(
            document_text(&strict.document)
                .matches(PASSIVE_ADVANCE_MARKER)
                .count(),
            0
        );
    }

    #[test]
    fn resultless_active_control_fields_do_not_create_controls_or_leak_payloads() {
        let input = r#"{\rtf1 Before {\field{\*\fldinst CONTROL Forms.CommandButton.1 "Hidden caption"}} html {\field{\*\fldinst HTMLCHECKBOX "secret-name" checked}} select {\field{\*\fldinst HTMLSELECT "Secret option"}} After\par}"#;
        let output = parse_rtf(input).unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Before [Field removed: no passive result] html [Field removed: no passive result] select [Field removed: no passive result] After"),
            "text was {text:?}"
        );
        for forbidden in [
            "CONTROL",
            "HTMLCHECKBOX",
            "HTMLSELECT",
            "Forms.CommandButton",
            "Hidden caption",
            "secret-name",
            "checked",
            "Secret option",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "active control field leaked instruction or payload text: {forbidden}"
            );
        }
        for name in ["CONTROL", "HTMLCHECKBOX", "HTMLSELECT"] {
            assert!(
                output.diagnostics.iter().any(|diagnostic| {
                    diagnostic.message.contains(&format!(
                        "active control field {name} rendered as passive placeholder without creating active form or control content"
                    ))
                }),
                "missing diagnostic for {name}"
            );
        }
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("active control field CONTROL removed")
        }));

        let strip_options = RtfParseOptions {
            active_content_policy: ActiveContentPolicy::Strip,
            ..RtfParseOptions::default()
        };
        let stripped = parse_rtf_bytes_with_options(input.as_bytes(), &strip_options).unwrap();
        let stripped_text = document_text(&stripped.document);
        assert!(
            stripped_text.contains("Before  html  select  After"),
            "stripped text was {stripped_text:?}"
        );
        assert!(!stripped_text.contains("[Field removed"));
        assert!(stripped.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("active control field CONTROL stripped")
        }));
    }

    #[test]
    fn resultless_private_and_print_fields_are_stripped_without_payload_leakage() {
        let input = r#"{\rtf1 Before {\field{\*\fldinst ADDIN "Hidden addin payload"}} addin {\field{\*\fldinst PRIVATE "414243 hidden private"}} private {\field{\*\fldinst PRINT "hidden printer command"}} print {\field{\*\fldinst RD "C:\\secret\\source.rtf"}} after\par}"#;
        let output = parse_rtf(input).unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Before  addin  private  print  after"),
            "text was {text:?}"
        );
        assert!(!text.contains("[Field removed"));
        for forbidden in [
            "ADDIN",
            "PRIVATE",
            "PRINT",
            "RD",
            "Hidden addin payload",
            "414243",
            "hidden private",
            "hidden printer command",
            "secret",
            "source.rtf",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "hidden active/storage field leaked text: {forbidden}"
            );
        }
        for name in ["ADDIN", "PRIVATE", "PRINT"] {
            assert!(
                output.diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .message
                        .contains(&format!("active non-visible field {name} stripped"))
                }),
                "missing diagnostic for {name}"
            );
        }
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("external non-visible field RD stripped")
        }));
    }

    #[test]
    fn resultless_index_entry_fields_are_stripped_without_placeholders() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\field{\*\fldinst XE "Hidden index"}} middle {\field{\*\fldinst TC "Hidden toc"}} more {\field{\*\fldinst TA "Hidden authority"}} After\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before"));
        assert!(text.contains("middle"));
        assert!(text.contains("more"));
        assert!(text.contains("After"));
        assert!(!text.contains("Hidden"));
        assert!(!text.contains("XE"));
        assert!(!text.contains("TC"));
        assert!(!text.contains("TA"));
        assert!(!text.contains("[Field removed"));
        for name in ["XE", "TC", "TA"] {
            assert!(output.diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains(&format!("non-visible field {name} stripped"))
            }));
        }
    }

    #[test]
    fn resultless_page_fields_create_typed_dynamic_fields() {
        let output = parse_rtf(
            r"{\rtf1 Page {\field{\*\fldinst PAGE}} of {\field{\*\fldinst NUMPAGES}}\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert_eq!(
            document_dynamic_fields(&output.document),
            vec![DynamicField::PageNumber, DynamicField::TotalPages]
        );
        assert!(!text.contains("[Field removed"));
        assert!(!text.contains("PAGE"));
        assert!(!text.contains("NUMPAGES"));
    }

    #[test]
    fn passive_field_marker_bypass_is_limited_to_marker_fields() {
        assert!(
            apply_field_format_switches(
                "PAGE",
                PassiveFieldResult::dynamic(DynamicField::PageNumber),
            )
                .is_some()
        );
        assert!(
            apply_field_format_switches(
                "NUMPAGES",
                PassiveFieldResult::dynamic(DynamicField::TotalPages),
            )
                .is_some()
        );
        assert!(
            apply_field_format_switches(
                "PAGEREF Target",
                PassiveFieldResult {
                    dynamic_field: None,
                    text: bookmark_page_ref_marker(7),
                    font_name: None,
                    font_size_half_points: None,
                    form_field: false,
                },
            )
            .is_some()
        );

        assert!(
            apply_field_format_switches(
                r#"QUOTE "hostile""#,
                PassiveFieldResult::dynamic(DynamicField::PageNumber)
            )
            .is_none()
        );
        assert!(
            apply_field_format_switches(
                "MACROBUTTON Launch Visible",
                PassiveFieldResult {
                    dynamic_field: None,
                    text: bookmark_page_ref_marker(7),
                    font_name: None,
                    font_size_half_points: None,
                    form_field: false,
                },
            )
            .is_none()
        );
        assert!(
            apply_field_format_switches(
                "SYMBOL 65",
                PassiveFieldResult {
                    dynamic_field: None,
                    text: PASSIVE_ADVANCE_MARKER.to_string(),
                    font_name: Some("ZapfDingbats".to_string()),
                    font_size_half_points: None,
                    form_field: false,
                },
            )
            .is_none()
        );
    }

    #[test]
    fn passive_field_results_without_switches_obey_central_text_bounds() {
        let oversized = "x".repeat(MAX_PASSIVE_FIELD_FORMAT_TEXT_CHARS + 1);
        assert!(
            apply_field_format_switches(
                r#"QUOTE "hostile""#,
                PassiveFieldResult {
                    dynamic_field: None,
                    text: oversized.clone(),
                    font_name: None,
                    font_size_half_points: None,
                    form_field: false,
                },
            )
            .is_none()
        );
        assert!(
            apply_field_format_switches(
                r#"QUOTE "hostile""#,
                PassiveFieldResult {
                    dynamic_field: None,
                    text: "safe\nunsafe".to_string(),
                    font_name: None,
                    font_size_half_points: None,
                    form_field: false,
                },
            )
            .is_none()
        );
        assert!(
            apply_field_format_switches(
                "SYMBOL 65",
                PassiveFieldResult {
                    dynamic_field: None,
                    text: oversized,
                    font_name: Some("ZapfDingbats".to_string()),
                    font_size_half_points: None,
                    form_field: false,
                },
            )
            .is_none()
        );
    }

    #[test]
    fn resultless_quote_fields_render_bounded_passive_literal() {
        let output =
            parse_rtf(r#"{\rtf1 Before {\field{\*\fldinst QUOTE "Visible literal"}} After\par}"#)
                .unwrap();
        let text = document_text(&output.document);

        assert!(visible_document_text(&output.document).contains("Before Visible literal After"));
        assert!(!text.contains("QUOTE"));
        assert!(!text.contains("fldinst"));
        assert!(!text.contains("[Field removed"));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("rendering passive field QUOTE without executing field instruction")
        }));
    }

    #[test]
    fn resultless_quote_field_instruction_unicode_renders_passive_literal() {
        let output =
            parse_rtf(r#"{\rtf1 Before {\field{\*\fldinst QUOTE "Caf\u233? \u937?"}} After\par}"#)
                .unwrap();
        let text = document_text(&output.document);

        assert!(
            visible_document_text(&output.document).contains("Before Caf\u{00e9} \u{03a9} After")
        );
        assert!(!text.contains("QUOTE"));
        assert!(!text.contains("fldinst"));
        assert!(!text.contains('?'));
        assert!(!text.contains("[Field removed"));
    }

    #[test]
    fn resultless_instruction_literal_fields_reject_active_pdf_markers() {
        let output = parse_rtf(
            r#"{\rtf1 quote {\field{\*\fldinst QUOTE "/JavaScript"}} if {\field{\*\fldinst IF 1 = 1 "/EmbeddedFile" "Safe"}} macro {\field{\*\fldinst MACROBUTTON LaunchPayload "/Launch"}} go {\field{\*\fldinst GOTOBUTTON Target "/OpenAction"}} merge {\field{\*\fldinst MERGEFIELD "/URI"}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);
        let visible_text = visible_document_text(&output.document);

        assert!(
            visible_text.contains(
                "quote [Field removed: no passive result] if [Field removed: no passive result] macro [Field removed: no passive result] go [Field removed: no passive result] merge [Field removed: no passive result]"
            ),
            "text was {text:?}"
        );
        for forbidden in [
            "QUOTE",
            "IF",
            "MACROBUTTON",
            "GOTOBUTTON",
            "MERGEFIELD",
            "/JavaScript",
            "/EmbeddedFile",
            "/Launch",
            "/OpenAction",
            "/URI",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "active marker field literal leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_if_fields_render_passive_literal_branch() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\field{\*\fldinst IF 5 > 3 "Greater" "Lower"}} and {\field{\*\fldinst IF "Alpha" = "Beta" "Match" "Different"}} After\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            visible_document_text(&output.document).contains("Before Greater and Different After")
        );
        for forbidden in ["IF", "Alpha", "Beta", "fldinst", "[Field removed"] {
            assert!(
                !text.contains(forbidden),
                "forbidden IF field content leaked to text: {forbidden}"
            );
        }
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("rendering passive field IF without executing field instruction")
        }));
    }

    #[test]
    fn resultless_if_fields_resolve_bounded_set_and_bookmark_values() {
        let output = parse_rtf(
            r#"{\rtf1{\field{\*\fldinst SET Units "7"}}If {\field{\*\fldinst IF Units > 5 "High" "Low"}} {\*\bkmkstart Rate}4{\*\bkmkend Rate} rate {\field{\*\fldinst IF Rate = 4 "Match" "Miss" \\* Upper}} missing {\field{\*\fldinst IF Missing = 1 "Yes" "No"}} unsafe {\field{\*\fldinst SET Bad "PAYLOAD /JavaScript"}}{\field{\*\fldinst IF Bad = 1 "Yes" "No"}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);
        let visible_text = visible_document_text(&output.document);

        assert!(
            visible_text.contains("If High 4 rate MATCH missing No unsafe No"),
            "text was {text:?}"
        );
        for forbidden in [
            "SET",
            "IF",
            "Units > 5",
            "Rate = 4",
            "Missing = 1",
            "Bad = 1",
            "PAYLOAD",
            "fldinst",
            "[Field removed",
        ] {
            assert!(
                !text.contains(forbidden),
                "referenced IF field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_compare_fields_render_passive_boolean_numbers() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\field{\*\fldinst COMPARE 5 > 3}} and {\field{\*\fldinst COMPARE "Alpha" = "Beta"}} and {\field{\*\fldinst COMPARE 3 <= 3 \\* ROMAN}} After\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);
        let visible_text = visible_document_text(&output.document);

        assert!(
            visible_text.contains("Before 1 and 0 and I After"),
            "text was {text:?}"
        );
        for forbidden in [
            "COMPARE",
            "Alpha",
            "Beta",
            "fldinst",
            "ROMAN",
            "[Field removed",
        ] {
            assert!(
                !text.contains(forbidden),
                "forbidden COMPARE field content leaked to text: {forbidden}"
            );
        }
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("rendering passive field COMPARE without executing field instruction")
        }));
    }

    #[test]
    fn resultless_compare_fields_resolve_bounded_set_and_bookmark_values() {
        let output = parse_rtf(
            r#"{\rtf1{\field{\*\fldinst SET Units "7"}}Compare {\field{\*\fldinst COMPARE Units > 5}} {\*\bkmkstart Rate}4{\*\bkmkend Rate} rate {\field{\*\fldinst COMPARE Rate = 4 \\* ROMAN}} missing {\field{\*\fldinst COMPARE Missing = 1}} unsafe {\field{\*\fldinst SET Bad "PAYLOAD /JavaScript"}}{\field{\*\fldinst COMPARE Bad = 1}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);
        let visible_text = visible_document_text(&output.document);

        assert!(
            visible_text.contains("Compare 1 4 rate I missing 0 unsafe 0"),
            "text was {text:?}"
        );
        for forbidden in [
            "SET",
            "COMPARE",
            "Units > 5",
            "Rate = 4",
            "Missing = 1",
            "Bad = 1",
            "PAYLOAD",
            "/JavaScript",
            "ROMAN",
            "fldinst",
            "[Field removed",
        ] {
            assert!(
                !text.contains(forbidden),
                "referenced COMPARE field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn malformed_compare_fields_do_not_leak_instruction_tail() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\field{\*\fldinst COMPARE 1 = 1 HIDDEN-TRAIL}} After\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);
        let visible_text = visible_document_text(&output.document);

        assert!(
            visible_text.contains("Before [Field removed: no passive result] After"),
            "text was {text:?}"
        );
        for forbidden in ["COMPARE", "HIDDEN-TRAIL", "fldinst"] {
            assert!(
                !text.contains(forbidden),
                "malformed COMPARE field leaked instruction text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_fields_apply_passive_case_format_switches() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\field{\*\fldinst QUOTE "mixed Case" \\* Upper}} and {\field{\*\fldinst QUOTE "MIXED Case" \\* Lower \\* FirstCap}} and {\field{\*\fldinst IF 1 = 1 "checked status" "other" \\* Caps}} After\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);
        let visible_text = visible_document_text(&output.document);

        assert!(
            visible_text.contains("Before MIXED CASE and Mixed case and Checked Status After"),
            "normalized field switch text was {text:?}"
        );
        for forbidden in [
            "QUOTE",
            "IF",
            "fldinst",
            "Upper",
            "Lower",
            "FirstCap",
            "Caps",
            "[Field removed",
        ] {
            assert!(
                !text.contains(forbidden),
                "forbidden field switch content leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_fields_apply_passive_number_format_switches() {
        let output = parse_rtf(
            r#"{\rtf1 Values {\field{\*\fldinst SEQ Figure \\r 4 \\* ROMAN}} {\field{\*\fldinst SEQ Figure \\* roman}} {\field{\*\fldinst = 27 \\* alphabetic}} {\field{\*\fldinst = 27 \\* ALPHABETIC}} {\field{\*\fldinst = 255 \\* Hex}} {\field{\*\fldinst IF 1 = 1 "7" "0" \\* Ordinal}} {\field{\*\fldinst = 42 \\* CardText \\* MERGEFORMAT}} {\field{\*\fldinst = 513 \\* OrdText \\* CHARFORMAT}} {\field{\*\fldinst = 42 \\* DollarText}} {\field{\*\fldinst = 9 \\* ArabicDash}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);
        let visible_text = visible_document_text(&output.document);

        assert!(
            visible_text.contains(
                "Values IV v aa AA FF 7th forty-two five hundred thirteenth forty-two and 00/100 - 9 -"
            ),
            "normalized field number-switch text was {text:?}"
        );
        for forbidden in [
            "SEQ",
            "IF",
            "fldinst",
            "ROMAN",
            "roman",
            "alphabetic",
            "ALPHABETIC",
            "Ordinal",
            "Hex",
            "CardText",
            "OrdText",
            "DollarText",
            "ArabicDash",
            "MERGEFORMAT",
            "CHARFORMAT",
            "[Field removed",
        ] {
            assert!(
                !text.contains(forbidden),
                "forbidden field number-switch content leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_sum_function_renders_bounded_passive_total() {
        let output = parse_rtf(
            r#"{\rtf1 Total {\field{\*\fldinst = SUM(1, 2 + 3, (4 * 5)) \\# "000"}} malformed {\field{\*\fldinst = AVG(1,2)}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Total 026 malformed [Field removed: no passive result]"));
        for forbidden in ["SUM", "AVG", "1,2", "2 + 3", "4 * 5", "fldinst", "\\#"] {
            assert!(
                !text.contains(forbidden),
                "formula function field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_resolves_bounded_set_and_bookmark_values() {
        let output = parse_rtf(
            r#"{\rtf1{\field{\*\fldinst SET Units "7"}}Units {\field{\*\fldinst = Units * 3 \\# "00"}} {\*\bkmkstart Rate}4{\*\bkmkend Rate} rate {\field{\*\fldinst = Rate + 2}} bad {\field{\*\fldinst = Missing + 1}} expr {\field{\*\fldinst SET Expr "2 + 3"}}{\field{\*\fldinst = Expr * 4}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Units 21 4 rate 6 bad [Field removed: no passive result] expr 20"),
            "normalized text was {text:?}"
        );
        for forbidden in [
            "SET",
            "Units * 3",
            "Rate + 2",
            "Missing + 1",
            "Expr * 4",
            "2 + 3",
            "\\#",
            "\"00\"",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "formula reference field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_semicolon_arguments_render_bounded_passive_values() {
        let output = parse_rtf(
            r#"{\rtf1 Sum {\field{\*\fldinst = SUM(1; 2 + 3; (4 * 5)) \\# "000"}} min {\field{\*\fldinst = MIN(9; 2 + 3; -4)}} bool {\field{\*\fldinst = AND(1 < 2; TRUE())}} malformed {\field{\*\fldinst = SUM(1;)}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Sum 026 min -4 bool 1 malformed [Field removed: no passive result]"),
            "normalized text was {text:?}"
        );
        for forbidden in [
            "SUM", "MIN", "AND", "1;", "2 + 3", "4 * 5", "TRUE()", "\\#", "\"000\"", "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "formula semicolon-argument field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_unary_plus_renders_bounded_passive_value() {
        let output = parse_rtf(
            r#"{\rtf1 Positive {\field{\*\fldinst = +5}} sum {\field{\*\fldinst = SUM(+1, 2)}} fn {\field{\*\fldinst = +ABS(-3)}} malformed {\field{\*\fldinst = ++5}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Positive 5 sum 3 fn 3 malformed [Field removed: no passive result]")
        );
        for forbidden in ["+5", "+1", "+ABS", "++5", "fldinst"] {
            assert!(
                !text.contains(forbidden),
                "formula unary plus field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_comparisons_render_bounded_passive_boolean_numbers() {
        let output = parse_rtf(
            r#"{\rtf1 Equal {\field{\*\fldinst = 2 + 2 = 4}} neq {\field{\*\fldinst = 3 <> 3}} less {\field{\*\fldinst = 2 < 5}} le {\field{\*\fldinst = 5 <= 5 \\* ROMAN}} gt {\field{\*\fldinst = 6 > 9}} ge {\field{\*\fldinst = 9 >= 6}} malformed {\field{\*\fldinst = 1 < 2 < 3}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains(
                "Equal 1 neq 0 less 1 le I gt 0 ge 1 malformed [Field removed: no passive result]"
            ),
            "normalized text was {text:?}"
        );
        for forbidden in [
            "2 + 2",
            "3 <> 3",
            "2 < 5",
            "5 <= 5",
            "6 > 9",
            "9 >= 6",
            "1 < 2 < 3",
            "ROMAN",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "formula comparison field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_nested_comparisons_render_bounded_passive_values() {
        let output = parse_rtf(
            r#"{\rtf1 Arithmetic {\field{\*\fldinst = (2 > 1) + 4}} and {\field{\*\fldinst = AND(1 < 2, TRUE)}} or {\field{\*\fldinst = OR(2 < 1, 3 = 3)}} not {\field{\*\fldinst = NOT(2 <> 2)}} malformed {\field{\*\fldinst = AND(1 < 2 < 3, TRUE)}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains(
                "Arithmetic 5 and 1 or 1 not 1 malformed [Field removed: no passive result]"
            ),
            "normalized text was {text:?}"
        );
        for forbidden in [
            "2 > 1",
            "1 < 2",
            "TRUE",
            "2 < 1",
            "3 = 3",
            "2 <> 2",
            "1 < 2 < 3",
            "AND",
            "OR",
            "NOT",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "nested formula comparison leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_boolean_constants_render_bounded_passive_values() {
        let output = parse_rtf(
            r#"{\rtf1 True {\field{\*\fldinst = TRUE}} false {\field{\*\fldinst = FALSE}} truefn {\field{\*\fldinst = TRUE()}} falsefn {\field{\*\fldinst = FALSE()}} sum {\field{\*\fldinst = TRUE() + 4}} cmp {\field{\*\fldinst = TRUE > FALSE()}} not {\field{\*\fldinst = NOT(FALSE())}} neg {\field{\*\fldinst = -TRUE}} badarity {\field{\*\fldinst = TRUE(1)}} malformed {\field{\*\fldinst = MAYBE()}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains(
                "True 1 false 0 truefn 1 falsefn 0 sum 5 cmp 1 not 1 neg -1 badarity [Field removed: no passive result] malformed [Field removed: no passive result]"
            ),
            "normalized text was {text:?}"
        );
        for forbidden in [
            "TRUE",
            "FALSE",
            "MAYBE",
            "NOT",
            "TRUE() + 4",
            "TRUE > FALSE()",
            "TRUE(1)",
            "MAYBE()",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "formula boolean constant field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_if_function_renders_bounded_passive_branch_values() {
        let output = parse_rtf(
            r#"{\rtf1 True {\field{\*\fldinst = IF(2 > 1, 7 + 3, 4)}} false {\field{\*\fldinst = IF(FALSE(); 1; 9) \\# "000"}} nested {\field{\*\fldinst = IF(AND(1 < 2, TRUE()), POWER(2,3), 0)}} arity {\field{\*\fldinst = IF(1,2)}} malformed {\field{\*\fldinst = IF(1 < 2 < 3, 7, 0)}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains(
                "True 10 false 009 nested 8 arity [Field removed: no passive result] malformed [Field removed: no passive result]"
            ),
            "normalized text was {text:?}"
        );
        for forbidden in [
            "IF",
            "2 > 1",
            "7 + 3",
            "FALSE()",
            "AND",
            "1 < 2",
            "TRUE()",
            "POWER",
            "1 < 2 < 3",
            "\\#",
            "\"000\"",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "formula IF field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_min_max_functions_render_bounded_passive_values() {
        let output = parse_rtf(
            r#"{\rtf1 Range {\field{\*\fldinst = MIN(9, 2 + 3, -4)}} / {\field{\*\fldinst = MAX(9, 2 + 3, -4) \\* ROMAN}} malformed {\field{\*\fldinst = PRODUCT(2,3)}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Range -4 / IX malformed 6"),
            "normalized text was {text:?}"
        );
        for forbidden in ["MIN", "MAX", "PRODUCT", "2 + 3", "fldinst", "ROMAN"] {
            assert!(
                !text.contains(forbidden),
                "formula min/max field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_abs_function_renders_bounded_passive_value() {
        let output = parse_rtf(
            r#"{\rtf1 Magnitude {\field{\*\fldinst = ABS(-8 + 3) \\# "00"}} malformed {\field{\*\fldinst = ABS(1,2)}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Magnitude 05 malformed [Field removed: no passive result]"));
        for forbidden in ["ABS", "-8 + 3", "1,2", "fldinst", "\\#"] {
            assert!(
                !text.contains(forbidden),
                "formula ABS field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_product_function_renders_bounded_passive_value() {
        let output = parse_rtf(
            r#"{\rtf1 Product {\field{\*\fldinst = PRODUCT(2, 3 + 1, -5)}} malformed {\field{\*\fldinst = ROUND(2,0)}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Product -40 malformed 2"),
            "normalized text was {text:?}"
        );
        for forbidden in ["PRODUCT", "ROUND", "3 + 1", "2,0", "fldinst"] {
            assert!(
                !text.contains(forbidden),
                "formula PRODUCT field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_power_function_renders_bounded_passive_value() {
        let output = parse_rtf(
            r#"{\rtf1 Power {\field{\*\fldinst = POWER(2 + 1, 4)}} negative {\field{\*\fldinst = POWER(2,-1)}} extra {\field{\*\fldinst = POWER(2,3,1)}} overflow {\field{\*\fldinst = POWER(999999999,9)}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains(
            "Power 81 negative [Field removed: no passive result] extra [Field removed: no passive result] overflow [Field removed: no passive result]"
        ));
        for forbidden in ["POWER", "2 + 1", "2,-1", "2,3,1", "999999999", "fldinst"] {
            assert!(
                !text.contains(forbidden),
                "formula POWER field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_exponent_operator_renders_bounded_passive_value() {
        let output = parse_rtf(
            r#"{\rtf1 Power {\field{\*\fldinst = 2 ^ 3}} precedence {\field{\*\fldinst = 2 * 3 ^ 2}} formatted {\field{\*\fldinst = (1 + 2) ^ 3 \\# "000"}} negative {\field{\*\fldinst = 2 ^ -1}} overflow {\field{\*\fldinst = 999999999 ^ 9}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains(
                "Power 8 precedence 18 formatted 027 negative [Field removed: no passive result] overflow [Field removed: no passive result]"
            ),
            "normalized text was {text:?}"
        );
        for forbidden in [
            "2 ^ 3",
            "3 ^ 2",
            "(1 + 2)",
            "2 ^ -1",
            "999999999 ^ 9",
            "\\#",
            "\"000\"",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "formula exponent field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_mod_function_renders_bounded_passive_value() {
        let output = parse_rtf(
            r#"{\rtf1 Remainder {\field{\*\fldinst = MOD(17 + 3, 6)}} zero {\field{\*\fldinst = MOD(5,0)}} extra {\field{\*\fldinst = MOD(5,2,1)}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains(
            "Remainder 2 zero [Field removed: no passive result] extra [Field removed: no passive result]"
        ));
        for forbidden in ["MOD", "17 + 3", "5,0", "5,2,1", "fldinst"] {
            assert!(
                !text.contains(forbidden),
                "formula MOD field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_quotient_function_renders_bounded_passive_value() {
        let output = parse_rtf(
            r#"{\rtf1 Quotient {\field{\*\fldinst = QUOTIENT(17 + 3, 6)}} neg {\field{\*\fldinst = QUOTIENT(-20; 6) \\# "00"}} zero {\field{\*\fldinst = QUOTIENT(5,0)}} extra {\field{\*\fldinst = QUOTIENT(5,2,1)}} overflow {\field{\*\fldinst = QUOTIENT(-9223372036854775808,-1)}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains(
                "Quotient 3 neg -03 zero [Field removed: no passive result] extra [Field removed: no passive result] overflow [Field removed: no passive result]"
            ),
            "normalized text was {text:?}"
        );
        for forbidden in [
            "QUOTIENT",
            "17 + 3",
            "-20; 6",
            "5,0",
            "5,2,1",
            "-9223372036854775808",
            "\\#",
            "\"00\"",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "formula QUOTIENT field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_round_zero_precision_renders_bounded_passive_value() {
        let output = parse_rtf(
            r#"{\rtf1 Rounded {\field{\*\fldinst = ROUND(17 + 3, 0)}} precision {\field{\*\fldinst = ROUND(5,1)}} extra {\field{\*\fldinst = ROUND(5,0,1)}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains(
            "Rounded 20 precision [Field removed: no passive result] extra [Field removed: no passive result]"
        ));
        for forbidden in ["ROUND", "17 + 3", "5,1", "5,0,1", "fldinst"] {
            assert!(
                !text.contains(forbidden),
                "formula ROUND field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_count_function_renders_bounded_passive_value() {
        let output = parse_rtf(
            r#"{\rtf1 Count {\field{\*\fldinst = COUNT(2, 3 + 1, -5)}} empty {\field{\*\fldinst = COUNT()}} malformed {\field{\*\fldinst = COUNT(1,)}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains(
                "Count 3 empty [Field removed: no passive result] malformed [Field removed: no passive result]"
            ),
            "normalized text was {text:?}"
        );
        for forbidden in ["COUNT", "3 + 1", "1,", "fldinst"] {
            assert!(
                !text.contains(forbidden),
                "formula COUNT field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_n_function_renders_bounded_passive_numeric_value() {
        let output = parse_rtf(
            r#"{\rtf1 Number {\field{\*\fldinst = N(7 + 3)}} bool {\field{\*\fldinst = N(TRUE())}} cmp {\field{\*\fldinst = N(2 > 1) \\# "000"}} empty {\field{\*\fldinst = N()}} extra {\field{\*\fldinst = N(1,2)}} malformed {\field{\*\fldinst = N(MAYBE)}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains(
                "Number 10 bool 1 cmp 001 empty [Field removed: no passive result] extra [Field removed: no passive result] malformed [Field removed: no passive result]"
            ),
            "normalized text was {text:?}"
        );
        for forbidden in [
            "N(", "7 + 3", "TRUE()", "2 > 1", "MAYBE", "\\#", "\"000\"", "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "formula N field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_even_odd_functions_render_bounded_passive_values() {
        let output = parse_rtf(
            r#"{\rtf1 Even {\field{\*\fldinst = EVEN(5)}} neg {\field{\*\fldinst = EVEN(-5)}} same {\field{\*\fldinst = EVEN(4) \\# "000"}} Odd {\field{\*\fldinst = ODD(4)}} zero {\field{\*\fldinst = ODD(0)}} negodd {\field{\*\fldinst = ODD(-4)}} extra {\field{\*\fldinst = EVEN(1,2)}} overflow {\field{\*\fldinst = EVEN(9223372036854775807)}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains(
                "Even 6 neg -6 same 004 Odd 5 zero 1 negodd -5 extra [Field removed: no passive result] overflow [Field removed: no passive result]"
            ),
            "normalized text was {text:?}"
        );
        for forbidden in [
            "EVEN",
            "ODD",
            "9223372036854775807",
            "1,2",
            "\\#",
            "\"000\"",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "formula EVEN/ODD field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_sign_function_renders_bounded_passive_value() {
        let output = parse_rtf(
            r#"{\rtf1 Negative {\field{\*\fldinst = SIGN(-8 + 3)}} zero {\field{\*\fldinst = SIGN(4 - 4)}} positive {\field{\*\fldinst = SIGN(9)}} extra {\field{\*\fldinst = SIGN(1,2)}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Negative -1 zero 0 positive 1 extra [Field removed: no passive result]")
        );
        for forbidden in ["SIGN", "-8 + 3", "4 - 4", "1,2", "fldinst"] {
            assert!(
                !text.contains(forbidden),
                "formula SIGN field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_int_function_renders_bounded_passive_value() {
        let output = parse_rtf(
            r#"{\rtf1 Integer {\field{\*\fldinst = INT(17 + 3)}} negative {\field{\*\fldinst = INT(-5)}} decimal {\field{\*\fldinst = INT(1.5)}} extra {\field{\*\fldinst = INT(1,2)}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains(
            "Integer 20 negative -5 decimal [Field removed: no passive result] extra [Field removed: no passive result]"
        ));
        for forbidden in ["INT", "17 + 3", "1.5", "1,2", "fldinst"] {
            assert!(
                !text.contains(forbidden),
                "formula INT field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_sqrt_exact_integer_renders_bounded_passive_value() {
        let output = parse_rtf(
            r#"{\rtf1 Root {\field{\*\fldinst = SQRT(9 + 7)}} nonsquare {\field{\*\fldinst = SQRT(2)}} negative {\field{\*\fldinst = SQRT(-1)}} extra {\field{\*\fldinst = SQRT(4,1)}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains(
            "Root 4 nonsquare [Field removed: no passive result] negative [Field removed: no passive result] extra [Field removed: no passive result]"
        ));
        for forbidden in ["SQRT", "9 + 7", "SQRT(2)", "-1", "4,1", "fldinst"] {
            assert!(
                !text.contains(forbidden),
                "formula SQRT field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_boolean_functions_render_bounded_passive_values() {
        let output = parse_rtf(
            r#"{\rtf1 And {\field{\*\fldinst = AND(1, 2 + 3, -5)}} false {\field{\*\fldinst = AND(1,0)}} Or {\field{\*\fldinst = OR(0, 4 - 4, 7)}} empty {\field{\*\fldinst = OR()}} malformed {\field{\*\fldinst = AND(1,)}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains(
            "And 1 false 0 Or 1 empty [Field removed: no passive result] malformed [Field removed: no passive result]"
        ));
        for forbidden in ["AND", "OR", "2 + 3", "4 - 4", "1,", "fldinst"] {
            assert!(
                !text.contains(forbidden),
                "formula boolean field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_not_function_renders_bounded_passive_value() {
        let output = parse_rtf(
            r#"{\rtf1 True {\field{\*\fldinst = NOT(0)}} false {\field{\*\fldinst = NOT(2 + 3)}} zeroexpr {\field{\*\fldinst = NOT(4 - 4)}} empty {\field{\*\fldinst = NOT()}} extra {\field{\*\fldinst = NOT(1,0)}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains(
            "True 1 false 0 zeroexpr 1 empty [Field removed: no passive result] extra [Field removed: no passive result]"
        ));
        for forbidden in ["NOT", "2 + 3", "4 - 4", "1,0", "fldinst"] {
            assert!(
                !text.contains(forbidden),
                "formula NOT field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_xor_function_renders_bounded_passive_values() {
        let output = parse_rtf(
            r#"{\rtf1 Odd {\field{\*\fldinst = XOR(1, 0, 4 - 4)}} even {\field{\*\fldinst = XOR(1, -5)}} empty {\field{\*\fldinst = XOR()}} malformed {\field{\*\fldinst = XOR(1,)}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains(
            "Odd 1 even 0 empty [Field removed: no passive result] malformed [Field removed: no passive result]"
        ));
        for forbidden in ["XOR", "4 - 4", "1,", "fldinst"] {
            assert!(
                !text.contains(forbidden),
                "formula XOR field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_formula_average_exact_integer_renders_bounded_passive_value() {
        let output = parse_rtf(
            r#"{\rtf1 Average {\field{\*\fldinst = AVERAGE(2, 3 + 1, 6)}} fractional {\field{\*\fldinst = AVERAGE(1,2)}} empty {\field{\*\fldinst = AVERAGE()}} malformed {\field{\*\fldinst = AVERAGE(1,)}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains(
            "Average 4 fractional [Field removed: no passive result] empty [Field removed: no passive result] malformed [Field removed: no passive result]"
        ));
        for forbidden in ["AVERAGE", "3 + 1", "1,2", "1,", "fldinst"] {
            assert!(
                !text.contains(forbidden),
                "formula AVERAGE field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_fields_apply_simple_passive_numeric_picture_switches() {
        let output = parse_rtf(
            r##"{\rtf1 Values {\field{\*\fldinst = 42 \\# "0000"}} {\field{\*\fldinst = 1234567 \\# "#,##0"}} {\field{\*\fldinst IF 1 = 1 "5" "0" \\# "$0.00"}} {\field{\*\fldinst = -8 \\# "000"}}\par}"##,
        )
        .unwrap();
        let text = document_text(&output.document);
        let visible_text = visible_document_text(&output.document);

        assert!(
            visible_text.contains("Values 0042 1,234,567 $5.00 -008"),
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
    }

    #[test]
    fn resultless_macrobutton_fields_render_passive_display_text() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\field{\*\fldinst MACROBUTTON LaunchPayload Visible button text}} After\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            visible_document_text(&output.document).contains("Before Visible button text After")
        );
        assert!(!text.contains("MACROBUTTON"));
        assert!(!text.contains("LaunchPayload"));
        assert!(!text.contains("fldinst"));
        assert!(!text.contains("[Field removed"));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("rendering passive field MACROBUTTON without executing field instruction")
        }));
    }

    #[test]
    fn resultless_symbol_fields_render_bounded_passive_text() {
        let output = parse_rtf(
            r#"{\rtf1{\fonttbl{\f0 Arial;}{\f1\fcharset2 Symbol;}}Before {\field{\*\fldinst SYMBOL 183 \\f "Symbol"}} After\par}"#,
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();
        let symbol_run = paragraph
            .runs
            .iter()
            .find(|run| run.text == "\u{2022}")
            .expect("passive symbol field result");

        assert_eq!(text, "Before \u{2022} After");
        assert_eq!(symbol_run.style.font_index, 1);
        assert!(!text.contains("SYMBOL"));
        assert!(!text.contains("fldinst"));
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("rendering passive field SYMBOL without executing field instruction")
        }));
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("active content removed: field instruction")
        }));
    }

    #[test]
    fn field_switch_scanners_ignore_quoted_payload_text() {
        assert_eq!(
            field_switch_string_value(r#"SYMBOL 65 "literal \f Symbol""#, b'f'),
            None
        );
        assert_eq!(
            field_switch_string_value(r#"SYMBOL 65 "literal \f Symbol" \f Wingdings"#, b'f'),
            Some("Wingdings".to_string())
        );
        assert_eq!(
            field_switch_i32_value(r#"SYMBOL 65 "literal \s 72" \s 12"#, 's'),
            Some(12)
        );
        assert_eq!(
            field_switch_i32_value(r#"SYMBOL 65 "literal \s 72""#, 's'),
            None
        );
    }

    #[test]
    fn date_fields_render_stored_result_without_updating() {
        let output = parse_rtf(
            r#"{\rtf1 Created {\field{\*\fldinst DATE \\@ "MMMM d, yyyy"}{\fldrslt Stored visible date}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Created Stored visible date"));
        assert!(!text.contains("DATE"));
        assert!(!text.contains("MMMM"));
        assert!(!text.contains("fldinst"));
        assert!(!text.contains("[Field removed"));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("rendering stored result for field DATE")
        }));
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("active content removed: field instruction")
        }));
    }

    #[test]
    fn resultless_date_fields_are_not_updated_dynamically() {
        let output =
            parse_rtf(r#"{\rtf1 Before {\field{\*\fldinst DATE \\@ "yyyy"}} After\par}"#).unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before  After"));
        assert!(!text.contains("[Field removed: no passive result]"));
        assert!(!text.contains("DATE"));
        assert!(!text.contains("yyyy"));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("resultless field DATE omitted for Word-compatible passive rendering without dynamic evaluation")
        }));

        let strict_options = RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        };
        let strict = parse_rtf_bytes_with_options(
            br#"{\rtf1 Before {\field{\*\fldinst DATE \\@ "yyyy"}} After\par}"#,
            &strict_options,
        )
        .unwrap();
        assert!(document_text(&strict.document).contains("[Field removed: no passive result]"));
    }

    #[test]
    fn resultless_document_timestamp_fields_render_from_metadata_only() {
        let options = RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br#"{\rtf1{\info{\creatim\yr2024\mo7\dy5\hr14\min30\sec9}{\revtim\yr2025\mo1\dy2\hr9\min4\sec5}{\printim\yr2026\mo12\dy31}}Created {\field{\*\fldinst CREATEDATE \\@ "dddd, MMMM d, yyyy 'at' h:mm A/P"}} saved {\field{\*\fldinst SAVEDATE \\@ "ddd yyyy-MM-dd h:mm a/p"}} printed {\field{\*\fldinst PRINTDATE \\@ "M/d/yy"}} missing {\field{\*\fldinst SAVEDATE \\@ "unknown-token"}} dynamic {\field{\*\fldinst DATE \\@ "yyyy"}}\par}"#,
            &options,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Created Friday, July 5, 2024 at 2:30 P"));
        assert!(text.contains("saved Thu 2025-01-02 9:04 a"));
        assert!(text.contains("printed 12/31/26"));
        assert_eq!(
            text.matches("[Field removed: no passive result]").count(),
            2
        );
        for forbidden in [
            "CREATEDATE",
            "SAVEDATE",
            "PRINTDATE",
            "DATE",
            "MMMM",
            "yyyy",
            "unknown-token",
            "creatim",
            "revtim",
            "printim",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "timestamp field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_edit_time_fields_render_from_metadata_only() {
        let options = RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br#"{\rtf1{\info{\edmins42}}Edit {\field{\*\fldinst EDITTIME \\* ROMAN}} raw {\field{\*\fldinst EDITTIME \\# "0000"}}\par}"#,
            &options,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Edit XLII raw 0042"));
        for forbidden in ["EDITTIME", "edmins", "fldinst", "[Field removed"] {
            assert!(
                !text.contains(forbidden),
                "edit time field leaked unsafe text: {forbidden}"
            );
        }

        let missing = parse_rtf_bytes_with_options(
            br"{\rtf1 Missing {\field{\*\fldinst EDITTIME}}\par}",
            &options,
        )
        .unwrap();
        let missing_text = document_text(&missing.document);
        assert!(missing_text.contains("Missing [Field removed: no passive result]"));
        assert!(!missing_text.contains("EDITTIME"));
    }

    #[test]
    fn resultless_revision_number_fields_render_from_metadata_only() {
        let options = RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br#"{\rtf1{\info{\version12}}Rev {\field{\*\fldinst REVNUM \\* ROMAN}} raw {\field{\*\fldinst REVNUM \\# "0000"}}\par}"#,
            &options,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Rev XII raw 0012"));
        for forbidden in ["REVNUM", "version", "fldinst", "[Field removed"] {
            assert!(
                !text.contains(forbidden),
                "revision number field leaked unsafe text: {forbidden}"
            );
        }

        let missing = parse_rtf_bytes_with_options(
            br"{\rtf1 Missing {\field{\*\fldinst REVNUM}}\par}",
            &options,
        )
        .unwrap();
        let missing_text = document_text(&missing.document);
        assert!(missing_text.contains("Missing [Field removed: no passive result]"));
        assert!(!missing_text.contains("REVNUM"));
    }

    #[test]
    fn resultless_section_fields_create_typed_dynamic_fields() {
        let output = parse_rtf(r"{\rtf1 Section {\field{\*\fldinst SECTION}}\par}").unwrap();
        let text = document_text(&output.document);

        assert_eq!(
            document_dynamic_fields(&output.document),
            vec![DynamicField::SectionNumber]
        );
        assert!(!text.contains("[Field removed"));
        assert!(!text.contains("SECTION"));
        assert!(!text.contains("fldinst"));
    }

    #[test]
    fn field_instruction_growth_is_bounded() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_field_instruction_chars: 4,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        assert!(matches!(
            parse_rtf_bytes_with_options(
                br"{\rtf1{\field{\*\fldinst PAG\~EL}}}",
                &options,
            ),
            Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "field instruction"
        ));
    }

    #[test]
    fn field_instruction_limit_is_independent_from_visible_text_runs() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_text_run_len: 64,
                max_field_instruction_chars: 4,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        let visible = parse_rtf_bytes_with_options(br"{\rtf1 abcdefgh\par}", &options).unwrap();
        assert!(document_text(&visible.document).contains("abcdefgh"));

        assert!(matches!(
            parse_rtf_bytes_with_options(br"{\rtf1{\field{\*\fldinst HYPERLINK}}}", &options),
            Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "field instruction"
        ));
    }

    #[test]
    fn explicit_list_marker_field_result_remains_marker_text() {
        let output = parse_rtf(
            r#"{\rtf1{\pntext{\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt Link marker}}\tab}Body\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Link marker\tBody"), "text: {text:?}");
        for forbidden in [
            "HYPERLINK",
            "https://example.com",
            "fldinst",
            "fldrslt",
            "[Field removed",
        ] {
            assert!(
                !text.contains(forbidden),
                "explicit list marker field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_docproperty_fields_render_safe_metadata_values() {
        let output = parse_rtf(
            r#"{\rtf1{\info{\title Visible \u937? Title}{\author Alice}{\operator Bob}{\doccomm Hidden comment}{\hlinkbase https://example.test/base}}Title: {\field{\*\fldinst DOCPROPERTY Title}} by {\field{\*\fldinst DOCPROPERTY Author \\* Upper}} saved by {\field{\*\fldinst DOCPROPERTY LastSavedBy}} base {\field{\*\fldinst DOCPROPERTY "Hyperlink Base"}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains(
            "Title: Visible \u{3a9} Title by ALICE saved by Bob base https://example.test/base"
        ));
        for forbidden in [
            "DOCPROPERTY",
            "Hyperlink Base",
            "hlinkbase",
            "fldinst",
            "Hidden comment",
        ] {
            assert!(
                !text.contains(forbidden),
                "document property field leaked unsafe text: {forbidden}"
            );
        }
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("rendering passive field DOCPROPERTY without executing field instruction")
        }));
    }

    #[test]
    fn resultless_docproperty_timestamp_aliases_render_from_metadata_only() {
        let output = parse_rtf(
            r#"{\rtf1{\info{\creatim\yr2024\mo7\dy5\hr14\min30\sec9}{\revtim\yr2025\mo1\dy2\hr9\min4\sec5}{\printim\yr2026\mo12\dy31}}Created {\field{\*\fldinst DOCPROPERTY "Creation Date" \\@ "MMMM d, yyyy"}} saved {\field{\*\fldinst DOCPROPERTY "Last Save Time" \\@ "yyyy-MM-dd h:mm A/P"}} printed {\field{\*\fldinst DOCPROPERTY "Last Printed"}} missing {\field{\*\fldinst DOCPROPERTY "Last Saved Date" \\@ "bad-token"}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains(
                "Created July 5, 2024 saved 2025-01-02 9:04 A printed 2026-12-31 00:00:00"
            )
        );
        assert_eq!(
            text.matches("[Field removed: no passive result]").count(),
            1
        );
        for forbidden in [
            "DOCPROPERTY",
            "Creation Date",
            "Last Save Time",
            "Last Printed",
            "Last Saved Date",
            "bad-token",
            "creatim",
            "revtim",
            "printim",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "DOCPROPERTY timestamp alias leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_docproperty_stat_aliases_create_typed_dynamic_fields() {
        let output = parse_rtf(
            r#"{\rtf1 Alpha beta\page Gamma stats pages {\field{\*\fldinst DOCPROPERTY "Number of Pages"}} words {\field{\*\fldinst DOCPROPERTY "Number of Words"}} chars {\field{\*\fldinst DOCPROPERTY "Number of Characters"}} spaces {\field{\*\fldinst DOCPROPERTY "Number of Characters With Spaces"}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert_eq!(
            document_dynamic_fields(&output.document),
            vec![
                DynamicField::TotalPages,
                DynamicField::DocumentWords,
                DynamicField::DocumentCharacters,
                DynamicField::DocumentCharactersWithSpaces,
            ]
        );
        for forbidden in [
            "DOCPROPERTY",
            "Number of Pages",
            "Number of Words",
            "Number of Characters",
            "fldinst",
            "[Field removed",
        ] {
            assert!(
                !text.contains(forbidden),
                "DOCPROPERTY stat alias leaked unsafe text: {forbidden}"
            );
        }
        assert!(
            apply_field_format_switches(
                r#"DOCPROPERTY "Number of Words""#,
                PassiveFieldResult::dynamic(DynamicField::DocumentWords),
            )
            .is_some()
        );
        assert!(
            apply_field_format_switches(
                r#"DOCPROPERTY Title"#,
                PassiveFieldResult::dynamic(DynamicField::DocumentWords),
            )
            .is_none()
        );
    }

    #[test]
    fn resultless_docproperty_stat_aliases_prefer_stored_metadata_counts() {
        let output = parse_rtf(
            r##"{\rtf1{\info\nofpages7\nofwords1234\nofchars5678\nofcharsws6789}Stats pages {\field{\*\fldinst DOCPROPERTY "Number of Pages" \\# "000"}} words {\field{\*\fldinst DOCPROPERTY "Number of Words" \\# "#,##0"}} chars {\field{\*\fldinst INFO "Number of Characters"}} spaces {\field{\*\fldinst INFO "Number of Characters With Spaces"}}\par}"##,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Stats pages 007 words 1,234 chars 5678 spaces 6789"),
            "stored metadata stats were not rendered passively: {text:?}"
        );
        for forbidden in [
            "DOCPROPERTY",
            "Number of Pages",
            "Number of Words",
            "Number of Characters",
            "nofpages",
            "nofwords",
            "nofchars",
            "nofcharsws",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "stored stat metadata field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_docproperty_numeric_aliases_render_from_metadata_only() {
        let output = parse_rtf(
            r##"{\rtf1{\info{\version42}{\edmins17}\nofparas3\noflines12\nofbytes4096}Revision {\field{\*\fldinst DOCPROPERTY "Revision Number" \\* ROMAN}} edit {\field{\*\fldinst DOCPROPERTY "Total Editing Time" \\# "0000"}} paragraphs {\field{\*\fldinst DOCPROPERTY "Number of Paragraphs"}} lines {\field{\*\fldinst DOCPROPERTY "Number of Lines" \\# "000"}} bytes {\field{\*\fldinst DOCPROPERTY "Number of Bytes" \\# "#,##0"}} missing {\field{\*\fldinst DOCPROPERTY "Editing Time"}}\par}"##,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Revision XLII edit 0017 paragraphs 3 lines 012 bytes 4,096 missing 17")
        );
        for forbidden in [
            "DOCPROPERTY",
            "Revision Number",
            "Total Editing Time",
            "Number of Paragraphs",
            "Number of Lines",
            "Number of Bytes",
            "Editing Time",
            "version",
            "edmins",
            "nofparas",
            "noflines",
            "nofbytes",
            "fldinst",
            "[Field removed",
        ] {
            assert!(
                !text.contains(forbidden),
                "DOCPROPERTY numeric alias leaked unsafe text: {forbidden}"
            );
        }

        let missing =
            parse_rtf(r#"{\rtf1 Missing {\field{\*\fldinst DOCPROPERTY "Revision Number"}}\par}"#)
                .unwrap();
        let missing_text = document_text(&missing.document);
        assert!(missing_text.contains("Missing [Field removed: no passive result]"));
        assert!(!missing_text.contains("Revision Number"));
    }

    #[test]
    fn resultless_shortcut_document_property_fields_render_safe_metadata_values() {
        let output = parse_rtf(
            r#"{\rtf1{\info{\title Visible Title}{\subject Visible Subject}{\author Alice}{\keywords alpha beta}{\operator Bob}{\doccomm Hidden comment}}Doc {\field{\*\fldinst TITLE}} by {\field{\*\fldinst AUTHOR \\* Upper}} about {\field{\*\fldinst SUBJECT}} tags {\field{\*\fldinst KEYWORDS}} saved {\field{\*\fldinst LASTSAVEDBY}} note {\field{\*\fldinst COMMENTS}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains(
            "Doc Visible Title by ALICE about Visible Subject tags alpha beta saved Bob note Hidden comment"
        ));
        for forbidden in [
            "TITLE",
            "AUTHOR",
            "SUBJECT",
            "KEYWORDS",
            "LASTSAVEDBY",
            "COMMENTS",
            "fldinst",
            "[Field removed",
        ] {
            assert!(
                !text.contains(forbidden),
                "shortcut document property field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_extended_shortcut_document_property_fields_render_safe_metadata_values() {
        let output = parse_rtf(
            r#"{\rtf1{\info{\manager Alice Manager}{\company Contoso \u937?}{\category Internal Use}{\doccomm Hidden comment}}Mgr {\field{\*\fldinst MANAGER}} company {\field{\*\fldinst COMPANY \\* Upper}} category {\field{\*\fldinst CATEGORY}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Mgr Alice Manager company CONTOSO \u{3a9} category Internal Use"));
        for forbidden in [
            "MANAGER",
            "COMPANY",
            "CATEGORY",
            "fldinst",
            "Hidden comment",
            "[Field removed",
        ] {
            assert!(
                !text.contains(forbidden),
                "extended shortcut document property field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_info_fields_render_safe_metadata_values() {
        let output = parse_rtf(
            r#"{\rtf1{\info{\title Info Title}{\author Alice}{\operator Bob}{\doccomm Comment text}{\hlinkbase https://example.test/base}}Doc {\field{\*\fldinst INFO Title}} by {\field{\*\fldinst INFO Author \\* Upper}} saved {\field{\*\fldinst INFO "Last Author"}} modified {\field{\*\fldinst INFO "Last Modified By" \\* Upper}} note {\field{\*\fldinst INFO Comments}} base {\field{\*\fldinst INFO "Hyperlink Base"}} file {\field{\*\fldinst INFO Filename}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains(
                "Doc Info Title by ALICE saved Bob modified BOB note Comment text base https://example.test/base file"
            )
        );
        assert_eq!(
            text.matches("[Field removed: no passive result]").count(),
            1
        );
        for forbidden in [
            "fldinst",
            "Last Author",
            "Last Modified By",
            "Hyperlink Base",
            "hlinkbase",
            "Filename",
        ] {
            assert!(
                !text.contains(forbidden),
                "INFO field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_docproperty_fields_render_safe_custom_properties() {
        let output = parse_rtf(
            r#"{\rtf1{\*\userprops{\propname Client Name}{\proptype30}{\staticval Contoso \u937? {\staticval Nested overwrite}{\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt Hidden link}} tail}{\linkval Hidden linked value}}Client: {\field{\*\fldinst DOCPROPERTY "Client Name" \\* Upper}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Client: CONTOSO \u{3a9}  TAIL"));
        for forbidden in [
            "DOCPROPERTY",
            "fldinst",
            "Nested overwrite",
            "HYPERLINK",
            "https://example.com",
            "Hidden link",
            "Hidden linked value",
        ] {
            assert!(
                !text.contains(forbidden),
                "custom document property leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_docvariable_fields_render_safe_metadata_values() {
        let output = parse_rtf(
            r#"{\rtf1{\*\docvar {Client Name}{Contoso \u937? {\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt Hidden link}} tail}}Client: {\field{\*\fldinst DOCVARIABLE "Client Name" \\* Upper}} missing {\field{\*\fldinst DOCVARIABLE Missing}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Client: CONTOSO \u{3a9}  TAIL"));
        assert_eq!(
            text.matches("[Field removed: no passive result]").count(),
            1
        );
        for forbidden in [
            "DOCVARIABLE",
            "docvar",
            "fldinst",
            "HYPERLINK",
            "https://example.com",
            "Hidden link",
        ] {
            assert!(
                !text.contains(forbidden),
                "document variable leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_docvariable_fields_render_hex_metadata_values() {
        let output = parse_rtf(
            r#"{\rtf1{\*\docvar {Client}{Con\'74oso {\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt Hidden link}} ta\'69l}}Client: {\field{\*\fldinst DOCVARIABLE Client}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Client: Contoso  tail"),
            "hex-escaped document variable text should be captured for passive field rendering: {text:?}"
        );
        for forbidden in [
            "DOCVARIABLE",
            "docvar",
            "fldinst",
            "HYPERLINK",
            "https://example.com",
            "Hidden link",
            "\\'74",
            "\\'69",
        ] {
            assert!(
                !text.contains(forbidden),
                "document variable hex metadata leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn form_field_metadata_does_not_become_body_text() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\field{\*\fldinst FORMTEXT}{\formfield{\fftype0}{\ffname HiddenName}{\ffdeftext HiddenDefault}{\ffentrymcr launch.exe}{\ffexitmcr https://example.com}{\datafield 414243}}{\fldrslt Visible value}} After\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before Visible value After"));
        for forbidden in [
            "FORMTEXT",
            "HiddenName",
            "HiddenDefault",
            "launch.exe",
            "https://example.com",
            "414243",
            "ffentrymcr",
            "ffexitmcr",
            "datafield",
        ] {
            assert!(
                !text.contains(forbidden),
                "forbidden form-field metadata leaked to text: {forbidden}"
            );
        }
    }
