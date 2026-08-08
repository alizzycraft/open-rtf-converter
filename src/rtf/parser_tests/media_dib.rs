    #[test]
    fn explicit_cell_borders_do_not_propagate_to_a_borderless_neighbor() {
        let output = parse_rtf(
            r"{\rtf1\trowd\clbrdrt\brdrs\brdrw20\clbrdrl\brdrs\brdrw20\clbrdrb\brdrs\brdrw20\clbrdrr\brdrs\brdrw20\cellx2400 Bordered\cell\cellx4800 Borderless\cell\row}",
        )
        .unwrap();
        let Block::Table(table) = &output.document.blocks[0] else {
            panic!("expected table");
        };
        let second = table.rows[0].cells[1].borders;
        assert!(!second.top.visible);
        assert!(!second.right.visible);
        assert!(!second.bottom.visible);
        assert!(!second.left.visible);
    }

    #[test]
    fn word_compatible_parameterless_bar_tab_does_not_modify_following_tab_stop() {
        let output = parse_rtf(r"{\rtf1\tb\tx720\tx1440 Bar tab\tab text\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.tab_stops_twips, vec![720, 1440]);
        assert_eq!(
            paragraph.style.tab_stop_alignments,
            vec![TabAlignment::Left, TabAlignment::Left]
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("parameterless bar tab ignored"))
        );
    }

    #[test]
    fn word_compatible_parameterless_auto_spacing_does_not_synthesize_a_gap() {
        let output =
            parse_rtf(r"{\rtf1\sb0\sbauto\sa0\saauto Auto\par\sbauto0\saauto0 Manual\par}")
                .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected first paragraph"),
        };

        assert!(!first.style.auto_space_before);
        assert!(!first.style.auto_space_after);
    }

    #[test]
    fn dib_picture_brightness_metadata_updates_safe_rgb_bytes() {
        let input = format!(
            r"{{\rtf1{{\pict\dibitmap0{{\picprop{{\sp{{\sn pictureBrightness}}{{\sv -32768}}}}}}\picwgoal720\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_24bit_dib_with_rgb_pixels(
                2,
                1,
                &[[128, 64, 192], [32, 96, 160]]
            ))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Rgb8);
        assert_eq!(image.bytes, vec![0, 0, 0, 0, 0, 0]);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "DIB picture brightness/contrast property rendered as bounded passive image pixels",
            )
        }));
    }

    #[test]
    fn normalizes_uncompressed_dib_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\dibitmap0\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_24bit_dib_with_dimensions(2, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Rgb8);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0]);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
    }

    #[test]
    fn normalizes_16bit_dib_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\dibitmap0\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_16bit_dib_with_dimensions(2, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Rgb8);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0]);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
    }

    #[test]
    fn normalizes_16bit_bitfields_dib_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\dibitmap0\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_16bit_bitfields_dib_with_dimensions(2, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Rgb8);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0]);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
    }

    #[test]
    fn normalizes_8bit_paletted_dib_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\dibitmap0\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_8bit_dib_with_dimensions(2, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Rgb8);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0]);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
    }

    #[test]
    fn normalizes_rle8_dib_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\dibitmap0\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_rle8_dib_with_dimensions(3, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Rgb8);
        assert_eq!(image.width_px, 3);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0, 255, 0, 0]);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
    }

    #[test]
    fn normalizes_rle4_dib_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\dibitmap0\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_rle4_dib_with_dimensions(3, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Rgb8);
        assert_eq!(image.width_px, 3);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0, 255, 0, 0]);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
    }

    #[test]
    fn normalizes_4bit_paletted_dib_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\dibitmap0\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_4bit_dib_with_dimensions(2, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Rgb8);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0]);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
    }

    #[test]
    fn normalizes_1bit_paletted_dib_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\dibitmap0\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_1bit_dib_with_dimensions(2, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Rgb8);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0]);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
    }

    #[test]
    fn normalizes_bitmap_core_dib_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\dibitmap0\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_24bit_bitmap_core_dib_with_dimensions(2, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Rgb8);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0]);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
    }

    #[test]
    fn normalizes_paletted_bitmap_core_dib_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\dibitmap0\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_4bit_bitmap_core_dib_with_dimensions(2, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Rgb8);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0]);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
    }

    #[test]
    fn unsupported_dib_picture_becomes_placeholder() {
        let mut dib = minimal_24bit_dib_with_dimensions(1, 1);
        dib[16..20].copy_from_slice(&1u32.to_le_bytes());
        let input = format!("{{\\rtf1{{\\pict\\dibitmap0 {}}}}}", bytes_to_hex(&dib));
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Placeholder(text) if text.contains("unsupported DIB")
        ));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("unsupported or malformed DIB picture data replaced with bounded passive geometry placeholder")
        }));
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("DIB picture data was unsupported or malformed")
        }));
    }

    #[test]
    fn excessive_8bit_dib_palette_count_becomes_placeholder() {
        let mut dib = minimal_8bit_dib_with_dimensions(1, 1);
        dib[32..36].copy_from_slice(&257u32.to_le_bytes());
        let input = format!("{{\\rtf1{{\\pict\\dibitmap0 {}}}}}", bytes_to_hex(&dib));
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Placeholder(text) if text.contains("unsupported DIB")
        ));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("unsupported or malformed DIB picture data replaced with bounded passive geometry placeholder")
        }));
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("DIB picture data was unsupported or malformed")
        }));
    }
