    #[test]
    fn inline_image_line_uses_bounded_word_body_edge_tolerance() {
        let parsed = parse_rtf(include_str!("../../fixtures/wmf-dashed-pen-passive.rtf")).unwrap();
        let layout = LayoutEngine::layout(&parsed.document);
        let page = &layout.pages[0];
        let after = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(text) if text.text.starts_with("After") => Some(text),
                _ => None,
            })
            .expect("text after inline image");
        let dashed = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(text) if text.text.starts_with("dashed") && text.x > after.x => {
                    Some(text)
                }
                _ => None,
            })
            .expect("trailing text near body edge");

        assert!(
            (after.baseline_y - dashed.baseline_y).abs() < 0.01,
            "Word keeps trailing text on the inline-image line: after={}, dashed={}",
            after.baseline_y,
            dashed.baseline_y
        );
    }

    #[test]
    fn inline_image_establishes_word_baselines_for_later_paragraph_lines() {
        let parsed = parse_rtf(include_str!("../../fixtures/wmf-vector-passive.rtf")).unwrap();
        let provider = FontProvider::browser_safe_defaults();
        let layout = LayoutEngine::layout_with_font_provider(&parsed.document, Some(&provider));
        let page = &layout.pages[0];
        let before = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(text) if text.text.starts_with("Before") => Some(text),
                _ => None,
            })
            .expect("text before inline WMF");
        let after = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(text) if text.text.starts_with("After") => Some(text),
                _ => None,
            })
            .expect("text on inline WMF line");
        let trailing = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(text)
                    if text.text.starts_with("vector") && text.baseline_y < after.baseline_y =>
                {
                    Some(text)
                }
                _ => None,
            })
            .expect("text after inline WMF line");

        assert!((page.height - before.baseline_y - 47.30).abs() < 0.03);
        assert!((page.height - after.baseline_y - 85.82).abs() < 0.03);
        assert!((page.height - trailing.baseline_y - 99.62).abs() < 0.03);
    }

    #[test]
    fn justifies_non_final_paragraph_lines_with_passive_word_spacing() {
        let mut document = Document::default();
        document.page.width_twips = 5_000;
        document.page.margin_left_twips = 720;
        document.page.margin_right_twips = 720;
        let mut style = ParagraphStyle::default();
        style.alignment = Alignment::Justified;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style,
            runs: vec![Run {
                dynamic_field: None,
                text: "one two three four five six seven eight nine ten".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let word_spacings = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment.word_spacing),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(
            word_spacings.iter().any(|spacing| *spacing > 0.0),
            "expected at least one non-final justified line to add word spacing: {word_spacings:?}"
        );
        assert!(
            word_spacings.contains(&0.0),
            "expected final justified line to remain unstretched: {word_spacings:?}"
        );
    }

    #[test]
    fn distributes_final_paragraph_line_with_passive_character_spacing() {
        let mut document = Document::default();
        document.page.width_twips = 5_000;
        document.page.margin_left_twips = 720;
        document.page.margin_right_twips = 720;
        let mut style = ParagraphStyle::default();
        style.alignment = Alignment::Distributed;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style,
            runs: vec![Run {
                dynamic_field: None,
                text: "one two three four five six seven eight nine ten".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let fragments = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(fragments.len() >= 2);
        assert!(
            fragments[..fragments.len() - 1]
                .iter()
                .any(|fragment| fragment.word_spacing > 0.0)
        );
        assert!(
            fragments
                .last()
                .is_some_and(|fragment| fragment.style.character_spacing_twips > 0)
        );
    }

    #[test]
    fn lays_out_strikeout_and_baseline_shift() {
        let mut document = Document::default();
        let mut shifted_style = CharacterStyle::default();
        shifted_style.baseline_shift_half_points = 8;
        let mut struck_style = CharacterStyle::default();
        struck_style.strike = true;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    dynamic_field: None,
                    text: "base".to_string(),
                    style: Default::default(),
                },
                Run {
                    dynamic_field: None,
                    text: "up".to_string(),
                    style: shifted_style,
                },
                Run {
                    dynamic_field: None,
                    text: "strike".to_string(),
                    style: struck_style,
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let base_y = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "base" => Some(fragment.baseline_y),
                _ => None,
            })
            .expect("base text");
        let up_y = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "up" => Some(fragment.baseline_y),
                _ => None,
            })
            .expect("shifted text");

        assert!(
            up_y > base_y,
            "expected shifted baseline above base baseline, base_y={base_y}, up_y={up_y}"
        );
        assert!(
            page.items
                .iter()
                .any(|item| matches!(item, LayoutItem::Line { .. }))
        );
    }

    #[test]
    fn lays_out_overline_as_passive_line_above_text() {
        let mut overline_style = CharacterStyle::default();
        overline_style.overline = true;
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                dynamic_field: None,
                text: "over".to_string(),
                style: overline_style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let fragment = text_fragment_for(&layout.pages[0], "over");
        let line_y = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Line { y1, y2, .. } if (*y1 - *y2).abs() < 0.01 => Some(*y1),
                _ => None,
            })
            .expect("overline stroke");

        assert!(
            line_y > fragment.baseline_y,
            "overline should sit above text baseline, line_y={line_y}, baseline={}",
            fragment.baseline_y
        );
    }

    #[test]
    fn lays_out_word_underline_variants_as_passive_lines() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 0,
                green: 0,
                blue: 0,
            },
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
        ];
        let mut double_style = CharacterStyle::default();
        double_style.underline = UnderlineStyle::Double;
        double_style.underline_color_index = Some(2);
        let mut wave_style = CharacterStyle::default();
        wave_style.underline = UnderlineStyle::Wave;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    dynamic_field: None,
                    text: "double".to_string(),
                    style: double_style,
                },
                Run {
                    dynamic_field: None,
                    text: "wave".to_string(),
                    style: wave_style,
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let underlines = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Underline {
                    style,
                    color,
                    width,
                    ..
                } => Some((*style, *color, *width)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(underlines.len(), 2);
        assert_eq!(underlines[0].0, UnderlineStyle::Double);
        assert_eq!(underlines[0].1.red, 1.0);
        assert_eq!(underlines[1].0, UnderlineStyle::Wave);
        assert!(underlines.iter().all(|(_, _, width)| *width > 0.0));
    }

    #[test]
    fn lays_out_word_only_underline_as_separate_word_segments() {
        let mut document = Document::default();
        let mut style = CharacterStyle::default();
        style.underline = UnderlineStyle::Words;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                dynamic_field: None,
                text: "Two words".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let underlines = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Underline {
                    x, width, style, ..
                } => Some((*x, *width, *style)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(underlines.len(), 2);
        assert_eq!(underlines[0].2, UnderlineStyle::Single);
        assert_eq!(underlines[1].2, UnderlineStyle::Single);
        assert!(underlines[0].1 > 0.0);
        assert!(underlines[1].1 > 0.0);
        assert!(
            underlines[1].0 > underlines[0].0 + underlines[0].1,
            "expected a visible skipped gap between word underline segments: {underlines:?}"
        );
    }

    #[test]
    fn lays_out_border_styles_as_passive_line_styles() {
        let mut document = Document::default();
        let mut character_style = CharacterStyle::default();
        character_style.border = TableCellBorder {
            visible: true,
            width_twips: 40,
            color_index: None,
            style: BorderStyle::Dashed,
            ..TableCellBorder::default()
        };
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 60,
            color_index: None,
            style: BorderStyle::Double,
            ..TableCellBorder::default()
        };
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                dynamic_field: None,
                text: "Styled".to_string(),
                style: character_style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let line_styles = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Line { style, .. } => Some(*style),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(line_styles.contains(&LineStyle::Double));
        assert!(line_styles.contains(&LineStyle::Dashed));

        let mut triple_paragraph = ParagraphStyle::default();
        triple_paragraph.borders.top = TableCellBorder {
            visible: true,
            width_twips: 60,
            color_index: None,
            style: BorderStyle::Triple,
            ..TableCellBorder::default()
        };
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: triple_paragraph,
            runs: vec![Run {
                dynamic_field: None,
                text: "Triple".to_string(),
                style: CharacterStyle::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        assert!(layout.pages[0].items.iter().any(|item| matches!(
            item,
            LayoutItem::Line {
                style: LineStyle::Triple,
                ..
            }
        )));
    }

    #[test]
    fn lays_out_wavy_borders_as_passive_line_styles() {
        let mut document = Document::default();
        let mut character_style = CharacterStyle::default();
        character_style.border = TableCellBorder {
            visible: true,
            width_twips: 40,
            color_index: None,
            style: BorderStyle::Wavy,
            ..TableCellBorder::default()
        };
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 60,
            color_index: None,
            style: BorderStyle::Wavy,
            ..TableCellBorder::default()
        };
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                dynamic_field: None,
                text: "Wavy".to_string(),
                style: character_style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let wavy_lines = layout.pages[0]
            .items
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    LayoutItem::Line {
                        style: LineStyle::Wavy,
                        ..
                    }
                )
            })
            .count();

        assert!(wavy_lines >= 2);
    }

    #[test]
    fn lays_out_paragraph_border_spacing_as_passive_offset_lines() {
        let spaced = crate::rtf::parse_rtf(r"{\rtf1\brdrb\brdrs\brsp240 Spaced\par}").unwrap();
        let unspaced = crate::rtf::parse_rtf(r"{\rtf1\brdrb\brdrs Plain\par}").unwrap();

        let spaced_layout = LayoutEngine::layout(&spaced.document);
        let unspaced_layout = LayoutEngine::layout(&unspaced.document);
        let bottom_line_y = |page: &LayoutPage| {
            page.items
                .iter()
                .filter_map(|item| match item {
                    LayoutItem::Line { y1, y2, .. } if (*y1 - *y2).abs() < 0.01 => Some(*y1),
                    _ => None,
                })
                .min_by(|a, b| a.total_cmp(b))
                .expect("bottom paragraph border")
        };

        assert!(
            bottom_line_y(&spaced_layout.pages[0])
                < bottom_line_y(&unspaced_layout.pages[0]) - 10.0
        );
    }

    #[test]
    fn lays_out_character_border_spacing_as_larger_passive_box() {
        let spaced = crate::rtf::parse_rtf(r"{\rtf1\chbrdr\brdrs\brsp240 Boxed\par}").unwrap();
        let unspaced = crate::rtf::parse_rtf(r"{\rtf1\chbrdr\brdrs Boxed\par}").unwrap();

        let spaced_layout = LayoutEngine::layout(&spaced.document);
        let unspaced_layout = LayoutEngine::layout(&unspaced.document);
        let box_height = |page: &LayoutPage| {
            let ys = page
                .items
                .iter()
                .filter_map(|item| match item {
                    LayoutItem::Line { y1, y2, .. } => Some((*y1, *y2)),
                    LayoutItem::Highlight { y, height, .. } => Some((*y, *y + *height)),
                    _ => None,
                })
                .flat_map(|(y1, y2)| [y1, y2])
                .collect::<Vec<_>>();
            let min_y = ys
                .iter()
                .copied()
                .min_by(|a, b| a.total_cmp(b))
                .expect("min y");
            let max_y = ys
                .iter()
                .copied()
                .max_by(|a, b| a.total_cmp(b))
                .expect("max y");
            max_y - min_y
        };

        assert!(box_height(&spaced_layout.pages[0]) > box_height(&unspaced_layout.pages[0]) + 20.0);
    }

    #[test]
    fn consecutive_hyphenation_limit_never_reintroduces_synthetic_hyphens() {
        let document = Document::default();
        let style = CharacterStyle::default();
        let word = "AntidisestablishmentarianismAntidisestablishmentarianism";
        let mut unlimited_style = ParagraphStyle::default();
        unlimited_style.auto_hyphenation = true;
        let mut limited_style = unlimited_style.clone();
        limited_style.max_consecutive_hyphenated_lines = Some(1);
        let unlimited = Paragraph {
            style: unlimited_style,
            runs: vec![Run {
                dynamic_field: None,
                text: word.to_string(),
                style: style.clone(),
            }],
        };
        let limited = Paragraph {
            style: limited_style,
            runs: unlimited.runs.clone(),
        };

        let markers = test_markers("1", "1");
        let width = measure_text("Antidis", &style);
        let unlimited_lines = wrap_paragraph(&unlimited, width, &markers, &document);
        let limited_lines = wrap_paragraph(&limited, width, &markers, &document);
        for lines in [&unlimited_lines, &limited_lines] {
            assert!(lines.len() > 1);
            assert!(lines.iter().all(|line| line.width <= width + 0.01));
            assert!(lines.iter().all(|line| !line_text(line).contains('-')));
            assert_eq!(lines.iter().map(line_text).collect::<String>(), word);
        }
    }

    #[test]
    fn character_spacing_expands_measured_line_width() {
        let mut expanded = CharacterStyle::default();
        expanded.character_spacing_twips = 200;
        let plain_width = measure_text("ABC", &CharacterStyle::default());
        let expanded_width = measure_text("ABC", &expanded);

        assert!((expanded_width - plain_width - 20.0).abs() < 0.01);

        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                dynamic_field: None,
                text: "ABC".to_string(),
                style: expanded,
            }],
        })];
        let layout = LayoutEngine::layout(&document);
        let fragment = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "ABC" => Some(fragment),
                _ => None,
            })
            .expect("expanded text");

        assert_eq!(fragment.style.character_spacing_twips, 200);
    }

    #[test]
    fn character_spacing_from_prior_run_advances_the_following_run_boundary() {
        let following_x = |spacing_twips| {
            let mut spaced = CharacterStyle::default();
            spaced.character_spacing_twips = spacing_twips;
            let mut document = Document::default();
            document.blocks = vec![Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![
                    Run {
                        dynamic_field: None,
                        text: "ABCD".to_string(),
                        style: spaced,
                    },
                    Run {
                        dynamic_field: None,
                        text: "EFGH".to_string(),
                        style: Default::default(),
                    },
                ],
            })];
            let layout = LayoutEngine::layout(&document);
            layout.pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text == "EFGH" => Some(fragment.x),
                    _ => None,
                })
                .expect("following run")
        };
        let plain_width = measure_text("ABCD", &CharacterStyle::default());
        let mut expanded = CharacterStyle::default();
        expanded.character_spacing_twips = 20;
        let expanded_internal_width = measure_text("ABCD", &expanded);
        let line_origin = 72.0;

        assert!((following_x(0) - line_origin - plain_width).abs() < 0.01);
        assert!((following_x(20) - line_origin - expanded_internal_width - 1.0).abs() < 0.01);
        let mut condensed = CharacterStyle::default();
        condensed.character_spacing_twips = -20;
        let condensed_internal_width = measure_text("ABCD", &condensed);
        assert!((following_x(-20) - line_origin - condensed_internal_width + 1.0).abs() < 0.01);
    }

    #[test]
    fn character_scaling_changes_measured_line_width() {
        let mut expanded = CharacterStyle::default();
        expanded.character_scaling_percent = 150;
        let plain_width = measure_text("ABC", &CharacterStyle::default());
        let expanded_width = measure_text("ABC", &expanded);

        assert!((expanded_width - (plain_width * 1.5)).abs() < 0.01);

        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                dynamic_field: None,
                text: "ABC".to_string(),
                style: expanded,
            }],
        })];
        let layout = LayoutEngine::layout(&document);
        let fragment = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "ABC" => Some(fragment),
                _ => None,
            })
            .expect("scaled text");

        assert_eq!(fragment.style.character_scaling_percent, 150);
    }

    #[test]
    fn fit_text_width_scales_run_to_bounded_passive_target() {
        let target_twips = 360;
        let target_width = twips_to_points(target_twips);
        let mut fit_style = CharacterStyle::default();
        fit_style.fit_text_width_twips = Some(target_twips);
        let raw_width = measure_text("FittedText", &CharacterStyle::default());

        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                dynamic_field: None,
                text: "FittedText".to_string(),
                style: fit_style,
            }],
        })];
        let layout = LayoutEngine::layout(&document);
        let fragment = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "FittedText" => Some(fragment),
                _ => None,
            })
            .expect("fit-text fragment");
        let rendered_width = measure_text("FittedText", &fragment.style);

        assert!(fragment.style.character_scaling_percent < 100);
        assert!((rendered_width - target_width).abs() < 0.75);
        assert!(rendered_width < raw_width);
    }

    #[test]
    fn fit_text_width_scales_contiguous_runs_as_one_word_region() {
        let target_twips = 720;
        let target_width = twips_to_points(target_twips);
        let mut first_style = CharacterStyle::default();
        first_style.fit_text_width_twips = Some(target_twips);
        first_style.character_scaling_percent = 125;
        let mut second_style = CharacterStyle::default();
        second_style.fit_text_width_twips = Some(target_twips);

        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    dynamic_field: None,
                    text: "Visible styled typography".to_string(),
                    style: first_style,
                },
                Run {
                    dynamic_field: None,
                    text: "After typography.".to_string(),
                    style: second_style,
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let fragments = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment),
                _ => None,
            })
            .collect::<Vec<_>>();
        let rendered_width = fragments
            .iter()
            .map(|fragment| measure_text(&fragment.text, &fragment.style))
            .sum::<f32>();

        assert_eq!(fragments.len(), 5);
        assert!((rendered_width - target_width).abs() < 0.75);
        let first_scale = fragments[0].style.character_scaling_percent;
        let second_scale = fragments[3].style.character_scaling_percent;
        assert!(
            fragments[..3]
                .iter()
                .all(|fragment| fragment.style.character_scaling_percent == first_scale)
        );
        assert!(
            fragments[3..]
                .iter()
                .all(|fragment| fragment.style.character_scaling_percent == second_scale)
        );
        assert!((first_scale as f32 / second_scale as f32 - 1.25).abs() < 0.05);
    }

    #[test]
    fn lays_out_first_line_indent_only_on_first_body_line() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.left_indent_twips = 720;
        paragraph_style.first_line_indent_twips = 360;
        document_with_first_line_indent_body_test(paragraph_style);
    }

    #[test]
    fn lays_out_hanging_first_line_indent() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.left_indent_twips = 720;
        paragraph_style.first_line_indent_twips = -360;
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                dynamic_field: None,
                text: "First\nSecond".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let text_x = |text: &str| {
            layout.pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.trim() == text => Some(fragment.x),
                    _ => None,
                })
                .expect("body text")
        };

        assert!((text_x("First") - 90.0).abs() < 0.01);
        assert!((text_x("Second") - 108.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_right_indent_as_reduced_line_width() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.right_indent_twips = 7_200;
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                dynamic_field: None,
                text: "Alpha Bravo Charlie Delta Echo Foxtrot".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let baselines = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment.baseline_y),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(
            baselines
                .windows(2)
                .any(|pair| (pair[0] - pair[1]).abs() > 0.01)
        );
    }

    #[test]
    fn lays_out_paragraph_spacing_before_and_after() {
        let mut first_style = ParagraphStyle::default();
        first_style.space_after_twips = 400;
        let mut second_style = ParagraphStyle::default();
        second_style.space_before_twips = 200;
        let spaced_gap = paragraph_baseline_gap(first_style, second_style);
        let default_gap =
            paragraph_baseline_gap(ParagraphStyle::default(), ParagraphStyle::default());

        assert!((spaced_gap - default_gap - 30.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_auto_paragraph_spacing_with_bounded_passive_gap() {
        let mut first_style = ParagraphStyle::default();
        first_style.space_after_twips = 0;
        first_style.auto_space_after = true;
        let mut second_style = ParagraphStyle::default();
        second_style.space_before_twips = 0;
        second_style.auto_space_before = true;
        let auto_gap = paragraph_baseline_gap(first_style, second_style);

        let mut manual_first = ParagraphStyle::default();
        manual_first.space_after_twips = 0;
        let mut manual_second = ParagraphStyle::default();
        manual_second.space_before_twips = 0;
        let manual_gap = paragraph_baseline_gap(manual_first, manual_second);

        assert!(
            (auto_gap - manual_gap - 24.0).abs() < 0.01,
            "expected auto spacing to add bounded 24pt gap, manual={manual_gap}, auto={auto_gap}"
        );
    }

    #[test]
    fn collapses_contextual_spacing_between_adjacent_opt_in_paragraphs() {
        let mut normal_first = ParagraphStyle::default();
        normal_first.space_after_twips = 360;
        let mut normal_second = ParagraphStyle::default();
        normal_second.space_before_twips = 240;
        let normal_gap = paragraph_baseline_gap(normal_first.clone(), normal_second.clone());

        normal_first.contextual_spacing = true;
        normal_second.contextual_spacing = true;
        let contextual_gap = paragraph_baseline_gap(normal_first, normal_second);

        assert!(
            (normal_gap - contextual_gap - 30.0).abs() < 0.01,
            "contextual spacing should suppress 18pt after + 12pt before, normal={normal_gap}, contextual={contextual_gap}"
        );
    }

    #[test]
    fn contextual_spacing_collapses_a_boundary_when_either_paragraph_opts_in() {
        let mut first_style = ParagraphStyle::default();
        first_style.space_after_twips = 360;
        let mut second_style = ParagraphStyle::default();
        second_style.space_before_twips = 240;
        let normal_gap = paragraph_baseline_gap(first_style.clone(), second_style.clone());

        first_style.contextual_spacing = true;
        let previous_opt_in_gap = paragraph_baseline_gap(first_style, second_style.clone());
        second_style.contextual_spacing = true;
        let next_opt_in_gap = paragraph_baseline_gap(
            ParagraphStyle {
                space_after_twips: 360,
                ..ParagraphStyle::default()
            },
            second_style,
        );

        assert!((normal_gap - previous_opt_in_gap - 30.0).abs() < 0.01);
        assert!((normal_gap - next_opt_in_gap - 30.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_dot_tab_leaders_as_passive_text() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.tab_stops_twips = vec![1440];
        paragraph_style.tab_stop_leaders = vec![TabLeader::Dots];
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                dynamic_field: None,
                text: "Left\tRight".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        let leader = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text.starts_with("...") => Some(fragment),
                _ => None,
            })
            .expect("dot leader");
        assert!(
            (leader.x - 94.08).abs() < 0.01,
            "unexpected dot leader x {}",
            leader.x
        );
        assert_eq!(leader.style.character_scaling_percent, 101);
        assert!(page.items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Right" && (fragment.x - 144.0).abs() < 0.01)
        ));
    }

    #[test]
    fn word_dot_tab_leaders_snap_to_bounded_page_grid() {
        let mut document = Document::default();
        document.page.margin_left_twips = 720;
        document.blocks = [1440, 2160, 2880, 3600, 4320]
            .into_iter()
            .map(|stop| {
                let mut style = ParagraphStyle::default();
                style.tab_stops_twips = vec![stop];
                style.tab_stop_leaders = vec![TabLeader::Dots];
                style.tab_stop_alignments = vec![TabAlignment::Right];
                Block::Paragraph(Paragraph {
                    style,
                    runs: vec![Run {
                        dynamic_field: None,
                        text: "A\tX".to_string(),
                        style: CharacterStyle {
                            font_size_half_points: 24,
                            ..CharacterStyle::default()
                        },
                    }],
                })
            })
            .collect();

        let layout = LayoutEngine::layout(&document);
        let leaders = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment)
                    if !fragment.text.is_empty() && fragment.text.chars().all(|ch| ch == '.') =>
                {
                    Some(fragment)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            leaders
                .iter()
                .map(|leader| leader.text.len())
                .collect::<Vec<_>>(),
            vec![15, 26, 37, 47, 58]
        );
        assert!(leaders.iter().all(|leader| (leader.x - 47.04).abs() < 0.01));
    }

    #[test]
    fn word_tab_leader_variants_snap_to_measured_page_grids() {
        let paragraph = |label: &str, leader| {
            let mut style = ParagraphStyle::default();
            style.tab_stops_twips = vec![2880];
            style.tab_stop_leaders = vec![leader];
            style.tab_stop_alignments = vec![TabAlignment::Right];
            Block::Paragraph(Paragraph {
                style,
                runs: vec![Run {
                    dynamic_field: None,
                    text: format!("{label}\tX"),
                    style: CharacterStyle {
                        font_size_half_points: 24,
                        ..CharacterStyle::default()
                    },
                }],
            })
        };
        let mut document = Document::default();
        document.page.margin_left_twips = 720;
        document.blocks = vec![
            paragraph("Hyphen A", TabLeader::Hyphens),
            paragraph("Underline A", TabLeader::Underline),
            paragraph("Thick A", TabLeader::Underline),
            paragraph("Middle A", TabLeader::MiddleDots),
        ];

        let layout = LayoutEngine::layout(&document);
        let leaders = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment)
                    if !fragment.text.is_empty()
                        && fragment
                            .text
                            .chars()
                            .all(|ch| matches!(ch, '-' | '_' | '\u{00b7}')) =>
                {
                    Some(fragment)
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(leaders.len(), 4);
        for (leader, text, x, scaling) in [
            (leaders[0], "-".repeat(20), 91.08, 99),
            (leaders[1], "_".repeat(10), 100.8, 101),
            (leaders[2], "_".repeat(13), 80.64, 101),
            (leaders[3], "\u{00b7}".repeat(22), 83.16, 119),
        ] {
            assert_eq!(leader.text, text);
            assert!(
                (leader.x - x).abs() < 0.01,
                "unexpected leader x {}",
                leader.x
            );
            assert_eq!(leader.style.character_scaling_percent, scaling);
        }
        assert!(
            layout.pages[0]
                .items
                .iter()
                .all(|item| !matches!(item, LayoutItem::Line { .. }))
        );
    }

    #[test]
    fn lays_out_middle_dot_and_equals_tab_leaders_as_passive_text() {
        let paragraph = |leader| {
            let mut style = ParagraphStyle::default();
            style.tab_stops_twips = vec![1440];
            style.tab_stop_leaders = vec![leader];
            Block::Paragraph(Paragraph {
                style,
                runs: vec![Run {
                    dynamic_field: None,
                    text: "Left\tRight".to_string(),
                    style: Default::default(),
                }],
            })
        };
        let mut document = Document::default();
        document.blocks = vec![
            paragraph(TabLeader::MiddleDots),
            paragraph(TabLeader::Equals),
        ];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text.starts_with('\u{00b7}'))
        ));
        assert!(page.items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text.starts_with("==="))
        ));
    }

    #[test]
    fn lays_out_default_tab_width_from_document_settings() {
        let mut document = Document::default();
        document.default_tab_width_twips = 360;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: ParagraphStyle::default(),
            runs: vec![Run {
                dynamic_field: None,
                text: "A\tB".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!((text_x(page, "B").expect("tabbed text") - 90.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_right_aligned_tab_stops() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.tab_stops_twips = vec![1440];
        paragraph_style.tab_stop_leaders = vec![TabLeader::Dots];
        paragraph_style.tab_stop_alignments = vec![TabAlignment::Right];
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                dynamic_field: None,
                text: "Left\t9".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let digit = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "9" => Some(fragment),
                _ => None,
            })
            .expect("right-aligned tab text");
        let digit_width =
            measure_text_with_family(&digit.text, &digit.style, PdfFontFamily::Helvetica);

        assert!((digit.x + digit_width - 144.0).abs() < 0.01);
        assert!(page.items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text.starts_with("..."))
        ));
    }

    #[test]
    fn lays_out_bar_tab_stops_as_passive_vertical_lines() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.tab_stops_twips = vec![720, 1440];
        paragraph_style.tab_stop_alignments = vec![TabAlignment::Bar, TabAlignment::Left];
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                dynamic_field: None,
                text: "Left\tRight".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, x2, .. } if (*x1 - 108.0).abs() < 0.01 && (*x2 - 108.0).abs() < 0.01
        )));
        assert!(
            (text_x(page, "Right").expect("tabbed text") - 144.0).abs() < 0.01,
            "bar tab should not consume the normal left tab stop"
        );
    }

    #[test]
    fn lays_out_header_bar_tab_stops_as_passive_vertical_lines() {
        let mut header_style = ParagraphStyle::default();
        header_style.tab_stops_twips = vec![720, 1440];
        header_style.tab_stop_alignments = vec![TabAlignment::Bar, TabAlignment::Left];
        let mut document = Document::default();
        document.header = vec![Paragraph {
            style: header_style,
            runs: vec![Run {
                dynamic_field: None,
                text: "Head\tRight".to_string(),
                style: Default::default(),
            }],
        }];
        document.blocks = vec![paragraph_with_text("Body")];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, x2, .. } if (*x1 - 108.0).abs() < 0.01 && (*x2 - 108.0).abs() < 0.01
        )));
        assert!(
            (text_x(page, "Right").expect("header tabbed text") - 144.0).abs() < 0.01,
            "header bar tab should not consume the normal left tab stop"
        );
    }

    #[test]
    fn lays_out_center_aligned_tab_stops() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.tab_stops_twips = vec![1440];
        paragraph_style.tab_stop_alignments = vec![TabAlignment::Center];
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                dynamic_field: None,
                text: "Left\tMM".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let centered = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "MM" => Some(fragment),
                _ => None,
            })
            .expect("center-aligned tab text");
        let text_width =
            measure_text_with_family(&centered.text, &centered.style, PdfFontFamily::Helvetica);

        assert!((centered.x + (text_width / 2.0) - 144.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_decimal_aligned_tab_stops() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.tab_stops_twips = vec![1440];
        paragraph_style.tab_stop_leaders = vec![TabLeader::Dots];
        paragraph_style.tab_stop_alignments = vec![TabAlignment::Decimal];
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                dynamic_field: None,
                text: "Amount\t123.45".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let amount = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "123.45" => Some(fragment),
                _ => None,
            })
            .expect("decimal-aligned tab text");
        let prefix_width = measure_text_with_family("123", &amount.style, PdfFontFamily::Helvetica);

        assert!((amount.x + prefix_width - 144.0).abs() < 0.01);
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Text(fragment)
                if !fragment.text.is_empty() && fragment.text.chars().all(|ch| ch == '.')
        )));
    }

    #[test]
    fn caller_font_metrics_drive_line_height_without_system_fonts() {
        let mut document = Document::default();
        document.fonts = vec![FontDef {
            index: 0,
            name: "Tuffy".to_string(),
            alternate_name: None,
            charset: None,
            code_page: None,
            family: FontFamilyHint::Modern,
            pitch: FontPitch::Fixed,
        }];
        let style = CharacterStyle {
            font_size_half_points: 40,
            ..Default::default()
        };
        let paragraph = Paragraph {
            style: Default::default(),
            runs: vec![Run {
                dynamic_field: None,
                text: "A".to_string(),
                style: style.clone(),
            }],
        };
        let provider = FontProvider {
            assets: vec![FontAsset {
                family_names: vec!["Tuffy".to_string()],
                style: FontAssetStyle::default(),
                bytes: include_bytes!("../../fixtures/fonts/Tuffy.ttf").to_vec(),
            }],
            limits: FontProviderLimits {
                max_asset_bytes: 256 * 1024,
                max_total_bytes: 256 * 1024,
                ..FontProviderLimits::default()
            },
        };
        provider.validate().unwrap();
        let markers = test_markers("1", "1");
        let fallback_line =
            wrap_paragraph_with_font_provider(&paragraph, 500.0, &markers, &document, None)
                .into_iter()
                .next()
                .expect("fallback line");
        let supplied_line = wrap_paragraph_with_font_provider(
            &paragraph,
            500.0,
            &markers,
            &document,
            Some(&provider),
        )
        .into_iter()
        .next()
        .expect("supplied line");
        let expected_height = provider
            .glyph_metrics_for_char("Tuffy", 'A')
            .and_then(|metrics| metrics.line_height_points(style.font_size_points()))
            .expect("line metrics")
            .clamp(
                style.font_size_points().max(1.0),
                style.font_size_points().max(1.0) * 2.0,
            )
            .max(empty_line().height);

        assert!((supplied_line.height - expected_height).abs() < 0.01);
        assert!(
            (supplied_line.height - fallback_line.height).abs() > 0.1,
            "provided metrics should visibly differ from fallback line height"
        );
    }

    #[test]
    fn calibri_uses_bounded_source_line_metrics_with_bundled_fonts() {
        let mut document = Document::default();
        document.fonts = vec![FontDef {
            index: 0,
            name: "Calibri".to_string(),
            alternate_name: None,
            charset: None,
            code_page: None,
            family: FontFamilyHint::Swiss,
            pitch: FontPitch::Default,
        }];
        let paragraph = Paragraph {
            style: Default::default(),
            runs: vec![Run {
                dynamic_field: None,
                text: "Calibri line".to_string(),
                style: CharacterStyle {
                    font_index: 0,
                    font_size_half_points: 24,
                    ..Default::default()
                },
            }],
        };
        let markers = test_markers("1", "1");
        let line = wrap_paragraph_with_font_provider(
            &paragraph,
            500.0,
            &markers,
            &document,
            Some(&FontProvider::default()),
        )
        .into_iter()
        .next()
        .expect("Calibri line");

        assert!((line.height - 14.648_437_5).abs() < 0.001);
        assert!(
            (word_compatible_source_line_baseline_delta_points(
                &line,
                &document,
                Some(&FontProvider::default()),
            ) + (12.0 * WORD_CALIBRI_BASELINE_INSET_EM))
                .abs()
                < 0.001
        );

        let font = &document.fonts[0];
        for (bold, italic, units) in [
            (false, false, 1_185),
            (true, false, 1_241),
            (false, true, 1_185),
            (true, true, 1_241),
        ] {
            let style = CharacterStyle {
                font_index: 0,
                font_size_half_points: 24,
                bold,
                italic,
                ..Default::default()
            };
            let advance =
                source_word_compatible_advance_points(Some(font), &style, 'A', 12.0, None)
                    .expect("Calibri ASCII source advance");
            assert!((advance - (12.0 * units as f32 / 2_048.0)).abs() < 0.001);
        }
        assert!(
            source_word_compatible_advance_points(
                Some(font),
                &CharacterStyle::default(),
                '\u{03a9}',
                12.0,
                None,
            )
            .is_none()
        );
        let regular = CharacterStyle {
            font_index: 0,
            font_size_half_points: 24,
            ..Default::default()
        };
        for (left, right, units) in [('W', 'o', -86.0), ('r', '.', -206.0)] {
            let kerning =
                source_word_compatible_pair_kerning_points(Some(font), &regular, left, right, None)
                    .expect("registered Calibri pair");
            assert!((kerning - (12.0 * units / 2_048.0)).abs() < 0.001);
        }
        assert!(
            source_word_compatible_pair_kerning_points(Some(font), &regular, 'H', 'e', None,)
                .is_none()
        );
        assert!(
            source_word_compatible_pair_kerning_points(
                Some(font),
                &CharacterStyle {
                    bold: true,
                    ..regular
                },
                'W',
                'o',
                None,
            )
            .is_none()
        );
    }

    #[test]
    fn base14_fallback_line_height_is_tighter_than_legacy_uniform_spacing() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Times New Roman".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Roman,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Courier New".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Modern,
                pitch: FontPitch::Fixed,
            },
        ];
        let markers = test_markers("1", "1");
        let provider = FontProvider::default();
        let line_height_for_font = |font_index| {
            let paragraph = Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    dynamic_field: None,
                    text: "Line".to_string(),
                    style: CharacterStyle {
                        font_index,
                        font_size_half_points: 40,
                        ..Default::default()
                    },
                }],
            };
            wrap_paragraph_with_font_provider(
                &paragraph,
                500.0,
                &markers,
                &document,
                Some(&provider),
            )
            .into_iter()
            .next()
            .expect("line")
            .height
        };

        assert!((line_height_for_font(0) - 23.0).abs() < 0.01);
        assert!((line_height_for_font(1) - 22.4).abs() < 0.01);
    }

    #[test]
    fn times_run_fragments_use_base14_widths_for_word_spacing() {
        let mut document = Document::default();
        document.fonts = vec![FontDef {
            index: 0,
            name: "Times New Roman".to_string(),
            alternate_name: None,
            charset: None,
            code_page: None,
            family: FontFamilyHint::Roman,
            pitch: FontPitch::Default,
        }];
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                dynamic_field: None,
                text: "Here is a brief Times New Roman text.".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let times = text_x(&layout.pages[0], "Times ").expect("Times fragment");
        let new = text_x(&layout.pages[0], "New ").expect("New fragment");
        let roman = text_x(&layout.pages[0], "Roman ").expect("Roman fragment");

        assert!(
            (new - times - 33.0).abs() < 0.1,
            "Times fragment advanced by {}",
            new - times
        );
        assert!(
            (roman - new - 25.66).abs() < 0.1,
            "New fragment advanced by {}",
            roman - new
        );
    }

    #[test]
    fn passive_checkbox_paragraph_keeps_word_baseline_interval_with_default_provider() {
        let parsed = parse_rtf(include_str!("../../fixtures/wingdings-checkbox-passive.rtf")).unwrap();
        let provider = FontProvider::browser_safe_defaults();
        let layout = LayoutEngine::layout_with_font_provider(&parsed.document, Some(&provider));
        let page = &layout.pages[0];
        let fragment_starting_with = |prefix: &str| {
            page.items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.starts_with(prefix) => {
                        Some(fragment)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing text fragment starting with {prefix}"))
        };
        let before = fragment_starting_with("Before");
        let box_label = fragment_starting_with("Box");
        let field_label = fragment_starting_with("Field");
        let after = fragment_starting_with("After");

        assert!((before.baseline_y - box_label.baseline_y - 13.8).abs() < 0.01);
        assert!(
            (box_label.baseline_y - field_label.baseline_y - 13.8).abs() < 0.01,
            "box={}, field={}",
            box_label.baseline_y,
            field_label.baseline_y
        );
        assert!((field_label.baseline_y - after.baseline_y - 13.8).abs() < 0.01);
    }

    #[test]
    fn stacked_office_math_fraction_uses_word_inline_object_lead_in() {
        let parsed = parse_rtf(include_str!("../../fixtures/office-math-passive.rtf")).unwrap();
        let provider = FontProvider::browser_safe_defaults();
        let layout = LayoutEngine::layout_with_font_provider(&parsed.document, Some(&provider));
        let page = &layout.pages[0];
        let visible_text = page
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let fragment_containing = |text: &str| {
            page.items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.contains(text) => Some(fragment),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing text fragment {text:?}: {visible_text:?}"))
        };

        let numerator = fragment_containing("x");
        let following = fragment_containing("after ");
        assert!(
            (numerator.x - 174.44).abs() < 0.02,
            "numerator x={}",
            numerator.x
        );
        assert!(
            (following.x - 193.51).abs() < 0.02,
            "following x={}",
            following.x
        );
    }

    #[test]
    fn visible_nary_lines_use_word_cambria_math_line_boxes() {
        let parsed = parse_rtf(include_str!(
            "../../fixtures/office-math-nary-visible-passive.rtf"
        ))
        .unwrap();
        let paragraph = parsed
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Paragraph(paragraph) => Some(paragraph),
                _ => None,
            })
            .expect("n-ary paragraph");
        let lines = wrap_paragraph_with_font_provider(
            paragraph,
            234.0,
            &test_markers("1", "1"),
            &parsed.document,
            Some(&FontProvider::browser_safe_defaults()),
        );
        assert_eq!(lines.len(), 2);
        for line in &lines {
            assert!((line.height - (12.0 * 2_396.0 / 2_048.0)).abs() < 0.001);
            assert!((line.natural_height - line.height).abs() < 0.001);
        }
        assert!(lines[0].runs.iter().any(|run| run.text == "∑"));
        assert!(lines[1].runs.iter().any(|run| run.text == "∏"));
    }

    #[test]
    fn multiple_line_spacing_adds_leading_after_the_first_baseline() {
        let layout_with_style = |style: ParagraphStyle| {
            let mut document = Document::default();
            document.blocks = vec![
                Block::Paragraph(Paragraph {
                    style: ParagraphStyle::default(),
                    runs: vec![Run {
                        dynamic_field: None,
                        text: "Before".to_string(),
                        style: Default::default(),
                    }],
                }),
                Block::Paragraph(Paragraph {
                    style,
                    runs: vec![Run {
                        dynamic_field: None,
                        text: "First\nSecond".to_string(),
                        style: Default::default(),
                    }],
                }),
            ];
            LayoutEngine::layout(&document)
        };

        let automatic = layout_with_style(ParagraphStyle::default());
        let mut multiple_style = ParagraphStyle::default();
        multiple_style.line_spacing_twips = Some(480);
        multiple_style.line_spacing_multiple = true;
        let multiple = layout_with_style(multiple_style);
        let automatic_baselines = text_baselines(&automatic.pages[0]);
        let multiple_baselines = text_baselines(&multiple.pages[0]);

        assert_eq!(automatic_baselines.len(), 3);
        assert_eq!(multiple_baselines.len(), 3);
        assert!((automatic_baselines[1] - multiple_baselines[1]).abs() < 0.01);
        assert!((multiple_baselines[1] - multiple_baselines[2] - 30.0).abs() < 0.01);
    }

    #[test]
    fn applies_section_line_grid_to_flowed_paragraph_lines() {
        let mut document = Document::default();
        document.page.text_line_grid_twips = Some(720);
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: ParagraphStyle::default(),
            runs: vec![Run {
                dynamic_field: None,
                text: "First\nSecond".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let baselines = text_baselines(&layout.pages[0]);

        assert_eq!(baselines.len(), 2);
        assert!((baselines[0] - baselines[1] - 36.0).abs() < 0.01);
    }

    #[test]
    fn no_snap_line_grid_paragraphs_use_normal_line_spacing() {
        let mut document = Document::default();
        document.page.text_line_grid_twips = Some(720);
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.snap_to_line_grid = false;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                dynamic_field: None,
                text: "First\nSecond".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let baselines = text_baselines(&layout.pages[0]);

        assert_eq!(baselines.len(), 2);
        assert!((baselines[0] - baselines[1] - 15.0).abs() < 0.01);
    }

    #[test]
    fn advances_tabs_to_explicit_tab_stops() {
        let mut document = Document::default();
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.tab_stops_twips = vec![1440];
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                dynamic_field: None,
                text: "A\tB".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let second_x = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "B" => Some(fragment.x),
                _ => None,
            })
            .expect("text after tab");

        assert!((second_x - 144.0).abs() < 0.01);
    }

    #[test]
    fn page_line_numbering_restarts_after_explicit_page_breaks() {
        let mut document = Document::default();
        document.page.line_numbering.enabled = true;
        document.page.line_numbering.restart = LineNumberRestart::Page;
        document.page.line_numbering.start = 5;
        document.blocks = vec![
            paragraph_with_text("First page"),
            Block::PageBreak,
            paragraph_with_text("Second page"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert_eq!(layout_text(&layout.pages[0]), "5First page");
        assert_eq!(layout_text(&layout.pages[1]), "5Second page");
    }

    #[test]
    fn passive_line_numbers_use_word_default_size_color_and_body_baseline() {
        let mut document = Document::default();
        document.page.line_numbering.enabled = true;
        document.blocks = vec![paragraph_with_text("Body")];

        let layout = LayoutEngine::layout(&document);
        let fragments = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment),
                _ => None,
            })
            .collect::<Vec<_>>();
        let number = fragments
            .iter()
            .find(|fragment| fragment.text == "1")
            .expect("passive line number");
        let body = fragments
            .iter()
            .find(|fragment| fragment.text == "Body")
            .expect("body text");

        assert_eq!(number.style.font_size_half_points, 24);
        assert_eq!(number.color, PdfColor::default());
        assert!((number.baseline_y - body.baseline_y).abs() < 0.01);
    }

    #[test]
    fn continuous_line_numbering_survives_later_section_settings() {
        let mut document = Document::default();
        document.page.line_numbering.enabled = true;
        document.page.line_numbering.restart = LineNumberRestart::Continuous;
        document.page.line_numbering.start = 5;
        let mut second_section = document.page.clone();
        second_section.line_numbering.restart = LineNumberRestart::Continuous;
        document.blocks = vec![
            paragraph_with_text("First section"),
            Block::SectionBreak,
            Block::SectionSettings(second_section),
            paragraph_with_text("Second section"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert_eq!(layout_text(&layout.pages[0]), "5First section");
        assert_eq!(layout_text(&layout.pages[1]), "6Second section");
    }

    #[test]
    fn section_line_numbering_restarts_at_later_section_settings() {
        let mut document = Document::default();
        document.page.line_numbering.enabled = true;
        document.page.line_numbering.restart = LineNumberRestart::Continuous;
        let mut second_section = document.page.clone();
        second_section.line_numbering.restart = LineNumberRestart::Section;
        second_section.line_numbering.start = 7;
        document.blocks = vec![
            paragraph_with_text("First section"),
            Block::SectionBreak,
            Block::SectionSettings(second_section),
            paragraph_with_text("Second section"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert_eq!(layout_text(&layout.pages[0]), "1First section");
        assert_eq!(layout_text(&layout.pages[1]), "7Second section");
    }

    #[test]
    fn lays_out_line_between_columns_as_passive_rule() {
        let mut document = Document::default();
        document.page.column_count = 2;
        document.page.line_between_columns = true;
        document.blocks = vec![
            paragraph_with_text("Left"),
            Block::ColumnBreak,
            paragraph_with_text("Right"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert!(has_vertical_line_at(&layout.pages[0], 306.0));
    }

    #[test]
    fn page_borders_can_measure_spacing_from_page_edge() {
        let mut document = Document::default();
        document.page.width_twips = 5_760;
        document.page.height_twips = 5_760;
        document.page.margin_left_twips = 720;
        document.page.margin_right_twips = 720;
        document.page.margin_top_twips = 720;
        document.page.margin_bottom_twips = 720;
        document.page.page_border_from_page_edge = true;
        document.page.page_border_spacing_twips.left_twips = 120;
        document.page.page_border_spacing_twips.right_twips = 240;
        document.page.page_border_spacing_twips.top_twips = 360;
        document.page.page_border_spacing_twips.bottom_twips = 480;
        document.page.page_borders.top = TableCellBorder {
            visible: true,
            ..TableCellBorder::default()
        };
        document.page.page_borders.bottom = TableCellBorder {
            visible: true,
            ..TableCellBorder::default()
        };
        document.page.page_borders.left = TableCellBorder {
            visible: true,
            ..TableCellBorder::default()
        };
        document.page.page_borders.right = TableCellBorder {
            visible: true,
            ..TableCellBorder::default()
        };
        document.blocks = vec![paragraph_with_text("Body")];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(has_vertical_line_at(page, 6.0));
        assert!(has_vertical_line_at(page, 276.0));
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { y1, y2, .. }
                if (*y1 - 270.0).abs() < 0.01 && (*y2 - 270.0).abs() < 0.01
        )));
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { y1, y2, .. }
                if (*y1 - 24.0).abs() < 0.01 && (*y2 - 24.0).abs() < 0.01
        )));
    }

    #[test]
    fn keep_with_next_preflight_uses_section_line_grid_spacing() {
        let mut document = small_test_page_document();
        document.page.text_line_grid_twips = Some(720);
        let mut keep_next_style = ParagraphStyle::default();
        keep_next_style.keep_with_next = true;
        document.blocks = (0..2)
            .map(|_| paragraph_with_text("Filler"))
            .chain([
                Block::Paragraph(Paragraph {
                    style: keep_next_style,
                    runs: vec![Run {
                        dynamic_field: None,
                        text: "Keep next".to_string(),
                        style: Default::default(),
                    }],
                }),
                paragraph_with_text("Follower"),
            ])
            .collect();

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert_eq!(layout_text(&layout.pages[0]), "FillerFiller");
        assert_eq!(layout_text(&layout.pages[1]), "Keep nextFollower");
    }

    #[test]
    fn widow_control_moves_last_line_with_previous_line() {
        let mut document = small_test_page_document();
        let mut widow_style = ParagraphStyle::default();
        widow_style.widow_control = true;
        document.blocks = (0..6)
            .map(|_| paragraph_with_text("Filler"))
            .chain([Block::Paragraph(Paragraph {
                style: widow_style,
                runs: vec![Run {
                    dynamic_field: None,
                    text: "Alpha\nBeta\nGamma".to_string(),
                    style: Default::default(),
                }],
            })])
            .collect();

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert!(layout_text(&layout.pages[0]).contains("Alpha"));
        assert!(!layout_text(&layout.pages[0]).contains("Beta"));
        assert!(layout_text(&layout.pages[1]).contains("Beta"));
        assert!(layout_text(&layout.pages[1]).contains("Gamma"));
    }
