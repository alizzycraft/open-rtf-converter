    #[test]
    fn paginates_long_documents() {
        let mut document = Document::default();
        document.blocks.clear();
        for _ in 0..120 {
            document.blocks.push(Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "A paragraph that should force more than one page.".to_string(),
                    style: Default::default(),
                }],
            }));
        }

        let layout = LayoutEngine::layout(&document);
        assert!(layout.pages.len() > 1);
    }

    #[test]
    fn lays_out_scaled_static_images_without_goal_dimensions() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Jpeg,
            bytes: vec![0xff, 0xd8, 0xff, 0xd9],
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 100,
            height_px: 50,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: None,
            display_height_twips: None,
            scale_x_percent: Some(50),
            scale_y_percent: Some(200),
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let image = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .expect("scaled image");

        assert!((image.width - 37.5).abs() < 0.01);
        assert!((image.height - 75.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_picture_natural_size_hints_when_goals_are_absent() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Jpeg,
            bytes: vec![0xff, 0xd8, 0xff, 0xd9],
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: Some(80),
            natural_height_px_hint: Some(40),
            display_width_twips: None,
            display_height_twips: None,
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let image = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .expect("hint-sized image");

        assert!((image.width - 60.0).abs() < 0.01);
        assert!((image.height - 30.0).abs() < 0.01);
        assert_eq!(image.image.width_px, 1);
        assert_eq!(image.image.height_px, 1);
    }

    #[test]
    fn applies_picture_scaling_to_goal_dimensions() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Jpeg,
            bytes: vec![0xff, 0xd8, 0xff, 0xd9],
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 100,
            height_px: 50,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(1440),
            scale_x_percent: Some(50),
            scale_y_percent: Some(200),
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let image = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .expect("scaled image");

        assert!((image.width - 18.0).abs() < 0.01);
        assert!((image.height - 144.0).abs() < 0.01);
    }

    #[test]
    fn applies_picture_crop_before_goal_scaling() {
        let parsed = parse_rtf(include_str!("../../fixtures/picture-scaling-passive.rtf"))
            .expect("picture scaling fixture should parse");

        let layout = LayoutEngine::layout(&parsed.document);
        let image = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .expect("cropped scaled image");

        assert!((image.width - 30.0).abs() < 0.01);
        assert!((image.height - 60.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_one_sided_picture_goal_dimensions_with_natural_aspect_ratio() {
        let mut width_only = Document::default();
        width_only.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Jpeg,
            bytes: vec![0xff, 0xd8, 0xff, 0xd9],
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 200,
            height_px: 100,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(1440),
            display_height_twips: None,
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];
        let width_layout = LayoutEngine::layout(&width_only);
        let width_image = width_layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .expect("width-goal image");
        assert!((width_image.width - 72.0).abs() < 0.01);
        assert!((width_image.height - 36.0).abs() < 0.01);

        let mut height_only = Document::default();
        height_only.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Jpeg,
            bytes: vec![0xff, 0xd8, 0xff, 0xd9],
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 200,
            height_px: 100,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: None,
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];
        let height_layout = LayoutEngine::layout(&height_only);
        let height_image = height_layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .expect("height-goal image");
        assert!((height_image.width - 72.0).abs() < 0.01);
        assert!((height_image.height - 36.0).abs() < 0.01);
    }

    #[test]
    fn positions_nested_right_aligned_rows_inside_available_width() {
        assert_eq!(
            nested_table_row_left(10.0, 100.0, TableRowAlignment::Right, &[30.0, 10.0]),
            70.0
        );
        assert_eq!(
            nested_table_row_left(10.0, 100.0, TableRowAlignment::Center, &[30.0, 10.0]),
            40.0
        );
    }

    #[test]
    fn omits_tabs_for_nested_horizontal_merge_continuations() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1\trowd\cellx6000 Outer {\trowd\itap2\clmgf\cellx1000 A\nestcell\clmrg\cellx3000 B\nestrow}\cell\row}",
        )
        .unwrap();
        let table = match &parsed.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let paragraphs = table_cell_paragraphs_for_layout(&table.rows[0].cells[0]);
        let nested_row = paragraphs.last().expect("nested row paragraph");
        assert_eq!(
            nested_row
                .runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            "A"
        );
        assert!(nested_row.style.tab_stops_twips.is_empty());
    }

    #[test]
    fn spans_nested_vertical_merge_geometry_across_authored_rows() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1\trowd\cellx6000 Outer {\trowd\itap2\trrh720\clvmgf\cellx1000 A\nestcell\cellx2000 B\nestrow}{\trowd\itap2\trrh720\clvmrg\cellx1000 C\nestcell\cellx2000 D\nestrow}\cell\row}",
        )
        .unwrap();
        let table = match &parsed.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let nested = table.rows[0].cells[0]
            .nested_table
            .as_ref()
            .expect("nested table");
        assert_eq!(
            nested.rows[0].cells[0].vertical_merge,
            TableCellVerticalMerge::First
        );
        assert_eq!(
            nested.rows[1].cells[0].vertical_merge,
            TableCellVerticalMerge::Continuation
        );
        assert_eq!(
            nested_vertical_span_height(nested, 0, 0, 0, 2, &[36.0, 36.0]),
            72.0
        );
    }

    #[test]
    fn carries_nested_vertical_merge_shading_to_split_continuations() {
        let mut input = String::from(
            r"{\rtf1{\colortbl;\red255\green220\blue180;}\trowd\cellx6000 Outer {\trowd\itap2\trrh720\clvmgf\clcbpat1\clbrdrl\brdrs\brdrw20\clbrdrr\brdrs\brdrw20\cellx1000 Merged\nestcell\cellx3000 Row 0\nestrow}",
        );
        for index in 1..140 {
            input.push_str(&format!(
                r"{{\trowd\itap2\trrh720\clvmrg\cellx1000 \nestcell\cellx3000 Row {index}\nestrow}}"
            ));
        }
        input.push_str(r" Outer after\cell\row}");

        let parsed = crate::rtf::parse_rtf(&input).unwrap();
        let layout = LayoutEngine::layout(&parsed.document);
        assert!(layout.pages.len() > 1);
        assert!(layout.pages.iter().skip(1).any(|page| {
            page.items
                .iter()
                .any(|item| matches!(item, LayoutItem::Highlight { .. }))
        }));
        assert!(layout.pages.iter().skip(1).any(|page| {
            page.items
                .iter()
                .any(|item| matches!(item, LayoutItem::Line { .. }))
        }));
    }

    #[test]
    fn word_positioned_floating_rows_share_margin_anchor_and_reflow_body_text() {
        let parsed =
            crate::rtf::parse_rtf(include_str!("../../fixtures/floating-table-positioning.rtf"))
                .expect("floating-table fixture should parse");
        let layout = LayoutEngine::layout(&parsed.document);
        let page = &layout.pages[0];
        let text = |needle: &str| {
            page.items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.starts_with(needle) => {
                        Some(fragment)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing text fragment {needle:?}"))
        };

        let floating = text("Floating");
        let follow = text("Follow");
        let before_head = text("Bef");
        let before_tail = text("ore");
        assert!((before_head.x - 36.0).abs() < 0.01);
        assert!(before_tail.baseline_y < before_head.baseline_y);
        assert!((floating.x - 72.0).abs() < 0.01, "x={}", floating.x);
        assert!((floating.baseline_y - follow.baseline_y).abs() < 0.01);
        let after = text("After");
        assert!((after.x - 36.0).abs() < 0.01, "x={}", after.x);
        assert!(after.baseline_y < floating.baseline_y);
    }

    #[test]
    fn lays_out_superscript_as_smaller_shifted_text() {
        let mut document = Document::default();
        let mut super_style = CharacterStyle::default();
        super_style.baseline_shift_half_points = 6;
        super_style.font_size_scale_percent = 65;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    text: "base".to_string(),
                    style: Default::default(),
                },
                Run {
                    text: "sup".to_string(),
                    style: super_style,
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let fragment_for = |text: &str| {
            layout.pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text == text => Some(fragment),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing text fragment {text}"))
        };
        let base = fragment_for("base");
        let sup = fragment_for("sup");

        assert!((base.style.font_size_points() - 12.0).abs() < 0.01);
        assert!((sup.style.font_size_points() - 7.8).abs() < 0.01);
        assert!(sup.baseline_y > base.baseline_y);
    }

    #[test]
    fn lays_out_double_strikeout_as_two_passive_lines() {
        let mut double_style = CharacterStyle::default();
        double_style.strike = true;
        double_style.double_strike = true;
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "double".to_string(),
                style: double_style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let strike_lines = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Line { y1, y2, .. } if (*y1 - *y2).abs() < 0.01 => Some(*y1),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(strike_lines.len(), 2);
        assert!((strike_lines[0] - strike_lines[1]).abs() >= 2.0);
    }

    #[test]
    fn hidden_runs_do_not_layout_as_text() {
        let mut document = Document::default();
        let mut hidden_style = CharacterStyle::default();
        hidden_style.hidden = true;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    text: "Visible".to_string(),
                    style: Default::default(),
                },
                Run {
                    text: "Hidden".to_string(),
                    style: hidden_style,
                },
                Run {
                    text: "Shown".to_string(),
                    style: Default::default(),
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let text = layout_text(&layout.pages[0]);

        assert_eq!(text, "VisibleShown");
        assert!(!text.contains("Hidden"));
    }

    #[test]
    fn character_scaling_does_not_scale_explicit_tracking() {
        let mut scaled_and_spaced = CharacterStyle::default();
        scaled_and_spaced.character_scaling_percent = 50;
        scaled_and_spaced.character_spacing_twips = 20;
        let plain_width = measure_text("ABC", &CharacterStyle::default());
        let combined_width = measure_text("ABC", &scaled_and_spaced);

        assert!((combined_width - (plain_width * 0.5) - 2.0).abs() < 0.01);
    }

    #[test]
    fn ignores_shading_with_an_undefined_color_index() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1\ansi\trowd\trhdr\trrh720\clcbpat1\cellx3600 Header row\cell\row}",
        )
        .unwrap();

        let layout = LayoutEngine::layout(&parsed.document);
        let page = &layout.pages[0];

        assert!(layout_text(page).contains("Header row"));
        assert!(
            !page
                .items
                .iter()
                .any(|item| matches!(item, LayoutItem::Highlight { .. })),
            "an unresolved cell-color reference must not become an opaque black fill"
        );
    }

    #[test]
    fn lays_out_shading_patterns_as_passive_lines() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 240,
                green: 0,
                blue: 0,
            },
            Color {
                red: 0,
                green: 0,
                blue: 255,
            },
        ];
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.shading_color_index = Some(1);
        paragraph_style.shading_basis_points = 2_500;
        paragraph_style.shading_pattern =
            ShadingPattern::Horizontal.with_foreground_color_index(Some(2));
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: paragraph_style,
                runs: vec![Run {
                    text: "Horizontal paragraph".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::Table(Table {
                column_widths_twips: vec![1440],
                borders_visible: false,
                preserve_authored_widths: false,
                rows: vec![TableRow {
                    column_widths_twips: Vec::new(),
                    height_twips: None,
                    left_offset_twips: 0,
                    vertical_offset_twips: 0,
                    wrap_margins: TableRowWrapMargins::default(),
                    cell_gap_twips: 60,
                    alignment: TableRowAlignment::Left,
                    repeat_header: false,
                    keep_together: false,
                    keep_with_next: false,
                    no_overlap: false,
                    cells: vec![TableCell {
                        nested_table: None,
                        shading_color_index: Some(1),
                        shading_basis_points: 5_000,
                        shading_pattern: ShadingPattern::Vertical
                            .with_foreground_color_index(Some(2)),
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "Vertical cell".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    }],
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let blue = PdfColor {
            red: 0.0,
            green: 0.0,
            blue: 1.0,
        };

        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Highlight { color, .. }
                if color.red > 0.9 && color.green < 0.8 && color.blue < 0.8
        )));
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { y1, y2, width, color, style, .. }
                if (*y1 - *y2).abs() < 0.01
                    && (*width - 0.24).abs() < 0.01
                    && *color == blue
                    && *style == LineStyle::Solid
        )));
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, x2, width, color, style, .. }
                if (*x1 - *x2).abs() < 0.01
                    && (*width - 0.24).abs() < 0.01
                    && *color == blue
                    && *style == LineStyle::Solid
        )));
        assert!(layout_text(page).contains("Horizontal paragraph"));
        assert!(layout_text(page).contains("Vertical cell"));
    }

    #[test]
    fn lays_out_extended_shading_patterns_as_passive_lines() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 240,
                green: 0,
                blue: 0,
            },
        ];
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.shading_color_index = Some(1);
        paragraph_style.shading_pattern =
            ShadingPattern::ForwardDiagonal.with_foreground_color_index(Some(1));
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: paragraph_style,
                runs: vec![Run {
                    text: "Diagonal paragraph".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::Table(Table {
                column_widths_twips: vec![1440],
                borders_visible: false,
                preserve_authored_widths: false,
                rows: vec![TableRow {
                    column_widths_twips: Vec::new(),
                    height_twips: None,
                    left_offset_twips: 0,
                    vertical_offset_twips: 0,
                    wrap_margins: TableRowWrapMargins::default(),
                    cell_gap_twips: 60,
                    alignment: TableRowAlignment::Left,
                    repeat_header: false,
                    keep_together: false,
                    keep_with_next: false,
                    no_overlap: false,
                    cells: vec![TableCell {
                        nested_table: None,
                        shading_color_index: Some(1),
                        shading_basis_points: 10_000,
                        shading_pattern: ShadingPattern::DarkCross
                            .with_foreground_color_index(Some(1)),
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "Dark cross cell".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    }],
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let red = PdfColor {
            red: 240.0 / 255.0,
            green: 0.0,
            blue: 0.0,
        };

        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, y1, x2, y2, width, color, style }
                if (*x1 - *x2).abs() > 0.01
                    && (*y1 - *y2).abs() > 0.01
                    && (*width - 0.24).abs() < 0.01
                    && *color == red
                    && *style == LineStyle::Solid
        )));
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { y1, y2, width, color, .. }
                if (*y1 - *y2).abs() < 0.01
                    && (*width - 0.24).abs() < 0.01
                    && *color == red
        )));
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, x2, width, color, .. }
                if (*x1 - *x2).abs() < 0.01
                    && (*width - 0.24).abs() < 0.01
                    && *color == red
        )));
        assert!(layout_text(page).contains("Diagonal paragraph"));
        assert!(layout_text(page).contains("Dark cross cell"));
    }

    #[test]
    fn legacy_charset_calibri_fallback_yields_to_an_exact_supplied_face() {
        let font = FontDef {
            index: 0,
            name: "Mac Face".to_string(),
            alternate_name: Some("Calibri".to_string()),
            charset: Some(77),
            code_page: None,
            family: FontFamilyHint::Nil,
            pitch: FontPitch::Default,
        };
        let style = CharacterStyle {
            font_index: 0,
            font_size_half_points: 24,
            ..Default::default()
        };
        let browser_provider = FontProvider::browser_safe_defaults();
        for (ch, units) in [
            ('\u{00b3}', 685.0),
            ('\u{00d2}', 1356.0),
            ('\u{00d3}', 1356.0),
            ('\u{00e9}', 1019.0),
            ('\u{201a}', 511.0),
            ('\u{201c}', 857.0),
            ('\u{201d}', 857.0),
            ('\u{2502}', 1024.0),
        ] {
            assert!(
                (source_word_compatible_advance_points(
                    Some(&font),
                    &style,
                    ch,
                    12.0,
                    Some(&browser_provider),
                )
                .expect("missing legacy face should use bounded Calibri fallback metrics")
                    - (12.0 * units / 2_048.0))
                    .abs()
                    < 0.001
            );
        }

        let exact_provider = FontProvider {
            assets: vec![FontAsset {
                family_names: vec!["Mac Face".to_string()],
                style: FontAssetStyle::default(),
                bytes: include_bytes!("../../fixtures/fonts/Tuffy.ttf").to_vec(),
            }],
            limits: FontProviderLimits::default(),
        };
        assert!(
            source_word_compatible_advance_points(
                Some(&font),
                &style,
                '\u{201c}',
                12.0,
                Some(&exact_provider),
            )
            .is_none()
        );
    }

    #[test]
    fn resolves_symbol_bullet_runs_to_passive_sans_fallback() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Helvetica".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Symbol".to_string(),
                alternate_name: None,
                charset: Some(2),
                code_page: None,
                family: FontFamilyHint::Tech,
                pitch: FontPitch::Default,
            },
        ];
        let mut style = CharacterStyle::default();
        style.font_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "\u{2022}\t".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(|item| matches!(
            item,
            LayoutItem::Text(fragment)
                if fragment.text == "\u{2022}" && fragment.font_family == PdfFontFamily::Helvetica
        )));
        assert!(!layout.pages[0].items.iter().any(|item| matches!(
            item,
            LayoutItem::Text(fragment)
                if fragment.text == "\u{2022}" && fragment.font_family == PdfFontFamily::Symbol
        )));
    }

    #[test]
    fn resolves_wingdings_checkbox_runs_to_zapf_dingbats() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Helvetica".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Wingdings".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Tech,
                pitch: FontPitch::Default,
            },
        ];
        let mut style = CharacterStyle::default();
        style.font_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "\u{2611}".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.font_family == PdfFontFamily::ZapfDingbats)
        ));
    }

    #[test]
    fn resolves_unicode_checkbox_runs_to_zapf_dingbats() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Helvetica".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Segoe UI Symbol".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Nil,
                pitch: FontPitch::Default,
            },
        ];
        let mut style = CharacterStyle::default();
        style.font_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "\u{25a1} \u{2610} \u{2611} \u{2612} \u{2713} \u{2717}".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.font_family == PdfFontFamily::ZapfDingbats)
        ));
    }

    #[test]
    fn resolves_segoe_ui_symbol_latin_text_to_passive_sans_fallback() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Helvetica".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Segoe UI Symbol".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Nil,
                pitch: FontPitch::Default,
            },
        ];
        let mut style = CharacterStyle::default();
        style.font_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Label \u{2611}".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Label " && fragment.font_family == PdfFontFamily::Helvetica)
        ));
        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "\u{2611}" && fragment.font_family == PdfFontFamily::ZapfDingbats)
        ));
        assert!(
            !layout.pages[0]
                .items
                .iter()
                .any(|item| matches!(item, LayoutItem::Text(fragment) if fragment.font_family == PdfFontFamily::Symbol))
        );
    }

    #[test]
    fn lays_out_caps_as_uppercase_display_text() {
        let mut document = Document::default();
        let mut style = CharacterStyle::default();
        style.all_caps = true;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Mixed case".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let text = layout_text(&layout.pages[0]);

        assert_eq!(text, "MIXED CASE");
    }

    #[test]
    fn lays_out_small_caps_lowercase_as_smaller_uppercase_fragments() {
        let mut document = Document::default();
        let mut style = CharacterStyle::default();
        style.small_caps = true;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Mix".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let text_fragments = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(layout_text(&layout.pages[0]), "MIX");
        assert_eq!(text_fragments.len(), 2);
        assert_eq!(text_fragments[0].text, "M");
        assert_eq!(text_fragments[0].style.font_size_half_points, 24);
        assert_eq!(text_fragments[1].text, "IX");
        assert_eq!(text_fragments[1].style.font_size_half_points, 17);
        assert!(
            (text_fragments[0].baseline_y - text_fragments[1].baseline_y).abs() < 0.01,
            "small caps fragments should share a stable baseline"
        );
    }
