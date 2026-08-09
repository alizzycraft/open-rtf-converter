//! Static image and shape placement, drawing order, and shape text.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn layout_image(
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

pub(in crate::layout) fn image_layout_frame(
    image: &StaticImage,
    content_width: f32,
) -> (f32, f32, f32, f32) {
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

pub(in crate::layout) fn image_display_size(image: &StaticImage, content_width: f32) -> (f32, f32) {
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

pub(in crate::layout) fn shape_horizontal_origin(
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

pub(in crate::layout) fn shape_vertical_origin(
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

pub(in crate::layout) fn pdf_color_from_model_color(color: crate::model::Color) -> PdfColor {
    PdfColor {
        red: color.red as f32 / 255.0,
        green: color.green as f32 / 255.0,
        blue: color.blue as f32 / 255.0,
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn layout_shape(
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

pub(in crate::layout) fn wrap_static_shape_items(
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

pub(in crate::layout) fn push_shape_shadow_item(
    page: &mut LayoutPage,
    item: LayoutItem,
    shadow_opacity_percent: u8,
) {
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

pub(in crate::layout) fn apply_passive_drawing_z_order(pages: &mut [LayoutPage]) {
    for page in pages {
        move_below_text_drawings_to_background(page);
        sort_drawing_slots_by_z_order(&mut page.items);
        move_above_text_drawings_to_foreground(page);
    }
}

pub(in crate::layout) fn move_below_text_drawings_to_background(page: &mut LayoutPage) {
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

pub(in crate::layout) fn sort_drawing_slots_by_z_order(items: &mut [LayoutItem]) {
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

pub(in crate::layout) fn move_above_text_drawings_to_foreground(page: &mut LayoutPage) {
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
pub(in crate::layout) struct DrawingOrder {
    pub(in crate::layout) z_order: i32,
    pub(in crate::layout) below_text: bool,
}

pub(in crate::layout) fn drawing_order(item: &LayoutItem) -> Option<DrawingOrder> {
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
pub(in crate::layout) fn drawing_z_order(item: &LayoutItem) -> Option<i32> {
    drawing_order(item).map(|order| order.z_order)
}

pub(in crate::layout) fn shape_point_x(
    left: f32,
    width: f32,
    offset_x: f32,
    flip_horizontal: bool,
) -> f32 {
    if flip_horizontal {
        left + width - offset_x
    } else {
        left + offset_x
    }
}

pub(in crate::layout) fn shape_point_y(
    top: f32,
    height: f32,
    offset_y: f32,
    flip_vertical: bool,
) -> f32 {
    if flip_vertical {
        top - height + offset_y
    } else {
        top - offset_y
    }
}

pub(in crate::layout) fn offset_shape_points(
    points: &[LayoutPoint],
    offset_x: f32,
    offset_y: f32,
) -> Vec<LayoutPoint> {
    points
        .iter()
        .map(|point| LayoutPoint {
            x: point.x + offset_x,
            y: point.y - offset_y,
        })
        .collect()
}

pub(in crate::layout) fn offset_shape_paths(
    paths: &[Vec<LayoutPoint>],
    offset_x: f32,
    offset_y: f32,
) -> Vec<Vec<LayoutPoint>> {
    paths
        .iter()
        .map(|path| offset_shape_points(path, offset_x, offset_y))
        .collect()
}

pub(in crate::layout) fn push_shape_line(
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

pub(in crate::layout) fn push_static_shape_arrowhead(
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

pub(in crate::layout) fn layout_shape_text(
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
