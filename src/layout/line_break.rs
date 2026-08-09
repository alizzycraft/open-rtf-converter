//! Paragraph segmentation, wrapping, math stacking, hyphenation, and fit-text.

use super::*;

#[cfg(test)]
pub(in crate::layout) fn wrap_paragraph(
    paragraph: &Paragraph,
    content_width: f32,
    markers: &MarkerContext,
    document: &Document,
) -> Vec<Line> {
    wrap_paragraph_with_font_provider(paragraph, content_width, markers, document, None)
}

pub(in crate::layout) fn wrap_paragraph_with_font_provider(
    paragraph: &Paragraph,
    content_width: f32,
    markers: &MarkerContext,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> Vec<Line> {
    wrap_paragraph_with_font_provider_dynamic_width(
        paragraph,
        content_width,
        markers,
        document,
        font_provider,
        0.0,
        |_, _| vec![(0.0, content_width)],
    )
}

pub(in crate::layout) fn wrap_paragraph_with_font_provider_dynamic_width(
    paragraph: &Paragraph,
    content_width: f32,
    markers: &MarkerContext,
    document: &Document,
    font_provider: Option<&FontProvider>,
    first_line_top_y: f32,
    mut line_content_intervals: impl FnMut(f32, f32) -> Vec<(f32, f32)>,
) -> Vec<Line> {
    let fitted_paragraph;
    let paragraph = if paragraph
        .runs
        .iter()
        .any(|run| run.style.fit_text_width_twips.is_some())
    {
        fitted_paragraph = apply_fit_text_scaling_to_run_groups(paragraph, document, font_provider);
        &fitted_paragraph
    } else {
        paragraph
    };
    let operator_spaced_math_paragraph = apply_passive_math_operator_spacing(paragraph);
    let paragraph = operator_spaced_math_paragraph.as_ref().unwrap_or(paragraph);
    let grouped_math_nary_paragraph =
        group_word_compatible_passive_math_nary_runs(paragraph, document, font_provider);
    let paragraph = grouped_math_nary_paragraph.as_ref().unwrap_or(paragraph);
    let nary_spaced_math_paragraph = apply_passive_math_nary_base_spacing(paragraph);
    let paragraph = nary_spaced_math_paragraph.as_ref().unwrap_or(paragraph);
    let stacked_math_limit_paragraph =
        stack_passive_math_limits(paragraph, document, font_provider);
    let paragraph = stacked_math_limit_paragraph.as_ref().unwrap_or(paragraph);
    let stacked_math_paragraph = stack_passive_math_fractions(paragraph, document, font_provider);
    let paragraph = stacked_math_paragraph.as_ref().unwrap_or(paragraph);
    let stacked_math_array_paragraph =
        stack_passive_math_equation_arrays(paragraph, document, font_provider);
    let paragraph = stacked_math_array_paragraph.as_ref().unwrap_or(paragraph);
    let mut lines = Vec::new();
    let mut current = Line {
        runs: Vec::new(),
        width: 0.0,
        natural_height: 14.0,
        height: 14.0,
        segmented_flow: false,
        inline_image_baseline_context: false,
    };
    let mut is_first_line = true;
    let mut current_line_top_y = first_line_top_y;
    let mut drop_cap_applied = false;

    for (run_index, run) in paragraph.runs.iter().enumerate() {
        let trailing_character_spacing = character_spacing_boundary_after_run(paragraph, run_index);
        let mut segments = split_run_for_wrapping_with_drop_cap(
            run,
            markers,
            &paragraph.style,
            &mut drop_cap_applied,
        );
        if paragraph.style.auto_hyphenation && !paragraph.style.no_wrap {
            segments = apply_passive_auto_hyphenation(
                segments,
                content_width,
                &paragraph.style,
                document,
                font_provider,
            );
        }
        if paragraph.style.no_wrap {
            segments = apply_no_wrap_emergency_boundaries(
                segments,
                content_width,
                &paragraph.style,
                document,
                font_provider,
            );
        } else {
            segments = apply_emergency_word_wrapping(
                segments,
                content_width,
                &paragraph.style,
                document,
                font_provider,
            );
        }
        for (segment_index, mut segment) in segments.iter().cloned().enumerate() {
            if segment.text == "\n" {
                let finished_height = current.height;
                lines.push(current);
                current_line_top_y -= finished_height;
                current = empty_line();
                is_first_line = false;
                continue;
            }
            if is_passive_math_inline_lead_marker(&segment.text) {
                if current.width > 0.01 {
                    // Word leaves the invisible object lead-in on the preceding line
                    // when the following atomic equation wraps.
                    push_segment(
                        &mut current,
                        segment,
                        &paragraph.style,
                        document,
                        font_provider,
                    );
                }
                continue;
            }

            let mut width = measure_flow_run(
                &segment,
                current.width,
                &paragraph.style,
                document,
                font_provider,
            );
            let mut fit_width = inline_image_cluster_width(
                &segments,
                segment_index,
                width,
                current.width,
                &paragraph.style,
                document,
                font_provider,
            );
            if is_passive_math_fraction_rule_marker(&segment.text) {
                fit_width = passive_math_fraction_width_points(&segment.style);
            }
            if is_passive_math_stack_anchor_marker(&segment.text) {
                fit_width = passive_math_stack_width_points(&segment.style);
            }
            let interval_fit_width = if segment.text.ends_with(char::is_whitespace)
                && !contains_inline_marker(&segment.text)
            {
                let mut visible_segment = segment.clone();
                visible_segment.text = visible_segment
                    .text
                    .trim_end_matches(char::is_whitespace)
                    .to_string();
                measure_flow_run(
                    &visible_segment,
                    current.width,
                    &paragraph.style,
                    document,
                    font_provider,
                )
            } else {
                width
            };
            let mut intervals = line_content_intervals(current_line_top_y, current.height);
            intervals.retain(|(_, width)| *width >= 12.0);
            intervals.sort_by(|(left_a, _), (left_b, _)| left_a.total_cmp(left_b));
            if intervals.is_empty() {
                intervals.push((0.0, content_width));
            }
            let supports_segmented_flow = paragraph.style.alignment == Alignment::Left
                && paragraph.style.left_indent_twips == 0
                && paragraph.style.right_indent_twips == 0
                && paragraph.style.first_line_indent_twips == 0
                && paragraph.style.tab_stops_twips.is_empty()
                && intervals.len() > 1;
            current.segmented_flow |= supports_segmented_flow;
            let (line_width, ignore_trailing_space_for_fit) = if supports_segmented_flow {
                let current_interval = intervals
                    .iter()
                    .position(|(left, interval_width)| {
                        current.width + 0.01 >= *left
                            && current.width <= *left + *interval_width + 0.01
                    })
                    .unwrap_or(0);
                let (interval_left, interval_width) = intervals[current_interval];
                if current.runs.is_empty() && current.width + 0.01 < interval_left {
                    push_flow_exclusion_advance(&mut current, interval_left, &segment.style);
                }
                let interval_right = interval_left + interval_width;
                if current.width + interval_fit_width > interval_right {
                    if let Some((next_left, _)) = intervals
                        .iter()
                        .skip(current_interval + 1)
                        .find(|(_, next_width)| interval_fit_width <= *next_width)
                        .copied()
                    {
                        let advance = (next_left - current.width).max(0.0);
                        push_flow_exclusion_advance(&mut current, advance, &segment.style);
                    }
                }
                let line_width = intervals
                    .iter()
                    .find(|(left, interval_width)| {
                        current.width + 0.01 >= *left
                            && current.width <= *left + *interval_width + 0.01
                    })
                    .map(|(left, interval_width)| left + interval_width)
                    .unwrap_or(interval_right);
                (line_width, line_width + 0.01 < content_width)
            } else {
                let (requested_left, requested_content_width) = intervals
                    .into_iter()
                    .max_by(|(_, width_a), (_, width_b)| width_a.total_cmp(width_b))
                    .unwrap_or((0.0, content_width));
                let active_content_width = if content_width >= 12.0 {
                    requested_content_width.clamp(12.0, content_width)
                } else {
                    requested_content_width.clamp(0.0, content_width.max(0.0))
                };
                (
                    paragraph_line_width(active_content_width, &paragraph.style, is_first_line),
                    requested_left.abs() < 0.01
                        && (requested_content_width - content_width).abs() < 0.01,
                )
            };
            let dynamic_word = segment
                .text
                .trim_end_matches(char::is_whitespace)
                .to_string();
            let dynamic_trailing = segment.text[dynamic_word.len()..].to_string();
            if current.runs.is_empty()
                && width > line_width + 0.01
                && line_width + 0.01 < content_width
                && segment.style.fit_text_width_twips.is_none()
                && segment.tab_stop_position.is_none()
                && segment.text != "\t"
                && segment.style.passive_math_fraction_id.is_none()
                && !contains_inline_marker(&segment.text)
                && dynamic_word.chars().count() >= 2
                && !dynamic_word.chars().any(char::is_whitespace)
            {
                let display = display_text(&dynamic_word, &segment.style);
                let family = font_family_for_run_text(document, &segment.style, &display);
                let mut pieces = Vec::new();
                let mut split_segment = segment.clone();
                split_segment.text = dynamic_word;
                push_emergency_wrapped_segment(
                    &mut pieces,
                    split_segment,
                    line_width.max(1.0),
                    family,
                    document,
                    font_provider,
                );
                if pieces.len() > 1 {
                    segment = pieces.pop().expect("dynamic emergency segment tail");
                    segment.text.push_str(&dynamic_trailing);
                    for piece in pieces {
                        push_segment(
                            &mut current,
                            piece,
                            &paragraph.style,
                            document,
                            font_provider,
                        );
                        let finished_height = current.height;
                        lines.push(current);
                        current_line_top_y -= finished_height;
                        current = empty_line();
                        is_first_line = false;
                    }
                    width = measure_flow_run(
                        &segment,
                        current.width,
                        &paragraph.style,
                        document,
                        font_provider,
                    );
                    fit_width = width;
                }
            }
            if segment_index + 1 == segments.len() {
                fit_width += trailing_character_spacing;
            }
            let inline_image_body_edge_tolerance = if !supports_segmented_flow
                && (line_width - content_width).abs() < 0.01
                && current
                    .runs
                    .iter()
                    .any(|run| inline_image_marker_index(&run.text).is_some())
            {
                WORD_INLINE_IMAGE_BODY_EDGE_TOLERANCE_POINTS
            } else {
                0.0
            };
            let is_final_paragraph_segment =
                run_index + 1 == paragraph.runs.len() && segment_index + 1 == segments.len();
            if is_final_paragraph_segment
                && !segment.text.ends_with(char::is_whitespace)
                && current
                    .runs
                    .iter()
                    .any(|run| inline_image_marker_index(&run.text).is_some())
            {
                let mut paragraph_mark = segment.clone();
                paragraph_mark.text = " ".to_string();
                fit_width += measure_flow_run(
                    &paragraph_mark,
                    current.width + fit_width,
                    &paragraph.style,
                    document,
                    font_provider,
                );
            }
            let boundary_fit_width = if ignore_trailing_space_for_fit
                && segment.text.ends_with(char::is_whitespace)
                && !contains_inline_marker(&segment.text)
            {
                let mut visible_segment = segment.clone();
                visible_segment.text = visible_segment
                    .text
                    .trim_end_matches(char::is_whitespace)
                    .to_string();
                measure_flow_run(
                    &visible_segment,
                    current.width,
                    &paragraph.style,
                    document,
                    font_provider,
                )
            } else {
                fit_width
            };
            if current.width > 0.0
                && current.width + boundary_fit_width
                    > line_width + inline_image_body_edge_tolerance
            {
                let carried_whitespace = if paragraph.style.no_wrap {
                    take_trailing_whitespace(&mut current, document, font_provider)
                } else {
                    Vec::new()
                };
                materialize_line_end_soft_hyphen(&mut current, document, font_provider);
                let finished_height = current.height;
                lines.push(current);
                current_line_top_y -= finished_height;
                current = empty_line();
                is_first_line = false;
                let trimmed = if paragraph.style.no_wrap {
                    segment.text.clone()
                } else {
                    segment.text.trim_start().to_string()
                };
                if trimmed.is_empty() {
                    continue;
                }
                let next_segment = FlowRun {
                    dynamic_field: None,
                    text: trimmed,
                    style: segment.style,
                    width: 0.0,
                    line_height_points: segment.line_height_points,
                    tab_leader: TabLeader::None,
                    tab_alignment: TabAlignment::Left,
                    tab_stop_position: None,
                    soft_hyphen_after: segment.soft_hyphen_after,
                };
                let mut next_intervals = line_content_intervals(current_line_top_y, current.height);
                next_intervals.retain(|(_, interval_width)| *interval_width >= 12.0);
                next_intervals.sort_by(|(left_a, _), (left_b, _)| left_a.total_cmp(left_b));
                let next_supports_segmented_flow = paragraph.style.alignment == Alignment::Left
                    && paragraph.style.left_indent_twips == 0
                    && paragraph.style.right_indent_twips == 0
                    && paragraph.style.first_line_indent_twips == 0
                    && paragraph.style.tab_stops_twips.is_empty()
                    && next_intervals.len() > 1;
                if next_supports_segmented_flow {
                    current.segmented_flow = true;
                    let mut next_width = measure_flow_run(
                        &next_segment,
                        0.0,
                        &paragraph.style,
                        document,
                        font_provider,
                    );
                    if segment_index + 1 == segments.len() {
                        next_width += trailing_character_spacing;
                    }
                    let next_left = next_intervals
                        .iter()
                        .find(|(_, interval_width)| next_width <= *interval_width + 0.01)
                        .or_else(|| next_intervals.first())
                        .map(|(left, _)| *left)
                        .unwrap_or(0.0);
                    if next_left > 0.01 {
                        push_flow_exclusion_advance(&mut current, next_left, &next_segment.style);
                    }
                }
                for whitespace in carried_whitespace {
                    push_segment(
                        &mut current,
                        whitespace,
                        &paragraph.style,
                        document,
                        font_provider,
                    );
                }
                push_segment_with_trailing_character_spacing(
                    &mut current,
                    next_segment,
                    &paragraph.style,
                    document,
                    font_provider,
                    if segment_index + 1 == segments.len() {
                        trailing_character_spacing
                    } else {
                        0.0
                    },
                );
            } else {
                push_segment_with_trailing_character_spacing(
                    &mut current,
                    segment,
                    &paragraph.style,
                    document,
                    font_provider,
                    if segment_index + 1 == segments.len() {
                        trailing_character_spacing
                    } else {
                        0.0
                    },
                );
            }
        }
    }

    if !current.runs.is_empty() {
        lines.push(current);
    }

    if lines.is_empty() {
        lines.push(empty_line());
    }

    let mut inline_image_baseline_context = false;
    for line in &mut lines {
        inline_image_baseline_context |= line
            .runs
            .iter()
            .any(|run| inline_image_marker_index(&run.text).is_some());
        line.inline_image_baseline_context = inline_image_baseline_context;
    }

    lines
}

pub(in crate::layout) fn character_spacing_boundary_after_run(
    paragraph: &Paragraph,
    run_index: usize,
) -> f32 {
    let Some(run) = paragraph.runs.get(run_index) else {
        return 0.0;
    };
    let last = run
        .text
        .chars()
        .rev()
        .find(|ch| !is_zero_width_format_char(*ch));
    if run.style.hidden
        || run.style.character_spacing_twips == 0
        || contains_inline_marker(&run.text)
        || !last.is_some_and(is_character_spacing_boundary_char)
    {
        return 0.0;
    }
    let following = paragraph.runs.iter().skip(run_index + 1).find(|following| {
        !following.style.hidden
            && following
                .text
                .chars()
                .any(|ch| !is_zero_width_format_char(ch))
    });
    let first = following.and_then(|following| {
        (!contains_inline_marker(&following.text))
            .then(|| {
                following
                    .text
                    .chars()
                    .find(|ch| !is_zero_width_format_char(*ch))
            })
            .flatten()
    });
    if !first.is_some_and(is_character_spacing_boundary_char) {
        return 0.0;
    }
    twips_to_points(run.style.character_spacing_twips)
}

pub(in crate::layout) fn is_character_spacing_boundary_char(ch: char) -> bool {
    !is_zero_width_format_char(ch) && !matches!(ch, '\t' | '\r' | '\n') && !ch.is_control()
}

pub(in crate::layout) fn apply_passive_math_operator_spacing(
    paragraph: &Paragraph,
) -> Option<Paragraph> {
    let has_spaced_operator = paragraph.runs.iter().any(|run| {
        run.style.passive_math
            && !run.style.overline
            && run.style.underline == UnderlineStyle::None
            && run.text.char_indices().any(|(index, ch)| {
                passive_math_operator_side_space_em(ch).is_some()
                    && passive_math_operator_has_operands(&run.text, index, ch.len_utf8())
            })
    });
    if !has_spaced_operator {
        return None;
    }

    let mut output = Paragraph {
        style: paragraph.style.clone(),
        runs: Vec::with_capacity(paragraph.runs.len().saturating_add(8)),
    };
    for run in &paragraph.runs {
        if !run.style.passive_math
            || run.style.overline
            || run.style.underline != UnderlineStyle::None
        {
            output.runs.push(run.clone());
            continue;
        }
        let mut start = 0usize;
        for (index, ch) in run.text.char_indices() {
            let Some(side_space_em) = passive_math_operator_side_space_em(ch) else {
                continue;
            };
            if !passive_math_operator_has_operands(&run.text, index, ch.len_utf8()) {
                continue;
            }
            if start < index {
                output.runs.push(Run {
                    dynamic_field: None,
                    text: run.text[start..index].to_string(),
                    style: run.style.clone(),
                });
            }
            let side_space = run.style.font_size_points() * side_space_em;
            push_passive_math_advance(&mut output.runs, side_space, &run.style);
            output.runs.push(Run {
                dynamic_field: None,
                text: ch.to_string(),
                style: run.style.clone(),
            });
            push_passive_math_advance(&mut output.runs, side_space, &run.style);
            start = index + ch.len_utf8();
        }
        if start < run.text.len() {
            output.runs.push(Run {
                dynamic_field: None,
                text: run.text[start..].to_string(),
                style: run.style.clone(),
            });
        }
    }
    Some(output)
}

pub(in crate::layout) fn passive_math_operator_has_operands(
    text: &str,
    index: usize,
    operator_len: usize,
) -> bool {
    text[..index].chars().any(|ch| !ch.is_whitespace())
        && text[index + operator_len..]
            .chars()
            .any(|ch| !ch.is_whitespace())
}

pub(in crate::layout) fn passive_math_operator_side_space_em(ch: char) -> Option<f32> {
    if matches!(
        ch,
        '=' | '<' | '>' | '\u{2260}' | '\u{2248}' | '\u{2264}' | '\u{2265}'
    ) {
        Some(5.0 / 18.0)
    } else if matches!(
        ch,
        '+' | '-' | '\u{2212}' | '\u{00b1}' | '\u{2213}' | '\u{00d7}' | '\u{00f7}'
    ) {
        Some(1.0 / 7.0)
    } else {
        None
    }
}

pub(in crate::layout) fn apply_passive_math_nary_base_spacing(
    paragraph: &Paragraph,
) -> Option<Paragraph> {
    if !paragraph
        .runs
        .iter()
        .any(|run| run.style.passive_math_nary_base_id.is_some())
    {
        return None;
    }

    let mut output = Paragraph {
        style: paragraph.style.clone(),
        runs: Vec::with_capacity(paragraph.runs.len().saturating_add(4)),
    };
    let mut active_base_id = None;
    for run in &paragraph.runs {
        let base_id = run.style.passive_math_nary_base_id;
        if let Some(base_id) = base_id
            && active_base_id != Some(base_id)
        {
            let operator_gap = run.style.font_size_points() * WORD_NARY_OPERATOR_GAP_EM;
            push_passive_math_advance(&mut output.runs, operator_gap, &run.style);
        }
        output.runs.push(run.clone());
        active_base_id = base_id;
    }
    Some(output)
}

pub(in crate::layout) fn group_word_compatible_passive_math_nary_runs(
    paragraph: &Paragraph,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> Option<Paragraph> {
    if !paragraph
        .runs
        .iter()
        .any(|run| run.style.passive_math_nary_id.is_some())
    {
        return None;
    }

    let mut output = Paragraph {
        style: paragraph.style.clone(),
        runs: Vec::with_capacity(paragraph.runs.len().saturating_add(4)),
    };
    let mut start = 0usize;
    while start < paragraph.runs.len() {
        let Some(nary_id) = paragraph.runs[start].style.passive_math_nary_id else {
            output.runs.push(paragraph.runs[start].clone());
            start += 1;
            continue;
        };
        let mut end = start + 1;
        while end < paragraph.runs.len()
            && paragraph.runs[end].style.passive_math_nary_id == Some(nary_id)
        {
            end += 1;
        }
        let nary_runs = &paragraph.runs[start..end];
        let mut nary_width =
            passive_math_model_runs_width(nary_runs, &paragraph.style, document, font_provider)
                .max(0.1);
        let spacing_run_index = nary_runs
            .iter()
            .position(|run| run.style.passive_math_nary_base_id.is_some())
            .or((nary_runs.len() > 1).then_some(1));
        if let Some(spacing_run_index) = spacing_run_index {
            nary_width +=
                nary_runs[spacing_run_index].style.font_size_points() * WORD_NARY_OPERATOR_GAP_EM;
        }
        let mut marker_style = nary_runs[0].style.clone();
        marker_style.passive_math_nary_id = None;
        let mut lead_style = marker_style.clone();
        lead_style.character_spacing_twips =
            points_to_bounded_twips(WORD_INLINE_OFFICE_MATH_LEAD_IN_POINTS);
        output.runs.push(Run {
            dynamic_field: None,
            text: PASSIVE_MATH_INLINE_LEAD_MARKER.to_string(),
            style: lead_style,
        });
        marker_style.character_spacing_twips = points_to_bounded_twips(nary_width);
        output.runs.push(Run {
            dynamic_field: None,
            text: PASSIVE_MATH_STACK_ANCHOR_MARKER.to_string(),
            style: marker_style,
        });
        for (index, run) in nary_runs.iter().enumerate() {
            let mut run = run.clone();
            if Some(index) == spacing_run_index && run.style.passive_math_nary_base_id.is_none() {
                run.style.passive_math_nary_base_id = Some(nary_id);
            }
            output.runs.push(run);
        }
        start = end;
    }
    Some(output)
}

pub(in crate::layout) fn stack_passive_math_limits(
    paragraph: &Paragraph,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> Option<Paragraph> {
    if !paragraph
        .runs
        .iter()
        .any(|run| run.style.passive_math_limit_id.is_some())
    {
        return None;
    }

    let mut output = Paragraph {
        style: paragraph.style.clone(),
        runs: Vec::with_capacity(paragraph.runs.len().saturating_add(6)),
    };
    let mut start = 0usize;
    while start < paragraph.runs.len() {
        let Some(limit_id) = paragraph.runs[start].style.passive_math_limit_id else {
            output.runs.push(paragraph.runs[start].clone());
            start += 1;
            continue;
        };
        let mut end = start + 1;
        while end < paragraph.runs.len()
            && paragraph.runs[end].style.passive_math_limit_id == Some(limit_id)
        {
            end += 1;
        }
        let limit_runs = &paragraph.runs[start..end];
        let base = limit_runs
            .iter()
            .filter(|run| run.style.passive_math_limit_part == PassiveMathLimitPart::Base)
            .cloned()
            .collect::<Vec<_>>();
        let script = limit_runs
            .iter()
            .filter(|run| {
                matches!(
                    run.style.passive_math_limit_part,
                    PassiveMathLimitPart::Lower | PassiveMathLimitPart::Upper
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        if base.is_empty() || script.is_empty() {
            output.runs.extend_from_slice(limit_runs);
            start = end;
            continue;
        }

        let base_width =
            passive_math_model_runs_width(&base, &paragraph.style, document, font_provider);
        let script_width =
            passive_math_model_runs_width(&script, &paragraph.style, document, font_provider);
        let stack_width = base_width.max(script_width).max(0.1);
        let mut marker_style = limit_runs[0].style.clone();
        marker_style.passive_math_limit_part = PassiveMathLimitPart::None;
        marker_style.baseline_shift_half_points = 0;
        marker_style.font_size_scale_percent = 100;
        marker_style.character_spacing_twips = points_to_bounded_twips(stack_width);
        output.runs.push(Run {
            dynamic_field: None,
            text: PASSIVE_MATH_STACK_ANCHOR_MARKER.to_string(),
            style: marker_style.clone(),
        });

        let base_start = (stack_width - base_width) / 2.0;
        push_passive_math_advance(&mut output.runs, base_start, &marker_style);
        output.runs.extend(base);
        let script_start = (stack_width - script_width) / 2.0;
        push_passive_math_advance(
            &mut output.runs,
            script_start - base_start - base_width,
            &marker_style,
        );
        output.runs.extend(script);
        push_passive_math_advance(
            &mut output.runs,
            stack_width - script_start - script_width,
            &marker_style,
        );
        start = end;
    }
    Some(output)
}

pub(in crate::layout) fn stack_passive_math_fractions(
    paragraph: &Paragraph,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> Option<Paragraph> {
    if !paragraph
        .runs
        .iter()
        .any(|run| run.style.passive_math_fraction_id.is_some())
    {
        return None;
    }

    let mut output = Paragraph {
        style: paragraph.style.clone(),
        runs: Vec::with_capacity(paragraph.runs.len().saturating_add(6)),
    };
    let mut start = 0usize;
    while start < paragraph.runs.len() {
        let Some(fraction_id) = paragraph.runs[start].style.passive_math_fraction_id else {
            output.runs.push(paragraph.runs[start].clone());
            start += 1;
            continue;
        };
        let mut end = start + 1;
        while end < paragraph.runs.len()
            && paragraph.runs[end].style.passive_math_fraction_id == Some(fraction_id)
        {
            end += 1;
        }
        let fraction_runs = &paragraph.runs[start..end];
        let numerator = fraction_runs
            .iter()
            .filter(|run| {
                run.style.passive_math_fraction_part == PassiveMathFractionPart::Numerator
            })
            .cloned()
            .collect::<Vec<_>>();
        let denominator = fraction_runs
            .iter()
            .filter(|run| {
                run.style.passive_math_fraction_part == PassiveMathFractionPart::Denominator
            })
            .cloned()
            .collect::<Vec<_>>();
        if numerator.is_empty() || denominator.is_empty() {
            output.runs.extend_from_slice(fraction_runs);
            start = end;
            continue;
        }

        let numerator_width =
            passive_math_model_runs_width(&numerator, &paragraph.style, document, font_provider);
        let denominator_width =
            passive_math_model_runs_width(&denominator, &paragraph.style, document, font_provider);
        let fraction_width = numerator_width.max(denominator_width).max(0.1);
        let mut marker_style = fraction_runs[0].style.clone();
        marker_style.passive_math_fraction_part = PassiveMathFractionPart::None;
        marker_style.baseline_shift_half_points = 0;
        marker_style.font_size_scale_percent = 100;
        let mut lead_style = marker_style.clone();
        lead_style.character_spacing_twips =
            points_to_bounded_twips(WORD_INLINE_OFFICE_MATH_LEAD_IN_POINTS);
        output.runs.push(Run {
            dynamic_field: None,
            text: PASSIVE_MATH_INLINE_LEAD_MARKER.to_string(),
            style: lead_style,
        });
        marker_style.character_spacing_twips = points_to_bounded_twips(fraction_width);
        output.runs.push(Run {
            dynamic_field: None,
            text: PASSIVE_MATH_FRACTION_RULE_MARKER.to_string(),
            style: marker_style.clone(),
        });

        let numerator_start = (fraction_width - numerator_width) / 2.0;
        push_passive_math_advance(&mut output.runs, numerator_start, &marker_style);
        output.runs.extend(numerator);
        let denominator_start = (fraction_width - denominator_width) / 2.0;
        push_passive_math_advance(
            &mut output.runs,
            denominator_start - numerator_start - numerator_width,
            &marker_style,
        );
        output.runs.extend(denominator);
        push_passive_math_advance(
            &mut output.runs,
            fraction_width - denominator_start - denominator_width,
            &marker_style,
        );
        start = end;
    }
    Some(output)
}

pub(in crate::layout) fn stack_passive_math_equation_arrays(
    paragraph: &Paragraph,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> Option<Paragraph> {
    if !paragraph
        .runs
        .iter()
        .any(|run| run.style.passive_math_equation_array_id.is_some())
    {
        return None;
    }

    let mut output = Paragraph {
        style: paragraph.style.clone(),
        runs: Vec::with_capacity(paragraph.runs.len().saturating_add(8)),
    };
    let mut start = 0usize;
    while start < paragraph.runs.len() {
        let Some(array_id) = paragraph.runs[start].style.passive_math_equation_array_id else {
            output.runs.push(paragraph.runs[start].clone());
            start += 1;
            continue;
        };
        let mut end = start + 1;
        while end < paragraph.runs.len()
            && paragraph.runs[end].style.passive_math_equation_array_id == Some(array_id)
        {
            end += 1;
        }
        let array_runs = &paragraph.runs[start..end];
        let row_count = array_runs
            .iter()
            .map(|run| run.style.passive_math_equation_array_row)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .min(array_runs.len().saturating_add(1));
        let mut rows = vec![Vec::new(); row_count];
        for run in array_runs {
            let row_index = run
                .style
                .passive_math_equation_array_row
                .min(row_count.saturating_sub(1));
            let mut row_run = run.clone();
            row_run.text.retain(|ch| ch != '\n' && ch != '\r');
            if !row_run.text.is_empty() {
                rows[row_index].push(row_run);
            }
        }
        if rows.iter().any(Vec::is_empty) {
            output.runs.extend_from_slice(array_runs);
            start = end;
            continue;
        }

        let row_widths = rows
            .iter()
            .map(|row| {
                passive_math_model_runs_width(row, &paragraph.style, document, font_provider)
            })
            .collect::<Vec<_>>();
        let array_width = row_widths.iter().copied().fold(0.1_f32, f32::max);
        let mut marker_style = array_runs[0].style.clone();
        marker_style.passive_math_equation_array_id = None;
        marker_style.passive_math_equation_array_row = 0;
        marker_style.baseline_shift_half_points = 0;
        marker_style.font_size_scale_percent = 100;
        marker_style.character_spacing_twips = points_to_bounded_twips(array_width);
        output.runs.push(Run {
            dynamic_field: None,
            text: PASSIVE_MATH_STACK_ANCHOR_MARKER.to_string(),
            style: marker_style.clone(),
        });

        let mut cursor = 0.0_f32;
        let row_count_i64 = i64::try_from(row_count).unwrap_or(i64::MAX);
        for (row_index, (row, row_width)) in
            rows.into_iter().zip(row_widths.into_iter()).enumerate()
        {
            let row_start = (array_width - row_width) / 2.0;
            push_passive_math_advance(&mut output.runs, row_start - cursor, &marker_style);
            cursor = row_start;
            let row_index_i64 = i64::try_from(row_index).unwrap_or(i64::MAX);
            let centered_index = row_count_i64
                .saturating_sub(1)
                .saturating_sub(row_index_i64.saturating_mul(2));
            let row_shift_half_points = centered_index
                .saturating_mul(i64::from(
                    OFFICE_MATH_EQUATION_ARRAY_HALF_ROW_PITCH_HALF_POINTS,
                ))
                .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                as i32;
            for mut run in row {
                if !is_passive_advance_marker(&run.text)
                    && !is_passive_math_inline_lead_marker(&run.text)
                    && !is_passive_math_stack_anchor_marker(&run.text)
                {
                    run.style.baseline_shift_half_points = run
                        .style
                        .baseline_shift_half_points
                        .saturating_add(row_shift_half_points);
                }
                run.style.passive_math_equation_array_id = None;
                run.style.passive_math_equation_array_row = 0;
                output.runs.push(run);
            }
            cursor += row_width;
        }
        push_passive_math_advance(&mut output.runs, array_width - cursor, &marker_style);
        start = end;
    }
    Some(output)
}

pub(in crate::layout) fn passive_math_model_runs_width(
    runs: &[Run],
    paragraph_style: &ParagraphStyle,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> f32 {
    runs.iter()
        .map(|run| {
            let flow = FlowRun {
                dynamic_field: None,
                text: run.text.clone(),
                style: run.style.clone(),
                width: 0.0,
                line_height_points: run.style.font_size_points(),
                tab_leader: TabLeader::None,
                tab_alignment: TabAlignment::Left,
                tab_stop_position: None,
                soft_hyphen_after: false,
            };
            measure_flow_run(&flow, 0.0, paragraph_style, document, font_provider)
        })
        .sum()
}

pub(in crate::layout) fn push_passive_math_advance(
    runs: &mut Vec<Run>,
    width: f32,
    base_style: &CharacterStyle,
) {
    if width.abs() < 0.01 {
        return;
    }
    let mut style = base_style.clone();
    style.baseline_shift_half_points = 0;
    style.character_spacing_twips = points_to_bounded_twips(width);
    runs.push(Run {
        dynamic_field: None,
        text: PASSIVE_ADVANCE_MARKER.to_string(),
        style,
    });
}

pub(in crate::layout) fn points_to_bounded_twips(points: f32) -> i32 {
    (points * 20.0)
        .round()
        .clamp(i32::MIN as f32, i32::MAX as f32) as i32
}

pub(in crate::layout) fn apply_fit_text_scaling_to_run_groups(
    paragraph: &Paragraph,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> Paragraph {
    let mut paragraph = paragraph.clone();
    let mut group_start = 0usize;
    while group_start < paragraph.runs.len() {
        let Some(target_twips) = paragraph.runs[group_start].style.fit_text_width_twips else {
            group_start += 1;
            continue;
        };
        if target_twips <= 0 {
            paragraph.runs[group_start].style.fit_text_width_twips = None;
            group_start += 1;
            continue;
        }

        let mut group_end = group_start + 1;
        while group_end < paragraph.runs.len()
            && paragraph.runs[group_end].style.fit_text_width_twips == Some(target_twips)
        {
            group_end += 1;
        }

        let mut measured_width = 0.0;
        for run in &paragraph.runs[group_start..group_end] {
            let mut style = run.style.clone();
            style.fit_text_width_twips = None;
            let flow = FlowRun {
                dynamic_field: None,
                text: run.text.clone(),
                line_height_points: style.font_size_points(),
                style,
                width: 0.0,
                tab_leader: TabLeader::None,
                tab_alignment: TabAlignment::Left,
                tab_stop_position: None,
                soft_hyphen_after: false,
            };
            measured_width += measure_flow_run(
                &flow,
                measured_width,
                &paragraph.style,
                document,
                font_provider,
            );
        }

        if measured_width > 0.0 {
            let target_width = twips_to_points(target_twips).max(0.1);
            let group_scale = target_width / measured_width;
            for run in &mut paragraph.runs[group_start..group_end] {
                run.style.character_scaling_percent = (run.style.character_scaling_percent as f32
                    * group_scale)
                    .round()
                    .clamp(1.0, 600.0) as i32;
                run.style.fit_text_width_twips = None;
            }
        }

        group_start = group_end;
    }
    paragraph
}

pub(in crate::layout) fn split_run_for_wrapping_with_drop_cap(
    run: &Run,
    markers: &MarkerContext,
    paragraph_style: &ParagraphStyle,
    drop_cap_applied: &mut bool,
) -> Vec<FlowRun> {
    let segments = split_run_for_wrapping(run, markers);
    if *drop_cap_applied || paragraph_style.drop_cap_lines <= 1 {
        return segments;
    }
    apply_drop_cap_to_segments(segments, paragraph_style.drop_cap_lines, drop_cap_applied)
}

pub(in crate::layout) fn apply_drop_cap_to_segments(
    segments: Vec<FlowRun>,
    drop_cap_lines: i32,
    drop_cap_applied: &mut bool,
) -> Vec<FlowRun> {
    let mut output = Vec::new();
    for segment in segments {
        if *drop_cap_applied || segment.text == "\t" || segment.text == "\n" {
            output.push(segment);
            continue;
        }

        let Some((cap_start, cap)) = segment
            .text
            .char_indices()
            .find(|(_, ch)| !ch.is_whitespace())
        else {
            output.push(segment);
            continue;
        };
        let cap_end = cap_start + cap.len_utf8();

        if cap_start > 0 {
            output.push(FlowRun {
                dynamic_field: None,
                text: segment.text[..cap_start].to_string(),
                style: segment.style.clone(),
                width: 0.0,
                line_height_points: segment.line_height_points,
                tab_leader: segment.tab_leader,
                tab_alignment: segment.tab_alignment,
                tab_stop_position: segment.tab_stop_position,
                soft_hyphen_after: false,
            });
        }

        let mut cap_style = segment.style.clone();
        cap_style.font_size_half_points = cap_style
            .font_size_half_points
            .saturating_mul(drop_cap_lines.max(1))
            .clamp(2, MAX_SYNTHETIC_DROP_CAP_FONT_SIZE_HALF_POINTS);
        output.push(FlowRun {
            dynamic_field: None,
            text: cap.to_string(),
            line_height_points: cap_style.font_size_points(),
            style: cap_style,
            width: 0.0,
            tab_leader: segment.tab_leader,
            tab_alignment: segment.tab_alignment,
            tab_stop_position: segment.tab_stop_position,
            soft_hyphen_after: false,
        });

        if cap_end < segment.text.len() {
            output.push(FlowRun {
                dynamic_field: None,
                text: segment.text[cap_end..].to_string(),
                style: segment.style,
                width: 0.0,
                line_height_points: segment.line_height_points,
                tab_leader: segment.tab_leader,
                tab_alignment: segment.tab_alignment,
                tab_stop_position: segment.tab_stop_position,
                soft_hyphen_after: segment.soft_hyphen_after,
            });
        }

        *drop_cap_applied = true;
    }
    output
}

pub(in crate::layout) fn apply_passive_auto_hyphenation(
    segments: Vec<FlowRun>,
    content_width: f32,
    paragraph_style: &ParagraphStyle,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> Vec<FlowRun> {
    let max_line_width = paragraph_line_width(content_width, paragraph_style, false)
        .max(paragraph_line_width(content_width, paragraph_style, true));
    let hyphenation_zone = twips_to_points(paragraph_style.hyphenation_zone_twips.max(0));
    let mut output = Vec::new();
    let mut consecutive_hyphenated = 0usize;

    for segment in segments {
        if !segment_can_auto_hyphenate(&segment, paragraph_style) {
            consecutive_hyphenated = 0;
            output.push(segment);
            continue;
        }
        let display = display_text(&segment.text, &segment.style);
        let family = font_family_for_run_text(document, &segment.style, &display);
        if measure_text_with_document_font(
            &display,
            &segment.style,
            family,
            document,
            font_provider,
        ) <= max_line_width + hyphenation_zone
        {
            consecutive_hyphenated = 0;
            output.push(segment);
            continue;
        }
        push_auto_hyphenated_segment(
            &mut output,
            segment,
            max_line_width,
            family,
            document,
            font_provider,
            paragraph_style.max_consecutive_hyphenated_lines,
            &mut consecutive_hyphenated,
        );
    }

    output
}

pub(in crate::layout) fn segment_can_auto_hyphenate(
    segment: &FlowRun,
    paragraph_style: &ParagraphStyle,
) -> bool {
    segment.tab_stop_position.is_none()
        && segment.text != "\t"
        && segment.text != "\n"
        && !contains_inline_marker(&segment.text)
        && !segment.text.contains('\u{00ad}')
        && segment.text.chars().count() >= 10
        && segment.text.chars().all(|ch| ch.is_alphabetic())
        && (paragraph_style.hyphenate_caps || !is_all_caps_word(&segment.text))
}

pub(in crate::layout) fn apply_emergency_word_wrapping(
    segments: Vec<FlowRun>,
    content_width: f32,
    paragraph_style: &ParagraphStyle,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> Vec<FlowRun> {
    let max_line_width = paragraph_line_width(content_width, paragraph_style, false)
        .max(paragraph_line_width(content_width, paragraph_style, true));
    let mut output = Vec::new();

    for segment in segments {
        if segment.tab_stop_position.is_some()
            || segment.text == "\t"
            || segment.text == "\n"
            || contains_inline_marker(&segment.text)
            || segment.text.chars().count() < 2
            || segment.text.chars().any(char::is_whitespace)
        {
            output.push(segment);
            continue;
        }
        let display = display_text(&segment.text, &segment.style);
        let family = font_family_for_run_text(document, &segment.style, &display);
        if measure_text_with_document_font(
            &display,
            &segment.style,
            family,
            document,
            font_provider,
        ) <= max_line_width
        {
            output.push(segment);
            continue;
        }

        push_emergency_wrapped_segment(
            &mut output,
            segment,
            max_line_width,
            family,
            document,
            font_provider,
        );
    }

    output
}

pub(in crate::layout) fn apply_no_wrap_emergency_boundaries(
    segments: Vec<FlowRun>,
    content_width: f32,
    paragraph_style: &ParagraphStyle,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> Vec<FlowRun> {
    if !segments.iter().any(|segment| segment.text == "\n") {
        let mut measured_width = 0.0;
        for segment in &segments {
            measured_width += measure_flow_run(
                segment,
                measured_width,
                paragraph_style,
                document,
                font_provider,
            );
        }
        let max_line_width = paragraph_line_width(content_width, paragraph_style, false)
            .max(paragraph_line_width(content_width, paragraph_style, true));
        if measured_width <= max_line_width + 0.01 {
            return segments;
        }
    }

    let mut output = Vec::new();
    for segment in segments {
        if segment.style.fit_text_width_twips.is_some()
            || segment.tab_stop_position.is_some()
            || segment.text == "\t"
            || segment.text == "\n"
            || contains_inline_marker(&segment.text)
            || segment.text.chars().count() < 2
        {
            output.push(segment);
            continue;
        }

        let chars = segment.text.chars().collect::<Vec<_>>();
        let last = chars.len() - 1;
        for (index, ch) in chars.into_iter().enumerate() {
            let mut character = segment.clone();
            character.text = ch.to_string();
            character.width = 0.0;
            character.soft_hyphen_after = index == last && segment.soft_hyphen_after;
            output.push(character);
        }
    }
    output
}

pub(in crate::layout) fn take_trailing_whitespace(
    line: &mut Line,
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> Vec<FlowRun> {
    let mut carried = Vec::new();
    while line.runs.last().is_some_and(|run| {
        run.tab_stop_position.is_none()
            && run.text != "\t"
            && run.text.chars().all(char::is_whitespace)
    }) {
        let run = line.runs.pop().expect("trailing run exists");
        line.width = (line.width - run.width).max(0.0);
        carried.push(run);
    }
    carried.reverse();
    line.natural_height = natural_line_height_for_runs(&line.runs, document, font_provider);
    line.height = line.natural_height;
    carried
}

pub(in crate::layout) fn natural_line_height_for_runs(
    runs: &[FlowRun],
    document: &Document,
    font_provider: Option<&FontProvider>,
) -> f32 {
    let has_passive_checkbox = runs
        .iter()
        .any(|run| run.text.chars().any(is_passive_checkbox_char));
    if has_passive_checkbox {
        return runs
            .iter()
            .filter(|run| {
                inline_image_marker_index(&run.text).is_none()
                    && !is_passive_advance_marker(&run.text)
                    && !is_passive_math_inline_lead_marker(&run.text)
            })
            .map(|run| {
                if is_passive_math_fraction_rule_marker(&run.text) {
                    run.style.font_size_points() * PASSIVE_MATH_FRACTION_LINE_HEIGHT_MULTIPLIER
                } else {
                    (run.line_height_points * 1.15)
                        + run.style.baseline_shift_points().abs()
                        + (character_border_inset(&run.style) * 2.0)
                }
            })
            .filter(|height| *height > 0.0)
            .reduce(f32::max)
            .unwrap_or(14.0);
    }
    runs.iter()
        .map(|run| flow_run_line_height(run, document, font_provider))
        .filter(|height| *height > 0.0)
        .reduce(f32::max)
        .unwrap_or(14.0)
}

pub(in crate::layout) fn push_emergency_wrapped_segment(
    output: &mut Vec<FlowRun>,
    segment: FlowRun,
    max_line_width: f32,
    family: PdfFontFamily,
    document: &Document,
    font_provider: Option<&FontProvider>,
) {
    let chars = segment.text.chars().collect::<Vec<_>>();
    let mut start = 0;
    while start < chars.len() {
        let mut end = chars.len();
        while end > start + 1 {
            let text = chars[start..end].iter().collect::<String>();
            if measure_text_with_document_font(
                &text,
                &segment.style,
                family,
                document,
                font_provider,
            ) <= max_line_width
            {
                break;
            }
            end -= 1;
        }
        let is_last = end == chars.len();
        push_flow_run(
            output,
            &chars[start..end].iter().collect::<String>(),
            &segment.style,
            is_last && segment.soft_hyphen_after,
        );
        start = end;
    }
}

pub(in crate::layout) fn is_all_caps_word(text: &str) -> bool {
    let mut has_uppercase = false;
    for ch in text.chars().filter(|ch| ch.is_alphabetic()) {
        if ch.is_lowercase() {
            return false;
        }
        if ch.is_uppercase() {
            has_uppercase = true;
        }
    }
    has_uppercase
}

pub(in crate::layout) fn push_auto_hyphenated_segment(
    output: &mut Vec<FlowRun>,
    segment: FlowRun,
    max_line_width: f32,
    family: PdfFontFamily,
    document: &Document,
    font_provider: Option<&FontProvider>,
    _max_consecutive_hyphenated: Option<usize>,
    consecutive_hyphenated: &mut usize,
) {
    // Without a dictionary, Word's visible recovery is the same bounded
    // character-boundary emergency wrap used for an ordinary oversized word.
    // Do not reserve an artificial three-character suffix: Word may leave one
    // final character when that is the widest fitting split.
    push_emergency_wrapped_segment(
        output,
        segment,
        max_line_width,
        family,
        document,
        font_provider,
    );
    *consecutive_hyphenated = 0;
}

pub(in crate::layout) fn split_run_for_wrapping(
    run: &Run,
    markers: &MarkerContext,
) -> Vec<FlowRun> {
    let mut output = Vec::new();
    if run.style.hidden {
        return output;
    }
    if let Some(field) = run.dynamic_field {
        let text = dynamic_field_layout_text(field, markers);
        let first = output.len();
        push_display_text_segment(&mut output, text, &run.style, false);
        for segment in &mut output[first..] {
            segment.dynamic_field = Some(field);
        }
        return output;
    }
    let mut start = 0;
    let resolved_text;
    let text = if contains_inline_marker(&run.text) {
        resolved_text = run.text.replace(NESTED_TABLE_ANCHOR_MARKER, "");
        resolved_text.as_str()
    } else {
        run.text.as_str()
    };

    // Word treats a character-bordered run as one inline box. Keeping its
    // authored text together here prevents the ordinary whitespace splitter
    // from drawing a separate border around every word. Oversized boxes still
    // pass through the bounded emergency-splitting path during wrapping.
    if run.style.border.visible
        && !contains_inline_marker(text)
        && !text.contains(['\t', '\n', '\r'])
    {
        push_display_text_segment(&mut output, text, &run.style, false);
        return output;
    }

    for (idx, opportunity) in linebreaks(text) {
        if matches!(
            opportunity,
            BreakOpportunity::Allowed | BreakOpportunity::Mandatory
        ) {
            let mandatory = matches!(opportunity, BreakOpportunity::Mandatory);
            if run.style.form_field_shading && !mandatory {
                continue;
            }
            let raw_segment_text = &text[start..idx];
            let segment_text = if mandatory {
                raw_segment_text.trim_end_matches(|ch| {
                    matches!(ch, '\r' | '\n' | '\u{0085}' | '\u{2028}' | '\u{2029}')
                })
            } else {
                raw_segment_text
            };
            let has_break_terminator = segment_text.len() != raw_segment_text.len();
            if !segment_text.is_empty() {
                let soft_hyphen_after = segment_text.ends_with('\u{00ad}');
                push_text_segments_preserving_tabs(
                    &mut output,
                    segment_text,
                    &run.style,
                    soft_hyphen_after,
                );
            }
            if mandatory && (idx < text.len() || has_break_terminator) {
                output.push(FlowRun {
                    dynamic_field: None,
                    text: "\n".to_string(),
                    style: run.style.clone(),
                    width: 0.0,
                    line_height_points: run.style.font_size_points(),
                    tab_leader: TabLeader::None,
                    tab_alignment: TabAlignment::Left,
                    tab_stop_position: None,
                    soft_hyphen_after: false,
                });
            }
            start = idx;
        }
    }

    if start < text.len() {
        push_text_segments_preserving_tabs(&mut output, &text[start..], &run.style, false);
    }

    output
}

pub(in crate::layout) fn dynamic_field_layout_text(
    field: DynamicField,
    markers: &MarkerContext,
) -> &str {
    match field {
        DynamicField::PageNumber => &markers.page_number,
        DynamicField::TotalPages | DynamicField::SectionPages => LATE_PAGE_COUNT_LAYOUT_PLACEHOLDER,
        DynamicField::SectionNumber => &markers.section_number,
        DynamicField::DocumentWords => &markers.document_words,
        DynamicField::DocumentCharacters => &markers.document_chars,
        DynamicField::DocumentCharactersWithSpaces => &markers.document_chars_with_spaces,
    }
}

pub(in crate::layout) fn current_marker_context(
    pages: &[LayoutPage],
    document_stats: DocumentStats,
) -> MarkerContext {
    pages.last().map_or_else(
        || marker_context("1".to_string(), "1".to_string(), document_stats),
        |page| {
            marker_context(
                page.display_page_number.clone(),
                page.section_number.to_string(),
                document_stats,
            )
        },
    )
}

pub(in crate::layout) fn marker_context(
    page_number: String,
    section_number: String,
    document_stats: DocumentStats,
) -> MarkerContext {
    MarkerContext {
        page_number,
        section_number,
        document_words: document_stats.words.to_string(),
        document_chars: document_stats.chars.to_string(),
        document_chars_with_spaces: document_stats.chars_with_spaces.to_string(),
    }
}

pub(in crate::layout) fn contains_inline_marker(text: &str) -> bool {
    text.contains(FOOTNOTE_REFERENCE_MARKER)
        || text.contains(FOOTNOTE_REFERENCE_MARKER_END)
        || text.contains(ENDNOTE_REFERENCE_MARKER)
        || text.contains(ENDNOTE_REFERENCE_MARKER_END)
        || text.contains(PASSIVE_ADVANCE_MARKER)
        || text.contains(PASSIVE_MATH_FRACTION_RULE_MARKER)
        || text.contains(PASSIVE_MATH_INLINE_LEAD_MARKER)
        || text.contains(PASSIVE_MATH_STACK_ANCHOR_MARKER)
        || text.contains(NESTED_TABLE_ANCHOR_MARKER)
}
