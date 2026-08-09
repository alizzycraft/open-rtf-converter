    #[test]
    fn paragraph_start_images_consume_only_their_authored_paragraph() {
        let provider = FontProvider::browser_safe_defaults();
        let after_baseline_from_page_top = |rtf: &str, needle: &str| {
            let parsed = parse_rtf(rtf).expect("image fixture should parse");
            assert_eq!(parsed.document.inline_images.len(), 1);
            assert_eq!(parsed.document.inline_images.len(), 1);
            let layout = LayoutEngine::layout_with_font_provider(&parsed.document, Some(&provider));
            let after = layout.pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(text) if text.text.contains(needle) => Some(text),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing text starting with {needle:?}"));
            layout.pages[0].height - after.baseline_y
        };

        let alpha =
            after_baseline_from_page_top(include_str!("../../fixtures/png-alpha.rtf"), "After");
        let scaled = after_baseline_from_page_top(
            include_str!("../../fixtures/picture-scaling-passive.rtf"),
            "After",
        );

        assert!((alpha - 132.98).abs() < 0.03, "alpha after={alpha}");
        assert!((scaled - 156.98).abs() < 0.03, "scaled after={scaled}");
    }

    #[test]
    fn applies_homogeneous_nested_vertical_alignment_to_authored_height() {
        fn first_text_baseline(input: &str) -> f32 {
            let parsed = crate::rtf::parse_rtf(input).unwrap();
            LayoutEngine::layout(&parsed.document)
                .pages
                .iter()
                .flat_map(|page| page.items.iter())
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.contains('A') => {
                        Some(fragment.baseline_y)
                    }
                    _ => None,
                })
                .expect("nested text baseline")
        }

        let top = first_text_baseline(
            r"{\rtf1\trowd\cellx6000 Outer {\trowd\itap2\trrh2880\cellx2000 A\nestcell\cellx4000 B\nestrow}\cell\row}",
        );
        let centered = first_text_baseline(
            r"{\rtf1\trowd\cellx6000 Outer {\trowd\itap2\trrh2880\clvertalc\cellx2000 A\nestcell\clvertalc\cellx4000 B\nestrow}\cell\row}",
        );
        assert!(
            centered < top - 20.0,
            "centered nested row did not move within authored height: top={top}, centered={centered}"
        );
    }

    #[test]
    fn keeps_nested_keep_together_paragraph_lines_in_one_split_fragment() {
        let mut protected_style = ParagraphStyle::default();
        protected_style.keep_together = true;
        let line = |paragraph_index: usize, height: f32, style: ParagraphStyle| PreparedCellLine {
            line: Line {
                runs: Vec::new(),
                width: 0.0,
                natural_height: height,
                height,
                segmented_flow: false,
                inline_image_baseline_context: false,
            },
            style,
            paragraph_index,
            text_direction: TableCellTextDirection::LeftToRightTopToBottom,
            is_first_line: true,
            is_last_line: true,
            space_before: 0.0,
            space_after: 0.0,
        };
        let mut remaining = PreparedTableRow {
            visual_cells: vec![VisualTableCell {
                cell_index: 0,
                x_offset: 0.0,
                width: 100.0,
            }],
            cell_lines: vec![vec![
                line(0, 10.0, ParagraphStyle::default()),
                line(1, 10.0, protected_style.clone()),
                line(1, 10.0, protected_style),
            ]],
            cell_paddings: vec![ResolvedCellPadding {
                left: 0.0,
                right: 0.0,
                top: 0.0,
                bottom: 0.0,
            }],
            cell_spacings: vec![ResolvedCellSpacing {
                left: 0.0,
                right: 0.0,
                top: 0.0,
                bottom: 0.0,
            }],
            top_border_inset: 0.0,
            cell_text_directions: vec![TableCellTextDirection::LeftToRightTopToBottom],
            row_height: 30.0,
        };

        let fragment = split_prepared_table_row_fragment(&mut remaining, 25.0)
            .expect("fragment should contain the unprotected line");
        assert_eq!(fragment.cell_lines[0].len(), 1);
        assert_eq!(remaining.cell_lines[0].len(), 2);
        assert!(
            remaining.cell_lines[0]
                .iter()
                .all(|line| line.style.keep_together)
        );
    }

    #[test]
    fn keeps_nested_keep_with_next_paragraph_groups_in_one_split_fragment() {
        let mut keep_next_style = ParagraphStyle::default();
        keep_next_style.keep_with_next = true;
        let line = |paragraph_index: usize, style: ParagraphStyle| PreparedCellLine {
            line: Line {
                runs: Vec::new(),
                width: 0.0,
                natural_height: 10.0,
                height: 10.0,
                segmented_flow: false,
                inline_image_baseline_context: false,
            },
            style,
            paragraph_index,
            text_direction: TableCellTextDirection::LeftToRightTopToBottom,
            is_first_line: true,
            is_last_line: true,
            space_before: 0.0,
            space_after: 0.0,
        };
        let mut remaining = PreparedTableRow {
            visual_cells: vec![VisualTableCell {
                cell_index: 0,
                x_offset: 0.0,
                width: 100.0,
            }],
            cell_lines: vec![vec![
                line(0, ParagraphStyle::default()),
                line(1, keep_next_style),
                line(2, ParagraphStyle::default()),
            ]],
            cell_paddings: vec![ResolvedCellPadding {
                left: 0.0,
                right: 0.0,
                top: 0.0,
                bottom: 0.0,
            }],
            cell_spacings: vec![ResolvedCellSpacing {
                left: 0.0,
                right: 0.0,
                top: 0.0,
                bottom: 0.0,
            }],
            top_border_inset: 0.0,
            cell_text_directions: vec![TableCellTextDirection::LeftToRightTopToBottom],
            row_height: 30.0,
        };

        let fragment = split_prepared_table_row_fragment(&mut remaining, 25.0)
            .expect("fragment should contain the preceding line");
        assert_eq!(fragment.cell_lines[0].len(), 1);
        assert_eq!(remaining.cell_lines[0].len(), 2);
        assert_eq!(remaining.cell_lines[0][0].paragraph_index, 1);
        assert_eq!(remaining.cell_lines[0][1].paragraph_index, 2);
    }

    #[test]
    fn decimal_tabs_without_separator_fall_back_to_right_alignment() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.tab_stops_twips = vec![1440];
        paragraph_style.tab_stop_alignments = vec![TabAlignment::Decimal];
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "Amount\t123".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let amount = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "123" => Some(fragment),
                _ => None,
            })
            .expect("decimal fallback tab text");
        let width = measure_text_with_family(&amount.text, &amount.style, PdfFontFamily::Helvetica);

        assert!((amount.x + width - 144.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_rtl_paragraph_direction_as_right_alignment() {
        let mut right_style = ParagraphStyle::default();
        right_style.alignment = Alignment::Right;
        let mut document = Document::default();
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: right_style,
                runs: vec![Run {
                    text: "RTL".to_string(),
                    style: Default::default(),
                }],
            }),
            paragraph_with_text("LTR"),
        ];

        let layout = LayoutEngine::layout(&document);
        let right_x = text_x(&layout.pages[0], "RTL").expect("right text");
        let left_x = text_x(&layout.pages[0], "LTR").expect("left text");

        assert!(right_x > left_x + 200.0);
    }

    #[test]
    fn keep_with_next_preflights_the_complete_paragraph_chain() {
        let mut document = small_test_page_document();
        let mut keep_next_style = ParagraphStyle::default();
        keep_next_style.keep_with_next = true;
        let kept = |text: &str| {
            Block::Paragraph(Paragraph {
                style: keep_next_style.clone(),
                runs: vec![Run {
                    text: text.to_string(),
                    style: Default::default(),
                }],
            })
        };
        document.blocks = (0..7)
            .map(|_| paragraph_with_text("Filler"))
            .chain([
                kept("Keep chain one"),
                kept("Keep chain two"),
                paragraph_with_text("Keep chain three"),
            ])
            .collect();

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert!(!layout_text(&layout.pages[0]).contains("Keep chain"));
        assert_eq!(
            layout_text(&layout.pages[1]),
            "Keep chain oneKeep chain twoKeep chain three"
        );
    }
