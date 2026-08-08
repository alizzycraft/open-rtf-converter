use std::collections::{HashMap, HashSet};

use unicode_linebreak::{BreakOpportunity, linebreaks};

use crate::fonts::{FontAssetStyle, FontProvider};
use crate::model::StaticImageWrapSide;
use crate::model::{
    Alignment, BOOKMARK_PAGE_ANCHOR_MARKER, BOOKMARK_PAGE_MARKER_END, BOOKMARK_PAGE_REF_MARKER,
    Block, BorderStyle, CharacterStyle, DOCUMENT_CHARS_MARKER, DOCUMENT_CHARS_WITH_SPACES_MARKER,
    DOCUMENT_WORDS_MARKER, Document, ENDNOTE_REFERENCE_MARKER, ENDNOTE_REFERENCE_MARKER_END,
    EndnotePlacement, FOOTNOTE_REFERENCE_MARKER, FOOTNOTE_REFERENCE_MARKER_END, FontDef,
    FontFamilyHint, FontPitch, FootnotePlacement, ImageFormat, LineNumberRestart,
    NESTED_TABLE_ANCHOR_MARKER, NoteNumberRestart, NoteNumbering, PAGE_NUMBER_MARKER,
    PASSIVE_ADVANCE_MARKER, PASSIVE_MATH_FRACTION_RULE_MARKER, PASSIVE_MATH_INLINE_LEAD_MARKER,
    PASSIVE_MATH_STACK_ANCHOR_MARKER, PageNumberFormat, PageSettings, PageVerticalAlignment,
    Paragraph, ParagraphBorders, ParagraphFrameVerticalAlignment, ParagraphStyle,
    PassiveMathFractionPart, PassiveMathLimitPart, Run, SECTION_NUMBER_MARKER,
    SECTION_PAGES_MARKER, ShadingPattern, StaticImage, StaticImageVectorCommand,
    StaticImageVectorFillRule, StaticShape, StaticShapeArrowhead, StaticShapeHorizontalAnchor,
    StaticShapeKind, StaticShapeLineCap, StaticShapeLineJoin, StaticShapeTextVerticalAnchor,
    StaticShapeVerticalAnchor, TABLE_ROW_DYNAMIC_VERTICAL_BOTTOM_OFFSET_BASE,
    TABLE_ROW_DYNAMIC_VERTICAL_CENTER_OFFSET_BASE, TABLE_ROW_DYNAMIC_VERTICAL_OFFSET_SPAN_TWIPS,
    TOTAL_PAGES_MARKER, TabAlignment, TabLeader, Table, TableCell, TableCellBorder,
    TableCellHorizontalMerge, TableCellTextDirection, TableCellVerticalAlign,
    TableCellVerticalMerge, TableRow, TableRowAlignment, TableRowWrapMargins, UnderlineStyle,
    inline_image_marker_index,
};

const TWIPS_PER_POINT: f32 = 20.0;
const SMALL_CAPS_FONT_SCALE: f32 = 0.7;
const MAX_SYNTHETIC_DROP_CAP_FONT_SIZE_HALF_POINTS: i32 = 400;
const AUTO_PARAGRAPH_SPACING_TWIPS: i32 = 240;
const DEFAULT_PARAGRAPH_BORDER_SIDE_GAP_POINTS: f32 = 1.5;
const MAX_LAYOUT_COLUMNS: usize = 16;
const PASSIVE_NARROW_FONT_SCALE_PERCENT: i32 = 82;
const OFFICE_MATH_EQUATION_ARRAY_HALF_ROW_PITCH_HALF_POINTS: i32 = 14;
const WORD_INLINE_OFFICE_MATH_LEAD_IN_POINTS: f32 = 1.92;
const WORD_NARY_OPERATOR_GAP_EM: f32 = 1.0 / 6.0;
const PASSIVE_NOTE_LABEL_SHIFT_HALF_POINTS: i32 = 6;
const PASSIVE_NOTE_LABEL_FONT_SCALE_PERCENT: i32 = 65;
const WORD_COMPACT_BOTTOM_NOTE_RESERVATION_LEAD_POINTS: f32 = 1.2;
const LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER: &str = "888";
const MAX_VERTICAL_TABLE_TEXT_WRAP_POINTS: f32 = 720.0;
const WORD_INLINE_IMAGE_BODY_EDGE_TOLERANCE_POINTS: f32 = 1.0;
const PASSIVE_MATH_FRACTION_RULE_WIDTH_POINTS: f32 = 0.84;
const PASSIVE_MATH_FRACTION_RULE_BASELINE_OFFSET_POINTS: f32 = 3.42;
const PASSIVE_MATH_FRACTION_LINE_HEIGHT_MULTIPLIER: f32 = 1.47;
const WORD_ARIAL_BASELINE_INSET_EM: f32 = 19.0 / 240.0;
const WORD_CALIBRI_BASELINE_INSET_EM: f32 = 13.0 / 360.0;
const WORD_TIMES_BASELINE_INSET_EM: f32 = 5.0 / 72.0;
const WORD_CAMBRIA_MATH_NARY_BASELINE_RISE_EM: f32 = 0.04;
const MAX_ABSOLUTE_FRAME_LAYOUT_REFINEMENTS: usize = 4;
const WORD_PARAGRAPH_FRAME_PAGE_EDGE_INSET_POINTS: f32 = 18.0;
const WORD_PARAGRAPH_FRAME_DEVICE_PIXEL_POINTS: f32 = 72.0 / 600.0;

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
    flow_exclusions: Vec<FlowExclusion>,
    display_page_number: String,
    section_number: usize,
    geometry: PageGeometry,
}

#[derive(Debug, Copy, Clone)]
struct FlowExclusion {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    wrap_side: StaticImageWrapSide,
    wrap_bottom_inset: f32,
    kind: FlowExclusionKind,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum FlowExclusionKind {
    Generic,
    ParagraphFrame,
}

#[derive(Debug, Copy, Clone)]
struct PendingAbsoluteFramePreview {
    page_index: usize,
    exclusion: FlowExclusion,
}

#[derive(Debug, Copy, Clone)]
struct PlannedAbsoluteFrameExclusion {
    block_index: usize,
    exclusion: FlowExclusion,
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
    CappedLine {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        width: f32,
        color: PdfColor,
        style: LineStyle,
        cap: LineCap,
    },
    JoinedPolyline {
        points: Vec<LayoutPoint>,
        width: f32,
        color: PdfColor,
        style: LineStyle,
        cap: LineCap,
        join: LineJoin,
    },
    Opacity {
        fill_percent: u8,
        stroke_percent: u8,
        item: Box<LayoutItem>,
    },
    ClipRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        item: Box<LayoutItem>,
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
        fill_gradient_color: Option<PdfColor>,
        fill_pattern: ShadingPattern,
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
        fill_gradient_color: Option<PdfColor>,
        fill_pattern: ShadingPattern,
    },
    Polygon {
        points: Vec<LayoutPoint>,
        paths: Vec<Vec<LayoutPoint>>,
        overlay_paths: Vec<Vec<LayoutPoint>>,
        fill_rule: StaticImageVectorFillRule,
        stroke_width: f32,
        stroke_color: PdfColor,
        stroke_style: LineStyle,
        fill_color: Option<PdfColor>,
        fill_gradient_color: Option<PdfColor>,
        fill_pattern: ShadingPattern,
    },
    Image(ImageFragment),
    Drawing(DrawingFragment),
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
    Triple,
    Dotted,
    Dashed,
    FineDashed,
    Wavy,
    Emboss,
    Engrave,
    ThinThick,
    ThickThin,
    ThinThickThin,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum LineCap {
    #[default]
    Flat,
    Round,
    Square,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Clone)]
pub struct TextFragment {
    pub text: String,
    pub x: f32,
    pub baseline_y: f32,
    pub rotation: TextRotation,
    pub color: PdfColor,
    pub font_family: PdfFontFamily,
    pub word_spacing: f32,
    pub style: CharacterStyle,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum TextRotation {
    #[default]
    None,
    Clockwise90,
    CounterClockwise90,
}

#[derive(Debug, Clone)]
pub struct ImageFragment {
    pub image: StaticImage,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub z_order: i32,
    pub below_text: bool,
}

#[derive(Debug, Clone)]
pub struct DrawingFragment {
    pub z_order: i32,
    pub below_text: bool,
    pub item: Box<LayoutItem>,
}

impl LayoutDocument {
    pub(crate) fn for_each_text_fragment(&self, callback: &mut impl FnMut(&TextFragment)) {
        for page in &self.pages {
            page.for_each_text_fragment(callback);
        }
    }

    pub(crate) fn for_each_image_fragment(&self, callback: &mut impl FnMut(&ImageFragment)) {
        for page in &self.pages {
            page.for_each_image_fragment(callback);
        }
    }
}

impl LayoutPage {
    pub(crate) fn for_each_text_fragment(&self, callback: &mut impl FnMut(&TextFragment)) {
        for item in &self.items {
            item.for_each_text_fragment(callback);
        }
    }

    pub(crate) fn for_each_image_fragment(&self, callback: &mut impl FnMut(&ImageFragment)) {
        for item in &self.items {
            item.for_each_image_fragment(callback);
        }
    }
}

impl LayoutItem {
    fn for_each_text_fragment(&self, callback: &mut impl FnMut(&TextFragment)) {
        let mut item = self;
        loop {
            match item {
                Self::Text(fragment) => {
                    callback(fragment);
                    return;
                }
                Self::Drawing(fragment) => item = &fragment.item,
                Self::Opacity { item: nested, .. } | Self::ClipRect { item: nested, .. } => {
                    item = nested;
                }
                Self::Highlight { .. }
                | Self::Underline { .. }
                | Self::Line { .. }
                | Self::CappedLine { .. }
                | Self::JoinedPolyline { .. }
                | Self::Ellipse { .. }
                | Self::RoundedRectangle { .. }
                | Self::Polygon { .. }
                | Self::Image(_) => return,
            }
        }
    }

    fn for_each_image_fragment(&self, callback: &mut impl FnMut(&ImageFragment)) {
        let mut item = self;
        loop {
            match item {
                Self::Image(fragment) => {
                    callback(fragment);
                    return;
                }
                Self::Drawing(fragment) => item = &fragment.item,
                Self::Opacity { item: nested, .. } | Self::ClipRect { item: nested, .. } => {
                    item = nested;
                }
                Self::Text(_)
                | Self::Highlight { .. }
                | Self::Underline { .. }
                | Self::Line { .. }
                | Self::CappedLine { .. }
                | Self::JoinedPolyline { .. }
                | Self::Ellipse { .. }
                | Self::RoundedRectangle { .. }
                | Self::Polygon { .. } => return,
            }
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PdfColor {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
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
    facing_pages: bool,
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
    physical_page_number: usize,
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
        let authored_gutter = twips_to_points(settings.gutter_twips.max(0));
        let gutter = if settings.gutter_at_top {
            0.0
        } else {
            authored_gutter
        };
        let margin_top = twips_to_points(settings.margin_top_twips)
            + if settings.gutter_at_top {
                authored_gutter
            } else {
                0.0
            };
        let margin_bottom = twips_to_points(settings.margin_bottom_twips);
        let header_distance = twips_to_points(settings.header_distance_twips.max(0));
        let footer_distance = twips_to_points(settings.footer_distance_twips.max(0));
        let facing_pages = settings.facing_pages;
        let mirror_margins = settings.mirror_margins;
        let gutter_on_right = settings.gutter_on_right;
        let (margin_left, margin_right) = page_horizontal_margins(
            base_margin_left,
            base_margin_right,
            gutter,
            facing_pages,
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
            facing_pages,
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
            physical_page_number,
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
            self.facing_pages,
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
            physical_page_number,
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
    facing_pages: bool,
    mirror_margins: bool,
    gutter_on_right: bool,
    physical_page_number: usize,
) -> (f32, f32) {
    let is_even_page = physical_page_number % 2 == 0;
    let (mut margin_left, mut margin_right) = if facing_pages && mirror_margins && is_even_page {
        (base_margin_right, base_margin_left)
    } else {
        (base_margin_left, base_margin_right)
    };
    let gutter_is_right = if facing_pages {
        is_even_page != gutter_on_right
    } else {
        gutter_on_right
    };
    if gutter_is_right {
        margin_right += gutter;
    } else {
        margin_left += gutter;
    }
    (margin_left, margin_right)
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
            start: if settings.restart_page_numbering {
                settings.page_number_start.unwrap_or(1).max(1) as usize
            } else {
                1
            },
            base_physical_page: base_physical_page.max(1),
            format: settings
                .page_number_format
                .unwrap_or(PageNumberFormat::Decimal),
        }
    }

    fn with_section_settings(self, settings: &PageSettings, base_physical_page: usize) -> Self {
        let current_number = self.display_page_number_value(base_physical_page);
        Self {
            start: if settings.restart_page_numbering {
                settings.page_number_start.unwrap_or(1).max(1) as usize
            } else {
                current_number
            },
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
        flow_exclusions: Vec::new(),
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
    natural_height: f32,
    height: f32,
    segmented_flow: bool,
    inline_image_baseline_context: bool,
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
    cell_text_directions: Vec<TableCellTextDirection>,
    top_border_inset: f32,
    row_height: f32,
}

#[derive(Debug, Clone)]
struct PreparedCellLine {
    line: Line,
    style: ParagraphStyle,
    paragraph_index: usize,
    text_direction: TableCellTextDirection,
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
        let empty_targets = HashMap::new();
        let mut observed_targets = HashMap::new();
        let first_pass = Self::layout_pass(
            document,
            font_provider,
            &empty_targets,
            &mut observed_targets,
        );
        if observed_targets.is_empty() {
            return first_pass;
        }

        let mut targets = observed_targets;
        let mut output = first_pass;
        for _ in 0..MAX_ABSOLUTE_FRAME_LAYOUT_REFINEMENTS {
            let mut next_targets = HashMap::new();
            output = Self::layout_pass(document, font_provider, &targets, &mut next_targets);
            if next_targets == targets {
                return output;
            }
            targets = next_targets;
        }
        output
    }

    fn layout_pass(
        document: &Document,
        font_provider: Option<&FontProvider>,
        absolute_frame_target_pages: &HashMap<usize, usize>,
        observed_absolute_frame_pages: &mut HashMap<usize, usize>,
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
        let mut rendered_footnote_pages = vec![None; document.footnotes.len()];
        let mut rendered_endnote_pages = vec![None; document.endnotes.len()];
        let mut last_paragraph_anchor = None;
        let mut body_clipped_past_page_end = false;
        let mut pending_floating_table_preview = false;
        let mut pending_absolute_frame_previews = HashMap::new();
        let mut planned_absolute_frame_exclusions: HashMap<
            usize,
            Vec<PlannedAbsoluteFrameExclusion>,
        > = HashMap::new();
        let mut absolute_frame_segment_scanned = false;
        let mut consumed_absolute_frame_blocks = HashSet::new();

        for (block_idx, block) in document.blocks.iter().enumerate() {
            if consumed_absolute_frame_blocks.contains(&block_idx) {
                continue;
            }
            if body_clipped_past_page_end {
                if matches!(
                    block,
                    Block::PageBreak
                        | Block::SectionBreak
                        | Block::EvenPageSectionBreak
                        | Block::OddPageSectionBreak
                        | Block::ColumnBreak
                ) {
                    body_clipped_past_page_end = false;
                } else {
                    continue;
                }
            }
            match block {
                Block::PageBreak => {
                    layout_footnotes_until_block(
                        &mut pages,
                        &mut cursor_y,
                        document,
                        &mut rendered_footnote_pages,
                        block_idx,
                        geometry.content_width,
                        geometry.margin_left,
                        geometry.margin_bottom,
                        &mut geometry,
                        document_stats,
                        font_provider,
                    );
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
                    absolute_frame_segment_scanned = false;
                }
                Block::ContinuousSectionBreak => {
                    layout_notes_for_section_boundary(
                        &mut pages,
                        &mut cursor_y,
                        document,
                        &mut rendered_footnote_pages,
                        &mut rendered_endnote_pages,
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
                    absolute_frame_segment_scanned = false;
                }
                Block::SectionBreak => {
                    layout_notes_for_section_boundary(
                        &mut pages,
                        &mut cursor_y,
                        document,
                        &mut rendered_footnote_pages,
                        &mut rendered_endnote_pages,
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
                    absolute_frame_segment_scanned = false;
                }
                Block::EvenPageSectionBreak => {
                    layout_notes_for_section_boundary(
                        &mut pages,
                        &mut cursor_y,
                        document,
                        &mut rendered_footnote_pages,
                        &mut rendered_endnote_pages,
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
                    let target_parity = if geometry.facing_pages {
                        PageParity::Even
                    } else {
                        PageParity::Odd
                    };
                    start_new_page_with_parity(
                        &mut pages,
                        &mut cursor_y,
                        &mut geometry,
                        &mut current_column,
                        target_parity,
                    );
                    absolute_frame_segment_scanned = false;
                }
                Block::OddPageSectionBreak => {
                    layout_notes_for_section_boundary(
                        &mut pages,
                        &mut cursor_y,
                        document,
                        &mut rendered_footnote_pages,
                        &mut rendered_endnote_pages,
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
                    absolute_frame_segment_scanned = false;
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
                    absolute_frame_segment_scanned = false;
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
                    if pending_floating_table_preview {
                        if let Some(page) = pages.last_mut() {
                            page.flow_exclusions.pop();
                        }
                        pending_floating_table_preview = false;
                    }
                    body_clipped_past_page_end = layout_table(
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
                    if document.inline_image_block_indices.contains(&block_idx) {
                        continue;
                    }
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
                    let paragraph_anchor_y = last_paragraph_anchor
                        .filter(|(page_count, column, _)| {
                            *page_count == pages.len()
                                && *column == current_column
                                && previous_non_shape_block_is_paragraph(
                                    &document.blocks,
                                    block_idx,
                                )
                        })
                        .map(|(_, _, y)| y);
                    layout_shape(
                        &mut pages,
                        &mut cursor_y,
                        paragraph_anchor_y,
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
                    let framed_paragraphs =
                        absolute_paragraph_frame_group(&document.blocks, block_idx);
                    if is_complete_absolute_paragraph_frame(paragraph)
                        && !framed_paragraphs.is_empty()
                    {
                        observed_absolute_frame_pages.insert(block_idx, pages.len() - 1);
                    }
                    let pending_absolute_frame_preview = take_pending_absolute_frame_preview(
                        &pages,
                        &mut pending_absolute_frame_previews,
                        block_idx,
                    );
                    let planned_absolute_frame_preview = pages
                        .len()
                        .checked_sub(1)
                        .and_then(|page_index| {
                            planned_absolute_frame_exclusions
                                .get(&page_index)
                                .and_then(|planned| {
                                    planned
                                        .iter()
                                        .find(|planned| planned.block_index == block_idx)
                                })
                                .zip(pages.last())
                        })
                        .and_then(|(planned, page)| {
                            page.flow_exclusions
                                .iter()
                                .any(|exclusion| same_flow_exclusion(*exclusion, planned.exclusion))
                                .then_some(planned.exclusion)
                        });
                    let absolute_frame_preview =
                        pending_absolute_frame_preview.or(planned_absolute_frame_preview);
                    if !framed_paragraphs.is_empty()
                        && layout_framed_paragraph_group(
                            &mut pages,
                            &mut cursor_y,
                            &framed_paragraphs,
                            geometry.body_width(current_column),
                            &mut geometry,
                            &mut current_column,
                            document,
                            document_stats,
                            font_provider,
                            absolute_frame_preview,
                        )
                    {
                        consumed_absolute_frame_blocks
                            .extend((block_idx + 1)..(block_idx + framed_paragraphs.len()));
                        last_paragraph_anchor = Some((pages.len(), current_column, cursor_y));
                        continue;
                    }
                    if !absolute_frame_segment_scanned {
                        preview_future_absolute_frame_exclusions(
                            &mut pages,
                            &mut pending_absolute_frame_previews,
                            &mut planned_absolute_frame_exclusions,
                            absolute_frame_target_pages,
                            &document.blocks,
                            block_idx,
                            cursor_y,
                            geometry.body_width(current_column),
                            geometry,
                            current_column,
                            document,
                            document_stats,
                            font_provider,
                        );
                        absolute_frame_segment_scanned = true;
                    }
                    if let Some(Block::Table(table)) = document.blocks.get(block_idx + 1) {
                        pending_floating_table_preview =
                            preview_immediately_following_floating_table_exclusion(
                                &mut pages,
                                table,
                                geometry.body_width(current_column),
                                geometry.body_left(current_column),
                                geometry,
                                document,
                                document_stats,
                                font_provider,
                            );
                    }
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
                    let kept_following_paragraphs =
                        keep_with_next_chain(&document.blocks, block_idx);
                    if !kept_following_paragraphs.is_empty() {
                        let previous_page_count = pages.len();
                        start_new_page_for_kept_paragraphs(
                            &mut pages,
                            &mut cursor_y,
                            paragraph,
                            &kept_following_paragraphs,
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
                    let mut planned_page_advance_hook =
                        |pages: &mut Vec<LayoutPage>, _: &mut f32, _: &PageGeometry| {
                            let Some(page_index) = pages.len().checked_sub(1) else {
                                return;
                            };
                            let Some(exclusions) =
                                planned_absolute_frame_exclusions.get(&page_index)
                            else {
                                return;
                            };
                            if let Some(page) = pages.last_mut() {
                                page.flow_exclusions
                                    .extend(exclusions.iter().map(|planned| planned.exclusion));
                            }
                        };
                    layout_paragraph_with_auto_footnotes(
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
                        Some(AutoFootnoteFlush {
                            rendered_footnote_pages: &mut rendered_footnote_pages,
                            block_index: block_idx,
                            rendered_footnote_index_on_page: None,
                            reserved_footnote_margin_bottom_on_page: None,
                        }),
                        Some(&mut planned_page_advance_hook),
                        Some(&planned_absolute_frame_exclusions),
                    );
                    last_paragraph_anchor = Some((pages.len(), current_column, cursor_y));
                    let reaches_note_boundary = document
                        .blocks
                        .get(block_idx.saturating_add(1))
                        .is_none_or(section_breaks_end_current_section);
                    if !reaches_note_boundary {
                        layout_footnotes_until_block(
                            &mut pages,
                            &mut cursor_y,
                            document,
                            &mut rendered_footnote_pages,
                            block_idx,
                            geometry.content_width,
                            geometry.margin_left,
                            geometry.margin_bottom,
                            &mut geometry,
                            document_stats,
                            font_provider,
                        );
                    }
                }
            }
        }

        layout_remaining_section_endnotes(
            &mut pages,
            &mut cursor_y,
            document,
            &mut rendered_endnote_pages,
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
            &mut rendered_endnote_pages,
            EndnotePlacement::AfterBody,
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
            &mut rendered_endnote_pages,
            EndnotePlacement::EndOfDocument,
            geometry.content_width,
            geometry.margin_left,
            geometry.margin_bottom,
            &mut geometry,
            document_stats,
            font_provider,
        );
        layout_remaining_footnotes(
            &mut pages,
            &mut cursor_y,
            document,
            &mut rendered_footnote_pages,
            geometry.content_width,
            geometry.margin_left,
            geometry.margin_bottom,
            &mut geometry,
            document_stats,
            font_provider,
        );

        apply_page_vertical_alignment(&mut pages);
        let body_item_counts = pages
            .iter()
            .map(|page| page.items.len())
            .collect::<Vec<_>>();
        layout_column_separators(&mut pages);
        layout_page_borders(&mut pages, document, &header_footer_sets);
        layout_repeating_header_footer(
            &mut pages,
            document,
            &header_footer_sets,
            true,
            document_stats,
            font_provider,
            Some(&body_item_counts),
        );
        layout_repeating_header_footer(
            &mut pages,
            document,
            &header_footer_sets,
            false,
            document_stats,
            font_provider,
            None,
        );
        layout_legacy_auto_page_numbers(&mut pages, document, font_provider);
        layout_background_shapes(
            &mut pages,
            &header_footer_sets,
            document,
            document_stats,
            font_provider,
        );
        resolve_footnote_reference_markers(&mut pages, document, font_provider);
        resolve_endnote_reference_markers(
            &mut pages,
            document,
            &rendered_endnote_pages,
            font_provider,
        );
        resolve_bookmark_page_ref_markers(&mut pages, document, font_provider);
        resolve_section_page_markers(&mut pages);
        resolve_total_page_markers(&mut pages);
        apply_passive_drawing_z_order(&mut pages);

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
fn layout_remaining_footnotes(
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

#[allow(clippy::too_many_arguments)]
fn layout_footnotes_for_section(
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
fn layout_footnotes_until_block(
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
fn layout_footnotes_until_index(
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

fn footnote_entries_until_block(
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

fn footnote_entries_until_index(
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
fn layout_footnote_entries(
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
fn layout_footnote_paragraph(
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
fn push_note_separator(
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

fn push_default_note_separator(
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
fn position_cursor_for_bottom_note_entries(
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

fn footnote_entries_block_height(
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

fn footnote_reservation_margin_bottom(
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

fn footnote_numbering_for(document: &Document, index: usize) -> NoteNumbering {
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

fn endnote_numbering_for(document: &Document, index: usize) -> NoteNumbering {
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

fn footnote_sequence_for_index(document: &Document, index: usize) -> usize {
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

fn footnote_sequence_for_rendered_page(
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

fn endnote_sequence_for_rendered_page(
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

fn explicit_page_group_for_footnote(document: &Document, index: usize) -> usize {
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

fn note_sequence_for_index(
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

fn note_display_paragraph(
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
fn layout_endnote_entries(
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
fn layout_endnote_paragraph(
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
fn layout_endnotes_for_placement(
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
fn layout_notes_for_section_boundary(
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
fn layout_endnotes_for_section(
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
fn layout_remaining_section_endnotes(
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

    let placement = image.placement;
    let x_origin = placement
        .map(|placement| {
            shape_horizontal_origin(placement.horizontal_anchor, margin_left, geometry)
        })
        .unwrap_or(margin_left);
    let y_origin = placement
        .map(|placement| shape_vertical_origin(placement.vertical_anchor, *cursor_y, geometry))
        .unwrap_or(*cursor_y);
    let x = x_origin + left_offset;
    let y = y_origin - top_offset - height;
    let Some(page) = pages.last_mut() else {
        return;
    };
    page.items.push(LayoutItem::Image(ImageFragment {
        image: image.clone(),
        x,
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
    if image.placement.is_some_and(|placement| placement.text_wrap) {
        return;
    }
    *cursor_y = y;
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
    let (mut width, mut height) = match (image.display_width_twips, image.display_height_twips) {
        (Some(width_twips), Some(height_twips)) => {
            (twips_to_points(width_twips), twips_to_points(height_twips))
        }
        (Some(width_twips), None) => {
            let width = twips_to_points(width_twips);
            let height = width * natural_height / natural_width;
            (width, height)
        }
        (None, Some(height_twips)) => {
            let height = twips_to_points(height_twips);
            let width = height * natural_width / natural_height;
            (width, height)
        }
        (None, None) => (natural_width, natural_height),
    };
    width = width.max(1.0);
    height = height.max(1.0);
    width = (width - twips_to_points(image.crop.left_twips.saturating_add(image.crop.right_twips)))
        .max(1.0);
    height = (height
        - twips_to_points(image.crop.top_twips.saturating_add(image.crop.bottom_twips)))
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

fn shape_horizontal_origin(
    anchor: StaticShapeHorizontalAnchor,
    column_left: f32,
    geometry: &PageGeometry,
) -> f32 {
    match anchor {
        StaticShapeHorizontalAnchor::Column => column_left,
        StaticShapeHorizontalAnchor::Margin => geometry.margin_left,
        StaticShapeHorizontalAnchor::Page => 0.0,
    }
}

fn shape_vertical_origin(
    anchor: StaticShapeVerticalAnchor,
    paragraph_top: f32,
    geometry: &PageGeometry,
) -> f32 {
    match anchor {
        StaticShapeVerticalAnchor::Paragraph => paragraph_top,
        StaticShapeVerticalAnchor::Margin => geometry.height - geometry.margin_top,
        StaticShapeVerticalAnchor::Page => geometry.height,
    }
}

fn pdf_color_from_model_color(color: crate::model::Color) -> PdfColor {
    PdfColor {
        red: color.red as f32 / 255.0,
        green: color.green as f32 / 255.0,
        blue: color.blue as f32 / 255.0,
    }
}

#[allow(clippy::too_many_arguments)]
fn layout_shape(
    pages: &mut Vec<LayoutPage>,
    cursor_y: &mut f32,
    paragraph_anchor_y: Option<f32>,
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
    if shape.text_wrap && *cursor_y - block_height < margin_bottom {
        advance_column_or_page(pages, cursor_y, geometry, current_column);
        margin_left = geometry.body_left(*current_column);
    }

    let x = shape_horizontal_origin(shape.horizontal_anchor, margin_left, geometry) + left;
    let top_y = shape_vertical_origin(
        shape.vertical_anchor,
        paragraph_anchor_y.unwrap_or(*cursor_y),
        geometry,
    ) - top;
    let bottom_y = top_y - height;
    let right_x = x + width;
    let stroke_width_points = if shape.stroke_width_twips > 0 {
        Some(twips_to_points(shape.stroke_width_twips).clamp(0.14, 12.0))
    } else {
        None
    };
    let color = pdf_color_from_model_color(shape.stroke_color);
    let shadow_color = shape
        .shadow_enabled
        .then(|| pdf_color_from_model_color(shape.shadow_color));
    let fill_gradient_color = shape.fill_gradient_color.map(pdf_color_from_model_color);
    let shadow_offset_x = twips_to_points(shape.shadow_offset_x_twips);
    let shadow_offset_y = twips_to_points(shape.shadow_offset_y_twips);
    let stroke_style = line_style_for_border_style(shape.stroke_style);
    let stroke_cap = line_cap_for_shape_cap(shape.stroke_cap);
    let stroke_join = line_join_for_shape_join(shape.stroke_join);
    let Some(page) = pages.last_mut() else {
        return;
    };
    let shape_item_start = page.items.len();
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
                push_shape_line(
                    page,
                    start,
                    end,
                    width_points,
                    color,
                    stroke_style,
                    stroke_cap,
                );
                push_static_shape_arrowhead(
                    page,
                    shape.start_arrowhead,
                    shape.start_arrowhead_width_percent,
                    shape.start_arrowhead_length_percent,
                    start,
                    end,
                    width_points,
                    color,
                );
                push_static_shape_arrowhead(
                    page,
                    shape.end_arrowhead,
                    shape.end_arrowhead_width_percent,
                    shape.end_arrowhead_length_percent,
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
                if stroke_join == LineJoin::Miter {
                    for segment in points.windows(2) {
                        let start = segment[0];
                        let end = segment[1];
                        push_shape_line(
                            page,
                            start,
                            end,
                            width_points,
                            color,
                            stroke_style,
                            stroke_cap,
                        );
                    }
                } else if points.len() >= 2 {
                    page.items.push(LayoutItem::JoinedPolyline {
                        points: points.clone(),
                        width: width_points,
                        color,
                        style: stroke_style,
                        cap: stroke_cap,
                        join: stroke_join,
                    });
                }
                if let [first, second, ..] = points.as_slice() {
                    push_static_shape_arrowhead(
                        page,
                        shape.start_arrowhead,
                        shape.start_arrowhead_width_percent,
                        shape.start_arrowhead_length_percent,
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
                        shape.end_arrowhead_width_percent,
                        shape.end_arrowhead_length_percent,
                        last,
                        previous,
                        width_points,
                        color,
                    );
                }
                for overlay_path in &shape.overlay_paths {
                    let overlay_points = overlay_path
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
                    for segment in overlay_points.windows(2) {
                        push_shape_line(
                            page,
                            segment[0],
                            segment[1],
                            width_points,
                            color,
                            stroke_style,
                            stroke_cap,
                        );
                    }
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
            let paths = shape
                .point_paths
                .iter()
                .filter(|path| path.len() >= 3)
                .map(|path| {
                    path.iter()
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
                        .collect()
                })
                .collect::<Vec<_>>();
            let overlay_paths = shape
                .overlay_paths
                .iter()
                .filter(|path| path.len() >= 2)
                .map(|path| {
                    path.iter()
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
                        .collect()
                })
                .collect::<Vec<_>>();
            if let Some(shadow_color) = shadow_color {
                push_shape_shadow_item(
                    page,
                    LayoutItem::Polygon {
                        points: offset_shape_points(&points, shadow_offset_x, shadow_offset_y),
                        paths: offset_shape_paths(&paths, shadow_offset_x, shadow_offset_y),
                        overlay_paths: Vec::new(),
                        fill_rule: shape.fill_rule,
                        stroke_width: 0.0,
                        stroke_color: shadow_color,
                        stroke_style: LineStyle::Solid,
                        fill_color: Some(shadow_color),
                        fill_gradient_color: None,
                        fill_pattern: ShadingPattern::None,
                    },
                    shape.shadow_opacity_percent,
                );
            }
            page.items.push(LayoutItem::Polygon {
                points,
                paths,
                overlay_paths,
                fill_rule: shape.fill_rule,
                stroke_width: stroke_width_points.unwrap_or(0.0),
                stroke_color: color,
                stroke_style,
                fill_color: shape.fill_color.map(pdf_color_from_model_color),
                fill_gradient_color,
                fill_pattern: shape.fill_pattern,
            });
        }
        StaticShapeKind::Rectangle => {
            if let Some(shadow_color) =
                shadow_color.filter(|_| stroke_width_points.is_some() || shape.fill_color.is_some())
            {
                push_shape_shadow_item(
                    page,
                    LayoutItem::Highlight {
                        x: x + shadow_offset_x,
                        y: bottom_y - shadow_offset_y,
                        width,
                        height,
                        color: shadow_color,
                    },
                    shape.shadow_opacity_percent,
                );
            }
            if let Some(fill_color) = shape.fill_color {
                let color = PdfColor {
                    red: fill_color.red as f32 / 255.0,
                    green: fill_color.green as f32 / 255.0,
                    blue: fill_color.blue as f32 / 255.0,
                };
                if shape.fill_pattern == ShadingPattern::VerticalGradient {
                    let end_color = shape.fill_gradient_color.map(|gradient_color| PdfColor {
                        red: gradient_color.red as f32 / 255.0,
                        green: gradient_color.green as f32 / 255.0,
                        blue: gradient_color.blue as f32 / 255.0,
                    });
                    push_passive_vertical_gradient_bands(
                        page, x, bottom_y, width, height, color, end_color,
                    );
                } else {
                    page.items.push(LayoutItem::Highlight {
                        x,
                        y: bottom_y,
                        width,
                        height,
                        color,
                    });
                }
            }
            if shape.fill_pattern != ShadingPattern::VerticalGradient {
                push_shape_fill_pattern_lines(
                    page,
                    x,
                    bottom_y,
                    width,
                    height,
                    shape.fill_color,
                    shape.fill_pattern,
                );
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
            if let Some(shadow_color) = shadow_color {
                push_shape_shadow_item(
                    page,
                    LayoutItem::RoundedRectangle {
                        x: x + shadow_offset_x,
                        y: bottom_y - shadow_offset_y,
                        width,
                        height,
                        radius: (min_dimension * 0.2).clamp(1.0, min_dimension / 2.0),
                        stroke_width: 0.0,
                        stroke_color: shadow_color,
                        stroke_style: LineStyle::Solid,
                        fill_color: Some(shadow_color),
                        fill_gradient_color: None,
                        fill_pattern: ShadingPattern::None,
                    },
                    shape.shadow_opacity_percent,
                );
            }
            page.items.push(LayoutItem::RoundedRectangle {
                x,
                y: bottom_y,
                width,
                height,
                radius: (min_dimension * 0.2).clamp(1.0, min_dimension / 2.0),
                stroke_width: stroke_width_points.unwrap_or(0.0),
                stroke_color: color,
                stroke_style,
                fill_color: shape.fill_color.map(pdf_color_from_model_color),
                fill_gradient_color,
                fill_pattern: shape.fill_pattern,
            });
        }
        StaticShapeKind::Ellipse => {
            if stroke_width_points.is_some() || shape.fill_color.is_some() {
                if let Some(shadow_color) = shadow_color {
                    push_shape_shadow_item(
                        page,
                        LayoutItem::Ellipse {
                            x: x + shadow_offset_x,
                            y: bottom_y - shadow_offset_y,
                            width,
                            height,
                            stroke_width: 0.0,
                            stroke_color: shadow_color,
                            stroke_style: LineStyle::Solid,
                            fill_color: Some(shadow_color),
                            fill_gradient_color: None,
                            fill_pattern: ShadingPattern::None,
                        },
                        shape.shadow_opacity_percent,
                    );
                }
                page.items.push(LayoutItem::Ellipse {
                    x,
                    y: bottom_y,
                    width,
                    height,
                    stroke_width: stroke_width_points.unwrap_or(0.0),
                    stroke_color: color,
                    stroke_style,
                    fill_color: shape.fill_color.map(pdf_color_from_model_color),
                    fill_gradient_color,
                    fill_pattern: shape.fill_pattern,
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
    if let Some(page) = pages.last_mut() {
        if shape.text_wrap {
            let left_gap = twips_to_points(shape.wrap_margin_left_twips.max(0));
            let right_gap = twips_to_points(shape.wrap_margin_right_twips.max(0));
            let top_gap = twips_to_points(shape.wrap_margin_top_twips.max(0));
            let bottom_gap = twips_to_points(shape.wrap_margin_bottom_twips.max(0));
            page.flow_exclusions.push(FlowExclusion {
                x: (x - left_gap).max(0.0),
                y: bottom_y - bottom_gap,
                width: (width + left_gap + right_gap).max(1.0),
                height: (height + top_gap + bottom_gap).max(1.0),
                wrap_side: shape.wrap_side,
                wrap_bottom_inset: 0.0,
                kind: FlowExclusionKind::Generic,
            });
        }
        wrap_static_shape_items(
            page,
            shape_item_start,
            shape.z_order,
            shape.below_text,
            shape.fill_opacity_percent,
            shape.stroke_opacity_percent,
        );
    }
}

fn wrap_static_shape_items(
    page: &mut LayoutPage,
    start: usize,
    z_order: i32,
    below_text: bool,
    fill_opacity_percent: u8,
    stroke_opacity_percent: u8,
) {
    let has_order = z_order != 0 || below_text;
    let has_opacity = fill_opacity_percent < 100 || stroke_opacity_percent < 100;
    if (!has_order && !has_opacity) || start >= page.items.len() {
        return;
    }
    for item in &mut page.items[start..] {
        if has_opacity {
            let previous = item.clone();
            *item = LayoutItem::Opacity {
                fill_percent: fill_opacity_percent,
                stroke_percent: stroke_opacity_percent,
                item: Box::new(previous),
            };
        }
        if has_order {
            let previous = item.clone();
            *item = LayoutItem::Drawing(DrawingFragment {
                z_order,
                below_text,
                item: Box::new(previous),
            });
        }
    }
}

fn push_shape_shadow_item(page: &mut LayoutPage, item: LayoutItem, shadow_opacity_percent: u8) {
    if shadow_opacity_percent < 100 {
        page.items.push(LayoutItem::Opacity {
            fill_percent: shadow_opacity_percent,
            stroke_percent: shadow_opacity_percent,
            item: Box::new(item),
        });
    } else {
        page.items.push(item);
    }
}

fn apply_passive_drawing_z_order(pages: &mut [LayoutPage]) {
    for page in pages {
        move_below_text_drawings_to_background(page);
        sort_drawing_slots_by_z_order(&mut page.items);
        move_above_text_drawings_to_foreground(page);
    }
}

fn move_below_text_drawings_to_background(page: &mut LayoutPage) {
    let mut background = page
        .items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            drawing_order(item)
                .filter(|order| order.below_text)
                .map(|order| (order.z_order, index, item.clone()))
        })
        .collect::<Vec<_>>();
    if background.is_empty() {
        return;
    }
    background.sort_by(|(z_a, index_a, _), (z_b, index_b, _)| {
        z_a.cmp(z_b).then_with(|| index_a.cmp(index_b))
    });
    let foreground = page
        .items
        .iter()
        .filter(|item| !drawing_order(item).is_some_and(|order| order.below_text))
        .cloned()
        .collect::<Vec<_>>();
    page.items = background
        .into_iter()
        .map(|(_, _, item)| item)
        .chain(foreground)
        .collect();
}

fn sort_drawing_slots_by_z_order(items: &mut [LayoutItem]) {
    let mut drawing_slots = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            drawing_order(item)
                .filter(|order| !order.below_text)
                .map(|order| (index, order.z_order))
        })
        .collect::<Vec<_>>();
    if drawing_slots.len() <= 1 {
        return;
    }
    let mut drawings = drawing_slots
        .iter()
        .map(|(index, z_order)| (*z_order, *index, items[*index].clone()))
        .collect::<Vec<_>>();
    drawings.sort_by(|(z_a, index_a, _), (z_b, index_b, _)| {
        z_a.cmp(z_b).then_with(|| index_a.cmp(index_b))
    });
    drawing_slots.sort_by_key(|(index, _)| *index);
    for ((slot_index, _), (_, _, drawing)) in drawing_slots.into_iter().zip(drawings) {
        items[slot_index] = drawing;
    }
}

fn move_above_text_drawings_to_foreground(page: &mut LayoutPage) {
    let mut foreground = page
        .items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            drawing_order(item)
                .filter(|order| !order.below_text)
                .filter(|order| matches!(item, LayoutItem::Drawing(_)) || order.z_order != 0)
                .map(|order| (order.z_order, index, item.clone()))
        })
        .collect::<Vec<_>>();
    if foreground.is_empty() {
        return;
    }
    foreground.sort_by(|(z_a, index_a, _), (z_b, index_b, _)| {
        z_a.cmp(z_b).then_with(|| index_a.cmp(index_b))
    });
    let foreground_indices = foreground
        .iter()
        .map(|(_, index, _)| *index)
        .collect::<HashSet<_>>();
    let mut reordered = page
        .items
        .iter()
        .enumerate()
        .filter(|(index, _)| !foreground_indices.contains(index))
        .map(|(_, item)| item.clone())
        .collect::<Vec<_>>();
    reordered.extend(foreground.into_iter().map(|(_, _, item)| item));
    page.items = reordered;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct DrawingOrder {
    z_order: i32,
    below_text: bool,
}

fn drawing_order(item: &LayoutItem) -> Option<DrawingOrder> {
    match item {
        LayoutItem::Image(fragment) => Some(DrawingOrder {
            z_order: fragment.z_order,
            below_text: fragment.below_text,
        }),
        LayoutItem::Drawing(fragment) => Some(DrawingOrder {
            z_order: fragment.z_order,
            below_text: fragment.below_text,
        }),
        _ => None,
    }
}

#[cfg(test)]
fn drawing_z_order(item: &LayoutItem) -> Option<i32> {
    drawing_order(item).map(|order| order.z_order)
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

fn offset_shape_points(points: &[LayoutPoint], offset_x: f32, offset_y: f32) -> Vec<LayoutPoint> {
    points
        .iter()
        .map(|point| LayoutPoint {
            x: point.x + offset_x,
            y: point.y - offset_y,
        })
        .collect()
}

fn offset_shape_paths(
    paths: &[Vec<LayoutPoint>],
    offset_x: f32,
    offset_y: f32,
) -> Vec<Vec<LayoutPoint>> {
    paths
        .iter()
        .map(|path| offset_shape_points(path, offset_x, offset_y))
        .collect()
}

fn push_shape_line(
    page: &mut LayoutPage,
    start: LayoutPoint,
    end: LayoutPoint,
    width: f32,
    color: PdfColor,
    style: LineStyle,
    cap: LineCap,
) {
    if cap == LineCap::Flat {
        page.items.push(LayoutItem::Line {
            x1: start.x,
            y1: start.y,
            x2: end.x,
            y2: end.y,
            width,
            color,
            style,
        });
    } else {
        page.items.push(LayoutItem::CappedLine {
            x1: start.x,
            y1: start.y,
            x2: end.x,
            y2: end.y,
            width,
            color,
            style,
            cap,
        });
    }
}

fn push_static_shape_arrowhead(
    page: &mut LayoutPage,
    arrowhead: StaticShapeArrowhead,
    width_percent: i32,
    length_percent: i32,
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
    let length_scale = (length_percent.clamp(50, 160) as f32) / 100.0;
    let width_scale = (width_percent.clamp(50, 160) as f32) / 100.0;
    let arrow_length = ((stroke_width * 5.0 + 6.0) * length_scale)
        .clamp(6.0, 18.0)
        .min(length * 0.6);
    let half_width = (arrow_length * 0.45 * width_scale).clamp(2.0, 14.0);
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
                paths: Vec::new(),
                overlay_paths: Vec::new(),
                fill_rule: StaticImageVectorFillRule::Winding,
                stroke_width,
                stroke_color: color,
                stroke_style: LineStyle::Solid,
                fill_color: Some(color),
                fill_gradient_color: None,
                fill_pattern: ShadingPattern::None,
            });
        }
        StaticShapeArrowhead::Block => {
            let rear_half_width = half_width * 0.58;
            let rear_a = LayoutPoint {
                x: base.x + perpendicular_x * rear_half_width,
                y: base.y + perpendicular_y * rear_half_width,
            };
            let rear_b = LayoutPoint {
                x: base.x - perpendicular_x * rear_half_width,
                y: base.y - perpendicular_y * rear_half_width,
            };
            page.items.push(LayoutItem::Polygon {
                points: vec![tip, wing_a, rear_a, rear_b, wing_b],
                paths: Vec::new(),
                overlay_paths: Vec::new(),
                fill_rule: StaticImageVectorFillRule::Winding,
                stroke_width,
                stroke_color: color,
                stroke_style: LineStyle::Solid,
                fill_color: Some(color),
                fill_gradient_color: None,
                fill_pattern: ShadingPattern::None,
            });
        }
        StaticShapeArrowhead::Diamond => {
            let center = LayoutPoint {
                x: tip.x - unit_x * (arrow_length * 0.55),
                y: tip.y - unit_y * (arrow_length * 0.55),
            };
            let side_a = LayoutPoint {
                x: center.x + perpendicular_x * half_width,
                y: center.y + perpendicular_y * half_width,
            };
            let side_b = LayoutPoint {
                x: center.x - perpendicular_x * half_width,
                y: center.y - perpendicular_y * half_width,
            };
            let tail = LayoutPoint {
                x: tip.x - unit_x * arrow_length,
                y: tip.y - unit_y * arrow_length,
            };
            page.items.push(LayoutItem::Polygon {
                points: vec![tip, side_a, tail, side_b],
                paths: Vec::new(),
                overlay_paths: Vec::new(),
                fill_rule: StaticImageVectorFillRule::Winding,
                stroke_width,
                stroke_color: color,
                stroke_style: LineStyle::Solid,
                fill_color: Some(color),
                fill_gradient_color: None,
                fill_pattern: ShadingPattern::None,
            });
        }
        StaticShapeArrowhead::Oval => {
            let center = LayoutPoint {
                x: tip.x - unit_x * (arrow_length * 0.45),
                y: tip.y - unit_y * (arrow_length * 0.45),
            };
            let radius_x = (arrow_length * 0.38).clamp(2.0, 10.0);
            let radius_y = half_width.clamp(2.0, 10.0);
            const OVAL_ARROWHEAD_SEGMENTS: usize = 24;
            let mut points = Vec::with_capacity(OVAL_ARROWHEAD_SEGMENTS);
            for index in 0..OVAL_ARROWHEAD_SEGMENTS {
                let angle =
                    std::f32::consts::PI * 2.0 * (index as f32) / OVAL_ARROWHEAD_SEGMENTS as f32;
                let along = angle.cos() * radius_x;
                let across = angle.sin() * radius_y;
                points.push(LayoutPoint {
                    x: center.x + unit_x * along + perpendicular_x * across,
                    y: center.y + unit_y * along + perpendicular_y * across,
                });
            }
            page.items.push(LayoutItem::Polygon {
                points,
                paths: Vec::new(),
                overlay_paths: Vec::new(),
                fill_rule: StaticImageVectorFillRule::Winding,
                stroke_width,
                stroke_color: color,
                stroke_style: LineStyle::Solid,
                fill_color: Some(color),
                fill_gradient_color: None,
                fill_pattern: ShadingPattern::None,
            });
        }
        StaticShapeArrowhead::Stealth => {
            let notch = LayoutPoint {
                x: tip.x - unit_x * (arrow_length * 0.62),
                y: tip.y - unit_y * (arrow_length * 0.62),
            };
            let tail_half_width = half_width * 0.42;
            let tail_a = LayoutPoint {
                x: base.x + perpendicular_x * tail_half_width,
                y: base.y + perpendicular_y * tail_half_width,
            };
            let tail_b = LayoutPoint {
                x: base.x - perpendicular_x * tail_half_width,
                y: base.y - perpendicular_y * tail_half_width,
            };
            page.items.push(LayoutItem::Polygon {
                points: vec![tip, wing_a, tail_a, notch, tail_b, wing_b],
                paths: Vec::new(),
                overlay_paths: Vec::new(),
                fill_rule: StaticImageVectorFillRule::Winding,
                stroke_width,
                stroke_color: color,
                stroke_style: LineStyle::Solid,
                fill_color: Some(color),
                fill_gradient_color: None,
                fill_pattern: ShadingPattern::None,
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
    let margin_left = twips_to_points(shape.text_margin_left_twips.max(0));
    let margin_right = twips_to_points(shape.text_margin_right_twips.max(0));
    let margin_top = twips_to_points(shape.text_margin_top_twips.max(0));
    let margin_bottom = twips_to_points(shape.text_margin_bottom_twips.max(0));
    let content_x = x + margin_left;
    let content_width = (width - margin_left - margin_right).max(1.0);
    let min_y = bottom_y + margin_bottom;
    let markers = current_marker_context(pages, document_stats);
    let mut prepared = Vec::new();
    let mut text_height = 0.0;
    for paragraph in &shape.text {
        let space_before = twips_to_points(effective_space_before_twips(&paragraph.style));
        text_height += space_before;
        let mut lines = wrap_paragraph_with_font_provider(
            paragraph,
            content_width,
            &markers,
            document,
            font_provider,
        );
        for line in &mut lines {
            line.height = apply_line_spacing(line.height, &paragraph.style);
            if !paragraph.style.line_spacing_multiple {
                line.natural_height = line.height;
            }
        }
        text_height += lines.iter().map(|line| line.height).sum::<f32>();
        let space_after = twips_to_points(effective_space_after_twips(&paragraph.style));
        text_height += space_after;
        prepared.push((paragraph, lines, space_before, space_after));
    }
    let available_height = (top_y - margin_top - min_y).max(0.0);
    let vertical_offset = match shape.text_vertical_anchor {
        StaticShapeTextVerticalAnchor::Top => 0.0,
        StaticShapeTextVerticalAnchor::Middle => ((available_height - text_height) / 2.0).max(0.0),
        StaticShapeTextVerticalAnchor::Bottom => (available_height - text_height).max(0.0),
    };
    let mut cursor_y = top_y - margin_top - vertical_offset;

    for (paragraph, lines, space_before, space_after) in prepared {
        cursor_y -= space_before;
        let line_count = lines.len();
        for (line_idx, line) in lines.iter().enumerate() {
            if cursor_y - line.height < min_y {
                return;
            }
            let is_first_line = line_idx == 0;
            let is_last_line = line_idx + 1 == line_count;
            if shading_is_visible(
                document,
                paragraph.style.shading_color_index,
                paragraph.style.shading_pattern,
            ) {
                let line_left_indent = twips_to_points(paragraph_line_left_indent_twips(
                    &paragraph.style,
                    is_first_line,
                ));
                push_shading_rect(
                    pages,
                    document,
                    content_x + line_left_indent,
                    cursor_y - line.height,
                    paragraph_line_width(content_width, &paragraph.style, is_first_line),
                    line.height,
                    paragraph.style.shading_color_index,
                    paragraph.style.shading_basis_points,
                    paragraph.style.shading_pattern,
                );
            }
            let text_x = if shape.text_horizontal_anchor_centered {
                content_x + ((content_width - line.width) / 2.0).max(0.0)
            } else {
                aligned_x(
                    content_x,
                    content_width,
                    line.width,
                    &paragraph.style,
                    is_first_line,
                )
            };
            let word_spacing = justified_word_spacing(
                line,
                &paragraph.style,
                paragraph_line_width(content_width, &paragraph.style, is_first_line),
                is_last_line,
                document,
                font_provider,
            );
            let character_spacing = distributed_character_spacing(
                line,
                &paragraph.style,
                paragraph_line_width(content_width, &paragraph.style, is_first_line),
                is_last_line,
            );
            let (border_line_idx, border_line_count) =
                paragraph_border_line_position(is_first_line, is_last_line);
            push_paragraph_borders(
                pages,
                content_x,
                content_width,
                &paragraph.style,
                border_line_idx,
                border_line_count,
                cursor_y,
                line.height,
                document,
                false,
                false,
            );
            push_bar_tab_stops(pages, &paragraph.style, text_x, cursor_y, line.height);
            push_line(
                pages,
                line,
                text_x,
                cursor_y,
                document,
                font_provider,
                word_spacing,
                character_spacing,
            );
            cursor_y -= line.height;
        }
        cursor_y -= space_after;
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
fn kept_paragraphs_would_advance_to_page(
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
        + paragraph_top_border_reserve(&paragraph.style, document)
        + line_height
        + paragraph_bottom_border_reserve(&paragraph.style, false, document)
        + twips_to_points(effective_space_after_twips(&paragraph.style))
}

fn next_paragraph_block(blocks: &[Block], idx: usize) -> Option<&Paragraph> {
    blocks.get(idx + 1).and_then(|block| match block {
        Block::Paragraph(paragraph) => Some(paragraph),
        _ => None,
    })
}

fn keep_with_next_chain(blocks: &[Block], idx: usize) -> Vec<&Paragraph> {
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

fn previous_paragraph_block(blocks: &[Block], idx: usize) -> Option<&Paragraph> {
    idx.checked_sub(1)
        .and_then(|previous_idx| blocks.get(previous_idx))
        .and_then(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
}

fn previous_non_shape_block_is_paragraph(blocks: &[Block], idx: usize) -> bool {
    blocks[..idx]
        .iter()
        .rev()
        .find(|block| !matches!(block, Block::Shape(_)))
        .is_some_and(|block| matches!(block, Block::Paragraph(_)))
}

fn paragraph_spacing_is_contextual(first: &Paragraph, second: &Paragraph) -> bool {
    first.style.contextual_spacing || second.style.contextual_spacing
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
            for exclusion in &mut page.flow_exclusions {
                exclusion.y -= shift_down;
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
    layout_items_vertical_bounds(&page.items)
}

fn layout_items_vertical_bounds(items: &[LayoutItem]) -> Option<VerticalBounds> {
    items
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

fn translate_layout_item_y(item: &mut LayoutItem, delta_y: f32) {
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

fn layout_column_separators(pages: &mut [LayoutPage]) {
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

fn column_separator_content_bounds(items: &[LayoutItem]) -> Option<VerticalBounds> {
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

fn layout_page_borders(
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

fn layout_legacy_auto_page_numbers(
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

fn table_row_contains_no_wrap_text(row: &TableRow) -> bool {
    row.cells.iter().any(|cell| {
        cell.paragraphs
            .iter()
            .any(|paragraph| paragraph.style.no_wrap)
    })
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
            font_provider,
            borders_visible,
            true,
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

fn nested_repeat_header_lines(
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

fn prepend_nested_repeat_header_lines(
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

fn prepared_table_row_has_lines(prepared: &PreparedTableRow) -> bool {
    prepared.cell_lines.iter().any(|lines| !lines.is_empty())
}

fn prepared_table_row_content_height(prepared: &PreparedTableRow) -> f32 {
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

fn prepared_cell_lines_height(
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

fn prepared_cell_line_extent(line: &PreparedCellLine, direction: TableCellTextDirection) -> f32 {
    match direction {
        TableCellTextDirection::LeftToRightTopToBottom => line.line.height,
        TableCellTextDirection::TopToBottomRightToLeft
        | TableCellTextDirection::BottomToTopLeftToRight => line.line.width.max(line.line.height),
    }
}

fn table_cell_text_rotation(direction: TableCellTextDirection) -> TextRotation {
    match direction {
        TableCellTextDirection::LeftToRightTopToBottom => TextRotation::None,
        TableCellTextDirection::TopToBottomRightToLeft => TextRotation::Clockwise90,
        TableCellTextDirection::BottomToTopLeftToRight => TextRotation::CounterClockwise90,
    }
}

fn table_row_vertical_offset_points(row: &TableRow, row_height: f32) -> f32 {
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

fn dynamic_table_row_vertical_offset_twips(value: i32, base: i32) -> Option<i32> {
    let offset = value.checked_sub(base)?;
    if offset.abs() <= TABLE_ROW_DYNAMIC_VERTICAL_OFFSET_SPAN_TWIPS {
        Some(offset)
    } else {
        None
    }
}

#[derive(Debug, Copy, Clone, Default)]
struct TableRowWrapMarginPoints {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

fn table_row_wrap_margin_points(row: &TableRow) -> TableRowWrapMarginPoints {
    TableRowWrapMarginPoints {
        left: twips_to_points(row.wrap_margins.left_twips.max(0)),
        right: twips_to_points(row.wrap_margins.right_twips.max(0)),
        top: twips_to_points(row.wrap_margins.top_twips.max(0)),
        bottom: twips_to_points(row.wrap_margins.bottom_twips.max(0)),
    }
}

fn table_row_has_wrap_margins(row: &TableRow) -> bool {
    let TableRowWrapMargins {
        left_twips,
        right_twips,
        top_twips,
        bottom_twips,
    } = row.wrap_margins;
    left_twips > 0 || right_twips > 0 || top_twips > 0 || bottom_twips > 0
}

fn non_overlapping_table_row_top(
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

fn push_table_row_flow_exclusion(
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
fn preview_immediately_following_floating_table_exclusion(
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

fn rotated_table_cell_line_origin(
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

fn rotated_paragraph_frame_line_origin(
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

fn paragraph_frame_vertical_column_advance(line: &Line) -> f32 {
    let word_pitch = line
        .runs
        .iter()
        .map(|run| run.style.font_size_points() * 1.18)
        .fold(0.0_f32, f32::max);
    line.height.max(word_pitch).max(1.0)
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

fn table_cell_paragraphs_for_layout(cell: &TableCell) -> Vec<Paragraph> {
    let Some(nested_table) = cell.nested_table.as_deref() else {
        return cell.paragraphs.clone();
    };

    let has_anchor = cell.paragraphs.iter().any(|paragraph| {
        paragraph
            .runs
            .iter()
            .any(|run| run.text.contains(NESTED_TABLE_ANCHOR_MARKER))
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
        let Some((anchor_run_index, anchor_byte_index)) =
            paragraph.runs.iter().enumerate().find_map(|(index, run)| {
                run.text
                    .find(NESTED_TABLE_ANCHOR_MARKER)
                    .map(|byte_index| (index, byte_index))
            })
        else {
            paragraphs.push(paragraph.clone());
            continue;
        };

        let mut before = paragraph.runs[..anchor_run_index].to_vec();
        let anchor_run = &paragraph.runs[anchor_run_index];
        if anchor_byte_index > 0 {
            before.push(Run {
                text: anchor_run.text[..anchor_byte_index].to_string(),
                style: anchor_run.style.clone(),
            });
        }
        if !before.is_empty() {
            paragraphs.push(Paragraph {
                style: paragraph.style.clone(),
                runs: before,
            });
        }
        append_nested_table_paragraphs(&mut paragraphs, nested_table, 0);

        let suffix_start = anchor_byte_index + NESTED_TABLE_ANCHOR_MARKER.len();
        let mut after = Vec::new();
        if suffix_start < anchor_run.text.len() {
            after.push(Run {
                text: anchor_run.text[suffix_start..].to_string(),
                style: anchor_run.style.clone(),
            });
        }
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

fn append_nested_table_paragraphs(paragraphs: &mut Vec<Paragraph>, table: &Table, depth: usize) {
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
                    text: "\t".to_string(),
                    style: CharacterStyle::default(),
                });
            }
            for (paragraph_index, paragraph) in cell.paragraphs.iter().enumerate() {
                if paragraph_index > 0 {
                    runs.push(Run {
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

fn nested_cell_left_inset_twips(cell: &TableCell) -> i32 {
    cell.padding
        .left_twips
        .unwrap_or(0)
        .max(0)
        .saturating_add(cell.spacing.left_twips.unwrap_or(0).max(0))
        .min(100_000)
}

fn passive_fit_text_paragraph(paragraph: &Paragraph, content_width: f32) -> Paragraph {
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

fn has_heterogeneous_horizontal_cell_spacing(spacings: &[ResolvedCellSpacing]) -> bool {
    spacings.windows(2).any(|pair| {
        (pair[0].left - pair[1].left).abs() > f32::EPSILON
            || (pair[0].right - pair[1].right).abs() > f32::EPSILON
    })
}

fn distribute_heterogeneous_cell_spacing(spacings: &mut [ResolvedCellSpacing]) {
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

fn distributed_table_row_top_spacing_points(row: &TableRow) -> f32 {
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

fn positioned_table_row_left(
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

fn lines_for_cell<'a>(prepared: &'a PreparedTableRow, index: usize) -> &'a [PreparedCellLine] {
    prepared
        .cell_lines
        .get(index)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn nested_table_row_range(cell: &TableCell, lines: &[PreparedCellLine]) -> Option<(usize, usize)> {
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

fn nested_table_paragraph_start(cell: &TableCell) -> usize {
    let Some((paragraph_index, run_index, byte_index)) = cell
        .paragraphs
        .iter()
        .enumerate()
        .find_map(|(paragraph_index, paragraph)| {
            paragraph
                .runs
                .iter()
                .enumerate()
                .find_map(|(run_index, run)| {
                    run.text
                        .find(NESTED_TABLE_ANCHOR_MARKER)
                        .map(|byte_index| (paragraph_index, run_index, byte_index))
                })
        })
    else {
        return cell.paragraphs.len();
    };

    let has_prefix_text = byte_index > 0
        || cell.paragraphs[paragraph_index].runs[..run_index]
            .iter()
            .any(|run| !run.text.is_empty());
    paragraph_index + usize::from(has_prefix_text)
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

fn inside_outside_physical_alignment(
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

fn table_row_column_widths(
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

fn push_nested_table_grid(
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
fn push_nested_table_grid_at_depth(
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

fn nested_vertical_span_height(
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

fn nested_vertical_merge_origin(
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

fn nested_table_row_left(
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

fn nested_table_row_heights(
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
        .unwrap_or_default();
    let style = line_style_for_border_style(border.style);
    (width, color, style)
}

fn diagonal_table_border_stroke(
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

fn page_border_stroke(border: &TableCellBorder, document: &Document) -> (f32, PdfColor, LineStyle) {
    let (mut width, color, style) = table_border_stroke(border, document);
    if border.width_twips == TableCellBorder::default().width_twips
        && border.style == BorderStyle::Single
    {
        width = 0.72;
    }
    (width, color, style)
}

fn layout_scissors_page_border_art(
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

fn paragraph_top_border_reserve(style: &ParagraphStyle, document: &Document) -> f32 {
    if style.borders.top.visible {
        table_border_stroke(&style.borders.top, document).0
    } else {
        0.0
    }
}

fn paragraph_bottom_border_reserve(
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

struct AutoFootnoteFlush<'a> {
    rendered_footnote_pages: &'a mut [Option<usize>],
    block_index: usize,
    rendered_footnote_index_on_page: Option<usize>,
    reserved_footnote_margin_bottom_on_page: Option<f32>,
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

struct PreparedParagraphFrameGeometry {
    width: f32,
    height: f32,
    lines: Vec<Vec<Line>>,
}

#[allow(clippy::too_many_arguments)]
fn prepare_paragraph_frame_geometry(
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
fn layout_framed_paragraph_group(
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

fn word_paragraph_relative_drop_cap_alignment_paint_rise_points(style: &ParagraphStyle) -> f32 {
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

fn passive_frame_content_paragraph(paragraph: &Paragraph) -> Paragraph {
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

fn passive_negative_width_frame_paragraph(paragraph: &Paragraph) -> Paragraph {
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

fn paragraph_frame_wrap_side(paragraph: &Paragraph) -> StaticImageWrapSide {
    if paragraph.style.drop_cap_lines > 1 {
        StaticImageWrapSide::Both
    } else {
        StaticImageWrapSide::Right
    }
}

fn is_complete_absolute_paragraph_frame(paragraph: &Paragraph) -> bool {
    (paragraph.style.frame_width_twips.is_some() || paragraph.style.frame_height_twips.is_some())
        && !paragraph
            .style
            .frame_width_twips
            .is_some_and(|width| width < 0)
        && !paragraph_frame_uses_flow_position(&paragraph.style)
}

fn paragraph_frame_uses_flow_position(style: &ParagraphStyle) -> bool {
    style.frame_vertical_anchor == StaticShapeVerticalAnchor::Paragraph
        || (style.frame_vertical_anchor != StaticShapeVerticalAnchor::Page
            && style.frame_vertical_alignment == Some(ParagraphFrameVerticalAlignment::Inline))
}

fn same_absolute_paragraph_frame(first: &Paragraph, second: &Paragraph) -> bool {
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

fn absolute_paragraph_frame_group<'a>(
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

fn take_pending_absolute_frame_preview(
    pages: &[LayoutPage],
    pending: &mut HashMap<usize, PendingAbsoluteFramePreview>,
    block_index: usize,
) -> Option<FlowExclusion> {
    let Some(preview) = pending.remove(&block_index) else {
        return None;
    };
    (preview.page_index + 1 == pages.len()).then_some(preview.exclusion)
}

fn same_flow_exclusion(first: FlowExclusion, second: FlowExclusion) -> bool {
    (first.x - second.x).abs() < 0.01
        && (first.y - second.y).abs() < 0.01
        && (first.width - second.width).abs() < 0.01
        && (first.height - second.height).abs() < 0.01
        && first.wrap_side == second.wrap_side
        && (first.wrap_bottom_inset - second.wrap_bottom_inset).abs() < 0.01
        && first.kind == second.kind
}

fn stacked_absolute_frame_top(
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

fn paragraph_frame_vertical_text_distance(style: &ParagraphStyle, page_height: f32) -> f32 {
    if style.drop_cap_lines > 1 {
        0.0
    } else {
        twips_to_points(style.frame_vertical_text_distance_twips.max(0)).min(page_height.max(0.0))
    }
}

fn paragraph_frame_exclusion_vertical_geometry(
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
fn preview_future_absolute_frame_exclusions(
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
fn resolve_absolute_frame_exclusion(
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

fn paragraph_frame_left(
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

fn paragraph_frame_top(
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
fn layout_paragraph_with_auto_footnotes(
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

fn word_two_in_one_layout_paragraph(paragraph: &Paragraph) -> Option<Paragraph> {
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

fn run_page_advance_hook(
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
fn flush_auto_footnotes_before_page_advance(
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

fn max_footnote_reference_index_in_line(line: &Line) -> Option<usize> {
    line.runs
        .iter()
        .filter_map(|run| max_footnote_reference_index_in_text(&run.text))
        .max()
}

fn max_footnote_reference_index_in_text(text: &str) -> Option<usize> {
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

fn wrapped_image_text_intervals_for_line(
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

fn apply_flow_exclusions_to_text_intervals(
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
fn subtract_flow_exclusion_from_intervals(
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

fn line_has_visible_text(line: &Line) -> bool {
    line.runs.iter().any(|run| {
        !run.style.hidden
            && !is_passive_advance_marker(&run.text)
            && !is_passive_math_inline_lead_marker(&run.text)
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
        |_, _| vec![(0.0, content_width)],
    )
}

fn wrap_paragraph_with_font_provider_dynamic_width(
    paragraph: &Paragraph,
    content_width: f32,
    markers: &MarkerContext,
    document: &Document,
    font_provider: Option<&FontProvider>,
    first_line_top_y: f32,
    mut line_content_intervals: impl FnMut(f32, f32) -> Vec<(f32, f32)>,
) -> Vec<Line> {
    let fitted_paragraph;
    let paragraph = if paragraph
        .runs
        .iter()
        .any(|run| run.style.fit_text_width_twips.is_some())
    {
        fitted_paragraph = apply_fit_text_scaling_to_run_groups(paragraph, document, font_provider);
        &fitted_paragraph
    } else {
        paragraph
    };
    let operator_spaced_math_paragraph = apply_passive_math_operator_spacing(paragraph);
    let paragraph = operator_spaced_math_paragraph.as_ref().unwrap_or(paragraph);
    let grouped_math_nary_paragraph =
        group_word_compatible_passive_math_nary_runs(paragraph, document, font_provider);
    let paragraph = grouped_math_nary_paragraph.as_ref().unwrap_or(paragraph);
    let nary_spaced_math_paragraph = apply_passive_math_nary_base_spacing(paragraph);
    let paragraph = nary_spaced_math_paragraph.as_ref().unwrap_or(paragraph);
    let stacked_math_limit_paragraph =
        stack_passive_math_limits(paragraph, document, font_provider);
    let paragraph = stacked_math_limit_paragraph.as_ref().unwrap_or(paragraph);
    let stacked_math_paragraph = stack_passive_math_fractions(paragraph, document, font_provider);
    let paragraph = stacked_math_paragraph.as_ref().unwrap_or(paragraph);
    let stacked_math_array_paragraph =
        stack_passive_math_equation_arrays(paragraph, document, font_provider);
    let paragraph = stacked_math_array_paragraph.as_ref().unwrap_or(paragraph);
    let mut lines = Vec::new();
    let mut current = Line {
        runs: Vec::new(),
        width: 0.0,
        natural_height: 14.0,
        height: 14.0,
        segmented_flow: false,
        inline_image_baseline_context: false,
    };
    let mut is_first_line = true;
    let mut current_line_top_y = first_line_top_y;
    let mut drop_cap_applied = false;

    for (run_index, run) in paragraph.runs.iter().enumerate() {
        let trailing_character_spacing = character_spacing_boundary_after_run(paragraph, run_index);
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
        if paragraph.style.no_wrap {
            segments = apply_no_wrap_emergency_boundaries(
                segments,
                content_width,
                &paragraph.style,
                document,
                font_provider,
            );
        } else {
            segments = apply_emergency_word_wrapping(
                segments,
                content_width,
                &paragraph.style,
                document,
                font_provider,
            );
        }
        for (segment_index, mut segment) in segments.iter().cloned().enumerate() {
            if segment.text == "\n" {
                let finished_height = current.height;
                lines.push(current);
                current_line_top_y -= finished_height;
                current = empty_line();
                is_first_line = false;
                continue;
            }
            if is_passive_math_inline_lead_marker(&segment.text) {
                if current.width > 0.01 {
                    // Word leaves the invisible object lead-in on the preceding line
                    // when the following atomic equation wraps.
                    push_segment(
                        &mut current,
                        segment,
                        &paragraph.style,
                        document,
                        font_provider,
                    );
                }
                continue;
            }

            let mut width = measure_flow_run(
                &segment,
                current.width,
                &paragraph.style,
                document,
                font_provider,
            );
            let mut fit_width = inline_image_cluster_width(
                &segments,
                segment_index,
                width,
                current.width,
                &paragraph.style,
                document,
                font_provider,
            );
            if is_passive_math_fraction_rule_marker(&segment.text) {
                fit_width = passive_math_fraction_width_points(&segment.style);
            }
            if is_passive_math_stack_anchor_marker(&segment.text) {
                fit_width = passive_math_stack_width_points(&segment.style);
            }
            let interval_fit_width = if segment.text.ends_with(char::is_whitespace)
                && !contains_inline_marker(&segment.text)
            {
                let mut visible_segment = segment.clone();
                visible_segment.text = visible_segment
                    .text
                    .trim_end_matches(char::is_whitespace)
                    .to_string();
                measure_flow_run(
                    &visible_segment,
                    current.width,
                    &paragraph.style,
                    document,
                    font_provider,
                )
            } else {
                width
            };
            let mut intervals = line_content_intervals(current_line_top_y, current.height);
            intervals.retain(|(_, width)| *width >= 12.0);
            intervals.sort_by(|(left_a, _), (left_b, _)| left_a.total_cmp(left_b));
            if intervals.is_empty() {
                intervals.push((0.0, content_width));
            }
            let supports_segmented_flow = paragraph.style.alignment == Alignment::Left
                && paragraph.style.left_indent_twips == 0
                && paragraph.style.right_indent_twips == 0
                && paragraph.style.first_line_indent_twips == 0
                && paragraph.style.tab_stops_twips.is_empty()
                && intervals.len() > 1;
            current.segmented_flow |= supports_segmented_flow;
            let (line_width, ignore_trailing_space_for_fit) = if supports_segmented_flow {
                let current_interval = intervals
                    .iter()
                    .position(|(left, interval_width)| {
                        current.width + 0.01 >= *left
                            && current.width <= *left + *interval_width + 0.01
                    })
                    .unwrap_or(0);
                let (interval_left, interval_width) = intervals[current_interval];
                if current.runs.is_empty() && current.width + 0.01 < interval_left {
                    push_flow_exclusion_advance(&mut current, interval_left, &segment.style);
                }
                let interval_right = interval_left + interval_width;
                if current.width + interval_fit_width > interval_right {
                    if let Some((next_left, _)) = intervals
                        .iter()
                        .skip(current_interval + 1)
                        .find(|(_, next_width)| interval_fit_width <= *next_width)
                        .copied()
                    {
                        let advance = (next_left - current.width).max(0.0);
                        push_flow_exclusion_advance(&mut current, advance, &segment.style);
                    }
                }
                let line_width = intervals
                    .iter()
                    .find(|(left, interval_width)| {
                        current.width + 0.01 >= *left
                            && current.width <= *left + *interval_width + 0.01
                    })
                    .map(|(left, interval_width)| left + interval_width)
                    .unwrap_or(interval_right);
                (line_width, line_width + 0.01 < content_width)
            } else {
                let (requested_left, requested_content_width) = intervals
                    .into_iter()
                    .max_by(|(_, width_a), (_, width_b)| width_a.total_cmp(width_b))
                    .unwrap_or((0.0, content_width));
                let active_content_width = if content_width >= 12.0 {
                    requested_content_width.clamp(12.0, content_width)
                } else {
                    requested_content_width.clamp(0.0, content_width.max(0.0))
                };
                (
                    paragraph_line_width(active_content_width, &paragraph.style, is_first_line),
                    requested_left.abs() < 0.01
                        && (requested_content_width - content_width).abs() < 0.01,
                )
            };
            let dynamic_word = segment
                .text
                .trim_end_matches(char::is_whitespace)
                .to_string();
            let dynamic_trailing = segment.text[dynamic_word.len()..].to_string();
            if current.runs.is_empty()
                && width > line_width + 0.01
                && line_width + 0.01 < content_width
                && segment.style.fit_text_width_twips.is_none()
                && segment.tab_stop_position.is_none()
                && segment.text != "\t"
                && segment.style.passive_math_fraction_id.is_none()
                && !contains_inline_marker(&segment.text)
                && dynamic_word.chars().count() >= 2
                && !dynamic_word.chars().any(char::is_whitespace)
            {
                let display = display_text(&dynamic_word, &segment.style);
                let family = font_family_for_run_text(document, &segment.style, &display);
                let mut pieces = Vec::new();
                let mut split_segment = segment.clone();
                split_segment.text = dynamic_word;
                push_emergency_wrapped_segment(
                    &mut pieces,
                    split_segment,
                    line_width.max(1.0),
                    family,
                    document,
                    font_provider,
                );
                if pieces.len() > 1 {
                    segment = pieces.pop().expect("dynamic emergency segment tail");
                    segment.text.push_str(&dynamic_trailing);
                    for piece in pieces {
                        push_segment(
                            &mut current,
                            piece,
                            &paragraph.style,
                            document,
                            font_provider,
                        );
                        let finished_height = current.height;
                        lines.push(current);
                        current_line_top_y -= finished_height;
                        current = empty_line();
                        is_first_line = false;
                    }
                    width = measure_flow_run(
                        &segment,
                        current.width,
                        &paragraph.style,
                        document,
                        font_provider,
                    );
                    fit_width = width;
                }
            }
            if segment_index + 1 == segments.len() {
                fit_width += trailing_character_spacing;
            }
            let inline_image_body_edge_tolerance = if !supports_segmented_flow
                && (line_width - content_width).abs() < 0.01
                && current
                    .runs
                    .iter()
                    .any(|run| inline_image_marker_index(&run.text).is_some())
            {
                WORD_INLINE_IMAGE_BODY_EDGE_TOLERANCE_POINTS
            } else {
                0.0
            };
            let is_final_paragraph_segment =
                run_index + 1 == paragraph.runs.len() && segment_index + 1 == segments.len();
            if is_final_paragraph_segment
                && !segment.text.ends_with(char::is_whitespace)
                && current
                    .runs
                    .iter()
                    .any(|run| inline_image_marker_index(&run.text).is_some())
            {
                let mut paragraph_mark = segment.clone();
                paragraph_mark.text = " ".to_string();
                fit_width += measure_flow_run(
                    &paragraph_mark,
                    current.width + fit_width,
                    &paragraph.style,
                    document,
                    font_provider,
                );
            }
            let boundary_fit_width = if ignore_trailing_space_for_fit
                && segment.text.ends_with(char::is_whitespace)
                && !contains_inline_marker(&segment.text)
            {
                let mut visible_segment = segment.clone();
                visible_segment.text = visible_segment
                    .text
                    .trim_end_matches(char::is_whitespace)
                    .to_string();
                measure_flow_run(
                    &visible_segment,
                    current.width,
                    &paragraph.style,
                    document,
                    font_provider,
                )
            } else {
                fit_width
            };
            if current.width > 0.0
                && current.width + boundary_fit_width
                    > line_width + inline_image_body_edge_tolerance
            {
                let carried_whitespace = if paragraph.style.no_wrap {
                    take_trailing_whitespace(&mut current, document, font_provider)
                } else {
                    Vec::new()
                };
                materialize_line_end_soft_hyphen(&mut current, document, font_provider);
                let finished_height = current.height;
                lines.push(current);
                current_line_top_y -= finished_height;
                current = empty_line();
                is_first_line = false;
                let trimmed = if paragraph.style.no_wrap {
                    segment.text.clone()
                } else {
                    segment.text.trim_start().to_string()
                };
                if trimmed.is_empty() {
                    continue;
                }
                let next_segment = FlowRun {
                    text: trimmed,
                    style: segment.style,
                    width: 0.0,
                    line_height_points: segment.line_height_points,
                    tab_leader: TabLeader::None,
                    tab_alignment: TabAlignment::Left,
                    tab_stop_position: None,
                    soft_hyphen_after: segment.soft_hyphen_after,
                };
                let mut next_intervals = line_content_intervals(current_line_top_y, current.height);
                next_intervals.retain(|(_, interval_width)| *interval_width >= 12.0);
                next_intervals.sort_by(|(left_a, _), (left_b, _)| left_a.total_cmp(left_b));
                let next_supports_segmented_flow = paragraph.style.alignment == Alignment::Left
                    && paragraph.style.left_indent_twips == 0
                    && paragraph.style.right_indent_twips == 0
                    && paragraph.style.first_line_indent_twips == 0
                    && paragraph.style.tab_stops_twips.is_empty()
                    && next_intervals.len() > 1;
                if next_supports_segmented_flow {
                    current.segmented_flow = true;
                    let mut next_width = measure_flow_run(
                        &next_segment,
                        0.0,
                        &paragraph.style,
                        document,
                        font_provider,
                    );
                    if segment_index + 1 == segments.len() {
                        next_width += trailing_character_spacing;
                    }
                    let next_left = next_intervals
                        .iter()
                        .find(|(_, interval_width)| next_width <= *interval_width + 0.01)
                        .or_else(|| next_intervals.first())
                        .map(|(left, _)| *left)
                        .unwrap_or(0.0);
                    if next_left > 0.01 {
                        push_flow_exclusion_advance(&mut current, next_left, &next_segment.style);
                    }
                }
                for whitespace in carried_whitespace {
                    push_segment(
                        &mut current,
                        whitespace,
                        &paragraph.style,
                        document,
                        font_provider,
                    );
                }
                push_segment_with_trailing_character_spacing(
                    &mut current,
                    next_segment,
                    &paragraph.style,
                    document,
                    font_provider,
                    if segment_index + 1 == segments.len() {
                        trailing_character_spacing
                    } else {
                        0.0
                    },
                );
            } else {
                push_segment_with_trailing_character_spacing(
                    &mut current,
                    segment,
                    &paragraph.style,
                    document,
                    font_provider,
                    if segment_index + 1 == segments.len() {
                        trailing_character_spacing
                    } else {
                        0.0
                    },
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

    let mut inline_image_baseline_context = false;
    for line in &mut lines {
        inline_image_baseline_context |= line
            .runs
            .iter()
            .any(|run| inline_image_marker_index(&run.text).is_some());
        line.inline_image_baseline_context = inline_image_baseline_context;
    }

    lines
}

fn character_spacing_boundary_after_run(paragraph: &Paragraph, run_index: usize) -> f32 {
    let Some(run) = paragraph.runs.get(run_index) else {
        return 0.0;
    };
    let last = run
        .text
        .chars()
        .rev()
        .find(|ch| !is_zero_width_format_char(*ch));
    if run.style.hidden
        || run.style.character_spacing_twips == 0
        || contains_inline_marker(&run.text)
        || !last.is_some_and(is_character_spacing_boundary_char)
    {
        return 0.0;
    }
    let following = paragraph.runs.iter().skip(run_index + 1).find(|following| {
        !following.style.hidden
            && following
                .text
                .chars()
                .any(|ch| !is_zero_width_format_char(ch))
    });
    let first = following.and_then(|following| {
        (!contains_inline_marker(&following.text))
            .then(|| {
                following
                    .text
                    .chars()
                    .find(|ch| !is_zero_width_format_char(*ch))
            })
            .flatten()
    });
    if !first.is_some_and(is_character_spacing_boundary_char) {
        return 0.0;
    }
    twips_to_points(run.style.character_spacing_twips)
}

fn is_character_spacing_boundary_char(ch: char) -> bool {
    !is_zero_width_format_char(ch) && !matches!(ch, '\t' | '\r' | '\n') && !ch.is_control()
}

fn apply_passive_math_operator_spacing(paragraph: &Paragraph) -> Option<Paragraph> {
    let has_spaced_operator = paragraph.runs.iter().any(|run| {
        run.style.passive_math
            && !run.style.overline
            && run.style.underline == UnderlineStyle::None
            && run.text.char_indices().any(|(index, ch)| {
                passive_math_operator_side_space_em(ch).is_some()
                    && passive_math_operator_has_operands(&run.text, index, ch.len_utf8())
            })
    });
    if !has_spaced_operator {
        return None;
    }

    let mut output = Paragraph {
        style: paragraph.style.clone(),
        runs: Vec::with_capacity(paragraph.runs.len().saturating_add(8)),
    };
    for run in &paragraph.runs {
        if !run.style.passive_math
            || run.style.overline
            || run.style.underline != UnderlineStyle::None
        {
            output.runs.push(run.clone());
            continue;
        }
        let mut start = 0usize;
        for (index, ch) in run.text.char_indices() {
            let Some(side_space_em) = passive_math_operator_side_space_em(ch) else {
                continue;
            };
            if !passive_math_operator_has_operands(&run.text, index, ch.len_utf8()) {
                continue;
            }
            if start < index {
                output.runs.push(Run {
                    text: run.text[start..index].to_string(),
                    style: run.style.clone(),
                });
            }
            let side_space = run.style.font_size_points() * side_space_em;
            push_passive_math_advance(&mut output.runs, side_space, &run.style);
            output.runs.push(Run {
                text: ch.to_string(),
                style: run.style.clone(),
            });
            push_passive_math_advance(&mut output.runs, side_space, &run.style);
            start = index + ch.len_utf8();
        }
        if start < run.text.len() {
            output.runs.push(Run {
                text: run.text[start..].to_string(),
                style: run.style.clone(),
            });
        }
    }
    Some(output)
}

fn passive_math_operator_has_operands(text: &str, index: usize, operator_len: usize) -> bool {
    text[..index].chars().any(|ch| !ch.is_whitespace())
        && text[index + operator_len..]
            .chars()
            .any(|ch| !ch.is_whitespace())
}

fn passive_math_operator_side_space_em(ch: char) -> Option<f32> {
    if matches!(
        ch,
        '=' | '<' | '>' | '\u{2260}' | '\u{2248}' | '\u{2264}' | '\u{2265}'
    ) {
        Some(5.0 / 18.0)
    } else if matches!(
        ch,
        '+' | '-' | '\u{2212}' | '\u{00b1}' | '\u{2213}' | '\u{00d7}' | '\u{00f7}'
    ) {
        Some(1.0 / 7.0)
    } else {
        None
    }
}

fn apply_passive_math_nary_base_spacing(paragraph: &Paragraph) -> Option<Paragraph> {
    if !paragraph
        .runs
        .iter()
        .any(|run| run.style.passive_math_nary_base_id.is_some())
    {
        return None;
    }

    let mut output = Paragraph {
        style: paragraph.style.clone(),
        runs: Vec::with_capacity(paragraph.runs.len().saturating_add(4)),
    };
    let mut active_base_id = None;
    for run in &paragraph.runs {
        let base_id = run.style.passive_math_nary_base_id;
        if let Some(base_id) = base_id
            && active_base_id != Some(base_id)
        {
            let operator_gap = run.style.font_size_points() * WORD_NARY_OPERATOR_GAP_EM;
            push_passive_math_advance(&mut output.runs, operator_gap, &run.style);
        }
        output.runs.push(run.clone());
        active_base_id = base_id;
    }
    Some(output)
}

fn group_word_compatible_passive_math_nary_runs(
    paragraph: &Paragraph,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> Option<Paragraph> {
    if !paragraph
        .runs
        .iter()
        .any(|run| run.style.passive_math_nary_id.is_some())
    {
        return None;
    }

    let mut output = Paragraph {
        style: paragraph.style.clone(),
        runs: Vec::with_capacity(paragraph.runs.len().saturating_add(4)),
    };
    let mut start = 0usize;
    while start < paragraph.runs.len() {
        let Some(nary_id) = paragraph.runs[start].style.passive_math_nary_id else {
            output.runs.push(paragraph.runs[start].clone());
            start += 1;
            continue;
        };
        let mut end = start + 1;
        while end < paragraph.runs.len()
            && paragraph.runs[end].style.passive_math_nary_id == Some(nary_id)
        {
            end += 1;
        }
        let nary_runs = &paragraph.runs[start..end];
        let mut nary_width =
            passive_math_model_runs_width(nary_runs, &paragraph.style, document, font_provider)
                .max(0.1);
        let spacing_run_index = nary_runs
            .iter()
            .position(|run| run.style.passive_math_nary_base_id.is_some())
            .or((nary_runs.len() > 1).then_some(1));
        if let Some(spacing_run_index) = spacing_run_index {
            nary_width +=
                nary_runs[spacing_run_index].style.font_size_points() * WORD_NARY_OPERATOR_GAP_EM;
        }
        let mut marker_style = nary_runs[0].style.clone();
        marker_style.passive_math_nary_id = None;
        let mut lead_style = marker_style.clone();
        lead_style.character_spacing_twips =
            points_to_bounded_twips(WORD_INLINE_OFFICE_MATH_LEAD_IN_POINTS);
        output.runs.push(Run {
            text: PASSIVE_MATH_INLINE_LEAD_MARKER.to_string(),
            style: lead_style,
        });
        marker_style.character_spacing_twips = points_to_bounded_twips(nary_width);
        output.runs.push(Run {
            text: PASSIVE_MATH_STACK_ANCHOR_MARKER.to_string(),
            style: marker_style,
        });
        for (index, run) in nary_runs.iter().enumerate() {
            let mut run = run.clone();
            if Some(index) == spacing_run_index && run.style.passive_math_nary_base_id.is_none() {
                run.style.passive_math_nary_base_id = Some(nary_id);
            }
            output.runs.push(run);
        }
        start = end;
    }
    Some(output)
}

fn stack_passive_math_limits(
    paragraph: &Paragraph,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> Option<Paragraph> {
    if !paragraph
        .runs
        .iter()
        .any(|run| run.style.passive_math_limit_id.is_some())
    {
        return None;
    }

    let mut output = Paragraph {
        style: paragraph.style.clone(),
        runs: Vec::with_capacity(paragraph.runs.len().saturating_add(6)),
    };
    let mut start = 0usize;
    while start < paragraph.runs.len() {
        let Some(limit_id) = paragraph.runs[start].style.passive_math_limit_id else {
            output.runs.push(paragraph.runs[start].clone());
            start += 1;
            continue;
        };
        let mut end = start + 1;
        while end < paragraph.runs.len()
            && paragraph.runs[end].style.passive_math_limit_id == Some(limit_id)
        {
            end += 1;
        }
        let limit_runs = &paragraph.runs[start..end];
        let base = limit_runs
            .iter()
            .filter(|run| run.style.passive_math_limit_part == PassiveMathLimitPart::Base)
            .cloned()
            .collect::<Vec<_>>();
        let script = limit_runs
            .iter()
            .filter(|run| {
                matches!(
                    run.style.passive_math_limit_part,
                    PassiveMathLimitPart::Lower | PassiveMathLimitPart::Upper
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        if base.is_empty() || script.is_empty() {
            output.runs.extend_from_slice(limit_runs);
            start = end;
            continue;
        }

        let base_width =
            passive_math_model_runs_width(&base, &paragraph.style, document, font_provider);
        let script_width =
            passive_math_model_runs_width(&script, &paragraph.style, document, font_provider);
        let stack_width = base_width.max(script_width).max(0.1);
        let mut marker_style = limit_runs[0].style.clone();
        marker_style.passive_math_limit_part = PassiveMathLimitPart::None;
        marker_style.baseline_shift_half_points = 0;
        marker_style.font_size_scale_percent = 100;
        marker_style.character_spacing_twips = points_to_bounded_twips(stack_width);
        output.runs.push(Run {
            text: PASSIVE_MATH_STACK_ANCHOR_MARKER.to_string(),
            style: marker_style.clone(),
        });

        let base_start = (stack_width - base_width) / 2.0;
        push_passive_math_advance(&mut output.runs, base_start, &marker_style);
        output.runs.extend(base);
        let script_start = (stack_width - script_width) / 2.0;
        push_passive_math_advance(
            &mut output.runs,
            script_start - base_start - base_width,
            &marker_style,
        );
        output.runs.extend(script);
        push_passive_math_advance(
            &mut output.runs,
            stack_width - script_start - script_width,
            &marker_style,
        );
        start = end;
    }
    Some(output)
}

fn stack_passive_math_fractions(
    paragraph: &Paragraph,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> Option<Paragraph> {
    if !paragraph
        .runs
        .iter()
        .any(|run| run.style.passive_math_fraction_id.is_some())
    {
        return None;
    }

    let mut output = Paragraph {
        style: paragraph.style.clone(),
        runs: Vec::with_capacity(paragraph.runs.len().saturating_add(6)),
    };
    let mut start = 0usize;
    while start < paragraph.runs.len() {
        let Some(fraction_id) = paragraph.runs[start].style.passive_math_fraction_id else {
            output.runs.push(paragraph.runs[start].clone());
            start += 1;
            continue;
        };
        let mut end = start + 1;
        while end < paragraph.runs.len()
            && paragraph.runs[end].style.passive_math_fraction_id == Some(fraction_id)
        {
            end += 1;
        }
        let fraction_runs = &paragraph.runs[start..end];
        let numerator = fraction_runs
            .iter()
            .filter(|run| {
                run.style.passive_math_fraction_part == PassiveMathFractionPart::Numerator
            })
            .cloned()
            .collect::<Vec<_>>();
        let denominator = fraction_runs
            .iter()
            .filter(|run| {
                run.style.passive_math_fraction_part == PassiveMathFractionPart::Denominator
            })
            .cloned()
            .collect::<Vec<_>>();
        if numerator.is_empty() || denominator.is_empty() {
            output.runs.extend_from_slice(fraction_runs);
            start = end;
            continue;
        }

        let numerator_width =
            passive_math_model_runs_width(&numerator, &paragraph.style, document, font_provider);
        let denominator_width =
            passive_math_model_runs_width(&denominator, &paragraph.style, document, font_provider);
        let fraction_width = numerator_width.max(denominator_width).max(0.1);
        let mut marker_style = fraction_runs[0].style.clone();
        marker_style.passive_math_fraction_part = PassiveMathFractionPart::None;
        marker_style.baseline_shift_half_points = 0;
        marker_style.font_size_scale_percent = 100;
        let mut lead_style = marker_style.clone();
        lead_style.character_spacing_twips =
            points_to_bounded_twips(WORD_INLINE_OFFICE_MATH_LEAD_IN_POINTS);
        output.runs.push(Run {
            text: PASSIVE_MATH_INLINE_LEAD_MARKER.to_string(),
            style: lead_style,
        });
        marker_style.character_spacing_twips = points_to_bounded_twips(fraction_width);
        output.runs.push(Run {
            text: PASSIVE_MATH_FRACTION_RULE_MARKER.to_string(),
            style: marker_style.clone(),
        });

        let numerator_start = (fraction_width - numerator_width) / 2.0;
        push_passive_math_advance(&mut output.runs, numerator_start, &marker_style);
        output.runs.extend(numerator);
        let denominator_start = (fraction_width - denominator_width) / 2.0;
        push_passive_math_advance(
            &mut output.runs,
            denominator_start - numerator_start - numerator_width,
            &marker_style,
        );
        output.runs.extend(denominator);
        push_passive_math_advance(
            &mut output.runs,
            fraction_width - denominator_start - denominator_width,
            &marker_style,
        );
        start = end;
    }
    Some(output)
}

fn stack_passive_math_equation_arrays(
    paragraph: &Paragraph,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> Option<Paragraph> {
    if !paragraph
        .runs
        .iter()
        .any(|run| run.style.passive_math_equation_array_id.is_some())
    {
        return None;
    }

    let mut output = Paragraph {
        style: paragraph.style.clone(),
        runs: Vec::with_capacity(paragraph.runs.len().saturating_add(8)),
    };
    let mut start = 0usize;
    while start < paragraph.runs.len() {
        let Some(array_id) = paragraph.runs[start].style.passive_math_equation_array_id else {
            output.runs.push(paragraph.runs[start].clone());
            start += 1;
            continue;
        };
        let mut end = start + 1;
        while end < paragraph.runs.len()
            && paragraph.runs[end].style.passive_math_equation_array_id == Some(array_id)
        {
            end += 1;
        }
        let array_runs = &paragraph.runs[start..end];
        let row_count = array_runs
            .iter()
            .map(|run| run.style.passive_math_equation_array_row)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .min(array_runs.len().saturating_add(1));
        let mut rows = vec![Vec::new(); row_count];
        for run in array_runs {
            let row_index = run
                .style
                .passive_math_equation_array_row
                .min(row_count.saturating_sub(1));
            let mut row_run = run.clone();
            row_run.text.retain(|ch| ch != '\n' && ch != '\r');
            if !row_run.text.is_empty() {
                rows[row_index].push(row_run);
            }
        }
        if rows.iter().any(Vec::is_empty) {
            output.runs.extend_from_slice(array_runs);
            start = end;
            continue;
        }

        let row_widths = rows
            .iter()
            .map(|row| {
                passive_math_model_runs_width(row, &paragraph.style, document, font_provider)
            })
            .collect::<Vec<_>>();
        let array_width = row_widths.iter().copied().fold(0.1_f32, f32::max);
        let mut marker_style = array_runs[0].style.clone();
        marker_style.passive_math_equation_array_id = None;
        marker_style.passive_math_equation_array_row = 0;
        marker_style.baseline_shift_half_points = 0;
        marker_style.font_size_scale_percent = 100;
        marker_style.character_spacing_twips = points_to_bounded_twips(array_width);
        output.runs.push(Run {
            text: PASSIVE_MATH_STACK_ANCHOR_MARKER.to_string(),
            style: marker_style.clone(),
        });

        let mut cursor = 0.0_f32;
        let row_count_i64 = i64::try_from(row_count).unwrap_or(i64::MAX);
        for (row_index, (row, row_width)) in
            rows.into_iter().zip(row_widths.into_iter()).enumerate()
        {
            let row_start = (array_width - row_width) / 2.0;
            push_passive_math_advance(&mut output.runs, row_start - cursor, &marker_style);
            cursor = row_start;
            let row_index_i64 = i64::try_from(row_index).unwrap_or(i64::MAX);
            let centered_index = row_count_i64
                .saturating_sub(1)
                .saturating_sub(row_index_i64.saturating_mul(2));
            let row_shift_half_points = centered_index
                .saturating_mul(i64::from(
                    OFFICE_MATH_EQUATION_ARRAY_HALF_ROW_PITCH_HALF_POINTS,
                ))
                .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                as i32;
            for mut run in row {
                if !is_passive_advance_marker(&run.text)
                    && !is_passive_math_inline_lead_marker(&run.text)
                    && !is_passive_math_stack_anchor_marker(&run.text)
                {
                    run.style.baseline_shift_half_points = run
                        .style
                        .baseline_shift_half_points
                        .saturating_add(row_shift_half_points);
                }
                run.style.passive_math_equation_array_id = None;
                run.style.passive_math_equation_array_row = 0;
                output.runs.push(run);
            }
            cursor += row_width;
        }
        push_passive_math_advance(&mut output.runs, array_width - cursor, &marker_style);
        start = end;
    }
    Some(output)
}

fn passive_math_model_runs_width(
    runs: &[Run],
    paragraph_style: &ParagraphStyle,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> f32 {
    runs.iter()
        .map(|run| {
            let flow = FlowRun {
                text: run.text.clone(),
                style: run.style.clone(),
                width: 0.0,
                line_height_points: run.style.font_size_points(),
                tab_leader: TabLeader::None,
                tab_alignment: TabAlignment::Left,
                tab_stop_position: None,
                soft_hyphen_after: false,
            };
            measure_flow_run(&flow, 0.0, paragraph_style, document, font_provider)
        })
        .sum()
}

fn push_passive_math_advance(runs: &mut Vec<Run>, width: f32, base_style: &CharacterStyle) {
    if width.abs() < 0.01 {
        return;
    }
    let mut style = base_style.clone();
    style.baseline_shift_half_points = 0;
    style.character_spacing_twips = points_to_bounded_twips(width);
    runs.push(Run {
        text: PASSIVE_ADVANCE_MARKER.to_string(),
        style,
    });
}

fn points_to_bounded_twips(points: f32) -> i32 {
    (points * 20.0)
        .round()
        .clamp(i32::MIN as f32, i32::MAX as f32) as i32
}

fn apply_fit_text_scaling_to_run_groups(
    paragraph: &Paragraph,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> Paragraph {
    let mut paragraph = paragraph.clone();
    let mut group_start = 0usize;
    while group_start < paragraph.runs.len() {
        let Some(target_twips) = paragraph.runs[group_start].style.fit_text_width_twips else {
            group_start += 1;
            continue;
        };
        if target_twips <= 0 {
            paragraph.runs[group_start].style.fit_text_width_twips = None;
            group_start += 1;
            continue;
        }

        let mut group_end = group_start + 1;
        while group_end < paragraph.runs.len()
            && paragraph.runs[group_end].style.fit_text_width_twips == Some(target_twips)
        {
            group_end += 1;
        }

        let mut measured_width = 0.0;
        for run in &paragraph.runs[group_start..group_end] {
            let mut style = run.style.clone();
            style.fit_text_width_twips = None;
            let flow = FlowRun {
                text: run.text.clone(),
                line_height_points: style.font_size_points(),
                style,
                width: 0.0,
                tab_leader: TabLeader::None,
                tab_alignment: TabAlignment::Left,
                tab_stop_position: None,
                soft_hyphen_after: false,
            };
            measured_width += measure_flow_run(
                &flow,
                measured_width,
                &paragraph.style,
                document,
                font_provider,
            );
        }

        if measured_width > 0.0 {
            let target_width = twips_to_points(target_twips).max(0.1);
            let group_scale = target_width / measured_width;
            for run in &mut paragraph.runs[group_start..group_end] {
                run.style.character_scaling_percent = (run.style.character_scaling_percent as f32
                    * group_scale)
                    .round()
                    .clamp(1.0, 600.0) as i32;
                run.style.fit_text_width_twips = None;
            }
        }

        group_start = group_end;
    }
    paragraph
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

fn apply_emergency_word_wrapping(
    segments: Vec<FlowRun>,
    content_width: f32,
    paragraph_style: &ParagraphStyle,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> Vec<FlowRun> {
    let max_line_width = paragraph_line_width(content_width, paragraph_style, false)
        .max(paragraph_line_width(content_width, paragraph_style, true));
    let mut output = Vec::new();

    for segment in segments {
        if segment.tab_stop_position.is_some()
            || segment.text == "\t"
            || segment.text == "\n"
            || contains_inline_marker(&segment.text)
            || segment.text.chars().count() < 2
            || segment.text.chars().any(char::is_whitespace)
        {
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
        ) <= max_line_width
        {
            output.push(segment);
            continue;
        }

        push_emergency_wrapped_segment(
            &mut output,
            segment,
            max_line_width,
            family,
            document,
            font_provider,
        );
    }

    output
}

fn apply_no_wrap_emergency_boundaries(
    segments: Vec<FlowRun>,
    content_width: f32,
    paragraph_style: &ParagraphStyle,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> Vec<FlowRun> {
    if !segments.iter().any(|segment| segment.text == "\n") {
        let mut measured_width = 0.0;
        for segment in &segments {
            measured_width += measure_flow_run(
                segment,
                measured_width,
                paragraph_style,
                document,
                font_provider,
            );
        }
        let max_line_width = paragraph_line_width(content_width, paragraph_style, false)
            .max(paragraph_line_width(content_width, paragraph_style, true));
        if measured_width <= max_line_width + 0.01 {
            return segments;
        }
    }

    let mut output = Vec::new();
    for segment in segments {
        if segment.style.fit_text_width_twips.is_some()
            || segment.tab_stop_position.is_some()
            || segment.text == "\t"
            || segment.text == "\n"
            || contains_inline_marker(&segment.text)
            || segment.text.chars().count() < 2
        {
            output.push(segment);
            continue;
        }

        let chars = segment.text.chars().collect::<Vec<_>>();
        let last = chars.len() - 1;
        for (index, ch) in chars.into_iter().enumerate() {
            let mut character = segment.clone();
            character.text = ch.to_string();
            character.width = 0.0;
            character.soft_hyphen_after = index == last && segment.soft_hyphen_after;
            output.push(character);
        }
    }
    output
}

fn take_trailing_whitespace(
    line: &mut Line,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> Vec<FlowRun> {
    let mut carried = Vec::new();
    while line.runs.last().is_some_and(|run| {
        run.tab_stop_position.is_none()
            && run.text != "\t"
            && run.text.chars().all(char::is_whitespace)
    }) {
        let run = line.runs.pop().expect("trailing run exists");
        line.width = (line.width - run.width).max(0.0);
        carried.push(run);
    }
    carried.reverse();
    line.natural_height = natural_line_height_for_runs(&line.runs, document, font_provider);
    line.height = line.natural_height;
    carried
}

fn natural_line_height_for_runs(
    runs: &[FlowRun],
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> f32 {
    let has_passive_checkbox = runs
        .iter()
        .any(|run| run.text.chars().any(is_passive_checkbox_char));
    if has_passive_checkbox {
        return runs
            .iter()
            .filter(|run| {
                inline_image_marker_index(&run.text).is_none()
                    && !is_passive_advance_marker(&run.text)
                    && !is_passive_math_inline_lead_marker(&run.text)
            })
            .map(|run| {
                if is_passive_math_fraction_rule_marker(&run.text) {
                    run.style.font_size_points() * PASSIVE_MATH_FRACTION_LINE_HEIGHT_MULTIPLIER
                } else {
                    (run.line_height_points * 1.15)
                        + run.style.baseline_shift_points().abs()
                        + (character_border_inset(&run.style) * 2.0)
                }
            })
            .filter(|height| *height > 0.0)
            .reduce(f32::max)
            .unwrap_or(14.0);
    }
    runs.iter()
        .map(|run| flow_run_line_height(run, document, font_provider))
        .filter(|height| *height > 0.0)
        .reduce(f32::max)
        .unwrap_or(14.0)
}

fn push_emergency_wrapped_segment(
    output: &mut Vec<FlowRun>,
    segment: FlowRun,
    max_line_width: f32,
    family: PdfFontFamily,
    document: &Document,
    font_provider: Option<&FontProvider>,
) {
    let chars = segment.text.chars().collect::<Vec<_>>();
    let mut start = 0;
    while start < chars.len() {
        let mut end = chars.len();
        while end > start + 1 {
            let text = chars[start..end].iter().collect::<String>();
            if measure_text_with_document_font(
                &text,
                &segment.style,
                family,
                document,
                font_provider,
            ) <= max_line_width
            {
                break;
            }
            end -= 1;
        }
        let is_last = end == chars.len();
        push_flow_run(
            output,
            &chars[start..end].iter().collect::<String>(),
            &segment.style,
            is_last && segment.soft_hyphen_after,
        );
        start = end;
    }
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
    _max_consecutive_hyphenated: Option<usize>,
    consecutive_hyphenated: &mut usize,
) {
    // Without a dictionary, Word's visible recovery is the same bounded
    // character-boundary emergency wrap used for an ordinary oversized word.
    // Do not reserve an artificial three-character suffix: Word may leave one
    // final character when that is the widest fitting split.
    push_emergency_wrapped_segment(
        output,
        segment,
        max_line_width,
        family,
        document,
        font_provider,
    );
    *consecutive_hyphenated = 0;
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
            .replace(NESTED_TABLE_ANCHOR_MARKER, "")
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

    // Word treats a character-bordered run as one inline box. Keeping its
    // authored text together here prevents the ordinary whitespace splitter
    // from drawing a separate border around every word. Oversized boxes still
    // pass through the bounded emergency-splitting path during wrapping.
    if run.style.border.visible
        && !contains_inline_marker(text)
        && !text.contains(['\t', '\n', '\r'])
    {
        push_display_text_segment(&mut output, text, &run.style, false);
        return output;
    }

    for (idx, opportunity) in linebreaks(text) {
        if matches!(
            opportunity,
            BreakOpportunity::Allowed | BreakOpportunity::Mandatory
        ) {
            let mandatory = matches!(opportunity, BreakOpportunity::Mandatory);
            if run.style.form_field_shading && !mandatory {
                continue;
            }
            let raw_segment_text = &text[start..idx];
            let segment_text = if mandatory {
                raw_segment_text.trim_end_matches(|ch| {
                    matches!(ch, '\r' | '\n' | '\u{0085}' | '\u{2028}' | '\u{2029}')
                })
            } else {
                raw_segment_text
            };
            let has_break_terminator = segment_text.len() != raw_segment_text.len();
            if !segment_text.is_empty() {
                let soft_hyphen_after = segment_text.ends_with('\u{00ad}');
                push_text_segments_preserving_tabs(
                    &mut output,
                    segment_text,
                    &run.style,
                    soft_hyphen_after,
                );
            }
            if mandatory && (idx < text.len() || has_break_terminator) {
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
        || text.contains(FOOTNOTE_REFERENCE_MARKER)
        || text.contains(FOOTNOTE_REFERENCE_MARKER_END)
        || text.contains(ENDNOTE_REFERENCE_MARKER)
        || text.contains(ENDNOTE_REFERENCE_MARKER_END)
        || text.contains(PASSIVE_ADVANCE_MARKER)
        || text.contains(PASSIVE_MATH_FRACTION_RULE_MARKER)
        || text.contains(PASSIVE_MATH_INLINE_LEAD_MARKER)
        || text.contains(PASSIVE_MATH_STACK_ANCHOR_MARKER)
        || text.contains(NESTED_TABLE_ANCHOR_MARKER)
}

#[derive(Debug, Default)]
struct DocumentStatsBuilder {
    stats: DocumentStats,
    in_word: bool,
    counted_repeating_paragraphs: Vec<(usize, usize)>,
    counted_repeating_images: Vec<(usize, usize)>,
    counted_repeating_shapes: Vec<(usize, usize)>,
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

fn document_stats(document: &Document) -> DocumentStats {
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

fn push_block_stats(
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
struct PageVariantVisibility {
    first_page: usize,
    last_page: usize,
}

fn estimate_document_page_variant_visibility(document: &Document) -> PageVariantVisibility {
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

fn estimate_section_page_variant_visibility(
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

fn physical_page_before_block(document: &Document, block_idx: usize) -> usize {
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

fn page_settings_before_block(document: &Document, block_idx: usize) -> &PageSettings {
    let mut settings = &document.page;
    for block in document.blocks.iter().take(block_idx) {
        if let Block::SectionSettings(section_settings) = block {
            settings = section_settings;
        }
    }
    settings
}

fn content_width_for_page_settings(settings: &PageSettings) -> f32 {
    PageGeometry::from_settings(
        settings,
        PageNumbering::from_page_settings(settings, 1),
        1,
        0,
    )
    .content_width
}

fn advance_physical_page_for_variant_estimate(
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

fn section_breaks_end_current_section(block: &Block) -> bool {
    matches!(
        block,
        Block::ContinuousSectionBreak
            | Block::SectionBreak
            | Block::EvenPageSectionBreak
            | Block::OddPageSectionBreak
    )
}

fn push_repeating_paragraph_variant_stats(
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

fn push_repeating_shape_variant_stats(
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

fn push_repeating_image_variant_stats(
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

fn push_background_shape_stats(
    builder: &mut DocumentStatsBuilder,
    shapes: &[StaticShape],
    fallback_shapes: &[StaticShape],
) {
    builder.push_repeating_shapes_once(first_non_empty_shapes(shapes, fallback_shapes));
}

fn push_paragraphs_stats(builder: &mut DocumentStatsBuilder, paragraphs: &[Paragraph]) {
    for paragraph in paragraphs {
        push_paragraph_stats(builder, paragraph);
    }
}

fn push_shapes_stats(builder: &mut DocumentStatsBuilder, shapes: &[StaticShape]) {
    for shape in shapes {
        push_shape_stats(builder, shape);
    }
}

fn push_images_stats(
    builder: &mut DocumentStatsBuilder,
    images: &[StaticImage],
    content_width: f32,
) {
    for image in images {
        push_image_stats(builder, image, content_width);
    }
}

fn push_image_stats(builder: &mut DocumentStatsBuilder, image: &StaticImage, content_width: f32) {
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

fn image_placeholder_label_is_visible(image: &StaticImage, content_width: f32) -> bool {
    if image.format != ImageFormat::Placeholder {
        return false;
    }
    let (_, _, width, height) = image_layout_frame(image, content_width);
    width >= 74.0 && height >= 24.0
}

fn push_shape_stats(builder: &mut DocumentStatsBuilder, shape: &StaticShape) {
    for paragraph in &shape.text {
        push_paragraph_stats(builder, paragraph);
    }
    builder.finish_text_boundary();
}

fn table_cell_is_merge_continuation(cell: &TableCell) -> bool {
    cell.horizontal_merge == TableCellHorizontalMerge::Continuation
        || cell.vertical_merge == TableCellVerticalMerge::Continuation
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
        .replace(DOCUMENT_CHARS_WITH_SPACES_MARKER, "")
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

fn resolve_footnote_reference_markers(
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

fn resolve_endnote_reference_markers(
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

fn apply_late_inline_marker_shifts(page: &mut LayoutPage, shifts: Vec<(usize, f32, f32, f32)>) {
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

fn footnote_reference_pages(pages: &[LayoutPage], footnote_count: usize) -> Vec<Option<usize>> {
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
                    && let Some(reference_page) = reference_pages.get_mut(index)
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

fn replace_endnote_reference_markers_in_text(
    text: &str,
    document: &Document,
    rendered_endnote_pages: &[Option<usize>],
) -> String {
    let mut output = String::new();
    let mut search_start = 0;
    while let Some((start, end)) = next_endnote_reference_marker_range(text, search_start) {
        output.push_str(&text[search_start..start]);
        let replacement = parse_endnote_reference_marker_id(&text[start..end])
            .map(|index| {
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

fn replace_footnote_reference_markers_in_text(
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
            .map(|index| {
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

fn footnote_sequence_for_reference_page(
    document: &Document,
    index: usize,
    reference_pages: &[Option<usize>],
    current_page: usize,
) -> usize {
    match footnote_numbering_for(document, index).restart {
        NoteNumberRestart::Continuous => index.saturating_add(1),
        NoteNumberRestart::EachSection => note_sequence_for_index(
            NoteNumberRestart::EachSection,
            &document.footnote_section_indices,
            index,
        ),
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
        let Some(fragment) = layout_item_text_fragment_mut(item) else {
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
    let fragment = layout_item_text_fragment(page.items.get(adjustment.text_index)?)?;
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

fn same_late_marker_line(baseline_y: f32, geometry: LateMarkerGeometry) -> bool {
    (baseline_y - geometry.baseline_y).abs() < 0.01
}

fn starts_new_text_line(item: &LayoutItem, geometry: LateMarkerGeometry) -> bool {
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

fn shift_layout_item_x(item: &mut LayoutItem, delta: f32) {
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

fn layout_item_text_fragment(item: &LayoutItem) -> Option<&TextFragment> {
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

fn layout_item_text_fragment_mut(item: &mut LayoutItem) -> Option<&mut TextFragment> {
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

fn next_footnote_reference_marker_range(text: &str, start: usize) -> Option<(usize, usize)> {
    let marker_start = text[start..]
        .find(FOOTNOTE_REFERENCE_MARKER)
        .map(|relative| start + relative)?;
    let id_start = marker_start + FOOTNOTE_REFERENCE_MARKER.len();
    let marker_end_relative = text[id_start..].find(FOOTNOTE_REFERENCE_MARKER_END)?;
    let marker_end = id_start + marker_end_relative + FOOTNOTE_REFERENCE_MARKER_END.len();
    parse_footnote_reference_marker_id(&text[marker_start..marker_end])?;
    Some((marker_start, marker_end))
}

fn next_endnote_reference_marker_range(text: &str, start: usize) -> Option<(usize, usize)> {
    let marker_start = text[start..]
        .find(ENDNOTE_REFERENCE_MARKER)
        .map(|relative| start + relative)?;
    let id_start = marker_start + ENDNOTE_REFERENCE_MARKER.len();
    let marker_end_relative = text[id_start..].find(ENDNOTE_REFERENCE_MARKER_END)?;
    let marker_end = id_start + marker_end_relative + ENDNOTE_REFERENCE_MARKER_END.len();
    parse_endnote_reference_marker_id(&text[marker_start..marker_end])?;
    Some((marker_start, marker_end))
}

fn parse_bookmark_page_marker_id(text: &str, prefix: &str) -> Option<usize> {
    let rest = text.strip_prefix(prefix)?;
    let id = rest.strip_suffix(BOOKMARK_PAGE_MARKER_END)?;
    (!id.is_empty() && id.chars().all(|ch| ch.is_ascii_digit()))
        .then(|| id.parse::<usize>().ok())?
}

fn parse_footnote_reference_marker_id(text: &str) -> Option<usize> {
    let rest = text.strip_prefix(FOOTNOTE_REFERENCE_MARKER)?;
    let id = rest.strip_suffix(FOOTNOTE_REFERENCE_MARKER_END)?;
    (!id.is_empty() && id.chars().all(|ch| ch.is_ascii_digit()))
        .then(|| id.parse::<usize>().ok())?
}

fn parse_endnote_reference_marker_id(text: &str) -> Option<usize> {
    let rest = text.strip_prefix(ENDNOTE_REFERENCE_MARKER)?;
    let id = rest.strip_suffix(ENDNOTE_REFERENCE_MARKER_END)?;
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
        let is_passive_advance = ch.to_string() == PASSIVE_ADVANCE_MARKER;
        let is_math_fraction_rule = ch.to_string() == PASSIVE_MATH_FRACTION_RULE_MARKER;
        let is_math_inline_lead = ch.to_string() == PASSIVE_MATH_INLINE_LEAD_MARKER;
        let is_math_stack_anchor = ch.to_string() == PASSIVE_MATH_STACK_ANCHOR_MARKER;
        let inline_image_index = inline_image_marker_index(&ch.to_string());
        if ch == '\t'
            || is_passive_advance
            || is_math_fraction_rule
            || is_math_inline_lead
            || is_math_stack_anchor
            || inline_image_index.is_some()
        {
            if start < idx {
                let text = text[start..idx].replace('\u{00ad}', "");
                if !text.is_empty() {
                    push_display_text_segment(output, &text, style, false);
                }
            }
            if is_passive_advance {
                output.push(FlowRun {
                    text: PASSIVE_ADVANCE_MARKER.to_string(),
                    style: style.clone(),
                    width: passive_advance_width_points(style),
                    line_height_points: 0.0,
                    tab_leader: TabLeader::None,
                    tab_alignment: TabAlignment::Left,
                    tab_stop_position: None,
                    soft_hyphen_after: false,
                });
                start = idx + ch.len_utf8();
                continue;
            }
            if is_math_inline_lead {
                output.push(FlowRun {
                    text: PASSIVE_MATH_INLINE_LEAD_MARKER.to_string(),
                    style: style.clone(),
                    width: passive_advance_width_points(style),
                    line_height_points: 0.0,
                    tab_leader: TabLeader::None,
                    tab_alignment: TabAlignment::Left,
                    tab_stop_position: None,
                    soft_hyphen_after: false,
                });
                start = idx + ch.len_utf8();
                continue;
            }
            if is_math_fraction_rule {
                output.push(FlowRun {
                    text: PASSIVE_MATH_FRACTION_RULE_MARKER.to_string(),
                    style: style.clone(),
                    width: 0.0,
                    line_height_points: 0.0,
                    tab_leader: TabLeader::None,
                    tab_alignment: TabAlignment::Left,
                    tab_stop_position: None,
                    soft_hyphen_after: false,
                });
                start = idx + ch.len_utf8();
                continue;
            }
            if is_math_stack_anchor {
                output.push(FlowRun {
                    text: PASSIVE_MATH_STACK_ANCHOR_MARKER.to_string(),
                    style: style.clone(),
                    width: 0.0,
                    line_height_points: 0.0,
                    tab_leader: TabLeader::None,
                    tab_alignment: TabAlignment::Left,
                    tab_stop_position: None,
                    soft_hyphen_after: false,
                });
                start = idx + ch.len_utf8();
                continue;
            }
            if inline_image_index.is_some() {
                output.push(FlowRun {
                    text: ch.to_string(),
                    style: style.clone(),
                    width: 0.0,
                    line_height_points: 0.0,
                    tab_leader: TabLeader::None,
                    tab_alignment: TabAlignment::Left,
                    tab_stop_position: None,
                    soft_hyphen_after: false,
                });
                start = idx + ch.len_utf8();
                continue;
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
        natural_height: 14.0,
        height: 14.0,
        segmented_flow: false,
        inline_image_baseline_context: false,
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
    } else {
        apply_fit_text_scaling_to_segment(
            &mut segment,
            line.width,
            paragraph_style,
            document,
            font_provider,
        );
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
    if let Some(image) = inline_image_for_marker(document, &segment.text) {
        let (_, image_height) = image_display_size(image, f32::MAX);
        line.height = line.height.max(image_height);
        line.runs.push(segment);
        return;
    }
    let segment_height = flow_run_line_height(&segment, document, font_provider);
    let has_visible_height = line
        .runs
        .iter()
        .any(|run| flow_run_line_height(run, document, font_provider) > 0.0);
    if !has_visible_height && segment_height > 0.0 {
        line.natural_height = segment_height;
        line.height = segment_height;
    } else {
        line.natural_height = line.natural_height.max(segment_height);
        line.height = line.height.max(segment_height);
    }
    line.runs.push(segment);
}

fn push_segment_with_trailing_character_spacing(
    line: &mut Line,
    segment: FlowRun,
    paragraph_style: &ParagraphStyle,
    document: &Document,
    font_provider: Option<&FontProvider>,
    trailing_character_spacing: f32,
) {
    push_segment(line, segment, paragraph_style, document, font_provider);
    if trailing_character_spacing == 0.0 {
        return;
    }
    if let Some(last) = line.runs.last_mut() {
        last.width += trailing_character_spacing;
        line.width += trailing_character_spacing;
    }
}

fn push_flow_exclusion_advance(line: &mut Line, width: f32, style: &CharacterStyle) {
    if width <= 0.01 {
        return;
    }
    line.runs.push(FlowRun {
        text: String::new(),
        style: style.clone(),
        width,
        line_height_points: 0.0,
        tab_leader: TabLeader::None,
        tab_alignment: TabAlignment::Left,
        tab_stop_position: None,
        soft_hyphen_after: false,
    });
    line.width += width;
}

fn apply_fit_text_scaling_to_segment(
    segment: &mut FlowRun,
    current_line_width: f32,
    paragraph_style: &ParagraphStyle,
    document: &Document,
    font_provider: Option<&FontProvider>,
) {
    let Some(target_twips) = segment.style.fit_text_width_twips else {
        return;
    };
    if target_twips <= 0
        || segment.text.is_empty()
        || is_passive_advance_marker(&segment.text)
        || is_passive_math_inline_lead_marker(&segment.text)
        || is_passive_math_fraction_rule_marker(&segment.text)
        || is_passive_math_stack_anchor_marker(&segment.text)
        || parse_bookmark_page_marker_id(&segment.text, BOOKMARK_PAGE_ANCHOR_MARKER).is_some()
        || parse_bookmark_page_marker_id(&segment.text, BOOKMARK_PAGE_REF_MARKER).is_some()
    {
        return;
    }

    let mut measured_segment = segment.clone();
    measured_segment.style.fit_text_width_twips = None;
    let measured_width = measure_flow_run(
        &measured_segment,
        current_line_width,
        paragraph_style,
        document,
        font_provider,
    );
    if measured_width <= 0.0 {
        return;
    }

    let target_width = twips_to_points(target_twips).max(0.1);
    let scaled = (segment.style.character_scaling_percent as f32 * target_width / measured_width)
        .round()
        .clamp(1.0, 600.0);
    segment.style.character_scaling_percent = scaled as i32;
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
    let run_height = flow_run_line_height(last, document, font_provider);
    line.natural_height = line.natural_height.max(run_height);
    line.height = line.height.max(run_height);
}

fn flow_run_line_height(
    run: &FlowRun,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> f32 {
    if run.text.is_empty() {
        return 0.0;
    }
    if inline_image_marker_index(&run.text).is_some() {
        return 0.0;
    }
    if is_passive_advance_marker(&run.text) {
        return 0.0;
    }
    if is_passive_math_inline_lead_marker(&run.text) {
        return 0.0;
    }
    if is_passive_math_fraction_rule_marker(&run.text) {
        return (run.style.font_size_points() * PASSIVE_MATH_FRACTION_LINE_HEIGHT_MULTIPLIER)
            + run.style.baseline_shift_points().abs();
    }
    if is_passive_math_stack_anchor_marker(&run.text) {
        return 0.0;
    }
    let border_extra = character_border_inset(&run.style) * 2.0;
    if run.text.chars().any(is_passive_checkbox_char) {
        return (run.line_height_points * 1.15)
            + run.style.baseline_shift_points().abs()
            + border_extra;
    }
    let fallback = fallback_flow_run_line_height(run) + border_extra;
    let passive_base14_fallback =
        || passive_base14_fallback_flow_run_line_height(run, document) + border_extra;
    if let Some(line_height) = source_word_compatible_line_height_points(
        font_for_style(document, &run.style),
        run.line_height_points,
        font_provider,
    ) {
        return line_height.clamp(
            run.line_height_points.max(1.0),
            run.line_height_points.max(1.0) * 2.0,
        ) + run.style.baseline_shift_points().abs()
            + border_extra;
    }
    let Some(provider) = font_provider else {
        return fallback;
    };
    let Some(font) = font_for_style(document, &run.style) else {
        return passive_base14_fallback();
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
        return passive_base14_fallback();
    };
    let bounded_line_height = line_height.clamp(
        run.line_height_points.max(1.0),
        run.line_height_points.max(1.0) * 2.0,
    );
    bounded_line_height + run.style.baseline_shift_points().abs() + border_extra
}

fn source_word_compatible_line_height_points(
    font: Option<&FontDef>,
    size: f32,
    font_provider: Option<&FontProvider>,
) -> Option<f32> {
    let font = font?;
    let units = if font_uses_source_family(font, font_provider, &["calibri", "carlito", "aptos"]) {
        // Calibri/Carlito: 1536 - -512 + 452. Aptos: 1923 - -577.
        2_500.0
    } else if font_uses_source_family(font, font_provider, &["cambria math"]) {
        // Word's registered inline Office Math references use a 14.04-point
        // line box at 12 points: 2396 / 2048 em after PDF-unit rounding.
        2_396.0
    } else if font_uses_source_family(font, font_provider, &["cambria"]) {
        // Cambria: 1946 - -455.
        2_401.0
    } else {
        return None;
    };
    Some(size * (units / 2_048.0))
}

fn fallback_flow_run_line_height(run: &FlowRun) -> f32 {
    (run.line_height_points * 1.25) + run.style.baseline_shift_points().abs()
}

fn passive_base14_fallback_flow_run_line_height(run: &FlowRun, document: &Document) -> f32 {
    let style = passive_pdf_style_for_run(document, &run.style);
    let family = font_family_for_run_text(document, &style, &run.text);
    let multiplier = base14_line_height_multiplier(family);
    (run.line_height_points * multiplier) + run.style.baseline_shift_points().abs()
}

fn base14_line_height_multiplier(family: PdfFontFamily) -> f32 {
    match family {
        PdfFontFamily::Courier => 1.12,
        PdfFontFamily::Helvetica | PdfFontFamily::Times => 1.15,
        PdfFontFamily::Symbol | PdfFontFamily::ZapfDingbats => 1.15,
    }
}

fn measure_flow_run(
    run: &FlowRun,
    current_line_width: f32,
    paragraph_style: &ParagraphStyle,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> f32 {
    if let Some(image) = inline_image_for_marker(document, &run.text) {
        return image_display_size(image, f32::MAX).0;
    }
    if parse_bookmark_page_marker_id(&run.text, BOOKMARK_PAGE_ANCHOR_MARKER).is_some()
        || parse_bookmark_page_marker_id(&run.text, BOOKMARK_PAGE_REF_MARKER).is_some()
    {
        return 0.0;
    }
    if is_passive_advance_marker(&run.text) {
        return passive_advance_width_points(&run.style);
    }
    if is_passive_math_inline_lead_marker(&run.text) {
        return passive_advance_width_points(&run.style);
    }
    if is_passive_math_fraction_rule_marker(&run.text) {
        return 0.0;
    }
    if is_passive_math_stack_anchor_marker(&run.text) {
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
    let text_width = measure_text_with_document_font(
        &measurement_text,
        &style,
        font_family,
        document,
        font_provider,
    );
    text_width + (character_border_inset(&style) * 2.0)
}

fn character_border_inset(style: &CharacterStyle) -> f32 {
    if !style.border.visible {
        return 0.0;
    }
    let stroke_width = twips_to_points(style.border.width_twips.max(1));
    stroke_width + twips_to_points(style.border.spacing_twips.max(0))
}

fn inline_image_cluster_width(
    segments: &[FlowRun],
    start: usize,
    first_width: f32,
    current_line_width: f32,
    paragraph_style: &ParagraphStyle,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> f32 {
    let Some(first) = segments.get(start) else {
        return first_width;
    };
    if first.text.chars().last().is_some_and(char::is_whitespace) {
        return first_width;
    }

    let mut width = first_width;
    let mut contains_inline_image = inline_image_marker_index(&first.text).is_some();
    for segment in segments.iter().skip(start + 1) {
        if segment.text.chars().next().is_some_and(char::is_whitespace) {
            break;
        }
        contains_inline_image |= inline_image_marker_index(&segment.text).is_some();
        width += measure_flow_run(
            segment,
            current_line_width + width,
            paragraph_style,
            document,
            font_provider,
        );
        if segment.text.chars().last().is_some_and(char::is_whitespace) {
            break;
        }
    }
    if contains_inline_image {
        width
    } else {
        first_width
    }
}

fn is_passive_advance_marker(text: &str) -> bool {
    text == PASSIVE_ADVANCE_MARKER
}

fn is_passive_math_inline_lead_marker(text: &str) -> bool {
    text == PASSIVE_MATH_INLINE_LEAD_MARKER
}

fn is_passive_math_fraction_rule_marker(text: &str) -> bool {
    text == PASSIVE_MATH_FRACTION_RULE_MARKER
}

fn is_passive_math_stack_anchor_marker(text: &str) -> bool {
    text == PASSIVE_MATH_STACK_ANCHOR_MARKER
}

fn inline_image_for_marker<'a>(document: &'a Document, text: &str) -> Option<&'a StaticImage> {
    document.inline_images.get(inline_image_marker_index(text)?)
}

fn passive_advance_width_points(style: &CharacterStyle) -> f32 {
    twips_to_points(style.character_spacing_twips)
}

fn passive_advance_baseline_delta_points(style: &CharacterStyle) -> f32 {
    style.baseline_shift_points()
}

fn passive_math_fraction_width_points(style: &CharacterStyle) -> f32 {
    twips_to_points(style.character_spacing_twips).max(0.0)
}

fn passive_math_stack_width_points(style: &CharacterStyle) -> f32 {
    twips_to_points(style.character_spacing_twips).max(0.0)
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
    font_provider: Option<&FontProvider>,
    word_spacing: f32,
    character_spacing: f32,
) {
    push_line_with_rotation(
        pages,
        line,
        x,
        top_y,
        document,
        font_provider,
        word_spacing,
        character_spacing,
        TextRotation::None,
    );
}

fn push_line_with_rotation(
    pages: &mut [LayoutPage],
    line: &Line,
    x: f32,
    top_y: f32,
    document: &Document,
    font_provider: Option<&FontProvider>,
    word_spacing: f32,
    character_spacing: f32,
    rotation: TextRotation,
) {
    // Word treats positive `\slN\slmult1` as baseline-to-baseline advance:
    // the extra leading follows the glyphs instead of pushing the first line
    // down inside the enlarged line box. Exact/at-least spacing retains the
    // historical box-relative baseline behavior.
    let baseline_box_height = if line.height > line.natural_height {
        line.natural_height
    } else {
        line.height
    };
    let has_inline_image = line
        .runs
        .iter()
        .any(|run| inline_image_marker_index(&run.text).is_some());
    let line_border_inset = line
        .runs
        .iter()
        .map(|run| character_border_inset(&run.style))
        .fold(0.0_f32, f32::max);
    let baseline_y = if has_inline_image {
        top_y - line.height
    } else {
        top_y - baseline_box_height + (baseline_box_height * 0.25) + (line_border_inset * 0.5)
    };
    let mut cursor_advance = 0.0;
    let mut cursor_baseline_delta =
        word_compatible_source_line_baseline_delta_points(line, document, font_provider);
    let page = pages.last_mut().expect("layout always has a page");

    let last_spaced_run_index = line.runs.iter().rposition(|run| {
        !is_passive_advance_marker(&run.text)
            && !is_passive_math_inline_lead_marker(&run.text)
            && !is_passive_math_fraction_rule_marker(&run.text)
            && !is_passive_math_stack_anchor_marker(&run.text)
            && run.text != "\t"
            && !run.text.is_empty()
    });
    for (run_index, run) in line.runs.iter().enumerate() {
        if run.text.is_empty() {
            cursor_advance += run.width;
            continue;
        }
        if is_passive_advance_marker(&run.text) {
            cursor_advance += run.width;
            cursor_baseline_delta += passive_advance_baseline_delta_points(&run.style);
            continue;
        }
        if is_passive_math_inline_lead_marker(&run.text) {
            cursor_advance += run.width;
            continue;
        }
        if is_passive_math_fraction_rule_marker(&run.text) {
            if run.style.passive_math_fraction_has_bar {
                let fraction_width = passive_math_fraction_width_points(&run.style);
                let (x1, y1) = rotated_line_run_origin(
                    x,
                    baseline_y,
                    cursor_advance,
                    cursor_baseline_delta
                        + run.style.baseline_shift_points()
                        + PASSIVE_MATH_FRACTION_RULE_BASELINE_OFFSET_POINTS,
                    rotation,
                );
                let (x2, y2) = rotated_line_run_origin(
                    x,
                    baseline_y,
                    cursor_advance + fraction_width,
                    cursor_baseline_delta
                        + run.style.baseline_shift_points()
                        + PASSIVE_MATH_FRACTION_RULE_BASELINE_OFFSET_POINTS,
                    rotation,
                );
                page.items.push(LayoutItem::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    width: PASSIVE_MATH_FRACTION_RULE_WIDTH_POINTS,
                    color: style_color(document, &run.style),
                    style: LineStyle::Solid,
                });
            }
            continue;
        }
        if is_passive_math_stack_anchor_marker(&run.text) {
            continue;
        }
        if let Some(image) = inline_image_for_marker(document, &run.text) {
            let (width, height) = image_display_size(image, f32::MAX);
            let (run_x, run_baseline_y) = rotated_line_run_origin(
                x,
                baseline_y,
                cursor_advance,
                cursor_baseline_delta,
                rotation,
            );
            page.items.push(LayoutItem::Image(ImageFragment {
                image: image.clone(),
                x: run_x,
                y: run_baseline_y,
                width,
                height,
                z_order: 0,
                below_text: false,
            }));
            cursor_advance += width;
            continue;
        }
        let (run_x, run_baseline_y) = rotated_line_run_origin(
            x,
            baseline_y,
            cursor_advance,
            cursor_baseline_delta,
            rotation,
        );
        if parse_bookmark_page_marker_id(&run.text, BOOKMARK_PAGE_ANCHOR_MARKER).is_some()
            || parse_bookmark_page_marker_id(&run.text, BOOKMARK_PAGE_REF_MARKER).is_some()
        {
            page.items.push(LayoutItem::Text(TextFragment {
                text: run.text.clone(),
                x: run_x,
                baseline_y: run_baseline_y,
                rotation,
                color: style_color(document, &run.style),
                font_family: font_family_for_style(document, &run.style),
                word_spacing: 0.0,
                style: run.style.clone(),
            }));
            continue;
        }
        let extra_word_spacing = word_spacing * regular_space_count(&run.text) as f32;
        let character_count = run.text.chars().filter(|ch| *ch != '\n').count();
        let internal_character_gaps = character_count.saturating_sub(1);
        let extra_character_spacing = character_spacing * internal_character_gaps as f32;
        let width = run.width + extra_word_spacing + extra_character_spacing;
        if run.text == "\t" {
            push_tab_leader(page, run_x, run_baseline_y, width, run, document);
            cursor_advance += width;
            continue;
        }
        let mut style = passive_pdf_style_for_run(document, &run.style);
        if character_spacing > 0.0 {
            let added_twips = (character_spacing * 20.0)
                .round()
                .clamp(0.0, i32::MAX as f32) as i32;
            style.character_spacing_twips =
                style.character_spacing_twips.saturating_add(added_twips);
        }
        let border_inset = character_border_inset(&style);
        let text_width = (width - (border_inset * 2.0)).max(0.0);
        let text = display_text(&run.text, &style);
        let font_family = font_family_for_run_text(document, &style, &text);
        let (run_x, run_baseline_y) = rotated_line_run_origin(
            x,
            baseline_y,
            cursor_advance + border_inset,
            cursor_baseline_delta
                + style.baseline_shift_points()
                + word_compatible_source_run_baseline_delta_points(&style, document),
            rotation,
        );
        let color = style_color(document, &style);
        if let Some(highlight_index) = style.highlight_index
            && (highlight_index > 0 || style.highlight_shading_basis_points < 10_000)
        {
            let font_size = style.font_size_points();
            let (highlight_y, highlight_height) =
                if line_border_inset > 0.0 && !style.border.visible {
                    (
                        run_baseline_y - (font_size * 0.28) - line_border_inset,
                        line.height,
                    )
                } else {
                    (run_baseline_y - (font_size * 0.28), font_size * 1.12)
                };
            page.items.push(LayoutItem::Highlight {
                x: run_x,
                y: highlight_y,
                width: text_width,
                height: highlight_height,
                color: character_shading_color(document, highlight_index, &style),
            });
        }
        if style.form_field_shading {
            let font_size = style.font_size_points();
            let (shade_x, shade_width) =
                passive_form_field_shading_horizontal_bounds(run_x, text_width, font_size);
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
                run_x,
                run_baseline_y,
                text_width,
                &style,
                color,
                document,
            );
        }
        page.items.push(LayoutItem::Text(TextFragment {
            text: text.clone(),
            x: run_x,
            baseline_y: run_baseline_y,
            rotation,
            color,
            font_family,
            word_spacing,
            style: style.clone(),
        }));

        if style.overline {
            let y = run_baseline_y + (style.font_size_points() * 0.78);
            page.items.push(LayoutItem::Line {
                x1: run_x,
                y1: y,
                x2: run_x + text_width,
                y2: y,
                width: if style.passive_math { 0.84 } else { 0.5 },
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
                run_x,
                run_baseline_y
                    - if style.underline == UnderlineStyle::Thick {
                        2.8
                    } else {
                        2.0
                    },
                text_width,
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
                    x1: run_x,
                    y1: y + gap,
                    x2: run_x + text_width,
                    y2: y + gap,
                    width: 0.5,
                    color,
                    style: LineStyle::Solid,
                });
                page.items.push(LayoutItem::Line {
                    x1: run_x,
                    y1: y - gap,
                    x2: run_x + text_width,
                    y2: y - gap,
                    width: 0.5,
                    color,
                    style: LineStyle::Solid,
                });
            } else {
                page.items.push(LayoutItem::Line {
                    x1: run_x,
                    y1: y,
                    x2: run_x + text_width,
                    y2: y,
                    width: 0.5,
                    color,
                    style: LineStyle::Solid,
                });
            }
        }

        cursor_advance += width;
        if character_spacing > 0.0
            && character_count > 0
            && last_spaced_run_index.is_some_and(|last| run_index < last)
        {
            cursor_advance += character_spacing;
        }
    }
}

fn word_compatible_source_line_baseline_delta_points(
    line: &Line,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> f32 {
    if line.inline_image_baseline_context {
        return 0.0;
    }
    let source_inset = line
        .runs
        .iter()
        .filter(|run| {
            !run.text.trim().is_empty()
                && !contains_inline_marker(&run.text)
                && inline_image_marker_index(&run.text).is_none()
        })
        .filter_map(|run| {
            let inset_em = match font_for_style(document, &run.style) {
                Some(font) => {
                    let name = font.name.trim().to_ascii_lowercase();
                    if matches!(name.as_str(), "arial" | "liberation sans") {
                        WORD_ARIAL_BASELINE_INSET_EM
                    } else if font_uses_source_family(font, font_provider, &["calibri", "carlito"])
                    {
                        WORD_CALIBRI_BASELINE_INSET_EM
                    } else if name == "tinos"
                        || name == "times new roman"
                        || name.starts_with("times new roman ")
                    {
                        WORD_TIMES_BASELINE_INSET_EM
                    } else {
                        return None;
                    }
                }
                None if font_family_for_style(document, &run.style) == PdfFontFamily::Times => {
                    WORD_TIMES_BASELINE_INSET_EM
                }
                None => return None,
            };
            Some(run.style.font_size_points() * inset_em)
        })
        .reduce(f32::max);
    -source_inset.unwrap_or(0.0)
}

fn word_compatible_source_run_baseline_delta_points(
    style: &CharacterStyle,
    document: &Document,
) -> f32 {
    let is_cambria_math_nary = style.passive_math_nary_id.is_some()
        && font_for_style(document, style)
            .is_some_and(|font| font.name.trim().eq_ignore_ascii_case("cambria math"));
    if is_cambria_math_nary {
        style.font_size_points() * WORD_CAMBRIA_MATH_NARY_BASELINE_RISE_EM
    } else {
        0.0
    }
}

fn rotated_line_run_origin(
    x: f32,
    baseline_y: f32,
    advance: f32,
    baseline_delta: f32,
    rotation: TextRotation,
) -> (f32, f32) {
    match rotation {
        TextRotation::None => (x + advance, baseline_y + baseline_delta),
        TextRotation::Clockwise90 => (x + baseline_delta, baseline_y - advance),
        TextRotation::CounterClockwise90 => (x - baseline_delta, baseline_y + advance),
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
        TabLeader::Dots
        | TabLeader::Hyphens
        | TabLeader::Underline
        | TabLeader::MiddleDots
        | TabLeader::Equals => {
            let leader = match run.tab_leader {
                TabLeader::Dots => ".",
                TabLeader::Hyphens => "-",
                TabLeader::Underline => "_",
                TabLeader::MiddleDots => "\u{00b7}",
                TabLeader::Equals => "=",
                TabLeader::None => unreachable!("handled above"),
            };
            let family = font_family_for_style(document, &style);
            let leader_width = measure_text_with_family(leader, &style, family).max(1.0);
            let word_advance_em = match run.tab_leader {
                TabLeader::Dots => Some(0.28),
                TabLeader::Hyphens | TabLeader::MiddleDots => Some(0.33),
                TabLeader::Underline => Some(0.56),
                TabLeader::None | TabLeader::Equals => None,
            };
            let (leader_x, count, style) = if let Some(word_advance_em) = word_advance_em {
                let word_advance =
                    style.font_size_points() * word_advance_em * style.horizontal_scale();
                let word_advance = word_advance.max(1.0);
                let leader_x = ((cursor_x / word_advance).floor() + 1.0) * word_advance;
                let count = ((cursor_x + width - leader_x) / word_advance).floor() as usize;
                if count == 0 {
                    return;
                }
                let mut leader_style = style;
                let width_scale = (word_advance / leader_width).clamp(0.5, 2.0);
                leader_style.character_scaling_percent =
                    ((leader_style.character_scaling_percent.max(1) as f32 * width_scale)
                        .round()
                        .clamp(1.0, i32::MAX as f32)) as i32;
                (leader_x, count, leader_style)
            } else {
                (
                    cursor_x,
                    (width / leader_width).floor().max(1.0) as usize,
                    style,
                )
            };
            page.items.push(LayoutItem::Text(TextFragment {
                text: leader.repeat(count.min(512)),
                x: leader_x,
                baseline_y: baseline_y + style.baseline_shift_points(),
                rotation: TextRotation::None,
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
    let spacing = twips_to_points(style.border.spacing_twips.max(0));
    // LayoutItem::Line is center-stroked. Position each centerline so the
    // authored stroke occupies space outside the text's glyph box, matching
    // Word's filled-rectangle character-border geometry.
    let half_stroke = stroke_width * 0.5;
    let left = x - spacing - half_stroke;
    let right = x + text_width + spacing + half_stroke;
    let bottom = baseline_y - (style.font_size_points() * 0.28) - spacing - half_stroke;
    let top = baseline_y + (style.font_size_points() * 0.82) + spacing + half_stroke;

    if line_style == LineStyle::Solid {
        let outer_left = left - half_stroke;
        let outer_right = right + half_stroke;
        let outer_bottom = bottom - half_stroke;
        let outer_top = top + half_stroke;
        for (x, y, width, height) in [
            (
                outer_left,
                outer_top - stroke_width,
                outer_right - outer_left,
                stroke_width,
            ),
            (
                outer_left,
                outer_bottom,
                outer_right - outer_left,
                stroke_width,
            ),
            (
                outer_left,
                outer_bottom,
                stroke_width,
                outer_top - outer_bottom,
            ),
            (
                outer_right - stroke_width,
                outer_bottom,
                stroke_width,
                outer_top - outer_bottom,
            ),
        ] {
            page.items.push(LayoutItem::Highlight {
                x,
                y,
                width,
                height,
                color,
            });
        }
        return;
    }

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
        BorderStyle::Triple => LineStyle::Triple,
        BorderStyle::Dotted => LineStyle::Dotted,
        BorderStyle::Dashed => LineStyle::Dashed,
        BorderStyle::Wavy => LineStyle::Wavy,
        BorderStyle::Emboss => LineStyle::Emboss,
        BorderStyle::Engrave => LineStyle::Engrave,
        BorderStyle::ThinThick => LineStyle::ThinThick,
        BorderStyle::ThickThin => LineStyle::ThickThin,
        BorderStyle::ThinThickThin => LineStyle::ThinThickThin,
        BorderStyle::BorderArtScissors => LineStyle::Solid,
    }
}

fn line_cap_for_shape_cap(cap: StaticShapeLineCap) -> LineCap {
    match cap {
        StaticShapeLineCap::Flat => LineCap::Flat,
        StaticShapeLineCap::Round => LineCap::Round,
        StaticShapeLineCap::Square => LineCap::Square,
    }
}

fn line_join_for_shape_join(join: StaticShapeLineJoin) -> LineJoin {
    match join {
        StaticShapeLineJoin::Miter => LineJoin::Miter,
        StaticShapeLineJoin::Round => LineJoin::Round,
        StaticShapeLineJoin::Bevel => LineJoin::Bevel,
    }
}

fn justified_word_spacing(
    line: &Line,
    style: &ParagraphStyle,
    available_width: f32,
    is_last_line: bool,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> f32 {
    if !matches!(
        style.alignment,
        Alignment::Justified | Alignment::Distributed
    ) || is_last_line
        || line.width <= 0.0
    {
        return 0.0;
    }
    let total_space_count = line
        .runs
        .iter()
        .map(|run| regular_space_count(&run.text))
        .sum::<usize>();
    let (trailing_space_count, trailing_space_width) =
        trailing_regular_space_geometry(line, style, document, font_provider);
    let space_count = total_space_count.saturating_sub(trailing_space_count);
    if space_count == 0 {
        return 0.0;
    }
    ((available_width - (line.width - trailing_space_width)) / space_count as f32).max(0.0)
}

fn trailing_regular_space_geometry(
    line: &Line,
    paragraph_style: &ParagraphStyle,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> (usize, f32) {
    let mut count = 0usize;
    let mut width = 0.0;
    for run in line.runs.iter().rev() {
        let trailing_count = run.text.chars().rev().take_while(|ch| *ch == ' ').count();
        if trailing_count == 0 {
            break;
        }
        count += trailing_count;
        let trimmed_text = run.text.trim_end_matches(' ');
        let trimmed_width = if trimmed_text.is_empty() {
            0.0
        } else {
            let mut trimmed_run = run.clone();
            trimmed_run.text = trimmed_text.to_string();
            measure_flow_run(&trimmed_run, 0.0, paragraph_style, document, font_provider)
        };
        width += (run.width - trimmed_width).max(0.0);
        if !trimmed_text.is_empty() {
            break;
        }
    }
    (count, width)
}

fn distributed_character_spacing(
    line: &Line,
    style: &ParagraphStyle,
    available_width: f32,
    is_last_line: bool,
) -> f32 {
    if style.alignment != Alignment::Distributed || !is_last_line || line.width <= 0.0 {
        return 0.0;
    }
    let character_count = line
        .runs
        .iter()
        .filter(|run| {
            !is_passive_advance_marker(&run.text)
                && !is_passive_math_inline_lead_marker(&run.text)
                && run.text != "\t"
        })
        .map(|run| run.text.chars().filter(|ch| *ch != '\n').count())
        .sum::<usize>();
    let gap_count = character_count.saturating_sub(1);
    if gap_count == 0 {
        return 0.0;
    }
    ((available_width - line.width) / gap_count as f32).max(0.0)
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
        // Legacy Word-compatible RTF rendering uses Times New Roman when the
        // document provides no usable font table/default-font definition.
        return PdfFontFamily::Times;
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

pub(crate) fn passive_supplied_font_fallback_name(font: &FontDef) -> Option<&'static str> {
    if font.pitch == FontPitch::Fixed {
        Some("Courier New")
    } else {
        match font.family {
            FontFamilyHint::Roman => Some("Times New Roman"),
            FontFamilyHint::Modern | FontFamilyHint::Swiss => Some("Calibri"),
            _ => None,
        }
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
                | '\u{25c9}'
                | '\u{2610}'
                | '\u{2611}'
                | '\u{2612}'
                | '\u{263a}'
                | '\u{2713}'
                | '\u{2714}'
                | '\u{2717}'
                | '\u{2751}'
                | '\u{1f5f5}'
                | '\u{1f5f7}'
                | '\u{1f4e5}'
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
            | '\u{25c9}'
            | '\u{2610}'
            | '\u{2611}'
            | '\u{2612}'
            | '\u{263a}'
            | '\u{2713}'
            | '\u{2714}'
            | '\u{2717}'
            | '\u{2751}'
            | '\u{1f5f5}'
            | '\u{1f5f7}'
            | '\u{1f4e5}'
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
    let factor = (style.highlight_shading_basis_points as f32 / 10_000.0).clamp(0.0, 1.0);
    if index == 0 {
        let gray = 1.0 - factor;
        return PdfColor {
            red: gray,
            green: gray,
            blue: gray,
        };
    }
    let color = color_for_index(document, index);
    PdfColor {
        red: color.red * factor,
        green: color.green * factor,
        blue: color.blue * factor,
    }
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

fn shading_is_visible(
    document: &Document,
    background_color_index: Option<usize>,
    pattern: ShadingPattern,
) -> bool {
    background_color_index.is_some_and(|index| index > 0 && index < document.colors.len())
        || pattern
            .foreground_color_index()
            .is_some_and(|index| index < document.colors.len())
}

fn shading_fill_color(
    document: &Document,
    background_color_index: Option<usize>,
    basis_points: i32,
    pattern: ShadingPattern,
) -> PdfColor {
    let white = PdfColor {
        red: 1.0,
        green: 1.0,
        blue: 1.0,
    };
    let background = background_color_index
        .filter(|index| *index > 0 && *index < document.colors.len())
        .map(|index| color_for_index(document, index))
        .unwrap_or(white);

    if pattern.without_foreground_color() != ShadingPattern::None {
        return background;
    }

    if let Some(foreground_color_index) = pattern
        .foreground_color_index()
        .filter(|index| *index < document.colors.len())
    {
        let foreground = color_for_index(document, foreground_color_index);
        let factor = (basis_points as f32 / 10_000.0).clamp(0.0, 1.0);
        return PdfColor {
            red: background.red + ((foreground.red - background.red) * factor),
            green: background.green + ((foreground.green - background.green) * factor),
            blue: background.blue + ((foreground.blue - background.blue) * factor),
        };
    }

    background_color_index
        .filter(|index| *index > 0 && *index < document.colors.len())
        .map(|index| shading_color(document, index, basis_points))
        .unwrap_or(white)
}

#[allow(clippy::too_many_arguments)]
fn push_shading_rect(
    pages: &mut [LayoutPage],
    document: &Document,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    background_color_index: Option<usize>,
    basis_points: i32,
    pattern: ShadingPattern,
) {
    let fill_color = shading_fill_color(document, background_color_index, basis_points, pattern);
    let line_color = match pattern.foreground_color_index() {
        Some(index) => document
            .colors
            .get(index)
            .map(|_| color_for_index(document, index)),
        None => Some(PdfColor::default()),
    };
    let page = pages.last_mut().expect("layout always has a page");
    page.items.push(LayoutItem::Highlight {
        x,
        y,
        width,
        height,
        color: fill_color,
    });
    if let Some(line_color) = line_color {
        push_shading_pattern_lines(page, x, y, width, height, line_color, pattern);
    }
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
    let pattern = pattern.without_foreground_color();
    if pattern == ShadingPattern::None || width <= 0.5 || height <= 0.5 {
        return;
    }

    let spacing = shading_pattern_spacing(pattern);
    let stroke_width = 0.24;
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
        ShadingPattern::None | ShadingPattern::VerticalGradient => {}
        _ => {}
    }
}

fn push_shape_fill_pattern_lines(
    page: &mut LayoutPage,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    fill_color: Option<crate::model::Color>,
    pattern: ShadingPattern,
) {
    if pattern == ShadingPattern::None || fill_color.is_none() {
        return;
    }
    let color = fill_color.expect("checked above");
    push_shading_pattern_lines(
        page,
        x,
        y,
        width,
        height,
        PdfColor {
            red: color.red as f32 / 255.0,
            green: color.green as f32 / 255.0,
            blue: color.blue as f32 / 255.0,
        },
        pattern,
    );
}

fn push_passive_vertical_gradient_bands(
    page: &mut LayoutPage,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    start_color: PdfColor,
    end_color: Option<PdfColor>,
) {
    if width <= 0.5 || height <= 0.5 {
        return;
    }
    const BANDS: usize = 16;
    let band_height = height / BANDS as f32;
    for index in 0..BANDS {
        let t = if BANDS <= 1 {
            0.0
        } else {
            index as f32 / (BANDS - 1) as f32
        };
        let end_color = end_color.unwrap_or(PdfColor {
            red: 1.0,
            green: 1.0,
            blue: 1.0,
        });
        let lightened = PdfColor {
            red: start_color.red + ((end_color.red - start_color.red) * t),
            green: start_color.green + ((end_color.green - start_color.green) * t),
            blue: start_color.blue + ((end_color.blue - start_color.blue) * t),
        };
        page.items.push(LayoutItem::Highlight {
            x,
            y: y + (band_height * index as f32),
            width,
            height: if index + 1 == BANDS {
                height - (band_height * index as f32)
            } else {
                band_height + 0.1
            },
            color: lightened,
        });
    }
}

fn shading_pattern_spacing(pattern: ShadingPattern) -> f32 {
    match pattern.without_foreground_color() {
        ShadingPattern::DarkHorizontal
        | ShadingPattern::DarkVertical
        | ShadingPattern::DarkForwardDiagonal
        | ShadingPattern::DarkBackwardDiagonal
        | ShadingPattern::DarkCross
        | ShadingPattern::DarkDiagonalCross => 0.6,
        ShadingPattern::None
        | ShadingPattern::Horizontal
        | ShadingPattern::Vertical
        | ShadingPattern::ForwardDiagonal
        | ShadingPattern::BackwardDiagonal
        | ShadingPattern::Cross
        | ShadingPattern::DiagonalCross
        | ShadingPattern::VerticalGradient => 0.96,
        _ => 0.96,
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
        Alignment::Left | Alignment::Justified | Alignment::Distributed => left,
        Alignment::Center => left + ((available - line_width).max(0.0) / 2.0),
        Alignment::Right => left + (available - line_width).max(0.0),
    }
}

fn paragraph_line_width(content_width: f32, style: &ParagraphStyle, is_first_line: bool) -> f32 {
    let line_left_indent = paragraph_line_left_indent_twips(style, is_first_line);
    let no_wrap_emergency_inset = if style.no_wrap {
        style.no_wrap_emergency_right_inset_twips.clamp(0, 10_000)
    } else {
        0
    };
    (content_width
        - twips_to_points(line_left_indent + style.right_indent_twips + no_wrap_emergency_inset))
    .max(12.0)
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
    (base_width + passive_kerning_width(text, style, family)) * style.horizontal_scale()
        + character_spacing_width(text, style)
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
        if let Some(source_width) =
            source_word_compatible_advance_points(source_font, style, ch, size, font_provider)
        {
            width += source_width;
            continue;
        }
        let font_width = font_provider
            .zip(source_font)
            .filter(|_| !matches!(family, PdfFontFamily::Symbol | PdfFontFamily::ZapfDingbats))
            .and_then(|(provider, font)| supplied_font_glyph_metrics(provider, font, style, ch))
            .map(|metrics| metrics.advance_points(size));
        width += font_width.unwrap_or_else(|| base14_char_width_points(ch, size, family, style));
    }
    let kerning_width =
        passive_kerning_width_with_document_font(text, style, family, source_font, font_provider);
    (width + kerning_width) * style.horizontal_scale() + character_spacing_width(text, style)
}

pub(crate) fn source_word_compatible_advance_points(
    font: Option<&FontDef>,
    style: &CharacterStyle,
    ch: char,
    size: f32,
    font_provider: Option<&FontProvider>,
) -> Option<f32> {
    source_calibri_advance_points(font, style, ch, size, font_provider)
        .or_else(|| source_cambria_advance_points(font, style, ch, size, font_provider))
        .or_else(|| source_aptos_advance_points(font, style, ch, size, font_provider))
        .or_else(|| source_dingbat_advance_points(font, ch, size))
        .or_else(|| {
            (style.passive_math && style.passive_math_fraction_id.is_none())
                .then(|| source_cambria_math_advance_points(font, ch, size))
                .flatten()
        })
}

fn source_calibri_advance_points(
    font: Option<&FontDef>,
    style: &CharacterStyle,
    ch: char,
    size: f32,
    font_provider: Option<&FontProvider>,
) -> Option<f32> {
    if !font_uses_source_family(font?, font_provider, &["calibri", "carlito"]) {
        return None;
    }
    if !style.bold && !style.italic {
        let units = match ch {
            '\u{00b3}' => 685,
            '\u{00d2}' | '\u{00d3}' => 1356,
            '\u{00e9}' => 1019,
            '\u{201a}' => 511,
            '\u{201c}' | '\u{201d}' => 857,
            '\u{2502}' => 1024,
            _ => 0,
        };
        if units != 0 {
            return Some(size * units as f32 / 2_048.0);
        }
    }
    let index = usize::try_from(u32::from(ch).checked_sub(32)?).ok()?;
    const REGULAR: [u16; 95] = [
        463, 667, 821, 1020, 1038, 1464, 1397, 452, 621, 621, 1020, 1020, 511, 627, 517, 791, 1038,
        1038, 1038, 1038, 1038, 1038, 1038, 1038, 1038, 1038, 548, 548, 1020, 1020, 1020, 949,
        1831, 1185, 1114, 1092, 1260, 1000, 941, 1292, 1276, 516, 653, 1064, 861, 1751, 1322, 1356,
        1058, 1378, 1112, 941, 998, 1314, 1162, 1822, 1063, 998, 959, 628, 791, 628, 1020, 1020,
        596, 981, 1076, 866, 1076, 1019, 625, 964, 1076, 470, 490, 931, 470, 1636, 1076, 1080,
        1076, 1076, 714, 801, 686, 1076, 925, 1464, 887, 927, 809, 644, 943, 644, 1020,
    ];
    const BOLD: [u16; 95] = [
        463, 667, 898, 1020, 1038, 1493, 1443, 478, 638, 638, 1020, 1020, 528, 627, 547, 880, 1038,
        1038, 1038, 1038, 1038, 1038, 1038, 1038, 1038, 1038, 565, 565, 1020, 1020, 1020, 949,
        1840, 1241, 1148, 1084, 1291, 999, 940, 1305, 1292, 546, 678, 1120, 866, 1790, 1349, 1385,
        1090, 1405, 1153, 968, 1014, 1337, 1211, 1856, 1128, 1064, 979, 665, 880, 665, 1020, 1020,
        615, 1011, 1099, 857, 1099, 1031, 648, 971, 1099, 503, 523, 983, 503, 1666, 1099, 1101,
        1099, 1099, 728, 817, 710, 1099, 969, 1526, 941, 970, 814, 704, 973, 704, 1020,
    ];
    const ITALIC: [u16; 95] = [
        463, 667, 821, 1020, 1038, 1464, 1397, 452, 621, 621, 1020, 1020, 511, 627, 517, 794, 1038,
        1038, 1038, 1038, 1038, 1038, 1038, 1038, 1038, 1038, 548, 548, 1020, 1020, 1020, 949,
        1831, 1185, 1114, 1070, 1260, 1000, 941, 1292, 1276, 516, 653, 1064, 861, 1751, 1320, 1340,
        1058, 1360, 1112, 926, 998, 1314, 1162, 1823, 1063, 998, 959, 628, 787, 628, 1020, 1020,
        596, 1053, 1053, 852, 1053, 978, 625, 1053, 1053, 470, 490, 931, 470, 1620, 1053, 1051,
        1053, 1053, 702, 797, 686, 1053, 913, 1464, 887, 916, 809, 644, 943, 644, 1020,
    ];
    const BOLD_ITALIC: [u16; 95] = [
        463, 667, 898, 1020, 1038, 1493, 1443, 478, 638, 638, 1020, 1020, 528, 627, 547, 889, 1038,
        1038, 1038, 1038, 1038, 1038, 1038, 1038, 1038, 1038, 565, 565, 1020, 1020, 1020, 949,
        1840, 1241, 1148, 1062, 1291, 999, 940, 1305, 1292, 546, 678, 1120, 866, 1790, 1344, 1369,
        1090, 1387, 1153, 953, 1014, 1337, 1211, 1857, 1128, 1064, 979, 665, 870, 665, 1020, 1020,
        615, 1081, 1081, 843, 1081, 1006, 648, 1081, 1080, 503, 523, 983, 503, 1646, 1080, 1080,
        1081, 1081, 721, 807, 710, 1080, 961, 1526, 941, 963, 814, 704, 973, 704, 1020,
    ];
    let units = match (style.bold, style.italic) {
        (false, false) => *REGULAR.get(index)?,
        (true, false) => *BOLD.get(index)?,
        (false, true) => *ITALIC.get(index)?,
        (true, true) => *BOLD_ITALIC.get(index)?,
    };
    Some(size * f32::from(units) / 2_048.0)
}

fn source_cambria_advance_points(
    font: Option<&FontDef>,
    style: &CharacterStyle,
    ch: char,
    size: f32,
    font_provider: Option<&FontProvider>,
) -> Option<f32> {
    if style.bold || style.italic || !font_uses_source_family(font?, font_provider, &["cambria"]) {
        return None;
    }
    // Bounded regular-face hmtx values independently present in the registered
    // Word font-family-hint reference. No proprietary font bytes are retained.
    let units = match ch {
        ' ' => 451,
        '.' => 420,
        'R' => 1272,
        'a' => 1000,
        'e' => 999,
        'f' => 620,
        'i' => 569,
        'l' => 556,
        'm' => 1704,
        'n' => 1143,
        'o' => 1087,
        't' => 693,
        'x' => 990,
        'y' => 1032,
        _ => return None,
    };
    Some(size * units as f32 / 2_048.0)
}

fn source_aptos_advance_points(
    font: Option<&FontDef>,
    style: &CharacterStyle,
    ch: char,
    size: f32,
    font_provider: Option<&FontProvider>,
) -> Option<f32> {
    if style.bold || style.italic || !font_uses_source_family(font?, font_provider, &["aptos"]) {
        return None;
    }
    // Bounded regular-face hmtx values independently present in the registered
    // Word font-family-hint reference. No proprietary font bytes are retained.
    let units = match ch {
        ' ' => 416,
        '.' => 585,
        'T' => 980,
        'b' => 1149,
        'd' => 1148,
        'e' => 1079,
        'h' => 1129,
        'm' => 1747,
        'o' => 1130,
        't' => 662,
        'x' => 906,
        'y' => 926,
        _ => return None,
    };
    Some(size * units as f32 / 2_048.0)
}

fn font_uses_source_family(
    font: &FontDef,
    provider: Option<&FontProvider>,
    families: &[&str],
) -> bool {
    let matches_family = |name: &str| {
        let name = name.trim().to_ascii_lowercase();
        families.contains(&name.as_str())
    };
    matches_family(&font.name)
        || (font.alternate_name.as_deref().is_some_and(matches_family)
            && !provider.is_some_and(|provider| provider.has_exact_asset_for_family(&font.name)))
}

pub(crate) fn source_dingbat_advance_points(
    font: Option<&FontDef>,
    ch: char,
    size: f32,
) -> Option<f32> {
    let name = font?.name.to_ascii_lowercase();
    let em = if name.contains("segoe ui symbol") {
        match ch {
            '\u{25a1}' | '\u{2610}' | '\u{2611}' | '\u{2612}' => 0.86,
            '\u{2713}' | '\u{2714}' => 0.75,
            '\u{2717}' => 0.83,
            ch if ch.is_whitespace() => 0.27,
            _ => return None,
        }
    } else if name.contains("wingdings 3") || name.contains("wingdings3") {
        match ch {
            '\u{2190}' | '\u{2192}' => 0.89,
            '\u{2191}' | '\u{2193}' => 0.602,
            ch if ch.is_whitespace() => 1.0,
            _ => return None,
        }
    } else if name.contains("wingdings 2") || name.contains("wingdings2") {
        match ch {
            '\u{2717}' => 0.645,
            '\u{2713}' | '\u{2714}' => 0.766,
            '\u{25c9}' | '\u{2610}' | '\u{2611}' | '\u{2612}' | '\u{1f5f5}' | '\u{1f5f7}' => 0.892,
            ch if ch.is_whitespace() => 1.0,
            _ => return None,
        }
    } else if name.contains("wingdings") {
        match ch {
            '\u{25c9}' | '\u{2610}' | '\u{2611}' | '\u{2612}' => 0.892,
            '\u{2713}' | '\u{2714}' => 0.786,
            '\u{2717}' => 0.635,
            ch if ch.is_whitespace() => 1.0,
            _ => return None,
        }
    } else if name.contains("webdings") {
        match ch {
            '\u{25a1}' | '\u{2610}' | '\u{2611}' | '\u{2612}' | '\u{2713}' | '\u{2714}'
            | '\u{1f4e5}' => 1.0,
            ch if ch.is_whitespace() => 0.5,
            _ => return None,
        }
    } else {
        return None;
    };
    Some(size * em)
}

fn source_cambria_math_advance_points(font: Option<&FontDef>, ch: char, size: f32) -> Option<f32> {
    let name = font?.name.to_ascii_lowercase();
    if !name.contains("cambria math") {
        return None;
    }

    // Bounded horizontal metrics recovered from Word 16.0's embedded Cambria Math
    // subsets. Keeping only the characters independently present in the registered
    // Office Math references avoids host font discovery and proprietary font bytes.
    let units = match ch {
        ' ' | '\u{00a0}' => 451,
        '+' | '=' | '\u{2212}' => 1530,
        '.' => 420,
        '/' => 1004,
        '1' | '2' => 1134,
        ':' => 540,
        '?' => 865,
        '(' | ')' => 850,
        'A' => 1276,
        'B' => 1252,
        'D' => 1355,
        'E' => 1178,
        'F' | 'L' => 1099,
        'G' => 1251,
        'H' => 1406,
        'I' => 664,
        'M' => 1670,
        'N' => 1394,
        'O' => 1338,
        'P' => 1163,
        'R' => 1272,
        'S' => 1016,
        'T' => 1214,
        'U' => 1327,
        'V' => 1236,
        'X' => 1170,
        'Y' => 1168,
        'a' => 1000,
        'b' => 1121,
        'c' => 903,
        'd' => 1136,
        'e' => 999,
        'h' => 1130,
        'i' => 569,
        'k' => 1074,
        'l' => 556,
        'm' => 1704,
        'n' => 1143,
        'o' => 1087,
        'p' => 1139,
        'q' => 1120,
        'r' => 847,
        's' => 880,
        't' => 693,
        'v' => 1032,
        'x' => 990,
        'z' => 931,
        '\u{2211}' => 1451,
        '\u{220f}' => 1663,
        '\u{221a}' => 1345,
        _ => return None,
    };
    Some(size * units as f32 / 2048.0)
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
        .exact_glyph_metrics_for_char_with_style(&font.name, asset_style, ch)
        .or_else(|| {
            font.alternate_name.as_deref().and_then(|alternate| {
                provider.exact_glyph_metrics_for_char_with_style(alternate, asset_style, ch)
            })
        })
        .or_else(|| {
            passive_supplied_font_fallback_name(font).and_then(|fallback| {
                provider.exact_glyph_metrics_for_char_with_style(fallback, asset_style, ch)
            })
        })
        .or_else(|| provider.glyph_metrics_for_char_with_style(&font.name, asset_style, ch))
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

    let units = if ch == '\u{00b7}' {
        match family {
            PdfFontFamily::Courier => 600,
            PdfFontFamily::Helvetica => 278,
            PdfFontFamily::Times => 250,
            PdfFontFamily::Symbol | PdfFontFamily::ZapfDingbats => symbol_width_units(ch),
        }
    } else {
        match family {
            PdfFontFamily::Courier => 600,
            PdfFontFamily::Helvetica if style.bold => helvetica_bold_width_units(ch),
            PdfFontFamily::Helvetica => helvetica_width_units(ch),
            PdfFontFamily::Times if style.bold => times_bold_width_units(ch),
            PdfFontFamily::Times if style.italic => times_italic_width_units(ch),
            PdfFontFamily::Times => times_width_units(ch),
            PdfFontFamily::Symbol | PdfFontFamily::ZapfDingbats => symbol_width_units(ch),
        }
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

fn passive_kerning_width_with_document_font(
    text: &str,
    style: &CharacterStyle,
    family: PdfFontFamily,
    source_font: Option<&FontDef>,
    font_provider: Option<&FontProvider>,
) -> f32 {
    if !style_uses_passive_kerning(style) {
        return 0.0;
    }
    let mut total = 0.0;
    let mut previous = None;
    for ch in text.chars().filter(|ch| !is_zero_width_format_char(*ch)) {
        if let Some(left) = previous {
            total += source_word_compatible_pair_kerning_points(
                source_font,
                style,
                left,
                ch,
                font_provider,
            )
            .or_else(|| {
                supplied_font_pair_kerning_points(font_provider, source_font, style, left, ch)
            })
            .unwrap_or_else(|| passive_pair_kerning_points(left, ch, family, style));
        }
        previous = Some(ch);
    }
    total
}

pub(crate) fn source_word_compatible_pair_kerning_points(
    font: Option<&FontDef>,
    style: &CharacterStyle,
    left: char,
    right: char,
    font_provider: Option<&FontProvider>,
) -> Option<f32> {
    if !font_uses_source_family(font?, font_provider, &["calibri", "carlito"])
        || style.bold
        || style.italic
    {
        return None;
    }
    // Bounded regular-face pairs independently present in the registered
    // Calibri metadata references. Values are 2048-unit Calibri/Carlito kern
    // metrics; uncovered pairs continue through the supplied safe font.
    let units = match (left, right) {
        ('W', 'o') => -86,
        ('a', 'f') => -12,
        ('a', 't') => -19,
        ('a', 'y') => -38,
        ('e', 'f') => -18,
        ('e', 't') => -11,
        ('e', 'x') => -31,
        ('f', 'o') => -43,
        ('g', 'e') => -17,
        ('g', 'g') => 19,
        ('k', 'e') => -66,
        ('r', '.') => -206,
        ('r', 'd') => -28,
        ('r', 'e') => -27,
        ('s', 't') => -23,
        ('t', 'a') => -25,
        ('t', 'e') => -22,
        ('x', 't') => 6,
        ('y', 'o') => -24,
        _ => return None,
    };
    Some(style.font_size_points() * units as f32 / 2_048.0)
}

fn supplied_font_pair_kerning_points(
    provider: Option<&FontProvider>,
    font: Option<&FontDef>,
    style: &CharacterStyle,
    left: char,
    right: char,
) -> Option<f32> {
    let provider = provider?;
    let font = font?;
    let asset_style = FontAssetStyle {
        bold: style.bold,
        italic: style.italic,
    };
    let size = style.font_size_points();
    provider
        .exact_pair_kerning_points_for_chars_with_style(&font.name, asset_style, left, right, size)
        .or_else(|| {
            font.alternate_name.as_deref().and_then(|alternate| {
                provider.exact_pair_kerning_points_for_chars_with_style(
                    alternate,
                    asset_style,
                    left,
                    right,
                    size,
                )
            })
        })
        .or_else(|| {
            passive_supplied_font_fallback_name(font).and_then(|fallback| {
                provider.exact_pair_kerning_points_for_chars_with_style(
                    fallback,
                    asset_style,
                    left,
                    right,
                    size,
                )
            })
        })
        .or_else(|| {
            provider.pair_kerning_points_for_chars_with_style(
                &font.name,
                asset_style,
                left,
                right,
                size,
            )
        })
        .or_else(|| {
            font.alternate_name.as_deref().and_then(|alternate| {
                provider.pair_kerning_points_for_chars_with_style(
                    alternate,
                    asset_style,
                    left,
                    right,
                    size,
                )
            })
        })
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
        BOOKMARK_PAGE_ANCHOR_MARKER, BOOKMARK_PAGE_MARKER_END, BOOKMARK_PAGE_REF_MARKER, Block,
        Color, Document, FontDef, FontFamilyHint, ImageCrop, ImageFormat, PAGE_NUMBER_MARKER,
        PageNumberFormat, PageSettings, Paragraph, Run, SECTION_PAGES_MARKER, StaticImagePlacement,
        StaticShape, StaticShapeKind, StaticShapePoint, TOTAL_PAGES_MARKER, Table, TableCell,
        TableCellBorder, TableCellBorders, TableCellPadding, TableCellSpacing,
        TableCellVerticalMerge, TableRow, TableRowWrapMargins,
    };
    use crate::rtf::parse_rtf;

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

    fn deep_drawing_wrapped_item(mut item: LayoutItem, depth: usize) -> LayoutItem {
        for layer in 0..depth {
            item = LayoutItem::Drawing(DrawingFragment {
                z_order: layer.min(i32::MAX as usize) as i32,
                below_text: false,
                item: Box::new(item),
            });
        }
        item
    }

    fn leaf_text_fragment(item: &LayoutItem) -> &TextFragment {
        let mut current = item;
        loop {
            match current {
                LayoutItem::Drawing(drawing) => current = drawing.item.as_ref(),
                LayoutItem::Text(fragment) => return fragment,
                _ => panic!("expected text leaf"),
            }
        }
    }

    fn passive_text_item(text: &str, x: f32, baseline_y: f32) -> LayoutItem {
        LayoutItem::Text(TextFragment {
            text: text.to_string(),
            x,
            baseline_y,
            rotation: TextRotation::None,
            color: PdfColor::default(),
            font_family: PdfFontFamily::Helvetica,
            word_spacing: 0.0,
            style: CharacterStyle::default(),
        })
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
                column_widths_twips: Vec::new(),
                height_twips: None,
                left_offset_twips: 0,
                vertical_offset_twips: 0,
                wrap_margins: TableRowWrapMargins::default(),
                cell_gap_twips: 60,
                alignment: TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                keep_with_next: false,
                no_overlap: false,
                cells: vec![TableCell {
                    nested_table: None,
                    shading_color_index: Some(1),
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: TableCellBorders::default(),
                    fit_text: false,
                    text_direction: TableCellTextDirection::LeftToRightTopToBottom,
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
        character_border_inset(style)
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
                LayoutItem::Highlight {
                    x,
                    y,
                    width,
                    height,
                    ..
                } if *x > fragment.x
                    && *width <= fragment.style.font_size_points() + 0.01
                    && fragment.baseline_y >= *y - 0.01
                    && fragment.baseline_y <= *y + *height + 0.01 =>
                {
                    Some(*x + *width)
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

    fn has_horizontal_line_at(page: &LayoutPage, x1: f32, x2: f32, y: f32) -> bool {
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

    // Verbatim tests are grouped by subsystem; shared fixtures remain above.
    mod late_fields {
        use super::*;
        include!("layout_tests/late_fields.rs");
    }
    mod layout_core {
        use super::*;
        include!("layout_tests/layout_core.rs");
    }
    mod line_break {
        use super::*;
        include!("layout_tests/line_break.rs");
    }
    mod notes {
        use super::*;
        include!("layout_tests/notes.rs");
    }
    mod page {
        use super::*;
        include!("layout_tests/page.rs");
    }
    mod paragraph {
        use super::*;
        include!("layout_tests/paragraph.rs");
    }
    mod shapes {
        use super::*;
        include!("layout_tests/shapes.rs");
    }
    mod tables {
        use super::*;
        include!("layout_tests/tables.rs");
    }
    mod text_metrics {
        use super::*;
        include!("layout_tests/text_metrics.rs");
    }
}
