    #[test]
    fn lays_out_table_text_and_grid_lines() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440, 1440],
            borders_visible: true,
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
                cells: vec![
                    TableCell {
                        nested_table: None,
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
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
                                text: "A".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    },
                    TableCell {
                        nested_table: None,
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
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
                                text: "B".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    },
                ],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        assert!(
            page.items
                .iter()
                .any(|item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "A"))
        );
        assert!(
            page.items
                .iter()
                .any(|item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "B"))
        );
        assert!(
            page.items
                .iter()
                .any(|item| matches!(item, LayoutItem::Line { .. }))
        );
    }

    #[test]
    fn table_column_widths_preserve_authored_widths_when_autofit_growth_is_disabled() {
        let mut table = Table {
            column_widths_twips: vec![4_000, 4_000],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: Vec::new(),
        };

        let scaled = table_column_widths(&table, 2, 288.0);
        assert!((scaled.iter().sum::<f32>() - 288.0).abs() < 0.01);

        table.preserve_authored_widths = true;
        let preserved = table_column_widths(&table, 2, 288.0);
        assert!((preserved[0] - 200.0).abs() < 0.01);
        assert!((preserved[1] - 200.0).abs() < 0.01);
        assert!((preserved.iter().sum::<f32>() - 400.0).abs() < 0.01);

        let row = TableRow {
            cells: Vec::new(),
            column_widths_twips: vec![2_400, 2_400],
            height_twips: None,
            left_offset_twips: 0,
            vertical_offset_twips: 0,
            wrap_margins: TableRowWrapMargins::default(),
            cell_gap_twips: 0,
            alignment: TableRowAlignment::Left,
            repeat_header: false,
            keep_together: false,
            keep_with_next: false,
            no_overlap: false,
        };
        let row_widths = table_row_column_widths(&table, &row, 2, 288.0);
        assert!((row_widths[0] - 120.0).abs() < 0.01);
        assert!((row_widths[1] - 120.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_borderless_table_text_without_grid_lines() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
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
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
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
                            text: "No borders".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert_eq!(layout_text(page), "No borders");
        assert!(
            !page
                .items
                .iter()
                .any(|item| matches!(item, LayoutItem::Line { .. }))
        );
    }

    #[test]
    fn lays_out_table_cell_side_borders() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
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
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders {
                        left: TableCellBorder {
                            visible: false,
                            ..TableCellBorder::default()
                        },
                        right: TableCellBorder::default(),
                        top: TableCellBorder::default(),
                        bottom: TableCellBorder::default(),
                        ..TableCellBorders::default()
                    },
                    fit_text: false,
                    text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: "Borders".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(!has_vertical_line_at(page, 72.0));
        assert!(has_vertical_line_at(page, 144.0));
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { y1, y2, .. } if (*y1 - *y2).abs() < 0.01
        )));
    }

    #[test]
    fn lays_out_table_cell_border_spacing_as_passive_offsets() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                column_widths_twips: Vec::new(),
                height_twips: Some(720),
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
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders {
                        left: TableCellBorder {
                            spacing_twips: 240,
                            ..TableCellBorder::default()
                        },
                        right: TableCellBorder {
                            spacing_twips: 120,
                            ..TableCellBorder::default()
                        },
                        top: TableCellBorder {
                            spacing_twips: 60,
                            ..TableCellBorder::default()
                        },
                        bottom: TableCellBorder {
                            spacing_twips: 180,
                            ..TableCellBorder::default()
                        },
                        ..TableCellBorders::default()
                    },
                    fit_text: false,
                    text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: "Spaced borders".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(has_vertical_line_at(page, 60.0));
        assert!(has_vertical_line_at(page, 150.0));
        assert!(has_horizontal_line_at(page, 72.0, 144.0, 723.0));
        assert!(has_horizontal_line_at(page, 72.0, 144.0, 674.5));
    }

    #[test]
    fn lays_out_table_cell_diagonal_borders_as_passive_lines() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                column_widths_twips: Vec::new(),
                height_twips: Some(720),
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
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders {
                        left: TableCellBorder {
                            visible: false,
                            ..TableCellBorder::default()
                        },
                        right: TableCellBorder {
                            visible: false,
                            ..TableCellBorder::default()
                        },
                        top: TableCellBorder {
                            visible: false,
                            ..TableCellBorder::default()
                        },
                        bottom: TableCellBorder {
                            visible: false,
                            ..TableCellBorder::default()
                        },
                        diagonal_down: TableCellBorder::default(),
                        diagonal_up: TableCellBorder {
                            width_twips: 40,
                            ..TableCellBorder::default()
                        },
                    },
                    fit_text: false,
                    text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: "Diagonal".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let diagonals = layout.pages[0]
            .items
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    LayoutItem::Line { x1, y1, x2, y2, .. }
                        if (*x1 - *x2).abs() > 0.01 && (*y1 - *y2).abs() > 0.01
                )
            })
            .count();

        assert_eq!(diagonals, 2);
    }

    #[test]
    fn word_diagonal_cells_ignore_row_borders_and_use_fine_dashes() {
        let parsed = parse_rtf(include_str!("../../fixtures/table-borders-passive.rtf")).unwrap();
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
                    style,
                    ..
                } => Some((*x1, *y1, *x2, *y2, *width, *style)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(lines.len(), 4, "Word omits all three row-level borders");
        let fine_dash = lines
            .iter()
            .find(|line| line.5 == LineStyle::FineDashed)
            .expect("fine dashed red diagonal");
        assert!((fine_dash.4 - 0.14).abs() < 0.001);
        assert!(lines.iter().any(|line| {
            line.5 == LineStyle::Solid
                && (line.4 - 1.5).abs() < 0.001
                && (line.0 - line.2).abs() > 0.01
                && (line.1 - line.3).abs() > 0.01
        }));
    }

    #[test]
    fn lays_out_table_cell_border_width_and_color() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
        ];
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
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
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders {
                        left: TableCellBorder {
                            visible: true,
                            width_twips: 80,
                            color_index: Some(1),
                            ..TableCellBorder::default()
                        },
                        right: TableCellBorder::default(),
                        top: TableCellBorder::default(),
                        bottom: TableCellBorder::default(),
                        ..TableCellBorders::default()
                    },
                    fit_text: false,
                    text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: "Border style".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, x2, width, color, .. }
                if (*x1 - 72.0).abs() < 0.01
                    && (*x2 - 72.0).abs() < 0.01
                    && (*width - 4.0).abs() < 0.01
                    && *color == PdfColor { red: 1.0, green: 0.0, blue: 0.0 }
        )));
    }

    #[test]
    fn authored_table_border_without_color_defaults_to_word_black() {
        let parsed =
            crate::rtf::parse_rtf(r"{\rtf1\trowd\clbrdrt\brdrs\brdrw20\cellx1440 Border\cell\row}")
                .unwrap();

        let layout = LayoutEngine::layout(&parsed.document);
        let authored_lines = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Line { color, .. } => Some(*color),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(!authored_lines.is_empty());
        assert!(
            authored_lines
                .iter()
                .all(|color| *color == PdfColor::default())
        );
    }

    #[test]
    fn strict_spec_lays_out_table_row_borders_from_normalized_cell_perimeter() {
        let parsed = crate::rtf::parse_rtf_bytes_with_options(
            br"{\rtf1{\colortbl;\red255\green0\blue0;}\trowd\trbrdrt\brdrdb\brdrw80\brdrcf1\trbrdrl\brdrdash\brdrw40\cellx1440 A\cell\cellx2880 B\cell\row}",
            &crate::config::RtfParseOptions {
                compatibility_mode: crate::config::CompatibilityMode::StrictSpec,
                ..crate::config::RtfParseOptions::default()
            },
        )
        .unwrap();

        let layout = LayoutEngine::layout(&parsed.document);
        let page = &layout.pages[0];
        let red = PdfColor {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
        };
        let top_segments = page
            .items
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    LayoutItem::Line {
                        y1,
                        y2,
                        width,
                        color,
                        style,
                        ..
                    } if (*y1 - *y2).abs() < 0.01
                        && (*width - 4.0).abs() < 0.01
                        && *color == red
                        && *style == LineStyle::Double
                )
            })
            .count();

        assert_eq!(top_segments, 2);
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, x2, width, style, .. }
                if (*x1 - *x2).abs() < 0.01
                    && (*width - 2.0).abs() < 0.01
                    && *style == LineStyle::Dashed
        )));
        assert!(layout_text(page).contains("A"));
        assert!(layout_text(page).contains("B"));
    }

    #[test]
    fn does_not_repeat_nested_grid_on_split_outer_row_fragments() {
        let mut input = String::from(
            r"{\rtf1{\colortbl;\red255\green220\blue180;}\trowd\cellx6000 Outer {\trowd\itap2\clcbpat1\clbrdrl\brdrs\clbrdrr\brdrs\clbrdrt\brdrs\clbrdrb\brdrs\cellx1000 Inner A\nestcell\clbrdrl\brdrs\clbrdrr\brdrs\clbrdrt\brdrs\clbrdrb\brdrs\cellx3000 Inner B\nestrow}",
        );
        for index in 0..100 {
            input.push_str(&format!(
                r"{{\trowd\itap2\clbrdrl\brdrs\clbrdrr\brdrs\clbrdrt\brdrs\clbrdrb\brdrs\cellx1000 Row {index}\nestcell\clbrdrl\brdrs\clbrdrr\brdrs\clbrdrt\brdrs\clbrdrb\brdrs\cellx3000 Value {index}\nestrow}}"
            ));
        }
        input.push_str(r" Outer after\cell\row}");

        let parsed = crate::rtf::parse_rtf(&input).unwrap();
        let layout = LayoutEngine::layout(&parsed.document);
        assert!(layout.pages.len() > 1);
        let line_count = layout
            .pages
            .iter()
            .flat_map(|page| page.items.iter())
            .filter(|item| matches!(item, LayoutItem::Line { .. }))
            .count();
        assert_eq!(line_count, 808);
        assert!(layout.pages.iter().skip(1).all(|page| {
            page.items
                .iter()
                .filter(|item| matches!(item, LayoutItem::Line { .. }))
                .count()
                > 4
        }));
        assert!(layout.pages.iter().any(|page| {
            page.items
                .iter()
                .any(|item| matches!(item, LayoutItem::Highlight { .. }))
        }));
    }

    #[test]
    fn preserves_nested_cell_boundaries_as_bounded_tab_stops() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1\trowd\cellx6000 Outer {\trowd\itap2\cellx1000 A\nestcell\cellx3000 B\nestrow}\cell\row}",
        )
        .unwrap();
        let table = match &parsed.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let paragraphs = table_cell_paragraphs_for_layout(&table.rows[0].cells[0]);
        let nested_row = paragraphs.last().expect("nested row paragraph");
        assert_eq!(nested_row.style.tab_stops_twips, vec![1000]);
        assert_eq!(nested_row.style.tab_stop_leaders, vec![TabLeader::None]);
        assert_eq!(
            nested_row.style.tab_stop_alignments,
            vec![TabAlignment::Left]
        );
    }

    #[test]
    fn applies_nested_cell_insets_to_row_origin_and_tab_targets() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1\trowd\cellx6000 Outer {\trowd\itap2\clpadl120\cellx1000 A\nestcell\clpadl80\cellx3000 B\nestrow}\cell\row}",
        )
        .unwrap();
        let table = match &parsed.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let paragraphs = table_cell_paragraphs_for_layout(&table.rows[0].cells[0]);
        let nested_row = paragraphs.last().expect("nested row paragraph");
        assert_eq!(nested_row.style.left_indent_twips, 120);
        assert_eq!(nested_row.style.tab_stops_twips, vec![1080]);
    }

    #[test]
    fn preserves_nested_row_padding_and_spacing_defaults() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1\trowd\cellx6000 Outer {\trowd\itap2\trpaddl60\trpaddr80\trspdl40\trspdr100\cellx2000 A\nestcell\cellx4000 B\nestrow}\cell\row}",
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
        assert_eq!(nested.rows[0].cells[0].padding.left_twips, Some(60));
        assert_eq!(nested.rows[0].cells[0].padding.right_twips, Some(80));
        assert_eq!(nested.rows[0].cells[0].spacing.left_twips, Some(40));
        assert_eq!(nested.rows[0].cells[0].spacing.right_twips, Some(100));
        let paragraphs = table_cell_paragraphs_for_layout(&table.rows[0].cells[0]);
        let nested_row = paragraphs.last().expect("nested row paragraph");
        assert_eq!(nested_row.style.left_indent_twips, 100);
        assert_eq!(nested_row.style.tab_stops_twips, vec![2100]);
    }

    #[test]
    fn preserves_nested_cell_paragraph_breaks_as_passive_line_breaks() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1\trowd\cellx6000 Outer {\trowd\itap2\cellx2000 A\par B\nestcell\cellx4000 C\nestrow}\cell\row}",
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
            "A\nB\tC"
        );
    }

    #[test]
    fn retains_nested_row_shading_defaults_for_passive_cells() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1\trowd\cellx6000 Outer {\trowd\itap2\trcbpat2\trshdng2500\cellx2000 A\nestcell\clcbpat1\cellx4000 B\nestrow}\cell\row}",
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
        assert_eq!(nested.rows[0].cells[0].shading_color_index, Some(2));
        assert_eq!(nested.rows[0].cells[0].shading_basis_points, 2500);
        assert_eq!(nested.rows[0].cells[1].shading_color_index, Some(1));
        assert_eq!(nested.rows[0].cells[1].shading_basis_points, 2500);
    }

    #[test]
    fn propagates_nested_row_keep_hints_to_passive_paragraph_flow() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1\trowd\cellx6000 Outer {\trowd\itap2\trcantSplit\trkeepfollow\cellx2000 A\nestcell\cellx4000 B\nestrow}\cell\row}",
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
        assert!(nested.rows[0].keep_together);
        assert!(nested.rows[0].keep_with_next);
        let paragraphs = table_cell_paragraphs_for_layout(&table.rows[0].cells[0]);
        let nested_row = paragraphs.last().expect("nested row paragraph");
        assert!(nested_row.style.keep_together);
        assert!(nested_row.style.keep_with_next);
    }

    #[test]
    fn preserves_nested_rtl_row_cell_order_as_passive_visual_order() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;\red0\green0\blue255;}\trowd\cellx6000 Outer {\trowd\itap2\rtlrow\clcbpat1\cellx2000 Right\nestcell\clcbpat2\cellx4000 Left\nestrow}\cell\row}",
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
        assert_eq!(nested.rows[0].cells[0].paragraphs[0].runs[0].text, "Left");
        assert_eq!(nested.rows[0].cells[1].paragraphs[0].runs[0].text, "Right");
        assert_eq!(nested.rows[0].cells[0].shading_color_index, Some(2));
        assert_eq!(nested.rows[0].cells[1].shading_color_index, Some(1));
        let paragraphs = table_cell_paragraphs_for_layout(&table.rows[0].cells[0]);
        let nested_row = paragraphs.last().expect("nested row paragraph");
        assert_eq!(
            nested_row
                .runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            "Left\tRight"
        );
    }

    #[test]
    fn applies_homogeneous_nested_cell_text_direction_to_passive_lines() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1\trowd\cellx6000 Outer {\trowd\itap2\cltxtbrl\cellx2000 A\nestcell\cltxtbrl\cellx4000 B\nestrow}\cell\row}",
        )
        .unwrap();
        let table = match &parsed.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let paragraphs = table_cell_paragraphs_for_layout(&table.rows[0].cells[0]);
        assert_eq!(
            paragraphs
                .last()
                .and_then(|paragraph| { paragraph.style.table_cell_text_direction }),
            Some(TableCellTextDirection::TopToBottomRightToLeft)
        );

        let layout = LayoutEngine::layout(&parsed.document);
        assert!(layout.pages.iter().any(|page| {
            page.items.iter().any(|item| {
                matches!(
                    item,
                    LayoutItem::Text(fragment)
                        if fragment.rotation == TextRotation::Clockwise90
                )
            })
        }));
    }

    #[test]
    fn preserves_nested_authored_row_height_in_containing_cell_flow() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1{\colortbl;\red220\green230\blue240;}\trowd\clcbpat1\cellx6000 Outer {\trowd\itap2\trrh2880\cellx2000 A\nestcell\cellx4000 B\nestrow}\cell\row}",
        )
        .unwrap();
        let layout = LayoutEngine::layout(&parsed.document);
        let outer_height = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Highlight { height, .. } => Some(*height),
                _ => None,
            })
            .fold(0.0, f32::max);
        assert!(
            outer_height >= 140.0,
            "nested row height collapsed: {outer_height}"
        );
    }

    #[test]
    fn uses_nested_preferred_cell_widths_without_cell_boundaries() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1\trowd\cellx6000 Outer {\trowd\itap2\clftsWidth3\clwWidth1000 A\nestcell\clftsWidth3\clwWidth3000 B\nestrow}\cell\row}",
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
        assert_eq!(nested.column_widths_twips, vec![1000, 3000]);
        let paragraphs = table_cell_paragraphs_for_layout(&table.rows[0].cells[0]);
        let nested_row = paragraphs.last().expect("nested row paragraph");
        assert_eq!(nested_row.style.tab_stops_twips, vec![1000]);
    }

    #[test]
    fn uses_nested_preferred_row_width_without_cell_widths() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1\trowd\cellx6000 Outer {\trowd\itap2\trftsWidth3\trwWidth4000 A\nestcell B\nestrow}\cell\row}",
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
        assert_eq!(nested.column_widths_twips, vec![2000, 2000]);
    }

    #[test]
    fn preserves_nested_repeat_header_row_state() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1\trowd\cellx6000 Outer {\trowd\itap2\trhdr\cellx2000 Header\nestcell\cellx4000 H2\nestrow}{\trowd\itap2\trhdr0\cellx2000 Body\nestcell\cellx4000 B2\nestrow}\cell\row}",
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
        assert!(nested.rows[0].repeat_header);
        assert!(!nested.rows[1].repeat_header);
    }

    #[test]
    fn repeats_nested_header_text_on_split_outer_row_fragments() {
        let mut input = String::from(
            r"{\rtf1\trowd\cellx6000 Outer {\trowd\itap2\trhdr\cellx2000 Nested Header\nestcell\cellx4000 Header Value\nestrow}",
        );
        for index in 0..80 {
            input.push_str(&format!(
                r"{{\trowd\itap2\trhdr0\cellx2000 Nested Row {index}\nestcell\cellx4000 Value {index}\nestrow}}"
            ));
        }
        input.push_str(r" Outer after\cell\row}");

        let parsed = crate::rtf::parse_rtf(&input).unwrap();
        let layout = LayoutEngine::layout(&parsed.document);
        assert!(layout.pages.len() > 1);
        assert!(layout_text(&layout.pages[0]).contains("Nested Header"));
        assert!(
            layout
                .pages
                .iter()
                .skip(1)
                .map(layout_text)
                .any(|text| text.contains("Nested Header"))
        );
    }

    #[test]
    fn maps_nested_tab_alignment_from_visual_cells_after_merge_continuations() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1\trowd\cellx6000 Outer {\trowd\itap2\clmgf\cellx1000 A\nestcell\clmrg\cellx3000 B\nestcell\cellx5000\qr C\nestrow}\cell\row}",
        )
        .unwrap();
        let table = match &parsed.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let paragraphs = table_cell_paragraphs_for_layout(&table.rows[0].cells[0]);
        let nested_row = paragraphs.last().expect("nested row paragraph");
        assert_eq!(nested_row.style.tab_stops_twips, vec![3000]);
        assert_eq!(
            nested_row.style.tab_stop_alignments,
            vec![TabAlignment::Right]
        );
    }

    #[test]
    fn emits_nested_child_table_after_its_containing_row() {
        let paragraph = |text: &str| Paragraph {
            style: ParagraphStyle::default(),
            runs: vec![Run {
                text: text.to_string(),
                style: CharacterStyle::default(),
            }],
        };
        let cell = |paragraphs: Vec<Paragraph>, nested_table: Option<Box<Table>>| TableCell {
            paragraphs,
            nested_table,
            shading_color_index: None,
            shading_basis_points: 10_000,
            shading_pattern: ShadingPattern::None,
            padding: TableCellPadding::default(),
            spacing: TableCellSpacing::default(),
            borders: TableCellBorders::default(),
            fit_text: false,
            text_direction: TableCellTextDirection::default(),
            vertical_align: TableCellVerticalAlign::default(),
            horizontal_merge: TableCellHorizontalMerge::default(),
            vertical_merge: TableCellVerticalMerge::default(),
        };
        let row = |cells: Vec<TableCell>| TableRow {
            column_widths_twips: Vec::new(),
            cells,
            height_twips: None,
            left_offset_twips: 0,
            vertical_offset_twips: 0,
            wrap_margins: TableRowWrapMargins::default(),
            cell_gap_twips: 0,
            alignment: TableRowAlignment::Left,
            repeat_header: false,
            keep_together: false,
            keep_with_next: false,
            no_overlap: false,
        };
        let grandchild = Table {
            rows: vec![row(vec![cell(vec![paragraph("deep")], None)])],
            column_widths_twips: vec![720],
            borders_visible: true,
            preserve_authored_widths: false,
        };
        let nested = Table {
            rows: vec![row(vec![cell(
                vec![paragraph("nested")],
                Some(Box::new(grandchild)),
            )])],
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
        };
        let paragraphs = table_cell_paragraphs_for_layout(&cell(
            vec![paragraph("outer")],
            Some(Box::new(nested.clone())),
        ));
        let text = paragraphs
            .iter()
            .map(|paragraph| {
                paragraph
                    .runs
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        assert_eq!(text, vec!["outer", "nested", "deep"]);

        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            rows: vec![row(vec![cell(
                vec![paragraph("outer")],
                Some(Box::new(nested.clone())),
            )])],
            column_widths_twips: vec![2880],
            borders_visible: true,
            preserve_authored_widths: false,
        })];
        let outer_table = match &document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected outer table"),
        };
        let prepared = prepare_table_row(
            &outer_table.rows[0],
            &[144.0],
            144.0,
            &test_markers("1", "1"),
            &document,
            None,
        );
        assert_eq!(
            nested_table_row_range(&outer_table.rows[0].cells[0], lines_for_cell(&prepared, 0)),
            Some((0, 1)),
            "descendant paragraphs must not expand the parent nested-grid row range"
        );
        let layout = LayoutEngine::layout(&document);
        let line_count = layout.pages[0]
            .items
            .iter()
            .filter(|item| matches!(item, LayoutItem::Line { .. }))
            .count();
        assert!(
            line_count >= 8,
            "recursive nested grids should render both levels: {line_count}"
        );

        let mut first_fragment_layout = LayoutEngine::layout(&Document::default());
        push_nested_table_grid(
            &mut first_fragment_layout.pages,
            &nested,
            &Document::default(),
            (0, 1),
            0.0,
            100.0,
            100.0,
            100.0,
            false,
        );
        let first_fragment_line_count = first_fragment_layout.pages[0]
            .items
            .iter()
            .filter(|item| matches!(item, LayoutItem::Line { .. }))
            .count();
        assert_eq!(
            first_fragment_line_count, 4,
            "continuation fragments should omit descendant grids"
        );
    }

    #[test]
    fn preserves_nested_table_text_at_its_authored_cell_position() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1\ansi\trowd\cellx6000 Outer before {\trowd\itap2\cellx1000 Inner A\nestcell\cellx2000 Inner B\nestrow} Outer after\cell\row}",
        )
        .expect("nested table fixture should parse");
        let table = match &parsed.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let paragraphs = table_cell_paragraphs_for_layout(&table.rows[0].cells[0]);
        let text = paragraphs
            .iter()
            .flat_map(|paragraph| paragraph.runs.iter())
            .map(|run| run.text.as_str())
            .collect::<String>();
        assert!(text.find("Outer before").unwrap() < text.find("Inner A").unwrap());
        assert!(text.find("Inner B").unwrap() < text.find("Outer after").unwrap());
        assert!(!text.contains(NESTED_TABLE_ANCHOR_MARKER));
    }

    #[test]
    fn anchors_nested_grid_range_after_prior_cell_paragraphs() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1\trowd\cellx6000 First paragraph\par Outer before {\trowd\itap2\cellx1000 Inner\nestcell\cellx2000 Value\nestrow} Outer after\cell\row}",
        )
        .expect("nested paragraph-anchor fixture should parse");
        let table = match &parsed.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let prepared = prepare_table_row(
            &table.rows[0],
            &[300.0],
            300.0,
            &test_markers("1", "1"),
            &parsed.document,
            None,
        );

        assert_eq!(
            nested_table_row_range(&table.rows[0].cells[0], lines_for_cell(&prepared, 0)),
            Some((0, 1))
        );
    }

    #[test]
    fn lays_out_shaded_table_cell_background() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 220,
                green: 230,
                blue: 240,
            },
        ];
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
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
                    shading_pattern: crate::model::ShadingPattern::None,
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
                            text: "Shaded".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(|item| {
            matches!(
                item,
                LayoutItem::Highlight { color, .. }
                    if *color == PdfColor {
                        red: 220.0 / 255.0,
                        green: 230.0 / 255.0,
                        blue: 240.0 / 255.0
                    }
            )
        }));
        assert!(layout_text(page).contains("Shaded"));
    }

    #[test]
    fn lays_out_row_default_table_shading_from_normalized_cells() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1{\colortbl;\red220\green230\blue240;}\trowd\trcbpat1\cellx1440 A\cell\cellx2880 B\cell\row}",
        )
        .unwrap();

        let layout = LayoutEngine::layout(&parsed.document);
        let page = &layout.pages[0];
        let expected_color = PdfColor {
            red: 220.0 / 255.0,
            green: 230.0 / 255.0,
            blue: 240.0 / 255.0,
        };
        let row_backgrounds = page
            .items
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    LayoutItem::Highlight { width, color, .. }
                        if (*width - 72.0).abs() < 0.01 && *color == expected_color
                )
            })
            .count();

        assert_eq!(row_backgrounds, 2);
        assert!(layout_text(page).contains("A"));
        assert!(layout_text(page).contains("B"));
    }

    #[test]
    fn lays_out_table_row_minimum_height_plus_top_border_width() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 240,
                green: 240,
                blue: 240,
            },
        ];
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                column_widths_twips: Vec::new(),
                height_twips: Some(720),
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
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders {
                        top: TableCellBorder {
                            width_twips: 20,
                            ..TableCellBorder::default()
                        },
                        ..TableCellBorders::default()
                    },
                    fit_text: false,
                    text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: "Tall".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let height = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight { height, .. } => Some(*height),
                _ => None,
            })
            .expect("row background");

        assert!((height - 37.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_negative_table_row_height_as_exact_height() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 240,
                green: 240,
                blue: 240,
            },
        ];
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                column_widths_twips: Vec::new(),
                height_twips: Some(-360),
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
                    shading_pattern: crate::model::ShadingPattern::None,
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
                            text: "Exact row with text that would otherwise need more space"
                                .to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let height = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight { height, .. } => Some(*height),
                _ => None,
            })
            .expect("row background");

        assert!((height - 18.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_table_cell_text_direction_as_rotated_passive_text() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            borders_visible: true,
            preserve_authored_widths: true,
            column_widths_twips: vec![1440, 1440],
            rows: vec![TableRow {
                column_widths_twips: Vec::new(),
                height_twips: Some(1440),
                left_offset_twips: 0,
                vertical_offset_twips: 0,
                wrap_margins: TableRowWrapMargins::default(),
                cell_gap_twips: 0,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                keep_with_next: false,
                no_overlap: false,
                cells: vec![
                    TableCell {
                        nested_table: None,
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        text_direction: TableCellTextDirection::TopToBottomRightToLeft,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "ABC".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    },
                    TableCell {
                        nested_table: None,
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        text_direction: TableCellTextDirection::BottomToTopLeftToRight,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "XY".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    },
                ],
            }],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(|item| matches!(
            item,
            LayoutItem::Text(fragment)
                if fragment.text == "ABC" && fragment.rotation == TextRotation::Clockwise90
        )));
        assert!(layout.pages[0].items.iter().any(|item| matches!(
            item,
            LayoutItem::Text(fragment)
                if fragment.text == "XY" && fragment.rotation == TextRotation::CounterClockwise90
        )));
    }

    #[test]
    fn wraps_vertical_fixture_text_against_the_authored_row_extent() {
        let parsed = parse_rtf(include_str!(
            "../../fixtures/table-cell-text-direction-passive.rtf"
        ))
        .expect("text-direction fixture should parse");
        let table = parsed
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Table(table) => Some(table),
                _ => None,
            })
            .expect("expected table block");
        let prepared = prepare_table_row(
            &table.rows[0],
            &[60.0, 60.0, 60.0, 90.0],
            90.0,
            &test_markers("1", "1"),
            &parsed.document,
            None,
        );
        let layout = LayoutEngine::layout(&parsed.document);

        assert_eq!(prepared.cell_lines[0].len(), 1);
        assert_eq!(prepared.cell_lines[1].len(), 1);
        assert_eq!(prepared.cell_lines[2].len(), 2);
        assert!((prepared.row_height - 73.0).abs() < 0.01);
        let fragments = layout
            .pages
            .iter()
            .flat_map(|page| page.items.iter())
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment),
                _ => None,
            })
            .collect::<Vec<_>>();
        let fragment = |needle: &str| {
            fragments
                .iter()
                .copied()
                .find(|fragment| fragment.text.trim() == needle)
                .unwrap_or_else(|| panic!("missing `{needle}` fragment"))
        };
        let vertical = fragment("Vertical");
        let abc = fragment("ABC");
        let bottom = fragment("Bottom");
        let top = fragment("top");
        let xy = fragment("XY");

        assert_eq!(vertical.rotation, TextRotation::Clockwise90);
        assert!((vertical.x - abc.x).abs() < 0.01);
        assert!(abc.baseline_y < vertical.baseline_y);
        assert_eq!(bottom.rotation, TextRotation::CounterClockwise90);
        assert!((bottom.x - top.x).abs() < 0.01);
        assert!(top.baseline_y > bottom.baseline_y);
        assert!(xy.x > bottom.x);
    }

    #[test]
    fn clips_table_cell_lines_to_exact_row_height() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 240,
                green: 240,
                blue: 240,
            },
        ];
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                column_widths_twips: Vec::new(),
                height_twips: Some(-360),
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
                    shading_pattern: crate::model::ShadingPattern::None,
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
                            text: "Visible\nOverflow".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let text = layout_text(&layout.pages[0]);

        assert!(text.contains("Visible"));
        assert!(!text.contains("Overflow"));
    }

    #[test]
    fn lays_out_table_cell_paragraph_line_spacing() {
        let mut document = Document::default();
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.line_spacing_twips = Some(-480);
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
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
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: paragraph_style,
                        runs: vec![Run {
                            text: "First\nSecond".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let baselines = text_baselines(&layout.pages[0]);

        assert_eq!(baselines.len(), 2);
        assert!((baselines[0] - baselines[1] - 24.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_table_cell_paragraph_spacing_in_row_height() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 240,
                green: 240,
                blue: 240,
            },
        ];
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.space_before_twips = 240;
        paragraph_style.space_after_twips = 360;
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
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
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: paragraph_style,
                        runs: vec![Run {
                            text: "Spaced".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let height = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight { height, .. } => Some(*height),
                _ => None,
            })
            .expect("row background");

        assert!((height - 45.5).abs() < 0.01);
    }

    #[test]
    fn collapses_contextual_spacing_between_table_cell_paragraphs() {
        let normal_height = table_cell_two_paragraph_height(false);
        let contextual_height = table_cell_two_paragraph_height(true);

        assert!(
            (normal_height - contextual_height - 30.0).abs() < 0.01,
            "contextual spacing should suppress 18pt after + 12pt before inside table cells, normal={normal_height}, contextual={contextual_height}"
        );
    }

    #[test]
    fn lays_out_table_row_left_offset() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                column_widths_twips: Vec::new(),
                height_twips: None,
                left_offset_twips: 720,
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
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
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
                            text: "Offset".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let text_x = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "Offset" => Some(fragment.x),
                _ => None,
            })
            .expect("offset text");

        assert!((text_x - 111.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_table_row_vertical_offset_as_passive_top_adjustment() {
        fn baseline_for_row_offset(vertical_offset_twips: i32) -> f32 {
            let mut document = Document::default();
            document.blocks = vec![Block::Table(Table {
                column_widths_twips: vec![1440],
                borders_visible: true,
                preserve_authored_widths: false,
                rows: vec![TableRow {
                    column_widths_twips: Vec::new(),
                    height_twips: None,
                    left_offset_twips: 0,
                    vertical_offset_twips,
                    wrap_margins: TableRowWrapMargins::default(),
                    cell_gap_twips: 60,
                    alignment: TableRowAlignment::Left,
                    repeat_header: false,
                    keep_together: false,
                    keep_with_next: false,
                    no_overlap: false,
                    cells: vec![TableCell {
                        nested_table: None,
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
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
                                text: "Offset".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    }],
                }],
            })];

            LayoutEngine::layout(&document).pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text == "Offset" => {
                        Some(fragment.baseline_y)
                    }
                    _ => None,
                })
                .expect("offset text")
        }

        let default_baseline = baseline_for_row_offset(0);
        let offset_baseline = baseline_for_row_offset(720);

        assert!((default_baseline - offset_baseline - 36.0).abs() < 0.01);
    }

    #[test]
    fn dynamic_height_floating_table_bottom_alignment_uses_computed_row_height() {
        fn baseline_for(input: &str, text: &str) -> f32 {
            let parsed = parse_rtf(input).unwrap();
            LayoutEngine::layout(&parsed.document).pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text == text => {
                        Some(fragment.baseline_y)
                    }
                    _ => None,
                })
                .expect("table text")
        }

        let top_baseline = baseline_for(r"{\rtf1\trowd\cellx2000 Top\cell\row}", "Top");
        let bottom_baseline =
            baseline_for(r"{\rtf1\trowd\tposyb\cellx2000 Bottom\cell\row}", "Bottom");

        assert!(bottom_baseline > top_baseline + 10.0);
        assert!(bottom_baseline < top_baseline + 20.0);
    }

    #[test]
    fn lays_out_table_row_wrap_margins_as_passive_outer_spacing() {
        fn row(text: &str, wrap_margins: TableRowWrapMargins) -> TableRow {
            TableRow {
                column_widths_twips: Vec::new(),
                height_twips: None,
                left_offset_twips: 0,
                vertical_offset_twips: 0,
                wrap_margins,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                keep_with_next: false,
                no_overlap: false,
                cells: vec![TableCell {
                    nested_table: None,
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
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
                            text: text.to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }
        }

        fn positions(wrap_margins: TableRowWrapMargins) -> (f32, f32, f32) {
            let mut document = Document::default();
            document.blocks = vec![Block::Table(Table {
                column_widths_twips: vec![1440],
                borders_visible: true,
                preserve_authored_widths: false,
                rows: vec![
                    row("First", wrap_margins),
                    row("Second", TableRowWrapMargins::default()),
                ],
            })];

            let layout = LayoutEngine::layout(&document);
            let mut first = None;
            let mut second_baseline = None;
            for item in &layout.pages[0].items {
                match item {
                    LayoutItem::Text(fragment) if fragment.text == "First" => {
                        first = Some((fragment.x, fragment.baseline_y));
                    }
                    LayoutItem::Text(fragment) if fragment.text == "Second" => {
                        second_baseline = Some(fragment.baseline_y);
                    }
                    _ => {}
                }
            }
            let (first_x, first_baseline) = first.expect("first row text");
            (
                first_x,
                first_baseline,
                second_baseline.expect("second row text"),
            )
        }

        let default = positions(TableRowWrapMargins::default());
        let wrapped = positions(TableRowWrapMargins {
            left_twips: 240,
            right_twips: 0,
            top_twips: 120,
            bottom_twips: 240,
        });

        assert!((wrapped.0 - default.0).abs() < 0.01);
        assert!((default.1 - wrapped.1 - 6.0).abs() < 0.01);

        let default_gap = default.1 - default.2;
        let wrapped_gap = wrapped.1 - wrapped.2;
        assert!((wrapped_gap - default_gap - 12.0).abs() < 0.01);
    }

    #[test]
    fn wrapped_table_row_excludes_following_paragraph_lines() {
        let mut document = Document::default();
        document.blocks = vec![
            Block::Table(Table {
                column_widths_twips: vec![1440],
                borders_visible: true,
                preserve_authored_widths: false,
                rows: vec![TableRow {
                    column_widths_twips: Vec::new(),
                    height_twips: Some(720),
                    left_offset_twips: 0,
                    vertical_offset_twips: 0,
                    wrap_margins: TableRowWrapMargins {
                        left_twips: 0,
                        right_twips: 240,
                        top_twips: 0,
                        bottom_twips: 0,
                    },
                    cell_gap_twips: 60,
                    alignment: TableRowAlignment::Left,
                    repeat_header: false,
                    keep_together: false,
                    keep_with_next: false,
                    no_overlap: false,
                    cells: vec![TableCell {
                        nested_table: None,
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
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
                                text: "Floating".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    }],
                }],
            }),
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Wrapped paragraph beside table".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        assert_eq!(page.flow_exclusions.len(), 1);
        let table_text = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "Floating" => Some(fragment),
                _ => None,
            })
            .expect("table text");
        let wrapped_text = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text.contains("Wrapped") => Some(fragment),
                _ => None,
            })
            .expect("wrapped paragraph text");

        assert!(wrapped_text.x > table_text.x + 72.0);
        assert!((wrapped_text.baseline_y - table_text.baseline_y).abs() < 14.0);
    }

    #[test]
    fn positioned_table_row_from_rtf_excludes_following_paragraph_lines() {
        let parsed = parse_rtf(
            r"{\rtf1\trowd\tposx0\trrh720\cellx1440 Floating\cell\row\pard Wrapped paragraph beside table\par}",
        )
        .unwrap();

        let layout = LayoutEngine::layout(&parsed.document);
        let page = &layout.pages[0];
        assert_eq!(page.flow_exclusions.len(), 1);
        let table_text = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "Floating" => Some(fragment),
                _ => None,
            })
            .expect("table text");
        let wrapped_text = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text.contains("Wrapped") => Some(fragment),
                _ => None,
            })
            .expect("wrapped paragraph text");

        assert!(wrapped_text.x > table_text.x + 72.0);
        assert!((wrapped_text.baseline_y - table_text.baseline_y).abs() < 14.0);
        assert!(parsed.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "floating table default wrap distance interpreted as bounded passive row margin",
            )
        }));
    }

    #[test]
    fn no_overlap_floating_table_starts_below_previous_passive_exclusion() {
        let parsed = parse_rtf(
            r"{\rtf1\trowd\tposx0\trrh720\cellx1440 First floating\cell\row\pard\par\trowd\tabsnoovrlp\tposx0\trrh720\cellx1440 Second floating\cell\row}",
        )
        .unwrap();

        let layout = LayoutEngine::layout(&parsed.document);
        let page = &layout.pages[0];
        assert!(
            layout_text(page).contains("floating"),
            "layout text: {:?}",
            layout_text(page)
        );
        let first = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text.trim() == "First" => Some(fragment),
                _ => None,
            })
            .unwrap_or_else(|| panic!("first floating text in {:?}", layout_text(page)));
        let second = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text.trim() == "Second" => Some(fragment),
                _ => None,
            })
            .unwrap_or_else(|| panic!("second floating text in {:?}", layout_text(page)));

        assert_eq!(page.flow_exclusions.len(), 2);
        assert!(
            second.baseline_y < first.baseline_y - 24.0,
            "no-overlap row should move below the earlier passive exclusion: first={}, second={}",
            first.baseline_y,
            second.baseline_y
        );
    }

    #[test]
    fn lays_out_table_row_horizontal_alignment() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![
                TableRow {
                    column_widths_twips: Vec::new(),
                    height_twips: None,
                    left_offset_twips: 0,
                    vertical_offset_twips: 0,
                    wrap_margins: TableRowWrapMargins::default(),
                    cell_gap_twips: 60,
                    alignment: TableRowAlignment::Center,
                    repeat_header: false,
                    keep_together: false,
                    keep_with_next: false,
                    no_overlap: false,
                    cells: vec![TableCell {
                        nested_table: None,
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
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
                                text: "Center".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    }],
                },
                TableRow {
                    column_widths_twips: Vec::new(),
                    height_twips: None,
                    left_offset_twips: 0,
                    vertical_offset_twips: 0,
                    wrap_margins: TableRowWrapMargins::default(),
                    cell_gap_twips: 60,
                    alignment: TableRowAlignment::Right,
                    repeat_header: false,
                    keep_together: false,
                    keep_with_next: false,
                    no_overlap: false,
                    cells: vec![TableCell {
                        nested_table: None,
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
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
                                text: "Right".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    }],
                },
            ],
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
                .expect("aligned row text")
        };

        assert!((text_x("Center") - 273.0).abs() < 0.01);
        assert!((text_x("Right") - 471.0).abs() < 0.01);
    }

    #[test]
    fn mirrored_pages_resolve_inside_outside_table_alignment_by_parity() {
        fn aligned_row(text: &str, alignment: TableRowAlignment) -> TableRow {
            TableRow {
                column_widths_twips: Vec::new(),
                height_twips: None,
                left_offset_twips: 0,
                vertical_offset_twips: 0,
                wrap_margins: TableRowWrapMargins::default(),
                cell_gap_twips: 60,
                alignment,
                repeat_header: false,
                keep_together: false,
                keep_with_next: false,
                no_overlap: false,
                cells: vec![TableCell {
                    nested_table: None,
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
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
                            text: text.to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }
        }

        let mut document = Document::default();
        document.page.facing_pages = true;
        document.page.mirror_margins = true;
        document.blocks = vec![
            Block::Table(Table {
                column_widths_twips: vec![1440],
                borders_visible: true,
                preserve_authored_widths: false,
                rows: vec![
                    aligned_row("OddIn", TableRowAlignment::Inside),
                    aligned_row("OddOut", TableRowAlignment::Outside),
                ],
            }),
            Block::PageBreak,
            Block::Table(Table {
                column_widths_twips: vec![1440],
                borders_visible: true,
                preserve_authored_widths: false,
                rows: vec![
                    aligned_row("EvenIn", TableRowAlignment::Inside),
                    aligned_row("EvenOut", TableRowAlignment::Outside),
                ],
            }),
        ];

        let mut non_facing_document = document.clone();
        non_facing_document.page.facing_pages = false;
        let non_facing_layout = LayoutEngine::layout(&non_facing_document);
        let layout = LayoutEngine::layout(&document);
        let page_text_x = |page_idx: usize, text: &str| {
            layout.pages[page_idx]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.trim() == text => Some(fragment.x),
                    _ => None,
                })
                .expect("aligned table text")
        };

        assert!((page_text_x(0, "OddIn") - 75.0).abs() < 0.01);
        assert!((page_text_x(0, "OddOut") - 471.0).abs() < 0.01);
        assert!((page_text_x(1, "EvenIn") - 471.0).abs() < 0.01);
        assert!((page_text_x(1, "EvenOut") - 75.0).abs() < 0.01);
        let non_facing_page_text_x = |page_idx: usize, text: &str| {
            non_facing_layout.pages[page_idx]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.trim() == text => Some(fragment.x),
                    _ => None,
                })
                .expect("non-facing aligned table text")
        };
        assert!((non_facing_page_text_x(1, "EvenIn") - 75.0).abs() < 0.01);
        assert!((non_facing_page_text_x(1, "EvenOut") - 471.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_table_cell_gap_as_padding() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                column_widths_twips: Vec::new(),
                height_twips: None,
                left_offset_twips: 0,
                vertical_offset_twips: 0,
                wrap_margins: TableRowWrapMargins::default(),
                cell_gap_twips: 240,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                keep_with_next: false,
                no_overlap: false,
                cells: vec![TableCell {
                    nested_table: None,
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
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
                            text: "Padded".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let text_x = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "Padded" => Some(fragment.x),
                _ => None,
            })
            .expect("padded text");

        assert!((text_x - 84.0).abs() < 0.01);
    }

    #[test]
    fn table_cell_gap_does_not_add_implicit_vertical_padding() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 240,
                green: 240,
                blue: 240,
            },
        ];
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                column_widths_twips: Vec::new(),
                height_twips: Some(489),
                left_offset_twips: 0,
                vertical_offset_twips: 0,
                wrap_margins: TableRowWrapMargins::default(),
                cell_gap_twips: 108,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                keep_with_next: false,
                no_overlap: false,
                cells: vec![TableCell {
                    nested_table: None,
                    shading_color_index: Some(1),
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
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
                            text: "Compact".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let row_height = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight { height, .. } => Some(*height),
                _ => None,
            })
            .expect("row background");
        let text_x = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "Compact" => Some(fragment.x),
                _ => None,
            })
            .expect("compact cell text");

        assert!((row_height - twips_to_points(489) - 0.5).abs() < 0.01);
        assert!((text_x - 77.4).abs() < 0.01);
    }

    #[test]
    fn final_table_cell_gap_controls_following_paragraph_flow() {
        let following_y = |cell_gap: &str| {
            let source =
                format!(r"{{\rtf1\trowd{cell_gap}\cellx2000 Cell\cell\row\pard After\par}}");
            let parsed = crate::rtf::parse_rtf(&source).unwrap();
            let layout = LayoutEngine::layout(&parsed.document);
            layout.pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.trim() == "After" => {
                        Some(fragment.baseline_y)
                    }
                    _ => None,
                })
                .expect("following paragraph")
        };

        let without_gap = following_y("");
        let with_gap = following_y(r"\trgaph108");
        assert!((without_gap - with_gap - 2.7).abs() < 0.01);
    }

    #[test]
    fn final_table_bottom_padding_replaces_legacy_cell_gap_clearance() {
        let following_y = |row_controls: &str| {
            let source =
                format!(r"{{\rtf1\trowd{row_controls}\cellx2000 Cell\cell\row\pard After\par}}");
            let parsed = crate::rtf::parse_rtf(&source).unwrap();
            let layout = LayoutEngine::layout(&parsed.document);
            layout.pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.trim() == "After" => {
                        Some(fragment.baseline_y)
                    }
                    _ => None,
                })
                .expect("following paragraph")
        };

        let without_gap = following_y(r"\trgaph0\trpaddb120");
        let with_legacy_gap = following_y(r"\trgaph108\trpaddb120");
        assert!((without_gap - with_legacy_gap).abs() < 0.01);
    }

    #[test]
    fn auto_height_table_rows_reserve_the_top_cell_border_width() {
        let positions = |border: &str| {
            let source =
                format!(r"{{\rtf1\trowd\trgaph0{border}\cellx2000 Cell\cell\row\pard After\par}}");
            let parsed = crate::rtf::parse_rtf(&source).unwrap();
            let layout = LayoutEngine::layout(&parsed.document);
            let baseline = |target: &str| {
                layout.pages[0]
                    .items
                    .iter()
                    .find_map(|item| match item {
                        LayoutItem::Text(fragment) if fragment.text.trim() == target => {
                            Some(fragment.baseline_y)
                        }
                        _ => None,
                    })
                    .expect("target text")
            };
            (baseline("Cell"), baseline("After"))
        };

        let plain = positions("");
        let bordered = positions(r"\clbrdrt\brdrs\brdrw30");
        assert!((plain.0 - bordered.0 - 1.5).abs() < 0.01);
        assert!((plain.1 - bordered.1 - 1.5).abs() < 0.01);
    }

    #[test]
    fn final_table_bottom_border_replaces_cell_gap_flow_clearance() {
        let following_y = |row_controls: &str| {
            let source =
                format!(r"{{\rtf1\trowd{row_controls}\cellx2000 Cell\cell\row\pard After\par}}");
            let parsed = crate::rtf::parse_rtf(&source).unwrap();
            let layout = LayoutEngine::layout(&parsed.document);
            layout.pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.trim() == "After" => {
                        Some(fragment.baseline_y)
                    }
                    _ => None,
                })
                .expect("following paragraph")
        };

        let without_gap = following_y(r"\trgaph0");
        let with_border = following_y(r"\trgaph108\clbrdrb\brdrs\brdrw30");
        let with_gap = following_y(r"\trgaph108");
        assert!((without_gap - with_border - 1.5).abs() < 0.01);
        assert!((without_gap - with_gap - 2.7).abs() < 0.01);
    }

    #[test]
    fn lays_out_row_default_table_padding_from_normalized_cells() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1\trowd\trpaddl360\trpaddr120\cellx1440 A\cell\cellx2880 B\cell\row}",
        )
        .unwrap();

        let layout = LayoutEngine::layout(&parsed.document);
        let text_x = |text: &str| {
            layout.pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.trim() == text => Some(fragment.x),
                    _ => None,
                })
                .expect("padded cell text")
        };

        assert!((text_x("A") - 90.0).abs() < 0.01);
        assert!((text_x("B") - 162.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_table_cell_paragraph_alignment() {
        let mut center_style = ParagraphStyle::default();
        center_style.alignment = Alignment::Center;
        let mut right_style = ParagraphStyle::default();
        right_style.alignment = Alignment::Right;

        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![
                TableRow {
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
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: center_style,
                            runs: vec![Run {
                                text: "A".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    }],
                },
                TableRow {
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
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: right_style,
                            runs: vec![Run {
                                text: "B".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    }],
                },
            ],
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
                .expect("cell text")
        };

        let style = CharacterStyle::default();
        let text_width = measure_text_with_family("A", &style, PdfFontFamily::Helvetica);
        let content_left = 75.0;
        let content_width = 66.0;

        assert!((text_x("A") - (content_left + (content_width - text_width) / 2.0)).abs() < 0.01);
        assert!((text_x("B") - (content_left + content_width - text_width)).abs() < 0.01);
    }

    #[test]
    fn lays_out_table_cell_paragraph_borders() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 40,
            style: BorderStyle::Single,
            ..TableCellBorder::default()
        };

        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
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
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: paragraph_style,
                        runs: vec![Run {
                            text: "Cell".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(
            |item| matches!(item, LayoutItem::Line { x1, x2, .. } if (*x2 - *x1).abs() > 60.0)
        ));
    }

    #[test]
    fn lays_out_table_cell_first_line_indent_only_on_first_line() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.left_indent_twips = 360;
        paragraph_style.first_line_indent_twips = 360;

        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
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
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: paragraph_style,
                        runs: vec![Run {
                            text: "First\nSecond".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
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
                .expect("cell text")
        };

        assert!((text_x("First") - 111.0).abs() < 0.01);
        assert!((text_x("Second") - 93.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_table_cell_fit_text_as_bounded_passive_scaling() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![720],
            borders_visible: true,
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
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: true,
                    text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: "Fit text stays on one visual row".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
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
        let first_baseline = fragments
            .first()
            .map(|fragment| fragment.baseline_y)
            .expect("fit text fragments");

        assert!(
            fragments.iter().all(|fragment| {
                fragment.style.character_scaling_percent < 100
                    && (fragment.baseline_y - first_baseline).abs() < 0.01
            }),
            "fit text should render as one passively scaled visual row: {fragments:?}"
        );
    }

    #[test]
    fn lays_out_explicit_table_cell_padding() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 240,
                green: 240,
                blue: 240,
            },
        ];
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
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
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding {
                        left_twips: Some(240),
                        right_twips: Some(120),
                        top_twips: Some(240),
                        bottom_twips: Some(120),
                    },
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
                            text: "Padded".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let text_x = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "Padded" => Some(fragment.x),
                _ => None,
            })
            .expect("padded text");
        let row_height = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight { height, .. } => Some(*height),
                _ => None,
            })
            .expect("row background");

        assert!((text_x - 84.0).abs() < 0.01);
        assert!((row_height - 33.5).abs() < 0.01);
    }

    #[test]
    fn lays_out_table_cell_spacing_as_passive_border_gaps() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440, 1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                column_widths_twips: Vec::new(),
                height_twips: None,
                left_offset_twips: 0,
                vertical_offset_twips: 0,
                wrap_margins: TableRowWrapMargins::default(),
                cell_gap_twips: 0,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                keep_with_next: false,
                no_overlap: false,
                cells: vec![
                    TableCell {
                        nested_table: None,
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: TableCellSpacing {
                            left_twips: Some(120),
                            right_twips: Some(120),
                            top_twips: Some(60),
                            bottom_twips: Some(60),
                        },
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "Left".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    },
                    TableCell {
                        nested_table: None,
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: TableCellSpacing {
                            left_twips: Some(120),
                            right_twips: Some(120),
                            top_twips: Some(60),
                            bottom_twips: Some(60),
                        },
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "Right".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    },
                ],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let text_x = |text: &str| {
            layout.pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text == text => Some(fragment.x),
                    _ => None,
                })
                .expect("cell text")
        };
        let mut vertical_borders = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Line { x1, x2, .. } if (*x1 - *x2).abs() < 0.01 => Some(*x1),
                _ => None,
            })
            .collect::<Vec<_>>();
        vertical_borders.sort_by(f32::total_cmp);

        assert!((text_x("Left") - 78.0).abs() < 0.01);
        assert!((text_x("Right") - 150.0).abs() < 0.01);
        assert!(
            vertical_borders
                .windows(2)
                .any(|pair| (pair[0] - 138.0).abs() < 0.01 && (pair[1] - 150.0).abs() < 0.01),
            "expected spacing to create a passive gap between adjacent cell borders: {vertical_borders:?}"
        );
    }

    #[test]
    fn distributes_heterogeneous_cell_spacing_like_word() {
        let parsed = parse_rtf(include_str!(
            "../../fixtures/table-spacing-autofit-passive.rtf"
        ))
        .unwrap();
        let layout = LayoutEngine::layout(&parsed.document);
        let text_fragments = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment),
                _ => None,
            })
            .collect::<Vec<_>>();
        let before = text_fragments
            .iter()
            .copied()
            .find(|fragment| fragment.text == "Before ")
            .expect("before-table fragment");
        let unit_fragments = text_fragments
            .iter()
            .copied()
            .filter(|fragment| fragment.text == "Unit ")
            .collect::<Vec<_>>();
        let [left, right] = unit_fragments.as_slice() else {
            panic!("expected two Unit fragments, got {unit_fragments:?}");
        };

        assert!((left.x - 42.0).abs() < 0.01);
        assert!((right.x - 159.0).abs() < 0.01);
        let baseline_gap = before.baseline_y - left.baseline_y;
        assert!(
            (baseline_gap - 18.0).abs() < 0.01,
            "before={}, left={}, gap={baseline_gap}",
            before.baseline_y,
            left.baseline_y
        );
    }

    #[test]
    fn lays_out_table_cell_vertical_alignment() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440, 1440, 1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                column_widths_twips: Vec::new(),
                height_twips: Some(1440),
                left_offset_twips: 0,
                vertical_offset_twips: 0,
                wrap_margins: TableRowWrapMargins::default(),
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                keep_with_next: false,
                no_overlap: false,
                cells: vec![
                    TableCell {
                        nested_table: None,
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
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
                                text: "Top".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    },
                    TableCell {
                        nested_table: None,
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                        vertical_align: TableCellVerticalAlign::Center,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "Center".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    },
                    TableCell {
                        nested_table: None,
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                        vertical_align: TableCellVerticalAlign::Bottom,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "Bottom".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    },
                ],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let baseline_for = |text: &str| {
            layout.pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text == text => {
                        Some(fragment.baseline_y)
                    }
                    _ => None,
                })
                .expect("aligned text")
        };

        assert!(baseline_for("Top") > baseline_for("Center"));
        assert!(baseline_for("Center") > baseline_for("Bottom"));
    }

    #[test]
    fn lays_out_horizontal_merged_table_cells() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440, 1440, 1440],
            borders_visible: true,
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
                cells: vec![
                    TableCell {
                        nested_table: None,
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::First,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "Merged".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    },
                    TableCell {
                        nested_table: None,
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::Continuation,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "Hidden".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    },
                    TableCell {
                        nested_table: None,
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
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
                                text: "Plain".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    },
                ],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let text_x = |text: &str| {
            page.items.iter().find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == text => Some(fragment.x),
                _ => None,
            })
        };

        assert!((text_x("Merged").expect("merged text") - 75.0).abs() < 0.01);
        assert!(text_x("Hidden").is_none());
        assert!((text_x("Plain").expect("plain text") - 219.0).abs() < 0.01);
        assert!(!has_vertical_line_at(page, 144.0));
        assert!(has_vertical_line_at(page, 216.0));
    }

    #[test]
    fn lays_out_vertical_merged_table_cells() {
        let mut document = Document::default();
        document.colors = vec![
            Color {
                red: 255,
                green: 255,
                blue: 255,
            },
            Color {
                red: 210,
                green: 230,
                blue: 255,
            },
        ];
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440, 1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![
                TableRow {
                    column_widths_twips: Vec::new(),
                    height_twips: Some(720),
                    left_offset_twips: 0,
                    vertical_offset_twips: 0,
                    wrap_margins: TableRowWrapMargins::default(),
                    cell_gap_twips: 60,
                    alignment: TableRowAlignment::Left,
                    repeat_header: false,
                    keep_together: false,
                    keep_with_next: false,
                    no_overlap: false,
                    cells: vec![
                        TableCell {
                            nested_table: None,
                            shading_color_index: Some(1),
                            shading_basis_points: 10_000,
                            shading_pattern: crate::model::ShadingPattern::None,
                            padding: TableCellPadding::default(),
                            spacing: Default::default(),
                            borders: TableCellBorders::default(),
                            fit_text: false,
                            text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                            vertical_align: TableCellVerticalAlign::Bottom,
                            horizontal_merge: TableCellHorizontalMerge::None,
                            vertical_merge: TableCellVerticalMerge::First,
                            paragraphs: vec![Paragraph {
                                style: Default::default(),
                                runs: vec![Run {
                                    text: "Merged top".to_string(),
                                    style: Default::default(),
                                }],
                            }],
                        },
                        TableCell {
                            nested_table: None,
                            shading_color_index: None,
                            shading_basis_points: 10_000,
                            shading_pattern: crate::model::ShadingPattern::None,
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
                                    text: "Right top".to_string(),
                                    style: Default::default(),
                                }],
                            }],
                        },
                    ],
                },
                TableRow {
                    column_widths_twips: Vec::new(),
                    height_twips: Some(720),
                    left_offset_twips: 0,
                    vertical_offset_twips: 0,
                    wrap_margins: TableRowWrapMargins::default(),
                    cell_gap_twips: 60,
                    alignment: TableRowAlignment::Left,
                    repeat_header: false,
                    keep_together: false,
                    keep_with_next: false,
                    no_overlap: false,
                    cells: vec![
                        TableCell {
                            nested_table: None,
                            shading_color_index: None,
                            shading_basis_points: 10_000,
                            shading_pattern: crate::model::ShadingPattern::None,
                            padding: TableCellPadding::default(),
                            spacing: Default::default(),
                            borders: TableCellBorders::default(),
                            fit_text: false,
                            text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                            vertical_align: TableCellVerticalAlign::Top,
                            horizontal_merge: TableCellHorizontalMerge::None,
                            vertical_merge: TableCellVerticalMerge::Continuation,
                            paragraphs: vec![Paragraph {
                                style: Default::default(),
                                runs: vec![Run {
                                    text: "Hidden continuation".to_string(),
                                    style: Default::default(),
                                }],
                            }],
                        },
                        TableCell {
                            nested_table: None,
                            shading_color_index: None,
                            shading_basis_points: 10_000,
                            shading_pattern: crate::model::ShadingPattern::None,
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
                                    text: "Right bottom".to_string(),
                                    style: Default::default(),
                                }],
                            }],
                        },
                    ],
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let text = layout_text(page);
        let internal_y = internal_horizontal_line_y(page, 144.0, 216.0)
            .expect("unmerged column internal border");
        let baseline_y = |needle: &str| {
            page.items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.contains(needle) => {
                        Some(fragment.baseline_y)
                    }
                    _ => None,
                })
                .expect("table cell text")
        };
        let merged_shading_height = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight { x, height, .. } if (*x - 72.0).abs() < 0.01 => {
                    Some(*height)
                }
                _ => None,
            })
            .expect("merged cell shading");

        assert!(text.contains("Merged top"));
        assert!(!text.contains("Hidden continuation"));
        assert!(text.contains("Right bottom"));
        assert!((merged_shading_height - 73.0).abs() < 0.01);
        assert!(baseline_y("Merged") < baseline_y("Right"));
        assert!(!has_horizontal_line_segment_at_y(
            page, 72.0, 144.0, internal_y
        ));
    }

    #[test]
    fn repeats_table_header_rows_after_page_breaks() {
        fn row(text: &str, repeat_header: bool) -> TableRow {
            TableRow {
                column_widths_twips: Vec::new(),
                height_twips: Some(720),
                left_offset_twips: 0,
                vertical_offset_twips: 0,
                wrap_margins: TableRowWrapMargins::default(),
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header,
                keep_together: false,
                keep_with_next: false,
                no_overlap: false,
                cells: vec![TableCell {
                    nested_table: None,
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
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
                            text: text.to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }
        }

        let mut rows = vec![row("Header", true)];
        for idx in 0..30 {
            rows.push(row(&format!("Body {idx}"), false));
        }

        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows,
        })];

        let layout = LayoutEngine::layout(&document);
        assert!(layout.pages.len() > 1);
        for page in layout.pages.iter().skip(1) {
            assert!(layout_text(page).contains("Header"));
        }

        let header_count = layout
            .pages
            .iter()
            .flat_map(|page| &page.items)
            .filter(|item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Header"))
            .count();
        assert_eq!(header_count, layout.pages.len());
    }

    #[test]
    fn table_row_keep_with_next_starts_pair_on_next_page_when_pair_would_split() {
        fn row(text: &str, keep_with_next: bool) -> TableRow {
            TableRow {
                column_widths_twips: Vec::new(),
                height_twips: Some(720),
                left_offset_twips: 0,
                vertical_offset_twips: 0,
                wrap_margins: TableRowWrapMargins::default(),
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                keep_with_next,
                no_overlap: false,
                cells: vec![TableCell {
                    nested_table: None,
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
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
                            text: text.to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }
        }

        let mut document = small_test_page_document();
        document.blocks = vec![Block::Table(Table {
            rows: vec![
                row("Filler 1", false),
                row("Filler 2", false),
                row("Filler 3", false),
                row("Keep follow", true),
                row("Follower", false),
            ],
            column_widths_twips: vec![2_880],
            borders_visible: true,
            preserve_authored_widths: false,
        })];

        let layout = LayoutEngine::layout(&document);
        let first_page_text = layout_text(&layout.pages[0]);
        let second_page_text = layout_text(&layout.pages[1]);

        assert!(layout.pages.len() >= 2);
        assert!(first_page_text.contains("Filler 3"));
        assert!(!first_page_text.contains("Keep follow"));
        assert!(second_page_text.contains("Keep follow"));
        assert!(second_page_text.contains("Follower"));
    }

    #[test]
    fn splits_tall_auto_height_table_rows_across_pages() {
        fn row(text: String, repeat_header: bool) -> TableRow {
            TableRow {
                column_widths_twips: Vec::new(),
                height_twips: None,
                left_offset_twips: 0,
                vertical_offset_twips: 0,
                wrap_margins: TableRowWrapMargins::default(),
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header,
                keep_together: false,
                keep_with_next: false,
                no_overlap: false,
                cells: vec![TableCell {
                    nested_table: None,
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
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
                            text,
                            style: Default::default(),
                        }],
                    }],
                }],
            }
        }

        let tall_text = (0..14)
            .map(|idx| format!("Line {idx:02}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut document = small_test_page_document();
        document.blocks = vec![Block::Table(Table {
            rows: vec![row(tall_text, false)],
            column_widths_twips: vec![2_880],
            borders_visible: true,
            preserve_authored_widths: false,
        })];

        let layout = LayoutEngine::layout(&document);
        let first_page_text = layout_text(&layout.pages[0]);
        let later_page_text = layout
            .pages
            .iter()
            .skip(1)
            .map(layout_text)
            .collect::<String>();

        assert!(layout.pages.len() > 1);
        assert!(first_page_text.contains("Line 00"));
        assert!(!first_page_text.contains("Line 13"));
        assert!(later_page_text.contains("Line 13"));
        assert!(layout.pages.iter().all(|page| {
            page.items.iter().all(|item| {
            !matches!(item, LayoutItem::Text(fragment) if fragment.text.contains(['\r', '\n']))
        })
        }));
    }

    #[test]
    fn repeats_table_header_on_split_tall_row_continuation_pages() {
        fn row(text: String, repeat_header: bool) -> TableRow {
            TableRow {
                column_widths_twips: Vec::new(),
                height_twips: None,
                left_offset_twips: 0,
                vertical_offset_twips: 0,
                wrap_margins: TableRowWrapMargins::default(),
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header,
                keep_together: false,
                keep_with_next: false,
                no_overlap: false,
                cells: vec![TableCell {
                    nested_table: None,
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
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
                            text,
                            style: Default::default(),
                        }],
                    }],
                }],
            }
        }

        let tall_text = (0..14)
            .map(|idx| format!("Line {idx:02}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut document = small_test_page_document();
        document.blocks = vec![Block::Table(Table {
            rows: vec![row("Header".to_string(), true), row(tall_text, false)],
            column_widths_twips: vec![2_880],
            borders_visible: true,
            preserve_authored_widths: false,
        })];

        let layout = LayoutEngine::layout(&document);
        assert!(layout.pages.len() > 1);
        for page in layout.pages.iter().skip(1) {
            assert!(
                layout_text(page).contains("Header"),
                "split-row continuation page should repeat table header"
            );
        }
    }

    #[test]
    fn splits_tall_positive_minimum_height_table_rows_across_pages() {
        fn row(text: String, height_twips: Option<i32>) -> TableRow {
            TableRow {
                column_widths_twips: Vec::new(),
                height_twips,
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
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
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
                            text,
                            style: Default::default(),
                        }],
                    }],
                }],
            }
        }

        let tall_text = (0..14)
            .map(|idx| format!("Line {idx:02}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut document = small_test_page_document();
        document.blocks = vec![Block::Table(Table {
            rows: vec![row(tall_text, Some(720))],
            column_widths_twips: vec![2_880],
            borders_visible: true,
            preserve_authored_widths: false,
        })];

        let layout = LayoutEngine::layout(&document);
        let first_page_text = layout_text(&layout.pages[0]);
        let later_page_text = layout
            .pages
            .iter()
            .skip(1)
            .map(layout_text)
            .collect::<String>();

        assert!(layout.pages.len() > 1);
        assert!(first_page_text.contains("Line 00"));
        assert!(!first_page_text.contains("Line 13"));
        assert!(later_page_text.contains("Line 13"));
    }

    #[test]
    fn terminal_no_wrap_table_row_clips_at_word_page_boundary() {
        let parsed = crate::rtf::parse_rtf(include_str!("../../fixtures/no-wrap-passive.rtf"))
            .expect("no-wrap fixture should parse");
        assert!(parsed.document.blocks.iter().any(|block| {
            matches!(block, Block::Paragraph(paragraph) if paragraph.runs.iter().any(|run| run.text.contains("After passive no wrap.")))
        }));

        let layout = LayoutEngine::layout(&parsed.document);

        assert_eq!(layout.pages.len(), 1);
        let mut rendered_lines = Vec::<(f32, String)>::new();
        for fragment in layout.pages[0].items.iter().filter_map(|item| match item {
            LayoutItem::Text(fragment) => Some(fragment),
            _ => None,
        }) {
            if let Some((_, text)) = rendered_lines
                .iter_mut()
                .find(|(baseline, _)| (*baseline - fragment.baseline_y).abs() < 0.01)
            {
                text.push_str(&fragment.text);
            } else {
                rendered_lines.push((fragment.baseline_y, fragment.text.clone()));
            }
        }
        let rendered_lines = rendered_lines
            .iter()
            .map(|(_, text)| text.trim())
            .collect::<Vec<_>>();
        for expected in [
            "Epsilon Zeta Eta Theta Iota Ka",
            "ppa",
            "Direct wrapped paragraph can b",
            "reak normally across the narrow",
        ] {
            assert!(
                rendered_lines.contains(&expected),
                "missing line {expected:?}"
            );
        }
        let continuation = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "content" => Some(fragment),
                _ => None,
            })
            .expect("late-declared second-cell continuation");
        assert!((continuation.x - 95.4).abs() < 0.01);
        let visible = layout_text(&layout.pages[0]);
        assert!(visible.contains("Cell Alpha"));
        assert!(!visible.contains("After passive no wrap."));
    }

    #[test]
    fn lays_out_table_cell_shading_intensity_as_tinted_background() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
        ];
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: false,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                column_widths_twips: Vec::new(),
                height_twips: None,
                left_offset_twips: 0,
                vertical_offset_twips: 0,
                wrap_margins: TableRowWrapMargins::default(),
                cell_gap_twips: 0,
                alignment: Default::default(),
                repeat_header: false,
                keep_together: false,
                keep_with_next: false,
                no_overlap: false,
                cells: vec![TableCell {
                    nested_table: None,
                    shading_color_index: Some(1),
                    shading_basis_points: 5_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                    vertical_align: Default::default(),
                    horizontal_merge: Default::default(),
                    vertical_merge: Default::default(),
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: "Tinted cell".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
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
            .expect("table cell shading");

        assert_eq!(
            color,
            PdfColor {
                red: 1.0,
                green: 0.5,
                blue: 0.5
            }
        );
    }

    #[test]
    fn lays_out_table_cell_paragraph_shading_background() {
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
        document.blocks = vec![Block::Table(Table {
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
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: paragraph_style,
                        runs: vec![Run {
                            text: "Cell paragraph".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(|item| {
            matches!(
                item,
                LayoutItem::Highlight { x, width, color, .. }
                    if (*x - 75.0).abs() < 0.01
                        && (*width - 66.0).abs() < 0.01
                        && *color == PdfColor {
                            red: 240.0 / 255.0,
                            green: 240.0 / 255.0,
                            blue: 0.0
                        }
            )
        }));
        assert!(layout_text(page).contains("Cell paragraph"));
    }

    #[test]
    fn lays_out_table_cell_bar_tab_stops_as_passive_vertical_lines() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.tab_stops_twips = vec![360, 720];
        paragraph_style.tab_stop_alignments = vec![TabAlignment::Bar, TabAlignment::Left];
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
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
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    text_direction: TableCellTextDirection::LeftToRightTopToBottom,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: paragraph_style,
                        runs: vec![Run {
                            text: "A\tB".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, x2, .. } if (*x1 - 93.0).abs() < 0.01 && (*x2 - 93.0).abs() < 0.01
        )));
        assert!(
            (text_x(page, "B").expect("table tabbed text") - 111.0).abs() < 0.01,
            "table bar tab should not consume the normal left tab stop"
        );
    }

    #[test]
    fn resolves_safe_pdf_font_family_from_font_table() {
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
                name: "Courier New".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Modern,
                pitch: FontPitch::Default,
            },
        ];
        let mut style = CharacterStyle::default();
        style.font_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Mono".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.font_family == PdfFontFamily::Courier)
        ));
    }

    #[test]
    fn uses_word_legacy_serif_default_without_a_font_table() {
        let mut document = Document::default();
        document.fonts.clear();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Default text".to_string(),
                style: CharacterStyle::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let family = layout.pages[0].items.iter().find_map(|item| match item {
            LayoutItem::Text(fragment) if !fragment.text.trim().is_empty() => {
                Some(fragment.font_family)
            }
            _ => None,
        });

        assert_eq!(family, Some(PdfFontFamily::Times));
    }

    #[test]
    fn renders_narrow_font_aliases_with_passive_horizontal_scaling() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Arial".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Arial Narrow".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
        ];
        let normal_style = CharacterStyle {
            font_index: 0,
            ..Default::default()
        };
        let narrow_style = CharacterStyle {
            font_index: 1,
            ..Default::default()
        };
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    text: "Normal".to_string(),
                    style: normal_style,
                },
                Run {
                    text: "\n".to_string(),
                    style: CharacterStyle::default(),
                },
                Run {
                    text: "Normal".to_string(),
                    style: narrow_style,
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let fragments = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "Normal" => Some(fragment),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(fragments.len(), 2);
        assert_eq!(fragments[0].font_family, PdfFontFamily::Helvetica);
        assert_eq!(fragments[0].style.character_scaling_percent, 100);
        assert_eq!(fragments[1].font_family, PdfFontFamily::Helvetica);
        assert_eq!(
            fragments[1].style.character_scaling_percent,
            PASSIVE_NARROW_FONT_SCALE_PERCENT
        );
        assert!(
            fragments[1].x < fragments[0].x + 0.01,
            "narrow font scaling should not shift the left edge"
        );
    }
