    #[test]
    fn writes_paragraph_borders_as_passive_lines() {
        let mut document = Document::default();
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 80,
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
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Bordered");
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "S")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
        assert!(!pdf.windows(b"/URI".len()).any(|window| window == b"/URI"));
    }

    #[test]
    fn writes_ellipse_shape_as_passive_bezier_path() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Ellipse,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            z_order: 0,
            below_text: false,
            text_wrap: false,
            wrap_side: crate::model::StaticImageWrapSide::Both,
            wrap_margin_left_twips: 0,
            wrap_margin_right_twips: 0,
            wrap_margin_top_twips: 0,
            wrap_margin_bottom_twips: 0,
            horizontal_anchor: crate::model::StaticShapeHorizontalAnchor::Column,
            vertical_anchor: crate::model::StaticShapeVerticalAnchor::Paragraph,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            start_arrowhead_width_percent: 100,
            start_arrowhead_length_percent: 100,
            end_arrowhead_width_percent: 100,
            end_arrowhead_length_percent: 100,
            stroke_width_twips: 20,
            stroke_color: Color {
                red: 255,
                green: 128,
                blue: 0,
            },
            stroke_style: BorderStyle::Single,
            stroke_cap: StaticShapeLineCap::Flat,
            stroke_join: StaticShapeLineJoin::Miter,
            fill_opacity_percent: 100,
            stroke_opacity_percent: 100,
            fill_color: Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            }),
            fill_gradient_color: None,
            fill_pattern: crate::model::ShadingPattern::None,
            shadow_enabled: false,
            shadow_color: Color {
                red: 128,
                green: 128,
                blue: 128,
            },
            shadow_opacity_percent: 100,
            shadow_offset_x_twips: 60,
            shadow_offset_y_twips: 60,
            text_margin_left_twips: 80,
            text_margin_right_twips: 80,
            text_margin_top_twips: 80,
            text_margin_bottom_twips: 80,
            text_vertical_anchor: StaticShapeTextVerticalAnchor::Top,
            text_horizontal_anchor_centered: false,
            text: Vec::new(),
            points: Vec::new(),
            point_paths: Vec::new(),
            overlay_paths: Vec::new(),
            fill_rule: StaticImageVectorFillRule::Winding,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "c")
                .count(),
            4
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "B")
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "RG"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 1.0)
                && pdf_number(operation.operands[1].clone())
                    .is_some_and(|value| (value - 0.5019608).abs() < 0.001)
                && pdf_number(operation.operands[2].clone()).is_some_and(|value| value == 0.0)
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone())
                    .is_some_and(|value| (value - (10.0 / 255.0)).abs() < 0.001)
                && pdf_number(operation.operands[1].clone())
                    .is_some_and(|value| (value - (20.0 / 255.0)).abs() < 0.001)
                && pdf_number(operation.operands[2].clone())
                    .is_some_and(|value| (value - (30.0 / 255.0)).abs() < 0.001)
        }));
        for forbidden in [
            b"/JavaScript".as_slice(),
            b"/EmbeddedFile",
            b"/Launch",
            b"/OpenAction",
            b"/RichMedia",
        ] {
            assert!(
                !pdf.windows(forbidden.len())
                    .any(|window| window == forbidden)
            );
        }
    }

    #[test]
    fn writes_rounded_rectangle_shape_as_passive_bezier_path() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::RoundedRectangle,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            z_order: 0,
            below_text: false,
            text_wrap: false,
            wrap_side: crate::model::StaticImageWrapSide::Both,
            wrap_margin_left_twips: 0,
            wrap_margin_right_twips: 0,
            wrap_margin_top_twips: 0,
            wrap_margin_bottom_twips: 0,
            horizontal_anchor: crate::model::StaticShapeHorizontalAnchor::Column,
            vertical_anchor: crate::model::StaticShapeVerticalAnchor::Paragraph,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            start_arrowhead_width_percent: 100,
            start_arrowhead_length_percent: 100,
            end_arrowhead_width_percent: 100,
            end_arrowhead_length_percent: 100,
            stroke_width_twips: 20,
            stroke_color: Color {
                red: 255,
                green: 128,
                blue: 0,
            },
            stroke_style: BorderStyle::Single,
            stroke_cap: StaticShapeLineCap::Flat,
            stroke_join: StaticShapeLineJoin::Miter,
            fill_opacity_percent: 100,
            stroke_opacity_percent: 100,
            fill_color: Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            }),
            fill_gradient_color: None,
            fill_pattern: crate::model::ShadingPattern::None,
            shadow_enabled: false,
            shadow_color: Color {
                red: 128,
                green: 128,
                blue: 128,
            },
            shadow_opacity_percent: 100,
            shadow_offset_x_twips: 60,
            shadow_offset_y_twips: 60,
            text_margin_left_twips: 80,
            text_margin_right_twips: 80,
            text_margin_top_twips: 80,
            text_margin_bottom_twips: 80,
            text_vertical_anchor: StaticShapeTextVerticalAnchor::Top,
            text_horizontal_anchor_centered: false,
            text: Vec::new(),
            points: Vec::new(),
            point_paths: Vec::new(),
            overlay_paths: Vec::new(),
            fill_rule: StaticImageVectorFillRule::Winding,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "c")
                .count(),
            4
        );
        assert!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "l")
                .count()
                >= 4
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "B")
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "RG"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 1.0)
                && pdf_number(operation.operands[1].clone())
                    .is_some_and(|value| (value - 0.5019608).abs() < 0.001)
                && pdf_number(operation.operands[2].clone()).is_some_and(|value| value == 0.0)
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone())
                    .is_some_and(|value| (value - (10.0 / 255.0)).abs() < 0.001)
                && pdf_number(operation.operands[1].clone())
                    .is_some_and(|value| (value - (20.0 / 255.0)).abs() < 0.001)
                && pdf_number(operation.operands[2].clone())
                    .is_some_and(|value| (value - (30.0 / 255.0)).abs() < 0.001)
        }));
        for forbidden in [
            b"/JavaScript".as_slice(),
            b"/EmbeddedFile",
            b"/Launch",
            b"/OpenAction",
            b"/RichMedia",
        ] {
            assert!(
                !pdf.windows(forbidden.len())
                    .any(|window| window == forbidden)
            );
        }
    }

    #[test]
    fn writes_text_color_and_highlight_as_passive_drawing() {
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
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && format!("{:?}", operation.operands) == "[0, 1, 0]"
        }));
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "re")
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "f")
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && format!("{:?}", operation.operands) == "[1, 0, 0]"
        }));
    }

    #[test]
    fn writes_word_underline_variants_as_passive_strokes() {
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
        let mut dotted_style = CharacterStyle::default();
        dotted_style.underline = UnderlineStyle::Dotted;
        let mut dashed_style = CharacterStyle::default();
        dashed_style.underline = UnderlineStyle::Dashed;
        let mut wave_style = CharacterStyle::default();
        wave_style.underline = UnderlineStyle::Wave;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    text: "double ".to_string(),
                    style: double_style,
                },
                Run {
                    text: "dotted ".to_string(),
                    style: dotted_style,
                },
                Run {
                    text: "dashed ".to_string(),
                    style: dashed_style,
                },
                Run {
                    text: "wave".to_string(),
                    style: wave_style,
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "double dotted dashed wave");
        assert!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "S")
                .count()
                >= 5
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "d")
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "RG"
                && operation.operands.len() == 3
                && format!("{:?}", operation.operands).contains('1')
        }));
    }

    #[test]
    fn writes_outline_text_as_passive_pdf_text_state() {
        let mut style = CharacterStyle::default();
        style.outline = true;
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Outline".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Outline");
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tr"
                && operation.operands.len() == 1
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 1.0)
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tr"
                && operation.operands.len() == 1
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 0.0)
        }));
    }

    #[test]
    fn writes_word_legacy_checkbox_outlines_as_filled_passive_vectors() {
        for (input, minimum_filled_paths) in [
            (
                include_str!("../../fixtures/wingdings-checkbox-passive.rtf"),
                10,
            ),
            (
                include_str!("../../fixtures/wingdings2-checkbox-passive.rtf"),
                10,
            ),
            (include_str!("../../fixtures/webdings-checkbox-passive.rtf"), 4),
            (
                include_str!("../../fixtures/segoe-ui-symbol-checkbox-passive.rtf"),
                7,
            ),
        ] {
            let parsed = parse_rtf(input).unwrap();
            let layout = LayoutEngine::layout(&parsed.document);
            let pdf = render_pdf(&layout);
            let parsed_pdf = lopdf::Document::load_mem(&pdf).unwrap();
            let page_id = *parsed_pdf.get_pages().values().next().expect("page");
            let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
            let filled_paths = content
                .operations
                .iter()
                .filter(|operation| matches!(operation.operator.as_str(), "f" | "f*"))
                .count();

            assert!(
                filled_paths >= minimum_filled_paths,
                "expected filled legacy glyph contours"
            );
            assert!(
                content
                    .operations
                    .iter()
                    .any(|operation| operation.operator == "c"),
                "legacy check and X contours should retain bounded curves"
            );
        }
    }
