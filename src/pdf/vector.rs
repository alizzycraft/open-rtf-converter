//! Recovered vector commands, paths, fills, gradients, hatches, and shape drawing.

use super::*;

pub(in crate::pdf) fn draw_passive_wmf_vector_image(
    content: &mut Content,
    fragment: &crate::layout::ImageFragment,
    page_images: &[PdfImageRef],
    image_ref_cursor: &mut usize,
) {
    let draw = image_draw_rect(fragment);
    if draw.fully_cropped {
        *image_ref_cursor = (*image_ref_cursor).saturating_add(
            fragment
                .image
                .vector_commands
                .iter()
                .filter(|command| matches!(command, StaticImageVectorCommand::RasterImage { .. }))
                .count(),
        );
        return;
    }
    let source_width = fragment.image.width_px.max(1) as f32;
    let source_height = fragment.image.height_px.max(1) as f32;

    content.save_state();
    content.rect(fragment.x, fragment.y, fragment.width, fragment.height);
    content.clip_nonzero();
    content.end_path();
    let mut vector_state_depth = 0usize;
    for command in &fragment.image.vector_commands {
        match command {
            StaticImageVectorCommand::SaveState => {
                content.save_state();
                vector_state_depth = vector_state_depth.saturating_add(1);
            }
            StaticImageVectorCommand::RestoreState => {
                if vector_state_depth > 0 {
                    content.restore_state();
                    vector_state_depth -= 1;
                }
            }
            StaticImageVectorCommand::SetMiterLimit { limit } => {
                content.set_miter_limit((*limit).clamp(1.0, 256.0));
            }
            StaticImageVectorCommand::ClipRect {
                left,
                top,
                right,
                bottom,
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
                content.rect(rect.x, rect.y, rect.width, rect.height);
                content.clip_nonzero();
                content.end_path();
            }
            StaticImageVectorCommand::ClipPath {
                start,
                segments,
                closed,
                fill_rule,
            } => {
                let start =
                    vector_command_point(draw, source_width, source_height, start.0, start.1);
                let segments =
                    vector_command_path_segments(draw, source_width, source_height, segments);
                append_passive_vector_path(content, start, &segments, *closed);
                match fill_rule {
                    StaticImageVectorFillRule::Alternate => {
                        content.clip_even_odd();
                    }
                    StaticImageVectorFillRule::Winding => {
                        content.clip_nonzero();
                    }
                }
                content.end_path();
            }
            StaticImageVectorCommand::Line {
                x1,
                y1,
                x2,
                y2,
                stroke_color,
                stroke_width,
                stroke_style,
            } => {
                let stroke_width =
                    vector_command_stroke_width(draw, source_width, source_height, *stroke_width);
                let stroke_style = vector_command_line_style(*stroke_style);
                draw_passive_vector_line(
                    content,
                    vector_command_point(draw, source_width, source_height, *x1, *y1),
                    vector_command_point(draw, source_width, source_height, *x2, *y2),
                    *stroke_color,
                    stroke_width,
                    stroke_style,
                );
            }
            StaticImageVectorCommand::SquareCappedLine {
                x1,
                y1,
                x2,
                y2,
                stroke_color,
                stroke_width,
                stroke_style,
            } => {
                let stroke_width =
                    vector_command_stroke_width(draw, source_width, source_height, *stroke_width);
                let stroke_style = vector_command_line_style(*stroke_style);
                draw_passive_vector_capped_line(
                    content,
                    vector_command_point(draw, source_width, source_height, *x1, *y1),
                    vector_command_point(draw, source_width, source_height, *x2, *y2),
                    *stroke_color,
                    stroke_width,
                    stroke_style,
                    LineCap::Square,
                );
            }
            StaticImageVectorCommand::Polyline {
                points,
                stroke_color,
                stroke_width,
                stroke_style,
            } => {
                let points = vector_command_points(draw, source_width, source_height, points);
                let stroke_width =
                    vector_command_stroke_width(draw, source_width, source_height, *stroke_width);
                let stroke_style = vector_command_line_style(*stroke_style);
                draw_passive_vector_polyline(
                    content,
                    &points,
                    *stroke_color,
                    stroke_width,
                    stroke_style,
                );
            }
            StaticImageVectorCommand::Bezier {
                points,
                stroke_color,
                stroke_width,
                stroke_style,
            } => {
                let points = vector_command_points(draw, source_width, source_height, points);
                let stroke_width =
                    vector_command_stroke_width(draw, source_width, source_height, *stroke_width);
                let stroke_style = vector_command_line_style(*stroke_style);
                draw_passive_vector_bezier(
                    content,
                    &points,
                    *stroke_color,
                    stroke_width,
                    stroke_style,
                );
            }
            StaticImageVectorCommand::Polygon {
                points,
                stroke_color,
                stroke_width,
                stroke_style,
                fill_rule,
                fill_pattern,
                fill_color,
            } => {
                let points = vector_command_points(draw, source_width, source_height, points);
                let stroke_width =
                    vector_command_stroke_width(draw, source_width, source_height, *stroke_width);
                let stroke_style = vector_command_line_style(*stroke_style);
                draw_passive_vector_polygon(
                    content,
                    &points,
                    *stroke_color,
                    stroke_width,
                    stroke_style,
                    *fill_rule,
                    *fill_pattern,
                    *fill_color,
                );
            }
            StaticImageVectorCommand::Path {
                start,
                segments,
                closed,
                stroke_color,
                stroke_width,
                stroke_style,
                fill_rule,
                fill_pattern,
                fill_color,
            } => {
                let start =
                    vector_command_point(draw, source_width, source_height, start.0, start.1);
                let segments =
                    vector_command_path_segments(draw, source_width, source_height, segments);
                let stroke_width =
                    vector_command_stroke_width(draw, source_width, source_height, *stroke_width);
                let stroke_style = vector_command_line_style(*stroke_style);
                draw_passive_vector_path(
                    content,
                    start,
                    &segments,
                    *closed,
                    *stroke_color,
                    stroke_width,
                    stroke_style,
                    *fill_rule,
                    *fill_pattern,
                    *fill_color,
                );
            }
            StaticImageVectorCommand::Rectangle {
                left,
                top,
                right,
                bottom,
                stroke_color,
                stroke_width,
                stroke_style,
                fill_pattern,
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
                let stroke_width =
                    vector_command_stroke_width(draw, source_width, source_height, *stroke_width);
                let stroke_style = vector_command_line_style(*stroke_style);
                draw_passive_vector_rectangle(
                    content,
                    rect,
                    *stroke_color,
                    stroke_width,
                    stroke_style,
                    *fill_pattern,
                    *fill_color,
                );
            }
            StaticImageVectorCommand::RoundedRectangle {
                left,
                top,
                right,
                bottom,
                corner_width,
                corner_height,
                stroke_color,
                stroke_width,
                stroke_style,
                fill_pattern,
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
                let corner_width = (*corner_width / source_width) * draw.width;
                let corner_height = (*corner_height / source_height) * draw.height;
                let stroke_width =
                    vector_command_stroke_width(draw, source_width, source_height, *stroke_width);
                let stroke_style = vector_command_line_style(*stroke_style);
                draw_passive_vector_rounded_rectangle(
                    content,
                    rect,
                    corner_width,
                    corner_height,
                    *stroke_color,
                    stroke_width,
                    stroke_style,
                    *fill_pattern,
                    *fill_color,
                );
            }
            StaticImageVectorCommand::Ellipse {
                left,
                top,
                right,
                bottom,
                stroke_color,
                stroke_width,
                stroke_style,
                fill_pattern,
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
                let stroke_width =
                    vector_command_stroke_width(draw, source_width, source_height, *stroke_width);
                let stroke_style = vector_command_line_style(*stroke_style);
                draw_passive_vector_ellipse(
                    content,
                    rect,
                    *stroke_color,
                    stroke_width,
                    stroke_style,
                    *fill_pattern,
                    *fill_color,
                );
            }
            StaticImageVectorCommand::RasterImage {
                left,
                top,
                right,
                bottom,
                ..
            } => {
                let Some(image_ref) = page_images.get(*image_ref_cursor) else {
                    continue;
                };
                *image_ref_cursor = (*image_ref_cursor).saturating_add(1);
                let rect = vector_command_rect(
                    draw,
                    source_width,
                    source_height,
                    *left,
                    *top,
                    *right,
                    *bottom,
                );
                content.save_state();
                content.transform([rect.width, 0.0, 0.0, rect.height, rect.x, rect.y]);
                content.x_object(Name(&image_ref.name));
                content.restore_state();
            }
            StaticImageVectorCommand::Text {
                x,
                y,
                height,
                bold,
                vertical_scale_percent,
                text,
                color,
                background_color,
                clip_bounds,
                character_extra,
                word_extra,
                horizontal_align,
                vertical_align,
            } => {
                let point = vector_command_point(draw, source_width, source_height, *x, *y);
                let font_size = ((*height / source_height) * draw.height).clamp(4.0, 72.0);
                let character_extra = ((*character_extra / source_width) * draw.width)
                    .clamp(-font_size, draw.width.max(font_size * 4.0));
                let word_extra =
                    ((*word_extra / source_width) * draw.width).clamp(-font_size, font_size * 8.0);
                let clip_rect = clip_bounds.map(|bounds| {
                    vector_command_rect(
                        draw,
                        source_width,
                        source_height,
                        bounds.left,
                        bounds.top,
                        bounds.right,
                        bounds.bottom,
                    )
                });
                draw_passive_vector_text(
                    content,
                    point,
                    font_size,
                    *bold,
                    *vertical_scale_percent,
                    text,
                    *color,
                    *background_color,
                    clip_rect,
                    character_extra,
                    word_extra,
                    *horizontal_align,
                    *vertical_align,
                );
            }
        }
    }
    for _ in 0..vector_state_depth {
        content.restore_state();
    }
    content.restore_state();
}

pub(in crate::pdf) fn vector_command_points(
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

pub(in crate::pdf) fn vector_command_point(
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

pub(in crate::pdf) fn vector_command_path_segments(
    draw: ImageDrawRect,
    source_width: f32,
    source_height: f32,
    segments: &[StaticImageVectorPathSegment],
) -> Vec<StaticImageVectorPathSegment> {
    segments
        .iter()
        .map(|segment| match segment {
            StaticImageVectorPathSegment::MoveTo(x, y) => {
                let point = vector_command_point(draw, source_width, source_height, *x, *y);
                StaticImageVectorPathSegment::MoveTo(point.x, point.y)
            }
            StaticImageVectorPathSegment::LineTo(x, y) => {
                let point = vector_command_point(draw, source_width, source_height, *x, *y);
                StaticImageVectorPathSegment::LineTo(point.x, point.y)
            }
            StaticImageVectorPathSegment::CubicTo {
                control1,
                control2,
                end,
            } => {
                let control1 =
                    vector_command_point(draw, source_width, source_height, control1.0, control1.1);
                let control2 =
                    vector_command_point(draw, source_width, source_height, control2.0, control2.1);
                let end = vector_command_point(draw, source_width, source_height, end.0, end.1);
                StaticImageVectorPathSegment::CubicTo {
                    control1: (control1.x, control1.y),
                    control2: (control2.x, control2.y),
                    end: (end.x, end.y),
                }
            }
        })
        .collect()
}

#[derive(Debug, Copy, Clone)]
pub(in crate::pdf) struct VectorDrawRect {
    pub(in crate::pdf) x: f32,
    pub(in crate::pdf) y: f32,
    pub(in crate::pdf) width: f32,
    pub(in crate::pdf) height: f32,
}

pub(in crate::pdf) fn vector_command_rect(
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

pub(in crate::pdf) fn vector_command_stroke_width(
    draw: ImageDrawRect,
    source_width: f32,
    source_height: f32,
    width: f32,
) -> f32 {
    let width = width.max(0.0);
    if width == 0.0 {
        return 0.0;
    }
    let x_width = (width / source_width.max(1.0)) * draw.width;
    let y_width = (width / source_height.max(1.0)) * draw.height;
    ((x_width + y_width) * 0.5).clamp(0.25, 24.0)
}

pub(in crate::pdf) fn vector_command_line_style(style: BorderStyle) -> LineStyle {
    match style {
        BorderStyle::Dotted => LineStyle::Dotted,
        BorderStyle::Dashed => LineStyle::Dashed,
        BorderStyle::Emboss => LineStyle::Emboss,
        BorderStyle::Engrave => LineStyle::Engrave,
        BorderStyle::ThinThick => LineStyle::ThinThick,
        BorderStyle::ThickThin => LineStyle::ThickThin,
        BorderStyle::ThinThickThin => LineStyle::ThinThickThin,
        _ => LineStyle::Solid,
    }
}

pub(in crate::pdf) fn draw_passive_vector_line(
    content: &mut Content,
    from: crate::layout::LayoutPoint,
    to: crate::layout::LayoutPoint,
    stroke_color: Option<crate::model::Color>,
    stroke_width: f32,
    stroke_style: LineStyle,
) {
    let Some(color) = stroke_color else {
        return;
    };
    if stroke_width <= 0.0 {
        return;
    }
    draw_passive_line(
        content,
        from.x,
        from.y,
        to.x,
        to.y,
        stroke_width,
        pdf_color_from_model(color),
        stroke_style,
        LineCap::Flat,
    );
}

pub(in crate::pdf) fn draw_passive_vector_capped_line(
    content: &mut Content,
    from: crate::layout::LayoutPoint,
    to: crate::layout::LayoutPoint,
    stroke_color: Option<crate::model::Color>,
    stroke_width: f32,
    stroke_style: LineStyle,
    cap: LineCap,
) {
    let Some(color) = stroke_color else {
        return;
    };
    if stroke_width <= 0.0 {
        return;
    }
    draw_passive_line(
        content,
        from.x,
        from.y,
        to.x,
        to.y,
        stroke_width,
        pdf_color_from_model(color),
        stroke_style,
        cap,
    );
}

pub(in crate::pdf) fn passive_visual_text_for_fragment(fragment: &TextFragment) -> Option<String> {
    if !fragment.style.right_to_left || !text_has_strong_rtl_script(&fragment.text) {
        return None;
    }
    Some(fragment.text.chars().rev().collect())
}

pub(in crate::pdf) fn text_has_strong_rtl_script(text: &str) -> bool {
    text.chars().any(is_strong_rtl_script_char)
}

pub(in crate::pdf) fn is_strong_rtl_script_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{0590}'..='\u{05ff}'
            | '\u{0600}'..='\u{06ff}'
            | '\u{0750}'..='\u{077f}'
            | '\u{08a0}'..='\u{08ff}'
            | '\u{fb1d}'..='\u{fdff}'
            | '\u{fe70}'..='\u{fefc}'
    )
}

pub(in crate::pdf) fn draw_passive_vector_polyline(
    content: &mut Content,
    points: &[crate::layout::LayoutPoint],
    stroke_color: Option<crate::model::Color>,
    stroke_width: f32,
    stroke_style: LineStyle,
) {
    if points.len() < 2 {
        return;
    }
    let Some(color) = stroke_color else {
        return;
    };
    if stroke_width <= 0.0 {
        return;
    }
    set_stroke_color(content, pdf_color_from_model(color));
    content.set_line_width(stroke_width.max(0.25));
    set_passive_path_stroke_style(content, stroke_width, stroke_style);
    content.move_to(points[0].x, points[0].y);
    for point in &points[1..] {
        content.line_to(point.x, point.y);
    }
    content.stroke();
}

pub(in crate::pdf) fn draw_passive_vector_bezier(
    content: &mut Content,
    points: &[crate::layout::LayoutPoint],
    stroke_color: Option<crate::model::Color>,
    stroke_width: f32,
    stroke_style: LineStyle,
) {
    let Some(color) = stroke_color else {
        return;
    };
    if stroke_width <= 0.0 || points.len() < 4 || (points.len() - 1) % 3 != 0 {
        return;
    }
    set_stroke_color(content, pdf_color_from_model(color));
    content.set_line_width(stroke_width.max(0.25));
    set_passive_path_stroke_style(content, stroke_width, stroke_style);
    content.move_to(points[0].x, points[0].y);
    for chunk in points[1..].chunks_exact(3) {
        content.cubic_to(
            chunk[0].x, chunk[0].y, chunk[1].x, chunk[1].y, chunk[2].x, chunk[2].y,
        );
    }
    content.stroke();
}

pub(in crate::pdf) fn draw_passive_vector_polygon(
    content: &mut Content,
    points: &[crate::layout::LayoutPoint],
    stroke_color: Option<crate::model::Color>,
    stroke_width: f32,
    stroke_style: LineStyle,
    fill_rule: StaticImageVectorFillRule,
    fill_pattern: ShadingPattern,
    fill_color: Option<crate::model::Color>,
) {
    if points.len() < 3 {
        return;
    }
    if fill_pattern != ShadingPattern::None
        && let Some(fill_color) = fill_color
    {
        draw_passive_hatch_polygon(
            content,
            points,
            fill_rule,
            fill_pattern,
            pdf_color_from_model(fill_color),
        );
        if stroke_color.is_none() || stroke_width <= 0.0 {
            return;
        }
        draw_passive_polygon(
            content,
            points,
            stroke_width,
            stroke_color.map(pdf_color_from_model).unwrap_or(PdfColor {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
            }),
            stroke_style,
            fill_rule,
            None,
            None,
            ShadingPattern::None,
        );
        return;
    }
    draw_passive_polygon(
        content,
        points,
        stroke_color.map(|_| stroke_width).unwrap_or(0.0),
        stroke_color.map(pdf_color_from_model).unwrap_or(PdfColor {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
        }),
        stroke_style,
        fill_rule,
        fill_color.map(pdf_color_from_model),
        None,
        ShadingPattern::None,
    );
}

pub(in crate::pdf) fn draw_passive_vector_path(
    content: &mut Content,
    start: crate::layout::LayoutPoint,
    segments: &[StaticImageVectorPathSegment],
    closed: bool,
    stroke_color: Option<crate::model::Color>,
    stroke_width: f32,
    stroke_style: LineStyle,
    fill_rule: StaticImageVectorFillRule,
    fill_pattern: ShadingPattern,
    fill_color: Option<crate::model::Color>,
) {
    if segments.is_empty() {
        return;
    }
    if fill_color.is_none() && stroke_color.is_none() {
        return;
    }

    let has_stroke = stroke_color.is_some() && stroke_width > 0.0;
    if fill_pattern != ShadingPattern::None
        && let Some(color) = fill_color
    {
        draw_passive_hatch_path(
            content,
            start,
            segments,
            closed,
            fill_rule,
            fill_pattern,
            pdf_color_from_model(color),
        );
        if !has_stroke {
            return;
        }
        content.save_state();
        if let Some(color) = stroke_color {
            set_stroke_color(content, pdf_color_from_model(color));
            content.set_line_width(stroke_width.max(0.25));
            set_passive_path_stroke_style(content, stroke_width, stroke_style);
        }
        append_passive_vector_path(content, start, segments, closed);
        content.stroke();
        content.restore_state();
        return;
    }

    content.save_state();
    if let Some(color) = stroke_color {
        if has_stroke {
            set_stroke_color(content, pdf_color_from_model(color));
            content.set_line_width(stroke_width.max(0.25));
            set_passive_path_stroke_style(content, stroke_width, stroke_style);
        }
    }
    if let Some(color) = fill_color {
        set_fill_color(content, pdf_color_from_model(color));
    }
    append_passive_vector_path(content, start, segments, closed);
    if fill_color.is_some() && has_stroke {
        match fill_rule {
            StaticImageVectorFillRule::Alternate => {
                content.fill_even_odd_and_stroke();
            }
            StaticImageVectorFillRule::Winding => {
                content.fill_nonzero_and_stroke();
            }
        }
    } else if fill_color.is_some() {
        match fill_rule {
            StaticImageVectorFillRule::Alternate => {
                content.fill_even_odd();
            }
            StaticImageVectorFillRule::Winding => {
                content.fill_nonzero();
            }
        }
    } else if has_stroke {
        content.stroke();
    } else {
        content.end_path();
    }
    content.restore_state();
}

pub(in crate::pdf) fn append_passive_vector_path(
    content: &mut Content,
    start: crate::layout::LayoutPoint,
    segments: &[StaticImageVectorPathSegment],
    closed: bool,
) {
    content.move_to(start.x, start.y);
    for segment in segments {
        match segment {
            StaticImageVectorPathSegment::MoveTo(x, y) => {
                content.move_to(*x, *y);
            }
            StaticImageVectorPathSegment::LineTo(x, y) => {
                content.line_to(*x, *y);
            }
            StaticImageVectorPathSegment::CubicTo {
                control1,
                control2,
                end,
            } => {
                content.cubic_to(control1.0, control1.1, control2.0, control2.1, end.0, end.1);
            }
        }
    }
    if closed {
        content.close_path();
    }
}

pub(in crate::pdf) fn draw_passive_vector_rectangle(
    content: &mut Content,
    rect: VectorDrawRect,
    stroke_color: Option<crate::model::Color>,
    stroke_width: f32,
    stroke_style: LineStyle,
    fill_pattern: ShadingPattern,
    fill_color: Option<crate::model::Color>,
) {
    if fill_color.is_none() && stroke_color.is_none() {
        return;
    }
    if let Some(color) = fill_color
        && fill_pattern == ShadingPattern::None
    {
        set_fill_color(content, pdf_color_from_model(color));
    } else if let Some(color) = fill_color {
        draw_passive_hatch_rect(content, rect, fill_pattern, pdf_color_from_model(color));
    }
    if let Some(color) = stroke_color {
        set_stroke_color(content, pdf_color_from_model(color));
        content.set_line_width(stroke_width.max(0.25));
        set_passive_path_stroke_style(content, stroke_width, stroke_style);
    }
    content.rect(rect.x, rect.y, rect.width, rect.height);
    match (fill_color, stroke_color, fill_pattern) {
        (Some(_), Some(_), ShadingPattern::None) => {
            content.fill_nonzero_and_stroke();
        }
        (Some(_), None, ShadingPattern::None) => {
            content.fill_nonzero();
        }
        (_, Some(_), _) => {
            content.stroke();
        }
        _ => {
            content.end_path();
        }
    }
}

pub(in crate::pdf) fn draw_passive_hatch_rect(
    content: &mut Content,
    rect: VectorDrawRect,
    pattern: ShadingPattern,
    color: PdfColor,
) {
    if pattern == ShadingPattern::None || rect.width <= 0.5 || rect.height <= 0.5 {
        return;
    }
    content.save_state();
    content.rect(rect.x, rect.y, rect.width, rect.height);
    content.clip_nonzero();
    content.end_path();
    draw_passive_hatch_lines(content, rect, pattern, color);
    content.restore_state();
}

pub(in crate::pdf) fn draw_passive_gradient_bands(
    content: &mut Content,
    rect: VectorDrawRect,
    start_color: PdfColor,
    end_color: Option<PdfColor>,
) {
    if rect.width <= 0.5 || rect.height <= 0.5 {
        return;
    }
    const BANDS: usize = 16;
    let end_color = end_color.unwrap_or(PdfColor {
        red: 1.0,
        green: 1.0,
        blue: 1.0,
    });
    let band_height = rect.height / BANDS as f32;
    for index in 0..BANDS {
        let t = index as f32 / (BANDS - 1) as f32;
        set_fill_color(
            content,
            PdfColor {
                red: start_color.red + ((end_color.red - start_color.red) * t),
                green: start_color.green + ((end_color.green - start_color.green) * t),
                blue: start_color.blue + ((end_color.blue - start_color.blue) * t),
            },
        );
        content.rect(
            rect.x,
            rect.y + (band_height * index as f32),
            rect.width,
            if index + 1 == BANDS {
                rect.height - (band_height * index as f32)
            } else {
                band_height + 0.1
            },
        );
        content.fill_nonzero();
    }
}

pub(in crate::pdf) fn draw_passive_gradient_polygon(
    content: &mut Content,
    points: &[crate::layout::LayoutPoint],
    fill_rule: StaticImageVectorFillRule,
    start_color: PdfColor,
    end_color: Option<PdfColor>,
) {
    let Some(bounds) = vector_points_bounds(points) else {
        return;
    };
    if bounds.width <= 0.5 || bounds.height <= 0.5 {
        return;
    }
    content.save_state();
    append_passive_polygon_path(content, points);
    match fill_rule {
        StaticImageVectorFillRule::Alternate => content.clip_even_odd(),
        StaticImageVectorFillRule::Winding => content.clip_nonzero(),
    };
    content.end_path();
    draw_passive_gradient_bands(content, bounds, start_color, end_color);
    content.restore_state();
}

pub(in crate::pdf) fn draw_passive_gradient_compound_polygon(
    content: &mut Content,
    points: &[crate::layout::LayoutPoint],
    paths: &[Vec<crate::layout::LayoutPoint>],
    fill_rule: StaticImageVectorFillRule,
    start_color: PdfColor,
    end_color: Option<PdfColor>,
) {
    let mut bounds = vector_points_bounds(points).or_else(|| {
        paths
            .iter()
            .find_map(|path| vector_points_bounds(path.as_slice()))
    });
    let Some(mut bounds) = bounds.take() else {
        return;
    };
    for path in paths.iter().filter_map(|path| vector_points_bounds(path)) {
        let min_x = bounds.x.min(path.x);
        let min_y = bounds.y.min(path.y);
        let max_x = (bounds.x + bounds.width).max(path.x + path.width);
        let max_y = (bounds.y + bounds.height).max(path.y + path.height);
        bounds = VectorDrawRect {
            x: min_x,
            y: min_y,
            width: (max_x - min_x).max(0.1),
            height: (max_y - min_y).max(0.1),
        };
    }
    if bounds.width <= 0.5 || bounds.height <= 0.5 {
        return;
    }
    content.save_state();
    if points.len() >= 3 {
        append_passive_polygon_path(content, points);
    }
    for path in paths {
        if path.len() >= 3 {
            append_passive_polygon_path(content, path);
        }
    }
    match fill_rule {
        StaticImageVectorFillRule::Alternate => content.clip_even_odd(),
        StaticImageVectorFillRule::Winding => content.clip_nonzero(),
    };
    content.end_path();
    draw_passive_gradient_bands(content, bounds, start_color, end_color);
    content.restore_state();
}

pub(in crate::pdf) fn draw_passive_gradient_rounded_rectangle(
    content: &mut Content,
    rect: VectorDrawRect,
    radius: f32,
    start_color: PdfColor,
    end_color: Option<PdfColor>,
) {
    if rect.width <= 0.5 || rect.height <= 0.5 {
        return;
    }
    content.save_state();
    append_passive_rounded_rectangle_path(content, rect.x, rect.y, rect.width, rect.height, radius);
    content.clip_nonzero();
    content.end_path();
    draw_passive_gradient_bands(content, rect, start_color, end_color);
    content.restore_state();
}

pub(in crate::pdf) fn draw_passive_gradient_ellipse(
    content: &mut Content,
    rect: VectorDrawRect,
    start_color: PdfColor,
    end_color: Option<PdfColor>,
) {
    if rect.width <= 0.5 || rect.height <= 0.5 {
        return;
    }
    content.save_state();
    append_passive_ellipse_path(content, rect.x, rect.y, rect.width, rect.height);
    content.clip_nonzero();
    content.end_path();
    draw_passive_gradient_bands(content, rect, start_color, end_color);
    content.restore_state();
}

pub(in crate::pdf) fn draw_passive_hatch_polygon(
    content: &mut Content,
    points: &[crate::layout::LayoutPoint],
    fill_rule: StaticImageVectorFillRule,
    pattern: ShadingPattern,
    color: PdfColor,
) {
    let Some(bounds) = vector_points_bounds(points) else {
        return;
    };
    if pattern == ShadingPattern::None || bounds.width <= 0.5 || bounds.height <= 0.5 {
        return;
    }
    content.save_state();
    append_passive_polygon_path(content, points);
    match fill_rule {
        StaticImageVectorFillRule::Alternate => {
            content.clip_even_odd();
        }
        StaticImageVectorFillRule::Winding => {
            content.clip_nonzero();
        }
    }
    content.end_path();
    draw_passive_hatch_lines(content, bounds, pattern, color);
    content.restore_state();
}

pub(in crate::pdf) fn draw_passive_hatch_compound_polygon(
    content: &mut Content,
    points: &[crate::layout::LayoutPoint],
    paths: &[Vec<crate::layout::LayoutPoint>],
    fill_rule: StaticImageVectorFillRule,
    pattern: ShadingPattern,
    color: PdfColor,
) {
    let mut bounds = vector_points_bounds(points).or_else(|| {
        paths
            .iter()
            .find_map(|path| vector_points_bounds(path.as_slice()))
    });
    let Some(mut bounds) = bounds.take() else {
        return;
    };
    if pattern == ShadingPattern::None || bounds.width <= 0.5 || bounds.height <= 0.5 {
        return;
    }
    for path in paths.iter().filter_map(|path| vector_points_bounds(path)) {
        let min_x = bounds.x.min(path.x);
        let min_y = bounds.y.min(path.y);
        let max_x = (bounds.x + bounds.width).max(path.x + path.width);
        let max_y = (bounds.y + bounds.height).max(path.y + path.height);
        bounds = VectorDrawRect {
            x: min_x,
            y: min_y,
            width: (max_x - min_x).max(0.1),
            height: (max_y - min_y).max(0.1),
        };
    }
    content.save_state();
    if points.len() >= 3 {
        append_passive_polygon_path(content, points);
    }
    for path in paths {
        if path.len() >= 3 {
            append_passive_polygon_path(content, path);
        }
    }
    match fill_rule {
        StaticImageVectorFillRule::Alternate => {
            content.clip_even_odd();
        }
        StaticImageVectorFillRule::Winding => {
            content.clip_nonzero();
        }
    }
    content.end_path();
    draw_passive_hatch_lines(content, bounds, pattern, color);
    content.restore_state();
}

pub(in crate::pdf) fn draw_passive_hatch_path(
    content: &mut Content,
    start: crate::layout::LayoutPoint,
    segments: &[StaticImageVectorPathSegment],
    closed: bool,
    fill_rule: StaticImageVectorFillRule,
    pattern: ShadingPattern,
    color: PdfColor,
) {
    let Some(bounds) = vector_path_bounds(start, segments) else {
        return;
    };
    if pattern == ShadingPattern::None || bounds.width <= 0.5 || bounds.height <= 0.5 {
        return;
    }
    content.save_state();
    append_passive_vector_path(content, start, segments, closed);
    match fill_rule {
        StaticImageVectorFillRule::Alternate => {
            content.clip_even_odd();
        }
        StaticImageVectorFillRule::Winding => {
            content.clip_nonzero();
        }
    }
    content.end_path();
    draw_passive_hatch_lines(content, bounds, pattern, color);
    content.restore_state();
}

pub(in crate::pdf) fn draw_passive_hatch_rounded_rectangle(
    content: &mut Content,
    rect: VectorDrawRect,
    radius: f32,
    pattern: ShadingPattern,
    color: PdfColor,
) {
    if pattern == ShadingPattern::None || rect.width <= 0.5 || rect.height <= 0.5 {
        return;
    }
    content.save_state();
    append_passive_rounded_rectangle_path(content, rect.x, rect.y, rect.width, rect.height, radius);
    content.clip_nonzero();
    content.end_path();
    draw_passive_hatch_lines(content, rect, pattern, color);
    content.restore_state();
}

pub(in crate::pdf) fn draw_passive_hatch_ellipse(
    content: &mut Content,
    rect: VectorDrawRect,
    pattern: ShadingPattern,
    color: PdfColor,
) {
    if pattern == ShadingPattern::None || rect.width <= 0.5 || rect.height <= 0.5 {
        return;
    }
    content.save_state();
    append_passive_ellipse_path(content, rect.x, rect.y, rect.width, rect.height);
    content.clip_nonzero();
    content.end_path();
    draw_passive_hatch_lines(content, rect, pattern, color);
    content.restore_state();
}

pub(in crate::pdf) fn draw_passive_hatch_lines(
    content: &mut Content,
    rect: VectorDrawRect,
    pattern: ShadingPattern,
    color: PdfColor,
) {
    let pattern = pattern.without_foreground_color();
    set_stroke_color(content, color);
    content.set_line_width(0.35);

    let spacing = match pattern {
        ShadingPattern::DarkHorizontal
        | ShadingPattern::DarkVertical
        | ShadingPattern::DarkForwardDiagonal
        | ShadingPattern::DarkBackwardDiagonal
        | ShadingPattern::DarkCross
        | ShadingPattern::DarkDiagonalCross => 2.5,
        _ => 4.0,
    };
    match pattern {
        ShadingPattern::WordWmfBitmapHorizontal => {
            // Word rasterizes the legacy WMF HS_HORIZONTAL brush to a 32-row
            // monochrome tile, then scales and clips it with the vector shape.
            // The recovered reference has four one-row bands at an eight-row
            // pitch. Only one band intersects this bounded rectangle. Preserve
            // that appearance as a vector stroke so no raster/WMF payload is
            // retained in the PDF.
            const BAND_CENTER_FROM_BOTTOM: f32 = 0.320_55;
            const BAND_HEIGHT_FRACTION: f32 = 0.247_05;
            stroke_line(
                content,
                rect.x,
                rect.y + (rect.height * BAND_CENTER_FROM_BOTTOM),
                rect.x + rect.width,
                rect.y + (rect.height * BAND_CENTER_FROM_BOTTOM),
                rect.height * BAND_HEIGHT_FRACTION,
            );
        }
        ShadingPattern::Horizontal | ShadingPattern::DarkHorizontal => {
            draw_passive_horizontal_hatch_lines(content, rect, spacing);
        }
        ShadingPattern::Vertical | ShadingPattern::DarkVertical => {
            draw_passive_vertical_hatch_lines(content, rect, spacing);
        }
        ShadingPattern::ForwardDiagonal | ShadingPattern::DarkForwardDiagonal => {
            draw_passive_forward_diagonal_hatch_lines(content, rect, spacing);
        }
        ShadingPattern::BackwardDiagonal | ShadingPattern::DarkBackwardDiagonal => {
            draw_passive_backward_diagonal_hatch_lines(content, rect, spacing);
        }
        ShadingPattern::Cross | ShadingPattern::DarkCross => {
            draw_passive_horizontal_hatch_lines(content, rect, spacing);
            draw_passive_vertical_hatch_lines(content, rect, spacing);
        }
        ShadingPattern::DiagonalCross | ShadingPattern::DarkDiagonalCross => {
            draw_passive_forward_diagonal_hatch_lines(content, rect, spacing);
            draw_passive_backward_diagonal_hatch_lines(content, rect, spacing);
        }
        ShadingPattern::None | ShadingPattern::VerticalGradient => {}
        _ => {}
    }
}

pub(in crate::pdf) fn draw_passive_horizontal_hatch_lines(
    content: &mut Content,
    rect: VectorDrawRect,
    spacing: f32,
) {
    let mut cursor = rect.y + spacing;
    let max_y = rect.y + rect.height;
    while cursor < max_y {
        stroke_line(content, rect.x, cursor, rect.x + rect.width, cursor, 0.35);
        cursor += spacing;
    }
}

pub(in crate::pdf) fn draw_passive_vertical_hatch_lines(
    content: &mut Content,
    rect: VectorDrawRect,
    spacing: f32,
) {
    let mut cursor = rect.x + spacing;
    let max_x = rect.x + rect.width;
    while cursor < max_x {
        stroke_line(content, cursor, rect.y, cursor, rect.y + rect.height, 0.35);
        cursor += spacing;
    }
}

pub(in crate::pdf) fn draw_passive_forward_diagonal_hatch_lines(
    content: &mut Content,
    rect: VectorDrawRect,
    spacing: f32,
) {
    let mut offset = spacing;
    while offset < rect.width {
        let length = (rect.width - offset).min(rect.height);
        stroke_line(
            content,
            rect.x + offset,
            rect.y,
            rect.x + offset + length,
            rect.y + length,
            0.35,
        );
        offset += spacing;
    }

    let mut offset = spacing;
    while offset < rect.height {
        let length = rect.width.min(rect.height - offset);
        stroke_line(
            content,
            rect.x,
            rect.y + offset,
            rect.x + length,
            rect.y + offset + length,
            0.35,
        );
        offset += spacing;
    }
}

pub(in crate::pdf) fn draw_passive_backward_diagonal_hatch_lines(
    content: &mut Content,
    rect: VectorDrawRect,
    spacing: f32,
) {
    let mut offset = spacing;
    while offset < rect.height {
        let length = rect.width.min(offset);
        stroke_line(
            content,
            rect.x,
            rect.y + offset,
            rect.x + length,
            rect.y + offset - length,
            0.35,
        );
        offset += spacing;
    }

    let mut offset = spacing;
    while offset < rect.width {
        let length = (rect.width - offset).min(rect.height);
        stroke_line(
            content,
            rect.x + offset,
            rect.y + rect.height,
            rect.x + offset + length,
            rect.y + rect.height - length,
            0.35,
        );
        offset += spacing;
    }
}

pub(in crate::pdf) fn draw_passive_vector_rounded_rectangle(
    content: &mut Content,
    rect: VectorDrawRect,
    corner_width: f32,
    corner_height: f32,
    stroke_color: Option<crate::model::Color>,
    stroke_width: f32,
    stroke_style: LineStyle,
    fill_pattern: ShadingPattern,
    fill_color: Option<crate::model::Color>,
) {
    let radius_x = (corner_width * 0.5).max(0.1);
    let radius_y = (corner_height * 0.5).max(0.1);
    let fallback_radius = radius_x.min(radius_y);
    if fill_pattern != ShadingPattern::None
        && let Some(fill_color) = fill_color
    {
        draw_passive_hatch_rounded_rectangle(
            content,
            rect,
            fallback_radius,
            fill_pattern,
            pdf_color_from_model(fill_color),
        );
        if stroke_color.is_none() || stroke_width <= 0.0 {
            return;
        }
        draw_passive_rounded_rectangle(
            content,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            fallback_radius,
            stroke_width,
            stroke_color.map(pdf_color_from_model).unwrap_or(PdfColor {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
            }),
            stroke_style,
            None,
            None,
            ShadingPattern::None,
        );
        return;
    }
    draw_passive_vector_elliptical_rounded_rectangle(
        content,
        rect,
        radius_x,
        radius_y,
        stroke_color.map(|_| stroke_width).unwrap_or(0.0),
        stroke_color.map(pdf_color_from_model).unwrap_or(PdfColor {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
        }),
        stroke_style,
        fill_color.map(pdf_color_from_model),
    );
}

pub(in crate::pdf) fn draw_passive_vector_elliptical_rounded_rectangle(
    content: &mut Content,
    rect: VectorDrawRect,
    radius_x: f32,
    radius_y: f32,
    stroke_width: f32,
    stroke_color: PdfColor,
    stroke_style: LineStyle,
    fill_color: Option<PdfColor>,
) {
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
    append_passive_elliptical_rounded_rectangle_path(
        content,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        radius_x,
        radius_y,
    );
    if fill_color.is_some() && has_stroke {
        content.fill_nonzero_and_stroke();
    } else if fill_color.is_some() {
        content.fill_nonzero();
    } else {
        content.stroke();
    }
    content.restore_state();
}

pub(in crate::pdf) fn draw_passive_vector_ellipse(
    content: &mut Content,
    rect: VectorDrawRect,
    stroke_color: Option<crate::model::Color>,
    stroke_width: f32,
    stroke_style: LineStyle,
    fill_pattern: ShadingPattern,
    fill_color: Option<crate::model::Color>,
) {
    if fill_pattern != ShadingPattern::None
        && let Some(fill_color) = fill_color
    {
        draw_passive_hatch_ellipse(
            content,
            rect,
            fill_pattern,
            pdf_color_from_model(fill_color),
        );
        if stroke_color.is_none() || stroke_width <= 0.0 {
            return;
        }
        draw_passive_ellipse(
            content,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            stroke_width,
            stroke_color.map(pdf_color_from_model).unwrap_or(PdfColor {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
            }),
            stroke_style,
            None,
            None,
            ShadingPattern::None,
        );
        return;
    }
    draw_passive_ellipse(
        content,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        stroke_color.map(|_| stroke_width).unwrap_or(0.0),
        stroke_color.map(pdf_color_from_model).unwrap_or(PdfColor {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
        }),
        stroke_style,
        fill_color.map(pdf_color_from_model),
        None,
        ShadingPattern::None,
    );
}

pub(in crate::pdf) fn draw_passive_vector_text(
    content: &mut Content,
    point: crate::layout::LayoutPoint,
    font_size: f32,
    bold: bool,
    vertical_scale_percent: u8,
    text: &str,
    color: Option<crate::model::Color>,
    background_color: Option<crate::model::Color>,
    clip_rect: Option<VectorDrawRect>,
    character_extra: f32,
    word_extra: f32,
    horizontal_align: StaticImageTextHorizontalAlign,
    vertical_align: StaticImageTextVerticalAlign,
) {
    if text.is_empty() {
        return;
    }
    if let Some(rect) = clip_rect {
        content.save_state();
        content.rect(rect.x, rect.y, rect.width, rect.height);
        content.clip_nonzero();
        content.end_path();
    }
    let mut style = CharacterStyle::default();
    style.bold = bold;
    style.font_size_half_points = (font_size * 2.0).round().clamp(1.0, 144.0) as i32;
    style.character_spacing_twips = (character_extra * 20.0).round().clamp(-1440.0, 5760.0) as i32;
    let metrics = passive_vector_text_metrics(
        point,
        font_size,
        text,
        character_extra,
        word_extra,
        horizontal_align,
        vertical_align,
    );
    if let Some(background_color) = background_color {
        draw_passive_vector_rectangle(
            content,
            VectorDrawRect {
                x: metrics.x,
                y: metrics.bottom_y,
                width: metrics.width,
                height: metrics.top_y - metrics.bottom_y,
            },
            None,
            0.0,
            LineStyle::Solid,
            ShadingPattern::None,
            Some(background_color),
        );
    }
    set_fill_color(
        content,
        color.map(pdf_color_from_model).unwrap_or(PdfColor {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
        }),
    );
    let encoded = encode_pdf_text_for_font(text, PdfFontFamily::Helvetica);
    let vertical_scale = (f32::from(vertical_scale_percent) / 100.0).clamp(0.5, 1.0);
    if vertical_scale < 1.0 {
        content.save_state();
        content.transform([
            1.0,
            0.0,
            0.0,
            vertical_scale,
            0.0,
            metrics.top_y * (1.0 - vertical_scale),
        ]);
    }
    write_text_fragment(
        content,
        text,
        if bold {
            HELVETICA_BOLD
        } else {
            HELVETICA_REGULAR
        },
        Some(PdfFontFamily::Helvetica),
        None,
        &style,
        word_extra,
        metrics.x,
        metrics.baseline_y,
        TextRotation::None,
        &encoded,
        TextRenderingMode::Fill,
    );
    if vertical_scale < 1.0 {
        content.restore_state();
    }
    if clip_rect.is_some() {
        content.restore_state();
    }
}

#[derive(Debug, Copy, Clone)]
pub(in crate::pdf) struct PassiveVectorTextMetrics {
    x: f32,
    baseline_y: f32,
    top_y: f32,
    bottom_y: f32,
    width: f32,
}

pub(in crate::pdf) fn passive_vector_text_metrics(
    point: crate::layout::LayoutPoint,
    font_size: f32,
    text: &str,
    character_extra: f32,
    word_extra: f32,
    horizontal_align: StaticImageTextHorizontalAlign,
    vertical_align: StaticImageTextVerticalAlign,
) -> PassiveVectorTextMetrics {
    let text_width = estimated_passive_vector_text_width(text, font_size)
        + passive_vector_text_extra_width(text, character_extra)
        + passive_vector_text_word_extra_width(text, word_extra);
    let x = match horizontal_align {
        StaticImageTextHorizontalAlign::Left => point.x,
        StaticImageTextHorizontalAlign::Center => point.x - (text_width / 2.0),
        StaticImageTextHorizontalAlign::Right => point.x - text_width,
    };
    let (baseline_y, top_y, bottom_y) = match vertical_align {
        StaticImageTextVerticalAlign::Top => {
            let baseline_y = point.y - font_size;
            (baseline_y, point.y, baseline_y - (font_size * 0.2))
        }
        StaticImageTextVerticalAlign::Baseline => (
            point.y,
            point.y + (font_size * 0.8),
            point.y - (font_size * 0.2),
        ),
        StaticImageTextVerticalAlign::Bottom => {
            let baseline_y = point.y + (font_size * 0.2);
            (baseline_y, baseline_y + (font_size * 0.8), point.y)
        }
    };
    PassiveVectorTextMetrics {
        x,
        baseline_y,
        top_y,
        bottom_y,
        width: text_width.max(font_size * 0.25),
    }
}

pub(in crate::pdf) fn passive_vector_text_extra_width(text: &str, character_extra: f32) -> f32 {
    if character_extra == 0.0 {
        return 0.0;
    }
    let visible_count = text.chars().filter(|ch| !ch.is_control()).count();
    if visible_count <= 1 {
        0.0
    } else {
        character_extra * visible_count.saturating_sub(1) as f32
    }
}

pub(in crate::pdf) fn passive_vector_text_word_extra_width(text: &str, word_extra: f32) -> f32 {
    if word_extra == 0.0 {
        return 0.0;
    }
    word_extra * text.chars().filter(|ch| *ch == ' ').count() as f32
}

pub(in crate::pdf) fn estimated_passive_vector_text_width(text: &str, font_size: f32) -> f32 {
    text.chars()
        .map(|ch| {
            if ch.is_whitespace() {
                0.28
            } else if matches!(ch, 'i' | 'l' | 'I' | '.' | ',' | ':' | ';' | '!' | '|') {
                0.28
            } else if matches!(ch, 'm' | 'w' | 'M' | 'W') {
                0.85
            } else {
                0.55
            }
        })
        .sum::<f32>()
        * font_size
}

pub(in crate::pdf) fn pdf_color_from_model(color: crate::model::Color) -> PdfColor {
    PdfColor {
        red: color.red as f32 / 255.0,
        green: color.green as f32 / 255.0,
        blue: color.blue as f32 / 255.0,
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::pdf) fn draw_passive_line(
    content: &mut Content,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    width: f32,
    color: PdfColor,
    style: LineStyle,
    cap: LineCap,
) {
    content.save_state();
    set_stroke_color(content, color);
    set_passive_line_cap(content, cap);
    match style {
        LineStyle::Solid => stroke_line(content, x1, y1, x2, y2, width),
        LineStyle::Dotted => {
            let dot = width.max(0.5);
            content.set_dash_pattern([dot, dot * 2.0], 0.0);
            stroke_line(content, x1, y1, x2, y2, width);
        }
        LineStyle::Dashed => {
            let dash = (width * 3.0).max(3.0);
            content.set_dash_pattern([dash, width.max(1.0)], 0.0);
            stroke_line(content, x1, y1, x2, y2, width);
        }
        LineStyle::FineDashed => {
            content.set_dash_pattern([0.42, 0.14], 0.0);
            stroke_line(content, x1, y1, x2, y2, width);
        }
        LineStyle::Double => draw_double_line(content, x1, y1, x2, y2, width),
        LineStyle::Triple => draw_triple_line(content, x1, y1, x2, y2, width),
        LineStyle::Wavy => stroke_wave_line(content, x1, y1, x2, y2, width),
        LineStyle::Emboss => draw_relief_line(content, x1, y1, x2, y2, width, color, true),
        LineStyle::Engrave => draw_relief_line(content, x1, y1, x2, y2, width, color, false),
        LineStyle::ThinThick => draw_weighted_double_line(content, x1, y1, x2, y2, width, true),
        LineStyle::ThickThin => draw_weighted_double_line(content, x1, y1, x2, y2, width, false),
        LineStyle::ThinThickThin => draw_weighted_triple_line(content, x1, y1, x2, y2, width),
    }
    content.restore_state();
}

pub(in crate::pdf) fn draw_passive_joined_polyline(
    content: &mut Content,
    points: &[crate::layout::LayoutPoint],
    width: f32,
    color: PdfColor,
    style: LineStyle,
    cap: LineCap,
    join: LineJoin,
) {
    if points.len() < 2 {
        return;
    }
    if !matches!(
        style,
        LineStyle::Solid | LineStyle::Dotted | LineStyle::Dashed | LineStyle::FineDashed
    ) {
        for segment in points.windows(2) {
            draw_passive_line(
                content,
                segment[0].x,
                segment[0].y,
                segment[1].x,
                segment[1].y,
                width,
                color,
                style,
                cap,
            );
        }
        return;
    }

    content.save_state();
    set_stroke_color(content, color);
    content.set_line_width(width.max(0.25));
    set_passive_line_cap(content, cap);
    set_passive_line_join(content, join);
    set_passive_path_stroke_style(content, width, style);
    content.move_to(points[0].x, points[0].y);
    for point in &points[1..] {
        content.line_to(point.x, point.y);
    }
    content.stroke();
    content.restore_state();
}

pub(in crate::pdf) fn set_passive_path_stroke_style(
    content: &mut Content,
    width: f32,
    style: LineStyle,
) {
    match style {
        LineStyle::Solid
        | LineStyle::Double
        | LineStyle::Triple
        | LineStyle::Wavy
        | LineStyle::Emboss
        | LineStyle::Engrave
        | LineStyle::ThinThick
        | LineStyle::ThickThin
        | LineStyle::ThinThickThin => {}
        LineStyle::Dotted => {
            let dot = width.max(0.5);
            content.set_dash_pattern([dot, dot * 2.0], 0.0);
        }
        LineStyle::Dashed => {
            let dash = (width * 3.0).max(3.0);
            content.set_dash_pattern([dash, dash * 0.75], 0.0);
        }
        LineStyle::FineDashed => {
            content.set_dash_pattern([0.42, 0.14], 0.0);
        }
    }
}

pub(in crate::pdf) fn set_passive_line_cap(content: &mut Content, cap: LineCap) {
    match cap {
        LineCap::Flat => {}
        LineCap::Round => {
            content.set_line_cap(LineCapStyle::RoundCap);
        }
        LineCap::Square => {
            content.set_line_cap(LineCapStyle::ProjectingSquareCap);
        }
    }
}

pub(in crate::pdf) fn set_passive_line_join(content: &mut Content, join: LineJoin) {
    match join {
        LineJoin::Miter => {}
        LineJoin::Round => {
            content.set_line_join(LineJoinStyle::RoundJoin);
        }
        LineJoin::Bevel => {
            content.set_line_join(LineJoinStyle::BevelJoin);
        }
    }
}

pub(in crate::pdf) fn set_passive_shape_opacity(
    content: &mut Content,
    fill_percent: u8,
    stroke_percent: u8,
) {
    if fill_percent >= 100 && stroke_percent >= 100 {
        return;
    }
    let name = passive_shape_alpha_name(fill_percent.min(100), stroke_percent.min(100));
    content.set_parameters(Name(&name));
}

pub(in crate::pdf) fn passive_shape_alpha_name(fill_percent: u8, stroke_percent: u8) -> Vec<u8> {
    let mut name = PASSIVE_SHAPE_ALPHA_PREFIX.to_vec();
    push_decimal_percent(&mut name, fill_percent.min(100));
    name.push(b'S');
    push_decimal_percent(&mut name, stroke_percent.min(100));
    name
}

pub(in crate::pdf) fn push_decimal_percent(output: &mut Vec<u8>, percent: u8) {
    let bounded = percent.min(100);
    if bounded >= 100 {
        output.extend_from_slice(b"100");
    } else if bounded >= 10 {
        output.push(b'0' + (bounded / 10));
        output.push(b'0' + (bounded % 10));
    } else {
        output.push(b'0' + bounded);
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::pdf) fn draw_passive_ellipse(
    content: &mut Content,
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
) {
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
    append_passive_ellipse_path(content, x, y, width, height);
    if fill_color.is_some() && has_stroke {
        content.fill_nonzero_and_stroke();
    } else if fill_color.is_some() {
        content.fill_nonzero();
    } else {
        content.stroke();
    }
    content.restore_state();
    if fill_pattern == ShadingPattern::VerticalGradient
        && let Some(fill_color) = fill_color
    {
        draw_passive_gradient_ellipse(
            content,
            VectorDrawRect {
                x,
                y,
                width,
                height,
            },
            fill_color,
            fill_gradient_color,
        );
        if has_stroke {
            draw_passive_ellipse(
                content,
                x,
                y,
                width,
                height,
                stroke_width,
                stroke_color,
                stroke_style,
                None,
                None,
                ShadingPattern::None,
            );
        }
    } else if fill_pattern != ShadingPattern::None
        && let Some(fill_color) = fill_color
    {
        draw_passive_hatch_ellipse(
            content,
            VectorDrawRect {
                x,
                y,
                width,
                height,
            },
            fill_pattern,
            fill_color,
        );
        if has_stroke {
            draw_passive_ellipse(
                content,
                x,
                y,
                width,
                height,
                stroke_width,
                stroke_color,
                stroke_style,
                None,
                None,
                ShadingPattern::None,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::pdf) fn draw_passive_rounded_rectangle(
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
    fill_gradient_color: Option<PdfColor>,
    fill_pattern: ShadingPattern,
) {
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
    append_passive_rounded_rectangle_path(content, x, y, width, height, radius);
    if fill_color.is_some() && has_stroke {
        content.fill_nonzero_and_stroke();
    } else if fill_color.is_some() {
        content.fill_nonzero();
    } else {
        content.stroke();
    }
    content.restore_state();
    if fill_pattern == ShadingPattern::VerticalGradient
        && let Some(fill_color) = fill_color
    {
        draw_passive_gradient_rounded_rectangle(
            content,
            VectorDrawRect {
                x,
                y,
                width,
                height,
            },
            radius,
            fill_color,
            fill_gradient_color,
        );
        if has_stroke {
            draw_passive_rounded_rectangle(
                content,
                x,
                y,
                width,
                height,
                radius,
                stroke_width,
                stroke_color,
                stroke_style,
                None,
                None,
                ShadingPattern::None,
            );
        }
    } else if fill_pattern != ShadingPattern::None
        && let Some(fill_color) = fill_color
    {
        draw_passive_hatch_rounded_rectangle(
            content,
            VectorDrawRect {
                x,
                y,
                width,
                height,
            },
            radius,
            fill_pattern,
            fill_color,
        );
        if has_stroke {
            draw_passive_rounded_rectangle(
                content,
                x,
                y,
                width,
                height,
                radius,
                stroke_width,
                stroke_color,
                stroke_style,
                None,
                None,
                ShadingPattern::None,
            );
        }
    }
}

pub(in crate::pdf) fn append_passive_ellipse_path(
    content: &mut Content,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    const KAPPA: f32 = 0.552_284_8;

    let width = width.max(0.1);
    let height = height.max(0.1);
    let rx = width / 2.0;
    let ry = height / 2.0;
    let cx = x + rx;
    let cy = y + ry;
    let dx = rx * KAPPA;
    let dy = ry * KAPPA;

    content.move_to(cx + rx, cy);
    content.cubic_to(cx + rx, cy + dy, cx + dx, cy + ry, cx, cy + ry);
    content.cubic_to(cx - dx, cy + ry, cx - rx, cy + dy, cx - rx, cy);
    content.cubic_to(cx - rx, cy - dy, cx - dx, cy - ry, cx, cy - ry);
    content.cubic_to(cx + dx, cy - ry, cx + rx, cy - dy, cx + rx, cy);
    content.close_path();
}

pub(in crate::pdf) fn append_passive_rounded_rectangle_path(
    content: &mut Content,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
) {
    append_passive_elliptical_rounded_rectangle_path(content, x, y, width, height, radius, radius);
}

pub(in crate::pdf) fn append_passive_elliptical_rounded_rectangle_path(
    content: &mut Content,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius_x: f32,
    radius_y: f32,
) {
    const KAPPA: f32 = 0.552_284_8;

    let width = width.max(0.1);
    let height = height.max(0.1);
    let radius_x = radius_x.clamp(0.1, width / 2.0);
    let radius_y = radius_y.clamp(0.1, height / 2.0);
    let control_x = radius_x * KAPPA;
    let control_y = radius_y * KAPPA;
    let right = x + width;
    let top = y + height;

    content.move_to(x + radius_x, y);
    content.line_to(right - radius_x, y);
    content.cubic_to(
        right - radius_x + control_x,
        y,
        right,
        y + radius_y - control_y,
        right,
        y + radius_y,
    );
    content.line_to(right, top - radius_y);
    content.cubic_to(
        right,
        top - radius_y + control_y,
        right - radius_x + control_x,
        top,
        right - radius_x,
        top,
    );
    content.line_to(x + radius_x, top);
    content.cubic_to(
        x + radius_x - control_x,
        top,
        x,
        top - radius_y + control_y,
        x,
        top - radius_y,
    );
    content.line_to(x, y + radius_y);
    content.cubic_to(
        x,
        y + radius_y - control_y,
        x + radius_x - control_x,
        y,
        x + radius_x,
        y,
    );
    content.close_path();
}

pub(in crate::pdf) fn append_passive_polygon_path(
    content: &mut Content,
    points: &[crate::layout::LayoutPoint],
) {
    let Some(first) = points.first() else {
        return;
    };
    content.move_to(first.x, first.y);
    for point in points.iter().skip(1) {
        content.line_to(point.x, point.y);
    }
    content.close_path();
}

pub(in crate::pdf) fn append_passive_open_path(
    content: &mut Content,
    points: &[crate::layout::LayoutPoint],
) {
    let Some(first) = points.first() else {
        return;
    };
    content.move_to(first.x, first.y);
    for point in points.iter().skip(1) {
        content.line_to(point.x, point.y);
    }
}

pub(in crate::pdf) fn vector_points_bounds(
    points: &[crate::layout::LayoutPoint],
) -> Option<VectorDrawRect> {
    let first = points.first()?;
    let mut min_x = first.x;
    let mut max_x = first.x;
    let mut min_y = first.y;
    let mut max_y = first.y;
    for point in points.iter().skip(1) {
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
    }
    Some(VectorDrawRect {
        x: min_x,
        y: min_y,
        width: (max_x - min_x).max(0.1),
        height: (max_y - min_y).max(0.1),
    })
}

pub(in crate::pdf) fn vector_path_bounds(
    start: crate::layout::LayoutPoint,
    segments: &[StaticImageVectorPathSegment],
) -> Option<VectorDrawRect> {
    let mut min_x = start.x;
    let mut max_x = start.x;
    let mut min_y = start.y;
    let mut max_y = start.y;
    let mut saw_segment = false;

    for segment in segments {
        match segment {
            StaticImageVectorPathSegment::MoveTo(x, y)
            | StaticImageVectorPathSegment::LineTo(x, y) => {
                min_x = min_x.min(*x);
                max_x = max_x.max(*x);
                min_y = min_y.min(*y);
                max_y = max_y.max(*y);
                saw_segment = true;
            }
            StaticImageVectorPathSegment::CubicTo {
                control1,
                control2,
                end,
            } => {
                for (x, y) in [*control1, *control2, *end] {
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                    min_y = min_y.min(y);
                    max_y = max_y.max(y);
                }
                saw_segment = true;
            }
        }
    }

    if !saw_segment {
        return None;
    }

    Some(VectorDrawRect {
        x: min_x,
        y: min_y,
        width: (max_x - min_x).max(0.1),
        height: (max_y - min_y).max(0.1),
    })
}

pub(in crate::pdf) fn draw_passive_polygon(
    content: &mut Content,
    points: &[crate::layout::LayoutPoint],
    stroke_width: f32,
    stroke_color: PdfColor,
    stroke_style: LineStyle,
    fill_rule: StaticImageVectorFillRule,
    fill_color: Option<PdfColor>,
    fill_gradient_color: Option<PdfColor>,
    fill_pattern: ShadingPattern,
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
        match fill_rule {
            StaticImageVectorFillRule::Alternate => {
                content.fill_even_odd_and_stroke();
            }
            StaticImageVectorFillRule::Winding => {
                content.fill_nonzero_and_stroke();
            }
        }
    } else if fill_color.is_some() {
        match fill_rule {
            StaticImageVectorFillRule::Alternate => {
                content.fill_even_odd();
            }
            StaticImageVectorFillRule::Winding => {
                content.fill_nonzero();
            }
        }
    } else {
        content.stroke();
    }
    content.restore_state();
    if fill_pattern == ShadingPattern::VerticalGradient
        && let Some(fill_color) = fill_color
    {
        draw_passive_gradient_polygon(content, points, fill_rule, fill_color, fill_gradient_color);
        if has_stroke {
            draw_passive_polygon(
                content,
                points,
                stroke_width,
                stroke_color,
                stroke_style,
                fill_rule,
                None,
                None,
                ShadingPattern::None,
            );
        }
    } else if fill_pattern != ShadingPattern::None
        && let Some(fill_color) = fill_color
    {
        draw_passive_hatch_polygon(content, points, fill_rule, fill_pattern, fill_color);
        if has_stroke {
            draw_passive_polygon(
                content,
                points,
                stroke_width,
                stroke_color,
                stroke_style,
                fill_rule,
                None,
                None,
                ShadingPattern::None,
            );
        }
    }
}

pub(in crate::pdf) fn draw_passive_compound_polygon(
    content: &mut Content,
    points: &[crate::layout::LayoutPoint],
    paths: &[Vec<crate::layout::LayoutPoint>],
    stroke_width: f32,
    stroke_color: PdfColor,
    stroke_style: LineStyle,
    fill_rule: StaticImageVectorFillRule,
    fill_color: Option<PdfColor>,
    fill_gradient_color: Option<PdfColor>,
    fill_pattern: ShadingPattern,
    overlay_paths: &[Vec<crate::layout::LayoutPoint>],
) {
    if paths.is_empty() && overlay_paths.is_empty() {
        draw_passive_polygon(
            content,
            points,
            stroke_width,
            stroke_color,
            stroke_style,
            fill_rule,
            fill_color,
            fill_gradient_color,
            fill_pattern,
        );
        return;
    }
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
    if points.len() >= 3 {
        append_passive_polygon_path(content, points);
    }
    for path in paths {
        if path.len() >= 3 {
            append_passive_polygon_path(content, path);
        }
    }
    if fill_color.is_some() && has_stroke {
        match fill_rule {
            StaticImageVectorFillRule::Alternate => content.fill_even_odd_and_stroke(),
            StaticImageVectorFillRule::Winding => content.fill_nonzero_and_stroke(),
        };
    } else if fill_color.is_some() {
        match fill_rule {
            StaticImageVectorFillRule::Alternate => content.fill_even_odd(),
            StaticImageVectorFillRule::Winding => content.fill_nonzero(),
        };
    } else {
        content.stroke();
    }
    if let Some(fill_color) = fill_color {
        for path in overlay_paths {
            if path.len() >= 3 {
                set_fill_color(content, fill_color);
                append_passive_polygon_path(content, path);
                content.fill_nonzero();
            }
        }
    }
    if has_stroke {
        set_stroke_color(content, stroke_color);
        set_passive_path_stroke_style(content, stroke_width, stroke_style);
        for path in overlay_paths {
            if path.len() >= 2 {
                append_passive_open_path(content, path);
                content.stroke();
            }
        }
    }
    content.restore_state();
    if fill_pattern == ShadingPattern::VerticalGradient
        && let Some(fill_color) = fill_color
    {
        draw_passive_gradient_compound_polygon(
            content,
            points,
            paths,
            fill_rule,
            fill_color,
            fill_gradient_color,
        );
        if has_stroke {
            draw_passive_compound_polygon(
                content,
                points,
                paths,
                stroke_width,
                stroke_color,
                stroke_style,
                fill_rule,
                None,
                None,
                ShadingPattern::None,
                overlay_paths,
            );
        }
    } else if fill_pattern != ShadingPattern::None
        && let Some(fill_color) = fill_color
    {
        draw_passive_hatch_compound_polygon(
            content,
            points,
            paths,
            fill_rule,
            fill_pattern,
            fill_color,
        );
        if has_stroke {
            draw_passive_compound_polygon(
                content,
                points,
                paths,
                stroke_width,
                stroke_color,
                stroke_style,
                fill_rule,
                None,
                None,
                ShadingPattern::None,
                overlay_paths,
            );
        }
    }
}
