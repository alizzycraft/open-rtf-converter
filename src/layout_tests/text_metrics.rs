    #[test]
    fn lays_out_text_with_bounded_font_size() {
        let mut document = Document::default();
        let mut style = CharacterStyle::default();
        style.font_size_half_points = 96;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                dynamic_field: None,
                text: "Bounded".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let fragment = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "Bounded" => Some(fragment),
                _ => None,
            })
            .expect("bounded text");

        assert_eq!(fragment.style.font_size_half_points, 96);
        assert!(fragment.baseline_y.is_finite());
    }

    #[test]
    fn passive_kerning_reduces_measured_pair_width_when_threshold_matches() {
        let mut kerned = CharacterStyle {
            character_kerning_half_points: 2,
            ..Default::default()
        };
        let plain_width = measure_text("AV", &CharacterStyle::default());
        let kerned_width = measure_text("AV", &kerned);

        assert!(kerned_width < plain_width);
        assert!((plain_width - kerned_width - 1.32).abs() < 0.01);

        kerned.character_kerning_half_points = 48;
        assert!((measure_text("AV", &kerned) - plain_width).abs() < 0.01);
    }

    #[test]
    fn zero_width_formatting_marks_do_not_measure_or_emit_glyphs() {
        let mut spaced = CharacterStyle::default();
        spaced.character_spacing_twips = 200;
        let marked_text = "A\u{200b}\u{feff}\u{200c}\u{200d}\u{200e}\u{200f}B";
        assert!((measure_text(marked_text, &spaced) - measure_text("AB", &spaced)).abs() < 0.01);

        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                dynamic_field: None,
                text: marked_text.to_string(),
                style: spaced,
            }],
        })];
        let layout = LayoutEngine::layout(&document);
        let rendered_text = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment.text.as_str()),
                _ => None,
            })
            .collect::<String>();

        assert_eq!(rendered_text, "AB");
    }

    #[test]
    fn caller_font_metrics_drive_layout_widths_without_system_fonts() {
        let mut document = Document::default();
        document.fonts = vec![FontDef {
            index: 0,
            name: "Tuffy".to_string(),
            alternate_name: None,
            charset: None,
            code_page: None,
            family: FontFamilyHint::Swiss,
            pitch: FontPitch::Default,
        }];
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    dynamic_field: None,
                    text: "A".to_string(),
                    style: Default::default(),
                },
                Run {
                    dynamic_field: None,
                    text: "B".to_string(),
                    style: Default::default(),
                },
            ],
        })];
        let provider = FontProvider {
            assets: vec![FontAsset {
                family_names: vec!["Tuffy".to_string()],
                style: FontAssetStyle::default(),
                bytes: include_bytes!("../../fixtures/fonts/Tuffy.ttf").to_vec().into(),
            }],
            limits: FontProviderLimits {
                max_asset_bytes: 256 * 1024,
                max_total_bytes: 256 * 1024,
                ..FontProviderLimits::default()
            },
        };
        provider.validate().unwrap();
        let metrics = provider.glyph_metrics_for_char("Tuffy", 'A').unwrap();

        let fallback_layout = LayoutEngine::layout(&document);
        let provided_layout = LayoutEngine::layout_with_font_provider(&document, Some(&provider));
        let fallback_advance = text_x(&fallback_layout.pages[0], "B").unwrap()
            - text_x(&fallback_layout.pages[0], "A").unwrap();
        let provided_advance = text_x(&provided_layout.pages[0], "B").unwrap()
            - text_x(&provided_layout.pages[0], "A").unwrap();
        let expected_advance = metrics.advance_points(CharacterStyle::default().font_size_points());

        assert!((provided_advance - expected_advance).abs() < 0.01);
        assert!(
            (provided_advance - fallback_advance).abs() > 0.1,
            "provided metrics should visibly differ from fallback width"
        );
    }

    #[test]
    fn word_hint_substitutes_use_bounded_metrics_and_yield_to_exact_faces() {
        let provider = FontProvider::browser_safe_defaults();
        let style = CharacterStyle {
            font_index: 0,
            font_size_half_points: 24,
            ..Default::default()
        };
        let source_font = |name: &str, alternate: &str, family| FontDef {
            index: 0,
            name: name.to_string(),
            alternate_name: Some(alternate.to_string()),
            charset: None,
            code_page: None,
            family,
            pitch: FontPitch::Default,
        };
        let cambria = source_font("Mystery Serif", "Cambria", FontFamilyHint::Roman);
        let aptos = source_font("Mystery Body", "Aptos", FontFamilyHint::Swiss);

        assert!(
            (source_word_compatible_line_height_points(Some(&cambria), 12.0, Some(&provider))
                .expect("Cambria substitute line metric")
                - (12.0 * 2_401.0 / 2_048.0))
                .abs()
                < 0.001
        );
        assert!(
            (source_word_compatible_advance_points(
                Some(&cambria),
                &style,
                'R',
                12.0,
                Some(&provider),
            )
            .expect("Cambria substitute advance")
                - (12.0 * 1_272.0 / 2_048.0))
                .abs()
                < 0.001
        );
        assert!(
            (source_word_compatible_line_height_points(Some(&aptos), 12.0, Some(&provider))
                .expect("Aptos substitute line metric")
                - (12.0 * 2_500.0 / 2_048.0))
                .abs()
                < 0.001
        );
        assert!(
            (source_word_compatible_advance_points(
                Some(&aptos),
                &style,
                'T',
                12.0,
                Some(&provider),
            )
            .expect("Aptos substitute advance")
                - (12.0 * 980.0 / 2_048.0))
                .abs()
                < 0.001
        );

        let exact_provider = FontProvider {
            assets: vec![FontAsset {
                family_names: vec!["Mystery Serif".to_string()],
                style: FontAssetStyle::default(),
                bytes: include_bytes!("../../fixtures/fonts/Tuffy.ttf").to_vec().into(),
            }],
            limits: FontProviderLimits::default(),
        };
        assert!(
            source_word_compatible_advance_points(
                Some(&cambria),
                &style,
                'R',
                12.0,
                Some(&exact_provider),
            )
            .is_none()
        );
    }

    #[test]
    fn resolves_symbol_font_family_from_charset_hint() {
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
                dynamic_field: None,
                text: "\u{03b1}".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.font_family == PdfFontFamily::Symbol)
        ));
    }

    #[test]
    fn keeps_winansi_punctuation_in_normal_text_fonts() {
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
                text: "\u{2026}".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "\u{2026}" && fragment.font_family == PdfFontFamily::Times)
        ));
        assert!(
            !layout.pages[0]
                .items
                .iter()
            .any(|item| matches!(item, LayoutItem::Text(fragment) if fragment.font_family == PdfFontFamily::Symbol))
        );
    }

    #[test]
    fn arial_lines_use_word_source_baselines_without_changing_pitch() {
        let provider = FontProvider::browser_safe_defaults();
        let first_fragment = |rtf: &str, needle: &str| {
            let parsed = parse_rtf(rtf).unwrap();
            let layout = LayoutEngine::layout_with_font_provider(&parsed.document, Some(&provider));
            let fragment = layout.pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.contains(needle) => Some(fragment),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing text fragment containing {needle:?}"));
            (layout.pages[0].height, fragment.baseline_y)
        };

        let (twelve_point_page_height, twelve_point_baseline) = first_fragment(
            include_str!("../../fixtures/nonvisible-storage-fields-passive.rtf"),
            "Visible",
        );
        let (twenty_four_point_page_height, twenty_four_point_baseline) = first_fragment(
            include_str!("../../fixtures/paragraph-keep-pagination-passive.rtf"),
            "Filler",
        );

        assert!((twelve_point_page_height - twelve_point_baseline - 47.30).abs() < 0.03);
        assert!((twenty_four_point_page_height - twenty_four_point_baseline - 40.60).abs() < 0.03);
    }

    #[test]
    fn calibri_lines_use_word_source_baselines() {
        let parsed =
            parse_rtf(include_str!("../../fixtures/stylesheet-metadata-passive.rtf")).unwrap();
        let provider = FontProvider::browser_safe_defaults();
        let layout = LayoutEngine::layout_with_font_provider(&parsed.document, Some(&provider));
        let page = &layout.pages[0];
        let baseline_from_top = |needle: &str| {
            page.height
                - page
                    .items
                    .iter()
                    .find_map(|item| match item {
                        LayoutItem::Text(fragment) if fragment.text.contains(needle) => {
                            Some(fragment.baseline_y)
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("missing text fragment containing {needle:?}"))
        };
        let heading = baseline_from_top("Heading");

        assert!((heading - 83.42).abs() < 0.03, "heading baseline={heading}");
    }

    #[test]
    fn times_lines_use_word_source_baselines_for_explicit_and_legacy_defaults() {
        let provider = FontProvider::browser_safe_defaults();
        let first_fragment = |rtf: &str, needle: &str| {
            let parsed = parse_rtf(rtf).unwrap();
            let layout = LayoutEngine::layout_with_font_provider(&parsed.document, Some(&provider));
            let fragment = layout.pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.contains(needle) => Some(fragment),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing text fragment containing {needle:?}"));
            (layout.pages[0].height, fragment.baseline_y)
        };

        let (legacy_page_height, legacy_baseline) =
            first_fragment(include_str!("../../fixtures/table-ish.rtf"), "Name");
        let (explicit_page_height, explicit_baseline) =
            first_fragment(include_str!("../../docs/sample.rtf"), "It");

        assert!((legacy_page_height - legacy_baseline - 83.18).abs() < 0.03);
        assert!((explicit_page_height - explicit_baseline - 85.05).abs() < 0.03);
    }

    #[test]
    fn measures_legacy_dingbat_glyphs_with_word_source_font_advances() {
        let font = |name: &str| FontDef {
            index: 1,
            name: name.to_string(),
            alternate_name: None,
            charset: None,
            code_page: None,
            family: FontFamilyHint::Tech,
            pitch: FontPitch::Default,
        };
        let size = 12.0;

        assert_eq!(
            source_dingbat_advance_points(Some(&font("Wingdings 3")), ' ', size),
            Some(12.0)
        );
        assert_eq!(
            source_dingbat_advance_points(Some(&font("Wingdings 3")), '\u{2190}', size),
            Some(10.68)
        );
        assert_eq!(
            source_dingbat_advance_points(Some(&font("Webdings")), ' ', size),
            Some(6.0)
        );
        assert_eq!(
            source_dingbat_advance_points(Some(&font("Wingdings 2")), '\u{2611}', size),
            Some(10.704)
        );
        assert_eq!(
            source_dingbat_advance_points(Some(&font("Segoe UI Symbol")), '\u{2610}', size),
            Some(10.32)
        );
        assert_eq!(
            source_dingbat_advance_points(Some(&font("Segoe UI Symbol")), '\u{2713}', size),
            Some(9.0)
        );
        assert_eq!(
            source_dingbat_advance_points(Some(&font("Segoe UI Symbol")), '\u{2717}', size),
            Some(9.96)
        );
    }

    #[test]
    fn measures_cambria_math_with_bounded_word_source_advances() {
        let font = FontDef {
            index: 1,
            name: "Cambria Math".to_string(),
            alternate_name: None,
            charset: None,
            code_page: None,
            family: FontFamilyHint::Roman,
            pitch: FontPitch::Default,
        };
        let size = 12.0;
        let mut style = CharacterStyle::default();
        style.passive_math.enabled = true;
        let expected = [
            ('\u{2211}', 1451.0 / 2048.0),
            ('\u{220f}', 1663.0 / 2048.0),
            ('=', 1530.0 / 2048.0),
            ('1', 1134.0 / 2048.0),
            ('m', 1704.0 / 2048.0),
        ];

        for (ch, expected_em) in expected {
            let advance =
                source_word_compatible_advance_points(Some(&font), &style, ch, size, None).expect(
                    "registered Cambria Math character should have a bounded source metric",
                );
            assert!((advance - (size * expected_em)).abs() < 0.000_01);
        }
        assert_eq!(
            source_word_compatible_advance_points(Some(&font), &style, '\u{1f642}', size, None,),
            None
        );
        style.passive_math.nary_id = None;
        assert!(
            source_word_compatible_advance_points(Some(&font), &style, 'm', size, None).is_some()
        );
        style.passive_math.fraction_id = Some(MathGroupId::new(1));
        assert_eq!(
            source_word_compatible_advance_points(Some(&font), &style, '+', size, None),
            None,
            "stacked fractions retain their independently calibrated bounded width model"
        );
        style.passive_math.fraction_id = None;
        style.passive_math.enabled = false;
        assert_eq!(
            source_word_compatible_advance_points(Some(&font), &style, 'm', size, None),
            None
        );
    }

    #[test]
    fn splits_unicode_symbol_spans_to_passive_symbol_font() {
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
                dynamic_field: None,
                text: "Alpha \u{03b1}+\u{03b2} <= \u{03a9} \u{2717}".to_string(),
                style: CharacterStyle::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let families = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment.font_family),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(families.contains(&PdfFontFamily::Helvetica));
        assert!(families.contains(&PdfFontFamily::Symbol));
        assert!(families.contains(&PdfFontFamily::ZapfDingbats));
    }

    #[test]
    fn resolves_safe_pdf_font_family_from_rtf_family_hints() {
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
                    dynamic_field: None,
                    text: "Roman".to_string(),
                    style: roman_style,
                },
                Run {
                    dynamic_field: None,
                    text: "Modern".to_string(),
                    style: modern_style,
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Roman" && fragment.font_family == PdfFontFamily::Times)
        ));
        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Modern" && fragment.font_family == PdfFontFamily::Courier)
        ));
    }

    #[test]
    fn resolves_common_office_font_names_to_passive_base14_families() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Calibri".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Nil,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Cambria".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Nil,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 2,
                name: "Aptos Mono".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Nil,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 3,
                name: "MS Sans Serif".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Nil,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 4,
                name: "MS Serif".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Nil,
                pitch: FontPitch::Default,
            },
        ];
        let mut sans_style = CharacterStyle {
            font_index: 0,
            ..Default::default()
        };
        sans_style.font_index = 0;
        let serif_style = CharacterStyle {
            font_index: 1,
            ..Default::default()
        };
        let mono_style = CharacterStyle {
            font_index: 2,
            ..Default::default()
        };
        let legacy_sans_style = CharacterStyle {
            font_index: 3,
            ..Default::default()
        };
        let legacy_serif_style = CharacterStyle {
            font_index: 4,
            ..Default::default()
        };
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    dynamic_field: None,
                    text: "Sans".to_string(),
                    style: sans_style,
                },
                Run {
                    dynamic_field: None,
                    text: "Serif".to_string(),
                    style: serif_style,
                },
                Run {
                    dynamic_field: None,
                    text: "Mono".to_string(),
                    style: mono_style,
                },
                Run {
                    dynamic_field: None,
                    text: "LegacySans".to_string(),
                    style: legacy_sans_style,
                },
                Run {
                    dynamic_field: None,
                    text: "LegacySerif".to_string(),
                    style: legacy_serif_style,
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Sans" && fragment.font_family == PdfFontFamily::Helvetica)
        ));
        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Serif" && fragment.font_family == PdfFontFamily::Times)
        ));
        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Mono" && fragment.font_family == PdfFontFamily::Courier)
        ));
        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "LegacySans" && fragment.font_family == PdfFontFamily::Helvetica)
        ));
        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "LegacySerif" && fragment.font_family == PdfFontFamily::Times)
        ));
    }

    #[test]
    fn resolves_safe_pdf_font_family_from_alternate_font_name() {
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
                name: "Mystery Sans".to_string(),
                alternate_name: Some("Courier New".to_string()),
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
                dynamic_field: None,
                text: "Fallback".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Fallback" && fragment.font_family == PdfFontFamily::Courier)
        ));
    }

    #[test]
    fn resolves_fixed_pitch_font_hint_to_courier_fallback() {
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
            FontDef {
                index: 2,
                name: "Mystery Variable".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Nil,
                pitch: FontPitch::Variable,
            },
        ];
        let mut fixed_style = CharacterStyle {
            font_index: 1,
            ..Default::default()
        };
        fixed_style.font_index = 1;
        let variable_style = CharacterStyle {
            font_index: 2,
            ..Default::default()
        };
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    dynamic_field: None,
                    text: "Fixed".to_string(),
                    style: fixed_style,
                },
                Run {
                    dynamic_field: None,
                    text: "Variable".to_string(),
                    style: variable_style,
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Fixed" && fragment.font_family == PdfFontFamily::Courier)
        ));
        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Variable" && fragment.font_family == PdfFontFamily::Helvetica)
        ));
    }
