    #[test]
    fn lays_out_extended_word_border_styles_as_passive_lines() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1\box\brdrhair Hairline\par\pard\brdrb\brdrdashdot Dashed\par}",
        )
        .unwrap();

        let layout = LayoutEngine::layout(&parsed.document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { width, style, .. }
                if (*width - 0.25).abs() < 0.01 && *style == LineStyle::Solid
        )));
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { style, .. } if *style == LineStyle::Dashed
        )));
        assert!(layout_text(page).contains("Hairline"));
        assert!(layout_text(page).contains("Dashed"));
    }

    #[test]
    fn does_not_synthesize_nested_fallback_grid_on_borderless_continuation_rows() {
        let mut input = String::from(
            r"{\rtf1\trowd\cellx6000 Outer {\trowd\itap2\trrh720\clbrdrt\brdrs\brdrw20\cellx3000 First A\nestcell\cellx6000 First B\nestrow}",
        );
        for index in 1..140 {
            input.push_str(&format!(
                r"{{\trowd\itap2\trrh720\clbrdrt\brdrnone\cellx3000 Row {index} A\nestcell\cellx6000 Row {index} B\nestrow}}"
            ));
        }
        input.push_str(r" Outer after\cell\row}");

        let parsed = crate::rtf::parse_rtf(&input).unwrap();
        let layout = LayoutEngine::layout(&parsed.document);
        assert!(layout.pages.len() > 1);
        let fallback_color = PdfColor {
            red: 0.65,
            green: 0.65,
            blue: 0.65,
        };
        let fallback_counts = layout
            .pages
            .iter()
            .skip(1)
            .map(|page| {
                page.items
                    .iter()
                    .filter(|item| {
                        matches!(
                            item,
                            LayoutItem::Line { color, width, .. }
                                if *color == fallback_color && (*width - 0.25).abs() < 0.01
                        )
                    })
                    .count()
            })
            .collect::<Vec<_>>();
        assert!(
            fallback_counts.iter().all(|count| *count == 0),
            "fallback counts: {fallback_counts:?}"
        );
    }

    #[test]
    fn lays_out_character_border_as_passive_box_lines() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
        ];
        let mut style = CharacterStyle::default();
        style.border = TableCellBorder {
            visible: true,
            width_twips: 80,
            color_index: Some(1),
            ..TableCellBorder::default()
        };
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Boxed".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let border_rectangles = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Highlight { color, .. } => Some(*color),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(border_rectangles.len(), 4);
        assert!(border_rectangles.iter().all(|color| {
            *color
                == PdfColor {
                    red: 1.0,
                    green: 0.0,
                    blue: 0.0,
                }
        }));
    }

    #[test]
    fn applies_page_geometry_to_layout() {
        let mut document = Document::default();
        document.page.width_twips = 14_400;
        document.page.height_twips = 7_200;
        document.page.margin_left_twips = 720;
        document.page.gutter_twips = 360;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Geometry".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.width, 720.0);
        assert_eq!(layout.height, 360.0);
        assert!(
            layout.pages[0]
                .items
                .iter()
                .any(|item| matches!(item, LayoutItem::Text(fragment) if fragment.x == 54.0))
        );
    }

    #[test]
    fn vertically_aligns_page_body_content_inside_margins() {
        let mut top_document = small_test_page_document();
        top_document.blocks = vec![paragraph_with_text("Body")];

        let mut bottom_document = top_document.clone();
        bottom_document.page.vertical_alignment = PageVerticalAlignment::Bottom;

        let top_layout = LayoutEngine::layout(&top_document);
        let bottom_layout = LayoutEngine::layout(&bottom_document);
        let top_baseline = text_baseline_for(&top_layout.pages[0], "Body");
        let bottom_baseline = text_baseline_for(&bottom_layout.pages[0], "Body");

        assert!(
            bottom_baseline < top_baseline - 100.0,
            "bottom alignment should move body content down"
        );
        assert!(
            bottom_baseline > bottom_layout.pages[0].geometry.margin_bottom,
            "bottom-aligned body should stay above the bottom margin"
        );
    }

    #[test]
    fn applies_page_gutter_as_bounded_passive_binding_space() {
        let mut document = Document::default();
        document.page.width_twips = 5_760;
        document.page.margin_left_twips = 720;
        document.page.margin_right_twips = 720;
        document.page.gutter_twips = 720;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Gutter".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let text = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "Gutter" => Some(fragment),
                _ => None,
            })
            .expect("gutter text");

        assert!((text.x - 72.0).abs() < 0.01);
        assert!((layout.pages[0].geometry.content_width - 180.0).abs() < 0.01);
    }

    #[test]
    fn applies_parallel_page_gutter_as_top_binding_space() {
        let mut document = Document::default();
        document.page.margin_left_twips = 720;
        document.page.margin_top_twips = 720;
        document.page.gutter_twips = 720;
        document.page.gutter_at_top = true;
        document.blocks = vec![paragraph_with_text("TopGutter")];

        let layout = LayoutEngine::layout(&document);
        let text = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "TopGutter" => Some(fragment),
                _ => None,
            })
            .expect("top gutter text");

        assert_eq!(text.x, 36.0);
        assert!((layout.pages[0].geometry.margin_top - 72.0).abs() < 0.01);
    }

    #[test]
    fn applies_right_to_left_gutter_as_right_binding_space() {
        let mut document = Document::default();
        document.page.width_twips = 5_760;
        document.page.margin_left_twips = 720;
        document.page.margin_right_twips = 360;
        document.page.gutter_twips = 720;
        document.page.gutter_on_right = true;
        document.blocks = vec![paragraph_with_text("RtlGutter")];

        let layout = LayoutEngine::layout(&document);
        let text = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "RtlGutter" => Some(fragment),
                _ => None,
            })
            .expect("gutter text");

        assert_eq!(text.x, 36.0);
        assert!((layout.pages[0].geometry.content_width - 198.0).abs() < 0.01);
    }

    #[test]
    fn mirrors_page_gutter_binding_space_on_even_pages() {
        let mut document = Document::default();
        document.page.width_twips = 5_760;
        document.page.margin_left_twips = 720;
        document.page.margin_right_twips = 360;
        document.page.gutter_twips = 720;
        document.page.facing_pages = true;
        document.page.mirror_margins = true;
        document.blocks = vec![
            paragraph_with_text("Odd"),
            Block::PageBreak,
            paragraph_with_text("Even"),
        ];

        let layout = LayoutEngine::layout(&document);
        let odd_x = text_x(&layout.pages[0], "Odd").expect("odd page text");
        let even_x = text_x(&layout.pages[1], "Even").expect("even page text");

        assert!((odd_x - 72.0).abs() < 0.01, "odd_x={odd_x}");
        assert!((even_x - 18.0).abs() < 0.01, "even_x={even_x}");
        assert!((layout.pages[0].geometry.content_width - 198.0).abs() < 0.01);
        assert!((layout.pages[1].geometry.content_width - 198.0).abs() < 0.01);
    }

    #[test]
    fn mirrors_right_to_left_gutter_binding_space_by_page_parity() {
        let mut document = Document::default();
        document.page.width_twips = 5_760;
        document.page.margin_left_twips = 720;
        document.page.margin_right_twips = 360;
        document.page.gutter_twips = 720;
        document.page.gutter_on_right = true;
        document.page.facing_pages = true;
        document.page.mirror_margins = true;
        document.blocks = vec![
            paragraph_with_text("Odd"),
            Block::PageBreak,
            paragraph_with_text("Even"),
        ];

        let layout = LayoutEngine::layout(&document);
        let odd_x = text_x(&layout.pages[0], "Odd").expect("odd page text");
        let even_x = text_x(&layout.pages[1], "Even").expect("even page text");

        assert_eq!(odd_x, 36.0);
        assert_eq!(even_x, 54.0);
        assert!((layout.pages[0].geometry.content_width - 198.0).abs() < 0.01);
        assert!((layout.pages[1].geometry.content_width - 198.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_landscape_page_dimensions() {
        let mut document = Document::default();
        document.page.width_twips = 15_840;
        document.page.height_twips = 12_240;
        document.page.landscape = true;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Landscape".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.width, 792.0);
        assert_eq!(layout.height, 612.0);
    }

    #[test]
    fn lays_out_text_color_and_highlight_background() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
            Color {
                red: 0,
                green: 255,
                blue: 0,
            },
        ];
        let mut style = CharacterStyle::default();
        style.color_index = 1;
        style.highlight_index = Some(2);
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Marked".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(|item| {
            matches!(
                item,
                LayoutItem::Highlight { color, .. }
                    if *color == PdfColor { red: 0.0, green: 1.0, blue: 0.0 }
            )
        }));
        assert!(page.items.iter().any(|item| {
            matches!(
                item,
                LayoutItem::Text(fragment)
                    if fragment.text == "Marked"
                        && fragment.color == PdfColor { red: 1.0, green: 0.0, blue: 0.0 }
            )
        }));
    }

    #[test]
    fn lays_out_form_field_shading_as_passive_gray_background() {
        let mut document = Document::default();
        let mut style = CharacterStyle::default();
        style.form_field_shading = true;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Form value".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        let form_shading = page
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Highlight {
                    x, width, color, ..
                } if *color
                    == (PdfColor {
                        red: 0.82,
                        green: 0.82,
                        blue: 0.82,
                    }) =>
                {
                    Some((*x, *x + *width))
                }
                _ => None,
            })
            .reduce(|acc, bounds| (acc.0.min(bounds.0), acc.1.max(bounds.1)))
            .expect("form shading");
        let text_bounds = page
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => {
                    let width = measure_text_with_family(
                        &fragment.text,
                        &fragment.style,
                        fragment.font_family,
                    );
                    Some((fragment.x, fragment.x + width))
                }
                _ => None,
            })
            .reduce(|acc, bounds| (acc.0.min(bounds.0), acc.1.max(bounds.1)))
            .expect("form text");

        assert!(
            form_shading.0 < text_bounds.0,
            "form shading should start before text: shading={form_shading:?}, text={text_bounds:?}"
        );
        assert!(
            form_shading.1 > text_bounds.1,
            "form shading should end after text: shading={form_shading:?}, text={text_bounds:?}"
        );
        assert!(layout_text(page).contains("Form value"));
    }

    #[test]
    fn lays_out_character_shading_intensity_as_darkened_background() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
        ];
        let mut style = CharacterStyle::default();
        style.highlight_index = Some(1);
        style.highlight_shading_basis_points = 5_000;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Tinted".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let color = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight { color, .. } => Some(*color),
                _ => None,
            })
            .expect("highlight");

        assert_eq!(
            color,
            PdfColor {
                red: 0.5,
                green: 0.0,
                blue: 0.0
            }
        );
    }

    #[test]
    fn lays_out_paragraph_shading_intensity_as_tinted_background() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 0,
                green: 0,
                blue: 255,
            },
        ];
        let mut paragraph_style = ParagraphStyle {
            shading_color_index: Some(1),
            shading_basis_points: 2_500,
            ..ParagraphStyle::default()
        };
        paragraph_style.space_after_twips = 0;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "Tinted paragraph".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let color = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight { color, .. } => Some(*color),
                _ => None,
            })
            .expect("paragraph shading");

        assert_eq!(
            color,
            PdfColor {
                red: 0.75,
                green: 0.75,
                blue: 1.0
            }
        );
    }

    #[test]
    fn lays_out_paragraph_shading_background() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 240,
                green: 240,
                blue: 0,
            },
        ];
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.shading_color_index = Some(1);
        paragraph_style.left_indent_twips = 720;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "Shaded paragraph".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(|item| {
            matches!(
                item,
                LayoutItem::Highlight { x, width, color, .. }
                    if (*x - 106.56).abs() < 0.01
                        && (*width - 434.88).abs() < 0.01
                        && *color == PdfColor {
                            red: 240.0 / 255.0,
                            green: 240.0 / 255.0,
                            blue: 0.0
                        }
            )
        }));
        assert!(layout_text(page).contains("Shaded paragraph"));
    }

    #[test]
    fn lays_out_paragraph_borders_as_passive_lines() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 30,
                green: 60,
                blue: 90,
            },
        ];
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 80,
            color_index: Some(1),
            ..TableCellBorder::default()
        };
        paragraph_style.borders.left = TableCellBorder {
            visible: true,
            width_twips: 40,
            color_index: None,
            ..TableCellBorder::default()
        };
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "Bordered".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, x2, width, color, .. }
                if (*x1 - 70.5).abs() < 0.01
                    && (*x2 - 541.5).abs() < 0.01
                    && (*width - 4.0).abs() < 0.01
                    && *color == PdfColor { red: 30.0 / 255.0, green: 60.0 / 255.0, blue: 90.0 / 255.0 }
        )));
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, x2, width, .. }
                if (*x1 - 69.5).abs() < 0.01
                    && (*x2 - 69.5).abs() < 0.01
                    && (*width - 2.0).abs() < 0.01
        )));
    }

    #[test]
    fn lays_out_paragraph_between_borders_as_passive_lines() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 200,
                green: 10,
                blue: 20,
            },
        ];
        let mut bordered = ParagraphStyle::default();
        bordered.borders.between = TableCellBorder {
            visible: true,
            width_twips: 60,
            color_index: Some(1),
            ..TableCellBorder::default()
        };

        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: bordered.clone(),
                runs: vec![Run {
                    text: "First".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::Paragraph(Paragraph {
                style: bordered,
                runs: vec![Run {
                    text: "Second".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let red = PdfColor {
            red: 200.0 / 255.0,
            green: 10.0 / 255.0,
            blue: 20.0 / 255.0,
        };
        let between_lines = page
            .items
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    LayoutItem::Line { x1, x2, y1, y2, width, color, .. }
                        if (*x2 - *x1).abs() > 100.0
                            && (*y1 - *y2).abs() < 0.01
                            && (*width - 3.0).abs() < 0.01
                            && *color == red
                )
            })
            .count();

        assert_eq!(between_lines, 1);
        assert!(layout_text(page).contains("First"));
        assert!(layout_text(page).contains("Second"));
    }

    #[test]
    fn skips_paragraph_between_border_when_next_paragraph_does_not_opt_in() {
        let mut document = Document::default();
        let mut bordered = ParagraphStyle::default();
        bordered.borders.between = TableCellBorder {
            visible: true,
            width_twips: 60,
            ..TableCellBorder::default()
        };
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: bordered,
                runs: vec![Run {
                    text: "First".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::Paragraph(Paragraph {
                style: ParagraphStyle::default(),
                runs: vec![Run {
                    text: "Second".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let between_lines = layout.pages[0]
            .items
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    LayoutItem::Line { x1, x2, y1, y2, width, .. }
                        if (*x2 - *x1).abs() > 100.0
                            && (*y1 - *y2).abs() < 0.01
                            && (*width - 3.0).abs() < 0.01
                )
            })
            .count();

        assert_eq!(between_lines, 0);
    }

    #[test]
    fn lays_out_section_breaks_as_passive_page_breaks() {
        let mut document = Document::default();
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Before".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::SectionBreak,
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "After".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert_eq!(layout_text(&layout.pages[0]), "Before");
        assert_eq!(layout_text(&layout.pages[1]), "After");
    }

    #[test]
    fn lays_out_odd_and_even_section_breaks_with_blank_pages_when_needed() {
        let mut odd_document = Document::default();
        odd_document.blocks = vec![
            paragraph_with_text("Before"),
            Block::OddPageSectionBreak,
            paragraph_with_text("Odd start"),
        ];

        let odd_layout = LayoutEngine::layout(&odd_document);

        assert_eq!(odd_layout.pages.len(), 3);
        assert_eq!(layout_text(&odd_layout.pages[0]), "Before");
        assert!(odd_layout.pages[1].items.is_empty());
        assert_eq!(layout_text(&odd_layout.pages[2]), "Odd start");

        let mut even_document = Document::default();
        even_document.blocks = vec![
            paragraph_with_text("Before"),
            Block::EvenPageSectionBreak,
            paragraph_with_text("Even start"),
        ];

        let even_layout = LayoutEngine::layout(&even_document);

        assert_eq!(even_layout.pages.len(), 3);
        assert!(even_layout.pages[1].items.is_empty());
        assert_eq!(layout_text(&even_layout.pages[2]), "Even start");

        even_document.page.facing_pages = true;
        let facing_even_layout = LayoutEngine::layout(&even_document);
        assert_eq!(facing_even_layout.pages.len(), 2);
        assert_eq!(layout_text(&facing_even_layout.pages[1]), "Even start");
    }

    #[test]
    fn lays_out_explicit_column_breaks_on_same_page() {
        let mut document = Document::default();
        document.page.column_count = 2;
        document.blocks = vec![
            paragraph_with_text("Left"),
            Block::ColumnBreak,
            paragraph_with_text("Right"),
        ];

        let layout = LayoutEngine::layout(&document);
        let left_x = text_x(&layout.pages[0], "Left").expect("left text");
        let right_x = text_x(&layout.pages[0], "Right").expect("right text");

        assert_eq!(layout.pages.len(), 1);
        assert!(right_x > left_x + 200.0);
    }

    #[test]
    fn lays_out_explicit_section_column_widths_and_gaps() {
        let mut document = Document::default();
        document.page.column_count = 2;
        document.page.column_widths_twips = vec![1440, 2880];
        document.page.column_gaps_twips = vec![360];
        document.page.line_between_columns = true;
        document.blocks = vec![
            paragraph_with_text("Left"),
            Block::ColumnBreak,
            paragraph_with_text("Right"),
        ];

        let layout = LayoutEngine::layout(&document);
        let left_x = text_x(&layout.pages[0], "Left").expect("left text");
        let right_x = text_x(&layout.pages[0], "Right").expect("right text");

        assert!((left_x - 72.0).abs() < 0.01);
        assert!((right_x - 162.0).abs() < 0.01);
        assert!(has_vertical_line_at(&layout.pages[0], 153.0));
    }

    #[test]
    fn flows_body_text_to_next_column_before_next_page() {
        let mut document = small_test_page_document();
        document.page.column_count = 2;
        document.blocks = (0..11)
            .map(|idx| paragraph_with_text(&format!("P{idx}")))
            .collect();

        let layout = LayoutEngine::layout(&document);
        let first_x = text_x(&layout.pages[0], "P0").expect("first paragraph");
        let overflow_x = text_x(&layout.pages[0], "P10").expect("overflow paragraph");

        assert_eq!(layout.pages.len(), 1);
        assert!(overflow_x > first_x + 200.0);
    }

    #[test]
    fn word_column_separator_spans_only_the_occupied_section_content() {
        let parsed = parse_rtf(include_str!("../../fixtures/section-columns-passive.rtf")).unwrap();
        let layout = LayoutEngine::layout(&parsed.document);
        let separator = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    width,
                    color,
                    ..
                } if (*x1 - 135.0).abs() < 0.01 && (*x2 - 135.0).abs() < 0.01 => {
                    Some((*y1, *y2, *width, *color))
                }
                _ => None,
            })
            .expect("column separator");

        assert!((separator.1 - separator.0 - 13.8).abs() < 0.01);
        assert!((separator.2 - 0.72).abs() < 0.01);
        assert_eq!(separator.3, PdfColor::default());
    }

    #[test]
    fn lays_out_page_borders_as_passive_lines() {
        let mut document = Document::default();
        document.page.margin_left_twips = 720;
        document.page.margin_top_twips = 720;
        document.page.margin_bottom_twips = 720;
        document.page.page_border_spacing_twips.top_twips = 240;
        document.page.page_border_spacing_twips.left_twips = 120;
        document.page.page_borders.top = TableCellBorder {
            visible: true,
            width_twips: 80,
            color_index: None,
            style: BorderStyle::Double,
            ..TableCellBorder::default()
        };
        document.page.page_borders.left = TableCellBorder {
            visible: true,
            width_twips: 40,
            color_index: None,
            style: BorderStyle::Dashed,
            ..TableCellBorder::default()
        };
        document.blocks = vec![paragraph_with_text("Body")];

        let layout = LayoutEngine::layout(&document);
        let top_y = layout.pages[0].geometry.height - layout.pages[0].geometry.margin_top + 12.0;
        let left_x = layout.pages[0].geometry.margin_left - 6.0;
        let top_border = layout.pages[0].items.iter().find_map(|item| match item {
            LayoutItem::Line { y1, y2, style, .. }
                if (*y1 - top_y).abs() < 0.01 && (*y2 - top_y).abs() < 0.01 =>
            {
                Some(style)
            }
            _ => None,
        });
        let left_border = layout.pages[0].items.iter().find_map(|item| match item {
            LayoutItem::Line { x1, x2, style, .. }
                if (*x1 - left_x).abs() < 0.01 && (*x2 - left_x).abs() < 0.01 =>
            {
                Some(style)
            }
            _ => None,
        });

        assert_eq!(top_border, Some(&LineStyle::Double));
        assert_eq!(left_border, Some(&LineStyle::Dashed));
    }

    #[test]
    fn lays_out_word_border_art_42_as_bounded_scissors_and_strip() {
        let parsed = parse_rtf(include_str!("../../fixtures/page-border-art-passive.rtf")).unwrap();
        let layout = LayoutEngine::layout(&parsed.document);
        let page = &layout.pages[0];
        let art = page
            .items
            .iter()
            .filter(|item| !matches!(item, LayoutItem::Text(_)))
            .collect::<Vec<_>>();

        assert_eq!(art.len(), 16);
        assert!(matches!(
            art[1],
            LayoutItem::Polygon { points, stroke_width, .. }
                if *stroke_width == 0.0
                    && points.as_slice() == [
                        LayoutPoint { x: 47.52, y: 389.54 },
                        LayoutPoint { x: 38.76, y: 389.54 },
                        LayoutPoint { x: 42.216, y: 392.4 },
                        LayoutPoint { x: 51.216, y: 392.4 },
                    ]
        ));
        assert!(matches!(
            (art[2], art[3]),
            (
                LayoutItem::Ellipse { fill_color: Some(color), .. },
                LayoutItem::Ellipse { fill_color: Some(hole), .. },
            ) if *color == PdfColor::default() && *hole == PdfColor { red: 1.0, green: 1.0, blue: 1.0 }
        ));
        assert!(matches!(
            art[4],
            LayoutItem::Polygon { points, stroke_width, .. }
                if *stroke_width == 0.0
                    && points.as_slice() == [
                        LayoutPoint { x: 43.824, y: 396.6 },
                        LayoutPoint { x: 47.04, y: 392.4 },
                        LayoutPoint { x: 50.16, y: 392.4 },
                        LayoutPoint { x: 47.04, y: 396.6 },
                    ]
        ));
        assert!(art.iter().any(|item| matches!(
            item,
            LayoutItem::Ellipse { x, y, width, height, .. }
                if (*x - 35.88).abs() < 0.01
                    && (*y - 385.34).abs() < 0.01
                    && (*width - 9.46).abs() < 0.01
                    && (*height - 7.42).abs() < 0.01
        )));
        assert!(art.iter().any(|item| matches!(
            item,
            LayoutItem::Polygon { points, stroke_width, fill_color: Some(_), .. }
                if (*stroke_width - 0.14).abs() < 0.001
                    && points.iter().any(|point| (point.x - 68.12).abs() < 0.01)
        )));
        assert!(art.iter().any(|item| matches!(
            item,
            LayoutItem::Highlight { x, y, width, height, .. }
                if (*x - 72.38).abs() < 0.01
                    && (*y - 378.67).abs() < 0.01
                    && (*width - 25.34).abs() < 0.01
                    && (*height - 5.26).abs() < 0.01
        )));
        assert!(
            art.iter()
                .all(|item| !matches!(item, LayoutItem::Line { .. }))
        );
    }

    #[test]
    fn page_borders_can_surround_header_and_footer_regions() {
        let mut document = Document::default();
        document.page.margin_top_twips = 1_440;
        document.page.margin_bottom_twips = 1_440;
        document.page.header_distance_twips = 360;
        document.page.footer_distance_twips = 360;
        document.page.page_border_includes_header = true;
        document.page.page_border_includes_footer = true;
        let Block::Paragraph(header) = paragraph_with_text("Header") else {
            unreachable!()
        };
        let Block::Paragraph(footer) = paragraph_with_text("Footer") else {
            unreachable!()
        };
        document.header = vec![header];
        document.footer = vec![footer];
        document.page.page_borders.top = TableCellBorder {
            visible: true,
            ..TableCellBorder::default()
        };
        document.page.page_borders.bottom = TableCellBorder {
            visible: true,
            ..TableCellBorder::default()
        };
        document.blocks = vec![paragraph_with_text("Body")];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let top_y = page.height - 18.0;
        let bottom_y = 18.0;

        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { y1, y2, .. }
                if (*y1 - top_y).abs() < 0.01 && (*y2 - top_y).abs() < 0.01
        )));
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { y1, y2, .. }
                if (*y1 - bottom_y).abs() < 0.01 && (*y2 - bottom_y).abs() < 0.01
        )));
    }

    #[test]
    fn word_pgbrdropt_uses_text_margin_scope_without_empty_header_expansion() {
        let parsed = parse_rtf(include_str!("../../fixtures/page-border-scope-passive.rtf")).unwrap();
        assert!(!parsed.document.page.page_border_from_page_edge);
        let layout = LayoutEngine::layout(&parsed.document);
        let lines = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    width,
                    ..
                } => Some((*x1, *y1, *x2, *y2, *width)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 3);
        assert!(lines.iter().all(|line| (line.4 - 0.72).abs() < 0.001));
        assert!(lines.contains(&(36.0, 360.0, 270.0, 360.0, 0.72)));
        assert!(lines.contains(&(36.0, 36.0, 270.0, 36.0, 0.72)));
        assert!(lines.contains(&(36.0, 360.0, 36.0, 36.0, 0.72)));
    }

    #[test]
    fn page_break_before_starts_following_paragraph_on_new_page() {
        let mut document = Document::default();
        let mut style = ParagraphStyle::default();
        style.page_break_before = true;
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Before".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::Paragraph(Paragraph {
                style,
                runs: vec![Run {
                    text: "After".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert_eq!(layout_text(&layout.pages[0]), "Before");
        assert_eq!(layout_text(&layout.pages[1]), "After");
    }

    #[test]
    fn keep_together_starts_paragraph_on_next_page_when_it_would_split() {
        let mut document = small_test_page_document();
        let mut keep_style = ParagraphStyle::default();
        keep_style.keep_together = true;
        document.blocks = (0..8)
            .map(|_| paragraph_with_text("Filler"))
            .chain([Block::Paragraph(Paragraph {
                style: keep_style,
                runs: vec![Run {
                    text: "Keep\nTogether\nLines".to_string(),
                    style: Default::default(),
                }],
            })])
            .collect();

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert!(!layout_text(&layout.pages[0]).contains("Keep"));
        assert!(layout_text(&layout.pages[1]).contains("Keep"));
    }

    #[test]
    fn keep_with_next_starts_pair_on_next_page_when_pair_would_split() {
        let mut document = small_test_page_document();
        let mut keep_next_style = ParagraphStyle::default();
        keep_next_style.keep_with_next = true;
        document.blocks = (0..9)
            .map(|_| paragraph_with_text("Filler"))
            .chain([
                Block::Paragraph(Paragraph {
                    style: keep_next_style,
                    runs: vec![Run {
                        text: "Keep next".to_string(),
                        style: Default::default(),
                    }],
                }),
                paragraph_with_text("Follower"),
            ])
            .collect();

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert!(!layout_text(&layout.pages[0]).contains("Keep next"));
        assert!(layout_text(&layout.pages[1]).contains("Keep next"));
        assert!(layout_text(&layout.pages[1]).contains("Follower"));
    }

    #[test]
    fn repeats_header_and_footer_on_each_page() {
        let mut document = Document::default();
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Header".to_string(),
                style: Default::default(),
            }],
        }];
        document.footer = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Footer".to_string(),
                style: Default::default(),
            }],
        }];
        document.blocks.clear();
        for _ in 0..120 {
            document.blocks.push(Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Body paragraph.".to_string(),
                    style: Default::default(),
                }],
            }));
        }

        let layout = LayoutEngine::layout(&document);
        assert!(layout.pages.len() > 1);
        for page in &layout.pages {
            assert!(page.items.iter().any(
                |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Header")
            ));
            assert!(page.items.iter().any(
                |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Footer")
            ));
        }
    }

    #[test]
    fn repeats_header_images_on_each_page() {
        let mut document = Document::default();
        document.header_images = vec![StaticImage {
            format: ImageFormat::Jpeg,
            bytes: vec![0xff, 0xd8, 0xff, 0xd9],
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 8,
            height_px: 4,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(360),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        }];
        document.blocks.clear();
        for _ in 0..120 {
            document.blocks.push(Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Body paragraph.".to_string(),
                    style: Default::default(),
                }],
            }));
        }

        let layout = LayoutEngine::layout(&document);
        assert!(layout.pages.len() > 1);
        for page in &layout.pages {
            assert!(
                page.items
                    .iter()
                    .any(|item| matches!(item, LayoutItem::Image(_))),
                "expected repeated header image on every page"
            );
        }
    }

    #[test]
    fn lays_out_header_and_footer_at_configured_distances() {
        let mut document = Document::default();
        document.page.header_distance_twips = 360;
        document.page.footer_distance_twips = 1_080;
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Header".to_string(),
                style: Default::default(),
            }],
        }];
        document.footer = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Footer".to_string(),
                style: Default::default(),
            }],
        }];
        document.blocks = vec![paragraph_with_text("Body")];

        let layout = LayoutEngine::layout(&document);
        let header_y = text_baseline_for(&layout.pages[0], "Header");
        let footer_y = text_baseline_for(&layout.pages[0], "Footer");

        assert!(
            (header_y - (layout.pages[0].height - 29.25)).abs() < 0.01,
            "header_y={header_y}, height={}",
            layout.pages[0].height
        );
        assert!((footer_y - 57.75).abs() < 0.01, "footer_y={footer_y}");
    }

    #[test]
    fn lays_out_header_paragraph_shading_and_borders() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 220,
                green: 230,
                blue: 240,
            },
        ];
        let mut header_style = ParagraphStyle::default();
        header_style.shading_color_index = Some(1);
        header_style.borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 40,
            style: BorderStyle::Single,
            ..TableCellBorder::default()
        };
        document.header = vec![Paragraph {
            style: header_style,
            runs: vec![Run {
                text: "Header".to_string(),
                style: Default::default(),
            }],
        }];
        document.blocks = vec![paragraph_with_text("Body")];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(
            page.items
                .iter()
                .any(|item| matches!(item, LayoutItem::Highlight { .. }))
        );
        assert!(page.items.iter().any(
            |item| matches!(item, LayoutItem::Line { x1, x2, .. } if (*x2 - *x1).abs() > 100.0)
        ));
    }

    #[test]
    fn lays_out_section_header_footer_distances_on_later_pages() {
        let mut document = Document::default();
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Header".to_string(),
                style: Default::default(),
            }],
        }];
        document.footer = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Footer".to_string(),
                style: Default::default(),
            }],
        }];
        let mut second_section = PageSettings::default();
        second_section.header_distance_twips = 360;
        second_section.footer_distance_twips = 1_080;
        document.blocks = vec![
            paragraph_with_text("First section"),
            Block::SectionBreak,
            Block::SectionSettings(second_section),
            paragraph_with_text("Second section"),
        ];

        let layout = LayoutEngine::layout(&document);
        let first_header_y = text_baseline_for(&layout.pages[0], "Header");
        let second_header_y = text_baseline_for(&layout.pages[1], "Header");
        let second_footer_y = text_baseline_for(&layout.pages[1], "Footer");

        assert!(
            (first_header_y - (layout.pages[0].height - 47.25)).abs() < 0.01,
            "first_header_y={first_header_y}, height={}",
            layout.pages[0].height
        );
        assert!(
            (second_header_y - (layout.pages[1].height - 29.25)).abs() < 0.01,
            "second_header_y={second_header_y}, height={}",
            layout.pages[1].height
        );
        assert!(
            (second_footer_y - 57.75).abs() < 0.01,
            "second_footer_y={second_footer_y}"
        );
    }

    #[test]
    fn selects_section_specific_header_footer_on_later_pages() {
        let mut document = Document::default();
        document.header = vec![repeating_paragraph("Document header")];
        document.footer = vec![repeating_paragraph("Document footer")];
        let mut second_section = PageSettings::default();
        second_section.header = vec![repeating_paragraph("Section header")];
        second_section.footer = vec![repeating_paragraph("Section footer")];
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
        assert!(layout_text(&layout.pages[0]).contains("Document header"));
        assert!(layout_text(&layout.pages[0]).contains("Document footer"));
        assert!(!layout_text(&layout.pages[0]).contains("Section header"));
        assert!(layout_text(&layout.pages[1]).contains("Section header"));
        assert!(layout_text(&layout.pages[1]).contains("Section footer"));
        assert!(!layout_text(&layout.pages[1]).contains("Document header"));
        assert!(layout_text(&layout.pages[2]).contains("Section header"));
        assert!(layout_text(&layout.pages[2]).contains("Section footer"));
    }

    #[test]
    fn selects_first_even_and_default_header_footer_variants() {
        fn repeating_paragraph(text: &str) -> Paragraph {
            Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: text.to_string(),
                    style: Default::default(),
                }],
            }
        }

        let mut document = Document::default();
        document.page.title_page = true;
        document.page.facing_pages = true;
        document.header = vec![repeating_paragraph("Odd header")];
        document.first_page_header = vec![repeating_paragraph("First header")];
        document.even_page_header = vec![repeating_paragraph("Even header")];
        document.footer = vec![repeating_paragraph("Odd footer")];
        document.first_page_footer = vec![repeating_paragraph("First footer")];
        document.even_page_footer = vec![repeating_paragraph("Even footer")];
        document.blocks = vec![
            paragraph_with_text("Page one"),
            Block::PageBreak,
            paragraph_with_text("Page two"),
            Block::PageBreak,
            paragraph_with_text("Page three"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 3);
        assert!(layout_text(&layout.pages[0]).contains("First header"));
        assert!(layout_text(&layout.pages[0]).contains("First footer"));
        assert!(!layout_text(&layout.pages[0]).contains("Odd header"));
        assert!(layout_text(&layout.pages[1]).contains("Even header"));
        assert!(layout_text(&layout.pages[1]).contains("Even footer"));
        assert!(layout_text(&layout.pages[2]).contains("Odd header"));
        assert!(layout_text(&layout.pages[2]).contains("Odd footer"));
    }

    #[test]
    fn ignores_even_header_footer_variants_without_facing_pages() {
        let mut document = Document::default();
        document.page.margin_top_twips = 720;
        document.header = vec![repeating_paragraph("RightHeader")];
        document.even_page_header = vec![repeating_paragraph("LeftHeader")];
        document.footer = vec![repeating_paragraph("RightFooter")];
        document.even_page_footer = vec![repeating_paragraph("LeftFooter")];
        document.blocks = vec![
            paragraph_with_text("PageOne"),
            Block::PageBreak,
            paragraph_with_text("PageTwo"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert!(layout_text(&layout.pages[1]).contains("RightHeader"));
        assert!(layout_text(&layout.pages[1]).contains("RightFooter"));
        assert!(!layout_text(&layout.pages[1]).contains("LeftHeader"));
        assert!(!layout_text(&layout.pages[1]).contains("LeftFooter"));
        let header_y = text_baseline_for(&layout.pages[0], "RightHeader");
        let body_y = text_baseline_for(&layout.pages[0], "PageOne");
        assert!(
            (header_y - body_y - 15.0).abs() < 0.01,
            "header_y={header_y}, body_y={body_y}"
        );
    }

    #[test]
    fn selects_first_header_footer_on_later_section_title_page() {
        fn repeating_paragraph(text: &str) -> Paragraph {
            Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: text.to_string(),
                    style: Default::default(),
                }],
            }
        }

        let mut document = Document::default();
        document.header = vec![repeating_paragraph("Odd header")];
        document.first_page_header = vec![repeating_paragraph("First header")];
        document.footer = vec![repeating_paragraph("Odd footer")];
        document.first_page_footer = vec![repeating_paragraph("First footer")];
        let mut second_section = PageSettings::default();
        second_section.title_page = true;
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
        assert!(layout_text(&layout.pages[0]).contains("Odd header"));
        assert!(layout_text(&layout.pages[0]).contains("Odd footer"));
        assert!(!layout_text(&layout.pages[0]).contains("First header"));
        assert!(layout_text(&layout.pages[1]).contains("First header"));
        assert!(layout_text(&layout.pages[1]).contains("First footer"));
        assert!(!layout_text(&layout.pages[1]).contains("Odd header"));
        assert!(layout_text(&layout.pages[2]).contains("Odd header"));
        assert!(layout_text(&layout.pages[2]).contains("Odd footer"));
    }
