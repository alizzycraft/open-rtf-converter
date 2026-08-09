mod late_fields;
mod line_break;
mod notes;
mod page;
mod paragraph;
mod shapes;
mod tables;
mod text_metrics;

use late_fields::*;
use line_break::*;
use notes::*;
use page::*;
use paragraph::*;
use shapes::*;
use tables::*;
use text_metrics::*;

pub use text_metrics::{
    measure_text, passive_pair_kerning_points, style_uses_passive_kerning, twips_to_points,
};
pub(crate) use text_metrics::{
    passive_pdf_font_family_for_font, passive_supplied_font_fallback_name,
    source_dingbat_advance_points, source_word_compatible_advance_points,
    source_word_compatible_pair_kerning_points,
};

use std::collections::{HashMap, HashSet};

use unicode_linebreak::{BreakOpportunity, linebreaks};

use crate::fonts::{FontAssetStyle, FontProvider};
use crate::model::StaticImageWrapSide;
use crate::model::{
    Alignment, Block, BookmarkId, BorderStyle, CharacterStyle, Document, DynamicField,
    EndnotePlacement, FontDef, FontFamilyHint, FontPitch, FootnotePlacement, ImageFormat,
    InlineImageId, InlineKind, LayoutAdvance, LineNumberRestart, MathInline, ModelInvariantError,
    NoteNumberRestart, NoteNumbering, PageNumberFormat, PageSettings, PageVerticalAlignment,
    Paragraph, ParagraphBorders, ParagraphFrameVerticalAlignment, ParagraphStyle,
    PassiveMathFractionPart, PassiveMathLimitPart, Run, ShadingPattern, StaticImage,
    StaticImageVectorCommand, StaticImageVectorFillRule, StaticShape, StaticShapeArrowhead,
    StaticShapeHorizontalAnchor, StaticShapeKind, StaticShapeLineCap, StaticShapeLineJoin,
    StaticShapeTextVerticalAnchor, StaticShapeVerticalAnchor,
    TABLE_ROW_DYNAMIC_VERTICAL_BOTTOM_OFFSET_BASE, TABLE_ROW_DYNAMIC_VERTICAL_CENTER_OFFSET_BASE,
    TABLE_ROW_DYNAMIC_VERTICAL_OFFSET_SPAN_TWIPS, TabAlignment, TabLeader, Table, TableCell,
    TableCellBorder, TableCellHorizontalMerge, TableCellTextDirection, TableCellVerticalAlign,
    TableCellVerticalMerge, TableRow, TableRowAlignment, TableRowWrapMargins, UnderlineStyle,
    validate_document_invariants,
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
    pub dynamic_field: Option<InlineKind>,
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
    dynamic_field: Option<InlineKind>,
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
    /// Validate and lay out a temporarily public normalized document.
    pub fn try_layout(document: &Document) -> Result<LayoutDocument, ModelInvariantError> {
        Self::try_layout_with_font_provider(document, None)
    }

    /// Validate and lay out a normalized document with supplied fonts.
    pub fn try_layout_with_font_provider(
        document: &Document,
        font_provider: Option<&FontProvider>,
    ) -> Result<LayoutDocument, ModelInvariantError> {
        validate_document_invariants(document)?;
        Ok(Self::layout_with_font_provider(document, font_provider))
    }

    /// Internal compatibility helper for already validated parser output.
    #[cfg(test)]
    pub(crate) fn layout(document: &Document) -> LayoutDocument {
        Self::layout_with_font_provider(document, None)
    }

    /// Internal compatibility helper for already validated parser output.
    pub(crate) fn layout_with_font_provider(
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
                            dynamic_field: None,
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
                    if document
                        .inline_images
                        .iter()
                        .any(|entry| entry.body_block_index.get() == block_idx)
                    {
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
#[cfg(test)]
mod tests {
    use crate::fonts::{FontAsset, FontAssetStyle, FontProviderLimits};
    use crate::model::{
        Block, Color, Document, FontDef, FontFamilyHint, ImageCrop, ImageFormat, MathGroupId,
        PageNumberFormat, PageSettings, Paragraph, Run, StaticImagePlacement, StaticShape,
        StaticShapeKind, StaticShapePoint, Table, TableCell, TableCellBorder, TableCellBorders,
        TableCellPadding, TableCellSpacing, TableCellVerticalMerge, TableRow, TableRowWrapMargins,
    };
    use crate::rtf::parse_rtf;

    use super::*;

    const NESTED_TABLE_ANCHOR_MARKER: &str = "\u{f0010}";

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
            dynamic_field: None,
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
                                dynamic_field: None,
                                text: "First".to_string(),
                                style: Default::default(),
                            }],
                        },
                        Paragraph {
                            style: second_style,
                            runs: vec![Run {
                                dynamic_field: None,
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
                dynamic_field: None,
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
                    dynamic_field: None,
                    text: "First".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::Paragraph(Paragraph {
                style: second_style,
                runs: vec![Run {
                    dynamic_field: None,
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
                dynamic_field: None,
                text: text.to_string(),
                style: Default::default(),
            }],
        })
    }

    fn repeating_paragraph(text: &str) -> Paragraph {
        Paragraph {
            style: Default::default(),
            runs: vec![Run {
                dynamic_field: None,
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
