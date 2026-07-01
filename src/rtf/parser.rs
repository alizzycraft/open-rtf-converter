use thiserror::Error;

use crate::diagnostics::Diagnostic;
use crate::model::{
    Alignment, Block, CharacterStyle, Color, Document, FontDef, PageSettings, Paragraph,
    ParagraphStyle, Run,
};

use super::lexer::{Control, LexError, Lexer, Token, TokenKind};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error(transparent)]
    Lex(#[from] LexError),
    #[error("unbalanced RTF group ending at byte {0}")]
    UnbalancedGroup(usize),
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
    unicode_skip: usize,
    skip_bytes: usize,
    destination: Destination,
}

impl Default for ParserState {
    fn default() -> Self {
        Self {
            character: CharacterStyle::default(),
            paragraph: ParagraphStyle::default(),
            unicode_skip: 1,
            skip_bytes: 0,
            destination: Destination::Body,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Destination {
    Body,
    FontTable,
    ColorTable,
    StyleSheet,
    Header,
    Footer,
    Ignored,
}

struct Parser {
    tokens: Vec<Token>,
    document: Document,
    current_paragraph: Paragraph,
    state: ParserState,
    stack: Vec<ParserState>,
    diagnostics: Vec<Diagnostic>,
    current_font: Option<FontDef>,
    current_color: Color,
    current_color_seen: bool,
}

pub fn parse_rtf(input: &str) -> Result<ParseOutput, ParseError> {
    let tokens = Lexer::new(input).tokenize()?;
    Parser::new(tokens).parse()
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        let state = ParserState::default();
        Self {
            tokens,
            document: Document::default(),
            current_paragraph: Paragraph {
                style: state.paragraph.clone(),
                runs: Vec::new(),
            },
            state,
            stack: Vec::new(),
            diagnostics: Vec::new(),
            current_font: None,
            current_color: Color::default(),
            current_color_seen: false,
        }
    }

    fn parse(mut self) -> Result<ParseOutput, ParseError> {
        let tokens = std::mem::take(&mut self.tokens);
        for token in &tokens {
            match &token.kind {
                TokenKind::StartGroup => self.start_group(),
                TokenKind::EndGroup => self.end_group(token.offset)?,
                TokenKind::Control(control) => self.apply_control(control, token.offset),
                TokenKind::Text(text) => self.apply_text(text, token.offset),
            }
        }

        if !self.stack.is_empty() {
            let offset = self.tokens.last().map(|token| token.offset).unwrap_or(0);
            return Err(ParseError::UnbalancedGroup(offset));
        }

        self.finish_paragraph();
        self.document.blocks.retain(|block| match block {
            Block::Paragraph(paragraph) => !paragraph.runs.is_empty(),
            Block::PageBreak => true,
        });

        Ok(ParseOutput {
            document: self.document,
            diagnostics: self.diagnostics,
        })
    }

    fn start_group(&mut self) {
        self.stack.push(self.state.clone());
    }

    fn end_group(&mut self, offset: usize) -> Result<(), ParseError> {
        if let Some(previous) = self.stack.pop() {
            if self.state.destination == Destination::FontTable {
                self.flush_font();
            }
            self.state = previous;
            Ok(())
        } else {
            Err(ParseError::UnbalancedGroup(offset))
        }
    }

    fn apply_control(&mut self, control: &Control, offset: usize) {
        if self.state.skip_bytes > 0 && control.name != "u" {
            return;
        }

        match control.name.as_str() {
            "rtf" | "ansi" | "deff" | "viewkind" | "generator" => {}
            "*" => self.state.destination = Destination::Ignored,
            "fonttbl" => self.state.destination = Destination::FontTable,
            "colortbl" => self.state.destination = Destination::ColorTable,
            "stylesheet" => self.state.destination = Destination::StyleSheet,
            "header" | "headerl" | "headerr" | "headerf" => {
                self.state.destination = Destination::Header
            }
            "footer" | "footerl" | "footerr" | "footerf" => {
                self.state.destination = Destination::Footer
            }
            "uc" => self.state.unicode_skip = control.parameter.unwrap_or(1).max(0) as usize,
            "u" => self.push_unicode(control.parameter.unwrap_or(0)),
            "par" => self.finish_paragraph(),
            "line" => self.push_text("\n"),
            "tab" => self.push_text("\t"),
            "page" => {
                self.finish_paragraph();
                self.document.blocks.push(Block::PageBreak);
            }
            "b" => self.state.character.bold = control.parameter.unwrap_or(1) != 0,
            "i" => self.state.character.italic = control.parameter.unwrap_or(1) != 0,
            "ul" => self.state.character.underline = control.parameter.unwrap_or(1) != 0,
            "ulnone" => self.state.character.underline = false,
            "plain" => self.state.character = CharacterStyle::default(),
            "fs" => {
                self.state.character.font_size_half_points = control.parameter.unwrap_or(24).max(2)
            }
            "f" if self.state.destination == Destination::FontTable => {
                self.flush_font();
                self.current_font = Some(FontDef {
                    index: control.parameter.unwrap_or(0),
                    name: String::new(),
                });
            }
            "f" => self.state.character.font_index = control.parameter.unwrap_or(0),
            "cf" => {
                self.state.character.color_index = control.parameter.unwrap_or(0).max(0) as usize
            }
            "highlight" | "chshdng" => {
                self.state.character.highlight_index =
                    Some(control.parameter.unwrap_or(0).max(0) as usize);
            }
            "ql" => self.state.paragraph.alignment = Alignment::Left,
            "qc" => self.state.paragraph.alignment = Alignment::Center,
            "qr" => self.state.paragraph.alignment = Alignment::Right,
            "qj" => self.state.paragraph.alignment = Alignment::Justified,
            "li" => self.state.paragraph.left_indent_twips = control.parameter.unwrap_or(0),
            "ri" => self.state.paragraph.right_indent_twips = control.parameter.unwrap_or(0),
            "fi" => self.state.paragraph.first_line_indent_twips = control.parameter.unwrap_or(0),
            "sb" => self.state.paragraph.space_before_twips = control.parameter.unwrap_or(0).max(0),
            "sa" => self.state.paragraph.space_after_twips = control.parameter.unwrap_or(0).max(0),
            "pard" => self.state.paragraph = ParagraphStyle::default(),
            "paperw" => {
                self.document.page.width_twips = control
                    .parameter
                    .unwrap_or(PageSettings::default().width_twips)
            }
            "paperh" => {
                self.document.page.height_twips = control
                    .parameter
                    .unwrap_or(PageSettings::default().height_twips)
            }
            "margl" => {
                self.document.page.margin_left_twips = control
                    .parameter
                    .unwrap_or(PageSettings::default().margin_left_twips)
            }
            "margr" => {
                self.document.page.margin_right_twips = control
                    .parameter
                    .unwrap_or(PageSettings::default().margin_right_twips)
            }
            "margt" => {
                self.document.page.margin_top_twips = control
                    .parameter
                    .unwrap_or(PageSettings::default().margin_top_twips)
            }
            "margb" => {
                self.document.page.margin_bottom_twips = control
                    .parameter
                    .unwrap_or(PageSettings::default().margin_bottom_twips)
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
            name if is_known_ignored_control(name) => {}
            name => self.diagnostics.push(Diagnostic::warning(
                format!("unsupported RTF control '\\{name}'"),
                Some(offset),
            )),
        }
    }

    fn apply_text(&mut self, text: &str, _offset: usize) {
        if self.state.skip_bytes > 0 {
            let skip = self.state.skip_bytes.min(text.chars().count());
            self.state.skip_bytes -= skip;
            let remaining: String = text.chars().skip(skip).collect();
            if remaining.is_empty() {
                return;
            }
            self.apply_text(&remaining, _offset);
            return;
        }

        match self.state.destination {
            Destination::Body => self.push_text(text),
            Destination::FontTable => self.push_font_text(text),
            Destination::ColorTable => self.push_color_text(text),
            Destination::StyleSheet
            | Destination::Header
            | Destination::Footer
            | Destination::Ignored => {}
        }
    }

    fn push_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        if self.current_paragraph.style != self.state.paragraph {
            if self.current_paragraph.runs.is_empty() {
                self.current_paragraph.style = self.state.paragraph.clone();
            }
        }

        if let Some(last) = self.current_paragraph.runs.last_mut() {
            if last.style == self.state.character {
                last.text.push_str(text);
                return;
            }
        }

        self.current_paragraph.runs.push(Run {
            text: text.to_string(),
            style: self.state.character.clone(),
        });
    }

    fn push_unicode(&mut self, value: i32) {
        let unsigned = if value < 0 {
            (value + 65_536) as u32
        } else {
            value as u32
        };

        if let Some(ch) = char::from_u32(unsigned) {
            self.push_text(&ch.to_string());
        }
        self.state.skip_bytes = self.state.unicode_skip;
    }

    fn finish_paragraph(&mut self) {
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

    fn push_font_text(&mut self, text: &str) {
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
                self.flush_font();
            }
        }
    }

    fn flush_font(&mut self) {
        let Some(mut font) = self.current_font.take() else {
            return;
        };

        font.name = font.name.trim().trim_end_matches(';').to_string();
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
            self.document.fonts.push(font);
        }
    }

    fn push_color_text(&mut self, text: &str) {
        for ch in text.chars() {
            if ch == ';' {
                if self.current_color_seen {
                    self.document.colors.push(self.current_color);
                }
                self.current_color = Color::default();
                self.current_color_seen = false;
            }
        }
    }
}

fn is_known_ignored_control(name: &str) -> bool {
    matches!(
        name,
        "ansicpg"
            | "cocoartf"
            | "cocoasubrtf"
            | "fbidis"
            | "fromtext"
            | "fcharset"
            | "fprq"
            | "fmodern"
            | "fnil"
            | "froman"
            | "fswiss"
            | "fscript"
            | "fdecor"
            | "ftech"
            | "listtext"
            | "ltrch"
            | "ltrpar"
            | "nowidctlpar"
            | "saauto"
            | "sl"
            | "slmult"
            | "widctlpar"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn parses_font_and_color_tables() {
        let input = r"{\rtf1{\fonttbl{\f0\fswiss Arial;}{\f1 Courier New;}}{\colortbl;\red255\green0\blue0;}Hello}";
        let output = parse_rtf(input).unwrap();
        assert!(
            output
                .document
                .fonts
                .iter()
                .any(|font| font.name == "Arial")
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
}
