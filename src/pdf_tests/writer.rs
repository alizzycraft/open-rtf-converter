    #[test]
    fn writes_valid_pdf_bytes() {
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                dynamic_field: None,
                text: "Hello PDF".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
    }

    #[test]
    fn writes_landscape_media_box() {
        let mut document = Document::default();
        document.page.width_twips = 15_840;
        document.page.height_twips = 12_240;
        document.page.landscape = true;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                dynamic_field: None,
                text: "Landscape PDF".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let page = parsed.get_object(page_id).unwrap().as_dict().unwrap();
        let media_box = page.get(b"MediaBox").unwrap().as_array().unwrap();

        assert_eq!(pdf_number(media_box[2].clone()), Some(792.0));
        assert_eq!(pdf_number(media_box[3].clone()), Some(612.0));
    }

    #[test]
    fn writes_section_specific_media_boxes() {
        let mut document = Document::default();
        let mut second_page = PageSettings::default();
        second_page.width_twips = 10_080;
        second_page.height_twips = 7_200;
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    dynamic_field: None,
                    text: "First page".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::SectionBreak,
            Block::SectionSettings(second_page),
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    dynamic_field: None,
                    text: "Second page".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_ids = parsed.get_pages().values().copied().collect::<Vec<_>>();
        let first = parsed.get_object(page_ids[0]).unwrap().as_dict().unwrap();
        let second = parsed.get_object(page_ids[1]).unwrap().as_dict().unwrap();
        let first_media_box = first.get(b"MediaBox").unwrap().as_array().unwrap();
        let second_media_box = second.get(b"MediaBox").unwrap().as_array().unwrap();

        assert_eq!(pdf_number(first_media_box[2].clone()), Some(612.0));
        assert_eq!(pdf_number(first_media_box[3].clone()), Some(792.0));
        assert_eq!(pdf_number(second_media_box[2].clone()), Some(504.0));
        assert_eq!(pdf_number(second_media_box[3].clone()), Some(360.0));
    }

    #[test]
    fn writes_border_styles_as_passive_pdf_strokes() {
        let mut document = Document::default();
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 60,
            color_index: None,
            style: BorderStyle::Triple,
            ..TableCellBorder::default()
        };
        let mut character_style = CharacterStyle::default();
        character_style.border = TableCellBorder {
            visible: true,
            width_twips: 40,
            color_index: None,
            style: BorderStyle::Dashed,
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
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Styled");
        assert!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "S")
                .count()
                >= 7
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "d")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_wavy_borders_as_passive_path_strokes() {
        let mut document = Document::default();
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 80,
            color_index: None,
            style: BorderStyle::Wavy,
            ..TableCellBorder::default()
        };
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                dynamic_field: None,
                text: "Wavy".to_string(),
                style: CharacterStyle::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Wavy");
        assert!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "l")
                .count()
                > 2
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
        assert!(!pdf.windows(b"/URI".len()).any(|window| window == b"/URI"));
    }

    #[test]
    fn bundled_family_hint_substitutes_outrank_the_sans_wildcard() {
        let provider = FontProvider::browser_safe_defaults();
        let best_asset = |font: &FontDef| {
            provider
                .assets
                .iter()
                .enumerate()
                .filter_map(|(index, asset)| {
                    supplied_font_asset_match_priority(asset, font)
                        .map(|priority| (priority, index))
                })
                .min()
                .map(|(_, index)| &provider.assets[index])
                .expect("family fallback")
        };
        let roman = FontDef {
            index: 1,
            name: "Mystery Serif".to_string(),
            alternate_name: None,
            charset: None,
            code_page: None,
            family: FontFamilyHint::Roman,
            pitch: FontPitch::Default,
        };
        let modern = FontDef {
            index: 2,
            name: "Mystery Mono".to_string(),
            alternate_name: None,
            charset: None,
            code_page: None,
            family: FontFamilyHint::Modern,
            pitch: FontPitch::Default,
        };

        assert!(
            best_asset(&roman)
                .family_names
                .iter()
                .any(|name| name == "Times New Roman")
        );
        assert!(
            best_asset(&modern)
                .family_names
                .iter()
                .any(|name| name == "Calibri")
        );
    }

    #[test]
    fn writes_table_cell_shading_as_passive_fill() {
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
                alignment: crate::model::TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                keep_with_next: false,
                no_overlap: false,
                cells: vec![TableCell {
                    nested_table: None,
                    shading_color_index: Some(1),
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: crate::model::TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: crate::model::TableCellBorders::default(),
                    fit_text: false,
                    text_direction: crate::model::TableCellTextDirection::LeftToRightTopToBottom,
                    vertical_align: crate::model::TableCellVerticalAlign::Top,
                    horizontal_merge: crate::model::TableCellHorizontalMerge::None,
                    vertical_merge: crate::model::TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            dynamic_field: None,
                            text: "Shaded".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
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
                && format!("{:?}", operation.operands).contains("0.8627451")
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
        assert!(content_text(&content).contains("Shaded"));
    }

    #[test]
    fn writes_optional_hyphen_only_when_wrapping_uses_it() {
        let mut wide_document = Document::default();
        wide_document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                dynamic_field: None,
                text: "Alpha\u{00ad}Beta".to_string(),
                style: Default::default(),
            }],
        })];
        let wide_layout = LayoutEngine::layout(&wide_document);
        let wide_pdf = render_pdf(&wide_layout);
        let wide_parsed = lopdf::Document::load_mem(&wide_pdf).unwrap();
        let wide_page_id = *wide_parsed.get_pages().values().next().expect("page");
        let wide_content = wide_parsed
            .get_and_decode_page_content(wide_page_id)
            .unwrap();
        assert_eq!(content_text(&wide_content), "AlphaBeta");

        let mut narrow_style = ParagraphStyle::default();
        narrow_style.right_indent_twips = 8_400;
        let mut narrow_document = Document::default();
        narrow_document.blocks = vec![Block::Paragraph(Paragraph {
            style: narrow_style,
            runs: vec![Run {
                dynamic_field: None,
                text: "Alpha\u{00ad}Beta".to_string(),
                style: Default::default(),
            }],
        })];
        let narrow_layout = LayoutEngine::layout(&narrow_document);
        let narrow_pdf = render_pdf(&narrow_layout);
        let narrow_parsed = lopdf::Document::load_mem(&narrow_pdf).unwrap();
        let narrow_page_id = *narrow_parsed.get_pages().values().next().expect("page");
        let narrow_content = narrow_parsed
            .get_and_decode_page_content(narrow_page_id)
            .unwrap();
        assert_eq!(content_text(&narrow_content), "Alpha-Beta");
    }

    #[test]
    fn writes_inverse_scaled_pdf_tracking_for_constant_page_space() {
        let mut style = CharacterStyle::default();
        style.character_spacing_twips = 20;
        style.character_scaling_percent = 50;
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                dynamic_field: None,
                text: "Tracked".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Tracked");
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tc"
                && operation.operands.len() == 1
                && format!("{:?}", operation.operands[0]).starts_with('2')
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tz"
                && operation.operands.len() == 1
                && format!("{:?}", operation.operands[0]).contains("50")
        }));
    }

    #[test]
    fn rtl_script_fragments_use_passive_visual_order_without_reversing_latin_runs() {
        let mut rtl_style = CharacterStyle::default();
        rtl_style.right_to_left = true;
        let rtl_fragment = TextFragment {
            dynamic_field: None,
            text: "שלום".to_string(),
            x: 0.0,
            baseline_y: 0.0,
            rotation: TextRotation::None,
            color: Default::default(),
            font_family: PdfFontFamily::Helvetica,
            word_spacing: 0.0,
            style: rtl_style.clone(),
        };
        let latin_fragment = TextFragment {
            dynamic_field: None,
            text: "LTR".to_string(),
            style: rtl_style,
            ..rtl_fragment.clone()
        };

        assert_eq!(
            passive_visual_text_for_fragment(&rtl_fragment).as_deref(),
            Some("םולש")
        );
        assert_eq!(passive_visual_text_for_fragment(&latin_fragment), None);
    }

    #[test]
    fn writes_resolved_page_numbers_without_internal_marker() {
        let mut document = Document::default();
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![
                typed_field_run(DynamicField::PageNumber),
                Run {
                    dynamic_field: None,
                    text: " of ".to_string(),
                    style: Default::default(),
                },
                typed_field_run(DynamicField::TotalPages),
            ],
        }];
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    dynamic_field: None,
                    text: "First".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::PageBreak,
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    dynamic_field: None,
                    text: "Second".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_ids = parsed.get_pages().values().copied().collect::<Vec<_>>();
        let first_text = content_text(&parsed.get_and_decode_page_content(page_ids[0]).unwrap());
        let second_text = content_text(&parsed.get_and_decode_page_content(page_ids[1]).unwrap());

        assert!(first_text.contains("1 of 2"));
        assert!(second_text.contains("2 of 2"));
    }

    #[test]
    fn writes_resolved_section_numbers_without_internal_marker() {
        let mut document = Document::default();
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![
                    Run {
                        dynamic_field: None,
                        text: "Section ".to_string(),
                        style: Default::default(),
                    },
                    typed_field_run(DynamicField::SectionNumber),
                ],
            }),
            Block::ContinuousSectionBreak,
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![
                    Run {
                        dynamic_field: None,
                        text: "Section ".to_string(),
                        style: Default::default(),
                    },
                    typed_field_run(DynamicField::SectionNumber),
                ],
            }),
            Block::SectionBreak,
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![
                    Run {
                        dynamic_field: None,
                        text: "Section ".to_string(),
                        style: Default::default(),
                    },
                    typed_field_run(DynamicField::SectionNumber),
                ],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_ids = parsed.get_pages().values().copied().collect::<Vec<_>>();
        let first_text = content_text(&parsed.get_and_decode_page_content(page_ids[0]).unwrap());
        let second_text = content_text(&parsed.get_and_decode_page_content(page_ids[1]).unwrap());

        assert!(first_text.contains("Section 1"));
        assert!(first_text.contains("Section 2"));
        assert!(second_text.contains("Section 3"));
    }
    fn typed_field_run(field: DynamicField) -> Run {
        Run {
            dynamic_field: Some(field),
            text: String::new(),
            style: Default::default(),
        }
    }
