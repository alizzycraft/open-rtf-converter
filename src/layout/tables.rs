//! Table preparation, splitting, cell geometry, spacing, and borders.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn layout_table(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    table: &Table,
    content_width: f32,
    mut margin_left: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    current_column: &mut usize,
    document: &Document,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) -> bool {
    if table.rows.is_empty() {
        return false;
    }

    let column_count = table
        .rows
        .iter()
        .map(|row| row.cells.len())
        .max()
        .unwrap_or(0)
        .max(1);
    let header_rows = table
        .rows
        .iter()
        .take_while(|row| row.repeat_header)
        .collect::<Vec<_>>();
    let table_flow_cursor_y = *cursor_y;
    let mut has_flow_exclusion = false;
    let mut clipped_past_page_end = false;

    for (row_idx, row) in table.rows.iter().enumerate() {
        let column_widths = table_row_column_widths(table, row, column_count, content_width);
        let table_width: f32 = column_widths.iter().sum();
        let next_row = table.rows.get(row_idx + 1);
        let mut prepared = prepare_table_row(
            row,
            &column_widths,
            content_width / column_count as f32,
            &current_marker_context(pages, document_stats),
            document,
            font_provider,
        );
        let top_offset = table_row_vertical_offset_points(row, prepared.row_height);
        let row_wrap_margins = table_row_wrap_margin_points(row);
        let top_margin =
            row_wrap_margins.top.max(0.0) + distributed_table_row_top_spacing_points(row);
        let bottom_margin = row_wrap_margins.bottom.max(0.0);
        if table_row_has_wrap_margins(row) && row.vertical_offset_twips != 0 {
            // Positioned Word tables use their vertical offset from the page
            // margin anchor, independently for each row. Wrap distance expands
            // only the surrounding exclusion and does not move the table.
            *cursor_y = geometry.height - geometry.margin_top + top_margin;
        }

        if row.no_overlap {
            let adjusted_top = non_overlapping_table_row_top(
                pages,
                row,
                &prepared,
                *cursor_y - top_offset - top_margin,
                content_width,
                margin_left,
                table_width,
            );
            *cursor_y = adjusted_top + top_offset + top_margin;
        }

        if row.keep_with_next
            && !pages.last().is_none_or(|page| page.items.is_empty())
            && let Some(next_row) = next_row
        {
            let next_prepared = prepare_table_row(
                next_row,
                &column_widths,
                content_width / column_count as f32,
                &current_marker_context(pages, document_stats),
                document,
                font_provider,
            );
            let next_margins = table_row_wrap_margin_points(next_row);
            let kept_height = top_offset
                + top_margin
                + prepared.row_height
                + bottom_margin
                + next_margins.top.max(0.0)
                + next_prepared.row_height
                + next_margins.bottom.max(0.0);
            let usable_height = (geometry.height - geometry.margin_top - margin_bottom).max(0.0);
            if kept_height <= usable_height && *cursor_y - kept_height < margin_bottom {
                advance_column_or_page(pages, cursor_y, geometry, current_column);
                margin_left = geometry.body_left(*current_column);

                if !row.repeat_header {
                    push_repeating_table_headers(
                        pages,
                        cursor_y,
                        &header_rows,
                        &column_widths,
                        content_width,
                        column_count,
                        margin_bottom,
                        margin_left,
                        table_width,
                        document,
                        document_stats,
                        font_provider,
                        table.borders_visible,
                    );
                }
            }
        }

        if should_split_tall_table_row(row, &prepared, *geometry, margin_bottom) {
            if *cursor_y - 14.0 < margin_bottom {
                advance_column_or_page(pages, cursor_y, geometry, current_column);
                margin_left = geometry.body_left(*current_column);
                if !row.repeat_header {
                    push_repeating_table_headers(
                        pages,
                        cursor_y,
                        &header_rows,
                        &column_widths,
                        content_width,
                        column_count,
                        margin_bottom,
                        margin_left,
                        table_width,
                        document,
                        document_stats,
                        font_provider,
                        table.borders_visible,
                    );
                }
            }
            push_split_table_row(
                pages,
                cursor_y,
                row,
                prepared,
                &header_rows,
                &column_widths,
                column_count,
                content_width,
                margin_left,
                table_width,
                margin_bottom,
                geometry,
                current_column,
                document,
                document_stats,
                font_provider,
                table.borders_visible,
                next_row,
            );
            continue;
        }

        let row_top = *cursor_y - top_offset - top_margin;
        let clips_terminal_no_wrap_row = row_idx + 1 == table.rows.len()
            && row.height_twips.is_some()
            && row_top > 0.0
            && row_top - prepared.row_height < 0.0
            && table_row_contains_no_wrap_text(row);
        if !clips_terminal_no_wrap_row
            && *cursor_y - top_offset - top_margin - prepared.row_height < margin_bottom
        {
            advance_column_or_page(pages, cursor_y, geometry, current_column);
            margin_left = geometry.body_left(*current_column);

            if !row.repeat_header {
                push_repeating_table_headers(
                    pages,
                    cursor_y,
                    &header_rows,
                    &column_widths,
                    content_width,
                    column_count,
                    margin_bottom,
                    margin_left,
                    table_width,
                    document,
                    document_stats,
                    font_provider,
                    table.borders_visible,
                );
                prepared = prepare_table_row(
                    row,
                    &column_widths,
                    content_width / column_count as f32,
                    &current_marker_context(pages, document_stats),
                    document,
                    font_provider,
                );
            }
        }
        clipped_past_page_end |= clips_terminal_no_wrap_row;

        let vertical_span_heights = table_vertical_span_heights(
            table,
            row_idx,
            &prepared,
            &column_widths,
            content_width / column_count as f32,
            current_marker_context(pages, document_stats),
            document,
            font_provider,
            (*cursor_y - top_offset - top_margin - geometry.margin_bottom).max(prepared.row_height),
        );
        let mut row_cursor_y = *cursor_y - top_offset - top_margin;
        let row_top = row_cursor_y;
        push_table_row(
            pages,
            &mut row_cursor_y,
            row,
            &prepared,
            &vertical_span_heights,
            content_width,
            margin_left,
            table_width,
            document,
            font_provider,
            table.borders_visible,
            true,
            next_row,
        );
        push_table_row_flow_exclusion(
            pages,
            row,
            row_top,
            row_cursor_y,
            content_width,
            margin_left,
            table_width,
        );
        has_flow_exclusion |= table_row_has_wrap_margins(row);
        *cursor_y = row_cursor_y - bottom_margin;
    }

    if has_flow_exclusion {
        *cursor_y = table_flow_cursor_y;
    } else {
        // Word reserves an authored bottom cell border below the final row.
        // Modern bottom cell padding already supplies that clearance; only a
        // borderless row without it carries half of the legacy `\trgaph`.
        // An omitted `\trgaph` therefore adds no synthetic flow gap.
        let following_gap = table
            .rows
            .last()
            .map(|row| {
                let bottom_border_width = row
                    .cells
                    .iter()
                    .filter(|cell| cell.borders.bottom.visible)
                    .map(|cell| table_border_stroke(&cell.borders.bottom, document).0)
                    .fold(0.0, f32::max);
                if bottom_border_width > 0.0 {
                    bottom_border_width
                } else if row
                    .cells
                    .iter()
                    .any(|cell| cell.padding.bottom_twips.is_some())
                {
                    0.0
                } else {
                    twips_to_points(row.cell_gap_twips.max(0)) * 0.5
                }
            })
            .unwrap_or(0.0);
        *cursor_y -= following_gap;
    }
    clipped_past_page_end
}

pub(in crate::layout) fn table_row_contains_no_wrap_text(row: &TableRow) -> bool {
    row.cells.iter().any(|cell| {
        cell.paragraphs
            .iter()
            .any(|paragraph| paragraph.style.no_wrap)
    })
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn push_repeating_table_headers(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    header_rows: &[&TableRow],
    column_widths: &[f32],
    content_width: f32,
    column_count: usize,
    margin_bottom: f32,
    margin_left: f32,
    table_width: f32,
    document: &Document,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
    borders_visible: bool,
) {
    for header_row in header_rows {
        let header = prepare_table_row(
            header_row,
            column_widths,
            content_width / column_count as f32,
            &current_marker_context(pages, document_stats),
            document,
            font_provider,
        );
        if *cursor_y - header.row_height < margin_bottom {
            break;
        }
        let header_vertical_span_heights = vec![header.row_height; header.visual_cells.len()];
        push_table_row(
            pages,
            cursor_y,
            header_row,
            &header,
            &header_vertical_span_heights,
            content_width,
            margin_left,
            table_width,
            document,
            font_provider,
            borders_visible,
            true,
            None,
        );
    }
}

pub(in crate::layout) fn should_split_tall_table_row(
    row: &TableRow,
    prepared: &PreparedTableRow,
    geometry: PageGeometry,
    margin_bottom: f32,
) -> bool {
    if row.keep_together
        || row.repeat_header
        || row.vertical_offset_twips != 0
        || table_row_has_wrap_margins(row)
        || row.height_twips.is_some_and(|height| height < 0)
    {
        return false;
    }
    if prepared
        .visual_cells
        .iter()
        .filter_map(|visual_cell| row.cells.get(visual_cell.cell_index))
        .any(|cell| cell.vertical_merge != TableCellVerticalMerge::None)
    {
        return false;
    }
    let usable_height = (geometry.height - geometry.margin_top - margin_bottom).max(14.0);
    let content_height = prepared_table_row_content_height(prepared);
    content_height > usable_height && prepared_table_row_has_lines(prepared)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn push_split_table_row(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    row: &TableRow,
    mut remaining: PreparedTableRow,
    header_rows: &[&TableRow],
    column_widths: &[f32],
    column_count: usize,
    content_width: f32,
    mut margin_left: f32,
    table_width: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    current_column: &mut usize,
    document: &Document,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
    borders_visible: bool,
    next_row: Option<&TableRow>,
) {
    let nested_repeat_header_lines = nested_repeat_header_lines(row, &remaining);
    let mut render_nested_child_grids = true;
    while prepared_table_row_has_lines(&remaining) {
        let available_height = (*cursor_y - margin_bottom).max(14.0);
        let Some(fragment) = split_prepared_table_row_fragment(&mut remaining, available_height)
        else {
            break;
        };
        let is_final_fragment = !prepared_table_row_has_lines(&remaining);
        let vertical_span_heights = vec![fragment.row_height; fragment.visual_cells.len()];
        push_table_row(
            pages,
            cursor_y,
            row,
            &fragment,
            &vertical_span_heights,
            content_width,
            margin_left,
            table_width,
            document,
            font_provider,
            borders_visible,
            render_nested_child_grids,
            is_final_fragment.then_some(()).and(next_row),
        );
        render_nested_child_grids = false;
        if !is_final_fragment {
            advance_column_or_page(pages, cursor_y, geometry, current_column);
            margin_left = geometry.body_left(*current_column);
            push_repeating_table_headers(
                pages,
                cursor_y,
                header_rows,
                column_widths,
                content_width,
                column_count,
                margin_bottom,
                margin_left,
                table_width,
                document,
                document_stats,
                font_provider,
                borders_visible,
            );
            prepend_nested_repeat_header_lines(&mut remaining, &nested_repeat_header_lines);
        }
    }
}

pub(in crate::layout) fn nested_repeat_header_lines(
    row: &TableRow,
    prepared: &PreparedTableRow,
) -> Vec<Vec<PreparedCellLine>> {
    prepared
        .visual_cells
        .iter()
        .enumerate()
        .map(|(idx, visual_cell)| {
            let Some(cell) = row.cells.get(visual_cell.cell_index) else {
                return Vec::new();
            };
            let Some(nested_table) = cell.nested_table.as_deref() else {
                return Vec::new();
            };
            let header_count = nested_table
                .rows
                .iter()
                .take_while(|nested_row| nested_row.repeat_header)
                .count();
            if header_count == 0 {
                return Vec::new();
            }
            let paragraph_prefix = cell.paragraphs.len();
            prepared
                .cell_lines
                .get(idx)
                .into_iter()
                .flatten()
                .filter(|line| {
                    line.paragraph_index
                        .checked_sub(paragraph_prefix)
                        .is_some_and(|nested_row_index| nested_row_index < header_count)
                })
                .cloned()
                .collect()
        })
        .collect()
}

pub(in crate::layout) fn prepend_nested_repeat_header_lines(
    remaining: &mut PreparedTableRow,
    nested_repeat_headers: &[Vec<PreparedCellLine>],
) {
    for (idx, lines) in remaining.cell_lines.iter_mut().enumerate() {
        let Some(headers) = nested_repeat_headers.get(idx) else {
            continue;
        };
        if headers.is_empty() || lines.is_empty() {
            continue;
        }
        let header_paragraph_indices = headers
            .iter()
            .map(|header| header.paragraph_index)
            .collect::<Vec<_>>();
        let starts_inside_header = header_paragraph_indices.contains(&lines[0].paragraph_index);
        let has_body_line = lines
            .iter()
            .any(|line| !header_paragraph_indices.contains(&line.paragraph_index));
        if starts_inside_header || !has_body_line {
            continue;
        }
        let mut repeated = Vec::with_capacity(headers.len() + lines.len());
        repeated.extend(headers.iter().cloned());
        repeated.append(lines);
        *lines = repeated;
    }
}

pub(in crate::layout) fn split_prepared_table_row_fragment(
    remaining: &mut PreparedTableRow,
    max_height: f32,
) -> Option<PreparedTableRow> {
    if !prepared_table_row_has_lines(remaining) {
        return None;
    }

    let mut fragment = PreparedTableRow {
        visual_cells: remaining.visual_cells.clone(),
        cell_lines: Vec::with_capacity(remaining.cell_lines.len()),
        cell_paddings: remaining.cell_paddings.clone(),
        cell_spacings: remaining.cell_spacings.clone(),
        cell_text_directions: remaining.cell_text_directions.clone(),
        top_border_inset: remaining.top_border_inset,
        row_height: 14.0,
    };
    let mut consumed = Vec::with_capacity(remaining.cell_lines.len());
    for (idx, lines) in remaining.cell_lines.iter().enumerate() {
        let fixed = remaining
            .cell_paddings
            .get(idx)
            .zip(remaining.cell_spacings.get(idx))
            .map(|(padding, spacing)| padding.top + padding.bottom + spacing.top + spacing.bottom)
            .unwrap_or(0.0);
        let capacity = (max_height - fixed).max(0.0);
        let mut used = 0.0;
        let mut take = 0usize;
        let mut line_index = 0usize;
        while line_index < lines.len() {
            let line = &lines[line_index];
            if line.style.keep_together || line.style.keep_with_next {
                let mut group_end = line_index + 1;
                while group_end < lines.len()
                    && lines[group_end].paragraph_index == line.paragraph_index
                {
                    group_end += 1;
                }
                if line.style.keep_with_next && group_end < lines.len() {
                    let next_paragraph_index = lines[group_end].paragraph_index;
                    while group_end < lines.len()
                        && lines[group_end].paragraph_index == next_paragraph_index
                    {
                        group_end += 1;
                    }
                }
                let group_height = lines[line_index..group_end]
                    .iter()
                    .map(|group_line| {
                        prepared_cell_line_extent(group_line, group_line.text_direction)
                            + group_line.space_before
                            + group_line.space_after
                    })
                    .sum::<f32>();
                if used > 0.0 && used + group_height > capacity {
                    break;
                }
                if used == 0.0 && group_height > capacity {
                    take += 1;
                    break;
                }
                take = group_end;
                used += group_height;
                line_index = group_end;
                continue;
            }

            let height = prepared_cell_line_extent(line, line.text_direction)
                + line.space_before
                + line.space_after;
            if take == 0 || used + height <= capacity {
                take += 1;
                used += height;
                line_index += 1;
            } else {
                break;
            }
        }
        consumed.push(take.min(lines.len()));
    }

    if consumed.iter().all(|count| *count == 0)
        && let Some((idx, _)) = remaining
            .cell_lines
            .iter()
            .enumerate()
            .find(|(_, lines)| !lines.is_empty())
    {
        consumed[idx] = 1;
    }

    for (idx, take) in consumed.into_iter().enumerate() {
        let fragment_lines = remaining.cell_lines[idx].drain(..take).collect::<Vec<_>>();
        fragment.cell_lines.push(fragment_lines);
    }
    fragment.row_height = prepared_table_row_content_height(&fragment);
    remaining.row_height = prepared_table_row_content_height(remaining);
    Some(fragment)
}

pub(in crate::layout) fn prepared_table_row_has_lines(prepared: &PreparedTableRow) -> bool {
    prepared.cell_lines.iter().any(|lines| !lines.is_empty())
}

pub(in crate::layout) fn prepared_table_row_content_height(prepared: &PreparedTableRow) -> f32 {
    let height = prepared
        .cell_lines
        .iter()
        .zip(prepared.cell_text_directions.iter())
        .zip(prepared.cell_paddings.iter())
        .zip(prepared.cell_spacings.iter())
        .map(|(((lines, direction), padding), spacing)| {
            prepared_cell_lines_height(lines, *direction)
                + padding.top
                + padding.bottom
                + spacing.top
                + spacing.bottom
        })
        .fold(0.0, f32::max);
    if prepared_table_row_has_lines(prepared) {
        height
    } else {
        height.max(14.0)
    }
}

pub(in crate::layout) fn prepared_cell_lines_height(
    lines: &[PreparedCellLine],
    direction: TableCellTextDirection,
) -> f32 {
    let line_extent = |line: &PreparedCellLine| {
        prepared_cell_line_extent(line, line.text_direction) + line.space_before + line.space_after
    };
    match direction {
        TableCellTextDirection::LeftToRightTopToBottom => {
            lines.iter().map(line_extent).sum::<f32>()
        }
        TableCellTextDirection::TopToBottomRightToLeft
        | TableCellTextDirection::BottomToTopLeftToRight => {
            lines.iter().map(line_extent).fold(0.0, f32::max)
        }
    }
}

pub(in crate::layout) fn prepared_cell_line_extent(
    line: &PreparedCellLine,
    direction: TableCellTextDirection,
) -> f32 {
    match direction {
        TableCellTextDirection::LeftToRightTopToBottom => line.line.height,
        TableCellTextDirection::TopToBottomRightToLeft
        | TableCellTextDirection::BottomToTopLeftToRight => line.line.width.max(line.line.height),
    }
}

pub(in crate::layout) fn table_cell_text_rotation(
    direction: TableCellTextDirection,
) -> TextRotation {
    match direction {
        TableCellTextDirection::LeftToRightTopToBottom => TextRotation::None,
        TableCellTextDirection::TopToBottomRightToLeft => TextRotation::Clockwise90,
        TableCellTextDirection::BottomToTopLeftToRight => TextRotation::CounterClockwise90,
    }
}

pub(in crate::layout) fn table_row_vertical_offset_points(row: &TableRow, row_height: f32) -> f32 {
    if let Some(base_offset) = dynamic_table_row_vertical_offset_twips(
        row.vertical_offset_twips,
        TABLE_ROW_DYNAMIC_VERTICAL_CENTER_OFFSET_BASE,
    ) {
        return twips_to_points(base_offset) - (row_height / 2.0);
    }
    if let Some(base_offset) = dynamic_table_row_vertical_offset_twips(
        row.vertical_offset_twips,
        TABLE_ROW_DYNAMIC_VERTICAL_BOTTOM_OFFSET_BASE,
    ) {
        return twips_to_points(base_offset) - row_height;
    }
    twips_to_points(row.vertical_offset_twips)
}

pub(in crate::layout) fn dynamic_table_row_vertical_offset_twips(
    value: i32,
    base: i32,
) -> Option<i32> {
    let offset = value.checked_sub(base)?;
    if offset.abs() <= TABLE_ROW_DYNAMIC_VERTICAL_OFFSET_SPAN_TWIPS {
        Some(offset)
    } else {
        None
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub(in crate::layout) struct TableRowWrapMarginPoints {
    pub(in crate::layout) left: f32,
    pub(in crate::layout) right: f32,
    pub(in crate::layout) top: f32,
    pub(in crate::layout) bottom: f32,
}

pub(in crate::layout) fn table_row_wrap_margin_points(row: &TableRow) -> TableRowWrapMarginPoints {
    TableRowWrapMarginPoints {
        left: twips_to_points(row.wrap_margins.left_twips.max(0)),
        right: twips_to_points(row.wrap_margins.right_twips.max(0)),
        top: twips_to_points(row.wrap_margins.top_twips.max(0)),
        bottom: twips_to_points(row.wrap_margins.bottom_twips.max(0)),
    }
}

pub(in crate::layout) fn table_row_has_wrap_margins(row: &TableRow) -> bool {
    let TableRowWrapMargins {
        left_twips,
        right_twips,
        top_twips,
        bottom_twips,
    } = row.wrap_margins;
    left_twips > 0 || right_twips > 0 || top_twips > 0 || bottom_twips > 0
}

pub(in crate::layout) fn non_overlapping_table_row_top(
    pages: &[LayoutPage],
    row: &TableRow,
    prepared: &PreparedTableRow,
    mut row_top: f32,
    content_width: f32,
    margin_left: f32,
    table_width: f32,
) -> f32 {
    if !table_row_has_wrap_margins(row) {
        return row_top;
    }
    let Some(page) = pages.last() else {
        return row_top;
    };
    let wrap_margins = table_row_wrap_margin_points(row);
    let row_left =
        positioned_table_row_left(row, margin_left, content_width, table_width, page.geometry);
    let row_excluded_left = (row_left - wrap_margins.left).max(margin_left);
    let row_excluded_right = (row_left + table_width + wrap_margins.right)
        .min(margin_left + content_width)
        .max(row_excluded_left);

    let mut changed = true;
    while changed {
        changed = false;
        let row_excluded_top = row_top + wrap_margins.top;
        let row_excluded_bottom = row_top - prepared.row_height - wrap_margins.bottom;
        for exclusion in &page.flow_exclusions {
            let exclusion_left = exclusion.x;
            let exclusion_right = exclusion.x + exclusion.width;
            let exclusion_bottom = exclusion.y;
            let exclusion_top = exclusion.y + exclusion.height;
            let horizontally_overlaps =
                row_excluded_left < exclusion_right && row_excluded_right > exclusion_left;
            let vertically_overlaps =
                row_excluded_bottom < exclusion_top && row_excluded_top > exclusion_bottom;
            if horizontally_overlaps && vertically_overlaps {
                let next_top = exclusion_bottom - wrap_margins.top;
                if next_top < row_top {
                    row_top = next_top;
                    changed = true;
                }
            }
        }
    }
    row_top
}

pub(in crate::layout) fn push_table_row_flow_exclusion(
    pages: &mut [LayoutPage],
    row: &TableRow,
    row_top: f32,
    row_bottom: f32,
    content_width: f32,
    margin_left: f32,
    table_width: f32,
) {
    if !table_row_has_wrap_margins(row) {
        return;
    }
    let Some(page) = pages.last_mut() else {
        return;
    };
    let wrap_margins = table_row_wrap_margin_points(row);
    let row_left =
        positioned_table_row_left(row, margin_left, content_width, table_width, page.geometry);
    let excluded_left = (row_left - wrap_margins.left).max(margin_left);
    let excluded_right = (row_left + table_width + wrap_margins.right)
        .min(margin_left + content_width)
        .max(excluded_left);
    let excluded_bottom = row_bottom - wrap_margins.bottom;
    let excluded_top = row_top + wrap_margins.top;
    let height = (excluded_top - excluded_bottom).max(0.0);
    let width = (excluded_right - excluded_left).max(0.0);
    if width < 1.0 || height < 1.0 {
        return;
    }
    page.flow_exclusions.push(FlowExclusion {
        x: excluded_left,
        y: excluded_bottom,
        width,
        height,
        wrap_side: StaticImageWrapSide::Both,
        wrap_bottom_inset: 0.0,
        kind: FlowExclusionKind::Generic,
    });
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn preview_immediately_following_floating_table_exclusion(
    pages: &mut [LayoutPage],
    table: &Table,
    content_width: f32,
    margin_left: f32,
    geometry: PageGeometry,
    document: &Document,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) -> bool {
    let Some(row) = table
        .rows
        .first()
        .filter(|row| table_row_has_wrap_margins(row) && row.vertical_offset_twips != 0)
    else {
        return false;
    };
    let column_count = table
        .rows
        .iter()
        .map(|row| row.cells.len())
        .max()
        .unwrap_or(0)
        .max(1);
    let column_widths = table_row_column_widths(table, row, column_count, content_width);
    let table_width = column_widths.iter().sum();
    let prepared = prepare_table_row(
        row,
        &column_widths,
        content_width / column_count as f32,
        &current_marker_context(pages, document_stats),
        document,
        font_provider,
    );
    let row_top = geometry.height
        - geometry.margin_top
        - table_row_vertical_offset_points(row, prepared.row_height);
    push_table_row_flow_exclusion(
        pages,
        row,
        row_top,
        row_top - prepared.row_height,
        content_width,
        margin_left,
        table_width,
    );
    true
}

pub(in crate::layout) fn rotated_table_cell_line_origin(
    content_left: f32,
    content_width: f32,
    line_top: f32,
    line: &Line,
    direction: TableCellTextDirection,
    cross_offset: f32,
) -> (f32, f32) {
    match direction {
        TableCellTextDirection::LeftToRightTopToBottom => (content_left, line_top),
        TableCellTextDirection::TopToBottomRightToLeft => (
            content_left + content_width - (line.height * 0.75) - cross_offset,
            line_top + (line.height * 0.75),
        ),
        TableCellTextDirection::BottomToTopLeftToRight => (
            content_left + (line.height * 0.825) + cross_offset,
            line_top - line.width,
        ),
    }
}

pub(in crate::layout) fn rotated_paragraph_frame_line_origin(
    frame_left: f32,
    frame_width: f32,
    frame_top: f32,
    frame_height: f32,
    line: &Line,
    direction: TableCellTextDirection,
    cross_offset: f32,
) -> (f32, f32) {
    match direction {
        TableCellTextDirection::LeftToRightTopToBottom => (frame_left, frame_top),
        TableCellTextDirection::TopToBottomRightToLeft => (
            frame_left + frame_width - (line.height * 0.825) - cross_offset,
            frame_top + (line.height * 0.75),
        ),
        TableCellTextDirection::BottomToTopLeftToRight => (
            frame_left + (line.height * 0.825) + cross_offset,
            frame_top - frame_height + (line.height * 0.75),
        ),
    }
}

pub(in crate::layout) fn paragraph_frame_vertical_column_advance(line: &Line) -> f32 {
    let word_pitch = line
        .runs
        .iter()
        .map(|run| run.style.font_size_points() * 1.18)
        .fold(0.0_f32, f32::max);
    line.height.max(word_pitch).max(1.0)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn table_vertical_span_heights(
    table: &Table,
    row_idx: usize,
    prepared: &PreparedTableRow,
    column_widths: &[f32],
    default_column_width: f32,
    markers: MarkerContext,
    document: &Document,
    font_provider: Option<&FontProvider>,
    max_span_height: f32,
) -> Vec<f32> {
    let Some(row) = table.rows.get(row_idx) else {
        return vec![prepared.row_height; prepared.visual_cells.len()];
    };

    prepared
        .visual_cells
        .iter()
        .map(|visual_cell| {
            let Some(cell) = row.cells.get(visual_cell.cell_index) else {
                return prepared.row_height;
            };
            if cell.vertical_merge != TableCellVerticalMerge::First {
                return prepared.row_height;
            }

            let mut span_height = prepared.row_height;
            for next_row in table.rows.iter().skip(row_idx + 1) {
                let continues =
                    next_row
                        .cells
                        .get(visual_cell.cell_index)
                        .is_some_and(|next_cell| {
                            next_cell.vertical_merge == TableCellVerticalMerge::Continuation
                        });
                if !continues {
                    break;
                }

                let next_prepared = prepare_table_row(
                    next_row,
                    column_widths,
                    default_column_width,
                    &markers,
                    document,
                    font_provider,
                );
                if span_height + next_prepared.row_height > max_span_height {
                    break;
                }
                span_height += next_prepared.row_height;
            }
            span_height
        })
        .collect()
}

pub(in crate::layout) fn prepare_table_row(
    row: &TableRow,
    column_widths: &[f32],
    default_column_width: f32,
    markers: &MarkerContext,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> PreparedTableRow {
    let visual_cells = table_visual_cells(row, column_widths, default_column_width);
    let mut cell_paddings = visual_cells
        .iter()
        .map(|visual_cell| resolve_cell_padding(row, &row.cells[visual_cell.cell_index]))
        .collect::<Vec<_>>();
    let has_diagonal_cell_border = visual_cells.iter().any(|visual_cell| {
        let borders = &row.cells[visual_cell.cell_index].borders;
        borders.diagonal_down.visible || borders.diagonal_up.visible
    });
    // Word places text below the top border and grows auto-height and positive
    // minimum-height rows by that stroke width. Negative exact-height rows keep
    // their authored height; diagonal cells retain the inset so their endpoints
    // meet the border even when the exact height clips the available content.
    let top_border_inset =
        if row.height_twips.is_none_or(|height| height >= 0) || has_diagonal_cell_border {
            visual_cells
                .iter()
                .filter_map(|visual_cell| {
                    let border = &row.cells[visual_cell.cell_index].borders.top;
                    border
                        .visible
                        .then(|| table_border_stroke(border, document).0)
                })
                .fold(0.0, f32::max)
        } else {
            0.0
        };
    for (visual_cell, padding) in visual_cells.iter().zip(cell_paddings.iter_mut()) {
        let cell = &row.cells[visual_cell.cell_index];
        padding.top += top_border_inset;
        if has_diagonal_cell_border && cell.borders.left.visible {
            padding.left += table_border_stroke(&cell.borders.left, document).0 / 2.0;
        }
    }
    let mut cell_spacings = visual_cells
        .iter()
        .map(|visual_cell| resolve_cell_spacing(&row.cells[visual_cell.cell_index]))
        .collect::<Vec<_>>();
    distribute_heterogeneous_cell_spacing(&mut cell_spacings);
    let cell_text_directions = visual_cells
        .iter()
        .map(|visual_cell| row.cells[visual_cell.cell_index].text_direction)
        .collect::<Vec<_>>();
    let cell_lines = visual_cells
        .iter()
        .zip(cell_paddings.iter())
        .zip(cell_spacings.iter())
        .map(|((visual_cell, padding), spacing)| {
            let cell = &row.cells[visual_cell.cell_index];
            let paragraphs = table_cell_paragraphs_for_layout(cell);
            let cell_content_width =
                (visual_cell.width - spacing.left - spacing.right - padding.left - padding.right)
                    .max(12.0);
            paragraphs
                .iter()
                .enumerate()
                .flat_map(|(paragraph_idx, paragraph)| {
                    let fit_text_paragraph;
                    let layout_paragraph = if cell.fit_text {
                        fit_text_paragraph =
                            passive_fit_text_paragraph(paragraph, cell_content_width);
                        &fit_text_paragraph
                    } else {
                        paragraph
                    };
                    let paragraph_text_direction = layout_paragraph
                        .style
                        .table_cell_text_direction
                        .unwrap_or(cell.text_direction);
                    let wrap_width = match paragraph_text_direction {
                        TableCellTextDirection::LeftToRightTopToBottom => cell_content_width,
                        TableCellTextDirection::TopToBottomRightToLeft
                        | TableCellTextDirection::BottomToTopLeftToRight => row
                            .height_twips
                            .filter(|height| *height != 0)
                            .map(|height| twips_to_points(height.saturating_abs()).max(12.0))
                            .unwrap_or(MAX_VERTICAL_TABLE_TEXT_WRAP_POINTS),
                    };
                    let mut lines = wrap_paragraph_with_font_provider(
                        layout_paragraph,
                        wrap_width,
                        markers,
                        document,
                        font_provider,
                    );
                    for line in &mut lines {
                        if cell.fit_text {
                            apply_passive_table_cell_fit_text(line, cell_content_width);
                        }
                        line.height = apply_line_spacing(line.height, &layout_paragraph.style);
                        if !layout_paragraph.style.line_spacing_multiple {
                            line.natural_height = line.height;
                        }
                    }
                    let line_count = lines.len();
                    let suppress_contextual_space_before = paragraph_idx
                        .checked_sub(1)
                        .and_then(|previous_idx| paragraphs.get(previous_idx))
                        .is_some_and(|previous| {
                            paragraph_spacing_is_contextual(previous, paragraph)
                        });
                    let suppress_contextual_space_after = paragraphs
                        .get(paragraph_idx + 1)
                        .is_some_and(|next| paragraph_spacing_is_contextual(paragraph, next));
                    let nested_row_extra = paragraph
                        .style
                        .nested_table_row_height_twips
                        .map(|height_twips| {
                            let requested_height =
                                twips_to_points(height_twips.saturating_abs().clamp(0, 100_000));
                            let paragraph_direction = paragraph
                                .style
                                .table_cell_text_direction
                                .unwrap_or(cell.text_direction);
                            let natural_height = lines
                                .iter()
                                .map(|line| match paragraph_direction {
                                    TableCellTextDirection::LeftToRightTopToBottom => line.height,
                                    TableCellTextDirection::TopToBottomRightToLeft
                                    | TableCellTextDirection::BottomToTopLeftToRight => {
                                        line.width.max(line.height)
                                    }
                                })
                                .sum::<f32>();
                            (requested_height - natural_height).max(0.0)
                        })
                        .unwrap_or(0.0);
                    let (nested_extra_before, nested_extra_after) = match paragraph
                        .style
                        .nested_table_row_vertical_align
                        .unwrap_or(TableCellVerticalAlign::Top)
                    {
                        TableCellVerticalAlign::Top => (0.0, nested_row_extra),
                        TableCellVerticalAlign::Center => {
                            (nested_row_extra / 2.0, nested_row_extra / 2.0)
                        }
                        TableCellVerticalAlign::Bottom => (nested_row_extra, 0.0),
                    };
                    let paragraph_space_before = if suppress_contextual_space_before {
                        0.0
                    } else {
                        twips_to_points(effective_space_before_twips(&paragraph.style))
                    } + nested_extra_before;
                    let paragraph_space_after = if suppress_contextual_space_after {
                        0.0
                    } else {
                        twips_to_points(effective_space_after_twips(&paragraph.style))
                    } + nested_extra_after;
                    lines
                        .into_iter()
                        .enumerate()
                        .map(move |(idx, line)| PreparedCellLine {
                            line,
                            style: paragraph.style.clone(),
                            paragraph_index: paragraph_idx,
                            text_direction: paragraph
                                .style
                                .table_cell_text_direction
                                .unwrap_or(cell.text_direction),
                            is_first_line: idx == 0,
                            is_last_line: idx + 1 == line_count,
                            space_before: if idx == 0 {
                                paragraph_space_before
                            } else {
                                0.0
                            },
                            space_after: if idx + 1 == line_count {
                                paragraph_space_after
                            } else {
                                0.0
                            },
                        })
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let has_lines = cell_lines.iter().any(|lines| !lines.is_empty());
    let row_height = cell_lines
        .iter()
        .zip(visual_cells.iter())
        .zip(cell_paddings.iter())
        .zip(cell_spacings.iter())
        .map(|(((lines, visual_cell), padding), spacing)| {
            let cell = &row.cells[visual_cell.cell_index];
            prepared_cell_lines_height(lines, cell.text_direction)
                + padding.top
                + padding.bottom
                + spacing.top
                + spacing.bottom
        })
        .fold(0.0, f32::max);
    let row_height = if has_lines {
        row_height
    } else {
        row_height.max(14.0)
    };
    let row_height = match row.height_twips {
        Some(height) if height < 0 => {
            twips_to_points(height.checked_abs().unwrap_or(i32::MAX)).max(1.0)
        }
        Some(height) => {
            let natural_content_height = (row_height - top_border_inset).max(0.0);
            twips_to_points(height).max(natural_content_height) + top_border_inset
        }
        None => row_height,
    };

    PreparedTableRow {
        visual_cells,
        cell_lines,
        cell_paddings,
        cell_spacings,
        cell_text_directions,
        top_border_inset,
        row_height,
    }
}

pub(in crate::layout) fn table_cell_paragraphs_for_layout(cell: &TableCell) -> Vec<Paragraph> {
    let Some(nested_table) = cell.nested_table.as_deref() else {
        return cell.paragraphs.clone();
    };

    let has_anchor = cell.paragraphs.iter().any(|paragraph| {
        paragraph
            .runs
            .iter()
            .any(|run| run.dynamic_field == Some(InlineKind::NestedTableAnchor))
    });
    if !has_anchor {
        let mut paragraphs = cell.paragraphs.clone();
        append_nested_table_paragraphs(&mut paragraphs, nested_table, 0);
        return paragraphs;
    }

    let mut paragraphs = Vec::new();
    let mut nested_inserted = false;
    for paragraph in &cell.paragraphs {
        if nested_inserted {
            paragraphs.push(paragraph.clone());
            continue;
        }
        let Some(anchor_run_index) = paragraph.runs.iter().enumerate().find_map(|(index, run)| {
            (run.dynamic_field == Some(InlineKind::NestedTableAnchor)).then_some(index)
        }) else {
            paragraphs.push(paragraph.clone());
            continue;
        };

        let before = paragraph.runs[..anchor_run_index].to_vec();
        if !before.is_empty() {
            paragraphs.push(Paragraph {
                style: paragraph.style.clone(),
                runs: before,
            });
        }
        append_nested_table_paragraphs(&mut paragraphs, nested_table, 0);

        let mut after = Vec::new();
        after.extend(paragraph.runs[anchor_run_index + 1..].iter().cloned());
        if !after.is_empty() {
            paragraphs.push(Paragraph {
                style: paragraph.style.clone(),
                runs: after,
            });
        }
        nested_inserted = true;
    }
    if !nested_inserted {
        append_nested_table_paragraphs(&mut paragraphs, nested_table, 0);
    }
    paragraphs
}

pub(in crate::layout) fn append_nested_table_paragraphs(
    paragraphs: &mut Vec<Paragraph>,
    table: &Table,
    depth: usize,
) {
    if depth >= 16 {
        return;
    }
    for row in &table.rows {
        let mut runs = Vec::new();
        let mut deferred_child_paragraphs = Vec::new();
        let source_widths = if table.column_widths_twips.len() >= row.cells.len() {
            table
                .column_widths_twips
                .iter()
                .take(row.cells.len().max(1))
                .map(|width| twips_to_points((*width).max(1)))
                .collect::<Vec<_>>()
        } else {
            vec![twips_to_points(1440); row.cells.len().max(1)]
        };
        let visual_cells = table_visual_cells(row, &source_widths, 72.0);
        for (visual_index, visual_cell) in visual_cells.iter().enumerate() {
            let Some(cell) = row.cells.get(visual_cell.cell_index) else {
                continue;
            };
            if visual_index > 0 {
                runs.push(Run {
                    dynamic_field: None,
                    text: "\t".to_string(),
                    style: CharacterStyle::default(),
                });
            }
            for (paragraph_index, paragraph) in cell.paragraphs.iter().enumerate() {
                if paragraph_index > 0 {
                    runs.push(Run {
                        dynamic_field: None,
                        text: "\n".to_string(),
                        style: CharacterStyle::default(),
                    });
                }
                runs.extend(paragraph.runs.clone());
            }
            if let Some(nested_table) = cell.nested_table.as_deref() {
                append_nested_table_paragraphs(
                    &mut deferred_child_paragraphs,
                    nested_table,
                    depth + 1,
                );
            }
        }
        let mut style = row
            .cells
            .get(
                visual_cells
                    .first()
                    .map(|cell| cell.cell_index)
                    .unwrap_or(0),
            )
            .and_then(|cell| cell.paragraphs.first())
            .map(|paragraph| paragraph.style.clone())
            .unwrap_or_default();
        style.keep_together |= row.keep_together;
        style.keep_with_next |= row.keep_with_next;
        if let Some(first_cell) = visual_cells
            .first()
            .and_then(|visual_cell| row.cells.get(visual_cell.cell_index))
        {
            style.left_indent_twips = style
                .left_indent_twips
                .saturating_add(nested_cell_left_inset_twips(first_cell));
        }
        style.left_indent_twips = style
            .left_indent_twips
            .saturating_add(row.left_offset_twips);
        let stop_count = visual_cells.len().saturating_sub(1).min(64);
        if stop_count > 0 {
            let mut position_twips = 0i32;
            let mut stops = Vec::with_capacity(stop_count);
            for (visual_index, visual_cell) in visual_cells.iter().take(stop_count).enumerate() {
                let end_column = visual_cells
                    .iter()
                    .find(|candidate| candidate.cell_index > visual_cell.cell_index)
                    .map(|candidate| candidate.cell_index)
                    .unwrap_or(row.cells.len());
                let span_width = table
                    .column_widths_twips
                    .iter()
                    .skip(visual_cell.cell_index)
                    .take(end_column.saturating_sub(visual_cell.cell_index))
                    .copied()
                    .fold(0i32, |total, width| total.saturating_add(width.max(1)));
                let span_width = if span_width > 0 {
                    span_width
                } else {
                    let span_count =
                        i32::try_from(end_column.saturating_sub(visual_cell.cell_index))
                            .unwrap_or(i32::MAX);
                    1440i32.saturating_mul(span_count)
                };
                position_twips = position_twips.saturating_add(span_width.max(1));
                let next_cell_inset = visual_cells
                    .get(visual_index + 1)
                    .and_then(|visual_cell| row.cells.get(visual_cell.cell_index))
                    .map(nested_cell_left_inset_twips)
                    .unwrap_or(0);
                stops.push(position_twips.saturating_add(next_cell_inset));
            }
            if stops.len() == stop_count {
                style.tab_stops_twips = stops;
                style.tab_stop_leaders = vec![TabLeader::None; stop_count];
                style.tab_stop_alignments = visual_cells
                    .iter()
                    .skip(1)
                    .take(stop_count)
                    .filter_map(|visual_cell| row.cells.get(visual_cell.cell_index))
                    .map(|cell| {
                        match cell
                            .paragraphs
                            .first()
                            .map(|paragraph| paragraph.style.alignment)
                            .unwrap_or(Alignment::Left)
                        {
                            Alignment::Center => TabAlignment::Center,
                            Alignment::Right => TabAlignment::Right,
                            Alignment::Justified | Alignment::Distributed => TabAlignment::Left,
                            Alignment::Left => TabAlignment::Left,
                        }
                    })
                    .collect();
            }
        }
        if let Some(direction) = visual_cells
            .first()
            .and_then(|visual_cell| row.cells.get(visual_cell.cell_index))
            .map(|cell| cell.text_direction)
            .filter(|direction| *direction != TableCellTextDirection::LeftToRightTopToBottom)
            && visual_cells.iter().all(|visual_cell| {
                row.cells
                    .get(visual_cell.cell_index)
                    .is_some_and(|cell| cell.text_direction == direction)
            })
        {
            style.table_cell_text_direction = Some(direction);
        }
        if let Some(height_twips) = row.height_twips {
            style.nested_table_row_height_twips = Some(height_twips.clamp(-1_000_000, 1_000_000));
        }
        if let Some(vertical_align) = visual_cells
            .first()
            .and_then(|visual_cell| row.cells.get(visual_cell.cell_index))
            .map(|cell| cell.vertical_align)
            .filter(|vertical_align| *vertical_align != TableCellVerticalAlign::Top)
            && visual_cells.iter().all(|visual_cell| {
                row.cells
                    .get(visual_cell.cell_index)
                    .is_some_and(|cell| cell.vertical_align == vertical_align)
            })
        {
            style.nested_table_row_vertical_align = Some(vertical_align);
        }
        paragraphs.push(Paragraph { style, runs });
        paragraphs.extend(deferred_child_paragraphs);
    }
}

pub(in crate::layout) fn nested_cell_left_inset_twips(cell: &TableCell) -> i32 {
    cell.padding
        .left_twips
        .unwrap_or(0)
        .max(0)
        .saturating_add(cell.spacing.left_twips.unwrap_or(0).max(0))
        .min(100_000)
}

pub(in crate::layout) fn passive_fit_text_paragraph(
    paragraph: &Paragraph,
    content_width: f32,
) -> Paragraph {
    let mut paragraph = paragraph.clone();
    paragraph.style.no_wrap = true;
    let target_twips = (content_width.max(0.1) * 20.0)
        .round()
        .clamp(1.0, 100_000.0) as i32;
    for run in &mut paragraph.runs {
        run.style.fit_text_width_twips = Some(target_twips);
    }
    paragraph
}

pub(in crate::layout) fn apply_passive_table_cell_fit_text(line: &mut Line, content_width: f32) {
    if line.width <= content_width || line.width <= 0.01 {
        return;
    }
    let scale = (content_width / line.width).clamp(0.05, 1.0);
    for run in &mut line.runs {
        if run.text != "\t" {
            let scaled_percent = ((run.style.character_scaling_percent as f32) * scale)
                .round()
                .clamp(5.0, run.style.character_scaling_percent.max(5) as f32);
            run.style.character_scaling_percent = scaled_percent as i32;
        }
        run.width *= scale;
    }
    line.width *= scale;
}

pub(in crate::layout) fn resolve_cell_padding(
    row: &TableRow,
    cell: &TableCell,
) -> ResolvedCellPadding {
    let horizontal_fallback = row.cell_gap_twips.max(0);
    ResolvedCellPadding {
        left: twips_to_points(
            cell.padding
                .left_twips
                .unwrap_or(horizontal_fallback)
                .max(0),
        ),
        right: twips_to_points(
            cell.padding
                .right_twips
                .unwrap_or(horizontal_fallback)
                .max(0),
        ),
        top: twips_to_points(cell.padding.top_twips.unwrap_or(0).max(0)),
        bottom: twips_to_points(cell.padding.bottom_twips.unwrap_or(0).max(0)),
    }
}

pub(in crate::layout) fn resolve_cell_spacing(cell: &TableCell) -> ResolvedCellSpacing {
    let spacing = cell.spacing;
    ResolvedCellSpacing {
        left: twips_to_points(spacing.left_twips.unwrap_or(0).max(0)),
        right: twips_to_points(spacing.right_twips.unwrap_or(0).max(0)),
        top: twips_to_points(spacing.top_twips.unwrap_or(0).max(0)),
        bottom: twips_to_points(spacing.bottom_twips.unwrap_or(0).max(0)),
    }
}

pub(in crate::layout) fn has_heterogeneous_horizontal_cell_spacing(
    spacings: &[ResolvedCellSpacing],
) -> bool {
    spacings.windows(2).any(|pair| {
        (pair[0].left - pair[1].left).abs() > f32::EPSILON
            || (pair[0].right - pair[1].right).abs() > f32::EPSILON
    })
}

pub(in crate::layout) fn distribute_heterogeneous_cell_spacing(
    spacings: &mut [ResolvedCellSpacing],
) {
    if spacings.len() < 2 || !has_heterogeneous_horizontal_cell_spacing(spacings) {
        return;
    }
    let authored = spacings.to_vec();
    let outer_left = authored
        .iter()
        .map(|spacing| spacing.left)
        .fold(f32::INFINITY, f32::min);
    let outer_right = authored
        .iter()
        .map(|spacing| spacing.right)
        .fold(f32::INFINITY, f32::min);
    let last = spacings.len() - 1;
    for (index, spacing) in spacings.iter_mut().enumerate() {
        spacing.left = if index == 0 {
            authored[index].left + outer_left
        } else {
            (authored[index].left - authored[index - 1].right).max(0.0)
        };
        spacing.right = if index == last {
            authored[index].right + outer_right
        } else {
            (authored[index].right - authored[index + 1].left).max(0.0)
        };
    }
}

pub(in crate::layout) fn distributed_table_row_top_spacing_points(row: &TableRow) -> f32 {
    let spacings = row
        .cells
        .iter()
        .map(resolve_cell_spacing)
        .collect::<Vec<_>>();
    if !has_heterogeneous_horizontal_cell_spacing(&spacings) {
        return 0.0;
    }
    spacings
        .iter()
        .map(|spacing| spacing.top)
        .fold(f32::INFINITY, f32::min)
        .max(0.0)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn push_table_row(
    pages: &mut [LayoutPage],
    cursor_y: &mut f32,
    row: &TableRow,
    prepared: &PreparedTableRow,
    vertical_span_heights: &[f32],
    content_width: f32,
    margin_left: f32,
    table_width: f32,
    document: &Document,
    font_provider: Option<&FontProvider>,
    borders_visible: bool,
    render_nested_child_grids: bool,
    next_row: Option<&TableRow>,
) {
    let Some(page_geometry) = pages.last().map(|page| page.geometry) else {
        return;
    };
    let row_left =
        positioned_table_row_left(row, margin_left, content_width, table_width, page_geometry);
    let top = *cursor_y;
    let bottom = top - prepared.row_height;

    for (idx, visual_cell) in prepared.visual_cells.iter().enumerate() {
        let cell = &row.cells[visual_cell.cell_index];
        if cell.vertical_merge == TableCellVerticalMerge::Continuation {
            continue;
        }
        let spacing = prepared.cell_spacings[idx];
        let span_height = vertical_span_heights
            .get(idx)
            .copied()
            .unwrap_or(prepared.row_height);
        let cell_left = row_left + visual_cell.x_offset + spacing.left;
        let cell_top = top - spacing.top;
        let cell_width = (visual_cell.width - spacing.left - spacing.right).max(1.0);
        let cell_height = (span_height - spacing.top - spacing.bottom).max(1.0);
        if shading_is_visible(document, cell.shading_color_index, cell.shading_pattern) {
            push_shading_rect(
                pages,
                document,
                cell_left,
                cell_top - cell_height,
                cell_width,
                cell_height,
                cell.shading_color_index,
                cell.shading_basis_points,
                cell.shading_pattern,
            );
        }
        if let Some(nested_table) = cell.nested_table.as_deref()
            && let Some(row_range) = nested_table_row_range(cell, lines_for_cell(prepared, idx))
        {
            let padding = prepared.cell_paddings[idx];
            push_nested_table_grid(
                pages,
                nested_table,
                document,
                row_range,
                cell_left + padding.left,
                cell_top - padding.top,
                (cell_width - padding.left - padding.right).max(1.0),
                (cell_height - padding.top - padding.bottom).max(1.0),
                render_nested_child_grids,
            );
        }
    }
    if borders_visible {
        push_table_borders(
            pages, row_left, top, bottom, row, prepared, document, next_row,
        );
    }

    for (idx, lines) in prepared.cell_lines.iter().enumerate() {
        let visual_cell = &prepared.visual_cells[idx];
        let padding = prepared.cell_paddings[idx];
        let spacing = prepared.cell_spacings[idx];
        let cell = &row.cells[visual_cell.cell_index];
        if cell.vertical_merge == TableCellVerticalMerge::Continuation {
            continue;
        }
        let span_height = vertical_span_heights
            .get(idx)
            .copied()
            .unwrap_or(prepared.row_height);
        let content_height = prepared_cell_lines_height(lines, cell.text_direction);
        let available_height =
            (span_height - spacing.top - spacing.bottom - padding.top - padding.bottom).max(0.0);
        let extra_height = (available_height - content_height).max(0.0);
        let vertical_offset = match cell.vertical_align {
            TableCellVerticalAlign::Top => 0.0,
            TableCellVerticalAlign::Center => extra_height / 2.0,
            TableCellVerticalAlign::Bottom => extra_height,
        };
        let content_bottom = top - span_height + spacing.bottom + padding.bottom;
        let mut line_top = top - spacing.top - padding.top - vertical_offset;
        let mut vertical_cross_offset = 0.0;
        for prepared_line in lines {
            let current_line_top = line_top - prepared_line.space_before;
            let line_extent =
                prepared_cell_line_extent(prepared_line, prepared_line.text_direction);
            if current_line_top - line_extent < content_bottom {
                break;
            }
            let content_left = row_left + visual_cell.x_offset + spacing.left + padding.left;
            let cell_content_width =
                (visual_cell.width - spacing.left - spacing.right - padding.left - padding.right)
                    .max(1.0);
            if shading_is_visible(
                document,
                prepared_line.style.shading_color_index,
                prepared_line.style.shading_pattern,
            ) {
                let line_left_indent = twips_to_points(paragraph_line_left_indent_twips(
                    &prepared_line.style,
                    prepared_line.is_first_line,
                ));
                push_shading_rect(
                    pages,
                    document,
                    content_left + line_left_indent,
                    line_top - prepared_line.line.height,
                    paragraph_line_width(
                        cell_content_width,
                        &prepared_line.style,
                        prepared_line.is_first_line,
                    ),
                    prepared_line.line.height,
                    prepared_line.style.shading_color_index,
                    prepared_line.style.shading_basis_points,
                    prepared_line.style.shading_pattern,
                );
            }
            let x = aligned_x(
                content_left,
                cell_content_width,
                prepared_line.line.width,
                &prepared_line.style,
                prepared_line.is_first_line,
            );
            let word_spacing = justified_word_spacing(
                &prepared_line.line,
                &prepared_line.style,
                paragraph_line_width(
                    cell_content_width,
                    &prepared_line.style,
                    prepared_line.is_first_line,
                ),
                prepared_line.is_last_line,
                document,
                font_provider,
            );
            let character_spacing = distributed_character_spacing(
                &prepared_line.line,
                &prepared_line.style,
                paragraph_line_width(
                    cell_content_width,
                    &prepared_line.style,
                    prepared_line.is_first_line,
                ),
                prepared_line.is_last_line,
            );
            let (border_line_idx, border_line_count) = paragraph_border_line_position(
                prepared_line.is_first_line,
                prepared_line.is_last_line,
            );
            push_paragraph_borders(
                pages,
                content_left,
                cell_content_width,
                &prepared_line.style,
                border_line_idx,
                border_line_count,
                current_line_top,
                prepared_line.line.height,
                document,
                false,
                false,
            );
            let rotation = table_cell_text_rotation(prepared_line.text_direction);
            if rotation == TextRotation::None {
                push_bar_tab_stops(
                    pages,
                    &prepared_line.style,
                    x,
                    current_line_top,
                    prepared_line.line.height,
                );
                push_line(
                    pages,
                    &prepared_line.line,
                    x,
                    current_line_top,
                    document,
                    font_provider,
                    word_spacing,
                    character_spacing,
                );
            } else {
                let rotated_line_top = match prepared_line.text_direction {
                    TableCellTextDirection::BottomToTopLeftToRight => {
                        current_line_top - (content_height - line_extent).max(0.0)
                    }
                    _ => current_line_top,
                };
                let (rotated_x, rotated_y) = rotated_table_cell_line_origin(
                    content_left,
                    cell_content_width,
                    rotated_line_top,
                    &prepared_line.line,
                    prepared_line.text_direction,
                    vertical_cross_offset,
                );
                push_line_with_rotation(
                    pages,
                    &prepared_line.line,
                    rotated_x,
                    rotated_y,
                    document,
                    font_provider,
                    word_spacing,
                    character_spacing,
                    rotation,
                );
            }
            if rotation == TextRotation::None {
                line_top = current_line_top - line_extent - prepared_line.space_after;
            } else {
                vertical_cross_offset += prepared_line.line.height;
            }
        }
    }

    *cursor_y -= prepared.row_height;
}

pub(in crate::layout) fn positioned_table_row_left(
    row: &TableRow,
    margin_left: f32,
    content_width: f32,
    table_width: f32,
    page_geometry: PageGeometry,
) -> f32 {
    let floating_gap = if table_row_has_wrap_margins(row)
        && (row.left_offset_twips != 0 || row.vertical_offset_twips != 0)
    {
        twips_to_points(row.cell_gap_twips.max(0))
    } else {
        0.0
    };
    table_row_left(
        margin_left,
        content_width.max(table_width).max(1.0),
        table_width,
        row.alignment,
        page_geometry,
    ) + twips_to_points(row.left_offset_twips)
        - floating_gap
}

pub(in crate::layout) fn lines_for_cell<'a>(
    prepared: &'a PreparedTableRow,
    index: usize,
) -> &'a [PreparedCellLine] {
    prepared
        .cell_lines
        .get(index)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

pub(in crate::layout) fn nested_table_row_range(
    cell: &TableCell,
    lines: &[PreparedCellLine],
) -> Option<(usize, usize)> {
    let nested_row_count = cell.nested_table.as_deref()?.rows.len().min(256);
    if nested_row_count == 0 {
        return None;
    }
    let nested_start = nested_table_paragraph_start(cell);
    let mut range: Option<(usize, usize)> = None;
    for line in lines {
        let Some(row_index) = line.paragraph_index.checked_sub(nested_start) else {
            continue;
        };
        if row_index >= nested_row_count {
            continue;
        }
        range = Some(match range {
            Some((start, end)) => (start.min(row_index), end.max(row_index + 1)),
            None => (row_index, row_index + 1),
        });
    }
    range
}

pub(in crate::layout) fn nested_table_paragraph_start(cell: &TableCell) -> usize {
    let Some((paragraph_index, run_index)) =
        cell.paragraphs
            .iter()
            .enumerate()
            .find_map(|(paragraph_index, paragraph)| {
                paragraph
                    .runs
                    .iter()
                    .enumerate()
                    .find_map(|(run_index, run)| {
                        (run.dynamic_field == Some(InlineKind::NestedTableAnchor))
                            .then_some((paragraph_index, run_index))
                    })
            })
    else {
        return cell.paragraphs.len();
    };

    let has_prefix_text = cell.paragraphs[paragraph_index].runs[..run_index]
        .iter()
        .any(|run| !run.text.is_empty());
    paragraph_index + usize::from(has_prefix_text)
}

pub(in crate::layout) fn paragraph_border_line_position(
    is_first_line: bool,
    is_last_line: bool,
) -> (usize, usize) {
    match (is_first_line, is_last_line) {
        (true, true) => (0, 1),
        (true, false) => (0, 2),
        (false, true) => (1, 2),
        (false, false) => (1, 3),
    }
}

pub(in crate::layout) fn table_visual_cells(
    row: &TableRow,
    column_widths: &[f32],
    default_width: f32,
) -> Vec<VisualTableCell> {
    let mut cells = Vec::new();
    let mut idx = 0;
    let mut x_offset = 0.0;
    while idx < row.cells.len() {
        let merge = row.cells[idx].horizontal_merge;
        let mut span = 1;
        if merge == TableCellHorizontalMerge::First {
            while idx + span < row.cells.len()
                && row.cells[idx + span].horizontal_merge == TableCellHorizontalMerge::Continuation
            {
                span += 1;
            }
        }

        let width = (idx..idx + span)
            .map(|column_idx| {
                column_widths
                    .get(column_idx)
                    .copied()
                    .unwrap_or(default_width)
            })
            .sum();
        cells.push(VisualTableCell {
            cell_index: idx,
            x_offset,
            width,
        });
        x_offset += width;
        idx += span;
    }

    cells
}

pub(in crate::layout) fn table_row_left(
    margin_left: f32,
    content_width: f32,
    table_width: f32,
    alignment: TableRowAlignment,
    geometry: PageGeometry,
) -> f32 {
    match alignment {
        TableRowAlignment::Left => margin_left,
        TableRowAlignment::Center => margin_left + ((content_width - table_width) / 2.0).max(0.0),
        TableRowAlignment::Right => margin_left + (content_width - table_width).max(0.0),
        TableRowAlignment::Inside => table_row_left(
            margin_left,
            content_width,
            table_width,
            inside_outside_physical_alignment(TableRowAlignment::Inside, geometry),
            geometry,
        ),
        TableRowAlignment::Outside => table_row_left(
            margin_left,
            content_width,
            table_width,
            inside_outside_physical_alignment(TableRowAlignment::Outside, geometry),
            geometry,
        ),
    }
}

pub(in crate::layout) fn inside_outside_physical_alignment(
    alignment: TableRowAlignment,
    geometry: PageGeometry,
) -> TableRowAlignment {
    if !geometry.facing_pages {
        return match alignment {
            TableRowAlignment::Inside => TableRowAlignment::Left,
            TableRowAlignment::Outside => TableRowAlignment::Right,
            other => other,
        };
    }
    let even_page = geometry.physical_page_number % 2 == 0;
    match (alignment, even_page) {
        (TableRowAlignment::Inside, false) | (TableRowAlignment::Outside, true) => {
            TableRowAlignment::Left
        }
        (TableRowAlignment::Inside, true) | (TableRowAlignment::Outside, false) => {
            TableRowAlignment::Right
        }
        (other, _) => other,
    }
}

pub(in crate::layout) fn table_column_widths(
    table: &Table,
    column_count: usize,
    content_width: f32,
) -> Vec<f32> {
    if table.column_widths_twips.len() >= column_count {
        let widths = table
            .column_widths_twips
            .iter()
            .take(column_count)
            .map(|width| twips_to_points(*width).max(12.0))
            .collect::<Vec<_>>();
        let total: f32 = widths.iter().sum();
        if total > content_width && !table.preserve_authored_widths {
            let scale = content_width / total;
            return widths.into_iter().map(|width| width * scale).collect();
        }
        return widths;
    }

    vec![content_width / column_count as f32; column_count]
}

pub(in crate::layout) fn table_row_column_widths(
    table: &Table,
    row: &TableRow,
    column_count: usize,
    content_width: f32,
) -> Vec<f32> {
    if row.column_widths_twips.len() < column_count {
        return table_column_widths(table, column_count, content_width);
    }

    let widths = row
        .column_widths_twips
        .iter()
        .take(column_count)
        .map(|width| twips_to_points(*width).max(12.0))
        .collect::<Vec<_>>();
    let total: f32 = widths.iter().sum();
    if total > content_width && !table.preserve_authored_widths {
        let scale = content_width / total;
        return widths.into_iter().map(|width| width * scale).collect();
    }
    widths
}

pub(in crate::layout) fn push_table_borders(
    pages: &mut [LayoutPage],
    left: f32,
    top: f32,
    bottom: f32,
    row: &TableRow,
    prepared: &PreparedTableRow,
    document: &Document,
    next_row: Option<&TableRow>,
) {
    let Some(page) = pages.last_mut() else {
        return;
    };

    for (idx, visual_cell) in prepared.visual_cells.iter().enumerate() {
        let Some(cell) = row.cells.get(visual_cell.cell_index) else {
            continue;
        };
        let spacing = prepared.cell_spacings[idx];
        let x1 = left + visual_cell.x_offset + spacing.left;
        let x2 = (left + visual_cell.x_offset + visual_cell.width - spacing.right).max(x1 + 1.0);
        let cell_top = top - spacing.top;
        let cell_bottom = (bottom + spacing.bottom).min(cell_top - 1.0);
        if cell.borders.top.visible && cell.vertical_merge != TableCellVerticalMerge::Continuation {
            let (width, color, style) = table_border_stroke(&cell.borders.top, document);
            let border_y = cell_top + twips_to_points(cell.borders.top.spacing_twips.max(0));
            page.items.push(LayoutItem::Line {
                x1,
                y1: border_y,
                x2,
                y2: border_y,
                width,
                color,
                style,
            });
        }
        if cell.borders.bottom.visible
            && !next_row_continues_vertical_merge(row, next_row, visual_cell.cell_index)
        {
            let (width, color, style) = table_border_stroke(&cell.borders.bottom, document);
            let border_y = cell_bottom - twips_to_points(cell.borders.bottom.spacing_twips.max(0));
            page.items.push(LayoutItem::Line {
                x1,
                y1: border_y,
                x2,
                y2: border_y,
                width,
                color,
                style,
            });
        }
        if cell.borders.left.visible {
            let (width, color, style) = table_border_stroke(&cell.borders.left, document);
            let border_x = x1 - twips_to_points(cell.borders.left.spacing_twips.max(0));
            page.items.push(LayoutItem::Line {
                x1: border_x,
                y1: cell_top,
                x2: border_x,
                y2: cell_bottom,
                width,
                color,
                style,
            });
        }
        if cell.borders.right.visible {
            let (width, color, style) = table_border_stroke(&cell.borders.right, document);
            let border_x = x2 + twips_to_points(cell.borders.right.spacing_twips.max(0));
            page.items.push(LayoutItem::Line {
                x1: border_x,
                y1: cell_top,
                x2: border_x,
                y2: cell_bottom,
                width,
                color,
                style,
            });
        }
        if cell.borders.diagonal_down.visible {
            let (width, color, style) =
                diagonal_table_border_stroke(&cell.borders.diagonal_down, document);
            page.items.push(LayoutItem::Line {
                x1,
                y1: cell_top - prepared.top_border_inset,
                x2,
                y2: cell_bottom,
                width,
                color,
                style,
            });
        }
        if cell.borders.diagonal_up.visible {
            let (width, color, style) =
                diagonal_table_border_stroke(&cell.borders.diagonal_up, document);
            page.items.push(LayoutItem::Line {
                x1,
                y1: cell_bottom,
                x2,
                y2: cell_top - prepared.top_border_inset,
                width,
                color,
                style,
            });
        }
    }
}

pub(in crate::layout) fn push_nested_table_grid(
    pages: &mut [LayoutPage],
    table: &Table,
    document: &Document,
    row_range: (usize, usize),
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    render_child_tables: bool,
) {
    push_nested_table_grid_at_depth(
        pages,
        table,
        document,
        row_range,
        left,
        top,
        width,
        height,
        0,
        render_child_tables,
    );
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn push_nested_table_grid_at_depth(
    pages: &mut [LayoutPage],
    table: &Table,
    document: &Document,
    row_range: (usize, usize),
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    depth: usize,
    render_child_tables: bool,
) {
    const MAX_NESTED_GRID_DEPTH: usize = 8;
    if depth >= MAX_NESTED_GRID_DEPTH {
        return;
    }
    let total_row_count = table.rows.len().min(256);
    let row_start = row_range.0.min(total_row_count);
    let row_end = row_range.1.min(total_row_count).max(row_start);
    let row_count = row_end.saturating_sub(row_start);
    let column_count = table
        .rows
        .iter()
        .map(|row| row.cells.len())
        .max()
        .unwrap_or(0)
        .min(64);
    if row_count == 0 || column_count == 0 {
        return;
    }

    // Nested tables are normalized into a passive model. Use a stable, light
    // grid when the source did not retain nested border geometry.
    let fallback_color = PdfColor {
        red: 0.65,
        green: 0.65,
        blue: 0.65,
    };
    let row_heights = nested_table_row_heights(table, row_start, row_count, height);
    let column_widths = table_column_widths(table, column_count, width);
    let mut column_positions = Vec::with_capacity(column_count + 1);
    column_positions.push(left);
    for column_width in &column_widths {
        column_positions.push(column_positions.last().copied().unwrap_or(left) + *column_width);
    }
    for (row_index, row) in table
        .rows
        .iter()
        .skip(row_start)
        .take(row_count)
        .enumerate()
    {
        let cell_top = top - row_heights[..row_index].iter().sum::<f32>();
        let table_row_index = row_start + row_index;
        let row_left = nested_table_row_left(
            left + twips_to_points(row.left_offset_twips),
            width,
            row.alignment,
            &column_widths,
        );
        let visual_cells = table_visual_cells(row, &column_widths, width / column_count as f32);
        for visual_cell in visual_cells {
            let Some(cell) = row.cells.get(visual_cell.cell_index) else {
                continue;
            };
            let render_cell = if cell.vertical_merge == TableCellVerticalMerge::Continuation {
                nested_vertical_merge_origin(table, table_row_index, visual_cell.cell_index)
                    .and_then(|(origin_row, origin_cell)| {
                        table.rows.get(origin_row)?.cells.get(origin_cell)
                    })
                    .unwrap_or(cell)
            } else {
                cell
            };
            let cell_height = nested_vertical_span_height(
                table,
                table_row_index,
                visual_cell.cell_index,
                row_start,
                row_end,
                &row_heights,
            );
            if shading_is_visible(
                document,
                render_cell.shading_color_index,
                render_cell.shading_pattern,
            ) {
                push_shading_rect(
                    pages,
                    document,
                    row_left + visual_cell.x_offset,
                    cell_top - cell_height,
                    visual_cell.width.max(1.0),
                    cell_height.max(1.0),
                    render_cell.shading_color_index,
                    render_cell.shading_basis_points,
                    render_cell.shading_pattern,
                );
            }
            if render_child_tables && let Some(child_table) = render_cell.nested_table.as_deref() {
                let left_inset = twips_to_points(
                    render_cell
                        .padding
                        .left_twips
                        .unwrap_or(0)
                        .saturating_add(render_cell.spacing.left_twips.unwrap_or(0))
                        .max(0),
                );
                let right_inset = twips_to_points(
                    render_cell
                        .padding
                        .right_twips
                        .unwrap_or(0)
                        .saturating_add(render_cell.spacing.right_twips.unwrap_or(0))
                        .max(0),
                );
                let top_inset = twips_to_points(
                    render_cell
                        .padding
                        .top_twips
                        .unwrap_or(0)
                        .saturating_add(render_cell.spacing.top_twips.unwrap_or(0))
                        .max(0),
                );
                let bottom_inset = twips_to_points(
                    render_cell
                        .padding
                        .bottom_twips
                        .unwrap_or(0)
                        .saturating_add(render_cell.spacing.bottom_twips.unwrap_or(0))
                        .max(0),
                );
                let child_row_count = child_table.rows.len().min(256);
                if child_row_count > 0 {
                    push_nested_table_grid_at_depth(
                        pages,
                        child_table,
                        document,
                        (0, child_row_count),
                        row_left + visual_cell.x_offset + left_inset,
                        cell_top - top_inset,
                        (visual_cell.width - left_inset - right_inset).max(1.0),
                        (cell_height - top_inset - bottom_inset).max(1.0),
                        depth + 1,
                        true,
                    );
                }
            }
        }
    }
    // Decide whether to use the fallback grid from the complete bounded table,
    // not only the currently visible continuation range. A nested table may
    // author borders on its first/last rows while middle rows remain borderless;
    // switching to a synthetic grid on a later fragment changes the visible
    // Word-like result and can introduce lines that were never authored.
    let has_explicit_borders = table.rows.iter().any(|row| {
        row.cells.iter().any(|cell| {
            cell.borders.top.visible
                || cell.borders.right.visible
                || cell.borders.bottom.visible
                || cell.borders.left.visible
                || cell.borders.diagonal_down.visible
                || cell.borders.diagonal_up.visible
        })
    });
    if has_explicit_borders {
        let Some(page) = pages.last_mut() else {
            return;
        };
        for (row_index, row) in table
            .rows
            .iter()
            .skip(row_start)
            .take(row_count)
            .enumerate()
        {
            let cell_top = top - row_heights[..row_index].iter().sum::<f32>();
            let table_row_index = row_start + row_index;
            let row_left = nested_table_row_left(
                left + twips_to_points(row.left_offset_twips),
                width,
                row.alignment,
                &column_widths,
            );
            let visual_cells = table_visual_cells(row, &column_widths, width / column_count as f32);
            for visual_cell in visual_cells {
                let Some(cell) = row.cells.get(visual_cell.cell_index) else {
                    continue;
                };
                let merge_origin = if cell.vertical_merge == TableCellVerticalMerge::Continuation {
                    nested_vertical_merge_origin(table, table_row_index, visual_cell.cell_index)
                } else {
                    None
                };
                let border_cell = merge_origin
                    .and_then(|(origin_row, origin_cell)| {
                        table.rows.get(origin_row)?.cells.get(origin_cell)
                    })
                    .unwrap_or(cell);
                let cell_height = nested_vertical_span_height(
                    table,
                    table_row_index,
                    visual_cell.cell_index,
                    row_start,
                    row_end,
                    &row_heights,
                );
                let x1 = row_left + visual_cell.x_offset;
                let x2 = x1 + visual_cell.width;
                let cell_bottom = cell_top - cell_height;
                let continuation_has_next_row = merge_origin.is_some()
                    && table
                        .rows
                        .get(table_row_index + 1)
                        .and_then(|next_row| next_row.cells.get(visual_cell.cell_index))
                        .is_some_and(|next_cell| {
                            next_cell.vertical_merge == TableCellVerticalMerge::Continuation
                        });
                for (border_index, (border, border_x1, border_y1, border_x2, border_y2)) in [
                    (&border_cell.borders.top, x1, cell_top, x2, cell_top),
                    (
                        &border_cell.borders.bottom,
                        x1,
                        cell_bottom,
                        x2,
                        cell_bottom,
                    ),
                    (&border_cell.borders.left, x1, cell_top, x1, cell_bottom),
                    (&border_cell.borders.right, x2, cell_top, x2, cell_bottom),
                ]
                .into_iter()
                .enumerate()
                {
                    if merge_origin.is_some()
                        && (border_index == 0 || (border_index == 1 && continuation_has_next_row))
                    {
                        continue;
                    }
                    if !border.visible {
                        continue;
                    }
                    let (line_width, color, style) = table_border_stroke(border, document);
                    page.items.push(LayoutItem::Line {
                        x1: border_x1,
                        y1: border_y1,
                        x2: border_x2,
                        y2: border_y2,
                        width: line_width,
                        color,
                        style,
                    });
                }
            }
        }
        return;
    }
    if !table.borders_visible {
        return;
    }
    let has_merges = table
        .rows
        .iter()
        .skip(row_start)
        .take(row_count)
        .flat_map(|row| row.cells.iter())
        .any(|cell| {
            cell.horizontal_merge != TableCellHorizontalMerge::None
                || cell.vertical_merge != TableCellVerticalMerge::None
        });
    if has_merges {
        let Some(page) = pages.last_mut() else {
            return;
        };
        let mut y = top;
        for boundary_index in 0..=row_count {
            let mut intervals = Vec::new();
            if boundary_index > 0 && boundary_index < row_count {
                if let Some(row) = table.rows.get(row_start + boundary_index) {
                    let visual_cells =
                        table_visual_cells(row, &column_widths, width / column_count as f32);
                    for visual_cell in visual_cells {
                        if row.cells.get(visual_cell.cell_index).is_some_and(|cell| {
                            cell.vertical_merge == TableCellVerticalMerge::Continuation
                        }) {
                            intervals.push((
                                left + visual_cell.x_offset,
                                left + visual_cell.x_offset + visual_cell.width,
                            ));
                        }
                    }
                }
            }
            intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut segment_start = left;
            for (interval_start, interval_end) in intervals {
                if interval_start > segment_start {
                    page.items.push(LayoutItem::Line {
                        x1: segment_start,
                        y1: y,
                        x2: interval_start,
                        y2: y,
                        width: 0.25,
                        color: fallback_color,
                        style: LineStyle::Solid,
                    });
                }
                segment_start = segment_start.max(interval_end);
            }
            if segment_start < left + width {
                page.items.push(LayoutItem::Line {
                    x1: segment_start,
                    y1: y,
                    x2: left + width,
                    y2: y,
                    width: 0.25,
                    color: fallback_color,
                    style: LineStyle::Solid,
                });
            }
            if boundary_index < row_count {
                y -= row_heights[boundary_index];
            }
        }
        for (row_index, row) in table
            .rows
            .iter()
            .skip(row_start)
            .take(row_count)
            .enumerate()
        {
            let row_top = top - row_heights[..row_index].iter().sum::<f32>();
            let row_bottom = row_top - row_heights[row_index];
            let visual_cells = table_visual_cells(row, &column_widths, width / column_count as f32);
            let mut boundaries = visual_cells
                .iter()
                .flat_map(|visual_cell| {
                    [
                        left + visual_cell.x_offset,
                        left + visual_cell.x_offset + visual_cell.width,
                    ]
                })
                .collect::<Vec<_>>();
            boundaries.sort_by(|a, b| a.total_cmp(b));
            boundaries.dedup_by(|a, b| (*a - *b).abs() < 0.01);
            for x in boundaries {
                page.items.push(LayoutItem::Line {
                    x1: x,
                    y1: row_top,
                    x2: x,
                    y2: row_bottom,
                    width: 0.25,
                    color: fallback_color,
                    style: LineStyle::Solid,
                });
            }
        }
        return;
    }
    let Some(page) = pages.last_mut() else {
        return;
    };
    let mut y = top;
    for row_index in 0..=row_count {
        page.items.push(LayoutItem::Line {
            x1: left,
            y1: y,
            x2: left + width,
            y2: y,
            width: 0.25,
            color: fallback_color,
            style: LineStyle::Solid,
        });
        if row_index < row_count {
            y -= row_heights[row_index];
        }
    }
    for column_index in 0..=column_count {
        let x = column_positions[column_index];
        page.items.push(LayoutItem::Line {
            x1: x,
            y1: top,
            x2: x,
            y2: top - height,
            width: 0.25,
            color: fallback_color,
            style: LineStyle::Solid,
        });
    }
}

pub(in crate::layout) fn nested_vertical_span_height(
    table: &Table,
    row_index: usize,
    cell_index: usize,
    row_start: usize,
    row_end: usize,
    row_heights: &[f32],
) -> f32 {
    let local_index = row_index.saturating_sub(row_start);
    let mut height = row_heights.get(local_index).copied().unwrap_or(0.0);
    if table
        .rows
        .get(row_index)
        .and_then(|row| row.cells.get(cell_index))
        .is_none_or(|cell| cell.vertical_merge != TableCellVerticalMerge::First)
    {
        return height;
    }
    let mut next_row = row_index + 1;
    while next_row < row_end {
        let continues = table
            .rows
            .get(next_row)
            .and_then(|row| row.cells.get(cell_index))
            .is_some_and(|cell| cell.vertical_merge == TableCellVerticalMerge::Continuation);
        if !continues {
            break;
        }
        let next_local_index = next_row.saturating_sub(row_start);
        height += row_heights.get(next_local_index).copied().unwrap_or(0.0);
        next_row += 1;
    }
    height
}

pub(in crate::layout) fn nested_vertical_merge_origin(
    table: &Table,
    row_index: usize,
    cell_index: usize,
) -> Option<(usize, usize)> {
    let current = table.rows.get(row_index)?.cells.get(cell_index)?;
    if current.vertical_merge != TableCellVerticalMerge::Continuation {
        return None;
    }
    for candidate_row in (0..row_index).rev() {
        let candidate = table.rows.get(candidate_row)?.cells.get(cell_index)?;
        match candidate.vertical_merge {
            TableCellVerticalMerge::First => return Some((candidate_row, cell_index)),
            TableCellVerticalMerge::Continuation => {}
            TableCellVerticalMerge::None => return None,
        }
    }
    None
}

pub(in crate::layout) fn nested_table_row_left(
    left: f32,
    width: f32,
    alignment: TableRowAlignment,
    column_widths: &[f32],
) -> f32 {
    let row_width = column_widths.iter().sum::<f32>().min(width.max(0.0));
    let remaining = (width - row_width).max(0.0);
    match alignment {
        TableRowAlignment::Center => left + remaining / 2.0,
        TableRowAlignment::Right => left + remaining,
        _ => left,
    }
}

pub(in crate::layout) fn nested_table_row_heights(
    table: &Table,
    row_start: usize,
    row_count: usize,
    height: f32,
) -> Vec<f32> {
    let default_height = height / row_count as f32;
    let mut heights = table
        .rows
        .iter()
        .skip(row_start)
        .take(row_count)
        .map(|row| {
            row.height_twips
                .map(|twips| twips_to_points(twips.saturating_abs()).max(1.0))
        })
        .collect::<Vec<_>>();
    let missing_count = heights.iter().filter(|height| height.is_none()).count();
    let requested_total = heights.iter().flatten().sum::<f32>();
    if missing_count > 0 {
        let remaining = (height - requested_total).max(0.0);
        let missing_height = if remaining > 0.0 {
            remaining / missing_count as f32
        } else {
            default_height
        };
        for row_height in &mut heights {
            if row_height.is_none() {
                *row_height = Some(missing_height);
            }
        }
    }
    let mut heights = heights
        .into_iter()
        .map(Option::unwrap_or_default)
        .collect::<Vec<_>>();
    let total = heights.iter().sum::<f32>();
    if total > height && total > 0.0 {
        let scale = height / total;
        for row_height in &mut heights {
            *row_height *= scale;
        }
    } else if total < height && !heights.is_empty() {
        let last = heights.len() - 1;
        heights[last] += height - total;
    }
    heights
}

pub(in crate::layout) fn next_row_continues_vertical_merge(
    row: &TableRow,
    next_row: Option<&TableRow>,
    cell_index: usize,
) -> bool {
    let Some(cell) = row.cells.get(cell_index) else {
        return false;
    };
    if !matches!(
        cell.vertical_merge,
        TableCellVerticalMerge::First | TableCellVerticalMerge::Continuation
    ) {
        return false;
    }
    next_row
        .and_then(|next_row| next_row.cells.get(cell_index))
        .is_some_and(|next_cell| next_cell.vertical_merge == TableCellVerticalMerge::Continuation)
}

pub(in crate::layout) fn table_border_stroke(
    border: &TableCellBorder,
    document: &Document,
) -> (f32, PdfColor, LineStyle) {
    let width = match border.style {
        BorderStyle::Hairline => 0.25,
        BorderStyle::Thick => twips_to_points(border.width_twips.max(1)).max(1.2),
        _ => twips_to_points(border.width_twips.max(1)).max(0.25),
    };
    let color = border
        .color_index
        .map(|index| color_for_index(document, index))
        .unwrap_or_default();
    let style = line_style_for_border_style(border.style);
    (width, color, style)
}

pub(in crate::layout) fn diagonal_table_border_stroke(
    border: &TableCellBorder,
    document: &Document,
) -> (f32, PdfColor, LineStyle) {
    let (width, color, style) = table_border_stroke(border, document);
    if style == LineStyle::Dashed {
        (0.14, color, LineStyle::FineDashed)
    } else {
        (width, color, style)
    }
}

pub(in crate::layout) fn page_border_stroke(
    border: &TableCellBorder,
    document: &Document,
) -> (f32, PdfColor, LineStyle) {
    let (mut width, color, style) = table_border_stroke(border, document);
    if border.width_twips == TableCellBorder::default().width_twips
        && border.style == BorderStyle::Single
    {
        width = 0.72;
    }
    (width, color, style)
}

pub(in crate::layout) fn layout_scissors_page_border_art(
    page: &mut LayoutPage,
    left: f32,
    right: f32,
    top: f32,
    document: &Document,
    border: &TableCellBorder,
) {
    let color = border
        .color_index
        .map(|index| color_for_index(document, index))
        .unwrap_or_default();
    let white = PdfColor {
        red: 1.0,
        green: 1.0,
        blue: 1.0,
    };
    let push_fill = |page: &mut LayoutPage, x: f32, y: f32, width: f32, height: f32, color| {
        if width > 0.0 && height > 0.0 {
            page.items.push(LayoutItem::Highlight {
                x,
                y,
                width,
                height,
                color,
            });
        }
    };

    // Word's ID-42 tile starts with a clipped scissors glyph. These bounded
    // primitives preserve its visible silhouette without retaining Office art payloads.
    page.items.push(LayoutItem::Ellipse {
        x: left + 5.33,
        y: top + 22.97,
        width: 7.25,
        height: 8.95,
        stroke_width: 0.0,
        stroke_color: color,
        stroke_style: LineStyle::Solid,
        fill_color: Some(color),
        fill_gradient_color: None,
        fill_pattern: ShadingPattern::None,
    });
    page.items.push(LayoutItem::Polygon {
        points: vec![
            LayoutPoint {
                x: left + 11.52,
                y: top + 17.54,
            },
            LayoutPoint {
                x: left + 2.76,
                y: top + 17.54,
            },
            LayoutPoint {
                x: left + 6.216,
                y: top + 20.4,
            },
            LayoutPoint {
                x: left + 15.216,
                y: top + 20.4,
            },
        ],
        paths: Vec::new(),
        overlay_paths: Vec::new(),
        fill_rule: StaticImageVectorFillRule::Winding,
        stroke_width: 0.0,
        stroke_color: color,
        stroke_style: LineStyle::Solid,
        fill_color: Some(color),
        fill_gradient_color: None,
        fill_pattern: ShadingPattern::None,
    });
    // Word restores the lower ring contours after painting the filled joint.
    // Keeping that order prevents the joint from leaking into the white hole.
    page.items.push(LayoutItem::Ellipse {
        x: left - 0.12,
        y: top + 13.34,
        width: 9.46,
        height: 7.42,
        stroke_width: 0.0,
        stroke_color: color,
        stroke_style: LineStyle::Solid,
        fill_color: Some(color),
        fill_gradient_color: None,
        fill_pattern: ShadingPattern::None,
    });
    page.items.push(LayoutItem::Ellipse {
        x: left + 1.08,
        y: top + 14.95,
        width: 6.77,
        height: 4.25,
        stroke_width: 0.0,
        stroke_color: white,
        stroke_style: LineStyle::Solid,
        fill_color: Some(white),
        fill_gradient_color: None,
        fill_pattern: ShadingPattern::None,
    });
    page.items.push(LayoutItem::Polygon {
        points: vec![
            LayoutPoint {
                x: left + 7.824,
                y: top + 24.6,
            },
            LayoutPoint {
                x: left + 11.04,
                y: top + 20.4,
            },
            LayoutPoint {
                x: left + 14.16,
                y: top + 20.4,
            },
            LayoutPoint {
                x: left + 11.04,
                y: top + 24.6,
            },
        ],
        paths: Vec::new(),
        overlay_paths: Vec::new(),
        fill_rule: StaticImageVectorFillRule::Winding,
        stroke_width: 0.0,
        stroke_color: color,
        stroke_style: LineStyle::Solid,
        fill_color: Some(color),
        fill_gradient_color: None,
        fill_pattern: ShadingPattern::None,
    });
    page.items.push(LayoutItem::Polygon {
        points: vec![
            LayoutPoint {
                x: left + 14.12,
                y: top + 20.41,
            },
            LayoutPoint {
                x: left + 10.81,
                y: top + 17.36,
            },
            LayoutPoint {
                x: left + 28.57,
                y: top + 17.39,
            },
            LayoutPoint {
                x: left + 32.12,
                y: top + 20.41,
            },
        ],
        paths: Vec::new(),
        overlay_paths: Vec::new(),
        fill_rule: StaticImageVectorFillRule::Winding,
        stroke_width: 0.14,
        stroke_color: color,
        stroke_style: LineStyle::Solid,
        fill_color: Some(white),
        fill_gradient_color: None,
        fill_pattern: ShadingPattern::None,
    });
    page.items.push(LayoutItem::Polygon {
        points: vec![
            LayoutPoint {
                x: left + 13.12,
                y: top + 17.36,
            },
            LayoutPoint {
                x: left + 16.04,
                y: top + 17.36,
            },
            LayoutPoint {
                x: left + 19.28,
                y: top + 13.28,
            },
            LayoutPoint {
                x: left + 16.04,
                y: top + 13.28,
            },
        ],
        paths: Vec::new(),
        overlay_paths: Vec::new(),
        fill_rule: StaticImageVectorFillRule::Winding,
        stroke_width: 0.14,
        stroke_color: color,
        stroke_style: LineStyle::Solid,
        fill_color: Some(white),
        fill_gradient_color: None,
        fill_pattern: ShadingPattern::None,
    });

    // Preserve Word's complete-tile cadence: a short lead-in, five long bars,
    // and a clipped final tile rather than stretching the pattern to the margin.
    push_fill(page, left + 20.06, top + 6.79, 12.0, 5.38, color);
    push_fill(page, left + 20.06, top - 0.12, 5.35, 12.29, color);
    for (x, width) in [
        (36.38, 25.34),
        (70.34, 25.34),
        (104.33, 25.34),
        (138.31, 25.34),
        (172.30, 25.42),
    ] {
        push_fill(page, left + x, top + 6.67, width, 5.26, color);
    }
    let final_left = (left + 202.06).min(right);
    let final_right = (left + 214.11).min(right);
    push_fill(
        page,
        final_left,
        top + 6.65,
        (final_right - final_left).max(0.0),
        5.23,
        color,
    );
    let cap_left = (left + 208.75).min(right);
    let cap_right = (left + 214.08).min(right);
    push_fill(
        page,
        cap_left,
        top - 0.12,
        (cap_right - cap_left).max(0.0),
        12.05,
        color,
    );
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn push_paragraph_borders(
    pages: &mut [LayoutPage],
    margin_left: f32,
    content_width: f32,
    style: &ParagraphStyle,
    line_idx: usize,
    line_count: usize,
    top_y: f32,
    line_height: f32,
    document: &Document,
    render_between_border_after: bool,
    allow_outward_side_gap: bool,
) {
    if line_count == 0 {
        return;
    }
    let x1 = margin_left
        + twips_to_points(paragraph_line_left_indent_twips(style, line_idx == 0))
            .min(content_width);
    let x2 = x1 + paragraph_line_width(content_width, style, line_idx == 0);
    let y1 = top_y;
    let y2 = top_y - line_height;
    let side_gap = if allow_outward_side_gap {
        DEFAULT_PARAGRAPH_BORDER_SIDE_GAP_POINTS
    } else {
        0.0
    };
    let Some(page) = pages.last_mut() else {
        return;
    };

    if style.borders.top.visible && line_idx == 0 {
        let border = &style.borders.top;
        let (width, color, style) = table_border_stroke(border, document);
        let spacing = twips_to_points(border.spacing_twips.max(0));
        page.items.push(LayoutItem::Line {
            x1: x1 - side_gap,
            y1: y1 + spacing + (width * 0.5),
            x2: x2 + side_gap,
            y2: y1 + spacing + (width * 0.5),
            width,
            color,
            style,
        });
    }
    if style.borders.bottom.visible && line_idx + 1 == line_count {
        let border = &style.borders.bottom;
        let (width, color, style) = table_border_stroke(border, document);
        let spacing = twips_to_points(border.spacing_twips.max(0));
        page.items.push(LayoutItem::Line {
            x1: x1 - side_gap,
            y1: y2 - spacing - (width * 0.5),
            x2: x2 + side_gap,
            y2: y2 - spacing - (width * 0.5),
            width,
            color,
            style,
        });
    }
    if render_between_border_after && style.borders.between.visible && line_idx + 1 == line_count {
        let border = &style.borders.between;
        let (width, color, style) = table_border_stroke(border, document);
        let spacing = twips_to_points(border.spacing_twips.max(0));
        page.items.push(LayoutItem::Line {
            x1: x1 - side_gap,
            y1: y2 - spacing,
            x2: x2 + side_gap,
            y2: y2 - spacing,
            width,
            color,
            style,
        });
    }
    if style.borders.left.visible {
        let border = &style.borders.left;
        let (width, color, style) = table_border_stroke(border, document);
        let spacing = twips_to_points(border.spacing_twips.max(0));
        let x = x1 - spacing - side_gap - (width * 0.5);
        page.items.push(LayoutItem::Line {
            x1: x,
            y1,
            x2: x,
            y2,
            width,
            color,
            style,
        });
    }
    if style.borders.right.visible {
        let border = &style.borders.right;
        let (width, color, style) = table_border_stroke(border, document);
        let spacing = twips_to_points(border.spacing_twips.max(0));
        let x = x2 + spacing + side_gap + (width * 0.5);
        page.items.push(LayoutItem::Line {
            x1: x,
            y1,
            x2: x,
            y2,
            width,
            color,
            style,
        });
    }
}

pub(in crate::layout) fn paragraph_top_border_reserve(
    style: &ParagraphStyle,
    document: &Document,
) -> f32 {
    if style.borders.top.visible {
        table_border_stroke(&style.borders.top, document).0
    } else {
        0.0
    }
}

pub(in crate::layout) fn paragraph_bottom_border_reserve(
    style: &ParagraphStyle,
    render_between_border_after: bool,
    document: &Document,
) -> f32 {
    let bottom = if style.borders.bottom.visible {
        table_border_stroke(&style.borders.bottom, document).0
    } else {
        0.0
    };
    let between = if render_between_border_after && style.borders.between.visible {
        table_border_stroke(&style.borders.between, document).0
    } else {
        0.0
    };
    bottom.max(between)
}
