//! Transitional validation for normalized-model protocols that are not yet
//! enforced by Rust types.

use std::collections::HashSet;

use thiserror::Error;

use super::*;

const MAX_TRANSITIONAL_NESTED_TABLE_DEPTH: usize = 128;

/// A violated invariant in the normalized document model.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ModelInvariantError {
    #[error("parallel model vector {vector} has length {actual}; expected {expected}")]
    ParallelVectorLength {
        vector: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("{kind} index {index} at {path} is outside the valid range 0..{len}")]
    IndexOutOfBounds {
        kind: &'static str,
        index: usize,
        len: usize,
        path: String,
    },
    #[error("font index {index} at {path} does not identify a document font")]
    UnknownFontIndex { index: i32, path: String },
    #[error("document font index {index} occurs more than once")]
    DuplicateFontIndex { index: i32 },
    #[error("semantic inline node at {path} must not also carry text")]
    InlineKindWithText { path: String },
    #[error("pending note reference at {path} survived parser normalization")]
    PendingNoteReference { path: String },
    #[error("nested table depth {depth} at {path} exceeds the transitional bound {limit}")]
    NestedTableDepthExceeded {
        depth: usize,
        limit: usize,
        path: String,
    },
}

/// Validate invariants that remain implicit while the model is migrated to
/// typed inline nodes and aggregate note/image records.
pub(crate) fn validate_document_invariants(document: &Document) -> Result<(), ModelInvariantError> {
    let section_count = 1usize.saturating_add(
        document
            .blocks
            .iter()
            .filter(|block| {
                matches!(
                    block,
                    Block::ContinuousSectionBreak
                        | Block::SectionBreak
                        | Block::EvenPageSectionBreak
                        | Block::OddPageSectionBreak
                )
            })
            .count(),
    );
    for (note, entry) in document.footnotes.iter().enumerate() {
        validate_one_based_index(
            "footnote section",
            entry.section_index.get(),
            section_count,
            format!("footnotes[{note}]"),
        )?;
        validate_index(
            "footnote block",
            entry.body_block_index.get(),
            document.blocks.len(),
            format!("footnotes[{note}]"),
        )?;
    }
    for (note, entry) in document.endnotes.iter().enumerate() {
        validate_one_based_index(
            "endnote section",
            entry.section_index.get(),
            section_count,
            format!("endnotes[{note}]"),
        )?;
    }
    for (image, entry) in document.inline_images.iter().enumerate() {
        validate_index(
            "inline-image block",
            entry.body_block_index.get(),
            document.blocks.len(),
            format!("inline_images[{image}]"),
        )?;
    }

    let mut font_indexes = HashSet::with_capacity(document.fonts.len());
    for font in &document.fonts {
        if !font_indexes.insert(font.index) {
            return Err(ModelInvariantError::DuplicateFontIndex { index: font.index });
        }
    }

    let context = ValidationContext {
        fonts: &font_indexes,
        color_count: document.colors.len(),
        inline_image_count: document.inline_images.len(),
        footnote_count: document.footnotes.len(),
        endnote_count: document.endnotes.len(),
    };

    validate_page_settings(&document.page, "page", &context)?;
    validate_paragraph_list(&document.header, "header", &context)?;
    validate_paragraph_list(&document.first_page_header, "first_page_header", &context)?;
    validate_paragraph_list(&document.even_page_header, "even_page_header", &context)?;
    validate_paragraph_list(&document.footer, "footer", &context)?;
    validate_paragraph_list(&document.first_page_footer, "first_page_footer", &context)?;
    validate_paragraph_list(&document.even_page_footer, "even_page_footer", &context)?;
    validate_optional_paragraph(&document.footnote_separator, "footnote_separator", &context)?;
    validate_optional_paragraph(
        &document.footnote_continuation_separator,
        "footnote_continuation_separator",
        &context,
    )?;
    validate_optional_paragraph(&document.endnote_separator, "endnote_separator", &context)?;
    validate_optional_paragraph(
        &document.endnote_continuation_separator,
        "endnote_continuation_separator",
        &context,
    )?;
    for (index, note) in document.footnotes.iter().enumerate() {
        validate_paragraph(&note.content, &format!("footnotes[{index}]"), &context)?;
    }
    for (index, note) in document.endnotes.iter().enumerate() {
        validate_paragraph(&note.content, &format!("endnotes[{index}]"), &context)?;
    }

    validate_shape_list(&document.header_shapes, "header_shapes", &context)?;
    validate_shape_list(
        &document.first_page_header_shapes,
        "first_page_header_shapes",
        &context,
    )?;
    validate_shape_list(
        &document.even_page_header_shapes,
        "even_page_header_shapes",
        &context,
    )?;
    validate_shape_list(&document.footer_shapes, "footer_shapes", &context)?;
    validate_shape_list(
        &document.first_page_footer_shapes,
        "first_page_footer_shapes",
        &context,
    )?;
    validate_shape_list(
        &document.even_page_footer_shapes,
        "even_page_footer_shapes",
        &context,
    )?;
    validate_shape_list(&document.background_shapes, "background_shapes", &context)?;

    for (index, block) in document.blocks.iter().enumerate() {
        let path = format!("blocks[{index}]");
        match block {
            Block::Paragraph(paragraph) => validate_paragraph(paragraph, &path, &context)?,
            Block::Table(table) => validate_table(table, &path, 1, &context)?,
            Block::Shape(shape) => validate_shape(shape, &path, &context)?,
            Block::SectionSettings(settings) => validate_page_settings(settings, &path, &context)?,
            Block::Image(_)
            | Block::Placeholder(_)
            | Block::PageBreak
            | Block::ColumnBreak
            | Block::ContinuousSectionBreak
            | Block::SectionBreak
            | Block::EvenPageSectionBreak
            | Block::OddPageSectionBreak => {}
        }
    }
    Ok(())
}

/// Normalize undefined parser font IDs to one bounded unnamed Roman fallback.
/// The empty name keeps this internal record out of font-family inspection,
/// while the Roman hint preserves layout's prior Times fallback behavior.
pub(crate) fn normalize_missing_font_references(document: &mut Document) {
    let known = document
        .fonts
        .iter()
        .map(|font| font.index)
        .collect::<HashSet<_>>();
    let mut fallback_index = -1;
    while known.contains(&fallback_index) {
        fallback_index = fallback_index.saturating_sub(1);
    }
    let mut changed = false;

    normalize_paragraph_fonts(&mut document.header, &known, fallback_index, &mut changed);
    normalize_paragraph_fonts(
        &mut document.first_page_header,
        &known,
        fallback_index,
        &mut changed,
    );
    normalize_paragraph_fonts(
        &mut document.even_page_header,
        &known,
        fallback_index,
        &mut changed,
    );
    normalize_paragraph_fonts(&mut document.footer, &known, fallback_index, &mut changed);
    normalize_paragraph_fonts(
        &mut document.first_page_footer,
        &known,
        fallback_index,
        &mut changed,
    );
    normalize_paragraph_fonts(
        &mut document.even_page_footer,
        &known,
        fallback_index,
        &mut changed,
    );
    normalize_optional_paragraph_font(
        &mut document.footnote_separator,
        &known,
        fallback_index,
        &mut changed,
    );
    normalize_optional_paragraph_font(
        &mut document.footnote_continuation_separator,
        &known,
        fallback_index,
        &mut changed,
    );
    normalize_optional_paragraph_font(
        &mut document.endnote_separator,
        &known,
        fallback_index,
        &mut changed,
    );
    normalize_optional_paragraph_font(
        &mut document.endnote_continuation_separator,
        &known,
        fallback_index,
        &mut changed,
    );
    for note in &mut document.footnotes {
        normalize_paragraph_font(&mut note.content, &known, fallback_index, &mut changed);
    }
    for note in &mut document.endnotes {
        normalize_paragraph_font(&mut note.content, &known, fallback_index, &mut changed);
    }
    for shapes in [
        &mut document.header_shapes,
        &mut document.first_page_header_shapes,
        &mut document.even_page_header_shapes,
        &mut document.footer_shapes,
        &mut document.first_page_footer_shapes,
        &mut document.even_page_footer_shapes,
        &mut document.background_shapes,
    ] {
        for shape in shapes {
            normalize_paragraph_fonts(&mut shape.text, &known, fallback_index, &mut changed);
        }
    }
    for block in &mut document.blocks {
        match block {
            Block::Paragraph(paragraph) => {
                normalize_paragraph_font(paragraph, &known, fallback_index, &mut changed)
            }
            Block::Table(table) => {
                normalize_table_fonts(table, &known, fallback_index, &mut changed)
            }
            Block::Shape(shape) => {
                normalize_paragraph_fonts(&mut shape.text, &known, fallback_index, &mut changed)
            }
            Block::Image(_)
            | Block::Placeholder(_)
            | Block::PageBreak
            | Block::ColumnBreak
            | Block::ContinuousSectionBreak
            | Block::SectionBreak
            | Block::EvenPageSectionBreak
            | Block::OddPageSectionBreak
            | Block::SectionSettings(_) => {}
        }
    }

    if changed {
        document.fonts.push(FontDef {
            index: fallback_index,
            name: String::new(),
            alternate_name: None,
            charset: None,
            code_page: None,
            family: FontFamilyHint::Roman,
            pitch: FontPitch::Default,
        });
    }
}

/// Normalize undefined direct character-style colors to one bounded default
/// color entry. Border and shading indexes remain safely inert by design.
pub(crate) fn normalize_missing_color_references(document: &mut Document) {
    let valid_len = document.colors.len();
    let fallback_index = valid_len;
    let mut changed = false;

    normalize_paragraph_colors(
        &mut document.header,
        valid_len,
        fallback_index,
        &mut changed,
    );
    normalize_paragraph_colors(
        &mut document.first_page_header,
        valid_len,
        fallback_index,
        &mut changed,
    );
    normalize_paragraph_colors(
        &mut document.even_page_header,
        valid_len,
        fallback_index,
        &mut changed,
    );
    normalize_paragraph_colors(
        &mut document.footer,
        valid_len,
        fallback_index,
        &mut changed,
    );
    normalize_paragraph_colors(
        &mut document.first_page_footer,
        valid_len,
        fallback_index,
        &mut changed,
    );
    normalize_paragraph_colors(
        &mut document.even_page_footer,
        valid_len,
        fallback_index,
        &mut changed,
    );
    normalize_optional_paragraph_color(
        &mut document.footnote_separator,
        valid_len,
        fallback_index,
        &mut changed,
    );
    normalize_optional_paragraph_color(
        &mut document.footnote_continuation_separator,
        valid_len,
        fallback_index,
        &mut changed,
    );
    normalize_optional_paragraph_color(
        &mut document.endnote_separator,
        valid_len,
        fallback_index,
        &mut changed,
    );
    normalize_optional_paragraph_color(
        &mut document.endnote_continuation_separator,
        valid_len,
        fallback_index,
        &mut changed,
    );
    for note in &mut document.footnotes {
        normalize_paragraph_color(&mut note.content, valid_len, fallback_index, &mut changed);
    }
    for note in &mut document.endnotes {
        normalize_paragraph_color(&mut note.content, valid_len, fallback_index, &mut changed);
    }
    for shapes in [
        &mut document.header_shapes,
        &mut document.first_page_header_shapes,
        &mut document.even_page_header_shapes,
        &mut document.footer_shapes,
        &mut document.first_page_footer_shapes,
        &mut document.even_page_footer_shapes,
        &mut document.background_shapes,
    ] {
        for shape in shapes {
            normalize_paragraph_colors(&mut shape.text, valid_len, fallback_index, &mut changed);
        }
    }
    for block in &mut document.blocks {
        match block {
            Block::Paragraph(paragraph) => {
                normalize_paragraph_color(paragraph, valid_len, fallback_index, &mut changed)
            }
            Block::Table(table) => {
                normalize_table_colors(table, valid_len, fallback_index, &mut changed)
            }
            Block::Shape(shape) => {
                normalize_paragraph_colors(&mut shape.text, valid_len, fallback_index, &mut changed)
            }
            Block::Image(_)
            | Block::Placeholder(_)
            | Block::PageBreak
            | Block::ColumnBreak
            | Block::ContinuousSectionBreak
            | Block::SectionBreak
            | Block::EvenPageSectionBreak
            | Block::OddPageSectionBreak
            | Block::SectionSettings(_) => {}
        }
    }

    if changed {
        document.colors.push(Color::default());
    }
}

fn normalize_optional_paragraph_color(
    paragraph: &mut Option<Paragraph>,
    valid_len: usize,
    fallback_index: usize,
    changed: &mut bool,
) {
    if let Some(paragraph) = paragraph {
        normalize_paragraph_color(paragraph, valid_len, fallback_index, changed);
    }
}

fn normalize_paragraph_colors(
    paragraphs: &mut [Paragraph],
    valid_len: usize,
    fallback_index: usize,
    changed: &mut bool,
) {
    for paragraph in paragraphs {
        normalize_paragraph_color(paragraph, valid_len, fallback_index, changed);
    }
}

fn normalize_paragraph_color(
    paragraph: &mut Paragraph,
    valid_len: usize,
    fallback_index: usize,
    changed: &mut bool,
) {
    for run in &mut paragraph.runs {
        if run.style.color_index >= valid_len {
            run.style.color_index = fallback_index;
            *changed = true;
        }
        if run
            .style
            .highlight_index
            .is_some_and(|index| index >= valid_len)
        {
            run.style.highlight_index = Some(fallback_index);
            *changed = true;
        }
        if run
            .style
            .underline_color_index
            .is_some_and(|index| index >= valid_len)
        {
            run.style.underline_color_index = Some(fallback_index);
            *changed = true;
        }
    }
}

fn normalize_table_colors(
    table: &mut Table,
    valid_len: usize,
    fallback_index: usize,
    changed: &mut bool,
) {
    for row in &mut table.rows {
        for cell in &mut row.cells {
            normalize_paragraph_colors(&mut cell.paragraphs, valid_len, fallback_index, changed);
            if let Some(nested) = &mut cell.nested_table {
                normalize_table_colors(nested, valid_len, fallback_index, changed);
            }
        }
    }
}

fn normalize_optional_paragraph_font(
    paragraph: &mut Option<Paragraph>,
    known: &HashSet<i32>,
    fallback_index: i32,
    changed: &mut bool,
) {
    if let Some(paragraph) = paragraph {
        normalize_paragraph_font(paragraph, known, fallback_index, changed);
    }
}

fn normalize_paragraph_fonts(
    paragraphs: &mut [Paragraph],
    known: &HashSet<i32>,
    fallback_index: i32,
    changed: &mut bool,
) {
    for paragraph in paragraphs {
        normalize_paragraph_font(paragraph, known, fallback_index, changed);
    }
}

fn normalize_paragraph_font(
    paragraph: &mut Paragraph,
    known: &HashSet<i32>,
    fallback_index: i32,
    changed: &mut bool,
) {
    for run in &mut paragraph.runs {
        if !known.contains(&run.style.font_index) {
            run.style.font_index = fallback_index;
            *changed = true;
        }
    }
}

fn normalize_table_fonts(
    table: &mut Table,
    known: &HashSet<i32>,
    fallback_index: i32,
    changed: &mut bool,
) {
    for row in &mut table.rows {
        for cell in &mut row.cells {
            normalize_paragraph_fonts(&mut cell.paragraphs, known, fallback_index, changed);
            if let Some(nested) = &mut cell.nested_table {
                normalize_table_fonts(nested, known, fallback_index, changed);
            }
        }
    }
}

struct ValidationContext<'a> {
    fonts: &'a HashSet<i32>,
    color_count: usize,
    inline_image_count: usize,
    footnote_count: usize,
    endnote_count: usize,
}

fn validate_parallel_prefix_len(
    vector: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), ModelInvariantError> {
    if actual <= expected {
        Ok(())
    } else {
        Err(ModelInvariantError::ParallelVectorLength {
            vector,
            expected,
            actual,
        })
    }
}

fn validate_one_based_index(
    kind: &'static str,
    index: usize,
    len: usize,
    path: String,
) -> Result<(), ModelInvariantError> {
    if index > 0 && index <= len {
        Ok(())
    } else {
        Err(ModelInvariantError::IndexOutOfBounds {
            kind,
            index,
            len,
            path,
        })
    }
}

fn validate_index(
    kind: &'static str,
    index: usize,
    len: usize,
    path: String,
) -> Result<(), ModelInvariantError> {
    if index < len {
        Ok(())
    } else {
        Err(ModelInvariantError::IndexOutOfBounds {
            kind,
            index,
            len,
            path,
        })
    }
}

fn validate_optional_paragraph(
    paragraph: &Option<Paragraph>,
    path: &str,
    context: &ValidationContext<'_>,
) -> Result<(), ModelInvariantError> {
    if let Some(paragraph) = paragraph {
        validate_paragraph(paragraph, path, context)?;
    }
    Ok(())
}

fn validate_paragraph_list(
    paragraphs: &[Paragraph],
    path: &str,
    context: &ValidationContext<'_>,
) -> Result<(), ModelInvariantError> {
    for (index, paragraph) in paragraphs.iter().enumerate() {
        validate_paragraph(paragraph, &format!("{path}[{index}]"), context)?;
    }
    Ok(())
}

fn validate_shape_list(
    shapes: &[StaticShape],
    path: &str,
    context: &ValidationContext<'_>,
) -> Result<(), ModelInvariantError> {
    for (index, shape) in shapes.iter().enumerate() {
        validate_shape(shape, &format!("{path}[{index}]"), context)?;
    }
    Ok(())
}

fn validate_shape(
    shape: &StaticShape,
    path: &str,
    context: &ValidationContext<'_>,
) -> Result<(), ModelInvariantError> {
    validate_paragraph_list(&shape.text, &format!("{path}.text"), context)
}

fn validate_table(
    table: &Table,
    path: &str,
    depth: usize,
    context: &ValidationContext<'_>,
) -> Result<(), ModelInvariantError> {
    if depth > MAX_TRANSITIONAL_NESTED_TABLE_DEPTH {
        return Err(ModelInvariantError::NestedTableDepthExceeded {
            depth,
            limit: MAX_TRANSITIONAL_NESTED_TABLE_DEPTH,
            path: path.to_owned(),
        });
    }
    for (row_index, row) in table.rows.iter().enumerate() {
        for (cell_index, cell) in row.cells.iter().enumerate() {
            let cell_path = format!("{path}.rows[{row_index}].cells[{cell_index}]");
            validate_paragraph_list(
                &cell.paragraphs,
                &format!("{cell_path}.paragraphs"),
                context,
            )?;
            if let Some(nested) = &cell.nested_table {
                validate_table(
                    nested,
                    &format!("{cell_path}.nested_table"),
                    depth + 1,
                    context,
                )?;
            }
        }
    }
    Ok(())
}

fn validate_paragraph(
    paragraph: &Paragraph,
    path: &str,
    context: &ValidationContext<'_>,
) -> Result<(), ModelInvariantError> {
    let style = &paragraph.style;
    validate_parallel_prefix_len(
        "paragraph tab_stop_leaders",
        style.tab_stops_twips.len(),
        style.tab_stop_leaders.len(),
    )?;
    validate_parallel_prefix_len(
        "paragraph tab_stop_alignments",
        style.tab_stops_twips.len(),
        style.tab_stop_alignments.len(),
    )?;
    for (run_index, run) in paragraph.runs.iter().enumerate() {
        let run_path = format!("{path}.runs[{run_index}]");
        validate_character_style(&run.style, &run_path, context)?;
        if let Some(kind) = run.dynamic_field {
            if !run.text.is_empty() {
                return Err(ModelInvariantError::InlineKindWithText { path: run_path });
            }
            validate_inline_kind(kind, &run_path, context)?;
        }
    }
    Ok(())
}

fn validate_character_style(
    style: &CharacterStyle,
    path: &str,
    context: &ValidationContext<'_>,
) -> Result<(), ModelInvariantError> {
    if !context.fonts.contains(&style.font_index)
        && !(context.fonts.is_empty() && style.font_index == 0)
    {
        return Err(ModelInvariantError::UnknownFontIndex {
            index: style.font_index,
            path: path.to_owned(),
        });
    }
    validate_color_index(
        Some(style.color_index),
        context.color_count,
        "text color",
        path,
    )?;
    validate_color_index(
        style.highlight_index,
        context.color_count,
        "highlight color",
        path,
    )?;
    validate_color_index(
        style.underline_color_index,
        context.color_count,
        "underline color",
        path,
    )?;
    Ok(())
}

fn validate_page_settings(
    page: &PageSettings,
    _path: &str,
    _context: &ValidationContext<'_>,
) -> Result<(), ModelInvariantError> {
    // Authored page-border color indexes deliberately retain safe inert
    // out-of-range values; layout ignores them rather than dereferencing them.
    validate_parallel_prefix_len(
        "page column_widths_twips",
        page.column_count,
        page.column_widths_twips.len(),
    )?;
    validate_parallel_prefix_len(
        "page column_gaps_twips",
        page.column_count,
        page.column_gaps_twips.len(),
    )
}

fn validate_color_index(
    index: Option<usize>,
    len: usize,
    kind: &'static str,
    path: &str,
) -> Result<(), ModelInvariantError> {
    if let Some(index) = index {
        validate_index(kind, index, len, path.to_owned())?;
    }
    Ok(())
}

fn validate_inline_kind(
    kind: InlineKind,
    path: &str,
    context: &ValidationContext<'_>,
) -> Result<(), ModelInvariantError> {
    match kind {
        InlineKind::InlineImage(id) => validate_index(
            "inline image",
            id.get(),
            context.inline_image_count,
            path.to_owned(),
        ),
        InlineKind::FootnoteReference(id) => validate_index(
            "footnote reference",
            id.get(),
            context.footnote_count,
            path.to_owned(),
        ),
        InlineKind::EndnoteReference(id) => validate_index(
            "endnote reference",
            id.get(),
            context.endnote_count,
            path.to_owned(),
        ),
        InlineKind::PendingNoteReference => Err(ModelInvariantError::PendingNoteReference {
            path: path.to_owned(),
        }),
        InlineKind::DynamicField(_)
        | InlineKind::BookmarkPageAnchor(_)
        | InlineKind::BookmarkPageReference(_)
        | InlineKind::LayoutAdvance(_)
        | InlineKind::Math(_)
        | InlineKind::NestedTableAnchor => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_aggregate_note_metadata() {
        let mut document = Document::default();
        document.footnotes.push(Footnote {
            content: Paragraph::default(),
            section_index: SectionIndex::new(2),
            body_block_index: BlockIndex::new(0),
            numbering: None,
        });
        assert!(matches!(
            validate_document_invariants(&document),
            Err(ModelInvariantError::IndexOutOfBounds {
                kind: "footnote section",
                index: 2,
                len: 1,
                ..
            })
        ));
    }

    #[test]
    fn rejects_invalid_inline_image_identifier() {
        let mut document = Document::default();
        let Block::Paragraph(paragraph) = &mut document.blocks[0] else {
            panic!("default block must be a paragraph");
        };
        paragraph.runs.push(Run {
            dynamic_field: Some(InlineKind::InlineImage(InlineImageId::new(3))),
            text: String::new(),
            style: CharacterStyle::default(),
        });
        assert!(matches!(
            validate_document_invariants(&document),
            Err(ModelInvariantError::IndexOutOfBounds {
                kind: "inline image",
                index: 3,
                len: 0,
                ..
            })
        ));
    }

    #[test]
    fn rejects_invalid_font_and_color_indexes() {
        let mut document = Document::default();
        {
            let Block::Paragraph(paragraph) = &mut document.blocks[0] else {
                panic!("default block must be a paragraph");
            };
            paragraph.runs.push(Run {
                dynamic_field: None,
                text: "malformed".to_owned(),
                style: CharacterStyle {
                    font_index: 99,
                    ..CharacterStyle::default()
                },
            });
        }
        assert!(matches!(
            validate_document_invariants(&document),
            Err(ModelInvariantError::UnknownFontIndex { index: 99, .. })
        ));

        let Block::Paragraph(paragraph) = &mut document.blocks[0] else {
            panic!("default block must be a paragraph");
        };
        paragraph.runs[0].style.font_index = 0;
        paragraph.runs[0].style.color_index = 9;
        assert!(matches!(
            validate_document_invariants(&document),
            Err(ModelInvariantError::IndexOutOfBounds {
                kind: "text color",
                index: 9,
                len: 1,
                ..
            })
        ));
    }

    #[test]
    fn rejects_pending_note_reference() {
        let mut document = Document::default();
        let Block::Paragraph(paragraph) = &mut document.blocks[0] else {
            panic!("default block must be a paragraph");
        };
        paragraph.runs.push(Run {
            dynamic_field: Some(InlineKind::PendingNoteReference),
            text: String::new(),
            style: CharacterStyle::default(),
        });
        assert!(matches!(
            validate_document_invariants(&document),
            Err(ModelInvariantError::PendingNoteReference { .. })
        ));
    }

    #[test]
    fn rejects_semantic_inline_node_with_text() {
        let mut document = Document::default();
        let Block::Paragraph(paragraph) = &mut document.blocks[0] else {
            panic!("default block must be a paragraph");
        };
        paragraph.runs.push(Run {
            dynamic_field: Some(InlineKind::DynamicField(DynamicField::TotalPages)),
            text: "not atomic".to_owned(),
            style: CharacterStyle::default(),
        });
        assert!(matches!(
            validate_document_invariants(&document),
            Err(ModelInvariantError::InlineKindWithText { .. })
        ));
    }
}
