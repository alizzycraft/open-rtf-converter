//! Deterministic resource discovery and exact PDF object-reference assignment.

use super::*;

pub(crate) struct PdfPlan<'a, 'fonts> {
    pub(in crate::pdf) layout: &'a LayoutDocument,
    pub(in crate::pdf) font_session: &'a FontSession<'fonts>,
    pub(in crate::pdf) catalog_ref: Ref,
    pub(in crate::pdf) page_tree_ref: Ref,
    pub(in crate::pdf) pages: Vec<PlannedPage>,
    pub(in crate::pdf) builtin_fonts: Vec<PlannedBuiltinFont>,
    pub(in crate::pdf) supplied_fonts: Vec<PlannedSuppliedFont>,
    pub(in crate::pdf) images: Vec<PlannedImage>,
    pub(in crate::pdf) object_count: usize,
    pub(in crate::pdf) highest_object_id: i32,
}

pub(crate) struct PlannedPage {
    pub(crate) page_index: usize,
    pub(crate) page_ref: Ref,
    pub(crate) content_ref: Ref,
    pub(crate) builtin_font_indexes: Vec<usize>,
    pub(crate) supplied_font_indexes: Vec<usize>,
    pub(crate) opacity_pairs: Vec<(u8, u8)>,
    pub(crate) image_count: usize,
    pub(crate) text_encodings: Vec<Option<SuppliedTextEncoding>>,
    pub(crate) uses_luminosity_blend: bool,
}

pub(crate) struct PlannedBuiltinFont {
    pub(crate) font_index: usize,
    pub(crate) font_ref: Ref,
    pub(crate) to_unicode_ref: Option<Ref>,
    pub(crate) extended_latin_usage: ExtendedLatinUsage,
}

pub(crate) struct PlannedSuppliedFont {
    pub(crate) asset_index: usize,
    pub(crate) type0_ref: Ref,
    pub(crate) cid_ref: Ref,
    pub(crate) descriptor_ref: Ref,
    pub(crate) font_file_ref: Ref,
    pub(crate) cid_to_gid_map_ref: Ref,
    pub(crate) to_unicode_ref: Ref,
    pub(crate) resource_name: Vec<u8>,
    pub(crate) base_name: Vec<u8>,
    pub(crate) used_glyphs: Vec<SuppliedGlyph>,
}

pub(crate) struct PlannedImage {
    pub(crate) page_index: usize,
    pub(crate) format: ImageFormat,
    pub(crate) image_ref: Ref,
    pub(crate) alpha_mask_ref: Option<Ref>,
    pub(crate) is_vector_raster: bool,
    pub(crate) name: Vec<u8>,
}

pub(in crate::pdf) struct ObjectAllocator {
    next: usize,
    count: usize,
    limit: usize,
}

impl ObjectAllocator {
    fn new(limit: usize) -> Self {
        Self {
            next: 1,
            count: 0,
            limit,
        }
    }

    fn allocate(&mut self) -> Result<Ref, PdfRenderError> {
        let count = self
            .count
            .checked_add(1)
            .ok_or(PdfRenderError::ObjectIdOverflow)?;
        if count > self.limit {
            return Err(PdfRenderError::TooManyObjects {
                count,
                limit: self.limit,
            });
        }
        let id = i32::try_from(self.next).map_err(|_| PdfRenderError::ObjectIdOverflow)?;
        self.next = self
            .next
            .checked_add(1)
            .ok_or(PdfRenderError::ObjectIdOverflow)?;
        self.count = count;
        Ok(Ref::new(id))
    }

    fn count(&self) -> usize {
        self.count
    }

    fn highest_object_id(&self) -> i32 {
        i32::try_from(self.count).unwrap_or(i32::MAX)
    }

    #[cfg(test)]
    pub(in crate::pdf) fn with_state_for_test(next: usize, count: usize, limit: usize) -> Self {
        Self { next, count, limit }
    }

    #[cfg(test)]
    pub(in crate::pdf) fn allocate_for_test(&mut self) -> Result<Ref, PdfRenderError> {
        self.allocate()
    }
}

impl<'a, 'fonts> PdfPlan<'a, 'fonts> {
    #[cfg(test)]
    pub(crate) fn layout(&self) -> &'a LayoutDocument {
        self.layout
    }

    #[cfg(test)]
    pub(crate) fn font_session(&self) -> &'a FontSession<'fonts> {
        self.font_session
    }

    pub(crate) fn object_count(&self) -> usize {
        self.object_count
    }

    #[cfg(test)]
    pub(crate) fn highest_object_id(&self) -> i32 {
        self.highest_object_id
    }

    #[cfg(test)]
    pub(crate) fn pages(&self) -> &[PlannedPage] {
        &self.pages
    }

    #[cfg(test)]
    pub(crate) fn builtin_fonts(&self) -> &[PlannedBuiltinFont] {
        &self.builtin_fonts
    }

    #[cfg(test)]
    pub(crate) fn supplied_fonts(&self) -> &[PlannedSuppliedFont] {
        &self.supplied_fonts
    }

    #[cfg(test)]
    pub(crate) fn images(&self) -> &[PlannedImage] {
        &self.images
    }

    #[cfg(test)]
    pub(crate) fn corrupt_page_index_for_test(&mut self, page_index: usize, value: usize) {
        self.pages[page_index].page_index = value;
    }

    #[cfg(test)]
    pub(crate) fn corrupt_first_supplied_encoding_for_test(&mut self) {
        let encoding = self
            .pages
            .iter_mut()
            .flat_map(|page| page.text_encodings.iter_mut())
            .find_map(Option::as_mut)
            .expect("test plan should contain a supplied encoding");
        encoding.font_index = usize::MAX;
    }

    pub(crate) fn validate(&self) -> Result<(), PdfRenderError> {
        if self.pages.len() != self.layout.pages.len()
            || self.pages.iter().enumerate().any(|(index, page)| {
                let mut text_count = 0usize;
                self.layout.pages[index]
                    .for_each_text_fragment(&mut |_| text_count = text_count.saturating_add(1));
                page.page_index != index
                    || page.builtin_font_indexes.iter().any(|font| *font >= 14)
                    || page.builtin_font_indexes.iter().any(|font_index| {
                        !self
                            .builtin_fonts
                            .iter()
                            .any(|font| font.font_index == *font_index)
                    })
                    || page
                        .supplied_font_indexes
                        .iter()
                        .any(|font| *font >= self.supplied_fonts.len())
                    || page.text_encodings.iter().any(|encoding| {
                        encoding.as_ref().is_some_and(|encoding| {
                            encoding.font_index >= self.supplied_fonts.len()
                        })
                    })
                    || page.text_encodings.len() != text_count
                    || page
                        .opacity_pairs
                        .iter()
                        .any(|(fill, stroke)| *fill > 100 || *stroke > 100)
            })
        {
            return Err(PdfRenderError::InternalPlanMismatch {
                context: "page resource metadata",
            });
        }
        if self
            .pages
            .iter()
            .try_fold(0usize, |count, page| count.checked_add(page.image_count))
            != Some(self.images.len())
            || self.images.iter().any(|image| {
                image.page_index >= self.pages.len()
                    || (!image.is_vector_raster && !image_format_uses_xobject(image.format))
            })
        {
            return Err(PdfRenderError::InternalPlanMismatch {
                context: "image resource metadata",
            });
        }
        let mut image_start = 0usize;
        for page in &self.pages {
            let Some(image_end) = image_start.checked_add(page.image_count) else {
                return Err(PdfRenderError::InternalPlanMismatch {
                    context: "image resource metadata",
                });
            };
            if self.images[image_start..image_end]
                .iter()
                .any(|image| image.page_index != page.page_index)
            {
                return Err(PdfRenderError::InternalPlanMismatch {
                    context: "image resource metadata",
                });
            }
            image_start = image_end;
        }
        if let Some(asset_index) = self
            .supplied_fonts
            .iter()
            .map(|font| font.asset_index)
            .find(|asset_index| *asset_index >= self.font_session.provider().assets.len())
        {
            return Err(PdfRenderError::InvalidPreparedFontReference { asset_index });
        }
        if self.builtin_fonts.iter().any(|font| font.font_index >= 14) {
            return Err(PdfRenderError::InternalPlanMismatch {
                context: "built-in font resource metadata",
            });
        }
        if self.builtin_fonts.iter().any(|font| {
            let (_resource_name, base_font) = BUILTIN_FONTS[font.font_index];
            let needs_cmap = base_font == b"Symbol"
                || base_font == b"ZapfDingbats"
                || !font.extended_latin_usage.is_empty();
            font.to_unicode_ref.is_some() != needs_cmap
        }) {
            return Err(PdfRenderError::InternalPlanMismatch {
                context: "built-in font CMap metadata",
            });
        }
        if self.object_count == 0
            || self.highest_object_id != i32::try_from(self.object_count).unwrap_or(i32::MAX)
        {
            return Err(PdfRenderError::InternalPlanMismatch {
                context: "object allocation summary",
            });
        }
        let mut allocated = vec![false; self.object_count.saturating_add(1)];
        let references_are_valid = {
            let mut record = |reference: Ref| {
                let Ok(id) = usize::try_from(reference.get()) else {
                    return false;
                };
                if id == 0 || id > self.object_count || allocated[id] {
                    return false;
                }
                allocated[id] = true;
                true
            };
            record(self.catalog_ref)
                && record(self.page_tree_ref)
                && self.pages.iter().all(|page| record(page.page_ref))
                && self.pages.iter().all(|page| record(page.content_ref))
                && self.builtin_fonts.iter().all(|font| record(font.font_ref))
                && self
                    .builtin_fonts
                    .iter()
                    .filter_map(|font| font.to_unicode_ref)
                    .all(&mut record)
                && self.supplied_fonts.iter().all(|font| {
                    record(font.type0_ref)
                        && record(font.cid_ref)
                        && record(font.descriptor_ref)
                        && record(font.font_file_ref)
                        && record(font.cid_to_gid_map_ref)
                        && record(font.to_unicode_ref)
                })
                && self.images.iter().all(|image| {
                    record(image.image_ref) && image.alpha_mask_ref.is_none_or(&mut record)
                })
        };
        if !references_are_valid || allocated.iter().skip(1).any(|allocated| !allocated) {
            return Err(PdfRenderError::InternalPlanMismatch {
                context: "object reference allocation",
            });
        }
        Ok(())
    }
}

pub(crate) fn plan_pdf<'a, 'fonts>(
    layout: &'a LayoutDocument,
    font_session: &'a FontSession<'fonts>,
    object_limit: usize,
) -> Result<PdfPlan<'a, 'fonts>, PdfRenderError> {
    let mut allocator = ObjectAllocator::new(object_limit);
    let catalog_ref = allocator.allocate()?;
    let page_tree_ref = allocator.allocate()?;
    let page_refs = (0..layout.pages.len())
        .map(|_| allocator.allocate())
        .collect::<Result<Vec<_>, _>>()?;
    let content_refs = (0..layout.pages.len())
        .map(|_| allocator.allocate())
        .collect::<Result<Vec<_>, _>>()?;
    let font_provider = font_session.provider();
    let mut used_builtin_fonts = [false; 14];
    let mut extended_latin_usage: [ExtendedLatinUsage; 14] =
        std::array::from_fn(|_| ExtendedLatinUsage::default());
    let mut pending_supplied_fonts = Vec::<(usize, Vec<SuppliedGlyph>)>::new();
    let mut image_metadata = Vec::new();
    let mut pages = layout
        .pages
        .iter()
        .enumerate()
        .map(|(page_index, page)| {
            let mut page_builtin_fonts = [false; 14];
            let mut supplied_font_indexes = Vec::new();
            let mut text_encodings = Vec::new();
            page.for_each_text_fragment(&mut |fragment| {
                if let Some((asset_index, glyphs, encoded)) = supplied_text_encoding_parts_for_text(
                    fragment,
                    &fragment.text,
                    layout,
                    font_provider,
                ) {
                    let font_index = pending_supplied_fonts
                        .iter()
                        .position(|(candidate, _)| *candidate == asset_index)
                        .unwrap_or_else(|| {
                            pending_supplied_fonts.push((asset_index, Vec::new()));
                            pending_supplied_fonts.len() - 1
                        });
                    for glyph in &glyphs {
                        add_supplied_glyph(&mut pending_supplied_fonts[font_index].1, *glyph);
                    }
                    if !supplied_font_indexes.contains(&font_index) {
                        supplied_font_indexes.push(font_index);
                    }
                    if text_fragment_renders_as_passive_vector_only(fragment) {
                        text_encodings.push(None);
                        return;
                    }
                    text_encodings.push(passive_visual_text_for_fragment(fragment).is_none().then(
                        || {
                            build_supplied_text_encoding(
                                fragment,
                                &fragment.text,
                                layout,
                                font_provider,
                                asset_index,
                                font_index,
                                &glyphs,
                                encoded,
                            )
                        },
                    ));
                    return;
                }
                if text_fragment_renders_as_passive_vector_only(fragment) {
                    text_encodings.push(None);
                    return;
                }
                let resource = font_resource_for_style(fragment.font_family, &fragment.style);
                if let Some(font_index) = font_index_for_resource(resource) {
                    page_builtin_fonts[font_index] = true;
                    if is_normal_text_font_index(font_index) {
                        for ch in fragment.text.chars() {
                            if let Some(entry) = extended_latin_entry_for_char(ch) {
                                extended_latin_usage[font_index].add(entry);
                            }
                        }
                    }
                }
                text_encodings.push(None);
            });
            let image_start = image_metadata.len();
            let mut uses_luminosity_blend = false;
            page.for_each_image_fragment(&mut |fragment| {
                uses_luminosity_blend |=
                    image_format_uses_passive_jpeg_luminosity_blend(fragment.image.format);
                if fragment.image.format == ImageFormat::Placeholder
                    && fragment.width >= 96.0
                    && fragment.height >= 24.0
                {
                    mark_used_font_resource(&mut page_builtin_fonts, HELVETICA_REGULAR);
                }
                for command in &fragment.image.vector_commands {
                    if let StaticImageVectorCommand::Text { bold, .. } = command {
                        mark_used_font_resource(
                            &mut page_builtin_fonts,
                            if *bold {
                                HELVETICA_BOLD
                            } else {
                                HELVETICA_REGULAR
                            },
                        );
                    }
                }
                if image_format_uses_xobject(fragment.image.format) {
                    image_metadata.push((
                        page_index,
                        fragment.image.format,
                        fragment.image.alpha_mask.is_some(),
                        false,
                    ));
                }
                if fragment.image.format == ImageFormat::WmfVector {
                    for command in &fragment.image.vector_commands {
                        if let StaticImageVectorCommand::RasterImage { image, .. } = command {
                            image_metadata.push((
                                page_index,
                                image.format,
                                image.alpha_mask.is_some(),
                                true,
                            ));
                        }
                    }
                }
            });
            for (font_index, used) in page_builtin_fonts.iter().enumerate() {
                used_builtin_fonts[font_index] |= *used;
            }
            PlannedPage {
                page_index,
                page_ref: page_refs[page_index],
                content_ref: content_refs[page_index],
                builtin_font_indexes: used_font_index_list(&page_builtin_fonts),
                supplied_font_indexes,
                opacity_pairs: collect_page_opacity_pairs(page),
                image_count: image_metadata.len() - image_start,
                text_encodings,
                uses_luminosity_blend,
            }
        })
        .collect::<Vec<_>>();
    let mut builtin_fonts = used_font_index_list(&used_builtin_fonts)
        .into_iter()
        .map(|font_index| {
            Ok(PlannedBuiltinFont {
                font_index,
                font_ref: allocator.allocate()?,
                to_unicode_ref: None,
                extended_latin_usage: extended_latin_usage[font_index].clone(),
            })
        })
        .collect::<Result<Vec<_>, PdfRenderError>>()?;
    for special_font in [SYMBOL_REGULAR, ZAPF_DINGBATS_REGULAR] {
        if let Some(font_index) = font_index_for_resource(special_font)
            && let Some(font) = builtin_fonts
                .iter_mut()
                .find(|font| font.font_index == font_index)
        {
            font.to_unicode_ref = Some(allocator.allocate()?);
        }
    }
    for font in &mut builtin_fonts {
        if font.to_unicode_ref.is_none() && !font.extended_latin_usage.is_empty() {
            font.to_unicode_ref = Some(allocator.allocate()?);
        }
    }
    let mut supplied_fonts = Vec::with_capacity(pending_supplied_fonts.len());
    for (font_index, (asset_index, used_glyphs)) in pending_supplied_fonts.into_iter().enumerate() {
        let asset = font_provider
            .assets
            .get(asset_index)
            .ok_or(PdfRenderError::InvalidPreparedFontReference { asset_index })?;
        supplied_fonts.push(PlannedSuppliedFont {
            asset_index,
            type0_ref: allocator.allocate()?,
            cid_ref: allocator.allocate()?,
            descriptor_ref: allocator.allocate()?,
            font_file_ref: allocator.allocate()?,
            cid_to_gid_map_ref: allocator.allocate()?,
            to_unicode_ref: allocator.allocate()?,
            resource_name: format!("TF{}", font_index + 1).into_bytes(),
            base_name: supplied_pdf_font_base_name(asset, font_index),
            used_glyphs,
        });
    }
    for (page_index, page) in layout.pages.iter().enumerate() {
        let mut text_index = 0usize;
        page.for_each_text_fragment(&mut |fragment| {
            let Some(slot) = pages[page_index].text_encodings.get_mut(text_index) else {
                return;
            };
            text_index = text_index.saturating_add(1);
            if text_fragment_renders_as_passive_vector_only(fragment) || slot.is_some() {
                return;
            }
            let Some(visual_text) = passive_visual_text_for_fragment(fragment) else {
                return;
            };
            let Some((asset_index, glyphs, encoded)) = supplied_text_encoding_parts_for_text(
                fragment,
                &visual_text,
                layout,
                font_provider,
            ) else {
                return;
            };
            let Some(font_index) = supplied_fonts
                .iter()
                .position(|font| font.asset_index == asset_index)
            else {
                return;
            };
            *slot = Some(build_supplied_text_encoding(
                fragment,
                &visual_text,
                layout,
                font_provider,
                asset_index,
                font_index,
                &glyphs,
                encoded,
            ));
        });
    }
    let mut images = Vec::with_capacity(image_metadata.len());
    let first_image_id = allocator.count().saturating_add(1);
    for (page_index, format, has_alpha_mask, is_vector_raster) in image_metadata {
        let image_ref = allocator.allocate()?;
        images.push(PlannedImage {
            page_index,
            format,
            name: format!("Im{}", image_ref.get() as usize - first_image_id + 1).into_bytes(),
            image_ref,
            alpha_mask_ref: has_alpha_mask.then(|| allocator.allocate()).transpose()?,
            is_vector_raster,
        });
    }
    let object_count = allocator.count();
    let highest_object_id = allocator.highest_object_id();
    Ok(PdfPlan {
        layout,
        font_session,
        catalog_ref,
        page_tree_ref,
        pages,
        builtin_fonts,
        supplied_fonts,
        images,
        object_count,
        highest_object_id,
    })
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

fn build_supplied_text_encoding(
    fragment: &TextFragment,
    text: &str,
    layout: &LayoutDocument,
    font_provider: &FontProvider,
    asset_index: usize,
    font_index: usize,
    glyphs: &[SuppliedGlyph],
    encoded: Vec<u8>,
) -> SuppliedTextEncoding {
    let asset = &font_provider.assets[asset_index];
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
    SuppliedTextEncoding {
        font_index,
        encoded,
        positioning_points,
    }
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
