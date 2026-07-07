use std::error::Error;
use std::fmt;

use pdf_writer::types::{Predictor, TextRenderingMode};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str};

use crate::layout::{
    LayoutDocument, LayoutItem, LineStyle, PdfColor, PdfFontFamily, TextFragment,
    passive_pair_kerning_points, style_uses_passive_kerning, twips_to_points,
};
use crate::model::{
    CharacterEmphasisMark, CharacterStyle, ImageFormat, StaticImageVectorCommand, TextRelief,
    UnderlineStyle,
};

const HELVETICA_REGULAR: &[u8] = b"F1";
const HELVETICA_BOLD: &[u8] = b"F2";
const HELVETICA_ITALIC: &[u8] = b"F3";
const HELVETICA_BOLD_ITALIC: &[u8] = b"F4";
const COURIER_REGULAR: &[u8] = b"F5";
const COURIER_BOLD: &[u8] = b"F6";
const COURIER_ITALIC: &[u8] = b"F7";
const COURIER_BOLD_ITALIC: &[u8] = b"F8";
const TIMES_REGULAR: &[u8] = b"F9";
const TIMES_BOLD: &[u8] = b"F10";
const TIMES_ITALIC: &[u8] = b"F11";
const TIMES_BOLD_ITALIC: &[u8] = b"F12";
const SYMBOL_REGULAR: &[u8] = b"F13";
const ZAPF_DINGBATS_REGULAR: &[u8] = b"F14";

const BUILTIN_FONTS: [(&[u8], &[u8]); 14] = [
    (HELVETICA_REGULAR, b"Helvetica"),
    (HELVETICA_BOLD, b"Helvetica-Bold"),
    (HELVETICA_ITALIC, b"Helvetica-Oblique"),
    (HELVETICA_BOLD_ITALIC, b"Helvetica-BoldOblique"),
    (COURIER_REGULAR, b"Courier"),
    (COURIER_BOLD, b"Courier-Bold"),
    (COURIER_ITALIC, b"Courier-Oblique"),
    (COURIER_BOLD_ITALIC, b"Courier-BoldOblique"),
    (TIMES_REGULAR, b"Times-Roman"),
    (TIMES_BOLD, b"Times-Bold"),
    (TIMES_ITALIC, b"Times-Italic"),
    (TIMES_BOLD_ITALIC, b"Times-BoldItalic"),
    (SYMBOL_REGULAR, b"Symbol"),
    (ZAPF_DINGBATS_REGULAR, b"ZapfDingbats"),
];

const ACTIVE_PDF_NAME_TOKENS: &[(&[u8], &str)] = &[
    (b"/3D", "/3D"),
    (b"/AA", "/AA"),
    (b"/AF", "/AF"),
    (b"/AcroForm", "/AcroForm"),
    (b"/Action", "/Action"),
    (b"/Annot", "/Annot"),
    (b"/Annots", "/Annots"),
    (b"/Collection", "/Collection"),
    (b"/EmbeddedFile", "/EmbeddedFile"),
    (b"/Encrypt", "/Encrypt"),
    (b"/FileAttachment", "/FileAttachment"),
    (b"/Filespec", "/Filespec"),
    (b"/GoTo", "/GoTo"),
    (b"/GoToE", "/GoToE"),
    (b"/GoToR", "/GoToR"),
    (b"/Hide", "/Hide"),
    (b"/ImportData", "/ImportData"),
    (b"/JavaScript", "/JavaScript"),
    (b"/JS", "/JS"),
    (b"/Launch", "/Launch"),
    (b"/Link", "/Link"),
    (b"/Movie", "/Movie"),
    (b"/Names", "/Names"),
    (b"/Named", "/Named"),
    (b"/OpenAction", "/OpenAction"),
    (b"/Perms", "/Perms"),
    (b"/Rendition", "/Rendition"),
    (b"/ResetForm", "/ResetForm"),
    (b"/RichMedia", "/RichMedia"),
    (b"/Screen", "/Screen"),
    (b"/Sound", "/Sound"),
    (b"/SubmitForm", "/SubmitForm"),
    (b"/URI", "/URI"),
    (b"/Widget", "/Widget"),
    (b"/XFA", "/XFA"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassivePdfIssue {
    pub token: &'static str,
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassivePdfError {
    pub issues: Vec<PassivePdfIssue>,
}

impl fmt::Display for PassivePdfError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(issue) = self.issues.first() {
            write!(
                formatter,
                "passive PDF audit found forbidden token {} at byte {}",
                issue.token, issue.offset
            )
        } else {
            formatter.write_str("passive PDF audit failed")
        }
    }
}

impl Error for PassivePdfError {}

pub fn audit_passive_pdf_bytes(pdf: &[u8]) -> Result<(), PassivePdfError> {
    let mut issues = Vec::new();
    let mut offset = 0;

    while offset < pdf.len() {
        if is_stream_marker_at(pdf, offset) {
            let Some(stream_start) = stream_data_start(pdf, offset) else {
                issues.push(PassivePdfIssue {
                    token: "<malformed stream>",
                    offset,
                });
                break;
            };
            let Some(stream_end) = find_endstream_marker(pdf, stream_start) else {
                issues.push(PassivePdfIssue {
                    token: "<unterminated stream>",
                    offset,
                });
                break;
            };
            offset = stream_end + b"endstream".len();
            continue;
        }

        if pdf[offset] == b'/' {
            for (token, label) in ACTIVE_PDF_NAME_TOKENS {
                if pdf[offset..].starts_with(token)
                    && is_pdf_name_boundary(pdf.get(offset + token.len()).copied())
                {
                    issues.push(PassivePdfIssue {
                        token: label,
                        offset,
                    });
                }
            }
        }

        offset += 1;
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(PassivePdfError { issues })
    }
}

pub fn render_pdf(layout: &LayoutDocument) -> Vec<u8> {
    let mut pdf = Pdf::new();
    let catalog_id = Ref::new(1);
    let page_tree_id = Ref::new(2);
    let first_page_id = 3;
    let first_content_id = first_page_id + layout.pages.len() as i32;
    let first_font_id = first_content_id + layout.pages.len() as i32;
    let first_image_id = first_font_id + BUILTIN_FONTS.len() as i32;

    let page_refs = (0..layout.pages.len())
        .map(|idx| Ref::new(first_page_id + idx as i32))
        .collect::<Vec<_>>();
    let image_refs = collect_image_refs(layout, first_image_id);

    pdf.catalog(catalog_id).pages(page_tree_id);
    pdf.pages(page_tree_id)
        .kids(page_refs.iter().copied())
        .count(page_refs.len() as i32);

    for (idx, page) in layout.pages.iter().enumerate() {
        let page_id = page_refs[idx];
        let content_id = Ref::new(first_content_id + idx as i32);

        let mut page_writer = pdf.page(page_id);
        page_writer.parent(page_tree_id);
        page_writer.media_box(Rect::new(0.0, 0.0, page.width, page.height));
        page_writer.contents(content_id);
        {
            let mut resources = page_writer.resources();
            {
                let mut fonts = resources.fonts();
                for (idx, (resource_name, _base_font)) in BUILTIN_FONTS.iter().enumerate() {
                    fonts.pair(Name(resource_name), Ref::new(first_font_id + idx as i32));
                }
            }
            if let Some(page_images) = image_refs.get(idx)
                && !page_images.is_empty()
            {
                let mut x_objects = resources.x_objects();
                for image in page_images {
                    x_objects.pair(Name(&image.name), image.id);
                }
            }
        }
        page_writer.finish();

        let mut content = Content::new();
        for (item_idx, item) in page.items.iter().enumerate() {
            match item {
                LayoutItem::Highlight {
                    x,
                    y,
                    width,
                    height,
                    color,
                } => {
                    set_fill_color(&mut content, *color);
                    content.rect(*x, *y, *width, *height);
                    content.fill_nonzero();
                }
                LayoutItem::Text(fragment) => {
                    let encoded = encode_pdf_text_for_font(&fragment.text, fragment.font_family);
                    if fragment.style.shadow {
                        set_fill_color(&mut content, shadow_color(fragment.color));
                        write_text_fragment(
                            &mut content,
                            &fragment.text,
                            fragment.font_family,
                            &fragment.style,
                            fragment.word_spacing,
                            fragment.x + shadow_offset(&fragment.style),
                            fragment.baseline_y - shadow_offset(&fragment.style),
                            &encoded,
                            TextRenderingMode::Fill,
                        );
                    }
                    if fragment.style.relief != TextRelief::None {
                        let offset = relief_offset(&fragment.style);
                        let (first_color, first_dx, first_dy, second_color, second_dx, second_dy) =
                            relief_layers(fragment.color, fragment.style.relief, offset);
                        set_fill_color(&mut content, first_color);
                        write_text_fragment(
                            &mut content,
                            &fragment.text,
                            fragment.font_family,
                            &fragment.style,
                            fragment.word_spacing,
                            fragment.x + first_dx,
                            fragment.baseline_y + first_dy,
                            &encoded,
                            TextRenderingMode::Fill,
                        );
                        set_fill_color(&mut content, second_color);
                        write_text_fragment(
                            &mut content,
                            &fragment.text,
                            fragment.font_family,
                            &fragment.style,
                            fragment.word_spacing,
                            fragment.x + second_dx,
                            fragment.baseline_y + second_dy,
                            &encoded,
                            TextRenderingMode::Fill,
                        );
                    }
                    set_fill_color(&mut content, fragment.color);
                    if fragment.style.outline {
                        set_stroke_color(&mut content, fragment.color);
                        content.set_line_width(
                            (fragment.style.font_size_points() * 0.035).clamp(0.25, 1.25),
                        );
                    }
                    write_text_fragment(
                        &mut content,
                        &fragment.text,
                        fragment.font_family,
                        &fragment.style,
                        fragment.word_spacing,
                        fragment.x,
                        fragment.baseline_y,
                        &encoded,
                        if fragment.style.outline {
                            TextRenderingMode::Stroke
                        } else {
                            TextRenderingMode::Fill
                        },
                    );
                    draw_passive_text_overlays(&mut content, fragment);
                }
                LayoutItem::Underline {
                    x,
                    y,
                    width,
                    color,
                    style,
                } => {
                    draw_underline(&mut content, *x, *y, *width, *color, *style);
                }
                LayoutItem::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    width,
                    color,
                    style,
                } => {
                    draw_passive_line(&mut content, *x1, *y1, *x2, *y2, *width, *color, *style);
                }
                LayoutItem::Ellipse {
                    x,
                    y,
                    width,
                    height,
                    stroke_width,
                    stroke_color,
                    stroke_style,
                    fill_color,
                } => {
                    draw_passive_ellipse(
                        &mut content,
                        *x,
                        *y,
                        *width,
                        *height,
                        *stroke_width,
                        *stroke_color,
                        *stroke_style,
                        *fill_color,
                    );
                }
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
                } => {
                    draw_passive_rounded_rectangle(
                        &mut content,
                        *x,
                        *y,
                        *width,
                        *height,
                        *radius,
                        *stroke_width,
                        *stroke_color,
                        *stroke_style,
                        *fill_color,
                    );
                }
                LayoutItem::Polygon {
                    points,
                    stroke_width,
                    stroke_color,
                    stroke_style,
                    fill_color,
                } => {
                    draw_passive_polygon(
                        &mut content,
                        points,
                        *stroke_width,
                        *stroke_color,
                        *stroke_style,
                        *fill_color,
                    );
                }
                LayoutItem::Image(fragment) => {
                    if fragment.image.format == ImageFormat::Placeholder {
                        draw_passive_image_placeholder(&mut content, fragment);
                        continue;
                    }
                    if fragment.image.format == ImageFormat::WmfVector {
                        draw_passive_wmf_vector_image(&mut content, fragment);
                        continue;
                    }
                    if let Some(image_ref) = image_refs.get(idx).and_then(|page_images| {
                        page_images
                            .iter()
                            .find(|image| image.item_index == item_idx)
                    }) {
                        let draw = image_draw_rect(fragment);
                        content.save_state();
                        if draw.clipped {
                            content.rect(fragment.x, fragment.y, fragment.width, fragment.height);
                            content.clip_nonzero();
                            content.end_path();
                        }
                        content.transform([draw.width, 0.0, 0.0, draw.height, draw.x, draw.y]);
                        content.x_object(Name(&image_ref.name));
                        content.restore_state();
                    }
                }
            }
        }
        pdf.stream(content_id, &content.finish());
    }

    for (idx, (_resource_name, base_font)) in BUILTIN_FONTS.iter().enumerate() {
        let mut font = pdf.type1_font(Ref::new(first_font_id + idx as i32));
        font.base_font(Name(base_font));
        if *base_font != b"Symbol" && *base_font != b"ZapfDingbats" {
            font.encoding_predefined(Name(b"WinAnsiEncoding"));
        }
    }

    for (page_idx, page) in layout.pages.iter().enumerate() {
        for (item_idx, item) in page.items.iter().enumerate() {
            let LayoutItem::Image(fragment) = item else {
                continue;
            };
            let Some(image_ref) = image_refs.get(page_idx).and_then(|page_images| {
                page_images
                    .iter()
                    .find(|image| image.item_index == item_idx)
            }) else {
                continue;
            };
            match fragment.image.format {
                ImageFormat::Jpeg | ImageFormat::JpegGrayscale | ImageFormat::JpegCmyk => {
                    let mut image = pdf.image_xobject(image_ref.id, &fragment.image.bytes);
                    image.width(fragment.image.width_px as i32);
                    image.height(fragment.image.height_px as i32);
                    match fragment.image.format {
                        ImageFormat::Jpeg => image.color_space().device_rgb(),
                        ImageFormat::JpegGrayscale => image.color_space().device_gray(),
                        ImageFormat::JpegCmyk => image.color_space().device_cmyk(),
                        _ => unreachable!("only JPEG formats enter this branch"),
                    }
                    image.bits_per_component(8);
                    image.filter(Filter::DctDecode);
                }
                ImageFormat::Png | ImageFormat::PngGrayscale | ImageFormat::PngIndexed => {
                    let mut image = pdf.image_xobject(image_ref.id, &fragment.image.bytes);
                    image.width(fragment.image.width_px as i32);
                    image.height(fragment.image.height_px as i32);
                    let color_components = match fragment.image.format {
                        ImageFormat::Png => {
                            image.color_space().device_rgb();
                            3
                        }
                        ImageFormat::PngGrayscale => {
                            image.color_space().device_gray();
                            1
                        }
                        ImageFormat::PngIndexed => {
                            let hival = fragment
                                .image
                                .palette
                                .len()
                                .checked_div(3)
                                .and_then(|entries| entries.checked_sub(1))
                                .unwrap_or(0) as i32;
                            image.color_space().indexed(
                                Name(b"DeviceRGB"),
                                hival,
                                &fragment.image.palette,
                            );
                            1
                        }
                        _ => unreachable!("only PNG formats enter this branch"),
                    };
                    image.bits_per_component(8);
                    image.filter(Filter::FlateDecode);
                    image
                        .decode_parms()
                        .predictor(Predictor::PngOptimum)
                        .colors(color_components)
                        .bits_per_component(8)
                        .columns(fragment.image.width_px as i32);
                }
                ImageFormat::Rgb8 => {
                    let mut image = pdf.image_xobject(image_ref.id, &fragment.image.bytes);
                    image.width(fragment.image.width_px as i32);
                    image.height(fragment.image.height_px as i32);
                    image.color_space().device_rgb();
                    image.bits_per_component(8);
                }
                ImageFormat::WmfVector | ImageFormat::Placeholder => {}
            }
        }
    }

    pdf.finish()
}

#[derive(Debug, Copy, Clone)]
struct ImageDrawRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    clipped: bool,
}

fn image_draw_rect(fragment: &crate::layout::ImageFragment) -> ImageDrawRect {
    let natural_width = (fragment.image.width_px as f32 * 0.75).max(1.0);
    let natural_height = (fragment.image.height_px as f32 * 0.75).max(1.0);
    let crop = fragment.image.crop;
    let mut left = twips_to_points(crop.left_twips.max(0)) / natural_width;
    let mut right = twips_to_points(crop.right_twips.max(0)) / natural_width;
    let mut top = twips_to_points(crop.top_twips.max(0)) / natural_height;
    let mut bottom = twips_to_points(crop.bottom_twips.max(0)) / natural_height;
    clamp_crop_pair(&mut left, &mut right);
    clamp_crop_pair(&mut top, &mut bottom);

    let visible_width = (1.0 - left - right).max(0.05);
    let visible_height = (1.0 - top - bottom).max(0.05);
    let width = fragment.width / visible_width;
    let height = fragment.height / visible_height;

    ImageDrawRect {
        x: fragment.x - (width * left),
        y: fragment.y - (height * bottom),
        width,
        height,
        clipped: left > 0.0 || right > 0.0 || top > 0.0 || bottom > 0.0,
    }
}

fn clamp_crop_pair(first: &mut f32, second: &mut f32) {
    let total = *first + *second;
    if total > 0.95 {
        let scale = 0.95 / total;
        *first *= scale;
        *second *= scale;
    }
}

#[derive(Debug, Clone)]
struct PdfImageRef {
    item_index: usize,
    name: Vec<u8>,
    id: Ref,
}

fn collect_image_refs(layout: &LayoutDocument, first_image_id: i32) -> Vec<Vec<PdfImageRef>> {
    let mut next_id = first_image_id;
    layout
        .pages
        .iter()
        .map(|page| {
            page.items
                .iter()
                .enumerate()
                .filter_map(|(idx, item)| {
                    if matches!(item, LayoutItem::Image(fragment) if image_format_uses_xobject(fragment.image.format))
                    {
                        let id = Ref::new(next_id);
                        let name = format!("Im{}", next_id - first_image_id + 1).into_bytes();
                        next_id += 1;
                        Some(PdfImageRef {
                            item_index: idx,
                            name,
                            id,
                        })
                    } else {
                        None
                    }
                })
                .collect()
        })
        .collect()
}

fn image_format_uses_xobject(format: ImageFormat) -> bool {
    matches!(
        format,
        ImageFormat::Jpeg
            | ImageFormat::JpegGrayscale
            | ImageFormat::JpegCmyk
            | ImageFormat::Png
            | ImageFormat::PngGrayscale
            | ImageFormat::PngIndexed
            | ImageFormat::Rgb8
    )
}

fn draw_passive_wmf_vector_image(content: &mut Content, fragment: &crate::layout::ImageFragment) {
    let draw = image_draw_rect(fragment);
    let source_width = fragment.image.width_px.max(1) as f32;
    let source_height = fragment.image.height_px.max(1) as f32;

    content.save_state();
    content.rect(fragment.x, fragment.y, fragment.width, fragment.height);
    content.clip_nonzero();
    content.end_path();
    for command in &fragment.image.vector_commands {
        match command {
            StaticImageVectorCommand::Line {
                x1,
                y1,
                x2,
                y2,
                stroke_color,
            } => {
                draw_passive_vector_line(
                    content,
                    vector_command_point(draw, source_width, source_height, *x1, *y1),
                    vector_command_point(draw, source_width, source_height, *x2, *y2),
                    *stroke_color,
                );
            }
            StaticImageVectorCommand::Polyline {
                points,
                stroke_color,
            } => {
                let points = vector_command_points(draw, source_width, source_height, points);
                draw_passive_vector_polyline(content, &points, *stroke_color);
            }
            StaticImageVectorCommand::Polygon {
                points,
                stroke_color,
                fill_color,
            } => {
                let points = vector_command_points(draw, source_width, source_height, points);
                draw_passive_vector_polygon(content, &points, *stroke_color, *fill_color);
            }
            StaticImageVectorCommand::Rectangle {
                left,
                top,
                right,
                bottom,
                stroke_color,
                fill_color,
            } => {
                let rect = vector_command_rect(
                    draw,
                    source_width,
                    source_height,
                    *left,
                    *top,
                    *right,
                    *bottom,
                );
                draw_passive_vector_rectangle(content, rect, *stroke_color, *fill_color);
            }
            StaticImageVectorCommand::Ellipse {
                left,
                top,
                right,
                bottom,
                stroke_color,
                fill_color,
            } => {
                let rect = vector_command_rect(
                    draw,
                    source_width,
                    source_height,
                    *left,
                    *top,
                    *right,
                    *bottom,
                );
                draw_passive_vector_ellipse(content, rect, *stroke_color, *fill_color);
            }
        }
    }
    content.restore_state();
}

fn vector_command_points(
    draw: ImageDrawRect,
    source_width: f32,
    source_height: f32,
    points: &[(f32, f32)],
) -> Vec<crate::layout::LayoutPoint> {
    points
        .iter()
        .map(|(x, y)| vector_command_point(draw, source_width, source_height, *x, *y))
        .collect()
}

fn vector_command_point(
    draw: ImageDrawRect,
    source_width: f32,
    source_height: f32,
    x: f32,
    y: f32,
) -> crate::layout::LayoutPoint {
    crate::layout::LayoutPoint {
        x: draw.x + (x / source_width) * draw.width,
        y: draw.y + draw.height - (y / source_height) * draw.height,
    }
}

#[derive(Debug, Copy, Clone)]
struct VectorDrawRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

fn vector_command_rect(
    draw: ImageDrawRect,
    source_width: f32,
    source_height: f32,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
) -> VectorDrawRect {
    let x = draw.x + (left / source_width) * draw.width;
    let width = ((right - left) / source_width * draw.width).max(0.1);
    let y = draw.y + draw.height - (bottom / source_height) * draw.height;
    let height = ((bottom - top) / source_height * draw.height).max(0.1);
    VectorDrawRect {
        x,
        y,
        width,
        height,
    }
}

fn draw_passive_vector_line(
    content: &mut Content,
    from: crate::layout::LayoutPoint,
    to: crate::layout::LayoutPoint,
    stroke_color: Option<crate::model::Color>,
) {
    let Some(color) = stroke_color else {
        return;
    };
    draw_passive_line(
        content,
        from.x,
        from.y,
        to.x,
        to.y,
        0.75,
        pdf_color_from_model(color),
        LineStyle::Solid,
    );
}

fn draw_passive_vector_polyline(
    content: &mut Content,
    points: &[crate::layout::LayoutPoint],
    stroke_color: Option<crate::model::Color>,
) {
    if points.len() < 2 {
        return;
    }
    for pair in points.windows(2) {
        draw_passive_vector_line(content, pair[0], pair[1], stroke_color);
    }
}

fn draw_passive_vector_polygon(
    content: &mut Content,
    points: &[crate::layout::LayoutPoint],
    stroke_color: Option<crate::model::Color>,
    fill_color: Option<crate::model::Color>,
) {
    if points.len() < 3 {
        return;
    }
    draw_passive_polygon(
        content,
        points,
        stroke_color.map(|_| 0.75).unwrap_or(0.0),
        stroke_color.map(pdf_color_from_model).unwrap_or(PdfColor {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
        }),
        LineStyle::Solid,
        fill_color.map(pdf_color_from_model),
    );
}

fn draw_passive_vector_rectangle(
    content: &mut Content,
    rect: VectorDrawRect,
    stroke_color: Option<crate::model::Color>,
    fill_color: Option<crate::model::Color>,
) {
    if fill_color.is_none() && stroke_color.is_none() {
        return;
    }
    if let Some(color) = fill_color {
        set_fill_color(content, pdf_color_from_model(color));
    }
    if let Some(color) = stroke_color {
        set_stroke_color(content, pdf_color_from_model(color));
        content.set_line_width(0.75);
    }
    content.rect(rect.x, rect.y, rect.width, rect.height);
    match (fill_color, stroke_color) {
        (Some(_), Some(_)) => {
            content.fill_nonzero_and_stroke();
        }
        (Some(_), None) => {
            content.fill_nonzero();
        }
        (None, Some(_)) => {
            content.stroke();
        }
        (None, None) => {}
    }
}

fn draw_passive_vector_ellipse(
    content: &mut Content,
    rect: VectorDrawRect,
    stroke_color: Option<crate::model::Color>,
    fill_color: Option<crate::model::Color>,
) {
    draw_passive_ellipse(
        content,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        stroke_color.map(|_| 0.75).unwrap_or(0.0),
        stroke_color.map(pdf_color_from_model).unwrap_or(PdfColor {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
        }),
        LineStyle::Solid,
        fill_color.map(pdf_color_from_model),
    );
}

fn pdf_color_from_model(color: crate::model::Color) -> PdfColor {
    PdfColor {
        red: color.red as f32 / 255.0,
        green: color.green as f32 / 255.0,
        blue: color.blue as f32 / 255.0,
    }
}

fn draw_passive_image_placeholder(content: &mut Content, fragment: &crate::layout::ImageFragment) {
    let width = fragment.width.max(1.0);
    let height = fragment.height.max(1.0);
    set_fill_color(
        content,
        PdfColor {
            red: 0.96,
            green: 0.96,
            blue: 0.96,
        },
    );
    content.rect(fragment.x, fragment.y, width, height);
    content.fill_nonzero();
    set_stroke_color(
        content,
        PdfColor {
            red: 0.45,
            green: 0.45,
            blue: 0.45,
        },
    );
    content.set_line_width(0.75);
    content.rect(fragment.x, fragment.y, width, height);
    content.stroke();
    if width > 24.0 && height > 24.0 {
        stroke_line(
            content,
            fragment.x,
            fragment.y,
            fragment.x + width,
            fragment.y + height,
            0.5,
        );
        stroke_line(
            content,
            fragment.x,
            fragment.y + height,
            fragment.x + width,
            fragment.y,
            0.5,
        );
    }
    if width < 96.0 || height < 24.0 {
        return;
    }

    let mut style = CharacterStyle::default();
    style.font_size_half_points = 18;
    let label = "Image skipped";
    let encoded = encode_pdf_text_for_font(label, PdfFontFamily::Helvetica);
    set_fill_color(
        content,
        PdfColor {
            red: 0.25,
            green: 0.25,
            blue: 0.25,
        },
    );
    write_text_fragment(
        content,
        label,
        PdfFontFamily::Helvetica,
        &style,
        0.0,
        fragment.x + 6.0,
        fragment.y + (height / 2.0) - 3.0,
        &encoded,
        TextRenderingMode::Fill,
    );
}

fn is_stream_marker_at(pdf: &[u8], offset: usize) -> bool {
    pdf[offset..].starts_with(b"stream")
        && offset
            .checked_sub(1)
            .and_then(|idx| pdf.get(idx))
            .is_some_and(|byte| is_pdf_whitespace(*byte))
        && pdf
            .get(offset + b"stream".len())
            .is_some_and(|byte| matches!(*byte, b'\r' | b'\n'))
}

fn stream_data_start(pdf: &[u8], stream_offset: usize) -> Option<usize> {
    let after_marker = stream_offset.checked_add(b"stream".len())?;
    match pdf.get(after_marker).copied()? {
        b'\r' if pdf.get(after_marker + 1).copied() == Some(b'\n') => Some(after_marker + 2),
        b'\r' | b'\n' => Some(after_marker + 1),
        _ => None,
    }
}

fn find_endstream_marker(pdf: &[u8], stream_start: usize) -> Option<usize> {
    let mut offset = stream_start;
    while offset < pdf.len() {
        if pdf[offset..].starts_with(b"endstream")
            && offset
                .checked_sub(1)
                .and_then(|idx| pdf.get(idx))
                .is_some_and(|byte| is_pdf_whitespace(*byte))
            && is_pdf_whitespace_or_eof(pdf.get(offset + b"endstream".len()).copied())
        {
            return Some(offset);
        }
        offset += 1;
    }
    None
}

fn is_pdf_name_boundary(byte: Option<u8>) -> bool {
    byte.is_none_or(|byte| {
        is_pdf_whitespace(byte)
            || matches!(byte, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'/' | b'%')
    })
}

fn is_pdf_whitespace_or_eof(byte: Option<u8>) -> bool {
    byte.is_none_or(is_pdf_whitespace)
}

fn is_pdf_whitespace(byte: u8) -> bool {
    matches!(byte, b'\0' | b'\t' | b'\n' | b'\x0c' | b'\r' | b' ')
}

fn font_name_for_style(family: PdfFontFamily, style: &CharacterStyle) -> Name<'static> {
    match (family, style.bold, style.italic) {
        (PdfFontFamily::Helvetica, true, true) => Name(HELVETICA_BOLD_ITALIC),
        (PdfFontFamily::Helvetica, true, false) => Name(HELVETICA_BOLD),
        (PdfFontFamily::Helvetica, false, true) => Name(HELVETICA_ITALIC),
        (PdfFontFamily::Helvetica, false, false) => Name(HELVETICA_REGULAR),
        (PdfFontFamily::Courier, true, true) => Name(COURIER_BOLD_ITALIC),
        (PdfFontFamily::Courier, true, false) => Name(COURIER_BOLD),
        (PdfFontFamily::Courier, false, true) => Name(COURIER_ITALIC),
        (PdfFontFamily::Courier, false, false) => Name(COURIER_REGULAR),
        (PdfFontFamily::Times, true, true) => Name(TIMES_BOLD_ITALIC),
        (PdfFontFamily::Times, true, false) => Name(TIMES_BOLD),
        (PdfFontFamily::Times, false, true) => Name(TIMES_ITALIC),
        (PdfFontFamily::Times, false, false) => Name(TIMES_REGULAR),
        (PdfFontFamily::Symbol, _, _) => Name(SYMBOL_REGULAR),
        (PdfFontFamily::ZapfDingbats, _, _) => Name(ZAPF_DINGBATS_REGULAR),
    }
}

fn draw_underline(
    content: &mut Content,
    x: f32,
    y: f32,
    width: f32,
    color: PdfColor,
    style: UnderlineStyle,
) {
    if width <= 0.0 || style == UnderlineStyle::None {
        return;
    }

    content.save_state();
    set_stroke_color(content, color);

    match style {
        UnderlineStyle::None => {}
        UnderlineStyle::Single | UnderlineStyle::Words => {
            stroke_line(content, x, y, x + width, y, 0.5)
        }
        UnderlineStyle::Double => {
            stroke_line(content, x, y, x + width, y, 0.5);
            stroke_line(content, x, y - 2.0, x + width, y - 2.0, 0.5);
        }
        UnderlineStyle::Thick => stroke_line(content, x, y, x + width, y, 1.2),
        UnderlineStyle::Dotted => {
            content.set_dash_pattern([0.5, 2.0], 0.0);
            stroke_line(content, x, y, x + width, y, 0.8);
        }
        UnderlineStyle::Dashed => {
            content.set_dash_pattern([3.0, 2.0], 0.0);
            stroke_line(content, x, y, x + width, y, 0.5);
        }
        UnderlineStyle::Wave => stroke_wave_underline(content, x, y, width),
    }

    content.restore_state();
}

#[allow(clippy::too_many_arguments)]
fn draw_passive_line(
    content: &mut Content,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    width: f32,
    color: PdfColor,
    style: LineStyle,
) {
    content.save_state();
    set_stroke_color(content, color);
    match style {
        LineStyle::Solid => stroke_line(content, x1, y1, x2, y2, width),
        LineStyle::Dotted => {
            let dot = width.max(0.5);
            content.set_dash_pattern([dot, dot * 2.0], 0.0);
            stroke_line(content, x1, y1, x2, y2, width);
        }
        LineStyle::Dashed => {
            let dash = (width * 3.0).max(3.0);
            content.set_dash_pattern([dash, dash * 0.75], 0.0);
            stroke_line(content, x1, y1, x2, y2, width);
        }
        LineStyle::Double => draw_double_line(content, x1, y1, x2, y2, width),
        LineStyle::Wavy => stroke_wave_line(content, x1, y1, x2, y2, width),
    }
    content.restore_state();
}

fn set_passive_path_stroke_style(content: &mut Content, width: f32, style: LineStyle) {
    match style {
        LineStyle::Solid | LineStyle::Double | LineStyle::Wavy => {}
        LineStyle::Dotted => {
            let dot = width.max(0.5);
            content.set_dash_pattern([dot, dot * 2.0], 0.0);
        }
        LineStyle::Dashed => {
            let dash = (width * 3.0).max(3.0);
            content.set_dash_pattern([dash, dash * 0.75], 0.0);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_passive_ellipse(
    content: &mut Content,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    stroke_width: f32,
    stroke_color: PdfColor,
    stroke_style: LineStyle,
    fill_color: Option<PdfColor>,
) {
    const KAPPA: f32 = 0.552_284_8;

    if stroke_width <= 0.0 && fill_color.is_none() {
        return;
    }
    let width = width.max(0.1);
    let height = height.max(0.1);
    let rx = width / 2.0;
    let ry = height / 2.0;
    let cx = x + rx;
    let cy = y + ry;
    let dx = rx * KAPPA;
    let dy = ry * KAPPA;

    let has_stroke = stroke_width > 0.0;
    content.save_state();
    if has_stroke {
        content.set_line_width(stroke_width.max(0.25));
        set_stroke_color(content, stroke_color);
        set_passive_path_stroke_style(content, stroke_width, stroke_style);
    }
    if let Some(fill_color) = fill_color {
        set_fill_color(content, fill_color);
    }
    content.move_to(cx + rx, cy);
    content.cubic_to(cx + rx, cy + dy, cx + dx, cy + ry, cx, cy + ry);
    content.cubic_to(cx - dx, cy + ry, cx - rx, cy + dy, cx - rx, cy);
    content.cubic_to(cx - rx, cy - dy, cx - dx, cy - ry, cx, cy - ry);
    content.cubic_to(cx + dx, cy - ry, cx + rx, cy - dy, cx + rx, cy);
    content.close_path();
    if fill_color.is_some() && has_stroke {
        content.fill_nonzero_and_stroke();
    } else if fill_color.is_some() {
        content.fill_nonzero();
    } else {
        content.stroke();
    }
    content.restore_state();
}

#[allow(clippy::too_many_arguments)]
fn draw_passive_rounded_rectangle(
    content: &mut Content,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    stroke_width: f32,
    stroke_color: PdfColor,
    stroke_style: LineStyle,
    fill_color: Option<PdfColor>,
) {
    const KAPPA: f32 = 0.552_284_8;

    if stroke_width <= 0.0 && fill_color.is_none() {
        return;
    }
    let width = width.max(0.1);
    let height = height.max(0.1);
    let radius = radius.clamp(0.1, width.min(height) / 2.0);
    let control = radius * KAPPA;
    let right = x + width;
    let top = y + height;

    let has_stroke = stroke_width > 0.0;
    content.save_state();
    if has_stroke {
        content.set_line_width(stroke_width.max(0.25));
        set_stroke_color(content, stroke_color);
        set_passive_path_stroke_style(content, stroke_width, stroke_style);
    }
    if let Some(fill_color) = fill_color {
        set_fill_color(content, fill_color);
    }
    content.move_to(x + radius, y);
    content.line_to(right - radius, y);
    content.cubic_to(
        right - radius + control,
        y,
        right,
        y + radius - control,
        right,
        y + radius,
    );
    content.line_to(right, top - radius);
    content.cubic_to(
        right,
        top - radius + control,
        right - radius + control,
        top,
        right - radius,
        top,
    );
    content.line_to(x + radius, top);
    content.cubic_to(
        x + radius - control,
        top,
        x,
        top - radius + control,
        x,
        top - radius,
    );
    content.line_to(x, y + radius);
    content.cubic_to(
        x,
        y + radius - control,
        x + radius - control,
        y,
        x + radius,
        y,
    );
    content.close_path();
    if fill_color.is_some() && has_stroke {
        content.fill_nonzero_and_stroke();
    } else if fill_color.is_some() {
        content.fill_nonzero();
    } else {
        content.stroke();
    }
    content.restore_state();
}

fn draw_passive_polygon(
    content: &mut Content,
    points: &[crate::layout::LayoutPoint],
    stroke_width: f32,
    stroke_color: PdfColor,
    stroke_style: LineStyle,
    fill_color: Option<PdfColor>,
) {
    let Some(first) = points.first() else {
        return;
    };
    if stroke_width <= 0.0 && fill_color.is_none() {
        return;
    }

    let has_stroke = stroke_width > 0.0;
    content.save_state();
    if has_stroke {
        content.set_line_width(stroke_width.max(0.25));
        set_stroke_color(content, stroke_color);
        set_passive_path_stroke_style(content, stroke_width, stroke_style);
    }
    if let Some(fill_color) = fill_color {
        set_fill_color(content, fill_color);
    }
    content.move_to(first.x, first.y);
    for point in points.iter().skip(1) {
        content.line_to(point.x, point.y);
    }
    content.close_path();
    if fill_color.is_some() && has_stroke {
        content.fill_nonzero_and_stroke();
    } else if fill_color.is_some() {
        content.fill_nonzero();
    } else {
        content.stroke();
    }
    content.restore_state();
}

fn draw_passive_text_overlays(content: &mut Content, fragment: &TextFragment) {
    draw_passive_emphasis_marks(content, fragment);
    draw_passive_checkbox_overlays(content, fragment);
    draw_passive_dingbat_overlays(content, fragment);
}

fn draw_passive_emphasis_marks(content: &mut Content, fragment: &TextFragment) {
    if fragment.style.emphasis_mark == CharacterEmphasisMark::None {
        return;
    }

    let font_size = fragment.style.font_size_points();
    let horizontal_scale = fragment.style.horizontal_scale();
    let character_spacing = twips_to_points(fragment.style.character_spacing_twips);
    let visible_count = fragment
        .text
        .chars()
        .filter(|ch| !is_zero_width_pdf_char(*ch))
        .count();
    if visible_count == 0 {
        return;
    }

    let mut visible_index = 0usize;
    let mut cursor = fragment.x;

    content.save_state();
    set_fill_color(content, fragment.color);
    set_stroke_color(content, fragment.color);
    content.set_line_width((font_size * 0.06).clamp(0.35, 1.0));
    for ch in fragment.text.chars() {
        if !is_zero_width_pdf_char(ch) {
            visible_index += 1;
        }
        if !is_zero_width_pdf_char(ch) && !ch.is_whitespace() {
            let advance = pdf_base_glyph_advance(ch, fragment.font_family, &fragment.style);
            let center_x = cursor + (advance * horizontal_scale * 0.5);
            draw_passive_emphasis_mark(
                content,
                fragment.style.emphasis_mark,
                center_x,
                fragment.baseline_y,
                font_size,
            );
        }
        if !is_zero_width_pdf_char(ch) {
            let mut advance = pdf_base_glyph_advance(ch, fragment.font_family, &fragment.style);
            if ch == ' ' || ch == '\u{00a0}' {
                advance += fragment.word_spacing;
            }
            if visible_index < visible_count {
                advance += character_spacing;
            }
            cursor += advance * horizontal_scale;
        }
    }
    content.restore_state();
}

fn draw_passive_emphasis_mark(
    content: &mut Content,
    mark: CharacterEmphasisMark,
    center_x: f32,
    baseline_y: f32,
    font_size: f32,
) {
    let mark_y = baseline_y + font_size * 0.82;
    match mark {
        CharacterEmphasisMark::None => {}
        CharacterEmphasisMark::Dot => {
            let radius = (font_size * 0.065).clamp(0.45, 1.25);
            content.rect(
                center_x - radius,
                mark_y - radius,
                radius * 2.0,
                radius * 2.0,
            );
            content.fill_nonzero();
        }
        CharacterEmphasisMark::Comma => {
            let height = (font_size * 0.22).clamp(1.0, 4.0);
            content.move_to(center_x, mark_y);
            content.line_to(center_x - height * 0.25, mark_y - height);
            content.stroke();
        }
    }
}

fn draw_passive_checkbox_overlays(content: &mut Content, fragment: &TextFragment) {
    if fragment.font_family != PdfFontFamily::ZapfDingbats
        || (!fragment.text.contains('\u{2611}') && !fragment.text.contains('\u{2612}'))
    {
        return;
    }

    let font_size = fragment.style.font_size_points();
    let horizontal_scale = fragment.style.horizontal_scale();
    let character_spacing = twips_to_points(fragment.style.character_spacing_twips);
    let visible_count = fragment
        .text
        .chars()
        .filter(|ch| !is_zero_width_pdf_char(*ch))
        .count();
    let mut visible_index = 0usize;
    let mut cursor = fragment.x;

    content.save_state();
    set_stroke_color(content, fragment.color);
    content.set_line_width((font_size * 0.075).clamp(0.5, 1.25));
    for ch in fragment.text.chars() {
        match ch {
            '\u{2611}' => {
                draw_passive_checkbox_tick(content, cursor, fragment.baseline_y, font_size)
            }
            '\u{2612}' => draw_passive_checkbox_x(content, cursor, fragment.baseline_y, font_size),
            _ => {}
        }
        if !is_zero_width_pdf_char(ch) {
            visible_index += 1;
            let mut advance = pdf_base_glyph_advance(ch, fragment.font_family, &fragment.style);
            if ch == ' ' || ch == '\u{00a0}' {
                advance += fragment.word_spacing;
            }
            if visible_index < visible_count {
                advance += character_spacing;
            }
            cursor += advance * horizontal_scale;
        }
    }
    content.restore_state();
}

fn draw_passive_checkbox_tick(
    content: &mut Content,
    glyph_x: f32,
    baseline_y: f32,
    font_size: f32,
) {
    let box_size = font_size * 0.62;
    let left = glyph_x + font_size * 0.04;
    let bottom = baseline_y + font_size * 0.04;

    content.move_to(left + box_size * 0.22, bottom + box_size * 0.48);
    content.line_to(left + box_size * 0.42, bottom + box_size * 0.25);
    content.line_to(left + box_size * 0.82, bottom + box_size * 0.78);
    content.stroke();
}

fn draw_passive_checkbox_x(content: &mut Content, glyph_x: f32, baseline_y: f32, font_size: f32) {
    let box_size = font_size * 0.62;
    let left = glyph_x + font_size * 0.04;
    let bottom = baseline_y + font_size * 0.04;

    content.move_to(left + box_size * 0.24, bottom + box_size * 0.24);
    content.line_to(left + box_size * 0.76, bottom + box_size * 0.76);
    content.move_to(left + box_size * 0.76, bottom + box_size * 0.24);
    content.line_to(left + box_size * 0.24, bottom + box_size * 0.76);
    content.stroke();
}

fn draw_passive_dingbat_overlays(content: &mut Content, fragment: &TextFragment) {
    if fragment.font_family != PdfFontFamily::ZapfDingbats || !fragment.text.contains('\u{263a}') {
        return;
    }

    let font_size = fragment.style.font_size_points();
    let horizontal_scale = fragment.style.horizontal_scale();
    let character_spacing = twips_to_points(fragment.style.character_spacing_twips);
    let visible_count = fragment
        .text
        .chars()
        .filter(|ch| !is_zero_width_pdf_char(*ch))
        .count();
    let mut visible_index = 0usize;
    let mut cursor = fragment.x;

    content.save_state();
    set_stroke_color(content, fragment.color);
    set_fill_color(content, fragment.color);
    content.set_line_width((font_size * 0.055).clamp(0.45, 1.0));
    for ch in fragment.text.chars() {
        if ch == '\u{263a}' {
            draw_passive_smiley(content, cursor, fragment.baseline_y, font_size);
        }
        if !is_zero_width_pdf_char(ch) {
            visible_index += 1;
            let mut advance = pdf_base_glyph_advance(ch, fragment.font_family, &fragment.style);
            if ch == ' ' || ch == '\u{00a0}' {
                advance += fragment.word_spacing;
            }
            if visible_index < visible_count {
                advance += character_spacing;
            }
            cursor += advance * horizontal_scale;
        }
    }
    content.restore_state();
}

fn draw_passive_smiley(content: &mut Content, glyph_x: f32, baseline_y: f32, font_size: f32) {
    let radius = font_size * 0.28;
    let center_x = glyph_x + font_size * 0.27;
    let center_y = baseline_y + font_size * 0.34;
    draw_passive_circle(content, center_x, center_y, radius);

    let eye_radius = (font_size * 0.035).clamp(0.25, 0.7);
    draw_passive_filled_circle(
        content,
        center_x - radius * 0.35,
        center_y + radius * 0.25,
        eye_radius,
    );
    draw_passive_filled_circle(
        content,
        center_x + radius * 0.35,
        center_y + radius * 0.25,
        eye_radius,
    );

    content.move_to(center_x - radius * 0.48, center_y - radius * 0.12);
    content.cubic_to(
        center_x - radius * 0.26,
        center_y - radius * 0.48,
        center_x + radius * 0.26,
        center_y - radius * 0.48,
        center_x + radius * 0.48,
        center_y - radius * 0.12,
    );
    content.stroke();
}

fn draw_passive_circle(content: &mut Content, cx: f32, cy: f32, radius: f32) {
    add_passive_circle_path(content, cx, cy, radius);
    content.stroke();
}

fn draw_passive_filled_circle(content: &mut Content, cx: f32, cy: f32, radius: f32) {
    add_passive_circle_path(content, cx, cy, radius);
    content.fill_nonzero();
}

fn add_passive_circle_path(content: &mut Content, cx: f32, cy: f32, radius: f32) {
    const KAPPA: f32 = 0.552_284_8;
    let control = radius * KAPPA;
    content.move_to(cx + radius, cy);
    content.cubic_to(
        cx + radius,
        cy + control,
        cx + control,
        cy + radius,
        cx,
        cy + radius,
    );
    content.cubic_to(
        cx - control,
        cy + radius,
        cx - radius,
        cy + control,
        cx - radius,
        cy,
    );
    content.cubic_to(
        cx - radius,
        cy - control,
        cx - control,
        cy - radius,
        cx,
        cy - radius,
    );
    content.cubic_to(
        cx + control,
        cy - radius,
        cx + radius,
        cy - control,
        cx + radius,
        cy,
    );
    content.close_path();
}

fn pdf_base_glyph_advance(ch: char, family: PdfFontFamily, style: &CharacterStyle) -> f32 {
    let size = style.font_size_points();
    match family {
        PdfFontFamily::Courier => size * 0.6,
        PdfFontFamily::Times if matches!(ch, 'i' | 'l' | '!' | '.' | ',' | ':' | ';' | '\'') => {
            size * 0.22
        }
        PdfFontFamily::Helvetica
            if matches!(ch, 'i' | 'l' | '!' | '.' | ',' | ':' | ';' | '\'') =>
        {
            size * 0.25
        }
        PdfFontFamily::Times if matches!(ch, 'm' | 'w' | 'M' | 'W') => size * 0.72,
        PdfFontFamily::Helvetica if matches!(ch, 'm' | 'w' | 'M' | 'W') => size * 0.78,
        _ if matches!(ch, ' ' | '\u{00a0}') => size * 0.28,
        _ if is_zero_width_pdf_char(ch) => 0.0,
        PdfFontFamily::Helvetica if style.bold => size * 0.56,
        PdfFontFamily::Helvetica => size * 0.52,
        PdfFontFamily::Times if style.bold => size * 0.54,
        PdfFontFamily::Times => size * 0.48,
        PdfFontFamily::Symbol | PdfFontFamily::ZapfDingbats => size * 0.54,
    }
}

fn is_zero_width_pdf_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{00ad}' | '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{200e}' | '\u{200f}' | '\u{feff}'
    )
}

fn draw_double_line(content: &mut Content, x1: f32, y1: f32, x2: f32, y2: f32, width: f32) {
    let stroke_width = (width * 0.35).clamp(0.25, width.max(0.25));
    let offset = (width * 0.55).max(1.0);
    if (y1 - y2).abs() < 0.01 {
        stroke_line(content, x1, y1 + offset, x2, y2 + offset, stroke_width);
        stroke_line(content, x1, y1 - offset, x2, y2 - offset, stroke_width);
    } else if (x1 - x2).abs() < 0.01 {
        stroke_line(content, x1 + offset, y1, x2 + offset, y2, stroke_width);
        stroke_line(content, x1 - offset, y1, x2 - offset, y2, stroke_width);
    } else {
        stroke_line(content, x1, y1, x2, y2, width);
    }
}

fn stroke_line(content: &mut Content, x1: f32, y1: f32, x2: f32, y2: f32, width: f32) {
    content.set_line_width(width);
    content.move_to(x1, y1);
    content.line_to(x2, y2);
    content.stroke();
}

fn stroke_wave_underline(content: &mut Content, x: f32, y: f32, width: f32) {
    content.set_line_width(0.5);
    let step = 2.0;
    let amplitude = 1.0;
    let mut cursor = 0.0;
    content.move_to(x, y);
    while cursor < width {
        let next = (cursor + step).min(width);
        let segment_index = (cursor / step) as i32;
        let next_y = if segment_index % 2 == 0 {
            y - amplitude
        } else {
            y + amplitude
        };
        content.line_to(x + next, next_y);
        cursor = next;
    }
    content.stroke();
}

fn stroke_wave_line(content: &mut Content, x1: f32, y1: f32, x2: f32, y2: f32, width: f32) {
    content.set_line_width(width.max(0.5));
    let step = (width * 2.0).clamp(2.0, 6.0);
    let amplitude = width.clamp(1.0, 3.0);

    if (y1 - y2).abs() < 0.01 {
        let direction = if x2 >= x1 { 1.0 } else { -1.0 };
        let total = (x2 - x1).abs();
        let mut cursor = 0.0;
        content.move_to(x1, y1);
        while cursor < total {
            let next = (cursor + step).min(total);
            let segment_index = (cursor / step) as i32;
            let next_y = if segment_index % 2 == 0 {
                y1 - amplitude
            } else {
                y1 + amplitude
            };
            content.line_to(x1 + (next * direction), next_y);
            cursor = next;
        }
        content.stroke();
    } else if (x1 - x2).abs() < 0.01 {
        let direction = if y2 >= y1 { 1.0 } else { -1.0 };
        let total = (y2 - y1).abs();
        let mut cursor = 0.0;
        content.move_to(x1, y1);
        while cursor < total {
            let next = (cursor + step).min(total);
            let segment_index = (cursor / step) as i32;
            let next_x = if segment_index % 2 == 0 {
                x1 - amplitude
            } else {
                x1 + amplitude
            };
            content.line_to(next_x, y1 + (next * direction));
            cursor = next;
        }
        content.stroke();
    } else {
        stroke_line(content, x1, y1, x2, y2, width);
    }
}

fn encode_pdf_text(text: &str) -> Vec<u8> {
    text.chars().map(encode_win_ansi_char).collect()
}

fn encode_pdf_text_for_font(text: &str, family: PdfFontFamily) -> Vec<u8> {
    match family {
        PdfFontFamily::Symbol => text.chars().map(encode_symbol_char).collect(),
        PdfFontFamily::ZapfDingbats => text.chars().map(encode_zapf_dingbats_char).collect(),
        PdfFontFamily::Helvetica | PdfFontFamily::Courier | PdfFontFamily::Times => {
            encode_pdf_text(text)
        }
    }
}

fn encode_symbol_char(ch: char) -> u8 {
    match ch {
        '\u{0391}' => b'A',
        '\u{0392}' => b'B',
        '\u{03a7}' => b'C',
        '\u{0394}' => b'D',
        '\u{0395}' => b'E',
        '\u{03a6}' => b'F',
        '\u{0393}' => b'G',
        '\u{0397}' => b'H',
        '\u{0399}' => b'I',
        '\u{039a}' => b'K',
        '\u{039b}' => b'L',
        '\u{039c}' => b'M',
        '\u{039d}' => b'N',
        '\u{039f}' => b'O',
        '\u{03a0}' => b'P',
        '\u{0398}' => b'Q',
        '\u{03a1}' => b'R',
        '\u{03a3}' => b'S',
        '\u{03a4}' => b'T',
        '\u{03a5}' => b'U',
        '\u{03a9}' => b'W',
        '\u{039e}' => b'X',
        '\u{03a8}' => b'Y',
        '\u{0396}' => b'Z',
        '\u{03b1}' => b'a',
        '\u{03b2}' => b'b',
        '\u{03c7}' => b'c',
        '\u{03b4}' => b'd',
        '\u{03b5}' => b'e',
        '\u{03c6}' => b'f',
        '\u{03b3}' => b'g',
        '\u{03b7}' => b'h',
        '\u{03b9}' => b'i',
        '\u{03d5}' => b'j',
        '\u{03ba}' => b'k',
        '\u{03bb}' => b'l',
        '\u{03bc}' => b'm',
        '\u{03bd}' => b'n',
        '\u{03bf}' => b'o',
        '\u{03c0}' => b'p',
        '\u{03b8}' => b'q',
        '\u{03c1}' => b'r',
        '\u{03c3}' => b's',
        '\u{03c4}' => b't',
        '\u{03c5}' => b'u',
        '\u{03d6}' => b'v',
        '\u{03c9}' => b'w',
        '\u{03be}' => b'x',
        '\u{03c8}' => b'y',
        '\u{03b6}' => b'z',
        '\u{2200}' => b'"',
        '\u{2203}' => b'$',
        '\u{220b}' => b'\'',
        '\u{2217}' => b'*',
        '\u{2245}' => b'@',
        '\u{2206}' => b'D',
        '\u{03d1}' => b'J',
        '\u{03c2}' => b'V',
        '\u{2126}' => b'W',
        '\u{00b5}' => b'm',
        '\u{2234}' => b'\\',
        '\u{22a5}' => b'^',
        '\u{223c}' => b'~',
        '\u{03d2}' => 0xa1,
        '\u{2032}' => 0xa2,
        '\u{2022}' => 0xb7,
        '\u{2264}' => 0xa3,
        '\u{2044}' | '\u{2215}' => 0xa4,
        '\u{221e}' => 0xa5,
        '\u{0192}' => 0xa6,
        '\u{2663}' => 0xa7,
        '\u{2666}' => 0xa8,
        '\u{2665}' => 0xa9,
        '\u{2660}' => 0xaa,
        '\u{2194}' => 0xab,
        '\u{00b0}' => 0xb0,
        '\u{00b1}' => 0xb1,
        '\u{2033}' => 0xb2,
        '\u{2265}' => 0xb3,
        '\u{00d7}' => 0xb4,
        '\u{221d}' => 0xb5,
        '\u{2202}' => 0xb6,
        '\u{2212}' => 0x2d,
        '\u{00f7}' => 0xb8,
        '\u{2260}' => 0xb9,
        '\u{2261}' => 0xba,
        '\u{2248}' => 0xbb,
        '\u{2026}' => 0xbc,
        '\u{21b5}' => 0xbf,
        '\u{2135}' => 0xc0,
        '\u{2111}' => 0xc1,
        '\u{211c}' => 0xc2,
        '\u{2118}' => 0xc3,
        '\u{2297}' => 0xc4,
        '\u{2295}' => 0xc5,
        '\u{2205}' => 0xc6,
        '\u{2229}' => 0xc7,
        '\u{222a}' => 0xc8,
        '\u{2283}' => 0xc9,
        '\u{2287}' => 0xca,
        '\u{2284}' => 0xcb,
        '\u{2282}' => 0xcc,
        '\u{2286}' => 0xcd,
        '\u{2208}' => 0xce,
        '\u{2209}' => 0xcf,
        '\u{2220}' => 0xd0,
        '\u{2207}' => 0xd1,
        '\u{00ae}' => 0xd2,
        '\u{00a9}' => 0xd3,
        '\u{2122}' => 0xd4,
        '\u{220f}' => 0xd5,
        '\u{2211}' => 0xe5,
        '\u{221a}' => 0xd6,
        '\u{22c5}' => 0xd7,
        '\u{00ac}' => 0xd8,
        '\u{2227}' => 0xd9,
        '\u{2228}' => 0xda,
        '\u{21d4}' => 0xdb,
        '\u{21d0}' => 0xdc,
        '\u{21d1}' => 0xdd,
        '\u{21d2}' => 0xde,
        '\u{21d3}' => 0xdf,
        '\u{25ca}' => 0xe0,
        '\u{2329}' => 0xe1,
        '\u{232a}' => 0xf1,
        '\u{239b}' => 0xe6,
        '\u{239c}' => 0xe7,
        '\u{239d}' => 0xe8,
        '\u{23a1}' => 0xe9,
        '\u{23a2}' => 0xea,
        '\u{23a3}' => 0xeb,
        '\u{23a7}' => 0xec,
        '\u{23a8}' => 0xed,
        '\u{23a9}' => 0xee,
        '\u{23aa}' => 0xef,
        '\u{20ac}' => 0xf0,
        '\u{222b}' => 0xf2,
        '\u{2320}' => 0xf3,
        '\u{23af}' => 0xbe,
        '\u{2321}' => 0xf5,
        '\u{23ae}' => 0xf4,
        '\u{239e}' => 0xf7,
        '\u{239f}' => 0xf8,
        '\u{23a0}' => 0xf9,
        '\u{23a4}' => 0xfa,
        '\u{23a5}' => 0xfb,
        '\u{23a6}' => 0xfc,
        '\u{23ab}' => 0xfd,
        '\u{23ac}' => 0xfe,
        '\u{23ad}' => 0xff,
        '\u{23d0}' => 0xbd,
        '\u{2190}' => 0xac,
        '\u{2191}' => 0xad,
        '\u{2192}' => 0xae,
        '\u{2193}' => 0xaf,
        ' ' => b' ',
        ch if ch.is_ascii() => ch as u8,
        _ => b'?',
    }
}

fn encode_zapf_dingbats_char(ch: char) -> u8 {
    match ch {
        '\u{25a1}' | '\u{2610}' | '\u{2611}' | '\u{2612}' | '\u{2751}' => b'q',
        '\u{263a}' => b'J',
        '\u{2713}' | '\u{2714}' => b'3',
        '\u{2717}' => b'7',
        ' ' => b' ',
        ch if ch.is_ascii() => ch as u8,
        _ => b'?',
    }
}

fn encode_win_ansi_char(ch: char) -> u8 {
    match ch {
        '\u{20ac}' => 0x80,
        '\u{201a}' => 0x82,
        '\u{0192}' => 0x83,
        '\u{201e}' => 0x84,
        '\u{2026}' => 0x85,
        '\u{2020}' => 0x86,
        '\u{2021}' => 0x87,
        '\u{02c6}' => 0x88,
        '\u{2030}' => 0x89,
        '\u{0160}' => 0x8a,
        '\u{2039}' => 0x8b,
        '\u{0152}' => 0x8c,
        '\u{017d}' => 0x8e,
        '\u{2018}' => 0x91,
        '\u{2019}' => 0x92,
        '\u{201c}' => 0x93,
        '\u{201d}' => 0x94,
        '\u{2022}' => 0x95,
        '\u{2013}' => 0x96,
        '\u{2014}' => 0x97,
        '\u{02dc}' => 0x98,
        '\u{2122}' => 0x99,
        '\u{0161}' => 0x9a,
        '\u{203a}' => 0x9b,
        '\u{0153}' => 0x9c,
        '\u{017e}' => 0x9e,
        '\u{0178}' => 0x9f,
        '\u{00a0}' => 0xa0,
        '\u{00ad}' => 0xad,
        '\u{00a1}'..='\u{00ff}' => ch as u8,
        '\u{2011}' => b'-',
        '\u{2002}' | '\u{2003}' | '\u{2005}' => b' ',
        ch if ch.is_ascii() => ch as u8,
        _ => b'?',
    }
}

fn set_fill_color(content: &mut Content, color: PdfColor) {
    content.set_fill_rgb(color.red, color.green, color.blue);
}

fn set_stroke_color(content: &mut Content, color: PdfColor) {
    content.set_stroke_rgb(color.red, color.green, color.blue);
}

fn write_text_fragment(
    content: &mut Content,
    text: &str,
    font_family: PdfFontFamily,
    style: &CharacterStyle,
    word_spacing: f32,
    x: f32,
    baseline_y: f32,
    encoded: &[u8],
    rendering_mode: TextRenderingMode,
) {
    content.begin_text();
    content.set_font(
        font_name_for_style(font_family, style),
        style.font_size_points(),
    );
    if rendering_mode != TextRenderingMode::Fill {
        content.set_text_rendering_mode(rendering_mode);
    }
    content.set_word_spacing(word_spacing);
    content.set_char_spacing(twips_to_points(style.character_spacing_twips));
    content.set_horizontal_scaling(style.character_scaling_percent as f32);
    content.next_line(x, baseline_y);
    if style_uses_passive_kerning(style) && text_has_passive_kerning(text, font_family, style) {
        write_positioned_text(content, text, font_family, style);
    } else {
        content.show(Str(encoded));
    }
    content.set_horizontal_scaling(100.0);
    content.set_char_spacing(0.0);
    content.set_word_spacing(0.0);
    if rendering_mode != TextRenderingMode::Fill {
        content.set_text_rendering_mode(TextRenderingMode::Fill);
    }
    content.end_text();
}

fn text_has_passive_kerning(
    text: &str,
    font_family: PdfFontFamily,
    style: &CharacterStyle,
) -> bool {
    let mut previous = None;
    for ch in text.chars().filter(|ch| !is_zero_width_pdf_char(*ch)) {
        if let Some(left) = previous
            && passive_pair_kerning_points(left, ch, font_family, style) != 0.0
        {
            return true;
        }
        previous = Some(ch);
    }
    false
}

fn write_positioned_text(
    content: &mut Content,
    text: &str,
    font_family: PdfFontFamily,
    style: &CharacterStyle,
) {
    let mut positioned = content.show_positioned();
    let mut items = positioned.items();
    let mut previous = None;
    let font_size = style.font_size_points().max(1.0);

    for ch in text.chars().filter(|ch| !is_zero_width_pdf_char(*ch)) {
        if let Some(left) = previous {
            let adjustment = passive_pair_kerning_points(left, ch, font_family, style);
            if adjustment != 0.0 {
                items.adjust((-adjustment * 1000.0 / font_size).clamp(-1000.0, 1000.0));
            }
        }
        let mut buffer = [0; 4];
        let encoded = encode_pdf_text_for_font(ch.encode_utf8(&mut buffer), font_family);
        items.show(Str(&encoded));
        previous = Some(ch);
    }
}

fn shadow_offset(style: &CharacterStyle) -> f32 {
    (style.font_size_points() * 0.08).clamp(0.75, 2.5)
}

fn shadow_color(color: PdfColor) -> PdfColor {
    PdfColor {
        red: color.red * 0.35,
        green: color.green * 0.35,
        blue: color.blue * 0.35,
    }
}

fn relief_offset(style: &CharacterStyle) -> f32 {
    (style.font_size_points() * 0.045).clamp(0.5, 1.5)
}

fn relief_layers(
    color: PdfColor,
    relief: TextRelief,
    offset: f32,
) -> (PdfColor, f32, f32, PdfColor, f32, f32) {
    let light = lighten_color(color);
    let dark = shadow_color(color);
    match relief {
        TextRelief::None => (color, 0.0, 0.0, color, 0.0, 0.0),
        TextRelief::Emboss => (light, -offset, offset, dark, offset, -offset),
        TextRelief::Engrave => (dark, -offset, offset, light, offset, -offset),
    }
}

fn lighten_color(color: PdfColor) -> PdfColor {
    PdfColor {
        red: 1.0 - ((1.0 - color.red) * 0.25),
        green: 1.0 - ((1.0 - color.green) * 0.25),
        blue: 1.0 - ((1.0 - color.blue) * 0.25),
    }
}

#[cfg(test)]
mod tests {
    use crate::layout::LayoutEngine;
    use crate::model::{
        Alignment, Block, BorderStyle, CharacterStyle, Color, Document, FontDef, FontFamilyHint,
        FontPitch, ImageCrop, ImageFormat, PAGE_NUMBER_MARKER, PageSettings, Paragraph,
        ParagraphStyle, Run, SECTION_NUMBER_MARKER, StaticImage, StaticShape, StaticShapeKind,
        TOTAL_PAGES_MARKER, Table, TableCell, TableCellBorder, TableRow, UnderlineStyle,
    };

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

    #[test]
    fn passive_pdf_audit_rejects_active_names_outside_streams() {
        let pdf = b"%PDF-1.7
1 0 obj
<< /Type /Catalog /OpenAction << /S /JavaScript /JS (app.alert) >> >>
2 0 obj
<< /Type /Page /Annots [3 0 R] >>
endobj
3 0 obj
<< /Type /Annot /Subtype /Widget /A << /S /URI /URI (https://example.com) >> >>
endobj
4 0 obj
<< /Type /Filespec /AFRelationship /Data /EF << /F 5 0 R >> >>
endobj
5 0 obj
<< /Type /EmbeddedFile /Subtype /FileAttachment >>
endobj
6 0 obj
<< /AF [4 0 R] /Perms << /DocMDP 7 0 R >> /Encrypt 8 0 R /Collection << >> >>
endobj
7 0 obj
<< /S /GoTo /D [2 0 R /Fit] /Next << /S /Named /N /Print >> >>
endobj
8 0 obj
<< /S /ResetForm >>
endobj
9 0 obj
<< /S /Rendition /OP 0 >>
endobj
%%EOF";

        let error = audit_passive_pdf_bytes(pdf).expect_err("active PDF names must be rejected");

        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.token == "/OpenAction")
        );
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.token == "/JavaScript")
        );
        assert!(error.issues.iter().any(|issue| issue.token == "/Annots"));
        assert!(error.issues.iter().any(|issue| issue.token == "/Widget"));
        assert!(error.issues.iter().any(|issue| issue.token == "/URI"));
        assert!(error.issues.iter().any(|issue| issue.token == "/AF"));
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.token == "/FileAttachment")
        );
        assert!(error.issues.iter().any(|issue| issue.token == "/Encrypt"));
        assert!(error.issues.iter().any(|issue| issue.token == "/Perms"));
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.token == "/Collection")
        );
        assert!(error.issues.iter().any(|issue| issue.token == "/GoTo"));
        assert!(error.issues.iter().any(|issue| issue.token == "/Named"));
        assert!(error.issues.iter().any(|issue| issue.token == "/ResetForm"));
        assert!(error.issues.iter().any(|issue| issue.token == "/Rendition"));
    }

    #[test]
    fn passive_pdf_audit_ignores_visible_words_inside_content_streams() {
        let pdf = b"%PDF-1.7
1 0 obj
<< /Length 54 >>
stream
BT (/JavaScript /Launch /URI /Annots /Widget) Tj ET
endstream
endobj
%%EOF";

        audit_passive_pdf_bytes(pdf).unwrap();
    }

    #[test]
    fn writes_landscape_media_box() {
        let mut document = Document::default();
        document.page.width_twips = 15_840;
        document.page.height_twips = 12_240;
        document.page.landscape = true;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Landscape PDF".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let page = parsed.get_object(page_id).unwrap().as_dict().unwrap();
        let media_box = page.get(b"MediaBox").unwrap().as_array().unwrap();

        assert_eq!(pdf_number(media_box[2].clone()), Some(792.0));
        assert_eq!(pdf_number(media_box[3].clone()), Some(612.0));
    }

    #[test]
    fn writes_section_specific_media_boxes() {
        let mut document = Document::default();
        let mut second_page = PageSettings::default();
        second_page.width_twips = 10_080;
        second_page.height_twips = 7_200;
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "First page".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::SectionBreak,
            Block::SectionSettings(second_page),
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Second page".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_ids = parsed.get_pages().values().copied().collect::<Vec<_>>();
        let first = parsed.get_object(page_ids[0]).unwrap().as_dict().unwrap();
        let second = parsed.get_object(page_ids[1]).unwrap().as_dict().unwrap();
        let first_media_box = first.get(b"MediaBox").unwrap().as_array().unwrap();
        let second_media_box = second.get(b"MediaBox").unwrap().as_array().unwrap();

        assert_eq!(pdf_number(first_media_box[2].clone()), Some(612.0));
        assert_eq!(pdf_number(first_media_box[3].clone()), Some(792.0));
        assert_eq!(pdf_number(second_media_box[2].clone()), Some(504.0));
        assert_eq!(pdf_number(second_media_box[3].clone()), Some(360.0));
    }

    #[test]
    fn writes_column_layout_as_passive_text_and_rules() {
        let mut document = Document::default();
        document.page.column_count = 2;
        document.page.line_between_columns = true;
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Left".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::ColumnBreak,
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Right".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(parsed.get_pages().len(), 1);
        assert_eq!(content_text(&content), "LeftRight");
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "m")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
        assert!(!pdf.windows(b"/URI".len()).any(|window| window == b"/URI"));
    }

    #[test]
    fn writes_paragraph_borders_as_passive_lines() {
        let mut document = Document::default();
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 80,
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
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Bordered");
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "S")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
        assert!(!pdf.windows(b"/URI".len()).any(|window| window == b"/URI"));
    }

    #[test]
    fn writes_character_border_as_passive_strokes() {
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
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Boxed");
        assert!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "S")
                .count()
                >= 4
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "RG"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 1.0)
                && pdf_number(operation.operands[1].clone()).is_some_and(|value| value == 0.0)
                && pdf_number(operation.operands[2].clone()).is_some_and(|value| value == 0.0)
        }));
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
        assert!(!pdf.windows(b"/URI".len()).any(|window| window == b"/URI"));
    }

    #[test]
    fn writes_ellipse_shape_as_passive_bezier_path() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Ellipse,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
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
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "c")
                .count(),
            4
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "B")
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "RG"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 1.0)
                && pdf_number(operation.operands[1].clone())
                    .is_some_and(|value| (value - 0.5019608).abs() < 0.001)
                && pdf_number(operation.operands[2].clone()).is_some_and(|value| value == 0.0)
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone())
                    .is_some_and(|value| (value - (10.0 / 255.0)).abs() < 0.001)
                && pdf_number(operation.operands[1].clone())
                    .is_some_and(|value| (value - (20.0 / 255.0)).abs() < 0.001)
                && pdf_number(operation.operands[2].clone())
                    .is_some_and(|value| (value - (30.0 / 255.0)).abs() < 0.001)
        }));
        for forbidden in [
            b"/JavaScript".as_slice(),
            b"/EmbeddedFile",
            b"/Launch",
            b"/OpenAction",
            b"/RichMedia",
        ] {
            assert!(
                !pdf.windows(forbidden.len())
                    .any(|window| window == forbidden)
            );
        }
    }

    #[test]
    fn writes_rounded_rectangle_shape_as_passive_bezier_path() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::RoundedRectangle,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
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
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "c")
                .count(),
            4
        );
        assert!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "l")
                .count()
                >= 4
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "B")
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "RG"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 1.0)
                && pdf_number(operation.operands[1].clone())
                    .is_some_and(|value| (value - 0.5019608).abs() < 0.001)
                && pdf_number(operation.operands[2].clone()).is_some_and(|value| value == 0.0)
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone())
                    .is_some_and(|value| (value - (10.0 / 255.0)).abs() < 0.001)
                && pdf_number(operation.operands[1].clone())
                    .is_some_and(|value| (value - (20.0 / 255.0)).abs() < 0.001)
                && pdf_number(operation.operands[2].clone())
                    .is_some_and(|value| (value - (30.0 / 255.0)).abs() < 0.001)
        }));
        for forbidden in [
            b"/JavaScript".as_slice(),
            b"/EmbeddedFile",
            b"/Launch",
            b"/OpenAction",
            b"/RichMedia",
        ] {
            assert!(
                !pdf.windows(forbidden.len())
                    .any(|window| window == forbidden)
            );
        }
    }

    #[test]
    fn writes_border_styles_as_passive_pdf_strokes() {
        let mut document = Document::default();
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 60,
            color_index: None,
            style: BorderStyle::Double,
            ..TableCellBorder::default()
        };
        let mut character_style = CharacterStyle::default();
        character_style.border = TableCellBorder {
            visible: true,
            width_twips: 40,
            color_index: None,
            style: BorderStyle::Dashed,
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
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Styled");
        assert!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "S")
                .count()
                >= 6
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "d")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_wavy_borders_as_passive_path_strokes() {
        let mut document = Document::default();
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 80,
            color_index: None,
            style: BorderStyle::Wavy,
            ..TableCellBorder::default()
        };
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "Wavy".to_string(),
                style: CharacterStyle::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Wavy");
        assert!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "l")
                .count()
                > 2
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
        assert!(!pdf.windows(b"/URI".len()).any(|window| window == b"/URI"));
    }

    #[test]
    fn writes_passive_jpeg_image_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Jpeg,
            bytes: minimal_jpeg_with_dimensions(1, 1),
            palette: Vec::new(),
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/Subtype /Image".len())
                .any(|window| window == b"/Subtype /Image")
        );
        assert!(
            pdf.windows(b"/DCTDecode".len())
                .any(|window| window == b"/DCTDecode")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_passive_grayscale_jpeg_image_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::JpegGrayscale,
            bytes: minimal_grayscale_jpeg_with_dimensions(1, 1),
            palette: Vec::new(),
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/ColorSpace /DeviceGray".len())
                .any(|window| window == b"/ColorSpace /DeviceGray")
        );
        assert!(
            pdf.windows(b"/DCTDecode".len())
                .any(|window| window == b"/DCTDecode")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_passive_cmyk_jpeg_image_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::JpegCmyk,
            bytes: minimal_cmyk_jpeg_with_dimensions(1, 1),
            palette: Vec::new(),
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/ColorSpace /DeviceCMYK".len())
                .any(|window| window == b"/ColorSpace /DeviceCMYK")
        );
        assert!(
            pdf.windows(b"/DCTDecode".len())
                .any(|window| window == b"/DCTDecode")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_passive_png_image_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Png,
            bytes: minimal_png_idat_for_1x1_rgb(),
            palette: Vec::new(),
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/Subtype /Image".len())
                .any(|window| window == b"/Subtype /Image")
        );
        assert!(
            pdf.windows(b"/FlateDecode".len())
                .any(|window| window == b"/FlateDecode")
        );
        assert!(
            pdf.windows(b"/Predictor 15".len())
                .any(|window| window == b"/Predictor 15")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_passive_indexed_png_image_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::PngIndexed,
            bytes: minimal_png_idat_for_1x1_indexed(),
            palette: vec![255, 0, 0, 0, 255, 0],
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/Subtype /Image".len())
                .any(|window| window == b"/Subtype /Image")
        );
        assert!(
            pdf.windows(b"/Indexed /DeviceRGB 1".len())
                .any(|window| window == b"/Indexed /DeviceRGB 1")
        );
        assert!(
            pdf.windows(b"/FlateDecode".len())
                .any(|window| window == b"/FlateDecode")
        );
        assert!(
            pdf.windows(b"/Colors 1".len())
                .any(|window| window == b"/Colors 1")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_passive_raw_rgb_image_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Rgb8,
            bytes: vec![255, 0, 0, 0, 255, 0],
            palette: Vec::new(),
            vector_commands: Vec::new(),
            width_px: 2,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/Subtype /Image".len())
                .any(|window| window == b"/Subtype /Image")
        );
        assert!(
            !pdf.windows(b"/DCTDecode".len())
                .any(|window| window == b"/DCTDecode")
        );
        assert!(
            !pdf.windows(b"/Predictor".len())
                .any(|window| window == b"/Predictor")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_cropped_image_as_passive_clip_and_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Jpeg,
            bytes: minimal_jpeg_with_dimensions(100, 100),
            palette: Vec::new(),
            vector_commands: Vec::new(),
            width_px: 100,
            height_px: 100,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(1440),
            display_height_twips: Some(1440),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop {
                left_twips: 240,
                top_twips: 0,
                right_twips: 0,
                bottom_twips: 0,
            },
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "W")
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "Do")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
    }

    #[test]
    fn writes_text_color_and_highlight_as_passive_drawing() {
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
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && format!("{:?}", operation.operands) == "[0, 1, 0]"
        }));
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "re")
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "f")
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && format!("{:?}", operation.operands) == "[1, 0, 0]"
        }));
    }

    #[test]
    fn writes_character_shading_intensity_as_passive_fill() {
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
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Tinted");
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 1.0)
                && pdf_number(operation.operands[1].clone()).is_some_and(|value| value == 0.5)
                && pdf_number(operation.operands[2].clone()).is_some_and(|value| value == 0.5)
        }));
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "re")
        );
    }

    #[test]
    fn writes_courier_font_resource_for_monospace_rtf_font() {
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
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(
            pdf.windows(b"/BaseFont /Courier".len())
                .any(|window| window == b"/BaseFont /Courier")
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tf" && format!("{:?}", operation.operands).contains("/F5")
        }));
    }

    #[test]
    fn writes_base14_font_resources_from_rtf_family_hints() {
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
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tf" && format!("{:?}", operation.operands).contains("/F9")
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tf" && format!("{:?}", operation.operands).contains("/F5")
        }));
        for forbidden in [b"/JavaScript".as_slice(), b"/EmbeddedFile", b"/Launch"] {
            assert!(
                !pdf.windows(forbidden.len())
                    .any(|window| window == forbidden)
            );
        }
    }

    #[test]
    fn writes_courier_font_resource_for_fixed_pitch_font_hint() {
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
        ];
        let mut style = CharacterStyle::default();
        style.font_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Fixed".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tf" && format!("{:?}", operation.operands).contains("/F5")
        }));
        assert!(
            !pdf.windows(b"Mystery Fixed".len())
                .any(|window| window == b"Mystery Fixed")
        );
    }

    #[test]
    fn writes_table_cell_shading_as_passive_fill() {
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
            rows: vec![TableRow {
                height_twips: None,
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: crate::model::TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: Some(1),
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: crate::model::TableCellPadding::default(),
                    borders: crate::model::TableCellBorders::default(),
                    vertical_align: crate::model::TableCellVerticalAlign::Top,
                    horizontal_merge: crate::model::TableCellHorizontalMerge::None,
                    vertical_merge: crate::model::TableCellVerticalMerge::None,
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
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && format!("{:?}", operation.operands).contains("0.8627451")
        }));
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "re")
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "f")
        );
        assert!(content_text(&content).contains("Shaded"));
    }

    #[test]
    fn writes_caps_as_uppercase_text() {
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
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "MIXED CASE");
    }

    #[test]
    fn writes_small_caps_as_passive_scaled_uppercase_text() {
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
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "MIX");
        assert!(
            content.operations.iter().any(|operation| {
                operation.operator == "Tf" && format!("{:?}", operation.operands).contains("8.5")
            }),
            "lowercase small-caps glyphs should be emitted with a reduced passive font size"
        );
    }

    #[test]
    fn writes_word_underline_variants_as_passive_strokes() {
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
        let mut dotted_style = CharacterStyle::default();
        dotted_style.underline = UnderlineStyle::Dotted;
        let mut dashed_style = CharacterStyle::default();
        dashed_style.underline = UnderlineStyle::Dashed;
        let mut wave_style = CharacterStyle::default();
        wave_style.underline = UnderlineStyle::Wave;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    text: "double ".to_string(),
                    style: double_style,
                },
                Run {
                    text: "dotted ".to_string(),
                    style: dotted_style,
                },
                Run {
                    text: "dashed ".to_string(),
                    style: dashed_style,
                },
                Run {
                    text: "wave".to_string(),
                    style: wave_style,
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "double dotted dashed wave");
        assert!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "S")
                .count()
                >= 5
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "d")
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "RG"
                && operation.operands.len() == 3
                && format!("{:?}", operation.operands).contains('1')
        }));
    }

    #[test]
    fn writes_optional_hyphen_only_when_wrapping_uses_it() {
        let mut wide_document = Document::default();
        wide_document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Alpha\u{00ad}Beta".to_string(),
                style: Default::default(),
            }],
        })];
        let wide_layout = LayoutEngine::layout(&wide_document);
        let wide_pdf = render_pdf(&wide_layout);
        let wide_parsed = lopdf::Document::load_mem(&wide_pdf).unwrap();
        let wide_page_id = *wide_parsed.get_pages().values().next().expect("page");
        let wide_content = wide_parsed
            .get_and_decode_page_content(wide_page_id)
            .unwrap();
        assert_eq!(content_text(&wide_content), "AlphaBeta");

        let mut narrow_style = ParagraphStyle::default();
        narrow_style.right_indent_twips = 8_400;
        let mut narrow_document = Document::default();
        narrow_document.blocks = vec![Block::Paragraph(Paragraph {
            style: narrow_style,
            runs: vec![Run {
                text: "Alpha\u{00ad}Beta".to_string(),
                style: Default::default(),
            }],
        })];
        let narrow_layout = LayoutEngine::layout(&narrow_document);
        let narrow_pdf = render_pdf(&narrow_layout);
        let narrow_parsed = lopdf::Document::load_mem(&narrow_pdf).unwrap();
        let narrow_page_id = *narrow_parsed.get_pages().values().next().expect("page");
        let narrow_content = narrow_parsed
            .get_and_decode_page_content(narrow_page_id)
            .unwrap();
        assert_eq!(content_text(&narrow_content), "Alpha-Beta");
    }

    #[test]
    fn does_not_write_hidden_runs_to_pdf_text() {
        let mut hidden_style = CharacterStyle::default();
        hidden_style.hidden = true;
        let mut document = Document::default();
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
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "VisibleShown");
        assert!(
            !pdf.windows(b"Hidden".len())
                .any(|window| window == b"Hidden")
        );
    }

    #[test]
    fn writes_bounded_font_size_to_pdf_text_state() {
        let mut style = CharacterStyle::default();
        style.font_size_half_points = 96;
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Bounded".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Bounded");
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tf"
                && operation.operands.len() == 2
                && format!("{:?}", operation.operands[1]).contains("48")
        }));
    }

    #[test]
    fn writes_character_spacing_as_passive_pdf_text_state() {
        let mut style = CharacterStyle::default();
        style.character_spacing_twips = 200;
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Wide".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Wide");
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tc"
                && operation.operands.len() == 1
                && format!("{:?}", operation.operands[0]).contains("10")
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tc"
                && operation.operands.len() == 1
                && format!("{:?}", operation.operands[0]).contains('0')
        }));
    }

    #[test]
    fn writes_character_scaling_as_passive_pdf_text_state() {
        let mut style = CharacterStyle::default();
        style.character_scaling_percent = 150;
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Scaled".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Scaled");
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tz"
                && operation.operands.len() == 1
                && format!("{:?}", operation.operands[0]).contains("150")
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tz"
                && operation.operands.len() == 1
                && format!("{:?}", operation.operands[0]).contains("100")
        }));
    }

    #[test]
    fn writes_justified_word_spacing_as_passive_pdf_text_state() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.alignment = Alignment::Justified;
        let mut document = Document::default();
        document.page.width_twips = 5_000;
        document.page.margin_left_twips = 720;
        document.page.margin_right_twips = 720;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "one two three four five six seven eight nine ten".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(content_text(&content).contains("one"));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tw"
                && operation.operands.len() == 1
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value > 0.0)
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tw"
                && operation.operands.len() == 1
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 0.0)
        }));
    }

    #[test]
    fn writes_outline_text_as_passive_pdf_text_state() {
        let mut style = CharacterStyle::default();
        style.outline = true;
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Outline".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Outline");
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tr"
                && operation.operands.len() == 1
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 1.0)
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tr"
                && operation.operands.len() == 1
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 0.0)
        }));
    }

    #[test]
    fn writes_shadow_text_as_passive_offset_text() {
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
        style.shadow = true;
        style.color_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Shadow".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "ShadowShadow");
        assert_eq!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "Tj")
                .count(),
            2
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone())
                    .is_some_and(|value| (value - 0.35).abs() < 0.001)
                && pdf_number(operation.operands[1].clone()).is_some_and(|value| value == 0.0)
                && pdf_number(operation.operands[2].clone()).is_some_and(|value| value == 0.0)
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 1.0)
                && pdf_number(operation.operands[1].clone()).is_some_and(|value| value == 0.0)
                && pdf_number(operation.operands[2].clone()).is_some_and(|value| value == 0.0)
        }));
    }

    #[test]
    fn writes_relief_text_as_passive_offset_text_layers() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 0,
                green: 0,
                blue: 255,
            },
        ];
        let mut style = CharacterStyle::default();
        style.relief = TextRelief::Emboss;
        style.color_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Relief".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "ReliefReliefRelief");
        assert_eq!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "Tj")
                .count(),
            3
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 0.75)
                && pdf_number(operation.operands[1].clone()).is_some_and(|value| value == 0.75)
                && pdf_number(operation.operands[2].clone()).is_some_and(|value| value == 1.0)
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 0.0)
                && pdf_number(operation.operands[1].clone()).is_some_and(|value| value == 0.0)
                && pdf_number(operation.operands[2].clone())
                    .is_some_and(|value| (value - 0.35).abs() < 0.001)
        }));
    }

    #[test]
    fn encodes_word_symbol_text_as_safe_pdf_fallbacks() {
        let encoded = encode_pdf_text(
            "\u{2018}q\u{2019} \u{201c}qq\u{201d} \u{2014}\u{2013}\u{2011}\u{00ad} \u{2022} \u{00a0}\u{2002}\u{2003}\u{2005}",
        );

        assert_eq!(
            encoded,
            vec![
                0x91, b'q', 0x92, b' ', 0x93, b'q', b'q', 0x94, b' ', 0x97, 0x96, b'-', 0xad, b' ',
                0x95, b' ', 0xa0, b' ', b' ', b' '
            ]
        );
    }

    #[test]
    fn writes_winansi_font_encoding_and_text_bytes() {
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "\u{201c}Quote\u{201d} \u{2014} \u{2022}".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(
            pdf.windows(b"/Encoding /WinAnsiEncoding".len())
                .any(|window| window == b"/Encoding /WinAnsiEncoding")
        );
        assert_eq!(content_bytes(&content), b"\x93Quote\x94 \x97 \x95");
    }

    #[test]
    fn writes_symbol_font_encoding_for_symbol_charset_text() {
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
                text: "\u{03b1}\u{03b2} \u{2022}".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(
            pdf.windows(b"/BaseFont /Symbol".len())
                .any(|window| window == b"/BaseFont /Symbol")
        );
        assert_eq!(content_bytes_for_font(&content, b"F13"), b"ab ");
        assert_eq!(content_bytes_for_font(&content, b"F1"), b"\x95");
    }

    #[test]
    fn writes_zapf_dingbats_checkbox_glyphs() {
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
                name: "ZapfDingbats".to_string(),
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
                text: "\u{25a1}\u{2610}\u{2611}\u{2612}\u{2713}\u{2717}".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(
            pdf.windows(b"/BaseFont /ZapfDingbats".len())
                .any(|window| window == b"/BaseFont /ZapfDingbats")
        );
        assert_eq!(content_bytes(&content), b"qqqq37");
        assert_eq!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "m")
                .count(),
            3
        );
        assert_eq!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "l")
                .count(),
            4
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "S")
        );
    }

    #[test]
    fn writes_passive_kerning_as_positioned_text_without_control_payload() {
        let mut document = Document::default();
        let mut style = CharacterStyle {
            character_kerning_half_points: 2,
            ..Default::default()
        };
        style.font_size_half_points = 24;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "AV".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();
        let positioned = content
            .operations
            .iter()
            .find(|operation| operation.operator == "TJ")
            .expect("kerning should use positioned text");

        assert_eq!(content_bytes(&content), b"AV");
        assert!(
            positioned.operands.iter().any(|operand| {
                operand.as_array().is_ok_and(|items| {
                    items.iter().any(|item| {
                        matches!(item, lopdf::Object::Integer(value) if *value > 0)
                            || matches!(item, lopdf::Object::Real(value) if *value > 0.0)
                    })
                })
            }),
            "positioned text should include a positive kerning adjustment"
        );
        for forbidden in [b"kerning".as_slice(), b"/JavaScript", b"/EmbeddedFile"] {
            assert!(
                !pdf.windows(forbidden.len())
                    .any(|window| window == forbidden)
            );
        }
    }

    #[test]
    fn writes_resolved_page_numbers_without_internal_marker() {
        let mut document = Document::default();
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: format!("{PAGE_NUMBER_MARKER} of {TOTAL_PAGES_MARKER}"),
                style: Default::default(),
            }],
        }];
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "First".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::PageBreak,
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Second".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_ids = parsed.get_pages().values().copied().collect::<Vec<_>>();
        let first_text = content_text(&parsed.get_and_decode_page_content(page_ids[0]).unwrap());
        let second_text = content_text(&parsed.get_and_decode_page_content(page_ids[1]).unwrap());

        assert!(first_text.contains("1 of 2"));
        assert!(second_text.contains("2 of 2"));
        assert!(!first_text.contains(PAGE_NUMBER_MARKER));
        assert!(!second_text.contains(PAGE_NUMBER_MARKER));
        assert!(!first_text.contains(TOTAL_PAGES_MARKER));
        assert!(!second_text.contains(TOTAL_PAGES_MARKER));
    }

    #[test]
    fn writes_resolved_section_numbers_without_internal_marker() {
        let mut document = Document::default();
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: format!("Section {SECTION_NUMBER_MARKER}"),
                    style: Default::default(),
                }],
            }),
            Block::ContinuousSectionBreak,
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: format!("Section {SECTION_NUMBER_MARKER}"),
                    style: Default::default(),
                }],
            }),
            Block::SectionBreak,
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: format!("Section {SECTION_NUMBER_MARKER}"),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_ids = parsed.get_pages().values().copied().collect::<Vec<_>>();
        let first_text = content_text(&parsed.get_and_decode_page_content(page_ids[0]).unwrap());
        let second_text = content_text(&parsed.get_and_decode_page_content(page_ids[1]).unwrap());

        assert!(first_text.contains("Section 1"));
        assert!(first_text.contains("Section 2"));
        assert!(second_text.contains("Section 3"));
        assert!(!first_text.contains(SECTION_NUMBER_MARKER));
        assert!(!second_text.contains(SECTION_NUMBER_MARKER));
    }

    fn minimal_jpeg_with_dimensions(width: u16, height: u16) -> Vec<u8> {
        minimal_jpeg_with_components(width, height, 3)
    }

    fn minimal_grayscale_jpeg_with_dimensions(width: u16, height: u16) -> Vec<u8> {
        minimal_jpeg_with_components(width, height, 1)
    }

    fn minimal_cmyk_jpeg_with_dimensions(width: u16, height: u16) -> Vec<u8> {
        minimal_jpeg_with_components(width, height, 4)
    }

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

    fn minimal_png_idat_for_1x1_rgb() -> Vec<u8> {
        vec![
            0x78, 0x01, 0x01, 0x04, 0x00, 0xfb, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00,
            0x01,
        ]
    }

    fn minimal_png_idat_for_1x1_indexed() -> Vec<u8> {
        vec![
            0x78, 0x01, 0x01, 0x02, 0x00, 0xfd, 0xff, 0x00, 0x01, 0x00, 0x02, 0x00, 0x02,
        ]
    }

    fn pdf_number(object: lopdf::Object) -> Option<f32> {
        match object {
            lopdf::Object::Integer(value) => Some(value as f32),
            lopdf::Object::Real(value) => Some(value),
            _ => None,
        }
    }

    fn content_text(content: &lopdf::content::Content) -> String {
        let mut text = String::new();
        for operation in &content.operations {
            if operation.operator == "Tj" {
                for operand in &operation.operands {
                    if let Ok(bytes) = operand.as_str() {
                        text.push_str(&String::from_utf8_lossy(bytes));
                    }
                }
            } else if operation.operator == "TJ" {
                for operand in &operation.operands {
                    if let Ok(items) = operand.as_array() {
                        for item in items {
                            if let Ok(bytes) = item.as_str() {
                                text.push_str(&String::from_utf8_lossy(bytes));
                            }
                        }
                    }
                }
            }
        }
        text
    }

    fn content_bytes(content: &lopdf::content::Content) -> Vec<u8> {
        let mut text = Vec::new();
        for operation in &content.operations {
            if operation.operator == "Tj" {
                for operand in &operation.operands {
                    if let Ok(bytes) = operand.as_str() {
                        text.extend_from_slice(bytes);
                    }
                }
            } else if operation.operator == "TJ" {
                for operand in &operation.operands {
                    if let Ok(items) = operand.as_array() {
                        for item in items {
                            if let Ok(bytes) = item.as_str() {
                                text.extend_from_slice(bytes);
                            }
                        }
                    }
                }
            }
        }
        text
    }

    fn content_bytes_for_font(content: &lopdf::content::Content, font_name: &[u8]) -> Vec<u8> {
        let mut current_font: Option<Vec<u8>> = None;
        let mut text = Vec::new();
        for operation in &content.operations {
            if operation.operator == "Tf" {
                current_font = operation
                    .operands
                    .first()
                    .and_then(|operand| operand.as_name().ok().map(|name| name.to_vec()));
            } else if operation.operator == "Tj" && current_font.as_deref() == Some(font_name) {
                for operand in &operation.operands {
                    if let Ok(bytes) = operand.as_str() {
                        text.extend_from_slice(bytes);
                    }
                }
            } else if operation.operator == "TJ" && current_font.as_deref() == Some(font_name) {
                for operand in &operation.operands {
                    if let Ok(items) = operand.as_array() {
                        for item in items {
                            if let Ok(bytes) = item.as_str() {
                                text.extend_from_slice(bytes);
                            }
                        }
                    }
                }
            }
        }
        text
    }
}
