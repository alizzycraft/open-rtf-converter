    #[test]
    fn centralized_fragment_traversal_reaches_deep_wrapped_leaf_once() {
        let item = deep_drawing_wrapped_item(
            LayoutItem::Opacity {
                fill_percent: 80,
                stroke_percent: 70,
                item: Box::new(LayoutItem::ClipRect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 20.0,
                    item: Box::new(passive_text_item("deep leaf", 0.0, 12.0)),
                }),
            },
            128,
        );

        let mut text = Vec::new();
        item.for_each_text_fragment(&mut |fragment| text.push(fragment.text.clone()));
        let mut image_count = 0;
        item.for_each_image_fragment(&mut |_| image_count += 1);

        assert_eq!(text, ["deep leaf"]);
        assert_eq!(image_count, 0);
    }

    #[test]
    fn deep_drawing_wrapped_vertical_bounds_are_iterative() {
        let item = deep_drawing_wrapped_item(passive_text_item("Layered", 18.0, 64.0), 2048);

        let bounds = layout_item_vertical_bounds(&item).expect("vertical bounds");

        assert!(bounds.top > 64.0);
        assert!(bounds.bottom < 64.0);
    }

    #[test]
    fn deep_drawing_wrapped_layout_shifts_are_iterative() {
        let mut item = deep_drawing_wrapped_item(passive_text_item("Shifted", 18.0, 64.0), 2048);

        translate_layout_item_y(&mut item, -12.0);
        shift_layout_item_x(&mut item, 7.5);

        let fragment = leaf_text_fragment(&item);
        assert!((fragment.x - 25.5).abs() < 0.01);
        assert!((fragment.baseline_y - 52.0).abs() < 0.01);
    }

    #[test]
    fn deep_drawing_wrapped_late_marker_checks_are_iterative() {
        let same_line = deep_drawing_wrapped_item(passive_text_item("After", 72.0, 64.0), 2048);
        let next_line = deep_drawing_wrapped_item(passive_text_item("Next", 72.0, 40.0), 2048);
        let geometry = LateMarkerGeometry {
            x: 36.0,
            baseline_y: 64.0,
            old_width: 24.0,
            new_width: 30.0,
            font_size: 12.0,
        };

        assert!(item_starts_after_late_marker_on_line(
            &same_line,
            geometry.x + geometry.old_width,
            geometry
        ));
        assert!(!starts_new_text_line(&same_line, geometry));
        assert!(starts_new_text_line(&next_line, geometry));
    }

    #[test]
    fn drawing_wrapped_bookmark_page_markers_resolve_without_internal_marker_leakage() {
        let settings = PageSettings::default();
        let geometry = PageGeometry::from_settings(
            &settings,
            PageNumbering::from_page_settings(&settings, 1),
            1,
            0,
        );
        let mut pages = vec![
            new_layout_page(geometry, 1),
            new_layout_page(geometry.for_physical_page(2), 2),
        ];
        let mut anchor = passive_text_item("", 36.0, 64.0);
        if let LayoutItem::Text(fragment) = &mut anchor {
            fragment.dynamic_field = Some(InlineKind::BookmarkPageAnchor(BookmarkId::new(7)));
        }
        pages[0]
            .items
            .push(deep_drawing_wrapped_item(anchor, 2048));
        let mut reference = passive_text_item("", 36.0, 64.0);
        if let LayoutItem::Text(fragment) = &mut reference {
            fragment.dynamic_field = Some(InlineKind::BookmarkPageReference(BookmarkId::new(7)));
        }
        pages[1]
            .items
            .push(deep_drawing_wrapped_item(reference, 2048));
        pages[1].items.push(deep_drawing_wrapped_item(
            passive_text_item(" after", 44.0, 64.0),
            2048,
        ));
        let old_suffix_x = leaf_text_fragment(&pages[1].items[1]).x;
        let document = Document::default();

        resolve_bookmark_page_ref_markers(&mut pages, &document, None);

        assert_eq!(leaf_text_fragment(&pages[0].items[0]).text, "");
        let resolved = leaf_text_fragment(&pages[1].items[0]);
        assert_eq!(resolved.text, "1");
        assert_eq!(resolved.dynamic_field, None);
        assert!(leaf_text_fragment(&pages[1].items[1]).x > old_suffix_x);
    }

    #[test]
    fn drawing_wrapped_late_page_count_markers_resolve_without_internal_marker_leakage() {
        let settings = PageSettings::default();
        let geometry = PageGeometry::from_settings(
            &settings,
            PageNumbering::from_page_settings(&settings, 1),
            1,
            0,
        );
        let mut pages = vec![
            new_layout_page(geometry, 1),
            new_layout_page(geometry.for_physical_page(2), 2),
        ];
        let marker_x = 36.0;
        let marker_baseline = 64.0;
        let old_marker_width = measure_text_with_family(
            LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER,
            &CharacterStyle::default(),
            PdfFontFamily::Helvetica,
        );
        pages[0].items.push(deep_drawing_wrapped_item(
            passive_dynamic_field_item(DynamicField::TotalPages, marker_x, marker_baseline),
            2048,
        ));
        pages[0].items.push(deep_drawing_wrapped_item(
            passive_text_item(" after", marker_x + old_marker_width + 2.0, marker_baseline),
            2048,
        ));
        pages[0].items.push(deep_drawing_wrapped_item(
            passive_dynamic_field_item(DynamicField::SectionPages, 36.0, 40.0),
            2048,
        ));
        let old_suffix_x = leaf_text_fragment(&pages[0].items[1]).x;

        resolve_section_page_markers(&mut pages);
        resolve_total_page_markers(&mut pages);

        let total = leaf_text_fragment(&pages[0].items[0]);
        let suffix = leaf_text_fragment(&pages[0].items[1]);
        let section_pages = leaf_text_fragment(&pages[0].items[2]);
        assert_eq!(total.text, "2");
        assert_eq!(section_pages.text, "2");
        assert_eq!(total.dynamic_field, None);
        assert_eq!(section_pages.dynamic_field, None);
        assert!(
            suffix.x < old_suffix_x,
            "suffix should shift left after replacing the bounded page-count placeholder"
        );
    }

    #[test]
    fn lays_out_shape_picture_images_with_bounded_passive_placement_frame() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Jpeg,
            bytes: vec![0xff, 0xd8, 0xff, 0xd9],
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 100,
            height_px: 50,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(7200),
            display_height_twips: Some(3600),
            scale_x_percent: Some(25),
            scale_y_percent: Some(25),
            crop: ImageCrop::default(),
            placement: Some(StaticImagePlacement {
                left_twips: 720,
                top_twips: 360,
                width_twips: 1440,
                height_twips: 720,
                z_order: 0,
                below_text: false,
                text_wrap: false,
                wrap_side: StaticImageWrapSide::Both,
                wrap_margin_left_twips: 120,
                wrap_margin_right_twips: 120,
                wrap_margin_top_twips: 0,
                wrap_margin_bottom_twips: 0,
                horizontal_anchor: StaticShapeHorizontalAnchor::Column,
                vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
            }),
        })];

        let layout = LayoutEngine::layout(&document);
        let image = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .expect("placed shape image");

        assert!((image.x - 108.0).abs() < 0.01);
        assert!((image.y - 666.0).abs() < 0.01);
        assert!((image.width - 72.0).abs() < 0.01);
        assert!((image.height - 36.0).abs() < 0.01);
    }

    #[test]
    fn wraps_paragraph_line_beside_prior_passive_shape_picture_frame() {
        let mut document = Document::default();
        document.blocks = vec![
            Block::Image(StaticImage {
                format: ImageFormat::Jpeg,
                bytes: vec![0xff, 0xd8, 0xff, 0xd9],
                palette: Vec::new(),
                alpha_mask: None,
                tone_adjustment: None,
                vector_commands: Vec::new(),
                width_px: 100,
                height_px: 50,
                natural_width_px_hint: None,
                natural_height_px_hint: None,
                display_width_twips: None,
                display_height_twips: None,
                scale_x_percent: None,
                scale_y_percent: None,
                crop: ImageCrop::default(),
                placement: Some(StaticImagePlacement {
                    left_twips: 0,
                    top_twips: 0,
                    width_twips: 1440,
                    height_twips: 720,
                    z_order: 0,
                    below_text: false,
                    text_wrap: true,
                    wrap_side: StaticImageWrapSide::Both,
                    wrap_margin_left_twips: 120,
                    wrap_margin_right_twips: 120,
                    wrap_margin_top_twips: 0,
                    wrap_margin_bottom_twips: 0,
                    horizontal_anchor: StaticShapeHorizontalAnchor::Column,
                    vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
                }),
            }),
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    dynamic_field: None,
                    text: "Wrapped beside image".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let image = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .expect("wrapped shape image");
        let text = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(text) if text.text.contains("Wrapped") => Some(text),
                _ => None,
            })
            .expect("paragraph text");

        assert!((image.x - 72.0).abs() < 0.01);
        assert!(
            text.x > image.x + image.width,
            "text should be shifted beside wrapped image: text {}, image right {}",
            text.x,
            image.x + image.width
        );
        assert!(text.baseline_y < image.y + image.height && text.baseline_y > image.y);
    }

    #[test]
    fn wrapped_paragraph_honors_passive_shape_picture_wrap_margins() {
        let mut document = Document::default();
        document.blocks = vec![
            Block::Image(StaticImage {
                format: ImageFormat::Jpeg,
                bytes: vec![0xff, 0xd8, 0xff, 0xd9],
                palette: Vec::new(),
                alpha_mask: None,
                tone_adjustment: None,
                vector_commands: Vec::new(),
                width_px: 100,
                height_px: 50,
                natural_width_px_hint: None,
                natural_height_px_hint: None,
                display_width_twips: None,
                display_height_twips: None,
                scale_x_percent: None,
                scale_y_percent: None,
                crop: ImageCrop::default(),
                placement: Some(StaticImagePlacement {
                    left_twips: 0,
                    top_twips: 0,
                    width_twips: 1440,
                    height_twips: 720,
                    z_order: 0,
                    below_text: false,
                    text_wrap: true,
                    wrap_side: StaticImageWrapSide::Both,
                    wrap_margin_left_twips: 120,
                    wrap_margin_right_twips: 720,
                    wrap_margin_top_twips: 0,
                    wrap_margin_bottom_twips: 0,
                    horizontal_anchor: StaticShapeHorizontalAnchor::Column,
                    vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
                }),
            }),
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    dynamic_field: None,
                    text: "Wrapped with larger margin".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let image = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .expect("wrapped shape image");
        let text = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(text) if text.text.contains("Wrapped") => Some(text),
                _ => None,
            })
            .expect("paragraph text");

        assert!(
            text.x >= image.x + image.width + 36.0 - 0.01,
            "text should honor normalized right wrap margin: text {}, expected at least {}",
            text.x,
            image.x + image.width + 36.0
        );
    }

    #[test]
    fn wrapped_line_exclusion_honors_passive_vertical_wrap_margins() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Jpeg,
            bytes: vec![0xff, 0xd8, 0xff, 0xd9],
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 100,
            height_px: 50,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: None,
            display_height_twips: None,
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: Some(StaticImagePlacement {
                left_twips: 0,
                top_twips: 0,
                width_twips: 1440,
                height_twips: 720,
                z_order: 0,
                below_text: false,
                text_wrap: true,
                wrap_side: StaticImageWrapSide::Both,
                wrap_margin_left_twips: 120,
                wrap_margin_right_twips: 120,
                wrap_margin_top_twips: 0,
                wrap_margin_bottom_twips: 480,
                horizontal_anchor: StaticShapeHorizontalAnchor::Column,
                vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
            }),
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let image = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .expect("wrapped shape image");
        let (line_margin_left, line_content_width) = wrapped_image_text_area_for_line(
            &layout.pages,
            page.geometry.margin_left,
            page.geometry.content_width,
            image.y - 1.0,
            12.0,
        )
        .expect("line inside passive bottom wrap margin should be excluded");

        assert!(
            line_margin_left > image.x + image.width,
            "text should stay beside image within passive vertical wrap margin: text {}, image right {}",
            line_margin_left,
            image.x + image.width
        );
        assert!(line_content_width < page.geometry.content_width);
    }

    #[test]
    fn wrapped_line_exclusion_honors_passive_wrap_side() {
        let wrapped_image = |left_twips, wrap_side| StaticImage {
            format: ImageFormat::Jpeg,
            bytes: vec![0xff, 0xd8, 0xff, 0xd9],
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 100,
            height_px: 50,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: None,
            display_height_twips: None,
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: Some(StaticImagePlacement {
                left_twips,
                top_twips: 0,
                width_twips: 1440,
                height_twips: 720,
                z_order: 0,
                below_text: false,
                text_wrap: true,
                wrap_side,
                wrap_margin_left_twips: 120,
                wrap_margin_right_twips: 120,
                wrap_margin_top_twips: 0,
                wrap_margin_bottom_twips: 0,
                horizontal_anchor: StaticShapeHorizontalAnchor::Column,
                vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
            }),
        };

        let mut right_only_document = Document::default();
        right_only_document.blocks = vec![Block::Image(wrapped_image(
            1440,
            StaticImageWrapSide::Right,
        ))];
        let right_only = LayoutEngine::layout(&right_only_document);
        let right_page = &right_only.pages[0];
        let right_image = right_page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .expect("right-only wrapped image");
        let (right_margin_left, _) = wrapped_image_text_area_for_line(
            &right_only.pages,
            right_page.geometry.margin_left,
            right_page.geometry.content_width,
            right_image.y + right_image.height,
            12.0,
        )
        .expect("right-only side interval");
        assert!(
            right_margin_left > right_image.x + right_image.width,
            "right-side wrapping should discard the left interval: text {}, image right {}",
            right_margin_left,
            right_image.x + right_image.width
        );

        let mut left_only_document = Document::default();
        left_only_document.blocks =
            vec![Block::Image(wrapped_image(5760, StaticImageWrapSide::Left))];
        let left_only = LayoutEngine::layout(&left_only_document);
        let left_page = &left_only.pages[0];
        let left_image = left_page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .expect("left-only wrapped image");
        let (left_margin_left, left_width) = wrapped_image_text_area_for_line(
            &left_only.pages,
            left_page.geometry.margin_left,
            left_page.geometry.content_width,
            left_image.y + left_image.height,
            12.0,
        )
        .expect("left-only side interval");
        assert!(
            left_margin_left < left_image.x,
            "left-side wrapping should keep the left interval: text {}, image left {}",
            left_margin_left,
            left_image.x
        );
        assert!(
            left_margin_left + left_width < left_image.x,
            "left-side wrapping should discard the right interval: text right {}, image left {}",
            left_margin_left + left_width,
            left_image.x
        );
    }

    #[test]
    fn wrapped_paragraph_returns_to_full_width_below_passive_shape_picture_frame() {
        let mut document = Document::default();
        document.blocks = vec![
            Block::Image(StaticImage {
                format: ImageFormat::Jpeg,
                bytes: vec![0xff, 0xd8, 0xff, 0xd9],
                palette: Vec::new(),
                alpha_mask: None,
                tone_adjustment: None,
                vector_commands: Vec::new(),
                width_px: 100,
                height_px: 50,
                natural_width_px_hint: None,
                natural_height_px_hint: None,
                display_width_twips: None,
                display_height_twips: None,
                scale_x_percent: None,
                scale_y_percent: None,
                crop: ImageCrop::default(),
                placement: Some(StaticImagePlacement {
                    left_twips: 0,
                    top_twips: 0,
                    width_twips: 5760,
                    height_twips: 560,
                    z_order: 0,
                    below_text: false,
                    text_wrap: true,
                    wrap_side: StaticImageWrapSide::Both,
                    wrap_margin_left_twips: 120,
                    wrap_margin_right_twips: 120,
                    wrap_margin_top_twips: 0,
                    wrap_margin_bottom_twips: 0,
                    horizontal_anchor: StaticShapeHorizontalAnchor::Column,
                    vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
                }),
            }),
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    dynamic_field: None,
                    text: "one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo twentythree twentyfour".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let image = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .expect("wrapped shape image");
        let mut text_lines: Vec<(f32, f32, String)> = Vec::new();
        for item in &page.items {
            let LayoutItem::Text(fragment) = item else {
                continue;
            };
            if let Some((_, _, text)) = text_lines
                .iter_mut()
                .find(|(baseline_y, _, _)| (*baseline_y - fragment.baseline_y).abs() < 0.01)
            {
                text.push_str(&fragment.text);
            } else {
                text_lines.push((fragment.baseline_y, fragment.x, fragment.text.clone()));
            }
        }

        assert!(
            text_lines
                .iter()
                .any(|(baseline_y, x, text)| *baseline_y > image.y
                    && *baseline_y < image.y + image.height
                    && *x > image.x + image.width
                    && text.len() < 32),
            "expected a short line beside the wrapped image: {:?}",
            text_lines
        );
        assert!(
            text_lines
                .iter()
                .any(|(baseline_y, x, text)| *baseline_y < image.y
                    && (*x - page.geometry.margin_left).abs() < 0.01
                    && text.len() > 40),
            "expected a full-width line below the wrapped image: {:?}",
            text_lines
        );
    }

    #[test]
    fn wrapped_paragraph_uses_free_interval_between_passive_shape_picture_frames() {
        let wrapped_image = |left_twips| StaticImage {
            format: ImageFormat::Jpeg,
            bytes: vec![0xff, 0xd8, 0xff, 0xd9],
            palette: Vec::new(),
            alpha_mask: None,
            tone_adjustment: None,
            vector_commands: Vec::new(),
            width_px: 100,
            height_px: 50,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: None,
            display_height_twips: None,
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: Some(StaticImagePlacement {
                left_twips,
                top_twips: 0,
                width_twips: 2160,
                height_twips: 720,
                z_order: 0,
                below_text: false,
                text_wrap: true,
                wrap_side: StaticImageWrapSide::Both,
                wrap_margin_left_twips: 120,
                wrap_margin_right_twips: 120,
                wrap_margin_top_twips: 0,
                wrap_margin_bottom_twips: 0,
                horizontal_anchor: StaticShapeHorizontalAnchor::Column,
                vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
            }),
        };
        let mut document = Document::default();
        document.blocks = vec![
            Block::Image(wrapped_image(0)),
            Block::Image(wrapped_image(5760)),
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    dynamic_field: None,
                    text: "center gap words should flow between the two passive picture frames before returning below them"
                        .to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let images = page
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(images.len(), 2);
        let text = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(text)
                    if text.baseline_y > images[0].y
                        && text.baseline_y < images[0].y + images[0].height =>
                {
                    Some(text)
                }
                _ => None,
            })
            .expect("text beside wrapped images");

        assert!(
            text.x > images[0].x + images[0].width && text.x < images[1].x,
            "text should use the free interval between wrapped images: text x {}, left image right {}, right image left {}",
            text.x,
            images[0].x + images[0].width,
            images[1].x
        );
    }

    #[test]
    fn wrapped_paragraph_flows_beside_prior_passive_static_shape_frame() {
        let mut document = Document::default();
        document.blocks = vec![
            Block::Shape(StaticShape {
                kind: StaticShapeKind::Rectangle,
                left_twips: 0,
                top_twips: 0,
                width_twips: 2160,
                height_twips: 720,
                z_order: 0,
                below_text: false,
                text_wrap: true,
                wrap_side: StaticImageWrapSide::Both,
                wrap_margin_left_twips: 120,
                wrap_margin_right_twips: 240,
                wrap_margin_top_twips: 0,
                wrap_margin_bottom_twips: 0,
                horizontal_anchor: StaticShapeHorizontalAnchor::Column,
                vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
                flip_horizontal: false,
                flip_vertical: false,
                start_arrowhead: StaticShapeArrowhead::None,
                end_arrowhead: StaticShapeArrowhead::None,
                start_arrowhead_width_percent: 100,
                start_arrowhead_length_percent: 100,
                end_arrowhead_width_percent: 100,
                end_arrowhead_length_percent: 100,
                stroke_width_twips: 15,
                stroke_color: Default::default(),
                stroke_style: BorderStyle::Single,
                stroke_cap: StaticShapeLineCap::Flat,
                stroke_join: StaticShapeLineJoin::Miter,
                fill_opacity_percent: 100,
                stroke_opacity_percent: 100,
                fill_color: None,
                fill_gradient_color: None,
                fill_pattern: ShadingPattern::None,
                shadow_enabled: false,
                shadow_color: Default::default(),
                shadow_opacity_percent: 100,
                shadow_offset_x_twips: 0,
                shadow_offset_y_twips: 0,
                text_margin_left_twips: 0,
                text_margin_right_twips: 0,
                text_margin_top_twips: 0,
                text_margin_bottom_twips: 0,
                text_vertical_anchor: StaticShapeTextVerticalAnchor::Top,
                text_horizontal_anchor_centered: false,
                text: Vec::new(),
                points: Vec::new(),
                point_paths: Vec::new(),
                overlay_paths: Vec::new(),
                fill_rule: StaticImageVectorFillRule::Winding,
            }),
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    dynamic_field: None,
                    text: "wrapped words should start beside the passive static shape frame"
                        .to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let shape_right = page
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Line { x1, x2, .. } => Some(x1.max(*x2)),
                _ => None,
            })
            .fold(0.0_f32, f32::max);
        let text = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(text) if text.text.contains("wrapped") => Some(text),
                _ => None,
            })
            .expect("paragraph text beside wrapped static shape");

        assert!(
            text.x > shape_right,
            "text should be shifted beside wrapped static shape: text {}, shape right {}",
            text.x,
            shape_right
        );
    }

    #[test]
    fn paragraph_anchored_shape_starts_after_prior_paragraph_and_uses_word_edge_tolerance() {
        let parsed = parse_rtf(include_str!("../../fixtures/static-shape-wrap.rtf")).unwrap();
        let layout = LayoutEngine::layout(&parsed.document);
        let page = &layout.pages[0];
        let shape = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight {
                    y, height, color, ..
                } if color.green > 0.9 && color.blue > 0.9 => Some((*y, *height)),
                _ => None,
            })
            .expect("wrapped cyan shape");
        let wrapped = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(text) if text.text.starts_with("Wrapped") => Some(text),
                _ => None,
            })
            .expect("Word-compatible first wrapped line");

        assert!(
            (shape.0 + shape.1 - 345.0).abs() < 0.01,
            "shape top was {}",
            shape.0 + shape.1
        );
        assert!((wrapped.x - 153.0).abs() < 0.01);
    }

    #[test]
    fn modern_shape_defaults_to_word_margin_anchor_and_white_fill() {
        let parsed =
            parse_rtf(include_str!("../../fixtures/shape-disabled-line-passive.rtf")).unwrap();
        let shape = parsed
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .expect("modern rectangle shape");
        assert_eq!(shape.vertical_anchor, StaticShapeVerticalAnchor::Margin);
        assert_eq!(
            shape.fill_color,
            Some(Color {
                red: 255,
                green: 255,
                blue: 255
            })
        );

        let layout = LayoutEngine::layout(&parsed.document);
        let fill = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight {
                    x,
                    y,
                    width,
                    height,
                    color,
                    ..
                } => Some((*x, *y, *width, *height, *color)),
                _ => None,
            })
            .expect("Word default white fill");

        assert_eq!(
            fill.4,
            PdfColor {
                red: 1.0,
                green: 1.0,
                blue: 1.0
            }
        );
        assert!((fill.0 - 108.0).abs() < 0.01);
        assert!((fill.1 - 648.0).abs() < 0.01);
        assert!((fill.2 - 108.0).abs() < 0.01);
        assert!((fill.3 - 36.0).abs() < 0.01);
        assert!(
            !layout.pages[0]
                .items
                .iter()
                .any(|item| matches!(item, LayoutItem::Line { .. }))
        );
    }

    #[test]
    fn modern_shape_shadow_uses_word_two_point_default_offset() {
        let parsed =
            parse_rtf(include_str!("../../fixtures/shape-shadow-opacity-passive.rtf")).unwrap();
        let shape = parsed
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .expect("modern shadowed rectangle");

        assert_eq!(shape.shadow_offset_x_twips, 40);
        assert_eq!(shape.shadow_offset_y_twips, 40);

        let layout = LayoutEngine::layout(&parsed.document);
        let fills = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| {
                let mut leaf = item;
                loop {
                    leaf = match leaf {
                        LayoutItem::Drawing(drawing) => drawing.item.as_ref(),
                        LayoutItem::Opacity { item, .. } => item.as_ref(),
                        _ => break,
                    };
                }
                match leaf {
                    LayoutItem::Highlight {
                        x,
                        y,
                        width,
                        height,
                        ..
                    } => Some((*x, *y, *width, *height)),
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        assert!(fills.iter().any(|fill| {
            (fill.0 - 110.0).abs() < 0.01
                && (fill.1 - 646.0).abs() < 0.01
                && (fill.2 - 108.0).abs() < 0.01
                && (fill.3 - 36.0).abs() < 0.01
        }));
        assert!(fills.iter().any(|fill| {
            (fill.0 - 108.0).abs() < 0.01
                && (fill.1 - 648.0).abs() < 0.01
                && (fill.2 - 108.0).abs() < 0.01
                && (fill.3 - 36.0).abs() < 0.01
        }));
    }

    #[test]
    fn word_ignores_textual_office_shape_fill_type_aliases() {
        for fixture in [
            include_str!("../../fixtures/shape-pattern-fill-passive.rtf"),
            include_str!("../../fixtures/shape-gradient-fill-passive.rtf"),
        ] {
            let parsed = parse_rtf(fixture).unwrap();
            let shape = parsed
                .document
                .blocks
                .iter()
                .find_map(|block| match block {
                    Block::Shape(shape) => Some(shape),
                    _ => None,
                })
                .expect("modern filled rectangle");

            assert_eq!(shape.fill_pattern, ShadingPattern::None);
            assert_eq!(shape.fill_gradient_color, None);
        }
    }

    #[test]
    fn word_uses_default_colors_for_office_shape_color_aliases() {
        for fixture in [
            include_str!("../../fixtures/shape-fill-fore-color-passive.rtf"),
            include_str!("../../fixtures/shape-fill-back-color-passive.rtf"),
            include_str!("../../fixtures/shape-line-fore-color-passive.rtf"),
            include_str!("../../fixtures/shape-line-back-color-passive.rtf"),
        ] {
            let parsed = parse_rtf(fixture).unwrap();
            let shape = parsed
                .document
                .blocks
                .iter()
                .find_map(|block| match block {
                    Block::Shape(shape) => Some(shape),
                    _ => None,
                })
                .expect("modern rectangle with color alias");

            assert_eq!(
                shape.fill_color,
                Some(Color {
                    red: 255,
                    green: 255,
                    blue: 255,
                })
            );
            assert_eq!(
                shape.stroke_color,
                Color {
                    red: 0,
                    green: 0,
                    blue: 0,
                }
            );
        }
    }

    #[test]
    fn word_background_shape_covers_page_without_outline() {
        let parsed = parse_rtf(include_str!("../../fixtures/background-shape-passive.rtf")).unwrap();
        let layout = LayoutEngine::layout(&parsed.document);
        let page = &layout.pages[0];
        let background = page.items.iter().find_map(|item| match item {
            LayoutItem::Drawing(drawing) if drawing.below_text => match drawing.item.as_ref() {
                LayoutItem::Highlight {
                    x,
                    y,
                    width,
                    height,
                    color,
                } => Some((*x, *y, *width, *height, *color)),
                _ => None,
            },
            LayoutItem::Highlight {
                x,
                y,
                width,
                height,
                color,
            } => Some((*x, *y, *width, *height, *color)),
            _ => None,
        });
        let (x, y, width, height, color) = background.expect("page background fill");
        assert!(x.abs() < f32::EPSILON);
        assert!((y + 0.1).abs() < 0.01, "y={y}");
        assert!((width - 612.1).abs() < 0.01, "width={width}");
        assert!((height - 792.1).abs() < 0.01, "height={height}");
        assert_eq!(color.red, 0.0);
        assert_eq!(color.green, 1.0);
        assert_eq!(color.blue, 1.0);
        assert!(
            page.items
                .iter()
                .all(|item| !matches!(item, LayoutItem::Line { .. })
                    && !matches!(item, LayoutItem::Drawing(drawing)
                        if matches!(drawing.item.as_ref(), LayoutItem::Line { .. }))),
            "Word suppresses the authored background-shape outline: {:?}",
            page.items
        );
    }

    #[test]
    fn inline_image_final_paragraph_mark_wraps_and_preserves_word_baseline_pitch() {
        let parsed = parse_rtf(include_str!("../../fixtures/wmf-hatch-brush-passive.rtf")).unwrap();
        let provider = FontProvider::browser_safe_defaults();
        let layout = LayoutEngine::layout_with_font_provider(&parsed.document, Some(&provider));
        let page = &layout.pages[0];
        let after = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(text) if text.text.starts_with("After") => Some(text),
                _ => None,
            })
            .expect("text after inline hatch image");
        let trailing = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(text)
                    if text.text.starts_with("hatch") && text.baseline_y < after.baseline_y =>
                {
                    Some(text)
                }
                _ => None,
            })
            .expect("paragraph-final word wrapped after inline image");

        assert!((trailing.x - 36.0).abs() < 0.01);
        assert!(
            ((after.baseline_y - trailing.baseline_y) - 13.8).abs() < 0.01,
            "Word advances a normal Arial line pitch after an inline image: after={}, trailing={}",
            after.baseline_y,
            trailing.baseline_y
        );
    }

    #[test]
    fn passive_drawing_slots_are_sorted_by_normalized_z_order() {
        let mut document = Document::default();
        document.blocks = vec![
            Block::Image(StaticImage {
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
                display_width_twips: None,
                display_height_twips: None,
                scale_x_percent: None,
                scale_y_percent: None,
                crop: ImageCrop::default(),
                placement: Some(StaticImagePlacement {
                    left_twips: 0,
                    top_twips: 0,
                    width_twips: 720,
                    height_twips: 720,
                    z_order: 30,
                    below_text: false,
                    text_wrap: true,
                    wrap_side: StaticImageWrapSide::Both,
                    wrap_margin_left_twips: 120,
                    wrap_margin_right_twips: 120,
                    wrap_margin_top_twips: 0,
                    wrap_margin_bottom_twips: 0,
                    horizontal_anchor: StaticShapeHorizontalAnchor::Column,
                    vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
                }),
            }),
            Block::Shape(StaticShape {
                kind: StaticShapeKind::Rectangle,
                left_twips: 0,
                top_twips: 0,
                width_twips: 720,
                height_twips: 720,
                z_order: 10,
                below_text: false,
                text_wrap: false,
                wrap_side: StaticImageWrapSide::Both,
                wrap_margin_left_twips: 0,
                wrap_margin_right_twips: 0,
                wrap_margin_top_twips: 0,
                wrap_margin_bottom_twips: 0,
                horizontal_anchor: StaticShapeHorizontalAnchor::Column,
                vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
                flip_horizontal: false,
                flip_vertical: false,
                start_arrowhead: StaticShapeArrowhead::None,
                end_arrowhead: StaticShapeArrowhead::None,
                start_arrowhead_width_percent: 100,
                start_arrowhead_length_percent: 100,
                end_arrowhead_width_percent: 100,
                end_arrowhead_length_percent: 100,
                stroke_width_twips: 0,
                stroke_color: Color::default(),
                stroke_style: BorderStyle::Single,
                stroke_cap: StaticShapeLineCap::Flat,
                stroke_join: StaticShapeLineJoin::Miter,
                fill_opacity_percent: 100,
                stroke_opacity_percent: 100,
                fill_color: Some(Color {
                    red: 200,
                    green: 20,
                    blue: 20,
                }),
                fill_gradient_color: None,
                fill_pattern: crate::model::ShadingPattern::None,
                shadow_enabled: false,
                shadow_color: Color {
                    red: 128,
                    green: 128,
                    blue: 128,
                },
                shadow_opacity_percent: 100,
                shadow_offset_x_twips: 60,
                shadow_offset_y_twips: 60,
                text_margin_left_twips: 80,
                text_margin_right_twips: 80,
                text_margin_top_twips: 80,
                text_margin_bottom_twips: 80,
                text_vertical_anchor: StaticShapeTextVerticalAnchor::Top,
                text_horizontal_anchor_centered: false,
                text: Vec::new(),
                points: Vec::new(),
                point_paths: Vec::new(),
                overlay_paths: Vec::new(),
                fill_rule: StaticImageVectorFillRule::Winding,
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let drawing_items = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| drawing_z_order(item).map(|z_order| (z_order, item)))
            .collect::<Vec<_>>();

        assert_eq!(drawing_items.len(), 2);
        assert_eq!(drawing_items[0].0, 10);
        assert_eq!(drawing_items[1].0, 30);
        assert!(matches!(drawing_items[0].1, LayoutItem::Drawing(_)));
        assert!(matches!(drawing_items[1].1, LayoutItem::Image(_)));
    }

    #[test]
    fn z_ordered_shape_text_stays_in_shape_drawing_layer() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Rectangle,
            left_twips: 0,
            top_twips: 0,
            width_twips: 2160,
            height_twips: 720,
            z_order: 12,
            below_text: false,
            text_wrap: false,
            wrap_side: StaticImageWrapSide::Both,
            wrap_margin_left_twips: 0,
            wrap_margin_right_twips: 0,
            wrap_margin_top_twips: 0,
            wrap_margin_bottom_twips: 0,
            horizontal_anchor: StaticShapeHorizontalAnchor::Column,
            vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            start_arrowhead_width_percent: 100,
            start_arrowhead_length_percent: 100,
            end_arrowhead_width_percent: 100,
            end_arrowhead_length_percent: 100,
            stroke_width_twips: 0,
            stroke_color: Color::default(),
            stroke_style: BorderStyle::Single,
            stroke_cap: StaticShapeLineCap::Flat,
            stroke_join: StaticShapeLineJoin::Miter,
            fill_opacity_percent: 100,
            stroke_opacity_percent: 100,
            fill_color: Some(Color {
                red: 200,
                green: 20,
                blue: 20,
            }),
            fill_gradient_color: None,
            fill_pattern: crate::model::ShadingPattern::None,
            shadow_enabled: false,
            shadow_color: Color {
                red: 128,
                green: 128,
                blue: 128,
            },
            shadow_opacity_percent: 100,
            shadow_offset_x_twips: 60,
            shadow_offset_y_twips: 60,
            text_margin_left_twips: 80,
            text_margin_right_twips: 80,
            text_margin_top_twips: 80,
            text_margin_bottom_twips: 80,
            text_vertical_anchor: StaticShapeTextVerticalAnchor::Top,
            text_horizontal_anchor_centered: false,
            text: vec![Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    dynamic_field: None,
                    text: "Layered Box".to_string(),
                    style: Default::default(),
                }],
            }],
            points: Vec::new(),
            point_paths: Vec::new(),
            overlay_paths: Vec::new(),
            fill_rule: StaticImageVectorFillRule::Winding,
        })];

        let layout = LayoutEngine::layout(&document);
        let layered_text = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Drawing(drawing) if drawing.z_order == 12 => {
                    match drawing.item.as_ref() {
                        LayoutItem::Text(fragment) => Some(fragment.text.as_str()),
                        _ => None,
                    }
                }
                _ => None,
            })
            .collect::<String>();

        assert_eq!(
            layered_text, "Layered Box",
            "z-ordered shape text should be wrapped in the shape drawing layer: {:?}",
            layout.pages[0].items
        );
    }

    #[test]
    fn word_modern_shape_text_uses_default_insets_and_foreground_layer() {
        let parsed = parse_rtf(include_str!("../../fixtures/shape-z-order-text-passive.rtf")).unwrap();
        let shape = parsed
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .expect("z-ordered modern shape");
        assert_eq!(shape.text_margin_left_twips, 144);
        assert_eq!(shape.text_margin_right_twips, 144);
        assert_eq!(shape.text_margin_top_twips, 80);
        assert_eq!(shape.text_margin_bottom_twips, 80);

        let layout = LayoutEngine::layout(&parsed.document);
        let items = &layout.pages[0].items;
        let after_index = items
            .iter()
            .position(|item| {
                matches!(item, LayoutItem::Text(fragment) if fragment.text.starts_with("After"))
            })
            .expect("body text after shape source position");
        let layered_index = items
            .iter()
            .position(|item| {
                matches!(item, LayoutItem::Drawing(drawing)
                    if matches!(drawing.item.as_ref(), LayoutItem::Text(fragment) if fragment.text.starts_with("Layered")))
            })
            .expect("shape text in foreground drawing layer");
        assert!(
            after_index < layered_index,
            "Word paints ordinary body text before foreground shape content: {items:?}"
        );
    }

    #[test]
    fn page_anchored_shape_picture_uses_page_origin() {
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
            display_width_twips: None,
            display_height_twips: None,
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: Some(StaticImagePlacement {
                left_twips: 720,
                top_twips: 720,
                width_twips: 720,
                height_twips: 720,
                z_order: 0,
                below_text: false,
                text_wrap: true,
                wrap_side: StaticImageWrapSide::Both,
                wrap_margin_left_twips: 120,
                wrap_margin_right_twips: 120,
                wrap_margin_top_twips: 0,
                wrap_margin_bottom_twips: 0,
                horizontal_anchor: StaticShapeHorizontalAnchor::Page,
                vertical_anchor: StaticShapeVerticalAnchor::Page,
            }),
        })];

        let layout = LayoutEngine::layout(&document);
        let image = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .expect("page anchored image");

        assert!((image.x - 36.0).abs() < 0.01);
        assert!((image.y - (layout.pages[0].height - 72.0)).abs() < 0.01);
    }

    #[test]
    fn below_text_passive_drawings_move_to_page_background_layer() {
        let mut document = Document::default();
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    dynamic_field: None,
                    text: "Body text".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::Shape(StaticShape {
                kind: StaticShapeKind::Rectangle,
                left_twips: 0,
                top_twips: 0,
                width_twips: 720,
                height_twips: 720,
                z_order: 0,
                below_text: true,
                text_wrap: false,
                wrap_side: StaticImageWrapSide::Both,
                wrap_margin_left_twips: 0,
                wrap_margin_right_twips: 0,
                wrap_margin_top_twips: 0,
                wrap_margin_bottom_twips: 0,
                horizontal_anchor: StaticShapeHorizontalAnchor::Column,
                vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
                flip_horizontal: false,
                flip_vertical: false,
                start_arrowhead: StaticShapeArrowhead::None,
                end_arrowhead: StaticShapeArrowhead::None,
                start_arrowhead_width_percent: 100,
                start_arrowhead_length_percent: 100,
                end_arrowhead_width_percent: 100,
                end_arrowhead_length_percent: 100,
                stroke_width_twips: 0,
                stroke_color: Color::default(),
                stroke_style: BorderStyle::Single,
                stroke_cap: StaticShapeLineCap::Flat,
                stroke_join: StaticShapeLineJoin::Miter,
                fill_opacity_percent: 100,
                stroke_opacity_percent: 100,
                fill_color: Some(Color {
                    red: 200,
                    green: 20,
                    blue: 20,
                }),
                fill_gradient_color: None,
                fill_pattern: crate::model::ShadingPattern::None,
                shadow_enabled: false,
                shadow_color: Color {
                    red: 128,
                    green: 128,
                    blue: 128,
                },
                shadow_opacity_percent: 100,
                shadow_offset_x_twips: 60,
                shadow_offset_y_twips: 60,
                text_margin_left_twips: 80,
                text_margin_right_twips: 80,
                text_margin_top_twips: 80,
                text_margin_bottom_twips: 80,
                text_vertical_anchor: StaticShapeTextVerticalAnchor::Top,
                text_horizontal_anchor_centered: false,
                text: Vec::new(),
                points: Vec::new(),
                point_paths: Vec::new(),
                overlay_paths: Vec::new(),
                fill_rule: StaticImageVectorFillRule::Winding,
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let first_drawing = layout.pages[0]
            .items
            .iter()
            .position(|item| matches!(item, LayoutItem::Drawing(_)))
            .expect("below-text drawing");
        let first_text = layout.pages[0]
            .items
            .iter()
            .position(
                |item| matches!(item, LayoutItem::Text(fragment) if fragment.text.contains("Body")),
            )
            .expect("body text");

        assert!(
            first_drawing < first_text,
            "below-text drawing should be emitted before text: drawing {first_drawing}, text {first_text}"
        );
    }

    #[test]
    fn page_and_margin_anchored_static_shapes_use_normalized_origins() {
        let mut page_document = Document::default();
        page_document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Rectangle,
            left_twips: 720,
            top_twips: 720,
            width_twips: 720,
            height_twips: 720,
            z_order: 0,
            below_text: false,
            text_wrap: false,
            wrap_side: StaticImageWrapSide::Both,
            wrap_margin_left_twips: 0,
            wrap_margin_right_twips: 0,
            wrap_margin_top_twips: 0,
            wrap_margin_bottom_twips: 0,
            horizontal_anchor: StaticShapeHorizontalAnchor::Page,
            vertical_anchor: StaticShapeVerticalAnchor::Page,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            start_arrowhead_width_percent: 100,
            start_arrowhead_length_percent: 100,
            end_arrowhead_width_percent: 100,
            end_arrowhead_length_percent: 100,
            stroke_width_twips: 0,
            stroke_color: Color::default(),
            stroke_style: BorderStyle::Single,
            stroke_cap: StaticShapeLineCap::Flat,
            stroke_join: StaticShapeLineJoin::Miter,
            fill_opacity_percent: 100,
            stroke_opacity_percent: 100,
            fill_color: Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            }),
            fill_gradient_color: None,
            fill_pattern: crate::model::ShadingPattern::None,
            shadow_enabled: false,
            shadow_color: Color {
                red: 128,
                green: 128,
                blue: 128,
            },
            shadow_opacity_percent: 100,
            shadow_offset_x_twips: 60,
            shadow_offset_y_twips: 60,
            text_margin_left_twips: 80,
            text_margin_right_twips: 80,
            text_margin_top_twips: 80,
            text_margin_bottom_twips: 80,
            text_vertical_anchor: StaticShapeTextVerticalAnchor::Top,
            text_horizontal_anchor_centered: false,
            text: Vec::new(),
            points: Vec::new(),
            point_paths: Vec::new(),
            overlay_paths: Vec::new(),
            fill_rule: StaticImageVectorFillRule::Winding,
        })];
        let page_layout = LayoutEngine::layout(&page_document);
        let page_fill = page_layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight { x, y, .. } => Some((*x, *y)),
                _ => None,
            })
            .expect("page anchored fill");

        assert!((page_fill.0 - 36.0).abs() < 0.01);
        assert!((page_fill.1 - (page_layout.pages[0].height - 72.0)).abs() < 0.01);

        let mut margin_document = page_document.clone();
        if let Block::Shape(shape) = &mut margin_document.blocks[0] {
            shape.horizontal_anchor = StaticShapeHorizontalAnchor::Margin;
            shape.vertical_anchor = StaticShapeVerticalAnchor::Margin;
        }
        let margin_layout = LayoutEngine::layout(&margin_document);
        let margin_fill = margin_layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight { x, y, .. } => Some((*x, *y)),
                _ => None,
            })
            .expect("margin anchored fill");

        assert!((margin_fill.0 - (72.0 + 36.0)).abs() < 0.01);
        assert!((margin_fill.1 - (margin_layout.pages[0].height - 72.0 - 72.0)).abs() < 0.01);
    }

    #[test]
    fn lays_out_legacy_static_drawing_shapes_as_passive_lines() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Rectangle,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            z_order: 0,
            below_text: false,
            text_wrap: false,
            wrap_side: StaticImageWrapSide::Both,
            wrap_margin_left_twips: 0,
            wrap_margin_right_twips: 0,
            wrap_margin_top_twips: 0,
            wrap_margin_bottom_twips: 0,
            horizontal_anchor: StaticShapeHorizontalAnchor::Column,
            vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            start_arrowhead_width_percent: 100,
            start_arrowhead_length_percent: 100,
            end_arrowhead_width_percent: 100,
            end_arrowhead_length_percent: 100,
            stroke_width_twips: 30,
            stroke_color: Color {
                red: 255,
                green: 128,
                blue: 0,
            },
            stroke_style: BorderStyle::Single,
            stroke_cap: StaticShapeLineCap::Flat,
            stroke_join: StaticShapeLineJoin::Miter,
            fill_opacity_percent: 100,
            stroke_opacity_percent: 100,
            fill_color: None,
            fill_gradient_color: None,
            fill_pattern: crate::model::ShadingPattern::None,
            shadow_enabled: false,
            shadow_color: Color {
                red: 128,
                green: 128,
                blue: 128,
            },
            shadow_opacity_percent: 100,
            shadow_offset_x_twips: 60,
            shadow_offset_y_twips: 60,
            text_margin_left_twips: 80,
            text_margin_right_twips: 80,
            text_margin_top_twips: 80,
            text_margin_bottom_twips: 80,
            text_vertical_anchor: StaticShapeTextVerticalAnchor::Top,
            text_horizontal_anchor_centered: false,
            text: Vec::new(),
            points: Vec::new(),
            point_paths: Vec::new(),
            overlay_paths: Vec::new(),
            fill_rule: StaticImageVectorFillRule::Winding,
        })];

        let layout = LayoutEngine::layout(&document);
        let lines = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    width,
                    color,
                    ..
                } => Some((*x1, *y1, *x2, *y2, *width, *color)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(lines.len(), 4);
        assert!(lines.iter().any(|(x1, y1, x2, y2, width, color)| {
            (*x1 - 90.0).abs() < 0.01
                && (*x2 - 162.0).abs() < 0.01
                && (*y1 - *y2).abs() < 0.01
                && (*width - 1.5).abs() < 0.01
                && (color.red - 1.0).abs() < 0.01
                && (color.green - 0.5019608).abs() < 0.01
                && color.blue == 0.0
        }));
    }

    #[test]
    fn lays_out_zero_width_static_drawing_outline_as_fill_only() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Rectangle,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            z_order: 0,
            below_text: false,
            text_wrap: false,
            wrap_side: StaticImageWrapSide::Both,
            wrap_margin_left_twips: 0,
            wrap_margin_right_twips: 0,
            wrap_margin_top_twips: 0,
            wrap_margin_bottom_twips: 0,
            horizontal_anchor: StaticShapeHorizontalAnchor::Column,
            vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            start_arrowhead_width_percent: 100,
            start_arrowhead_length_percent: 100,
            end_arrowhead_width_percent: 100,
            end_arrowhead_length_percent: 100,
            stroke_width_twips: 0,
            stroke_color: Color {
                red: 255,
                green: 0,
                blue: 0,
            },
            stroke_style: BorderStyle::Single,
            stroke_cap: StaticShapeLineCap::Flat,
            stroke_join: StaticShapeLineJoin::Miter,
            fill_opacity_percent: 100,
            stroke_opacity_percent: 100,
            fill_color: Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            }),
            fill_gradient_color: None,
            fill_pattern: crate::model::ShadingPattern::None,
            shadow_enabled: false,
            shadow_color: Color {
                red: 128,
                green: 128,
                blue: 128,
            },
            shadow_opacity_percent: 100,
            shadow_offset_x_twips: 60,
            shadow_offset_y_twips: 60,
            text_margin_left_twips: 80,
            text_margin_right_twips: 80,
            text_margin_top_twips: 80,
            text_margin_bottom_twips: 80,
            text_vertical_anchor: StaticShapeTextVerticalAnchor::Top,
            text_horizontal_anchor_centered: false,
            text: Vec::new(),
            points: Vec::new(),
            point_paths: Vec::new(),
            overlay_paths: Vec::new(),
            fill_rule: StaticImageVectorFillRule::Winding,
        })];

        let layout = LayoutEngine::layout(&document);
        let fill_count = layout.pages[0]
            .items
            .iter()
            .filter(|item| matches!(item, LayoutItem::Highlight { .. }))
            .count();
        let stroke_count = layout.pages[0]
            .items
            .iter()
            .filter(|item| matches!(item, LayoutItem::Line { .. }))
            .count();

        assert_eq!(fill_count, 1);
        assert_eq!(stroke_count, 0);
    }

    #[test]
    fn lays_out_shape_text_paragraph_shading_and_borders_inside_shape_bounds() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 240,
                green: 240,
                blue: 0,
            },
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
        ];
        let mut paragraph_style = ParagraphStyle {
            shading_color_index: Some(1),
            ..ParagraphStyle::default()
        };
        paragraph_style.borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 40,
            color_index: Some(2),
            ..TableCellBorder::default()
        };
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Rectangle,
            left_twips: 720,
            top_twips: 720,
            width_twips: 2160,
            height_twips: 720,
            z_order: 0,
            below_text: false,
            text_wrap: false,
            wrap_side: StaticImageWrapSide::Both,
            wrap_margin_left_twips: 0,
            wrap_margin_right_twips: 0,
            wrap_margin_top_twips: 0,
            wrap_margin_bottom_twips: 0,
            horizontal_anchor: StaticShapeHorizontalAnchor::Column,
            vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            start_arrowhead_width_percent: 100,
            start_arrowhead_length_percent: 100,
            end_arrowhead_width_percent: 100,
            end_arrowhead_length_percent: 100,
            stroke_width_twips: 0,
            stroke_color: Color::default(),
            stroke_style: BorderStyle::Single,
            stroke_cap: StaticShapeLineCap::Flat,
            stroke_join: StaticShapeLineJoin::Miter,
            fill_opacity_percent: 100,
            stroke_opacity_percent: 100,
            fill_color: None,
            fill_gradient_color: None,
            fill_pattern: crate::model::ShadingPattern::None,
            shadow_enabled: false,
            shadow_color: Color {
                red: 128,
                green: 128,
                blue: 128,
            },
            shadow_opacity_percent: 100,
            shadow_offset_x_twips: 60,
            shadow_offset_y_twips: 60,
            text_margin_left_twips: 360,
            text_margin_right_twips: 180,
            text_margin_top_twips: 240,
            text_margin_bottom_twips: 120,
            text_vertical_anchor: StaticShapeTextVerticalAnchor::Top,
            text_horizontal_anchor_centered: false,
            text: vec![Paragraph {
                style: paragraph_style,
                runs: vec![Run {
                    dynamic_field: None,
                    text: "Styled shape".to_string(),
                    style: Default::default(),
                }],
            }],
            points: Vec::new(),
            point_paths: Vec::new(),
            overlay_paths: Vec::new(),
            fill_rule: StaticImageVectorFillRule::Winding,
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let shape_left = 108.0;
        let shape_right = 216.0;
        let shape_bottom = 648.0;
        let shape_top = 684.0;
        let content_left = shape_left + 18.0;
        let content_right = shape_right - 9.0;
        let content_bottom = shape_bottom + 6.0;
        let content_top = shape_top - 12.0;
        let text = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text.contains("Styled") => Some(fragment),
                _ => None,
            })
            .expect("shape text fragment");

        assert!(text.x >= content_left && text.x < content_right);
        assert!(text.baseline_y > content_bottom && text.baseline_y < content_top);
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Highlight { x, y, width, height, color }
                if *x >= content_left
                    && *x + *width <= content_right + 0.01
                    && *y >= content_bottom
                    && *y + *height <= content_top
                    && *color == PdfColor {
                        red: 240.0 / 255.0,
                        green: 240.0 / 255.0,
                        blue: 0.0
                    }
        )));
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, y1, x2, y2, color, .. }
                if *x1 >= content_left
                    && *x2 <= content_right + 0.01
                    && (*y1 - *y2).abs() < 0.01
                    && *y1 >= content_bottom
                    && *y1 <= content_top
                    && *color == PdfColor {
                        red: 1.0,
                        green: 0.0,
                        blue: 0.0
                    }
        )));
    }

    #[test]
    fn lays_out_bottom_anchored_shape_text_inside_passive_shape_bounds() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Rectangle,
            left_twips: 720,
            top_twips: 720,
            width_twips: 2160,
            height_twips: 1440,
            z_order: 0,
            below_text: false,
            text_wrap: false,
            wrap_side: StaticImageWrapSide::Both,
            wrap_margin_left_twips: 0,
            wrap_margin_right_twips: 0,
            wrap_margin_top_twips: 0,
            wrap_margin_bottom_twips: 0,
            horizontal_anchor: StaticShapeHorizontalAnchor::Column,
            vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            start_arrowhead_width_percent: 100,
            start_arrowhead_length_percent: 100,
            end_arrowhead_width_percent: 100,
            end_arrowhead_length_percent: 100,
            stroke_width_twips: 0,
            stroke_color: Color::default(),
            stroke_style: BorderStyle::Single,
            stroke_cap: StaticShapeLineCap::Flat,
            stroke_join: StaticShapeLineJoin::Miter,
            fill_opacity_percent: 100,
            stroke_opacity_percent: 100,
            fill_color: None,
            fill_gradient_color: None,
            fill_pattern: crate::model::ShadingPattern::None,
            shadow_enabled: false,
            shadow_color: Color {
                red: 128,
                green: 128,
                blue: 128,
            },
            shadow_opacity_percent: 100,
            shadow_offset_x_twips: 60,
            shadow_offset_y_twips: 60,
            text_margin_left_twips: 80,
            text_margin_right_twips: 80,
            text_margin_top_twips: 80,
            text_margin_bottom_twips: 80,
            text_vertical_anchor: StaticShapeTextVerticalAnchor::Bottom,
            text_horizontal_anchor_centered: false,
            text: vec![Paragraph {
                style: ParagraphStyle::default(),
                runs: vec![Run {
                    dynamic_field: None,
                    text: "Bottom anchored".to_string(),
                    style: Default::default(),
                }],
            }],
            points: Vec::new(),
            point_paths: Vec::new(),
            overlay_paths: Vec::new(),
            fill_rule: StaticImageVectorFillRule::Winding,
        })];

        let layout = LayoutEngine::layout(&document);
        let fragment = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text.contains("Bottom") => Some(fragment),
                _ => None,
            })
            .expect("bottom anchored shape text");
        let shape_bottom = 612.0;
        let shape_top = 684.0;

        assert!(fragment.baseline_y > shape_bottom + 4.0);
        assert!(fragment.baseline_y < shape_top - 4.0);
        assert!(
            fragment.baseline_y < shape_bottom + 28.0,
            "bottom anchored text should move toward the lower content edge: {:?}",
            fragment
        );
    }

    #[test]
    fn lays_out_center_anchored_shape_text_horizontally_inside_passive_shape_bounds() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Rectangle,
            left_twips: 720,
            top_twips: 720,
            width_twips: 2880,
            height_twips: 720,
            z_order: 0,
            below_text: false,
            text_wrap: false,
            wrap_side: StaticImageWrapSide::Both,
            wrap_margin_left_twips: 0,
            wrap_margin_right_twips: 0,
            wrap_margin_top_twips: 0,
            wrap_margin_bottom_twips: 0,
            horizontal_anchor: StaticShapeHorizontalAnchor::Column,
            vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            start_arrowhead_width_percent: 100,
            start_arrowhead_length_percent: 100,
            end_arrowhead_width_percent: 100,
            end_arrowhead_length_percent: 100,
            stroke_width_twips: 0,
            stroke_color: Color::default(),
            stroke_style: BorderStyle::Single,
            stroke_cap: StaticShapeLineCap::Flat,
            stroke_join: StaticShapeLineJoin::Miter,
            fill_opacity_percent: 100,
            stroke_opacity_percent: 100,
            fill_color: None,
            fill_gradient_color: None,
            fill_pattern: crate::model::ShadingPattern::None,
            shadow_enabled: false,
            shadow_color: Color {
                red: 128,
                green: 128,
                blue: 128,
            },
            shadow_opacity_percent: 100,
            shadow_offset_x_twips: 60,
            shadow_offset_y_twips: 60,
            text_margin_left_twips: 80,
            text_margin_right_twips: 80,
            text_margin_top_twips: 80,
            text_margin_bottom_twips: 80,
            text_vertical_anchor: StaticShapeTextVerticalAnchor::Top,
            text_horizontal_anchor_centered: true,
            text: vec![Paragraph {
                style: ParagraphStyle::default(),
                runs: vec![Run {
                    dynamic_field: None,
                    text: "Centered".to_string(),
                    style: Default::default(),
                }],
            }],
            points: Vec::new(),
            point_paths: Vec::new(),
            overlay_paths: Vec::new(),
            fill_rule: StaticImageVectorFillRule::Winding,
        })];

        let centered_layout = LayoutEngine::layout(&document);
        let centered_fragment = centered_layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text.contains("Centered") => Some(fragment),
                _ => None,
            })
            .expect("center anchored shape text");
        let mut left_document = document.clone();
        if let Block::Shape(shape) = &mut left_document.blocks[0] {
            shape.text_horizontal_anchor_centered = false;
        }
        let left_layout = LayoutEngine::layout(&left_document);
        let left_fragment = left_layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text.contains("Centered") => Some(fragment),
                _ => None,
            })
            .expect("left anchored shape text");

        assert!(
            centered_fragment.x > left_fragment.x + 20.0,
            "center anchored shape text should move right from normal left placement: centered={:?} left={:?}",
            centered_fragment,
            left_fragment
        );
    }

    #[test]
    fn lays_out_legacy_static_drawing_line_styles_as_passive_line_styles() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Line,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            z_order: 0,
            below_text: false,
            text_wrap: false,
            wrap_side: StaticImageWrapSide::Both,
            wrap_margin_left_twips: 0,
            wrap_margin_right_twips: 0,
            wrap_margin_top_twips: 0,
            wrap_margin_bottom_twips: 0,
            horizontal_anchor: StaticShapeHorizontalAnchor::Column,
            vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            start_arrowhead_width_percent: 100,
            start_arrowhead_length_percent: 100,
            end_arrowhead_width_percent: 100,
            end_arrowhead_length_percent: 100,
            stroke_width_twips: 30,
            stroke_color: Color::default(),
            stroke_style: BorderStyle::Dashed,
            stroke_cap: StaticShapeLineCap::Flat,
            stroke_join: StaticShapeLineJoin::Miter,
            fill_opacity_percent: 100,
            stroke_opacity_percent: 100,
            fill_color: None,
            fill_gradient_color: None,
            fill_pattern: crate::model::ShadingPattern::None,
            shadow_enabled: false,
            shadow_color: Color {
                red: 128,
                green: 128,
                blue: 128,
            },
            shadow_opacity_percent: 100,
            shadow_offset_x_twips: 60,
            shadow_offset_y_twips: 60,
            text_margin_left_twips: 80,
            text_margin_right_twips: 80,
            text_margin_top_twips: 80,
            text_margin_bottom_twips: 80,
            text_vertical_anchor: StaticShapeTextVerticalAnchor::Top,
            text_horizontal_anchor_centered: false,
            text: Vec::new(),
            points: Vec::new(),
            point_paths: Vec::new(),
            overlay_paths: Vec::new(),
            fill_rule: StaticImageVectorFillRule::Winding,
        })];

        let layout = LayoutEngine::layout(&document);
        let line = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Line { width, style, .. } => Some((*width, *style)),
                _ => None,
            })
            .expect("styled line");

        assert!((line.0 - 1.5).abs() < 0.01);
        assert_eq!(line.1, LineStyle::Dashed);
    }

    #[test]
    fn lays_out_flipped_static_drawing_lines_as_mirrored_passive_geometry() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Line,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            z_order: 0,
            below_text: false,
            text_wrap: false,
            wrap_side: StaticImageWrapSide::Both,
            wrap_margin_left_twips: 0,
            wrap_margin_right_twips: 0,
            wrap_margin_top_twips: 0,
            wrap_margin_bottom_twips: 0,
            horizontal_anchor: StaticShapeHorizontalAnchor::Column,
            vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
            flip_horizontal: true,
            flip_vertical: true,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            start_arrowhead_width_percent: 100,
            start_arrowhead_length_percent: 100,
            end_arrowhead_width_percent: 100,
            end_arrowhead_length_percent: 100,
            stroke_width_twips: 30,
            stroke_color: Color::default(),
            stroke_style: BorderStyle::Single,
            stroke_cap: StaticShapeLineCap::Flat,
            stroke_join: StaticShapeLineJoin::Miter,
            fill_opacity_percent: 100,
            stroke_opacity_percent: 100,
            fill_color: None,
            fill_gradient_color: None,
            fill_pattern: crate::model::ShadingPattern::None,
            shadow_enabled: false,
            shadow_color: Color {
                red: 128,
                green: 128,
                blue: 128,
            },
            shadow_opacity_percent: 100,
            shadow_offset_x_twips: 60,
            shadow_offset_y_twips: 60,
            text_margin_left_twips: 80,
            text_margin_right_twips: 80,
            text_margin_top_twips: 80,
            text_margin_bottom_twips: 80,
            text_vertical_anchor: StaticShapeTextVerticalAnchor::Top,
            text_horizontal_anchor_centered: false,
            text: Vec::new(),
            points: Vec::new(),
            point_paths: Vec::new(),
            overlay_paths: Vec::new(),
            fill_rule: StaticImageVectorFillRule::Winding,
        })];

        let layout = LayoutEngine::layout(&document);
        let line = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Line { x1, y1, x2, y2, .. } => Some((*x1, *y1, *x2, *y2)),
                _ => None,
            })
            .expect("flipped line");

        assert!((line.0 - 162.0).abs() < 0.01);
        assert!((line.1 - 672.0).abs() < 0.01);
        assert!((line.2 - 90.0).abs() < 0.01);
        assert!((line.3 - 708.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_static_drawing_arrowheads_as_passive_geometry() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Line,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            z_order: 0,
            below_text: false,
            text_wrap: false,
            wrap_side: StaticImageWrapSide::Both,
            wrap_margin_left_twips: 0,
            wrap_margin_right_twips: 0,
            wrap_margin_top_twips: 0,
            wrap_margin_bottom_twips: 0,
            horizontal_anchor: StaticShapeHorizontalAnchor::Column,
            vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::Open,
            end_arrowhead: StaticShapeArrowhead::Triangle,
            start_arrowhead_width_percent: 100,
            start_arrowhead_length_percent: 100,
            end_arrowhead_width_percent: 100,
            end_arrowhead_length_percent: 100,
            stroke_width_twips: 30,
            stroke_color: Color {
                red: 255,
                green: 0,
                blue: 0,
            },
            stroke_style: BorderStyle::Single,
            stroke_cap: StaticShapeLineCap::Flat,
            stroke_join: StaticShapeLineJoin::Miter,
            fill_opacity_percent: 100,
            stroke_opacity_percent: 100,
            fill_color: None,
            fill_gradient_color: None,
            fill_pattern: crate::model::ShadingPattern::None,
            shadow_enabled: false,
            shadow_color: Color {
                red: 128,
                green: 128,
                blue: 128,
            },
            shadow_opacity_percent: 100,
            shadow_offset_x_twips: 60,
            shadow_offset_y_twips: 60,
            text_margin_left_twips: 80,
            text_margin_right_twips: 80,
            text_margin_top_twips: 80,
            text_margin_bottom_twips: 80,
            text_vertical_anchor: StaticShapeTextVerticalAnchor::Top,
            text_horizontal_anchor_centered: false,
            text: Vec::new(),
            points: Vec::new(),
            point_paths: Vec::new(),
            overlay_paths: Vec::new(),
            fill_rule: StaticImageVectorFillRule::Winding,
        })];

        let layout = LayoutEngine::layout(&document);
        let lines = layout.pages[0]
            .items
            .iter()
            .filter(|item| matches!(item, LayoutItem::Line { .. }))
            .count();
        let arrow_polygon = layout.pages[0].items.iter().find_map(|item| match item {
            LayoutItem::Polygon {
                points, fill_color, ..
            } => Some((points, fill_color)),
            _ => None,
        });

        assert_eq!(lines, 3);
        let (points, fill_color) = arrow_polygon.expect("filled triangle arrowhead");
        assert_eq!(points.len(), 3);
        assert_eq!(
            *fill_color,
            Some(PdfColor {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
            })
        );
    }

    #[test]
    fn lays_out_legacy_static_drawing_polylines_as_passive_line_segments() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Polyline,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            z_order: 0,
            below_text: false,
            text_wrap: false,
            wrap_side: StaticImageWrapSide::Both,
            wrap_margin_left_twips: 0,
            wrap_margin_right_twips: 0,
            wrap_margin_top_twips: 0,
            wrap_margin_bottom_twips: 0,
            horizontal_anchor: StaticShapeHorizontalAnchor::Column,
            vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            start_arrowhead_width_percent: 100,
            start_arrowhead_length_percent: 100,
            end_arrowhead_width_percent: 100,
            end_arrowhead_length_percent: 100,
            stroke_width_twips: 30,
            stroke_color: Color::default(),
            stroke_style: BorderStyle::Dotted,
            stroke_cap: StaticShapeLineCap::Flat,
            stroke_join: StaticShapeLineJoin::Miter,
            fill_opacity_percent: 100,
            stroke_opacity_percent: 100,
            fill_color: None,
            fill_gradient_color: None,
            fill_pattern: crate::model::ShadingPattern::None,
            shadow_enabled: false,
            shadow_color: Color {
                red: 128,
                green: 128,
                blue: 128,
            },
            shadow_opacity_percent: 100,
            shadow_offset_x_twips: 60,
            shadow_offset_y_twips: 60,
            text_margin_left_twips: 80,
            text_margin_right_twips: 80,
            text_margin_top_twips: 80,
            text_margin_bottom_twips: 80,
            text_vertical_anchor: StaticShapeTextVerticalAnchor::Top,
            text_horizontal_anchor_centered: false,
            text: Vec::new(),
            points: vec![
                StaticShapePoint {
                    x_twips: 0,
                    y_twips: 0,
                },
                StaticShapePoint {
                    x_twips: 720,
                    y_twips: 720,
                },
                StaticShapePoint {
                    x_twips: 1440,
                    y_twips: 0,
                },
            ],
            point_paths: Vec::new(),
            overlay_paths: Vec::new(),
            fill_rule: StaticImageVectorFillRule::Winding,
        })];

        let layout = LayoutEngine::layout(&document);
        let lines = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    width,
                    style,
                    ..
                } => Some((*x1, *y1, *x2, *y2, *width, *style)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(lines.len(), 2);
        assert!(lines.iter().any(|line| {
            (line.0 - 90.0).abs() < 0.01
                && (line.2 - 126.0).abs() < 0.01
                && (line.4 - 1.5).abs() < 0.01
                && line.5 == LineStyle::Dotted
        }));
        assert!(lines.iter().any(|line| {
            (line.0 - 126.0).abs() < 0.01
                && (line.2 - 162.0).abs() < 0.01
                && line.5 == LineStyle::Dotted
        }));
    }

    #[test]
    fn paragraph_anchored_static_shape_does_not_advance_body_flow() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1\ansi\deff0{\fonttbl{\f0 Arial;}}\fs24 Before.\par{\do\dpline\dpx360\dpy480\dpxsize1440\dpysize720}After.\par}",
        )
        .unwrap();
        let layout = LayoutEngine::layout(&parsed.document);
        let text = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment),
                _ => None,
            })
            .collect::<Vec<_>>();
        let before = text
            .iter()
            .find(|fragment| fragment.text.contains("Before."))
            .expect("before text");
        let after = text
            .iter()
            .find(|fragment| fragment.text.contains("After."))
            .expect("after text");
        let line = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Line { x1, y1, x2, y2, .. } => Some((*x1, *y1, *x2, *y2)),
                _ => None,
            })
            .expect("shape line");

        assert!(
            (before.baseline_y - after.baseline_y - 15.0).abs() < 0.05,
            "before={}, after={}",
            before.baseline_y,
            after.baseline_y
        );
        assert!((line.0 - 90.0).abs() < 0.01);
        assert!((line.1 - 681.0).abs() < 0.01);
        assert!((line.2 - 162.0).abs() < 0.01);
        assert!((line.3 - 645.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_legacy_static_drawing_polygons_as_passive_closed_paths() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Polygon,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            z_order: 0,
            below_text: false,
            text_wrap: false,
            wrap_side: StaticImageWrapSide::Both,
            wrap_margin_left_twips: 0,
            wrap_margin_right_twips: 0,
            wrap_margin_top_twips: 0,
            wrap_margin_bottom_twips: 0,
            horizontal_anchor: StaticShapeHorizontalAnchor::Column,
            vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            start_arrowhead_width_percent: 100,
            start_arrowhead_length_percent: 100,
            end_arrowhead_width_percent: 100,
            end_arrowhead_length_percent: 100,
            stroke_width_twips: 30,
            stroke_color: Color::default(),
            stroke_style: BorderStyle::Dotted,
            stroke_cap: StaticShapeLineCap::Flat,
            stroke_join: StaticShapeLineJoin::Miter,
            fill_opacity_percent: 100,
            stroke_opacity_percent: 100,
            fill_color: Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            }),
            fill_gradient_color: None,
            fill_pattern: crate::model::ShadingPattern::None,
            shadow_enabled: false,
            shadow_color: Color {
                red: 128,
                green: 128,
                blue: 128,
            },
            shadow_opacity_percent: 100,
            shadow_offset_x_twips: 60,
            shadow_offset_y_twips: 60,
            text_margin_left_twips: 80,
            text_margin_right_twips: 80,
            text_margin_top_twips: 80,
            text_margin_bottom_twips: 80,
            text_vertical_anchor: StaticShapeTextVerticalAnchor::Top,
            text_horizontal_anchor_centered: false,
            text: Vec::new(),
            points: vec![
                StaticShapePoint {
                    x_twips: 0,
                    y_twips: 0,
                },
                StaticShapePoint {
                    x_twips: 720,
                    y_twips: 720,
                },
                StaticShapePoint {
                    x_twips: 1440,
                    y_twips: 0,
                },
            ],
            point_paths: Vec::new(),
            overlay_paths: Vec::new(),
            fill_rule: StaticImageVectorFillRule::Winding,
        })];

        let layout = LayoutEngine::layout(&document);
        let polygon = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Polygon {
                    points,
                    stroke_width,
                    stroke_style,
                    fill_color,
                    ..
                } => Some((points.clone(), *stroke_width, *stroke_style, *fill_color)),
                _ => None,
            })
            .expect("polygon");

        assert_eq!(polygon.0.len(), 3);
        assert!((polygon.0[0].x - 90.0).abs() < 0.01);
        assert!((polygon.0[1].x - 126.0).abs() < 0.01);
        assert!((polygon.0[2].x - 162.0).abs() < 0.01);
        assert!((polygon.1 - 1.5).abs() < 0.01);
        assert_eq!(polygon.2, LineStyle::Dotted);
        let fill = polygon.3.expect("polygon fill");
        assert!((fill.red - (10.0 / 255.0)).abs() < 0.01);
        assert!((fill.green - (20.0 / 255.0)).abs() < 0.01);
        assert!((fill.blue - (30.0 / 255.0)).abs() < 0.01);
    }

    #[test]
    fn lays_out_filled_legacy_static_drawing_rectangles_as_passive_fill() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Rectangle,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            z_order: 0,
            below_text: false,
            text_wrap: false,
            wrap_side: StaticImageWrapSide::Both,
            wrap_margin_left_twips: 0,
            wrap_margin_right_twips: 0,
            wrap_margin_top_twips: 0,
            wrap_margin_bottom_twips: 0,
            horizontal_anchor: StaticShapeHorizontalAnchor::Column,
            vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            start_arrowhead_width_percent: 100,
            start_arrowhead_length_percent: 100,
            end_arrowhead_width_percent: 100,
            end_arrowhead_length_percent: 100,
            stroke_width_twips: 20,
            stroke_color: Color::default(),
            stroke_style: BorderStyle::Single,
            stroke_cap: StaticShapeLineCap::Flat,
            stroke_join: StaticShapeLineJoin::Miter,
            fill_opacity_percent: 100,
            stroke_opacity_percent: 100,
            fill_color: Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            }),
            fill_gradient_color: None,
            fill_pattern: crate::model::ShadingPattern::None,
            shadow_enabled: false,
            shadow_color: Color {
                red: 128,
                green: 128,
                blue: 128,
            },
            shadow_opacity_percent: 100,
            shadow_offset_x_twips: 60,
            shadow_offset_y_twips: 60,
            text_margin_left_twips: 80,
            text_margin_right_twips: 80,
            text_margin_top_twips: 80,
            text_margin_bottom_twips: 80,
            text_vertical_anchor: StaticShapeTextVerticalAnchor::Top,
            text_horizontal_anchor_centered: false,
            text: Vec::new(),
            points: Vec::new(),
            point_paths: Vec::new(),
            overlay_paths: Vec::new(),
            fill_rule: StaticImageVectorFillRule::Winding,
        })];

        let layout = LayoutEngine::layout(&document);
        let fill = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight {
                    x,
                    y,
                    width,
                    height,
                    color,
                } => Some((*x, *y, *width, *height, *color)),
                _ => None,
            })
            .expect("filled rectangle");

        assert!((fill.0 - 90.0).abs() < 0.01);
        assert!((fill.2 - 72.0).abs() < 0.01);
        assert!((fill.3 - 36.0).abs() < 0.01);
        assert!((fill.4.red - (10.0 / 255.0)).abs() < 0.01);
        assert!((fill.4.green - (20.0 / 255.0)).abs() < 0.01);
        assert!((fill.4.blue - (30.0 / 255.0)).abs() < 0.01);
    }

    #[test]
    fn lays_out_legacy_static_drawing_ellipses_as_passive_paths() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Ellipse,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            z_order: 0,
            below_text: false,
            text_wrap: false,
            wrap_side: StaticImageWrapSide::Both,
            wrap_margin_left_twips: 0,
            wrap_margin_right_twips: 0,
            wrap_margin_top_twips: 0,
            wrap_margin_bottom_twips: 0,
            horizontal_anchor: StaticShapeHorizontalAnchor::Column,
            vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            start_arrowhead_width_percent: 100,
            start_arrowhead_length_percent: 100,
            end_arrowhead_width_percent: 100,
            end_arrowhead_length_percent: 100,
            stroke_width_twips: 20,
            stroke_color: Color {
                red: 255,
                green: 128,
                blue: 0,
            },
            stroke_style: BorderStyle::Single,
            stroke_cap: StaticShapeLineCap::Flat,
            stroke_join: StaticShapeLineJoin::Miter,
            fill_opacity_percent: 100,
            stroke_opacity_percent: 100,
            fill_color: Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            }),
            fill_gradient_color: None,
            fill_pattern: crate::model::ShadingPattern::None,
            shadow_enabled: false,
            shadow_color: Color {
                red: 128,
                green: 128,
                blue: 128,
            },
            shadow_opacity_percent: 100,
            shadow_offset_x_twips: 60,
            shadow_offset_y_twips: 60,
            text_margin_left_twips: 80,
            text_margin_right_twips: 80,
            text_margin_top_twips: 80,
            text_margin_bottom_twips: 80,
            text_vertical_anchor: StaticShapeTextVerticalAnchor::Top,
            text_horizontal_anchor_centered: false,
            text: Vec::new(),
            points: Vec::new(),
            point_paths: Vec::new(),
            overlay_paths: Vec::new(),
            fill_rule: StaticImageVectorFillRule::Winding,
        })];

        let layout = LayoutEngine::layout(&document);
        let ellipse = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Ellipse {
                    x,
                    y,
                    width,
                    height,
                    stroke_width,
                    stroke_color,
                    stroke_style,
                    fill_color,
                    ..
                } => Some((
                    *x,
                    *y,
                    *width,
                    *height,
                    *stroke_width,
                    *stroke_color,
                    *stroke_style,
                    *fill_color,
                )),
                _ => None,
            })
            .expect("ellipse");

        assert!((ellipse.0 - 90.0).abs() < 0.01);
        assert!((ellipse.2 - 72.0).abs() < 0.01);
        assert!((ellipse.3 - 36.0).abs() < 0.01);
        assert!((ellipse.4 - 1.0).abs() < 0.01);
        assert!((ellipse.5.red - 1.0).abs() < 0.01);
        assert!((ellipse.5.green - 0.5019608).abs() < 0.01);
        assert_eq!(ellipse.5.blue, 0.0);
        assert_eq!(ellipse.6, LineStyle::Solid);
        let fill = ellipse.7.expect("ellipse fill");
        assert!((fill.red - (10.0 / 255.0)).abs() < 0.01);
        assert!((fill.green - (20.0 / 255.0)).abs() < 0.01);
        assert!((fill.blue - (30.0 / 255.0)).abs() < 0.01);
    }

    #[test]
    fn lays_out_legacy_static_drawing_rounded_rectangles_as_passive_paths() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::RoundedRectangle,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            z_order: 0,
            below_text: false,
            text_wrap: false,
            wrap_side: StaticImageWrapSide::Both,
            wrap_margin_left_twips: 0,
            wrap_margin_right_twips: 0,
            wrap_margin_top_twips: 0,
            wrap_margin_bottom_twips: 0,
            horizontal_anchor: StaticShapeHorizontalAnchor::Column,
            vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            start_arrowhead_width_percent: 100,
            start_arrowhead_length_percent: 100,
            end_arrowhead_width_percent: 100,
            end_arrowhead_length_percent: 100,
            stroke_width_twips: 20,
            stroke_color: Color {
                red: 255,
                green: 128,
                blue: 0,
            },
            stroke_style: BorderStyle::Single,
            stroke_cap: StaticShapeLineCap::Flat,
            stroke_join: StaticShapeLineJoin::Miter,
            fill_opacity_percent: 100,
            stroke_opacity_percent: 100,
            fill_color: Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            }),
            fill_gradient_color: None,
            fill_pattern: crate::model::ShadingPattern::None,
            shadow_enabled: false,
            shadow_color: Color {
                red: 128,
                green: 128,
                blue: 128,
            },
            shadow_opacity_percent: 100,
            shadow_offset_x_twips: 60,
            shadow_offset_y_twips: 60,
            text_margin_left_twips: 80,
            text_margin_right_twips: 80,
            text_margin_top_twips: 80,
            text_margin_bottom_twips: 80,
            text_vertical_anchor: StaticShapeTextVerticalAnchor::Top,
            text_horizontal_anchor_centered: false,
            text: Vec::new(),
            points: Vec::new(),
            point_paths: Vec::new(),
            overlay_paths: Vec::new(),
            fill_rule: StaticImageVectorFillRule::Winding,
        })];

        let layout = LayoutEngine::layout(&document);
        let rounded = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::RoundedRectangle {
                    x,
                    y,
                    width,
                    height,
                    radius,
                    stroke_width,
                    stroke_color,
                    stroke_style,
                    fill_color,
                    ..
                } => Some((
                    *x,
                    *y,
                    *width,
                    *height,
                    *radius,
                    *stroke_width,
                    *stroke_color,
                    *stroke_style,
                    *fill_color,
                )),
                _ => None,
            })
            .expect("rounded rectangle");

        assert!((rounded.0 - 90.0).abs() < 0.01);
        assert!((rounded.2 - 72.0).abs() < 0.01);
        assert!((rounded.3 - 36.0).abs() < 0.01);
        assert!((rounded.4 - 7.2).abs() < 0.01);
        assert!((rounded.5 - 1.0).abs() < 0.01);
        assert!((rounded.6.red - 1.0).abs() < 0.01);
        assert!((rounded.6.green - 0.5019608).abs() < 0.01);
        assert_eq!(rounded.6.blue, 0.0);
        assert_eq!(rounded.7, LineStyle::Solid);
        let fill = rounded.8.expect("rounded rectangle fill");
        assert!((fill.red - (10.0 / 255.0)).abs() < 0.01);
        assert!((fill.green - (20.0 / 255.0)).abs() < 0.01);
        assert!((fill.blue - (30.0 / 255.0)).abs() < 0.01);
    }

    #[test]
    fn justification_excludes_wrapped_trailing_space_at_every_character_scale() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1\ansi\paperw4320\paperh7920\margl360\margr360\fs24
            \qj Plain one two three four five six seven eight nine ten eleven twelve.\par
            \qj\charscalex50 Scale50 one two three four five six seven eight nine ten eleven twelve.\par
            \qj\charscalex150 Scale150 one two three four five six seven eight nine ten eleven twelve.\par}",
        )
        .unwrap();
        let layout = LayoutEngine::layout(&parsed.document);
        let right_edge = 198.0;
        let mut lines = std::collections::BTreeMap::<i32, Vec<&TextFragment>>::new();
        for item in &layout.pages[0].items {
            if let LayoutItem::Text(fragment) = item {
                lines
                    .entry((fragment.baseline_y * 100.0).round() as i32)
                    .or_default()
                    .push(fragment);
            }
        }
        let justified_endpoints = lines
            .values()
            .filter(|line| line.iter().any(|fragment| fragment.word_spacing > 0.0))
            .map(|line| {
                line.iter()
                    .map(|fragment| {
                        fragment.x
                            + measure_text_with_family(
                                fragment.text.trim_end_matches(' '),
                                &fragment.style,
                                fragment.font_family,
                            )
                    })
                    .fold(f32::NEG_INFINITY, f32::max)
            })
            .collect::<Vec<_>>();
        assert!(justified_endpoints.len() >= 3, "{justified_endpoints:?}");
        assert!(
            justified_endpoints
                .iter()
                .all(|endpoint| (*endpoint - right_edge).abs() < 0.02),
            "justified glyph endpoints must reach the body edge: {justified_endpoints:?}"
        );
    }

    #[test]
    fn wraps_optional_hyphen_as_visible_only_at_line_end() {
        let document = Document::default();
        let style = CharacterStyle::default();
        let paragraph = Paragraph {
            style: Default::default(),
            runs: vec![Run {
                dynamic_field: None,
                text: "Alpha\u{00ad}Beta".to_string(),
                style: style.clone(),
            }],
        };

        let markers = test_markers("1", "1");
        let wide_lines = wrap_paragraph(&paragraph, 500.0, &markers, &document);
        assert_eq!(line_text(&wide_lines[0]), "AlphaBeta");

        let narrow_width = measure_text("Alpha", &style) + 1.0;
        let narrow_lines = wrap_paragraph(&paragraph, narrow_width, &markers, &document);
        assert_eq!(narrow_lines.len(), 2);
        assert_eq!(line_text(&narrow_lines[0]), "Alpha-");
        assert_eq!(line_text(&narrow_lines[1]), "Beta");
    }

    #[test]
    fn emergency_wrapping_breaks_long_words_without_synthetic_hyphens() {
        let document = Document::default();
        let style = CharacterStyle::default();
        let word = "Antidisestablishmentarianism";
        let mut hyphenated_style = ParagraphStyle::default();
        hyphenated_style.auto_hyphenation = true;
        let hyphenated = Paragraph {
            style: hyphenated_style,
            runs: vec![Run {
                dynamic_field: None,
                text: word.to_string(),
                style: style.clone(),
            }],
        };
        let plain = Paragraph {
            style: Default::default(),
            runs: hyphenated.runs.clone(),
        };

        let markers = test_markers("1", "1");
        let width = measure_text("Antidis", &style);
        let plain_lines = wrap_paragraph(&plain, width, &markers, &document);
        let hyphenated_lines = wrap_paragraph(&hyphenated, width, &markers, &document);

        assert!(plain_lines.len() > 1);
        assert!(hyphenated_lines.len() > 1);
        for lines in [&plain_lines, &hyphenated_lines] {
            assert!(lines.iter().all(|line| line.width <= width + 0.01));
            assert!(lines.iter().all(|line| !line_text(line).contains('-')));
            assert_eq!(lines.iter().map(line_text).collect::<String>(), word);
        }
        assert_eq!(
            plain_lines.iter().map(line_text).collect::<Vec<_>>(),
            hyphenated_lines.iter().map(line_text).collect::<Vec<_>>()
        );
    }

    #[test]
    fn emergency_wrapping_still_bounds_words_inside_a_wide_hyphenation_zone() {
        let document = Document::default();
        let style = CharacterStyle::default();
        let word = "Antidisestablishment";
        let tight_zone_style = ParagraphStyle {
            auto_hyphenation: true,
            hyphenation_zone_twips: 0,
            ..ParagraphStyle::default()
        };
        let wide_zone_style = ParagraphStyle {
            hyphenation_zone_twips: 1_440,
            ..tight_zone_style.clone()
        };
        let tight_zone = Paragraph {
            style: tight_zone_style.clone(),
            runs: vec![Run {
                dynamic_field: None,
                text: word.to_string(),
                style: style.clone(),
            }],
        };
        let wide_zone = Paragraph {
            style: wide_zone_style,
            runs: tight_zone.runs.clone(),
        };

        let markers = test_markers("1", "1");
        let width = measure_text(word, &style) - 1.0;
        let tight_lines = wrap_paragraph(&tight_zone, width, &markers, &document);
        let wide_lines = wrap_paragraph(&wide_zone, width, &markers, &document);

        assert!(tight_lines.len() > 1);
        assert!(wide_lines.len() > 1);
        for lines in [&tight_lines, &wide_lines] {
            assert!(lines.iter().all(|line| line.width <= width + 0.01));
            assert!(lines.iter().all(|line| !line_text(line).contains('-')));
            assert_eq!(lines.iter().map(line_text).collect::<String>(), word);
        }
    }

    #[test]
    fn capital_word_suppression_does_not_disable_emergency_wrapping() {
        let document = Document::default();
        let style = CharacterStyle::default();
        let word = "ANTIDISESTABLISHMENTARIANISM";
        let mut hyphenated_style = ParagraphStyle::default();
        hyphenated_style.auto_hyphenation = true;
        let mut suppressed_style = hyphenated_style.clone();
        suppressed_style.hyphenate_caps = false;
        let hyphenated = Paragraph {
            style: hyphenated_style,
            runs: vec![Run {
                dynamic_field: None,
                text: word.to_string(),
                style: style.clone(),
            }],
        };
        let suppressed = Paragraph {
            style: suppressed_style,
            runs: hyphenated.runs.clone(),
        };

        let markers = test_markers("1", "1");
        let width = measure_text("ANTIDIS", &style);
        let hyphenated_lines = wrap_paragraph(&hyphenated, width, &markers, &document);
        let suppressed_lines = wrap_paragraph(&suppressed, width, &markers, &document);

        assert!(hyphenated_lines.len() > 1);
        assert!(suppressed_lines.len() > 1);
        for lines in [&hyphenated_lines, &suppressed_lines] {
            assert!(lines.iter().all(|line| line.width <= width + 0.01));
            assert!(lines.iter().all(|line| !line_text(line).contains('-')));
            assert_eq!(lines.iter().map(line_text).collect::<String>(), word);
        }
    }

    #[test]
    fn keeps_nonbreaking_space_segments_together_when_wrapping() {
        let document = Document::default();
        let style = CharacterStyle::default();
        let paragraph = Paragraph {
            style: Default::default(),
            runs: vec![Run {
                dynamic_field: None,
                text: "A\u{00a0}B C".to_string(),
                style: style.clone(),
            }],
        };

        let narrow_width = measure_text("A ", &style);
        let markers = test_markers("1", "1");
        let lines = wrap_paragraph(&paragraph, narrow_width, &markers, &document);

        assert_eq!(lines.len(), 2);
        assert_eq!(line_text(&lines[0]), "A\u{00a0}B ");
        assert_eq!(line_text(&lines[1]), "C");
    }

    #[test]
    fn keeps_nonbreaking_hyphen_segments_together_when_wrapping() {
        let document = Document::default();
        let style = CharacterStyle::default();
        let paragraph = Paragraph {
            style: Default::default(),
            runs: vec![Run {
                dynamic_field: None,
                text: "A\u{2011}B C".to_string(),
                style: style.clone(),
            }],
        };

        let narrow_width = measure_text("A-", &style);
        let markers = test_markers("1", "1");
        let lines = wrap_paragraph(&paragraph, narrow_width, &markers, &document);

        assert_eq!(lines.len(), 2);
        assert_eq!(line_text(&lines[0]), "A\u{2011}B ");
        assert_eq!(line_text(&lines[1]), "C");
    }

    #[test]
    fn no_wrap_paragraph_uses_emergency_character_boundaries() {
        let document = Document::default();
        let style = CharacterStyle::default();
        let mut paragraph = Paragraph {
            style: Default::default(),
            runs: vec![Run {
                dynamic_field: None,
                text: "Alpha Beta".to_string(),
                style: style.clone(),
            }],
        };
        let narrow_width = measure_text("Alpha ", &style);

        let markers = test_markers("1", "1");
        let wrapped = wrap_paragraph(&paragraph, narrow_width, &markers, &document);
        assert_eq!(wrapped.len(), 2);

        paragraph.style.no_wrap = true;
        let emergency_wrapped = wrap_paragraph(&paragraph, narrow_width, &markers, &document);
        assert_eq!(emergency_wrapped.len(), 2);
        assert_eq!(line_text(&emergency_wrapped[0]), "Alpha");
        assert_eq!(line_text(&emergency_wrapped[1]), " Beta");
        assert_eq!(
            emergency_wrapped.iter().map(line_text).collect::<String>(),
            "Alpha Beta"
        );
    }

    #[test]
    fn lays_out_drop_cap_as_enlarged_first_visible_character() {
        let mut document = Document::default();
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.drop_cap_lines = 3;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                dynamic_field: None,
                text: "Dropped paragraph".to_string(),
                style: CharacterStyle::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let fragments = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(layout_text(&layout.pages[0]), "Dropped paragraph");
        assert_eq!(fragments[0].text, "D");
        assert_eq!(fragments[0].style.font_size_half_points, 72);
        assert!(fragments.iter().skip(1).all(|fragment| {
            fragment.style.font_size_half_points == CharacterStyle::default().font_size_half_points
        }));
    }

    #[test]
    fn framed_word_drop_cap_floats_without_enlarging_and_wraps_following_text() {
        let parsed = crate::rtf::parse_rtf(include_str!("../../fixtures/framed-drop-cap-passive.rtf"))
            .expect("framed drop-cap fixture should parse");
        let layout = LayoutEngine::layout(&parsed.document);
        let fragments = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment),
                _ => None,
            })
            .collect::<Vec<_>>();
        let fragment = |needle: &str| {
            fragments
                .iter()
                .copied()
                .find(|fragment| fragment.text.contains(needle))
                .unwrap_or_else(|| panic!("missing text fragment {needle:?}"))
        };

        let first_cap = fragment("D");
        let first_wrapped = fragment("First ");
        let first_full_width = fragment("body ");
        let second_cap = fragment("I");
        let second_wrapped = fragment("Second ");
        assert_eq!(first_cap.style.font_size_half_points, 24);
        assert_eq!(second_cap.style.font_size_half_points, 24);
        assert!((first_cap.x - 36.0).abs() < 0.01);
        assert!((first_wrapped.x - 72.0).abs() < 0.01);
        assert!((first_full_width.x - 36.0).abs() < 0.01);
        assert!((first_cap.baseline_y - first_wrapped.baseline_y).abs() < 0.01);
        assert!((second_cap.baseline_y - second_wrapped.baseline_y).abs() < 0.01);
        assert_eq!(layout.pages[0].flow_exclusions.len(), 2);
    }

    #[test]
    fn framed_word_drop_cap_derives_each_missing_dimension_from_content() {
        let parsed = crate::rtf::parse_rtf(include_str!(
            "../../fixtures/framed-drop-cap-derived-dimensions-passive.rtf"
        ))
        .expect("derived drop-cap fixture should parse");
        let layout = LayoutEngine::layout(&parsed.document);
        let fragments = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment),
                _ => None,
            })
            .collect::<Vec<_>>();
        let containing = |text: &str| {
            fragments
                .iter()
                .copied()
                .find(|fragment| fragment.text.contains(text))
                .unwrap_or_else(|| {
                    panic!(
                        "missing text fragment containing {text:?}: {:?}",
                        fragments
                            .iter()
                            .map(|fragment| fragment.text.as_str())
                            .collect::<Vec<_>>()
                    )
                })
        };

        let width_cap = fragments
            .iter()
            .copied()
            .find(|fragment| fragment.text == "W")
            .expect("width cap");
        let width_first_line = containing("Width-");
        let width_second_line = containing("continues ");
        let height_cap = fragments
            .iter()
            .copied()
            .find(|fragment| fragment.text == "H")
            .expect("height cap");
        let height_first_line = containing("Height-");
        let height_fourth_line = containing("and ");
        assert!((width_cap.x - 36.0).abs() < 0.01);
        assert!((width_first_line.x - 72.0).abs() < 0.01);
        assert!((width_second_line.x - 36.0).abs() < 0.01);
        assert!((height_cap.x - 36.0).abs() < 0.01);
        assert!((height_first_line.x - 44.666).abs() < 0.01);
        assert!((height_fourth_line.x - 36.0).abs() < 0.01);
        assert!((height_cap.baseline_y - height_first_line.baseline_y).abs() < 0.01);
        assert_eq!(layout.pages[0].flow_exclusions.len(), 2);
    }

    #[test]
    fn signed_frame_width_controls_float_or_in_flow_recovery() {
        let parsed = crate::rtf::parse_rtf(include_str!(
            "../../fixtures/framed-drop-cap-signed-dimensions-passive.rtf"
        ))
        .expect("signed frame-dimension fixture should parse");
        let provider = FontProvider::browser_safe_defaults();
        let layout = LayoutEngine::layout_with_font_provider(&parsed.document, Some(&provider));
        assert_eq!(layout.pages.len(), 8);
        let fragments = |page_index: usize| {
            layout.pages[page_index]
                .items
                .iter()
                .filter_map(layout_item_text_fragment)
                .collect::<Vec<_>>()
        };
        let exact = |page_index: usize, text: &str| {
            fragments(page_index)
                .into_iter()
                .find(|fragment| fragment.text == text)
                .unwrap_or_else(|| panic!("missing page {} fragment {text:?}", page_index + 1))
        };

        for (page_index, cap, body) in [
            (0, "P", "Positive "),
            (2, "H", "Positive "),
            (6, "L", "Last "),
        ] {
            let cap = exact(page_index, cap);
            let body = exact(page_index, body);
            assert!((cap.x - 36.0).abs() < 0.01);
            assert!((body.x - 72.0).abs() < 0.01);
            assert!((cap.baseline_y - body.baseline_y).abs() < 0.01);
        }
        for (page_index, cap, body, expected_advance) in [
            (1, "W", "Negative ", 36.0),
            (3, "B", "Negative ", 36.0),
            (5, "T", "Negative ", 72.0),
            (7, "N", "Last ", 36.0),
        ] {
            let cap = exact(page_index, cap);
            let body = exact(page_index, body);
            assert!((cap.x - 36.0).abs() < 0.01);
            assert!((body.x - 36.0).abs() < 0.01);
            assert!(((cap.baseline_y - body.baseline_y) - expected_advance).abs() < 0.01);
        }
        assert!(fragments(4).iter().all(|fragment| fragment.text != "O"));
        assert_eq!(
            layout
                .pages
                .iter()
                .map(|page| page.flow_exclusions.len())
                .collect::<Vec<_>>(),
            vec![1, 0, 1, 0, 0, 0, 1, 0]
        );
    }

    #[test]
    fn colliding_absolute_frames_stack_by_natural_line_advance() {
        let parsed = crate::rtf::parse_rtf(include_str!(
            "../../fixtures/framed-drop-cap-collision-controls-passive.rtf"
        ))
        .expect("frame collision-control fixture should parse");
        let provider = FontProvider::browser_safe_defaults();
        let layout = LayoutEngine::layout_with_font_provider(&parsed.document, Some(&provider));
        assert_eq!(layout.pages.len(), 3);
        for (page_index, expected_caps) in
            [["A", "B"], ["C", "D"], ["E", "F"]].into_iter().enumerate()
        {
            let caps = layout.pages[page_index]
                .items
                .iter()
                .filter_map(|item| match item {
                    LayoutItem::Text(fragment)
                        if expected_caps.contains(&fragment.text.as_str()) =>
                    {
                        Some(fragment)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(caps.len(), 2);
            assert_eq!(caps[0].text, expected_caps[0]);
            assert_eq!(caps[1].text, expected_caps[1]);
            assert!((caps[0].x - 36.0).abs() < 0.01);
            assert!((caps[1].x - 36.0).abs() < 0.01);
            assert!(((caps[0].baseline_y - caps[1].baseline_y) - 13.7988).abs() < 0.01);
            let exclusions = &layout.pages[page_index].flow_exclusions;
            assert_eq!(exclusions.len(), 2);
            let first_top = exclusions[0].y + exclusions[0].height;
            let second_top = exclusions[1].y + exclusions[1].height;
            assert!(((first_top - second_top) - 13.7988).abs() < 0.01);
            assert!(
                exclusions
                    .iter()
                    .all(|exclusion| exclusion.kind == FlowExclusionKind::ParagraphFrame)
            );
        }
    }

    #[test]
    fn unequal_absolute_frames_keep_authored_origins_through_intersections() {
        let parsed = crate::rtf::parse_rtf(include_str!(
            "../../fixtures/framed-drop-cap-overlap-geometry-passive.rtf"
        ))
        .expect("frame overlap-geometry fixture should parse");
        let provider = FontProvider::browser_safe_defaults();
        let layout = LayoutEngine::layout_with_font_provider(&parsed.document, Some(&provider));
        assert_eq!(layout.pages.len(), 5);

        let cap = |page_index: usize, text: &str| {
            layout.pages[page_index]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text == text => Some(fragment),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing page {} cap {text:?}", page_index + 1))
        };

        for (page_index, first, second) in [(0, "A", "B"), (4, "I", "J")] {
            let first = cap(page_index, first);
            let second = cap(page_index, second);
            assert!((first.x - 36.0).abs() < 0.01);
            assert!((second.x - 36.0).abs() < 0.01);
            assert!((first.baseline_y - second.baseline_y).abs() < 0.01);
        }

        assert!((cap(1, "C").x - 36.0).abs() < 0.01);
        assert!((cap(1, "D").x - 72.0).abs() < 0.01);
        assert!((cap(1, "C").baseline_y - cap(1, "D").baseline_y).abs() < 0.01);
        assert!((cap(2, "E").x - 36.0).abs() < 0.01);
        assert!((cap(2, "F").x - 36.0).abs() < 0.01);
        assert!(((cap(2, "E").baseline_y - cap(2, "F").baseline_y) - 18.0).abs() < 0.01);
        assert!((cap(3, "G").x - 36.0).abs() < 0.01);
        assert!((cap(3, "H").x - 90.0).abs() < 0.01);

        assert_eq!(
            layout
                .pages
                .iter()
                .map(|page| page.flow_exclusions.len())
                .collect::<Vec<_>>(),
            vec![2, 2, 2, 2, 2]
        );
    }

    #[test]
    fn ordinary_absolute_paragraph_frames_group_and_reflow_body_text() {
        let parsed = crate::rtf::parse_rtf(include_str!("../../fixtures/paragraph-frame-passive.rtf"))
            .expect("ordinary paragraph-frame fixture should parse");
        let provider = FontProvider::browser_safe_defaults();
        let layout = LayoutEngine::layout_with_font_provider(&parsed.document, Some(&provider));
        assert_eq!(layout.pages.len(), 3);

        let containing = |page_index: usize, text: &str| {
            layout.pages[page_index]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.contains(text) => Some(fragment),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing page {} text {text:?}", page_index + 1))
        };

        assert!((containing(0, "Framed ").x - 36.0).abs() < 0.01);
        assert!((containing(0, "After ").x - 108.0).abs() < 0.01);
        assert!((containing(1, "Group ").x - 36.0).abs() < 0.01);
        assert!(containing(1, "first ").baseline_y > containing(1, "second ").baseline_y);
        assert!((containing(1, "After ").x - 126.0).abs() < 0.01);
        assert!((containing(2, "Derived ").x - 72.0).abs() < 0.01);
        assert!((containing(2, "After ").x - 144.0).abs() < 0.01);

        let exclusions = layout
            .pages
            .iter()
            .map(|page| &page.flow_exclusions)
            .collect::<Vec<_>>();
        assert!(exclusions.iter().all(|page| page.len() == 1));
        assert!(exclusions.iter().all(|page| {
            page[0].kind == FlowExclusionKind::ParagraphFrame
                && page[0].wrap_side == StaticImageWrapSide::Right
        }));
        assert!((exclusions[0][0].height - 72.0).abs() < 0.01);
        assert!((exclusions[1][0].height - 90.0).abs() < 0.01);
        assert!((exclusions[2][0].height - 55.195).abs() < 0.01);
    }

    #[test]
    fn ordinary_frame_height_sign_and_paragraph_anchor_match_word_flow() {
        let parsed = crate::rtf::parse_rtf(include_str!(
            "../../fixtures/paragraph-frame-height-flow-passive.rtf"
        ))
        .expect("paragraph-frame height/flow fixture should parse");
        let provider = FontProvider::browser_safe_defaults();
        let layout = LayoutEngine::layout_with_font_provider(&parsed.document, Some(&provider));
        assert_eq!(layout.pages.len(), 6);

        let fragments = |page_index: usize| {
            layout.pages[page_index]
                .items
                .iter()
                .filter_map(layout_item_text_fragment)
                .collect::<Vec<_>>()
        };
        let containing = |page_index: usize, text: &str| {
            fragments(page_index)
                .into_iter()
                .find(|fragment| fragment.text.contains(text))
                .unwrap_or_else(|| panic!("missing page {} text {text:?}", page_index + 1))
        };

        assert!(
            fragments(0)
                .iter()
                .any(|fragment| fragment.text.contains("zeta."))
        );
        let clips = layout.pages[1]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::ClipRect {
                    x,
                    y,
                    width,
                    height,
                    item,
                } => Some((*x, *y, *width, *height, item.as_ref())),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(!clips.is_empty());
        assert!(clips.iter().all(|(x, y, width, height, _)| {
            (*x - 36.0).abs() < 0.01
                && (*y - 414.0).abs() < 0.01
                && (*width - 72.0).abs() < 0.01
                && (*height - 18.0).abs() < 0.01
        }));
        assert!(clips.iter().any(|(_, _, _, _, item)| {
            layout_item_text_fragment(item).is_some_and(|fragment| fragment.text == "Negative ")
        }));

        for page_index in 2..=4 {
            assert!((containing(page_index, "After ").x - 108.0).abs() < 0.01);
            assert_eq!(layout.pages[page_index].flow_exclusions.len(), 1);
            assert_eq!(
                layout.pages[page_index].flow_exclusions[0].wrap_side,
                StaticImageWrapSide::Right
            );
        }
        assert_eq!(
            layout
                .pages
                .iter()
                .map(|page| page.flow_exclusions.len())
                .collect::<Vec<_>>(),
            vec![1, 1, 1, 1, 1, 0]
        );
        assert_eq!(containing(5, "N").text, "N");
        assert!(
            fragments(5)
                .iter()
                .all(|fragment| !fragment.text.contains("Negative width ordinary alpha"))
        );
        assert!(
            ((containing(5, "N").baseline_y - containing(5, "After ").baseline_y) - 36.0).abs()
                < 0.01
        );
    }

    #[test]
    fn ordinary_frame_text_direction_rotates_and_wraps_by_vertical_extent() {
        let parsed = crate::rtf::parse_rtf(include_str!(
            "../../fixtures/paragraph-frame-text-direction-passive.rtf"
        ))
        .expect("paragraph-frame text-direction fixture should parse");
        let provider = FontProvider::browser_safe_defaults();
        let layout = LayoutEngine::layout_with_font_provider(&parsed.document, Some(&provider));
        assert_eq!(layout.pages.len(), 6);

        let rotated = |page_index: usize| {
            layout.pages[page_index]
                .items
                .iter()
                .filter_map(layout_item_text_fragment)
                .filter(|fragment| fragment.rotation != TextRotation::None)
                .collect::<Vec<_>>()
        };
        assert!(rotated(0).is_empty());
        assert!(rotated(1).is_empty());
        assert!(rotated(4).is_empty());

        let column_origins = |fragments: &[&TextFragment]| {
            let mut origins = fragments
                .iter()
                .map(|fragment| fragment.x)
                .collect::<Vec<_>>();
            origins.sort_by(f32::total_cmp);
            origins.dedup_by(|left, right| (*left - *right).abs() < 0.01);
            origins
        };

        let clockwise = rotated(2);
        assert!(
            clockwise
                .iter()
                .all(|fragment| fragment.rotation == TextRotation::Clockwise90)
        );
        let clockwise_columns = column_origins(&clockwise);
        assert_eq!(clockwise_columns.len(), 2);
        assert!(((clockwise_columns[1] - clockwise_columns[0]) - 14.16).abs() < 0.01);

        let counterclockwise = rotated(3);
        assert!(
            counterclockwise
                .iter()
                .all(|fragment| fragment.rotation == TextRotation::CounterClockwise90)
        );
        let counterclockwise_columns = column_origins(&counterclockwise);
        assert_eq!(counterclockwise_columns.len(), 2);
        assert!(((counterclockwise_columns[1] - counterclockwise_columns[0]) - 14.16).abs() < 0.01);

        let vertical_alias = rotated(5);
        assert!(
            vertical_alias
                .iter()
                .all(|fragment| fragment.rotation == TextRotation::Clockwise90)
        );
        assert_eq!(column_origins(&vertical_alias).len(), 2);
        assert!(
            layout
                .pages
                .iter()
                .all(|page| page.flow_exclusions.len() == 1)
        );
    }

    #[test]
    fn ordinary_frame_text_distance_expands_independent_exclusion_axes() {
        let parsed = crate::rtf::parse_rtf(include_str!(
            "../../fixtures/paragraph-frame-text-distance-passive.rtf"
        ))
        .expect("paragraph-frame text-distance fixture should parse");
        let provider = FontProvider::browser_safe_defaults();
        let layout = LayoutEngine::layout_with_font_provider(&parsed.document, Some(&provider));
        assert_eq!(layout.pages.len(), 6);

        let before_x = |page_index: usize| {
            layout.pages[page_index]
                .items
                .iter()
                .filter_map(layout_item_text_fragment)
                .find(|fragment| fragment.text.starts_with("Before"))
                .expect("before-frame text fragment")
                .x
        };
        let expected_before_x = [36.0, 117.0, 36.0, 108.0, 120.0, 36.0];
        for (page_index, expected) in expected_before_x.into_iter().enumerate() {
            assert!((before_x(page_index) - expected).abs() < 0.01);
        }

        let expected_widths = [72.0, 81.0, 84.0, 72.0, 84.0, 72.0];
        let expected_heights = [72.0, 90.0, 72.0, 108.0, 108.0, 72.0];
        for (page_index, page) in layout.pages.iter().enumerate() {
            assert_eq!(page.flow_exclusions.len(), 1);
            let exclusion = page.flow_exclusions[0];
            assert!((exclusion.width - expected_widths[page_index]).abs() < 0.01);
            assert!((exclusion.height - expected_heights[page_index]).abs() < 0.01);
        }
    }

    #[test]
    fn ordinary_frame_vertical_alignment_uses_page_and_margin_anchor_bounds() {
        let parsed = crate::rtf::parse_rtf(include_str!(
            "../../fixtures/paragraph-frame-vertical-alignment-passive.rtf"
        ))
        .expect("paragraph-frame vertical-alignment fixture should parse");
        let provider = FontProvider::browser_safe_defaults();
        let layout = LayoutEngine::layout_with_font_provider(&parsed.document, Some(&provider));
        assert_eq!(layout.pages.len(), 7);

        let expected_exclusion_tops = [504.0, 288.0, 72.0, 468.0, 288.0, 108.0, 426.402];
        for (page_index, expected_top) in expected_exclusion_tops.into_iter().enumerate() {
            let exclusions = &layout.pages[page_index].flow_exclusions;
            assert_eq!(exclusions.len(), 1);
            let top = exclusions[0].y + exclusions[0].height;
            assert!(
                (top - expected_top).abs() < 0.02,
                "page {} exclusion top {top} != {expected_top}",
                page_index + 1
            );
        }

        let frame_fragment = |page_index: usize| {
            layout.pages[page_index]
                .items
                .iter()
                .filter_map(layout_item_text_fragment)
                .find(|fragment| fragment.text.contains("frame."))
                .unwrap_or_else(|| panic!("missing page {} frame text", page_index + 1))
        };
        assert!(frame_fragment(0).baseline_y > frame_fragment(1).baseline_y);
        assert!(frame_fragment(1).baseline_y > frame_fragment(2).baseline_y);
        assert!(frame_fragment(3).baseline_y > frame_fragment(4).baseline_y);
        assert!(frame_fragment(4).baseline_y > frame_fragment(5).baseline_y);
    }

    #[test]
    fn ordinary_frame_vertical_precedence_and_facing_edges_match_word() {
        let parsed = crate::rtf::parse_rtf(include_str!(
            "../../fixtures/paragraph-frame-vertical-precedence-passive.rtf"
        ))
        .expect("paragraph-frame vertical-precedence fixture should parse");
        let provider = FontProvider::browser_safe_defaults();
        let layout = LayoutEngine::layout_with_font_provider(&parsed.document, Some(&provider));
        assert_eq!(layout.pages.len(), 10);

        let expected_exclusion_tops = [
            432.0, 288.0, 432.0, 108.0, 486.0, 90.0, 90.0, 486.0, 504.0, 440.201,
        ];
        for (page_index, expected_top) in expected_exclusion_tops.into_iter().enumerate() {
            let exclusions = &layout.pages[page_index].flow_exclusions;
            assert_eq!(exclusions.len(), 1);
            let top = exclusions[0].y + exclusions[0].height;
            assert!(
                (top - expected_top).abs() < 0.02,
                "page {} exclusion top {top} != {expected_top}",
                page_index + 1
            );
        }

        let frame_baseline = |page_index: usize| {
            layout.pages[page_index]
                .items
                .iter()
                .filter_map(layout_item_text_fragment)
                .find(|fragment| fragment.text.contains("frame."))
                .expect("frame text")
                .baseline_y
        };
        assert!(frame_baseline(4) > frame_baseline(5));
        assert!((frame_baseline(5) - frame_baseline(6)).abs() < 0.01);
        assert!(frame_baseline(7) > frame_baseline(6));
        assert!(frame_baseline(8) > frame_baseline(9));
    }

    #[test]
    fn framed_word_drop_cap_honors_horizontal_text_distance_only() {
        let parsed = crate::rtf::parse_rtf(include_str!(
            "../../fixtures/framed-drop-cap-wrap-distance-passive.rtf"
        ))
        .expect("framed drop-cap distance fixture should parse");
        let layout = LayoutEngine::layout(&parsed.document);
        assert_eq!(layout.pages.len(), 1);
        let fragments = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment),
                _ => None,
            })
            .collect::<Vec<_>>();
        let containing = |text: &str| {
            fragments
                .iter()
                .copied()
                .find(|fragment| fragment.text.contains(text))
                .unwrap_or_else(|| panic!("missing text fragment {text:?}"))
        };

        assert!((containing("No ").x - 72.0).abs() < 0.01);
        assert!((containing("All ").x - 81.0).abs() < 0.01);
        assert!((containing("X ").x - 84.0).abs() < 0.01);
        assert!((containing("Y ").x - 72.0).abs() < 0.01);
        let exclusions = &layout.pages[0].flow_exclusions;
        assert_eq!(exclusions.len(), 4);
        for (exclusion, expected_width) in exclusions.iter().zip([36.0, 45.0, 48.0, 36.0]) {
            assert!((exclusion.width - expected_width).abs() < 0.01);
            assert!((exclusion.height - 36.0).abs() < 0.01);
        }
    }

    #[test]
    fn framed_word_drop_cap_honors_bounded_positioning_and_segmented_flow() {
        let parsed = crate::rtf::parse_rtf(include_str!(
            "../../fixtures/framed-drop-cap-positioning-passive.rtf"
        ))
        .expect("framed drop-cap positioning fixture should parse");
        let layout = LayoutEngine::layout(&parsed.document);
        assert_eq!(layout.pages.len(), 1);
        let fragments = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment),
                _ => None,
            })
            .collect::<Vec<_>>();
        let containing = |text: &str| {
            fragments
                .iter()
                .copied()
                .find(|fragment| fragment.text.contains(text))
                .unwrap_or_else(|| panic!("missing text fragment {text:?}"))
        };

        let margin_cap = containing("M");
        let margin_body = containing("Margin ");
        assert!((margin_cap.x - 72.0).abs() < 0.01);
        assert!((margin_body.x - 108.0).abs() < 0.01);
        assert!((margin_cap.baseline_y - margin_body.baseline_y).abs() < 0.01);
        assert!(fragments.iter().all(|fragment| fragment.text != "Margi"));

        assert!((containing("P").x - 36.0).abs() < 0.01);
        assert!((containing("Page ").x - 72.0).abs() < 0.01);
        assert!((containing("C").x - 135.0).abs() < 0.01);
        assert!((containing("Centered ").x - 36.0).abs() < 0.01);

        let vertical_cap = containing("V");
        let vertical_body = containing("Vertical ");
        assert!(((vertical_body.baseline_y - vertical_cap.baseline_y) - 18.0).abs() < 0.01);
        assert!((containing("R").x - 234.0).abs() < 0.01);
        assert!((containing("Right ").x - 72.0).abs() < 0.01);

        let exclusions = &layout.pages[0].flow_exclusions;
        assert_eq!(exclusions.len(), 6);
        for (exclusion, expected_x) in exclusions
            .iter()
            .zip([36.0, 72.0, 36.0, 135.0, 36.0, 234.0])
        {
            assert!((exclusion.x - expected_x).abs() < 0.01);
            assert!((exclusion.width - 36.0).abs() < 0.01);
            assert!((exclusion.height - 36.0).abs() < 0.01);
            assert_eq!(exclusion.wrap_side, StaticImageWrapSide::Both);
        }
    }

    #[test]
    fn framed_word_drop_cap_vertical_alignments_remain_inert() {
        let parsed = crate::rtf::parse_rtf(include_str!(
            "../../fixtures/framed-drop-cap-vertical-alignment-passive.rtf"
        ))
        .expect("framed drop-cap vertical-alignment fixture should parse");
        let layout = LayoutEngine::layout(&parsed.document);
        assert_eq!(layout.pages.len(), 1);
        let fragments = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment),
                _ => None,
            })
            .collect::<Vec<_>>();
        let containing = |text: &str| {
            fragments
                .iter()
                .copied()
                .find(|fragment| fragment.text.contains(text))
                .unwrap_or_else(|| panic!("missing text fragment {text:?}"))
        };

        let caps = fragments
            .iter()
            .copied()
            .filter(|fragment| fragment.text == "X")
            .collect::<Vec<_>>();
        assert_eq!(caps.len(), 4);
        for ((cap, body), expected_rise) in caps
            .into_iter()
            .zip(["Zero ", "Tops ", "Cent ", "Bott "])
            .zip([0.0, 0.24, 0.36, 0.60])
        {
            let body = containing(body);
            assert!((cap.x - 36.0).abs() < 0.01);
            assert!((body.x - 72.0).abs() < 0.01);
            assert!((cap.baseline_y - body.baseline_y - expected_rise).abs() < 0.01);
        }
        assert_eq!(layout.pages[0].flow_exclusions.len(), 4);
    }

    #[test]
    fn absolute_drop_cap_frames_preview_bounded_reflow_on_preceding_paragraphs() {
        let parsed = crate::rtf::parse_rtf(include_str!(
            "../../fixtures/framed-drop-cap-absolute-overlap-passive.rtf"
        ))
        .expect("absolute overlap fixture should parse");
        let layout = LayoutEngine::layout(&parsed.document);
        assert_eq!(layout.pages.len(), 2);
        let page_fragments = |page_index: usize| {
            layout.pages[page_index]
                .items
                .iter()
                .filter_map(|item| match item {
                    LayoutItem::Text(fragment) => Some(fragment),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };

        let first_page = page_fragments(0);
        let first_exact = |text: &str| {
            first_page
                .iter()
                .copied()
                .find(|fragment| fragment.text == text)
                .unwrap_or_else(|| panic!("missing first-page fragment {text:?}"))
        };
        assert!((first_exact("M").x - 72.0).abs() < 0.01);
        assert!((first_exact("xi ").x - 108.0).abs() < 0.01);
        assert!((first_exact("theta.").x - 36.0).abs() < 0.01);

        let second_page = page_fragments(1);
        let page_starts = second_page
            .iter()
            .copied()
            .filter(|fragment| fragment.text == "Page ")
            .collect::<Vec<_>>();
        assert_eq!(page_starts.len(), 3);
        assert!((page_starts[0].x - 72.0).abs() < 0.01);
        assert!((page_starts[1].x - 72.0).abs() < 0.01);
        assert!((page_starts[2].x - 36.0).abs() < 0.01);
        let page_cap = second_page
            .iter()
            .copied()
            .find(|fragment| fragment.text == "P")
            .expect("page cap");
        assert!((page_cap.x - 36.0).abs() < 0.01);
        assert!((page_cap.baseline_y - page_starts[0].baseline_y).abs() < 0.01);

        for page in &layout.pages {
            assert_eq!(page.flow_exclusions.len(), 1);
            assert!((page.flow_exclusions[0].wrap_bottom_inset - 1.0).abs() < 0.01);
        }
    }

    #[test]
    fn absolute_drop_cap_frame_preview_tracks_natural_page_association() {
        let parsed = crate::rtf::parse_rtf(include_str!(
            "../../fixtures/framed-drop-cap-natural-pagination-passive.rtf"
        ))
        .expect("natural-pagination frame fixture should parse");
        let provider = FontProvider::browser_safe_defaults();
        let layout = LayoutEngine::layout_with_font_provider(&parsed.document, Some(&provider));
        assert_eq!(layout.pages.len(), 2);
        let page_text = |page_index: usize| layout_text(&layout.pages[page_index]);
        assert!(page_text(0).contains("A75 "));
        assert!(!page_text(0).contains("A76 "));
        assert!(!page_text(0).contains('X'));
        assert!(page_text(1).contains("A76 A77 A78 A79 A80."));
        assert!(page_text(1).contains("After natural pagination."));

        let second_page_fragments = layout.pages[1]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment),
                _ => None,
            })
            .collect::<Vec<_>>();
        let exact = |text: &str| {
            second_page_fragments
                .iter()
                .copied()
                .find(|fragment| fragment.text == text)
                .unwrap_or_else(|| panic!("missing second-page fragment {text:?}"))
        };
        let cap = exact("X");
        let tail = exact("A76 ");
        assert!((cap.x - 36.0).abs() < 0.01);
        assert!((tail.x - 72.0).abs() < 0.01);
        assert!((cap.baseline_y - tail.baseline_y).abs() < 0.01);
        assert!(layout.pages[0].flow_exclusions.is_empty());
        assert_eq!(layout.pages[1].flow_exclusions.len(), 1);
    }

    #[test]
    fn facing_drop_cap_frames_resolve_inside_outside_by_physical_page() {
        let parsed = crate::rtf::parse_rtf(include_str!(
            "../../fixtures/framed-drop-cap-facing-alignment-passive.rtf"
        ))
        .expect("facing frame fixture should parse");
        let provider = FontProvider::browser_safe_defaults();
        let layout = LayoutEngine::layout_with_font_provider(&parsed.document, Some(&provider));
        assert_eq!(layout.pages.len(), 2);
        let caps = |page_index: usize| {
            layout.pages[page_index]
                .items
                .iter()
                .filter_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text == "I" || fragment.text == "O" => {
                        Some(fragment)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let odd_caps = caps(0);
        let even_caps = caps(1);
        assert_eq!(odd_caps.len(), 2);
        assert_eq!(even_caps.len(), 2);
        assert!((odd_caps[0].x - 36.0).abs() < 0.01);
        assert!((odd_caps[1].x - 216.0).abs() < 0.01);
        assert!((even_caps[0].x - 216.0).abs() < 0.01);
        assert!((even_caps[1].x - 36.0).abs() < 0.01);
    }

    #[test]
    fn signed_drop_cap_frame_offsets_preserve_word_coordinates() {
        let parsed = crate::rtf::parse_rtf(include_str!(
            "../../fixtures/framed-drop-cap-negative-positioning-passive.rtf"
        ))
        .expect("signed frame-position fixture should parse");
        let provider = FontProvider::browser_safe_defaults();
        let layout = LayoutEngine::layout_with_font_provider(&parsed.document, Some(&provider));
        assert_eq!(layout.pages.len(), 4);
        for (page_index, expected) in [
            vec![("A", 54.0), ("B", 54.0), ("C", 72.0)],
            vec![("D", 36.0), ("E", 108.0)],
            vec![("F", 18.0)],
            vec![("G", 18.0), ("H", 108.0)],
        ]
        .into_iter()
        .enumerate()
        {
            let fragments = layout.pages[page_index]
                .items
                .iter()
                .filter_map(|item| match item {
                    LayoutItem::Text(fragment) => Some(fragment),
                    _ => None,
                })
                .collect::<Vec<_>>();
            for (text, expected_x) in expected {
                let fragment = fragments
                    .iter()
                    .find(|fragment| fragment.text == text)
                    .unwrap_or_else(|| panic!("missing frame text {text:?}"));
                assert!((fragment.x - expected_x).abs() < 0.01);
            }
        }
        let page_three_fragments = layout.pages[2]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment),
                _ => None,
            })
            .collect::<Vec<_>>();
        let page_body = page_three_fragments
            .iter()
            .find(|fragment| fragment.text == "Page ")
            .expect("page-anchored frame body");
        assert!((page_body.x - 54.0).abs() < 0.01);
        let page_two_theta = layout.pages[1]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "theta." => Some(fragment),
                _ => None,
            })
            .next()
            .expect("first page-two theta fragment");
        assert!((page_two_theta.x - 36.0).abs() < 0.01);
        assert!(
            layout
                .pages
                .iter()
                .flat_map(|page| &page.flow_exclusions)
                .all(|exclusion| (exclusion.wrap_bottom_inset - 1.0).abs() < 0.01)
        );
    }

    #[test]
    fn applies_exact_line_spacing_to_wrapped_lines() {
        let mut document = Document::default();
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.line_spacing_twips = Some(-480);
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                dynamic_field: None,
                text: "First\nSecond".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let baselines = text_baselines(&layout.pages[0]);

        assert_eq!(baselines.len(), 2);
        assert!((baselines[0] - baselines[1] - 24.0).abs() < 0.01);
    }

    #[test]
    fn applies_multiple_line_spacing_to_wrapped_lines() {
        let mut document = Document::default();
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.line_spacing_twips = Some(480);
        paragraph_style.line_spacing_multiple = true;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                dynamic_field: None,
                text: "First\nSecond".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let baselines = text_baselines(&layout.pages[0]);

        assert_eq!(baselines.len(), 2);
        assert!((baselines[0] - baselines[1] - 30.0).abs() < 0.01);
    }

    #[test]
    fn repeats_header_shapes_on_each_page() {
        let mut document = Document::default();
        document.header_shapes = vec![StaticShape {
            kind: StaticShapeKind::Rectangle,
            left_twips: 0,
            top_twips: 0,
            width_twips: 720,
            height_twips: 360,
            z_order: 0,
            below_text: false,
            text_wrap: false,
            wrap_side: StaticImageWrapSide::Both,
            wrap_margin_left_twips: 0,
            wrap_margin_right_twips: 0,
            wrap_margin_top_twips: 0,
            wrap_margin_bottom_twips: 0,
            horizontal_anchor: StaticShapeHorizontalAnchor::Column,
            vertical_anchor: StaticShapeVerticalAnchor::Paragraph,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            start_arrowhead_width_percent: 100,
            start_arrowhead_length_percent: 100,
            end_arrowhead_width_percent: 100,
            end_arrowhead_length_percent: 100,
            stroke_width_twips: 30,
            stroke_color: Color {
                red: 255,
                green: 0,
                blue: 0,
            },
            stroke_style: BorderStyle::Single,
            stroke_cap: StaticShapeLineCap::Flat,
            stroke_join: StaticShapeLineJoin::Miter,
            fill_opacity_percent: 100,
            stroke_opacity_percent: 100,
            fill_color: Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            }),
            fill_gradient_color: None,
            fill_pattern: crate::model::ShadingPattern::None,
            shadow_enabled: false,
            shadow_color: Color {
                red: 128,
                green: 128,
                blue: 128,
            },
            shadow_opacity_percent: 100,
            shadow_offset_x_twips: 60,
            shadow_offset_y_twips: 60,
            text_margin_left_twips: 80,
            text_margin_right_twips: 80,
            text_margin_top_twips: 80,
            text_margin_bottom_twips: 80,
            text_vertical_anchor: StaticShapeTextVerticalAnchor::Top,
            text_horizontal_anchor_centered: false,
            text: Vec::new(),
            points: Vec::new(),
            point_paths: Vec::new(),
            overlay_paths: Vec::new(),
            fill_rule: StaticImageVectorFillRule::Winding,
        }];
        document.blocks.clear();
        for _ in 0..120 {
            document.blocks.push(Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    dynamic_field: None,
                    text: "Body paragraph.".to_string(),
                    style: Default::default(),
                }],
            }));
        }

        let layout = LayoutEngine::layout(&document);
        assert!(layout.pages.len() > 1);
        for page in &layout.pages {
            assert!(
                page.items
                    .iter()
                    .any(|item| matches!(item, LayoutItem::Highlight { .. })),
                "expected repeated header shape fill on every page"
            );
            assert!(
                page.items
                    .iter()
                    .any(|item| matches!(item, LayoutItem::Line { .. })),
                "expected repeated header shape outline on every page"
            );
        }
    }

    #[test]
    fn late_total_page_marker_uses_bounded_width_for_wrapping() {
        let document = Document::default();
        let style = CharacterStyle::default();
        let paragraph = Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    dynamic_field: None,
                    text: "A ".to_string(),
                    style: style.clone(),
                },
                Run {
                    dynamic_field: Some(InlineKind::DynamicField(DynamicField::TotalPages)),
                    text: String::new(),
                    style: style.clone(),
                },
                Run {
                    dynamic_field: None,
                    text: " tail".to_string(),
                    style: style.clone(),
                },
            ],
        };
        let placeholder_width = measure_text_with_family(
            LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER,
            &style,
            PdfFontFamily::Helvetica,
        );
        let prefix_width = measure_text_with_family("A ", &style, PdfFontFamily::Helvetica);
        let line_width = prefix_width + placeholder_width - 0.5;
        assert!(
            line_width < prefix_width + placeholder_width,
            "fixture should force the typed field onto the next line"
        );

        let lines = wrap_paragraph(&paragraph, line_width, &test_markers("1", "1"), &document);

        assert_eq!(line_text(&lines[0]), "A ");
        assert!(
            line_text(&lines[1]).starts_with(LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER),
            "late NUMPAGES field should wrap using bounded placeholder width"
        );
    }

    #[test]
    fn late_section_page_marker_uses_bounded_width_for_wrapping() {
        let document = Document::default();
        let style = CharacterStyle::default();
        let paragraph = Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    dynamic_field: None,
                    text: "A ".to_string(),
                    style: style.clone(),
                },
                Run {
                    dynamic_field: Some(InlineKind::DynamicField(DynamicField::SectionPages)),
                    text: String::new(),
                    style: style.clone(),
                },
                Run {
                    dynamic_field: None,
                    text: " tail".to_string(),
                    style: style.clone(),
                },
            ],
        };
        let placeholder_width = measure_text_with_family(
            LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER,
            &style,
            PdfFontFamily::Helvetica,
        );
        let prefix_width = measure_text_with_family("A ", &style, PdfFontFamily::Helvetica);
        let line_width = prefix_width + placeholder_width - 0.5;
        assert!(
            line_width < prefix_width + placeholder_width,
            "fixture should force the typed field onto the next line"
        );

        let lines = wrap_paragraph(&paragraph, line_width, &test_markers("1", "1"), &document);

        assert_eq!(line_text(&lines[0]), "A ");
        assert!(
            line_text(&lines[1]).starts_with(LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER),
            "late SECTIONPAGES field should wrap using bounded placeholder width"
        );
    }
    fn passive_dynamic_field_item(
        field: DynamicField,
        x: f32,
        baseline_y: f32,
    ) -> LayoutItem {
        let mut item = passive_text_item(LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER, x, baseline_y);
        let LayoutItem::Text(fragment) = &mut item else {
            unreachable!("passive text helper must create a text fragment");
        };
        fragment.dynamic_field = Some(InlineKind::DynamicField(field));
        item
    }
