#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub page: PageSettings,
    pub fonts: Vec<FontDef>,
    pub colors: Vec<Color>,
    pub blocks: Vec<Block>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            page: PageSettings::default(),
            fonts: vec![FontDef {
                index: 0,
                name: "Helvetica".to_string(),
            }],
            colors: vec![Color::default()],
            blocks: vec![Block::Paragraph(Paragraph::default())],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageSettings {
    pub width_twips: i32,
    pub height_twips: i32,
    pub margin_left_twips: i32,
    pub margin_right_twips: i32,
    pub margin_top_twips: i32,
    pub margin_bottom_twips: i32,
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontDef {
    pub index: i32,
    pub name: String,
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
    PageBreak,
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParagraphStyle {
    pub alignment: Alignment,
    pub left_indent_twips: i32,
    pub right_indent_twips: i32,
    pub first_line_indent_twips: i32,
    pub space_before_twips: i32,
    pub space_after_twips: i32,
}

impl Default for ParagraphStyle {
    fn default() -> Self {
        Self {
            alignment: Alignment::Left,
            left_indent_twips: 0,
            right_indent_twips: 0,
            first_line_indent_twips: 0,
            space_before_twips: 0,
            space_after_twips: 120,
        }
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
    pub underline: bool,
    pub font_index: i32,
    pub font_size_half_points: i32,
    pub color_index: usize,
    pub highlight_index: Option<usize>,
}

impl Default for CharacterStyle {
    fn default() -> Self {
        Self {
            bold: false,
            italic: false,
            underline: false,
            font_index: 0,
            font_size_half_points: 24,
            color_index: 0,
            highlight_index: None,
        }
    }
}

impl CharacterStyle {
    pub fn font_size_points(&self) -> f32 {
        self.font_size_half_points.max(2) as f32 / 2.0
    }
}
