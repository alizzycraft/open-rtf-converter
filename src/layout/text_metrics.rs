//! Text emission, decoration, font selection, measurement, and kerning.

use super::*;

pub(in crate::layout) fn push_text_segments_preserving_tabs(
    output: &mut Vec<FlowRun>,
    text: &str,
    style: &CharacterStyle,
    soft_hyphen_after: bool,
) {
    let mut start = 0;
    for (idx, ch) in text.char_indices() {
        if ch == '\t' {
            if start < idx {
                let text = text[start..idx].replace('\u{00ad}', "");
                if !text.is_empty() {
                    push_display_text_segment(output, &text, style, false);
                }
            }
            output.push(FlowRun {
                dynamic_field: None,
                text: "\t".to_string(),
                style: style.clone(),
                width: 0.0,
                line_height_points: style.font_size_points(),
                tab_leader: TabLeader::None,
                tab_alignment: TabAlignment::Left,
                tab_stop_position: None,
                soft_hyphen_after: false,
            });
            start = idx + ch.len_utf8();
        }
    }
    if start < text.len() {
        let text = text[start..].replace('\u{00ad}', "");
        if text.is_empty() {
            mark_last_soft_hyphen(output);
        } else {
            push_display_text_segment(output, &text, style, soft_hyphen_after);
        }
    } else if soft_hyphen_after {
        mark_last_soft_hyphen(output);
    }
}

pub(in crate::layout) fn push_display_text_segment(
    output: &mut Vec<FlowRun>,
    text: &str,
    style: &CharacterStyle,
    soft_hyphen_after: bool,
) {
    push_display_text_segment_without_bookmark_markers(output, text, style, soft_hyphen_after);
}

pub(in crate::layout) fn push_display_text_segment_without_bookmark_markers(
    output: &mut Vec<FlowRun>,
    text: &str,
    style: &CharacterStyle,
    soft_hyphen_after: bool,
) {
    if style.small_caps && !style.all_caps {
        push_small_caps_segments(output, text, style, soft_hyphen_after);
        return;
    }

    push_passive_font_fallback_segments(output, text, style, soft_hyphen_after);
}

pub(in crate::layout) fn push_passive_font_fallback_segments(
    output: &mut Vec<FlowRun>,
    text: &str,
    style: &CharacterStyle,
    soft_hyphen_after: bool,
) {
    let mut start = 0;
    let mut current_kind: Option<PassiveFontRunKind> = None;
    let mut segments = Vec::new();

    for (idx, ch) in text.char_indices() {
        let kind = PassiveFontRunKind::for_char(ch);
        match current_kind {
            None => current_kind = Some(kind),
            Some(previous) if previous != kind => {
                segments.push((start, idx));
                start = idx;
                current_kind = Some(kind);
            }
            Some(_) => {}
        }
    }

    if start < text.len() {
        segments.push((start, text.len()));
    }

    if segments.is_empty() {
        return;
    }

    let last_index = segments.len().saturating_sub(1);
    for (idx, (start, end)) in segments.into_iter().enumerate() {
        push_flow_run(
            output,
            &text[start..end],
            style,
            soft_hyphen_after && idx == last_index,
        );
    }
}

pub(in crate::layout) fn push_flow_run(
    output: &mut Vec<FlowRun>,
    text: &str,
    style: &CharacterStyle,
    soft_hyphen_after: bool,
) {
    output.push(FlowRun {
        dynamic_field: None,
        text: text.to_string(),
        style: style.clone(),
        width: 0.0,
        line_height_points: style.font_size_points(),
        tab_leader: TabLeader::None,
        tab_alignment: TabAlignment::Left,
        tab_stop_position: None,
        soft_hyphen_after,
    });
}

pub(in crate::layout) fn push_small_caps_segments(
    output: &mut Vec<FlowRun>,
    text: &str,
    style: &CharacterStyle,
    soft_hyphen_after: bool,
) {
    let mut start = 0;
    let mut current_lowercase = None;
    for (idx, ch) in text.char_indices() {
        let is_lowercase = ch.is_lowercase();
        match current_lowercase {
            None => current_lowercase = Some(is_lowercase),
            Some(previous) if previous != is_lowercase => {
                push_small_caps_segment(output, &text[start..idx], style, previous, false);
                start = idx;
                current_lowercase = Some(is_lowercase);
            }
            Some(_) => {}
        }
    }

    if start < text.len() {
        push_small_caps_segment(
            output,
            &text[start..],
            style,
            current_lowercase.unwrap_or(false),
            soft_hyphen_after,
        );
    }
}

pub(in crate::layout) fn push_small_caps_segment(
    output: &mut Vec<FlowRun>,
    text: &str,
    style: &CharacterStyle,
    lowercase: bool,
    soft_hyphen_after: bool,
) {
    let mut segment_style = style.clone();
    if lowercase {
        segment_style.font_size_half_points =
            scaled_small_caps_font_size(style.font_size_half_points);
    }
    output.push(FlowRun {
        dynamic_field: None,
        text: text.to_string(),
        style: segment_style,
        width: 0.0,
        line_height_points: style.font_size_points(),
        tab_leader: TabLeader::None,
        tab_alignment: TabAlignment::Left,
        tab_stop_position: None,
        soft_hyphen_after,
    });
}

pub(in crate::layout) fn scaled_small_caps_font_size(font_size_half_points: i32) -> i32 {
    ((font_size_half_points.max(2) as f32) * SMALL_CAPS_FONT_SCALE)
        .round()
        .max(2.0)
        .min(font_size_half_points.max(2) as f32) as i32
}

pub(in crate::layout) fn mark_last_soft_hyphen(output: &mut [FlowRun]) {
    if let Some(last) = output
        .iter_mut()
        .rev()
        .find(|run| run.text != "\t" && run.text != "\n")
    {
        last.soft_hyphen_after = true;
    }
}

pub(in crate::layout) fn empty_line() -> Line {
    Line {
        runs: Vec::new(),
        width: 0.0,
        natural_height: 14.0,
        height: 14.0,
        segmented_flow: false,
        inline_image_baseline_context: false,
    }
}

pub(in crate::layout) fn push_segment(
    line: &mut Line,
    mut segment: FlowRun,
    paragraph_style: &ParagraphStyle,
    document: &Document,
    font_provider: Option<&FontProvider>,
) {
    if segment.text == "\t" {
        segment.tab_leader = next_tab_leader(line.width, paragraph_style);
        segment.tab_alignment = next_tab_alignment(line.width, paragraph_style);
        segment.tab_stop_position = Some(next_tab_position(line.width, paragraph_style, document));
    } else {
        apply_fit_text_scaling_to_segment(
            &mut segment,
            line.width,
            paragraph_style,
            document,
            font_provider,
        );
    }
    segment.width = measure_flow_run(
        &segment,
        line.width,
        paragraph_style,
        document,
        font_provider,
    );
    if segment.text != "\t" {
        adjust_pending_aligned_tab(line, &segment, document, font_provider);
    }
    line.width += segment.width;
    if let Some(image) = inline_image_for_kind(document, segment.dynamic_field) {
        let (_, image_height) = image_display_size(image, f32::MAX);
        line.height = line.height.max(image_height);
        line.runs.push(segment);
        return;
    }
    let segment_height = flow_run_line_height(&segment, document, font_provider);
    let has_visible_height = line
        .runs
        .iter()
        .any(|run| flow_run_line_height(run, document, font_provider) > 0.0);
    if !has_visible_height && segment_height > 0.0 {
        line.natural_height = segment_height;
        line.height = segment_height;
    } else {
        line.natural_height = line.natural_height.max(segment_height);
        line.height = line.height.max(segment_height);
    }
    line.runs.push(segment);
}

pub(in crate::layout) fn push_segment_with_trailing_character_spacing(
    line: &mut Line,
    segment: FlowRun,
    paragraph_style: &ParagraphStyle,
    document: &Document,
    font_provider: Option<&FontProvider>,
    trailing_character_spacing: f32,
) {
    push_segment(line, segment, paragraph_style, document, font_provider);
    if trailing_character_spacing == 0.0 {
        return;
    }
    if let Some(last) = line.runs.last_mut() {
        last.width += trailing_character_spacing;
        line.width += trailing_character_spacing;
    }
}

pub(in crate::layout) fn push_flow_exclusion_advance(
    line: &mut Line,
    width: f32,
    style: &CharacterStyle,
) {
    if width <= 0.01 {
        return;
    }
    line.runs.push(FlowRun {
        dynamic_field: None,
        text: String::new(),
        style: style.clone(),
        width,
        line_height_points: 0.0,
        tab_leader: TabLeader::None,
        tab_alignment: TabAlignment::Left,
        tab_stop_position: None,
        soft_hyphen_after: false,
    });
    line.width += width;
}

pub(in crate::layout) fn apply_fit_text_scaling_to_segment(
    segment: &mut FlowRun,
    current_line_width: f32,
    paragraph_style: &ParagraphStyle,
    document: &Document,
    font_provider: Option<&FontProvider>,
) {
    let Some(target_twips) = segment.style.fit_text_width_twips else {
        return;
    };
    if target_twips <= 0
        || segment.text.is_empty()
        || layout_advance(segment.dynamic_field).is_some()
        || is_passive_math_inline_lead_marker(segment.dynamic_field)
        || is_passive_math_fraction_rule_marker(segment.dynamic_field)
        || is_passive_math_stack_anchor_marker(segment.dynamic_field)
        || is_bookmark_page_kind(segment.dynamic_field)
    {
        return;
    }

    let mut measured_segment = segment.clone();
    measured_segment.style.fit_text_width_twips = None;
    let measured_width = measure_flow_run(
        &measured_segment,
        current_line_width,
        paragraph_style,
        document,
        font_provider,
    );
    if measured_width <= 0.0 {
        return;
    }

    let target_width = twips_to_points(target_twips).max(0.1);
    let scaled = (segment.style.character_scaling_percent as f32 * target_width / measured_width)
        .round()
        .clamp(1.0, 600.0);
    segment.style.character_scaling_percent = scaled as i32;
}

pub(in crate::layout) fn adjust_pending_aligned_tab(
    line: &mut Line,
    following: &FlowRun,
    document: &Document,
    font_provider: Option<&FontProvider>,
) {
    let Some(tab) = line.runs.last_mut() else {
        return;
    };
    if tab.text != "\t" || matches!(tab.tab_alignment, TabAlignment::Left | TabAlignment::Bar) {
        return;
    }
    let Some(target) = tab.tab_stop_position else {
        return;
    };
    let width_before_tab = line.width - tab.width;
    let desired_start = match tab.tab_alignment {
        TabAlignment::Left => target,
        TabAlignment::Center => target - (following.width / 2.0),
        TabAlignment::Right => target - following.width,
        TabAlignment::Decimal => decimal_tab_start(target, following, document, font_provider)
            .unwrap_or(target - following.width),
        TabAlignment::Bar => target,
    };
    let adjusted_width = (desired_start - width_before_tab).max(0.0);
    tab.width = adjusted_width;
    line.width = width_before_tab + adjusted_width;
}

pub(in crate::layout) fn decimal_tab_start(
    target: f32,
    following: &FlowRun,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> Option<f32> {
    let style = passive_pdf_style_for_run(document, &following.style);
    let text = display_text(&following.text, &style);
    let decimal_idx = text.find(['.', ','])?;
    let prefix = &text[..decimal_idx];
    Some(
        target
            - measure_text_with_document_font(
                prefix,
                &style,
                font_family_for_run_text(document, &style, prefix),
                document,
                font_provider,
            ),
    )
}

pub(in crate::layout) fn materialize_line_end_soft_hyphen(
    line: &mut Line,
    document: &Document,
    font_provider: Option<&FontProvider>,
) {
    let Some(last) = line
        .runs
        .iter_mut()
        .rev()
        .find(|run| run.text != "\t" && run.text != "\n")
    else {
        return;
    };
    if !last.soft_hyphen_after {
        return;
    }
    last.text.push('-');
    last.soft_hyphen_after = false;
    let style = passive_pdf_style_for_run(document, &last.style);
    let added_width = measure_text_with_document_font(
        "-",
        &style,
        font_family_for_style(document, &style),
        document,
        font_provider,
    );
    last.width += added_width;
    line.width += added_width;
    let run_height = flow_run_line_height(last, document, font_provider);
    line.natural_height = line.natural_height.max(run_height);
    line.height = line.height.max(run_height);
}

pub(in crate::layout) fn flow_run_line_height(
    run: &FlowRun,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> f32 {
    if is_passive_math_inline_lead_marker(run.dynamic_field) {
        return 0.0;
    }
    if is_passive_math_fraction_rule_marker(run.dynamic_field) {
        return (run.style.font_size_points() * PASSIVE_MATH_FRACTION_LINE_HEIGHT_MULTIPLIER)
            + run.style.baseline_shift_points().abs();
    }
    if is_passive_math_stack_anchor_marker(run.dynamic_field) {
        return 0.0;
    }
    if run.text.is_empty() {
        return 0.0;
    }
    if inline_image_id(run.dynamic_field).is_some() {
        return 0.0;
    }
    if layout_advance(run.dynamic_field).is_some() {
        return 0.0;
    }
    let border_extra = character_border_inset(&run.style) * 2.0;
    if run.text.chars().any(is_passive_checkbox_char) {
        return (run.line_height_points * 1.15)
            + run.style.baseline_shift_points().abs()
            + border_extra;
    }
    let fallback = fallback_flow_run_line_height(run) + border_extra;
    let passive_base14_fallback =
        || passive_base14_fallback_flow_run_line_height(run, document) + border_extra;
    if let Some(line_height) = source_word_compatible_line_height_points(
        font_for_style(document, &run.style),
        run.line_height_points,
        font_provider,
    ) {
        return line_height.clamp(
            run.line_height_points.max(1.0),
            run.line_height_points.max(1.0) * 2.0,
        ) + run.style.baseline_shift_points().abs()
            + border_extra;
    }
    let Some(provider) = font_provider else {
        return fallback;
    };
    let Some(font) = font_for_style(document, &run.style) else {
        return passive_base14_fallback();
    };
    let metric_char = run
        .text
        .chars()
        .find(|ch| !ch.is_control() && !ch.is_whitespace())
        .unwrap_or('A');
    let Some(line_height) = supplied_font_glyph_metrics(provider, font, &run.style, metric_char)
        .or_else(|| supplied_font_glyph_metrics(provider, font, &run.style, 'A'))
        .and_then(|metrics| metrics.line_height_points(run.line_height_points))
    else {
        return passive_base14_fallback();
    };
    let bounded_line_height = line_height.clamp(
        run.line_height_points.max(1.0),
        run.line_height_points.max(1.0) * 2.0,
    );
    bounded_line_height + run.style.baseline_shift_points().abs() + border_extra
}

pub(in crate::layout) fn source_word_compatible_line_height_points(
    font: Option<&FontDef>,
    size: f32,
    font_provider: Option<&FontProvider>,
) -> Option<f32> {
    let font = font?;
    let units = if font_uses_source_family(font, font_provider, &["calibri", "carlito", "aptos"]) {
        // Calibri/Carlito: 1536 - -512 + 452. Aptos: 1923 - -577.
        2_500.0
    } else if font_uses_source_family(font, font_provider, &["cambria math"]) {
        // Word's registered inline Office Math references use a 14.04-point
        // line box at 12 points: 2396 / 2048 em after PDF-unit rounding.
        2_396.0
    } else if font_uses_source_family(font, font_provider, &["cambria"]) {
        // Cambria: 1946 - -455.
        2_401.0
    } else {
        return None;
    };
    Some(size * (units / 2_048.0))
}

pub(in crate::layout) fn fallback_flow_run_line_height(run: &FlowRun) -> f32 {
    (run.line_height_points * 1.25) + run.style.baseline_shift_points().abs()
}

pub(in crate::layout) fn passive_base14_fallback_flow_run_line_height(
    run: &FlowRun,
    document: &Document,
) -> f32 {
    let style = passive_pdf_style_for_run(document, &run.style);
    let family = font_family_for_run_text(document, &style, &run.text);
    let multiplier = base14_line_height_multiplier(family);
    (run.line_height_points * multiplier) + run.style.baseline_shift_points().abs()
}

pub(in crate::layout) fn base14_line_height_multiplier(family: PdfFontFamily) -> f32 {
    match family {
        PdfFontFamily::Courier => 1.12,
        PdfFontFamily::Helvetica | PdfFontFamily::Times => 1.15,
        PdfFontFamily::Symbol | PdfFontFamily::ZapfDingbats => 1.15,
    }
}

pub(in crate::layout) fn measure_flow_run(
    run: &FlowRun,
    current_line_width: f32,
    paragraph_style: &ParagraphStyle,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> f32 {
    if let Some(image) = inline_image_for_kind(document, run.dynamic_field) {
        return image_display_size(image, f32::MAX).0;
    }
    if is_bookmark_page_kind(run.dynamic_field) {
        return 0.0;
    }
    if let Some(advance) = layout_advance(run.dynamic_field) {
        return twips_to_points(advance.horizontal_twips);
    }
    if is_passive_math_inline_lead_marker(run.dynamic_field) {
        return passive_advance_width_points(&run.style);
    }
    if is_passive_math_fraction_rule_marker(run.dynamic_field) {
        return 0.0;
    }
    if is_passive_math_stack_anchor_marker(run.dynamic_field) {
        return 0.0;
    }
    if run.text == "\t" {
        return next_tab_position(current_line_width, paragraph_style, document)
            - current_line_width;
    }
    let style = passive_pdf_style_for_run(document, &run.style);
    let text = display_text(&run.text, &style);
    let font_family = font_family_for_run_text(document, &style, &text);
    let text_width =
        measure_text_with_document_font(&text, &style, font_family, document, font_provider);
    text_width + (character_border_inset(&style) * 2.0)
}

pub(in crate::layout) fn character_border_inset(style: &CharacterStyle) -> f32 {
    if !style.border.visible {
        return 0.0;
    }
    let stroke_width = twips_to_points(style.border.width_twips.max(1));
    stroke_width + twips_to_points(style.border.spacing_twips.max(0))
}

pub(in crate::layout) fn inline_image_cluster_width(
    segments: &[FlowRun],
    start: usize,
    first_width: f32,
    current_line_width: f32,
    paragraph_style: &ParagraphStyle,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> f32 {
    let Some(first) = segments.get(start) else {
        return first_width;
    };
    if first.text.chars().last().is_some_and(char::is_whitespace) {
        return first_width;
    }

    let mut width = first_width;
    let mut contains_inline_image = inline_image_id(first.dynamic_field).is_some();
    for segment in segments.iter().skip(start + 1) {
        if segment.text.chars().next().is_some_and(char::is_whitespace) {
            break;
        }
        contains_inline_image |= inline_image_id(segment.dynamic_field).is_some();
        width += measure_flow_run(
            segment,
            current_line_width + width,
            paragraph_style,
            document,
            font_provider,
        );
        if segment.text.chars().last().is_some_and(char::is_whitespace) {
            break;
        }
    }
    if contains_inline_image {
        width
    } else {
        first_width
    }
}

pub(in crate::layout) fn layout_advance(kind: Option<InlineKind>) -> Option<LayoutAdvance> {
    match kind {
        Some(InlineKind::LayoutAdvance(advance)) => Some(advance),
        _ => None,
    }
}

pub(in crate::layout) fn is_passive_math_inline_lead_marker(kind: Option<InlineKind>) -> bool {
    kind == Some(InlineKind::Math(MathInline::InlineLead))
}

pub(in crate::layout) fn is_passive_math_fraction_rule_marker(kind: Option<InlineKind>) -> bool {
    kind == Some(InlineKind::Math(MathInline::FractionRule))
}

pub(in crate::layout) fn is_passive_math_stack_anchor_marker(kind: Option<InlineKind>) -> bool {
    kind == Some(InlineKind::Math(MathInline::StackAnchor))
}

pub(in crate::layout) fn inline_image_id(kind: Option<InlineKind>) -> Option<InlineImageId> {
    match kind {
        Some(InlineKind::InlineImage(id)) => Some(id),
        _ => None,
    }
}

pub(in crate::layout) fn is_bookmark_page_kind(kind: Option<InlineKind>) -> bool {
    matches!(
        kind,
        Some(InlineKind::BookmarkPageAnchor(_) | InlineKind::BookmarkPageReference(_))
    )
}

pub(in crate::layout) fn is_note_reference_kind(kind: Option<InlineKind>) -> bool {
    matches!(
        kind,
        Some(InlineKind::FootnoteReference(_) | InlineKind::EndnoteReference(_))
    )
}

pub(in crate::layout) fn inline_image_for_kind<'a>(
    document: &'a Document,
    kind: Option<InlineKind>,
) -> Option<&'a StaticImage> {
    document
        .inline_images
        .get(inline_image_id(kind)?.get())
        .map(|entry| &entry.image)
}

pub(in crate::layout) fn passive_advance_width_points(style: &CharacterStyle) -> f32 {
    twips_to_points(style.character_spacing_twips)
}

pub(in crate::layout) fn passive_advance_baseline_delta_points(advance: LayoutAdvance) -> f32 {
    twips_to_points(advance.vertical_twips)
}

pub(in crate::layout) fn passive_math_fraction_width_points(style: &CharacterStyle) -> f32 {
    twips_to_points(style.character_spacing_twips).max(0.0)
}

pub(in crate::layout) fn passive_math_stack_width_points(style: &CharacterStyle) -> f32 {
    twips_to_points(style.character_spacing_twips).max(0.0)
}

pub(in crate::layout) fn next_tab_position(
    current_line_width: f32,
    paragraph_style: &ParagraphStyle,
    document: &Document,
) -> f32 {
    for (idx, stop_twips) in paragraph_style.tab_stops_twips.iter().enumerate() {
        if paragraph_style
            .tab_stop_alignments
            .get(idx)
            .is_some_and(|alignment| *alignment == TabAlignment::Bar)
        {
            continue;
        }
        let stop = twips_to_points(*stop_twips);
        if stop > current_line_width + 0.01 {
            return stop;
        }
    }

    let default_stop = twips_to_points(document.default_tab_width_twips.max(1));
    (((current_line_width / default_stop).floor() + 1.0) * default_stop).max(default_stop)
}

pub(in crate::layout) fn next_tab_leader(
    current_line_width: f32,
    paragraph_style: &ParagraphStyle,
) -> TabLeader {
    for (idx, stop_twips) in paragraph_style.tab_stops_twips.iter().enumerate() {
        if paragraph_style
            .tab_stop_alignments
            .get(idx)
            .is_some_and(|alignment| *alignment == TabAlignment::Bar)
        {
            continue;
        }
        let stop = twips_to_points(*stop_twips);
        if stop > current_line_width + 0.01 {
            return paragraph_style
                .tab_stop_leaders
                .get(idx)
                .copied()
                .unwrap_or_default();
        }
    }
    TabLeader::None
}

pub(in crate::layout) fn next_tab_alignment(
    current_line_width: f32,
    paragraph_style: &ParagraphStyle,
) -> TabAlignment {
    for (idx, stop_twips) in paragraph_style.tab_stops_twips.iter().enumerate() {
        if paragraph_style
            .tab_stop_alignments
            .get(idx)
            .is_some_and(|alignment| *alignment == TabAlignment::Bar)
        {
            continue;
        }
        let stop = twips_to_points(*stop_twips);
        if stop > current_line_width + 0.01 {
            return paragraph_style
                .tab_stop_alignments
                .get(idx)
                .copied()
                .unwrap_or_default();
        }
    }
    TabAlignment::Left
}

pub(in crate::layout) fn apply_line_spacing(line_height: f32, style: &ParagraphStyle) -> f32 {
    let Some(spacing_twips) = style.line_spacing_twips else {
        return line_height;
    };
    if spacing_twips == 0 {
        return line_height;
    }

    if style.line_spacing_multiple {
        let multiplier = (spacing_twips.unsigned_abs() as f32 / 240.0).clamp(0.25, 12.0);
        return (line_height * multiplier).max(1.0);
    }

    let spacing_points = twips_to_points(spacing_twips.abs()).max(1.0);
    if spacing_twips < 0 {
        spacing_points
    } else {
        line_height.max(spacing_points)
    }
}

pub(in crate::layout) fn apply_line_spacing_with_grid(
    line_height: f32,
    style: &ParagraphStyle,
    geometry: PageGeometry,
) -> f32 {
    let spaced = apply_line_spacing(line_height, style);
    if !style.snap_to_line_grid {
        return spaced;
    }
    let Some(grid_twips) = geometry.text_line_grid_twips else {
        return spaced;
    };
    if grid_twips <= 0 {
        return spaced;
    }
    spaced.max(twips_to_points(grid_twips))
}

pub(in crate::layout) fn push_bar_tab_stops(
    pages: &mut [LayoutPage],
    style: &ParagraphStyle,
    line_left: f32,
    top_y: f32,
    line_height: f32,
) {
    let page = pages.last_mut().expect("layout always has a page");
    for (idx, stop_twips) in style.tab_stops_twips.iter().enumerate() {
        if !style
            .tab_stop_alignments
            .get(idx)
            .is_some_and(|alignment| *alignment == TabAlignment::Bar)
        {
            continue;
        }
        let x = line_left + twips_to_points(*stop_twips);
        page.items.push(LayoutItem::Line {
            x1: x,
            y1: top_y,
            x2: x,
            y2: top_y - line_height,
            width: 0.5,
            color: PdfColor {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
            },
            style: LineStyle::Solid,
        });
    }
}

pub(in crate::layout) fn push_line(
    pages: &mut [LayoutPage],
    line: &Line,
    x: f32,
    top_y: f32,
    document: &Document,
    font_provider: Option<&FontProvider>,
    word_spacing: f32,
    character_spacing: f32,
) {
    push_line_with_rotation(
        pages,
        line,
        x,
        top_y,
        document,
        font_provider,
        word_spacing,
        character_spacing,
        TextRotation::None,
    );
}

pub(in crate::layout) fn push_line_with_rotation(
    pages: &mut [LayoutPage],
    line: &Line,
    x: f32,
    top_y: f32,
    document: &Document,
    font_provider: Option<&FontProvider>,
    word_spacing: f32,
    character_spacing: f32,
    rotation: TextRotation,
) {
    // Word treats positive `\slN\slmult1` as baseline-to-baseline advance:
    // the extra leading follows the glyphs instead of pushing the first line
    // down inside the enlarged line box. Exact/at-least spacing retains the
    // historical box-relative baseline behavior.
    let baseline_box_height = if line.height > line.natural_height {
        line.natural_height
    } else {
        line.height
    };
    let has_inline_image = line
        .runs
        .iter()
        .any(|run| inline_image_id(run.dynamic_field).is_some());
    let line_border_inset = line
        .runs
        .iter()
        .map(|run| character_border_inset(&run.style))
        .fold(0.0_f32, f32::max);
    let baseline_y = if has_inline_image {
        top_y - line.height
    } else {
        top_y - baseline_box_height + (baseline_box_height * 0.25) + (line_border_inset * 0.5)
    };
    let mut cursor_advance = 0.0;
    let mut cursor_baseline_delta =
        word_compatible_source_line_baseline_delta_points(line, document, font_provider);
    let page = pages.last_mut().expect("layout always has a page");

    let last_spaced_run_index = line.runs.iter().rposition(|run| {
        layout_advance(run.dynamic_field).is_none()
            && !is_passive_math_inline_lead_marker(run.dynamic_field)
            && !is_passive_math_fraction_rule_marker(run.dynamic_field)
            && !is_passive_math_stack_anchor_marker(run.dynamic_field)
            && run.text != "\t"
            && !run.text.is_empty()
    });
    for (run_index, run) in line.runs.iter().enumerate() {
        if run.text.is_empty()
            && inline_image_id(run.dynamic_field).is_none()
            && !is_bookmark_page_kind(run.dynamic_field)
            && !is_note_reference_kind(run.dynamic_field)
            && layout_advance(run.dynamic_field).is_none()
            && !contains_inline_marker(run.dynamic_field)
        {
            cursor_advance += run.width;
            continue;
        }
        if let Some(advance) = layout_advance(run.dynamic_field) {
            cursor_advance += run.width;
            cursor_baseline_delta += passive_advance_baseline_delta_points(advance);
            continue;
        }
        if is_passive_math_inline_lead_marker(run.dynamic_field) {
            cursor_advance += run.width;
            continue;
        }
        if is_passive_math_fraction_rule_marker(run.dynamic_field) {
            if run.style.passive_math.fraction_has_bar {
                let fraction_width = passive_math_fraction_width_points(&run.style);
                let (x1, y1) = rotated_line_run_origin(
                    x,
                    baseline_y,
                    cursor_advance,
                    cursor_baseline_delta
                        + run.style.baseline_shift_points()
                        + PASSIVE_MATH_FRACTION_RULE_BASELINE_OFFSET_POINTS,
                    rotation,
                );
                let (x2, y2) = rotated_line_run_origin(
                    x,
                    baseline_y,
                    cursor_advance + fraction_width,
                    cursor_baseline_delta
                        + run.style.baseline_shift_points()
                        + PASSIVE_MATH_FRACTION_RULE_BASELINE_OFFSET_POINTS,
                    rotation,
                );
                page.items.push(LayoutItem::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    width: PASSIVE_MATH_FRACTION_RULE_WIDTH_POINTS,
                    color: style_color(document, &run.style),
                    style: LineStyle::Solid,
                });
            }
            continue;
        }
        if is_passive_math_stack_anchor_marker(run.dynamic_field) {
            continue;
        }
        if let Some(image) = inline_image_for_kind(document, run.dynamic_field) {
            let (width, height) = image_display_size(image, f32::MAX);
            let (run_x, run_baseline_y) = rotated_line_run_origin(
                x,
                baseline_y,
                cursor_advance,
                cursor_baseline_delta,
                rotation,
            );
            page.items.push(LayoutItem::Image(ImageFragment {
                image: image.clone(),
                x: run_x,
                y: run_baseline_y,
                width,
                height,
                z_order: 0,
                below_text: false,
            }));
            cursor_advance += width;
            continue;
        }
        let (run_x, run_baseline_y) = rotated_line_run_origin(
            x,
            baseline_y,
            cursor_advance,
            cursor_baseline_delta,
            rotation,
        );
        if is_bookmark_page_kind(run.dynamic_field) || is_note_reference_kind(run.dynamic_field) {
            page.items.push(LayoutItem::Text(TextFragment {
                dynamic_field: run.dynamic_field,
                text: run.text.clone(),
                x: run_x,
                baseline_y: run_baseline_y,
                rotation,
                color: style_color(document, &run.style),
                font_family: font_family_for_style(document, &run.style),
                word_spacing: 0.0,
                style: run.style.clone(),
            }));
            continue;
        }
        let extra_word_spacing = word_spacing * regular_space_count(&run.text) as f32;
        let character_count = run.text.chars().filter(|ch| *ch != '\n').count();
        let internal_character_gaps = character_count.saturating_sub(1);
        let extra_character_spacing = character_spacing * internal_character_gaps as f32;
        let width = run.width + extra_word_spacing + extra_character_spacing;
        if run.text == "\t" {
            push_tab_leader(page, run_x, run_baseline_y, width, run, document);
            cursor_advance += width;
            continue;
        }
        let mut style = passive_pdf_style_for_run(document, &run.style);
        if character_spacing > 0.0 {
            let added_twips = (character_spacing * 20.0)
                .round()
                .clamp(0.0, i32::MAX as f32) as i32;
            style.character_spacing_twips =
                style.character_spacing_twips.saturating_add(added_twips);
        }
        let border_inset = character_border_inset(&style);
        let text_width = (width - (border_inset * 2.0)).max(0.0);
        let text = display_text(&run.text, &style);
        let font_family = font_family_for_run_text(document, &style, &text);
        let (run_x, run_baseline_y) = rotated_line_run_origin(
            x,
            baseline_y,
            cursor_advance + border_inset,
            cursor_baseline_delta
                + style.baseline_shift_points()
                + word_compatible_source_run_baseline_delta_points(&style, document),
            rotation,
        );
        let color = style_color(document, &style);
        if let Some(highlight_index) = style.highlight_index
            && (highlight_index > 0 || style.highlight_shading_basis_points < 10_000)
        {
            let font_size = style.font_size_points();
            let (highlight_y, highlight_height) =
                if line_border_inset > 0.0 && !style.border.visible {
                    (
                        run_baseline_y - (font_size * 0.28) - line_border_inset,
                        line.height,
                    )
                } else {
                    (run_baseline_y - (font_size * 0.28), font_size * 1.12)
                };
            page.items.push(LayoutItem::Highlight {
                x: run_x,
                y: highlight_y,
                width: text_width,
                height: highlight_height,
                color: character_shading_color(document, highlight_index, &style),
            });
        }
        if style.form_field_shading {
            let font_size = style.font_size_points();
            let (shade_x, shade_width) =
                passive_form_field_shading_horizontal_bounds(run_x, text_width, font_size);
            page.items.push(LayoutItem::Highlight {
                x: shade_x,
                y: run_baseline_y - (font_size * 0.32),
                width: shade_width,
                height: font_size * 1.2,
                color: passive_form_field_shading_color(),
            });
        }
        if style.border.visible {
            push_character_border(
                page,
                run_x,
                run_baseline_y,
                text_width,
                &style,
                color,
                document,
            );
        }
        page.items.push(LayoutItem::Text(TextFragment {
            dynamic_field: run.dynamic_field,
            text: text.clone(),
            x: run_x,
            baseline_y: run_baseline_y,
            rotation,
            color,
            font_family,
            word_spacing,
            style: style.clone(),
        }));

        if style.overline {
            let y = run_baseline_y + (style.font_size_points() * 0.78);
            page.items.push(LayoutItem::Line {
                x1: run_x,
                y1: y,
                x2: run_x + text_width,
                y2: y,
                width: if style.passive_math.enabled {
                    0.84
                } else {
                    0.5
                },
                color,
                style: LineStyle::Solid,
            });
        }

        if style.underline != UnderlineStyle::None {
            let underline_color = style
                .underline_color_index
                .filter(|index| *index > 0)
                .map(|index| color_for_index(document, index))
                .unwrap_or(color);
            push_underline_items(
                page,
                run_x,
                run_baseline_y
                    - if style.underline == UnderlineStyle::Thick {
                        2.8
                    } else {
                        2.0
                    },
                text_width,
                &text,
                &style,
                font_family,
                underline_color,
                word_spacing,
            );
        }

        if style.strike {
            let y = run_baseline_y + (style.font_size_points() * 0.32);
            if style.double_strike {
                let gap = (style.font_size_points() * 0.1).clamp(1.0, 3.0);
                page.items.push(LayoutItem::Line {
                    x1: run_x,
                    y1: y + gap,
                    x2: run_x + text_width,
                    y2: y + gap,
                    width: 0.5,
                    color,
                    style: LineStyle::Solid,
                });
                page.items.push(LayoutItem::Line {
                    x1: run_x,
                    y1: y - gap,
                    x2: run_x + text_width,
                    y2: y - gap,
                    width: 0.5,
                    color,
                    style: LineStyle::Solid,
                });
            } else {
                page.items.push(LayoutItem::Line {
                    x1: run_x,
                    y1: y,
                    x2: run_x + text_width,
                    y2: y,
                    width: 0.5,
                    color,
                    style: LineStyle::Solid,
                });
            }
        }

        cursor_advance += width;
        if character_spacing > 0.0
            && character_count > 0
            && last_spaced_run_index.is_some_and(|last| run_index < last)
        {
            cursor_advance += character_spacing;
        }
    }
}

pub(in crate::layout) fn word_compatible_source_line_baseline_delta_points(
    line: &Line,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> f32 {
    if line.inline_image_baseline_context {
        return 0.0;
    }
    let source_inset = line
        .runs
        .iter()
        .filter(|run| {
            !run.text.trim().is_empty()
                && !contains_inline_marker(run.dynamic_field)
                && inline_image_id(run.dynamic_field).is_none()
        })
        .filter_map(|run| {
            let inset_em = match font_for_style(document, &run.style) {
                Some(font) => {
                    let name = font.name.trim().to_ascii_lowercase();
                    if matches!(name.as_str(), "arial" | "liberation sans") {
                        WORD_ARIAL_BASELINE_INSET_EM
                    } else if font_uses_source_family(font, font_provider, &["calibri", "carlito"])
                    {
                        WORD_CALIBRI_BASELINE_INSET_EM
                    } else if name == "tinos"
                        || name == "times new roman"
                        || name.starts_with("times new roman ")
                    {
                        WORD_TIMES_BASELINE_INSET_EM
                    } else {
                        return None;
                    }
                }
                None if font_family_for_style(document, &run.style) == PdfFontFamily::Times => {
                    WORD_TIMES_BASELINE_INSET_EM
                }
                None => return None,
            };
            Some(run.style.font_size_points() * inset_em)
        })
        .reduce(f32::max);
    -source_inset.unwrap_or(0.0)
}

pub(in crate::layout) fn word_compatible_source_run_baseline_delta_points(
    style: &CharacterStyle,
    document: &Document,
) -> f32 {
    let is_cambria_math_nary = style.passive_math.nary_id.is_some()
        && font_for_style(document, style)
            .is_some_and(|font| font.name.trim().eq_ignore_ascii_case("cambria math"));
    if is_cambria_math_nary {
        style.font_size_points() * WORD_CAMBRIA_MATH_NARY_BASELINE_RISE_EM
    } else {
        0.0
    }
}

pub(in crate::layout) fn rotated_line_run_origin(
    x: f32,
    baseline_y: f32,
    advance: f32,
    baseline_delta: f32,
    rotation: TextRotation,
) -> (f32, f32) {
    match rotation {
        TextRotation::None => (x + advance, baseline_y + baseline_delta),
        TextRotation::Clockwise90 => (x + baseline_delta, baseline_y - advance),
        TextRotation::CounterClockwise90 => (x - baseline_delta, baseline_y + advance),
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn push_underline_items(
    page: &mut LayoutPage,
    x: f32,
    y: f32,
    width: f32,
    text: &str,
    style: &CharacterStyle,
    font_family: PdfFontFamily,
    color: PdfColor,
    word_spacing: f32,
) {
    if style.underline != UnderlineStyle::Words {
        page.items.push(LayoutItem::Underline {
            x,
            y,
            width,
            color,
            style: style.underline,
        });
        return;
    }

    for (offset, span_width) in word_underline_spans(text, style, font_family, word_spacing) {
        page.items.push(LayoutItem::Underline {
            x: x + offset,
            y,
            width: span_width,
            color,
            style: UnderlineStyle::Single,
        });
    }
}

pub(in crate::layout) fn word_underline_spans(
    text: &str,
    style: &CharacterStyle,
    font_family: PdfFontFamily,
    word_spacing: f32,
) -> Vec<(f32, f32)> {
    let mut spans = Vec::new();
    let mut cursor = 0.0;
    let mut word_start = None;
    let mut word = String::new();

    for ch in text.chars() {
        if is_word_underline_gap(ch) {
            if let Some(start) = word_start.take() {
                let width = measure_late_page_count_text_with_family(&word, style, font_family);
                if width > 0.0 {
                    spans.push((start, width));
                }
                cursor += width;
                word.clear();
            }
            cursor += measure_late_page_count_text_with_family(&ch.to_string(), style, font_family);
            if ch == ' ' {
                cursor += word_spacing;
            }
        } else {
            if word_start.is_none() {
                word_start = Some(cursor);
            }
            word.push(ch);
        }
    }

    if let Some(start) = word_start {
        let width = measure_late_page_count_text_with_family(&word, style, font_family);
        if width > 0.0 {
            spans.push((start, width));
        }
    }

    spans
}

pub(in crate::layout) fn measure_late_page_count_text_with_family(
    text: &str,
    style: &CharacterStyle,
    font_family: PdfFontFamily,
) -> f32 {
    measure_text_with_family(text, style, font_family)
}

pub(in crate::layout) fn is_word_underline_gap(ch: char) -> bool {
    ch == '\u{00a0}' || ch.is_whitespace()
}

pub(in crate::layout) fn push_tab_leader(
    page: &mut LayoutPage,
    cursor_x: f32,
    baseline_y: f32,
    width: f32,
    run: &FlowRun,
    document: &Document,
) {
    if width <= 1.0 || run.tab_leader == TabLeader::None {
        return;
    }
    let style = passive_pdf_style_for_run(document, &run.style);
    let color = style_color(document, &style);
    match run.tab_leader {
        TabLeader::None => {}
        TabLeader::Dots
        | TabLeader::Hyphens
        | TabLeader::Underline
        | TabLeader::MiddleDots
        | TabLeader::Equals => {
            let leader = match run.tab_leader {
                TabLeader::Dots => ".",
                TabLeader::Hyphens => "-",
                TabLeader::Underline => "_",
                TabLeader::MiddleDots => "\u{00b7}",
                TabLeader::Equals => "=",
                TabLeader::None => unreachable!("handled above"),
            };
            let family = font_family_for_style(document, &style);
            let leader_width = measure_text_with_family(leader, &style, family).max(1.0);
            let word_advance_em = match run.tab_leader {
                TabLeader::Dots => Some(0.28),
                TabLeader::Hyphens | TabLeader::MiddleDots => Some(0.33),
                TabLeader::Underline => Some(0.56),
                TabLeader::None | TabLeader::Equals => None,
            };
            let (leader_x, count, style) = if let Some(word_advance_em) = word_advance_em {
                let word_advance =
                    style.font_size_points() * word_advance_em * style.horizontal_scale();
                let word_advance = word_advance.max(1.0);
                let leader_x = ((cursor_x / word_advance).floor() + 1.0) * word_advance;
                let count = ((cursor_x + width - leader_x) / word_advance).floor() as usize;
                if count == 0 {
                    return;
                }
                let mut leader_style = style;
                let width_scale = (word_advance / leader_width).clamp(0.5, 2.0);
                leader_style.character_scaling_percent =
                    ((leader_style.character_scaling_percent.max(1) as f32 * width_scale)
                        .round()
                        .clamp(1.0, i32::MAX as f32)) as i32;
                (leader_x, count, leader_style)
            } else {
                (
                    cursor_x,
                    (width / leader_width).floor().max(1.0) as usize,
                    style,
                )
            };
            page.items.push(LayoutItem::Text(TextFragment {
                dynamic_field: None,
                text: leader.repeat(count.min(512)),
                x: leader_x,
                baseline_y: baseline_y + style.baseline_shift_points(),
                rotation: TextRotation::None,
                color,
                font_family: family,
                word_spacing: 0.0,
                style,
            }));
        }
    }
}

pub(in crate::layout) fn push_character_border(
    page: &mut LayoutPage,
    x: f32,
    baseline_y: f32,
    text_width: f32,
    style: &CharacterStyle,
    fallback_color: PdfColor,
    document: &Document,
) {
    let (stroke_width, color, line_style) =
        character_border_stroke(&style.border, fallback_color, document);
    let spacing = twips_to_points(style.border.spacing_twips.max(0));
    // LayoutItem::Line is center-stroked. Position each centerline so the
    // authored stroke occupies space outside the text's glyph box, matching
    // Word's filled-rectangle character-border geometry.
    let half_stroke = stroke_width * 0.5;
    let left = x - spacing - half_stroke;
    let right = x + text_width + spacing + half_stroke;
    let bottom = baseline_y - (style.font_size_points() * 0.28) - spacing - half_stroke;
    let top = baseline_y + (style.font_size_points() * 0.82) + spacing + half_stroke;

    if line_style == LineStyle::Solid {
        let outer_left = left - half_stroke;
        let outer_right = right + half_stroke;
        let outer_bottom = bottom - half_stroke;
        let outer_top = top + half_stroke;
        for (x, y, width, height) in [
            (
                outer_left,
                outer_top - stroke_width,
                outer_right - outer_left,
                stroke_width,
            ),
            (
                outer_left,
                outer_bottom,
                outer_right - outer_left,
                stroke_width,
            ),
            (
                outer_left,
                outer_bottom,
                stroke_width,
                outer_top - outer_bottom,
            ),
            (
                outer_right - stroke_width,
                outer_bottom,
                stroke_width,
                outer_top - outer_bottom,
            ),
        ] {
            page.items.push(LayoutItem::Highlight {
                x,
                y,
                width,
                height,
                color,
            });
        }
        return;
    }

    page.items.push(LayoutItem::Line {
        x1: left,
        y1: top,
        x2: right,
        y2: top,
        width: stroke_width,
        color,
        style: line_style,
    });
    page.items.push(LayoutItem::Line {
        x1: right,
        y1: top,
        x2: right,
        y2: bottom,
        width: stroke_width,
        color,
        style: line_style,
    });
    page.items.push(LayoutItem::Line {
        x1: right,
        y1: bottom,
        x2: left,
        y2: bottom,
        width: stroke_width,
        color,
        style: line_style,
    });
    page.items.push(LayoutItem::Line {
        x1: left,
        y1: bottom,
        x2: left,
        y2: top,
        width: stroke_width,
        color,
        style: line_style,
    });
}

pub(in crate::layout) fn character_border_stroke(
    border: &TableCellBorder,
    fallback_color: PdfColor,
    document: &Document,
) -> (f32, PdfColor, LineStyle) {
    let width = match border.style {
        BorderStyle::Hairline => 0.25,
        BorderStyle::Thick => twips_to_points(border.width_twips.max(1)).max(1.2),
        _ => twips_to_points(border.width_twips.max(1)).max(0.25),
    };
    let color = border
        .color_index
        .map(|index| color_for_index(document, index))
        .unwrap_or(fallback_color);
    let style = line_style_for_border_style(border.style);
    (width, color, style)
}

pub(in crate::layout) fn line_style_for_border_style(style: BorderStyle) -> LineStyle {
    match style {
        BorderStyle::Single | BorderStyle::Thick | BorderStyle::Hairline => LineStyle::Solid,
        BorderStyle::Double => LineStyle::Double,
        BorderStyle::Triple => LineStyle::Triple,
        BorderStyle::Dotted => LineStyle::Dotted,
        BorderStyle::Dashed => LineStyle::Dashed,
        BorderStyle::Wavy => LineStyle::Wavy,
        BorderStyle::Emboss => LineStyle::Emboss,
        BorderStyle::Engrave => LineStyle::Engrave,
        BorderStyle::ThinThick => LineStyle::ThinThick,
        BorderStyle::ThickThin => LineStyle::ThickThin,
        BorderStyle::ThinThickThin => LineStyle::ThinThickThin,
        BorderStyle::BorderArtScissors => LineStyle::Solid,
    }
}

pub(in crate::layout) fn line_cap_for_shape_cap(cap: StaticShapeLineCap) -> LineCap {
    match cap {
        StaticShapeLineCap::Flat => LineCap::Flat,
        StaticShapeLineCap::Round => LineCap::Round,
        StaticShapeLineCap::Square => LineCap::Square,
    }
}

pub(in crate::layout) fn line_join_for_shape_join(join: StaticShapeLineJoin) -> LineJoin {
    match join {
        StaticShapeLineJoin::Miter => LineJoin::Miter,
        StaticShapeLineJoin::Round => LineJoin::Round,
        StaticShapeLineJoin::Bevel => LineJoin::Bevel,
    }
}

pub(in crate::layout) fn justified_word_spacing(
    line: &Line,
    style: &ParagraphStyle,
    available_width: f32,
    is_last_line: bool,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> f32 {
    if !matches!(
        style.alignment,
        Alignment::Justified | Alignment::Distributed
    ) || is_last_line
        || line.width <= 0.0
    {
        return 0.0;
    }
    let total_space_count = line
        .runs
        .iter()
        .map(|run| regular_space_count(&run.text))
        .sum::<usize>();
    let (trailing_space_count, trailing_space_width) =
        trailing_regular_space_geometry(line, style, document, font_provider);
    let space_count = total_space_count.saturating_sub(trailing_space_count);
    if space_count == 0 {
        return 0.0;
    }
    ((available_width - (line.width - trailing_space_width)) / space_count as f32).max(0.0)
}

pub(in crate::layout) fn trailing_regular_space_geometry(
    line: &Line,
    paragraph_style: &ParagraphStyle,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> (usize, f32) {
    let mut count = 0usize;
    let mut width = 0.0;
    for run in line.runs.iter().rev() {
        let trailing_count = run.text.chars().rev().take_while(|ch| *ch == ' ').count();
        if trailing_count == 0 {
            break;
        }
        count += trailing_count;
        let trimmed_text = run.text.trim_end_matches(' ');
        let trimmed_width = if trimmed_text.is_empty() {
            0.0
        } else {
            let mut trimmed_run = run.clone();
            trimmed_run.text = trimmed_text.to_string();
            measure_flow_run(&trimmed_run, 0.0, paragraph_style, document, font_provider)
        };
        width += (run.width - trimmed_width).max(0.0);
        if !trimmed_text.is_empty() {
            break;
        }
    }
    (count, width)
}

pub(in crate::layout) fn distributed_character_spacing(
    line: &Line,
    style: &ParagraphStyle,
    available_width: f32,
    is_last_line: bool,
) -> f32 {
    if style.alignment != Alignment::Distributed || !is_last_line || line.width <= 0.0 {
        return 0.0;
    }
    let character_count = line
        .runs
        .iter()
        .filter(|run| {
            layout_advance(run.dynamic_field).is_none()
                && !is_passive_math_inline_lead_marker(run.dynamic_field)
                && run.text != "\t"
        })
        .map(|run| run.text.chars().filter(|ch| *ch != '\n').count())
        .sum::<usize>();
    let gap_count = character_count.saturating_sub(1);
    if gap_count == 0 {
        return 0.0;
    }
    ((available_width - line.width) / gap_count as f32).max(0.0)
}

pub(in crate::layout) fn regular_space_count(text: &str) -> usize {
    text.chars().filter(|ch| *ch == ' ').count()
}

pub(in crate::layout) fn display_text(text: &str, style: &CharacterStyle) -> String {
    if style.hidden {
        return String::new();
    }
    let text = text
        .chars()
        .filter(|ch| !is_zero_width_format_char(*ch))
        .collect::<String>();
    if style.all_caps || style.small_caps {
        text.chars().flat_map(char::to_uppercase).collect()
    } else {
        text
    }
}

pub(in crate::layout) fn font_family_for_run_text(
    document: &Document,
    style: &CharacterStyle,
    text: &str,
) -> PdfFontFamily {
    if is_passive_bullet_text(text) {
        return PdfFontFamily::Helvetica;
    }
    if is_passive_checkbox_text(text) {
        return PdfFontFamily::ZapfDingbats;
    }
    if is_passive_symbol_text(text) {
        return PdfFontFamily::Symbol;
    }

    font_family_for_style(document, style)
}

pub(in crate::layout) fn is_passive_bullet_text(text: &str) -> bool {
    let mut has_bullet = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            continue;
        }
        if ch == '\u{2022}' {
            has_bullet = true;
        } else {
            return false;
        }
    }
    has_bullet
}

pub(in crate::layout) fn font_family_for_style(
    document: &Document,
    style: &CharacterStyle,
) -> PdfFontFamily {
    let Some(font) = font_for_style(document, style) else {
        // Legacy Word-compatible RTF rendering uses Times New Roman when the
        // document provides no usable font table/default-font definition.
        return PdfFontFamily::Times;
    };
    passive_pdf_font_family_for_font(font)
}

pub(in crate::layout) fn font_for_style<'a>(
    document: &'a Document,
    style: &CharacterStyle,
) -> Option<&'a FontDef> {
    document
        .fonts
        .iter()
        .find(|font| font.index == style.font_index)
}

pub(crate) fn passive_pdf_font_family_for_font(font: &FontDef) -> PdfFontFamily {
    if let Some(family) = font_family_from_name(&font.name) {
        family
    } else if let Some(family) = font
        .alternate_name
        .as_deref()
        .and_then(font_family_from_name)
    {
        family
    } else if font.charset == Some(2) {
        PdfFontFamily::Symbol
    } else if font.pitch == FontPitch::Fixed {
        PdfFontFamily::Courier
    } else if font.family == FontFamilyHint::Modern {
        PdfFontFamily::Courier
    } else if font.family == FontFamilyHint::Roman {
        PdfFontFamily::Times
    } else if font.family == FontFamilyHint::Swiss {
        PdfFontFamily::Helvetica
    } else {
        PdfFontFamily::Helvetica
    }
}

pub(crate) fn passive_supplied_font_fallback_name(font: &FontDef) -> Option<&'static str> {
    if font.pitch == FontPitch::Fixed {
        Some("Courier New")
    } else {
        match font.family {
            FontFamilyHint::Roman => Some("Times New Roman"),
            FontFamilyHint::Modern | FontFamilyHint::Swiss => Some("Calibri"),
            _ => None,
        }
    }
}

pub(in crate::layout) fn passive_pdf_style_for_run(
    document: &Document,
    style: &CharacterStyle,
) -> CharacterStyle {
    let scale_percent = passive_source_font_width_scale_percent(document, style);
    if scale_percent == 100 {
        return style.clone();
    }
    let mut output = style.clone();
    let scaled = i64::from(output.character_scaling_percent.max(1))
        .saturating_mul(i64::from(scale_percent))
        / 100;
    output.character_scaling_percent = scaled.clamp(1, 600) as i32;
    output
}

pub(in crate::layout) fn passive_source_font_width_scale_percent(
    document: &Document,
    style: &CharacterStyle,
) -> i32 {
    let Some(font) = document
        .fonts
        .iter()
        .find(|font| font.index == style.font_index)
    else {
        return 100;
    };
    if is_passive_narrow_font_name(&font.name)
        || font
            .alternate_name
            .as_deref()
            .is_some_and(is_passive_narrow_font_name)
    {
        PASSIVE_NARROW_FONT_SCALE_PERCENT
    } else {
        100
    }
}

pub(in crate::layout) fn is_passive_narrow_font_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    font_name_contains_any(&lower, &["narrow", "condensed", "compressed"])
}

pub(in crate::layout) fn is_passive_checkbox_text(text: &str) -> bool {
    let mut has_checkbox = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            continue;
        }
        if matches!(
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
        ) {
            has_checkbox = true;
        } else {
            return false;
        }
    }
    has_checkbox
}

pub(in crate::layout) fn is_passive_symbol_text(text: &str) -> bool {
    let mut has_symbol = false;
    for ch in text.chars() {
        if is_symbol_fallback_required_char(ch) {
            has_symbol = true;
        } else {
            return false;
        }
    }
    has_symbol
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(in crate::layout) enum PassiveFontRunKind {
    Normal,
    Symbol,
    Checkbox,
}

impl PassiveFontRunKind {
    fn for_char(ch: char) -> Self {
        if is_passive_checkbox_char(ch) {
            Self::Checkbox
        } else if is_symbol_fallback_required_char(ch) {
            Self::Symbol
        } else {
            Self::Normal
        }
    }
}

pub(in crate::layout) fn is_passive_checkbox_char(ch: char) -> bool {
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

pub(in crate::layout) fn is_symbol_fallback_required_char(ch: char) -> bool {
    is_passive_symbol_fallback_char(ch) && !is_normal_text_preferred_symbol_overlap_char(ch)
}

pub(in crate::layout) fn is_normal_text_preferred_symbol_overlap_char(ch: char) -> bool {
    matches!(ch, '\u{2026}')
}

pub(in crate::layout) fn is_passive_symbol_fallback_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{0391}'
            | '\u{0392}'
            | '\u{0393}'
            | '\u{0394}'
            | '\u{0395}'
            | '\u{0396}'
            | '\u{0397}'
            | '\u{0398}'
            | '\u{0399}'
            | '\u{039a}'
            | '\u{039b}'
            | '\u{039c}'
            | '\u{039d}'
            | '\u{039e}'
            | '\u{039f}'
            | '\u{03a0}'
            | '\u{03a1}'
            | '\u{03a3}'
            | '\u{03a4}'
            | '\u{03a5}'
            | '\u{03a6}'
            | '\u{03a7}'
            | '\u{03a8}'
            | '\u{03a9}'
            | '\u{03b1}'
            | '\u{03b2}'
            | '\u{03b3}'
            | '\u{03b4}'
            | '\u{03b5}'
            | '\u{03b6}'
            | '\u{03b7}'
            | '\u{03b8}'
            | '\u{03b9}'
            | '\u{03ba}'
            | '\u{03bb}'
            | '\u{03bc}'
            | '\u{03bd}'
            | '\u{03be}'
            | '\u{03bf}'
            | '\u{03c0}'
            | '\u{03c1}'
            | '\u{03c3}'
            | '\u{03c4}'
            | '\u{03c5}'
            | '\u{03c6}'
            | '\u{03c7}'
            | '\u{03c8}'
            | '\u{03c9}'
            | '\u{03d5}'
            | '\u{03d6}'
            | '\u{03d1}'
            | '\u{03d2}'
            | '\u{00b0}'
            | '\u{00b1}'
            | '\u{00b5}'
            | '\u{00a9}'
            | '\u{00ae}'
            | '\u{00d7}'
            | '\u{00f7}'
            | '\u{0192}'
            | '\u{20ac}'
            | '\u{2026}'
            | '\u{2032}'
            | '\u{2033}'
            | '\u{2044}'
            | '\u{2111}'
            | '\u{211c}'
            | '\u{2118}'
            | '\u{2126}'
            | '\u{2122}'
            | '\u{2135}'
            | '\u{2190}'
            | '\u{2191}'
            | '\u{2192}'
            | '\u{2193}'
            | '\u{2194}'
            | '\u{21b5}'
            | '\u{21d0}'
            | '\u{21d1}'
            | '\u{21d2}'
            | '\u{21d3}'
            | '\u{21d4}'
            | '\u{00ac}'
            | '\u{2200}'
            | '\u{2202}'
            | '\u{2203}'
            | '\u{2205}'
            | '\u{2206}'
            | '\u{2207}'
            | '\u{2208}'
            | '\u{2209}'
            | '\u{220b}'
            | '\u{220f}'
            | '\u{2211}'
            | '\u{2212}'
            | '\u{2215}'
            | '\u{2217}'
            | '\u{221a}'
            | '\u{221d}'
            | '\u{221e}'
            | '\u{222b}'
            | '\u{2220}'
            | '\u{2227}'
            | '\u{2228}'
            | '\u{2229}'
            | '\u{222a}'
            | '\u{223c}'
            | '\u{2234}'
            | '\u{2248}'
            | '\u{2260}'
            | '\u{2261}'
            | '\u{2264}'
            | '\u{2265}'
            | '\u{22a5}'
            | '\u{2282}'
            | '\u{2283}'
            | '\u{2284}'
            | '\u{2286}'
            | '\u{2287}'
            | '\u{2295}'
            | '\u{2297}'
            | '\u{22c5}'
            | '\u{2320}'
            | '\u{2321}'
            | '\u{2329}'
            | '\u{232a}'
            | '\u{239b}'
            | '\u{239c}'
            | '\u{239d}'
            | '\u{239e}'
            | '\u{239f}'
            | '\u{23a0}'
            | '\u{23a1}'
            | '\u{23a2}'
            | '\u{23a3}'
            | '\u{23a4}'
            | '\u{23a5}'
            | '\u{23a6}'
            | '\u{23a7}'
            | '\u{23a8}'
            | '\u{23a9}'
            | '\u{23aa}'
            | '\u{23ab}'
            | '\u{23ac}'
            | '\u{23ad}'
            | '\u{23ae}'
            | '\u{23af}'
            | '\u{23d0}'
            | '\u{25ca}'
            | '\u{2660}'
            | '\u{2663}'
            | '\u{2665}'
            | '\u{2666}'
    )
}

pub(in crate::layout) fn font_family_from_name(name: &str) -> Option<PdfFontFamily> {
    let name = name.to_ascii_lowercase();
    if font_name_contains_any(
        &name,
        &[
            "courier",
            "consolas",
            "monaco",
            "lucida console",
            "aptos mono",
            "cascadia mono",
            "cascadia code",
            "monospace",
        ],
    ) {
        Some(PdfFontFamily::Courier)
    } else if font_name_contains_any(
        &name,
        &["zapfdingbats", "zapf dingbats", "wingdings", "webdings"],
    ) {
        Some(PdfFontFamily::ZapfDingbats)
    } else if is_pdf_symbol_font_name(&name) {
        Some(PdfFontFamily::Symbol)
    } else if is_sans_serif_font_name(&name) {
        Some(PdfFontFamily::Helvetica)
    } else if font_name_contains_any(
        &name,
        &[
            "times",
            "roman",
            "serif",
            "cambria",
            "georgia",
            "garamond",
            "palatino",
            "book antiqua",
            "constantia",
        ],
    ) {
        Some(PdfFontFamily::Times)
    } else if font_name_contains_any(
        &name,
        &[
            "arial",
            "helvetica",
            "calibri",
            "aptos",
            "segoe ui",
            "tahoma",
            "verdana",
            "trebuchet",
            "franklin gothic",
        ],
    ) {
        Some(PdfFontFamily::Helvetica)
    } else {
        None
    }
}

pub(in crate::layout) fn font_name_contains_any(name: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| name.contains(needle))
}

pub(in crate::layout) fn is_sans_serif_font_name(name: &str) -> bool {
    font_name_contains_any(name, &["sans serif", "sans-serif", "microsoft sans serif"])
}

pub(in crate::layout) fn is_pdf_symbol_font_name(name: &str) -> bool {
    matches!(
        name.trim(),
        "symbol" | "symbol mt" | "symbolmt" | "standard symbols l"
    )
}

pub(in crate::layout) fn style_color(document: &Document, style: &CharacterStyle) -> PdfColor {
    color_for_index(document, style.color_index)
}

pub(in crate::layout) fn color_for_index(document: &Document, index: usize) -> PdfColor {
    document
        .colors
        .get(index)
        .map(|color| PdfColor {
            red: color.red as f32 / 255.0,
            green: color.green as f32 / 255.0,
            blue: color.blue as f32 / 255.0,
        })
        .unwrap_or_default()
}

pub(in crate::layout) fn character_shading_color(
    document: &Document,
    index: usize,
    style: &CharacterStyle,
) -> PdfColor {
    let factor = (style.highlight_shading_basis_points as f32 / 10_000.0).clamp(0.0, 1.0);
    if index == 0 {
        let gray = 1.0 - factor;
        return PdfColor {
            red: gray,
            green: gray,
            blue: gray,
        };
    }
    let color = color_for_index(document, index);
    PdfColor {
        red: color.red * factor,
        green: color.green * factor,
        blue: color.blue * factor,
    }
}

pub(in crate::layout) fn passive_form_field_shading_color() -> PdfColor {
    PdfColor {
        red: 0.82,
        green: 0.82,
        blue: 0.82,
    }
}

pub(in crate::layout) fn passive_form_field_shading_horizontal_bounds(
    x: f32,
    width: f32,
    font_size: f32,
) -> (f32, f32) {
    let padding = (font_size * 0.16).clamp(1.0, 3.0);
    let shaded_x = (x - padding).max(0.0);
    let left_padding = x - shaded_x;
    (shaded_x, width.max(0.0) + left_padding + padding)
}

pub(in crate::layout) fn shading_color(
    document: &Document,
    index: usize,
    basis_points: i32,
) -> PdfColor {
    let color = color_for_index(document, index);
    let factor = (basis_points as f32 / 10_000.0).clamp(0.0, 1.0);
    PdfColor {
        red: 1.0 - ((1.0 - color.red) * factor),
        green: 1.0 - ((1.0 - color.green) * factor),
        blue: 1.0 - ((1.0 - color.blue) * factor),
    }
}

pub(in crate::layout) fn shading_is_visible(
    document: &Document,
    background_color_index: Option<usize>,
    pattern: ShadingPattern,
) -> bool {
    background_color_index.is_some_and(|index| index > 0 && index < document.colors.len())
        || pattern
            .foreground_color_index()
            .is_some_and(|index| index < document.colors.len())
}

pub(in crate::layout) fn shading_fill_color(
    document: &Document,
    background_color_index: Option<usize>,
    basis_points: i32,
    pattern: ShadingPattern,
) -> PdfColor {
    let white = PdfColor {
        red: 1.0,
        green: 1.0,
        blue: 1.0,
    };
    let background = background_color_index
        .filter(|index| *index > 0 && *index < document.colors.len())
        .map(|index| color_for_index(document, index))
        .unwrap_or(white);

    if pattern.without_foreground_color() != ShadingPattern::None {
        return background;
    }

    if let Some(foreground_color_index) = pattern
        .foreground_color_index()
        .filter(|index| *index < document.colors.len())
    {
        let foreground = color_for_index(document, foreground_color_index);
        let factor = (basis_points as f32 / 10_000.0).clamp(0.0, 1.0);
        return PdfColor {
            red: background.red + ((foreground.red - background.red) * factor),
            green: background.green + ((foreground.green - background.green) * factor),
            blue: background.blue + ((foreground.blue - background.blue) * factor),
        };
    }

    background_color_index
        .filter(|index| *index > 0 && *index < document.colors.len())
        .map(|index| shading_color(document, index, basis_points))
        .unwrap_or(white)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn push_shading_rect(
    pages: &mut [LayoutPage],
    document: &Document,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    background_color_index: Option<usize>,
    basis_points: i32,
    pattern: ShadingPattern,
) {
    let fill_color = shading_fill_color(document, background_color_index, basis_points, pattern);
    let line_color = match pattern.foreground_color_index() {
        Some(index) => document
            .colors
            .get(index)
            .map(|_| color_for_index(document, index)),
        None => Some(PdfColor::default()),
    };
    let page = pages.last_mut().expect("layout always has a page");
    page.items.push(LayoutItem::Highlight {
        x,
        y,
        width,
        height,
        color: fill_color,
    });
    if let Some(line_color) = line_color {
        push_shading_pattern_lines(page, x, y, width, height, line_color, pattern);
    }
}

pub(in crate::layout) fn push_shading_pattern_lines(
    page: &mut LayoutPage,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: PdfColor,
    pattern: ShadingPattern,
) {
    let pattern = pattern.without_foreground_color();
    if pattern == ShadingPattern::None || width <= 0.5 || height <= 0.5 {
        return;
    }

    let spacing = shading_pattern_spacing(pattern);
    let stroke_width = 0.24;
    match pattern {
        ShadingPattern::Horizontal | ShadingPattern::DarkHorizontal => {
            push_horizontal_shading_lines(page, x, y, width, height, spacing, stroke_width, color);
        }
        ShadingPattern::Vertical | ShadingPattern::DarkVertical => {
            push_vertical_shading_lines(page, x, y, width, height, spacing, stroke_width, color);
        }
        ShadingPattern::ForwardDiagonal | ShadingPattern::DarkForwardDiagonal => {
            push_forward_diagonal_shading_lines(
                page,
                x,
                y,
                width,
                height,
                spacing,
                stroke_width,
                color,
            );
        }
        ShadingPattern::BackwardDiagonal | ShadingPattern::DarkBackwardDiagonal => {
            push_backward_diagonal_shading_lines(
                page,
                x,
                y,
                width,
                height,
                spacing,
                stroke_width,
                color,
            );
        }
        ShadingPattern::Cross | ShadingPattern::DarkCross => {
            push_horizontal_shading_lines(page, x, y, width, height, spacing, stroke_width, color);
            push_vertical_shading_lines(page, x, y, width, height, spacing, stroke_width, color);
        }
        ShadingPattern::DiagonalCross | ShadingPattern::DarkDiagonalCross => {
            push_forward_diagonal_shading_lines(
                page,
                x,
                y,
                width,
                height,
                spacing,
                stroke_width,
                color,
            );
            push_backward_diagonal_shading_lines(
                page,
                x,
                y,
                width,
                height,
                spacing,
                stroke_width,
                color,
            );
        }
        ShadingPattern::None | ShadingPattern::VerticalGradient => {}
        _ => {}
    }
}

pub(in crate::layout) fn push_shape_fill_pattern_lines(
    page: &mut LayoutPage,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    fill_color: Option<crate::model::Color>,
    pattern: ShadingPattern,
) {
    if pattern == ShadingPattern::None || fill_color.is_none() {
        return;
    }
    let color = fill_color.expect("checked above");
    push_shading_pattern_lines(
        page,
        x,
        y,
        width,
        height,
        PdfColor {
            red: color.red as f32 / 255.0,
            green: color.green as f32 / 255.0,
            blue: color.blue as f32 / 255.0,
        },
        pattern,
    );
}

pub(in crate::layout) fn push_passive_vertical_gradient_bands(
    page: &mut LayoutPage,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    start_color: PdfColor,
    end_color: Option<PdfColor>,
) {
    if width <= 0.5 || height <= 0.5 {
        return;
    }
    const BANDS: usize = 16;
    let band_height = height / BANDS as f32;
    for index in 0..BANDS {
        let t = if BANDS <= 1 {
            0.0
        } else {
            index as f32 / (BANDS - 1) as f32
        };
        let end_color = end_color.unwrap_or(PdfColor {
            red: 1.0,
            green: 1.0,
            blue: 1.0,
        });
        let lightened = PdfColor {
            red: start_color.red + ((end_color.red - start_color.red) * t),
            green: start_color.green + ((end_color.green - start_color.green) * t),
            blue: start_color.blue + ((end_color.blue - start_color.blue) * t),
        };
        page.items.push(LayoutItem::Highlight {
            x,
            y: y + (band_height * index as f32),
            width,
            height: if index + 1 == BANDS {
                height - (band_height * index as f32)
            } else {
                band_height + 0.1
            },
            color: lightened,
        });
    }
}

pub(in crate::layout) fn shading_pattern_spacing(pattern: ShadingPattern) -> f32 {
    match pattern.without_foreground_color() {
        ShadingPattern::DarkHorizontal
        | ShadingPattern::DarkVertical
        | ShadingPattern::DarkForwardDiagonal
        | ShadingPattern::DarkBackwardDiagonal
        | ShadingPattern::DarkCross
        | ShadingPattern::DarkDiagonalCross => 0.6,
        ShadingPattern::None
        | ShadingPattern::Horizontal
        | ShadingPattern::Vertical
        | ShadingPattern::ForwardDiagonal
        | ShadingPattern::BackwardDiagonal
        | ShadingPattern::Cross
        | ShadingPattern::DiagonalCross
        | ShadingPattern::VerticalGradient => 0.96,
        _ => 0.96,
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn push_horizontal_shading_lines(
    page: &mut LayoutPage,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    spacing: f32,
    stroke_width: f32,
    color: PdfColor,
) {
    let mut cursor = y + spacing;
    let max_y = y + height;
    while cursor < max_y {
        push_shading_line(page, x, cursor, x + width, cursor, stroke_width, color);
        cursor += spacing;
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn push_vertical_shading_lines(
    page: &mut LayoutPage,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    spacing: f32,
    stroke_width: f32,
    color: PdfColor,
) {
    let mut cursor = x + spacing;
    let max_x = x + width;
    while cursor < max_x {
        push_shading_line(page, cursor, y, cursor, y + height, stroke_width, color);
        cursor += spacing;
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn push_forward_diagonal_shading_lines(
    page: &mut LayoutPage,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    spacing: f32,
    stroke_width: f32,
    color: PdfColor,
) {
    let mut offset = spacing;
    while offset < width {
        let length = (width - offset).min(height);
        push_shading_line(
            page,
            x + offset,
            y,
            x + offset + length,
            y + length,
            stroke_width,
            color,
        );
        offset += spacing;
    }

    let mut offset = spacing;
    while offset < height {
        let length = width.min(height - offset);
        push_shading_line(
            page,
            x,
            y + offset,
            x + length,
            y + offset + length,
            stroke_width,
            color,
        );
        offset += spacing;
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn push_backward_diagonal_shading_lines(
    page: &mut LayoutPage,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    spacing: f32,
    stroke_width: f32,
    color: PdfColor,
) {
    let mut offset = spacing;
    while offset < height {
        let length = width.min(offset);
        push_shading_line(
            page,
            x,
            y + offset,
            x + length,
            y + offset - length,
            stroke_width,
            color,
        );
        offset += spacing;
    }

    let mut offset = spacing;
    while offset < width {
        let length = (width - offset).min(height);
        push_shading_line(
            page,
            x + offset,
            y + height,
            x + offset + length,
            y + height - length,
            stroke_width,
            color,
        );
        offset += spacing;
    }
}

pub(in crate::layout) fn push_shading_line(
    page: &mut LayoutPage,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    width: f32,
    color: PdfColor,
) {
    page.items.push(LayoutItem::Line {
        x1,
        y1,
        x2,
        y2,
        width,
        color,
        style: LineStyle::Solid,
    });
}

pub(in crate::layout) fn aligned_x(
    left: f32,
    content_width: f32,
    line_width: f32,
    style: &ParagraphStyle,
    is_first_line: bool,
) -> f32 {
    let line_left_indent = paragraph_line_left_indent_twips(style, is_first_line);
    let left = left + twips_to_points(line_left_indent);
    let available = paragraph_line_width(content_width, style, is_first_line);
    match style.alignment {
        Alignment::Left | Alignment::Justified | Alignment::Distributed => left,
        Alignment::Center => left + ((available - line_width).max(0.0) / 2.0),
        Alignment::Right => left + (available - line_width).max(0.0),
    }
}

pub(in crate::layout) fn paragraph_line_width(
    content_width: f32,
    style: &ParagraphStyle,
    is_first_line: bool,
) -> f32 {
    let line_left_indent = paragraph_line_left_indent_twips(style, is_first_line);
    let no_wrap_emergency_inset = if style.no_wrap {
        style.no_wrap_emergency_right_inset_twips.clamp(0, 10_000)
    } else {
        0
    };
    (content_width
        - twips_to_points(line_left_indent + style.right_indent_twips + no_wrap_emergency_inset))
    .max(12.0)
}

pub(in crate::layout) fn paragraph_line_left_indent_twips(
    style: &ParagraphStyle,
    is_first_line: bool,
) -> i32 {
    style.left_indent_twips
        + if is_first_line {
            style.first_line_indent_twips
        } else {
            0
        }
}

pub fn measure_text(text: &str, style: &CharacterStyle) -> f32 {
    measure_text_with_family(text, style, PdfFontFamily::Helvetica)
}

pub(in crate::layout) fn measure_text_with_family(
    text: &str,
    style: &CharacterStyle,
    family: PdfFontFamily,
) -> f32 {
    let size = style.font_size_points();
    let base_width = text
        .chars()
        .map(|ch| base14_char_width_points(ch, size, family, style))
        .sum::<f32>();
    (base_width + passive_kerning_width(text, style, family)) * style.horizontal_scale()
        + character_spacing_width(text, style)
}

pub(in crate::layout) fn measure_text_with_document_font(
    text: &str,
    style: &CharacterStyle,
    family: PdfFontFamily,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> f32 {
    let size = style.font_size_points();
    let mut width = 0.0;
    let source_font = font_for_style(document, style);
    for ch in text.chars() {
        if is_zero_width_format_char(ch) {
            continue;
        }
        if let Some(source_width) =
            source_word_compatible_advance_points(source_font, style, ch, size, font_provider)
        {
            width += source_width;
            continue;
        }
        let font_width = font_provider
            .zip(source_font)
            .filter(|_| !matches!(family, PdfFontFamily::Symbol | PdfFontFamily::ZapfDingbats))
            .and_then(|(provider, font)| supplied_font_glyph_metrics(provider, font, style, ch))
            .map(|metrics| metrics.advance_points(size));
        width += font_width.unwrap_or_else(|| base14_char_width_points(ch, size, family, style));
    }
    let kerning_width =
        passive_kerning_width_with_document_font(text, style, family, source_font, font_provider);
    (width + kerning_width) * style.horizontal_scale() + character_spacing_width(text, style)
}

pub(crate) fn source_word_compatible_advance_points(
    font: Option<&FontDef>,
    style: &CharacterStyle,
    ch: char,
    size: f32,
    font_provider: Option<&FontProvider>,
) -> Option<f32> {
    source_calibri_advance_points(font, style, ch, size, font_provider)
        .or_else(|| source_cambria_advance_points(font, style, ch, size, font_provider))
        .or_else(|| source_aptos_advance_points(font, style, ch, size, font_provider))
        .or_else(|| source_dingbat_advance_points(font, ch, size))
        .or_else(|| {
            (style.passive_math.enabled && style.passive_math.fraction_id.is_none())
                .then(|| source_cambria_math_advance_points(font, ch, size))
                .flatten()
        })
}

pub(in crate::layout) fn source_calibri_advance_points(
    font: Option<&FontDef>,
    style: &CharacterStyle,
    ch: char,
    size: f32,
    font_provider: Option<&FontProvider>,
) -> Option<f32> {
    if !font_uses_source_family(font?, font_provider, &["calibri", "carlito"]) {
        return None;
    }
    if !style.bold && !style.italic {
        let units = match ch {
            '\u{00b3}' => 685,
            '\u{00d2}' | '\u{00d3}' => 1356,
            '\u{00e9}' => 1019,
            '\u{201a}' => 511,
            '\u{201c}' | '\u{201d}' => 857,
            '\u{2502}' => 1024,
            _ => 0,
        };
        if units != 0 {
            return Some(size * units as f32 / 2_048.0);
        }
    }
    let index = usize::try_from(u32::from(ch).checked_sub(32)?).ok()?;
    const REGULAR: [u16; 95] = [
        463, 667, 821, 1020, 1038, 1464, 1397, 452, 621, 621, 1020, 1020, 511, 627, 517, 791, 1038,
        1038, 1038, 1038, 1038, 1038, 1038, 1038, 1038, 1038, 548, 548, 1020, 1020, 1020, 949,
        1831, 1185, 1114, 1092, 1260, 1000, 941, 1292, 1276, 516, 653, 1064, 861, 1751, 1322, 1356,
        1058, 1378, 1112, 941, 998, 1314, 1162, 1822, 1063, 998, 959, 628, 791, 628, 1020, 1020,
        596, 981, 1076, 866, 1076, 1019, 625, 964, 1076, 470, 490, 931, 470, 1636, 1076, 1080,
        1076, 1076, 714, 801, 686, 1076, 925, 1464, 887, 927, 809, 644, 943, 644, 1020,
    ];
    const BOLD: [u16; 95] = [
        463, 667, 898, 1020, 1038, 1493, 1443, 478, 638, 638, 1020, 1020, 528, 627, 547, 880, 1038,
        1038, 1038, 1038, 1038, 1038, 1038, 1038, 1038, 1038, 565, 565, 1020, 1020, 1020, 949,
        1840, 1241, 1148, 1084, 1291, 999, 940, 1305, 1292, 546, 678, 1120, 866, 1790, 1349, 1385,
        1090, 1405, 1153, 968, 1014, 1337, 1211, 1856, 1128, 1064, 979, 665, 880, 665, 1020, 1020,
        615, 1011, 1099, 857, 1099, 1031, 648, 971, 1099, 503, 523, 983, 503, 1666, 1099, 1101,
        1099, 1099, 728, 817, 710, 1099, 969, 1526, 941, 970, 814, 704, 973, 704, 1020,
    ];
    const ITALIC: [u16; 95] = [
        463, 667, 821, 1020, 1038, 1464, 1397, 452, 621, 621, 1020, 1020, 511, 627, 517, 794, 1038,
        1038, 1038, 1038, 1038, 1038, 1038, 1038, 1038, 1038, 548, 548, 1020, 1020, 1020, 949,
        1831, 1185, 1114, 1070, 1260, 1000, 941, 1292, 1276, 516, 653, 1064, 861, 1751, 1320, 1340,
        1058, 1360, 1112, 926, 998, 1314, 1162, 1823, 1063, 998, 959, 628, 787, 628, 1020, 1020,
        596, 1053, 1053, 852, 1053, 978, 625, 1053, 1053, 470, 490, 931, 470, 1620, 1053, 1051,
        1053, 1053, 702, 797, 686, 1053, 913, 1464, 887, 916, 809, 644, 943, 644, 1020,
    ];
    const BOLD_ITALIC: [u16; 95] = [
        463, 667, 898, 1020, 1038, 1493, 1443, 478, 638, 638, 1020, 1020, 528, 627, 547, 889, 1038,
        1038, 1038, 1038, 1038, 1038, 1038, 1038, 1038, 1038, 565, 565, 1020, 1020, 1020, 949,
        1840, 1241, 1148, 1062, 1291, 999, 940, 1305, 1292, 546, 678, 1120, 866, 1790, 1344, 1369,
        1090, 1387, 1153, 953, 1014, 1337, 1211, 1857, 1128, 1064, 979, 665, 870, 665, 1020, 1020,
        615, 1081, 1081, 843, 1081, 1006, 648, 1081, 1080, 503, 523, 983, 503, 1646, 1080, 1080,
        1081, 1081, 721, 807, 710, 1080, 961, 1526, 941, 963, 814, 704, 973, 704, 1020,
    ];
    let units = match (style.bold, style.italic) {
        (false, false) => *REGULAR.get(index)?,
        (true, false) => *BOLD.get(index)?,
        (false, true) => *ITALIC.get(index)?,
        (true, true) => *BOLD_ITALIC.get(index)?,
    };
    Some(size * f32::from(units) / 2_048.0)
}

pub(in crate::layout) fn source_cambria_advance_points(
    font: Option<&FontDef>,
    style: &CharacterStyle,
    ch: char,
    size: f32,
    font_provider: Option<&FontProvider>,
) -> Option<f32> {
    if style.bold || style.italic || !font_uses_source_family(font?, font_provider, &["cambria"]) {
        return None;
    }
    // Bounded regular-face hmtx values independently present in the registered
    // Word font-family-hint reference. No proprietary font bytes are retained.
    let units = match ch {
        ' ' => 451,
        '.' => 420,
        'R' => 1272,
        'a' => 1000,
        'e' => 999,
        'f' => 620,
        'i' => 569,
        'l' => 556,
        'm' => 1704,
        'n' => 1143,
        'o' => 1087,
        't' => 693,
        'x' => 990,
        'y' => 1032,
        _ => return None,
    };
    Some(size * units as f32 / 2_048.0)
}

pub(in crate::layout) fn source_aptos_advance_points(
    font: Option<&FontDef>,
    style: &CharacterStyle,
    ch: char,
    size: f32,
    font_provider: Option<&FontProvider>,
) -> Option<f32> {
    if style.bold || style.italic || !font_uses_source_family(font?, font_provider, &["aptos"]) {
        return None;
    }
    // Bounded regular-face hmtx values independently present in the registered
    // Word font-family-hint reference. No proprietary font bytes are retained.
    let units = match ch {
        ' ' => 416,
        '.' => 585,
        'T' => 980,
        'b' => 1149,
        'd' => 1148,
        'e' => 1079,
        'h' => 1129,
        'm' => 1747,
        'o' => 1130,
        't' => 662,
        'x' => 906,
        'y' => 926,
        _ => return None,
    };
    Some(size * units as f32 / 2_048.0)
}

pub(in crate::layout) fn font_uses_source_family(
    font: &FontDef,
    provider: Option<&FontProvider>,
    families: &[&str],
) -> bool {
    let matches_family = |name: &str| {
        let name = name.trim().to_ascii_lowercase();
        families.contains(&name.as_str())
    };
    matches_family(&font.name)
        || (font.alternate_name.as_deref().is_some_and(matches_family)
            && !provider.is_some_and(|provider| provider.has_exact_asset_for_family(&font.name)))
}

pub(crate) fn source_dingbat_advance_points(
    font: Option<&FontDef>,
    ch: char,
    size: f32,
) -> Option<f32> {
    let name = font?.name.to_ascii_lowercase();
    let em = if name.contains("segoe ui symbol") {
        match ch {
            '\u{25a1}' | '\u{2610}' | '\u{2611}' | '\u{2612}' => 0.86,
            '\u{2713}' | '\u{2714}' => 0.75,
            '\u{2717}' => 0.83,
            ch if ch.is_whitespace() => 0.27,
            _ => return None,
        }
    } else if name.contains("wingdings 3") || name.contains("wingdings3") {
        match ch {
            '\u{2190}' | '\u{2192}' => 0.89,
            '\u{2191}' | '\u{2193}' => 0.602,
            ch if ch.is_whitespace() => 1.0,
            _ => return None,
        }
    } else if name.contains("wingdings 2") || name.contains("wingdings2") {
        match ch {
            '\u{2717}' => 0.645,
            '\u{2713}' | '\u{2714}' => 0.766,
            '\u{25c9}' | '\u{2610}' | '\u{2611}' | '\u{2612}' | '\u{1f5f5}' | '\u{1f5f7}' => 0.892,
            ch if ch.is_whitespace() => 1.0,
            _ => return None,
        }
    } else if name.contains("wingdings") {
        match ch {
            '\u{25c9}' | '\u{2610}' | '\u{2611}' | '\u{2612}' => 0.892,
            '\u{2713}' | '\u{2714}' => 0.786,
            '\u{2717}' => 0.635,
            ch if ch.is_whitespace() => 1.0,
            _ => return None,
        }
    } else if name.contains("webdings") {
        match ch {
            '\u{25a1}' | '\u{2610}' | '\u{2611}' | '\u{2612}' | '\u{2713}' | '\u{2714}'
            | '\u{1f4e5}' => 1.0,
            ch if ch.is_whitespace() => 0.5,
            _ => return None,
        }
    } else {
        return None;
    };
    Some(size * em)
}

pub(in crate::layout) fn source_cambria_math_advance_points(
    font: Option<&FontDef>,
    ch: char,
    size: f32,
) -> Option<f32> {
    let name = font?.name.to_ascii_lowercase();
    if !name.contains("cambria math") {
        return None;
    }

    // Bounded horizontal metrics recovered from Word 16.0's embedded Cambria Math
    // subsets. Keeping only the characters independently present in the registered
    // Office Math references avoids host font discovery and proprietary font bytes.
    let units = match ch {
        ' ' | '\u{00a0}' => 451,
        '+' | '=' | '\u{2212}' => 1530,
        '.' => 420,
        '/' => 1004,
        '1' | '2' => 1134,
        ':' => 540,
        '?' => 865,
        '(' | ')' => 850,
        'A' => 1276,
        'B' => 1252,
        'D' => 1355,
        'E' => 1178,
        'F' | 'L' => 1099,
        'G' => 1251,
        'H' => 1406,
        'I' => 664,
        'M' => 1670,
        'N' => 1394,
        'O' => 1338,
        'P' => 1163,
        'R' => 1272,
        'S' => 1016,
        'T' => 1214,
        'U' => 1327,
        'V' => 1236,
        'X' => 1170,
        'Y' => 1168,
        'a' => 1000,
        'b' => 1121,
        'c' => 903,
        'd' => 1136,
        'e' => 999,
        'h' => 1130,
        'i' => 569,
        'k' => 1074,
        'l' => 556,
        'm' => 1704,
        'n' => 1143,
        'o' => 1087,
        'p' => 1139,
        'q' => 1120,
        'r' => 847,
        's' => 880,
        't' => 693,
        'v' => 1032,
        'x' => 990,
        'z' => 931,
        '\u{2211}' => 1451,
        '\u{220f}' => 1663,
        '\u{221a}' => 1345,
        _ => return None,
    };
    Some(size * units as f32 / 2048.0)
}

pub(in crate::layout) fn supplied_font_glyph_metrics(
    provider: &FontProvider,
    font: &FontDef,
    style: &CharacterStyle,
    ch: char,
) -> Option<crate::fonts::FontGlyphMetrics> {
    let asset_style = FontAssetStyle {
        bold: style.bold,
        italic: style.italic,
    };
    provider
        .exact_glyph_metrics_for_char_with_style(&font.name, asset_style, ch)
        .or_else(|| {
            font.alternate_name.as_deref().and_then(|alternate| {
                provider.exact_glyph_metrics_for_char_with_style(alternate, asset_style, ch)
            })
        })
        .or_else(|| {
            passive_supplied_font_fallback_name(font).and_then(|fallback| {
                provider.exact_glyph_metrics_for_char_with_style(fallback, asset_style, ch)
            })
        })
        .or_else(|| provider.glyph_metrics_for_char_with_style(&font.name, asset_style, ch))
        .or_else(|| {
            font.alternate_name.as_deref().and_then(|alternate| {
                provider.glyph_metrics_for_char_with_style(alternate, asset_style, ch)
            })
        })
}

pub(in crate::layout) fn base14_char_width_points(
    ch: char,
    size: f32,
    family: PdfFontFamily,
    style: &CharacterStyle,
) -> f32 {
    if is_zero_width_format_char(ch) {
        return 0.0;
    }

    let units = if ch == '\u{00b7}' {
        match family {
            PdfFontFamily::Courier => 600,
            PdfFontFamily::Helvetica => 278,
            PdfFontFamily::Times => 250,
            PdfFontFamily::Symbol | PdfFontFamily::ZapfDingbats => symbol_width_units(ch),
        }
    } else {
        match family {
            PdfFontFamily::Courier => 600,
            PdfFontFamily::Helvetica if style.bold => helvetica_bold_width_units(ch),
            PdfFontFamily::Helvetica => helvetica_width_units(ch),
            PdfFontFamily::Times if style.bold => times_bold_width_units(ch),
            PdfFontFamily::Times if style.italic => times_italic_width_units(ch),
            PdfFontFamily::Times => times_width_units(ch),
            PdfFontFamily::Symbol | PdfFontFamily::ZapfDingbats => symbol_width_units(ch),
        }
    };
    size * units as f32 / 1000.0
}

pub(in crate::layout) fn helvetica_width_units(ch: char) -> u16 {
    match ch {
        ' ' | '\u{00a0}' | '!' | ',' | '.' | ':' | ';' | '[' | '\\' | ']' => 278,
        '"' => 355,
        '#' | '$' | '0'..='9' | '<' | '=' | '>' | '_' | '?' => 556,
        '%' => 889,
        '&' | 'A' | 'B' | 'K' | 'R' | 'S' | 'X' | 'Y' => 667,
        '\'' => 191,
        '(' | ')' | '`' | '-' => 333,
        '*' => 389,
        '+' | '~' => 584,
        '/' | 'I' | 'f' | 't' => 278,
        '@' => 1015,
        'C' | 'H' | 'N' | 'U' | 'w' => 722,
        'D' | 'G' | 'O' => 778,
        'E' | 'P' | 'V' => 667,
        'F' | 'Z' => 611,
        'J' | 'a' | 'b' | 'd' | 'e' | 'g' | 'n' | 'o' | 'p' | 'q' => 556,
        'L' => 556,
        'M' | 'm' => 833,
        'Q' => 778,
        'T' => 611,
        'W' => 944,
        '^' => 469,
        'c' | 'k' | 's' | 'v' | 'x' | 'y' | 'z' => 500,
        'h' | 'u' => 556,
        'i' | 'l' => 222,
        'j' => 222,
        'r' | '{' | '}' => 333,
        '|' => 260,
        '\u{2013}' => 556,
        '\u{2014}' => 1000,
        '\u{2018}' | '\u{2019}' => 222,
        '\u{201c}' | '\u{201d}' => 333,
        '\u{2022}' => 350,
        '\u{2026}' => 1000,
        _ if ch.is_whitespace() => 278,
        _ => 556,
    }
}

pub(in crate::layout) fn helvetica_bold_width_units(ch: char) -> u16 {
    match ch {
        ' ' | '\u{00a0}' | ',' | '.' | 'I' | 'i' | 'j' | 'l' | '\\' => 278,
        '!' | ':' | ';' | '(' | ')' | '[' | ']' | '`' | '-' | 'f' | 't' => 333,
        '"' => 474,
        '#'
        | '$'
        | '0'..='9'
        | '<'
        | '='
        | '>'
        | '_'
        | 'J'
        | 'c'
        | 'e'
        | 'k'
        | 's'
        | 'u'
        | 'v'
        | 'x' => 556,
        '%' => 889,
        '&' | 'A' | 'B' | 'C' | 'D' | 'H' | 'K' | 'N' | 'R' | 'S' | 'X' | 'Y' => 722,
        '\'' => 238,
        '*' => 389,
        '+' | '^' | '~' => 584,
        '/' => 278,
        '?' | 'F' | 'L' | 'P' | 'Z' | 'b' | 'd' | 'g' | 'h' | 'n' | 'o' | 'p' | 'q' => 611,
        '@' => 975,
        'E' | 'T' => 667,
        'G' | 'O' | 'Q' | 'w' => 778,
        'M' | 'm' => 833,
        'U' => 722,
        'V' => 667,
        'W' => 944,
        'a' => 556,
        'r' | '{' | '}' => 389,
        'y' => 556,
        'z' => 500,
        '|' => 280,
        '\u{2013}' => 556,
        '\u{2014}' => 1000,
        '\u{2018}' | '\u{2019}' => 278,
        '\u{201c}' | '\u{201d}' => 500,
        '\u{2022}' => 350,
        '\u{2026}' => 1000,
        _ if ch.is_whitespace() => 278,
        _ => 556,
    }
}

pub(in crate::layout) fn times_width_units(ch: char) -> u16 {
    match ch {
        ' ' | '\u{00a0}' | ',' | '.' => 250,
        '!' | '(' | ')' | '[' | ']' | '`' | 'f' | 'r' => 333,
        '"' => 408,
        '#'
        | '$'
        | '*'
        | '0'..='9'
        | '_'
        | 'b'
        | 'd'
        | 'g'
        | 'h'
        | 'k'
        | 'n'
        | 'o'
        | 'p'
        | 'q'
        | 'u'
        | 'v' => 500,
        '%' => 833,
        '&' => 778,
        '\'' => 180,
        '+' | '<' | '=' | '>' => 564,
        '-' => 333,
        '/' | ':' | ';' | 'i' | 'j' | 'l' => 278,
        '?' | 'a' | 'c' | 'e' | 'z' => 444,
        '@' => 921,
        'A' | 'D' | 'G' | 'H' | 'K' | 'O' | 'Q' | 'V' | 'X' | 'Y' | 'w' => 722,
        'B' | 'C' | 'R' => 667,
        'E' | 'L' | 'Z' | 'T' => 611,
        'F' | 'P' | 'S' => 556,
        'I' => 333,
        'J' | 's' => 389,
        'M' => 889,
        'N' => 722,
        'U' => 722,
        'W' => 944,
        '\\' => 278,
        '^' => 469,
        'm' => 778,
        't' => 278,
        'x' => 500,
        'y' => 500,
        '{' | '}' => 480,
        '|' => 200,
        '~' => 541,
        '\u{2013}' => 500,
        '\u{2014}' => 1000,
        '\u{2018}' | '\u{2019}' => 333,
        '\u{201c}' | '\u{201d}' => 444,
        '\u{2022}' => 350,
        '\u{2026}' => 1000,
        _ if ch.is_whitespace() => 250,
        _ => 500,
    }
}

pub(in crate::layout) fn times_bold_width_units(ch: char) -> u16 {
    match ch {
        ' ' | '\u{00a0}' | ',' | '.' => 250,
        '!' | '(' | ')' | '-' | '[' | ']' | '`' | 'f' | ':' | ';' => 333,
        '"' | 'J' | '?' | '#' | '$' | '*' | '0'..='9' | '_' | 'a' | 'o' | 'v' | 'x' | 'y' => 500,
        '%' => 1000,
        '&' | 'm' => 833,
        '\'' | 'i' | 'l' | '\\' => 278,
        '+' | '<' | '=' | '>' => 570,
        '/' => 278,
        '@' => 930,
        'A' | 'D' | 'N' | 'R' | 'U' | 'V' | 'X' => 722,
        'B' | 'E' | 'L' | 'T' | 'Z' => 667,
        'C' | 'G' | 'H' | 'K' | 'O' | 'Q' | 'Y' => 778,
        'F' | 'P' | 'S' | 'b' | 'd' | 'h' | 'k' | 'n' | 'p' | 'q' | 'u' => 556,
        'I' | 's' | '{' | '}' => 389,
        'M' => 944,
        'W' => 1000,
        '^' => 581,
        'c' | 'e' | 'z' => 444,
        'g' => 500,
        'j' | 't' => 333,
        'r' => 444,
        'w' => 722,
        '|' => 220,
        '~' => 520,
        '\u{2013}' => 500,
        '\u{2014}' => 1000,
        '\u{2018}' | '\u{2019}' => 333,
        '\u{201c}' | '\u{201d}' => 500,
        '\u{2022}' => 350,
        '\u{2026}' => 1000,
        _ if ch.is_whitespace() => 250,
        _ => 500,
    }
}

pub(in crate::layout) fn times_italic_width_units(ch: char) -> u16 {
    match ch {
        ' ' | '\u{00a0}' | ',' | '.' => 250,
        '!' | '(' | ')' | '`' | ':' | ';' => 333,
        '"' => 420,
        '#'
        | '$'
        | '*'
        | '0'..='9'
        | '?'
        | '_'
        | 'a'
        | 'b'
        | 'd'
        | 'g'
        | 'h'
        | 'n'
        | 'o'
        | 'p'
        | 'q'
        | 'u' => 500,
        '%' | 'M' | 'w' => 833,
        '&' => 778,
        '\'' => 214,
        '+' | '<' | '=' | '>' => 675,
        '-' => 333,
        '/' | 'f' | 'i' | 'j' | 'l' | 't' | '\\' => 278,
        '@' => 920,
        'A' | 'B' | 'E' | 'F' | 'P' | 'R' | 'V' | 'X' => 611,
        'C' | 'K' | 'W' => 667,
        'D' | 'H' | 'O' | 'Q' | 'U' | 'm' => 722,
        'G' => 722,
        'I' => 333,
        'J' | 'c' | 'e' | 'x' | 'y' => 444,
        'L' | 'T' | 'Y' | 'Z' => 556,
        'N' => 667,
        'S' => 500,
        '[' | ']' | 'r' | 's' | 'z' => 389,
        '^' => 422,
        'k' => 444,
        'v' => 444,
        '{' | '}' => 400,
        '|' => 275,
        '~' => 541,
        '\u{2013}' => 500,
        '\u{2014}' => 889,
        '\u{2018}' | '\u{2019}' => 333,
        '\u{201c}' | '\u{201d}' => 556,
        '\u{2022}' => 350,
        '\u{2026}' => 889,
        _ if ch.is_whitespace() => 250,
        _ => 500,
    }
}

pub(in crate::layout) fn symbol_width_units(ch: char) -> u16 {
    match ch {
        ' ' | '\u{00a0}' => 250,
        _ if ch.is_whitespace() => 250,
        _ => 600,
    }
}

pub(in crate::layout) fn character_spacing_width(text: &str, style: &CharacterStyle) -> f32 {
    let visible_count = text
        .chars()
        .filter(|ch| !is_zero_width_format_char(*ch))
        .count();
    if visible_count <= 1 || style.character_spacing_twips == 0 {
        return 0.0;
    }
    twips_to_points(style.character_spacing_twips) * visible_count.saturating_sub(1) as f32
}

pub(in crate::layout) fn passive_kerning_width(
    text: &str,
    style: &CharacterStyle,
    family: PdfFontFamily,
) -> f32 {
    let mut total = 0.0;
    let mut previous = None;
    for ch in text.chars().filter(|ch| !is_zero_width_format_char(*ch)) {
        if let Some(left) = previous {
            total += passive_pair_kerning_points(left, ch, family, style);
        }
        previous = Some(ch);
    }
    total
}

pub(in crate::layout) fn passive_kerning_width_with_document_font(
    text: &str,
    style: &CharacterStyle,
    family: PdfFontFamily,
    source_font: Option<&FontDef>,
    font_provider: Option<&FontProvider>,
) -> f32 {
    if !style_uses_passive_kerning(style) {
        return 0.0;
    }
    let mut total = 0.0;
    let mut previous = None;
    for ch in text.chars().filter(|ch| !is_zero_width_format_char(*ch)) {
        if let Some(left) = previous {
            total += source_word_compatible_pair_kerning_points(
                source_font,
                style,
                left,
                ch,
                font_provider,
            )
            .or_else(|| {
                supplied_font_pair_kerning_points(font_provider, source_font, style, left, ch)
            })
            .unwrap_or_else(|| passive_pair_kerning_points(left, ch, family, style));
        }
        previous = Some(ch);
    }
    total
}

pub(crate) fn source_word_compatible_pair_kerning_points(
    font: Option<&FontDef>,
    style: &CharacterStyle,
    left: char,
    right: char,
    font_provider: Option<&FontProvider>,
) -> Option<f32> {
    if !font_uses_source_family(font?, font_provider, &["calibri", "carlito"])
        || style.bold
        || style.italic
    {
        return None;
    }
    // Bounded regular-face pairs independently present in the registered
    // Calibri metadata references. Values are 2048-unit Calibri/Carlito kern
    // metrics; uncovered pairs continue through the supplied safe font.
    let units = match (left, right) {
        ('W', 'o') => -86,
        ('a', 'f') => -12,
        ('a', 't') => -19,
        ('a', 'y') => -38,
        ('e', 'f') => -18,
        ('e', 't') => -11,
        ('e', 'x') => -31,
        ('f', 'o') => -43,
        ('g', 'e') => -17,
        ('g', 'g') => 19,
        ('k', 'e') => -66,
        ('r', '.') => -206,
        ('r', 'd') => -28,
        ('r', 'e') => -27,
        ('s', 't') => -23,
        ('t', 'a') => -25,
        ('t', 'e') => -22,
        ('x', 't') => 6,
        ('y', 'o') => -24,
        _ => return None,
    };
    Some(style.font_size_points() * units as f32 / 2_048.0)
}

pub(in crate::layout) fn supplied_font_pair_kerning_points(
    provider: Option<&FontProvider>,
    font: Option<&FontDef>,
    style: &CharacterStyle,
    left: char,
    right: char,
) -> Option<f32> {
    let provider = provider?;
    let font = font?;
    let asset_style = FontAssetStyle {
        bold: style.bold,
        italic: style.italic,
    };
    let size = style.font_size_points();
    provider
        .exact_pair_kerning_points_for_chars_with_style(&font.name, asset_style, left, right, size)
        .or_else(|| {
            font.alternate_name.as_deref().and_then(|alternate| {
                provider.exact_pair_kerning_points_for_chars_with_style(
                    alternate,
                    asset_style,
                    left,
                    right,
                    size,
                )
            })
        })
        .or_else(|| {
            passive_supplied_font_fallback_name(font).and_then(|fallback| {
                provider.exact_pair_kerning_points_for_chars_with_style(
                    fallback,
                    asset_style,
                    left,
                    right,
                    size,
                )
            })
        })
        .or_else(|| {
            provider.pair_kerning_points_for_chars_with_style(
                &font.name,
                asset_style,
                left,
                right,
                size,
            )
        })
        .or_else(|| {
            font.alternate_name.as_deref().and_then(|alternate| {
                provider.pair_kerning_points_for_chars_with_style(
                    alternate,
                    asset_style,
                    left,
                    right,
                    size,
                )
            })
        })
}

pub fn passive_pair_kerning_points(
    left: char,
    right: char,
    family: PdfFontFamily,
    style: &CharacterStyle,
) -> f32 {
    if !style_uses_passive_kerning(style) {
        return 0.0;
    }

    let factor = match (left, right) {
        ('A', 'V') | ('A', 'W') | ('A', 'Y') => 0.11,
        ('F', 'A') | ('L', 'T') | ('L', 'V') | ('L', 'W') | ('L', 'Y') => 0.08,
        ('P', 'A') | ('T', 'A') | ('T', 'a') | ('T', 'e') | ('T', 'o') | ('T', 'y') => 0.10,
        ('V', 'A') | ('V', 'a') | ('V', 'e') | ('V', 'o') => 0.10,
        ('W', 'A') | ('W', 'a') | ('W', 'e') | ('W', 'o') => 0.08,
        ('Y', 'A') | ('Y', 'a') | ('Y', 'e') | ('Y', 'o') => 0.12,
        _ => return 0.0,
    };

    match family {
        PdfFontFamily::Helvetica | PdfFontFamily::Times => -(style.font_size_points() * factor),
        PdfFontFamily::Courier | PdfFontFamily::Symbol | PdfFontFamily::ZapfDingbats => 0.0,
    }
}

pub fn style_uses_passive_kerning(style: &CharacterStyle) -> bool {
    style.character_kerning_half_points > 0
        && style.font_size_half_points >= style.character_kerning_half_points
}

pub(in crate::layout) fn is_zero_width_format_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{00ad}' | '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{200e}' | '\u{200f}' | '\u{feff}'
    )
}

pub fn twips_to_points(twips: i32) -> f32 {
    twips as f32 / TWIPS_PER_POINT
}
