//! Paragraph flow, frames, exclusions, footnote hooks, and line numbering.

use super::*;

pub(in crate::layout) struct AutoFootnoteFlush<'a> {
    pub(in crate::layout) rendered_footnote_pages: &'a mut [Option<usize>],
    pub(in crate::layout) block_index: usize,
    pub(in crate::layout) rendered_footnote_index_on_page: Option<usize>,
    pub(in crate::layout) reserved_footnote_margin_bottom_on_page: Option<f32>,
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn layout_paragraph(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    paragraph: &Paragraph,
    suppress_contextual_space_before: bool,
    suppress_contextual_space_after: bool,
    render_between_border_after: bool,
    content_width: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    current_column: &mut usize,
    document: &Document,
    document_stats: DocumentStats,
    line_numbers: Option<&mut LineNumberState>,
    markers: &MarkerContext,
    font_provider: Option<&FontProvider>,
) {
    layout_paragraph_with_auto_footnotes(
        pages,
        cursor_y,
        paragraph,
        suppress_contextual_space_before,
        suppress_contextual_space_after,
        render_between_border_after,
        content_width,
        margin_bottom,
        geometry,
        current_column,
        document,
        document_stats,
        line_numbers,
        markers,
        font_provider,
        None,
        None,
        None,
    );
}

pub(in crate::layout) struct PreparedParagraphFrameGeometry {
    pub(in crate::layout) width: f32,
    pub(in crate::layout) height: f32,
    pub(in crate::layout) lines: Vec<Vec<Line>>,
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn prepare_paragraph_frame_geometry(
    paragraphs: &[Paragraph],
    style: &ParagraphStyle,
    content_width: f32,
    max_height: f32,
    markers: &MarkerContext,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> PreparedParagraphFrameGeometry {
    let natural_width = paragraphs
        .iter()
        .flat_map(|paragraph| {
            wrap_paragraph_with_font_provider(
                paragraph,
                content_width,
                markers,
                document,
                font_provider,
            )
        })
        .map(|line| line.width)
        .fold(0.0_f32, f32::max)
        .max(1.0);
    let vertical = style.frame_text_direction != TableCellTextDirection::LeftToRightTopToBottom;
    let authored_width = style
        .frame_width_twips
        .map(|twips| twips_to_points(twips.saturating_abs()));
    let authored_height = style
        .frame_height_twips
        .map(|twips| twips_to_points(twips.saturating_abs()));

    if vertical {
        let height = authored_height
            .unwrap_or(natural_width)
            .clamp(1.0, max_height.max(1.0));
        let lines = paragraphs
            .iter()
            .map(|paragraph| {
                wrap_paragraph_with_font_provider(
                    paragraph,
                    height,
                    markers,
                    document,
                    font_provider,
                )
            })
            .collect::<Vec<_>>();
        let natural_cross_extent = lines
            .iter()
            .flatten()
            .map(|line| line.height)
            .sum::<f32>()
            .max(1.0);
        PreparedParagraphFrameGeometry {
            width: authored_width
                .unwrap_or(natural_cross_extent)
                .clamp(1.0, content_width.max(1.0)),
            height,
            lines,
        }
    } else {
        let width = authored_width
            .unwrap_or(natural_width)
            .clamp(1.0, content_width.max(1.0));
        let lines = paragraphs
            .iter()
            .map(|paragraph| {
                wrap_paragraph_with_font_provider(
                    paragraph,
                    width,
                    markers,
                    document,
                    font_provider,
                )
            })
            .collect::<Vec<_>>();
        let natural_height = lines
            .iter()
            .flatten()
            .map(|line| line.height)
            .sum::<f32>()
            .max(1.0);
        PreparedParagraphFrameGeometry {
            width,
            height: authored_height
                .unwrap_or(natural_height)
                .clamp(1.0, max_height.max(1.0)),
            lines,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn layout_framed_paragraph_group(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    paragraphs: &[&Paragraph],
    content_width: f32,
    geometry: &mut PageGeometry,
    current_column: &mut usize,
    document: &Document,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
    absolute_frame_preview: Option<FlowExclusion>,
) -> bool {
    let Some(paragraph) = paragraphs.first().copied() else {
        return false;
    };
    if paragraph.style.frame_width_twips.is_none() && paragraph.style.frame_height_twips.is_none() {
        return false;
    }
    let negative_width_frame = paragraph
        .style
        .frame_width_twips
        .is_some_and(|width| width < 0);
    let passive_frame_paragraphs = paragraphs
        .iter()
        .map(|paragraph| {
            if negative_width_frame && paragraph.style.drop_cap_lines <= 1 {
                passive_negative_width_frame_paragraph(paragraph)
            } else {
                passive_frame_content_paragraph(paragraph)
            }
        })
        .collect::<Vec<_>>();
    let markers = current_marker_context(pages, document_stats);
    let prepared_frame = prepare_paragraph_frame_geometry(
        &passive_frame_paragraphs,
        &paragraph.style,
        content_width,
        geometry.height - geometry.margin_top - geometry.margin_bottom,
        &markers,
        document,
        font_provider,
    );
    let frame_width = prepared_frame.width;
    let frame_height = prepared_frame.height;
    let framed_lines = prepared_frame.lines;
    let frame_uses_flow_position = paragraph_frame_uses_flow_position(&paragraph.style);
    if negative_width_frame && paragraph.style.frame_height_twips.is_none() {
        return true;
    }
    let requested_frame_top = |cursor_y: f32, geometry: &PageGeometry| {
        if negative_width_frame {
            cursor_y
        } else {
            paragraph_frame_top(&paragraph.style, frame_height, cursor_y, *geometry)
        }
    };
    if (negative_width_frame || frame_uses_flow_position)
        && requested_frame_top(*cursor_y, geometry) - frame_height < geometry.margin_bottom
    {
        advance_column_or_page(pages, cursor_y, geometry, current_column);
    }
    let mut frame_top_y =
        requested_frame_top(*cursor_y, geometry).clamp(frame_height, geometry.height);
    let frame_left = paragraph_frame_left(
        &paragraph.style,
        frame_width,
        content_width,
        *geometry,
        *current_column,
    );
    if negative_width_frame {
        let frame_item_start = pages.last().map(|page| page.items.len()).unwrap_or(0);
        let mut frame_cursor_y = frame_top_y;
        let mut frame_geometry = *geometry;
        let mut frame_column = *current_column;
        for passive_frame_paragraph in &passive_frame_paragraphs {
            layout_paragraph(
                pages,
                &mut frame_cursor_y,
                passive_frame_paragraph,
                false,
                false,
                false,
                frame_width,
                geometry.margin_bottom,
                &mut frame_geometry,
                &mut frame_column,
                document,
                document_stats,
                None,
                &markers,
                font_provider,
            );
        }
        if let Some(page) = pages.last_mut() {
            let frame_x_shift = frame_left - geometry.body_left(*current_column);
            for item in page.items.iter_mut().skip(frame_item_start) {
                shift_layout_item_x(item, frame_x_shift);
            }
        }
        *cursor_y = frame_top_y - frame_height;
        return true;
    }
    let frame_text_distance = twips_to_points(paragraph.style.frame_text_distance_twips.max(0));
    let exclusion_width =
        (frame_width + frame_text_distance).min((geometry.width - frame_left).max(1.0));
    let frame_vertical_text_distance =
        paragraph_frame_vertical_text_distance(&paragraph.style, geometry.height);
    let (mut exclusion_y, mut exclusion_height) = paragraph_frame_exclusion_vertical_geometry(
        frame_top_y,
        frame_height,
        frame_vertical_text_distance,
        geometry.height,
    );
    if !frame_uses_flow_position {
        let stack_advance = framed_lines
            .first()
            .and_then(|lines| lines.first())
            .map(|line| line.height)
            .unwrap_or(14.0)
            .max(1.0);
        let requested_exclusion_top = exclusion_y + exclusion_height;
        let stacked_exclusion_top = if let Some(preview) = absolute_frame_preview {
            preview.y + preview.height
        } else {
            stacked_absolute_frame_top(
                pages.last(),
                frame_left,
                exclusion_width,
                exclusion_height,
                requested_exclusion_top,
                stack_advance,
            )
        };
        frame_top_y += stacked_exclusion_top - requested_exclusion_top;
        (exclusion_y, exclusion_height) = paragraph_frame_exclusion_vertical_geometry(
            frame_top_y,
            frame_height,
            frame_vertical_text_distance,
            geometry.height,
        );
    }
    let frame_exclusion = FlowExclusion {
        x: frame_left,
        y: exclusion_y,
        width: exclusion_width,
        height: exclusion_height,
        wrap_side: paragraph_frame_wrap_side(paragraph),
        wrap_bottom_inset: if frame_uses_flow_position { 0.0 } else { 1.0 },
        kind: FlowExclusionKind::ParagraphFrame,
    };
    let frame_page_index = pages.len().saturating_sub(1);
    let mut removed_frame_exclusions = Vec::new();
    if !frame_uses_flow_position {
        if let Some(page) = pages.get_mut(frame_page_index) {
            let mut index = 0;
            while index < page.flow_exclusions.len() {
                if page.flow_exclusions[index].kind == FlowExclusionKind::ParagraphFrame {
                    let original_index = index + removed_frame_exclusions.len();
                    removed_frame_exclusions
                        .push((original_index, page.flow_exclusions.remove(index)));
                } else {
                    index += 1;
                }
            }
        }
    } else if let Some(preview) = absolute_frame_preview {
        if let Some(page) = pages.get_mut(frame_page_index) {
            if let Some(index) = page
                .flow_exclusions
                .iter()
                .position(|exclusion| same_flow_exclusion(*exclusion, preview))
            {
                removed_frame_exclusions.push((index, page.flow_exclusions.remove(index)));
            }
        }
    }
    let frame_item_start = pages.last().map(|page| page.items.len()).unwrap_or(0);
    let frame_text_direction = paragraph.style.frame_text_direction;
    let vertical_frame_text =
        frame_text_direction != TableCellTextDirection::LeftToRightTopToBottom;
    let frame_paint_top_y = frame_top_y
        + word_paragraph_relative_drop_cap_alignment_paint_rise_points(&paragraph.style);
    if vertical_frame_text {
        let rotation = table_cell_text_rotation(frame_text_direction);
        let mut cross_offset = 0.0;
        for (passive_frame_paragraph, lines) in passive_frame_paragraphs.iter().zip(&framed_lines) {
            cross_offset += twips_to_points(passive_frame_paragraph.style.space_before_twips);
            for (line_index, line) in lines.iter().enumerate() {
                let is_first_line = line_index == 0;
                let is_last_line = line_index + 1 == lines.len();
                let line_width = paragraph_line_width(
                    frame_height,
                    &passive_frame_paragraph.style,
                    is_first_line,
                );
                let word_spacing = justified_word_spacing(
                    line,
                    &passive_frame_paragraph.style,
                    line_width,
                    is_last_line,
                    document,
                    font_provider,
                );
                let character_spacing = distributed_character_spacing(
                    line,
                    &passive_frame_paragraph.style,
                    line_width,
                    is_last_line,
                );
                let (rotated_x, rotated_y) = rotated_paragraph_frame_line_origin(
                    frame_left,
                    frame_width,
                    frame_paint_top_y,
                    frame_height,
                    line,
                    frame_text_direction,
                    cross_offset,
                );
                push_line_with_rotation(
                    pages,
                    line,
                    rotated_x,
                    rotated_y,
                    document,
                    font_provider,
                    word_spacing,
                    character_spacing,
                    rotation,
                );
                cross_offset += paragraph_frame_vertical_column_advance(line);
            }
            cross_offset += twips_to_points(passive_frame_paragraph.style.space_after_twips);
        }
    } else {
        let mut frame_cursor_y = frame_paint_top_y;
        let mut frame_geometry = *geometry;
        let mut frame_column = *current_column;
        for passive_frame_paragraph in &passive_frame_paragraphs {
            layout_paragraph(
                pages,
                &mut frame_cursor_y,
                passive_frame_paragraph,
                false,
                false,
                false,
                frame_width,
                geometry.margin_bottom,
                &mut frame_geometry,
                &mut frame_column,
                document,
                document_stats,
                None,
                &markers,
                font_provider,
            );
        }
    }
    if let Some(page) = pages.last_mut() {
        let frame_x_shift = if vertical_frame_text {
            0.0
        } else {
            frame_left - geometry.body_left(*current_column)
        };
        for item in page.items.iter_mut().skip(frame_item_start) {
            if frame_x_shift != 0.0 {
                shift_layout_item_x(item, frame_x_shift);
            }
            if paragraph
                .style
                .frame_height_twips
                .is_some_and(|height| height < 0)
            {
                *item = LayoutItem::ClipRect {
                    x: frame_left,
                    y: frame_paint_top_y - frame_height,
                    width: frame_width,
                    height: frame_height,
                    item: Box::new(item.clone()),
                };
            }
        }
        if absolute_frame_preview.is_none() {
            page.flow_exclusions.push(frame_exclusion);
        }
    }
    if let Some(page) = pages.get_mut(frame_page_index) {
        for (index, exclusion) in removed_frame_exclusions {
            page.flow_exclusions
                .insert(index.min(page.flow_exclusions.len()), exclusion);
        }
    }
    true
}

pub(in crate::layout) fn word_paragraph_relative_drop_cap_alignment_paint_rise_points(
    style: &ParagraphStyle,
) -> f32 {
    if style.drop_cap_lines <= 1
        || style.frame_vertical_anchor != StaticShapeVerticalAnchor::Paragraph
        || style.frame_vertical_offset_twips != 0
    {
        return 0.0;
    }

    let device_pixels = match style.frame_vertical_alignment {
        Some(ParagraphFrameVerticalAlignment::Top) => 2.0,
        Some(ParagraphFrameVerticalAlignment::Center) => 3.0,
        Some(ParagraphFrameVerticalAlignment::Bottom) => 5.0,
        _ => 0.0,
    };
    device_pixels * WORD_PARAGRAPH_FRAME_DEVICE_PIXEL_POINTS
}

pub(in crate::layout) fn passive_frame_content_paragraph(paragraph: &Paragraph) -> Paragraph {
    let mut passive = paragraph.clone();
    let is_drop_cap = passive.style.drop_cap_lines > 1;
    passive.style.drop_cap_lines = 0;
    passive.style.frame_width_twips = None;
    passive.style.frame_height_twips = None;
    passive.style.frame_text_distance_twips = 0;
    passive.style.frame_vertical_text_distance_twips = 0;
    passive.style.frame_horizontal_anchor = StaticShapeHorizontalAnchor::Column;
    passive.style.frame_horizontal_alignment = None;
    passive.style.frame_horizontal_offset_twips = 0;
    passive.style.frame_vertical_anchor = StaticShapeVerticalAnchor::Paragraph;
    passive.style.frame_vertical_alignment = None;
    passive.style.frame_vertical_offset_twips = 0;
    if is_drop_cap {
        passive.style.space_before_twips = 0;
        passive.style.space_after_twips = 0;
    }
    passive
}

pub(in crate::layout) fn passive_negative_width_frame_paragraph(
    paragraph: &Paragraph,
) -> Paragraph {
    let mut passive = passive_frame_content_paragraph(paragraph);
    let first_visible = passive.runs.iter().find_map(|run| {
        run.text
            .chars()
            .find(|ch| !ch.is_control() && !ch.is_whitespace())
            .map(|ch| (ch, run.style.clone()))
    });
    passive.runs = first_visible.map_or_else(Vec::new, |(ch, style)| {
        vec![Run {
            text: ch.to_string(),
            style,
        }]
    });
    passive
}

pub(in crate::layout) fn paragraph_frame_wrap_side(paragraph: &Paragraph) -> StaticImageWrapSide {
    if paragraph.style.drop_cap_lines > 1 {
        StaticImageWrapSide::Both
    } else {
        StaticImageWrapSide::Right
    }
}

pub(in crate::layout) fn is_complete_absolute_paragraph_frame(paragraph: &Paragraph) -> bool {
    (paragraph.style.frame_width_twips.is_some() || paragraph.style.frame_height_twips.is_some())
        && !paragraph
            .style
            .frame_width_twips
            .is_some_and(|width| width < 0)
        && !paragraph_frame_uses_flow_position(&paragraph.style)
}

pub(in crate::layout) fn paragraph_frame_uses_flow_position(style: &ParagraphStyle) -> bool {
    style.frame_vertical_anchor == StaticShapeVerticalAnchor::Paragraph
        || (style.frame_vertical_anchor != StaticShapeVerticalAnchor::Page
            && style.frame_vertical_alignment == Some(ParagraphFrameVerticalAlignment::Inline))
}

pub(in crate::layout) fn same_absolute_paragraph_frame(
    first: &Paragraph,
    second: &Paragraph,
) -> bool {
    first.style.drop_cap_lines <= 1
        && second.style.drop_cap_lines <= 1
        && first.style.frame_width_twips == second.style.frame_width_twips
        && first.style.frame_height_twips == second.style.frame_height_twips
        && first.style.frame_text_distance_twips == second.style.frame_text_distance_twips
        && first.style.frame_vertical_text_distance_twips
            == second.style.frame_vertical_text_distance_twips
        && first.style.frame_horizontal_anchor == second.style.frame_horizontal_anchor
        && first.style.frame_horizontal_alignment == second.style.frame_horizontal_alignment
        && first.style.frame_horizontal_offset_twips == second.style.frame_horizontal_offset_twips
        && first.style.frame_vertical_anchor == second.style.frame_vertical_anchor
        && first.style.frame_vertical_alignment == second.style.frame_vertical_alignment
        && first.style.frame_vertical_offset_twips == second.style.frame_vertical_offset_twips
        && first.style.frame_text_direction == second.style.frame_text_direction
}

pub(in crate::layout) fn absolute_paragraph_frame_group<'a>(
    blocks: &'a [Block],
    block_index: usize,
) -> Vec<&'a Paragraph> {
    let Some(Block::Paragraph(first)) = blocks.get(block_index) else {
        return Vec::new();
    };
    let has_frame_dimensions =
        first.style.frame_width_twips.is_some() || first.style.frame_height_twips.is_some();
    if first.style.drop_cap_lines > 1 {
        return has_frame_dimensions.then_some(first).into_iter().collect();
    }
    if !has_frame_dimensions || first.style.two_in_one {
        return Vec::new();
    }
    if first.style.frame_width_twips.is_some_and(|width| width < 0) {
        return vec![first];
    }
    if block_index.checked_sub(1).is_some_and(|previous_index| {
        matches!(
            blocks.get(previous_index),
            Some(Block::Paragraph(previous)) if same_absolute_paragraph_frame(previous, first)
        )
    }) {
        return Vec::new();
    }

    blocks
        .iter()
        .skip(block_index)
        .map_while(|block| match block {
            Block::Paragraph(paragraph) if same_absolute_paragraph_frame(first, paragraph) => {
                Some(paragraph)
            }
            _ => None,
        })
        .collect()
}

pub(in crate::layout) fn take_pending_absolute_frame_preview(
    pages: &[LayoutPage],
    pending: &mut HashMap<usize, PendingAbsoluteFramePreview>,
    block_index: usize,
) -> Option<FlowExclusion> {
    let Some(preview) = pending.remove(&block_index) else {
        return None;
    };
    (preview.page_index + 1 == pages.len()).then_some(preview.exclusion)
}

pub(in crate::layout) fn same_flow_exclusion(first: FlowExclusion, second: FlowExclusion) -> bool {
    (first.x - second.x).abs() < 0.01
        && (first.y - second.y).abs() < 0.01
        && (first.width - second.width).abs() < 0.01
        && (first.height - second.height).abs() < 0.01
        && first.wrap_side == second.wrap_side
        && (first.wrap_bottom_inset - second.wrap_bottom_inset).abs() < 0.01
        && first.kind == second.kind
}

pub(in crate::layout) fn stacked_absolute_frame_top(
    page: Option<&LayoutPage>,
    frame_left: f32,
    exclusion_width: f32,
    exclusion_height: f32,
    requested_top: f32,
    stack_advance: f32,
) -> f32 {
    let Some(page) = page else {
        return requested_top;
    };
    let mut top = requested_top;
    for _ in 0..page.flow_exclusions.len().min(128) {
        let collides_at_same_origin = page.flow_exclusions.iter().any(|exclusion| {
            exclusion.kind == FlowExclusionKind::ParagraphFrame
                && (exclusion.x - frame_left).abs() < 0.01
                && (exclusion.width - exclusion_width).abs() < 0.01
                && (exclusion.height - exclusion_height).abs() < 0.01
                && ((exclusion.y + exclusion.height) - top).abs() < 0.01
        });
        if !collides_at_same_origin {
            break;
        }
        top = (top - stack_advance).max(exclusion_height);
    }
    top
}

pub(in crate::layout) fn paragraph_frame_vertical_text_distance(
    style: &ParagraphStyle,
    page_height: f32,
) -> f32 {
    if style.drop_cap_lines > 1 {
        0.0
    } else {
        twips_to_points(style.frame_vertical_text_distance_twips.max(0)).min(page_height.max(0.0))
    }
}

pub(in crate::layout) fn paragraph_frame_exclusion_vertical_geometry(
    frame_top: f32,
    frame_height: f32,
    distance: f32,
    page_height: f32,
) -> (f32, f32) {
    let bottom = (frame_top - frame_height - distance).max(0.0);
    let top = (frame_top + distance).min(page_height.max(1.0));
    (bottom, (top - bottom).max(1.0))
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn preview_future_absolute_frame_exclusions(
    pages: &mut [LayoutPage],
    pending: &mut HashMap<usize, PendingAbsoluteFramePreview>,
    planned: &mut HashMap<usize, Vec<PlannedAbsoluteFrameExclusion>>,
    target_pages: &HashMap<usize, usize>,
    blocks: &[Block],
    block_index: usize,
    cursor_y: f32,
    content_width: f32,
    geometry: PageGeometry,
    current_column: usize,
    document: &Document,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) {
    let Some(page_index) = pages.len().checked_sub(1) else {
        return;
    };
    for (future_index, block) in blocks.iter().enumerate().skip(block_index + 1) {
        if matches!(
            block,
            Block::PageBreak
                | Block::ContinuousSectionBreak
                | Block::SectionBreak
                | Block::EvenPageSectionBreak
                | Block::OddPageSectionBreak
                | Block::SectionSettings(_)
        ) {
            break;
        }
        let Block::Paragraph(_) = block else {
            continue;
        };
        let frame_group = absolute_paragraph_frame_group(blocks, future_index);
        if frame_group.is_empty() {
            continue;
        }
        let Some(&target_page_index) = target_pages.get(&future_index) else {
            continue;
        };
        if pending.contains_key(&future_index) {
            continue;
        }
        let Some(exclusion) = resolve_absolute_frame_exclusion(
            pages,
            cursor_y,
            &frame_group,
            content_width,
            geometry,
            current_column,
            document,
            document_stats,
            font_provider,
        ) else {
            continue;
        };
        planned
            .entry(target_page_index)
            .or_default()
            .push(PlannedAbsoluteFrameExclusion {
                block_index: future_index,
                exclusion,
            });
        if target_page_index != page_index {
            continue;
        }
        let Some(page) = pages.last_mut() else {
            return;
        };
        page.flow_exclusions.push(exclusion);
        pending.insert(
            future_index,
            PendingAbsoluteFramePreview {
                page_index,
                exclusion,
            },
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn resolve_absolute_frame_exclusion(
    pages: &[LayoutPage],
    cursor_y: f32,
    paragraphs: &[&Paragraph],
    content_width: f32,
    geometry: PageGeometry,
    current_column: usize,
    document: &Document,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) -> Option<FlowExclusion> {
    let paragraph = paragraphs.first().copied()?;
    if !is_complete_absolute_paragraph_frame(paragraph) {
        return None;
    }

    let passive_frame_paragraphs = paragraphs
        .iter()
        .map(|paragraph| passive_frame_content_paragraph(paragraph))
        .collect::<Vec<_>>();
    let markers = current_marker_context(pages, document_stats);
    let prepared_frame = prepare_paragraph_frame_geometry(
        &passive_frame_paragraphs,
        &paragraph.style,
        content_width,
        geometry.height - geometry.margin_top - geometry.margin_bottom,
        &markers,
        document,
        font_provider,
    );
    let frame_width = prepared_frame.width;
    let frame_height = prepared_frame.height;
    let framed_lines = prepared_frame.lines;
    let requested_frame_top =
        paragraph_frame_top(&paragraph.style, frame_height, cursor_y, geometry)
            .clamp(frame_height, geometry.height);
    let frame_left = paragraph_frame_left(
        &paragraph.style,
        frame_width,
        content_width,
        geometry,
        current_column,
    );
    let frame_text_distance = twips_to_points(paragraph.style.frame_text_distance_twips.max(0));
    let exclusion_width =
        (frame_width + frame_text_distance).min((geometry.width - frame_left).max(1.0));
    let frame_vertical_text_distance =
        paragraph_frame_vertical_text_distance(&paragraph.style, geometry.height);
    let (requested_exclusion_y, requested_exclusion_height) =
        paragraph_frame_exclusion_vertical_geometry(
            requested_frame_top,
            frame_height,
            frame_vertical_text_distance,
            geometry.height,
        );
    let requested_exclusion_top = requested_exclusion_y + requested_exclusion_height;
    let stacked_exclusion_top = stacked_absolute_frame_top(
        pages.last(),
        frame_left,
        exclusion_width,
        requested_exclusion_height,
        requested_exclusion_top,
        framed_lines
            .first()
            .and_then(|lines| lines.first())
            .map(|line| line.height)
            .unwrap_or(14.0)
            .max(1.0),
    );
    let frame_top_y = requested_frame_top + stacked_exclusion_top - requested_exclusion_top;
    let (exclusion_y, exclusion_height) = paragraph_frame_exclusion_vertical_geometry(
        frame_top_y,
        frame_height,
        frame_vertical_text_distance,
        geometry.height,
    );
    Some(FlowExclusion {
        x: frame_left,
        y: exclusion_y,
        width: exclusion_width,
        height: exclusion_height,
        wrap_side: paragraph_frame_wrap_side(paragraph),
        wrap_bottom_inset: 1.0,
        kind: FlowExclusionKind::ParagraphFrame,
    })
}

pub(in crate::layout) fn paragraph_frame_left(
    style: &ParagraphStyle,
    frame_width: f32,
    content_width: f32,
    geometry: PageGeometry,
    current_column: usize,
) -> f32 {
    let (anchor_left, anchor_width) = match style.frame_horizontal_anchor {
        StaticShapeHorizontalAnchor::Column => {
            (geometry.body_left(current_column), content_width.max(1.0))
        }
        StaticShapeHorizontalAnchor::Margin => {
            (geometry.margin_left, geometry.content_width.max(1.0))
        }
        StaticShapeHorizontalAnchor::Page => (0.0, geometry.width.max(1.0)),
    };
    let requested = style.frame_horizontal_alignment.map_or_else(
        || anchor_left + twips_to_points(style.frame_horizontal_offset_twips),
        |alignment| table_row_left(anchor_left, anchor_width, frame_width, alignment, geometry),
    );
    requested.clamp(0.0, (geometry.width - frame_width).max(0.0))
}

pub(in crate::layout) fn paragraph_frame_top(
    style: &ParagraphStyle,
    frame_height: f32,
    paragraph_top: f32,
    geometry: PageGeometry,
) -> f32 {
    let Some(alignment) = style.frame_vertical_alignment else {
        return shape_vertical_origin(style.frame_vertical_anchor, paragraph_top, &geometry)
            - twips_to_points(style.frame_vertical_offset_twips);
    };
    if style.frame_vertical_anchor == StaticShapeVerticalAnchor::Paragraph {
        return paragraph_top - twips_to_points(style.frame_vertical_offset_twips);
    }
    if alignment == ParagraphFrameVerticalAlignment::Inline
        && style.frame_vertical_anchor != StaticShapeVerticalAnchor::Page
    {
        return paragraph_top;
    }
    let (anchor_bottom, anchor_top) = match style.frame_vertical_anchor {
        StaticShapeVerticalAnchor::Paragraph => unreachable!(),
        StaticShapeVerticalAnchor::Margin => (
            geometry.margin_bottom,
            geometry.height - geometry.margin_top,
        ),
        StaticShapeVerticalAnchor::Page => (0.0, geometry.height),
    };
    let physical_alignment = match alignment {
        ParagraphFrameVerticalAlignment::Inside => {
            if geometry.facing_pages && geometry.physical_page_number % 2 == 0 {
                ParagraphFrameVerticalAlignment::Bottom
            } else {
                ParagraphFrameVerticalAlignment::Top
            }
        }
        ParagraphFrameVerticalAlignment::Outside => {
            if geometry.facing_pages && geometry.physical_page_number % 2 == 0 {
                ParagraphFrameVerticalAlignment::Top
            } else {
                ParagraphFrameVerticalAlignment::Bottom
            }
        }
        ParagraphFrameVerticalAlignment::Inline => ParagraphFrameVerticalAlignment::Top,
        other => other,
    };
    let page_edge_inset = if style.frame_vertical_anchor == StaticShapeVerticalAnchor::Page
        && matches!(
            alignment,
            ParagraphFrameVerticalAlignment::Inside | ParagraphFrameVerticalAlignment::Outside
        ) {
        WORD_PARAGRAPH_FRAME_PAGE_EDGE_INSET_POINTS
    } else {
        0.0
    };
    match physical_alignment {
        ParagraphFrameVerticalAlignment::Top => anchor_top - page_edge_inset,
        ParagraphFrameVerticalAlignment::Center => {
            anchor_bottom + ((anchor_top - anchor_bottom + frame_height) / 2.0)
        }
        ParagraphFrameVerticalAlignment::Bottom => anchor_bottom + frame_height + page_edge_inset,
        ParagraphFrameVerticalAlignment::Inside
        | ParagraphFrameVerticalAlignment::Outside
        | ParagraphFrameVerticalAlignment::Inline => unreachable!(),
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn layout_paragraph_with_auto_footnotes(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    paragraph: &Paragraph,
    suppress_contextual_space_before: bool,
    suppress_contextual_space_after: bool,
    render_between_border_after: bool,
    content_width: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    current_column: &mut usize,
    document: &Document,
    document_stats: DocumentStats,
    mut line_numbers: Option<&mut LineNumberState>,
    markers: &MarkerContext,
    font_provider: Option<&FontProvider>,
    mut auto_footnotes: Option<AutoFootnoteFlush<'_>>,
    mut page_advance_hook: Option<&mut dyn FnMut(&mut Vec<LayoutPage>, &mut f32, &PageGeometry)>,
    planned_flow_exclusions: Option<&HashMap<usize, Vec<PlannedAbsoluteFrameExclusion>>>,
) {
    let two_in_one_paragraph = word_two_in_one_layout_paragraph(paragraph);
    let paragraph = two_in_one_paragraph.as_ref().unwrap_or(paragraph);
    let mut markers = markers.clone();
    if paragraph.style.page_break_before && !pages.last().is_none_or(|page| page.items.is_empty()) {
        flush_auto_footnotes_before_page_advance(
            pages,
            cursor_y,
            document,
            &mut auto_footnotes,
            false,
            true,
            margin_bottom,
            geometry,
            document_stats,
            font_provider,
        );
        let previous_page_count = pages.len();
        start_new_page(pages, cursor_y, geometry, current_column);
        reset_line_numbers_for_page_restart(
            line_numbers.as_mut().map(|state| &mut **state),
            *geometry,
            previous_page_count,
            pages.len(),
        );
        run_page_advance_hook(
            &mut page_advance_hook,
            pages,
            cursor_y,
            geometry,
            previous_page_count,
        );
        markers = current_marker_context(pages, document_stats);
    }
    let previous_page_count = pages.len();
    if kept_paragraphs_would_advance_to_page(
        pages,
        cursor_y,
        paragraph,
        &[],
        content_width,
        margin_bottom,
        geometry,
        current_column,
        document,
        &markers,
        font_provider,
    ) {
        flush_auto_footnotes_before_page_advance(
            pages,
            cursor_y,
            document,
            &mut auto_footnotes,
            false,
            true,
            margin_bottom,
            geometry,
            document_stats,
            font_provider,
        );
    }
    start_new_page_for_kept_paragraphs(
        pages,
        cursor_y,
        paragraph,
        &[],
        content_width,
        margin_bottom,
        geometry,
        current_column,
        document,
        &markers,
        font_provider,
    );
    reset_line_numbers_for_page_restart(
        line_numbers.as_mut().map(|state| &mut **state),
        *geometry,
        previous_page_count,
        pages.len(),
    );
    run_page_advance_hook(
        &mut page_advance_hook,
        pages,
        cursor_y,
        geometry,
        previous_page_count,
    );
    let mut margin_left = geometry.body_left(*current_column);
    markers = current_marker_context(pages, document_stats);

    let paragraph_top = if suppress_contextual_space_before {
        0.0
    } else {
        twips_to_points(effective_space_before_twips(&paragraph.style))
    };
    *cursor_y -= paragraph_top;
    let wrapping_start_page_index = pages.len().saturating_sub(1);
    let mut wrapping_page_index = wrapping_start_page_index;
    let mut wrapping_column = *current_column;
    let mut wrapping_line_top = *cursor_y;
    let mut previous_source_line_top = *cursor_y;
    let mut wrapping_line_height = 0.0_f32;
    let wrapping_body_top = geometry.height - geometry.margin_top;
    let mut lines = wrap_paragraph_with_font_provider_dynamic_width(
        paragraph,
        content_width,
        &markers,
        document,
        font_provider,
        *cursor_y,
        |source_line_top, line_height| {
            if (source_line_top - previous_source_line_top).abs() > 0.01 {
                wrapping_line_top -= previous_source_line_top - source_line_top;
                previous_source_line_top = source_line_top;
                wrapping_line_height = 0.0;
            }
            if line_height > wrapping_line_height && wrapping_line_top - line_height < margin_bottom
            {
                if wrapping_column + 1 < geometry.column_count {
                    wrapping_column += 1;
                } else {
                    wrapping_page_index = wrapping_page_index.saturating_add(1);
                    wrapping_column = 0;
                }
                wrapping_line_top = wrapping_body_top;
            }
            wrapping_line_height = wrapping_line_height.max(line_height);
            let wrapping_margin_left = geometry.body_left(wrapping_column);
            let mut intervals = if wrapping_page_index == wrapping_start_page_index {
                wrapped_image_text_intervals_for_line(
                    pages,
                    wrapping_margin_left,
                    content_width,
                    wrapping_line_top,
                    line_height,
                )
                .unwrap_or_else(|| vec![(wrapping_margin_left, content_width)])
            } else {
                vec![(wrapping_margin_left, content_width)]
            };
            if let Some(exclusions) =
                planned_flow_exclusions.and_then(|planned| planned.get(&wrapping_page_index))
            {
                intervals = apply_flow_exclusions_to_text_intervals(
                    intervals,
                    exclusions,
                    wrapping_margin_left,
                    content_width,
                    wrapping_line_top,
                    line_height,
                );
            }
            intervals
                .into_iter()
                .map(|(left, width)| ((left - wrapping_margin_left).max(0.0), width))
                .collect()
        },
    );
    for line in &mut lines {
        line.height = apply_line_spacing_with_grid(line.height, &paragraph.style, *geometry);
        if !paragraph.style.line_spacing_multiple {
            line.natural_height = line.height;
        }
    }
    let line_count = lines.len();
    let mut rendered_current_block_content = false;
    for (line_idx, line) in lines.iter().enumerate() {
        let marker_margin_bottom = auto_footnotes.as_ref().and_then(|auto_footnotes| {
            max_footnote_reference_index_in_line(line).map(|idx| {
                footnote_reservation_margin_bottom(
                    document,
                    auto_footnotes.rendered_footnote_pages,
                    idx,
                    geometry.content_width,
                    *geometry,
                    document_stats,
                    font_provider,
                )
            })
        });
        if let Some(marker_margin_bottom) = marker_margin_bottom
            && let Some(auto_footnotes) = auto_footnotes.as_mut()
        {
            auto_footnotes.reserved_footnote_margin_bottom_on_page = Some(
                auto_footnotes
                    .reserved_footnote_margin_bottom_on_page
                    .map_or(marker_margin_bottom, |current| {
                        current.max(marker_margin_bottom)
                    }),
            );
        }
        let line_margin_bottom = auto_footnotes
            .as_ref()
            .and_then(|auto_footnotes| auto_footnotes.reserved_footnote_margin_bottom_on_page)
            .unwrap_or(margin_bottom);
        if should_advance_for_widow_control(
            &paragraph.style,
            &lines,
            line_idx,
            *cursor_y,
            line_margin_bottom,
        ) {
            flush_auto_footnotes_before_page_advance(
                pages,
                cursor_y,
                document,
                &mut auto_footnotes,
                rendered_current_block_content,
                *current_column + 1 >= geometry.column_count,
                margin_bottom,
                geometry,
                document_stats,
                font_provider,
            );
            let previous_page_count = pages.len();
            advance_column_or_page(pages, cursor_y, geometry, current_column);
            reset_line_numbers_for_page_restart(
                line_numbers.as_mut().map(|state| &mut **state),
                *geometry,
                previous_page_count,
                pages.len(),
            );
            run_page_advance_hook(
                &mut page_advance_hook,
                pages,
                cursor_y,
                geometry,
                previous_page_count,
            );
            margin_left = geometry.body_left(*current_column);
        }
        let top_border_reserve = if line_idx == 0 {
            paragraph_top_border_reserve(&paragraph.style, document)
        } else {
            0.0
        };
        if *cursor_y - top_border_reserve - line.height < line_margin_bottom {
            flush_auto_footnotes_before_page_advance(
                pages,
                cursor_y,
                document,
                &mut auto_footnotes,
                rendered_current_block_content,
                *current_column + 1 >= geometry.column_count,
                margin_bottom,
                geometry,
                document_stats,
                font_provider,
            );
            let previous_page_count = pages.len();
            advance_column_or_page(pages, cursor_y, geometry, current_column);
            reset_line_numbers_for_page_restart(
                line_numbers.as_mut().map(|state| &mut **state),
                *geometry,
                previous_page_count,
                pages.len(),
            );
            run_page_advance_hook(
                &mut page_advance_hook,
                pages,
                cursor_y,
                geometry,
                previous_page_count,
            );
            margin_left = geometry.body_left(*current_column);
        }
        *cursor_y -= top_border_reserve;

        let uses_segmented_flow = line.segmented_flow;
        let (line_margin_left, line_content_width) = if uses_segmented_flow {
            (margin_left, content_width)
        } else {
            wrapped_image_text_area_for_line(
                pages,
                margin_left,
                content_width,
                *cursor_y,
                line.height,
            )
            .unwrap_or((margin_left, content_width))
        };

        let x = aligned_x(
            line_margin_left,
            line_content_width,
            line.width,
            &paragraph.style,
            line_idx == 0,
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
            let shading_outset = 1.44;
            push_shading_rect(
                pages,
                document,
                (line_margin_left + line_left_indent - shading_outset).max(0.0),
                *cursor_y - line.height,
                paragraph_line_width(line_content_width, &paragraph.style, line_idx == 0)
                    + (shading_outset * 2.0),
                line.height,
                paragraph.style.shading_color_index,
                paragraph.style.shading_basis_points,
                paragraph.style.shading_pattern,
            );
        }
        push_paragraph_borders(
            pages,
            line_margin_left,
            line_content_width,
            &paragraph.style,
            line_idx,
            line_count,
            *cursor_y,
            line.height,
            document,
            render_between_border_after,
            true,
        );
        let word_spacing = justified_word_spacing(
            &line,
            &paragraph.style,
            paragraph_line_width(line_content_width, &paragraph.style, line_idx == 0),
            line_idx + 1 == line_count,
            document,
            font_provider,
        );
        let character_spacing = distributed_character_spacing(
            &line,
            &paragraph.style,
            paragraph_line_width(line_content_width, &paragraph.style, line_idx == 0),
            line_idx + 1 == line_count,
        );
        if let Some(state) = line_numbers.as_mut().map(|state| &mut **state) {
            push_passive_line_number(
                pages,
                line,
                line_margin_left,
                *cursor_y,
                *geometry,
                state,
                paragraph.style.suppress_line_numbers,
                document,
                font_provider,
            );
        }
        push_bar_tab_stops(pages, &paragraph.style, x, *cursor_y, line.height);
        push_line(
            pages,
            &line,
            x,
            *cursor_y,
            document,
            font_provider,
            word_spacing,
            character_spacing,
        );
        if let Some(auto_footnotes) = auto_footnotes.as_mut()
            && let Some(note_index) = max_footnote_reference_index_in_line(line)
        {
            auto_footnotes.rendered_footnote_index_on_page = Some(
                auto_footnotes
                    .rendered_footnote_index_on_page
                    .map_or(note_index, |current| current.max(note_index)),
            );
        }
        rendered_current_block_content = true;
        *cursor_y -= line.height;
        if line_idx + 1 < line_count
            && line
                .runs
                .iter()
                .any(|run| inline_image_marker_index(&run.text).is_some())
        {
            let text_line_height = line
                .runs
                .iter()
                .filter(|run| inline_image_marker_index(&run.text).is_none())
                .map(|run| flow_run_line_height(run, document, font_provider))
                .fold(0.0_f32, f32::max);
            *cursor_y -= text_line_height * 0.25;
        }
    }
    *cursor_y -=
        paragraph_bottom_border_reserve(&paragraph.style, render_between_border_after, document);
    if !suppress_contextual_space_after {
        *cursor_y -= twips_to_points(effective_space_after_twips(&paragraph.style));
    }
}

pub(in crate::layout) fn word_two_in_one_layout_paragraph(
    paragraph: &Paragraph,
) -> Option<Paragraph> {
    if !paragraph.style.two_in_one || paragraph.runs.is_empty() {
        return None;
    }

    let mut layout_paragraph = paragraph.clone();
    layout_paragraph.style.two_in_one = false;
    let mut saw_visible_character = false;
    let mut inserted_break = false;
    for run in &mut layout_paragraph.runs {
        run.style.font_size_scale_percent =
            ((run.style.font_size_scale_percent.max(1) + 1) / 2).max(1);
        if inserted_break {
            continue;
        }
        let mut break_byte = None;
        for (index, ch) in run.text.char_indices() {
            if ch.is_whitespace() && saw_visible_character {
                break_byte = Some(index + ch.len_utf8());
                break;
            }
            if !ch.is_whitespace() {
                saw_visible_character = true;
            }
        }
        if let Some(index) = break_byte {
            run.text.insert(index, '\n');
            inserted_break = true;
        }
    }
    Some(layout_paragraph)
}

pub(in crate::layout) fn run_page_advance_hook(
    page_advance_hook: &mut Option<&mut dyn FnMut(&mut Vec<LayoutPage>, &mut f32, &PageGeometry)>,
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    geometry: &PageGeometry,
    previous_page_count: usize,
) {
    if pages.len() <= previous_page_count {
        return;
    }
    if let Some(hook) = page_advance_hook.as_mut() {
        hook(pages, cursor_y, geometry);
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn flush_auto_footnotes_before_page_advance(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    document: &Document,
    auto_footnotes: &mut Option<AutoFootnoteFlush<'_>>,
    include_current_block: bool,
    will_start_new_page: bool,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) {
    if !will_start_new_page {
        return;
    }
    let Some(auto_footnotes) = auto_footnotes.as_mut() else {
        return;
    };
    if include_current_block
        && let Some(max_note_index) = auto_footnotes.rendered_footnote_index_on_page.take()
    {
        layout_footnotes_until_index(
            pages,
            cursor_y,
            document,
            auto_footnotes.rendered_footnote_pages,
            max_note_index,
            geometry.content_width,
            geometry.margin_left,
            margin_bottom,
            geometry,
            document_stats,
            font_provider,
        );
        auto_footnotes.reserved_footnote_margin_bottom_on_page = None;
        return;
    }
    if include_current_block {
        auto_footnotes.reserved_footnote_margin_bottom_on_page = None;
        return;
    }
    let block_index = if include_current_block {
        auto_footnotes.block_index
    } else if let Some(previous_block_index) = auto_footnotes.block_index.checked_sub(1) {
        previous_block_index
    } else {
        return;
    };
    layout_footnotes_until_block(
        pages,
        cursor_y,
        document,
        auto_footnotes.rendered_footnote_pages,
        block_index,
        geometry.content_width,
        geometry.margin_left,
        margin_bottom,
        geometry,
        document_stats,
        font_provider,
    );
    auto_footnotes.reserved_footnote_margin_bottom_on_page = None;
}

pub(in crate::layout) fn max_footnote_reference_index_in_line(line: &Line) -> Option<usize> {
    line.runs
        .iter()
        .filter_map(|run| max_footnote_reference_index_in_text(&run.text))
        .max()
}

pub(in crate::layout) fn max_footnote_reference_index_in_text(text: &str) -> Option<usize> {
    let mut max_index = None;
    let mut search_start = 0;
    while let Some((start, end)) = next_footnote_reference_marker_range(text, search_start) {
        if let Some(index) = parse_footnote_reference_marker_id(&text[start..end]) {
            max_index = Some(max_index.map_or(index, |current: usize| current.max(index)));
        }
        search_start = end;
    }
    max_index
}

pub(in crate::layout) fn reset_line_numbers_for_page_restart(
    state: Option<&mut LineNumberState>,
    geometry: PageGeometry,
    previous_page_count: usize,
    current_page_count: usize,
) {
    if current_page_count > previous_page_count
        && geometry.line_numbering.restart == LineNumberRestart::Page
        && let Some(state) = state
    {
        state.reset_for_geometry(geometry);
    }
}

pub(in crate::layout) fn wrapped_image_text_area_for_line(
    pages: &[LayoutPage],
    margin_left: f32,
    content_width: f32,
    line_top_y: f32,
    line_height: f32,
) -> Option<(f32, f32)> {
    wrapped_image_text_intervals_for_line(
        pages,
        margin_left,
        content_width,
        line_top_y,
        line_height,
    )?
    .into_iter()
    .max_by(|(left_a, width_a), (left_b, width_b)| {
        width_a
            .total_cmp(width_b)
            .then_with(|| left_b.total_cmp(left_a))
    })
}

pub(in crate::layout) fn wrapped_image_text_intervals_for_line(
    pages: &[LayoutPage],
    margin_left: f32,
    content_width: f32,
    line_top_y: f32,
    line_height: f32,
) -> Option<Vec<(f32, f32)>> {
    let page = pages.last()?;
    let content_right = margin_left + content_width;
    let line_bottom_y = line_top_y - line_height.max(0.0);
    let mut free_intervals = vec![(margin_left, content_right)];
    for exclusion in &page.flow_exclusions {
        let excluded_left = exclusion.x.max(margin_left);
        let excluded_right = (exclusion.x + exclusion.width).min(content_right);
        let excluded_bottom = exclusion.y + exclusion.wrap_bottom_inset.max(0.0);
        let excluded_top = exclusion.y + exclusion.height;
        free_intervals = subtract_flow_exclusion_from_intervals(
            free_intervals,
            margin_left,
            content_right,
            line_top_y,
            line_bottom_y,
            excluded_left,
            excluded_right,
            excluded_bottom,
            excluded_top,
            exclusion.wrap_side,
        );
    }
    for item in &page.items {
        let LayoutItem::Image(image) = item else {
            continue;
        };
        let Some(placement) = image.image.placement else {
            continue;
        };
        if !placement.text_wrap {
            continue;
        }
        let image_left = image.x.max(margin_left);
        let image_right = (image.x + image.width).min(content_right);
        if image_right <= margin_left || image_left >= content_right {
            continue;
        }
        let top_gap = twips_to_points(placement.wrap_margin_top_twips.max(0));
        let bottom_gap = twips_to_points(placement.wrap_margin_bottom_twips.max(0));
        let image_bottom = image.y - bottom_gap;
        let image_top = image.y + image.height + top_gap;
        if line_top_y <= image_bottom || line_bottom_y >= image_top {
            continue;
        }
        let left_gap = twips_to_points(placement.wrap_margin_left_twips.max(0));
        let right_gap = twips_to_points(placement.wrap_margin_right_twips.max(0));
        let excluded_left = (image_left - left_gap).max(margin_left);
        let excluded_right = (image_right + right_gap).min(content_right);
        free_intervals = subtract_flow_exclusion_from_intervals(
            free_intervals,
            margin_left,
            content_right,
            line_top_y,
            line_bottom_y,
            excluded_left,
            excluded_right,
            image_bottom,
            image_top,
            placement.wrap_side,
        );
    }
    Some(
        free_intervals
            .into_iter()
            .map(|(left, mut right)| {
                // Word allows the final glyph box beside a floating object to
                // overhang the nominal body edge by roughly 2.8 points. Keep
                // that tolerance bounded to the right-side interval ending at
                // the content edge; ordinary lines and interior gaps retain
                // their exact authored bounds.
                if left > margin_left + f32::EPSILON && (right - content_right).abs() < f32::EPSILON
                {
                    right += 3.0;
                }
                (left, (right - left).max(0.0))
            })
            .filter(|(_, width)| *width >= 12.0)
            .collect(),
    )
}

pub(in crate::layout) fn apply_flow_exclusions_to_text_intervals(
    intervals: Vec<(f32, f32)>,
    exclusions: &[PlannedAbsoluteFrameExclusion],
    margin_left: f32,
    content_width: f32,
    line_top_y: f32,
    line_height: f32,
) -> Vec<(f32, f32)> {
    let content_right = margin_left + content_width;
    let line_bottom_y = line_top_y - line_height.max(0.0);
    let mut free_intervals = intervals
        .into_iter()
        .map(|(left, width)| (left, (left + width).min(content_right)))
        .collect::<Vec<_>>();
    for planned in exclusions {
        let exclusion = &planned.exclusion;
        let excluded_left = exclusion.x.max(margin_left);
        let excluded_right = (exclusion.x + exclusion.width).min(content_right);
        let excluded_bottom = exclusion.y + exclusion.wrap_bottom_inset.max(0.0);
        let excluded_top = exclusion.y + exclusion.height;
        free_intervals = subtract_flow_exclusion_from_intervals(
            free_intervals,
            margin_left,
            content_right,
            line_top_y,
            line_bottom_y,
            excluded_left,
            excluded_right,
            excluded_bottom,
            excluded_top,
            exclusion.wrap_side,
        );
    }
    free_intervals
        .into_iter()
        .map(|(left, mut right)| {
            if left > margin_left + f32::EPSILON && (right - content_right).abs() < f32::EPSILON {
                right += 3.0;
            }
            (left, (right - left).max(0.0))
        })
        .filter(|(_, width)| *width >= 12.0)
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn subtract_flow_exclusion_from_intervals(
    free_intervals: Vec<(f32, f32)>,
    margin_left: f32,
    content_right: f32,
    line_top_y: f32,
    line_bottom_y: f32,
    excluded_left: f32,
    excluded_right: f32,
    excluded_bottom: f32,
    excluded_top: f32,
    wrap_side: StaticImageWrapSide,
) -> Vec<(f32, f32)> {
    if excluded_right <= margin_left
        || excluded_left >= content_right
        || excluded_left >= excluded_right
        || line_top_y <= excluded_bottom
        || line_bottom_y >= excluded_top
    {
        return free_intervals;
    }
    let side_left = match wrap_side {
        StaticImageWrapSide::Left => excluded_left,
        StaticImageWrapSide::Right => margin_left,
        StaticImageWrapSide::Both | StaticImageWrapSide::Largest => excluded_left,
    };
    let side_right = match wrap_side {
        StaticImageWrapSide::Left => content_right,
        StaticImageWrapSide::Right => excluded_right,
        StaticImageWrapSide::Both | StaticImageWrapSide::Largest => excluded_right,
    };
    let mut next_intervals = Vec::with_capacity(free_intervals.len() + 1);
    for (left, right) in free_intervals {
        if side_right <= left || side_left >= right {
            next_intervals.push((left, right));
            continue;
        }
        if left < side_left {
            next_intervals.push((left, side_left));
        }
        if side_right < right {
            next_intervals.push((side_right, right));
        }
    }
    next_intervals
}

pub(in crate::layout) fn reset_line_numbers_for_section_restart(
    state: Option<&mut LineNumberState>,
    geometry: PageGeometry,
) {
    if geometry.line_numbering.restart == LineNumberRestart::Section
        && let Some(state) = state
    {
        state.reset_for_geometry(geometry);
    }
}

pub(in crate::layout) fn next_block_is_section_settings(
    document: &Document,
    block_idx: usize,
) -> bool {
    document
        .blocks
        .get(block_idx.saturating_add(1))
        .is_some_and(|block| matches!(block, Block::SectionSettings(_)))
}

pub(in crate::layout) fn push_passive_line_number(
    pages: &mut [LayoutPage],
    line: &Line,
    margin_left: f32,
    top_y: f32,
    geometry: PageGeometry,
    state: &mut LineNumberState,
    suppress_number: bool,
    document: &Document,
    font_provider: Option<&FontProvider>,
) {
    if !geometry.line_numbering.enabled || !line_has_visible_text(line) {
        return;
    }

    let current = state.next.max(1);
    let step = geometry.line_numbering.step.max(1);
    let start = geometry.line_numbering.start.max(1);
    let should_render = current >= start && (current - start) % step == 0;
    state.next = state.next.saturating_add(1).max(1);
    if suppress_number || !should_render {
        return;
    }

    let mut style = CharacterStyle::default();
    style.font_size_half_points = 24;
    let text = current.to_string();
    let width = measure_text_with_family(&text, &style, PdfFontFamily::Helvetica);
    let distance = twips_to_points(geometry.line_numbering.distance_twips.max(0));
    let x = (margin_left - distance - width).max(0.0);
    let baseline_y = top_y - line.height
        + (line.height * 0.25)
        + word_compatible_source_line_baseline_delta_points(line, document, font_provider);
    let Some(page) = pages.last_mut() else {
        return;
    };
    page.items.push(LayoutItem::Text(TextFragment {
        text,
        x,
        baseline_y,
        rotation: TextRotation::None,
        color: PdfColor::default(),
        font_family: PdfFontFamily::Helvetica,
        word_spacing: 0.0,
        style,
    }));
}

pub(in crate::layout) fn line_has_visible_text(line: &Line) -> bool {
    line.runs.iter().any(|run| {
        !run.style.hidden
            && !is_passive_advance_marker(&run.text)
            && !is_passive_math_inline_lead_marker(&run.text)
            && !run.text.trim().is_empty()
            && parse_bookmark_page_marker_id(&run.text, BOOKMARK_PAGE_ANCHOR_MARKER).is_none()
            && parse_bookmark_page_marker_id(&run.text, BOOKMARK_PAGE_REF_MARKER).is_none()
    })
}

pub(in crate::layout) fn effective_space_before_twips(style: &ParagraphStyle) -> i32 {
    if style.auto_space_before {
        AUTO_PARAGRAPH_SPACING_TWIPS
    } else {
        style.space_before_twips
    }
}

pub(in crate::layout) fn effective_space_after_twips(style: &ParagraphStyle) -> i32 {
    if style.auto_space_after {
        AUTO_PARAGRAPH_SPACING_TWIPS
    } else {
        style.space_after_twips
    }
}

pub(in crate::layout) fn should_advance_for_widow_control(
    style: &ParagraphStyle,
    lines: &[Line],
    line_idx: usize,
    cursor_y: f32,
    margin_bottom: f32,
) -> bool {
    if !style.widow_control || lines.len() < 2 {
        return false;
    }
    let Some(next_line) = lines.get(line_idx + 1) else {
        return false;
    };
    let current_line = &lines[line_idx];
    let current_fits = cursor_y - current_line.height >= margin_bottom;
    let next_fits = cursor_y - current_line.height - next_line.height >= margin_bottom;
    if !current_fits || next_fits {
        return false;
    }

    line_idx == 0 || line_idx + 2 == lines.len()
}
