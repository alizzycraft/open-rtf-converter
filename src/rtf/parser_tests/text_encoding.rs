    #[test]
    fn extracts_unicode_and_skips_fallback() {
        let output = parse_rtf(r"{\rtf1\ansi\uc1 Hello \u8212- world\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        assert_eq!(paragraph.runs[0].text, "Hello \u{2014} world");
    }

    #[test]
    fn clamps_unicode_fallback_skip_count() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_unicode_fallback_skip: 1,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\ansi\uc999 Before \u8212- after\par}", &options)
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "Before \u{2014} after");
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("Unicode fallback skip clamped from 999 to 1")
        }));
    }

    #[test]
    fn prefers_unicode_alternate_destination_over_ansi_fallback() {
        let output =
            parse_rtf(r"{\rtf1 Before {\upr{fallback-ascii}{\*\ud{\u937?}}} After\par}").unwrap();
        let text = document_text(&output.document);
        assert!(text.contains("Before \u{03a9} After"));
        assert!(!text.contains("fallback-ascii"));
        assert!(!text.contains("upr"));
        assert!(!text.contains("ud"));
    }

    #[test]
    fn unicode_alternate_destination_preserves_parent_destination() {
        let output = parse_rtf(
            r"{\rtf1{\header Header {\upr{fallback-header}{\*\ud{\u937?}}}\par}Body\par}",
        )
        .unwrap();
        assert_eq!(output.document.header.len(), 1);
        assert_eq!(output.document.header[0].runs[0].text, "Header \u{03a9}");
        assert_eq!(document_text(&output.document), "Body");
    }

    #[test]
    fn decodes_hex_escapes_with_windows_1252_semantics() {
        let output =
            parse_rtf(r"{\rtf1\ansi\ansicpg1252 Quote \'93Hello\'94 dash \'97\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(text, "Quote \u{201c}Hello\u{201d} dash \u{2014}");
    }

    #[test]
    fn decodes_direct_ansi_bytes_with_document_code_page() {
        let output =
            parse_rtf_bytes(b"{\\rtf1\\ansi\\ansicpg1252 Quote \x93Hello\x94 dash \x97\\par}")
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

        assert_eq!(text, "Quote \u{201c}Hello\u{201d} dash \u{2014}");
    }

    #[test]
    fn font_charset_overrides_direct_ansi_byte_decoding() {
        let output = parse_rtf_bytes(
            b"{\\rtf1\\ansi\\ansicpg1252{\\fonttbl{\\f0 Times New Roman;}{\\f37\\fcharset238 Times New Roman CE;}}\\f37 \xf6t \xe1rv\xedzt\xfbr\xf5\\par}",
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

        assert_eq!(text.trim(), "\u{f6}t \u{e1}rv\u{ed}zt\u{171}r\u{151}");
    }

    #[test]
    fn unicode_fallback_skip_consumes_hex_escape_tokens() {
        let output = parse_rtf(r"{\rtf1\ansi\uc1 Unicode \u8217\'92 kept\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(text, "Unicode \u{2019} kept");
    }

    #[test]
    fn unicode_surrogate_pairs_render_as_single_scalar_value() {
        let output = parse_rtf(r"{\rtf1\ansi\uc1 Face \u-10179?\u-8704? done\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(text, "Face \u{1f600} done");
    }

    #[test]
    fn unmatched_unicode_surrogates_do_not_cross_plain_text_boundaries() {
        let output =
            parse_rtf(r"{\rtf1\ansi\uc1 Broken \u-10179? text \u-8704? done\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(text, "Broken  text  done");
    }

    #[test]
    fn decodes_hex_escapes_with_ibm437_semantics() {
        let output = parse_rtf(r"{\rtf1\ansicpg437 high \'82 box \'b3 ascii \'41\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(text, "high \u{00e9} box \u{2502} ascii A");
        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn word_compatible_font_code_page_uses_global_hex_escape_decoding() {
        const INPUT: &str = r"{\rtf1\ansi\ansicpg1252{\fonttbl{\f0\cpg437 Terminal;}{\f1\cpg10000 Mac Face;}{\f2 Unsupported;}}\f0 IBM \'b3 \f1 Mac \'d2Hello\'d3 \f2 Win \'93Hello\'94\par}";
        let output = parse_rtf(INPUT).unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();
        let code_page_for = |name: &str| {
            output
                .document
                .fonts
                .iter()
                .find(|font| font.name == name)
                .and_then(|font| font.code_page)
                .unwrap_or_else(|| panic!("missing font code page {name}"))
        };

        assert_eq!(code_page_for("Terminal"), 437);
        assert_eq!(code_page_for("Mac Face"), 10000);
        assert_eq!(
            text,
            "IBM \u{00b3} Mac \u{00d2}Hello\u{00d3} Win \u{201c}Hello\u{201d}"
        );
        assert!(!text.contains("cpg"));
        assert!(!text.contains("Terminal"));
        for name in ["Terminal", "Mac Face"] {
            assert_eq!(
                output
                    .document
                    .fonts
                    .iter()
                    .find(|font| font.name == name)
                    .and_then(|font| font.alternate_name.as_deref()),
                Some("Calibri")
            );
        }
        assert_eq!(
            output
                .document
                .fonts
                .iter()
                .find(|font| font.name == "Unsupported")
                .and_then(|font| font.alternate_name.as_deref()),
            None
        );

        let strict = parse_rtf_bytes_with_options(
            INPUT.as_bytes(),
            &RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
        )
        .unwrap();
        assert_eq!(
            document_text(&strict.document),
            "IBM \u{2502} Mac \u{201c}Hello\u{201d} Win \u{201c}Hello\u{201d}"
        );
        assert!(
            strict
                .document
                .fonts
                .iter()
                .all(|font| font.alternate_name.is_none())
        );
    }

    #[test]
    fn word_compatible_font_code_page_uses_global_direct_byte_decoding() {
        let mut input =
            br"{\rtf1\ansi\ansicpg1252{\fonttbl{\f0\cpg437 Terminal;}}\f0 raw ".to_vec();
        input.push(0xb3);
        input.extend_from_slice(br"\par}");

        let output = parse_rtf_bytes(&input).unwrap();
        assert_eq!(document_text(&output.document), "raw \u{00b3}");

        let strict = parse_rtf_bytes_with_options(
            &input,
            &RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
        )
        .unwrap();
        assert_eq!(document_text(&strict.document), "raw \u{2502}");
    }

    #[test]
    fn font_charset_fallback_guides_hex_escape_decoding() {
        let output = parse_rtf(
            r"{\rtf1\ansi\ansicpg1252{\fonttbl{\f0\fcharset77 Mac Face;}{\f1\fcharset255 Oem Face;}}\f0 Mac \'d2Hello\'d3 \f1 OEM \'82 line \'b3\par}",
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

        assert_eq!(text, "Mac \u{201c}Hello\u{201d} OEM \u{00e9} line \u{2502}");
        assert!(!text.contains("fcharset"));
        assert!(!text.contains("Mac Face"));
        assert!(
            output
                .document
                .fonts
                .iter()
                .all(|font| { font.alternate_name.as_deref() == Some("Calibri") })
        );

        let strict = parse_rtf_bytes_with_options(
            br"{\rtf1\ansi\ansicpg1252{\fonttbl{\f0\fcharset77 Mac Face;}{\f1\fcharset255 Oem Face;}}\f0 Mac \'d2Hello\'d3 \f1 OEM \'82 line \'b3\par}",
            &RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
        )
        .unwrap();
        assert!(
            strict
                .document
                .fonts
                .iter()
                .all(|font| font.alternate_name.is_none())
        );
    }

    #[test]
    fn ansi_font_charset_overrides_legacy_document_code_page() {
        let output = parse_rtf(
            r"{\rtf1\ansi\ansicpg437{\fonttbl{\f0\fcharset0 Ansi Face;}}\f0 Quote \'93Hello\'94 dash \'97\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert_eq!(text, "Quote \u{201c}Hello\u{201d} dash \u{2014}");
        assert!(!text.contains("fcharset"));
        assert!(!text.contains("Ansi Face"));
    }

    #[test]
    fn central_european_font_charset_guides_hex_escape_decoding() {
        let output = parse_rtf(
            r"{\rtf1\ansi\ansicpg1252{\fonttbl{\f0 Times New Roman;}{\f37\fcharset238 Times New Roman CE;}}\f0 Latin \f37 \'f6t \'e1rv\'edzt\'fbr\'f5 \'fctvef\'far\'f3g\'e9p\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert_eq!(
            text,
            "Latin \u{00f6}t \u{00e1}rv\u{00ed}zt\u{0171}r\u{0151} \u{00fc}tvef\u{00fa}r\u{00f3}g\u{00e9}p"
        );
        assert!(!text.contains("fcharset"));
        assert!(!text.contains("Times New Roman CE"));
    }

    #[test]
    fn decodes_hex_escapes_with_windows_1250_semantics() {
        let output =
            parse_rtf(r"{\rtf1\ansi\ansicpg1250 Central \'8a\'9a \'8c\'9c \'8e\'9e\par}").unwrap();
        let text = document_text(&output.document);

        assert_eq!(
            text,
            "Central \u{0160}\u{0161} \u{015a}\u{015b} \u{017d}\u{017e}"
        );
    }

    #[test]
    fn cyrillic_font_charset_guides_hex_escape_decoding() {
        let output = parse_rtf(
            r"{\rtf1\ansi\ansicpg1252{\fonttbl{\f0 Times New Roman;}{\f38\fcharset204 Times New Roman Cyr;}}\f38 Cyrillic \'cf\'f0\'e8\'e2\'e5\'f2\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert_eq!(
            text,
            "Cyrillic \u{041f}\u{0440}\u{0438}\u{0432}\u{0435}\u{0442}"
        );
        assert!(!text.contains("fcharset"));
        assert!(!text.contains("Times New Roman Cyr"));
    }

    #[test]
    fn decodes_hex_escapes_with_windows_1251_semantics() {
        let output =
            parse_rtf(r"{\rtf1\ansi\ansicpg1251 Cyrillic \'c0\'df \'e0\'ff \'a8\'b8\par}").unwrap();
        let text = document_text(&output.document);

        assert_eq!(
            text,
            "Cyrillic \u{0410}\u{042f} \u{0430}\u{044f} \u{0401}\u{0451}"
        );
    }

    #[test]
    fn greek_font_charset_guides_hex_escape_decoding() {
        let output = parse_rtf(
            r"{\rtf1\ansi\ansicpg1252{\fonttbl{\f0 Times New Roman;}{\f40\fcharset161 Times New Roman Greek;}}\f40 Greek \'c1\'e1 \'d0\'f0 \'d9\'f9\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert_eq!(
            text,
            "Greek \u{0391}\u{03b1} \u{03a0}\u{03c0} \u{03a9}\u{03c9}"
        );
        assert!(!text.contains("fcharset"));
        assert!(!text.contains("Times New Roman Greek"));
    }

    #[test]
    fn decodes_hex_escapes_with_windows_1253_semantics() {
        let output =
            parse_rtf(r"{\rtf1\ansi\ansicpg1253 Greek \'c1\'e1 \'d3\'f3 \'da\'fa\par}").unwrap();
        let text = document_text(&output.document);

        assert_eq!(
            text,
            "Greek \u{0391}\u{03b1} \u{03a3}\u{03c3} \u{03aa}\u{03ca}"
        );
    }

    #[test]
    fn turkish_font_charset_guides_hex_escape_decoding() {
        let output = parse_rtf(
            r"{\rtf1\ansi\ansicpg1252{\fonttbl{\f0 Times New Roman;}{\f41\fcharset162 Times New Roman Tur;}}\f41 Turkish \'d0\'dd\'de \'f0\'fd\'fe\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert_eq!(
            text,
            "Turkish \u{011e}\u{0130}\u{015e} \u{011f}\u{0131}\u{015f}"
        );
        assert!(!text.contains("fcharset"));
        assert!(!text.contains("Times New Roman Tur"));
    }

    #[test]
    fn decodes_hex_escapes_with_windows_1254_semantics() {
        let output =
            parse_rtf(r"{\rtf1\ansi\ansicpg1254 Turkish \'d0\'dd\'de \'f0\'fd\'fe\par}").unwrap();
        let text = document_text(&output.document);

        assert_eq!(
            text,
            "Turkish \u{011e}\u{0130}\u{015e} \u{011f}\u{0131}\u{015f}"
        );
    }

    #[test]
    fn baltic_font_charset_guides_hex_escape_decoding() {
        let output = parse_rtf(
            r"{\rtf1\ansi\ansicpg1252{\fonttbl{\f0 Times New Roman;}{\f42\fcharset186 Times New Roman Baltic;}}\f42 Baltic \'c0\'e0 \'c8\'e8 \'d8\'f8 \'da\'fa \'dd\'fd\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert_eq!(
            text,
            "Baltic \u{0104}\u{0105} \u{010c}\u{010d} \u{0172}\u{0173} \u{015a}\u{015b} \u{017b}\u{017c}"
        );
        assert!(!text.contains("fcharset"));
        assert!(!text.contains("Times New Roman Baltic"));
    }

    #[test]
    fn decodes_hex_escapes_with_windows_1257_semantics() {
        let output =
            parse_rtf(r"{\rtf1\ansi\ansicpg1257 Baltic \'c1\'e1 \'cc\'ec \'d9\'f9\par}").unwrap();
        let text = document_text(&output.document);

        assert_eq!(
            text,
            "Baltic \u{012e}\u{012f} \u{0122}\u{0123} \u{0141}\u{0142}"
        );
    }

    #[test]
    fn explicit_font_code_page_overrides_font_charset_only_in_strict_spec() {
        const INPUT: &str = r"{\rtf1\ansi\ansicpg1252{\fonttbl{\f0\fcharset77\cpg437 Mixed Face;}}\f0 Mixed \'b3\par}";
        let output = parse_rtf(INPUT).unwrap();
        let text = document_text(&output.document);

        assert_eq!(text, "Mixed \u{2265}");
        let strict = parse_rtf_bytes_with_options(
            INPUT.as_bytes(),
            &RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
        )
        .unwrap();
        assert_eq!(document_text(&strict.document), "Mixed \u{2502}");
    }

    #[test]
    fn unsupported_font_code_page_is_ignored_by_word_mode_and_bounded_in_strict_spec() {
        const INPUT: &str =
            r"{\rtf1\ansi{\fonttbl{\f0\cpg932 ShiftJis;}}\f0 high \'82 ascii \'41\par}";
        let output = parse_rtf(INPUT).unwrap();
        let text = document_text(&output.document);

        assert_eq!(text, "high \u{201a} ascii A");
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("unsupported RTF font code page 932")
        }));
        let strict = parse_rtf_bytes_with_options(
            INPUT.as_bytes(),
            &RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
        )
        .unwrap();
        assert_eq!(document_text(&strict.document), "high \u{fffd} ascii A");
    }

    #[test]
    fn decodes_hex_escapes_with_ibm850_semantics() {
        let output = parse_rtf(r"{\rtf1\pca high \'9b line \'c4 ascii \'41\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(text, "high \u{00f8} line \u{2500} ascii A");
        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn decodes_hex_escapes_with_mac_roman_semantics() {
        let output =
            parse_rtf(r"{\rtf1\mac quote \'d2Hello\'d3 bullet \'a5 ascii \'41\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(text, "quote \u{201c}Hello\u{201d} bullet \u{2022} ascii A");
        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn unsupported_code_page_replaces_non_ascii_hex_escapes() {
        let output = parse_rtf(r"{\rtf1\ansicpg932 high \'82 ascii \'41\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(text, "high \u{fffd} ascii A");
        assert!(!output.diagnostics.is_empty());
    }

    #[test]
    fn normalizes_named_visible_text_controls() {
        let output = parse_rtf(
            r"{\rtf1 A\emdash B\endash C\bullet D\lquote E\rquote F\ldblquote G\rdblquote\par}",
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

        assert_eq!(
            text,
            "A\u{2014}B\u{2013}C\u{2022}D\u{2018}E\u{2019}F\u{201c}G\u{201d}"
        );
    }

    #[test]
    fn hidden_text_controls_do_not_become_body_text() {
        let output = parse_rtf(
            r"{\rtf1 Visible {\v hidden \emdash \bullet \tab \line \u8217? \'41 \~ \- \_ \\ \{ \}}{\v0 shown} {\vanish hidden2}{\vanish0 visible2}\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Visible"));
        assert!(text.contains("shown"));
        assert!(text.contains("visible2"));
        assert!(!text.contains("hidden"));
        assert!(!text.contains("hidden2"));
        assert!(!text.contains("\u{2014}"));
        assert!(!text.contains("\u{2022}"));
        assert!(!text.contains('\t'));
        assert!(!text.contains('\n'));
        assert!(!text.contains("\u{2019}"));
        assert!(!text.contains('A'));
        assert!(!text.contains('\u{00a0}'));
        assert!(!text.contains('\u{00ad}'));
        assert!(!text.contains('\u{2011}'));
        assert!(!text.contains('\\'));
        assert!(!text.contains('{'));
        assert!(!text.contains('}'));
    }

    #[test]
    fn word_associated_fonts_follow_character_ranges() {
        let output = parse_rtf(
            r"{\rtf1{\fonttbl{\f0\fswiss Calibri;}{\f1\froman Times New Roman;}}\loch\af0\hich\af0\dbch\af1\ltrch Latin \u20013? text\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let runs = paragraph
            .runs
            .iter()
            .map(|run| (run.text.as_str(), run.style.font_index))
            .collect::<Vec<_>>();

        assert_eq!(runs, vec![("Latin ", 0), ("中", 1), (" text", 0)]);
    }

    #[test]
    fn primary_font_name_stops_at_first_separator() {
        let input = r"{\rtf1{\fonttbl{\f0 Tuffy;Ignored;}{\f1 Tuff\u121?\'3bIgnored;}{\f2 Tuff\u121?\u59?Ignored;}}\f0 A\f1 B\f2 C\par}";
        let output = parse_rtf(input).unwrap();

        for index in [0, 1, 2] {
            let font = output
                .document
                .fonts
                .iter()
                .find(|font| font.index == index)
                .unwrap_or_else(|| panic!("missing font {index}"));
            assert_eq!(font.name, "Tuffy");
        }
        for forbidden in ["Ignored", "u121", "u59", "?", ";", "Tuff?"] {
            assert!(
                output
                    .document
                    .fonts
                    .iter()
                    .all(|font| !font.name.contains(forbidden)
                        && !font
                            .alternate_name
                            .as_deref()
                            .is_some_and(|name| name.contains(forbidden))),
                "primary font terminator/fallback leaked into font metadata: {forbidden}"
            );
        }
    }

    #[test]
    fn normalizes_font_family_hints_as_safe_metadata() {
        let input = r"{\rtf1{\fonttbl{\f0\froman Mystery Serif;}{\f1\fmodern Mystery Mono;}{\f2\ftech Symbolish;}}\f0 A\f1 B\f2 C\par}";
        let output = parse_rtf(input).unwrap();
        let family_for = |name: &str| {
            output
                .document
                .fonts
                .iter()
                .find(|font| font.name == name)
                .map(|font| font.family)
                .unwrap_or_else(|| panic!("missing font {name}"))
        };

        assert_eq!(family_for("Mystery Serif"), FontFamilyHint::Roman);
        assert_eq!(family_for("Mystery Mono"), FontFamilyHint::Modern);
        assert_eq!(family_for("Symbolish"), FontFamilyHint::Tech);
        assert_eq!(
            output.document.fonts[0].alternate_name.as_deref(),
            Some("Cambria")
        );
        assert_eq!(
            output.document.fonts[1].alternate_name.as_deref(),
            Some("Calibri")
        );
        assert_eq!(output.document.fonts[2].alternate_name, None);

        let strict = parse_rtf_bytes_with_options(
            input.as_bytes(),
            &RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
        )
        .unwrap();
        assert!(
            strict
                .document
                .fonts
                .iter()
                .all(|font| font.alternate_name.is_none())
        );
    }

    #[test]
    fn authored_font_alternate_overrides_word_hint_substitute() {
        let output = parse_rtf(
            r"{\rtf1{\fonttbl{\f0\froman Mystery Serif{\*\falt Caller Serif;};}}\f0 A\par}",
        )
        .unwrap();

        assert_eq!(
            output.document.fonts[0].alternate_name.as_deref(),
            Some("Caller Serif")
        );
    }

    #[test]
    fn normalizes_theme_font_hints_as_safe_family_metadata() {
        let input = r"{\rtf1{\fonttbl{\f0\flomajor Mystery Heading;}{\f1\fhiminor Mystery Body;}{\f2\fdbmajor Mystery EastAsia Heading;}{\f3\fbiminor Mystery Bidi Body;}}\f0 A\f1 B\f2 C\f3 D\par}";
        let output = parse_rtf(input).unwrap();
        let family_for = |name: &str| {
            output
                .document
                .fonts
                .iter()
                .find(|font| font.name == name)
                .map(|font| font.family)
                .unwrap_or_else(|| panic!("missing font {name}"))
        };

        assert_eq!(family_for("Mystery Heading"), FontFamilyHint::Roman);
        assert_eq!(family_for("Mystery Body"), FontFamilyHint::Swiss);
        assert_eq!(
            family_for("Mystery EastAsia Heading"),
            FontFamilyHint::Roman
        );
        assert_eq!(family_for("Mystery Bidi Body"), FontFamilyHint::Swiss);
        assert_eq!(
            output.document.fonts[0].alternate_name.as_deref(),
            Some("Times New Roman")
        );
        assert_eq!(
            output.document.fonts[1].alternate_name.as_deref(),
            Some("Aptos")
        );
        assert_eq!(output.document.fonts[2].alternate_name, None);
        assert_eq!(output.document.fonts[3].alternate_name, None);
    }

    #[test]
    fn normalizes_font_pitch_hints_as_safe_metadata() {
        let input = r"{\rtf1{\fonttbl{\f0\fnil\fprq1 Mystery Fixed;}{\f1\fnil\fprq2 Mystery Variable;}{\f2\fnil\fprq42 Mystery Default;}}\f0 Fixed\f1 Variable\f2 Default\par}";
        let output = parse_rtf(input).unwrap();
        let pitch_for = |name: &str| {
            output
                .document
                .fonts
                .iter()
                .find(|font| font.name == name)
                .map(|font| font.pitch)
                .unwrap_or_else(|| panic!("missing font {name}"))
        };
        let text = document_text(&output.document);

        assert_eq!(pitch_for("Mystery Fixed"), FontPitch::Fixed);
        assert_eq!(pitch_for("Mystery Variable"), FontPitch::Variable);
        assert_eq!(pitch_for("Mystery Default"), FontPitch::Default);
        assert!(text.contains("Fixed"));
        assert!(text.contains("Variable"));
        assert!(text.contains("Default"));
        assert!(!text.contains("fprq"));
        assert!(!text.contains("Mystery Fixed"));
    }

    #[test]
    fn skips_inert_font_face_metadata_without_polluting_font_names() {
        let input = r"{\rtf1{\fonttbl{\f0\fnil\fprq2\fttruetype\fbias0{\panose 02020603050405020304}Metadata Sans;}{\f1\froman{\*\panose 05050102010706020507}Metadata Serif;}}\f0 Sans\f1 Serif\par}";
        let output = parse_rtf(input).unwrap();
        let font_name_for = |index: i32| {
            output
                .document
                .fonts
                .iter()
                .find(|font| font.index == index)
                .map(|font| font.name.as_str())
                .unwrap_or_else(|| panic!("missing font {index}"))
        };
        let text = document_text(&output.document);

        assert_eq!(font_name_for(0), "Metadata Sans");
        assert_eq!(font_name_for(1), "Metadata Serif");
        assert!(text.contains("SansSerif"));
        for forbidden in ["panose", "fttruetype", "fbias", "020206", "050501"] {
            assert!(
                !text.contains(forbidden),
                "font metadata leaked into body text: {forbidden}"
            );
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
            "font metadata should not be unsupported: {:?}",
            output.diagnostics
        );
    }

    #[test]
    fn normalizes_font_alternate_name_as_safe_metadata() {
        let input = r"{\rtf1{\fonttbl{\f0\fnil Mystery Sans{\*\falt Courier New;};}}\f0 Body\par}";
        let output = parse_rtf(input).unwrap();
        let text = document_text(&output.document);
        let font = output
            .document
            .fonts
            .iter()
            .find(|font| font.name == "Mystery Sans")
            .expect("font with alternate");

        assert_eq!(font.alternate_name.as_deref(), Some("Courier New"));
        assert!(text.contains("Body"));
        for forbidden in ["falt", "Courier New"] {
            assert!(
                !text.contains(forbidden),
                "font alternate metadata leaked into text: {forbidden}"
            );
        }
    }

    #[test]
    fn font_alternate_name_stops_at_first_separator() {
        let input = r"{\rtf1{\fonttbl{\f0 Mystery Raw{\*\falt Tuff\u121?;Ignored;};}{\f1 Mystery Hex{\*\falt Tuff\u121?\'3bIgnored;};}{\f2 Mystery Unicode{\*\falt Tuff\u121?\u59?Ignored;};}}\f0 A\f1 B\f2 C\par}";
        let output = parse_rtf(input).unwrap();

        for name in ["Mystery Raw", "Mystery Hex", "Mystery Unicode"] {
            let font = output
                .document
                .fonts
                .iter()
                .find(|font| font.name == name)
                .unwrap_or_else(|| panic!("missing font {name}"));
            assert_eq!(font.alternate_name.as_deref(), Some("Tuffy"));
        }
        for forbidden in ["Ignored", "u121", "u59", "?", ";", "Tuff?"] {
            assert!(
                output
                    .document
                    .fonts
                    .iter()
                    .all(|font| !font.name.contains(forbidden)
                        && !font
                            .alternate_name
                            .as_deref()
                            .is_some_and(|name| name.contains(forbidden))),
                "font alternate terminator/fallback leaked into font metadata: {forbidden}"
            );
        }
    }

    #[test]
    fn normalizes_font_switches_on_runs() {
        let input = r"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Courier New;}}\f1 Mono\par}";
        let output = parse_rtf(input).unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].style.font_index, 1);
        assert_eq!(paragraph.runs[0].text, "Mono");
    }

    #[test]
    fn default_font_control_sets_untagged_text_and_plain_reset_font() {
        let input = r"{\rtf1\deff1{\fonttbl{\f0\fswiss Arial;}{\f1\froman Times New Roman;}}\plain Default \f0 Sans \plain Back\par}";
        let output = parse_rtf(input).unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
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

        assert_eq!(font_for("Default"), 1);
        assert_eq!(font_for("Sans"), 0);
        assert_eq!(font_for("Back"), 1);
    }

    #[test]
    fn fontless_rtf_uses_word_legacy_default_font() {
        let output = parse_rtf(r"{\rtf1\ansi Default text\par}").unwrap();

        assert_eq!(output.document.fonts[0].index, 0);
        assert_eq!(output.document.fonts[0].name, "Times New Roman");
        assert_eq!(output.document.fonts[0].family, FontFamilyHint::Roman);
    }

    #[test]
    fn normalizes_symbol_font_charset_text_to_unicode() {
        let output =
            parse_rtf(r"{\rtf1{\fonttbl{\f0 Arial;}{\f1\fcharset2 Symbol;}}\f1 ab \'b7\par}")
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(output.document.fonts[1].charset, Some(2));
        assert_eq!(paragraph.runs[0].text, "\u{03b1}\u{03b2} \u{2022}");
    }

    #[test]
    fn normalizes_wingdings_checkbox_glyphs_to_safe_unicode() {
        let output = parse_rtf(
            r"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Wingdings;}}\f1 \'a3 \'fe \'fc \'fb \'fd \u61693?\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(
            paragraph.runs[0].text,
            "\u{25c9} \u{2611} \u{2713} \u{2717} \u{2612} \u{2612}"
        );
        assert_eq!(paragraph.runs[0].style.font_index, 1);
    }

    #[test]
    fn normalizes_wingdings2_checkbox_glyphs_to_safe_unicode() {
        let output =
            parse_rtf(r"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Wingdings 2;}}\f1 O P Q R S T\par}")
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(
            paragraph.runs[0].text,
            "\u{2717} \u{2713} \u{2612} \u{2611} \u{1f5f5} \u{1f5f7}"
        );
        assert_eq!(paragraph.runs[0].style.font_index, 1);
    }

    #[test]
    fn normalizes_webdings_checkbox_glyphs_to_safe_unicode() {
        let output =
            parse_rtf(r"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Webdings;}}\f1 \'3f \'61 \'63\par}")
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "\u{1f4e5} \u{2714} \u{25a1}");
        assert_eq!(paragraph.runs[0].style.font_index, 1);
    }

    #[test]
    fn prefixes_plain_pntext_marker_to_following_paragraph() {
        let output = parse_rtf(r"{\rtf1{\pntext 1.\tab}First item\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected list paragraph"),
        };
        assert_eq!(paragraph.runs[0].text, "1.\tFirst item");
    }

    #[test]
    fn word_suppresses_pntext_marker_when_ignorable_pn_metadata_follows() {
        let output = parse_rtf(
            r"{\rtf1{\fonttbl{\f0 Arial;}{\f1\fcharset2 Symbol;}}{\pntext\pard\plain\f1 \'b7\tab}{\*\pn\pnlvlblt\pnf1{\pntxtb \'b7}}\pard\fi-360\li360\tx360 Bullet item\par{\pntext I.\tab}{\*\pn\pnucrm{\pntxta .}}\pard\fi-720\li720\tx720 Roman item\par}",
        )
        .unwrap();
        let paragraph_text = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph
                .runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            _ => panic!("expected list paragraph"),
        };

        assert_eq!(paragraph_text(0), "Bullet item");
        assert_eq!(paragraph_text(1), "Roman item");
        let paragraph_style = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => &paragraph.style,
            _ => panic!("expected list paragraph"),
        };
        assert_eq!(paragraph_style(0).left_indent_twips, 0);
        assert_eq!(paragraph_style(0).first_line_indent_twips, 0);
        assert_eq!(paragraph_style(1).left_indent_twips, 0);
        assert_eq!(paragraph_style(1).first_line_indent_twips, 0);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn strict_spec_retains_explicit_pntext_marker_and_indents() {
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1{\fonttbl{\f0 Arial;}{\f1\fcharset2 Symbol;}}{\pntext\pard\plain\f1 \'b7\tab}{\*\pn\pnlvlblt\pnf1{\pntxtb \'b7}}\pard\fi-360\li360\tx360 Bullet item\par}",
            &RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected list paragraph"),
        };
        assert_eq!(
            paragraph_plain_text(paragraph).as_deref(),
            Some("\u{2022}\tBullet item")
        );
        assert_eq!(paragraph.style.left_indent_twips, 360);
        assert_eq!(paragraph.style.first_line_indent_twips, -360);
    }

    #[test]
    fn ignored_pn_section_level_marker_text_does_not_prefix_body_text() {
        let output =
            parse_rtf(r"{\rtf1{\*\pnseclvl1\pnucrm\pnstart1{\pntxta .}}Title text\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "Title text");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn synthesizes_multilevel_markers_from_leveltext_placeholders() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}{\listlevel\levelnfc0\levelstartat1{\leveltext\'04\'00.\'01.;}{\levelnumbers\'01\'03;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Top\par\pard\ls1\ilvl1 Child\par\pard\ls1\ilvl1 Child two\par\pard\ls1\ilvl0 Next top\par\pard\ls1\ilvl1 Next child\par}",
        )
        .unwrap();
        let paragraph_text = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph.runs[0].text.as_str(),
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph_text(0), "1.\tTop");
        assert_eq!(paragraph_text(1), "1.1.\tChild");
        assert_eq!(paragraph_text(2), "1.2.\tChild two");
        assert_eq!(paragraph_text(3), "2.\tNext top");
        assert_eq!(paragraph_text(4), "2.1.\tNext child");
    }

    #[test]
    fn unicode_in_ignored_destinations_does_not_become_body_text() {
        let output = parse_rtf(r"{\rtf1{\*\unknown \u8226? hidden}Visible\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "Visible");
    }

    #[test]
    fn strict_spec_resultless_form_text_renders_passive_default_without_metadata() {
        let options = RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br#"{\rtf1 Before {\field{\*\fldinst FORMTEXT}{\formfield{\fftype0}{\ffname HiddenName}{\ffdeftext Default \u937? value}{\ffentrymcr launch.exe}{\datafield 414243}}} After\par}"#,
            &options,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Before Default \u{03a9} value After"),
            "normalized text was {text:?}; diagnostics were {:?}",
            output.diagnostics
        );
        for forbidden in [
            "FORMTEXT",
            "HiddenName",
            "launch.exe",
            "414243",
            "ffentrymcr",
            "datafield",
        ] {
            assert!(
                !text.contains(forbidden),
                "forbidden form-text metadata leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_form_text_without_default_is_blank_in_word_compatible_output() {
        let output = parse_rtf(
            r"{\rtf1 Before {\field{\*\fldinst FORMTEXT}{\formfield{\fftype0}}} After\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before  After"));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "form field FORMTEXT is editor-only and omitted from passive print output",
            )
        }));
    }

    #[test]
    fn revision_insert_metadata_preserves_visible_text_without_controls() {
        let output = parse_rtf(
            r"{\rtf1 Before {\revised\revauth1\revdttm123456789\insrsid42 Inserted text} {\deleted Deleted text} after\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before Inserted text  after"));
        assert!(!text.contains("Deleted text"));
        assert!(!text.contains("revised"));
        assert!(!text.contains("revauth"));
        assert!(!text.contains("revdttm"));
        assert!(!text.contains("insrsid"));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_outline_text_controls() {
        let output = parse_rtf(r"{\rtf1{\outl outline} {\outl0 plain}\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let outline = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "outline")
            .expect("outline run");
        let plain = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "plain")
            .expect("plain run");

        assert!(outline.style.outline);
        assert!(!plain.style.outline);
    }

    #[test]
    fn normalizes_shadow_text_controls() {
        let output = parse_rtf(r"{\rtf1{\shad shadow} {\shad0 plain}\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let shadow = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "shadow")
            .expect("shadow run");
        let plain = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "plain")
            .expect("plain run");

        assert!(shadow.style.shadow);
        assert!(!plain.style.shadow);
    }

    #[test]
    fn normalizes_emboss_and_engrave_text_controls() {
        let output =
            parse_rtf(r"{\rtf1{\embo emboss} {\impr engrave} {\impr0 plain}\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let style_for = |text: &str| {
            paragraph
                .runs
                .iter()
                .find(|run| run.text.trim() == text)
                .map(|run| &run.style)
                .unwrap_or_else(|| panic!("missing run {text}"))
        };

        assert_eq!(style_for("emboss").relief, TextRelief::Emboss);
        assert_eq!(style_for("engrave").relief, TextRelief::Engrave);
        assert_eq!(style_for("plain").relief, TextRelief::None);
    }

    #[test]
    fn clamps_extreme_font_size_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_font_size_half_points: 96,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\fs0 tiny \fs99999 huge\par}", &options).unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let tiny = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "tiny")
            .expect("tiny run");
        let huge = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "huge")
            .expect("huge run");

        assert_eq!(tiny.style.font_size_half_points, 2);
        assert_eq!(huge.style.font_size_half_points, 96);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("font size clamped"))
        );
    }

    #[test]
    fn normalizes_character_fit_text_as_safe_layout_metadata() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_page_dimension_twips: 720,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\fittext360 fitted \fittext0 plain \fittext9999 clamped\par}",
            &options,
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let style_for = |text: &str| {
            paragraph
                .runs
                .iter()
                .find(|run| run.text.trim() == text)
                .map(|run| &run.style)
                .unwrap_or_else(|| panic!("missing run {text}"))
        };

        assert_eq!(style_for("fitted").fit_text_width_twips, Some(360));
        assert_eq!(style_for("plain").fit_text_width_twips, None);
        assert_eq!(style_for("clamped").fit_text_width_twips, Some(720));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("character fit-text rendered as bounded passive horizontal scaling")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("character fit-text width clamped")
        }));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn default_section_text_grid_clears_passive_line_grid() {
        let output = parse_rtf(r"{\rtf1\sectd\sectlinegrid480\sectdefaultcl Body\par}").unwrap();

        assert_eq!(output.document.page.text_line_grid_twips, None);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("section default text grid cleared bounded passive paragraph line pitch")
        }));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn paragraph_frame_text_direction_controls_remain_passive_metadata() {
        let output = parse_rtf(include_str!(
            "../../../fixtures/paragraph-frame-text-direction-passive.rtf"
        ))
        .expect("paragraph-frame text-direction fixture should parse");
        let directions = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph) if paragraph.style.frame_width_twips.is_some() => {
                    Some(paragraph.style.frame_text_direction)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            directions,
            vec![
                TableCellTextDirection::LeftToRightTopToBottom,
                TableCellTextDirection::LeftToRightTopToBottom,
                TableCellTextDirection::TopToBottomRightToLeft,
                TableCellTextDirection::BottomToTopLeftToRight,
                TableCellTextDirection::LeftToRightTopToBottom,
                TableCellTextDirection::TopToBottomRightToLeft,
            ]
        );
        for control in [
            "frmtxlrtb",
            "frmtxtbrl",
            "frmtxbtlr",
            "frmtxlrtbv",
            "frmtxtbrlv",
        ] {
            assert!(output.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(&format!(
                    "paragraph frame text direction \\{control} interpreted through bounded passive rotation"
                ))
            }));
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn word_compatible_mode_retains_bounded_horizontal_frame_text_distance() {
        let output = parse_rtf(include_str!(
            "../../../fixtures/framed-drop-cap-wrap-distance-passive.rtf"
        ))
        .unwrap();
        let paragraphs = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph) => Some(paragraph),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(paragraphs[0].style.frame_text_distance_twips, 0);
        assert_eq!(paragraphs[2].style.frame_text_distance_twips, 180);
        assert_eq!(paragraphs[4].style.frame_text_distance_twips, 240);
        assert_eq!(paragraphs[6].style.frame_text_distance_twips, 0);
        assert_eq!(paragraphs[0].style.frame_vertical_text_distance_twips, 0);
        assert_eq!(paragraphs[2].style.frame_vertical_text_distance_twips, 180);
        assert_eq!(paragraphs[4].style.frame_vertical_text_distance_twips, 0);
        assert_eq!(paragraphs[6].style.frame_vertical_text_distance_twips, 360);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("paragraph wrap distance interpreted through bounded passive flow layout")
        }));
    }

    #[test]
    fn paragraph_frame_text_distance_clamps_malformed_values() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_shape_dimension_twips: 480,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\dfrmtxtx-120\absw720\dropcapli3\dropcapt1 N\par\pard\dxfrtext9999\absw720\dropcapli3\dropcapt1 X\par}",
            &options,
        )
        .unwrap();
        let paragraphs = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph) => Some(paragraph),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(paragraphs[0].style.frame_text_distance_twips, 0);
        assert_eq!(paragraphs[1].style.frame_text_distance_twips, 480);
        assert_eq!(paragraphs[0].style.frame_vertical_text_distance_twips, 0);
        assert_eq!(paragraphs[1].style.frame_vertical_text_distance_twips, 480);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("paragraph frame text distance clamped from -120 to 0 twips")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("paragraph frame text distance clamped from 9999 to 480 twips")
        }));
    }

    #[test]
    fn ordinary_frame_text_distance_controls_retain_independent_bounded_axes() {
        let output = parse_rtf(include_str!(
            "../../../fixtures/paragraph-frame-text-distance-passive.rtf"
        ))
        .unwrap();
        let frames = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph) if paragraph.style.frame_width_twips.is_some() => {
                    Some((
                        paragraph.style.frame_text_distance_twips,
                        paragraph.style.frame_vertical_text_distance_twips,
                    ))
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            frames,
            vec![(0, 0), (180, 180), (240, 0), (0, 360), (240, 360), (0, 0)]
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn non_hex_picture_data_becomes_passive_geometry_placeholder() {
        let output =
            parse_rtf(r"{\rtf1 before {\pict\pngblip\picw100\pich50\picwgoal720\pichgoal360 4142XX4344} after\par}")
                .unwrap();
        let text = document_text(&output.document);
        let image = output
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Image(image) => Some(image),
                _ => None,
            })
            .or_else(|| output.document.inline_images.first())
            .expect("passive geometry placeholder");

        assert!(text.contains("before"));
        assert!(text.contains("after"));
        assert!(!text.contains("XX4344"));
        assert_eq!(image.format, ImageFormat::Placeholder);
        assert!(image.bytes.is_empty());
        assert_eq!(image.natural_width_px_hint, Some(100));
        assert_eq!(image.natural_height_px_hint, Some(50));
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(360));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("non-hex picture data replaced with bounded passive geometry placeholder")
        }));
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("non-hex picture data ignored and picture replaced with placeholder")
        }));
    }

    #[test]
    fn odd_trailing_picture_hex_nibble_becomes_passive_geometry_placeholder() {
        let output = parse_rtf(
            r"{\rtf1 before {\pict\jpegblip\picw100\pich50\picwgoal720\pichgoal360 f} after\par}",
        )
        .unwrap();
        let text = document_text(&output.document);
        let image = output
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Image(image) => Some(image),
                _ => None,
            })
            .or_else(|| output.document.inline_images.first())
            .expect("passive geometry placeholder");

        assert!(text.contains("before"));
        assert!(text.contains("after"));
        assert_eq!(image.format, ImageFormat::Placeholder);
        assert!(image.bytes.is_empty());
        assert_eq!(image.natural_width_px_hint, Some(100));
        assert_eq!(image.natural_height_px_hint, Some(50));
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(360));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "malformed picture hex data replaced with bounded passive geometry placeholder",
            )
        }));
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("picture data had an odd trailing hex nibble and was skipped")
        }));
    }

    #[test]
    fn hex_picture_byte_limit_is_enforced_before_decoding() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_image_bytes: 2,
                max_binary_blob_size: 1024,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        assert!(matches!(
            parse_rtf_bytes_with_options(br"{\rtf1{\pict\pngblip 000102}}", &options),
            Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "picture bytes"
        ));
    }
