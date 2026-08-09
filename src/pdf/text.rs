//! Layout item dispatch and normal, positioned, and supplied-font text emission.

use super::*;

pub(in crate::pdf) fn draw_layout_item(
    content: &mut Content,
    item: &LayoutItem,
    layout: &LayoutDocument,
    supplied_fonts: &[PlannedSuppliedFont],
    page_text_encodings: &[Option<SuppliedTextEncoding>],
    text_encoding_cursor: &mut usize,
    page_images: &[PlannedImage],
    image_ref_cursor: &mut usize,
) {
    let mut item = item;
    loop {
        match item {
            LayoutItem::Highlight {
                x,
                y,
                width,
                height,
                color,
            } => {
                set_fill_color(content, *color);
                content.rect(*x, *y, *width, *height);
                content.fill_nonzero();
                return;
            }
            LayoutItem::Text(fragment) => {
                let supplied_encoding = page_text_encodings
                    .get(*text_encoding_cursor)
                    .and_then(Option::as_ref);
                *text_encoding_cursor = (*text_encoding_cursor).saturating_add(1);
                draw_text_layout_item(content, fragment, layout, supplied_fonts, supplied_encoding);
                return;
            }
            LayoutItem::Underline {
                x,
                y,
                width,
                color,
                style,
            } => {
                draw_underline(content, *x, *y, *width, *color, *style);
                return;
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
                draw_passive_line(
                    content,
                    *x1,
                    *y1,
                    *x2,
                    *y2,
                    *width,
                    *color,
                    *style,
                    LineCap::Flat,
                );
                return;
            }
            LayoutItem::CappedLine {
                x1,
                y1,
                x2,
                y2,
                width,
                color,
                style,
                cap,
            } => {
                draw_passive_line(content, *x1, *y1, *x2, *y2, *width, *color, *style, *cap);
                return;
            }
            LayoutItem::JoinedPolyline {
                points,
                width,
                color,
                style,
                cap,
                join,
            } => {
                draw_passive_joined_polyline(content, points, *width, *color, *style, *cap, *join);
                return;
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
                fill_gradient_color,
                fill_pattern,
            } => {
                draw_passive_ellipse(
                    content,
                    *x,
                    *y,
                    *width,
                    *height,
                    *stroke_width,
                    *stroke_color,
                    *stroke_style,
                    *fill_color,
                    *fill_gradient_color,
                    *fill_pattern,
                );
                return;
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
                fill_gradient_color,
                fill_pattern,
            } => {
                draw_passive_rounded_rectangle(
                    content,
                    *x,
                    *y,
                    *width,
                    *height,
                    *radius,
                    *stroke_width,
                    *stroke_color,
                    *stroke_style,
                    *fill_color,
                    *fill_gradient_color,
                    *fill_pattern,
                );
                return;
            }
            LayoutItem::Polygon {
                points,
                paths,
                overlay_paths,
                fill_rule,
                stroke_width,
                stroke_color,
                stroke_style,
                fill_color,
                fill_gradient_color,
                fill_pattern,
            } => {
                draw_passive_compound_polygon(
                    content,
                    points,
                    paths,
                    *stroke_width,
                    *stroke_color,
                    *stroke_style,
                    *fill_rule,
                    *fill_color,
                    *fill_gradient_color,
                    *fill_pattern,
                    overlay_paths,
                );
                return;
            }
            LayoutItem::Image(fragment) => {
                draw_image_layout_item(content, fragment, page_images, image_ref_cursor);
                return;
            }
            LayoutItem::Drawing(fragment) => {
                item = &fragment.item;
            }
            LayoutItem::Opacity {
                fill_percent,
                stroke_percent,
                item: nested,
            } => {
                content.save_state();
                set_passive_shape_opacity(content, *fill_percent, *stroke_percent);
                draw_layout_item(
                    content,
                    nested,
                    layout,
                    supplied_fonts,
                    page_text_encodings,
                    text_encoding_cursor,
                    page_images,
                    image_ref_cursor,
                );
                content.restore_state();
                return;
            }
            LayoutItem::ClipRect {
                x,
                y,
                width,
                height,
                item: nested,
            } => {
                content.save_state();
                content.rect(*x, *y, *width, *height);
                content.clip_nonzero();
                content.end_path();
                draw_layout_item(
                    content,
                    nested,
                    layout,
                    supplied_fonts,
                    page_text_encodings,
                    text_encoding_cursor,
                    page_images,
                    image_ref_cursor,
                );
                content.restore_state();
                return;
            }
        }
    }
}

pub(in crate::pdf) fn draw_text_layout_item(
    content: &mut Content,
    fragment: &TextFragment,
    layout: &LayoutDocument,
    supplied_fonts: &[PlannedSuppliedFont],
    supplied_encoding: Option<&SuppliedTextEncoding>,
) {
    if text_fragment_renders_as_passive_vector_only(fragment) {
        draw_passive_vector_only_dingbat_text(content, fragment, layout);
        return;
    }
    let visual_text = passive_visual_text_for_fragment(fragment);
    let text = visual_text.as_deref().unwrap_or(&fragment.text);
    let mut base_encoded = Vec::new();
    let font_resource = if let Some(supplied) = supplied_encoding {
        supplied_fonts[supplied.font_index].resource_name.as_slice()
    } else {
        base_encoded = encode_pdf_text_for_font(text, fragment.font_family);
        font_name_for_style(fragment.font_family, &fragment.style).0
    };
    let passive_kerning_family = supplied_encoding.is_none().then_some(fragment.font_family);
    let supplied_positioning_points =
        supplied_encoding.map(|supplied| supplied.positioning_points.as_slice());
    let encoded = supplied_encoding
        .map(|supplied| supplied.encoded.as_slice())
        .unwrap_or(base_encoded.as_slice());
    if fragment.style.shadow {
        set_fill_color(content, shadow_color(fragment.color));
        write_text_fragment(
            content,
            text,
            font_resource,
            passive_kerning_family,
            supplied_positioning_points,
            &fragment.style,
            fragment.word_spacing,
            fragment.x + shadow_offset(&fragment.style),
            fragment.baseline_y - shadow_offset(&fragment.style),
            fragment.rotation,
            encoded,
            TextRenderingMode::Fill,
        );
    }
    let (final_dx, final_dy) = if fragment.style.relief != TextRelief::None {
        let offset = relief_offset(&fragment.style);
        let (first_color, first_dx, first_dy, second_color, second_dx, second_dy) =
            relief_layers(fragment.color, fragment.style.relief, offset);
        set_fill_color(content, first_color);
        write_text_fragment(
            content,
            text,
            font_resource,
            passive_kerning_family,
            supplied_positioning_points,
            &fragment.style,
            fragment.word_spacing,
            fragment.x + first_dx,
            fragment.baseline_y + first_dy,
            fragment.rotation,
            encoded,
            TextRenderingMode::Fill,
        );
        set_fill_color(content, second_color);
        write_text_fragment(
            content,
            text,
            font_resource,
            passive_kerning_family,
            supplied_positioning_points,
            &fragment.style,
            fragment.word_spacing,
            fragment.x + second_dx,
            fragment.baseline_y + second_dy,
            fragment.rotation,
            encoded,
            TextRenderingMode::Fill,
        );
        (-offset, offset)
    } else {
        (0.0, 0.0)
    };
    set_fill_color(content, fragment.color);
    if fragment.style.outline {
        set_stroke_color(content, fragment.color);
        content.set_line_width((fragment.style.font_size_points() * 0.035).clamp(0.25, 1.25));
    }
    write_text_fragment(
        content,
        text,
        font_resource,
        passive_kerning_family,
        supplied_positioning_points,
        &fragment.style,
        fragment.word_spacing,
        fragment.x + final_dx,
        fragment.baseline_y + final_dy,
        fragment.rotation,
        encoded,
        if fragment.style.outline {
            TextRenderingMode::Stroke
        } else {
            TextRenderingMode::Fill
        },
    );
    draw_passive_text_overlays(content, fragment);
}

pub(in crate::pdf) fn draw_underline(
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
            stroke_line(content, x, y, x + width, y, 0.8)
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

pub(in crate::pdf) fn draw_passive_text_overlays(content: &mut Content, fragment: &TextFragment) {
    draw_passive_emphasis_marks(content, fragment);
    draw_passive_checkbox_overlays(content, fragment);
    draw_passive_dingbat_overlays(content, fragment);
}

pub(in crate::pdf) fn text_fragment_renders_as_passive_vector_only(
    fragment: &TextFragment,
) -> bool {
    if fragment.font_family != PdfFontFamily::ZapfDingbats
        || fragment.style.shadow
        || fragment.style.outline
        || fragment.style.relief != TextRelief::None
        || fragment.text.is_empty()
    {
        return false;
    }

    let mut has_passive_content = false;
    for ch in fragment
        .text
        .chars()
        .filter(|ch| !is_zero_width_pdf_char(*ch))
    {
        if ch.is_whitespace() {
            has_passive_content = true;
            continue;
        }
        if is_passive_vector_only_dingbat_char(ch) {
            has_passive_content = true;
        } else {
            return false;
        }
    }
    has_passive_content
}

pub(in crate::pdf) fn is_passive_vector_only_dingbat_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{25a1}'
            | '\u{25c9}'
            | '\u{2610}'
            | '\u{2611}'
            | '\u{2612}'
            | '\u{263a}'
            | '\u{2713}'
            | '\u{2714}'
            | '\u{2717}'
            | '\u{2751}'
            | '\u{1f5f5}'
            | '\u{1f5f7}'
            | '\u{1f4e5}'
    )
}

pub(in crate::pdf) fn draw_passive_vector_only_dingbat_text(
    content: &mut Content,
    fragment: &TextFragment,
    layout: &LayoutDocument,
) {
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
    let source_font = layout
        .fonts
        .iter()
        .find(|font| font.index == fragment.style.font_index);

    content.save_state();
    set_stroke_color(content, fragment.color);
    set_fill_color(content, fragment.color);
    content.set_line_width((font_size * 0.075).clamp(0.5, 1.25));
    for ch in fragment.text.chars() {
        if draw_source_dingbat_outline(
            content,
            source_font.map(|font| font.name.as_str()),
            ch,
            cursor,
            fragment.baseline_y,
            font_size,
        ) {
            // The source-font-specific outline has already been emitted.
        } else {
            match ch {
                '\u{25c9}' => {
                    draw_passive_bullseye(content, cursor, fragment.baseline_y, font_size)
                }
                '\u{25a1}' | '\u{2610}' | '\u{2751}' => {
                    draw_passive_checkbox_box(content, cursor, fragment.baseline_y, font_size)
                }
                '\u{2611}' => {
                    draw_passive_checkbox_box(content, cursor, fragment.baseline_y, font_size);
                    draw_passive_checkbox_tick(content, cursor, fragment.baseline_y, font_size);
                }
                '\u{2612}' => {
                    draw_passive_checkbox_box(content, cursor, fragment.baseline_y, font_size);
                    draw_passive_checkbox_x(content, cursor, fragment.baseline_y, font_size);
                }
                '\u{2713}' | '\u{2714}' => {
                    draw_passive_checkbox_tick(content, cursor, fragment.baseline_y, font_size)
                }
                '\u{2717}' => {
                    draw_passive_checkbox_x(content, cursor, fragment.baseline_y, font_size)
                }
                '\u{263a}' => draw_passive_smiley(content, cursor, fragment.baseline_y, font_size),
                '\u{1f4e5}' => draw_passive_inbox_tray(
                    content,
                    cursor,
                    fragment.baseline_y,
                    font_size,
                    fragment.color,
                ),
                _ => {}
            }
        }
        if !is_zero_width_pdf_char(ch) {
            visible_index += 1;
            let advance =
                source_dingbat_advance_points(source_font, ch, font_size).unwrap_or_else(|| {
                    pdf_base_glyph_advance(ch, fragment.font_family, &fragment.style)
                });
            cursor += advance * horizontal_scale;
            if ch == ' ' || ch == '\u{00a0}' {
                cursor += fragment.word_spacing;
            }
            if visible_index < visible_count {
                cursor += character_spacing;
            }
        }
    }
    content.restore_state();
}

pub(in crate::pdf) fn draw_passive_emphasis_marks(content: &mut Content, fragment: &TextFragment) {
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
            let advance = pdf_base_glyph_advance(ch, fragment.font_family, &fragment.style);
            cursor += advance * horizontal_scale;
            if ch == ' ' || ch == '\u{00a0}' {
                cursor += fragment.word_spacing;
            }
            if visible_index < visible_count {
                cursor += character_spacing;
            }
        }
    }
    content.restore_state();
}

pub(in crate::pdf) fn draw_passive_emphasis_mark(
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

pub(in crate::pdf) fn draw_passive_checkbox_overlays(
    content: &mut Content,
    fragment: &TextFragment,
) {
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
            let advance = pdf_base_glyph_advance(ch, fragment.font_family, &fragment.style);
            cursor += advance * horizontal_scale;
            if ch == ' ' || ch == '\u{00a0}' {
                cursor += fragment.word_spacing;
            }
            if visible_index < visible_count {
                cursor += character_spacing;
            }
        }
    }
    content.restore_state();
}

#[derive(Clone, Copy)]
pub(in crate::pdf) struct TrueTypePoint {
    x: i16,
    y: i16,
    on_curve: bool,
}

const fn tt_on(x: i16, y: i16) -> TrueTypePoint {
    TrueTypePoint {
        x,
        y,
        on_curve: true,
    }
}

const fn tt_off(x: i16, y: i16) -> TrueTypePoint {
    TrueTypePoint {
        x,
        y,
        on_curve: false,
    }
}

const WINGDINGS_CHECK: &[TrueTypePoint] = &[
    tt_on(1500, 1568),
    tt_on(1535, 1518),
    tt_off(1321, 1357),
    tt_off(797, 687),
    tt_on(659, 396),
    tt_on(585, 346),
    tt_off(493, 282),
    tt_on(460, 251),
    tt_off(447, 298),
    tt_on(403, 405),
    tt_on(375, 470),
    tt_off(315, 610),
    tt_off(212, 744),
    tt_on(148, 766),
    tt_off(256, 880),
    tt_on(346, 880),
    tt_off(423, 880),
    tt_on(517, 671),
    tt_on(548, 601),
    tt_off(717, 886),
    tt_off(1247, 1424),
];

const WINGDINGS_X: &[TrueTypePoint] = &[
    tt_on(645, 551),
    tt_on(630, 530),
    tt_off(474, 294),
    tt_on(374, 294),
    tt_off(259, 294),
    tt_on(148, 481),
    tt_off(163, 480),
    tt_on(170, 480),
    tt_off(301, 480),
    tt_on(462, 680),
    tt_on(489, 714),
    tt_on(454, 752),
    tt_off(307, 908),
    tt_on(307, 1017),
    tt_off(307, 1106),
    tt_on(468, 1227),
    tt_off(491, 1072),
    tt_on(605, 925),
    tt_on(635, 887),
    tt_on(659, 918),
    tt_off(826, 1131),
    tt_on(962, 1131),
    tt_off(1068, 1131),
    tt_on(1123, 970),
    tt_off(1107, 972),
    tt_on(1100, 972),
    tt_off(1052, 972),
    tt_off(878, 843),
    tt_on(818, 762),
    tt_on(788, 721),
    tt_on(817, 693),
    tt_off(978, 537),
    tt_on(1153, 537),
    tt_off(1059, 347),
    tt_on(957, 347),
    tt_off(865, 347),
    tt_on(691, 509),
];

const WINGDINGS2_CHECK: &[TrueTypePoint] = &[
    tt_on(1464, 1568),
    tt_on(1494, 1516),
    tt_off(1201, 1309),
    tt_off(701, 675),
    tt_on(590, 371),
    tt_on(546, 342),
    tt_off(489, 305),
    tt_on(448, 268),
    tt_off(441, 304),
    tt_on(409, 384),
    tt_on(386, 441),
    tt_off(311, 626),
    tt_off(208, 773),
    tt_on(148, 779),
    tt_off(229, 853),
    tt_on(289, 853),
    tt_off(372, 853),
    tt_on(471, 630),
    tt_on(507, 550),
    tt_off(684, 867),
    tt_off(1185, 1401),
];

const WINGDINGS2_X: &[TrueTypePoint] = &[
    tt_on(623, 605),
    tt_on(598, 573),
    tt_off(411, 328),
    tt_on(307, 328),
    tt_off(229, 328),
    tt_on(148, 456),
    tt_off(176, 450),
    tt_on(191, 450),
    tt_off(322, 450),
    tt_on(489, 665),
    tt_on(528, 714),
    tt_on(500, 748),
    tt_off(298, 983),
    tt_on(298, 1078),
    tt_off(298, 1146),
    tt_on(408, 1251),
    tt_off(454, 1045),
    tt_on(600, 862),
    tt_on(622, 834),
    tt_on(664, 885),
    tt_off(870, 1140),
    tt_on(992, 1140),
    tt_off(1052, 1140),
    tt_on(1091, 1081),
    tt_off(1098, 1071),
    tt_on(1111, 1053),
    tt_off(956, 983),
    tt_on(764, 770),
    tt_on(723, 724),
    tt_on(753, 691),
    tt_off(935, 491),
    tt_on(1089, 491),
    tt_off(1134, 491),
    tt_on(1173, 501),
    tt_off(1123, 397),
    tt_off(1056, 333),
    tt_on(998, 333),
    tt_off(872, 333),
    tt_on(670, 554),
];

const WEBDINGS_INBOX_PAPER: &[TrueTypePoint] = &[
    tt_on(1905, 969),
    tt_off(1832, 939),
    tt_on(1764, 908),
    tt_off(1601, 834),
    tt_on(1491, 767),
    tt_off(1388, 703),
    tt_on(1265, 563),
    tt_on(1229, 569),
    tt_off(1266, 613),
    tt_on(1305, 651),
    tt_off(1396, 742),
    tt_on(1475, 789),
    tt_off(1628, 881),
    tt_on(1860, 977),
    tt_off(1789, 1014),
    tt_on(1728, 1050),
    tt_off(1580, 1137),
    tt_on(1503, 1205),
    tt_off(1408, 1290),
    tt_on(1273, 1472),
    tt_off(1207, 1437),
    tt_on(1146, 1403),
    tt_off(1002, 1319),
    tt_on(911, 1247),
    tt_off(785, 1148),
    tt_on(666, 1000),
    tt_off(711, 976),
    tt_on(752, 949),
    tt_off(852, 886),
    tt_on(920, 825),
    tt_off(998, 754),
    tt_on(1095, 595),
    tt_on(1039, 603),
    tt_off(1017, 644),
    tt_on(993, 680),
    tt_off(937, 767),
    tt_on(882, 816),
    tt_off(801, 888),
    tt_on(621, 993),
    tt_off(658, 1039),
    tt_on(698, 1083),
    tt_off(793, 1187),
    tt_on(884, 1259),
    tt_off(1030, 1374),
    tt_on(1274, 1500),
    tt_off(1316, 1443),
    tt_on(1356, 1394),
    tt_off(1452, 1275),
    tt_on(1521, 1213),
    tt_off(1644, 1104),
];

const WEBDINGS_INBOX_PAPER_CUTOUT: &[TrueTypePoint] = &[
    tt_on(1728, 978),
    tt_off(1446, 1024),
    tt_on(1263, 1025),
    tt_off(1266, 829),
    tt_on(1191, 662),
    tt_on(1098, 789),
    tt_off(1121, 824),
    tt_on(1138, 860),
    tt_off(1179, 946),
    tt_on(1185, 1022),
    tt_off(1052, 1013),
    tt_on(852, 976),
    tt_on(768, 1027),
    tt_off(891, 1049),
    tt_on(1186, 1079),
    tt_off(1192, 1264),
    tt_on(1257, 1378),
    tt_on(1303, 1321),
    tt_off(1260, 1244),
    tt_on(1261, 1080),
    tt_off(1405, 1078),
    tt_on(1615, 1053),
];

const WEBDINGS_INBOX_TRAY: &[TrueTypePoint] = &[
    tt_on(1810, 51),
    tt_on(1017, -274),
    tt_on(171, 51),
    tt_on(171, 634),
    tt_on(885, 777),
    tt_on(913, 773),
    tt_off(944, 737),
    tt_on(1002, 651),
    tt_on(661, 711),
    tt_on(560, 688),
    tt_on(1278, 551),
    tt_on(1410, 579),
    tt_on(1311, 597),
    tt_off(1339, 641),
    tt_on(1411, 695),
    tt_on(1810, 634),
];

const SEGOE_SYMBOL_CHECK: &[TrueTypePoint] = &[
    tt_on(147, 489),
    tt_off(124, 512),
    tt_off(94, 549),
    tt_on(94, 563),
    tt_off(94, 578),
    tt_off(121, 601),
    tt_off(159, 616),
    tt_off(202, 623),
    tt_on(217, 623),
    tt_off(233, 623),
    tt_off(254, 608),
    tt_on(268, 594),
    tt_off(362, 500),
    tt_off(515, 284),
    tt_on(580, 178),
    tt_off(705, 524),
    tt_off(1037, 1174),
    tt_on(1245, 1458),
    tt_off(1265, 1486),
    tt_off(1350, 1516),
    tt_on(1403, 1516),
    tt_off(1420, 1516),
    tt_off(1440, 1503),
    tt_on(1440, 1491),
    tt_off(1440, 1479),
    tt_off(1434, 1462),
    tt_on(1423, 1448),
    tt_off(1312, 1301),
    tt_off(1103, 979),
    tt_off(913, 622),
    tt_off(743, 230),
    tt_on(672, 14),
    tt_off(670, 8),
    tt_off(652, -1),
    tt_off(626, -7),
    tt_off(594, -10),
    tt_on(578, -10),
    tt_off(555, -10),
    tt_off(517, -1),
    tt_on(508, 14),
    tt_off(460, 96),
    tt_off(378, 221),
    tt_off(294, 329),
    tt_off(203, 431),
];

const SEGOE_SYMBOL_X: &[TrueTypePoint] = &[
    tt_on(541, 1415),
    tt_on(555, 1384),
    tt_off(620, 1244),
    tt_off(749, 987),
    tt_on(817, 860),
    tt_off(929, 1022),
    tt_off(1165, 1332),
    tt_on(1292, 1481),
    tt_off(1306, 1496),
    tt_off(1328, 1512),
    tt_on(1339, 1516),
    tt_on(1341, 1493),
    tt_on(1376, 1516),
    tt_on(1360, 1477),
    tt_on(1411, 1501),
    tt_on(1399, 1470),
    tt_on(1432, 1475),
    tt_on(1401, 1434),
    tt_on(1442, 1452),
    tt_off(1437, 1441),
    tt_off(1428, 1425),
    tt_on(1421, 1417),
    tt_off(1281, 1252),
    tt_off(1021, 905),
    tt_on(897, 715),
    tt_off(975, 576),
    tt_off(1150, 284),
    tt_on(1251, 121),
    tt_off(1258, 110),
    tt_off(1270, 85),
    tt_on(1278, 68),
    tt_on(1251, 70),
    tt_on(1257, 35),
    tt_on(1214, 61),
    tt_on(1231, 10),
    tt_on(1194, 27),
    tt_on(1192, 0),
    tt_on(1157, 43),
    tt_on(1163, 8),
    tt_on(1141, 33),
    tt_on(1141, 23),
    tt_off(1041, 171),
    tt_off(876, 436),
    tt_on(803, 565),
    tt_off(694, 391),
    tt_off(493, 19),
    tt_on(397, -182),
    tt_on(381, -211),
    tt_on(367, -195),
    tt_on(342, -227),
    tt_on(342, -178),
    tt_on(305, -227),
    tt_on(305, -188),
    tt_on(270, -213),
    tt_on(287, -152),
    tt_on(248, -184),
    tt_on(254, -147),
    tt_on(238, -158),
    tt_off(241, -147),
    tt_off(245, -132),
    tt_on(250, -123),
    tt_off(349, 86),
    tt_off(585, 505),
    tt_on(721, 713),
    tt_off(688, 773),
    tt_off(610, 923),
    tt_off(530, 1086),
    tt_off(453, 1250),
    tt_on(422, 1323),
    tt_on(403, 1366),
    tt_on(432, 1346),
    tt_on(420, 1397),
    tt_on(446, 1380),
    tt_on(440, 1419),
    tt_on(481, 1384),
    tt_on(465, 1432),
    tt_on(492, 1413),
    tt_on(492, 1434),
    tt_on(516, 1403),
    tt_on(512, 1430),
    tt_on(541, 1399),
];

pub(in crate::pdf) fn draw_source_dingbat_outline(
    content: &mut Content,
    source_font_name: Option<&str>,
    ch: char,
    glyph_x: f32,
    baseline_y: f32,
    font_size: f32,
) -> bool {
    let Some(source_font_name) = source_font_name else {
        return false;
    };
    let name = source_font_name.to_ascii_lowercase();
    if name.contains("segoe ui symbol") {
        match ch {
            '\u{25a1}' | '\u{2610}' => {
                draw_segoe_symbol_box(content, glyph_x, baseline_y, font_size)
            }
            '\u{2611}' => {
                draw_segoe_symbol_box(content, glyph_x, baseline_y, font_size);
                draw_true_type_polygon(
                    content,
                    glyph_x,
                    baseline_y,
                    font_size,
                    &[
                        (695, 226),
                        (371, 700),
                        (623, 700),
                        (767, 472),
                        (1148, 1215),
                        (1376, 1215),
                        (870, 226),
                    ],
                );
            }
            '\u{2612}' => {
                draw_segoe_symbol_box(content, glyph_x, baseline_y, font_size);
                draw_true_type_polygon(
                    content,
                    glyph_x,
                    baseline_y,
                    font_size,
                    &[
                        (976, 717),
                        (1404, 289),
                        (1310, 195),
                        (882, 623),
                        (454, 195),
                        (360, 289),
                        (788, 717),
                        (360, 1146),
                        (454, 1240),
                        (882, 811),
                        (1310, 1240),
                        (1404, 1146),
                    ],
                );
            }
            '\u{2713}' | '\u{2714}' => draw_true_type_contour(
                content,
                glyph_x,
                baseline_y,
                font_size,
                0,
                SEGOE_SYMBOL_CHECK,
            ),
            '\u{2717}' => {
                draw_true_type_contour(content, glyph_x, baseline_y, font_size, 0, SEGOE_SYMBOL_X)
            }
            _ => return false,
        }
        return true;
    }
    if name.contains("webdings") {
        match ch {
            '\u{1f4e5}' => {
                add_true_type_contour(
                    content,
                    glyph_x,
                    baseline_y,
                    font_size,
                    0,
                    WEBDINGS_INBOX_PAPER,
                );
                add_true_type_contour(
                    content,
                    glyph_x,
                    baseline_y,
                    font_size,
                    0,
                    WEBDINGS_INBOX_PAPER_CUTOUT,
                );
                add_true_type_contour(
                    content,
                    glyph_x,
                    baseline_y,
                    font_size,
                    0,
                    WEBDINGS_INBOX_TRAY,
                );
                content.fill_nonzero();
            }
            '\u{2713}' | '\u{2714}' => draw_true_type_polygon(
                content,
                glyph_x,
                baseline_y,
                font_size,
                &[
                    (1408, 640),
                    (832, 64),
                    (512, 384),
                    (512, 768),
                    (832, 449),
                    (1408, 1024),
                ],
            ),
            '\u{25a1}' | '\u{2610}' => {
                let scale = font_size / 2048.0;
                content.rect(glyph_x, baseline_y - 410.0 * scale, font_size, font_size);
                content.rect(
                    glyph_x + 128.0 * scale,
                    baseline_y - 282.0 * scale,
                    font_size - 256.0 * scale,
                    font_size - 256.0 * scale,
                );
                content.fill_even_odd();
            }
            _ => return false,
        }
        return true;
    }
    if name.contains("wingdings 2") || name.contains("wingdings2") {
        match ch {
            '\u{2717}' => {
                draw_true_type_contour(content, glyph_x, baseline_y, font_size, 0, WINGDINGS2_X)
            }
            '\u{2713}' | '\u{2714}' => {
                draw_true_type_contour(content, glyph_x, baseline_y, font_size, 0, WINGDINGS2_CHECK)
            }
            '\u{2612}' => {
                draw_true_type_box(content, glyph_x, baseline_y, font_size, 99);
                draw_true_type_contour(content, glyph_x, baseline_y, font_size, 253, WINGDINGS2_X);
            }
            '\u{2611}' => {
                draw_true_type_box(content, glyph_x, baseline_y, font_size, 98);
                draw_true_type_contour(
                    content,
                    glyph_x,
                    baseline_y,
                    font_size,
                    274,
                    WINGDINGS2_CHECK,
                );
            }
            '\u{1f5f5}' => {
                draw_true_type_box(content, glyph_x, baseline_y, font_size, 99);
                draw_true_type_polygon(
                    content,
                    glyph_x,
                    baseline_y,
                    font_size,
                    &[
                        (913, 810),
                        (1297, 1194),
                        (1367, 1124),
                        (983, 740),
                        (1367, 356),
                        (1297, 287),
                        (913, 670),
                        (529, 287),
                        (460, 356),
                        (843, 740),
                        (460, 1124),
                        (529, 1194),
                    ],
                );
            }
            '\u{1f5f7}' => {
                draw_true_type_box(content, glyph_x, baseline_y, font_size, 148);
                draw_true_type_polygon(
                    content,
                    glyph_x,
                    baseline_y,
                    font_size,
                    &[
                        (913, 845),
                        (1279, 1211),
                        (1384, 1106),
                        (1018, 740),
                        (1384, 374),
                        (1279, 269),
                        (913, 635),
                        (547, 269),
                        (442, 374),
                        (808, 740),
                        (442, 1106),
                        (547, 1211),
                    ],
                );
            }
            _ => return false,
        }
        return true;
    }
    if name.contains("wingdings") {
        match ch {
            '\u{25c9}' => draw_true_type_bullseye(content, glyph_x, baseline_y, font_size),
            '\u{2713}' | '\u{2714}' => {
                draw_true_type_contour(content, glyph_x, baseline_y, font_size, 0, WINGDINGS_CHECK)
            }
            '\u{2717}' => {
                draw_true_type_contour(content, glyph_x, baseline_y, font_size, 0, WINGDINGS_X)
            }
            '\u{2611}' => {
                draw_true_type_box(content, glyph_x, baseline_y, font_size, 148);
                draw_true_type_contour(
                    content,
                    glyph_x,
                    baseline_y,
                    font_size,
                    243,
                    WINGDINGS_CHECK,
                );
            }
            '\u{2612}' => {
                draw_true_type_box(content, glyph_x, baseline_y, font_size, 148);
                draw_true_type_contour(content, glyph_x, baseline_y, font_size, 262, WINGDINGS_X);
            }
            _ => return false,
        }
        return true;
    }
    false
}

pub(in crate::pdf) fn draw_segoe_symbol_box(
    content: &mut Content,
    glyph_x: f32,
    baseline_y: f32,
    font_size: f32,
) {
    let scale = font_size / 2048.0;
    content.rect(
        glyph_x + 165.0 * scale,
        baseline_y,
        1434.0 * scale,
        1434.0 * scale,
    );
    content.rect(
        glyph_x + 289.0 * scale,
        baseline_y + 124.0 * scale,
        1186.0 * scale,
        1186.0 * scale,
    );
    content.fill_even_odd();
}

pub(in crate::pdf) fn draw_true_type_bullseye(
    content: &mut Content,
    glyph_x: f32,
    baseline_y: f32,
    font_size: f32,
) {
    let scale = font_size / 2048.0;
    let center_x = glyph_x + 913.0 * scale;
    let center_y = baseline_y + 740.0 * scale;
    add_passive_circle_path(content, center_x, center_y, 740.0 * scale);
    add_passive_circle_path(content, center_x, center_y, 296.0 * scale);
    content.fill_even_odd();
}

pub(in crate::pdf) fn draw_true_type_box(
    content: &mut Content,
    glyph_x: f32,
    baseline_y: f32,
    font_size: f32,
    inset_units: i16,
) {
    let scale = font_size / 2048.0;
    let outer_left = glyph_x + 173.0 * scale;
    let outer_bottom = baseline_y;
    let outer_size = 1480.0 * scale;
    let inset = f32::from(inset_units) * scale;
    content.rect(outer_left, outer_bottom, outer_size, outer_size);
    content.rect(
        outer_left + inset,
        outer_bottom + inset,
        outer_size - 2.0 * inset,
        outer_size - 2.0 * inset,
    );
    content.fill_even_odd();
}

pub(in crate::pdf) fn draw_true_type_polygon(
    content: &mut Content,
    glyph_x: f32,
    baseline_y: f32,
    font_size: f32,
    points: &[(i16, i16)],
) {
    let Some(&(first_x, first_y)) = points.first() else {
        return;
    };
    let scale = font_size / 2048.0;
    content.move_to(
        glyph_x + f32::from(first_x) * scale,
        baseline_y + f32::from(first_y) * scale,
    );
    for &(x, y) in &points[1..] {
        content.line_to(
            glyph_x + f32::from(x) * scale,
            baseline_y + f32::from(y) * scale,
        );
    }
    content.close_path();
    content.fill_nonzero();
}

pub(in crate::pdf) fn draw_true_type_contour(
    content: &mut Content,
    glyph_x: f32,
    baseline_y: f32,
    font_size: f32,
    x_offset_units: i16,
    source: &[TrueTypePoint],
) {
    add_true_type_contour(
        content,
        glyph_x,
        baseline_y,
        font_size,
        x_offset_units,
        source,
    );
    content.fill_nonzero();
}

pub(in crate::pdf) fn add_true_type_contour(
    content: &mut Content,
    glyph_x: f32,
    baseline_y: f32,
    font_size: f32,
    x_offset_units: i16,
    source: &[TrueTypePoint],
) {
    if source.is_empty() {
        return;
    }
    let mut points = Vec::with_capacity(source.len() * 2);
    for (index, point) in source.iter().copied().enumerate() {
        points.push(point);
        let next = source[(index + 1) % source.len()];
        if !point.on_curve && !next.on_curve {
            points.push(tt_on(
                ((i32::from(point.x) + i32::from(next.x)) / 2) as i16,
                ((i32::from(point.y) + i32::from(next.y)) / 2) as i16,
            ));
        }
    }
    let Some(start) = points.iter().position(|point| point.on_curve) else {
        return;
    };
    let scale = font_size / 2048.0;
    let map = |point: TrueTypePoint| {
        (
            glyph_x + f32::from(point.x + x_offset_units) * scale,
            baseline_y + f32::from(point.y) * scale,
        )
    };
    let (start_x, start_y) = map(points[start]);
    content.move_to(start_x, start_y);
    let mut current = points[start];
    let mut step = 1usize;
    while step <= points.len() {
        let point = points[(start + step) % points.len()];
        if point.on_curve {
            let (x, y) = map(point);
            content.line_to(x, y);
            current = point;
            step += 1;
        } else {
            let end = points[(start + step + 1) % points.len()];
            let (current_x, current_y) = map(current);
            let (control_x, control_y) = map(point);
            let (end_x, end_y) = map(end);
            content.cubic_to(
                current_x + (control_x - current_x) * (2.0 / 3.0),
                current_y + (control_y - current_y) * (2.0 / 3.0),
                end_x + (control_x - end_x) * (2.0 / 3.0),
                end_y + (control_y - end_y) * (2.0 / 3.0),
                end_x,
                end_y,
            );
            current = end;
            step += 2;
        }
    }
    content.close_path();
}

pub(in crate::pdf) fn draw_passive_checkbox_tick(
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

pub(in crate::pdf) fn draw_passive_checkbox_box(
    content: &mut Content,
    glyph_x: f32,
    baseline_y: f32,
    font_size: f32,
) {
    let box_size = font_size * 0.62;
    let left = glyph_x + font_size * 0.04;
    let bottom = baseline_y + font_size * 0.04;

    content.rect(left, bottom, box_size, box_size);
    content.stroke();
}

pub(in crate::pdf) fn draw_passive_checkbox_x(
    content: &mut Content,
    glyph_x: f32,
    baseline_y: f32,
    font_size: f32,
) {
    let box_size = font_size * 0.62;
    let left = glyph_x + font_size * 0.04;
    let bottom = baseline_y + font_size * 0.04;

    content.move_to(left + box_size * 0.24, bottom + box_size * 0.24);
    content.line_to(left + box_size * 0.76, bottom + box_size * 0.76);
    content.move_to(left + box_size * 0.76, bottom + box_size * 0.24);
    content.line_to(left + box_size * 0.24, bottom + box_size * 0.76);
    content.stroke();
}

pub(in crate::pdf) fn draw_passive_bullseye(
    content: &mut Content,
    glyph_x: f32,
    baseline_y: f32,
    font_size: f32,
) {
    let center_x = glyph_x + font_size * 0.445;
    let center_y = baseline_y + font_size * 0.35;
    add_passive_circle_path(content, center_x, center_y, font_size * 0.445);
    add_passive_circle_path(content, center_x, center_y, font_size * 0.17);
    content.fill_even_odd();
}

pub(in crate::pdf) fn draw_passive_inbox_tray(
    content: &mut Content,
    glyph_x: f32,
    baseline_y: f32,
    font_size: f32,
    color: PdfColor,
) {
    let left = glyph_x + font_size * 0.05;
    let bottom = baseline_y;
    let width = font_size * 0.8;
    let tray_top = bottom + font_size * 0.28;

    content.move_to(left, tray_top);
    content.line_to(left + width * 0.5, tray_top - font_size * 0.13);
    content.line_to(left + width, tray_top);
    content.line_to(left + width * 0.92, bottom);
    content.line_to(left + width * 0.5, bottom - font_size * 0.1);
    content.line_to(left + width * 0.08, bottom);
    content.close_path();
    content.fill_nonzero();

    let center_x = left + width * 0.5;
    let paper_top = bottom + font_size * 0.9;
    let paper_left = left + width * 0.14;
    let paper_right = left + width * 0.86;
    let paper_mid_y = bottom + font_size * 0.54;
    let paper_bottom = bottom + font_size * 0.35;
    content.save_state();
    set_fill_color(content, color);
    set_stroke_color(content, color);
    content.set_line_width((font_size * 0.035).clamp(0.3, 0.65));
    content.move_to(center_x, paper_top);
    content.line_to(paper_right, paper_mid_y);
    content.line_to(center_x, paper_bottom);
    content.line_to(paper_left, paper_mid_y);
    content.close_path();
    content.stroke();
    content.move_to(paper_left, paper_mid_y);
    content.line_to(center_x, paper_mid_y + font_size * 0.08);
    content.line_to(paper_right, paper_mid_y);
    content.move_to(center_x, paper_top);
    content.line_to(center_x, paper_bottom);
    content.stroke();
    content.restore_state();
}

pub(in crate::pdf) fn draw_passive_dingbat_overlays(
    content: &mut Content,
    fragment: &TextFragment,
) {
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
            let advance = pdf_base_glyph_advance(ch, fragment.font_family, &fragment.style);
            cursor += advance * horizontal_scale;
            if ch == ' ' || ch == '\u{00a0}' {
                cursor += fragment.word_spacing;
            }
            if visible_index < visible_count {
                cursor += character_spacing;
            }
        }
    }
    content.restore_state();
}

pub(in crate::pdf) fn draw_passive_smiley(
    content: &mut Content,
    glyph_x: f32,
    baseline_y: f32,
    font_size: f32,
) {
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

pub(in crate::pdf) fn draw_passive_circle(content: &mut Content, cx: f32, cy: f32, radius: f32) {
    add_passive_circle_path(content, cx, cy, radius);
    content.stroke();
}

pub(in crate::pdf) fn draw_passive_filled_circle(
    content: &mut Content,
    cx: f32,
    cy: f32,
    radius: f32,
) {
    add_passive_circle_path(content, cx, cy, radius);
    content.fill_nonzero();
}

pub(in crate::pdf) fn add_passive_circle_path(
    content: &mut Content,
    cx: f32,
    cy: f32,
    radius: f32,
) {
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

pub(in crate::pdf) fn pdf_base_glyph_advance(
    ch: char,
    family: PdfFontFamily,
    style: &CharacterStyle,
) -> f32 {
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

pub(in crate::pdf) fn is_zero_width_pdf_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{00ad}' | '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{200e}' | '\u{200f}' | '\u{feff}'
    )
}

pub(in crate::pdf) fn draw_double_line(
    content: &mut Content,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    width: f32,
) {
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

pub(in crate::pdf) fn draw_triple_line(
    content: &mut Content,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    width: f32,
) {
    let stroke_width = (width * 0.22).clamp(0.25, width.max(0.25));
    let offset = (width * 0.55).max(1.0);
    if (y1 - y2).abs() < 0.01 {
        stroke_line(content, x1, y1 + offset, x2, y2 + offset, stroke_width);
        stroke_line(content, x1, y1, x2, y2, stroke_width);
        stroke_line(content, x1, y1 - offset, x2, y2 - offset, stroke_width);
    } else if (x1 - x2).abs() < 0.01 {
        stroke_line(content, x1 + offset, y1, x2 + offset, y2, stroke_width);
        stroke_line(content, x1, y1, x2, y2, stroke_width);
        stroke_line(content, x1 - offset, y1, x2 - offset, y2, stroke_width);
    } else {
        stroke_line(content, x1, y1, x2, y2, width);
    }
}

pub(in crate::pdf) fn draw_weighted_double_line(
    content: &mut Content,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    width: f32,
    thin_first: bool,
) {
    let thin_width = (width * 0.24).clamp(0.25, width.max(0.25));
    let thick_width = (width * 0.52).clamp(0.35, width.max(0.35));
    let offset = (width * 0.55).max(1.0);
    let (first_width, second_width) = if thin_first {
        (thin_width, thick_width)
    } else {
        (thick_width, thin_width)
    };
    if (y1 - y2).abs() < 0.01 {
        stroke_line(content, x1, y1 + offset, x2, y2 + offset, first_width);
        stroke_line(content, x1, y1 - offset, x2, y2 - offset, second_width);
    } else if (x1 - x2).abs() < 0.01 {
        stroke_line(content, x1 + offset, y1, x2 + offset, y2, first_width);
        stroke_line(content, x1 - offset, y1, x2 - offset, y2, second_width);
    } else {
        stroke_line(content, x1, y1, x2, y2, width);
    }
}

pub(in crate::pdf) fn draw_weighted_triple_line(
    content: &mut Content,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    width: f32,
) {
    let thin_width = (width * 0.18).clamp(0.25, width.max(0.25));
    let thick_width = (width * 0.38).clamp(0.35, width.max(0.35));
    let offset = (width * 0.62).max(1.0);
    if (y1 - y2).abs() < 0.01 {
        stroke_line(content, x1, y1 + offset, x2, y2 + offset, thin_width);
        stroke_line(content, x1, y1, x2, y2, thick_width);
        stroke_line(content, x1, y1 - offset, x2, y2 - offset, thin_width);
    } else if (x1 - x2).abs() < 0.01 {
        stroke_line(content, x1 + offset, y1, x2 + offset, y2, thin_width);
        stroke_line(content, x1, y1, x2, y2, thick_width);
        stroke_line(content, x1 - offset, y1, x2 - offset, y2, thin_width);
    } else {
        stroke_line(content, x1, y1, x2, y2, width);
    }
}

pub(in crate::pdf) fn draw_relief_line(
    content: &mut Content,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    width: f32,
    color: PdfColor,
    raised: bool,
) {
    let stroke_width = (width * 0.45).clamp(0.25, width.max(0.25));
    let offset = (width * 0.35).max(0.75);
    let light = lighten_color(color);
    let dark = shadow_color(color);
    let (first_color, second_color) = if raised { (light, dark) } else { (dark, light) };
    if (y1 - y2).abs() < 0.01 {
        set_stroke_color(content, first_color);
        stroke_line(content, x1, y1 + offset, x2, y2 + offset, stroke_width);
        set_stroke_color(content, second_color);
        stroke_line(content, x1, y1 - offset, x2, y2 - offset, stroke_width);
    } else if (x1 - x2).abs() < 0.01 {
        set_stroke_color(content, first_color);
        stroke_line(content, x1 - offset, y1, x2 - offset, y2, stroke_width);
        set_stroke_color(content, second_color);
        stroke_line(content, x1 + offset, y1, x2 + offset, y2, stroke_width);
    } else {
        stroke_line(content, x1, y1, x2, y2, width);
    }
}

pub(in crate::pdf) fn stroke_line(
    content: &mut Content,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    width: f32,
) {
    content.set_line_width(width);
    content.move_to(x1, y1);
    content.line_to(x2, y2);
    content.stroke();
}

pub(in crate::pdf) fn stroke_wave_underline(content: &mut Content, x: f32, y: f32, width: f32) {
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

pub(in crate::pdf) fn stroke_wave_line(
    content: &mut Content,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    width: f32,
) {
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

pub(in crate::pdf) fn encode_pdf_text(text: &str) -> Vec<u8> {
    text.chars().map(encode_win_ansi_char).collect()
}

pub(in crate::pdf) fn encode_pdf_text_for_font(text: &str, family: PdfFontFamily) -> Vec<u8> {
    match family {
        PdfFontFamily::Symbol => text.chars().map(encode_symbol_char).collect(),
        PdfFontFamily::ZapfDingbats => text.chars().map(encode_zapf_dingbats_char).collect(),
        PdfFontFamily::Helvetica | PdfFontFamily::Courier | PdfFontFamily::Times => {
            encode_pdf_text(text)
        }
    }
}

pub(in crate::pdf) fn encode_symbol_char(ch: char) -> u8 {
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

pub(in crate::pdf) fn encode_zapf_dingbats_char(ch: char) -> u8 {
    match ch {
        '\u{25a1}' | '\u{25c9}' | '\u{2610}' | '\u{2611}' | '\u{2612}' | '\u{2751}'
        | '\u{1f5f5}' | '\u{1f5f7}' | '\u{1f4e5}' => b'q',
        '\u{263a}' => b'J',
        '\u{2713}' | '\u{2714}' => b'3',
        '\u{2717}' => b'7',
        ' ' => b' ',
        ch if ch.is_ascii() => ch as u8,
        _ => b'?',
    }
}

pub(in crate::pdf) fn encode_win_ansi_char(ch: char) -> u8 {
    match ch {
        '\u{0100}' => 0xc2,
        '\u{0101}' => 0xe2,
        '\u{0104}' => 0xc0,
        '\u{0105}' => 0xe0,
        '\u{0106}' => 0xc3,
        '\u{0107}' => 0xe3,
        '\u{010c}' => 0xc8,
        '\u{010d}' => 0xe8,
        '\u{0112}' => 0xc7,
        '\u{0113}' => 0xe7,
        '\u{0116}' => 0xcb,
        '\u{0117}' => 0xeb,
        '\u{0118}' => 0xc6,
        '\u{0119}' => 0xe6,
        '\u{0150}' => 0x81,
        '\u{0151}' => 0x8d,
        '\u{0170}' => 0x8f,
        '\u{0171}' => 0x90,
        '\u{011e}' => 0xd0,
        '\u{0130}' => 0xdd,
        '\u{015e}' => 0xde,
        '\u{011f}' => 0xf0,
        '\u{0131}' => 0xfd,
        '\u{015f}' => 0xfe,
        '\u{0122}' => 0xcc,
        '\u{0123}' => 0xec,
        '\u{012a}' => 0xce,
        '\u{012b}' => 0xee,
        '\u{012e}' => 0xc1,
        '\u{012f}' => 0xe1,
        '\u{0136}' => 0xcd,
        '\u{0137}' => 0xed,
        '\u{013b}' => 0xcf,
        '\u{013c}' => 0xef,
        '\u{0141}' => 0xd9,
        '\u{0142}' => 0xf9,
        '\u{0143}' => 0xd1,
        '\u{0144}' => 0xf1,
        '\u{0145}' => 0xd2,
        '\u{0146}' => 0xf2,
        '\u{014c}' => 0xd4,
        '\u{014d}' => 0xf4,
        '\u{0156}' => 0xaa,
        '\u{0157}' => 0xba,
        '\u{015a}' => 0xda,
        '\u{015b}' => 0xfa,
        '\u{016a}' => 0xdb,
        '\u{016b}' => 0xfb,
        '\u{0172}' => 0xd8,
        '\u{0173}' => 0xf8,
        '\u{0179}' => 0xca,
        '\u{017a}' => 0xea,
        '\u{017b}' => 0x8c,
        '\u{017c}' => 0x9c,
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

pub(crate) fn passive_base14_covers_latin_extended_char(ch: char) -> bool {
    extended_latin_entry_for_char(ch).is_some()
}

pub(in crate::pdf) fn extended_latin_entry_for_char(ch: char) -> Option<ExtendedLatinEntry> {
    let (byte, glyph_name) = match ch {
        '\u{0100}' => (0xc2, b"Amacron".as_slice()),
        '\u{0101}' => (0xe2, b"amacron".as_slice()),
        '\u{0104}' => (0xc0, b"Aogonek".as_slice()),
        '\u{0105}' => (0xe0, b"aogonek".as_slice()),
        '\u{0106}' => (0xc3, b"Cacute".as_slice()),
        '\u{0107}' => (0xe3, b"cacute".as_slice()),
        '\u{010c}' => (0xc8, b"Ccaron".as_slice()),
        '\u{010d}' => (0xe8, b"ccaron".as_slice()),
        '\u{0112}' => (0xc7, b"Emacron".as_slice()),
        '\u{0113}' => (0xe7, b"emacron".as_slice()),
        '\u{0116}' => (0xcb, b"Edotaccent".as_slice()),
        '\u{0117}' => (0xeb, b"edotaccent".as_slice()),
        '\u{0118}' => (0xc6, b"Eogonek".as_slice()),
        '\u{0119}' => (0xe6, b"eogonek".as_slice()),
        '\u{011e}' => (0xd0, b"Gbreve".as_slice()),
        '\u{011f}' => (0xf0, b"gbreve".as_slice()),
        '\u{0122}' => (0xcc, b"Gcommaaccent".as_slice()),
        '\u{0123}' => (0xec, b"gcommaaccent".as_slice()),
        '\u{012a}' => (0xce, b"Imacron".as_slice()),
        '\u{012b}' => (0xee, b"imacron".as_slice()),
        '\u{012e}' => (0xc1, b"Iogonek".as_slice()),
        '\u{012f}' => (0xe1, b"iogonek".as_slice()),
        '\u{0130}' => (0xdd, b"Idotaccent".as_slice()),
        '\u{0131}' => (0xfd, b"dotlessi".as_slice()),
        '\u{0136}' => (0xcd, b"Kcommaaccent".as_slice()),
        '\u{0137}' => (0xed, b"kcommaaccent".as_slice()),
        '\u{013b}' => (0xcf, b"Lcommaaccent".as_slice()),
        '\u{013c}' => (0xef, b"lcommaaccent".as_slice()),
        '\u{0141}' => (0xd9, b"Lslash".as_slice()),
        '\u{0142}' => (0xf9, b"lslash".as_slice()),
        '\u{0143}' => (0xd1, b"Nacute".as_slice()),
        '\u{0144}' => (0xf1, b"nacute".as_slice()),
        '\u{0145}' => (0xd2, b"Ncommaaccent".as_slice()),
        '\u{0146}' => (0xf2, b"ncommaaccent".as_slice()),
        '\u{014c}' => (0xd4, b"Omacron".as_slice()),
        '\u{014d}' => (0xf4, b"omacron".as_slice()),
        '\u{0150}' => (0x81, b"Ohungarumlaut".as_slice()),
        '\u{0151}' => (0x8d, b"ohungarumlaut".as_slice()),
        '\u{0156}' => (0xaa, b"Rcommaaccent".as_slice()),
        '\u{0157}' => (0xba, b"rcommaaccent".as_slice()),
        '\u{015a}' => (0xda, b"Sacute".as_slice()),
        '\u{015b}' => (0xfa, b"sacute".as_slice()),
        '\u{015e}' => (0xde, b"Scedilla".as_slice()),
        '\u{015f}' => (0xfe, b"scedilla".as_slice()),
        '\u{016a}' => (0xdb, b"Umacron".as_slice()),
        '\u{016b}' => (0xfb, b"umacron".as_slice()),
        '\u{0170}' => (0x8f, b"Uhungarumlaut".as_slice()),
        '\u{0171}' => (0x90, b"uhungarumlaut".as_slice()),
        '\u{0172}' => (0xd8, b"Uogonek".as_slice()),
        '\u{0173}' => (0xf8, b"uogonek".as_slice()),
        '\u{0179}' => (0xca, b"Zacute".as_slice()),
        '\u{017a}' => (0xea, b"zacute".as_slice()),
        '\u{017b}' => (0x8c, b"Zdotaccent".as_slice()),
        '\u{017c}' => (0x9c, b"zdotaccent".as_slice()),
        _ => return None,
    };
    Some(ExtendedLatinEntry {
        byte,
        unicode: ch,
        glyph_name,
    })
}

pub(in crate::pdf) fn set_fill_color(content: &mut Content, color: PdfColor) {
    content.set_fill_rgb(color.red, color.green, color.blue);
}

pub(in crate::pdf) fn set_stroke_color(content: &mut Content, color: PdfColor) {
    content.set_stroke_rgb(color.red, color.green, color.blue);
}

pub(in crate::pdf) fn write_text_fragment(
    content: &mut Content,
    text: &str,
    font_resource: &[u8],
    passive_kerning_family: Option<PdfFontFamily>,
    supplied_kerning_points: Option<&[f32]>,
    style: &CharacterStyle,
    word_spacing: f32,
    x: f32,
    baseline_y: f32,
    rotation: TextRotation,
    encoded: &[u8],
    rendering_mode: TextRenderingMode,
) {
    content.begin_text();
    content.set_font(Name(font_resource), style.font_size_points());
    if rendering_mode != TextRenderingMode::Fill {
        content.set_text_rendering_mode(rendering_mode);
    }
    content.set_word_spacing(word_spacing);
    content.set_char_spacing(
        twips_to_points(style.character_spacing_twips) / style.horizontal_scale().max(0.01),
    );
    content.set_horizontal_scaling(style.character_scaling_percent as f32);
    set_text_position(content, x, baseline_y, rotation);
    if let Some(adjustments) = supplied_kerning_points
        && adjustments.iter().any(|adjustment| *adjustment != 0.0)
    {
        write_positioned_supplied_text(content, encoded, adjustments, style);
    } else if let Some(font_family) = passive_kerning_family
        && style_uses_passive_kerning(style)
        && text_has_passive_kerning(text, font_family, style)
    {
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

pub(in crate::pdf) fn write_positioned_supplied_text(
    content: &mut Content,
    encoded: &[u8],
    adjustments: &[f32],
    style: &CharacterStyle,
) {
    if encoded.len() != adjustments.len().saturating_mul(2) {
        content.show(Str(encoded));
        return;
    }
    let mut positioned = content.show_positioned();
    let mut items = positioned.items();
    let font_size = style.font_size_points().max(1.0);
    let mut run_start = 0usize;
    for (index, adjustment) in adjustments.iter().enumerate() {
        if *adjustment != 0.0 {
            let boundary = index.saturating_mul(2);
            if run_start < boundary {
                items.show(Str(&encoded[run_start..boundary]));
            }
            items.adjust((-adjustment * 1000.0 / font_size).clamp(-1000.0, 1000.0));
            run_start = boundary;
        }
    }
    if run_start < encoded.len() {
        items.show(Str(&encoded[run_start..]));
    }
}

pub(in crate::pdf) fn set_text_position(
    content: &mut Content,
    x: f32,
    baseline_y: f32,
    rotation: TextRotation,
) {
    match rotation {
        TextRotation::None => {
            content.next_line(x, baseline_y);
        }
        TextRotation::Clockwise90 => {
            content.set_text_matrix([0.0, -1.0, 1.0, 0.0, x, baseline_y]);
        }
        TextRotation::CounterClockwise90 => {
            content.set_text_matrix([0.0, 1.0, -1.0, 0.0, x, baseline_y]);
        }
    }
}

pub(in crate::pdf) fn text_has_passive_kerning(
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

pub(in crate::pdf) fn write_positioned_text(
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

pub(in crate::pdf) fn shadow_offset(style: &CharacterStyle) -> f32 {
    (style.font_size_points() * 0.08).clamp(0.75, 2.5)
}

pub(in crate::pdf) fn shadow_color(color: PdfColor) -> PdfColor {
    PdfColor {
        red: color.red * 0.35,
        green: color.green * 0.35,
        blue: color.blue * 0.35,
    }
}

pub(in crate::pdf) fn relief_offset(style: &CharacterStyle) -> f32 {
    (style.font_size_points() * 0.08).clamp(0.5, 2.5)
}

pub(in crate::pdf) fn relief_layers(
    color: PdfColor,
    relief: TextRelief,
    offset: f32,
) -> (PdfColor, f32, f32, PdfColor, f32, f32) {
    let light = relief_lighten_color(color);
    let dark = shadow_color(color);
    match relief {
        TextRelief::None => (color, 0.0, 0.0, color, 0.0, 0.0),
        TextRelief::Emboss => (light, 0.0, 0.0, dark, -2.0 * offset, 2.0 * offset),
        TextRelief::Engrave => (dark, 0.0, 0.0, light, -2.0 * offset, 2.0 * offset),
    }
}

pub(in crate::pdf) fn relief_lighten_color(color: PdfColor) -> PdfColor {
    PdfColor {
        red: 0.6 + color.red * 0.4,
        green: 0.6 + color.green * 0.4,
        blue: 0.6 + color.blue * 0.4,
    }
}

pub(in crate::pdf) fn lighten_color(color: PdfColor) -> PdfColor {
    PdfColor {
        red: 1.0 - ((1.0 - color.red) * 0.25),
        green: 1.0 - ((1.0 - color.green) * 0.25),
        blue: 1.0 - ((1.0 - color.blue) * 0.25),
    }
}
