    #[test]
    fn rejects_invalid_signed_limits_without_panicking() {
        let mut options = RtfParseOptions::default();
        options.limits.max_table_row_offset_twips = -1;
        assert!(matches!(
            parse_rtf_bytes_with_options(br"{\rtf1\trowd\trleft10\cellx100 A\cell\row}", &options),
            Err(ParseError::InvalidLimits(LimitValidationError::Negative {
                field: "max_table_row_offset_twips",
                value: -1,
            }))
        ));

        let mut options = RtfParseOptions::default();
        options.limits.min_page_dimension_twips = 2_000;
        options.limits.max_page_dimension_twips = 1_000;
        assert!(matches!(
            parse_rtf_bytes_with_options(br"{\rtf1\paperw1500 Text}", &options),
            Err(ParseError::InvalidLimits(
                LimitValidationError::InvertedRange {
                    min_field: "min_page_dimension_twips",
                    min: 2_000,
                    max_field: "max_page_dimension_twips",
                    max: 1_000,
                }
            ))
        ));

        let mut options = RtfParseOptions::default();
        options.limits.min_image_scaling_percent = 200;
        options.limits.max_image_scaling_percent = 100;
        assert!(matches!(
            parse_rtf_bytes_with_options(br"{\rtf1{\pict\picscalex150 00}}", &options),
            Err(ParseError::InvalidLimits(
                LimitValidationError::InvertedRange {
                    min_field: "min_image_scaling_percent",
                    min: 200,
                    max_field: "max_image_scaling_percent",
                    max: 100,
                }
            ))
        ));
    }

    #[test]
    fn hidden_resultless_active_content_does_not_create_placeholders() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\v {\field{\*\fldinst INCLUDEPICTURE "https://example.com/a.png"}}{\object\objdata 414243}{\shp{\shpinst{\sp{\sn pFragments}{\sv hidden-payload}}}}} After\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before  After"));
        assert!(!text.contains("[Field removed"));
        assert!(!text.contains("[Embedded object removed]"));
        assert!(!text.contains("[Shape skipped"));
        assert!(!text.contains("https://example.com"));
        assert!(!text.contains("414243"));
        assert!(!text.contains("hidden-payload"));
    }

    #[test]
    fn note_count_limit_bounds_combined_footnotes_and_endnotes() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_notes: 1,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        let result = parse_rtf_bytes_with_options(
            br"{\rtf1 Body\chftn{\footnote First footnote\par} End\chftn{\endnote Second endnote\par}\par}",
            &options,
        );

        assert!(matches!(
            result,
            Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "notes"
        ));
    }

    #[test]
    fn invalid_fet_value_is_ignored_without_changing_note_type() {
        let output =
            parse_rtf(r"{\rtf1\fet99 Body\chftn{\footnote \chftn Footnote text\par}\par}").unwrap();

        assert_eq!(output.document.footnotes.len(), 1);
        assert!(output.document.endnotes.is_empty());
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("note type value 99 ignored"))
        );
    }

    #[test]
    fn resultless_set_values_reject_active_pdf_markers_before_refs() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\field{\*\fldinst SET Unsafe "PAYLOAD /JavaScript"}} ref {\field{\*\fldinst REF Unsafe}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Before  ref [Field removed: no passive result]"),
            "text was {text:?}"
        );
        for forbidden in ["PAYLOAD", "/JavaScript", "SET", "REF Unsafe", "fldinst"] {
            assert!(
                !text.contains(forbidden),
                "unsafe SET/REF field leaked text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_set_values_reject_form_and_signature_pdf_markers_before_refs() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\field{\*\fldinst SET Unsafe "PAYLOAD /FDF /NeedAppearances /XDP /XFA /SigFlags /DocMDP"}} ref {\field{\*\fldinst REF Unsafe}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Before  ref [Field removed: no passive result]"),
            "text was {text:?}"
        );
        for forbidden in [
            "PAYLOAD",
            "/FDF",
            "/NeedAppearances",
            "/XDP",
            "/XFA",
            "/SigFlags",
            "/DocMDP",
            "SET",
            "REF Unsafe",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "unsafe form/signature SET/REF field leaked text: {forbidden}"
            );
        }
    }

    #[test]
    fn bookmark_names_reject_active_pdf_markers_before_refs() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\*\bkmkstart "/JavaScript"}Hidden{\*\bkmkend "/JavaScript"} ref {\field{\*\fldinst REF "/JavaScript"}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Before Hidden ref [Field removed: no passive result]"),
            "text was {text:?}"
        );
        for forbidden in ["/JavaScript", "bkmkstart", "bkmkend", "REF", "fldinst"] {
            assert!(
                !text.contains(forbidden),
                "unsafe bookmark/REF field leaked text: {forbidden}"
            );
        }
    }

    #[test]
    fn malformed_resultless_advance_arguments_recover_as_word_literal_text() {
        let output =
            parse_rtf(r"{\rtf1 Advance {\field{\*\fldinst ADVANCE \r 240 \d 120}}after\par}")
                .unwrap();
        let text = document_text(&output.document);

        assert_eq!(text, "Advance 240 120after");
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(
                    "malformed ADVANCE arguments recovered as Word-compatible passive literal text"
                ))
        );
    }

    #[test]
    fn document_property_limit_bounds_combined_custom_properties_and_variables() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_document_properties: 1,
                max_styles: 0,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        let result = parse_rtf_bytes_with_options(
            br#"{\rtf1{\*\docvar {Client}{Contoso}}{\*\userprops{\propname Region}{\proptype30}{\staticval North}}\par}"#,
            &options,
        );

        assert!(
            matches!(
                result,
                Err(ParseError::ResourceLimitExceeded { ref resource, .. })
                    if resource == "custom document properties"
            ),
            "unexpected result: {result:?}"
        );
    }

    #[test]
    fn strict_spec_form_checkbox_uses_bounded_passive_size_metadata() {
        let options = RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br#"{\rtf1\fs20 Before {\field{\*\fldinst FORMCHECKBOX}{\formfield{\fftype1}{\ffres1}{\ffhps40}{\ffentrymcr launch.exe}{\datafield 414243}}} After\par}"#,
            &options,
        )
        .unwrap();
        let paragraph = match output.document.blocks.first().expect("paragraph") {
            Block::Paragraph(paragraph) => paragraph,
            block => panic!("unexpected block: {block:?}"),
        };
        let checkbox = paragraph
            .runs
            .iter()
            .find(|run| run.text == "\u{2611}")
            .expect("checkbox run");

        assert_eq!(checkbox.style.font_size_half_points, 40);
        assert_ne!(
            checkbox.style.font_size_half_points,
            paragraph
                .runs
                .iter()
                .find(|run| run.text.contains("Before"))
                .expect("ambient run")
                .style
                .font_size_half_points
        );
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "rendering passive form field FORMCHECKBOX without creating PDF form actions",
            )
        }));
    }

    #[test]
    fn resultless_form_dropdown_entry_count_is_bounded() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_form_field_entries: 1,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let err = parse_rtf_bytes_with_options(
            br"{\rtf1{\field{\*\fldinst FORMDROPDOWN}{\formfield{\fftype2}{\*\ffl One}{\*\ffl Two}}}\par}",
            &options,
        )
        .unwrap_err();

        assert!(matches!(
            err,
            ParseError::ResourceLimitExceeded { ref resource, .. }
                if resource == "form dropdown entries"
        ));
    }

    #[test]
    fn object_metadata_destinations_do_not_become_body_text() {
        let output = parse_rtf(
            r"{\rtf1 Before{\objclass HiddenClass {\object\objdata 414243}} middle{\objname HiddenName}{\objalias HiddenAlias}{\objtopic HiddenTopic} after\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before middle after"));
        for forbidden in [
            "HiddenClass",
            "HiddenName",
            "HiddenAlias",
            "HiddenTopic",
            "Embedded object removed",
            "objclass",
            "objname",
            "objalias",
            "objtopic",
            "objdata",
            "414243",
        ] {
            assert!(
                !text.contains(forbidden),
                "object metadata leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn object_metadata_inside_object_does_not_block_safe_result() {
        let output = parse_rtf(
            r"{\rtf1{\object{\*\objclass HiddenClass}{\objname HiddenName}\objdata 414243{\result visible fallback}}}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("visible fallback"));
        assert!(!text.contains("[Embedded object removed]"));
        assert!(!text.contains("HiddenClass"));
        assert!(!text.contains("HiddenName"));
        assert!(!text.contains("414243"));
    }

    #[test]
    fn header_object_result_stays_in_safe_repeating_metadata() {
        let output = parse_rtf(
            r"{\rtf1{\header Prefix {\object\objdata 414243{\result Object fallback\par}}\par}Body\par}",
        )
        .unwrap();
        let header_text = output.document.header[0]
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();
        let body_text = document_text(&output.document);

        assert_eq!(header_text, "Prefix Object fallback");
        assert_eq!(body_text, "Body");
        for forbidden in ["objdata", "414243", "[Embedded object removed]"] {
            assert!(
                !header_text.contains(forbidden),
                "object internals leaked to header result text: {forbidden}"
            );
            assert!(
                !body_text.contains(forbidden),
                "object internals leaked to body text: {forbidden}"
            );
        }
    }

    #[test]
    fn header_resultless_object_placeholder_stays_in_safe_repeating_metadata() {
        let output =
            parse_rtf(r"{\rtf1{\header Prefix {\object\objdata 414243}\par}Body\par}").unwrap();
        let header_text = output.document.header[0]
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();
        let body_text = document_text(&output.document);

        assert_eq!(header_text, "Prefix [Embedded object removed]");
        assert_eq!(body_text, "Body");
        assert!(!body_text.contains("[Embedded object removed]"));
        assert!(!header_text.contains("objdata"));
        assert!(!header_text.contains("414243"));
    }

    #[test]
    fn active_controls_nested_in_metadata_remain_non_visual() {
        let output = parse_rtf(
            r#"{\rtf1 Start{\info{\title Hidden {\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt Hidden link}}{\pict\pngblip 00}{\shp{\shpinst{\shptxt Hidden shape}}}{\header Hidden header\par}{\listtext Hidden marker\tab}}} End\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Start End"));
        assert!(!text.contains("Hidden"));
        assert!(!text.contains("https://example.com"));
        assert!(!text.contains("Image skipped"));
        assert!(!text.contains("Shape skipped"));
        assert!(!text.contains("Field removed"));
    }

    #[test]
    fn external_template_destination_obeys_reject_policy() {
        let options = RtfParseOptions {
            active_content_policy: ActiveContentPolicy::Reject,
            ..RtfParseOptions::default()
        };

        assert!(matches!(
            parse_rtf_bytes_with_options(
                br"{\rtf1\template https://example.com/t.dotm Body}",
                &options,
            ),
            Err(ParseError::ActiveContentRejected { feature, .. }) if feature == "external template"
        ));
    }

    #[test]
    fn suppresses_malformed_legacy_polyline_vertex_counts_in_word_compatible_mode() {
        for drawing in [
            r"{\do\dppolyline\dpptx0\dppty0\dpptx10\dppty10\dpptx20\dppty0}",
            r"{\do\dppolygon\dppolycount2\dpptx0\dppty0\dpptx10\dppty10\dpptx20\dppty0}",
        ] {
            let output = parse_rtf(&format!(r"{{\rtf1 Before\par{drawing}After\par}}")).unwrap();

            assert!(
                output
                    .document
                    .blocks
                    .iter()
                    .all(|block| !matches!(block, Block::Shape(_)))
            );
            assert!(!document_text(&output.document).contains("Shape skipped"));
            assert!(output.diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("malformed legacy polyline vertex count suppressed")
            }));
        }
    }

    #[test]
    fn disabling_engrave_restores_still_active_emboss_state() {
        let output = parse_rtf(
            r"{\rtf1\embo embossed \impr engraved \impr0 embossed again\par next paragraph\par}",
        )
        .unwrap();
        let style_for = |paragraph_index: usize, text: &str| {
            let Block::Paragraph(paragraph) = &output.document.blocks[paragraph_index] else {
                panic!("expected paragraph");
            };
            paragraph
                .runs
                .iter()
                .find(|run| run.text.contains(text))
                .map(|run| run.style.relief)
                .expect("expected matching run")
        };

        assert_eq!(style_for(0, "embossed "), TextRelief::Emboss);
        assert_eq!(style_for(0, "engraved"), TextRelief::Emboss);
        assert_eq!(style_for(0, "embossed again"), TextRelief::Emboss);
        assert_eq!(style_for(1, "next paragraph"), TextRelief::Emboss);
    }

    #[test]
    fn later_section_settings_are_updated_retroactively_after_visible_text() {
        let output =
            parse_rtf(r"{\rtf1 First\par\sect\sectd\vertalc Center\par\vertalt Top reset\par}")
                .unwrap();
        let settings = match &output.document.blocks[2] {
            Block::SectionSettings(settings) => settings,
            _ => panic!("expected section page settings block"),
        };

        assert_eq!(settings.vertical_alignment, PageVerticalAlignment::Top);
        assert_eq!(
            output
                .document
                .blocks
                .iter()
                .filter(|block| matches!(block, Block::SectionSettings(_)))
                .count(),
            1
        );
    }

    #[test]
    fn right_to_left_character_direction_is_bounded_passive_visual_metadata() {
        let output = parse_rtf(
            r"{\rtf1 Normal {\rtlch \u1513?\u1500?\u1493?\u1501?} {\ltrch LTR text}\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert_eq!(text, "Normal \u{5e9}\u{5dc}\u{5d5}\u{5dd} LTR text");
        assert!(!text.contains("rtlch"));
        assert!(!text.contains("ltrch"));
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected paragraph, got {other:?}"),
        };
        let rtl_run = paragraph
            .runs
            .iter()
            .find(|run| run.text.contains("\u{5e9}\u{5dc}\u{5d5}\u{5dd}"))
            .expect("RTL run");
        let ltr_run = paragraph
            .runs
            .iter()
            .find(|run| run.text.contains("LTR text"))
            .expect("LTR run");
        assert!(rtl_run.style.right_to_left);
        assert!(!ltr_run.style.right_to_left);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "right-to-left character direction rendered by bounded passive visual text order",
            )
        }));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
            "character direction controls should not be unsupported: {:?}",
            output.diagnostics
        );
    }

    #[test]
    fn consecutive_hyphenation_limit_sets_paragraph_default_across_resets() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_hyphenation_consecutive_lines: 2,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\hyphauto\hyphconsec1\pard Limited\par\hyphconsec0\pard Unlimited\par\hyphconsec999\pard Clamped\par}",
            &options,
        )
        .unwrap();
        let paragraph = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected paragraph {index}, got {other:?}"),
        };

        assert!(paragraph(0).style.auto_hyphenation);
        assert_eq!(paragraph(0).style.max_consecutive_hyphenated_lines, Some(1));
        assert!(paragraph(1).style.auto_hyphenation);
        assert_eq!(paragraph(1).style.max_consecutive_hyphenated_lines, None);
        assert!(paragraph(2).style.auto_hyphenation);
        assert_eq!(paragraph(2).style.max_consecutive_hyphenated_lines, Some(2));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("consecutive automatic hyphenation limit applied")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("consecutive hyphenation limit clamped from 999 to 2")
        }));
    }

    #[test]
    fn unequal_paragraph_frame_intersections_remain_bounded_metadata() {
        let output = parse_rtf(include_str!(
            "../../../fixtures/framed-drop-cap-overlap-geometry-passive.rtf"
        ))
        .unwrap();
        let frames = output
            .document
            .blocks
            .iter()
            .filter(|block| {
                matches!(block, Block::Paragraph(paragraph) if paragraph.style.drop_cap_lines > 1)
            })
            .count();
        assert_eq!(frames, 10);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "paragraph frame control \\absnoovrlp retained through bounded passive placement",
            )
        }));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn ordinary_paragraph_frames_retain_only_bounded_passive_metadata() {
        let output = parse_rtf(include_str!("../../../fixtures/paragraph-frame-passive.rtf"))
            .expect("ordinary paragraph-frame fixture should parse");
        let frames = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph)
                    if paragraph.style.frame_width_twips.is_some()
                        || paragraph.style.frame_height_twips.is_some() =>
                {
                    Some(paragraph)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(frames.len(), 4);
        assert!(
            frames
                .iter()
                .all(|paragraph| paragraph.style.drop_cap_lines == 0)
        );
        assert_eq!(frames[0].style.frame_width_twips, Some(1_440));
        assert_eq!(frames[0].style.frame_height_twips, Some(1_440));
        assert_eq!(frames[1].style.frame_width_twips, Some(1_800));
        assert_eq!(frames[2].style.frame_width_twips, Some(1_800));
        assert_eq!(frames[3].style.frame_width_twips, Some(1_440));
        assert_eq!(frames[3].style.frame_height_twips, None);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn ordinary_paragraph_frame_height_and_width_signs_remain_bounded() {
        let output = parse_rtf(include_str!(
            "../../../fixtures/paragraph-frame-height-flow-passive.rtf"
        ))
        .expect("paragraph-frame height/flow fixture should parse");
        let frames = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph)
                    if paragraph.style.frame_width_twips.is_some()
                        || paragraph.style.frame_height_twips.is_some() =>
                {
                    Some(paragraph)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(frames.len(), 6);
        assert_eq!(frames[0].style.frame_height_twips, Some(360));
        assert_eq!(frames[1].style.frame_height_twips, Some(-360));
        assert_eq!(
            frames[2].style.frame_vertical_anchor,
            StaticShapeVerticalAnchor::Paragraph
        );
        assert_eq!(
            frames[3].style.frame_vertical_anchor,
            StaticShapeVerticalAnchor::Paragraph
        );
        assert_eq!(frames[4].style.frame_height_twips, None);
        assert_eq!(frames[5].style.frame_width_twips, Some(-1_440));
        assert_eq!(frames[5].style.frame_height_twips, Some(720));
        assert!(
            frames
                .iter()
                .all(|paragraph| paragraph.style.drop_cap_lines == 0)
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn word_compatible_mode_retains_bounded_drop_cap_frame_positioning() {
        let output = parse_rtf(include_str!(
            "../../../fixtures/framed-drop-cap-positioning-passive.rtf"
        ))
        .unwrap();
        let paragraphs = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph) => Some(paragraph),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            paragraphs[2].style.frame_horizontal_anchor,
            StaticShapeHorizontalAnchor::Margin
        );
        assert_eq!(paragraphs[2].style.frame_horizontal_offset_twips, 720);
        assert_eq!(paragraphs[2].style.frame_horizontal_alignment, None);
        assert_eq!(
            paragraphs[4].style.frame_horizontal_anchor,
            StaticShapeHorizontalAnchor::Page
        );
        assert_eq!(paragraphs[4].style.frame_horizontal_offset_twips, 720);
        assert_eq!(
            paragraphs[6].style.frame_horizontal_alignment,
            Some(TableRowAlignment::Center)
        );
        assert_eq!(
            paragraphs[8].style.frame_vertical_anchor,
            StaticShapeVerticalAnchor::Paragraph
        );
        assert_eq!(paragraphs[8].style.frame_vertical_offset_twips, 360);
        assert_eq!(
            paragraphs[10].style.frame_horizontal_alignment,
            Some(TableRowAlignment::Right)
        );
        assert_eq!(paragraphs[1].style, ParagraphStyle::default());
    }

    #[test]
    fn paragraph_frame_position_offsets_clamp_malformed_values() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_shape_offset_twips: 480,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\posx9999\absw720\dropcapli3\dropcapt1 X\par\pard\posnegy9999\absw720\dropcapli3\dropcapt1 Y\par\pard\posnegy-9999\absw720\dropcapli3\dropcapt1 Z\par}",
            &options,
        )
        .unwrap();
        let paragraphs = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph) => Some(paragraph),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(paragraphs[0].style.frame_horizontal_offset_twips, 480);
        assert_eq!(paragraphs[1].style.frame_vertical_offset_twips, 480);
        assert_eq!(paragraphs[2].style.frame_vertical_offset_twips, -480);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("paragraph frame horizontal position clamped from 9999 to 480 twips")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("paragraph frame vertical position clamped from -9999 to -480 twips")
        }));
    }

    #[test]
    fn recovers_extreme_page_margins_to_bounded_defaults() {
        let output = parse_rtf(
            r"{\rtf1\paperw3000\paperh20000\margl720\margr720\margt720720\margb720720 Body\par}",
        )
        .unwrap();

        assert_eq!(output.document.page.margin_left_twips, 720);
        assert_eq!(output.document.page.margin_right_twips, 720);
        assert_eq!(output.document.page.margin_top_twips, 1_440);
        assert_eq!(output.document.page.margin_bottom_twips, 1_440);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("top margin 720720 twips exceeds")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("bottom margin 720720 twips exceeds")
        }));
    }

    #[test]
    fn tab_stop_count_limit_is_enforced() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_tab_stops: 1,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        assert!(matches!(
            parse_rtf_bytes_with_options(br"{\rtf1\tx720\tx1440 Too many tabs\par}", &options),
            Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "tab stops"
        ));
    }

    #[test]
    fn clamps_object_placeholder_geometry_before_96dpi_hints() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_image_display_twips: 720,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let input = br"{\rtf1{\object\objw999999\objh999999{\objdata 414243}} after\par}".to_vec();
        let output = parse_rtf_bytes_with_options(&input, &options).unwrap();
        let image = output
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Image(image) => Some(image),
                _ => None,
            })
            .expect("expected passive object placeholder");

        assert_eq!(image.format, ImageFormat::Placeholder);
        assert!(image.bytes.is_empty());
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
        assert_eq!(image.width_px, 48);
        assert_eq!(image.height_px, 48);
        assert_eq!(image.natural_width_px_hint, Some(48));
        assert_eq!(image.natural_height_px_hint, Some(48));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("picture display object width clamped")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("picture display object height clamped")
        }));
    }

    #[test]
    fn binary_picture_byte_limit_is_enforced_before_decoding() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_image_bytes: 2,
                max_binary_blob_size: 1024,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        assert!(matches!(
            parse_rtf_bytes_with_options(br"{\rtf1{\pict\pngblip\bin3 abc}}", &options),
            Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "picture bytes"
        ));
    }

    #[test]
    fn image_pixel_limit_is_enforced() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_image_pixels: 10,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let input = format!(
            "{{\\rtf1{{\\pict\\jpegblip {}}}}}",
            bytes_to_hex(&minimal_jpeg_with_dimensions(11, 1))
        );
        assert!(matches!(
            parse_rtf_bytes_with_options(input.as_bytes(), &options),
            Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "image pixels"
        ));
    }
