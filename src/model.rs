pub const PAGE_NUMBER_MARKER: &str = "\u{f0000}";
pub const TOTAL_PAGES_MARKER: &str = "\u{f0001}";
pub const SECTION_NUMBER_MARKER: &str = "\u{f0002}";
pub const SECTION_PAGES_MARKER: &str = "\u{f0004}";
pub const BOOKMARK_PAGE_ANCHOR_MARKER: &str = "\u{f0005}";
pub const BOOKMARK_PAGE_REF_MARKER: &str = "\u{f0006}";
pub const BOOKMARK_PAGE_MARKER_END: &str = "\u{f0007}";
pub const DOCUMENT_WORDS_MARKER: &str = "\u{f0008}";
pub const DOCUMENT_CHARS_MARKER: &str = "\u{f0009}";
pub const DOCUMENT_CHARS_WITH_SPACES_MARKER: &str = "\u{f000a}";
pub const PASSIVE_ADVANCE_MARKER: &str = "\u{f000b}";
pub const PASSIVE_MATH_FRACTION_RULE_MARKER: &str = "\u{f0011}";
pub const PASSIVE_MATH_STACK_ANCHOR_MARKER: &str = "\u{f0012}";
pub const NESTED_TABLE_ANCHOR_MARKER: &str = "\u{f0010}";
pub const INLINE_IMAGE_MARKER_BASE: u32 = 0xF1000;
pub const FOOTNOTE_REFERENCE_MARKER: &str = "\u{f000c}";
pub const FOOTNOTE_REFERENCE_MARKER_END: &str = "\u{f000d}";
pub const ENDNOTE_REFERENCE_MARKER: &str = "\u{f000e}";
pub const ENDNOTE_REFERENCE_MARKER_END: &str = "\u{f000f}";

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub page: PageSettings,
    pub default_tab_width_twips: i32,
    pub footnote_number_start: i32,
    pub endnote_number_start: i32,
    pub footnote_number_format: PageNumberFormat,
    pub endnote_number_format: PageNumberFormat,
    pub footnote_number_restart: NoteNumberRestart,
    pub endnote_number_restart: NoteNumberRestart,
    pub footnote_numbering: Vec<NoteNumbering>,
    pub endnote_numbering: Vec<NoteNumbering>,
    pub footnote_placement: FootnotePlacement,
    pub endnote_placement: EndnotePlacement,
    pub fonts: Vec<FontDef>,
    pub colors: Vec<Color>,
    pub header: Vec<Paragraph>,
    pub first_page_header: Vec<Paragraph>,
    pub even_page_header: Vec<Paragraph>,
    pub header_images: Vec<StaticImage>,
    pub first_page_header_images: Vec<StaticImage>,
    pub even_page_header_images: Vec<StaticImage>,
    pub header_shapes: Vec<StaticShape>,
    pub first_page_header_shapes: Vec<StaticShape>,
    pub even_page_header_shapes: Vec<StaticShape>,
    pub footer: Vec<Paragraph>,
    pub first_page_footer: Vec<Paragraph>,
    pub even_page_footer: Vec<Paragraph>,
    pub footer_images: Vec<StaticImage>,
    pub first_page_footer_images: Vec<StaticImage>,
    pub even_page_footer_images: Vec<StaticImage>,
    pub footer_shapes: Vec<StaticShape>,
    pub first_page_footer_shapes: Vec<StaticShape>,
    pub even_page_footer_shapes: Vec<StaticShape>,
    pub background_shapes: Vec<StaticShape>,
    pub footnote_separator: Option<Paragraph>,
    pub footnote_continuation_separator: Option<Paragraph>,
    pub endnote_separator: Option<Paragraph>,
    pub endnote_continuation_separator: Option<Paragraph>,
    pub footnotes: Vec<Paragraph>,
    pub footnote_section_indices: Vec<usize>,
    pub footnote_block_indices: Vec<usize>,
    pub endnotes: Vec<Paragraph>,
    pub endnote_section_indices: Vec<usize>,
    pub endnote_placements: Vec<EndnotePlacement>,
    pub inline_images: Vec<StaticImage>,
    pub inline_image_block_indices: Vec<usize>,
    pub blocks: Vec<Block>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            page: PageSettings::default(),
            default_tab_width_twips: 720,
            footnote_number_start: 1,
            endnote_number_start: 1,
            footnote_number_format: PageNumberFormat::Decimal,
            endnote_number_format: PageNumberFormat::Decimal,
            footnote_number_restart: NoteNumberRestart::Continuous,
            endnote_number_restart: NoteNumberRestart::Continuous,
            footnote_numbering: Vec::new(),
            endnote_numbering: Vec::new(),
            footnote_placement: FootnotePlacement::BeneathText,
            endnote_placement: EndnotePlacement::AfterBody,
            fonts: vec![FontDef {
                index: 0,
                name: "Helvetica".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            }],
            colors: vec![Color::default()],
            header: Vec::new(),
            first_page_header: Vec::new(),
            even_page_header: Vec::new(),
            header_images: Vec::new(),
            first_page_header_images: Vec::new(),
            even_page_header_images: Vec::new(),
            header_shapes: Vec::new(),
            first_page_header_shapes: Vec::new(),
            even_page_header_shapes: Vec::new(),
            footer: Vec::new(),
            first_page_footer: Vec::new(),
            even_page_footer: Vec::new(),
            footer_images: Vec::new(),
            first_page_footer_images: Vec::new(),
            even_page_footer_images: Vec::new(),
            footer_shapes: Vec::new(),
            first_page_footer_shapes: Vec::new(),
            even_page_footer_shapes: Vec::new(),
            background_shapes: Vec::new(),
            footnote_separator: None,
            footnote_continuation_separator: None,
            endnote_separator: None,
            endnote_continuation_separator: None,
            footnotes: Vec::new(),
            footnote_section_indices: Vec::new(),
            footnote_block_indices: Vec::new(),
            endnotes: Vec::new(),
            endnote_section_indices: Vec::new(),
            endnote_placements: Vec::new(),
            inline_images: Vec::new(),
            inline_image_block_indices: Vec::new(),
            blocks: vec![Block::Paragraph(Paragraph::default())],
        }
    }
}

pub fn inline_image_marker(index: usize) -> Option<char> {
    let codepoint = INLINE_IMAGE_MARKER_BASE.checked_add(u32::try_from(index).ok()?)?;
    char::from_u32(codepoint).filter(|ch| *ch <= '\u{ffffd}')
}

pub fn inline_image_marker_index(text: &str) -> Option<usize> {
    let mut chars = text.chars();
    let codepoint = chars.next()? as u32;
    if chars.next().is_some() || codepoint < INLINE_IMAGE_MARKER_BASE {
        return None;
    }
    usize::try_from(codepoint - INLINE_IMAGE_MARKER_BASE).ok()
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageSettings {
    pub width_twips: i32,
    pub height_twips: i32,
    pub margin_left_twips: i32,
    pub margin_right_twips: i32,
    pub margin_top_twips: i32,
    pub margin_bottom_twips: i32,
    pub gutter_twips: i32,
    pub facing_pages: bool,
    pub mirror_margins: bool,
    pub gutter_on_right: bool,
    pub gutter_at_top: bool,
    pub header_distance_twips: i32,
    pub footer_distance_twips: i32,
    pub landscape: bool,
    pub column_count: usize,
    pub column_gap_twips: i32,
    pub column_widths_twips: Vec<i32>,
    pub column_gaps_twips: Vec<i32>,
    pub line_between_columns: bool,
    pub title_page: bool,
    pub vertical_alignment: PageVerticalAlignment,
    pub page_number_start: Option<i32>,
    pub restart_page_numbering: bool,
    pub page_number_format: Option<PageNumberFormat>,
    pub page_number_x_twips: Option<i32>,
    pub page_number_y_twips: Option<i32>,
    pub line_numbering: LineNumbering,
    pub text_line_grid_twips: Option<i32>,
    pub page_borders: ParagraphBorders,
    pub page_border_spacing_twips: PageBorderSpacing,
    pub page_border_from_page_edge: bool,
    pub page_border_includes_header: bool,
    pub page_border_includes_footer: bool,
    pub header: Vec<Paragraph>,
    pub first_page_header: Vec<Paragraph>,
    pub even_page_header: Vec<Paragraph>,
    pub header_images: Vec<StaticImage>,
    pub first_page_header_images: Vec<StaticImage>,
    pub even_page_header_images: Vec<StaticImage>,
    pub header_shapes: Vec<StaticShape>,
    pub first_page_header_shapes: Vec<StaticShape>,
    pub even_page_header_shapes: Vec<StaticShape>,
    pub footer: Vec<Paragraph>,
    pub first_page_footer: Vec<Paragraph>,
    pub even_page_footer: Vec<Paragraph>,
    pub footer_images: Vec<StaticImage>,
    pub first_page_footer_images: Vec<StaticImage>,
    pub even_page_footer_images: Vec<StaticImage>,
    pub footer_shapes: Vec<StaticShape>,
    pub first_page_footer_shapes: Vec<StaticShape>,
    pub even_page_footer_shapes: Vec<StaticShape>,
    pub background_shapes: Vec<StaticShape>,
}

impl Default for PageSettings {
    fn default() -> Self {
        Self {
            width_twips: 12_240,
            height_twips: 15_840,
            margin_left_twips: 1_440,
            margin_right_twips: 1_440,
            margin_top_twips: 1_440,
            margin_bottom_twips: 1_440,
            gutter_twips: 0,
            facing_pages: false,
            mirror_margins: false,
            gutter_on_right: false,
            gutter_at_top: false,
            header_distance_twips: 720,
            footer_distance_twips: 720,
            landscape: false,
            column_count: 1,
            column_gap_twips: 720,
            column_widths_twips: Vec::new(),
            column_gaps_twips: Vec::new(),
            line_between_columns: false,
            title_page: false,
            vertical_alignment: PageVerticalAlignment::Top,
            page_number_start: None,
            restart_page_numbering: false,
            page_number_format: None,
            page_number_x_twips: None,
            page_number_y_twips: None,
            line_numbering: LineNumbering::default(),
            text_line_grid_twips: None,
            page_borders: ParagraphBorders::default(),
            page_border_spacing_twips: PageBorderSpacing::default(),
            page_border_from_page_edge: false,
            page_border_includes_header: false,
            page_border_includes_footer: false,
            header: Vec::new(),
            first_page_header: Vec::new(),
            even_page_header: Vec::new(),
            header_images: Vec::new(),
            first_page_header_images: Vec::new(),
            even_page_header_images: Vec::new(),
            header_shapes: Vec::new(),
            first_page_header_shapes: Vec::new(),
            even_page_header_shapes: Vec::new(),
            footer: Vec::new(),
            first_page_footer: Vec::new(),
            even_page_footer: Vec::new(),
            footer_images: Vec::new(),
            first_page_footer_images: Vec::new(),
            even_page_footer_images: Vec::new(),
            footer_shapes: Vec::new(),
            first_page_footer_shapes: Vec::new(),
            even_page_footer_shapes: Vec::new(),
            background_shapes: Vec::new(),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct LineNumbering {
    pub enabled: bool,
    pub start: i32,
    pub step: i32,
    pub distance_twips: i32,
    pub restart: LineNumberRestart,
}

impl Default for LineNumbering {
    fn default() -> Self {
        Self {
            enabled: false,
            start: 1,
            step: 1,
            distance_twips: 360,
            restart: LineNumberRestart::Continuous,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum LineNumberRestart {
    #[default]
    Continuous,
    Page,
    Section,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum FootnotePlacement {
    #[default]
    BeneathText,
    BottomOfPage,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum EndnotePlacement {
    #[default]
    AfterBody,
    EndOfDocument,
    EndOfSection,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum NoteNumberRestart {
    #[default]
    Continuous,
    EachSection,
    EachPage,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum PageVerticalAlignment {
    #[default]
    Top,
    Center,
    Bottom,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct PageBorderSpacing {
    pub left_twips: i32,
    pub right_twips: i32,
    pub top_twips: i32,
    pub bottom_twips: i32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PageNumberFormat {
    Decimal,
    UpperRoman,
    LowerRoman,
    UpperLetter,
    LowerLetter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontDef {
    pub index: i32,
    pub name: String,
    pub alternate_name: Option<String>,
    pub charset: Option<i32>,
    pub code_page: Option<i32>,
    pub family: FontFamilyHint,
    pub pitch: FontPitch,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum FontFamilyHint {
    #[default]
    Nil,
    Roman,
    Swiss,
    Modern,
    Script,
    Decor,
    Tech,
    Bidi,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum FontPitch {
    #[default]
    Default,
    Fixed,
    Variable,
}

impl FontPitch {
    pub fn from_rtf_parameter(value: i32) -> Self {
        match value {
            1 => Self::Fixed,
            2 => Self::Variable,
            _ => Self::Default,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Paragraph(Paragraph),
    Table(Table),
    Image(StaticImage),
    Shape(StaticShape),
    Placeholder(String),
    PageBreak,
    ColumnBreak,
    ContinuousSectionBreak,
    SectionBreak,
    EvenPageSectionBreak,
    OddPageSectionBreak,
    SectionSettings(PageSettings),
}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticImage {
    pub format: ImageFormat,
    pub bytes: Vec<u8>,
    pub palette: Vec<u8>,
    pub alpha_mask: Option<StaticImageAlphaMask>,
    pub tone_adjustment: Option<ImageToneAdjustment>,
    pub vector_commands: Vec<StaticImageVectorCommand>,
    pub width_px: u32,
    pub height_px: u32,
    pub natural_width_px_hint: Option<u32>,
    pub natural_height_px_hint: Option<u32>,
    pub display_width_twips: Option<i32>,
    pub display_height_twips: Option<i32>,
    pub scale_x_percent: Option<i32>,
    pub scale_y_percent: Option<i32>,
    pub crop: ImageCrop,
    pub placement: Option<StaticImagePlacement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticImageAlphaMask {
    pub bytes: Vec<u8>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ImageToneAdjustment {
    pub decode_low: f32,
    pub decode_high: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticImageVectorRaster {
    pub format: ImageFormat,
    pub width_px: u32,
    pub height_px: u32,
    pub bytes: Vec<u8>,
    pub palette: Vec<u8>,
    pub alpha_mask: Option<StaticImageAlphaMask>,
    pub tone_adjustment: Option<ImageToneAdjustment>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct ImageCrop {
    pub left_twips: i32,
    pub top_twips: i32,
    pub right_twips: i32,
    pub bottom_twips: i32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum StaticImageWrapSide {
    Both,
    Left,
    Right,
    Largest,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum StaticShapeHorizontalAnchor {
    Column,
    Margin,
    Page,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum StaticShapeVerticalAnchor {
    Paragraph,
    Margin,
    Page,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct StaticImagePlacement {
    pub left_twips: i32,
    pub top_twips: i32,
    pub width_twips: i32,
    pub height_twips: i32,
    pub z_order: i32,
    pub below_text: bool,
    pub text_wrap: bool,
    pub wrap_side: StaticImageWrapSide,
    pub wrap_margin_left_twips: i32,
    pub wrap_margin_right_twips: i32,
    pub wrap_margin_top_twips: i32,
    pub wrap_margin_bottom_twips: i32,
    pub horizontal_anchor: StaticShapeHorizontalAnchor,
    pub vertical_anchor: StaticShapeVerticalAnchor,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    JpegInverted,
    JpegPassiveGrayscale,
    JpegPassiveBilevel,
    JpegGrayscale,
    JpegGrayscaleInverted,
    JpegCmyk,
    JpegCmykInverted,
    JpegCmykPassiveGrayscale,
    JpegCmykPassiveBilevel,
    Png,
    PngGrayscale,
    PngIndexed,
    Rgb8,
    WmfVector,
    Placeholder,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StaticImageVectorCommand {
    SaveState,
    RestoreState,
    SetMiterLimit {
        limit: f32,
    },
    ClipRect {
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
    },
    ClipPath {
        start: (f32, f32),
        segments: Vec<StaticImageVectorPathSegment>,
        closed: bool,
        fill_rule: StaticImageVectorFillRule,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        stroke_color: Option<Color>,
        stroke_width: f32,
        stroke_style: BorderStyle,
    },
    Polyline {
        points: Vec<(f32, f32)>,
        stroke_color: Option<Color>,
        stroke_width: f32,
        stroke_style: BorderStyle,
    },
    Bezier {
        points: Vec<(f32, f32)>,
        stroke_color: Option<Color>,
        stroke_width: f32,
        stroke_style: BorderStyle,
    },
    Polygon {
        points: Vec<(f32, f32)>,
        stroke_color: Option<Color>,
        stroke_width: f32,
        stroke_style: BorderStyle,
        fill_rule: StaticImageVectorFillRule,
        fill_pattern: ShadingPattern,
        fill_color: Option<Color>,
    },
    Path {
        start: (f32, f32),
        segments: Vec<StaticImageVectorPathSegment>,
        closed: bool,
        stroke_color: Option<Color>,
        stroke_width: f32,
        stroke_style: BorderStyle,
        fill_rule: StaticImageVectorFillRule,
        fill_pattern: ShadingPattern,
        fill_color: Option<Color>,
    },
    Rectangle {
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
        stroke_color: Option<Color>,
        stroke_width: f32,
        stroke_style: BorderStyle,
        fill_pattern: ShadingPattern,
        fill_color: Option<Color>,
    },
    RoundedRectangle {
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
        corner_width: f32,
        corner_height: f32,
        stroke_color: Option<Color>,
        stroke_width: f32,
        stroke_style: BorderStyle,
        fill_pattern: ShadingPattern,
        fill_color: Option<Color>,
    },
    Ellipse {
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
        stroke_color: Option<Color>,
        stroke_width: f32,
        stroke_style: BorderStyle,
        fill_pattern: ShadingPattern,
        fill_color: Option<Color>,
    },
    RasterImage {
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
        image: StaticImageVectorRaster,
    },
    Text {
        x: f32,
        y: f32,
        height: f32,
        bold: bool,
        text: String,
        color: Option<Color>,
        background_color: Option<Color>,
        clip_bounds: Option<StaticImageVectorTextBounds>,
        character_extra: f32,
        word_extra: f32,
        horizontal_align: StaticImageTextHorizontalAlign,
        vertical_align: StaticImageTextVerticalAlign,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum StaticImageVectorPathSegment {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    CubicTo {
        control1: (f32, f32),
        control2: (f32, f32),
        end: (f32, f32),
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum StaticImageVectorFillRule {
    #[default]
    Alternate,
    Winding,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct StaticImageVectorTextBounds {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum StaticImageTextHorizontalAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum StaticImageTextVerticalAlign {
    #[default]
    Top,
    Baseline,
    Bottom,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticShape {
    pub kind: StaticShapeKind,
    pub left_twips: i32,
    pub top_twips: i32,
    pub width_twips: i32,
    pub height_twips: i32,
    pub z_order: i32,
    pub below_text: bool,
    pub text_wrap: bool,
    pub wrap_side: StaticImageWrapSide,
    pub wrap_margin_left_twips: i32,
    pub wrap_margin_right_twips: i32,
    pub wrap_margin_top_twips: i32,
    pub wrap_margin_bottom_twips: i32,
    pub horizontal_anchor: StaticShapeHorizontalAnchor,
    pub vertical_anchor: StaticShapeVerticalAnchor,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
    pub start_arrowhead: StaticShapeArrowhead,
    pub end_arrowhead: StaticShapeArrowhead,
    pub start_arrowhead_width_percent: i32,
    pub start_arrowhead_length_percent: i32,
    pub end_arrowhead_width_percent: i32,
    pub end_arrowhead_length_percent: i32,
    pub stroke_width_twips: i32,
    pub stroke_color: Color,
    pub stroke_style: BorderStyle,
    pub stroke_cap: StaticShapeLineCap,
    pub stroke_join: StaticShapeLineJoin,
    pub fill_opacity_percent: u8,
    pub stroke_opacity_percent: u8,
    pub fill_color: Option<Color>,
    pub fill_gradient_color: Option<Color>,
    pub fill_pattern: ShadingPattern,
    pub shadow_enabled: bool,
    pub shadow_color: Color,
    pub shadow_opacity_percent: u8,
    pub shadow_offset_x_twips: i32,
    pub shadow_offset_y_twips: i32,
    pub text_margin_left_twips: i32,
    pub text_margin_right_twips: i32,
    pub text_margin_top_twips: i32,
    pub text_margin_bottom_twips: i32,
    pub text_vertical_anchor: StaticShapeTextVerticalAnchor,
    pub text_horizontal_anchor_centered: bool,
    pub text: Vec<Paragraph>,
    pub points: Vec<StaticShapePoint>,
    pub point_paths: Vec<Vec<StaticShapePoint>>,
    pub overlay_paths: Vec<Vec<StaticShapePoint>>,
    pub fill_rule: StaticImageVectorFillRule,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum StaticShapeKind {
    Line,
    Rectangle,
    RoundedRectangle,
    Ellipse,
    Polyline,
    Polygon,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum StaticShapeTextVerticalAnchor {
    #[default]
    Top,
    Middle,
    Bottom,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum StaticShapeArrowhead {
    #[default]
    None,
    Open,
    Triangle,
    Diamond,
    Oval,
    Stealth,
    Block,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum StaticShapeLineCap {
    #[default]
    Flat,
    Round,
    Square,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum StaticShapeLineJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct StaticShapePoint {
    pub x_twips: i32,
    pub y_twips: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub rows: Vec<TableRow>,
    pub column_widths_twips: Vec<i32>,
    pub borders_visible: bool,
    pub preserve_authored_widths: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
    /// Bounded widths derived from this row's authored `\\cellx` boundaries.
    ///
    /// Rows may define different cell boundaries within one RTF table. Layout
    /// falls back to `Table::column_widths_twips` when this vector is empty.
    pub column_widths_twips: Vec<i32>,
    pub height_twips: Option<i32>,
    pub left_offset_twips: i32,
    pub vertical_offset_twips: i32,
    pub wrap_margins: TableRowWrapMargins,
    pub cell_gap_twips: i32,
    pub alignment: TableRowAlignment,
    pub repeat_header: bool,
    pub keep_together: bool,
    pub keep_with_next: bool,
    pub no_overlap: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct NoteNumbering {
    pub start: i32,
    pub format: PageNumberFormat,
    pub restart: NoteNumberRestart,
    pub word_compact_label: bool,
}

pub const TABLE_ROW_DYNAMIC_VERTICAL_CENTER_OFFSET_BASE: i32 = -1_000_000_000;
pub const TABLE_ROW_DYNAMIC_VERTICAL_BOTTOM_OFFSET_BASE: i32 = -1_100_000_000;
pub const TABLE_ROW_DYNAMIC_VERTICAL_OFFSET_SPAN_TWIPS: i32 = 100_000;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct TableRowWrapMargins {
    pub left_twips: i32,
    pub right_twips: i32,
    pub top_twips: i32,
    pub bottom_twips: i32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum TableRowAlignment {
    #[default]
    Left,
    Center,
    Right,
    Inside,
    Outside,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableCell {
    pub paragraphs: Vec<Paragraph>,
    /// A bounded nested table parsed from this cell, when present.
    pub nested_table: Option<Box<Table>>,
    pub shading_color_index: Option<usize>,
    pub shading_basis_points: i32,
    pub shading_pattern: ShadingPattern,
    pub padding: TableCellPadding,
    pub spacing: TableCellSpacing,
    pub borders: TableCellBorders,
    pub fit_text: bool,
    pub text_direction: TableCellTextDirection,
    pub vertical_align: TableCellVerticalAlign,
    pub horizontal_merge: TableCellHorizontalMerge,
    pub vertical_merge: TableCellVerticalMerge,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum TableCellTextDirection {
    #[default]
    LeftToRightTopToBottom,
    TopToBottomRightToLeft,
    BottomToTopLeftToRight,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TableCellBorders {
    pub left: TableCellBorder,
    pub right: TableCellBorder,
    pub top: TableCellBorder,
    pub bottom: TableCellBorder,
    pub diagonal_down: TableCellBorder,
    pub diagonal_up: TableCellBorder,
}

impl Default for TableCellBorders {
    fn default() -> Self {
        Self {
            left: TableCellBorder::default(),
            right: TableCellBorder::default(),
            top: TableCellBorder::default(),
            bottom: TableCellBorder::default(),
            diagonal_down: hidden_border(),
            diagonal_up: hidden_border(),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TableCellBorder {
    pub visible: bool,
    pub width_twips: i32,
    pub spacing_twips: i32,
    pub color_index: Option<usize>,
    pub style: BorderStyle,
}

impl Default for TableCellBorder {
    fn default() -> Self {
        Self {
            visible: true,
            width_twips: 10,
            spacing_twips: 0,
            color_index: None,
            style: BorderStyle::Single,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum BorderStyle {
    #[default]
    Single,
    Thick,
    Hairline,
    Double,
    Triple,
    Dotted,
    Dashed,
    Wavy,
    Emboss,
    Engrave,
    ThinThick,
    ThickThin,
    ThinThickThin,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct TableCellPadding {
    pub left_twips: Option<i32>,
    pub right_twips: Option<i32>,
    pub top_twips: Option<i32>,
    pub bottom_twips: Option<i32>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct TableCellSpacing {
    pub left_twips: Option<i32>,
    pub right_twips: Option<i32>,
    pub top_twips: Option<i32>,
    pub bottom_twips: Option<i32>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum TableCellVerticalAlign {
    #[default]
    Top,
    Center,
    Bottom,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum TableCellHorizontalMerge {
    #[default]
    None,
    First,
    Continuation,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum TableCellVerticalMerge {
    #[default]
    None,
    First,
    Continuation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Paragraph {
    pub style: ParagraphStyle,
    pub runs: Vec<Run>,
}

impl Default for Paragraph {
    fn default() -> Self {
        Self {
            style: ParagraphStyle::default(),
            runs: Vec::new(),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
    Justified,
    Distributed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParagraphStyle {
    pub alignment: Alignment,
    /// Safe direction hint used when a bounded nested-table row is flattened
    /// into a passive paragraph. `None` retains the containing cell's normal
    /// direction behavior.
    pub table_cell_text_direction: Option<TableCellTextDirection>,
    /// Bounded authored nested-table row height, in twips, when present.
    pub nested_table_row_height_twips: Option<i32>,
    /// Safe homogeneous nested-row vertical alignment for authored row height.
    pub nested_table_row_vertical_align: Option<TableCellVerticalAlign>,
    pub page_break_before: bool,
    pub keep_together: bool,
    pub keep_with_next: bool,
    pub widow_control: bool,
    pub no_wrap: bool,
    /// Word's bounded two-in-one character layout: visible text is rendered as
    /// two half-size horizontal rows within one ordinary paragraph line slot.
    pub two_in_one: bool,
    pub suppress_line_numbers: bool,
    pub auto_hyphenation: bool,
    pub hyphenate_caps: bool,
    pub max_consecutive_hyphenated_lines: Option<usize>,
    pub hyphenation_zone_twips: i32,
    pub drop_cap_lines: i32,
    pub left_indent_twips: i32,
    pub right_indent_twips: i32,
    pub first_line_indent_twips: i32,
    pub space_before_twips: i32,
    pub space_after_twips: i32,
    pub auto_space_before: bool,
    pub auto_space_after: bool,
    pub contextual_spacing: bool,
    pub line_spacing_twips: Option<i32>,
    pub line_spacing_multiple: bool,
    pub snap_to_line_grid: bool,
    pub shading_color_index: Option<usize>,
    pub shading_basis_points: i32,
    pub shading_pattern: ShadingPattern,
    pub tab_stops_twips: Vec<i32>,
    pub tab_stop_leaders: Vec<TabLeader>,
    pub tab_stop_alignments: Vec<TabAlignment>,
    pub borders: ParagraphBorders,
}

impl Default for ParagraphStyle {
    fn default() -> Self {
        Self {
            alignment: Alignment::Left,
            table_cell_text_direction: None,
            nested_table_row_height_twips: None,
            nested_table_row_vertical_align: None,
            page_break_before: false,
            keep_together: false,
            keep_with_next: false,
            widow_control: false,
            no_wrap: false,
            two_in_one: false,
            suppress_line_numbers: false,
            auto_hyphenation: false,
            hyphenate_caps: true,
            max_consecutive_hyphenated_lines: None,
            hyphenation_zone_twips: 360,
            drop_cap_lines: 0,
            left_indent_twips: 0,
            right_indent_twips: 0,
            first_line_indent_twips: 0,
            space_before_twips: 0,
            space_after_twips: 0,
            auto_space_before: false,
            auto_space_after: false,
            contextual_spacing: false,
            line_spacing_twips: None,
            line_spacing_multiple: false,
            snap_to_line_grid: true,
            shading_color_index: None,
            shading_basis_points: 10_000,
            shading_pattern: ShadingPattern::None,
            tab_stops_twips: Vec::new(),
            tab_stop_leaders: Vec::new(),
            tab_stop_alignments: Vec::new(),
            borders: ParagraphBorders::default(),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
enum ShadingPatternKind {
    #[default]
    None,
    Horizontal,
    Vertical,
    ForwardDiagonal,
    BackwardDiagonal,
    Cross,
    DiagonalCross,
    DarkHorizontal,
    DarkVertical,
    DarkForwardDiagonal,
    DarkBackwardDiagonal,
    DarkCross,
    DarkDiagonalCross,
    VerticalGradient,
    WordWmfBitmapHorizontal,
}

/// A passive RTF shading pattern and its independently authored foreground color.
///
/// RTF stores the pattern background (`\cbpat`, `\clcbpat`, or `\trcbpat`) and
/// foreground (`\cfpat`, `\clcfpat`, or `\trcfpat`) separately. The background
/// remains on the containing paragraph or cell while this value carries the
/// foreground so existing model consumers can continue treating pattern state as
/// one bounded, copyable value.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct ShadingPattern {
    kind: ShadingPatternKind,
    foreground_color_index: Option<usize>,
}

#[allow(non_upper_case_globals)]
impl ShadingPattern {
    pub const None: Self = Self::new(ShadingPatternKind::None);
    pub const Horizontal: Self = Self::new(ShadingPatternKind::Horizontal);
    pub const Vertical: Self = Self::new(ShadingPatternKind::Vertical);
    pub const ForwardDiagonal: Self = Self::new(ShadingPatternKind::ForwardDiagonal);
    pub const BackwardDiagonal: Self = Self::new(ShadingPatternKind::BackwardDiagonal);
    pub const Cross: Self = Self::new(ShadingPatternKind::Cross);
    pub const DiagonalCross: Self = Self::new(ShadingPatternKind::DiagonalCross);
    pub const DarkHorizontal: Self = Self::new(ShadingPatternKind::DarkHorizontal);
    pub const DarkVertical: Self = Self::new(ShadingPatternKind::DarkVertical);
    pub const DarkForwardDiagonal: Self = Self::new(ShadingPatternKind::DarkForwardDiagonal);
    pub const DarkBackwardDiagonal: Self = Self::new(ShadingPatternKind::DarkBackwardDiagonal);
    pub const DarkCross: Self = Self::new(ShadingPatternKind::DarkCross);
    pub const DarkDiagonalCross: Self = Self::new(ShadingPatternKind::DarkDiagonalCross);
    pub const VerticalGradient: Self = Self::new(ShadingPatternKind::VerticalGradient);
    /// Word-compatible playback of a legacy WMF `HS_HORIZONTAL` brush.
    ///
    /// Word rasterizes this brush as a scaled monochrome tile rather than using
    /// the document-shading line pitch. Keeping the distinction in the passive
    /// model lets the PDF backend reproduce that bounded appearance without
    /// retaining the WMF payload or embedding a raster image.
    pub const WordWmfBitmapHorizontal: Self =
        Self::new(ShadingPatternKind::WordWmfBitmapHorizontal);

    const fn new(kind: ShadingPatternKind) -> Self {
        Self {
            kind,
            foreground_color_index: None,
        }
    }

    pub fn foreground_color_index(self) -> Option<usize> {
        self.foreground_color_index
    }

    pub fn with_foreground_color_index(mut self, color_index: Option<usize>) -> Self {
        self.foreground_color_index = color_index;
        self
    }

    pub fn with_pattern_from(self, pattern: Self) -> Self {
        Self {
            kind: pattern.kind,
            foreground_color_index: self.foreground_color_index,
        }
    }

    pub fn without_foreground_color(self) -> Self {
        Self {
            kind: self.kind,
            foreground_color_index: None,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum TabAlignment {
    #[default]
    Left,
    Center,
    Right,
    Decimal,
    Bar,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum TabLeader {
    #[default]
    None,
    Dots,
    Hyphens,
    Underline,
    MiddleDots,
    Equals,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ParagraphBorders {
    pub left: TableCellBorder,
    pub right: TableCellBorder,
    pub top: TableCellBorder,
    pub bottom: TableCellBorder,
    pub between: TableCellBorder,
}

impl Default for ParagraphBorders {
    fn default() -> Self {
        Self {
            left: hidden_border(),
            right: hidden_border(),
            top: hidden_border(),
            bottom: hidden_border(),
            between: hidden_border(),
        }
    }
}

const fn hidden_border() -> TableCellBorder {
    TableCellBorder {
        visible: false,
        width_twips: 10,
        spacing_twips: 0,
        color_index: None,
        style: BorderStyle::Single,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub text: String,
    pub style: CharacterStyle,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CharacterStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: UnderlineStyle,
    pub underline_color_index: Option<usize>,
    pub overline: bool,
    pub strike: bool,
    pub double_strike: bool,
    pub outline: bool,
    pub shadow: bool,
    pub relief: TextRelief,
    pub emphasis_mark: CharacterEmphasisMark,
    pub all_caps: bool,
    pub small_caps: bool,
    pub right_to_left: bool,
    pub hidden: bool,
    pub form_field_shading: bool,
    pub passive_math: bool,
    pub passive_math_fraction_id: Option<usize>,
    pub passive_math_fraction_part: PassiveMathFractionPart,
    pub passive_math_fraction_has_bar: bool,
    pub passive_math_limit_id: Option<usize>,
    pub passive_math_limit_part: PassiveMathLimitPart,
    pub passive_math_equation_array_id: Option<usize>,
    pub passive_math_equation_array_row: usize,
    pub passive_math_nary_base_id: Option<usize>,
    pub passive_math_phantom_id: Option<usize>,
    pub baseline_shift_half_points: i32,
    pub font_size_scale_percent: i32,
    pub character_spacing_twips: i32,
    pub character_kerning_half_points: i32,
    pub character_scaling_percent: i32,
    pub fit_text_width_twips: Option<i32>,
    pub font_index: i32,
    pub font_size_half_points: i32,
    pub color_index: usize,
    pub highlight_index: Option<usize>,
    pub highlight_shading_basis_points: i32,
    pub border: TableCellBorder,
}

impl Default for CharacterStyle {
    fn default() -> Self {
        Self {
            bold: false,
            italic: false,
            underline: UnderlineStyle::None,
            underline_color_index: None,
            overline: false,
            strike: false,
            double_strike: false,
            outline: false,
            shadow: false,
            relief: TextRelief::None,
            emphasis_mark: CharacterEmphasisMark::None,
            all_caps: false,
            small_caps: false,
            right_to_left: false,
            hidden: false,
            form_field_shading: false,
            passive_math: false,
            passive_math_fraction_id: None,
            passive_math_fraction_part: PassiveMathFractionPart::None,
            passive_math_fraction_has_bar: false,
            passive_math_limit_id: None,
            passive_math_limit_part: PassiveMathLimitPart::None,
            passive_math_equation_array_id: None,
            passive_math_equation_array_row: 0,
            passive_math_nary_base_id: None,
            passive_math_phantom_id: None,
            baseline_shift_half_points: 0,
            font_size_scale_percent: 100,
            character_spacing_twips: 0,
            character_kerning_half_points: 0,
            character_scaling_percent: 100,
            fit_text_width_twips: None,
            font_index: 0,
            font_size_half_points: 24,
            color_index: 0,
            highlight_index: None,
            highlight_shading_basis_points: 10_000,
            border: TableCellBorder {
                visible: false,
                width_twips: 10,
                spacing_twips: 0,
                color_index: None,
                style: BorderStyle::Single,
            },
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum PassiveMathFractionPart {
    #[default]
    None,
    Numerator,
    Denominator,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum PassiveMathLimitPart {
    #[default]
    None,
    Base,
    Lower,
    Upper,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum TextRelief {
    #[default]
    None,
    Emboss,
    Engrave,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum CharacterEmphasisMark {
    #[default]
    None,
    Dot,
    Comma,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum UnderlineStyle {
    #[default]
    None,
    Single,
    Words,
    Double,
    Thick,
    Dotted,
    Dashed,
    Wave,
}

impl CharacterStyle {
    pub fn font_size_points(&self) -> f32 {
        let base = self.font_size_half_points.max(2) as f32 / 2.0;
        base * (self.font_size_scale_percent.max(1) as f32 / 100.0)
    }

    pub fn baseline_shift_points(&self) -> f32 {
        self.baseline_shift_half_points as f32 / 2.0
    }

    pub fn horizontal_scale(&self) -> f32 {
        self.character_scaling_percent.max(1) as f32 / 100.0
    }
}
