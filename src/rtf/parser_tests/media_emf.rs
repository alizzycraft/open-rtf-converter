    #[test]
    fn emf_header_dimensions_become_passive_placeholder_geometry() {
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_bounds_and_frame(160, 80, 2540, 1270))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive EMF placeholder"),
        };
        assert_eq!(image.format, ImageFormat::Placeholder);
        assert!(image.bytes.is_empty());
        assert!(image.palette.is_empty());
        assert_eq!(image.width_px, 160);
        assert_eq!(image.height_px, 80);
        assert_eq!(image.natural_width_px_hint, Some(160));
        assert_eq!(image.natural_height_px_hint, Some(80));
        assert_eq!(image.display_width_twips, Some(1440));
        assert_eq!(image.display_height_twips, Some(720));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("EMF picture records stripped; header dimensions used")
        }));
    }

    #[test]
    fn emf_rectangle_and_ellipse_records_become_passive_vector_commands() {
        let records = [
            emf_rect_record(43, 10, 10, 70, 40),
            emf_rect_record(42, 80, 20, 150, 70),
            emf_unknown_record(999),
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
        assert!(image.palette.is_empty());
        assert_eq!(image.width_px, 160);
        assert_eq!(image.height_px, 80);
        assert_eq!(image.natural_width_px_hint, Some(160));
        assert_eq!(image.natural_height_px_hint, Some(80));
        assert_eq!(image.display_width_twips, Some(1440));
        assert_eq!(image.display_height_twips, Some(720));
        assert_eq!(
            image.vector_commands,
            vec![
                StaticImageVectorCommand::Rectangle {
                    left: 10.0,
                    top: 10.0,
                    right: 70.0,
                    bottom: 40.0,
                    stroke_color: Some(Color::default()),
                    stroke_width: 1.0,
                    stroke_style: BorderStyle::Single,
                    fill_pattern: ShadingPattern::None,
                    fill_color: None,
                },
                StaticImageVectorCommand::Ellipse {
                    left: 80.0,
                    top: 20.0,
                    right: 150.0,
                    bottom: 70.0,
                    stroke_color: Some(Color::default()),
                    stroke_width: 1.0,
                    stroke_style: BorderStyle::Single,
                    fill_pattern: ShadingPattern::None,
                    fill_color: None,
                },
            ]
        );
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("EMF picture rendered as bounded passive vector preview with 1 unsupported record(s) skipped")
        }));
    }

    #[test]
    fn emf_roundrect_records_become_passive_vector_commands() {
        let records = [emf_roundrect_record(10, 10, 70, 40, 90, 80)];
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
        assert_eq!(
            image.vector_commands,
            vec![StaticImageVectorCommand::RoundedRectangle {
                left: 10.0,
                top: 10.0,
                right: 70.0,
                bottom: 40.0,
                corner_width: 60.0,
                corner_height: 30.0,
                stroke_color: Some(Color::default()),
                stroke_width: 1.0,
                stroke_style: BorderStyle::Single,
                fill_pattern: ShadingPattern::None,
                fill_color: None,
            }]
        );
    }

    #[test]
    fn emf_pen_and_brush_objects_style_passive_vector_commands() {
        let records = [
            emf_create_pen_record(
                2,
                1,
                3,
                Color {
                    red: 20,
                    green: 40,
                    blue: 60,
                },
            ),
            emf_select_object_record(2),
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 220,
                    green: 180,
                    blue: 120,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_rect_record(43, 10, 10, 70, 40),
            emf_point_record(27, 10, 50),
            emf_point_record(54, 70, 70),
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
        assert_eq!(
            image.vector_commands,
            vec![
                StaticImageVectorCommand::Rectangle {
                    left: 10.0,
                    top: 10.0,
                    right: 70.0,
                    bottom: 40.0,
                    stroke_color: Some(Color {
                        red: 20,
                        green: 40,
                        blue: 60
                    }),
                    stroke_width: 3.0,
                    stroke_style: BorderStyle::Dashed,
                    fill_pattern: ShadingPattern::None,
                    fill_color: Some(Color {
                        red: 220,
                        green: 180,
                        blue: 120
                    }),
                },
                StaticImageVectorCommand::Line {
                    x1: 10.0,
                    y1: 50.0,
                    x2: 70.0,
                    y2: 70.0,
                    stroke_color: Some(Color {
                        red: 20,
                        green: 40,
                        blue: 60
                    }),
                    stroke_width: 3.0,
                    stroke_style: BorderStyle::Dashed,
                },
            ]
        );
    }

    #[test]
    fn emf_extcreatepen_objects_style_passive_vector_commands() {
        let records = [
            emf_extcreatepen_record(
                4,
                1,
                5,
                0,
                Color {
                    red: 90,
                    green: 30,
                    blue: 210,
                },
                b"ignored-dib-pen-payload",
            ),
            emf_select_object_record(4),
            emf_point_record(27, 10, 20),
            emf_point_record(54, 80, 60),
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
        assert_eq!(
            image.vector_commands,
            vec![StaticImageVectorCommand::Line {
                x1: 10.0,
                y1: 20.0,
                x2: 80.0,
                y2: 60.0,
                stroke_color: Some(Color {
                    red: 90,
                    green: 30,
                    blue: 210,
                }),
                stroke_width: 5.0,
                stroke_style: BorderStyle::Dashed,
            }]
        );
    }

    #[test]
    fn emf_extcreatepen_with_irrelevant_invalid_optional_payload_renders() {
        let records = [
            emf_malformed_extcreatepen_record(4, 999, 4),
            emf_select_object_record(4),
            emf_point_record(27, 10, 20),
            emf_point_record(54, 80, 60),
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
            StaticImageVectorCommand::Line {
                stroke_color: Some(Color {
                    red: 0,
                    green: 0,
                    blue: 0
                }),
                stroke_width: 1.0,
                stroke_style: BorderStyle::Single,
                ..
            }
        ));
    }

    #[test]
    fn emf_stroked_path_records_become_passive_vector_commands() {
        let records = [
            emf_create_pen_record(
                2,
                1,
                4,
                Color {
                    red: 120,
                    green: 40,
                    blue: 10,
                },
            ),
            emf_select_object_record(2),
            emf_unknown_record(59),
            emf_point_record(27, 10, 10),
            emf_point_record(54, 50, 10),
            emf_poly_record(6, &[(50, 40), (10, 40)]),
            emf_unknown_record(61),
            emf_point_record(27, 70, 20),
            emf_poly_record(5, &[(80, 5), (100, 35), (110, 20)]),
            emf_unknown_record(60),
            emf_unknown_record(64),
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
            &image.vector_commands[0],
            StaticImageVectorCommand::Path {
                start: (10.0, 10.0),
                segments,
                stroke_color: Some(Color { red: 120, green: 40, blue: 10 }),
                stroke_width: 4.0,
                stroke_style: BorderStyle::Dashed,
                fill_color: None,
                ..
            } if matches!(
                &segments[..],
                [
                    StaticImageVectorPathSegment::LineTo(50.0, 10.0),
                    StaticImageVectorPathSegment::LineTo(50.0, 40.0),
                    StaticImageVectorPathSegment::LineTo(10.0, 40.0),
                    StaticImageVectorPathSegment::LineTo(10.0, 10.0),
                    StaticImageVectorPathSegment::MoveTo(70.0, 20.0),
                    StaticImageVectorPathSegment::CubicTo {
                        control1: (80.0, 5.0),
                        control2: (100.0, 35.0),
                        end: (110.0, 20.0),
                    },
                ]
            )
        ));
    }

    #[test]
    fn emf_unstroked_open_path_becomes_blank_passive_vector() {
        let records = [
            emf_unknown_record(59),
            emf_point_record(27, 10, 10),
            emf_point_record(54, 50, 10),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Image(image)
                if image.format == ImageFormat::WmfVector
                    && image.bytes.is_empty()
            && image.vector_commands.is_empty()
        ));
    }

    #[test]
    fn emf_dangling_unpainted_path_at_eof_preserves_prior_drawing() {
        let records = [
            emf_rect_record(43, 0, 0, 60, 30),
            emf_unknown_record(59),
            emf_point_record(27, 10, 10),
            emf_point_record(54, 50, 10),
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
    fn emf_abortpath_discards_unpainted_path_before_later_drawing() {
        let records = [
            emf_unknown_record(59),
            emf_point_record(27, 10, 10),
            emf_point_record(54, 50, 10),
            emf_unknown_record(68),
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
    fn emf_empty_ended_path_paint_records_are_passive_noops() {
        let mut stroke = emf_unknown_record(64);
        stroke.extend_from_slice(b"EMPTY-EMF-STROKEPATH /JavaScript");
        stroke.resize(stroke.len().next_multiple_of(4), 0);
        let stroke_len = stroke.len() as u32;
        write_test_le_u32(&mut stroke, 4, stroke_len);

        let mut fill = emf_unknown_record(62);
        fill.extend_from_slice(b"EMPTY-EMF-FILLPATH /EmbeddedFile");
        fill.resize(fill.len().next_multiple_of(4), 0);
        let fill_len = fill.len() as u32;
        write_test_le_u32(&mut fill, 4, fill_len);

        let mut stroke_and_fill = emf_unknown_record(63);
        stroke_and_fill.extend_from_slice(b"EMPTY-EMF-STROKEANDFILLPATH /Launch");
        stroke_and_fill.resize(stroke_and_fill.len().next_multiple_of(4), 0);
        let stroke_and_fill_len = stroke_and_fill.len() as u32;
        write_test_le_u32(&mut stroke_and_fill, 4, stroke_and_fill_len);

        let records = [
            emf_unknown_record(59),
            emf_point_record(27, 20, 20),
            emf_point_record(54, 20, 20),
            emf_unknown_record(60),
            stroke,
            emf_unknown_record(59),
            emf_point_record(27, 30, 30),
            emf_point_record(54, 30, 30),
            emf_unknown_record(60),
            fill,
            emf_unknown_record(59),
            emf_point_record(27, 40, 40),
            emf_point_record(54, 40, 40),
            emf_unknown_record(60),
            stroke_and_fill,
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 0.0,
                top: 0.0,
                right: 60.0,
                bottom: 30.0,
                ..
            }
        ));
    }

    #[test]
    fn emf_null_object_path_paint_records_are_passive_noops() {
        let records = [
            emf_select_object_record(0x8000_0008),
            emf_unknown_record(59),
            emf_point_record(27, 10, 10),
            emf_point_record(54, 80, 10),
            emf_unknown_record(60),
            emf_unknown_record(64),
            emf_select_object_record(0x8000_0005),
            emf_unknown_record(59),
            emf_point_record(27, 20, 20),
            emf_poly_record(6, &[(80, 20), (80, 50), (20, 50)]),
            emf_unknown_record(61),
            emf_unknown_record(60),
            emf_unknown_record(62),
            emf_select_object_record(0x8000_0008),
            emf_select_object_record(0x8000_0005),
            emf_unknown_record(59),
            emf_point_record(27, 30, 30),
            emf_poly_record(6, &[(90, 30), (90, 60), (30, 60)]),
            emf_unknown_record(61),
            emf_unknown_record(60),
            emf_unknown_record(63),
            emf_select_object_record(0x8000_0007),
            emf_select_object_record(0x8000_0004),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 0.0,
                top: 0.0,
                right: 60.0,
                bottom: 30.0,
                stroke_color: Some(Color {
                    red: 0,
                    green: 0,
                    blue: 0
                }),
                fill_color: Some(Color {
                    red: 0,
                    green: 0,
                    blue: 0
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_fillpath_records_become_passive_polygon_commands() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 20,
                    green: 160,
                    blue: 90,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_unknown_record(59),
            emf_point_record(27, 10, 10),
            emf_poly_record(6, &[(80, 10), (80, 50), (10, 50)]),
            emf_unknown_record(61),
            emf_unknown_record(60),
            emf_unknown_record(62),
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
            &image.vector_commands[0],
            StaticImageVectorCommand::Polygon {
                points,
                stroke_color: None,
                stroke_width: 0.0,
                fill_color: Some(Color { red: 20, green: 160, blue: 90 }),
                fill_rule: StaticImageVectorFillRule::Alternate,
                ..
            } if points == &vec![(10.0, 10.0), (80.0, 10.0), (80.0, 50.0), (10.0, 50.0), (10.0, 10.0)]
        ));
    }

    #[test]
    fn emf_strokeandfillpath_records_become_passive_polygon_commands() {
        let records = [
            emf_create_pen_record(
                2,
                0,
                3,
                Color {
                    red: 180,
                    green: 30,
                    blue: 40,
                },
            ),
            emf_select_object_record(2),
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 230,
                    green: 210,
                    blue: 80,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_unknown_record(59),
            emf_point_record(27, 20, 20),
            emf_poly_record(6, &[(70, 20), (45, 60)]),
            emf_unknown_record(61),
            emf_unknown_record(60),
            emf_unknown_record(63),
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
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::Polygon {
                points,
                stroke_color: Some(Color { red: 180, green: 30, blue: 40 }),
                stroke_width: 3.0,
                fill_color: Some(Color { red: 230, green: 210, blue: 80 }),
                ..
            } if points == &vec![(20.0, 20.0), (70.0, 20.0), (45.0, 60.0), (20.0, 20.0)]
        ));
    }

    #[test]
    fn emf_intersectcliprect_records_become_passive_clip_commands() {
        let records = [
            emf_rect_record(30, 20, 20, 100, 60),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipRect {
                left: 20.0,
                top: 20.0,
                right: 100.0,
                bottom: 60.0,
            }
        );
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle {
                left: 0.0,
                top: 0.0,
                right: 160.0,
                bottom: 80.0,
                ..
            }
        ));
    }

    #[test]
    fn emf_offsetcliprgn_offsets_unpainted_clip_rect() {
        let records = [
            emf_rect_record(30, 20, 20, 100, 60),
            emf_point_record(26, 10, 5),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipRect {
                left: 30.0,
                top: 25.0,
                right: 110.0,
                bottom: 65.0,
            }
        );
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_zero_offsetcliprgn_is_passive_noop_without_clip_state() {
        let mut offset = emf_point_record(26, 0, 0);
        offset.extend_from_slice(b"ZERO-OFFSET-EMF-OFFSETCLIPRGN /JavaScript");
        offset.resize(offset.len().next_multiple_of(4), 0);
        let offset_len = offset.len() as u32;
        write_test_le_u32(&mut offset, 4, offset_len);
        let records = [
            emf_rect_record(43, 0, 0, 30, 30),
            offset,
            emf_rect_record(43, 40, 10, 80, 40),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 0.0,
                top: 0.0,
                right: 30.0,
                bottom: 30.0,
                ..
            }
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle {
                left: 40.0,
                top: 10.0,
                right: 80.0,
                bottom: 40.0,
                ..
            }
        ));
    }

    #[test]
    fn emf_offsetcliprgn_after_painted_clip_offsets_unscoped_clip() {
        let records = [
            emf_rect_record(30, 20, 20, 100, 60),
            emf_rect_record(43, 0, 0, 30, 30),
            emf_point_record(26, 10, 5),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 7);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::SaveState
        ));
        assert_eq!(
            image.vector_commands[1],
            StaticImageVectorCommand::ClipRect {
                left: 20.0,
                top: 20.0,
                right: 100.0,
                bottom: 60.0,
            }
        );
        assert!(matches!(
            image.vector_commands[2],
            StaticImageVectorCommand::Rectangle { .. }
        ));
        assert!(matches!(
            image.vector_commands[3],
            StaticImageVectorCommand::RestoreState
        ));
        assert!(matches!(
            image.vector_commands[4],
            StaticImageVectorCommand::SaveState
        ));
        assert_eq!(
            image.vector_commands[5],
            StaticImageVectorCommand::ClipRect {
                left: 30.0,
                top: 25.0,
                right: 110.0,
                bottom: 65.0,
            }
        );
        assert!(matches!(
            image.vector_commands[6],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_offsetcliprgn_after_painted_unscoped_clip_mutation_offsets_combined_clip() {
        let records = [
            emf_rect_record(30, 20, 20, 100, 60),
            emf_rect_record(43, 0, 0, 30, 30),
            emf_rect_record(30, 40, 30, 120, 70),
            emf_point_record(26, 10, 5),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 9);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::SaveState
        ));
        assert_eq!(
            image.vector_commands[1],
            StaticImageVectorCommand::ClipRect {
                left: 20.0,
                top: 20.0,
                right: 100.0,
                bottom: 60.0,
            }
        );
        assert!(matches!(
            image.vector_commands[2],
            StaticImageVectorCommand::Rectangle { .. }
        ));
        assert_eq!(
            image.vector_commands[3],
            StaticImageVectorCommand::ClipRect {
                left: 40.0,
                top: 30.0,
                right: 120.0,
                bottom: 70.0,
            }
        );
        assert!(matches!(
            image.vector_commands[4],
            StaticImageVectorCommand::RestoreState
        ));
        assert!(matches!(
            image.vector_commands[5],
            StaticImageVectorCommand::SaveState
        ));
        assert_eq!(
            image.vector_commands[6],
            StaticImageVectorCommand::ClipRect {
                left: 30.0,
                top: 25.0,
                right: 110.0,
                bottom: 65.0,
            }
        );
        assert_eq!(
            image.vector_commands[7],
            StaticImageVectorCommand::ClipRect {
                left: 50.0,
                top: 35.0,
                right: 130.0,
                bottom: 75.0,
            }
        );
        assert!(matches!(
            image.vector_commands[8],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_offsetcliprgn_after_painted_saved_clip_offsets_scoped_clip() {
        let records = [
            emf_unknown_record(33),
            emf_rect_record(30, 20, 20, 100, 60),
            emf_rect_record(43, 0, 0, 30, 30),
            emf_point_record(26, 10, 5),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 8);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::SaveState
        ));
        assert_eq!(
            image.vector_commands[1],
            StaticImageVectorCommand::ClipRect {
                left: 20.0,
                top: 20.0,
                right: 100.0,
                bottom: 60.0,
            }
        );
        assert!(matches!(
            image.vector_commands[2],
            StaticImageVectorCommand::Rectangle { .. }
        ));
        assert!(matches!(
            image.vector_commands[3],
            StaticImageVectorCommand::RestoreState
        ));
        assert!(matches!(
            image.vector_commands[4],
            StaticImageVectorCommand::SaveState
        ));
        assert_eq!(
            image.vector_commands[5],
            StaticImageVectorCommand::ClipRect {
                left: 30.0,
                top: 25.0,
                right: 110.0,
                bottom: 65.0,
            }
        );
        assert!(matches!(
            image.vector_commands[6],
            StaticImageVectorCommand::Rectangle { .. }
        ));
        assert!(matches!(
            image.vector_commands[7],
            StaticImageVectorCommand::RestoreState
        ));
    }

    #[test]
    fn emf_offsetcliprgn_after_painted_saved_clip_mutation_offsets_combined_clip() {
        let records = [
            emf_unknown_record(33),
            emf_rect_record(30, 20, 20, 100, 60),
            emf_rect_record(43, 0, 0, 30, 30),
            emf_rect_record(30, 40, 30, 120, 70),
            emf_point_record(26, 10, 5),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 10);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::SaveState
        ));
        assert_eq!(
            image.vector_commands[1],
            StaticImageVectorCommand::ClipRect {
                left: 20.0,
                top: 20.0,
                right: 100.0,
                bottom: 60.0,
            }
        );
        assert!(matches!(
            image.vector_commands[2],
            StaticImageVectorCommand::Rectangle { .. }
        ));
        assert_eq!(
            image.vector_commands[3],
            StaticImageVectorCommand::ClipRect {
                left: 40.0,
                top: 30.0,
                right: 120.0,
                bottom: 70.0,
            }
        );
        assert!(matches!(
            image.vector_commands[4],
            StaticImageVectorCommand::RestoreState
        ));
        assert!(matches!(
            image.vector_commands[5],
            StaticImageVectorCommand::SaveState
        ));
        assert_eq!(
            image.vector_commands[6],
            StaticImageVectorCommand::ClipRect {
                left: 30.0,
                top: 25.0,
                right: 110.0,
                bottom: 65.0,
            }
        );
        assert_eq!(
            image.vector_commands[7],
            StaticImageVectorCommand::ClipRect {
                left: 50.0,
                top: 35.0,
                right: 130.0,
                bottom: 75.0,
            }
        );
        assert!(matches!(
            image.vector_commands[8],
            StaticImageVectorCommand::Rectangle { .. }
        ));
        assert!(matches!(
            image.vector_commands[9],
            StaticImageVectorCommand::RestoreState
        ));
    }

    #[test]
    fn emf_empty_intersectcliprect_becomes_passive_empty_clip() {
        let records = [
            emf_rect_record(30, 20, 20, 20, 60),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipRect {
                left: 20.0,
                top: 20.0,
                right: 20.0,
                bottom: 60.0
            }
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_extselectcliprgn_records_become_passive_clip_path_commands() {
        let records = [
            emf_extselectcliprgn_record(1, &[(20, 10, 90, 50), (100, 20, 140, 70)]),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 2);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipPath {
                start: (20.0, 10.0),
                ref segments,
                fill_rule: StaticImageVectorFillRule::Winding,
                ..
            } if segments.len() == 9
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle {
                left: 0.0,
                top: 0.0,
                right: 160.0,
                bottom: 80.0,
                ..
            }
        ));
    }

    #[test]
    fn emf_initial_extselectcliprgn_or_is_passive_noop() {
        let records = [
            emf_extselectcliprgn_record(EMF_RGN_OR, &[(20, 10, 90, 50), (100, 20, 140, 70)]),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_contained_extselectcliprgn_or_is_passive_noop() {
        let records = [
            emf_rect_record(30, 10, 10, 120, 70),
            emf_extselectcliprgn_record(EMF_RGN_OR, &[(20, 20, 80, 50), (90, 25, 110, 60)]),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipRect {
                left: 10.0,
                top: 10.0,
                right: 120.0,
                bottom: 70.0,
            }
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_empty_extselectcliprgn_or_diff_xor_are_passive_noops() {
        let records = [
            emf_rect_record(30, 10, 10, 120, 70),
            emf_extselectcliprgn_record(EMF_RGN_OR, &[]),
            emf_extselectcliprgn_record(EMF_RGN_DIFF, &[]),
            emf_extselectcliprgn_record(EMF_RGN_XOR, &[]),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipRect {
                left: 10.0,
                top: 10.0,
                right: 120.0,
                bottom: 70.0,
            }
        );
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_empty_extselectcliprgn_and_copy_create_empty_clip() {
        for mode in [EMF_RGN_AND, EMF_RGN_COPY] {
            let records = [
                emf_extselectcliprgn_record(mode, &[]),
                emf_rect_record(43, 0, 0, 160, 80),
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
            assert_eq!(image.vector_commands.len(), 2);
            assert_eq!(output.diagnostics.len(), 0);
            assert_eq!(
                image.vector_commands[0],
                StaticImageVectorCommand::ClipRect {
                    left: 0.0,
                    top: 0.0,
                    right: 0.0,
                    bottom: 0.0,
                }
            );
            assert!(matches!(
                image.vector_commands[1],
                StaticImageVectorCommand::Rectangle { .. }
            ));
        }
    }

    #[test]
    fn emf_empty_extselectcliprgn_copy_replaces_prior_unpainted_clip() {
        let records = [
            emf_rect_record(30, 10, 10, 120, 70),
            emf_extselectcliprgn_record(EMF_RGN_COPY, &[]),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipRect {
                left: 0.0,
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
            }
        );
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_extselectcliprgn_or_containing_unpainted_clip_rect_replaces_prior_clip() {
        let records = [
            emf_rect_record(30, 40, 20, 100, 60),
            emf_extselectcliprgn_record(EMF_RGN_OR, &[(10, 10, 130, 70)]),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipRect {
                left: 10.0,
                top: 10.0,
                right: 130.0,
                bottom: 70.0,
            }
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_extselectcliprgn_or_outside_unpainted_clip_rect_becomes_placeholder() {
        let records = [
            emf_rect_record(30, 10, 10, 120, 70),
            emf_extselectcliprgn_record(EMF_RGN_OR, &[(20, 20, 130, 50)]),
            emf_rect_record(43, 0, 0, 160, 80),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Image(image)
                if image.format == ImageFormat::Placeholder
                    && image.bytes.is_empty()
                    && image.vector_commands.is_empty()
        ));
    }

    #[test]
    fn emf_initial_extselectcliprgn_diff_becomes_passive_exclude_clip_path() {
        let records = [
            emf_extselectcliprgn_record(EMF_RGN_DIFF, &[(20, 10, 90, 50), (100, 20, 140, 70)]),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipPath {
                start: (0.0, 0.0),
                ref segments,
                fill_rule: StaticImageVectorFillRule::Alternate,
                ..
            } if segments.len() == 14
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_initial_extselectcliprgn_diff_with_overlapping_rects_becomes_placeholder() {
        let records = [
            emf_extselectcliprgn_record(EMF_RGN_DIFF, &[(20, 10, 90, 50), (80, 20, 140, 70)]),
            emf_rect_record(43, 0, 0, 160, 80),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Image(image)
                if image.format == ImageFormat::Placeholder
                    && image.bytes.is_empty()
                    && image.vector_commands.is_empty()
        ));
    }

    #[test]
    fn emf_extselectcliprgn_diff_from_unpainted_clip_rect_becomes_passive_exclude_clip_path() {
        let records = [
            emf_rect_record(30, 10, 10, 120, 70),
            emf_extselectcliprgn_record(EMF_RGN_DIFF, &[(40, 20, 80, 50)]),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipPath {
                start: (10.0, 10.0),
                ref segments,
                fill_rule: StaticImageVectorFillRule::Alternate,
                ..
            } if segments.starts_with(&[
                StaticImageVectorPathSegment::LineTo(120.0, 10.0),
                StaticImageVectorPathSegment::LineTo(120.0, 70.0),
                StaticImageVectorPathSegment::LineTo(10.0, 70.0),
                StaticImageVectorPathSegment::LineTo(10.0, 10.0),
                StaticImageVectorPathSegment::MoveTo(40.0, 20.0),
            ]) && segments.len() == 9
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_extselectcliprgn_diff_disjoint_from_unpainted_clip_rect_is_passive_noop() {
        let records = [
            emf_rect_record(30, 10, 10, 120, 70),
            emf_extselectcliprgn_record(EMF_RGN_DIFF, &[(130, 20, 150, 50)]),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipRect {
                left: 10.0,
                top: 10.0,
                right: 120.0,
                bottom: 70.0,
            }
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_extselectcliprgn_diff_from_unpainted_clip_rect_with_overlaps_becomes_placeholder() {
        let records = [
            emf_rect_record(30, 10, 10, 120, 70),
            emf_extselectcliprgn_record(EMF_RGN_DIFF, &[(30, 20, 80, 50), (70, 30, 100, 60)]),
            emf_rect_record(43, 0, 0, 160, 80),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Image(image)
                if image.format == ImageFormat::Placeholder
                    && image.bytes.is_empty()
            && image.vector_commands.is_empty()
        ));
    }

    #[test]
    fn emf_initial_extselectcliprgn_xor_becomes_passive_exclude_clip_path() {
        let records = [
            emf_extselectcliprgn_record(EMF_RGN_XOR, &[(20, 10, 90, 50), (100, 20, 140, 70)]),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipPath {
                start: (0.0, 0.0),
                ref segments,
                fill_rule: StaticImageVectorFillRule::Alternate,
                ..
            } if segments.len() == 14
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_extselectcliprgn_xor_inside_unpainted_clip_rect_excludes_region() {
        let records = [
            emf_rect_record(30, 10, 10, 120, 70),
            emf_extselectcliprgn_record(EMF_RGN_XOR, &[(40, 20, 80, 50)]),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipPath {
                start: (10.0, 10.0),
                ref segments,
                fill_rule: StaticImageVectorFillRule::Alternate,
                ..
            } if segments.starts_with(&[
                StaticImageVectorPathSegment::LineTo(120.0, 10.0),
                StaticImageVectorPathSegment::LineTo(120.0, 70.0),
                StaticImageVectorPathSegment::LineTo(10.0, 70.0),
                StaticImageVectorPathSegment::LineTo(10.0, 10.0),
                StaticImageVectorPathSegment::MoveTo(40.0, 20.0),
            ]) && segments.len() == 9
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_extselectcliprgn_xor_outside_unpainted_clip_rect_becomes_placeholder() {
        let records = [
            emf_rect_record(30, 10, 10, 120, 70),
            emf_extselectcliprgn_record(EMF_RGN_XOR, &[(130, 20, 150, 50)]),
            emf_rect_record(43, 0, 0, 160, 80),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Image(image)
                if image.format == ImageFormat::Placeholder
                    && image.bytes.is_empty()
                    && image.vector_commands.is_empty()
        ));
    }

    #[test]
    fn emf_extselectcliprgn_copy_over_unpainted_clip_replaces_prior_clip() {
        let records = [
            emf_rect_record(30, 5, 5, 30, 30),
            emf_extselectcliprgn_record(5, &[(20, 10, 90, 50)]),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipPath {
                start: (20.0, 10.0),
                ..
            }
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_extselectcliprgn_with_malformed_region_data_keeps_later_safe_drawing() {
        let mut record = emf_extselectcliprgn_record(1, &[(20, 10, 90, 50)]);
        write_test_le_u32(&mut record, 8, 31);
        let records = [record, emf_rect_record(43, 0, 0, 160, 80)];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Image(image)
                if image.format == ImageFormat::WmfVector
                    && image.bytes.is_empty()
                    && image.vector_commands.iter().any(|command| matches!(
                        command,
                        StaticImageVectorCommand::Rectangle { .. }
                    ))
        ));
    }

    #[test]
    fn emf_selectclippath_records_become_passive_clip_path_commands() {
        let records = [
            emf_unknown_record(59),
            emf_point_record(27, 20, 20),
            emf_poly_record(6, &[(80, 20), (50, 60)]),
            emf_unknown_record(61),
            emf_unknown_record(60),
            emf_u32_record(67, 1),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::ClipPath {
                start: (20.0, 20.0),
                segments,
                closed: true,
                fill_rule: StaticImageVectorFillRule::Alternate,
            } if matches!(
                &segments[..],
                [
                    StaticImageVectorPathSegment::LineTo(80.0, 20.0),
                    StaticImageVectorPathSegment::LineTo(50.0, 60.0),
                    StaticImageVectorPathSegment::LineTo(20.0, 20.0),
                ]
            )
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_selectclippath_copy_first_clip_becomes_passive_clip_path_command() {
        let records = [
            emf_unknown_record(59),
            emf_point_record(27, 20, 20),
            emf_poly_record(6, &[(80, 20), (50, 60)]),
            emf_unknown_record(61),
            emf_unknown_record(60),
            emf_u32_record(67, 5),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::ClipPath {
                start: (20.0, 20.0),
                segments,
                closed: true,
                fill_rule: StaticImageVectorFillRule::Alternate,
            } if matches!(
                &segments[..],
                [
                    StaticImageVectorPathSegment::LineTo(80.0, 20.0),
                    StaticImageVectorPathSegment::LineTo(50.0, 60.0),
                    StaticImageVectorPathSegment::LineTo(20.0, 20.0),
                ]
            )
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_initial_selectclippath_xor_becomes_passive_exclude_clip_path() {
        let records = [
            emf_unknown_record(59),
            emf_point_record(27, 20, 20),
            emf_poly_record(6, &[(80, 20), (50, 60)]),
            emf_unknown_record(60),
            emf_u32_record(67, EMF_RGN_XOR),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipPath {
                start: (0.0, 0.0),
                ref segments,
                fill_rule: StaticImageVectorFillRule::Alternate,
                ..
            } if segments.starts_with(&[
                StaticImageVectorPathSegment::LineTo(160.0, 0.0),
                StaticImageVectorPathSegment::LineTo(160.0, 80.0),
                StaticImageVectorPathSegment::LineTo(0.0, 80.0),
                StaticImageVectorPathSegment::LineTo(0.0, 0.0),
                StaticImageVectorPathSegment::MoveTo(20.0, 20.0),
            ]) && segments.contains(&StaticImageVectorPathSegment::LineTo(50.0, 60.0))
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_initial_selectclippath_or_is_passive_noop() {
        let records = [
            emf_unknown_record(59),
            emf_point_record(27, 20, 20),
            emf_poly_record(6, &[(80, 20), (50, 60)]),
            emf_unknown_record(60),
            emf_u32_record(67, EMF_RGN_OR),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_contained_selectclippath_or_is_passive_noop() {
        let records = [
            emf_rect_record(30, 10, 10, 120, 70),
            emf_unknown_record(59),
            emf_point_record(27, 20, 20),
            emf_poly_record(6, &[(80, 20), (50, 60)]),
            emf_unknown_record(60),
            emf_u32_record(67, EMF_RGN_OR),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipRect {
                left: 10.0,
                top: 10.0,
                right: 120.0,
                bottom: 70.0,
            }
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_selectclippath_or_rect_containing_unpainted_clip_rect_replaces_prior_clip() {
        let records = [
            emf_rect_record(30, 40, 20, 100, 60),
            emf_unknown_record(59),
            emf_point_record(27, 10, 10),
            emf_poly_record(6, &[(130, 10), (130, 70), (10, 70), (10, 10)]),
            emf_unknown_record(60),
            emf_u32_record(67, EMF_RGN_OR),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipRect {
                left: 10.0,
                top: 10.0,
                right: 130.0,
                bottom: 70.0,
            }
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_selectclippath_or_outside_unpainted_clip_rect_becomes_placeholder() {
        let records = [
            emf_rect_record(30, 10, 10, 120, 70),
            emf_unknown_record(59),
            emf_point_record(27, 20, 20),
            emf_poly_record(6, &[(130, 20), (50, 60)]),
            emf_unknown_record(60),
            emf_u32_record(67, EMF_RGN_OR),
            emf_rect_record(43, 0, 0, 160, 80),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Image(image)
                if image.format == ImageFormat::Placeholder
                    && image.bytes.is_empty()
                    && image.vector_commands.is_empty()
        ));
    }

    #[test]
    fn emf_initial_selectclippath_diff_becomes_passive_exclude_clip_path() {
        let records = [
            emf_unknown_record(59),
            emf_point_record(27, 20, 20),
            emf_poly_record(6, &[(80, 20), (50, 60)]),
            emf_unknown_record(60),
            emf_u32_record(67, EMF_RGN_DIFF),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipPath {
                start: (0.0, 0.0),
                ref segments,
                fill_rule: StaticImageVectorFillRule::Alternate,
                ..
            } if segments.starts_with(&[
                StaticImageVectorPathSegment::LineTo(160.0, 0.0),
                StaticImageVectorPathSegment::LineTo(160.0, 80.0),
                StaticImageVectorPathSegment::LineTo(0.0, 80.0),
                StaticImageVectorPathSegment::LineTo(0.0, 0.0),
                StaticImageVectorPathSegment::MoveTo(20.0, 20.0),
            ]) && segments.contains(&StaticImageVectorPathSegment::LineTo(50.0, 60.0))
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_initial_selectclippath_diff_with_winding_fill_becomes_placeholder() {
        let records = [
            emf_u32_record(19, u32::from(WMF_POLYFILLMODE_WINDING)),
            emf_unknown_record(59),
            emf_point_record(27, 20, 20),
            emf_poly_record(6, &[(80, 20), (50, 60)]),
            emf_unknown_record(60),
            emf_u32_record(67, EMF_RGN_DIFF),
            emf_rect_record(43, 0, 0, 160, 80),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Image(image)
                if image.format == ImageFormat::Placeholder
                    && image.bytes.is_empty()
                    && image.vector_commands.is_empty()
        ));
    }

    #[test]
    fn emf_selectclippath_diff_from_unpainted_clip_rect_becomes_passive_exclude_clip_path() {
        let records = [
            emf_rect_record(30, 10, 10, 120, 70),
            emf_unknown_record(59),
            emf_point_record(27, 40, 20),
            emf_poly_record(6, &[(80, 20), (50, 60)]),
            emf_unknown_record(60),
            emf_u32_record(67, EMF_RGN_DIFF),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipPath {
                start: (10.0, 10.0),
                ref segments,
                fill_rule: StaticImageVectorFillRule::Alternate,
                ..
            } if segments.starts_with(&[
                StaticImageVectorPathSegment::LineTo(120.0, 10.0),
                StaticImageVectorPathSegment::LineTo(120.0, 70.0),
                StaticImageVectorPathSegment::LineTo(10.0, 70.0),
                StaticImageVectorPathSegment::LineTo(10.0, 10.0),
                StaticImageVectorPathSegment::MoveTo(40.0, 20.0),
            ]) && segments.contains(&StaticImageVectorPathSegment::LineTo(50.0, 60.0))
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_selectclippath_diff_from_unpainted_clip_rect_with_outside_path_becomes_placeholder() {
        let records = [
            emf_rect_record(30, 10, 10, 120, 70),
            emf_unknown_record(59),
            emf_point_record(27, 40, 20),
            emf_poly_record(6, &[(140, 20), (50, 60)]),
            emf_unknown_record(60),
            emf_u32_record(67, EMF_RGN_DIFF),
            emf_rect_record(43, 0, 0, 160, 80),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Image(image)
                if image.format == ImageFormat::Placeholder
                    && image.bytes.is_empty()
                    && image.vector_commands.is_empty()
        ));
    }

    #[test]
    fn emf_selectclippath_copy_over_unpainted_clip_replaces_prior_clip() {
        let records = [
            emf_rect_record(30, 10, 10, 90, 70),
            emf_unknown_record(59),
            emf_point_record(27, 20, 20),
            emf_poly_record(6, &[(80, 20), (50, 60)]),
            emf_unknown_record(60),
            emf_u32_record(67, 5),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::ClipPath {
                start: (20.0, 20.0),
                segments,
                closed: false,
                fill_rule: StaticImageVectorFillRule::Alternate,
            } if matches!(
                &segments[..],
                [
                    StaticImageVectorPathSegment::LineTo(80.0, 20.0),
                    StaticImageVectorPathSegment::LineTo(50.0, 60.0),
                ]
            )
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_selectclippath_copy_after_painted_clip_replaces_unscoped_clip() {
        let records = [
            emf_rect_record(30, 10, 10, 90, 70),
            emf_rect_record(43, 0, 0, 30, 30),
            emf_unknown_record(59),
            emf_point_record(27, 20, 20),
            emf_poly_record(6, &[(80, 20), (50, 60)]),
            emf_unknown_record(60),
            emf_u32_record(67, 5),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 7);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::SaveState
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::ClipRect { .. }
        ));
        assert!(matches!(
            image.vector_commands[2],
            StaticImageVectorCommand::Rectangle { .. }
        ));
        assert!(matches!(
            image.vector_commands[3],
            StaticImageVectorCommand::RestoreState
        ));
        assert!(matches!(
            image.vector_commands[4],
            StaticImageVectorCommand::SaveState
        ));
        assert!(matches!(
            image.vector_commands[5],
            StaticImageVectorCommand::ClipPath { .. }
        ));
        assert!(matches!(
            image.vector_commands[6],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_selectclippath_copy_after_painted_clip_mutation_replaces_current_clip() {
        let records = [
            emf_rect_record(30, 10, 10, 90, 70),
            emf_rect_record(43, 0, 0, 30, 30),
            emf_rect_record(30, 20, 20, 100, 60),
            emf_unknown_record(59),
            emf_point_record(27, 20, 20),
            emf_poly_record(6, &[(80, 20), (50, 60)]),
            emf_unknown_record(60),
            emf_u32_record(67, 5),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 8);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::SaveState
        ));
        assert_eq!(
            image.vector_commands[1],
            StaticImageVectorCommand::ClipRect {
                left: 10.0,
                top: 10.0,
                right: 90.0,
                bottom: 70.0,
            }
        );
        assert!(matches!(
            image.vector_commands[2],
            StaticImageVectorCommand::Rectangle { .. }
        ));
        assert_eq!(
            image.vector_commands[3],
            StaticImageVectorCommand::ClipRect {
                left: 20.0,
                top: 20.0,
                right: 100.0,
                bottom: 60.0,
            }
        );
        assert!(matches!(
            image.vector_commands[4],
            StaticImageVectorCommand::RestoreState
        ));
        assert!(matches!(
            image.vector_commands[5],
            StaticImageVectorCommand::SaveState
        ));
        assert!(matches!(
            &image.vector_commands[6],
            StaticImageVectorCommand::ClipPath {
                start: (20.0, 20.0),
                segments,
                closed: false,
                fill_rule: StaticImageVectorFillRule::Alternate,
            } if matches!(
                &segments[..],
                [
                    StaticImageVectorPathSegment::LineTo(80.0, 20.0),
                    StaticImageVectorPathSegment::LineTo(50.0, 60.0),
                ]
            )
        ));
        assert!(matches!(
            image.vector_commands[7],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_selectclippath_copy_after_painted_saved_clip_replaces_scoped_clip() {
        let records = [
            emf_unknown_record(33),
            emf_rect_record(30, 10, 10, 90, 70),
            emf_rect_record(43, 0, 0, 30, 30),
            emf_unknown_record(59),
            emf_point_record(27, 20, 20),
            emf_poly_record(6, &[(80, 20), (50, 60)]),
            emf_unknown_record(60),
            emf_u32_record(67, 5),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 8);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::SaveState
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::ClipRect { .. }
        ));
        assert!(matches!(
            image.vector_commands[2],
            StaticImageVectorCommand::Rectangle { .. }
        ));
        assert!(matches!(
            image.vector_commands[3],
            StaticImageVectorCommand::RestoreState
        ));
        assert!(matches!(
            image.vector_commands[4],
            StaticImageVectorCommand::SaveState
        ));
        assert!(matches!(
            image.vector_commands[5],
            StaticImageVectorCommand::ClipPath { .. }
        ));
        assert!(matches!(
            image.vector_commands[6],
            StaticImageVectorCommand::Rectangle { .. }
        ));
        assert!(matches!(
            image.vector_commands[7],
            StaticImageVectorCommand::RestoreState
        ));
    }

    #[test]
    fn emf_excludecliprect_records_become_passive_even_odd_clip_path() {
        let records = [
            emf_rect_record(29, 20, 20, 100, 60),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::ClipPath {
                start: (0.0, 0.0),
                segments,
                fill_rule: StaticImageVectorFillRule::Alternate,
                ..
            } if matches!(
                &segments[..],
                [
                    StaticImageVectorPathSegment::LineTo(160.0, 0.0),
                    StaticImageVectorPathSegment::LineTo(160.0, 80.0),
                    StaticImageVectorPathSegment::LineTo(0.0, 80.0),
                    StaticImageVectorPathSegment::LineTo(0.0, 0.0),
                    StaticImageVectorPathSegment::MoveTo(20.0, 20.0),
                    StaticImageVectorPathSegment::LineTo(100.0, 20.0),
                    StaticImageVectorPathSegment::LineTo(100.0, 60.0),
                    StaticImageVectorPathSegment::LineTo(20.0, 60.0),
                    StaticImageVectorPathSegment::LineTo(20.0, 20.0),
                ]
            )
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_excludecliprect_after_unpainted_clip_rect_becomes_passive_exclude_clip_path() {
        let records = [
            emf_rect_record(30, 10, 10, 120, 70),
            emf_rect_record(29, 40, 20, 80, 50),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 2);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipPath {
                start: (10.0, 10.0),
                ref segments,
                fill_rule: StaticImageVectorFillRule::Alternate,
                ..
            } if segments.starts_with(&[
                StaticImageVectorPathSegment::LineTo(120.0, 10.0),
                StaticImageVectorPathSegment::LineTo(120.0, 70.0),
                StaticImageVectorPathSegment::LineTo(10.0, 70.0),
                StaticImageVectorPathSegment::LineTo(10.0, 10.0),
                StaticImageVectorPathSegment::MoveTo(40.0, 20.0),
            ]) && segments.len() == 9
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_excludecliprect_disjoint_from_unpainted_clip_rect_is_passive_noop() {
        let records = [
            emf_rect_record(30, 10, 10, 120, 70),
            emf_rect_record(29, 130, 20, 150, 50),
            emf_rect_record(43, 0, 0, 160, 80),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipRect {
                left: 10.0,
                top: 10.0,
                right: 120.0,
                bottom: 70.0,
            }
        );
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_filled_bezier_path_becomes_passive_path_command() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 80,
                    green: 140,
                    blue: 210,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_unknown_record(59),
            emf_point_record(27, 20, 20),
            emf_poly_record(5, &[(30, 5), (60, 55), (80, 20)]),
            emf_unknown_record(60),
            emf_unknown_record(62),
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
            &image.vector_commands[0],
            StaticImageVectorCommand::Path {
                start: (20.0, 20.0),
                segments,
                closed: false,
                stroke_color: None,
                stroke_width: 0.0,
                fill_color: Some(Color { red: 80, green: 140, blue: 210 }),
                ..
            } if matches!(
                &segments[..],
                [StaticImageVectorPathSegment::CubicTo {
                    control1: (30.0, 5.0),
                    control2: (60.0, 55.0),
                    end: (80.0, 20.0),
                }]
            )
        ));
    }

    #[test]
    fn strict_spec_emf_hatched_bezier_fillpath_becomes_passive_path_command() {
        let records = [
            emf_create_brush_record(
                3,
                2,
                Color {
                    red: 20,
                    green: 90,
                    blue: 180,
                },
                5,
            ),
            emf_select_object_record(3),
            emf_unknown_record(59),
            emf_point_record(27, 20, 20),
            emf_poly_record(5, &[(30, 5), (60, 55), (80, 20)]),
            emf_unknown_record(60),
            emf_unknown_record(62),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf_bytes_with_options(
            input.as_bytes(),
            &RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
        )
        .unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive EMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::Path {
                start: (20.0, 20.0),
                segments,
                fill_pattern: ShadingPattern::DiagonalCross,
                fill_color: Some(Color { red: 20, green: 90, blue: 180 }),
                ..
            } if matches!(
                &segments[..],
                [StaticImageVectorPathSegment::CubicTo {
                    control1: (30.0, 5.0),
                    control2: (60.0, 55.0),
                    end: (80.0, 20.0),
                }]
            )
        ));
    }

    #[test]
    fn word_compatible_emf_without_reference_device_metrics_is_suppressed() {
        let records = [
            emf_create_brush_record(
                3,
                2,
                Color {
                    red: 20,
                    green: 90,
                    blue: 180,
                },
                5,
            ),
            emf_select_object_record(3),
            emf_unknown_record(59),
            emf_point_record(27, 20, 20),
            emf_poly_record(5, &[(30, 5), (60, 55), (80, 20)]),
            emf_unknown_record(60),
            emf_unknown_record(62),
        ];
        let mut emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
        for offset in [72, 76, 80, 84] {
            write_test_le_i32(&mut emf, offset, 0);
        }
        let input = format!(
            r"{{\rtf1 Before{{\pict\emfblip {}}}After}}",
            bytes_to_hex(&emf)
        );
        let output = parse_rtf(&input).unwrap();

        assert!(
            !output
                .document
                .blocks
                .iter()
                .any(|block| matches!(block, Block::Image(_)))
        );
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("EMF picture with missing reference-device metrics suppressed")
        }));
    }

    #[test]
    fn emf_multi_subpath_fillpath_becomes_passive_path_command() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 30,
                    green: 170,
                    blue: 210,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_unknown_record(59),
            emf_point_record(27, 10, 10),
            emf_poly_record(6, &[(50, 10), (30, 40)]),
            emf_unknown_record(61),
            emf_point_record(27, 80, 20),
            emf_poly_record(6, &[(120, 20), (100, 50)]),
            emf_unknown_record(61),
            emf_unknown_record(60),
            emf_unknown_record(62),
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
            &image.vector_commands[0],
            StaticImageVectorCommand::Path {
                start: (10.0, 10.0),
                segments,
                fill_color: Some(Color { red: 30, green: 170, blue: 210 }),
                ..
            } if matches!(
                &segments[..],
                [
                    StaticImageVectorPathSegment::LineTo(50.0, 10.0),
                    StaticImageVectorPathSegment::LineTo(30.0, 40.0),
                    StaticImageVectorPathSegment::LineTo(10.0, 10.0),
                    StaticImageVectorPathSegment::MoveTo(80.0, 20.0),
                    StaticImageVectorPathSegment::LineTo(120.0, 20.0),
                    StaticImageVectorPathSegment::LineTo(100.0, 50.0),
                    StaticImageVectorPathSegment::LineTo(80.0, 20.0),
                ]
            )
        ));
    }

    #[test]
    fn emf_multi_subpath_strokepath_becomes_passive_path_command() {
        let records = [
            emf_unknown_record(59),
            emf_point_record(27, 10, 10),
            emf_poly_record(6, &[(50, 10), (30, 40)]),
            emf_point_record(27, 80, 20),
            emf_poly_record(6, &[(120, 20), (100, 50)]),
            emf_unknown_record(60),
            emf_unknown_record(64),
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
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::Path {
                start: (10.0, 10.0),
                segments,
                stroke_color: Some(Color { red: 0, green: 0, blue: 0 }),
                fill_color: None,
                ..
            } if segments.iter().any(|segment| matches!(
                segment,
                StaticImageVectorPathSegment::MoveTo(80.0, 20.0)
            ))
        ));
    }

    #[test]
    fn emf_stock_null_objects_suppress_passive_stroke_and_fill() {
        let records = [
            emf_select_object_record(0x8000_0005),
            emf_select_object_record(0x8000_0008),
            emf_rect_record(43, 10, 10, 70, 40),
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
        assert_eq!(
            image.vector_commands,
            vec![StaticImageVectorCommand::Rectangle {
                left: 10.0,
                top: 10.0,
                right: 70.0,
                bottom: 40.0,
                stroke_color: None,
                stroke_width: 1.0,
                stroke_style: BorderStyle::Single,
                fill_pattern: ShadingPattern::None,
                fill_color: None,
            }]
        );
    }

    #[test]
    fn emf_exttextoutw_records_become_passive_text_commands() {
        let records = [
            emf_u32_record(24, 0x0033_2211),
            emf_u32_record(22, 0x0000_0006),
            emf_exttextoutw_record(40, 20, "Hi\u{2019}", 0, None, false),
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
        assert_eq!(
            image.vector_commands,
            vec![StaticImageVectorCommand::Text {
                x: 40.0,
                y: 20.0,
                height: 6.6666665,
                bold: false,
                vertical_scale_percent: 100,
                text: "Hi\u{2019}".to_string(),
                color: Some(Color {
                    red: 17,
                    green: 34,
                    blue: 51,
                }),
                background_color: None,
                clip_bounds: None,
                character_extra: 0.0,
                word_extra: 0.0,
                horizontal_align: StaticImageTextHorizontalAlign::Center,
                vertical_align: StaticImageTextVerticalAlign::Top,
            }]
        );
    }

    #[test]
    fn emf_exttextouta_records_become_passive_text_commands() {
        let records = [
            emf_u32_record(24, 0x0066_4422),
            emf_u32_record(22, 0x0000_0002),
            emf_exttextouta_record(30, 18, b"Ansi\x97Text", 0, None, false),
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
            &image.vector_commands[0],
            StaticImageVectorCommand::Text {
                x: 30.0,
                y: 18.0,
                text,
                color: Some(Color { red: 34, green: 68, blue: 102 }),
                horizontal_align: StaticImageTextHorizontalAlign::Right,
                ..
            } if text == "Ansi\u{2014}Text"
        ));
    }

    #[test]
    fn emf_polytextoutw_records_become_passive_text_commands() {
        let records = [
            emf_u32_record(24, 0x0033_2211),
            emf_u32_record(22, 0x0000_0006),
            emf_polytextoutw_record(&[(40, 20, "One", 0, None), (70, 35, "Two", 0, None)]),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 2);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::Text {
                x: 40.0,
                y: 20.0,
                text,
                color: Some(Color { red: 17, green: 34, blue: 51 }),
                horizontal_align: StaticImageTextHorizontalAlign::Center,
                ..
            } if text == "One"
        ));
        assert!(matches!(
            &image.vector_commands[1],
            StaticImageVectorCommand::Text {
                x: 70.0,
                y: 35.0,
                text,
                color: Some(Color { red: 17, green: 34, blue: 51 }),
                horizontal_align: StaticImageTextHorizontalAlign::Center,
                ..
            } if text == "Two"
        ));
    }

    #[test]
    fn emf_polytextoutw_dx_records_become_positioned_passive_text_commands() {
        let records = [
            emf_u32_record(24, 0x0033_2211),
            emf_polytextoutw_dx_record(40, 20, "ABC", &[10, 20, 30]),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 3);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::Text { x: 40.0, text, .. } if text == "A"
        ));
        assert!(matches!(
            &image.vector_commands[1],
            StaticImageVectorCommand::Text { x: 50.0, text, .. } if text == "B"
        ));
        assert!(matches!(
            &image.vector_commands[2],
            StaticImageVectorCommand::Text { x: 70.0, text, .. } if text == "C"
        ));
    }

    #[test]
    fn emf_polytextouta_records_become_passive_text_commands() {
        let records = [
            emf_u32_record(24, 0x0066_4422),
            emf_polytextouta_record(&[(30, 18, b"Ansi\x97One".as_slice(), 0, None)]),
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
            &image.vector_commands[0],
            StaticImageVectorCommand::Text {
                x: 30.0,
                y: 18.0,
                text,
                color: Some(Color { red: 34, green: 68, blue: 102 }),
                ..
            } if text == "Ansi\u{2014}One"
        ));
    }

    #[test]
    fn emf_polytextouta_dx_records_become_positioned_passive_text_commands() {
        let records = [
            emf_u32_record(24, 0x0066_4422),
            emf_polytextouta_dx_record(30, 18, b"ABC", &[8, 12, 16]),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 3);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::Text { x: 30.0, text, .. } if text == "A"
        ));
        assert!(matches!(
            &image.vector_commands[1],
            StaticImageVectorCommand::Text { x: 38.0, text, .. } if text == "B"
        ));
        assert!(matches!(
            &image.vector_commands[2],
            StaticImageVectorCommand::Text { x: 50.0, text, .. } if text == "C"
        ));
    }

    #[test]
    fn emf_polytextoutw_with_excessive_total_text_becomes_passive_placeholder() {
        let first = "A".repeat(MAX_PASSIVE_WMF_TEXT_BYTES / 4);
        let second = "B".repeat((MAX_PASSIVE_WMF_TEXT_BYTES / 4) + 1);
        let records = [emf_polytextoutw_record(&[
            (40, 20, first.as_str(), 0, None),
            (70, 35, second.as_str(), 0, None),
        ])];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Image(image)
                if image.format == ImageFormat::Placeholder
                    && image.bytes.is_empty()
                    && image.vector_commands.is_empty()
        ));
    }

    #[test]
    fn emf_exttextoutw_opaque_bounds_become_passive_background_rectangle() {
        let records = [
            emf_u32_record(25, 0x0078_b4dc),
            emf_exttextoutw_record(40, 20, "Hi", 0x0002, Some((30, 10, 90, 35)), false),
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
        assert_eq!(
            image.vector_commands,
            vec![
                StaticImageVectorCommand::Rectangle {
                    left: 30.0,
                    top: 10.0,
                    right: 90.0,
                    bottom: 35.0,
                    stroke_color: None,
                    stroke_width: 0.0,
                    stroke_style: BorderStyle::Single,
                    fill_pattern: ShadingPattern::None,
                    fill_color: Some(Color {
                        red: 220,
                        green: 180,
                        blue: 120,
                    }),
                },
                StaticImageVectorCommand::Text {
                    x: 40.0,
                    y: 20.0,
                    height: 6.6666665,
                    bold: false,
                    vertical_scale_percent: 100,
                    text: "Hi".to_string(),
                    color: Some(Color::default()),
                    background_color: None,
                    clip_bounds: None,
                    character_extra: 0.0,
                    word_extra: 0.0,
                    horizontal_align: StaticImageTextHorizontalAlign::Left,
                    vertical_align: StaticImageTextVerticalAlign::Top,
                },
            ]
        );
    }

    #[test]
    fn emf_settextjustification_adds_passive_word_spacing() {
        let records = [
            emf_size_record(9, 320, 160),
            emf_size_record(11, 160, 80),
            emf_i32_pair_record(120, 40, 2),
            emf_exttextoutw_record(40, 20, "A B C", 0, None, false),
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
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Text {
                ref text,
                word_extra,
                character_extra: 0.0,
                ..
            } if text == "A B C" && (word_extra - 10.0).abs() < 0.01
        ));
    }

    #[test]
    fn emf_exttextoutw_with_excessive_text_becomes_passive_placeholder() {
        let text = "A".repeat((MAX_PASSIVE_WMF_TEXT_BYTES / 2) + 1);
        let records = [emf_exttextoutw_record(40, 20, &text, 0, None, false)];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Image(image)
                if image.format == ImageFormat::Placeholder
                    && image.bytes.is_empty()
                    && image.vector_commands.is_empty()
        ));
    }

    #[test]
    fn emf_exttextoutw_glyph_index_text_becomes_passive_placeholder() {
        let records = [emf_exttextoutw_record(40, 20, "Hi", 0, None, true)];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Image(image)
                if image.format == ImageFormat::Placeholder
                    && image.bytes.is_empty()
                    && image.vector_commands.is_empty()
        ));
    }

    #[test]
    fn emf_polyline_and_polygon_records_become_passive_vector_commands() {
        let records = [
            emf_poly_record(4, &[(0, 0), (40, 20), (200, 100)]),
            emf_poly_record(3, &[(20, 10), (60, 70), (100, 10)]),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(
            image.vector_commands[0],
            StaticImageVectorCommand::Polyline {
                points: vec![(0.0, 0.0), (40.0, 20.0), (160.0, 80.0)],
                stroke_color: Some(Color::default()),
                stroke_width: 1.0,
                stroke_style: BorderStyle::Single,
            }
        );
        assert_eq!(
            image.vector_commands[1],
            StaticImageVectorCommand::Polygon {
                points: vec![(20.0, 10.0), (60.0, 70.0), (100.0, 10.0)],
                stroke_color: Some(Color::default()),
                stroke_width: 1.0,
                stroke_style: BorderStyle::Single,
                fill_rule: StaticImageVectorFillRule::Alternate,
                fill_pattern: ShadingPattern::None,
                fill_color: None,
            }
        );
    }

    #[test]
    fn emf_polypolyline_and_polypolygon_records_become_passive_vector_commands() {
        let records = [
            emf_poly_poly_record(7, &[vec![(0, 0), (40, 20)], vec![(50, 10), (200, 100)]]),
            emf_poly_poly_record(
                8,
                &[
                    vec![(20, 10), (60, 70), (100, 10)],
                    vec![(100, 20), (140, 70), (200, 20)],
                ],
            ),
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
        assert_eq!(image.vector_commands.len(), 3);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Polyline { ref points, .. }
                if points == &vec![(0.0, 0.0), (40.0, 20.0)]
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Polyline { ref points, .. }
                if points == &vec![(50.0, 10.0), (160.0, 80.0)]
        ));
        assert!(matches!(
            image.vector_commands[2],
            StaticImageVectorCommand::Path {
                start: (20.0, 10.0),
                ref segments,
                fill_rule: StaticImageVectorFillRule::Alternate,
                ..
            } if matches!(
                &segments[..],
                [
                    StaticImageVectorPathSegment::LineTo(60.0, 70.0),
                    StaticImageVectorPathSegment::LineTo(100.0, 10.0),
                    StaticImageVectorPathSegment::LineTo(20.0, 10.0),
                    StaticImageVectorPathSegment::MoveTo(100.0, 20.0),
                    StaticImageVectorPathSegment::LineTo(140.0, 70.0),
                    StaticImageVectorPathSegment::LineTo(160.0, 20.0),
                    StaticImageVectorPathSegment::LineTo(100.0, 20.0),
                ]
            )
        ));
    }

    #[test]
    fn emf_polypolygon_with_mismatched_point_counts_becomes_passive_placeholder() {
        let records = [emf_poly_poly_record_with_counts(
            8,
            &[2, 2],
            &[(20, 10), (60, 70), (100, 10)],
        )];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Image(image)
                if image.format == ImageFormat::Placeholder
                    && image.bytes.is_empty()
                    && image.vector_commands.is_empty()
        ));
    }

    #[test]
    fn emf_polyline16_and_polygon16_records_become_passive_vector_commands() {
        let records = [
            emf_poly16_record(87, &[(0, 0), (40, 20), (200, 100)]),
            emf_poly16_record(86, &[(20, 10), (60, 70), (100, 10)]),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Polyline { ref points, .. }
                if points == &vec![(0.0, 0.0), (40.0, 20.0), (160.0, 80.0)]
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Polygon { ref points, .. }
                if points == &vec![(20.0, 10.0), (60.0, 70.0), (100.0, 10.0)]
        ));
    }

    #[test]
    fn emf_polypolyline16_and_polypolygon16_records_become_passive_vector_commands() {
        let records = [
            emf_poly_poly16_record(90, &[vec![(0, 0), (40, 20)], vec![(50, 10), (200, 100)]]),
            emf_poly_poly16_record(
                91,
                &[
                    vec![(20, 10), (60, 70), (100, 10)],
                    vec![(100, 20), (140, 70), (200, 20)],
                ],
            ),
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
        assert_eq!(image.vector_commands.len(), 3);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Polyline { ref points, .. }
                if points == &vec![(0.0, 0.0), (40.0, 20.0)]
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Polyline { ref points, .. }
                if points == &vec![(50.0, 10.0), (160.0, 80.0)]
        ));
        assert!(matches!(
            image.vector_commands[2],
            StaticImageVectorCommand::Path {
                start: (20.0, 10.0),
                ref segments,
                fill_rule: StaticImageVectorFillRule::Alternate,
                ..
            } if matches!(
                &segments[..],
                [
                    StaticImageVectorPathSegment::LineTo(60.0, 70.0),
                    StaticImageVectorPathSegment::LineTo(100.0, 10.0),
                    StaticImageVectorPathSegment::LineTo(20.0, 10.0),
                    StaticImageVectorPathSegment::MoveTo(100.0, 20.0),
                    StaticImageVectorPathSegment::LineTo(140.0, 70.0),
                    StaticImageVectorPathSegment::LineTo(160.0, 20.0),
                    StaticImageVectorPathSegment::LineTo(100.0, 20.0),
                ]
            )
        ));
    }

    #[test]
    fn emf_polypolygon_records_preserve_fill_rule_as_single_passive_path() {
        let records = [
            emf_u32_record(19, 2),
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 40,
                    green: 180,
                    blue: 90,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_poly_poly_record(
                8,
                &[
                    vec![(10, 10), (140, 10), (140, 70), (10, 70)],
                    vec![(50, 25), (100, 25), (100, 55), (50, 55)],
                ],
            ),
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
            &image.vector_commands[0],
            StaticImageVectorCommand::Path {
                start: (10.0, 10.0),
                segments,
                fill_rule: StaticImageVectorFillRule::Winding,
                fill_color: Some(Color { red: 40, green: 180, blue: 90 }),
                ..
            } if segments.iter().any(|segment| matches!(
                segment,
                StaticImageVectorPathSegment::MoveTo(50.0, 25.0)
            ))
        ));
    }

    #[test]
    fn emf_polypolygon16_with_mismatched_point_counts_becomes_passive_placeholder() {
        let records = [emf_poly_poly16_record_with_counts(
            91,
            &[2, 2],
            &[(20, 10), (60, 70), (100, 10)],
        )];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Image(image)
                if image.format == ImageFormat::Placeholder
                    && image.bytes.is_empty()
                    && image.vector_commands.is_empty()
        ));
    }

    #[test]
    fn emf_setpolyfillmode_changes_passive_polygon_fill_rule() {
        let records = [
            emf_u32_record(19, 2),
            emf_poly_record(3, &[(20, 10), (60, 70), (100, 10)]),
            emf_poly16_record(86, &[(100, 20), (140, 70), (200, 20)]),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Polygon {
                fill_rule: StaticImageVectorFillRule::Winding,
                ..
            }
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Polygon {
                fill_rule: StaticImageVectorFillRule::Winding,
                ..
            }
        ));
    }

    #[test]
    fn strict_spec_emf_setpixelv_records_become_passive_filled_rectangles() {
        let records = [
            emf_setpixelv_record(
                10,
                20,
                Color {
                    red: 20,
                    green: 40,
                    blue: 60,
                },
            ),
            emf_setpixelv_record(
                200,
                100,
                Color {
                    red: 220,
                    green: 180,
                    blue: 120,
                },
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf_bytes_with_options(
            input.as_bytes(),
            &RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
        )
        .unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive EMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(
            image.vector_commands,
            vec![
                StaticImageVectorCommand::Rectangle {
                    left: 10.0,
                    top: 20.0,
                    right: 11.0,
                    bottom: 21.0,
                    stroke_color: None,
                    stroke_width: 0.0,
                    stroke_style: BorderStyle::Single,
                    fill_pattern: ShadingPattern::None,
                    fill_color: Some(Color {
                        red: 20,
                        green: 40,
                        blue: 60
                    }),
                },
                StaticImageVectorCommand::Rectangle {
                    left: 159.0,
                    top: 79.0,
                    right: 160.0,
                    bottom: 80.0,
                    stroke_color: None,
                    stroke_width: 0.0,
                    stroke_style: BorderStyle::Single,
                    fill_pattern: ShadingPattern::None,
                    fill_color: Some(Color {
                        red: 220,
                        green: 180,
                        blue: 120
                    }),
                },
            ]
        );
    }

    #[test]
    fn word_compatible_valid_pixel_only_emf_is_rendered() {
        let records = [emf_setpixelv_record(
            10,
            20,
            Color {
                red: 20,
                green: 40,
                blue: 60,
            },
        )];
        let input = format!(
            r"{{\rtf1 Before{{\pict\emfblip {}}}After}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        assert!(
            output
                .document
                .blocks
                .iter()
                .any(|block| matches!(block, Block::Image(_)))
        );
        assert!(!output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("missing reference-device metrics")
        }));
    }

    #[test]
    fn emf_fillrgn_records_become_passive_filled_rectangles() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 40,
                    green: 120,
                    blue: 200,
                },
                0,
            ),
            emf_fillrgn_record(3, &[(20, 10, 90, 50), (100, 20, 140, 70)]),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 2);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 10.0,
                right: 90.0,
                bottom: 50.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 40,
                    green: 120,
                    blue: 200
                }),
                ..
            }
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle {
                left: 100.0,
                top: 20.0,
                right: 140.0,
                bottom: 70.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 40,
                    green: 120,
                    blue: 200
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_fillrgn_with_malformed_region_data_becomes_passive_placeholder() {
        let mut record = emf_fillrgn_record(3, &[(20, 10, 90, 50)]);
        write_test_le_u32(&mut record, 24, 31);
        let records = [emf_create_brush_record(3, 0, Color::default(), 0), record];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Image(image)
                if image.format == ImageFormat::Placeholder
                    && image.bytes.is_empty()
                    && image.vector_commands.is_empty()
        ));
    }

    #[test]
    fn emf_framergn_records_become_passive_border_rectangles() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 80,
                    green: 20,
                    blue: 210,
                },
                0,
            ),
            emf_framergn_record(3, 6, 4, &[(20, 10, 90, 50)]),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 4);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 10.0,
                right: 90.0,
                bottom: 14.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 80,
                    green: 20,
                    blue: 210
                }),
                ..
            }
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 46.0,
                right: 90.0,
                bottom: 50.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 80,
                    green: 20,
                    blue: 210
                }),
                ..
            }
        ));
        assert!(matches!(
            image.vector_commands[2],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 10.0,
                right: 26.0,
                bottom: 50.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 80,
                    green: 20,
                    blue: 210
                }),
                ..
            }
        ));
        assert!(matches!(
            image.vector_commands[3],
            StaticImageVectorCommand::Rectangle {
                left: 84.0,
                top: 10.0,
                right: 90.0,
                bottom: 50.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 80,
                    green: 20,
                    blue: 210
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_framergn_with_malformed_region_data_becomes_passive_placeholder() {
        let mut record = emf_framergn_record(3, 6, 4, &[(20, 10, 90, 50)]);
        write_test_le_u32(&mut record, 24, 31);
        let records = [emf_create_brush_record(3, 0, Color::default(), 0), record];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Image(image)
                if image.format == ImageFormat::Placeholder
                    && image.bytes.is_empty()
                    && image.vector_commands.is_empty()
        ));
    }

    #[test]
    fn emf_zero_frame_framergn_is_passive_noop_without_parsing_region_payload() {
        let mut frame = emf_framergn_record(0xFFFF_FFFE, 0, 4, &[(20, 10, 90, 50)]);
        frame.extend_from_slice(b"ZERO-FRAME-EMF-FRAMERGN /JavaScript /EmbeddedFile");
        frame.resize(frame.len().next_multiple_of(4), 0);
        let frame_len = frame.len() as u32;
        write_test_le_u32(&mut frame, 4, frame_len);
        write_test_le_u32(&mut frame, 24, 31);
        let records = [emf_rect_record(43, 0, 0, 30, 30), frame];
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_empty_paint_regions_are_passive_noops_without_parsing_payload() {
        let mut fill = emf_fillrgn_record(0xFFFF_FFFE, &[]);
        fill.extend_from_slice(b"EMPTY-EMF-FILLRGN /JavaScript");
        fill.resize(fill.len().next_multiple_of(4), 0);
        let fill_len = fill.len() as u32;
        write_test_le_u32(&mut fill, 4, fill_len);

        let mut frame = emf_framergn_record(0xFFFF_FFFE, 6, 4, &[]);
        frame.extend_from_slice(b"EMPTY-EMF-FRAMERGN /EmbeddedFile");
        frame.resize(frame.len().next_multiple_of(4), 0);
        let frame_len = frame.len() as u32;
        write_test_le_u32(&mut frame, 4, frame_len);

        let mut paint = emf_paintrgn_record(&[]);
        paint.extend_from_slice(b"EMPTY-EMF-PAINTRGN /Launch");
        paint.resize(paint.len().next_multiple_of(4), 0);
        let paint_len = paint.len() as u32;
        write_test_le_u32(&mut paint, 4, paint_len);

        let mut invert = emf_invertrgn_record(&[]);
        invert.extend_from_slice(b"EMPTY-EMF-INVERTRGN /OpenAction");
        invert.resize(invert.len().next_multiple_of(4), 0);
        let invert_len = invert.len() as u32;
        write_test_le_u32(&mut invert, 4, invert_len);

        let records = [
            emf_rect_record(43, 0, 0, 30, 30),
            fill,
            frame,
            paint,
            invert,
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_paintrgn_records_become_passive_filled_rectangles() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 220,
                    green: 90,
                    blue: 30,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_paintrgn_record(&[(15, 12, 70, 40), (80, 20, 130, 65)]),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 2);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 15.0,
                top: 12.0,
                right: 70.0,
                bottom: 40.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 220,
                    green: 90,
                    blue: 30
                }),
                ..
            }
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle {
                left: 80.0,
                top: 20.0,
                right: 130.0,
                bottom: 65.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 220,
                    green: 90,
                    blue: 30
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_initial_invertrgn_records_become_passive_black_rectangles() {
        let records = [emf_invertrgn_record(&[(15, 12, 70, 40), (80, 20, 130, 65)])];
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
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 2);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 15.0,
                top: 12.0,
                right: 70.0,
                bottom: 40.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 0,
                    green: 0,
                    blue: 0
                }),
                ..
            }
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle {
                left: 80.0,
                top: 20.0,
                right: 130.0,
                bottom: 65.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 0,
                    green: 0,
                    blue: 0
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_clipped_invertrgn_records_become_passive_clipped_black_rectangles() {
        let records = [
            emf_rect_record(30, 20, 10, 100, 60),
            emf_invertrgn_record(&[(15, 12, 70, 40), (80, 20, 130, 65)]),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 3);
        assert_eq!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipRect {
                left: 20.0,
                top: 10.0,
                right: 100.0,
                bottom: 60.0,
            }
        );
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle {
                left: 15.0,
                top: 12.0,
                right: 70.0,
                bottom: 40.0,
                fill_color: Some(Color {
                    red: 0,
                    green: 0,
                    blue: 0
                }),
                ..
            }
        ));
        assert!(matches!(
            image.vector_commands[2],
            StaticImageVectorCommand::Rectangle {
                left: 80.0,
                top: 20.0,
                right: 130.0,
                bottom: 65.0,
                fill_color: Some(Color {
                    red: 0,
                    green: 0,
                    blue: 0
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_same_bounds_invertrgn_after_solid_paint_inverts_fill_color() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 100,
                    green: 30,
                    blue: 200,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_bitblt_record(20, 15, 60, 30, WMF_PATCOPY_RASTER_OP, b"INVERTRGN-FILL"),
            emf_invertrgn_record(&[(20, 15, 80, 45)]),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected partial passive EMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 155,
                    green: 225,
                    blue: 55
                }),
                ..
            }
        ));
        assert_eq!(output.diagnostics.len(), 0);
    }

    #[test]
    fn emf_multi_rect_same_bounds_invertrgn_after_solid_paint_inverts_fill_colors() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 40,
                    green: 120,
                    blue: 210,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_fillrgn_record(3, &[(20, 15, 70, 35), (90, 20, 140, 50)]),
            emf_invertrgn_record(&[(20, 15, 70, 35), (90, 20, 140, 50)]),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(180, 90, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected partial passive EMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(image.vector_commands.len(), 2);
        for command in &image.vector_commands {
            assert!(matches!(
                command,
                StaticImageVectorCommand::Rectangle {
                    stroke_color: None,
                    fill_color: Some(Color {
                        red: 215,
                        green: 135,
                        blue: 45
                    }),
                    ..
                }
            ));
        }
        assert_eq!(output.diagnostics.len(), 0);
    }

    #[test]
    fn emf_nonmatching_invertrgn_after_paint_is_skipped_as_backdrop_dependent() {
        let records = [
            emf_rect_record(43, 0, 0, 30, 30),
            emf_invertrgn_record(&[(15, 12, 70, 40)]),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected partial passive EMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("1 unsupported record(s) skipped")
        }));
    }

    #[test]
    fn emf_paintrgn_with_malformed_region_data_becomes_passive_placeholder() {
        let mut record = emf_paintrgn_record(&[(20, 10, 90, 50)]);
        write_test_le_u32(&mut record, 24, 31);
        let records = [
            emf_create_brush_record(3, 0, Color::default(), 0),
            emf_select_object_record(3),
            record,
        ];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Image(image)
                if image.format == ImageFormat::Placeholder
                    && image.bytes.is_empty()
                    && image.vector_commands.is_empty()
        ));
    }

    #[test]
    fn emf_patcopy_bitblt_records_become_passive_filled_rectangles() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 120,
                    green: 80,
                    blue: 40,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_bitblt_record(
                10,
                20,
                80,
                40,
                WMF_PATCOPY_RASTER_OP,
                b"BITBLT-SOURCE-PAYLOAD",
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 10.0,
                top: 20.0,
                right: 90.0,
                bottom: 60.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 120,
                    green: 80,
                    blue: 40
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_black_brush_mergecopy_bitblt_record_becomes_passive_black_rectangle() {
        let records = [
            emf_create_brush_record(3, 0, Color::default(), 0),
            emf_select_object_record(3),
            emf_bitblt_record(
                10,
                20,
                80,
                40,
                WMF_MERGECOPY_RASTER_OP,
                b"MERGECOPY-SOURCE-PAYLOAD",
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 10.0,
                top: 20.0,
                right: 90.0,
                bottom: 60.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 0,
                    green: 0,
                    blue: 0
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_nonblack_mergecopy_stretchdibits_masks_embedded_dib_source() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 240,
                    green: 15,
                    blue: 170,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_rect_record(43, 0, 0, 30, 30),
            emf_stretchdibits_dib_record(
                20,
                15,
                60,
                30,
                WMF_MERGECOPY_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[255, 63, 170], [15, 240, 85]]),
            ),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
        assert!(matches!(
            &image.vector_commands[1],
            StaticImageVectorCommand::RasterImage {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                image,
            } if image.format == ImageFormat::Rgb8
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == vec![240, 15, 170, 0, 0, 0]
        ));
    }

    #[test]
    fn emf_mergecopy_solid_source_paints_source_and_brush_rectangle() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 240,
                    green: 15,
                    blue: 170,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_stretchdibits_dib_record(
                20,
                15,
                60,
                30,
                WMF_MERGECOPY_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 85, 204], [170, 85, 204]]),
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 160,
                    green: 5,
                    blue: 136
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_notpatcopy_stretchdibits_ignores_source_and_paints_inverted_brush() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 20,
                    green: 80,
                    blue: 140,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_rect_record(43, 0, 0, 30, 30),
            emf_stretchdibits_dib_record(
                20,
                15,
                60,
                30,
                WMF_NOTPATCOPY_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[255, 63, 170], [15, 240, 85]]),
            ),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 235,
                    green: 175,
                    blue: 115
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_black_brush_mergecopy_stretchdibits_jpeg_becomes_passive_black_rectangle() {
        let records = [
            emf_create_brush_record(3, 0, Color::default(), 0),
            emf_select_object_record(3),
            emf_rect_record(43, 0, 0, 30, 30),
            emf_stretchdibits_dib_record(
                20,
                15,
                60,
                30,
                WMF_MERGECOPY_RASTER_OP,
                &minimal_compressed_dib_with_payload(2, 1, 4, &minimal_jpeg_with_dimensions(2, 1)),
            ),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 0,
                    green: 0,
                    blue: 0
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_white_brush_mergecopy_stretchdibits_jpeg_copies_embedded_source() {
        let jpeg = minimal_jpeg_with_dimensions(2, 1);
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: u8::MAX,
                    green: u8::MAX,
                    blue: u8::MAX,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_stretchdibits_dib_record(
                20,
                15,
                60,
                30,
                WMF_MERGECOPY_RASTER_OP,
                &minimal_compressed_dib_with_payload(2, 1, 4, &jpeg),
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                image,
            } if image.format == ImageFormat::Jpeg
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == jpeg
        ));
    }

    #[test]
    fn emf_initial_dstinvert_bitblt_record_becomes_passive_black_rectangle() {
        let records = [emf_bitblt_record(
            10,
            20,
            80,
            40,
            WMF_DSTINVERT_RASTER_OP,
            b"DSTINVERT-SOURCE-PAYLOAD",
        )];
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 10.0,
                top: 20.0,
                right: 90.0,
                bottom: 60.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 0,
                    green: 0,
                    blue: 0
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_blank_source_erasing_raster_ops_become_passive_black_rectangles() {
        let cases = [
            (
                WMF_SRCERASE_RASTER_OP,
                b"SRCERASE-SOURCE-PAYLOAD".as_slice(),
            ),
            (
                WMF_NOTSRCERASE_RASTER_OP,
                b"NOTSRCERASE-SOURCE-PAYLOAD".as_slice(),
            ),
        ];

        for (raster_op, payload) in cases {
            let records = [emf_bitblt_record(10, 20, 80, 40, raster_op, payload)];
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
            assert_eq!(output.diagnostics.len(), 0);
            assert!(matches!(
                image.vector_commands[0],
                StaticImageVectorCommand::Rectangle {
                    left: 10.0,
                    top: 20.0,
                    right: 90.0,
                    bottom: 60.0,
                    stroke_color: None,
                    fill_color: Some(Color {
                        red: 0,
                        green: 0,
                        blue: 0
                    }),
                    ..
                }
            ));
        }
    }

    #[test]
    fn emf_blank_source_or_pattern_paint_raster_ops_become_passive_white_rectangles() {
        let cases = [
            (
                WMF_MERGEPAINT_RASTER_OP,
                b"MERGEPAINT-SOURCE-PAYLOAD".as_slice(),
            ),
            (
                WMF_SRCPAINT_RASTER_OP,
                b"SRCPAINT-SOURCE-PAYLOAD".as_slice(),
            ),
            (
                WMF_PATPAINT_RASTER_OP,
                b"PATPAINT-SOURCE-PAYLOAD".as_slice(),
            ),
        ];

        for (raster_op, payload) in cases {
            let records = [emf_bitblt_record(10, 20, 80, 40, raster_op, payload)];
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
            assert_eq!(output.diagnostics.len(), 0);
            assert!(matches!(
                image.vector_commands[0],
                StaticImageVectorCommand::Rectangle {
                    left: 10.0,
                    top: 20.0,
                    right: 90.0,
                    bottom: 60.0,
                    stroke_color: None,
                    fill_color: Some(Color {
                        red: 255,
                        green: 255,
                        blue: 255
                    }),
                    ..
                }
            ));
        }
    }

    #[test]
    fn emf_srcerase_after_paint_is_skipped_as_backdrop_dependent() {
        let records = [
            emf_rect_record(43, 0, 0, 30, 30),
            emf_bitblt_record(
                10,
                20,
                80,
                40,
                WMF_SRCERASE_RASTER_OP,
                b"SRCERASE-SOURCE-PAYLOAD",
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected partial passive EMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("1 unsupported record(s) skipped")
        }));
    }

    #[test]
    fn emf_white_brush_patpaint_after_paint_becomes_passive_white_rectangle() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: u8::MAX,
                    green: u8::MAX,
                    blue: u8::MAX,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_rect_record(43, 0, 0, 30, 30),
            emf_bitblt_record(
                10,
                20,
                80,
                40,
                WMF_PATPAINT_RASTER_OP,
                b"PATPAINT-SOURCE-PAYLOAD",
            ),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle {
                left: 10.0,
                top: 20.0,
                right: 90.0,
                bottom: 60.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 255,
                    green: 255,
                    blue: 255
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_patpaint_solid_source_on_blank_destination_paints_pattern_or_inverted_source() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 20,
                    green: 80,
                    blue: 140,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_stretchdibits_dib_record(
                20,
                15,
                60,
                30,
                WMF_PATPAINT_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[255, 255, 255], [255, 255, 255]]),
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 20,
                    green: 80,
                    blue: 140
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_same_bounds_patpaint_solid_source_after_solid_fill_ors_pattern_and_inverted_source() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 20,
                    green: 80,
                    blue: 140,
                },
                0,
            ),
            emf_create_brush_record(
                4,
                0,
                Color {
                    red: 100,
                    green: 30,
                    blue: 200,
                },
                0,
            ),
            emf_select_object_record(4),
            emf_bitblt_record(20, 15, 60, 30, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            emf_select_object_record(3),
            emf_stretchdibits_dib_record(
                20,
                15,
                60,
                30,
                WMF_PATPAINT_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]),
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 117,
                    green: 95,
                    blue: 221
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_nonwhite_patpaint_after_paint_is_skipped_as_backdrop_dependent() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 20,
                    green: 80,
                    blue: 140,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_rect_record(43, 0, 0, 30, 30),
            emf_bitblt_record(
                10,
                20,
                80,
                40,
                WMF_PATPAINT_RASTER_OP,
                b"PATPAINT-SOURCE-PAYLOAD",
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected partial passive EMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(image.vector_commands.len(), 1);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("1 unsupported record(s) skipped")
        }));
    }

    #[test]
    fn emf_after_paint_patpaint_black_source_becomes_passive_white_rectangle() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 20,
                    green: 80,
                    blue: 140,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_rect_record(43, 0, 0, 30, 30),
            emf_stretchdibits_dib_record(
                20,
                15,
                60,
                30,
                WMF_PATPAINT_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[0, 0, 0], [0, 0, 0]]),
            ),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 255,
                    green: 255,
                    blue: 255
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_after_paint_srcand_black_source_becomes_passive_black_rectangle() {
        let records = [
            emf_rect_record(43, 0, 0, 30, 30),
            emf_stretchdibits_dib_record(
                20,
                15,
                60,
                30,
                WMF_SRCAND_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[0, 0, 0], [0, 0, 0]]),
            ),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 0,
                    green: 0,
                    blue: 0
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_same_bounds_srcand_solid_source_after_solid_fill_ands_rectangle() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 100,
                    green: 30,
                    blue: 200,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_bitblt_record(20, 15, 60, 30, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            emf_stretchdibits_dib_record(
                20,
                15,
                60,
                30,
                WMF_SRCAND_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]),
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 32,
                    green: 10,
                    blue: 136
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_same_bounds_notsrcerase_solid_source_after_solid_fill_nors_rectangle() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 100,
                    green: 30,
                    blue: 200,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_bitblt_record(20, 15, 60, 30, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            emf_stretchdibits_dib_record(
                20,
                15,
                60,
                30,
                WMF_NOTSRCERASE_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]),
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 17,
                    green: 65,
                    blue: 21
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_same_bounds_srcpaint_solid_source_after_solid_fill_ors_rectangle() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 100,
                    green: 30,
                    blue: 200,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_bitblt_record(20, 15, 60, 30, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            emf_stretchdibits_dib_record(
                20,
                15,
                60,
                30,
                WMF_SRCPAINT_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]),
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 238,
                    green: 190,
                    blue: 234
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_same_bounds_mergepaint_solid_source_after_solid_fill_ors_inverted_source_rectangle() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 100,
                    green: 30,
                    blue: 200,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_bitblt_record(20, 15, 60, 30, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            emf_stretchdibits_dib_record(
                20,
                15,
                60,
                30,
                WMF_MERGEPAINT_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]),
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 117,
                    green: 95,
                    blue: 221
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_same_bounds_srcerase_white_source_after_solid_fill_inverts_dest_rectangle() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 100,
                    green: 30,
                    blue: 200,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_bitblt_record(20, 15, 60, 30, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            emf_stretchdibits_dib_record(
                20,
                15,
                60,
                30,
                WMF_SRCERASE_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[255, 255, 255], [255, 255, 255]]),
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 155,
                    green: 225,
                    blue: 55
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_same_bounds_srcerase_solid_source_after_solid_fill_ands_inverted_dest_rectangle() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 100,
                    green: 30,
                    blue: 200,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_bitblt_record(20, 15, 60, 30, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            emf_stretchdibits_dib_record(
                20,
                15,
                60,
                30,
                WMF_SRCERASE_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]),
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 138,
                    green: 160,
                    blue: 34
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_after_paint_srcerase_black_source_is_passive_noop_without_skipped_diagnostic() {
        let records = [
            emf_rect_record(43, 0, 0, 30, 30),
            emf_stretchdibits_dib_record(
                20,
                15,
                60,
                30,
                WMF_SRCERASE_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[0, 0, 0], [0, 0, 0]]),
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_after_paint_srcinvert_black_source_is_passive_noop_without_skipped_diagnostic() {
        let records = [
            emf_rect_record(43, 0, 0, 30, 30),
            emf_stretchdibits_dib_record(
                20,
                15,
                60,
                30,
                WMF_SRCINVERT_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[0, 0, 0], [0, 0, 0]]),
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_dstinvert_bitblt_after_paint_is_skipped_as_backdrop_dependent() {
        let records = [
            emf_rect_record(43, 0, 0, 30, 30),
            emf_bitblt_record(
                10,
                20,
                80,
                40,
                WMF_DSTINVERT_RASTER_OP,
                b"DSTINVERT-SOURCE-PAYLOAD",
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected partial passive EMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("1 unsupported record(s) skipped")
        }));
    }

    #[test]
    fn emf_same_bounds_srcinvert_white_source_after_solid_fill_inverts_rectangle() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 100,
                    green: 30,
                    blue: 200,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_bitblt_record(20, 15, 60, 30, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            emf_stretchdibits_dib_record(
                20,
                15,
                60,
                30,
                WMF_SRCINVERT_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[255, 255, 255], [255, 255, 255]]),
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 155,
                    green: 225,
                    blue: 55
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_same_bounds_srcinvert_solid_source_after_solid_fill_xors_rectangle() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 100,
                    green: 30,
                    blue: 200,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_bitblt_record(20, 15, 60, 30, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            emf_stretchdibits_dib_record(
                20,
                15,
                60,
                30,
                WMF_SRCINVERT_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]),
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 206,
                    green: 180,
                    blue: 98
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_same_bounds_dstinvert_bitblt_after_solid_fill_inverts_rectangle() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 100,
                    green: 30,
                    blue: 200,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_bitblt_record(10, 20, 80, 40, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            emf_bitblt_record(
                10,
                20,
                80,
                40,
                WMF_DSTINVERT_RASTER_OP,
                b"DSTINVERT-SOURCE-PAYLOAD",
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 10.0,
                top: 20.0,
                right: 90.0,
                bottom: 60.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 155,
                    green: 225,
                    blue: 55
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_dstcopy_bitblt_record_is_passive_noop_without_skipped_diagnostic() {
        let records = [
            emf_rect_record(43, 0, 0, 30, 30),
            emf_bitblt_record(
                10,
                20,
                80,
                40,
                WMF_DSTCOPY_RASTER_OP,
                b"DSTCOPY-SOURCE-PAYLOAD",
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_black_brush_patinvert_after_paint_is_passive_noop_without_skipped_diagnostic() {
        let records = [
            emf_create_brush_record(3, 0, Color::default(), 0),
            emf_select_object_record(3),
            emf_rect_record(43, 0, 0, 30, 30),
            emf_bitblt_record(
                10,
                20,
                80,
                40,
                WMF_PATINVERT_RASTER_OP,
                b"PATINVERT-SOURCE-PAYLOAD",
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_initial_patinvert_bitblt_record_becomes_passive_inverted_brush_rectangle() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 20,
                    green: 80,
                    blue: 140,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_bitblt_record(
                10,
                20,
                80,
                40,
                WMF_PATINVERT_RASTER_OP,
                b"PATINVERT-SOURCE-PAYLOAD",
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 10.0,
                top: 20.0,
                right: 90.0,
                bottom: 60.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 235,
                    green: 175,
                    blue: 115
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_patinvert_bitblt_after_paint_is_skipped_as_backdrop_dependent() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 20,
                    green: 80,
                    blue: 140,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_rect_record(43, 0, 0, 30, 30),
            emf_bitblt_record(
                10,
                20,
                80,
                40,
                WMF_PATINVERT_RASTER_OP,
                b"PATINVERT-SOURCE-PAYLOAD",
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected partial passive EMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("1 unsupported record(s) skipped")
        }));
    }

    #[test]
    fn emf_same_bounds_patinvert_bitblt_after_solid_fill_xors_rectangle() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 100,
                    green: 30,
                    blue: 200,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_bitblt_record(10, 20, 80, 40, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            emf_create_brush_record(
                4,
                0,
                Color {
                    red: 20,
                    green: 80,
                    blue: 140,
                },
                0,
            ),
            emf_select_object_record(4),
            emf_bitblt_record(
                10,
                20,
                80,
                40,
                WMF_PATINVERT_RASTER_OP,
                b"PATINVERT-SOURCE-PAYLOAD",
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 10.0,
                top: 20.0,
                right: 90.0,
                bottom: 60.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 112,
                    green: 78,
                    blue: 68
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_source_backed_patinvert_after_solid_fill_ignores_dib_and_xors_rectangle() {
        let mut dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[1, 2, 3], [250, 251, 252]]);
        dib.extend_from_slice(b"TRAILING-EMF-PATINVERT-DIB-SOURCE /JavaScript");
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 100,
                    green: 30,
                    blue: 200,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_bitblt_record(10, 20, 80, 40, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            emf_create_brush_record(
                4,
                0,
                Color {
                    red: 20,
                    green: 80,
                    blue: 140,
                },
                0,
            ),
            emf_select_object_record(4),
            emf_stretchdibits_dib_record(10, 20, 80, 40, WMF_PATINVERT_RASTER_OP, &dib),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 10.0,
                top: 20.0,
                right: 90.0,
                bottom: 60.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 112,
                    green: 78,
                    blue: 68
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_notpatcopy_bitblt_record_becomes_passive_inverted_brush_rectangle() {
        let records = [
            emf_rect_record(43, 0, 0, 30, 30),
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 20,
                    green: 80,
                    blue: 140,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_bitblt_record(
                10,
                20,
                80,
                40,
                WMF_NOTPATCOPY_RASTER_OP,
                b"NOTPATCOPY-SOURCE-PAYLOAD",
            ),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle {
                left: 10.0,
                top: 20.0,
                right: 90.0,
                bottom: 60.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 235,
                    green: 175,
                    blue: 115
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_patcopy_stretchblt_records_become_passive_filled_rectangles() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 30,
                    green: 150,
                    blue: 200,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_stretchblt_record(
                15,
                25,
                70,
                35,
                WMF_PATCOPY_RASTER_OP,
                b"STRETCHBLT-SOURCE-PAYLOAD",
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 15.0,
                top: 25.0,
                right: 85.0,
                bottom: 60.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 30,
                    green: 150,
                    blue: 200
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_srccopy_bitblt_dib_record_becomes_passive_raster_image() {
        let records = [emf_bitblt_dib_record(
            10,
            20,
            2,
            1,
            WMF_SRCCOPY_RASTER_OP,
            &minimal_24bit_dib_with_dimensions(2, 1),
        )];
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage {
                left: 10.0,
                top: 20.0,
                right: 12.0,
                bottom: 21.0,
                image,
            } if image.format == ImageFormat::Rgb8
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == vec![255, 0, 0, 0, 255, 0]
        ));
    }

    #[test]
    fn emf_srccopy_solid_source_dib_record_becomes_passive_solid_rectangle() {
        let records = [emf_stretchdibits_dib_record(
            20,
            15,
            60,
            30,
            WMF_SRCCOPY_RASTER_OP,
            &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[12, 34, 56], [12, 34, 56]]),
        )];
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 12,
                    green: 34,
                    blue: 56
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_notsrccopy_solid_source_dib_record_becomes_passive_solid_rectangle() {
        let records = [emf_stretchdibits_dib_record(
            20,
            15,
            60,
            30,
            WMF_NOTSRCCOPY_RASTER_OP,
            &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[12, 34, 56], [12, 34, 56]]),
        )];
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 243,
                    green: 221,
                    blue: 199
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_srccopy_flat_color_alpha_source_remains_passive_raster_image() {
        let records = [emf_stretchdibits_dib_record(
            20,
            15,
            60,
            30,
            WMF_SRCCOPY_RASTER_OP,
            &minimal_compressed_dib_with_payload(
                2,
                1,
                5,
                &minimal_rgba_png(&[[12, 34, 56, 64], [12, 34, 56, 192]]),
            ),
        )];
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
        assert_eq!(output.diagnostics.len(), 0);
        let StaticImageVectorCommand::RasterImage {
            left: 20.0,
            top: 15.0,
            right: 80.0,
            bottom: 45.0,
            image,
        } = &image.vector_commands[0]
        else {
            panic!("flat RGB source with alpha must not collapse to an opaque rectangle");
        };
        assert_eq!(image.format, ImageFormat::Png);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        assert_eq!(
            unfiltered_png_scanlines(&image.bytes, 2, 1, 3).unwrap().3,
            vec![0, 12, 34, 56, 12, 34, 56]
        );
        let alpha_mask = image.alpha_mask.as_ref().expect("source alpha mask");
        assert_eq!(
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&alpha_mask.bytes, 3).unwrap(),
            vec![0, 64, 192]
        );
    }

    #[test]
    fn emf_srccopy_stretchblt_bi_png_record_becomes_passive_raster_image() {
        let png = minimal_rgb_png_with_dimensions(2, 1);
        let records = [emf_stretchblt_dib_record(
            15,
            25,
            70,
            35,
            WMF_SRCCOPY_RASTER_OP,
            &minimal_compressed_dib_with_payload(2, 1, 5, &png),
        )];
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage {
                left: 15.0,
                top: 25.0,
                right: 85.0,
                bottom: 60.0,
                image,
            } if image.format == ImageFormat::Png
                && image.width_px == 2
                && image.height_px == 1
                && image.palette.is_empty()
                && image.alpha_mask.is_none()
        ));
    }

    #[test]
    fn emf_setdibitstodevice_dib_record_becomes_passive_raster_image() {
        let rows = [
            [10, 20, 30],
            [40, 50, 60],
            [70, 80, 90],
            [100, 110, 120],
            [130, 140, 150],
            [160, 170, 180],
        ];
        let mut top_down_compressed_png_dib =
            minimal_compressed_dib_with_payload(2, 3, 5, &minimal_rgb_png_with_rows(2, 3, &rows));
        write_test_le_i32(&mut top_down_compressed_png_dib, 8, -3);
        let records = [
            emf_setdibitstodevice_dib_record(
                12,
                18,
                2,
                1,
                0,
                1,
                &minimal_24bit_dib_with_dimensions(2, 1),
            ),
            emf_setdibitstodevice_dib_record(
                50,
                30,
                2,
                3,
                1,
                1,
                &minimal_24bit_dib_with_rgb_pixels(2, 3, &rows),
            ),
            emf_setdibitstodevice_dib_record(
                80,
                44,
                2,
                3,
                3,
                1,
                &minimal_24bit_dib_with_rgb_pixels(2, 3, &rows),
            ),
            emf_setdibitstodevice_dib_record(
                86,
                48,
                2,
                3,
                1,
                1,
                &minimal_compressed_dib_with_payload(
                    2,
                    3,
                    5,
                    &minimal_rgb_png_with_rows(2, 3, &rows),
                ),
            ),
            emf_setdibitstodevice_dib_record(112, 56, 2, 3, 0, 1, &top_down_compressed_png_dib),
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
        assert_eq!(image.vector_commands.len(), 4);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("1 unsupported record(s) skipped")
        }));
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage {
                left: 12.0,
                top: 18.0,
                right: 14.0,
                bottom: 19.0,
                image,
            } if image.format == ImageFormat::Rgb8
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == vec![255, 0, 0, 0, 255, 0]
        ));
        assert!(matches!(
            &image.vector_commands[1],
            StaticImageVectorCommand::RasterImage {
                left: 50.0,
                top: 30.0,
                right: 52.0,
                bottom: 31.0,
                image,
            } if image.format == ImageFormat::Rgb8
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == vec![70, 80, 90, 100, 110, 120]
        ));
        assert!(matches!(
            &image.vector_commands[2],
            StaticImageVectorCommand::RasterImage {
                left: 86.0,
                top: 48.0,
                right: 88.0,
                bottom: 49.0,
                image,
            } if image.format == ImageFormat::Png
                && image.width_px == 2
                && image.height_px == 1
                && image.palette.is_empty()
                && image.alpha_mask.is_none()
                && unfiltered_png_scanlines(&image.bytes, 2, 1, 3).unwrap().3
                    == vec![0, 70, 80, 90, 100, 110, 120]
        ));
        assert!(matches!(
            &image.vector_commands[3],
            StaticImageVectorCommand::RasterImage {
                left: 112.0,
                top: 56.0,
                right: 114.0,
                bottom: 57.0,
                image,
            } if image.format == ImageFormat::Png
                && image.width_px == 2
                && image.height_px == 1
                && image.palette.is_empty()
                && image.alpha_mask.is_none()
                && unfiltered_png_scanlines(&image.bytes, 2, 1, 3).unwrap().3
                    == vec![0, 10, 20, 30, 40, 50, 60]
        ));
    }

    #[test]
    fn emf_constant_alpha_alphablend_dib_record_becomes_passive_alpha_mask() {
        let records = [
            emf_alphablend_dib_record(
                14,
                22,
                44,
                24,
                0xff,
                0,
                &minimal_24bit_dib_with_dimensions(2, 1),
            ),
            emf_alphablend_dib_record(
                60,
                30,
                44,
                24,
                0x80,
                0,
                &minimal_24bit_dib_with_dimensions(2, 1),
            ),
            emf_alphablend_dib_record(
                96,
                38,
                44,
                24,
                0xff,
                1,
                &minimal_32bit_dib_with_rgba_pixels(2, 1, &[[255, 0, 0, 64], [0, 255, 0, 192]]),
            ),
            emf_alphablend_dib_record(
                112,
                44,
                44,
                24,
                0xff,
                1,
                &minimal_32bit_bitfields_dib_with_rgba_pixels(
                    2,
                    1,
                    &[[255, 0, 0, 32], [0, 255, 0, 224]],
                ),
            ),
            emf_alphablend_dib_record(
                128,
                48,
                44,
                24,
                0xff,
                1,
                &minimal_compressed_dib_with_payload(
                    2,
                    1,
                    5,
                    &minimal_rgba_png(&[[255, 0, 0, 16], [0, 255, 0, 240]]),
                ),
            ),
            emf_alphablend_dib_record(
                10,
                56,
                44,
                20,
                0x60,
                0,
                &minimal_compressed_dib_with_payload(2, 1, 4, &minimal_jpeg_with_dimensions(2, 1)),
            ),
            emf_alphablend_dib_record(
                144,
                52,
                44,
                24,
                0xff,
                1,
                &minimal_24bit_dib_with_dimensions(2, 1),
            ),
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
        assert_eq!(image.vector_commands.len(), 6);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("1 unsupported record(s) skipped")
        }));
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage {
                left: 14.0,
                top: 22.0,
                right: 58.0,
                bottom: 46.0,
                image,
            } if image.format == ImageFormat::Rgb8
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == vec![255, 0, 0, 0, 255, 0]
                && image.alpha_mask.is_none()
        ));
        let alpha_mask = match &image.vector_commands[1] {
            StaticImageVectorCommand::RasterImage { image, .. } => {
                image.alpha_mask.as_ref().expect("constant alpha mask")
            }
            _ => panic!("expected constant-alpha raster image"),
        };
        assert_eq!(
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&alpha_mask.bytes, 3).unwrap(),
            vec![0, 0x80, 0x80]
        );
        let source_alpha_mask = match &image.vector_commands[2] {
            StaticImageVectorCommand::RasterImage { image, .. } => image
                .alpha_mask
                .as_ref()
                .expect("per-pixel source alpha mask"),
            _ => panic!("expected per-pixel alpha raster image"),
        };
        assert_eq!(
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&source_alpha_mask.bytes, 3)
                .unwrap(),
            vec![0, 64, 192]
        );
        let bitfield_alpha_mask = match &image.vector_commands[3] {
            StaticImageVectorCommand::RasterImage { image, .. } => image
                .alpha_mask
                .as_ref()
                .expect("bitfield source alpha mask"),
            _ => panic!("expected bitfield alpha raster image"),
        };
        assert_eq!(
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&bitfield_alpha_mask.bytes, 3)
                .unwrap(),
            vec![0, 32, 224]
        );
        let compressed_png_alpha_mask = match &image.vector_commands[4] {
            StaticImageVectorCommand::RasterImage { image, .. } => image
                .alpha_mask
                .as_ref()
                .expect("compressed PNG source alpha mask"),
            _ => panic!("expected compressed PNG alpha raster image"),
        };
        assert_eq!(
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(
                &compressed_png_alpha_mask.bytes,
                3,
            )
            .unwrap(),
            vec![0, 16, 240]
        );
        let jpeg_alpha_mask = match &image.vector_commands[5] {
            StaticImageVectorCommand::RasterImage { image, .. } => image
                .alpha_mask
                .as_ref()
                .expect("constant-alpha JPEG source mask"),
            _ => panic!("expected constant-alpha JPEG raster image"),
        };
        assert_eq!(
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&jpeg_alpha_mask.bytes, 3)
                .unwrap(),
            vec![0, 0x60, 0x60]
        );
        assert!(matches!(
            &image.vector_commands[5],
            StaticImageVectorCommand::RasterImage {
                left: 10.0,
                top: 56.0,
                right: 54.0,
                bottom: 76.0,
                image,
            } if image.format == ImageFormat::Jpeg
                && image.width_px == 2
                && image.height_px == 1
        ));
    }

    #[test]
    fn emf_zero_alpha_alphablend_is_passive_noop_without_parsing_dib_payload() {
        let mut hidden_dib = b"NOT-A-DIB /JavaScript /EmbeddedFile".to_vec();
        hidden_dib.resize(64, 0x41);
        let records = [
            emf_rect_record(43, 0, 0, 30, 30),
            emf_alphablend_dib_record(10, 20, 44, 24, 0, 0, &hidden_dib),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_invisible_alpha_and_transparent_bitmap_transfers_are_passive_noops() {
        let mut hidden_dib =
            b"INVISIBLE-EMF-BITMAP-TRANSFER-DIB /JavaScript /EmbeddedFile".to_vec();
        hidden_dib.resize(64, 0x41);
        let records = [
            emf_rect_record(43, 0, 0, 30, 30),
            emf_alphablend_dib_record(10, 20, 0, 24, 0xff, 0, &hidden_dib),
            emf_transparentblt_dib_record(
                40,
                20,
                44,
                0,
                Color {
                    red: 255,
                    green: 0,
                    blue: 0,
                },
                &hidden_dib,
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_zero_source_alpha_and_transparent_bitmap_transfers_are_passive_noops() {
        let mut hidden_dib =
            b"ZERO-SOURCE-EMF-BITMAP-TRANSFER-DIB /JavaScript /EmbeddedFile".to_vec();
        hidden_dib.resize(64, 0x41);
        let mut alphablend = emf_alphablend_dib_record(10, 20, 44, 24, 0xff, 0, &hidden_dib);
        write_test_le_i32(&mut alphablend, 100, 0);
        let mut transparentblt = emf_transparentblt_dib_record(
            40,
            20,
            44,
            24,
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
            &hidden_dib,
        );
        write_test_le_i32(&mut transparentblt, 104, 0);
        let records = [
            emf_rect_record(43, 0, 0, 30, 30),
            alphablend,
            transparentblt,
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_zero_extent_and_zero_scan_rasters_are_passive_noops_without_parsing_payloads() {
        let mut hidden_dib =
            b"INVISIBLE-EMF-RASTER-PAYLOAD /JavaScript /EmbeddedFile /Launch".to_vec();
        hidden_dib.resize(64, 0x41);
        let records = [
            emf_rect_record(43, 0, 0, 30, 30),
            emf_bitblt_record(10, 20, 0, 24, WMF_PATCOPY_RASTER_OP, &hidden_dib),
            emf_stretchblt_record(40, 20, 44, 0, WMF_PATCOPY_RASTER_OP, &hidden_dib),
            emf_stretchdibits_record(70, 20, 0, 24, WMF_PATCOPY_RASTER_OP, &hidden_dib),
            emf_setdibitstodevice_dib_record(100, 20, 44, 24, 0, 0, &hidden_dib),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_zero_source_extent_stretch_rasters_are_passive_noops_without_parsing_payloads() {
        let mut hidden_dib =
            b"ZERO-SOURCE-EMF-RASTER-PAYLOAD /JavaScript /EmbeddedFile /Launch".to_vec();
        hidden_dib.resize(64, 0x41);
        let mut stretchblt =
            emf_stretchblt_dib_record(40, 20, 44, 24, WMF_SRCCOPY_RASTER_OP, &hidden_dib);
        write_test_le_i32(&mut stretchblt, 100, 0);
        let mut stretchdibits =
            emf_stretchdibits_dib_record(70, 20, 44, 24, WMF_SRCCOPY_RASTER_OP, &hidden_dib);
        write_test_le_i32(&mut stretchdibits, 44, 0);
        let records = [emf_rect_record(43, 0, 0, 30, 30), stretchblt, stretchdibits];
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn emf_transparentblt_keyed_color_becomes_passive_alpha_mask() {
        let indexed_dib = minimal_compressed_dib_with_payload(
            2,
            1,
            5,
            &minimal_low_bit_indexed_png(&[0, 1], 8, 2),
        );
        let indexed_image = parse_vector_raster_dib_image(&indexed_dib, 0, 0, 2, 1, false)
            .expect("indexed compressed DIB fixture should normalize");
        assert_eq!(indexed_image.format, ImageFormat::PngIndexed);
        assert_eq!(indexed_image.palette, vec![255, 0, 0, 0, 255, 0]);
        assert!(
            keyed_vector_raster_alpha_mask(
                indexed_image.format,
                &indexed_image.bytes,
                indexed_image.width_px,
                indexed_image.height_px,
                Color {
                    red: 255,
                    green: 0,
                    blue: 0,
                },
                &indexed_image.palette,
                indexed_image.alpha_mask,
            )
            .is_some()
        );
        let grayscale_dib =
            minimal_compressed_dib_with_payload(2, 1, 5, &minimal_grayscale_png(&[128, 255]));
        let grayscale_image = parse_vector_raster_dib_image(&grayscale_dib, 0, 0, 2, 1, false)
            .expect("grayscale compressed DIB fixture should normalize");
        assert_eq!(grayscale_image.format, ImageFormat::PngGrayscale);
        assert!(
            keyed_vector_raster_alpha_mask(
                grayscale_image.format,
                &grayscale_image.bytes,
                grayscale_image.width_px,
                grayscale_image.height_px,
                Color {
                    red: 128,
                    green: 128,
                    blue: 128,
                },
                &grayscale_image.palette,
                grayscale_image.alpha_mask,
            )
            .is_some()
        );
        let cropped_rgb_dib = minimal_compressed_dib_with_payload(
            3,
            1,
            5,
            &minimal_rgb_png(&[[255, 0, 0], [0, 255, 0], [0, 0, 255]]),
        );
        let cropped_rgb = parse_vector_raster_dib_image(&cropped_rgb_dib, 1, 0, 2, 1, false)
            .expect("cropped RGB compressed DIB should normalize");
        assert_eq!(cropped_rgb.format, ImageFormat::Png);
        assert_eq!(cropped_rgb.width_px, 2);
        assert_eq!(cropped_rgb.height_px, 1);
        let (_, _, _, cropped_rgb_scanlines) =
            unfiltered_png_scanlines(&cropped_rgb.bytes, 2, 1, 3).unwrap();
        assert_eq!(cropped_rgb_scanlines, vec![0, 0, 255, 0, 0, 0, 255]);
        let cropped_rgba_dib = minimal_compressed_dib_with_payload(
            3,
            1,
            5,
            &minimal_rgba_png(&[[255, 0, 0, 16], [0, 255, 0, 64], [0, 0, 255, 192]]),
        );
        let cropped_rgba = parse_vector_raster_dib_image(&cropped_rgba_dib, 1, 0, 2, 1, false)
            .expect("cropped RGBA compressed DIB should normalize");
        assert_eq!(cropped_rgba.format, ImageFormat::Png);
        assert_eq!(cropped_rgba.width_px, 2);
        assert_eq!(cropped_rgba.height_px, 1);
        let (_, _, _, cropped_rgba_scanlines) =
            unfiltered_png_scanlines(&cropped_rgba.bytes, 2, 1, 3).unwrap();
        assert_eq!(cropped_rgba_scanlines, vec![0, 0, 255, 0, 0, 0, 255]);
        let cropped_rgba_alpha = cropped_rgba
            .alpha_mask
            .expect("cropped RGBA alpha mask should be preserved");
        assert_eq!(
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&cropped_rgba_alpha.bytes, 3,)
                .unwrap(),
            vec![0, 64, 192]
        );
        let jpeg = minimal_jpeg_with_dimensions(2, 1);
        let jpeg_dib = minimal_compressed_dib_with_payload(2, 1, 4, &jpeg);
        let jpeg_image = parse_vector_raster_dib_image(&jpeg_dib, 0, 0, 2, 1, false)
            .expect("JPEG compressed DIB fixture should normalize");
        assert_eq!(jpeg_image.format, ImageFormat::Jpeg);
        assert_eq!(jpeg_image.bytes, jpeg);
        assert!(matches!(
            keyed_vector_raster_alpha_mask(
                jpeg_image.format,
                &jpeg_image.bytes,
                jpeg_image.width_px,
                jpeg_image.height_px,
                Color {
                    red: 255,
                    green: 0,
                    blue: 0,
                },
                &jpeg_image.palette,
                jpeg_image.alpha_mask,
            ),
            Some(None)
        ));
        let records = [
            emf_transparentblt_dib_record(
                18,
                26,
                42,
                20,
                Color {
                    red: 12,
                    green: 34,
                    blue: 56,
                },
                &minimal_24bit_dib_with_dimensions(2, 1),
            ),
            emf_transparentblt_dib_record(
                70,
                30,
                42,
                20,
                Color {
                    red: 255,
                    green: 0,
                    blue: 0,
                },
                &minimal_24bit_dib_with_dimensions(2, 1),
            ),
            emf_transparentblt_dib_record(
                94,
                34,
                42,
                20,
                Color {
                    red: 12,
                    green: 34,
                    blue: 56,
                },
                &minimal_compressed_dib_with_payload(
                    2,
                    1,
                    5,
                    &minimal_rgb_png(&[[255, 0, 0], [0, 255, 0]]),
                ),
            ),
            emf_transparentblt_dib_record(
                118,
                38,
                42,
                20,
                Color {
                    red: 255,
                    green: 0,
                    blue: 0,
                },
                &minimal_compressed_dib_with_payload(
                    2,
                    1,
                    5,
                    &minimal_rgb_png(&[[255, 0, 0], [0, 255, 0]]),
                ),
            ),
            emf_transparentblt_dib_record(
                142,
                42,
                42,
                20,
                Color {
                    red: 0,
                    green: 255,
                    blue: 0,
                },
                &minimal_compressed_dib_with_payload(
                    2,
                    1,
                    5,
                    &minimal_rgba_png(&[[255, 0, 0, 128], [0, 255, 0, 255]]),
                ),
            ),
            emf_transparentblt_dib_record(
                150,
                46,
                42,
                20,
                Color {
                    red: 255,
                    green: 0,
                    blue: 0,
                },
                &minimal_compressed_dib_with_payload(
                    2,
                    1,
                    5,
                    &minimal_low_bit_indexed_png(&[0, 1], 8, 2),
                ),
            ),
            emf_transparentblt_dib_record(
                154,
                50,
                42,
                20,
                Color {
                    red: 128,
                    green: 128,
                    blue: 128,
                },
                &minimal_compressed_dib_with_payload(2, 1, 5, &minimal_grayscale_png(&[128, 255])),
            ),
            emf_transparentblt_dib_record(
                10,
                54,
                42,
                20,
                Color {
                    red: 255,
                    green: 0,
                    blue: 0,
                },
                &minimal_compressed_dib_with_payload(2, 1, 4, &minimal_jpeg_with_dimensions(2, 1)),
            ),
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
        assert_eq!(image.vector_commands.len(), 8);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage {
                left: 18.0,
                top: 26.0,
                right: 60.0,
                bottom: 46.0,
                image,
            } if image.format == ImageFormat::Rgb8
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == vec![255, 0, 0, 0, 255, 0]
                && image.alpha_mask.is_none()
        ));
        let alpha_mask = match &image.vector_commands[1] {
            StaticImageVectorCommand::RasterImage { image, .. } => image
                .alpha_mask
                .as_ref()
                .expect("transparent color key alpha mask"),
            _ => panic!("expected keyed transparent raster image"),
        };
        assert_eq!(
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&alpha_mask.bytes, 3).unwrap(),
            vec![0, 0, 255]
        );
        assert!(matches!(
            &image.vector_commands[2],
            StaticImageVectorCommand::RasterImage {
                left: 94.0,
                top: 34.0,
                right: 136.0,
                bottom: 54.0,
                image,
            } if image.format == ImageFormat::Png
                && image.width_px == 2
                && image.height_px == 1
                && image.alpha_mask.is_none()
        ));
        let compressed_png_alpha_mask = match &image.vector_commands[3] {
            StaticImageVectorCommand::RasterImage { image, .. } => image
                .alpha_mask
                .as_ref()
                .expect("compressed PNG transparent color key alpha mask"),
            _ => panic!("expected compressed PNG keyed transparent raster image"),
        };
        assert_eq!(
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(
                &compressed_png_alpha_mask.bytes,
                3,
            )
            .unwrap(),
            vec![0, 0, 255]
        );
        let rgba_png_alpha_mask = match &image.vector_commands[4] {
            StaticImageVectorCommand::RasterImage { image, .. } => image
                .alpha_mask
                .as_ref()
                .expect("RGBA PNG transparent color key alpha mask"),
            _ => panic!("expected RGBA PNG keyed transparent raster image"),
        };
        assert_eq!(
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&rgba_png_alpha_mask.bytes, 3)
                .unwrap(),
            vec![0, 128, 0]
        );
        let indexed_png_alpha_mask = match &image.vector_commands[5] {
            StaticImageVectorCommand::RasterImage { image, .. } => image
                .alpha_mask
                .as_ref()
                .expect("indexed PNG transparent color key alpha mask"),
            _ => panic!("expected indexed PNG keyed transparent raster image"),
        };
        assert_eq!(
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(
                &indexed_png_alpha_mask.bytes,
                3,
            )
            .unwrap(),
            vec![0, 0, 255]
        );
        let grayscale_png_alpha_mask = match &image.vector_commands[6] {
            StaticImageVectorCommand::RasterImage { image, .. } => image
                .alpha_mask
                .as_ref()
                .expect("grayscale PNG transparent color key alpha mask"),
            _ => panic!("expected grayscale PNG keyed transparent raster image"),
        };
        assert_eq!(
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(
                &grayscale_png_alpha_mask.bytes,
                3,
            )
            .unwrap(),
            vec![0, 0, 255]
        );
        assert!(matches!(
            &image.vector_commands[7],
            StaticImageVectorCommand::RasterImage {
                left: 10.0,
                top: 54.0,
                right: 52.0,
                bottom: 74.0,
                image,
            } if image.format == ImageFormat::Jpeg
                && image.width_px == 2
                && image.height_px == 1
                && image.alpha_mask.is_none()
        ));
    }

    #[test]
    fn emf_blackness_and_whiteness_raster_transfers_become_passive_filled_rectangles() {
        let records = [
            emf_bitblt_record(
                10,
                20,
                30,
                15,
                WMF_BLACKNESS_RASTER_OP,
                b"BLACKNESS-SOURCE-PAYLOAD",
            ),
            emf_stretchblt_record(
                50,
                25,
                35,
                20,
                WMF_WHITENESS_RASTER_OP,
                b"WHITENESS-STRETCH-PAYLOAD",
            ),
            emf_stretchdibits_record(
                90,
                30,
                40,
                25,
                WMF_BLACKNESS_RASTER_OP,
                b"BLACKNESS-DIB-PAYLOAD",
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 3);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 10.0,
                top: 20.0,
                right: 40.0,
                bottom: 35.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 0,
                    green: 0,
                    blue: 0
                }),
                ..
            }
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle {
                left: 50.0,
                top: 25.0,
                right: 85.0,
                bottom: 45.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 255,
                    green: 255,
                    blue: 255
                }),
                ..
            }
        ));
        assert!(matches!(
            image.vector_commands[2],
            StaticImageVectorCommand::Rectangle {
                left: 90.0,
                top: 30.0,
                right: 130.0,
                bottom: 55.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 0,
                    green: 0,
                    blue: 0
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_move_to_and_line_to_records_become_passive_line_commands() {
        let records = [
            emf_point_record(27, 10, 10),
            emf_point_record(54, 70, 40),
            emf_point_record(54, 70, 40),
            emf_point_record(54, 200, 100),
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
        assert_eq!(
            image.vector_commands,
            vec![
                StaticImageVectorCommand::Line {
                    x1: 10.0,
                    y1: 10.0,
                    x2: 70.0,
                    y2: 40.0,
                    stroke_color: Some(Color::default()),
                    stroke_width: 1.0,
                    stroke_style: BorderStyle::Single,
                },
                StaticImageVectorCommand::Line {
                    x1: 70.0,
                    y1: 40.0,
                    x2: 160.0,
                    y2: 80.0,
                    stroke_color: Some(Color::default()),
                    stroke_width: 1.0,
                    stroke_style: BorderStyle::Single,
                },
            ]
        );
    }

    #[test]
    fn emf_polylineto_records_extend_from_current_position() {
        let records = [
            emf_point_record(27, 10, 10),
            emf_poly_record(6, &[(20, 20), (200, 100)]),
            emf_point_record(54, 20, 10),
            emf_point_record(27, 30, 30),
            emf_poly16_record(89, &[(40, 40), (200, 120)]),
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
        assert_eq!(
            image.vector_commands,
            vec![
                StaticImageVectorCommand::Polyline {
                    points: vec![(10.0, 10.0), (20.0, 20.0), (160.0, 80.0)],
                    stroke_color: Some(Color::default()),
                    stroke_width: 1.0,
                    stroke_style: BorderStyle::Single,
                },
                StaticImageVectorCommand::Line {
                    x1: 160.0,
                    y1: 80.0,
                    x2: 20.0,
                    y2: 10.0,
                    stroke_color: Some(Color::default()),
                    stroke_width: 1.0,
                    stroke_style: BorderStyle::Single,
                },
                StaticImageVectorCommand::Polyline {
                    points: vec![(30.0, 30.0), (40.0, 40.0), (160.0, 80.0)],
                    stroke_color: Some(Color::default()),
                    stroke_width: 1.0,
                    stroke_style: BorderStyle::Single,
                },
            ]
        );
    }

    #[test]
    fn emf_polybezier_records_become_passive_bezier_commands() {
        let records = [
            emf_poly_record(2, &[(10, 10), (30, 5), (50, 35), (70, 30)]),
            emf_point_record(27, 80, 20),
            emf_poly_record(5, &[(90, 10), (110, 50), (130, 40)]),
            emf_poly16_record(85, &[(20, 50), (40, 30), (60, 70), (80, 60)]),
            emf_point_record(27, 90, 50),
            emf_poly16_record(88, &[(100, 40), (120, 70), (200, 100)]),
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
        assert_eq!(
            image.vector_commands,
            vec![
                StaticImageVectorCommand::Bezier {
                    points: vec![(10.0, 10.0), (30.0, 5.0), (50.0, 35.0), (70.0, 30.0)],
                    stroke_color: Some(Color::default()),
                    stroke_width: 1.0,
                    stroke_style: BorderStyle::Single,
                },
                StaticImageVectorCommand::Bezier {
                    points: vec![(80.0, 20.0), (90.0, 10.0), (110.0, 50.0), (130.0, 40.0)],
                    stroke_color: Some(Color::default()),
                    stroke_width: 1.0,
                    stroke_style: BorderStyle::Single,
                },
                StaticImageVectorCommand::Bezier {
                    points: vec![(20.0, 50.0), (40.0, 30.0), (60.0, 70.0), (80.0, 60.0)],
                    stroke_color: Some(Color::default()),
                    stroke_width: 1.0,
                    stroke_style: BorderStyle::Single,
                },
                StaticImageVectorCommand::Bezier {
                    points: vec![(90.0, 50.0), (100.0, 40.0), (120.0, 70.0), (160.0, 80.0)],
                    stroke_color: Some(Color::default()),
                    stroke_width: 1.0,
                    stroke_style: BorderStyle::Single,
                },
            ]
        );
    }

    #[test]
    fn emf_polybezier_with_partial_segments_becomes_blank_passive_vector() {
        let records = [emf_poly_record(
            2,
            &[(10, 10), (30, 5), (50, 35), (70, 30), (80, 40)],
        )];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Image(image)
                if image.format == ImageFormat::WmfVector
                    && image.bytes.is_empty()
                    && image.vector_commands.is_empty()
        ));
    }

    #[test]
    fn emf_patcopy_stretchdibits_records_become_passive_filled_rectangles() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 180,
                    green: 90,
                    blue: 30,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_stretchdibits_record(
                20,
                15,
                60,
                30,
                WMF_PATCOPY_RASTER_OP,
                b"STRETCHDIBITS-SOURCE-PAYLOAD",
            ),
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 180,
                    green: 90,
                    blue: 30
                }),
                ..
            }
        ));
    }

    #[test]
    fn emf_srccopy_stretchdibits_record_becomes_passive_raster_image() {
        let records = [emf_stretchdibits_dib_record(
            20,
            15,
            60,
            30,
            WMF_SRCCOPY_RASTER_OP,
            &minimal_24bit_dib_with_dimensions(2, 1),
        )];
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                image,
            } if image.format == ImageFormat::Rgb8
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == vec![255, 0, 0, 0, 255, 0]
        ));
    }

    #[test]
    fn emf_blank_destination_srcand_stretchdibits_record_becomes_passive_raster_image() {
        let records = [emf_stretchdibits_dib_record(
            20,
            15,
            60,
            30,
            WMF_SRCAND_RASTER_OP,
            &minimal_24bit_dib_with_dimensions(2, 1),
        )];
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                image,
            } if image.format == ImageFormat::Rgb8
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == vec![255, 0, 0, 0, 255, 0]
        ));
    }

    #[test]
    fn emf_blank_destination_srcinvert_stretchdibits_record_becomes_passive_inverted_raster_image()
    {
        let records = [emf_stretchdibits_dib_record(
            20,
            15,
            60,
            30,
            WMF_SRCINVERT_RASTER_OP,
            &minimal_24bit_dib_with_dimensions(2, 1),
        )];
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                image,
            } if image.format == ImageFormat::Rgb8
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == vec![0, 255, 255, 255, 0, 255]
        ));
    }

    #[test]
    fn emf_blank_srcinvert_bi_png_stretchdibits_inverts_scanlines() {
        let png = minimal_rgb_png(&[[10, 20, 30], [200, 210, 220]]);
        let dib = minimal_compressed_dib_with_payload(2, 1, 5, &png);
        let records = [emf_stretchdibits_dib_record(
            20,
            15,
            60,
            30,
            WMF_SRCINVERT_RASTER_OP,
            &dib,
        )];
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
        assert_eq!(output.diagnostics.len(), 0);
        let StaticImageVectorCommand::RasterImage { image, .. } = &image.vector_commands[0] else {
            panic!("expected passive raster command");
        };
        assert_eq!(image.format, ImageFormat::Png);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        let scanlines = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 7)
            .expect("inverted PNG scanlines");
        assert_eq!(scanlines, vec![0, 245, 235, 225, 55, 45, 35]);
    }

    #[test]
    fn emf_notsrccopy_stretchdibits_after_paint_becomes_passive_inverted_raster_image() {
        let records = [
            emf_rect_record(43, 0, 0, 30, 30),
            emf_stretchdibits_dib_record(
                20,
                15,
                60,
                30,
                WMF_NOTSRCCOPY_RASTER_OP,
                &minimal_24bit_dib_with_dimensions(2, 1),
            ),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
        assert!(matches!(
            &image.vector_commands[1],
            StaticImageVectorCommand::RasterImage {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                image,
            } if image.format == ImageFormat::Rgb8
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == vec![0, 255, 255, 255, 0, 255]
        ));
    }

    #[test]
    fn emf_notsrccopy_stretchdibits_bi_png_record_becomes_passive_inverted_raster_image() {
        let png = minimal_rgb_png(&[[10, 20, 30], [200, 210, 220]]);
        let dib = minimal_compressed_dib_with_payload(2, 1, 5, &png);
        let records = [emf_stretchdibits_dib_record(
            20,
            15,
            60,
            30,
            WMF_NOTSRCCOPY_RASTER_OP,
            &dib,
        )];
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
        assert_eq!(output.diagnostics.len(), 0);
        let StaticImageVectorCommand::RasterImage { image, .. } = &image.vector_commands[0] else {
            panic!("expected passive raster command");
        };
        assert_eq!(image.format, ImageFormat::Png);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        let scanlines = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 7)
            .expect("inverted PNG scanlines");
        assert_eq!(scanlines, vec![0, 245, 235, 225, 55, 45, 35]);
    }

    #[test]
    fn emf_notsrccopy_stretchdibits_bi_jpeg_record_becomes_passive_inverted_raster_image() {
        let jpeg = minimal_jpeg_with_dimensions(2, 1);
        let dib = minimal_compressed_dib_with_payload(2, 1, 4, &jpeg);
        let records = [emf_stretchdibits_dib_record(
            20,
            15,
            60,
            30,
            WMF_NOTSRCCOPY_RASTER_OP,
            &dib,
        )];
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                image,
            } if image.format == ImageFormat::JpegInverted
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == jpeg
        ));
    }

    #[test]
    fn emf_srcand_stretchdibits_after_paint_is_skipped_as_backdrop_dependent() {
        let records = [
            emf_rect_record(43, 0, 0, 30, 30),
            emf_stretchdibits_dib_record(
                20,
                15,
                60,
                30,
                WMF_SRCAND_RASTER_OP,
                &minimal_24bit_dib_with_dimensions(2, 1),
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected partial passive EMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("1 unsupported record(s) skipped")
        }));
    }

    #[test]
    fn emf_srcinvert_stretchdibits_after_paint_is_skipped_as_backdrop_dependent() {
        let records = [
            emf_rect_record(43, 0, 0, 30, 30),
            emf_stretchdibits_dib_record(
                20,
                15,
                60,
                30,
                WMF_SRCINVERT_RASTER_OP,
                &minimal_24bit_dib_with_dimensions(2, 1),
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected partial passive EMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle { .. }
        ));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("1 unsupported record(s) skipped")
        }));
    }

    #[test]
    fn emf_srccopy_stretchdibits_bi_jpeg_record_becomes_passive_raster_image() {
        let jpeg = minimal_jpeg_with_dimensions(2, 1);
        let dib = minimal_compressed_dib_with_payload(2, 1, 4, &jpeg);
        let records = [emf_stretchdibits_dib_record(
            20,
            15,
            60,
            30,
            WMF_SRCCOPY_RASTER_OP,
            &dib,
        )];
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                image,
            } if image.format == ImageFormat::Jpeg
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == jpeg
        ));
    }

    #[test]
    fn emf_setdibits_full_frame_bi_jpeg_record_becomes_passive_raster_image() {
        let jpeg = minimal_jpeg_with_dimensions(2, 1);
        let dib = minimal_compressed_dib_with_payload(2, 1, 4, &jpeg);
        let records = [emf_setdibitstodevice_dib_record(20, 15, 2, 1, 0, 1, &dib)];
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage {
                left: 20.0,
                top: 15.0,
                right: 22.0,
                bottom: 16.0,
                image,
            } if image.format == ImageFormat::Jpeg
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == jpeg
        ));
    }

    #[test]
    fn emf_setdibits_partial_scan_bi_jpeg_record_becomes_passive_clipped_raster_image() {
        let jpeg = minimal_jpeg_with_dimensions(3, 2);
        let dib = minimal_compressed_dib_with_payload(3, 2, 4, &jpeg);
        let records = [emf_setdibitstodevice_dib_record(20, 15, 3, 2, 0, 1, &dib)];
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
        assert_eq!(image.vector_commands.len(), 4);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::SaveState
        ));
        assert_eq!(
            image.vector_commands[1],
            StaticImageVectorCommand::ClipRect {
                left: 20.0,
                top: 15.0,
                right: 23.0,
                bottom: 16.0,
            }
        );
        assert!(matches!(
            &image.vector_commands[2],
            StaticImageVectorCommand::RasterImage {
                left: 20.0,
                top: 14.0,
                right: 23.0,
                bottom: 16.0,
                image,
            } if image.format == ImageFormat::Jpeg
                && image.width_px == 3
                && image.height_px == 2
                && image.bytes == jpeg
                && image.alpha_mask.is_none()
        ));
        assert!(matches!(
            image.vector_commands[3],
            StaticImageVectorCommand::RestoreState
        ));
    }

    #[test]
    fn emf_srccopy_stretchdibits_source_crop_becomes_passive_raster_image() {
        let dib = minimal_24bit_dib_with_rgb_pixels(
            3,
            2,
            &[
                [10, 20, 30],
                [40, 50, 60],
                [70, 80, 90],
                [100, 110, 120],
                [130, 140, 150],
                [160, 170, 180],
            ],
        );
        let mut record = emf_stretchdibits_dib_record(20, 15, 60, 30, WMF_SRCCOPY_RASTER_OP, &dib);
        write_test_le_i32(&mut record, 32, 1);
        write_test_le_i32(&mut record, 36, 1);
        write_test_le_i32(&mut record, 40, 2);
        write_test_le_i32(&mut record, 44, 1);
        let records = [record];
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
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
                image,
            } if image.format == ImageFormat::Rgb8
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == vec![130, 140, 150, 160, 170, 180]
        ));
    }

    #[test]
    fn emf_srccopy_stretchdibits_jpeg_source_crop_becomes_passive_clipped_raster_image() {
        let jpeg = minimal_jpeg_with_dimensions(3, 2);
        let dib = minimal_compressed_dib_with_payload(3, 2, 4, &jpeg);
        let mut record = emf_stretchdibits_dib_record(20, 15, 60, 30, WMF_SRCCOPY_RASTER_OP, &dib);
        write_test_le_i32(&mut record, 32, 1);
        write_test_le_i32(&mut record, 36, 0);
        write_test_le_i32(&mut record, 40, 1);
        write_test_le_i32(&mut record, 44, 2);
        let records = [record];
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
        assert_eq!(image.vector_commands.len(), 4);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::SaveState
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::ClipRect {
                left: 20.0,
                top: 15.0,
                right: 80.0,
                bottom: 45.0,
            }
        ));
        assert!(matches!(
            &image.vector_commands[2],
            StaticImageVectorCommand::RasterImage {
                left: -40.0,
                top: 15.0,
                right: 140.0,
                bottom: 45.0,
                image,
            } if image.format == ImageFormat::Jpeg
                && image.width_px == 3
                && image.height_px == 2
                && image.bytes == jpeg
                && image.alpha_mask.is_none()
        ));
        assert!(matches!(
            image.vector_commands[3],
            StaticImageVectorCommand::RestoreState
        ));
    }

    #[test]
    fn emf_empty_excludecliprect_is_passive_noop() {
        let records = [
            emf_rect_record(29, 20, 20, 20, 60),
            emf_rect_record(43, 0, 0, 160, 80),
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
    fn emf_window_and_viewport_records_map_passive_coordinates() {
        let records = [
            emf_size_record(9, 320, 160),
            emf_point_record(10, 10, 20),
            emf_size_record(11, 160, 80),
            emf_point_record(12, 5, 3),
            emf_rect_record(43, 10, 20, 170, 100),
            emf_point_record(27, 10, 20),
            emf_point_record(54, 330, 180),
            emf_poly_record(4, &[(10, 20), (170, 100), (330, 180)]),
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
        assert_eq!(
            image.vector_commands,
            vec![
                StaticImageVectorCommand::Rectangle {
                    left: 5.0,
                    top: 3.0,
                    right: 85.0,
                    bottom: 43.0,
                    stroke_color: Some(Color::default()),
                    stroke_width: 1.0,
                    stroke_style: BorderStyle::Single,
                    fill_pattern: ShadingPattern::None,
                    fill_color: None,
                },
                StaticImageVectorCommand::Line {
                    x1: 5.0,
                    y1: 3.0,
                    x2: 160.0,
                    y2: 80.0,
                    stroke_color: Some(Color::default()),
                    stroke_width: 1.0,
                    stroke_style: BorderStyle::Single,
                },
                StaticImageVectorCommand::Polyline {
                    points: vec![(5.0, 3.0), (85.0, 43.0), (160.0, 80.0)],
                    stroke_color: Some(Color::default()),
                    stroke_width: 1.0,
                    stroke_style: BorderStyle::Single,
                },
            ]
        );
    }

    #[test]
    fn emf_scale_window_and_viewport_records_map_passive_coordinates() {
        let records = [
            emf_scale_record(31, 2, 1, 2, 1),
            emf_rect_record(43, 10, 10, 50, 30),
            emf_scale_record(32, 2, 1, 2, 1),
            emf_rect_record(43, 10, 10, 50, 30),
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
        assert_eq!(
            image.vector_commands,
            vec![
                StaticImageVectorCommand::Rectangle {
                    left: 20.0,
                    top: 20.0,
                    right: 100.0,
                    bottom: 60.0,
                    stroke_color: Some(Color::default()),
                    stroke_width: 2.0,
                    stroke_style: BorderStyle::Single,
                    fill_pattern: ShadingPattern::None,
                    fill_color: None,
                },
                StaticImageVectorCommand::Rectangle {
                    left: 10.0,
                    top: 10.0,
                    right: 50.0,
                    bottom: 30.0,
                    stroke_color: Some(Color::default()),
                    stroke_width: 1.0,
                    stroke_style: BorderStyle::Single,
                    fill_pattern: ShadingPattern::None,
                    fill_color: None,
                },
            ]
        );
    }

    #[test]
    fn emf_scale_records_with_zero_factor_are_passive_noops() {
        let records = [
            emf_scale_record(31, 0, 1, 1, 1),
            emf_scale_record(32, 1, 1, 0, 1),
            emf_rect_record(43, 10, 20, 170, 100),
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
    fn emf_arc_chord_and_pie_records_become_passive_vector_commands() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 40,
                    green: 120,
                    blue: 200,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_arc_record(45, 20, 10, 100, 70, 100, 40, 60, 70),
            emf_arc_record(46, 20, 10, 100, 70, 100, 40, 60, 70),
            emf_arc_record(47, 20, 10, 100, 70, 100, 40, 60, 70),
            emf_u32_record(57, 2),
            emf_arc_record(55, 20, 10, 100, 70, 100, 40, 60, 70),
            emf_point_record(54, 120, 70),
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
        assert_eq!(image.vector_commands.len(), 5);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::Polyline { points, .. }
                if points.first() == Some(&(100.0, 40.0))
                    && points.last() == Some(&(60.0, 70.0))
                    && points.len() <= MAX_PASSIVE_WMF_POINTS_PER_RECORD
        ));
        assert!(matches!(
            &image.vector_commands[1],
            StaticImageVectorCommand::Polygon { points, .. }
                if points.first() == Some(&(100.0, 40.0))
                    && points.last() == Some(&(60.0, 70.0))
                    && points.len() <= MAX_PASSIVE_WMF_POINTS_PER_RECORD
        ));
        assert!(matches!(
            &image.vector_commands[2],
            StaticImageVectorCommand::Polygon { points, .. }
                if points.first() == Some(&(60.0, 40.0))
                    && points.get(1) == Some(&(100.0, 40.0))
                    && points.last() == Some(&(60.0, 70.0))
                    && points.len() <= MAX_PASSIVE_WMF_POINTS_PER_RECORD
        ));
        assert!(matches!(
            &image.vector_commands[3],
            StaticImageVectorCommand::Polyline { points, .. }
                if points.first() == Some(&(100.0, 40.0))
                    && points.last() == Some(&(60.0, 70.0))
                    && points.len() <= MAX_PASSIVE_WMF_POINTS_PER_RECORD
        ));
        assert!(matches!(
            image.vector_commands[4],
            StaticImageVectorCommand::Line {
                x1: 60.0,
                y1: 70.0,
                x2: 120.0,
                y2: 70.0,
                ..
            }
        ));
    }

    #[test]
    fn emf_arcto_inside_fillpath_becomes_passive_filled_polygon() {
        let records = [
            emf_create_brush_record(
                3,
                0,
                Color {
                    red: 210,
                    green: 120,
                    blue: 40,
                },
                0,
            ),
            emf_select_object_record(3),
            emf_unknown_record(59),
            emf_point_record(27, 60, 70),
            emf_arc_record(55, 20, 10, 100, 70, 100, 40, 60, 70),
            emf_unknown_record(61),
            emf_unknown_record(60),
            emf_unknown_record(62),
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
            &image.vector_commands[0],
            StaticImageVectorCommand::Polygon {
                points,
                stroke_color: None,
                fill_color: Some(Color { red: 210, green: 120, blue: 40 }),
                ..
            } if points.first() == Some(&(60.0, 70.0))
                && points.last() == Some(&(60.0, 70.0))
                && points.len() > 4
                && points.len() <= MAX_PASSIVE_WMF_POINTS_PER_RECORD
        ));
    }

    #[test]
    fn emf_arc_records_with_degenerate_bounds_are_passive_noops() {
        let records = [
            emf_arc_record(45, 20, 10, 20, 70, 100, 40, 60, 70),
            emf_arc_record(46, 20, 10, 100, 10, 100, 40, 60, 70),
            emf_arc_record(47, 20, 10, 20, 70, 100, 40, 60, 70),
            emf_arc_record(55, 20, 10, 100, 10, 100, 40, 60, 70),
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
            StaticImageVectorCommand::Rectangle {
                left: 0.0,
                top: 0.0,
                right: 60.0,
                bottom: 30.0,
                ..
            }
        ));
    }

    #[test]
    fn emf_polydraw_records_become_passive_vector_commands() {
        let records = [
            emf_polydraw_record(
                56,
                &[
                    (10, 10),
                    (50, 10),
                    (50, 40),
                    (60, 10),
                    (70, 5),
                    (80, 40),
                    (90, 30),
                ],
                &[0x06, 0x02, 0x03, 0x06, 0x04, 0x04, 0x05],
            ),
            emf_point_record(54, 100, 10),
            emf_polydraw16_record(92, &[(20, 50), (60, 50), (60, 70)], &[0x06, 0x02, 0x02]),
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
        assert_eq!(image.vector_commands.len(), 5);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::Polyline { points, .. }
                if points == &vec![(10.0, 10.0), (50.0, 10.0), (50.0, 40.0), (10.0, 10.0)]
        ));
        assert!(matches!(
            &image.vector_commands[1],
            StaticImageVectorCommand::Bezier { points, .. }
                if points == &vec![(60.0, 10.0), (70.0, 5.0), (80.0, 40.0), (90.0, 30.0)]
        ));
        assert!(matches!(
            &image.vector_commands[2],
            StaticImageVectorCommand::Polyline { points, .. }
                if points == &vec![(90.0, 30.0), (60.0, 10.0)]
        ));
        assert!(matches!(
            image.vector_commands[3],
            StaticImageVectorCommand::Line {
                x1: 60.0,
                y1: 10.0,
                x2: 100.0,
                y2: 10.0,
                ..
            }
        ));
        assert!(matches!(
            &image.vector_commands[4],
            StaticImageVectorCommand::Polyline { points, .. }
                if points == &vec![(20.0, 50.0), (60.0, 50.0), (60.0, 70.0)]
        ));
    }

    #[test]
    fn emf_polydraw_records_with_partial_bezier_become_blank_passive_vectors() {
        let records = [emf_polydraw_record(
            56,
            &[(10, 10), (30, 20), (40, 30)],
            &[0x06, 0x04, 0x04],
        )];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Image(image)
                if image.format == ImageFormat::WmfVector
                    && image.bytes.is_empty()
                    && image.vector_commands.is_empty()
        ));
    }

    #[test]
    fn emf_anglearc_records_become_passive_polyline_commands() {
        let records = [
            emf_point_record(27, 10, 40),
            emf_anglearc_record(60, 40, 40, 0.0, 90.0),
            emf_point_record(54, 60, 70),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::Polyline { points, .. }
                if points.first() == Some(&(10.0, 40.0))
                    && points.get(1) == Some(&(100.0, 40.0))
                    && points.last() == Some(&(60.0, 0.0))
                    && points.len() <= MAX_PASSIVE_WMF_POINTS_PER_RECORD
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Line {
                x1: 60.0,
                y1: 0.0,
                x2: 60.0,
                y2: 70.0,
                ..
            }
        ));
    }

    #[test]
    fn emf_anglearc_records_with_zero_radius_draw_line_to_center() {
        let records = [
            emf_point_record(27, 10, 40),
            emf_anglearc_record(60, 40, 0, 0.0, 90.0),
            emf_point_record(54, 60, 70),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::Polyline { points, .. }
                if points == &vec![(10.0, 40.0), (60.0, 40.0)]
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Line {
                x1: 60.0,
                y1: 40.0,
                x2: 60.0,
                y2: 70.0,
                ..
            }
        ));
    }

    #[test]
    fn emf_anglearc_records_with_zero_sweep_draw_line_to_arc_start() {
        let records = [
            emf_point_record(27, 10, 40),
            emf_anglearc_record(60, 40, 40, 0.0, 0.0),
            emf_point_record(54, 60, 70),
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
        assert_eq!(image.vector_commands.len(), 2);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::Polyline { points, .. }
                if points == &vec![(10.0, 40.0), (100.0, 40.0)]
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Line {
                x1: 100.0,
                y1: 40.0,
                x2: 60.0,
                y2: 70.0,
                ..
            }
        ));
    }

    #[test]
    fn emf_coordinate_records_with_zero_extent_are_passive_noops() {
        let records = [
            emf_size_record(9, 0, 160),
            emf_size_record(11, 160, 0),
            emf_rect_record(43, 10, 20, 170, 100),
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
    fn emf_savedc_and_restoredc_restore_passive_state() {
        let records = [
            emf_create_pen_record(
                2,
                0,
                2,
                Color {
                    red: 200,
                    green: 20,
                    blue: 20,
                },
            ),
            emf_select_object_record(2),
            emf_point_record(12, 5, 0),
            emf_unknown_record(33),
            emf_create_pen_record(
                3,
                1,
                4,
                Color {
                    red: 20,
                    green: 40,
                    blue: 200,
                },
            ),
            emf_select_object_record(3),
            emf_point_record(12, 20, 0),
            emf_point_record(27, 0, 10),
            emf_point_record(54, 40, 10),
            emf_i32_record(34, -1),
            emf_point_record(27, 0, 20),
            emf_point_record(54, 40, 20),
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
        assert_eq!(image.vector_commands.len(), 4);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::SaveState
        ));
        assert!(matches!(
            image.vector_commands[2],
            StaticImageVectorCommand::RestoreState
        ));
        let line_commands: Vec<_> = image
            .vector_commands
            .iter()
            .filter(|command| matches!(command, StaticImageVectorCommand::Line { .. }))
            .collect();
        assert_eq!(line_commands.len(), 2);
        assert!(matches!(
            line_commands[0],
            StaticImageVectorCommand::Line {
                x1: 20.0,
                y1: 10.0,
                x2: 60.0,
                y2: 10.0,
                stroke_color: Some(Color {
                    red: 20,
                    green: 40,
                    blue: 200
                }),
                stroke_width: 4.0,
                stroke_style: BorderStyle::Dashed,
            }
        ));
        assert!(matches!(
            line_commands[1],
            StaticImageVectorCommand::Line {
                x1: 5.0,
                y1: 20.0,
                x2: 45.0,
                y2: 20.0,
                stroke_color: Some(Color {
                    red: 200,
                    green: 20,
                    blue: 20
                }),
                stroke_width: 2.0,
                stroke_style: BorderStyle::Single,
            }
        ));
    }

    #[test]
    fn emf_restoredc_restores_passive_clip_state() {
        let records = [
            emf_unknown_record(33),
            emf_rect_record(30, 20, 20, 60, 60),
            emf_rect_record(43, 0, 0, 160, 80),
            emf_i32_record(34, -1),
            emf_rect_record(43, 80, 0, 150, 70),
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
        assert_eq!(image.vector_commands.len(), 5);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::SaveState
        ));
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::ClipRect { .. }
        ));
        assert!(matches!(
            image.vector_commands[3],
            StaticImageVectorCommand::RestoreState
        ));
        assert!(matches!(
            image.vector_commands[4],
            StaticImageVectorCommand::Rectangle {
                left: 80.0,
                top: 0.0,
                right: 150.0,
                bottom: 70.0,
                ..
            }
        ));
    }

    #[test]
    fn emf_restoredc_without_saved_state_is_passive_noop() {
        let records = [
            emf_i32_record(34, -1),
            emf_rect_record(43, 10, 20, 170, 100),
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
    fn emf_poly_records_with_excessive_points_become_passive_placeholders() {
        let points = vec![(1, 1); MAX_PASSIVE_WMF_POINTS_PER_RECORD + 1];
        let records = [emf_poly_record(4, &points)];
        let input = format!(
            r"{{\rtf1{{\pict\emfblip {}}}}}",
            bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive EMF placeholder"),
        };
        assert_eq!(image.format, ImageFormat::Placeholder);
        assert!(image.bytes.is_empty());
        assert!(image.vector_commands.is_empty());
        assert_eq!(image.width_px, 160);
        assert_eq!(image.height_px, 80);
    }

    #[test]
    fn emf_vector_parser_ignores_records_after_declared_byte_range() {
        let records = [
            emf_rect_record(43, 10, 10, 70, 40),
            emf_rect_record(42, 80, 20, 150, 70),
        ];
        let mut emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
        write_test_le_u32(&mut emf, 48, 112);
        write_test_le_u32(&mut emf, 52, 2);
        let input = format!(r"{{\rtf1{{\pict\emfblip {}}}}}", bytes_to_hex(&emf));
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive EMF placeholder"),
        };
        assert_eq!(image.format, ImageFormat::Placeholder);
        assert!(image.bytes.is_empty());
        assert!(image.vector_commands.is_empty());
        assert_eq!(image.width_px, 160);
        assert_eq!(image.height_px, 80);
    }

    #[test]
    fn emf_vector_parser_ignores_records_after_declared_record_count() {
        let records = [
            emf_rect_record(43, 10, 10, 70, 40),
            emf_rect_record(42, 80, 20, 150, 70),
        ];
        let mut emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
        write_test_le_u32(&mut emf, 52, 2);
        let input = format!(r"{{\rtf1{{\pict\emfblip {}}}}}", bytes_to_hex(&emf));
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive EMF placeholder"),
        };
        assert_eq!(image.format, ImageFormat::Placeholder);
        assert!(image.bytes.is_empty());
        assert!(image.vector_commands.is_empty());
        assert_eq!(image.width_px, 160);
        assert_eq!(image.height_px, 80);
    }
