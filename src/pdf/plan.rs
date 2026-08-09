//! Deterministic resource discovery and exact PDF object-reference assignment.

use super::*;

pub(in crate::pdf) fn next_ref(next_object_id: &mut i32) -> Ref {
    let reference = Ref::new(*next_object_id);
    *next_object_id += 1;
    reference
}

pub(in crate::pdf) fn collect_page_used_font_indexes(
    layout: &LayoutDocument,
    font_provider: Option<&FontProvider>,
) -> Vec<Vec<usize>> {
    layout
        .pages
        .iter()
        .map(|page| collect_used_font_indexes_for_page(layout, page, font_provider))
        .collect()
}

pub(in crate::pdf) fn collect_used_font_indexes(
    page_used_font_indexes: &[Vec<usize>],
) -> Vec<usize> {
    let mut used = [false; 14];
    for page_fonts in page_used_font_indexes {
        for font_idx in page_fonts {
            used[*font_idx] = true;
        }
    }
    used_font_index_list(&used)
}

pub(in crate::pdf) fn collect_used_font_indexes_for_page(
    layout: &LayoutDocument,
    page: &crate::layout::LayoutPage,
    font_provider: Option<&FontProvider>,
) -> Vec<usize> {
    let mut used = [false; 14];
    page.for_each_text_fragment(&mut |fragment| {
        if text_fragment_renders_as_passive_vector_only(fragment) {
            return;
        }
        if font_provider
            .and_then(|provider| supplied_text_encoding_parts(fragment, layout, provider))
            .is_some()
        {
            return;
        }
        mark_used_font_resource(
            &mut used,
            font_resource_for_style(fragment.font_family, &fragment.style),
        );
    });
    page.for_each_image_fragment(&mut |fragment| {
        if fragment.image.format == ImageFormat::Placeholder
            && fragment.width >= 96.0
            && fragment.height >= 24.0
        {
            mark_used_font_resource(&mut used, HELVETICA_REGULAR);
        }
        for command in &fragment.image.vector_commands {
            if let StaticImageVectorCommand::Text { bold, .. } = command {
                mark_used_font_resource(
                    &mut used,
                    if *bold {
                        HELVETICA_BOLD
                    } else {
                        HELVETICA_REGULAR
                    },
                );
            }
        }
    });
    used_font_index_list(&used)
}

pub(in crate::pdf) fn collect_page_opacity_pairs(
    page: &crate::layout::LayoutPage,
) -> Vec<(u8, u8)> {
    let mut pairs = Vec::new();
    for item in &page.items {
        collect_layout_item_opacity_pairs(item, &mut pairs);
    }
    pairs
}

pub(in crate::pdf) fn collect_layout_item_opacity_pairs(
    item: &LayoutItem,
    pairs: &mut Vec<(u8, u8)>,
) {
    match item {
        LayoutItem::Opacity {
            fill_percent,
            stroke_percent,
            item,
        } => {
            let pair = (*fill_percent, *stroke_percent);
            if !pairs.contains(&pair) {
                pairs.push(pair);
            }
            collect_layout_item_opacity_pairs(item, pairs);
        }
        LayoutItem::Drawing(fragment) => collect_layout_item_opacity_pairs(&fragment.item, pairs),
        LayoutItem::ClipRect { item, .. } => collect_layout_item_opacity_pairs(item, pairs),
        LayoutItem::Text(_)
        | LayoutItem::Highlight { .. }
        | LayoutItem::Underline { .. }
        | LayoutItem::Line { .. }
        | LayoutItem::CappedLine { .. }
        | LayoutItem::JoinedPolyline { .. }
        | LayoutItem::Ellipse { .. }
        | LayoutItem::RoundedRectangle { .. }
        | LayoutItem::Polygon { .. }
        | LayoutItem::Image(_) => {}
    }
}

pub(in crate::pdf) fn collect_supplied_pdf_fonts(
    layout: &LayoutDocument,
    font_provider: Option<&FontProvider>,
    next_object_id: &mut i32,
) -> Vec<SuppliedPdfFont> {
    let Some(font_provider) = font_provider else {
        return Vec::new();
    };
    let mut fonts = Vec::<SuppliedPdfFont>::new();
    for page in &layout.pages {
        page.for_each_text_fragment(&mut |fragment| {
            let Some((asset_index, glyphs, _encoded)) =
                supplied_text_encoding_parts(fragment, layout, font_provider)
            else {
                return;
            };
            let font_index = if let Some(index) = fonts
                .iter()
                .position(|font| font.asset_index == asset_index)
            {
                index
            } else {
                let index = fonts.len();
                let base_name = font_provider
                    .assets
                    .get(asset_index)
                    .map(|asset| supplied_pdf_font_base_name(asset, index))
                    .unwrap_or_else(|| supplied_pdf_font_fallback_base_name(index));
                fonts.push(SuppliedPdfFont {
                    asset_index,
                    resource_name: format!("TF{}", index + 1).into_bytes(),
                    base_name,
                    type0_ref: next_ref(next_object_id),
                    cid_ref: next_ref(next_object_id),
                    descriptor_ref: next_ref(next_object_id),
                    font_file_ref: next_ref(next_object_id),
                    cid_to_gid_map_ref: next_ref(next_object_id),
                    to_unicode_ref: next_ref(next_object_id),
                    used_glyphs: Vec::new(),
                });
                index
            };
            for glyph in glyphs {
                add_supplied_glyph(&mut fonts[font_index].used_glyphs, glyph);
            }
        });
    }
    fonts
}

pub(in crate::pdf) fn collect_used_supplied_font_asset_indexes(
    layout: &LayoutDocument,
    font_provider: Option<&FontProvider>,
) -> Vec<usize> {
    let Some(font_provider) = font_provider else {
        return Vec::new();
    };
    let mut indexes = Vec::<usize>::new();
    for page in &layout.pages {
        page.for_each_text_fragment(&mut |fragment| {
            let Some((asset_index, _glyphs, _encoded)) =
                supplied_text_encoding_parts(fragment, layout, font_provider)
            else {
                return;
            };
            if !indexes.contains(&asset_index) {
                indexes.push(asset_index);
            }
        });
    }
    indexes
}

pub(in crate::pdf) fn collect_page_supplied_font_indexes(
    layout: &LayoutDocument,
    font_provider: Option<&FontProvider>,
    supplied_fonts: &[SuppliedPdfFont],
) -> Vec<Vec<usize>> {
    layout
        .pages
        .iter()
        .map(|page| {
            let mut indexes = Vec::new();
            page.for_each_text_fragment(&mut |fragment| {
                let Some(encoding) =
                    encode_supplied_text_fragment(fragment, layout, font_provider, supplied_fonts)
                else {
                    return;
                };
                if !indexes.contains(&encoding.font_index) {
                    indexes.push(encoding.font_index);
                }
            });
            indexes
        })
        .collect()
}

pub(in crate::pdf) fn collect_extended_latin_font_usage(
    layout: &LayoutDocument,
    font_provider: Option<&FontProvider>,
) -> [ExtendedLatinUsage; 14] {
    let mut usage: [ExtendedLatinUsage; 14] =
        std::array::from_fn(|_| ExtendedLatinUsage::default());
    for page in &layout.pages {
        page.for_each_text_fragment(&mut |fragment| {
            if text_fragment_renders_as_passive_vector_only(fragment) {
                return;
            }
            if font_provider
                .and_then(|provider| supplied_text_encoding_parts(fragment, layout, provider))
                .is_some()
            {
                return;
            }
            let resource = font_resource_for_style(fragment.font_family, &fragment.style);
            let Some(font_idx) = font_index_for_resource(resource) else {
                return;
            };
            if !is_normal_text_font_index(font_idx) {
                return;
            }
            for ch in fragment.text.chars() {
                if let Some(entry) = extended_latin_entry_for_char(ch) {
                    usage[font_idx].add(entry);
                }
            }
        });
    }
    usage
}

pub(in crate::pdf) fn encode_supplied_text_fragment(
    fragment: &TextFragment,
    layout: &LayoutDocument,
    font_provider: Option<&FontProvider>,
    supplied_fonts: &[SuppliedPdfFont],
) -> Option<SuppliedTextEncoding> {
    encode_supplied_text_fragment_with_text(
        fragment,
        &fragment.text,
        layout,
        font_provider,
        supplied_fonts,
    )
}

pub(in crate::pdf) fn encode_supplied_text_fragment_with_text(
    fragment: &TextFragment,
    text: &str,
    layout: &LayoutDocument,
    font_provider: Option<&FontProvider>,
    supplied_fonts: &[SuppliedPdfFont],
) -> Option<SuppliedTextEncoding> {
    let font_provider = font_provider?;
    let (asset_index, glyphs, encoded) =
        supplied_text_encoding_parts_for_text(fragment, text, layout, font_provider)?;
    let font_index = supplied_fonts
        .iter()
        .position(|font| font.asset_index == asset_index)?;
    let asset = font_provider.assets.get(asset_index)?;
    let source_font = layout
        .fonts
        .iter()
        .find(|font| font.index == fragment.style.font_index);
    let font_size = fragment.style.font_size_points();
    let mut positioning_points = Vec::with_capacity(encoded.len() / 2);
    let mut previous = None;
    let mut deferred_whitespace_adjustment = 0.0;
    for (index, ch) in text
        .chars()
        .filter(|ch| !is_zero_width_pdf_char(*ch))
        .enumerate()
    {
        let source_advance_adjustment = index
            .checked_sub(1)
            .and_then(|previous_index| {
                let previous_char = previous?;
                let source_advance = source_word_compatible_advance_points(
                    source_font,
                    &fragment.style,
                    previous_char,
                    font_size,
                    Some(font_provider),
                )?;
                let supplied_advance = glyphs.get(previous_index)?.width * font_size / 1000.0;
                Some(source_advance - supplied_advance)
            })
            .unwrap_or(0.0);
        let kerning_adjustment = if style_uses_passive_kerning(&fragment.style) {
            previous
                .and_then(|left| {
                    source_word_compatible_pair_kerning_points(
                        source_font,
                        &fragment.style,
                        left,
                        ch,
                        Some(font_provider),
                    )
                    .or_else(|| {
                        font_asset_pair_kerning_points(
                            asset,
                            left,
                            ch,
                            fragment.style.font_size_points(),
                        )
                    })
                })
                .unwrap_or(0.0)
        } else {
            0.0
        };
        let adjustment =
            source_advance_adjustment + kerning_adjustment + deferred_whitespace_adjustment;
        if ch.is_whitespace() {
            // A whitespace glyph has no painted contour, so carry its incoming source-
            // metric correction to the next visible glyph. Besides preserving identical
            // visible geometry, this keeps the whitespace in the same two-byte CID
            // string as the preceding glyph for conservative PDF text extractors.
            deferred_whitespace_adjustment = adjustment;
            positioning_points.push(0.0);
        } else {
            deferred_whitespace_adjustment = 0.0;
            positioning_points.push(adjustment);
        }
        previous = Some(ch);
    }
    Some(SuppliedTextEncoding {
        font_index,
        encoded,
        positioning_points,
    })
}

pub(in crate::pdf) fn supplied_text_encoding_parts(
    fragment: &TextFragment,
    layout: &LayoutDocument,
    font_provider: &FontProvider,
) -> Option<(usize, Vec<SuppliedGlyph>, Vec<u8>)> {
    supplied_text_encoding_parts_for_text(fragment, &fragment.text, layout, font_provider)
}

pub(in crate::pdf) fn supplied_text_encoding_parts_for_text(
    fragment: &TextFragment,
    text: &str,
    layout: &LayoutDocument,
    font_provider: &FontProvider,
) -> Option<(usize, Vec<SuppliedGlyph>, Vec<u8>)> {
    if text.is_empty() {
        return None;
    }
    let source_font = layout
        .fonts
        .iter()
        .find(|font| font.index == fragment.style.font_index)?;
    let mut best = None;
    let mut best_key = (u8::MAX, u8::MAX, usize::MAX);
    for (asset_index, asset) in font_provider.assets.iter().enumerate() {
        let Some(family_priority) = supplied_font_asset_match_priority(asset, source_font) else {
            continue;
        };
        let style_score = supplied_font_style_mismatch_score(asset.style(), &fragment.style);
        let key = (family_priority, style_score, asset_index);
        if key >= best_key {
            continue;
        }
        let Some((glyphs, encoded)) = encode_text_with_font_asset(text, asset) else {
            continue;
        };
        if glyphs.is_empty() {
            continue;
        }
        best_key = key;
        best = Some((asset_index, glyphs, encoded));
        if (family_priority, style_score) == (0, 0) {
            break;
        }
    }
    best
}

pub(in crate::pdf) fn supplied_font_asset_match_priority(
    asset: &FontAsset,
    font: &crate::model::FontDef,
) -> Option<u8> {
    let primary = asset.family_match_priority(&font.name);
    let alternate = font
        .alternate_name
        .as_deref()
        .and_then(|name| asset.family_match_priority(name));
    if primary
        .into_iter()
        .chain(alternate)
        .any(|priority| priority == 0)
    {
        return Some(0);
    }
    if passive_supplied_font_fallback_name(font)
        .is_some_and(|fallback| asset.family_match_priority(fallback) == Some(0))
    {
        return Some(1);
    }
    primary
        .into_iter()
        .chain(alternate)
        .any(|priority| priority == 1)
        .then_some(2)
}

pub(in crate::pdf) fn supplied_pdf_font_base_name(asset: &FontAsset, font_index: usize) -> Vec<u8> {
    let Some(name) = supplied_pdf_font_postscript_name(asset) else {
        return supplied_pdf_font_fallback_base_name(font_index);
    };
    let sanitized = sanitize_pdf_font_name(&name, 96);
    if sanitized.is_empty() {
        supplied_pdf_font_fallback_base_name(font_index)
    } else {
        format!("ORTF{:02}+{sanitized}", font_index + 1).into_bytes()
    }
}

pub(in crate::pdf) fn supplied_pdf_font_fallback_base_name(font_index: usize) -> Vec<u8> {
    format!("ORTFSuppliedFont{}", font_index + 1).into_bytes()
}

pub(in crate::pdf) fn supplied_pdf_font_postscript_name(asset: &FontAsset) -> Option<String> {
    let face = Face::parse(asset.bytes(), 0).ok()?;
    for name in face.names() {
        if name.name_id != name_id::POST_SCRIPT_NAME {
            continue;
        }
        let Some(value) = font_name_to_string(&name) else {
            continue;
        };
        if !value.trim().is_empty() {
            return Some(value);
        }
    }
    None
}

pub(in crate::pdf) fn font_name_to_string(name: &ttf_parser::name::Name<'_>) -> Option<String> {
    if !name.is_unicode() || name.name.len() % 2 != 0 {
        return None;
    }
    let utf16 = name
        .name
        .chunks_exact(2)
        .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&utf16).ok()
}

pub(in crate::pdf) fn sanitize_pdf_font_name(value: &str, max_len: usize) -> String {
    let mut output = String::new();
    for ch in value.chars() {
        if output.len() >= max_len {
            break;
        }
        let byte = if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '+') {
            ch as u8
        } else {
            b'-'
        };
        if output.len() + 1 > max_len {
            break;
        }
        output.push(byte as char);
    }
    output.trim_matches('-').to_string()
}

pub(in crate::pdf) fn supplied_font_style_mismatch_score(
    asset_style: crate::fonts::FontAssetStyle,
    run_style: &CharacterStyle,
) -> u8 {
    u8::from(asset_style.bold != run_style.bold) + u8::from(asset_style.italic != run_style.italic)
}

pub(in crate::pdf) fn encode_text_with_font_asset(
    text: &str,
    asset: &FontAsset,
) -> Option<(Vec<SuppliedGlyph>, Vec<u8>)> {
    let face = Face::parse(asset.bytes(), 0).ok()?;
    let units_per_em = f32::from(face.units_per_em()).max(1.0);
    let mut glyphs = Vec::new();
    let mut encoded = Vec::new();
    for ch in text.chars() {
        if is_zero_width_pdf_char(ch) {
            continue;
        }
        let glyph_id = face.glyph_index(ch)?;
        if glyph_id.0 == 0 {
            return None;
        }
        let advance = face.glyph_hor_advance(glyph_id)?;
        let cid = supplied_pdf_cid_for_char(ch)?;
        encoded.extend_from_slice(&cid.to_be_bytes());
        glyphs.push(SuppliedGlyph {
            cid,
            gid: glyph_id.0,
            unicode: ch,
            width: f32::from(advance) * 1000.0 / units_per_em,
        });
    }
    Some((glyphs, encoded))
}

pub(in crate::pdf) fn supplied_pdf_cid_for_char(ch: char) -> Option<u16> {
    let codepoint = u32::from(ch);
    if codepoint == 0 || codepoint > u32::from(u16::MAX) {
        return None;
    }
    Some(codepoint as u16)
}

pub(in crate::pdf) fn add_supplied_glyph(glyphs: &mut Vec<SuppliedGlyph>, glyph: SuppliedGlyph) {
    if glyphs.iter().any(|existing| existing.cid == glyph.cid) {
        return;
    }
    glyphs.push(glyph);
    glyphs.sort_by_key(|glyph| glyph.cid);
}

pub(in crate::pdf) fn is_normal_text_font_index(font_idx: usize) -> bool {
    let (_resource_name, base_font) = BUILTIN_FONTS[font_idx];
    base_font != b"Symbol" && base_font != b"ZapfDingbats"
}

pub(in crate::pdf) fn used_font_index_list(used: &[bool; 14]) -> Vec<usize> {
    used.iter()
        .enumerate()
        .filter_map(|(idx, is_used)| is_used.then_some(idx))
        .collect()
}

pub(in crate::pdf) fn mark_used_font_resource(used: &mut [bool; 14], resource_name: &[u8]) {
    if let Some(index) = font_index_for_resource(resource_name) {
        used[index] = true;
    }
}

pub(in crate::pdf) fn font_index_for_resource(resource_name: &[u8]) -> Option<usize> {
    BUILTIN_FONTS
        .iter()
        .position(|(candidate, _base_font)| *candidate == resource_name)
}

#[derive(Debug, Clone)]
pub(in crate::pdf) struct PdfImageRef {
    pub(in crate::pdf) name: Vec<u8>,
    pub(in crate::pdf) id: Ref,
    pub(in crate::pdf) alpha_mask_id: Option<Ref>,
}

pub(in crate::pdf) fn collect_image_refs(
    layout: &LayoutDocument,
    first_image_id: i32,
) -> Vec<Vec<PdfImageRef>> {
    let mut next_id = first_image_id;
    layout
        .pages
        .iter()
        .map(|page| {
            let mut refs = Vec::new();
            page.for_each_image_fragment(&mut |fragment| {
                if image_format_uses_xobject(fragment.image.format) {
                    let id = Ref::new(next_id);
                    let name = format!("Im{}", next_id - first_image_id + 1).into_bytes();
                    next_id += 1;
                    let alpha_mask_id = fragment.image.alpha_mask.as_ref().map(|_| {
                        let id = Ref::new(next_id);
                        next_id += 1;
                        id
                    });
                    refs.push(PdfImageRef {
                        name,
                        id,
                        alpha_mask_id,
                    });
                }
                if fragment.image.format == ImageFormat::WmfVector {
                    for command in &fragment.image.vector_commands {
                        if let StaticImageVectorCommand::RasterImage { image, .. } = command {
                            let id = Ref::new(next_id);
                            let name = format!("Im{}", next_id - first_image_id + 1).into_bytes();
                            next_id += 1;
                            let alpha_mask_id = image.alpha_mask.as_ref().map(|_| {
                                let id = Ref::new(next_id);
                                next_id += 1;
                                id
                            });
                            refs.push(PdfImageRef {
                                name,
                                id,
                                alpha_mask_id,
                            });
                        }
                    }
                }
            });
            refs
        })
        .collect()
}

pub(in crate::pdf) fn image_format_uses_xobject(format: ImageFormat) -> bool {
    matches!(
        format,
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
            | ImageFormat::Rgb8
    )
}

pub(in crate::pdf) fn image_format_uses_passive_jpeg_luminosity_blend(format: ImageFormat) -> bool {
    matches!(
        format,
        ImageFormat::JpegPassiveGrayscale
            | ImageFormat::JpegPassiveBilevel
            | ImageFormat::JpegCmykPassiveGrayscale
            | ImageFormat::JpegCmykPassiveBilevel
    )
}

pub(in crate::pdf) fn page_uses_passive_jpeg_luminosity_blend(
    page: &crate::layout::LayoutPage,
) -> bool {
    let mut uses_blend = false;
    page.for_each_image_fragment(&mut |fragment| {
        uses_blend |= image_format_uses_passive_jpeg_luminosity_blend(fragment.image.format);
    });
    uses_blend
}

pub(in crate::pdf) fn count_image_xobjects(layout: &LayoutDocument) -> usize {
    layout
        .pages
        .iter()
        .map(|page| {
            let mut count = 0usize;
            page.for_each_image_fragment(&mut |fragment| {
                if image_format_uses_xobject(fragment.image.format) {
                    count = count
                        .saturating_add(1)
                        .saturating_add(usize::from(fragment.image.alpha_mask.is_some()));
                }
                if fragment.image.format == ImageFormat::WmfVector {
                    for command in &fragment.image.vector_commands {
                        if let StaticImageVectorCommand::RasterImage { image, .. } = command {
                            count = count
                                .saturating_add(1)
                                .saturating_add(usize::from(image.alpha_mask.is_some()));
                        }
                    }
                }
            });
            count
        })
        .sum()
}
