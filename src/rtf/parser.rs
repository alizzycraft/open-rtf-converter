use thiserror::Error;

use crate::config::{ActiveContentPolicy, RtfLimits, RtfParseOptions};
use crate::diagnostics::Diagnostic;
use crate::model::{
    Alignment, BOOKMARK_PAGE_ANCHOR_MARKER, BOOKMARK_PAGE_MARKER_END, BOOKMARK_PAGE_REF_MARKER,
    Block, BorderStyle, CharacterStyle, Color, DOCUMENT_CHARS_MARKER,
    DOCUMENT_CHARS_WITH_SPACES_MARKER, DOCUMENT_WORDS_MARKER, Document, EndnotePlacement, FontDef,
    FontFamilyHint, FontPitch, FootnotePlacement, ImageCrop, ImageFormat, LineNumberRestart,
    PAGE_NUMBER_MARKER, PageNumberFormat, PageSettings, PageVerticalAlignment, Paragraph,
    ParagraphStyle, Run, SECTION_NUMBER_MARKER, SECTION_PAGES_MARKER, ShadingPattern, StaticImage,
    StaticShape, StaticShapeKind, StaticShapePoint, TOTAL_PAGES_MARKER, TabAlignment, TabLeader,
    Table, TableCell, TableCellBorder, TableCellBorders, TableCellHorizontalMerge,
    TableCellPadding, TableCellVerticalAlign, TableCellVerticalMerge, TableRow, TableRowAlignment,
    TextRelief, UnderlineStyle,
};

use super::lexer::{Control, LexError, Lexer, Token, TokenKind};

const DEFAULT_SUPERSCRIPT_SHIFT_HALF_POINTS: i32 = 6;
const DEFAULT_SUBSCRIPT_SHIFT_HALF_POINTS: i32 = -6;
const DEFAULT_SCRIPT_FONT_SCALE_PERCENT: i32 = 65;
const MAX_BASELINE_SHIFT_HALF_POINTS: i32 = 96;
const DEFAULT_TABLE_CELL_GAP_TWIPS: i32 = 60;
const PENDING_NOTE_REFERENCE_MARKER: &str = "\u{f0003}";

#[derive(Debug, Error)]
pub enum ParseError {
    #[error(transparent)]
    Lex(#[from] LexError),
    #[error("unbalanced RTF group ending at byte {0}")]
    UnbalancedGroup(usize),
    #[error("RTF group depth limit exceeded at byte {0}")]
    GroupDepthExceeded(usize),
    #[error("RTF destination data limit exceeded at byte {0}")]
    DestinationTooLarge(usize),
    #[error("output text character limit exceeded at byte {0}")]
    OutputTextTooLarge(usize),
    #[error("resource limit exceeded for {resource} at byte {offset}")]
    ResourceLimitExceeded { resource: String, offset: usize },
    #[error("active content rejected: {feature} at byte {offset}")]
    ActiveContentRejected { feature: String, offset: usize },
}

#[derive(Debug, Clone)]
pub struct ParseOutput {
    pub document: Document,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
struct ParserState {
    character: CharacterStyle,
    paragraph: ParagraphStyle,
    style_index: Option<i32>,
    style_based_on: Option<i32>,
    style_next: Option<i32>,
    style_kind: StyleKind,
    paragraph_style_index: Option<i32>,
    list_override_index: Option<i32>,
    list_level_index: usize,
    list_context: ListContext,
    section_break_kind: SectionBreakKind,
    paragraph_border_selection: BorderSelection,
    current_tab_leader: TabLeader,
    current_tab_alignment: TabAlignment,
    code_page: CodePage,
    unicode_skip: usize,
    pending_unicode_high_surrogate: Option<u16>,
    skip_bytes: usize,
    destination: Destination,
    inside_metadata: bool,
    inside_document_info: bool,
    inside_user_properties: bool,
    inside_object: bool,
    object_owner_destination: Destination,
    object_result_seen: bool,
    inside_shape: bool,
    shape_result_seen: bool,
    inside_shape_picture: bool,
    shape_picture_rendered: bool,
    suppressing_nonshape_picture: bool,
    inside_field: bool,
    field_owner_destination: Destination,
    field_result_seen: bool,
    field_instruction: String,
    field_form_result_value: Option<i32>,
    field_form_default_result_value: Option<i32>,
    field_form_checkbox_result: Option<bool>,
    field_form_checkbox_default: Option<bool>,
    field_form_default_text: String,
    field_form_dropdown_entries: Vec<String>,
    field_form_dropdown_entry_text: String,
    bookmark_name_text: String,
    capturing_form_default_text: bool,
    capturing_form_dropdown_entry: bool,
    skip_password_hash_payload: bool,
    table_nesting_level: i32,
    unicode_alternate_branch: UnicodeAlternateBranch,
    unicode_alternate_child_count: usize,
    unicode_alternate_destination: Destination,
    metadata_property: Option<DocumentProperty>,
    metadata_property_text: String,
    metadata_timestamp: Option<DocumentTimestampKind>,
    metadata_timestamp_value: DocumentTimestamp,
    user_property_capture: Option<UserPropertyCapture>,
    user_property_capture_text: String,
    at_group_start: bool,
}

impl Default for ParserState {
    fn default() -> Self {
        Self {
            character: CharacterStyle::default(),
            paragraph: ParagraphStyle::default(),
            style_index: None,
            style_based_on: None,
            style_next: None,
            style_kind: StyleKind::Paragraph,
            paragraph_style_index: None,
            list_override_index: None,
            list_level_index: 0,
            list_context: ListContext::None,
            section_break_kind: SectionBreakKind::Page,
            paragraph_border_selection: BorderSelection::None,
            current_tab_leader: TabLeader::None,
            current_tab_alignment: TabAlignment::Left,
            code_page: CodePage::Windows1252,
            unicode_skip: 1,
            pending_unicode_high_surrogate: None,
            skip_bytes: 0,
            destination: Destination::Body,
            inside_metadata: false,
            inside_document_info: false,
            inside_user_properties: false,
            inside_object: false,
            object_owner_destination: Destination::Body,
            object_result_seen: false,
            inside_shape: false,
            shape_result_seen: false,
            inside_shape_picture: false,
            shape_picture_rendered: false,
            suppressing_nonshape_picture: false,
            inside_field: false,
            field_owner_destination: Destination::Body,
            field_result_seen: false,
            field_instruction: String::new(),
            field_form_result_value: None,
            field_form_default_result_value: None,
            field_form_checkbox_result: None,
            field_form_checkbox_default: None,
            field_form_default_text: String::new(),
            field_form_dropdown_entries: Vec::new(),
            field_form_dropdown_entry_text: String::new(),
            bookmark_name_text: String::new(),
            capturing_form_default_text: false,
            capturing_form_dropdown_entry: false,
            skip_password_hash_payload: false,
            table_nesting_level: 1,
            unicode_alternate_branch: UnicodeAlternateBranch::None,
            unicode_alternate_child_count: 0,
            unicode_alternate_destination: Destination::Body,
            metadata_property: None,
            metadata_property_text: String::new(),
            metadata_timestamp: None,
            metadata_timestamp_value: DocumentTimestamp::default(),
            user_property_capture: None,
            user_property_capture_text: String::new(),
            at_group_start: false,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Destination {
    Body,
    FontTable,
    FontAlternate,
    ColorTable,
    StyleSheet,
    ListTable,
    ListOverrideTable,
    Header,
    FirstPageHeader,
    EvenPageHeader,
    Footer,
    FirstPageFooter,
    EvenPageFooter,
    Footnote,
    Endnote,
    ListText,
    Picture,
    Shape,
    Ignored,
    Metadata,
    ObjectData,
    FieldInstruction,
    BookmarkStart,
    BookmarkEnd,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum UnicodeAlternateBranch {
    None,
    Container,
    AnsiFallback,
    UnicodeDestination,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum CodePage {
    Windows1252,
    MacRoman,
    Ibm437,
    Ibm850,
    Unsupported,
}

impl CodePage {
    fn from_rtf_code_page(code_page: i32) -> Option<Self> {
        match code_page {
            1252 => Some(Self::Windows1252),
            437 => Some(Self::Ibm437),
            850 => Some(Self::Ibm850),
            10000 => Some(Self::MacRoman),
            _ => None,
        }
    }

    fn from_font_charset(charset: i32) -> Option<Self> {
        match charset {
            0 => Some(Self::Windows1252),
            77 => Some(Self::MacRoman),
            255 => Some(Self::Ibm437),
            _ => None,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum ListContext {
    None,
    List,
    ListLevel,
    ListLevelText,
    ListOverride,
    ListOverrideLevel,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
enum BorderSelection {
    #[default]
    None,
    Character,
    Paragraph(TableCellBorderSide),
    ParagraphBetween,
    ParagraphBox,
    Page(TableCellBorderSide),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum SectionBreakKind {
    Continuous,
    Page,
    EvenPage,
    OddPage,
    Column,
}

#[derive(Debug)]
struct TableBuilder {
    rows: Vec<TableRow>,
    cell_right_edges_twips: Vec<i32>,
    borders_visible: bool,
}

impl Default for TableBuilder {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            cell_right_edges_twips: Vec::new(),
            borders_visible: true,
        }
    }
}

impl TableBuilder {
    fn merge_cell_right_edges(&mut self, edges: &[i32]) {
        if edges.len() > self.cell_right_edges_twips.len() {
            self.cell_right_edges_twips = edges.to_vec();
        }
    }

    fn column_widths_twips(&self) -> Vec<i32> {
        let mut widths = Vec::new();
        let mut previous = 0;
        for edge in &self.cell_right_edges_twips {
            let width = (*edge - previous).max(1);
            widths.push(width);
            previous = *edge;
        }
        widths
    }
}

#[derive(Debug)]
struct TableRowBuilder {
    cells: Vec<TableCell>,
    cell_right_edges_twips: Vec<i32>,
    cell_shading_color_indices: Vec<Option<usize>>,
    cell_shading_basis_points: Vec<i32>,
    cell_shading_patterns: Vec<ShadingPattern>,
    cell_paddings: Vec<TableCellPadding>,
    cell_borders: Vec<TableCellBorders>,
    cell_border_flags: Vec<TableCellBorderFlags>,
    cell_preferred_widths_twips: Vec<Option<i32>>,
    cell_no_wraps: Vec<bool>,
    cell_text_directions: Vec<TableCellTextDirection>,
    cell_vertical_alignments: Vec<TableCellVerticalAlign>,
    cell_horizontal_merges: Vec<TableCellHorizontalMerge>,
    cell_vertical_merges: Vec<TableCellVerticalMerge>,
    default_cell_shading_color_index: Option<usize>,
    current_cell_shading_color_index: Option<usize>,
    default_cell_shading_basis_points: i32,
    current_cell_shading_basis_points: i32,
    default_cell_shading_pattern: ShadingPattern,
    current_cell_shading_pattern: ShadingPattern,
    default_cell_padding: TableCellPadding,
    current_cell_padding: TableCellPadding,
    current_cell_borders: TableCellBorders,
    current_cell_border_flags: TableCellBorderFlags,
    current_cell_border_side: Option<TableCellBorderSide>,
    current_cell_preferred_width: PreferredTableWidth,
    current_cell_no_wrap: bool,
    current_cell_text_direction: TableCellTextDirection,
    row_borders: TableCellBorders,
    row_border_flags: TableCellBorderFlags,
    current_row_border_side: Option<TableCellBorderSide>,
    current_cell_vertical_align: TableCellVerticalAlign,
    current_cell_horizontal_merge: TableCellHorizontalMerge,
    current_cell_vertical_merge: TableCellVerticalMerge,
    height_twips: Option<i32>,
    left_offset_twips: i32,
    cell_gap_twips: i32,
    alignment: TableRowAlignment,
    repeat_header: bool,
    keep_together: bool,
    right_to_left: bool,
    current_cell_paragraphs: Vec<Paragraph>,
    current_cell_paragraph: Paragraph,
    cell_open: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct PreferredTableWidth {
    unit: PreferredTableWidthUnit,
    value: Option<i32>,
}

impl Default for PreferredTableWidth {
    fn default() -> Self {
        Self {
            unit: PreferredTableWidthUnit::Auto,
            value: None,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
enum PreferredTableWidthUnit {
    #[default]
    Auto,
    FiftiethsPercent,
    Twips,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
enum TableCellTextDirection {
    #[default]
    LeftToRightTopToBottom,
    TopToBottomRightToLeft,
    BottomToTopLeftToRight,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum TableCellBorderSide {
    Left,
    Right,
    Top,
    Bottom,
    DiagonalDown,
    DiagonalUp,
}

#[derive(Debug, Copy, Clone, Default)]
struct TableCellBorderFlags {
    left: bool,
    right: bool,
    top: bool,
    bottom: bool,
    diagonal_down: bool,
    diagonal_up: bool,
}

impl TableCellBorderFlags {
    fn is_set(self, side: TableCellBorderSide) -> bool {
        match side {
            TableCellBorderSide::Left => self.left,
            TableCellBorderSide::Right => self.right,
            TableCellBorderSide::Top => self.top,
            TableCellBorderSide::Bottom => self.bottom,
            TableCellBorderSide::DiagonalDown => self.diagonal_down,
            TableCellBorderSide::DiagonalUp => self.diagonal_up,
        }
    }

    fn set(&mut self, side: TableCellBorderSide) {
        match side {
            TableCellBorderSide::Left => self.left = true,
            TableCellBorderSide::Right => self.right = true,
            TableCellBorderSide::Top => self.top = true,
            TableCellBorderSide::Bottom => self.bottom = true,
            TableCellBorderSide::DiagonalDown => self.diagonal_down = true,
            TableCellBorderSide::DiagonalUp => self.diagonal_up = true,
        }
    }
}

#[derive(Debug)]
struct PictureBuilder {
    kind: PictureKind,
    owner_destination: Destination,
    bytes: Vec<u8>,
    pending_hex: Option<u8>,
    width_px_hint: Option<u32>,
    height_px_hint: Option<u32>,
    display_width_twips: Option<i32>,
    display_height_twips: Option<i32>,
    scale_x_percent: Option<i32>,
    scale_y_percent: Option<i32>,
    crop: ImageCrop,
}

impl Default for PictureBuilder {
    fn default() -> Self {
        Self {
            kind: PictureKind::Unknown,
            owner_destination: Destination::Body,
            bytes: Vec::new(),
            pending_hex: None,
            width_px_hint: None,
            height_px_hint: None,
            display_width_twips: None,
            display_height_twips: None,
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
        }
    }
}

#[derive(Debug, Clone)]
struct ShapeBuilder {
    owner_destination: Destination,
    kind: Option<StaticShapeKind>,
    rounded_rectangle: bool,
    base_x_twips: i32,
    base_y_twips: i32,
    left_twips: i32,
    top_twips: i32,
    width_twips: i32,
    height_twips: i32,
    stroke_width_twips: i32,
    stroke_color: Color,
    stroke_style: BorderStyle,
    fill_color: Option<Color>,
    points: Vec<StaticShapePoint>,
    pending_point_x_twips: Option<i32>,
}

impl Default for ShapeBuilder {
    fn default() -> Self {
        Self {
            owner_destination: Destination::Body,
            kind: None,
            rounded_rectangle: false,
            base_x_twips: 0,
            base_y_twips: 0,
            left_twips: 0,
            top_twips: 0,
            width_twips: 0,
            height_twips: 0,
            stroke_width_twips: 15,
            stroke_color: Color::default(),
            stroke_style: BorderStyle::Single,
            fill_color: None,
            points: Vec::new(),
            pending_point_x_twips: None,
        }
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
enum PictureKind {
    #[default]
    Unknown,
    Png,
    Jpeg,
    Wmf,
    Emf,
    Dib,
    Unsupported,
}

#[derive(Debug, Clone)]
struct ListDefinition {
    list_id: i32,
    levels: Vec<ListLevelDefinition>,
}

#[derive(Debug, Clone)]
struct ListLevelDefinition {
    format: ListNumberFormat,
    start_at: i32,
    text_template: String,
    indent_twips: Option<i32>,
    space_twips: Option<i32>,
    follow: ListLevelFollow,
    legal_numbering: bool,
    no_restart: bool,
    character_style: CharacterStyle,
    has_character_style: bool,
}

impl Default for ListLevelDefinition {
    fn default() -> Self {
        Self {
            format: ListNumberFormat::Decimal,
            start_at: 1,
            text_template: String::new(),
            indent_twips: None,
            space_twips: None,
            follow: ListLevelFollow::Tab,
            legal_numbering: false,
            no_restart: false,
            character_style: CharacterStyle::default(),
            has_character_style: false,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
enum ListLevelFollow {
    #[default]
    Tab,
    Space,
    Nothing,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum ListNumberFormat {
    Decimal,
    UpperRoman,
    LowerRoman,
    UpperLetter,
    LowerLetter,
    Ordinal,
    DecimalLeadingZero(usize),
    Bullet,
    Other,
}

#[derive(Debug, Clone)]
struct ListOverride {
    list_id: i32,
    override_index: i32,
    level_overrides: Vec<ListLevelOverride>,
}

#[derive(Debug, Clone)]
struct ListLevelOverride {
    level_index: usize,
    start_at: Option<i32>,
    restart_enabled: bool,
    format_override_enabled: bool,
    level_definition: Option<ListLevelDefinition>,
}

#[derive(Debug, Clone)]
struct ListCounter {
    override_index: i32,
    level_index: usize,
    value: i32,
}

#[derive(Debug, Clone)]
struct OldStyleListMarker {
    format: ListNumberFormat,
    start_at: i32,
    indent_twips: Option<i32>,
    hanging: bool,
    space_twips: Option<i32>,
    character_style: CharacterStyle,
    has_character_style: bool,
}

impl Default for OldStyleListMarker {
    fn default() -> Self {
        Self {
            format: ListNumberFormat::Decimal,
            start_at: 1,
            indent_twips: None,
            hanging: false,
            space_twips: None,
            character_style: CharacterStyle::default(),
            has_character_style: false,
        }
    }
}

#[derive(Debug, Clone)]
struct PendingListMarker {
    text: String,
    character_style: Option<CharacterStyle>,
    runs: Vec<Run>,
}

#[derive(Debug, Clone)]
struct FieldSequenceCounter {
    name: String,
    value: i32,
}

#[derive(Debug, Clone)]
struct BookmarkCapture {
    id: usize,
    name: String,
    text: String,
    active: bool,
}

#[derive(Debug, Clone)]
struct StyleDefinition {
    index: i32,
    based_on: Option<i32>,
    next_style: Option<i32>,
    kind: StyleKind,
    paragraph: ParagraphStyle,
    character: CharacterStyle,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum StyleKind {
    Paragraph,
    Character,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum DocumentProperty {
    Title,
    Subject,
    Author,
    Keywords,
    Comments,
    Operator,
    Manager,
    Company,
    Category,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum UserPropertyCapture {
    Name,
    Value,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum DocumentTimestampKind {
    Created,
    Saved,
    Printed,
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
struct DocumentTimestamp {
    year: Option<i32>,
    month: Option<i32>,
    day: Option<i32>,
    hour: Option<i32>,
    minute: Option<i32>,
    second: Option<i32>,
}

struct Parser {
    tokens: Vec<Token>,
    document: Document,
    default_font_index: i32,
    form_field_shading: bool,
    default_paragraph_style: ParagraphStyle,
    current_paragraph: Paragraph,
    current_header_paragraph: Paragraph,
    current_first_page_header_paragraph: Paragraph,
    current_even_page_header_paragraph: Paragraph,
    current_footer_paragraph: Paragraph,
    current_first_page_footer_paragraph: Paragraph,
    current_even_page_footer_paragraph: Paragraph,
    current_footnote_paragraph: Paragraph,
    current_endnote_paragraph: Paragraph,
    state: ParserState,
    stack: Vec<ParserState>,
    diagnostics: Vec<Diagnostic>,
    current_font: Option<FontDef>,
    current_color: Color,
    current_color_seen: bool,
    current_table: Option<TableBuilder>,
    current_table_row: Option<TableRowBuilder>,
    table_cell_count: usize,
    pending_list_marker: String,
    pending_list_marker_runs: Vec<Run>,
    current_picture: Option<PictureBuilder>,
    current_shape: Option<ShapeBuilder>,
    image_count: usize,
    shape_count: usize,
    list_definitions: Vec<ListDefinition>,
    list_overrides: Vec<ListOverride>,
    current_list: Option<ListDefinition>,
    current_list_level: Option<ListLevelDefinition>,
    current_list_override: Option<ListOverride>,
    current_list_override_level: Option<ListLevelOverride>,
    list_counters: Vec<ListCounter>,
    pending_old_style_list_marker: Option<OldStyleListMarker>,
    field_sequence_counters: Vec<FieldSequenceCounter>,
    field_auto_number_counter: i32,
    field_list_number_counters: Vec<FieldSequenceCounter>,
    document_properties: Vec<(DocumentProperty, String)>,
    custom_document_properties: Vec<(String, String)>,
    pending_custom_property_name: Option<String>,
    document_timestamps: Vec<(DocumentTimestampKind, DocumentTimestamp)>,
    document_edit_minutes: Option<i32>,
    bookmark_captures: Vec<BookmarkCapture>,
    next_bookmark_marker_id: usize,
    styles: Vec<StyleDefinition>,
    current_section_page: PageSettings,
    current_section_column_index: usize,
    footnote_reference_count: usize,
    endnote_reference_count: usize,
    options: RtfParseOptions,
    skipped_destination_bytes: usize,
    output_text_chars: usize,
    last_offset: usize,
}

pub fn parse_rtf(input: &str) -> Result<ParseOutput, ParseError> {
    parse_rtf_bytes(input.as_bytes())
}

pub fn parse_rtf_bytes(input: &[u8]) -> Result<ParseOutput, ParseError> {
    parse_rtf_bytes_with_options(input, &RtfParseOptions::default())
}

pub fn parse_rtf_bytes_with_options(
    input: &[u8],
    options: &RtfParseOptions,
) -> Result<ParseOutput, ParseError> {
    let tokens = Lexer::new(input, options.limits.clone()).tokenize()?;
    Parser::new(tokens, options.clone()).parse()
}

impl Parser {
    fn new(tokens: Vec<Token>, options: RtfParseOptions) -> Self {
        let state = ParserState::default();
        Self {
            tokens,
            document: Document::default(),
            default_font_index: 0,
            form_field_shading: false,
            default_paragraph_style: state.paragraph.clone(),
            current_paragraph: Paragraph {
                style: state.paragraph.clone(),
                runs: Vec::new(),
            },
            current_header_paragraph: Paragraph {
                style: state.paragraph.clone(),
                runs: Vec::new(),
            },
            current_first_page_header_paragraph: Paragraph {
                style: state.paragraph.clone(),
                runs: Vec::new(),
            },
            current_even_page_header_paragraph: Paragraph {
                style: state.paragraph.clone(),
                runs: Vec::new(),
            },
            current_footer_paragraph: Paragraph {
                style: state.paragraph.clone(),
                runs: Vec::new(),
            },
            current_first_page_footer_paragraph: Paragraph {
                style: state.paragraph.clone(),
                runs: Vec::new(),
            },
            current_even_page_footer_paragraph: Paragraph {
                style: state.paragraph.clone(),
                runs: Vec::new(),
            },
            current_footnote_paragraph: Paragraph {
                style: state.paragraph.clone(),
                runs: Vec::new(),
            },
            current_endnote_paragraph: Paragraph {
                style: state.paragraph.clone(),
                runs: Vec::new(),
            },
            state,
            stack: Vec::new(),
            diagnostics: Vec::new(),
            current_font: None,
            current_color: Color::default(),
            current_color_seen: false,
            current_table: None,
            current_table_row: None,
            table_cell_count: 0,
            pending_list_marker: String::new(),
            pending_list_marker_runs: Vec::new(),
            current_picture: None,
            current_shape: None,
            image_count: 0,
            shape_count: 0,
            list_definitions: Vec::new(),
            list_overrides: Vec::new(),
            current_list: None,
            current_list_level: None,
            current_list_override: None,
            current_list_override_level: None,
            list_counters: Vec::new(),
            pending_old_style_list_marker: None,
            field_sequence_counters: Vec::new(),
            field_auto_number_counter: 0,
            field_list_number_counters: Vec::new(),
            document_properties: Vec::new(),
            custom_document_properties: Vec::new(),
            pending_custom_property_name: None,
            document_timestamps: Vec::new(),
            document_edit_minutes: None,
            bookmark_captures: Vec::new(),
            next_bookmark_marker_id: 1,
            styles: Vec::new(),
            current_section_page: Document::default().page,
            current_section_column_index: 0,
            footnote_reference_count: 0,
            endnote_reference_count: 0,
            options,
            skipped_destination_bytes: 0,
            output_text_chars: 0,
            last_offset: 0,
        }
    }

    fn is_parsing_list_level_definition(&self) -> bool {
        matches!(
            self.state.destination,
            Destination::ListTable | Destination::ListOverrideTable
        ) && self.state.list_context == ListContext::ListLevel
    }

    fn parse(mut self) -> Result<ParseOutput, ParseError> {
        let tokens = std::mem::take(&mut self.tokens);
        for token in &tokens {
            self.last_offset = token.offset;
            match &token.kind {
                TokenKind::StartGroup => self.start_group(token.offset)?,
                TokenKind::EndGroup => self.end_group(token.offset)?,
                TokenKind::Control(control) => self.apply_control(control, token.offset)?,
                TokenKind::Text(text) => self.apply_text(text, token.offset)?,
                TokenKind::HexByte(byte) => self.apply_hex_byte(*byte, token.offset)?,
                TokenKind::Binary(bytes) => self.apply_binary(bytes, token.offset)?,
            }
        }

        if !self.stack.is_empty() {
            return Err(ParseError::UnbalancedGroup(self.last_offset));
        }

        self.finish_table(self.last_offset)?;
        self.finish_paragraph();
        self.finish_footnote_paragraph();
        self.finish_endnote_paragraph();
        self.resolve_unmatched_note_reference_markers();
        self.normalize_page_orientation();
        self.document.blocks.retain(|block| match block {
            Block::Paragraph(paragraph) => !paragraph.runs.is_empty(),
            Block::Table(table) => !table.rows.is_empty(),
            Block::Image(_) => true,
            Block::Shape(_) => true,
            Block::Placeholder(_)
            | Block::PageBreak
            | Block::ColumnBreak
            | Block::ContinuousSectionBreak
            | Block::SectionBreak
            | Block::EvenPageSectionBreak
            | Block::OddPageSectionBreak => true,
            Block::SectionSettings(_) => true,
        });

        Ok(ParseOutput {
            document: self.document,
            diagnostics: self.diagnostics,
        })
    }

    fn limits(&self) -> &RtfLimits {
        &self.options.limits
    }

    fn destination_allows_ignorable_metadata(&self) -> bool {
        destination_allows_visible_content(&self.state)
            || (self.state.destination == Destination::Ignored
                && self
                    .stack
                    .last()
                    .is_some_and(destination_allows_visible_content))
    }

    fn current_page_content_width_twips(&self) -> i32 {
        let page = normalized_page_settings(self.current_section_page.clone());
        page.width_twips
            .saturating_sub(page.margin_left_twips)
            .saturating_sub(page.margin_right_twips)
            .max(1)
    }

    fn default_character_style(&self) -> CharacterStyle {
        CharacterStyle {
            font_index: self.default_font_index,
            ..CharacterStyle::default()
        }
    }

    fn set_default_font(&mut self, font_index: i32) {
        self.default_font_index = font_index.max(0);
        if !self.has_started_visible_body() {
            self.state.character.font_index = self.default_font_index;
            self.current_paragraph
                .runs
                .iter_mut()
                .for_each(|run| run.style.font_index = self.default_font_index);
        }
    }

    fn start_group(&mut self, offset: usize) -> Result<(), ParseError> {
        if self.stack.len() >= self.limits().max_group_depth {
            return Err(ParseError::GroupDepthExceeded(offset));
        }
        let mut parent = self.state.clone();
        let mut child = self.state.clone();
        child.at_group_start = true;
        if parent.metadata_property.is_some() {
            child.metadata_property = None;
            child.metadata_property_text.clear();
            child.inside_document_info = false;
        }
        if parent.metadata_timestamp.is_some() {
            child.metadata_timestamp = None;
            child.metadata_timestamp_value = DocumentTimestamp::default();
            child.inside_document_info = false;
        }
        if parent.user_property_capture.is_some() {
            child.user_property_capture = None;
            child.user_property_capture_text.clear();
            child.inside_user_properties = false;
        }
        if parent.unicode_alternate_branch == UnicodeAlternateBranch::Container {
            parent.unicode_alternate_child_count += 1;
            child.unicode_alternate_child_count = 0;
            child.unicode_alternate_destination = parent.unicode_alternate_destination;
            match parent.unicode_alternate_child_count {
                1 => {
                    child.unicode_alternate_branch = UnicodeAlternateBranch::AnsiFallback;
                    child.destination = Destination::Ignored;
                }
                _ => {
                    child.unicode_alternate_branch = UnicodeAlternateBranch::UnicodeDestination;
                    child.destination = parent.unicode_alternate_destination;
                }
            }
        }
        self.stack.push(parent);
        self.state = child;
        Ok(())
    }

    fn end_group(&mut self, offset: usize) -> Result<(), ParseError> {
        if let Some(mut previous) = self.stack.pop() {
            if self.state.destination == Destination::FontTable {
                self.flush_font(offset)?;
            }
            if is_header_destination(self.state.destination) {
                self.finish_header_paragraph();
            }
            if is_footer_destination(self.state.destination) {
                self.finish_footer_paragraph();
            }
            if self.state.destination == Destination::Footnote {
                self.finish_footnote_paragraph();
            }
            if self.state.destination == Destination::Endnote {
                self.finish_endnote_paragraph();
            }
            if self.state.destination == Destination::Picture {
                self.finish_picture(offset)?;
            }
            if self.state.destination == Destination::StyleSheet {
                self.finish_style_definition(offset)?;
            }
            if let Some(property) = self.state.metadata_property
                && previous.metadata_property.is_none()
            {
                let text = self.state.metadata_property_text.clone();
                self.store_document_property(property, &text, offset)?;
            }
            if let Some(kind) = self.state.metadata_timestamp
                && previous.metadata_timestamp.is_none()
            {
                let timestamp = self.state.metadata_timestamp_value;
                self.store_document_timestamp(kind, timestamp, offset)?;
            }
            if let Some(capture) = self.state.user_property_capture
                && previous.user_property_capture.is_none()
            {
                let text = self.state.user_property_capture_text.clone();
                match capture {
                    UserPropertyCapture::Name => {
                        self.pending_custom_property_name =
                            clean_document_property_text(&text, offset, self.limits())?;
                    }
                    UserPropertyCapture::Value => {
                        let name = self.pending_custom_property_name.clone();
                        if let Some(name) = name {
                            self.store_custom_document_property(&name, &text, offset)?;
                            self.pending_custom_property_name = None;
                        }
                    }
                }
            }
            match self.state.list_context {
                ListContext::ListLevelText => self.normalize_current_list_level_text(),
                ListContext::ListLevel => self.finish_list_level(offset)?,
                ListContext::List => self.finish_list_definition(offset)?,
                ListContext::ListOverrideLevel => self.finish_list_override_level(offset)?,
                ListContext::ListOverride => self.finish_list_override(offset)?,
                ListContext::None => {}
            }

            if self.state.inside_object && previous.inside_object {
                previous.object_result_seen |= self.state.object_result_seen;
            } else if self.state.inside_object && !previous.inside_object {
                if !self.state.object_result_seen
                    && self.options.active_content_policy == ActiveContentPolicy::Placeholder
                    && !self.state.character.hidden
                {
                    self.push_placeholder_for_destination(
                        self.state.object_owner_destination,
                        "[Embedded object removed]".to_string(),
                        offset,
                    )?;
                }
            }

            if self.state.inside_shape && previous.inside_shape {
                previous.shape_result_seen |= self.state.shape_result_seen;
            } else if self.state.inside_shape && !previous.inside_shape {
                self.finish_paragraph();
                let rendered_shape = self.finish_shape(offset)?;
                if !self.state.shape_result_seen && !rendered_shape && !self.state.character.hidden
                {
                    self.push_placeholder("[Shape skipped: unsupported shape]".to_string());
                }
            }

            if self.state.inside_shape_picture {
                previous.shape_picture_rendered |= self.state.shape_picture_rendered;
            }
            if self.state.suppressing_nonshape_picture && !previous.suppressing_nonshape_picture {
                previous.shape_picture_rendered = false;
            }

            if self.state.inside_field && previous.inside_field {
                if self.state.capturing_form_dropdown_entry
                    && previous.capturing_form_dropdown_entry
                {
                    merge_child_form_dropdown_entry_text(
                        &mut previous.field_form_dropdown_entry_text,
                        &self.state.field_form_dropdown_entry_text,
                        self.limits().max_text_run_len,
                        offset,
                    )?;
                } else if self.state.capturing_form_dropdown_entry {
                    push_form_dropdown_entry(
                        &mut previous.field_form_dropdown_entries,
                        &self.state.field_form_dropdown_entry_text,
                        self.limits().max_form_field_entries,
                        offset,
                    )?;
                }
                previous.field_result_seen |= self.state.field_result_seen;
                previous.field_form_result_value = self
                    .state
                    .field_form_result_value
                    .or(previous.field_form_result_value);
                previous.field_form_default_result_value = self
                    .state
                    .field_form_default_result_value
                    .or(previous.field_form_default_result_value);
                previous.field_form_checkbox_result = self
                    .state
                    .field_form_checkbox_result
                    .or(previous.field_form_checkbox_result);
                previous.field_form_checkbox_default = self
                    .state
                    .field_form_checkbox_default
                    .or(previous.field_form_checkbox_default);
                merge_child_form_default_text(
                    &mut previous.field_form_default_text,
                    &self.state.field_form_default_text,
                    self.limits().max_text_run_len,
                    offset,
                )?;
                merge_child_form_dropdown_entries(
                    &mut previous.field_form_dropdown_entries,
                    &self.state.field_form_dropdown_entries,
                    self.limits().max_form_field_entries,
                    offset,
                )?;
                merge_child_field_instruction(
                    &mut previous.field_instruction,
                    &self.state.field_instruction,
                    self.limits().max_text_run_len,
                    offset,
                )?;
            } else if self.state.inside_field && !previous.inside_field {
                if self.state.capturing_form_dropdown_entry {
                    let entry_text = self.state.field_form_dropdown_entry_text.clone();
                    let entry_limit = self.limits().max_form_field_entries;
                    push_form_dropdown_entry(
                        &mut self.state.field_form_dropdown_entries,
                        &entry_text,
                        entry_limit,
                        offset,
                    )?;
                }
                let field_result_seen = self.state.field_result_seen;
                let field_owner_destination = self.state.field_owner_destination;
                let field_instruction = self.state.field_instruction.clone();
                let field_form_checkbox_checked = self
                    .state
                    .field_form_checkbox_result
                    .or(self.state.field_form_checkbox_default);
                let field_form_default_text = self.state.field_form_default_text.clone();
                let field_form_dropdown_entries = self.state.field_form_dropdown_entries.clone();
                let field_form_dropdown_selected_index = self
                    .state
                    .field_form_result_value
                    .or(self.state.field_form_default_result_value);
                let field_hidden = self.state.character.hidden;
                self.state = previous;
                if !field_result_seen
                    && self.options.active_content_policy == ActiveContentPolicy::Placeholder
                    && !field_hidden
                {
                    if let Some(name) = field_instruction_name(&field_instruction)
                        && is_non_visible_resultless_field(name)
                    {
                        self.diagnostics.push(Diagnostic::warning(
                            format!("non-visible field {name} stripped without executing field instruction"),
                            Some(offset),
                        ));
                    } else if let Some(name) = field_instruction_name(&field_instruction)
                        && is_external_resultless_field(name)
                    {
                        self.diagnostics.push(Diagnostic::warning(
                            format!(
                                "external field {name} removed without fetching external resource"
                            ),
                            Some(offset),
                        ));
                        self.push_placeholder_for_destination(
                            field_owner_destination,
                            "[Field removed: no passive result]".to_string(),
                            offset,
                        )?;
                    } else if let Some(result) = self.passive_field_result_for_instruction(
                        &field_instruction,
                        field_form_checkbox_checked,
                        &field_form_default_text,
                        &field_form_dropdown_entries,
                        field_form_dropdown_selected_index,
                        offset,
                    )? {
                        self.diagnostics.push(Diagnostic::warning(
                            format!(
                                "rendering passive field {} without executing field instruction",
                                field_instruction_name(&field_instruction).unwrap_or("unknown")
                            ),
                            Some(offset),
                        ));
                        self.push_passive_field_result(result, offset)?;
                    } else {
                        if let Some(name) = field_instruction_name(&field_instruction) {
                            self.diagnostics.push(Diagnostic::warning(
                                format!(
                                    "field {name} has no stored result and was not evaluated dynamically"
                                ),
                                Some(offset),
                            ));
                        }
                        self.push_placeholder_for_destination(
                            field_owner_destination,
                            "[Field removed: no passive result]".to_string(),
                            offset,
                        )?;
                    }
                } else if !field_result_seen
                    && self.options.active_content_policy == ActiveContentPolicy::Strip
                    && !field_hidden
                {
                    if let Some(name) = field_instruction_name(&field_instruction)
                        && is_external_resultless_field(name)
                    {
                        self.diagnostics.push(Diagnostic::warning(
                            format!(
                                "external field {name} stripped without fetching external resource"
                            ),
                            Some(offset),
                        ));
                    } else if !field_instruction_name(&field_instruction)
                        .is_some_and(is_non_visible_resultless_field)
                    {
                        if let Some(result) = self.passive_field_result_for_instruction(
                            &field_instruction,
                            field_form_checkbox_checked,
                            &field_form_default_text,
                            &field_form_dropdown_entries,
                            field_form_dropdown_selected_index,
                            offset,
                        )? {
                            self.diagnostics.push(Diagnostic::warning(
                                format!(
                                    "rendering passive field {} without executing field instruction",
                                    field_instruction_name(&field_instruction).unwrap_or("unknown")
                                ),
                                Some(offset),
                            ));
                            self.push_passive_field_result(result, offset)?;
                        }
                    }
                }
                return Ok(());
            }

            if self.state.destination == Destination::BookmarkStart {
                let name = self.state.bookmark_name_text.clone();
                self.state = previous;
                if let Some(id) = self.start_bookmark_capture(name, offset)? {
                    let marker = bookmark_page_anchor_marker(id);
                    self.push_text(&marker, offset)?;
                }
                return Ok(());
            }
            if self.state.destination == Destination::BookmarkEnd {
                let name = self.state.bookmark_name_text.clone();
                self.state = previous;
                self.end_bookmark_capture(name);
                return Ok(());
            }

            self.state = previous;
            Ok(())
        } else {
            Err(ParseError::UnbalancedGroup(offset))
        }
    }

    fn apply_control(&mut self, control: &Control, offset: usize) -> Result<(), ParseError> {
        let control_starts_group = self.state.at_group_start;
        self.state.at_group_start = false;
        if self.state.skip_bytes > 0 && control.name != "u" {
            return Ok(());
        }
        if self.state.skip_password_hash_payload {
            self.state.skip_password_hash_payload = false;
        }
        if control.name != "u" {
            self.state.pending_unicode_high_surrogate = None;
        }
        if self.handle_nested_table_control(control, offset)? {
            return Ok(());
        }

        match control.name.as_str() {
            "rtf" | "viewkind" | "generator" => {}
            "deff" => self.set_default_font(control.parameter.unwrap_or(0)),
            "ansi" => self.state.code_page = CodePage::Windows1252,
            "ansicpg" => {
                let code_page = control.parameter.unwrap_or(1252);
                self.state.code_page = if let Some(code_page) =
                    CodePage::from_rtf_code_page(code_page)
                {
                    code_page
                } else {
                    self.diagnostics.push(Diagnostic::warning(
                            format!(
                                "unsupported RTF code page {code_page}; non-ASCII hex escapes use replacement characters"
                            ),
                            Some(offset),
                        ));
                    CodePage::Unsupported
                };
            }
            "mac" => self.state.code_page = CodePage::MacRoman,
            "pc" => self.state.code_page = CodePage::Ibm437,
            "pca" => self.state.code_page = CodePage::Ibm850,
            "*" => self.state.destination = Destination::Ignored,
            "upr" if destination_allows_visible_content(&self.state) => {
                self.state.unicode_alternate_branch = UnicodeAlternateBranch::Container;
                self.state.unicode_alternate_child_count = 0;
                self.state.unicode_alternate_destination = self.state.destination;
            }
            "ud" if self.state.unicode_alternate_branch
                == UnicodeAlternateBranch::UnicodeDestination =>
            {
                self.state.destination = self.state.unicode_alternate_destination;
            }
            "mden" if destination_allows_visible_content(&self.state) => {
                self.push_text("/", offset)?;
            }
            "mbar" if destination_allows_visible_content(&self.state) => {
                self.push_text("\u{00af}", offset)?;
            }
            "mrad" if destination_allows_visible_content(&self.state) => {
                self.push_text("\u{221a}", offset)?;
            }
            "msub" if destination_allows_visible_content(&self.state) => {
                self.push_text("_", offset)?;
            }
            "msup" if destination_allows_visible_content(&self.state) => {
                self.push_text("^", offset)?;
            }
            name if is_office_math_control(name)
                && destination_allows_visible_content(&self.state) =>
            {
                if name == "mmath" || name == "moMath" {
                    self.diagnostics.push(Diagnostic::warning(
                        "Office math layout approximated as passive text",
                        Some(offset),
                    ));
                }
            }
            "fonttbl" if destination_allows_safe_structural_content(&self.state) => {
                self.state.destination = Destination::FontTable
            }
            "falt"
                if self.state.destination == Destination::FontTable
                    || (self.state.destination == Destination::Ignored
                        && self
                            .stack
                            .last()
                            .is_some_and(|state| state.destination == Destination::FontTable)) =>
            {
                self.state.destination = Destination::FontAlternate;
            }
            "colortbl" if destination_allows_safe_structural_content(&self.state) => {
                self.state.destination = Destination::ColorTable
            }
            "stylesheet" if destination_allows_safe_structural_content(&self.state) => {
                self.state.destination = Destination::StyleSheet
            }
            "template" if destination_allows_visible_content(&self.state) => {
                self.handle_active_content("external template", offset)?;
                self.state.destination = Destination::Metadata;
                self.state.inside_metadata = true;
            }
            "fontemb" | "fontfile" if destination_allows_safe_structural_content(&self.state) => {
                self.handle_active_content("embedded font payload", offset)?;
                self.state.destination = Destination::Metadata;
                self.state.inside_metadata = true;
            }
            name if is_mail_merge_destination(name)
                && destination_allows_safe_structural_content(&self.state) =>
            {
                self.handle_active_content("mail merge data source", offset)?;
                self.state.destination = Destination::Metadata;
                self.state.inside_metadata = true;
            }
            name if is_annotation_destination(name)
                && destination_allows_safe_structural_content(&self.state) =>
            {
                self.handle_active_content("annotation metadata", offset)?;
                self.state.destination = Destination::Metadata;
                self.state.inside_metadata = true;
            }
            name if is_object_metadata_destination(name)
                && self.state.destination == Destination::ObjectData => {}
            name if is_object_metadata_destination(name)
                && destination_allows_visible_content(&self.state) =>
            {
                self.state.destination = Destination::Metadata;
                self.state.inside_metadata = true;
            }
            "info" if destination_allows_visible_content(&self.state) => {
                self.state.destination = Destination::Metadata;
                self.state.inside_metadata = true;
                self.state.inside_document_info = true;
            }
            "userprops" if self.destination_allows_ignorable_metadata() => {
                self.state.destination = Destination::Metadata;
                self.state.inside_metadata = true;
                self.state.inside_user_properties = true;
                self.pending_custom_property_name = None;
            }
            name if control_starts_group
                && self.state.inside_document_info
                && document_property_control(name).is_some()
                && self.state.destination == Destination::Metadata =>
            {
                self.state.destination = Destination::Metadata;
                self.state.inside_metadata = true;
                self.state.metadata_property = document_property_control(name);
                self.state.metadata_property_text.clear();
            }
            name if control_starts_group
                && self.state.inside_document_info
                && document_timestamp_control(name).is_some()
                && self.state.destination == Destination::Metadata =>
            {
                self.state.destination = Destination::Metadata;
                self.state.inside_metadata = true;
                self.state.metadata_timestamp = document_timestamp_control(name);
                self.state.metadata_timestamp_value = DocumentTimestamp::default();
            }
            "edmins"
                if self.state.inside_document_info
                    && self.state.destination == Destination::Metadata =>
            {
                self.set_document_edit_minutes(control.parameter, offset)?;
            }
            "propname"
                if control_starts_group
                    && self.state.inside_user_properties
                    && self.state.destination == Destination::Metadata =>
            {
                self.state.user_property_capture = Some(UserPropertyCapture::Name);
                self.state.user_property_capture_text.clear();
            }
            "staticval"
                if control_starts_group
                    && self.state.inside_user_properties
                    && self.state.destination == Destination::Metadata =>
            {
                self.state.user_property_capture = Some(UserPropertyCapture::Value);
                self.state.user_property_capture_text.clear();
            }
            name if is_metadata_destination(name)
                && destination_allows_visible_content(&self.state) =>
            {
                self.state.destination = Destination::Metadata;
                self.state.inside_metadata = true;
            }
            "listtable" if destination_allows_safe_structural_content(&self.state) => {
                self.state.destination = Destination::ListTable;
            }
            "listoverridetable" if destination_allows_safe_structural_content(&self.state) => {
                self.state.destination = Destination::ListOverrideTable;
            }
            "list" if self.state.destination == Destination::ListTable => {
                self.start_list_definition();
            }
            "listlevel" if self.state.destination == Destination::ListTable => {
                self.start_list_level();
            }
            "listlevel"
                if self.state.destination == Destination::ListOverrideTable
                    && self.state.list_context == ListContext::ListOverrideLevel =>
            {
                self.start_list_level();
            }
            "leveltext" if self.state.destination == Destination::ListTable => {
                self.start_list_level_text();
            }
            "leveltext"
                if self.state.destination == Destination::ListOverrideTable
                    && self.state.list_context == ListContext::ListLevel =>
            {
                self.start_list_level_text();
            }
            "levelnfc" | "levelnfcn" if self.state.destination == Destination::ListTable => {
                self.set_current_list_level_format(control.parameter.unwrap_or(0));
            }
            "levelnfc" | "levelnfcn"
                if self.state.destination == Destination::ListOverrideTable
                    && self.state.list_context == ListContext::ListLevel =>
            {
                self.set_current_list_level_format(control.parameter.unwrap_or(0));
            }
            "levelstartat"
                if self.state.destination == Destination::ListOverrideTable
                    && self.state.list_context == ListContext::ListOverrideLevel =>
            {
                self.set_current_list_override_level_start(control.parameter.unwrap_or(1));
            }
            "levelstartat" if self.state.destination == Destination::ListTable => {
                self.set_current_list_level_start(control.parameter.unwrap_or(1));
            }
            "levelstartat"
                if self.state.destination == Destination::ListOverrideTable
                    && self.state.list_context == ListContext::ListLevel =>
            {
                self.set_current_list_level_start(control.parameter.unwrap_or(1));
            }
            "levelindent" if self.state.destination == Destination::ListTable => {
                self.set_current_list_level_indent(control.parameter, offset);
            }
            "levelspace" if self.state.destination == Destination::ListTable => {
                self.set_current_list_level_spacing(control.parameter, offset);
            }
            "levelfollow" if self.state.destination == Destination::ListTable => {
                self.set_current_list_level_follow(control.parameter.unwrap_or(0));
            }
            "levelfollow"
                if self.state.destination == Destination::ListOverrideTable
                    && self.state.list_context == ListContext::ListLevel =>
            {
                self.set_current_list_level_follow(control.parameter.unwrap_or(0));
            }
            "levellegal" if self.state.destination == Destination::ListTable => {
                self.set_current_list_level_legal_numbering(control.parameter.unwrap_or(1) != 0);
            }
            "levellegal"
                if self.state.destination == Destination::ListOverrideTable
                    && self.state.list_context == ListContext::ListLevel =>
            {
                self.set_current_list_level_legal_numbering(control.parameter.unwrap_or(1) != 0);
            }
            "levelnorestart" if self.state.destination == Destination::ListTable => {
                self.set_current_list_level_no_restart(control.parameter.unwrap_or(1) != 0);
            }
            "levelnorestart"
                if self.state.destination == Destination::ListOverrideTable
                    && self.state.list_context == ListContext::ListLevel =>
            {
                self.set_current_list_level_no_restart(control.parameter.unwrap_or(1) != 0);
            }
            "b" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_bold(control.parameter.unwrap_or(1) != 0);
            }
            "i" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_italic(control.parameter.unwrap_or(1) != 0);
            }
            "ul" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_underline(UnderlineStyle::Single, control);
            }
            "ulnone" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_underline_style(UnderlineStyle::None);
            }
            "uldb" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_underline(UnderlineStyle::Double, control);
            }
            "ulth" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_underline(UnderlineStyle::Thick, control);
            }
            "uld" | "ulthd" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_underline(UnderlineStyle::Dotted, control);
            }
            "uldash" | "uldashd" | "uldashdd" | "ulldash" | "ulthdash" | "ulthdashd"
            | "ulthdashdd" | "ulthldash"
                if self.is_parsing_list_level_definition() =>
            {
                self.set_current_list_level_underline(UnderlineStyle::Dashed, control);
            }
            "ulwave" | "ulhwave" | "uldbwave" | "ululdbwave"
                if self.is_parsing_list_level_definition() =>
            {
                self.set_current_list_level_underline(UnderlineStyle::Wave, control);
            }
            "ulw" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_underline(UnderlineStyle::Words, control);
            }
            "strike" | "striked" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_strike(control.parameter.unwrap_or(1) != 0);
            }
            "strikedl" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_double_strike(control.parameter.unwrap_or(1) != 0);
            }
            "outl" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_outline(control.parameter.unwrap_or(1) != 0);
            }
            "shad" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_shadow(control.parameter.unwrap_or(1) != 0);
            }
            "embo" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_relief(if control.parameter.unwrap_or(1) == 0 {
                    TextRelief::None
                } else {
                    TextRelief::Emboss
                });
            }
            "impr" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_relief(if control.parameter.unwrap_or(1) == 0 {
                    TextRelief::None
                } else {
                    TextRelief::Engrave
                });
            }
            "caps" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_caps(control.parameter.unwrap_or(1) != 0);
            }
            "scaps" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_small_caps(control.parameter.unwrap_or(1) != 0);
            }
            "plain" if self.is_parsing_list_level_definition() => {
                self.reset_current_list_level_character_style();
            }
            "cs" if self.is_parsing_list_level_definition() => {
                self.apply_current_list_level_character_style(
                    control.parameter.unwrap_or(0),
                    offset,
                );
            }
            "super" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_super(control.parameter.unwrap_or(1) != 0);
            }
            "sub" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_sub(control.parameter.unwrap_or(1) != 0);
            }
            "nosupersub" if self.is_parsing_list_level_definition() => {
                self.reset_current_list_level_script_position();
            }
            "up" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_baseline_shift(
                    control
                        .parameter
                        .unwrap_or(0)
                        .max(0)
                        .min(MAX_BASELINE_SHIFT_HALF_POINTS),
                );
            }
            "dn" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_baseline_shift(
                    -control
                        .parameter
                        .unwrap_or(0)
                        .max(0)
                        .min(MAX_BASELINE_SHIFT_HALF_POINTS),
                );
            }
            "expnd" if self.is_parsing_list_level_definition() => {
                let half_points = control.parameter.unwrap_or(0);
                self.set_current_list_level_character_spacing(
                    half_points.saturating_mul(10),
                    offset,
                );
            }
            "expndtw" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_character_spacing(
                    control.parameter.unwrap_or(0),
                    offset,
                );
            }
            "kerning" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_character_kerning(
                    control.parameter.unwrap_or(0),
                    offset,
                );
            }
            "charscalex" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_character_scaling(
                    control.parameter.unwrap_or(100),
                    offset,
                );
            }
            "f" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_font(control.parameter.unwrap_or(0));
            }
            "fs" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_font_size(control.parameter.unwrap_or(24), offset);
            }
            "ulc" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_underline_color(control.parameter.unwrap_or(0));
            }
            "cf" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_color(control.parameter.unwrap_or(0));
            }
            "highlight" | "cb" | "chcbpat" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_highlight(control.parameter.unwrap_or(0));
            }
            "chshdng" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_highlight_shading(
                    control.parameter.unwrap_or(10_000),
                    offset,
                );
            }
            "chbrdr" if self.is_parsing_list_level_definition() => {}
            "brdrnone" | "brdrnil" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_border_visible(false);
            }
            "brdrs" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_border_style(BorderStyle::Single);
            }
            "brdrth" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_border_style(BorderStyle::Thick);
            }
            "brdrhair" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_border_style(BorderStyle::Hairline);
            }
            "brdrdb" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_border_style(BorderStyle::Double);
            }
            "brdrdot" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_border_style(BorderStyle::Dotted);
            }
            "brdrdash" | "brdrdashsm" | "brdrdashd" | "brdrdashdd" | "brdrdashdot"
            | "brdrdashdotstr" | "brdrdashdotdot"
                if self.is_parsing_list_level_definition() =>
            {
                self.set_current_list_level_border_style(BorderStyle::Dashed);
            }
            "brdrwavy" | "brdrwavydb" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_border_style(BorderStyle::Wavy);
            }
            "brdrw" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_border_width(control.parameter, offset);
            }
            "brsp" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_border_spacing(control.parameter, offset);
            }
            "brdrcf" if self.is_parsing_list_level_definition() => {
                self.set_current_list_level_border_color(control.parameter);
            }
            "listid" if self.state.destination == Destination::ListTable => {
                self.set_current_list_id(control.parameter.unwrap_or(0));
            }
            "listoverride" if self.state.destination == Destination::ListOverrideTable => {
                self.start_list_override();
            }
            "listid" if self.state.destination == Destination::ListOverrideTable => {
                self.set_current_list_override_list_id(control.parameter.unwrap_or(0));
            }
            "ls" if self.state.destination == Destination::ListOverrideTable => {
                self.set_current_list_override_index(control.parameter.unwrap_or(0));
            }
            "lfolevel" if self.state.destination == Destination::ListOverrideTable => {
                self.start_list_override_level();
            }
            "listoverrideformat"
                if self.state.destination == Destination::ListOverrideTable
                    && self.state.list_context == ListContext::ListOverrideLevel =>
            {
                self.enable_current_list_override_level_format(control.parameter.unwrap_or(1));
            }
            "listoverridestartat"
                if self.state.destination == Destination::ListOverrideTable
                    && self.state.list_context == ListContext::ListOverrideLevel =>
            {
                self.enable_current_list_override_level_restart(control.parameter.unwrap_or(1));
            }
            "header" | "headerr" if destination_allows_visible_content(&self.state) => {
                self.state.destination = Destination::Header
            }
            "headerf" if destination_allows_visible_content(&self.state) => {
                self.state.destination = Destination::FirstPageHeader
            }
            "headerl" if destination_allows_visible_content(&self.state) => {
                self.state.destination = Destination::EvenPageHeader
            }
            "footer" | "footerr" if destination_allows_visible_content(&self.state) => {
                self.state.destination = Destination::Footer
            }
            "footerf" if destination_allows_visible_content(&self.state) => {
                self.state.destination = Destination::FirstPageFooter
            }
            "footerl" if destination_allows_visible_content(&self.state) => {
                self.state.destination = Destination::EvenPageFooter
            }
            "footnote" if destination_allows_visible_content(&self.state) => {
                self.start_note_destination(Destination::Footnote, offset)?
            }
            "endnote" | "aendnote" | "ftnalt"
                if destination_allows_visible_content(&self.state) =>
            {
                self.start_note_destination(Destination::Endnote, offset)?
            }
            "shppict"
                if self.state.destination == Destination::Ignored
                    || destination_allows_visible_content(&self.state) =>
            {
                self.state.destination = self
                    .current_shape
                    .as_ref()
                    .map(|shape| shape.owner_destination)
                    .unwrap_or(Destination::Body);
                self.state.inside_shape_picture = true;
                self.state.shape_picture_rendered = false;
                if self.state.inside_shape {
                    self.state.shape_result_seen = true;
                }
            }
            "nonshppict"
                if self.state.destination == Destination::Ignored
                    || destination_allows_visible_content(&self.state) =>
            {
                if self.state.shape_picture_rendered {
                    self.state.destination = Destination::Ignored;
                    self.state.suppressing_nonshape_picture = true;
                } else {
                    self.state.destination = self
                        .current_shape
                        .as_ref()
                        .map(|shape| shape.owner_destination)
                        .unwrap_or(Destination::Body);
                }
            }
            "pict" if destination_allows_visible_content(&self.state) => {
                self.finish_paragraph();
                let owner_destination = self.state.destination;
                self.state.destination = Destination::Picture;
                self.current_picture = Some(PictureBuilder {
                    owner_destination,
                    ..PictureBuilder::default()
                });
                if self.state.inside_shape {
                    self.state.shape_result_seen = true;
                }
            }
            "shp" | "do" if destination_allows_visible_content(&self.state) => {
                let owner_destination = self.state.destination;
                self.finish_table(offset)?;
                self.finish_paragraph();
                self.state.inside_shape = true;
                self.state.shape_result_seen = false;
                self.state.destination = Destination::Shape;
                self.current_shape = Some(ShapeBuilder {
                    owner_destination,
                    ..ShapeBuilder::default()
                });
            }
            "shptxt" | "shprslt" | "dptxbx"
                if self.state.inside_shape && destination_allows_visible_content(&self.state) =>
            {
                self.finish_paragraph();
                self.state.shape_result_seen = true;
                self.state.destination = self
                    .current_shape
                    .as_ref()
                    .map(|shape| shape.owner_destination)
                    .unwrap_or(Destination::Body);
                self.diagnostics.push(Diagnostic::warning(
                    "rendering safe passive shape text/result and stripping shape properties",
                    Some(offset),
                ));
            }
            "dpline" if self.state.destination == Destination::Shape => {
                self.set_current_shape_kind(StaticShapeKind::Line);
            }
            "dppolyline" if self.state.destination == Destination::Shape => {
                self.set_current_shape_kind(StaticShapeKind::Polyline);
            }
            "dppolygon" if self.state.destination == Destination::Shape => {
                self.set_current_shape_kind(StaticShapeKind::Polygon);
            }
            "dprect" if self.state.destination == Destination::Shape => {
                self.set_current_shape_kind(StaticShapeKind::Rectangle);
            }
            "dproundr" if self.state.destination == Destination::Shape => {
                self.set_current_shape_rounded_rectangle();
            }
            "dpellipse" if self.state.destination == Destination::Shape => {
                self.set_current_shape_kind(StaticShapeKind::Ellipse);
            }
            "dobx" if self.state.destination == Destination::Shape => {
                self.set_current_shape_base_x(control.parameter, offset);
            }
            "doby" if self.state.destination == Destination::Shape => {
                self.set_current_shape_base_y(control.parameter, offset);
            }
            "dpx" if self.state.destination == Destination::Shape => {
                self.set_current_shape_left(control.parameter, offset);
            }
            "dpy" if self.state.destination == Destination::Shape => {
                self.set_current_shape_top(control.parameter, offset);
            }
            "dpxsize" if self.state.destination == Destination::Shape => {
                self.set_current_shape_width(control.parameter, offset);
            }
            "dpysize" if self.state.destination == Destination::Shape => {
                self.set_current_shape_height(control.parameter, offset);
            }
            "dpptx" if self.state.destination == Destination::Shape => {
                self.set_current_shape_point_x(control.parameter, offset);
            }
            "dppty" if self.state.destination == Destination::Shape => {
                self.push_current_shape_point_y(control.parameter, offset)?;
            }
            "dplinew" if self.state.destination == Destination::Shape => {
                self.set_current_shape_stroke_width(control.parameter, offset);
            }
            "dplinesolid" if self.state.destination == Destination::Shape => {
                self.set_current_shape_stroke_style(BorderStyle::Single);
            }
            "dplinedot" if self.state.destination == Destination::Shape => {
                self.set_current_shape_stroke_style(BorderStyle::Dotted);
            }
            "dplinedash" | "dplinedado" | "dplinedadodo"
                if self.state.destination == Destination::Shape =>
            {
                self.set_current_shape_stroke_style(BorderStyle::Dashed);
            }
            "dplinehollow" if self.state.destination == Destination::Shape => {
                self.set_current_shape_stroke_style(BorderStyle::Double);
            }
            "dplinecor" if self.state.destination == Destination::Shape => {
                self.set_current_shape_stroke_red(control.parameter);
            }
            "dplinecog" if self.state.destination == Destination::Shape => {
                self.set_current_shape_stroke_green(control.parameter);
            }
            "dplinecob" if self.state.destination == Destination::Shape => {
                self.set_current_shape_stroke_blue(control.parameter);
            }
            "dpfillfgcr" if self.state.destination == Destination::Shape => {
                self.set_current_shape_fill_red(control.parameter);
            }
            "dpfillfgcg" if self.state.destination == Destination::Shape => {
                self.set_current_shape_fill_green(control.parameter);
            }
            "dpfillfgcb" if self.state.destination == Destination::Shape => {
                self.set_current_shape_fill_blue(control.parameter);
            }
            "dpfillpat" if self.state.destination == Destination::Shape => {
                self.set_current_shape_fill_pattern(control.parameter);
            }
            "jpegblip" if self.state.destination == Destination::Picture => {
                self.set_picture_kind(PictureKind::Jpeg)
            }
            "pngblip" if self.state.destination == Destination::Picture => {
                self.set_picture_kind(PictureKind::Png)
            }
            "emfblip" if self.state.destination == Destination::Picture => {
                self.set_picture_kind(PictureKind::Emf)
            }
            "wmetafile" if self.state.destination == Destination::Picture => {
                self.set_picture_kind(PictureKind::Wmf)
            }
            "dibitmap" if self.state.destination == Destination::Picture => {
                self.set_picture_kind(PictureKind::Dib)
            }
            "macpict" | "pmmetafile" | "wbitmap"
                if self.state.destination == Destination::Picture =>
            {
                self.set_picture_kind(PictureKind::Unsupported)
            }
            "picw" if self.state.destination == Destination::Picture => {
                self.set_picture_width_hint(control.parameter, offset)
            }
            "pich" if self.state.destination == Destination::Picture => {
                self.set_picture_height_hint(control.parameter, offset)
            }
            "picwgoal" if self.state.destination == Destination::Picture => {
                self.set_picture_display_width(control.parameter, offset)
            }
            "pichgoal" if self.state.destination == Destination::Picture => {
                self.set_picture_display_height(control.parameter, offset)
            }
            "picscalex" if self.state.destination == Destination::Picture => {
                self.set_picture_scale_x(control.parameter, offset)
            }
            "picscaley" if self.state.destination == Destination::Picture => {
                self.set_picture_scale_y(control.parameter, offset)
            }
            "piccropl" if self.state.destination == Destination::Picture => {
                self.set_picture_crop_left(control.parameter, offset)
            }
            "piccropt" if self.state.destination == Destination::Picture => {
                self.set_picture_crop_top(control.parameter, offset)
            }
            "piccropr" if self.state.destination == Destination::Picture => {
                self.set_picture_crop_right(control.parameter, offset)
            }
            "piccropb" if self.state.destination == Destination::Picture => {
                self.set_picture_crop_bottom(control.parameter, offset)
            }
            "listtext" | "pntext" if destination_allows_safe_structural_content(&self.state) => {
                self.pending_list_marker.clear();
                self.pending_list_marker_runs.clear();
                self.pending_old_style_list_marker = None;
                self.state.destination = Destination::ListText;
            }
            "pn" if destination_allows_safe_structural_content(&self.state) => {
                self.start_old_style_list_marker();
            }
            "pncard" | "pndec" if destination_allows_safe_structural_content(&self.state) => {
                self.set_old_style_list_marker_format(ListNumberFormat::Decimal);
            }
            "pnucrm" if destination_allows_safe_structural_content(&self.state) => {
                self.set_old_style_list_marker_format(ListNumberFormat::UpperRoman);
            }
            "pnlcrm" if destination_allows_safe_structural_content(&self.state) => {
                self.set_old_style_list_marker_format(ListNumberFormat::LowerRoman);
            }
            "pnucltr" if destination_allows_safe_structural_content(&self.state) => {
                self.set_old_style_list_marker_format(ListNumberFormat::UpperLetter);
            }
            "pnlcltr" if destination_allows_safe_structural_content(&self.state) => {
                self.set_old_style_list_marker_format(ListNumberFormat::LowerLetter);
            }
            "pnord" if destination_allows_safe_structural_content(&self.state) => {
                self.set_old_style_list_marker_format(ListNumberFormat::Ordinal);
            }
            "pnbul" | "pnlvlblt" if destination_allows_safe_structural_content(&self.state) => {
                self.set_old_style_list_marker_format(ListNumberFormat::Bullet);
            }
            "pnstart" if destination_allows_safe_structural_content(&self.state) => {
                self.set_old_style_list_marker_start(control.parameter.unwrap_or(1));
            }
            "pnb" if destination_allows_safe_structural_content(&self.state) => {
                self.set_old_style_list_marker_bold(control.parameter.unwrap_or(1) != 0);
            }
            "pni" if destination_allows_safe_structural_content(&self.state) => {
                self.set_old_style_list_marker_italic(control.parameter.unwrap_or(1) != 0);
            }
            "pnul" if destination_allows_safe_structural_content(&self.state) => {
                self.set_old_style_list_marker_underline(control.parameter.unwrap_or(1) != 0);
            }
            "pnstrike" if destination_allows_safe_structural_content(&self.state) => {
                self.set_old_style_list_marker_strike(control.parameter.unwrap_or(1) != 0);
            }
            "pncaps" if destination_allows_safe_structural_content(&self.state) => {
                self.set_old_style_list_marker_caps(control.parameter.unwrap_or(1) != 0);
            }
            "pncf" if destination_allows_safe_structural_content(&self.state) => {
                self.set_old_style_list_marker_color(control.parameter.unwrap_or(0));
            }
            "pnf" if destination_allows_safe_structural_content(&self.state) => {
                self.set_old_style_list_marker_font(control.parameter.unwrap_or(0));
            }
            "pnfs" if destination_allows_safe_structural_content(&self.state) => {
                self.set_old_style_list_marker_font_size(control.parameter.unwrap_or(24), offset);
            }
            "pnindent" if destination_allows_safe_structural_content(&self.state) => {
                self.set_old_style_list_marker_indent(control.parameter, offset);
            }
            "pnhang" if destination_allows_safe_structural_content(&self.state) => {
                self.set_old_style_list_marker_hanging(control.parameter.unwrap_or(1) != 0);
            }
            "pnsp" if destination_allows_safe_structural_content(&self.state) => {
                self.set_old_style_list_marker_spacing(control.parameter, offset);
            }
            "pntxtb" if destination_allows_safe_structural_content(&self.state) => {
                self.pending_list_marker.clear();
                self.pending_old_style_list_marker = None;
                self.state.destination = Destination::ListText;
            }
            "pntxta" if destination_allows_safe_structural_content(&self.state) => {
                self.state.destination = Destination::ListText;
            }
            "bkmkstart"
                if self.state.destination == Destination::Ignored
                    || destination_allows_safe_structural_content(&self.state) =>
            {
                self.state.destination = Destination::BookmarkStart;
                self.state.bookmark_name_text.clear();
            }
            "bkmkend"
                if self.state.destination == Destination::Ignored
                    || destination_allows_safe_structural_content(&self.state) =>
            {
                self.state.destination = Destination::BookmarkEnd;
                self.state.bookmark_name_text.clear();
            }
            "object" if destination_allows_visible_content(&self.state) => {
                let owner_destination = self.state.destination;
                self.handle_active_content("OLE object", offset)?;
                self.state.inside_object = true;
                self.state.object_owner_destination = owner_destination;
                self.state.object_result_seen = false;
                self.state.destination = Destination::ObjectData;
            }
            "objdata" | "objocx" | "objemb" | "objlink" | "objautlink" | "objupdate"
                if destination_allows_visible_content(&self.state) =>
            {
                self.handle_active_content(control.name.as_str(), offset)?;
                self.state.destination = Destination::ObjectData;
            }
            "result"
                if self.state.inside_object && destination_allows_visible_content(&self.state) =>
            {
                self.state.object_result_seen = true;
                self.state.destination = self.state.object_owner_destination;
                self.diagnostics.push(Diagnostic::warning(
                    "rendering safe embedded object result and stripping active object data",
                    Some(offset),
                ));
            }
            "field" if destination_allows_visible_content(&self.state) => {
                let owner_destination = self.state.destination;
                self.handle_active_content("field instruction", offset)?;
                self.state.inside_field = true;
                self.state.field_owner_destination = owner_destination;
                self.state.field_result_seen = false;
                self.state.destination = Destination::FieldInstruction;
            }
            "fldinst"
                if self.state.inside_field
                    && destination_allows_safe_structural_content(&self.state) =>
            {
                self.state.destination = Destination::FieldInstruction
            }
            "fldrslt"
                if self.state.inside_field && destination_allows_visible_content(&self.state) =>
            {
                self.state.field_result_seen = true;
                self.state.destination = self.state.field_owner_destination;
            }
            "fldrslt" if destination_allows_visible_content(&self.state) => {
                self.state.destination = Destination::Body
            }
            "passwordhash" => {
                self.state.skip_password_hash_payload = true;
                self.diagnostics.push(Diagnostic::warning(
                    "document protection password hash stripped before normalization",
                    Some(offset),
                ));
            }
            "f" if self.state.destination == Destination::FontTable => {
                self.flush_font(offset)?;
                self.current_font = Some(FontDef {
                    index: control.parameter.unwrap_or(0),
                    name: String::new(),
                    alternate_name: None,
                    charset: None,
                    code_page: None,
                    family: FontFamilyHint::Nil,
                    pitch: FontPitch::Default,
                });
            }
            "fnil" if self.state.destination == Destination::FontTable => {
                self.set_current_font_family(FontFamilyHint::Nil);
            }
            "froman" if self.state.destination == Destination::FontTable => {
                self.set_current_font_family(FontFamilyHint::Roman);
            }
            "flomajor" | "fhimajor" | "fdbmajor" | "fbimajor"
                if self.state.destination == Destination::FontTable =>
            {
                self.set_current_font_family(FontFamilyHint::Roman);
            }
            "fswiss" if self.state.destination == Destination::FontTable => {
                self.set_current_font_family(FontFamilyHint::Swiss);
            }
            "flominor" | "fhiminor" | "fdbminor" | "fbiminor"
                if self.state.destination == Destination::FontTable =>
            {
                self.set_current_font_family(FontFamilyHint::Swiss);
            }
            "fmodern" if self.state.destination == Destination::FontTable => {
                self.set_current_font_family(FontFamilyHint::Modern);
            }
            "fscript" if self.state.destination == Destination::FontTable => {
                self.set_current_font_family(FontFamilyHint::Script);
            }
            "fdecor" if self.state.destination == Destination::FontTable => {
                self.set_current_font_family(FontFamilyHint::Decor);
            }
            "ftech" if self.state.destination == Destination::FontTable => {
                self.set_current_font_family(FontFamilyHint::Tech);
            }
            "fbidi" if self.state.destination == Destination::FontTable => {
                self.set_current_font_family(FontFamilyHint::Bidi);
            }
            "fcharset" if self.state.destination == Destination::FontTable => {
                if let Some(font) = self.current_font.as_mut() {
                    font.charset = Some(control.parameter.unwrap_or(0));
                }
            }
            "cpg" if self.state.destination == Destination::FontTable => {
                let code_page = control.parameter.unwrap_or(1252);
                if let Some(font) = self.current_font.as_mut() {
                    font.code_page = Some(code_page);
                }
                if CodePage::from_rtf_code_page(code_page).is_none() {
                    self.diagnostics.push(Diagnostic::warning(
                        format!(
                            "unsupported RTF font code page {code_page}; non-ASCII hex escapes for this font use replacement characters"
                        ),
                        Some(offset),
                    ));
                }
            }
            "fprq" if self.state.destination == Destination::FontTable => {
                self.set_current_font_pitch(FontPitch::from_rtf_parameter(
                    control.parameter.unwrap_or(0),
                ));
            }
            "red" if self.state.destination == Destination::ColorTable => {
                self.current_color.red = control.parameter.unwrap_or(0).clamp(0, 255) as u8;
                self.current_color_seen = true;
            }
            "green" if self.state.destination == Destination::ColorTable => {
                self.current_color.green = control.parameter.unwrap_or(0).clamp(0, 255) as u8;
                self.current_color_seen = true;
            }
            "blue" if self.state.destination == Destination::ColorTable => {
                self.current_color.blue = control.parameter.unwrap_or(0).clamp(0, 255) as u8;
                self.current_color_seen = true;
            }
            "bin" => {}
            "yr" | "mo" | "dy" | "hr" | "min" | "sec"
                if self.state.metadata_timestamp.is_some() =>
            {
                self.set_document_timestamp_part(control.name.as_str(), control.parameter, offset)?;
            }
            "uc" => {
                self.state.unicode_skip =
                    self.clamp_unicode_fallback_skip(control.parameter.unwrap_or(1), offset)
            }
            "u" if self.state.capturing_form_default_text => {
                self.push_form_default_unicode(control.parameter.unwrap_or(0), offset)?
            }
            "u" if self.state.capturing_form_dropdown_entry => {
                self.push_form_dropdown_unicode(control.parameter.unwrap_or(0), offset)?
            }
            "u" if self.state.destination == Destination::ListText => {
                self.push_list_marker_unicode(control.parameter.unwrap_or(0), offset)?
            }
            "u" if self.state.destination == Destination::ListTable
                && self.state.list_context == ListContext::ListLevelText =>
            {
                self.push_list_level_unicode(control.parameter.unwrap_or(0), offset)?
            }
            "u" if self.state.destination == Destination::ListOverrideTable
                && self.state.list_context == ListContext::ListLevelText =>
            {
                self.push_list_level_unicode(control.parameter.unwrap_or(0), offset)?
            }
            "u" if matches!(
                self.state.destination,
                Destination::Body
                    | Destination::Header
                    | Destination::FirstPageHeader
                    | Destination::EvenPageHeader
                    | Destination::Footer
                    | Destination::FirstPageFooter
                    | Destination::EvenPageFooter
                    | Destination::Footnote
                    | Destination::Endnote
            ) =>
            {
                self.push_unicode(control.parameter.unwrap_or(0), offset)?
            }
            "u" if self.state.user_property_capture.is_some() => {
                self.push_user_property_unicode(control.parameter.unwrap_or(0), offset)?
            }
            "u" if self.state.metadata_property.is_some() => {
                self.push_document_property_unicode(control.parameter.unwrap_or(0), offset)?
            }
            "u" => {
                self.state.skip_bytes = self.state.unicode_skip;
                self.count_skipped_destination_bytes(1, offset)?;
            }
            "ffres" if self.state.inside_field && self.state.inside_metadata => {
                let value = control.parameter.unwrap_or(0);
                self.state.field_form_result_value = Some(value);
                self.state.field_form_checkbox_result = Some(value != 0);
                self.count_skipped_destination_bytes(control.name.len(), offset)?;
            }
            "ffdefres" if self.state.inside_field && self.state.inside_metadata => {
                let value = control.parameter.unwrap_or(0);
                self.state.field_form_default_result_value = Some(value);
                self.state.field_form_checkbox_default = Some(value != 0);
                self.count_skipped_destination_bytes(control.name.len(), offset)?;
            }
            "ffdeftext" if self.state.inside_field && self.state.inside_metadata => {
                self.state.capturing_form_default_text = true;
                self.count_skipped_destination_bytes(control.name.len(), offset)?;
            }
            "ffl" if self.state.inside_field && self.state.inside_metadata => {
                self.state.capturing_form_dropdown_entry = true;
                self.state.field_form_dropdown_entry_text.clear();
                self.count_skipped_destination_bytes(control.name.len(), offset)?;
            }
            "ffentrymcr" | "ffexitmcr" if self.state.inside_metadata => {
                self.handle_active_content("form field macro", offset)?;
                self.count_skipped_destination_bytes(control.name.len(), offset)?;
            }
            name if self.state.destination == Destination::Ignored
                && skipped_destination_active_feature(name).is_some() =>
            {
                let feature = skipped_destination_active_feature(name).expect("checked above");
                self.handle_active_content(feature, offset)?;
                self.count_skipped_destination_bytes(name.len(), offset)?;
            }
            name if matches!(
                self.state.destination,
                Destination::Ignored
                    | Destination::ObjectData
                    | Destination::FieldInstruction
                    | Destination::Picture
                    | Destination::Shape
                    | Destination::Metadata
                    | Destination::FontTable
                    | Destination::ColorTable
                    | Destination::ListTable
                    | Destination::ListOverrideTable
            ) =>
            {
                self.count_skipped_destination_bytes(name.len(), offset)?;
            }
            "par" => self.finish_current_paragraph_for_destination(offset),
            "line" => self.push_text("\n", offset)?,
            "chpgn" => self.push_text(PAGE_NUMBER_MARKER, offset)?,
            "sectnum" => self.push_text(SECTION_NUMBER_MARKER, offset)?,
            "chftn" => self.push_note_reference(offset)?,
            "chdate" | "chtime" | "chdpa" | "chdpl" => {
                self.handle_dynamic_date_time_control(control.name.as_str(), offset)?
            }
            "emdash" => self.push_text("\u{2014}", offset)?,
            "endash" => self.push_text("\u{2013}", offset)?,
            "bullet" => self.push_text("\u{2022}", offset)?,
            "lquote" => self.push_text("\u{2018}", offset)?,
            "rquote" => self.push_text("\u{2019}", offset)?,
            "ldblquote" => self.push_text("\u{201c}", offset)?,
            "rdblquote" => self.push_text("\u{201d}", offset)?,
            "emspace" => self.push_text("\u{2003}", offset)?,
            "enspace" => self.push_text("\u{2002}", offset)?,
            "qmspace" => self.push_text("\u{2005}", offset)?,
            "zwbo" => self.push_text("\u{200b}", offset)?,
            "zwnbo" => self.push_text("\u{feff}", offset)?,
            "zwnj" => self.push_text("\u{200c}", offset)?,
            "zwj" => self.push_text("\u{200d}", offset)?,
            "ltrmark" => self.push_text("\u{200e}", offset)?,
            "rtlmark" => self.push_text("\u{200f}", offset)?,
            "tab" if self.state.destination == Destination::ListText => {
                self.push_list_marker_text("\t", offset)?
            }
            "tab" => self.push_text("\t", offset)?,
            "trowd" => self.start_table_row(),
            "trrh" => self.set_current_table_row_height(control.parameter, offset),
            "trleft" => self.set_current_table_row_left_offset(control.parameter, offset),
            "trgaph" => self.set_current_table_cell_gap(control.parameter, offset),
            "trql" => self.set_current_table_row_alignment(TableRowAlignment::Left),
            "trqc" => self.set_current_table_row_alignment(TableRowAlignment::Center),
            "trqr" => self.set_current_table_row_alignment(TableRowAlignment::Right),
            "taprtl" | "rtlrow" => {
                self.set_current_table_row_right_to_left(control.parameter.unwrap_or(1) != 0)
            }
            "trhdr" => {
                self.set_current_table_row_repeat_header(control.parameter.unwrap_or(1) != 0)
            }
            "trkeep" => {
                self.set_current_table_row_keep_together(control.parameter.unwrap_or(1) != 0)
            }
            "trpaddl" => self.set_current_table_row_padding_left(control.parameter, offset),
            "trpaddr" => self.set_current_table_row_padding_right(control.parameter, offset),
            "trpaddt" => self.set_current_table_row_padding_top(control.parameter, offset),
            "trpaddb" => self.set_current_table_row_padding_bottom(control.parameter, offset),
            "trcbpat" | "trcfpat" => {
                self.set_current_table_row_shading(control.parameter.unwrap_or(0).max(0) as usize)
            }
            "trshdng" => self.set_current_table_row_shading_basis(control.parameter, offset),
            name if table_row_shading_pattern_control(name).is_some() => self
                .set_current_table_row_shading_pattern(
                    table_row_shading_pattern_control(name).expect("checked above"),
                ),
            "cellx" => self.push_table_cell_boundary(control.parameter.unwrap_or(0).max(0)),
            "clcbpat" | "clcfpat" => {
                self.set_current_cell_shading(control.parameter.unwrap_or(0).max(0) as usize)
            }
            "clshdng" => self.set_current_cell_shading_basis(control.parameter, offset),
            name if table_cell_shading_pattern_control(name).is_some() => self
                .set_current_cell_shading_pattern(
                    table_cell_shading_pattern_control(name).expect("checked above"),
                ),
            "clpadl" => self.set_current_cell_padding_left(control.parameter, offset),
            "clpadr" => self.set_current_cell_padding_right(control.parameter, offset),
            "clpadt" => self.set_current_cell_padding_top(control.parameter, offset),
            "clpadb" => self.set_current_cell_padding_bottom(control.parameter, offset),
            "clftsWidth" => self.set_current_cell_preferred_width_unit(control.parameter),
            "clwWidth" => self.set_current_cell_preferred_width(control.parameter),
            "clNoWrap" | "clnowrap" => {
                self.set_current_cell_no_wrap(control.parameter.unwrap_or(1) != 0)
            }
            "cltxlrtb" => {
                self.set_current_cell_text_direction(TableCellTextDirection::LeftToRightTopToBottom)
            }
            "cltxtbrlv" | "cltxlrtbv" => {
                self.set_current_cell_text_direction(TableCellTextDirection::TopToBottomRightToLeft)
            }
            "cltxbtlr" => {
                self.set_current_cell_text_direction(TableCellTextDirection::BottomToTopLeftToRight)
            }
            "clbrdrl" => self.select_current_cell_border(TableCellBorderSide::Left),
            "clbrdrr" => self.select_current_cell_border(TableCellBorderSide::Right),
            "clbrdrt" => self.select_current_cell_border(TableCellBorderSide::Top),
            "clbrdrb" => self.select_current_cell_border(TableCellBorderSide::Bottom),
            "cldgll" => self.select_current_cell_border(TableCellBorderSide::DiagonalDown),
            "cldglu" => self.select_current_cell_border(TableCellBorderSide::DiagonalUp),
            "trbrdrl" => self.select_current_table_row_border(TableCellBorderSide::Left),
            "trbrdrr" => self.select_current_table_row_border(TableCellBorderSide::Right),
            "trbrdrt" => self.select_current_table_row_border(TableCellBorderSide::Top),
            "trbrdrb" => self.select_current_table_row_border(TableCellBorderSide::Bottom),
            "brdrl" => self.select_current_paragraph_border(TableCellBorderSide::Left),
            "brdrr" => self.select_current_paragraph_border(TableCellBorderSide::Right),
            "brdrt" => self.select_current_paragraph_border(TableCellBorderSide::Top),
            "brdrb" => self.select_current_paragraph_border(TableCellBorderSide::Bottom),
            "brdrbar" => self.select_current_paragraph_border(TableCellBorderSide::Left),
            "brdrbtw" => self.select_current_paragraph_between_border(),
            "pgbrdrl" => self.select_current_page_border(TableCellBorderSide::Left),
            "pgbrdrr" => self.select_current_page_border(TableCellBorderSide::Right),
            "pgbrdrt" => self.select_current_page_border(TableCellBorderSide::Top),
            "pgbrdrb" => self.select_current_page_border(TableCellBorderSide::Bottom),
            "pgbrdrhead" => {
                self.current_section_page.page_border_includes_header =
                    control.parameter.unwrap_or(1) != 0;
                self.upsert_current_section_settings();
            }
            "pgbrdrfoot" => {
                self.current_section_page.page_border_includes_footer =
                    control.parameter.unwrap_or(1) != 0;
                self.upsert_current_section_settings();
            }
            "pgbrdropt" => {
                self.current_section_page.page_border_from_page_edge =
                    control.parameter.unwrap_or(1) != 0;
                self.upsert_current_section_settings();
            }
            "box" => self.select_current_paragraph_box_border(),
            "chbrdr" => self.select_current_character_border(),
            "clvertalt" => self.set_current_cell_vertical_align(TableCellVerticalAlign::Top),
            "clvertalc" => self.set_current_cell_vertical_align(TableCellVerticalAlign::Center),
            "clvertalb" => self.set_current_cell_vertical_align(TableCellVerticalAlign::Bottom),
            "clmgf" => self.set_current_cell_horizontal_merge(TableCellHorizontalMerge::First),
            "clmrg" => {
                self.set_current_cell_horizontal_merge(TableCellHorizontalMerge::Continuation)
            }
            "clvmgf" => self.set_current_cell_vertical_merge(TableCellVerticalMerge::First),
            "clvmrg" => self.set_current_cell_vertical_merge(TableCellVerticalMerge::Continuation),
            "brdrnone" | "brdrnil" => self.set_current_border_visible(false),
            "brdrs" => self.set_current_border_style(BorderStyle::Single),
            "brdrth" => self.set_current_border_style(BorderStyle::Thick),
            "brdrhair" => self.set_current_border_style(BorderStyle::Hairline),
            "brdrdb" => self.set_current_border_style(BorderStyle::Double),
            "brdrdot" => self.set_current_border_style(BorderStyle::Dotted),
            "brdrdash" | "brdrdashsm" | "brdrdashd" | "brdrdashdd" | "brdrdashdot"
            | "brdrdashdotstr" | "brdrdashdotdot" => {
                self.set_current_border_style(BorderStyle::Dashed)
            }
            "brdrwavy" | "brdrwavydb" => self.set_current_border_style(BorderStyle::Wavy),
            "brdrw" => self.set_current_border_width(control.parameter, offset),
            "brsp" => self.set_current_border_spacing(control.parameter, offset),
            "brdrcf" => self.set_current_border_color(control.parameter),
            "cell" => self.finish_table_cell(offset)?,
            "row" => self.finish_table_row(offset)?,
            "page" => {
                self.finish_table(offset)?;
                self.finish_paragraph();
                self.document.blocks.push(Block::PageBreak);
            }
            "column" => {
                self.finish_table(offset)?;
                self.finish_paragraph();
                self.document.blocks.push(Block::ColumnBreak);
            }
            "sect" => {
                self.finish_table(offset)?;
                self.finish_paragraph();
                match self.state.section_break_kind {
                    SectionBreakKind::Continuous => {
                        self.document.blocks.push(Block::ContinuousSectionBreak)
                    }
                    SectionBreakKind::Page => self.document.blocks.push(Block::SectionBreak),
                    SectionBreakKind::EvenPage => {
                        self.document.blocks.push(Block::EvenPageSectionBreak)
                    }
                    SectionBreakKind::OddPage => {
                        self.document.blocks.push(Block::OddPageSectionBreak)
                    }
                    SectionBreakKind::Column => self.document.blocks.push(Block::ColumnBreak),
                }
            }
            "sectd" => {
                self.state.section_break_kind = SectionBreakKind::Page;
                self.current_section_page = PageSettings::default();
                self.current_section_page.page_number_format = Some(PageNumberFormat::Decimal);
                self.upsert_current_section_settings();
            }
            "sbknone" => self.state.section_break_kind = SectionBreakKind::Continuous,
            "sbkpage" => self.state.section_break_kind = SectionBreakKind::Page,
            "sbkeven" => self.state.section_break_kind = SectionBreakKind::EvenPage,
            "sbkodd" => self.state.section_break_kind = SectionBreakKind::OddPage,
            "sbkcol" => self.state.section_break_kind = SectionBreakKind::Column,
            "b" => self.state.character.bold = control.parameter.unwrap_or(1) != 0,
            "i" => self.state.character.italic = control.parameter.unwrap_or(1) != 0,
            "ul" => {
                self.state.character.underline = if control.parameter.unwrap_or(1) == 0 {
                    UnderlineStyle::None
                } else {
                    UnderlineStyle::Single
                }
            }
            "ulnone" => self.state.character.underline = UnderlineStyle::None,
            "uldb" => {
                self.state.character.underline = if control.parameter.unwrap_or(1) == 0 {
                    UnderlineStyle::None
                } else {
                    UnderlineStyle::Double
                }
            }
            "ulth" => {
                self.state.character.underline = if control.parameter.unwrap_or(1) == 0 {
                    UnderlineStyle::None
                } else {
                    UnderlineStyle::Thick
                }
            }
            "uld" | "ulthd" => {
                self.state.character.underline = if control.parameter.unwrap_or(1) == 0 {
                    UnderlineStyle::None
                } else {
                    UnderlineStyle::Dotted
                }
            }
            "uldash" | "uldashd" | "uldashdd" | "ulldash" | "ulthdash" | "ulthdashd"
            | "ulthdashdd" | "ulthldash" => {
                self.state.character.underline = if control.parameter.unwrap_or(1) == 0 {
                    UnderlineStyle::None
                } else {
                    UnderlineStyle::Dashed
                }
            }
            "ulwave" | "ulhwave" | "uldbwave" | "ululdbwave" => {
                self.state.character.underline = if control.parameter.unwrap_or(1) == 0 {
                    UnderlineStyle::None
                } else {
                    UnderlineStyle::Wave
                }
            }
            "ulw" => {
                self.state.character.underline = if control.parameter.unwrap_or(1) == 0 {
                    UnderlineStyle::None
                } else {
                    UnderlineStyle::Words
                }
            }
            "ulc" => {
                self.state.character.underline_color_index =
                    Some(control.parameter.unwrap_or(0).max(0) as usize)
            }
            "strike" | "striked" => {
                let enabled = control.parameter.unwrap_or(1) != 0;
                self.state.character.strike = enabled;
                self.state.character.double_strike = false;
            }
            "strikedl" => {
                let enabled = control.parameter.unwrap_or(1) != 0;
                self.state.character.strike = enabled;
                self.state.character.double_strike = enabled;
            }
            "outl" => self.state.character.outline = control.parameter.unwrap_or(1) != 0,
            "shad" => self.state.character.shadow = control.parameter.unwrap_or(1) != 0,
            "embo" => {
                self.state.character.relief = if control.parameter.unwrap_or(1) == 0 {
                    TextRelief::None
                } else {
                    TextRelief::Emboss
                };
            }
            "impr" => {
                self.state.character.relief = if control.parameter.unwrap_or(1) == 0 {
                    TextRelief::None
                } else {
                    TextRelief::Engrave
                };
            }
            "caps" => self.state.character.all_caps = control.parameter.unwrap_or(1) != 0,
            "scaps" => self.state.character.small_caps = control.parameter.unwrap_or(1) != 0,
            "v" | "vanish" | "webhidden" => {
                self.state.character.hidden = control.parameter.unwrap_or(1) != 0
            }
            "super" => {
                if control.parameter.unwrap_or(1) == 0 {
                    self.reset_script_position();
                } else {
                    self.state.character.baseline_shift_half_points =
                        DEFAULT_SUPERSCRIPT_SHIFT_HALF_POINTS;
                    self.state.character.font_size_scale_percent =
                        DEFAULT_SCRIPT_FONT_SCALE_PERCENT;
                }
            }
            "sub" => {
                if control.parameter.unwrap_or(1) == 0 {
                    self.reset_script_position();
                } else {
                    self.state.character.baseline_shift_half_points =
                        DEFAULT_SUBSCRIPT_SHIFT_HALF_POINTS;
                    self.state.character.font_size_scale_percent =
                        DEFAULT_SCRIPT_FONT_SCALE_PERCENT;
                }
            }
            "nosupersub" => self.reset_script_position(),
            "up" => {
                self.state.character.baseline_shift_half_points = control
                    .parameter
                    .unwrap_or(0)
                    .max(0)
                    .min(MAX_BASELINE_SHIFT_HALF_POINTS);
                self.state.character.font_size_scale_percent = 100;
            }
            "dn" => {
                self.state.character.baseline_shift_half_points = -control
                    .parameter
                    .unwrap_or(0)
                    .max(0)
                    .min(MAX_BASELINE_SHIFT_HALF_POINTS);
                self.state.character.font_size_scale_percent = 100;
            }
            "expnd" => {
                let half_points = control.parameter.unwrap_or(0);
                self.state.character.character_spacing_twips =
                    self.clamp_character_spacing(half_points.saturating_mul(10), offset);
            }
            "expndtw" => {
                self.state.character.character_spacing_twips =
                    self.clamp_character_spacing(control.parameter.unwrap_or(0), offset);
            }
            "kerning" => {
                self.state.character.character_kerning_half_points =
                    self.clamp_character_kerning(control.parameter.unwrap_or(0), offset);
                self.diagnostics.push(Diagnostic::warning(
                    "character kerning approximated by passive pair spacing",
                    Some(offset),
                ));
            }
            "charscalex" => {
                self.state.character.character_scaling_percent =
                    self.clamp_character_scaling(control.parameter.unwrap_or(100), offset);
            }
            "plain" => self.state.character = self.default_character_style(),
            "fs" => {
                self.state.character.font_size_half_points =
                    self.clamp_font_size(control.parameter.unwrap_or(24), offset)
            }
            "sbasedon" if self.state.destination == Destination::StyleSheet => {
                let based_on = control.parameter.unwrap_or(-1);
                self.state.style_based_on = if based_on < 0 || based_on == 222 {
                    None
                } else {
                    Some(based_on)
                };
            }
            "snext" if self.state.destination == Destination::StyleSheet => {
                let next_style = control.parameter.unwrap_or(-1);
                self.state.style_next = if next_style < 0 || next_style == 222 {
                    None
                } else {
                    Some(next_style)
                };
            }
            name if self.state.destination == Destination::StyleSheet
                && is_stylesheet_metadata_control(name) =>
            {
                self.count_skipped_destination_bytes(name.len(), offset)?;
            }
            "s" if self.state.destination == Destination::StyleSheet => {
                self.state.style_index = Some(control.parameter.unwrap_or(0));
                self.state.style_kind = StyleKind::Paragraph;
            }
            "cs" if self.state.destination == Destination::StyleSheet => {
                self.state.style_index = Some(control.parameter.unwrap_or(0));
                self.state.style_kind = StyleKind::Character;
            }
            "s" => {
                self.apply_paragraph_style(control.parameter.unwrap_or(0), offset);
            }
            "cs" => {
                self.apply_character_style(control.parameter.unwrap_or(0), offset);
            }
            "ls" => {
                let index = control.parameter.unwrap_or(0);
                self.state.list_override_index = if index <= 0 { None } else { Some(index) };
            }
            "ilvl" => {
                self.state.list_level_index = control.parameter.unwrap_or(0).clamp(0, 8) as usize;
            }
            "f" => self.state.character.font_index = control.parameter.unwrap_or(0),
            "cf" => {
                self.state.character.color_index = control.parameter.unwrap_or(0).max(0) as usize
            }
            "highlight" | "cb" | "chcbpat" => {
                let color_index = control.parameter.unwrap_or(0).max(0) as usize;
                self.state.character.highlight_index = if color_index == 0 {
                    None
                } else {
                    Some(color_index)
                };
            }
            "chshdng" => {
                self.state.character.highlight_shading_basis_points =
                    self.clamp_character_shading(control.parameter.unwrap_or(10_000), offset);
            }
            "cbpat" | "cfpat" => {
                let color_index = control.parameter.unwrap_or(0).max(0) as usize;
                self.state.paragraph.shading_color_index = if color_index == 0 {
                    None
                } else {
                    Some(color_index)
                };
            }
            "shading" => {
                self.state.paragraph.shading_basis_points = self.clamp_shading_basis(
                    control.parameter.unwrap_or(10_000),
                    "paragraph",
                    offset,
                );
            }
            name if paragraph_shading_pattern_control(name).is_some() => {
                self.state.paragraph.shading_pattern =
                    paragraph_shading_pattern_control(name).expect("checked above");
            }
            "ql" => self.state.paragraph.alignment = Alignment::Left,
            "qc" => self.state.paragraph.alignment = Alignment::Center,
            "qr" => self.state.paragraph.alignment = Alignment::Right,
            "qj" | "qd" | "qk" => self.state.paragraph.alignment = Alignment::Justified,
            "rtlpar" => self.state.paragraph.alignment = Alignment::Right,
            "ltrpar" => self.state.paragraph.alignment = Alignment::Left,
            "pagebb" => {
                self.state.paragraph.page_break_before = control.parameter.unwrap_or(1) != 0
            }
            "keep" => self.state.paragraph.keep_together = control.parameter.unwrap_or(1) != 0,
            "keepn" => self.state.paragraph.keep_with_next = control.parameter.unwrap_or(1) != 0,
            "widowctrl" => {
                self.default_paragraph_style.widow_control = control.parameter.unwrap_or(1) != 0;
                self.state.paragraph.widow_control = self.default_paragraph_style.widow_control;
            }
            "widctlpar" => self.state.paragraph.widow_control = control.parameter.unwrap_or(1) != 0,
            "nowidctlpar" => self.state.paragraph.widow_control = false,
            "nowwrap" => self.state.paragraph.no_wrap = control.parameter.unwrap_or(1) != 0,
            "dropcapli" => self.set_drop_cap_lines(control.parameter, offset),
            "dropcapt" => self.set_drop_cap_type(control.parameter),
            "li" => {
                self.state.paragraph.left_indent_twips =
                    self.clamp_paragraph_indent(control.parameter, "left indent", offset)
            }
            "ri" => {
                self.state.paragraph.right_indent_twips =
                    self.clamp_paragraph_indent(control.parameter, "right indent", offset)
            }
            "fi" => {
                self.state.paragraph.first_line_indent_twips =
                    self.clamp_paragraph_indent(control.parameter, "first-line indent", offset)
            }
            "sb" => {
                self.state.paragraph.space_before_twips = self.clamp_paragraph_spacing(
                    control.parameter,
                    "paragraph space before",
                    offset,
                )
            }
            "sa" => {
                self.state.paragraph.space_after_twips =
                    self.clamp_paragraph_spacing(control.parameter, "paragraph space after", offset)
            }
            "sbauto" => {
                self.state.paragraph.auto_space_before = control.parameter.unwrap_or(1) != 0
            }
            "saauto" => self.state.paragraph.auto_space_after = control.parameter.unwrap_or(1) != 0,
            "contextualspace" => {
                self.state.paragraph.contextual_spacing = control.parameter.unwrap_or(1) != 0
            }
            "sl" => {
                self.state.paragraph.line_spacing_twips =
                    self.clamp_line_spacing(control.parameter, offset)
            }
            "slmult" => {
                self.state.paragraph.line_spacing_multiple = control.parameter.unwrap_or(0) != 0
            }
            "hyphauto" => {
                self.default_paragraph_style.auto_hyphenation = control.parameter.unwrap_or(1) != 0;
                self.state.paragraph.auto_hyphenation =
                    self.default_paragraph_style.auto_hyphenation;
                let message = if self.default_paragraph_style.auto_hyphenation {
                    "document hyphenation approximated by bounded passive soft hyphenation"
                } else {
                    "document hyphenation disabled"
                };
                self.diagnostics
                    .push(Diagnostic::warning(message, Some(offset)));
            }
            "hyphcaps" => {
                self.default_paragraph_style.hyphenate_caps = control.parameter.unwrap_or(1) != 0;
                self.state.paragraph.hyphenate_caps = self.default_paragraph_style.hyphenate_caps;
                let message = if self.default_paragraph_style.hyphenate_caps {
                    "capitalized word hyphenation enabled for passive automatic hyphenation"
                } else {
                    "capitalized word hyphenation disabled for passive automatic hyphenation"
                };
                self.diagnostics
                    .push(Diagnostic::warning(message, Some(offset)));
            }
            "hyphconsec" => {
                self.default_paragraph_style
                    .max_consecutive_hyphenated_lines =
                    self.clamp_consecutive_hyphenated_lines(control.parameter, offset);
                self.state.paragraph.max_consecutive_hyphenated_lines = self
                    .default_paragraph_style
                    .max_consecutive_hyphenated_lines;
                self.diagnostics.push(Diagnostic::warning(
                    "consecutive automatic hyphenation limit applied to passive layout",
                    Some(offset),
                ));
            }
            "hyphhotz" => {
                self.default_paragraph_style.hyphenation_zone_twips =
                    self.clamp_hyphenation_zone(control.parameter, offset);
                self.state.paragraph.hyphenation_zone_twips =
                    self.default_paragraph_style.hyphenation_zone_twips;
                self.diagnostics.push(Diagnostic::warning(
                    "hyphenation zone applied to bounded passive hyphenation",
                    Some(offset),
                ));
            }
            "hyphpar" => {
                self.state.paragraph.auto_hyphenation = control.parameter.unwrap_or(1) != 0;
                let message = if self.state.paragraph.auto_hyphenation {
                    "paragraph hyphenation approximated by bounded passive soft hyphenation"
                } else {
                    "paragraph hyphenation disabled"
                };
                self.diagnostics
                    .push(Diagnostic::warning(message, Some(offset)));
            }
            "formshade" => {
                self.form_field_shading = control.parameter.unwrap_or(1) != 0;
                self.diagnostics.push(Diagnostic::warning(
                    "form-field shading approximated by passive highlight rectangles",
                    Some(offset),
                ));
            }
            "ftnbj" => self.set_footnote_placement(FootnotePlacement::BeneathText, offset),
            "sftnbj" => self.set_footnote_placement(FootnotePlacement::BottomOfPage, offset),
            "aenddoc" => self.set_endnote_placement(EndnotePlacement::EndOfDocument, offset),
            "endnhere" => self.set_endnote_placement(EndnotePlacement::EndOfSection, offset),
            "fet" => self.diagnostics.push(Diagnostic::warning(
                "note placement control approximated by passive note layout",
                Some(offset),
            )),
            "linex" => self.set_line_number_distance(control.parameter, offset),
            "linemod" => self.set_line_number_step(control.parameter, offset),
            "linestarts" => self.set_line_number_start(control.parameter, offset),
            "linerestart" => self.set_line_number_restart(LineNumberRestart::Section, offset),
            "lineppage" => self.set_line_number_restart(LineNumberRestart::Page, offset),
            "linecont" => self.set_line_number_restart(LineNumberRestart::Continuous, offset),
            "tldot" => self.state.current_tab_leader = TabLeader::Dots,
            "tlhyph" => self.state.current_tab_leader = TabLeader::Hyphens,
            "tlul" | "tlth" | "tluldb" => self.state.current_tab_leader = TabLeader::Underline,
            "tlmdot" => self.state.current_tab_leader = TabLeader::MiddleDots,
            "tleq" => self.state.current_tab_leader = TabLeader::Equals,
            "tlnone" => self.state.current_tab_leader = TabLeader::None,
            "tql" => self.state.current_tab_alignment = TabAlignment::Left,
            "tqc" => self.state.current_tab_alignment = TabAlignment::Center,
            "tqr" => self.state.current_tab_alignment = TabAlignment::Right,
            "tqdec" => self.state.current_tab_alignment = TabAlignment::Decimal,
            "tb" => self.state.current_tab_alignment = TabAlignment::Bar,
            "tx" => self.push_tab_stop(control.parameter, offset)?,
            "deftab" => {
                self.document.default_tab_width_twips =
                    self.clamp_default_tab_width(control.parameter, offset)
            }
            "pard" => {
                if self.current_table.is_some() && self.current_table_row.is_none() {
                    self.finish_table(offset)?;
                }
                self.state.paragraph = self.default_paragraph_style.clone();
                self.state.current_tab_leader = TabLeader::None;
                self.state.current_tab_alignment = TabAlignment::Left;
                self.state.list_override_index = None;
                self.state.list_level_index = 0;
                self.state.paragraph_style_index = None;
            }
            "paperw" | "pgwsxn" => {
                self.current_section_page.width_twips = self.clamp_page_dimension(
                    control.parameter,
                    PageSettings::default().width_twips,
                    "paper width",
                    offset,
                );
                self.upsert_current_section_settings();
            }
            "paperh" | "pghsxn" => {
                self.current_section_page.height_twips = self.clamp_page_dimension(
                    control.parameter,
                    PageSettings::default().height_twips,
                    "paper height",
                    offset,
                );
                self.upsert_current_section_settings();
            }
            "margl" | "marglsxn" => {
                self.current_section_page.margin_left_twips = self.clamp_page_margin(
                    control.parameter,
                    PageSettings::default().margin_left_twips,
                    "left margin",
                    offset,
                );
                self.upsert_current_section_settings();
            }
            "margr" | "margrsxn" => {
                self.current_section_page.margin_right_twips = self.clamp_page_margin(
                    control.parameter,
                    PageSettings::default().margin_right_twips,
                    "right margin",
                    offset,
                );
                self.upsert_current_section_settings();
            }
            "margt" | "margtsxn" => {
                self.current_section_page.margin_top_twips = self.clamp_page_margin(
                    control.parameter,
                    PageSettings::default().margin_top_twips,
                    "top margin",
                    offset,
                );
                self.upsert_current_section_settings();
            }
            "margb" | "margbsxn" => {
                self.current_section_page.margin_bottom_twips = self.clamp_page_margin(
                    control.parameter,
                    PageSettings::default().margin_bottom_twips,
                    "bottom margin",
                    offset,
                );
                self.upsert_current_section_settings();
            }
            "gutter" | "guttersxn" => {
                self.current_section_page.gutter_twips =
                    self.clamp_page_gutter(control.parameter, offset);
                self.upsert_current_section_settings();
            }
            "facingp" | "margmirror" => {
                self.current_section_page.mirror_margins = control.parameter.unwrap_or(1) != 0;
                self.upsert_current_section_settings();
            }
            "rtlgutter" | "rtlguttersxn" => {
                self.current_section_page.gutter_on_right = control.parameter.unwrap_or(1) != 0;
                self.upsert_current_section_settings();
            }
            "headery" | "headerysxn" => {
                self.current_section_page.header_distance_twips = self
                    .clamp_header_footer_distance(
                        control.parameter,
                        PageSettings::default().header_distance_twips,
                        "header distance",
                        offset,
                    );
                self.upsert_current_section_settings();
            }
            "footery" | "footerysxn" => {
                self.current_section_page.footer_distance_twips = self
                    .clamp_header_footer_distance(
                        control.parameter,
                        PageSettings::default().footer_distance_twips,
                        "footer distance",
                        offset,
                    );
                self.upsert_current_section_settings();
            }
            "landscape" | "lndscpsxn" => {
                self.current_section_page.landscape = true;
                self.upsert_current_section_settings();
            }
            "cols" => {
                self.current_section_page.column_count =
                    self.clamp_section_columns(control.parameter, offset);
                self.current_section_column_index = 0;
                self.current_section_page.column_widths_twips.clear();
                self.current_section_page.column_gaps_twips.clear();
                self.upsert_current_section_settings();
            }
            "colsx" => {
                self.current_section_page.column_gap_twips =
                    self.clamp_column_gap(control.parameter, offset);
                self.upsert_current_section_settings();
            }
            "colno" => {
                self.current_section_column_index =
                    self.clamp_section_column_index(control.parameter, offset);
            }
            "colw" => {
                let width = self.clamp_section_column_width(control.parameter, offset);
                self.set_current_section_column_width(width);
                self.upsert_current_section_settings();
            }
            "colsr" => {
                let gap = self.clamp_column_gap(control.parameter, offset);
                self.set_current_section_column_gap(gap);
                self.upsert_current_section_settings();
            }
            "linebetcol" => {
                self.current_section_page.line_between_columns =
                    control.parameter.unwrap_or(1) != 0;
                self.upsert_current_section_settings();
            }
            "titlepg" => {
                self.current_section_page.title_page = control.parameter.unwrap_or(1) != 0;
                self.upsert_current_section_settings();
            }
            "vertalt" => {
                self.current_section_page.vertical_alignment = PageVerticalAlignment::Top;
                self.upsert_current_section_settings();
            }
            "vertalc" => {
                self.current_section_page.vertical_alignment = PageVerticalAlignment::Center;
                self.upsert_current_section_settings();
            }
            "vertalb" => {
                self.current_section_page.vertical_alignment = PageVerticalAlignment::Bottom;
                self.upsert_current_section_settings();
            }
            "pgnstarts" | "pgnstart" => {
                self.current_section_page.page_number_start =
                    Some(self.clamp_page_number_start(control.parameter, offset));
                self.upsert_current_section_settings();
            }
            "pgnrestart" => {
                self.current_section_page.page_number_start = if control.parameter.unwrap_or(1) == 0
                {
                    None
                } else {
                    Some(1)
                };
                self.upsert_current_section_settings();
            }
            "pgncont" => {
                self.current_section_page.page_number_start = None;
                self.upsert_current_section_settings();
            }
            "pgndec" => self.set_page_number_format(PageNumberFormat::Decimal),
            "pgnucrm" => self.set_page_number_format(PageNumberFormat::UpperRoman),
            "pgnlcrm" => self.set_page_number_format(PageNumberFormat::LowerRoman),
            "pgnucltr" => self.set_page_number_format(PageNumberFormat::UpperLetter),
            "pgnlcltr" => self.set_page_number_format(PageNumberFormat::LowerLetter),
            "ftnstart" => {
                self.document.footnote_number_start =
                    self.clamp_page_number_start(control.parameter, offset);
            }
            "aftnstart" => {
                self.document.endnote_number_start =
                    self.clamp_page_number_start(control.parameter, offset);
            }
            "ftnnar" => self.document.footnote_number_format = PageNumberFormat::Decimal,
            "ftnnruc" => self.document.footnote_number_format = PageNumberFormat::UpperRoman,
            "ftnnrlc" => self.document.footnote_number_format = PageNumberFormat::LowerRoman,
            "ftnnauc" => self.document.footnote_number_format = PageNumberFormat::UpperLetter,
            "ftnnalc" => self.document.footnote_number_format = PageNumberFormat::LowerLetter,
            "aftnnar" => self.document.endnote_number_format = PageNumberFormat::Decimal,
            "aftnnruc" => self.document.endnote_number_format = PageNumberFormat::UpperRoman,
            "aftnnrlc" => self.document.endnote_number_format = PageNumberFormat::LowerRoman,
            "aftnnauc" => self.document.endnote_number_format = PageNumberFormat::UpperLetter,
            "aftnnalc" => self.document.endnote_number_format = PageNumberFormat::LowerLetter,
            name if is_known_ignored_control(name) => {}
            name if self.state.destination == Destination::Ignored => {
                self.count_skipped_destination_bytes(name.len(), offset)?;
            }
            name if control_starts_group && destination_allows_visible_content(&self.state) => {
                self.diagnostics.push(Diagnostic::warning(
                    format!("unknown RTF destination '\\{name}' skipped"),
                    Some(offset),
                ));
                self.count_skipped_destination_bytes(name.len(), offset)?;
                self.state.destination = Destination::Ignored;
            }
            name => self.diagnostics.push(Diagnostic::warning(
                format!("unsupported RTF control '\\{name}'"),
                Some(offset),
            )),
        }
        Ok(())
    }

    fn apply_text(&mut self, text: &str, offset: usize) -> Result<(), ParseError> {
        self.state.at_group_start = false;
        if self.state.skip_bytes > 0 {
            let skip = self.state.skip_bytes.min(text.chars().count());
            self.state.skip_bytes -= skip;
            let remaining: String = text.chars().skip(skip).collect();
            if remaining.is_empty() {
                return Ok(());
            }
            return self.apply_text(&remaining, offset);
        }
        self.state.pending_unicode_high_surrogate = None;
        if self.state.skip_password_hash_payload {
            return self.apply_password_hash_payload_text(text, offset);
        }
        if self.state.capturing_form_default_text {
            return self.push_form_default_text(text, offset);
        }
        if self.state.capturing_form_dropdown_entry {
            return self.push_form_dropdown_text(text, offset);
        }
        if self.state.user_property_capture.is_some() {
            return self.push_user_property_text(text, offset);
        }
        if self.state.metadata_property.is_some() {
            return self.push_document_property_text(text, offset);
        }

        match self.state.destination {
            Destination::Body => {
                if self.current_table.is_some() && self.current_table_row.is_none() {
                    self.finish_table(offset)?;
                }
                if self.state.character.hidden {
                    self.count_skipped_destination_bytes(text.len(), offset)?;
                } else {
                    self.push_text(text, offset)?;
                }
            }
            Destination::ListText => {
                if self.state.character.hidden {
                    self.count_skipped_destination_bytes(text.len(), offset)?;
                } else {
                    self.push_list_marker_text(text, offset)?;
                }
            }
            Destination::Header
            | Destination::FirstPageHeader
            | Destination::EvenPageHeader
            | Destination::Footer
            | Destination::FirstPageFooter
            | Destination::EvenPageFooter
            | Destination::Footnote
            | Destination::Endnote => {
                if self.state.character.hidden {
                    self.count_skipped_destination_bytes(text.len(), offset)?;
                } else {
                    self.push_text(text, offset)?
                }
            }
            Destination::Picture => self.push_picture_hex_text(text, offset)?,
            Destination::FontTable => self.push_font_text(text, offset)?,
            Destination::FontAlternate => self.push_font_alternate_text(text, offset)?,
            Destination::ColorTable => self.push_color_text(text, offset)?,
            Destination::ListTable if self.state.list_context == ListContext::ListLevelText => {
                self.push_list_level_text(text, offset)?
            }
            Destination::ListOverrideTable
                if self.state.list_context == ListContext::ListLevelText =>
            {
                self.push_list_level_text(text, offset)?
            }
            Destination::FieldInstruction => self.push_field_instruction_text(text, offset)?,
            Destination::BookmarkStart | Destination::BookmarkEnd => {
                self.push_bookmark_name_text(text, offset)?
            }
            Destination::StyleSheet
            | Destination::ListTable
            | Destination::ListOverrideTable
            | Destination::Shape
            | Destination::Ignored
            | Destination::Metadata
            | Destination::ObjectData => {
                self.count_skipped_destination_bytes(text.len(), offset)?;
            }
        }
        Ok(())
    }

    fn apply_password_hash_payload_text(
        &mut self,
        text: &str,
        offset: usize,
    ) -> Result<(), ParseError> {
        let mut seen_hex = false;
        let mut consumed = text.len();
        'scan: for (index, ch) in text.char_indices() {
            if ch.is_ascii_hexdigit() {
                seen_hex = true;
                continue;
            }
            if ch.is_ascii_whitespace() {
                if seen_hex {
                    consumed = index + ch.len_utf8();
                    let tail_start = consumed;
                    for (next_index, next_ch) in text[tail_start..].char_indices() {
                        if !next_ch.is_ascii_whitespace() {
                            consumed = tail_start + next_index;
                            break 'scan;
                        }
                    }
                    consumed = text.len();
                    self.state.skip_password_hash_payload = false;
                    break;
                }
                continue;
            }

            consumed = index;
            self.state.skip_password_hash_payload = false;
            break;
        }

        self.count_skipped_destination_bytes(consumed, offset)?;
        if consumed < text.len() {
            self.apply_text(&text[consumed..], offset)?;
        }
        Ok(())
    }

    fn apply_hex_byte(&mut self, byte: u8, offset: usize) -> Result<(), ParseError> {
        self.state.at_group_start = false;
        if self.state.skip_bytes > 0 {
            self.state.skip_bytes -= 1;
            return Ok(());
        }
        if self.state.skip_password_hash_payload {
            self.count_skipped_destination_bytes(1, offset)?;
            return Ok(());
        }
        self.state.pending_unicode_high_surrogate = None;
        if self.state.capturing_form_default_text {
            let ch = self.decode_text_hex_byte(byte);
            return self.push_form_default_text(&ch.to_string(), offset);
        }
        if self.state.capturing_form_dropdown_entry {
            let ch = self.decode_text_hex_byte(byte);
            return self.push_form_dropdown_text(&ch.to_string(), offset);
        }
        if self.state.user_property_capture.is_some() {
            let ch = self.decode_text_hex_byte(byte);
            return self.push_user_property_text(&ch.to_string(), offset);
        }
        if self.state.metadata_property.is_some() {
            let ch = self.decode_text_hex_byte(byte);
            return self.push_document_property_text(&ch.to_string(), offset);
        }

        match self.state.destination {
            Destination::Body => {
                if self.current_table.is_some() && self.current_table_row.is_none() {
                    self.finish_table(offset)?;
                }
                if self.state.character.hidden {
                    self.count_skipped_destination_bytes(1, offset)?;
                } else {
                    let ch = self.decode_text_hex_byte(byte);
                    self.push_text(&ch.to_string(), offset)?;
                }
            }
            Destination::ListText => {
                if self.state.character.hidden {
                    self.count_skipped_destination_bytes(1, offset)?;
                } else {
                    let ch = self.decode_text_hex_byte(byte);
                    self.push_list_marker_text(&ch.to_string(), offset)?;
                }
            }
            Destination::Header
            | Destination::FirstPageHeader
            | Destination::EvenPageHeader
            | Destination::Footer
            | Destination::FirstPageFooter
            | Destination::EvenPageFooter
            | Destination::Footnote
            | Destination::Endnote => {
                if self.state.character.hidden {
                    self.count_skipped_destination_bytes(1, offset)?;
                } else {
                    let ch = self.decode_text_hex_byte(byte);
                    self.push_text(&ch.to_string(), offset)?;
                }
            }
            Destination::Picture => self.push_picture_bytes(&[byte], offset)?,
            Destination::FontTable => {
                let ch = decode_hex_byte(byte, self.state.code_page);
                self.push_font_text(&ch.to_string(), offset)?;
            }
            Destination::FontAlternate => {
                let ch = decode_hex_byte(byte, self.state.code_page);
                self.push_font_alternate_text(&ch.to_string(), offset)?;
            }
            Destination::ListTable if self.state.list_context == ListContext::ListLevelText => {
                let ch = decode_hex_byte(byte, self.state.code_page);
                self.push_list_level_text(&ch.to_string(), offset)?;
            }
            Destination::ListOverrideTable
                if self.state.list_context == ListContext::ListLevelText =>
            {
                let ch = decode_hex_byte(byte, self.state.code_page);
                self.push_list_level_text(&ch.to_string(), offset)?;
            }
            Destination::FieldInstruction => {
                let ch = decode_hex_byte(byte, self.state.code_page);
                self.push_field_instruction_text(&ch.to_string(), offset)?;
            }
            Destination::BookmarkStart | Destination::BookmarkEnd => {
                let ch = decode_hex_byte(byte, self.state.code_page);
                self.push_bookmark_name_text(&ch.to_string(), offset)?;
            }
            Destination::ColorTable
            | Destination::StyleSheet
            | Destination::ListTable
            | Destination::ListOverrideTable
            | Destination::Shape
            | Destination::Ignored
            | Destination::Metadata
            | Destination::ObjectData => {
                self.count_skipped_destination_bytes(1, offset)?;
            }
        }
        Ok(())
    }

    fn apply_binary(&mut self, bytes: &[u8], offset: usize) -> Result<(), ParseError> {
        self.state.at_group_start = false;
        self.state.pending_unicode_high_surrogate = None;
        if self.state.destination == Destination::Picture {
            return self.push_picture_bytes(bytes, offset);
        }
        if self.options.active_content_policy == ActiveContentPolicy::Reject {
            return Err(ParseError::ActiveContentRejected {
                feature: "binary RTF payload".to_string(),
                offset,
            });
        }
        self.count_skipped_destination_bytes(bytes.len(), offset)?;
        self.diagnostics.push(Diagnostic::warning(
            "binary RTF payload stripped before document normalization",
            Some(offset),
        ));
        Ok(())
    }

    fn clamp_page_dimension(
        &mut self,
        value: Option<i32>,
        default: i32,
        label: &str,
        offset: usize,
    ) -> i32 {
        self.clamp_page_value(
            value.unwrap_or(default),
            self.limits().min_page_dimension_twips,
            self.limits().max_page_dimension_twips,
            label,
            offset,
        )
    }

    fn clamp_page_margin(
        &mut self,
        value: Option<i32>,
        default: i32,
        label: &str,
        offset: usize,
    ) -> i32 {
        self.clamp_page_value(
            value.unwrap_or(default),
            0,
            self.limits().max_page_margin_twips,
            label,
            offset,
        )
    }

    fn clamp_header_footer_distance(
        &mut self,
        value: Option<i32>,
        default: i32,
        label: &str,
        offset: usize,
    ) -> i32 {
        self.clamp_page_value(
            value.unwrap_or(default),
            0,
            self.limits().max_header_footer_distance_twips,
            label,
            offset,
        )
    }

    fn clamp_page_gutter(&mut self, value: Option<i32>, offset: usize) -> i32 {
        self.clamp_page_value(
            value.unwrap_or(0),
            0,
            self.limits().max_page_gutter_twips,
            "page gutter",
            offset,
        )
    }

    fn clamp_section_columns(&mut self, value: Option<i32>, offset: usize) -> usize {
        let value = value.unwrap_or(1).max(1) as usize;
        let clamped = value.min(self.limits().max_section_columns.max(1));
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("section columns clamped from {value} to {clamped}"),
                Some(offset),
            ));
        }
        clamped
    }

    fn clamp_section_column_index(&mut self, value: Option<i32>, offset: usize) -> usize {
        let value = value.unwrap_or(1).max(1) as usize;
        let clamped = value.min(self.current_section_page.column_count.max(1));
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("section column index clamped from {value} to {clamped}"),
                Some(offset),
            ));
        }
        clamped.saturating_sub(1)
    }

    fn clamp_section_column_width(&mut self, value: Option<i32>, offset: usize) -> i32 {
        self.clamp_page_value(
            value.unwrap_or(0),
            0,
            self.limits().max_page_dimension_twips,
            "column width",
            offset,
        )
    }

    fn clamp_column_gap(&mut self, value: Option<i32>, offset: usize) -> i32 {
        self.clamp_page_value(
            value.unwrap_or(PageSettings::default().column_gap_twips),
            0,
            self.limits().max_column_gap_twips,
            "column gap",
            offset,
        )
    }

    fn set_current_section_column_width(&mut self, width_twips: i32) {
        let index = self.current_section_column_index;
        resize_column_vector(&mut self.current_section_page.column_widths_twips, index, 0);
        self.current_section_page.column_widths_twips[index] = width_twips;
    }

    fn set_current_section_column_gap(&mut self, gap_twips: i32) {
        let index = self.current_section_column_index;
        resize_column_vector(
            &mut self.current_section_page.column_gaps_twips,
            index,
            self.current_section_page.column_gap_twips,
        );
        self.current_section_page.column_gaps_twips[index] = gap_twips;
    }

    fn clamp_default_tab_width(&mut self, value: Option<i32>, offset: usize) -> i32 {
        let value = value.unwrap_or(Document::default().default_tab_width_twips);
        let clamped = value.clamp(1, self.limits().max_tab_stop_twips.max(1));
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("default tab width clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        clamped
    }

    fn clamp_page_number_start(&mut self, value: Option<i32>, offset: usize) -> i32 {
        let value = value.unwrap_or(1).max(1);
        let max = self.limits().max_page_number_start.max(1);
        let clamped = value.min(max);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("page number start clamped from {value} to {clamped}"),
                Some(offset),
            ));
        }
        clamped
    }

    fn set_page_number_format(&mut self, format: PageNumberFormat) {
        self.current_section_page.page_number_format = Some(format);
        self.upsert_current_section_settings();
    }

    fn set_line_number_start(&mut self, value: Option<i32>, offset: usize) {
        self.enable_line_numbering(offset);
        self.current_section_page.line_numbering.start =
            self.clamp_page_number_start(value, offset);
        self.upsert_current_section_settings();
    }

    fn set_line_number_step(&mut self, value: Option<i32>, offset: usize) {
        self.enable_line_numbering(offset);
        self.current_section_page.line_numbering.step = self.clamp_page_number_start(value, offset);
        self.upsert_current_section_settings();
    }

    fn set_line_number_distance(&mut self, value: Option<i32>, offset: usize) {
        self.enable_line_numbering(offset);
        let value = value.unwrap_or(self.current_section_page.line_numbering.distance_twips);
        self.current_section_page.line_numbering.distance_twips = self.clamp_page_value(
            value,
            0,
            self.limits().max_page_margin_twips.max(0),
            "line number distance",
            offset,
        );
        self.upsert_current_section_settings();
    }

    fn set_line_number_restart(&mut self, restart: LineNumberRestart, offset: usize) {
        self.enable_line_numbering(offset);
        self.current_section_page.line_numbering.restart = restart;
        self.upsert_current_section_settings();
    }

    fn enable_line_numbering(&mut self, offset: usize) {
        if !self.current_section_page.line_numbering.enabled {
            self.diagnostics.push(Diagnostic::warning(
                "line numbering approximated by passive margin text",
                Some(offset),
            ));
        }
        self.current_section_page.line_numbering.enabled = true;
    }

    fn set_footnote_placement(&mut self, placement: FootnotePlacement, offset: usize) {
        self.document.footnote_placement = placement;
        let message = match placement {
            FootnotePlacement::BeneathText => {
                "footnote placement rendered beneath document text as passive note layout"
            }
            FootnotePlacement::BottomOfPage => {
                "footnote bottom-of-page placement approximated by passive note layout"
            }
        };
        self.diagnostics
            .push(Diagnostic::warning(message, Some(offset)));
    }

    fn set_endnote_placement(&mut self, placement: EndnotePlacement, offset: usize) {
        self.document.endnote_placement = placement;
        let message = match placement {
            EndnotePlacement::EndOfDocument => {
                "endnotes placed on passive final page without active note behavior"
            }
            EndnotePlacement::EndOfSection => {
                "endnote section placement approximated by passive note layout"
            }
            EndnotePlacement::AfterBody => "endnote placement rendered after body text",
        };
        self.diagnostics
            .push(Diagnostic::warning(message, Some(offset)));
    }

    fn clamp_paragraph_spacing(&mut self, value: Option<i32>, label: &str, offset: usize) -> i32 {
        let value = value.unwrap_or(0).max(0);
        let max = self.limits().max_paragraph_spacing_twips.max(0);
        let clamped = value.min(max);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("{label} clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        clamped
    }

    fn clamp_paragraph_indent(&mut self, value: Option<i32>, label: &str, offset: usize) -> i32 {
        let value = value.unwrap_or(0);
        let max = self.limits().max_paragraph_indent_twips.max(0);
        let clamped = value.clamp(-max, max);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("{label} clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        clamped
    }

    fn set_drop_cap_lines(&mut self, value: Option<i32>, offset: usize) {
        let value = value.unwrap_or(0).max(0);
        let clamped = value.min(10);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("drop cap lines clamped from {value} to {clamped}"),
                Some(offset),
            ));
        }
        self.state.paragraph.drop_cap_lines = clamped;
    }

    fn set_drop_cap_type(&mut self, value: Option<i32>) {
        let value = value.unwrap_or(0);
        if value <= 0 {
            self.state.paragraph.drop_cap_lines = 0;
        } else if self.state.paragraph.drop_cap_lines == 0 {
            self.state.paragraph.drop_cap_lines = 3;
        }
    }

    fn clamp_line_spacing(&mut self, value: Option<i32>, offset: usize) -> Option<i32> {
        let value = value.unwrap_or(0);
        if value == 0 {
            return None;
        }
        let max = self.limits().max_line_spacing_twips;
        let clamped = value.clamp(-max, max);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("line spacing clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        Some(clamped)
    }

    fn push_tab_stop(&mut self, value: Option<i32>, offset: usize) -> Result<(), ParseError> {
        let leader = self.state.current_tab_leader;
        let alignment = self.state.current_tab_alignment;
        self.insert_paragraph_tab_stop(value, "tab stop", offset, leader, alignment)?;
        self.state.current_tab_leader = TabLeader::None;
        self.state.current_tab_alignment = TabAlignment::Left;
        Ok(())
    }

    fn insert_paragraph_tab_stop(
        &mut self,
        value: Option<i32>,
        label: &str,
        offset: usize,
        leader: TabLeader,
        alignment: TabAlignment,
    ) -> Result<(), ParseError> {
        let value = value.unwrap_or(0).max(0);
        let clamped = value.min(self.limits().max_tab_stop_twips);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("{label} clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        let max_tab_stops = self.limits().max_tab_stops;
        let paragraph = &mut self.state.paragraph;
        if let Some(existing_idx) = paragraph
            .tab_stops_twips
            .iter()
            .position(|stop| *stop == clamped)
        {
            if let Some(existing_leader) = paragraph.tab_stop_leaders.get_mut(existing_idx) {
                *existing_leader = leader;
            }
            if let Some(existing_alignment) = paragraph.tab_stop_alignments.get_mut(existing_idx) {
                *existing_alignment = alignment;
            }
        } else {
            if paragraph.tab_stops_twips.len() >= max_tab_stops {
                return Err(ParseError::ResourceLimitExceeded {
                    resource: "tab stops".to_string(),
                    offset,
                });
            }
            paragraph.tab_stops_twips.push(clamped);
            paragraph.tab_stop_leaders.push(leader);
            paragraph.tab_stop_alignments.push(alignment);
            let mut stops = paragraph
                .tab_stops_twips
                .iter()
                .copied()
                .zip(paragraph.tab_stop_leaders.iter().copied())
                .zip(paragraph.tab_stop_alignments.iter().copied())
                .map(|((stop, leader), alignment)| (stop, leader, alignment))
                .collect::<Vec<_>>();
            stops.sort_by_key(|(stop, _, _)| *stop);
            paragraph.tab_stops_twips = stops.iter().map(|(stop, _, _)| *stop).collect();
            paragraph.tab_stop_leaders = stops.iter().map(|(_, leader, _)| *leader).collect();
            paragraph.tab_stop_alignments =
                stops.iter().map(|(_, _, alignment)| *alignment).collect();
        }
        Ok(())
    }

    fn upsert_current_section_settings(&mut self) {
        let page = normalized_page_settings(self.current_section_page.clone());
        if !self.has_started_visible_body() {
            self.document.page = page;
            return;
        }

        match self.document.blocks.last_mut() {
            Some(Block::SectionSettings(existing)) => *existing = page,
            _ => self.document.blocks.push(Block::SectionSettings(page)),
        }
    }

    fn has_started_visible_body(&self) -> bool {
        if !self.current_paragraph.runs.is_empty() {
            return true;
        }

        self.document.blocks.iter().any(|block| match block {
            Block::Paragraph(paragraph) => !paragraph.runs.is_empty(),
            Block::Table(table) => !table.rows.is_empty(),
            Block::Image(_)
            | Block::Shape(_)
            | Block::Placeholder(_)
            | Block::PageBreak
            | Block::ContinuousSectionBreak
            | Block::SectionBreak
            | Block::EvenPageSectionBreak
            | Block::OddPageSectionBreak
            | Block::ColumnBreak => true,
            Block::SectionSettings(_) => false,
        })
    }

    fn normalize_page_orientation(&mut self) {
        self.document.page = normalized_page_settings(self.document.page.clone());
        self.current_section_page = normalized_page_settings(self.current_section_page.clone());
        for block in &mut self.document.blocks {
            if let Block::SectionSettings(page) = block {
                *page = normalized_page_settings(page.clone());
            }
        }
    }

    fn clamp_page_value(
        &mut self,
        value: i32,
        min: i32,
        max: i32,
        label: &str,
        offset: usize,
    ) -> i32 {
        let clamped = value.clamp(min, max);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("{label} clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        clamped
    }

    fn clamp_font_size(&mut self, value: i32, offset: usize) -> i32 {
        let clamped = value.clamp(2, self.limits().max_font_size_half_points.max(2));
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("font size clamped from {value} to {clamped} half-points"),
                Some(offset),
            ));
        }
        clamped
    }

    fn clamp_unicode_fallback_skip(&mut self, value: i32, offset: usize) -> usize {
        let normalized = value.max(0) as usize;
        let limit = self.limits().max_unicode_fallback_skip;
        let clamped = normalized.min(limit);
        if clamped != normalized {
            self.diagnostics.push(Diagnostic::warning(
                format!("Unicode fallback skip clamped from {normalized} to {clamped} characters"),
                Some(offset),
            ));
        }
        clamped
    }

    fn clamp_consecutive_hyphenated_lines(
        &mut self,
        value: Option<i32>,
        offset: usize,
    ) -> Option<usize> {
        let value = value.unwrap_or(0);
        if value <= 0 {
            return None;
        }
        let limit = self.limits().max_hyphenation_consecutive_lines.max(1);
        let normalized = value as usize;
        let clamped = normalized.min(limit);
        if clamped != normalized {
            self.diagnostics.push(Diagnostic::warning(
                format!("consecutive hyphenation limit clamped from {normalized} to {clamped}"),
                Some(offset),
            ));
        }
        Some(clamped)
    }

    fn clamp_hyphenation_zone(&mut self, value: Option<i32>, offset: usize) -> i32 {
        let value = value.unwrap_or(ParagraphStyle::default().hyphenation_zone_twips);
        let limit = self.limits().max_hyphenation_zone_twips.max(0);
        let clamped = value.clamp(0, limit);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("hyphenation zone clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        clamped
    }

    fn clamp_character_spacing(&mut self, value: i32, offset: usize) -> i32 {
        let limit = self.limits().max_character_spacing_twips.max(0);
        let clamped = value.clamp(-limit, limit);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("character spacing clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        clamped
    }

    fn clamp_character_kerning(&mut self, value: i32, offset: usize) -> i32 {
        let clamped = value.clamp(0, self.limits().max_font_size_half_points.max(0));
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!(
                    "character kerning threshold clamped from {value} to {clamped} half-points"
                ),
                Some(offset),
            ));
        }
        clamped
    }

    fn reset_script_position(&mut self) {
        self.state.character.baseline_shift_half_points = 0;
        self.state.character.font_size_scale_percent = 100;
    }

    fn clamp_character_scaling(&mut self, value: i32, offset: usize) -> i32 {
        let min = self.limits().min_character_scaling_percent.max(1);
        let max = self.limits().max_character_scaling_percent.max(min);
        let clamped = value.clamp(min, max);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("character scaling clamped from {value}% to {clamped}%"),
                Some(offset),
            ));
        }
        clamped
    }

    fn clamp_character_shading(&mut self, value: i32, offset: usize) -> i32 {
        let clamped = Self::clamped_shading_basis(value);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("character shading clamped from {value} to {clamped}"),
                Some(offset),
            ));
        }
        clamped
    }

    fn clamp_shading_basis(&mut self, value: i32, label: &str, offset: usize) -> i32 {
        let clamped = Self::clamped_shading_basis(value);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("{label} shading clamped from {value} to {clamped}"),
                Some(offset),
            ));
        }
        clamped
    }

    fn clamped_shading_basis(value: i32) -> i32 {
        value.clamp(0, 10_000)
    }

    fn start_note_destination(
        &mut self,
        destination: Destination,
        offset: usize,
    ) -> Result<(), ParseError> {
        let replacement = match destination {
            Destination::Footnote => {
                self.footnote_reference_count = self.footnote_reference_count.saturating_add(1);
                format_note_number(
                    self.document.footnote_number_start,
                    self.footnote_reference_count,
                    self.document.footnote_number_format,
                )
            }
            Destination::Endnote => {
                self.endnote_reference_count = self.endnote_reference_count.saturating_add(1);
                format_note_number(
                    self.document.endnote_number_start,
                    self.endnote_reference_count,
                    self.document.endnote_number_format,
                )
            }
            _ => "1".to_string(),
        };
        self.resolve_latest_pending_note_reference(&replacement, offset)?;
        self.state.destination = destination;
        Ok(())
    }

    fn push_note_reference(&mut self, offset: usize) -> Result<(), ParseError> {
        if self.state.character.hidden {
            self.count_skipped_destination_bytes(1, offset)?;
            return Ok(());
        }

        if matches!(
            self.state.destination,
            Destination::Footnote | Destination::Endnote
        ) {
            return Ok(());
        }

        self.output_text_chars = self
            .output_text_chars
            .checked_add(PENDING_NOTE_REFERENCE_MARKER.chars().count())
            .ok_or(ParseError::OutputTextTooLarge(offset))?;
        if self.output_text_chars > self.limits().max_output_text_chars {
            return Err(ParseError::OutputTextTooLarge(offset));
        }

        let mut style = self.state.character.clone();
        style.baseline_shift_half_points = DEFAULT_SUPERSCRIPT_SHIFT_HALF_POINTS;
        style.font_size_scale_percent = DEFAULT_SCRIPT_FONT_SCALE_PERCENT;

        let paragraph = if self.state.destination == Destination::Header {
            &mut self.current_header_paragraph
        } else if self.state.destination == Destination::FirstPageHeader {
            &mut self.current_first_page_header_paragraph
        } else if self.state.destination == Destination::EvenPageHeader {
            &mut self.current_even_page_header_paragraph
        } else if self.state.destination == Destination::Footer {
            &mut self.current_footer_paragraph
        } else if self.state.destination == Destination::FirstPageFooter {
            &mut self.current_first_page_footer_paragraph
        } else if self.state.destination == Destination::EvenPageFooter {
            &mut self.current_even_page_footer_paragraph
        } else if let Some(row) = self.current_table_row.as_mut() {
            row.cell_open = true;
            &mut row.current_cell_paragraph
        } else {
            &mut self.current_paragraph
        };

        push_text_to_paragraph(
            paragraph,
            PENDING_NOTE_REFERENCE_MARKER,
            &self.state.paragraph,
            &style,
        );
        Ok(())
    }

    fn resolve_latest_pending_note_reference(
        &mut self,
        replacement: &str,
        offset: usize,
    ) -> Result<(), ParseError> {
        if replacement.chars().count() > PENDING_NOTE_REFERENCE_MARKER.chars().count() {
            let extra = replacement
                .chars()
                .count()
                .saturating_sub(PENDING_NOTE_REFERENCE_MARKER.chars().count());
            self.output_text_chars = self
                .output_text_chars
                .checked_add(extra)
                .ok_or(ParseError::OutputTextTooLarge(offset))?;
            if self.output_text_chars > self.limits().max_output_text_chars {
                return Err(ParseError::OutputTextTooLarge(offset));
            }
        }

        if let Some(row) = self.current_table_row.as_mut()
            && replace_last_pending_note_marker_in_paragraph(
                &mut row.current_cell_paragraph,
                &replacement,
            )
        {
            return Ok(());
        }

        if replace_last_pending_note_marker_in_paragraph(&mut self.current_paragraph, &replacement)
            || replace_last_pending_note_marker_in_paragraph(
                &mut self.current_header_paragraph,
                &replacement,
            )
            || replace_last_pending_note_marker_in_paragraph(
                &mut self.current_first_page_header_paragraph,
                &replacement,
            )
            || replace_last_pending_note_marker_in_paragraph(
                &mut self.current_even_page_header_paragraph,
                &replacement,
            )
            || replace_last_pending_note_marker_in_paragraph(
                &mut self.current_footer_paragraph,
                &replacement,
            )
            || replace_last_pending_note_marker_in_paragraph(
                &mut self.current_first_page_footer_paragraph,
                &replacement,
            )
            || replace_last_pending_note_marker_in_paragraph(
                &mut self.current_even_page_footer_paragraph,
                &replacement,
            )
        {
            return Ok(());
        }

        for block in self.document.blocks.iter_mut().rev() {
            if replace_last_pending_note_marker_in_block(block, &replacement) {
                return Ok(());
            }
        }
        for paragraphs in [
            &mut self.document.header,
            &mut self.document.first_page_header,
            &mut self.document.even_page_header,
            &mut self.document.footer,
            &mut self.document.first_page_footer,
            &mut self.document.even_page_footer,
            &mut self.document.footnotes,
            &mut self.document.endnotes,
        ] {
            if replace_last_pending_note_marker_in_paragraphs(paragraphs, &replacement) {
                return Ok(());
            }
        }

        Ok(())
    }

    fn resolve_unmatched_note_reference_markers(&mut self) {
        replace_all_pending_note_markers_in_paragraph(&mut self.current_paragraph, "*");
        replace_all_pending_note_markers_in_paragraph(&mut self.current_header_paragraph, "*");
        replace_all_pending_note_markers_in_paragraph(
            &mut self.current_first_page_header_paragraph,
            "*",
        );
        replace_all_pending_note_markers_in_paragraph(
            &mut self.current_even_page_header_paragraph,
            "*",
        );
        replace_all_pending_note_markers_in_paragraph(&mut self.current_footer_paragraph, "*");
        replace_all_pending_note_markers_in_paragraph(
            &mut self.current_first_page_footer_paragraph,
            "*",
        );
        replace_all_pending_note_markers_in_paragraph(
            &mut self.current_even_page_footer_paragraph,
            "*",
        );
        replace_all_pending_note_markers_in_paragraph(&mut self.current_footnote_paragraph, "*");
        replace_all_pending_note_markers_in_paragraph(&mut self.current_endnote_paragraph, "*");
        if let Some(row) = self.current_table_row.as_mut() {
            replace_all_pending_note_markers_in_paragraph(&mut row.current_cell_paragraph, "*");
        }
        for block in &mut self.document.blocks {
            replace_all_pending_note_markers_in_block(block, "*");
        }
        for paragraphs in [
            &mut self.document.header,
            &mut self.document.first_page_header,
            &mut self.document.even_page_header,
            &mut self.document.footer,
            &mut self.document.first_page_footer,
            &mut self.document.even_page_footer,
            &mut self.document.footnotes,
            &mut self.document.endnotes,
        ] {
            replace_all_pending_note_markers_in_paragraphs(paragraphs, "*");
        }
    }

    fn push_text(&mut self, text: &str, offset: usize) -> Result<(), ParseError> {
        if text.is_empty() {
            return Ok(());
        }
        if self.state.character.hidden {
            self.count_skipped_destination_bytes(text.len(), offset)?;
            return Ok(());
        }
        let pending_marker = self.take_pending_or_synthesized_list_marker(offset)?;
        let marker_run_chars = pending_marker
            .as_ref()
            .map(|marker| {
                marker
                    .runs
                    .iter()
                    .map(|run| run.text.chars().count())
                    .sum::<usize>()
            })
            .unwrap_or(0);
        let styled_marker = pending_marker.as_ref().and_then(|marker| {
            if marker.runs.is_empty() {
                marker
                    .character_style
                    .clone()
                    .map(|style| (marker.text.clone(), style))
            } else {
                None
            }
        });
        let synthesized_text;
        let text = if let Some(marker) = pending_marker
            .as_ref()
            .filter(|marker| marker.runs.is_empty() && marker.character_style.is_none())
        {
            synthesized_text = format!("{}{text}", marker.text);
            synthesized_text.as_str()
        } else {
            text
        };
        let sanitized_text;
        let text = if is_internal_marker(text) {
            text
        } else if contains_internal_marker(text) {
            sanitized_text = sanitize_internal_markers(text);
            &sanitized_text
        } else {
            text
        };
        let font_mapped_text;
        let text = if !contains_internal_marker(text)
            && let Some(mapped) = self.map_symbol_like_font_text(text)
        {
            font_mapped_text = mapped;
            font_mapped_text.as_str()
        } else {
            text
        };

        let styled_marker_chars = marker_run_chars
            .checked_add(
                styled_marker
                    .as_ref()
                    .map(|(marker_text, _)| marker_text.chars().count())
                    .unwrap_or(0),
            )
            .ok_or(ParseError::OutputTextTooLarge(offset))?;
        let output_text_chars = styled_marker_chars
            .checked_add(text.chars().count())
            .ok_or(ParseError::OutputTextTooLarge(offset))?;
        self.output_text_chars = self
            .output_text_chars
            .checked_add(output_text_chars)
            .ok_or(ParseError::OutputTextTooLarge(offset))?;
        if self.output_text_chars > self.limits().max_output_text_chars {
            return Err(ParseError::OutputTextTooLarge(offset));
        }
        if let Some(marker) = pending_marker.as_ref() {
            for run in &marker.runs {
                self.capture_bookmark_text(&run.text, offset)?;
            }
        }
        if let Some((marker_text, _)) = styled_marker.as_ref() {
            self.capture_bookmark_text(marker_text, offset)?;
        }
        self.capture_bookmark_text(text, offset)?;

        let paragraph = if self.state.destination == Destination::Header {
            &mut self.current_header_paragraph
        } else if self.state.destination == Destination::FirstPageHeader {
            &mut self.current_first_page_header_paragraph
        } else if self.state.destination == Destination::EvenPageHeader {
            &mut self.current_even_page_header_paragraph
        } else if self.state.destination == Destination::Footer {
            &mut self.current_footer_paragraph
        } else if self.state.destination == Destination::FirstPageFooter {
            &mut self.current_first_page_footer_paragraph
        } else if self.state.destination == Destination::EvenPageFooter {
            &mut self.current_even_page_footer_paragraph
        } else if self.state.destination == Destination::Footnote {
            &mut self.current_footnote_paragraph
        } else if self.state.destination == Destination::Endnote {
            &mut self.current_endnote_paragraph
        } else if let Some(row) = self.current_table_row.as_mut() {
            row.cell_open = true;
            &mut row.current_cell_paragraph
        } else {
            &mut self.current_paragraph
        };

        if let Some(marker) = pending_marker.as_ref() {
            for run in &marker.runs {
                push_text_to_paragraph(paragraph, &run.text, &self.state.paragraph, &run.style);
            }
        }
        if let Some((marker_text, marker_style)) = styled_marker.as_ref() {
            push_text_to_paragraph(paragraph, marker_text, &self.state.paragraph, marker_style);
        }
        push_text_to_paragraph(
            paragraph,
            text,
            &self.state.paragraph,
            &self.state.character,
        );
        Ok(())
    }

    fn map_symbol_like_font_text(&self, text: &str) -> Option<String> {
        let font = self
            .document
            .fonts
            .iter()
            .find(|font| font.index == self.state.character.font_index)?;
        let font_name = font.name.to_ascii_lowercase();
        let mapped = if font.charset == Some(2) || is_legacy_symbol_font_name(&font_name) {
            text.chars().map(map_symbol_char).collect::<String>()
        } else if let Some(mapper) = dingbats_mapper_for_font_name(&font_name) {
            text.chars().map(mapper).collect::<String>()
        } else if is_webdings_font_name(&font_name) {
            text.chars().map(map_webdings_char).collect::<String>()
        } else {
            return None;
        };
        if mapped == text { None } else { Some(mapped) }
    }

    fn push_list_marker_text(&mut self, text: &str, offset: usize) -> Result<(), ParseError> {
        if self.state.character.hidden {
            self.count_skipped_destination_bytes(text.len(), offset)?;
            return Ok(());
        }
        let font_mapped_text;
        let text = if !contains_internal_marker(text)
            && let Some(mapped) = self.map_symbol_like_font_text(text)
        {
            font_mapped_text = mapped;
            font_mapped_text.as_str()
        } else {
            text
        };
        let new_len = self
            .pending_list_marker
            .chars()
            .count()
            .checked_add(text.chars().count())
            .ok_or(ParseError::OutputTextTooLarge(offset))?;
        if new_len > self.limits().max_text_run_len {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "list marker text".to_string(),
                offset,
            });
        }
        self.pending_list_marker.push_str(text);
        push_text_to_runs(
            &mut self.pending_list_marker_runs,
            text,
            &self.state.character,
        );
        Ok(())
    }

    fn push_field_instruction_text(&mut self, text: &str, offset: usize) -> Result<(), ParseError> {
        self.count_skipped_destination_bytes(text.len(), offset)?;
        let limit = self.limits().max_text_run_len;
        append_field_instruction(&mut self.state.field_instruction, text, limit, offset)
    }

    fn push_bookmark_name_text(&mut self, text: &str, offset: usize) -> Result<(), ParseError> {
        self.count_skipped_destination_bytes(text.len(), offset)?;
        let new_len = self
            .state
            .bookmark_name_text
            .chars()
            .count()
            .checked_add(text.chars().count())
            .ok_or(ParseError::OutputTextTooLarge(offset))?;
        if new_len > self.limits().max_text_run_len {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "bookmark name".to_string(),
                offset,
            });
        }
        self.state.bookmark_name_text.push_str(text);
        Ok(())
    }

    fn start_bookmark_capture(
        &mut self,
        name: String,
        offset: usize,
    ) -> Result<Option<usize>, ParseError> {
        let Some(name) = clean_bookmark_name(name) else {
            return Ok(None);
        };
        if let Some(bookmark) = self
            .bookmark_captures
            .iter_mut()
            .find(|bookmark| bookmark.name == name)
        {
            bookmark.text.clear();
            bookmark.active = true;
            return Ok(Some(bookmark.id));
        }

        if self.bookmark_captures.len() >= self.limits().max_styles {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "bookmarks".to_string(),
                offset,
            });
        }
        let id = self.next_bookmark_marker_id;
        self.next_bookmark_marker_id = self.next_bookmark_marker_id.checked_add(1).ok_or(
            ParseError::ResourceLimitExceeded {
                resource: "bookmarks".to_string(),
                offset,
            },
        )?;
        self.bookmark_captures.push(BookmarkCapture {
            id,
            name,
            text: String::new(),
            active: true,
        });
        Ok(Some(id))
    }

    fn end_bookmark_capture(&mut self, name: String) {
        let Some(name) = clean_bookmark_name(name) else {
            return;
        };
        if let Some(bookmark) = self
            .bookmark_captures
            .iter_mut()
            .find(|bookmark| bookmark.name == name && bookmark.active)
        {
            bookmark.active = false;
        }
    }

    fn capture_bookmark_text(&mut self, text: &str, offset: usize) -> Result<(), ParseError> {
        if is_bookmark_page_marker(text) {
            return Ok(());
        }
        let max_text_run_len = self.limits().max_text_run_len;
        for bookmark in self
            .bookmark_captures
            .iter_mut()
            .filter(|bookmark| bookmark.active)
        {
            let new_len = bookmark
                .text
                .chars()
                .count()
                .checked_add(text.chars().count())
                .ok_or(ParseError::OutputTextTooLarge(offset))?;
            if new_len > max_text_run_len {
                return Err(ParseError::ResourceLimitExceeded {
                    resource: "bookmark text".to_string(),
                    offset,
                });
            }
            bookmark.text.push_str(text);
        }
        Ok(())
    }

    fn bookmark_text(&self, name: &str) -> Option<String> {
        let name = clean_bookmark_name(name.to_string())?;
        self.bookmark_captures
            .iter()
            .find(|bookmark| bookmark.name == name && !bookmark.active && !bookmark.text.is_empty())
            .map(|bookmark| bookmark.text.clone())
    }

    fn ensure_bookmark_marker_id(
        &mut self,
        name: String,
        offset: usize,
    ) -> Result<Option<usize>, ParseError> {
        let Some(name) = clean_bookmark_name(name) else {
            return Ok(None);
        };
        if let Some(bookmark) = self
            .bookmark_captures
            .iter()
            .find(|bookmark| bookmark.name == name)
        {
            return Ok(Some(bookmark.id));
        }
        if self.bookmark_captures.len() >= self.limits().max_styles {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "bookmarks".to_string(),
                offset,
            });
        }
        let id = self.next_bookmark_marker_id;
        self.next_bookmark_marker_id = self.next_bookmark_marker_id.checked_add(1).ok_or(
            ParseError::ResourceLimitExceeded {
                resource: "bookmarks".to_string(),
                offset,
            },
        )?;
        self.bookmark_captures.push(BookmarkCapture {
            id,
            name,
            text: String::new(),
            active: false,
        });
        Ok(Some(id))
    }

    fn push_form_default_text(&mut self, text: &str, offset: usize) -> Result<(), ParseError> {
        self.count_skipped_destination_bytes(text.len(), offset)?;
        let new_len = self
            .state
            .field_form_default_text
            .chars()
            .count()
            .checked_add(text.chars().count())
            .ok_or(ParseError::OutputTextTooLarge(offset))?;
        if new_len > self.limits().max_text_run_len {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "form default text".to_string(),
                offset,
            });
        }
        self.state.field_form_default_text.push_str(text);
        Ok(())
    }

    fn push_form_default_unicode(&mut self, value: i32, offset: usize) -> Result<(), ParseError> {
        if let Some(ch) =
            take_rtf_unicode_char(&mut self.state.pending_unicode_high_surrogate, value)
        {
            self.push_form_default_text(&ch.to_string(), offset)?;
        } else {
            self.count_skipped_destination_bytes(1, offset)?;
        }
        self.state.skip_bytes = self.state.unicode_skip;
        Ok(())
    }

    fn push_form_dropdown_text(&mut self, text: &str, offset: usize) -> Result<(), ParseError> {
        self.count_skipped_destination_bytes(text.len(), offset)?;
        let new_len = self
            .state
            .field_form_dropdown_entry_text
            .chars()
            .count()
            .checked_add(text.chars().count())
            .ok_or(ParseError::OutputTextTooLarge(offset))?;
        if new_len > self.limits().max_text_run_len {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "form dropdown entry text".to_string(),
                offset,
            });
        }
        self.state.field_form_dropdown_entry_text.push_str(text);
        Ok(())
    }

    fn push_form_dropdown_unicode(&mut self, value: i32, offset: usize) -> Result<(), ParseError> {
        if let Some(ch) =
            take_rtf_unicode_char(&mut self.state.pending_unicode_high_surrogate, value)
        {
            self.push_form_dropdown_text(&ch.to_string(), offset)?;
        } else {
            self.count_skipped_destination_bytes(1, offset)?;
        }
        self.state.skip_bytes = self.state.unicode_skip;
        Ok(())
    }

    fn push_document_property_text(&mut self, text: &str, offset: usize) -> Result<(), ParseError> {
        self.count_skipped_destination_bytes(text.len(), offset)?;
        if contains_internal_marker(text) {
            return Ok(());
        }

        let safe_text = text
            .chars()
            .filter(|ch| !ch.is_control())
            .collect::<String>();
        let new_len = self
            .state
            .metadata_property_text
            .chars()
            .count()
            .checked_add(safe_text.chars().count())
            .ok_or(ParseError::OutputTextTooLarge(offset))?;
        if new_len > self.limits().max_text_run_len {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "document property text".to_string(),
                offset,
            });
        }
        self.state.metadata_property_text.push_str(&safe_text);
        Ok(())
    }

    fn push_document_property_unicode(
        &mut self,
        value: i32,
        offset: usize,
    ) -> Result<(), ParseError> {
        if let Some(ch) =
            take_rtf_unicode_char(&mut self.state.pending_unicode_high_surrogate, value)
        {
            self.push_document_property_text(&ch.to_string(), offset)?;
        } else {
            self.count_skipped_destination_bytes(1, offset)?;
        }
        self.state.skip_bytes = self.state.unicode_skip;
        Ok(())
    }

    fn store_document_property(
        &mut self,
        property: DocumentProperty,
        text: &str,
        offset: usize,
    ) -> Result<(), ParseError> {
        let text = text.trim().to_string();
        if text.is_empty()
            || text.chars().any(|ch| ch.is_control())
            || contains_internal_marker(&text)
        {
            return Ok(());
        }

        if text.chars().count() > self.limits().max_text_run_len {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "document property text".to_string(),
                offset,
            });
        }

        if let Some((_, existing)) = self
            .document_properties
            .iter_mut()
            .find(|(stored_property, _)| *stored_property == property)
        {
            *existing = text;
        } else {
            self.document_properties.push((property, text));
        }
        Ok(())
    }

    fn set_document_timestamp_part(
        &mut self,
        name: &str,
        value: Option<i32>,
        offset: usize,
    ) -> Result<(), ParseError> {
        let Some(value) = value else {
            return Ok(());
        };
        match name {
            "yr" => self.state.metadata_timestamp_value.year = Some(value),
            "mo" => self.state.metadata_timestamp_value.month = Some(value),
            "dy" => self.state.metadata_timestamp_value.day = Some(value),
            "hr" => self.state.metadata_timestamp_value.hour = Some(value),
            "min" => self.state.metadata_timestamp_value.minute = Some(value),
            "sec" => self.state.metadata_timestamp_value.second = Some(value),
            _ => {}
        }
        self.count_skipped_destination_bytes(name.len(), offset)
    }

    fn store_document_timestamp(
        &mut self,
        kind: DocumentTimestampKind,
        timestamp: DocumentTimestamp,
        offset: usize,
    ) -> Result<(), ParseError> {
        let Some(timestamp) = normalize_document_timestamp(timestamp) else {
            return Ok(());
        };
        if let Some((_, existing)) = self
            .document_timestamps
            .iter_mut()
            .find(|(stored_kind, _)| *stored_kind == kind)
        {
            *existing = timestamp;
        } else {
            if self.document_timestamps.len() >= 3 {
                return Err(ParseError::ResourceLimitExceeded {
                    resource: "document timestamps".to_string(),
                    offset,
                });
            }
            self.document_timestamps.push((kind, timestamp));
        }
        Ok(())
    }

    fn set_document_edit_minutes(
        &mut self,
        value: Option<i32>,
        offset: usize,
    ) -> Result<(), ParseError> {
        self.count_skipped_destination_bytes("edmins".len(), offset)?;
        let Some(value) = value else {
            return Ok(());
        };
        if value < 0 {
            return Ok(());
        }
        let max = self.limits().max_output_text_chars.min(i32::MAX as usize) as i32;
        if value > max {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "document edit minutes".to_string(),
                offset,
            });
        }
        self.document_edit_minutes = Some(value);
        Ok(())
    }

    fn push_user_property_text(&mut self, text: &str, offset: usize) -> Result<(), ParseError> {
        self.count_skipped_destination_bytes(text.len(), offset)?;
        if contains_internal_marker(text) {
            return Ok(());
        }

        let safe_text = text
            .chars()
            .filter(|ch| !ch.is_control())
            .collect::<String>();
        let new_len = self
            .state
            .user_property_capture_text
            .chars()
            .count()
            .checked_add(safe_text.chars().count())
            .ok_or(ParseError::OutputTextTooLarge(offset))?;
        if new_len > self.limits().max_text_run_len {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "custom document property text".to_string(),
                offset,
            });
        }
        self.state.user_property_capture_text.push_str(&safe_text);
        Ok(())
    }

    fn push_user_property_unicode(&mut self, value: i32, offset: usize) -> Result<(), ParseError> {
        if let Some(ch) =
            take_rtf_unicode_char(&mut self.state.pending_unicode_high_surrogate, value)
        {
            self.push_user_property_text(&ch.to_string(), offset)?;
        } else {
            self.count_skipped_destination_bytes(1, offset)?;
        }
        self.state.skip_bytes = self.state.unicode_skip;
        Ok(())
    }

    fn store_custom_document_property(
        &mut self,
        name: &str,
        text: &str,
        offset: usize,
    ) -> Result<(), ParseError> {
        let Some(text) = clean_document_property_text(text, offset, self.limits())? else {
            return Ok(());
        };

        if let Some((_, existing)) = self
            .custom_document_properties
            .iter_mut()
            .find(|(stored_name, _)| stored_name.eq_ignore_ascii_case(name))
        {
            *existing = text;
        } else {
            if self.custom_document_properties.len() >= self.limits().max_styles {
                return Err(ParseError::ResourceLimitExceeded {
                    resource: "custom document properties".to_string(),
                    offset,
                });
            }
            self.custom_document_properties
                .push((name.to_string(), text));
        }
        Ok(())
    }

    fn push_passive_field_result(
        &mut self,
        result: PassiveFieldResult,
        offset: usize,
    ) -> Result<(), ParseError> {
        let previous_shading = self.state.character.form_field_shading;
        self.state.character.form_field_shading = self.form_field_shading && result.form_field;
        let previous_font = self.state.character.font_index;

        if let Some(font_name) = result.font_name.as_deref()
            && let Some(font_index) = self.ensure_passive_result_font(font_name, offset)?
        {
            self.state.character.font_index = font_index;
        }

        let result = self.push_text(&result.text, offset);
        self.state.character.font_index = previous_font;
        self.state.character.form_field_shading = previous_shading;
        result
    }

    fn passive_field_result_for_instruction(
        &mut self,
        instruction: &str,
        form_checkbox_checked: Option<bool>,
        form_default_text: &str,
        form_dropdown_entries: &[String],
        form_dropdown_selected_index: Option<i32>,
        offset: usize,
    ) -> Result<Option<PassiveFieldResult>, ParseError> {
        let result = if field_instruction_name(instruction) == Some("SEQ") {
            self.passive_sequence_field_result(instruction, offset)?
        } else if field_instruction_name(instruction).is_some_and(is_auto_number_field) {
            self.passive_auto_number_field_result(offset)?
        } else if field_instruction_name(instruction) == Some("LISTNUM") {
            self.passive_list_number_field_result(instruction, offset)?
        } else if field_instruction_name(instruction) == Some("REF") {
            self.passive_ref_field_result(instruction)
        } else if field_instruction_name(instruction) == Some("PAGEREF") {
            self.passive_page_ref_field_result(instruction, offset)?
        } else if field_instruction_name(instruction) == Some("DOCPROPERTY") {
            self.passive_doc_property_field_result(instruction)
        } else if field_instruction_name(instruction) == Some("INFO") {
            self.passive_info_field_result(instruction)
        } else if let Some(property) =
            field_instruction_name(instruction).and_then(document_shortcut_property_field_name)
        {
            self.passive_builtin_document_property_field_result(property)
        } else if let Some(kind) =
            field_instruction_name(instruction).and_then(document_timestamp_field_name)
        {
            self.passive_document_timestamp_field_result(kind, instruction)
        } else if field_instruction_name(instruction) == Some("EDITTIME") {
            self.passive_edit_time_field_result()
        } else {
            passive_field_result(
                instruction,
                form_checkbox_checked,
                form_default_text,
                form_dropdown_entries,
                form_dropdown_selected_index,
            )
        };

        Ok(result.and_then(|result| apply_field_format_switches(instruction, result)))
    }

    fn passive_doc_property_field_result(&self, instruction: &str) -> Option<PassiveFieldResult> {
        let name = field_first_argument(instruction)?;
        let text = if let Some(property) = document_property_field_name(&name) {
            self.document_property_text(property)?
        } else {
            self.custom_document_properties
                .iter()
                .find_map(|(stored_name, text)| {
                    stored_name.eq_ignore_ascii_case(&name).then(|| text)
                })?
        };
        Some(PassiveFieldResult {
            text: text.clone(),
            font_name: None,
            form_field: false,
        })
    }

    fn passive_info_field_result(&self, instruction: &str) -> Option<PassiveFieldResult> {
        let name = field_first_argument(instruction)?;
        let property = document_property_field_name(&name)?;
        Some(PassiveFieldResult {
            text: self.document_property_text(property)?.clone(),
            font_name: None,
            form_field: false,
        })
    }

    fn passive_builtin_document_property_field_result(
        &self,
        property: DocumentProperty,
    ) -> Option<PassiveFieldResult> {
        Some(PassiveFieldResult {
            text: self.document_property_text(property)?.clone(),
            font_name: None,
            form_field: false,
        })
    }

    fn document_property_text(&self, property: DocumentProperty) -> Option<&String> {
        self.document_properties
            .iter()
            .find_map(|(stored_property, text)| (*stored_property == property).then(|| text))
    }

    fn passive_document_timestamp_field_result(
        &self,
        kind: DocumentTimestampKind,
        instruction: &str,
    ) -> Option<PassiveFieldResult> {
        let timestamp = self
            .document_timestamps
            .iter()
            .find_map(|(stored_kind, timestamp)| (*stored_kind == kind).then_some(timestamp))?;
        let text = if let Some(picture) = field_date_picture_switch(instruction)? {
            apply_field_date_picture(timestamp, &picture)?
        } else {
            format_default_document_timestamp(timestamp)
        };
        Some(PassiveFieldResult {
            text,
            font_name: None,
            form_field: false,
        })
    }

    fn passive_edit_time_field_result(&self) -> Option<PassiveFieldResult> {
        Some(PassiveFieldResult {
            text: self.document_edit_minutes?.to_string(),
            font_name: None,
            form_field: false,
        })
    }

    fn passive_sequence_field_result(
        &mut self,
        instruction: &str,
        offset: usize,
    ) -> Result<Option<PassiveFieldResult>, ParseError> {
        let Some(sequence) = field_sequence_instruction(instruction) else {
            return Ok(None);
        };

        let max_value = self.limits().max_page_number_start.max(1);
        let value = if let Some(reset_value) = sequence.reset_value {
            if reset_value < 0 || reset_value > max_value {
                return Err(ParseError::ResourceLimitExceeded {
                    resource: "field sequence value".to_string(),
                    offset,
                });
            }
            self.set_sequence_counter(sequence.name, reset_value, offset)?
        } else if sequence.repeat_current {
            let Some(counter) = self
                .field_sequence_counters
                .iter()
                .find(|counter| counter.name == sequence.name)
            else {
                return Ok(None);
            };
            counter.value
        } else {
            self.increment_sequence_counter(sequence.name, max_value, offset)?
        };

        let text = if sequence.hidden {
            String::new()
        } else {
            value.to_string()
        };
        Ok(Some(PassiveFieldResult {
            text,
            font_name: None,
            form_field: false,
        }))
    }

    fn set_sequence_counter(
        &mut self,
        name: String,
        value: i32,
        offset: usize,
    ) -> Result<i32, ParseError> {
        if let Some(counter) = self
            .field_sequence_counters
            .iter_mut()
            .find(|counter| counter.name == name)
        {
            counter.value = value;
            return Ok(value);
        }

        if self.field_sequence_counters.len() >= self.limits().max_styles {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "field sequences".to_string(),
                offset,
            });
        }
        self.field_sequence_counters
            .push(FieldSequenceCounter { name, value });
        Ok(value)
    }

    fn increment_sequence_counter(
        &mut self,
        name: String,
        max_value: i32,
        offset: usize,
    ) -> Result<i32, ParseError> {
        if let Some(counter) = self
            .field_sequence_counters
            .iter_mut()
            .find(|counter| counter.name == name)
        {
            if counter.value >= max_value {
                return Err(ParseError::ResourceLimitExceeded {
                    resource: "field sequence value".to_string(),
                    offset,
                });
            }
            counter.value += 1;
            return Ok(counter.value);
        }

        self.set_sequence_counter(name, 1, offset)
    }

    fn passive_auto_number_field_result(
        &mut self,
        offset: usize,
    ) -> Result<Option<PassiveFieldResult>, ParseError> {
        let max_value = self.limits().max_page_number_start.max(1);
        if self.field_auto_number_counter >= max_value {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "field auto number value".to_string(),
                offset,
            });
        }
        self.field_auto_number_counter += 1;
        Ok(Some(PassiveFieldResult {
            text: self.field_auto_number_counter.to_string(),
            font_name: None,
            form_field: false,
        }))
    }

    fn passive_list_number_field_result(
        &mut self,
        instruction: &str,
        offset: usize,
    ) -> Result<Option<PassiveFieldResult>, ParseError> {
        let Some(list_number) = field_list_number_instruction(instruction) else {
            return Ok(None);
        };

        let max_value = self.limits().max_page_number_start.max(1);
        let key = format!("{}\u{1f}{}", list_number.name, list_number.level);
        let value = if let Some(reset_value) = list_number.reset_value {
            if reset_value < 0 || reset_value > max_value {
                return Err(ParseError::ResourceLimitExceeded {
                    resource: "field list number value".to_string(),
                    offset,
                });
            }
            self.set_list_number_counter(key, reset_value, offset)?
        } else {
            self.increment_list_number_counter(key, max_value, offset)?
        };

        Ok(Some(PassiveFieldResult {
            text: value.to_string(),
            font_name: None,
            form_field: false,
        }))
    }

    fn set_list_number_counter(
        &mut self,
        name: String,
        value: i32,
        offset: usize,
    ) -> Result<i32, ParseError> {
        if let Some(counter) = self
            .field_list_number_counters
            .iter_mut()
            .find(|counter| counter.name == name)
        {
            counter.value = value;
            return Ok(value);
        }

        if self.field_list_number_counters.len() >= self.limits().max_styles {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "field list numbers".to_string(),
                offset,
            });
        }
        self.field_list_number_counters
            .push(FieldSequenceCounter { name, value });
        Ok(value)
    }

    fn increment_list_number_counter(
        &mut self,
        name: String,
        max_value: i32,
        offset: usize,
    ) -> Result<i32, ParseError> {
        if let Some(counter) = self
            .field_list_number_counters
            .iter_mut()
            .find(|counter| counter.name == name)
        {
            if counter.value >= max_value {
                return Err(ParseError::ResourceLimitExceeded {
                    resource: "field list number value".to_string(),
                    offset,
                });
            }
            counter.value += 1;
            return Ok(counter.value);
        }

        self.set_list_number_counter(name, 1, offset)
    }

    fn passive_ref_field_result(&self, instruction: &str) -> Option<PassiveFieldResult> {
        let name = field_first_argument(instruction)?;
        let text = self.bookmark_text(&name)?;
        Some(PassiveFieldResult {
            text,
            font_name: None,
            form_field: false,
        })
    }

    fn passive_page_ref_field_result(
        &mut self,
        instruction: &str,
        offset: usize,
    ) -> Result<Option<PassiveFieldResult>, ParseError> {
        let Some(name) = field_first_argument(instruction) else {
            return Ok(None);
        };
        let Some(id) = self.ensure_bookmark_marker_id(name, offset)? else {
            return Ok(None);
        };
        Ok(Some(PassiveFieldResult {
            text: bookmark_page_ref_marker(id),
            font_name: None,
            form_field: false,
        }))
    }

    fn ensure_passive_result_font(
        &mut self,
        font_name: &str,
        offset: usize,
    ) -> Result<Option<i32>, ParseError> {
        if let Some(font_index) = self
            .document
            .fonts
            .iter()
            .find(|font| font.name.eq_ignore_ascii_case(font_name))
            .map(|font| font.index)
        {
            return Ok(Some(font_index));
        }

        if !is_builtin_passive_result_font(font_name) {
            return Ok(None);
        }

        if self.document.fonts.len() >= self.limits().max_fonts {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "fonts".to_string(),
                offset,
            });
        }

        let next_index = self
            .document
            .fonts
            .iter()
            .map(|font| font.index)
            .max()
            .unwrap_or(-1)
            .checked_add(1)
            .ok_or(ParseError::ResourceLimitExceeded {
                resource: "fonts".to_string(),
                offset,
            })?;
        self.document.fonts.push(FontDef {
            index: next_index,
            name: "ZapfDingbats".to_string(),
            alternate_name: None,
            charset: None,
            code_page: None,
            family: FontFamilyHint::Tech,
            pitch: FontPitch::Default,
        });
        Ok(Some(next_index))
    }

    fn push_list_marker_unicode(&mut self, value: i32, offset: usize) -> Result<(), ParseError> {
        if let Some(ch) =
            take_rtf_unicode_char(&mut self.state.pending_unicode_high_surrogate, value)
        {
            self.push_list_marker_text(&ch.to_string(), offset)?;
        }
        self.state.skip_bytes = self.state.unicode_skip;
        Ok(())
    }

    fn push_unicode(&mut self, value: i32, offset: usize) -> Result<(), ParseError> {
        if let Some(ch) =
            take_rtf_unicode_char(&mut self.state.pending_unicode_high_surrogate, value)
        {
            self.push_text(&ch.to_string(), offset)?;
        }
        self.state.skip_bytes = self.state.unicode_skip;
        Ok(())
    }

    fn finish_paragraph(&mut self) {
        if let Some(row) = self.current_table_row.as_mut() {
            if !row.current_cell_paragraph.runs.is_empty() {
                let paragraph = std::mem::replace(
                    &mut row.current_cell_paragraph,
                    Paragraph {
                        style: self.state.paragraph.clone(),
                        runs: Vec::new(),
                    },
                );
                row.current_cell_paragraphs.push(paragraph);
                row.cell_open = true;
            }
            return;
        }

        if !self.current_paragraph.runs.is_empty() {
            let paragraph = std::mem::replace(
                &mut self.current_paragraph,
                Paragraph {
                    style: self.state.paragraph.clone(),
                    runs: Vec::new(),
                },
            );
            self.document.blocks.push(Block::Paragraph(paragraph));
        }
    }

    fn finish_current_paragraph_for_destination(&mut self, offset: usize) {
        let advance_next_style = match self.state.destination {
            Destination::Body => {
                self.finish_paragraph();
                true
            }
            destination if is_header_destination(destination) => {
                self.finish_header_paragraph();
                true
            }
            destination if is_footer_destination(destination) => {
                self.finish_footer_paragraph();
                true
            }
            Destination::Footnote => {
                self.finish_footnote_paragraph();
                true
            }
            Destination::Endnote => {
                self.finish_endnote_paragraph();
                true
            }
            Destination::ListText => false,
            _ => {
                self.finish_paragraph();
                false
            }
        };
        if advance_next_style {
            self.apply_next_style_after_paragraph(offset);
        }
    }

    fn finish_header_paragraph(&mut self) {
        let destination = self.state.destination;
        let current = match destination {
            Destination::FirstPageHeader => &mut self.current_first_page_header_paragraph,
            Destination::EvenPageHeader => &mut self.current_even_page_header_paragraph,
            _ => &mut self.current_header_paragraph,
        };
        if !current.runs.is_empty() {
            let paragraph = std::mem::replace(
                current,
                Paragraph {
                    style: self.state.paragraph.clone(),
                    runs: Vec::new(),
                },
            );
            if self.has_started_visible_body() {
                match destination {
                    Destination::FirstPageHeader => {
                        self.current_section_page.first_page_header.push(paragraph)
                    }
                    Destination::EvenPageHeader => {
                        self.current_section_page.even_page_header.push(paragraph)
                    }
                    _ => self.current_section_page.header.push(paragraph),
                }
                self.upsert_current_section_settings();
            } else {
                match destination {
                    Destination::FirstPageHeader => self.document.first_page_header.push(paragraph),
                    Destination::EvenPageHeader => self.document.even_page_header.push(paragraph),
                    _ => self.document.header.push(paragraph),
                }
            }
        }
    }

    fn finish_footer_paragraph(&mut self) {
        let destination = self.state.destination;
        let current = match destination {
            Destination::FirstPageFooter => &mut self.current_first_page_footer_paragraph,
            Destination::EvenPageFooter => &mut self.current_even_page_footer_paragraph,
            _ => &mut self.current_footer_paragraph,
        };
        if !current.runs.is_empty() {
            let paragraph = std::mem::replace(
                current,
                Paragraph {
                    style: self.state.paragraph.clone(),
                    runs: Vec::new(),
                },
            );
            if self.has_started_visible_body() {
                match destination {
                    Destination::FirstPageFooter => {
                        self.current_section_page.first_page_footer.push(paragraph)
                    }
                    Destination::EvenPageFooter => {
                        self.current_section_page.even_page_footer.push(paragraph)
                    }
                    _ => self.current_section_page.footer.push(paragraph),
                }
                self.upsert_current_section_settings();
            } else {
                match destination {
                    Destination::FirstPageFooter => self.document.first_page_footer.push(paragraph),
                    Destination::EvenPageFooter => self.document.even_page_footer.push(paragraph),
                    _ => self.document.footer.push(paragraph),
                }
            }
        }
    }

    fn finish_footnote_paragraph(&mut self) {
        if !self.current_footnote_paragraph.runs.is_empty() {
            let paragraph = std::mem::replace(
                &mut self.current_footnote_paragraph,
                Paragraph {
                    style: self.state.paragraph.clone(),
                    runs: Vec::new(),
                },
            );
            self.document.footnotes.push(paragraph);
        }
    }

    fn finish_endnote_paragraph(&mut self) {
        if !self.current_endnote_paragraph.runs.is_empty() {
            let paragraph = std::mem::replace(
                &mut self.current_endnote_paragraph,
                Paragraph {
                    style: self.state.paragraph.clone(),
                    runs: Vec::new(),
                },
            );
            self.document.endnotes.push(paragraph);
        }
    }

    fn push_placeholder(&mut self, text: String) {
        self.finish_paragraph();
        self.document.blocks.push(Block::Placeholder(text));
    }

    fn push_placeholder_for_destination(
        &mut self,
        destination: Destination,
        text: String,
        offset: usize,
    ) -> Result<(), ParseError> {
        match destination {
            destination
                if is_header_destination(destination) || is_footer_destination(destination) =>
            {
                let previous_destination = self.state.destination;
                self.state.destination = destination;
                self.push_text(&text, offset)?;
                self.finish_current_paragraph_for_destination(offset);
                self.state.destination = previous_destination;
                Ok(())
            }
            Destination::Footnote | Destination::Endnote => {
                let previous_destination = self.state.destination;
                self.state.destination = destination;
                self.push_text(&text, offset)?;
                self.finish_current_paragraph_for_destination(offset);
                self.state.destination = previous_destination;
                Ok(())
            }
            _ => {
                self.push_placeholder(text);
                Ok(())
            }
        }
    }

    fn start_table_row(&mut self) {
        self.finish_paragraph();
        if self.current_table.is_none() {
            self.current_table = Some(TableBuilder::default());
        }
        self.current_table_row = Some(TableRowBuilder {
            cells: Vec::new(),
            cell_right_edges_twips: Vec::new(),
            cell_shading_color_indices: Vec::new(),
            cell_shading_basis_points: Vec::new(),
            cell_shading_patterns: Vec::new(),
            cell_paddings: Vec::new(),
            cell_borders: Vec::new(),
            cell_border_flags: Vec::new(),
            cell_preferred_widths_twips: Vec::new(),
            cell_no_wraps: Vec::new(),
            cell_text_directions: Vec::new(),
            cell_vertical_alignments: Vec::new(),
            cell_horizontal_merges: Vec::new(),
            cell_vertical_merges: Vec::new(),
            default_cell_shading_color_index: None,
            current_cell_shading_color_index: None,
            default_cell_shading_basis_points: 10_000,
            current_cell_shading_basis_points: 10_000,
            default_cell_shading_pattern: ShadingPattern::None,
            current_cell_shading_pattern: ShadingPattern::None,
            default_cell_padding: TableCellPadding::default(),
            current_cell_padding: TableCellPadding::default(),
            current_cell_borders: TableCellBorders::default(),
            current_cell_border_flags: TableCellBorderFlags::default(),
            current_cell_border_side: None,
            current_cell_preferred_width: PreferredTableWidth::default(),
            current_cell_no_wrap: false,
            current_cell_text_direction: TableCellTextDirection::LeftToRightTopToBottom,
            row_borders: TableCellBorders::default(),
            row_border_flags: TableCellBorderFlags::default(),
            current_row_border_side: None,
            current_cell_vertical_align: TableCellVerticalAlign::Top,
            current_cell_horizontal_merge: TableCellHorizontalMerge::None,
            current_cell_vertical_merge: TableCellVerticalMerge::None,
            height_twips: None,
            left_offset_twips: 0,
            cell_gap_twips: DEFAULT_TABLE_CELL_GAP_TWIPS,
            alignment: TableRowAlignment::Left,
            repeat_header: false,
            keep_together: false,
            right_to_left: false,
            current_cell_paragraphs: Vec::new(),
            current_cell_paragraph: Paragraph {
                style: self.state.paragraph.clone(),
                runs: Vec::new(),
            },
            cell_open: true,
        });
    }

    fn handle_nested_table_control(
        &mut self,
        control: &Control,
        offset: usize,
    ) -> Result<bool, ParseError> {
        if self.current_table_row.is_none() {
            return Ok(false);
        }

        match control.name.as_str() {
            "itap" if control.parameter.unwrap_or(1) > 1 => {
                self.state.table_nesting_level = control.parameter.unwrap_or(2).max(2);
                return Ok(true);
            }
            "itap" if self.state.table_nesting_level > 1 => {
                self.state.table_nesting_level = control.parameter.unwrap_or(1).max(1);
                return Ok(true);
            }
            "trowd" => {
                self.state.table_nesting_level = self.state.table_nesting_level.max(2);
                return Ok(true);
            }
            _ => {}
        }

        if self.state.table_nesting_level <= 1 {
            return Ok(false);
        }

        match control.name.as_str() {
            "nestcell" | "cell" => {
                self.push_text("\t", offset)?;
                Ok(true)
            }
            "nestrow" | "row" => {
                self.push_text("\n", offset)?;
                self.state.table_nesting_level = 1;
                Ok(true)
            }
            name if is_nested_table_structural_control(name) => Ok(true),
            _ => Ok(false),
        }
    }

    fn push_table_cell_boundary(&mut self, right_edge_twips: i32) {
        let page_content_width_twips = self.current_page_content_width_twips();
        let max_width_twips = self.limits().max_page_dimension_twips;
        if let Some(row) = self.current_table_row.as_mut()
            && right_edge_twips > 0
        {
            let preferred_width_twips = normalized_preferred_table_width_twips(
                row.current_cell_preferred_width,
                page_content_width_twips,
                max_width_twips,
            );
            row.cell_right_edges_twips.push(right_edge_twips);
            row.cell_shading_color_indices
                .push(row.current_cell_shading_color_index);
            row.cell_shading_basis_points
                .push(row.current_cell_shading_basis_points);
            row.cell_shading_patterns
                .push(row.current_cell_shading_pattern);
            row.cell_paddings.push(row.current_cell_padding);
            row.cell_borders.push(row.current_cell_borders);
            row.cell_border_flags.push(row.current_cell_border_flags);
            row.cell_preferred_widths_twips.push(preferred_width_twips);
            row.cell_no_wraps.push(row.current_cell_no_wrap);
            row.cell_text_directions
                .push(row.current_cell_text_direction);
            row.cell_vertical_alignments
                .push(row.current_cell_vertical_align);
            row.cell_horizontal_merges
                .push(row.current_cell_horizontal_merge);
            row.cell_vertical_merges
                .push(row.current_cell_vertical_merge);
            row.current_cell_shading_color_index = row.default_cell_shading_color_index;
            row.current_cell_shading_basis_points = row.default_cell_shading_basis_points;
            row.current_cell_shading_pattern = row.default_cell_shading_pattern;
            row.current_cell_padding = row.default_cell_padding;
            row.current_cell_borders = TableCellBorders::default();
            row.current_cell_border_flags = TableCellBorderFlags::default();
            row.current_cell_border_side = None;
            row.current_cell_preferred_width = PreferredTableWidth::default();
            row.current_cell_no_wrap = false;
            row.current_cell_text_direction = TableCellTextDirection::LeftToRightTopToBottom;
            row.current_cell_vertical_align = TableCellVerticalAlign::Top;
            row.current_cell_horizontal_merge = TableCellHorizontalMerge::None;
            row.current_cell_vertical_merge = TableCellVerticalMerge::None;
        }
    }

    fn set_current_cell_shading(&mut self, color_index: usize) {
        if let Some(row) = self.current_table_row.as_mut() {
            row.current_cell_shading_color_index = if color_index == 0 {
                None
            } else {
                Some(color_index)
            };
        }
    }

    fn set_current_cell_shading_basis(&mut self, value: Option<i32>, offset: usize) {
        let basis = self.clamp_shading_basis(value.unwrap_or(10_000), "table cell", offset);
        if let Some(row) = self.current_table_row.as_mut() {
            row.current_cell_shading_basis_points = basis;
        }
    }

    fn set_current_cell_shading_pattern(&mut self, pattern: ShadingPattern) {
        if let Some(row) = self.current_table_row.as_mut() {
            row.current_cell_shading_pattern = pattern;
        }
    }

    fn set_current_table_row_shading(&mut self, color_index: usize) {
        if let Some(row) = self.current_table_row.as_mut() {
            let shading = if color_index == 0 {
                None
            } else {
                Some(color_index)
            };
            row.default_cell_shading_color_index = shading;
            row.current_cell_shading_color_index = shading;
        }
    }

    fn set_current_table_row_shading_basis(&mut self, value: Option<i32>, offset: usize) {
        let basis = self.clamp_shading_basis(value.unwrap_or(10_000), "table row", offset);
        if let Some(row) = self.current_table_row.as_mut() {
            row.default_cell_shading_basis_points = basis;
            row.current_cell_shading_basis_points = basis;
        }
    }

    fn set_current_table_row_shading_pattern(&mut self, pattern: ShadingPattern) {
        if let Some(row) = self.current_table_row.as_mut() {
            row.default_cell_shading_pattern = pattern;
            row.current_cell_shading_pattern = pattern;
        }
    }

    fn set_current_cell_padding_left(&mut self, value: Option<i32>, offset: usize) {
        let padding = self.normalized_cell_padding(value, "left", offset);
        if let Some(row) = self.current_table_row.as_mut() {
            row.current_cell_padding.left_twips = Some(padding);
        }
    }

    fn set_current_cell_padding_right(&mut self, value: Option<i32>, offset: usize) {
        let padding = self.normalized_cell_padding(value, "right", offset);
        if let Some(row) = self.current_table_row.as_mut() {
            row.current_cell_padding.right_twips = Some(padding);
        }
    }

    fn set_current_cell_padding_top(&mut self, value: Option<i32>, offset: usize) {
        let padding = self.normalized_cell_padding(value, "top", offset);
        if let Some(row) = self.current_table_row.as_mut() {
            row.current_cell_padding.top_twips = Some(padding);
        }
    }

    fn set_current_cell_padding_bottom(&mut self, value: Option<i32>, offset: usize) {
        let padding = self.normalized_cell_padding(value, "bottom", offset);
        if let Some(row) = self.current_table_row.as_mut() {
            row.current_cell_padding.bottom_twips = Some(padding);
        }
    }

    fn set_current_cell_preferred_width_unit(&mut self, value: Option<i32>) {
        if let Some(row) = self.current_table_row.as_mut() {
            row.current_cell_preferred_width.unit = match value.unwrap_or(0) {
                2 => PreferredTableWidthUnit::FiftiethsPercent,
                3 => PreferredTableWidthUnit::Twips,
                _ => PreferredTableWidthUnit::Auto,
            };
        }
    }

    fn set_current_cell_preferred_width(&mut self, value: Option<i32>) {
        if let Some(row) = self.current_table_row.as_mut() {
            row.current_cell_preferred_width.value = value;
        }
    }

    fn set_current_cell_no_wrap(&mut self, no_wrap: bool) {
        if let Some(row) = self.current_table_row.as_mut() {
            row.current_cell_no_wrap = no_wrap;
        }
    }

    fn set_current_cell_text_direction(&mut self, direction: TableCellTextDirection) {
        if let Some(row) = self.current_table_row.as_mut() {
            row.current_cell_text_direction = direction;
        }
    }

    fn set_current_table_row_padding_left(&mut self, value: Option<i32>, offset: usize) {
        let padding = self.normalized_cell_padding(value, "row left", offset);
        if let Some(row) = self.current_table_row.as_mut() {
            row.default_cell_padding.left_twips = Some(padding);
            row.current_cell_padding.left_twips = Some(padding);
        }
    }

    fn set_current_table_row_padding_right(&mut self, value: Option<i32>, offset: usize) {
        let padding = self.normalized_cell_padding(value, "row right", offset);
        if let Some(row) = self.current_table_row.as_mut() {
            row.default_cell_padding.right_twips = Some(padding);
            row.current_cell_padding.right_twips = Some(padding);
        }
    }

    fn set_current_table_row_padding_top(&mut self, value: Option<i32>, offset: usize) {
        let padding = self.normalized_cell_padding(value, "row top", offset);
        if let Some(row) = self.current_table_row.as_mut() {
            row.default_cell_padding.top_twips = Some(padding);
            row.current_cell_padding.top_twips = Some(padding);
        }
    }

    fn set_current_table_row_padding_bottom(&mut self, value: Option<i32>, offset: usize) {
        let padding = self.normalized_cell_padding(value, "row bottom", offset);
        if let Some(row) = self.current_table_row.as_mut() {
            row.default_cell_padding.bottom_twips = Some(padding);
            row.current_cell_padding.bottom_twips = Some(padding);
        }
    }

    fn normalized_cell_padding(&mut self, value: Option<i32>, side: &str, offset: usize) -> i32 {
        let value = value.unwrap_or(0).max(0);
        let clamped = value.min(self.limits().max_table_cell_gap_twips);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("table cell {side} padding clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        clamped
    }

    fn set_current_cell_vertical_align(&mut self, align: TableCellVerticalAlign) {
        if let Some(row) = self.current_table_row.as_mut() {
            row.current_cell_vertical_align = align;
        }
    }

    fn set_current_cell_horizontal_merge(&mut self, merge: TableCellHorizontalMerge) {
        if let Some(row) = self.current_table_row.as_mut() {
            row.current_cell_horizontal_merge = merge;
        }
    }

    fn set_current_cell_vertical_merge(&mut self, merge: TableCellVerticalMerge) {
        if let Some(row) = self.current_table_row.as_mut() {
            row.current_cell_vertical_merge = merge;
        }
    }

    fn select_current_cell_border(&mut self, side: TableCellBorderSide) {
        if let Some(row) = self.current_table_row.as_mut() {
            row.current_cell_border_side = Some(side);
            row.current_row_border_side = None;
        }
    }

    fn select_current_table_row_border(&mut self, side: TableCellBorderSide) {
        if let Some(row) = self.current_table_row.as_mut() {
            row.current_row_border_side = Some(side);
            row.current_cell_border_side = None;
        }
    }

    fn select_current_paragraph_border(&mut self, side: TableCellBorderSide) {
        self.state.paragraph_border_selection = BorderSelection::Paragraph(side);
    }

    fn select_current_paragraph_between_border(&mut self) {
        self.state.paragraph_border_selection = BorderSelection::ParagraphBetween;
    }

    fn select_current_paragraph_box_border(&mut self) {
        self.state.paragraph_border_selection = BorderSelection::ParagraphBox;
    }

    fn select_current_character_border(&mut self) {
        self.state.paragraph_border_selection = BorderSelection::Character;
    }

    fn select_current_page_border(&mut self, side: TableCellBorderSide) {
        self.state.paragraph_border_selection = BorderSelection::Page(side);
    }

    fn set_current_cell_border_visible(&mut self, visible: bool) -> bool {
        self.update_current_cell_border(|border| {
            border.visible = visible;
        })
    }

    fn update_current_cell_border(&mut self, update: impl FnOnce(&mut TableCellBorder)) -> bool {
        let Some(row) = self.current_table_row.as_mut() else {
            return false;
        };
        let Some(side) = row.current_cell_border_side else {
            return false;
        };

        let border = match side {
            TableCellBorderSide::Left => &mut row.current_cell_borders.left,
            TableCellBorderSide::Right => &mut row.current_cell_borders.right,
            TableCellBorderSide::Top => &mut row.current_cell_borders.top,
            TableCellBorderSide::Bottom => &mut row.current_cell_borders.bottom,
            TableCellBorderSide::DiagonalDown => &mut row.current_cell_borders.diagonal_down,
            TableCellBorderSide::DiagonalUp => &mut row.current_cell_borders.diagonal_up,
        };
        row.current_cell_border_flags.set(side);
        update(border);
        true
    }

    fn update_current_table_row_border(
        &mut self,
        update: impl FnOnce(&mut TableCellBorder),
    ) -> bool {
        let Some(row) = self.current_table_row.as_mut() else {
            return false;
        };
        let Some(side) = row.current_row_border_side else {
            return false;
        };
        if matches!(
            side,
            TableCellBorderSide::DiagonalDown | TableCellBorderSide::DiagonalUp
        ) {
            return false;
        }

        let border = match side {
            TableCellBorderSide::Left => &mut row.row_borders.left,
            TableCellBorderSide::Right => &mut row.row_borders.right,
            TableCellBorderSide::Top => &mut row.row_borders.top,
            TableCellBorderSide::Bottom => &mut row.row_borders.bottom,
            TableCellBorderSide::DiagonalDown | TableCellBorderSide::DiagonalUp => unreachable!(),
        };
        row.row_border_flags.set(side);
        update(border);
        true
    }

    fn update_current_paragraph_border(
        &mut self,
        mut update: impl FnMut(&mut TableCellBorder),
    ) -> bool {
        match self.state.paragraph_border_selection {
            BorderSelection::None => false,
            BorderSelection::Character => {
                if self.current_list_level.is_some()
                    && self.state.list_context == ListContext::ListLevel
                {
                    self.update_current_list_level_character_style(|style| {
                        update(&mut style.border);
                    });
                    return true;
                }
                update(&mut self.state.character.border);
                true
            }
            BorderSelection::Paragraph(side) => {
                let border = match side {
                    TableCellBorderSide::Left => &mut self.state.paragraph.borders.left,
                    TableCellBorderSide::Right => &mut self.state.paragraph.borders.right,
                    TableCellBorderSide::Top => &mut self.state.paragraph.borders.top,
                    TableCellBorderSide::Bottom => &mut self.state.paragraph.borders.bottom,
                    TableCellBorderSide::DiagonalDown | TableCellBorderSide::DiagonalUp => {
                        return false;
                    }
                };
                update(border);
                true
            }
            BorderSelection::ParagraphBetween => {
                update(&mut self.state.paragraph.borders.between);
                true
            }
            BorderSelection::ParagraphBox => {
                update(&mut self.state.paragraph.borders.left);
                update(&mut self.state.paragraph.borders.right);
                update(&mut self.state.paragraph.borders.top);
                update(&mut self.state.paragraph.borders.bottom);
                true
            }
            BorderSelection::Page(_) => false,
        }
    }

    fn update_current_page_border(&mut self, update: impl FnOnce(&mut TableCellBorder)) -> bool {
        let BorderSelection::Page(side) = self.state.paragraph_border_selection else {
            return false;
        };
        let border = match side {
            TableCellBorderSide::Left => &mut self.current_section_page.page_borders.left,
            TableCellBorderSide::Right => &mut self.current_section_page.page_borders.right,
            TableCellBorderSide::Top => &mut self.current_section_page.page_borders.top,
            TableCellBorderSide::Bottom => &mut self.current_section_page.page_borders.bottom,
            TableCellBorderSide::DiagonalDown | TableCellBorderSide::DiagonalUp => return false,
        };
        update(border);
        self.upsert_current_section_settings();
        true
    }

    fn update_current_page_border_spacing(&mut self, value: i32) -> bool {
        let BorderSelection::Page(side) = self.state.paragraph_border_selection else {
            return false;
        };
        match side {
            TableCellBorderSide::Left => {
                self.current_section_page
                    .page_border_spacing_twips
                    .left_twips = value
            }
            TableCellBorderSide::Right => {
                self.current_section_page
                    .page_border_spacing_twips
                    .right_twips = value
            }
            TableCellBorderSide::Top => {
                self.current_section_page
                    .page_border_spacing_twips
                    .top_twips = value
            }
            TableCellBorderSide::Bottom => {
                self.current_section_page
                    .page_border_spacing_twips
                    .bottom_twips = value
            }
            TableCellBorderSide::DiagonalDown | TableCellBorderSide::DiagonalUp => return false,
        }
        self.upsert_current_section_settings();
        true
    }

    fn update_current_paragraph_or_character_border_spacing(&mut self, value: i32) -> bool {
        match self.state.paragraph_border_selection {
            BorderSelection::Character => {
                if self.current_list_level.is_some()
                    && self.state.list_context == ListContext::ListLevel
                {
                    self.update_current_list_level_character_style(|style| {
                        style.border.spacing_twips = value;
                    });
                    return true;
                }
                self.state.character.border.spacing_twips = value;
                true
            }
            BorderSelection::Paragraph(side) => {
                let border = match side {
                    TableCellBorderSide::Left => &mut self.state.paragraph.borders.left,
                    TableCellBorderSide::Right => &mut self.state.paragraph.borders.right,
                    TableCellBorderSide::Top => &mut self.state.paragraph.borders.top,
                    TableCellBorderSide::Bottom => &mut self.state.paragraph.borders.bottom,
                    TableCellBorderSide::DiagonalDown | TableCellBorderSide::DiagonalUp => {
                        return false;
                    }
                };
                border.spacing_twips = value;
                true
            }
            BorderSelection::ParagraphBetween => {
                self.state.paragraph.borders.between.spacing_twips = value;
                true
            }
            BorderSelection::ParagraphBox => {
                self.state.paragraph.borders.left.spacing_twips = value;
                self.state.paragraph.borders.right.spacing_twips = value;
                self.state.paragraph.borders.top.spacing_twips = value;
                self.state.paragraph.borders.bottom.spacing_twips = value;
                true
            }
            BorderSelection::Page(_) | BorderSelection::None => false,
        }
    }

    fn set_current_border_width(&mut self, value: Option<i32>, offset: usize) {
        let value = value
            .unwrap_or(TableCellBorder::default().width_twips)
            .max(0);
        let clamped = value.min(self.limits().max_table_border_width_twips);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("table border width clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        if !self.update_current_cell_border(|border| {
            border.width_twips = clamped;
        }) && !self.update_current_table_row_border(|border| {
            border.width_twips = clamped;
        }) && !self.update_current_paragraph_border(|border| {
            border.width_twips = clamped;
        }) && !self.update_current_page_border(|border| {
            border.width_twips = clamped;
        }) {
            self.set_current_table_borders_visible(true);
        }
    }

    fn set_current_border_color(&mut self, value: Option<i32>) {
        let color_index = value.unwrap_or(0).max(0) as usize;
        let color_index = if color_index == 0 {
            None
        } else {
            Some(color_index)
        };
        if !self.update_current_cell_border(|border| {
            border.color_index = color_index;
        }) && !self.update_current_table_row_border(|border| {
            border.color_index = color_index;
        }) && !self.update_current_paragraph_border(|border| {
            border.color_index = color_index;
        }) && !self.update_current_page_border(|border| {
            border.color_index = color_index;
        }) {
            self.set_current_table_borders_visible(true);
        }
    }

    fn set_current_border_spacing(&mut self, value: Option<i32>, offset: usize) {
        let value = value.unwrap_or(0).max(0);
        let max = self.limits().max_page_border_spacing_twips.max(0);
        let clamped = value.min(max);
        if clamped != value {
            let resource = if matches!(
                self.state.paragraph_border_selection,
                BorderSelection::Page(_)
            ) {
                "page border spacing"
            } else {
                "border spacing"
            };
            self.diagnostics.push(Diagnostic::warning(
                format!("{resource} clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        if !self.update_current_paragraph_or_character_border_spacing(clamped) {
            self.update_current_page_border_spacing(clamped);
        }
    }

    fn set_current_border_style(&mut self, style: BorderStyle) {
        if !self.update_current_cell_border(|border| {
            border.visible = true;
            border.style = style;
        }) && !self.update_current_table_row_border(|border| {
            border.visible = true;
            border.style = style;
        }) && !self.update_current_paragraph_border(|border| {
            border.visible = true;
            border.style = style;
        }) && !self.update_current_page_border(|border| {
            border.visible = true;
            border.style = style;
        }) {
            self.set_current_table_borders_visible(true);
        }
    }

    fn set_current_border_visible(&mut self, visible: bool) {
        if !self.set_current_cell_border_visible(visible)
            && !self.update_current_table_row_border(|border| {
                border.visible = visible;
            })
            && !self.update_current_paragraph_border(|border| {
                border.visible = visible;
            })
            && !self.update_current_page_border(|border| {
                border.visible = visible;
            })
        {
            self.set_current_table_borders_visible(visible);
        }
    }

    fn set_current_table_borders_visible(&mut self, visible: bool) {
        if self.current_table_row.is_some() {
            self.current_table
                .get_or_insert_with(TableBuilder::default)
                .borders_visible = visible;
        }
    }

    fn set_current_table_row_height(&mut self, value: Option<i32>, offset: usize) {
        let max_table_row_height_twips = self.limits().max_table_row_height_twips;
        let Some(row) = self.current_table_row.as_mut() else {
            return;
        };
        let raw_value = value.unwrap_or(0);
        let exact_height = raw_value < 0;
        let value = raw_value.checked_abs().unwrap_or(i32::MAX);
        if value == 0 {
            row.height_twips = None;
            return;
        }

        let clamped = value.min(max_table_row_height_twips);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("table row height clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        row.height_twips = Some(if exact_height { -clamped } else { clamped });
    }

    fn set_current_table_row_left_offset(&mut self, value: Option<i32>, offset: usize) {
        let max_table_row_offset_twips = self.limits().max_table_row_offset_twips;
        let Some(row) = self.current_table_row.as_mut() else {
            return;
        };
        let value = value.unwrap_or(0);
        let clamped = value.clamp(-max_table_row_offset_twips, max_table_row_offset_twips);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("table row left offset clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        row.left_offset_twips = clamped;
    }

    fn set_current_table_cell_gap(&mut self, value: Option<i32>, offset: usize) {
        let max_table_cell_gap_twips = self.limits().max_table_cell_gap_twips;
        let Some(row) = self.current_table_row.as_mut() else {
            return;
        };
        let value = value.unwrap_or(DEFAULT_TABLE_CELL_GAP_TWIPS).max(0);
        let clamped = value.min(max_table_cell_gap_twips);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("table cell gap clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        row.cell_gap_twips = clamped;
    }

    fn set_current_table_row_alignment(&mut self, alignment: TableRowAlignment) {
        if let Some(row) = self.current_table_row.as_mut() {
            row.alignment = alignment;
        }
    }

    fn set_current_table_row_right_to_left(&mut self, right_to_left: bool) {
        if let Some(row) = self.current_table_row.as_mut() {
            row.right_to_left = right_to_left;
        }
    }

    fn set_current_table_row_repeat_header(&mut self, repeat_header: bool) {
        if let Some(row) = self.current_table_row.as_mut() {
            row.repeat_header = repeat_header;
        }
    }

    fn set_current_table_row_keep_together(&mut self, keep_together: bool) {
        if let Some(row) = self.current_table_row.as_mut() {
            row.keep_together = keep_together;
        }
    }

    fn finish_table_cell(&mut self, offset: usize) -> Result<(), ParseError> {
        let max_table_cells = self.limits().max_table_cells;
        let page_content_width_twips = self.current_page_content_width_twips();
        let max_width_twips = self.limits().max_page_dimension_twips;
        let Some(row) = self.current_table_row.as_mut() else {
            return self.push_text("\t", offset);
        };

        self.table_cell_count =
            self.table_cell_count
                .checked_add(1)
                .ok_or(ParseError::ResourceLimitExceeded {
                    resource: "table cells".to_string(),
                    offset,
                })?;
        if self.table_cell_count > max_table_cells {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "table cells".to_string(),
                offset,
            });
        }

        if !row.current_cell_paragraph.runs.is_empty() {
            let paragraph = std::mem::replace(
                &mut row.current_cell_paragraph,
                Paragraph {
                    style: self.state.paragraph.clone(),
                    runs: Vec::new(),
                },
            );
            row.current_cell_paragraphs.push(paragraph);
        }

        let mut paragraphs = if row.current_cell_paragraphs.is_empty() {
            vec![Paragraph::default()]
        } else {
            std::mem::take(&mut row.current_cell_paragraphs)
        };
        let cell_index = row.cells.len();
        let shading_color_index = row
            .cell_shading_color_indices
            .get(cell_index)
            .copied()
            .flatten();
        let shading_basis_points = row
            .cell_shading_basis_points
            .get(cell_index)
            .copied()
            .unwrap_or(10_000);
        let shading_pattern = row
            .cell_shading_patterns
            .get(cell_index)
            .copied()
            .unwrap_or_default();
        let padding = row
            .cell_paddings
            .get(cell_index)
            .copied()
            .unwrap_or_default();
        let borders = row
            .cell_borders
            .get(cell_index)
            .copied()
            .unwrap_or_default();
        let no_wrap = row.cell_no_wraps.get(cell_index).copied().unwrap_or(false);
        if no_wrap {
            for paragraph in &mut paragraphs {
                paragraph.style.no_wrap = true;
            }
        }
        let text_direction = row
            .cell_text_directions
            .get(cell_index)
            .copied()
            .unwrap_or_default();
        normalize_table_cell_text_direction(&mut paragraphs, text_direction);
        let vertical_align = row
            .cell_vertical_alignments
            .get(cell_index)
            .copied()
            .unwrap_or_default();
        let horizontal_merge = row
            .cell_horizontal_merges
            .get(cell_index)
            .copied()
            .unwrap_or_default();
        let vertical_merge = row
            .cell_vertical_merges
            .get(cell_index)
            .copied()
            .unwrap_or_default();
        row.cells.push(TableCell {
            paragraphs,
            shading_color_index,
            shading_basis_points,
            shading_pattern,
            padding,
            borders,
            vertical_align,
            horizontal_merge,
            vertical_merge,
        });
        if row.cell_right_edges_twips.len() <= cell_index {
            let preferred_width_twips = row
                .cell_preferred_widths_twips
                .get(cell_index)
                .copied()
                .flatten()
                .or_else(|| {
                    normalized_preferred_table_width_twips(
                        row.current_cell_preferred_width,
                        page_content_width_twips,
                        max_width_twips,
                    )
                });
            if let Some(width_twips) = preferred_width_twips {
                let previous_edge = row.cell_right_edges_twips.last().copied().unwrap_or(0);
                row.cell_right_edges_twips
                    .push(previous_edge.saturating_add(width_twips.max(1)));
            }
        }
        row.current_cell_preferred_width = PreferredTableWidth::default();
        row.cell_open = false;
        Ok(())
    }

    fn finish_table_row(&mut self, offset: usize) -> Result<(), ParseError> {
        let Some(mut row) = self.current_table_row.take() else {
            self.finish_paragraph();
            return Ok(());
        };

        if row.cell_open || row.cells.is_empty() {
            self.current_table_row = Some(row);
            self.finish_table_cell(offset)?;
            row = self
                .current_table_row
                .take()
                .expect("finish_table_cell preserves the active row");
        }

        if row.right_to_left {
            Self::normalize_right_to_left_table_row(&mut row);
        }
        Self::apply_table_row_borders(&mut row);

        let table = self.current_table.get_or_insert_with(TableBuilder::default);
        table.merge_cell_right_edges(&row.cell_right_edges_twips);
        table.rows.push(TableRow {
            cells: row.cells,
            height_twips: row.height_twips,
            left_offset_twips: row.left_offset_twips,
            cell_gap_twips: row.cell_gap_twips,
            alignment: row.alignment,
            repeat_header: row.repeat_header,
            keep_together: row.keep_together,
        });
        Ok(())
    }

    fn normalize_right_to_left_table_row(row: &mut TableRowBuilder) {
        if row.cells.len() <= 1 {
            return;
        }

        let cells = std::mem::take(&mut row.cells);
        let mut cell_border_flags = std::mem::take(&mut row.cell_border_flags);
        let mut cell_widths = Self::table_row_cell_widths_twips(&row.cell_right_edges_twips);
        cell_border_flags.resize(cells.len(), TableCellBorderFlags::default());
        cell_widths.resize(cells.len(), 1);

        let mut groups = Vec::new();
        let mut idx = 0;
        while idx < cells.len() {
            let mut span = 1;
            if cells[idx].horizontal_merge == TableCellHorizontalMerge::First {
                while idx + span < cells.len()
                    && cells[idx + span].horizontal_merge == TableCellHorizontalMerge::Continuation
                {
                    span += 1;
                }
            }

            groups.push((
                cells[idx..idx + span].to_vec(),
                cell_border_flags[idx..idx + span].to_vec(),
                cell_widths[idx..idx + span].to_vec(),
            ));
            idx += span;
        }

        groups.reverse();

        let mut visual_widths = Vec::new();
        for (cells, flags, widths) in groups {
            row.cells.extend(cells);
            row.cell_border_flags.extend(flags);
            visual_widths.extend(widths);
        }
        row.cell_right_edges_twips = Self::cell_right_edges_from_widths_twips(&visual_widths);
    }

    fn table_row_cell_widths_twips(edges: &[i32]) -> Vec<i32> {
        let mut widths = Vec::new();
        let mut previous = 0;
        for edge in edges {
            widths.push((*edge - previous).max(1));
            previous = *edge;
        }
        widths
    }

    fn cell_right_edges_from_widths_twips(widths: &[i32]) -> Vec<i32> {
        let mut edge = 0;
        widths
            .iter()
            .map(|width| {
                edge += (*width).max(1);
                edge
            })
            .collect()
    }

    fn apply_table_row_borders(row: &mut TableRowBuilder) {
        let cell_count = row.cells.len();
        for (idx, cell) in row.cells.iter_mut().enumerate() {
            let cell_flags = row.cell_border_flags.get(idx).copied().unwrap_or_default();
            if row.row_border_flags.is_set(TableCellBorderSide::Top)
                && !cell_flags.is_set(TableCellBorderSide::Top)
            {
                cell.borders.top = row.row_borders.top;
            }
            if row.row_border_flags.is_set(TableCellBorderSide::Bottom)
                && !cell_flags.is_set(TableCellBorderSide::Bottom)
            {
                cell.borders.bottom = row.row_borders.bottom;
            }
            if idx == 0
                && row.row_border_flags.is_set(TableCellBorderSide::Left)
                && !cell_flags.is_set(TableCellBorderSide::Left)
            {
                cell.borders.left = row.row_borders.left;
            }
            if idx + 1 == cell_count
                && row.row_border_flags.is_set(TableCellBorderSide::Right)
                && !cell_flags.is_set(TableCellBorderSide::Right)
            {
                cell.borders.right = row.row_borders.right;
            }
        }
    }

    fn finish_table(&mut self, offset: usize) -> Result<(), ParseError> {
        if self.current_table_row.is_some() {
            self.finish_table_row(offset)?;
        }

        let Some(table) = self.current_table.take() else {
            return Ok(());
        };

        if table.rows.is_empty() {
            return Ok(());
        }

        let column_widths_twips = table.column_widths_twips();
        self.document.blocks.push(Block::Table(Table {
            rows: table.rows,
            column_widths_twips,
            borders_visible: table.borders_visible,
        }));
        Ok(())
    }

    fn finish_picture(&mut self, offset: usize) -> Result<(), ParseError> {
        let Some(picture) = self.current_picture.take() else {
            return Ok(());
        };

        if picture.pending_hex.is_some() {
            self.diagnostics.push(Diagnostic::warning(
                "picture data had an odd trailing hex nibble and was skipped",
                Some(offset),
            ));
            self.push_placeholder("[Image skipped: malformed picture data]".to_string());
            return Ok(());
        }

        if picture.bytes.is_empty() {
            self.push_placeholder("[Image skipped: empty picture]".to_string());
            return Ok(());
        }

        self.image_count =
            self.image_count
                .checked_add(1)
                .ok_or(ParseError::ResourceLimitExceeded {
                    resource: "images".to_string(),
                    offset,
                })?;
        if self.image_count > self.limits().max_images {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "images".to_string(),
                offset,
            });
        }

        match picture.kind {
            PictureKind::Jpeg => match parse_jpeg_image_data(&picture.bytes) {
                Some(jpeg) => {
                    let width_px = jpeg.width_px;
                    let height_px = jpeg.height_px;
                    self.ensure_image_pixels(width_px, height_px, offset)?;
                    self.push_static_image(
                        picture.owner_destination,
                        StaticImage {
                            format: jpeg.format,
                            bytes: picture.bytes,
                            palette: Vec::new(),
                            width_px,
                            height_px,
                            natural_width_px_hint: picture.width_px_hint,
                            natural_height_px_hint: picture.height_px_hint,
                            display_width_twips: picture.display_width_twips,
                            display_height_twips: picture.display_height_twips,
                            scale_x_percent: picture.scale_x_percent,
                            scale_y_percent: picture.scale_y_percent,
                            crop: picture.crop,
                        },
                    );
                    if self.state.inside_shape_picture {
                        self.state.shape_picture_rendered = true;
                    }
                }
                None => {
                    self.diagnostics.push(Diagnostic::warning(
                        "JPEG picture data was malformed and replaced with a placeholder",
                        Some(offset),
                    ));
                    self.push_placeholder("[Image skipped: malformed JPEG]".to_string());
                }
            },
            PictureKind::Png => match parse_png_image_data(&picture.bytes) {
                Some(png) => {
                    self.ensure_image_pixels(png.width_px, png.height_px, offset)?;
                    self.push_static_image(
                        picture.owner_destination,
                        StaticImage {
                            format: png.format,
                            bytes: png.idat,
                            palette: png.palette,
                            width_px: png.width_px,
                            height_px: png.height_px,
                            natural_width_px_hint: picture.width_px_hint,
                            natural_height_px_hint: picture.height_px_hint,
                            display_width_twips: picture.display_width_twips,
                            display_height_twips: picture.display_height_twips,
                            scale_x_percent: picture.scale_x_percent,
                            scale_y_percent: picture.scale_y_percent,
                            crop: picture.crop,
                        },
                    );
                    if self.state.inside_shape_picture {
                        self.state.shape_picture_rendered = true;
                    }
                }
                None => {
                    self.diagnostics.push(Diagnostic::warning(
                            "PNG picture data was unsupported or malformed and replaced with a placeholder",
                            Some(offset),
                        ));
                    self.push_placeholder("[Image skipped: unsupported PNG]".to_string());
                }
            },
            PictureKind::Dib => {
                match parse_dib_image_data(&picture.bytes, self.limits().max_image_pixels) {
                    Some(dib) => {
                        self.ensure_image_pixels(dib.width_px, dib.height_px, offset)?;
                        self.push_static_image(
                            picture.owner_destination,
                            StaticImage {
                                format: ImageFormat::Rgb8,
                                bytes: dib.rgb,
                                palette: Vec::new(),
                                width_px: dib.width_px,
                                height_px: dib.height_px,
                                natural_width_px_hint: picture.width_px_hint,
                                natural_height_px_hint: picture.height_px_hint,
                                display_width_twips: picture.display_width_twips,
                                display_height_twips: picture.display_height_twips,
                                scale_x_percent: picture.scale_x_percent,
                                scale_y_percent: picture.scale_y_percent,
                                crop: picture.crop,
                            },
                        );
                        if self.state.inside_shape_picture {
                            self.state.shape_picture_rendered = true;
                        }
                    }
                    None => {
                        self.diagnostics.push(Diagnostic::warning(
                        "DIB picture data was unsupported or malformed and replaced with a placeholder",
                        Some(offset),
                    ));
                        self.push_placeholder("[Image skipped: unsupported DIB]".to_string());
                    }
                }
            }
            PictureKind::Wmf
            | PictureKind::Emf
            | PictureKind::Unsupported
            | PictureKind::Unknown => {
                self.diagnostics.push(Diagnostic::warning(
                    "unsupported picture format replaced with a placeholder",
                    Some(offset),
                ));
                self.push_placeholder("[Image skipped: unsupported format]".to_string());
            }
        }
        Ok(())
    }

    fn push_static_image(&mut self, destination: Destination, image: StaticImage) {
        if is_header_destination(destination) {
            self.finish_header_paragraph();
            if self.has_started_visible_body() {
                match destination {
                    Destination::FirstPageHeader => self
                        .current_section_page
                        .first_page_header_images
                        .push(image),
                    Destination::EvenPageHeader => self
                        .current_section_page
                        .even_page_header_images
                        .push(image),
                    _ => self.current_section_page.header_images.push(image),
                }
                self.upsert_current_section_settings();
            } else {
                match destination {
                    Destination::FirstPageHeader => {
                        self.document.first_page_header_images.push(image)
                    }
                    Destination::EvenPageHeader => {
                        self.document.even_page_header_images.push(image)
                    }
                    _ => self.document.header_images.push(image),
                }
            }
        } else if is_footer_destination(destination) {
            self.finish_footer_paragraph();
            if self.has_started_visible_body() {
                match destination {
                    Destination::FirstPageFooter => self
                        .current_section_page
                        .first_page_footer_images
                        .push(image),
                    Destination::EvenPageFooter => self
                        .current_section_page
                        .even_page_footer_images
                        .push(image),
                    _ => self.current_section_page.footer_images.push(image),
                }
                self.upsert_current_section_settings();
            } else {
                match destination {
                    Destination::FirstPageFooter => {
                        self.document.first_page_footer_images.push(image)
                    }
                    Destination::EvenPageFooter => {
                        self.document.even_page_footer_images.push(image)
                    }
                    _ => self.document.footer_images.push(image),
                }
            }
        } else {
            self.document.blocks.push(Block::Image(image));
        }
    }

    fn finish_shape(&mut self, offset: usize) -> Result<bool, ParseError> {
        let Some(shape) = self.current_shape.take() else {
            return Ok(false);
        };
        let Some(mut kind) = shape.kind else {
            return Ok(false);
        };
        if kind == StaticShapeKind::Rectangle && shape.rounded_rectangle {
            kind = StaticShapeKind::RoundedRectangle;
        }
        match kind {
            StaticShapeKind::Polyline if shape.points.len() < 2 => return Ok(false),
            StaticShapeKind::Polygon if shape.points.len() < 3 => return Ok(false),
            _ => {}
        }
        let (left_twips, top_twips, width_twips, height_twips, points) =
            if matches!(kind, StaticShapeKind::Polyline | StaticShapeKind::Polygon) {
                let base_x = shape.base_x_twips.saturating_add(shape.left_twips);
                let base_y = shape.base_y_twips.saturating_add(shape.top_twips);
                let absolute_points = shape
                    .points
                    .iter()
                    .map(|point| StaticShapePoint {
                        x_twips: base_x.saturating_add(point.x_twips),
                        y_twips: base_y.saturating_add(point.y_twips),
                    })
                    .collect::<Vec<_>>();
                normalize_shape_points(
                    &absolute_points,
                    self.limits().max_shape_dimension_twips.max(1),
                )
                .unwrap_or((0, 0, 0, 0, Vec::new()))
            } else {
                (
                    shape
                        .base_x_twips
                        .saturating_add(shape.left_twips)
                        .clamp(0, self.limits().max_shape_offset_twips.max(0)),
                    shape
                        .base_y_twips
                        .saturating_add(shape.top_twips)
                        .clamp(0, self.limits().max_shape_offset_twips.max(0)),
                    shape
                        .width_twips
                        .clamp(1, self.limits().max_shape_dimension_twips.max(1)),
                    shape
                        .height_twips
                        .clamp(1, self.limits().max_shape_dimension_twips.max(1)),
                    Vec::new(),
                )
            };
        if width_twips <= 0 || height_twips <= 0 {
            return Ok(false);
        }
        if self.shape_count >= self.limits().max_shapes {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "shapes".to_string(),
                offset,
            });
        }
        self.shape_count += 1;
        self.push_static_shape(
            shape.owner_destination,
            StaticShape {
                kind,
                left_twips,
                top_twips,
                width_twips,
                height_twips,
                stroke_width_twips: shape
                    .stroke_width_twips
                    .clamp(0, self.limits().max_shape_stroke_width_twips.max(1)),
                stroke_color: shape.stroke_color,
                stroke_style: shape.stroke_style,
                fill_color: shape.fill_color,
                points,
            },
        );
        self.diagnostics.push(Diagnostic::warning(
            "rendering bounded passive static drawing shape and stripping raw drawing properties",
            Some(offset),
        ));
        Ok(true)
    }

    fn push_static_shape(&mut self, destination: Destination, shape: StaticShape) {
        if is_header_destination(destination) {
            self.finish_header_paragraph();
            if self.has_started_visible_body() {
                match destination {
                    Destination::FirstPageHeader => self
                        .current_section_page
                        .first_page_header_shapes
                        .push(shape),
                    Destination::EvenPageHeader => self
                        .current_section_page
                        .even_page_header_shapes
                        .push(shape),
                    _ => self.current_section_page.header_shapes.push(shape),
                }
                self.upsert_current_section_settings();
            } else {
                match destination {
                    Destination::FirstPageHeader => {
                        self.document.first_page_header_shapes.push(shape)
                    }
                    Destination::EvenPageHeader => {
                        self.document.even_page_header_shapes.push(shape)
                    }
                    _ => self.document.header_shapes.push(shape),
                }
            }
        } else if is_footer_destination(destination) {
            self.finish_footer_paragraph();
            if self.has_started_visible_body() {
                match destination {
                    Destination::FirstPageFooter => self
                        .current_section_page
                        .first_page_footer_shapes
                        .push(shape),
                    Destination::EvenPageFooter => self
                        .current_section_page
                        .even_page_footer_shapes
                        .push(shape),
                    _ => self.current_section_page.footer_shapes.push(shape),
                }
                self.upsert_current_section_settings();
            } else {
                match destination {
                    Destination::FirstPageFooter => {
                        self.document.first_page_footer_shapes.push(shape)
                    }
                    Destination::EvenPageFooter => {
                        self.document.even_page_footer_shapes.push(shape)
                    }
                    _ => self.document.footer_shapes.push(shape),
                }
            }
        } else {
            self.document.blocks.push(Block::Shape(shape));
        }
    }

    fn set_current_shape_kind(&mut self, kind: StaticShapeKind) {
        if let Some(shape) = self.current_shape.as_mut() {
            shape.kind = Some(kind);
        }
    }

    fn set_current_shape_rounded_rectangle(&mut self) {
        if let Some(shape) = self.current_shape.as_mut() {
            shape.rounded_rectangle = true;
        }
    }

    fn set_current_shape_base_x(&mut self, value: Option<i32>, offset: usize) {
        let value = self.clamp_shape_offset(value.unwrap_or(0), "shape base x", offset);
        if let Some(shape) = self.current_shape.as_mut() {
            shape.base_x_twips = value;
        }
    }

    fn set_current_shape_base_y(&mut self, value: Option<i32>, offset: usize) {
        let value = self.clamp_shape_offset(value.unwrap_or(0), "shape base y", offset);
        if let Some(shape) = self.current_shape.as_mut() {
            shape.base_y_twips = value;
        }
    }

    fn set_current_shape_left(&mut self, value: Option<i32>, offset: usize) {
        let value = self.clamp_shape_offset(value.unwrap_or(0), "shape left", offset);
        if let Some(shape) = self.current_shape.as_mut() {
            shape.left_twips = value;
        }
    }

    fn set_current_shape_top(&mut self, value: Option<i32>, offset: usize) {
        let value = self.clamp_shape_offset(value.unwrap_or(0), "shape top", offset);
        if let Some(shape) = self.current_shape.as_mut() {
            shape.top_twips = value;
        }
    }

    fn set_current_shape_width(&mut self, value: Option<i32>, offset: usize) {
        let value = self.clamp_shape_dimension(value.unwrap_or(0), "shape width", offset);
        if let Some(shape) = self.current_shape.as_mut() {
            shape.width_twips = value;
        }
    }

    fn set_current_shape_height(&mut self, value: Option<i32>, offset: usize) {
        let value = self.clamp_shape_dimension(value.unwrap_or(0), "shape height", offset);
        if let Some(shape) = self.current_shape.as_mut() {
            shape.height_twips = value;
        }
    }

    fn set_current_shape_point_x(&mut self, value: Option<i32>, offset: usize) {
        let value = self.clamp_shape_offset(value.unwrap_or(0), "shape point x", offset);
        if let Some(shape) = self.current_shape.as_mut() {
            shape.pending_point_x_twips = Some(value);
        }
    }

    fn push_current_shape_point_y(
        &mut self,
        value: Option<i32>,
        offset: usize,
    ) -> Result<(), ParseError> {
        let y_twips = self.clamp_shape_offset(value.unwrap_or(0), "shape point y", offset);
        let max_shape_points = self.limits().max_shape_points;
        let Some(shape) = self.current_shape.as_mut() else {
            return Ok(());
        };
        let Some(x_twips) = shape.pending_point_x_twips.take() else {
            return Ok(());
        };
        if shape.points.len() >= max_shape_points {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "shape points".to_string(),
                offset,
            });
        }
        shape.points.push(StaticShapePoint { x_twips, y_twips });
        Ok(())
    }

    fn set_current_shape_stroke_width(&mut self, value: Option<i32>, offset: usize) {
        let value = self.clamp_shape_stroke_width(value.unwrap_or(15), offset);
        if let Some(shape) = self.current_shape.as_mut() {
            shape.stroke_width_twips = value;
        }
    }

    fn set_current_shape_stroke_style(&mut self, style: BorderStyle) {
        if let Some(shape) = self.current_shape.as_mut() {
            shape.stroke_style = style;
        }
    }

    fn set_current_shape_stroke_red(&mut self, value: Option<i32>) {
        if let Some(shape) = self.current_shape.as_mut() {
            shape.stroke_color.red = value.unwrap_or(0).clamp(0, 255) as u8;
        }
    }

    fn set_current_shape_stroke_green(&mut self, value: Option<i32>) {
        if let Some(shape) = self.current_shape.as_mut() {
            shape.stroke_color.green = value.unwrap_or(0).clamp(0, 255) as u8;
        }
    }

    fn set_current_shape_stroke_blue(&mut self, value: Option<i32>) {
        if let Some(shape) = self.current_shape.as_mut() {
            shape.stroke_color.blue = value.unwrap_or(0).clamp(0, 255) as u8;
        }
    }

    fn set_current_shape_fill_red(&mut self, value: Option<i32>) {
        if let Some(shape) = self.current_shape.as_mut() {
            let color = shape.fill_color.get_or_insert(Color {
                red: 255,
                green: 255,
                blue: 255,
            });
            color.red = value.unwrap_or(0).clamp(0, 255) as u8;
        }
    }

    fn set_current_shape_fill_green(&mut self, value: Option<i32>) {
        if let Some(shape) = self.current_shape.as_mut() {
            let color = shape.fill_color.get_or_insert(Color {
                red: 255,
                green: 255,
                blue: 255,
            });
            color.green = value.unwrap_or(0).clamp(0, 255) as u8;
        }
    }

    fn set_current_shape_fill_blue(&mut self, value: Option<i32>) {
        if let Some(shape) = self.current_shape.as_mut() {
            let color = shape.fill_color.get_or_insert(Color {
                red: 255,
                green: 255,
                blue: 255,
            });
            color.blue = value.unwrap_or(0).clamp(0, 255) as u8;
        }
    }

    fn set_current_shape_fill_pattern(&mut self, value: Option<i32>) {
        if let Some(shape) = self.current_shape.as_mut() {
            if value.unwrap_or(1) == 0 {
                shape.fill_color = None;
            } else {
                shape.fill_color.get_or_insert(Color {
                    red: 255,
                    green: 255,
                    blue: 255,
                });
            }
        }
    }

    fn clamp_shape_offset(&mut self, value: i32, label: &str, offset: usize) -> i32 {
        let limit = self.limits().max_shape_offset_twips.max(0);
        let clamped = value.clamp(0, limit);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("{label} clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        clamped
    }

    fn clamp_shape_dimension(&mut self, value: i32, label: &str, offset: usize) -> i32 {
        let max = self.limits().max_shape_dimension_twips.max(1);
        let clamped = value.clamp(0, max);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("{label} clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        clamped
    }

    fn clamp_shape_stroke_width(&mut self, value: i32, offset: usize) -> i32 {
        let max = self.limits().max_shape_stroke_width_twips.max(1);
        let clamped = value.clamp(0, max);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("shape stroke width clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        clamped
    }

    fn set_picture_kind(&mut self, kind: PictureKind) {
        if let Some(picture) = self.current_picture.as_mut() {
            picture.kind = kind;
        }
    }

    fn set_picture_width_hint(&mut self, width: Option<i32>, offset: usize) {
        let normalized = self.clamp_picture_dimension_hint(width, "width", offset);
        if let Some(picture) = self.current_picture.as_mut() {
            picture.width_px_hint = normalized;
        }
    }

    fn set_picture_height_hint(&mut self, height: Option<i32>, offset: usize) {
        let normalized = self.clamp_picture_dimension_hint(height, "height", offset);
        if let Some(picture) = self.current_picture.as_mut() {
            picture.height_px_hint = normalized;
        }
    }

    fn clamp_picture_dimension_hint(
        &mut self,
        dimension: Option<i32>,
        axis: &str,
        offset: usize,
    ) -> Option<u32> {
        let value = dimension.unwrap_or(0);
        let max = self
            .limits()
            .max_image_dimension_hint_px
            .clamp(1, i32::MAX as u32) as i32;
        let clamped = value.clamp(0, max);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("picture natural {axis} clamped from {value} to {clamped} px"),
                Some(offset),
            ));
        }
        (clamped > 0).then_some(clamped as u32)
    }

    fn set_picture_display_width(&mut self, width: Option<i32>, offset: usize) {
        let normalized = self.clamp_picture_display_dimension(width, "width", offset);
        if let Some(picture) = self.current_picture.as_mut() {
            picture.display_width_twips = Some(normalized);
        }
    }

    fn set_picture_display_height(&mut self, height: Option<i32>, offset: usize) {
        let normalized = self.clamp_picture_display_dimension(height, "height", offset);
        if let Some(picture) = self.current_picture.as_mut() {
            picture.display_height_twips = Some(normalized);
        }
    }

    fn clamp_picture_display_dimension(
        &mut self,
        dimension: Option<i32>,
        axis: &str,
        offset: usize,
    ) -> i32 {
        let value = dimension.unwrap_or(0);
        let max = self.limits().max_image_display_twips.max(1);
        let clamped = value.clamp(0, max);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("picture display {axis} clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        clamped
    }

    fn set_picture_scale_x(&mut self, scale: Option<i32>, offset: usize) {
        let normalized = self.clamp_picture_scale(scale, "horizontal", offset);
        if let Some(picture) = self.current_picture.as_mut() {
            picture.scale_x_percent = normalized;
        }
    }

    fn set_picture_scale_y(&mut self, scale: Option<i32>, offset: usize) {
        let normalized = self.clamp_picture_scale(scale, "vertical", offset);
        if let Some(picture) = self.current_picture.as_mut() {
            picture.scale_y_percent = normalized;
        }
    }

    fn clamp_picture_scale(
        &mut self,
        scale: Option<i32>,
        axis: &str,
        offset: usize,
    ) -> Option<i32> {
        let value = scale.unwrap_or(100);
        let clamped = value.clamp(
            self.limits().min_image_scaling_percent,
            self.limits().max_image_scaling_percent,
        );
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("picture {axis} scaling clamped from {value}% to {clamped}%"),
                Some(offset),
            ));
        }
        Some(clamped)
    }

    fn set_picture_crop_left(&mut self, crop: Option<i32>, offset: usize) {
        let normalized = self.clamp_picture_crop(crop, "left", offset);
        if let Some(picture) = self.current_picture.as_mut() {
            picture.crop.left_twips = normalized;
        }
    }

    fn set_picture_crop_top(&mut self, crop: Option<i32>, offset: usize) {
        let normalized = self.clamp_picture_crop(crop, "top", offset);
        if let Some(picture) = self.current_picture.as_mut() {
            picture.crop.top_twips = normalized;
        }
    }

    fn set_picture_crop_right(&mut self, crop: Option<i32>, offset: usize) {
        let normalized = self.clamp_picture_crop(crop, "right", offset);
        if let Some(picture) = self.current_picture.as_mut() {
            picture.crop.right_twips = normalized;
        }
    }

    fn set_picture_crop_bottom(&mut self, crop: Option<i32>, offset: usize) {
        let normalized = self.clamp_picture_crop(crop, "bottom", offset);
        if let Some(picture) = self.current_picture.as_mut() {
            picture.crop.bottom_twips = normalized;
        }
    }

    fn clamp_picture_crop(&mut self, crop: Option<i32>, side: &str, offset: usize) -> i32 {
        let value = crop.unwrap_or(0);
        let limit = self.limits().max_image_crop_twips;
        let clamped = value.clamp(-limit, limit);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("picture {side} crop clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        clamped
    }

    fn push_picture_hex_text(&mut self, text: &str, offset: usize) -> Result<(), ParseError> {
        for byte in text.bytes() {
            if byte.is_ascii_whitespace() {
                continue;
            }
            let Some(value) = hex_value(byte) else {
                self.diagnostics.push(Diagnostic::warning(
                    "non-hex picture data ignored and picture replaced with placeholder",
                    Some(offset),
                ));
                self.current_picture = None;
                self.push_placeholder("[Image skipped: malformed picture data]".to_string());
                return Ok(());
            };
            let Some(picture) = self.current_picture.as_mut() else {
                return Ok(());
            };
            if let Some(high) = picture.pending_hex.take() {
                self.push_picture_byte((high << 4) | value, offset)?;
            } else {
                picture.pending_hex = Some(value);
            }
        }
        Ok(())
    }

    fn push_picture_bytes(&mut self, bytes: &[u8], offset: usize) -> Result<(), ParseError> {
        for byte in bytes {
            self.push_picture_byte(*byte, offset)?;
        }
        Ok(())
    }

    fn push_picture_byte(&mut self, byte: u8, offset: usize) -> Result<(), ParseError> {
        let max_size = self.limits().max_binary_blob_size;
        let Some(picture) = self.current_picture.as_mut() else {
            return Ok(());
        };
        if picture.bytes.len() >= max_size {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "picture bytes".to_string(),
                offset,
            });
        }
        picture.bytes.push(byte);
        Ok(())
    }

    fn ensure_image_pixels(
        &self,
        width_px: u32,
        height_px: u32,
        offset: usize,
    ) -> Result<(), ParseError> {
        let pixels = (width_px as usize).checked_mul(height_px as usize).ok_or(
            ParseError::ResourceLimitExceeded {
                resource: "image pixels".to_string(),
                offset,
            },
        )?;
        if pixels > self.limits().max_image_pixels {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "image pixels".to_string(),
                offset,
            });
        }
        Ok(())
    }

    fn push_font_text(&mut self, text: &str, offset: usize) -> Result<(), ParseError> {
        if let Some(font) = self.current_font.as_mut() {
            for segment in text.split(';') {
                let trimmed = segment.trim();
                if !trimmed.is_empty() {
                    if !font.name.is_empty() {
                        font.name.push(' ');
                    }
                    font.name.push_str(trimmed);
                }
            }
            if text.contains(';') {
                self.flush_font(offset)?;
            }
        }
        Ok(())
    }

    fn push_font_alternate_text(&mut self, text: &str, _offset: usize) -> Result<(), ParseError> {
        if let Some(font) = self.current_font.as_mut() {
            for segment in text.split(';') {
                let trimmed = segment.trim();
                if !trimmed.is_empty() {
                    let alternate = font.alternate_name.get_or_insert_with(String::new);
                    if !alternate.is_empty() {
                        alternate.push(' ');
                    }
                    alternate.push_str(trimmed);
                }
            }
        }
        Ok(())
    }

    fn flush_font(&mut self, offset: usize) -> Result<(), ParseError> {
        let Some(mut font) = self.current_font.take() else {
            return Ok(());
        };

        font.name = font.name.trim().trim_end_matches(';').to_string();
        font.alternate_name = font
            .alternate_name
            .take()
            .map(|name| name.trim().trim_end_matches(';').to_string())
            .filter(|name| !name.is_empty());
        if font.name.is_empty() {
            font.name = format!("Font{}", font.index);
        }

        if let Some(existing) = self
            .document
            .fonts
            .iter_mut()
            .find(|existing| existing.index == font.index)
        {
            *existing = font;
        } else {
            if self.document.fonts.len() >= self.limits().max_fonts {
                return Err(ParseError::ResourceLimitExceeded {
                    resource: "fonts".to_string(),
                    offset,
                });
            }
            self.document.fonts.push(font);
        }
        Ok(())
    }

    fn set_current_font_family(&mut self, family: FontFamilyHint) {
        if let Some(font) = self.current_font.as_mut() {
            font.family = family;
        }
    }

    fn set_current_font_pitch(&mut self, pitch: FontPitch) {
        if let Some(font) = self.current_font.as_mut() {
            font.pitch = pitch;
        }
    }

    fn decode_text_hex_byte(&self, byte: u8) -> char {
        let font = self
            .document
            .fonts
            .iter()
            .find(|font| font.index == self.state.character.font_index);
        let code_page = match font.and_then(|font| font.code_page) {
            Some(code_page) => {
                CodePage::from_rtf_code_page(code_page).unwrap_or(CodePage::Unsupported)
            }
            None => font
                .and_then(|font| font.charset)
                .and_then(CodePage::from_font_charset)
                .unwrap_or(self.state.code_page),
        };
        decode_hex_byte(byte, code_page)
    }

    fn push_color_text(&mut self, text: &str, offset: usize) -> Result<(), ParseError> {
        for ch in text.chars() {
            if ch == ';' {
                if self.current_color_seen {
                    if self.document.colors.len() >= self.limits().max_colors {
                        return Err(ParseError::ResourceLimitExceeded {
                            resource: "colors".to_string(),
                            offset,
                        });
                    }
                    self.document.colors.push(self.current_color);
                }
                self.current_color = Color::default();
                self.current_color_seen = false;
            }
        }
        Ok(())
    }

    fn take_pending_or_synthesized_list_marker(
        &mut self,
        offset: usize,
    ) -> Result<Option<PendingListMarker>, ParseError> {
        if self.state.destination != Destination::Body || !self.current_output_paragraph_is_empty()
        {
            return Ok(None);
        }

        if !self.pending_list_marker.is_empty() {
            self.pending_old_style_list_marker = None;
            return Ok(Some(PendingListMarker {
                text: std::mem::take(&mut self.pending_list_marker),
                character_style: None,
                runs: std::mem::take(&mut self.pending_list_marker_runs),
            }));
        }

        if let Some(marker) = self.take_old_style_list_marker(offset)? {
            return Ok(Some(marker));
        }

        let Some(override_index) = self.state.list_override_index else {
            return Ok(None);
        };
        self.synthesize_list_marker(override_index, self.state.list_level_index, offset)
    }

    fn current_output_paragraph_is_empty(&self) -> bool {
        if let Some(row) = self.current_table_row.as_ref() {
            row.current_cell_paragraph.runs.is_empty()
        } else {
            self.current_paragraph.runs.is_empty()
        }
    }

    fn synthesize_list_marker(
        &mut self,
        override_index: i32,
        level_index: usize,
        offset: usize,
    ) -> Result<Option<PendingListMarker>, ParseError> {
        let list_override = self
            .list_overrides
            .iter()
            .find(|list_override| list_override.override_index == override_index)
            .cloned();
        let Some(list_override) = list_override else {
            return Ok(None);
        };
        let list = self
            .list_definitions
            .iter()
            .find(|list| list.list_id == list_override.list_id)
            .cloned();
        let Some(list) = list else {
            return Ok(None);
        };
        let base_level = list.levels.get(level_index).cloned();
        let Some(base_level) = base_level else {
            return Ok(None);
        };
        let level = list_override
            .level_overrides
            .iter()
            .find(|level_override| level_override.level_index == level_index)
            .filter(|level_override| level_override.format_override_enabled)
            .and_then(|level_override| level_override.level_definition.clone())
            .unwrap_or(base_level);
        self.apply_list_level_layout(&level, offset)?;
        let follow = list_level_follow_text(level.follow);
        let character_style = level
            .has_character_style
            .then_some(level.character_style.clone());

        let marker = match level.format {
            ListNumberFormat::Bullet => {
                let bullet = if level.text_template.is_empty() {
                    "\u{2022}".to_string()
                } else {
                    level.text_template.replace('\0', "")
                };
                PendingListMarker {
                    text: format!("{bullet}{follow}"),
                    character_style,
                    runs: Vec::new(),
                }
            }
            ListNumberFormat::Decimal
            | ListNumberFormat::UpperRoman
            | ListNumberFormat::LowerRoman
            | ListNumberFormat::UpperLetter
            | ListNumberFormat::LowerLetter
            | ListNumberFormat::Ordinal
            | ListNumberFormat::DecimalLeadingZero(_)
            | ListNumberFormat::Other => {
                let start_at = list_level_start_at(&list_override, &level, level_index);
                let value =
                    self.next_list_counter_value(override_index, level_index, start_at, &list);
                let marker = format_list_counter(value, level.format);
                if has_list_level_placeholders(&level.text_template) {
                    PendingListMarker {
                        text: format!(
                            "{}{}",
                            self.render_list_level_template(
                                &level.text_template,
                                &list,
                                &list_override,
                                override_index,
                                level_index,
                                &marker,
                                level.legal_numbering,
                            ),
                            follow
                        ),
                        character_style,
                        runs: Vec::new(),
                    }
                } else {
                    PendingListMarker {
                        text: format!("{marker}.{follow}"),
                        character_style,
                        runs: Vec::new(),
                    }
                }
            }
        };

        Ok(Some(marker))
    }

    fn apply_list_level_layout(
        &mut self,
        level: &ListLevelDefinition,
        offset: usize,
    ) -> Result<(), ParseError> {
        let default = ParagraphStyle::default();
        if let Some(indent_twips) = level.indent_twips
            && self.state.paragraph.left_indent_twips == default.left_indent_twips
        {
            self.state.paragraph.left_indent_twips = indent_twips;
        }
        if let Some(space_twips) = level.space_twips {
            self.insert_paragraph_tab_stop(
                Some(space_twips),
                "list level spacing",
                offset,
                TabLeader::None,
                TabAlignment::Left,
            )?;
        }
        Ok(())
    }

    fn render_list_level_template(
        &self,
        current_template: &str,
        list: &ListDefinition,
        list_override: &ListOverride,
        override_index: i32,
        current_level_index: usize,
        current_marker: &str,
        legal_numbering: bool,
    ) -> String {
        let mut output = String::new();
        for ch in current_template.chars() {
            if let Some(template_level_index) = list_level_placeholder_index(ch) {
                if template_level_index == current_level_index {
                    output.push_str(current_marker);
                    continue;
                }
                let Some(template_level) = list.levels.get(template_level_index) else {
                    output.push_str(current_marker);
                    continue;
                };
                let value = self
                    .list_counters
                    .iter()
                    .find(|counter| {
                        counter.override_index == override_index
                            && counter.level_index == template_level_index
                    })
                    .map(|counter| counter.value)
                    .unwrap_or_else(|| {
                        list_level_start_at(list_override, template_level, template_level_index)
                    });
                let format = if legal_numbering && template_level_index < current_level_index {
                    ListNumberFormat::Decimal
                } else {
                    template_level.format
                };
                output.push_str(&format_list_counter(value, format));
            } else {
                output.push(ch);
            }
        }
        output
    }

    fn next_list_counter_value(
        &mut self,
        override_index: i32,
        level_index: usize,
        start_at: i32,
        list: &ListDefinition,
    ) -> i32 {
        self.list_counters.retain(|counter| {
            counter.override_index != override_index
                || counter.level_index <= level_index
                || list
                    .levels
                    .get(counter.level_index)
                    .is_some_and(|level| level.no_restart)
        });

        if let Some(counter) = self.list_counters.iter_mut().find(|counter| {
            counter.override_index == override_index && counter.level_index == level_index
        }) {
            counter.value += 1;
            counter.value
        } else {
            let value = start_at.max(0);
            self.list_counters.push(ListCounter {
                override_index,
                level_index,
                value,
            });
            value
        }
    }

    fn start_old_style_list_marker(&mut self) {
        self.pending_old_style_list_marker = Some(OldStyleListMarker::default());
    }

    fn set_old_style_list_marker_format(&mut self, format: ListNumberFormat) {
        let marker = self
            .pending_old_style_list_marker
            .get_or_insert_with(OldStyleListMarker::default);
        marker.format = format;
    }

    fn set_old_style_list_marker_start(&mut self, start_at: i32) {
        let marker = self
            .pending_old_style_list_marker
            .get_or_insert_with(OldStyleListMarker::default);
        marker.start_at = start_at.max(0);
    }

    fn update_old_style_list_marker_character_style(
        &mut self,
        update: impl FnOnce(&mut CharacterStyle),
    ) {
        let marker = self
            .pending_old_style_list_marker
            .get_or_insert_with(OldStyleListMarker::default);
        update(&mut marker.character_style);
        marker.has_character_style = true;
    }

    fn set_old_style_list_marker_bold(&mut self, enabled: bool) {
        self.update_old_style_list_marker_character_style(|style| style.bold = enabled);
    }

    fn set_old_style_list_marker_italic(&mut self, enabled: bool) {
        self.update_old_style_list_marker_character_style(|style| style.italic = enabled);
    }

    fn set_old_style_list_marker_underline(&mut self, enabled: bool) {
        self.update_old_style_list_marker_character_style(|style| {
            style.underline = if enabled {
                UnderlineStyle::Single
            } else {
                UnderlineStyle::None
            };
        });
    }

    fn set_old_style_list_marker_strike(&mut self, enabled: bool) {
        self.update_old_style_list_marker_character_style(|style| {
            style.strike = enabled;
            style.double_strike = false;
        });
    }

    fn set_old_style_list_marker_caps(&mut self, enabled: bool) {
        self.update_old_style_list_marker_character_style(|style| style.all_caps = enabled);
    }

    fn set_old_style_list_marker_color(&mut self, color_index: i32) {
        self.update_old_style_list_marker_character_style(|style| {
            style.color_index = color_index.max(0) as usize
        });
    }

    fn set_old_style_list_marker_font(&mut self, font_index: i32) {
        self.update_old_style_list_marker_character_style(|style| {
            style.font_index = font_index.max(0)
        });
    }

    fn set_old_style_list_marker_font_size(&mut self, font_size_half_points: i32, offset: usize) {
        let font_size_half_points = self.clamp_font_size(font_size_half_points, offset);
        self.update_old_style_list_marker_character_style(|style| {
            style.font_size_half_points = font_size_half_points
        });
    }

    fn set_old_style_list_marker_indent(&mut self, indent_twips: Option<i32>, offset: usize) {
        let indent_twips =
            self.clamp_paragraph_indent(indent_twips, "old-style list indent", offset);
        let marker = self
            .pending_old_style_list_marker
            .get_or_insert_with(OldStyleListMarker::default);
        marker.indent_twips = Some(indent_twips.max(0));
    }

    fn set_old_style_list_marker_hanging(&mut self, hanging: bool) {
        let marker = self
            .pending_old_style_list_marker
            .get_or_insert_with(OldStyleListMarker::default);
        marker.hanging = hanging;
    }

    fn set_old_style_list_marker_spacing(&mut self, space_twips: Option<i32>, offset: usize) {
        let space_twips = self.clamp_old_style_list_marker_spacing(space_twips, offset);
        let marker = self
            .pending_old_style_list_marker
            .get_or_insert_with(OldStyleListMarker::default);
        marker.space_twips = Some(space_twips);
    }

    fn clamp_old_style_list_marker_spacing(
        &mut self,
        space_twips: Option<i32>,
        offset: usize,
    ) -> i32 {
        let value = space_twips.unwrap_or(0).max(0);
        let max = self.limits().max_tab_stop_twips.max(0);
        let clamped = value.min(max);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("old-style list marker spacing clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        clamped
    }

    fn take_old_style_list_marker(
        &mut self,
        offset: usize,
    ) -> Result<Option<PendingListMarker>, ParseError> {
        let Some(marker) = self.pending_old_style_list_marker.take() else {
            return Ok(None);
        };
        self.apply_old_style_list_marker_layout(&marker, offset)?;
        let text = match marker.format {
            ListNumberFormat::Bullet => "\u{2022}\t".to_string(),
            _ => format!("{}.\t", format_list_counter(marker.start_at, marker.format)),
        };
        let character_style = marker
            .has_character_style
            .then_some(marker.character_style.clone());
        Ok(Some(PendingListMarker {
            text,
            character_style,
            runs: Vec::new(),
        }))
    }

    fn apply_old_style_list_marker_layout(
        &mut self,
        marker: &OldStyleListMarker,
        offset: usize,
    ) -> Result<(), ParseError> {
        if let Some(indent_twips) = marker.indent_twips {
            self.state.paragraph.left_indent_twips = indent_twips;
            if marker.hanging {
                self.state.paragraph.first_line_indent_twips = -indent_twips.min(360);
            }
        }

        if let Some(space_twips) = marker.space_twips {
            self.insert_paragraph_tab_stop(
                Some(space_twips),
                "old-style list marker spacing",
                offset,
                TabLeader::None,
                TabAlignment::Left,
            )?;
        }

        Ok(())
    }

    fn count_skipped_destination_bytes(
        &mut self,
        len: usize,
        offset: usize,
    ) -> Result<(), ParseError> {
        self.skipped_destination_bytes = self
            .skipped_destination_bytes
            .checked_add(len)
            .ok_or(ParseError::DestinationTooLarge(offset))?;
        if self.skipped_destination_bytes > self.limits().max_destination_bytes {
            return Err(ParseError::DestinationTooLarge(offset));
        }
        Ok(())
    }

    fn start_list_definition(&mut self) {
        self.current_list = Some(ListDefinition {
            list_id: 0,
            levels: Vec::new(),
        });
        self.state.list_context = ListContext::List;
    }

    fn start_list_level(&mut self) {
        self.current_list_level = Some(ListLevelDefinition::default());
        self.state.list_context = ListContext::ListLevel;
    }

    fn start_list_level_text(&mut self) {
        if let Some(level) = self.current_list_level.as_mut() {
            level.text_template.clear();
        }
        self.state.list_context = ListContext::ListLevelText;
    }

    fn set_current_list_id(&mut self, list_id: i32) {
        if let Some(list) = self.current_list.as_mut() {
            list.list_id = list_id;
        }
    }

    fn set_current_list_level_format(&mut self, format: i32) {
        if let Some(level) = self.current_list_level.as_mut() {
            level.format = match format {
                0 => ListNumberFormat::Decimal,
                1 => ListNumberFormat::UpperRoman,
                2 => ListNumberFormat::LowerRoman,
                3 => ListNumberFormat::UpperLetter,
                4 => ListNumberFormat::LowerLetter,
                5 => ListNumberFormat::Ordinal,
                22 => ListNumberFormat::DecimalLeadingZero(2),
                23 => ListNumberFormat::Bullet,
                62 => ListNumberFormat::DecimalLeadingZero(3),
                63 => ListNumberFormat::DecimalLeadingZero(4),
                64 => ListNumberFormat::DecimalLeadingZero(5),
                _ => ListNumberFormat::Other,
            };
        }
    }

    fn set_current_list_level_start(&mut self, start_at: i32) {
        if let Some(level) = self.current_list_level.as_mut() {
            level.start_at = start_at.max(0);
        }
    }

    fn set_current_list_level_indent(&mut self, indent_twips: Option<i32>, offset: usize) {
        let indent_twips = self.clamp_paragraph_indent(indent_twips, "list level indent", offset);
        if let Some(level) = self.current_list_level.as_mut() {
            level.indent_twips = Some(indent_twips.max(0));
        }
    }

    fn set_current_list_level_spacing(&mut self, space_twips: Option<i32>, offset: usize) {
        let space_twips = self.clamp_list_level_spacing(space_twips, offset);
        if let Some(level) = self.current_list_level.as_mut() {
            level.space_twips = Some(space_twips);
        }
    }

    fn set_current_list_level_follow(&mut self, follow: i32) {
        if let Some(level) = self.current_list_level.as_mut() {
            level.follow = match follow {
                1 => ListLevelFollow::Space,
                2 => ListLevelFollow::Nothing,
                _ => ListLevelFollow::Tab,
            };
        }
    }

    fn set_current_list_level_legal_numbering(&mut self, enabled: bool) {
        if let Some(level) = self.current_list_level.as_mut() {
            level.legal_numbering = enabled;
        }
    }

    fn set_current_list_level_no_restart(&mut self, enabled: bool) {
        if let Some(level) = self.current_list_level.as_mut() {
            level.no_restart = enabled;
        }
    }

    fn update_current_list_level_character_style(
        &mut self,
        update: impl FnOnce(&mut CharacterStyle),
    ) {
        if let Some(level) = self.current_list_level.as_mut() {
            update(&mut level.character_style);
            level.has_character_style = true;
        }
    }

    fn set_current_list_level_bold(&mut self, enabled: bool) {
        self.update_current_list_level_character_style(|style| style.bold = enabled);
    }

    fn set_current_list_level_italic(&mut self, enabled: bool) {
        self.update_current_list_level_character_style(|style| style.italic = enabled);
    }

    fn reset_current_list_level_character_style(&mut self) {
        let default_style = self.default_character_style();
        self.update_current_list_level_character_style(|style| {
            *style = default_style;
        });
    }

    fn apply_current_list_level_character_style(&mut self, index: i32, offset: usize) -> bool {
        let mut visited = Vec::new();
        if let Some(style) = self.resolve_style(index, &mut visited) {
            self.update_current_list_level_character_style(|current| {
                *current = inherit_character_style(current, &style.character);
            });
            true
        } else {
            self.diagnostics.push(Diagnostic::warning(
                format!("unknown RTF style index {index}"),
                Some(offset),
            ));
            false
        }
    }

    fn set_current_list_level_underline(&mut self, style: UnderlineStyle, control: &Control) {
        let style = if control.parameter.unwrap_or(1) == 0 {
            UnderlineStyle::None
        } else {
            style
        };
        self.set_current_list_level_underline_style(style);
    }

    fn set_current_list_level_underline_style(&mut self, underline: UnderlineStyle) {
        self.update_current_list_level_character_style(|style| {
            style.underline = underline;
        });
    }

    fn set_current_list_level_strike(&mut self, enabled: bool) {
        self.update_current_list_level_character_style(|style| {
            style.strike = enabled;
            style.double_strike = false;
        });
    }

    fn set_current_list_level_double_strike(&mut self, enabled: bool) {
        self.update_current_list_level_character_style(|style| {
            style.strike = enabled;
            style.double_strike = enabled;
        });
    }

    fn set_current_list_level_outline(&mut self, enabled: bool) {
        self.update_current_list_level_character_style(|style| style.outline = enabled);
    }

    fn set_current_list_level_shadow(&mut self, enabled: bool) {
        self.update_current_list_level_character_style(|style| style.shadow = enabled);
    }

    fn set_current_list_level_relief(&mut self, relief: TextRelief) {
        self.update_current_list_level_character_style(|style| style.relief = relief);
    }

    fn set_current_list_level_caps(&mut self, enabled: bool) {
        self.update_current_list_level_character_style(|style| style.all_caps = enabled);
    }

    fn set_current_list_level_small_caps(&mut self, enabled: bool) {
        self.update_current_list_level_character_style(|style| style.small_caps = enabled);
    }

    fn set_current_list_level_super(&mut self, enabled: bool) {
        if enabled {
            self.update_current_list_level_character_style(|style| {
                style.baseline_shift_half_points = DEFAULT_SUPERSCRIPT_SHIFT_HALF_POINTS;
                style.font_size_scale_percent = DEFAULT_SCRIPT_FONT_SCALE_PERCENT;
            });
        } else {
            self.reset_current_list_level_script_position();
        }
    }

    fn set_current_list_level_sub(&mut self, enabled: bool) {
        if enabled {
            self.update_current_list_level_character_style(|style| {
                style.baseline_shift_half_points = DEFAULT_SUBSCRIPT_SHIFT_HALF_POINTS;
                style.font_size_scale_percent = DEFAULT_SCRIPT_FONT_SCALE_PERCENT;
            });
        } else {
            self.reset_current_list_level_script_position();
        }
    }

    fn reset_current_list_level_script_position(&mut self) {
        self.update_current_list_level_character_style(|style| {
            style.baseline_shift_half_points = 0;
            style.font_size_scale_percent = 100;
        });
    }

    fn set_current_list_level_baseline_shift(&mut self, shift_half_points: i32) {
        self.update_current_list_level_character_style(|style| {
            style.baseline_shift_half_points = shift_half_points;
            style.font_size_scale_percent = 100;
        });
    }

    fn set_current_list_level_character_spacing(&mut self, spacing_twips: i32, offset: usize) {
        let spacing_twips = self.clamp_character_spacing(spacing_twips, offset);
        self.update_current_list_level_character_style(|style| {
            style.character_spacing_twips = spacing_twips
        });
    }

    fn set_current_list_level_character_kerning(
        &mut self,
        threshold_half_points: i32,
        offset: usize,
    ) {
        let threshold_half_points = self.clamp_character_kerning(threshold_half_points, offset);
        self.update_current_list_level_character_style(|style| {
            style.character_kerning_half_points = threshold_half_points
        });
        self.diagnostics.push(Diagnostic::warning(
            "character kerning approximated by passive pair spacing",
            Some(offset),
        ));
    }

    fn set_current_list_level_character_scaling(&mut self, scaling_percent: i32, offset: usize) {
        let scaling_percent = self.clamp_character_scaling(scaling_percent, offset);
        self.update_current_list_level_character_style(|style| {
            style.character_scaling_percent = scaling_percent
        });
    }

    fn set_current_list_level_font(&mut self, font_index: i32) {
        self.update_current_list_level_character_style(|style| {
            style.font_index = font_index.max(0)
        });
    }

    fn set_current_list_level_font_size(&mut self, font_size_half_points: i32, offset: usize) {
        let font_size_half_points = self.clamp_font_size(font_size_half_points, offset);
        self.update_current_list_level_character_style(|style| {
            style.font_size_half_points = font_size_half_points
        });
    }

    fn set_current_list_level_underline_color(&mut self, color_index: i32) {
        self.update_current_list_level_character_style(|style| {
            style.underline_color_index = Some(color_index.max(0) as usize)
        });
    }

    fn set_current_list_level_color(&mut self, color_index: i32) {
        self.update_current_list_level_character_style(|style| {
            style.color_index = color_index.max(0) as usize
        });
    }

    fn set_current_list_level_highlight(&mut self, color_index: i32) {
        self.update_current_list_level_character_style(|style| {
            let color_index = color_index.max(0) as usize;
            style.highlight_index = if color_index == 0 {
                None
            } else {
                Some(color_index)
            };
        });
    }

    fn set_current_list_level_highlight_shading(&mut self, basis_points: i32, offset: usize) {
        let basis_points = self.clamp_character_shading(basis_points, offset);
        self.update_current_list_level_character_style(|style| {
            style.highlight_shading_basis_points = basis_points
        });
    }

    fn set_current_list_level_border_visible(&mut self, visible: bool) {
        self.update_current_list_level_character_style(|style| {
            style.border.visible = visible;
        });
    }

    fn set_current_list_level_border_style(&mut self, border_style: BorderStyle) {
        self.update_current_list_level_character_style(|style| {
            style.border.visible = true;
            style.border.style = border_style;
        });
    }

    fn set_current_list_level_border_width(&mut self, value: Option<i32>, offset: usize) {
        let value = value
            .unwrap_or(TableCellBorder::default().width_twips)
            .max(0);
        let clamped = value.min(self.limits().max_table_border_width_twips);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("table border width clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        self.update_current_list_level_character_style(|style| {
            style.border.width_twips = clamped;
        });
    }

    fn set_current_list_level_border_spacing(&mut self, value: Option<i32>, offset: usize) {
        let value = value.unwrap_or(0).max(0);
        let max = self.limits().max_page_border_spacing_twips.max(0);
        let clamped = value.min(max);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("border spacing clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        self.update_current_list_level_character_style(|style| {
            style.border.spacing_twips = clamped;
        });
    }

    fn set_current_list_level_border_color(&mut self, value: Option<i32>) {
        let color_index = value.unwrap_or(0).max(0) as usize;
        let color_index = if color_index == 0 {
            None
        } else {
            Some(color_index)
        };
        self.update_current_list_level_character_style(|style| {
            style.border.color_index = color_index;
        });
    }

    fn clamp_list_level_spacing(&mut self, space_twips: Option<i32>, offset: usize) -> i32 {
        let value = space_twips.unwrap_or(0).max(0);
        let max = self.limits().max_tab_stop_twips.max(0);
        let clamped = value.min(max);
        if clamped != value {
            self.diagnostics.push(Diagnostic::warning(
                format!("list level spacing clamped from {value} to {clamped} twips"),
                Some(offset),
            ));
        }
        clamped
    }

    fn push_list_level_text(&mut self, text: &str, offset: usize) -> Result<(), ParseError> {
        self.count_skipped_destination_bytes(text.len(), offset)?;
        let max_text_run_len = self.limits().max_text_run_len;
        let Some(level) = self.current_list_level.as_mut() else {
            return Ok(());
        };
        let new_len = level
            .text_template
            .chars()
            .count()
            .checked_add(text.chars().count())
            .ok_or(ParseError::OutputTextTooLarge(offset))?;
        if new_len > max_text_run_len {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "list level text".to_string(),
                offset,
            });
        }
        level.text_template.push_str(text);
        Ok(())
    }

    fn push_list_level_unicode(&mut self, value: i32, offset: usize) -> Result<(), ParseError> {
        if let Some(ch) =
            take_rtf_unicode_char(&mut self.state.pending_unicode_high_surrogate, value)
        {
            self.push_list_level_text(&ch.to_string(), offset)?;
        }
        self.state.skip_bytes = self.state.unicode_skip;
        Ok(())
    }

    fn normalize_current_list_level_text(&mut self) {
        if let Some(level) = self.current_list_level.as_mut() {
            level.text_template = normalize_list_level_template(&level.text_template);
        }
    }

    fn finish_list_level(&mut self, offset: usize) -> Result<(), ParseError> {
        let Some(level) = self.current_list_level.take() else {
            return Ok(());
        };
        if self.state.destination == Destination::ListOverrideTable {
            if let Some(level_override) = self.current_list_override_level.as_mut() {
                level_override.level_definition = Some(level);
            }
            return Ok(());
        }
        let Some(list) = self.current_list.as_mut() else {
            return Ok(());
        };
        if list.levels.len() >= 9 {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "list levels".to_string(),
                offset,
            });
        }
        list.levels.push(level);
        Ok(())
    }

    fn finish_list_definition(&mut self, offset: usize) -> Result<(), ParseError> {
        let Some(list) = self.current_list.take() else {
            return Ok(());
        };
        if let Some(existing) = self
            .list_definitions
            .iter_mut()
            .find(|existing| existing.list_id == list.list_id)
        {
            *existing = list;
            return Ok(());
        }
        if self.list_definitions.len() >= self.limits().max_styles {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "lists".to_string(),
                offset,
            });
        }
        self.list_definitions.push(list);
        Ok(())
    }

    fn start_list_override(&mut self) {
        self.current_list_override = Some(ListOverride {
            list_id: 0,
            override_index: 0,
            level_overrides: Vec::new(),
        });
        self.state.list_context = ListContext::ListOverride;
    }

    fn start_list_override_level(&mut self) {
        let level_index = self
            .current_list_override
            .as_ref()
            .map(|list_override| list_override.level_overrides.len())
            .unwrap_or(0);
        self.current_list_override_level = Some(ListLevelOverride {
            level_index,
            start_at: None,
            restart_enabled: false,
            format_override_enabled: false,
            level_definition: None,
        });
        self.state.list_context = ListContext::ListOverrideLevel;
    }

    fn set_current_list_override_list_id(&mut self, list_id: i32) {
        if let Some(list_override) = self.current_list_override.as_mut() {
            list_override.list_id = list_id;
        }
    }

    fn set_current_list_override_index(&mut self, override_index: i32) {
        if let Some(list_override) = self.current_list_override.as_mut() {
            list_override.override_index = override_index;
        }
    }

    fn set_current_list_override_level_start(&mut self, start_at: i32) {
        if let Some(level_override) = self.current_list_override_level.as_mut() {
            level_override.start_at = Some(start_at.max(0));
        }
    }

    fn enable_current_list_override_level_restart(&mut self, enabled: i32) {
        if let Some(level_override) = self.current_list_override_level.as_mut() {
            level_override.restart_enabled = enabled != 0;
        }
    }

    fn enable_current_list_override_level_format(&mut self, enabled: i32) {
        if let Some(level_override) = self.current_list_override_level.as_mut() {
            level_override.format_override_enabled = enabled != 0;
        }
    }

    fn finish_list_override_level(&mut self, offset: usize) -> Result<(), ParseError> {
        let Some(level_override) = self.current_list_override_level.take() else {
            return Ok(());
        };
        let has_start_override =
            level_override.restart_enabled && level_override.start_at.is_some();
        let has_format_override =
            level_override.format_override_enabled && level_override.level_definition.is_some();
        if !has_start_override && !has_format_override {
            return Ok(());
        }
        let Some(list_override) = self.current_list_override.as_mut() else {
            return Ok(());
        };
        if list_override.level_overrides.len() >= 9 {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "list override levels".to_string(),
                offset,
            });
        }
        list_override.level_overrides.push(level_override);
        Ok(())
    }

    fn finish_list_override(&mut self, offset: usize) -> Result<(), ParseError> {
        let Some(list_override) = self.current_list_override.take() else {
            return Ok(());
        };
        if let Some(existing) = self
            .list_overrides
            .iter_mut()
            .find(|existing| existing.override_index == list_override.override_index)
        {
            *existing = list_override;
            return Ok(());
        }
        if self.list_overrides.len() >= self.limits().max_styles {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "list overrides".to_string(),
                offset,
            });
        }
        self.list_overrides.push(list_override);
        Ok(())
    }

    fn finish_style_definition(&mut self, offset: usize) -> Result<(), ParseError> {
        let Some(index) = self.state.style_index else {
            return Ok(());
        };

        if let Some(style) = self.styles.iter_mut().find(|style| style.index == index) {
            style.based_on = self.state.style_based_on;
            style.next_style = self.state.style_next;
            style.kind = self.state.style_kind;
            style.paragraph = self.state.paragraph.clone();
            style.character = self.state.character.clone();
            return Ok(());
        }

        if self.styles.len() >= self.limits().max_styles {
            return Err(ParseError::ResourceLimitExceeded {
                resource: "styles".to_string(),
                offset,
            });
        }

        self.styles.push(StyleDefinition {
            index,
            based_on: self.state.style_based_on,
            next_style: self.state.style_next,
            kind: self.state.style_kind,
            paragraph: self.state.paragraph.clone(),
            character: self.state.character.clone(),
        });
        Ok(())
    }

    fn apply_paragraph_style(&mut self, index: i32, offset: usize) -> bool {
        let mut visited = Vec::new();
        if let Some(style) = self.resolve_style(index, &mut visited) {
            if style.kind != StyleKind::Paragraph {
                self.diagnostics.push(Diagnostic::warning(
                    format!("RTF character style {index} ignored as paragraph style"),
                    Some(offset),
                ));
                return false;
            }
            self.state.paragraph = style.paragraph;
            self.state.character = style.character;
            self.state.paragraph_style_index = Some(index);
            true
        } else {
            self.diagnostics.push(Diagnostic::warning(
                format!("unknown RTF style index {index}"),
                Some(offset),
            ));
            false
        }
    }

    fn apply_character_style(&mut self, index: i32, offset: usize) -> bool {
        let mut visited = Vec::new();
        if let Some(style) = self.resolve_style(index, &mut visited) {
            self.state.character = inherit_character_style(&self.state.character, &style.character);
            true
        } else {
            self.diagnostics.push(Diagnostic::warning(
                format!("unknown RTF style index {index}"),
                Some(offset),
            ));
            false
        }
    }

    fn apply_next_style_after_paragraph(&mut self, offset: usize) {
        let Some(current_style_index) = self.state.paragraph_style_index else {
            return;
        };
        let mut visited = Vec::new();
        let Some(current_style) = self.resolve_style(current_style_index, &mut visited) else {
            self.state.paragraph_style_index = None;
            return;
        };
        if let Some(next_style_index) = current_style.next_style {
            self.apply_paragraph_style(next_style_index, offset);
        }
    }

    fn resolve_style(&self, index: i32, visited: &mut Vec<i32>) -> Option<StyleDefinition> {
        if visited.contains(&index) {
            return None;
        }
        visited.push(index);
        let style = self
            .styles
            .iter()
            .find(|style| style.index == index)?
            .clone();
        let Some(base_index) = style.based_on else {
            return Some(style);
        };
        if base_index == style.index {
            return Some(style);
        }
        let Some(base) = self.resolve_style(base_index, visited) else {
            return Some(style);
        };
        Some(StyleDefinition {
            index: style.index,
            based_on: style.based_on,
            next_style: style.next_style,
            kind: style.kind,
            paragraph: inherit_paragraph_style(&base.paragraph, &style.paragraph),
            character: inherit_character_style(&base.character, &style.character),
        })
    }

    fn handle_active_content(&mut self, feature: &str, offset: usize) -> Result<(), ParseError> {
        match self.options.active_content_policy {
            ActiveContentPolicy::Reject => Err(ParseError::ActiveContentRejected {
                feature: feature.to_string(),
                offset,
            }),
            ActiveContentPolicy::Strip | ActiveContentPolicy::Placeholder => {
                self.diagnostics.push(Diagnostic::warning(
                    format!("active content removed: {feature}"),
                    Some(offset),
                ));
                Ok(())
            }
        }
    }

    fn handle_dynamic_date_time_control(
        &mut self,
        control_name: &str,
        offset: usize,
    ) -> Result<(), ParseError> {
        match self.options.active_content_policy {
            ActiveContentPolicy::Reject => Err(ParseError::ActiveContentRejected {
                feature: "dynamic date/time control".to_string(),
                offset,
            }),
            ActiveContentPolicy::Strip => {
                self.diagnostics.push(Diagnostic::warning(
                    format!(
                        "dynamic date/time control {control_name} removed without evaluating current time"
                    ),
                    Some(offset),
                ));
                Ok(())
            }
            ActiveContentPolicy::Placeholder => {
                self.diagnostics.push(Diagnostic::warning(
                    format!(
                        "dynamic date/time control {control_name} placeholdered without evaluating current time"
                    ),
                    Some(offset),
                ));
                self.push_text("[Dynamic date/time removed]", offset)
            }
        }
    }
}

fn normalize_shape_points(
    points: &[StaticShapePoint],
    max_dimension_twips: i32,
) -> Option<(i32, i32, i32, i32, Vec<StaticShapePoint>)> {
    if points.len() < 2 {
        return None;
    }
    let min_x = points.iter().map(|point| point.x_twips).min()?.max(0);
    let min_y = points.iter().map(|point| point.y_twips).min()?.max(0);
    let max_x = points
        .iter()
        .map(|point| point.x_twips)
        .max()?
        .max(min_x + 1);
    let max_y = points
        .iter()
        .map(|point| point.y_twips)
        .max()?
        .max(min_y + 1);
    let width_twips = max_x.saturating_sub(min_x).clamp(1, max_dimension_twips);
    let height_twips = max_y.saturating_sub(min_y).clamp(1, max_dimension_twips);
    let normalized = points
        .iter()
        .map(|point| StaticShapePoint {
            x_twips: point.x_twips.saturating_sub(min_x).clamp(0, width_twips),
            y_twips: point.y_twips.saturating_sub(min_y).clamp(0, height_twips),
        })
        .collect::<Vec<_>>();
    Some((min_x, min_y, width_twips, height_twips, normalized))
}

fn paragraph_shading_pattern_control(name: &str) -> Option<ShadingPattern> {
    match name {
        "bghoriz" => Some(ShadingPattern::Horizontal),
        "bgvert" => Some(ShadingPattern::Vertical),
        "bgfdiag" => Some(ShadingPattern::ForwardDiagonal),
        "bgbdiag" => Some(ShadingPattern::BackwardDiagonal),
        "bgcross" => Some(ShadingPattern::Cross),
        "bgdcross" => Some(ShadingPattern::DiagonalCross),
        "bgdkhoriz" => Some(ShadingPattern::DarkHorizontal),
        "bgdkvert" => Some(ShadingPattern::DarkVertical),
        "bgdkfdiag" => Some(ShadingPattern::DarkForwardDiagonal),
        "bgdkbdiag" => Some(ShadingPattern::DarkBackwardDiagonal),
        "bgdkcross" => Some(ShadingPattern::DarkCross),
        "bgdkdcross" => Some(ShadingPattern::DarkDiagonalCross),
        _ => None,
    }
}

fn table_cell_shading_pattern_control(name: &str) -> Option<ShadingPattern> {
    match name {
        "clbghoriz" => Some(ShadingPattern::Horizontal),
        "clbgvert" => Some(ShadingPattern::Vertical),
        "clbgfdiag" => Some(ShadingPattern::ForwardDiagonal),
        "clbgbdiag" => Some(ShadingPattern::BackwardDiagonal),
        "clbgcross" => Some(ShadingPattern::Cross),
        "clbgdcross" => Some(ShadingPattern::DiagonalCross),
        "clbgdkhor" | "clbgdkhoriz" => Some(ShadingPattern::DarkHorizontal),
        "clbgdkvert" => Some(ShadingPattern::DarkVertical),
        "clbgdkfdiag" => Some(ShadingPattern::DarkForwardDiagonal),
        "clbgdkbdiag" => Some(ShadingPattern::DarkBackwardDiagonal),
        "clbgdkcross" => Some(ShadingPattern::DarkCross),
        "clbgdkdcross" => Some(ShadingPattern::DarkDiagonalCross),
        _ => None,
    }
}

fn table_row_shading_pattern_control(name: &str) -> Option<ShadingPattern> {
    match name {
        "trbghoriz" => Some(ShadingPattern::Horizontal),
        "trbgvert" => Some(ShadingPattern::Vertical),
        "trbgfdiag" => Some(ShadingPattern::ForwardDiagonal),
        "trbgbdiag" => Some(ShadingPattern::BackwardDiagonal),
        "trbgcross" => Some(ShadingPattern::Cross),
        "trbgdcross" => Some(ShadingPattern::DiagonalCross),
        "trbgdkhor" | "trbgdkhoriz" => Some(ShadingPattern::DarkHorizontal),
        "trbgdkvert" => Some(ShadingPattern::DarkVertical),
        "trbgdkfdiag" => Some(ShadingPattern::DarkForwardDiagonal),
        "trbgdkbdiag" => Some(ShadingPattern::DarkBackwardDiagonal),
        "trbgdkcross" => Some(ShadingPattern::DarkCross),
        "trbgdkdcross" => Some(ShadingPattern::DarkDiagonalCross),
        _ => None,
    }
}

fn is_known_ignored_control(name: &str) -> bool {
    paragraph_shading_pattern_control(name).is_some()
        || table_cell_shading_pattern_control(name).is_some()
        || table_row_shading_pattern_control(name).is_some()
        || matches!(
            name,
            "ansicpg"
                | "cocoartf"
                | "cocoasubrtf"
                | "cpg"
                | "adjustright"
                | "af"
                | "alang"
                | "dbch"
                | "deflang"
                | "deflangfe"
                | "fbidis"
                | "fromtext"
                | "fcharset"
                | "fbimajor"
                | "fbiminor"
                | "fdbmajor"
                | "fdbminor"
                | "fhimajor"
                | "fhiminor"
                | "flomajor"
                | "flominor"
                | "fprq"
                | "fmodern"
                | "fnil"
                | "froman"
                | "fswiss"
                | "fscript"
                | "fdecor"
                | "ftech"
                | "horzdoc"
                | "itap"
                | "intbl"
                | "listtable"
                | "listoverridetable"
                | "hich"
                | "lang"
                | "langfe"
                | "langfenp"
                | "langnp"
                | "loch"
                | "ltrch"
                | "ltrpar"
                | "nouicompat"
                | "pn"
                | "pnb"
                | "pncaps"
                | "pncard"
                | "pncf"
                | "pndec"
                | "pnf"
                | "pnfs"
                | "pnhang"
                | "pni"
                | "pnindent"
                | "pnlcltr"
                | "pnlcrm"
                | "pnlvlblt"
                | "pnlvlbody"
                | "pnlvlcont"
                | "pnqc"
                | "pnql"
                | "pnqr"
                | "pnrestart"
                | "pnseclvl"
                | "pnsp"
                | "pnstart"
                | "pnstrike"
                | "pnul"
                | "pnucltr"
                | "pnucrm"
                | "charrsid"
                | "delrsid"
                | "insrsid"
                | "pararsid"
                | "revauth"
                | "revdttm"
                | "revised"
                | "rsidroot"
                | "sectrsid"
                | "softline"
                | "softpage"
                | "tsrsid"
                | "trowd"
                | "cellx"
                | "clftsWidth"
                | "clwWidth"
                | "viewscale"
                | "viewzk"
                | "taprtl"
                | "rtlrow"
                | "allprot"
                | "annotprot"
                | "enforceprot"
                | "formprot"
                | "protlevel"
                | "readprot"
                | "readonlyrecommended"
                | "revisions"
                | "revprot"
                | "rtlch"
        )
}

fn is_stylesheet_metadata_control(name: &str) -> bool {
    matches!(
        name,
        "additive"
            | "sadditive"
            | "sautoupd"
            | "slink"
            | "slocked"
            | "slocked0"
            | "spriority"
            | "sqformat"
            | "ssemihidden"
            | "sunhideused"
            | "styrsid"
    )
}

fn is_office_math_control(name: &str) -> bool {
    matches!(
        name,
        "mmath"
            | "moMath"
            | "moMathPara"
            | "mtext"
            | "mr"
            | "me"
            | "marg"
            | "macc"
            | "maccPr"
            | "mbar"
            | "mbarPr"
            | "mborderBox"
            | "mborderBoxPr"
            | "mbox"
            | "mboxPr"
            | "mchr"
            | "md"
            | "mdegHide"
            | "mden"
            | "mdiff"
            | "meqArr"
            | "meqArrPr"
            | "mf"
            | "mfPr"
            | "mfunc"
            | "mfName"
            | "mfuncPr"
            | "mgroupChr"
            | "mgroupChrPr"
            | "mgrow"
            | "mlimLow"
            | "mlimLowPr"
            | "mlimLoc"
            | "mlimUpp"
            | "mlimUppPr"
            | "mmatrix"
            | "mmatrixPr"
            | "mnary"
            | "mnaryPr"
            | "mnum"
            | "mobjDist"
            | "mopEmu"
            | "mphant"
            | "mphantPr"
            | "mrad"
            | "mradPr"
            | "msepChr"
            | "msPre"
            | "msPrePr"
            | "msSub"
            | "msSubPr"
            | "msSubSup"
            | "msSubSupPr"
            | "msSup"
            | "msSupPr"
            | "msub"
            | "msubHide"
            | "msubsup"
            | "msup"
            | "msupHide"
            | "mctrlPr"
            | "mdeg"
            | "mdPr"
            | "mtype"
            | "msty"
    )
}

fn is_nested_table_structural_control(name: &str) -> bool {
    paragraph_shading_pattern_control(name).is_some()
        || table_cell_shading_pattern_control(name).is_some()
        || table_row_shading_pattern_control(name).is_some()
        || matches!(
            name,
            "nesttableprops"
                | "nonesttables"
                | "intbl"
                | "cellx"
                | "clcbpat"
                | "clcfpat"
                | "clshdng"
                | "clbghoriz"
                | "clbgvert"
                | "clpadl"
                | "clpadr"
                | "clpadt"
                | "clpadb"
                | "clftsWidth"
                | "clwWidth"
                | "clNoWrap"
                | "clnowrap"
                | "cltxlrtb"
                | "cltxtbrlv"
                | "cltxlrtbv"
                | "cltxbtlr"
                | "clbrdrl"
                | "clbrdrr"
                | "clbrdrt"
                | "clbrdrb"
                | "clvertalt"
                | "clvertalc"
                | "clvertalb"
                | "clmgf"
                | "clmrg"
                | "clvmgf"
                | "clvmrg"
                | "trrh"
                | "trleft"
                | "trgaph"
                | "trql"
                | "trqc"
                | "trqr"
                | "taprtl"
                | "rtlrow"
                | "trhdr"
                | "trkeep"
                | "trpaddl"
                | "trpaddr"
                | "trpaddt"
                | "trpaddb"
                | "trcbpat"
                | "trcfpat"
                | "trshdng"
                | "trbghoriz"
                | "trbgvert"
                | "trbrdrl"
                | "trbrdrr"
                | "trbrdrt"
                | "trbrdrb"
                | "brdrnone"
                | "brdrnil"
                | "brdrs"
                | "brdrth"
                | "brdrhair"
                | "brdrdb"
                | "brdrdot"
                | "brdrdash"
                | "brdrdashsm"
                | "brdrdashd"
                | "brdrdashdd"
                | "brdrdashdot"
                | "brdrdashdotstr"
                | "brdrdashdotdot"
                | "brdrwavy"
                | "brdrwavydb"
                | "brdrw"
                | "brsp"
                | "brdrcf"
                | "bghoriz"
                | "bgvert"
        )
}

fn inherit_paragraph_style(base: &ParagraphStyle, derived: &ParagraphStyle) -> ParagraphStyle {
    let default = ParagraphStyle::default();
    let mut output = derived.clone();
    if output.alignment == default.alignment {
        output.alignment = base.alignment;
    }
    if output.page_break_before == default.page_break_before {
        output.page_break_before = base.page_break_before;
    }
    if output.keep_together == default.keep_together {
        output.keep_together = base.keep_together;
    }
    if output.keep_with_next == default.keep_with_next {
        output.keep_with_next = base.keep_with_next;
    }
    if output.widow_control == default.widow_control {
        output.widow_control = base.widow_control;
    }
    if output.no_wrap == default.no_wrap {
        output.no_wrap = base.no_wrap;
    }
    if output.auto_hyphenation == default.auto_hyphenation {
        output.auto_hyphenation = base.auto_hyphenation;
    }
    if output.hyphenate_caps == default.hyphenate_caps {
        output.hyphenate_caps = base.hyphenate_caps;
    }
    if output.max_consecutive_hyphenated_lines == default.max_consecutive_hyphenated_lines {
        output.max_consecutive_hyphenated_lines = base.max_consecutive_hyphenated_lines;
    }
    if output.hyphenation_zone_twips == default.hyphenation_zone_twips {
        output.hyphenation_zone_twips = base.hyphenation_zone_twips;
    }
    if output.drop_cap_lines == default.drop_cap_lines {
        output.drop_cap_lines = base.drop_cap_lines;
    }
    if output.left_indent_twips == default.left_indent_twips {
        output.left_indent_twips = base.left_indent_twips;
    }
    if output.right_indent_twips == default.right_indent_twips {
        output.right_indent_twips = base.right_indent_twips;
    }
    if output.first_line_indent_twips == default.first_line_indent_twips {
        output.first_line_indent_twips = base.first_line_indent_twips;
    }
    if output.space_before_twips == default.space_before_twips {
        output.space_before_twips = base.space_before_twips;
    }
    if output.space_after_twips == default.space_after_twips {
        output.space_after_twips = base.space_after_twips;
    }
    if output.auto_space_before == default.auto_space_before {
        output.auto_space_before = base.auto_space_before;
    }
    if output.auto_space_after == default.auto_space_after {
        output.auto_space_after = base.auto_space_after;
    }
    if output.contextual_spacing == default.contextual_spacing {
        output.contextual_spacing = base.contextual_spacing;
    }
    if output.line_spacing_twips == default.line_spacing_twips {
        output.line_spacing_twips = base.line_spacing_twips;
    }
    if output.line_spacing_multiple == default.line_spacing_multiple {
        output.line_spacing_multiple = base.line_spacing_multiple;
    }
    if output.shading_color_index == default.shading_color_index {
        output.shading_color_index = base.shading_color_index;
    }
    if output.shading_basis_points == default.shading_basis_points {
        output.shading_basis_points = base.shading_basis_points;
    }
    if output.shading_pattern == default.shading_pattern {
        output.shading_pattern = base.shading_pattern;
    }
    if output.tab_stops_twips == default.tab_stops_twips {
        output.tab_stops_twips = base.tab_stops_twips.clone();
        output.tab_stop_leaders = base.tab_stop_leaders.clone();
        output.tab_stop_alignments = base.tab_stop_alignments.clone();
    }
    if output.borders == default.borders {
        output.borders = base.borders;
    }
    output
}

fn inherit_character_style(base: &CharacterStyle, derived: &CharacterStyle) -> CharacterStyle {
    let default = CharacterStyle::default();
    let mut output = derived.clone();
    if output.bold == default.bold {
        output.bold = base.bold;
    }
    if output.italic == default.italic {
        output.italic = base.italic;
    }
    if output.underline == default.underline {
        output.underline = base.underline;
    }
    if output.underline_color_index == default.underline_color_index {
        output.underline_color_index = base.underline_color_index;
    }
    if output.strike == default.strike {
        output.strike = base.strike;
    }
    if output.double_strike == default.double_strike {
        output.double_strike = base.double_strike;
    }
    if output.outline == default.outline {
        output.outline = base.outline;
    }
    if output.shadow == default.shadow {
        output.shadow = base.shadow;
    }
    if output.relief == default.relief {
        output.relief = base.relief;
    }
    if output.all_caps == default.all_caps {
        output.all_caps = base.all_caps;
    }
    if output.small_caps == default.small_caps {
        output.small_caps = base.small_caps;
    }
    if output.hidden == default.hidden {
        output.hidden = base.hidden;
    }
    if output.form_field_shading == default.form_field_shading {
        output.form_field_shading = base.form_field_shading;
    }
    if output.baseline_shift_half_points == default.baseline_shift_half_points {
        output.baseline_shift_half_points = base.baseline_shift_half_points;
    }
    if output.font_size_scale_percent == default.font_size_scale_percent {
        output.font_size_scale_percent = base.font_size_scale_percent;
    }
    if output.character_spacing_twips == default.character_spacing_twips {
        output.character_spacing_twips = base.character_spacing_twips;
    }
    if output.character_kerning_half_points == default.character_kerning_half_points {
        output.character_kerning_half_points = base.character_kerning_half_points;
    }
    if output.character_scaling_percent == default.character_scaling_percent {
        output.character_scaling_percent = base.character_scaling_percent;
    }
    if output.font_index == default.font_index {
        output.font_index = base.font_index;
    }
    if output.font_size_half_points == default.font_size_half_points {
        output.font_size_half_points = base.font_size_half_points;
    }
    if output.color_index == default.color_index {
        output.color_index = base.color_index;
    }
    if output.highlight_index == default.highlight_index {
        output.highlight_index = base.highlight_index;
    }
    if output.highlight_shading_basis_points == default.highlight_shading_basis_points {
        output.highlight_shading_basis_points = base.highlight_shading_basis_points;
    }
    if output.border == default.border {
        output.border = base.border;
    }
    output
}

fn skipped_destination_active_feature(name: &str) -> Option<&'static str> {
    match name {
        "object" | "objdata" | "objocx" | "objemb" | "objlink" | "objautlink" | "objupdate" => {
            Some("object payload in skipped destination")
        }
        "field" | "fldinst" => Some("field instruction in skipped destination"),
        "fontemb" | "fontfile" => Some("embedded font payload in skipped destination"),
        _ => None,
    }
}

fn is_metadata_destination(name: &str) -> bool {
    matches!(
        name,
        "info"
            | "title"
            | "subject"
            | "author"
            | "keywords"
            | "doccomm"
            | "comment"
            | "comments"
            | "operator"
            | "manager"
            | "company"
            | "category"
            | "hlinkbase"
            | "creatim"
            | "revtim"
            | "printim"
            | "buptim"
            | "version"
            | "nofpages"
            | "nofwords"
            | "nofchars"
            | "nofcharsws"
            | "edmins"
            | "vern"
            | "ftnsep"
            | "ftnsepc"
            | "ftncn"
            | "aftnsep"
            | "aftnsepc"
            | "aftncn"
            | "userprops"
            | "revtbl"
            | "rsidtbl"
            | "xmlnstbl"
            | "themedata"
            | "colorschememapping"
            | "datastore"
            | "datafield"
            | "formfield"
            | "mailmerge"
            | "mmconnectstr"
            | "mmdatasource"
            | "mmheadersource"
            | "mmquery"
            | "mmlinktoquery"
            | "mmodso"
            | "mmodsocolumn"
            | "mmodsofilter"
            | "mmodsofldmpdata"
            | "mmodsofhdr"
            | "mmodsorecipientdata"
            | "mmodsosort"
            | "mmodsosrc"
            | "mmodsotable"
            | "mmodsoudl"
            | "mmodsoudldata"
            | "fontemb"
            | "fontfile"
            | "annotation"
            | "atnid"
            | "atnauthor"
            | "atntime"
            | "atndate"
            | "atnref"
            | "atnparent"
            | "atnicn"
            | "atrfstart"
            | "atrfend"
            | "bkmkstart"
            | "bkmkend"
            | "deleted"
            | "deletedtext"
    )
}

fn document_property_control(name: &str) -> Option<DocumentProperty> {
    match name {
        "title" => Some(DocumentProperty::Title),
        "subject" => Some(DocumentProperty::Subject),
        "author" => Some(DocumentProperty::Author),
        "keywords" => Some(DocumentProperty::Keywords),
        "doccomm" | "comment" | "comments" => Some(DocumentProperty::Comments),
        "operator" => Some(DocumentProperty::Operator),
        "manager" => Some(DocumentProperty::Manager),
        "company" => Some(DocumentProperty::Company),
        "category" => Some(DocumentProperty::Category),
        _ => None,
    }
}

fn document_timestamp_control(name: &str) -> Option<DocumentTimestampKind> {
    match name {
        "creatim" => Some(DocumentTimestampKind::Created),
        "revtim" => Some(DocumentTimestampKind::Saved),
        "printim" => Some(DocumentTimestampKind::Printed),
        _ => None,
    }
}

fn is_mail_merge_destination(name: &str) -> bool {
    matches!(
        name,
        "mailmerge"
            | "mmconnectstr"
            | "mmdatasource"
            | "mmheadersource"
            | "mmquery"
            | "mmlinktoquery"
            | "mmodso"
            | "mmodsocolumn"
            | "mmodsofilter"
            | "mmodsofldmpdata"
            | "mmodsofhdr"
            | "mmodsorecipientdata"
            | "mmodsosort"
            | "mmodsosrc"
            | "mmodsotable"
            | "mmodsoudl"
            | "mmodsoudldata"
    )
}

fn is_annotation_destination(name: &str) -> bool {
    matches!(
        name,
        "annotation"
            | "atnid"
            | "atnauthor"
            | "atntime"
            | "atndate"
            | "atnref"
            | "atnparent"
            | "atnicn"
            | "atrfstart"
            | "atrfend"
    )
}

fn is_object_metadata_destination(name: &str) -> bool {
    matches!(
        name,
        "objclass"
            | "objname"
            | "objalias"
            | "objsect"
            | "objitem"
            | "objtopic"
            | "objsrv"
            | "objtime"
    )
}

fn destination_allows_visible_content(state: &ParserState) -> bool {
    !state.inside_metadata && state.destination != Destination::Ignored
}

fn destination_allows_safe_structural_content(state: &ParserState) -> bool {
    !state.inside_metadata
}

fn is_header_destination(destination: Destination) -> bool {
    matches!(
        destination,
        Destination::Header | Destination::FirstPageHeader | Destination::EvenPageHeader
    )
}

fn is_footer_destination(destination: Destination) -> bool {
    matches!(
        destination,
        Destination::Footer | Destination::FirstPageFooter | Destination::EvenPageFooter
    )
}

fn append_field_instruction(
    target: &mut String,
    text: &str,
    limit: usize,
    offset: usize,
) -> Result<(), ParseError> {
    let new_len = target
        .chars()
        .count()
        .checked_add(text.chars().count())
        .ok_or(ParseError::OutputTextTooLarge(offset))?;
    if new_len > limit {
        return Err(ParseError::ResourceLimitExceeded {
            resource: "field instruction".to_string(),
            offset,
        });
    }
    target.push_str(text);
    Ok(())
}

fn merge_child_field_instruction(
    target: &mut String,
    child: &str,
    limit: usize,
    offset: usize,
) -> Result<(), ParseError> {
    let addition = child.strip_prefix(target.as_str()).unwrap_or(child);
    append_field_instruction(target, addition, limit, offset)
}

fn merge_child_form_default_text(
    target: &mut String,
    child: &str,
    limit: usize,
    offset: usize,
) -> Result<(), ParseError> {
    let addition = child.strip_prefix(target.as_str()).unwrap_or(child);
    let new_len = target
        .chars()
        .count()
        .checked_add(addition.chars().count())
        .ok_or(ParseError::OutputTextTooLarge(offset))?;
    if new_len > limit {
        return Err(ParseError::ResourceLimitExceeded {
            resource: "form default text".to_string(),
            offset,
        });
    }
    target.push_str(addition);
    Ok(())
}

fn merge_child_form_dropdown_entry_text(
    target: &mut String,
    child: &str,
    limit: usize,
    offset: usize,
) -> Result<(), ParseError> {
    let addition = child.strip_prefix(target.as_str()).unwrap_or(child);
    let new_len = target
        .chars()
        .count()
        .checked_add(addition.chars().count())
        .ok_or(ParseError::OutputTextTooLarge(offset))?;
    if new_len > limit {
        return Err(ParseError::ResourceLimitExceeded {
            resource: "form dropdown entry text".to_string(),
            offset,
        });
    }
    target.push_str(addition);
    Ok(())
}

fn push_form_dropdown_entry(
    entries: &mut Vec<String>,
    entry: &str,
    limit: usize,
    offset: usize,
) -> Result<(), ParseError> {
    let entry = entry.trim_end();
    if entry.is_empty() || entry.chars().any(|ch| ch.is_control()) {
        return Ok(());
    }
    if entries.len() >= limit {
        return Err(ParseError::ResourceLimitExceeded {
            resource: "form dropdown entries".to_string(),
            offset,
        });
    }
    entries.push(entry.to_string());
    Ok(())
}

fn merge_child_form_dropdown_entries(
    target: &mut Vec<String>,
    child: &[String],
    limit: usize,
    offset: usize,
) -> Result<(), ParseError> {
    let additions = if child.len() >= target.len() && child.starts_with(target.as_slice()) {
        &child[target.len()..]
    } else {
        child
    };
    for entry in additions {
        push_form_dropdown_entry(target, entry, limit, offset)?;
    }
    Ok(())
}

fn normalize_table_cell_text_direction(
    paragraphs: &mut [Paragraph],
    direction: TableCellTextDirection,
) {
    if direction == TableCellTextDirection::LeftToRightTopToBottom {
        return;
    }

    let reverse = direction == TableCellTextDirection::BottomToTopLeftToRight;
    for paragraph in paragraphs {
        let source_runs = std::mem::take(&mut paragraph.runs);
        let mut output_runs = Vec::with_capacity(source_runs.len());
        let mut needs_separator = false;
        let iterator: Box<dyn Iterator<Item = Run>> = if reverse {
            Box::new(source_runs.into_iter().rev())
        } else {
            Box::new(source_runs.into_iter())
        };

        for run in iterator {
            if run.style.hidden || run.text.is_empty() {
                output_runs.push(run);
                continue;
            }

            let chars = if reverse {
                run.text.chars().rev().collect::<Vec<_>>()
            } else {
                run.text.chars().collect::<Vec<_>>()
            };
            let mut text = String::with_capacity(run.text.len() + chars.len());
            for ch in chars {
                if needs_separator {
                    text.push('\n');
                }
                text.push(ch);
                needs_separator = true;
            }
            output_runs.push(Run {
                text,
                style: run.style,
            });
        }

        paragraph.runs = output_runs;
    }
}

struct PassiveFieldResult {
    text: String,
    font_name: Option<String>,
    form_field: bool,
}

struct FieldSequenceInstruction {
    name: String,
    repeat_current: bool,
    hidden: bool,
    reset_value: Option<i32>,
}

struct FieldListNumberInstruction {
    name: String,
    level: i32,
    reset_value: Option<i32>,
}

struct FormulaParser<'a> {
    input: &'a str,
    pos: usize,
}

const MAX_PASSIVE_FIELD_FORMAT_TEXT_CHARS: usize = 4096;
const MAX_PASSIVE_FIELD_NUMERIC_PICTURE_CHARS: usize = 64;
const MAX_PASSIVE_FIELD_DATE_PICTURE_CHARS: usize = 96;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum FieldTextFormatSwitch {
    Upper,
    Lower,
    FirstCap,
    Caps,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum FieldNumberFormatSwitch {
    Arabic,
    UpperAlphabetic,
    LowerAlphabetic,
    UpperRoman,
    LowerRoman,
    Ordinal,
    Hex,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum FieldFormatSwitch {
    Text(FieldTextFormatSwitch),
    Number(FieldNumberFormatSwitch),
}

impl PassiveFieldResult {
    fn text(text: &'static str) -> Self {
        Self {
            text: text.to_string(),
            font_name: None,
            form_field: false,
        }
    }
}

fn apply_field_format_switches(
    instruction: &str,
    mut result: PassiveFieldResult,
) -> Option<PassiveFieldResult> {
    if result.font_name.is_some() || contains_internal_marker(&result.text) {
        return Some(result);
    }

    if let Some(picture) = field_numeric_picture_switch(instruction)? {
        result.text = apply_field_numeric_picture_switch(&result.text, &picture)?;
        if result.text.chars().count() > MAX_PASSIVE_FIELD_FORMAT_TEXT_CHARS {
            return None;
        }
    }

    let switches = field_format_switches(instruction)?;
    if switches.is_empty() {
        return Some(result);
    }
    if result.text.chars().count() > MAX_PASSIVE_FIELD_FORMAT_TEXT_CHARS {
        return None;
    }

    for switch in switches {
        result.text = match switch {
            FieldFormatSwitch::Text(switch) => apply_field_text_format_switch(&result.text, switch),
            FieldFormatSwitch::Number(switch) => {
                apply_field_number_format_switch(&result.text, switch)?
            }
        };
        if result.text.chars().count() > MAX_PASSIVE_FIELD_FORMAT_TEXT_CHARS {
            return None;
        }
    }

    if result.text.chars().any(|ch| ch.is_control()) || contains_internal_marker(&result.text) {
        return None;
    }

    Some(result)
}

fn passive_field_result(
    instruction: &str,
    form_checkbox_checked: Option<bool>,
    form_default_text: &str,
    form_dropdown_entries: &[String],
    form_dropdown_selected_index: Option<i32>,
) -> Option<PassiveFieldResult> {
    match field_instruction_name(instruction)? {
        "PAGE" => Some(PassiveFieldResult::text(PAGE_NUMBER_MARKER)),
        "NUMPAGES" => Some(PassiveFieldResult::text(TOTAL_PAGES_MARKER)),
        "NUMWORDS" => Some(PassiveFieldResult::text(DOCUMENT_WORDS_MARKER)),
        "NUMCHARS" => Some(PassiveFieldResult::text(DOCUMENT_CHARS_MARKER)),
        "NUMCHARSWS" => Some(PassiveFieldResult::text(DOCUMENT_CHARS_WITH_SPACES_MARKER)),
        "SECTION" => Some(PassiveFieldResult::text(SECTION_NUMBER_MARKER)),
        "SECTIONPAGES" => Some(PassiveFieldResult::text(SECTION_PAGES_MARKER)),
        "FORMTEXT" => passive_form_text_field_result(form_default_text),
        "FORMDROPDOWN" => passive_form_dropdown_field_result(
            form_dropdown_entries,
            form_dropdown_selected_index.unwrap_or(0),
        ),
        "FORMCHECKBOX" => Some(PassiveFieldResult {
            text: if form_checkbox_checked.unwrap_or(false) {
                "\u{2611}"
            } else {
                "\u{2610}"
            }
            .to_string(),
            font_name: Some("ZapfDingbats".to_string()),
            form_field: true,
        }),
        "FORMULA" => passive_formula_field_result(instruction),
        "IF" => passive_if_field_result(instruction),
        "MACROBUTTON" => passive_macrobutton_field_result(instruction),
        "MERGEFIELD" => passive_mergefield_result(instruction),
        "QUOTE" => passive_quote_field_result(instruction),
        "SYMBOL" => passive_symbol_field_result(instruction),
        _ => None,
    }
}

fn field_instruction_name(instruction: &str) -> Option<&'static str> {
    if instruction.trim_start().starts_with('=') {
        return Some("FORMULA");
    }
    let name = instruction
        .trim_start()
        .chars()
        .take_while(|ch| ch.is_ascii_alphabetic())
        .collect::<String>()
        .to_ascii_uppercase();
    match name.as_str() {
        "AUTHOR" => Some("AUTHOR"),
        "AUTONUM" => Some("AUTONUM"),
        "AUTONUMLGL" => Some("AUTONUMLGL"),
        "AUTONUMOUT" => Some("AUTONUMOUT"),
        "COMMENTS" => Some("COMMENTS"),
        "PAGE" => Some("PAGE"),
        "NUMPAGES" => Some("NUMPAGES"),
        "NUMWORDS" => Some("NUMWORDS"),
        "NUMCHARS" => Some("NUMCHARS"),
        "NUMCHARSWS" => Some("NUMCHARSWS"),
        "PAGEREF" => Some("PAGEREF"),
        "QUOTE" => Some("QUOTE"),
        "REF" => Some("REF"),
        "SECTION" => Some("SECTION"),
        "SECTIONPAGES" => Some("SECTIONPAGES"),
        "SEQ" => Some("SEQ"),
        "TA" => Some("TA"),
        "TC" => Some("TC"),
        "CREATEDATE" => Some("CREATEDATE"),
        "DATE" => Some("DATE"),
        "DOCPROPERTY" => Some("DOCPROPERTY"),
        "EDITTIME" => Some("EDITTIME"),
        "FORMDROPDOWN" => Some("FORMDROPDOWN"),
        "FORMTEXT" => Some("FORMTEXT"),
        "FORMCHECKBOX" => Some("FORMCHECKBOX"),
        "DATABASE" => Some("DATABASE"),
        "DDE" => Some("DDE"),
        "DDEAUTO" => Some("DDEAUTO"),
        "HYPERLINK" => Some("HYPERLINK"),
        "IF" => Some("IF"),
        "IMPORT" => Some("IMPORT"),
        "INCLUDEPICTURE" => Some("INCLUDEPICTURE"),
        "INCLUDETEXT" => Some("INCLUDETEXT"),
        "INFO" => Some("INFO"),
        "KEYWORDS" => Some("KEYWORDS"),
        "LASTSAVEDBY" => Some("LASTSAVEDBY"),
        "LINK" => Some("LINK"),
        "LISTNUM" => Some("LISTNUM"),
        "MACROBUTTON" => Some("MACROBUTTON"),
        "MERGEFIELD" => Some("MERGEFIELD"),
        "PRINTDATE" => Some("PRINTDATE"),
        "SAVEDATE" => Some("SAVEDATE"),
        "SUBJECT" => Some("SUBJECT"),
        "SYMBOL" => Some("SYMBOL"),
        "TIME" => Some("TIME"),
        "TITLE" => Some("TITLE"),
        "XE" => Some("XE"),
        _ => None,
    }
}

fn document_property_field_name(name: &str) -> Option<DocumentProperty> {
    match name.trim().to_ascii_lowercase().as_str() {
        "title" => Some(DocumentProperty::Title),
        "subject" => Some(DocumentProperty::Subject),
        "author" => Some(DocumentProperty::Author),
        "keywords" => Some(DocumentProperty::Keywords),
        "comments" | "comment" => Some(DocumentProperty::Comments),
        "lastsavedby" | "last saved by" | "operator" => Some(DocumentProperty::Operator),
        "manager" => Some(DocumentProperty::Manager),
        "company" => Some(DocumentProperty::Company),
        "category" => Some(DocumentProperty::Category),
        _ => None,
    }
}

fn document_shortcut_property_field_name(name: &str) -> Option<DocumentProperty> {
    match name {
        "AUTHOR" => Some(DocumentProperty::Author),
        "TITLE" => Some(DocumentProperty::Title),
        "SUBJECT" => Some(DocumentProperty::Subject),
        "KEYWORDS" => Some(DocumentProperty::Keywords),
        "COMMENTS" => Some(DocumentProperty::Comments),
        "LASTSAVEDBY" => Some(DocumentProperty::Operator),
        _ => None,
    }
}

fn document_timestamp_field_name(name: &str) -> Option<DocumentTimestampKind> {
    match name {
        "CREATEDATE" => Some(DocumentTimestampKind::Created),
        "SAVEDATE" => Some(DocumentTimestampKind::Saved),
        "PRINTDATE" => Some(DocumentTimestampKind::Printed),
        _ => None,
    }
}

fn clean_document_property_text(
    text: &str,
    offset: usize,
    limits: &RtfLimits,
) -> Result<Option<String>, ParseError> {
    let text = text.trim().to_string();
    if text.is_empty() || text.chars().any(|ch| ch.is_control()) || contains_internal_marker(&text)
    {
        return Ok(None);
    }
    if text.chars().count() > limits.max_text_run_len {
        return Err(ParseError::ResourceLimitExceeded {
            resource: "document property text".to_string(),
            offset,
        });
    }
    Ok(Some(text))
}

fn normalize_document_timestamp(timestamp: DocumentTimestamp) -> Option<DocumentTimestamp> {
    let year = timestamp.year?;
    let month = timestamp.month?;
    let day = timestamp.day?;
    let hour = timestamp.hour.unwrap_or(0);
    let minute = timestamp.minute.unwrap_or(0);
    let second = timestamp.second.unwrap_or(0);
    if !(1..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
        || !(0..=59).contains(&second)
    {
        return None;
    }
    let max_day = days_in_month(year, month)?;
    if !(1..=max_day).contains(&day) {
        return None;
    }
    Some(DocumentTimestamp {
        year: Some(year),
        month: Some(month),
        day: Some(day),
        hour: Some(hour),
        minute: Some(minute),
        second: Some(second),
    })
}

fn days_in_month(year: i32, month: i32) -> Option<i32> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 if is_leap_year(year) => Some(29),
        2 => Some(28),
        _ => None,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn is_non_visible_resultless_field(name: &str) -> bool {
    matches!(name, "XE" | "TC" | "TA")
}

fn is_auto_number_field(name: &str) -> bool {
    matches!(name, "AUTONUM" | "AUTONUMLGL" | "AUTONUMOUT")
}

fn is_external_resultless_field(name: &str) -> bool {
    matches!(
        name,
        "DATABASE"
            | "DDE"
            | "DDEAUTO"
            | "HYPERLINK"
            | "IMPORT"
            | "INCLUDEPICTURE"
            | "INCLUDETEXT"
            | "LINK"
    )
}

fn is_builtin_passive_result_font(font_name: &str) -> bool {
    font_name.eq_ignore_ascii_case("ZapfDingbats")
        || font_name.eq_ignore_ascii_case("Zapf Dingbats")
}

fn is_legacy_symbol_font_name(name: &str) -> bool {
    matches!(
        name.trim(),
        "symbol" | "symbol mt" | "symbolmt" | "standard symbols l"
    )
}

fn passive_form_text_field_result(form_default_text: &str) -> Option<PassiveFieldResult> {
    let text = form_default_text.trim_end().to_string();
    if text.is_empty() || text.chars().any(|ch| ch.is_control()) {
        return None;
    }
    Some(PassiveFieldResult {
        text,
        font_name: None,
        form_field: true,
    })
}

fn passive_form_dropdown_field_result(
    entries: &[String],
    selected_index: i32,
) -> Option<PassiveFieldResult> {
    let selected_index = usize::try_from(selected_index).ok()?;
    let text = entries.get(selected_index)?.trim_end().to_string();
    if text.is_empty() || text.chars().any(|ch| ch.is_control()) {
        return None;
    }
    Some(PassiveFieldResult {
        text,
        font_name: None,
        form_field: true,
    })
}

fn passive_formula_field_result(instruction: &str) -> Option<PassiveFieldResult> {
    let expression = field_formula_expression(instruction)?;
    if expression.is_empty() || expression.len() > 1024 {
        return None;
    }
    let value = FormulaParser::new(expression).parse()?;
    Some(PassiveFieldResult {
        text: value.to_string(),
        font_name: None,
        form_field: false,
    })
}

fn passive_if_field_result(instruction: &str) -> Option<PassiveFieldResult> {
    let rest = field_rest_after_name(instruction)?.trim_start();
    let (left, rest) = field_if_operand(rest)?;
    let (operator, rest) = field_if_operator(rest.trim_start())?;
    let (right, rest) = field_if_operand(rest.trim_start())?;
    let (true_text, rest) = field_if_result_text(rest.trim_start())?;
    let (false_text, rest) = field_if_optional_result_text(rest.trim_start())?;
    if !field_remainder_contains_only_passive_format_switches(rest) {
        return None;
    }

    let text = if field_if_condition_matches(&left, operator, &right) {
        true_text
    } else {
        false_text
    };
    if text.chars().count() > 1024
        || text.chars().any(|ch| ch.is_control())
        || contains_internal_marker(&text)
    {
        return None;
    }
    Some(PassiveFieldResult {
        text,
        font_name: None,
        form_field: false,
    })
}

fn passive_quote_field_result(instruction: &str) -> Option<PassiveFieldResult> {
    let text = field_first_quoted_argument(instruction)?;
    if text.chars().any(|ch| ch.is_control()) {
        return None;
    }
    Some(PassiveFieldResult {
        text,
        font_name: None,
        form_field: false,
    })
}

fn passive_macrobutton_field_result(instruction: &str) -> Option<PassiveFieldResult> {
    let rest = field_rest_after_name(instruction)?;
    let rest = skip_field_argument(rest)?;
    let display = rest.trim_start();
    if display.is_empty() {
        return None;
    }

    let text = if display.starts_with('"') {
        field_quoted_prefix(display)?
    } else {
        display.trim_end().to_string()
    };
    if text.is_empty() || text.chars().any(|ch| ch.is_control()) {
        return None;
    }

    Some(PassiveFieldResult {
        text,
        font_name: None,
        form_field: false,
    })
}

fn passive_mergefield_result(instruction: &str) -> Option<PassiveFieldResult> {
    let name = field_first_argument(instruction)?;
    if name.is_empty() || name.chars().any(|ch| ch.is_control()) || contains_internal_marker(&name)
    {
        return None;
    }

    Some(PassiveFieldResult {
        text: format!("\u{00ab}{name}\u{00bb}"),
        font_name: None,
        form_field: false,
    })
}

fn passive_symbol_field_result(instruction: &str) -> Option<PassiveFieldResult> {
    let mut chars = instruction.trim_start().chars().peekable();
    while chars.next_if(|ch| ch.is_ascii_alphabetic()).is_some() {}
    while chars.next_if(|ch| ch.is_whitespace()).is_some() {}

    let negative = chars.next_if_eq(&'-').is_some();
    if negative {
        return None;
    }

    let mut value: u32 = 0;
    let mut digit_seen = false;
    while let Some(ch) = chars.peek().copied() {
        let Some(digit) = ch.to_digit(10) else {
            break;
        };
        digit_seen = true;
        value = value.checked_mul(10)?.checked_add(digit)?;
        chars.next();
    }
    if !digit_seen {
        return None;
    }

    let font_name = field_switch_quoted_value(instruction, b'f');
    let mut passive_font_name = font_name.clone();
    let text = if let Some(name) = font_name.as_deref()
        && name.eq_ignore_ascii_case("Symbol")
        && value <= u8::MAX as u32
    {
        map_symbol_char(value as u8 as char).to_string()
    } else if let Some(name) = font_name.as_deref()
        && let Some(mapped) = map_dingbats_codepoint(&name.to_ascii_lowercase(), value)
    {
        passive_font_name = Some("ZapfDingbats".to_string());
        mapped.to_string()
    } else {
        let ch = char::from_u32(value)?;
        if ch.is_control() {
            return None;
        }
        ch.to_string()
    };

    Some(PassiveFieldResult {
        text,
        font_name: passive_font_name,
        form_field: false,
    })
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum FieldIfOperator {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

fn field_if_operand(input: &str) -> Option<(String, &str)> {
    let input = input.trim_start();
    if input.is_empty() || input.starts_with('\\') {
        return None;
    }
    let (text, rest) = if input.starts_with('"') {
        (field_quoted_prefix(input)?, skip_field_argument(input)?)
    } else {
        let end = input
            .char_indices()
            .find_map(|(index, ch)| ch.is_whitespace().then_some(index))
            .unwrap_or(input.len());
        (input[..end].to_string(), &input[end..])
    };
    let text = text.trim().to_string();
    if text.is_empty() || text.chars().any(|ch| ch.is_control()) || contains_internal_marker(&text)
    {
        return None;
    }
    Some((text, rest))
}

fn field_if_operator(input: &str) -> Option<(FieldIfOperator, &str)> {
    for (token, operator) in [
        ("<>", FieldIfOperator::NotEqual),
        ("<=", FieldIfOperator::LessOrEqual),
        (">=", FieldIfOperator::GreaterOrEqual),
        ("=", FieldIfOperator::Equal),
        ("<", FieldIfOperator::Less),
        (">", FieldIfOperator::Greater),
    ] {
        if let Some(rest) = input.strip_prefix(token) {
            return Some((operator, rest));
        }
    }
    None
}

fn field_if_result_text(input: &str) -> Option<(String, &str)> {
    let input = input.trim_start();
    if !input.starts_with('"') {
        return None;
    }
    let text = field_quoted_prefix(input)?;
    let rest = skip_field_argument(input)?;
    Some((text, rest))
}

fn field_if_optional_result_text(input: &str) -> Option<(String, &str)> {
    let input = input.trim_start();
    if input.is_empty() {
        return Some((String::new(), input));
    }
    field_if_result_text(input)
}

fn field_if_condition_matches(left: &str, operator: FieldIfOperator, right: &str) -> bool {
    if let (Ok(left), Ok(right)) = (left.parse::<i64>(), right.parse::<i64>()) {
        return match operator {
            FieldIfOperator::Equal => left == right,
            FieldIfOperator::NotEqual => left != right,
            FieldIfOperator::Less => left < right,
            FieldIfOperator::LessOrEqual => left <= right,
            FieldIfOperator::Greater => left > right,
            FieldIfOperator::GreaterOrEqual => left >= right,
        };
    }

    match operator {
        FieldIfOperator::Equal => left == right,
        FieldIfOperator::NotEqual => left != right,
        FieldIfOperator::Less => left < right,
        FieldIfOperator::LessOrEqual => left <= right,
        FieldIfOperator::Greater => left > right,
        FieldIfOperator::GreaterOrEqual => left >= right,
    }
}

fn field_numeric_picture_switch(instruction: &str) -> Option<Option<String>> {
    let mut in_quote = false;
    let mut escaped = false;

    for (index, ch) in instruction.char_indices() {
        if in_quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_quote = false;
            }
            continue;
        }

        if ch == '"' {
            in_quote = true;
            continue;
        }

        if ch == '\\' {
            let after_backslash = index + ch.len_utf8();
            if instruction[after_backslash..].starts_with('#') {
                let after_hash = after_backslash + '#'.len_utf8();
                return field_numeric_picture_argument(&instruction[after_hash..]).map(Some);
            }
        }
    }

    Some(None)
}

fn field_numeric_picture_argument(input: &str) -> Option<String> {
    field_numeric_picture_argument_with_rest(input).map(|(picture, _)| picture)
}

fn field_numeric_picture_argument_with_rest(input: &str) -> Option<(String, &str)> {
    let input = input.trim_start();
    if input.is_empty() {
        return None;
    }

    let (picture, rest) = if input.starts_with('"') {
        (field_quoted_prefix(input)?, skip_field_argument(input)?)
    } else {
        let end = input
            .char_indices()
            .find_map(|(index, ch)| (ch.is_whitespace() || ch == '\\').then_some(index))
            .unwrap_or(input.len());
        (input[..end].to_string(), &input[end..])
    };
    let picture = picture.trim().to_string();
    if picture.is_empty()
        || picture.chars().count() > MAX_PASSIVE_FIELD_NUMERIC_PICTURE_CHARS
        || picture.chars().any(|ch| ch.is_control())
        || contains_internal_marker(&picture)
    {
        return None;
    }
    Some((picture, rest))
}

fn apply_field_numeric_picture_switch(text: &str, picture: &str) -> Option<String> {
    let value = text.trim().parse::<i64>().ok()?;
    let parsed = parse_simple_numeric_picture(picture)?;
    if parsed.decimal_places > 8 || parsed.min_integer_digits > 18 {
        return None;
    }

    let negative = value < 0;
    let magnitude = value.checked_abs()?;
    let mut integer = magnitude.to_string();
    if integer.len() < parsed.min_integer_digits {
        let mut padded = String::with_capacity(parsed.min_integer_digits);
        for _ in 0..parsed.min_integer_digits - integer.len() {
            padded.push('0');
        }
        padded.push_str(&integer);
        integer = padded;
    }
    if parsed.group_thousands {
        integer = group_decimal_digits(&integer);
    }

    let mut output = String::new();
    if negative {
        output.push('-');
    }
    output.push_str(&parsed.prefix);
    output.push_str(&integer);
    if parsed.decimal_places > 0 {
        output.push('.');
        for _ in 0..parsed.decimal_places {
            output.push('0');
        }
    }
    output.push_str(&parsed.suffix);
    Some(output)
}

fn field_date_picture_switch(instruction: &str) -> Option<Option<String>> {
    let mut in_quote = false;
    let mut escaped = false;

    for (index, ch) in instruction.char_indices() {
        if in_quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_quote = false;
            }
            continue;
        }

        if ch == '"' {
            in_quote = true;
            continue;
        }

        if ch == '\\' {
            let after_backslash = index + ch.len_utf8();
            if instruction[after_backslash..].starts_with('@') {
                let after_at = after_backslash + '@'.len_utf8();
                return field_date_picture_argument(&instruction[after_at..]).map(Some);
            }
        }
    }

    Some(None)
}

fn field_date_picture_argument(input: &str) -> Option<String> {
    let input = input.trim_start();
    if input.is_empty() {
        return None;
    }

    let picture = if input.starts_with('"') {
        field_quoted_prefix(input)?
    } else {
        let end = input
            .char_indices()
            .find_map(|(index, ch)| (ch.is_whitespace() || ch == '\\').then_some(index))
            .unwrap_or(input.len());
        input[..end].to_string()
    };
    let picture = picture.trim().to_string();
    if picture.is_empty()
        || picture.chars().count() > MAX_PASSIVE_FIELD_DATE_PICTURE_CHARS
        || picture.chars().any(|ch| ch.is_control())
        || contains_internal_marker(&picture)
        || !picture
            .chars()
            .all(|ch| ch.is_ascii() && !matches!(ch, '\\' | '{' | '}'))
    {
        return None;
    }
    Some(picture)
}

fn format_default_document_timestamp(timestamp: &DocumentTimestamp) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        timestamp.year.unwrap_or(1),
        timestamp.month.unwrap_or(1),
        timestamp.day.unwrap_or(1),
        timestamp.hour.unwrap_or(0),
        timestamp.minute.unwrap_or(0),
        timestamp.second.unwrap_or(0)
    )
}

fn apply_field_date_picture(timestamp: &DocumentTimestamp, picture: &str) -> Option<String> {
    let mut output = String::new();
    let mut index = 0;
    while index < picture.len() {
        let rest = &picture[index..];
        if let Some(value) = rest.strip_prefix("AM/PM") {
            output.push_str(if timestamp.hour.unwrap_or(0) < 12 {
                "AM"
            } else {
                "PM"
            });
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("am/pm") {
            output.push_str(if timestamp.hour.unwrap_or(0) < 12 {
                "am"
            } else {
                "pm"
            });
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("yyyy") {
            output.push_str(&format!("{:04}", timestamp.year.unwrap_or(1)));
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("yy") {
            output.push_str(&format!("{:02}", timestamp.year.unwrap_or(1) % 100));
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("MMMM") {
            output.push_str(month_name(timestamp.month.unwrap_or(1))?);
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("MMM") {
            output.push_str(month_abbreviation(timestamp.month.unwrap_or(1))?);
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("MM") {
            output.push_str(&format!("{:02}", timestamp.month.unwrap_or(1)));
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix('M') {
            output.push_str(&timestamp.month.unwrap_or(1).to_string());
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("dd") {
            output.push_str(&format!("{:02}", timestamp.day.unwrap_or(1)));
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix('d') {
            output.push_str(&timestamp.day.unwrap_or(1).to_string());
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("HH") {
            output.push_str(&format!("{:02}", timestamp.hour.unwrap_or(0)));
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix('H') {
            output.push_str(&timestamp.hour.unwrap_or(0).to_string());
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("hh") {
            output.push_str(&format!("{:02}", twelve_hour(timestamp.hour.unwrap_or(0))));
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix('h') {
            output.push_str(&twelve_hour(timestamp.hour.unwrap_or(0)).to_string());
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("mm") {
            output.push_str(&format!("{:02}", timestamp.minute.unwrap_or(0)));
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix('m') {
            output.push_str(&timestamp.minute.unwrap_or(0).to_string());
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("ss") {
            output.push_str(&format!("{:02}", timestamp.second.unwrap_or(0)));
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix('s') {
            output.push_str(&timestamp.second.unwrap_or(0).to_string());
            index = picture.len() - value.len();
        } else {
            let ch = rest.chars().next()?;
            if ch.is_ascii_alphabetic() {
                return None;
            }
            output.push(ch);
            index += ch.len_utf8();
        }
        if output.chars().count() > MAX_PASSIVE_FIELD_FORMAT_TEXT_CHARS {
            return None;
        }
    }
    Some(output)
}

fn twelve_hour(hour: i32) -> i32 {
    let hour = hour % 12;
    if hour == 0 { 12 } else { hour }
}

fn month_name(month: i32) -> Option<&'static str> {
    Some(match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => return None,
    })
}

fn month_abbreviation(month: i32) -> Option<&'static str> {
    Some(match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => return None,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SimpleNumericPicture {
    prefix: String,
    suffix: String,
    min_integer_digits: usize,
    group_thousands: bool,
    decimal_places: usize,
}

fn parse_simple_numeric_picture(picture: &str) -> Option<SimpleNumericPicture> {
    if picture.contains(';')
        || picture.contains('%')
        || picture.contains('E')
        || picture.contains('e')
        || picture.contains('x')
        || picture.contains('X')
    {
        return None;
    }

    let first_digit = picture
        .char_indices()
        .find_map(|(index, ch)| matches!(ch, '0' | '#').then_some(index))?;
    let last_digit = picture
        .char_indices()
        .filter_map(|(index, ch)| matches!(ch, '0' | '#').then_some(index))
        .last()?;
    let suffix_start = last_digit + picture[last_digit..].chars().next()?.len_utf8();
    let prefix = picture[..first_digit].to_string();
    let suffix = picture[suffix_start..].to_string();
    if !is_safe_numeric_picture_literal(&prefix) || !is_safe_numeric_picture_literal(&suffix) {
        return None;
    }

    let core = &picture[first_digit..suffix_start];
    if core.chars().any(|ch| !matches!(ch, '0' | '#' | ',' | '.')) {
        return None;
    }
    if core.matches('.').count() > 1 {
        return None;
    }

    let (integer_pattern, decimal_pattern) = core.split_once('.').unwrap_or((core, ""));
    if integer_pattern.is_empty()
        || integer_pattern.chars().filter(|ch| *ch == '0').count() == 0
        || integer_pattern
            .chars()
            .rev()
            .skip_while(|ch| *ch == ',')
            .next()
            .is_none()
    {
        return None;
    }
    if decimal_pattern.chars().any(|ch| ch != '0') {
        return None;
    }

    let min_integer_digits = integer_pattern.chars().filter(|ch| *ch == '0').count();
    Some(SimpleNumericPicture {
        prefix,
        suffix,
        min_integer_digits,
        group_thousands: integer_pattern.contains(','),
        decimal_places: decimal_pattern.len(),
    })
}

fn is_safe_numeric_picture_literal(text: &str) -> bool {
    text.chars()
        .all(|ch| ch.is_ascii() && !ch.is_ascii_control() && !matches!(ch, '\\' | '{' | '}'))
}

fn group_decimal_digits(digits: &str) -> String {
    let mut output = String::new();
    let first_group_len = digits.len() % 3;
    for (index, ch) in digits.chars().enumerate() {
        if index > 0
            && (index == first_group_len
                || (index > first_group_len && (index - first_group_len) % 3 == 0))
        {
            output.push(',');
        }
        output.push(ch);
    }
    output
}

fn field_format_switches(instruction: &str) -> Option<Vec<FieldFormatSwitch>> {
    let mut switches = Vec::new();
    let mut in_quote = false;
    let mut escaped = false;

    for (index, ch) in instruction.char_indices() {
        if in_quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_quote = false;
            }
            continue;
        }

        if ch == '"' {
            in_quote = true;
            continue;
        }

        if ch == '\\' {
            let after_backslash = index + ch.len_utf8();
            if !instruction[after_backslash..].starts_with('*') {
                continue;
            }
            let after_star = after_backslash + '*'.len_utf8();
            if let Some((switch, _)) = field_format_switch_after_star(&instruction[after_star..]) {
                if switches.len() >= 16 {
                    return None;
                }
                switches.push(switch);
            }
        }
    }

    Some(switches)
}

fn field_remainder_contains_only_passive_format_switches(input: &str) -> bool {
    let mut rest = input.trim_start();
    while !rest.is_empty() {
        let Some(after_backslash) = rest.strip_prefix('\\') else {
            return false;
        };
        if let Some(after_star) = after_backslash.strip_prefix('*') {
            let Some((_, after_switch)) = field_format_switch_after_star(after_star) else {
                return false;
            };
            rest = after_switch.trim_start();
        } else if let Some(after_hash) = after_backslash.strip_prefix('#') {
            let Some((_, after_picture)) = field_numeric_picture_argument_with_rest(after_hash)
            else {
                return false;
            };
            rest = after_picture.trim_start();
        } else {
            return false;
        }
    }
    true
}

fn field_format_switch_after_star(input: &str) -> Option<(FieldFormatSwitch, &str)> {
    let input = input.trim_start();
    let (name, rest) = if input.starts_with('"') {
        (field_quoted_prefix(input)?, skip_field_argument(input)?)
    } else {
        let end = input
            .char_indices()
            .find_map(|(index, ch)| (!ch.is_ascii_alphabetic()).then_some(index))
            .unwrap_or(input.len());
        if end == 0 {
            return None;
        }
        (input[..end].to_string(), &input[end..])
    };

    let name = name.trim();
    let switch = if let Some(switch) = field_text_format_switch(name) {
        FieldFormatSwitch::Text(switch)
    } else if let Some(switch) = field_number_format_switch(name) {
        FieldFormatSwitch::Number(switch)
    } else {
        return None;
    };
    Some((switch, rest))
}

fn field_text_format_switch(name: &str) -> Option<FieldTextFormatSwitch> {
    match name.to_ascii_uppercase().as_str() {
        "UPPER" => Some(FieldTextFormatSwitch::Upper),
        "LOWER" => Some(FieldTextFormatSwitch::Lower),
        "FIRSTCAP" => Some(FieldTextFormatSwitch::FirstCap),
        "CAPS" => Some(FieldTextFormatSwitch::Caps),
        _ => None,
    }
}

fn field_number_format_switch(name: &str) -> Option<FieldNumberFormatSwitch> {
    let upper = name.to_ascii_uppercase();
    match upper.as_str() {
        "ARABIC" => Some(FieldNumberFormatSwitch::Arabic),
        "ALPHABETIC" if name.chars().all(|ch| !ch.is_ascii_lowercase()) => {
            Some(FieldNumberFormatSwitch::UpperAlphabetic)
        }
        "ALPHABETIC" => Some(FieldNumberFormatSwitch::LowerAlphabetic),
        "ROMAN" if name.chars().all(|ch| !ch.is_ascii_lowercase()) => {
            Some(FieldNumberFormatSwitch::UpperRoman)
        }
        "ROMAN" => Some(FieldNumberFormatSwitch::LowerRoman),
        "ORDINAL" => Some(FieldNumberFormatSwitch::Ordinal),
        "HEX" => Some(FieldNumberFormatSwitch::Hex),
        _ => None,
    }
}

fn apply_field_text_format_switch(text: &str, switch: FieldTextFormatSwitch) -> String {
    match switch {
        FieldTextFormatSwitch::Upper => text.chars().flat_map(char::to_uppercase).collect(),
        FieldTextFormatSwitch::Lower => text.chars().flat_map(char::to_lowercase).collect(),
        FieldTextFormatSwitch::FirstCap => uppercase_first_alphabetic(text),
        FieldTextFormatSwitch::Caps => uppercase_each_word_start(text),
    }
}

fn apply_field_number_format_switch(text: &str, switch: FieldNumberFormatSwitch) -> Option<String> {
    let value = text.trim().parse::<i32>().ok()?;
    match switch {
        FieldNumberFormatSwitch::Arabic => Some(value.to_string()),
        FieldNumberFormatSwitch::UpperAlphabetic => Some(format_alpha_counter(value, false)),
        FieldNumberFormatSwitch::LowerAlphabetic => Some(format_alpha_counter(value, true)),
        FieldNumberFormatSwitch::UpperRoman => Some(format_roman_counter(value, false)),
        FieldNumberFormatSwitch::LowerRoman => Some(format_roman_counter(value, true)),
        FieldNumberFormatSwitch::Ordinal => Some(format_ordinal_counter(value)),
        FieldNumberFormatSwitch::Hex if value >= 0 => Some(format!("{value:X}")),
        FieldNumberFormatSwitch::Hex => None,
    }
}

fn uppercase_first_alphabetic(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut changed = false;
    for ch in text.chars() {
        if !changed && ch.is_alphabetic() {
            output.extend(ch.to_uppercase());
            changed = true;
        } else {
            output.push(ch);
        }
    }
    output
}

fn uppercase_each_word_start(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut capitalize_next = true;
    for ch in text.chars() {
        if ch.is_alphabetic() {
            if capitalize_next {
                output.extend(ch.to_uppercase());
            } else {
                output.push(ch);
            }
            capitalize_next = false;
        } else {
            output.push(ch);
            capitalize_next = !ch.is_alphanumeric();
        }
    }
    output
}

fn field_first_quoted_argument(instruction: &str) -> Option<String> {
    field_quoted_prefix(field_rest_after_name(instruction)?.trim_start())
}

fn field_formula_expression(instruction: &str) -> Option<&str> {
    let rest = instruction.trim_start().strip_prefix('=')?;
    let end = rest
        .char_indices()
        .find_map(|(index, ch)| (ch == '\\').then_some(index))
        .unwrap_or(rest.len());
    Some(rest[..end].trim())
}

fn field_first_argument(instruction: &str) -> Option<String> {
    let rest = field_rest_after_name(instruction)?.trim_start();
    if rest.starts_with('"') {
        return field_quoted_prefix(rest).map(|text| text.trim().to_string());
    }

    let end = rest
        .char_indices()
        .find_map(|(index, ch)| (ch.is_whitespace() || ch == '\\').then_some(index))
        .unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}

fn clean_bookmark_name(name: String) -> Option<String> {
    let name = name.trim().to_string();
    if name.is_empty()
        || name.starts_with('_')
        || name.chars().any(|ch| ch.is_control())
        || contains_internal_marker(&name)
    {
        return None;
    }
    Some(name)
}

fn bookmark_page_anchor_marker(id: usize) -> String {
    format!("{BOOKMARK_PAGE_ANCHOR_MARKER}{id}{BOOKMARK_PAGE_MARKER_END}")
}

fn bookmark_page_ref_marker(id: usize) -> String {
    format!("{BOOKMARK_PAGE_REF_MARKER}{id}{BOOKMARK_PAGE_MARKER_END}")
}

impl<'a> FormulaParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn parse(mut self) -> Option<i64> {
        let value = self.parse_expression()?;
        self.skip_ws();
        (self.pos == self.input.len()).then_some(value)
    }

    fn parse_expression(&mut self) -> Option<i64> {
        let mut value = self.parse_term()?;
        loop {
            self.skip_ws();
            if self.consume('+') {
                value = value.checked_add(self.parse_term()?)?;
            } else if self.consume('-') {
                value = value.checked_sub(self.parse_term()?)?;
            } else {
                return Some(value);
            }
        }
    }

    fn parse_term(&mut self) -> Option<i64> {
        let mut value = self.parse_factor()?;
        loop {
            self.skip_ws();
            if self.consume('*') {
                value = value.checked_mul(self.parse_factor()?)?;
            } else if self.consume('/') {
                let divisor = self.parse_factor()?;
                if divisor == 0 {
                    return None;
                }
                value = value.checked_div(divisor)?;
            } else {
                return Some(value);
            }
        }
    }

    fn parse_factor(&mut self) -> Option<i64> {
        self.skip_ws();
        if self.consume('(') {
            let value = self.parse_expression()?;
            self.skip_ws();
            return self.consume(')').then_some(value);
        }

        let negative = self.consume('-');
        let value = self.parse_number()?;
        if negative {
            value.checked_neg()
        } else {
            Some(value)
        }
    }

    fn parse_number(&mut self) -> Option<i64> {
        self.skip_ws();
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
        (self.pos > start).then(|| self.input[start..self.pos].parse::<i64>().ok())?
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    fn consume(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.pos += expected.len_utf8();
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }
}

fn field_sequence_instruction(instruction: &str) -> Option<FieldSequenceInstruction> {
    let rest = field_rest_after_name(instruction)?.trim_start();
    if rest.is_empty() || rest.starts_with('\\') {
        return None;
    }

    let text = if rest.starts_with('"') {
        field_quoted_prefix(rest)?
    } else {
        let end = rest
            .char_indices()
            .find_map(|(index, ch)| (ch.is_whitespace() || ch == '\\').then_some(index))
            .unwrap_or(rest.len());
        rest[..end].to_string()
    };
    let text = text.trim().to_string();
    if text.is_empty() || text.chars().any(|ch| ch.is_control()) || contains_internal_marker(&text)
    {
        return None;
    }

    let mut rest = skip_field_argument(rest).unwrap_or("");
    let mut repeat_current = false;
    let mut hidden = false;
    let mut reset_value = None;
    while let Some((switch, after_switch)) = next_field_switch(rest) {
        rest = after_switch;
        match switch {
            'c' => repeat_current = true,
            'h' => hidden = true,
            'n' => repeat_current = false,
            'r' => {
                let (value, after_value) = field_switch_i32(rest);
                reset_value = value;
                rest = after_value;
            }
            _ => {}
        }
    }

    Some(FieldSequenceInstruction {
        name: text,
        repeat_current,
        hidden,
        reset_value,
    })
}

fn field_list_number_instruction(instruction: &str) -> Option<FieldListNumberInstruction> {
    let rest = field_rest_after_name(instruction)?.trim_start();
    let (name, mut rest) = if rest.is_empty() || rest.starts_with('\\') {
        (String::new(), rest)
    } else if rest.starts_with('"') {
        let name = field_quoted_prefix(rest)?.trim().to_string();
        (name, skip_field_argument(rest).unwrap_or(""))
    } else {
        let end = rest
            .char_indices()
            .find_map(|(index, ch)| (ch.is_whitespace() || ch == '\\').then_some(index))
            .unwrap_or(rest.len());
        (rest[..end].trim().to_string(), &rest[end..])
    };
    if name.chars().any(|ch| ch.is_control()) || contains_internal_marker(&name) {
        return None;
    }

    let mut level = 1;
    let mut reset_value = None;
    while let Some((switch, after_switch)) = next_field_switch(rest) {
        rest = after_switch;
        match switch {
            'l' => {
                let (value, after_value) = field_switch_i32(rest);
                if let Some(value) = value
                    && value > 0
                {
                    level = value;
                }
                rest = after_value;
            }
            's' => {
                let (value, after_value) = field_switch_i32(rest);
                reset_value = value;
                rest = after_value;
            }
            _ => {}
        }
    }

    Some(FieldListNumberInstruction {
        name,
        level,
        reset_value,
    })
}

fn field_rest_after_name(instruction: &str) -> Option<&str> {
    let instruction = instruction.trim_start();
    let name_len = instruction
        .chars()
        .take_while(|ch| ch.is_ascii_alphabetic())
        .map(char::len_utf8)
        .sum::<usize>();
    (name_len > 0).then_some(&instruction[name_len..])
}

fn field_quoted_prefix(input: &str) -> Option<String> {
    let mut rest = input.chars();
    if rest.next()? != '"' {
        return None;
    }
    let mut output = String::new();
    let mut escaped = false;
    for ch in rest {
        if escaped {
            match ch {
                '"' | '\\' => output.push(ch),
                _ => {
                    output.push('\\');
                    output.push(ch);
                }
            }
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some(output);
        } else {
            output.push(ch);
        }
    }
    None
}

fn skip_field_argument(input: &str) -> Option<&str> {
    let input = input.trim_start();
    if input.is_empty() {
        return None;
    }

    if input.starts_with('"') {
        let mut escaped = false;
        for (index, ch) in input.char_indices().skip(1) {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                return Some(&input[index + ch.len_utf8()..]);
            }
        }
        return None;
    }

    let end = input
        .char_indices()
        .find_map(|(index, ch)| ch.is_whitespace().then_some(index))
        .unwrap_or(input.len());
    Some(&input[end..])
}

fn next_field_switch(input: &str) -> Option<(char, &str)> {
    let input = input.trim_start();
    let rest = input.strip_prefix('\\')?;
    let mut chars = rest.char_indices();
    let (_, switch) = chars.next()?;
    let switch_end = switch.len_utf8();
    Some((switch.to_ascii_lowercase(), &rest[switch_end..]))
}

fn field_switch_i32(input: &str) -> (Option<i32>, &str) {
    let input = input.trim_start();
    let mut end = 0;
    for (index, ch) in input.char_indices() {
        if index == 0 && ch == '-' {
            end = ch.len_utf8();
            continue;
        }
        if !ch.is_ascii_digit() {
            break;
        }
        end = index + ch.len_utf8();
    }

    if end == 0 || input[..end].chars().all(|ch| ch == '-') {
        return (None, input);
    }

    let value = input[..end].parse::<i32>().ok();
    (value, &input[end..])
}

fn field_switch_quoted_value(instruction: &str, switch: u8) -> Option<String> {
    let bytes = instruction.as_bytes();
    let switch = switch.to_ascii_lowercase();
    let mut index = 0;
    while index + 1 < bytes.len() {
        if bytes[index] == b'\\' && bytes[index + 1].to_ascii_lowercase() == switch {
            let mut value_start = index + 2;
            while value_start < bytes.len() && bytes[value_start].is_ascii_whitespace() {
                value_start += 1;
            }
            if value_start < bytes.len() && bytes[value_start] == b'"' {
                value_start += 1;
                let value_end = bytes[value_start..]
                    .iter()
                    .position(|byte| *byte == b'"')
                    .map(|relative| value_start + relative)?;
                return Some(instruction[value_start..value_end].to_string());
            }
        }
        index += 1;
    }
    None
}

fn is_internal_marker(text: &str) -> bool {
    matches!(
        text,
        PAGE_NUMBER_MARKER
            | TOTAL_PAGES_MARKER
            | SECTION_NUMBER_MARKER
            | SECTION_PAGES_MARKER
            | DOCUMENT_WORDS_MARKER
            | DOCUMENT_CHARS_MARKER
            | DOCUMENT_CHARS_WITH_SPACES_MARKER
            | PENDING_NOTE_REFERENCE_MARKER
    ) || is_bookmark_page_marker(text)
}

fn contains_internal_marker(text: &str) -> bool {
    text.contains(PAGE_NUMBER_MARKER)
        || text.contains(TOTAL_PAGES_MARKER)
        || text.contains(SECTION_NUMBER_MARKER)
        || text.contains(SECTION_PAGES_MARKER)
        || text.contains(DOCUMENT_WORDS_MARKER)
        || text.contains(DOCUMENT_CHARS_MARKER)
        || text.contains(DOCUMENT_CHARS_WITH_SPACES_MARKER)
        || text.contains(PENDING_NOTE_REFERENCE_MARKER)
        || text.contains(BOOKMARK_PAGE_ANCHOR_MARKER)
        || text.contains(BOOKMARK_PAGE_REF_MARKER)
        || text.contains(BOOKMARK_PAGE_MARKER_END)
}

fn sanitize_internal_markers(text: &str) -> String {
    text.replace(PAGE_NUMBER_MARKER, "\u{fffd}")
        .replace(TOTAL_PAGES_MARKER, "\u{fffd}")
        .replace(SECTION_NUMBER_MARKER, "\u{fffd}")
        .replace(SECTION_PAGES_MARKER, "\u{fffd}")
        .replace(DOCUMENT_WORDS_MARKER, "\u{fffd}")
        .replace(DOCUMENT_CHARS_MARKER, "\u{fffd}")
        .replace(DOCUMENT_CHARS_WITH_SPACES_MARKER, "\u{fffd}")
        .replace(PENDING_NOTE_REFERENCE_MARKER, "\u{fffd}")
        .replace(BOOKMARK_PAGE_ANCHOR_MARKER, "\u{fffd}")
        .replace(BOOKMARK_PAGE_REF_MARKER, "\u{fffd}")
        .replace(BOOKMARK_PAGE_MARKER_END, "\u{fffd}")
}

fn is_bookmark_page_marker(text: &str) -> bool {
    parse_bookmark_page_marker_id(text, BOOKMARK_PAGE_ANCHOR_MARKER).is_some()
        || parse_bookmark_page_marker_id(text, BOOKMARK_PAGE_REF_MARKER).is_some()
}

fn contains_bookmark_page_marker(text: &str) -> bool {
    text.contains(BOOKMARK_PAGE_ANCHOR_MARKER)
        || text.contains(BOOKMARK_PAGE_REF_MARKER)
        || text.contains(BOOKMARK_PAGE_MARKER_END)
}

fn parse_bookmark_page_marker_id(text: &str, prefix: &str) -> Option<usize> {
    let rest = text.strip_prefix(prefix)?;
    let id = rest.strip_suffix(BOOKMARK_PAGE_MARKER_END)?;
    (!id.is_empty() && id.chars().all(|ch| ch.is_ascii_digit()))
        .then(|| id.parse::<usize>().ok())?
}

fn format_list_counter(value: i32, format: ListNumberFormat) -> String {
    match format {
        ListNumberFormat::Decimal | ListNumberFormat::Other | ListNumberFormat::Bullet => {
            value.to_string()
        }
        ListNumberFormat::UpperRoman => format_roman_counter(value, false),
        ListNumberFormat::LowerRoman => format_roman_counter(value, true),
        ListNumberFormat::UpperLetter => format_alpha_counter(value, false),
        ListNumberFormat::LowerLetter => format_alpha_counter(value, true),
        ListNumberFormat::Ordinal => format_ordinal_counter(value),
        ListNumberFormat::DecimalLeadingZero(width) => {
            format_zero_padded_decimal_counter(value, width)
        }
    }
}

fn format_note_number(start: i32, sequence: usize, format: PageNumberFormat) -> String {
    let sequence_offset = sequence.saturating_sub(1).min(i32::MAX as usize) as i32;
    let value = start.max(1).saturating_add(sequence_offset);
    match format {
        PageNumberFormat::Decimal => value.to_string(),
        PageNumberFormat::UpperRoman => format_roman_counter(value, false),
        PageNumberFormat::LowerRoman => format_roman_counter(value, true),
        PageNumberFormat::UpperLetter => format_alpha_counter(value, false),
        PageNumberFormat::LowerLetter => format_alpha_counter(value, true),
    }
}

fn list_level_start_at(
    list_override: &ListOverride,
    level: &ListLevelDefinition,
    level_index: usize,
) -> i32 {
    list_override
        .level_overrides
        .iter()
        .find(|level_override| level_override.level_index == level_index)
        .and_then(|level_override| level_override.start_at)
        .unwrap_or(level.start_at)
}

fn has_list_level_placeholders(template: &str) -> bool {
    template
        .chars()
        .any(|ch| list_level_placeholder_index(ch).is_some())
}

fn list_level_placeholder_index(ch: char) -> Option<usize> {
    if ch <= '\u{8}' {
        Some(ch as usize)
    } else {
        None
    }
}

fn list_level_follow_text(follow: ListLevelFollow) -> &'static str {
    match follow {
        ListLevelFollow::Tab => "\t",
        ListLevelFollow::Space => " ",
        ListLevelFollow::Nothing => "",
    }
}

fn format_roman_counter(value: i32, lowercase: bool) -> String {
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

fn format_alpha_counter(value: i32, lowercase: bool) -> String {
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

fn format_ordinal_counter(value: i32) -> String {
    if value <= 0 {
        return value.to_string();
    }

    let suffix = match value % 100 {
        11..=13 => "th",
        _ => match value % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    format!("{value}{suffix}")
}

fn format_zero_padded_decimal_counter(value: i32, width: usize) -> String {
    if value < 0 {
        return value.to_string();
    }
    format!("{value:0width$}")
}

fn push_text_to_paragraph(
    paragraph: &mut Paragraph,
    text: &str,
    paragraph_style: &ParagraphStyle,
    character_style: &CharacterStyle,
) {
    if paragraph.style != *paragraph_style && paragraph.runs.is_empty() {
        paragraph.style = paragraph_style.clone();
    }

    if let Some(last) = paragraph.runs.last_mut()
        && last.style == *character_style
        && !contains_bookmark_page_marker(text)
        && !contains_bookmark_page_marker(&last.text)
    {
        last.text.push_str(text);
        return;
    }

    paragraph.runs.push(Run {
        text: text.to_string(),
        style: character_style.clone(),
    });
}

fn push_text_to_runs(runs: &mut Vec<Run>, text: &str, character_style: &CharacterStyle) {
    if let Some(last) = runs.last_mut()
        && last.style == *character_style
    {
        last.text.push_str(text);
        return;
    }

    runs.push(Run {
        text: text.to_string(),
        style: character_style.clone(),
    });
}

fn replace_last_pending_note_marker_in_block(block: &mut Block, replacement: &str) -> bool {
    match block {
        Block::Paragraph(paragraph) => {
            replace_last_pending_note_marker_in_paragraph(paragraph, replacement)
        }
        Block::Table(table) => {
            for row in table.rows.iter_mut().rev() {
                for cell in row.cells.iter_mut().rev() {
                    for paragraph in cell.paragraphs.iter_mut().rev() {
                        if replace_last_pending_note_marker_in_paragraph(paragraph, replacement) {
                            return true;
                        }
                    }
                }
            }
            false
        }
        _ => false,
    }
}

fn replace_last_pending_note_marker_in_paragraph(
    paragraph: &mut Paragraph,
    replacement: &str,
) -> bool {
    for run in paragraph.runs.iter_mut().rev() {
        if run.text.contains(PENDING_NOTE_REFERENCE_MARKER) {
            run.text = replace_last_marker(&run.text, PENDING_NOTE_REFERENCE_MARKER, replacement);
            return true;
        }
    }
    false
}

fn replace_last_pending_note_marker_in_paragraphs(
    paragraphs: &mut [Paragraph],
    replacement: &str,
) -> bool {
    for paragraph in paragraphs.iter_mut().rev() {
        if replace_last_pending_note_marker_in_paragraph(paragraph, replacement) {
            return true;
        }
    }
    false
}

fn replace_last_marker(text: &str, marker: &str, replacement: &str) -> String {
    let Some(idx) = text.rfind(marker) else {
        return text.to_string();
    };
    let mut output = String::with_capacity(text.len() + replacement.len());
    output.push_str(&text[..idx]);
    output.push_str(replacement);
    output.push_str(&text[idx + marker.len()..]);
    output
}

fn replace_all_pending_note_markers_in_block(block: &mut Block, replacement: &str) {
    match block {
        Block::Paragraph(paragraph) => {
            replace_all_pending_note_markers_in_paragraph(paragraph, replacement);
        }
        Block::Table(table) => {
            for row in &mut table.rows {
                for cell in &mut row.cells {
                    for paragraph in &mut cell.paragraphs {
                        replace_all_pending_note_markers_in_paragraph(paragraph, replacement);
                    }
                }
            }
        }
        _ => {}
    }
}

fn replace_all_pending_note_markers_in_paragraph(paragraph: &mut Paragraph, replacement: &str) {
    for run in &mut paragraph.runs {
        if run.text.contains(PENDING_NOTE_REFERENCE_MARKER) {
            run.text = run.text.replace(PENDING_NOTE_REFERENCE_MARKER, replacement);
        }
    }
}

fn replace_all_pending_note_markers_in_paragraphs(paragraphs: &mut [Paragraph], replacement: &str) {
    for paragraph in paragraphs {
        replace_all_pending_note_markers_in_paragraph(paragraph, replacement);
    }
}

fn normalize_list_level_template(template: &str) -> String {
    let mut chars = template.chars().collect::<Vec<_>>();
    if chars
        .first()
        .is_some_and(|ch| ch.is_control() && *ch != '\0')
    {
        chars.remove(0);
    }
    while chars.last().is_some_and(|ch| *ch == ';') {
        chars.pop();
    }
    chars.into_iter().collect()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn take_rtf_unicode_char(pending_high_surrogate: &mut Option<u16>, value: i32) -> Option<char> {
    if value > 0xFFFF {
        *pending_high_surrogate = None;
        return char::from_u32(value as u32);
    }

    let unit = if value < 0 {
        let Some(adjusted) = value.checked_add(65_536) else {
            *pending_high_surrogate = None;
            return None;
        };
        match u16::try_from(adjusted) {
            Ok(unit) => unit,
            Err(_) => {
                *pending_high_surrogate = None;
                return None;
            }
        }
    } else {
        match u16::try_from(value) {
            Ok(unit) => unit,
            Err(_) => {
                *pending_high_surrogate = None;
                return None;
            }
        }
    };

    if (0xD800..=0xDBFF).contains(&unit) {
        *pending_high_surrogate = Some(unit);
        return None;
    }

    if (0xDC00..=0xDFFF).contains(&unit) {
        let high = pending_high_surrogate.take()?;
        let codepoint =
            0x1_0000 + (((u32::from(high) - 0xD800) << 10) | (u32::from(unit) - 0xDC00));
        return char::from_u32(codepoint);
    }

    *pending_high_surrogate = None;
    char::from_u32(u32::from(unit))
}

fn decode_hex_byte(byte: u8, code_page: CodePage) -> char {
    match code_page {
        CodePage::Windows1252 => decode_windows_1252(byte),
        CodePage::MacRoman => decode_high_byte(byte, &MAC_ROMAN_HIGH),
        CodePage::Ibm437 => decode_high_byte(byte, &CP437_HIGH),
        CodePage::Ibm850 => decode_high_byte(byte, &CP850_HIGH),
        CodePage::Unsupported if byte.is_ascii() => byte as char,
        CodePage::Unsupported => '\u{fffd}',
    }
}

fn decode_high_byte(byte: u8, high_table: &[char; 128]) -> char {
    if byte.is_ascii() {
        byte as char
    } else {
        high_table[usize::from(byte - 0x80)]
    }
}

fn map_symbol_char(ch: char) -> char {
    let code = ch as u32;
    if code > 0xff {
        return ch;
    }
    match code as u8 {
        b'A' => '\u{0391}',
        b'B' => '\u{0392}',
        b'C' => '\u{03a7}',
        b'D' => '\u{0394}',
        b'E' => '\u{0395}',
        b'F' => '\u{03a6}',
        b'G' => '\u{0393}',
        b'H' => '\u{0397}',
        b'I' => '\u{0399}',
        b'K' => '\u{039a}',
        b'L' => '\u{039b}',
        b'M' => '\u{039c}',
        b'N' => '\u{039d}',
        b'O' => '\u{039f}',
        b'P' => '\u{03a0}',
        b'Q' => '\u{0398}',
        b'R' => '\u{03a1}',
        b'S' => '\u{03a3}',
        b'T' => '\u{03a4}',
        b'U' => '\u{03a5}',
        b'W' => '\u{03a9}',
        b'X' => '\u{039e}',
        b'Y' => '\u{03a8}',
        b'Z' => '\u{0396}',
        b'a' => '\u{03b1}',
        b'b' => '\u{03b2}',
        b'c' => '\u{03c7}',
        b'd' => '\u{03b4}',
        b'e' => '\u{03b5}',
        b'f' => '\u{03c6}',
        b'g' => '\u{03b3}',
        b'h' => '\u{03b7}',
        b'i' => '\u{03b9}',
        b'j' => '\u{03d5}',
        b'k' => '\u{03ba}',
        b'l' => '\u{03bb}',
        b'm' => '\u{03bc}',
        b'n' => '\u{03bd}',
        b'o' => '\u{03bf}',
        b'p' => '\u{03c0}',
        b'q' => '\u{03b8}',
        b'r' => '\u{03c1}',
        b's' => '\u{03c3}',
        b't' => '\u{03c4}',
        b'u' => '\u{03c5}',
        b'v' => '\u{03d6}',
        b'w' => '\u{03c9}',
        b'x' => '\u{03be}',
        b'y' => '\u{03c8}',
        b'z' => '\u{03b6}',
        0x22 => '\u{2200}',
        0x24 => '\u{2203}',
        0x27 => '\u{220b}',
        0x2a => '\u{2217}',
        0x2d => '\u{2212}',
        0x40 => '\u{2245}',
        0x5c => '\u{2234}',
        0x5e => '\u{22a5}',
        0x7e => '\u{223c}',
        0xa3 => '\u{2264}',
        0xb3 => '\u{2265}',
        0xb4 => '\u{00d7}',
        0xb5 => '\u{221d}',
        0xb6 => '\u{2202}',
        0xb7 => '\u{2022}',
        0xb8 => '\u{00f7}',
        0xb9 => '\u{2260}',
        0xba => '\u{2261}',
        0xbb => '\u{2248}',
        0xbc => '\u{2026}',
        0xbd => '\u{23d0}',
        0xbe => '\u{23af}',
        0xbf => '\u{21b5}',
        0xc0 => '\u{2135}',
        0xc1 => '\u{2111}',
        0xc2 => '\u{211c}',
        0xc3 => '\u{2118}',
        0xc4 => '\u{2297}',
        0xc5 => '\u{2295}',
        0xc6 => '\u{2205}',
        0xc7 => '\u{2229}',
        0xc8 => '\u{222a}',
        0xc9 => '\u{2283}',
        0xca => '\u{2287}',
        0xcb => '\u{2284}',
        0xcc => '\u{2282}',
        0xcd => '\u{2286}',
        0xce => '\u{2208}',
        0xcf => '\u{2209}',
        0xd0 => '\u{2220}',
        0xd1 => '\u{2207}',
        0xd2 => '\u{00ae}',
        0xd3 => '\u{00a9}',
        0xd4 => '\u{2122}',
        0xd5 => '\u{220f}',
        0xd6 => '\u{221a}',
        0xd7 => '\u{22c5}',
        0xd8 => '\u{00ac}',
        0xd9 => '\u{2227}',
        0xda => '\u{2228}',
        0xdb => '\u{21d4}',
        0xdc => '\u{21d0}',
        0xdd => '\u{21d1}',
        0xde => '\u{21d2}',
        0xdf => '\u{21d3}',
        0xe0 => '\u{25ca}',
        0xe5 => '\u{2211}',
        0xe6 => '\u{239b}',
        0xe7 => '\u{239c}',
        0xe8 => '\u{239d}',
        0xe9 => '\u{23a1}',
        0xea => '\u{23a2}',
        0xeb => '\u{23a3}',
        0xec => '\u{23a7}',
        0xed => '\u{23a8}',
        0xee => '\u{23a9}',
        0xef => '\u{23aa}',
        0xf0 => '\u{20ac}',
        0xf1 => '\u{2329}',
        0xf2 => '\u{222b}',
        0xf3 => '\u{232a}',
        0xf4 => '\u{2320}',
        0xf5 => '\u{23ae}',
        0xf6 => '\u{2321}',
        0xf7 => '\u{239e}',
        0xf8 => '\u{239f}',
        0xf9 => '\u{23a0}',
        0xfa => '\u{23a4}',
        0xfb => '\u{23a5}',
        0xfc => '\u{23a6}',
        0xfd => '\u{23ab}',
        0xfe => '\u{23ac}',
        0xff => '\u{23ad}',
        _ => ch,
    }
}

fn is_wingdings_font_name(name: &str) -> bool {
    name.contains("wingdings") && !is_wingdings2_font_name(name) && !is_wingdings3_font_name(name)
}

fn is_wingdings2_font_name(name: &str) -> bool {
    name.contains("wingdings 2") || name.contains("wingdings2")
}

fn is_wingdings3_font_name(name: &str) -> bool {
    name.contains("wingdings 3") || name.contains("wingdings3")
}

fn is_webdings_font_name(name: &str) -> bool {
    name.contains("webdings")
}

fn dingbats_mapper_for_font_name(name: &str) -> Option<fn(char) -> char> {
    if is_wingdings2_font_name(name) {
        Some(map_wingdings2_char)
    } else if is_wingdings3_font_name(name) {
        Some(map_wingdings3_char)
    } else if is_wingdings_font_name(name) {
        Some(map_wingdings_char)
    } else {
        None
    }
}

fn map_dingbats_codepoint(font_name: &str, codepoint: u32) -> Option<char> {
    if is_wingdings2_font_name(font_name) {
        map_wingdings2_codepoint(codepoint)
    } else if is_wingdings3_font_name(font_name) {
        map_wingdings3_codepoint(codepoint)
    } else if is_wingdings_font_name(font_name) {
        map_wingdings_codepoint(codepoint)
    } else if is_webdings_font_name(font_name) {
        map_webdings_codepoint(codepoint)
    } else {
        None
    }
}

fn map_wingdings_char(ch: char) -> char {
    map_wingdings_codepoint(ch as u32).unwrap_or(ch)
}

fn map_wingdings_codepoint(codepoint: u32) -> Option<char> {
    let code = if (0xf000..=0xf0ff).contains(&codepoint) {
        (codepoint - 0xf000) as u8
    } else if codepoint <= u8::MAX as u32 {
        codepoint as u8
    } else {
        return None;
    };

    match code {
        0xa3 => Some('\u{2610}'),
        0xfb => Some('\u{2717}'),
        0xfc => Some('\u{2713}'),
        0xfe => Some('\u{2611}'),
        _ => None,
    }
}

fn map_wingdings2_char(ch: char) -> char {
    map_wingdings2_codepoint(ch as u32).unwrap_or(ch)
}

fn map_wingdings2_codepoint(codepoint: u32) -> Option<char> {
    let code = if (0xf000..=0xf0ff).contains(&codepoint) {
        (codepoint - 0xf000) as u8
    } else if codepoint <= u8::MAX as u32 {
        codepoint as u8
    } else {
        return None;
    };

    match code {
        0x4f => Some('\u{2717}'),
        0x50 => Some('\u{2713}'),
        0x51 | 0x53 | 0x54 => Some('\u{2612}'),
        0x52 => Some('\u{2611}'),
        _ => None,
    }
}

fn map_wingdings3_char(ch: char) -> char {
    map_wingdings3_codepoint(ch as u32).unwrap_or(ch)
}

fn map_wingdings3_codepoint(codepoint: u32) -> Option<char> {
    let code = if (0xf000..=0xf0ff).contains(&codepoint) {
        (codepoint - 0xf000) as u8
    } else if codepoint <= u8::MAX as u32 {
        codepoint as u8
    } else {
        return None;
    };

    match code {
        b'f' => Some('\u{2190}'),
        b'g' => Some('\u{2192}'),
        b'h' => Some('\u{2191}'),
        b'i' => Some('\u{2193}'),
        _ => None,
    }
}

fn map_webdings_char(ch: char) -> char {
    map_webdings_codepoint(ch as u32).unwrap_or(ch)
}

fn map_webdings_codepoint(codepoint: u32) -> Option<char> {
    let code = if (0xf000..=0xf0ff).contains(&codepoint) {
        (codepoint - 0xf000) as u8
    } else if codepoint <= u8::MAX as u32 {
        codepoint as u8
    } else {
        return None;
    };

    match code {
        0x3f => Some('\u{2612}'),
        0x61 => Some('\u{2714}'),
        0x63 => Some('\u{25a1}'),
        _ => None,
    }
}

fn normalized_page_settings(mut page: PageSettings) -> PageSettings {
    if page.landscape && page.width_twips < page.height_twips {
        std::mem::swap(&mut page.width_twips, &mut page.height_twips);
    }
    page.column_count = page.column_count.max(1);
    page.column_widths_twips.truncate(page.column_count);
    page.column_gaps_twips.truncate(page.column_count);
    page
}

fn normalized_preferred_table_width_twips(
    preferred_width: PreferredTableWidth,
    page_content_width_twips: i32,
    max_width_twips: i32,
) -> Option<i32> {
    let value = preferred_width.value?.max(0);
    let width = match preferred_width.unit {
        PreferredTableWidthUnit::Twips => value,
        PreferredTableWidthUnit::FiftiethsPercent => {
            let content_width = i64::from(page_content_width_twips.max(1));
            let scaled = content_width.saturating_mul(i64::from(value)) / 5_000;
            i32::try_from(scaled).unwrap_or(i32::MAX)
        }
        PreferredTableWidthUnit::Auto => return None,
    };
    Some(width.clamp(1, max_width_twips.max(1)))
}

fn resize_column_vector(values: &mut Vec<i32>, index: usize, default: i32) {
    if values.len() <= index {
        values.resize(index + 1, default);
    }
}

fn decode_windows_1252(byte: u8) -> char {
    match byte {
        0x80 => '\u{20ac}',
        0x81 => '\u{fffd}',
        0x82 => '\u{201a}',
        0x83 => '\u{0192}',
        0x84 => '\u{201e}',
        0x85 => '\u{2026}',
        0x86 => '\u{2020}',
        0x87 => '\u{2021}',
        0x88 => '\u{02c6}',
        0x89 => '\u{2030}',
        0x8a => '\u{0160}',
        0x8b => '\u{2039}',
        0x8c => '\u{0152}',
        0x8d => '\u{fffd}',
        0x8e => '\u{017d}',
        0x8f => '\u{fffd}',
        0x90 => '\u{fffd}',
        0x91 => '\u{2018}',
        0x92 => '\u{2019}',
        0x93 => '\u{201c}',
        0x94 => '\u{201d}',
        0x95 => '\u{2022}',
        0x96 => '\u{2013}',
        0x97 => '\u{2014}',
        0x98 => '\u{02dc}',
        0x99 => '\u{2122}',
        0x9a => '\u{0161}',
        0x9b => '\u{203a}',
        0x9c => '\u{0153}',
        0x9d => '\u{fffd}',
        0x9e => '\u{017e}',
        0x9f => '\u{0178}',
        _ => byte as char,
    }
}

const MAC_ROMAN_HIGH: [char; 128] = [
    '\u{00c4}', '\u{00c5}', '\u{00c7}', '\u{00c9}', '\u{00d1}', '\u{00d6}', '\u{00dc}', '\u{00e1}',
    '\u{00e0}', '\u{00e2}', '\u{00e4}', '\u{00e3}', '\u{00e5}', '\u{00e7}', '\u{00e9}', '\u{00e8}',
    '\u{00ea}', '\u{00eb}', '\u{00ed}', '\u{00ec}', '\u{00ee}', '\u{00ef}', '\u{00f1}', '\u{00f3}',
    '\u{00f2}', '\u{00f4}', '\u{00f6}', '\u{00f5}', '\u{00fa}', '\u{00f9}', '\u{00fb}', '\u{00fc}',
    '\u{2020}', '\u{00b0}', '\u{00a2}', '\u{00a3}', '\u{00a7}', '\u{2022}', '\u{00b6}', '\u{00df}',
    '\u{00ae}', '\u{00a9}', '\u{2122}', '\u{00b4}', '\u{00a8}', '\u{2260}', '\u{00c6}', '\u{00d8}',
    '\u{221e}', '\u{00b1}', '\u{2264}', '\u{2265}', '\u{00a5}', '\u{00b5}', '\u{2202}', '\u{2211}',
    '\u{220f}', '\u{03c0}', '\u{222b}', '\u{00aa}', '\u{00ba}', '\u{03a9}', '\u{00e6}', '\u{00f8}',
    '\u{00bf}', '\u{00a1}', '\u{00ac}', '\u{221a}', '\u{0192}', '\u{2248}', '\u{2206}', '\u{00ab}',
    '\u{00bb}', '\u{2026}', '\u{00a0}', '\u{00c0}', '\u{00c3}', '\u{00d5}', '\u{0152}', '\u{0153}',
    '\u{2013}', '\u{2014}', '\u{201c}', '\u{201d}', '\u{2018}', '\u{2019}', '\u{00f7}', '\u{25ca}',
    '\u{00ff}', '\u{0178}', '\u{2044}', '\u{20ac}', '\u{2039}', '\u{203a}', '\u{fb01}', '\u{fb02}',
    '\u{2021}', '\u{00b7}', '\u{201a}', '\u{201e}', '\u{2030}', '\u{00c2}', '\u{00ca}', '\u{00c1}',
    '\u{00cb}', '\u{00c8}', '\u{00cd}', '\u{00ce}', '\u{00cf}', '\u{00cc}', '\u{00d3}', '\u{00d4}',
    '\u{f8ff}', '\u{00d2}', '\u{00da}', '\u{00db}', '\u{00d9}', '\u{0131}', '\u{02c6}', '\u{02dc}',
    '\u{00af}', '\u{02d8}', '\u{02d9}', '\u{02da}', '\u{00b8}', '\u{02dd}', '\u{02db}', '\u{02c7}',
];

const CP437_HIGH: [char; 128] = [
    '\u{00c7}', '\u{00fc}', '\u{00e9}', '\u{00e2}', '\u{00e4}', '\u{00e0}', '\u{00e5}', '\u{00e7}',
    '\u{00ea}', '\u{00eb}', '\u{00e8}', '\u{00ef}', '\u{00ee}', '\u{00ec}', '\u{00c4}', '\u{00c5}',
    '\u{00c9}', '\u{00e6}', '\u{00c6}', '\u{00f4}', '\u{00f6}', '\u{00f2}', '\u{00fb}', '\u{00f9}',
    '\u{00ff}', '\u{00d6}', '\u{00dc}', '\u{00a2}', '\u{00a3}', '\u{00a5}', '\u{20a7}', '\u{0192}',
    '\u{00e1}', '\u{00ed}', '\u{00f3}', '\u{00fa}', '\u{00f1}', '\u{00d1}', '\u{00aa}', '\u{00ba}',
    '\u{00bf}', '\u{2310}', '\u{00ac}', '\u{00bd}', '\u{00bc}', '\u{00a1}', '\u{00ab}', '\u{00bb}',
    '\u{2591}', '\u{2592}', '\u{2593}', '\u{2502}', '\u{2524}', '\u{2561}', '\u{2562}', '\u{2556}',
    '\u{2555}', '\u{2563}', '\u{2551}', '\u{2557}', '\u{255d}', '\u{255c}', '\u{255b}', '\u{2510}',
    '\u{2514}', '\u{2534}', '\u{252c}', '\u{251c}', '\u{2500}', '\u{253c}', '\u{255e}', '\u{255f}',
    '\u{255a}', '\u{2554}', '\u{2569}', '\u{2566}', '\u{2560}', '\u{2550}', '\u{256c}', '\u{2567}',
    '\u{2568}', '\u{2564}', '\u{2565}', '\u{2559}', '\u{2558}', '\u{2552}', '\u{2553}', '\u{256b}',
    '\u{256a}', '\u{2518}', '\u{250c}', '\u{2588}', '\u{2584}', '\u{258c}', '\u{2590}', '\u{2580}',
    '\u{03b1}', '\u{00df}', '\u{0393}', '\u{03c0}', '\u{03a3}', '\u{03c3}', '\u{00b5}', '\u{03c4}',
    '\u{03a6}', '\u{0398}', '\u{03a9}', '\u{03b4}', '\u{221e}', '\u{03c6}', '\u{03b5}', '\u{2229}',
    '\u{2261}', '\u{00b1}', '\u{2265}', '\u{2264}', '\u{2320}', '\u{2321}', '\u{00f7}', '\u{2248}',
    '\u{00b0}', '\u{2219}', '\u{00b7}', '\u{221a}', '\u{207f}', '\u{00b2}', '\u{25a0}', '\u{00a0}',
];

const CP850_HIGH: [char; 128] = [
    '\u{00c7}', '\u{00fc}', '\u{00e9}', '\u{00e2}', '\u{00e4}', '\u{00e0}', '\u{00e5}', '\u{00e7}',
    '\u{00ea}', '\u{00eb}', '\u{00e8}', '\u{00ef}', '\u{00ee}', '\u{00ec}', '\u{00c4}', '\u{00c5}',
    '\u{00c9}', '\u{00e6}', '\u{00c6}', '\u{00f4}', '\u{00f6}', '\u{00f2}', '\u{00fb}', '\u{00f9}',
    '\u{00ff}', '\u{00d6}', '\u{00dc}', '\u{00f8}', '\u{00a3}', '\u{00d8}', '\u{00d7}', '\u{0192}',
    '\u{00e1}', '\u{00ed}', '\u{00f3}', '\u{00fa}', '\u{00f1}', '\u{00d1}', '\u{00aa}', '\u{00ba}',
    '\u{00bf}', '\u{00ae}', '\u{00ac}', '\u{00bd}', '\u{00bc}', '\u{00a1}', '\u{00ab}', '\u{00bb}',
    '\u{2591}', '\u{2592}', '\u{2593}', '\u{2502}', '\u{2524}', '\u{00c1}', '\u{00c2}', '\u{00c0}',
    '\u{00a9}', '\u{2563}', '\u{2551}', '\u{2557}', '\u{255d}', '\u{00a2}', '\u{00a5}', '\u{2510}',
    '\u{2514}', '\u{2534}', '\u{252c}', '\u{251c}', '\u{2500}', '\u{253c}', '\u{00e3}', '\u{00c3}',
    '\u{255a}', '\u{2554}', '\u{2569}', '\u{2566}', '\u{2560}', '\u{2550}', '\u{256c}', '\u{00a4}',
    '\u{00f0}', '\u{00d0}', '\u{00ca}', '\u{00cb}', '\u{00c8}', '\u{0131}', '\u{00cd}', '\u{00ce}',
    '\u{00cf}', '\u{2518}', '\u{250c}', '\u{2588}', '\u{2584}', '\u{00a6}', '\u{00cc}', '\u{2580}',
    '\u{00d3}', '\u{00df}', '\u{00d4}', '\u{00d2}', '\u{00f5}', '\u{00d5}', '\u{00b5}', '\u{00fe}',
    '\u{00de}', '\u{00da}', '\u{00db}', '\u{00d9}', '\u{00fd}', '\u{00dd}', '\u{00af}', '\u{00b4}',
    '\u{00ad}', '\u{00b1}', '\u{2017}', '\u{00be}', '\u{00b6}', '\u{00a7}', '\u{00f7}', '\u{00b8}',
    '\u{00b0}', '\u{00a8}', '\u{00b7}', '\u{00b9}', '\u{00b3}', '\u{00b2}', '\u{25a0}', '\u{00a0}',
];

#[derive(Debug)]
struct ParsedJpeg {
    width_px: u32,
    height_px: u32,
    format: ImageFormat,
}

fn parse_jpeg_image_data(bytes: &[u8]) -> Option<ParsedJpeg> {
    if bytes.len() < 4 || bytes[0] != 0xff || bytes[1] != 0xd8 {
        return None;
    }

    let mut pos = 2;
    while pos + 3 < bytes.len() {
        while pos < bytes.len() && bytes[pos] != 0xff {
            pos += 1;
        }
        while pos < bytes.len() && bytes[pos] == 0xff {
            pos += 1;
        }
        if pos >= bytes.len() {
            return None;
        }

        let marker = bytes[pos];
        pos += 1;
        if marker == 0xd9 || marker == 0xda {
            return None;
        }
        if is_standalone_jpeg_marker(marker) {
            continue;
        }
        if pos + 1 >= bytes.len() {
            return None;
        }

        let segment_len = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as usize;
        if segment_len < 2 || pos + segment_len > bytes.len() {
            return None;
        }

        if is_jpeg_start_of_frame(marker) {
            if segment_len < 8 {
                return None;
            }
            let height = u16::from_be_bytes([bytes[pos + 3], bytes[pos + 4]]) as u32;
            let width = u16::from_be_bytes([bytes[pos + 5], bytes[pos + 6]]) as u32;
            let components = bytes[pos + 7];
            let expected_segment_len =
                8usize.checked_add(usize::from(components).checked_mul(3)?)?;
            if segment_len < expected_segment_len {
                return None;
            }
            if width == 0 || height == 0 {
                return None;
            }
            let format = match components {
                1 => ImageFormat::JpegGrayscale,
                3 => ImageFormat::Jpeg,
                4 => ImageFormat::JpegCmyk,
                _ => return None,
            };
            return Some(ParsedJpeg {
                width_px: width,
                height_px: height,
                format,
            });
        }

        pos += segment_len;
    }

    None
}

#[derive(Debug)]
struct ParsedPng {
    width_px: u32,
    height_px: u32,
    format: ImageFormat,
    idat: Vec<u8>,
    palette: Vec<u8>,
}

#[derive(Debug)]
struct ParsedDib {
    width_px: u32,
    height_px: u32,
    rgb: Vec<u8>,
}

fn parse_dib_image_data(bytes: &[u8], max_pixels: usize) -> Option<ParsedDib> {
    const BITMAPINFOHEADER_SIZE: usize = 40;
    const BI_RGB: u32 = 0;

    if bytes.len() < BITMAPINFOHEADER_SIZE {
        return None;
    }

    let header_size = read_le_u32(bytes, 0)? as usize;
    if header_size < BITMAPINFOHEADER_SIZE || header_size > bytes.len() {
        return None;
    }

    let width = read_le_i32(bytes, 4)?;
    let raw_height = read_le_i32(bytes, 8)?;
    let planes = read_le_u16(bytes, 12)?;
    let bits_per_pixel = read_le_u16(bytes, 14)?;
    let compression = read_le_u32(bytes, 16)?;
    if width <= 0 || raw_height == 0 || planes != 1 || compression != BI_RGB {
        return None;
    }

    let width_px = u32::try_from(width).ok()?;
    let height_px = raw_height.unsigned_abs();
    let pixels = usize::try_from(width_px)
        .ok()?
        .checked_mul(usize::try_from(height_px).ok()?)?;
    if pixels == 0 || pixels > max_pixels {
        return None;
    }

    let width = usize::try_from(width_px).ok()?;
    let height = usize::try_from(height_px).ok()?;
    let colors_used = read_le_u32(bytes, 32)?;
    let (row_stride, pixel_start, palette_entries) = match bits_per_pixel {
        1 | 4 | 8 => {
            let palette_capacity = 1usize.checked_shl(u32::from(bits_per_pixel))?;
            let palette_entries = if colors_used == 0 {
                palette_capacity
            } else {
                usize::try_from(colors_used).ok()?
            };
            if palette_entries == 0 || palette_entries > palette_capacity {
                return None;
            }
            let palette_bytes = palette_entries.checked_mul(4)?;
            let palette_end = header_size.checked_add(palette_bytes)?;
            if palette_end > bytes.len() {
                return None;
            }
            let row_bits = width.checked_mul(usize::from(bits_per_pixel))?;
            let row_stride = row_bits.checked_add(31)?.checked_div(32)?.checked_mul(4)?;
            (row_stride, palette_end, Some(palette_entries))
        }
        24 => {
            let unpadded_row_bytes = width.checked_mul(3)?;
            let row_stride = unpadded_row_bytes
                .checked_add(3)?
                .checked_div(4)?
                .checked_mul(4)?;
            (row_stride, header_size, None)
        }
        32 => {
            let row_stride = width.checked_mul(4)?;
            (row_stride, header_size, None)
        }
        _ => return None,
    };
    let pixel_bytes = row_stride.checked_mul(usize::try_from(height_px).ok()?)?;
    let pixel_end = pixel_start.checked_add(pixel_bytes)?;
    if pixel_end > bytes.len() {
        return None;
    }

    let output_len = pixels.checked_mul(3)?;
    let mut rgb = vec![0; output_len];
    let top_down = raw_height < 0;
    for output_y in 0..height {
        let source_y = if top_down {
            output_y
        } else {
            height - 1 - output_y
        };
        let source_row = pixel_start.checked_add(source_y.checked_mul(row_stride)?)?;
        let output_row = output_y.checked_mul(width)?.checked_mul(3)?;
        for x in 0..width {
            let output = output_row.checked_add(x.checked_mul(3)?)?;
            match bits_per_pixel {
                1 => {
                    let source = source_row.checked_add(x / 8)?;
                    let byte = *bytes.get(source)?;
                    let shift = 7 - (x % 8);
                    let palette_index = usize::from((byte >> shift) & 0x01);
                    copy_dib_palette_color(
                        bytes,
                        header_size,
                        palette_entries?,
                        palette_index,
                        &mut rgb[output..output + 3],
                    )?;
                }
                4 => {
                    let source = source_row.checked_add(x / 2)?;
                    let byte = *bytes.get(source)?;
                    let palette_index = if x % 2 == 0 {
                        usize::from(byte >> 4)
                    } else {
                        usize::from(byte & 0x0f)
                    };
                    copy_dib_palette_color(
                        bytes,
                        header_size,
                        palette_entries?,
                        palette_index,
                        &mut rgb[output..output + 3],
                    )?;
                }
                8 => {
                    let source = source_row.checked_add(x)?;
                    let palette_index = usize::from(*bytes.get(source)?);
                    copy_dib_palette_color(
                        bytes,
                        header_size,
                        palette_entries?,
                        palette_index,
                        &mut rgb[output..output + 3],
                    )?;
                }
                24 => {
                    let source = source_row.checked_add(x.checked_mul(3)?)?;
                    rgb[output] = bytes[source + 2];
                    rgb[output + 1] = bytes[source + 1];
                    rgb[output + 2] = bytes[source];
                }
                32 => {
                    let source = source_row.checked_add(x.checked_mul(4)?)?;
                    rgb[output] = bytes[source + 2];
                    rgb[output + 1] = bytes[source + 1];
                    rgb[output + 2] = bytes[source];
                }
                _ => return None,
            }
        }
    }

    Some(ParsedDib {
        width_px,
        height_px,
        rgb,
    })
}

fn copy_dib_palette_color(
    bytes: &[u8],
    palette_start: usize,
    palette_entries: usize,
    palette_index: usize,
    output: &mut [u8],
) -> Option<()> {
    if palette_index >= palette_entries || output.len() < 3 {
        return None;
    }
    let palette = palette_start.checked_add(palette_index.checked_mul(4)?)?;
    output[0] = *bytes.get(palette + 2)?;
    output[1] = *bytes.get(palette + 1)?;
    output[2] = *bytes.get(palette)?;
    Some(())
}

fn read_le_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    Some(u16::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn read_le_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn read_le_i32(bytes: &[u8], offset: usize) -> Option<i32> {
    let end = offset.checked_add(4)?;
    Some(i32::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn parse_png_image_data(bytes: &[u8]) -> Option<ParsedPng> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < PNG_SIGNATURE.len() || &bytes[..8] != PNG_SIGNATURE {
        return None;
    }

    let mut pos = PNG_SIGNATURE.len();
    let mut width_px = 0;
    let mut height_px = 0;
    let mut format = None;
    let mut saw_ihdr = false;
    let mut saw_plte = false;
    let mut saw_iend = false;
    let mut idat = Vec::new();
    let mut palette = Vec::new();

    while pos.checked_add(12)? <= bytes.len() {
        let len = u32::from_be_bytes(bytes[pos..pos + 4].try_into().ok()?) as usize;
        let chunk_type = &bytes[pos + 4..pos + 8];
        let data_start = pos + 8;
        let data_end = data_start.checked_add(len)?;
        let next = data_end.checked_add(4)?;
        if next > bytes.len() {
            return None;
        }
        let data = &bytes[data_start..data_end];

        match chunk_type {
            b"IHDR" => {
                if saw_ihdr || len != 13 || pos != PNG_SIGNATURE.len() {
                    return None;
                }
                width_px = u32::from_be_bytes(data[0..4].try_into().ok()?);
                height_px = u32::from_be_bytes(data[4..8].try_into().ok()?);
                let bit_depth = data[8];
                let color_type = data[9];
                let compression = data[10];
                let filter = data[11];
                let interlace = data[12];
                if width_px == 0
                    || height_px == 0
                    || bit_depth != 8
                    || !matches!(color_type, 0 | 2 | 3)
                    || compression != 0
                    || filter != 0
                    || interlace != 0
                {
                    return None;
                }
                format = Some(match color_type {
                    0 => ImageFormat::PngGrayscale,
                    2 => ImageFormat::Png,
                    3 => ImageFormat::PngIndexed,
                    _ => return None,
                });
                saw_ihdr = true;
            }
            b"PLTE" => {
                if !saw_ihdr || saw_plte || saw_iend || !idat.is_empty() {
                    return None;
                }
                if len == 0 || len % 3 != 0 || len > 256 * 3 {
                    return None;
                }
                palette.extend_from_slice(data);
                saw_plte = true;
            }
            b"IDAT" => {
                if !saw_ihdr || saw_iend {
                    return None;
                }
                if matches!(format, Some(ImageFormat::PngIndexed)) && !saw_plte {
                    return None;
                }
                idat.extend_from_slice(data);
            }
            b"IEND" => {
                if len != 0 || !saw_ihdr {
                    return None;
                }
                saw_iend = true;
                break;
            }
            _ => {}
        }

        pos = next;
    }

    if !saw_ihdr || !saw_iend || idat.is_empty() {
        return None;
    }
    if matches!(format, Some(ImageFormat::PngIndexed)) && palette.is_empty() {
        return None;
    }

    Some(ParsedPng {
        width_px,
        height_px,
        format: format?,
        idat,
        palette,
    })
}

fn is_standalone_jpeg_marker(marker: u8) -> bool {
    marker == 0x01 || (0xd0..=0xd7).contains(&marker)
}

fn is_jpeg_start_of_frame(marker: u8) -> bool {
    matches!(
        marker,
        0xc0 | 0xc1 | 0xc2 | 0xc3 | 0xc5 | 0xc6 | 0xc7 | 0xc9 | 0xca | 0xcb | 0xcd | 0xce | 0xcf
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document_text(document: &Document) -> String {
        document
            .blocks
            .iter()
            .flat_map(|block| match block {
                Block::Paragraph(paragraph) => paragraph
                    .runs
                    .iter()
                    .filter(|run| !is_bookmark_page_marker(&run.text))
                    .map(|run| run.text.as_str())
                    .collect::<Vec<_>>(),
                Block::Placeholder(text) => vec![text.as_str()],
                _ => Vec::new(),
            })
            .collect::<String>()
    }

    #[test]
    fn extracts_unicode_and_skips_fallback() {
        let output = parse_rtf(r"{\rtf1\ansi\uc1 Hello \u8212- world\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        assert_eq!(paragraph.runs[0].text, "Hello \u{2014} world");
    }

    #[test]
    fn clamps_unicode_fallback_skip_count() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_unicode_fallback_skip: 1,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\ansi\uc999 Before \u8212- after\par}", &options)
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "Before \u{2014} after");
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("Unicode fallback skip clamped from 999 to 1")
        }));
    }

    #[test]
    fn prefers_unicode_alternate_destination_over_ansi_fallback() {
        let output =
            parse_rtf(r"{\rtf1 Before {\upr{fallback-ascii}{\*\ud{\u937?}}} After\par}").unwrap();
        let text = document_text(&output.document);
        assert!(text.contains("Before \u{03a9} After"));
        assert!(!text.contains("fallback-ascii"));
        assert!(!text.contains("upr"));
        assert!(!text.contains("ud"));
    }

    #[test]
    fn unicode_alternate_destination_preserves_parent_destination() {
        let output = parse_rtf(
            r"{\rtf1{\header Header {\upr{fallback-header}{\*\ud{\u937?}}}\par}Body\par}",
        )
        .unwrap();
        assert_eq!(output.document.header.len(), 1);
        assert_eq!(output.document.header[0].runs[0].text, "Header \u{03a9}");
        assert_eq!(document_text(&output.document), "Body");
    }

    #[test]
    fn decodes_hex_escapes_with_windows_1252_semantics() {
        let output =
            parse_rtf(r"{\rtf1\ansi\ansicpg1252 Quote \'93Hello\'94 dash \'97\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(text, "Quote \u{201c}Hello\u{201d} dash \u{2014}");
    }

    #[test]
    fn unicode_fallback_skip_consumes_hex_escape_tokens() {
        let output = parse_rtf(r"{\rtf1\ansi\uc1 Unicode \u8217\'92 kept\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(text, "Unicode \u{2019} kept");
    }

    #[test]
    fn unicode_surrogate_pairs_render_as_single_scalar_value() {
        let output = parse_rtf(r"{\rtf1\ansi\uc1 Face \u-10179?\u-8704? done\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(text, "Face \u{1f600} done");
    }

    #[test]
    fn unmatched_unicode_surrogates_do_not_cross_plain_text_boundaries() {
        let output =
            parse_rtf(r"{\rtf1\ansi\uc1 Broken \u-10179? text \u-8704? done\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(text, "Broken  text  done");
    }

    #[test]
    fn decodes_hex_escapes_with_ibm437_semantics() {
        let output = parse_rtf(r"{\rtf1\ansicpg437 high \'82 box \'b3 ascii \'41\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(text, "high \u{00e9} box \u{2502} ascii A");
        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn font_code_page_overrides_global_hex_escape_decoding() {
        let output = parse_rtf(
            r"{\rtf1\ansi\ansicpg1252{\fonttbl{\f0\cpg437 Terminal;}{\f1\cpg10000 Mac Face;}{\f2 Unsupported;}}\f0 IBM \'b3 \f1 Mac \'d2Hello\'d3 \f2 Win \'93Hello\'94\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();
        let code_page_for = |name: &str| {
            output
                .document
                .fonts
                .iter()
                .find(|font| font.name == name)
                .and_then(|font| font.code_page)
                .unwrap_or_else(|| panic!("missing font code page {name}"))
        };

        assert_eq!(code_page_for("Terminal"), 437);
        assert_eq!(code_page_for("Mac Face"), 10000);
        assert_eq!(
            text,
            "IBM \u{2502} Mac \u{201c}Hello\u{201d} Win \u{201c}Hello\u{201d}"
        );
        assert!(!text.contains("cpg"));
        assert!(!text.contains("Terminal"));
    }

    #[test]
    fn font_charset_fallback_guides_hex_escape_decoding() {
        let output = parse_rtf(
            r"{\rtf1\ansi\ansicpg1252{\fonttbl{\f0\fcharset77 Mac Face;}{\f1\fcharset255 Oem Face;}}\f0 Mac \'d2Hello\'d3 \f1 OEM \'82 line \'b3\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(text, "Mac \u{201c}Hello\u{201d} OEM \u{00e9} line \u{2502}");
        assert!(!text.contains("fcharset"));
        assert!(!text.contains("Mac Face"));
    }

    #[test]
    fn ansi_font_charset_overrides_legacy_document_code_page() {
        let output = parse_rtf(
            r"{\rtf1\ansi\ansicpg437{\fonttbl{\f0\fcharset0 Ansi Face;}}\f0 Quote \'93Hello\'94 dash \'97\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert_eq!(text, "Quote \u{201c}Hello\u{201d} dash \u{2014}");
        assert!(!text.contains("fcharset"));
        assert!(!text.contains("Ansi Face"));
    }

    #[test]
    fn explicit_font_code_page_overrides_font_charset_fallback() {
        let output = parse_rtf(
            r"{\rtf1\ansi\ansicpg1252{\fonttbl{\f0\fcharset77\cpg437 Mixed Face;}}\f0 Mixed \'b3\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert_eq!(text, "Mixed \u{2502}");
    }

    #[test]
    fn unsupported_font_code_page_replaces_non_ascii_hex_escapes() {
        let output =
            parse_rtf(r"{\rtf1\ansi{\fonttbl{\f0\cpg932 ShiftJis;}}\f0 high \'82 ascii \'41\par}")
                .unwrap();
        let text = document_text(&output.document);

        assert_eq!(text, "high \u{fffd} ascii A");
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("unsupported RTF font code page 932")
        }));
    }

    #[test]
    fn decodes_hex_escapes_with_ibm850_semantics() {
        let output = parse_rtf(r"{\rtf1\pca high \'9b line \'c4 ascii \'41\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(text, "high \u{00f8} line \u{2500} ascii A");
        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn decodes_hex_escapes_with_mac_roman_semantics() {
        let output =
            parse_rtf(r"{\rtf1\mac quote \'d2Hello\'d3 bullet \'a5 ascii \'41\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(text, "quote \u{201c}Hello\u{201d} bullet \u{2022} ascii A");
        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn unsupported_code_page_replaces_non_ascii_hex_escapes() {
        let output = parse_rtf(r"{\rtf1\ansicpg932 high \'82 ascii \'41\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(text, "high \u{fffd} ascii A");
        assert!(!output.diagnostics.is_empty());
    }

    #[test]
    fn normalizes_named_visible_text_controls() {
        let output = parse_rtf(
            r"{\rtf1 A\emdash B\endash C\bullet D\lquote E\rquote F\ldblquote G\rdblquote\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(
            text,
            "A\u{2014}B\u{2013}C\u{2022}D\u{2018}E\u{2019}F\u{201c}G\u{201d}"
        );
    }

    #[test]
    fn strips_soft_line_and_page_break_layout_artifacts() {
        let output =
            parse_rtf(r"{\rtf1 Before \softline soft line \softpage soft page\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(text, "Before soft line soft page");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
        assert!(!output.document.blocks.iter().any(|block| {
            matches!(
                block,
                Block::PageBreak | Block::ColumnBreak | Block::SectionBreak
            )
        }));
    }

    #[test]
    fn normalizes_escaped_special_visible_characters() {
        let output = parse_rtf(r"{\rtf1 non\~break optional\-hyphen non\_hyphen\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(
            text,
            "non\u{00a0}break optional\u{00ad}hyphen non\u{2011}hyphen"
        );
    }

    #[test]
    fn normalizes_zero_width_formatting_controls() {
        let output =
            parse_rtf(r"{\rtf1 A\zwbo B\zwnbo C\zwnj D\zwj E\ltrmark L\rtlmark R\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(
            text,
            "A\u{200b}B\u{feff}C\u{200c}D\u{200d}E\u{200e}L\u{200f}R"
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn hidden_text_controls_do_not_become_body_text() {
        let output = parse_rtf(
            r"{\rtf1 Visible {\v hidden \emdash \u8217? \'41}{\v0 shown} {\vanish hidden2}{\vanish0 visible2}\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Visible"));
        assert!(text.contains("shown"));
        assert!(text.contains("visible2"));
        assert!(!text.contains("hidden"));
        assert!(!text.contains("hidden2"));
        assert!(!text.contains("\u{2014}"));
        assert!(!text.contains("\u{2019}"));
        assert!(!text.contains('A'));
    }

    #[test]
    fn hidden_resultless_active_content_does_not_create_placeholders() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\v {\field{\*\fldinst INCLUDEPICTURE "https://example.com/a.png"}}{\object\objdata 414243}{\shp{\shpinst{\sp{\sn pFragments}{\sv hidden-payload}}}}} After\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before  After"));
        assert!(!text.contains("[Field removed"));
        assert!(!text.contains("[Embedded object removed]"));
        assert!(!text.contains("[Shape skipped"));
        assert!(!text.contains("https://example.com"));
        assert!(!text.contains("414243"));
        assert!(!text.contains("hidden-payload"));
    }

    #[test]
    fn parses_font_and_color_tables() {
        let input = r"{\rtf1{\fonttbl{\f0\fswiss Arial;}{\f1 Courier New;}}{\colortbl;\red255\green0\blue0;}Hello}";
        let output = parse_rtf(input).unwrap();
        assert!(
            output
                .document
                .fonts
                .iter()
                .any(|font| font.name == "Arial" && font.family == FontFamilyHint::Swiss)
        );
        assert!(
            output
                .document
                .fonts
                .iter()
                .any(|font| font.name == "Courier New" && font.family == FontFamilyHint::Nil)
        );
        assert_eq!(
            output.document.colors[0],
            Color {
                red: 0,
                green: 0,
                blue: 0
            }
        );
        assert_eq!(
            output.document.colors[1],
            Color {
                red: 255,
                green: 0,
                blue: 0
            }
        );
    }

    #[test]
    fn strips_embedded_font_destinations_from_font_table_metadata() {
        let input = r"{\rtf1{\fonttbl{\f0\fswiss Arial{\fontemb{\fontfile HOSTILE-FONT-PAYLOAD {\object\objdata 414243}}};}{\f1 Courier New;}}Visible\par}";
        let output = parse_rtf(input).unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Visible"));
        assert!(
            output
                .document
                .fonts
                .iter()
                .any(|font| { font.name == "Arial" && font.family == FontFamilyHint::Swiss })
        );
        assert!(
            output
                .document
                .fonts
                .iter()
                .any(|font| font.name == "Courier New")
        );
        for forbidden in [
            "HOSTILE-FONT-PAYLOAD",
            "fontemb",
            "fontfile",
            "object",
            "objdata",
            "414243",
        ] {
            assert!(
                !text.contains(forbidden),
                "embedded font payload leaked into text: {forbidden}"
            );
            assert!(
                output
                    .document
                    .fonts
                    .iter()
                    .all(|font| !font.name.contains(forbidden)),
                "embedded font payload leaked into font name: {forbidden}"
            );
        }
    }

    #[test]
    fn normalizes_font_family_hints_as_safe_metadata() {
        let input = r"{\rtf1{\fonttbl{\f0\froman Mystery Serif;}{\f1\fmodern Mystery Mono;}{\f2\ftech Symbolish;}}\f0 A\f1 B\f2 C\par}";
        let output = parse_rtf(input).unwrap();
        let family_for = |name: &str| {
            output
                .document
                .fonts
                .iter()
                .find(|font| font.name == name)
                .map(|font| font.family)
                .unwrap_or_else(|| panic!("missing font {name}"))
        };

        assert_eq!(family_for("Mystery Serif"), FontFamilyHint::Roman);
        assert_eq!(family_for("Mystery Mono"), FontFamilyHint::Modern);
        assert_eq!(family_for("Symbolish"), FontFamilyHint::Tech);
    }

    #[test]
    fn normalizes_theme_font_hints_as_safe_family_metadata() {
        let input = r"{\rtf1{\fonttbl{\f0\flomajor Mystery Heading;}{\f1\fhiminor Mystery Body;}{\f2\fdbmajor Mystery EastAsia Heading;}{\f3\fbiminor Mystery Bidi Body;}}\f0 A\f1 B\f2 C\f3 D\par}";
        let output = parse_rtf(input).unwrap();
        let family_for = |name: &str| {
            output
                .document
                .fonts
                .iter()
                .find(|font| font.name == name)
                .map(|font| font.family)
                .unwrap_or_else(|| panic!("missing font {name}"))
        };

        assert_eq!(family_for("Mystery Heading"), FontFamilyHint::Roman);
        assert_eq!(family_for("Mystery Body"), FontFamilyHint::Swiss);
        assert_eq!(
            family_for("Mystery EastAsia Heading"),
            FontFamilyHint::Roman
        );
        assert_eq!(family_for("Mystery Bidi Body"), FontFamilyHint::Swiss);
    }

    #[test]
    fn normalizes_font_pitch_hints_as_safe_metadata() {
        let input = r"{\rtf1{\fonttbl{\f0\fnil\fprq1 Mystery Fixed;}{\f1\fnil\fprq2 Mystery Variable;}{\f2\fnil\fprq42 Mystery Default;}}\f0 Fixed\f1 Variable\f2 Default\par}";
        let output = parse_rtf(input).unwrap();
        let pitch_for = |name: &str| {
            output
                .document
                .fonts
                .iter()
                .find(|font| font.name == name)
                .map(|font| font.pitch)
                .unwrap_or_else(|| panic!("missing font {name}"))
        };
        let text = document_text(&output.document);

        assert_eq!(pitch_for("Mystery Fixed"), FontPitch::Fixed);
        assert_eq!(pitch_for("Mystery Variable"), FontPitch::Variable);
        assert_eq!(pitch_for("Mystery Default"), FontPitch::Default);
        assert!(text.contains("Fixed"));
        assert!(text.contains("Variable"));
        assert!(text.contains("Default"));
        assert!(!text.contains("fprq"));
        assert!(!text.contains("Mystery Fixed"));
    }

    #[test]
    fn normalizes_font_alternate_name_as_safe_metadata() {
        let input = r"{\rtf1{\fonttbl{\f0\fnil Mystery Sans{\*\falt Courier New;};}}\f0 Body\par}";
        let output = parse_rtf(input).unwrap();
        let text = document_text(&output.document);
        let font = output
            .document
            .fonts
            .iter()
            .find(|font| font.name == "Mystery Sans")
            .expect("font with alternate");

        assert_eq!(font.alternate_name.as_deref(), Some("Courier New"));
        assert!(text.contains("Body"));
        for forbidden in ["falt", "Courier New"] {
            assert!(
                !text.contains(forbidden),
                "font alternate metadata leaked into text: {forbidden}"
            );
        }
    }

    #[test]
    fn normalizes_font_switches_on_runs() {
        let input = r"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Courier New;}}\f1 Mono\par}";
        let output = parse_rtf(input).unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].style.font_index, 1);
        assert_eq!(paragraph.runs[0].text, "Mono");
    }

    #[test]
    fn default_font_control_sets_untagged_text_and_plain_reset_font() {
        let input = r"{\rtf1\deff1{\fonttbl{\f0\fswiss Arial;}{\f1\froman Times New Roman;}}\plain Default \f0 Sans \plain Back\par}";
        let output = parse_rtf(input).unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let font_for = |text: &str| {
            paragraph
                .runs
                .iter()
                .find(|run| run.text.trim() == text)
                .map(|run| run.style.font_index)
                .unwrap_or_else(|| panic!("missing run {text}"))
        };

        assert_eq!(font_for("Default"), 1);
        assert_eq!(font_for("Sans"), 0);
        assert_eq!(font_for("Back"), 1);
    }

    #[test]
    fn normalizes_symbol_font_charset_text_to_unicode() {
        let output =
            parse_rtf(r"{\rtf1{\fonttbl{\f0 Arial;}{\f1\fcharset2 Symbol;}}\f1 ab \'b7\par}")
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(output.document.fonts[1].charset, Some(2));
        assert_eq!(paragraph.runs[0].text, "\u{03b1}\u{03b2} \u{2022}");
    }

    #[test]
    fn normalizes_wingdings_checkbox_glyphs_to_safe_unicode() {
        let output =
            parse_rtf(r"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Wingdings;}}\f1 \'a3 \'fe \'fc \'fb\par}")
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(
            paragraph.runs[0].text,
            "\u{2610} \u{2611} \u{2713} \u{2717}"
        );
        assert_eq!(paragraph.runs[0].style.font_index, 1);
    }

    #[test]
    fn normalizes_wingdings2_checkbox_glyphs_to_safe_unicode() {
        let output =
            parse_rtf(r"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Wingdings 2;}}\f1 O P Q R S T\par}")
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(
            paragraph.runs[0].text,
            "\u{2717} \u{2713} \u{2612} \u{2611} \u{2612} \u{2612}"
        );
        assert_eq!(paragraph.runs[0].style.font_index, 1);
    }

    #[test]
    fn normalizes_wingdings3_basic_arrows_to_safe_unicode() {
        let output =
            parse_rtf(r"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Wingdings 3;}}\f1 f g h i\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(
            paragraph.runs[0].text,
            "\u{2190} \u{2192} \u{2191} \u{2193}"
        );
        assert_eq!(paragraph.runs[0].style.font_index, 1);
    }

    #[test]
    fn normalizes_webdings_checkbox_glyphs_to_safe_unicode() {
        let output =
            parse_rtf(r"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Webdings;}}\f1 \'3f \'61 \'63\par}")
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "\u{2612} \u{2714} \u{25a1}");
        assert_eq!(paragraph.runs[0].style.font_index, 1);
    }

    #[test]
    fn applies_stylesheet_paragraph_and_character_styles() {
        let output = parse_rtf(
            r"{\rtf1{\stylesheet{\s1\qc\li720\b Heading;}{\s2\ri360 Plain;}}\s1 Styled\par\s2 Plain\par}",
        )
        .unwrap();

        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(first.style.alignment, Alignment::Center);
        assert_eq!(first.style.left_indent_twips, 720);
        assert!(first.runs[0].style.bold);
        assert_eq!(first.runs[0].text, "Styled");

        assert_eq!(second.style.right_indent_twips, 360);
        assert!(!second.runs[0].style.bold);
        assert_eq!(second.runs[0].text, "Plain");
    }

    #[test]
    fn applies_character_styles_without_resetting_paragraph_style() {
        let output = parse_rtf(
            r"{\rtf1{\stylesheet{\cs5\qc\li1440\b Emphasis;}}\qr Right \cs5 Bold only\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.alignment, Alignment::Right);
        assert_eq!(paragraph.style.left_indent_twips, 0);
        assert_eq!(paragraph.runs[0].text, "Right ");
        assert!(!paragraph.runs[0].style.bold);
        assert_eq!(paragraph.runs[1].text, "Bold only");
        assert!(paragraph.runs[1].style.bold);
    }

    #[test]
    fn character_styles_preserve_direct_character_formatting() {
        let output = parse_rtf(
            r"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Courier New;}}{\stylesheet{\cs5\b Emphasis;}}\f1\i Direct \cs5 Direct and styled\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "Direct ");
        assert_eq!(paragraph.runs[0].style.font_index, 1);
        assert!(paragraph.runs[0].style.italic);
        assert!(!paragraph.runs[0].style.bold);
        assert_eq!(paragraph.runs[1].text, "Direct and styled");
        assert_eq!(paragraph.runs[1].style.font_index, 1);
        assert!(paragraph.runs[1].style.italic);
        assert!(paragraph.runs[1].style.bold);
    }

    #[test]
    fn applies_stylesheet_based_on_inheritance() {
        let output = parse_rtf(
            r"{\rtf1{\stylesheet{\s2\sbasedon1\i Child;}{\s1\qc\li720\b Base;}}\s2 Inherited\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.alignment, Alignment::Center);
        assert_eq!(paragraph.style.left_indent_twips, 720);
        assert!(paragraph.runs[0].style.bold);
        assert!(paragraph.runs[0].style.italic);
        assert_eq!(paragraph.runs[0].text, "Inherited");
    }

    #[test]
    fn applies_stylesheet_next_style_after_paragraph_break() {
        let output = parse_rtf(
            r"{\rtf1{\stylesheet{\s1\snext2\b Heading;}{\s2\qc\i Body;}}\s1 Heading\par Body text\par}",
        )
        .unwrap();

        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(first.runs[0].text, "Heading");
        assert!(first.runs[0].style.bold);
        assert!(!first.runs[0].style.italic);

        assert_eq!(second.runs[0].text, "Body text");
        assert_eq!(second.style.alignment, Alignment::Center);
        assert!(!second.runs[0].style.bold);
        assert!(second.runs[0].style.italic);
    }

    #[test]
    fn cyclic_stylesheet_inheritance_is_bounded() {
        let output = parse_rtf(
            r"{\rtf1{\stylesheet{\s1\sbasedon2\b First;}{\s2\sbasedon1\i Second;}}\s1 Safe\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert!(paragraph.runs[0].style.bold);
        assert_eq!(paragraph.runs[0].text, "Safe");
    }

    #[test]
    fn stylesheet_count_limit_is_enforced() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_styles: 1,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let error = parse_rtf_bytes_with_options(
            br"{\rtf1{\stylesheet{\s1 One;}{\s2 Two;}}Body\par}",
            &options,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ParseError::ResourceLimitExceeded { resource, .. } if resource == "styles"
        ));
    }

    #[test]
    fn normalizes_basic_table_rows_into_safe_table_model() {
        let output = parse_rtf(
            r"{\rtf1\trowd\cellx2000\cellx4000\intbl Name\cell Value\cell\row After\par}",
        )
        .unwrap();

        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        assert_eq!(table.column_widths_twips, vec![2000, 2000]);
        assert!(table.borders_visible);
        assert_eq!(table.rows.len(), 1);
        assert_eq!(table.rows[0].cells.len(), 2);
        assert_eq!(table.rows[0].cells[0].paragraphs[0].runs[0].text, "Name");
        assert_eq!(table.rows[0].cells[1].paragraphs[0].runs[0].text, "Value");

        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected following paragraph"),
        };
        assert_eq!(second.runs[0].text, "After");
    }

    #[test]
    fn flattens_nested_table_cells_inside_outer_cell() {
        let output = parse_rtf(
            r"{\rtf1\trowd\cellx6000 Outer before {\trowd\itap2\cellx1000 Inner A\nestcell\cellx2000 Inner B\nestrow} Outer after\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let text = table.rows[0].cells[0]
            .paragraphs
            .iter()
            .flat_map(|paragraph| paragraph.runs.iter())
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(table.rows.len(), 1);
        assert_eq!(table.rows[0].cells.len(), 1);
        assert!(text.contains("Outer before Inner A\tInner B\n"));
        assert!(text.contains("Outer after"));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_table_row_height_controls() {
        let output =
            parse_rtf(r"{\rtf1\trowd\trrh720\cellx2000 Tall\cell\row\trowd\trrh-360\cellx2000 Exact\cell\row}")
                .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].height_twips, Some(720));
        assert_eq!(table.rows[1].height_twips, Some(-360));
    }

    #[test]
    fn clamps_extreme_table_row_height_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_table_row_height_twips: 720,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\trowd\trrh9999\cellx2000 Tall\cell\row}",
            &options,
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].height_twips, Some(720));
        assert!(!output.diagnostics.is_empty());

        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\trowd\trrh-9999\cellx2000 Exact\cell\row}",
            &options,
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].height_twips, Some(-720));
    }

    #[test]
    fn normalizes_table_row_left_offset_controls() {
        let output = parse_rtf(r"{\rtf1\trowd\trleft720\cellx2000 Offset\cell\row}").unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].left_offset_twips, 720);
    }

    #[test]
    fn clamps_extreme_table_row_left_offset_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_table_row_offset_twips: 720,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\trowd\trleft-9999\cellx2000 Offset\cell\row}",
            &options,
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].left_offset_twips, -720);
        assert!(!output.diagnostics.is_empty());
    }

    #[test]
    fn normalizes_table_row_alignment_controls() {
        let output = parse_rtf(
            r"{\rtf1\trowd\trqc\cellx2000 Center\cell\row\trowd\trqr\cellx2000 Right\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].alignment, TableRowAlignment::Center);
        assert_eq!(table.rows[1].alignment, TableRowAlignment::Right);
    }

    #[test]
    fn normalizes_rtl_table_row_controls_to_visual_cell_order() {
        let output = parse_rtf(
            r"{\rtf1\trowd\taprtl\cellx1000 Right\cell\cellx3000 Left wide\cell\row\trowd\taprtl0\cellx1000 A\cell\cellx3000 B\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.column_widths_twips, vec![2000, 1000]);
        assert_eq!(
            table.rows[0].cells[0].paragraphs[0].runs[0].text,
            "Left wide"
        );
        assert_eq!(table.rows[0].cells[1].paragraphs[0].runs[0].text, "Right");
        assert_eq!(table.rows[1].cells[0].paragraphs[0].runs[0].text, "A");
        assert_eq!(table.rows[1].cells[1].paragraphs[0].runs[0].text, "B");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_rtl_table_row_horizontal_merge_groups_without_breaking_merge_markers() {
        let output = parse_rtf(
            r"{\rtf1\trowd\rtlrow\clmgf\cellx1000 Merged\cell\clmrg\cellx2500 Hidden\cell\cellx4000 Plain\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.column_widths_twips, vec![1500, 1000, 1500]);
        assert_eq!(table.rows[0].cells[0].paragraphs[0].runs[0].text, "Plain");
        assert_eq!(
            table.rows[0].cells[1].horizontal_merge,
            TableCellHorizontalMerge::First
        );
        assert_eq!(
            table.rows[0].cells[2].horizontal_merge,
            TableCellHorizontalMerge::Continuation
        );
    }

    #[test]
    fn normalizes_table_row_header_repeat_controls() {
        let output = parse_rtf(
            r"{\rtf1\trowd\trhdr\cellx2000 Header\cell\row\trowd\trhdr0\cellx2000 Body\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert!(table.rows[0].repeat_header);
        assert!(!table.rows[1].repeat_header);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_table_row_keep_together_controls() {
        let output = parse_rtf(
            r"{\rtf1\trowd\trkeep\cellx2000 Kept\cell\row\trowd\trkeep0\cellx2000 Normal\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert!(table.rows[0].keep_together);
        assert!(!table.rows[1].keep_together);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_table_cell_gap_controls() {
        let output = parse_rtf(r"{\rtf1\trowd\trgaph240\cellx2000 Gap\cell\row}").unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].cell_gap_twips, 240);
    }

    #[test]
    fn clamps_extreme_table_cell_gap_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_table_cell_gap_twips: 240,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\trowd\trgaph9999\cellx2000 Gap\cell\row}",
            &options,
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].cell_gap_twips, 240);
        assert!(!output.diagnostics.is_empty());
    }

    #[test]
    fn normalizes_table_cell_padding_controls() {
        let output = parse_rtf(
            r"{\rtf1\trowd\clpadl240\clpadr120\clpadt60\clpadb180\cellx2000 Padded\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let padding = table.rows[0].cells[0].padding;

        assert_eq!(padding.left_twips, Some(240));
        assert_eq!(padding.right_twips, Some(120));
        assert_eq!(padding.top_twips, Some(60));
        assert_eq!(padding.bottom_twips, Some(180));
    }

    #[test]
    fn normalizes_table_row_default_padding_controls() {
        let output = parse_rtf(
            r"{\rtf1\trowd\trpaddl240\trpaddr120\trpaddt60\trpaddb180\cellx2000 A\cell\cellx4000 B\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        for cell in &table.rows[0].cells {
            assert_eq!(cell.padding.left_twips, Some(240));
            assert_eq!(cell.padding.right_twips, Some(120));
            assert_eq!(cell.padding.top_twips, Some(60));
            assert_eq!(cell.padding.bottom_twips, Some(180));
        }
    }

    #[test]
    fn uses_preferred_cell_widths_when_table_cell_boundaries_are_missing() {
        let output = parse_rtf(
            r"{\rtf1\trowd\clftsWidth3\clwWidth1440 A\cell\clftsWidth3\clwWidth2880 B\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.column_widths_twips, vec![1440, 2880]);
        assert_eq!(table.rows[0].cells[0].paragraphs[0].runs[0].text, "A");
        assert_eq!(table.rows[0].cells[1].paragraphs[0].runs[0].text, "B");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn preferred_cell_widths_support_percent_units_without_overriding_cellx() {
        let output = parse_rtf(
            r"{\rtf1\paperw7200\margl720\margr720\trowd\clftsWidth2\clwWidth2500 Half\cell\clftsWidth2\clwWidth1250 Quarter\cell\row\trowd\clftsWidth3\clwWidth9999\cellx1000 Exact\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.column_widths_twips, vec![2880, 1440]);
        assert_eq!(table.rows[1].cells[0].paragraphs[0].runs[0].text, "Exact");
    }

    #[test]
    fn clamps_extreme_table_cell_padding_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_table_cell_gap_twips: 120,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\trowd\clpadl9999\clpadt-50\cellx2000 Padded\cell\row}",
            &options,
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let padding = table.rows[0].cells[0].padding;

        assert_eq!(padding.left_twips, Some(120));
        assert_eq!(padding.top_twips, Some(0));
        assert!(!output.diagnostics.is_empty());
    }

    #[test]
    fn normalizes_table_border_visibility_controls() {
        let output = parse_rtf(r"{\rtf1\trowd\brdrnone\cellx2000 Borderless\cell\row}").unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        assert!(!table.borders_visible);

        let output =
            parse_rtf(r"{\rtf1\trowd\brdrnone\brdrs\cellx2000 Bordered\cell\row}").unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        assert!(table.borders_visible);
    }

    #[test]
    fn normalizes_table_cell_side_border_controls() {
        let output = parse_rtf(
            r"{\rtf1\trowd\clbrdrl\brdrnone\clbrdrr\brdrs\clbrdrt\brdrnil\clbrdrb\brdrs\cellx2000 Cell\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let borders = table.rows[0].cells[0].borders;

        assert!(table.borders_visible);
        assert!(!borders.left.visible);
        assert!(borders.right.visible);
        assert!(!borders.top.visible);
        assert!(borders.bottom.visible);
    }

    #[test]
    fn normalizes_table_cell_diagonal_border_controls() {
        let output = parse_rtf(
            r"{\rtf1\trowd\cldgll\brdrdash\brdrw40\brdrcf1\cldglu\brdrs\brdrw30\cellx2000 Cell\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let borders = table.rows[0].cells[0].borders;

        assert!(borders.diagonal_down.visible);
        assert_eq!(borders.diagonal_down.style, BorderStyle::Dashed);
        assert_eq!(borders.diagonal_down.width_twips, 40);
        assert_eq!(borders.diagonal_down.color_index, Some(1));
        assert!(borders.diagonal_up.visible);
        assert_eq!(borders.diagonal_up.style, BorderStyle::Single);
        assert_eq!(borders.diagonal_up.width_twips, 30);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_table_cell_border_width_and_color_controls() {
        let output =
            parse_rtf(r"{\rtf1\trowd\clbrdrl\brdrs\brdrw80\brdrcf2\cellx2000 Cell\cell\row}")
                .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let border = table.rows[0].cells[0].borders.left;

        assert!(border.visible);
        assert_eq!(border.width_twips, 80);
        assert_eq!(border.color_index, Some(2));
    }

    #[test]
    fn clamps_extreme_table_cell_border_width_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_table_border_width_twips: 40,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\trowd\clbrdrl\brdrs\brdrw9999\cellx2000 Cell\cell\row}",
            &options,
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let border = table.rows[0].cells[0].borders.left;

        assert_eq!(border.width_twips, 40);
        assert!(!output.diagnostics.is_empty());
    }

    #[test]
    fn normalizes_table_cell_shading_controls() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red220\green230\blue240;}\trowd\clcbpat1\cellx2000 Shaded\cell\clcbpat0\cellx4000 Plain\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].cells[0].shading_color_index, Some(1));
        assert_eq!(table.rows[0].cells[1].shading_color_index, None);
    }

    #[test]
    fn normalizes_table_row_default_shading_controls() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red220\green230\blue240;}\trowd\trcbpat1\cellx2000 A\cell\cellx4000 B\cell\row\trowd\trcfpat1\cellx2000 C\cell\row\trowd\trcbpat0\cellx2000 Plain\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].cells[0].shading_color_index, Some(1));
        assert_eq!(table.rows[0].cells[1].shading_color_index, Some(1));
        assert_eq!(table.rows[1].cells[0].shading_color_index, Some(1));
        assert_eq!(table.rows[2].cells[0].shading_color_index, None);
    }

    #[test]
    fn normalizes_table_cell_vertical_alignment_controls() {
        let output = parse_rtf(
            r"{\rtf1\trowd\clvertalc\cellx2000 Center\cell\clvertalb\cellx4000 Bottom\cell\cellx6000 Top\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(
            table.rows[0].cells[0].vertical_align,
            TableCellVerticalAlign::Center
        );
        assert_eq!(
            table.rows[0].cells[1].vertical_align,
            TableCellVerticalAlign::Bottom
        );
        assert_eq!(
            table.rows[0].cells[2].vertical_align,
            TableCellVerticalAlign::Top
        );
    }

    #[test]
    fn normalizes_table_cell_text_direction_controls_as_passive_lines() {
        let output = parse_rtf(
            r"{\rtf1\trowd\cltxtbrlv\cellx2000 ABC\cell\cltxbtlr\cellx4000 XY\cell\cltxlrtb\cellx6000 Flat\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].cells[0].paragraphs[0].runs[0].text, "A\nB\nC");
        assert_eq!(table.rows[0].cells[1].paragraphs[0].runs[0].text, "Y\nX");
        assert_eq!(table.rows[0].cells[2].paragraphs[0].runs[0].text, "Flat");
        assert!(!document_text(&output.document).contains("cltxtbrlv"));
        assert!(!document_text(&output.document).contains("cltxbtlr"));
    }

    #[test]
    fn normalizes_table_cell_horizontal_merge_controls() {
        let output = parse_rtf(
            r"{\rtf1\trowd\clmgf\cellx2000 Merged\cell\clmrg\cellx4000 Hidden\cell\cellx6000 Plain\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(
            table.rows[0].cells[0].horizontal_merge,
            TableCellHorizontalMerge::First
        );
        assert_eq!(
            table.rows[0].cells[1].horizontal_merge,
            TableCellHorizontalMerge::Continuation
        );
        assert_eq!(
            table.rows[0].cells[2].horizontal_merge,
            TableCellHorizontalMerge::None
        );
    }

    #[test]
    fn normalizes_table_cell_vertical_merge_controls() {
        let output = parse_rtf(
            r"{\rtf1\trowd\clvmgf\cellx2000 Top\cell\cellx4000 A\cell\row\trowd\clvmrg\cellx2000 Hidden\cell\cellx4000 B\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(
            table.rows[0].cells[0].vertical_merge,
            TableCellVerticalMerge::First
        );
        assert_eq!(
            table.rows[1].cells[0].vertical_merge,
            TableCellVerticalMerge::Continuation
        );
        assert_eq!(
            table.rows[1].cells[1].vertical_merge,
            TableCellVerticalMerge::None
        );
    }

    #[test]
    fn normalizes_headers_and_footers_as_safe_paragraphs() {
        let output =
            parse_rtf(r"{\rtf1{\header Header text\par}{\footer Footer text\par}Body text\par}")
                .unwrap();

        assert_eq!(output.document.header.len(), 1);
        assert_eq!(output.document.header[0].runs[0].text, "Header text");
        assert_eq!(output.document.footer.len(), 1);
        assert_eq!(output.document.footer[0].runs[0].text, "Footer text");

        let body = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected body paragraph"),
        };
        assert_eq!(body.runs[0].text, "Body text");
    }

    #[test]
    fn normalizes_header_and_footer_variants_as_safe_paragraphs() {
        let output = parse_rtf(
            r"{\rtf1\titlepg{\headerf First header\par}{\headerl Even header\par}{\headerr Odd header\par}{\footerf First footer\par}{\footerl Even footer\par}{\footerr Odd footer\par}Body\page More\page Last\par}",
        )
        .unwrap();

        assert!(output.document.page.title_page);
        assert_eq!(output.document.first_page_header.len(), 1);
        assert_eq!(
            output.document.first_page_header[0].runs[0].text,
            "First header"
        );
        assert_eq!(output.document.even_page_header.len(), 1);
        assert_eq!(
            output.document.even_page_header[0].runs[0].text,
            "Even header"
        );
        assert_eq!(output.document.header.len(), 1);
        assert_eq!(output.document.header[0].runs[0].text, "Odd header");
        assert_eq!(output.document.first_page_footer.len(), 1);
        assert_eq!(
            output.document.first_page_footer[0].runs[0].text,
            "First footer"
        );
        assert_eq!(output.document.even_page_footer.len(), 1);
        assert_eq!(
            output.document.even_page_footer[0].runs[0].text,
            "Even footer"
        );
        assert_eq!(output.document.footer.len(), 1);
        assert_eq!(output.document.footer[0].runs[0].text, "Odd footer");
    }

    #[test]
    fn normalizes_later_section_title_page_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1 First\par\sect\sectd\titlepg Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert!(settings.title_page);
    }

    #[test]
    fn normalizes_later_section_headers_and_footers_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1{\header Document header\par}{\footer Document footer\par}First\par\sect\sectd{\header Section header\par}{\footer Section footer\par}Second\par}",
        )
        .unwrap();

        assert_eq!(output.document.header[0].runs[0].text, "Document header");
        assert_eq!(output.document.footer[0].runs[0].text, "Document footer");
        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert_eq!(settings.header[0].runs[0].text, "Section header");
        assert_eq!(settings.footer[0].runs[0].text, "Section footer");
    }

    #[test]
    fn normalizes_page_number_control_as_safe_marker() {
        let output = parse_rtf(r"{\rtf1{\header \chpgn\par}Body text\page More text\par}").unwrap();

        assert_eq!(output.document.header.len(), 1);
        assert_eq!(output.document.header[0].runs[0].text, PAGE_NUMBER_MARKER);
    }

    #[test]
    fn normalizes_page_number_start_control_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1\pgnstarts7{\header Page \chpgn\par}Body\par}").unwrap();

        assert_eq!(output.document.page.page_number_start, Some(7));
        assert_eq!(
            output.document.header[0].runs[0].text,
            format!("Page {PAGE_NUMBER_MARKER}")
        );
    }

    #[test]
    fn normalizes_page_number_format_controls_as_safe_metadata() {
        for (control, expected) in [
            ("pgndec", PageNumberFormat::Decimal),
            ("pgnucrm", PageNumberFormat::UpperRoman),
            ("pgnlcrm", PageNumberFormat::LowerRoman),
            ("pgnucltr", PageNumberFormat::UpperLetter),
            ("pgnlcltr", PageNumberFormat::LowerLetter),
        ] {
            let input = format!(r"{{\rtf1\{control}{{\header Page \chpgn\par}}Body\par}}");
            let output = parse_rtf(&input).unwrap();

            assert_eq!(output.document.page.page_number_format, Some(expected));
            assert_eq!(
                output.document.header[0].runs[0].text,
                format!("Page {PAGE_NUMBER_MARKER}")
            );
        }
    }

    #[test]
    fn normalizes_later_section_page_number_start_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1 First\par\sect\sectd\pgnstarts3 Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert_eq!(settings.page_number_start, Some(3));
    }

    #[test]
    fn normalizes_later_section_page_number_restart_and_continue_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1 First\par\sect\sectd\pgnrestart Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert_eq!(settings.page_number_start, Some(1));

        let output =
            parse_rtf(r"{\rtf1\pgnstarts7 First\par\sect\sectd\pgncont Second\par}").unwrap();
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert_eq!(settings.page_number_start, None);

        let output = parse_rtf(r"{\rtf1 First\par\sect\sectd\pgnrestart0 Second\par}").unwrap();
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert_eq!(settings.page_number_start, None);
    }

    #[test]
    fn normalizes_later_section_page_number_format_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1 First\par\sect\sectd\pgnlcltr Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert_eq!(
            settings.page_number_format,
            Some(PageNumberFormat::LowerLetter)
        );
    }

    #[test]
    fn clamps_page_number_start_control() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_page_number_start: 9,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\pgnstart999 Body\par}", &options).unwrap();

        assert_eq!(output.document.page.page_number_start, Some(9));
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("page number start clamped"))
        );
    }

    #[test]
    fn normalizes_footnotes_as_safe_paragraphs() {
        let output =
            parse_rtf(r"{\rtf1 Body\chftn{\footnote \chftn Footnote text\par}\par}").unwrap();
        let body = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected body paragraph"),
        };

        assert_eq!(
            body.runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            "Body1"
        );
        let reference = body
            .runs
            .iter()
            .find(|run| run.text == "1")
            .expect("footnote reference run");
        assert_eq!(
            reference.style.baseline_shift_half_points,
            DEFAULT_SUPERSCRIPT_SHIFT_HALF_POINTS
        );
        assert_eq!(
            reference.style.font_size_scale_percent,
            DEFAULT_SCRIPT_FONT_SCALE_PERCENT
        );
        assert_eq!(output.document.footnotes.len(), 1);
        assert_eq!(output.document.footnotes[0].runs[0].text, "Footnote text");
    }

    #[test]
    fn normalizes_endnotes_as_safe_paragraphs() {
        let output =
            parse_rtf(r"{\rtf1 Body\chftn{\endnote \chftn Endnote text\par}\par}").unwrap();
        let body = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected body paragraph"),
        };

        assert_eq!(
            body.runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            "Body1"
        );
        let reference = body
            .runs
            .iter()
            .find(|run| run.text == "1")
            .expect("endnote reference run");
        assert_eq!(
            reference.style.baseline_shift_half_points,
            DEFAULT_SUPERSCRIPT_SHIFT_HALF_POINTS
        );
        assert_eq!(output.document.endnotes.len(), 1);
        assert_eq!(output.document.endnotes[0].runs[0].text, "Endnote text");
    }

    #[test]
    fn footnote_and_endnote_references_use_separate_passive_numbering() {
        let output = parse_rtf(
            r"{\rtf1 Foot\chftn{\footnote \chftn Footnote text\par} End\chftn{\endnote \chftn Endnote text\par}\par}",
        )
        .unwrap();
        let body = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected body paragraph"),
        };

        assert_eq!(
            body.runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            "Foot1 End1"
        );
        assert_eq!(output.document.footnotes[0].runs[0].text, "Footnote text");
        assert_eq!(output.document.endnotes[0].runs[0].text, "Endnote text");
    }

    #[test]
    fn normalizes_note_numbering_controls_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1\ftnstart4\ftnnruc\aftnstart2\aftnnalc Body\chftn{\footnote \chftn Footnote text\par} End\chftn{\endnote \chftn Endnote text\par}\par}",
        )
        .unwrap();
        let body = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected body paragraph"),
        };

        assert_eq!(output.document.footnote_number_start, 4);
        assert_eq!(
            output.document.footnote_number_format,
            PageNumberFormat::UpperRoman
        );
        assert_eq!(output.document.endnote_number_start, 2);
        assert_eq!(
            output.document.endnote_number_format,
            PageNumberFormat::LowerLetter
        );
        assert_eq!(
            body.runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            "BodyIV Endb"
        );
        assert_eq!(output.document.footnotes[0].runs[0].text, "Footnote text");
        assert_eq!(output.document.endnotes[0].runs[0].text, "Endnote text");
    }

    #[test]
    fn normalizes_note_placement_controls_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1\ftnbj\aenddoc Body\chftn{\footnote Footnote text\par} End\chftn{\endnote Endnote text\par}\par}",
        )
        .unwrap();

        assert_eq!(
            output.document.footnote_placement,
            FootnotePlacement::BeneathText
        );
        assert_eq!(
            output.document.endnote_placement,
            EndnotePlacement::EndOfDocument
        );
        assert!(document_text(&output.document).contains("Body1 End1"));
        assert!(!document_text(&output.document).contains("aenddoc"));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("endnotes placed on passive final page")
        }));
    }

    #[test]
    fn note_separator_destinations_do_not_become_body_text() {
        let output = parse_rtf(
            r"{\rtf1{\ftnsep Hidden footnote separator {\object\objdata 414243}\par}{\ftnsepc Hidden footnote continuation}{\aftnsep Hidden endnote separator}{\aftnsepc Hidden endnote continuation}Body\chftn{\footnote \chftn Footnote text\par} End\chftn{\endnote \chftn Endnote text\par}\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Body1 End1"), "text: {text:?}");
        assert_eq!(output.document.footnotes[0].runs[0].text, "Footnote text");
        assert_eq!(output.document.endnotes[0].runs[0].text, "Endnote text");
        for forbidden in [
            "Hidden footnote separator",
            "Hidden footnote continuation",
            "Hidden endnote separator",
            "Hidden endnote continuation",
            "Embedded object removed",
            "ftnsep",
            "ftnsepc",
            "aftnsep",
            "aftnsepc",
            "objdata",
            "414243",
        ] {
            assert!(
                !text.contains(forbidden),
                "note separator definition leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_fields_become_passive_placeholders() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\field{\*\fldinst INCLUDEPICTURE "https://example.com/a.png"}} After\par}"#,
        )
        .unwrap();
        let text = output
            .document
            .blocks
            .iter()
            .map(|block| match block {
                Block::Paragraph(paragraph) => paragraph
                    .runs
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<String>(),
                Block::Placeholder(text) => text.clone(),
                _ => String::new(),
            })
            .collect::<String>();

        assert!(text.contains("Before"));
        assert!(text.contains("[Field removed: no passive result]"));
        assert!(text.contains("After"));
        assert!(!text.contains("INCLUDEPICTURE"));
        assert!(!text.contains("https://example.com"));
    }

    #[test]
    fn header_field_stored_result_stays_in_safe_repeating_metadata() {
        let output = parse_rtf(
            r#"{\rtf1{\header Prefix {\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt Stored link}}\par}Body\par}"#,
        )
        .unwrap();
        let header_text = output.document.header[0]
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();
        let body_text = document_text(&output.document);

        assert_eq!(header_text, "Prefix Stored link");
        assert_eq!(body_text, "Body");
        for forbidden in ["HYPERLINK", "https://example.com", "fldinst", "fldrslt"] {
            assert!(
                !header_text.contains(forbidden),
                "field instruction leaked to header result text: {forbidden}"
            );
            assert!(
                !body_text.contains(forbidden),
                "field instruction leaked to body text: {forbidden}"
            );
        }
    }

    #[test]
    fn header_resultless_external_field_placeholder_stays_in_safe_repeating_metadata() {
        let output = parse_rtf(
            r#"{\rtf1{\header Prefix {\field{\*\fldinst INCLUDEPICTURE "https://example.com/a.png"}}\par}Body\par}"#,
        )
        .unwrap();
        let header_text = output.document.header[0]
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();
        let body_text = document_text(&output.document);

        assert_eq!(header_text, "Prefix [Field removed: no passive result]");
        assert_eq!(body_text, "Body");
        assert!(!body_text.contains("[Field removed"));
        for forbidden in ["INCLUDEPICTURE", "https://example.com", "fldinst"] {
            assert!(
                !header_text.contains(forbidden),
                "field instruction leaked to header placeholder text: {forbidden}"
            );
            assert!(
                !body_text.contains(forbidden),
                "field instruction leaked to body text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_external_fields_are_classified_without_fetching() {
        let input = r#"{\rtf1 Before
{\field{\*\fldinst INCLUDEPICTURE "https://example.com/a.png"}}
{\field{\*\fldinst INCLUDETEXT "https://example.com/doc.rtf"}}
{\field{\*\fldinst LINK Word.Document.8 "https://example.com/doc.doc"}}
{\field{\*\fldinst DDEAUTO Excel "Sheet1" "R1C1"}}
{\field{\*\fldinst DATABASE \d "https://example.com/db" \s "SELECT * FROM Hidden"}}
After\par}"#;
        let output = parse_rtf(input).unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before"));
        assert!(text.contains("After"));
        assert_eq!(
            text.matches("[Field removed: no passive result]").count(),
            5
        );
        for forbidden in [
            "INCLUDEPICTURE",
            "INCLUDETEXT",
            "LINK",
            "DDEAUTO",
            "DATABASE",
            "example.com",
            "SELECT *",
        ] {
            assert!(
                !text.contains(forbidden),
                "external field leaked to text: {forbidden}"
            );
        }
        for name in [
            "INCLUDEPICTURE",
            "INCLUDETEXT",
            "LINK",
            "DDEAUTO",
            "DATABASE",
        ] {
            assert!(
                output.diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .message
                        .contains(&format!("external field {name} removed"))
                }),
                "missing diagnostic for {name}"
            );
        }
    }

    #[test]
    fn resultless_index_entry_fields_are_stripped_without_placeholders() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\field{\*\fldinst XE "Hidden index"}} middle {\field{\*\fldinst TC "Hidden toc"}} more {\field{\*\fldinst TA "Hidden authority"}} After\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before"));
        assert!(text.contains("middle"));
        assert!(text.contains("more"));
        assert!(text.contains("After"));
        assert!(!text.contains("Hidden"));
        assert!(!text.contains("XE"));
        assert!(!text.contains("TC"));
        assert!(!text.contains("TA"));
        assert!(!text.contains("[Field removed"));
        for name in ["XE", "TC", "TA"] {
            assert!(output.diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains(&format!("non-visible field {name} stripped"))
            }));
        }
    }

    #[test]
    fn resultless_page_fields_render_passive_page_marker() {
        let output = parse_rtf(
            r"{\rtf1 Page {\field{\*\fldinst PAGE}} of {\field{\*\fldinst NUMPAGES}}\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains(PAGE_NUMBER_MARKER));
        assert!(text.contains(TOTAL_PAGES_MARKER));
        assert!(!text.contains("[Field removed"));
        assert!(!text.contains("PAGE"));
        assert!(!text.contains("NUMPAGES"));
    }

    #[test]
    fn resultless_quote_fields_render_bounded_passive_literal() {
        let output =
            parse_rtf(r#"{\rtf1 Before {\field{\*\fldinst QUOTE "Visible literal"}} After\par}"#)
                .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before Visible literal After"));
        assert!(!text.contains("QUOTE"));
        assert!(!text.contains("fldinst"));
        assert!(!text.contains("[Field removed"));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("rendering passive field QUOTE without executing field instruction")
        }));
    }

    #[test]
    fn resultless_if_fields_render_passive_literal_branch() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\field{\*\fldinst IF 5 > 3 "Greater" "Lower"}} and {\field{\*\fldinst IF "Alpha" = "Beta" "Match" "Different"}} After\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before Greater and Different After"));
        for forbidden in ["IF", "Alpha", "Beta", "fldinst", "[Field removed"] {
            assert!(
                !text.contains(forbidden),
                "forbidden IF field content leaked to text: {forbidden}"
            );
        }
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("rendering passive field IF without executing field instruction")
        }));
    }

    #[test]
    fn resultless_fields_apply_passive_case_format_switches() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\field{\*\fldinst QUOTE "mixed Case" \\* Upper}} and {\field{\*\fldinst QUOTE "MIXED Case" \\* Lower \\* FirstCap}} and {\field{\*\fldinst IF 1 = 1 "checked status" "other" \\* Caps}} After\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Before MIXED CASE and Mixed case and Checked Status After"),
            "normalized field switch text was {text:?}"
        );
        for forbidden in [
            "QUOTE",
            "IF",
            "fldinst",
            "Upper",
            "Lower",
            "FirstCap",
            "Caps",
            "[Field removed",
        ] {
            assert!(
                !text.contains(forbidden),
                "forbidden field switch content leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_fields_apply_passive_number_format_switches() {
        let output = parse_rtf(
            r#"{\rtf1 Values {\field{\*\fldinst SEQ Figure \\r 4 \\* ROMAN}} {\field{\*\fldinst SEQ Figure \\* roman}} {\field{\*\fldinst = 27 \\* alphabetic}} {\field{\*\fldinst = 27 \\* ALPHABETIC}} {\field{\*\fldinst = 255 \\* Hex}} {\field{\*\fldinst IF 1 = 1 "7" "0" \\* Ordinal}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Values IV v aa AA FF 7th"),
            "normalized field number-switch text was {text:?}"
        );
        for forbidden in [
            "SEQ",
            "IF",
            "fldinst",
            "ROMAN",
            "roman",
            "alphabetic",
            "ALPHABETIC",
            "Ordinal",
            "Hex",
            "[Field removed",
        ] {
            assert!(
                !text.contains(forbidden),
                "forbidden field number-switch content leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_fields_apply_simple_passive_numeric_picture_switches() {
        let output = parse_rtf(
            r##"{\rtf1 Values {\field{\*\fldinst = 42 \\# "0000"}} {\field{\*\fldinst = 1234567 \\# "#,##0"}} {\field{\*\fldinst IF 1 = 1 "5" "0" \\# "$0.00"}} {\field{\*\fldinst = -8 \\# "000"}}\par}"##,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Values 0042 1,234,567 $5.00 -008"),
            "normalized numeric-picture text was {text:?}"
        );
        for forbidden in [
            "fldinst",
            "\\#",
            "#,##0",
            "$0.00",
            "\"0000\"",
            "[Field removed",
        ] {
            assert!(
                !text.contains(forbidden),
                "forbidden numeric-picture content leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_macrobutton_fields_render_passive_display_text() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\field{\*\fldinst MACROBUTTON LaunchPayload Visible button text}} After\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before Visible button text After"));
        assert!(!text.contains("MACROBUTTON"));
        assert!(!text.contains("LaunchPayload"));
        assert!(!text.contains("fldinst"));
        assert!(!text.contains("[Field removed"));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("rendering passive field MACROBUTTON without executing field instruction")
        }));
    }

    #[test]
    fn resultless_symbol_fields_render_bounded_passive_text() {
        let output = parse_rtf(
            r#"{\rtf1{\fonttbl{\f0 Arial;}{\f1\fcharset2 Symbol;}}Before {\field{\*\fldinst SYMBOL 183 \\f "Symbol"}} After\par}"#,
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();
        let symbol_run = paragraph
            .runs
            .iter()
            .find(|run| run.text == "\u{2022}")
            .expect("passive symbol field result");

        assert_eq!(text, "Before \u{2022} After");
        assert_eq!(symbol_run.style.font_index, 1);
        assert!(!text.contains("SYMBOL"));
        assert!(!text.contains("fldinst"));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("rendering passive field SYMBOL without executing field instruction")
        }));
    }

    #[test]
    fn date_fields_render_stored_result_without_updating() {
        let output = parse_rtf(
            r#"{\rtf1 Created {\field{\*\fldinst DATE \\@ "MMMM d, yyyy"}{\fldrslt Stored visible date}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Created Stored visible date"));
        assert!(!text.contains("DATE"));
        assert!(!text.contains("MMMM"));
        assert!(!text.contains("fldinst"));
        assert!(!text.contains("[Field removed"));
    }

    #[test]
    fn resultless_date_fields_are_not_updated_dynamically() {
        let output =
            parse_rtf(r#"{\rtf1 Before {\field{\*\fldinst DATE \\@ "yyyy"}} After\par}"#).unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before"));
        assert!(text.contains("[Field removed: no passive result]"));
        assert!(text.contains("After"));
        assert!(!text.contains("DATE"));
        assert!(!text.contains("yyyy"));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("field DATE has no stored result and was not evaluated dynamically")
        }));
    }

    #[test]
    fn resultless_document_timestamp_fields_render_from_metadata_only() {
        let output = parse_rtf(
            r#"{\rtf1{\info{\creatim\yr2024\mo7\dy5\hr14\min30\sec9}{\revtim\yr2025\mo1\dy2\hr9\min4\sec5}{\printim\yr2026\mo12\dy31}}Created {\field{\*\fldinst CREATEDATE \\@ "MMMM d, yyyy"}} saved {\field{\*\fldinst SAVEDATE \\@ "yyyy-MM-dd HH:mm:ss"}} printed {\field{\*\fldinst PRINTDATE \\@ "M/d/yy"}} missing {\field{\*\fldinst SAVEDATE \\@ "unknown-token"}} dynamic {\field{\*\fldinst DATE \\@ "yyyy"}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Created July 5, 2024"));
        assert!(text.contains("saved 2025-01-02 09:04:05"));
        assert!(text.contains("printed 12/31/26"));
        assert_eq!(
            text.matches("[Field removed: no passive result]").count(),
            2
        );
        for forbidden in [
            "CREATEDATE",
            "SAVEDATE",
            "PRINTDATE",
            "DATE",
            "MMMM",
            "yyyy",
            "unknown-token",
            "creatim",
            "revtim",
            "printim",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "timestamp field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_edit_time_fields_render_from_metadata_only() {
        let output = parse_rtf(
            r#"{\rtf1{\info{\edmins42}}Edit {\field{\*\fldinst EDITTIME \\* ROMAN}} raw {\field{\*\fldinst EDITTIME \\# "0000"}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Edit XLII raw 0042"));
        for forbidden in ["EDITTIME", "edmins", "fldinst", "[Field removed"] {
            assert!(
                !text.contains(forbidden),
                "edit time field leaked unsafe text: {forbidden}"
            );
        }

        let missing = parse_rtf(r"{\rtf1 Missing {\field{\*\fldinst EDITTIME}}\par}").unwrap();
        let missing_text = document_text(&missing.document);
        assert!(missing_text.contains("Missing [Field removed: no passive result]"));
        assert!(!missing_text.contains("EDITTIME"));
    }

    #[test]
    fn edit_time_metadata_obeys_output_bounds() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_output_text_chars: 3,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        assert!(matches!(
            parse_rtf_bytes_with_options(br"{\rtf1{\info{\edmins42}}\par}", &options),
            Err(ParseError::ResourceLimitExceeded { resource, .. })
                if resource == "document edit minutes"
        ));
    }

    #[test]
    fn dynamic_date_time_controls_follow_policy_without_evaluating() {
        let input = br"{\rtf1 Before \chdate / \chtime / \chdpa / \chdpl After\par}";
        let output = parse_rtf_bytes_with_options(input, &RtfParseOptions::default()).unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before"));
        assert!(text.contains("[Dynamic date/time removed]"));
        assert!(text.contains("After"));
        assert!(!text.contains("chdate"));
        assert!(!text.contains("chtime"));
        assert!(!text.contains("chdpa"));
        assert!(!text.contains("chdpl"));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("dynamic date/time control chdate placeholdered")
        }));

        let strip_options = RtfParseOptions {
            active_content_policy: ActiveContentPolicy::Strip,
            ..RtfParseOptions::default()
        };
        let stripped = parse_rtf_bytes_with_options(input, &strip_options).unwrap();
        let stripped_text = document_text(&stripped.document);
        assert!(stripped_text.contains("Before"));
        assert!(stripped_text.contains("After"));
        assert!(!stripped_text.contains("[Dynamic date/time removed]"));
        assert!(stripped.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("dynamic date/time control chdate removed")
        }));

        let reject_options = RtfParseOptions {
            active_content_policy: ActiveContentPolicy::Reject,
            ..RtfParseOptions::default()
        };
        assert!(matches!(
            parse_rtf_bytes_with_options(input, &reject_options),
            Err(ParseError::ActiveContentRejected { feature, .. })
                if feature == "dynamic date/time control"
        ));
    }

    #[test]
    fn section_number_controls_render_passive_section_marker() {
        let output =
            parse_rtf(r"{\rtf1 Section \sectnum\par\sbknone\sect Section \sectnum\par}").unwrap();
        let text = document_text(&output.document);

        assert!(text.contains(SECTION_NUMBER_MARKER));
        assert!(!text.contains("sectnum"));
        assert!(
            output
                .document
                .blocks
                .iter()
                .any(|block| matches!(block, Block::ContinuousSectionBreak))
        );
    }

    #[test]
    fn resultless_section_fields_render_passive_section_marker() {
        let output = parse_rtf(r"{\rtf1 Section {\field{\*\fldinst SECTION}}\par}").unwrap();
        let text = document_text(&output.document);

        assert!(text.contains(SECTION_NUMBER_MARKER));
        assert!(!text.contains("[Field removed"));
        assert!(!text.contains("SECTION"));
        assert!(!text.contains("fldinst"));
    }

    #[test]
    fn field_instruction_growth_is_bounded() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_text_run_len: 4,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        assert!(matches!(
            parse_rtf_bytes_with_options(
                br"{\rtf1{\field{\*\fldinst PAG\~EL}}}",
                &options,
            ),
            Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "field instruction"
        ));
    }

    #[test]
    fn prefixes_plain_pntext_marker_to_following_paragraph() {
        let output = parse_rtf(r"{\rtf1{\pntext 1.\tab}First item\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected list paragraph"),
        };
        assert_eq!(paragraph.runs[0].text, "1.\tFirst item");
    }

    #[test]
    fn prefixes_old_style_pn_marker_text_to_following_paragraph() {
        let output =
            parse_rtf(r"{\rtf1{\pn\pndec\pnstart3{\pntxtb 3}{\pntxta .\tab}}Third item\par}")
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected old-style list paragraph"),
        };
        assert_eq!(paragraph.runs[0].text, "3.\tThird item");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn synthesizes_old_style_roman_marker_without_explicit_marker_text() {
        let output = parse_rtf(r"{\rtf1{\pn\pnucrm\pnstart4}Fourth item\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected old-style list paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "IV.\tFourth item");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn synthesizes_old_style_alpha_ordinal_and_bullet_markers() {
        let output = parse_rtf(
            r"{\rtf1{\pn\pnlcltr\pnstart28}Lower alpha\par{\pn\pnord\pnstart13}Ordinal\par{\pn\pnbul}Bullet\par}",
        )
        .unwrap();
        let paragraph_text = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph.runs[0].text.as_str(),
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph_text(0), "ab.\tLower alpha");
        assert_eq!(paragraph_text(1), "13th.\tOrdinal");
        assert_eq!(paragraph_text(2), "\u{2022}\tBullet");
    }

    #[test]
    fn applies_old_style_list_indent_controls_to_marker_paragraph() {
        let output =
            parse_rtf(r"{\rtf1{\pn\pndec\pnstart2\pnindent720\pnhang}Indented\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected old-style list paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "2.\tIndented");
        assert_eq!(paragraph.style.left_indent_twips, 720);
        assert_eq!(paragraph.style.first_line_indent_twips, -360);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn applies_old_style_list_spacing_control_as_safe_tab_stop() {
        let output =
            parse_rtf(r"{\rtf1{\pn\pndec\pnstart2\pnindent720\pnhang\pnsp360}Spaced\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected old-style list paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "2.\tSpaced");
        assert_eq!(paragraph.style.left_indent_twips, 720);
        assert_eq!(paragraph.style.first_line_indent_twips, -360);
        assert_eq!(paragraph.style.tab_stops_twips, vec![360]);
        assert_eq!(paragraph.style.tab_stop_leaders, vec![TabLeader::None]);
        assert_eq!(
            paragraph.style.tab_stop_alignments,
            vec![TabAlignment::Left]
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn old_style_list_spacing_obeys_tab_stop_limit() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_tab_stops: 0,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        assert!(matches!(
            parse_rtf_bytes_with_options(br"{\rtf1{\pn\pndec\pnsp360}Blocked\par}", &options),
            Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "tab stops"
        ));
    }

    #[test]
    fn applies_old_style_list_marker_character_format_controls_to_marker_run() {
        let output = parse_rtf(
            r"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Courier New;}}{\colortbl;\red255\green0\blue0;}{\pn\pndec\pnb\pni\pnul\pnstrike\pncaps\pncf1\pnf1\pnfs28}Formatted item\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected old-style list paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "1.\t");
        assert!(paragraph.runs[0].style.bold);
        assert!(paragraph.runs[0].style.italic);
        assert_eq!(paragraph.runs[0].style.underline, UnderlineStyle::Single);
        assert!(paragraph.runs[0].style.strike);
        assert!(paragraph.runs[0].style.all_caps);
        assert_eq!(paragraph.runs[0].style.color_index, 1);
        assert_eq!(paragraph.runs[0].style.font_index, 1);
        assert_eq!(paragraph.runs[0].style.font_size_half_points, 28);
        assert_eq!(paragraph.runs[1].text, "Formatted item");
        assert!(!paragraph.runs[1].style.bold);
        assert!(!paragraph.runs[1].style.italic);
        assert_eq!(paragraph.runs[1].style.underline, UnderlineStyle::None);
        assert!(!paragraph.runs[1].style.strike);
        assert_eq!(paragraph.runs[1].style.color_index, 0);
        assert_eq!(paragraph.runs[1].style.font_index, 0);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn prefixes_ignorable_listtext_marker_to_following_paragraph() {
        let output = parse_rtf(r"{\rtf1{\*\listtext \u8226?\tab}Bullet item\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected list paragraph"),
        };
        assert_eq!(paragraph.runs[0].text, "\u{2022}\tBullet item");
    }

    #[test]
    fn preserves_formatted_explicit_listtext_marker_style() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;}{\*\listtext\b\cf1 1.\tab}Styled explicit\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected list paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "1.\t");
        assert!(paragraph.runs[0].style.bold);
        assert_eq!(paragraph.runs[0].style.color_index, 1);
        assert_eq!(paragraph.runs[1].text, "Styled explicit");
        assert!(!paragraph.runs[1].style.bold);
        assert_eq!(paragraph.runs[1].style.color_index, 0);
    }

    #[test]
    fn preserves_mixed_formatted_explicit_listtext_marker_runs() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;}{\*\listtext\b 1\b0\cf1 .\cf0\tab}Styled explicit\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected list paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "1");
        assert!(paragraph.runs[0].style.bold);
        assert_eq!(paragraph.runs[0].style.color_index, 0);
        assert_eq!(paragraph.runs[1].text, ".");
        assert!(!paragraph.runs[1].style.bold);
        assert_eq!(paragraph.runs[1].style.color_index, 1);
        assert_eq!(paragraph.runs[2].text, "\tStyled explicit");
        assert!(!paragraph.runs[2].style.bold);
        assert_eq!(paragraph.runs[2].style.color_index, 0);
    }

    #[test]
    fn synthesizes_decimal_markers_from_list_tables() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 First\par\pard\ls1\ilvl0 Second\par}",
        )
        .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(first.runs[0].text, "1.\tFirst");
        assert_eq!(second.runs[0].text, "2.\tSecond");
    }

    #[test]
    fn applies_list_level_indent_and_spacing_as_safe_paragraph_metadata() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\levelindent720\levelspace1080{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Indented\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "1.\tIndented");
        assert_eq!(paragraph.style.left_indent_twips, 720);
        assert_eq!(paragraph.style.tab_stops_twips, vec![1080]);
        assert_eq!(paragraph.style.tab_stop_leaders, vec![TabLeader::None]);
        assert_eq!(
            paragraph.style.tab_stop_alignments,
            vec![TabAlignment::Left]
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn applies_list_level_follow_controls_to_synthesized_marker_suffix() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelfollow0{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid1}{\list{\listlevel\levelnfc0\levelfollow1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid2}{\list{\listlevel\levelnfc0\levelfollow2{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid3}}{\*\listoverridetable{\listoverride\listid1\ls1}{\listoverride\listid2\ls2}{\listoverride\listid3\ls3}}\pard\ls1\ilvl0 Tab\par\pard\ls2\ilvl0 Space\par\pard\ls3\ilvl0 Nothing\par}",
        )
        .unwrap();
        let paragraph_text = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph.runs[0].text.as_str(),
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph_text(0), "1.\tTab");
        assert_eq!(paragraph_text(1), "1. Space");
        assert_eq!(paragraph_text(2), "1.Nothing");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn applies_list_level_marker_character_formatting_to_marker_run() {
        let output = parse_rtf(
            r"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Courier New;}}{\colortbl;\red255\green0\blue0;\red255\green255\blue0;\red0\green0\blue255;}{\*\listtable{\list{\listlevel\levelnfc0\f1\fs28\b\i\ul\ulc3\strike\caps\cf1\highlight2\chshdng5000{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Styled item\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "1.\t");
        assert!(paragraph.runs[0].style.bold);
        assert!(paragraph.runs[0].style.italic);
        assert_eq!(paragraph.runs[0].style.underline, UnderlineStyle::Single);
        assert!(paragraph.runs[0].style.strike);
        assert!(paragraph.runs[0].style.all_caps);
        assert_eq!(paragraph.runs[0].style.font_index, 1);
        assert_eq!(paragraph.runs[0].style.font_size_half_points, 28);
        assert_eq!(paragraph.runs[0].style.color_index, 1);
        assert_eq!(paragraph.runs[0].style.highlight_index, Some(2));
        assert_eq!(
            paragraph.runs[0].style.highlight_shading_basis_points,
            5_000
        );
        assert_eq!(paragraph.runs[0].style.underline_color_index, Some(3));
        assert_eq!(paragraph.runs[1].text, "Styled item");
        assert!(!paragraph.runs[1].style.bold);
        assert!(!paragraph.runs[1].style.italic);
        assert_eq!(paragraph.runs[1].style.underline, UnderlineStyle::None);
        assert!(!paragraph.runs[1].style.strike);
        assert_eq!(paragraph.runs[1].style.font_index, 0);
        assert_eq!(paragraph.runs[1].style.color_index, 0);
        assert_eq!(paragraph.runs[1].style.highlight_index, None);
        assert_eq!(paragraph.runs[1].style.underline_color_index, None);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn applies_list_level_marker_double_strike_to_marker_run_only() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\strikedl{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Double strike item\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "1.\t");
        assert!(paragraph.runs[0].style.strike);
        assert!(paragraph.runs[0].style.double_strike);
        assert_eq!(paragraph.runs[1].text, "Double strike item");
        assert!(!paragraph.runs[1].style.strike);
        assert!(!paragraph.runs[1].style.double_strike);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn applies_list_level_marker_underline_variants_to_marker_runs_only() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\uldb{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}{\list{\listlevel\levelnfc0\ulth{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid6}{\list{\listlevel\levelnfc0\uld{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid7}{\list{\listlevel\levelnfc0\uldash{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid8}{\list{\listlevel\levelnfc0\ulwave{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid9}{\list{\listlevel\levelnfc0\ulw{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid10}}{\*\listoverridetable{\listoverride\listid5\ls1}{\listoverride\listid6\ls2}{\listoverride\listid7\ls3}{\listoverride\listid8\ls4}{\listoverride\listid9\ls5}{\listoverride\listid10\ls6}}\pard\ls1\ilvl0 Double marker\par\pard\ls2\ilvl0 Thick marker\par\pard\ls3\ilvl0 Dotted marker\par\pard\ls4\ilvl0 Dashed marker\par\pard\ls5\ilvl0 Wave marker\par\pard\ls6\ilvl0 Words marker\par}",
        )
        .unwrap();

        for (index, (text, underline)) in [
            ("Double marker", UnderlineStyle::Double),
            ("Thick marker", UnderlineStyle::Thick),
            ("Dotted marker", UnderlineStyle::Dotted),
            ("Dashed marker", UnderlineStyle::Dashed),
            ("Wave marker", UnderlineStyle::Wave),
            ("Words marker", UnderlineStyle::Words),
        ]
        .into_iter()
        .enumerate()
        {
            let paragraph = match &output.document.blocks[index] {
                Block::Paragraph(paragraph) => paragraph,
                _ => panic!("expected paragraph"),
            };

            assert_eq!(paragraph.runs[0].text, "1.\t");
            assert_eq!(paragraph.runs[0].style.underline, underline);
            assert_eq!(paragraph.runs[1].text, text);
            assert_eq!(paragraph.runs[1].style.underline, UnderlineStyle::None);
        }

        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn applies_list_level_marker_text_effects_to_marker_runs_only() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\outl{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}{\list{\listlevel\levelnfc0\shad{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid6}{\list{\listlevel\levelnfc0\embo{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid7}{\list{\listlevel\levelnfc0\impr{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid8}{\list{\listlevel\levelnfc0\scaps{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid9}}{\*\listoverridetable{\listoverride\listid5\ls1}{\listoverride\listid6\ls2}{\listoverride\listid7\ls3}{\listoverride\listid8\ls4}{\listoverride\listid9\ls5}}\pard\ls1\ilvl0 Outline marker\par\pard\ls2\ilvl0 Shadow marker\par\pard\ls3\ilvl0 Emboss marker\par\pard\ls4\ilvl0 Engrave marker\par\pard\ls5\ilvl0 Small caps marker\par}",
        )
        .unwrap();

        let marker_style = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => {
                assert_eq!(paragraph.runs[0].text, "1.\t");
                assert_eq!(paragraph.runs[1].style, CharacterStyle::default());
                &paragraph.runs[0].style
            }
            _ => panic!("expected paragraph"),
        };

        assert!(marker_style(0).outline);
        assert!(marker_style(1).shadow);
        assert_eq!(marker_style(2).relief, TextRelief::Emboss);
        assert_eq!(marker_style(3).relief, TextRelief::Engrave);
        assert!(marker_style(4).small_caps);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn applies_list_level_marker_script_and_spacing_to_marker_runs_only() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\super{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}{\list{\listlevel\levelnfc0\sub{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid6}{\list{\listlevel\levelnfc0\up8{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid7}{\list{\listlevel\levelnfc0\dn6{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid8}{\list{\listlevel\levelnfc0\expndtw80{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid9}{\list{\listlevel\levelnfc0\kerning2{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid10}{\list{\listlevel\levelnfc0\charscalex150{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid11}}{\*\listoverridetable{\listoverride\listid5\ls1}{\listoverride\listid6\ls2}{\listoverride\listid7\ls3}{\listoverride\listid8\ls4}{\listoverride\listid9\ls5}{\listoverride\listid10\ls6}{\listoverride\listid11\ls7}}\pard\ls1\ilvl0 Raised marker\par\pard\ls2\ilvl0 Lowered marker\par\pard\ls3\ilvl0 Manual up marker\par\pard\ls4\ilvl0 Manual down marker\par\pard\ls5\ilvl0 Spaced marker\par\pard\ls6\ilvl0 Kerned marker\par\pard\ls7\ilvl0 Scaled marker\par}",
        )
        .unwrap();

        let marker_style = |index: usize, text: &str| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => {
                assert_eq!(paragraph.runs[0].text, "1.\t");
                assert_eq!(paragraph.runs[1].text, text);
                assert_eq!(paragraph.runs[1].style, CharacterStyle::default());
                &paragraph.runs[0].style
            }
            _ => panic!("expected paragraph"),
        };

        let raised = marker_style(0, "Raised marker");
        assert_eq!(
            raised.baseline_shift_half_points,
            DEFAULT_SUPERSCRIPT_SHIFT_HALF_POINTS
        );
        assert_eq!(
            raised.font_size_scale_percent,
            DEFAULT_SCRIPT_FONT_SCALE_PERCENT
        );
        let lowered = marker_style(1, "Lowered marker");
        assert_eq!(
            lowered.baseline_shift_half_points,
            DEFAULT_SUBSCRIPT_SHIFT_HALF_POINTS
        );
        assert_eq!(
            lowered.font_size_scale_percent,
            DEFAULT_SCRIPT_FONT_SCALE_PERCENT
        );
        assert_eq!(
            marker_style(2, "Manual up marker").baseline_shift_half_points,
            8
        );
        assert_eq!(
            marker_style(3, "Manual down marker").baseline_shift_half_points,
            -6
        );
        assert_eq!(marker_style(4, "Spaced marker").character_spacing_twips, 80);
        assert_eq!(
            marker_style(5, "Kerned marker").character_kerning_half_points,
            2
        );
        assert_eq!(
            marker_style(6, "Scaled marker").character_scaling_percent,
            150
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn applies_list_level_marker_character_border_to_marker_run_only() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;}{\*\listtable{\list{\listlevel\levelnfc0\chbrdr\brdrdash\brdrw80\brdrcf1\brsp120{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Bordered marker\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "1.\t");
        assert!(paragraph.runs[0].style.border.visible);
        assert_eq!(paragraph.runs[0].style.border.style, BorderStyle::Dashed);
        assert_eq!(paragraph.runs[0].style.border.width_twips, 80);
        assert_eq!(paragraph.runs[0].style.border.color_index, Some(1));
        assert_eq!(paragraph.runs[0].style.border.spacing_twips, 120);
        assert_eq!(paragraph.runs[1].text, "Bordered marker");
        assert!(!paragraph.runs[1].style.border.visible);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn list_level_marker_plain_resets_accumulated_marker_character_style() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;}{\*\listtable{\list{\listlevel\levelnfc0\b\ul\chbrdr\brdrs\brdrw80\plain\i\cf1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Reset item\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "1.\t");
        assert!(!paragraph.runs[0].style.bold);
        assert!(paragraph.runs[0].style.italic);
        assert_eq!(paragraph.runs[0].style.underline, UnderlineStyle::None);
        assert_eq!(paragraph.runs[0].style.color_index, 1);
        assert!(!paragraph.runs[0].style.border.visible);
        assert_eq!(paragraph.runs[1].text, "Reset item");
        assert!(!paragraph.runs[1].style.bold);
        assert!(!paragraph.runs[1].style.italic);
        assert_eq!(paragraph.runs[1].style.color_index, 0);
        assert!(!paragraph.runs[1].style.border.visible);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn list_level_marker_character_style_applies_to_marker_run_only() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;}{\stylesheet{\cs5\b\ul\cf1 Marker emphasis;}}{\*\listtable{\list{\listlevel\levelnfc0\i\cs5{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Styled marker\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "1.\t");
        assert!(paragraph.runs[0].style.bold);
        assert!(paragraph.runs[0].style.italic);
        assert_eq!(paragraph.runs[0].style.underline, UnderlineStyle::Single);
        assert_eq!(paragraph.runs[0].style.color_index, 1);
        assert_eq!(paragraph.runs[1].text, "Styled marker");
        assert!(!paragraph.runs[1].style.bold);
        assert!(!paragraph.runs[1].style.italic);
        assert_eq!(paragraph.runs[1].style.underline, UnderlineStyle::None);
        assert_eq!(paragraph.runs[1].style.color_index, 0);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn list_level_spacing_obeys_tab_stop_limit() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_tab_stops: 0,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        assert!(matches!(
            parse_rtf_bytes_with_options(
                br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelspace1080{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Blocked\par}",
                &options
            ),
            Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "tab stops"
        ));
    }

    #[test]
    fn list_override_start_values_restart_synthesized_markers() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5{\lfolevel\listoverridestartat\levelstartat3}\ls1}}\pard\ls1\ilvl0 First\par\pard\ls1\ilvl0 Second\par}",
        )
        .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(first.runs[0].text, "3.\tFirst");
        assert_eq!(second.runs[0].text, "4.\tSecond");
    }

    #[test]
    fn list_override_format_replaces_synthesized_marker_definition() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5{\lfolevel\listoverrideformat{\listlevel\levelnfc4\levelstartat3{\leveltext\'02\'00);}{\levelnumbers\'01;}}}\ls1}}\pard\ls1\ilvl0 First\par\pard\ls1\ilvl0 Second\par}",
        )
        .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(first.runs[0].text, "c)\tFirst");
        assert_eq!(second.runs[0].text, "d)\tSecond");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn list_override_format_applies_marker_character_style_only() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;}{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5{\lfolevel\listoverrideformat{\listlevel\levelnfc4\b\cf1\levelstartat3{\leveltext\'02\'00);}{\levelnumbers\'01;}}}\ls1}}\pard\ls1\ilvl0 Styled override\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "c)\t");
        assert!(paragraph.runs[0].style.bold);
        assert_eq!(paragraph.runs[0].style.color_index, 1);
        assert_eq!(paragraph.runs[1].text, "Styled override");
        assert!(!paragraph.runs[1].style.bold);
        assert_eq!(paragraph.runs[1].style.color_index, 0);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn synthesizes_multilevel_markers_from_leveltext_placeholders() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}{\listlevel\levelnfc0\levelstartat1{\leveltext\'04\'00.\'01.;}{\levelnumbers\'01\'03;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Top\par\pard\ls1\ilvl1 Child\par\pard\ls1\ilvl1 Child two\par\pard\ls1\ilvl0 Next top\par\pard\ls1\ilvl1 Next child\par}",
        )
        .unwrap();
        let paragraph_text = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph.runs[0].text.as_str(),
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph_text(0), "1.\tTop");
        assert_eq!(paragraph_text(1), "1.1.\tChild");
        assert_eq!(paragraph_text(2), "1.2.\tChild two");
        assert_eq!(paragraph_text(3), "2.\tNext top");
        assert_eq!(paragraph_text(4), "2.1.\tNext child");
    }

    #[test]
    fn list_level_legal_numbering_renders_parent_placeholders_as_decimal() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc1\levelstartat4{\leveltext\'02\'00.;}{\levelnumbers\'01;}}{\listlevel\levelnfc0\levellegal1\levelstartat1{\leveltext\'04\'00.\'01.;}{\levelnumbers\'01\'03;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Parent\par\pard\ls1\ilvl1 Child\par}",
        )
        .unwrap();
        let paragraph_text = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph.runs[0].text.as_str(),
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph_text(0), "IV.\tParent");
        assert_eq!(paragraph_text(1), "4.1.\tChild");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn list_level_no_restart_preserves_lower_level_counter_across_parent_items() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}{\listlevel\levelnfc0\levelnorestart1\levelstartat1{\leveltext\'04\'00.\'01.;}{\levelnumbers\'01\'03;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Top\par\pard\ls1\ilvl1 Child\par\pard\ls1\ilvl0 Next top\par\pard\ls1\ilvl1 Continued child\par}",
        )
        .unwrap();
        let paragraph_text = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph.runs[0].text.as_str(),
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph_text(0), "1.\tTop");
        assert_eq!(paragraph_text(1), "1.1.\tChild");
        assert_eq!(paragraph_text(2), "2.\tNext top");
        assert_eq!(paragraph_text(3), "2.2.\tContinued child");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn synthesizes_roman_and_alpha_markers_from_list_tables() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc1\levelstartat4{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid10}{\list{\listlevel\levelnfc2\levelstartat9{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid11}{\list{\listlevel\levelnfc3\levelstartat27{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid12}{\list{\listlevel\levelnfc4\levelstartat28{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid13}}{\*\listoverridetable{\listoverride\listid10\ls10}{\listoverride\listid11\ls11}{\listoverride\listid12\ls12}{\listoverride\listid13\ls13}}\pard\ls10\ilvl0 Upper roman\par\pard\ls10\ilvl0 Upper roman next\par\pard\ls11\ilvl0 Lower roman\par\pard\ls12\ilvl0 Upper alpha\par\pard\ls13\ilvl0 Lower alpha\par}",
        )
        .unwrap();
        let paragraph_text = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph.runs[0].text.as_str(),
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph_text(0), "IV.\tUpper roman");
        assert_eq!(paragraph_text(1), "V.\tUpper roman next");
        assert_eq!(paragraph_text(2), "ix.\tLower roman");
        assert_eq!(paragraph_text(3), "AA.\tUpper alpha");
        assert_eq!(paragraph_text(4), "ab.\tLower alpha");
    }

    #[test]
    fn synthesizes_ordinal_markers_from_list_tables() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc5\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid14}}{\*\listoverridetable{\listoverride\listid14\ls14}}\pard\ls14\ilvl0 First\par\pard\ls14\ilvl0 Second\par\pard\ls14\ilvl0 Third\par\pard\ls14\ilvl0 Fourth\par}",
        )
        .unwrap();
        let paragraph_text = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph.runs[0].text.as_str(),
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph_text(0), "1st.\tFirst");
        assert_eq!(paragraph_text(1), "2nd.\tSecond");
        assert_eq!(paragraph_text(2), "3rd.\tThird");
        assert_eq!(paragraph_text(3), "4th.\tFourth");
    }

    #[test]
    fn ordinal_list_markers_handle_teen_suffix_exceptions() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc5\levelstartat11{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid15}}{\*\listoverridetable{\listoverride\listid15\ls15}}\pard\ls15\ilvl0 Eleventh\par\pard\ls15\ilvl0 Twelfth\par\pard\ls15\ilvl0 Thirteenth\par\pard\ls15\ilvl0 Fourteenth\par}",
        )
        .unwrap();
        let paragraph_text = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph.runs[0].text.as_str(),
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph_text(0), "11th.\tEleventh");
        assert_eq!(paragraph_text(1), "12th.\tTwelfth");
        assert_eq!(paragraph_text(2), "13th.\tThirteenth");
        assert_eq!(paragraph_text(3), "14th.\tFourteenth");
    }

    #[test]
    fn synthesizes_zero_padded_decimal_markers_from_list_tables() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc22\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid16}{\list{\listlevel\levelnfc62\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid17}{\list{\listlevel\levelnfc63\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid18}{\list{\listlevel\levelnfc64\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid19}}{\*\listoverridetable{\listoverride\listid16\ls16}{\listoverride\listid17\ls17}{\listoverride\listid18\ls18}{\listoverride\listid19\ls19}}\pard\ls16\ilvl0 Two digits\par\pard\ls17\ilvl0 Three digits\par\pard\ls18\ilvl0 Four digits\par\pard\ls19\ilvl0 Five digits\par}",
        )
        .unwrap();
        let paragraph_text = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph.runs[0].text.as_str(),
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph_text(0), "01.\tTwo digits");
        assert_eq!(paragraph_text(1), "001.\tThree digits");
        assert_eq!(paragraph_text(2), "0001.\tFour digits");
        assert_eq!(paragraph_text(3), "00001.\tFive digits");
    }

    #[test]
    fn oversized_roman_list_markers_fall_back_to_bounded_decimal() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc1\levelstartat5000{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid20}}{\*\listoverridetable{\listoverride\listid20\ls20}}\pard\ls20\ilvl0 Big roman\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "5000.\tBig roman");
    }

    #[test]
    fn synthesizes_bullet_markers_from_list_tables() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc23{\leveltext\'01\u8226 ?;}{\levelnumbers;}}\listid7}}{\*\listoverridetable{\listoverride\listid7\ls2}}\pard\ls2\ilvl0 Bullet\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "\u{2022}\tBullet");
    }

    #[test]
    fn list_definition_count_limit_is_enforced() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_styles: 1,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let error = parse_rtf_bytes_with_options(
            br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0{\leveltext\'02\'00.;}}\listid1}{\list{\listlevel\levelnfc0{\leveltext\'02\'00.;}}\listid2}}Body\par}",
            &options,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ParseError::ResourceLimitExceeded { resource, .. } if resource == "lists"
        ));
    }

    #[test]
    fn unicode_in_ignored_destinations_does_not_become_body_text() {
        let output = parse_rtf(r"{\rtf1{\*\unknown \u8226? hidden}Visible\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "Visible");
    }

    #[test]
    fn unknown_destination_groups_do_not_become_body_text() {
        let output =
            parse_rtf(r"{\rtf1 Visible {\unknown Hidden {\object\objdata 414243}} after\par}")
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(text, "Visible  after");
        assert!(!text.contains("Hidden"));
        assert!(!text.contains("objdata"));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("unknown RTF destination '\\unknown' skipped")
        }));
    }

    #[test]
    fn unknown_destination_groups_consume_skip_budget() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_destination_bytes: 4,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        assert!(matches!(
            parse_rtf_bytes_with_options(br"{\rtf1{\unknown abcde} visible\par}", &options),
            Err(ParseError::DestinationTooLarge(_))
        ));
    }

    #[test]
    fn unsupported_inline_controls_do_not_swallow_visible_text() {
        let output = parse_rtf(r"{\rtf1 Visible \unknown inline text\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(text, "Visible inline text");
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("unsupported RTF control '\\unknown'")
        }));
    }

    #[test]
    fn visible_controls_in_non_visual_destinations_do_not_become_body_text() {
        let output = parse_rtf(
            r"{\rtf1{\fonttbl{\f0\tab Arial;}}{\colortbl;\tab\red1\green2\blue3;}{\pict\pngblip\tab 00}Visible\par}",
        )
        .unwrap();
        let paragraph_text = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph) => Some(
                    paragraph
                        .runs
                        .iter()
                        .map(|run| run.text.as_str())
                        .collect::<String>(),
                ),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(paragraph_text, vec!["Visible"]);
    }

    #[test]
    fn metadata_destinations_do_not_become_body_text() {
        let output = parse_rtf(
            r"{\rtf1{\info{\title Hidden title}{\author Hidden author}{\doccomm Hidden comment}}{\template C:\remote\template.dotm}Visible\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Visible"));
        assert!(!text.contains("Hidden title"));
        assert!(!text.contains("Hidden author"));
        assert!(!text.contains("Hidden comment"));
        assert!(!text.contains("template.dotm"));
    }

    #[test]
    fn resultless_docproperty_fields_render_safe_metadata_values() {
        let output = parse_rtf(
            r#"{\rtf1{\info{\title Visible \u937? Title}{\author Alice}{\operator Bob}{\doccomm Hidden comment}}Title: {\field{\*\fldinst DOCPROPERTY Title}} by {\field{\*\fldinst DOCPROPERTY Author \\* Upper}} saved by {\field{\*\fldinst DOCPROPERTY LastSavedBy}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Title: Visible \u{3a9} Title by ALICE saved by Bob"));
        for forbidden in ["DOCPROPERTY", "fldinst", "Hidden comment"] {
            assert!(
                !text.contains(forbidden),
                "document property field leaked unsafe text: {forbidden}"
            );
        }
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("rendering passive field DOCPROPERTY without executing field instruction")
        }));
    }

    #[test]
    fn resultless_shortcut_document_property_fields_render_safe_metadata_values() {
        let output = parse_rtf(
            r#"{\rtf1{\info{\title Visible Title}{\subject Visible Subject}{\author Alice}{\keywords alpha beta}{\operator Bob}{\doccomm Hidden comment}}Doc {\field{\*\fldinst TITLE}} by {\field{\*\fldinst AUTHOR \\* Upper}} about {\field{\*\fldinst SUBJECT}} tags {\field{\*\fldinst KEYWORDS}} saved {\field{\*\fldinst LASTSAVEDBY}} note {\field{\*\fldinst COMMENTS}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains(
            "Doc Visible Title by ALICE about Visible Subject tags alpha beta saved Bob note Hidden comment"
        ));
        for forbidden in [
            "TITLE",
            "AUTHOR",
            "SUBJECT",
            "KEYWORDS",
            "LASTSAVEDBY",
            "COMMENTS",
            "fldinst",
            "[Field removed",
        ] {
            assert!(
                !text.contains(forbidden),
                "shortcut document property field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_info_fields_render_safe_metadata_values() {
        let output = parse_rtf(
            r#"{\rtf1{\info{\title Info Title}{\author Alice}{\doccomm Comment text}}Doc {\field{\*\fldinst INFO Title}} by {\field{\*\fldinst INFO Author \\* Upper}} note {\field{\*\fldinst INFO Comments}} file {\field{\*\fldinst INFO Filename}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Doc Info Title by ALICE note Comment text file"));
        assert_eq!(
            text.matches("[Field removed: no passive result]").count(),
            1
        );
        for forbidden in ["fldinst", "Filename"] {
            assert!(
                !text.contains(forbidden),
                "INFO field leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_docproperty_fields_render_safe_custom_properties() {
        let output = parse_rtf(
            r#"{\rtf1{\*\userprops{\propname Client Name}{\proptype30}{\staticval Contoso \u937? {\staticval Nested overwrite}{\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt Hidden link}} tail}{\linkval Hidden linked value}}Client: {\field{\*\fldinst DOCPROPERTY "Client Name" \\* Upper}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Client: CONTOSO \u{3a9}  TAIL"));
        for forbidden in [
            "DOCPROPERTY",
            "fldinst",
            "Nested overwrite",
            "HYPERLINK",
            "https://example.com",
            "Hidden link",
            "Hidden linked value",
        ] {
            assert!(
                !text.contains(forbidden),
                "custom document property leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn form_field_metadata_does_not_become_body_text() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\field{\*\fldinst FORMTEXT}{\formfield{\fftype0}{\ffname HiddenName}{\ffdeftext HiddenDefault}{\ffentrymcr launch.exe}{\ffexitmcr https://example.com}{\datafield 414243}}{\fldrslt Visible value}} After\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before Visible value After"));
        for forbidden in [
            "FORMTEXT",
            "HiddenName",
            "HiddenDefault",
            "launch.exe",
            "https://example.com",
            "414243",
            "ffentrymcr",
            "ffexitmcr",
            "datafield",
        ] {
            assert!(
                !text.contains(forbidden),
                "forbidden form-field metadata leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_form_text_renders_passive_default_without_metadata() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\field{\*\fldinst FORMTEXT}{\formfield{\fftype0}{\ffname HiddenName}{\ffdeftext Default \u937? value}{\ffentrymcr launch.exe}{\datafield 414243}}} After\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Before Default \u{03a9} value After"),
            "normalized text was {text:?}; diagnostics were {:?}",
            output.diagnostics
        );
        for forbidden in [
            "FORMTEXT",
            "HiddenName",
            "launch.exe",
            "414243",
            "ffentrymcr",
            "datafield",
        ] {
            assert!(
                !text.contains(forbidden),
                "forbidden form-text metadata leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_form_text_without_default_uses_existing_placeholder_policy() {
        let output = parse_rtf(
            r"{\rtf1 Before {\field{\*\fldinst FORMTEXT}{\formfield{\fftype0}}} After\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before [Field removed: no passive result] After"));
    }

    #[test]
    fn resultless_form_checkbox_renders_passive_glyph_without_metadata() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\field{\*\fldinst FORMCHECKBOX}{\formfield{\fftype1}{\ffname HiddenName}{\ffdefres0}{\ffres1}{\ffentrymcr launch.exe}{\datafield 414243}}} After\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Before \u{2611} After"),
            "normalized text was {text:?}; diagnostics were {:?}",
            output.diagnostics
        );
        assert!(
            output
                .document
                .fonts
                .iter()
                .any(|font| font.name == "ZapfDingbats")
        );
        for forbidden in [
            "FORMCHECKBOX",
            "HiddenName",
            "launch.exe",
            "414243",
            "ffentrymcr",
            "datafield",
        ] {
            assert!(
                !text.contains(forbidden),
                "forbidden checkbox form-field metadata leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_form_checkbox_defaults_to_unchecked_passive_glyph() {
        let output =
            parse_rtf(r"{\rtf1{\field{\*\fldinst FORMCHECKBOX}{\formfield{\fftype1}}}\par}")
                .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("\u{2610}"),
            "normalized text was {text:?}; diagnostics were {:?}",
            output.diagnostics
        );
    }

    #[test]
    fn resultless_form_dropdown_renders_selected_entry_without_metadata() {
        let output = parse_rtf(
            r#"{\rtf1 Before {\field{\*\fldinst FORMDROPDOWN}{\formfield{\fftype2}{\ffname HiddenName}{\ffdefres0}{\ffres1}{\*\ffl First choice}{\*\ffl Second \u937? choice}{\ffentrymcr launch.exe}{\datafield 414243}}} After\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Before Second \u{03a9} choice After"),
            "normalized text was {text:?}; diagnostics were {:?}",
            output.diagnostics
        );
        for forbidden in [
            "FORMDROPDOWN",
            "HiddenName",
            "First choice",
            "launch.exe",
            "414243",
            "ffentrymcr",
            "datafield",
        ] {
            assert!(
                !text.contains(forbidden),
                "forbidden dropdown form-field metadata leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn resultless_form_dropdown_entry_count_is_bounded() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_form_field_entries: 1,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let err = parse_rtf_bytes_with_options(
            br"{\rtf1{\field{\*\fldinst FORMDROPDOWN}{\formfield{\fftype2}{\*\ffl One}{\*\ffl Two}}}\par}",
            &options,
        )
        .unwrap_err();

        assert!(matches!(
            err,
            ParseError::ResourceLimitExceeded { ref resource, .. }
                if resource == "form dropdown entries"
        ));
    }

    #[test]
    fn document_protection_metadata_does_not_become_body_text() {
        let output = parse_rtf(
            r"{\rtf1\formprot\revprot\annotprot{\passwordhash DEADBEEFCAFE0123456789ABCDEF0123}Visible protected body\par\passwordhash AABBCCDDEEFF Inline body\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Visible protected body"));
        assert!(text.contains("Inline body"));
        for forbidden in [
            "passwordhash",
            "DEADBEEF",
            "AABBCC",
            "formprot",
            "revprot",
            "annotprot",
        ] {
            assert!(
                !text.contains(forbidden),
                "forbidden document protection metadata leaked to text: {forbidden}"
            );
        }
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("document protection password hash stripped")
        }));
    }

    #[test]
    fn review_and_bookmark_destinations_do_not_become_body_text() {
        let output = parse_rtf(
            r"{\rtf1 Before{\annotation Hidden comment {\object\objdata 414243}{\result Hidden result}} middle{\*\bkmkstart SecretBookmark} visible{\*\bkmkend SecretBookmark}{\deleted Deleted text} after\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before middle visible after"));
        assert!(!text.contains("Hidden comment"));
        assert!(!text.contains("Hidden result"));
        assert!(!text.contains("Embedded object removed"));
        assert!(!text.contains("SecretBookmark"));
        assert!(!text.contains("Deleted text"));
        assert!(!text.contains("414243"));
    }

    #[test]
    fn mail_merge_metadata_does_not_become_body_text() {
        let output = parse_rtf(
            r"{\rtf1 Before{\mailmerge{\mmconnectstr Provider=SQLOLEDB;Password=secret}{\mmdatasource C:\remote\contacts.mdb}{\mmquery SELECT * FROM Contacts}{\mmodsoudl http://example.com/source.udl}{\object\objdata 414243}} after\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before after"));
        for forbidden in [
            "Provider=SQLOLEDB",
            "Password=secret",
            "contacts.mdb",
            "SELECT *",
            "example.com",
            "Embedded object removed",
            "objdata",
            "414243",
        ] {
            assert!(
                !text.contains(forbidden),
                "mail merge metadata leaked to text: {forbidden}"
            );
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn object_metadata_destinations_do_not_become_body_text() {
        let output = parse_rtf(
            r"{\rtf1 Before{\objclass HiddenClass {\object\objdata 414243}} middle{\objname HiddenName}{\objalias HiddenAlias}{\objtopic HiddenTopic} after\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before middle after"));
        for forbidden in [
            "HiddenClass",
            "HiddenName",
            "HiddenAlias",
            "HiddenTopic",
            "Embedded object removed",
            "objclass",
            "objname",
            "objalias",
            "objtopic",
            "objdata",
            "414243",
        ] {
            assert!(
                !text.contains(forbidden),
                "object metadata leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn object_metadata_inside_object_does_not_block_safe_result() {
        let output = parse_rtf(
            r"{\rtf1{\object{\*\objclass HiddenClass}{\objname HiddenName}\objdata 414243{\result visible fallback}}}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("visible fallback"));
        assert!(!text.contains("[Embedded object removed]"));
        assert!(!text.contains("HiddenClass"));
        assert!(!text.contains("HiddenName"));
        assert!(!text.contains("414243"));
    }

    #[test]
    fn header_object_result_stays_in_safe_repeating_metadata() {
        let output = parse_rtf(
            r"{\rtf1{\header Prefix {\object\objdata 414243{\result Object fallback\par}}\par}Body\par}",
        )
        .unwrap();
        let header_text = output.document.header[0]
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();
        let body_text = document_text(&output.document);

        assert_eq!(header_text, "Prefix Object fallback");
        assert_eq!(body_text, "Body");
        for forbidden in ["objdata", "414243", "[Embedded object removed]"] {
            assert!(
                !header_text.contains(forbidden),
                "object internals leaked to header result text: {forbidden}"
            );
            assert!(
                !body_text.contains(forbidden),
                "object internals leaked to body text: {forbidden}"
            );
        }
    }

    #[test]
    fn header_resultless_object_placeholder_stays_in_safe_repeating_metadata() {
        let output =
            parse_rtf(r"{\rtf1{\header Prefix {\object\objdata 414243}\par}Body\par}").unwrap();
        let header_text = output.document.header[0]
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();
        let body_text = document_text(&output.document);

        assert_eq!(header_text, "Prefix [Embedded object removed]");
        assert_eq!(body_text, "Body");
        assert!(!body_text.contains("[Embedded object removed]"));
        assert!(!header_text.contains("objdata"));
        assert!(!header_text.contains("414243"));
    }

    #[test]
    fn revision_insert_metadata_preserves_visible_text_without_controls() {
        let output = parse_rtf(
            r"{\rtf1 Before {\revised\revauth1\revdttm123456789\insrsid42 Inserted text} {\deleted Deleted text} after\par}",
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Before Inserted text  after"));
        assert!(!text.contains("Deleted text"));
        assert!(!text.contains("revised"));
        assert!(!text.contains("revauth"));
        assert!(!text.contains("revdttm"));
        assert!(!text.contains("insrsid"));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn active_controls_nested_in_metadata_remain_non_visual() {
        let output = parse_rtf(
            r#"{\rtf1 Start{\info{\title Hidden {\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt Hidden link}}{\pict\pngblip 00}{\shp{\shpinst{\shptxt Hidden shape}}}{\header Hidden header\par}{\listtext Hidden marker\tab}}} End\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Start End"));
        assert!(!text.contains("Hidden"));
        assert!(!text.contains("https://example.com"));
        assert!(!text.contains("Image skipped"));
        assert!(!text.contains("Shape skipped"));
        assert!(!text.contains("Field removed"));
    }

    #[test]
    fn external_template_destination_obeys_reject_policy() {
        let options = RtfParseOptions {
            active_content_policy: ActiveContentPolicy::Reject,
            ..RtfParseOptions::default()
        };

        assert!(matches!(
            parse_rtf_bytes_with_options(
                br"{\rtf1\template https://example.com/t.dotm Body}",
                &options,
            ),
            Err(ParseError::ActiveContentRejected { feature, .. }) if feature == "external template"
        ));
    }

    #[test]
    fn renders_shape_text_as_safe_passive_paragraph() {
        let output =
            parse_rtf(r"{\rtf1 Before\par{\shp{\shpinst{\shptxt Box text\par}}}After\par}")
                .unwrap();
        let paragraph_text = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph) => Some(
                    paragraph
                        .runs
                        .iter()
                        .map(|run| run.text.as_str())
                        .collect::<String>(),
                ),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(paragraph_text, vec!["Before", "Box text", "After"]);
        assert!(output.document.blocks.iter().all(|block| {
            !matches!(
                block,
                Block::Placeholder(text) if text.contains("Shape skipped")
            )
        }));
    }

    #[test]
    fn renders_old_drawing_text_box_as_safe_passive_paragraph() {
        let output = parse_rtf(
            r"{\rtf1 Before\par{\do\dobx720\doby720\dodhgt1{\dptxbx Legacy box text\par}}After\par}",
        )
        .unwrap();
        let paragraph_text = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph) => Some(
                    paragraph
                        .runs
                        .iter()
                        .map(|run| run.text.as_str())
                        .collect::<String>(),
                ),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(paragraph_text, vec!["Before", "Legacy box text", "After"]);
        assert!(output.document.blocks.iter().all(|block| {
            !matches!(
                block,
                Block::Placeholder(text) if text.contains("Shape skipped")
            )
        }));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_legacy_static_drawing_shapes_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1 Before\par{\do\dprect\dobx120\doby240\dpx360\dpy480\dpxsize1440\dpysize720\dplinew30\dplinecor255\dplinecog128\dplinecob0\dpfillfgcr10\dpfillfgcg20\dpfillfgcb30{\sp{\sn pFragments}{\sv hostile-payload}}}After\par}",
        )
        .unwrap();
        let shapes = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .collect::<Vec<_>>();
        let text = document_text(&output.document);

        assert_eq!(shapes.len(), 1);
        assert_eq!(shapes[0].kind, StaticShapeKind::Rectangle);
        assert_eq!(shapes[0].left_twips, 480);
        assert_eq!(shapes[0].top_twips, 720);
        assert_eq!(shapes[0].width_twips, 1440);
        assert_eq!(shapes[0].height_twips, 720);
        assert_eq!(shapes[0].stroke_width_twips, 30);
        assert_eq!(shapes[0].stroke_color.red, 255);
        assert_eq!(shapes[0].stroke_color.green, 128);
        assert_eq!(shapes[0].stroke_color.blue, 0);
        assert_eq!(
            shapes[0].fill_color,
            Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            })
        );
        assert!(text.contains("BeforeAfter"));
        for forbidden in ["dprect", "dobx", "dpfill", "pFragments", "hostile-payload"] {
            assert!(!text.contains(forbidden));
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_header_static_drawing_shapes_as_safe_repeating_metadata() {
        let output = parse_rtf(
            r"{\rtf1{\header Logo {\do\dprect\dpxsize1440\dpysize720\dplinew30\dpfillfgcr10\dpfillfgcg20\dpfillfgcb30}\par}Body\par}",
        )
        .unwrap();

        assert_eq!(output.document.header.len(), 1);
        assert_eq!(output.document.header[0].runs[0].text, "Logo ");
        assert_eq!(output.document.header_shapes.len(), 1);
        assert!(
            output
                .document
                .blocks
                .iter()
                .all(|block| !matches!(block, Block::Shape(_)))
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_header_shape_text_as_safe_repeating_metadata() {
        let output = parse_rtf(
            r"{\rtf1{\header Logo {\shp{\*\shpinst{\sp{\sn pFragments}{\sv hostile-shape-payload}}}{\shptxt Box text\par}}\par}Body\par}",
        )
        .unwrap();

        let body_text = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph) => Some(
                    paragraph
                        .runs
                        .iter()
                        .map(|run| run.text.as_str())
                        .collect::<String>(),
                ),
                _ => None,
            })
            .collect::<String>();

        assert_eq!(output.document.header.len(), 1);
        assert_eq!(output.document.header[0].runs[0].text, "Logo Box text");
        assert_eq!(body_text, "Body");
        for forbidden in [
            "pFragments",
            "hostile-shape-payload",
            "shpinst",
            "shptxt",
            "[Shape skipped",
        ] {
            assert!(
                !body_text.contains(forbidden),
                "forbidden shape text content leaked to body: {forbidden}"
            );
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_zero_width_static_drawing_outline_as_safe_fill_only_shape() {
        let output = parse_rtf(
            r"{\rtf1 Before\par{\do\dprect\dpxsize1440\dpysize720\dplinew0\dpfillfgcr10\dpfillfgcg20\dpfillfgcb30}After\par}",
        )
        .unwrap();
        let shape = output
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .expect("shape");

        assert_eq!(shape.kind, StaticShapeKind::Rectangle);
        assert_eq!(shape.stroke_width_twips, 0);
        assert_eq!(
            shape.fill_color,
            Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            })
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_legacy_static_drawing_ellipses_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1 Before\par{\do\dpellipse\dobx120\doby240\dpx360\dpy480\dpxsize1440\dpysize720\dplinew30\dplinecor255\dplinecog128\dplinecob0\dpfillfgcr10\dpfillfgcg20\dpfillfgcb30{\sp{\sn pFragments}{\sv hostile-payload}}}After\par}",
        )
        .unwrap();
        let shapes = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .collect::<Vec<_>>();
        let text = document_text(&output.document);

        assert_eq!(shapes.len(), 1);
        assert_eq!(shapes[0].kind, StaticShapeKind::Ellipse);
        assert_eq!(shapes[0].left_twips, 480);
        assert_eq!(shapes[0].top_twips, 720);
        assert_eq!(shapes[0].width_twips, 1440);
        assert_eq!(shapes[0].height_twips, 720);
        assert_eq!(shapes[0].stroke_width_twips, 30);
        assert_eq!(shapes[0].stroke_color.red, 255);
        assert_eq!(shapes[0].stroke_color.green, 128);
        assert_eq!(shapes[0].stroke_color.blue, 0);
        assert_eq!(
            shapes[0].fill_color,
            Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            })
        );
        assert!(text.contains("BeforeAfter"));
        for forbidden in [
            "dpellipse",
            "dobx",
            "dpfill",
            "pFragments",
            "hostile-payload",
        ] {
            assert!(!text.contains(forbidden));
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_legacy_static_drawing_rounded_rectangles_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1 Before\par{\do\dprect\dproundr\dobx120\doby240\dpx360\dpy480\dpxsize1440\dpysize720\dplinew30\dplinecor255\dplinecog128\dplinecob0\dpfillfgcr10\dpfillfgcg20\dpfillfgcb30{\sp{\sn pFragments}{\sv hostile-payload}}}After\par}",
        )
        .unwrap();
        let shapes = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .collect::<Vec<_>>();
        let text = document_text(&output.document);

        assert_eq!(shapes.len(), 1);
        assert_eq!(shapes[0].kind, StaticShapeKind::RoundedRectangle);
        assert_eq!(shapes[0].left_twips, 480);
        assert_eq!(shapes[0].top_twips, 720);
        assert_eq!(shapes[0].width_twips, 1440);
        assert_eq!(shapes[0].height_twips, 720);
        assert_eq!(shapes[0].stroke_width_twips, 30);
        assert_eq!(shapes[0].stroke_color.red, 255);
        assert_eq!(shapes[0].stroke_color.green, 128);
        assert_eq!(shapes[0].stroke_color.blue, 0);
        assert_eq!(
            shapes[0].fill_color,
            Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            })
        );
        assert!(text.contains("BeforeAfter"));
        for forbidden in [
            "dprect",
            "dproundr",
            "dobx",
            "dpfill",
            "pFragments",
            "hostile-payload",
        ] {
            assert!(!text.contains(forbidden));
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_legacy_static_drawing_line_styles_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1 Before\par{\do\dpline\dplinedot\dpx360\dpy480\dpxsize1440\dpysize720\dplinew30{\sp{\sn pFragments}{\sv hostile-payload}}}After\par}",
        )
        .unwrap();
        let shape = output
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .expect("shape");
        let text = document_text(&output.document);

        assert_eq!(shape.kind, StaticShapeKind::Line);
        assert_eq!(shape.stroke_style, BorderStyle::Dotted);
        assert!(text.contains("BeforeAfter"));
        for forbidden in ["dpline", "dplinedot", "pFragments", "hostile-payload"] {
            assert!(!text.contains(forbidden));
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_legacy_static_drawing_polylines_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1 Before\par{\do\dppolyline\dplinedash\dplinew30\dpptx360\dppty480\dpptx1080\dppty1200\dpptx1800\dppty480{\sp{\sn pFragments}{\sv hostile-payload}}}After\par}",
        )
        .unwrap();
        let shape = output
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .expect("polyline");
        let text = document_text(&output.document);

        assert_eq!(shape.kind, StaticShapeKind::Polyline);
        assert_eq!(shape.left_twips, 360);
        assert_eq!(shape.top_twips, 480);
        assert_eq!(shape.width_twips, 1440);
        assert_eq!(shape.height_twips, 720);
        assert_eq!(shape.stroke_style, BorderStyle::Dashed);
        assert_eq!(shape.points.len(), 3);
        assert_eq!(shape.points[0], StaticShapePoint::default());
        assert_eq!(
            shape.points[1],
            StaticShapePoint {
                x_twips: 720,
                y_twips: 720,
            }
        );
        assert_eq!(
            shape.points[2],
            StaticShapePoint {
                x_twips: 1440,
                y_twips: 0,
            }
        );
        assert!(text.contains("BeforeAfter"));
        for forbidden in [
            "dppolyline",
            "dpptx",
            "dppty",
            "dplinedash",
            "pFragments",
            "hostile-payload",
        ] {
            assert!(!text.contains(forbidden));
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_legacy_static_drawing_polygons_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1 Before\par{\do\dppolygon\dplinedot\dplinew30\dpfillfgcr10\dpfillfgcg20\dpfillfgcb30\dpptx360\dppty480\dpptx1080\dppty1200\dpptx1800\dppty480{\sp{\sn pFragments}{\sv hostile-payload}}}After\par}",
        )
        .unwrap();
        let shape = output
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Shape(shape) => Some(shape),
                _ => None,
            })
            .expect("polygon");
        let text = document_text(&output.document);

        assert_eq!(shape.kind, StaticShapeKind::Polygon);
        assert_eq!(shape.left_twips, 360);
        assert_eq!(shape.top_twips, 480);
        assert_eq!(shape.width_twips, 1440);
        assert_eq!(shape.height_twips, 720);
        assert_eq!(shape.stroke_style, BorderStyle::Dotted);
        assert_eq!(
            shape.fill_color,
            Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            })
        );
        assert_eq!(shape.points.len(), 3);
        assert_eq!(shape.points[0], StaticShapePoint::default());
        assert!(text.contains("BeforeAfter"));
        for forbidden in [
            "dppolygon",
            "dpptx",
            "dppty",
            "dplinedot",
            "dpfillfg",
            "pFragments",
            "hostile-payload",
        ] {
            assert!(!text.contains(forbidden));
        }
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn bounds_legacy_static_drawing_polyline_points() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_shape_points: 2,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        assert!(matches!(
            parse_rtf_bytes_with_options(
                br"{\rtf1{\do\dppolyline\dpptx0\dppty0\dpptx10\dppty10\dpptx20\dppty0}}",
                &options,
            ),
            Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "shape points"
        ));
    }

    #[test]
    fn unsupported_shape_properties_become_placeholder_without_payload_leakage() {
        let output = parse_rtf(
            r"{\rtf1{\shp{\shpinst{\sp{\sn pFragments}{\sv hostile-payload}}}}After\par}",
        )
        .unwrap();
        let text = output
            .document
            .blocks
            .iter()
            .flat_map(|block| match block {
                Block::Paragraph(paragraph) => paragraph
                    .runs
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<Vec<_>>(),
                Block::Placeholder(text) => vec![text.as_str()],
                _ => Vec::new(),
            })
            .collect::<String>();

        assert!(text.contains("[Shape skipped: unsupported shape]"));
        assert!(text.contains("After"));
        assert!(!text.contains("pFragments"));
        assert!(!text.contains("hostile-payload"));
    }

    #[test]
    fn normalizes_visible_character_positioning_controls() {
        let output = parse_rtf(
            r"{\rtf1{\strike struck} {\strikedl double} {\super sup} {\sub sub} {\nosupersub base} {\super0 superoff} {\sub0 suboff} {\up8 up} {\dn6 down}\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        let run_containing = |text: &str| {
            paragraph
                .runs
                .iter()
                .find(|run| run.text.contains(text))
                .unwrap_or_else(|| panic!("missing run containing {text}"))
        };

        let struck = run_containing("struck");
        assert!(struck.style.strike);
        assert!(!struck.style.double_strike);

        let double = run_containing("double");
        assert!(double.style.strike);
        assert!(double.style.double_strike);

        let sup = run_containing("sup");
        assert_eq!(
            sup.style.baseline_shift_half_points,
            DEFAULT_SUPERSCRIPT_SHIFT_HALF_POINTS
        );
        assert_eq!(
            sup.style.font_size_scale_percent,
            DEFAULT_SCRIPT_FONT_SCALE_PERCENT
        );

        let sub = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "sub")
            .expect("sub run");
        assert_eq!(
            sub.style.baseline_shift_half_points,
            DEFAULT_SUBSCRIPT_SHIFT_HALF_POINTS
        );
        assert_eq!(
            sub.style.font_size_scale_percent,
            DEFAULT_SCRIPT_FONT_SCALE_PERCENT
        );

        let base = run_containing("base");
        assert_eq!(base.style.baseline_shift_half_points, 0);
        assert_eq!(base.style.font_size_scale_percent, 100);

        let superoff = run_containing("superoff");
        assert_eq!(superoff.style.baseline_shift_half_points, 0);
        assert_eq!(superoff.style.font_size_scale_percent, 100);

        let suboff = run_containing("suboff");
        assert_eq!(suboff.style.baseline_shift_half_points, 0);
        assert_eq!(suboff.style.font_size_scale_percent, 100);

        let up = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "up")
            .expect("up run");
        assert_eq!(up.style.baseline_shift_half_points, 8);
        assert_eq!(up.style.font_size_scale_percent, 100);

        let down = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "down")
            .expect("down run");
        assert_eq!(down.style.baseline_shift_half_points, -6);
        assert_eq!(down.style.font_size_scale_percent, 100);
    }

    #[test]
    fn normalizes_outline_text_controls() {
        let output = parse_rtf(r"{\rtf1{\outl outline} {\outl0 plain}\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let outline = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "outline")
            .expect("outline run");
        let plain = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "plain")
            .expect("plain run");

        assert!(outline.style.outline);
        assert!(!plain.style.outline);
    }

    #[test]
    fn normalizes_shadow_text_controls() {
        let output = parse_rtf(r"{\rtf1{\shad shadow} {\shad0 plain}\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let shadow = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "shadow")
            .expect("shadow run");
        let plain = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "plain")
            .expect("plain run");

        assert!(shadow.style.shadow);
        assert!(!plain.style.shadow);
    }

    #[test]
    fn normalizes_emboss_and_engrave_text_controls() {
        let output =
            parse_rtf(r"{\rtf1{\embo emboss} {\impr engrave} {\impr0 plain}\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let style_for = |text: &str| {
            paragraph
                .runs
                .iter()
                .find(|run| run.text.trim() == text)
                .map(|run| &run.style)
                .unwrap_or_else(|| panic!("missing run {text}"))
        };

        assert_eq!(style_for("emboss").relief, TextRelief::Emboss);
        assert_eq!(style_for("engrave").relief, TextRelief::Engrave);
        assert_eq!(style_for("plain").relief, TextRelief::None);
    }

    #[test]
    fn normalizes_word_underline_style_controls() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red0\green0\blue0;\red255\green0\blue0;}{\ul single} {\ulw words only} {\uldb double} {\ulth thick} {\uld dotted} {\uldash dashed} {\ulwave wave} {\ulc2 colored} {\ulnone plain}\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let style_for = |text: &str| {
            paragraph
                .runs
                .iter()
                .find(|run| run.text.trim() == text)
                .map(|run| &run.style)
                .unwrap_or_else(|| panic!("missing run {text}"))
        };

        assert_eq!(style_for("single").underline, UnderlineStyle::Single);
        assert_eq!(style_for("words only").underline, UnderlineStyle::Words);
        assert_eq!(style_for("double").underline, UnderlineStyle::Double);
        assert_eq!(style_for("thick").underline, UnderlineStyle::Thick);
        assert_eq!(style_for("dotted").underline, UnderlineStyle::Dotted);
        assert_eq!(style_for("dashed").underline, UnderlineStyle::Dashed);
        assert_eq!(style_for("wave").underline, UnderlineStyle::Wave);
        assert_eq!(style_for("colored").underline_color_index, Some(2));
        assert_eq!(style_for("plain").underline, UnderlineStyle::None);
    }

    #[test]
    fn normalizes_caps_controls() {
        let output =
            parse_rtf(r"{\rtf1{\caps Shout} {\caps0 quiet} {\scaps Small} {\scaps0 normal}\par}")
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        let shout = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Shout")
            .expect("caps run");
        assert!(shout.style.all_caps);

        let quiet = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "quiet")
            .expect("caps cleared run");
        assert!(!quiet.style.all_caps);

        let small = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Small")
            .expect("small caps run");
        assert!(small.style.small_caps);

        let normal = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "normal")
            .expect("small caps cleared run");
        assert!(!normal.style.small_caps);
    }

    #[test]
    fn clamps_extreme_font_size_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_font_size_half_points: 96,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\fs0 tiny \fs99999 huge\par}", &options).unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let tiny = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "tiny")
            .expect("tiny run");
        let huge = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "huge")
            .expect("huge run");

        assert_eq!(tiny.style.font_size_half_points, 2);
        assert_eq!(huge.style.font_size_half_points, 96);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("font size clamped"))
        );
    }

    #[test]
    fn normalizes_and_clamps_character_spacing_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_character_spacing_twips: 120,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\expnd6 expanded \expndtw-80 condensed \expndtw9999 clamped\par}",
            &options,
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let style_for = |text: &str| {
            paragraph
                .runs
                .iter()
                .find(|run| run.text.trim() == text)
                .map(|run| &run.style)
                .unwrap_or_else(|| panic!("missing run {text}"))
        };

        assert_eq!(style_for("expanded").character_spacing_twips, 60);
        assert_eq!(style_for("condensed").character_spacing_twips, -80);
        assert_eq!(style_for("clamped").character_spacing_twips, 120);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("character spacing clamped"))
        );
    }

    #[test]
    fn normalizes_and_clamps_character_kerning_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_font_size_half_points: 96,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\kerning2 kerned \kerning999 clamped \kerning0 plain\par}",
            &options,
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let style_for = |text: &str| {
            paragraph
                .runs
                .iter()
                .find(|run| run.text.trim() == text)
                .map(|run| &run.style)
                .unwrap_or_else(|| panic!("missing run {text}"))
        };

        assert_eq!(style_for("kerned").character_kerning_half_points, 2);
        assert_eq!(style_for("clamped").character_kerning_half_points, 96);
        assert_eq!(style_for("plain").character_kerning_half_points, 0);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("character kerning approximated by passive pair spacing")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("character kerning threshold clamped")
        }));
    }

    #[test]
    fn normalizes_and_clamps_character_scaling_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                min_character_scaling_percent: 50,
                max_character_scaling_percent: 180,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\charscalex150 wide \charscalex5 narrow \charscalex999 clamped\par}",
            &options,
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let style_for = |text: &str| {
            paragraph
                .runs
                .iter()
                .find(|run| run.text.trim() == text)
                .map(|run| &run.style)
                .unwrap_or_else(|| panic!("missing run {text}"))
        };

        assert_eq!(style_for("wide").character_scaling_percent, 150);
        assert_eq!(style_for("narrow").character_scaling_percent, 50);
        assert_eq!(style_for("clamped").character_scaling_percent, 180);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("character scaling clamped"))
        );
    }

    #[test]
    fn normalizes_page_geometry_controls() {
        let output = parse_rtf(
            r"{\rtf1\paperw15840\paperh12240\margl720\margr720\margt1440\margb1440\gutter360\headery360\footery1080 Body\par}",
        )
        .unwrap();

        assert_eq!(output.document.page.width_twips, 15_840);
        assert_eq!(output.document.page.height_twips, 12_240);
        assert_eq!(output.document.page.margin_left_twips, 720);
        assert_eq!(output.document.page.margin_right_twips, 720);
        assert_eq!(output.document.page.margin_top_twips, 1_440);
        assert_eq!(output.document.page.margin_bottom_twips, 1_440);
        assert_eq!(output.document.page.gutter_twips, 360);
        assert!(!output.document.page.mirror_margins);
        assert!(!output.document.page.gutter_on_right);
        assert_eq!(output.document.page.header_distance_twips, 360);
        assert_eq!(output.document.page.footer_distance_twips, 1_080);
    }

    #[test]
    fn normalizes_page_vertical_alignment_controls() {
        let output = parse_rtf(r"{\rtf1\vertalc Centered\par}").unwrap();
        assert_eq!(
            output.document.page.vertical_alignment,
            PageVerticalAlignment::Center
        );

        let output = parse_rtf(r"{\rtf1\vertalb Bottom\par}").unwrap();
        assert_eq!(
            output.document.page.vertical_alignment,
            PageVerticalAlignment::Bottom
        );

        let output = parse_rtf(r"{\rtf1\vertalt Top\par}").unwrap();
        assert_eq!(
            output.document.page.vertical_alignment,
            PageVerticalAlignment::Top
        );
    }

    #[test]
    fn normalizes_facing_page_mirror_margin_controls_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1\facingp\gutter360 Body\par}").unwrap();

        assert!(output.document.page.mirror_margins);
        assert_eq!(output.document.page.gutter_twips, 360);

        let output = parse_rtf(r"{\rtf1\margmirror0 Body\par}").unwrap();
        assert!(!output.document.page.mirror_margins);
    }

    #[test]
    fn normalizes_right_to_left_gutter_controls_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1\rtlgutter\gutter360 Body\par}").unwrap();

        assert!(output.document.page.gutter_on_right);
        assert_eq!(output.document.page.gutter_twips, 360);

        let output = parse_rtf(r"{\rtf1\rtlgutter0 Body\par}").unwrap();
        assert!(!output.document.page.gutter_on_right);
    }

    #[test]
    fn normalizes_page_border_controls_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1\pgbrdrt\brdrs\brdrw80\brsp240\pgbrdrb\brdrdb\brdrw40\brsp120 Body\par}",
        )
        .unwrap();

        assert!(output.document.page.page_borders.top.visible);
        assert_eq!(
            output.document.page.page_borders.top.style,
            BorderStyle::Single
        );
        assert_eq!(output.document.page.page_borders.top.width_twips, 80);
        assert_eq!(
            output.document.page.page_border_spacing_twips.top_twips,
            240
        );
        assert!(output.document.page.page_borders.bottom.visible);
        assert_eq!(
            output.document.page.page_borders.bottom.style,
            BorderStyle::Double
        );
        assert_eq!(output.document.page.page_borders.bottom.width_twips, 40);
        assert_eq!(
            output.document.page.page_border_spacing_twips.bottom_twips,
            120
        );
        assert!(!output.document.page.page_borders.left.visible);
        assert!(!output.document.page.page_borders.right.visible);
    }

    #[test]
    fn normalizes_page_border_header_footer_controls_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1\pgbrdrt\pgbrdrhead\pgbrdrb\pgbrdrfoot Body\par}").unwrap();

        assert!(output.document.page.page_border_includes_header);
        assert!(output.document.page.page_border_includes_footer);

        let output = parse_rtf(r"{\rtf1\pgbrdrhead0\pgbrdrfoot0 Body\par}").unwrap();
        assert!(!output.document.page.page_border_includes_header);
        assert!(!output.document.page.page_border_includes_footer);
    }

    #[test]
    fn normalizes_page_border_reference_mode_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1\pgbrdropt\pgbrdrt\brdrs Body\par}").unwrap();

        assert!(output.document.page.page_border_from_page_edge);

        let output = parse_rtf(r"{\rtf1\pgbrdropt0\pgbrdrt\brdrs Body\par}").unwrap();
        assert!(!output.document.page.page_border_from_page_edge);
    }

    #[test]
    fn normalizes_later_section_page_vertical_alignment_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1 First\par\sect\sectd\vertalb Second\par}").unwrap();
        let settings = match &output.document.blocks[2] {
            Block::SectionSettings(settings) => settings,
            _ => panic!("expected section page settings block"),
        };

        assert_eq!(settings.vertical_alignment, PageVerticalAlignment::Bottom);
    }

    #[test]
    fn normalizes_later_section_rtl_gutter_as_safe_metadata() {
        let output =
            parse_rtf(r"{\rtf1 First\par\sect\sectd\rtlguttersxn\gutter360 Second\par}").unwrap();
        let settings = match &output.document.blocks[2] {
            Block::SectionSettings(settings) => settings,
            _ => panic!("expected section page settings block"),
        };

        assert!(settings.gutter_on_right);
        assert_eq!(settings.gutter_twips, 360);
    }

    #[test]
    fn normalizes_later_section_page_borders_as_safe_metadata() {
        let output =
            parse_rtf(r"{\rtf1 First\par\sect\sectd\pgbrdrl\brdrdash Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert!(settings.page_borders.left.visible);
        assert_eq!(settings.page_borders.left.style, BorderStyle::Dashed);
        assert!(!settings.page_borders.top.visible);
    }

    #[test]
    fn normalizes_later_section_page_border_header_footer_controls_as_safe_metadata() {
        let output =
            parse_rtf(r"{\rtf1 First\par\sect\sectd\pgbrdrhead\pgbrdrfoot Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert!(settings.page_border_includes_header);
        assert!(settings.page_border_includes_footer);
    }

    #[test]
    fn normalizes_later_section_page_border_reference_mode_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1 First\par\sect\sectd\pgbrdropt Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert!(settings.page_border_from_page_edge);
    }

    #[test]
    fn clamps_extreme_page_border_width_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_table_border_width_twips: 40,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\pgbrdrt\brdrs\brdrw9999 Bordered\par}",
            &options,
        )
        .unwrap();

        assert_eq!(output.document.page.page_borders.top.width_twips, 40);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("border width clamped"))
        );
    }

    #[test]
    fn clamps_extreme_page_border_spacing_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_page_border_spacing_twips: 360,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\pgbrdrl\brdrs\brsp9999 Bordered\par}", &options)
                .unwrap();

        assert_eq!(
            output.document.page.page_border_spacing_twips.left_twips,
            360
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("page border spacing clamped"))
        );
    }

    #[test]
    fn normalizes_word_section_page_geometry_controls() {
        let output = parse_rtf(
            r"{\rtf1\pgwsxn15840\pghsxn12240\marglsxn720\margrsxn720\margtsxn1440\margbsxn1440\guttersxn360 Body\par}",
        )
        .unwrap();

        assert_eq!(output.document.page.width_twips, 15_840);
        assert_eq!(output.document.page.height_twips, 12_240);
        assert_eq!(output.document.page.margin_left_twips, 720);
        assert_eq!(output.document.page.margin_right_twips, 720);
        assert_eq!(output.document.page.margin_top_twips, 1_440);
        assert_eq!(output.document.page.margin_bottom_twips, 1_440);
        assert_eq!(output.document.page.gutter_twips, 360);
    }

    #[test]
    fn normalizes_later_section_header_footer_distances_as_safe_metadata() {
        let output =
            parse_rtf(r"{\rtf1 First\par\sect\sectd\headery480\footery960 Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert_eq!(settings.header_distance_twips, 480);
        assert_eq!(settings.footer_distance_twips, 960);
    }

    #[test]
    fn clamps_extreme_header_footer_distance_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_header_footer_distance_twips: 720,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\headery9999\footery9999 Body\par}", &options)
                .unwrap();

        assert_eq!(output.document.page.header_distance_twips, 720);
        assert_eq!(output.document.page.footer_distance_twips, 720);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("header distance clamped"))
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("footer distance clamped"))
        );
    }

    #[test]
    fn clamps_extreme_page_gutter_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_page_gutter_twips: 720,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\gutter9999 Body\par}", &options).unwrap();

        assert_eq!(output.document.page.gutter_twips, 720);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("page gutter clamped"))
        );
    }

    #[test]
    fn normalizes_later_section_page_geometry_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1 First\par\sect\sectd\pgwsxn10080\pghsxn7200\marglsxn720\guttersxn360 Second\par}",
        )
        .unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert_eq!(settings.width_twips, 10_080);
        assert_eq!(settings.height_twips, 7_200);
        assert_eq!(settings.margin_left_twips, 720);
        assert_eq!(settings.gutter_twips, 360);
    }

    #[test]
    fn normalizes_later_section_mirror_margins_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1 First\par\sect\sectd\facingp Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section page settings block");
        };
        assert!(settings.mirror_margins);
    }

    #[test]
    fn normalizes_section_column_controls_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1\cols2\colsx720\linebetcol Body\par}").unwrap();

        assert_eq!(output.document.page.column_count, 2);
        assert_eq!(output.document.page.column_gap_twips, 720);
        assert!(output.document.page.line_between_columns);
    }

    #[test]
    fn normalizes_explicit_section_column_widths_as_safe_metadata() {
        let output = parse_rtf(
            r"{\rtf1\cols3\colsx360\colno1\colw1440\colsr240\colno2\colw2880\colsr480\colno3\colw1440 Body\par}",
        )
        .unwrap();

        assert_eq!(output.document.page.column_count, 3);
        assert_eq!(
            output.document.page.column_widths_twips,
            vec![1440, 2880, 1440]
        );
        assert_eq!(output.document.page.column_gaps_twips, vec![240, 480]);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_later_section_column_controls_as_safe_metadata() {
        let output = parse_rtf(r"{\rtf1 First\par\sect\sectd\cols3\colsx360 Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
        let Block::SectionSettings(settings) = &output.document.blocks[2] else {
            panic!("expected section column settings block");
        };
        assert_eq!(settings.column_count, 3);
        assert_eq!(settings.column_gap_twips, 360);
    }

    #[test]
    fn normalizes_explicit_column_breaks_as_safe_passive_blocks() {
        let output = parse_rtf(r"{\rtf1\cols2 First\par\column Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::ColumnBreak));
    }

    #[test]
    fn normalizes_landscape_orientation_after_page_geometry() {
        let output = parse_rtf(r"{\rtf1\landscape\paperw12240\paperh15840 Body\par}").unwrap();

        assert!(output.document.page.landscape);
        assert_eq!(output.document.page.width_twips, 15_840);
        assert_eq!(output.document.page.height_twips, 12_240);
    }

    #[test]
    fn normalizes_section_breaks_as_safe_passive_blocks() {
        let output = parse_rtf(r"{\rtf1 First\par\sect Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::SectionBreak));
    }

    #[test]
    fn odd_and_even_section_breaks_become_safe_parity_blocks() {
        let odd = parse_rtf(r"{\rtf1 First\par\sbkodd\sect Second\par}").unwrap();
        let even = parse_rtf(r"{\rtf1 First\par\sbkeven\sect Second\par}").unwrap();

        assert!(matches!(odd.document.blocks[1], Block::OddPageSectionBreak));
        assert!(matches!(
            even.document.blocks[1],
            Block::EvenPageSectionBreak
        ));
    }

    #[test]
    fn column_section_breaks_become_safe_column_break_blocks() {
        let output = parse_rtf(r"{\rtf1\cols2 First\par\sbkcol\sect Second\par}").unwrap();

        assert!(matches!(output.document.blocks[1], Block::ColumnBreak));
        assert!(
            !output
                .document
                .blocks
                .iter()
                .any(|block| matches!(block, Block::SectionBreak | Block::PageBreak))
        );
    }

    #[test]
    fn continuous_section_breaks_do_not_force_page_break_blocks() {
        let output = parse_rtf(r"{\rtf1 First\par\sbknone\sect Second\par}").unwrap();

        assert!(
            !output
                .document
                .blocks
                .iter()
                .any(|block| matches!(block, Block::SectionBreak | Block::PageBreak))
        );
    }

    #[test]
    fn normalizes_page_break_before_paragraph_style() {
        let output = parse_rtf(r"{\rtf1 First\par\pagebb Second\par\pagebb0 Third\par}").unwrap();

        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected second paragraph, got {other:?}"),
        };
        assert!(second.style.page_break_before);

        let third = match &output.document.blocks[2] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected third paragraph, got {other:?}"),
        };
        assert!(!third.style.page_break_before);
    }

    #[test]
    fn normalizes_paragraph_direction_controls_as_alignment_metadata() {
        let output =
            parse_rtf(r"{\rtf1\rtlpar Right placed\par\ltrpar Left placed\par\rtlpar Right again\par\pard Default left\par}")
                .unwrap();

        let paragraph = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected paragraph {index}, got {other:?}"),
        };

        assert_eq!(paragraph(0).style.alignment, Alignment::Right);
        assert_eq!(paragraph(1).style.alignment, Alignment::Left);
        assert_eq!(paragraph(2).style.alignment, Alignment::Right);
        assert_eq!(paragraph(3).style.alignment, Alignment::Left);
    }

    #[test]
    fn normalizes_distributed_alignment_controls_as_justified_metadata() {
        let output =
            parse_rtf(r"{\rtf1\qd Distributed\par\qk Thai distributed\par\ql Left\par}").unwrap();

        let paragraph = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected paragraph {index}, got {other:?}"),
        };

        assert_eq!(paragraph(0).style.alignment, Alignment::Justified);
        assert_eq!(paragraph(1).style.alignment, Alignment::Justified);
        assert_eq!(paragraph(2).style.alignment, Alignment::Left);
    }

    #[test]
    fn normalizes_keep_paragraph_pagination_controls() {
        let output =
            parse_rtf(r"{\rtf1\keep Keep together\par\keep0 Plain\par\keepn Keep next\par\widctlpar Widow\par\nowidctlpar No widow\par}")
                .unwrap();

        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected first paragraph, got {other:?}"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected second paragraph, got {other:?}"),
        };
        let third = match &output.document.blocks[2] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected third paragraph, got {other:?}"),
        };
        let fourth = match &output.document.blocks[3] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected fourth paragraph, got {other:?}"),
        };
        let fifth = match &output.document.blocks[4] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected fifth paragraph, got {other:?}"),
        };

        assert!(first.style.keep_together);
        assert!(!second.style.keep_together);
        assert!(third.style.keep_with_next);
        assert!(fourth.style.widow_control);
        assert!(!fifth.style.widow_control);
    }

    #[test]
    fn normalizes_paragraph_hyphenation_controls() {
        let output = parse_rtf(r"{\rtf1\hyphpar Hyphenated\par\hyphpar0 Plain\par}").unwrap();
        let paragraph = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph {index}"),
        };

        assert!(paragraph(0).style.auto_hyphenation);
        assert!(!paragraph(1).style.auto_hyphenation);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("paragraph hyphenation approximated by bounded passive soft hyphenation")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("paragraph hyphenation disabled")
        }));
    }

    #[test]
    fn document_hyphenation_sets_paragraph_default_across_resets() {
        let output = parse_rtf(
            r"{\rtf1\hyphauto\pard Document default\par\hyphpar0 Override off\par\pard Restored default\par\hyphauto0\pard Disabled default\par}",
        )
        .unwrap();
        let paragraph = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected paragraph {index}, got {other:?}"),
        };

        assert!(paragraph(0).style.auto_hyphenation);
        assert!(!paragraph(1).style.auto_hyphenation);
        assert!(paragraph(2).style.auto_hyphenation);
        assert!(!paragraph(3).style.auto_hyphenation);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("document hyphenation approximated by bounded passive soft hyphenation")
        }));
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("document hyphenation disabled") })
        );
    }

    #[test]
    fn capital_word_hyphenation_sets_paragraph_default_across_resets() {
        let output = parse_rtf(
            r"{\rtf1\hyphauto\hyphcaps0\pard Caps suppressed\par\hyphcaps\pard Caps enabled\par}",
        )
        .unwrap();
        let paragraph = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected paragraph {index}, got {other:?}"),
        };

        assert!(paragraph(0).style.auto_hyphenation);
        assert!(!paragraph(0).style.hyphenate_caps);
        assert!(paragraph(1).style.auto_hyphenation);
        assert!(paragraph(1).style.hyphenate_caps);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("capitalized word hyphenation disabled")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("capitalized word hyphenation enabled")
        }));
    }

    #[test]
    fn consecutive_hyphenation_limit_sets_paragraph_default_across_resets() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_hyphenation_consecutive_lines: 2,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\hyphauto\hyphconsec1\pard Limited\par\hyphconsec0\pard Unlimited\par\hyphconsec999\pard Clamped\par}",
            &options,
        )
        .unwrap();
        let paragraph = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected paragraph {index}, got {other:?}"),
        };

        assert!(paragraph(0).style.auto_hyphenation);
        assert_eq!(paragraph(0).style.max_consecutive_hyphenated_lines, Some(1));
        assert!(paragraph(1).style.auto_hyphenation);
        assert_eq!(paragraph(1).style.max_consecutive_hyphenated_lines, None);
        assert!(paragraph(2).style.auto_hyphenation);
        assert_eq!(paragraph(2).style.max_consecutive_hyphenated_lines, Some(2));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("consecutive automatic hyphenation limit applied")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("consecutive hyphenation limit clamped from 999 to 2")
        }));
    }

    #[test]
    fn hyphenation_zone_sets_paragraph_default_across_resets() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_hyphenation_zone_twips: 720,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\hyphauto\hyphhotz480\pard First\par\hyphhotz9999\pard Second\par}",
            &options,
        )
        .unwrap();
        let paragraph = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected paragraph {index}, got {other:?}"),
        };

        assert!(paragraph(0).style.auto_hyphenation);
        assert_eq!(paragraph(0).style.hyphenation_zone_twips, 480);
        assert!(paragraph(1).style.auto_hyphenation);
        assert_eq!(paragraph(1).style.hyphenation_zone_twips, 720);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("hyphenation zone applied to bounded passive hyphenation")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("hyphenation zone clamped from 9999 to 720 twips")
        }));
    }

    #[test]
    fn document_widow_control_sets_paragraph_default_across_resets() {
        let output = parse_rtf(
            r"{\rtf1\widowctrl\pard First\par\nowidctlpar Second\par\pard Third\par\widowctrl0\pard Fourth\par}",
        )
        .unwrap();

        let paragraph = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected paragraph {index}, got {other:?}"),
        };

        assert!(paragraph(0).style.widow_control);
        assert!(!paragraph(1).style.widow_control);
        assert!(paragraph(2).style.widow_control);
        assert!(!paragraph(3).style.widow_control);
    }

    #[test]
    fn normalizes_no_wrap_controls_for_paragraphs_and_table_cells() {
        let output = parse_rtf(
            r"{\rtf1\nowwrap No wrap paragraph\par\nowwrap0 Wrapped paragraph\par\trowd\clNoWrap\cellx1440 Cell no wrap\cell\cellx2880 Cell wrapped\cell\row}",
        )
        .unwrap();

        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected first paragraph, got {other:?}"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected second paragraph, got {other:?}"),
        };
        let table = match &output.document.blocks[2] {
            Block::Table(table) => table,
            other => panic!("expected table, got {other:?}"),
        };

        assert!(first.style.no_wrap);
        assert!(!second.style.no_wrap);
        assert!(table.rows[0].cells[0].paragraphs[0].style.no_wrap);
        assert!(!table.rows[0].cells[1].paragraphs[0].style.no_wrap);
    }

    #[test]
    fn normalizes_drop_cap_controls_as_safe_paragraph_metadata() {
        let output = parse_rtf(r"{\rtf1\dropcapli3\dropcapt1 Dropped\par\pard Plain\par}").unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected first paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected second paragraph"),
        };

        assert_eq!(first.style.drop_cap_lines, 3);
        assert_eq!(second.style.drop_cap_lines, 0);
        assert_eq!(first.runs[0].text, "Dropped");
        assert_eq!(second.runs[0].text, "Plain");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn clamps_extreme_drop_cap_line_controls() {
        let output = parse_rtf(r"{\rtf1\dropcapli999 Oversized\par\dropcapt0 Normal\par}").unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected first paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected second paragraph"),
        };

        assert_eq!(first.style.drop_cap_lines, 10);
        assert_eq!(second.style.drop_cap_lines, 0);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("drop cap lines clamped"))
        );
    }

    #[test]
    fn normalizes_paragraph_indent_controls() {
        let output = parse_rtf(r"{\rtf1\li720\ri360\fi-240 Indented\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.left_indent_twips, 720);
        assert_eq!(paragraph.style.right_indent_twips, 360);
        assert_eq!(paragraph.style.first_line_indent_twips, -240);
    }

    #[test]
    fn clamps_extreme_paragraph_indent_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_paragraph_indent_twips: 480,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\li99999\ri-99999\fi99999 Too much indent\par}",
            &options,
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.left_indent_twips, 480);
        assert_eq!(paragraph.style.right_indent_twips, -480);
        assert_eq!(paragraph.style.first_line_indent_twips, 480);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("left indent clamped"))
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("right indent clamped"))
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("first-line indent clamped"))
        );
    }

    #[test]
    fn clamps_extreme_page_geometry_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                min_page_dimension_twips: 1_000,
                max_page_dimension_twips: 2_000,
                max_page_margin_twips: 500,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\paperw50\paperh99999\margl-20\margr9999 Body\par}",
            &options,
        )
        .unwrap();

        assert_eq!(output.document.page.width_twips, 1_000);
        assert_eq!(output.document.page.height_twips, 2_000);
        assert_eq!(output.document.page.margin_left_twips, 0);
        assert_eq!(output.document.page.margin_right_twips, 500);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("paper width clamped"))
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("right margin clamped"))
        );
    }

    #[test]
    fn clamps_extreme_section_column_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_section_columns: 2,
                max_column_gap_twips: 360,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\cols99\colsx9999 Body\par}", &options).unwrap();

        assert_eq!(output.document.page.column_count, 2);
        assert_eq!(output.document.page.column_gap_twips, 360);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("section columns clamped"))
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("column gap clamped"))
        );
    }

    #[test]
    fn normalizes_foreground_and_background_color_controls() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;\red0\green255\blue0;}{\cf1 Red} {\highlight2 Marked} {\highlight0 Plain} {\cb2 Shaded} {\chshdng5000\chcbpat2 Tinted}\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        let red = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Red")
            .expect("red run");
        assert_eq!(red.style.color_index, 1);

        let marked = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Marked")
            .expect("marked run");
        assert_eq!(marked.style.highlight_index, Some(2));

        let plain = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Plain")
            .expect("plain run");
        assert_eq!(plain.style.highlight_index, None);

        let shaded = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Shaded")
            .expect("shaded run");
        assert_eq!(shaded.style.highlight_index, Some(2));

        let tinted = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Tinted")
            .expect("tinted run");
        assert_eq!(tinted.style.highlight_index, Some(2));
        assert_eq!(tinted.style.highlight_shading_basis_points, 5_000);
    }

    #[test]
    fn clamps_extreme_character_shading_controls() {
        let output = parse_rtf_bytes(br"{\rtf1\chshdng-200 low \chshdng99999 high\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let style_for = |text: &str| {
            paragraph
                .runs
                .iter()
                .find(|run| run.text.trim() == text)
                .map(|run| &run.style)
                .unwrap_or_else(|| panic!("missing run {text}"))
        };

        assert_eq!(style_for("low").highlight_shading_basis_points, 0);
        assert_eq!(style_for("high").highlight_shading_basis_points, 10_000);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("character shading clamped"))
        );
    }

    #[test]
    fn normalizes_paragraph_and_table_shading_intensity_controls() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;}\cbpat1\shading2500 paragraph\par\trowd\trcbpat1\trshdng5000\cellx1440 row\cell\clcbpat1\clshdng7500\cellx2880 cell\cell\row}",
        )
        .unwrap();

        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        assert_eq!(paragraph.style.shading_color_index, Some(1));
        assert_eq!(paragraph.style.shading_basis_points, 2_500);

        let table = match &output.document.blocks[1] {
            Block::Table(table) => table,
            _ => panic!("expected table"),
        };
        assert_eq!(table.rows[0].cells[0].shading_color_index, Some(1));
        assert_eq!(table.rows[0].cells[0].shading_basis_points, 5_000);
        assert_eq!(table.rows[0].cells[1].shading_color_index, Some(1));
        assert_eq!(table.rows[0].cells[1].shading_basis_points, 7_500);
    }

    #[test]
    fn normalizes_paragraph_and_table_shading_pattern_controls() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;}\cbpat1\shading2500\bghoriz paragraph\par\trowd\trcbpat1\trshdng5000\trbgvert\cellx1440 row\cell\clcbpat1\clshdng7500\clbghoriz\cellx2880 cell\cell\row}",
        )
        .unwrap();

        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        assert_eq!(paragraph.style.shading_color_index, Some(1));
        assert_eq!(paragraph.style.shading_basis_points, 2_500);
        assert_eq!(paragraph.style.shading_pattern, ShadingPattern::Horizontal);

        let table = match &output.document.blocks[1] {
            Block::Table(table) => table,
            _ => panic!("expected table"),
        };
        assert_eq!(table.rows[0].cells[0].shading_color_index, Some(1));
        assert_eq!(table.rows[0].cells[0].shading_basis_points, 5_000);
        assert_eq!(
            table.rows[0].cells[0].shading_pattern,
            ShadingPattern::Vertical
        );
        assert_eq!(table.rows[0].cells[1].shading_color_index, Some(1));
        assert_eq!(table.rows[0].cells[1].shading_basis_points, 7_500);
        assert_eq!(
            table.rows[0].cells[1].shading_pattern,
            ShadingPattern::Horizontal
        );
        for forbidden in ["bghoriz", "trbgvert", "clbghoriz"] {
            assert!(
                !document_text(&output.document).contains(forbidden),
                "shading pattern control leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn normalizes_extended_shading_pattern_controls() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;}\cbpat1\bgfdiag forward\par\pard\cbpat1\bgdkdcross dark\par\trowd\trcbpat1\trbgdcross\cellx1440 row\cell\clcbpat1\clbgdkcross\cellx2880 cell\cell\row}",
        )
        .unwrap();

        let forward = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected forward paragraph"),
        };
        let dark = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected dark paragraph"),
        };
        assert_eq!(
            forward.style.shading_pattern,
            ShadingPattern::ForwardDiagonal
        );
        assert_eq!(
            dark.style.shading_pattern,
            ShadingPattern::DarkDiagonalCross
        );

        let table = match &output.document.blocks[2] {
            Block::Table(table) => table,
            _ => panic!("expected table"),
        };
        assert_eq!(
            table.rows[0].cells[0].shading_pattern,
            ShadingPattern::DiagonalCross
        );
        assert_eq!(
            table.rows[0].cells[1].shading_pattern,
            ShadingPattern::DarkCross
        );
        for forbidden in ["bgfdiag", "bgdkdcross", "trbgdcross", "clbgdkcross"] {
            assert!(
                !document_text(&output.document).contains(forbidden),
                "extended shading pattern control leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn clamps_extreme_paragraph_and_table_shading_intensity_controls() {
        let output = parse_rtf_bytes(
            br"{\rtf1{\colortbl;\red255\green0\blue0;}\cbpat1\shading-200 low\par\pard\cbpat1\shading99999 high\par\trowd\trcbpat1\trshdng99999\cellx1440 row\cell\clcbpat1\clshdng-50\cellx2880 cell\cell\row}",
        )
        .unwrap();

        let low = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected low paragraph"),
        };
        let high = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected high paragraph"),
        };
        assert_eq!(low.style.shading_basis_points, 0);
        assert_eq!(high.style.shading_basis_points, 10_000);

        let table = match &output.document.blocks[2] {
            Block::Table(table) => table,
            _ => panic!("expected table"),
        };
        assert_eq!(table.rows[0].cells[0].shading_basis_points, 10_000);
        assert_eq!(table.rows[0].cells[1].shading_basis_points, 0);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("paragraph shading clamped") })
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("table row shading clamped") })
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("table cell shading clamped") })
        );
    }

    #[test]
    fn normalizes_paragraph_shading_controls() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red240\green240\blue0;}\cbpat1 Shaded\par\cbpat0 Plain\par}",
        )
        .unwrap();

        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(first.style.shading_color_index, Some(1));
        assert_eq!(second.style.shading_color_index, None);
    }

    #[test]
    fn normalizes_paragraph_border_controls() {
        let output =
            parse_rtf(r"{\rtf1\brdrb\brdrs\brdrw80\brdrcf2 Bordered\par\pard Plain\par}").unwrap();

        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert!(first.style.borders.bottom.visible);
        assert_eq!(first.style.borders.bottom.width_twips, 80);
        assert_eq!(first.style.borders.bottom.color_index, Some(2));
        assert!(!second.style.borders.bottom.visible);
    }

    #[test]
    fn normalizes_paragraph_bar_border_as_safe_left_border() {
        let output =
            parse_rtf(r"{\rtf1\brdrbar\brdrs\brdrw60\brdrcf1 Barred\par\pard Plain\par}").unwrap();

        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert!(first.style.borders.left.visible);
        assert_eq!(first.style.borders.left.width_twips, 60);
        assert_eq!(first.style.borders.left.color_index, Some(1));
        assert!(!second.style.borders.left.visible);
    }

    #[test]
    fn normalizes_paragraph_between_border_controls() {
        let output =
            parse_rtf(r"{\rtf1\brdrbtw\brdrs\brdrw60\brdrcf1 First\par\brdrbtw\brdrs\brdrw60\brdrcf1 Second\par}").unwrap();

        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        for paragraph in [first, second] {
            assert!(paragraph.style.borders.between.visible);
            assert_eq!(paragraph.style.borders.between.width_twips, 60);
            assert_eq!(paragraph.style.borders.between.color_index, Some(1));
        }
    }

    #[test]
    fn normalizes_paragraph_box_border_controls() {
        let output = parse_rtf(r"{\rtf1\box\brdrs\brdrw40 Boxed\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert!(paragraph.style.borders.left.visible);
        assert!(paragraph.style.borders.right.visible);
        assert!(paragraph.style.borders.top.visible);
        assert!(paragraph.style.borders.bottom.visible);
        assert_eq!(paragraph.style.borders.left.width_twips, 40);
        assert_eq!(paragraph.style.borders.right.width_twips, 40);
        assert_eq!(paragraph.style.borders.top.width_twips, 40);
        assert_eq!(paragraph.style.borders.bottom.width_twips, 40);
    }

    #[test]
    fn normalizes_character_border_controls() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;}\chbrdr\brdrs\brdrw80\brdrcf1 Bordered \chbrdr\brdrnone Plain\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let bordered = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Bordered")
            .expect("bordered run");
        let plain = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Plain")
            .expect("plain run");

        assert!(bordered.style.border.visible);
        assert_eq!(bordered.style.border.width_twips, 80);
        assert_eq!(bordered.style.border.color_index, Some(1));
        assert!(!plain.style.border.visible);
    }

    #[test]
    fn normalizes_paragraph_and_character_border_spacing_controls() {
        let output = parse_rtf(
            r"{\rtf1\box\brdrs\brsp240 Boxed\par\pard\chbrdr\brdrs\brsp120 Character\par}",
        )
        .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(first.style.borders.left.spacing_twips, 240);
        assert_eq!(first.style.borders.right.spacing_twips, 240);
        assert_eq!(first.style.borders.top.spacing_twips, 240);
        assert_eq!(first.style.borders.bottom.spacing_twips, 240);
        assert_eq!(second.runs[0].style.border.spacing_twips, 120);
    }

    #[test]
    fn clamps_extreme_paragraph_border_spacing_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_page_border_spacing_twips: 360,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\box\brdrs\brsp9999 Boxed\par}", &options)
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.borders.left.spacing_twips, 360);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("border spacing clamped"))
        );
    }

    #[test]
    fn normalizes_border_style_controls() {
        let output = parse_rtf(
            r"{\rtf1\brdrb\brdrdb Double paragraph\par\pard\chbrdr\brdrdash Dashed character\par\trowd\clbrdrl\brdrdot\cellx1440 dotted\cell\row}",
        )
        .unwrap();

        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let character = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => &paragraph.runs[0].style,
            _ => panic!("expected paragraph"),
        };
        let table = match &output.document.blocks[2] {
            Block::Table(table) => table,
            _ => panic!("expected table"),
        };

        assert_eq!(paragraph.style.borders.bottom.style, BorderStyle::Double);
        assert_eq!(character.border.style, BorderStyle::Dashed);
        assert_eq!(
            table.rows[0].cells[0].borders.left.style,
            BorderStyle::Dotted
        );
    }

    #[test]
    fn normalizes_extended_word_border_style_controls() {
        let output = parse_rtf(
            r"{\rtf1\box\brdrhair Hairline paragraph\par\pard\brdrb\brdrdashdot Dash dot paragraph\par\trowd\clbrdrl\brdrdashdd\cellx1440 dashdd\cell\row}",
        )
        .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let table = match &output.document.blocks[2] {
            Block::Table(table) => table,
            _ => panic!("expected table"),
        };

        assert_eq!(first.style.borders.left.style, BorderStyle::Hairline);
        assert_eq!(first.style.borders.right.style, BorderStyle::Hairline);
        assert_eq!(first.style.borders.top.style, BorderStyle::Hairline);
        assert_eq!(first.style.borders.bottom.style, BorderStyle::Hairline);
        assert_eq!(second.style.borders.bottom.style, BorderStyle::Dashed);
        assert_eq!(
            table.rows[0].cells[0].borders.left.style,
            BorderStyle::Dashed
        );
    }

    #[test]
    fn normalizes_wavy_word_border_style_controls() {
        let output = parse_rtf(
            r"{\rtf1\brdrb\brdrwavy Wavy paragraph\par\pard\chbrdr\brdrwavydb Wavy character\par\trowd\clbrdrl\brdrwavy\cellx1440 wavy cell\cell\row}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let character = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => &paragraph.runs[0].style,
            _ => panic!("expected paragraph"),
        };
        let table = match &output.document.blocks[2] {
            Block::Table(table) => table,
            _ => panic!("expected table"),
        };

        assert_eq!(paragraph.style.borders.bottom.style, BorderStyle::Wavy);
        assert_eq!(character.border.style, BorderStyle::Wavy);
        assert_eq!(table.rows[0].cells[0].borders.left.style, BorderStyle::Wavy);
    }

    #[test]
    fn normalizes_table_row_border_controls_as_cell_perimeter_borders() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;}\trowd\trbrdrt\brdrdb\brdrw80\brdrcf1\trbrdrl\brdrdash\brdrw40\trbrdrr\brdrdot\brdrw60\cellx1440 A\cell\cellx2880 B\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table"),
        };
        let first = &table.rows[0].cells[0];
        let second = &table.rows[0].cells[1];

        assert_eq!(first.borders.top.style, BorderStyle::Double);
        assert_eq!(first.borders.top.width_twips, 80);
        assert_eq!(first.borders.top.color_index, Some(1));
        assert_eq!(second.borders.top.style, BorderStyle::Double);
        assert_eq!(second.borders.top.width_twips, 80);
        assert_eq!(second.borders.top.color_index, Some(1));
        assert_eq!(first.borders.left.style, BorderStyle::Dashed);
        assert_eq!(first.borders.left.width_twips, 40);
        assert_eq!(second.borders.left.style, BorderStyle::Single);
        assert_eq!(first.borders.right.style, BorderStyle::Single);
        assert_eq!(second.borders.right.style, BorderStyle::Dotted);
        assert_eq!(second.borders.right.width_twips, 60);
    }

    #[test]
    fn clamps_extreme_paragraph_border_width_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_table_border_width_twips: 40,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\brdrb\brdrs\brdrw9999 Bordered\par}", &options)
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.borders.bottom.width_twips, 40);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("border width clamped"))
        );
    }

    #[test]
    fn clamps_extreme_character_border_width_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_table_border_width_twips: 120,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\chbrdr\brdrs\brdrw9999 Bordered\par}", &options)
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].style.border.width_twips, 120);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("border width clamped") })
        );
    }

    #[test]
    fn normalizes_line_spacing_controls() {
        let output = parse_rtf(r"{\rtf1\sl480\slmult1 Double spaced\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.line_spacing_twips, Some(480));
        assert!(paragraph.style.line_spacing_multiple);
    }

    #[test]
    fn normalizes_explicit_tab_stops() {
        let output = parse_rtf(r"{\rtf1\tx1440 Left\tab Right\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.tab_stops_twips, vec![1440]);
        assert_eq!(
            paragraph
                .runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            "Left\tRight"
        );
    }

    #[test]
    fn normalizes_document_default_tab_width() {
        let output = parse_rtf(r"{\rtf1\deftab360 Left\tab Right\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(output.document.default_tab_width_twips, 360);
        assert!(paragraph.style.tab_stops_twips.is_empty());
        assert_eq!(
            paragraph
                .runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            "Left\tRight"
        );
    }

    #[test]
    fn normalizes_tab_leader_controls_with_tab_stops() {
        let output = parse_rtf(
            r"{\rtf1\tldot\tx1440 Dot\tab Right\par\pard\tlmdot\tx1440 Middle\tab Right\par\pard\tleq\tx1440 Equal\tab Right\par}",
        )
        .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let third = match &output.document.blocks[2] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(first.style.tab_stops_twips, vec![1440]);
        assert_eq!(first.style.tab_stop_leaders, vec![TabLeader::Dots]);
        assert_eq!(second.style.tab_stops_twips, vec![1440]);
        assert_eq!(second.style.tab_stop_leaders, vec![TabLeader::MiddleDots]);
        assert_eq!(third.style.tab_stops_twips, vec![1440]);
        assert_eq!(third.style.tab_stop_leaders, vec![TabLeader::Equals]);
    }

    #[test]
    fn normalizes_tab_alignment_controls_with_tab_stops() {
        let output =
            parse_rtf(r"{\rtf1\tqr\tldot\tx1440 Left\tab 9\par\tqc\tx2160 Center\tab C\par\tqdec\tx2880 Value\tab 12.3\par}")
                .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let third = match &output.document.blocks[2] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(first.style.tab_stops_twips, vec![1440]);
        assert_eq!(first.style.tab_stop_leaders, vec![TabLeader::Dots]);
        assert_eq!(first.style.tab_stop_alignments, vec![TabAlignment::Right]);
        assert_eq!(second.style.tab_stops_twips, vec![1440, 2160]);
        assert_eq!(
            second.style.tab_stop_alignments,
            vec![TabAlignment::Right, TabAlignment::Center]
        );
        assert_eq!(third.style.tab_stops_twips, vec![1440, 2160, 2880]);
        assert_eq!(
            third.style.tab_stop_alignments,
            vec![
                TabAlignment::Right,
                TabAlignment::Center,
                TabAlignment::Decimal
            ]
        );
    }

    #[test]
    fn normalizes_bar_tab_stops_as_passive_tab_metadata() {
        let output =
            parse_rtf(r"{\rtf1\tb\tx720\tqr\tx1440 Left\tab 9\par\tb\tx2160 Bar only\par}")
                .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(first.style.tab_stops_twips, vec![720, 1440]);
        assert_eq!(
            first.style.tab_stop_alignments,
            vec![TabAlignment::Bar, TabAlignment::Right]
        );
        assert_eq!(second.style.tab_stops_twips, vec![720, 1440, 2160]);
        assert_eq!(
            second.style.tab_stop_alignments,
            vec![TabAlignment::Bar, TabAlignment::Right, TabAlignment::Bar]
        );
    }

    #[test]
    fn tab_stop_count_limit_is_enforced() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_tab_stops: 1,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        assert!(matches!(
            parse_rtf_bytes_with_options(br"{\rtf1\tx720\tx1440 Too many tabs\par}", &options),
            Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "tab stops"
        ));
    }

    #[test]
    fn clamps_extreme_default_tab_width_control() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_tab_stop_twips: 480,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\deftab9999 Body\par}", &options).unwrap();

        assert_eq!(output.document.default_tab_width_twips, 480);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("default tab width clamped") })
        );
    }

    #[test]
    fn clamps_extreme_line_spacing_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_line_spacing_twips: 720,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output =
            parse_rtf_bytes_with_options(br"{\rtf1\sl99999 Too much spacing\par}", &options)
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.line_spacing_twips, Some(720));
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("line spacing clamped"))
        );
    }

    #[test]
    fn normalizes_paragraph_spacing_controls() {
        let output = parse_rtf(r"{\rtf1\sb240\sa360 Spaced\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.space_before_twips, 240);
        assert_eq!(paragraph.style.space_after_twips, 360);
    }

    #[test]
    fn normalizes_paragraph_auto_spacing_controls() {
        let output =
            parse_rtf(r"{\rtf1\sb0\sbauto\sa0\saauto Auto\par\sbauto0\saauto0 Manual\par}")
                .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected first paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected second paragraph"),
        };

        assert_eq!(first.style.space_before_twips, 0);
        assert_eq!(first.style.space_after_twips, 0);
        assert!(first.style.auto_space_before);
        assert!(first.style.auto_space_after);
        assert!(!second.style.auto_space_before);
        assert!(!second.style.auto_space_after);
    }

    #[test]
    fn normalizes_contextual_paragraph_spacing_control() {
        let output =
            parse_rtf(r"{\rtf1\contextualspace Same style\par\contextualspace0 Normal\par}")
                .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected first paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected second paragraph"),
        };

        assert!(first.style.contextual_spacing);
        assert!(!second.style.contextual_spacing);
    }

    #[test]
    fn clamps_extreme_paragraph_spacing_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_paragraph_spacing_twips: 480,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\sb99999\sa99999 Too much spacing\par}",
            &options,
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.space_before_twips, 480);
        assert_eq!(paragraph.style.space_after_twips, 480);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("paragraph space before clamped")
        }));
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("paragraph space after clamped"))
        );
    }

    #[test]
    fn normalizes_jpeg_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\jpegblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_jpeg_with_dimensions(1, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };
        assert_eq!(image.format, ImageFormat::Jpeg);
        assert_eq!(image.width_px, 1);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
        assert_eq!(image.scale_x_percent, None);
        assert_eq!(image.scale_y_percent, None);
    }

    #[test]
    fn normalizes_grayscale_jpeg_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\jpegblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_grayscale_jpeg_with_dimensions(1, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };
        assert_eq!(image.format, ImageFormat::JpegGrayscale);
        assert_eq!(image.width_px, 1);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
        assert_eq!(image.scale_x_percent, None);
        assert_eq!(image.scale_y_percent, None);
    }

    #[test]
    fn normalizes_cmyk_jpeg_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\jpegblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_cmyk_jpeg_with_dimensions(1, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };
        assert_eq!(image.format, ImageFormat::JpegCmyk);
        assert_eq!(image.width_px, 1);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
        assert_eq!(image.scale_x_percent, None);
        assert_eq!(image.scale_y_percent, None);
    }

    #[test]
    fn normalizes_rgb_png_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picscalex50\\picscaley200 {}}}}}",
            bytes_to_hex(&minimal_rgb_png_with_dimensions(1, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };
        assert_eq!(image.format, ImageFormat::Png);
        assert_eq!(image.width_px, 1);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.display_width_twips, None);
        assert_eq!(image.display_height_twips, None);
        assert_eq!(image.scale_x_percent, Some(50));
        assert_eq!(image.scale_y_percent, Some(200));
        assert!(!image.bytes.is_empty());
    }

    #[test]
    fn normalizes_header_picture_as_safe_repeating_image_metadata() {
        let input = format!(
            "{{\\rtf1{{\\header Logo {{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}\\par}}Body\\par}}",
            bytes_to_hex(&minimal_rgb_png_with_dimensions(1, 1))
        );
        let output = parse_rtf(&input).unwrap();

        assert_eq!(output.document.header.len(), 1);
        assert_eq!(output.document.header[0].runs[0].text, "Logo ");
        assert_eq!(output.document.header_images.len(), 1);
        assert!(
            output
                .document
                .blocks
                .iter()
                .all(|block| !matches!(block, Block::Image(_)))
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_picture_natural_size_hints_as_safe_layout_metadata() {
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picw80\\pich40 {}}}}}",
            bytes_to_hex(&minimal_rgb_png_with_dimensions(1, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Png);
        assert_eq!(image.width_px, 1);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.natural_width_px_hint, Some(80));
        assert_eq!(image.natural_height_px_hint, Some(40));
        assert_eq!(image.display_width_twips, None);
        assert_eq!(image.display_height_twips, None);
    }

    #[test]
    fn normalizes_grayscale_png_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_grayscale_png_with_dimensions(1, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::PngGrayscale);
        assert_eq!(image.width_px, 1);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
        assert!(!image.bytes.is_empty());
    }

    #[test]
    fn normalizes_indexed_png_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_indexed_png_with_dimensions(1, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::PngIndexed);
        assert_eq!(image.width_px, 1);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
        assert_eq!(image.palette, vec![255, 0, 0, 0, 255, 0]);
        assert!(!image.bytes.is_empty());
    }

    #[test]
    fn normalizes_uncompressed_dib_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\dibitmap\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_24bit_dib_with_dimensions(2, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Rgb8);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0]);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
    }

    #[test]
    fn normalizes_8bit_paletted_dib_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\dibitmap\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_8bit_dib_with_dimensions(2, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Rgb8);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0]);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
    }

    #[test]
    fn normalizes_4bit_paletted_dib_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\dibitmap\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_4bit_dib_with_dimensions(2, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Rgb8);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0]);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
    }

    #[test]
    fn normalizes_1bit_paletted_dib_picture_as_safe_static_image() {
        let input = format!(
            "{{\\rtf1{{\\pict\\dibitmap\\picwgoal720\\pichgoal720 {}}}}}",
            bytes_to_hex(&minimal_1bit_dib_with_dimensions(2, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.format, ImageFormat::Rgb8);
        assert_eq!(image.width_px, 2);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0]);
        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(720));
    }

    #[test]
    fn normalizes_ignorable_shape_picture_and_suppresses_nonshape_fallback() {
        let image_hex = bytes_to_hex(&minimal_rgb_png_with_dimensions(1, 1));
        let input = format!(
            "{{\\rtf1{{\\*\\shppict{{\\pict\\pngblip\\picwgoal720 {image_hex}}}}}{{\\nonshppict{{\\pict\\pngblip\\picwgoal1440 {image_hex}}}}}}}"
        );
        let output = parse_rtf(&input).unwrap();
        let images = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Image(image) => Some(image),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(images.len(), 1);
        assert_eq!(images[0].display_width_twips, Some(720));
        assert!(!document_text(&output.document).contains("nonshppict"));
        assert!(!document_text(&output.document).contains("shppict"));
    }

    #[test]
    fn nested_shape_picture_counts_as_safe_shape_result() {
        let image_hex = bytes_to_hex(&minimal_rgb_png_with_dimensions(1, 1));
        let input = format!(
            "{{\\rtf1 Before{{\\shp{{\\*\\shpinst{{\\sp{{\\sn pFragments}}{{\\sv hidden-payload}}}}}}{{\\*\\shppict{{\\pict\\pngblip\\picwgoal720 {image_hex}}}}}}}After\\par}}"
        );
        let output = parse_rtf(&input).unwrap();
        let images = output
            .document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Image(image) => Some(image),
                _ => None,
            })
            .collect::<Vec<_>>();
        let text = document_text(&output.document);

        assert_eq!(images.len(), 1);
        assert_eq!(images[0].display_width_twips, Some(720));
        assert!(text.contains("BeforeAfter"));
        assert!(!text.contains("[Shape skipped"));
        assert!(!text.contains("hidden-payload"));
    }

    #[test]
    fn clamps_extreme_picture_scaling_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                min_image_scaling_percent: 10,
                max_image_scaling_percent: 250,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picscalex0\\picscaley9999 {}}}}}",
            bytes_to_hex(&minimal_rgb_png_with_dimensions(1, 1))
        );
        let output = parse_rtf_bytes_with_options(input.as_bytes(), &options).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.scale_x_percent, Some(10));
        assert_eq!(image.scale_y_percent, Some(250));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("picture horizontal scaling clamped")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("picture vertical scaling clamped")
        }));
    }

    #[test]
    fn clamps_extreme_picture_display_goal_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_image_display_twips: 720,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picwgoal999999\\pichgoal-99 {}}}}}",
            bytes_to_hex(&minimal_rgb_png_with_dimensions(1, 1))
        );
        let output = parse_rtf_bytes_with_options(input.as_bytes(), &options).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.display_width_twips, Some(720));
        assert_eq!(image.display_height_twips, Some(0));
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("picture display width clamped") })
        );
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("picture display height clamped")
        }));
    }

    #[test]
    fn clamps_extreme_picture_natural_size_hints() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_image_dimension_hint_px: 10,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip\\picw999999\\pich-99 {}}}}}",
            bytes_to_hex(&minimal_rgb_png_with_dimensions(1, 1))
        );
        let output = parse_rtf_bytes_with_options(input.as_bytes(), &options).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.width_px, 1);
        assert_eq!(image.height_px, 1);
        assert_eq!(image.natural_width_px_hint, Some(10));
        assert_eq!(image.natural_height_px_hint, None);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("picture natural width clamped") })
        );
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("picture natural height clamped")
        }));
    }

    #[test]
    fn normalizes_picture_crop_controls_as_safe_metadata() {
        let input = format!(
            "{{\\rtf1{{\\pict\\jpegblip\\piccropl120\\piccropt240\\piccropr360\\piccropb480 {}}}}}",
            bytes_to_hex(&minimal_jpeg_with_dimensions(1, 1))
        );
        let output = parse_rtf(&input).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(
            image.crop,
            ImageCrop {
                left_twips: 120,
                top_twips: 240,
                right_twips: 360,
                bottom_twips: 480,
            }
        );
    }

    #[test]
    fn clamps_extreme_picture_crop_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_image_crop_twips: 240,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let input = format!(
            "{{\\rtf1{{\\pict\\jpegblip\\piccropl9999\\piccropt-9999 {}}}}}",
            bytes_to_hex(&minimal_jpeg_with_dimensions(1, 1))
        );
        let output = parse_rtf_bytes_with_options(input.as_bytes(), &options).unwrap();
        let image = match &output.document.blocks[0] {
            Block::Image(image) => image,
            _ => panic!("expected image block"),
        };

        assert_eq!(image.crop.left_twips, 240);
        assert_eq!(image.crop.top_twips, -240);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("picture left crop clamped") })
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("picture top crop clamped") })
        );
    }

    #[test]
    fn malformed_png_picture_becomes_placeholder() {
        let output = parse_rtf(r"{\rtf1{\pict\pngblip 89504e470d0a1a0a}}").unwrap();
        assert!(matches!(
            &output.document.blocks[0],
            Block::Placeholder(text) if text.contains("unsupported PNG")
        ));
    }

    #[test]
    fn unsupported_jpeg_component_count_becomes_placeholder() {
        let output = parse_rtf(&format!(
            "{{\\rtf1{{\\pict\\jpegblip {}}}}}",
            bytes_to_hex(&minimal_jpeg_with_components(1, 1, 2))
        ))
        .unwrap();
        assert!(matches!(
            &output.document.blocks[0],
            Block::Placeholder(text) if text.contains("malformed JPEG")
        ));
    }

    #[test]
    fn indexed_png_without_palette_becomes_placeholder() {
        let output = parse_rtf(&format!(
            "{{\\rtf1{{\\pict\\pngblip {}}}}}",
            bytes_to_hex(&minimal_indexed_png_without_palette(1, 1))
        ))
        .unwrap();
        assert!(matches!(
            &output.document.blocks[0],
            Block::Placeholder(text) if text.contains("unsupported PNG")
        ));
    }

    #[test]
    fn unsupported_word_picture_formats_become_placeholders() {
        for control in ["wmetafile8", "emfblip", "macpict", "pmmetafile1", "wbitmap"] {
            let input = format!(r"{{\rtf1{{\pict\{control} 41424344}}}}");
            let output = parse_rtf(&input).unwrap();

            assert!(matches!(
                &output.document.blocks[0],
                Block::Placeholder(text) if text.contains("unsupported format")
            ));
            assert!(
                output
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("unsupported picture format"))
            );
        }
    }

    #[test]
    fn unsupported_dib_picture_becomes_placeholder() {
        let mut dib = minimal_24bit_dib_with_dimensions(1, 1);
        dib[16..20].copy_from_slice(&1u32.to_le_bytes());
        let input = format!("{{\\rtf1{{\\pict\\dibitmap {}}}}}", bytes_to_hex(&dib));
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Placeholder(text) if text.contains("unsupported DIB")
        ));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("DIB picture data was unsupported")
        }));
    }

    #[test]
    fn excessive_8bit_dib_palette_count_becomes_placeholder() {
        let mut dib = minimal_8bit_dib_with_dimensions(1, 1);
        dib[32..36].copy_from_slice(&257u32.to_le_bytes());
        let input = format!("{{\\rtf1{{\\pict\\dibitmap {}}}}}", bytes_to_hex(&dib));
        let output = parse_rtf(&input).unwrap();

        assert!(matches!(
            &output.document.blocks[0],
            Block::Placeholder(text) if text.contains("unsupported DIB")
        ));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("DIB picture data was unsupported")
        }));
    }

    #[test]
    fn image_pixel_limit_is_enforced() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_image_pixels: 10,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let input = format!(
            "{{\\rtf1{{\\pict\\jpegblip {}}}}}",
            bytes_to_hex(&minimal_jpeg_with_dimensions(11, 1))
        );
        assert!(matches!(
            parse_rtf_bytes_with_options(input.as_bytes(), &options),
            Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "image pixels"
        ));
    }

    #[test]
    fn png_image_pixel_limit_is_enforced() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_image_pixels: 10,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let input = format!(
            "{{\\rtf1{{\\pict\\pngblip {}}}}}",
            bytes_to_hex(&minimal_rgb_png_with_dimensions(11, 1))
        );
        assert!(matches!(
            parse_rtf_bytes_with_options(input.as_bytes(), &options),
            Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "image pixels"
        ));
    }
}

#[cfg(test)]
fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
fn minimal_jpeg_with_dimensions(width: u16, height: u16) -> Vec<u8> {
    minimal_jpeg_with_components(width, height, 3)
}

#[cfg(test)]
fn minimal_grayscale_jpeg_with_dimensions(width: u16, height: u16) -> Vec<u8> {
    minimal_jpeg_with_components(width, height, 1)
}

#[cfg(test)]
fn minimal_cmyk_jpeg_with_dimensions(width: u16, height: u16) -> Vec<u8> {
    minimal_jpeg_with_components(width, height, 4)
}

#[cfg(test)]
fn minimal_jpeg_with_components(width: u16, height: u16, components: u8) -> Vec<u8> {
    let [height_hi, height_lo] = height.to_be_bytes();
    let [width_hi, width_lo] = width.to_be_bytes();
    let segment_len = 8 + u16::from(components) * 3;
    let [segment_hi, segment_lo] = segment_len.to_be_bytes();
    let mut jpeg = vec![
        0xff, 0xd8, 0xff, 0xc0, segment_hi, segment_lo, 0x08, height_hi, height_lo, width_hi,
        width_lo, components,
    ];
    for component in 1..=components {
        jpeg.extend_from_slice(&[component, 0x11, 0x00]);
    }
    jpeg.extend_from_slice(&[0xff, 0xd9]);
    jpeg
}

#[cfg(test)]
fn minimal_24bit_dib_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    let row_stride = ((width as usize * 3) + 3) / 4 * 4;
    let pixel_bytes = row_stride * height as usize;
    let mut dib = Vec::with_capacity(40 + pixel_bytes);
    dib.extend_from_slice(&40u32.to_le_bytes());
    dib.extend_from_slice(&(width as i32).to_le_bytes());
    dib.extend_from_slice(&(height as i32).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&24u16.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());

    for _ in 0..height {
        let mut row = Vec::with_capacity(row_stride);
        for x in 0..width {
            if x % 2 == 0 {
                row.extend_from_slice(&[0, 0, 255]);
            } else {
                row.extend_from_slice(&[0, 255, 0]);
            }
        }
        row.resize(row_stride, 0);
        dib.extend_from_slice(&row);
    }
    dib
}

#[cfg(test)]
fn minimal_8bit_dib_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    minimal_indexed_dib_with_dimensions(width, height, 8)
}

#[cfg(test)]
fn minimal_4bit_dib_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    minimal_indexed_dib_with_dimensions(width, height, 4)
}

#[cfg(test)]
fn minimal_1bit_dib_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    minimal_indexed_dib_with_dimensions(width, height, 1)
}

#[cfg(test)]
fn minimal_indexed_dib_with_dimensions(width: u32, height: u32, bits_per_pixel: u16) -> Vec<u8> {
    let row_stride = ((width as usize * usize::from(bits_per_pixel)).div_ceil(32)) * 4;
    let pixel_bytes = row_stride * height as usize;
    let palette_entries = 2u32;
    let mut dib = Vec::with_capacity(40 + (palette_entries as usize * 4) + pixel_bytes);
    dib.extend_from_slice(&40u32.to_le_bytes());
    dib.extend_from_slice(&(width as i32).to_le_bytes());
    dib.extend_from_slice(&(height as i32).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&bits_per_pixel.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&palette_entries.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&[0, 0, 255, 0]);
    dib.extend_from_slice(&[0, 255, 0, 0]);

    for _ in 0..height {
        let mut row = indexed_dib_test_row(width, bits_per_pixel);
        row.resize(row_stride, 0);
        dib.extend_from_slice(&row);
    }
    dib
}

#[cfg(test)]
fn indexed_dib_test_row(width: u32, bits_per_pixel: u16) -> Vec<u8> {
    match bits_per_pixel {
        1 => {
            let mut row = Vec::new();
            let mut byte = 0u8;
            for x in 0..width {
                if x % 2 == 1 {
                    byte |= 1 << (7 - (x % 8));
                }
                if x % 8 == 7 {
                    row.push(byte);
                    byte = 0;
                }
            }
            if width % 8 != 0 {
                row.push(byte);
            }
            row
        }
        4 => {
            let mut row = Vec::new();
            let mut high = None;
            for x in 0..width {
                let index = (x % 2) as u8;
                if let Some(high_index) = high.take() {
                    row.push((high_index << 4) | index);
                } else {
                    high = Some(index);
                }
            }
            if let Some(high_index) = high {
                row.push(high_index << 4);
            }
            row
        }
        8 => (0..width).map(|x| (x % 2) as u8).collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
fn minimal_rgb_png_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    push_png_chunk(&mut png, b"IHDR", &ihdr);

    // Zlib-wrapped deflate store block for one RGB scanline with filter byte 0.
    let idat = [
        0x78, 0x01, 0x01, 0x04, 0x00, 0xfb, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x01,
    ];
    push_png_chunk(&mut png, b"IDAT", &idat);
    push_png_chunk(&mut png, b"IEND", &[]);
    png
}

#[cfg(test)]
fn minimal_grayscale_png_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 0, 0, 0, 0]);
    push_png_chunk(&mut png, b"IHDR", &ihdr);

    // Zlib-wrapped deflate store block for one grayscale scanline with filter byte 0.
    let idat = [
        0x78, 0x01, 0x01, 0x02, 0x00, 0xfd, 0xff, 0x00, 0x80, 0x00, 0x81, 0x00, 0x81,
    ];
    push_png_chunk(&mut png, b"IDAT", &idat);
    push_png_chunk(&mut png, b"IEND", &[]);
    png
}

#[cfg(test)]
fn minimal_indexed_png_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    minimal_indexed_png(width, height, true)
}

#[cfg(test)]
fn minimal_indexed_png_without_palette(width: u32, height: u32) -> Vec<u8> {
    minimal_indexed_png(width, height, false)
}

#[cfg(test)]
fn minimal_indexed_png(width: u32, height: u32, include_palette: bool) -> Vec<u8> {
    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 3, 0, 0, 0]);
    push_png_chunk(&mut png, b"IHDR", &ihdr);
    if include_palette {
        push_png_chunk(&mut png, b"PLTE", &[255, 0, 0, 0, 255, 0]);
    }

    // Zlib-wrapped deflate store block for one indexed scanline with filter byte 0.
    let idat = [
        0x78, 0x01, 0x01, 0x02, 0x00, 0xfd, 0xff, 0x00, 0x01, 0x00, 0x02, 0x00, 0x02,
    ];
    push_png_chunk(&mut png, b"IDAT", &idat);
    push_png_chunk(&mut png, b"IEND", &[]);
    png
}

#[cfg(test)]
fn push_png_chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(kind);
    png.extend_from_slice(data);
    png.extend_from_slice(&0u32.to_be_bytes());
}
