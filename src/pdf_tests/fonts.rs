    #[test]
    fn styled_supplied_font_assets_are_selected_by_run_style() {
        let mut document = Document {
            fonts: vec![FontDef {
                index: 0,
                name: "Tuffy".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Variable,
            }],
            ..Document::default()
        };
        let mut regular_style = CharacterStyle::default();
        regular_style.font_index = 0;
        let mut bold_style = regular_style.clone();
        bold_style.bold = true;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: ParagraphStyle::default(),
            runs: vec![
                Run {
                    text: "Regular".to_string(),
                    style: regular_style,
                },
                Run {
                    text: " Bold".to_string(),
                    style: bold_style,
                },
            ],
        })];
        let provider = FontProvider {
            assets: vec![
                FontAsset {
                    family_names: vec!["Tuffy".to_string()],
                    style: FontAssetStyle::default(),
                    bytes: include_bytes!("../../fixtures/fonts/Tuffy.ttf").to_vec(),
                },
                FontAsset {
                    family_names: vec!["Tuffy".to_string()],
                    style: FontAssetStyle {
                        bold: true,
                        italic: false,
                    },
                    bytes: include_bytes!("../../fixtures/fonts/Tuffy.ttf").to_vec(),
                },
            ],
            limits: Default::default(),
        };

        let layout = LayoutEngine::layout_with_font_provider(&document, Some(&provider));
        let pdf = render_pdf_with_font_provider(&layout, Some(&provider));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();
        let page_fonts = page_font_resource_names(&pdf, 0);

        assert!(page_fonts.iter().any(|name| name == b"TF1"));
        assert!(page_fonts.iter().any(|name| name == b"TF2"));
        assert!(!content_bytes_for_font(&content, b"TF1").is_empty());
        assert!(!content_bytes_for_font(&content, b"TF2").is_empty());
        assert!(
            pdf.windows(b"ORTF01+Tuffy".len())
                .any(|window| window == b"ORTF01+Tuffy")
        );
        assert!(
            pdf.windows(b"ORTF02+Tuffy".len())
                .any(|window| window == b"ORTF02+Tuffy")
        );
        assert!(
            !pdf.windows(b"ORTFSuppliedFont".len())
                .any(|window| window == b"ORTFSuppliedFont")
        );
        assert!(
            pdf.windows(b"/FontFile2".len())
                .any(|window| window == b"/FontFile2")
        );
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn supplied_extended_latin_text_does_not_emit_unused_base_font_cmap() {
        let mut document = Document {
            fonts: vec![FontDef {
                index: 0,
                name: "Tuffy".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Variable,
            }],
            ..Document::default()
        };
        let mut style = CharacterStyle::default();
        style.font_index = 0;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: ParagraphStyle::default(),
            runs: vec![Run {
                text: "Covered ő «Client»".to_string(),
                style,
            }],
        })];
        let provider = FontProvider {
            assets: vec![FontAsset {
                family_names: vec!["Tuffy".to_string()],
                style: FontAssetStyle::default(),
                bytes: include_bytes!("../../fixtures/fonts/Tuffy.ttf").to_vec(),
            }],
            limits: Default::default(),
        };

        let layout = LayoutEngine::layout_with_font_provider(&document, Some(&provider));
        let estimated = estimate_passive_pdf_object_count(&layout, Some(&provider));
        let pdf = render_pdf_with_font_provider(&layout, Some(&provider));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_fonts = page_font_resource_names(&pdf, 0);

        assert_eq!(estimated, parsed.objects.len());
        assert_eq!(estimated, 10);
        assert!(page_fonts.iter().any(|name| name == b"TF1"));
        assert!(!page_fonts.iter().any(|name| name == b"F1"));
        assert!(
            pdf.windows(b"/Subtype /Type0".len())
                .any(|window| window == b"/Subtype /Type0")
        );
        assert!(
            pdf.windows(b"/CMapType 2 def".len())
                .any(|window| window == b"/CMapType 2 def")
        );
        assert!(
            !pdf.windows(b"/CMapType 0 def".len())
                .any(|window| window == b"/CMapType 0 def")
        );
        assert!(
            pdf.windows(b"<00AB> <00AB>".len())
                .any(|window| window == b"<00AB> <00AB>")
        );
        assert!(
            pdf.windows(b"<00BB> <00BB>".len())
                .any(|window| window == b"<00BB> <00BB>")
        );
        assert!(
            !pdf.windows(b"OpenRtfConverter-ExtendedLatin".len())
                .any(|window| window == b"OpenRtfConverter-ExtendedLatin"),
            "supplied Type0 font text should not allocate an unused base-font Extended Latin CMap"
        );
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn supplied_pdf_font_names_are_metadata_derived_and_sanitized() {
        let asset = FontAsset {
            family_names: vec!["Tuffy".to_string()],
            style: FontAssetStyle::default(),
            bytes: include_bytes!("../../fixtures/fonts/Tuffy.ttf").to_vec(),
        };

        assert_eq!(
            supplied_pdf_font_base_name(&asset, 2),
            b"ORTF03+Tuffy".to_vec()
        );
        assert_eq!(
            sanitize_pdf_font_name("Bad Font/Name#1", 64),
            "Bad-Font-Name-1"
        );
        assert_eq!(sanitize_pdf_font_name(" --- ", 64), "");
    }

    #[test]
    fn unknown_builtin_font_resource_lookup_is_non_panicking() {
        assert_eq!(font_index_for_resource(b"UnknownFontResource"), None);
        assert!(font_index_for_resource(HELVETICA_REGULAR).is_some());
    }

    #[test]
    fn omits_unused_pdf_base_font_resources() {
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Plain Helvetica text".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);

        assert!(
            pdf.windows(b"/BaseFont /Helvetica".len())
                .any(|window| window == b"/BaseFont /Helvetica")
        );
        for forbidden in [
            b"/BaseFont /Helvetica-Bold".as_slice(),
            b"/BaseFont /Courier",
            b"/BaseFont /Times-Roman",
            b"/BaseFont /Symbol",
            b"/BaseFont /ZapfDingbats",
            b"OpenRtfConverter-Symbol",
            b"OpenRtfConverter-ZapfDingbats",
        ] {
            assert!(
                !pdf.windows(forbidden.len())
                    .any(|window| window == forbidden),
                "unused font resource leaked into PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn page_resources_omit_fonts_used_only_on_other_pages() {
        let mut document = Document::default();
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Plain page".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::PageBreak,
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "\u{03b1}".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let first_page_fonts = page_font_resource_names(&pdf, 0);
        let second_page_fonts = page_font_resource_names(&pdf, 1);

        assert!(first_page_fonts.iter().any(|name| name == b"F1"));
        assert!(!first_page_fonts.iter().any(|name| name == b"F13"));
        assert!(second_page_fonts.iter().any(|name| name == b"F13"));
        assert!(!second_page_fonts.iter().any(|name| name == b"F1"));
        assert!(
            pdf.windows(b"/BaseFont /Symbol".len())
                .any(|window| window == b"/BaseFont /Symbol"),
            "shared font object should still be emitted when used by any page"
        );
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn symbol_font_includes_passive_to_unicode_map_while_checkboxes_render_as_vectors() {
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "alpha \u{03b1} check \u{2611}".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(
            pdf.windows(b"/BaseFont /Symbol".len())
                .any(|window| window == b"/BaseFont /Symbol")
        );
        assert!(
            !pdf.windows(b"/BaseFont /ZapfDingbats".len())
                .any(|window| window == b"/BaseFont /ZapfDingbats"),
            "passive checkbox vector path should not require a ZapfDingbats font resource"
        );
        assert!(
            pdf.windows(b"/ToUnicode".len())
                .filter(|window| *window == b"/ToUnicode")
                .count()
                >= 1
        );
        assert!(
            pdf.windows(b"/CMapType 2 def".len())
                .any(|window| window == b"/CMapType 2 def")
        );
        assert!(
            !pdf.windows(b"/CMapType 0 def".len())
                .any(|window| window == b"/CMapType 0 def")
        );
        assert!(
            pdf.windows(b"<61> <03B1>".len())
                .any(|window| window == b"<61> <03B1>"),
            "Symbol alpha byte must map back to Unicode alpha"
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "re"),
            "checkbox should render as passive vector geometry"
        );
    }

    #[test]
    fn smiley_dingbat_can_render_as_passive_vector_without_zapf_font_resource() {
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "\u{263a}".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(
            !pdf.windows(b"/BaseFont /ZapfDingbats".len())
                .any(|window| window == b"/BaseFont /ZapfDingbats"),
            "vector-only smiley should not require a viewer ZapfDingbats font"
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "c"),
            "vector-only smiley should draw passive Bezier paths"
        );
        assert!(content_bytes_for_font(&content, b"F14").is_empty());
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn writes_courier_font_resource_for_monospace_rtf_font() {
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
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(
            pdf.windows(b"/BaseFont /Courier".len())
                .any(|window| window == b"/BaseFont /Courier")
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tf" && format!("{:?}", operation.operands).contains("/F5")
        }));
    }

    #[test]
    fn writes_base14_font_resources_from_rtf_family_hints() {
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
                name: "Mystery Serif".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Roman,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 2,
                name: "Mystery Mono".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Modern,
                pitch: FontPitch::Default,
            },
        ];
        let mut roman_style = CharacterStyle::default();
        roman_style.font_index = 1;
        let mut modern_style = CharacterStyle::default();
        modern_style.font_index = 2;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    text: "Roman".to_string(),
                    style: roman_style,
                },
                Run {
                    text: "Modern".to_string(),
                    style: modern_style,
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tf" && format!("{:?}", operation.operands).contains("/F9")
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tf" && format!("{:?}", operation.operands).contains("/F5")
        }));
        for forbidden in [b"/JavaScript".as_slice(), b"/EmbeddedFile", b"/Launch"] {
            assert!(
                !pdf.windows(forbidden.len())
                    .any(|window| window == forbidden)
            );
        }
    }

    #[test]
    fn writes_courier_font_resource_for_fixed_pitch_font_hint() {
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
                name: "Mystery Fixed".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Nil,
                pitch: FontPitch::Fixed,
            },
        ];
        let mut style = CharacterStyle::default();
        style.font_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Fixed".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tf" && format!("{:?}", operation.operands).contains("/F5")
        }));
        assert!(
            !pdf.windows(b"Mystery Fixed".len())
                .any(|window| window == b"Mystery Fixed")
        );
    }

    #[test]
    fn writes_bounded_font_size_to_pdf_text_state() {
        let mut style = CharacterStyle::default();
        style.font_size_half_points = 96;
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Bounded".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Bounded");
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tf"
                && operation.operands.len() == 2
                && format!("{:?}", operation.operands[1]).contains("48")
        }));
    }

    #[test]
    fn writes_winansi_font_encoding_and_text_bytes() {
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "\u{201c}Quote\u{201d} \u{2014} \u{2022}".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(
            pdf.windows(b"/Encoding /WinAnsiEncoding".len())
                .any(|window| window == b"/Encoding /WinAnsiEncoding")
        );
        assert_eq!(content_bytes(&content), b"\x93Quote\x94 \x97 \x95");
    }

    #[test]
    fn writes_symbol_font_encoding_for_symbol_charset_text() {
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
                text: "\u{03b1}\u{03b2} \u{2022}".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(
            pdf.windows(b"/BaseFont /Symbol".len())
                .any(|window| window == b"/BaseFont /Symbol")
        );
        assert_eq!(content_bytes_for_font(&content, b"F13"), b"ab ");
        assert_eq!(content_bytes_for_font(&content, b"F1"), b"\x95");
    }

    #[test]
    fn supplied_font_assets_can_cover_symbol_family_text() {
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
                text: "\u{03b1}\u{03b2} ".to_string(),
                style,
            }],
        })];
        let provider = FontProvider {
            assets: vec![FontAsset {
                family_names: vec!["Symbol".to_string()],
                style: FontAssetStyle::default(),
                bytes: include_bytes!("../../fixtures/fonts/Tuffy.ttf").to_vec(),
            }],
            limits: Default::default(),
        };

        let layout = LayoutEngine::layout_with_font_provider(&document, Some(&provider));
        let pdf = render_pdf_with_font_provider(&layout, Some(&provider));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();
        let page_fonts = page_font_resource_names(&pdf, 0);

        assert!(
            pdf.windows(b"/Subtype /Type0".len())
                .any(|window| window == b"/Subtype /Type0")
        );
        assert!(
            pdf.windows(b"/FontFile2".len())
                .any(|window| window == b"/FontFile2")
        );
        assert!(!content_bytes_for_font(&content, b"TF1").is_empty());
        assert!(content_bytes_for_font(&content, b"F13").is_empty());
        assert!(page_fonts.iter().any(|name| name == b"TF1"));
        assert!(!page_fonts.iter().any(|name| name == b"F13"));
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn supplied_font_assets_can_cover_zapf_dingbats_family_text() {
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
                name: "ZapfDingbats".to_string(),
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
                text: "ABC".to_string(),
                style,
            }],
        })];
        let provider = FontProvider {
            assets: vec![FontAsset {
                family_names: vec!["ZapfDingbats".to_string()],
                style: FontAssetStyle::default(),
                bytes: include_bytes!("../../fixtures/fonts/Tuffy.ttf").to_vec(),
            }],
            limits: Default::default(),
        };

        let layout = LayoutEngine::layout_with_font_provider(&document, Some(&provider));
        let pdf = render_pdf_with_font_provider(&layout, Some(&provider));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();
        let page_fonts = page_font_resource_names(&pdf, 0);

        assert!(!content_bytes_for_font(&content, b"TF1").is_empty());
        assert!(content_bytes_for_font(&content, b"F14").is_empty());
        assert!(page_fonts.iter().any(|name| name == b"TF1"));
        assert!(!page_fonts.iter().any(|name| name == b"F14"));
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn writes_checkbox_dingbats_as_passive_vectors() {
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
                name: "ZapfDingbats".to_string(),
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
                text: "\u{25a1}\u{2610}\u{2611}\u{2612}\u{2713}\u{2717}".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(
            !pdf.windows(b"/BaseFont /ZapfDingbats".len())
                .any(|window| window == b"/BaseFont /ZapfDingbats"),
            "supported checkbox dingbats should not require a viewer ZapfDingbats font"
        );
        assert!(content_bytes_for_font(&content, b"F14").is_empty());
        assert!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "re")
                .count()
                >= 4,
            "checkbox boxes should render as passive rectangles"
        );
        assert!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "m")
                .count()
                >= 3
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
                .any(|operation| operation.operator == "S")
        );
    }

    #[test]
    fn supplied_font_kerning_uses_positioned_cids_at_inclusive_threshold() {
        let provider = FontProvider::browser_safe_defaults();
        let render = |font_size_half_points| {
            let mut document = Document::default();
            document.fonts = vec![FontDef {
                index: 0,
                name: "Arial".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            }];
            document.blocks = vec![Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "AVAVAV".to_string(),
                    style: CharacterStyle {
                        font_size_half_points,
                        character_kerning_half_points: 24,
                        ..Default::default()
                    },
                }],
            })];
            let layout = LayoutEngine::layout_with_font_provider(&document, Some(&provider));
            render_pdf_with_font_provider(&layout, Some(&provider))
        };

        let below_threshold = render(23);
        let below = lopdf::Document::load_mem(&below_threshold).unwrap();
        let below_page = *below.get_pages().values().next().expect("page");
        let below_content = below.get_and_decode_page_content(below_page).unwrap();
        assert!(
            below_content
                .operations
                .iter()
                .all(|operation| operation.operator != "TJ")
        );

        let at_threshold = render(24);
        let at = lopdf::Document::load_mem(&at_threshold).unwrap();
        let at_page = *at.get_pages().values().next().expect("page");
        let at_content = at.get_and_decode_page_content(at_page).unwrap();
        let positioned = at_content
            .operations
            .iter()
            .find(|operation| operation.operator == "TJ")
            .expect("supplied-font kerning should position CIDs at the inclusive threshold");
        assert!(positioned.operands.iter().any(|operand| {
            operand.as_array().is_ok_and(|items| {
                items.iter().any(|item| {
                    matches!(item, lopdf::Object::Integer(value) if *value > 0)
                        || matches!(item, lopdf::Object::Real(value) if *value > 0.0)
                })
            })
        }));
        assert!(
            at_threshold
                .windows(b"/Subtype /Type0".len())
                .any(|window| window == b"/Subtype /Type0")
        );
        audit_passive_pdf_bytes(&below_threshold).unwrap();
        audit_passive_pdf_bytes(&at_threshold).unwrap();
    }
