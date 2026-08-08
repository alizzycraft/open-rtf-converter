    #[test]
    fn normalizes_rgb_png_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picscalex50\\picscaley200 {}}}}}",
            bytes_to_hex(&minimal_rgb_png_with_dimensions(1, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };
        assert_eq!(image.format, ImageFormat::Png);
        assert_eq!(image.width_px, 1);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.display_width_twips, None);
        assert_eq!(image.display_height_twips, None);
        assert_eq!(image.scale_x_percent, Some(50));
        assert_eq!(image.scale_y_percent, Some(200));
        assert!(!image.bytes.is_empty());
    }

    #[test]
    fn rgb_png_grayscale_metadata_updates_stored_scanlines() {
        let input = [
            r"{\rtf1{\pict\pngblip{\picprop{\sp{\sn pictureGray}{\sv 1}}}\picwgoal720\pichgoal720 ",
            bytes_to_hex(&minimal_rgb_png_with_pixel(255, 0, 0)).as_str(),
            "}}",
        ]
        .concat();
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Png);
        assert!(
            output.diagnostics.iter().all(|diagnostic| !diagnostic
                .message
                .contains("picture grayscale property approximated")),
            "RGB PNG grayscale should be applied to stored scanlines: {:?}",
            output.diagnostics
        );
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("PNG picture grayscale property rendered as bounded passive image pixels")
        }));
        let scanlines = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 4)
            .expect("PNG scanline");
        assert_eq!(scanlines, vec![0, 54, 54, 54]);
    }

    #[test]
    fn rgb_png_grayscale_metadata_updates_compressed_filtered_scanlines() {
        let input = [
            r"{\rtf1{\pict\pngblip{\picprop{\sp{\sn pictureGray}{\sv 1}}}\picwgoal720\pichgoal720 ",
            bytes_to_hex(&minimal_compressed_subfiltered_rgb_png()).as_str(),
            "}}",
        ]
        .concat();
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Png);
        assert!(
            output.diagnostics.iter().all(|diagnostic| !diagnostic
                .message
                .contains("picture grayscale property approximated")),
            "compressed RGB PNG grayscale should be applied after unfiltering: {:?}",
            output.diagnostics
        );
        let scanlines = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 10)
            .expect("compressed PNG scanline");
        assert_eq!(scanlines, vec![0, 54, 54, 54, 182, 182, 182, 18, 18, 18]);
    }

    #[test]
    fn grayscale_png_bilevel_metadata_updates_stored_scanlines() {
        let input = [
            r"{\rtf1{\pict\pngblip{\picprop{\sp{\sn pictureBiLevel}{\sv 1}}}\picwgoal720\pichgoal720 ",
            bytes_to_hex(&minimal_valid_grayscale_png_with_sample(128)).as_str(),
            "}}",
        ]
        .concat();
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::PngGrayscale);
        assert!(
            output.diagnostics.iter().all(|diagnostic| !diagnostic
                .message
                .contains("picture bilevel property approximated")),
            "grayscale PNG bilevel should be applied to stored scanlines: {:?}",
            output.diagnostics
        );
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("PNG picture bilevel property rendered as bounded passive image pixels")
        }));
        let scanlines = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 2)
            .expect("PNG scanline");
        assert_eq!(scanlines, vec![0, 255]);
    }

    #[test]
    fn rgb_png_brightness_contrast_metadata_updates_stored_scanlines() {
        let input = [
            r"{\rtf1{\pict\pngblip{\picprop{\sp{\sn pictureBrightness}{\sv 32768}}{\sp{\sn pictureContrast}{\sv 131072}}}\picwgoal720\pichgoal720 ",
            bytes_to_hex(&minimal_rgb_png_with_pixel(128, 64, 192)).as_str(),
            "}}",
        ]
        .concat();
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Png);
        assert!(
            output.diagnostics.iter().all(|diagnostic| !diagnostic
                .message
                .contains("picture brightness/contrast property approximated")),
            "RGB PNG brightness/contrast should be applied to stored scanlines: {:?}",
            output.diagnostics
        );
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "PNG picture brightness/contrast property rendered as bounded passive image pixels",
            )
        }));
        let scanlines = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 4)
            .expect("PNG scanline");
        assert_eq!(scanlines, vec![0, 255, 255, 255]);
    }

    #[test]
    fn normalizes_grayscale_png_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_grayscale_png_with_dimensions(1, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::PngGrayscale);
        assert_eq!(image.width_px, 1);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
        assert!(!image.bytes.is_empty());
    }

    #[test]
    fn normalizes_indexed_png_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_indexed_png_with_dimensions(1, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::PngIndexed);
        assert_eq!(image.width_px, 1);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
        assert_eq!(image.palette, vec![255, 0, 0, 0, 255, 0]);
        assert!(!image.bytes.is_empty());
    }

    #[test]
    fn normalizes_rgba_png_picture_as_rgb_with_passive_alpha_mask() {
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_rgba_png(&[
                [255, 0, 0, 255],
                [0, 0, 255, 128],
                [0, 0, 0, 0],
            ]))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Png);
        assert_eq!(image.width_px, 3);
        assert_eq!(image.height_px, 1);
        let scanlines = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 10)
            .expect("normalized RGBA PNG scanline");
        assert_eq!(scanlines, vec![0, 255, 0, 0, 0, 0, 255, 0, 0, 0]);
        let alpha_mask = image.alpha_mask.as_ref().expect("RGBA alpha mask");
        let mask_scanlines =
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&alpha_mask.bytes, 4)
                .expect("normalized RGBA alpha mask scanline");
        assert_eq!(mask_scanlines, vec![0, 255, 128, 0]);
    }

    #[test]
    fn normalizes_grayscale_alpha_png_picture_with_passive_alpha_mask() {
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_grayscale_alpha_png(&[
                [0, 255],
                [0, 128],
                [255, 0]
            ]))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::PngGrayscale);
        assert_eq!(image.width_px, 3);
        assert_eq!(image.height_px, 1);
        let scanlines = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 4)
            .expect("normalized grayscale-alpha PNG scanline");
        assert_eq!(scanlines, vec![0, 0, 0, 255]);
        let alpha_mask = image
            .alpha_mask
            .as_ref()
            .expect("grayscale-alpha PNG alpha mask");
        let mask_scanlines =
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&alpha_mask.bytes, 4)
                .expect("normalized grayscale-alpha PNG mask scanline");
        assert_eq!(mask_scanlines, vec![0, 255, 128, 0]);
    }

    #[test]
    fn normalizes_16bit_rgb_png_picture_as_8bit_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_16bit_rgb_png(&[
                [0, 65_535, 32_768],
                [65_535, 0, 0],
            ]))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Png);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        let scanlines = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 7)
            .expect("normalized 16-bit RGB PNG scanline");
        assert_eq!(scanlines, vec![0, 0, 255, 128, 255, 0, 0]);
    }

    #[test]
    fn normalizes_16bit_grayscale_png_picture_as_8bit_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_16bit_grayscale_png(&[0, 32_768, 65_535]))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::PngGrayscale);
        assert_eq!(image.width_px, 3);
        assert_eq!(image.height_px, 1);
        let scanlines = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 4)
            .expect("normalized 16-bit grayscale PNG scanline");
        assert_eq!(scanlines, vec![0, 0, 128, 255]);
    }

    #[test]
    fn normalizes_16bit_rgba_png_picture_as_rgb_with_passive_alpha_mask() {
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_16bit_rgba_png(&[
                [65_535, 0, 0, 65_535],
                [0, 0, 65_535, 32_768],
                [0, 0, 0, 0],
            ]))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Png);
        assert_eq!(image.width_px, 3);
        assert_eq!(image.height_px, 1);
        let scanlines = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 10)
            .expect("normalized 16-bit RGBA PNG scanline");
        assert_eq!(scanlines, vec![0, 255, 0, 0, 0, 0, 255, 0, 0, 0]);
        let alpha_mask = image.alpha_mask.as_ref().expect("16-bit RGBA alpha mask");
        let mask_scanlines =
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&alpha_mask.bytes, 4)
                .expect("normalized 16-bit RGBA mask scanline");
        assert_eq!(mask_scanlines, vec![0, 255, 128, 0]);
    }

    #[test]
    fn normalizes_16bit_grayscale_alpha_png_picture_with_passive_alpha_mask() {
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_16bit_grayscale_alpha_png(&[
                [0, 65_535],
                [0, 32_768],
                [65_535, 0],
            ]))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::PngGrayscale);
        assert_eq!(image.width_px, 3);
        assert_eq!(image.height_px, 1);
        let scanlines = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 4)
            .expect("normalized 16-bit grayscale-alpha PNG scanline");
        assert_eq!(scanlines, vec![0, 0, 0, 255]);
        let alpha_mask = image
            .alpha_mask
            .as_ref()
            .expect("16-bit grayscale-alpha PNG alpha mask");
        let mask_scanlines =
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&alpha_mask.bytes, 4)
                .expect("normalized 16-bit grayscale-alpha PNG mask scanline");
        assert_eq!(mask_scanlines, vec![0, 255, 128, 0]);
    }

    #[test]
    fn normalizes_indexed_png_transparency_chunk_into_passive_alpha_mask() {
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_indexed_png_with_transparency(&[0, 128]))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::PngIndexed);
        assert_eq!(image.palette, vec![255, 0, 0, 0, 255, 0]);
        let scanlines = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 2)
            .expect("indexed PNG scanline");
        assert_eq!(scanlines, vec![0, 1]);
        let alpha_mask = image.alpha_mask.as_ref().expect("indexed tRNS alpha mask");
        let mask_scanlines =
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&alpha_mask.bytes, 2)
                .expect("indexed PNG tRNS mask scanline");
        assert_eq!(mask_scanlines, vec![0, 128]);
    }

    #[test]
    fn normalizes_rgb_png_transparency_chunk_into_passive_alpha_mask() {
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_rgb_png_with_transparency(
                &[[255, 0, 0], [0, 255, 0]],
                [255, 0, 0]
            ))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Png);
        let scanlines = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 7)
            .expect("RGB PNG scanline");
        assert_eq!(scanlines, vec![0, 255, 0, 0, 0, 255, 0]);
        let alpha_mask = image.alpha_mask.as_ref().expect("RGB tRNS alpha mask");
        let mask_scanlines =
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&alpha_mask.bytes, 3)
                .expect("RGB PNG tRNS mask scanline");
        assert_eq!(mask_scanlines, vec![0, 0, 255]);
    }

    #[test]
    fn normalizes_grayscale_png_transparency_chunk_into_passive_alpha_mask() {
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_grayscale_png_with_transparency(
                &[0, 128, 255],
                128
            ))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::PngGrayscale);
        let scanlines = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 4)
            .expect("grayscale PNG scanline");
        assert_eq!(scanlines, vec![0, 0, 128, 255]);
        let alpha_mask = image
            .alpha_mask
            .as_ref()
            .expect("grayscale tRNS alpha mask");
        let mask_scanlines =
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&alpha_mask.bytes, 4)
                .expect("grayscale PNG tRNS mask scanline");
        assert_eq!(mask_scanlines, vec![0, 255, 0, 255]);
    }

    #[test]
    fn normalizes_interlaced_rgb_png_picture_as_safe_static_image() {
        let width = 5usize;
        let height = 5usize;
        let mut pixels = Vec::new();
        for y in 0..height {
            for x in 0..width {
                pixels.extend_from_slice(&[(x * 40) as u8, (y * 40) as u8, ((x + y) * 20) as u8]);
            }
        }
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_interlaced_png(width, height, 2, None, &pixels))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Png);
        assert_eq!(image.width_px, width as u32);
        assert_eq!(image.height_px, height as u32);
        let expected = png_scanlines_from_pixels(width, height, 3, &pixels);
        let scanlines =
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, expected.len())
                .expect("interlaced RGB PNG scanlines");
        assert_eq!(scanlines, expected);
    }

    #[test]
    fn normalizes_interlaced_grayscale_png_picture_as_safe_static_image() {
        let width = 5usize;
        let height = 5usize;
        let pixels = (0..width * height)
            .map(|index| (index * 7) as u8)
            .collect::<Vec<_>>();
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_interlaced_png(width, height, 0, None, &pixels))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::PngGrayscale);
        let expected = png_scanlines_from_pixels(width, height, 1, &pixels);
        let scanlines =
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, expected.len())
                .expect("interlaced grayscale PNG scanlines");
        assert_eq!(scanlines, expected);
    }

    #[test]
    fn normalizes_interlaced_indexed_png_picture_as_safe_static_image() {
        let width = 5usize;
        let height = 5usize;
        let pixels = (0..width * height)
            .map(|index| (index % 2) as u8)
            .collect::<Vec<_>>();
        let palette = [255, 0, 0, 0, 255, 0];
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_interlaced_png(
                width,
                height,
                3,
                Some(&palette),
                &pixels
            ))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::PngIndexed);
        assert_eq!(image.palette, palette);
        let expected = png_scanlines_from_pixels(width, height, 1, &pixels);
        let scanlines =
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, expected.len())
                .expect("interlaced indexed PNG scanlines");
        assert_eq!(scanlines, expected);
    }

    #[test]
    fn normalizes_interlaced_low_bit_indexed_png_picture_as_8bit_safe_static_image() {
        let width = 5usize;
        let height = 5usize;
        let pixels = (0..width * height)
            .map(|index| (index % 2) as u8)
            .collect::<Vec<_>>();
        let palette = [255, 0, 0, 0, 255, 0];
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_interlaced_low_bit_png(
                width,
                height,
                1,
                3,
                Some(&palette),
                &pixels
            ))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::PngIndexed);
        assert_eq!(image.palette, palette);
        let expected = png_scanlines_from_pixels(width, height, 1, &pixels);
        let scanlines =
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, expected.len())
                .expect("interlaced low-bit indexed PNG scanlines");
        assert_eq!(scanlines, expected);
    }

    #[test]
    fn normalizes_interlaced_low_bit_grayscale_png_picture_as_8bit_safe_static_image() {
        let width = 5usize;
        let height = 5usize;
        let samples = (0..width * height)
            .map(|index| (index % 16) as u8)
            .collect::<Vec<_>>();
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_interlaced_low_bit_png(
                width, height, 4, 0, None, &samples
            ))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::PngGrayscale);
        let expected_pixels = samples
            .iter()
            .map(|sample| ((u16::from(*sample) * 255) / 15) as u8)
            .collect::<Vec<_>>();
        let expected = png_scanlines_from_pixels(width, height, 1, &expected_pixels);
        let scanlines =
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, expected.len())
                .expect("interlaced low-bit grayscale PNG scanlines");
        assert_eq!(scanlines, expected);
    }

    #[test]
    fn normalizes_interlaced_16bit_rgb_png_picture_as_8bit_safe_static_image() {
        let width = 5usize;
        let height = 5usize;
        let mut pixels = Vec::new();
        for y in 0..height {
            for x in 0..width {
                pixels.push([
                    (x as u16) * 10_000,
                    (y as u16) * 10_000,
                    ((x + y) as u16) * 5_000,
                ]);
            }
        }
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_interlaced_16bit_png(width, height, 2, &pixels))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Png);
        assert_eq!(image.width_px, width as u32);
        assert_eq!(image.height_px, height as u32);
        let expected_pixels =
            png_16bit_samples_to_8bit_pixels(pixels.iter().flat_map(|pixel| pixel.iter().copied()));
        let expected = png_scanlines_from_pixels(width, height, 3, &expected_pixels);
        let scanlines =
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, expected.len())
                .expect("interlaced 16-bit RGB PNG scanlines");
        assert_eq!(scanlines, expected);
    }

    #[test]
    fn normalizes_interlaced_16bit_grayscale_png_picture_as_8bit_safe_static_image() {
        let width = 5usize;
        let height = 5usize;
        let samples = (0..width * height)
            .map(|index| (index as u16) * 2_500)
            .collect::<Vec<_>>();
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_interlaced_16bit_grayscale_png(
                width, height, &samples
            ))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::PngGrayscale);
        let expected_pixels = png_16bit_samples_to_8bit_pixels(samples.iter().copied());
        let expected = png_scanlines_from_pixels(width, height, 1, &expected_pixels);
        let scanlines =
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, expected.len())
                .expect("interlaced 16-bit grayscale PNG scanlines");
        assert_eq!(scanlines, expected);
    }

    #[test]
    fn normalizes_interlaced_16bit_rgba_png_picture_as_rgb_with_passive_alpha_mask() {
        let width = 5usize;
        let height = 5usize;
        let mut pixels = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let alpha = if (x + y) % 2 == 0 { 65_535 } else { 32_768 };
                pixels.push([(x as u16) * 10_000, 0, (y as u16) * 10_000, alpha]);
            }
        }
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_interlaced_16bit_png(width, height, 6, &pixels))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Png);
        let mut expected_pixels = Vec::new();
        let mut expected_alpha = Vec::new();
        for pixel in &pixels {
            expected_pixels.push(scale_png_16_to_8(pixel[0]));
            expected_pixels.push(scale_png_16_to_8(pixel[1]));
            expected_pixels.push(scale_png_16_to_8(pixel[2]));
            expected_alpha.push(scale_png_16_to_8(pixel[3]));
        }
        let expected = png_scanlines_from_pixels(width, height, 3, &expected_pixels);
        let scanlines =
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, expected.len())
                .expect("interlaced 16-bit RGBA PNG scanlines");
        assert_eq!(scanlines, expected);
        let expected_mask = png_scanlines_from_pixels(width, height, 1, &expected_alpha);
        let alpha_mask = image
            .alpha_mask
            .as_ref()
            .expect("interlaced 16-bit RGBA alpha mask");
        let mask_scanlines = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(
            &alpha_mask.bytes,
            expected_mask.len(),
        )
        .expect("interlaced 16-bit RGBA mask scanlines");
        assert_eq!(mask_scanlines, expected_mask);
    }

    #[test]
    fn normalizes_low_bit_indexed_png_picture_as_8bit_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_low_bit_indexed_png(&[0, 1, 0], 1, 2))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::PngIndexed);
        assert_eq!(image.width_px, 3);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.palette, vec![255, 0, 0, 0, 255, 0]);
        let scanlines = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 4)
            .expect("normalized indexed PNG scanline");
        assert_eq!(scanlines, vec![0, 0, 1, 0]);
    }

    #[test]
    fn normalizes_low_bit_grayscale_png_picture_as_8bit_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_low_bit_grayscale_png(&[0, 8, 15], 4))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::PngGrayscale);
        assert_eq!(image.width_px, 3);
        assert_eq!(image.height_px, 1);
        let scanlines = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&image.bytes, 4)
            .expect("normalized grayscale PNG scanline");
        assert_eq!(scanlines, vec![0, 0, 136, 255]);
    }

    #[test]
    fn low_bit_indexed_png_with_out_of_palette_sample_becomes_placeholder() {
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_low_bit_indexed_png(&[1], 1, 1))
        );
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Placeholder(text) if text.contains("unsupported PNG")
        ));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("unsupported or malformed PNG picture data replaced with bounded passive geometry placeholder")
        }));
    }

    #[test]
    fn normalizes_bi_png_dib_picture_as_safe_static_image() {
        let png = minimal_rgb_png_with_dimensions(2, 1);
        let input = format!(
            "{{\\rtf1{{\\pict\\dibitmap0\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_compressed_dib_with_payload(2, 1, 5, &png))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Png);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        assert!(image.palette.is_empty());
        assert!(image.alpha_mask.is_none());
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
    }

    #[test]
    fn malformed_png_picture_becomes_placeholder() {
        let output = parse_rtf(r"{\rtf1{\pict\pngblip 89504e470d0a1a0a}}").unwrap();
        assert!(matches!(
            &output.document.blocks[0],
            Block::Placeholder(text) if text.contains("unsupported PNG")
        ));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("unsupported or malformed PNG picture data replaced with bounded passive geometry placeholder")
        }));
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("PNG picture data was unsupported or malformed")
        }));
    }

    #[test]
    fn indexed_png_without_palette_becomes_placeholder() {
        let output = parse_rtf(&format!(
            "{{\\rtf1{{\\pict\\pngblip {}}}}}",
            bytes_to_hex(&minimal_indexed_png_without_palette(1, 1))
        ))
        .unwrap();
        assert!(matches!(
            &output.document.blocks[0],
            Block::Placeholder(text) if text.contains("unsupported PNG")
        ));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("unsupported or malformed PNG picture data replaced with bounded passive geometry placeholder")
        }));
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("PNG picture data was unsupported or malformed")
        }));
    }

    #[test]
    fn png_image_pixel_limit_is_enforced() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_image_pixels: 10,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip {}}}}}",
            bytes_to_hex(&minimal_rgb_png_with_dimensions(11, 1))
        );
        assert!(matches!(
            parse_rtf_bytes_with_options(input.as_bytes(), &options),
            Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "image pixels"
        ));
    }
