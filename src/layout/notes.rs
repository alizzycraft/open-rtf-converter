//! Footnote and endnote reservation, numbering, placement, and separators.

use super::*;

pub(in crate::layout) fn layout_remaining_footnotes(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    document: &Document,
    rendered_footnote_pages: &mut [Option<usize>],
    content_width: f32,
    margin_left: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) {
    let entries = document
        .footnotes
        .iter()
        .enumerate()
        .filter(|(idx, _)| {
            rendered_footnote_pages
                .get(*idx)
                .is_none_or(Option::is_none)
        })
        .map(|(idx, note)| (idx, note.clone()))
        .collect::<Vec<_>>();
    if entries.is_empty() {
        return;
    }

    layout_footnote_entries(
        pages,
        cursor_y,
        &entries,
        document,
        content_width,
        margin_left,
        margin_bottom,
        geometry,
        document_stats,
        font_provider,
        rendered_footnote_pages,
    );
}

pub(in crate::layout) fn layout_footnotes_for_section(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    document: &Document,
    rendered_footnote_pages: &mut [Option<usize>],
    section_number: usize,
    content_width: f32,
    margin_left: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) {
    let entries = document
        .footnotes
        .iter()
        .enumerate()
        .filter(|(idx, _)| {
            rendered_footnote_pages
                .get(*idx)
                .is_none_or(Option::is_none)
        })
        .filter(|(idx, _)| {
            document
                .footnote_section_indices
                .get(*idx)
                .copied()
                .unwrap_or(1)
                == section_number
        })
        .map(|(idx, note)| (idx, note.clone()))
        .collect::<Vec<_>>();
    if entries.is_empty() {
        return;
    }

    layout_footnote_entries(
        pages,
        cursor_y,
        &entries,
        document,
        content_width,
        margin_left,
        margin_bottom,
        geometry,
        document_stats,
        font_provider,
        rendered_footnote_pages,
    );
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn layout_footnotes_until_block(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    document: &Document,
    rendered_footnote_pages: &mut [Option<usize>],
    block_index: usize,
    content_width: f32,
    margin_left: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) {
    let entries = footnote_entries_until_block(document, rendered_footnote_pages, block_index);
    if entries.is_empty() {
        return;
    }

    layout_footnote_entries(
        pages,
        cursor_y,
        &entries,
        document,
        content_width,
        margin_left,
        margin_bottom,
        geometry,
        document_stats,
        font_provider,
        rendered_footnote_pages,
    );
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn layout_footnotes_until_index(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    document: &Document,
    rendered_footnote_pages: &mut [Option<usize>],
    max_note_index: usize,
    content_width: f32,
    margin_left: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) {
    let entries = document
        .footnotes
        .iter()
        .enumerate()
        .filter(|(idx, _)| *idx <= max_note_index)
        .filter(|(idx, _)| {
            rendered_footnote_pages
                .get(*idx)
                .is_none_or(Option::is_none)
        })
        .map(|(idx, note)| (idx, note.clone()))
        .collect::<Vec<_>>();
    if entries.is_empty() {
        return;
    }

    layout_footnote_entries(
        pages,
        cursor_y,
        &entries,
        document,
        content_width,
        margin_left,
        margin_bottom,
        geometry,
        document_stats,
        font_provider,
        rendered_footnote_pages,
    );
}

pub(in crate::layout) fn footnote_entries_until_block(
    document: &Document,
    rendered_footnote_pages: &[Option<usize>],
    block_index: usize,
) -> Vec<(usize, Paragraph)> {
    document
        .footnotes
        .iter()
        .enumerate()
        .filter(|(idx, _)| {
            rendered_footnote_pages
                .get(*idx)
                .is_none_or(Option::is_none)
        })
        .filter(|(idx, _)| {
            document
                .footnote_block_indices
                .get(*idx)
                .copied()
                .unwrap_or(usize::MAX)
                <= block_index
        })
        .map(|(idx, note)| (idx, note.clone()))
        .collect()
}

pub(in crate::layout) fn footnote_entries_until_index(
    document: &Document,
    rendered_footnote_pages: &[Option<usize>],
    max_note_index: usize,
) -> Vec<(usize, Paragraph)> {
    document
        .footnotes
        .iter()
        .enumerate()
        .filter(|(idx, _)| *idx <= max_note_index)
        .filter(|(idx, _)| {
            rendered_footnote_pages
                .get(*idx)
                .is_none_or(Option::is_none)
        })
        .map(|(idx, note)| (idx, note.clone()))
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn layout_footnote_entries(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    entries: &[(usize, Paragraph)],
    document: &Document,
    content_width: f32,
    mut margin_left: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
    rendered_footnote_pages: &mut [Option<usize>],
) {
    if entries.is_empty() {
        return;
    }

    if document.footnote_placement == FootnotePlacement::BottomOfPage {
        position_cursor_for_bottom_note_entries(
            pages,
            cursor_y,
            entries,
            document,
            content_width,
            margin_bottom,
            geometry,
            document_stats,
            font_provider,
        );
        margin_left = geometry.body_left(0);
    }

    if *cursor_y - 18.0 < margin_bottom {
        let mut column = 0;
        start_new_page(pages, cursor_y, geometry, &mut column);
        margin_left = geometry.body_left(0);
    }

    push_note_separator(
        pages,
        cursor_y,
        document.footnote_separator.as_ref(),
        margin_left,
        content_width,
        margin_bottom,
        geometry,
        document,
        document_stats,
        font_provider,
    );

    for (idx, footnote) in entries {
        let numbering = footnote_numbering_for(document, *idx);
        let note_page = pages.len().max(1);
        let sequence =
            footnote_sequence_for_rendered_page(document, *idx, rendered_footnote_pages, note_page);
        let paragraph = note_display_paragraph(
            footnote,
            numbering.start,
            sequence,
            numbering.format,
            numbering.word_compact_label,
        );
        let markers = current_marker_context(pages, document_stats);
        layout_footnote_paragraph(
            pages,
            cursor_y,
            &paragraph,
            content_width,
            margin_bottom,
            geometry,
            document,
            document_stats,
            &markers,
            font_provider,
        );
        if let Some(rendered_page) = rendered_footnote_pages.get_mut(*idx) {
            *rendered_page = Some(note_page);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn layout_footnote_paragraph(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    paragraph: &Paragraph,
    content_width: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    document: &Document,
    document_stats: DocumentStats,
    markers: &MarkerContext,
    font_provider: Option<&FontProvider>,
) {
    let mut footnote_column = 0;
    let mut continuation_hook =
        |pages: &mut Vec<LayoutPage>, cursor_y: &mut f32, geometry: &PageGeometry| {
            let mut separator_geometry = *geometry;
            push_note_separator(
                pages,
                cursor_y,
                document.footnote_continuation_separator.as_ref(),
                geometry.body_left(0),
                content_width,
                margin_bottom,
                &mut separator_geometry,
                document,
                document_stats,
                font_provider,
            );
        };
    layout_paragraph_with_auto_footnotes(
        pages,
        cursor_y,
        paragraph,
        false,
        false,
        false,
        content_width,
        margin_bottom,
        geometry,
        &mut footnote_column,
        document,
        document_stats,
        None,
        markers,
        font_provider,
        None,
        Some(&mut continuation_hook),
        None,
    );
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn push_note_separator(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    separator: Option<&Paragraph>,
    margin_left: f32,
    content_width: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    document: &Document,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) {
    if let Some(separator) = separator.filter(|paragraph| !paragraph.runs.is_empty()) {
        let mut separator_column = 0;
        let markers = current_marker_context(pages, document_stats);
        layout_paragraph(
            pages,
            cursor_y,
            separator,
            false,
            false,
            false,
            content_width,
            margin_bottom,
            geometry,
            &mut separator_column,
            document,
            document_stats,
            None,
            &markers,
            font_provider,
        );
        return;
    }

    push_default_note_separator(pages, cursor_y, margin_left, content_width);
}

pub(in crate::layout) fn push_default_note_separator(
    pages: &mut [LayoutPage],
    cursor_y: &mut f32,
    margin_left: f32,
    content_width: f32,
) {
    let Some(page) = pages.last_mut() else {
        return;
    };
    page.items.push(LayoutItem::Line {
        x1: margin_left,
        y1: *cursor_y - 3.0,
        x2: margin_left + content_width.min(144.0),
        y2: *cursor_y - 3.0,
        width: 0.6,
        color: PdfColor::default(),
        style: LineStyle::Solid,
    });
    *cursor_y -= 9.0;
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn position_cursor_for_bottom_note_entries(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    entries: &[(usize, Paragraph)],
    document: &Document,
    content_width: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) {
    let note_height = footnote_entries_block_height(
        entries,
        content_width,
        document,
        document_stats,
        font_provider,
    );
    let body_top = geometry.height - geometry.margin_top;
    let target_cursor_y = (margin_bottom + note_height).min(body_top);
    if *cursor_y < target_cursor_y {
        let mut column = 0;
        start_new_page(pages, cursor_y, geometry, &mut column);
    }
    *cursor_y = target_cursor_y;
}

pub(in crate::layout) fn footnote_entries_block_height(
    entries: &[(usize, Paragraph)],
    content_width: f32,
    document: &Document,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) -> f32 {
    let compact_word_band = entries
        .first()
        .is_some_and(|(idx, _)| footnote_numbering_for(document, *idx).word_compact_label);
    let mut height = 9.0
        + if compact_word_band {
            WORD_COMPACT_BOTTOM_NOTE_RESERVATION_LEAD_POINTS
        } else {
            0.0
        };
    let markers = marker_context("1".to_string(), "1".to_string(), document_stats);
    for (idx, footnote) in entries {
        let numbering = footnote_numbering_for(document, *idx);
        let sequence = footnote_sequence_for_index(document, *idx);
        let paragraph = note_display_paragraph(
            footnote,
            numbering.start,
            sequence,
            numbering.format,
            numbering.word_compact_label,
        );
        height += twips_to_points(effective_space_before_twips(&paragraph.style));
        height += wrap_paragraph_with_font_provider(
            &paragraph,
            content_width,
            &markers,
            document,
            font_provider,
        )
        .into_iter()
        .map(|line| apply_line_spacing(line.height, &paragraph.style))
        .sum::<f32>();
        height += twips_to_points(effective_space_after_twips(&paragraph.style));
    }
    height
}

pub(in crate::layout) fn footnote_reservation_margin_bottom(
    document: &Document,
    rendered_footnote_pages: &[Option<usize>],
    max_note_index: usize,
    content_width: f32,
    geometry: PageGeometry,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) -> f32 {
    let entries = footnote_entries_until_index(document, rendered_footnote_pages, max_note_index);
    if entries.is_empty() {
        return geometry.margin_bottom;
    }
    let reserve_height = footnote_entries_block_height(
        &entries,
        content_width,
        document,
        document_stats,
        font_provider,
    );
    let body_top = geometry.height - geometry.margin_top;
    let max_reserved_margin = (body_top - 18.0).max(geometry.margin_bottom);
    (geometry.margin_bottom + reserve_height).min(max_reserved_margin)
}

pub(in crate::layout) fn footnote_numbering_for(
    document: &Document,
    index: usize,
) -> NoteNumbering {
    document
        .footnote_numbering
        .get(index)
        .copied()
        .unwrap_or(NoteNumbering {
            start: document.footnote_number_start,
            format: document.footnote_number_format,
            restart: document.footnote_number_restart,
            word_compact_label: false,
        })
}

pub(in crate::layout) fn endnote_numbering_for(document: &Document, index: usize) -> NoteNumbering {
    document
        .endnote_numbering
        .get(index)
        .copied()
        .unwrap_or(NoteNumbering {
            start: document.endnote_number_start,
            format: document.endnote_number_format,
            restart: document.endnote_number_restart,
            word_compact_label: false,
        })
}

pub(in crate::layout) fn footnote_sequence_for_index(document: &Document, index: usize) -> usize {
    match footnote_numbering_for(document, index).restart {
        NoteNumberRestart::Continuous => index.saturating_add(1),
        NoteNumberRestart::EachSection => note_sequence_for_index(
            NoteNumberRestart::EachSection,
            &document.footnote_section_indices,
            index,
        ),
        NoteNumberRestart::EachPage => {
            let page_group = explicit_page_group_for_footnote(document, index);
            document
                .footnote_block_indices
                .iter()
                .enumerate()
                .take(index.saturating_add(1))
                .filter(|(candidate_idx, _)| {
                    explicit_page_group_for_footnote(document, *candidate_idx) == page_group
                })
                .count()
                .max(1)
        }
    }
}

pub(in crate::layout) fn footnote_sequence_for_rendered_page(
    document: &Document,
    index: usize,
    rendered_footnote_pages: &[Option<usize>],
    current_page: usize,
) -> usize {
    match footnote_numbering_for(document, index).restart {
        NoteNumberRestart::Continuous => index.saturating_add(1),
        NoteNumberRestart::EachSection => note_sequence_for_index(
            NoteNumberRestart::EachSection,
            &document.footnote_section_indices,
            index,
        ),
        NoteNumberRestart::EachPage => rendered_footnote_pages
            .iter()
            .enumerate()
            .take(index.saturating_add(1))
            .filter(|(candidate_idx, rendered_page)| {
                if *candidate_idx == index {
                    true
                } else {
                    rendered_page.is_some_and(|page| page == current_page)
                }
            })
            .count()
            .max(1),
    }
}

pub(in crate::layout) fn endnote_sequence_for_rendered_page(
    document: &Document,
    index: usize,
    rendered_endnote_pages: &[Option<usize>],
    current_page: usize,
) -> usize {
    match endnote_numbering_for(document, index).restart {
        NoteNumberRestart::Continuous => index.saturating_add(1),
        NoteNumberRestart::EachSection => note_sequence_for_index(
            NoteNumberRestart::EachSection,
            &document.endnote_section_indices,
            index,
        ),
        NoteNumberRestart::EachPage => rendered_endnote_pages
            .iter()
            .enumerate()
            .take(index.saturating_add(1))
            .filter(|(candidate_idx, rendered_page)| {
                if *candidate_idx == index {
                    true
                } else {
                    rendered_page.is_some_and(|page| page == current_page)
                }
            })
            .count()
            .max(1),
    }
}

pub(in crate::layout) fn explicit_page_group_for_footnote(
    document: &Document,
    index: usize,
) -> usize {
    let block_index = document
        .footnote_block_indices
        .get(index)
        .copied()
        .unwrap_or(usize::MAX);
    document
        .blocks
        .iter()
        .take(block_index)
        .filter(|block| matches!(block, Block::PageBreak))
        .count()
        .saturating_add(1)
}

pub(in crate::layout) fn note_sequence_for_index(
    restart: NoteNumberRestart,
    section_indices: &[usize],
    index: usize,
) -> usize {
    match restart {
        NoteNumberRestart::Continuous | NoteNumberRestart::EachPage => index.saturating_add(1),
        NoteNumberRestart::EachSection => {
            let section = section_indices.get(index).copied().unwrap_or(1);
            section_indices
                .iter()
                .take(index.saturating_add(1))
                .filter(|candidate| **candidate == section)
                .count()
                .max(1)
        }
    }
}

pub(in crate::layout) fn note_display_paragraph(
    footnote: &Paragraph,
    number_start: i32,
    sequence: usize,
    number_format: PageNumberFormat,
    word_compact_label: bool,
) -> Paragraph {
    let mut paragraph = footnote.clone();
    if let Some(first_run) = paragraph.runs.first_mut() {
        let label = format_note_number(number_start, sequence, number_format);
        let mut label_style = first_run.style.clone();
        label_style.baseline_shift_half_points = if word_compact_label {
            0
        } else {
            PASSIVE_NOTE_LABEL_SHIFT_HALF_POINTS
        };
        label_style.font_size_scale_percent = if word_compact_label {
            100
        } else {
            PASSIVE_NOTE_LABEL_FONT_SCALE_PERCENT
        };
        if word_compact_label {
            label_style.font_size_half_points = label_style.font_size_half_points.max(2);
        } else {
            label_style.font_size_half_points = label_style.font_size_half_points.min(20).max(2);
            first_run.text = format!(". {}", first_run.text);
            first_run.style.font_size_half_points =
                first_run.style.font_size_half_points.min(20).max(2);
        }
        paragraph.runs.insert(
            0,
            Run {
                text: label,
                style: label_style,
            },
        );
    }
    paragraph.style.space_before_twips = if word_compact_label {
        paragraph.style.space_before_twips.max(26)
    } else {
        paragraph.style.space_before_twips.min(60)
    };
    paragraph.style.space_after_twips = if word_compact_label {
        paragraph.style.space_after_twips.max(250)
    } else {
        paragraph.style.space_after_twips.min(60)
    };
    paragraph
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn layout_endnote_entries(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    endnotes: &[(usize, Paragraph)],
    content_width: f32,
    margin_left: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    document: &Document,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
    rendered_endnote_pages: &mut [Option<usize>],
) {
    if endnotes.is_empty() {
        return;
    }
    if *cursor_y - 18.0 < margin_bottom {
        let mut column = 0;
        start_new_page(pages, cursor_y, geometry, &mut column);
    }
    if endnotes
        .first()
        .is_some_and(|(idx, _)| endnote_numbering_for(document, *idx).word_compact_label)
    {
        *cursor_y -= twips_to_points(86);
    }
    push_note_separator(
        pages,
        cursor_y,
        document.endnote_separator.as_ref(),
        margin_left,
        content_width,
        margin_bottom,
        geometry,
        document,
        document_stats,
        font_provider,
    );

    for (idx, endnote) in endnotes {
        let numbering = endnote_numbering_for(document, *idx);
        let note_page = pages.len().max(1);
        let sequence =
            endnote_sequence_for_rendered_page(document, *idx, rendered_endnote_pages, note_page);
        let paragraph = note_display_paragraph(
            endnote,
            numbering.start,
            sequence,
            numbering.format,
            numbering.word_compact_label,
        );
        let markers = current_marker_context(pages, document_stats);
        layout_endnote_paragraph(
            pages,
            cursor_y,
            &paragraph,
            content_width,
            margin_bottom,
            geometry,
            document,
            document_stats,
            &markers,
            font_provider,
        );
        if let Some(rendered_page) = rendered_endnote_pages.get_mut(*idx) {
            *rendered_page = Some(note_page);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn layout_endnote_paragraph(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    paragraph: &Paragraph,
    content_width: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    document: &Document,
    document_stats: DocumentStats,
    markers: &MarkerContext,
    font_provider: Option<&FontProvider>,
) {
    let mut endnote_column = 0;
    let mut continuation_hook =
        |pages: &mut Vec<LayoutPage>, cursor_y: &mut f32, geometry: &PageGeometry| {
            let mut separator_geometry = *geometry;
            push_note_separator(
                pages,
                cursor_y,
                document.endnote_continuation_separator.as_ref(),
                geometry.body_left(0),
                content_width,
                margin_bottom,
                &mut separator_geometry,
                document,
                document_stats,
                font_provider,
            );
        };
    layout_paragraph_with_auto_footnotes(
        pages,
        cursor_y,
        paragraph,
        false,
        false,
        false,
        content_width,
        margin_bottom,
        geometry,
        &mut endnote_column,
        document,
        document_stats,
        None,
        markers,
        font_provider,
        None,
        Some(&mut continuation_hook),
        None,
    );
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn layout_endnotes_for_placement(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    document: &Document,
    rendered_endnote_pages: &mut [Option<usize>],
    placement: EndnotePlacement,
    content_width: f32,
    margin_left: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) {
    let entries = document
        .endnotes
        .iter()
        .enumerate()
        .filter(|(idx, _)| rendered_endnote_pages.get(*idx).is_none_or(Option::is_none))
        .filter(|(idx, _)| endnote_placement_for(document, *idx) == placement)
        .map(|(idx, note)| (idx, note.clone()))
        .collect::<Vec<_>>();
    if entries.is_empty() {
        return;
    }
    layout_endnote_entries(
        pages,
        cursor_y,
        &entries,
        content_width,
        margin_left,
        margin_bottom,
        geometry,
        document,
        document_stats,
        font_provider,
        rendered_endnote_pages,
    );
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn layout_notes_for_section_boundary(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    document: &Document,
    rendered_footnote_pages: &mut [Option<usize>],
    rendered_endnote_pages: &mut [Option<usize>],
    section_number: usize,
    content_width: f32,
    margin_left: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) {
    // Endnotes are body-flow content at the section boundary. Commit them before
    // reserving the page-bottom footnote band so both note kinds can share the page.
    layout_endnotes_for_section(
        pages,
        cursor_y,
        document,
        rendered_endnote_pages,
        section_number,
        content_width,
        margin_left,
        margin_bottom,
        geometry,
        document_stats,
        font_provider,
    );
    layout_footnotes_for_section(
        pages,
        cursor_y,
        document,
        rendered_footnote_pages,
        section_number,
        content_width,
        margin_left,
        margin_bottom,
        geometry,
        document_stats,
        font_provider,
    );
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn layout_endnotes_for_section(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    document: &Document,
    rendered_endnote_pages: &mut [Option<usize>],
    section_number: usize,
    content_width: f32,
    margin_left: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) {
    let mut section_notes = Vec::new();
    for (idx, endnote) in document.endnotes.iter().enumerate() {
        if rendered_endnote_pages.get(idx).is_some_and(Option::is_some) {
            continue;
        }
        if endnote_placement_for(document, idx) != EndnotePlacement::EndOfSection {
            continue;
        }
        let note_section = document
            .endnote_section_indices
            .get(idx)
            .copied()
            .unwrap_or(1);
        if note_section == section_number {
            section_notes.push((idx, endnote.clone()));
        }
    }
    if section_notes.is_empty() {
        return;
    }

    layout_endnote_entries(
        pages,
        cursor_y,
        &section_notes,
        content_width,
        margin_left,
        margin_bottom,
        geometry,
        document,
        document_stats,
        font_provider,
        rendered_endnote_pages,
    );
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn layout_remaining_section_endnotes(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    document: &Document,
    rendered_endnote_pages: &mut [Option<usize>],
    content_width: f32,
    margin_left: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) {
    let mut sections = document
        .endnotes
        .iter()
        .enumerate()
        .filter(|(idx, _)| rendered_endnote_pages.get(*idx).is_none_or(Option::is_none))
        .filter(|(idx, _)| endnote_placement_for(document, *idx) == EndnotePlacement::EndOfSection)
        .map(|(idx, _)| {
            document
                .endnote_section_indices
                .get(idx)
                .copied()
                .unwrap_or(1)
        })
        .collect::<Vec<_>>();
    sections.sort_unstable();
    sections.dedup();

    for section_number in sections {
        layout_endnotes_for_section(
            pages,
            cursor_y,
            document,
            rendered_endnote_pages,
            section_number,
            content_width,
            margin_left,
            margin_bottom,
            geometry,
            document_stats,
            font_provider,
        );
    }
}

pub(in crate::layout) fn endnote_placement_for(
    document: &Document,
    index: usize,
) -> EndnotePlacement {
    document
        .endnote_placements
        .get(index)
        .copied()
        .unwrap_or(document.endnote_placement)
}
