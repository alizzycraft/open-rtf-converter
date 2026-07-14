use unicode_linebreak::{BreakOpportunity, linebreaks};

use crate::fonts::{FontAssetStyle, FontProvider};
use crate::model::{
    Alignment, BOOKMARK_PAGE_ANCHOR_MARKER, BOOKMARK_PAGE_MARKER_END, BOOKMARK_PAGE_REF_MARKER,
    Block, BorderStyle, CharacterStyle, DOCUMENT_CHARS_MARKER, DOCUMENT_CHARS_WITH_SPACES_MARKER,
    DOCUMENT_WORDS_MARKER, Document, EndnotePlacement, FontDef, FontFamilyHint, FontPitch,
    FootnotePlacement, LineNumberRestart, PAGE_NUMBER_MARKER, PageNumberFormat, PageSettings,
    PageVerticalAlignment, Paragraph, ParagraphBorders, ParagraphStyle, Run, SECTION_NUMBER_MARKER,
    SECTION_PAGES_MARKER, ShadingPattern, StaticImage, StaticShape, StaticShapeArrowhead,
    StaticShapeKind, TOTAL_PAGES_MARKER, TabAlignment, TabLeader, Table, TableCell,
    TableCellBorder, TableCellHorizontalMerge, TableCellVerticalAlign, TableCellVerticalMerge,
    TableRow, TableRowAlignment, UnderlineStyle,
};

const TWIPS_PER_POINT: f32 = 20.0;
const SMALL_CAPS_FONT_SCALE: f32 = 0.7;
const MAX_SYNTHETIC_DROP_CAP_FONT_SIZE_HALF_POINTS: i32 = 400;
const AUTO_PARAGRAPH_SPACING_TWIPS: i32 = 240;
const MAX_LAYOUT_COLUMNS: usize = 16;
const PASSIVE_NARROW_FONT_SCALE_PERCENT: i32 = 82;
const PASSIVE_NOTE_LABEL_SHIFT_HALF_POINTS: i32 = 6;
const PASSIVE_NOTE_LABEL_FONT_SCALE_PERCENT: i32 = 65;
const LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER: &str = "888";

#[derive(Debug, Clone)]
pub struct LayoutDocument {
    pub width: f32,
    pub height: f32,
    pub fonts: Vec<FontDef>,
    pub pages: Vec<LayoutPage>,
}

#[derive(Debug, Clone)]
pub struct LayoutPage {
    pub width: f32,
    pub height: f32,
    pub items: Vec<LayoutItem>,
    display_page_number: String,
    section_number: usize,
    geometry: PageGeometry,
}

#[derive(Debug, Clone)]
pub enum LayoutItem {
    Highlight {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: PdfColor,
    },
    Text(TextFragment),
    Underline {
        x: f32,
        y: f32,
        width: f32,
        color: PdfColor,
        style: UnderlineStyle,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        width: f32,
        color: PdfColor,
        style: LineStyle,
    },
    Ellipse {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        stroke_width: f32,
        stroke_color: PdfColor,
        stroke_style: LineStyle,
        fill_color: Option<PdfColor>,
    },
    RoundedRectangle {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
        stroke_width: f32,
        stroke_color: PdfColor,
        stroke_style: LineStyle,
        fill_color: Option<PdfColor>,
    },
    Polygon {
        points: Vec<LayoutPoint>,
        stroke_width: f32,
        stroke_color: PdfColor,
        stroke_style: LineStyle,
        fill_color: Option<PdfColor>,
    },
    Image(ImageFragment),
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct LayoutPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum LineStyle {
    #[default]
    Solid,
    Double,
    Dotted,
    Dashed,
    Wavy,
}

#[derive(Debug, Clone)]
pub struct TextFragment {
    pub text: String,
    pub x: f32,
    pub baseline_y: f32,
    pub color: PdfColor,
    pub font_family: PdfFontFamily,
    pub word_spacing: f32,
    pub style: CharacterStyle,
}

#[derive(Debug, Clone)]
pub struct ImageFragment {
    pub image: StaticImage,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PdfColor {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PdfFontFamily {
    Helvetica,
    Courier,
    Times,
    Symbol,
    ZapfDingbats,
}

impl Default for PdfColor {
    fn default() -> Self {
        Self {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
        }
    }
}

pub struct LayoutEngine;

#[derive(Debug, Copy, Clone)]
struct PageGeometry {
    width: f32,
    height: f32,
    base_margin_left: f32,
    base_margin_right: f32,
    gutter: f32,
    mirror_margins: bool,
    gutter_on_right: bool,
    margin_left: f32,
    margin_top: f32,
    margin_bottom: f32,
    header_distance: f32,
    footer_distance: f32,
    content_width: f32,
    column_count: usize,
    column_gap: f32,
    column_width: f32,
    column_lefts: [f32; MAX_LAYOUT_COLUMNS],
    column_widths: [f32; MAX_LAYOUT_COLUMNS],
    column_gaps: [f32; MAX_LAYOUT_COLUMNS],
    line_between_columns: bool,
    vertical_alignment: PageVerticalAlignment,
    page_borders: ParagraphBorders,
    page_border_spacing: crate::model::PageBorderSpacing,
    page_border_from_page_edge: bool,
    page_border_includes_header: bool,
    page_border_includes_footer: bool,
    line_numbering: crate::model::LineNumbering,
    page_number_x: Option<f32>,
    page_number_y: Option<f32>,
    text_line_grid_twips: Option<i32>,
    numbering: PageNumbering,
    section_number: usize,
    title_page: bool,
    header_footer_index: usize,
}

impl PageGeometry {
    fn from_settings(
        settings: &PageSettings,
        numbering: PageNumbering,
        physical_page_number: usize,
        header_footer_index: usize,
    ) -> Self {
        let width = twips_to_points(settings.width_twips);
        let height = twips_to_points(settings.height_twips);
        let base_margin_left = twips_to_points(settings.margin_left_twips);
        let base_margin_right = twips_to_points(settings.margin_right_twips);
        let gutter = twips_to_points(settings.gutter_twips.max(0));
        let margin_top = twips_to_points(settings.margin_top_twips);
        let margin_bottom = twips_to_points(settings.margin_bottom_twips);
        let header_distance = twips_to_points(settings.header_distance_twips.max(0));
        let footer_distance = twips_to_points(settings.footer_distance_twips.max(0));
        let mirror_margins = settings.mirror_margins;
        let gutter_on_right = settings.gutter_on_right;
        let (margin_left, margin_right) = page_horizontal_margins(
            base_margin_left,
            base_margin_right,
            gutter,
            mirror_margins,
            gutter_on_right,
            physical_page_number,
        );
        let content_width = (width - margin_left - margin_right).max(72.0);
        let column_count = settings.column_count.clamp(1, MAX_LAYOUT_COLUMNS);
        let columns = compute_page_columns(settings, margin_left, content_width, column_count);
        let column_gap = columns.gaps[0];
        let column_width = columns.widths[0];

        Self {
            width,
            height,
            base_margin_left,
            base_margin_right,
            gutter,
            mirror_margins,
            gutter_on_right,
            margin_left,
            margin_top,
            margin_bottom,
            header_distance,
            footer_distance,
            content_width,
            column_count,
            column_gap,
            column_width,
            column_lefts: columns.lefts,
            column_widths: columns.widths,
            column_gaps: columns.gaps,
            line_between_columns: settings.line_between_columns && column_count > 1,
            vertical_alignment: settings.vertical_alignment,
            page_borders: settings.page_borders,
            page_border_spacing: settings.page_border_spacing_twips,
            page_border_from_page_edge: settings.page_border_from_page_edge,
            page_border_includes_header: settings.page_border_includes_header,
            page_border_includes_footer: settings.page_border_includes_footer,
            line_numbering: settings.line_numbering,
            page_number_x: settings.page_number_x_twips.map(twips_to_points),
            page_number_y: settings.page_number_y_twips.map(twips_to_points),
            text_line_grid_twips: settings.text_line_grid_twips,
            numbering,
            section_number: 1,
            title_page: settings.title_page,
            header_footer_index,
        }
    }

    fn body_left(self, column_index: usize) -> f32 {
        self.column_lefts
            .get(column_index)
            .copied()
            .unwrap_or(self.margin_left)
    }

    fn body_width(self, column_index: usize) -> f32 {
        self.column_widths
            .get(column_index)
            .copied()
            .unwrap_or(self.column_width)
    }

    fn for_physical_page(self, physical_page_number: usize) -> Self {
        let (margin_left, margin_right) = page_horizontal_margins(
            self.base_margin_left,
            self.base_margin_right,
            self.gutter,
            self.mirror_margins,
            self.gutter_on_right,
            physical_page_number,
        );
        let content_width = (self.width - margin_left - margin_right).max(72.0);
        let mut columns = compute_equal_page_columns(
            margin_left,
            content_width,
            self.column_count,
            self.column_gap,
        );
        if self.column_count > 1
            && self
                .column_widths
                .iter()
                .take(self.column_count)
                .any(|width| *width > 0.0)
        {
            columns.lefts = self.column_lefts;
            columns.widths = self.column_widths;
            columns.gaps = self.column_gaps;
            let shift = margin_left - self.margin_left;
            for left in columns.lefts.iter_mut().take(self.column_count) {
                *left += shift;
            }
        }
        let column_width = columns.widths[0];

        Self {
            margin_left,
            content_width,
            column_width,
            column_lefts: columns.lefts,
            column_widths: columns.widths,
            column_gaps: columns.gaps,
            ..self
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct PageColumns {
    lefts: [f32; MAX_LAYOUT_COLUMNS],
    widths: [f32; MAX_LAYOUT_COLUMNS],
    gaps: [f32; MAX_LAYOUT_COLUMNS],
}

fn compute_page_columns(
    settings: &PageSettings,
    margin_left: f32,
    content_width: f32,
    column_count: usize,
) -> PageColumns {
    if column_count <= 1
        || settings.column_widths_twips.len() < column_count
        || settings
            .column_widths_twips
            .iter()
            .take(column_count)
            .any(|width| *width <= 0)
    {
        return compute_equal_page_columns(
            margin_left,
            content_width,
            column_count,
            if column_count > 1 {
                twips_to_points(settings.column_gap_twips.max(0))
            } else {
                0.0
            },
        );
    }

    let mut columns = PageColumns {
        lefts: [margin_left; MAX_LAYOUT_COLUMNS],
        widths: [content_width; MAX_LAYOUT_COLUMNS],
        gaps: [0.0; MAX_LAYOUT_COLUMNS],
    };
    for idx in 0..column_count {
        columns.widths[idx] = twips_to_points(settings.column_widths_twips[idx].max(0));
        columns.gaps[idx] = if idx + 1 < column_count {
            twips_to_points(
                settings
                    .column_gaps_twips
                    .get(idx)
                    .copied()
                    .unwrap_or(settings.column_gap_twips)
                    .max(0),
            )
        } else {
            0.0
        };
    }

    let total_width = (0..column_count)
        .map(|idx| columns.widths[idx] + columns.gaps[idx])
        .sum::<f32>()
        .max(1.0);
    if total_width > content_width {
        let scale = content_width / total_width;
        for idx in 0..column_count {
            columns.widths[idx] = (columns.widths[idx] * scale).max(12.0);
            columns.gaps[idx] *= scale;
        }
    }

    let mut left = margin_left;
    for idx in 0..column_count {
        columns.lefts[idx] = left;
        left += columns.widths[idx] + columns.gaps[idx];
    }
    columns
}

fn compute_equal_page_columns(
    margin_left: f32,
    content_width: f32,
    column_count: usize,
    column_gap: f32,
) -> PageColumns {
    let column_count = column_count.clamp(1, MAX_LAYOUT_COLUMNS);
    let gap = if column_count > 1 {
        column_gap.max(0.0)
    } else {
        0.0
    };
    let total_gap = gap * (column_count.saturating_sub(1)) as f32;
    let width = if column_count > 1 {
        ((content_width - total_gap) / column_count as f32).max(36.0)
    } else {
        content_width
    };
    let mut columns = PageColumns {
        lefts: [margin_left; MAX_LAYOUT_COLUMNS],
        widths: [width; MAX_LAYOUT_COLUMNS],
        gaps: [0.0; MAX_LAYOUT_COLUMNS],
    };
    let mut left = margin_left;
    for idx in 0..column_count {
        columns.lefts[idx] = left;
        columns.widths[idx] = width;
        columns.gaps[idx] = if idx + 1 < column_count { gap } else { 0.0 };
        left += width + columns.gaps[idx];
    }
    columns
}

fn page_horizontal_margins(
    base_margin_left: f32,
    base_margin_right: f32,
    gutter: f32,
    mirror_margins: bool,
    gutter_on_right: bool,
    physical_page_number: usize,
) -> (f32, f32) {
    match (
        mirror_margins,
        gutter_on_right,
        physical_page_number % 2 == 0,
    ) {
        (true, true, true) => (base_margin_right + gutter, base_margin_left),
        (true, true, false) => (base_margin_left, base_margin_right + gutter),
        (true, false, true) => (base_margin_right, base_margin_left + gutter),
        (true, false, false) => (base_margin_left + gutter, base_margin_right),
        (false, true, _) => (base_margin_left, base_margin_right + gutter),
        (false, false, _) => (base_margin_left + gutter, base_margin_right),
    }
}

#[derive(Debug, Copy, Clone)]
struct PageNumbering {
    start: usize,
    base_physical_page: usize,
    format: PageNumberFormat,
}

impl PageNumbering {
    fn from_page_settings(settings: &PageSettings, base_physical_page: usize) -> Self {
        Self {
            start: settings.page_number_start.unwrap_or(1).max(1) as usize,
            base_physical_page: base_physical_page.max(1),
            format: settings
                .page_number_format
                .unwrap_or(PageNumberFormat::Decimal),
        }
    }

    fn with_section_settings(self, settings: &PageSettings, base_physical_page: usize) -> Self {
        let current_number = self.display_page_number_value(base_physical_page);
        Self {
            start: settings
                .page_number_start
                .unwrap_or(current_number.min(i32::MAX as usize) as i32)
                .max(1) as usize,
            base_physical_page: base_physical_page.max(1),
            format: settings.page_number_format.unwrap_or(self.format),
        }
    }

    fn display_page_number_value(self, physical_page_number: usize) -> usize {
        self.start
            .saturating_add(physical_page_number.saturating_sub(self.base_physical_page))
    }

    fn display_page_number(self, physical_page_number: usize) -> String {
        format_page_number(
            self.display_page_number_value(physical_page_number),
            self.format,
        )
    }
}

fn new_layout_page(geometry: PageGeometry, physical_page_number: usize) -> LayoutPage {
    LayoutPage {
        width: geometry.width,
        height: geometry.height,
        items: Vec::new(),
        display_page_number: geometry.numbering.display_page_number(physical_page_number),
        section_number: geometry.section_number,
        geometry,
    }
}

#[derive(Debug, Clone)]
struct FlowRun {
    text: String,
    style: CharacterStyle,
    width: f32,
    line_height_points: f32,
    tab_leader: TabLeader,
    tab_alignment: TabAlignment,
    tab_stop_position: Option<f32>,
    soft_hyphen_after: bool,
}

#[derive(Debug, Clone)]
struct MarkerContext {
    page_number: String,
    section_number: String,
    document_words: String,
    document_chars: String,
    document_chars_with_spaces: String,
}

#[derive(Debug, Copy, Clone, Default)]
struct DocumentStats {
    words: usize,
    chars: usize,
    chars_with_spaces: usize,
}

#[derive(Debug, Copy, Clone)]
struct LineNumberState {
    next: i32,
}

impl LineNumberState {
    fn new(settings: &PageSettings) -> Self {
        Self {
            next: settings.line_numbering.start.max(1),
        }
    }

    fn reset_for_geometry(&mut self, geometry: PageGeometry) {
        self.next = geometry.line_numbering.start.max(1);
    }
}

#[derive(Debug, Clone, Default)]
struct HeaderFooterSet {
    header: Vec<Paragraph>,
    first_page_header: Vec<Paragraph>,
    even_page_header: Vec<Paragraph>,
    header_images: Vec<StaticImage>,
    first_page_header_images: Vec<StaticImage>,
    even_page_header_images: Vec<StaticImage>,
    header_shapes: Vec<StaticShape>,
    first_page_header_shapes: Vec<StaticShape>,
    even_page_header_shapes: Vec<StaticShape>,
    footer: Vec<Paragraph>,
    first_page_footer: Vec<Paragraph>,
    even_page_footer: Vec<Paragraph>,
    footer_images: Vec<StaticImage>,
    first_page_footer_images: Vec<StaticImage>,
    even_page_footer_images: Vec<StaticImage>,
    footer_shapes: Vec<StaticShape>,
    first_page_footer_shapes: Vec<StaticShape>,
    even_page_footer_shapes: Vec<StaticShape>,
    background_shapes: Vec<StaticShape>,
}

impl HeaderFooterSet {
    fn from_document(document: &Document) -> Self {
        Self {
            header: document.header.clone(),
            first_page_header: document.first_page_header.clone(),
            even_page_header: document.even_page_header.clone(),
            header_images: document.header_images.clone(),
            first_page_header_images: document.first_page_header_images.clone(),
            even_page_header_images: document.even_page_header_images.clone(),
            header_shapes: document.header_shapes.clone(),
            first_page_header_shapes: document.first_page_header_shapes.clone(),
            even_page_header_shapes: document.even_page_header_shapes.clone(),
            footer: document.footer.clone(),
            first_page_footer: document.first_page_footer.clone(),
            even_page_footer: document.even_page_footer.clone(),
            footer_images: document.footer_images.clone(),
            first_page_footer_images: document.first_page_footer_images.clone(),
            even_page_footer_images: document.even_page_footer_images.clone(),
            footer_shapes: document.footer_shapes.clone(),
            first_page_footer_shapes: document.first_page_footer_shapes.clone(),
            even_page_footer_shapes: document.even_page_footer_shapes.clone(),
            background_shapes: document.background_shapes.clone(),
        }
    }

    fn from_page_settings(settings: &PageSettings) -> Self {
        Self {
            header: settings.header.clone(),
            first_page_header: settings.first_page_header.clone(),
            even_page_header: settings.even_page_header.clone(),
            header_images: settings.header_images.clone(),
            first_page_header_images: settings.first_page_header_images.clone(),
            even_page_header_images: settings.even_page_header_images.clone(),
            header_shapes: settings.header_shapes.clone(),
            first_page_header_shapes: settings.first_page_header_shapes.clone(),
            even_page_header_shapes: settings.even_page_header_shapes.clone(),
            footer: settings.footer.clone(),
            first_page_footer: settings.first_page_footer.clone(),
            even_page_footer: settings.even_page_footer.clone(),
            footer_images: settings.footer_images.clone(),
            first_page_footer_images: settings.first_page_footer_images.clone(),
            even_page_footer_images: settings.even_page_footer_images.clone(),
            footer_shapes: settings.footer_shapes.clone(),
            first_page_footer_shapes: settings.first_page_footer_shapes.clone(),
            even_page_footer_shapes: settings.even_page_footer_shapes.clone(),
            background_shapes: settings.background_shapes.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct Line {
    runs: Vec<FlowRun>,
    width: f32,
    height: f32,
}

#[derive(Debug, Clone)]
struct VisualTableCell {
    cell_index: usize,
    x_offset: f32,
    width: f32,
}

#[derive(Debug, Clone)]
struct PreparedTableRow {
    visual_cells: Vec<VisualTableCell>,
    cell_lines: Vec<Vec<PreparedCellLine>>,
    cell_paddings: Vec<ResolvedCellPadding>,
    cell_spacings: Vec<ResolvedCellSpacing>,
    row_height: f32,
}

#[derive(Debug, Clone)]
struct PreparedCellLine {
    line: Line,
    style: ParagraphStyle,
    is_first_line: bool,
    is_last_line: bool,
    space_before: f32,
    space_after: f32,
}

#[derive(Debug, Copy, Clone)]
struct ResolvedCellPadding {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

#[derive(Debug, Copy, Clone)]
struct ResolvedCellSpacing {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

impl LayoutEngine {
    pub fn layout(document: &Document) -> LayoutDocument {
        Self::layout_with_font_provider(document, None)
    }

    pub fn layout_with_font_provider(
        document: &Document,
        font_provider: Option<&FontProvider>,
    ) -> LayoutDocument {
        let document_stats = document_stats(document);
        let mut header_footer_sets = vec![HeaderFooterSet::from_document(document)];
        let mut geometry = PageGeometry::from_settings(
            &document.page,
            PageNumbering::from_page_settings(&document.page, 1),
            1,
            0,
        );

        let mut pages = vec![new_layout_page(geometry, 1)];
        let mut cursor_y = geometry.height - geometry.margin_top;
        let mut current_column = 0usize;
        let mut section_number = 1usize;
        let mut line_number_state = LineNumberState::new(&document.page);
        let mut rendered_endnotes = vec![false; document.endnotes.len()];

        for (block_idx, block) in document.blocks.iter().enumerate() {
            match block {
                Block::PageBreak => {
                    let previous_page_count = pages.len();
                    start_new_page(
                        &mut pages,
                        &mut cursor_y,
                        &mut geometry,
                        &mut current_column,
                    );
                    reset_line_numbers_for_page_restart(
                        Some(&mut line_number_state),
                        geometry,
                        previous_page_count,
                        pages.len(),
                    );
                }
                Block::ContinuousSectionBreak => {
                    layout_endnotes_for_section(
                        &mut pages,
                        &mut cursor_y,
                        document,
                        &mut rendered_endnotes,
                        section_number,
                        geometry.content_width,
                        geometry.margin_left,
                        geometry.margin_bottom,
                        &mut geometry,
                        document_stats,
                        font_provider,
                    );
                    section_number = section_number.saturating_add(1);
                    geometry.section_number = section_number;
                    if !next_block_is_section_settings(document, block_idx) {
                        reset_line_numbers_for_section_restart(
                            Some(&mut line_number_state),
                            geometry,
                        );
                    }
                    if let Some(page) = pages.last_mut() {
                        page.section_number = section_number;
                        page.geometry.section_number = section_number;
                    }
                }
                Block::SectionBreak => {
                    layout_endnotes_for_section(
                        &mut pages,
                        &mut cursor_y,
                        document,
                        &mut rendered_endnotes,
                        section_number,
                        geometry.content_width,
                        geometry.margin_left,
                        geometry.margin_bottom,
                        &mut geometry,
                        document_stats,
                        font_provider,
                    );
                    section_number = section_number.saturating_add(1);
                    geometry.section_number = section_number;
                    if !next_block_is_section_settings(document, block_idx) {
                        reset_line_numbers_for_section_restart(
                            Some(&mut line_number_state),
                            geometry,
                        );
                    }
                    start_new_page(
                        &mut pages,
                        &mut cursor_y,
                        &mut geometry,
                        &mut current_column,
                    );
                }
                Block::EvenPageSectionBreak => {
                    layout_endnotes_for_section(
                        &mut pages,
                        &mut cursor_y,
                        document,
                        &mut rendered_endnotes,
                        section_number,
                        geometry.content_width,
                        geometry.margin_left,
                        geometry.margin_bottom,
                        &mut geometry,
                        document_stats,
                        font_provider,
                    );
                    section_number = section_number.saturating_add(1);
                    geometry.section_number = section_number;
                    if !next_block_is_section_settings(document, block_idx) {
                        reset_line_numbers_for_section_restart(
                            Some(&mut line_number_state),
                            geometry,
                        );
                    }
                    start_new_page_with_parity(
                        &mut pages,
                        &mut cursor_y,
                        &mut geometry,
                        &mut current_column,
                        PageParity::Even,
                    );
                }
                Block::OddPageSectionBreak => {
                    layout_endnotes_for_section(
                        &mut pages,
                        &mut cursor_y,
                        document,
                        &mut rendered_endnotes,
                        section_number,
                        geometry.content_width,
                        geometry.margin_left,
                        geometry.margin_bottom,
                        &mut geometry,
                        document_stats,
                        font_provider,
                    );
                    section_number = section_number.saturating_add(1);
                    geometry.section_number = section_number;
                    if !next_block_is_section_settings(document, block_idx) {
                        reset_line_numbers_for_section_restart(
                            Some(&mut line_number_state),
                            geometry,
                        );
                    }
                    start_new_page_with_parity(
                        &mut pages,
                        &mut cursor_y,
                        &mut geometry,
                        &mut current_column,
                        PageParity::Odd,
                    );
                }
                Block::ColumnBreak => {
                    let previous_page_count = pages.len();
                    advance_column_or_page(
                        &mut pages,
                        &mut cursor_y,
                        &mut geometry,
                        &mut current_column,
                    );
                    reset_line_numbers_for_page_restart(
                        Some(&mut line_number_state),
                        geometry,
                        previous_page_count,
                        pages.len(),
                    );
                }
                Block::SectionSettings(settings) => {
                    let numbering = geometry
                        .numbering
                        .with_section_settings(settings, pages.len());
                    let physical_page_number = pages.len();
                    let header_footer_index = header_footer_sets.len();
                    header_footer_sets.push(HeaderFooterSet::from_page_settings(settings));
                    geometry = PageGeometry::from_settings(
                        settings,
                        numbering,
                        physical_page_number,
                        header_footer_index,
                    );
                    geometry.section_number = section_number;
                    reset_line_numbers_for_section_restart(Some(&mut line_number_state), geometry);
                    if let Some(page) = pages.last_mut()
                        && page.items.is_empty()
                    {
                        page.width = geometry.width;
                        page.height = geometry.height;
                        page.display_page_number =
                            geometry.numbering.display_page_number(physical_page_number);
                        page.section_number = geometry.section_number;
                        page.geometry = geometry;
                        cursor_y = geometry.height - geometry.margin_top;
                        current_column = 0;
                    }
                }
                Block::Placeholder(text) => {
                    let markers = current_marker_context(&pages, document_stats);
                    let paragraph = Paragraph {
                        style: ParagraphStyle::default(),
                        runs: vec![Run {
                            text: text.clone(),
                            style: CharacterStyle::default(),
                        }],
                    };
                    layout_paragraph(
                        &mut pages,
                        &mut cursor_y,
                        &paragraph,
                        false,
                        false,
                        false,
                        geometry.body_width(current_column),
                        geometry.margin_bottom,
                        &mut geometry,
                        &mut current_column,
                        document,
                        document_stats,
                        Some(&mut line_number_state),
                        &markers,
                        font_provider,
                    );
                }
                Block::Table(table) => {
                    layout_table(
                        &mut pages,
                        &mut cursor_y,
                        table,
                        geometry.body_width(current_column),
                        geometry.body_left(current_column),
                        geometry.margin_bottom,
                        &mut geometry,
                        &mut current_column,
                        document,
                        document_stats,
                        font_provider,
                    );
                }
                Block::Image(image) => {
                    layout_image(
                        &mut pages,
                        &mut cursor_y,
                        image,
                        geometry.body_width(current_column),
                        geometry.body_left(current_column),
                        geometry.margin_bottom,
                        &mut geometry,
                        &mut current_column,
                    );
                }
                Block::Shape(shape) => {
                    layout_shape(
                        &mut pages,
                        &mut cursor_y,
                        shape,
                        geometry.body_width(current_column),
                        geometry.body_left(current_column),
                        geometry.margin_bottom,
                        &mut geometry,
                        &mut current_column,
                        document,
                        document_stats,
                        font_provider,
                    );
                }
                Block::Paragraph(paragraph) => {
                    let markers = current_marker_context(&pages, document_stats);
                    let suppress_contextual_before =
                        previous_paragraph_block(&document.blocks, block_idx).is_some_and(
                            |previous| paragraph_spacing_is_contextual(previous, paragraph),
                        );
                    let suppress_contextual_after =
                        next_paragraph_block(&document.blocks, block_idx)
                            .is_some_and(|next| paragraph_spacing_is_contextual(paragraph, next));
                    let render_between_border_after =
                        next_paragraph_block(&document.blocks, block_idx).is_some_and(|next| {
                            paragraph_between_border_is_visible(paragraph, next)
                        });
                    if paragraph.style.keep_with_next
                        && let Some(next_paragraph) =
                            next_paragraph_block(&document.blocks, block_idx)
                    {
                        let previous_page_count = pages.len();
                        start_new_page_for_kept_paragraphs(
                            &mut pages,
                            &mut cursor_y,
                            paragraph,
                            Some(next_paragraph),
                            geometry.body_width(current_column),
                            geometry.margin_bottom,
                            &mut geometry,
                            &mut current_column,
                            document,
                            &markers,
                            font_provider,
                        );
                        reset_line_numbers_for_page_restart(
                            Some(&mut line_number_state),
                            geometry,
                            previous_page_count,
                            pages.len(),
                        );
                    }
                    layout_paragraph(
                        &mut pages,
                        &mut cursor_y,
                        paragraph,
                        suppress_contextual_before,
                        suppress_contextual_after,
                        render_between_border_after,
                        geometry.body_width(current_column),
                        geometry.margin_bottom,
                        &mut geometry,
                        &mut current_column,
                        document,
                        document_stats,
                        Some(&mut line_number_state),
                        &markers,
                        font_provider,
                    );
                }
            }
        }

        layout_footnotes(
            &mut pages,
            &mut cursor_y,
            &document.footnotes,
            document.footnote_number_start,
            document.footnote_number_format,
            0,
            document.footnote_placement,
            geometry.content_width,
            geometry.margin_left,
            geometry.margin_bottom,
            &mut geometry,
            document,
            document_stats,
            font_provider,
        );
        layout_remaining_section_endnotes(
            &mut pages,
            &mut cursor_y,
            document,
            &mut rendered_endnotes,
            geometry.content_width,
            geometry.margin_left,
            geometry.margin_bottom,
            &mut geometry,
            document_stats,
            font_provider,
        );
        layout_endnotes_for_placement(
            &mut pages,
            &mut cursor_y,
            document,
            &mut rendered_endnotes,
            EndnotePlacement::AfterBody,
            false,
            geometry.content_width,
            geometry.margin_left,
            geometry.margin_bottom,
            &mut geometry,
            document_stats,
            font_provider,
        );
        layout_endnotes_for_placement(
            &mut pages,
            &mut cursor_y,
            document,
            &mut rendered_endnotes,
            EndnotePlacement::EndOfDocument,
            true,
            geometry.content_width,
            geometry.margin_left,
            geometry.margin_bottom,
            &mut geometry,
            document_stats,
            font_provider,
        );

        apply_page_vertical_alignment(&mut pages);
        layout_column_separators(&mut pages);
        layout_page_borders(&mut pages, document);
        layout_repeating_header_footer(
            &mut pages,
            document,
            &header_footer_sets,
            true,
            document_stats,
            font_provider,
        );
        layout_repeating_header_footer(
            &mut pages,
            document,
            &header_footer_sets,
            false,
            document_stats,
            font_provider,
        );
        layout_background_shapes(
            &mut pages,
            &header_footer_sets,
            document,
            document_stats,
            font_provider,
        );
        resolve_bookmark_page_ref_markers(&mut pages, document, font_provider);
        resolve_section_page_markers(&mut pages);
        resolve_total_page_markers(&mut pages);

        let width = pages
            .first()
            .map(|page| page.width)
            .unwrap_or(geometry.width);
        let height = pages
            .first()
            .map(|page| page.height)
            .unwrap_or(geometry.height);

        LayoutDocument {
            width,
            height,
            fonts: document.fonts.clone(),
            pages,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn layout_footnotes(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    footnotes: &[Paragraph],
    number_start: i32,
    number_format: PageNumberFormat,
    number_offset: usize,
    placement: FootnotePlacement,
    content_width: f32,
    mut margin_left: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    document: &Document,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) {
    if footnotes.is_empty() {
        return;
    }

    if placement == FootnotePlacement::BottomOfPage {
        position_cursor_for_bottom_notes(
            pages,
            cursor_y,
            footnotes,
            number_start,
            number_format,
            number_offset,
            content_width,
            margin_bottom,
            geometry,
            document,
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
    let Some(page) = pages.last_mut() else {
        return;
    };
    page.items.push(LayoutItem::Line {
        x1: margin_left,
        y1: *cursor_y - 3.0,
        x2: margin_left + content_width.min(144.0),
        y2: *cursor_y - 3.0,
        width: 0.5,
        color: PdfColor {
            red: 0.35,
            green: 0.35,
            blue: 0.35,
        },
        style: LineStyle::Solid,
    });
    *cursor_y -= 9.0;

    for (idx, footnote) in footnotes.iter().enumerate() {
        let paragraph = note_display_paragraph(
            footnote,
            number_start,
            number_offset + idx + 1,
            number_format,
        );
        let markers = current_marker_context(pages, document_stats);
        let mut footnote_column = 0;
        layout_paragraph(
            pages,
            cursor_y,
            &paragraph,
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
            &markers,
            font_provider,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn position_cursor_for_bottom_notes(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    footnotes: &[Paragraph],
    number_start: i32,
    number_format: PageNumberFormat,
    number_offset: usize,
    content_width: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    document: &Document,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) {
    let note_height = note_block_height(
        footnotes,
        number_start,
        number_format,
        number_offset,
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

fn note_block_height(
    footnotes: &[Paragraph],
    number_start: i32,
    number_format: PageNumberFormat,
    number_offset: usize,
    content_width: f32,
    document: &Document,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) -> f32 {
    let mut height = 9.0;
    let markers = marker_context("1".to_string(), "1".to_string(), document_stats);
    for (idx, footnote) in footnotes.iter().enumerate() {
        let paragraph = note_display_paragraph(
            footnote,
            number_start,
            number_offset + idx + 1,
            number_format,
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

fn note_display_paragraph(
    footnote: &Paragraph,
    number_start: i32,
    sequence: usize,
    number_format: PageNumberFormat,
) -> Paragraph {
    let mut paragraph = footnote.clone();
    if let Some(first_run) = paragraph.runs.first_mut() {
        let label = format_note_number(number_start, sequence, number_format);
        let mut label_style = first_run.style.clone();
        label_style.baseline_shift_half_points = PASSIVE_NOTE_LABEL_SHIFT_HALF_POINTS;
        label_style.font_size_scale_percent = PASSIVE_NOTE_LABEL_FONT_SCALE_PERCENT;
        label_style.font_size_half_points = label_style.font_size_half_points.min(20).max(2);
        first_run.text = format!(". {}", first_run.text);
        first_run.style.font_size_half_points =
            first_run.style.font_size_half_points.min(20).max(2);
        paragraph.runs.insert(
            0,
            Run {
                text: label,
                style: label_style,
            },
        );
    }
    paragraph.style.space_before_twips = paragraph.style.space_before_twips.min(60);
    paragraph.style.space_after_twips = paragraph.style.space_after_twips.min(60);
    paragraph
}

#[allow(clippy::too_many_arguments)]
fn layout_endnote_entries(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    endnotes: &[(usize, Paragraph)],
    number_start: i32,
    number_format: PageNumberFormat,
    content_width: f32,
    margin_left: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    document: &Document,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) {
    if endnotes.is_empty() {
        return;
    }
    if *cursor_y - 18.0 < margin_bottom {
        let mut column = 0;
        start_new_page(pages, cursor_y, geometry, &mut column);
    }
    let Some(page) = pages.last_mut() else {
        return;
    };
    page.items.push(LayoutItem::Line {
        x1: margin_left,
        y1: *cursor_y - 3.0,
        x2: margin_left + content_width.min(144.0),
        y2: *cursor_y - 3.0,
        width: 0.5,
        color: PdfColor {
            red: 0.35,
            green: 0.35,
            blue: 0.35,
        },
        style: LineStyle::Solid,
    });
    *cursor_y -= 9.0;

    for (idx, endnote) in endnotes {
        let paragraph =
            note_display_paragraph(endnote, number_start, idx.saturating_add(1), number_format);
        let markers = current_marker_context(pages, document_stats);
        let mut endnote_column = 0;
        layout_paragraph(
            pages,
            cursor_y,
            &paragraph,
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
            &markers,
            font_provider,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn layout_endnotes_for_placement(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    document: &Document,
    rendered_endnotes: &mut [bool],
    placement: EndnotePlacement,
    force_new_page: bool,
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
        .filter(|(idx, _)| !rendered_endnotes.get(*idx).copied().unwrap_or(false))
        .filter(|(idx, _)| endnote_placement_for(document, *idx) == placement)
        .map(|(idx, note)| (idx, note.clone()))
        .collect::<Vec<_>>();
    if entries.is_empty() {
        return;
    }
    if force_new_page && pages.last().is_some_and(|page| !page.items.is_empty()) {
        let mut endnote_column = 0;
        start_new_page(pages, cursor_y, geometry, &mut endnote_column);
    }
    layout_endnote_entries(
        pages,
        cursor_y,
        &entries,
        document.endnote_number_start,
        document.endnote_number_format,
        content_width,
        margin_left,
        margin_bottom,
        geometry,
        document,
        document_stats,
        font_provider,
    );
    for (idx, _) in entries {
        if let Some(rendered) = rendered_endnotes.get_mut(idx) {
            *rendered = true;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn layout_endnotes_for_section(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    document: &Document,
    rendered_endnotes: &mut [bool],
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
        if rendered_endnotes.get(idx).copied().unwrap_or(false) {
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
        document.endnote_number_start,
        document.endnote_number_format,
        content_width,
        margin_left,
        margin_bottom,
        geometry,
        document,
        document_stats,
        font_provider,
    );

    for (idx, _) in section_notes {
        if let Some(rendered) = rendered_endnotes.get_mut(idx) {
            *rendered = true;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn layout_remaining_section_endnotes(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    document: &Document,
    rendered_endnotes: &mut [bool],
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
        .filter(|(idx, _)| !rendered_endnotes.get(*idx).copied().unwrap_or(false))
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
            rendered_endnotes,
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

fn endnote_placement_for(document: &Document, index: usize) -> EndnotePlacement {
    document
        .endnote_placements
        .get(index)
        .copied()
        .unwrap_or(document.endnote_placement)
}

#[allow(clippy::too_many_arguments)]
fn layout_image(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    image: &StaticImage,
    content_width: f32,
    mut margin_left: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    current_column: &mut usize,
) {
    let (left_offset, top_offset, mut width, mut height) = image_layout_frame(image, content_width);
    let block_height = top_offset + height;

    if *cursor_y - block_height < margin_bottom {
        advance_column_or_page(pages, cursor_y, geometry, current_column);
        margin_left = geometry.body_left(*current_column);
        (_, _, width, height) = image_layout_frame(image, content_width);
    }

    let x = margin_left + left_offset;
    let y = *cursor_y - top_offset - height;
    let Some(page) = pages.last_mut() else {
        return;
    };
    page.items.push(LayoutItem::Image(ImageFragment {
        image: image.clone(),
        x,
        y,
        width,
        height,
    }));
    if image.placement.is_some_and(|placement| placement.text_wrap) {
        return;
    }
    *cursor_y = y - 6.0;
}

fn image_layout_frame(image: &StaticImage, content_width: f32) -> (f32, f32, f32, f32) {
    if let Some(placement) = image.placement {
        let left = twips_to_points(placement.left_twips.max(0));
        let top = twips_to_points(placement.top_twips.max(0));
        let mut width = twips_to_points(placement.width_twips.max(1));
        let mut height = twips_to_points(placement.height_twips.max(1));
        let max_width = (content_width - left).max(1.0);
        if width > max_width {
            let scale = max_width / width;
            width *= scale;
            height *= scale;
        }
        (left, top, width.max(1.0), height.max(1.0))
    } else {
        let (width, height) = image_display_size(image, content_width);
        (0.0, 0.0, width, height)
    }
}

fn image_display_size(image: &StaticImage, content_width: f32) -> (f32, f32) {
    let natural_width_px = image.natural_width_px_hint.unwrap_or(image.width_px).max(1);
    let natural_height_px = image
        .natural_height_px_hint
        .unwrap_or(image.height_px)
        .max(1);
    let natural_width = natural_width_px as f32 * 0.75;
    let natural_height = natural_height_px as f32 * 0.75;
    let mut width = image
        .display_width_twips
        .map(twips_to_points)
        .unwrap_or(natural_width)
        .max(1.0);
    let mut height = image
        .display_height_twips
        .map(twips_to_points)
        .unwrap_or(natural_height)
        .max(1.0);
    width *= image.scale_x_percent.unwrap_or(100) as f32 / 100.0;
    height *= image.scale_y_percent.unwrap_or(100) as f32 / 100.0;
    width = width.max(1.0);
    height = height.max(1.0);

    if width > content_width {
        let scale = content_width / width;
        width *= scale;
        height *= scale;
    }

    (width, height)
}

#[allow(clippy::too_many_arguments)]
fn layout_shape(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    shape: &StaticShape,
    content_width: f32,
    mut margin_left: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    current_column: &mut usize,
    document: &Document,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) {
    let left = twips_to_points(shape.left_twips.max(0));
    let top = twips_to_points(shape.top_twips.max(0));
    let mut width = twips_to_points(shape.width_twips.max(1));
    let mut height = twips_to_points(shape.height_twips.max(1));
    let max_width = (content_width - left).max(1.0);
    if width > max_width {
        let scale = max_width / width;
        width *= scale;
        height *= scale;
    }
    let block_height = top + height;
    if *cursor_y - block_height < margin_bottom {
        advance_column_or_page(pages, cursor_y, geometry, current_column);
        margin_left = geometry.body_left(*current_column);
    }

    let x = margin_left + left;
    let top_y = *cursor_y - top;
    let bottom_y = top_y - height;
    let right_x = x + width;
    let stroke_width_points = if shape.stroke_width_twips > 0 {
        Some(twips_to_points(shape.stroke_width_twips).clamp(0.25, 12.0))
    } else {
        None
    };
    let color = PdfColor {
        red: shape.stroke_color.red as f32 / 255.0,
        green: shape.stroke_color.green as f32 / 255.0,
        blue: shape.stroke_color.blue as f32 / 255.0,
    };
    let stroke_style = line_style_for_border_style(shape.stroke_style);
    let Some(page) = pages.last_mut() else {
        return;
    };
    match shape.kind {
        StaticShapeKind::Line => {
            if let Some(width_points) = stroke_width_points {
                let start = LayoutPoint {
                    x: shape_point_x(x, width, 0.0, shape.flip_horizontal),
                    y: shape_point_y(top_y, height, 0.0, shape.flip_vertical),
                };
                let end = LayoutPoint {
                    x: shape_point_x(x, width, width, shape.flip_horizontal),
                    y: shape_point_y(top_y, height, height, shape.flip_vertical),
                };
                page.items.push(LayoutItem::Line {
                    x1: start.x,
                    y1: start.y,
                    x2: end.x,
                    y2: end.y,
                    width: width_points,
                    color,
                    style: stroke_style,
                });
                push_static_shape_arrowhead(
                    page,
                    shape.start_arrowhead,
                    start,
                    end,
                    width_points,
                    color,
                );
                push_static_shape_arrowhead(
                    page,
                    shape.end_arrowhead,
                    end,
                    start,
                    width_points,
                    color,
                );
            }
        }
        StaticShapeKind::Polyline => {
            let natural_width = twips_to_points(shape.width_twips.max(1)).max(1.0);
            let natural_height = twips_to_points(shape.height_twips.max(1)).max(1.0);
            let scale_x = width / natural_width;
            let scale_y = height / natural_height;
            if let Some(width_points) = stroke_width_points {
                let points = shape
                    .points
                    .iter()
                    .map(|point| LayoutPoint {
                        x: shape_point_x(
                            x,
                            width,
                            twips_to_points(point.x_twips) * scale_x,
                            shape.flip_horizontal,
                        ),
                        y: shape_point_y(
                            top_y,
                            height,
                            twips_to_points(point.y_twips) * scale_y,
                            shape.flip_vertical,
                        ),
                    })
                    .collect::<Vec<_>>();
                for segment in points.windows(2) {
                    let start = segment[0];
                    let end = segment[1];
                    page.items.push(LayoutItem::Line {
                        x1: start.x,
                        y1: start.y,
                        x2: end.x,
                        y2: end.y,
                        width: width_points,
                        color,
                        style: stroke_style,
                    });
                }
                if let [first, second, ..] = points.as_slice() {
                    push_static_shape_arrowhead(
                        page,
                        shape.start_arrowhead,
                        *first,
                        *second,
                        width_points,
                        color,
                    );
                }
                if points.len() >= 2 {
                    let last = points[points.len() - 1];
                    let previous = points[points.len() - 2];
                    push_static_shape_arrowhead(
                        page,
                        shape.end_arrowhead,
                        last,
                        previous,
                        width_points,
                        color,
                    );
                }
            }
        }
        StaticShapeKind::Polygon => {
            if stroke_width_points.is_none() && shape.fill_color.is_none() {
                *cursor_y -= block_height + 6.0;
                return;
            }
            let natural_width = twips_to_points(shape.width_twips.max(1)).max(1.0);
            let natural_height = twips_to_points(shape.height_twips.max(1)).max(1.0);
            let scale_x = width / natural_width;
            let scale_y = height / natural_height;
            let points = shape
                .points
                .iter()
                .map(|point| LayoutPoint {
                    x: shape_point_x(
                        x,
                        width,
                        twips_to_points(point.x_twips) * scale_x,
                        shape.flip_horizontal,
                    ),
                    y: shape_point_y(
                        top_y,
                        height,
                        twips_to_points(point.y_twips) * scale_y,
                        shape.flip_vertical,
                    ),
                })
                .collect::<Vec<_>>();
            page.items.push(LayoutItem::Polygon {
                points,
                stroke_width: stroke_width_points.unwrap_or(0.0),
                stroke_color: color,
                stroke_style,
                fill_color: shape.fill_color.map(|fill_color| PdfColor {
                    red: fill_color.red as f32 / 255.0,
                    green: fill_color.green as f32 / 255.0,
                    blue: fill_color.blue as f32 / 255.0,
                }),
            });
        }
        StaticShapeKind::Rectangle => {
            if let Some(fill_color) = shape.fill_color {
                page.items.push(LayoutItem::Highlight {
                    x,
                    y: bottom_y,
                    width,
                    height,
                    color: PdfColor {
                        red: fill_color.red as f32 / 255.0,
                        green: fill_color.green as f32 / 255.0,
                        blue: fill_color.blue as f32 / 255.0,
                    },
                });
            }
            if let Some(width_points) = stroke_width_points {
                page.items.push(LayoutItem::Line {
                    x1: x,
                    y1: top_y,
                    x2: right_x,
                    y2: top_y,
                    width: width_points,
                    color,
                    style: stroke_style,
                });
                page.items.push(LayoutItem::Line {
                    x1: right_x,
                    y1: top_y,
                    x2: right_x,
                    y2: bottom_y,
                    width: width_points,
                    color,
                    style: stroke_style,
                });
                page.items.push(LayoutItem::Line {
                    x1: right_x,
                    y1: bottom_y,
                    x2: x,
                    y2: bottom_y,
                    width: width_points,
                    color,
                    style: stroke_style,
                });
                page.items.push(LayoutItem::Line {
                    x1: x,
                    y1: bottom_y,
                    x2: x,
                    y2: top_y,
                    width: width_points,
                    color,
                    style: stroke_style,
                });
            }
        }
        StaticShapeKind::RoundedRectangle => {
            if stroke_width_points.is_none() && shape.fill_color.is_none() {
                *cursor_y -= block_height + 6.0;
                return;
            }
            let min_dimension = width.min(height).max(1.0);
            page.items.push(LayoutItem::RoundedRectangle {
                x,
                y: bottom_y,
                width,
                height,
                radius: (min_dimension * 0.2).clamp(1.0, min_dimension / 2.0),
                stroke_width: stroke_width_points.unwrap_or(0.0),
                stroke_color: color,
                stroke_style,
                fill_color: shape.fill_color.map(|fill_color| PdfColor {
                    red: fill_color.red as f32 / 255.0,
                    green: fill_color.green as f32 / 255.0,
                    blue: fill_color.blue as f32 / 255.0,
                }),
            });
        }
        StaticShapeKind::Ellipse => {
            if stroke_width_points.is_some() || shape.fill_color.is_some() {
                page.items.push(LayoutItem::Ellipse {
                    x,
                    y: bottom_y,
                    width,
                    height,
                    stroke_width: stroke_width_points.unwrap_or(0.0),
                    stroke_color: color,
                    stroke_style,
                    fill_color: shape.fill_color.map(|fill_color| PdfColor {
                        red: fill_color.red as f32 / 255.0,
                        green: fill_color.green as f32 / 255.0,
                        blue: fill_color.blue as f32 / 255.0,
                    }),
                });
            }
        }
    }
    layout_shape_text(
        pages,
        shape,
        x,
        top_y,
        bottom_y,
        width,
        document,
        document_stats,
        font_provider,
    );
    *cursor_y -= block_height + 6.0;
}

fn shape_point_x(left: f32, width: f32, offset_x: f32, flip_horizontal: bool) -> f32 {
    if flip_horizontal {
        left + width - offset_x
    } else {
        left + offset_x
    }
}

fn shape_point_y(top: f32, height: f32, offset_y: f32, flip_vertical: bool) -> f32 {
    if flip_vertical {
        top - height + offset_y
    } else {
        top - offset_y
    }
}

fn push_static_shape_arrowhead(
    page: &mut LayoutPage,
    arrowhead: StaticShapeArrowhead,
    tip: LayoutPoint,
    tail: LayoutPoint,
    stroke_width: f32,
    color: PdfColor,
) {
    if arrowhead == StaticShapeArrowhead::None {
        return;
    }
    let dx = tip.x - tail.x;
    let dy = tip.y - tail.y;
    let length = (dx.mul_add(dx, dy * dy)).sqrt();
    if length <= 0.01 {
        return;
    }

    let unit_x = dx / length;
    let unit_y = dy / length;
    let perpendicular_x = -unit_y;
    let perpendicular_y = unit_x;
    let arrow_length = (stroke_width * 5.0 + 6.0)
        .clamp(6.0, 18.0)
        .min(length * 0.6);
    let half_width = arrow_length * 0.45;
    let base = LayoutPoint {
        x: tip.x - unit_x * arrow_length,
        y: tip.y - unit_y * arrow_length,
    };
    let wing_a = LayoutPoint {
        x: base.x + perpendicular_x * half_width,
        y: base.y + perpendicular_y * half_width,
    };
    let wing_b = LayoutPoint {
        x: base.x - perpendicular_x * half_width,
        y: base.y - perpendicular_y * half_width,
    };

    match arrowhead {
        StaticShapeArrowhead::None => {}
        StaticShapeArrowhead::Open => {
            page.items.push(LayoutItem::Line {
                x1: tip.x,
                y1: tip.y,
                x2: wing_a.x,
                y2: wing_a.y,
                width: stroke_width,
                color,
                style: LineStyle::Solid,
            });
            page.items.push(LayoutItem::Line {
                x1: tip.x,
                y1: tip.y,
                x2: wing_b.x,
                y2: wing_b.y,
                width: stroke_width,
                color,
                style: LineStyle::Solid,
            });
        }
        StaticShapeArrowhead::Triangle => {
            page.items.push(LayoutItem::Polygon {
                points: vec![tip, wing_a, wing_b],
                stroke_width,
                stroke_color: color,
                stroke_style: LineStyle::Solid,
                fill_color: Some(color),
            });
        }
    }
}

fn layout_shape_text(
    pages: &mut Vec<LayoutPage>,
    shape: &StaticShape,
    x: f32,
    top_y: f32,
    bottom_y: f32,
    width: f32,
    document: &Document,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
) {
    if shape.text.is_empty() {
        return;
    }
    let padding = 4.0;
    let content_width = (width - padding * 2.0).max(1.0);
    let mut cursor_y = top_y - padding;
    let min_y = bottom_y + padding;
    let markers = current_marker_context(pages, document_stats);

    for paragraph in &shape.text {
        cursor_y -= twips_to_points(effective_space_before_twips(&paragraph.style));
        let mut lines = wrap_paragraph_with_font_provider(
            paragraph,
            content_width,
            &markers,
            document,
            font_provider,
        );
        for line in &mut lines {
            line.height = apply_line_spacing(line.height, &paragraph.style);
        }
        let line_count = lines.len();
        for (line_idx, line) in lines.iter().enumerate() {
            if cursor_y - line.height < min_y {
                return;
            }
            let is_first_line = line_idx == 0;
            let is_last_line = line_idx + 1 == line_count;
            if let Some(color_index) = paragraph.style.shading_color_index
                && color_index > 0
            {
                let line_left_indent = twips_to_points(paragraph_line_left_indent_twips(
                    &paragraph.style,
                    is_first_line,
                ));
                push_shading_rect(
                    pages,
                    document,
                    x + padding + line_left_indent,
                    cursor_y - line.height,
                    paragraph_line_width(content_width, &paragraph.style, is_first_line),
                    line.height,
                    color_index,
                    paragraph.style.shading_basis_points,
                    paragraph.style.shading_pattern,
                );
            }
            let text_x = aligned_x(
                x + padding,
                content_width,
                line.width,
                &paragraph.style,
                is_first_line,
            );
            let word_spacing = justified_word_spacing(
                line,
                &paragraph.style,
                paragraph_line_width(content_width, &paragraph.style, is_first_line),
                is_last_line,
            );
            let (border_line_idx, border_line_count) =
                paragraph_border_line_position(is_first_line, is_last_line);
            push_paragraph_borders(
                pages,
                x + padding,
                content_width,
                &paragraph.style,
                border_line_idx,
                border_line_count,
                cursor_y,
                line.height,
                document,
                false,
            );
            push_bar_tab_stops(pages, &paragraph.style, text_x, cursor_y, line.height);
            push_line(pages, line, text_x, cursor_y, document, word_spacing);
            cursor_y -= line.height;
        }
        cursor_y -= twips_to_points(effective_space_after_twips(&paragraph.style));
        if cursor_y < min_y {
            return;
        }
    }
}

fn start_new_page(
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
enum PageParity {
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

fn start_new_page_with_parity(
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

fn advance_column_or_page(
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

#[allow(clippy::too_many_arguments)]
fn start_new_page_for_kept_paragraphs(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    paragraph: &Paragraph,
    next_paragraph: Option<&Paragraph>,
    content_width: f32,
    margin_bottom: f32,
    geometry: &mut PageGeometry,
    current_column: &mut usize,
    document: &Document,
    markers: &MarkerContext,
    font_provider: Option<&FontProvider>,
) {
    if !paragraph.style.keep_together && next_paragraph.is_none() {
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
    ) + next_paragraph
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
        .unwrap_or(0.0);
    let usable_height = (geometry.height - geometry.margin_top - margin_bottom).max(0.0);
    if kept_height <= usable_height && *cursor_y - kept_height < margin_bottom {
        advance_column_or_page(pages, cursor_y, geometry, current_column);
    }
}

fn paragraph_block_height(
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
        + line_height
        + twips_to_points(effective_space_after_twips(&paragraph.style))
}

fn next_paragraph_block(blocks: &[Block], idx: usize) -> Option<&Paragraph> {
    blocks.get(idx + 1).and_then(|block| match block {
        Block::Paragraph(paragraph) => Some(paragraph),
        _ => None,
    })
}

fn previous_paragraph_block(blocks: &[Block], idx: usize) -> Option<&Paragraph> {
    idx.checked_sub(1)
        .and_then(|previous_idx| blocks.get(previous_idx))
        .and_then(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
}

fn paragraph_spacing_is_contextual(first: &Paragraph, second: &Paragraph) -> bool {
    first.style.contextual_spacing && second.style.contextual_spacing
}

fn paragraph_between_border_is_visible(first: &Paragraph, second: &Paragraph) -> bool {
    first.style.borders.between.visible && second.style.borders.between.visible
}

fn apply_page_vertical_alignment(pages: &mut [LayoutPage]) {
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
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct VerticalBounds {
    top: f32,
    bottom: f32,
}

fn page_body_item_bounds(page: &LayoutPage) -> Option<VerticalBounds> {
    page.items
        .iter()
        .filter_map(layout_item_vertical_bounds)
        .reduce(|acc, bounds| VerticalBounds {
            top: acc.top.max(bounds.top),
            bottom: acc.bottom.min(bounds.bottom),
        })
}

fn layout_item_vertical_bounds(item: &LayoutItem) -> Option<VerticalBounds> {
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
        LayoutItem::Line { y1, y2, width, .. } => {
            let half_width = *width / 2.0;
            Some(VerticalBounds {
                top: (*y1).max(*y2) + half_width,
                bottom: (*y1).min(*y2) - half_width,
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
            stroke_width,
            ..
        } => {
            if points.is_empty() {
                return None;
            }
            let half_width = *stroke_width / 2.0;
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
        LayoutItem::Image(image) => Some(VerticalBounds {
            top: image.y + image.height,
            bottom: image.y,
        }),
    }
}

fn translate_layout_item_y(item: &mut LayoutItem, delta_y: f32) {
    match item {
        LayoutItem::Highlight { y, .. } => *y += delta_y,
        LayoutItem::Text(fragment) => fragment.baseline_y += delta_y,
        LayoutItem::Underline { y, .. } => *y += delta_y,
        LayoutItem::Line { y1, y2, .. } => {
            *y1 += delta_y;
            *y2 += delta_y;
        }
        LayoutItem::Ellipse { y, .. } => *y += delta_y,
        LayoutItem::RoundedRectangle { y, .. } => *y += delta_y,
        LayoutItem::Polygon { points, .. } => {
            for point in points {
                point.y += delta_y;
            }
        }
        LayoutItem::Image(image) => image.y += delta_y,
    }
}

fn layout_column_separators(pages: &mut [LayoutPage]) {
    for page in pages {
        let geometry = page.geometry;
        if !geometry.line_between_columns {
            continue;
        }
        for column_idx in 1..geometry.column_count {
            let previous = column_idx - 1;
            let x = geometry.body_left(previous)
                + geometry.body_width(previous)
                + (geometry.column_gaps[previous] / 2.0);
            page.items.push(LayoutItem::Line {
                x1: x,
                y1: geometry.margin_bottom,
                x2: x,
                y2: geometry.height - geometry.margin_top,
                width: 0.5,
                color: PdfColor {
                    red: 0.65,
                    green: 0.65,
                    blue: 0.65,
                },
                style: LineStyle::Solid,
            });
        }
    }
}

fn layout_page_borders(pages: &mut [LayoutPage], document: &Document) {
    for page in pages {
        let geometry = page.geometry;
        let borders = geometry.page_borders;
        let spacing = geometry.page_border_spacing;
        let body_left = geometry.margin_left;
        let body_right = geometry.margin_left + geometry.content_width;
        let body_top = if geometry.page_border_includes_header {
            (geometry.height - geometry.header_distance).max(geometry.height - geometry.margin_top)
        } else {
            geometry.height - geometry.margin_top
        };
        let body_bottom = if geometry.page_border_includes_footer {
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
            let (width, color, style) = table_border_stroke(&borders.top, document);
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
        if borders.bottom.visible {
            let (width, color, style) = table_border_stroke(&borders.bottom, document);
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
            let (width, color, style) = table_border_stroke(&borders.left, document);
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
            let (width, color, style) = table_border_stroke(&borders.right, document);
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

fn layout_background_shapes(
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
            layout_shape(
                &mut scratch_pages,
                &mut cursor_y,
                shape,
                page.width,
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

fn background_shapes_for_page<'a>(
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
fn layout_repeating_header_footer(
    pages: &mut [LayoutPage],
    document: &Document,
    header_footer_sets: &[HeaderFooterSet],
    is_header: bool,
    document_stats: DocumentStats,
    font_provider: Option<&FontProvider>,
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
        let mut cursor_y = if is_header {
            (page.height - geometry.header_distance).clamp(0.0, page.height)
        } else {
            geometry.footer_distance.clamp(0.0, page.height)
        };

        for paragraph in paragraphs {
            let paragraph_contains_page_number = paragraph_contains_page_number_marker(paragraph);
            let lines = wrap_paragraph_with_font_provider(
                paragraph,
                geometry.content_width,
                &markers,
                document,
                font_provider,
            );
            let line_count = lines.len();
            for (line_idx, line) in lines.into_iter().enumerate() {
                let mut x = aligned_x(
                    geometry.margin_left,
                    geometry.content_width,
                    line.width,
                    &paragraph.style,
                    line_idx == 0,
                );
                let mut line_cursor_y = cursor_y;
                if paragraph_contains_page_number {
                    if let Some(page_number_x) = geometry.page_number_x {
                        x = page_number_x.clamp(0.0, geometry.width);
                    }
                    if let Some(page_number_y) = geometry.page_number_y {
                        line_cursor_y =
                            (geometry.height - page_number_y).clamp(0.0, geometry.height);
                    }
                }
                let word_spacing = justified_word_spacing(
                    &line,
                    &paragraph.style,
                    paragraph_line_width(geometry.content_width, &paragraph.style, line_idx == 0),
                    line_idx + 1 == line_count,
                );
                if let Some(color_index) = paragraph.style.shading_color_index
                    && color_index > 0
                {
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
                        color_index,
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
                    word_spacing,
                );
                cursor_y -= line.height;
            }
        }

        for image in images {
            let (left_offset, top_offset, width, height) =
                image_layout_frame(image, geometry.content_width);
            let y = cursor_y - top_offset - height;
            scratch_pages[0]
                .items
                .push(LayoutItem::Image(ImageFragment {
                    image: image.clone(),
                    x: geometry.margin_left + left_offset,
                    y,
                    width,
                    height,
                }));
            cursor_y = y - 6.0;
        }

        for shape in shapes {
            let mut shape_geometry = geometry;
            let mut shape_column = 0usize;
            layout_shape(
                &mut scratch_pages,
                &mut cursor_y,
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

        page.items.extend(scratch_pages.remove(0).items);
    }
}

fn repeating_paragraphs_for_page<'a>(
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
        if physical_page_number % 2 == 0 {
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
        if physical_page_number % 2 == 0 {
            let paragraphs = first_non_empty(&section.even_page_footer, &document.even_page_footer);
            if !paragraphs.is_empty() {
                return paragraphs;
            }
        }
        first_non_empty(&section.footer, &document.footer)
    }
}

fn first_non_empty<'a>(primary: &'a [Paragraph], fallback: &'a [Paragraph]) -> &'a [Paragraph] {
    if primary.is_empty() {
        fallback
    } else {
        primary
    }
}

fn select_repeating_header_footer_images<'a>(
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
        if physical_page_number % 2 == 0 {
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
        if physical_page_number % 2 == 0 {
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

fn first_non_empty_images<'a>(
    primary: &'a [StaticImage],
    fallback: &'a [StaticImage],
) -> &'a [StaticImage] {
    if primary.is_empty() {
        fallback
    } else {
        primary
    }
}

fn select_repeating_header_footer_shapes<'a>(
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
        if physical_page_number % 2 == 0 {
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
        if physical_page_number % 2 == 0 {
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

fn first_non_empty_shapes<'a>(
    primary: &'a [StaticShape],
    fallback: &'a [StaticShape],
) -> &'a [StaticShape] {
    if primary.is_empty() {
        fallback
    } else {
        primary
    }
}

#[allow(clippy::too_many_arguments)]
fn layout_table(
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
) {
    if table.rows.is_empty() {
        return;
    }

    let column_count = table
        .rows
        .iter()
        .map(|row| row.cells.len())
        .max()
        .unwrap_or(0)
        .max(1);
    let column_widths = table_column_widths(table, column_count, content_width);
    let table_width: f32 = column_widths.iter().sum();
    let header_rows = table
        .rows
        .iter()
        .take_while(|row| row.repeat_header)
        .collect::<Vec<_>>();

    for (row_idx, row) in table.rows.iter().enumerate() {
        let next_row = table.rows.get(row_idx + 1);
        let mut prepared = prepare_table_row(
            row,
            &column_widths,
            content_width / column_count as f32,
            &current_marker_context(pages, document_stats),
            document,
            font_provider,
        );

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

        if *cursor_y - prepared.row_height < margin_bottom {
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

        let vertical_span_heights = table_vertical_span_heights(
            table,
            row_idx,
            &prepared,
            &column_widths,
            content_width / column_count as f32,
            current_marker_context(pages, document_stats),
            document,
            font_provider,
            (*cursor_y - geometry.margin_bottom).max(prepared.row_height),
        );
        push_table_row(
            pages,
            cursor_y,
            row,
            &prepared,
            &vertical_span_heights,
            content_width,
            margin_left,
            table_width,
            document,
            table.borders_visible,
            next_row,
        );
    }

    *cursor_y -= 6.0;
}

#[allow(clippy::too_many_arguments)]
fn push_repeating_table_headers(
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
            borders_visible,
            None,
        );
    }
}

fn should_split_tall_table_row(
    row: &TableRow,
    prepared: &PreparedTableRow,
    geometry: PageGeometry,
    margin_bottom: f32,
) -> bool {
    if row.keep_together || row.repeat_header || row.height_twips.is_some_and(|height| height < 0) {
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
fn push_split_table_row(
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
            borders_visible,
            is_final_fragment.then_some(()).and(next_row),
        );
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
        }
    }
}

fn split_prepared_table_row_fragment(
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
        for line in lines {
            let height = prepared_cell_line_height(line);
            if take == 0 || used + height <= capacity {
                take += 1;
                used += height;
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

fn prepared_table_row_has_lines(prepared: &PreparedTableRow) -> bool {
    prepared.cell_lines.iter().any(|lines| !lines.is_empty())
}

fn prepared_table_row_content_height(prepared: &PreparedTableRow) -> f32 {
    prepared
        .cell_lines
        .iter()
        .zip(prepared.cell_paddings.iter())
        .zip(prepared.cell_spacings.iter())
        .map(|((lines, padding), spacing)| {
            lines.iter().map(prepared_cell_line_height).sum::<f32>()
                + padding.top
                + padding.bottom
                + spacing.top
                + spacing.bottom
        })
        .fold(0.0, f32::max)
        .max(14.0)
}

fn prepared_cell_line_height(line: &PreparedCellLine) -> f32 {
    line.space_before + line.line.height + line.space_after
}

#[allow(clippy::too_many_arguments)]
fn table_vertical_span_heights(
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

fn prepare_table_row(
    row: &TableRow,
    column_widths: &[f32],
    default_column_width: f32,
    markers: &MarkerContext,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> PreparedTableRow {
    let visual_cells = table_visual_cells(row, column_widths, default_column_width);
    let cell_paddings = visual_cells
        .iter()
        .map(|visual_cell| resolve_cell_padding(row, &row.cells[visual_cell.cell_index]))
        .collect::<Vec<_>>();
    let cell_spacings = visual_cells
        .iter()
        .map(|visual_cell| resolve_cell_spacing(&row.cells[visual_cell.cell_index]))
        .collect::<Vec<_>>();
    let cell_lines = visual_cells
        .iter()
        .zip(cell_paddings.iter())
        .zip(cell_spacings.iter())
        .map(|((visual_cell, padding), spacing)| {
            let cell = &row.cells[visual_cell.cell_index];
            let cell_content_width =
                (visual_cell.width - spacing.left - spacing.right - padding.left - padding.right)
                    .max(12.0);
            cell.paragraphs
                .iter()
                .enumerate()
                .flat_map(|(paragraph_idx, paragraph)| {
                    let fit_text_paragraph;
                    let layout_paragraph = if cell.fit_text {
                        fit_text_paragraph = passive_fit_text_paragraph(paragraph);
                        &fit_text_paragraph
                    } else {
                        paragraph
                    };
                    let mut lines = wrap_paragraph_with_font_provider(
                        layout_paragraph,
                        cell_content_width,
                        markers,
                        document,
                        font_provider,
                    );
                    for line in &mut lines {
                        if cell.fit_text {
                            apply_passive_table_cell_fit_text(line, cell_content_width);
                        }
                        line.height = apply_line_spacing(line.height, &layout_paragraph.style);
                    }
                    let line_count = lines.len();
                    let suppress_contextual_space_before = paragraph_idx
                        .checked_sub(1)
                        .and_then(|previous_idx| cell.paragraphs.get(previous_idx))
                        .is_some_and(|previous| {
                            paragraph_spacing_is_contextual(previous, paragraph)
                        });
                    let suppress_contextual_space_after = cell
                        .paragraphs
                        .get(paragraph_idx + 1)
                        .is_some_and(|next| paragraph_spacing_is_contextual(paragraph, next));
                    let paragraph_space_before = if suppress_contextual_space_before {
                        0.0
                    } else {
                        twips_to_points(effective_space_before_twips(&paragraph.style))
                    };
                    let paragraph_space_after = if suppress_contextual_space_after {
                        0.0
                    } else {
                        twips_to_points(effective_space_after_twips(&paragraph.style))
                    };
                    lines
                        .into_iter()
                        .enumerate()
                        .map(move |(idx, line)| PreparedCellLine {
                            line,
                            style: paragraph.style.clone(),
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

    let row_height = cell_lines
        .iter()
        .zip(cell_paddings.iter())
        .zip(cell_spacings.iter())
        .map(|((lines, padding), spacing)| {
            lines.iter().map(prepared_cell_line_height).sum::<f32>()
                + padding.top
                + padding.bottom
                + spacing.top
                + spacing.bottom
        })
        .fold(0.0, f32::max)
        .max(14.0);
    let row_height = match row.height_twips {
        Some(height) if height < 0 => {
            twips_to_points(height.checked_abs().unwrap_or(i32::MAX)).max(1.0)
        }
        Some(height) => twips_to_points(height).max(row_height),
        None => row_height,
    };

    PreparedTableRow {
        visual_cells,
        cell_lines,
        cell_paddings,
        cell_spacings,
        row_height,
    }
}

fn passive_fit_text_paragraph(paragraph: &Paragraph) -> Paragraph {
    let mut paragraph = paragraph.clone();
    paragraph.style.no_wrap = true;
    paragraph
}

fn apply_passive_table_cell_fit_text(line: &mut Line, content_width: f32) {
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

fn resolve_cell_padding(row: &TableRow, cell: &TableCell) -> ResolvedCellPadding {
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

fn resolve_cell_spacing(cell: &TableCell) -> ResolvedCellSpacing {
    let spacing = cell.spacing;
    ResolvedCellSpacing {
        left: twips_to_points(spacing.left_twips.unwrap_or(0).max(0)),
        right: twips_to_points(spacing.right_twips.unwrap_or(0).max(0)),
        top: twips_to_points(spacing.top_twips.unwrap_or(0).max(0)),
        bottom: twips_to_points(spacing.bottom_twips.unwrap_or(0).max(0)),
    }
}

#[allow(clippy::too_many_arguments)]
fn push_table_row(
    pages: &mut [LayoutPage],
    cursor_y: &mut f32,
    row: &TableRow,
    prepared: &PreparedTableRow,
    vertical_span_heights: &[f32],
    content_width: f32,
    margin_left: f32,
    table_width: f32,
    document: &Document,
    borders_visible: bool,
    next_row: Option<&TableRow>,
) {
    let row_left = table_row_left(margin_left, content_width, table_width, row.alignment)
        + twips_to_points(row.left_offset_twips);
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
        if let Some(color_index) = cell.shading_color_index
            && color_index > 0
        {
            push_shading_rect(
                pages,
                document,
                cell_left,
                cell_top - cell_height,
                cell_width,
                cell_height,
                color_index,
                cell.shading_basis_points,
                cell.shading_pattern,
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
        let content_height = lines.iter().map(prepared_cell_line_height).sum::<f32>();
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
        for prepared_line in lines {
            line_top -= prepared_line.space_before;
            if line_top - prepared_line.line.height < content_bottom {
                break;
            }
            let content_left = row_left + visual_cell.x_offset + spacing.left + padding.left;
            let cell_content_width =
                (visual_cell.width - spacing.left - spacing.right - padding.left - padding.right)
                    .max(1.0);
            if let Some(color_index) = prepared_line.style.shading_color_index
                && color_index > 0
            {
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
                    color_index,
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
                line_top,
                prepared_line.line.height,
                document,
                false,
            );
            push_bar_tab_stops(
                pages,
                &prepared_line.style,
                x,
                line_top,
                prepared_line.line.height,
            );
            push_line(
                pages,
                &prepared_line.line,
                x,
                line_top,
                document,
                word_spacing,
            );
            line_top -= prepared_line.line.height + prepared_line.space_after;
        }
    }

    *cursor_y -= prepared.row_height;
}

fn paragraph_border_line_position(is_first_line: bool, is_last_line: bool) -> (usize, usize) {
    match (is_first_line, is_last_line) {
        (true, true) => (0, 1),
        (true, false) => (0, 2),
        (false, true) => (1, 2),
        (false, false) => (1, 3),
    }
}

fn table_visual_cells(
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

fn table_row_left(
    margin_left: f32,
    content_width: f32,
    table_width: f32,
    alignment: TableRowAlignment,
) -> f32 {
    match alignment {
        TableRowAlignment::Left => margin_left,
        TableRowAlignment::Center => margin_left + ((content_width - table_width) / 2.0).max(0.0),
        TableRowAlignment::Right => margin_left + (content_width - table_width).max(0.0),
    }
}

fn table_column_widths(table: &Table, column_count: usize, content_width: f32) -> Vec<f32> {
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

fn push_table_borders(
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
            page.items.push(LayoutItem::Line {
                x1,
                y1: cell_top,
                x2,
                y2: cell_top,
                width,
                color,
                style,
            });
        }
        if cell.borders.bottom.visible
            && !next_row_continues_vertical_merge(row, next_row, visual_cell.cell_index)
        {
            let (width, color, style) = table_border_stroke(&cell.borders.bottom, document);
            page.items.push(LayoutItem::Line {
                x1,
                y1: cell_bottom,
                x2,
                y2: cell_bottom,
                width,
                color,
                style,
            });
        }
        if cell.borders.left.visible {
            let (width, color, style) = table_border_stroke(&cell.borders.left, document);
            page.items.push(LayoutItem::Line {
                x1,
                y1: cell_top,
                x2: x1,
                y2: cell_bottom,
                width,
                color,
                style,
            });
        }
        if cell.borders.right.visible {
            let (width, color, style) = table_border_stroke(&cell.borders.right, document);
            page.items.push(LayoutItem::Line {
                x1: x2,
                y1: cell_top,
                x2,
                y2: cell_bottom,
                width,
                color,
                style,
            });
        }
        if cell.borders.diagonal_down.visible {
            let (width, color, style) = table_border_stroke(&cell.borders.diagonal_down, document);
            page.items.push(LayoutItem::Line {
                x1,
                y1: cell_top,
                x2,
                y2: cell_bottom,
                width,
                color,
                style,
            });
        }
        if cell.borders.diagonal_up.visible {
            let (width, color, style) = table_border_stroke(&cell.borders.diagonal_up, document);
            page.items.push(LayoutItem::Line {
                x1,
                y1: cell_bottom,
                x2,
                y2: cell_top,
                width,
                color,
                style,
            });
        }
    }
}

fn next_row_continues_vertical_merge(
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

fn table_border_stroke(
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
        .unwrap_or(PdfColor {
            red: 0.65,
            green: 0.65,
            blue: 0.65,
        });
    let style = line_style_for_border_style(border.style);
    (width, color, style)
}

#[allow(clippy::too_many_arguments)]
fn push_paragraph_borders(
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
    let Some(page) = pages.last_mut() else {
        return;
    };

    if style.borders.top.visible && line_idx == 0 {
        let border = &style.borders.top;
        let (width, color, style) = table_border_stroke(border, document);
        let spacing = twips_to_points(border.spacing_twips.max(0));
        page.items.push(LayoutItem::Line {
            x1,
            y1: y1 + spacing,
            x2,
            y2: y1 + spacing,
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
            x1,
            y1: y2 - spacing,
            x2,
            y2: y2 - spacing,
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
            x1,
            y1: y2 - spacing,
            x2,
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
        page.items.push(LayoutItem::Line {
            x1: x1 - spacing,
            y1,
            x2: x1 - spacing,
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
        page.items.push(LayoutItem::Line {
            x1: x2 + spacing,
            y1,
            x2: x2 + spacing,
            y2,
            width,
            color,
            style,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn layout_paragraph(
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
) {
    let mut markers = markers.clone();
    if paragraph.style.page_break_before && !pages.last().is_none_or(|page| page.items.is_empty()) {
        let previous_page_count = pages.len();
        start_new_page(pages, cursor_y, geometry, current_column);
        reset_line_numbers_for_page_restart(
            line_numbers.as_mut().map(|state| &mut **state),
            *geometry,
            previous_page_count,
            pages.len(),
        );
        markers = current_marker_context(pages, document_stats);
    }
    let previous_page_count = pages.len();
    start_new_page_for_kept_paragraphs(
        pages,
        cursor_y,
        paragraph,
        None,
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
    let mut margin_left = geometry.body_left(*current_column);
    markers = current_marker_context(pages, document_stats);

    let paragraph_top = if suppress_contextual_space_before {
        0.0
    } else {
        twips_to_points(effective_space_before_twips(&paragraph.style))
    };
    *cursor_y -= paragraph_top;
    let mut lines = wrap_paragraph_with_font_provider_dynamic_width(
        paragraph,
        content_width,
        &markers,
        document,
        font_provider,
        *cursor_y,
        |line_top_y, line_height| {
            wrapped_image_text_area_for_line(
                pages,
                margin_left,
                content_width,
                line_top_y,
                line_height,
            )
            .map(|(_, width)| width)
            .unwrap_or(content_width)
        },
    );
    for line in &mut lines {
        line.height = apply_line_spacing_with_grid(line.height, &paragraph.style, *geometry);
    }
    let line_count = lines.len();
    for (line_idx, line) in lines.iter().enumerate() {
        if should_advance_for_widow_control(
            &paragraph.style,
            &lines,
            line_idx,
            *cursor_y,
            margin_bottom,
        ) {
            let previous_page_count = pages.len();
            advance_column_or_page(pages, cursor_y, geometry, current_column);
            reset_line_numbers_for_page_restart(
                line_numbers.as_mut().map(|state| &mut **state),
                *geometry,
                previous_page_count,
                pages.len(),
            );
            margin_left = geometry.body_left(*current_column);
        }
        if *cursor_y - line.height < margin_bottom {
            let previous_page_count = pages.len();
            advance_column_or_page(pages, cursor_y, geometry, current_column);
            reset_line_numbers_for_page_restart(
                line_numbers.as_mut().map(|state| &mut **state),
                *geometry,
                previous_page_count,
                pages.len(),
            );
            margin_left = geometry.body_left(*current_column);
        }

        let (line_margin_left, line_content_width) = wrapped_image_text_area_for_line(
            pages,
            margin_left,
            content_width,
            *cursor_y,
            line.height,
        )
        .unwrap_or((margin_left, content_width));

        let x = aligned_x(
            line_margin_left,
            line_content_width,
            line.width,
            &paragraph.style,
            line_idx == 0,
        );
        if let Some(color_index) = paragraph.style.shading_color_index
            && color_index > 0
        {
            let line_left_indent = twips_to_points(paragraph_line_left_indent_twips(
                &paragraph.style,
                line_idx == 0,
            ));
            push_shading_rect(
                pages,
                document,
                line_margin_left + line_left_indent,
                *cursor_y - line.height,
                paragraph_line_width(line_content_width, &paragraph.style, line_idx == 0),
                line.height,
                color_index,
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
        );
        let word_spacing = justified_word_spacing(
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
            );
        }
        push_bar_tab_stops(pages, &paragraph.style, x, *cursor_y, line.height);
        push_line(pages, &line, x, *cursor_y, document, word_spacing);
        *cursor_y -= line.height;
    }
    if !suppress_contextual_space_after {
        *cursor_y -= twips_to_points(effective_space_after_twips(&paragraph.style));
    }
}

fn reset_line_numbers_for_page_restart(
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

fn wrapped_image_text_area_for_line(
    pages: &[LayoutPage],
    margin_left: f32,
    content_width: f32,
    line_top_y: f32,
    line_height: f32,
) -> Option<(f32, f32)> {
    let page = pages.last()?;
    let content_right = margin_left + content_width;
    let line_bottom_y = line_top_y - line_height.max(0.0);
    let mut free_intervals = vec![(margin_left, content_right)];
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
        let image_bottom = image.y;
        let image_top = image.y + image.height;
        if line_top_y <= image_bottom || line_bottom_y >= image_top {
            continue;
        }
        let left_gap = twips_to_points(placement.wrap_margin_left_twips.max(0));
        let right_gap = twips_to_points(placement.wrap_margin_right_twips.max(0));
        let excluded_left = (image_left - left_gap).max(margin_left);
        let excluded_right = (image_right + right_gap).min(content_right);
        if excluded_left >= excluded_right {
            continue;
        }
        let mut next_intervals = Vec::with_capacity(free_intervals.len() + 1);
        for (left, right) in free_intervals {
            if excluded_right <= left || excluded_left >= right {
                next_intervals.push((left, right));
                continue;
            }
            if left < excluded_left {
                next_intervals.push((left, excluded_left));
            }
            if excluded_right < right {
                next_intervals.push((excluded_right, right));
            }
        }
        free_intervals = next_intervals;
    }
    free_intervals
        .into_iter()
        .map(|(left, right)| (left, (right - left).max(0.0)))
        .filter(|(_, width)| *width >= 12.0)
        .max_by(|(left_a, width_a), (left_b, width_b)| {
            width_a
                .total_cmp(width_b)
                .then_with(|| left_b.total_cmp(left_a))
        })
}

fn reset_line_numbers_for_section_restart(
    state: Option<&mut LineNumberState>,
    geometry: PageGeometry,
) {
    if geometry.line_numbering.restart == LineNumberRestart::Section
        && let Some(state) = state
    {
        state.reset_for_geometry(geometry);
    }
}

fn next_block_is_section_settings(document: &Document, block_idx: usize) -> bool {
    document
        .blocks
        .get(block_idx.saturating_add(1))
        .is_some_and(|block| matches!(block, Block::SectionSettings(_)))
}

fn push_passive_line_number(
    pages: &mut [LayoutPage],
    line: &Line,
    margin_left: f32,
    top_y: f32,
    geometry: PageGeometry,
    state: &mut LineNumberState,
    suppress_number: bool,
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
    style.font_size_half_points = 14;
    let text = current.to_string();
    let width = measure_text_with_family(&text, &style, PdfFontFamily::Helvetica);
    let distance = twips_to_points(geometry.line_numbering.distance_twips.max(0));
    let x = (margin_left - distance - width).max(0.0);
    let baseline_y = top_y - line.height + (line.height * 0.25);
    let Some(page) = pages.last_mut() else {
        return;
    };
    page.items.push(LayoutItem::Text(TextFragment {
        text,
        x,
        baseline_y,
        color: PdfColor {
            red: 0.35,
            green: 0.35,
            blue: 0.35,
        },
        font_family: PdfFontFamily::Helvetica,
        word_spacing: 0.0,
        style,
    }));
}

fn line_has_visible_text(line: &Line) -> bool {
    line.runs.iter().any(|run| {
        !run.style.hidden
            && !run.text.trim().is_empty()
            && parse_bookmark_page_marker_id(&run.text, BOOKMARK_PAGE_ANCHOR_MARKER).is_none()
            && parse_bookmark_page_marker_id(&run.text, BOOKMARK_PAGE_REF_MARKER).is_none()
    })
}

fn effective_space_before_twips(style: &ParagraphStyle) -> i32 {
    if style.auto_space_before {
        AUTO_PARAGRAPH_SPACING_TWIPS
    } else {
        style.space_before_twips
    }
}

fn effective_space_after_twips(style: &ParagraphStyle) -> i32 {
    if style.auto_space_after {
        AUTO_PARAGRAPH_SPACING_TWIPS
    } else {
        style.space_after_twips
    }
}

fn should_advance_for_widow_control(
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

#[cfg(test)]
fn wrap_paragraph(
    paragraph: &Paragraph,
    content_width: f32,
    markers: &MarkerContext,
    document: &Document,
) -> Vec<Line> {
    wrap_paragraph_with_font_provider(paragraph, content_width, markers, document, None)
}

fn wrap_paragraph_with_font_provider(
    paragraph: &Paragraph,
    content_width: f32,
    markers: &MarkerContext,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> Vec<Line> {
    wrap_paragraph_with_font_provider_dynamic_width(
        paragraph,
        content_width,
        markers,
        document,
        font_provider,
        0.0,
        |_, _| content_width,
    )
}

fn wrap_paragraph_with_font_provider_dynamic_width(
    paragraph: &Paragraph,
    content_width: f32,
    markers: &MarkerContext,
    document: &Document,
    font_provider: Option<&FontProvider>,
    first_line_top_y: f32,
    mut line_content_width: impl FnMut(f32, f32) -> f32,
) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut current = Line {
        runs: Vec::new(),
        width: 0.0,
        height: 14.0,
    };
    let mut is_first_line = true;
    let mut current_line_top_y = first_line_top_y;
    let mut drop_cap_applied = false;

    for run in &paragraph.runs {
        let mut segments = split_run_for_wrapping_with_drop_cap(
            run,
            markers,
            &paragraph.style,
            &mut drop_cap_applied,
        );
        if paragraph.style.auto_hyphenation && !paragraph.style.no_wrap {
            segments = apply_passive_auto_hyphenation(
                segments,
                content_width,
                &paragraph.style,
                document,
                font_provider,
            );
        }
        for segment in segments {
            if segment.text == "\n" {
                let finished_height = current.height;
                lines.push(current);
                current_line_top_y -= finished_height;
                current = empty_line();
                is_first_line = false;
                continue;
            }

            let width = measure_flow_run(
                &segment,
                current.width,
                &paragraph.style,
                document,
                font_provider,
            );
            let requested_content_width = line_content_width(current_line_top_y, current.height);
            let active_content_width = if content_width >= 12.0 {
                requested_content_width.clamp(12.0, content_width)
            } else {
                requested_content_width.clamp(0.0, content_width.max(0.0))
            };
            let line_width =
                paragraph_line_width(active_content_width, &paragraph.style, is_first_line);
            if !paragraph.style.no_wrap && current.width > 0.0 && current.width + width > line_width
            {
                materialize_line_end_soft_hyphen(&mut current, document, font_provider);
                let finished_height = current.height;
                lines.push(current);
                current_line_top_y -= finished_height;
                current = empty_line();
                is_first_line = false;
                let trimmed = segment.text.trim_start().to_string();
                if trimmed.is_empty() {
                    continue;
                }
                push_segment(
                    &mut current,
                    FlowRun {
                        text: trimmed,
                        style: segment.style,
                        width: 0.0,
                        line_height_points: segment.line_height_points,
                        tab_leader: TabLeader::None,
                        tab_alignment: TabAlignment::Left,
                        tab_stop_position: None,
                        soft_hyphen_after: segment.soft_hyphen_after,
                    },
                    &paragraph.style,
                    document,
                    font_provider,
                );
            } else {
                push_segment(
                    &mut current,
                    segment,
                    &paragraph.style,
                    document,
                    font_provider,
                );
            }
        }
    }

    if !current.runs.is_empty() {
        lines.push(current);
    }

    if lines.is_empty() {
        lines.push(empty_line());
    }

    lines
}

fn split_run_for_wrapping_with_drop_cap(
    run: &Run,
    markers: &MarkerContext,
    paragraph_style: &ParagraphStyle,
    drop_cap_applied: &mut bool,
) -> Vec<FlowRun> {
    let segments = split_run_for_wrapping(run, markers);
    if *drop_cap_applied || paragraph_style.drop_cap_lines <= 1 {
        return segments;
    }
    apply_drop_cap_to_segments(segments, paragraph_style.drop_cap_lines, drop_cap_applied)
}

fn apply_drop_cap_to_segments(
    segments: Vec<FlowRun>,
    drop_cap_lines: i32,
    drop_cap_applied: &mut bool,
) -> Vec<FlowRun> {
    let mut output = Vec::new();
    for segment in segments {
        if *drop_cap_applied || segment.text == "\t" || segment.text == "\n" {
            output.push(segment);
            continue;
        }

        let Some((cap_start, cap)) = segment
            .text
            .char_indices()
            .find(|(_, ch)| !ch.is_whitespace())
        else {
            output.push(segment);
            continue;
        };
        let cap_end = cap_start + cap.len_utf8();

        if cap_start > 0 {
            output.push(FlowRun {
                text: segment.text[..cap_start].to_string(),
                style: segment.style.clone(),
                width: 0.0,
                line_height_points: segment.line_height_points,
                tab_leader: segment.tab_leader,
                tab_alignment: segment.tab_alignment,
                tab_stop_position: segment.tab_stop_position,
                soft_hyphen_after: false,
            });
        }

        let mut cap_style = segment.style.clone();
        cap_style.font_size_half_points = cap_style
            .font_size_half_points
            .saturating_mul(drop_cap_lines.max(1))
            .clamp(2, MAX_SYNTHETIC_DROP_CAP_FONT_SIZE_HALF_POINTS);
        output.push(FlowRun {
            text: cap.to_string(),
            line_height_points: cap_style.font_size_points(),
            style: cap_style,
            width: 0.0,
            tab_leader: segment.tab_leader,
            tab_alignment: segment.tab_alignment,
            tab_stop_position: segment.tab_stop_position,
            soft_hyphen_after: false,
        });

        if cap_end < segment.text.len() {
            output.push(FlowRun {
                text: segment.text[cap_end..].to_string(),
                style: segment.style,
                width: 0.0,
                line_height_points: segment.line_height_points,
                tab_leader: segment.tab_leader,
                tab_alignment: segment.tab_alignment,
                tab_stop_position: segment.tab_stop_position,
                soft_hyphen_after: segment.soft_hyphen_after,
            });
        }

        *drop_cap_applied = true;
    }
    output
}

fn apply_passive_auto_hyphenation(
    segments: Vec<FlowRun>,
    content_width: f32,
    paragraph_style: &ParagraphStyle,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> Vec<FlowRun> {
    let max_line_width = paragraph_line_width(content_width, paragraph_style, false)
        .max(paragraph_line_width(content_width, paragraph_style, true));
    let hyphenation_zone = twips_to_points(paragraph_style.hyphenation_zone_twips.max(0));
    let mut output = Vec::new();
    let mut consecutive_hyphenated = 0usize;

    for segment in segments {
        if !segment_can_auto_hyphenate(&segment, paragraph_style) {
            consecutive_hyphenated = 0;
            output.push(segment);
            continue;
        }
        let display = display_text(&segment.text, &segment.style);
        let family = font_family_for_run_text(document, &segment.style, &display);
        if measure_text_with_document_font(
            &display,
            &segment.style,
            family,
            document,
            font_provider,
        ) <= max_line_width + hyphenation_zone
        {
            consecutive_hyphenated = 0;
            output.push(segment);
            continue;
        }
        push_auto_hyphenated_segment(
            &mut output,
            segment,
            max_line_width,
            family,
            document,
            font_provider,
            paragraph_style.max_consecutive_hyphenated_lines,
            &mut consecutive_hyphenated,
        );
    }

    output
}

fn segment_can_auto_hyphenate(segment: &FlowRun, paragraph_style: &ParagraphStyle) -> bool {
    segment.tab_stop_position.is_none()
        && segment.text != "\t"
        && segment.text != "\n"
        && !contains_inline_marker(&segment.text)
        && !segment.text.contains('\u{00ad}')
        && segment.text.chars().count() >= 10
        && segment.text.chars().all(|ch| ch.is_alphabetic())
        && (paragraph_style.hyphenate_caps || !is_all_caps_word(&segment.text))
}

fn is_all_caps_word(text: &str) -> bool {
    let mut has_uppercase = false;
    for ch in text.chars().filter(|ch| ch.is_alphabetic()) {
        if ch.is_lowercase() {
            return false;
        }
        if ch.is_uppercase() {
            has_uppercase = true;
        }
    }
    has_uppercase
}

fn push_auto_hyphenated_segment(
    output: &mut Vec<FlowRun>,
    segment: FlowRun,
    max_line_width: f32,
    family: PdfFontFamily,
    document: &Document,
    font_provider: Option<&FontProvider>,
    max_consecutive_hyphenated: Option<usize>,
    consecutive_hyphenated: &mut usize,
) {
    let chars = segment.text.chars().collect::<Vec<_>>();
    let mut start = 0;
    while start < chars.len() {
        if max_consecutive_hyphenated.is_some_and(|max| *consecutive_hyphenated >= max) {
            push_flow_run(
                output,
                &chars[start..].iter().collect::<String>(),
                &segment.style,
                segment.soft_hyphen_after,
            );
            *consecutive_hyphenated = 0;
            break;
        }

        let remaining = chars.len() - start;
        if remaining <= 6 {
            push_flow_run(
                output,
                &chars[start..].iter().collect::<String>(),
                &segment.style,
                segment.soft_hyphen_after,
            );
            *consecutive_hyphenated = 0;
            break;
        }

        let mut end = chars.len() - 3;
        while end > start + 3 {
            let text = chars[start..end].iter().collect::<String>();
            let text_width = measure_text_with_document_font(
                &text,
                &segment.style,
                family,
                document,
                font_provider,
            );
            let hyphen_width = measure_text_with_document_font(
                "-",
                &segment.style,
                family,
                document,
                font_provider,
            );
            if text_width + hyphen_width <= max_line_width {
                break;
            }
            end -= 1;
        }

        if end <= start + 3 {
            push_flow_run(
                output,
                &chars[start..].iter().collect::<String>(),
                &segment.style,
                segment.soft_hyphen_after,
            );
            *consecutive_hyphenated = 0;
            break;
        }

        push_flow_run(
            output,
            &chars[start..end].iter().collect::<String>(),
            &segment.style,
            true,
        );
        *consecutive_hyphenated = consecutive_hyphenated.saturating_add(1);
        start = end;
    }
}

fn split_run_for_wrapping(run: &Run, markers: &MarkerContext) -> Vec<FlowRun> {
    let mut output = Vec::new();
    if run.style.hidden {
        return output;
    }
    let mut start = 0;
    let resolved_text;
    let text = if contains_inline_marker(&run.text) {
        resolved_text = run
            .text
            .replace(PAGE_NUMBER_MARKER, &markers.page_number)
            .replace(SECTION_NUMBER_MARKER, &markers.section_number)
            .replace(DOCUMENT_WORDS_MARKER, &markers.document_words)
            .replace(DOCUMENT_CHARS_MARKER, &markers.document_chars)
            .replace(
                DOCUMENT_CHARS_WITH_SPACES_MARKER,
                &markers.document_chars_with_spaces,
            );
        resolved_text.as_str()
    } else {
        run.text.as_str()
    };

    for (idx, opportunity) in linebreaks(text) {
        if matches!(
            opportunity,
            BreakOpportunity::Allowed | BreakOpportunity::Mandatory
        ) {
            if run.style.form_field_shading && matches!(opportunity, BreakOpportunity::Allowed) {
                continue;
            }
            let segment_text = &text[start..idx];
            if !segment_text.is_empty() {
                let soft_hyphen_after = segment_text.ends_with('\u{00ad}');
                push_text_segments_preserving_tabs(
                    &mut output,
                    segment_text,
                    &run.style,
                    soft_hyphen_after,
                );
            }
            if matches!(opportunity, BreakOpportunity::Mandatory) && idx < text.len() {
                output.push(FlowRun {
                    text: "\n".to_string(),
                    style: run.style.clone(),
                    width: 0.0,
                    line_height_points: run.style.font_size_points(),
                    tab_leader: TabLeader::None,
                    tab_alignment: TabAlignment::Left,
                    tab_stop_position: None,
                    soft_hyphen_after: false,
                });
            }
            start = idx;
        }
    }

    if start < text.len() {
        push_text_segments_preserving_tabs(&mut output, &text[start..], &run.style, false);
    }

    output
}

fn current_marker_context(pages: &[LayoutPage], document_stats: DocumentStats) -> MarkerContext {
    pages.last().map_or_else(
        || marker_context("1".to_string(), "1".to_string(), document_stats),
        |page| {
            marker_context(
                page.display_page_number.clone(),
                page.section_number.to_string(),
                document_stats,
            )
        },
    )
}

fn marker_context(
    page_number: String,
    section_number: String,
    document_stats: DocumentStats,
) -> MarkerContext {
    MarkerContext {
        page_number,
        section_number,
        document_words: document_stats.words.to_string(),
        document_chars: document_stats.chars.to_string(),
        document_chars_with_spaces: document_stats.chars_with_spaces.to_string(),
    }
}

fn contains_inline_marker(text: &str) -> bool {
    text.contains(PAGE_NUMBER_MARKER)
        || text.contains(SECTION_NUMBER_MARKER)
        || text.contains(TOTAL_PAGES_MARKER)
        || text.contains(SECTION_PAGES_MARKER)
        || text.contains(DOCUMENT_WORDS_MARKER)
        || text.contains(DOCUMENT_CHARS_MARKER)
        || text.contains(DOCUMENT_CHARS_WITH_SPACES_MARKER)
}

fn paragraph_contains_page_number_marker(paragraph: &Paragraph) -> bool {
    paragraph.runs.iter().any(|run| {
        run.text.contains(PAGE_NUMBER_MARKER)
            || run.text.contains(TOTAL_PAGES_MARKER)
            || run.text.contains(SECTION_PAGES_MARKER)
    })
}

#[derive(Debug, Default)]
struct DocumentStatsBuilder {
    stats: DocumentStats,
    in_word: bool,
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
}

fn document_stats(document: &Document) -> DocumentStats {
    let mut builder = DocumentStatsBuilder::default();
    for paragraph in &document.header {
        push_paragraph_stats(&mut builder, paragraph);
    }
    for paragraph in &document.first_page_header {
        push_paragraph_stats(&mut builder, paragraph);
    }
    for paragraph in &document.even_page_header {
        push_paragraph_stats(&mut builder, paragraph);
    }
    for paragraph in &document.footer {
        push_paragraph_stats(&mut builder, paragraph);
    }
    for paragraph in &document.first_page_footer {
        push_paragraph_stats(&mut builder, paragraph);
    }
    for paragraph in &document.even_page_footer {
        push_paragraph_stats(&mut builder, paragraph);
    }
    for paragraph in &document.footnotes {
        push_paragraph_stats(&mut builder, paragraph);
    }
    for paragraph in &document.endnotes {
        push_paragraph_stats(&mut builder, paragraph);
    }
    for block in &document.blocks {
        push_block_stats(&mut builder, block);
    }
    builder.stats
}

fn push_block_stats(builder: &mut DocumentStatsBuilder, block: &Block) {
    match block {
        Block::Paragraph(paragraph) => push_paragraph_stats(builder, paragraph),
        Block::Table(table) => {
            for row in &table.rows {
                for cell in &row.cells {
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
            for paragraph in &settings.header {
                push_paragraph_stats(builder, paragraph);
            }
            for paragraph in &settings.first_page_header {
                push_paragraph_stats(builder, paragraph);
            }
            for paragraph in &settings.even_page_header {
                push_paragraph_stats(builder, paragraph);
            }
            for paragraph in &settings.footer {
                push_paragraph_stats(builder, paragraph);
            }
            for paragraph in &settings.first_page_footer {
                push_paragraph_stats(builder, paragraph);
            }
            for paragraph in &settings.even_page_footer {
                push_paragraph_stats(builder, paragraph);
            }
        }
        Block::Shape(shape) => {
            for paragraph in &shape.text {
                push_paragraph_stats(builder, paragraph);
            }
            builder.finish_text_boundary();
        }
        Block::Image(_)
        | Block::PageBreak
        | Block::ColumnBreak
        | Block::ContinuousSectionBreak
        | Block::SectionBreak
        | Block::EvenPageSectionBreak
        | Block::OddPageSectionBreak => builder.finish_text_boundary(),
    }
}

fn push_paragraph_stats(builder: &mut DocumentStatsBuilder, paragraph: &Paragraph) {
    for run in &paragraph.runs {
        if !run.style.hidden {
            builder.push_text(&run.text);
        }
    }
    builder.finish_text_boundary();
}

fn remove_internal_stat_markers(text: &str) -> String {
    let mut cleaned = text
        .replace(PAGE_NUMBER_MARKER, "")
        .replace(TOTAL_PAGES_MARKER, "")
        .replace(SECTION_NUMBER_MARKER, "")
        .replace(SECTION_PAGES_MARKER, "")
        .replace(DOCUMENT_WORDS_MARKER, "")
        .replace(DOCUMENT_CHARS_MARKER, "")
        .replace(DOCUMENT_CHARS_WITH_SPACES_MARKER, "");
    while let Some((start, end)) = next_bookmark_page_marker_range(&cleaned, 0) {
        cleaned.replace_range(start..end, "");
    }
    cleaned
}

fn format_page_number(value: usize, format: PageNumberFormat) -> String {
    let value = value.min(i32::MAX as usize) as i32;
    match format {
        PageNumberFormat::Decimal => value.to_string(),
        PageNumberFormat::UpperRoman => format_roman_page_number(value, false),
        PageNumberFormat::LowerRoman => format_roman_page_number(value, true),
        PageNumberFormat::UpperLetter => format_alpha_page_number(value, false),
        PageNumberFormat::LowerLetter => format_alpha_page_number(value, true),
    }
}

fn format_note_number(start: i32, sequence: usize, format: PageNumberFormat) -> String {
    let start = start.max(1) as usize;
    format_page_number(start.saturating_add(sequence.saturating_sub(1)), format)
}

fn format_roman_page_number(value: i32, lowercase: bool) -> String {
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

fn format_alpha_page_number(value: i32, lowercase: bool) -> String {
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

fn resolve_total_page_markers(pages: &mut [LayoutPage]) {
    let total_pages = pages.len().to_string();
    for page in pages {
        replace_late_page_marker(page, TOTAL_PAGES_MARKER, &total_pages);
    }
}

fn resolve_section_page_markers(pages: &mut [LayoutPage]) {
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
        replace_late_page_marker(page, SECTION_PAGES_MARKER, &section_pages);
    }
}

#[derive(Debug, Copy, Clone)]
struct LateMarkerWidthAdjustment {
    text_index: usize,
    old_width: f32,
    new_width: f32,
}

#[derive(Debug, Copy, Clone)]
struct LateMarkerGeometry {
    x: f32,
    baseline_y: f32,
    old_width: f32,
    new_width: f32,
    font_size: f32,
}

fn replace_late_page_marker(page: &mut LayoutPage, marker: &str, replacement: &str) {
    let mut adjustments = Vec::new();
    for (idx, item) in page.items.iter_mut().enumerate() {
        let LayoutItem::Text(fragment) = item else {
            continue;
        };
        if !fragment.text.contains(marker) {
            continue;
        }
        let old_text = fragment.text.clone();
        let new_text = old_text.replace(marker, replacement);
        let old_width = passive_text_fragment_width(fragment, &old_text);
        let new_width = passive_text_fragment_width(fragment, &new_text);
        fragment.text = new_text;
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

fn passive_text_fragment_width(fragment: &TextFragment, text: &str) -> f32 {
    let text = late_page_count_measurement_text(text);
    measure_text_with_family(&text, &fragment.style, fragment.font_family)
        + fragment.word_spacing * regular_space_count(&text) as f32
}

fn late_page_count_measurement_text(text: &str) -> String {
    if text.contains(TOTAL_PAGES_MARKER) || text.contains(SECTION_PAGES_MARKER) {
        text.replace(TOTAL_PAGES_MARKER, LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER)
            .replace(SECTION_PAGES_MARKER, LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER)
    } else {
        text.to_string()
    }
}

fn late_marker_geometry(
    page: &LayoutPage,
    adjustment: LateMarkerWidthAdjustment,
) -> Option<LateMarkerGeometry> {
    let LayoutItem::Text(fragment) = page.items.get(adjustment.text_index)? else {
        return None;
    };
    Some(LateMarkerGeometry {
        x: fragment.x,
        baseline_y: fragment.baseline_y,
        old_width: adjustment.old_width,
        new_width: adjustment.new_width,
        font_size: fragment.style.font_size_points(),
    })
}

fn adjust_late_marker_decorations(
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

fn character_border_horizontal_line_matches(
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

fn adjust_character_border_horizontal_line(
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

fn character_border_right_line_matches(
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

fn shift_late_marker_following_items(
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

fn item_starts_after_late_marker_on_line(
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
        _ => false,
    }
}

fn same_late_marker_line(baseline_y: f32, geometry: LateMarkerGeometry) -> bool {
    (baseline_y - geometry.baseline_y).abs() < 0.01
}

fn starts_new_text_line(item: &LayoutItem, geometry: LateMarkerGeometry) -> bool {
    matches!(
        item,
        LayoutItem::Text(fragment) if !same_late_marker_line(fragment.baseline_y, geometry)
    )
}

fn shift_layout_item_x(item: &mut LayoutItem, delta: f32) {
    match item {
        LayoutItem::Text(fragment) => fragment.x += delta,
        LayoutItem::Highlight { x, .. } | LayoutItem::Underline { x, .. } => *x += delta,
        LayoutItem::Line { x1, x2, .. } => {
            *x1 += delta;
            *x2 += delta;
        }
        _ => {}
    }
}

fn same_pdf_coord(left: f32, right: f32) -> bool {
    (left - right).abs() < 0.01
}

fn same_pdf_length(left: f32, right: f32) -> bool {
    (left - right).abs() < 0.01
}

fn resolve_bookmark_page_ref_markers(
    pages: &mut [LayoutPage],
    document: &Document,
    font_provider: Option<&FontProvider>,
) {
    let mut page_for_bookmark = Vec::<(usize, String)>::new();
    for page in pages.iter() {
        for item in &page.items {
            if let LayoutItem::Text(fragment) = item
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
            if let LayoutItem::Text(fragment) = item {
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
                if let LayoutItem::Text(fragment) = item
                    && (fragment.baseline_y - baseline_y).abs() < 0.01
                    && fragment.x >= marker_x
                {
                    fragment.x += delta;
                }
            }
        }
    }
}

fn next_bookmark_page_marker_range(text: &str, start: usize) -> Option<(usize, usize)> {
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

fn parse_bookmark_page_marker_id(text: &str, prefix: &str) -> Option<usize> {
    let rest = text.strip_prefix(prefix)?;
    let id = rest.strip_suffix(BOOKMARK_PAGE_MARKER_END)?;
    (!id.is_empty() && id.chars().all(|ch| ch.is_ascii_digit()))
        .then(|| id.parse::<usize>().ok())?
}

fn push_text_segments_preserving_tabs(
    output: &mut Vec<FlowRun>,
    text: &str,
    style: &CharacterStyle,
    soft_hyphen_after: bool,
) {
    let mut start = 0;
    for (idx, ch) in text.char_indices() {
        if ch == '\t' {
            if start < idx {
                let text = text[start..idx].replace('\u{00ad}', "");
                if !text.is_empty() {
                    push_display_text_segment(output, &text, style, false);
                }
            }
            output.push(FlowRun {
                text: "\t".to_string(),
                style: style.clone(),
                width: 0.0,
                line_height_points: style.font_size_points(),
                tab_leader: TabLeader::None,
                tab_alignment: TabAlignment::Left,
                tab_stop_position: None,
                soft_hyphen_after: false,
            });
            start = idx + ch.len_utf8();
        }
    }
    if start < text.len() {
        let text = text[start..].replace('\u{00ad}', "");
        if text.is_empty() {
            mark_last_soft_hyphen(output);
        } else {
            push_display_text_segment(output, &text, style, soft_hyphen_after);
        }
    } else if soft_hyphen_after {
        mark_last_soft_hyphen(output);
    }
}

fn push_display_text_segment(
    output: &mut Vec<FlowRun>,
    text: &str,
    style: &CharacterStyle,
    soft_hyphen_after: bool,
) {
    if text.contains(BOOKMARK_PAGE_ANCHOR_MARKER) || text.contains(BOOKMARK_PAGE_REF_MARKER) {
        push_display_text_segment_with_bookmark_markers(output, text, style, soft_hyphen_after);
        return;
    }
    push_display_text_segment_without_bookmark_markers(output, text, style, soft_hyphen_after);
}

fn push_display_text_segment_with_bookmark_markers(
    output: &mut Vec<FlowRun>,
    text: &str,
    style: &CharacterStyle,
    soft_hyphen_after: bool,
) {
    let mut cursor = 0;
    let mut marker_seen = false;
    while let Some((start, end)) = next_bookmark_page_marker_range(text, cursor) {
        if start > cursor {
            push_display_text_segment_without_bookmark_markers(
                output,
                &text[cursor..start],
                style,
                false,
            );
        }
        push_flow_run(output, &text[start..end], style, false);
        marker_seen = true;
        cursor = end;
    }
    if cursor < text.len() {
        push_display_text_segment_without_bookmark_markers(
            output,
            &text[cursor..],
            style,
            soft_hyphen_after,
        );
    } else if soft_hyphen_after && !marker_seen {
        mark_last_soft_hyphen(output);
    }
}

fn push_display_text_segment_without_bookmark_markers(
    output: &mut Vec<FlowRun>,
    text: &str,
    style: &CharacterStyle,
    soft_hyphen_after: bool,
) {
    if style.small_caps && !style.all_caps {
        push_small_caps_segments(output, text, style, soft_hyphen_after);
        return;
    }

    push_passive_font_fallback_segments(output, text, style, soft_hyphen_after);
}

fn push_passive_font_fallback_segments(
    output: &mut Vec<FlowRun>,
    text: &str,
    style: &CharacterStyle,
    soft_hyphen_after: bool,
) {
    let mut start = 0;
    let mut current_kind: Option<PassiveFontRunKind> = None;
    let mut segments = Vec::new();

    for (idx, ch) in text.char_indices() {
        let kind = PassiveFontRunKind::for_char(ch);
        match current_kind {
            None => current_kind = Some(kind),
            Some(previous) if previous != kind => {
                segments.push((start, idx));
                start = idx;
                current_kind = Some(kind);
            }
            Some(_) => {}
        }
    }

    if start < text.len() {
        segments.push((start, text.len()));
    }

    if segments.is_empty() {
        return;
    }

    let last_index = segments.len().saturating_sub(1);
    for (idx, (start, end)) in segments.into_iter().enumerate() {
        push_flow_run(
            output,
            &text[start..end],
            style,
            soft_hyphen_after && idx == last_index,
        );
    }
}

fn push_flow_run(
    output: &mut Vec<FlowRun>,
    text: &str,
    style: &CharacterStyle,
    soft_hyphen_after: bool,
) {
    output.push(FlowRun {
        text: text.to_string(),
        style: style.clone(),
        width: 0.0,
        line_height_points: style.font_size_points(),
        tab_leader: TabLeader::None,
        tab_alignment: TabAlignment::Left,
        tab_stop_position: None,
        soft_hyphen_after,
    });
}

fn push_small_caps_segments(
    output: &mut Vec<FlowRun>,
    text: &str,
    style: &CharacterStyle,
    soft_hyphen_after: bool,
) {
    let mut start = 0;
    let mut current_lowercase = None;
    for (idx, ch) in text.char_indices() {
        let is_lowercase = ch.is_lowercase();
        match current_lowercase {
            None => current_lowercase = Some(is_lowercase),
            Some(previous) if previous != is_lowercase => {
                push_small_caps_segment(output, &text[start..idx], style, previous, false);
                start = idx;
                current_lowercase = Some(is_lowercase);
            }
            Some(_) => {}
        }
    }

    if start < text.len() {
        push_small_caps_segment(
            output,
            &text[start..],
            style,
            current_lowercase.unwrap_or(false),
            soft_hyphen_after,
        );
    }
}

fn push_small_caps_segment(
    output: &mut Vec<FlowRun>,
    text: &str,
    style: &CharacterStyle,
    lowercase: bool,
    soft_hyphen_after: bool,
) {
    let mut segment_style = style.clone();
    if lowercase {
        segment_style.font_size_half_points =
            scaled_small_caps_font_size(style.font_size_half_points);
    }
    output.push(FlowRun {
        text: text.to_string(),
        style: segment_style,
        width: 0.0,
        line_height_points: style.font_size_points(),
        tab_leader: TabLeader::None,
        tab_alignment: TabAlignment::Left,
        tab_stop_position: None,
        soft_hyphen_after,
    });
}

fn scaled_small_caps_font_size(font_size_half_points: i32) -> i32 {
    ((font_size_half_points.max(2) as f32) * SMALL_CAPS_FONT_SCALE)
        .round()
        .max(2.0)
        .min(font_size_half_points.max(2) as f32) as i32
}

fn mark_last_soft_hyphen(output: &mut [FlowRun]) {
    if let Some(last) = output
        .iter_mut()
        .rev()
        .find(|run| run.text != "\t" && run.text != "\n")
    {
        last.soft_hyphen_after = true;
    }
}

fn empty_line() -> Line {
    Line {
        runs: Vec::new(),
        width: 0.0,
        height: 14.0,
    }
}

fn push_segment(
    line: &mut Line,
    mut segment: FlowRun,
    paragraph_style: &ParagraphStyle,
    document: &Document,
    font_provider: Option<&FontProvider>,
) {
    if segment.text == "\t" {
        segment.tab_leader = next_tab_leader(line.width, paragraph_style);
        segment.tab_alignment = next_tab_alignment(line.width, paragraph_style);
        segment.tab_stop_position = Some(next_tab_position(line.width, paragraph_style, document));
    }
    segment.width = measure_flow_run(
        &segment,
        line.width,
        paragraph_style,
        document,
        font_provider,
    );
    if segment.text != "\t" {
        adjust_pending_aligned_tab(line, &segment, document, font_provider);
    }
    line.width += segment.width;
    line.height = line
        .height
        .max(flow_run_line_height(&segment, document, font_provider));
    line.runs.push(segment);
}

fn adjust_pending_aligned_tab(
    line: &mut Line,
    following: &FlowRun,
    document: &Document,
    font_provider: Option<&FontProvider>,
) {
    let Some(tab) = line.runs.last_mut() else {
        return;
    };
    if tab.text != "\t" || matches!(tab.tab_alignment, TabAlignment::Left | TabAlignment::Bar) {
        return;
    }
    let Some(target) = tab.tab_stop_position else {
        return;
    };
    let width_before_tab = line.width - tab.width;
    let desired_start = match tab.tab_alignment {
        TabAlignment::Left => target,
        TabAlignment::Center => target - (following.width / 2.0),
        TabAlignment::Right => target - following.width,
        TabAlignment::Decimal => decimal_tab_start(target, following, document, font_provider)
            .unwrap_or(target - following.width),
        TabAlignment::Bar => target,
    };
    let adjusted_width = (desired_start - width_before_tab).max(0.0);
    tab.width = adjusted_width;
    line.width = width_before_tab + adjusted_width;
}

fn decimal_tab_start(
    target: f32,
    following: &FlowRun,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> Option<f32> {
    let style = passive_pdf_style_for_run(document, &following.style);
    let text = display_text(&following.text, &style);
    let decimal_idx = text.find(['.', ','])?;
    let prefix = &text[..decimal_idx];
    Some(
        target
            - measure_text_with_document_font(
                prefix,
                &style,
                font_family_for_run_text(document, &style, prefix),
                document,
                font_provider,
            ),
    )
}

fn materialize_line_end_soft_hyphen(
    line: &mut Line,
    document: &Document,
    font_provider: Option<&FontProvider>,
) {
    let Some(last) = line
        .runs
        .iter_mut()
        .rev()
        .find(|run| run.text != "\t" && run.text != "\n")
    else {
        return;
    };
    if !last.soft_hyphen_after {
        return;
    }
    last.text.push('-');
    last.soft_hyphen_after = false;
    let style = passive_pdf_style_for_run(document, &last.style);
    let added_width = measure_text_with_document_font(
        "-",
        &style,
        font_family_for_style(document, &style),
        document,
        font_provider,
    );
    last.width += added_width;
    line.width += added_width;
    line.height = line
        .height
        .max(flow_run_line_height(last, document, font_provider));
}

fn flow_run_line_height(
    run: &FlowRun,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> f32 {
    let fallback = fallback_flow_run_line_height(run);
    let Some(provider) = font_provider else {
        return fallback;
    };
    let Some(font) = font_for_style(document, &run.style) else {
        return fallback;
    };
    let metric_char = run
        .text
        .chars()
        .find(|ch| !ch.is_control() && !ch.is_whitespace())
        .unwrap_or('A');
    let Some(line_height) = supplied_font_glyph_metrics(provider, font, &run.style, metric_char)
        .or_else(|| supplied_font_glyph_metrics(provider, font, &run.style, 'A'))
        .and_then(|metrics| metrics.line_height_points(run.line_height_points))
    else {
        return fallback;
    };
    let bounded_line_height = line_height.clamp(
        run.line_height_points.max(1.0),
        run.line_height_points.max(1.0) * 2.0,
    );
    bounded_line_height + run.style.baseline_shift_points().abs()
}

fn fallback_flow_run_line_height(run: &FlowRun) -> f32 {
    (run.line_height_points * 1.25) + run.style.baseline_shift_points().abs()
}

fn measure_flow_run(
    run: &FlowRun,
    current_line_width: f32,
    paragraph_style: &ParagraphStyle,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> f32 {
    if parse_bookmark_page_marker_id(&run.text, BOOKMARK_PAGE_ANCHOR_MARKER).is_some()
        || parse_bookmark_page_marker_id(&run.text, BOOKMARK_PAGE_REF_MARKER).is_some()
    {
        return 0.0;
    }
    if run.text == "\t" {
        return next_tab_position(current_line_width, paragraph_style, document)
            - current_line_width;
    }
    let style = passive_pdf_style_for_run(document, &run.style);
    let text = display_text(&run.text, &style);
    let measurement_text = late_page_count_measurement_text(&text);
    let font_family = font_family_for_run_text(document, &style, &text);
    measure_text_with_document_font(
        &measurement_text,
        &style,
        font_family,
        document,
        font_provider,
    )
}

fn next_tab_position(
    current_line_width: f32,
    paragraph_style: &ParagraphStyle,
    document: &Document,
) -> f32 {
    for (idx, stop_twips) in paragraph_style.tab_stops_twips.iter().enumerate() {
        if paragraph_style
            .tab_stop_alignments
            .get(idx)
            .is_some_and(|alignment| *alignment == TabAlignment::Bar)
        {
            continue;
        }
        let stop = twips_to_points(*stop_twips);
        if stop > current_line_width + 0.01 {
            return stop;
        }
    }

    let default_stop = twips_to_points(document.default_tab_width_twips.max(1));
    (((current_line_width / default_stop).floor() + 1.0) * default_stop).max(default_stop)
}

fn next_tab_leader(current_line_width: f32, paragraph_style: &ParagraphStyle) -> TabLeader {
    for (idx, stop_twips) in paragraph_style.tab_stops_twips.iter().enumerate() {
        if paragraph_style
            .tab_stop_alignments
            .get(idx)
            .is_some_and(|alignment| *alignment == TabAlignment::Bar)
        {
            continue;
        }
        let stop = twips_to_points(*stop_twips);
        if stop > current_line_width + 0.01 {
            return paragraph_style
                .tab_stop_leaders
                .get(idx)
                .copied()
                .unwrap_or_default();
        }
    }
    TabLeader::None
}

fn next_tab_alignment(current_line_width: f32, paragraph_style: &ParagraphStyle) -> TabAlignment {
    for (idx, stop_twips) in paragraph_style.tab_stops_twips.iter().enumerate() {
        if paragraph_style
            .tab_stop_alignments
            .get(idx)
            .is_some_and(|alignment| *alignment == TabAlignment::Bar)
        {
            continue;
        }
        let stop = twips_to_points(*stop_twips);
        if stop > current_line_width + 0.01 {
            return paragraph_style
                .tab_stop_alignments
                .get(idx)
                .copied()
                .unwrap_or_default();
        }
    }
    TabAlignment::Left
}

fn apply_line_spacing(line_height: f32, style: &ParagraphStyle) -> f32 {
    let Some(spacing_twips) = style.line_spacing_twips else {
        return line_height;
    };
    if spacing_twips == 0 {
        return line_height;
    }

    if style.line_spacing_multiple {
        let multiplier = (spacing_twips.unsigned_abs() as f32 / 240.0).clamp(0.25, 12.0);
        return (line_height * multiplier).max(1.0);
    }

    let spacing_points = twips_to_points(spacing_twips.abs()).max(1.0);
    if spacing_twips < 0 {
        spacing_points
    } else {
        line_height.max(spacing_points)
    }
}

fn apply_line_spacing_with_grid(
    line_height: f32,
    style: &ParagraphStyle,
    geometry: PageGeometry,
) -> f32 {
    let spaced = apply_line_spacing(line_height, style);
    if !style.snap_to_line_grid {
        return spaced;
    }
    let Some(grid_twips) = geometry.text_line_grid_twips else {
        return spaced;
    };
    if grid_twips <= 0 {
        return spaced;
    }
    spaced.max(twips_to_points(grid_twips))
}

fn push_bar_tab_stops(
    pages: &mut [LayoutPage],
    style: &ParagraphStyle,
    line_left: f32,
    top_y: f32,
    line_height: f32,
) {
    let page = pages.last_mut().expect("layout always has a page");
    for (idx, stop_twips) in style.tab_stops_twips.iter().enumerate() {
        if !style
            .tab_stop_alignments
            .get(idx)
            .is_some_and(|alignment| *alignment == TabAlignment::Bar)
        {
            continue;
        }
        let x = line_left + twips_to_points(*stop_twips);
        page.items.push(LayoutItem::Line {
            x1: x,
            y1: top_y,
            x2: x,
            y2: top_y - line_height,
            width: 0.5,
            color: PdfColor {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
            },
            style: LineStyle::Solid,
        });
    }
}

fn push_line(
    pages: &mut [LayoutPage],
    line: &Line,
    x: f32,
    top_y: f32,
    document: &Document,
    word_spacing: f32,
) {
    let baseline_y = top_y - line.height + (line.height * 0.25);
    let mut cursor_x = x;
    let page = pages.last_mut().expect("layout always has a page");

    for run in &line.runs {
        if parse_bookmark_page_marker_id(&run.text, BOOKMARK_PAGE_ANCHOR_MARKER).is_some()
            || parse_bookmark_page_marker_id(&run.text, BOOKMARK_PAGE_REF_MARKER).is_some()
        {
            page.items.push(LayoutItem::Text(TextFragment {
                text: run.text.clone(),
                x: cursor_x,
                baseline_y,
                color: style_color(document, &run.style),
                font_family: font_family_for_style(document, &run.style),
                word_spacing: 0.0,
                style: run.style.clone(),
            }));
            continue;
        }
        let extra_word_spacing = word_spacing * regular_space_count(&run.text) as f32;
        let width = run.width + extra_word_spacing;
        if run.text == "\t" {
            push_tab_leader(page, cursor_x, baseline_y, width, run, document);
            cursor_x += width;
            continue;
        }
        let style = passive_pdf_style_for_run(document, &run.style);
        let text = display_text(&run.text, &style);
        let font_family = font_family_for_run_text(document, &style, &text);
        let run_baseline_y = baseline_y + style.baseline_shift_points();
        let color = style_color(document, &style);
        if let Some(highlight_index) = style.highlight_index
            && highlight_index > 0
        {
            let font_size = style.font_size_points();
            page.items.push(LayoutItem::Highlight {
                x: cursor_x,
                y: run_baseline_y - (font_size * 0.28),
                width,
                height: font_size * 1.12,
                color: character_shading_color(document, highlight_index, &style),
            });
        }
        if style.form_field_shading {
            let font_size = style.font_size_points();
            let (shade_x, shade_width) =
                passive_form_field_shading_horizontal_bounds(cursor_x, width, font_size);
            page.items.push(LayoutItem::Highlight {
                x: shade_x,
                y: run_baseline_y - (font_size * 0.32),
                width: shade_width,
                height: font_size * 1.2,
                color: passive_form_field_shading_color(),
            });
        }
        if style.border.visible {
            push_character_border(
                page,
                cursor_x,
                run_baseline_y,
                width,
                &style,
                color,
                document,
            );
        }
        page.items.push(LayoutItem::Text(TextFragment {
            text: text.clone(),
            x: cursor_x,
            baseline_y: run_baseline_y,
            color,
            font_family,
            word_spacing,
            style: style.clone(),
        }));

        if style.overline {
            let y = run_baseline_y + (style.font_size_points() * 0.78);
            page.items.push(LayoutItem::Line {
                x1: cursor_x,
                y1: y,
                x2: cursor_x + width,
                y2: y,
                width: 0.5,
                color,
                style: LineStyle::Solid,
            });
        }

        if style.underline != UnderlineStyle::None {
            let underline_color = style
                .underline_color_index
                .filter(|index| *index > 0)
                .map(|index| color_for_index(document, index))
                .unwrap_or(color);
            push_underline_items(
                page,
                cursor_x,
                run_baseline_y - 2.0,
                width,
                &text,
                &style,
                font_family,
                underline_color,
                word_spacing,
            );
        }

        if style.strike {
            let y = run_baseline_y + (style.font_size_points() * 0.32);
            if style.double_strike {
                let gap = (style.font_size_points() * 0.1).clamp(1.0, 3.0);
                page.items.push(LayoutItem::Line {
                    x1: cursor_x,
                    y1: y + gap,
                    x2: cursor_x + width,
                    y2: y + gap,
                    width: 0.5,
                    color,
                    style: LineStyle::Solid,
                });
                page.items.push(LayoutItem::Line {
                    x1: cursor_x,
                    y1: y - gap,
                    x2: cursor_x + width,
                    y2: y - gap,
                    width: 0.5,
                    color,
                    style: LineStyle::Solid,
                });
            } else {
                page.items.push(LayoutItem::Line {
                    x1: cursor_x,
                    y1: y,
                    x2: cursor_x + width,
                    y2: y,
                    width: 0.5,
                    color,
                    style: LineStyle::Solid,
                });
            }
        }

        cursor_x += width;
    }
}

#[allow(clippy::too_many_arguments)]
fn push_underline_items(
    page: &mut LayoutPage,
    x: f32,
    y: f32,
    width: f32,
    text: &str,
    style: &CharacterStyle,
    font_family: PdfFontFamily,
    color: PdfColor,
    word_spacing: f32,
) {
    if style.underline != UnderlineStyle::Words {
        page.items.push(LayoutItem::Underline {
            x,
            y,
            width,
            color,
            style: style.underline,
        });
        return;
    }

    for (offset, span_width) in word_underline_spans(text, style, font_family, word_spacing) {
        page.items.push(LayoutItem::Underline {
            x: x + offset,
            y,
            width: span_width,
            color,
            style: UnderlineStyle::Single,
        });
    }
}

fn word_underline_spans(
    text: &str,
    style: &CharacterStyle,
    font_family: PdfFontFamily,
    word_spacing: f32,
) -> Vec<(f32, f32)> {
    let mut spans = Vec::new();
    let mut cursor = 0.0;
    let mut word_start = None;
    let mut word = String::new();

    for ch in text.chars() {
        if is_word_underline_gap(ch) {
            if let Some(start) = word_start.take() {
                let width = measure_late_page_count_text_with_family(&word, style, font_family);
                if width > 0.0 {
                    spans.push((start, width));
                }
                cursor += width;
                word.clear();
            }
            cursor += measure_late_page_count_text_with_family(&ch.to_string(), style, font_family);
            if ch == ' ' {
                cursor += word_spacing;
            }
        } else {
            if word_start.is_none() {
                word_start = Some(cursor);
            }
            word.push(ch);
        }
    }

    if let Some(start) = word_start {
        let width = measure_late_page_count_text_with_family(&word, style, font_family);
        if width > 0.0 {
            spans.push((start, width));
        }
    }

    spans
}

fn measure_late_page_count_text_with_family(
    text: &str,
    style: &CharacterStyle,
    font_family: PdfFontFamily,
) -> f32 {
    let text = late_page_count_measurement_text(text);
    measure_text_with_family(&text, style, font_family)
}

fn is_word_underline_gap(ch: char) -> bool {
    ch == '\u{00a0}' || ch.is_whitespace()
}

fn push_tab_leader(
    page: &mut LayoutPage,
    cursor_x: f32,
    baseline_y: f32,
    width: f32,
    run: &FlowRun,
    document: &Document,
) {
    if width <= 1.0 || run.tab_leader == TabLeader::None {
        return;
    }
    let style = passive_pdf_style_for_run(document, &run.style);
    let color = style_color(document, &style);
    match run.tab_leader {
        TabLeader::None => {}
        TabLeader::Underline => {
            page.items.push(LayoutItem::Line {
                x1: cursor_x,
                y1: baseline_y - 2.0,
                x2: cursor_x + width,
                y2: baseline_y - 2.0,
                width: 0.5,
                color,
                style: LineStyle::Solid,
            });
        }
        TabLeader::Dots | TabLeader::Hyphens | TabLeader::MiddleDots | TabLeader::Equals => {
            let leader = match run.tab_leader {
                TabLeader::Dots => ".",
                TabLeader::Hyphens => "-",
                TabLeader::MiddleDots => "\u{00b7}",
                TabLeader::Equals => "=",
                TabLeader::None | TabLeader::Underline => unreachable!("handled above"),
            };
            let family = font_family_for_style(document, &style);
            let leader_width = measure_text_with_family(leader, &style, family).max(1.0);
            let count = (width / leader_width).floor().max(1.0) as usize;
            page.items.push(LayoutItem::Text(TextFragment {
                text: leader.repeat(count.min(512)),
                x: cursor_x,
                baseline_y: baseline_y + style.baseline_shift_points(),
                color,
                font_family: family,
                word_spacing: 0.0,
                style,
            }));
        }
    }
}

fn push_character_border(
    page: &mut LayoutPage,
    x: f32,
    baseline_y: f32,
    text_width: f32,
    style: &CharacterStyle,
    fallback_color: PdfColor,
    document: &Document,
) {
    let (stroke_width, color, line_style) =
        character_border_stroke(&style.border, fallback_color, document);
    let pad = 1.5 + (stroke_width * 0.5) + twips_to_points(style.border.spacing_twips.max(0));
    let left = x - pad;
    let right = x + text_width + pad;
    let bottom = baseline_y - (style.font_size_points() * 0.28) - pad;
    let top = baseline_y + (style.font_size_points() * 0.82) + pad;

    page.items.push(LayoutItem::Line {
        x1: left,
        y1: top,
        x2: right,
        y2: top,
        width: stroke_width,
        color,
        style: line_style,
    });
    page.items.push(LayoutItem::Line {
        x1: right,
        y1: top,
        x2: right,
        y2: bottom,
        width: stroke_width,
        color,
        style: line_style,
    });
    page.items.push(LayoutItem::Line {
        x1: right,
        y1: bottom,
        x2: left,
        y2: bottom,
        width: stroke_width,
        color,
        style: line_style,
    });
    page.items.push(LayoutItem::Line {
        x1: left,
        y1: bottom,
        x2: left,
        y2: top,
        width: stroke_width,
        color,
        style: line_style,
    });
}

fn character_border_stroke(
    border: &TableCellBorder,
    fallback_color: PdfColor,
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
        .unwrap_or(fallback_color);
    let style = line_style_for_border_style(border.style);
    (width, color, style)
}

fn line_style_for_border_style(style: BorderStyle) -> LineStyle {
    match style {
        BorderStyle::Single | BorderStyle::Thick | BorderStyle::Hairline => LineStyle::Solid,
        BorderStyle::Double => LineStyle::Double,
        BorderStyle::Dotted => LineStyle::Dotted,
        BorderStyle::Dashed => LineStyle::Dashed,
        BorderStyle::Wavy => LineStyle::Wavy,
    }
}

fn justified_word_spacing(
    line: &Line,
    style: &ParagraphStyle,
    available_width: f32,
    is_last_line: bool,
) -> f32 {
    if style.alignment != Alignment::Justified || is_last_line || line.width <= 0.0 {
        return 0.0;
    }
    let space_count = line
        .runs
        .iter()
        .map(|run| regular_space_count(&run.text))
        .sum::<usize>();
    if space_count == 0 {
        return 0.0;
    }
    ((available_width - line.width) / space_count as f32).max(0.0)
}

fn regular_space_count(text: &str) -> usize {
    text.chars().filter(|ch| *ch == ' ').count()
}

fn display_text(text: &str, style: &CharacterStyle) -> String {
    if style.hidden {
        return String::new();
    }
    let text = text
        .chars()
        .filter(|ch| !is_zero_width_format_char(*ch))
        .collect::<String>();
    if style.all_caps || style.small_caps {
        text.chars().flat_map(char::to_uppercase).collect()
    } else {
        text
    }
}

fn font_family_for_run_text(
    document: &Document,
    style: &CharacterStyle,
    text: &str,
) -> PdfFontFamily {
    if is_passive_bullet_text(text) {
        return PdfFontFamily::Helvetica;
    }
    if is_passive_checkbox_text(text) {
        return PdfFontFamily::ZapfDingbats;
    }
    if is_passive_symbol_text(text) {
        return PdfFontFamily::Symbol;
    }

    font_family_for_style(document, style)
}

fn is_passive_bullet_text(text: &str) -> bool {
    let mut has_bullet = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            continue;
        }
        if ch == '\u{2022}' {
            has_bullet = true;
        } else {
            return false;
        }
    }
    has_bullet
}

fn font_family_for_style(document: &Document, style: &CharacterStyle) -> PdfFontFamily {
    let Some(font) = font_for_style(document, style) else {
        return PdfFontFamily::Helvetica;
    };
    passive_pdf_font_family_for_font(font)
}

fn font_for_style<'a>(document: &'a Document, style: &CharacterStyle) -> Option<&'a FontDef> {
    document
        .fonts
        .iter()
        .find(|font| font.index == style.font_index)
}

pub(crate) fn passive_pdf_font_family_for_font(font: &FontDef) -> PdfFontFamily {
    if let Some(family) = font_family_from_name(&font.name) {
        family
    } else if let Some(family) = font
        .alternate_name
        .as_deref()
        .and_then(font_family_from_name)
    {
        family
    } else if font.charset == Some(2) {
        PdfFontFamily::Symbol
    } else if font.pitch == FontPitch::Fixed {
        PdfFontFamily::Courier
    } else if font.family == FontFamilyHint::Modern {
        PdfFontFamily::Courier
    } else if font.family == FontFamilyHint::Roman {
        PdfFontFamily::Times
    } else if font.family == FontFamilyHint::Swiss {
        PdfFontFamily::Helvetica
    } else {
        PdfFontFamily::Helvetica
    }
}

fn passive_pdf_style_for_run(document: &Document, style: &CharacterStyle) -> CharacterStyle {
    let scale_percent = passive_source_font_width_scale_percent(document, style);
    if scale_percent == 100 {
        return style.clone();
    }
    let mut output = style.clone();
    let scaled = i64::from(output.character_scaling_percent.max(1))
        .saturating_mul(i64::from(scale_percent))
        / 100;
    output.character_scaling_percent = scaled.clamp(1, 600) as i32;
    output
}

fn passive_source_font_width_scale_percent(document: &Document, style: &CharacterStyle) -> i32 {
    let Some(font) = document
        .fonts
        .iter()
        .find(|font| font.index == style.font_index)
    else {
        return 100;
    };
    if is_passive_narrow_font_name(&font.name)
        || font
            .alternate_name
            .as_deref()
            .is_some_and(is_passive_narrow_font_name)
    {
        PASSIVE_NARROW_FONT_SCALE_PERCENT
    } else {
        100
    }
}

fn is_passive_narrow_font_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    font_name_contains_any(&lower, &["narrow", "condensed", "compressed"])
}

fn is_passive_checkbox_text(text: &str) -> bool {
    let mut has_checkbox = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            continue;
        }
        if matches!(
            ch,
            '\u{25a1}'
                | '\u{2610}'
                | '\u{2611}'
                | '\u{2612}'
                | '\u{263a}'
                | '\u{2713}'
                | '\u{2714}'
                | '\u{2717}'
                | '\u{2751}'
        ) {
            has_checkbox = true;
        } else {
            return false;
        }
    }
    has_checkbox
}

fn is_passive_symbol_text(text: &str) -> bool {
    let mut has_symbol = false;
    for ch in text.chars() {
        if is_symbol_fallback_required_char(ch) {
            has_symbol = true;
        } else {
            return false;
        }
    }
    has_symbol
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum PassiveFontRunKind {
    Normal,
    Symbol,
    Checkbox,
}

impl PassiveFontRunKind {
    fn for_char(ch: char) -> Self {
        if is_passive_checkbox_char(ch) {
            Self::Checkbox
        } else if is_symbol_fallback_required_char(ch) {
            Self::Symbol
        } else {
            Self::Normal
        }
    }
}

fn is_passive_checkbox_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{25a1}'
            | '\u{2610}'
            | '\u{2611}'
            | '\u{2612}'
            | '\u{263a}'
            | '\u{2713}'
            | '\u{2714}'
            | '\u{2717}'
            | '\u{2751}'
    )
}

fn is_symbol_fallback_required_char(ch: char) -> bool {
    is_passive_symbol_fallback_char(ch) && !is_normal_text_preferred_symbol_overlap_char(ch)
}

fn is_normal_text_preferred_symbol_overlap_char(ch: char) -> bool {
    matches!(ch, '\u{2026}')
}

fn is_passive_symbol_fallback_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{0391}'
            | '\u{0392}'
            | '\u{0393}'
            | '\u{0394}'
            | '\u{0395}'
            | '\u{0396}'
            | '\u{0397}'
            | '\u{0398}'
            | '\u{0399}'
            | '\u{039a}'
            | '\u{039b}'
            | '\u{039c}'
            | '\u{039d}'
            | '\u{039e}'
            | '\u{039f}'
            | '\u{03a0}'
            | '\u{03a1}'
            | '\u{03a3}'
            | '\u{03a4}'
            | '\u{03a5}'
            | '\u{03a6}'
            | '\u{03a7}'
            | '\u{03a8}'
            | '\u{03a9}'
            | '\u{03b1}'
            | '\u{03b2}'
            | '\u{03b3}'
            | '\u{03b4}'
            | '\u{03b5}'
            | '\u{03b6}'
            | '\u{03b7}'
            | '\u{03b8}'
            | '\u{03b9}'
            | '\u{03ba}'
            | '\u{03bb}'
            | '\u{03bc}'
            | '\u{03bd}'
            | '\u{03be}'
            | '\u{03bf}'
            | '\u{03c0}'
            | '\u{03c1}'
            | '\u{03c3}'
            | '\u{03c4}'
            | '\u{03c5}'
            | '\u{03c6}'
            | '\u{03c7}'
            | '\u{03c8}'
            | '\u{03c9}'
            | '\u{03d5}'
            | '\u{03d6}'
            | '\u{03d1}'
            | '\u{03d2}'
            | '\u{00b0}'
            | '\u{00b1}'
            | '\u{00b5}'
            | '\u{00a9}'
            | '\u{00ae}'
            | '\u{00d7}'
            | '\u{00f7}'
            | '\u{0192}'
            | '\u{20ac}'
            | '\u{2026}'
            | '\u{2032}'
            | '\u{2033}'
            | '\u{2044}'
            | '\u{2111}'
            | '\u{211c}'
            | '\u{2118}'
            | '\u{2126}'
            | '\u{2122}'
            | '\u{2135}'
            | '\u{2190}'
            | '\u{2191}'
            | '\u{2192}'
            | '\u{2193}'
            | '\u{2194}'
            | '\u{21b5}'
            | '\u{21d0}'
            | '\u{21d1}'
            | '\u{21d2}'
            | '\u{21d3}'
            | '\u{21d4}'
            | '\u{00ac}'
            | '\u{2200}'
            | '\u{2202}'
            | '\u{2203}'
            | '\u{2205}'
            | '\u{2206}'
            | '\u{2207}'
            | '\u{2208}'
            | '\u{2209}'
            | '\u{220b}'
            | '\u{220f}'
            | '\u{2211}'
            | '\u{2212}'
            | '\u{2215}'
            | '\u{2217}'
            | '\u{221a}'
            | '\u{221d}'
            | '\u{221e}'
            | '\u{222b}'
            | '\u{2220}'
            | '\u{2227}'
            | '\u{2228}'
            | '\u{2229}'
            | '\u{222a}'
            | '\u{223c}'
            | '\u{2234}'
            | '\u{2248}'
            | '\u{2260}'
            | '\u{2261}'
            | '\u{2264}'
            | '\u{2265}'
            | '\u{22a5}'
            | '\u{2282}'
            | '\u{2283}'
            | '\u{2284}'
            | '\u{2286}'
            | '\u{2287}'
            | '\u{2295}'
            | '\u{2297}'
            | '\u{22c5}'
            | '\u{2320}'
            | '\u{2321}'
            | '\u{2329}'
            | '\u{232a}'
            | '\u{239b}'
            | '\u{239c}'
            | '\u{239d}'
            | '\u{239e}'
            | '\u{239f}'
            | '\u{23a0}'
            | '\u{23a1}'
            | '\u{23a2}'
            | '\u{23a3}'
            | '\u{23a4}'
            | '\u{23a5}'
            | '\u{23a6}'
            | '\u{23a7}'
            | '\u{23a8}'
            | '\u{23a9}'
            | '\u{23aa}'
            | '\u{23ab}'
            | '\u{23ac}'
            | '\u{23ad}'
            | '\u{23ae}'
            | '\u{23af}'
            | '\u{23d0}'
            | '\u{25ca}'
            | '\u{2660}'
            | '\u{2663}'
            | '\u{2665}'
            | '\u{2666}'
    )
}

fn font_family_from_name(name: &str) -> Option<PdfFontFamily> {
    let name = name.to_ascii_lowercase();
    if font_name_contains_any(
        &name,
        &[
            "courier",
            "consolas",
            "monaco",
            "lucida console",
            "aptos mono",
            "cascadia mono",
            "cascadia code",
            "monospace",
        ],
    ) {
        Some(PdfFontFamily::Courier)
    } else if font_name_contains_any(
        &name,
        &["zapfdingbats", "zapf dingbats", "wingdings", "webdings"],
    ) {
        Some(PdfFontFamily::ZapfDingbats)
    } else if is_pdf_symbol_font_name(&name) {
        Some(PdfFontFamily::Symbol)
    } else if is_sans_serif_font_name(&name) {
        Some(PdfFontFamily::Helvetica)
    } else if font_name_contains_any(
        &name,
        &[
            "times",
            "roman",
            "serif",
            "cambria",
            "georgia",
            "garamond",
            "palatino",
            "book antiqua",
            "constantia",
        ],
    ) {
        Some(PdfFontFamily::Times)
    } else if font_name_contains_any(
        &name,
        &[
            "arial",
            "helvetica",
            "calibri",
            "aptos",
            "segoe ui",
            "tahoma",
            "verdana",
            "trebuchet",
            "franklin gothic",
        ],
    ) {
        Some(PdfFontFamily::Helvetica)
    } else {
        None
    }
}

fn font_name_contains_any(name: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| name.contains(needle))
}

fn is_sans_serif_font_name(name: &str) -> bool {
    font_name_contains_any(name, &["sans serif", "sans-serif", "microsoft sans serif"])
}

fn is_pdf_symbol_font_name(name: &str) -> bool {
    matches!(
        name.trim(),
        "symbol" | "symbol mt" | "symbolmt" | "standard symbols l"
    )
}

fn style_color(document: &Document, style: &CharacterStyle) -> PdfColor {
    color_for_index(document, style.color_index)
}

fn color_for_index(document: &Document, index: usize) -> PdfColor {
    document
        .colors
        .get(index)
        .map(|color| PdfColor {
            red: color.red as f32 / 255.0,
            green: color.green as f32 / 255.0,
            blue: color.blue as f32 / 255.0,
        })
        .unwrap_or_default()
}

fn character_shading_color(document: &Document, index: usize, style: &CharacterStyle) -> PdfColor {
    shading_color(document, index, style.highlight_shading_basis_points)
}

fn passive_form_field_shading_color() -> PdfColor {
    PdfColor {
        red: 0.82,
        green: 0.82,
        blue: 0.82,
    }
}

fn passive_form_field_shading_horizontal_bounds(x: f32, width: f32, font_size: f32) -> (f32, f32) {
    let padding = (font_size * 0.16).clamp(1.0, 3.0);
    let shaded_x = (x - padding).max(0.0);
    let left_padding = x - shaded_x;
    (shaded_x, width.max(0.0) + left_padding + padding)
}

fn shading_color(document: &Document, index: usize, basis_points: i32) -> PdfColor {
    let color = color_for_index(document, index);
    let factor = (basis_points as f32 / 10_000.0).clamp(0.0, 1.0);
    PdfColor {
        red: 1.0 - ((1.0 - color.red) * factor),
        green: 1.0 - ((1.0 - color.green) * factor),
        blue: 1.0 - ((1.0 - color.blue) * factor),
    }
}

#[allow(clippy::too_many_arguments)]
fn push_shading_rect(
    pages: &mut [LayoutPage],
    document: &Document,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color_index: usize,
    basis_points: i32,
    pattern: ShadingPattern,
) {
    let fill_color = shading_color(document, color_index, basis_points);
    let line_color = color_for_index(document, color_index);
    let page = pages.last_mut().expect("layout always has a page");
    page.items.push(LayoutItem::Highlight {
        x,
        y,
        width,
        height,
        color: fill_color,
    });
    push_shading_pattern_lines(page, x, y, width, height, line_color, pattern);
}

fn push_shading_pattern_lines(
    page: &mut LayoutPage,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: PdfColor,
    pattern: ShadingPattern,
) {
    if pattern == ShadingPattern::None || width <= 0.5 || height <= 0.5 {
        return;
    }

    let spacing = shading_pattern_spacing(pattern);
    let stroke_width = 0.35;
    match pattern {
        ShadingPattern::Horizontal | ShadingPattern::DarkHorizontal => {
            push_horizontal_shading_lines(page, x, y, width, height, spacing, stroke_width, color);
        }
        ShadingPattern::Vertical | ShadingPattern::DarkVertical => {
            push_vertical_shading_lines(page, x, y, width, height, spacing, stroke_width, color);
        }
        ShadingPattern::ForwardDiagonal | ShadingPattern::DarkForwardDiagonal => {
            push_forward_diagonal_shading_lines(
                page,
                x,
                y,
                width,
                height,
                spacing,
                stroke_width,
                color,
            );
        }
        ShadingPattern::BackwardDiagonal | ShadingPattern::DarkBackwardDiagonal => {
            push_backward_diagonal_shading_lines(
                page,
                x,
                y,
                width,
                height,
                spacing,
                stroke_width,
                color,
            );
        }
        ShadingPattern::Cross | ShadingPattern::DarkCross => {
            push_horizontal_shading_lines(page, x, y, width, height, spacing, stroke_width, color);
            push_vertical_shading_lines(page, x, y, width, height, spacing, stroke_width, color);
        }
        ShadingPattern::DiagonalCross | ShadingPattern::DarkDiagonalCross => {
            push_forward_diagonal_shading_lines(
                page,
                x,
                y,
                width,
                height,
                spacing,
                stroke_width,
                color,
            );
            push_backward_diagonal_shading_lines(
                page,
                x,
                y,
                width,
                height,
                spacing,
                stroke_width,
                color,
            );
        }
        ShadingPattern::None => {}
    }
}

fn shading_pattern_spacing(pattern: ShadingPattern) -> f32 {
    match pattern {
        ShadingPattern::DarkHorizontal
        | ShadingPattern::DarkVertical
        | ShadingPattern::DarkForwardDiagonal
        | ShadingPattern::DarkBackwardDiagonal
        | ShadingPattern::DarkCross
        | ShadingPattern::DarkDiagonalCross => 2.5,
        ShadingPattern::None
        | ShadingPattern::Horizontal
        | ShadingPattern::Vertical
        | ShadingPattern::ForwardDiagonal
        | ShadingPattern::BackwardDiagonal
        | ShadingPattern::Cross
        | ShadingPattern::DiagonalCross => 4.0,
    }
}

#[allow(clippy::too_many_arguments)]
fn push_horizontal_shading_lines(
    page: &mut LayoutPage,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    spacing: f32,
    stroke_width: f32,
    color: PdfColor,
) {
    let mut cursor = y + spacing;
    let max_y = y + height;
    while cursor < max_y {
        push_shading_line(page, x, cursor, x + width, cursor, stroke_width, color);
        cursor += spacing;
    }
}

#[allow(clippy::too_many_arguments)]
fn push_vertical_shading_lines(
    page: &mut LayoutPage,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    spacing: f32,
    stroke_width: f32,
    color: PdfColor,
) {
    let mut cursor = x + spacing;
    let max_x = x + width;
    while cursor < max_x {
        push_shading_line(page, cursor, y, cursor, y + height, stroke_width, color);
        cursor += spacing;
    }
}

#[allow(clippy::too_many_arguments)]
fn push_forward_diagonal_shading_lines(
    page: &mut LayoutPage,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    spacing: f32,
    stroke_width: f32,
    color: PdfColor,
) {
    let mut offset = spacing;
    while offset < width {
        let length = (width - offset).min(height);
        push_shading_line(
            page,
            x + offset,
            y,
            x + offset + length,
            y + length,
            stroke_width,
            color,
        );
        offset += spacing;
    }

    let mut offset = spacing;
    while offset < height {
        let length = width.min(height - offset);
        push_shading_line(
            page,
            x,
            y + offset,
            x + length,
            y + offset + length,
            stroke_width,
            color,
        );
        offset += spacing;
    }
}

#[allow(clippy::too_many_arguments)]
fn push_backward_diagonal_shading_lines(
    page: &mut LayoutPage,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    spacing: f32,
    stroke_width: f32,
    color: PdfColor,
) {
    let mut offset = spacing;
    while offset < height {
        let length = width.min(offset);
        push_shading_line(
            page,
            x,
            y + offset,
            x + length,
            y + offset - length,
            stroke_width,
            color,
        );
        offset += spacing;
    }

    let mut offset = spacing;
    while offset < width {
        let length = (width - offset).min(height);
        push_shading_line(
            page,
            x + offset,
            y + height,
            x + offset + length,
            y + height - length,
            stroke_width,
            color,
        );
        offset += spacing;
    }
}

fn push_shading_line(
    page: &mut LayoutPage,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    width: f32,
    color: PdfColor,
) {
    page.items.push(LayoutItem::Line {
        x1,
        y1,
        x2,
        y2,
        width,
        color,
        style: LineStyle::Solid,
    });
}

fn aligned_x(
    left: f32,
    content_width: f32,
    line_width: f32,
    style: &ParagraphStyle,
    is_first_line: bool,
) -> f32 {
    let line_left_indent = paragraph_line_left_indent_twips(style, is_first_line);
    let left = left + twips_to_points(line_left_indent);
    let available = paragraph_line_width(content_width, style, is_first_line);
    match style.alignment {
        Alignment::Left | Alignment::Justified => left,
        Alignment::Center => left + ((available - line_width).max(0.0) / 2.0),
        Alignment::Right => left + (available - line_width).max(0.0),
    }
}

fn paragraph_line_width(content_width: f32, style: &ParagraphStyle, is_first_line: bool) -> f32 {
    let line_left_indent = paragraph_line_left_indent_twips(style, is_first_line);
    (content_width - twips_to_points(line_left_indent + style.right_indent_twips)).max(12.0)
}

fn paragraph_line_left_indent_twips(style: &ParagraphStyle, is_first_line: bool) -> i32 {
    style.left_indent_twips
        + if is_first_line {
            style.first_line_indent_twips
        } else {
            0
        }
}

pub fn measure_text(text: &str, style: &CharacterStyle) -> f32 {
    measure_text_with_family(text, style, PdfFontFamily::Helvetica)
}

fn measure_text_with_family(text: &str, style: &CharacterStyle, family: PdfFontFamily) -> f32 {
    let size = style.font_size_points();
    let base_width = text
        .chars()
        .map(|ch| base14_char_width_points(ch, size, family, style))
        .sum::<f32>();
    (base_width + character_spacing_width(text, style) + passive_kerning_width(text, style, family))
        * style.horizontal_scale()
}

fn measure_text_with_document_font(
    text: &str,
    style: &CharacterStyle,
    family: PdfFontFamily,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> f32 {
    let size = style.font_size_points();
    let mut width = 0.0;
    let source_font = font_for_style(document, style);
    for ch in text.chars() {
        if is_zero_width_format_char(ch) {
            continue;
        }
        let font_width = font_provider
            .zip(source_font)
            .filter(|_| !matches!(family, PdfFontFamily::Symbol | PdfFontFamily::ZapfDingbats))
            .and_then(|(provider, font)| supplied_font_glyph_metrics(provider, font, style, ch))
            .map(|metrics| metrics.advance_points(size));
        width += font_width.unwrap_or_else(|| base14_char_width_points(ch, size, family, style));
    }
    (width + character_spacing_width(text, style) + passive_kerning_width(text, style, family))
        * style.horizontal_scale()
}

fn supplied_font_glyph_metrics(
    provider: &FontProvider,
    font: &FontDef,
    style: &CharacterStyle,
    ch: char,
) -> Option<crate::fonts::FontGlyphMetrics> {
    let asset_style = FontAssetStyle {
        bold: style.bold,
        italic: style.italic,
    };
    provider
        .glyph_metrics_for_char_with_style(&font.name, asset_style, ch)
        .or_else(|| {
            font.alternate_name.as_deref().and_then(|alternate| {
                provider.glyph_metrics_for_char_with_style(alternate, asset_style, ch)
            })
        })
}

fn base14_char_width_points(
    ch: char,
    size: f32,
    family: PdfFontFamily,
    style: &CharacterStyle,
) -> f32 {
    if is_zero_width_format_char(ch) {
        return 0.0;
    }

    let units = match family {
        PdfFontFamily::Courier => 600,
        PdfFontFamily::Helvetica if style.bold => helvetica_bold_width_units(ch),
        PdfFontFamily::Helvetica => helvetica_width_units(ch),
        PdfFontFamily::Times if style.bold => times_bold_width_units(ch),
        PdfFontFamily::Times if style.italic => times_italic_width_units(ch),
        PdfFontFamily::Times => times_width_units(ch),
        PdfFontFamily::Symbol | PdfFontFamily::ZapfDingbats => symbol_width_units(ch),
    };
    size * units as f32 / 1000.0
}

fn helvetica_width_units(ch: char) -> u16 {
    match ch {
        ' ' | '\u{00a0}' | '!' | ',' | '.' | ':' | ';' | '[' | '\\' | ']' => 278,
        '"' => 355,
        '#' | '$' | '0'..='9' | '<' | '=' | '>' | '_' | '?' => 556,
        '%' => 889,
        '&' | 'A' | 'B' | 'K' | 'R' | 'S' | 'X' | 'Y' => 667,
        '\'' => 191,
        '(' | ')' | '`' | '-' => 333,
        '*' => 389,
        '+' | '~' => 584,
        '/' | 'I' | 'f' | 't' => 278,
        '@' => 1015,
        'C' | 'H' | 'N' | 'U' | 'w' => 722,
        'D' | 'G' | 'O' => 778,
        'E' | 'P' | 'V' => 667,
        'F' | 'Z' => 611,
        'J' | 'a' | 'b' | 'd' | 'e' | 'g' | 'n' | 'o' | 'p' | 'q' => 556,
        'L' => 556,
        'M' | 'm' => 833,
        'Q' => 778,
        'T' => 611,
        'W' => 944,
        '^' => 469,
        'c' | 'k' | 's' | 'v' | 'x' | 'y' | 'z' => 500,
        'h' | 'u' => 556,
        'i' | 'l' => 222,
        'j' => 222,
        'r' | '{' | '}' => 333,
        '|' => 260,
        '\u{2013}' => 556,
        '\u{2014}' => 1000,
        '\u{2018}' | '\u{2019}' => 222,
        '\u{201c}' | '\u{201d}' => 333,
        '\u{2022}' => 350,
        '\u{2026}' => 1000,
        _ if ch.is_whitespace() => 278,
        _ => 556,
    }
}

fn helvetica_bold_width_units(ch: char) -> u16 {
    match ch {
        ' ' | '\u{00a0}' | ',' | '.' | 'I' | 'i' | 'j' | 'l' | '\\' => 278,
        '!' | ':' | ';' | '(' | ')' | '[' | ']' | '`' | '-' | 'f' | 't' => 333,
        '"' => 474,
        '#'
        | '$'
        | '0'..='9'
        | '<'
        | '='
        | '>'
        | '_'
        | 'J'
        | 'c'
        | 'e'
        | 'k'
        | 's'
        | 'u'
        | 'v'
        | 'x' => 556,
        '%' => 889,
        '&' | 'A' | 'B' | 'C' | 'D' | 'H' | 'K' | 'N' | 'R' | 'S' | 'X' | 'Y' => 722,
        '\'' => 238,
        '*' => 389,
        '+' | '^' | '~' => 584,
        '/' => 278,
        '?' | 'F' | 'L' | 'P' | 'Z' | 'b' | 'd' | 'g' | 'h' | 'n' | 'o' | 'p' | 'q' => 611,
        '@' => 975,
        'E' | 'T' => 667,
        'G' | 'O' | 'Q' | 'w' => 778,
        'M' | 'm' => 833,
        'U' => 722,
        'V' => 667,
        'W' => 944,
        'a' => 556,
        'r' | '{' | '}' => 389,
        'y' => 556,
        'z' => 500,
        '|' => 280,
        '\u{2013}' => 556,
        '\u{2014}' => 1000,
        '\u{2018}' | '\u{2019}' => 278,
        '\u{201c}' | '\u{201d}' => 500,
        '\u{2022}' => 350,
        '\u{2026}' => 1000,
        _ if ch.is_whitespace() => 278,
        _ => 556,
    }
}

fn times_width_units(ch: char) -> u16 {
    match ch {
        ' ' | '\u{00a0}' | ',' | '.' => 250,
        '!' | '(' | ')' | '[' | ']' | '`' | 'f' | 'r' => 333,
        '"' => 408,
        '#'
        | '$'
        | '*'
        | '0'..='9'
        | '_'
        | 'b'
        | 'd'
        | 'g'
        | 'h'
        | 'k'
        | 'n'
        | 'o'
        | 'p'
        | 'q'
        | 'u'
        | 'v' => 500,
        '%' => 833,
        '&' => 778,
        '\'' => 180,
        '+' | '<' | '=' | '>' => 564,
        '-' => 333,
        '/' | ':' | ';' | 'i' | 'j' | 'l' => 278,
        '?' | 'a' | 'c' | 'e' | 'z' => 444,
        '@' => 921,
        'A' | 'D' | 'G' | 'H' | 'K' | 'O' | 'Q' | 'V' | 'X' | 'Y' | 'w' => 722,
        'B' | 'C' | 'R' => 667,
        'E' | 'L' | 'Z' | 'T' => 611,
        'F' | 'P' | 'S' => 556,
        'I' => 333,
        'J' | 's' => 389,
        'M' => 889,
        'N' => 722,
        'U' => 722,
        'W' => 944,
        '\\' => 278,
        '^' => 469,
        'm' => 778,
        't' => 278,
        'x' => 500,
        'y' => 500,
        '{' | '}' => 480,
        '|' => 200,
        '~' => 541,
        '\u{2013}' => 500,
        '\u{2014}' => 1000,
        '\u{2018}' | '\u{2019}' => 333,
        '\u{201c}' | '\u{201d}' => 444,
        '\u{2022}' => 350,
        '\u{2026}' => 1000,
        _ if ch.is_whitespace() => 250,
        _ => 500,
    }
}

fn times_bold_width_units(ch: char) -> u16 {
    match ch {
        ' ' | '\u{00a0}' | ',' | '.' => 250,
        '!' | '(' | ')' | '-' | '[' | ']' | '`' | 'f' | ':' | ';' => 333,
        '"' | 'J' | '?' | '#' | '$' | '*' | '0'..='9' | '_' | 'a' | 'o' | 'v' | 'x' | 'y' => 500,
        '%' => 1000,
        '&' | 'm' => 833,
        '\'' | 'i' | 'l' | '\\' => 278,
        '+' | '<' | '=' | '>' => 570,
        '/' => 278,
        '@' => 930,
        'A' | 'D' | 'N' | 'R' | 'U' | 'V' | 'X' => 722,
        'B' | 'E' | 'L' | 'T' | 'Z' => 667,
        'C' | 'G' | 'H' | 'K' | 'O' | 'Q' | 'Y' => 778,
        'F' | 'P' | 'S' | 'b' | 'd' | 'h' | 'k' | 'n' | 'p' | 'q' | 'u' => 556,
        'I' | 's' | '{' | '}' => 389,
        'M' => 944,
        'W' => 1000,
        '^' => 581,
        'c' | 'e' | 'z' => 444,
        'g' => 500,
        'j' | 't' => 333,
        'r' => 444,
        'w' => 722,
        '|' => 220,
        '~' => 520,
        '\u{2013}' => 500,
        '\u{2014}' => 1000,
        '\u{2018}' | '\u{2019}' => 333,
        '\u{201c}' | '\u{201d}' => 500,
        '\u{2022}' => 350,
        '\u{2026}' => 1000,
        _ if ch.is_whitespace() => 250,
        _ => 500,
    }
}

fn times_italic_width_units(ch: char) -> u16 {
    match ch {
        ' ' | '\u{00a0}' | ',' | '.' => 250,
        '!' | '(' | ')' | '`' | ':' | ';' => 333,
        '"' => 420,
        '#'
        | '$'
        | '*'
        | '0'..='9'
        | '?'
        | '_'
        | 'a'
        | 'b'
        | 'd'
        | 'g'
        | 'h'
        | 'n'
        | 'o'
        | 'p'
        | 'q'
        | 'u' => 500,
        '%' | 'M' | 'w' => 833,
        '&' => 778,
        '\'' => 214,
        '+' | '<' | '=' | '>' => 675,
        '-' => 333,
        '/' | 'f' | 'i' | 'j' | 'l' | 't' | '\\' => 278,
        '@' => 920,
        'A' | 'B' | 'E' | 'F' | 'P' | 'R' | 'V' | 'X' => 611,
        'C' | 'K' | 'W' => 667,
        'D' | 'H' | 'O' | 'Q' | 'U' | 'm' => 722,
        'G' => 722,
        'I' => 333,
        'J' | 'c' | 'e' | 'x' | 'y' => 444,
        'L' | 'T' | 'Y' | 'Z' => 556,
        'N' => 667,
        'S' => 500,
        '[' | ']' | 'r' | 's' | 'z' => 389,
        '^' => 422,
        'k' => 444,
        'v' => 444,
        '{' | '}' => 400,
        '|' => 275,
        '~' => 541,
        '\u{2013}' => 500,
        '\u{2014}' => 889,
        '\u{2018}' | '\u{2019}' => 333,
        '\u{201c}' | '\u{201d}' => 556,
        '\u{2022}' => 350,
        '\u{2026}' => 889,
        _ if ch.is_whitespace() => 250,
        _ => 500,
    }
}

fn symbol_width_units(ch: char) -> u16 {
    match ch {
        ' ' | '\u{00a0}' => 250,
        _ if ch.is_whitespace() => 250,
        _ => 600,
    }
}

fn character_spacing_width(text: &str, style: &CharacterStyle) -> f32 {
    let visible_count = text
        .chars()
        .filter(|ch| !is_zero_width_format_char(*ch))
        .count();
    if visible_count <= 1 || style.character_spacing_twips == 0 {
        return 0.0;
    }
    twips_to_points(style.character_spacing_twips) * visible_count.saturating_sub(1) as f32
}

fn passive_kerning_width(text: &str, style: &CharacterStyle, family: PdfFontFamily) -> f32 {
    let mut total = 0.0;
    let mut previous = None;
    for ch in text.chars().filter(|ch| !is_zero_width_format_char(*ch)) {
        if let Some(left) = previous {
            total += passive_pair_kerning_points(left, ch, family, style);
        }
        previous = Some(ch);
    }
    total
}

pub fn passive_pair_kerning_points(
    left: char,
    right: char,
    family: PdfFontFamily,
    style: &CharacterStyle,
) -> f32 {
    if !style_uses_passive_kerning(style) {
        return 0.0;
    }

    let factor = match (left, right) {
        ('A', 'V') | ('A', 'W') | ('A', 'Y') => 0.11,
        ('F', 'A') | ('L', 'T') | ('L', 'V') | ('L', 'W') | ('L', 'Y') => 0.08,
        ('P', 'A') | ('T', 'A') | ('T', 'a') | ('T', 'e') | ('T', 'o') | ('T', 'y') => 0.10,
        ('V', 'A') | ('V', 'a') | ('V', 'e') | ('V', 'o') => 0.10,
        ('W', 'A') | ('W', 'a') | ('W', 'e') | ('W', 'o') => 0.08,
        ('Y', 'A') | ('Y', 'a') | ('Y', 'e') | ('Y', 'o') => 0.12,
        _ => return 0.0,
    };

    match family {
        PdfFontFamily::Helvetica | PdfFontFamily::Times => -(style.font_size_points() * factor),
        PdfFontFamily::Courier | PdfFontFamily::Symbol | PdfFontFamily::ZapfDingbats => 0.0,
    }
}

pub fn style_uses_passive_kerning(style: &CharacterStyle) -> bool {
    style.character_kerning_half_points > 0
        && style.font_size_half_points >= style.character_kerning_half_points
}

fn is_zero_width_format_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{00ad}' | '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{200e}' | '\u{200f}' | '\u{feff}'
    )
}

pub fn twips_to_points(twips: i32) -> f32 {
    twips as f32 / TWIPS_PER_POINT
}

#[cfg(test)]
mod tests {
    use crate::fonts::{FontAsset, FontAssetStyle, FontProviderLimits};
    use crate::model::{
        Block, Color, Document, FontDef, FontFamilyHint, ImageCrop, ImageFormat,
        PAGE_NUMBER_MARKER, PageNumberFormat, PageSettings, Paragraph, Run, StaticImagePlacement,
        StaticShape, StaticShapeKind, StaticShapePoint, Table, TableCell, TableCellBorder,
        TableCellBorders, TableCellPadding, TableCellSpacing, TableCellVerticalMerge, TableRow,
    };

    use super::*;

    fn test_markers(page_number: &str, section_number: &str) -> MarkerContext {
        MarkerContext {
            page_number: page_number.to_string(),
            section_number: section_number.to_string(),
            document_words: "0".to_string(),
            document_chars: "0".to_string(),
            document_chars_with_spaces: "0".to_string(),
        }
    }

    #[test]
    fn paginates_long_documents() {
        let mut document = Document::default();
        document.blocks.clear();
        for _ in 0..120 {
            document.blocks.push(Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "A paragraph that should force more than one page.".to_string(),
                    style: Default::default(),
                }],
            }));
        }

        let layout = LayoutEngine::layout(&document);
        assert!(layout.pages.len() > 1);
    }

    #[test]
    fn lays_out_scaled_static_images_without_goal_dimensions() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Jpeg,
            bytes: vec![0xff, 0xd8, 0xff, 0xd9],
            palette: Vec::new(),
            vector_commands: Vec::new(),
            width_px: 100,
            height_px: 50,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: None,
            display_height_twips: None,
            scale_x_percent: Some(50),
            scale_y_percent: Some(200),
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let image = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .expect("scaled image");

        assert!((image.width - 37.5).abs() < 0.01);
        assert!((image.height - 75.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_picture_natural_size_hints_when_goals_are_absent() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Jpeg,
            bytes: vec![0xff, 0xd8, 0xff, 0xd9],
            palette: Vec::new(),
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: Some(80),
            natural_height_px_hint: Some(40),
            display_width_twips: None,
            display_height_twips: None,
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let image = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .expect("hint-sized image");

        assert!((image.width - 60.0).abs() < 0.01);
        assert!((image.height - 30.0).abs() < 0.01);
        assert_eq!(image.image.width_px, 1);
        assert_eq!(image.image.height_px, 1);
    }

    #[test]
    fn applies_picture_scaling_to_goal_dimensions() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Jpeg,
            bytes: vec![0xff, 0xd8, 0xff, 0xd9],
            palette: Vec::new(),
            vector_commands: Vec::new(),
            width_px: 100,
            height_px: 50,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(1440),
            scale_x_percent: Some(50),
            scale_y_percent: Some(200),
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let image = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .expect("scaled image");

        assert!((image.width - 18.0).abs() < 0.01);
        assert!((image.height - 144.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_shape_picture_images_with_bounded_passive_placement_frame() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Jpeg,
            bytes: vec![0xff, 0xd8, 0xff, 0xd9],
            palette: Vec::new(),
            vector_commands: Vec::new(),
            width_px: 100,
            height_px: 50,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(7200),
            display_height_twips: Some(3600),
            scale_x_percent: Some(25),
            scale_y_percent: Some(25),
            crop: ImageCrop::default(),
            placement: Some(StaticImagePlacement {
                left_twips: 720,
                top_twips: 360,
                width_twips: 1440,
                height_twips: 720,
                text_wrap: false,
                wrap_margin_left_twips: 120,
                wrap_margin_right_twips: 120,
            }),
        })];

        let layout = LayoutEngine::layout(&document);
        let image = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .expect("placed shape image");

        assert!((image.x - 108.0).abs() < 0.01);
        assert!((image.y - 666.0).abs() < 0.01);
        assert!((image.width - 72.0).abs() < 0.01);
        assert!((image.height - 36.0).abs() < 0.01);
    }

    #[test]
    fn wraps_paragraph_line_beside_prior_passive_shape_picture_frame() {
        let mut document = Document::default();
        document.blocks = vec![
            Block::Image(StaticImage {
                format: ImageFormat::Jpeg,
                bytes: vec![0xff, 0xd8, 0xff, 0xd9],
                palette: Vec::new(),
                vector_commands: Vec::new(),
                width_px: 100,
                height_px: 50,
                natural_width_px_hint: None,
                natural_height_px_hint: None,
                display_width_twips: None,
                display_height_twips: None,
                scale_x_percent: None,
                scale_y_percent: None,
                crop: ImageCrop::default(),
                placement: Some(StaticImagePlacement {
                    left_twips: 0,
                    top_twips: 0,
                    width_twips: 1440,
                    height_twips: 720,
                    text_wrap: true,
                    wrap_margin_left_twips: 120,
                    wrap_margin_right_twips: 120,
                }),
            }),
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Wrapped beside image".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let image = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .expect("wrapped shape image");
        let text = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(text) if text.text.contains("Wrapped") => Some(text),
                _ => None,
            })
            .expect("paragraph text");

        assert!((image.x - 72.0).abs() < 0.01);
        assert!(
            text.x > image.x + image.width,
            "text should be shifted beside wrapped image: text {}, image right {}",
            text.x,
            image.x + image.width
        );
        assert!(text.baseline_y < image.y + image.height && text.baseline_y > image.y);
    }

    #[test]
    fn wrapped_paragraph_honors_passive_shape_picture_wrap_margins() {
        let mut document = Document::default();
        document.blocks = vec![
            Block::Image(StaticImage {
                format: ImageFormat::Jpeg,
                bytes: vec![0xff, 0xd8, 0xff, 0xd9],
                palette: Vec::new(),
                vector_commands: Vec::new(),
                width_px: 100,
                height_px: 50,
                natural_width_px_hint: None,
                natural_height_px_hint: None,
                display_width_twips: None,
                display_height_twips: None,
                scale_x_percent: None,
                scale_y_percent: None,
                crop: ImageCrop::default(),
                placement: Some(StaticImagePlacement {
                    left_twips: 0,
                    top_twips: 0,
                    width_twips: 1440,
                    height_twips: 720,
                    text_wrap: true,
                    wrap_margin_left_twips: 120,
                    wrap_margin_right_twips: 720,
                }),
            }),
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Wrapped with larger margin".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let image = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .expect("wrapped shape image");
        let text = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(text) if text.text.contains("Wrapped") => Some(text),
                _ => None,
            })
            .expect("paragraph text");

        assert!(
            text.x >= image.x + image.width + 36.0 - 0.01,
            "text should honor normalized right wrap margin: text {}, expected at least {}",
            text.x,
            image.x + image.width + 36.0
        );
    }

    #[test]
    fn wrapped_paragraph_returns_to_full_width_below_passive_shape_picture_frame() {
        let mut document = Document::default();
        document.blocks = vec![
            Block::Image(StaticImage {
                format: ImageFormat::Jpeg,
                bytes: vec![0xff, 0xd8, 0xff, 0xd9],
                palette: Vec::new(),
                vector_commands: Vec::new(),
                width_px: 100,
                height_px: 50,
                natural_width_px_hint: None,
                natural_height_px_hint: None,
                display_width_twips: None,
                display_height_twips: None,
                scale_x_percent: None,
                scale_y_percent: None,
                crop: ImageCrop::default(),
                placement: Some(StaticImagePlacement {
                    left_twips: 0,
                    top_twips: 0,
                    width_twips: 5760,
                    height_twips: 560,
                    text_wrap: true,
                    wrap_margin_left_twips: 120,
                    wrap_margin_right_twips: 120,
                }),
            }),
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo twentythree twentyfour".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let image = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .expect("wrapped shape image");
        let mut text_lines: Vec<(f32, f32, String)> = Vec::new();
        for item in &page.items {
            let LayoutItem::Text(fragment) = item else {
                continue;
            };
            if let Some((_, _, text)) = text_lines
                .iter_mut()
                .find(|(baseline_y, _, _)| (*baseline_y - fragment.baseline_y).abs() < 0.01)
            {
                text.push_str(&fragment.text);
            } else {
                text_lines.push((fragment.baseline_y, fragment.x, fragment.text.clone()));
            }
        }

        assert!(
            text_lines
                .iter()
                .any(|(baseline_y, x, text)| *baseline_y > image.y
                    && *baseline_y < image.y + image.height
                    && *x > image.x + image.width
                    && text.len() < 32),
            "expected a short line beside the wrapped image: {:?}",
            text_lines
        );
        assert!(
            text_lines
                .iter()
                .any(|(baseline_y, x, text)| *baseline_y < image.y
                    && (*x - page.geometry.margin_left).abs() < 0.01
                    && text.len() > 40),
            "expected a full-width line below the wrapped image: {:?}",
            text_lines
        );
    }

    #[test]
    fn wrapped_paragraph_uses_free_interval_between_passive_shape_picture_frames() {
        let wrapped_image = |left_twips| StaticImage {
            format: ImageFormat::Jpeg,
            bytes: vec![0xff, 0xd8, 0xff, 0xd9],
            palette: Vec::new(),
            vector_commands: Vec::new(),
            width_px: 100,
            height_px: 50,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: None,
            display_height_twips: None,
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: Some(StaticImagePlacement {
                left_twips,
                top_twips: 0,
                width_twips: 2160,
                height_twips: 720,
                text_wrap: true,
                wrap_margin_left_twips: 120,
                wrap_margin_right_twips: 120,
            }),
        };
        let mut document = Document::default();
        document.blocks = vec![
            Block::Image(wrapped_image(0)),
            Block::Image(wrapped_image(5760)),
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "center gap words should flow between the two passive picture frames before returning below them"
                        .to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let images = page
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Image(image) => Some(image),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(images.len(), 2);
        let text = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(text)
                    if text.baseline_y > images[0].y
                        && text.baseline_y < images[0].y + images[0].height =>
                {
                    Some(text)
                }
                _ => None,
            })
            .expect("text beside wrapped images");

        assert!(
            text.x > images[0].x + images[0].width && text.x < images[1].x,
            "text should use the free interval between wrapped images: text x {}, left image right {}, right image left {}",
            text.x,
            images[0].x + images[0].width,
            images[1].x
        );
    }

    #[test]
    fn lays_out_legacy_static_drawing_shapes_as_passive_lines() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Rectangle,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            stroke_width_twips: 30,
            stroke_color: Color {
                red: 255,
                green: 128,
                blue: 0,
            },
            stroke_style: BorderStyle::Single,
            fill_color: None,
            text: Vec::new(),
            points: Vec::new(),
        })];

        let layout = LayoutEngine::layout(&document);
        let lines = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    width,
                    color,
                    ..
                } => Some((*x1, *y1, *x2, *y2, *width, *color)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(lines.len(), 4);
        assert!(lines.iter().any(|(x1, y1, x2, y2, width, color)| {
            (*x1 - 90.0).abs() < 0.01
                && (*x2 - 162.0).abs() < 0.01
                && (*y1 - *y2).abs() < 0.01
                && (*width - 1.5).abs() < 0.01
                && (color.red - 1.0).abs() < 0.01
                && (color.green - 0.5019608).abs() < 0.01
                && color.blue == 0.0
        }));
    }

    #[test]
    fn lays_out_zero_width_static_drawing_outline_as_fill_only() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Rectangle,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            stroke_width_twips: 0,
            stroke_color: Color {
                red: 255,
                green: 0,
                blue: 0,
            },
            stroke_style: BorderStyle::Single,
            fill_color: Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            }),
            text: Vec::new(),
            points: Vec::new(),
        })];

        let layout = LayoutEngine::layout(&document);
        let fill_count = layout.pages[0]
            .items
            .iter()
            .filter(|item| matches!(item, LayoutItem::Highlight { .. }))
            .count();
        let stroke_count = layout.pages[0]
            .items
            .iter()
            .filter(|item| matches!(item, LayoutItem::Line { .. }))
            .count();

        assert_eq!(fill_count, 1);
        assert_eq!(stroke_count, 0);
    }

    #[test]
    fn lays_out_shape_text_paragraph_shading_and_borders_inside_shape_bounds() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 240,
                green: 240,
                blue: 0,
            },
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
        ];
        let mut paragraph_style = ParagraphStyle {
            shading_color_index: Some(1),
            ..ParagraphStyle::default()
        };
        paragraph_style.borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 40,
            color_index: Some(2),
            ..TableCellBorder::default()
        };
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Rectangle,
            left_twips: 720,
            top_twips: 720,
            width_twips: 2160,
            height_twips: 720,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            stroke_width_twips: 0,
            stroke_color: Color::default(),
            stroke_style: BorderStyle::Single,
            fill_color: None,
            text: vec![Paragraph {
                style: paragraph_style,
                runs: vec![Run {
                    text: "Styled shape".to_string(),
                    style: Default::default(),
                }],
            }],
            points: Vec::new(),
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let shape_left = 108.0;
        let shape_right = 216.0;
        let shape_bottom = 648.0;
        let shape_top = 684.0;
        let content_left = shape_left + 4.0;
        let content_right = shape_right - 4.0;
        let text = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text.contains("Styled") => Some(fragment),
                _ => None,
            })
            .expect("shape text fragment");

        assert!(text.x >= content_left && text.x < content_right);
        assert!(text.baseline_y > shape_bottom && text.baseline_y < shape_top);
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Highlight { x, y, width, height, color }
                if *x >= content_left
                    && *x + *width <= content_right + 0.01
                    && *y >= shape_bottom
                    && *y + *height <= shape_top
                    && *color == PdfColor {
                        red: 240.0 / 255.0,
                        green: 240.0 / 255.0,
                        blue: 0.0
                    }
        )));
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, y1, x2, y2, color, .. }
                if *x1 >= content_left
                    && *x2 <= content_right + 0.01
                    && (*y1 - *y2).abs() < 0.01
                    && *y1 >= shape_bottom
                    && *y1 <= shape_top
                    && *color == PdfColor {
                        red: 1.0,
                        green: 0.0,
                        blue: 0.0
                    }
        )));
    }

    #[test]
    fn lays_out_legacy_static_drawing_line_styles_as_passive_line_styles() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Line,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            stroke_width_twips: 30,
            stroke_color: Color::default(),
            stroke_style: BorderStyle::Dashed,
            fill_color: None,
            text: Vec::new(),
            points: Vec::new(),
        })];

        let layout = LayoutEngine::layout(&document);
        let line = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Line { width, style, .. } => Some((*width, *style)),
                _ => None,
            })
            .expect("styled line");

        assert!((line.0 - 1.5).abs() < 0.01);
        assert_eq!(line.1, LineStyle::Dashed);
    }

    #[test]
    fn lays_out_flipped_static_drawing_lines_as_mirrored_passive_geometry() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Line,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            flip_horizontal: true,
            flip_vertical: true,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            stroke_width_twips: 30,
            stroke_color: Color::default(),
            stroke_style: BorderStyle::Single,
            fill_color: None,
            text: Vec::new(),
            points: Vec::new(),
        })];

        let layout = LayoutEngine::layout(&document);
        let line = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Line { x1, y1, x2, y2, .. } => Some((*x1, *y1, *x2, *y2)),
                _ => None,
            })
            .expect("flipped line");

        assert!((line.0 - 162.0).abs() < 0.01);
        assert!((line.1 - 672.0).abs() < 0.01);
        assert!((line.2 - 90.0).abs() < 0.01);
        assert!((line.3 - 708.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_static_drawing_arrowheads_as_passive_geometry() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Line,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::Open,
            end_arrowhead: StaticShapeArrowhead::Triangle,
            stroke_width_twips: 30,
            stroke_color: Color {
                red: 255,
                green: 0,
                blue: 0,
            },
            stroke_style: BorderStyle::Single,
            fill_color: None,
            text: Vec::new(),
            points: Vec::new(),
        })];

        let layout = LayoutEngine::layout(&document);
        let lines = layout.pages[0]
            .items
            .iter()
            .filter(|item| matches!(item, LayoutItem::Line { .. }))
            .count();
        let arrow_polygon = layout.pages[0].items.iter().find_map(|item| match item {
            LayoutItem::Polygon {
                points, fill_color, ..
            } => Some((points, fill_color)),
            _ => None,
        });

        assert_eq!(lines, 3);
        let (points, fill_color) = arrow_polygon.expect("filled triangle arrowhead");
        assert_eq!(points.len(), 3);
        assert_eq!(
            *fill_color,
            Some(PdfColor {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
            })
        );
    }

    #[test]
    fn lays_out_legacy_static_drawing_polylines_as_passive_line_segments() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Polyline,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            stroke_width_twips: 30,
            stroke_color: Color::default(),
            stroke_style: BorderStyle::Dotted,
            fill_color: None,
            text: Vec::new(),
            points: vec![
                StaticShapePoint {
                    x_twips: 0,
                    y_twips: 0,
                },
                StaticShapePoint {
                    x_twips: 720,
                    y_twips: 720,
                },
                StaticShapePoint {
                    x_twips: 1440,
                    y_twips: 0,
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let lines = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    width,
                    style,
                    ..
                } => Some((*x1, *y1, *x2, *y2, *width, *style)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(lines.len(), 2);
        assert!(lines.iter().any(|line| {
            (line.0 - 90.0).abs() < 0.01
                && (line.2 - 126.0).abs() < 0.01
                && (line.4 - 1.5).abs() < 0.01
                && line.5 == LineStyle::Dotted
        }));
        assert!(lines.iter().any(|line| {
            (line.0 - 126.0).abs() < 0.01
                && (line.2 - 162.0).abs() < 0.01
                && line.5 == LineStyle::Dotted
        }));
    }

    #[test]
    fn lays_out_legacy_static_drawing_polygons_as_passive_closed_paths() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Polygon,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            stroke_width_twips: 30,
            stroke_color: Color::default(),
            stroke_style: BorderStyle::Dotted,
            fill_color: Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            }),
            text: Vec::new(),
            points: vec![
                StaticShapePoint {
                    x_twips: 0,
                    y_twips: 0,
                },
                StaticShapePoint {
                    x_twips: 720,
                    y_twips: 720,
                },
                StaticShapePoint {
                    x_twips: 1440,
                    y_twips: 0,
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let polygon = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Polygon {
                    points,
                    stroke_width,
                    stroke_style,
                    fill_color,
                    ..
                } => Some((points.clone(), *stroke_width, *stroke_style, *fill_color)),
                _ => None,
            })
            .expect("polygon");

        assert_eq!(polygon.0.len(), 3);
        assert!((polygon.0[0].x - 90.0).abs() < 0.01);
        assert!((polygon.0[1].x - 126.0).abs() < 0.01);
        assert!((polygon.0[2].x - 162.0).abs() < 0.01);
        assert!((polygon.1 - 1.5).abs() < 0.01);
        assert_eq!(polygon.2, LineStyle::Dotted);
        let fill = polygon.3.expect("polygon fill");
        assert!((fill.red - (10.0 / 255.0)).abs() < 0.01);
        assert!((fill.green - (20.0 / 255.0)).abs() < 0.01);
        assert!((fill.blue - (30.0 / 255.0)).abs() < 0.01);
    }

    #[test]
    fn lays_out_filled_legacy_static_drawing_rectangles_as_passive_fill() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Rectangle,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            stroke_width_twips: 20,
            stroke_color: Color::default(),
            stroke_style: BorderStyle::Single,
            fill_color: Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            }),
            text: Vec::new(),
            points: Vec::new(),
        })];

        let layout = LayoutEngine::layout(&document);
        let fill = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight {
                    x,
                    y,
                    width,
                    height,
                    color,
                } => Some((*x, *y, *width, *height, *color)),
                _ => None,
            })
            .expect("filled rectangle");

        assert!((fill.0 - 90.0).abs() < 0.01);
        assert!((fill.2 - 72.0).abs() < 0.01);
        assert!((fill.3 - 36.0).abs() < 0.01);
        assert!((fill.4.red - (10.0 / 255.0)).abs() < 0.01);
        assert!((fill.4.green - (20.0 / 255.0)).abs() < 0.01);
        assert!((fill.4.blue - (30.0 / 255.0)).abs() < 0.01);
    }

    #[test]
    fn lays_out_legacy_static_drawing_ellipses_as_passive_paths() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Ellipse,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            stroke_width_twips: 20,
            stroke_color: Color {
                red: 255,
                green: 128,
                blue: 0,
            },
            stroke_style: BorderStyle::Single,
            fill_color: Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            }),
            text: Vec::new(),
            points: Vec::new(),
        })];

        let layout = LayoutEngine::layout(&document);
        let ellipse = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Ellipse {
                    x,
                    y,
                    width,
                    height,
                    stroke_width,
                    stroke_color,
                    stroke_style,
                    fill_color,
                } => Some((
                    *x,
                    *y,
                    *width,
                    *height,
                    *stroke_width,
                    *stroke_color,
                    *stroke_style,
                    *fill_color,
                )),
                _ => None,
            })
            .expect("ellipse");

        assert!((ellipse.0 - 90.0).abs() < 0.01);
        assert!((ellipse.2 - 72.0).abs() < 0.01);
        assert!((ellipse.3 - 36.0).abs() < 0.01);
        assert!((ellipse.4 - 1.0).abs() < 0.01);
        assert!((ellipse.5.red - 1.0).abs() < 0.01);
        assert!((ellipse.5.green - 0.5019608).abs() < 0.01);
        assert_eq!(ellipse.5.blue, 0.0);
        assert_eq!(ellipse.6, LineStyle::Solid);
        let fill = ellipse.7.expect("ellipse fill");
        assert!((fill.red - (10.0 / 255.0)).abs() < 0.01);
        assert!((fill.green - (20.0 / 255.0)).abs() < 0.01);
        assert!((fill.blue - (30.0 / 255.0)).abs() < 0.01);
    }

    #[test]
    fn lays_out_legacy_static_drawing_rounded_rectangles_as_passive_paths() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::RoundedRectangle,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            stroke_width_twips: 20,
            stroke_color: Color {
                red: 255,
                green: 128,
                blue: 0,
            },
            stroke_style: BorderStyle::Single,
            fill_color: Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            }),
            text: Vec::new(),
            points: Vec::new(),
        })];

        let layout = LayoutEngine::layout(&document);
        let rounded = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::RoundedRectangle {
                    x,
                    y,
                    width,
                    height,
                    radius,
                    stroke_width,
                    stroke_color,
                    stroke_style,
                    fill_color,
                } => Some((
                    *x,
                    *y,
                    *width,
                    *height,
                    *radius,
                    *stroke_width,
                    *stroke_color,
                    *stroke_style,
                    *fill_color,
                )),
                _ => None,
            })
            .expect("rounded rectangle");

        assert!((rounded.0 - 90.0).abs() < 0.01);
        assert!((rounded.2 - 72.0).abs() < 0.01);
        assert!((rounded.3 - 36.0).abs() < 0.01);
        assert!((rounded.4 - 7.2).abs() < 0.01);
        assert!((rounded.5 - 1.0).abs() < 0.01);
        assert!((rounded.6.red - 1.0).abs() < 0.01);
        assert!((rounded.6.green - 0.5019608).abs() < 0.01);
        assert_eq!(rounded.6.blue, 0.0);
        assert_eq!(rounded.7, LineStyle::Solid);
        let fill = rounded.8.expect("rounded rectangle fill");
        assert!((fill.red - (10.0 / 255.0)).abs() < 0.01);
        assert!((fill.green - (20.0 / 255.0)).abs() < 0.01);
        assert!((fill.blue - (30.0 / 255.0)).abs() < 0.01);
    }

    #[test]
    fn lays_out_table_text_and_grid_lines() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440, 1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: None,
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![
                    TableCell {
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "A".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    },
                    TableCell {
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "B".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    },
                ],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        assert!(
            page.items
                .iter()
                .any(|item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "A"))
        );
        assert!(
            page.items
                .iter()
                .any(|item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "B"))
        );
        assert!(
            page.items
                .iter()
                .any(|item| matches!(item, LayoutItem::Line { .. }))
        );
    }

    #[test]
    fn table_column_widths_preserve_authored_widths_when_autofit_growth_is_disabled() {
        let mut table = Table {
            column_widths_twips: vec![4_000, 4_000],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: Vec::new(),
        };

        let scaled = table_column_widths(&table, 2, 288.0);
        assert!((scaled.iter().sum::<f32>() - 288.0).abs() < 0.01);

        table.preserve_authored_widths = true;
        let preserved = table_column_widths(&table, 2, 288.0);
        assert!((preserved[0] - 200.0).abs() < 0.01);
        assert!((preserved[1] - 200.0).abs() < 0.01);
        assert!((preserved.iter().sum::<f32>() - 400.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_borderless_table_text_without_grid_lines() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: false,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: None,
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: "No borders".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert_eq!(layout_text(page), "No borders");
        assert!(
            !page
                .items
                .iter()
                .any(|item| matches!(item, LayoutItem::Line { .. }))
        );
    }

    #[test]
    fn lays_out_table_cell_side_borders() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: None,
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders {
                        left: TableCellBorder {
                            visible: false,
                            ..TableCellBorder::default()
                        },
                        right: TableCellBorder::default(),
                        top: TableCellBorder::default(),
                        bottom: TableCellBorder::default(),
                        ..TableCellBorders::default()
                    },
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: "Borders".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(!has_vertical_line_at(page, 72.0));
        assert!(has_vertical_line_at(page, 144.0));
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { y1, y2, .. } if (*y1 - *y2).abs() < 0.01
        )));
    }

    #[test]
    fn lays_out_table_cell_diagonal_borders_as_passive_lines() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: Some(720),
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders {
                        left: TableCellBorder {
                            visible: false,
                            ..TableCellBorder::default()
                        },
                        right: TableCellBorder {
                            visible: false,
                            ..TableCellBorder::default()
                        },
                        top: TableCellBorder {
                            visible: false,
                            ..TableCellBorder::default()
                        },
                        bottom: TableCellBorder {
                            visible: false,
                            ..TableCellBorder::default()
                        },
                        diagonal_down: TableCellBorder::default(),
                        diagonal_up: TableCellBorder {
                            width_twips: 40,
                            ..TableCellBorder::default()
                        },
                    },
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: "Diagonal".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let diagonals = layout.pages[0]
            .items
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    LayoutItem::Line { x1, y1, x2, y2, .. }
                        if (*x1 - *x2).abs() > 0.01 && (*y1 - *y2).abs() > 0.01
                )
            })
            .count();

        assert_eq!(diagonals, 2);
    }

    #[test]
    fn lays_out_table_cell_border_width_and_color() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
        ];
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: None,
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders {
                        left: TableCellBorder {
                            visible: true,
                            width_twips: 80,
                            color_index: Some(1),
                            ..TableCellBorder::default()
                        },
                        right: TableCellBorder::default(),
                        top: TableCellBorder::default(),
                        bottom: TableCellBorder::default(),
                        ..TableCellBorders::default()
                    },
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: "Border style".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, x2, width, color, .. }
                if (*x1 - 72.0).abs() < 0.01
                    && (*x2 - 72.0).abs() < 0.01
                    && (*width - 4.0).abs() < 0.01
                    && *color == PdfColor { red: 1.0, green: 0.0, blue: 0.0 }
        )));
    }

    #[test]
    fn lays_out_table_row_borders_from_normalized_cell_perimeter() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;}\trowd\trbrdrt\brdrdb\brdrw80\brdrcf1\trbrdrl\brdrdash\brdrw40\cellx1440 A\cell\cellx2880 B\cell\row}",
        )
        .unwrap();

        let layout = LayoutEngine::layout(&parsed.document);
        let page = &layout.pages[0];
        let red = PdfColor {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
        };
        let top_segments = page
            .items
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    LayoutItem::Line {
                        y1,
                        y2,
                        width,
                        color,
                        style,
                        ..
                    } if (*y1 - *y2).abs() < 0.01
                        && (*width - 4.0).abs() < 0.01
                        && *color == red
                        && *style == LineStyle::Double
                )
            })
            .count();

        assert_eq!(top_segments, 2);
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, x2, width, style, .. }
                if (*x1 - *x2).abs() < 0.01
                    && (*width - 2.0).abs() < 0.01
                    && *style == LineStyle::Dashed
        )));
        assert!(layout_text(page).contains("A"));
        assert!(layout_text(page).contains("B"));
    }

    #[test]
    fn lays_out_extended_word_border_styles_as_passive_lines() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1\box\brdrhair Hairline\par\pard\brdrb\brdrdashdot Dashed\par}",
        )
        .unwrap();

        let layout = LayoutEngine::layout(&parsed.document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { width, style, .. }
                if (*width - 0.25).abs() < 0.01 && *style == LineStyle::Solid
        )));
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { style, .. } if *style == LineStyle::Dashed
        )));
        assert!(layout_text(page).contains("Hairline"));
        assert!(layout_text(page).contains("Dashed"));
    }

    #[test]
    fn lays_out_shaded_table_cell_background() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 220,
                green: 230,
                blue: 240,
            },
        ];
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: None,
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: Some(1),
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: "Shaded".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(|item| {
            matches!(
                item,
                LayoutItem::Highlight { color, .. }
                    if *color == PdfColor {
                        red: 220.0 / 255.0,
                        green: 230.0 / 255.0,
                        blue: 240.0 / 255.0
                    }
            )
        }));
        assert!(layout_text(page).contains("Shaded"));
    }

    #[test]
    fn lays_out_row_default_table_shading_from_normalized_cells() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1{\colortbl;\red220\green230\blue240;}\trowd\trcbpat1\cellx1440 A\cell\cellx2880 B\cell\row}",
        )
        .unwrap();

        let layout = LayoutEngine::layout(&parsed.document);
        let page = &layout.pages[0];
        let expected_color = PdfColor {
            red: 220.0 / 255.0,
            green: 230.0 / 255.0,
            blue: 240.0 / 255.0,
        };
        let row_backgrounds = page
            .items
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    LayoutItem::Highlight { width, color, .. }
                        if (*width - 72.0).abs() < 0.01 && *color == expected_color
                )
            })
            .count();

        assert_eq!(row_backgrounds, 2);
        assert!(layout_text(page).contains("A"));
        assert!(layout_text(page).contains("B"));
    }

    #[test]
    fn lays_out_table_row_height_as_minimum_height() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 240,
                green: 240,
                blue: 240,
            },
        ];
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: Some(720),
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: Some(1),
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: "Tall".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let height = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight { height, .. } => Some(*height),
                _ => None,
            })
            .expect("row background");

        assert!((height - 36.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_negative_table_row_height_as_exact_height() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 240,
                green: 240,
                blue: 240,
            },
        ];
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: Some(-360),
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: Some(1),
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: "Exact row with text that would otherwise need more space"
                                .to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let height = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight { height, .. } => Some(*height),
                _ => None,
            })
            .expect("row background");

        assert!((height - 18.0).abs() < 0.01);
    }

    #[test]
    fn clips_table_cell_lines_to_exact_row_height() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 240,
                green: 240,
                blue: 240,
            },
        ];
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: Some(-360),
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: Some(1),
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: "Visible\nOverflow".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let text = layout_text(&layout.pages[0]);

        assert!(text.contains("Visible"));
        assert!(!text.contains("Overflow"));
    }

    #[test]
    fn lays_out_table_cell_paragraph_line_spacing() {
        let mut document = Document::default();
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.line_spacing_twips = Some(-480);
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: None,
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: paragraph_style,
                        runs: vec![Run {
                            text: "First\nSecond".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let baselines = text_baselines(&layout.pages[0]);

        assert_eq!(baselines.len(), 2);
        assert!((baselines[0] - baselines[1] - 24.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_table_cell_paragraph_spacing_in_row_height() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 240,
                green: 240,
                blue: 240,
            },
        ];
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.space_before_twips = 240;
        paragraph_style.space_after_twips = 360;
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: None,
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: Some(1),
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: paragraph_style,
                        runs: vec![Run {
                            text: "Spaced".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let height = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight { height, .. } => Some(*height),
                _ => None,
            })
            .expect("row background");

        assert!((height - 45.0).abs() < 0.01);
    }

    #[test]
    fn collapses_contextual_spacing_between_table_cell_paragraphs() {
        let normal_height = table_cell_two_paragraph_height(false);
        let contextual_height = table_cell_two_paragraph_height(true);

        assert!(
            (normal_height - contextual_height - 30.0).abs() < 0.01,
            "contextual spacing should suppress 18pt after + 12pt before inside table cells, normal={normal_height}, contextual={contextual_height}"
        );
    }

    fn table_cell_two_paragraph_height(contextual_spacing: bool) -> f32 {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 240,
                green: 240,
                blue: 240,
            },
        ];
        let mut first_style = ParagraphStyle::default();
        first_style.space_after_twips = 360;
        first_style.contextual_spacing = contextual_spacing;
        let mut second_style = ParagraphStyle::default();
        second_style.space_before_twips = 240;
        second_style.contextual_spacing = contextual_spacing;
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: None,
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: Some(1),
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![
                        Paragraph {
                            style: first_style,
                            runs: vec![Run {
                                text: "First".to_string(),
                                style: Default::default(),
                            }],
                        },
                        Paragraph {
                            style: second_style,
                            runs: vec![Run {
                                text: "Second".to_string(),
                                style: Default::default(),
                            }],
                        },
                    ],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight { height, .. } => Some(*height),
                _ => None,
            })
            .expect("row background")
    }

    #[test]
    fn lays_out_table_row_left_offset() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: None,
                left_offset_twips: 720,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: "Offset".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let text_x = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "Offset" => Some(fragment.x),
                _ => None,
            })
            .expect("offset text");

        assert!((text_x - 111.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_table_row_horizontal_alignment() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![
                TableRow {
                    height_twips: None,
                    left_offset_twips: 0,
                    cell_gap_twips: 60,
                    alignment: TableRowAlignment::Center,
                    repeat_header: false,
                    keep_together: false,
                    cells: vec![TableCell {
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "Center".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    }],
                },
                TableRow {
                    height_twips: None,
                    left_offset_twips: 0,
                    cell_gap_twips: 60,
                    alignment: TableRowAlignment::Right,
                    repeat_header: false,
                    keep_together: false,
                    cells: vec![TableCell {
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "Right".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    }],
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let text_x = |text: &str| {
            layout.pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.trim() == text => Some(fragment.x),
                    _ => None,
                })
                .expect("aligned row text")
        };

        assert!((text_x("Center") - 273.0).abs() < 0.01);
        assert!((text_x("Right") - 471.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_table_cell_gap_as_padding() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: None,
                left_offset_twips: 0,
                cell_gap_twips: 240,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: "Padded".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let text_x = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "Padded" => Some(fragment.x),
                _ => None,
            })
            .expect("padded text");

        assert!((text_x - 84.0).abs() < 0.01);
    }

    #[test]
    fn table_cell_gap_does_not_add_implicit_vertical_padding() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 240,
                green: 240,
                blue: 240,
            },
        ];
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: Some(489),
                left_offset_twips: 0,
                cell_gap_twips: 108,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: Some(1),
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: "Compact".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let row_height = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight { height, .. } => Some(*height),
                _ => None,
            })
            .expect("row background");
        let text_x = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "Compact" => Some(fragment.x),
                _ => None,
            })
            .expect("compact cell text");

        assert!((row_height - twips_to_points(489)).abs() < 0.01);
        assert!((text_x - 77.4).abs() < 0.01);
    }

    #[test]
    fn lays_out_row_default_table_padding_from_normalized_cells() {
        let parsed = crate::rtf::parse_rtf(
            r"{\rtf1\trowd\trpaddl360\trpaddr120\cellx1440 A\cell\cellx2880 B\cell\row}",
        )
        .unwrap();

        let layout = LayoutEngine::layout(&parsed.document);
        let text_x = |text: &str| {
            layout.pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.trim() == text => Some(fragment.x),
                    _ => None,
                })
                .expect("padded cell text")
        };

        assert!((text_x("A") - 90.0).abs() < 0.01);
        assert!((text_x("B") - 162.0).abs() < 0.01);
    }

    #[test]
    fn justifies_non_final_paragraph_lines_with_passive_word_spacing() {
        let mut document = Document::default();
        document.page.width_twips = 5_000;
        document.page.margin_left_twips = 720;
        document.page.margin_right_twips = 720;
        let mut style = ParagraphStyle::default();
        style.alignment = Alignment::Justified;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style,
            runs: vec![Run {
                text: "one two three four five six seven eight nine ten".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let word_spacings = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment.word_spacing),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(
            word_spacings.iter().any(|spacing| *spacing > 0.0),
            "expected at least one non-final justified line to add word spacing: {word_spacings:?}"
        );
        assert!(
            word_spacings.iter().any(|spacing| *spacing == 0.0),
            "expected final justified line to remain unstretched: {word_spacings:?}"
        );
    }

    #[test]
    fn lays_out_table_cell_paragraph_alignment() {
        let mut center_style = ParagraphStyle::default();
        center_style.alignment = Alignment::Center;
        let mut right_style = ParagraphStyle::default();
        right_style.alignment = Alignment::Right;

        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![
                TableRow {
                    height_twips: None,
                    left_offset_twips: 0,
                    cell_gap_twips: 60,
                    alignment: TableRowAlignment::Left,
                    repeat_header: false,
                    keep_together: false,
                    cells: vec![TableCell {
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: center_style,
                            runs: vec![Run {
                                text: "A".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    }],
                },
                TableRow {
                    height_twips: None,
                    left_offset_twips: 0,
                    cell_gap_twips: 60,
                    alignment: TableRowAlignment::Left,
                    repeat_header: false,
                    keep_together: false,
                    cells: vec![TableCell {
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: right_style,
                            runs: vec![Run {
                                text: "B".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    }],
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let text_x = |text: &str| {
            layout.pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.trim() == text => Some(fragment.x),
                    _ => None,
                })
                .expect("cell text")
        };

        let style = CharacterStyle::default();
        let text_width = measure_text_with_family("A", &style, PdfFontFamily::Helvetica);
        let content_left = 75.0;
        let content_width = 66.0;

        assert!((text_x("A") - (content_left + (content_width - text_width) / 2.0)).abs() < 0.01);
        assert!((text_x("B") - (content_left + content_width - text_width)).abs() < 0.01);
    }

    #[test]
    fn lays_out_table_cell_paragraph_borders() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 40,
            style: BorderStyle::Single,
            ..TableCellBorder::default()
        };

        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: false,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: None,
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: paragraph_style,
                        runs: vec![Run {
                            text: "Cell".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(
            |item| matches!(item, LayoutItem::Line { x1, x2, .. } if (*x2 - *x1).abs() > 60.0)
        ));
    }

    #[test]
    fn lays_out_table_cell_first_line_indent_only_on_first_line() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.left_indent_twips = 360;
        paragraph_style.first_line_indent_twips = 360;

        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: None,
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: paragraph_style,
                        runs: vec![Run {
                            text: "First\nSecond".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let text_x = |text: &str| {
            layout.pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.trim() == text => Some(fragment.x),
                    _ => None,
                })
                .expect("cell text")
        };

        assert!((text_x("First") - 111.0).abs() < 0.01);
        assert!((text_x("Second") - 93.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_table_cell_fit_text_as_bounded_passive_scaling() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![720],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: None,
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: true,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: "Fit text stays on one visual row".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let fragments = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment),
                _ => None,
            })
            .collect::<Vec<_>>();
        let first_baseline = fragments
            .first()
            .map(|fragment| fragment.baseline_y)
            .expect("fit text fragments");

        assert!(
            fragments.iter().all(|fragment| {
                fragment.style.character_scaling_percent < 100
                    && (fragment.baseline_y - first_baseline).abs() < 0.01
            }),
            "fit text should render as one passively scaled visual row: {fragments:?}"
        );
    }

    #[test]
    fn lays_out_explicit_table_cell_padding() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 240,
                green: 240,
                blue: 240,
            },
        ];
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: None,
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: Some(1),
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding {
                        left_twips: Some(240),
                        right_twips: Some(120),
                        top_twips: Some(240),
                        bottom_twips: Some(120),
                    },
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: "Padded".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let text_x = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "Padded" => Some(fragment.x),
                _ => None,
            })
            .expect("padded text");
        let row_height = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight { height, .. } => Some(*height),
                _ => None,
            })
            .expect("row background");

        assert!((text_x - 84.0).abs() < 0.01);
        assert!((row_height - 33.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_table_cell_spacing_as_passive_border_gaps() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440, 1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: None,
                left_offset_twips: 0,
                cell_gap_twips: 0,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![
                    TableCell {
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: TableCellSpacing {
                            left_twips: Some(120),
                            right_twips: Some(120),
                            top_twips: Some(60),
                            bottom_twips: Some(60),
                        },
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "Left".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    },
                    TableCell {
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: TableCellSpacing {
                            left_twips: Some(120),
                            right_twips: Some(120),
                            top_twips: Some(60),
                            bottom_twips: Some(60),
                        },
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "Right".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    },
                ],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let text_x = |text: &str| {
            layout.pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text == text => Some(fragment.x),
                    _ => None,
                })
                .expect("cell text")
        };
        let mut vertical_borders = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Line { x1, x2, .. } if (*x1 - *x2).abs() < 0.01 => Some(*x1),
                _ => None,
            })
            .collect::<Vec<_>>();
        vertical_borders.sort_by(f32::total_cmp);

        assert!((text_x("Left") - 78.0).abs() < 0.01);
        assert!((text_x("Right") - 150.0).abs() < 0.01);
        assert!(
            vertical_borders
                .windows(2)
                .any(|pair| (pair[0] - 138.0).abs() < 0.01 && (pair[1] - 150.0).abs() < 0.01),
            "expected spacing to create a passive gap between adjacent cell borders: {vertical_borders:?}"
        );
    }

    #[test]
    fn lays_out_table_cell_vertical_alignment() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440, 1440, 1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: Some(1440),
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![
                    TableCell {
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "Top".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    },
                    TableCell {
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        vertical_align: TableCellVerticalAlign::Center,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "Center".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    },
                    TableCell {
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        vertical_align: TableCellVerticalAlign::Bottom,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "Bottom".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    },
                ],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let baseline_for = |text: &str| {
            layout.pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text == text => {
                        Some(fragment.baseline_y)
                    }
                    _ => None,
                })
                .expect("aligned text")
        };

        assert!(baseline_for("Top") > baseline_for("Center"));
        assert!(baseline_for("Center") > baseline_for("Bottom"));
    }

    #[test]
    fn lays_out_horizontal_merged_table_cells() {
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440, 1440, 1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: None,
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![
                    TableCell {
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::First,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "Merged".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    },
                    TableCell {
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::Continuation,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "Hidden".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    },
                    TableCell {
                        shading_color_index: None,
                        shading_basis_points: 10_000,
                        shading_pattern: crate::model::ShadingPattern::None,
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "Plain".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    },
                ],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let text_x = |text: &str| {
            page.items.iter().find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == text => Some(fragment.x),
                _ => None,
            })
        };

        assert!((text_x("Merged").expect("merged text") - 75.0).abs() < 0.01);
        assert!(text_x("Hidden").is_none());
        assert!((text_x("Plain").expect("plain text") - 219.0).abs() < 0.01);
        assert!(!has_vertical_line_at(page, 144.0));
        assert!(has_vertical_line_at(page, 216.0));
    }

    #[test]
    fn lays_out_vertical_merged_table_cells() {
        let mut document = Document::default();
        document.colors = vec![
            Color {
                red: 255,
                green: 255,
                blue: 255,
            },
            Color {
                red: 210,
                green: 230,
                blue: 255,
            },
        ];
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440, 1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![
                TableRow {
                    height_twips: Some(720),
                    left_offset_twips: 0,
                    cell_gap_twips: 60,
                    alignment: TableRowAlignment::Left,
                    repeat_header: false,
                    keep_together: false,
                    cells: vec![
                        TableCell {
                            shading_color_index: Some(1),
                            shading_basis_points: 10_000,
                            shading_pattern: crate::model::ShadingPattern::None,
                            padding: TableCellPadding::default(),
                            spacing: Default::default(),
                            borders: TableCellBorders::default(),
                            fit_text: false,
                            vertical_align: TableCellVerticalAlign::Bottom,
                            horizontal_merge: TableCellHorizontalMerge::None,
                            vertical_merge: TableCellVerticalMerge::First,
                            paragraphs: vec![Paragraph {
                                style: Default::default(),
                                runs: vec![Run {
                                    text: "Merged top".to_string(),
                                    style: Default::default(),
                                }],
                            }],
                        },
                        TableCell {
                            shading_color_index: None,
                            shading_basis_points: 10_000,
                            shading_pattern: crate::model::ShadingPattern::None,
                            padding: TableCellPadding::default(),
                            spacing: Default::default(),
                            borders: TableCellBorders::default(),
                            fit_text: false,
                            vertical_align: TableCellVerticalAlign::Top,
                            horizontal_merge: TableCellHorizontalMerge::None,
                            vertical_merge: TableCellVerticalMerge::None,
                            paragraphs: vec![Paragraph {
                                style: Default::default(),
                                runs: vec![Run {
                                    text: "Right top".to_string(),
                                    style: Default::default(),
                                }],
                            }],
                        },
                    ],
                },
                TableRow {
                    height_twips: Some(720),
                    left_offset_twips: 0,
                    cell_gap_twips: 60,
                    alignment: TableRowAlignment::Left,
                    repeat_header: false,
                    keep_together: false,
                    cells: vec![
                        TableCell {
                            shading_color_index: None,
                            shading_basis_points: 10_000,
                            shading_pattern: crate::model::ShadingPattern::None,
                            padding: TableCellPadding::default(),
                            spacing: Default::default(),
                            borders: TableCellBorders::default(),
                            fit_text: false,
                            vertical_align: TableCellVerticalAlign::Top,
                            horizontal_merge: TableCellHorizontalMerge::None,
                            vertical_merge: TableCellVerticalMerge::Continuation,
                            paragraphs: vec![Paragraph {
                                style: Default::default(),
                                runs: vec![Run {
                                    text: "Hidden continuation".to_string(),
                                    style: Default::default(),
                                }],
                            }],
                        },
                        TableCell {
                            shading_color_index: None,
                            shading_basis_points: 10_000,
                            shading_pattern: crate::model::ShadingPattern::None,
                            padding: TableCellPadding::default(),
                            spacing: Default::default(),
                            borders: TableCellBorders::default(),
                            fit_text: false,
                            vertical_align: TableCellVerticalAlign::Top,
                            horizontal_merge: TableCellHorizontalMerge::None,
                            vertical_merge: TableCellVerticalMerge::None,
                            paragraphs: vec![Paragraph {
                                style: Default::default(),
                                runs: vec![Run {
                                    text: "Right bottom".to_string(),
                                    style: Default::default(),
                                }],
                            }],
                        },
                    ],
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let text = layout_text(page);
        let internal_y = internal_horizontal_line_y(page, 144.0, 216.0)
            .expect("unmerged column internal border");
        let baseline_y = |needle: &str| {
            page.items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.contains(needle) => {
                        Some(fragment.baseline_y)
                    }
                    _ => None,
                })
                .expect("table cell text")
        };
        let merged_shading_height = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight { x, height, .. } if (*x - 72.0).abs() < 0.01 => {
                    Some(*height)
                }
                _ => None,
            })
            .expect("merged cell shading");

        assert!(text.contains("Merged top"));
        assert!(!text.contains("Hidden continuation"));
        assert!(text.contains("Right bottom"));
        assert!((merged_shading_height - 72.0).abs() < 0.01);
        assert!(baseline_y("Merged") < baseline_y("Right"));
        assert!(!has_horizontal_line_segment_at_y(
            page, 72.0, 144.0, internal_y
        ));
    }

    #[test]
    fn repeats_table_header_rows_after_page_breaks() {
        fn row(text: &str, repeat_header: bool) -> TableRow {
            TableRow {
                height_twips: Some(720),
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: text.to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }
        }

        let mut rows = vec![row("Header", true)];
        for idx in 0..30 {
            rows.push(row(&format!("Body {idx}"), false));
        }

        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows,
        })];

        let layout = LayoutEngine::layout(&document);
        assert!(layout.pages.len() > 1);
        for page in layout.pages.iter().skip(1) {
            assert!(layout_text(page).contains("Header"));
        }

        let header_count = layout
            .pages
            .iter()
            .flat_map(|page| &page.items)
            .filter(|item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Header"))
            .count();
        assert_eq!(header_count, layout.pages.len());
    }

    #[test]
    fn splits_tall_auto_height_table_rows_across_pages() {
        fn row(text: String, repeat_header: bool) -> TableRow {
            TableRow {
                height_twips: None,
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text,
                            style: Default::default(),
                        }],
                    }],
                }],
            }
        }

        let tall_text = (0..14)
            .map(|idx| format!("Line {idx:02}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut document = small_test_page_document();
        document.blocks = vec![Block::Table(Table {
            rows: vec![row(tall_text, false)],
            column_widths_twips: vec![2_880],
            borders_visible: true,
            preserve_authored_widths: false,
        })];

        let layout = LayoutEngine::layout(&document);
        let first_page_text = layout_text(&layout.pages[0]);
        let later_page_text = layout
            .pages
            .iter()
            .skip(1)
            .map(layout_text)
            .collect::<String>();

        assert!(layout.pages.len() > 1);
        assert!(first_page_text.contains("Line 00"));
        assert!(!first_page_text.contains("Line 13"));
        assert!(later_page_text.contains("Line 13"));
    }

    #[test]
    fn repeats_table_header_on_split_tall_row_continuation_pages() {
        fn row(text: String, repeat_header: bool) -> TableRow {
            TableRow {
                height_twips: None,
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text,
                            style: Default::default(),
                        }],
                    }],
                }],
            }
        }

        let tall_text = (0..14)
            .map(|idx| format!("Line {idx:02}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut document = small_test_page_document();
        document.blocks = vec![Block::Table(Table {
            rows: vec![row("Header".to_string(), true), row(tall_text, false)],
            column_widths_twips: vec![2_880],
            borders_visible: true,
            preserve_authored_widths: false,
        })];

        let layout = LayoutEngine::layout(&document);
        assert!(layout.pages.len() > 1);
        for page in layout.pages.iter().skip(1) {
            assert!(
                layout_text(page).contains("Header"),
                "split-row continuation page should repeat table header"
            );
        }
    }

    #[test]
    fn splits_tall_positive_minimum_height_table_rows_across_pages() {
        fn row(text: String, height_twips: Option<i32>) -> TableRow {
            TableRow {
                height_twips,
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text,
                            style: Default::default(),
                        }],
                    }],
                }],
            }
        }

        let tall_text = (0..14)
            .map(|idx| format!("Line {idx:02}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut document = small_test_page_document();
        document.blocks = vec![Block::Table(Table {
            rows: vec![row(tall_text, Some(720))],
            column_widths_twips: vec![2_880],
            borders_visible: true,
            preserve_authored_widths: false,
        })];

        let layout = LayoutEngine::layout(&document);
        let first_page_text = layout_text(&layout.pages[0]);
        let later_page_text = layout
            .pages
            .iter()
            .skip(1)
            .map(layout_text)
            .collect::<String>();

        assert!(layout.pages.len() > 1);
        assert!(first_page_text.contains("Line 00"));
        assert!(!first_page_text.contains("Line 13"));
        assert!(later_page_text.contains("Line 13"));
    }

    #[test]
    fn lays_out_strikeout_and_baseline_shift() {
        let mut document = Document::default();
        let mut shifted_style = CharacterStyle::default();
        shifted_style.baseline_shift_half_points = 8;
        let mut struck_style = CharacterStyle::default();
        struck_style.strike = true;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    text: "base".to_string(),
                    style: Default::default(),
                },
                Run {
                    text: "up".to_string(),
                    style: shifted_style,
                },
                Run {
                    text: "strike".to_string(),
                    style: struck_style,
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let base_y = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "base" => Some(fragment.baseline_y),
                _ => None,
            })
            .expect("base text");
        let up_y = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "up" => Some(fragment.baseline_y),
                _ => None,
            })
            .expect("shifted text");

        assert!(
            up_y > base_y,
            "expected shifted baseline above base baseline, base_y={base_y}, up_y={up_y}"
        );
        assert!(
            page.items
                .iter()
                .any(|item| matches!(item, LayoutItem::Line { .. }))
        );
    }

    #[test]
    fn lays_out_superscript_as_smaller_shifted_text() {
        let mut document = Document::default();
        let mut super_style = CharacterStyle::default();
        super_style.baseline_shift_half_points = 6;
        super_style.font_size_scale_percent = 65;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    text: "base".to_string(),
                    style: Default::default(),
                },
                Run {
                    text: "sup".to_string(),
                    style: super_style,
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let fragment_for = |text: &str| {
            layout.pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text == text => Some(fragment),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing text fragment {text}"))
        };
        let base = fragment_for("base");
        let sup = fragment_for("sup");

        assert!((base.style.font_size_points() - 12.0).abs() < 0.01);
        assert!((sup.style.font_size_points() - 7.8).abs() < 0.01);
        assert!(sup.baseline_y > base.baseline_y);
    }

    #[test]
    fn lays_out_overline_as_passive_line_above_text() {
        let mut overline_style = CharacterStyle::default();
        overline_style.overline = true;
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "over".to_string(),
                style: overline_style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let fragment = text_fragment_for(&layout.pages[0], "over");
        let line_y = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Line { y1, y2, .. } if (*y1 - *y2).abs() < 0.01 => Some(*y1),
                _ => None,
            })
            .expect("overline stroke");

        assert!(
            line_y > fragment.baseline_y,
            "overline should sit above text baseline, line_y={line_y}, baseline={}",
            fragment.baseline_y
        );
    }

    #[test]
    fn lays_out_double_strikeout_as_two_passive_lines() {
        let mut double_style = CharacterStyle::default();
        double_style.strike = true;
        double_style.double_strike = true;
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "double".to_string(),
                style: double_style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let strike_lines = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Line { y1, y2, .. } if (*y1 - *y2).abs() < 0.01 => Some(*y1),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(strike_lines.len(), 2);
        assert!((strike_lines[0] - strike_lines[1]).abs() >= 2.0);
    }

    #[test]
    fn lays_out_word_underline_variants_as_passive_lines() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 0,
                green: 0,
                blue: 0,
            },
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
        ];
        let mut double_style = CharacterStyle::default();
        double_style.underline = UnderlineStyle::Double;
        double_style.underline_color_index = Some(2);
        let mut wave_style = CharacterStyle::default();
        wave_style.underline = UnderlineStyle::Wave;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    text: "double".to_string(),
                    style: double_style,
                },
                Run {
                    text: "wave".to_string(),
                    style: wave_style,
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let underlines = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Underline {
                    style,
                    color,
                    width,
                    ..
                } => Some((*style, *color, *width)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(underlines.len(), 2);
        assert_eq!(underlines[0].0, UnderlineStyle::Double);
        assert_eq!(underlines[0].1.red, 1.0);
        assert_eq!(underlines[1].0, UnderlineStyle::Wave);
        assert!(underlines.iter().all(|(_, _, width)| *width > 0.0));
    }

    #[test]
    fn lays_out_word_only_underline_as_separate_word_segments() {
        let mut document = Document::default();
        let mut style = CharacterStyle::default();
        style.underline = UnderlineStyle::Words;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Two words".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let underlines = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Underline {
                    x, width, style, ..
                } => Some((*x, *width, *style)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(underlines.len(), 2);
        assert_eq!(underlines[0].2, UnderlineStyle::Single);
        assert_eq!(underlines[1].2, UnderlineStyle::Single);
        assert!(underlines[0].1 > 0.0);
        assert!(underlines[1].1 > 0.0);
        assert!(
            underlines[1].0 > underlines[0].0 + underlines[0].1,
            "expected a visible skipped gap between word underline segments: {underlines:?}"
        );
    }

    #[test]
    fn lays_out_character_border_as_passive_box_lines() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
        ];
        let mut style = CharacterStyle::default();
        style.border = TableCellBorder {
            visible: true,
            width_twips: 80,
            color_index: Some(1),
            ..TableCellBorder::default()
        };
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Boxed".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let border_lines = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Line { width, color, .. } if (*width - 4.0).abs() < 0.01 => {
                    Some(*color)
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(border_lines.len(), 4);
        assert!(border_lines.iter().all(|color| {
            *color
                == PdfColor {
                    red: 1.0,
                    green: 0.0,
                    blue: 0.0,
                }
        }));
    }

    #[test]
    fn lays_out_border_styles_as_passive_line_styles() {
        let mut document = Document::default();
        let mut character_style = CharacterStyle::default();
        character_style.border = TableCellBorder {
            visible: true,
            width_twips: 40,
            color_index: None,
            style: BorderStyle::Dashed,
            ..TableCellBorder::default()
        };
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 60,
            color_index: None,
            style: BorderStyle::Double,
            ..TableCellBorder::default()
        };
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "Styled".to_string(),
                style: character_style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let line_styles = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Line { style, .. } => Some(*style),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(line_styles.contains(&LineStyle::Double));
        assert!(line_styles.contains(&LineStyle::Dashed));
    }

    #[test]
    fn lays_out_wavy_borders_as_passive_line_styles() {
        let mut document = Document::default();
        let mut character_style = CharacterStyle::default();
        character_style.border = TableCellBorder {
            visible: true,
            width_twips: 40,
            color_index: None,
            style: BorderStyle::Wavy,
            ..TableCellBorder::default()
        };
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 60,
            color_index: None,
            style: BorderStyle::Wavy,
            ..TableCellBorder::default()
        };
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "Wavy".to_string(),
                style: character_style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let wavy_lines = layout.pages[0]
            .items
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    LayoutItem::Line {
                        style: LineStyle::Wavy,
                        ..
                    }
                )
            })
            .count();

        assert!(wavy_lines >= 2);
    }

    #[test]
    fn lays_out_paragraph_border_spacing_as_passive_offset_lines() {
        let spaced = crate::rtf::parse_rtf(r"{\rtf1\brdrb\brdrs\brsp240 Spaced\par}").unwrap();
        let unspaced = crate::rtf::parse_rtf(r"{\rtf1\brdrb\brdrs Plain\par}").unwrap();

        let spaced_layout = LayoutEngine::layout(&spaced.document);
        let unspaced_layout = LayoutEngine::layout(&unspaced.document);
        let bottom_line_y = |page: &LayoutPage| {
            page.items
                .iter()
                .filter_map(|item| match item {
                    LayoutItem::Line { y1, y2, .. } if (*y1 - *y2).abs() < 0.01 => Some(*y1),
                    _ => None,
                })
                .min_by(|a, b| a.total_cmp(b))
                .expect("bottom paragraph border")
        };

        assert!(
            bottom_line_y(&spaced_layout.pages[0])
                < bottom_line_y(&unspaced_layout.pages[0]) - 10.0
        );
    }

    #[test]
    fn lays_out_character_border_spacing_as_larger_passive_box() {
        let spaced = crate::rtf::parse_rtf(r"{\rtf1\chbrdr\brdrs\brsp240 Boxed\par}").unwrap();
        let unspaced = crate::rtf::parse_rtf(r"{\rtf1\chbrdr\brdrs Boxed\par}").unwrap();

        let spaced_layout = LayoutEngine::layout(&spaced.document);
        let unspaced_layout = LayoutEngine::layout(&unspaced.document);
        let box_height = |page: &LayoutPage| {
            let ys = page
                .items
                .iter()
                .filter_map(|item| match item {
                    LayoutItem::Line { y1, y2, .. } => Some((*y1, *y2)),
                    _ => None,
                })
                .flat_map(|(y1, y2)| [y1, y2])
                .collect::<Vec<_>>();
            let min_y = ys
                .iter()
                .copied()
                .min_by(|a, b| a.total_cmp(b))
                .expect("min y");
            let max_y = ys
                .iter()
                .copied()
                .max_by(|a, b| a.total_cmp(b))
                .expect("max y");
            max_y - min_y
        };

        assert!(box_height(&spaced_layout.pages[0]) > box_height(&unspaced_layout.pages[0]) + 20.0);
    }

    #[test]
    fn wraps_optional_hyphen_as_visible_only_at_line_end() {
        let document = Document::default();
        let style = CharacterStyle::default();
        let paragraph = Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Alpha\u{00ad}Beta".to_string(),
                style: style.clone(),
            }],
        };

        let markers = test_markers("1", "1");
        let wide_lines = wrap_paragraph(&paragraph, 500.0, &markers, &document);
        assert_eq!(line_text(&wide_lines[0]), "AlphaBeta");

        let narrow_width = measure_text("Alpha", &style) + 1.0;
        let narrow_lines = wrap_paragraph(&paragraph, narrow_width, &markers, &document);
        assert_eq!(narrow_lines.len(), 2);
        assert_eq!(line_text(&narrow_lines[0]), "Alpha-");
        assert_eq!(line_text(&narrow_lines[1]), "Beta");
    }

    #[test]
    fn passive_auto_hyphenation_breaks_long_overflow_words() {
        let document = Document::default();
        let style = CharacterStyle::default();
        let word = "Antidisestablishmentarianism";
        let mut hyphenated_style = ParagraphStyle::default();
        hyphenated_style.auto_hyphenation = true;
        let hyphenated = Paragraph {
            style: hyphenated_style,
            runs: vec![Run {
                text: word.to_string(),
                style: style.clone(),
            }],
        };
        let plain = Paragraph {
            style: Default::default(),
            runs: hyphenated.runs.clone(),
        };

        let markers = test_markers("1", "1");
        let width = measure_text("Antidis", &style);
        let plain_lines = wrap_paragraph(&plain, width, &markers, &document);
        let hyphenated_lines = wrap_paragraph(&hyphenated, width, &markers, &document);

        assert_eq!(plain_lines.len(), 1);
        assert!(hyphenated_lines.len() > 1);
        assert!(line_text(&hyphenated_lines[0]).ends_with('-'));
        let joined = hyphenated_lines
            .iter()
            .map(line_text)
            .collect::<String>()
            .replace('-', "");
        assert_eq!(joined, word);
    }

    #[test]
    fn passive_auto_hyphenation_respects_hot_zone() {
        let document = Document::default();
        let style = CharacterStyle::default();
        let word = "Antidisestablishment";
        let tight_zone_style = ParagraphStyle {
            auto_hyphenation: true,
            hyphenation_zone_twips: 0,
            ..ParagraphStyle::default()
        };
        let wide_zone_style = ParagraphStyle {
            hyphenation_zone_twips: 1_440,
            ..tight_zone_style.clone()
        };
        let tight_zone = Paragraph {
            style: tight_zone_style.clone(),
            runs: vec![Run {
                text: word.to_string(),
                style: style.clone(),
            }],
        };
        let wide_zone = Paragraph {
            style: wide_zone_style,
            runs: tight_zone.runs.clone(),
        };

        let markers = test_markers("1", "1");
        let width = measure_text(word, &style) - 1.0;
        let tight_lines = wrap_paragraph(&tight_zone, width, &markers, &document);
        let wide_lines = wrap_paragraph(&wide_zone, width, &markers, &document);

        assert!(tight_lines.len() > 1);
        assert!(line_text(&tight_lines[0]).ends_with('-'));
        assert_eq!(wide_lines.len(), 1);
        assert_eq!(line_text(&wide_lines[0]), word);
    }

    #[test]
    fn passive_auto_hyphenation_respects_capital_word_suppression() {
        let document = Document::default();
        let style = CharacterStyle::default();
        let word = "ANTIDISESTABLISHMENTARIANISM";
        let mut hyphenated_style = ParagraphStyle::default();
        hyphenated_style.auto_hyphenation = true;
        let mut suppressed_style = hyphenated_style.clone();
        suppressed_style.hyphenate_caps = false;
        let hyphenated = Paragraph {
            style: hyphenated_style,
            runs: vec![Run {
                text: word.to_string(),
                style: style.clone(),
            }],
        };
        let suppressed = Paragraph {
            style: suppressed_style,
            runs: hyphenated.runs.clone(),
        };

        let markers = test_markers("1", "1");
        let width = measure_text("ANTIDIS", &style);
        let hyphenated_lines = wrap_paragraph(&hyphenated, width, &markers, &document);
        let suppressed_lines = wrap_paragraph(&suppressed, width, &markers, &document);

        assert!(hyphenated_lines.len() > 1);
        assert!(line_text(&hyphenated_lines[0]).ends_with('-'));
        assert_eq!(suppressed_lines.len(), 1);
        assert_eq!(line_text(&suppressed_lines[0]), word);
    }

    #[test]
    fn passive_auto_hyphenation_limits_consecutive_hyphenated_lines() {
        let document = Document::default();
        let style = CharacterStyle::default();
        let word = "AntidisestablishmentarianismAntidisestablishmentarianism";
        let mut unlimited_style = ParagraphStyle::default();
        unlimited_style.auto_hyphenation = true;
        let mut limited_style = unlimited_style.clone();
        limited_style.max_consecutive_hyphenated_lines = Some(1);
        let unlimited = Paragraph {
            style: unlimited_style,
            runs: vec![Run {
                text: word.to_string(),
                style: style.clone(),
            }],
        };
        let limited = Paragraph {
            style: limited_style,
            runs: unlimited.runs.clone(),
        };

        let markers = test_markers("1", "1");
        let width = measure_text("Antidis", &style);
        let unlimited_lines = wrap_paragraph(&unlimited, width, &markers, &document);
        let limited_lines = wrap_paragraph(&limited, width, &markers, &document);
        let unlimited_hyphens = unlimited_lines
            .iter()
            .filter(|line| line_text(line).ends_with('-'))
            .count();
        let limited_hyphens = limited_lines
            .iter()
            .filter(|line| line_text(line).ends_with('-'))
            .count();

        assert!(unlimited_hyphens > 1);
        assert_eq!(limited_lines.len(), 2);
        assert!(line_text(&limited_lines[0]).ends_with('-'));
        assert_eq!(limited_hyphens, 1);
        let joined = limited_lines
            .iter()
            .map(line_text)
            .collect::<String>()
            .replace('-', "");
        assert_eq!(joined, word);
    }

    #[test]
    fn keeps_nonbreaking_space_segments_together_when_wrapping() {
        let document = Document::default();
        let style = CharacterStyle::default();
        let paragraph = Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "A\u{00a0}B C".to_string(),
                style: style.clone(),
            }],
        };

        let narrow_width = measure_text("A ", &style);
        let markers = test_markers("1", "1");
        let lines = wrap_paragraph(&paragraph, narrow_width, &markers, &document);

        assert_eq!(lines.len(), 2);
        assert_eq!(line_text(&lines[0]), "A\u{00a0}B ");
        assert_eq!(line_text(&lines[1]), "C");
    }

    #[test]
    fn keeps_nonbreaking_hyphen_segments_together_when_wrapping() {
        let document = Document::default();
        let style = CharacterStyle::default();
        let paragraph = Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "A\u{2011}B C".to_string(),
                style: style.clone(),
            }],
        };

        let narrow_width = measure_text("A-", &style);
        let markers = test_markers("1", "1");
        let lines = wrap_paragraph(&paragraph, narrow_width, &markers, &document);

        assert_eq!(lines.len(), 2);
        assert_eq!(line_text(&lines[0]), "A\u{2011}B ");
        assert_eq!(line_text(&lines[1]), "C");
    }

    #[test]
    fn no_wrap_paragraph_suppresses_soft_line_breaks() {
        let document = Document::default();
        let style = CharacterStyle::default();
        let mut paragraph = Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Alpha Beta".to_string(),
                style: style.clone(),
            }],
        };
        let narrow_width = measure_text("Alpha ", &style);

        let markers = test_markers("1", "1");
        let wrapped = wrap_paragraph(&paragraph, narrow_width, &markers, &document);
        assert_eq!(wrapped.len(), 2);

        paragraph.style.no_wrap = true;
        let unwrapped = wrap_paragraph(&paragraph, narrow_width, &markers, &document);
        assert_eq!(unwrapped.len(), 1);
        assert_eq!(line_text(&unwrapped[0]), "Alpha Beta");
    }

    #[test]
    fn lays_out_drop_cap_as_enlarged_first_visible_character() {
        let mut document = Document::default();
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.drop_cap_lines = 3;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "Dropped paragraph".to_string(),
                style: CharacterStyle::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let fragments = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(layout_text(&layout.pages[0]), "Dropped paragraph");
        assert_eq!(fragments[0].text, "D");
        assert_eq!(fragments[0].style.font_size_half_points, 72);
        assert!(fragments.iter().skip(1).all(|fragment| {
            fragment.style.font_size_half_points == CharacterStyle::default().font_size_half_points
        }));
    }

    #[test]
    fn hidden_runs_do_not_layout_as_text() {
        let mut document = Document::default();
        let mut hidden_style = CharacterStyle::default();
        hidden_style.hidden = true;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    text: "Visible".to_string(),
                    style: Default::default(),
                },
                Run {
                    text: "Hidden".to_string(),
                    style: hidden_style,
                },
                Run {
                    text: "Shown".to_string(),
                    style: Default::default(),
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let text = layout_text(&layout.pages[0]);

        assert_eq!(text, "VisibleShown");
        assert!(!text.contains("Hidden"));
    }

    #[test]
    fn lays_out_text_with_bounded_font_size() {
        let mut document = Document::default();
        let mut style = CharacterStyle::default();
        style.font_size_half_points = 96;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Bounded".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let fragment = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "Bounded" => Some(fragment),
                _ => None,
            })
            .expect("bounded text");

        assert_eq!(fragment.style.font_size_half_points, 96);
        assert!(fragment.baseline_y.is_finite());
    }

    #[test]
    fn character_spacing_expands_measured_line_width() {
        let mut expanded = CharacterStyle::default();
        expanded.character_spacing_twips = 200;
        let plain_width = measure_text("ABC", &CharacterStyle::default());
        let expanded_width = measure_text("ABC", &expanded);

        assert!((expanded_width - plain_width - 20.0).abs() < 0.01);

        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "ABC".to_string(),
                style: expanded,
            }],
        })];
        let layout = LayoutEngine::layout(&document);
        let fragment = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "ABC" => Some(fragment),
                _ => None,
            })
            .expect("expanded text");

        assert_eq!(fragment.style.character_spacing_twips, 200);
    }

    #[test]
    fn passive_kerning_reduces_measured_pair_width_when_threshold_matches() {
        let mut kerned = CharacterStyle {
            character_kerning_half_points: 2,
            ..Default::default()
        };
        let plain_width = measure_text("AV", &CharacterStyle::default());
        let kerned_width = measure_text("AV", &kerned);

        assert!(kerned_width < plain_width);
        assert!((plain_width - kerned_width - 1.32).abs() < 0.01);

        kerned.character_kerning_half_points = 48;
        assert!((measure_text("AV", &kerned) - plain_width).abs() < 0.01);
    }

    #[test]
    fn zero_width_formatting_marks_do_not_measure_or_emit_glyphs() {
        let mut spaced = CharacterStyle::default();
        spaced.character_spacing_twips = 200;
        let marked_text = "A\u{200b}\u{feff}\u{200c}\u{200d}\u{200e}\u{200f}B";
        assert!((measure_text(marked_text, &spaced) - measure_text("AB", &spaced)).abs() < 0.01);

        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: marked_text.to_string(),
                style: spaced,
            }],
        })];
        let layout = LayoutEngine::layout(&document);
        let rendered_text = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment.text.as_str()),
                _ => None,
            })
            .collect::<String>();

        assert_eq!(rendered_text, "AB");
    }

    #[test]
    fn character_scaling_changes_measured_line_width() {
        let mut expanded = CharacterStyle::default();
        expanded.character_scaling_percent = 150;
        let plain_width = measure_text("ABC", &CharacterStyle::default());
        let expanded_width = measure_text("ABC", &expanded);

        assert!((expanded_width - (plain_width * 1.5)).abs() < 0.01);

        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "ABC".to_string(),
                style: expanded,
            }],
        })];
        let layout = LayoutEngine::layout(&document);
        let fragment = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "ABC" => Some(fragment),
                _ => None,
            })
            .expect("scaled text");

        assert_eq!(fragment.style.character_scaling_percent, 150);
    }

    #[test]
    fn applies_page_geometry_to_layout() {
        let mut document = Document::default();
        document.page.width_twips = 14_400;
        document.page.height_twips = 7_200;
        document.page.margin_left_twips = 720;
        document.page.gutter_twips = 360;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Geometry".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.width, 720.0);
        assert_eq!(layout.height, 360.0);
        assert!(
            layout.pages[0]
                .items
                .iter()
                .any(|item| matches!(item, LayoutItem::Text(fragment) if fragment.x == 54.0))
        );
    }

    #[test]
    fn vertically_aligns_page_body_content_inside_margins() {
        let mut top_document = small_test_page_document();
        top_document.blocks = vec![paragraph_with_text("Body")];

        let mut bottom_document = top_document.clone();
        bottom_document.page.vertical_alignment = PageVerticalAlignment::Bottom;

        let top_layout = LayoutEngine::layout(&top_document);
        let bottom_layout = LayoutEngine::layout(&bottom_document);
        let top_baseline = text_baseline_for(&top_layout.pages[0], "Body");
        let bottom_baseline = text_baseline_for(&bottom_layout.pages[0], "Body");

        assert!(
            bottom_baseline < top_baseline - 100.0,
            "bottom alignment should move body content down"
        );
        assert!(
            bottom_baseline > bottom_layout.pages[0].geometry.margin_bottom,
            "bottom-aligned body should stay above the bottom margin"
        );
    }

    #[test]
    fn applies_page_gutter_as_bounded_passive_binding_space() {
        let mut document = Document::default();
        document.page.width_twips = 5_760;
        document.page.margin_left_twips = 720;
        document.page.margin_right_twips = 720;
        document.page.gutter_twips = 720;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Gutter".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let text = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "Gutter" => Some(fragment),
                _ => None,
            })
            .expect("gutter text");

        assert!((text.x - 72.0).abs() < 0.01);
        assert!((layout.pages[0].geometry.content_width - 180.0).abs() < 0.01);
    }

    #[test]
    fn applies_right_to_left_gutter_as_right_binding_space() {
        let mut document = Document::default();
        document.page.width_twips = 5_760;
        document.page.margin_left_twips = 720;
        document.page.margin_right_twips = 360;
        document.page.gutter_twips = 720;
        document.page.gutter_on_right = true;
        document.blocks = vec![paragraph_with_text("RtlGutter")];

        let layout = LayoutEngine::layout(&document);
        let text = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "RtlGutter" => Some(fragment),
                _ => None,
            })
            .expect("gutter text");

        assert_eq!(text.x, 36.0);
        assert!((layout.pages[0].geometry.content_width - 198.0).abs() < 0.01);
    }

    #[test]
    fn mirrors_page_gutter_binding_space_on_even_pages() {
        let mut document = Document::default();
        document.page.width_twips = 5_760;
        document.page.margin_left_twips = 720;
        document.page.margin_right_twips = 360;
        document.page.gutter_twips = 720;
        document.page.mirror_margins = true;
        document.blocks = vec![
            paragraph_with_text("Odd"),
            Block::PageBreak,
            paragraph_with_text("Even"),
        ];

        let layout = LayoutEngine::layout(&document);
        let odd_x = text_x(&layout.pages[0], "Odd").expect("odd page text");
        let even_x = text_x(&layout.pages[1], "Even").expect("even page text");

        assert!((odd_x - 72.0).abs() < 0.01, "odd_x={odd_x}");
        assert!((even_x - 18.0).abs() < 0.01, "even_x={even_x}");
        assert!((layout.pages[0].geometry.content_width - 198.0).abs() < 0.01);
        assert!((layout.pages[1].geometry.content_width - 198.0).abs() < 0.01);
    }

    #[test]
    fn mirrors_right_to_left_gutter_binding_space_by_page_parity() {
        let mut document = Document::default();
        document.page.width_twips = 5_760;
        document.page.margin_left_twips = 720;
        document.page.margin_right_twips = 360;
        document.page.gutter_twips = 720;
        document.page.gutter_on_right = true;
        document.page.mirror_margins = true;
        document.blocks = vec![
            paragraph_with_text("Odd"),
            Block::PageBreak,
            paragraph_with_text("Even"),
        ];

        let layout = LayoutEngine::layout(&document);
        let odd_x = text_x(&layout.pages[0], "Odd").expect("odd page text");
        let even_x = text_x(&layout.pages[1], "Even").expect("even page text");

        assert_eq!(odd_x, 36.0);
        assert_eq!(even_x, 54.0);
        assert!((layout.pages[0].geometry.content_width - 198.0).abs() < 0.01);
        assert!((layout.pages[1].geometry.content_width - 198.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_landscape_page_dimensions() {
        let mut document = Document::default();
        document.page.width_twips = 15_840;
        document.page.height_twips = 12_240;
        document.page.landscape = true;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Landscape".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.width, 792.0);
        assert_eq!(layout.height, 612.0);
    }

    #[test]
    fn lays_out_text_color_and_highlight_background() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
            Color {
                red: 0,
                green: 255,
                blue: 0,
            },
        ];
        let mut style = CharacterStyle::default();
        style.color_index = 1;
        style.highlight_index = Some(2);
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Marked".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(|item| {
            matches!(
                item,
                LayoutItem::Highlight { color, .. }
                    if *color == PdfColor { red: 0.0, green: 1.0, blue: 0.0 }
            )
        }));
        assert!(page.items.iter().any(|item| {
            matches!(
                item,
                LayoutItem::Text(fragment)
                    if fragment.text == "Marked"
                        && fragment.color == PdfColor { red: 1.0, green: 0.0, blue: 0.0 }
            )
        }));
    }

    #[test]
    fn lays_out_form_field_shading_as_passive_gray_background() {
        let mut document = Document::default();
        let mut style = CharacterStyle::default();
        style.form_field_shading = true;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Form value".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        let form_shading = page
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Highlight {
                    x, width, color, ..
                } if *color
                    == (PdfColor {
                        red: 0.82,
                        green: 0.82,
                        blue: 0.82,
                    }) =>
                {
                    Some((*x, *x + *width))
                }
                _ => None,
            })
            .reduce(|acc, bounds| (acc.0.min(bounds.0), acc.1.max(bounds.1)))
            .expect("form shading");
        let text_bounds = page
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => {
                    let width = measure_text_with_family(
                        &fragment.text,
                        &fragment.style,
                        fragment.font_family,
                    );
                    Some((fragment.x, fragment.x + width))
                }
                _ => None,
            })
            .reduce(|acc, bounds| (acc.0.min(bounds.0), acc.1.max(bounds.1)))
            .expect("form text");

        assert!(
            form_shading.0 < text_bounds.0,
            "form shading should start before text: shading={form_shading:?}, text={text_bounds:?}"
        );
        assert!(
            form_shading.1 > text_bounds.1,
            "form shading should end after text: shading={form_shading:?}, text={text_bounds:?}"
        );
        assert!(layout_text(page).contains("Form value"));
    }

    #[test]
    fn lays_out_character_shading_intensity_as_tinted_background() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
        ];
        let mut style = CharacterStyle::default();
        style.highlight_index = Some(1);
        style.highlight_shading_basis_points = 5_000;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Tinted".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let color = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight { color, .. } => Some(*color),
                _ => None,
            })
            .expect("highlight");

        assert_eq!(
            color,
            PdfColor {
                red: 1.0,
                green: 0.5,
                blue: 0.5
            }
        );
    }

    #[test]
    fn lays_out_paragraph_shading_intensity_as_tinted_background() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 0,
                green: 0,
                blue: 255,
            },
        ];
        let mut paragraph_style = ParagraphStyle {
            shading_color_index: Some(1),
            shading_basis_points: 2_500,
            ..ParagraphStyle::default()
        };
        paragraph_style.space_after_twips = 0;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "Tinted paragraph".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let color = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight { color, .. } => Some(*color),
                _ => None,
            })
            .expect("paragraph shading");

        assert_eq!(
            color,
            PdfColor {
                red: 0.75,
                green: 0.75,
                blue: 1.0
            }
        );
    }

    #[test]
    fn lays_out_table_cell_shading_intensity_as_tinted_background() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
        ];
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: false,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: None,
                left_offset_twips: 0,
                cell_gap_twips: 0,
                alignment: Default::default(),
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: Some(1),
                    shading_basis_points: 5_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    vertical_align: Default::default(),
                    horizontal_merge: Default::default(),
                    vertical_merge: Default::default(),
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: "Tinted cell".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let color = layout.pages[0]
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Highlight { color, .. } => Some(*color),
                _ => None,
            })
            .expect("table cell shading");

        assert_eq!(
            color,
            PdfColor {
                red: 1.0,
                green: 0.5,
                blue: 0.5
            }
        );
    }

    #[test]
    fn lays_out_footnotes_after_body_with_separator() {
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Body".to_string(),
                style: Default::default(),
            }],
        })];
        document.footnotes = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "* Footnote text".to_string(),
                style: Default::default(),
            }],
        }];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let text = layout_text(page);

        assert!(text.contains("Body"));
        assert!(text.contains("1. * Footnote text"));
        let label = text_fragment_for(page, "1");
        let note_text = first_text_fragment_except(page, &["Body", "1"]);
        assert_eq!(
            label.style.font_size_scale_percent,
            PASSIVE_NOTE_LABEL_FONT_SCALE_PERCENT
        );
        assert!(label.baseline_y > note_text.baseline_y);
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, x2, width, .. }
                if (*x1 - 72.0).abs() < 0.01
                    && (*x2 - 216.0).abs() < 0.01
                    && (*width - 0.5).abs() < 0.01
        )));
    }

    #[test]
    fn lays_out_bottom_footnotes_at_page_bottom() {
        let mut document = Document::default();
        document.footnote_placement = FootnotePlacement::BottomOfPage;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Body".to_string(),
                style: Default::default(),
            }],
        })];
        document.footnotes = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Bottom footnote".to_string(),
                style: Default::default(),
            }],
        }];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let body_y = text_baseline_for(page, "Body");
        let note_y = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text.contains("Bottom") => {
                    Some(fragment.baseline_y)
                }
                _ => None,
            })
            .expect("bottom footnote text");
        let separator_y = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Line { x1, x2, y1, y2, .. }
                    if (*x1 - 72.0).abs() < 0.01
                        && (*x2 - 216.0).abs() < 0.01
                        && (*y1 - *y2).abs() < 0.01 =>
                {
                    Some(*y1)
                }
                _ => None,
            })
            .expect("footnote separator");

        assert!(body_y > 650.0);
        assert!(
            note_y < 90.0,
            "bottom footnote baseline should stay near page bottom, got {note_y}"
        );
        assert!(
            (separator_y - 92.0).abs() < 0.01,
            "separator should sit above bottom footnote band, got {separator_y}"
        );
    }

    #[test]
    fn lays_out_endnotes_after_body_with_separator() {
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Body".to_string(),
                style: Default::default(),
            }],
        })];
        document.endnotes = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "* Endnote text".to_string(),
                style: Default::default(),
            }],
        }];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let text = layout_text(page);

        assert!(text.contains("Body"));
        assert!(text.contains("1. * Endnote text"));
        let label = text_fragment_for(page, "1");
        let note_text = first_text_fragment_except(page, &["Body", "1"]);
        assert_eq!(
            label.style.font_size_scale_percent,
            PASSIVE_NOTE_LABEL_FONT_SCALE_PERCENT
        );
        assert!(label.baseline_y > note_text.baseline_y);
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, x2, width, .. }
                if (*x1 - 72.0).abs() < 0.01
                    && (*x2 - 216.0).abs() < 0.01
                    && (*width - 0.5).abs() < 0.01
        )));
    }

    #[test]
    fn lays_out_end_of_document_endnotes_on_final_page() {
        let mut document = Document::default();
        document.endnote_placement = EndnotePlacement::EndOfDocument;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Body".to_string(),
                style: Default::default(),
            }],
        })];
        document.endnotes = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Endnote text".to_string(),
                style: Default::default(),
            }],
        }];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert!(layout_text(&layout.pages[0]).contains("Body"));
        assert!(!layout_text(&layout.pages[0]).contains("Endnote text"));
        assert!(layout_text(&layout.pages[1]).contains("1. Endnote text"));
    }

    #[test]
    fn lays_out_paragraph_shading_background() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 240,
                green: 240,
                blue: 0,
            },
        ];
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.shading_color_index = Some(1);
        paragraph_style.left_indent_twips = 720;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "Shaded paragraph".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(|item| {
            matches!(
                item,
                LayoutItem::Highlight { x, width, color, .. }
                    if (*x - 108.0).abs() < 0.01
                        && (*width - 432.0).abs() < 0.01
                        && *color == PdfColor {
                            red: 240.0 / 255.0,
                            green: 240.0 / 255.0,
                            blue: 0.0
                        }
            )
        }));
        assert!(layout_text(page).contains("Shaded paragraph"));
    }

    #[test]
    fn lays_out_shading_patterns_as_passive_lines() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 240,
                green: 0,
                blue: 0,
            },
        ];
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.shading_color_index = Some(1);
        paragraph_style.shading_basis_points = 2_500;
        paragraph_style.shading_pattern = ShadingPattern::Horizontal;
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: paragraph_style,
                runs: vec![Run {
                    text: "Horizontal paragraph".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::Table(Table {
                column_widths_twips: vec![1440],
                borders_visible: false,
                preserve_authored_widths: false,
                rows: vec![TableRow {
                    height_twips: None,
                    left_offset_twips: 0,
                    cell_gap_twips: 60,
                    alignment: TableRowAlignment::Left,
                    repeat_header: false,
                    keep_together: false,
                    cells: vec![TableCell {
                        shading_color_index: Some(1),
                        shading_basis_points: 5_000,
                        shading_pattern: ShadingPattern::Vertical,
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "Vertical cell".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    }],
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let red = PdfColor {
            red: 240.0 / 255.0,
            green: 0.0,
            blue: 0.0,
        };

        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Highlight { color, .. }
                if color.red > 0.9 && color.green < 0.8 && color.blue < 0.8
        )));
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { y1, y2, width, color, style, .. }
                if (*y1 - *y2).abs() < 0.01
                    && (*width - 0.35).abs() < 0.01
                    && *color == red
                    && *style == LineStyle::Solid
        )));
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, x2, width, color, style, .. }
                if (*x1 - *x2).abs() < 0.01
                    && (*width - 0.35).abs() < 0.01
                    && *color == red
                    && *style == LineStyle::Solid
        )));
        assert!(layout_text(page).contains("Horizontal paragraph"));
        assert!(layout_text(page).contains("Vertical cell"));
    }

    #[test]
    fn lays_out_extended_shading_patterns_as_passive_lines() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 240,
                green: 0,
                blue: 0,
            },
        ];
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.shading_color_index = Some(1);
        paragraph_style.shading_pattern = ShadingPattern::ForwardDiagonal;
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: paragraph_style,
                runs: vec![Run {
                    text: "Diagonal paragraph".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::Table(Table {
                column_widths_twips: vec![1440],
                borders_visible: false,
                preserve_authored_widths: false,
                rows: vec![TableRow {
                    height_twips: None,
                    left_offset_twips: 0,
                    cell_gap_twips: 60,
                    alignment: TableRowAlignment::Left,
                    repeat_header: false,
                    keep_together: false,
                    cells: vec![TableCell {
                        shading_color_index: Some(1),
                        shading_basis_points: 10_000,
                        shading_pattern: ShadingPattern::DarkCross,
                        padding: TableCellPadding::default(),
                        spacing: Default::default(),
                        borders: TableCellBorders::default(),
                        fit_text: false,
                        vertical_align: TableCellVerticalAlign::Top,
                        horizontal_merge: TableCellHorizontalMerge::None,
                        vertical_merge: TableCellVerticalMerge::None,
                        paragraphs: vec![Paragraph {
                            style: Default::default(),
                            runs: vec![Run {
                                text: "Dark cross cell".to_string(),
                                style: Default::default(),
                            }],
                        }],
                    }],
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let red = PdfColor {
            red: 240.0 / 255.0,
            green: 0.0,
            blue: 0.0,
        };

        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, y1, x2, y2, width, color, style }
                if (*x1 - *x2).abs() > 0.01
                    && (*y1 - *y2).abs() > 0.01
                    && (*width - 0.35).abs() < 0.01
                    && *color == red
                    && *style == LineStyle::Solid
        )));
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { y1, y2, width, color, .. }
                if (*y1 - *y2).abs() < 0.01
                    && (*width - 0.35).abs() < 0.01
                    && *color == red
        )));
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, x2, width, color, .. }
                if (*x1 - *x2).abs() < 0.01
                    && (*width - 0.35).abs() < 0.01
                    && *color == red
        )));
        assert!(layout_text(page).contains("Diagonal paragraph"));
        assert!(layout_text(page).contains("Dark cross cell"));
    }

    #[test]
    fn lays_out_paragraph_borders_as_passive_lines() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 30,
                green: 60,
                blue: 90,
            },
        ];
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 80,
            color_index: Some(1),
            ..TableCellBorder::default()
        };
        paragraph_style.borders.left = TableCellBorder {
            visible: true,
            width_twips: 40,
            color_index: None,
            ..TableCellBorder::default()
        };
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "Bordered".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, x2, width, color, .. }
                if (*x1 - 72.0).abs() < 0.01
                    && (*x2 - 540.0).abs() < 0.01
                    && (*width - 4.0).abs() < 0.01
                    && *color == PdfColor { red: 30.0 / 255.0, green: 60.0 / 255.0, blue: 90.0 / 255.0 }
        )));
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, x2, width, .. }
                if (*x1 - 72.0).abs() < 0.01
                    && (*x2 - 72.0).abs() < 0.01
                    && (*width - 2.0).abs() < 0.01
        )));
    }

    #[test]
    fn lays_out_paragraph_between_borders_as_passive_lines() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 200,
                green: 10,
                blue: 20,
            },
        ];
        let mut bordered = ParagraphStyle::default();
        bordered.borders.between = TableCellBorder {
            visible: true,
            width_twips: 60,
            color_index: Some(1),
            ..TableCellBorder::default()
        };

        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: bordered.clone(),
                runs: vec![Run {
                    text: "First".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::Paragraph(Paragraph {
                style: bordered,
                runs: vec![Run {
                    text: "Second".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let red = PdfColor {
            red: 200.0 / 255.0,
            green: 10.0 / 255.0,
            blue: 20.0 / 255.0,
        };
        let between_lines = page
            .items
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    LayoutItem::Line { x1, x2, y1, y2, width, color, .. }
                        if (*x2 - *x1).abs() > 100.0
                            && (*y1 - *y2).abs() < 0.01
                            && (*width - 3.0).abs() < 0.01
                            && *color == red
                )
            })
            .count();

        assert_eq!(between_lines, 1);
        assert!(layout_text(page).contains("First"));
        assert!(layout_text(page).contains("Second"));
    }

    #[test]
    fn skips_paragraph_between_border_when_next_paragraph_does_not_opt_in() {
        let mut document = Document::default();
        let mut bordered = ParagraphStyle::default();
        bordered.borders.between = TableCellBorder {
            visible: true,
            width_twips: 60,
            ..TableCellBorder::default()
        };
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: bordered,
                runs: vec![Run {
                    text: "First".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::Paragraph(Paragraph {
                style: ParagraphStyle::default(),
                runs: vec![Run {
                    text: "Second".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let between_lines = layout.pages[0]
            .items
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    LayoutItem::Line { x1, x2, y1, y2, width, .. }
                        if (*x2 - *x1).abs() > 100.0
                            && (*y1 - *y2).abs() < 0.01
                            && (*width - 3.0).abs() < 0.01
                )
            })
            .count();

        assert_eq!(between_lines, 0);
    }

    #[test]
    fn lays_out_table_cell_paragraph_shading_background() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 240,
                green: 240,
                blue: 0,
            },
        ];
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.shading_color_index = Some(1);
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: false,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: None,
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: paragraph_style,
                        runs: vec![Run {
                            text: "Cell paragraph".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(|item| {
            matches!(
                item,
                LayoutItem::Highlight { x, width, color, .. }
                    if (*x - 75.0).abs() < 0.01
                        && (*width - 66.0).abs() < 0.01
                        && *color == PdfColor {
                            red: 240.0 / 255.0,
                            green: 240.0 / 255.0,
                            blue: 0.0
                        }
            )
        }));
        assert!(layout_text(page).contains("Cell paragraph"));
    }

    #[test]
    fn lays_out_first_line_indent_only_on_first_body_line() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.left_indent_twips = 720;
        paragraph_style.first_line_indent_twips = 360;
        document_with_first_line_indent_body_test(paragraph_style);
    }

    #[test]
    fn lays_out_hanging_first_line_indent() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.left_indent_twips = 720;
        paragraph_style.first_line_indent_twips = -360;
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "First\nSecond".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let text_x = |text: &str| {
            layout.pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.trim() == text => Some(fragment.x),
                    _ => None,
                })
                .expect("body text")
        };

        assert!((text_x("First") - 90.0).abs() < 0.01);
        assert!((text_x("Second") - 108.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_right_indent_as_reduced_line_width() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.right_indent_twips = 7_200;
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "Alpha Bravo Charlie Delta Echo Foxtrot".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let baselines = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment.baseline_y),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(
            baselines
                .windows(2)
                .any(|pair| (pair[0] - pair[1]).abs() > 0.01)
        );
    }

    fn document_with_first_line_indent_body_test(paragraph_style: ParagraphStyle) {
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "First\nSecond".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let text_x = |text: &str| {
            layout.pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.trim() == text => Some(fragment.x),
                    _ => None,
                })
                .expect("body text")
        };

        assert!((text_x("First") - 126.0).abs() < 0.01);
        assert!((text_x("Second") - 108.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_paragraph_spacing_before_and_after() {
        let mut first_style = ParagraphStyle::default();
        first_style.space_after_twips = 400;
        let mut second_style = ParagraphStyle::default();
        second_style.space_before_twips = 200;
        let spaced_gap = paragraph_baseline_gap(first_style, second_style);
        let default_gap =
            paragraph_baseline_gap(ParagraphStyle::default(), ParagraphStyle::default());

        assert!((spaced_gap - default_gap - 30.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_auto_paragraph_spacing_with_bounded_passive_gap() {
        let mut first_style = ParagraphStyle::default();
        first_style.space_after_twips = 0;
        first_style.auto_space_after = true;
        let mut second_style = ParagraphStyle::default();
        second_style.space_before_twips = 0;
        second_style.auto_space_before = true;
        let auto_gap = paragraph_baseline_gap(first_style, second_style);

        let mut manual_first = ParagraphStyle::default();
        manual_first.space_after_twips = 0;
        let mut manual_second = ParagraphStyle::default();
        manual_second.space_before_twips = 0;
        let manual_gap = paragraph_baseline_gap(manual_first, manual_second);

        assert!(
            (auto_gap - manual_gap - 24.0).abs() < 0.01,
            "expected auto spacing to add bounded 24pt gap, manual={manual_gap}, auto={auto_gap}"
        );
    }

    #[test]
    fn collapses_contextual_spacing_between_adjacent_opt_in_paragraphs() {
        let mut normal_first = ParagraphStyle::default();
        normal_first.space_after_twips = 360;
        let mut normal_second = ParagraphStyle::default();
        normal_second.space_before_twips = 240;
        let normal_gap = paragraph_baseline_gap(normal_first.clone(), normal_second.clone());

        normal_first.contextual_spacing = true;
        normal_second.contextual_spacing = true;
        let contextual_gap = paragraph_baseline_gap(normal_first, normal_second);

        assert!(
            (normal_gap - contextual_gap - 30.0).abs() < 0.01,
            "contextual spacing should suppress 18pt after + 12pt before, normal={normal_gap}, contextual={contextual_gap}"
        );
    }

    fn paragraph_baseline_gap(first_style: ParagraphStyle, second_style: ParagraphStyle) -> f32 {
        let mut document = Document::default();
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: first_style,
                runs: vec![Run {
                    text: "First".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::Paragraph(Paragraph {
                style: second_style,
                runs: vec![Run {
                    text: "Second".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let baseline_y = |text: &str| {
            layout.pages[0]
                .items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text == text => {
                        Some(fragment.baseline_y)
                    }
                    _ => None,
                })
                .expect("paragraph text")
        };

        baseline_y("First") - baseline_y("Second")
    }

    #[test]
    fn lays_out_dot_tab_leaders_as_passive_text() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.tab_stops_twips = vec![1440];
        paragraph_style.tab_stop_leaders = vec![TabLeader::Dots];
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "Left\tRight".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text.starts_with("..."))
        ));
        assert!(page.items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Right" && (fragment.x - 144.0).abs() < 0.01)
        ));
    }

    #[test]
    fn lays_out_middle_dot_and_equals_tab_leaders_as_passive_text() {
        let paragraph = |leader| {
            let mut style = ParagraphStyle::default();
            style.tab_stops_twips = vec![1440];
            style.tab_stop_leaders = vec![leader];
            Block::Paragraph(Paragraph {
                style,
                runs: vec![Run {
                    text: "Left\tRight".to_string(),
                    style: Default::default(),
                }],
            })
        };
        let mut document = Document::default();
        document.blocks = vec![
            paragraph(TabLeader::MiddleDots),
            paragraph(TabLeader::Equals),
        ];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text.starts_with('\u{00b7}'))
        ));
        assert!(page.items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text.starts_with("==="))
        ));
    }

    #[test]
    fn lays_out_default_tab_width_from_document_settings() {
        let mut document = Document::default();
        document.default_tab_width_twips = 360;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: ParagraphStyle::default(),
            runs: vec![Run {
                text: "A\tB".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!((text_x(page, "B").expect("tabbed text") - 90.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_right_aligned_tab_stops() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.tab_stops_twips = vec![1440];
        paragraph_style.tab_stop_leaders = vec![TabLeader::Dots];
        paragraph_style.tab_stop_alignments = vec![TabAlignment::Right];
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "Left\t9".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let digit = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "9" => Some(fragment),
                _ => None,
            })
            .expect("right-aligned tab text");
        let digit_width =
            measure_text_with_family(&digit.text, &digit.style, PdfFontFamily::Helvetica);

        assert!((digit.x + digit_width - 144.0).abs() < 0.01);
        assert!(page.items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text.starts_with("..."))
        ));
    }

    #[test]
    fn lays_out_bar_tab_stops_as_passive_vertical_lines() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.tab_stops_twips = vec![720, 1440];
        paragraph_style.tab_stop_alignments = vec![TabAlignment::Bar, TabAlignment::Left];
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "Left\tRight".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, x2, .. } if (*x1 - 108.0).abs() < 0.01 && (*x2 - 108.0).abs() < 0.01
        )));
        assert!(
            (text_x(page, "Right").expect("tabbed text") - 144.0).abs() < 0.01,
            "bar tab should not consume the normal left tab stop"
        );
    }

    #[test]
    fn lays_out_header_bar_tab_stops_as_passive_vertical_lines() {
        let mut header_style = ParagraphStyle::default();
        header_style.tab_stops_twips = vec![720, 1440];
        header_style.tab_stop_alignments = vec![TabAlignment::Bar, TabAlignment::Left];
        let mut document = Document::default();
        document.header = vec![Paragraph {
            style: header_style,
            runs: vec![Run {
                text: "Head\tRight".to_string(),
                style: Default::default(),
            }],
        }];
        document.blocks = vec![paragraph_with_text("Body")];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, x2, .. } if (*x1 - 108.0).abs() < 0.01 && (*x2 - 108.0).abs() < 0.01
        )));
        assert!(
            (text_x(page, "Right").expect("header tabbed text") - 144.0).abs() < 0.01,
            "header bar tab should not consume the normal left tab stop"
        );
    }

    #[test]
    fn lays_out_table_cell_bar_tab_stops_as_passive_vertical_lines() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.tab_stops_twips = vec![360, 720];
        paragraph_style.tab_stop_alignments = vec![TabAlignment::Bar, TabAlignment::Left];
        let mut document = Document::default();
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: false,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: None,
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: None,
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    vertical_align: TableCellVerticalAlign::Top,
                    horizontal_merge: TableCellHorizontalMerge::None,
                    vertical_merge: TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: paragraph_style,
                        runs: vec![Run {
                            text: "A\tB".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, x2, .. } if (*x1 - 93.0).abs() < 0.01 && (*x2 - 93.0).abs() < 0.01
        )));
        assert!(
            (text_x(page, "B").expect("table tabbed text") - 111.0).abs() < 0.01,
            "table bar tab should not consume the normal left tab stop"
        );
    }

    #[test]
    fn lays_out_center_aligned_tab_stops() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.tab_stops_twips = vec![1440];
        paragraph_style.tab_stop_alignments = vec![TabAlignment::Center];
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "Left\tMM".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let centered = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "MM" => Some(fragment),
                _ => None,
            })
            .expect("center-aligned tab text");
        let text_width =
            measure_text_with_family(&centered.text, &centered.style, PdfFontFamily::Helvetica);

        assert!((centered.x + (text_width / 2.0) - 144.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_decimal_aligned_tab_stops() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.tab_stops_twips = vec![1440];
        paragraph_style.tab_stop_leaders = vec![TabLeader::Dots];
        paragraph_style.tab_stop_alignments = vec![TabAlignment::Decimal];
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "Amount\t123.45".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let amount = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "123.45" => Some(fragment),
                _ => None,
            })
            .expect("decimal-aligned tab text");
        let prefix_width = measure_text_with_family("123", &amount.style, PdfFontFamily::Helvetica);

        assert!((amount.x + prefix_width - 144.0).abs() < 0.01);
        assert!(page.items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text.starts_with("..."))
        ));
    }

    #[test]
    fn decimal_tabs_without_separator_fall_back_to_right_alignment() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.tab_stops_twips = vec![1440];
        paragraph_style.tab_stop_alignments = vec![TabAlignment::Decimal];
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "Amount\t123".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let amount = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "123" => Some(fragment),
                _ => None,
            })
            .expect("decimal fallback tab text");
        let width = measure_text_with_family(&amount.text, &amount.style, PdfFontFamily::Helvetica);

        assert!((amount.x + width - 144.0).abs() < 0.01);
    }

    #[test]
    fn resolves_safe_pdf_font_family_from_font_table() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Helvetica".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Courier New".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Modern,
                pitch: FontPitch::Default,
            },
        ];
        let mut style = CharacterStyle::default();
        style.font_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Mono".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.font_family == PdfFontFamily::Courier)
        ));
    }

    #[test]
    fn caller_font_metrics_drive_layout_widths_without_system_fonts() {
        let mut document = Document::default();
        document.fonts = vec![FontDef {
            index: 0,
            name: "Tuffy".to_string(),
            alternate_name: None,
            charset: None,
            code_page: None,
            family: FontFamilyHint::Swiss,
            pitch: FontPitch::Default,
        }];
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    text: "A".to_string(),
                    style: Default::default(),
                },
                Run {
                    text: "B".to_string(),
                    style: Default::default(),
                },
            ],
        })];
        let provider = FontProvider {
            assets: vec![FontAsset {
                family_names: vec!["Tuffy".to_string()],
                style: FontAssetStyle::default(),
                bytes: include_bytes!("../fixtures/fonts/Tuffy.ttf").to_vec(),
            }],
            limits: FontProviderLimits {
                max_asset_bytes: 256 * 1024,
                max_total_bytes: 256 * 1024,
                ..FontProviderLimits::default()
            },
        };
        provider.validate().unwrap();
        let metrics = provider.glyph_metrics_for_char("Tuffy", 'A').unwrap();

        let fallback_layout = LayoutEngine::layout(&document);
        let provided_layout = LayoutEngine::layout_with_font_provider(&document, Some(&provider));
        let fallback_advance = text_x(&fallback_layout.pages[0], "B").unwrap()
            - text_x(&fallback_layout.pages[0], "A").unwrap();
        let provided_advance = text_x(&provided_layout.pages[0], "B").unwrap()
            - text_x(&provided_layout.pages[0], "A").unwrap();
        let expected_advance = metrics.advance_points(CharacterStyle::default().font_size_points());

        assert!((provided_advance - expected_advance).abs() < 0.01);
        assert!(
            (provided_advance - fallback_advance).abs() > 0.1,
            "provided metrics should visibly differ from fallback width"
        );
    }

    #[test]
    fn caller_font_metrics_drive_line_height_without_system_fonts() {
        let mut document = Document::default();
        document.fonts = vec![FontDef {
            index: 0,
            name: "Tuffy".to_string(),
            alternate_name: None,
            charset: None,
            code_page: None,
            family: FontFamilyHint::Swiss,
            pitch: FontPitch::Default,
        }];
        let paragraph = Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "A".to_string(),
                style: Default::default(),
            }],
        };
        let provider = FontProvider {
            assets: vec![FontAsset {
                family_names: vec!["Tuffy".to_string()],
                style: FontAssetStyle::default(),
                bytes: include_bytes!("../fixtures/fonts/Tuffy.ttf").to_vec(),
            }],
            limits: FontProviderLimits {
                max_asset_bytes: 256 * 1024,
                max_total_bytes: 256 * 1024,
                ..FontProviderLimits::default()
            },
        };
        provider.validate().unwrap();
        let markers = test_markers("1", "1");
        let fallback_line =
            wrap_paragraph_with_font_provider(&paragraph, 500.0, &markers, &document, None)
                .into_iter()
                .next()
                .expect("fallback line");
        let supplied_line = wrap_paragraph_with_font_provider(
            &paragraph,
            500.0,
            &markers,
            &document,
            Some(&provider),
        )
        .into_iter()
        .next()
        .expect("supplied line");
        let expected_height = provider
            .glyph_metrics_for_char("Tuffy", 'A')
            .and_then(|metrics| {
                metrics.line_height_points(CharacterStyle::default().font_size_points())
            })
            .expect("line metrics")
            .clamp(
                CharacterStyle::default().font_size_points().max(1.0),
                CharacterStyle::default().font_size_points().max(1.0) * 2.0,
            )
            .max(empty_line().height);

        assert!((supplied_line.height - expected_height).abs() < 0.01);
        assert!(
            (supplied_line.height - fallback_line.height).abs() > 0.1,
            "provided metrics should visibly differ from fallback line height"
        );
    }

    #[test]
    fn resolves_symbol_font_family_from_charset_hint() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Helvetica".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Symbol".to_string(),
                alternate_name: None,
                charset: Some(2),
                code_page: None,
                family: FontFamilyHint::Tech,
                pitch: FontPitch::Default,
            },
        ];
        let mut style = CharacterStyle::default();
        style.font_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "\u{03b1}".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.font_family == PdfFontFamily::Symbol)
        ));
    }

    #[test]
    fn keeps_winansi_punctuation_in_normal_text_fonts() {
        let mut document = Document::default();
        document.fonts = vec![FontDef {
            index: 0,
            name: "Times New Roman".to_string(),
            alternate_name: None,
            charset: None,
            code_page: None,
            family: FontFamilyHint::Roman,
            pitch: FontPitch::Default,
        }];
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "\u{2026}".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "\u{2026}" && fragment.font_family == PdfFontFamily::Times)
        ));
        assert!(
            !layout.pages[0]
                .items
                .iter()
            .any(|item| matches!(item, LayoutItem::Text(fragment) if fragment.font_family == PdfFontFamily::Symbol))
        );
    }

    #[test]
    fn times_run_fragments_use_base14_widths_for_word_spacing() {
        let mut document = Document::default();
        document.fonts = vec![FontDef {
            index: 0,
            name: "Times New Roman".to_string(),
            alternate_name: None,
            charset: None,
            code_page: None,
            family: FontFamilyHint::Roman,
            pitch: FontPitch::Default,
        }];
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Here is a brief Times New Roman text.".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let times = text_x(&layout.pages[0], "Times ").expect("Times fragment");
        let new = text_x(&layout.pages[0], "New ").expect("New fragment");
        let roman = text_x(&layout.pages[0], "Roman ").expect("Roman fragment");

        assert!(
            (new - times - 33.0).abs() < 0.1,
            "Times fragment advanced by {}",
            new - times
        );
        assert!(
            (roman - new - 25.66).abs() < 0.1,
            "New fragment advanced by {}",
            roman - new
        );
    }

    #[test]
    fn resolves_symbol_bullet_runs_to_passive_sans_fallback() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Helvetica".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Symbol".to_string(),
                alternate_name: None,
                charset: Some(2),
                code_page: None,
                family: FontFamilyHint::Tech,
                pitch: FontPitch::Default,
            },
        ];
        let mut style = CharacterStyle::default();
        style.font_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "\u{2022}\t".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(|item| matches!(
            item,
            LayoutItem::Text(fragment)
                if fragment.text == "\u{2022}" && fragment.font_family == PdfFontFamily::Helvetica
        )));
        assert!(!layout.pages[0].items.iter().any(|item| matches!(
            item,
            LayoutItem::Text(fragment)
                if fragment.text == "\u{2022}" && fragment.font_family == PdfFontFamily::Symbol
        )));
    }

    #[test]
    fn resolves_wingdings_checkbox_runs_to_zapf_dingbats() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Helvetica".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Wingdings".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Tech,
                pitch: FontPitch::Default,
            },
        ];
        let mut style = CharacterStyle::default();
        style.font_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "\u{2611}".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.font_family == PdfFontFamily::ZapfDingbats)
        ));
    }

    #[test]
    fn resolves_unicode_checkbox_runs_to_zapf_dingbats() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Helvetica".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Segoe UI Symbol".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Nil,
                pitch: FontPitch::Default,
            },
        ];
        let mut style = CharacterStyle::default();
        style.font_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "\u{25a1} \u{2610} \u{2611} \u{2612} \u{2713} \u{2717}".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.font_family == PdfFontFamily::ZapfDingbats)
        ));
    }

    #[test]
    fn resolves_segoe_ui_symbol_latin_text_to_passive_sans_fallback() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Helvetica".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Segoe UI Symbol".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Nil,
                pitch: FontPitch::Default,
            },
        ];
        let mut style = CharacterStyle::default();
        style.font_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Label \u{2611}".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Label " && fragment.font_family == PdfFontFamily::Helvetica)
        ));
        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "\u{2611}" && fragment.font_family == PdfFontFamily::ZapfDingbats)
        ));
        assert!(
            !layout.pages[0]
                .items
                .iter()
                .any(|item| matches!(item, LayoutItem::Text(fragment) if fragment.font_family == PdfFontFamily::Symbol))
        );
    }

    #[test]
    fn splits_unicode_symbol_spans_to_passive_symbol_font() {
        let mut document = Document::default();
        document.fonts = vec![FontDef {
            index: 0,
            name: "Arial".to_string(),
            alternate_name: None,
            charset: None,
            code_page: None,
            family: FontFamilyHint::Swiss,
            pitch: FontPitch::Default,
        }];
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Alpha \u{03b1}+\u{03b2} <= \u{03a9} \u{2717}".to_string(),
                style: CharacterStyle::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let families = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment.font_family),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(families.contains(&PdfFontFamily::Helvetica));
        assert!(families.contains(&PdfFontFamily::Symbol));
        assert!(families.contains(&PdfFontFamily::ZapfDingbats));
    }

    #[test]
    fn resolves_safe_pdf_font_family_from_rtf_family_hints() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Helvetica".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Mystery Serif".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Roman,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 2,
                name: "Mystery Mono".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Modern,
                pitch: FontPitch::Default,
            },
        ];
        let mut roman_style = CharacterStyle::default();
        roman_style.font_index = 1;
        let mut modern_style = CharacterStyle::default();
        modern_style.font_index = 2;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    text: "Roman".to_string(),
                    style: roman_style,
                },
                Run {
                    text: "Modern".to_string(),
                    style: modern_style,
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Roman" && fragment.font_family == PdfFontFamily::Times)
        ));
        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Modern" && fragment.font_family == PdfFontFamily::Courier)
        ));
    }

    #[test]
    fn resolves_common_office_font_names_to_passive_base14_families() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Calibri".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Nil,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Cambria".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Nil,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 2,
                name: "Aptos Mono".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Nil,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 3,
                name: "MS Sans Serif".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Nil,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 4,
                name: "MS Serif".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Nil,
                pitch: FontPitch::Default,
            },
        ];
        let mut sans_style = CharacterStyle {
            font_index: 0,
            ..Default::default()
        };
        sans_style.font_index = 0;
        let serif_style = CharacterStyle {
            font_index: 1,
            ..Default::default()
        };
        let mono_style = CharacterStyle {
            font_index: 2,
            ..Default::default()
        };
        let legacy_sans_style = CharacterStyle {
            font_index: 3,
            ..Default::default()
        };
        let legacy_serif_style = CharacterStyle {
            font_index: 4,
            ..Default::default()
        };
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    text: "Sans".to_string(),
                    style: sans_style,
                },
                Run {
                    text: "Serif".to_string(),
                    style: serif_style,
                },
                Run {
                    text: "Mono".to_string(),
                    style: mono_style,
                },
                Run {
                    text: "LegacySans".to_string(),
                    style: legacy_sans_style,
                },
                Run {
                    text: "LegacySerif".to_string(),
                    style: legacy_serif_style,
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Sans" && fragment.font_family == PdfFontFamily::Helvetica)
        ));
        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Serif" && fragment.font_family == PdfFontFamily::Times)
        ));
        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Mono" && fragment.font_family == PdfFontFamily::Courier)
        ));
        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "LegacySans" && fragment.font_family == PdfFontFamily::Helvetica)
        ));
        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "LegacySerif" && fragment.font_family == PdfFontFamily::Times)
        ));
    }

    #[test]
    fn renders_narrow_font_aliases_with_passive_horizontal_scaling() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Arial".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Arial Narrow".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
        ];
        let normal_style = CharacterStyle {
            font_index: 0,
            ..Default::default()
        };
        let narrow_style = CharacterStyle {
            font_index: 1,
            ..Default::default()
        };
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    text: "Normal".to_string(),
                    style: normal_style,
                },
                Run {
                    text: "\n".to_string(),
                    style: CharacterStyle::default(),
                },
                Run {
                    text: "Normal".to_string(),
                    style: narrow_style,
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let fragments = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "Normal" => Some(fragment),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(fragments.len(), 2);
        assert_eq!(fragments[0].font_family, PdfFontFamily::Helvetica);
        assert_eq!(fragments[0].style.character_scaling_percent, 100);
        assert_eq!(fragments[1].font_family, PdfFontFamily::Helvetica);
        assert_eq!(
            fragments[1].style.character_scaling_percent,
            PASSIVE_NARROW_FONT_SCALE_PERCENT
        );
        assert!(
            fragments[1].x < fragments[0].x + 0.01,
            "narrow font scaling should not shift the left edge"
        );
    }

    #[test]
    fn resolves_safe_pdf_font_family_from_alternate_font_name() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Helvetica".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Mystery Sans".to_string(),
                alternate_name: Some("Courier New".to_string()),
                charset: None,
                code_page: None,
                family: FontFamilyHint::Nil,
                pitch: FontPitch::Default,
            },
        ];
        let mut style = CharacterStyle::default();
        style.font_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Fallback".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Fallback" && fragment.font_family == PdfFontFamily::Courier)
        ));
    }

    #[test]
    fn resolves_fixed_pitch_font_hint_to_courier_fallback() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Helvetica".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Mystery Fixed".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Nil,
                pitch: FontPitch::Fixed,
            },
            FontDef {
                index: 2,
                name: "Mystery Variable".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Nil,
                pitch: FontPitch::Variable,
            },
        ];
        let mut fixed_style = CharacterStyle {
            font_index: 1,
            ..Default::default()
        };
        fixed_style.font_index = 1;
        let variable_style = CharacterStyle {
            font_index: 2,
            ..Default::default()
        };
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    text: "Fixed".to_string(),
                    style: fixed_style,
                },
                Run {
                    text: "Variable".to_string(),
                    style: variable_style,
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Fixed" && fragment.font_family == PdfFontFamily::Courier)
        ));
        assert!(layout.pages[0].items.iter().any(
            |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Variable" && fragment.font_family == PdfFontFamily::Helvetica)
        ));
    }

    #[test]
    fn applies_exact_line_spacing_to_wrapped_lines() {
        let mut document = Document::default();
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.line_spacing_twips = Some(-480);
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "First\nSecond".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let baselines = text_baselines(&layout.pages[0]);

        assert_eq!(baselines.len(), 2);
        assert!((baselines[0] - baselines[1] - 24.0).abs() < 0.01);
    }

    #[test]
    fn applies_multiple_line_spacing_to_wrapped_lines() {
        let mut document = Document::default();
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.line_spacing_twips = Some(480);
        paragraph_style.line_spacing_multiple = true;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "First\nSecond".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let baselines = text_baselines(&layout.pages[0]);

        assert_eq!(baselines.len(), 2);
        assert!((baselines[0] - baselines[1] - 30.0).abs() < 0.01);
    }

    #[test]
    fn applies_section_line_grid_to_flowed_paragraph_lines() {
        let mut document = Document::default();
        document.page.text_line_grid_twips = Some(720);
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: ParagraphStyle::default(),
            runs: vec![Run {
                text: "First\nSecond".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let baselines = text_baselines(&layout.pages[0]);

        assert_eq!(baselines.len(), 2);
        assert!((baselines[0] - baselines[1] - 36.0).abs() < 0.01);
    }

    #[test]
    fn no_snap_line_grid_paragraphs_use_normal_line_spacing() {
        let mut document = Document::default();
        document.page.text_line_grid_twips = Some(720);
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.snap_to_line_grid = false;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "First\nSecond".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let baselines = text_baselines(&layout.pages[0]);

        assert_eq!(baselines.len(), 2);
        assert!((baselines[0] - baselines[1] - 15.0).abs() < 0.01);
    }

    #[test]
    fn advances_tabs_to_explicit_tab_stops() {
        let mut document = Document::default();
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.tab_stops_twips = vec![1440];
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "A\tB".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let second_x = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == "B" => Some(fragment.x),
                _ => None,
            })
            .expect("text after tab");

        assert!((second_x - 144.0).abs() < 0.01);
    }

    #[test]
    fn lays_out_caps_as_uppercase_display_text() {
        let mut document = Document::default();
        let mut style = CharacterStyle::default();
        style.all_caps = true;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Mixed case".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let text = layout_text(&layout.pages[0]);

        assert_eq!(text, "MIXED CASE");
    }

    #[test]
    fn lays_out_small_caps_lowercase_as_smaller_uppercase_fragments() {
        let mut document = Document::default();
        let mut style = CharacterStyle::default();
        style.small_caps = true;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Mix".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let text_fragments = layout.pages[0]
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(layout_text(&layout.pages[0]), "MIX");
        assert_eq!(text_fragments.len(), 2);
        assert_eq!(text_fragments[0].text, "M");
        assert_eq!(text_fragments[0].style.font_size_half_points, 24);
        assert_eq!(text_fragments[1].text, "IX");
        assert_eq!(text_fragments[1].style.font_size_half_points, 17);
        assert!(
            (text_fragments[0].baseline_y - text_fragments[1].baseline_y).abs() < 0.01,
            "small caps fragments should share a stable baseline"
        );
    }

    #[test]
    fn lays_out_section_breaks_as_passive_page_breaks() {
        let mut document = Document::default();
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Before".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::SectionBreak,
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "After".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert_eq!(layout_text(&layout.pages[0]), "Before");
        assert_eq!(layout_text(&layout.pages[1]), "After");
    }

    #[test]
    fn page_line_numbering_restarts_after_explicit_page_breaks() {
        let mut document = Document::default();
        document.page.line_numbering.enabled = true;
        document.page.line_numbering.restart = LineNumberRestart::Page;
        document.blocks = vec![
            paragraph_with_text("First page"),
            Block::PageBreak,
            paragraph_with_text("Second page"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert_eq!(layout_text(&layout.pages[0]), "1First page");
        assert_eq!(layout_text(&layout.pages[1]), "1Second page");
    }

    #[test]
    fn continuous_line_numbering_survives_later_section_settings() {
        let mut document = Document::default();
        document.page.line_numbering.enabled = true;
        document.page.line_numbering.restart = LineNumberRestart::Continuous;
        let mut second_section = document.page.clone();
        second_section.line_numbering.restart = LineNumberRestart::Continuous;
        document.blocks = vec![
            paragraph_with_text("First section"),
            Block::SectionBreak,
            Block::SectionSettings(second_section),
            paragraph_with_text("Second section"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert_eq!(layout_text(&layout.pages[0]), "1First section");
        assert_eq!(layout_text(&layout.pages[1]), "2Second section");
    }

    #[test]
    fn section_line_numbering_restarts_at_later_section_settings() {
        let mut document = Document::default();
        document.page.line_numbering.enabled = true;
        document.page.line_numbering.restart = LineNumberRestart::Continuous;
        let mut second_section = document.page.clone();
        second_section.line_numbering.restart = LineNumberRestart::Section;
        document.blocks = vec![
            paragraph_with_text("First section"),
            Block::SectionBreak,
            Block::SectionSettings(second_section),
            paragraph_with_text("Second section"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert_eq!(layout_text(&layout.pages[0]), "1First section");
        assert_eq!(layout_text(&layout.pages[1]), "1Second section");
    }

    #[test]
    fn lays_out_odd_and_even_section_breaks_with_blank_pages_when_needed() {
        let mut odd_document = Document::default();
        odd_document.blocks = vec![
            paragraph_with_text("Before"),
            Block::OddPageSectionBreak,
            paragraph_with_text("Odd start"),
        ];

        let odd_layout = LayoutEngine::layout(&odd_document);

        assert_eq!(odd_layout.pages.len(), 3);
        assert_eq!(layout_text(&odd_layout.pages[0]), "Before");
        assert!(odd_layout.pages[1].items.is_empty());
        assert_eq!(layout_text(&odd_layout.pages[2]), "Odd start");

        let mut even_document = Document::default();
        even_document.blocks = vec![
            paragraph_with_text("Before"),
            Block::EvenPageSectionBreak,
            paragraph_with_text("Even start"),
        ];

        let even_layout = LayoutEngine::layout(&even_document);

        assert_eq!(even_layout.pages.len(), 2);
        assert_eq!(layout_text(&even_layout.pages[1]), "Even start");
    }

    #[test]
    fn lays_out_section_page_settings_on_following_page() {
        let mut document = Document::default();
        let mut second_page = PageSettings::default();
        second_page.width_twips = 10_080;
        second_page.height_twips = 7_200;
        second_page.margin_left_twips = 720;
        second_page.margin_top_twips = 720;
        second_page.gutter_twips = 360;
        document.blocks = vec![
            paragraph_with_text("Before"),
            Block::SectionBreak,
            Block::SectionSettings(second_page),
            paragraph_with_text("After"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert_eq!(layout.pages[0].width, 612.0);
        assert_eq!(layout.pages[0].height, 792.0);
        assert_eq!(layout.pages[1].width, 504.0);
        assert_eq!(layout.pages[1].height, 360.0);
        assert!(
            layout.pages[1].items.iter().any(
                |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "After" && (fragment.x - 54.0).abs() < 0.01)
            )
        );
    }

    #[test]
    fn lays_out_explicit_column_breaks_on_same_page() {
        let mut document = Document::default();
        document.page.column_count = 2;
        document.blocks = vec![
            paragraph_with_text("Left"),
            Block::ColumnBreak,
            paragraph_with_text("Right"),
        ];

        let layout = LayoutEngine::layout(&document);
        let left_x = text_x(&layout.pages[0], "Left").expect("left text");
        let right_x = text_x(&layout.pages[0], "Right").expect("right text");

        assert_eq!(layout.pages.len(), 1);
        assert!(right_x > left_x + 200.0);
    }

    #[test]
    fn lays_out_explicit_section_column_widths_and_gaps() {
        let mut document = Document::default();
        document.page.column_count = 2;
        document.page.column_widths_twips = vec![1440, 2880];
        document.page.column_gaps_twips = vec![360];
        document.page.line_between_columns = true;
        document.blocks = vec![
            paragraph_with_text("Left"),
            Block::ColumnBreak,
            paragraph_with_text("Right"),
        ];

        let layout = LayoutEngine::layout(&document);
        let left_x = text_x(&layout.pages[0], "Left").expect("left text");
        let right_x = text_x(&layout.pages[0], "Right").expect("right text");

        assert!((left_x - 72.0).abs() < 0.01);
        assert!((right_x - 162.0).abs() < 0.01);
        assert!(has_vertical_line_at(&layout.pages[0], 153.0));
    }

    #[test]
    fn lays_out_rtl_paragraph_direction_as_right_alignment() {
        let mut right_style = ParagraphStyle::default();
        right_style.alignment = Alignment::Right;
        let mut document = Document::default();
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: right_style,
                runs: vec![Run {
                    text: "RTL".to_string(),
                    style: Default::default(),
                }],
            }),
            paragraph_with_text("LTR"),
        ];

        let layout = LayoutEngine::layout(&document);
        let right_x = text_x(&layout.pages[0], "RTL").expect("right text");
        let left_x = text_x(&layout.pages[0], "LTR").expect("left text");

        assert!(right_x > left_x + 200.0);
    }

    #[test]
    fn flows_body_text_to_next_column_before_next_page() {
        let mut document = small_test_page_document();
        document.page.column_count = 2;
        document.blocks = (0..11)
            .map(|idx| paragraph_with_text(&format!("P{idx}")))
            .collect();

        let layout = LayoutEngine::layout(&document);
        let first_x = text_x(&layout.pages[0], "P0").expect("first paragraph");
        let overflow_x = text_x(&layout.pages[0], "P10").expect("overflow paragraph");

        assert_eq!(layout.pages.len(), 1);
        assert!(overflow_x > first_x + 200.0);
    }

    #[test]
    fn lays_out_line_between_columns_as_passive_rule() {
        let mut document = Document::default();
        document.page.column_count = 2;
        document.page.line_between_columns = true;
        document.blocks = vec![
            paragraph_with_text("Left"),
            Block::ColumnBreak,
            paragraph_with_text("Right"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert!(has_vertical_line_at(&layout.pages[0], 306.0));
    }

    #[test]
    fn lays_out_page_borders_as_passive_lines() {
        let mut document = Document::default();
        document.page.margin_left_twips = 720;
        document.page.margin_top_twips = 720;
        document.page.margin_bottom_twips = 720;
        document.page.page_border_spacing_twips.top_twips = 240;
        document.page.page_border_spacing_twips.left_twips = 120;
        document.page.page_borders.top = TableCellBorder {
            visible: true,
            width_twips: 80,
            color_index: None,
            style: BorderStyle::Double,
            ..TableCellBorder::default()
        };
        document.page.page_borders.left = TableCellBorder {
            visible: true,
            width_twips: 40,
            color_index: None,
            style: BorderStyle::Dashed,
            ..TableCellBorder::default()
        };
        document.blocks = vec![paragraph_with_text("Body")];

        let layout = LayoutEngine::layout(&document);
        let top_y = layout.pages[0].geometry.height - layout.pages[0].geometry.margin_top + 12.0;
        let left_x = layout.pages[0].geometry.margin_left - 6.0;
        let top_border = layout.pages[0].items.iter().find_map(|item| match item {
            LayoutItem::Line { y1, y2, style, .. }
                if (*y1 - top_y).abs() < 0.01 && (*y2 - top_y).abs() < 0.01 =>
            {
                Some(style)
            }
            _ => None,
        });
        let left_border = layout.pages[0].items.iter().find_map(|item| match item {
            LayoutItem::Line { x1, x2, style, .. }
                if (*x1 - left_x).abs() < 0.01 && (*x2 - left_x).abs() < 0.01 =>
            {
                Some(style)
            }
            _ => None,
        });

        assert_eq!(top_border, Some(&LineStyle::Double));
        assert_eq!(left_border, Some(&LineStyle::Dashed));
    }

    #[test]
    fn page_borders_can_surround_header_and_footer_regions() {
        let mut document = Document::default();
        document.page.margin_top_twips = 1_440;
        document.page.margin_bottom_twips = 1_440;
        document.page.header_distance_twips = 360;
        document.page.footer_distance_twips = 360;
        document.page.page_border_includes_header = true;
        document.page.page_border_includes_footer = true;
        document.page.page_borders.top = TableCellBorder {
            visible: true,
            ..TableCellBorder::default()
        };
        document.page.page_borders.bottom = TableCellBorder {
            visible: true,
            ..TableCellBorder::default()
        };
        document.blocks = vec![paragraph_with_text("Body")];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let top_y = page.height - 18.0;
        let bottom_y = 18.0;

        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { y1, y2, .. }
                if (*y1 - top_y).abs() < 0.01 && (*y2 - top_y).abs() < 0.01
        )));
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { y1, y2, .. }
                if (*y1 - bottom_y).abs() < 0.01 && (*y2 - bottom_y).abs() < 0.01
        )));
    }

    #[test]
    fn page_borders_can_measure_spacing_from_page_edge() {
        let mut document = Document::default();
        document.page.width_twips = 5_760;
        document.page.height_twips = 5_760;
        document.page.margin_left_twips = 720;
        document.page.margin_right_twips = 720;
        document.page.margin_top_twips = 720;
        document.page.margin_bottom_twips = 720;
        document.page.page_border_from_page_edge = true;
        document.page.page_border_spacing_twips.left_twips = 120;
        document.page.page_border_spacing_twips.right_twips = 240;
        document.page.page_border_spacing_twips.top_twips = 360;
        document.page.page_border_spacing_twips.bottom_twips = 480;
        document.page.page_borders.top = TableCellBorder {
            visible: true,
            ..TableCellBorder::default()
        };
        document.page.page_borders.bottom = TableCellBorder {
            visible: true,
            ..TableCellBorder::default()
        };
        document.page.page_borders.left = TableCellBorder {
            visible: true,
            ..TableCellBorder::default()
        };
        document.page.page_borders.right = TableCellBorder {
            visible: true,
            ..TableCellBorder::default()
        };
        document.blocks = vec![paragraph_with_text("Body")];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(has_vertical_line_at(page, 6.0));
        assert!(has_vertical_line_at(page, 276.0));
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { y1, y2, .. }
                if (*y1 - 270.0).abs() < 0.01 && (*y2 - 270.0).abs() < 0.01
        )));
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { y1, y2, .. }
                if (*y1 - 24.0).abs() < 0.01 && (*y2 - 24.0).abs() < 0.01
        )));
    }

    #[test]
    fn lays_out_section_page_borders_on_later_pages() {
        let mut document = Document::default();
        let mut second_section = PageSettings::default();
        second_section.page_borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 60,
            color_index: None,
            style: BorderStyle::Dotted,
            ..TableCellBorder::default()
        };
        document.blocks = vec![
            paragraph_with_text("First section"),
            Block::SectionBreak,
            Block::SectionSettings(second_section),
            paragraph_with_text("Second section"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert!(!layout.pages[0].items.iter().any(|item| matches!(
            item,
            LayoutItem::Line {
                style: LineStyle::Dotted,
                ..
            }
        )));
        assert!(layout.pages[1].items.iter().any(|item| matches!(
            item,
            LayoutItem::Line {
                style: LineStyle::Dotted,
                ..
            }
        )));
    }

    #[test]
    fn page_break_before_starts_following_paragraph_on_new_page() {
        let mut document = Document::default();
        let mut style = ParagraphStyle::default();
        style.page_break_before = true;
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Before".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::Paragraph(Paragraph {
                style,
                runs: vec![Run {
                    text: "After".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert_eq!(layout_text(&layout.pages[0]), "Before");
        assert_eq!(layout_text(&layout.pages[1]), "After");
    }

    #[test]
    fn keep_together_starts_paragraph_on_next_page_when_it_would_split() {
        let mut document = small_test_page_document();
        let mut keep_style = ParagraphStyle::default();
        keep_style.keep_together = true;
        document.blocks = (0..8)
            .map(|_| paragraph_with_text("Filler"))
            .chain([Block::Paragraph(Paragraph {
                style: keep_style,
                runs: vec![Run {
                    text: "Keep\nTogether\nLines".to_string(),
                    style: Default::default(),
                }],
            })])
            .collect();

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert!(!layout_text(&layout.pages[0]).contains("Keep"));
        assert!(layout_text(&layout.pages[1]).contains("Keep"));
    }

    #[test]
    fn keep_with_next_starts_pair_on_next_page_when_pair_would_split() {
        let mut document = small_test_page_document();
        let mut keep_next_style = ParagraphStyle::default();
        keep_next_style.keep_with_next = true;
        document.blocks = (0..9)
            .map(|_| paragraph_with_text("Filler"))
            .chain([
                Block::Paragraph(Paragraph {
                    style: keep_next_style,
                    runs: vec![Run {
                        text: "Keep next".to_string(),
                        style: Default::default(),
                    }],
                }),
                paragraph_with_text("Follower"),
            ])
            .collect();

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert!(!layout_text(&layout.pages[0]).contains("Keep next"));
        assert!(layout_text(&layout.pages[1]).contains("Keep next"));
        assert!(layout_text(&layout.pages[1]).contains("Follower"));
    }

    #[test]
    fn keep_with_next_preflight_uses_section_line_grid_spacing() {
        let mut document = small_test_page_document();
        document.page.text_line_grid_twips = Some(720);
        let mut keep_next_style = ParagraphStyle::default();
        keep_next_style.keep_with_next = true;
        document.blocks = (0..2)
            .map(|_| paragraph_with_text("Filler"))
            .chain([
                Block::Paragraph(Paragraph {
                    style: keep_next_style,
                    runs: vec![Run {
                        text: "Keep next".to_string(),
                        style: Default::default(),
                    }],
                }),
                paragraph_with_text("Follower"),
            ])
            .collect();

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert_eq!(layout_text(&layout.pages[0]), "FillerFiller");
        assert_eq!(layout_text(&layout.pages[1]), "Keep nextFollower");
    }

    #[test]
    fn widow_control_moves_last_line_with_previous_line() {
        let mut document = small_test_page_document();
        let mut widow_style = ParagraphStyle::default();
        widow_style.widow_control = true;
        document.blocks = (0..6)
            .map(|_| paragraph_with_text("Filler"))
            .chain([Block::Paragraph(Paragraph {
                style: widow_style,
                runs: vec![Run {
                    text: "Alpha\nBeta\nGamma".to_string(),
                    style: Default::default(),
                }],
            })])
            .collect();

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert!(layout_text(&layout.pages[0]).contains("Alpha"));
        assert!(!layout_text(&layout.pages[0]).contains("Beta"));
        assert!(layout_text(&layout.pages[1]).contains("Beta"));
        assert!(layout_text(&layout.pages[1]).contains("Gamma"));
    }

    #[test]
    fn repeats_header_and_footer_on_each_page() {
        let mut document = Document::default();
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Header".to_string(),
                style: Default::default(),
            }],
        }];
        document.footer = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Footer".to_string(),
                style: Default::default(),
            }],
        }];
        document.blocks.clear();
        for _ in 0..120 {
            document.blocks.push(Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Body paragraph.".to_string(),
                    style: Default::default(),
                }],
            }));
        }

        let layout = LayoutEngine::layout(&document);
        assert!(layout.pages.len() > 1);
        for page in &layout.pages {
            assert!(page.items.iter().any(
                |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Header")
            ));
            assert!(page.items.iter().any(
                |item| matches!(item, LayoutItem::Text(fragment) if fragment.text == "Footer")
            ));
        }
    }

    #[test]
    fn repeats_header_images_on_each_page() {
        let mut document = Document::default();
        document.header_images = vec![StaticImage {
            format: ImageFormat::Jpeg,
            bytes: vec![0xff, 0xd8, 0xff, 0xd9],
            palette: Vec::new(),
            vector_commands: Vec::new(),
            width_px: 8,
            height_px: 4,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(360),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        }];
        document.blocks.clear();
        for _ in 0..120 {
            document.blocks.push(Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Body paragraph.".to_string(),
                    style: Default::default(),
                }],
            }));
        }

        let layout = LayoutEngine::layout(&document);
        assert!(layout.pages.len() > 1);
        for page in &layout.pages {
            assert!(
                page.items
                    .iter()
                    .any(|item| matches!(item, LayoutItem::Image(_))),
                "expected repeated header image on every page"
            );
        }
    }

    #[test]
    fn repeats_header_shapes_on_each_page() {
        let mut document = Document::default();
        document.header_shapes = vec![StaticShape {
            kind: StaticShapeKind::Rectangle,
            left_twips: 0,
            top_twips: 0,
            width_twips: 720,
            height_twips: 360,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            stroke_width_twips: 30,
            stroke_color: Color {
                red: 255,
                green: 0,
                blue: 0,
            },
            stroke_style: BorderStyle::Single,
            fill_color: Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            }),
            text: Vec::new(),
            points: Vec::new(),
        }];
        document.blocks.clear();
        for _ in 0..120 {
            document.blocks.push(Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Body paragraph.".to_string(),
                    style: Default::default(),
                }],
            }));
        }

        let layout = LayoutEngine::layout(&document);
        assert!(layout.pages.len() > 1);
        for page in &layout.pages {
            assert!(
                page.items
                    .iter()
                    .any(|item| matches!(item, LayoutItem::Highlight { .. })),
                "expected repeated header shape fill on every page"
            );
            assert!(
                page.items
                    .iter()
                    .any(|item| matches!(item, LayoutItem::Line { .. })),
                "expected repeated header shape outline on every page"
            );
        }
    }

    #[test]
    fn lays_out_header_and_footer_at_configured_distances() {
        let mut document = Document::default();
        document.page.header_distance_twips = 360;
        document.page.footer_distance_twips = 1_080;
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Header".to_string(),
                style: Default::default(),
            }],
        }];
        document.footer = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Footer".to_string(),
                style: Default::default(),
            }],
        }];
        document.blocks = vec![paragraph_with_text("Body")];

        let layout = LayoutEngine::layout(&document);
        let header_y = text_baseline_for(&layout.pages[0], "Header");
        let footer_y = text_baseline_for(&layout.pages[0], "Footer");

        assert!(
            (header_y - (layout.pages[0].height - 29.25)).abs() < 0.01,
            "header_y={header_y}, height={}",
            layout.pages[0].height
        );
        assert!((footer_y - 42.75).abs() < 0.01, "footer_y={footer_y}");
    }

    #[test]
    fn lays_out_header_paragraph_shading_and_borders() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 220,
                green: 230,
                blue: 240,
            },
        ];
        let mut header_style = ParagraphStyle::default();
        header_style.shading_color_index = Some(1);
        header_style.borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 40,
            style: BorderStyle::Single,
            ..TableCellBorder::default()
        };
        document.header = vec![Paragraph {
            style: header_style,
            runs: vec![Run {
                text: "Header".to_string(),
                style: Default::default(),
            }],
        }];
        document.blocks = vec![paragraph_with_text("Body")];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];

        assert!(
            page.items
                .iter()
                .any(|item| matches!(item, LayoutItem::Highlight { .. }))
        );
        assert!(page.items.iter().any(
            |item| matches!(item, LayoutItem::Line { x1, x2, .. } if (*x2 - *x1).abs() > 100.0)
        ));
    }

    #[test]
    fn lays_out_section_header_footer_distances_on_later_pages() {
        let mut document = Document::default();
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Header".to_string(),
                style: Default::default(),
            }],
        }];
        document.footer = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Footer".to_string(),
                style: Default::default(),
            }],
        }];
        let mut second_section = PageSettings::default();
        second_section.header_distance_twips = 360;
        second_section.footer_distance_twips = 1_080;
        document.blocks = vec![
            paragraph_with_text("First section"),
            Block::SectionBreak,
            Block::SectionSettings(second_section),
            paragraph_with_text("Second section"),
        ];

        let layout = LayoutEngine::layout(&document);
        let first_header_y = text_baseline_for(&layout.pages[0], "Header");
        let second_header_y = text_baseline_for(&layout.pages[1], "Header");
        let second_footer_y = text_baseline_for(&layout.pages[1], "Footer");

        assert!(
            (first_header_y - (layout.pages[0].height - 47.25)).abs() < 0.01,
            "first_header_y={first_header_y}, height={}",
            layout.pages[0].height
        );
        assert!(
            (second_header_y - (layout.pages[1].height - 29.25)).abs() < 0.01,
            "second_header_y={second_header_y}, height={}",
            layout.pages[1].height
        );
        assert!(
            (second_footer_y - 42.75).abs() < 0.01,
            "second_footer_y={second_footer_y}"
        );
    }

    #[test]
    fn selects_section_specific_header_footer_on_later_pages() {
        let mut document = Document::default();
        document.header = vec![repeating_paragraph("Document header")];
        document.footer = vec![repeating_paragraph("Document footer")];
        let mut second_section = PageSettings::default();
        second_section.header = vec![repeating_paragraph("Section header")];
        second_section.footer = vec![repeating_paragraph("Section footer")];
        document.blocks = vec![
            paragraph_with_text("First section"),
            Block::SectionBreak,
            Block::SectionSettings(second_section),
            paragraph_with_text("Second section"),
            Block::PageBreak,
            paragraph_with_text("Second section page two"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 3);
        assert!(layout_text(&layout.pages[0]).contains("Document header"));
        assert!(layout_text(&layout.pages[0]).contains("Document footer"));
        assert!(!layout_text(&layout.pages[0]).contains("Section header"));
        assert!(layout_text(&layout.pages[1]).contains("Section header"));
        assert!(layout_text(&layout.pages[1]).contains("Section footer"));
        assert!(!layout_text(&layout.pages[1]).contains("Document header"));
        assert!(layout_text(&layout.pages[2]).contains("Section header"));
        assert!(layout_text(&layout.pages[2]).contains("Section footer"));
    }

    #[test]
    fn selects_first_even_and_default_header_footer_variants() {
        fn repeating_paragraph(text: &str) -> Paragraph {
            Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: text.to_string(),
                    style: Default::default(),
                }],
            }
        }

        let mut document = Document::default();
        document.page.title_page = true;
        document.header = vec![repeating_paragraph("Odd header")];
        document.first_page_header = vec![repeating_paragraph("First header")];
        document.even_page_header = vec![repeating_paragraph("Even header")];
        document.footer = vec![repeating_paragraph("Odd footer")];
        document.first_page_footer = vec![repeating_paragraph("First footer")];
        document.even_page_footer = vec![repeating_paragraph("Even footer")];
        document.blocks = vec![
            paragraph_with_text("Page one"),
            Block::PageBreak,
            paragraph_with_text("Page two"),
            Block::PageBreak,
            paragraph_with_text("Page three"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 3);
        assert!(layout_text(&layout.pages[0]).contains("First header"));
        assert!(layout_text(&layout.pages[0]).contains("First footer"));
        assert!(!layout_text(&layout.pages[0]).contains("Odd header"));
        assert!(layout_text(&layout.pages[1]).contains("Even header"));
        assert!(layout_text(&layout.pages[1]).contains("Even footer"));
        assert!(layout_text(&layout.pages[2]).contains("Odd header"));
        assert!(layout_text(&layout.pages[2]).contains("Odd footer"));
    }

    #[test]
    fn selects_first_header_footer_on_later_section_title_page() {
        fn repeating_paragraph(text: &str) -> Paragraph {
            Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: text.to_string(),
                    style: Default::default(),
                }],
            }
        }

        let mut document = Document::default();
        document.header = vec![repeating_paragraph("Odd header")];
        document.first_page_header = vec![repeating_paragraph("First header")];
        document.footer = vec![repeating_paragraph("Odd footer")];
        document.first_page_footer = vec![repeating_paragraph("First footer")];
        let mut second_section = PageSettings::default();
        second_section.title_page = true;
        document.blocks = vec![
            paragraph_with_text("First section"),
            Block::SectionBreak,
            Block::SectionSettings(second_section),
            paragraph_with_text("Second section"),
            Block::PageBreak,
            paragraph_with_text("Second section page two"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 3);
        assert!(layout_text(&layout.pages[0]).contains("Odd header"));
        assert!(layout_text(&layout.pages[0]).contains("Odd footer"));
        assert!(!layout_text(&layout.pages[0]).contains("First header"));
        assert!(layout_text(&layout.pages[1]).contains("First header"));
        assert!(layout_text(&layout.pages[1]).contains("First footer"));
        assert!(!layout_text(&layout.pages[1]).contains("Odd header"));
        assert!(layout_text(&layout.pages[2]).contains("Odd header"));
        assert!(layout_text(&layout.pages[2]).contains("Odd footer"));
    }

    #[test]
    fn resolves_repeating_header_page_numbers_per_page() {
        let mut document = Document::default();
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: PAGE_NUMBER_MARKER.to_string(),
                style: Default::default(),
            }],
        }];
        document.blocks.clear();
        for _ in 0..120 {
            document.blocks.push(Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Body paragraph.".to_string(),
                    style: Default::default(),
                }],
            }));
        }

        let layout = LayoutEngine::layout(&document);

        assert!(layout.pages.len() > 1);
        assert!(layout_text(&layout.pages[0]).contains('1'));
        assert!(layout_text(&layout.pages[1]).contains('2'));
    }

    #[test]
    fn resolves_page_number_markers_from_configured_start() {
        let mut document = Document::default();
        document.page.page_number_start = Some(7);
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: format!("Page {PAGE_NUMBER_MARKER}"),
                style: Default::default(),
            }],
        }];
        document.blocks = vec![
            paragraph_with_text("Page one"),
            Block::PageBreak,
            paragraph_with_text("Page two"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert!(layout_text(&layout.pages[0]).contains("Page 7"));
        assert!(layout_text(&layout.pages[1]).contains("Page 8"));
        assert!(!layout_text(&layout.pages[0]).contains("Page 1"));
        assert!(!layout_text(&layout.pages[1]).contains("Page 2"));
    }

    #[test]
    fn resolves_page_number_markers_from_configured_format() {
        let mut document = Document::default();
        document.page.page_number_start = Some(4);
        document.page.page_number_format = Some(PageNumberFormat::UpperRoman);
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: format!("Page {PAGE_NUMBER_MARKER}"),
                style: Default::default(),
            }],
        }];
        document.blocks = vec![
            paragraph_with_text("Page one"),
            Block::PageBreak,
            paragraph_with_text("Page two"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 2);
        assert!(layout_text(&layout.pages[0]).contains("Page IV"));
        assert!(layout_text(&layout.pages[1]).contains("Page V"));
        assert!(!layout_text(&layout.pages[0]).contains("Page 4"));
        assert!(!layout_text(&layout.pages[1]).contains("Page 5"));
    }

    #[test]
    fn positions_header_page_number_from_safe_page_number_coordinates() {
        let mut document = Document::default();
        document.page.page_number_x_twips = Some(360);
        document.page.page_number_y_twips = Some(1_440);
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: format!("Page {PAGE_NUMBER_MARKER}"),
                style: Default::default(),
            }],
        }];
        document.blocks = vec![paragraph_with_text("Body")];

        let layout = LayoutEngine::layout(&document);
        let fragment = text_fragment_for(&layout.pages[0], "Page ");

        assert!((fragment.x - 18.0).abs() < 0.01);
        assert!((fragment.baseline_y - 708.75).abs() < 0.01);
    }

    #[test]
    fn positions_header_total_page_count_from_safe_page_number_coordinates() {
        let mut document = Document::default();
        document.page.page_number_x_twips = Some(360);
        document.page.page_number_y_twips = Some(1_440);
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: format!("Pages {TOTAL_PAGES_MARKER}"),
                style: Default::default(),
            }],
        }];
        document.blocks = vec![
            paragraph_with_text("Page one"),
            Block::PageBreak,
            paragraph_with_text("Page two"),
        ];

        let layout = LayoutEngine::layout(&document);
        let fragment = text_fragment_for(&layout.pages[0], "Pages ");

        assert!((fragment.x - 18.0).abs() < 0.01);
        assert!((fragment.baseline_y - 708.75).abs() < 0.01);
        assert!(layout_text(&layout.pages[0]).contains("Pages 2"));
    }

    #[test]
    fn positions_header_section_page_count_from_safe_page_number_coordinates() {
        let mut document = Document::default();
        document.page.page_number_x_twips = Some(360);
        document.page.page_number_y_twips = Some(1_440);
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: format!("Section pages {SECTION_PAGES_MARKER}"),
                style: Default::default(),
            }],
        }];
        document.blocks = vec![
            paragraph_with_text("First body page one"),
            Block::PageBreak,
            paragraph_with_text("First body page two"),
            Block::SectionBreak,
            paragraph_with_text("Second section"),
        ];

        let layout = LayoutEngine::layout(&document);
        let fragment = text_fragment_for(&layout.pages[0], "Section ");

        assert!((fragment.x - 18.0).abs() < 0.01);
        assert!((fragment.baseline_y - 708.75).abs() < 0.01);
        assert!(layout_text(&layout.pages[0]).contains("Section pages 2"));
    }

    #[test]
    fn resolves_page_number_markers_from_section_start() {
        let mut document = Document::default();
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: format!("Page {PAGE_NUMBER_MARKER}"),
                style: Default::default(),
            }],
        }];
        let mut second_section = PageSettings::default();
        second_section.page_number_start = Some(3);
        document.blocks = vec![
            paragraph_with_text("First section"),
            Block::SectionBreak,
            Block::SectionSettings(second_section),
            paragraph_with_text("Second section"),
            Block::PageBreak,
            paragraph_with_text("Second section page two"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 3);
        assert!(layout_text(&layout.pages[0]).contains("Page 1"));
        assert!(layout_text(&layout.pages[1]).contains("Page 3"));
        assert!(layout_text(&layout.pages[2]).contains("Page 4"));
        assert!(!layout_text(&layout.pages[1]).contains("Page 2"));
    }

    #[test]
    fn resolves_page_number_markers_from_continued_section_numbering() {
        let mut document = Document::default();
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: format!("Page {PAGE_NUMBER_MARKER}"),
                style: Default::default(),
            }],
        }];
        document.blocks = vec![
            paragraph_with_text("First section"),
            Block::PageBreak,
            paragraph_with_text("First section page two"),
            Block::SectionBreak,
            Block::SectionSettings(PageSettings::default()),
            paragraph_with_text("Second section continues"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 3);
        assert!(layout_text(&layout.pages[0]).contains("Page 1"));
        assert!(layout_text(&layout.pages[1]).contains("Page 2"));
        assert!(layout_text(&layout.pages[2]).contains("Page 3"));
        assert!(!layout_text(&layout.pages[2]).contains("Page 1"));
    }

    #[test]
    fn resolves_page_number_markers_from_section_format() {
        let mut document = Document::default();
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: format!("Page {PAGE_NUMBER_MARKER}"),
                style: Default::default(),
            }],
        }];
        let mut second_section = PageSettings::default();
        second_section.page_number_start = Some(2);
        second_section.page_number_format = Some(PageNumberFormat::LowerLetter);
        document.blocks = vec![
            paragraph_with_text("First section"),
            Block::SectionBreak,
            Block::SectionSettings(second_section),
            paragraph_with_text("Second section"),
            Block::PageBreak,
            paragraph_with_text("Second section page two"),
        ];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 3);
        assert!(layout_text(&layout.pages[0]).contains("Page 1"));
        assert!(layout_text(&layout.pages[1]).contains("Page b"));
        assert!(layout_text(&layout.pages[2]).contains("Page c"));
        assert!(!layout_text(&layout.pages[1]).contains("Page 2"));
        assert!(!layout_text(&layout.pages[2]).contains("Page 3"));
    }

    #[test]
    fn late_total_page_marker_updates_passive_underline_width() {
        let mut document = small_test_page_document();
        let mut style = CharacterStyle::default();
        style.underline = UnderlineStyle::Single;
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: format!("Pages {TOTAL_PAGES_MARKER}"),
                style: style.clone(),
            }],
        }];
        document.blocks.clear();
        for idx in 0..80 {
            document
                .blocks
                .push(paragraph_with_text(&format!("Body {idx}")));
        }

        let layout = LayoutEngine::layout(&document);
        let page_text = layout_text(&layout.pages[0]);
        assert!(
            layout.pages.len() >= 10,
            "fixture should cross into two-digit page count, got {} page(s)",
            layout.pages.len()
        );
        let expected = format!("Pages {}", layout.pages.len());
        assert!(
            page_text.contains(&expected),
            "header text was {page_text:?}"
        );
        let resolved_marker = layout.pages.len().to_string();
        let expected_width =
            measure_text_with_family(&resolved_marker, &style, PdfFontFamily::Helvetica);
        let old_width = measure_text_with_family(
            LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER,
            &style,
            PdfFontFamily::Helvetica,
        );

        let underline_width =
            underline_width_for_fragment_after(&layout.pages[0], "Pages ", &resolved_marker);
        assert!(
            (underline_width - expected_width).abs() < 0.01,
            "late NUMPAGES underline width should match resolved text width"
        );
        assert!(
            (underline_width - old_width).abs() > 1.0,
            "late NUMPAGES underline should not keep marker placeholder width"
        );
    }

    #[test]
    fn late_total_page_marker_updates_word_underline_width() {
        let mut document = small_test_page_document();
        let mut style = CharacterStyle::default();
        style.underline = UnderlineStyle::Words;
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: format!("Pages {TOTAL_PAGES_MARKER}"),
                style: style.clone(),
            }],
        }];
        document.blocks.clear();
        for idx in 0..80 {
            document
                .blocks
                .push(paragraph_with_text(&format!("Body {idx}")));
        }

        let layout = LayoutEngine::layout(&document);
        assert!(
            layout.pages.len() >= 10,
            "fixture should cross into two-digit page count, got {} page(s)",
            layout.pages.len()
        );
        let resolved_marker = layout.pages.len().to_string();
        let expected_width =
            measure_text_with_family(&resolved_marker, &style, PdfFontFamily::Helvetica);
        let old_width = measure_text_with_family(
            LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER,
            &style,
            PdfFontFamily::Helvetica,
        );

        let underline_width =
            underline_width_for_fragment_after(&layout.pages[0], "Pages ", &resolved_marker);
        assert!(
            (underline_width - expected_width).abs() < 0.01,
            "late NUMPAGES word underline width should match resolved text width"
        );
        assert!(
            (underline_width - old_width).abs() > 1.0,
            "late NUMPAGES word underline should not keep marker placeholder width"
        );
    }

    #[test]
    fn late_total_page_marker_updates_character_border_width() {
        let mut document = small_test_page_document();
        let mut style = CharacterStyle::default();
        style.border.visible = true;
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: format!("Pages {TOTAL_PAGES_MARKER}"),
                style: style.clone(),
            }],
        }];
        document.blocks.clear();
        for idx in 0..80 {
            document
                .blocks
                .push(paragraph_with_text(&format!("Body {idx}")));
        }

        let layout = LayoutEngine::layout(&document);
        assert!(
            layout.pages.len() >= 10,
            "fixture should cross into two-digit page count, got {} page(s)",
            layout.pages.len()
        );
        let resolved_marker = layout.pages.len().to_string();
        let fragment = text_fragment_after(&layout.pages[0], "Pages ", &resolved_marker);
        let expected_width =
            measure_text_with_family(&resolved_marker, &style, PdfFontFamily::Helvetica);
        let old_width = measure_text_with_family(
            LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER,
            &style,
            PdfFontFamily::Helvetica,
        );
        let pad = character_border_test_pad(&style);
        let right_edge = character_border_right_edge_for_fragment(
            &layout.pages[0],
            fragment,
            fragment.x + expected_width + pad,
        );

        assert!(
            (right_edge - (fragment.x + expected_width + pad)).abs() < 0.01,
            "late NUMPAGES character border should match resolved text width"
        );
        assert!(
            (right_edge - (fragment.x + old_width + pad)).abs() > 1.0,
            "late NUMPAGES character border should not keep marker placeholder width"
        );
    }

    #[test]
    fn late_total_page_marker_uses_bounded_width_for_wrapping() {
        let document = Document::default();
        let style = CharacterStyle::default();
        let paragraph = Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: format!("A {TOTAL_PAGES_MARKER} tail"),
                style: style.clone(),
            }],
        };
        let raw_marker_width =
            measure_text_with_family(TOTAL_PAGES_MARKER, &style, PdfFontFamily::Helvetica);
        let placeholder_width = measure_text_with_family(
            LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER,
            &style,
            PdfFontFamily::Helvetica,
        );
        let prefix_width = measure_text_with_family("A ", &style, PdfFontFamily::Helvetica);
        let line_width = prefix_width + raw_marker_width + 0.5;
        assert!(
            line_width < prefix_width + placeholder_width,
            "fixture should distinguish raw marker width from placeholder width"
        );

        let lines = wrap_paragraph(&paragraph, line_width, &test_markers("1", "1"), &document);

        assert_eq!(line_text(&lines[0]), "A ");
        assert!(
            line_text(&lines[1]).starts_with(TOTAL_PAGES_MARKER),
            "late NUMPAGES marker should wrap using bounded placeholder width"
        );
    }

    #[test]
    fn late_total_page_marker_shifts_following_same_line_text() {
        let mut document = small_test_page_document();
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    text: format!("Pages {TOTAL_PAGES_MARKER}"),
                    style: Default::default(),
                },
                Run {
                    text: " total".to_string(),
                    style: Default::default(),
                },
            ],
        }];
        document.blocks.clear();
        for idx in 0..80 {
            document
                .blocks
                .push(paragraph_with_text(&format!("Body {idx}")));
        }

        let layout = LayoutEngine::layout(&document);
        assert!(
            layout.pages.len() >= 10,
            "fixture should cross into two-digit page count, got {} page(s)",
            layout.pages.len()
        );
        let resolved_marker = layout.pages.len().to_string();
        let marker = text_fragment_after(&layout.pages[0], "Pages ", &resolved_marker);
        let suffix = text_fragment_after(&layout.pages[0], &resolved_marker, " ");
        let expected_width = measure_text_with_family(
            &resolved_marker,
            &CharacterStyle::default(),
            PdfFontFamily::Helvetica,
        );
        let old_width = measure_text_with_family(
            LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER,
            &CharacterStyle::default(),
            PdfFontFamily::Helvetica,
        );

        assert!(
            (suffix.x - (marker.x + expected_width)).abs() < 0.01,
            "late NUMPAGES following text should start after resolved marker text"
        );
        assert!(
            (suffix.x - (marker.x + old_width)).abs() > 1.0,
            "late NUMPAGES following text should not keep marker placeholder position"
        );
    }

    #[test]
    fn late_section_page_marker_updates_passive_underline_width() {
        let mut document = small_test_page_document();
        let mut style = CharacterStyle::default();
        style.underline = UnderlineStyle::Single;
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: format!("Section pages {SECTION_PAGES_MARKER}"),
                style: style.clone(),
            }],
        }];
        document.blocks.clear();
        for idx in 0..80 {
            document
                .blocks
                .push(paragraph_with_text(&format!("First section body {idx}")));
        }
        document.blocks.push(Block::SectionBreak);
        document
            .blocks
            .push(Block::SectionSettings(PageSettings::default()));
        document.blocks.push(paragraph_with_text("Second section"));

        let layout = LayoutEngine::layout(&document);
        let first_section_pages = layout
            .pages
            .iter()
            .filter(|page| page.section_number == 1)
            .count();
        assert!(
            first_section_pages >= 10,
            "fixture should cross into two-digit section page count, got {first_section_pages}"
        );
        let resolved_marker = first_section_pages.to_string();
        let expected = format!("Section pages {resolved_marker}");
        let page_text = layout_text(&layout.pages[0]);
        assert!(
            page_text.contains(&expected),
            "header text was {page_text:?}"
        );
        let expected_width =
            measure_text_with_family(&resolved_marker, &style, PdfFontFamily::Helvetica);
        let old_width = measure_text_with_family(
            LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER,
            &style,
            PdfFontFamily::Helvetica,
        );

        let underline_width =
            underline_width_for_fragment_after(&layout.pages[0], "pages ", &resolved_marker);
        assert!(
            (underline_width - expected_width).abs() < 0.01,
            "late SECTIONPAGES underline width should match resolved text width"
        );
        assert!(
            (underline_width - old_width).abs() > 1.0,
            "late SECTIONPAGES underline should not keep marker placeholder width"
        );
    }

    #[test]
    fn late_section_page_marker_updates_word_underline_width() {
        let mut document = small_test_page_document();
        let mut style = CharacterStyle::default();
        style.underline = UnderlineStyle::Words;
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: format!("Section pages {SECTION_PAGES_MARKER}"),
                style: style.clone(),
            }],
        }];
        document.blocks.clear();
        for idx in 0..80 {
            document
                .blocks
                .push(paragraph_with_text(&format!("First section body {idx}")));
        }
        document.blocks.push(Block::SectionBreak);
        document
            .blocks
            .push(Block::SectionSettings(PageSettings::default()));
        document.blocks.push(paragraph_with_text("Second section"));

        let layout = LayoutEngine::layout(&document);
        let first_section_pages = layout
            .pages
            .iter()
            .filter(|page| page.section_number == 1)
            .count();
        assert!(
            first_section_pages >= 10,
            "fixture should cross into two-digit section page count, got {first_section_pages}"
        );
        let resolved_marker = first_section_pages.to_string();
        let expected_width =
            measure_text_with_family(&resolved_marker, &style, PdfFontFamily::Helvetica);
        let old_width = measure_text_with_family(
            LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER,
            &style,
            PdfFontFamily::Helvetica,
        );

        let underline_width =
            underline_width_for_fragment_after(&layout.pages[0], "pages ", &resolved_marker);
        assert!(
            (underline_width - expected_width).abs() < 0.01,
            "late SECTIONPAGES word underline width should match resolved text width"
        );
        assert!(
            (underline_width - old_width).abs() > 1.0,
            "late SECTIONPAGES word underline should not keep marker placeholder width"
        );
    }

    #[test]
    fn late_section_page_marker_updates_character_border_width() {
        let mut document = small_test_page_document();
        let mut style = CharacterStyle::default();
        style.border.visible = true;
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: format!("Section pages {SECTION_PAGES_MARKER}"),
                style: style.clone(),
            }],
        }];
        document.blocks.clear();
        for idx in 0..80 {
            document
                .blocks
                .push(paragraph_with_text(&format!("First section body {idx}")));
        }
        document.blocks.push(Block::SectionBreak);
        document
            .blocks
            .push(Block::SectionSettings(PageSettings::default()));
        document.blocks.push(paragraph_with_text("Second section"));

        let layout = LayoutEngine::layout(&document);
        let first_section_pages = layout
            .pages
            .iter()
            .filter(|page| page.section_number == 1)
            .count();
        assert!(
            first_section_pages >= 10,
            "fixture should cross into two-digit section page count, got {first_section_pages}"
        );
        let resolved_marker = first_section_pages.to_string();
        let fragment = text_fragment_after(&layout.pages[0], "pages ", &resolved_marker);
        let expected_width =
            measure_text_with_family(&resolved_marker, &style, PdfFontFamily::Helvetica);
        let old_width = measure_text_with_family(
            LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER,
            &style,
            PdfFontFamily::Helvetica,
        );
        let pad = character_border_test_pad(&style);
        let right_edge = character_border_right_edge_for_fragment(
            &layout.pages[0],
            fragment,
            fragment.x + expected_width + pad,
        );

        assert!(
            (right_edge - (fragment.x + expected_width + pad)).abs() < 0.01,
            "late SECTIONPAGES character border should match resolved text width"
        );
        assert!(
            (right_edge - (fragment.x + old_width + pad)).abs() > 1.0,
            "late SECTIONPAGES character border should not keep marker placeholder width"
        );
    }

    #[test]
    fn late_section_page_marker_uses_bounded_width_for_wrapping() {
        let document = Document::default();
        let style = CharacterStyle::default();
        let paragraph = Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: format!("A {SECTION_PAGES_MARKER} tail"),
                style: style.clone(),
            }],
        };
        let raw_marker_width =
            measure_text_with_family(SECTION_PAGES_MARKER, &style, PdfFontFamily::Helvetica);
        let placeholder_width = measure_text_with_family(
            LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER,
            &style,
            PdfFontFamily::Helvetica,
        );
        let prefix_width = measure_text_with_family("A ", &style, PdfFontFamily::Helvetica);
        let line_width = prefix_width + raw_marker_width + 0.5;
        assert!(
            line_width < prefix_width + placeholder_width,
            "fixture should distinguish raw marker width from placeholder width"
        );

        let lines = wrap_paragraph(&paragraph, line_width, &test_markers("1", "1"), &document);

        assert_eq!(line_text(&lines[0]), "A ");
        assert!(
            line_text(&lines[1]).starts_with(SECTION_PAGES_MARKER),
            "late SECTIONPAGES marker should wrap using bounded placeholder width"
        );
    }

    #[test]
    fn late_section_page_marker_shifts_following_same_line_text() {
        let mut document = small_test_page_document();
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    text: format!("Section pages {SECTION_PAGES_MARKER}"),
                    style: Default::default(),
                },
                Run {
                    text: " total".to_string(),
                    style: Default::default(),
                },
            ],
        }];
        document.blocks.clear();
        for idx in 0..80 {
            document
                .blocks
                .push(paragraph_with_text(&format!("First section body {idx}")));
        }
        document.blocks.push(Block::SectionBreak);
        document
            .blocks
            .push(Block::SectionSettings(PageSettings::default()));
        document.blocks.push(paragraph_with_text("Second section"));

        let layout = LayoutEngine::layout(&document);
        let first_section_pages = layout
            .pages
            .iter()
            .filter(|page| page.section_number == 1)
            .count();
        assert!(
            first_section_pages >= 10,
            "fixture should cross into two-digit section page count, got {first_section_pages}"
        );
        let resolved_marker = first_section_pages.to_string();
        let marker = text_fragment_after(&layout.pages[0], "pages ", &resolved_marker);
        let suffix = text_fragment_after(&layout.pages[0], &resolved_marker, " ");
        let expected_width = measure_text_with_family(
            &resolved_marker,
            &CharacterStyle::default(),
            PdfFontFamily::Helvetica,
        );
        let old_width = measure_text_with_family(
            LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER,
            &CharacterStyle::default(),
            PdfFontFamily::Helvetica,
        );

        assert!(
            (suffix.x - (marker.x + expected_width)).abs() < 0.01,
            "late SECTIONPAGES following text should start after resolved marker text"
        );
        assert!(
            (suffix.x - (marker.x + old_width)).abs() > 1.0,
            "late SECTIONPAGES following text should not keep marker placeholder position"
        );
    }

    fn text_baselines(page: &LayoutPage) -> Vec<f32> {
        page.items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment.baseline_y),
                _ => None,
            })
            .collect()
    }

    fn layout_text(page: &LayoutPage) -> String {
        page.items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Text(fragment) => Some(fragment.text.as_str()),
                _ => None,
            })
            .collect()
    }

    fn text_baseline_for(page: &LayoutPage, text: &str) -> f32 {
        page.items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == text => Some(fragment.baseline_y),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing text fragment {text}"))
    }

    fn text_fragment_for<'a>(page: &'a LayoutPage, text: &str) -> &'a TextFragment {
        page.items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text == text => Some(fragment),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing text fragment {text}"))
    }

    fn first_text_fragment_except<'a>(
        page: &'a LayoutPage,
        excluded_text: &[&str],
    ) -> &'a TextFragment {
        page.items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment)
                    if !excluded_text
                        .iter()
                        .any(|excluded| fragment.text == *excluded) =>
                {
                    Some(fragment)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing text fragment outside {excluded_text:?}"))
    }

    fn underline_width_for_fragment_after(page: &LayoutPage, prefix: &str, text: &str) -> f32 {
        let fragment = text_fragment_after(page, prefix, text);
        page.items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Underline { x, width, .. } if (*x - fragment.x).abs() < 0.01 => {
                    Some(*width)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing underline for {text}"))
    }

    fn text_fragment_after<'a>(page: &'a LayoutPage, prefix: &str, text: &str) -> &'a TextFragment {
        let prefix = page
            .items
            .iter()
            .enumerate()
            .find_map(|(idx, item)| match item {
                LayoutItem::Text(fragment) if fragment.text == prefix => Some((idx, fragment)),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing prefix fragment {prefix}"));
        let fragment = page
            .items
            .iter()
            .skip(prefix.0.saturating_add(1))
            .find_map(|item| match item {
                LayoutItem::Text(fragment)
                    if fragment.text == text
                        && (fragment.baseline_y - prefix.1.baseline_y).abs() < 0.01 =>
                {
                    Some(fragment)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing text fragment {text} after {prefix:?}"));
        fragment
    }

    fn character_border_test_pad(style: &CharacterStyle) -> f32 {
        let stroke_width = twips_to_points(style.border.width_twips.max(1)).max(0.25);
        1.5 + (stroke_width * 0.5) + twips_to_points(style.border.spacing_twips.max(0))
    }

    fn character_border_right_edge_for_fragment(
        page: &LayoutPage,
        fragment: &TextFragment,
        expected_x: f32,
    ) -> f32 {
        page.items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Line { x1, y1, x2, y2, .. }
                    if same_pdf_coord(*x1, *x2)
                        && *x1 > fragment.x
                        && fragment.baseline_y >= y1.min(*y2) - 0.01
                        && fragment.baseline_y <= y1.max(*y2) + 0.01 =>
                {
                    Some(*x1)
                }
                _ => None,
            })
            .min_by(|left, right| {
                (left - expected_x)
                    .abs()
                    .total_cmp(&(right - expected_x).abs())
            })
            .unwrap_or_else(|| {
                panic!(
                    "missing character border right edge for {:?}",
                    fragment.text
                )
            })
    }

    fn line_text(line: &Line) -> String {
        line.runs.iter().map(|run| run.text.as_str()).collect()
    }

    fn text_x(page: &LayoutPage, text: &str) -> Option<f32> {
        page.items.iter().find_map(|item| match item {
            LayoutItem::Text(fragment) if fragment.text == text => Some(fragment.x),
            _ => None,
        })
    }

    fn small_test_page_document() -> Document {
        let mut document = Document::default();
        document.page.height_twips = 4_000;
        document.page.margin_top_twips = 720;
        document.page.margin_bottom_twips = 720;
        document
    }

    fn paragraph_with_text(text: &str) -> Block {
        Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: text.to_string(),
                style: Default::default(),
            }],
        })
    }

    fn repeating_paragraph(text: &str) -> Paragraph {
        Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: text.to_string(),
                style: Default::default(),
            }],
        }
    }

    fn has_vertical_line_at(page: &LayoutPage, x: f32) -> bool {
        page.items.iter().any(|item| {
            matches!(
                item,
                LayoutItem::Line { x1, x2, .. }
                    if (*x1 - x).abs() < 0.01 && (*x2 - x).abs() < 0.01
            )
        })
    }

    fn internal_horizontal_line_y(page: &LayoutPage, x1: f32, x2: f32) -> Option<f32> {
        let mut ys = page
            .items
            .iter()
            .filter_map(|item| match item {
                LayoutItem::Line {
                    x1: line_x1,
                    y1,
                    x2: line_x2,
                    y2,
                    ..
                } if (*line_x1 - x1).abs() < 0.01
                    && (*line_x2 - x2).abs() < 0.01
                    && (*y1 - *y2).abs() < 0.01 =>
                {
                    Some(*y1)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        ys.sort_by(f32::total_cmp);
        ys.dedup_by(|a, b| (*a - *b).abs() < 0.01);
        if ys.len() < 3 {
            return None;
        }
        Some(ys[1])
    }

    fn has_horizontal_line_segment_at_y(page: &LayoutPage, x1: f32, x2: f32, y: f32) -> bool {
        page.items.iter().any(|item| {
            matches!(
                item,
                LayoutItem::Line {
                    x1: line_x1,
                    y1,
                    x2: line_x2,
                    y2,
                    ..
                } if (*line_x1 - x1).abs() < 0.01
                    && (*line_x2 - x2).abs() < 0.01
                    && (*y1 - y).abs() < 0.01
                    && (*y2 - y).abs() < 0.01
            )
        })
    }
}
