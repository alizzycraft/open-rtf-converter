    #[test]
    fn estimated_pdf_object_count_matches_rendered_supplied_font_and_image_objects() {
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
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: ParagraphStyle::default(),
                runs: vec![Run {
                    dynamic_field: None,
                    text: "Supplied font".to_string(),
                    style,
                }],
            }),
            Block::Image(StaticImage {
                format: ImageFormat::Rgb8,
                bytes: vec![255, 0, 0, 0, 0, 255],
                palette: Vec::new(),
                alpha_mask: None,
                tone_adjustment: None,
                vector_commands: Vec::new(),
                width_px: 2,
                height_px: 1,
                natural_width_px_hint: None,
                natural_height_px_hint: None,
                display_width_twips: Some(720),
                display_height_twips: Some(720),
                scale_x_percent: None,
                scale_y_percent: None,
                crop: ImageCrop::default(),
                placement: None,
            }),
        ];
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

        assert_eq!(estimated, parsed.objects.len());
        assert_eq!(estimated, 11);
        assert!(
            pdf.windows(b"/FontFile2".len())
                .any(|window| window == b"/FontFile2")
        );
        assert!(
            pdf.windows(b"/Subtype /Image".len())
                .any(|window| window == b"/Subtype /Image")
        );
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn drawing_wrapped_placeholder_image_renders_passive_label() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Placeholder,
            bytes: Vec::new(),
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(2160),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];
        let mut layout = LayoutEngine::layout(&document);
        let image_index = layout.pages[0]
            .items
            .iter()
            .position(|item| matches!(item, LayoutItem::Image(_)))
            .expect("laid-out placeholder image");
        let image = layout.pages[0].items[image_index].clone();
        layout.pages[0].items[image_index] = LayoutItem::Drawing(DrawingFragment {
            z_order: 7,
            below_text: false,
            item: Box::new(image),
        });

        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Image skipped");
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "S"),
            "drawing-wrapped placeholder image should render passive frame"
        );
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn compact_placeholder_image_renders_label_when_it_fits() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Placeholder,
            bytes: Vec::new(),
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(1600),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Image skipped");
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn drawing_wrapped_rgb_image_writes_passive_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Rgb8,
            bytes: vec![255, 0, 0, 0, 0, 255],
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 2,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(1440),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];
        let mut layout = LayoutEngine::layout(&document);
        let image_index = layout.pages[0]
            .items
            .iter()
            .position(|item| matches!(item, LayoutItem::Image(_)))
            .expect("laid-out RGB image");
        let image = layout.pages[0].items[image_index].clone();
        layout.pages[0].items[image_index] = LayoutItem::Drawing(DrawingFragment {
            z_order: 7,
            below_text: false,
            item: Box::new(image),
        });

        let estimated = estimate_passive_pdf_object_count(&layout, None);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(estimated, parsed.objects.len());
        assert!(
            pdf.windows(b"/Subtype /Image".len())
                .any(|window| window == b"/Subtype /Image")
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "Do"),
            "drawing-wrapped RGB image should invoke a passive image XObject"
        );
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn nested_drawing_wrapped_rgb_image_writes_passive_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Rgb8,
            bytes: vec![255, 0, 0, 0, 0, 255],
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 2,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(1440),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];
        let mut layout = LayoutEngine::layout(&document);
        let image_index = layout.pages[0]
            .items
            .iter()
            .position(|item| matches!(item, LayoutItem::Image(_)))
            .expect("laid-out RGB image");
        let image = layout.pages[0].items[image_index].clone();
        layout.pages[0].items[image_index] = LayoutItem::Drawing(DrawingFragment {
            z_order: 9,
            below_text: false,
            item: Box::new(LayoutItem::Drawing(DrawingFragment {
                z_order: 7,
                below_text: false,
                item: Box::new(image),
            })),
        });

        let estimated = estimate_passive_pdf_object_count(&layout, None);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(estimated, parsed.objects.len());
        assert!(
            pdf.windows(b"/Subtype /Image".len())
                .any(|window| window == b"/Subtype /Image")
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "Do"),
            "nested drawing-wrapped RGB image should invoke a passive image XObject"
        );
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn deep_drawing_wrapped_rgb_image_renders_with_stack_bounded_traversal() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Rgb8,
            bytes: vec![255, 0, 0, 0, 0, 255],
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 2,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(1440),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];
        let mut layout = LayoutEngine::layout(&document);
        let image_index = layout.pages[0]
            .items
            .iter()
            .position(|item| matches!(item, LayoutItem::Image(_)))
            .expect("laid-out RGB image");
        let mut image = layout.pages[0].items[image_index].clone();
        for z_order in 0..512 {
            image = LayoutItem::Drawing(DrawingFragment {
                z_order,
                below_text: false,
                item: Box::new(image),
            });
        }
        layout.pages[0].items[image_index] = image;

        let estimated = estimate_passive_pdf_object_count(&layout, None);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(estimated, parsed.objects.len());
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "Do"),
            "deep drawing-wrapped RGB image should invoke a passive image XObject"
        );
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn writes_passive_jpeg_image_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Jpeg,
            bytes: minimal_jpeg_with_dimensions(1, 1),
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/Subtype /Image".len())
                .any(|window| window == b"/Subtype /Image")
        );
        assert!(
            pdf.windows(b"/DCTDecode".len())
                .any(|window| window == b"/DCTDecode")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_passive_grayscale_jpeg_image_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::JpegGrayscale,
            bytes: minimal_grayscale_jpeg_with_dimensions(1, 1),
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/ColorSpace /DeviceGray".len())
                .any(|window| window == b"/ColorSpace /DeviceGray")
        );
        assert!(
            pdf.windows(b"/DCTDecode".len())
                .any(|window| window == b"/DCTDecode")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_passive_inverted_jpeg_image_xobject_with_decode_array() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::JpegInverted,
            bytes: minimal_jpeg_with_dimensions(1, 1),
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/ColorSpace /DeviceRGB".len())
                .any(|window| window == b"/ColorSpace /DeviceRGB")
        );
        assert!(
            pdf.windows(b"/DCTDecode".len())
                .any(|window| window == b"/DCTDecode")
        );
        assert!(
            pdf.windows(b"/Decode [1 0 1 0 1 0]".len())
                .any(|window| window == b"/Decode [1 0 1 0 1 0]")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_passive_inverted_jpeg_tone_adjustment_as_composed_decode_array() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::JpegInverted,
            bytes: minimal_jpeg_with_dimensions(1, 1),
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: Some(ImageToneAdjustment {
                decode_low: 1.0,
                decode_high: 127.0 / 255.0,
            }),
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/Decode [1 0.49803922 1 0.49803922 1 0.49803922]".len())
                .any(|window| window == b"/Decode [1 0.49803922 1 0.49803922 1 0.49803922]")
        );
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn writes_passive_jpeg_tone_adjustment_as_decode_array() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Jpeg,
            bytes: minimal_jpeg_with_dimensions(1, 1),
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: Some(ImageToneAdjustment {
                decode_low: 128.0 / 255.0,
                decode_high: 1.0,
            }),
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/Decode [0.5019608 1 0.5019608 1 0.5019608 1]".len())
                .any(|window| window == b"/Decode [0.5019608 1 0.5019608 1 0.5019608 1]")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
    }

    #[test]
    fn writes_passive_grayscale_jpeg_tone_adjustment_as_decode_array() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::JpegGrayscale,
            bytes: minimal_grayscale_jpeg_with_dimensions(1, 1),
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: Some(ImageToneAdjustment {
                decode_low: 0.0,
                decode_high: 128.0 / 255.0,
            }),
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/ColorSpace /DeviceGray".len())
                .any(|window| window == b"/ColorSpace /DeviceGray")
        );
        assert!(
            pdf.windows(b"/Decode [0 0.5019608]".len())
                .any(|window| window == b"/Decode [0 0.5019608]")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
    }

    #[test]
    fn writes_passive_inverted_grayscale_jpeg_tone_adjustment_as_composed_decode_array() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::JpegGrayscaleInverted,
            bytes: minimal_grayscale_jpeg_with_dimensions(1, 1),
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: Some(ImageToneAdjustment {
                decode_low: 1.0,
                decode_high: 127.0 / 255.0,
            }),
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/ColorSpace /DeviceGray".len())
                .any(|window| window == b"/ColorSpace /DeviceGray")
        );
        assert!(
            pdf.windows(b"/Decode [1 0.49803922]".len())
                .any(|window| window == b"/Decode [1 0.49803922]")
        );
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn writes_passive_cmyk_jpeg_tone_adjustment_as_decode_array() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::JpegCmyk,
            bytes: minimal_cmyk_jpeg_with_dimensions(1, 1),
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: Some(ImageToneAdjustment {
                decode_low: 128.0 / 255.0,
                decode_high: 1.0,
            }),
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/ColorSpace /DeviceCMYK".len())
                .any(|window| window == b"/ColorSpace /DeviceCMYK")
        );
        assert!(
            pdf.windows(b"/Decode [0.5019608 1 0.5019608 1 0.5019608 1 0.5019608 1]".len())
                .any(
                    |window| window == b"/Decode [0.5019608 1 0.5019608 1 0.5019608 1 0.5019608 1]"
                )
        );
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn writes_passive_inverted_cmyk_jpeg_tone_adjustment_as_composed_decode_array() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::JpegCmykInverted,
            bytes: minimal_cmyk_jpeg_with_dimensions(1, 1),
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: Some(ImageToneAdjustment {
                decode_low: 1.0,
                decode_high: 127.0 / 255.0,
            }),
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/ColorSpace /DeviceCMYK".len())
                .any(|window| window == b"/ColorSpace /DeviceCMYK")
        );
        assert!(
            pdf.windows(b"/Decode [1 0.49803922 1 0.49803922 1 0.49803922 1 0.49803922]".len())
                .any(|window| window
                    == b"/Decode [1 0.49803922 1 0.49803922 1 0.49803922 1 0.49803922]")
        );
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn writes_passive_rgb_jpeg_tone_before_luminosity_blend() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::JpegPassiveGrayscale,
            bytes: minimal_jpeg_with_dimensions(1, 1),
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: Some(ImageToneAdjustment {
                decode_low: 127.0 / 255.0,
                decode_high: 1.0,
            }),
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/Decode [0.49803922 1 0.49803922 1 0.49803922 1]".len())
                .any(|window| window == b"/Decode [0.49803922 1 0.49803922 1 0.49803922 1]")
        );
        assert!(
            pdf.windows(b"/BM /Luminosity".len())
                .any(|window| window == b"/BM /Luminosity")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
    }

    #[test]
    fn writes_passive_cmyk_jpeg_tone_before_luminosity_blend() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::JpegCmykPassiveGrayscale,
            bytes: minimal_cmyk_jpeg_with_dimensions(1, 1),
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: Some(ImageToneAdjustment {
                decode_low: 127.0 / 255.0,
                decode_high: 1.0,
            }),
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/ColorSpace /DeviceCMYK".len())
                .any(|window| window == b"/ColorSpace /DeviceCMYK")
        );
        assert!(
            pdf.windows(b"/Decode [0.49803922 1 0.49803922 1 0.49803922 1 0.49803922 1]".len())
                .any(|window| window
                    == b"/Decode [0.49803922 1 0.49803922 1 0.49803922 1 0.49803922 1]")
        );
        assert!(
            pdf.windows(b"/BM /Luminosity".len())
                .any(|window| window == b"/BM /Luminosity")
        );
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn writes_passive_jpeg_luminosity_blend_for_authored_grayscale_metadata() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::JpegPassiveGrayscale,
            bytes: minimal_jpeg_with_dimensions(1, 1),
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();
        assert!(
            pdf.windows(b"/ColorSpace /DeviceRGB".len())
                .any(|window| window == b"/ColorSpace /DeviceRGB")
        );
        assert!(
            pdf.windows(b"/DCTDecode".len())
                .any(|window| window == b"/DCTDecode")
        );
        assert!(
            pdf.windows(b"/BM /Luminosity".len())
                .any(|window| window == b"/BM /Luminosity")
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "gs"
                && operation
                    .operands
                    .first()
                    .and_then(|operand| operand.as_name().ok())
                    .is_some_and(|name| name == b"GSImageLuminosity")
        }));
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_passive_jpeg_bilevel_decode_with_luminosity_blend() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::JpegPassiveBilevel,
            bytes: minimal_jpeg_with_dimensions(1, 1),
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(
            pdf.windows(b"/Decode [1 0 1 0 1 0]".len())
                .any(|window| window == b"/Decode [1 0 1 0 1 0]")
        );
        assert!(
            pdf.windows(b"/BM /Luminosity".len())
                .any(|window| window == b"/BM /Luminosity")
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "gs"
                && operation
                    .operands
                    .first()
                    .and_then(|operand| operand.as_name().ok())
                    .is_some_and(|name| name == b"GSImageLuminosity")
        }));
        for forbidden in [b"/JavaScript".as_slice(), b"/EmbeddedFile"] {
            assert!(
                !pdf.windows(forbidden.len())
                    .any(|window| window == forbidden)
            );
        }
    }

    #[test]
    fn writes_passive_cmyk_jpeg_bilevel_decode_with_luminosity_blend() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::JpegCmykPassiveBilevel,
            bytes: minimal_cmyk_jpeg_with_dimensions(1, 1),
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(
            pdf.windows(b"/ColorSpace /DeviceCMYK".len())
                .any(|window| window == b"/ColorSpace /DeviceCMYK")
        );
        assert!(
            pdf.windows(b"/Decode [1 0 1 0 1 0 1 0]".len())
                .any(|window| window == b"/Decode [1 0 1 0 1 0 1 0]")
        );
        assert!(
            pdf.windows(b"/BM /Luminosity".len())
                .any(|window| window == b"/BM /Luminosity")
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "gs"
                && operation
                    .operands
                    .first()
                    .and_then(|operand| operand.as_name().ok())
                    .is_some_and(|name| name == b"GSImageLuminosity")
        }));
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn writes_passive_cmyk_jpeg_image_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::JpegCmyk,
            bytes: minimal_cmyk_jpeg_with_dimensions(1, 1),
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/ColorSpace /DeviceCMYK".len())
                .any(|window| window == b"/ColorSpace /DeviceCMYK")
        );
        assert!(
            pdf.windows(b"/DCTDecode".len())
                .any(|window| window == b"/DCTDecode")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_passive_png_image_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Png,
            bytes: minimal_png_idat_for_1x1_rgb(),
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/Subtype /Image".len())
                .any(|window| window == b"/Subtype /Image")
        );
        assert!(
            pdf.windows(b"/FlateDecode".len())
                .any(|window| window == b"/FlateDecode")
        );
        assert!(
            pdf.windows(b"/Predictor 15".len())
                .any(|window| window == b"/Predictor 15")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_passive_png_alpha_mask_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Png,
            bytes: minimal_png_idat_for_1x1_rgb(),
            palette: Vec::new(),
            alpha_mask: Some(StaticImageAlphaMask {
                bytes: miniz_oxide::deflate::compress_to_vec_zlib(&[0, 128], 6),
            }),
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/SMask".len())
                .any(|window| window == b"/SMask")
        );
        assert_eq!(
            pdf.windows(b"/Subtype /Image".len())
                .filter(|window| *window == b"/Subtype /Image")
                .count(),
            2
        );
        assert!(
            pdf.windows(b"/ColorSpace /DeviceGray".len())
                .any(|window| window == b"/ColorSpace /DeviceGray")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_passive_indexed_png_image_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::PngIndexed,
            bytes: minimal_png_idat_for_1x1_indexed(),
            palette: vec![255, 0, 0, 0, 255, 0],
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/Subtype /Image".len())
                .any(|window| window == b"/Subtype /Image")
        );
        assert!(
            pdf.windows(b"/Indexed /DeviceRGB 1".len())
                .any(|window| window == b"/Indexed /DeviceRGB 1")
        );
        assert!(
            pdf.windows(b"/FlateDecode".len())
                .any(|window| window == b"/FlateDecode")
        );
        assert!(
            pdf.windows(b"/Colors 1".len())
                .any(|window| window == b"/Colors 1")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_passive_raw_rgb_image_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Rgb8,
            bytes: vec![255, 0, 0, 0, 255, 0],
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 2,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/Subtype /Image".len())
                .any(|window| window == b"/Subtype /Image")
        );
        assert!(
            !pdf.windows(b"/DCTDecode".len())
                .any(|window| window == b"/DCTDecode")
        );
        assert!(
            !pdf.windows(b"/Predictor".len())
                .any(|window| window == b"/Predictor")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_cropped_image_as_passive_clip_and_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Jpeg,
            bytes: minimal_jpeg_with_dimensions(100, 100),
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 100,
            height_px: 100,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(1440),
            display_height_twips: Some(1440),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop {
                left_twips: 240,
                top_twips: 0,
                right_twips: 0,
                bottom_twips: 0,
            },
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "W")
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "Do")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
    }

    #[test]
    fn crops_image_against_authored_goal_dimensions() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Jpeg,
            bytes: minimal_jpeg_with_dimensions(2, 1),
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 2,
            height_px: 1,
            natural_width_px_hint: Some(80),
            natural_height_px_hint: Some(40),
            display_width_twips: Some(1440),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop {
                left_twips: 720,
                top_twips: 0,
                right_twips: 0,
                bottom_twips: 0,
            },
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();
        let do_pos = content
            .operations
            .iter()
            .position(|operation| operation.operator == "Do")
            .expect("image draw operation");
        let image_transform = content.operations[..do_pos]
            .iter()
            .rfind(|operation| operation.operator == "cm")
            .expect("image transform");

        assert!(
            pdf_number(image_transform.operands[0].clone())
                .is_some_and(|value| (value - 72.0).abs() < 0.01),
            "left crop should preserve the authored full goal width, got {:?}",
            image_transform.operands
        );
        assert!(
            pdf_number(image_transform.operands[3].clone())
                .is_some_and(|value| (value - 36.0).abs() < 0.01),
            "vertical image scale should remain the displayed height, got {:?}",
            image_transform.operands
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
    }

    #[test]
    fn fully_natural_cropped_image_keeps_flow_frame_without_drawing() {
        let parsed = crate::rtf::parse_rtf(include_str!("../../fixtures/picture-scaling-passive.rtf"))
            .expect("picture scaling fixture should parse");
        let layout = LayoutEngine::layout(&parsed.document);
        let pdf = render_pdf(&layout);
        let parsed_pdf = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed_pdf.get_pages().values().next().expect("page");
        let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

        assert!(
            content
                .operations
                .iter()
                .all(|operation| operation.operator != "Do")
        );
    }
