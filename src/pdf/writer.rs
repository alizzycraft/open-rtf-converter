//! Catalog, page, resource, stream, and final PDF byte serialization.

use super::*;

pub fn render_pdf(layout: &LayoutDocument) -> Vec<u8> {
    render_pdf_with_font_provider(layout, None)
}

pub fn estimate_passive_pdf_object_count(
    layout: &LayoutDocument,
    font_provider: Option<&FontProvider>,
) -> usize {
    let page_used_font_indexes = collect_page_used_font_indexes(layout, font_provider);
    let used_font_indexes = collect_used_font_indexes(&page_used_font_indexes);
    let extended_latin_usage = collect_extended_latin_font_usage(layout, font_provider);
    2usize
        .saturating_add(layout.pages.len().saturating_mul(2))
        .saturating_add(used_font_indexes.len())
        .saturating_add(usize::from(
            font_index_for_resource(SYMBOL_REGULAR)
                .is_some_and(|index| used_font_indexes.contains(&index)),
        ))
        .saturating_add(usize::from(
            font_index_for_resource(ZAPF_DINGBATS_REGULAR)
                .is_some_and(|index| used_font_indexes.contains(&index)),
        ))
        .saturating_add(
            extended_latin_usage
                .iter()
                .filter(|usage| !usage.is_empty())
                .count(),
        )
        .saturating_add(
            collect_used_supplied_font_asset_indexes(layout, font_provider)
                .len()
                .saturating_mul(6),
        )
        .saturating_add(count_image_xobjects(layout))
}

pub fn render_pdf_with_font_provider(
    layout: &LayoutDocument,
    font_provider: Option<&FontProvider>,
) -> Vec<u8> {
    try_render_pdf_with_font_provider(layout, font_provider).unwrap_or_default()
}

fn try_render_pdf_with_font_provider(
    layout: &LayoutDocument,
    font_provider: Option<&FontProvider>,
) -> Result<Vec<u8>, PdfRenderError> {
    let mut pdf = Pdf::new();
    let catalog_id = Ref::new(1);
    let page_tree_id = Ref::new(2);
    let first_page_id = 3;
    let first_content_id = first_page_id + layout.pages.len() as i32;
    let first_font_id = first_content_id + layout.pages.len() as i32;
    let page_used_font_indexes = collect_page_used_font_indexes(layout, font_provider);
    let used_font_indexes = collect_used_font_indexes(&page_used_font_indexes);
    let mut next_object_id = first_font_id;
    let mut font_refs = [None; 14];
    for font_idx in &used_font_indexes {
        font_refs[*font_idx] = Some(Ref::new(next_object_id));
        next_object_id += 1;
    }
    let symbol_to_unicode_ref = font_index_for_resource(SYMBOL_REGULAR)
        .and_then(|index| font_refs[index])
        .map(|_| next_ref(&mut next_object_id));
    let zapf_dingbats_to_unicode_ref = font_index_for_resource(ZAPF_DINGBATS_REGULAR)
        .and_then(|index| font_refs[index])
        .map(|_| next_ref(&mut next_object_id));
    let extended_latin_usage = collect_extended_latin_font_usage(layout, font_provider);
    let mut extended_latin_to_unicode_refs = [None; 14];
    for (font_idx, usage) in extended_latin_usage.iter().enumerate() {
        if !usage.is_empty() {
            extended_latin_to_unicode_refs[font_idx] = Some(next_ref(&mut next_object_id));
        }
    }
    let supplied_fonts = collect_supplied_pdf_fonts(layout, font_provider, &mut next_object_id);
    let page_supplied_font_indexes =
        collect_page_supplied_font_indexes(layout, font_provider, &supplied_fonts);
    let first_image_id = next_object_id;

    let page_refs = (0..layout.pages.len())
        .map(|idx| Ref::new(first_page_id + idx as i32))
        .collect::<Vec<_>>();
    let image_refs = collect_image_refs(layout, first_image_id);

    pdf.catalog(catalog_id).pages(page_tree_id);
    pdf.pages(page_tree_id)
        .kids(page_refs.iter().copied())
        .count(page_refs.len() as i32);

    for (idx, page) in layout.pages.iter().enumerate() {
        let page_id = page_refs[idx];
        let content_id = Ref::new(first_content_id + idx as i32);

        let mut page_writer = pdf.page(page_id);
        page_writer.parent(page_tree_id);
        page_writer.media_box(Rect::new(0.0, 0.0, page.width, page.height));
        page_writer.contents(content_id);
        {
            let mut resources = page_writer.resources();
            {
                let mut fonts = resources.fonts();
                for font_idx in &page_used_font_indexes[idx] {
                    let (resource_name, _base_font) = BUILTIN_FONTS[*font_idx];
                    if let Some(font_ref) = font_refs[*font_idx] {
                        fonts.pair(Name(resource_name), font_ref);
                    }
                }
                for supplied_idx in &page_supplied_font_indexes[idx] {
                    let supplied = &supplied_fonts[*supplied_idx];
                    fonts.pair(Name(&supplied.resource_name), supplied.type0_ref);
                }
            }
            if let Some(page_images) = image_refs.get(idx)
                && !page_images.is_empty()
            {
                let mut x_objects = resources.x_objects();
                for image in page_images {
                    x_objects.pair(Name(&image.name), image.id);
                }
            }
            let opacity_pairs = collect_page_opacity_pairs(page);
            if page_uses_passive_jpeg_luminosity_blend(page) || !opacity_pairs.is_empty() {
                let mut ext_g_states = resources.ext_g_states();
                if page_uses_passive_jpeg_luminosity_blend(page) {
                    ext_g_states
                        .insert(Name(PASSIVE_JPEG_LUMINOSITY_GS))
                        .start::<pdf_writer::writers::ExtGraphicsState>()
                        .blend_mode(BlendMode::Luminosity);
                }
                for (fill_percent, stroke_percent) in opacity_pairs {
                    let name = passive_shape_alpha_name(fill_percent, stroke_percent);
                    let mut state = ext_g_states
                        .insert(Name(&name))
                        .start::<pdf_writer::writers::ExtGraphicsState>();
                    state.non_stroking_alpha(f32::from(fill_percent) / 100.0);
                    state.stroking_alpha(f32::from(stroke_percent) / 100.0);
                }
            }
        }
        page_writer.finish();

        let mut content = Content::new();
        let mut image_ref_cursor = 0usize;
        let page_images = image_refs.get(idx).map(Vec::as_slice).unwrap_or(&[]);
        for item in &page.items {
            draw_layout_item(
                &mut content,
                item,
                layout,
                font_provider,
                &supplied_fonts,
                page_images,
                &mut image_ref_cursor,
            );
        }
        pdf.stream(content_id, &content.finish());
    }

    for font_idx in used_font_indexes {
        let Some(font_ref) = font_refs[font_idx] else {
            continue;
        };
        let (_resource_name, base_font) = BUILTIN_FONTS[font_idx];
        let mut font = pdf.type1_font(font_ref);
        font.base_font(Name(base_font));
        if base_font == b"Symbol" {
            if let Some(to_unicode_ref) = symbol_to_unicode_ref {
                font.to_unicode(to_unicode_ref);
            }
        } else if base_font == b"ZapfDingbats" {
            if let Some(to_unicode_ref) = zapf_dingbats_to_unicode_ref {
                font.to_unicode(to_unicode_ref);
            }
        } else if !extended_latin_usage[font_idx].is_empty() {
            {
                let mut encoding = font.encoding_custom();
                encoding.base_encoding(Name(b"WinAnsiEncoding"));
                let mut differences = encoding.differences();
                for entry in &extended_latin_usage[font_idx].entries {
                    differences.consecutive(entry.byte, [Name(entry.glyph_name)]);
                }
            }
            if let Some(to_unicode_ref) = extended_latin_to_unicode_refs[font_idx] {
                font.to_unicode(to_unicode_ref);
            }
        } else {
            font.encoding_predefined(Name(b"WinAnsiEncoding"));
        }
    }

    if let Some(to_unicode_ref) = symbol_to_unicode_ref {
        let symbol_to_unicode =
            passive_to_unicode_cmap(b"OpenRtfConverter-Symbol", PASSIVE_SYMBOL_TO_UNICODE)?;
        pdf.stream(to_unicode_ref, &symbol_to_unicode);
    }
    if let Some(to_unicode_ref) = zapf_dingbats_to_unicode_ref {
        let zapf_dingbats_to_unicode = passive_to_unicode_cmap(
            b"OpenRtfConverter-ZapfDingbats",
            PASSIVE_ZAPF_DINGBATS_TO_UNICODE,
        )?;
        pdf.stream(to_unicode_ref, &zapf_dingbats_to_unicode);
    }
    for (font_idx, to_unicode_ref) in extended_latin_to_unicode_refs.iter().enumerate() {
        let Some(to_unicode_ref) = *to_unicode_ref else {
            continue;
        };
        let entries = extended_latin_usage[font_idx].to_unicode_entries();
        let extended_latin_to_unicode =
            passive_to_unicode_cmap(b"OpenRtfConverter-ExtendedLatin", &entries)?;
        pdf.stream(to_unicode_ref, &extended_latin_to_unicode);
    }
    write_supplied_pdf_fonts(&mut pdf, font_provider, &supplied_fonts)?;

    for (page_idx, page) in layout.pages.iter().enumerate() {
        let page_images = image_refs.get(page_idx).map(Vec::as_slice).unwrap_or(&[]);
        let mut image_ref_cursor = 0usize;
        page.for_each_image_fragment(&mut |fragment| {
            if !image_format_uses_xobject(fragment.image.format) {
                if fragment.image.format == ImageFormat::WmfVector {
                    for command in &fragment.image.vector_commands {
                        let StaticImageVectorCommand::RasterImage { image, .. } = command else {
                            continue;
                        };
                        let Some(image_ref) = page_images.get(image_ref_cursor) else {
                            return;
                        };
                        image_ref_cursor = image_ref_cursor.saturating_add(1);
                        write_passive_image_xobject(
                            &mut pdf,
                            image_ref.id,
                            image.format,
                            image.width_px,
                            image.height_px,
                            &image.bytes,
                            &image.palette,
                            image.tone_adjustment,
                            image_ref.alpha_mask_id,
                        );
                        if let (Some(alpha_mask), Some(alpha_mask_id)) =
                            (image.alpha_mask.as_ref(), image_ref.alpha_mask_id)
                        {
                            write_image_alpha_mask(
                                &mut pdf,
                                alpha_mask_id,
                                alpha_mask,
                                image.width_px,
                                image.height_px,
                            );
                        }
                    }
                }
                return;
            }
            let Some(image_ref) = page_images.get(image_ref_cursor) else {
                return;
            };
            image_ref_cursor = image_ref_cursor.saturating_add(1);
            match fragment.image.format {
                ImageFormat::Jpeg
                | ImageFormat::JpegInverted
                | ImageFormat::JpegPassiveGrayscale
                | ImageFormat::JpegPassiveBilevel
                | ImageFormat::JpegGrayscale
                | ImageFormat::JpegGrayscaleInverted
                | ImageFormat::JpegCmyk
                | ImageFormat::JpegCmykInverted
                | ImageFormat::JpegCmykPassiveGrayscale
                | ImageFormat::JpegCmykPassiveBilevel
                | ImageFormat::Png
                | ImageFormat::PngGrayscale
                | ImageFormat::PngIndexed
                | ImageFormat::Rgb8 => write_passive_image_xobject(
                    &mut pdf,
                    image_ref.id,
                    fragment.image.format,
                    fragment.image.width_px,
                    fragment.image.height_px,
                    &fragment.image.bytes,
                    &fragment.image.palette,
                    fragment.image.tone_adjustment,
                    image_ref.alpha_mask_id,
                ),
                ImageFormat::WmfVector | ImageFormat::Placeholder => {}
            }
            if let (Some(alpha_mask), Some(alpha_mask_id)) =
                (fragment.image.alpha_mask.as_ref(), image_ref.alpha_mask_id)
            {
                write_image_alpha_mask(
                    &mut pdf,
                    alpha_mask_id,
                    alpha_mask,
                    fragment.image.width_px,
                    fragment.image.height_px,
                );
            }
        });
    }

    Ok(pdf.finish())
}

pub(crate) fn render_pdf_plan(plan: &PdfPlan<'_, '_>) -> Result<Vec<u8>, PdfRenderError> {
    plan.validate()?;
    if estimate_passive_pdf_object_count(plan.layout(), Some(plan.font_session().provider()))
        != plan.object_count()
    {
        return Err(PdfRenderError::InternalPlanMismatch {
            context: "planned object count",
        });
    }
    try_render_pdf_with_font_provider(plan.layout(), Some(plan.font_session().provider()))
}
