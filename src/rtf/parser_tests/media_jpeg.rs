    #[test]
    fn normalizes_jpeg_picture_as_safe_static_image() {
        let jpeg = minimal_jpeg_with_dimensions(1, 1);
        let input = format!(
            "{{\\rtf1{{\\pict\\jpegblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&jpeg)
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };
        assert_eq!(image.format, ImageFormat::Jpeg);
        assert_eq!(image.width_px, 1);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.bytes, jpeg);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
        assert_eq!(image.scale_x_percent, None);
        assert_eq!(image.scale_y_percent, None);
    }

    #[test]
    fn normalizes_jpeg_picture_without_trailing_bytes() {
        let jpeg = minimal_jpeg_with_dimensions(1, 1);
        let mut payload = jpeg.clone();
        payload.extend_from_slice(b"TRAILING-JPEG-PAYLOAD /JavaScript");
        let input = format!(r"{{\rtf1{{\pict\jpegblip {}}}}}", bytes_to_hex(&payload));
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };
        assert_eq!(image.format, ImageFormat::Jpeg);
        assert_eq!(image.width_px, 1);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.bytes, jpeg);
    }

    #[test]
    fn normalizes_grayscale_jpeg_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\jpegblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_grayscale_jpeg_with_dimensions(1, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };
        assert_eq!(image.format, ImageFormat::JpegGrayscale);
        assert_eq!(image.width_px, 1);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
        assert_eq!(image.scale_x_percent, None);
        assert_eq!(image.scale_y_percent, None);
    }

    #[test]
    fn normalizes_cmyk_jpeg_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\jpegblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_cmyk_jpeg_with_dimensions(1, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };
        assert_eq!(image.format, ImageFormat::JpegCmyk);
        assert_eq!(image.width_px, 1);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
        assert_eq!(image.scale_x_percent, None);
        assert_eq!(image.scale_y_percent, None);
    }

    #[test]
    fn passive_jpeg_tone_adjustment_composes_inverted_rgb_endpoints() {
        assert_eq!(
            passive_jpeg_tone_adjustment(
                ImageFormat::JpegInverted,
                PictureAdjustments {
                    brightness_fixed: Some(32_768),
                    ..PictureAdjustments::default()
                },
            ),
            Some(ImageToneAdjustment {
                decode_low: 1.0,
                decode_high: 1.0,
            })
        );
    }

    #[test]
    fn passive_jpeg_tone_adjustment_composes_inverted_grayscale_and_cmyk_endpoints() {
        let adjustments = PictureAdjustments {
            brightness_fixed: Some(32_768),
            ..PictureAdjustments::default()
        };

        assert_eq!(
            passive_jpeg_tone_adjustment(ImageFormat::JpegGrayscaleInverted, adjustments),
            Some(ImageToneAdjustment {
                decode_low: 1.0,
                decode_high: 1.0,
            })
        );
        assert_eq!(
            passive_jpeg_tone_adjustment(ImageFormat::JpegCmykInverted, adjustments),
            Some(ImageToneAdjustment {
                decode_low: 1.0,
                decode_high: 1.0,
            })
        );
    }

    #[test]
    fn normalizes_bi_jpeg_dib_picture_as_safe_static_image() {
        let jpeg = minimal_jpeg_with_dimensions(2, 1);
        let mut dib = minimal_compressed_dib_with_payload(2, 1, 4, &jpeg);
        dib.extend_from_slice(b"TRAILING-DIB-JPEG-PAYLOAD");
        let input = format!(
            "{{\\rtf1{{\\pict\\dibitmap0\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&dib)
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Jpeg);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.bytes, jpeg);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
    }

    #[test]
    fn strict_spec_unsupported_jpeg_component_count_becomes_placeholder() {
        let output = parse_rtf_strict(&format!(
            "{{\\rtf1{{\\pict\\jpegblip {}}}}}",
            bytes_to_hex(&minimal_jpeg_with_components(1, 1, 2))
        ))
        .unwrap();
        assert!(matches!(
            &output.document.blocks[0],
            Block::Placeholder(text) if text.contains("malformed JPEG")
        ));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "malformed JPEG picture data replaced with bounded passive geometry placeholder",
            )
        }));
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("JPEG picture data was malformed")
        }));
    }

    #[test]
    fn word_compatible_malformed_jpeg_is_suppressed_inline_without_spacing() {
        let jpeg = minimal_jpeg_frame_header_with_components(2, 2, 3);
        let input = format!(
            "{{\\rtf1 Before JPEG.{{\\pict\\jpegblip {}}}After JPEG.\\par}}",
            bytes_to_hex(&jpeg)
        );
        let output = parse_rtf(&input).unwrap();

        assert!(
            output
                .document
                .blocks
                .iter()
                .all(|block| { !matches!(block, Block::Image(_) | Block::Placeholder(_)) })
        );
        assert!(output.document.inline_images.is_empty());
        assert_eq!(
            document_text(&output.document),
            "Before JPEG.\u{200b}After JPEG."
        );
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("malformed JPEG picture suppressed for Word-compatible passive rendering")
        }));
    }

    #[test]
    fn undecodable_jpeg_scan_is_suppressed_in_word_mode_and_placeholdered_in_strict_mode() {
        let mut jpeg = minimal_jpeg_frame_header_with_components(2, 2, 3);
        jpeg.truncate(jpeg.len() - 2);
        jpeg.extend_from_slice(&[
            0xff, 0xda, 0x00, 0x0c, 0x03, 0x01, 0x00, 0x02, 0x11, 0x03, 0x11, 0x00, 0x3f, 0x00,
            0xff, 0xd9,
        ]);
        assert!(
            parse_jpeg_image_data(&jpeg, RtfLimits::default().max_image_pixels).is_ok()
        );
        let input = format!("{{\\rtf1{{\\pict\\jpegblip {}}}}}", bytes_to_hex(&jpeg));

        let word = parse_rtf(&input).unwrap();
        assert!(
            word.document
                .blocks
                .iter()
                .all(|block| !matches!(block, Block::Image(_) | Block::Placeholder(_)))
        );
        assert!(word.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("malformed JPEG picture suppressed for Word-compatible passive rendering")
        }));

        let strict = parse_rtf_strict(&input).unwrap();
        assert!(matches!(
            strict.document.blocks.first(),
            Some(Block::Placeholder(text)) if text.contains("malformed JPEG")
        ));
    }
