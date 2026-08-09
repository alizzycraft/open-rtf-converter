    #[test]
    fn passive_advance_marker_offsets_following_text_without_rendering_marker() {
        fn position_for_after(runs: Vec<Run>) -> (f32, f32, bool) {
            let mut document = Document::default();
            document.blocks = vec![Block::Paragraph(Paragraph {
                style: Default::default(),
                runs,
            })];
            let layout = LayoutEngine::layout(&document);
            let mut marker_rendered = false;
            let mut after_x = None;
            let mut after_y = None;
            for item in &layout.pages[0].items {
                if let LayoutItem::Text(fragment) = item {
                    if matches!(fragment.dynamic_field, Some(InlineKind::LayoutAdvance(_))) {
                        marker_rendered = true;
                    }
                    if fragment.text == "After" {
                        after_x = Some(fragment.x);
                        after_y = Some(fragment.baseline_y);
                    }
                }
            }
            (
                after_x.expect("After text"),
                after_y.expect("After text baseline"),
                marker_rendered,
            )
        }

        let base_runs = vec![
            Run {
                dynamic_field: None,
                text: "Before ".to_string(),
                style: Default::default(),
            },
            Run {
                dynamic_field: None,
                text: "After".to_string(),
                style: Default::default(),
            },
        ];
        let advanced_runs = vec![
            Run {
                dynamic_field: None,
                text: "Before ".to_string(),
                style: Default::default(),
            },
            Run {
                dynamic_field: Some(InlineKind::LayoutAdvance(LayoutAdvance {
                    horizontal_twips: 240,
                    vertical_twips: -120,
                })),
                text: String::new(),
                style: CharacterStyle::default(),
            },
            Run {
                dynamic_field: None,
                text: "After".to_string(),
                style: Default::default(),
            },
        ];

        let (base_x, base_y, _) = position_for_after(base_runs);
        let (advanced_x, advanced_y, marker_rendered) = position_for_after(advanced_runs);

        assert!(!marker_rendered);
        assert!((advanced_x - base_x - 12.0).abs() < 0.01);
        assert!((base_y - advanced_y - 6.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_section_page_settings_on_following_page() {
        let mut document = Document::default();
        let mut second_page = PageSettings::default();
        second_page.width_twips = 10_080;
        second_page.height_twips = 7_200;
        second_page.margin_left_twips = 720;
        second_page.margin_top_twips = 720;
        second_page.gutter_twips = 360;
        document.blocks = vec![
            paragraph_with_text("Before"),
            Block::SectionBreak,
            Block::SectionSettings(second_page),
            paragraph_with_text("After"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert_eq!(layout.pages[0].width, 612.0);
        assert_eq!(layout.pages[0].height, 792.0);
        assert_eq!(layout.pages[1].width, 504.0);
        assert_eq!(layout.pages[1].height, 360.0);
        assert!(
            layout.pages[1].items.iter().any(
                |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "After" && (fragment.x - 54.0).abs() < 0.01)
            )
        );
    }

    #[test]
    fn lays_out_section_page_borders_on_later_pages() {
        let mut document = Document::default();
        let mut second_section = PageSettings::default();
        second_section.page_borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 60,
            color_index: None,
            style: BorderStyle::Dotted,
            ..TableCellBorder::default()
        };
        document.blocks = vec![
            paragraph_with_text("First section"),
            Block::SectionBreak,
            Block::SectionSettings(second_section),
            paragraph_with_text("Second section"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert!(!layout.pages[0].items.iter().any(|item| matches!(
            item,
            LayoutItem::Line {
                style: LineStyle::Dotted,
                ..
            }
        )));
        assert!(layout.pages[1].items.iter().any(|item| matches!(
            item,
            LayoutItem::Line {
                style: LineStyle::Dotted,
                ..
            }
        )));
    }

    #[test]
    fn resolves_repeating_header_page_numbers_per_page() {
        let mut document = Document::default();
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: dynamic_field_runs("", DynamicField::PageNumber, Default::default()),
        }];
        document.blocks.clear();
        for _ in 0..120 {
            document.blocks.push(Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    dynamic_field: None,
                    text: "Body paragraph.".to_string(),
                    style: Default::default(),
                }],
            }));
        }

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages.len() > 1);
        assert!(layout_text(&layout.pages[0]).contains('1'));
        assert!(layout_text(&layout.pages[1]).contains('2'));
    }

    #[test]
    fn resolves_page_number_markers_from_configured_start() {
        let mut document = Document::default();
        document.page.page_number_start = Some(7);
        document.page.restart_page_numbering = true;
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: dynamic_field_runs("Page ", DynamicField::PageNumber, Default::default()),
        }];
        document.blocks = vec![
            paragraph_with_text("Page one"),
            Block::PageBreak,
            paragraph_with_text("Page two"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert!(layout_text(&layout.pages[0]).contains("Page 7"));
        assert!(layout_text(&layout.pages[1]).contains("Page 8"));
        assert!(!layout_text(&layout.pages[0]).contains("Page 1"));
        assert!(!layout_text(&layout.pages[1]).contains("Page 2"));
    }

    #[test]
    fn resolves_page_number_markers_from_configured_format() {
        let mut document = Document::default();
        document.page.page_number_start = Some(4);
        document.page.restart_page_numbering = true;
        document.page.page_number_format = Some(PageNumberFormat::UpperRoman);
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: dynamic_field_runs("Page ", DynamicField::PageNumber, Default::default()),
        }];
        document.blocks = vec![
            paragraph_with_text("Page one"),
            Block::PageBreak,
            paragraph_with_text("Page two"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert!(layout_text(&layout.pages[0]).contains("Page IV"));
        assert!(layout_text(&layout.pages[1]).contains("Page V"));
        assert!(!layout_text(&layout.pages[0]).contains("Page 4"));
        assert!(!layout_text(&layout.pages[1]).contains("Page 5"));
    }

    #[test]
    fn keeps_header_page_number_in_header_and_emits_legacy_auto_page_number() {
        let mut document = Document::default();
        document.page.page_number_x_twips = Some(360);
        document.page.page_number_y_twips = Some(1_440);
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: dynamic_field_runs("Page ", DynamicField::PageNumber, Default::default()),
        }];
        document.blocks = vec![paragraph_with_text("Body")];

        let layout = LayoutEngine::layout(&document);
        let fragment = text_fragment_for(&layout.pages[0], "Page ");
        let legacy_number = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "1" => Some(fragment),
                _ => None,
            })
            .max_by(|left, right| left.x.total_cmp(&right.x))
            .expect("legacy page number");

        assert!((fragment.x - 72.0).abs() < 0.01);
        assert!((fragment.baseline_y - 744.75).abs() < 0.01);
        assert!(legacy_number.x > 540.0);
        assert!((legacy_number.baseline_y - 708.75).abs() < 0.01);
    }

    #[test]
    fn positions_header_total_page_count_from_safe_page_number_coordinates() {
        let mut document = Document::default();
        document.page.page_number_x_twips = Some(360);
        document.page.page_number_y_twips = Some(1_440);
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: dynamic_field_runs("Pages ", DynamicField::TotalPages, Default::default()),
        }];
        document.blocks = vec![
            paragraph_with_text("Page one"),
            Block::PageBreak,
            paragraph_with_text("Page two"),
        ];

        let layout = LayoutEngine::layout(&document);
        let fragment = text_fragment_for(&layout.pages[0], "Pages ");

        assert!((fragment.x - 72.0).abs() < 0.01);
        assert!((fragment.baseline_y - 744.75).abs() < 0.01);
        assert!(layout_text(&layout.pages[0]).contains("Pages 2"));
    }

    #[test]
    fn positions_header_section_page_count_from_safe_page_number_coordinates() {
        let mut document = Document::default();
        document.page.page_number_x_twips = Some(360);
        document.page.page_number_y_twips = Some(1_440);
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: dynamic_field_runs("Section pages ", DynamicField::SectionPages, Default::default()),
        }];
        document.blocks = vec![
            paragraph_with_text("First body page one"),
            Block::PageBreak,
            paragraph_with_text("First body page two"),
            Block::SectionBreak,
            paragraph_with_text("Second section"),
        ];

        let layout = LayoutEngine::layout(&document);
        let fragment = text_fragment_for(&layout.pages[0], "Section ");

        assert!((fragment.x - 72.0).abs() < 0.01);
        assert!((fragment.baseline_y - 744.75).abs() < 0.01);
        assert!(layout_text(&layout.pages[0]).contains("Section pages 2"));
    }

    #[test]
    fn resolves_page_number_markers_from_section_start() {
        let mut document = Document::default();
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: dynamic_field_runs("Page ", DynamicField::PageNumber, Default::default()),
        }];
        let mut second_section = PageSettings::default();
        second_section.page_number_start = Some(3);
        second_section.restart_page_numbering = true;
        document.blocks = vec![
            paragraph_with_text("First section"),
            Block::SectionBreak,
            Block::SectionSettings(second_section),
            paragraph_with_text("Second section"),
            Block::PageBreak,
            paragraph_with_text("Second section page two"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 3);
        assert!(layout_text(&layout.pages[0]).contains("Page 1"));
        assert!(layout_text(&layout.pages[1]).contains("Page 3"));
        assert!(layout_text(&layout.pages[2]).contains("Page 4"));
        assert!(!layout_text(&layout.pages[1]).contains("Page 2"));
    }

    #[test]
    fn resolves_page_number_markers_from_continued_section_numbering() {
        let mut document = Document::default();
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: dynamic_field_runs("Page ", DynamicField::PageNumber, Default::default()),
        }];
        document.blocks = vec![
            paragraph_with_text("First section"),
            Block::PageBreak,
            paragraph_with_text("First section page two"),
            Block::SectionBreak,
            Block::SectionSettings(PageSettings::default()),
            paragraph_with_text("Second section continues"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 3);
        assert!(layout_text(&layout.pages[0]).contains("Page 1"));
        assert!(layout_text(&layout.pages[1]).contains("Page 2"));
        assert!(layout_text(&layout.pages[2]).contains("Page 3"));
        assert!(!layout_text(&layout.pages[2]).contains("Page 1"));
    }

    #[test]
    fn resolves_page_number_markers_from_section_format() {
        let mut document = Document::default();
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: dynamic_field_runs("Page ", DynamicField::PageNumber, Default::default()),
        }];
        let mut second_section = PageSettings::default();
        second_section.page_number_start = Some(2);
        second_section.restart_page_numbering = true;
        second_section.page_number_format = Some(PageNumberFormat::LowerLetter);
        document.blocks = vec![
            paragraph_with_text("First section"),
            Block::SectionBreak,
            Block::SectionSettings(second_section),
            paragraph_with_text("Second section"),
            Block::PageBreak,
            paragraph_with_text("Second section page two"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 3);
        assert!(layout_text(&layout.pages[0]).contains("Page 1"));
        assert!(layout_text(&layout.pages[1]).contains("Page b"));
        assert!(layout_text(&layout.pages[2]).contains("Page c"));
        assert!(!layout_text(&layout.pages[1]).contains("Page 2"));
        assert!(!layout_text(&layout.pages[2]).contains("Page 3"));
    }

    #[test]
    fn late_total_page_marker_updates_passive_underline_width() {
        let mut document = small_test_page_document();
        let mut style = CharacterStyle::default();
        style.underline = UnderlineStyle::Single;
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: dynamic_field_runs("Pages ", DynamicField::TotalPages, style.clone()),
        }];
        document.blocks.clear();
        for idx in 0..80 {
            document
                .blocks
                .push(paragraph_with_text(&format!("Body {idx}")));
        }

        let layout = LayoutEngine::layout(&document);
        let page_text = layout_text(&layout.pages[0]);
        assert!(
            layout.pages.len() >= 10,
            "fixture should cross into two-digit page count, got {} page(s)",
            layout.pages.len()
        );
        let expected = format!("Pages {}", layout.pages.len());
        assert!(
            page_text.contains(&expected),
            "header text was {page_text:?}"
        );
        let resolved_marker = layout.pages.len().to_string();
        let expected_width =
            measure_text_with_family(&resolved_marker, &style, PdfFontFamily::Helvetica);
        let old_width = measure_text_with_family(
            LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER,
            &style,
            PdfFontFamily::Helvetica,
        );

        let underline_width =
            underline_width_for_fragment_after(&layout.pages[0], "Pages ", &resolved_marker);
        assert!(
            (underline_width - expected_width).abs() < 0.01,
            "late NUMPAGES underline width should match resolved text width"
        );
        assert!(
            (underline_width - old_width).abs() > 1.0,
            "late NUMPAGES underline should not keep marker placeholder width"
        );
    }

    #[test]
    fn late_total_page_marker_updates_word_underline_width() {
        let mut document = small_test_page_document();
        let mut style = CharacterStyle::default();
        style.underline = UnderlineStyle::Words;
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: dynamic_field_runs("Pages ", DynamicField::TotalPages, style.clone()),
        }];
        document.blocks.clear();
        for idx in 0..80 {
            document
                .blocks
                .push(paragraph_with_text(&format!("Body {idx}")));
        }

        let layout = LayoutEngine::layout(&document);
        assert!(
            layout.pages.len() >= 10,
            "fixture should cross into two-digit page count, got {} page(s)",
            layout.pages.len()
        );
        let resolved_marker = layout.pages.len().to_string();
        let expected_width =
            measure_text_with_family(&resolved_marker, &style, PdfFontFamily::Helvetica);
        let old_width = measure_text_with_family(
            LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER,
            &style,
            PdfFontFamily::Helvetica,
        );

        let underline_width =
            underline_width_for_fragment_after(&layout.pages[0], "Pages ", &resolved_marker);
        assert!(
            (underline_width - expected_width).abs() < 0.01,
            "late NUMPAGES word underline width should match resolved text width"
        );
        assert!(
            (underline_width - old_width).abs() > 1.0,
            "late NUMPAGES word underline should not keep marker placeholder width"
        );
    }

    #[test]
    fn late_total_page_marker_updates_character_border_width() {
        let mut document = small_test_page_document();
        let mut style = CharacterStyle::default();
        style.border.visible = true;
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: dynamic_field_runs("Pages ", DynamicField::TotalPages, style.clone()),
        }];
        document.blocks.clear();
        for idx in 0..80 {
            document
                .blocks
                .push(paragraph_with_text(&format!("Body {idx}")));
        }

        let layout = LayoutEngine::layout(&document);
        assert!(
            layout.pages.len() >= 10,
            "fixture should cross into two-digit page count, got {} page(s)",
            layout.pages.len()
        );
        let resolved_marker = layout.pages.len().to_string();
        let fragment = text_fragment_after(&layout.pages[0], "Pages ", &resolved_marker);
        let expected_width =
            measure_text_with_family(&resolved_marker, &style, PdfFontFamily::Helvetica);
        let old_width = measure_text_with_family(
            LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER,
            &style,
            PdfFontFamily::Helvetica,
        );
        let pad = character_border_test_pad(&style);
        let right_edge = character_border_right_edge_for_fragment(
            &layout.pages[0],
            fragment,
            fragment.x + expected_width + pad,
        );

        assert!(
            (right_edge - (fragment.x + expected_width + pad)).abs() < 0.01,
            "late NUMPAGES character border should match resolved text width: right={right_edge}, expected={}",
            fragment.x + expected_width + pad
        );
        assert!(
            (right_edge - (fragment.x + old_width + pad)).abs() > 1.0,
            "late NUMPAGES character border should not keep marker placeholder width"
        );
    }

    #[test]
    fn late_total_page_marker_shifts_following_same_line_text() {
        let mut document = small_test_page_document();
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    dynamic_field: None,
                    text: "Pages ".to_string(),
                    style: Default::default(),
                },
                Run {
                    dynamic_field: Some(InlineKind::DynamicField(DynamicField::TotalPages)),
                    text: String::new(),
                    style: Default::default(),
                },
                Run {
                    dynamic_field: None,
                    text: " total".to_string(),
                    style: Default::default(),
                },
            ],
        }];
        document.blocks.clear();
        for idx in 0..80 {
            document
                .blocks
                .push(paragraph_with_text(&format!("Body {idx}")));
        }

        let layout = LayoutEngine::layout(&document);
        assert!(
            layout.pages.len() >= 10,
            "fixture should cross into two-digit page count, got {} page(s)",
            layout.pages.len()
        );
        let resolved_marker = layout.pages.len().to_string();
        let marker = text_fragment_after(&layout.pages[0], "Pages ", &resolved_marker);
        let suffix = text_fragment_after(&layout.pages[0], &resolved_marker, " ");
        let expected_width = measure_text_with_family(
            &resolved_marker,
            &CharacterStyle::default(),
            PdfFontFamily::Helvetica,
        );
        let old_width = measure_text_with_family(
            LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER,
            &CharacterStyle::default(),
            PdfFontFamily::Helvetica,
        );

        assert!(
            (suffix.x - (marker.x + expected_width)).abs() < 0.01,
            "late NUMPAGES following text should start after resolved marker text"
        );
        assert!(
            (suffix.x - (marker.x + old_width)).abs() > 1.0,
            "late NUMPAGES following text should not keep marker placeholder position"
        );
    }

    #[test]
    fn late_section_page_marker_updates_passive_underline_width() {
        let mut document = small_test_page_document();
        let mut style = CharacterStyle::default();
        style.underline = UnderlineStyle::Single;
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: dynamic_field_runs("Section pages ", DynamicField::SectionPages, style.clone()),
        }];
        document.blocks.clear();
        for idx in 0..80 {
            document
                .blocks
                .push(paragraph_with_text(&format!("First section body {idx}")));
        }
        document.blocks.push(Block::SectionBreak);
        document
            .blocks
            .push(Block::SectionSettings(PageSettings::default()));
        document.blocks.push(paragraph_with_text("Second section"));

        let layout = LayoutEngine::layout(&document);
        let first_section_pages = layout
            .pages
            .iter()
            .filter(|page| page.section_number == 1)
            .count();
        assert!(
            first_section_pages >= 10,
            "fixture should cross into two-digit section page count, got {first_section_pages}"
        );
        let resolved_marker = first_section_pages.to_string();
        let expected = format!("Section pages {resolved_marker}");
        let page_text = layout_text(&layout.pages[0]);
        assert!(
            page_text.contains(&expected),
            "header text was {page_text:?}"
        );
        let expected_width =
            measure_text_with_family(&resolved_marker, &style, PdfFontFamily::Helvetica);
        let old_width = measure_text_with_family(
            LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER,
            &style,
            PdfFontFamily::Helvetica,
        );

        let underline_width =
            underline_width_for_fragment_after(&layout.pages[0], "pages ", &resolved_marker);
        assert!(
            (underline_width - expected_width).abs() < 0.01,
            "late SECTIONPAGES underline width should match resolved text width"
        );
        assert!(
            (underline_width - old_width).abs() > 1.0,
            "late SECTIONPAGES underline should not keep marker placeholder width"
        );
    }

    #[test]
    fn late_section_page_marker_updates_word_underline_width() {
        let mut document = small_test_page_document();
        let mut style = CharacterStyle::default();
        style.underline = UnderlineStyle::Words;
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: dynamic_field_runs("Section pages ", DynamicField::SectionPages, style.clone()),
        }];
        document.blocks.clear();
        for idx in 0..80 {
            document
                .blocks
                .push(paragraph_with_text(&format!("First section body {idx}")));
        }
        document.blocks.push(Block::SectionBreak);
        document
            .blocks
            .push(Block::SectionSettings(PageSettings::default()));
        document.blocks.push(paragraph_with_text("Second section"));

        let layout = LayoutEngine::layout(&document);
        let first_section_pages = layout
            .pages
            .iter()
            .filter(|page| page.section_number == 1)
            .count();
        assert!(
            first_section_pages >= 10,
            "fixture should cross into two-digit section page count, got {first_section_pages}"
        );
        let resolved_marker = first_section_pages.to_string();
        let expected_width =
            measure_text_with_family(&resolved_marker, &style, PdfFontFamily::Helvetica);
        let old_width = measure_text_with_family(
            LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER,
            &style,
            PdfFontFamily::Helvetica,
        );

        let underline_width =
            underline_width_for_fragment_after(&layout.pages[0], "pages ", &resolved_marker);
        assert!(
            (underline_width - expected_width).abs() < 0.01,
            "late SECTIONPAGES word underline width should match resolved text width"
        );
        assert!(
            (underline_width - old_width).abs() > 1.0,
            "late SECTIONPAGES word underline should not keep marker placeholder width"
        );
    }

    #[test]
    fn late_section_page_marker_updates_character_border_width() {
        let mut document = small_test_page_document();
        let mut style = CharacterStyle::default();
        style.border.visible = true;
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: dynamic_field_runs("Section pages ", DynamicField::SectionPages, style.clone()),
        }];
        document.blocks.clear();
        for idx in 0..80 {
            document
                .blocks
                .push(paragraph_with_text(&format!("First section body {idx}")));
        }
        document.blocks.push(Block::SectionBreak);
        document
            .blocks
            .push(Block::SectionSettings(PageSettings::default()));
        document.blocks.push(paragraph_with_text("Second section"));

        let layout = LayoutEngine::layout(&document);
        let first_section_pages = layout
            .pages
            .iter()
            .filter(|page| page.section_number == 1)
            .count();
        assert!(
            first_section_pages >= 10,
            "fixture should cross into two-digit section page count, got {first_section_pages}"
        );
        let resolved_marker = first_section_pages.to_string();
        let fragment = text_fragment_after(&layout.pages[0], "Section pages ", &resolved_marker);
        let expected_width =
            measure_text_with_family(&resolved_marker, &style, PdfFontFamily::Helvetica);
        let old_width = measure_text_with_family(
            LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER,
            &style,
            PdfFontFamily::Helvetica,
        );
        let pad = character_border_test_pad(&style);
        let right_edge = character_border_right_edge_for_fragment(
            &layout.pages[0],
            fragment,
            fragment.x + expected_width + pad,
        );

        assert!(
            (right_edge - (fragment.x + expected_width + pad)).abs() < 0.01,
            "late SECTIONPAGES character border should match resolved text width: right={right_edge}, expected={}",
            fragment.x + expected_width + pad
        );
        assert!(
            (right_edge - (fragment.x + old_width + pad)).abs() > 1.0,
            "late SECTIONPAGES character border should not keep marker placeholder width"
        );
    }

    #[test]
    fn late_section_page_marker_shifts_following_same_line_text() {
        let mut document = small_test_page_document();
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    dynamic_field: None,
                    text: "Section pages ".to_string(),
                    style: Default::default(),
                },
                Run {
                    dynamic_field: Some(InlineKind::DynamicField(DynamicField::SectionPages)),
                    text: String::new(),
                    style: Default::default(),
                },
                Run {
                    dynamic_field: None,
                    text: " total".to_string(),
                    style: Default::default(),
                },
            ],
        }];
        document.blocks.clear();
        for idx in 0..80 {
            document
                .blocks
                .push(paragraph_with_text(&format!("First section body {idx}")));
        }
        document.blocks.push(Block::SectionBreak);
        document
            .blocks
            .push(Block::SectionSettings(PageSettings::default()));
        document.blocks.push(paragraph_with_text("Second section"));

        let layout = LayoutEngine::layout(&document);
        let first_section_pages = layout
            .pages
            .iter()
            .filter(|page| page.section_number == 1)
            .count();
        assert!(
            first_section_pages >= 10,
            "fixture should cross into two-digit section page count, got {first_section_pages}"
        );
        let resolved_marker = first_section_pages.to_string();
        let marker = text_fragment_after(&layout.pages[0], "pages ", &resolved_marker);
        let suffix = text_fragment_after(&layout.pages[0], &resolved_marker, " ");
        let expected_width = measure_text_with_family(
            &resolved_marker,
            &CharacterStyle::default(),
            PdfFontFamily::Helvetica,
        );
        let old_width = measure_text_with_family(
            LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER,
            &CharacterStyle::default(),
            PdfFontFamily::Helvetica,
        );

        assert!(
            (suffix.x - (marker.x + expected_width)).abs() < 0.01,
            "late SECTIONPAGES following text should start after resolved marker text"
        );
        assert!(
            (suffix.x - (marker.x + old_width)).abs() > 1.0,
            "late SECTIONPAGES following text should not keep marker placeholder position"
        );
    }
    fn dynamic_field_runs(
        prefix: &str,
        field: DynamicField,
        style: CharacterStyle,
    ) -> Vec<Run> {
        let mut runs = Vec::new();
        if !prefix.is_empty() {
            runs.push(Run {
                dynamic_field: None,
                text: prefix.to_owned(),
                style: style.clone(),
            });
        }
        runs.push(Run {
            dynamic_field: Some(InlineKind::DynamicField(field)),
            text: String::new(),
            style,
        });
        runs
    }
