use unicode_linebreak::{BreakOpportunity, linebreaks};

use crate::model::{Alignment, Block, CharacterStyle, Document, Paragraph, ParagraphStyle, Run};

const TWIPS_PER_POINT: f32 = 20.0;

#[derive(Debug, Clone)]
pub struct LayoutDocument {
    pub width: f32,
    pub height: f32,
    pub pages: Vec<LayoutPage>,
}

#[derive(Debug, Clone)]
pub struct LayoutPage {
    pub items: Vec<LayoutItem>,
}

#[derive(Debug, Clone)]
pub enum LayoutItem {
    Text(TextFragment),
    Underline {
        x: f32,
        y: f32,
        width: f32,
        color: PdfColor,
    },
}

#[derive(Debug, Clone)]
pub struct TextFragment {
    pub text: String,
    pub x: f32,
    pub baseline_y: f32,
    pub style: CharacterStyle,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PdfColor {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
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

#[derive(Debug, Clone)]
struct FlowRun {
    text: String,
    style: CharacterStyle,
}

#[derive(Debug, Clone)]
struct Line {
    runs: Vec<FlowRun>,
    width: f32,
    height: f32,
}

impl LayoutEngine {
    pub fn layout(document: &Document) -> LayoutDocument {
        let width = twips_to_points(document.page.width_twips);
        let height = twips_to_points(document.page.height_twips);
        let margin_left = twips_to_points(document.page.margin_left_twips);
        let margin_right = twips_to_points(document.page.margin_right_twips);
        let margin_top = twips_to_points(document.page.margin_top_twips);
        let margin_bottom = twips_to_points(document.page.margin_bottom_twips);
        let content_width = (width - margin_left - margin_right).max(72.0);

        let mut pages = vec![LayoutPage { items: Vec::new() }];
        let mut cursor_y = height - margin_top;

        for block in &document.blocks {
            match block {
                Block::PageBreak => {
                    pages.push(LayoutPage { items: Vec::new() });
                    cursor_y = height - margin_top;
                }
                Block::Paragraph(paragraph) => {
                    let paragraph_top = twips_to_points(paragraph.style.space_before_twips);
                    cursor_y -= paragraph_top;
                    let lines = wrap_paragraph(paragraph, content_width);
                    for line in lines {
                        if cursor_y - line.height < margin_bottom {
                            pages.push(LayoutPage { items: Vec::new() });
                            cursor_y = height - margin_top;
                        }

                        let x = aligned_x(margin_left, content_width, line.width, &paragraph.style);
                        push_line(&mut pages, &line, x, cursor_y, document);
                        cursor_y -= line.height;
                    }
                    cursor_y -= twips_to_points(paragraph.style.space_after_twips);
                }
            }
        }

        LayoutDocument {
            width,
            height,
            pages,
        }
    }
}

fn wrap_paragraph(paragraph: &Paragraph, content_width: f32) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut current = Line {
        runs: Vec::new(),
        width: 0.0,
        height: 14.0,
    };

    for run in &paragraph.runs {
        for segment in split_run_for_wrapping(run) {
            if segment.text == "\n" {
                lines.push(current);
                current = empty_line();
                continue;
            }

            let width = measure_text(&segment.text, &segment.style);
            if current.width > 0.0 && current.width + width > content_width {
                lines.push(current);
                current = empty_line();
                let trimmed = segment.text.trim_start().to_string();
                if trimmed.is_empty() {
                    continue;
                }
                push_segment(
                    &mut current,
                    FlowRun {
                        text: trimmed,
                        style: segment.style,
                    },
                );
            } else {
                push_segment(&mut current, segment);
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

fn split_run_for_wrapping(run: &Run) -> Vec<FlowRun> {
    let mut output = Vec::new();
    let mut start = 0;

    for (idx, opportunity) in linebreaks(&run.text) {
        if matches!(
            opportunity,
            BreakOpportunity::Allowed | BreakOpportunity::Mandatory
        ) {
            let text = run.text[start..idx].replace('\t', "    ");
            if !text.is_empty() {
                output.push(FlowRun {
                    text,
                    style: run.style.clone(),
                });
            }
            if matches!(opportunity, BreakOpportunity::Mandatory) {
                output.push(FlowRun {
                    text: "\n".to_string(),
                    style: run.style.clone(),
                });
            }
            start = idx;
        }
    }

    if start < run.text.len() {
        output.push(FlowRun {
            text: run.text[start..].replace('\t', "    "),
            style: run.style.clone(),
        });
    }

    output
}

fn empty_line() -> Line {
    Line {
        runs: Vec::new(),
        width: 0.0,
        height: 14.0,
    }
}

fn push_segment(line: &mut Line, segment: FlowRun) {
    line.width += measure_text(&segment.text, &segment.style);
    line.height = line.height.max(segment.style.font_size_points() * 1.25);
    line.runs.push(segment);
}

fn push_line(pages: &mut [LayoutPage], line: &Line, x: f32, top_y: f32, document: &Document) {
    let baseline_y = top_y - line.height + (line.height * 0.25);
    let mut cursor_x = x;
    let page = pages.last_mut().expect("layout always has a page");

    for run in &line.runs {
        let width = measure_text(&run.text, &run.style);
        page.items.push(LayoutItem::Text(TextFragment {
            text: run.text.clone(),
            x: cursor_x,
            baseline_y,
            style: run.style.clone(),
        }));

        if run.style.underline {
            page.items.push(LayoutItem::Underline {
                x: cursor_x,
                y: baseline_y - 2.0,
                width,
                color: document
                    .colors
                    .get(run.style.color_index)
                    .map(|color| PdfColor {
                        red: color.red as f32 / 255.0,
                        green: color.green as f32 / 255.0,
                        blue: color.blue as f32 / 255.0,
                    })
                    .unwrap_or_default(),
            });
        }

        cursor_x += width;
    }
}

fn aligned_x(left: f32, content_width: f32, line_width: f32, style: &ParagraphStyle) -> f32 {
    let left = left + twips_to_points(style.left_indent_twips + style.first_line_indent_twips);
    let available =
        content_width - twips_to_points(style.left_indent_twips + style.right_indent_twips);
    match style.alignment {
        Alignment::Left | Alignment::Justified => left,
        Alignment::Center => left + ((available - line_width).max(0.0) / 2.0),
        Alignment::Right => left + (available - line_width).max(0.0),
    }
}

pub fn measure_text(text: &str, style: &CharacterStyle) -> f32 {
    let size = style.font_size_points();
    let weight = if style.bold { 0.56 } else { 0.52 };
    text.chars()
        .map(|ch| match ch {
            'i' | 'l' | '!' | '.' | ',' | ':' | ';' | '\'' => size * 0.25,
            'm' | 'w' | 'M' | 'W' => size * 0.78,
            ' ' | '\u{00a0}' => size * 0.28,
            _ => size * weight,
        })
        .sum()
}

pub fn twips_to_points(twips: i32) -> f32 {
    twips as f32 / TWIPS_PER_POINT
}

#[cfg(test)]
mod tests {
    use crate::model::{Block, Document, Paragraph, Run};

    use super::*;

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
}
