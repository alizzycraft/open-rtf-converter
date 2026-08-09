//! Document statistics and late page, note, section, and bookmark resolution.

use super::*;

#[derive(Debug, Default)]
pub(in crate::layout) struct DocumentStatsBuilder {
    pub(in crate::layout) stats: DocumentStats,
    pub(in crate::layout) in_word: bool,
    pub(in crate::layout) counted_repeating_paragraphs: Vec<(usize, usize)>,
    pub(in crate::layout) counted_repeating_images: Vec<(usize, usize)>,
    pub(in crate::layout) counted_repeating_shapes: Vec<(usize, usize)>,
}

impl DocumentStatsBuilder {
    fn push_text(&mut self, text: &str) {
        let text = remove_internal_stat_markers(text);
        for ch in text.chars() {
            if ch.is_whitespace() {
                self.in_word = false;
                self.stats.chars_with_spaces = self.stats.chars_with_spaces.saturating_add(1);
                continue;
            }
            if !self.in_word {
                self.stats.words = self.stats.words.saturating_add(1);
                self.in_word = true;
            }
            self.stats.chars = self.stats.chars.saturating_add(1);
            self.stats.chars_with_spaces = self.stats.chars_with_spaces.saturating_add(1);
        }
    }

    fn finish_text_boundary(&mut self) {
        self.in_word = false;
    }

    fn push_repeating_paragraphs_once(&mut self, paragraphs: &[Paragraph]) {
        if paragraphs.is_empty() {
            return;
        }
        let key = (paragraphs.as_ptr() as usize, paragraphs.len());
        if self.counted_repeating_paragraphs.contains(&key) {
            return;
        }
        self.counted_repeating_paragraphs.push(key);
        push_paragraphs_stats(self, paragraphs);
    }

    fn push_repeating_shapes_once(&mut self, shapes: &[StaticShape]) {
        if shapes.is_empty() {
            return;
        }
        let key = (shapes.as_ptr() as usize, shapes.len());
        if self.counted_repeating_shapes.contains(&key) {
            return;
        }
        self.counted_repeating_shapes.push(key);
        push_shapes_stats(self, shapes);
    }

    fn push_repeating_images_once(&mut self, images: &[StaticImage], content_width: f32) {
        if images.is_empty() {
            return;
        }
        let key = (images.as_ptr() as usize, images.len());
        if self.counted_repeating_images.contains(&key) {
            return;
        }
        self.counted_repeating_images.push(key);
        push_images_stats(self, images, content_width);
    }
}

pub(in crate::layout) fn document_stats(document: &Document) -> DocumentStats {
    let mut builder = DocumentStatsBuilder::default();
    let page_visibility = estimate_document_page_variant_visibility(document);
    let document_content_width = content_width_for_page_settings(&document.page);
    push_repeating_paragraph_variant_stats(
        &mut builder,
        &document.header,
        &document.first_page_header,
        &document.even_page_header,
        &document.header,
        &document.first_page_header,
        &document.even_page_header,
        document.page.title_page,
        document.page.facing_pages,
        page_visibility,
    );
    push_repeating_image_variant_stats(
        &mut builder,
        &document.header_images,
        &document.first_page_header_images,
        &document.even_page_header_images,
        &document.header_images,
        &document.first_page_header_images,
        &document.even_page_header_images,
        document.page.title_page,
        document.page.facing_pages,
        page_visibility,
        document_content_width,
    );
    push_repeating_shape_variant_stats(
        &mut builder,
        &document.header_shapes,
        &document.first_page_header_shapes,
        &document.even_page_header_shapes,
        &document.header_shapes,
        &document.first_page_header_shapes,
        &document.even_page_header_shapes,
        document.page.title_page,
        document.page.facing_pages,
        page_visibility,
    );
    push_repeating_paragraph_variant_stats(
        &mut builder,
        &document.footer,
        &document.first_page_footer,
        &document.even_page_footer,
        &document.footer,
        &document.first_page_footer,
        &document.even_page_footer,
        document.page.title_page,
        document.page.facing_pages,
        page_visibility,
    );
    push_repeating_image_variant_stats(
        &mut builder,
        &document.footer_images,
        &document.first_page_footer_images,
        &document.even_page_footer_images,
        &document.footer_images,
        &document.first_page_footer_images,
        &document.even_page_footer_images,
        document.page.title_page,
        document.page.facing_pages,
        page_visibility,
        document_content_width,
    );
    push_repeating_shape_variant_stats(
        &mut builder,
        &document.footer_shapes,
        &document.first_page_footer_shapes,
        &document.even_page_footer_shapes,
        &document.footer_shapes,
        &document.first_page_footer_shapes,
        &document.even_page_footer_shapes,
        document.page.title_page,
        document.page.facing_pages,
        page_visibility,
    );
    push_background_shape_stats(
        &mut builder,
        &document.background_shapes,
        &document.background_shapes,
    );
    for paragraph in &document.footnotes {
        push_paragraph_stats(&mut builder, paragraph);
    }
    for paragraph in &document.endnotes {
        push_paragraph_stats(&mut builder, paragraph);
    }
    for (block_idx, block) in document.blocks.iter().enumerate() {
        push_block_stats(&mut builder, document, block_idx, block);
    }
    builder.stats
}

pub(in crate::layout) fn push_block_stats(
    builder: &mut DocumentStatsBuilder,
    document: &Document,
    block_idx: usize,
    block: &Block,
) {
    match block {
        Block::Paragraph(paragraph) => push_paragraph_stats(builder, paragraph),
        Block::Table(table) => {
            for row in &table.rows {
                for cell in &row.cells {
                    if table_cell_is_merge_continuation(cell) {
                        continue;
                    }
                    for paragraph in &cell.paragraphs {
                        push_paragraph_stats(builder, paragraph);
                    }
                }
            }
        }
        Block::Placeholder(text) => {
            builder.push_text(text);
            builder.finish_text_boundary();
        }
        Block::SectionSettings(settings) => {
            let page_visibility = estimate_section_page_variant_visibility(document, block_idx);
            let section_content_width = content_width_for_page_settings(settings);
            push_repeating_paragraph_variant_stats(
                builder,
                &settings.header,
                &settings.first_page_header,
                &settings.even_page_header,
                &document.header,
                &document.first_page_header,
                &document.even_page_header,
                settings.title_page,
                settings.facing_pages,
                page_visibility,
            );
            push_repeating_image_variant_stats(
                builder,
                &settings.header_images,
                &settings.first_page_header_images,
                &settings.even_page_header_images,
                &document.header_images,
                &document.first_page_header_images,
                &document.even_page_header_images,
                settings.title_page,
                settings.facing_pages,
                page_visibility,
                section_content_width,
            );
            push_repeating_shape_variant_stats(
                builder,
                &settings.header_shapes,
                &settings.first_page_header_shapes,
                &settings.even_page_header_shapes,
                &document.header_shapes,
                &document.first_page_header_shapes,
                &document.even_page_header_shapes,
                settings.title_page,
                settings.facing_pages,
                page_visibility,
            );
            push_repeating_paragraph_variant_stats(
                builder,
                &settings.footer,
                &settings.first_page_footer,
                &settings.even_page_footer,
                &document.footer,
                &document.first_page_footer,
                &document.even_page_footer,
                settings.title_page,
                settings.facing_pages,
                page_visibility,
            );
            push_repeating_image_variant_stats(
                builder,
                &settings.footer_images,
                &settings.first_page_footer_images,
                &settings.even_page_footer_images,
                &document.footer_images,
                &document.first_page_footer_images,
                &document.even_page_footer_images,
                settings.title_page,
                settings.facing_pages,
                page_visibility,
                section_content_width,
            );
            push_repeating_shape_variant_stats(
                builder,
                &settings.footer_shapes,
                &settings.first_page_footer_shapes,
                &settings.even_page_footer_shapes,
                &document.footer_shapes,
                &document.first_page_footer_shapes,
                &document.even_page_footer_shapes,
                settings.title_page,
                settings.facing_pages,
                page_visibility,
            );
            push_background_shape_stats(
                builder,
                &settings.background_shapes,
                &document.background_shapes,
            );
        }
        Block::Image(image) => {
            let content_width =
                content_width_for_page_settings(page_settings_before_block(document, block_idx));
            push_image_stats(builder, image, content_width);
        }
        Block::Shape(shape) => {
            push_shape_stats(builder, shape);
        }
        Block::PageBreak
        | Block::ColumnBreak
        | Block::ContinuousSectionBreak
        | Block::SectionBreak
        | Block::EvenPageSectionBreak
        | Block::OddPageSectionBreak => builder.finish_text_boundary(),
    }
}

#[derive(Debug, Copy, Clone)]
pub(in crate::layout) struct PageVariantVisibility {
    pub(in crate::layout) first_page: usize,
    pub(in crate::layout) last_page: usize,
}

pub(in crate::layout) fn estimate_document_page_variant_visibility(
    document: &Document,
) -> PageVariantVisibility {
    let mut physical_page_number = 1usize;
    for (block_idx, block) in document.blocks.iter().enumerate() {
        if matches!(block, Block::SectionSettings(_))
            || (section_breaks_end_current_section(block)
                && next_block_is_section_settings(document, block_idx))
        {
            break;
        }
        physical_page_number = advance_physical_page_for_variant_estimate(
            physical_page_number,
            block,
            page_settings_before_block(document, block_idx).facing_pages,
        );
    }
    PageVariantVisibility {
        first_page: 1,
        last_page: physical_page_number.max(1),
    }
}

pub(in crate::layout) fn estimate_section_page_variant_visibility(
    document: &Document,
    settings_block_idx: usize,
) -> PageVariantVisibility {
    let section_start_page = physical_page_before_block(document, settings_block_idx);
    let mut section_last_page = section_start_page;
    for (block_idx, block) in document
        .blocks
        .iter()
        .enumerate()
        .skip(settings_block_idx + 1)
    {
        if section_breaks_end_current_section(block) {
            break;
        }
        section_last_page = advance_physical_page_for_variant_estimate(
            section_last_page,
            block,
            page_settings_before_block(document, block_idx).facing_pages,
        );
    }
    PageVariantVisibility {
        first_page: section_start_page.max(1),
        last_page: section_last_page.max(section_start_page.max(1)),
    }
}

pub(in crate::layout) fn physical_page_before_block(
    document: &Document,
    block_idx: usize,
) -> usize {
    let mut physical_page_number = 1usize;
    for (idx, block) in document.blocks.iter().enumerate().take(block_idx) {
        physical_page_number = advance_physical_page_for_variant_estimate(
            physical_page_number,
            block,
            page_settings_before_block(document, idx).facing_pages,
        );
    }
    physical_page_number
}

pub(in crate::layout) fn page_settings_before_block(
    document: &Document,
    block_idx: usize,
) -> &PageSettings {
    let mut settings = &document.page;
    for block in document.blocks.iter().take(block_idx) {
        if let Block::SectionSettings(section_settings) = block {
            settings = section_settings;
        }
    }
    settings
}

pub(in crate::layout) fn content_width_for_page_settings(settings: &PageSettings) -> f32 {
    PageGeometry::from_settings(
        settings,
        PageNumbering::from_page_settings(settings, 1),
        1,
        0,
    )
    .content_width
}

pub(in crate::layout) fn advance_physical_page_for_variant_estimate(
    physical_page_number: usize,
    block: &Block,
    facing_pages: bool,
) -> usize {
    match block {
        Block::PageBreak | Block::SectionBreak => physical_page_number.saturating_add(1),
        Block::EvenPageSectionBreak => {
            let next_page = physical_page_number.saturating_add(1);
            let target_is_even = facing_pages;
            if (next_page % 2 == 0) != target_is_even {
                next_page.saturating_add(1)
            } else {
                next_page
            }
        }
        Block::OddPageSectionBreak => {
            let next_page = physical_page_number.saturating_add(1);
            if next_page % 2 == 0 {
                next_page.saturating_add(1)
            } else {
                next_page
            }
        }
        Block::ColumnBreak
        | Block::ContinuousSectionBreak
        | Block::Paragraph(_)
        | Block::Table(_)
        | Block::Image(_)
        | Block::Shape(_)
        | Block::Placeholder(_)
        | Block::SectionSettings(_) => physical_page_number,
    }
}

pub(in crate::layout) fn section_breaks_end_current_section(block: &Block) -> bool {
    matches!(
        block,
        Block::ContinuousSectionBreak
            | Block::SectionBreak
            | Block::EvenPageSectionBreak
            | Block::OddPageSectionBreak
    )
}

pub(in crate::layout) fn push_repeating_paragraph_variant_stats(
    builder: &mut DocumentStatsBuilder,
    default: &[Paragraph],
    first: &[Paragraph],
    even: &[Paragraph],
    fallback_default: &[Paragraph],
    fallback_first: &[Paragraph],
    fallback_even: &[Paragraph],
    title_page: bool,
    facing_pages: bool,
    visibility: PageVariantVisibility,
) {
    for physical_page_number in visibility.first_page..=visibility.last_page {
        let is_first_section_page = title_page && physical_page_number == visibility.first_page;
        if is_first_section_page {
            let selected = first_non_empty(first, fallback_first);
            if !selected.is_empty() {
                builder.push_repeating_paragraphs_once(selected);
                continue;
            }
        }
        if facing_pages && physical_page_number % 2 == 0 {
            let selected = first_non_empty(even, fallback_even);
            if !selected.is_empty() {
                builder.push_repeating_paragraphs_once(selected);
                continue;
            }
        }
        builder.push_repeating_paragraphs_once(first_non_empty(default, fallback_default));
    }
}

pub(in crate::layout) fn push_repeating_shape_variant_stats(
    builder: &mut DocumentStatsBuilder,
    default: &[StaticShape],
    first: &[StaticShape],
    even: &[StaticShape],
    fallback_default: &[StaticShape],
    fallback_first: &[StaticShape],
    fallback_even: &[StaticShape],
    title_page: bool,
    facing_pages: bool,
    visibility: PageVariantVisibility,
) {
    for physical_page_number in visibility.first_page..=visibility.last_page {
        let is_first_section_page = title_page && physical_page_number == visibility.first_page;
        if is_first_section_page {
            let selected = first_non_empty_shapes(first, fallback_first);
            if !selected.is_empty() {
                builder.push_repeating_shapes_once(selected);
                continue;
            }
        }
        if facing_pages && physical_page_number % 2 == 0 {
            let selected = first_non_empty_shapes(even, fallback_even);
            if !selected.is_empty() {
                builder.push_repeating_shapes_once(selected);
                continue;
            }
        }
        builder.push_repeating_shapes_once(first_non_empty_shapes(default, fallback_default));
    }
}

pub(in crate::layout) fn push_repeating_image_variant_stats(
    builder: &mut DocumentStatsBuilder,
    default: &[StaticImage],
    first: &[StaticImage],
    even: &[StaticImage],
    fallback_default: &[StaticImage],
    fallback_first: &[StaticImage],
    fallback_even: &[StaticImage],
    title_page: bool,
    facing_pages: bool,
    visibility: PageVariantVisibility,
    content_width: f32,
) {
    for physical_page_number in visibility.first_page..=visibility.last_page {
        let is_first_section_page = title_page && physical_page_number == visibility.first_page;
        if is_first_section_page {
            let selected = first_non_empty_images(first, fallback_first);
            if !selected.is_empty() {
                builder.push_repeating_images_once(selected, content_width);
                continue;
            }
        }
        if facing_pages && physical_page_number % 2 == 0 {
            let selected = first_non_empty_images(even, fallback_even);
            if !selected.is_empty() {
                builder.push_repeating_images_once(selected, content_width);
                continue;
            }
        }
        builder.push_repeating_images_once(
            first_non_empty_images(default, fallback_default),
            content_width,
        );
    }
}

pub(in crate::layout) fn push_background_shape_stats(
    builder: &mut DocumentStatsBuilder,
    shapes: &[StaticShape],
    fallback_shapes: &[StaticShape],
) {
    builder.push_repeating_shapes_once(first_non_empty_shapes(shapes, fallback_shapes));
}

pub(in crate::layout) fn push_paragraphs_stats(
    builder: &mut DocumentStatsBuilder,
    paragraphs: &[Paragraph],
) {
    for paragraph in paragraphs {
        push_paragraph_stats(builder, paragraph);
    }
}

pub(in crate::layout) fn push_shapes_stats(
    builder: &mut DocumentStatsBuilder,
    shapes: &[StaticShape],
) {
    for shape in shapes {
        push_shape_stats(builder, shape);
    }
}

pub(in crate::layout) fn push_images_stats(
    builder: &mut DocumentStatsBuilder,
    images: &[StaticImage],
    content_width: f32,
) {
    for image in images {
        push_image_stats(builder, image, content_width);
    }
}

pub(in crate::layout) fn push_image_stats(
    builder: &mut DocumentStatsBuilder,
    image: &StaticImage,
    content_width: f32,
) {
    if image_placeholder_label_is_visible(image, content_width) {
        builder.push_text("Image skipped");
        builder.finish_text_boundary();
    }
    if image.format == ImageFormat::WmfVector {
        for command in &image.vector_commands {
            if let StaticImageVectorCommand::Text { text, .. } = command {
                builder.push_text(text);
                builder.finish_text_boundary();
            }
        }
    }
}

pub(in crate::layout) fn image_placeholder_label_is_visible(
    image: &StaticImage,
    content_width: f32,
) -> bool {
    if image.format != ImageFormat::Placeholder {
        return false;
    }
    let (_, _, width, height) = image_layout_frame(image, content_width);
    width >= 74.0 && height >= 24.0
}

pub(in crate::layout) fn push_shape_stats(builder: &mut DocumentStatsBuilder, shape: &StaticShape) {
    for paragraph in &shape.text {
        push_paragraph_stats(builder, paragraph);
    }
    builder.finish_text_boundary();
}

pub(in crate::layout) fn table_cell_is_merge_continuation(cell: &TableCell) -> bool {
    cell.horizontal_merge == TableCellHorizontalMerge::Continuation
        || cell.vertical_merge == TableCellVerticalMerge::Continuation
}

pub(in crate::layout) fn push_paragraph_stats(
    builder: &mut DocumentStatsBuilder,
    paragraph: &Paragraph,
) {
    for run in &paragraph.runs {
        if !run.style.hidden && run.dynamic_field.is_none() {
            builder.push_text(&run.text);
        }
    }
    builder.finish_text_boundary();
}

pub(in crate::layout) fn remove_internal_stat_markers(text: &str) -> String {
    let mut cleaned = text
        .replace(PASSIVE_ADVANCE_MARKER, "")
        .replace(PASSIVE_MATH_FRACTION_RULE_MARKER, "")
        .replace(PASSIVE_MATH_INLINE_LEAD_MARKER, "")
        .replace(PASSIVE_MATH_STACK_ANCHOR_MARKER, "");
    while let Some((start, end)) = next_bookmark_page_marker_range(&cleaned, 0) {
        cleaned.replace_range(start..end, "");
    }
    while let Some((start, end)) = next_footnote_reference_marker_range(&cleaned, 0) {
        cleaned.replace_range(start..end, "");
    }
    while let Some((start, end)) = next_endnote_reference_marker_range(&cleaned, 0) {
        cleaned.replace_range(start..end, "");
    }
    cleaned
}

pub(in crate::layout) fn format_page_number(value: usize, format: PageNumberFormat) -> String {
    let value = value.min(i32::MAX as usize) as i32;
    match format {
        PageNumberFormat::Decimal => value.to_string(),
        PageNumberFormat::UpperRoman => format_roman_page_number(value, false),
        PageNumberFormat::LowerRoman => format_roman_page_number(value, true),
        PageNumberFormat::UpperLetter => format_alpha_page_number(value, false),
        PageNumberFormat::LowerLetter => format_alpha_page_number(value, true),
    }
}

pub(in crate::layout) fn format_note_number(
    start: i32,
    sequence: usize,
    format: PageNumberFormat,
) -> String {
    let start = start.max(1) as usize;
    format_page_number(start.saturating_add(sequence.saturating_sub(1)), format)
}

pub(in crate::layout) fn format_roman_page_number(value: i32, lowercase: bool) -> String {
    if !(1..=3999).contains(&value) {
        return value.to_string();
    }
    let mut remaining = value;
    let mut output = String::new();
    for (unit, symbol) in [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ] {
        while remaining >= unit {
            output.push_str(symbol);
            remaining -= unit;
        }
    }
    if lowercase {
        output.to_ascii_lowercase()
    } else {
        output
    }
}

pub(in crate::layout) fn format_alpha_page_number(value: i32, lowercase: bool) -> String {
    if value <= 0 {
        return value.to_string();
    }
    let mut value = value as u32;
    let mut chars = Vec::new();
    while value > 0 {
        value -= 1;
        let ch = (b'A' + (value % 26) as u8) as char;
        chars.push(if lowercase {
            ch.to_ascii_lowercase()
        } else {
            ch
        });
        value /= 26;
    }
    chars.iter().rev().collect()
}

pub(in crate::layout) fn resolve_total_page_markers(pages: &mut [LayoutPage]) {
    let total_pages = pages.len().to_string();
    for page in pages {
        replace_late_page_marker(page, DynamicField::TotalPages, &total_pages);
    }
}

pub(in crate::layout) fn resolve_section_page_markers(pages: &mut [LayoutPage]) {
    let max_section = pages
        .iter()
        .map(|page| page.section_number)
        .max()
        .unwrap_or(0);
    let mut section_page_counts = vec![0usize; max_section.saturating_add(1)];
    for page in pages.iter() {
        if let Some(count) = section_page_counts.get_mut(page.section_number) {
            *count = count.saturating_add(1);
        }
    }

    for page in pages {
        let section_pages = section_page_counts
            .get(page.section_number)
            .copied()
            .unwrap_or(1)
            .max(1)
            .to_string();
        replace_late_page_marker(page, DynamicField::SectionPages, &section_pages);
    }
}

pub(in crate::layout) fn resolve_footnote_reference_markers(
    pages: &mut [LayoutPage],
    document: &Document,
    font_provider: Option<&FontProvider>,
) {
    let reference_pages = footnote_reference_pages(pages, document.footnotes.len());
    for (page_idx, page) in pages.iter_mut().enumerate() {
        let current_page = page_idx.saturating_add(1);
        let mut shifts = Vec::<(usize, f32, f32, f32)>::new();
        for (item_idx, item) in page.items.iter_mut().enumerate() {
            let Some(fragment) = layout_item_text_fragment_mut(item) else {
                continue;
            };
            if !fragment.text.contains(FOOTNOTE_REFERENCE_MARKER) {
                continue;
            }
            let old_text = fragment.text.clone();
            let new_text = replace_footnote_reference_markers_in_text(
                &old_text,
                document,
                &reference_pages,
                current_page,
            );
            if new_text == old_text {
                continue;
            }
            let old_width = measure_text_with_document_font(
                &old_text,
                &fragment.style,
                fragment.font_family,
                document,
                font_provider,
            );
            let new_width = measure_text_with_document_font(
                &new_text,
                &fragment.style,
                fragment.font_family,
                document,
                font_provider,
            );
            fragment.text = new_text;
            let delta = new_width - old_width;
            if delta.abs() > 0.01 {
                shifts.push((item_idx, fragment.baseline_y, fragment.x, delta));
            }
        }
        apply_late_inline_marker_shifts(page, shifts);
    }
}

pub(in crate::layout) fn resolve_endnote_reference_markers(
    pages: &mut [LayoutPage],
    document: &Document,
    rendered_endnote_pages: &[Option<usize>],
    font_provider: Option<&FontProvider>,
) {
    for page in pages.iter_mut() {
        let mut shifts = Vec::<(usize, f32, f32, f32)>::new();
        for (item_idx, item) in page.items.iter_mut().enumerate() {
            let Some(fragment) = layout_item_text_fragment_mut(item) else {
                continue;
            };
            if !fragment.text.contains(ENDNOTE_REFERENCE_MARKER) {
                continue;
            }
            let old_text = fragment.text.clone();
            let new_text = replace_endnote_reference_markers_in_text(
                &old_text,
                document,
                rendered_endnote_pages,
            );
            if new_text == old_text {
                continue;
            }
            let old_width = measure_text_with_document_font(
                &old_text,
                &fragment.style,
                fragment.font_family,
                document,
                font_provider,
            );
            let new_width = measure_text_with_document_font(
                &new_text,
                &fragment.style,
                fragment.font_family,
                document,
                font_provider,
            );
            fragment.text = new_text;
            let delta = new_width - old_width;
            if delta.abs() > 0.01 {
                shifts.push((item_idx, fragment.baseline_y, fragment.x, delta));
            }
        }
        apply_late_inline_marker_shifts(page, shifts);
    }
}

pub(in crate::layout) fn apply_late_inline_marker_shifts(
    page: &mut LayoutPage,
    shifts: Vec<(usize, f32, f32, f32)>,
) {
    let mut applied = Vec::<(f32, f32, f32)>::new();
    for (marker_idx, baseline_y, original_marker_x, delta) in shifts {
        let prior_delta = applied
            .iter()
            .filter(|(prior_baseline, prior_x, _)| {
                (*prior_baseline - baseline_y).abs() < 0.01 && *prior_x <= original_marker_x
            })
            .map(|(_, _, prior_delta)| *prior_delta)
            .sum::<f32>();
        let marker_x = original_marker_x + prior_delta;
        for item in page.items.iter_mut().skip(marker_idx.saturating_add(1)) {
            if let Some(fragment) = layout_item_text_fragment_mut(item)
                && (fragment.baseline_y - baseline_y).abs() < 0.01
                && fragment.x >= marker_x
            {
                fragment.x += delta;
            }
        }
        applied.push((baseline_y, original_marker_x, delta));
    }
}

pub(in crate::layout) fn footnote_reference_pages(
    pages: &[LayoutPage],
    footnote_count: usize,
) -> Vec<Option<usize>> {
    let mut reference_pages = vec![None; footnote_count];
    for (page_idx, page) in pages.iter().enumerate() {
        let page_number = page_idx.saturating_add(1);
        for item in &page.items {
            let Some(fragment) = layout_item_text_fragment(item) else {
                continue;
            };
            let mut search_start = 0;
            while let Some((start, end)) =
                next_footnote_reference_marker_range(&fragment.text, search_start)
            {
                if let Some(index) = parse_footnote_reference_marker_id(&fragment.text[start..end])
                    && let Some(reference_page) = reference_pages.get_mut(index.get())
                    && reference_page.is_none()
                {
                    *reference_page = Some(page_number);
                }
                search_start = end;
            }
        }
    }
    reference_pages
}

pub(in crate::layout) fn replace_endnote_reference_markers_in_text(
    text: &str,
    document: &Document,
    rendered_endnote_pages: &[Option<usize>],
) -> String {
    let mut output = String::new();
    let mut search_start = 0;
    while let Some((start, end)) = next_endnote_reference_marker_range(text, search_start) {
        output.push_str(&text[search_start..start]);
        let replacement = parse_endnote_reference_marker_id(&text[start..end])
            .map(|id| {
                let index = id.get();
                let numbering = endnote_numbering_for(document, index);
                let note_page = rendered_endnote_pages
                    .get(index)
                    .copied()
                    .flatten()
                    .unwrap_or(1);
                let sequence = endnote_sequence_for_rendered_page(
                    document,
                    index,
                    rendered_endnote_pages,
                    note_page,
                );
                format_note_number(numbering.start, sequence, numbering.format)
            })
            .unwrap_or_else(|| "?".to_string());
        output.push_str(&replacement);
        search_start = end;
    }
    output.push_str(&text[search_start..]);
    output
}

pub(in crate::layout) fn replace_footnote_reference_markers_in_text(
    text: &str,
    document: &Document,
    reference_pages: &[Option<usize>],
    current_page: usize,
) -> String {
    let mut output = String::with_capacity(text.len());
    let mut search_start = 0;
    while let Some((start, end)) = next_footnote_reference_marker_range(text, search_start) {
        output.push_str(&text[search_start..start]);
        let replacement = parse_footnote_reference_marker_id(&text[start..end])
            .map(|id| {
                let index = id.get();
                let numbering = footnote_numbering_for(document, index);
                let sequence = footnote_sequence_for_reference_page(
                    document,
                    index,
                    reference_pages,
                    current_page,
                );
                format_note_number(numbering.start, sequence, numbering.format)
            })
            .unwrap_or_else(|| "?".to_string());
        output.push_str(&replacement);
        search_start = end;
    }
    output.push_str(&text[search_start..]);
    output
}

pub(in crate::layout) fn footnote_sequence_for_reference_page(
    document: &Document,
    index: usize,
    reference_pages: &[Option<usize>],
    current_page: usize,
) -> usize {
    match footnote_numbering_for(document, index).restart {
        NoteNumberRestart::Continuous => index.saturating_add(1),
        NoteNumberRestart::EachSection => footnote_section_sequence(document, index),
        NoteNumberRestart::EachPage => reference_pages
            .iter()
            .enumerate()
            .take(index.saturating_add(1))
            .filter(|(candidate_idx, reference_page)| {
                *candidate_idx == index || reference_page.is_some_and(|page| page == current_page)
            })
            .count()
            .max(1),
    }
}

#[derive(Debug, Copy, Clone)]
pub(in crate::layout) struct LateMarkerWidthAdjustment {
    pub(in crate::layout) text_index: usize,
    pub(in crate::layout) old_width: f32,
    pub(in crate::layout) new_width: f32,
}

#[derive(Debug, Copy, Clone)]
pub(in crate::layout) struct LateMarkerGeometry {
    pub(in crate::layout) x: f32,
    pub(in crate::layout) baseline_y: f32,
    pub(in crate::layout) old_width: f32,
    pub(in crate::layout) new_width: f32,
    pub(in crate::layout) font_size: f32,
}

pub(in crate::layout) fn replace_late_page_marker(
    page: &mut LayoutPage,
    field: DynamicField,
    replacement: &str,
) {
    let mut adjustments = Vec::new();
    for (idx, item) in page.items.iter_mut().enumerate() {
        let Some(fragment) = layout_item_text_fragment_mut(item) else {
            continue;
        };
        if fragment.dynamic_field != Some(field) {
            continue;
        }
        let old_text = fragment.text.clone();
        let new_text = replacement.to_owned();
        let old_width = passive_text_fragment_width(fragment, &old_text);
        let new_width = passive_text_fragment_width(fragment, &new_text);
        fragment.text = new_text;
        fragment.dynamic_field = None;
        if (new_width - old_width).abs() > 0.01 {
            adjustments.push(LateMarkerWidthAdjustment {
                text_index: idx,
                old_width,
                new_width,
            });
        }
    }
    for adjustment in adjustments {
        let Some(geometry) = late_marker_geometry(page, adjustment) else {
            continue;
        };
        adjust_late_marker_decorations(page, adjustment.text_index, geometry);
        shift_late_marker_following_items(page, adjustment.text_index, geometry);
    }
}

pub(in crate::layout) fn passive_text_fragment_width(fragment: &TextFragment, text: &str) -> f32 {
    measure_text_with_family(text, &fragment.style, fragment.font_family)
        + fragment.word_spacing * regular_space_count(text) as f32
}

pub(in crate::layout) fn late_marker_geometry(
    page: &LayoutPage,
    adjustment: LateMarkerWidthAdjustment,
) -> Option<LateMarkerGeometry> {
    let fragment = layout_item_text_fragment(page.items.get(adjustment.text_index)?)?;
    Some(LateMarkerGeometry {
        x: fragment.x,
        baseline_y: fragment.baseline_y,
        old_width: adjustment.old_width,
        new_width: adjustment.new_width,
        font_size: fragment.style.font_size_points(),
    })
}

pub(in crate::layout) fn adjust_late_marker_decorations(
    page: &mut LayoutPage,
    text_index: usize,
    geometry: LateMarkerGeometry,
) {
    let start = text_index.saturating_sub(3);
    let end = text_index
        .saturating_add(6)
        .min(page.items.len().saturating_sub(1));
    for idx in start..=end {
        if idx == text_index {
            continue;
        }
        match &mut page.items[idx] {
            LayoutItem::Highlight { x, width, .. }
                if same_pdf_coord(*x, geometry.x)
                    && same_pdf_length(*width, geometry.old_width) =>
            {
                *width = geometry.new_width;
            }
            LayoutItem::Highlight {
                x,
                y,
                width,
                height,
                ..
            } if *x < geometry.x
                && *x + *width > geometry.x + geometry.old_width
                && geometry.baseline_y >= *y - 0.01
                && geometry.baseline_y <= *y + *height + 0.01 =>
            {
                *width += geometry.new_width - geometry.old_width;
            }
            LayoutItem::Highlight {
                x,
                y,
                width,
                height,
                ..
            } if *x >= geometry.x + geometry.old_width - 0.01
                && *width <= geometry.font_size + 0.01
                && geometry.baseline_y >= *y - 0.01
                && geometry.baseline_y <= *y + *height + 0.01 =>
            {
                *x += geometry.new_width - geometry.old_width;
            }
            LayoutItem::Underline { x, width, .. }
                if same_pdf_coord(*x, geometry.x)
                    && same_pdf_length(*width, geometry.old_width) =>
            {
                *width = geometry.new_width;
            }
            LayoutItem::Line { x1, y1, x2, y2, .. }
                if same_pdf_coord(*x1, geometry.x)
                    && same_pdf_coord(*x2, geometry.x + geometry.old_width)
                    && same_pdf_coord(*y1, *y2)
                    && (*y1 - geometry.baseline_y).abs() <= geometry.font_size =>
            {
                *x2 = geometry.x + geometry.new_width;
            }
            LayoutItem::Line { x1, y1, x2, y2, .. }
                if character_border_horizontal_line_matches(*x1, *y1, *x2, *y2, geometry) =>
            {
                adjust_character_border_horizontal_line(x1, x2, geometry);
            }
            LayoutItem::Line { x1, y1, x2, y2, .. }
                if character_border_right_line_matches(*x1, *y1, *x2, *y2, geometry) =>
            {
                let delta = geometry.new_width - geometry.old_width;
                *x1 += delta;
                *x2 += delta;
            }
            _ => {}
        }
    }
}

pub(in crate::layout) fn character_border_horizontal_line_matches(
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    geometry: LateMarkerGeometry,
) -> bool {
    if !same_pdf_coord(y1, y2) || (y1 - geometry.baseline_y).abs() > geometry.font_size * 1.5 {
        return false;
    }
    let left = x1.min(x2);
    let right = x1.max(x2);
    if left > geometry.x + 0.01 || right < geometry.x + geometry.old_width - 0.01 {
        return false;
    }
    let left_pad = geometry.x - left;
    let right_pad = right - (geometry.x + geometry.old_width);
    left_pad >= -0.01 && right_pad >= -0.01 && (left_pad - right_pad).abs() < 0.05
}

pub(in crate::layout) fn adjust_character_border_horizontal_line(
    x1: &mut f32,
    x2: &mut f32,
    geometry: LateMarkerGeometry,
) {
    let delta = geometry.new_width - geometry.old_width;
    if *x1 >= *x2 {
        *x1 += delta;
    } else {
        *x2 += delta;
    }
}

pub(in crate::layout) fn character_border_right_line_matches(
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    geometry: LateMarkerGeometry,
) -> bool {
    if !same_pdf_coord(x1, x2) || x1 <= geometry.x + geometry.old_width {
        return false;
    }
    let pad = x1 - (geometry.x + geometry.old_width);
    let baseline_between =
        geometry.baseline_y >= y1.min(y2) - 0.01 && geometry.baseline_y <= y1.max(y2) + 0.01;
    baseline_between && pad >= 0.0 && pad <= geometry.font_size
}

pub(in crate::layout) fn shift_late_marker_following_items(
    page: &mut LayoutPage,
    text_index: usize,
    geometry: LateMarkerGeometry,
) {
    let delta = geometry.new_width - geometry.old_width;
    if delta.abs() <= 0.01 {
        return;
    }
    let old_end = geometry.x + geometry.old_width;
    let mut saw_following_line_item = false;
    for item in page.items.iter_mut().skip(text_index.saturating_add(1)) {
        if item_starts_after_late_marker_on_line(item, old_end, geometry) {
            shift_layout_item_x(item, delta);
            saw_following_line_item = true;
        } else if saw_following_line_item && starts_new_text_line(item, geometry) {
            break;
        }
    }
}

pub(in crate::layout) fn item_starts_after_late_marker_on_line(
    item: &LayoutItem,
    old_end: f32,
    geometry: LateMarkerGeometry,
) -> bool {
    match item {
        LayoutItem::Text(fragment) => {
            same_late_marker_line(fragment.baseline_y, geometry) && fragment.x >= old_end - 0.01
        }
        LayoutItem::Highlight { x, y, height, .. } => {
            *x >= old_end - 0.01
                && geometry.baseline_y >= *y - 0.01
                && geometry.baseline_y <= *y + *height + 0.01
        }
        LayoutItem::Underline { x, y, .. } => {
            *x >= old_end - 0.01 && (*y - geometry.baseline_y).abs() <= geometry.font_size
        }
        LayoutItem::Line { x1, y1, x2, y2, .. } => {
            x1.min(*x2) >= old_end - 0.01
                && same_pdf_coord(*y1, *y2)
                && (*y1 - geometry.baseline_y).abs() <= geometry.font_size
        }
        LayoutItem::Drawing(drawing) => {
            let mut current = drawing.item.as_ref();
            loop {
                match current {
                    LayoutItem::Drawing(nested) => current = nested.item.as_ref(),
                    leaf => return item_starts_after_late_marker_on_line(leaf, old_end, geometry),
                }
            }
        }
        _ => false,
    }
}

pub(in crate::layout) fn same_late_marker_line(
    baseline_y: f32,
    geometry: LateMarkerGeometry,
) -> bool {
    (baseline_y - geometry.baseline_y).abs() < 0.01
}

pub(in crate::layout) fn starts_new_text_line(
    item: &LayoutItem,
    geometry: LateMarkerGeometry,
) -> bool {
    match item {
        LayoutItem::Text(fragment) if !same_late_marker_line(fragment.baseline_y, geometry) => true,
        LayoutItem::Drawing(drawing) => {
            let mut current = drawing.item.as_ref();
            loop {
                match current {
                    LayoutItem::Drawing(nested) => current = nested.item.as_ref(),
                    LayoutItem::ClipRect { item, .. } => current = item.as_ref(),
                    leaf => return starts_new_text_line(leaf, geometry),
                }
            }
        }
        LayoutItem::ClipRect { item, .. } => starts_new_text_line(item, geometry),
        _ => false,
    }
}

pub(in crate::layout) fn shift_layout_item_x(item: &mut LayoutItem, delta: f32) {
    let mut current = item;
    loop {
        match current {
            LayoutItem::Text(fragment) => fragment.x += delta,
            LayoutItem::Highlight { x, .. } | LayoutItem::Underline { x, .. } => *x += delta,
            LayoutItem::Line { x1, x2, .. } => {
                *x1 += delta;
                *x2 += delta;
            }
            LayoutItem::Drawing(drawing) => {
                current = drawing.item.as_mut();
                continue;
            }
            LayoutItem::ClipRect { x, item, .. } => {
                *x += delta;
                current = item.as_mut();
                continue;
            }
            _ => {}
        }
        break;
    }
}

pub(in crate::layout) fn layout_item_text_fragment(item: &LayoutItem) -> Option<&TextFragment> {
    let mut current = item;
    loop {
        match current {
            LayoutItem::Text(fragment) => return Some(fragment),
            LayoutItem::Drawing(drawing) => current = drawing.item.as_ref(),
            LayoutItem::ClipRect { item, .. } => current = item.as_ref(),
            _ => return None,
        }
    }
}

pub(in crate::layout) fn layout_item_text_fragment_mut(
    item: &mut LayoutItem,
) -> Option<&mut TextFragment> {
    let mut current = item;
    loop {
        match current {
            LayoutItem::Text(fragment) => return Some(fragment),
            LayoutItem::Drawing(drawing) => current = drawing.item.as_mut(),
            LayoutItem::ClipRect { item, .. } => current = item.as_mut(),
            _ => return None,
        }
    }
}

pub(in crate::layout) fn same_pdf_coord(left: f32, right: f32) -> bool {
    (left - right).abs() < 0.01
}

pub(in crate::layout) fn same_pdf_length(left: f32, right: f32) -> bool {
    (left - right).abs() < 0.01
}

pub(in crate::layout) fn resolve_bookmark_page_ref_markers(
    pages: &mut [LayoutPage],
    document: &Document,
    font_provider: Option<&FontProvider>,
) {
    let mut page_for_bookmark = Vec::<(usize, String)>::new();
    for page in pages.iter() {
        for item in &page.items {
            if let Some(fragment) = layout_item_text_fragment(item)
                && let Some(id) =
                    parse_bookmark_page_marker_id(&fragment.text, BOOKMARK_PAGE_ANCHOR_MARKER)
            {
                if let Some((_, page_number)) = page_for_bookmark
                    .iter_mut()
                    .find(|(existing_id, _)| *existing_id == id)
                {
                    *page_number = page.display_page_number.clone();
                } else {
                    page_for_bookmark.push((id, page.display_page_number.clone()));
                }
            }
        }
    }

    for page in pages {
        let mut shifts = Vec::<(usize, f32, f32, f32)>::new();
        for (item_idx, item) in page.items.iter_mut().enumerate() {
            if let Some(fragment) = layout_item_text_fragment_mut(item) {
                if parse_bookmark_page_marker_id(&fragment.text, BOOKMARK_PAGE_ANCHOR_MARKER)
                    .is_some()
                {
                    fragment.text.clear();
                } else if let Some(id) =
                    parse_bookmark_page_marker_id(&fragment.text, BOOKMARK_PAGE_REF_MARKER)
                {
                    fragment.text = page_for_bookmark
                        .iter()
                        .find_map(|(bookmark_id, page_number)| {
                            (*bookmark_id == id).then(|| page_number.clone())
                        })
                        .unwrap_or_else(|| "?".to_string());
                    let width = measure_text_with_document_font(
                        &fragment.text,
                        &fragment.style,
                        fragment.font_family,
                        document,
                        font_provider,
                    );
                    shifts.push((item_idx, fragment.baseline_y, fragment.x, width));
                }
            }
        }
        for (marker_idx, baseline_y, marker_x, delta) in shifts {
            if delta <= 0.0 {
                continue;
            }
            for item in page.items.iter_mut().skip(marker_idx.saturating_add(1)) {
                if let Some(fragment) = layout_item_text_fragment_mut(item)
                    && (fragment.baseline_y - baseline_y).abs() < 0.01
                    && fragment.x >= marker_x
                {
                    fragment.x += delta;
                }
            }
        }
    }
}

pub(in crate::layout) fn next_bookmark_page_marker_range(
    text: &str,
    start: usize,
) -> Option<(usize, usize)> {
    let anchor = text[start..]
        .find(BOOKMARK_PAGE_ANCHOR_MARKER)
        .map(|relative| start + relative);
    let page_ref = text[start..]
        .find(BOOKMARK_PAGE_REF_MARKER)
        .map(|relative| start + relative);
    let marker_start = match (anchor, page_ref) {
        (Some(anchor), Some(page_ref)) => anchor.min(page_ref),
        (Some(anchor), None) => anchor,
        (None, Some(page_ref)) => page_ref,
        (None, None) => return None,
    };
    let prefix = if text[marker_start..].starts_with(BOOKMARK_PAGE_ANCHOR_MARKER) {
        BOOKMARK_PAGE_ANCHOR_MARKER
    } else {
        BOOKMARK_PAGE_REF_MARKER
    };
    let id_start = marker_start + prefix.len();
    let marker_end_relative = text[id_start..].find(BOOKMARK_PAGE_MARKER_END)?;
    let marker_end = id_start + marker_end_relative + BOOKMARK_PAGE_MARKER_END.len();
    parse_bookmark_page_marker_id(&text[marker_start..marker_end], prefix)?;
    Some((marker_start, marker_end))
}

pub(in crate::layout) fn next_footnote_reference_marker_range(
    text: &str,
    start: usize,
) -> Option<(usize, usize)> {
    let marker_start = text[start..]
        .find(FOOTNOTE_REFERENCE_MARKER)
        .map(|relative| start + relative)?;
    let id_start = marker_start + FOOTNOTE_REFERENCE_MARKER.len();
    let marker_end_relative = text[id_start..].find(FOOTNOTE_REFERENCE_MARKER_END)?;
    let marker_end = id_start + marker_end_relative + FOOTNOTE_REFERENCE_MARKER_END.len();
    parse_footnote_reference_marker_id(&text[marker_start..marker_end])?;
    Some((marker_start, marker_end))
}

pub(in crate::layout) fn next_endnote_reference_marker_range(
    text: &str,
    start: usize,
) -> Option<(usize, usize)> {
    let marker_start = text[start..]
        .find(ENDNOTE_REFERENCE_MARKER)
        .map(|relative| start + relative)?;
    let id_start = marker_start + ENDNOTE_REFERENCE_MARKER.len();
    let marker_end_relative = text[id_start..].find(ENDNOTE_REFERENCE_MARKER_END)?;
    let marker_end = id_start + marker_end_relative + ENDNOTE_REFERENCE_MARKER_END.len();
    parse_endnote_reference_marker_id(&text[marker_start..marker_end])?;
    Some((marker_start, marker_end))
}

pub(in crate::layout) fn parse_bookmark_page_marker_id(text: &str, prefix: &str) -> Option<usize> {
    let rest = text.strip_prefix(prefix)?;
    let id = rest.strip_suffix(BOOKMARK_PAGE_MARKER_END)?;
    (!id.is_empty() && id.chars().all(|ch| ch.is_ascii_digit()))
        .then(|| id.parse::<usize>().ok())?
}

pub(in crate::layout) fn parse_footnote_reference_marker_id(text: &str) -> Option<FootnoteId> {
    let rest = text.strip_prefix(FOOTNOTE_REFERENCE_MARKER)?;
    let id = rest.strip_suffix(FOOTNOTE_REFERENCE_MARKER_END)?;
    (!id.is_empty() && id.chars().all(|ch| ch.is_ascii_digit()))
        .then(|| id.parse::<usize>().ok().map(FootnoteId::new))?
}

pub(in crate::layout) fn parse_endnote_reference_marker_id(text: &str) -> Option<EndnoteId> {
    let rest = text.strip_prefix(ENDNOTE_REFERENCE_MARKER)?;
    let id = rest.strip_suffix(ENDNOTE_REFERENCE_MARKER_END)?;
    (!id.is_empty() && id.chars().all(|ch| ch.is_ascii_digit()))
        .then(|| id.parse::<usize>().ok().map(EndnoteId::new))?
}
