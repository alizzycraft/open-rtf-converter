    #[test]
    fn wmf_polybezier_with_partial_segments_becomes_blank_passive_vector() {
        let wmf_hex = concat!(
            "0100090000031b00000001000a0000000000",
            "050000000c026400c800",
            "0a000000051003000a000a001e00050046003200",
            "030000000000",
        );
        let output = parse_rtf(&format!(r"{{\rtf1{{\pict\wmetafile8 {wmf_hex}}}}}")).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Image(image)
                if image.format == ImageFormat::WmfVector
                    && image.bytes.is_empty()
                    && image.vector_commands.is_empty()
        ));
    }

    #[test]
    fn wmf_incomplete_bezier_records_are_passive_noops_before_later_drawing() {
        let records = [
            wmf_point_list_record(0x1005, &[(10, 10), (30, 5), (70, 50)]),
            wmf_yx_record(0x0214, 20, 20),
            wmf_point_list_record(0x1004, &[(30, 5), (70, 50)]),
            wmf_bounds_record(0x041b, 0, 0, 80, 40),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8\picw160\pich80\picwgoal2160\pichgoal1080 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
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
    fn strict_spec_wmf_polypolygon_preserves_compound_fill_rule_as_passive_path() {
        let wmf_hex = concat!(
            "010009000003360000000100160000000000",
            "050000000c026400c800",
            "07000000fc020000dcdcdc000000",
            "040000002d010000",
            "0400000006010200",
            "160000003805020004000400",
            "0a000a008c000a008c0046000a004600",
            "32001900640019006400370032003700",
            "030000000000",
        );
        let output =
            parse_rtf_strict(&format!(r"{{\rtf1{{\pict\wmetafile8 {wmf_hex}}}}}")).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
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
                fill_color: Some(Color { red: 220, green: 220, blue: 220 }),
                ..
            } if segments.iter().any(|segment| matches!(
                segment,
                StaticImageVectorPathSegment::MoveTo(50.0, 25.0)
            ))
        ));
    }

    #[test]
    fn strict_spec_wmf_patcopy_stretchblt_records_become_passive_filled_rectangles() {
        let wmf_hex = concat!(
            "0100090000032900000001000d0000000000",
            "050000000c026400c800",
            "07000000fc020000285078000000",
            "040000002d010000",
            "0d000000230b2100f00000000000000000002800500019000f00",
            "030000000000",
        );
        let input = format!(r"{{\rtf1{{\pict\wmetafile8 {wmf_hex}}}}}");
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
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 15.0,
                top: 25.0,
                right: 95.0,
                bottom: 65.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 40,
                    green: 80,
                    blue: 120
                }),
                ..
            }
        ));
    }

    #[test]
    fn strict_spec_wmf_patcopy_bitblt_records_become_passive_filled_rectangles() {
        let wmf_hex = concat!(
            "0100090000032700000001000b0000000000",
            "050000000c026400c800",
            "07000000fc0200001e6eb4000000",
            "040000002d010000",
            "0b00000022092100f000000000003c00460014000a00",
            "030000000000",
        );
        let input = format!(r"{{\rtf1{{\pict\wmetafile8 {wmf_hex}}}}}");
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
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 10.0,
                top: 20.0,
                right: 80.0,
                bottom: 80.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 30,
                    green: 110,
                    blue: 180
                }),
                ..
            }
        ));
    }

    #[test]
    fn strict_spec_wmf_patcopy_dibbitblt_records_become_passive_filled_rectangles() {
        let wmf_hex = concat!(
            "0100090000032800000001000c0000000000",
            "050000000c026400c800",
            "07000000fc020000785028000000",
            "040000002d010000",
            "0c00000040092100f000000000003c00460014000a000000",
            "030000000000",
        );
        let output =
            parse_rtf_strict(&format!(r"{{\rtf1{{\pict\wmetafile8 {wmf_hex}}}}}")).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 10.0,
                top: 20.0,
                right: 80.0,
                bottom: 80.0,
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
    fn strict_spec_wmf_patcopy_dibstretchblt_records_become_passive_filled_rectangles() {
        let wmf_hex = concat!(
            "0100090000032900000001000d0000000000",
            "050000000c026400c800",
            "07000000fc0200002846a0000000",
            "040000002d010000",
            "0d000000410b2100f00000000000000000002800500019000f00",
            "030000000000",
        );
        let input = format!(r"{{\rtf1{{\pict\wmetafile8 {wmf_hex}}}}}");
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
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 15.0,
                top: 25.0,
                right: 95.0,
                bottom: 65.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 40,
                    green: 70,
                    blue: 160
                }),
                ..
            }
        ));
    }

    #[test]
    fn wmf_zero_extent_and_zero_scan_rasters_are_passive_noops_without_parsing_payloads() {
        let mut hidden_dib =
            b"INVISIBLE-WMF-RASTER-PAYLOAD /JavaScript /EmbeddedFile /Launch".to_vec();
        hidden_dib.resize(64, 0x41);
        let records = [
            wmf_create_brush_record(Color {
                red: 30,
                green: 90,
                blue: 150,
            }),
            wmf_select_object_record(0),
            wmf_bounds_record(0x041b, 0, 0, 30, 30),
            wmf_patblt_record(10, 20, 0, 24, WMF_PATCOPY_RASTER_OP),
            wmf_bitblt_record(40, 20, 44, 0, WMF_PATCOPY_RASTER_OP, &hidden_dib),
            wmf_stretchblt_record(70, 20, 0, 24, WMF_PATCOPY_RASTER_OP, &hidden_dib),
            wmf_dibbitblt_record(100, 20, 44, 0, WMF_PATCOPY_RASTER_OP, &hidden_dib),
            wmf_dibstretchblt_record(10, 50, 0, 24, WMF_PATCOPY_RASTER_OP, &hidden_dib),
            wmf_stretchdib_dib_record(40, 50, 44, 0, WMF_PATCOPY_RASTER_OP, &hidden_dib),
            wmf_setdib_to_dev_record(70, 50, 44, 24, 0, 0, &hidden_dib),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(160, 100, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
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
    fn wmf_zero_source_extent_stretch_rasters_are_passive_noops_without_parsing_payloads() {
        let mut hidden_dib =
            b"ZERO-SOURCE-WMF-RASTER-PAYLOAD /JavaScript /EmbeddedFile /Launch".to_vec();
        hidden_dib.resize(64, 0x41);
        let records = [
            wmf_create_brush_record(Color {
                red: 30,
                green: 90,
                blue: 150,
            }),
            wmf_select_object_record(0),
            wmf_bounds_record(0x041b, 0, 0, 30, 30),
            wmf_stretchblt_record(40, 20, 44, 24, WMF_SRCCOPY_RASTER_OP, &hidden_dib),
            wmf_dibstretchblt_record_with_source(70, 20, 44, 24, 0, 0, 0, 24, &hidden_dib),
            wmf_stretchdib_dib_record_with_source(100, 20, 44, 24, 0, 0, 44, 0, &hidden_dib),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
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
    fn strict_spec_wmf_blackness_and_whiteness_transfers_become_passive_filled_rectangles() {
        let cases = [
            (
                concat!(
                    "010009000003250000000100090000000000",
                    "050000000c026400c800",
                    "07000000fc020000ff0000000000",
                    "040000002d010000",
                    "090000001d06420000001e00320014002800",
                    "030000000000",
                ),
                (40.0, 20.0, 90.0, 50.0),
                Color {
                    red: 0,
                    green: 0,
                    blue: 0,
                },
            ),
            (
                concat!(
                    "0100090000032700000001000b0000000000",
                    "050000000c026400c800",
                    "07000000fc0200001e6eb4000000",
                    "040000002d010000",
                    "0b00000022096200ff00000000003c00460014000a00",
                    "030000000000",
                ),
                (10.0, 20.0, 80.0, 80.0),
                Color {
                    red: 255,
                    green: 255,
                    blue: 255,
                },
            ),
            (
                concat!(
                    "0100090000032900000001000d0000000000",
                    "050000000c026400c800",
                    "07000000fc020000285078000000",
                    "040000002d010000",
                    "0d000000230b4200000000000000000000002800500019000f00",
                    "030000000000",
                ),
                (15.0, 25.0, 95.0, 65.0),
                Color {
                    red: 0,
                    green: 0,
                    blue: 0,
                },
            ),
            (
                concat!(
                    "0100090000032800000001000c0000000000",
                    "050000000c026400c800",
                    "07000000fc020000785028000000",
                    "040000002d010000",
                    "0c00000040096200ff00000000003c00460014000a000000",
                    "030000000000",
                ),
                (10.0, 20.0, 80.0, 80.0),
                Color {
                    red: 255,
                    green: 255,
                    blue: 255,
                },
            ),
            (
                concat!(
                    "0100090000032900000001000d0000000000",
                    "050000000c026400c800",
                    "07000000fc0200002846a0000000",
                    "040000002d010000",
                    "0d000000410b4200000000000000000000002800500019000f00",
                    "030000000000",
                ),
                (15.0, 25.0, 95.0, 65.0),
                Color {
                    red: 0,
                    green: 0,
                    blue: 0,
                },
            ),
        ];

        for (wmf_hex, (left, top, right, bottom), fill_color) in cases {
            let input = format!(r"{{\rtf1{{\pict\wmetafile8 {wmf_hex}}}}}");
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
                _ => panic!("expected passive WMF vector image for {wmf_hex}"),
            };

            assert_eq!(image.format, ImageFormat::WmfVector, "{wmf_hex}");
            assert!(image.bytes.is_empty());
            assert_eq!(output.diagnostics.len(), 0);
            assert_eq!(image.vector_commands.len(), 1);
            assert!(matches!(
                image.vector_commands[0],
                StaticImageVectorCommand::Rectangle {
                    left: actual_left,
                    top: actual_top,
                    right: actual_right,
                    bottom: actual_bottom,
                    stroke_color: None,
                    fill_color: Some(actual_fill_color),
                    ..
                } if actual_left == left
                    && actual_top == top
                    && actual_right == right
                    && actual_bottom == bottom
                    && actual_fill_color == fill_color
            ));
        }
    }

    #[test]
    fn wmf_whiteness_stretchdib_record_becomes_passive_filled_rectangle() {
        let wmf_hex = concat!(
            "010009000003340000000200180000000000",
            "050000000c026400c800",
            "07000000fc020000a04628000000",
            "040000002d010000",
            "18000000430f6200ff000000000000000000000019002d001e0023005241572d535452455443484449422d5748495445",
            "030000000000",
        );
        let output = parse_rtf(&format!(r"{{\rtf1{{\pict\wmetafile8 {wmf_hex}}}}}")).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 35.0,
                top: 30.0,
                right: 80.0,
                bottom: 55.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 255,
                    green: 255,
                    blue: 255
                }),
                ..
            }
        ));
        assert_eq!(output.diagnostics.len(), 0);
    }

    #[test]
    fn wmf_white_brush_patpaint_after_paint_becomes_passive_white_rectangle() {
        let records = [
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color {
                red: u8::MAX,
                green: u8::MAX,
                blue: u8::MAX,
            }),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(
                10,
                20,
                70,
                60,
                WMF_PATCOPY_RASTER_OP,
                b"PATCOPY-SOURCE-PAYLOAD",
            ),
            wmf_dibbitblt_record(
                15,
                25,
                80,
                40,
                WMF_PATPAINT_RASTER_OP,
                b"PATPAINT-SOURCE-PAYLOAD",
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 2);
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle {
                left: 15.0,
                top: 25.0,
                right: 95.0,
                bottom: 65.0,
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
    fn wmf_patpaint_solid_source_on_blank_destination_paints_pattern_or_inverted_source() {
        let records = [
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color {
                red: 20,
                green: 80,
                blue: 140,
            }),
            wmf_select_object_record(0),
            wmf_stretchdib_dib_record(
                15,
                25,
                80,
                40,
                WMF_PATPAINT_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[255, 255, 255], [255, 255, 255]]),
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 15.0,
                top: 25.0,
                right: 95.0,
                bottom: 65.0,
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
    fn strict_spec_wmf_same_bounds_patpaint_solid_source_after_solid_fill_ors_pattern_and_inverted_source()
     {
        let records = [
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color {
                red: 20,
                green: 80,
                blue: 140,
            }),
            wmf_create_brush_record(Color {
                red: 100,
                green: 30,
                blue: 200,
            }),
            wmf_select_object_record(1),
            wmf_dibbitblt_record(15, 25, 80, 40, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            wmf_select_object_record(0),
            wmf_stretchdib_dib_record(
                15,
                25,
                80,
                40,
                WMF_PATPAINT_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]),
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
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
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 15.0,
                top: 25.0,
                right: 95.0,
                bottom: 65.0,
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
    fn wmf_after_paint_srcpaint_white_source_becomes_passive_white_rectangle() {
        let record = wmf_stretchdib_dib_record(
            15,
            25,
            80,
            40,
            WMF_SRCPAINT_RASTER_OP,
            &minimal_compressed_dib_with_payload(2, 1, 5, &minimal_grayscale_png(&[255, 255])),
        );
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(
                200,
                100,
                &[
                    wmf_set_window_ext_record(200, 100),
                    wmf_dibbitblt_record(
                        10,
                        20,
                        70,
                        60,
                        WMF_WHITENESS_RASTER_OP,
                        b"WHITENESS-SOURCE-PAYLOAD",
                    ),
                    record,
                ],
            ))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 2);
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle {
                left: 15.0,
                top: 25.0,
                right: 95.0,
                bottom: 65.0,
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
    fn wmf_after_paint_patpaint_white_source_with_black_brush_is_passive_noop() {
        let record = wmf_stretchdib_dib_record(
            15,
            25,
            80,
            40,
            WMF_PATPAINT_RASTER_OP,
            &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[255, 255, 255], [255, 255, 255]]),
        );
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(
                200,
                100,
                &[
                    wmf_set_window_ext_record(200, 100),
                    wmf_create_brush_record(Color::default()),
                    wmf_select_object_record(0),
                    wmf_dibbitblt_record(
                        10,
                        20,
                        70,
                        60,
                        WMF_WHITENESS_RASTER_OP,
                        b"WHITENESS-SOURCE-PAYLOAD",
                    ),
                    record,
                ],
            ))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 10.0,
                top: 20.0,
                right: 80.0,
                bottom: 80.0,
                ..
            }
        ));
    }

    #[test]
    fn wmf_after_paint_srcand_white_source_is_passive_noop_without_skipped_diagnostic() {
        let record = wmf_stretchdib_dib_record(
            15,
            25,
            80,
            40,
            WMF_SRCAND_RASTER_OP,
            &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[255, 255, 255], [255, 255, 255]]),
        );
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(
                200,
                100,
                &[
                    wmf_set_window_ext_record(200, 100),
                    wmf_dibbitblt_record(
                        10,
                        20,
                        70,
                        60,
                        WMF_BLACKNESS_RASTER_OP,
                        b"BLACKNESS-PAINT",
                    ),
                    record,
                ],
            ))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 10.0,
                top: 20.0,
                right: 80.0,
                bottom: 80.0,
                ..
            }
        ));
    }

    #[test]
    fn wmf_initial_dstinvert_bitblt_record_becomes_passive_black_rectangle() {
        let records = [
            wmf_set_window_ext_record(200, 100),
            wmf_dibbitblt_record(
                15,
                25,
                80,
                40,
                WMF_DSTINVERT_RASTER_OP,
                b"DSTINVERT-SOURCE-PAYLOAD",
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
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
                right: 95.0,
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
    fn strict_spec_wmf_same_bounds_dstinvert_bitblt_after_solid_fill_inverts_rectangle() {
        let records = [
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color {
                red: 100,
                green: 30,
                blue: 200,
            }),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(15, 25, 80, 40, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            wmf_dibbitblt_record(
                15,
                25,
                80,
                40,
                WMF_DSTINVERT_RASTER_OP,
                b"DSTINVERT-SOURCE-PAYLOAD",
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
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
            _ => panic!("expected passive WMF vector image"),
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
                right: 95.0,
                bottom: 65.0,
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
    fn strict_spec_wmf_same_bounds_srcinvert_white_source_after_solid_fill_inverts_rectangle() {
        let records = [
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color {
                red: 100,
                green: 30,
                blue: 200,
            }),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(15, 25, 80, 40, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            wmf_stretchdib_dib_record(
                15,
                25,
                80,
                40,
                WMF_SRCINVERT_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[255, 255, 255], [255, 255, 255]]),
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
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
            _ => panic!("expected passive WMF vector image"),
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
                right: 95.0,
                bottom: 65.0,
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
    fn strict_spec_wmf_same_bounds_srcand_solid_source_after_solid_fill_ands_rectangle() {
        let records = [
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color {
                red: 100,
                green: 30,
                blue: 200,
            }),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(15, 25, 80, 40, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            wmf_stretchdib_dib_record(
                15,
                25,
                80,
                40,
                WMF_SRCAND_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]),
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
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
            _ => panic!("expected passive WMF vector image"),
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
                right: 95.0,
                bottom: 65.0,
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
    fn strict_spec_wmf_same_bounds_notsrcerase_solid_source_after_solid_fill_nors_rectangle() {
        let records = [
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color {
                red: 100,
                green: 30,
                blue: 200,
            }),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(15, 25, 80, 40, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            wmf_stretchdib_dib_record(
                15,
                25,
                80,
                40,
                WMF_NOTSRCERASE_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]),
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
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
            _ => panic!("expected passive WMF vector image"),
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
                right: 95.0,
                bottom: 65.0,
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
    fn strict_spec_wmf_same_bounds_srcerase_solid_source_after_solid_fill_ands_inverted_dest_rectangle()
     {
        let records = [
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color {
                red: 100,
                green: 30,
                blue: 200,
            }),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(15, 25, 80, 40, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            wmf_stretchdib_dib_record(
                15,
                25,
                80,
                40,
                WMF_SRCERASE_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]),
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
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
            _ => panic!("expected passive WMF vector image"),
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
                right: 95.0,
                bottom: 65.0,
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
    fn strict_spec_wmf_same_bounds_srcpaint_solid_source_after_solid_fill_ors_rectangle() {
        let records = [
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color {
                red: 100,
                green: 30,
                blue: 200,
            }),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(15, 25, 80, 40, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            wmf_stretchdib_dib_record(
                15,
                25,
                80,
                40,
                WMF_SRCPAINT_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]),
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
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
            _ => panic!("expected passive WMF vector image"),
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
                right: 95.0,
                bottom: 65.0,
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
    fn strict_spec_wmf_same_bounds_mergepaint_solid_source_after_solid_fill_ors_inverted_source_rectangle()
     {
        let records = [
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color {
                red: 100,
                green: 30,
                blue: 200,
            }),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(15, 25, 80, 40, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            wmf_stretchdib_dib_record(
                15,
                25,
                80,
                40,
                WMF_MERGEPAINT_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]),
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
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
            _ => panic!("expected passive WMF vector image"),
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
                right: 95.0,
                bottom: 65.0,
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
    fn strict_spec_wmf_same_bounds_srcinvert_solid_source_after_solid_fill_xors_rectangle() {
        let records = [
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color {
                red: 100,
                green: 30,
                blue: 200,
            }),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(15, 25, 80, 40, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            wmf_stretchdib_dib_record(
                15,
                25,
                80,
                40,
                WMF_SRCINVERT_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]),
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
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
            _ => panic!("expected passive WMF vector image"),
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
                right: 95.0,
                bottom: 65.0,
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
    fn wmf_black_brush_mergecopy_bitblt_record_becomes_passive_black_rectangle() {
        let records = [
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color::default()),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(
                15,
                25,
                80,
                40,
                WMF_MERGECOPY_RASTER_OP,
                b"MERGECOPY-SOURCE-PAYLOAD",
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
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
                right: 95.0,
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
    fn wmf_mergecopy_solid_source_paints_source_and_brush_rectangle() {
        let records = [
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color {
                red: 240,
                green: 15,
                blue: 170,
            }),
            wmf_select_object_record(0),
            wmf_stretchdib_dib_record(
                15,
                25,
                80,
                40,
                WMF_MERGECOPY_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 85, 204], [170, 85, 204]]),
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
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
                right: 95.0,
                bottom: 65.0,
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
    fn wmf_blank_source_raster_ops_become_passive_solid_rectangles() {
        let cases = [
            (
                WMF_SRCERASE_RASTER_OP,
                b"SRCERASE-SOURCE-PAYLOAD".as_slice(),
                Color {
                    red: 0,
                    green: 0,
                    blue: 0,
                },
            ),
            (
                WMF_NOTSRCERASE_RASTER_OP,
                b"NOTSRCERASE-SOURCE-PAYLOAD".as_slice(),
                Color {
                    red: 0,
                    green: 0,
                    blue: 0,
                },
            ),
            (
                WMF_SRCPAINT_RASTER_OP,
                b"SRCPAINT-SOURCE-PAYLOAD".as_slice(),
                Color {
                    red: 255,
                    green: 255,
                    blue: 255,
                },
            ),
            (
                WMF_PATPAINT_RASTER_OP,
                b"PATPAINT-SOURCE-PAYLOAD".as_slice(),
                Color {
                    red: 255,
                    green: 255,
                    blue: 255,
                },
            ),
        ];

        for (raster_op, payload, fill_color) in cases {
            let records = [
                wmf_set_window_ext_record(200, 100),
                wmf_dibbitblt_record(15, 25, 80, 40, raster_op, payload),
            ];
            let input = format!(
                r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
                bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
            );
            let output = parse_rtf(&input).unwrap();

            let image = match &output.document.blocks[0] {
                Block::Image(image) => image,
                _ => panic!("expected passive WMF vector image"),
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
                    right: 95.0,
                    bottom: 65.0,
                    stroke_color: None,
                    fill_color: Some(actual_fill_color),
                    ..
                } if actual_fill_color == fill_color
            ));
        }
    }

    #[test]
    fn wmf_dstcopy_bitblt_record_is_passive_noop_without_skipped_diagnostic() {
        let records = [
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color {
                red: 30,
                green: 110,
                blue: 180,
            }),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(
                10,
                20,
                70,
                60,
                WMF_PATCOPY_RASTER_OP,
                b"PATCOPY-SOURCE-PAYLOAD",
            ),
            wmf_dibbitblt_record(
                15,
                25,
                80,
                40,
                WMF_DSTCOPY_RASTER_OP,
                b"DSTCOPY-SOURCE-PAYLOAD",
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
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
    fn wmf_black_brush_patinvert_after_paint_is_passive_noop_without_skipped_diagnostic() {
        let records = [
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color::default()),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(
                10,
                20,
                70,
                60,
                WMF_PATCOPY_RASTER_OP,
                b"PATCOPY-SOURCE-PAYLOAD",
            ),
            wmf_dibbitblt_record(
                15,
                25,
                80,
                40,
                WMF_PATINVERT_RASTER_OP,
                b"PATINVERT-SOURCE-PAYLOAD",
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
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
    fn wmf_initial_patinvert_bitblt_record_becomes_passive_inverted_brush_rectangle() {
        let records = [
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color {
                red: 20,
                green: 80,
                blue: 140,
            }),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(
                15,
                25,
                80,
                40,
                WMF_PATINVERT_RASTER_OP,
                b"PATINVERT-SOURCE-PAYLOAD",
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
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
                right: 95.0,
                bottom: 65.0,
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
    fn strict_spec_wmf_same_bounds_patinvert_bitblt_after_solid_fill_xors_rectangle() {
        let records = [
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color {
                red: 100,
                green: 30,
                blue: 200,
            }),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(15, 25, 80, 40, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            wmf_create_brush_record(Color {
                red: 20,
                green: 80,
                blue: 140,
            }),
            wmf_select_object_record(1),
            wmf_dibbitblt_record(
                15,
                25,
                80,
                40,
                WMF_PATINVERT_RASTER_OP,
                b"PATINVERT-SOURCE-PAYLOAD",
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
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
            _ => panic!("expected passive WMF vector image"),
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
                right: 95.0,
                bottom: 65.0,
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
    fn strict_spec_wmf_source_backed_patinvert_after_solid_fill_ignores_dib_and_xors_rectangle() {
        let mut dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[1, 2, 3], [250, 251, 252]]);
        dib.extend_from_slice(b"TRAILING-WMF-PATINVERT-DIB-SOURCE /EmbeddedFile");
        let records = [
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color {
                red: 100,
                green: 30,
                blue: 200,
            }),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(15, 25, 80, 40, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            wmf_create_brush_record(Color {
                red: 20,
                green: 80,
                blue: 140,
            }),
            wmf_select_object_record(1),
            wmf_stretchdib_dib_record(15, 25, 80, 40, WMF_PATINVERT_RASTER_OP, &dib),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
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
            _ => panic!("expected passive WMF vector image"),
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
                right: 95.0,
                bottom: 65.0,
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
    fn wmf_notpatcopy_bitblt_record_becomes_passive_inverted_brush_rectangle() {
        let records = [
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color {
                red: 20,
                green: 80,
                blue: 140,
            }),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(
                15,
                25,
                80,
                40,
                WMF_NOTPATCOPY_RASTER_OP,
                b"NOTPATCOPY-SOURCE-PAYLOAD",
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
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
                right: 95.0,
                bottom: 65.0,
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
    fn wmf_notpatcopy_stretchdib_ignores_source_and_paints_inverted_brush() {
        let records = [
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(Color {
                red: 20,
                green: 80,
                blue: 140,
            }),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(15, 25, 80, 40, WMF_PATCOPY_RASTER_OP, b"PATCOPY-FILL"),
            wmf_stretchdib_dib_record(
                15,
                25,
                80,
                40,
                WMF_NOTPATCOPY_RASTER_OP,
                &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[255, 63, 170], [15, 240, 85]]),
            ),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(output.diagnostics.len(), 0);
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle {
                left: 15.0,
                top: 25.0,
                right: 95.0,
                bottom: 65.0,
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
    fn strict_spec_wmf_patcopy_stretchdib_records_become_passive_filled_rectangles() {
        let wmf_hex = concat!(
            "0100090000032a00000001000e0000000000",
            "050000000c026400c800",
            "07000000fc020000a04628000000",
            "040000002d010000",
            "0e000000430f2100f000000000000000000000002800500019000f00",
            "030000000000",
        );
        let input = format!(r"{{\rtf1{{\pict\wmetafile8 {wmf_hex}}}}}");
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
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 15.0,
                top: 25.0,
                right: 95.0,
                bottom: 65.0,
                stroke_color: None,
                fill_color: Some(Color {
                    red: 160,
                    green: 70,
                    blue: 40
                }),
                ..
            }
        ));
    }

    #[test]
    fn wmf_nonblack_mergecopy_stretchdib_indexed_png_masks_palette() {
        let png = minimal_indexed_png_with_dimensions(2, 1);
        let record = wmf_stretchdib_dib_record(
            15,
            25,
            80,
            40,
            WMF_MERGECOPY_RASTER_OP,
            &minimal_compressed_dib_with_payload(2, 1, 5, &png),
        );
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(
                200,
                100,
                &[
                    wmf_create_brush_record(Color {
                        red: 240,
                        green: 15,
                        blue: 170
                    }),
                    wmf_select_object_record(0),
                    record,
                ],
            ))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        let StaticImageVectorCommand::RasterImage { image, .. } = &image.vector_commands[0] else {
            panic!("expected passive raster command");
        };
        assert_eq!(image.format, ImageFormat::PngIndexed);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.palette, vec![240, 0, 0, 0, 15, 0]);
        assert!(!image.bytes.is_empty());
    }

    #[test]
    fn wmf_black_brush_mergecopy_stretchdib_jpeg_becomes_passive_black_rectangle() {
        let record = wmf_stretchdib_dib_record(
            15,
            25,
            80,
            40,
            WMF_MERGECOPY_RASTER_OP,
            &minimal_compressed_dib_with_payload(2, 1, 4, &minimal_jpeg_with_dimensions(2, 1)),
        );
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(
                200,
                100,
                &[
                    wmf_set_window_ext_record(200, 100),
                    wmf_create_brush_record(Color::default()),
                    wmf_select_object_record(0),
                    wmf_dibbitblt_record(
                        10,
                        20,
                        70,
                        60,
                        WMF_WHITENESS_RASTER_OP,
                        b"WHITENESS-PAINT",
                    ),
                    record,
                ],
            ))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 2);
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle {
                left: 15.0,
                top: 25.0,
                right: 95.0,
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
    fn wmf_white_brush_mergecopy_stretchdib_jpeg_copies_embedded_source() {
        let jpeg = minimal_jpeg_with_dimensions(2, 1);
        let record = wmf_stretchdib_dib_record(
            15,
            25,
            80,
            40,
            WMF_MERGECOPY_RASTER_OP,
            &minimal_compressed_dib_with_payload(2, 1, 4, &jpeg),
        );
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(
                200,
                100,
                &[
                    wmf_create_brush_record(Color {
                        red: u8::MAX,
                        green: u8::MAX,
                        blue: u8::MAX,
                    }),
                    wmf_select_object_record(0),
                    record,
                ],
            ))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage {
                left: 15.0,
                top: 25.0,
                right: 95.0,
                bottom: 65.0,
                image,
            } if image.format == ImageFormat::Jpeg
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == jpeg
        ));
    }

    #[test]
    fn wmf_srccopy_stretchdib_record_becomes_passive_raster_image() {
        let record = wmf_stretchdib_dib_record(
            15,
            25,
            80,
            40,
            WMF_SRCCOPY_RASTER_OP,
            &minimal_24bit_dib_with_dimensions(2, 1),
        );
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &[record]))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        let StaticImageVectorCommand::RasterImage {
            left,
            top,
            right,
            bottom,
            image,
        } = &image.vector_commands[0]
        else {
            panic!("expected passive raster command");
        };
        assert!((*left - 15.079_498).abs() < 0.000_1);
        assert!((*top - 26.530_811).abs() < 0.000_1);
        assert!((*right - 93.652_44).abs() < 0.000_1);
        assert!((*bottom - 69.388_46).abs() < 0.000_1);
        assert_eq!(image.format, ImageFormat::Rgb8);
        assert_eq!((image.width_px, image.height_px), (2, 1));
        assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0]);
    }

    #[test]
    fn wmf_blank_destination_srcand_stretchdib_record_becomes_passive_raster_image() {
        let record = wmf_stretchdib_dib_record(
            15,
            25,
            80,
            40,
            WMF_SRCAND_RASTER_OP,
            &minimal_24bit_dib_with_dimensions(2, 1),
        );
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &[record]))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage {
                left: 15.0,
                top: 25.0,
                right: 95.0,
                bottom: 65.0,
                image,
            } if image.format == ImageFormat::Rgb8
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == vec![255, 0, 0, 0, 255, 0]
        ));
    }

    #[test]
    fn wmf_blank_destination_srcinvert_stretchdib_record_becomes_passive_inverted_raster_image() {
        let record = wmf_stretchdib_dib_record(
            15,
            25,
            80,
            40,
            WMF_SRCINVERT_RASTER_OP,
            &minimal_24bit_dib_with_dimensions(2, 1),
        );
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &[record]))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage {
                left: 15.0,
                top: 25.0,
                right: 95.0,
                bottom: 65.0,
                image,
            } if image.format == ImageFormat::Rgb8
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == vec![0, 255, 255, 255, 0, 255]
        ));
    }

    #[test]
    fn wmf_blank_destination_srcinvert_stretchdib_indexed_bi_png_record_inverts_palette() {
        let png = minimal_indexed_png_with_dimensions(2, 1);
        let record = wmf_stretchdib_dib_record(
            15,
            25,
            80,
            40,
            WMF_SRCINVERT_RASTER_OP,
            &minimal_compressed_dib_with_payload(2, 1, 5, &png),
        );
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &[record]))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        let StaticImageVectorCommand::RasterImage { image, .. } = &image.vector_commands[0] else {
            panic!("expected passive raster command");
        };
        assert_eq!(image.format, ImageFormat::PngIndexed);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.palette, vec![0, 255, 255, 255, 0, 255]);
        assert!(!image.bytes.is_empty());
    }

    #[test]
    fn wmf_notsrccopy_stretchdib_record_becomes_passive_inverted_raster_image() {
        let record = wmf_stretchdib_dib_record(
            15,
            25,
            80,
            40,
            WMF_NOTSRCCOPY_RASTER_OP,
            &minimal_24bit_dib_with_dimensions(2, 1),
        );
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &[record]))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage {
                left: 15.0,
                top: 25.0,
                right: 95.0,
                bottom: 65.0,
                image,
            } if image.format == ImageFormat::Rgb8
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == vec![0, 255, 255, 255, 0, 255]
        ));
    }

    #[test]
    fn wmf_srccopy_solid_source_dib_record_becomes_passive_solid_rectangle() {
        let record = wmf_stretchdib_dib_record(
            15,
            25,
            80,
            40,
            WMF_SRCCOPY_RASTER_OP,
            &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[12, 34, 56], [12, 34, 56]]),
        );
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &[record]))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        let StaticImageVectorCommand::Rectangle {
            left,
            top,
            right,
            bottom,
            stroke_color,
            fill_color,
            ..
        } = image.vector_commands[0]
        else {
            panic!("expected passive solid rectangle");
        };
        assert!((left - 15.079_498).abs() < 0.000_1);
        assert!((top - 26.530_811).abs() < 0.000_1);
        assert!((right - 93.652_44).abs() < 0.000_1);
        assert!((bottom - 69.388_46).abs() < 0.000_1);
        assert_eq!(stroke_color, None);
        assert_eq!(
            fill_color,
            Some(Color {
                red: 12,
                green: 34,
                blue: 56
            })
        );
    }

    #[test]
    fn wmf_notsrccopy_solid_source_dib_record_becomes_passive_solid_rectangle() {
        let record = wmf_stretchdib_dib_record(
            15,
            25,
            80,
            40,
            WMF_NOTSRCCOPY_RASTER_OP,
            &minimal_24bit_dib_with_rgb_pixels(2, 1, &[[12, 34, 56], [12, 34, 56]]),
        );
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &[record]))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Rectangle {
                left: 15.0,
                top: 25.0,
                right: 95.0,
                bottom: 65.0,
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
    fn wmf_notsrccopy_flat_color_alpha_source_remains_passive_raster_image() {
        let record = wmf_stretchdib_dib_record(
            15,
            25,
            80,
            40,
            WMF_NOTSRCCOPY_RASTER_OP,
            &minimal_compressed_dib_with_payload(
                2,
                1,
                5,
                &minimal_rgba_png(&[[12, 34, 56, 64], [12, 34, 56, 192]]),
            ),
        );
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &[record]))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        let StaticImageVectorCommand::RasterImage {
            left: 15.0,
            top: 25.0,
            right: 95.0,
            bottom: 65.0,
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
            vec![0, 243, 221, 199, 243, 221, 199]
        );
        let alpha_mask = image.alpha_mask.as_ref().expect("source alpha mask");
        assert_eq!(
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&alpha_mask.bytes, 3).unwrap(),
            vec![0, 64, 192]
        );
    }

    #[test]
    fn wmf_notsrccopy_stretchdib_indexed_bi_png_record_inverts_palette() {
        let png = minimal_indexed_png_with_dimensions(2, 1);
        let record = wmf_stretchdib_dib_record(
            15,
            25,
            80,
            40,
            WMF_NOTSRCCOPY_RASTER_OP,
            &minimal_compressed_dib_with_payload(2, 1, 5, &png),
        );
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &[record]))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        let StaticImageVectorCommand::RasterImage { image, .. } = &image.vector_commands[0] else {
            panic!("expected passive raster command");
        };
        assert_eq!(image.format, ImageFormat::PngIndexed);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.palette, vec![0, 255, 255, 255, 0, 255]);
        assert!(!image.bytes.is_empty());
    }

    #[test]
    fn wmf_notsrccopy_stretchdib_bi_jpeg_record_becomes_passive_inverted_raster_image() {
        let jpeg = minimal_jpeg_with_dimensions(2, 1);
        let record = wmf_stretchdib_dib_record(
            15,
            25,
            80,
            40,
            WMF_NOTSRCCOPY_RASTER_OP,
            &minimal_compressed_dib_with_payload(2, 1, 4, &jpeg),
        );
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &[record]))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage {
                left: 15.0,
                top: 25.0,
                right: 95.0,
                bottom: 65.0,
                image,
            } if image.format == ImageFormat::JpegInverted
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == jpeg
        ));
    }

    #[test]
    fn wmf_srccopy_stretchdib_bi_png_record_becomes_passive_raster_image() {
        let png = minimal_rgb_png_with_dimensions(2, 1);
        let record = wmf_stretchdib_dib_record(
            15,
            25,
            80,
            40,
            WMF_SRCCOPY_RASTER_OP,
            &minimal_compressed_dib_with_payload(2, 1, 5, &png),
        );
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &[record]))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        let StaticImageVectorCommand::RasterImage {
            left,
            top,
            right,
            bottom,
            image,
        } = &image.vector_commands[0]
        else {
            panic!("expected passive PNG raster command");
        };
        assert!((*left - 15.079_498).abs() < 0.000_1);
        assert!((*top - 26.530_811).abs() < 0.000_1);
        assert!((*right - 93.652_44).abs() < 0.000_1);
        assert!((*bottom - 69.388_46).abs() < 0.000_1);
        assert_eq!(image.format, ImageFormat::Png);
        assert_eq!((image.width_px, image.height_px), (2, 1));
        assert!(image.palette.is_empty());
        assert!(image.alpha_mask.is_none());
    }

    #[test]
    fn strict_spec_wmf_setdib_to_dev_record_becomes_passive_raster_image() {
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
            wmf_setdib_to_dev_record(12, 18, 2, 1, 0, 1, &minimal_24bit_dib_with_dimensions(2, 1)),
            wmf_setdib_to_dev_record(
                50,
                30,
                2,
                3,
                1,
                1,
                &minimal_24bit_dib_with_rgb_pixels(2, 3, &rows),
            ),
            wmf_setdib_to_dev_record(
                80,
                44,
                2,
                3,
                3,
                1,
                &minimal_24bit_dib_with_rgb_pixels(2, 3, &rows),
            ),
            wmf_setdib_to_dev_record(
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
            wmf_setdib_to_dev_record(112, 56, 2, 3, 0, 1, &top_down_compressed_png_dib),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
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
            _ => panic!("expected passive WMF vector image"),
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
    fn wmf_srccopy_dibbitblt_record_becomes_passive_raster_image() {
        let record = wmf_dibbitblt_record(
            15,
            25,
            2,
            1,
            WMF_SRCCOPY_RASTER_OP,
            &minimal_24bit_dib_with_dimensions(2, 1),
        );
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &[record]))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        let StaticImageVectorCommand::RasterImage {
            left,
            top,
            right,
            bottom,
            image,
        } = &image.vector_commands[0]
        else {
            panic!("expected passive DIBBITBLT raster command");
        };
        assert!((*left - 15.079_498).abs() < 0.000_1);
        assert!((*top - 26.530_811).abs() < 0.000_1);
        assert!((*right - 16.666_801).abs() < 0.000_1);
        assert!((*bottom - 27.891_36).abs() < 0.000_1);
        assert_eq!(image.format, ImageFormat::Rgb8);
        assert_eq!((image.width_px, image.height_px), (2, 1));
        assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0]);
    }

    #[test]
    fn wmf_srccopy_dibstretchblt_record_becomes_passive_raster_image() {
        let record = wmf_dibstretchblt_record(
            15,
            25,
            80,
            40,
            WMF_SRCCOPY_RASTER_OP,
            &minimal_24bit_dib_with_dimensions(2, 1),
        );
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &[record]))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 1);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage {
                left: 15.0,
                top: 25.0,
                right: 95.0,
                bottom: 65.0,
                image,
            } if image.format == ImageFormat::Rgb8
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == vec![255, 0, 0, 0, 255, 0]
        ));
    }

    #[test]
    fn word_compatible_wmf_srccopy_cropped_embedded_dib_records_use_record_specific_rows() {
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
        let records = [
            wmf_stretchdib_dib_record_with_source(15, 25, 80, 40, 1, 1, 2, 1, &dib),
            wmf_dibbitblt_record_with_source(25, 35, 2, 1, 1, 1, &dib),
            wmf_dibstretchblt_record_with_source(35, 45, 60, 30, 1, 1, 2, 1, &dib),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 3);
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage { image, .. }
                if image.format == ImageFormat::Rgb8
                    && image.width_px == 2
                    && image.height_px == 1
                    && image.bytes == vec![40, 50, 60, 70, 80, 90]
        ));
        for command in &image.vector_commands[1..] {
            assert!(matches!(
                command,
                StaticImageVectorCommand::RasterImage { image, .. }
                    if image.format == ImageFormat::Rgb8
                        && image.width_px == 2
                        && image.height_px == 1
                        && image.bytes == vec![130, 140, 150, 160, 170, 180]
            ));
        }
        let StaticImageVectorCommand::RasterImage {
            left,
            top,
            right,
            bottom,
            ..
        } = image.vector_commands[0]
        else {
            panic!("expected passive raster command");
        };
        assert!((left - 15.079_498).abs() < 0.000_1);
        assert!((top - 26.530_811).abs() < 0.000_1);
        assert!((right - 93.652_44).abs() < 0.000_1);
        assert!((bottom - 69.388_46).abs() < 0.000_1);
        let StaticImageVectorCommand::RasterImage {
            left,
            top,
            right,
            bottom,
            ..
        } = image.vector_commands[1]
        else {
            panic!("expected cropped passive DIBBITBLT raster command");
        };
        assert!((left - 24.603_168).abs() < 0.000_1);
        assert!((top - 37.413_803).abs() < 0.000_1);
        assert!((right - 26.984_123).abs() < 0.000_1);
        assert!((bottom - 38.774_35).abs() < 0.000_1);
        assert!(matches!(
            image.vector_commands[2],
            StaticImageVectorCommand::RasterImage {
                left: 35.0,
                top: 45.0,
                right: 95.0,
                bottom: 75.0,
                ..
            }
        ));
    }

    #[test]
    fn strict_spec_wmf_srccopy_cropped_embedded_dib_records_use_top_down_rows() {
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
        let records = [
            wmf_stretchdib_dib_record_with_source(15, 25, 80, 40, 1, 1, 2, 1, &dib),
            wmf_dibbitblt_record_with_source(25, 35, 2, 1, 1, 1, &dib),
            wmf_dibstretchblt_record_with_source(35, 45, 60, 30, 1, 1, 2, 1, &dib),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
        );
        let output = parse_rtf_strict(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.vector_commands.len(), 3);
        for command in &image.vector_commands {
            assert!(matches!(
                command,
                StaticImageVectorCommand::RasterImage { image, .. }
                    if image.format == ImageFormat::Rgb8
                        && image.width_px == 2
                        && image.height_px == 1
                        && image.bytes == vec![130, 140, 150, 160, 170, 180]
            ));
        }
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::RasterImage {
                left: 15.0,
                top: 25.0,
                right: 95.0,
                bottom: 65.0,
                ..
            }
        ));
    }

    #[test]
    fn wmf_srccopy_cropped_jpeg_embedded_dib_records_become_passive_clipped_raster_images() {
        let jpeg = minimal_jpeg_with_dimensions(3, 2);
        let dib = minimal_compressed_dib_with_payload(3, 2, 4, &jpeg);
        let records = [
            wmf_stretchdib_dib_record_with_source(15, 25, 80, 40, 1, 0, 1, 2, &dib),
            wmf_dibstretchblt_record_with_source(35, 45, 60, 30, 1, 0, 1, 2, &dib),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
        assert_eq!(image.vector_commands.len(), 8);
        for chunk in image.vector_commands.chunks_exact(4) {
            assert!(matches!(chunk[0], StaticImageVectorCommand::SaveState));
            assert!(matches!(chunk[3], StaticImageVectorCommand::RestoreState));
            assert!(matches!(
                &chunk[2],
                StaticImageVectorCommand::RasterImage { image, .. }
                    if image.format == ImageFormat::Jpeg
                        && image.width_px == 3
                        && image.height_px == 2
                        && image.bytes == jpeg
                        && image.alpha_mask.is_none()
            ));
        }
        let StaticImageVectorCommand::ClipRect {
            left,
            top,
            right,
            bottom,
        } = image.vector_commands[1]
        else {
            panic!("expected passive JPEG clip rectangle");
        };
        assert!((left - 15.079_498).abs() < 0.000_1);
        assert!((top - 26.530_811).abs() < 0.000_1);
        assert!((right - 93.652_44).abs() < 0.000_1);
        assert!((bottom - 69.388_46).abs() < 0.000_1);
        let StaticImageVectorCommand::RasterImage {
            left,
            top,
            right,
            bottom,
            ..
        } = image.vector_commands[2]
        else {
            panic!("expected clipped passive JPEG raster");
        };
        assert!((left - -63.493_446).abs() < 0.000_1);
        assert!((top - 26.530_811).abs() < 0.000_1);
        assert!((right - 172.225_39).abs() < 0.000_1);
        assert!((bottom - 69.388_46).abs() < 0.000_1);
        assert!(matches!(
            image.vector_commands[5],
            StaticImageVectorCommand::ClipRect {
                left: 35.0,
                top: 45.0,
                right: 95.0,
                bottom: 75.0,
            }
        ));
        assert!(matches!(
            image.vector_commands[6],
            StaticImageVectorCommand::RasterImage {
                left: -25.0,
                top: 45.0,
                right: 155.0,
                bottom: 75.0,
                ..
            }
        ));
    }

    #[test]
    fn word_compatible_wmf_rectangle_and_ellipse_use_record_specific_recovery_bounds() {
        let records = [
            wmf_bounds_record(0x041b, 20, 10, 180, 80),
            wmf_bounds_record(0x0418, 100, 20, 190, 90),
        ];
        let output = parse_rtf(&format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
        ))
        .unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.vector_commands.len(), 2);
        let StaticImageVectorCommand::Rectangle {
            left,
            top,
            right,
            bottom,
            ..
        } = image.vector_commands[0]
        else {
            panic!("expected recovered rectangle");
        };
        assert!((left - 19.841_167).abs() < 0.001);
        assert!((top - 10.882_992).abs() < 0.001);
        assert!((right - 158.729_89).abs() < 0.001);
        assert!((bottom - 68.707_52).abs() < 0.001);
        let StaticImageVectorCommand::Ellipse {
            left,
            top,
            right,
            bottom,
            ..
        } = image.vector_commands[1]
        else {
            panic!("expected recovered ellipse");
        };
        assert!((left - 98.413_3).abs() < 0.001);
        assert!((top - 21.769).abs() < 0.001);
        assert!((right - 169.846_57).abs() < 0.001);
        assert!((bottom - 78.231).abs() < 0.001);
    }

    #[test]
    fn word_compatible_wmf_line_and_polygon_use_path_recovery_map() {
        let records = [
            wmf_yx_record(0x0214, 20, 20),
            wmf_yx_record(0x0213, 20, 180),
            wmf_point_list_record(0x0324, &[(40, 20), (40, 180), (90, 100)]),
        ];
        let output = parse_rtf(&format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
        ))
        .unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.vector_commands.len(), 2);
        assert!(matches!(
            image.vector_commands[0],
            StaticImageVectorCommand::Line { x1, y1, x2, y2, .. }
                if (x1 - 28.174_584).abs() < 0.001
                    && (y1 - 28.758_945).abs() < 0.001
                    && (x2 - 185.952_26).abs() < 0.001
                    && (y2 - 28.758_945).abs() < 0.001
        ));
        assert!(matches!(
            &image.vector_commands[1],
            StaticImageVectorCommand::Polygon { points, .. }
                if (points[0].0 - 28.174_584).abs() < 0.001
                    && (points[0].1 - 50.128_84).abs() < 0.001
                    && (points[2].0 - 107.063_42).abs() < 0.001
                    && (points[2].1 - 103.553_58).abs() < 0.001
        ));
    }

    #[test]
    fn wmf_offsetcliprgn_offsets_unpainted_clip_rect() {
        let wmf_hex = concat!(
            "0100090000032f0000000100070000000000",
            "050000000c026400c800",
            "07000000fc020000dcdcdc000000",
            "040000002d010000",
            "0700000016043c00640014001400",
            "05000000200205000a00",
            "070000001b045000b4000a001400",
            "030000000000",
        );
        let output =
            parse_rtf_strict(&format!(r"{{\rtf1{{\pict\wmetafile8 {wmf_hex}}}}}")).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
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
    fn wmf_zero_offsetcliprgn_is_passive_noop_without_clip_state() {
        let mut offset = wmf_yx_record(0x0220, 0, 0);
        offset.extend_from_slice(b"ZERO-OFFSET-WMF-OFFSETCLIPRGN /JavaScript");
        offset.resize(offset.len().next_multiple_of(2), 0);
        let offset_len_words = (offset.len() / 2) as u32;
        write_test_le_u32(&mut offset, 0, offset_len_words);
        let records = [
            wmf_bounds_record(0x041b, 0, 0, 30, 30),
            offset,
            wmf_bounds_record(0x041b, 40, 10, 80, 40),
        ];
        let output = parse_rtf_strict(&format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
        ))
        .unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
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
    fn wmf_excludecliprect_after_unpainted_clip_rect_becomes_passive_exclude_clip_path() {
        let records = [
            wmf_bounds_record(0x0416, 10, 10, 120, 70),
            wmf_bounds_record(0x0415, 40, 20, 80, 50),
            wmf_bounds_record(0x041b, 0, 0, 160, 80),
        ];
        let output = parse_rtf_strict(&format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
        ))
        .unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
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
    fn wmf_empty_intersectcliprect_becomes_passive_empty_clip() {
        let records = [
            wmf_bounds_record(0x0416, 20, 20, 20, 60),
            wmf_bounds_record(0x041b, 0, 0, 160, 80),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8\picw160\pich80\picwgoal2160\pichgoal1080 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
        );
        let output = parse_rtf_strict(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(image.vector_commands.len(), 2);
        assert_eq!(
            image.vector_commands[0],
            StaticImageVectorCommand::ClipRect {
                left: 20.0,
                top: 20.0,
                right: 20.0,
                bottom: 60.0,
            }
        );
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle { .. }
        ));
    }

    #[test]
    fn wmf_empty_excludecliprect_is_passive_noop() {
        let records = [
            wmf_bounds_record(0x0415, 20, 20, 20, 60),
            wmf_bounds_record(0x041b, 0, 0, 160, 80),
        ];
        let input = format!(
            r"{{\rtf1{{\pict\wmetafile8\picw160\pich80\picwgoal2160\pichgoal1080 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(160, 80, &records))
        );
        let output = parse_rtf(&input).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
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
    fn wmf_excludecliprect_disjoint_from_unpainted_clip_rect_is_passive_noop() {
        let records = [
            wmf_bounds_record(0x0416, 10, 10, 120, 70),
            wmf_bounds_record(0x0415, 130, 20, 150, 50),
            wmf_bounds_record(0x041b, 0, 0, 160, 80),
        ];
        let output = parse_rtf_strict(&format!(
            r"{{\rtf1{{\pict\wmetafile8 {}}}}}",
            bytes_to_hex(&minimal_wmf_with_records(200, 100, &records))
        ))
        .unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
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
    fn wmf_offsetcliprgn_after_painted_clip_offsets_unscoped_clip() {
        let wmf_hex = concat!(
            "010009000003360000000100070000000000",
            "050000000c026400c800",
            "07000000fc020000dcdcdc000000",
            "040000002d010000",
            "0700000016043c00640014001400",
            "070000001b042800280000000000",
            "05000000200205000a00",
            "070000001b045000b4000a001400",
            "030000000000",
        );
        let output =
            parse_rtf_strict(&format!(r"{{\rtf1{{\pict\wmetafile8 {wmf_hex}}}}}")).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
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
    fn wmf_offsetcliprgn_after_painted_unscoped_clip_mutation_offsets_combined_clip() {
        let wmf_hex = concat!(
            "0100090000033d0000000100070000000000",
            "050000000c026400c800",
            "07000000fc020000dcdcdc000000",
            "040000002d010000",
            "0700000016043c00640014001400",
            "070000001b042800280000000000",
            "0700000016044600780028001e00",
            "05000000200205000a00",
            "070000001b045000b4000a001400",
            "030000000000",
        );
        let output =
            parse_rtf_strict(&format!(r"{{\rtf1{{\pict\wmetafile8 {wmf_hex}}}}}")).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
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
                left: 30.0,
                top: 40.0,
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
                left: 40.0,
                top: 45.0,
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
    fn wmf_offsetcliprgn_after_painted_saved_clip_offsets_scoped_clip() {
        let wmf_hex = concat!(
            "010009000003390000000100070000000000",
            "050000000c026400c800",
            "07000000fc020000dcdcdc000000",
            "040000002d010000",
            "030000001e00",
            "0700000016043c00640014001400",
            "070000001b042800280000000000",
            "05000000200205000a00",
            "070000001b045000b4000a001400",
            "030000000000",
        );
        let output =
            parse_rtf_strict(&format!(r"{{\rtf1{{\pict\wmetafile8 {wmf_hex}}}}}")).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(output.diagnostics.len(), 0);
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
    fn wmf_offsetcliprgn_after_painted_saved_clip_mutation_offsets_combined_clip() {
        let wmf_hex = concat!(
            "010009000003400000000100070000000000",
            "050000000c026400c800",
            "07000000fc020000dcdcdc000000",
            "040000002d010000",
            "030000001e00",
            "0700000016043c00640014001400",
            "070000001b042800280000000000",
            "0700000016044600780028001e00",
            "05000000200205000a00",
            "070000001b045000b4000a001400",
            "030000000000",
        );
        let output =
            parse_rtf_strict(&format!(r"{{\rtf1{{\pict\wmetafile8 {wmf_hex}}}}}")).unwrap();

        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
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
                left: 30.0,
                top: 40.0,
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
                left: 40.0,
                top: 45.0,
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
    fn wmf_polybezierto_with_partial_segments_becomes_blank_passive_vector() {
        let wmf_hex = concat!(
            "0100090000031e0000000100080000000000",
            "050000000c026400c800",
            "0500000014020a000a00",
            "08000000041002001e00050046003200",
            "030000000000",
        );
        let output = parse_rtf(&format!(r"{{\rtf1{{\pict\wmetafile8 {wmf_hex}}}}}")).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Image(image)
                if image.format == ImageFormat::WmfVector
                    && image.bytes.is_empty()
                    && image.vector_commands.is_empty()
        ));
    }

    #[test]
    fn wmf_stock_black_pen_uses_word_pdf_playback_width() {
        let wmf_hex = concat!(
            "0100090000032a0000000100070000000000",
            "050000000c026400c800",
            "07000000fc020000dcdcdc000000",
            "040000002d010000",
            "070000001b045000b4000a001400",
            "0700000018045a00be0014006400",
            "030000000000",
        );
        let output = parse_rtf(&format!(r"{{\rtf1{{\pict\wmetafile8 {wmf_hex}}}}}")).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected passive WMF vector image"),
        };

        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(matches!(
            image.vector_commands.as_slice(),
            [
                StaticImageVectorCommand::Rectangle { stroke_width, .. },
                StaticImageVectorCommand::Ellipse {
                    stroke_width: ellipse_width,
                    ..
                }
            ] if (*stroke_width - WMF_WORD_STOCK_PEN_LOGICAL_WIDTH).abs() < f32::EPSILON
                && (*ellipse_width - WMF_WORD_STOCK_PEN_LOGICAL_WIDTH).abs() < f32::EPSILON
        ));
    }
