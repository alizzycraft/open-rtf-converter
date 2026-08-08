    #[test]
    fn hidden_shape_does_not_split_visible_paragraph() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\v hidden {\shp{\shpinst{\sp{\sn shapeType}{\sv 1}}{\sp{\sn pFragments}{\sv hidden-payload}}}}}{\v0 After}\par}"#,
        )
        .unwrap();

        assert_eq!(output.document.blocks.len(), 1);
        assert_eq!(document_text(&output.document), "Before After");
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("hidden shape stripped before safe model normalization")
        }));
    }

    #[test]
    fn normalizes_wingdings3_basic_arrows_to_safe_unicode() {
        let output =
            parse_rtf(r"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Wingdings 3;}}\f1 f g h i\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(
            paragraph.runs[0].text,
            "\u{2190} \u{2192} \u{2191} \u{2193}"
        );
        assert_eq!(paragraph.runs[0].style.font_index, 1);
    }

    #[test]
    fn positioned_floating_table_controls_receive_default_wrap_distance() {
        let output =
            parse_rtf(r"{\rtf1\trowd\tposx720\tposy120\cellx2000 Offset\cell\row}").unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(
            table.rows[0].wrap_margins.left_twips,
            DEFAULT_FLOATING_TABLE_WRAP_MARGIN_TWIPS
        );
        assert_eq!(
            table.rows[0].wrap_margins.right_twips,
            DEFAULT_FLOATING_TABLE_WRAP_MARGIN_TWIPS
        );
        assert_eq!(table.rows[0].wrap_margins.top_twips, 0);
        assert_eq!(table.rows[0].wrap_margins.bottom_twips, 0);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "floating table default wrap distance interpreted as bounded passive row margin",
            )
        }));
    }

    #[test]
    fn normalizes_floating_table_wrap_distances_as_bounded_row_margins() {
        let output = parse_rtf(
            r"{\rtf1\trowd\tdfrmtxtLeft180\tdfrmtxtRight240\tdfrmtxtTop120\tdfrmtxtBottom300\cellx2000 Wrapped\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].wrap_margins.left_twips, 180);
        assert_eq!(table.rows[0].wrap_margins.right_twips, 240);
        assert_eq!(table.rows[0].wrap_margins.top_twips, 120);
        assert_eq!(table.rows[0].wrap_margins.bottom_twips, 300);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("floating table wrap distance interpreted")
        }));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn clamps_extreme_floating_table_wrap_distance_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_table_cell_gap_twips: 240,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\trowd\tdfrmtxtLeft9999\tdfrmtxtTop9999\cellx2000 Wrapped\cell\row}",
            &options,
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].wrap_margins.left_twips, 240);
        assert_eq!(table.rows[0].wrap_margins.top_twips, 240);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("floating table wrap margin clamped")
        }));
    }

    #[test]
    fn metadata_ignored_shape_pictures_do_not_reenter_body_flow() {
        let output = parse_rtf(
            r"{\rtf1 Start{\info{\title Hidden {\*\shppict{\pict\pngblip 00}}{\*\nonshppict{\pict\wmetafile8 41424344}}}} End\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert_eq!(text, "Start End");
        assert!(
            output
                .document
                .blocks
                .iter()
                .all(|block| !matches!(block, Block::Image(_)))
        );
        for forbidden in [
            "Image skipped",
            "unsupported picture",
            "shppict",
            "nonshppict",
            "pict",
            "pngblip",
            "wmetafile",
            "41424344",
        ] {
            assert!(
                !text.contains(forbidden),
                "metadata shape-picture fallback leaked to body text: {forbidden}"
            );
        }
    }

    #[test]
    fn renders_shape_text_as_safe_passive_paragraph() {
        let output =
            parse_rtf(r"{\rtf1 Before\par{\shp{\shpinst{\shptxt Box text\par}}}After\par}")
                .unwrap();
        let paragraph_text = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph) => Some(
                    paragraph
                        .runs
                        .iter()
                        .map(|run| run.text.as_str())
                        .collect::<String>(),
                ),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(paragraph_text, vec!["Before", "Box text", "After"]);
        assert!(output.document.blocks.iter().all(|block| {
            !matches!(
                block,
                Block::Placeholder(text) if text.contains("Shape skipped")
            )
        }));
    }

    #[test]
    fn renders_old_drawing_text_box_as_safe_passive_shape() {
        let output = parse_rtf(
            r"{\rtf1 Before\par{\do\dobx720\doby720\dodhgt1{\dptxbx Legacy box text\par}}After\par}",
        )
        .unwrap();
        let paragraph_text = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph) => Some(
                    paragraph
                        .runs
                        .iter()
                        .map(|run| run.text.as_str())
                        .collect::<String>(),
                ),
                _ => None,
            })
            .collect::<Vec<_>>();
        let shape = output
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .expect("legacy text box shape");
        let shape_text = shape
            .text
            .iter()
            .flat_map(|paragraph| &paragraph.runs)
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(paragraph_text, vec!["Before", "After"]);
        assert_eq!(shape.kind, StaticShapeKind::Rectangle);
        assert_eq!(shape.z_order, 1);
        assert_eq!(shape_text, "Legacy box text");
        assert!(output.document.blocks.iter().all(|block| {
            !matches!(
                block,
                Block::Placeholder(text) if text.contains("Shape skipped")
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
    fn normalizes_legacy_static_drawing_shapes_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1 Before\par{\do\dprect\dobx120\doby240\dpx360\dpy480\dpxsize1440\dpysize720\dplinew30\dplinecor255\dplinecog128\dplinecob0\dpfillfgcr10\dpfillfgcg20\dpfillfgcb30{\sp{\sn pFragments}{\sv hostile-payload}}}After\par}",
        )
        .unwrap();
        let shapes = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .collect::<Vec<_>>();
        let text = document_text(&output.document);

        assert_eq!(shapes.len(), 1);
        assert_eq!(shapes[0].kind, StaticShapeKind::Rectangle);
        assert_eq!(shapes[0].left_twips, 480);
        assert_eq!(shapes[0].top_twips, 720);
        assert_eq!(shapes[0].width_twips, 1440);
        assert_eq!(shapes[0].height_twips, 720);
        assert_eq!(shapes[0].stroke_width_twips, 30);
        assert_eq!(shapes[0].stroke_color.red, 255);
        assert_eq!(shapes[0].stroke_color.green, 128);
        assert_eq!(shapes[0].stroke_color.blue, 0);
        assert_eq!(
            shapes[0].fill_color,
            Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            })
        );
        assert!(text.contains("BeforeAfter"));
        for forbidden in ["dprect", "dobx", "dpfill", "pFragments", "hostile-payload"] {
            assert!(!text.contains(forbidden));
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_header_static_drawing_shapes_as_safe_repeating_metadata() {
        let output = parse_rtf(
            r"{\rtf1{\header Logo {\do\dprect\dpxsize1440\dpysize720\dplinew30\dpfillfgcr10\dpfillfgcg20\dpfillfgcb30}\par}Body\par}",
        )
        .unwrap();

        assert_eq!(output.document.header.len(), 1);
        assert_eq!(output.document.header[0].runs[0].text, "Logo ");
        assert_eq!(output.document.header_shapes.len(), 1);
        assert!(
            output
                .document
                .blocks
                .iter()
                .all(|block| !matches!(block, Block::Shape(_)))
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_header_shape_text_as_safe_repeating_metadata() {
        let output = parse_rtf(
            r"{\rtf1{\header Logo {\shp{\*\shpinst{\sp{\sn pFragments}{\sv hostile-shape-payload}}}{\shptxt Box text\par}}\par}Body\par}",
        )
        .unwrap();

        let body_text = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph) => Some(
                    paragraph
                        .runs
                        .iter()
                        .map(|run| run.text.as_str())
                        .collect::<String>(),
                ),
                _ => None,
            })
            .collect::<String>();

        assert_eq!(output.document.header.len(), 2);
        assert_eq!(output.document.header[0].runs[0].text, "Logo ");
        assert_eq!(output.document.header[1].runs[0].text, "Box text");
        assert_eq!(body_text, "Body");
        for forbidden in [
            "pFragments",
            "hostile-shape-payload",
            "shpinst",
            "shptxt",
            "[Shape skipped",
        ] {
            assert!(
                !body_text.contains(forbidden),
                "forbidden shape text content leaked to body: {forbidden}"
            );
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_zero_width_static_drawing_outline_as_safe_fill_only_shape() {
        let output = parse_rtf(
            r"{\rtf1 Before\par{\do\dprect\dpxsize1440\dpysize720\dplinew0\dpfillfgcr10\dpfillfgcg20\dpfillfgcb30}After\par}",
        )
        .unwrap();
        let shape = output
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .expect("shape");

        assert_eq!(shape.kind, StaticShapeKind::Rectangle);
        assert_eq!(shape.stroke_width_twips, 0);
        assert_eq!(
            shape.fill_color,
            Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            })
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_legacy_static_drawing_ellipses_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1 Before\par{\do\dpellipse\dobx120\doby240\dpx360\dpy480\dpxsize1440\dpysize720\dplinew30\dplinecor255\dplinecog128\dplinecob0\dpfillfgcr10\dpfillfgcg20\dpfillfgcb30{\sp{\sn pFragments}{\sv hostile-payload}}}After\par}",
        )
        .unwrap();
        let shapes = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .collect::<Vec<_>>();
        let text = document_text(&output.document);

        assert_eq!(shapes.len(), 1);
        assert_eq!(shapes[0].kind, StaticShapeKind::Ellipse);
        assert_eq!(shapes[0].left_twips, 480);
        assert_eq!(shapes[0].top_twips, 720);
        assert_eq!(shapes[0].width_twips, 1440);
        assert_eq!(shapes[0].height_twips, 720);
        assert_eq!(shapes[0].stroke_width_twips, 30);
        assert_eq!(shapes[0].stroke_color.red, 255);
        assert_eq!(shapes[0].stroke_color.green, 128);
        assert_eq!(shapes[0].stroke_color.blue, 0);
        assert_eq!(
            shapes[0].fill_color,
            Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            })
        );
        assert!(text.contains("BeforeAfter"));
        for forbidden in [
            "dpellipse",
            "dobx",
            "dpfill",
            "pFragments",
            "hostile-payload",
        ] {
            assert!(!text.contains(forbidden));
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_legacy_static_drawing_rounded_rectangles_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1 Before\par{\do\dprect\dproundr\dobx120\doby240\dpx360\dpy480\dpxsize1440\dpysize720\dplinew30\dplinecor255\dplinecog128\dplinecob0\dpfillfgcr10\dpfillfgcg20\dpfillfgcb30{\sp{\sn pFragments}{\sv hostile-payload}}}After\par}",
        )
        .unwrap();
        let shapes = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .collect::<Vec<_>>();
        let text = document_text(&output.document);

        assert_eq!(shapes.len(), 1);
        assert_eq!(shapes[0].kind, StaticShapeKind::RoundedRectangle);
        assert_eq!(shapes[0].left_twips, 480);
        assert_eq!(shapes[0].top_twips, 720);
        assert_eq!(shapes[0].width_twips, 1440);
        assert_eq!(shapes[0].height_twips, 720);
        assert_eq!(shapes[0].stroke_width_twips, 30);
        assert_eq!(shapes[0].stroke_color.red, 255);
        assert_eq!(shapes[0].stroke_color.green, 128);
        assert_eq!(shapes[0].stroke_color.blue, 0);
        assert_eq!(
            shapes[0].fill_color,
            Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            })
        );
        assert!(text.contains("BeforeAfter"));
        for forbidden in [
            "dprect",
            "dproundr",
            "dobx",
            "dpfill",
            "pFragments",
            "hostile-payload",
        ] {
            assert!(!text.contains(forbidden));
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_legacy_static_drawing_line_styles_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1 Before\par{\do\dpline\dplinedot\dpx360\dpy480\dpxsize1440\dpysize720\dplinew30{\sp{\sn pFragments}{\sv hostile-payload}}}After\par}",
        )
        .unwrap();
        let shape = output
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .expect("shape");
        let text = document_text(&output.document);

        assert_eq!(shape.kind, StaticShapeKind::Line);
        assert_eq!(shape.stroke_style, BorderStyle::Dotted);
        assert!(text.contains("BeforeAfter"));
        for forbidden in ["dpline", "dplinedot", "pFragments", "hostile-payload"] {
            assert!(!text.contains(forbidden));
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn strict_spec_normalizes_legacy_shape_flip_properties_as_safe_metadata() {
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1 Before\par{\do\dpline\dpx360\dpy480\dpxsize1440\dpysize720{\sp{\sn fFlipH}{\sv 1}}{\sp{\sn fFlipV}{\sv 1}}{\sp{\sn pFragments}{\sv hostile-payload}}}After\par}",
            &RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
        )
        .unwrap();
        let shape = output
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .expect("shape");
        let text = document_text(&output.document);

        assert_eq!(shape.kind, StaticShapeKind::Line);
        assert!(shape.flip_horizontal);
        assert!(shape.flip_vertical);
        assert!(text.contains("BeforeAfter"));
        for forbidden in ["fFlipH", "fFlipV", "pFragments", "hostile-payload"] {
            assert!(!text.contains(forbidden));
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn strict_spec_normalizes_legacy_shape_line_dashing_property_as_safe_stroke_style() {
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1 Before\par{\do\dpline\dpx360\dpy480\dpxsize1440\dpysize720{\sp{\sn lineDashing}{\sv dashDot}}{\sp{\sn pFragments}{\sv hostile-payload}}}After\par}",
            &RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
        )
        .unwrap();
        let shape = output
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .expect("shape");
        let text = document_text(&output.document);

        assert_eq!(shape.kind, StaticShapeKind::Line);
        assert_eq!(shape.stroke_style, BorderStyle::Dashed);
        assert!(text.contains("BeforeAfter"));
        for forbidden in ["lineDashing", "dashDot", "pFragments", "hostile-payload"] {
            assert!(!text.contains(forbidden));
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn strict_spec_normalizes_legacy_shape_arrowhead_properties_as_safe_metadata() {
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1 Before\par{\do\dpline\dpx360\dpy480\dpxsize1440\dpysize720{\sp{\sn lineStartArrowhead}{\sv open}}{\sp{\sn lineEndArrowhead}{\sv triangle}}{\sp{\sn pFragments}{\sv hostile-payload}}}After\par}",
            &RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
        )
        .unwrap();
        let shape = output
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .expect("shape");
        let text = document_text(&output.document);

        assert_eq!(shape.kind, StaticShapeKind::Line);
        assert_eq!(shape.start_arrowhead, StaticShapeArrowhead::Open);
        assert_eq!(shape.end_arrowhead, StaticShapeArrowhead::Triangle);
        assert!(text.contains("BeforeAfter"));
        for forbidden in [
            "lineStartArrowhead",
            "lineEndArrowhead",
            "open",
            "triangle",
            "pFragments",
            "hostile-payload",
        ] {
            assert!(!text.contains(forbidden));
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn word_compatible_legacy_drawing_ignores_office_shape_properties() {
        let output = parse_rtf(
            r"{\rtf1{\do\dpline\dpx360\dpy480\dpxsize1440\dpysize720{\sp{\sn lineDashing}{\sv dashDot}}{\sp{\sn lineEndCap}{\sv round}}{\sp{\sn lineStartArrowhead}{\sv open}}{\sp{\sn lineEndArrowhead}{\sv triangle}}{\sp{\sn lineStartArrowWidth}{\sv wide}}{\sp{\sn lineStartArrowLength}{\sv long}}{\sp{\sn lineEndArrowWidth}{\sv narrow}}{\sp{\sn lineEndArrowLength}{\sv short}}}}",
        )
        .unwrap();
        let shape = output
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .expect("shape");

        assert_eq!(shape.stroke_width_twips, 3);
        assert_eq!(shape.stroke_style, BorderStyle::Single);
        assert_eq!(shape.stroke_cap, StaticShapeLineCap::Flat);
        assert_eq!(shape.start_arrowhead, StaticShapeArrowhead::None);
        assert_eq!(shape.end_arrowhead, StaticShapeArrowhead::None);
        assert_eq!(shape.start_arrowhead_width_percent, 100);
        assert_eq!(shape.start_arrowhead_length_percent, 100);
        assert_eq!(shape.end_arrowhead_width_percent, 100);
        assert_eq!(shape.end_arrowhead_length_percent, 100);
    }

    #[test]
    fn normalizes_legacy_static_drawing_polylines_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1 Before\par{\do\dppolyline\dppolycount3\dplinedash\dplinew30\dpptx360\dppty480\dpptx1080\dppty1200\dpptx1800\dppty480{\sp{\sn pFragments}{\sv hostile-payload}}}After\par}",
        )
        .unwrap();
        let shape = output
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .expect("polyline");
        let text = document_text(&output.document);

        assert_eq!(shape.kind, StaticShapeKind::Polyline);
        assert_eq!(shape.left_twips, 360);
        assert_eq!(shape.top_twips, 480);
        assert_eq!(shape.width_twips, 1440);
        assert_eq!(shape.height_twips, 720);
        assert_eq!(shape.stroke_style, BorderStyle::Dashed);
        assert_eq!(shape.points.len(), 3);
        assert_eq!(shape.points[0], StaticShapePoint::default());
        assert_eq!(
            shape.points[1],
            StaticShapePoint {
                x_twips: 720,
                y_twips: 720,
            }
        );
        assert_eq!(
            shape.points[2],
            StaticShapePoint {
                x_twips: 1440,
                y_twips: 0,
            }
        );
        assert!(text.contains("BeforeAfter"));
        for forbidden in [
            "dppolyline",
            "dpptx",
            "dppty",
            "dplinedash",
            "pFragments",
            "hostile-payload",
        ] {
            assert!(!text.contains(forbidden));
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_legacy_static_drawing_polygons_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1 Before\par{\do\dppolygon\dppolycount3\dplinedot\dplinew30\dpfillfgcr10\dpfillfgcg20\dpfillfgcb30\dpptx360\dppty480\dpptx1080\dppty1200\dpptx1800\dppty480{\sp{\sn pFragments}{\sv hostile-payload}}}After\par}",
        )
        .unwrap();
        let shape = output
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .expect("polygon");
        let text = document_text(&output.document);

        assert_eq!(shape.kind, StaticShapeKind::Polygon);
        assert_eq!(shape.left_twips, 360);
        assert_eq!(shape.top_twips, 480);
        assert_eq!(shape.width_twips, 1440);
        assert_eq!(shape.height_twips, 720);
        assert_eq!(shape.stroke_style, BorderStyle::Dotted);
        assert_eq!(
            shape.fill_color,
            Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            })
        );
        assert_eq!(shape.points.len(), 3);
        assert_eq!(shape.points[0], StaticShapePoint::default());
        assert!(text.contains("BeforeAfter"));
        for forbidden in [
            "dppolygon",
            "dpptx",
            "dppty",
            "dplinedot",
            "dpfillfg",
            "pFragments",
            "hostile-payload",
        ] {
            assert!(!text.contains(forbidden));
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn bounds_legacy_static_drawing_polyline_points() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_shape_points: 2,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        assert!(matches!(
            parse_rtf_bytes_with_options(
                br"{\rtf1{\do\dppolyline\dpptx0\dppty0\dpptx10\dppty10\dpptx20\dppty0}}",
                &options,
            ),
            Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "shape points"
        ));
    }

    #[test]
    fn unsupported_shape_properties_become_placeholder_without_payload_leakage() {
        let output = parse_rtf(
            r"{\rtf1{\shp{\shpinst{\sp{\sn pFragments}{\sv hostile-payload}}}}After\par}",
        )
        .unwrap();
        let text = output
            .document
            .blocks
            .iter()
            .flat_map(|block| match block {
                Block::Paragraph(paragraph) => paragraph
                    .runs
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<Vec<_>>(),
                Block::Placeholder(text) => vec![text.as_str()],
                _ => Vec::new(),
            })
            .collect::<String>();

        assert!(text.contains("[Shape skipped: unsupported shape]"));
        assert!(text.contains("After"));
        assert!(!text.contains("pFragments"));
        assert!(!text.contains("hostile-payload"));
    }

    #[test]
    fn renders_old_drawing_text_box_text_destination_as_safe_passive_shape() {
        let output = parse_rtf(
            r"{\rtf1 Before\par{\do\dobx720\doby720\dodhgt1{\dptxbx{\dptxbxtext Legacy box text\par}}}After\par}",
        )
        .unwrap();
        let paragraph_text = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph) => Some(
                    paragraph
                        .runs
                        .iter()
                        .map(|run| run.text.as_str())
                        .collect::<String>(),
                ),
                _ => None,
            })
            .collect::<Vec<_>>();
        let shape = output
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .expect("legacy text box shape");
        let shape_text = shape
            .text
            .iter()
            .flat_map(|paragraph| &paragraph.runs)
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(paragraph_text, vec!["Before", "After"]);
        assert_eq!(shape.kind, StaticShapeKind::Rectangle);
        assert_eq!(shape.z_order, 1);
        assert_eq!(shape_text, "Legacy box text");
        assert!(output.document.blocks.iter().all(|block| {
            !matches!(
                block,
                Block::Placeholder(text) if text.contains("Shape skipped")
            )
        }));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("dptxbxtext"))
        );
    }

    #[test]
    fn normalizes_no_wrap_controls_for_paragraphs_and_table_cells() {
        let output = parse_rtf(
            r"{\rtf1\nowrap Alias no wrap paragraph\par\nowrap0 Alias wrapped paragraph\par\nowwrap No wrap paragraph\par\nowwrap0 Wrapped paragraph\par\trowd\clNoWrap\cellx1440 Cell no wrap\cell\cellx2880 Cell wrapped\cell\row}",
        )
        .unwrap();

        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected first paragraph, got {other:?}"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected second paragraph, got {other:?}"),
        };
        let third = match &output.document.blocks[2] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected third paragraph, got {other:?}"),
        };
        let fourth = match &output.document.blocks[3] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected fourth paragraph, got {other:?}"),
        };
        let table = match &output.document.blocks[4] {
            Block::Table(table) => table,
            other => panic!("expected table, got {other:?}"),
        };

        assert!(!first.style.no_wrap);
        assert!(!second.style.no_wrap);
        assert!(third.style.no_wrap);
        assert!(fourth.style.no_wrap);
        assert_eq!(third.style.no_wrap_emergency_right_inset_twips, 144);
        assert_eq!(fourth.style.no_wrap_emergency_right_inset_twips, 144);
        assert!(table.rows[0].cells[0].paragraphs[0].style.no_wrap);
        assert!(!table.rows[0].cells[1].paragraphs[0].style.no_wrap);
        assert_eq!(
            table.rows[0].cells[0].paragraphs[0]
                .style
                .no_wrap_emergency_right_inset_twips,
            0
        );
        assert_eq!(
            table.rows[0].cells[0].paragraphs[0].style.left_indent_twips,
            0
        );
        assert_eq!(
            table.rows[0].cells[1].paragraphs[0].style.left_indent_twips,
            108
        );
        assert_eq!(
            table.rows[0].cells[1].paragraphs[0]
                .style
                .first_line_indent_twips,
            -108
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn strict_spec_mode_accepts_no_wrap_alias() {
        let options = RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\nowrap Alias no wrap\par\nowrap0 Alias wrapped\par}",
            &options,
        )
        .unwrap();

        let Block::Paragraph(first) = &output.document.blocks[0] else {
            panic!("expected first paragraph");
        };
        let Block::Paragraph(second) = &output.document.blocks[1] else {
            panic!("expected second paragraph");
        };
        assert!(first.style.no_wrap);
        assert!(!second.style.no_wrap);
        assert_eq!(first.style.no_wrap_emergency_right_inset_twips, 0);
        assert_eq!(second.style.no_wrap_emergency_right_inset_twips, 0);
    }

    #[test]
    fn normalizes_ignorable_shape_picture_and_suppresses_nonshape_fallback() {
        let image_hex = bytes_to_hex(&minimal_rgb_png_with_dimensions(1, 1));
        let input = format!(
            "{{\\rtf1{{\\*\\shppict{{\\pict\\pngblip\\picwgoal720 {image_hex}}}}}{{\\nonshppict{{\\pict\\pngblip\\picwgoal1440 {image_hex}}}}}}}"
        );
        let output = parse_rtf(&input).unwrap();
        let images = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Image(image) => Some(image),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(images.len(), 1);
        assert_eq!(images[0].display_width_twips, Some(720));
        assert!(!document_text(&output.document).contains("nonshppict"));
        assert!(!document_text(&output.document).contains("shppict"));
    }

    #[test]
    fn nested_shape_picture_counts_as_safe_shape_result() {
        let image_hex = bytes_to_hex(&minimal_rgb_png_with_dimensions(1, 1));
        let input = format!(
            "{{\\rtf1 Before{{\\shp{{\\*\\shpinst{{\\sp{{\\sn pFragments}}{{\\sv hidden-payload}}}}}}{{\\*\\shppict{{\\pict\\pngblip\\picwgoal720 {image_hex}}}}}}}After\\par}}"
        );
        let output = parse_rtf(&input).unwrap();
        let images = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Image(image) => Some(image),
                _ => None,
            })
            .collect::<Vec<_>>();
        let text = document_text(&output.document);

        assert_eq!(images.len(), 1);
        assert_eq!(images[0].display_width_twips, Some(720));
        assert!(text.contains("BeforeAfter"));
        assert!(!text.contains("[Shape skipped"));
        assert!(!text.contains("hidden-payload"));
    }

    #[test]
    fn incomplete_bezier_records_are_passive_noops_before_later_drawing() {
        let records = [
            emf_poly_record(2, &[(10, 10), (30, 5), (50, 35), (70, 30), (80, 40)]),
            emf_point_record(27, 10, 10),
            emf_poly_record(5, &[(30, 5), (50, 35)]),
            emf_poly16_record(85, &[(20, 50), (40, 30), (60, 70), (80, 60), (90, 65)]),
            emf_point_record(27, 20, 20),
            emf_poly16_record(88, &[(40, 30), (60, 70)]),
            emf_rect_record(43, 0, 0, 60, 30),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive EMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn incomplete_polydraw_bezier_runs_are_passive_noops_before_later_drawing() {
        let records = [
            emf_polydraw_record(56, &[(10, 10), (30, 20), (40, 30)], &[0x06, 0x04, 0x04]),
            emf_polydraw16_record(92, &[(20, 20), (60, 20)], &[0x06, 0x04]),
            emf_rect_record(43, 0, 0, 60, 30),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive EMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }
