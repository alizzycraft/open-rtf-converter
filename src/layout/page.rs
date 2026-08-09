//! Page and column advancement, repeating content, borders, and backgrounds.

use super::*;

pub(in crate::layout) fn start_new_page(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    geometry: &mut PageGeometry,
    current_column: &mut usize,
) {
    let physical_page_number = pages.len() + 1;
    let page_geometry = geometry.for_physical_page(physical_page_number);
    pages.push(new_layout_page(page_geometry, physical_page_number));
    *geometry = page_geometry;
    *current_column = 0;
    *cursor_y = page_geometry.height - page_geometry.margin_top;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(in crate::layout) enum PageParity {
    Even,
    Odd,
}

impl PageParity {
    fn matches(self, page_number: usize) -> bool {
        match self {
            Self::Even => page_number % 2 == 0,
            Self::Odd => page_number % 2 == 1,
        }
    }
}

pub(in crate::layout) fn start_new_page_with_parity(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    geometry: &mut PageGeometry,
    current_column: &mut usize,
    target: PageParity,
) {
    start_new_page(pages, cursor_y, geometry, current_column);
    if !target.matches(pages.len()) {
        start_new_page(pages, cursor_y, geometry, current_column);
    }
}

pub(in crate::layout) fn advance_column_or_page(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    geometry: &mut PageGeometry,
    current_column: &mut usize,
) {
    if *current_column + 1 < geometry.column_count {
        *current_column += 1;
        *cursor_y = geometry.height - geometry.margin_top;
    } else {
        start_new_page(pages, cursor_y, geometry, current_column);
    }
}

pub(in crate::layout) fn start_new_page_for_kept_paragraphs(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    paragraph: &Paragraph,
    following_paragraphs: &[&Paragraph],
    content_width: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    current_column: &mut usize,
    document: &Document,
    markers: &MarkerContext,
    font_provider: Option<&FontProvider>,
) {
    if !paragraph.style.keep_together && following_paragraphs.is_empty() {
        return;
    }
    if pages.last().is_none_or(|page| page.items.is_empty()) {
        return;
    }

    let kept_height = paragraph_block_height(
        paragraph,
        content_width,
        markers,
        document,
        *geometry,
        font_provider,
    ) + following_paragraphs
        .iter()
        .map(|paragraph| {
            paragraph_block_height(
                paragraph,
                content_width,
                markers,
                document,
                *geometry,
                font_provider,
            )
        })
        .sum::<f32>();
    let usable_height = (geometry.height - geometry.margin_top - margin_bottom).max(0.0);
    if kept_height <= usable_height && *cursor_y - kept_height < margin_bottom {
        advance_column_or_page(pages, cursor_y, geometry, current_column);
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn kept_paragraphs_would_advance_to_page(
    pages: &[LayoutPage],
    cursor_y: &f32,
    paragraph: &Paragraph,
    following_paragraphs: &[&Paragraph],
    content_width: f32,
    margin_bottom: f32,
    geometry: &PageGeometry,
    current_column: &usize,
    document: &Document,
    markers: &MarkerContext,
    font_provider: Option<&FontProvider>,
) -> bool {
    if !paragraph.style.keep_together && following_paragraphs.is_empty() {
        return false;
    }
    if pages.last().is_none_or(|page| page.items.is_empty()) {
        return false;
    }

    let kept_height = paragraph_block_height(
        paragraph,
        content_width,
        markers,
        document,
        *geometry,
        font_provider,
    ) + following_paragraphs
        .iter()
        .map(|paragraph| {
            paragraph_block_height(
                paragraph,
                content_width,
                markers,
                document,
                *geometry,
                font_provider,
            )
        })
        .sum::<f32>();
    let usable_height = (geometry.height - geometry.margin_top - margin_bottom).max(0.0);
    kept_height <= usable_height
        && *cursor_y - kept_height < margin_bottom
        && *current_column + 1 >= geometry.column_count
}

pub(in crate::layout) fn paragraph_block_height(
    paragraph: &Paragraph,
    content_width: f32,
    markers: &MarkerContext,
    document: &Document,
    geometry: PageGeometry,
    font_provider: Option<&FontProvider>,
) -> f32 {
    let line_height = wrap_paragraph_with_font_provider(
        paragraph,
        content_width,
        markers,
        document,
        font_provider,
    )
    .into_iter()
    .map(|line| apply_line_spacing_with_grid(line.height, &paragraph.style, geometry))
    .sum::<f32>();
    twips_to_points(effective_space_before_twips(&paragraph.style))
        + paragraph_top_border_reserve(&paragraph.style, document)
        + line_height
        + paragraph_bottom_border_reserve(&paragraph.style, false, document)
        + twips_to_points(effective_space_after_twips(&paragraph.style))
}

pub(in crate::layout) fn next_paragraph_block(blocks: &[Block], idx: usize) -> Option<&Paragraph> {
    blocks.get(idx + 1).and_then(|block| match block {
        Block::Paragraph(paragraph) => Some(paragraph),
        _ => None,
    })
}

pub(in crate::layout) fn keep_with_next_chain(blocks: &[Block], idx: usize) -> Vec<&Paragraph> {
    let mut chain = Vec::new();
    let mut current_idx = idx;
    while blocks.get(current_idx).is_some_and(
        |block| matches!(block, Block::Paragraph(paragraph) if paragraph.style.keep_with_next),
    ) {
        let Some(next_idx) = current_idx.checked_add(1) else {
            break;
        };
        let Some(Block::Paragraph(next)) = blocks.get(next_idx) else {
            break;
        };
        chain.push(next);
        current_idx = next_idx;
    }
    chain
}

pub(in crate::layout) fn previous_paragraph_block(
    blocks: &[Block],
    idx: usize,
) -> Option<&Paragraph> {
    idx.checked_sub(1)
        .and_then(|previous_idx| blocks.get(previous_idx))
        .and_then(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
}

pub(in crate::layout) fn previous_non_shape_block_is_paragraph(
    blocks: &[Block],
    idx: usize,
) -> bool {
    blocks[..idx]
        .iter()
        .rev()
        .find(|block| !matches!(block, Block::Shape(_)))
        .is_some_and(|block| matches!(block, Block::Paragraph(_)))
}

pub(in crate::layout) fn paragraph_spacing_is_contextual(
    first: &Paragraph,
    second: &Paragraph,
) -> bool {
    first.style.contextual_spacing || second.style.contextual_spacing
}

pub(in crate::layout) fn paragraph_between_border_is_visible(
    first: &Paragraph,
    second: &Paragraph,
) -> bool {
    first.style.borders.between.visible && second.style.borders.between.visible
}

pub(in crate::layout) fn apply_page_vertical_alignment(pages: &mut [LayoutPage]) {
    for page in pages {
        let alignment = page.geometry.vertical_alignment;
        if alignment == PageVerticalAlignment::Top {
            continue;
        }

        let Some(bounds) = page_body_item_bounds(page) else {
            continue;
        };
        let body_top = page.geometry.height - page.geometry.margin_top;
        let body_bottom = page.geometry.margin_bottom;
        let available_height = (body_top - body_bottom).max(0.0);
        if available_height <= 0.0 {
            continue;
        }

        let current_center = (bounds.top + bounds.bottom) / 2.0;
        let target_center = body_bottom + (available_height / 2.0);
        let max_shift_down = (bounds.bottom - body_bottom).max(0.0);
        let shift_down = match alignment {
            PageVerticalAlignment::Top => 0.0,
            PageVerticalAlignment::Center => (current_center - target_center)
                .max(0.0)
                .min(max_shift_down),
            PageVerticalAlignment::Bottom => max_shift_down,
        };

        if shift_down > 0.01 {
            for item in &mut page.items {
                translate_layout_item_y(item, -shift_down);
            }
            for exclusion in &mut page.flow_exclusions {
                exclusion.y -= shift_down;
            }
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub(in crate::layout) struct VerticalBounds {
    pub(in crate::layout) top: f32,
    pub(in crate::layout) bottom: f32,
}

pub(in crate::layout) fn page_body_item_bounds(page: &LayoutPage) -> Option<VerticalBounds> {
    layout_items_vertical_bounds(&page.items)
}

pub(in crate::layout) fn layout_items_vertical_bounds(
    items: &[LayoutItem],
) -> Option<VerticalBounds> {
    items
        .iter()
        .filter_map(layout_item_vertical_bounds)
        .reduce(|acc, bounds| VerticalBounds {
            top: acc.top.max(bounds.top),
            bottom: acc.bottom.min(bounds.bottom),
        })
}

pub(in crate::layout) fn layout_item_vertical_bounds(item: &LayoutItem) -> Option<VerticalBounds> {
    match item {
        LayoutItem::Highlight { y, height, .. } => Some(VerticalBounds {
            top: *y + *height,
            bottom: *y,
        }),
        LayoutItem::Text(fragment) => {
            let font_size = fragment.style.font_size_points();
            Some(VerticalBounds {
                top: fragment.baseline_y + (font_size * 0.25),
                bottom: fragment.baseline_y - font_size,
            })
        }
        LayoutItem::Underline { y, .. } => Some(VerticalBounds {
            top: *y + 1.0,
            bottom: *y - 1.0,
        }),
        LayoutItem::Line { y1, y2, width, .. } | LayoutItem::CappedLine { y1, y2, width, .. } => {
            let half_width = *width / 2.0;
            Some(VerticalBounds {
                top: (*y1).max(*y2) + half_width,
                bottom: (*y1).min(*y2) - half_width,
            })
        }
        LayoutItem::JoinedPolyline { points, width, .. } => {
            if points.is_empty() {
                return None;
            }
            let half_width = *width / 2.0;
            Some(VerticalBounds {
                top: points
                    .iter()
                    .map(|point| point.y)
                    .fold(f32::NEG_INFINITY, f32::max)
                    + half_width,
                bottom: points
                    .iter()
                    .map(|point| point.y)
                    .fold(f32::INFINITY, f32::min)
                    - half_width,
            })
        }
        LayoutItem::Ellipse {
            y,
            height,
            stroke_width,
            ..
        } => {
            let half_width = *stroke_width / 2.0;
            Some(VerticalBounds {
                top: *y + *height + half_width,
                bottom: *y - half_width,
            })
        }
        LayoutItem::RoundedRectangle {
            y,
            height,
            stroke_width,
            ..
        } => {
            let half_width = *stroke_width / 2.0;
            Some(VerticalBounds {
                top: *y + *height + half_width,
                bottom: *y - half_width,
            })
        }
        LayoutItem::Polygon {
            points,
            paths,
            overlay_paths,
            stroke_width,
            ..
        } => {
            if points.is_empty()
                && paths.iter().all(Vec::is_empty)
                && overlay_paths.iter().all(Vec::is_empty)
            {
                return None;
            }
            let half_width = *stroke_width / 2.0;
            let all_points = points
                .iter()
                .chain(paths.iter().flatten())
                .chain(overlay_paths.iter().flatten());
            Some(VerticalBounds {
                top: all_points
                    .clone()
                    .map(|point| point.y)
                    .fold(f32::NEG_INFINITY, f32::max)
                    + half_width,
                bottom: all_points
                    .map(|point| point.y)
                    .fold(f32::INFINITY, f32::min)
                    - half_width,
            })
        }
        LayoutItem::Image(image) => Some(VerticalBounds {
            top: image.y + image.height,
            bottom: image.y,
        }),
        LayoutItem::Drawing(drawing) => {
            let mut current = drawing.item.as_ref();
            loop {
                match current {
                    LayoutItem::Drawing(nested) => current = nested.item.as_ref(),
                    LayoutItem::Opacity { item, .. } => current = item.as_ref(),
                    LayoutItem::ClipRect {
                        y, height, item, ..
                    } => {
                        let bounds = layout_item_vertical_bounds(item)?;
                        let top = bounds.top.min(*y + *height);
                        let bottom = bounds.bottom.max(*y);
                        return (top >= bottom).then_some(VerticalBounds { top, bottom });
                    }
                    leaf => return layout_item_vertical_bounds(leaf),
                }
            }
        }
        LayoutItem::Opacity { item, .. } => layout_item_vertical_bounds(item),
        LayoutItem::ClipRect {
            y, height, item, ..
        } => {
            let bounds = layout_item_vertical_bounds(item)?;
            let top = bounds.top.min(*y + *height);
            let bottom = bounds.bottom.max(*y);
            (top >= bottom).then_some(VerticalBounds { top, bottom })
        }
    }
}

pub(in crate::layout) fn translate_layout_item_y(item: &mut LayoutItem, delta_y: f32) {
    let mut current = item;
    loop {
        match current {
            LayoutItem::Highlight { y, .. } => *y += delta_y,
            LayoutItem::Text(fragment) => fragment.baseline_y += delta_y,
            LayoutItem::Underline { y, .. } => *y += delta_y,
            LayoutItem::Line { y1, y2, .. } | LayoutItem::CappedLine { y1, y2, .. } => {
                *y1 += delta_y;
                *y2 += delta_y;
            }
            LayoutItem::JoinedPolyline { points, .. } => {
                for point in points {
                    point.y += delta_y;
                }
            }
            LayoutItem::Ellipse { y, .. } => *y += delta_y,
            LayoutItem::RoundedRectangle { y, .. } => *y += delta_y,
            LayoutItem::Polygon { points, paths, .. } => {
                for point in points {
                    point.y += delta_y;
                }
                for path in paths {
                    for point in path {
                        point.y += delta_y;
                    }
                }
            }
            LayoutItem::Image(image) => image.y += delta_y,
            LayoutItem::Drawing(drawing) => {
                current = drawing.item.as_mut();
                continue;
            }
            LayoutItem::Opacity { item, .. } => {
                current = item.as_mut();
                continue;
            }
            LayoutItem::ClipRect { y, item, .. } => {
                *y += delta_y;
                current = item.as_mut();
                continue;
            }
        }
        break;
    }
}

pub(in crate::layout) fn layout_column_separators(pages: &mut [LayoutPage]) {
    for page in pages {
        let geometry = page.geometry;
        if !geometry.line_between_columns {
            continue;
        }
        let Some(bounds) = column_separator_content_bounds(&page.items) else {
            continue;
        };
        for column_idx in 1..geometry.column_count {
            let previous = column_idx - 1;
            let x = geometry.body_left(previous)
                + geometry.body_width(previous)
                + (geometry.column_gaps[previous] / 2.0);
            page.items.push(LayoutItem::Line {
                x1: x,
                y1: bounds.bottom,
                x2: x,
                y2: bounds.top,
                width: 0.72,
                color: PdfColor {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                },
                style: LineStyle::Solid,
            });
        }
    }
}

pub(in crate::layout) fn column_separator_content_bounds(
    items: &[LayoutItem],
) -> Option<VerticalBounds> {
    items
        .iter()
        .filter_map(|item| match item {
            LayoutItem::Text(fragment) => {
                let font_size = fragment.style.font_size_points();
                Some(VerticalBounds {
                    top: fragment.baseline_y + (font_size * 0.94),
                    bottom: fragment.baseline_y - (font_size * 0.21),
                })
            }
            item => layout_item_vertical_bounds(item),
        })
        .reduce(|acc, bounds| VerticalBounds {
            top: acc.top.max(bounds.top),
            bottom: acc.bottom.min(bounds.bottom),
        })
}

pub(in crate::layout) fn layout_page_borders(
    pages: &mut [LayoutPage],
    document: &Document,
    header_footer_sets: &[HeaderFooterSet],
) {
    for page in pages {
        let geometry = page.geometry;
        let header_footer_set = header_footer_sets
            .get(geometry.header_footer_index)
            .or_else(|| header_footer_sets.first());
        let has_header_content = header_footer_set.is_some_and(|set| {
            !set.header.is_empty()
                || !set.first_page_header.is_empty()
                || !set.even_page_header.is_empty()
                || !set.header_images.is_empty()
                || !set.first_page_header_images.is_empty()
                || !set.even_page_header_images.is_empty()
                || !set.header_shapes.is_empty()
                || !set.first_page_header_shapes.is_empty()
                || !set.even_page_header_shapes.is_empty()
        });
        let has_footer_content = header_footer_set.is_some_and(|set| {
            !set.footer.is_empty()
                || !set.first_page_footer.is_empty()
                || !set.even_page_footer.is_empty()
                || !set.footer_images.is_empty()
                || !set.first_page_footer_images.is_empty()
                || !set.even_page_footer_images.is_empty()
                || !set.footer_shapes.is_empty()
                || !set.first_page_footer_shapes.is_empty()
                || !set.even_page_footer_shapes.is_empty()
        });
        let borders = geometry.page_borders;
        let spacing = geometry.page_border_spacing;
        let body_left = geometry.margin_left;
        let body_right = geometry.margin_left + geometry.content_width;
        let body_top = if geometry.page_border_includes_header && has_header_content {
            (geometry.height - geometry.header_distance).max(geometry.height - geometry.margin_top)
        } else {
            geometry.height - geometry.margin_top
        };
        let body_bottom = if geometry.page_border_includes_footer && has_footer_content {
            geometry.footer_distance.min(geometry.margin_bottom)
        } else {
            geometry.margin_bottom
        };
        let (left, right, top, bottom) = if geometry.page_border_from_page_edge {
            (
                twips_to_points(spacing.left_twips).clamp(0.0, geometry.width),
                (geometry.width - twips_to_points(spacing.right_twips)).clamp(0.0, geometry.width),
                (geometry.height - twips_to_points(spacing.top_twips)).clamp(0.0, geometry.height),
                twips_to_points(spacing.bottom_twips).clamp(0.0, geometry.height),
            )
        } else {
            (
                (body_left - twips_to_points(spacing.left_twips)).clamp(0.0, geometry.width),
                (body_right + twips_to_points(spacing.right_twips)).clamp(0.0, geometry.width),
                (body_top + twips_to_points(spacing.top_twips)).clamp(0.0, geometry.height),
                (body_bottom - twips_to_points(spacing.bottom_twips)).clamp(0.0, geometry.height),
            )
        };

        if borders.top.visible {
            if borders.top.style == BorderStyle::BorderArtScissors {
                layout_scissors_page_border_art(page, left, right, top, document, &borders.top);
            } else {
                let (width, color, style) = page_border_stroke(&borders.top, document);
                page.items.push(LayoutItem::Line {
                    x1: left,
                    y1: top,
                    x2: right,
                    y2: top,
                    width,
                    color,
                    style,
                });
            }
        }
        if borders.bottom.visible {
            let (width, color, style) = page_border_stroke(&borders.bottom, document);
            page.items.push(LayoutItem::Line {
                x1: left,
                y1: bottom,
                x2: right,
                y2: bottom,
                width,
                color,
                style,
            });
        }
        if borders.left.visible {
            let (width, color, style) = page_border_stroke(&borders.left, document);
            page.items.push(LayoutItem::Line {
                x1: left,
                y1: top,
                x2: left,
                y2: bottom,
                width,
                color,
                style,
            });
        }
        if borders.right.visible {
            let (width, color, style) = page_border_stroke(&borders.right, document);
            page.items.push(LayoutItem::Line {
                x1: right,
                y1: top,
                x2: right,
                y2: bottom,
                width,
                color,
                style,
            });
        }
    }
}

pub(in crate::layout) fn layout_background_shapes(
    pages: &mut [LayoutPage],
    header_footer_sets: &[HeaderFooterSet],
    document: &Document,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) {
    for (page_idx, page) in pages.iter_mut().enumerate() {
        let shapes = background_shapes_for_page(header_footer_sets, page.geometry);
        if shapes.is_empty() {
            continue;
        }

        let mut scratch_pages = vec![new_layout_page(page.geometry, page_idx + 1)];
        let mut cursor_y = page.height;
        let mut background_geometry = page.geometry;
        background_geometry.margin_left = 0.0;
        background_geometry.margin_top = 0.0;
        background_geometry.margin_bottom = -1_000_000.0;
        background_geometry.content_width = page.width;
        background_geometry.column_count = 1;
        background_geometry.column_lefts = [0.0; MAX_LAYOUT_COLUMNS];
        background_geometry.column_widths = [page.width; MAX_LAYOUT_COLUMNS];
        background_geometry.column_gaps = [0.0; MAX_LAYOUT_COLUMNS];
        let mut background_column = 0usize;

        for shape in shapes {
            let mut page_background = shape.clone();
            page_background.left_twips = 0;
            page_background.top_twips = 0;
            page_background.width_twips = ((page.width * 20.0).round() as i32).saturating_add(2);
            page_background.height_twips = ((page.height * 20.0).round() as i32).saturating_add(2);
            page_background.horizontal_anchor = StaticShapeHorizontalAnchor::Page;
            page_background.vertical_anchor = StaticShapeVerticalAnchor::Page;
            page_background.below_text = true;
            page_background.text_wrap = false;
            page_background.stroke_width_twips = 0;
            layout_shape(
                &mut scratch_pages,
                &mut cursor_y,
                None,
                &page_background,
                page.width + 0.1,
                0.0,
                -1_000_000.0,
                &mut background_geometry,
                &mut background_column,
                document,
                document_stats,
                font_provider,
            );
        }

        let mut items = scratch_pages.remove(0).items;
        items.append(&mut page.items);
        page.items = items;
    }
}

pub(in crate::layout) fn background_shapes_for_page<'a>(
    header_footer_sets: &'a [HeaderFooterSet],
    geometry: PageGeometry,
) -> &'a [StaticShape] {
    header_footer_sets
        .get(geometry.header_footer_index)
        .filter(|set| !set.background_shapes.is_empty())
        .or_else(|| header_footer_sets.first())
        .map(|set| set.background_shapes.as_slice())
        .unwrap_or(&[])
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn layout_repeating_header_footer(
    pages: &mut [LayoutPage],
    document: &Document,
    header_footer_sets: &[HeaderFooterSet],
    is_header: bool,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
    body_item_counts: Option<&[usize]>,
) {
    for (page_idx, page) in pages.iter_mut().enumerate() {
        let geometry = page.geometry;
        let physical_page_number = page_idx + 1;
        let paragraphs = repeating_paragraphs_for_page(
            document,
            header_footer_sets,
            physical_page_number,
            geometry,
            is_header,
        );
        let images = select_repeating_header_footer_images(
            document,
            header_footer_sets,
            physical_page_number,
            geometry,
            is_header,
        );
        let shapes = select_repeating_header_footer_shapes(
            document,
            header_footer_sets,
            physical_page_number,
            geometry,
            is_header,
        );
        if paragraphs.is_empty() && images.is_empty() && shapes.is_empty() {
            continue;
        }
        let mut scratch_pages = vec![new_layout_page(geometry, physical_page_number)];
        let markers = marker_context(
            page.display_page_number.clone(),
            page.section_number.to_string(),
            document_stats,
        );
        let initial_cursor_y = if is_header {
            (page.height - geometry.header_distance).clamp(0.0, page.height)
        } else {
            geometry.footer_distance.clamp(0.0, page.height)
        };
        let mut cursor_y = initial_cursor_y;

        for paragraph in paragraphs {
            let lines = wrap_paragraph_with_font_provider(
                paragraph,
                geometry.content_width,
                &markers,
                document,
                font_provider,
            );
            let line_count = lines.len();
            for (line_idx, line) in lines.into_iter().enumerate() {
                let x = aligned_x(
                    geometry.margin_left,
                    geometry.content_width,
                    line.width,
                    &paragraph.style,
                    line_idx == 0,
                );
                let line_cursor_y = cursor_y;
                let word_spacing = justified_word_spacing(
                    &line,
                    &paragraph.style,
                    paragraph_line_width(geometry.content_width, &paragraph.style, line_idx == 0),
                    line_idx + 1 == line_count,
                    document,
                    font_provider,
                );
                let character_spacing = distributed_character_spacing(
                    &line,
                    &paragraph.style,
                    paragraph_line_width(geometry.content_width, &paragraph.style, line_idx == 0),
                    line_idx + 1 == line_count,
                );
                if shading_is_visible(
                    document,
                    paragraph.style.shading_color_index,
                    paragraph.style.shading_pattern,
                ) {
                    let line_left_indent = twips_to_points(paragraph_line_left_indent_twips(
                        &paragraph.style,
                        line_idx == 0,
                    ));
                    push_shading_rect(
                        &mut scratch_pages,
                        document,
                        geometry.margin_left + line_left_indent,
                        line_cursor_y - line.height,
                        paragraph_line_width(
                            geometry.content_width,
                            &paragraph.style,
                            line_idx == 0,
                        ),
                        line.height,
                        paragraph.style.shading_color_index,
                        paragraph.style.shading_basis_points,
                        paragraph.style.shading_pattern,
                    );
                }
                push_paragraph_borders(
                    &mut scratch_pages,
                    geometry.margin_left,
                    geometry.content_width,
                    &paragraph.style,
                    line_idx,
                    line_count,
                    line_cursor_y,
                    line.height,
                    document,
                    false,
                    true,
                );
                push_bar_tab_stops(
                    &mut scratch_pages,
                    &paragraph.style,
                    x,
                    line_cursor_y,
                    line.height,
                );
                push_line(
                    &mut scratch_pages,
                    &line,
                    x,
                    line_cursor_y,
                    document,
                    font_provider,
                    word_spacing,
                    character_spacing,
                );
                cursor_y -= line.height;
            }
        }

        for image in images {
            let (left_offset, top_offset, width, height) =
                image_layout_frame(image, geometry.content_width);
            let placement = image.placement;
            let x_origin = placement
                .map(|placement| {
                    shape_horizontal_origin(
                        placement.horizontal_anchor,
                        geometry.margin_left,
                        &geometry,
                    )
                })
                .unwrap_or(geometry.margin_left);
            let y_origin = placement
                .map(|placement| {
                    shape_vertical_origin(placement.vertical_anchor, cursor_y, &geometry)
                })
                .unwrap_or(cursor_y);
            let y = y_origin - top_offset - height;
            scratch_pages[0]
                .items
                .push(LayoutItem::Image(ImageFragment {
                    image: image.clone(),
                    x: x_origin + left_offset,
                    y,
                    width,
                    height,
                    z_order: image
                        .placement
                        .map(|placement| placement.z_order)
                        .unwrap_or(0),
                    below_text: image
                        .placement
                        .map(|placement| placement.below_text)
                        .unwrap_or(false),
                }));
            cursor_y = y - 6.0;
        }

        for shape in shapes {
            let mut shape_geometry = geometry;
            let mut shape_column = 0usize;
            layout_shape(
                &mut scratch_pages,
                &mut cursor_y,
                None,
                shape,
                geometry.content_width,
                geometry.margin_left,
                -1_000_000.0,
                &mut shape_geometry,
                &mut shape_column,
                document,
                document_stats,
                font_provider,
            );
        }

        if is_header {
            let body_item_count = body_item_counts
                .and_then(|counts| counts.get(page_idx))
                .copied()
                .unwrap_or(0)
                .min(page.items.len());
            let body_bounds = layout_items_vertical_bounds(&page.items[..body_item_count]);
            let header_bounds = page_body_item_bounds(&scratch_pages[0]);
            if let (Some(body_bounds), Some(header_bounds)) = (body_bounds, header_bounds) {
                let flow_collision = (page.height - geometry.margin_top - cursor_y).max(0.0);
                let visual_collision = (body_bounds.top - header_bounds.bottom).max(0.0);
                let available_shift = (body_bounds.bottom - geometry.margin_bottom).max(0.0);
                let mut shift_down = flow_collision.min(visual_collision).min(available_shift);
                if shift_down > 0.01 && geometry.text_line_grid_twips.is_some() {
                    shift_down = (shift_down + 1.0).min(available_shift);
                }
                if shift_down > 0.01 {
                    for item in page.items.iter_mut().take(body_item_count) {
                        translate_layout_item_y(item, -shift_down);
                    }
                    for exclusion in &mut page.flow_exclusions {
                        exclusion.y -= shift_down;
                    }
                }
            }
        } else {
            let shift_up = (initial_cursor_y - cursor_y).max(0.0);
            if shift_up > 0.01 {
                for item in &mut scratch_pages[0].items {
                    translate_layout_item_y(item, shift_up);
                }
            }
        }

        page.items.extend(scratch_pages.remove(0).items);
    }
}

pub(in crate::layout) fn layout_legacy_auto_page_numbers(
    pages: &mut [LayoutPage],
    document: &Document,
    font_provider: Option<&FontProvider>,
) {
    for page in pages {
        let geometry = page.geometry;
        if geometry.page_number_x.is_none() && geometry.page_number_y.is_none() {
            continue;
        }

        let paragraph = Paragraph {
            style: ParagraphStyle::default(),
            runs: vec![Run {
                dynamic_field: None,
                text: page.display_page_number.clone(),
                style: CharacterStyle::default(),
            }],
        };
        let markers = marker_context(
            page.display_page_number.clone(),
            page.section_number.to_string(),
            DocumentStats::default(),
        );
        let Some(line) = wrap_paragraph_with_font_provider(
            &paragraph,
            geometry.content_width,
            &markers,
            document,
            font_provider,
        )
        .into_iter()
        .next() else {
            continue;
        };

        let horizontal_offset = geometry.page_number_x.unwrap_or(36.0);
        let vertical_offset = geometry.page_number_y.unwrap_or(36.0);
        let right_edge = (geometry.margin_left + geometry.content_width + horizontal_offset)
            .clamp(0.0, geometry.width);
        let x = (right_edge - line.width).clamp(0.0, geometry.width);
        let line_cursor_y = (geometry.height - vertical_offset).clamp(0.0, geometry.height);
        let mut scratch_pages = vec![new_layout_page(geometry, geometry.physical_page_number)];
        push_line(
            &mut scratch_pages,
            &line,
            x,
            line_cursor_y,
            document,
            font_provider,
            0.0,
            0.0,
        );
        page.items.extend(scratch_pages.remove(0).items);
    }
}

pub(in crate::layout) fn repeating_paragraphs_for_page<'a>(
    document: &'a Document,
    header_footer_sets: &'a [HeaderFooterSet],
    physical_page_number: usize,
    geometry: PageGeometry,
    is_header: bool,
) -> &'a [Paragraph] {
    let Some(section) = header_footer_sets
        .get(geometry.header_footer_index)
        .or_else(|| header_footer_sets.first())
    else {
        return &[];
    };
    let is_first_section_page =
        geometry.title_page && physical_page_number == geometry.numbering.base_physical_page;
    if is_header {
        if is_first_section_page {
            let paragraphs =
                first_non_empty(&section.first_page_header, &document.first_page_header);
            if !paragraphs.is_empty() {
                return paragraphs;
            }
        }
        if geometry.facing_pages && physical_page_number % 2 == 0 {
            let paragraphs = first_non_empty(&section.even_page_header, &document.even_page_header);
            if !paragraphs.is_empty() {
                return paragraphs;
            }
        }
        first_non_empty(&section.header, &document.header)
    } else {
        if is_first_section_page {
            let paragraphs =
                first_non_empty(&section.first_page_footer, &document.first_page_footer);
            if !paragraphs.is_empty() {
                return paragraphs;
            }
        }
        if geometry.facing_pages && physical_page_number % 2 == 0 {
            let paragraphs = first_non_empty(&section.even_page_footer, &document.even_page_footer);
            if !paragraphs.is_empty() {
                return paragraphs;
            }
        }
        first_non_empty(&section.footer, &document.footer)
    }
}

pub(in crate::layout) fn first_non_empty<'a>(
    primary: &'a [Paragraph],
    fallback: &'a [Paragraph],
) -> &'a [Paragraph] {
    if primary.is_empty() {
        fallback
    } else {
        primary
    }
}

pub(in crate::layout) fn select_repeating_header_footer_images<'a>(
    document: &'a Document,
    header_footer_sets: &'a [HeaderFooterSet],
    physical_page_number: usize,
    geometry: PageGeometry,
    is_header: bool,
) -> &'a [StaticImage] {
    let Some(section) = header_footer_sets
        .get(geometry.header_footer_index)
        .or_else(|| header_footer_sets.first())
    else {
        return &[];
    };
    let is_first_section_page =
        geometry.title_page && physical_page_number == geometry.numbering.base_physical_page;
    if is_header {
        if is_first_section_page {
            let images = first_non_empty_images(
                &section.first_page_header_images,
                &document.first_page_header_images,
            );
            if !images.is_empty() {
                return images;
            }
        }
        if geometry.facing_pages && physical_page_number % 2 == 0 {
            let images = first_non_empty_images(
                &section.even_page_header_images,
                &document.even_page_header_images,
            );
            if !images.is_empty() {
                return images;
            }
        }
        first_non_empty_images(&section.header_images, &document.header_images)
    } else {
        if is_first_section_page {
            let images = first_non_empty_images(
                &section.first_page_footer_images,
                &document.first_page_footer_images,
            );
            if !images.is_empty() {
                return images;
            }
        }
        if geometry.facing_pages && physical_page_number % 2 == 0 {
            let images = first_non_empty_images(
                &section.even_page_footer_images,
                &document.even_page_footer_images,
            );
            if !images.is_empty() {
                return images;
            }
        }
        first_non_empty_images(&section.footer_images, &document.footer_images)
    }
}

pub(in crate::layout) fn first_non_empty_images<'a>(
    primary: &'a [StaticImage],
    fallback: &'a [StaticImage],
) -> &'a [StaticImage] {
    if primary.is_empty() {
        fallback
    } else {
        primary
    }
}

pub(in crate::layout) fn select_repeating_header_footer_shapes<'a>(
    document: &'a Document,
    header_footer_sets: &'a [HeaderFooterSet],
    physical_page_number: usize,
    geometry: PageGeometry,
    is_header: bool,
) -> &'a [StaticShape] {
    let Some(section) = header_footer_sets
        .get(geometry.header_footer_index)
        .or_else(|| header_footer_sets.first())
    else {
        return &[];
    };
    let is_first_section_page =
        geometry.title_page && physical_page_number == geometry.numbering.base_physical_page;
    if is_header {
        if is_first_section_page {
            let shapes = first_non_empty_shapes(
                &section.first_page_header_shapes,
                &document.first_page_header_shapes,
            );
            if !shapes.is_empty() {
                return shapes;
            }
        }
        if geometry.facing_pages && physical_page_number % 2 == 0 {
            let shapes = first_non_empty_shapes(
                &section.even_page_header_shapes,
                &document.even_page_header_shapes,
            );
            if !shapes.is_empty() {
                return shapes;
            }
        }
        first_non_empty_shapes(&section.header_shapes, &document.header_shapes)
    } else {
        if is_first_section_page {
            let shapes = first_non_empty_shapes(
                &section.first_page_footer_shapes,
                &document.first_page_footer_shapes,
            );
            if !shapes.is_empty() {
                return shapes;
            }
        }
        if geometry.facing_pages && physical_page_number % 2 == 0 {
            let shapes = first_non_empty_shapes(
                &section.even_page_footer_shapes,
                &document.even_page_footer_shapes,
            );
            if !shapes.is_empty() {
                return shapes;
            }
        }
        first_non_empty_shapes(&section.footer_shapes, &document.footer_shapes)
    }
}

pub(in crate::layout) fn first_non_empty_shapes<'a>(
    primary: &'a [StaticShape],
    fallback: &'a [StaticShape],
) -> &'a [StaticShape] {
    if primary.is_empty() {
        fallback
    } else {
        primary
    }
}
