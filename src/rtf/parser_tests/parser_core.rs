    #[test]
    fn word_compatibility_retains_explicit_empty_paragraphs() {
        let output = parse_rtf(r"{\rtf1\ansi A\par\par B\par}").unwrap();
        assert_eq!(output.document.blocks.len(), 3);
        let Block::Paragraph(empty) = &output.document.blocks[1] else {
            panic!("expected explicit empty paragraph");
        };
        assert_eq!(empty.runs.len(), 1);
        assert!(empty.runs[0].text.is_empty());

        let strict = parse_rtf_strict(r"{\rtf1\ansi A\par\par B\par}").unwrap();
        assert_eq!(strict.document.blocks.len(), 2);
    }

    #[test]
    fn strips_soft_line_and_page_break_layout_artifacts() {
        let output = parse_rtf(
            r"{\rtf1 Before \softline soft line \softcol soft column \softpage soft page\par}",
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

        assert_eq!(text, "Before soft line soft column soft page");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
        assert!(!output.document.blocks.iter().any(|block| {
            matches!(
                block,
                Block::PageBreak | Block::ColumnBreak | Block::SectionBreak
            )
        }));
    }

    #[test]
    fn normalizes_word_grid_and_outline_metadata_without_visible_output() {
        let output = parse_rtf(
            r"{\rtf1{\stylesheet{\s1\outlinelevel2\cgrid Heading;}}\pard\outlinelevel1\cgrid Visible\par}",
        )
        .unwrap();

        assert_eq!(document_text(&output.document), "Visible");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
        for forbidden in ["outlinelevel", "cgrid", "stylesheet"] {
            assert!(!document_text(&output.document).contains(forbidden));
        }
    }

    #[test]
    fn normalizes_escaped_special_visible_characters() {
        let output = parse_rtf(r"{\rtf1 non\~break optional\-hyphen non\_hyphen\par}").unwrap();
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
            "non\u{00a0}break optional\u{00ad}hyphen non\u{2011}hyphen"
        );
    }

    #[test]
    fn escaped_special_visible_characters_obey_hidden_text() {
        let output = parse_rtf(r"{\rtf1 Visible {\v \~ \- \_ \\ \{ \}}{\v0 shown}\par}").unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Visible"));
        assert!(text.contains("shown"));
        assert!(!text.contains('\u{00a0}'));
        assert!(!text.contains('\u{00ad}'));
        assert!(!text.contains('\u{2011}'));
        assert!(!text.contains('\\'));
        assert!(!text.contains('{'));
        assert!(!text.contains('}'));
    }

    #[test]
    fn normalizes_zero_width_formatting_controls() {
        let output =
            parse_rtf(r"{\rtf1 A\zwbo B\zwnbo C\zwnj D\zwj E\ltrmark L\rtlmark R\par}").unwrap();
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
            "A\u{200b}B\u{feff}C\u{200c}D\u{200d}E\u{200e}L\u{200f}R"
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_headers_and_footers_as_safe_paragraphs() {
        let output =
            parse_rtf(r"{\rtf1{\header Header text\par}{\footer Footer text\par}Body text\par}")
                .unwrap();

        assert_eq!(output.document.header.len(), 1);
        assert_eq!(output.document.header[0].runs[0].text, "Header text");
        assert_eq!(output.document.footer.len(), 1);
        assert_eq!(output.document.footer[0].runs[0].text, "Footer text");

        let body = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected body paragraph"),
        };
        assert_eq!(body.runs[0].text, "Body text");
    }

    #[test]
    fn normalizes_header_and_footer_variants_as_safe_paragraphs() {
        let output = parse_rtf(
            r"{\rtf1\titlepg{\headerf First header\par}{\headerl Even header\par}{\headerr Odd header\par}{\footerf First footer\par}{\footerl Even footer\par}{\footerr Odd footer\par}Body\page More\page Last\par}",
        )
        .unwrap();

        assert!(output.document.page.title_page);
        assert_eq!(output.document.first_page_header.len(), 1);
        assert_eq!(
            output.document.first_page_header[0].runs[0].text,
            "First header"
        );
        assert_eq!(output.document.even_page_header.len(), 1);
        assert_eq!(
            output.document.even_page_header[0].runs[0].text,
            "Even header"
        );
        assert_eq!(output.document.header.len(), 1);
        assert_eq!(output.document.header[0].runs[0].text, "Odd header");
        assert_eq!(output.document.first_page_footer.len(), 1);
        assert_eq!(
            output.document.first_page_footer[0].runs[0].text,
            "First footer"
        );
        assert_eq!(output.document.even_page_footer.len(), 1);
        assert_eq!(
            output.document.even_page_footer[0].runs[0].text,
            "Even footer"
        );
        assert_eq!(output.document.footer.len(), 1);
        assert_eq!(output.document.footer[0].runs[0].text, "Odd footer");
    }

    #[test]
    fn section_defaults_preserve_document_page_geometry() {
        let output = parse_rtf(
            r"{\rtf1\paperw6120\paperh7920\margl720\margr720\margt720\margb720\sectd Body\par\sect\sectd More\par}",
        )
        .unwrap();

        assert_eq!(output.document.page.width_twips, 6120);
        assert_eq!(output.document.page.height_twips, 7920);
        let settings = output
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::SectionSettings(settings) => Some(settings),
                _ => None,
            })
            .expect("expected later section settings");
        assert_eq!(settings.width_twips, 6120);
        assert_eq!(settings.height_twips, 7920);
        assert_eq!(settings.margin_left_twips, 720);
        assert_eq!(settings.margin_top_twips, 720);
    }

    #[test]
    fn normalizes_later_section_title_page_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1 First\par\sect\sectd\titlepg Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert!(settings.title_page);
    }

    #[test]
    fn normalizes_later_section_headers_and_footers_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1{\header Document header\par}{\footer Document footer\par}First\par\sect\sectd{\header Section header\par}{\footer Section footer\par}Second\par}",
        )
        .unwrap();

        assert_eq!(output.document.header[0].runs[0].text, "Document header");
        assert_eq!(output.document.footer[0].runs[0].text, "Document footer");
        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert_eq!(settings.header[0].runs[0].text, "Section header");
        assert_eq!(settings.footer[0].runs[0].text, "Section footer");
    }

    #[test]
    fn normalizes_page_number_control_as_typed_dynamic_field() {
        let output = parse_rtf(r"{\rtf1{\header \chpgn\par}Body text\page More text\par}").unwrap();

        assert_eq!(output.document.header.len(), 1);
        let text = output.document.header[0]
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();
        assert_eq!(text.replace(PASSIVE_ADVANCE_MARKER, ""), "");
        assert_eq!(text.matches(PASSIVE_ADVANCE_MARKER).count(), 1);
        assert_eq!(
            paragraph_dynamic_fields(&output.document.header[0]),
            vec![DynamicField::PageNumber]
        );
    }

    #[test]
    fn normalizes_page_number_start_control_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1\pgnstarts7{\header Page \chpgn\par}Body\par}").unwrap();

        assert_eq!(output.document.page.page_number_start, Some(7));
        assert!(!output.document.page.restart_page_numbering);
        assert_eq!(
            output.document.header[0]
                .runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>()
                .replace(PASSIVE_ADVANCE_MARKER, ""),
            "Page "
        );
        assert_eq!(
            paragraph_dynamic_fields(&output.document.header[0]),
            vec![DynamicField::PageNumber]
        );
    }

    #[test]
    fn normalizes_page_number_format_controls_as_safe_metadata() {
        for (control, expected) in [
            ("pgndec", PageNumberFormat::Decimal),
            ("pgnucrm", PageNumberFormat::UpperRoman),
            ("pgnlcrm", PageNumberFormat::LowerRoman),
            ("pgnucltr", PageNumberFormat::UpperLetter),
            ("pgnlcltr", PageNumberFormat::LowerLetter),
        ] {
            let input = format!(r"{{\rtf1\{control}{{\header Page \chpgn\par}}Body\par}}");
            let output = parse_rtf(&input).unwrap();

            assert_eq!(output.document.page.page_number_format, Some(expected));
            assert_eq!(
                output.document.header[0]
                    .runs
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<String>()
                    .replace(PASSIVE_ADVANCE_MARKER, ""),
                "Page "
            );
            assert_eq!(
                paragraph_dynamic_fields(&output.document.header[0]),
                vec![DynamicField::PageNumber]
            );
        }
    }

    #[test]
    fn normalizes_page_number_position_controls_as_safe_metadata() {
        let output =
            parse_rtf(r"{\rtf1\pgnx360\pgny1440{\header Page \chpgn\par}Body\par}").unwrap();

        assert_eq!(output.document.page.page_number_x_twips, Some(360));
        assert_eq!(output.document.page.page_number_y_twips, Some(1_440));
        assert_eq!(
            output.document.header[0]
                .runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>()
                .replace(PASSIVE_ADVANCE_MARKER, ""),
            "Page "
        );
        assert_eq!(
            paragraph_dynamic_fields(&output.document.header[0]),
            vec![DynamicField::PageNumber]
        );
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("page number position approximated")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "page number position rendered as bounded passive header/footer coordinates",
            )
        }));
    }

    #[test]
    fn clamps_page_number_position_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_page_dimension_twips: 1_440,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\pgnx-20\pgny9999 Body\par}", &options).unwrap();

        assert_eq!(output.document.page.page_number_x_twips, Some(0));
        assert_eq!(output.document.page.page_number_y_twips, Some(1_440));
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("page number y clamped"))
        );
    }

    #[test]
    fn normalizes_later_section_page_number_start_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1 First\par\sect\sectd\pgnstarts3 Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert_eq!(settings.page_number_start, Some(3));
        assert!(!settings.restart_page_numbering);
    }

    #[test]
    fn normalizes_later_section_page_number_restart_and_continue_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1 First\par\sect\sectd\pgnrestart Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert_eq!(settings.page_number_start, None);
        assert!(settings.restart_page_numbering);

        let output =
            parse_rtf(r"{\rtf1\pgnstarts7 First\par\sect\sectd\pgncont Second\par}").unwrap();
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert_eq!(settings.page_number_start, None);
        assert!(!settings.restart_page_numbering);

        let output = parse_rtf(r"{\rtf1 First\par\sect\sectd\pgnrestart0 Second\par}").unwrap();
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert_eq!(settings.page_number_start, None);
        assert!(!settings.restart_page_numbering);

        let output =
            parse_rtf(r"{\rtf1 First\par\sect\sectd\pgnstarts3\pgnrestart Second\par}").unwrap();
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert_eq!(settings.page_number_start, Some(3));
        assert!(settings.restart_page_numbering);
    }

    #[test]
    fn normalizes_later_section_page_number_format_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1 First\par\sect\sectd\pgnlcltr Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert_eq!(
            settings.page_number_format,
            Some(PageNumberFormat::LowerLetter)
        );
    }

    #[test]
    fn clamps_page_number_start_control() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_page_number_start: 9,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\pgnstart999 Body\par}", &options).unwrap();

        assert_eq!(output.document.page.page_number_start, Some(9));
        assert!(output.document.page.restart_page_numbering);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("page number start clamped"))
        );
    }

    #[test]
    fn normalizes_footnotes_as_safe_paragraphs() {
        let output =
            parse_rtf(r"{\rtf1 Body\chftn{\footnote \chftn Footnote text\par}\par}").unwrap();
        let body = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected body paragraph"),
        };

        assert_eq!(
            body.runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            format!("Body{}", footnote_reference_marker(FootnoteId::new(0)))
        );
        let reference = body
            .runs
            .iter()
            .find(|run| run.text == footnote_reference_marker(FootnoteId::new(0)))
            .expect("footnote reference run");
        assert_eq!(
            reference.style.baseline_shift_half_points,
            DEFAULT_SUPERSCRIPT_SHIFT_HALF_POINTS
        );
        assert_eq!(
            reference.style.font_size_scale_percent,
            DEFAULT_SCRIPT_FONT_SCALE_PERCENT
        );
        assert_eq!(output.document.footnotes.len(), 1);
        assert_eq!(output.document.footnotes[0].runs[0].text, "Footnote text");
    }

    #[test]
    fn normalizes_endnotes_as_safe_paragraphs() {
        let output =
            parse_rtf(r"{\rtf1 Body\chftn{\endnote \chftn Endnote text\par}\par}").unwrap();
        let body = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected body paragraph"),
        };

        assert_eq!(
            body.runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            "Body1"
        );
        let reference = body
            .runs
            .iter()
            .find(|run| run.text == "1")
            .expect("endnote reference run");
        assert_eq!(
            reference.style.baseline_shift_half_points,
            DEFAULT_SUPERSCRIPT_SHIFT_HALF_POINTS
        );
        assert_eq!(output.document.endnotes.len(), 1);
        assert_eq!(output.document.endnotes[0].runs[0].text, "Endnote text");
    }

    #[test]
    fn fet1_normalizes_legacy_footnote_groups_as_endnotes() {
        let output =
            parse_rtf(r"{\rtf1\fet1 Body\chftn{\footnote \chftn Legacy endnote\par}\par}").unwrap();
        let body = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected body paragraph"),
        };

        assert_eq!(
            body.runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            "Bodyi"
        );
        assert!(output.document.footnotes.is_empty());
        assert_eq!(output.document.endnotes.len(), 1);
        assert_eq!(output.document.endnotes[0].section_index.get(), 1);
        assert_eq!(output.document.endnotes[0].runs[0].text, "Legacy endnote");
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("passive endnote-only interpretation")
        }));
    }

    #[test]
    fn footnote_and_endnote_references_use_separate_passive_numbering() {
        let output = parse_rtf(
            r"{\rtf1 Foot\chftn{\footnote \chftn Footnote text\par} End\chftn{\endnote \chftn Endnote text\par}\par}",
        )
        .unwrap();
        let body = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected body paragraph"),
        };

        assert_eq!(
            body.runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            format!("Foot{} End1", footnote_reference_marker(FootnoteId::new(0)))
        );
        assert_eq!(output.document.footnotes[0].runs[0].text, "Footnote text");
        assert_eq!(output.document.endnotes[0].runs[0].text, "Endnote text");
    }

    #[test]
    fn normalizes_note_numbering_controls_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1\fet2\ftnstart4\ftnnruc\aftnstart2\aftnnalc Body\chftn{\footnote \chftn Footnote text\par} End\chftn{\footnote\ftnalt \chftn Endnote text\par}\par}",
        )
        .unwrap();
        let body = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected body paragraph"),
        };

        assert_eq!(output.document.footnote_number_start, 4);
        assert_eq!(
            output.document.footnote_number_format,
            PageNumberFormat::UpperRoman
        );
        assert_eq!(output.document.endnote_number_start, 2);
        assert_eq!(
            output.document.endnote_number_format,
            PageNumberFormat::LowerLetter
        );
        assert_eq!(
            body.runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            format!("Body{} Endi", footnote_reference_marker(FootnoteId::new(0)))
        );
        assert_eq!(output.document.footnotes[0].runs[0].text, "Footnote text");
        assert_eq!(output.document.endnotes[0].runs[0].text, "Endnote text");
        assert_eq!(
            output.document.footnotes[0].numbering.expect("footnote numbering"),
            NoteNumbering {
                start: 1,
                format: PageNumberFormat::Decimal,
                restart: NoteNumberRestart::Continuous,
                word_compact_label: true,
            }
        );
        assert_eq!(
            output.document.endnotes[0].numbering.expect("endnote numbering"),
            NoteNumbering {
                start: 1,
                format: PageNumberFormat::LowerRoman,
                restart: NoteNumberRestart::Continuous,
                word_compact_label: true,
            }
        );
    }

    #[test]
    fn section_note_numbering_controls_override_word_section_defaults() {
        let output = parse_rtf(
            r"{\rtf1\fet2\ftnstart1\ftnnar\aftnstart1\aftnnrlc\sftnstart4\sftnnruc\saftnstart2\saftnnalc Body\chftn{\footnote \chftn Footnote text\par} End\chftn{\footnote\ftnalt \chftn Endnote text\par}\par}",
        )
        .unwrap();
        let body = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected body paragraph"),
        };

        assert_eq!(
            body.runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            format!("Body{} Endb", footnote_reference_marker(FootnoteId::new(0)))
        );
        assert_eq!(output.document.footnotes[0].numbering.expect("footnote numbering").start, 4);
        assert_eq!(
            output.document.footnotes[0].numbering.expect("footnote numbering").format,
            PageNumberFormat::UpperRoman
        );
        assert_eq!(output.document.endnotes[0].numbering.expect("endnote numbering").start, 2);
        assert_eq!(
            output.document.endnotes[0].numbering.expect("endnote numbering").format,
            PageNumberFormat::LowerLetter
        );
    }

    #[test]
    fn standard_ftnalt_endnote_does_not_create_a_ghost_footnote() {
        let output = parse_rtf(
            r"{\rtf1\fet2\ftnstart4\ftnnruc\aftnstart2\aftnnalc Body\chftn{\footnote\ftnalt \chftn Endnote text\par}\par}",
        )
        .unwrap();
        let body = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected body paragraph"),
        };

        assert!(output.document.footnotes.is_empty());
        assert_eq!(output.document.endnotes.len(), 1);
        assert_eq!(output.document.endnotes[0].runs[0].text, "Endnote text");
        assert_eq!(
            body.runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            "Bodyi"
        );
    }

    #[test]
    fn standard_ftnalt_does_not_advance_an_endnote_only_document_twice() {
        let output = parse_rtf(
            r"{\rtf1\fet1\aftnstart2\aftnnalc Body\chftn{\footnote\ftnalt \chftn Endnote text\par}\par}",
        )
        .unwrap();
        let body = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected body paragraph"),
        };

        assert!(output.document.footnotes.is_empty());
        assert_eq!(output.document.endnotes.len(), 1);
        assert_eq!(
            body.runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            "Bodyi"
        );
    }

    #[test]
    fn normalizes_note_placement_controls_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1\ftntj\aenddoc Body\chftn{\footnote Footnote text\par} End\chftn{\endnote Endnote text\par}\par}",
        )
        .unwrap();

        assert_eq!(
            output.document.footnote_placement,
            FootnotePlacement::BeneathText
        );
        assert_eq!(
            output.document.endnote_placement,
            EndnotePlacement::EndOfDocument
        );
        assert_eq!(
            output.document.endnotes.iter().map(|note| note.placement).collect::<Vec<_>>(),
            vec![EndnotePlacement::EndOfDocument]
        );
        assert!(
            document_text(&output.document)
                .contains(&format!("Body{} End1", footnote_reference_marker(FootnoteId::new(0))))
        );
        assert!(!document_text(&output.document).contains("aenddoc"));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("endnotes placed on passive final page")
        }));
    }

    #[test]
    fn note_option_controls_without_notes_do_not_emit_rendering_approximation_diagnostics() {
        let output = parse_rtf(r"{\rtf1\ftnbj\aenddoc\endnhere\fet1 Body only\par}").unwrap();

        assert_eq!(
            output.document.footnote_placement,
            FootnotePlacement::BottomOfPage
        );
        assert_eq!(
            output.document.endnote_placement,
            EndnotePlacement::EndOfSection
        );
        assert!(output.document.footnotes.is_empty());
        assert!(output.document.endnotes.is_empty());
        assert!(
            output.diagnostics.iter().all(|diagnostic| {
                !diagnostic.message.contains("footnotes placed")
                    && !diagnostic.message.contains("endnotes placed")
                    && !diagnostic.message.contains("note type set")
            }),
            "inert note options should not warn as rendered approximations: {:?}",
            output.diagnostics
        );
    }

    #[test]
    fn normalizes_bottom_footnote_placement_control_as_safe_metadata() {
        let output =
            parse_rtf(r"{\rtf1\ftnbj Body\chftn{\footnote Bottom footnote text\par}\par}").unwrap();

        assert_eq!(
            output.document.footnote_placement,
            FootnotePlacement::BottomOfPage
        );
        assert!(
            document_text(&output.document)
                .contains(&format!("Body{}", footnote_reference_marker(FootnoteId::new(0))))
        );
        assert!(!document_text(&output.document).contains("ftnbj"));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("footnotes placed at passive page bottom")
        }));
    }

    #[test]
    fn endnotes_capture_effective_passive_placement_per_note() {
        let output = parse_rtf(
            r"{\rtf1\aenddoc First\chftn{\endnote Final note\par}\sect\sectd\endnhere Second\chftn{\endnote Section note\par}\par}",
        )
        .unwrap();

        assert_eq!(output.document.endnotes.len(), 2);
        assert_eq!(
            output.document.endnotes.iter().map(|note| note.section_index.get()).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(
            output.document.endnotes.iter().map(|note| note.placement).collect::<Vec<_>>(),
            vec![
                EndnotePlacement::EndOfDocument,
                EndnotePlacement::EndOfSection
            ]
        );
        assert!(document_text(&output.document).contains("First1"));
        assert!(document_text(&output.document).contains("Second2"));
        for forbidden in ["aenddoc", "endnhere", "chftn"] {
            assert!(
                !document_text(&output.document).contains(forbidden),
                "endnote placement control leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn note_separator_destinations_do_not_become_body_text() {
        let output = parse_rtf(
            r"{\rtf1{\ftnsep Safe footnote separator {\object\objdata 414243}\par}{\ftnsepc Safe footnote continuation}{\aftnsep Safe endnote separator}{\aftnsepc Safe endnote continuation}Body\chftn{\footnote \chftn Footnote text\par} End\chftn{\endnote \chftn Endnote text\par}\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains(&format!("Body{} End1", footnote_reference_marker(FootnoteId::new(0)))),
            "text: {text:?}"
        );
        assert_eq!(output.document.footnotes[0].runs[0].text, "Footnote text");
        assert_eq!(output.document.endnotes[0].runs[0].text, "Endnote text");
        assert_eq!(
            output
                .document
                .footnote_separator
                .as_ref()
                .and_then(paragraph_plain_text)
                .as_deref(),
            Some("Safe footnote separator")
        );
        assert_eq!(
            output
                .document
                .footnote_continuation_separator
                .as_ref()
                .and_then(paragraph_plain_text)
                .as_deref(),
            Some("Safe footnote continuation")
        );
        assert_eq!(
            output
                .document
                .endnote_separator
                .as_ref()
                .and_then(paragraph_plain_text)
                .as_deref(),
            Some("Safe endnote separator")
        );
        assert_eq!(
            output
                .document
                .endnote_continuation_separator
                .as_ref()
                .and_then(paragraph_plain_text)
                .as_deref(),
            Some("Safe endnote continuation")
        );
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
            "414243",
        ] {
            assert!(
                !text.contains(forbidden),
                "note separator definition leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_layout_advance_zero_offsets_normalize_without_stale_strip_warning() {
        let output =
            parse_rtf(r"{\rtf1 Before {\field{\*\fldinst ADVANCE \\r 0 \\d 0}} after\par}")
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert!(
            paragraph
                .runs
                .iter()
                .all(|run| run.text != PASSIVE_ADVANCE_MARKER)
        );
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("layout field ADVANCE normalized with no visible passive cursor movement")
        }));
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("ADVANCE stripped without applying cursor positioning")
        }));
    }

    #[test]
    fn resultless_fillin_unsafe_default_renders_prompt_placeholder() {
        let output =
            parse_rtf(r#"{\rtf1 Fill {\field{\*\fldinst FILLIN "Prompt" \d "/JavaScript"}}\par}"#)
                .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Fill [Field removed: no passive result]"),
            "text was {text:?}"
        );
        for forbidden in ["FILLIN", "Prompt", "/JavaScript", "fldinst"] {
            assert!(
                !text.contains(forbidden),
                "unsafe FILLIN default leaked prompt text: {forbidden}"
            );
        }
    }

    #[test]
    fn edit_time_metadata_obeys_output_bounds() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_output_text_chars: 3,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        assert!(matches!(
            parse_rtf_bytes_with_options(br"{\rtf1{\info{\edmins42}}\par}", &options),
            Err(ParseError::ResourceLimitExceeded { resource, .. })
                if resource == "document edit minutes"
        ));
    }

    #[test]
    fn revision_number_metadata_obeys_output_bounds() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_output_text_chars: 3,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        assert!(matches!(
            parse_rtf_bytes_with_options(br"{\rtf1{\info{\version42}}\par}", &options),
            Err(ParseError::ResourceLimitExceeded { resource, .. })
                if resource == "document revision number"
        ));
    }

    #[test]
    fn dynamic_date_time_controls_follow_policy_without_evaluating() {
        let input = br"{\rtf1 Before \chdate / \chtime / \chdpa / \chdpl After\par}";
        let output = parse_rtf_bytes_with_options(input, &RtfParseOptions::default()).unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before"));
        assert!(text.contains("[Dynamic date/time removed]"));
        assert!(text.contains("After"));
        assert!(!text.contains("chdate"));
        assert!(!text.contains("chtime"));
        assert!(!text.contains("chdpa"));
        assert!(!text.contains("chdpl"));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "dynamic date/time control chdate rendered as passive placeholder without evaluating current time",
            )
        }));
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("dynamic date/time control chdate placeholdered")
        }));

        let strip_options = RtfParseOptions {
            active_content_policy: ActiveContentPolicy::Strip,
            ..RtfParseOptions::default()
        };
        let stripped = parse_rtf_bytes_with_options(input, &strip_options).unwrap();
        let stripped_text = document_text(&stripped.document);
        assert!(stripped_text.contains("Before"));
        assert!(stripped_text.contains("After"));
        assert!(!stripped_text.contains("[Dynamic date/time removed]"));
        assert!(stripped.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("dynamic date/time control chdate removed")
        }));

        let reject_options = RtfParseOptions {
            active_content_policy: ActiveContentPolicy::Reject,
            ..RtfParseOptions::default()
        };
        assert!(matches!(
            parse_rtf_bytes_with_options(input, &reject_options),
            Err(ParseError::ActiveContentRejected { feature, .. })
                if feature == "dynamic date/time control"
        ));
    }

    #[test]
    fn section_number_controls_create_typed_dynamic_fields() {
        let output =
            parse_rtf(r"{\rtf1 Section \sectnum\par\sbknone\sect Section \sectnum\par}").unwrap();
        let text = document_text(&output.document);

        assert_eq!(
            document_dynamic_fields(&output.document),
            vec![DynamicField::SectionNumber, DynamicField::SectionNumber]
        );
        assert!(!text.contains("sectnum"));
        assert!(
            output
                .document
                .blocks
                .iter()
                .any(|block| matches!(block, Block::SectionBreak))
        );
    }

    #[test]
    fn unknown_destination_groups_do_not_become_body_text() {
        let output =
            parse_rtf(r"{\rtf1 Visible {\unknown Hidden {\object\objdata 414243}} after\par}")
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

        assert_eq!(text, "Visible  after");
        assert!(!text.contains("Hidden"));
        assert!(!text.contains("objdata"));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("unknown RTF destination '\\unknown' skipped")
        }));
    }

    #[test]
    fn unknown_destination_groups_consume_skip_budget() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_destination_bytes: 4,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        assert!(matches!(
            parse_rtf_bytes_with_options(br"{\rtf1{\unknown abcde} visible\par}", &options),
            Err(ParseError::DestinationTooLarge(_))
        ));
    }

    #[test]
    fn unsupported_inline_controls_do_not_swallow_visible_text() {
        let output = parse_rtf(r"{\rtf1 Visible \unknown inline text\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(text, "Visible inline text");
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("unsupported RTF control '\\unknown'")
        }));
    }

    #[test]
    fn visible_controls_in_non_visual_destinations_do_not_become_body_text() {
        let output = parse_rtf(
            r"{\rtf1{\fonttbl{\f0\tab Arial;}}{\colortbl;\tab\red1\green2\blue3;}{\pict\pngblip\tab 00}Visible\par}",
        )
        .unwrap();
        let paragraph_text = output
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
            .collect::<Vec<_>>();

        assert_eq!(paragraph_text, vec!["Visible"]);
    }

    #[test]
    fn metadata_destinations_do_not_become_body_text() {
        let output = parse_rtf(
            r"{\rtf1{\info{\title Hidden title}{\author Hidden author}{\doccomm Hidden comment}}{\template C:\remote\template.dotm}Visible\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Visible"));
        assert!(!text.contains("Hidden title"));
        assert!(!text.contains("Hidden author"));
        assert!(!text.contains("Hidden comment"));
        assert!(!text.contains("template.dotm"));
    }

    #[test]
    fn resultless_info_metadata_aliases_render_passive_values() {
        let output = parse_rtf(
            r##"{\rtf1{\info{\creatim\yr2024\mo7\dy5\hr14\min30}{\revtim\yr2025\mo1\dy2}{\version9}{\edmins17}\nofparas3\noflines12\nofbytes4096}Alpha beta Gamma info created {\field{\*\fldinst INFO "Creation Date" \\@ "yyyy-MM-dd"}} saved {\field{\*\fldinst INFO "Last Save Time"}} revision {\field{\*\fldinst INFO "Revision Number" \\* ROMAN}} edit {\field{\*\fldinst INFO "Total Editing Time" \\# "0000"}} words {\field{\*\fldinst INFO "Number of Words"}} paragraphs {\field{\*\fldinst INFO "Number of Paragraphs"}} lines {\field{\*\fldinst INFO "Number of Lines" \\# "000"}} bytes {\field{\*\fldinst INFO "Number of Bytes" \\# "#,##0"}} file {\field{\*\fldinst INFO Filename}}\par}"##,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("created 2024-07-05 saved 2025-01-02 00:00:00"));
        assert!(text.contains("revision IX edit 0017"));
        assert!(text.contains("paragraphs 3 lines 012"));
        assert!(text.contains("bytes 4,096"));
        assert_eq!(
            document_dynamic_fields(&output.document),
            vec![DynamicField::DocumentWords]
        );
        assert_eq!(
            text.matches("[Field removed: no passive result]").count(),
            1
        );
        for forbidden in [
            "INFO",
            "Creation Date",
            "Last Save Time",
            "Revision Number",
            "Total Editing Time",
            "Number of Words",
            "Number of Paragraphs",
            "Number of Lines",
            "Number of Bytes",
            "Filename",
            "creatim",
            "revtim",
            "version",
            "edmins",
            "nofparas",
            "noflines",
            "nofbytes",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "INFO metadata alias leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn strict_spec_resultless_filesize_renders_passive_metadata_byte_count() {
        let options = RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br##"{\rtf1{\info\nofbytes4096}Size {\field{\*\fldinst FILESIZE \\# "#,##0"}} missing {\field{\*\fldinst FILENAME \p}}\par}"##,
            &options,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Size 4,096 missing [Field removed: no passive result]"));
        for forbidden in ["FILESIZE", "FILENAME", "nofbytes", "fldinst"] {
            assert!(
                !text.contains(forbidden),
                "FILESIZE metadata field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn word_compatible_resultless_filesize_is_suppressed() {
        let output = parse_rtf(
            r##"{\rtf1{\info\nofbytes4096}Size {\field{\*\fldinst FILESIZE \\# "#,##0"}} after\par}"##,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert_eq!(text, "Size  after");
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("resultless FILESIZE suppressed for Word-compatible passive rendering")
        }));
    }

    #[test]
    fn document_variable_metadata_obeys_count_bounds() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_document_properties: 0,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        let result =
            parse_rtf_bytes_with_options(br"{\rtf1{\*\docvar {Key}{Value}}\par}", &options);
        assert!(
            matches!(
                result,
                Err(ParseError::ResourceLimitExceeded { ref resource, .. })
                    if resource == "document variables"
            ),
            "unexpected result: {result:?}"
        );
    }

    #[test]
    fn resultless_form_checkbox_is_blank_in_word_compatible_print_output() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\field{\*\fldinst FORMCHECKBOX}{\formfield{\fftype1}{\ffname HiddenName}{\ffdefres0}{\ffres1}{\ffentrymcr launch.exe}{\datafield 414243}}} After\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Before  After"),
            "normalized text was {text:?}; diagnostics were {:?}",
            output.diagnostics
        );
        assert!(!text.contains(['\u{2610}', '\u{2611}']));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("form checkbox is editor-only and omitted from passive print output")
        }));
        for forbidden in [
            "FORMCHECKBOX",
            "HiddenName",
            "launch.exe",
            "414243",
            "ffentrymcr",
            "datafield",
        ] {
            assert!(
                !text.contains(forbidden),
                "forbidden checkbox form-field metadata leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_form_checkbox_defaults_to_blank_word_compatible_output() {
        let output =
            parse_rtf(r"{\rtf1{\field{\*\fldinst FORMCHECKBOX}{\formfield{\fftype1}}}\par}")
                .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.is_empty(),
            "normalized text was {text:?}; diagnostics were {:?}",
            output.diagnostics
        );
    }

    #[test]
    fn strict_spec_form_dropdown_renders_selected_entry_without_metadata() {
        let options = RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br#"{\rtf1 Before {\field{\*\fldinst FORMDROPDOWN}{\formfield{\fftype2}{\ffname HiddenName}{\ffdefres0}{\ffres1}{\*\ffl First choice}{\*\ffl Second \u937? choice}{\ffentrymcr launch.exe}{\datafield 414243}}} After\par}"#,
            &options,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Before Second \u{03a9} choice After"),
            "normalized text was {text:?}; diagnostics were {:?}",
            output.diagnostics
        );
        for forbidden in [
            "FORMDROPDOWN",
            "HiddenName",
            "First choice",
            "launch.exe",
            "414243",
            "ffentrymcr",
            "datafield",
        ] {
            assert!(
                !text.contains(forbidden),
                "forbidden dropdown form-field metadata leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn document_protection_metadata_does_not_become_body_text() {
        let output = parse_rtf_strict(
            r"{\rtf1\formprot\revprot\annotprot{\passwordhash DEADBEEFCAFE0123456789ABCDEF0123}Visible protected body\par\passwordhash AABBCCDDEEFF Inline body\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Visible protected body"));
        assert!(text.contains("Inline body"));
        for forbidden in [
            "passwordhash",
            "DEADBEEF",
            "AABBCC",
            "formprot",
            "revprot",
            "annotprot",
        ] {
            assert!(
                !text.contains(forbidden),
                "forbidden document protection metadata leaked to text: {forbidden}"
            );
        }
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("document protection password hash stripped")
        }));
    }

    #[test]
    fn word_treats_ungrouped_password_hash_as_remainder_destination() {
        let output = parse_rtf(
            r"{\rtf1{\passwordhash DEADBEEF}Visible protected body\par\passwordhash AABBCC Inline body\par Visible after\par}",
        )
        .unwrap();

        assert_eq!(document_text(&output.document), "Visible protected body");
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("document protection password hash stripped")
        }));
    }

    #[test]
    fn review_and_bookmark_destinations_do_not_become_body_text() {
        let output = parse_rtf(
            r"{\rtf1 Before{\annotation Hidden comment {\object\objdata 414243}{\result Hidden result}} middle{\*\bkmkstart SecretBookmark} visible{\*\bkmkend SecretBookmark}{\deleted Deleted text} after\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before middle visible after"));
        assert!(!text.contains("Hidden comment"));
        assert!(!text.contains("Hidden result"));
        assert!(!text.contains("Embedded object removed"));
        assert!(!text.contains("SecretBookmark"));
        assert!(!text.contains("Deleted text"));
        assert!(!text.contains("414243"));

        let spaced = parse_rtf(r"{\rtf1 Before {\annotation Hidden comment} after\par}").unwrap();
        assert_eq!(document_text(&spaced.document), "Before after");
    }

    #[test]
    fn mail_merge_metadata_does_not_become_body_text() {
        let output = parse_rtf(
            r"{\rtf1 Before{\mailmerge{\mmconnectstr Provider=SQLOLEDB;Password=secret}{\mmdatasource C:\remote\contacts.mdb}{\mmquery SELECT * FROM Contacts}{\mmodsoudl http://example.com/source.udl}{\object\objdata 414243}} after\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before after"));
        for forbidden in [
            "Provider=SQLOLEDB",
            "Password=secret",
            "contacts.mdb",
            "SELECT *",
            "example.com",
            "Embedded object removed",
            "objdata",
            "414243",
        ] {
            assert!(
                !text.contains(forbidden),
                "mail merge metadata leaked to text: {forbidden}"
            );
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn strict_spec_recovers_legacy_polyline_without_declared_vertex_count() {
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1{\do\dppolyline\dpptx0\dppty0\dpptx10\dppty10\dpptx20\dppty0}}",
            &RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
        )
        .unwrap();

        assert!(output.document.blocks.iter().any(
            |block| matches!(block, Block::Shape(shape) if shape.kind == StaticShapeKind::Polyline)
        ));
    }

    #[test]
    fn normalizes_visible_character_positioning_controls() {
        let output = parse_rtf(
            r"{\rtf1{\strike struck} {\strikedl double} {\super sup} {\sub sub} {\nosupersub base} {\super0 superoff} {\sub0 suboff} {\up8 up} {\dn6 down}\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        let run_containing = |text: &str| {
            paragraph
                .runs
                .iter()
                .find(|run| run.text.contains(text))
                .unwrap_or_else(|| panic!("missing run containing {text}"))
        };

        let struck = run_containing("struck");
        assert!(struck.style.strike);
        assert!(!struck.style.double_strike);

        let double = run_containing("double");
        assert!(double.style.strike);
        assert!(double.style.double_strike);

        let sup = run_containing("sup");
        assert_eq!(
            sup.style.baseline_shift_half_points,
            DEFAULT_SUPERSCRIPT_SHIFT_HALF_POINTS
        );
        assert_eq!(
            sup.style.font_size_scale_percent,
            DEFAULT_SCRIPT_FONT_SCALE_PERCENT
        );

        let sub = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "sub")
            .expect("sub run");
        assert_eq!(
            sub.style.baseline_shift_half_points,
            DEFAULT_SUBSCRIPT_SHIFT_HALF_POINTS
        );
        assert_eq!(
            sub.style.font_size_scale_percent,
            DEFAULT_SCRIPT_FONT_SCALE_PERCENT
        );

        let base = run_containing("base");
        assert_eq!(base.style.baseline_shift_half_points, 0);
        assert_eq!(base.style.font_size_scale_percent, 100);

        let superoff = run_containing("superoff");
        assert_eq!(superoff.style.baseline_shift_half_points, 0);
        assert_eq!(superoff.style.font_size_scale_percent, 100);

        let suboff = run_containing("suboff");
        assert_eq!(suboff.style.baseline_shift_half_points, 0);
        assert_eq!(suboff.style.font_size_scale_percent, 100);

        let up = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "up")
            .expect("up run");
        assert_eq!(up.style.baseline_shift_half_points, 8);
        assert_eq!(up.style.font_size_scale_percent, 100);

        let down = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "down")
            .expect("down run");
        assert_eq!(down.style.baseline_shift_half_points, -6);
        assert_eq!(down.style.font_size_scale_percent, 100);
    }

    #[test]
    fn character_animation_controls_are_static_passive_approximations() {
        let output = parse_rtf(r"{\rtf1\animtext5 Animated \animtext0 Plain\par}").unwrap();

        assert_eq!(document_text(&output.document), "Animated Plain");
        assert!(
            output.diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("character animation stripped for passive static PDF output")),
            "missing character animation diagnostic: {:?}",
            output.diagnostics
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
            "animtext should not be reported as unsupported: {:?}",
            output.diagnostics
        );
    }

    #[test]
    fn normalizes_caps_controls() {
        let output =
            parse_rtf(r"{\rtf1{\caps Shout} {\caps0 quiet} {\scaps Small} {\scaps0 normal}\par}")
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        let shout = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Shout")
            .expect("caps run");
        assert!(shout.style.all_caps);

        let quiet = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "quiet")
            .expect("caps cleared run");
        assert!(!quiet.style.all_caps);

        let small = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Small")
            .expect("small caps run");
        assert!(small.style.small_caps);

        let normal = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "normal")
            .expect("small caps cleared run");
        assert!(!normal.style.small_caps);
    }

    #[test]
    fn normalizes_and_clamps_character_spacing_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_character_spacing_twips: 120,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\expnd6 expanded \expndtw-80 condensed \expndtw9999 clamped\par}",
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

        assert_eq!(style_for("expanded").character_spacing_twips, 30);
        assert_eq!(style_for("condensed").character_spacing_twips, -80);
        assert_eq!(style_for("clamped").character_spacing_twips, 120);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("character spacing clamped"))
        );
    }

    #[test]
    fn normalizes_and_clamps_character_kerning_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_font_size_half_points: 96,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\kerning2 kerned \kerning999 clamped \kerning0 plain\par}",
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

        assert_eq!(style_for("kerned").character_kerning_half_points, 2);
        assert_eq!(style_for("clamped").character_kerning_half_points, 96);
        assert_eq!(style_for("plain").character_kerning_half_points, 0);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("character kerning rendered as bounded passive pair spacing")
        }));
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("character kerning approximated")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("character kerning threshold clamped")
        }));
    }

    #[test]
    fn normalizes_and_clamps_character_scaling_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                min_character_scaling_percent: 50,
                max_character_scaling_percent: 180,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\charscalex150 wide \charscalex5 narrow \charscalex999 clamped \charscalex0 reset\par}",
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

        assert_eq!(style_for("wide").character_scaling_percent, 150);
        assert_eq!(style_for("narrow").character_scaling_percent, 50);
        assert_eq!(style_for("clamped").character_scaling_percent, 180);
        assert_eq!(style_for("reset").character_scaling_percent, 100);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("character scaling clamped"))
        );
    }

    #[test]
    fn strict_spec_clamps_zero_character_scaling_instead_of_word_resetting_it() {
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\charscalex0 bounded\par}",
            &RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].style.character_scaling_percent, 25);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("clamped from 0% to 25%"))
        );
    }

    #[test]
    fn normalizes_page_geometry_controls() {
        let output = parse_rtf(
            r"{\rtf1\paperw15840\paperh12240\margl720\margr720\margt1440\margb1440\gutter360\headery360\footery1080 Body\par}",
        )
        .unwrap();

        assert_eq!(output.document.page.width_twips, 15_840);
        assert_eq!(output.document.page.height_twips, 12_240);
        assert_eq!(output.document.page.margin_left_twips, 720);
        assert_eq!(output.document.page.margin_right_twips, 720);
        assert_eq!(output.document.page.margin_top_twips, 1_440);
        assert_eq!(output.document.page.margin_bottom_twips, 1_440);
        assert_eq!(output.document.page.gutter_twips, 360);
        assert!(!output.document.page.mirror_margins);
        assert!(!output.document.page.gutter_on_right);
        assert_eq!(output.document.page.header_distance_twips, 360);
        assert_eq!(output.document.page.footer_distance_twips, 1_080);
    }

    #[test]
    fn normalizes_page_vertical_alignment_controls() {
        let output = parse_rtf(r"{\rtf1\vertalc Centered\par}").unwrap();
        assert_eq!(
            output.document.page.vertical_alignment,
            PageVerticalAlignment::Center
        );

        let output = parse_rtf(r"{\rtf1\vertalb Bottom\par}").unwrap();
        assert_eq!(
            output.document.page.vertical_alignment,
            PageVerticalAlignment::Bottom
        );

        let output = parse_rtf(r"{\rtf1\vertalt Top\par}").unwrap();
        assert_eq!(
            output.document.page.vertical_alignment,
            PageVerticalAlignment::Top
        );
    }

    #[test]
    fn final_page_vertical_alignment_applies_to_the_whole_current_section() {
        let output = parse_rtf(r"{\rtf1\vertalc Centered\par\vertalt Top reset\par}").unwrap();

        assert_eq!(
            output.document.page.vertical_alignment,
            PageVerticalAlignment::Top
        );
        assert!(
            !output
                .document
                .blocks
                .iter()
                .any(|block| matches!(block, Block::SectionSettings(_)))
        );
    }

    #[test]
    fn normalizes_facing_page_and_mirror_margin_controls_as_distinct_safe_metadata() {
        let output = parse_rtf(r"{\rtf1\facingp\gutter360 Body\par}").unwrap();

        assert!(output.document.page.facing_pages);
        assert!(!output.document.page.mirror_margins);
        assert_eq!(output.document.page.gutter_twips, 360);

        let output = parse_rtf(r"{\rtf1\facingp0 Body\par}").unwrap();
        assert!(!output.document.page.facing_pages);

        let output = parse_rtf(r"{\rtf1\margmirror0 Body\par}").unwrap();
        assert!(!output.document.page.facing_pages);
        assert!(!output.document.page.mirror_margins);

        let options = RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\margmirsxn\gutter360 Body\par}", &options)
                .unwrap();
        assert!(output.document.page.mirror_margins);
        assert_eq!(output.document.page.gutter_twips, 360);

        let output = parse_rtf(r"{\rtf1\margmirsxn0 Body\par}").unwrap();
        assert!(!output.document.page.mirror_margins);

        let output = parse_rtf(r"{\rtf1\margmirsxn Body\par}").unwrap();
        assert!(!output.document.page.mirror_margins);
    }

    #[test]
    fn normalizes_right_to_left_gutter_controls_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1\rtlgutter\gutter360 Body\par}").unwrap();

        assert!(output.document.page.gutter_on_right);
        assert_eq!(output.document.page.gutter_twips, 360);

        let output = parse_rtf(r"{\rtf1\rtlgutter0 Body\par}").unwrap();
        assert!(!output.document.page.gutter_on_right);

        let output = parse_rtf(r"{\rtf1\gutterprl\gutter360 Body\par}").unwrap();
        assert!(output.document.page.gutter_at_top);
        assert!(!output.document.page.gutter_on_right);
        assert_eq!(output.document.page.gutter_twips, 360);

        let output = parse_rtf(r"{\rtf1\gutterprl0 Body\par}").unwrap();
        assert!(!output.document.page.gutter_at_top);
    }

    #[test]
    fn normalizes_page_border_controls_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1\pgbrdrt\brdrs\brdrw80\brsp240\pgbrdrb\brdrdb\brdrw40\brsp120 Body\par}",
        )
        .unwrap();

        assert!(output.document.page.page_borders.top.visible);
        assert_eq!(
            output.document.page.page_borders.top.style,
            BorderStyle::Single
        );
        assert_eq!(output.document.page.page_borders.top.width_twips, 80);
        assert_eq!(
            output.document.page.page_border_spacing_twips.top_twips,
            240
        );
        assert!(output.document.page.page_borders.bottom.visible);
        assert_eq!(
            output.document.page.page_borders.bottom.style,
            BorderStyle::Double
        );
        assert_eq!(output.document.page.page_borders.bottom.width_twips, 40);
        assert_eq!(
            output.document.page.page_border_spacing_twips.bottom_twips,
            120
        );
        assert!(!output.document.page.page_borders.left.visible);
        assert!(!output.document.page.page_borders.right.visible);
    }

    #[test]
    fn normalizes_page_border_art_42_as_passive_vector_template() {
        let output = parse_rtf(r"{\rtf1\pgbrdrt\brdrart42\brdrw80 Body\par}").unwrap();

        assert!(output.document.page.page_borders.top.visible);
        assert_eq!(
            output.document.page.page_borders.top.style,
            BorderStyle::BorderArtScissors
        );
        assert_eq!(output.document.page.page_borders.top.width_twips, 80);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("page border art 42 rendered as bounded passive vector template")
        }));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
            "page border art should be consumed as bounded passive page metadata: {:?}",
            output.diagnostics
        );
    }

    #[test]
    fn normalizes_page_border_header_footer_controls_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1\pgbrdrt\pgbrdrhead\pgbrdrb\pgbrdrfoot Body\par}").unwrap();

        assert!(output.document.page.page_border_includes_header);
        assert!(output.document.page.page_border_includes_footer);

        let output = parse_rtf(r"{\rtf1\pgbrdrhead0\pgbrdrfoot0 Body\par}").unwrap();
        assert!(!output.document.page.page_border_includes_header);
        assert!(!output.document.page.page_border_includes_footer);
    }

    #[test]
    fn normalizes_page_border_reference_mode_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1\pgbrdropt\pgbrdrt\brdrs Body\par}").unwrap();

        assert!(!output.document.page.page_border_from_page_edge);

        let output = parse_rtf(r"{\rtf1\pgbrdropt0\pgbrdrt\brdrs Body\par}").unwrap();
        assert!(output.document.page.page_border_from_page_edge);
    }

    #[test]
    fn normalizes_later_section_page_vertical_alignment_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1 First\par\sect\sectd\vertalb Second\par}").unwrap();
        let settings = match &output.document.blocks[2] {
            Block::SectionSettings(settings) => settings,
            _ => panic!("expected section page settings block"),
        };

        assert_eq!(settings.vertical_alignment, PageVerticalAlignment::Bottom);
    }

    #[test]
    fn normalizes_later_section_rtl_gutter_as_safe_metadata() {
        let output =
            parse_rtf(r"{\rtf1 First\par\sect\sectd\rtlguttersxn\gutter360 Second\par}").unwrap();
        let settings = match &output.document.blocks[2] {
            Block::SectionSettings(settings) => settings,
            _ => panic!("expected section page settings block"),
        };

        assert!(settings.gutter_on_right);
        assert!(!settings.gutter_at_top);
        assert_eq!(settings.gutter_twips, 360);

        let output =
            parse_rtf(r"{\rtf1 First\par\sect\sectd\gutterprl\gutter360 Second\par}").unwrap();
        let settings = match &output.document.blocks[2] {
            Block::SectionSettings(settings) => settings,
            _ => panic!("expected section page settings block"),
        };

        assert!(settings.gutter_at_top);
        assert!(!settings.gutter_on_right);
        assert_eq!(settings.gutter_twips, 360);
        assert!(output.document.page.gutter_at_top);
    }

    #[test]
    fn normalizes_later_section_page_borders_as_safe_metadata() {
        let output =
            parse_rtf(r"{\rtf1 First\par\sect\sectd\pgbrdrl\brdrdash Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert!(settings.page_borders.left.visible);
        assert_eq!(settings.page_borders.left.style, BorderStyle::Dashed);
        assert!(!settings.page_borders.top.visible);
    }

    #[test]
    fn normalizes_later_section_page_border_header_footer_controls_as_safe_metadata() {
        let output =
            parse_rtf(r"{\rtf1 First\par\sect\sectd\pgbrdrhead\pgbrdrfoot Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert!(settings.page_border_includes_header);
        assert!(settings.page_border_includes_footer);
    }

    #[test]
    fn normalizes_later_section_page_border_reference_mode_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1 First\par\sect\sectd\pgbrdropt Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert!(!settings.page_border_from_page_edge);
    }

    #[test]
    fn clamps_extreme_page_border_width_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_table_border_width_twips: 40,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\pgbrdrt\brdrs\brdrw9999 Bordered\par}",
            &options,
        )
        .unwrap();

        assert_eq!(output.document.page.page_borders.top.width_twips, 40);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("border width clamped"))
        );
    }

    #[test]
    fn clamps_extreme_page_border_spacing_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_page_border_spacing_twips: 360,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\pgbrdrl\brdrs\brsp9999 Bordered\par}", &options)
                .unwrap();

        assert_eq!(
            output.document.page.page_border_spacing_twips.left_twips,
            360
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("page border spacing clamped"))
        );
    }

    #[test]
    fn normalizes_word_section_page_geometry_controls() {
        let output = parse_rtf(
            r"{\rtf1\pgwsxn15840\pghsxn12240\marglsxn720\margrsxn720\margtsxn1440\margbsxn1440\guttersxn360 Body\par}",
        )
        .unwrap();

        assert_eq!(output.document.page.width_twips, 15_840);
        assert_eq!(output.document.page.height_twips, 12_240);
        assert_eq!(output.document.page.margin_left_twips, 720);
        assert_eq!(output.document.page.margin_right_twips, 720);
        assert_eq!(output.document.page.margin_top_twips, 1_440);
        assert_eq!(output.document.page.margin_bottom_twips, 1_440);
        assert_eq!(output.document.page.gutter_twips, 360);
    }

    #[test]
    fn normalizes_later_section_header_footer_distances_as_safe_metadata() {
        let output =
            parse_rtf(r"{\rtf1 First\par\sect\sectd\headery480\footery960 Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert_eq!(settings.header_distance_twips, 480);
        assert_eq!(settings.footer_distance_twips, 960);
    }

    #[test]
    fn clamps_extreme_header_footer_distance_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_header_footer_distance_twips: 720,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\headery9999\footery9999 Body\par}", &options)
                .unwrap();

        assert_eq!(output.document.page.header_distance_twips, 720);
        assert_eq!(output.document.page.footer_distance_twips, 720);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("header distance clamped"))
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("footer distance clamped"))
        );
    }

    #[test]
    fn clamps_extreme_page_gutter_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_page_gutter_twips: 720,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\gutter9999 Body\par}", &options).unwrap();

        assert_eq!(output.document.page.gutter_twips, 720);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("page gutter clamped"))
        );
    }

    #[test]
    fn normalizes_later_section_page_geometry_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1 First\par\sect\sectd\pgwsxn10080\pghsxn7200\marglsxn720\guttersxn360 Second\par}",
        )
        .unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert_eq!(settings.width_twips, 10_080);
        assert_eq!(settings.height_twips, 7_200);
        assert_eq!(settings.margin_left_twips, 720);
        assert_eq!(settings.gutter_twips, 360);
    }

    #[test]
    fn normalizes_later_section_facing_pages_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1 First\par\sect\sectd\facingp Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert!(settings.facing_pages);
        assert!(!settings.mirror_margins);
    }

    #[test]
    fn normalizes_section_column_controls_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1\cols2\colsx720\linebetcol Body\par}").unwrap();

        assert_eq!(output.document.page.column_count, 2);
        assert_eq!(output.document.page.column_gap_twips, 720);
        assert!(output.document.page.line_between_columns);
    }

    #[test]
    fn normalizes_explicit_section_column_widths_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1\cols3\colsx360\colno1\colw1440\colsr240\colno2\colw2880\colsr480\colno3\colw1440 Body\par}",
        )
        .unwrap();

        assert_eq!(output.document.page.column_count, 3);
        assert_eq!(
            output.document.page.column_widths_twips,
            vec![1440, 2880, 1440]
        );
        assert_eq!(output.document.page.column_gaps_twips, vec![240, 480]);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_section_line_grid_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1\sectd\sectlinegrid480 Body\par}").unwrap();

        assert_eq!(output.document.page.text_line_grid_twips, Some(480));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("section line grid applied as bounded passive paragraph line pitch")
        }));
    }

    #[test]
    fn word_section_grid_specification_restores_preceding_line_grid() {
        let input =
            r"{\rtf1\sectd\sectlinegrid480\sectdefaultcl\sectspecifycl\sectspecifyl Body\par}";
        let output = parse_rtf(input).unwrap();
        assert_eq!(output.document.page.text_line_grid_twips, Some(480));

        let strict = parse_rtf_strict(input).unwrap();
        assert_eq!(strict.document.page.text_line_grid_twips, None);
    }

    #[test]
    fn normalizes_paragraph_no_snap_line_grid_control() {
        let output = parse_rtf(
            r"{\rtf1\sectd\sectlinegrid720 Grid\par\nosnaplinegrid Loose\par\nosnaplinegrid0 Grid again\par}",
        )
        .unwrap();
        let paragraph = |idx| match &output.document.blocks[idx] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(output.document.page.text_line_grid_twips, Some(720));
        assert!(paragraph(0).style.snap_to_line_grid);
        assert!(!paragraph(1).style.snap_to_line_grid);
        assert!(paragraph(2).style.snap_to_line_grid);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_later_section_column_controls_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1 First\par\sect\sectd\cols3\colsx360 Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section column settings block");
        };
        assert_eq!(settings.column_count, 3);
        assert_eq!(settings.column_gap_twips, 360);
    }

    #[test]
    fn normalizes_explicit_column_breaks_as_safe_passive_blocks() {
        let output = parse_rtf(r"{\rtf1\cols2 First\par\column Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::ColumnBreak));
    }

    #[test]
    fn normalizes_landscape_orientation_after_page_geometry() {
        let output = parse_rtf(r"{\rtf1\landscape\paperw12240\paperh15840 Body\par}").unwrap();

        assert!(output.document.page.landscape);
        assert_eq!(output.document.page.width_twips, 15_840);
        assert_eq!(output.document.page.height_twips, 12_240);
    }

    #[test]
    fn landscape_zero_keeps_portrait_page_geometry() {
        let output = parse_rtf(
            r"{\rtf1\landscape0\paperw12240\paperh15840 Body\par\sect\sectd\lndscpsxn0\pgwsxn12240\pghsxn15840 Later\par}",
        )
        .unwrap();

        assert!(!output.document.page.landscape);
        assert_eq!(output.document.page.width_twips, 12_240);
        assert_eq!(output.document.page.height_twips, 15_840);
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected later section settings");
        };
        assert!(!settings.landscape);
        assert_eq!(settings.width_twips, 12_240);
        assert_eq!(settings.height_twips, 15_840);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn zero_line_number_distance_does_not_enable_line_numbering() {
        let output = parse_rtf(r"{\rtf1\sectd\linex0 Body\par}").unwrap();

        assert!(!output.document.page.line_numbering.enabled);
        assert_eq!(output.document.page.line_numbering.distance_twips, 0);
        assert_eq!(document_text(&output.document), "Body");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("line numbering approximated"))
        );
    }

    #[test]
    fn normalizes_no_line_number_paragraph_control() {
        let output = parse_rtf(
            r"{\rtf1\sectd\linex360 Numbered\par\noline Suppressed\par\noline0 Numbered again\par}",
        )
        .unwrap();
        let paragraph = |idx| match &output.document.blocks[idx] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert!(!paragraph(0).style.suppress_line_numbers);
        assert!(paragraph(1).style.suppress_line_numbers);
        assert!(!paragraph(2).style.suppress_line_numbers);
        assert!(output.document.page.line_numbering.enabled);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("line numbering rendered as bounded passive margin text")
        }));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("line numbering approximated"))
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_section_breaks_as_safe_passive_blocks() {
        let output = parse_rtf(r"{\rtf1 First\par\sect Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
    }

    #[test]
    fn odd_and_even_section_breaks_become_safe_parity_blocks() {
        let odd = parse_rtf(r"{\rtf1 First\par\sbkodd\sect Second\par}").unwrap();
        let even = parse_rtf(r"{\rtf1 First\par\sbkeven\sect Second\par}").unwrap();

        assert!(matches!(odd.document.blocks[1], Block::OddPageSectionBreak));
        assert!(matches!(
            even.document.blocks[1],
            Block::EvenPageSectionBreak
        ));
    }

    #[test]
    fn column_section_breaks_become_safe_column_break_blocks() {
        let output = parse_rtf(r"{\rtf1\cols2 First\par\sbkcol\sect Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::ColumnBreak));
        assert!(
            !output
                .document
                .blocks
                .iter()
                .any(|block| matches!(block, Block::SectionBreak | Block::PageBreak))
        );
    }

    #[test]
    fn word_compatible_no_break_section_control_matches_word_page_break_recovery() {
        let output = parse_rtf(r"{\rtf1 First\par\sbknone\sect Second\par}").unwrap();

        assert!(
            output
                .document
                .blocks
                .iter()
                .any(|block| matches!(block, Block::SectionBreak))
        );
    }

    #[test]
    fn strict_spec_no_break_section_control_remains_continuous() {
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1 First\par\sbknone\sect Second\par}",
            &RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
        )
        .unwrap();

        assert!(
            output
                .document
                .blocks
                .iter()
                .any(|block| matches!(block, Block::ContinuousSectionBreak))
        );
    }

    #[test]
    fn normalizes_paragraph_direction_controls_as_alignment_metadata() {
        let output =
            parse_rtf(r"{\rtf1\rtlpar Right placed\par\ltrpar Left placed\par\rtlpar Right again\par\pard Default left\par}")
                .unwrap();

        let paragraph = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected paragraph {index}, got {other:?}"),
        };

        assert_eq!(paragraph(0).style.alignment, Alignment::Right);
        assert_eq!(paragraph(1).style.alignment, Alignment::Left);
        assert_eq!(paragraph(2).style.alignment, Alignment::Right);
        assert_eq!(paragraph(3).style.alignment, Alignment::Left);
    }

    #[test]
    fn normalizes_distributed_and_thai_alignment_controls() {
        let output = parse_rtf(
            r"{\rtf1\qd Distributed\par\qk Thai distributed\par\qt Thai justified\par\ql Left\par}",
        )
        .unwrap();

        let paragraph = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected paragraph {index}, got {other:?}"),
        };

        assert_eq!(paragraph(0).style.alignment, Alignment::Distributed);
        assert_eq!(paragraph(1).style.alignment, Alignment::Justified);
        assert_eq!(paragraph(2).style.alignment, Alignment::Justified);
        assert_eq!(paragraph(3).style.alignment, Alignment::Left);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_keep_paragraph_pagination_controls() {
        let output = parse_rtf(
            r"{\rtf1\keep Keep together\par\keep0 Plain\par\keepn Keep next\par\widctlpar Widow\par\nowidctlpar No widow\par\widctlpar Widow alias\par\nowidowctrl No widow alias\par}",
        )
        .unwrap();

        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected first paragraph, got {other:?}"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected second paragraph, got {other:?}"),
        };
        let third = match &output.document.blocks[2] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected third paragraph, got {other:?}"),
        };
        let fourth = match &output.document.blocks[3] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected fourth paragraph, got {other:?}"),
        };
        let fifth = match &output.document.blocks[4] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected fifth paragraph, got {other:?}"),
        };
        let sixth = match &output.document.blocks[5] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected sixth paragraph, got {other:?}"),
        };
        let seventh = match &output.document.blocks[6] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected seventh paragraph, got {other:?}"),
        };

        assert!(first.style.keep_together);
        assert!(!second.style.keep_together);
        assert!(third.style.keep_with_next);
        assert!(fourth.style.widow_control);
        assert!(!fifth.style.widow_control);
        assert!(sixth.style.widow_control);
        assert!(!seventh.style.widow_control);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_paragraph_hyphenation_controls() {
        let output = parse_rtf(r"{\rtf1\hyphpar Hyphenated\par\hyphpar0 Plain\par}").unwrap();
        let paragraph = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph {index}"),
        };

        assert!(paragraph(0).style.auto_hyphenation);
        assert!(!paragraph(1).style.auto_hyphenation);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("paragraph hyphenation rendered as bounded passive soft hyphenation")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("paragraph hyphenation disabled")
        }));
    }

    #[test]
    fn document_hyphenation_sets_paragraph_default_across_resets() {
        let output = parse_rtf(
            r"{\rtf1\hyphauto\pard Document default\par\hyphpar0 Override off\par\pard Restored default\par\hyphauto0\pard Disabled default\par}",
        )
        .unwrap();
        let paragraph = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected paragraph {index}, got {other:?}"),
        };

        assert!(paragraph(0).style.auto_hyphenation);
        assert!(!paragraph(1).style.auto_hyphenation);
        assert!(paragraph(2).style.auto_hyphenation);
        assert!(!paragraph(3).style.auto_hyphenation);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("document hyphenation rendered as bounded passive soft hyphenation")
        }));
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("document hyphenation disabled") })
        );
    }

    #[test]
    fn capital_word_hyphenation_sets_paragraph_default_across_resets() {
        let output = parse_rtf(
            r"{\rtf1\hyphauto\hyphcaps0\pard Caps suppressed\par\hyphcaps\pard Caps enabled\par}",
        )
        .unwrap();
        let paragraph = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected paragraph {index}, got {other:?}"),
        };

        assert!(paragraph(0).style.auto_hyphenation);
        assert!(!paragraph(0).style.hyphenate_caps);
        assert!(paragraph(1).style.auto_hyphenation);
        assert!(paragraph(1).style.hyphenate_caps);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("document hyphenation rendered as bounded passive soft hyphenation")
        }));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| { !diagnostic.message.contains("capitalized word hyphenation") })
        );
    }

    #[test]
    fn hyphenation_zone_sets_paragraph_default_across_resets() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_hyphenation_zone_twips: 720,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\hyphauto\hyphhotz480\pard First\par\hyphhotz9999\pard Second\par}",
            &options,
        )
        .unwrap();
        let paragraph = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected paragraph {index}, got {other:?}"),
        };

        assert!(paragraph(0).style.auto_hyphenation);
        assert_eq!(paragraph(0).style.hyphenation_zone_twips, 480);
        assert!(paragraph(1).style.auto_hyphenation);
        assert_eq!(paragraph(1).style.hyphenation_zone_twips, 720);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("hyphenation zone applied to bounded passive hyphenation")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("hyphenation zone clamped from 9999 to 720 twips")
        }));
    }

    #[test]
    fn document_widow_control_sets_paragraph_default_across_resets() {
        let output = parse_rtf(
            r"{\rtf1\widowctrl\pard First\par\nowidctlpar Second\par\pard Third\par\widowctrl0\pard Fourth\par}",
        )
        .unwrap();

        let paragraph = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected paragraph {index}, got {other:?}"),
        };

        assert!(paragraph(0).style.widow_control);
        assert!(!paragraph(1).style.widow_control);
        assert!(paragraph(2).style.widow_control);
        assert!(!paragraph(3).style.widow_control);
    }

    #[test]
    fn word_compatible_mode_ignores_drop_cap_without_frame_dimensions() {
        let output = parse_rtf(r"{\rtf1\dropcapli3\dropcapt1 Dropped\par\pard Plain\par}").unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected first paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected second paragraph"),
        };

        assert_eq!(first.style.drop_cap_lines, 0);
        assert_eq!(second.style.drop_cap_lines, 0);
        assert_eq!(first.runs[0].text, "Dropped");
        assert_eq!(second.runs[0].text, "Plain");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn word_compatible_mode_retains_drop_cap_with_frame_dimensions() {
        let output =
            parse_rtf(r"{\rtf1\absw1440\absh720\dropcapli3\dropcapt1 Dropped\par}").unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected first paragraph"),
        };

        assert_eq!(first.style.drop_cap_lines, 3);
        assert_eq!(first.style.frame_width_twips, Some(1_440));
        assert_eq!(first.style.frame_height_twips, Some(720));
    }

    #[test]
    fn word_compatible_mode_retains_each_independent_drop_cap_frame_dimension() {
        let output = parse_rtf(
            r"{\rtf1\absw720\dropcapli3\dropcapt1 W\par\pard\absh720\dropcapli3\dropcapt1 H\par}",
        )
        .unwrap();
        let Block::Paragraph(width_only) = &output.document.blocks[0] else {
            panic!("expected width-only paragraph");
        };
        let Block::Paragraph(height_only) = &output.document.blocks[1] else {
            panic!("expected height-only paragraph");
        };
        assert_eq!(width_only.style.drop_cap_lines, 3);
        assert_eq!(width_only.style.frame_width_twips, Some(720));
        assert_eq!(width_only.style.frame_height_twips, None);
        assert_eq!(height_only.style.drop_cap_lines, 3);
        assert_eq!(height_only.style.frame_width_twips, None);
        assert_eq!(height_only.style.frame_height_twips, Some(720));
    }

    #[test]
    fn word_compatible_mode_retains_signed_drop_cap_frame_dimensions() {
        let output = parse_rtf(include_str!(
            "../../../fixtures/framed-drop-cap-signed-dimensions-passive.rtf"
        ))
        .unwrap();
        let frames = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph) if paragraph.style.drop_cap_lines > 1 => {
                    Some(paragraph)
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(frames.len(), 8);
        assert_eq!(frames[0].style.frame_width_twips, Some(720));
        assert_eq!(frames[0].style.frame_height_twips, Some(720));
        assert_eq!(frames[1].style.frame_width_twips, Some(-720));
        assert_eq!(frames[1].style.frame_height_twips, Some(720));
        assert_eq!(frames[2].style.frame_width_twips, Some(720));
        assert_eq!(frames[2].style.frame_height_twips, Some(-720));
        assert_eq!(frames[3].style.frame_width_twips, Some(-720));
        assert_eq!(frames[3].style.frame_height_twips, Some(-720));
        assert_eq!(frames[4].style.frame_width_twips, Some(-720));
        assert_eq!(frames[4].style.frame_height_twips, None);
        assert_eq!(frames[5].style.frame_height_twips, Some(1_440));
        assert_eq!(frames[6].style.frame_width_twips, Some(720));
        assert_eq!(frames[7].style.frame_width_twips, Some(-720));
    }

    #[test]
    fn paragraph_frame_dimensions_clamp_signed_magnitudes() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_shape_dimension_twips: 480,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\absw-9999\absh9999\dropcapli3\dropcapt1 X\par}",
            &options,
        )
        .unwrap();
        let Block::Paragraph(paragraph) = &output.document.blocks[0] else {
            panic!("expected paragraph");
        };
        assert_eq!(paragraph.style.frame_width_twips, Some(-480));
        assert_eq!(paragraph.style.frame_height_twips, Some(480));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("paragraph frame dimension clamped from -9999 to -480 twips")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("paragraph frame dimension clamped from 9999 to 480 twips")
        }));
    }

    #[test]
    fn paragraph_frame_collision_controls_remain_passive_metadata() {
        let output = parse_rtf(include_str!(
            "../../../fixtures/framed-drop-cap-collision-controls-passive.rtf"
        ))
        .unwrap();
        let frames = output
            .document
            .blocks
            .iter()
            .filter(|block| {
                matches!(block, Block::Paragraph(paragraph) if paragraph.style.drop_cap_lines > 1)
            })
            .count();
        assert_eq!(frames, 6);
        for control in ["absnoovrlp", "abslock"] {
            assert!(output.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(&format!(
                    "paragraph frame control \\{control} retained through bounded passive placement"
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
    fn word_compatible_mode_keeps_drop_cap_vertical_alignments_passive() {
        let output = parse_rtf(include_str!(
            "../../../fixtures/framed-drop-cap-vertical-alignment-passive.rtf"
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

        assert_eq!(paragraphs.len(), 8);
        for index in [0, 2, 4, 6] {
            assert_eq!(
                paragraphs[index].style.frame_vertical_anchor,
                StaticShapeVerticalAnchor::Paragraph
            );
            assert_eq!(paragraphs[index].style.frame_vertical_offset_twips, 0);
        }
        assert_eq!(paragraphs[0].style.frame_vertical_alignment, None);
        assert_eq!(
            paragraphs[2].style.frame_vertical_alignment,
            Some(ParagraphFrameVerticalAlignment::Top)
        );
        assert_eq!(
            paragraphs[4].style.frame_vertical_alignment,
            Some(ParagraphFrameVerticalAlignment::Center)
        );
        assert_eq!(
            paragraphs[6].style.frame_vertical_alignment,
            Some(ParagraphFrameVerticalAlignment::Bottom)
        );
        for index in [1, 3, 5, 7] {
            assert_eq!(paragraphs[index].style, ParagraphStyle::default());
        }
    }

    #[test]
    fn word_compatible_mode_retains_ordinary_frame_vertical_alignments() {
        let output = parse_rtf(include_str!(
            "../../../fixtures/paragraph-frame-vertical-alignment-passive.rtf"
        ))
        .unwrap();
        let frames = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph) if paragraph.style.frame_width_twips.is_some() => {
                    Some((
                        paragraph.style.frame_vertical_anchor,
                        paragraph.style.frame_vertical_alignment,
                    ))
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            frames,
            vec![
                (
                    StaticShapeVerticalAnchor::Page,
                    Some(ParagraphFrameVerticalAlignment::Top)
                ),
                (
                    StaticShapeVerticalAnchor::Page,
                    Some(ParagraphFrameVerticalAlignment::Center)
                ),
                (
                    StaticShapeVerticalAnchor::Page,
                    Some(ParagraphFrameVerticalAlignment::Bottom)
                ),
                (
                    StaticShapeVerticalAnchor::Margin,
                    Some(ParagraphFrameVerticalAlignment::Top)
                ),
                (
                    StaticShapeVerticalAnchor::Margin,
                    Some(ParagraphFrameVerticalAlignment::Center)
                ),
                (
                    StaticShapeVerticalAnchor::Margin,
                    Some(ParagraphFrameVerticalAlignment::Bottom)
                ),
                (
                    StaticShapeVerticalAnchor::Paragraph,
                    Some(ParagraphFrameVerticalAlignment::Center)
                ),
            ]
        );
        for control in ["posyt", "posyc", "posyb"] {
            assert!(output.diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains(&format!("paragraph frame vertical alignment \\{control}"))
            }));
        }
    }

    #[test]
    fn ordinary_frame_vertical_precedence_and_edge_controls_are_normalized() {
        let output = parse_rtf(include_str!(
            "../../../fixtures/paragraph-frame-vertical-precedence-passive.rtf"
        ))
        .unwrap();
        let frames = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph) if paragraph.style.frame_width_twips.is_some() => {
                    Some((
                        paragraph.style.frame_vertical_anchor,
                        paragraph.style.frame_vertical_alignment,
                        paragraph.style.frame_vertical_offset_twips,
                    ))
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            frames,
            vec![
                (StaticShapeVerticalAnchor::Page, None, 1440),
                (
                    StaticShapeVerticalAnchor::Page,
                    Some(ParagraphFrameVerticalAlignment::Center),
                    1440
                ),
                (StaticShapeVerticalAnchor::Margin, None, 720),
                (
                    StaticShapeVerticalAnchor::Margin,
                    Some(ParagraphFrameVerticalAlignment::Bottom),
                    720
                ),
                (
                    StaticShapeVerticalAnchor::Page,
                    Some(ParagraphFrameVerticalAlignment::Inside),
                    0
                ),
                (
                    StaticShapeVerticalAnchor::Page,
                    Some(ParagraphFrameVerticalAlignment::Inside),
                    0
                ),
                (
                    StaticShapeVerticalAnchor::Page,
                    Some(ParagraphFrameVerticalAlignment::Outside),
                    0
                ),
                (
                    StaticShapeVerticalAnchor::Page,
                    Some(ParagraphFrameVerticalAlignment::Outside),
                    0
                ),
                (
                    StaticShapeVerticalAnchor::Page,
                    Some(ParagraphFrameVerticalAlignment::Inline),
                    0
                ),
                (
                    StaticShapeVerticalAnchor::Margin,
                    Some(ParagraphFrameVerticalAlignment::Inline),
                    0
                ),
            ]
        );
        for control in ["posyin", "posyout", "posyil"] {
            assert!(output.diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains(&format!("paragraph frame vertical alignment \\{control}"))
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
    fn word_compatible_mode_retains_absolute_drop_cap_frame_anchors() {
        let output = parse_rtf(include_str!(
            "../../../fixtures/framed-drop-cap-absolute-overlap-passive.rtf"
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

        assert_eq!(paragraphs.len(), 7);
        assert_eq!(
            paragraphs[1].style.frame_horizontal_anchor,
            StaticShapeHorizontalAnchor::Margin
        );
        assert_eq!(
            paragraphs[1].style.frame_vertical_anchor,
            StaticShapeVerticalAnchor::Margin
        );
        assert_eq!(paragraphs[1].style.frame_horizontal_offset_twips, 720);
        assert_eq!(paragraphs[1].style.frame_vertical_offset_twips, 720);
        assert_eq!(
            paragraphs[5].style.frame_horizontal_anchor,
            StaticShapeHorizontalAnchor::Page
        );
        assert_eq!(
            paragraphs[5].style.frame_vertical_anchor,
            StaticShapeVerticalAnchor::Page
        );
        assert_eq!(paragraphs[5].style.frame_horizontal_offset_twips, 720);
        assert_eq!(paragraphs[5].style.frame_vertical_offset_twips, 720);
    }

    #[test]
    fn word_compatible_mode_retains_facing_drop_cap_frame_alignment() {
        let output = parse_rtf(include_str!(
            "../../../fixtures/framed-drop-cap-facing-alignment-passive.rtf"
        ))
        .unwrap();
        assert!(output.document.page.facing_pages);
        assert!(!output.document.page.mirror_margins);
        let frames = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph) if paragraph.style.drop_cap_lines > 1 => {
                    Some(paragraph)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(frames.len(), 4);
        assert_eq!(
            frames[0].style.frame_horizontal_alignment,
            Some(TableRowAlignment::Inside)
        );
        assert_eq!(
            frames[1].style.frame_horizontal_alignment,
            Some(TableRowAlignment::Outside)
        );
        assert_eq!(
            frames[2].style.frame_horizontal_alignment,
            Some(TableRowAlignment::Inside)
        );
        assert_eq!(
            frames[3].style.frame_horizontal_alignment,
            Some(TableRowAlignment::Outside)
        );
    }

    #[test]
    fn word_compatible_mode_retains_signed_drop_cap_frame_offsets() {
        let output = parse_rtf(include_str!(
            "../../../fixtures/framed-drop-cap-negative-positioning-passive.rtf"
        ))
        .unwrap();
        let frames = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph) if paragraph.style.drop_cap_lines > 1 => {
                    Some(paragraph)
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(frames.len(), 8);
        assert_eq!(frames[0].style.frame_horizontal_offset_twips, 360);
        assert_eq!(frames[1].style.frame_horizontal_offset_twips, 360);
        assert_eq!(frames[2].style.frame_horizontal_offset_twips, 720);
        assert_eq!(frames[3].style.frame_vertical_offset_twips, 360);
        assert_eq!(frames[4].style.frame_vertical_offset_twips, 720);
        assert_eq!(frames[5].style.frame_horizontal_offset_twips, 360);
        assert_eq!(frames[5].style.frame_vertical_offset_twips, 360);
        assert_eq!(frames[6].style.frame_horizontal_offset_twips, -360);
        assert_eq!(frames[7].style.frame_vertical_offset_twips, -360);
    }

    #[test]
    fn word_compatible_mode_ignores_drop_cap_after_frame_dimension_reset() {
        let input = [
            "{\\rtf1",
            "\\absw720",
            "\\absw0",
            "\\dropcapli3",
            "\\dropcapt1 Dropped",
            "\\par}",
        ]
        .concat();
        let output = parse_rtf(&input).unwrap();
        let Block::Paragraph(paragraph) = &output.document.blocks[0] else {
            panic!("expected paragraph");
        };
        assert_eq!(paragraph.style.drop_cap_lines, 0);
        assert_eq!(paragraph.style.frame_width_twips, None);
    }

    #[test]
    fn strict_spec_mode_retains_standalone_drop_cap_metadata() {
        let options = RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\dropcapli3\dropcapt1 Dropped\par}", &options)
                .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected first paragraph"),
        };

        assert_eq!(first.style.drop_cap_lines, 3);
    }

    #[test]
    fn direct_overline_controls_are_strict_spec_only() {
        let input = br"{\rtf1\ol over\ol0 plain \ol again\olnone done\par}";
        let word = parse_rtf_bytes(input).unwrap();
        let strict = parse_rtf_bytes_with_options(
            input,
            &RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
        )
        .unwrap();
        fn paragraph(output: &ParseOutput) -> &Paragraph {
            match &output.document.blocks[0] {
                Block::Paragraph(paragraph) => paragraph,
                _ => panic!("expected paragraph"),
            }
        }

        assert!(paragraph(&word).runs.iter().all(|run| !run.style.overline));
        let strict = paragraph(&strict);
        assert!(
            strict
                .runs
                .iter()
                .any(|run| { run.text.trim() == "over" && run.style.overline })
        );
        assert!(
            strict
                .runs
                .iter()
                .any(|run| { run.text.trim() == "again" && run.style.overline })
        );
        assert!(
            strict
                .runs
                .iter()
                .any(|run| { run.text.trim() == "plain" && !run.style.overline })
        );
        assert!(
            strict
                .runs
                .iter()
                .any(|run| { run.text.trim() == "done" && !run.style.overline })
        );
    }

    #[test]
    fn clamps_extreme_drop_cap_line_controls() {
        let options = RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\dropcapli999 Oversized\par\dropcapt0 Normal\par}",
            &options,
        )
        .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected first paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected second paragraph"),
        };

        assert_eq!(first.style.drop_cap_lines, 10);
        assert_eq!(second.style.drop_cap_lines, 0);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("drop cap lines clamped"))
        );
    }

    #[test]
    fn normalizes_paragraph_indent_controls() {
        let output = parse_rtf(r"{\rtf1\li720\ri360\fi-240 Indented\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.left_indent_twips, 720);
        assert_eq!(paragraph.style.right_indent_twips, 360);
        assert_eq!(paragraph.style.first_line_indent_twips, -240);
    }

    #[test]
    fn normalizes_word_directional_indents_and_ignores_nonstandard_fin() {
        let output = parse_rtf(r"{\rtf1\lin720\rin360\fin-240 Indented\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.left_indent_twips, 720);
        assert_eq!(paragraph.style.right_indent_twips, 360);
        assert_eq!(paragraph.style.first_line_indent_twips, 0);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn clamps_extreme_paragraph_indent_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_paragraph_indent_twips: 480,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\li99999\ri-99999\fi99999 Too much indent\par}",
            &options,
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.left_indent_twips, 480);
        assert_eq!(paragraph.style.right_indent_twips, -480);
        assert_eq!(paragraph.style.first_line_indent_twips, 480);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("left indent clamped"))
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("right indent clamped"))
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("first-line indent clamped"))
        );
    }

    #[test]
    fn clamps_extreme_word_directional_indents_and_ignores_nonstandard_fin() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_paragraph_indent_twips: 480,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\lin99999\rin-99999\fin99999 Too much indent\par}",
            &options,
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.left_indent_twips, 480);
        assert_eq!(paragraph.style.right_indent_twips, -480);
        assert_eq!(paragraph.style.first_line_indent_twips, 0);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("left indent clamped"))
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("right indent clamped"))
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("first-line indent clamped"))
        );
    }

    #[test]
    fn clamps_extreme_page_geometry_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                min_page_dimension_twips: 1_000,
                max_page_dimension_twips: 2_000,
                max_page_margin_twips: 500,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\paperw50\paperh99999\margl-20\margr9999 Body\par}",
            &options,
        )
        .unwrap();

        assert_eq!(output.document.page.width_twips, 1_000);
        assert_eq!(output.document.page.height_twips, 2_000);
        assert_eq!(output.document.page.margin_left_twips, 0);
        assert_eq!(output.document.page.margin_right_twips, 500);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("paper width clamped"))
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("right margin"))
        );
    }

    #[test]
    fn clamps_extreme_section_column_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_section_columns: 2,
                max_column_gap_twips: 360,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\cols99\colsx9999 Body\par}", &options).unwrap();

        assert_eq!(output.document.page.column_count, 2);
        assert_eq!(output.document.page.column_gap_twips, 360);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("section columns clamped"))
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("column gap clamped"))
        );
    }

    #[test]
    fn normalizes_foreground_and_background_color_controls() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;\red0\green255\blue0;}{\cf1 Red} {\highlight2 Marked} {\highlight0 Plain} {\cb2 Shaded} {\chshdng5000\chcbpat2 Tinted} {\chshdng5000\chcbpat0 Automatic}\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        let red = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Red")
            .expect("red run");
        assert_eq!(red.style.color_index, 1);

        let marked = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Marked")
            .expect("marked run");
        assert_eq!(marked.style.highlight_index, Some(2));

        let plain = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Plain")
            .expect("plain run");
        assert_eq!(plain.style.highlight_index, None);

        let shaded = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Shaded")
            .expect("shaded run");
        assert_eq!(shaded.style.highlight_index, Some(2));

        let tinted = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Tinted")
            .expect("tinted run");
        assert_eq!(tinted.style.highlight_index, Some(2));
        assert_eq!(tinted.style.highlight_shading_basis_points, 5_000);

        let automatic = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Automatic")
            .expect("automatic-color shaded run");
        assert_eq!(automatic.style.highlight_index, Some(0));
        assert_eq!(automatic.style.highlight_shading_basis_points, 5_000);
    }

    #[test]
    fn clamps_extreme_character_shading_controls() {
        let output = parse_rtf_bytes(br"{\rtf1\chshdng-200 low \chshdng99999 high\par}").unwrap();
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

        assert_eq!(style_for("low").highlight_shading_basis_points, 0);
        assert_eq!(style_for("high").highlight_shading_basis_points, 10_000);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("character shading clamped"))
        );
    }

    #[test]
    fn normalizes_extended_shading_pattern_controls() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;}\cbpat1\bgfdiag forward\par\pard\cbpat1\bgdkdcross dark\par\trowd\trcbpat1\trbgdcross\cellx1440 row\cell\clcbpat1\clbgdkcross\cellx2880 cell\cell\row}",
        )
        .unwrap();

        let forward = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected forward paragraph"),
        };
        let dark = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected dark paragraph"),
        };
        assert_eq!(
            forward.style.shading_pattern,
            ShadingPattern::ForwardDiagonal
        );
        assert_eq!(
            dark.style.shading_pattern,
            ShadingPattern::DarkDiagonalCross
        );

        let table = match &output.document.blocks[2] {
            Block::Table(table) => table,
            _ => panic!("expected table"),
        };
        assert_eq!(
            table.rows[0].cells[0].shading_pattern,
            ShadingPattern::DiagonalCross
        );
        assert_eq!(
            table.rows[0].cells[1].shading_pattern,
            ShadingPattern::DarkCross
        );
        for forbidden in ["bgfdiag", "bgdkdcross", "trbgdcross", "clbgdkcross"] {
            assert!(
                !document_text(&output.document).contains(forbidden),
                "extended shading pattern control leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn normalizes_paragraph_shading_controls() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red240\green240\blue0;}\cbpat1 Shaded\par\cbpat0 Plain\par}",
        )
        .unwrap();

        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(first.style.shading_color_index, Some(1));
        assert_eq!(second.style.shading_color_index, None);
    }

    #[test]
    fn normalizes_paragraph_border_controls() {
        let output =
            parse_rtf(r"{\rtf1\brdrb\brdrs\brdrw80\brdrcf2 Bordered\par\pard Plain\par}").unwrap();

        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert!(first.style.borders.bottom.visible);
        assert_eq!(first.style.borders.bottom.width_twips, 80);
        assert_eq!(first.style.borders.bottom.color_index, Some(2));
        assert!(!second.style.borders.bottom.visible);
    }

    #[test]
    fn normalizes_paragraph_bar_border_as_safe_left_border() {
        let output =
            parse_rtf(r"{\rtf1\brdrbar\brdrs\brdrw60\brdrcf1 Barred\par\pard Plain\par}").unwrap();

        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert!(first.style.borders.left.visible);
        assert_eq!(first.style.borders.left.width_twips, 60);
        assert_eq!(first.style.borders.left.color_index, Some(1));
        assert!(!second.style.borders.left.visible);
    }

    #[test]
    fn normalizes_paragraph_between_border_controls() {
        let output =
            parse_rtf(r"{\rtf1\brdrbtw\brdrs\brdrw60\brdrcf1 First\par\brdrbtw\brdrs\brdrw60\brdrcf1 Second\par}").unwrap();

        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        for paragraph in [first, second] {
            assert!(paragraph.style.borders.between.visible);
            assert_eq!(paragraph.style.borders.between.width_twips, 60);
            assert_eq!(paragraph.style.borders.between.color_index, Some(1));
        }
    }

    #[test]
    fn normalizes_paragraph_box_border_controls() {
        let output = parse_rtf(r"{\rtf1\box\brdrs\brdrw40 Boxed\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert!(paragraph.style.borders.left.visible);
        assert!(paragraph.style.borders.right.visible);
        assert!(paragraph.style.borders.top.visible);
        assert!(paragraph.style.borders.bottom.visible);
        assert_eq!(paragraph.style.borders.left.width_twips, 40);
        assert_eq!(paragraph.style.borders.right.width_twips, 40);
        assert_eq!(paragraph.style.borders.top.width_twips, 40);
        assert_eq!(paragraph.style.borders.bottom.width_twips, 40);
    }

    #[test]
    fn normalizes_character_border_controls() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;}\chbrdr\brdrs\brdrw80\brdrcf1 Bordered \chbrdr\brdrnone Plain\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let bordered = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Bordered")
            .expect("bordered run");
        let plain = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Plain")
            .expect("plain run");

        assert!(bordered.style.border.visible);
        assert_eq!(bordered.style.border.width_twips, 80);
        assert_eq!(bordered.style.border.color_index, Some(1));
        assert!(!plain.style.border.visible);
    }

    #[test]
    fn normalizes_paragraph_and_character_border_spacing_controls() {
        let output = parse_rtf(
            r"{\rtf1\box\brdrs\brsp240 Boxed\par\pard\chbrdr\brdrs\brsp120 Character\par}",
        )
        .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(first.style.borders.left.spacing_twips, 240);
        assert_eq!(first.style.borders.right.spacing_twips, 240);
        assert_eq!(first.style.borders.top.spacing_twips, 240);
        assert_eq!(first.style.borders.bottom.spacing_twips, 240);
        assert_eq!(second.runs[0].style.border.spacing_twips, 120);
    }

    #[test]
    fn clamps_extreme_paragraph_border_spacing_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_page_border_spacing_twips: 360,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\box\brdrs\brsp9999 Boxed\par}", &options)
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.borders.left.spacing_twips, 360);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("border spacing clamped"))
        );
    }

    #[test]
    fn clamps_extreme_paragraph_border_width_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_table_border_width_twips: 40,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\brdrb\brdrs\brdrw9999 Bordered\par}", &options)
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.borders.bottom.width_twips, 40);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("border width clamped"))
        );
    }

    #[test]
    fn clamps_extreme_character_border_width_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_table_border_width_twips: 120,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\chbrdr\brdrs\brdrw9999 Bordered\par}", &options)
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].style.border.width_twips, 120);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("border width clamped") })
        );
    }

    #[test]
    fn normalizes_line_spacing_controls() {
        let output = parse_rtf(r"{\rtf1\sl480\slmult1 Double spaced\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.line_spacing_twips, Some(480));
        assert!(paragraph.style.line_spacing_multiple);
    }

    #[test]
    fn normalizes_explicit_tab_stops() {
        let output = parse_rtf(r"{\rtf1\tx1440 Left\tab Right\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.tab_stops_twips, vec![1440]);
        assert_eq!(
            paragraph
                .runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            "Left\tRight"
        );
    }

    #[test]
    fn normalizes_document_default_tab_width() {
        let output = parse_rtf(r"{\rtf1\deftab360 Left\tab Right\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(output.document.default_tab_width_twips, 360);
        assert!(paragraph.style.tab_stops_twips.is_empty());
        assert_eq!(
            paragraph
                .runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            "Left\tRight"
        );
    }

    #[test]
    fn normalizes_tab_leader_controls_with_tab_stops() {
        let output = parse_rtf(
            r"{\rtf1\tldot\tx1440 Dot\tab Right\par\pard\tlmdot\tx1440 Middle\tab Right\par\pard\tleq\tx1440 Equal\tab Right\par}",
        )
        .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let third = match &output.document.blocks[2] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(first.style.tab_stops_twips, vec![1440]);
        assert_eq!(first.style.tab_stop_leaders, vec![TabLeader::Dots]);
        assert_eq!(second.style.tab_stops_twips, vec![1440]);
        assert_eq!(second.style.tab_stop_leaders, vec![TabLeader::MiddleDots]);
        assert_eq!(third.style.tab_stops_twips, vec![1440]);
        assert_eq!(third.style.tab_stop_leaders, vec![TabLeader::None]);
    }

    #[test]
    fn strict_spec_retains_non_word_equal_and_double_underline_tab_leaders() {
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\tleq\tx1440 Equal\tab Right\par\pard\tluldb\tx1440 Double\tab Right\par}",
            &RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
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

        assert_eq!(
            paragraphs[0].style.tab_stop_leaders,
            vec![TabLeader::Equals]
        );
        assert_eq!(
            paragraphs[1].style.tab_stop_leaders,
            vec![TabLeader::Underline]
        );
    }

    #[test]
    fn normalizes_tab_alignment_controls_with_tab_stops() {
        let output =
            parse_rtf(r"{\rtf1\tqr\tldot\tx1440 Left\tab 9\par\tqc\tx2160 Center\tab C\par\tqdec\tx2880 Value\tab 12.3\par}")
                .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let third = match &output.document.blocks[2] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(first.style.tab_stops_twips, vec![1440]);
        assert_eq!(first.style.tab_stop_leaders, vec![TabLeader::Dots]);
        assert_eq!(first.style.tab_stop_alignments, vec![TabAlignment::Right]);
        assert_eq!(second.style.tab_stops_twips, vec![1440, 2160]);
        assert_eq!(
            second.style.tab_stop_alignments,
            vec![TabAlignment::Right, TabAlignment::Center]
        );
        assert_eq!(third.style.tab_stops_twips, vec![1440, 2160, 2880]);
        assert_eq!(
            third.style.tab_stop_alignments,
            vec![
                TabAlignment::Right,
                TabAlignment::Center,
                TabAlignment::Decimal
            ]
        );
    }

    #[test]
    fn normalizes_bar_tab_stops_as_passive_tab_metadata() {
        let output =
            parse_rtf(r"{\rtf1\tb720\tqr\tx1440 Left\tab 9\par\tb2160 Bar only\par}").unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(first.style.tab_stops_twips, vec![720, 1440]);
        assert_eq!(
            first.style.tab_stop_alignments,
            vec![TabAlignment::Bar, TabAlignment::Right]
        );
        assert_eq!(second.style.tab_stops_twips, vec![720, 1440, 2160]);
        assert_eq!(
            second.style.tab_stop_alignments,
            vec![TabAlignment::Bar, TabAlignment::Right, TabAlignment::Bar]
        );
    }

    #[test]
    fn clamps_extreme_default_tab_width_control() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_tab_stop_twips: 480,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\deftab9999 Body\par}", &options).unwrap();

        assert_eq!(output.document.default_tab_width_twips, 480);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("default tab width clamped") })
        );
    }

    #[test]
    fn recovers_extreme_line_spacing_controls_as_automatic() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_line_spacing_twips: 720,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\sl99999 Too much spacing\par}", &options)
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.line_spacing_twips, None);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("using automatic line spacing"))
        );
    }

    #[test]
    fn normalizes_paragraph_spacing_controls() {
        let output = parse_rtf(r"{\rtf1\sb240\sa360 Spaced\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.space_before_twips, 240);
        assert_eq!(paragraph.style.space_after_twips, 360);
    }

    #[test]
    fn strict_spec_normalizes_paragraph_auto_spacing_controls() {
        let options = RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\sb0\sbauto\sa0\saauto Auto\par\sbauto0\saauto0 Manual\par}",
            &options,
        )
        .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected first paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected second paragraph"),
        };

        assert_eq!(first.style.space_before_twips, 0);
        assert_eq!(first.style.space_after_twips, 0);
        assert!(first.style.auto_space_before);
        assert!(first.style.auto_space_after);
        assert!(!second.style.auto_space_before);
        assert!(!second.style.auto_space_after);
    }

    #[test]
    fn normalizes_contextual_paragraph_spacing_control() {
        let output =
            parse_rtf(r"{\rtf1\contextualspace Same style\par\contextualspace0 Normal\par}")
                .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected first paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected second paragraph"),
        };

        assert!(first.style.contextual_spacing);
        assert!(!second.style.contextual_spacing);
    }

    #[test]
    fn clamps_extreme_paragraph_spacing_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_paragraph_spacing_twips: 480,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\sb99999\sa99999 Too much spacing\par}",
            &options,
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.space_before_twips, 480);
        assert_eq!(paragraph.style.space_after_twips, 480);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("paragraph space before clamped")
        }));
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("paragraph space after clamped"))
        );
    }

    #[test]
    fn word_picture_tone_uses_signed_brightness_and_half_sample_contrast_center() {
        let sample = |value, brightness_fixed, contrast_fixed| {
            adjust_picture_sample(
                value,
                PictureAdjustments {
                    brightness_fixed,
                    contrast_fixed,
                    ..PictureAdjustments::default()
                },
            )
        };

        assert_eq!(sample(0, Some(16_384), None), 128);
        assert_eq!(sample(128, Some(-16_384), None), 1);
        assert_eq!(sample(64, None, Some(131_072)), 1);
        assert_eq!(sample(128, None, Some(131_072)), 129);
        assert_eq!(sample(0, None, Some(32_768)), 64);
        assert_eq!(sample(255, None, Some(32_768)), 191);
        assert_eq!(sample(0, Some(16_384), Some(131_072)), 64);
        assert_eq!(sample(32, Some(16_384), Some(131_072)), 128);
        assert_eq!(sample(0, Some(32_768), Some(131_072)), 255);
    }

    #[test]
    fn word_picture_adjustments_preserve_alpha_and_match_visible_pixels() {
        let output = parse_rtf(include_str!(
            "../../../fixtures/picture-adjustments-passive.rtf"
        ))
        .expect("picture adjustment fixture should parse");
        let images = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Image(image) => Some(image),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(images.len(), 3);
        assert!(images.iter().all(|image| image.alpha_mask.is_some()));
        let grayscale =
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&images[0].bytes, 7)
                .expect("grayscale scanline");
        let bilevel = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&images[1].bytes, 7)
            .expect("bilevel scanline");
        let tone = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&images[2].bytes, 7)
            .expect("tone scanline");

        assert_eq!(grayscale, vec![0, 0, 0, 0, 18, 18, 18]);
        assert_eq!(bilevel, vec![0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(tone, vec![0, 0, 0, 0, 255, 255, 255]);
    }

    #[test]
    fn normalizes_header_picture_as_safe_repeating_image_metadata() {
        let input = format!(
            "{{\\rtf1{{\\header Logo {{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}\\par}}Body\\par}}",
            bytes_to_hex(&minimal_rgb_png_with_dimensions(1, 1))
        );
        let output = parse_rtf(&input).unwrap();

        assert_eq!(output.document.header.len(), 1);
        assert_eq!(output.document.header[0].runs[0].text, "Logo ");
        assert_eq!(output.document.header_images.len(), 1);
        assert!(
            output
                .document
                .blocks
                .iter()
                .all(|block| !matches!(block, Block::Image(_)))
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_picture_natural_size_hints_as_safe_layout_metadata() {
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picw80\\pich40 {}}}}}",
            bytes_to_hex(&minimal_rgb_png_with_dimensions(1, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Png);
        assert_eq!(image.width_px, 1);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.natural_width_px_hint, Some(80));
        assert_eq!(image.natural_height_px_hint, Some(40));
        assert_eq!(image.display_width_twips, None);
        assert_eq!(image.display_height_twips, None);
    }

    #[test]
    fn clamps_extreme_picture_scaling_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                min_image_scaling_percent: 10,
                max_image_scaling_percent: 250,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picscalex0\\picscaley9999 {}}}}}",
            bytes_to_hex(&minimal_rgb_png_with_dimensions(1, 1))
        );
        let output = parse_rtf_bytes_with_options(input.as_bytes(), &options).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.scale_x_percent, Some(10));
        assert_eq!(image.scale_y_percent, Some(250));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("picture horizontal scaling clamped")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("picture vertical scaling clamped")
        }));
    }

    #[test]
    fn clamps_extreme_picture_display_goal_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_image_display_twips: 720,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal999999\\pichgoal-99 {}}}}}",
            bytes_to_hex(&minimal_rgb_png_with_dimensions(1, 1))
        );
        let output = parse_rtf_bytes_with_options(input.as_bytes(), &options).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(0));
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("picture display width clamped") })
        );
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("picture display height clamped")
        }));
    }

    #[test]
    fn clamps_extreme_picture_natural_size_hints() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_image_dimension_hint_px: 10,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picw999999\\pich-99 {}}}}}",
            bytes_to_hex(&minimal_rgb_png_with_dimensions(1, 1))
        );
        let output = parse_rtf_bytes_with_options(input.as_bytes(), &options).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.width_px, 1);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.natural_width_px_hint, Some(10));
        assert_eq!(image.natural_height_px_hint, None);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("picture natural width clamped") })
        );
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("picture natural height clamped")
        }));
    }

    #[test]
    fn normalizes_picture_crop_controls_as_safe_metadata() {
        let input = format!(
            "{{\\rtf1{{\\pict\\jpegblip\\piccropl120\\piccropt240\\piccropr360\\piccropb480 {}}}}}",
            bytes_to_hex(&minimal_jpeg_with_dimensions(1, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(
            image.crop,
            ImageCrop {
                left_twips: 120,
                top_twips: 240,
                right_twips: 360,
                bottom_twips: 480,
            }
        );
    }

    #[test]
    fn clamps_extreme_picture_crop_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_image_crop_twips: 240,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let input = format!(
            "{{\\rtf1{{\\pict\\jpegblip\\piccropl9999\\piccropt-9999 {}}}}}",
            bytes_to_hex(&minimal_jpeg_with_dimensions(1, 1))
        );
        let output = parse_rtf_bytes_with_options(input.as_bytes(), &options).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.crop.left_twips, 240);
        assert_eq!(image.crop.top_twips, -240);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("picture left crop clamped") })
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("picture top crop clamped") })
        );
    }

    #[test]
    fn unsupported_word_picture_formats_become_placeholders() {
        for control in ["wmetafile8", "emfblip", "macpict", "pmmetafile1"] {
            let input = format!(
                r"{{\rtf1{{\pict\{control}\picw100\pich50\picwgoal720\pichgoal360 41424344}}}}"
            );
            let output = parse_rtf(&input).unwrap();

            let image = match &output.document.blocks[0] {
                Block::Image(image) => image,
                _ => panic!("expected passive image placeholder"),
            };
            assert_eq!(image.format, ImageFormat::Placeholder);
            assert!(image.bytes.is_empty());
            assert!(image.palette.is_empty());
            assert_eq!(image.natural_width_px_hint, Some(100));
            assert_eq!(image.natural_height_px_hint, Some(50));
            assert_eq!(image.display_width_twips, Some(720));
            assert_eq!(image.display_height_twips, Some(360));
            assert!(
                output
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("unsupported picture format"))
            );
        }
    }
