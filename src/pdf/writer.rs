//! Catalog, page, resource, stream, and final PDF byte serialization.

use super::*;

pub fn render_pdf(layout: &LayoutDocument) -> Vec<u8> {
    render_pdf_with_font_provider(layout, None)
}

pub fn estimate_passive_pdf_object_count(
    layout: &LayoutDocument,
    font_provider: Option<&FontProvider>,
) -> usize {
    let provider = font_provider.cloned().unwrap_or_default();
    let Ok(catalog) = PreparedFontCatalog::prepare(&provider) else {
        return 0;
    };
    let session = FontSession::new(&catalog);
    plan_pdf(layout, &session, usize::MAX)
        .map(|plan| plan.object_count())
        .unwrap_or(0)
}

pub fn render_pdf_with_font_provider(
    layout: &LayoutDocument,
    font_provider: Option<&FontProvider>,
) -> Vec<u8> {
    let provider = font_provider.cloned().unwrap_or_default();
    let Ok(catalog) = PreparedFontCatalog::prepare(&provider) else {
        return Vec::new();
    };
    let session = FontSession::new(&catalog);
    let Ok(plan) = plan_pdf(layout, &session, usize::MAX) else {
        return Vec::new();
    };
    render_pdf_plan(&plan).unwrap_or_default()
}

fn try_render_pdf_plan(plan: &PdfPlan<'_, '_>) -> Result<Vec<u8>, PdfRenderError> {
    let layout = plan.layout;
    let font_provider = Some(plan.font_session.provider());
    let mut pdf = Pdf::new();
    let catalog_id = plan.catalog_ref;
    let page_tree_id = plan.page_tree_ref;
    let mut font_refs = [None; 14];
    for font in &plan.builtin_fonts {
        font_refs[font.font_index] = Some(font.font_ref);
    }
    let page_refs = plan
        .pages
        .iter()
        .map(|page| page.page_ref)
        .collect::<Vec<_>>();

    pdf.catalog(catalog_id).pages(page_tree_id);
    pdf.pages(page_tree_id)
        .kids(page_refs.iter().copied())
        .count(page_refs.len() as i32);

    let mut page_image_start = 0usize;
    for (idx, page) in layout.pages.iter().enumerate() {
        let planned_page = &plan.pages[idx];
        let page_id = planned_page.page_ref;
        let content_id = planned_page.content_ref;

        let mut page_writer = pdf.page(page_id);
        page_writer.parent(page_tree_id);
        page_writer.media_box(Rect::new(0.0, 0.0, page.width, page.height));
        page_writer.contents(content_id);
        {
            let mut resources = page_writer.resources();
            {
                let mut fonts = resources.fonts();
                for font_idx in &planned_page.builtin_font_indexes {
                    let (resource_name, _base_font) = BUILTIN_FONTS[*font_idx];
                    if let Some(font_ref) = font_refs[*font_idx] {
                        fonts.pair(Name(resource_name), font_ref);
                    }
                }
                for supplied_idx in &planned_page.supplied_font_indexes {
                    let supplied = &plan.supplied_fonts[*supplied_idx];
                    fonts.pair(Name(&supplied.resource_name), supplied.type0_ref);
                }
            }
            let page_image_end = page_image_start + planned_page.image_count;
            let page_images = &plan.images[page_image_start..page_image_end];
            if !page_images.is_empty() {
                let mut x_objects = resources.x_objects();
                for image in page_images {
                    x_objects.pair(Name(&image.name), image.image_ref);
                }
            }
            if planned_page.uses_luminosity_blend || !planned_page.opacity_pairs.is_empty() {
                let mut ext_g_states = resources.ext_g_states();
                if planned_page.uses_luminosity_blend {
                    ext_g_states
                        .insert(Name(PASSIVE_JPEG_LUMINOSITY_GS))
                        .start::<pdf_writer::writers::ExtGraphicsState>()
                        .blend_mode(BlendMode::Luminosity);
                }
                for (fill_percent, stroke_percent) in &planned_page.opacity_pairs {
                    let name = passive_shape_alpha_name(*fill_percent, *stroke_percent);
                    let mut state = ext_g_states
                        .insert(Name(&name))
                        .start::<pdf_writer::writers::ExtGraphicsState>();
                    state.non_stroking_alpha(f32::from(*fill_percent) / 100.0);
                    state.stroking_alpha(f32::from(*stroke_percent) / 100.0);
                }
            }
        }
        page_writer.finish();

        let mut content = Content::new();
        let mut image_ref_cursor = 0usize;
        let page_image_end = page_image_start + planned_page.image_count;
        let page_images = &plan.images[page_image_start..page_image_end];
        let mut text_encoding_cursor = 0usize;
        for item in &page.items {
            draw_layout_item(
                &mut content,
                item,
                layout,
                &plan.supplied_fonts,
                &planned_page.text_encodings,
                &mut text_encoding_cursor,
                page_images,
                &mut image_ref_cursor,
            );
        }
        if image_ref_cursor != page_images.len()
            || text_encoding_cursor != planned_page.text_encodings.len()
        {
            return Err(PdfRenderError::InternalPlanMismatch {
                context: "page content resource cursors",
            });
        }
        pdf.stream(content_id, &content.finish());
        page_image_start = page_image_end;
    }

    for planned_font in &plan.builtin_fonts {
        let font_idx = planned_font.font_index;
        let font_ref = planned_font.font_ref;
        let (_resource_name, base_font) = BUILTIN_FONTS[font_idx];
        let mut font = pdf.type1_font(font_ref);
        font.base_font(Name(base_font));
        if base_font == b"Symbol" {
            if let Some(to_unicode_ref) = planned_font.to_unicode_ref {
                font.to_unicode(to_unicode_ref);
            }
        } else if base_font == b"ZapfDingbats" {
            if let Some(to_unicode_ref) = planned_font.to_unicode_ref {
                font.to_unicode(to_unicode_ref);
            }
        } else if !planned_font.extended_latin_usage.is_empty() {
            {
                let mut encoding = font.encoding_custom();
                encoding.base_encoding(Name(b"WinAnsiEncoding"));
                let mut differences = encoding.differences();
                for entry in &planned_font.extended_latin_usage.entries {
                    differences.consecutive(entry.byte, [Name(entry.glyph_name)]);
                }
            }
            if let Some(to_unicode_ref) = planned_font.to_unicode_ref {
                font.to_unicode(to_unicode_ref);
            }
        } else {
            font.encoding_predefined(Name(b"WinAnsiEncoding"));
        }
    }

    for (resource, name, entries) in [
        (
            SYMBOL_REGULAR,
            b"OpenRtfConverter-Symbol".as_slice(),
            PASSIVE_SYMBOL_TO_UNICODE,
        ),
        (
            ZAPF_DINGBATS_REGULAR,
            b"OpenRtfConverter-ZapfDingbats".as_slice(),
            PASSIVE_ZAPF_DINGBATS_TO_UNICODE,
        ),
    ] {
        let Some(font_index) = font_index_for_resource(resource) else {
            continue;
        };
        let Some(to_unicode_ref) = plan
            .builtin_fonts
            .iter()
            .find(|font| font.font_index == font_index)
            .and_then(|font| font.to_unicode_ref)
        else {
            continue;
        };
        let to_unicode = passive_to_unicode_cmap(name, entries)?;
        pdf.stream(to_unicode_ref, &to_unicode);
    }
    for planned_font in &plan.builtin_fonts {
        if planned_font.extended_latin_usage.is_empty() {
            continue;
        }
        let Some(to_unicode_ref) = planned_font.to_unicode_ref else {
            return Err(PdfRenderError::InternalPlanMismatch {
                context: "extended Latin CMap reference",
            });
        };
        let entries = planned_font.extended_latin_usage.to_unicode_entries();
        let to_unicode = passive_to_unicode_cmap(b"OpenRtfConverter-ExtendedLatin", &entries)?;
        pdf.stream(to_unicode_ref, &to_unicode);
    }
    write_supplied_pdf_fonts(&mut pdf, font_provider, &plan.supplied_fonts)?;

    let mut page_image_start = 0usize;
    for (page_idx, page) in layout.pages.iter().enumerate() {
        let page_image_end = page_image_start + plan.pages[page_idx].image_count;
        let page_images = &plan.images[page_image_start..page_image_end];
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
                            image_ref.image_ref,
                            image.format,
                            image.width_px,
                            image.height_px,
                            &image.bytes,
                            &image.palette,
                            image.tone_adjustment,
                            image_ref.alpha_mask_ref,
                        );
                        if let (Some(alpha_mask), Some(alpha_mask_id)) =
                            (image.alpha_mask.as_ref(), image_ref.alpha_mask_ref)
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
                    image_ref.image_ref,
                    fragment.image.format,
                    fragment.image.width_px,
                    fragment.image.height_px,
                    &fragment.image.bytes,
                    &fragment.image.palette,
                    fragment.image.tone_adjustment,
                    image_ref.alpha_mask_ref,
                ),
                ImageFormat::WmfVector | ImageFormat::Placeholder => {}
            }
            if let (Some(alpha_mask), Some(alpha_mask_id)) =
                (fragment.image.alpha_mask.as_ref(), image_ref.alpha_mask_ref)
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
        if image_ref_cursor != page_images.len() {
            return Err(PdfRenderError::InvalidImageReference {
                page_index: page_idx,
            });
        }
        page_image_start = page_image_end;
    }

    Ok(pdf.finish())
}

pub(crate) fn render_pdf_plan(plan: &PdfPlan<'_, '_>) -> Result<Vec<u8>, PdfRenderError> {
    plan.validate()?;
    try_render_pdf_plan(plan)
}
