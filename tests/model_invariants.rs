use open_rtf_converter::{
    layout::LayoutEngine,
    model::{
        Block, CharacterStyle, Document, FOOTNOTE_REFERENCE_MARKER, FOOTNOTE_REFERENCE_MARKER_END,
        ModelInvariantError, Paragraph, Run, Table, TableCell, TableCellBorders,
        TableCellHorizontalMerge, TableCellPadding, TableCellSpacing, TableCellTextDirection,
        TableCellVerticalAlign, TableCellVerticalMerge, TableRow, TableRowAlignment,
        TableRowWrapMargins, inline_image_marker,
    },
};

fn first_paragraph(document: &mut Document) -> &mut Paragraph {
    let Block::Paragraph(paragraph) = &mut document.blocks[0] else {
        panic!("default document must start with a paragraph");
    };
    paragraph
}

#[test]
fn advanced_layout_rejects_misaligned_parallel_vectors() {
    let mut document = Document::default();
    document.footnotes.push(Paragraph::default());

    assert!(matches!(
        LayoutEngine::try_layout(&document),
        Err(ModelInvariantError::ParallelVectorLength {
            vector: "footnote_section_indices",
            expected: 1,
            actual: 0,
        })
    ));
}

#[test]
fn advanced_layout_rejects_out_of_range_section_and_block_indexes() {
    let mut document = Document::default();
    document.footnotes.push(Paragraph::default());
    document.footnote_section_indices.push(2);
    document.footnote_block_indices.push(0);

    assert!(matches!(
        LayoutEngine::try_layout(&document),
        Err(ModelInvariantError::IndexOutOfBounds {
            kind: "footnote section",
            index: 2,
            len: 1,
            ..
        })
    ));

    document.footnote_section_indices[0] = 1;
    document.footnote_block_indices[0] = document.blocks.len();
    assert!(matches!(
        LayoutEngine::try_layout(&document),
        Err(ModelInvariantError::IndexOutOfBounds {
            kind: "footnote block",
            index: 1,
            len: 1,
            ..
        })
    ));
}

#[test]
fn advanced_layout_rejects_invalid_inline_and_note_identifiers() {
    let mut document = Document::default();
    first_paragraph(&mut document).runs.push(Run {
        text: inline_image_marker(7).expect("bounded marker").to_string(),
        style: CharacterStyle::default(),
    });
    assert!(matches!(
        LayoutEngine::try_layout(&document),
        Err(ModelInvariantError::IndexOutOfBounds {
            kind: "inline image",
            index: 7,
            len: 0,
            ..
        })
    ));

    first_paragraph(&mut document).runs[0].text =
        format!("{FOOTNOTE_REFERENCE_MARKER}4{FOOTNOTE_REFERENCE_MARKER_END}");
    assert!(matches!(
        LayoutEngine::try_layout(&document),
        Err(ModelInvariantError::IndexOutOfBounds {
            kind: "footnote reference",
            index: 4,
            len: 0,
            ..
        })
    ));
}

#[test]
fn advanced_layout_rejects_invalid_style_indexes() {
    let mut document = Document::default();
    first_paragraph(&mut document).runs.push(Run {
        text: "invalid style".to_owned(),
        style: CharacterStyle {
            font_index: 42,
            ..CharacterStyle::default()
        },
    });
    assert!(matches!(
        LayoutEngine::try_layout(&document),
        Err(ModelInvariantError::UnknownFontIndex { index: 42, .. })
    ));

    first_paragraph(&mut document).runs[0].style.font_index = 0;
    first_paragraph(&mut document).runs[0].style.color_index = 9;
    assert!(matches!(
        LayoutEngine::try_layout(&document),
        Err(ModelInvariantError::IndexOutOfBounds {
            kind: "text color",
            index: 9,
            len: 1,
            ..
        })
    ));
}

#[test]
fn advanced_layout_rejects_excessive_nested_table_depth() {
    let mut nested = None;
    for _ in 0..129 {
        nested = Some(Box::new(Table {
            rows: vec![TableRow {
                cells: vec![TableCell {
                    paragraphs: Vec::new(),
                    nested_table: nested,
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: Default::default(),
                    padding: TableCellPadding::default(),
                    spacing: TableCellSpacing::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    text_direction: TableCellTextDirection::default(),
                    vertical_align: TableCellVerticalAlign::default(),
                    horizontal_merge: TableCellHorizontalMerge::default(),
                    vertical_merge: TableCellVerticalMerge::default(),
                }],
                column_widths_twips: Vec::new(),
                height_twips: None,
                left_offset_twips: 0,
                vertical_offset_twips: 0,
                wrap_margins: TableRowWrapMargins::default(),
                cell_gap_twips: 0,
                alignment: TableRowAlignment::default(),
                repeat_header: false,
                keep_together: false,
                keep_with_next: false,
                no_overlap: false,
            }],
            column_widths_twips: Vec::new(),
            borders_visible: false,
            preserve_authored_widths: false,
        }));
    }

    let mut document = Document::default();
    document.blocks = vec![Block::Table(*nested.expect("nested table"))];
    assert!(matches!(
        LayoutEngine::try_layout(&document),
        Err(ModelInvariantError::NestedTableDepthExceeded {
            depth: 129,
            limit: 128,
            ..
        })
    ));
}
