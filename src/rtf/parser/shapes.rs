//! Static shape property parsing, normalization, and bounded preset geometry.

use super::*;

pub(in crate::rtf::parser) fn normalize_shape_points(
    points: &[StaticShapePoint],
    max_dimension_twips: i32,
) -> Option<(i32, i32, i32, i32, Vec<StaticShapePoint>)> {
    if points.len() < 2 {
        return None;
    }
    let min_x = points.iter().map(|point| point.x_twips).min()?.max(0);
    let min_y = points.iter().map(|point| point.y_twips).min()?.max(0);
    let max_x = points
        .iter()
        .map(|point| point.x_twips)
        .max()?
        .max(min_x + 1);
    let max_y = points
        .iter()
        .map(|point| point.y_twips)
        .max()?
        .max(min_y + 1);
    let width_twips = max_x.saturating_sub(min_x).clamp(1, max_dimension_twips);
    let height_twips = max_y.saturating_sub(min_y).clamp(1, max_dimension_twips);
    let normalized = points
        .iter()
        .map(|point| StaticShapePoint {
            x_twips: point.x_twips.saturating_sub(min_x).clamp(0, width_twips),
            y_twips: point.y_twips.saturating_sub(min_y).clamp(0, height_twips),
        })
        .collect::<Vec<_>>();
    Some((min_x, min_y, width_twips, height_twips, normalized))
}

pub(in crate::rtf::parser) fn normalize_shape_point_paths(
    paths: &[Vec<StaticShapePoint>],
    base_x_twips: i32,
    base_y_twips: i32,
    left_twips: i32,
    top_twips: i32,
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    normalize_shape_point_paths_with_min_len(
        paths,
        base_x_twips,
        base_y_twips,
        left_twips,
        top_twips,
        width_twips,
        height_twips,
        3,
    )
}

pub(in crate::rtf::parser) fn normalize_shape_overlay_paths(
    paths: &[Vec<StaticShapePoint>],
    base_x_twips: i32,
    base_y_twips: i32,
    left_twips: i32,
    top_twips: i32,
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    normalize_shape_point_paths_with_min_len(
        paths,
        base_x_twips,
        base_y_twips,
        left_twips,
        top_twips,
        width_twips,
        height_twips,
        2,
    )
}

fn normalize_shape_point_paths_with_min_len(
    paths: &[Vec<StaticShapePoint>],
    base_x_twips: i32,
    base_y_twips: i32,
    left_twips: i32,
    top_twips: i32,
    width_twips: i32,
    height_twips: i32,
    min_len: usize,
) -> Vec<Vec<StaticShapePoint>> {
    paths
        .iter()
        .filter(|path| path.len() >= min_len)
        .map(|path| {
            path.iter()
                .map(|point| StaticShapePoint {
                    x_twips: base_x_twips
                        .saturating_add(point.x_twips)
                        .saturating_sub(left_twips)
                        .clamp(0, width_twips),
                    y_twips: base_y_twips
                        .saturating_add(point.y_twips)
                        .saturating_sub(top_twips)
                        .clamp(0, height_twips),
                })
                .collect()
        })
        .collect()
}

pub(in crate::rtf::parser) fn rotated_shape_rectangle_points(
    width_twips: i32,
    height_twips: i32,
    rotation_units: i32,
) -> Vec<StaticShapePoint> {
    [
        (0, 0),
        (width_twips, 0),
        (width_twips, height_twips),
        (0, height_twips),
    ]
    .into_iter()
    .map(|(x, y)| rotated_shape_point(x, y, width_twips, height_twips, rotation_units))
    .collect()
}

pub(in crate::rtf::parser) fn rotated_shape_line_points(
    width_twips: i32,
    height_twips: i32,
    rotation_units: i32,
) -> Vec<StaticShapePoint> {
    [(0, 0), (width_twips, height_twips)]
        .into_iter()
        .map(|(x, y)| rotated_shape_point(x, y, width_twips, height_twips, rotation_units))
        .collect()
}

pub(in crate::rtf::parser) fn polygon_preset_shape_points(
    preset: ShapePolygonPreset,
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    match preset {
        ShapePolygonPreset::Diamond => diamond_shape_points(width_twips, height_twips),
        ShapePolygonPreset::IsoscelesTriangle => {
            isosceles_triangle_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::RightTriangle => right_triangle_shape_points(width_twips, height_twips),
        ShapePolygonPreset::Trapezoid => trapezoid_shape_points(width_twips, height_twips),
        ShapePolygonPreset::Parallelogram => parallelogram_shape_points(width_twips, height_twips),
        ShapePolygonPreset::Octagon => octagon_shape_points(width_twips, height_twips),
        ShapePolygonPreset::Hexagon => hexagon_shape_points(width_twips, height_twips),
        ShapePolygonPreset::Cross => cross_shape_points(width_twips, height_twips),
        ShapePolygonPreset::Pentagon => pentagon_shape_points(width_twips, height_twips),
        ShapePolygonPreset::Can => can_shape_points(width_twips, height_twips),
        ShapePolygonPreset::Cube => cube_shape_points(width_twips, height_twips),
        ShapePolygonPreset::Bevel => bevel_shape_points(width_twips, height_twips),
        ShapePolygonPreset::FoldedCorner => folded_corner_shape_points(width_twips, height_twips),
        ShapePolygonPreset::SmileyFace => {
            regular_polygon_shape_points(width_twips, height_twips, 24)
        }
        ShapePolygonPreset::Donut => donut_shape_points(width_twips, height_twips),
        ShapePolygonPreset::NoSymbol => no_symbol_shape_points(width_twips, height_twips),
        ShapePolygonPreset::Plaque => plaque_shape_points(width_twips, height_twips),
        ShapePolygonPreset::DoubleBrace => double_brace_shape_points(width_twips, height_twips),
        ShapePolygonPreset::DoubleBracket => double_bracket_shape_points(width_twips, height_twips),
        ShapePolygonPreset::LeftBracket => left_bracket_shape_points(width_twips, height_twips),
        ShapePolygonPreset::RightBracket => right_bracket_shape_points(width_twips, height_twips),
        ShapePolygonPreset::LeftBrace => left_brace_shape_points(width_twips, height_twips),
        ShapePolygonPreset::RightBrace => right_brace_shape_points(width_twips, height_twips),
        ShapePolygonPreset::Chevron => chevron_shape_points(width_twips, height_twips),
        ShapePolygonPreset::NotchedRightArrow => {
            notched_right_arrow_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::RightArrowCallout => {
            right_arrow_callout_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::LeftArrowCallout => {
            left_arrow_callout_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::UpArrowCallout => {
            up_arrow_callout_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::DownArrowCallout => {
            down_arrow_callout_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::LeftRightArrowCallout => {
            left_right_arrow_callout_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::UpDownArrowCallout => {
            up_down_arrow_callout_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::QuadArrowCallout => {
            quad_arrow_callout_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::CircularArrow => circular_arrow_shape_points(width_twips, height_twips),
        ShapePolygonPreset::RightArrow => right_arrow_shape_points(width_twips, height_twips),
        ShapePolygonPreset::LeftArrow => left_arrow_shape_points(width_twips, height_twips),
        ShapePolygonPreset::UpArrow => up_arrow_shape_points(width_twips, height_twips),
        ShapePolygonPreset::DownArrow => down_arrow_shape_points(width_twips, height_twips),
        ShapePolygonPreset::LeftRightArrow => {
            left_right_arrow_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::UpDownArrow => up_down_arrow_shape_points(width_twips, height_twips),
        ShapePolygonPreset::QuadArrow => quad_arrow_shape_points(width_twips, height_twips),
        ShapePolygonPreset::LeftRightUpArrow => {
            left_right_up_arrow_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::BentArrow => bent_arrow_shape_points(width_twips, height_twips),
        ShapePolygonPreset::UTurnArrow => uturn_arrow_shape_points(width_twips, height_twips),
        ShapePolygonPreset::LeftUpArrow => left_up_arrow_shape_points(width_twips, height_twips),
        ShapePolygonPreset::BentUpArrow => bent_up_arrow_shape_points(width_twips, height_twips),
        ShapePolygonPreset::CurvedRightArrow => {
            curved_right_arrow_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::CurvedLeftArrow => {
            curved_left_arrow_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::CurvedUpArrow => {
            curved_up_arrow_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::CurvedDownArrow => {
            curved_down_arrow_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::StripedRightArrow => {
            striped_right_arrow_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::Heart => heart_shape_points(width_twips, height_twips),
        ShapePolygonPreset::LightningBolt => lightning_bolt_shape_points(width_twips, height_twips),
        ShapePolygonPreset::Sun => regular_star_shape_points(width_twips, height_twips, 16, 620),
        ShapePolygonPreset::Moon => moon_shape_points(width_twips, height_twips),
        ShapePolygonPreset::Explosion1 => explosion1_shape_points(width_twips, height_twips),
        ShapePolygonPreset::Explosion2 => explosion2_shape_points(width_twips, height_twips),
        ShapePolygonPreset::FourPointStar => {
            regular_star_shape_points(width_twips, height_twips, 4, 350)
        }
        ShapePolygonPreset::FivePointStar => {
            five_point_star_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::SixPointStar => {
            regular_star_shape_points(width_twips, height_twips, 6, 500)
        }
        ShapePolygonPreset::SevenPointStar => {
            regular_star_shape_points(width_twips, height_twips, 7, 520)
        }
        ShapePolygonPreset::EightPointStar => {
            eight_point_star_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::TenPointStar => {
            regular_star_shape_points(width_twips, height_twips, 10, 580)
        }
        ShapePolygonPreset::TwelvePointStar => {
            regular_star_shape_points(width_twips, height_twips, 12, 600)
        }
        ShapePolygonPreset::SixteenPointStar => {
            regular_star_shape_points(width_twips, height_twips, 16, 650)
        }
        ShapePolygonPreset::TwentyFourPointStar => {
            regular_star_shape_points(width_twips, height_twips, 24, 700)
        }
        ShapePolygonPreset::ThirtyTwoPointStar => {
            regular_star_shape_points(width_twips, height_twips, 32, 740)
        }
        ShapePolygonPreset::ManualInput => manual_input_shape_points(width_twips, height_twips),
        ShapePolygonPreset::ManualOperation => {
            manual_operation_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::FlowchartPredefinedProcess => {
            rectangle_polygon_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::FlowchartInternalStorage => {
            rectangle_polygon_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::FlowchartDocument => {
            flowchart_document_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::FlowchartMultidocument => {
            flowchart_multidocument_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::FlowchartPunchedCard => {
            flowchart_punched_card_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::FlowchartPunchedTape => {
            flowchart_punched_tape_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::FlowchartSummingJunction => {
            regular_polygon_shape_points(width_twips, height_twips, 24)
        }
        ShapePolygonPreset::FlowchartOr => {
            regular_polygon_shape_points(width_twips, height_twips, 24)
        }
        ShapePolygonPreset::FlowchartCollate => {
            flowchart_collate_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::FlowchartSort => flowchart_sort_shape_points(width_twips, height_twips),
        ShapePolygonPreset::FlowchartStoredData => can_shape_points(width_twips, height_twips),
        ShapePolygonPreset::FlowchartDelay => {
            flowchart_delay_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::FlowchartSequentialAccessStorage => {
            flowchart_sequential_access_storage_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::FlowchartMagneticDisk => can_shape_points(width_twips, height_twips),
        ShapePolygonPreset::FlowchartDirectAccessStorage => {
            flowchart_direct_access_storage_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::FlowchartDisplay => {
            flowchart_display_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::UpRibbon => up_ribbon_shape_points(width_twips, height_twips),
        ShapePolygonPreset::DownRibbon => down_ribbon_shape_points(width_twips, height_twips),
        ShapePolygonPreset::CurvedUpRibbon => {
            curved_up_ribbon_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::CurvedDownRibbon => {
            curved_down_ribbon_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::VerticalScroll => {
            vertical_scroll_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::HorizontalScroll => {
            horizontal_scroll_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::Wave => wave_shape_points(width_twips, height_twips),
        ShapePolygonPreset::DoubleWave => double_wave_shape_points(width_twips, height_twips),
        ShapePolygonPreset::RectangularCallout => {
            rectangular_callout_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::RoundedRectangularCallout => {
            rounded_rectangular_callout_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::OvalCallout => oval_callout_shape_points(width_twips, height_twips),
        ShapePolygonPreset::CloudCallout => cloud_callout_shape_points(width_twips, height_twips),
        ShapePolygonPreset::LineCalloutBorder | ShapePolygonPreset::LineCalloutBorderAccentBar => {
            line_callout_border_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::ActionButtonBlank => {
            action_button_blank_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::ActionButtonHome => {
            action_button_home_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::ActionButtonHelp => {
            action_button_help_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::ActionButtonInformation => {
            action_button_information_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::ActionButtonBackPrevious => {
            action_button_back_previous_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::ActionButtonForwardNext => {
            action_button_forward_next_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::ActionButtonBeginning => {
            action_button_beginning_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::ActionButtonEnd => {
            action_button_end_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::ActionButtonReturn => {
            action_button_return_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::ActionButtonDocument => {
            action_button_document_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::ActionButtonSound => {
            action_button_sound_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::ActionButtonMovie => {
            action_button_movie_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::Balloon => balloon_shape_points(width_twips, height_twips),
        ShapePolygonPreset::FlowchartOfflineStorage => {
            flowchart_offline_storage_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::LeftRightRibbon => {
            left_right_ribbon_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::DiagonalStripe => {
            diagonal_stripe_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::Pie => pie_shape_points(width_twips, height_twips),
        ShapePolygonPreset::NonIsoscelesTrapezoid => {
            non_isosceles_trapezoid_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::Decagon => regular_polygon_shape_points(width_twips, height_twips, 10),
        ShapePolygonPreset::Heptagon => regular_polygon_shape_points(width_twips, height_twips, 7),
        ShapePolygonPreset::Dodecagon => {
            regular_polygon_shape_points(width_twips, height_twips, 12)
        }
        ShapePolygonPreset::RoundOneRectangle => {
            round_one_rectangle_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::RoundTwoSameRectangle => {
            round_two_same_rectangle_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::RoundTwoDiagonalRectangle => {
            round_two_diagonal_rectangle_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::SnipRoundRectangle => {
            snip_round_rectangle_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::SnipOneRectangle => {
            snip_one_rectangle_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::SnipTwoSameRectangle => {
            snip_two_same_rectangle_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::SnipTwoDiagonalRectangle => {
            snip_two_diagonal_rectangle_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::Frame => rectangle_polygon_shape_points(width_twips, height_twips),
        ShapePolygonPreset::HalfFrame => half_frame_shape_points(width_twips, height_twips),
        ShapePolygonPreset::Tear => tear_shape_points(width_twips, height_twips),
        ShapePolygonPreset::Chord => chord_shape_points(width_twips, height_twips),
        ShapePolygonPreset::Corner => corner_shape_points(width_twips, height_twips),
        ShapePolygonPreset::MathPlus => math_plus_shape_points(width_twips, height_twips),
        ShapePolygonPreset::MathMinus => math_minus_shape_points(width_twips, height_twips),
        ShapePolygonPreset::MathMultiply => math_multiply_shape_points(width_twips, height_twips),
        ShapePolygonPreset::MathDivide => math_divide_shape_points(width_twips, height_twips),
        ShapePolygonPreset::MathEqual => math_equal_shape_points(width_twips, height_twips),
        ShapePolygonPreset::MathNotEqual => math_not_equal_shape_points(width_twips, height_twips),
        ShapePolygonPreset::CornerTabs => corner_tabs_shape_points(width_twips, height_twips),
        ShapePolygonPreset::SquareTabs => square_tabs_shape_points(width_twips, height_twips),
        ShapePolygonPreset::PlaqueTabs => plaque_tabs_shape_points(width_twips, height_twips),
        ShapePolygonPreset::Gear6 => gear_shape_points(width_twips, height_twips, 6, 620),
        ShapePolygonPreset::Gear9 => gear_shape_points(width_twips, height_twips, 9, 650),
        ShapePolygonPreset::Funnel => funnel_shape_points(width_twips, height_twips),
        ShapePolygonPreset::PieWedge => pie_wedge_shape_points(width_twips, height_twips),
        ShapePolygonPreset::LeftCircularArrow => {
            left_circular_arrow_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::LeftRightCircularArrow => {
            left_right_circular_arrow_shape_points(width_twips, height_twips)
        }
        ShapePolygonPreset::SwooshArrow => swoosh_arrow_shape_points(width_twips, height_twips),
        ShapePolygonPreset::Cloud => cloud_shape_points(width_twips, height_twips),
        ShapePolygonPreset::ChartX => chart_x_shape_points(width_twips, height_twips),
        ShapePolygonPreset::ChartStar => chart_star_shape_points(width_twips, height_twips),
        ShapePolygonPreset::ChartPlus => chart_plus_shape_points(width_twips, height_twips),
        ShapePolygonPreset::DownTriangle => down_triangle_shape_points(width_twips, height_twips),
    }
}

pub(in crate::rtf::parser) fn polygon_preset_shape_point_paths(
    preset: ShapePolygonPreset,
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    match preset {
        ShapePolygonPreset::Donut => donut_shape_point_paths(width_twips, height_twips),
        ShapePolygonPreset::NoSymbol => no_symbol_shape_point_paths(width_twips, height_twips),
        ShapePolygonPreset::Frame => frame_shape_point_paths(width_twips, height_twips),
        ShapePolygonPreset::MathDivide => math_divide_shape_point_paths(width_twips, height_twips),
        ShapePolygonPreset::MathEqual => math_equal_shape_point_paths(width_twips, height_twips),
        ShapePolygonPreset::MathNotEqual => {
            math_not_equal_shape_point_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::ActionButtonHelp => {
            action_button_help_shape_point_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::ActionButtonBackPrevious => {
            action_button_back_previous_shape_point_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::ActionButtonForwardNext => {
            action_button_forward_next_shape_point_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::CornerTabs => corner_tabs_shape_point_paths(width_twips, height_twips),
        ShapePolygonPreset::SquareTabs => square_tabs_shape_point_paths(width_twips, height_twips),
        ShapePolygonPreset::PlaqueTabs => plaque_tabs_shape_point_paths(width_twips, height_twips),
        ShapePolygonPreset::ChartX => chart_x_shape_point_paths(width_twips, height_twips),
        ShapePolygonPreset::ChartStar => chart_star_shape_point_paths(width_twips, height_twips),
        ShapePolygonPreset::ChartPlus => chart_plus_shape_point_paths(width_twips, height_twips),
        ShapePolygonPreset::Gear6 => gear_shape_point_paths(width_twips, height_twips, 6),
        ShapePolygonPreset::Gear9 => gear_shape_point_paths(width_twips, height_twips, 9),
        ShapePolygonPreset::ActionButtonBeginning => {
            action_button_beginning_shape_point_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::ActionButtonEnd => {
            action_button_end_shape_point_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::ActionButtonReturn => {
            action_button_return_shape_point_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::ActionButtonInformation => {
            action_button_information_shape_point_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::ActionButtonSound => {
            action_button_sound_shape_point_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::ActionButtonMovie => {
            action_button_movie_shape_point_paths(width_twips, height_twips)
        }
        _ => Vec::new(),
    }
}

pub(in crate::rtf::parser) fn polygon_preset_shape_fill_rule(
    preset: ShapePolygonPreset,
) -> StaticImageVectorFillRule {
    match preset {
        ShapePolygonPreset::Donut
        | ShapePolygonPreset::NoSymbol
        | ShapePolygonPreset::Frame
        | ShapePolygonPreset::Gear6
        | ShapePolygonPreset::Gear9 => StaticImageVectorFillRule::Alternate,
        _ => StaticImageVectorFillRule::Winding,
    }
}

pub(in crate::rtf::parser) fn polygon_preset_shape_overlay_paths(
    preset: ShapePolygonPreset,
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    match preset {
        ShapePolygonPreset::SmileyFace => smiley_face_overlay_paths(width_twips, height_twips),
        ShapePolygonPreset::NoSymbol => no_symbol_shape_overlay_paths(width_twips, height_twips),
        ShapePolygonPreset::Sun => sun_shape_overlay_paths(width_twips, height_twips),
        ShapePolygonPreset::Can
        | ShapePolygonPreset::FlowchartStoredData
        | ShapePolygonPreset::FlowchartMagneticDisk => {
            can_shape_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::Cube => cube_shape_overlay_paths(width_twips, height_twips),
        ShapePolygonPreset::Bevel => bevel_shape_overlay_paths(width_twips, height_twips),
        ShapePolygonPreset::ActionButtonBlank
        | ShapePolygonPreset::ActionButtonHome
        | ShapePolygonPreset::ActionButtonHelp
        | ShapePolygonPreset::ActionButtonInformation
        | ShapePolygonPreset::ActionButtonBackPrevious
        | ShapePolygonPreset::ActionButtonForwardNext
        | ShapePolygonPreset::ActionButtonBeginning
        | ShapePolygonPreset::ActionButtonEnd
        | ShapePolygonPreset::ActionButtonReturn
        | ShapePolygonPreset::ActionButtonSound
        | ShapePolygonPreset::ActionButtonMovie => {
            action_button_frame_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::FlowchartPredefinedProcess => {
            flowchart_predefined_process_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::FlowchartInternalStorage => {
            flowchart_internal_storage_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::FlowchartMultidocument => {
            flowchart_multidocument_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::FlowchartSummingJunction => {
            flowchart_summing_junction_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::FlowchartOr => flowchart_or_overlay_paths(width_twips, height_twips),
        ShapePolygonPreset::FlowchartSort => {
            flowchart_sort_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::UpRibbon => up_ribbon_overlay_paths(width_twips, height_twips),
        ShapePolygonPreset::DownRibbon => down_ribbon_overlay_paths(width_twips, height_twips),
        ShapePolygonPreset::CurvedUpRibbon => {
            curved_up_ribbon_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::CurvedDownRibbon => {
            curved_down_ribbon_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::VerticalScroll => {
            vertical_scroll_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::HorizontalScroll => {
            horizontal_scroll_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::LeftRightRibbon => {
            left_right_ribbon_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::DiagonalStripe => {
            diagonal_stripe_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::RightArrowCallout => {
            right_arrow_callout_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::LeftArrowCallout => {
            left_arrow_callout_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::UpArrowCallout => {
            up_arrow_callout_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::DownArrowCallout => {
            down_arrow_callout_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::LeftRightArrowCallout => {
            left_right_arrow_callout_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::UpDownArrowCallout => {
            up_down_arrow_callout_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::QuadArrowCallout => {
            quad_arrow_callout_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::FoldedCorner => {
            folded_corner_shape_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::ActionButtonDocument => {
            action_button_document_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::LineCalloutBorderAccentBar => {
            line_callout_border_accent_bar_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::CircularArrow => {
            circular_arrow_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::LeftCircularArrow => {
            left_circular_arrow_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::LeftRightCircularArrow => {
            left_right_circular_arrow_overlay_paths(width_twips, height_twips)
        }
        ShapePolygonPreset::StripedRightArrow => {
            striped_right_arrow_overlay_paths(width_twips, height_twips)
        }
        _ => Vec::new(),
    }
}

pub(in crate::rtf::parser) fn polyline_preset_shape_points(
    preset: ShapePolylinePreset,
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    match preset {
        ShapePolylinePreset::Arc => arc_shape_points(width_twips, height_twips),
        ShapePolylinePreset::LineCalloutHorizontal => {
            line_callout_horizontal_shape_points(width_twips, height_twips)
        }
        ShapePolylinePreset::LineCalloutHorizontalAccentBar => {
            line_callout_horizontal_shape_points(width_twips, height_twips)
        }
        ShapePolylinePreset::LineCalloutDiagonal => {
            line_callout_diagonal_shape_points(width_twips, height_twips)
        }
        ShapePolylinePreset::LineCalloutDiagonalAccentBar => {
            line_callout_diagonal_shape_points(width_twips, height_twips)
        }
        ShapePolylinePreset::LineCalloutAngled => {
            line_callout_angled_shape_points(width_twips, height_twips)
        }
        ShapePolylinePreset::LineCalloutAngledAccentBar => {
            line_callout_angled_shape_points(width_twips, height_twips)
        }
        ShapePolylinePreset::LineCalloutUShape => {
            line_callout_ushape_points(width_twips, height_twips)
        }
        ShapePolylinePreset::LineCalloutUShapeAccentBar => {
            line_callout_ushape_points(width_twips, height_twips)
        }
        ShapePolylinePreset::LineInverse => line_inverse_shape_points(width_twips, height_twips),
    }
}

pub(in crate::rtf::parser) fn polyline_preset_shape_overlay_paths(
    preset: ShapePolylinePreset,
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    match preset {
        ShapePolylinePreset::LineCalloutHorizontalAccentBar
        | ShapePolylinePreset::LineCalloutDiagonalAccentBar
        | ShapePolylinePreset::LineCalloutAngledAccentBar
        | ShapePolylinePreset::LineCalloutUShapeAccentBar => {
            line_callout_border_accent_bar_overlay_paths(width_twips, height_twips)
        }
        _ => Vec::new(),
    }
}

fn arc_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let center_x = f64::from(width_twips) / 2.0;
    let center_y = f64::from(height_twips) / 2.0;
    let radius_x = center_x;
    let radius_y = center_y;

    const ARC_SEGMENTS: usize = 48;

    (0..=ARC_SEGMENTS)
        .map(|index| {
            let angle =
                std::f64::consts::PI + (index as f64 * std::f64::consts::PI / ARC_SEGMENTS as f64);
            let x = (center_x + radius_x * angle.cos())
                .round()
                .clamp(0.0, f64::from(width_twips)) as i32;
            let y = (center_y + radius_y * angle.sin())
                .round()
                .clamp(0.0, f64::from(height_twips)) as i32;
            StaticShapePoint {
                x_twips: x,
                y_twips: y,
            }
        })
        .collect()
}

fn diamond_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let mid_x = width_twips / 2;
    let mid_y = height_twips / 2;
    [
        (mid_x, 0),
        (width_twips, mid_y),
        (mid_x, height_twips),
        (0, mid_y),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn isosceles_triangle_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let mid_x = width_twips / 2;
    [(mid_x, 0), (width_twips, height_twips), (0, height_twips)]
        .into_iter()
        .map(|(x, y)| StaticShapePoint {
            x_twips: x,
            y_twips: y,
        })
        .collect()
}

fn right_triangle_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    [(0, 0), (width_twips, height_twips), (0, height_twips)]
        .into_iter()
        .map(|(x, y)| StaticShapePoint {
            x_twips: x,
            y_twips: y,
        })
        .collect()
}

fn down_triangle_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    [(0, 0), (width_twips, 0), (width_twips / 2, height_twips)]
        .into_iter()
        .map(|(x, y)| StaticShapePoint {
            x_twips: x,
            y_twips: y,
        })
        .collect()
}

fn rectangle_polygon_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    [
        (0, 0),
        (width_twips, 0),
        (width_twips, height_twips),
        (0, height_twips),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn rounded_rectangle_polygon_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    let radius = (width_twips.min(height_twips) / 6).max(1);
    let right = width_twips.max(1);
    let bottom = height_twips.max(1);
    let samples: [(i32, i32); 9] = [
        (0, 1000),
        (20, 805),
        (76, 617),
        (169, 444),
        (293, 293),
        (444, 169),
        (617, 76),
        (805, 20),
        (1000, 0),
    ];
    let mut points = Vec::with_capacity(40);
    points.push(StaticShapePoint {
        x_twips: radius,
        y_twips: 0,
    });
    points.push(StaticShapePoint {
        x_twips: right.saturating_sub(radius),
        y_twips: 0,
    });
    for (sin, cos) in samples {
        points.push(StaticShapePoint {
            x_twips: right
                .saturating_sub(radius)
                .saturating_add((radius * sin) / 1000),
            y_twips: radius.saturating_sub((radius * cos) / 1000),
        });
    }
    points.push(StaticShapePoint {
        x_twips: right,
        y_twips: bottom.saturating_sub(radius),
    });
    for (sin, cos) in samples {
        points.push(StaticShapePoint {
            x_twips: right.saturating_sub((radius * cos) / 1000),
            y_twips: bottom
                .saturating_sub(radius)
                .saturating_add((radius * sin) / 1000),
        });
    }
    points.push(StaticShapePoint {
        x_twips: radius,
        y_twips: bottom,
    });
    for (sin, cos) in samples {
        points.push(StaticShapePoint {
            x_twips: radius.saturating_sub((radius * sin) / 1000),
            y_twips: bottom.saturating_sub((radius * cos) / 1000),
        });
    }
    points.push(StaticShapePoint {
        x_twips: 0,
        y_twips: radius,
    });
    for (sin, cos) in samples {
        points.push(StaticShapePoint {
            x_twips: (radius * cos) / 1000,
            y_twips: radius.saturating_sub((radius * sin) / 1000),
        });
    }
    points
}

pub(in crate::rtf::parser) fn rotated_rounded_rectangle_shape_points(
    width_twips: i32,
    height_twips: i32,
    rotation_units: i32,
) -> Vec<StaticShapePoint> {
    rounded_rectangle_polygon_shape_points(width_twips, height_twips)
        .into_iter()
        .map(|point| {
            rotated_shape_point(
                point.x_twips,
                point.y_twips,
                width_twips,
                height_twips,
                rotation_units,
            )
        })
        .collect()
}

fn flowchart_predefined_process_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    let left_x = width_twips / 8;
    let right_x = width_twips - left_x;
    vec![
        vec![
            StaticShapePoint {
                x_twips: left_x,
                y_twips: 0,
            },
            StaticShapePoint {
                x_twips: left_x,
                y_twips: height_twips,
            },
        ],
        vec![
            StaticShapePoint {
                x_twips: right_x,
                y_twips: 0,
            },
            StaticShapePoint {
                x_twips: right_x,
                y_twips: height_twips,
            },
        ],
    ]
}

fn flowchart_internal_storage_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    let inset_x = width_twips / 8;
    let inset_y = height_twips / 5;
    vec![
        vec![
            StaticShapePoint {
                x_twips: inset_x,
                y_twips: 0,
            },
            StaticShapePoint {
                x_twips: inset_x,
                y_twips: height_twips,
            },
        ],
        vec![
            StaticShapePoint {
                x_twips: 0,
                y_twips: inset_y,
            },
            StaticShapePoint {
                x_twips: width_twips,
                y_twips: inset_y,
            },
        ],
    ]
}

fn flowchart_document_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let wave_top = (height_twips * 4) / 5;
    let wave_mid = (height_twips * 9) / 10;
    [
        (0, 0),
        (width_twips, 0),
        (width_twips, wave_top),
        ((width_twips * 31) / 32, (wave_top * 3 + wave_mid) / 4),
        ((width_twips * 15) / 16, (wave_top + wave_mid) / 2),
        ((width_twips * 29) / 32, (wave_top + wave_mid * 3) / 4),
        ((width_twips * 7) / 8, wave_mid),
        ((width_twips * 13) / 16, (wave_mid + height_twips) / 2),
        ((width_twips * 3) / 4, height_twips),
        ((width_twips * 11) / 16, (wave_mid + height_twips) / 2),
        ((width_twips * 5) / 8, wave_mid),
        ((width_twips * 19) / 32, (wave_top + wave_mid * 3) / 4),
        ((width_twips * 9) / 16, (wave_top + wave_mid) / 2),
        ((width_twips * 17) / 32, (wave_top * 3 + wave_mid) / 4),
        (width_twips / 2, wave_top),
        ((width_twips * 15) / 32, (wave_top * 3 + wave_mid) / 4),
        ((width_twips * 7) / 16, (wave_top + wave_mid) / 2),
        ((width_twips * 13) / 32, (wave_top + wave_mid * 3) / 4),
        ((width_twips * 3) / 8, wave_mid),
        ((width_twips * 5) / 16, (wave_mid + height_twips) / 2),
        (width_twips / 4, height_twips),
        ((width_twips * 3) / 16, (wave_mid + height_twips) / 2),
        (width_twips / 8, wave_mid),
        ((width_twips * 3) / 32, (wave_top + wave_mid * 3) / 4),
        (width_twips / 16, (wave_top + wave_mid) / 2),
        (width_twips / 32, (wave_top * 3 + wave_mid) / 4),
        (0, wave_top),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn flowchart_multidocument_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    let offset_x = width_twips / 6;
    let offset_y = height_twips / 5;
    let wave_top = (height_twips * 4) / 5;
    let wave_mid = (height_twips * 9) / 10;
    [
        (offset_x, 0),
        (width_twips, 0),
        (width_twips, wave_top),
        ((width_twips * 47) / 48, (wave_top * 3 + wave_mid) / 4),
        ((width_twips * 23) / 24, (wave_top + wave_mid) / 2),
        ((width_twips * 45) / 48, (wave_top + wave_mid * 3) / 4),
        ((width_twips * 11) / 12, wave_mid),
        ((width_twips * 7) / 8, (wave_mid + height_twips) / 2),
        ((width_twips * 5) / 6, height_twips),
        ((width_twips * 19) / 24, (wave_mid + height_twips) / 2),
        ((width_twips * 3) / 4, wave_mid),
        ((width_twips * 35) / 48, (wave_top + wave_mid * 3) / 4),
        ((width_twips * 17) / 24, (wave_top + wave_mid) / 2),
        ((width_twips * 33) / 48, (wave_top * 3 + wave_mid) / 4),
        ((width_twips * 2) / 3, wave_top),
        ((width_twips * 31) / 48, (wave_top * 3 + wave_mid) / 4),
        ((width_twips * 5) / 8, (wave_top + wave_mid) / 2),
        ((width_twips * 29) / 48, (wave_top + wave_mid * 3) / 4),
        ((width_twips * 7) / 12, wave_mid),
        ((width_twips * 13) / 24, (wave_mid + height_twips) / 2),
        (width_twips / 2, height_twips),
        ((width_twips * 11) / 24, (wave_mid + height_twips) / 2),
        ((width_twips * 5) / 12, wave_mid),
        ((width_twips * 19) / 48, (wave_top + wave_mid * 3) / 4),
        ((width_twips * 3) / 8, (wave_top + wave_mid) / 2),
        ((width_twips * 17) / 48, (wave_top * 3 + wave_mid) / 4),
        (width_twips / 3, wave_top),
        ((width_twips * 7) / 24, (wave_top + wave_mid) / 2),
        (width_twips / 4, wave_mid),
        ((width_twips * 5) / 24, (wave_top + wave_mid) / 2),
        (offset_x, wave_top),
        (offset_x, offset_y),
        (0, offset_y),
        (0, height_twips),
        (offset_x, height_twips),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn flowchart_multidocument_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    let offset_x = width_twips / 6;
    let offset_y = height_twips / 5;
    let wave_top = (height_twips * 4) / 5;
    vec![
        vec![
            StaticShapePoint {
                x_twips: offset_x,
                y_twips: offset_y,
            },
            StaticShapePoint {
                x_twips: offset_x,
                y_twips: wave_top,
            },
        ],
        vec![
            StaticShapePoint {
                x_twips: offset_x,
                y_twips: height_twips,
            },
            StaticShapePoint {
                x_twips: 0,
                y_twips: height_twips,
            },
        ],
    ]
}

fn flowchart_punched_card_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    let cut = width_twips / 5;
    [
        (cut, 0),
        (width_twips, 0),
        (width_twips, height_twips),
        (0, height_twips),
        (0, cut.min(height_twips)),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn flowchart_punched_tape_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (0, 120),
            (62, 82),
            (125, 60),
            (188, 22),
            (250, 0),
            (312, 22),
            (375, 60),
            (438, 98),
            (500, 120),
            (562, 98),
            (625, 60),
            (688, 22),
            (750, 0),
            (812, 22),
            (875, 60),
            (938, 82),
            (1000, 120),
            (1000, 880),
            (938, 918),
            (875, 940),
            (812, 978),
            (750, 1000),
            (688, 978),
            (625, 940),
            (562, 902),
            (500, 880),
            (438, 902),
            (375, 940),
            (312, 978),
            (250, 1000),
            (188, 978),
            (125, 940),
            (62, 918),
            (0, 880),
        ],
    )
}

fn flowchart_collate_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    [
        (0, 0),
        (width_twips, 0),
        (width_twips / 2, height_twips / 2),
        (width_twips, height_twips),
        (0, height_twips),
        (width_twips / 2, height_twips / 2),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn flowchart_summing_junction_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![
        scaled_shape_points(width_twips, height_twips, &[(146, 146), (854, 854)]),
        scaled_shape_points(width_twips, height_twips, &[(854, 146), (146, 854)]),
    ]
}

fn flowchart_or_overlay_paths(width_twips: i32, height_twips: i32) -> Vec<Vec<StaticShapePoint>> {
    vec![
        vec![
            StaticShapePoint {
                x_twips: 0,
                y_twips: height_twips / 2,
            },
            StaticShapePoint {
                x_twips: width_twips,
                y_twips: height_twips / 2,
            },
        ],
        vec![
            StaticShapePoint {
                x_twips: width_twips / 2,
                y_twips: 0,
            },
            StaticShapePoint {
                x_twips: width_twips / 2,
                y_twips: height_twips,
            },
        ],
    ]
}

fn flowchart_sort_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    [
        (width_twips / 2, 0),
        (width_twips, height_twips / 2),
        (width_twips / 2, height_twips),
        (0, height_twips / 2),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn flowchart_sort_overlay_paths(width_twips: i32, height_twips: i32) -> Vec<Vec<StaticShapePoint>> {
    vec![vec![
        StaticShapePoint {
            x_twips: 0,
            y_twips: height_twips / 2,
        },
        StaticShapePoint {
            x_twips: width_twips,
            y_twips: height_twips / 2,
        },
    ]]
}

fn flowchart_delay_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (0, 0),
            (500, 0),
            (562, 4),
            (620, 15),
            (680, 34),
            (735, 60),
            (788, 100),
            (835, 155),
            (878, 220),
            (920, 300),
            (958, 380),
            (980, 420),
            (1000, 500),
            (980, 580),
            (958, 620),
            (920, 700),
            (878, 780),
            (835, 845),
            (788, 900),
            (735, 940),
            (680, 966),
            (620, 985),
            (562, 996),
            (500, 1000),
            (0, 1000),
        ],
    )
}

fn flowchart_sequential_access_storage_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    regular_polygon_shape_points(width_twips, height_twips, 32)
}

fn flowchart_direct_access_storage_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (180, 0),
            (820, 0),
            (1000, 500),
            (820, 1000),
            (180, 1000),
            (0, 500),
        ],
    )
}

fn flowchart_display_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (0, 0),
            (720, 0),
            (780, 8),
            (830, 30),
            (852, 48),
            (872, 68),
            (910, 120),
            (940, 190),
            (965, 275),
            (978, 332),
            (988, 388),
            (996, 444),
            (1000, 500),
            (996, 556),
            (988, 612),
            (978, 668),
            (965, 725),
            (940, 810),
            (910, 880),
            (872, 932),
            (852, 952),
            (830, 970),
            (780, 992),
            (720, 1000),
            (0, 1000),
            (30, 890),
            (60, 780),
            (78, 740),
            (92, 700),
            (100, 660),
            (105, 620),
            (114, 560),
            (120, 500),
            (114, 440),
            (105, 380),
            (100, 340),
            (92, 300),
            (78, 260),
            (60, 220),
            (30, 110),
        ],
    )
}

fn up_ribbon_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (0, 280),
            (180, 0),
            (820, 0),
            (1000, 280),
            (860, 280),
            (860, 1000),
            (500, 820),
            (140, 1000),
            (140, 280),
        ],
    )
}

fn down_ribbon_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (140, 0),
            (500, 180),
            (860, 0),
            (860, 720),
            (1000, 720),
            (820, 1000),
            (180, 1000),
            (0, 720),
            (140, 720),
        ],
    )
}

fn curved_up_ribbon_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (0, 340),
            (70, 230),
            (160, 120),
            (212, 82),
            (270, 55),
            (325, 35),
            (380, 20),
            (440, 10),
            (500, 5),
            (560, 10),
            (620, 20),
            (675, 35),
            (730, 55),
            (788, 82),
            (840, 120),
            (930, 230),
            (1000, 340),
            (840, 300),
            (840, 940),
            (730, 880),
            (620, 820),
            (560, 790),
            (500, 780),
            (440, 790),
            (380, 820),
            (270, 880),
            (160, 940),
            (160, 300),
        ],
    )
}

fn curved_down_ribbon_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (160, 60),
            (270, 120),
            (380, 180),
            (410, 198),
            (440, 210),
            (470, 217),
            (500, 220),
            (530, 217),
            (560, 210),
            (590, 198),
            (620, 180),
            (730, 120),
            (840, 60),
            (840, 700),
            (1000, 660),
            (930, 770),
            (840, 880),
            (730, 945),
            (675, 965),
            (620, 980),
            (560, 990),
            (500, 995),
            (440, 990),
            (380, 980),
            (325, 965),
            (270, 945),
            (160, 880),
            (70, 770),
            (0, 660),
            (160, 700),
        ],
    )
}

fn vertical_scroll_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (280, 0),
            (1000, 0),
            (930, 40),
            (908, 66),
            (890, 95),
            (872, 126),
            (860, 160),
            (872, 194),
            (890, 225),
            (908, 254),
            (930, 280),
            (1000, 320),
            (760, 320),
            (760, 1000),
            (0, 1000),
            (70, 960),
            (92, 934),
            (110, 905),
            (128, 874),
            (140, 840),
            (128, 806),
            (110, 775),
            (92, 746),
            (70, 720),
            (0, 680),
            (240, 680),
            (240, 0),
        ],
    )
}

fn horizontal_scroll_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (0, 240),
            (680, 240),
            (680, 0),
            (760, 70),
            (780, 88),
            (800, 105),
            (820, 122),
            (840, 140),
            (860, 122),
            (880, 105),
            (900, 88),
            (920, 70),
            (1000, 0),
            (1000, 760),
            (320, 760),
            (320, 1000),
            (240, 930),
            (220, 912),
            (200, 895),
            (180, 878),
            (160, 860),
            (140, 878),
            (120, 895),
            (100, 912),
            (80, 930),
            (0, 1000),
        ],
    )
}

fn wave_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (0, 360),
            (70, 300),
            (160, 220),
            (182, 212),
            (205, 205),
            (228, 201),
            (250, 200),
            (272, 201),
            (295, 207),
            (340, 220),
            (385, 248),
            (430, 285),
            (475, 322),
            (520, 360),
            (565, 395),
            (610, 430),
            (700, 500),
            (850, 500),
            (1000, 500),
            (1000, 760),
            (910, 830),
            (820, 900),
            (798, 908),
            (775, 915),
            (752, 919),
            (730, 920),
            (708, 919),
            (685, 913),
            (640, 900),
            (595, 872),
            (550, 835),
            (505, 798),
            (460, 760),
            (415, 725),
            (370, 690),
            (280, 620),
            (140, 620),
            (0, 620),
        ],
    )
}

fn double_wave_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (0, 360),
            (60, 300),
            (120, 240),
            (136, 232),
            (152, 226),
            (168, 222),
            (185, 220),
            (202, 222),
            (218, 226),
            (234, 232),
            (250, 240),
            (315, 300),
            (380, 360),
            (440, 430),
            (500, 480),
            (560, 430),
            (620, 360),
            (685, 300),
            (750, 240),
            (766, 232),
            (782, 226),
            (798, 222),
            (815, 220),
            (832, 222),
            (848, 226),
            (864, 232),
            (880, 240),
            (940, 300),
            (1000, 360),
            (1000, 640),
            (940, 700),
            (880, 760),
            (864, 768),
            (848, 774),
            (832, 778),
            (815, 780),
            (798, 778),
            (782, 774),
            (766, 768),
            (750, 760),
            (685, 700),
            (620, 640),
            (560, 570),
            (500, 520),
            (440, 570),
            (380, 640),
            (315, 700),
            (250, 760),
            (234, 768),
            (218, 774),
            (202, 778),
            (185, 780),
            (168, 778),
            (152, 774),
            (136, 768),
            (120, 760),
            (60, 700),
            (0, 640),
        ],
    )
}

fn up_ribbon_overlay_paths(width_twips: i32, height_twips: i32) -> Vec<Vec<StaticShapePoint>> {
    vec![
        scaled_shape_points(width_twips, height_twips, &[(140, 280), (500, 820)]),
        scaled_shape_points(width_twips, height_twips, &[(860, 280), (500, 820)]),
    ]
}

fn down_ribbon_overlay_paths(width_twips: i32, height_twips: i32) -> Vec<Vec<StaticShapePoint>> {
    vec![
        scaled_shape_points(width_twips, height_twips, &[(140, 720), (500, 180)]),
        scaled_shape_points(width_twips, height_twips, &[(860, 720), (500, 180)]),
    ]
}

fn curved_up_ribbon_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![
        scaled_shape_points(width_twips, height_twips, &[(160, 300), (500, 780)]),
        scaled_shape_points(width_twips, height_twips, &[(840, 300), (500, 780)]),
    ]
}

fn curved_down_ribbon_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![
        scaled_shape_points(width_twips, height_twips, &[(160, 700), (500, 220)]),
        scaled_shape_points(width_twips, height_twips, &[(840, 700), (500, 220)]),
    ]
}

fn vertical_scroll_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![
        scaled_shape_points(
            width_twips,
            height_twips,
            &[
                (280, 0),
                (318, 18),
                (355, 55),
                (378, 108),
                (390, 160),
                (378, 212),
                (355, 265),
                (318, 302),
                (280, 320),
            ],
        ),
        scaled_shape_points(
            width_twips,
            height_twips,
            &[
                (720, 680),
                (682, 698),
                (645, 735),
                (622, 788),
                (610, 840),
                (622, 892),
                (645, 945),
                (682, 982),
                (720, 1000),
            ],
        ),
    ]
}

fn horizontal_scroll_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![
        scaled_shape_points(
            width_twips,
            height_twips,
            &[
                (680, 280),
                (698, 318),
                (735, 355),
                (788, 378),
                (840, 390),
                (892, 378),
                (945, 355),
                (982, 318),
                (1000, 280),
            ],
        ),
        scaled_shape_points(
            width_twips,
            height_twips,
            &[
                (0, 720),
                (18, 682),
                (55, 645),
                (108, 622),
                (160, 610),
                (212, 622),
                (265, 645),
                (302, 682),
                (320, 720),
            ],
        ),
    ]
}

fn rectangular_callout_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (0, 0),
            (1000, 0),
            (1000, 720),
            (620, 720),
            (430, 1000),
            (460, 720),
            (0, 720),
        ],
    )
}

fn rounded_rectangular_callout_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (120, 0),
            (880, 0),
            (918, 8),
            (952, 28),
            (972, 48),
            (992, 82),
            (1000, 120),
            (1000, 600),
            (992, 638),
            (972, 672),
            (952, 692),
            (918, 712),
            (880, 720),
            (620, 720),
            (430, 1000),
            (460, 720),
            (120, 720),
            (82, 712),
            (48, 692),
            (28, 672),
            (8, 638),
            (0, 600),
            (0, 120),
            (8, 82),
            (28, 48),
            (48, 28),
            (82, 8),
        ],
    )
}

fn oval_callout_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (500, 0),
            (555, 4),
            (610, 15),
            (638, 24),
            (665, 34),
            (720, 60),
            (746, 75),
            (772, 90),
            (820, 125),
            (842, 145),
            (862, 166),
            (900, 210),
            (938, 260),
            (970, 315),
            (990, 368),
            (1000, 420),
            (998, 468),
            (990, 515),
            (978, 558),
            (960, 600),
            (935, 637),
            (900, 670),
            (862, 696),
            (820, 720),
            (796, 728),
            (772, 737),
            (746, 744),
            (720, 750),
            (695, 754),
            (670, 756),
            (620, 760),
            (430, 1000),
            (460, 740),
            (410, 728),
            (385, 722),
            (360, 715),
            (332, 708),
            (305, 700),
            (278, 691),
            (250, 680),
            (205, 648),
            (165, 610),
            (120, 568),
            (80, 520),
            (50, 472),
            (25, 425),
            (10, 378),
            (0, 330),
            (5, 288),
            (20, 245),
            (52, 195),
            (90, 150),
            (130, 114),
            (175, 85),
            (228, 60),
            (280, 40),
            (308, 30),
            (335, 22),
            (390, 10),
            (445, 2),
        ],
    )
}

fn cloud_callout_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (140, 440),
            (80, 410),
            (35, 360),
            (32, 340),
            (40, 320),
            (45, 255),
            (58, 225),
            (80, 200),
            (120, 170),
            (158, 154),
            (195, 145),
            (280, 150),
            (305, 90),
            (320, 64),
            (340, 40),
            (382, 20),
            (425, 5),
            (520, 0),
            (558, 12),
            (595, 30),
            (628, 60),
            (660, 90),
            (735, 70),
            (778, 68),
            (820, 80),
            (862, 105),
            (900, 130),
            (928, 166),
            (950, 210),
            (960, 290),
            (945, 326),
            (920, 360),
            (945, 390),
            (970, 420),
            (1000, 500),
            (990, 545),
            (970, 585),
            (900, 640),
            (880, 648),
            (858, 655),
            (837, 661),
            (815, 665),
            (720, 660),
            (665, 700),
            (653, 715),
            (642, 730),
            (631, 745),
            (620, 760),
            (540, 760),
            (470, 720),
            (350, 1000),
            (380, 720),
            (295, 720),
            (270, 717),
            (246, 712),
            (223, 707),
            (200, 700),
            (158, 682),
            (120, 660),
            (96, 632),
            (80, 600),
            (95, 515),
        ],
    )
}

fn line_callout_border_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (0, 0),
            (1000, 0),
            (1000, 620),
            (740, 620),
            (520, 1000),
            (560, 620),
            (0, 620),
        ],
    )
}

fn line_callout_border_accent_bar_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![scaled_shape_points(
        width_twips,
        height_twips,
        &[(160, 0), (160, 620)],
    )]
}

fn line_callout_horizontal_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    scaled_shape_points(width_twips, height_twips, &[(0, 500), (1000, 500)])
}

fn line_callout_diagonal_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    scaled_shape_points(width_twips, height_twips, &[(0, 1000), (1000, 0)])
}

fn line_callout_angled_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[(0, 1000), (450, 500), (1000, 500)],
    )
}

fn line_callout_ushape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[(0, 1000), (0, 500), (1000, 500), (1000, 0)],
    )
}

fn action_button_blank_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    bevel_shape_points(width_twips, height_twips)
}

fn action_button_frame_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    bevel_shape_overlay_paths(width_twips, height_twips)
}

fn action_button_document_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    let mut paths = action_button_frame_overlay_paths(width_twips, height_twips);
    paths.extend(folded_corner_shape_overlay_paths(width_twips, height_twips));
    paths
}

fn action_button_home_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (80, 500),
            (500, 120),
            (920, 500),
            (780, 500),
            (780, 880),
            (580, 880),
            (580, 650),
            (420, 650),
            (420, 880),
            (220, 880),
            (220, 500),
        ],
    )
}

fn action_button_help_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (500, 120),
            (556, 124),
            (610, 132),
            (658, 144),
            (700, 160),
            (775, 225),
            (804, 266),
            (820, 310),
            (822, 352),
            (815, 395),
            (780, 480),
            (742, 516),
            (705, 548),
            (610, 610),
            (610, 710),
            (430, 710),
            (430, 560),
            (510, 505),
            (556, 472),
            (600, 440),
            (620, 385),
            (620, 330),
            (565, 295),
            (500, 280),
            (435, 295),
            (380, 330),
            (338, 395),
            (300, 460),
            (180, 350),
            (228, 270),
            (300, 190),
            (390, 140),
        ],
    )
}

fn action_button_help_shape_point_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![scaled_shape_points(
        width_twips,
        height_twips,
        &[(430, 790), (570, 790), (570, 920), (430, 920)],
    )]
}

fn action_button_information_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (420, 270),
            (580, 270),
            (580, 820),
            (700, 820),
            (700, 900),
            (300, 900),
            (300, 820),
            (420, 820),
        ],
    )
}

fn action_button_information_shape_point_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![scaled_shape_points(
        width_twips,
        height_twips,
        &[(420, 100), (580, 100), (580, 220), (420, 220)],
    )]
}

fn action_button_back_previous_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    left_arrow_shape_points(width_twips, height_twips)
}

fn action_button_back_previous_shape_point_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![scaled_shape_points(
        width_twips,
        height_twips,
        &[(880, 180), (420, 500), (880, 820)],
    )]
}

fn action_button_forward_next_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    right_arrow_shape_points(width_twips, height_twips)
}

fn action_button_forward_next_shape_point_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![scaled_shape_points(
        width_twips,
        height_twips,
        &[(120, 180), (580, 500), (120, 820)],
    )]
}

fn action_button_beginning_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (80, 180),
            (220, 180),
            (220, 820),
            (80, 820),
            (80, 180),
            (920, 180),
            (420, 500),
            (920, 820),
        ],
    )
}

fn action_button_beginning_shape_point_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![scaled_shape_points(
        width_twips,
        height_twips,
        &[(920, 180), (420, 500), (920, 820)],
    )]
}

fn action_button_end_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (920, 180),
            (780, 180),
            (780, 820),
            (920, 820),
            (920, 180),
            (80, 180),
            (580, 500),
            (80, 820),
        ],
    )
}

fn action_button_end_shape_point_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![scaled_shape_points(
        width_twips,
        height_twips,
        &[(80, 180), (580, 500), (80, 820)],
    )]
}

fn action_button_return_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (850, 180),
            (850, 620),
            (360, 620),
            (360, 820),
            (80, 500),
            (360, 180),
            (360, 380),
            (650, 380),
            (650, 180),
        ],
    )
}

fn action_button_return_shape_point_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![scaled_shape_points(
        width_twips,
        height_twips,
        &[(850, 180), (850, 620), (360, 620)],
    )]
}

fn action_button_document_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    folded_corner_shape_points(width_twips, height_twips)
}

fn action_button_sound_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (100, 420),
            (300, 420),
            (560, 220),
            (560, 780),
            (300, 580),
            (100, 580),
        ],
    )
}

fn action_button_sound_shape_point_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![
        scaled_shape_points(
            width_twips,
            height_twips,
            &[
                (650, 335),
                (710, 375),
                (750, 430),
                (765, 500),
                (750, 570),
                (710, 625),
                (650, 665),
                (620, 620),
                (665, 580),
                (695, 535),
                (705, 500),
                (695, 465),
                (665, 420),
                (620, 380),
            ],
        ),
        scaled_shape_points(
            width_twips,
            height_twips,
            &[
                (760, 230),
                (835, 285),
                (890, 365),
                (920, 455),
                (925, 500),
                (920, 545),
                (890, 635),
                (835, 715),
                (760, 770),
                (725, 720),
                (785, 665),
                (830, 595),
                (850, 525),
                (850, 475),
                (830, 405),
                (785, 335),
                (725, 280),
            ],
        ),
    ]
}

fn action_button_movie_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (100, 200),
            (900, 200),
            (900, 800),
            (100, 800),
            (100, 200),
            (240, 200),
            (240, 800),
            (760, 800),
            (760, 200),
        ],
    )
}

fn action_button_movie_shape_point_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![
        scaled_shape_points(
            width_twips,
            height_twips,
            &[(140, 300), (200, 300), (200, 380), (140, 380)],
        ),
        scaled_shape_points(
            width_twips,
            height_twips,
            &[(140, 500), (200, 500), (200, 580), (140, 580)],
        ),
        scaled_shape_points(
            width_twips,
            height_twips,
            &[(800, 300), (860, 300), (860, 380), (800, 380)],
        ),
        scaled_shape_points(
            width_twips,
            height_twips,
            &[(800, 500), (860, 500), (860, 580), (800, 580)],
        ),
    ]
}

fn balloon_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (500, 0),
            (558, 5),
            (615, 20),
            (670, 38),
            (720, 60),
            (770, 92),
            (815, 130),
            (860, 172),
            (900, 220),
            (938, 272),
            (965, 330),
            (988, 390),
            (1000, 450),
            (995, 500),
            (980, 550),
            (965, 598),
            (940, 640),
            (900, 682),
            (860, 720),
            (810, 752),
            (760, 780),
            (710, 800),
            (660, 815),
            (610, 820),
            (560, 820),
            (545, 838),
            (528, 858),
            (490, 900),
            (455, 950),
            (420, 1000),
            (425, 900),
            (430, 800),
            (404, 794),
            (378, 787),
            (352, 779),
            (325, 770),
            (272, 748),
            (220, 720),
            (172, 688),
            (130, 650),
            (92, 608),
            (60, 560),
            (38, 508),
            (20, 455),
            (5, 402),
            (0, 350),
            (12, 298),
            (35, 245),
            (62, 195),
            (100, 150),
            (142, 112),
            (185, 80),
            (232, 58),
            (280, 40),
            (335, 22),
            (390, 10),
            (445, 2),
        ],
    )
}

fn flowchart_offline_storage_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    [
        (width_twips / 4, 0),
        (width_twips, 0),
        (width_twips, height_twips),
        (width_twips / 4, height_twips),
        (0, height_twips / 2),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn left_right_ribbon_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (0, 260),
            (45, 195),
            (90, 130),
            (135, 65),
            (180, 0),
            (240, 40),
            (300, 80),
            (360, 120),
            (420, 160),
            (580, 160),
            (640, 120),
            (700, 80),
            (760, 40),
            (820, 0),
            (865, 65),
            (910, 130),
            (955, 195),
            (1000, 260),
            (840, 260),
            (840, 900),
            (775, 865),
            (710, 830),
            (645, 795),
            (580, 760),
            (420, 760),
            (355, 795),
            (290, 830),
            (225, 865),
            (160, 900),
            (160, 260),
        ],
    )
}

fn left_right_ribbon_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![
        scaled_shape_points(width_twips, height_twips, &[(160, 260), (420, 760)]),
        scaled_shape_points(width_twips, height_twips, &[(840, 260), (580, 760)]),
    ]
}

fn diagonal_stripe_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[(260, 0), (1000, 0), (740, 1000), (0, 1000)],
    )
}

fn diagonal_stripe_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![rectangle_polygon_shape_points(width_twips, height_twips)]
}

fn pie_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (500, 500),
            (500, 0),
            (550, 2),
            (600, 10),
            (645, 22),
            (690, 40),
            (735, 60),
            (775, 85),
            (815, 115),
            (850, 150),
            (885, 185),
            (915, 225),
            (940, 265),
            (960, 310),
            (982, 360),
            (990, 405),
            (1000, 500),
            (998, 548),
            (990, 595),
            (982, 640),
            (960, 690),
            (940, 735),
            (915, 775),
            (885, 815),
            (850, 850),
            (815, 885),
            (775, 915),
            (735, 940),
            (690, 960),
            (645, 978),
            (600, 990),
            (550, 998),
            (500, 1000),
        ],
    )
}

fn non_isosceles_trapezoid_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[(260, 0), (1000, 0), (800, 1000), (0, 1000)],
    )
}

fn round_two_same_rectangle_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (160, 0),
            (840, 0),
            (890, 10),
            (930, 30),
            (958, 58),
            (970, 70),
            (992, 120),
            (1000, 160),
            (1000, 1000),
            (0, 1000),
            (0, 160),
            (8, 120),
            (20, 90),
            (42, 58),
            (70, 30),
            (110, 10),
        ],
    )
}

fn round_one_rectangle_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (160, 0),
            (1000, 0),
            (1000, 1000),
            (0, 1000),
            (0, 160),
            (8, 120),
            (20, 90),
            (42, 58),
            (70, 30),
            (110, 10),
        ],
    )
}

fn round_two_diagonal_rectangle_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (160, 0),
            (1000, 0),
            (1000, 840),
            (992, 890),
            (970, 930),
            (942, 958),
            (930, 970),
            (880, 992),
            (840, 1000),
            (0, 1000),
            (0, 160),
            (8, 120),
            (20, 90),
            (42, 58),
            (70, 30),
            (110, 10),
        ],
    )
}

fn snip_round_rectangle_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (160, 0),
            (840, 0),
            (890, 10),
            (930, 30),
            (958, 58),
            (970, 70),
            (992, 120),
            (1000, 160),
            (1000, 760),
            (760, 1000),
            (0, 1000),
            (0, 160),
            (8, 120),
            (20, 90),
            (42, 58),
            (70, 30),
            (110, 10),
        ],
    )
}

fn snip_one_rectangle_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[(0, 0), (1000, 0), (1000, 760), (760, 1000), (0, 1000)],
    )
}

fn snip_two_same_rectangle_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (240, 0),
            (760, 0),
            (1000, 240),
            (1000, 1000),
            (0, 1000),
            (0, 240),
        ],
    )
}

fn snip_two_diagonal_rectangle_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (0, 0),
            (760, 0),
            (1000, 240),
            (1000, 1000),
            (240, 1000),
            (0, 760),
        ],
    )
}

fn frame_shape_point_paths(width_twips: i32, height_twips: i32) -> Vec<Vec<StaticShapePoint>> {
    vec![scaled_shape_points(
        width_twips,
        height_twips,
        &[(220, 220), (220, 780), (780, 780), (780, 220)],
    )]
}

fn half_frame_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (0, 0),
            (1000, 0),
            (1000, 220),
            (220, 220),
            (220, 1000),
            (0, 1000),
        ],
    )
}

fn tear_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (500, 0),
            (542, 26),
            (585, 55),
            (628, 86),
            (670, 120),
            (712, 152),
            (750, 185),
            (786, 222),
            (820, 260),
            (854, 300),
            (885, 340),
            (910, 384),
            (930, 430),
            (952, 482),
            (975, 535),
            (988, 588),
            (1000, 640),
            (990, 688),
            (980, 735),
            (958, 778),
            (930, 820),
            (894, 856),
            (855, 890),
            (810, 926),
            (760, 960),
            (700, 978),
            (640, 995),
            (580, 1000),
            (520, 1000),
            (465, 986),
            (410, 970),
            (354, 946),
            (300, 920),
            (256, 886),
            (215, 850),
            (182, 806),
            (150, 760),
            (120, 710),
            (95, 660),
            (76, 610),
            (60, 560),
            (52, 510),
            (45, 460),
            (42, 410),
            (40, 360),
            (52, 312),
            (70, 265),
            (94, 222),
            (120, 180),
            (152, 144),
            (190, 110),
            (232, 84),
            (280, 60),
            (334, 38),
            (390, 20),
        ],
    )
}

fn chord_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (160, 180),
            (840, 180),
            (858, 188),
            (875, 202),
            (890, 215),
            (905, 230),
            (935, 262),
            (960, 300),
            (972, 324),
            (980, 350),
            (990, 400),
            (998, 450),
            (1000, 500),
            (996, 555),
            (985, 605),
            (965, 655),
            (940, 700),
            (908, 750),
            (870, 795),
            (830, 838),
            (780, 880),
            (715, 925),
            (650, 955),
            (575, 985),
            (500, 1000),
            (425, 985),
            (350, 955),
            (220, 880),
            (170, 838),
            (130, 795),
            (92, 750),
            (60, 700),
            (35, 655),
            (15, 605),
            (4, 555),
            (0, 500),
            (2, 450),
            (10, 400),
            (20, 350),
            (28, 324),
            (40, 300),
            (65, 262),
            (95, 230),
            (110, 215),
            (125, 202),
        ],
    )
}

fn corner_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (0, 0),
            (1000, 0),
            (1000, 220),
            (220, 220),
            (220, 1000),
            (0, 1000),
        ],
    )
}

fn math_plus_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    cross_shape_points(width_twips, height_twips)
}

fn math_minus_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[(120, 410), (880, 410), (880, 590), (120, 590)],
    )
}

fn math_multiply_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (160, 0),
            (500, 340),
            (840, 0),
            (1000, 160),
            (660, 500),
            (1000, 840),
            (840, 1000),
            (500, 660),
            (160, 1000),
            (0, 840),
            (340, 500),
            (0, 160),
        ],
    )
}

fn math_divide_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[(120, 410), (880, 410), (880, 590), (120, 590)],
    )
}

fn math_equal_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (120, 270),
            (880, 270),
            (880, 430),
            (120, 430),
            (120, 570),
            (880, 570),
            (880, 730),
            (120, 730),
        ],
    )
}

fn math_not_equal_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (120, 270),
            (560, 270),
            (670, 80),
            (800, 80),
            (690, 270),
            (880, 270),
            (880, 430),
            (600, 430),
            (520, 570),
            (880, 570),
            (880, 730),
            (430, 730),
            (320, 920),
            (190, 920),
            (300, 730),
            (120, 730),
            (120, 570),
            (390, 570),
            (470, 430),
            (120, 430),
        ],
    )
}

fn math_divide_shape_point_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![
        scaled_shape_points(
            width_twips,
            height_twips,
            &[
                (500, 120),
                (527, 125),
                (550, 140),
                (565, 163),
                (570, 190),
                (565, 217),
                (550, 240),
                (527, 255),
                (500, 260),
                (473, 255),
                (450, 240),
                (435, 217),
                (430, 190),
                (435, 163),
                (450, 140),
                (473, 125),
            ],
        ),
        scaled_shape_points(
            width_twips,
            height_twips,
            &[
                (500, 740),
                (527, 745),
                (550, 760),
                (565, 783),
                (570, 810),
                (565, 837),
                (550, 860),
                (527, 875),
                (500, 880),
                (473, 875),
                (450, 860),
                (435, 837),
                (430, 810),
                (435, 783),
                (450, 760),
                (473, 745),
            ],
        ),
    ]
}

fn math_equal_shape_point_paths(width_twips: i32, height_twips: i32) -> Vec<Vec<StaticShapePoint>> {
    vec![
        scaled_shape_points(
            width_twips,
            height_twips,
            &[(120, 270), (880, 270), (880, 430), (120, 430)],
        ),
        scaled_shape_points(
            width_twips,
            height_twips,
            &[(120, 570), (880, 570), (880, 730), (120, 730)],
        ),
    ]
}

fn math_not_equal_shape_point_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    let mut paths = math_equal_shape_point_paths(width_twips, height_twips);
    paths.push(scaled_shape_points(
        width_twips,
        height_twips,
        &[(670, 80), (800, 80), (320, 920), (190, 920)],
    ));
    paths
}

fn gear_shape_point_paths(
    width_twips: i32,
    height_twips: i32,
    points: usize,
) -> Vec<Vec<StaticShapePoint>> {
    vec![regular_polygon_inset_shape_points(
        width_twips,
        height_twips,
        points.max(3) * 4,
        310,
    )]
}

fn gear_shape_points(
    width_twips: i32,
    height_twips: i32,
    teeth: usize,
    root_radius_per_mille: i32,
) -> Vec<StaticShapePoint> {
    let points = teeth.max(3) * 4;
    let center_x = f64::from(width_twips) / 2.0;
    let center_y = f64::from(height_twips) / 2.0;
    let radius_x = center_x;
    let radius_y = center_y;
    let root_radius = f64::from(root_radius_per_mille) / 1000.0;
    let flank_radius = ((1.0 + root_radius) / 2.0).min(0.92);
    (0..points)
        .map(|index| {
            let angle = -std::f64::consts::FRAC_PI_2
                + 2.0 * std::f64::consts::PI * (index as f64) / (points as f64);
            let radius = match index % 4 {
                0 => 1.0,
                1 | 3 => flank_radius,
                _ => root_radius,
            };
            StaticShapePoint {
                x_twips: ((center_x + angle.cos() * radius_x * radius).round() as i32)
                    .clamp(0, width_twips),
                y_twips: ((center_y + angle.sin() * radius_y * radius).round() as i32)
                    .clamp(0, height_twips),
            }
        })
        .collect()
}

fn corner_tabs_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (0, 0),
            (380, 0),
            (380, 160),
            (160, 160),
            (160, 380),
            (0, 380),
        ],
    )
}

fn corner_tabs_shape_point_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (620, 620),
            (840, 620),
            (840, 840),
            (1000, 840),
            (1000, 1000),
            (620, 1000),
        ],
    )]
}

fn square_tabs_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (0, 0),
            (320, 0),
            (320, 180),
            (680, 180),
            (680, 0),
            (1000, 0),
        ],
    )
}

fn square_tabs_shape_point_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![
        scaled_shape_points(
            width_twips,
            height_twips,
            &[
                (1000, 0),
                (1000, 320),
                (820, 320),
                (820, 680),
                (1000, 680),
                (1000, 1000),
            ],
        ),
        scaled_shape_points(
            width_twips,
            height_twips,
            &[
                (1000, 1000),
                (680, 1000),
                (680, 820),
                (320, 820),
                (320, 1000),
                (0, 1000),
            ],
        ),
        scaled_shape_points(
            width_twips,
            height_twips,
            &[
                (0, 1000),
                (0, 680),
                (180, 680),
                (180, 320),
                (0, 320),
                (0, 0),
            ],
        ),
    ]
}

fn plaque_tabs_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (170, 0),
            (380, 0),
            (380, 160),
            (620, 160),
            (620, 0),
            (830, 0),
            (830, 170),
            (1000, 170),
        ],
    )
}

fn plaque_tabs_shape_point_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![
        scaled_shape_points(
            width_twips,
            height_twips,
            &[
                (1000, 170),
                (1000, 380),
                (840, 380),
                (840, 620),
                (1000, 620),
                (1000, 830),
            ],
        ),
        scaled_shape_points(
            width_twips,
            height_twips,
            &[
                (1000, 830),
                (830, 830),
                (830, 1000),
                (620, 1000),
                (620, 840),
                (380, 840),
                (380, 1000),
                (170, 1000),
                (170, 830),
                (0, 830),
            ],
        ),
        scaled_shape_points(
            width_twips,
            height_twips,
            &[
                (0, 830),
                (0, 620),
                (160, 620),
                (160, 380),
                (0, 380),
                (0, 170),
                (170, 170),
                (170, 0),
            ],
        ),
    ]
}

fn funnel_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (0, 120),
            (1000, 120),
            (660, 520),
            (660, 1000),
            (340, 1000),
            (340, 520),
        ],
    )
}

fn pie_wedge_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (500, 500),
            (500, 0),
            (525, 0),
            (550, 2),
            (575, 5),
            (600, 10),
            (623, 15),
            (645, 22),
            (668, 30),
            (690, 40),
            (712, 50),
            (735, 60),
            (755, 72),
            (775, 85),
            (795, 100),
            (815, 115),
            (832, 132),
            (850, 150),
            (868, 168),
            (885, 185),
            (900, 205),
            (915, 225),
            (928, 245),
            (940, 265),
            (950, 288),
            (960, 310),
            (972, 335),
            (982, 360),
            (986, 382),
            (990, 405),
            (994, 428),
            (998, 452),
            (999, 476),
            (1000, 500),
        ],
    )
}

fn left_circular_arrow_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (270, 180),
            (318, 140),
            (365, 105),
            (422, 78),
            (480, 60),
            (545, 52),
            (610, 50),
            (675, 62),
            (740, 80),
            (794, 112),
            (845, 150),
            (885, 195),
            (920, 250),
            (940, 315),
            (955, 380),
            (960, 450),
            (960, 520),
            (952, 555),
            (940, 590),
            (928, 622),
            (910, 655),
            (868, 708),
            (820, 760),
            (764, 802),
            (705, 840),
            (638, 865),
            (570, 880),
            (504, 876),
            (440, 865),
            (378, 846),
            (320, 820),
            (276, 784),
            (235, 745),
            (200, 704),
            (170, 660),
            (320, 660),
            (375, 694),
            (430, 720),
            (475, 730),
            (520, 735),
            (565, 730),
            (610, 720),
            (654, 700),
            (695, 675),
            (730, 644),
            (760, 610),
            (780, 568),
            (800, 525),
            (808, 478),
            (810, 430),
            (800, 386),
            (790, 345),
            (760, 306),
            (730, 270),
            (690, 246),
            (650, 225),
            (605, 215),
            (560, 210),
            (518, 214),
            (475, 220),
            (438, 235),
            (400, 250),
            (362, 274),
            (325, 300),
            (298, 330),
            (270, 360),
            (270, 500),
            (40, 300),
        ],
    )
}

fn left_circular_arrow_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (325, 300),
            (362, 274),
            (400, 250),
            (438, 235),
            (475, 220),
            (518, 214),
            (560, 210),
            (605, 215),
            (650, 225),
            (690, 246),
            (730, 270),
            (746, 288),
            (760, 306),
            (790, 345),
            (796, 366),
            (800, 386),
            (810, 430),
            (808, 478),
            (804, 502),
            (800, 525),
            (780, 568),
            (760, 610),
            (746, 628),
            (730, 644),
            (695, 675),
            (654, 700),
            (632, 710),
            (610, 720),
            (565, 730),
            (520, 735),
            (475, 730),
            (430, 720),
            (375, 694),
            (320, 660),
        ],
    )]
}

fn circular_arrow_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    mirror_shape_points_x(
        width_twips,
        left_circular_arrow_shape_points(width_twips, height_twips),
    )
}

fn circular_arrow_overlay_paths(width_twips: i32, height_twips: i32) -> Vec<Vec<StaticShapePoint>> {
    left_circular_arrow_overlay_paths(width_twips, height_twips)
        .into_iter()
        .map(|path| mirror_shape_points_x(width_twips, path))
        .collect()
}

fn left_right_circular_arrow_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (250, 190),
            (302, 146),
            (355, 105),
            (412, 78),
            (470, 60),
            (530, 55),
            (590, 55),
            (646, 70),
            (700, 90),
            (746, 122),
            (790, 155),
            (828, 195),
            (860, 240),
            (960, 200),
            (900, 500),
            (650, 330),
            (770, 330),
            (710, 240),
            (676, 220),
            (640, 205),
            (600, 195),
            (560, 190),
            (518, 196),
            (475, 205),
            (432, 220),
            (390, 240),
            (358, 265),
            (330, 295),
            (310, 328),
            (290, 360),
            (268, 400),
            (245, 440),
            (232, 480),
            (220, 520),
            (232, 562),
            (245, 605),
            (266, 642),
            (290, 680),
            (322, 710),
            (355, 740),
            (398, 758),
            (440, 770),
            (485, 774),
            (530, 775),
            (570, 764),
            (610, 750),
            (644, 732),
            (675, 710),
            (698, 680),
            (720, 650),
            (590, 650),
            (840, 500),
            (930, 790),
            (820, 750),
            (780, 788),
            (740, 825),
            (692, 864),
            (640, 900),
            (580, 916),
            (520, 930),
            (390, 930),
            (358, 922),
            (330, 912),
            (302, 900),
            (275, 885),
            (222, 844),
            (170, 800),
            (132, 748),
            (95, 695),
            (84, 665),
            (72, 635),
            (60, 602),
            (50, 570),
            (48, 508),
            (45, 445),
            (68, 388),
            (90, 330),
        ],
    )
}

fn left_right_circular_arrow_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![
        scaled_shape_points(
            width_twips,
            height_twips,
            &[
                (650, 330),
                (770, 330),
                (710, 240),
                (676, 220),
                (640, 205),
                (600, 195),
                (560, 190),
                (518, 196),
                (475, 205),
                (432, 220),
                (390, 240),
                (358, 265),
                (330, 295),
                (320, 312),
                (310, 328),
                (290, 360),
                (268, 400),
                (256, 420),
                (245, 440),
                (232, 480),
                (220, 520),
            ],
        ),
        scaled_shape_points(
            width_twips,
            height_twips,
            &[
                (245, 605),
                (266, 642),
                (290, 680),
                (322, 710),
                (355, 740),
                (398, 758),
                (440, 770),
                (462, 772),
                (485, 774),
                (530, 775),
                (570, 764),
                (610, 750),
                (628, 742),
                (644, 732),
                (675, 710),
                (698, 680),
                (720, 650),
                (590, 650),
            ],
        ),
    ]
}

fn swoosh_arrow_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (40, 760),
            (64, 720),
            (90, 680),
            (124, 644),
            (160, 610),
            (198, 580),
            (235, 550),
            (256, 536),
            (278, 524),
            (320, 500),
            (344, 488),
            (368, 478),
            (415, 460),
            (442, 452),
            (468, 444),
            (520, 430),
            (545, 424),
            (570, 418),
            (620, 405),
            (645, 400),
            (670, 396),
            (720, 390),
            (720, 240),
            (1000, 500),
            (720, 760),
            (720, 600),
            (675, 600),
            (630, 605),
            (585, 612),
            (540, 620),
            (490, 634),
            (440, 650),
            (414, 658),
            (388, 668),
            (340, 690),
            (316, 704),
            (292, 722),
            (245, 755),
            (222, 775),
            (200, 795),
            (180, 818),
            (160, 840),
        ],
    )
}

fn cloud_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (130, 610),
            (98, 606),
            (70, 600),
            (45, 575),
            (25, 540),
            (12, 496),
            (0, 450),
            (8, 415),
            (25, 385),
            (52, 362),
            (80, 345),
            (118, 336),
            (155, 330),
            (205, 310),
            (230, 200),
            (254, 170),
            (285, 145),
            (306, 136),
            (328, 130),
            (370, 120),
            (414, 124),
            (455, 130),
            (520, 170),
            (560, 115),
            (580, 86),
            (600, 60),
            (626, 54),
            (652, 50),
            (705, 45),
            (750, 55),
            (790, 70),
            (824, 98),
            (855, 130),
            (866, 152),
            (878, 174),
            (900, 220),
            (936, 232),
            (970, 245),
            (1000, 290),
            (1000, 375),
            (992, 415),
            (980, 450),
            (968, 466),
            (956, 480),
            (930, 505),
            (904, 524),
            (880, 540),
            (870, 610),
            (862, 646),
            (850, 680),
            (816, 714),
            (780, 745),
            (735, 768),
            (690, 780),
            (650, 776),
            (610, 770),
            (540, 730),
            (516, 762),
            (490, 790),
            (456, 826),
            (420, 860),
            (370, 865),
            (320, 865),
            (274, 850),
            (230, 830),
            (190, 808),
            (160, 780),
            (138, 750),
            (120, 720),
            (124, 666),
        ],
    )
}

fn chart_x_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    rectangle_polygon_shape_points(width_twips, height_twips)
}

fn chart_star_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    rectangle_polygon_shape_points(width_twips, height_twips)
}

fn chart_plus_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    rectangle_polygon_shape_points(width_twips, height_twips)
}

fn chart_x_shape_point_paths(width_twips: i32, height_twips: i32) -> Vec<Vec<StaticShapePoint>> {
    vec![scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (220, 220),
            (370, 220),
            (500, 380),
            (630, 220),
            (780, 220),
            (590, 500),
            (780, 780),
            (630, 780),
            (500, 620),
            (370, 780),
            (220, 780),
            (410, 500),
        ],
    )]
}

fn chart_star_shape_point_paths(width_twips: i32, height_twips: i32) -> Vec<Vec<StaticShapePoint>> {
    vec![scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (500, 160),
            (580, 390),
            (820, 390),
            (630, 540),
            (710, 780),
            (500, 640),
            (290, 780),
            (370, 540),
            (180, 390),
            (420, 390),
        ],
    )]
}

fn chart_plus_shape_point_paths(width_twips: i32, height_twips: i32) -> Vec<Vec<StaticShapePoint>> {
    vec![scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (420, 220),
            (580, 220),
            (580, 420),
            (780, 420),
            (780, 580),
            (580, 580),
            (580, 780),
            (420, 780),
            (420, 580),
            (220, 580),
            (220, 420),
            (420, 420),
        ],
    )]
}

fn line_inverse_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    [(width_twips, 0), (0, height_twips)]
        .into_iter()
        .map(|(x, y)| StaticShapePoint {
            x_twips: x,
            y_twips: y,
        })
        .collect()
}

fn trapezoid_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let inset = width_twips / 4;
    [
        (inset, 0),
        (width_twips.saturating_sub(inset), 0),
        (width_twips, height_twips),
        (0, height_twips),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn manual_input_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let top_inset = height_twips / 4;
    [
        (0, top_inset),
        (width_twips, 0),
        (width_twips, height_twips),
        (0, height_twips),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn manual_operation_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let inset = width_twips / 4;
    [
        (0, 0),
        (width_twips, 0),
        (width_twips.saturating_sub(inset), height_twips),
        (inset, height_twips),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn parallelogram_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let inset = width_twips / 4;
    [
        (inset, 0),
        (width_twips, 0),
        (width_twips.saturating_sub(inset), height_twips),
        (0, height_twips),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn octagon_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let inset_x = width_twips / 4;
    let inset_y = height_twips / 4;
    [
        (inset_x, 0),
        (width_twips.saturating_sub(inset_x), 0),
        (width_twips, inset_y),
        (width_twips, height_twips.saturating_sub(inset_y)),
        (width_twips.saturating_sub(inset_x), height_twips),
        (inset_x, height_twips),
        (0, height_twips.saturating_sub(inset_y)),
        (0, inset_y),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn hexagon_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let inset = width_twips / 4;
    let mid_y = height_twips / 2;
    [
        (inset, 0),
        (width_twips.saturating_sub(inset), 0),
        (width_twips, mid_y),
        (width_twips.saturating_sub(inset), height_twips),
        (inset, height_twips),
        (0, mid_y),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn cross_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let third_x = width_twips / 3;
    let two_thirds_x = (width_twips * 2) / 3;
    let third_y = height_twips / 3;
    let two_thirds_y = (height_twips * 2) / 3;
    [
        (third_x, 0),
        (two_thirds_x, 0),
        (two_thirds_x, third_y),
        (width_twips, third_y),
        (width_twips, two_thirds_y),
        (two_thirds_x, two_thirds_y),
        (two_thirds_x, height_twips),
        (third_x, height_twips),
        (third_x, two_thirds_y),
        (0, two_thirds_y),
        (0, third_y),
        (third_x, third_y),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn pentagon_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    [
        (width_twips / 2, 0),
        (width_twips, height_twips / 3),
        ((width_twips * 4) / 5, height_twips),
        (width_twips / 5, height_twips),
        (0, height_twips / 3),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn can_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let inset_y = height_twips / 8;
    let quarter_x = width_twips / 4;
    let three_quarter_x = width_twips.saturating_sub(quarter_x);
    let half_inset_y = inset_y / 2;
    let bottom_y = height_twips.saturating_sub(inset_y);
    [
        (width_twips / 2, 0),
        ((width_twips * 5) / 8, half_inset_y / 2),
        (three_quarter_x, half_inset_y),
        ((width_twips * 7) / 8, (half_inset_y * 3) / 2),
        (width_twips, inset_y),
        (width_twips, bottom_y),
        (
            (width_twips * 7) / 8,
            height_twips.saturating_sub((half_inset_y * 3) / 2),
        ),
        (three_quarter_x, height_twips.saturating_sub(half_inset_y)),
        (
            (width_twips * 5) / 8,
            height_twips.saturating_sub(half_inset_y / 2),
        ),
        (width_twips / 2, height_twips),
        (
            (width_twips * 3) / 8,
            height_twips.saturating_sub(half_inset_y / 2),
        ),
        (quarter_x, height_twips.saturating_sub(half_inset_y)),
        (
            width_twips / 8,
            height_twips.saturating_sub((half_inset_y * 3) / 2),
        ),
        (0, bottom_y),
        (0, inset_y),
        (width_twips / 8, (half_inset_y * 3) / 2),
        (quarter_x, half_inset_y),
        ((width_twips * 3) / 8, half_inset_y / 2),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn can_shape_overlay_paths(width_twips: i32, height_twips: i32) -> Vec<Vec<StaticShapePoint>> {
    vec![scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (0, 125),
            (62, 109),
            (125, 94),
            (188, 78),
            (250, 62),
            (312, 47),
            (375, 31),
            (438, 16),
            (500, 0),
            (562, 16),
            (625, 31),
            (688, 47),
            (750, 62),
            (812, 78),
            (875, 94),
            (938, 109),
            (1000, 125),
        ],
    )]
}

fn cube_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let inset_x = width_twips / 4;
    let inset_y = height_twips / 4;
    [
        (inset_x, 0),
        (width_twips, 0),
        (width_twips, height_twips.saturating_sub(inset_y)),
        (width_twips.saturating_sub(inset_x), height_twips),
        (0, height_twips),
        (0, inset_y),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn cube_shape_overlay_paths(width_twips: i32, height_twips: i32) -> Vec<Vec<StaticShapePoint>> {
    let inset_x = width_twips / 4;
    let inset_y = height_twips / 4;
    let front_right_x = width_twips.saturating_sub(inset_x);
    vec![
        vec![
            StaticShapePoint {
                x_twips: 0,
                y_twips: inset_y,
            },
            StaticShapePoint {
                x_twips: front_right_x,
                y_twips: inset_y,
            },
        ],
        vec![
            StaticShapePoint {
                x_twips: front_right_x,
                y_twips: inset_y,
            },
            StaticShapePoint {
                x_twips: front_right_x,
                y_twips: height_twips,
            },
        ],
        vec![
            StaticShapePoint {
                x_twips: front_right_x,
                y_twips: inset_y,
            },
            StaticShapePoint {
                x_twips: width_twips,
                y_twips: 0,
            },
        ],
    ]
}

fn bevel_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let inset_x = width_twips / 6;
    let inset_y = height_twips / 6;
    [
        (inset_x, 0),
        (width_twips.saturating_sub(inset_x), 0),
        (width_twips, inset_y),
        (width_twips, height_twips.saturating_sub(inset_y)),
        (width_twips.saturating_sub(inset_x), height_twips),
        (inset_x, height_twips),
        (0, height_twips.saturating_sub(inset_y)),
        (0, inset_y),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn bevel_shape_overlay_paths(width_twips: i32, height_twips: i32) -> Vec<Vec<StaticShapePoint>> {
    let inset_x = width_twips / 6;
    let inset_y = height_twips / 6;
    let right_x = width_twips.saturating_sub(inset_x);
    let bottom_y = height_twips.saturating_sub(inset_y);
    vec![
        vec![
            StaticShapePoint {
                x_twips: inset_x,
                y_twips: inset_y,
            },
            StaticShapePoint {
                x_twips: right_x,
                y_twips: inset_y,
            },
        ],
        vec![
            StaticShapePoint {
                x_twips: right_x,
                y_twips: inset_y,
            },
            StaticShapePoint {
                x_twips: right_x,
                y_twips: bottom_y,
            },
        ],
        vec![
            StaticShapePoint {
                x_twips: right_x,
                y_twips: bottom_y,
            },
            StaticShapePoint {
                x_twips: inset_x,
                y_twips: bottom_y,
            },
        ],
        vec![
            StaticShapePoint {
                x_twips: inset_x,
                y_twips: bottom_y,
            },
            StaticShapePoint {
                x_twips: inset_x,
                y_twips: inset_y,
            },
        ],
    ]
}

fn folded_corner_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let fold_x = (width_twips * 3) / 4;
    let fold_y = height_twips / 4;
    [
        (0, 0),
        (fold_x, 0),
        (width_twips, fold_y),
        (width_twips, height_twips),
        (0, height_twips),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn folded_corner_shape_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    let fold_x = (width_twips * 3) / 4;
    let fold_y = height_twips / 4;
    vec![
        vec![
            StaticShapePoint {
                x_twips: fold_x,
                y_twips: 0,
            },
            StaticShapePoint {
                x_twips: fold_x,
                y_twips: fold_y,
            },
        ],
        vec![
            StaticShapePoint {
                x_twips: fold_x,
                y_twips: fold_y,
            },
            StaticShapePoint {
                x_twips: width_twips,
                y_twips: fold_y,
            },
        ],
    ]
}

fn plaque_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (167, 0),
            (833, 0),
            (872, 9),
            (908, 35),
            (943, 74),
            (977, 124),
            (1000, 167),
            (1000, 833),
            (977, 876),
            (943, 926),
            (908, 965),
            (872, 991),
            (833, 1000),
            (167, 1000),
            (128, 991),
            (92, 965),
            (57, 926),
            (23, 876),
            (0, 833),
            (0, 167),
            (23, 124),
            (57, 74),
            (92, 35),
            (128, 9),
        ],
    )
}

fn double_bracket_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let gap_top = height_twips / 5;
    let gap_bottom = (height_twips * 4) / 5;
    [
        (0, 0),
        (width_twips, 0),
        (width_twips, gap_top),
        ((width_twips * 2) / 3, gap_top),
        ((width_twips * 2) / 3, gap_bottom),
        (width_twips, gap_bottom),
        (width_twips, height_twips),
        (0, height_twips),
        (0, gap_bottom),
        (width_twips / 3, gap_bottom),
        (width_twips / 3, gap_top),
        (0, gap_top),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn double_brace_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (480, 0),
            (240, 0),
            (175, 28),
            (100, 110),
            (72, 188),
            (100, 360),
            (72, 410),
            (0, 500),
            (72, 590),
            (100, 640),
            (72, 812),
            (100, 890),
            (175, 972),
            (240, 1000),
            (480, 1000),
            (480, 820),
            (300, 820),
            (288, 760),
            (300, 620),
            (350, 570),
            (420, 500),
            (350, 430),
            (300, 380),
            (288, 240),
            (300, 180),
            (480, 180),
            (520, 180),
            (700, 180),
            (712, 240),
            (700, 380),
            (650, 430),
            (580, 500),
            (650, 570),
            (700, 620),
            (712, 760),
            (700, 820),
            (520, 820),
            (520, 1000),
            (760, 1000),
            (825, 972),
            (900, 890),
            (928, 812),
            (900, 640),
            (928, 590),
            (1000, 500),
            (928, 410),
            (900, 360),
            (928, 188),
            (900, 110),
            (825, 28),
            (760, 0),
            (520, 0),
        ],
    )
}

fn left_bracket_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (1000, 0),
            (0, 0),
            (0, 1000),
            (1000, 1000),
            (1000, 800),
            (650, 800),
            (545, 790),
            (430, 760),
            (345, 705),
            (333, 620),
            (333, 380),
            (345, 295),
            (430, 240),
            (545, 210),
            (650, 200),
            (1000, 200),
        ],
    )
}

fn right_bracket_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    left_bracket_shape_points(width_twips, height_twips)
        .into_iter()
        .map(|point| StaticShapePoint {
            x_twips: width_twips.saturating_sub(point.x_twips),
            y_twips: point.y_twips,
        })
        .collect()
}

fn left_brace_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (1000, 0),
            (500, 0),
            (430, 8),
            (365, 35),
            (305, 78),
            (250, 125),
            (225, 178),
            (215, 235),
            (225, 292),
            (250, 350),
            (210, 383),
            (150, 420),
            (75, 460),
            (0, 500),
            (75, 540),
            (150, 580),
            (210, 617),
            (250, 650),
            (225, 708),
            (215, 765),
            (225, 822),
            (250, 875),
            (305, 922),
            (365, 965),
            (430, 992),
            (500, 1000),
            (1000, 1000),
            (1000, 800),
            (600, 800),
            (585, 765),
            (565, 720),
            (575, 670),
            (600, 620),
            (645, 588),
            (690, 555),
            (735, 528),
            (780, 500),
            (735, 472),
            (690, 445),
            (645, 412),
            (600, 380),
            (575, 330),
            (565, 280),
            (585, 235),
            (600, 200),
            (1000, 200),
        ],
    )
}

fn right_brace_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    left_brace_shape_points(width_twips, height_twips)
        .into_iter()
        .map(|point| StaticShapePoint {
            x_twips: width_twips.saturating_sub(point.x_twips),
            y_twips: point.y_twips,
        })
        .collect()
}

fn chevron_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let mid_y = height_twips / 2;
    let inset = width_twips / 3;
    [
        (0, 0),
        (width_twips.saturating_sub(inset), 0),
        (width_twips, mid_y),
        (width_twips.saturating_sub(inset), height_twips),
        (0, height_twips),
        (inset, mid_y),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn right_arrow_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let shaft_right = (width_twips * 2) / 3;
    let mid_y = height_twips / 2;
    let shaft_top = height_twips / 4;
    let shaft_bottom = height_twips.saturating_sub(shaft_top);
    [
        (0, shaft_top),
        (shaft_right, shaft_top),
        (shaft_right, 0),
        (width_twips, mid_y),
        (shaft_right, height_twips),
        (shaft_right, shaft_bottom),
        (0, shaft_bottom),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn notched_right_arrow_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let shaft_right = (width_twips * 2) / 3;
    let mid_y = height_twips / 2;
    let shaft_top = height_twips / 4;
    let shaft_bottom = height_twips.saturating_sub(shaft_top);
    let notch_right = width_twips / 5;
    [
        (0, shaft_top),
        (shaft_right, shaft_top),
        (shaft_right, 0),
        (width_twips, mid_y),
        (shaft_right, height_twips),
        (shaft_right, shaft_bottom),
        (0, shaft_bottom),
        (notch_right, mid_y),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn right_arrow_callout_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let body_right = (width_twips * 2) / 3;
    let mid_y = height_twips / 2;
    let shaft_top = height_twips / 4;
    let shaft_bottom = height_twips.saturating_sub(shaft_top);
    [
        (0, 0),
        (body_right, 0),
        (body_right, shaft_top),
        (width_twips, mid_y),
        (body_right, shaft_bottom),
        (body_right, height_twips),
        (0, height_twips),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn right_arrow_callout_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    let body_right = (width_twips * 2) / 3;
    vec![vec![
        StaticShapePoint {
            x_twips: body_right,
            y_twips: 0,
        },
        StaticShapePoint {
            x_twips: body_right,
            y_twips: height_twips,
        },
    ]]
}

fn left_arrow_callout_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let body_left = width_twips / 3;
    let mid_y = height_twips / 2;
    let shaft_top = height_twips / 4;
    let shaft_bottom = height_twips.saturating_sub(shaft_top);
    [
        (body_left, 0),
        (width_twips, 0),
        (width_twips, height_twips),
        (body_left, height_twips),
        (body_left, shaft_bottom),
        (0, mid_y),
        (body_left, shaft_top),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn left_arrow_callout_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    let body_left = width_twips / 3;
    vec![vec![
        StaticShapePoint {
            x_twips: body_left,
            y_twips: 0,
        },
        StaticShapePoint {
            x_twips: body_left,
            y_twips: height_twips,
        },
    ]]
}

fn up_arrow_callout_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let body_top = height_twips / 3;
    let mid_x = width_twips / 2;
    let shaft_left = width_twips / 4;
    let shaft_right = width_twips.saturating_sub(shaft_left);
    [
        (0, body_top),
        (shaft_left, body_top),
        (mid_x, 0),
        (shaft_right, body_top),
        (width_twips, body_top),
        (width_twips, height_twips),
        (0, height_twips),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn up_arrow_callout_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    let body_top = height_twips / 3;
    vec![vec![
        StaticShapePoint {
            x_twips: 0,
            y_twips: body_top,
        },
        StaticShapePoint {
            x_twips: width_twips,
            y_twips: body_top,
        },
    ]]
}

fn down_arrow_callout_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let body_bottom = (height_twips * 2) / 3;
    let mid_x = width_twips / 2;
    let shaft_left = width_twips / 4;
    let shaft_right = width_twips.saturating_sub(shaft_left);
    [
        (0, 0),
        (width_twips, 0),
        (width_twips, body_bottom),
        (shaft_right, body_bottom),
        (mid_x, height_twips),
        (shaft_left, body_bottom),
        (0, body_bottom),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn down_arrow_callout_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    let body_bottom = (height_twips * 2) / 3;
    vec![vec![
        StaticShapePoint {
            x_twips: 0,
            y_twips: body_bottom,
        },
        StaticShapePoint {
            x_twips: width_twips,
            y_twips: body_bottom,
        },
    ]]
}

fn left_right_arrow_callout_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    let head_width = width_twips / 4;
    let mid_y = height_twips / 2;
    let shaft_top = height_twips / 4;
    let shaft_bottom = height_twips.saturating_sub(shaft_top);
    [
        (0, mid_y),
        (head_width, shaft_top),
        (head_width, 0),
        (width_twips.saturating_sub(head_width), 0),
        (width_twips.saturating_sub(head_width), shaft_top),
        (width_twips, mid_y),
        (width_twips.saturating_sub(head_width), shaft_bottom),
        (width_twips.saturating_sub(head_width), height_twips),
        (head_width, height_twips),
        (head_width, shaft_bottom),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn left_right_arrow_callout_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    let head_width = width_twips / 4;
    vec![
        vec![
            StaticShapePoint {
                x_twips: head_width,
                y_twips: 0,
            },
            StaticShapePoint {
                x_twips: head_width,
                y_twips: height_twips,
            },
        ],
        vec![
            StaticShapePoint {
                x_twips: width_twips.saturating_sub(head_width),
                y_twips: 0,
            },
            StaticShapePoint {
                x_twips: width_twips.saturating_sub(head_width),
                y_twips: height_twips,
            },
        ],
    ]
}

fn up_down_arrow_callout_shape_points(
    width_twips: i32,
    height_twips: i32,
) -> Vec<StaticShapePoint> {
    let head_height = height_twips / 4;
    let mid_x = width_twips / 2;
    let shaft_left = width_twips / 4;
    let shaft_right = width_twips.saturating_sub(shaft_left);
    [
        (mid_x, 0),
        (shaft_right, head_height),
        (width_twips, head_height),
        (width_twips, height_twips.saturating_sub(head_height)),
        (shaft_right, height_twips.saturating_sub(head_height)),
        (mid_x, height_twips),
        (shaft_left, height_twips.saturating_sub(head_height)),
        (0, height_twips.saturating_sub(head_height)),
        (0, head_height),
        (shaft_left, head_height),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn up_down_arrow_callout_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    let head_height = height_twips / 4;
    vec![
        vec![
            StaticShapePoint {
                x_twips: 0,
                y_twips: head_height,
            },
            StaticShapePoint {
                x_twips: width_twips,
                y_twips: head_height,
            },
        ],
        vec![
            StaticShapePoint {
                x_twips: 0,
                y_twips: height_twips.saturating_sub(head_height),
            },
            StaticShapePoint {
                x_twips: width_twips,
                y_twips: height_twips.saturating_sub(head_height),
            },
        ],
    ]
}

fn quad_arrow_callout_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let mid_x = width_twips / 2;
    let mid_y = height_twips / 2;
    let body_left = width_twips / 3;
    let body_right = width_twips.saturating_sub(body_left);
    let body_top = height_twips / 3;
    let body_bottom = height_twips.saturating_sub(body_top);
    [
        (mid_x, 0),
        (body_right, body_top),
        (width_twips, body_top),
        (width_twips, mid_y),
        (body_right, body_bottom),
        (body_right, height_twips),
        (mid_x, height_twips),
        (body_left, body_bottom),
        (0, body_bottom),
        (0, mid_y),
        (body_left, body_top),
        (body_left, 0),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn quad_arrow_callout_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    let body_left = width_twips / 3;
    let body_right = width_twips.saturating_sub(body_left);
    let body_top = height_twips / 3;
    let body_bottom = height_twips.saturating_sub(body_top);
    vec![
        vec![
            StaticShapePoint {
                x_twips: body_left,
                y_twips: body_top,
            },
            StaticShapePoint {
                x_twips: body_right,
                y_twips: body_top,
            },
        ],
        vec![
            StaticShapePoint {
                x_twips: body_right,
                y_twips: body_top,
            },
            StaticShapePoint {
                x_twips: body_right,
                y_twips: body_bottom,
            },
        ],
        vec![
            StaticShapePoint {
                x_twips: body_right,
                y_twips: body_bottom,
            },
            StaticShapePoint {
                x_twips: body_left,
                y_twips: body_bottom,
            },
        ],
        vec![
            StaticShapePoint {
                x_twips: body_left,
                y_twips: body_bottom,
            },
            StaticShapePoint {
                x_twips: body_left,
                y_twips: body_top,
            },
        ],
    ]
}

fn left_arrow_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let shaft_left = width_twips / 3;
    let mid_y = height_twips / 2;
    let shaft_top = height_twips / 4;
    let shaft_bottom = height_twips.saturating_sub(shaft_top);
    [
        (width_twips, shaft_top),
        (shaft_left, shaft_top),
        (shaft_left, 0),
        (0, mid_y),
        (shaft_left, height_twips),
        (shaft_left, shaft_bottom),
        (width_twips, shaft_bottom),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn up_arrow_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let mid_x = width_twips / 2;
    let shaft_left = width_twips / 4;
    let shaft_right = width_twips.saturating_sub(shaft_left);
    let shaft_top = height_twips / 3;
    [
        (mid_x, 0),
        (width_twips, shaft_top),
        (shaft_right, shaft_top),
        (shaft_right, height_twips),
        (shaft_left, height_twips),
        (shaft_left, shaft_top),
        (0, shaft_top),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn down_arrow_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let mid_x = width_twips / 2;
    let shaft_left = width_twips / 4;
    let shaft_right = width_twips.saturating_sub(shaft_left);
    let shaft_bottom = (height_twips * 2) / 3;
    [
        (shaft_left, 0),
        (shaft_right, 0),
        (shaft_right, shaft_bottom),
        (width_twips, shaft_bottom),
        (mid_x, height_twips),
        (0, shaft_bottom),
        (shaft_left, shaft_bottom),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn left_right_arrow_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let head_width = width_twips / 4;
    let mid_y = height_twips / 2;
    let shaft_top = height_twips / 4;
    let shaft_bottom = height_twips.saturating_sub(shaft_top);
    [
        (0, mid_y),
        (head_width, 0),
        (head_width, shaft_top),
        (width_twips.saturating_sub(head_width), shaft_top),
        (width_twips.saturating_sub(head_width), 0),
        (width_twips, mid_y),
        (width_twips.saturating_sub(head_width), height_twips),
        (width_twips.saturating_sub(head_width), shaft_bottom),
        (head_width, shaft_bottom),
        (head_width, height_twips),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn up_down_arrow_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let mid_x = width_twips / 2;
    let head_height = height_twips / 4;
    let shaft_left = width_twips / 4;
    let shaft_right = width_twips.saturating_sub(shaft_left);
    [
        (mid_x, 0),
        (width_twips, head_height),
        (shaft_right, head_height),
        (shaft_right, height_twips.saturating_sub(head_height)),
        (width_twips, height_twips.saturating_sub(head_height)),
        (mid_x, height_twips),
        (0, height_twips.saturating_sub(head_height)),
        (shaft_left, height_twips.saturating_sub(head_height)),
        (shaft_left, head_height),
        (0, head_height),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn quad_arrow_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let mid_x = width_twips / 2;
    let mid_y = height_twips / 2;
    let arm_left = width_twips / 3;
    let arm_right = width_twips.saturating_sub(arm_left);
    let arm_top = height_twips / 3;
    let arm_bottom = height_twips.saturating_sub(arm_top);
    [
        (mid_x, 0),
        (arm_right, arm_top),
        (width_twips, arm_top),
        (width_twips, mid_y),
        (arm_right, arm_bottom),
        (mid_x, height_twips),
        (arm_left, arm_bottom),
        (0, arm_bottom),
        (0, mid_y),
        (0, arm_top),
        (arm_left, arm_top),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn left_right_up_arrow_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let mid_x = width_twips / 2;
    let mid_y = height_twips / 2;
    let arm_left = width_twips / 3;
    let arm_right = width_twips.saturating_sub(arm_left);
    let arm_top = height_twips / 3;
    let arm_bottom = height_twips.saturating_sub(height_twips / 4);
    [
        (mid_x, 0),
        (arm_right, arm_top),
        (width_twips, arm_top),
        (width_twips, mid_y),
        (arm_right, arm_bottom),
        (arm_right, height_twips),
        (arm_left, height_twips),
        (arm_left, arm_bottom),
        (0, arm_bottom),
        (0, mid_y),
        (arm_left, arm_top),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn bent_arrow_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let shaft_left = width_twips / 4;
    let shaft_right = width_twips / 2;
    let bend_top = height_twips / 4;
    let bend_bottom = height_twips / 2;
    let arrow_base_x = (width_twips * 3) / 4;
    [
        (shaft_left, height_twips),
        (shaft_left, bend_bottom),
        (0, bend_bottom),
        (0, bend_top),
        (arrow_base_x, bend_top),
        (arrow_base_x, 0),
        (width_twips, (bend_top + 1) / 2),
        (arrow_base_x, bend_top),
        (arrow_base_x, bend_bottom),
        (shaft_right, bend_bottom),
        (shaft_right, height_twips),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn uturn_arrow_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (400, 1000),
            (400, 600),
            (200, 600),
            (300, 520),
            (350, 475),
            (420, 430),
            (500, 400),
            (580, 430),
            (650, 475),
            (700, 520),
            (800, 600),
            (600, 600),
            (600, 200),
            (200, 200),
            (146, 207),
            (100, 225),
            (64, 254),
            (36, 292),
            (16, 340),
            (0, 400),
            (0, 0),
            (800, 0),
            (800, 1000),
        ],
    )
}

fn mirror_shape_points_x(width_twips: i32, points: Vec<StaticShapePoint>) -> Vec<StaticShapePoint> {
    points
        .into_iter()
        .map(|point| StaticShapePoint {
            x_twips: width_twips.saturating_sub(point.x_twips),
            y_twips: point.y_twips,
        })
        .collect()
}

fn curved_right_arrow_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (120, 1000),
            (120, 805),
            (120, 620),
            (128, 555),
            (140, 495),
            (160, 435),
            (174, 408),
            (190, 380),
            (225, 325),
            (260, 280),
            (284, 258),
            (310, 238),
            (360, 205),
            (420, 175),
            (450, 164),
            (480, 155),
            (545, 140),
            (610, 125),
            (685, 125),
            (760, 125),
            (760, 0),
            (1000, 250),
            (760, 500),
            (760, 360),
            (615, 360),
            (575, 366),
            (535, 375),
            (495, 388),
            (474, 396),
            (455, 405),
            (420, 430),
            (390, 455),
            (365, 485),
            (356, 500),
            (350, 515),
            (338, 555),
            (325, 600),
            (322, 622),
            (318, 645),
            (310, 690),
            (310, 845),
            (310, 1000),
        ],
    )
}

fn curved_left_arrow_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    mirror_shape_points_x(
        width_twips,
        curved_right_arrow_shape_points(width_twips, height_twips),
    )
}

fn curved_down_arrow_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (0, 120),
            (195, 120),
            (380, 120),
            (445, 128),
            (505, 140),
            (565, 160),
            (592, 174),
            (620, 190),
            (675, 225),
            (720, 260),
            (742, 284),
            (762, 310),
            (795, 360),
            (825, 420),
            (836, 450),
            (845, 480),
            (860, 545),
            (875, 610),
            (875, 685),
            (875, 760),
            (1000, 760),
            (750, 1000),
            (500, 760),
            (640, 760),
            (640, 615),
            (634, 575),
            (625, 535),
            (612, 495),
            (604, 474),
            (595, 455),
            (570, 420),
            (545, 390),
            (515, 365),
            (500, 356),
            (485, 350),
            (445, 338),
            (400, 325),
            (378, 322),
            (355, 318),
            (310, 310),
            (155, 310),
            (0, 310),
        ],
    )
}

fn curved_up_arrow_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (0, 880),
            (195, 880),
            (380, 880),
            (445, 872),
            (505, 860),
            (565, 840),
            (592, 826),
            (620, 810),
            (675, 775),
            (720, 740),
            (742, 716),
            (762, 690),
            (795, 640),
            (825, 580),
            (836, 550),
            (845, 520),
            (860, 455),
            (875, 390),
            (875, 315),
            (875, 240),
            (1000, 240),
            (750, 0),
            (500, 240),
            (640, 240),
            (640, 385),
            (634, 425),
            (625, 465),
            (612, 505),
            (604, 526),
            (595, 545),
            (570, 580),
            (545, 610),
            (515, 635),
            (500, 644),
            (485, 650),
            (445, 662),
            (400, 675),
            (378, 678),
            (355, 682),
            (310, 690),
            (155, 690),
            (0, 690),
        ],
    )
}

fn striped_right_arrow_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    right_arrow_shape_points(width_twips, height_twips)
}

fn striped_right_arrow_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    let shaft_top = height_twips / 4;
    let shaft_bottom = height_twips.saturating_sub(shaft_top);
    [width_twips / 8, width_twips / 4]
        .into_iter()
        .map(|x_twips| {
            vec![
                StaticShapePoint {
                    x_twips,
                    y_twips: shaft_top,
                },
                StaticShapePoint {
                    x_twips,
                    y_twips: shaft_bottom,
                },
            ]
        })
        .collect()
}

fn left_up_arrow_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let mid_x = width_twips / 2;
    let mid_y = height_twips / 2;
    let arm_left = width_twips / 3;
    let arm_right = width_twips.saturating_sub(arm_left);
    let arm_top = height_twips / 3;
    let arm_bottom = height_twips.saturating_sub(height_twips / 3);
    [
        (mid_x, 0),
        (width_twips, arm_top),
        (arm_right, arm_top),
        (arm_right, height_twips),
        (arm_left, height_twips),
        (arm_left, arm_bottom),
        (0, mid_y),
        (arm_left, arm_top),
        (arm_left, 0),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn bent_up_arrow_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    let mid_x = width_twips / 2;
    let shaft_left = width_twips / 3;
    let shaft_right = width_twips.saturating_sub(width_twips / 4);
    let arrow_base_y = height_twips / 3;
    let shaft_top = height_twips / 2;
    [
        (mid_x, 0),
        (width_twips, arrow_base_y),
        (shaft_right, arrow_base_y),
        (shaft_right, height_twips),
        (0, height_twips),
        (0, shaft_top),
        (shaft_left, shaft_top),
        (shaft_left, arrow_base_y),
        (mid_x, arrow_base_y),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn scaled_shape_points(
    width_twips: i32,
    height_twips: i32,
    points_per_mille: &[(i32, i32)],
) -> Vec<StaticShapePoint> {
    points_per_mille
        .iter()
        .map(|&(x, y)| StaticShapePoint {
            x_twips: ((i64::from(width_twips) * i64::from(x)) / 1000) as i32,
            y_twips: ((i64::from(height_twips) * i64::from(y)) / 1000) as i32,
        })
        .collect()
}

fn regular_polygon_shape_points(
    width_twips: i32,
    height_twips: i32,
    points: usize,
) -> Vec<StaticShapePoint> {
    regular_polygon_inset_shape_points(width_twips, height_twips, points, 1000)
}

fn donut_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    regular_polygon_shape_points(width_twips, height_twips, 48)
}

fn donut_shape_point_paths(width_twips: i32, height_twips: i32) -> Vec<Vec<StaticShapePoint>> {
    vec![regular_polygon_inset_shape_points(
        width_twips,
        height_twips,
        48,
        460,
    )]
}

fn no_symbol_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    regular_polygon_shape_points(width_twips, height_twips, 48)
}

fn no_symbol_shape_point_paths(width_twips: i32, height_twips: i32) -> Vec<Vec<StaticShapePoint>> {
    donut_shape_point_paths(width_twips, height_twips)
}

fn no_symbol_shape_overlay_paths(
    width_twips: i32,
    height_twips: i32,
) -> Vec<Vec<StaticShapePoint>> {
    vec![scaled_shape_points(
        width_twips,
        height_twips,
        &[(230, 120), (880, 770), (770, 880), (120, 230)],
    )]
}

fn smiley_face_overlay_paths(width_twips: i32, height_twips: i32) -> Vec<Vec<StaticShapePoint>> {
    vec![
        scaled_shape_points(width_twips, height_twips, &[(350, 315), (350, 425)]),
        scaled_shape_points(width_twips, height_twips, &[(650, 315), (650, 425)]),
        scaled_shape_points(
            width_twips,
            height_twips,
            &[
                (280, 610),
                (320, 670),
                (365, 720),
                (408, 748),
                (450, 765),
                (500, 775),
                (550, 765),
                (592, 748),
                (635, 720),
                (680, 670),
                (720, 610),
            ],
        ),
    ]
}

fn sun_shape_overlay_paths(width_twips: i32, height_twips: i32) -> Vec<Vec<StaticShapePoint>> {
    let mut center_ring = regular_polygon_inset_shape_points(width_twips, height_twips, 48, 500);
    if let Some(first) = center_ring.first().copied() {
        center_ring.push(first);
    }
    vec![center_ring]
}

fn regular_polygon_inset_shape_points(
    width_twips: i32,
    height_twips: i32,
    points: usize,
    radius_per_mille: i32,
) -> Vec<StaticShapePoint> {
    if points < 3 {
        return Vec::new();
    }

    let center_x = f64::from(width_twips) / 2.0;
    let center_y = f64::from(height_twips) / 2.0;
    let radius_scale = f64::from(radius_per_mille.clamp(1, 1000)) / 1000.0;
    let radius_x = center_x * radius_scale;
    let radius_y = center_y * radius_scale;

    (0..points)
        .map(|index| {
            let angle = -std::f64::consts::FRAC_PI_2
                + (index as f64 * std::f64::consts::TAU / points as f64);
            let x = (center_x + radius_x * angle.cos())
                .round()
                .clamp(0.0, f64::from(width_twips)) as i32;
            let y = (center_y + radius_y * angle.sin())
                .round()
                .clamp(0.0, f64::from(height_twips)) as i32;
            StaticShapePoint {
                x_twips: x,
                y_twips: y,
            }
        })
        .collect()
}

fn heart_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (500, 965),
            (455, 905),
            (405, 845),
            (335, 780),
            (265, 710),
            (195, 640),
            (130, 565),
            (80, 500),
            (45, 430),
            (20, 355),
            (20, 285),
            (35, 210),
            (70, 150),
            (120, 95),
            (185, 65),
            (218, 56),
            (250, 50),
            (285, 50),
            (320, 55),
            (380, 80),
            (430, 125),
            (448, 148),
            (465, 175),
            (482, 204),
            (500, 235),
            (518, 204),
            (535, 175),
            (552, 148),
            (570, 125),
            (620, 80),
            (680, 55),
            (715, 50),
            (750, 50),
            (782, 56),
            (815, 65),
            (880, 95),
            (930, 150),
            (965, 210),
            (980, 285),
            (980, 355),
            (955, 430),
            (920, 500),
            (870, 565),
            (805, 640),
            (735, 710),
            (665, 780),
            (595, 845),
            (545, 905),
        ],
    )
}

fn lightning_bolt_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (620, 0),
            (215, 475),
            (455, 475),
            (320, 1000),
            (785, 380),
            (545, 380),
        ],
    )
}

fn moon_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (775, 40),
            (730, 48),
            (685, 60),
            (600, 95),
            (520, 145),
            (455, 210),
            (405, 285),
            (365, 365),
            (354, 400),
            (345, 435),
            (341, 452),
            (338, 468),
            (336, 484),
            (335, 500),
            (336, 516),
            (338, 532),
            (341, 548),
            (345, 565),
            (354, 600),
            (365, 635),
            (405, 715),
            (455, 790),
            (520, 855),
            (600, 905),
            (685, 940),
            (730, 952),
            (775, 960),
            (695, 985),
            (610, 995),
            (588, 998),
            (565, 995),
            (542, 993),
            (520, 990),
            (435, 965),
            (350, 930),
            (275, 875),
            (205, 805),
            (145, 725),
            (100, 635),
            (70, 545),
            (70, 455),
            (100, 365),
            (145, 275),
            (205, 195),
            (275, 125),
            (350, 70),
            (435, 35),
            (520, 10),
            (610, 5),
        ],
    )
}

fn explosion1_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (500, 0),
            (585, 275),
            (830, 85),
            (750, 365),
            (1000, 500),
            (720, 590),
            (915, 850),
            (620, 760),
            (500, 1000),
            (380, 760),
            (85, 850),
            (280, 590),
            (0, 500),
            (250, 365),
            (170, 85),
            (415, 275),
        ],
    )
}

fn explosion2_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    scaled_shape_points(
        width_twips,
        height_twips,
        &[
            (520, 0),
            (620, 270),
            (820, 60),
            (770, 335),
            (1000, 380),
            (780, 520),
            (955, 705),
            (690, 690),
            (700, 1000),
            (520, 780),
            (330, 980),
            (340, 700),
            (55, 745),
            (235, 540),
            (0, 390),
            (270, 340),
            (195, 80),
            (420, 255),
        ],
    )
}

fn five_point_star_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    [
        (width_twips / 2, 0),
        ((width_twips * 61) / 100, (height_twips * 35) / 100),
        (width_twips, (height_twips * 38) / 100),
        ((width_twips * 68) / 100, (height_twips * 58) / 100),
        ((width_twips * 79) / 100, height_twips),
        (width_twips / 2, (height_twips * 72) / 100),
        ((width_twips * 21) / 100, height_twips),
        ((width_twips * 32) / 100, (height_twips * 58) / 100),
        (0, (height_twips * 38) / 100),
        ((width_twips * 39) / 100, (height_twips * 35) / 100),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn eight_point_star_shape_points(width_twips: i32, height_twips: i32) -> Vec<StaticShapePoint> {
    [
        (width_twips / 2, 0),
        ((width_twips * 60) / 100, (height_twips * 30) / 100),
        (width_twips, 0),
        ((width_twips * 70) / 100, (height_twips * 40) / 100),
        (width_twips, height_twips / 2),
        ((width_twips * 70) / 100, (height_twips * 60) / 100),
        (width_twips, height_twips),
        ((width_twips * 60) / 100, (height_twips * 70) / 100),
        (width_twips / 2, height_twips),
        ((width_twips * 40) / 100, (height_twips * 70) / 100),
        (0, height_twips),
        ((width_twips * 30) / 100, (height_twips * 60) / 100),
        (0, height_twips / 2),
        ((width_twips * 30) / 100, (height_twips * 40) / 100),
        (0, 0),
        ((width_twips * 40) / 100, (height_twips * 30) / 100),
    ]
    .into_iter()
    .map(|(x, y)| StaticShapePoint {
        x_twips: x,
        y_twips: y,
    })
    .collect()
}

fn regular_star_shape_points(
    width_twips: i32,
    height_twips: i32,
    outer_points: usize,
    inner_radius_per_mille: i32,
) -> Vec<StaticShapePoint> {
    let center_x = f64::from(width_twips) / 2.0;
    let center_y = f64::from(height_twips) / 2.0;
    let radius_x = center_x;
    let radius_y = center_y;
    let inner_scale = f64::from(inner_radius_per_mille.clamp(0, 1000)) / 1000.0;
    let total_points = outer_points.saturating_mul(2);
    (0..total_points)
        .map(|index| {
            let angle = -std::f64::consts::FRAC_PI_2
                + (index as f64 * std::f64::consts::PI) / outer_points as f64;
            let scale = if index % 2 == 0 { 1.0 } else { inner_scale };
            let x = (center_x + angle.cos() * radius_x * scale).round() as i32;
            let y = (center_y + angle.sin() * radius_y * scale).round() as i32;
            StaticShapePoint {
                x_twips: x.clamp(0, width_twips),
                y_twips: y.clamp(0, height_twips),
            }
        })
        .collect()
}

pub(in crate::rtf::parser) fn rotated_shape_point(
    x_twips: i32,
    y_twips: i32,
    width_twips: i32,
    height_twips: i32,
    rotation_units: i32,
) -> StaticShapePoint {
    let radians = (rotation_units as f64 / 65_536.0).to_radians();
    let (sin, cos) = radians.sin_cos();
    let center_x = width_twips as f64 / 2.0;
    let center_y = height_twips as f64 / 2.0;
    let dx = x_twips as f64 - center_x;
    let dy = y_twips as f64 - center_y;
    StaticShapePoint {
        x_twips: bounded_rotated_shape_coordinate(center_x + (dx * cos) - (dy * sin)),
        y_twips: bounded_rotated_shape_coordinate(center_y + (dx * sin) + (dy * cos)),
    }
}

fn bounded_rotated_shape_coordinate(value: f64) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    value.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32
}

pub(in crate::rtf::parser) fn paragraph_shading_pattern_control(
    name: &str,
) -> Option<ShadingPattern> {
    match name {
        "bghoriz" => Some(ShadingPattern::Horizontal),
        "bgvert" => Some(ShadingPattern::Vertical),
        "bgfdiag" => Some(ShadingPattern::ForwardDiagonal),
        "bgbdiag" => Some(ShadingPattern::BackwardDiagonal),
        "bgcross" => Some(ShadingPattern::Cross),
        "bgdcross" => Some(ShadingPattern::DiagonalCross),
        "bgdkhoriz" => Some(ShadingPattern::DarkHorizontal),
        "bgdkvert" => Some(ShadingPattern::DarkVertical),
        "bgdkfdiag" => Some(ShadingPattern::DarkForwardDiagonal),
        "bgdkbdiag" => Some(ShadingPattern::DarkBackwardDiagonal),
        "bgdkcross" => Some(ShadingPattern::DarkCross),
        "bgdkdcross" => Some(ShadingPattern::DarkDiagonalCross),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::rtf::parser) enum OldDrawingFillPattern {
    None,
    Solid,
    Hatch(ShadingPattern),
}

pub(in crate::rtf::parser) fn old_drawing_fill_pattern(value: i32) -> OldDrawingFillPattern {
    match value {
        0 => OldDrawingFillPattern::None,
        14 => OldDrawingFillPattern::Hatch(ShadingPattern::DarkHorizontal),
        15 => OldDrawingFillPattern::Hatch(ShadingPattern::DarkVertical),
        16 => OldDrawingFillPattern::Hatch(ShadingPattern::DarkForwardDiagonal),
        17 => OldDrawingFillPattern::Hatch(ShadingPattern::DarkBackwardDiagonal),
        18 => OldDrawingFillPattern::Hatch(ShadingPattern::DarkCross),
        19 => OldDrawingFillPattern::Hatch(ShadingPattern::DarkDiagonalCross),
        20 => OldDrawingFillPattern::Hatch(ShadingPattern::Horizontal),
        21 => OldDrawingFillPattern::Hatch(ShadingPattern::Vertical),
        22 => OldDrawingFillPattern::Hatch(ShadingPattern::ForwardDiagonal),
        23 => OldDrawingFillPattern::Hatch(ShadingPattern::BackwardDiagonal),
        24 => OldDrawingFillPattern::Hatch(ShadingPattern::Cross),
        25 => OldDrawingFillPattern::Hatch(ShadingPattern::DiagonalCross),
        _ => OldDrawingFillPattern::Solid,
    }
}

pub(in crate::rtf::parser) fn table_cell_shading_pattern_control(
    name: &str,
) -> Option<ShadingPattern> {
    match name {
        "clbghoriz" => Some(ShadingPattern::Horizontal),
        "clbgvert" => Some(ShadingPattern::Vertical),
        "clbgfdiag" => Some(ShadingPattern::ForwardDiagonal),
        "clbgbdiag" => Some(ShadingPattern::BackwardDiagonal),
        "clbgcross" => Some(ShadingPattern::Cross),
        "clbgdcross" => Some(ShadingPattern::DiagonalCross),
        "clbgdkhor" | "clbgdkhoriz" => Some(ShadingPattern::DarkHorizontal),
        "clbgdkvert" => Some(ShadingPattern::DarkVertical),
        "clbgdkfdiag" => Some(ShadingPattern::DarkForwardDiagonal),
        "clbgdkbdiag" => Some(ShadingPattern::DarkBackwardDiagonal),
        "clbgdkcross" => Some(ShadingPattern::DarkCross),
        "clbgdkdcross" => Some(ShadingPattern::DarkDiagonalCross),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn table_row_shading_pattern_control(
    name: &str,
) -> Option<ShadingPattern> {
    match name {
        "trbghoriz" => Some(ShadingPattern::Horizontal),
        "trbgvert" => Some(ShadingPattern::Vertical),
        "trbgfdiag" => Some(ShadingPattern::ForwardDiagonal),
        "trbgbdiag" => Some(ShadingPattern::BackwardDiagonal),
        "trbgcross" => Some(ShadingPattern::Cross),
        "trbgdcross" => Some(ShadingPattern::DiagonalCross),
        "trbgdkhor" | "trbgdkhoriz" => Some(ShadingPattern::DarkHorizontal),
        "trbgdkvert" => Some(ShadingPattern::DarkVertical),
        "trbgdkfdiag" => Some(ShadingPattern::DarkForwardDiagonal),
        "trbgdkbdiag" => Some(ShadingPattern::DarkBackwardDiagonal),
        "trbgdkcross" => Some(ShadingPattern::DarkCross),
        "trbgdkdcross" => Some(ShadingPattern::DarkDiagonalCross),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn word_compound_border_variant_style(
    name: &str,
) -> Option<BorderStyle> {
    match name {
        "brdrtnthsg" | "brdrtnthmg" | "brdrtnthlg" => Some(BorderStyle::ThinThick),
        "brdrthtnsg" | "brdrthtnmg" | "brdrthtnlg" => Some(BorderStyle::ThickThin),
        "brdrtnthtnsg" | "brdrtnthtnmg" | "brdrtnthtnlg" => Some(BorderStyle::ThinThickThin),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn is_known_ignored_control(name: &str) -> bool {
    paragraph_shading_pattern_control(name).is_some()
        || table_cell_shading_pattern_control(name).is_some()
        || table_row_shading_pattern_control(name).is_some()
        || word_compound_border_variant_style(name).is_some()
        || matches!(
            name,
            "brdrinset" | "brdroutset" | "brdrengrave" | "brdremboss"
        )
        || matches!(
            name,
            "ansicpg"
                | "cocoartf"
                | "cocoasubrtf"
                | "cpg"
                | "adjustright"
                | "af"
                | "adeflang"
                | "alang"
                | "dbch"
                | "deflang"
                | "deflangfe"
                | "dghorigin"
                | "dghshow"
                | "dghspace"
                | "dgvorigin"
                | "dgvshow"
                | "dgvspace"
                | "doctype"
                | "donotembedlingdata"
                | "donotembedsysfont"
                | "fbidis"
                | "fromtext"
                | "fcharset"
                | "fbimajor"
                | "fbiminor"
                | "fdbmajor"
                | "fdbminor"
                | "fhimajor"
                | "fhiminor"
                | "fbias"
                | "flddirty"
                | "fldedit"
                | "fldlock"
                | "fldpriv"
                | "flomajor"
                | "flominor"
                | "fromhtml"
                | "fprq"
                | "fttruetype"
                | "fmodern"
                | "fnil"
                | "froman"
                | "fswiss"
                | "fscript"
                | "fdecor"
                | "ftech"
                | "horzdoc"
                | "itap"
                | "intbl"
                | "listtable"
                | "listoverridetable"
                | "hich"
                | "htmlrtf"
                | "lang"
                | "langfe"
                | "langfenp"
                | "langnp"
                | "jclisttab"
                | "loch"
                | "ltrch"
                | "ltrpar"
                | "nobrkwrptbl"
                | "nouicompat"
                | "oldas"
                | "pn"
                | "pnb"
                | "pncaps"
                | "pncard"
                | "pncf"
                | "pndec"
                | "pnf"
                | "pnfs"
                | "pnhang"
                | "pni"
                | "pnindent"
                | "pnlcltr"
                | "pnlcrm"
                | "pnlvlblt"
                | "pnlvlbody"
                | "pnlvlcont"
                | "pnordt"
                | "pnqc"
                | "pnql"
                | "pnqr"
                | "pnrnot"
                | "pnrestart"
                | "pnseclvl"
                | "pnsp"
                | "pnstart"
                | "pnstrike"
                | "pnul"
                | "pnuldb"
                | "pnuld"
                | "pnuldash"
                | "pnuldashd"
                | "pnuldashdd"
                | "pnulnone"
                | "pnulw"
                | "pnulwave"
                | "pnucltr"
                | "pnucrm"
                | "charrsid"
                | "cgrid"
                | "delrsid"
                | "insrsid"
                | "outlinelevel"
                | "pararsid"
                | "revauth"
                | "revdttm"
                | "revised"
                | "rsid"
                | "rsidroot"
                | "sectrsid"
                | "sectunlocked"
                | "softcol"
                | "softline"
                | "softpage"
                | "subdocument"
                | "saveinvalidxml"
                | "showxmlerrors"
                | "sprstsp"
                | "rempersonalinfo"
                | "trackformatting"
                | "trackmoves"
                | "themelang"
                | "themelangcs"
                | "themelangfe"
                | "tsrsid"
                | "trowd"
                | "cellx"
                | "clftsWidth"
                | "clwWidth"
                | "trftsWidth"
                | "trwWidth"
                | "usenormstyforlist"
                | "validatexml"
                | "viewbksp"
                | "viewnobound"
                | "viewscale"
                | "viewzk"
                | "xmlclose"
                | "xmlopen"
                | "formdisp"
                | "taprtl"
                | "rtlrow"
                | "allprot"
                | "annotprot"
                | "enforceprot"
                | "formprot"
                | "protlevel"
                | "readprot"
                | "readonlyrecommended"
                | "revisions"
                | "revprot"
                | "rtlch"
        )
}

pub(in crate::rtf::parser) fn word_layout_compatibility_control_message(
    name: &str,
) -> Option<&'static str> {
    match name {
        "jexpand" | "jcompress" => {
            Some("Japanese text justification interpreted through bounded passive line layout")
        }
        "asianbrkrule" => {
            Some("Asian line-breaking rule interpreted through bounded passive Unicode line layout")
        }
        "faauto" => {
            Some("East Asian font alignment interpreted through bounded passive font metrics")
        }
        "twoinone" | "horzvert" => {
            Some("East Asian inline layout rendered through bounded passive horizontal fallback")
        }
        "aspalpha" | "aspnum" => {
            Some("Word auto-spacing compatibility interpreted through bounded passive text layout")
        }
        "sectexpand" | "sectspecifycl" | "sectspecifyl" => {
            Some("section text grid interpreted through bounded passive paragraph layout")
        }
        "dxfrtext" | "dfrmtxtx" | "dfrmtxty" => {
            Some("paragraph wrap distance interpreted through bounded passive flow layout")
        }
        "absw" | "absh" => {
            Some("paragraph frame dimensions interpreted through bounded passive flow layout")
        }
        "lytexcttp" | "lytprtmet" | "noextrasprl" | "notcvasp" | "notvatxbx" | "expshrtn"
        | "useltbaln" | "htmautsp" | "wraptrsp" | "sprsspbf" | "sprsbsp" | "nooverflow"
        | "wpjst" | "wpbrdr" | "otblrul" => {
            Some("Word typography compatibility option interpreted through bounded passive layout")
        }
        "vertdoc" => {
            Some("vertical document layout rendered through bounded passive horizontal fallback")
        }
        _ => None,
    }
}

pub(in crate::rtf::parser) fn append_shape_property_text(
    target: &mut String,
    text: &str,
    limit: usize,
    offset: usize,
) -> Result<(), ParseError> {
    let new_len = target
        .chars()
        .count()
        .checked_add(text.chars().count())
        .ok_or(ParseError::OutputTextTooLarge(offset))?;
    if new_len > limit {
        return Err(ParseError::ResourceLimitExceeded {
            resource: "shape property text".to_string(),
            offset,
        });
    }
    target.push_str(text);
    Ok(())
}

pub(in crate::rtf::parser) fn parse_shape_property_i64(value: &str) -> Option<i64> {
    value.trim().parse::<i64>().ok()
}

pub(in crate::rtf::parser) fn parse_shape_rotation_units(value: &str) -> Option<i32> {
    // OfficeArt's legacy `rotation` property is a 16.16 fixed-point
    // degree value, unlike DrawingML's later 1/60000-degree angles.
    const OFFICE_ROTATION_UNITS_PER_DEGREE: i64 = 65_536;
    const FULL_ROTATION_UNITS: i64 = 360 * OFFICE_ROTATION_UNITS_PER_DEGREE;
    let value = parse_shape_property_i64(value)?;
    let normalized = value.rem_euclid(FULL_ROTATION_UNITS);
    i32::try_from(normalized).ok()
}

pub(in crate::rtf::parser) fn parse_shape_property_emu_twips(value: &str) -> Option<i64> {
    let emu = parse_shape_property_i64(value)?;
    Some(((emu.max(0).saturating_add(317)) / 635).min(i32::MAX as i64))
}

pub(in crate::rtf::parser) fn parse_shape_property_signed_emu_twips(value: &str) -> Option<i64> {
    let emu = parse_shape_property_i64(value)?;
    let magnitude = (emu.unsigned_abs().saturating_add(317) / 635).min(i32::MAX as u64) as i64;
    Some(if emu < 0 { -magnitude } else { magnitude })
}

pub(in crate::rtf::parser) fn parse_shape_opacity_percent_property(value: &str) -> Option<u8> {
    let raw = parse_shape_property_i64(value)?;
    if raw < 0 {
        return None;
    }
    let percent = raw
        .saturating_mul(100)
        .saturating_add(32_768)
        .checked_div(65_536)?
        .clamp(0, 100);
    u8::try_from(percent).ok()
}

pub(in crate::rtf::parser) fn parse_shape_picture_fixed_property(value: &str) -> Option<i32> {
    let raw = parse_shape_property_i64(value)?;
    i32::try_from(raw.clamp(-65_536, 196_608)).ok()
}

pub(in crate::rtf::parser) fn parse_office_shape_color(value: &str) -> Option<Color> {
    let value = u32::try_from(parse_shape_property_i64(value)?).ok()?;
    if value > 0x00ff_ffff {
        return None;
    }
    Some(Color {
        red: (value & 0xff) as u8,
        green: ((value >> 8) & 0xff) as u8,
        blue: ((value >> 16) & 0xff) as u8,
    })
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(in crate::rtf::parser) enum ShapeFillType {
    Solid,
    Pattern,
    Shade,
    Background,
    Unsupported,
}

pub(in crate::rtf::parser) fn parse_shape_fill_type_property(value: &str) -> Option<ShapeFillType> {
    let normalized = value
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    match normalized.as_str() {
        "0" | "solid" | "msofillsolid" => Some(ShapeFillType::Solid),
        "1" | "pattern" | "msofillpattern" => Some(ShapeFillType::Pattern),
        "4" | "shade" | "msofillshade" | "5" | "shadecenter" | "msofillshadecenter" | "6"
        | "shadeshape" | "msofillshadeshape" | "7" | "shadescale" | "msofillshadescale" => {
            Some(ShapeFillType::Shade)
        }
        "9" | "background" | "msofillbackground" => Some(ShapeFillType::Background),
        "2" | "texture" | "msofilltexture" | "3" | "picture" | "msofillpicture" | "8"
        | "shadepreset" | "msofillshadepreset" => Some(ShapeFillType::Unsupported),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn parse_shape_line_dashing_property(
    value: &str,
) -> Option<BorderStyle> {
    let normalized = value
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    match normalized.as_str() {
        "0" | "1" | "solid" | "single" => Some(BorderStyle::Single),
        "2" | "3" | "dot" | "sysdot" | "squaredot" | "rounddot" | "linestyle2" | "linestyle3" => {
            Some(BorderStyle::Dotted)
        }
        "4" | "5" | "6" | "7" | "8" | "9" | "10" | "12" | "dash" | "sysdash" | "lgdash"
        | "longdash" | "dashdot" | "lgdashdot" | "dashdotdot" | "lgdashdotdot" | "longdashdot"
        | "longdashdotdot" | "sysdashdot" => Some(BorderStyle::Dashed),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn parse_shape_line_style_property(value: &str) -> Option<BorderStyle> {
    let normalized = value
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    match normalized.as_str() {
        "0" | "1" | "single" | "msolinesingle" => Some(BorderStyle::Single),
        "2" | "double" | "msolinedouble" | "thinthin" | "msolinethinthin" => {
            Some(BorderStyle::Double)
        }
        "3" | "thinthick" | "msolinethinthick" => Some(BorderStyle::ThinThick),
        "4" | "thickthin" | "msolinethickthin" => Some(BorderStyle::ThickThin),
        "5" | "thickbetweenthin" | "msolinethickbetweenthin" => Some(BorderStyle::ThinThickThin),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn parse_shape_line_cap_property(
    value: &str,
) -> Option<StaticShapeLineCap> {
    let normalized = value
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    match normalized.as_str() {
        "0" | "flat" | "butt" | "msolinecapflat" => Some(StaticShapeLineCap::Flat),
        "1" | "square" | "msolinecapsquare" => Some(StaticShapeLineCap::Square),
        "2" | "round" | "msolinecapround" => Some(StaticShapeLineCap::Round),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn parse_shape_line_join_property(
    value: &str,
) -> Option<StaticShapeLineJoin> {
    let normalized = normalize_shape_enum_property(value);
    match normalized.as_str() {
        "0" | "3" | "miter" | "msolinejoinmiter" => Some(StaticShapeLineJoin::Miter),
        "1" | "round" | "msolinejoinround" => Some(StaticShapeLineJoin::Round),
        "2" | "bevel" | "msolinejoinbevel" => Some(StaticShapeLineJoin::Bevel),
        _ => None,
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(in crate::rtf::parser) struct ShapeTextAnchor {
    pub(in crate::rtf::parser) vertical: StaticShapeTextVerticalAnchor,
    pub(in crate::rtf::parser) center_horizontal: bool,
}

pub(in crate::rtf::parser) fn parse_shape_text_anchor_property(
    value: &str,
) -> Option<ShapeTextAnchor> {
    let normalized = value
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    match normalized.as_str() {
        "0" | "top" | "t" => Some(ShapeTextAnchor {
            vertical: StaticShapeTextVerticalAnchor::Top,
            center_horizontal: false,
        }),
        "1" | "middle" | "center" | "ctr" | "mid" => Some(ShapeTextAnchor {
            vertical: StaticShapeTextVerticalAnchor::Middle,
            center_horizontal: false,
        }),
        "2" | "bottom" | "b" => Some(ShapeTextAnchor {
            vertical: StaticShapeTextVerticalAnchor::Bottom,
            center_horizontal: false,
        }),
        "3" | "topcentered" | "centertop" => Some(ShapeTextAnchor {
            vertical: StaticShapeTextVerticalAnchor::Top,
            center_horizontal: true,
        }),
        "4" | "middlecentered" | "centermiddle" | "centercentered" => Some(ShapeTextAnchor {
            vertical: StaticShapeTextVerticalAnchor::Middle,
            center_horizontal: true,
        }),
        "5" | "bottomcentered" | "centerbottom" => Some(ShapeTextAnchor {
            vertical: StaticShapeTextVerticalAnchor::Bottom,
            center_horizontal: true,
        }),
        "6" | "baseline" => Some(ShapeTextAnchor {
            vertical: StaticShapeTextVerticalAnchor::Top,
            center_horizontal: false,
        }),
        "7" | "bottombaseline" => Some(ShapeTextAnchor {
            vertical: StaticShapeTextVerticalAnchor::Bottom,
            center_horizontal: false,
        }),
        "8" | "centerbaseline" => Some(ShapeTextAnchor {
            vertical: StaticShapeTextVerticalAnchor::Top,
            center_horizontal: true,
        }),
        "9" | "centerbottombaseline" => Some(ShapeTextAnchor {
            vertical: StaticShapeTextVerticalAnchor::Bottom,
            center_horizontal: true,
        }),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn parse_shape_arrowhead_property(
    value: &str,
) -> Option<StaticShapeArrowhead> {
    let normalized = value
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    match normalized.as_str() {
        "0" | "1" | "none" | "noarrow" | "msoarrowheadnone" => Some(StaticShapeArrowhead::None),
        "3" | "open" | "arrow" | "msoarrowheadopen" => Some(StaticShapeArrowhead::Open),
        "2" | "triangle" | "msoarrowheadtriangle" => Some(StaticShapeArrowhead::Triangle),
        "4" | "diamond" | "msoarrowheaddiamond" => Some(StaticShapeArrowhead::Diamond),
        "5" | "oval" | "msoarrowheadoval" => Some(StaticShapeArrowhead::Oval),
        "stealth" | "msoarrowheadstealth" => Some(StaticShapeArrowhead::Stealth),
        "block" | "msoarrowheadblock" => Some(StaticShapeArrowhead::Block),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn parse_shape_arrowhead_width_property(value: &str) -> Option<i32> {
    let normalized = normalize_shape_enum_property(value);
    match normalized.as_str() {
        "0" | "narrow" | "small" | "msoarrowheadnarrow" => Some(70),
        "1" | "medium" | "msoarrowheadwidthmedium" => Some(100),
        "2" | "wide" | "large" | "msoarrowheadwide" => Some(135),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn parse_shape_arrowhead_length_property(value: &str) -> Option<i32> {
    let normalized = normalize_shape_enum_property(value);
    match normalized.as_str() {
        "0" | "short" | "small" | "msoarrowheadshort" => Some(75),
        "1" | "medium" | "msoarrowheadlengthmedium" => Some(100),
        "2" | "long" | "large" | "msoarrowheadlong" => Some(135),
        _ => None,
    }
}

fn normalize_shape_enum_property(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
