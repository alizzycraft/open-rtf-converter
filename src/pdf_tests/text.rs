    #[test]
    fn writes_column_layout_as_passive_text_and_rules() {
        let mut document = Document::default();
        document.page.column_count = 2;
        document.page.line_between_columns = true;
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Left".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::ColumnBreak,
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Right".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(parsed.get_pages().len(), 1);
        assert_eq!(content_text(&content), "LeftRight");
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "m")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
        assert!(!pdf.windows(b"/URI".len()).any(|window| window == b"/URI"));
    }

    #[test]
    fn writes_solid_character_border_as_passive_filled_rectangles() {
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
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Boxed");
        assert!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "re")
                .count()
                >= 4
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 1.0)
                && pdf_number(operation.operands[1].clone()).is_some_and(|value| value == 0.0)
                && pdf_number(operation.operands[2].clone()).is_some_and(|value| value == 0.0)
        }));
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
        assert!(!pdf.windows(b"/URI".len()).any(|window| window == b"/URI"));
    }

    #[test]
    fn writes_character_shading_intensity_as_passive_fill() {
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
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Tinted");
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 0.5)
                && pdf_number(operation.operands[1].clone()).is_some_and(|value| value == 0.0)
                && pdf_number(operation.operands[2].clone()).is_some_and(|value| value == 0.0)
        }));
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "re")
        );
    }

    #[test]
    fn writes_caps_as_uppercase_text() {
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
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "MIXED CASE");
    }

    #[test]
    fn writes_small_caps_as_passive_scaled_uppercase_text() {
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
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "MIX");
        assert!(
            content.operations.iter().any(|operation| {
                operation.operator == "Tf" && format!("{:?}", operation.operands).contains("8.5")
            }),
            "lowercase small-caps glyphs should be emitted with a reduced passive font size"
        );
    }

    #[test]
    fn does_not_write_hidden_runs_to_pdf_text() {
        let mut hidden_style = CharacterStyle::default();
        hidden_style.hidden = true;
        let mut document = Document::default();
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
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "VisibleShown");
        assert!(
            !pdf.windows(b"Hidden".len())
                .any(|window| window == b"Hidden")
        );
    }

    #[test]
    fn writes_character_spacing_as_passive_pdf_text_state() {
        let mut style = CharacterStyle::default();
        style.character_spacing_twips = 200;
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Wide".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Wide");
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tc"
                && operation.operands.len() == 1
                && format!("{:?}", operation.operands[0]).contains("10")
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tc"
                && operation.operands.len() == 1
                && format!("{:?}", operation.operands[0]).contains('0')
        }));
    }

    #[test]
    fn writes_character_scaling_as_passive_pdf_text_state() {
        let mut style = CharacterStyle::default();
        style.character_scaling_percent = 150;
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Scaled".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Scaled");
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tz"
                && operation.operands.len() == 1
                && format!("{:?}", operation.operands[0]).contains("150")
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tz"
                && operation.operands.len() == 1
                && format!("{:?}", operation.operands[0]).contains("100")
        }));
    }

    #[test]
    fn writes_justified_word_spacing_as_passive_pdf_text_state() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.alignment = Alignment::Justified;
        let mut document = Document::default();
        document.page.width_twips = 5_000;
        document.page.margin_left_twips = 720;
        document.page.margin_right_twips = 720;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "one two three four five six seven eight nine ten".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(content_text(&content).contains("one"));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tw"
                && operation.operands.len() == 1
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value > 0.0)
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tw"
                && operation.operands.len() == 1
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 0.0)
        }));
    }

    #[test]
    fn writes_shadow_text_as_passive_offset_text() {
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
        style.shadow = true;
        style.color_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Shadow".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "ShadowShadow");
        assert_eq!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "Tj")
                .count(),
            2
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone())
                    .is_some_and(|value| (value - 0.35).abs() < 0.001)
                && pdf_number(operation.operands[1].clone()).is_some_and(|value| value == 0.0)
                && pdf_number(operation.operands[2].clone()).is_some_and(|value| value == 0.0)
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 1.0)
                && pdf_number(operation.operands[1].clone()).is_some_and(|value| value == 0.0)
                && pdf_number(operation.operands[2].clone()).is_some_and(|value| value == 0.0)
        }));
    }

    #[test]
    fn writes_relief_text_as_passive_offset_text_layers() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 0,
                green: 0,
                blue: 255,
            },
        ];
        let mut style = CharacterStyle::default();
        style.relief = TextRelief::Emboss;
        style.color_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Relief".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "ReliefReliefRelief");
        assert_eq!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "Tj")
                .count(),
            3
        );
        let origins = content
            .operations
            .iter()
            .filter(|operation| operation.operator == "Td")
            .filter_map(|operation| {
                Some((
                    pdf_number(operation.operands.first()?.clone())?,
                    pdf_number(operation.operands.get(1)?.clone())?,
                ))
            })
            .collect::<Vec<_>>();
        assert_eq!(origins.len(), 3);
        assert!((origins[1].0 - origins[0].0 + 1.92).abs() < 0.001);
        assert!((origins[1].1 - origins[0].1 - 1.92).abs() < 0.001);
        assert!((origins[2].0 - origins[0].0 + 0.96).abs() < 0.001);
        assert!((origins[2].1 - origins[0].1 - 0.96).abs() < 0.001);
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 0.6)
                && pdf_number(operation.operands[1].clone()).is_some_and(|value| value == 0.6)
                && pdf_number(operation.operands[2].clone()).is_some_and(|value| value == 1.0)
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 0.0)
                && pdf_number(operation.operands[1].clone()).is_some_and(|value| value == 0.0)
                && pdf_number(operation.operands[2].clone())
                    .is_some_and(|value| (value - 0.35).abs() < 0.001)
        }));
    }

    #[test]
    fn encodes_word_symbol_text_as_safe_pdf_fallbacks() {
        let encoded = encode_pdf_text(
            "\u{2018}q\u{2019} \u{201c}qq\u{201d} \u{2014}\u{2013}\u{2011}\u{00ad} \u{2022} \u{00a0}\u{2002}\u{2003}\u{2005}",
        );

        assert_eq!(
            encoded,
            vec![
                0x91, b'q', 0x92, b' ', 0x93, b'q', b'q', 0x94, b' ', 0x97, 0x96, b'-', 0xad, b' ',
                0x95, b' ', 0xa0, b' ', b' ', b' '
            ]
        );
    }

    #[test]
    fn writes_passive_kerning_as_positioned_text_without_control_payload() {
        let mut document = Document::default();
        let mut style = CharacterStyle {
            character_kerning_half_points: 2,
            ..Default::default()
        };
        style.font_size_half_points = 24;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "AV".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();
        let positioned = content
            .operations
            .iter()
            .find(|operation| operation.operator == "TJ")
            .expect("kerning should use positioned text");

        assert_eq!(content_bytes(&content), b"AV");
        assert!(
            positioned.operands.iter().any(|operand| {
                operand.as_array().is_ok_and(|items| {
                    items.iter().any(|item| {
                        matches!(item, lopdf::Object::Integer(value) if *value > 0)
                            || matches!(item, lopdf::Object::Real(value) if *value > 0.0)
                    })
                })
            }),
            "positioned text should include a positive kerning adjustment"
        );
        for forbidden in [b"kerning".as_slice(), b"/JavaScript", b"/EmbeddedFile"] {
            assert!(
                !pdf.windows(forbidden.len())
                    .any(|window| window == forbidden)
            );
        }
    }
