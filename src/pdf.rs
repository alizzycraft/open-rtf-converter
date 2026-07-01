use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};

use crate::layout::{LayoutDocument, LayoutItem, PdfColor};
use crate::model::CharacterStyle;

const FONT_REGULAR: &[u8] = b"F1";
const FONT_BOLD: &[u8] = b"F2";
const FONT_ITALIC: &[u8] = b"F3";
const FONT_BOLD_ITALIC: &[u8] = b"F4";

pub fn render_pdf(layout: &LayoutDocument) -> Vec<u8> {
    let mut pdf = Pdf::new();
    let catalog_id = Ref::new(1);
    let page_tree_id = Ref::new(2);
    let first_page_id = 3;
    let first_content_id = first_page_id + layout.pages.len() as i32;
    let font_regular_id = Ref::new(first_content_id + layout.pages.len() as i32);
    let font_bold_id = Ref::new(first_content_id + layout.pages.len() as i32 + 1);
    let font_italic_id = Ref::new(first_content_id + layout.pages.len() as i32 + 2);
    let font_bold_italic_id = Ref::new(first_content_id + layout.pages.len() as i32 + 3);

    let page_refs = (0..layout.pages.len())
        .map(|idx| Ref::new(first_page_id + idx as i32))
        .collect::<Vec<_>>();

    pdf.catalog(catalog_id).pages(page_tree_id);
    pdf.pages(page_tree_id)
        .kids(page_refs.iter().copied())
        .count(page_refs.len() as i32);

    for (idx, page) in layout.pages.iter().enumerate() {
        let page_id = page_refs[idx];
        let content_id = Ref::new(first_content_id + idx as i32);

        let mut page_writer = pdf.page(page_id);
        page_writer.parent(page_tree_id);
        page_writer.media_box(Rect::new(0.0, 0.0, layout.width, layout.height));
        page_writer.contents(content_id);
        {
            let mut resources = page_writer.resources();
            let mut fonts = resources.fonts();
            fonts.pair(Name(FONT_REGULAR), font_regular_id);
            fonts.pair(Name(FONT_BOLD), font_bold_id);
            fonts.pair(Name(FONT_ITALIC), font_italic_id);
            fonts.pair(Name(FONT_BOLD_ITALIC), font_bold_italic_id);
        }
        page_writer.finish();

        let mut content = Content::new();
        for item in &page.items {
            match item {
                LayoutItem::Text(fragment) => {
                    set_fill_color(&mut content, PdfColor::default());
                    content.begin_text();
                    content.set_font(
                        font_name_for_style(&fragment.style),
                        fragment.style.font_size_points(),
                    );
                    content.next_line(fragment.x, fragment.baseline_y);
                    let encoded = encode_pdf_text(&fragment.text);
                    content.show(Str(&encoded));
                    content.end_text();
                }
                LayoutItem::Underline { x, y, width, color } => {
                    set_stroke_color(&mut content, *color);
                    content.set_line_width(0.5);
                    content.move_to(*x, *y);
                    content.line_to(*x + *width, *y);
                    content.stroke();
                }
            }
        }
        pdf.stream(content_id, &content.finish());
    }

    pdf.type1_font(font_regular_id)
        .base_font(Name(b"Helvetica"));
    pdf.type1_font(font_bold_id)
        .base_font(Name(b"Helvetica-Bold"));
    pdf.type1_font(font_italic_id)
        .base_font(Name(b"Helvetica-Oblique"));
    pdf.type1_font(font_bold_italic_id)
        .base_font(Name(b"Helvetica-BoldOblique"));

    pdf.finish()
}

fn font_name_for_style(style: &CharacterStyle) -> Name<'static> {
    match (style.bold, style.italic) {
        (true, true) => Name(FONT_BOLD_ITALIC),
        (true, false) => Name(FONT_BOLD),
        (false, true) => Name(FONT_ITALIC),
        (false, false) => Name(FONT_REGULAR),
    }
}

fn encode_pdf_text(text: &str) -> Vec<u8> {
    text.chars()
        .map(|ch| match ch {
            '\u{2018}' | '\u{2019}' => b'\'',
            '\u{201c}' | '\u{201d}' => b'"',
            '\u{2013}' | '\u{2014}' => b'-',
            '\u{00a0}' => b' ',
            ch if ch.is_ascii() => ch as u8,
            _ => b'?',
        })
        .collect()
}

fn set_fill_color(content: &mut Content, color: PdfColor) {
    content.set_fill_rgb(color.red, color.green, color.blue);
}

fn set_stroke_color(content: &mut Content, color: PdfColor) {
    content.set_stroke_rgb(color.red, color.green, color.blue);
}

#[cfg(test)]
mod tests {
    use crate::layout::LayoutEngine;
    use crate::model::{Block, Document, Paragraph, Run};

    use super::*;

    #[test]
    fn writes_valid_pdf_bytes() {
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Hello PDF".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
    }
}
