//! Bounded EMF parsing and passive vector-command recovery.

use super::super::*;
use super::wmf::*;

#[derive(Debug)]
pub(in crate::rtf::parser) struct ParsedEmfHeader {
    pub(in crate::rtf::parser) width_px: u32,
    pub(in crate::rtf::parser) height_px: u32,
    pub(in crate::rtf::parser) bounds_left: i32,
    pub(in crate::rtf::parser) bounds_top: i32,
    pub(in crate::rtf::parser) header_size: usize,
    pub(in crate::rtf::parser) declared_size: usize,
    pub(in crate::rtf::parser) declared_record_count: Option<usize>,
    pub(in crate::rtf::parser) frame_width_twips: Option<i32>,
    pub(in crate::rtf::parser) frame_height_twips: Option<i32>,
    pub(in crate::rtf::parser) has_valid_reference_device: bool,
}

#[derive(Debug)]
pub(in crate::rtf::parser) struct ParsedEmfVector {
    pub(in crate::rtf::parser) width_px: u32,
    pub(in crate::rtf::parser) height_px: u32,
    pub(in crate::rtf::parser) frame_width_twips: Option<i32>,
    pub(in crate::rtf::parser) frame_height_twips: Option<i32>,
    pub(in crate::rtf::parser) skipped_record_count: usize,
    pub(in crate::rtf::parser) has_valid_reference_device: bool,
    pub(in crate::rtf::parser) commands: Vec<StaticImageVectorCommand>,
}

pub(in crate::rtf::parser) fn emf_commands_are_hatch_fill_only(
    commands: &[StaticImageVectorCommand],
) -> bool {
    !commands.is_empty()
        && commands.iter().all(|command| {
            matches!(
                command,
                StaticImageVectorCommand::Path {
                    stroke_color: None,
                    fill_pattern,
                    fill_color: Some(_),
                    ..
                }
                    | StaticImageVectorCommand::Polygon {
                        stroke_color: None,
                        fill_pattern,
                        fill_color: Some(_),
                        ..
                    }
                    | StaticImageVectorCommand::Rectangle {
                        stroke_color: None,
                        fill_pattern,
                        fill_color: Some(_),
                        ..
                    }
                    | StaticImageVectorCommand::RoundedRectangle {
                        stroke_color: None,
                        fill_pattern,
                        fill_color: Some(_),
                        ..
                    }
                    | StaticImageVectorCommand::Ellipse {
                        stroke_color: None,
                        fill_pattern,
                        fill_color: Some(_),
                        ..
                    } if *fill_pattern != ShadingPattern::None
            )
        })
}

#[derive(Debug, Clone)]
struct ParsedEmfExtTextOut {
    x: f32,
    y: f32,
    text: String,
    opaque_bounds: Option<(f32, f32, f32, f32)>,
    clip_bounds: Option<(f32, f32, f32, f32)>,
}

#[derive(Debug)]
struct ParsedEmfArc {
    points: Vec<(f32, f32)>,
    end_position: (i32, i32),
}

#[derive(Debug)]
struct ParsedEmfPolyDraw {
    commands: Vec<StaticImageVectorCommand>,
    end_position: (i32, i32),
}

#[derive(Debug)]
struct EmfPathBuilder {
    collecting: bool,
    segments: Vec<EmfPathSegment>,
    figure_start: (i32, i32),
    current_position: (i32, i32),
}

#[derive(Debug)]
enum EmfPathSegment {
    MoveTo((f32, f32)),
    Polyline(Vec<(f32, f32)>),
    Bezier(Vec<(f32, f32)>),
}

#[derive(Debug, Copy, Clone)]
enum EmfObject {
    Pen {
        color: Option<Color>,
        width: i32,
        style: BorderStyle,
    },
    Brush {
        color: Option<Color>,
        pattern: ShadingPattern,
    },
    Font {
        height: i32,
        charset: Option<i32>,
    },
    Other,
}

#[derive(Debug, Copy, Clone)]
struct EmfDrawingState {
    stroke_color: Option<Color>,
    stroke_width: f32,
    stroke_width_logical: i32,
    stroke_style: BorderStyle,
    fill_color: Option<Color>,
    fill_pattern: ShadingPattern,
    fill_rule: StaticImageVectorFillRule,
    text_color: Option<Color>,
    background_color: Option<Color>,
    text_background_mode: WmfTextBackgroundMode,
    font_height: Option<i32>,
    font_charset: Option<i32>,
    text_horizontal_align: StaticImageTextHorizontalAlign,
    text_vertical_align: StaticImageTextVerticalAlign,
    text_character_extra: f32,
    text_word_extra: f32,
    arc_clockwise: bool,
    stroke_rop2: u16,
}

#[derive(Debug, Copy, Clone)]
struct EmfCoordinateState {
    window_origin_x: i32,
    window_origin_y: i32,
    window_extent_x: i32,
    window_extent_y: i32,
    viewport_origin_x: i32,
    viewport_origin_y: i32,
    viewport_extent_x: i32,
    viewport_extent_y: i32,
    world_transform: EmfWorldTransform,
}

#[derive(Debug, Copy, Clone)]
struct EmfWorldTransform {
    scale_x: f32,
    scale_y: f32,
    translate_x: f32,
    translate_y: f32,
}

impl Default for EmfWorldTransform {
    fn default() -> Self {
        Self {
            scale_x: 1.0,
            scale_y: 1.0,
            translate_x: 0.0,
            translate_y: 0.0,
        }
    }
}

impl EmfCoordinateState {
    fn from_header(header: &ParsedEmfHeader) -> Self {
        let width = i32::try_from(header.width_px.max(1)).unwrap_or(i32::MAX);
        let height = i32::try_from(header.height_px.max(1)).unwrap_or(i32::MAX);
        Self {
            window_origin_x: header.bounds_left,
            window_origin_y: header.bounds_top,
            window_extent_x: width,
            window_extent_y: height,
            viewport_origin_x: 0,
            viewport_origin_y: 0,
            viewport_extent_x: width,
            viewport_extent_y: height,
            world_transform: EmfWorldTransform::default(),
        }
    }

    fn set_window_extent(&mut self, width: i32, height: i32) -> Option<()> {
        if width == 0 || height == 0 {
            return None;
        }
        self.window_extent_x = width;
        self.window_extent_y = height;
        Some(())
    }

    fn set_window_origin(&mut self, x: i32, y: i32) {
        self.window_origin_x = x;
        self.window_origin_y = y;
    }

    fn set_viewport_extent(&mut self, width: i32, height: i32) -> Option<()> {
        if width == 0 || height == 0 {
            return None;
        }
        self.viewport_extent_x = width;
        self.viewport_extent_y = height;
        Some(())
    }

    fn set_viewport_origin(&mut self, x: i32, y: i32) {
        self.viewport_origin_x = x;
        self.viewport_origin_y = y;
    }

    fn scale_window_extent(
        &mut self,
        x_num: i32,
        x_denom: i32,
        y_num: i32,
        y_denom: i32,
    ) -> Option<()> {
        self.window_extent_x = scale_emf_extent(self.window_extent_x, x_num, x_denom)?;
        self.window_extent_y = scale_emf_extent(self.window_extent_y, y_num, y_denom)?;
        Some(())
    }

    fn scale_viewport_extent(
        &mut self,
        x_num: i32,
        x_denom: i32,
        y_num: i32,
        y_denom: i32,
    ) -> Option<()> {
        self.viewport_extent_x = scale_emf_extent(self.viewport_extent_x, x_num, x_denom)?;
        self.viewport_extent_y = scale_emf_extent(self.viewport_extent_y, y_num, y_denom)?;
        Some(())
    }

    fn set_world_transform(&mut self, transform: EmfWorldTransform) {
        self.world_transform = transform;
    }

    fn left_multiply_world_transform(&mut self, transform: EmfWorldTransform) -> Option<()> {
        self.world_transform = multiply_emf_world_transforms(transform, self.world_transform)?;
        Some(())
    }

    fn right_multiply_world_transform(&mut self, transform: EmfWorldTransform) -> Option<()> {
        self.world_transform = multiply_emf_world_transforms(self.world_transform, transform)?;
        Some(())
    }

    fn reset_world_transform(&mut self) {
        self.world_transform = EmfWorldTransform::default();
    }
}

#[derive(Debug, Copy, Clone)]
struct EmfSavedState {
    drawing: EmfDrawingState,
    coordinates: EmfCoordinateState,
    current_position: (i32, i32),
    clip_active: bool,
    clip_scope_command_start: Option<usize>,
}

#[derive(Debug, Copy, Clone)]
struct RestoredEmfSavedState {
    state: EmfSavedState,
    restore_count: usize,
}

impl Default for EmfDrawingState {
    fn default() -> Self {
        Self {
            stroke_color: Some(Color::default()),
            stroke_width: 1.0,
            stroke_width_logical: 1,
            stroke_style: BorderStyle::Single,
            fill_color: None,
            fill_pattern: ShadingPattern::None,
            fill_rule: StaticImageVectorFillRule::Alternate,
            text_color: Some(Color::default()),
            background_color: Some(Color {
                red: 255,
                green: 255,
                blue: 255,
            }),
            text_background_mode: WmfTextBackgroundMode::Transparent,
            font_height: None,
            font_charset: None,
            text_horizontal_align: StaticImageTextHorizontalAlign::Left,
            text_vertical_align: StaticImageTextVerticalAlign::Top,
            text_character_extra: 0.0,
            text_word_extra: 0.0,
            arc_clockwise: false,
            stroke_rop2: WMF_ROP2_COPYPEN,
        }
    }
}

impl EmfDrawingState {
    fn passive_stroke_color(self) -> Option<Color> {
        match self.stroke_rop2 {
            WMF_ROP2_BLACK => Some(Color::default()),
            WMF_ROP2_NOTCOPYPEN => self.stroke_color.map(inverted_color),
            WMF_ROP2_NOP => None,
            WMF_ROP2_COPYPEN => self.stroke_color,
            WMF_ROP2_WHITE => Some(white_color()),
            _ => None,
        }
    }

    fn passive_stroke_width(
        self,
        header: &ParsedEmfHeader,
        coordinates: &EmfCoordinateState,
    ) -> f32 {
        normalized_emf_stroke_width(self.stroke_width_logical, header, coordinates)
    }
}

impl EmfPathBuilder {
    fn new(current_position: (i32, i32)) -> Self {
        Self {
            collecting: true,
            segments: Vec::new(),
            figure_start: current_position,
            current_position,
        }
    }

    fn end(&mut self) -> Option<()> {
        if !self.collecting {
            return None;
        }
        self.collecting = false;
        Some(())
    }

    fn move_to(
        &mut self,
        point: (i32, i32),
        header: &ParsedEmfHeader,
        coordinates: &EmfCoordinateState,
    ) -> Option<()> {
        if !self.collecting {
            return None;
        }
        if !self.segments.is_empty() {
            if self.segments.len() >= MAX_PASSIVE_WMF_COMMANDS {
                return None;
            }
            self.segments
                .push(EmfPathSegment::MoveTo(normalized_emf_point(
                    point.0,
                    point.1,
                    header,
                    coordinates,
                )));
        }
        self.figure_start = point;
        self.current_position = point;
        Some(())
    }

    fn line_to(
        &mut self,
        point: (i32, i32),
        header: &ParsedEmfHeader,
        coordinates: &EmfCoordinateState,
    ) -> Option<()> {
        if !self.collecting {
            return None;
        }
        let start = normalized_emf_point(
            self.current_position.0,
            self.current_position.1,
            header,
            coordinates,
        );
        let end = normalized_emf_point(point.0, point.1, header, coordinates);
        if segment_is_visible(start, end) {
            if self.segments.len() >= MAX_PASSIVE_WMF_COMMANDS {
                return None;
            }
            self.segments
                .push(EmfPathSegment::Polyline(vec![start, end]));
        }
        self.current_position = point;
        Some(())
    }

    fn polyline_to(
        &mut self,
        raw_points: &[(i32, i32)],
        header: &ParsedEmfHeader,
        coordinates: &EmfCoordinateState,
    ) -> Option<()> {
        if !self.collecting {
            return None;
        }
        if raw_points.is_empty() {
            return Some(());
        }
        let mut points = Vec::with_capacity(raw_points.len().checked_add(1)?);
        points.push(normalized_emf_point(
            self.current_position.0,
            self.current_position.1,
            header,
            coordinates,
        ));
        for (x, y) in raw_points {
            points.push(normalized_emf_point(*x, *y, header, coordinates));
        }
        if points
            .windows(2)
            .any(|pair| segment_is_visible(pair[0], pair[1]))
        {
            if self.segments.len() >= MAX_PASSIVE_WMF_COMMANDS {
                return None;
            }
            self.segments.push(EmfPathSegment::Polyline(points));
        }
        self.current_position = *raw_points.last()?;
        Some(())
    }

    fn normalized_polyline_to(
        &mut self,
        points: Vec<(f32, f32)>,
        end_position: (i32, i32),
    ) -> Option<()> {
        if !self.collecting || points.len() < 2 {
            return None;
        }
        if points
            .windows(2)
            .any(|pair| segment_is_visible(pair[0], pair[1]))
        {
            if self.segments.len() >= MAX_PASSIVE_WMF_COMMANDS {
                return None;
            }
            self.segments.push(EmfPathSegment::Polyline(points));
        }
        self.current_position = end_position;
        Some(())
    }

    fn bezier_to(
        &mut self,
        raw_points: &[(i32, i32)],
        header: &ParsedEmfHeader,
        coordinates: &EmfCoordinateState,
    ) -> Option<()> {
        if !self.collecting || !polybezierto_point_count_is_valid(raw_points.len()) {
            return None;
        }
        let mut points = Vec::with_capacity(raw_points.len().checked_add(1)?);
        points.push(normalized_emf_point(
            self.current_position.0,
            self.current_position.1,
            header,
            coordinates,
        ));
        for (x, y) in raw_points {
            points.push(normalized_emf_point(*x, *y, header, coordinates));
        }
        if point_bounds_are_visible(&points) {
            if self.segments.len() >= MAX_PASSIVE_WMF_COMMANDS {
                return None;
            }
            self.segments.push(EmfPathSegment::Bezier(points));
        }
        self.current_position = *raw_points.last()?;
        Some(())
    }

    fn close_figure(
        &mut self,
        header: &ParsedEmfHeader,
        coordinates: &EmfCoordinateState,
    ) -> Option<()> {
        let figure_start = self.figure_start;
        self.line_to(figure_start, header, coordinates)
    }

    fn stroke_commands(
        self,
        stroke_color: Option<Color>,
        stroke_width: f32,
        stroke_style: BorderStyle,
    ) -> Option<Vec<StaticImageVectorCommand>> {
        if self.collecting {
            return None;
        }
        if self
            .segments
            .iter()
            .any(|segment| matches!(segment, EmfPathSegment::MoveTo(_)))
        {
            let (start, segments, closed) = self.path_segments()?;
            return Some(vec![StaticImageVectorCommand::Path {
                start,
                segments,
                closed,
                stroke_color,
                stroke_width,
                stroke_style,
                fill_rule: StaticImageVectorFillRule::Alternate,
                fill_pattern: ShadingPattern::None,
                fill_color: None,
            }]);
        }
        let mut commands = Vec::with_capacity(self.segments.len());
        for segment in self.segments {
            match segment {
                EmfPathSegment::MoveTo(_) => return None,
                EmfPathSegment::Polyline(points) => {
                    commands.push(StaticImageVectorCommand::Polyline {
                        points,
                        stroke_color,
                        stroke_width,
                        stroke_style,
                    });
                }
                EmfPathSegment::Bezier(points) => {
                    commands.push(StaticImageVectorCommand::Bezier {
                        points,
                        stroke_color,
                        stroke_width,
                        stroke_style,
                    });
                }
            }
        }
        Some(commands)
    }

    fn fill_command(
        self,
        stroke_color: Option<Color>,
        stroke_width: f32,
        stroke_style: BorderStyle,
        fill_rule: StaticImageVectorFillRule,
        fill_pattern: ShadingPattern,
        fill_color: Option<Color>,
    ) -> Option<StaticImageVectorCommand> {
        if self.collecting {
            return None;
        }
        if let Some(points) = self.line_only_points() {
            if points.len() < 3 || !point_bounds_are_visible(&points) {
                return None;
            }
            return Some(StaticImageVectorCommand::Polygon {
                points,
                stroke_color,
                stroke_width,
                stroke_style,
                fill_rule,
                fill_pattern,
                fill_color,
            });
        }

        if fill_color.is_none() && stroke_color.is_none() {
            return None;
        }
        let (start, segments, closed) = self.path_segments()?;
        Some(StaticImageVectorCommand::Path {
            start,
            segments,
            closed,
            stroke_color,
            stroke_width,
            stroke_style,
            fill_rule,
            fill_pattern,
            fill_color,
        })
    }

    fn clip_command(
        self,
        fill_rule: StaticImageVectorFillRule,
    ) -> Option<StaticImageVectorCommand> {
        if self.collecting {
            return None;
        }
        let (start, segments, closed) = self.path_segments()?;
        Some(StaticImageVectorCommand::ClipPath {
            start,
            segments,
            closed,
            fill_rule,
        })
    }

    fn is_passive_empty(&self) -> bool {
        !self.collecting && self.segments.is_empty()
    }

    fn line_only_points(&self) -> Option<Vec<(f32, f32)>> {
        let mut points = Vec::new();
        for segment in &self.segments {
            match segment {
                EmfPathSegment::MoveTo(_) => return None,
                EmfPathSegment::Polyline(segment_points) => {
                    if segment_points.len() < 2 {
                        return None;
                    }
                    for point in segment_points {
                        if points.last().is_some_and(|last| last == point) {
                            continue;
                        }
                        if points.len() >= MAX_PASSIVE_WMF_POINTS_PER_RECORD {
                            return None;
                        }
                        points.push(*point);
                    }
                }
                EmfPathSegment::Bezier(_) => return None,
            }
        }
        Some(points)
    }

    fn path_segments(&self) -> Option<((f32, f32), Vec<StaticImageVectorPathSegment>, bool)> {
        let mut iter = self.segments.iter();
        let first_segment = iter.next()?;
        let start = match first_segment {
            EmfPathSegment::MoveTo(point) => *point,
            EmfPathSegment::Polyline(points) => *points.first()?,
            EmfPathSegment::Bezier(points) => *points.first()?,
        };
        let mut current = start;
        let mut path_segments = Vec::new();
        let mut saw_visible_segment = false;
        for segment in std::iter::once(first_segment).chain(iter) {
            match segment {
                EmfPathSegment::MoveTo(point) => {
                    if path_segments.len() >= MAX_PASSIVE_WMF_POINTS_PER_RECORD {
                        return None;
                    }
                    path_segments.push(StaticImageVectorPathSegment::MoveTo(point.0, point.1));
                    current = *point;
                }
                EmfPathSegment::Polyline(points) => {
                    if points.len() < 2 {
                        return None;
                    }
                    if points.first().is_some_and(|point| *point != current) {
                        return None;
                    }
                    for point in points.iter().skip(1) {
                        if path_segments.len() >= MAX_PASSIVE_WMF_POINTS_PER_RECORD {
                            return None;
                        }
                        if *point != current {
                            saw_visible_segment |= segment_is_visible(current, *point);
                            path_segments
                                .push(StaticImageVectorPathSegment::LineTo(point.0, point.1));
                            current = *point;
                        }
                    }
                }
                EmfPathSegment::Bezier(points) => {
                    if points.len() < 4 || (points.len() - 1) % 3 != 0 {
                        return None;
                    }
                    if points.first().is_some_and(|point| *point != current) {
                        return None;
                    }
                    for chunk in points[1..].chunks_exact(3) {
                        if path_segments.len() >= MAX_PASSIVE_WMF_POINTS_PER_RECORD {
                            return None;
                        }
                        saw_visible_segment |=
                            point_bounds_are_visible(&[current, chunk[0], chunk[1], chunk[2]]);
                        path_segments.push(StaticImageVectorPathSegment::CubicTo {
                            control1: chunk[0],
                            control2: chunk[1],
                            end: chunk[2],
                        });
                        current = chunk[2];
                    }
                }
            }
        }
        if !saw_visible_segment {
            return None;
        }
        Some((start, path_segments, current == start))
    }
}

pub(in crate::rtf::parser) fn parse_emf_header_dimensions(bytes: &[u8]) -> Option<ParsedEmfHeader> {
    const EMR_HEADER_RECORD_TYPE: u32 = 1;
    const EMF_SIGNATURE: u32 = 0x464d_4520;
    const MIN_EMF_HEADER_BYTES: usize = 88;

    if bytes.len() < MIN_EMF_HEADER_BYTES {
        return None;
    }
    if read_le_u32(bytes, 0)? != EMR_HEADER_RECORD_TYPE {
        return None;
    }
    let header_size = read_le_u32(bytes, 4)? as usize;
    if header_size < MIN_EMF_HEADER_BYTES || header_size > bytes.len() {
        return None;
    }
    if read_le_u32(bytes, 40)? != EMF_SIGNATURE {
        return None;
    }
    let declared_size = read_le_u32(bytes, 48)? as usize;
    if declared_size < header_size || declared_size > bytes.len() {
        return None;
    }
    let declared_record_count = usize::try_from(read_le_u32(bytes, 52)?).ok();

    let bounds_left_raw = read_le_i32(bytes, 8)?;
    let bounds_top_raw = read_le_i32(bytes, 12)?;
    let bounds_right_raw = read_le_i32(bytes, 16)?;
    let bounds_bottom_raw = read_le_i32(bytes, 20)?;
    let bounds_width = signed_rect_extent(bounds_left_raw, bounds_right_raw);
    let bounds_height = signed_rect_extent(bounds_top_raw, bounds_bottom_raw);
    let frame_width_hundredth_mm =
        signed_rect_extent(read_le_i32(bytes, 24)?, read_le_i32(bytes, 32)?);
    let frame_height_hundredth_mm =
        signed_rect_extent(read_le_i32(bytes, 28)?, read_le_i32(bytes, 36)?);

    let frame_width_twips = hundredth_mm_to_twips(frame_width_hundredth_mm);
    let frame_height_twips = hundredth_mm_to_twips(frame_height_hundredth_mm);
    let has_valid_reference_device = read_le_i32(bytes, 72)? > 0
        && read_le_i32(bytes, 76)? > 0
        && read_le_i32(bytes, 80)? > 0
        && read_le_i32(bytes, 84)? > 0;
    let width_px = bounds_width.or_else(|| hundredth_mm_to_96dpi_px(frame_width_hundredth_mm))?;
    let height_px =
        bounds_height.or_else(|| hundredth_mm_to_96dpi_px(frame_height_hundredth_mm))?;

    Some(ParsedEmfHeader {
        width_px,
        height_px,
        bounds_left: bounds_left_raw.min(bounds_right_raw),
        bounds_top: bounds_top_raw.min(bounds_bottom_raw),
        header_size,
        declared_size,
        declared_record_count,
        frame_width_twips,
        frame_height_twips,
        has_valid_reference_device,
    })
}

pub(in crate::rtf::parser) fn parse_emf_vector_image_data_with_limits(
    bytes: &[u8],
    limits: VectorDecodeLimits,
) -> Option<ParsedEmfVector> {
    if limits.max_points_per_record < MAX_PASSIVE_WMF_POINTS_PER_RECORD
        || limits.max_raster_pixels < MAX_PASSIVE_VECTOR_RASTER_PIXELS
    {
        return None;
    }
    const EMR_EOF: u32 = 14;
    const EMR_SETWINDOWEXTEX: u32 = 9;
    const EMR_SETWINDOWORGEX: u32 = 10;
    const EMR_SETVIEWPORTEXTEX: u32 = 11;
    const EMR_SETVIEWPORTORGEX: u32 = 12;
    const EMR_SETBRUSHORGEX: u32 = 13;
    const EMR_SETMAPPERFLAGS: u32 = 16;
    const EMR_POLYBEZIER: u32 = 2;
    const EMR_POLYBEZIERTO: u32 = 5;
    const EMR_MOVETOEX: u32 = 27;
    const EMR_POLYGON: u32 = 3;
    const EMR_POLYLINE: u32 = 4;
    const EMR_POLYLINETO: u32 = 6;
    const EMR_POLYPOLYLINE: u32 = 7;
    const EMR_POLYPOLYGON: u32 = 8;
    const EMR_SETPIXELV: u32 = 15;
    const EMR_SETMAPMODE: u32 = 17;
    const EMR_SETBKMODE: u32 = 18;
    const EMR_SETPOLYFILLMODE: u32 = 19;
    const EMR_SETROP2: u32 = 20;
    const EMR_SETSTRETCHBLTMODE: u32 = 21;
    const EMR_SETTEXTALIGN: u32 = 22;
    const EMR_SETCOLORADJUSTMENT: u32 = 23;
    const EMR_SETTEXTCOLOR: u32 = 24;
    const EMR_SETBKCOLOR: u32 = 25;
    const EMR_OFFSETCLIPRGN: u32 = 26;
    const EMR_SETTEXTCHAREXTRA: u32 = 106;
    const EMR_SCALEVIEWPORTEXTEX: u32 = 31;
    const EMR_SCALEWINDOWEXTEX: u32 = 32;
    const EMR_EXCLUDECLIPRECT: u32 = 29;
    const EMR_INTERSECTCLIPRECT: u32 = 30;
    const EMR_EXTSELECTCLIPRGN: u32 = 75;
    const EMR_SETARCDIRECTION: u32 = 57;
    const EMR_SETTEXTJUSTIFICATION: u32 = 120;
    const EMR_SAVEDC: u32 = 33;
    const EMR_RESTOREDC: u32 = 34;
    const EMR_SETWORLDTRANSFORM: u32 = 35;
    const EMR_MODIFYWORLDTRANSFORM: u32 = 36;
    const EMR_SELECTOBJECT: u32 = 37;
    const EMR_CREATEPEN: u32 = 38;
    const EMR_CREATEBRUSHINDIRECT: u32 = 39;
    const EMR_DELETEOBJECT: u32 = 40;
    const EMR_SELECTPALETTE: u32 = 48;
    const EMR_CREATEPALETTE: u32 = 49;
    const EMR_SETPALETTEENTRIES: u32 = 50;
    const EMR_RESIZEPALETTE: u32 = 51;
    const EMR_REALIZEPALETTE: u32 = 52;
    const EMR_ANGLEARC: u32 = 41;
    const EMR_ARC: u32 = 45;
    const EMR_CHORD: u32 = 46;
    const EMR_PIE: u32 = 47;
    const EMR_RECTANGLE: u32 = 43;
    const EMR_ELLIPSE: u32 = 42;
    const EMR_ROUNDRECT: u32 = 44;
    const EMR_LINETO: u32 = 54;
    const EMR_ARCTO: u32 = 55;
    const EMR_POLYDRAW: u32 = 56;
    const EMR_BEGINPATH: u32 = 59;
    const EMR_ENDPATH: u32 = 60;
    const EMR_SETMITERLIMIT: u32 = 58;
    const EMR_CLOSEFIGURE: u32 = 61;
    const EMR_FILLPATH: u32 = 62;
    const EMR_STROKEANDFILLPATH: u32 = 63;
    const EMR_STROKEPATH: u32 = 64;
    const EMR_SELECTCLIPPATH: u32 = 67;
    const EMR_ABORTPATH: u32 = 68;
    const EMR_COMMENT: u32 = 70;
    const EMR_FILLRGN: u32 = 71;
    const EMR_FRAMERGN: u32 = 72;
    const EMR_INVERTRGN: u32 = 73;
    const EMR_PAINTRGN: u32 = 74;
    const EMR_BITBLT: u32 = 76;
    const EMR_STRETCHBLT: u32 = 77;
    const EMR_SETDIBITSTODEVICE: u32 = 80;
    const EMR_STRETCHDIBITS: u32 = 81;
    const EMR_EXTTEXTOUTA: u32 = 83;
    const EMR_EXTTEXTOUTW: u32 = 84;
    const EMR_POLYTEXTOUTA: u32 = 96;
    const EMR_POLYTEXTOUTW: u32 = 97;
    const EMR_POLYBEZIER16: u32 = 85;
    const EMR_POLYGON16: u32 = 86;
    const EMR_POLYLINE16: u32 = 87;
    const EMR_POLYBEZIERTO16: u32 = 88;
    const EMR_POLYLINETO16: u32 = 89;
    const EMR_POLYPOLYLINE16: u32 = 90;
    const EMR_POLYPOLYGON16: u32 = 91;
    const EMR_POLYDRAW16: u32 = 92;
    const EMR_EXTCREATEPEN: u32 = 95;
    const EMR_EXTCREATEFONTINDIRECTW: u32 = 82;
    const EMR_SETICMMODE: u32 = 98;
    const EMR_ALPHABLEND: u32 = 114;
    const EMR_SETLAYOUT: u32 = 115;
    const EMR_TRANSPARENTBLT: u32 = 116;

    let header = parse_emf_header_dimensions(bytes)?;
    if header
        .declared_record_count
        .is_some_and(|count| count == 0 || count > MAX_PASSIVE_WMF_RECORDS)
    {
        return None;
    }
    let mut pos = header.header_size;
    let mut record_count = 1usize;
    let mut commands = Vec::new();
    let mut skipped_record_count = 0usize;
    let mut reached_eof = false;
    let mut coordinates = EmfCoordinateState::from_header(&header);
    let mut current_position = (0i32, 0i32);
    let mut objects: Vec<Option<EmfObject>> = Vec::new();
    let mut state = EmfDrawingState::default();
    let mut saved_states: Vec<EmfSavedState> = Vec::new();
    let mut active_path: Option<EmfPathBuilder> = None;
    let mut clip_active = false;
    let mut replaceable_clip_command_start: Option<usize> = None;
    let mut clip_scope_command_start: Option<usize> = None;

    while pos + 8 <= header.declared_size {
        if header
            .declared_record_count
            .is_some_and(|limit| record_count >= limit)
        {
            break;
        }
        record_count = record_count.checked_add(1)?;
        if record_count > MAX_PASSIVE_WMF_RECORDS {
            return None;
        }
        let record_type = read_le_u32(bytes, pos)?;
        let record_size = read_le_u32(bytes, pos + 4)? as usize;
        if record_size < 8 || record_size % 4 != 0 {
            return None;
        }
        let record_end = pos.checked_add(record_size)?;
        if record_end > header.declared_size {
            return None;
        }
        let data = &bytes[pos + 8..record_end];

        match record_type {
            EMR_EOF => {
                reached_eof = true;
                break;
            }
            EMR_SETWINDOWEXTEX => match parse_emf_size_record(data) {
                Some((width, height)) if width != 0 && height != 0 => {
                    if coordinates.set_window_extent(width, height).is_none() {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                    }
                }
                Some(_) | None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            EMR_SETWINDOWORGEX => match parse_emf_raw_point_record(data) {
                Some((x, y)) => coordinates.set_window_origin(x, y),
                None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            EMR_SETVIEWPORTEXTEX => match parse_emf_size_record(data) {
                Some((width, height)) if width != 0 && height != 0 => {
                    if coordinates.set_viewport_extent(width, height).is_none() {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                    }
                }
                Some(_) | None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            EMR_SETVIEWPORTORGEX => match parse_emf_raw_point_record(data) {
                Some((x, y)) => coordinates.set_viewport_origin(x, y),
                None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            EMR_SETBRUSHORGEX => {
                if parse_emf_raw_point_record(data).is_none() {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_SETMAPPERFLAGS => {
                if read_le_u32(data, 0).is_none() {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_SCALEVIEWPORTEXTEX => match parse_emf_scale_record(data) {
                Some((x_num, x_denom, y_num, y_denom)) if x_num != 0 && y_num != 0 => {
                    if coordinates
                        .scale_viewport_extent(x_num, x_denom, y_num, y_denom)
                        .is_none()
                    {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                    }
                }
                Some(_) | None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            EMR_SCALEWINDOWEXTEX => match parse_emf_scale_record(data) {
                Some((x_num, x_denom, y_num, y_denom)) if x_num != 0 && y_num != 0 => {
                    if coordinates
                        .scale_window_extent(x_num, x_denom, y_num, y_denom)
                        .is_none()
                    {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                    }
                }
                Some(_) | None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            EMR_OFFSETCLIPRGN => {
                if is_passive_zero_emf_offset_clip_region(data) {
                    pos = record_end;
                    continue;
                }
                let Some((offset_x, offset_y)) =
                    parse_emf_offset_clip_region(data, &header, &coordinates)
                else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if let Some(clip_start) = replaceable_clip_command_start
                    && vector_commands_are_only_clip_updates(&commands[clip_start..])
                {
                    for command in &mut commands[clip_start..] {
                        offset_vector_clip_command(command, offset_x, offset_y)?;
                    }
                } else if let Some(scope_start) = clip_scope_command_start {
                    let mut clip_commands = scoped_vector_clip_commands(&commands, scope_start)?;
                    if commands
                        .len()
                        .checked_add(2)?
                        .checked_add(clip_commands.len())?
                        > MAX_PASSIVE_WMF_COMMANDS
                    {
                        return None;
                    }
                    for command in &mut clip_commands {
                        offset_vector_clip_command(command, offset_x, offset_y)?;
                    }
                    commands.push(StaticImageVectorCommand::RestoreState);
                    commands.push(StaticImageVectorCommand::SaveState);
                    clip_scope_command_start = commands.len().checked_sub(1);
                    replaceable_clip_command_start = Some(commands.len());
                    commands.extend(clip_commands);
                } else if let Some(clip_start) = replaceable_clip_command_start {
                    let mut clip_commands =
                        offsettable_clip_commands_after_paint(&commands[clip_start..])?;
                    if commands
                        .len()
                        .checked_add(3)?
                        .checked_add(clip_commands.len())?
                        > MAX_PASSIVE_WMF_COMMANDS
                    {
                        return None;
                    }
                    for command in &mut clip_commands {
                        offset_vector_clip_command(command, offset_x, offset_y)?;
                    }
                    commands.insert(clip_start, StaticImageVectorCommand::SaveState);
                    commands.push(StaticImageVectorCommand::RestoreState);
                    commands.push(StaticImageVectorCommand::SaveState);
                    clip_scope_command_start = commands.len().checked_sub(1);
                    replaceable_clip_command_start = Some(commands.len());
                    commands.extend(clip_commands);
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_EXCLUDECLIPRECT => {
                let Some((left, top, right, bottom)) =
                    parse_emf_record_rect_including_degenerate(data, &header, &coordinates)
                else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if !bounds_is_visible((left, top, right, bottom)) {
                    pos = record_end;
                    continue;
                }
                if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                    return None;
                }
                if let Some(clip_start) = replaceable_clip_command_start {
                    if let Some(command) = vector_diff_from_unpainted_clip_rect_command(
                        &commands,
                        clip_start,
                        vec![(left, top, right, bottom)],
                    )? {
                        commands.truncate(clip_start);
                        commands.push(command);
                    }
                } else {
                    commands.push(emf_exclude_clip_rect_command(
                        &header, left, top, right, bottom,
                    )?);
                }
                if replaceable_clip_command_start.is_none() {
                    replaceable_clip_command_start = commands.len().checked_sub(1);
                }
                clip_active = true;
            }
            EMR_INTERSECTCLIPRECT => {
                let Some((left, top, right, bottom)) =
                    parse_emf_record_rect_including_degenerate(data, &header, &coordinates)
                else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                    return None;
                }
                commands.push(StaticImageVectorCommand::ClipRect {
                    left,
                    top,
                    right,
                    bottom,
                });
                if replaceable_clip_command_start.is_none() {
                    replaceable_clip_command_start = commands.len().checked_sub(1);
                }
                clip_active = true;
            }
            EMR_EXTSELECTCLIPRGN => {
                let Some(region_mode) = read_le_u32(data, 4) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if is_passive_empty_emf_clip_combine_noop(region_mode)
                    && is_passive_empty_emf_region(data, 0, 8)
                {
                    pos = record_end;
                    continue;
                }
                if matches!(region_mode, EMF_RGN_AND | EMF_RGN_COPY)
                    && is_passive_empty_emf_region(data, 0, 8)
                {
                    if clip_active {
                        replace_emf_clip_scope(
                            &mut commands,
                            &mut replaceable_clip_command_start,
                            &mut clip_scope_command_start,
                        )?;
                    }
                    if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                        return None;
                    }
                    commands.push(StaticImageVectorCommand::ClipRect {
                        left: 0.0,
                        top: 0.0,
                        right: 0.0,
                        bottom: 0.0,
                    });
                    replaceable_clip_command_start = commands.len().checked_sub(1);
                    clip_active = true;
                    pos = record_end;
                    continue;
                }
                if region_mode == EMF_RGN_OR && !clip_active {
                    if parse_emf_region_data_rects(data, 0, 8, &header, &coordinates).is_none() {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                    }
                    pos = record_end;
                    continue;
                }
                if region_mode == EMF_RGN_OR && clip_active {
                    let Some(rects) =
                        parse_emf_region_data_rects(data, 0, 8, &header, &coordinates)
                    else {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                        pos = record_end;
                        continue;
                    };
                    if let Some(clip_start) = replaceable_clip_command_start {
                        if let Some(command) = vector_region_or_unpainted_clip_rect_command(
                            &commands, clip_start, &rects,
                        )? {
                            commands.truncate(clip_start);
                            commands.push(command);
                        }
                        pos = record_end;
                        continue;
                    }
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                }
                if region_mode == EMF_RGN_DIFF && !clip_active {
                    let Some(rects) =
                        parse_emf_region_data_rects(data, 0, 8, &header, &coordinates)
                    else {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                        pos = record_end;
                        continue;
                    };
                    let command = emf_exclude_clip_region_command(&header, rects)?;
                    if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                        return None;
                    }
                    commands.push(command);
                    if replaceable_clip_command_start.is_none() {
                        replaceable_clip_command_start = commands.len().checked_sub(1);
                    }
                    clip_active = true;
                    pos = record_end;
                    continue;
                }
                if region_mode == EMF_RGN_DIFF && clip_active {
                    let Some(rects) =
                        parse_emf_region_data_rects(data, 0, 8, &header, &coordinates)
                    else {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                        pos = record_end;
                        continue;
                    };
                    if let Some(clip_start) = replaceable_clip_command_start {
                        if let Some(command) = vector_diff_from_unpainted_clip_rect_command(
                            &commands, clip_start, rects,
                        )? {
                            commands.truncate(clip_start);
                            commands.push(command);
                        }
                        pos = record_end;
                        continue;
                    }
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                }
                if region_mode == EMF_RGN_XOR && !clip_active {
                    let Some(rects) =
                        parse_emf_region_data_rects(data, 0, 8, &header, &coordinates)
                    else {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                        pos = record_end;
                        continue;
                    };
                    let command = emf_exclude_clip_region_command(&header, rects)?;
                    if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                        return None;
                    }
                    commands.push(command);
                    if replaceable_clip_command_start.is_none() {
                        replaceable_clip_command_start = commands.len().checked_sub(1);
                    }
                    clip_active = true;
                    pos = record_end;
                    continue;
                }
                if region_mode == EMF_RGN_XOR && clip_active {
                    let Some(rects) =
                        parse_emf_region_data_rects(data, 0, 8, &header, &coordinates)
                    else {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                        pos = record_end;
                        continue;
                    };
                    if let Some(clip_start) = replaceable_clip_command_start {
                        if let Some(command) = vector_xor_inside_unpainted_clip_rect_command(
                            &commands, clip_start, rects,
                        )? {
                            commands.truncate(clip_start);
                            commands.push(command);
                        }
                        pos = record_end;
                        continue;
                    }
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                }
                if region_mode == EMF_RGN_COPY && clip_active {
                    replace_emf_clip_scope(
                        &mut commands,
                        &mut replaceable_clip_command_start,
                        &mut clip_scope_command_start,
                    )?;
                } else if region_mode != EMF_RGN_AND && region_mode != EMF_RGN_COPY {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                }
                let Some(rects) = parse_emf_region_data_rects(data, 0, 8, &header, &coordinates)
                else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                let command = emf_region_clip_path_command(rects)?;
                if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                    return None;
                }
                commands.push(command);
                if replaceable_clip_command_start.is_none() {
                    replaceable_clip_command_start = commands.len().checked_sub(1);
                }
                clip_active = true;
            }
            EMR_MOVETOEX => match parse_emf_raw_point_record(data) {
                Some(point) => {
                    current_position = point;
                    if let Some(path) = active_path.as_mut() {
                        path.move_to(current_position, &header, &coordinates)?;
                    }
                }
                None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            EMR_SETMAPMODE => match read_le_u32(data, 0) {
                Some(value)
                    if value == u32::from(WMF_MAPMODE_TEXT)
                        || value == u32::from(WMF_MAPMODE_ISOTROPIC)
                        || value == u32::from(WMF_MAPMODE_ANISOTROPIC) => {}
                Some(_) | None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            EMR_SETBKMODE => match read_le_u32(data, 0) {
                Some(value) if value == u32::from(WMF_BKMODE_TRANSPARENT) => {
                    state.text_background_mode = WmfTextBackgroundMode::Transparent
                }
                Some(value) if value == u32::from(WMF_BKMODE_OPAQUE) => {
                    state.text_background_mode = WmfTextBackgroundMode::Opaque
                }
                Some(_) | None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            EMR_SETPOLYFILLMODE => match read_le_u32(data, 0) {
                Some(value) if value == u32::from(WMF_POLYFILLMODE_ALTERNATE) => {
                    state.fill_rule = StaticImageVectorFillRule::Alternate
                }
                Some(value) if value == u32::from(WMF_POLYFILLMODE_WINDING) => {
                    state.fill_rule = StaticImageVectorFillRule::Winding
                }
                Some(_) | None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            EMR_SETROP2 => match read_le_u32(data, 0) {
                Some(mode)
                    if matches!(
                        u16::try_from(mode).ok(),
                        Some(
                            WMF_ROP2_BLACK
                                | WMF_ROP2_NOTCOPYPEN
                                | WMF_ROP2_NOP
                                | WMF_ROP2_COPYPEN
                                | WMF_ROP2_WHITE
                        )
                    ) =>
                {
                    state.stroke_rop2 = u16::try_from(mode).ok()?;
                }
                Some(_) | None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            EMR_SETSTRETCHBLTMODE => match read_le_u32(data, 0) {
                Some(value)
                    if matches!(
                        u16::try_from(value).ok(),
                        Some(
                            WMF_STRETCHMODE_BLACKONWHITE
                                | WMF_STRETCHMODE_WHITEONBLACK
                                | WMF_STRETCHMODE_COLORONCOLOR
                                | WMF_STRETCHMODE_HALFTONE
                        )
                    ) => {}
                Some(_) | None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            EMR_SETARCDIRECTION => match parse_emf_arc_direction(data) {
                Some(Some(clockwise)) => state.arc_clockwise = clockwise,
                Some(None) | None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            EMR_SETMITERLIMIT => {
                let Some(limit) = parse_passive_emf_miter_limit(data) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if limit != 10.0 {
                    if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                        return None;
                    }
                    commands.push(StaticImageVectorCommand::SetMiterLimit { limit });
                }
            }
            EMR_SETTEXTALIGN => {
                if let Some(mode) = read_le_u32(data, 0) {
                    if let Some(mode) = supported_wmf_text_align_mode(mode) {
                        state.text_horizontal_align = wmf_text_horizontal_align(mode);
                        state.text_vertical_align = wmf_text_vertical_align(mode);
                    } else {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                    }
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_SETCOLORADJUSTMENT => {
                if !matches!(is_neutral_emf_color_adjustment(data), Some(true)) {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_SETTEXTCOLOR => match color_from_colorref(data, 0) {
                Some(color) => state.text_color = Some(color),
                None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            EMR_SETBKCOLOR => match color_from_colorref(data, 0) {
                Some(color) => state.background_color = Some(color),
                None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            EMR_SETTEXTCHAREXTRA => {
                match normalized_emf_text_character_extra(data, &header, &coordinates) {
                    Some(extra) => state.text_character_extra = extra,
                    None => skipped_record_count = skipped_record_count.checked_add(1)?,
                }
            }
            EMR_SETTEXTJUSTIFICATION => {
                match parse_emf_text_justification(data, &header, &coordinates) {
                    Some(Some(extra)) => state.text_word_extra = extra,
                    Some(None) | None => {
                        skipped_record_count = skipped_record_count.checked_add(1)?
                    }
                }
            }
            EMR_SETLAYOUT => {
                let Some(layout) = read_le_u32(data, 0) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if !emf_layout_is_supported_passive_noop(layout) {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_SAVEDC => {
                if saved_states.len() >= MAX_PASSIVE_WMF_OBJECTS {
                    return None;
                }
                if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                    return None;
                }
                saved_states.push(EmfSavedState {
                    drawing: state,
                    coordinates,
                    current_position,
                    clip_active,
                    clip_scope_command_start,
                });
                commands.push(StaticImageVectorCommand::SaveState);
                clip_scope_command_start = commands.len().checked_sub(1);
            }
            EMR_RESTOREDC => {
                if data.len() < 4 {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                }
                if saved_states.is_empty() {
                    pos = record_end;
                    continue;
                }
                let Some(restored) = restore_emf_saved_state(data, &mut saved_states) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if commands.len().checked_add(restored.restore_count)? > MAX_PASSIVE_WMF_COMMANDS {
                    return None;
                }
                state = restored.state.drawing;
                coordinates = restored.state.coordinates;
                current_position = restored.state.current_position;
                clip_active = restored.state.clip_active;
                clip_scope_command_start = restored.state.clip_scope_command_start;
                replaceable_clip_command_start = None;
                for _ in 0..restored.restore_count {
                    commands.push(StaticImageVectorCommand::RestoreState);
                }
            }
            EMR_SETWORLDTRANSFORM => match parse_passive_emf_world_transform(data, 0) {
                Some(Some(transform)) => coordinates.set_world_transform(transform),
                Some(None) | None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            EMR_MODIFYWORLDTRANSFORM => match read_le_u32(data, 24) {
                Some(1) => coordinates.reset_world_transform(),
                Some(2) => {
                    if let Some(transform) = parse_passive_emf_world_transform(data, 0).flatten()
                        && coordinates
                            .left_multiply_world_transform(transform)
                            .is_some()
                    {
                    } else {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                    }
                }
                Some(3) => {
                    if let Some(transform) = parse_passive_emf_world_transform(data, 0).flatten()
                        && coordinates
                            .right_multiply_world_transform(transform)
                            .is_some()
                    {
                    } else {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                    }
                }
                Some(4) => {
                    if let Some(transform) = parse_passive_emf_world_transform(data, 0).flatten() {
                        coordinates.set_world_transform(transform);
                    } else {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                    }
                }
                Some(_) | None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            EMR_CREATEPEN => {
                let Some((handle, object)) = parse_emf_pen_object(data) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if store_emf_object(&mut objects, handle, object).is_none() {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_EXTCREATEPEN => {
                let Some((handle, object)) = parse_emf_ext_pen_object(data) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if store_emf_object(&mut objects, handle, object).is_none() {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_EXTCREATEFONTINDIRECTW => {
                let Some((handle, object)) = parse_emf_ext_font_object(data) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if store_emf_object(&mut objects, handle, object).is_none() {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_CREATEBRUSHINDIRECT => {
                let Some((handle, object)) = parse_emf_brush_object(data) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if store_emf_object(&mut objects, handle, object).is_none() {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_CREATEPALETTE => {
                let Some(handle) = parse_emf_palette_object(data) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if store_emf_object(&mut objects, handle, EmfObject::Other).is_none() {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_DELETEOBJECT => {
                let Some(handle) =
                    read_le_u32(data, 0).and_then(|value| usize::try_from(value).ok())
                else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if let Some(slot) = objects.get_mut(handle) {
                    *slot = None;
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_SELECTPALETTE => {
                if read_le_u32(data, 0).is_none() {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_SETPALETTEENTRIES => {
                if validate_emf_set_palette_entries(data).is_none() {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_RESIZEPALETTE => {
                if read_le_u32(data, 0).is_none() || read_le_u32(data, 4).is_none() {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_REALIZEPALETTE => {}
            EMR_SELECTOBJECT => {
                let Some(handle) = read_le_u32(data, 0) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if let Some(object) = emf_stock_object(handle).or_else(|| {
                    usize::try_from(handle)
                        .ok()
                        .and_then(|idx| objects.get(idx).copied().flatten())
                }) {
                    if matches!(object, EmfObject::Other) {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                    } else {
                        apply_emf_object(&mut state, object, &header, &coordinates);
                    }
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_LINETO => match parse_emf_raw_point_record(data) {
                Some(endpoint) => {
                    if let Some(path) = active_path.as_mut() {
                        path.line_to(endpoint, &header, &coordinates)?;
                    } else {
                        let start = normalized_emf_point(
                            current_position.0,
                            current_position.1,
                            &header,
                            &coordinates,
                        );
                        let endpoint_normalized =
                            normalized_emf_point(endpoint.0, endpoint.1, &header, &coordinates);
                        if segment_is_visible(start, endpoint_normalized) {
                            if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                                return None;
                            }
                            commands.push(StaticImageVectorCommand::Line {
                                x1: start.0,
                                y1: start.1,
                                x2: endpoint_normalized.0,
                                y2: endpoint_normalized.1,
                                stroke_color: state.passive_stroke_color(),
                                stroke_width: state.passive_stroke_width(&header, &coordinates),
                                stroke_style: state.stroke_style,
                            });
                        }
                    }
                    current_position = endpoint;
                }
                None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            EMR_BEGINPATH => {
                if active_path.is_some() {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                }
                active_path = Some(EmfPathBuilder::new(current_position));
            }
            EMR_ENDPATH => {
                let Some(path) = active_path.as_mut() else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                path.end()?;
            }
            EMR_CLOSEFIGURE => {
                let Some(path) = active_path.as_mut() else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                path.close_figure(&header, &coordinates)?;
                current_position = path.current_position;
            }
            EMR_STROKEPATH => {
                let Some(path) = active_path.take() else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if path.is_passive_empty() {
                    pos = record_end;
                    continue;
                }
                let stroke_color = state.passive_stroke_color();
                if stroke_color.is_none() {
                    pos = record_end;
                    continue;
                }
                let path_commands = path.stroke_commands(
                    stroke_color,
                    state.passive_stroke_width(&header, &coordinates),
                    state.stroke_style,
                )?;
                if commands.len().checked_add(path_commands.len())? > MAX_PASSIVE_WMF_COMMANDS {
                    return None;
                }
                commands.extend(path_commands);
            }
            EMR_FILLPATH | EMR_STROKEANDFILLPATH => {
                if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                    return None;
                }
                let Some(path) = active_path.take() else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if path.is_passive_empty() {
                    pos = record_end;
                    continue;
                }
                if record_type == EMR_FILLPATH && state.fill_color.is_none() {
                    pos = record_end;
                    continue;
                }
                if record_type == EMR_STROKEANDFILLPATH
                    && state.passive_stroke_color().is_none()
                    && state.fill_color.is_none()
                {
                    pos = record_end;
                    continue;
                }
                let (stroke_color, stroke_width) = if record_type == EMR_STROKEANDFILLPATH {
                    (
                        state.passive_stroke_color(),
                        state.passive_stroke_width(&header, &coordinates),
                    )
                } else {
                    (None, 0.0)
                };
                let command = path.fill_command(
                    stroke_color,
                    stroke_width,
                    state.stroke_style,
                    state.fill_rule,
                    state.fill_pattern,
                    state.fill_color,
                )?;
                commands.push(command);
            }
            EMR_ABORTPATH => {
                active_path = None;
            }
            EMR_COMMENT => {
                if parse_emf_comment_record(data).is_none() {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_FILLRGN => {
                // `data` starts after the EMF record Type and Size fields.
                if is_passive_empty_emf_region(data, 16, 24) {
                    pos = record_end;
                    continue;
                }
                let Some(handle) = read_le_u32(data, 20) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                let Some(EmfObject::Brush { color, pattern }) =
                    emf_stock_object(handle).or_else(|| {
                        usize::try_from(handle)
                            .ok()
                            .and_then(|idx| objects.get(idx).copied().flatten())
                    })
                else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                let Some(fill_color) = color else {
                    pos = record_end;
                    continue;
                };
                let Some(rects) = parse_emf_region_data_rects(data, 16, 24, &header, &coordinates)
                else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                push_emf_region_rectangles(&mut commands, rects, fill_color, pattern)?;
            }
            EMR_FRAMERGN => {
                if is_passive_noop_emf_framergn(data) || is_passive_empty_emf_region(data, 16, 32) {
                    pos = record_end;
                    continue;
                }
                let Some(handle) = read_le_u32(data, 20) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                let Some(EmfObject::Brush { color, pattern }) =
                    emf_stock_object(handle).or_else(|| {
                        usize::try_from(handle)
                            .ok()
                            .and_then(|idx| objects.get(idx).copied().flatten())
                    })
                else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                let Some(fill_color) = color else {
                    pos = record_end;
                    continue;
                };
                let Some(stroke_width) = read_le_i32(data, 24)
                    .and_then(|width| normalized_emf_extent_width(width, &header, &coordinates))
                else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                let Some(stroke_height) = read_le_i32(data, 28)
                    .and_then(|height| normalized_emf_extent_height(height, &header, &coordinates))
                else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                let Some(rects) = parse_emf_region_data_rects(data, 16, 32, &header, &coordinates)
                else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                push_emf_region_frame_rectangles(
                    &mut commands,
                    rects,
                    stroke_width,
                    stroke_height,
                    fill_color,
                    pattern,
                )?;
            }
            EMR_INVERTRGN => {
                if is_passive_empty_emf_region(data, 16, 20) {
                    pos = record_end;
                    continue;
                }
                let Some(rects) = parse_emf_region_data_rects(data, 16, 20, &header, &coordinates)
                else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if emf_commands_are_unpainted_clip_scope(&commands) {
                    push_emf_region_rectangles(
                        &mut commands,
                        rects,
                        Color {
                            red: 0,
                            green: 0,
                            blue: 0,
                        },
                        ShadingPattern::None,
                    )?;
                } else if apply_passive_same_bounds_invert_fills(&mut commands, &rects) {
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_PAINTRGN => {
                if is_passive_empty_emf_region(data, 16, 20) {
                    pos = record_end;
                    continue;
                }
                let Some(rects) = parse_emf_region_data_rects(data, 16, 20, &header, &coordinates)
                else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                let Some(fill_color) = state.fill_color else {
                    pos = record_end;
                    continue;
                };
                push_emf_region_rectangles(&mut commands, rects, fill_color, state.fill_pattern)?;
            }
            EMR_SELECTCLIPPATH => {
                let Some(region_mode) = read_le_u32(data, 0) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if region_mode == EMF_RGN_OR && !clip_active {
                    if active_path.take().is_none() {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                    }
                    pos = record_end;
                    continue;
                }
                if region_mode == EMF_RGN_OR && clip_active {
                    let Some(path) = active_path.take() else {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                        pos = record_end;
                        continue;
                    };
                    if let Some(clip_start) = replaceable_clip_command_start {
                        if let Some(command) = vector_path_or_unpainted_clip_rect_command(
                            &commands, clip_start, &path,
                        )? {
                            commands.truncate(clip_start);
                            commands.push(command);
                        }
                        pos = record_end;
                        continue;
                    }
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                }
                if region_mode == EMF_RGN_DIFF
                    && !clip_active
                    && state.fill_rule == StaticImageVectorFillRule::Alternate
                {
                    let Some(path) = active_path.take() else {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                        pos = record_end;
                        continue;
                    };
                    let command = emf_exclude_clip_path_command(&header, path)?;
                    if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                        return None;
                    }
                    commands.push(command);
                    if replaceable_clip_command_start.is_none() {
                        replaceable_clip_command_start = commands.len().checked_sub(1);
                    }
                    clip_active = true;
                    pos = record_end;
                    continue;
                }
                if region_mode == EMF_RGN_DIFF
                    && clip_active
                    && state.fill_rule == StaticImageVectorFillRule::Alternate
                {
                    let Some(path) = active_path.take() else {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                        pos = record_end;
                        continue;
                    };
                    if let Some(clip_start) = replaceable_clip_command_start {
                        if let Some(command) = vector_diff_path_from_unpainted_clip_rect_command(
                            &commands, clip_start, path,
                        )? {
                            commands.truncate(clip_start);
                            commands.push(command);
                        }
                        pos = record_end;
                        continue;
                    }
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                }
                if region_mode == EMF_RGN_XOR
                    && !clip_active
                    && state.fill_rule == StaticImageVectorFillRule::Alternate
                {
                    let Some(path) = active_path.take() else {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                        pos = record_end;
                        continue;
                    };
                    let command = emf_exclude_clip_path_command(&header, path)?;
                    if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                        return None;
                    }
                    commands.push(command);
                    if replaceable_clip_command_start.is_none() {
                        replaceable_clip_command_start = commands.len().checked_sub(1);
                    }
                    clip_active = true;
                    pos = record_end;
                    continue;
                }
                if region_mode == EMF_RGN_XOR
                    && clip_active
                    && state.fill_rule == StaticImageVectorFillRule::Alternate
                {
                    let Some(path) = active_path.take() else {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                        pos = record_end;
                        continue;
                    };
                    if let Some(clip_start) = replaceable_clip_command_start {
                        if let Some(command) = vector_diff_path_from_unpainted_clip_rect_command(
                            &commands, clip_start, path,
                        )? {
                            commands.truncate(clip_start);
                            commands.push(command);
                        }
                        pos = record_end;
                        continue;
                    }
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                }
                if region_mode == EMF_RGN_COPY && clip_active {
                    replace_emf_clip_scope(
                        &mut commands,
                        &mut replaceable_clip_command_start,
                        &mut clip_scope_command_start,
                    )?;
                } else if matches!(region_mode, EMF_RGN_OR | EMF_RGN_DIFF | EMF_RGN_XOR) {
                    return None;
                } else if region_mode != EMF_RGN_AND && region_mode != EMF_RGN_COPY {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                }
                let Some(path) = active_path.take() else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                let command = path.clip_command(state.fill_rule)?;
                if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                    return None;
                }
                commands.push(command);
                if replaceable_clip_command_start.is_none() {
                    replaceable_clip_command_start = commands.len().checked_sub(1);
                }
                clip_active = true;
            }
            EMR_EXTTEXTOUTA | EMR_EXTTEXTOUTW => {
                if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                    return None;
                }
                let texts = if record_type == EMR_EXTTEXTOUTA {
                    parse_emf_exttextouta(data, &header, &coordinates, state.font_charset)
                } else {
                    parse_emf_exttextoutw(data, &header, &coordinates)
                };
                if let Some(texts) = texts {
                    for text in texts {
                        push_emf_text_command(&mut commands, text, &state, &header, &coordinates)?;
                    }
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_POLYTEXTOUTA | EMR_POLYTEXTOUTW => {
                let texts = if record_type == EMR_POLYTEXTOUTA {
                    parse_emf_polytextout(data, &header, &coordinates, false, state.font_charset)
                } else {
                    parse_emf_polytextout(data, &header, &coordinates, true, None)
                };
                if let Some(texts) = texts {
                    for text in texts {
                        push_emf_text_command(&mut commands, text, &state, &header, &coordinates)?;
                    }
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_SETICMMODE => {
                if !matches!(read_le_u32(data, 0), Some(1 | 3 | 4)) {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_SETPIXELV => {
                if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                    return None;
                }
                if let Some((left, top, right, bottom, color)) =
                    parse_emf_setpixelv_rect(data, &header, &coordinates)
                {
                    commands.push(StaticImageVectorCommand::Rectangle {
                        left,
                        top,
                        right,
                        bottom,
                        stroke_color: None,
                        stroke_width: 0.0,
                        stroke_style: BorderStyle::Single,
                        fill_pattern: ShadingPattern::None,
                        fill_color: Some(color),
                    });
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_BITBLT | EMR_STRETCHBLT | EMR_SETDIBITSTODEVICE | EMR_STRETCHDIBITS => {
                if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                    return None;
                }
                let raster_transfer = match record_type {
                    EMR_BITBLT => Some((40, 32, 16, 24)),
                    EMR_STRETCHBLT => Some((100, 32, 16, 24)),
                    EMR_SETDIBITSTODEVICE => None,
                    EMR_STRETCHDIBITS => Some((72, 60, 16, 64)),
                    _ => unreachable!(),
                };
                let blank_destination = emf_commands_are_unpainted_clip_scope(&commands);
                if raster_transfer.is_some_and(|raster_transfer| {
                    is_passive_invisible_emf_raster_transfer(data, raster_transfer.3)
                }) {
                } else if record_type == EMR_SETDIBITSTODEVICE {
                    if is_passive_noop_emf_setdibitstodevice(data) {
                    } else if let Some(command) =
                        parse_emf_setdibitstodevice_srcopy(data, &header, &coordinates)
                    {
                        push_passive_source_raster_command(&mut commands, command)?;
                    } else {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                    }
                } else if record_type == EMR_STRETCHDIBITS {
                    if let Some(command) = parse_emf_stretchdibits_srcopy(
                        data,
                        &header,
                        &coordinates,
                        state.fill_color,
                        blank_destination,
                    ) {
                        push_passive_source_raster_command(&mut commands, command)?;
                    } else if let Some(raster_transfer) = raster_transfer
                        && let Some((left, top, right, bottom, fill_color)) =
                            parse_emf_passive_raster_transfer(
                                data,
                                raster_transfer.0,
                                raster_transfer.1,
                                raster_transfer.2,
                                raster_transfer.3,
                                &header,
                                &coordinates,
                                state.fill_color,
                                blank_destination,
                            )
                        && bounds_is_visible((left, top, right, bottom))
                    {
                        commands.push(StaticImageVectorCommand::Rectangle {
                            left,
                            top,
                            right,
                            bottom,
                            stroke_color: None,
                            stroke_width: 0.0,
                            stroke_style: BorderStyle::Single,
                            fill_pattern: ShadingPattern::None,
                            fill_color: Some(fill_color),
                        });
                    } else {
                        let raster_transfer = raster_transfer?;
                        if let (Some(bounds), Some(raster_op)) = (
                            parse_emf_raster_destination_bounds(
                                data,
                                raster_transfer.0,
                                raster_transfer.2,
                                raster_transfer.3,
                                &header,
                                &coordinates,
                            ),
                            read_le_u32(data, raster_transfer.1),
                        ) && apply_passive_solid_backdrop_raster_transfer(
                            &mut commands,
                            bounds,
                            raster_op,
                            state.fill_color,
                        ) {
                        } else if is_passive_noop_raster_transfer(
                            data,
                            raster_transfer.1,
                            state.fill_color,
                        ) {
                        } else if is_passive_noop_emf_stretchdibits_source_extent(data) {
                        } else {
                            skipped_record_count = skipped_record_count.checked_add(1)?;
                        }
                    }
                } else if matches!(record_type, EMR_BITBLT | EMR_STRETCHBLT)
                    && let Some(command) = parse_emf_bitmap_blt_srcopy(
                        data,
                        record_type,
                        &header,
                        &coordinates,
                        state.fill_color,
                        blank_destination,
                    )
                {
                    push_passive_source_raster_command(&mut commands, command)?;
                } else if let Some(raster_transfer) = raster_transfer
                    && let Some((left, top, right, bottom, fill_color)) =
                        parse_emf_passive_raster_transfer(
                            data,
                            raster_transfer.0,
                            raster_transfer.1,
                            raster_transfer.2,
                            raster_transfer.3,
                            &header,
                            &coordinates,
                            state.fill_color,
                            blank_destination,
                        )
                    && bounds_is_visible((left, top, right, bottom))
                {
                    commands.push(StaticImageVectorCommand::Rectangle {
                        left,
                        top,
                        right,
                        bottom,
                        stroke_color: None,
                        stroke_width: 0.0,
                        stroke_style: BorderStyle::Single,
                        fill_pattern: ShadingPattern::None,
                        fill_color: Some(fill_color),
                    });
                } else if let Some(raster_transfer) = raster_transfer
                    && let (Some(bounds), Some(raster_op)) = (
                        parse_emf_raster_destination_bounds(
                            data,
                            raster_transfer.0,
                            raster_transfer.2,
                            raster_transfer.3,
                            &header,
                            &coordinates,
                        ),
                        read_le_u32(data, raster_transfer.1),
                    )
                    && apply_passive_solid_backdrop_raster_transfer(
                        &mut commands,
                        bounds,
                        raster_op,
                        state.fill_color,
                    )
                {
                } else if let Some(raster_transfer) = raster_transfer
                    && is_passive_noop_raster_transfer(data, raster_transfer.1, state.fill_color)
                {
                } else if record_type == EMR_STRETCHBLT
                    && is_passive_noop_emf_stretchblt_source_extent(data)
                {
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_ALPHABLEND => {
                if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                    return None;
                }
                if is_passive_noop_emf_alphablend(data)
                    || is_passive_invisible_emf_bitmap_transfer(data)
                    || is_passive_noop_emf_bitmap_transfer_source_extent(data)
                {
                } else if let Some(command) =
                    parse_emf_alphablend_passive(data, &header, &coordinates)
                {
                    commands.push(command);
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_TRANSPARENTBLT => {
                if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                    return None;
                }
                if is_passive_invisible_emf_bitmap_transfer(data)
                    || is_passive_noop_emf_bitmap_transfer_source_extent(data)
                {
                } else if let Some(command) =
                    parse_emf_transparentblt_keyed(data, &header, &coordinates)
                {
                    push_passive_source_raster_command(&mut commands, command)?;
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            EMR_RECTANGLE | EMR_ELLIPSE | EMR_ROUNDRECT => {
                let Some((left, top, right, bottom)) =
                    parse_emf_record_rect(data, &header, &coordinates)
                else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if bounds_is_visible((left, top, right, bottom)) {
                    let command = match record_type {
                        EMR_RECTANGLE => StaticImageVectorCommand::Rectangle {
                            left,
                            top,
                            right,
                            bottom,
                            stroke_color: state.passive_stroke_color(),
                            stroke_width: state.passive_stroke_width(&header, &coordinates),
                            stroke_style: state.stroke_style,
                            fill_pattern: state.fill_pattern,
                            fill_color: state.fill_color,
                        },
                        EMR_ELLIPSE => StaticImageVectorCommand::Ellipse {
                            left,
                            top,
                            right,
                            bottom,
                            stroke_color: state.passive_stroke_color(),
                            stroke_width: state.passive_stroke_width(&header, &coordinates),
                            stroke_style: state.stroke_style,
                            fill_pattern: state.fill_pattern,
                            fill_color: state.fill_color,
                        },
                        EMR_ROUNDRECT => {
                            let Some((corner_width, corner_height)) =
                                parse_emf_roundrect_corners(data, right - left, bottom - top)
                            else {
                                skipped_record_count = skipped_record_count.checked_add(1)?;
                                pos = record_end;
                                continue;
                            };
                            StaticImageVectorCommand::RoundedRectangle {
                                left,
                                top,
                                right,
                                bottom,
                                corner_width,
                                corner_height,
                                stroke_color: state.passive_stroke_color(),
                                stroke_width: state.passive_stroke_width(&header, &coordinates),
                                stroke_style: state.stroke_style,
                                fill_pattern: state.fill_pattern,
                                fill_color: state.fill_color,
                            }
                        }
                        _ => return None,
                    };
                    if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                        return None;
                    }
                    commands.push(command);
                }
            }
            EMR_ANGLEARC => {
                let Some((points, end_position)) =
                    parse_emf_anglearc_record(data, &header, &coordinates, current_position)
                else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if let Some(path) = active_path.as_mut() {
                    path.normalized_polyline_to(points, end_position)?;
                } else if points
                    .windows(2)
                    .any(|pair| segment_is_visible(pair[0], pair[1]))
                {
                    if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                        return None;
                    }
                    commands.push(StaticImageVectorCommand::Polyline {
                        points,
                        stroke_color: state.passive_stroke_color(),
                        stroke_width: state.passive_stroke_width(&header, &coordinates),
                        stroke_style: state.stroke_style,
                    });
                }
                current_position = end_position;
            }
            EMR_ARC | EMR_ARCTO | EMR_CHORD | EMR_PIE => {
                match emf_arc_bounds_are_degenerate(data) {
                    Some(true) => {
                        pos = record_end;
                        continue;
                    }
                    Some(false) => {}
                    None => {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                        pos = record_end;
                        continue;
                    }
                }
                let Some(arc) =
                    parse_emf_arc_record(data, &header, &coordinates, state.arc_clockwise)
                else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                match record_type {
                    EMR_ARC | EMR_ARCTO => {
                        if record_type == EMR_ARCTO
                            && let Some(path) = active_path.as_mut()
                        {
                            let mut points = Vec::with_capacity(arc.points.len().checked_add(1)?);
                            points.push(normalized_emf_point(
                                current_position.0,
                                current_position.1,
                                &header,
                                &coordinates,
                            ));
                            points.extend(arc.points);
                            path.normalized_polyline_to(points, arc.end_position)?;
                            current_position = arc.end_position;
                        } else if arc
                            .points
                            .windows(2)
                            .any(|pair| segment_is_visible(pair[0], pair[1]))
                        {
                            if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                                return None;
                            }
                            commands.push(StaticImageVectorCommand::Polyline {
                                points: arc.points,
                                stroke_color: state.passive_stroke_color(),
                                stroke_width: state.passive_stroke_width(&header, &coordinates),
                                stroke_style: state.stroke_style,
                            });
                        }
                        if record_type == EMR_ARCTO {
                            current_position = arc.end_position;
                        }
                    }
                    EMR_CHORD => {
                        if arc.points.len() >= 3 && point_bounds_are_visible(&arc.points) {
                            if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                                return None;
                            }
                            commands.push(StaticImageVectorCommand::Polygon {
                                points: arc.points,
                                stroke_color: state.passive_stroke_color(),
                                stroke_width: state.passive_stroke_width(&header, &coordinates),
                                stroke_style: state.stroke_style,
                                fill_rule: state.fill_rule,
                                fill_pattern: state.fill_pattern,
                                fill_color: state.fill_color,
                            });
                        }
                    }
                    EMR_PIE => {
                        let mut points = Vec::with_capacity(arc.points.len().checked_add(1)?);
                        let Some(center) = normalized_emf_rect_center(data, &header, &coordinates)
                        else {
                            skipped_record_count = skipped_record_count.checked_add(1)?;
                            pos = record_end;
                            continue;
                        };
                        points.push(center);
                        points.extend(arc.points);
                        if points.len() >= 3 && point_bounds_are_visible(&points) {
                            if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                                return None;
                            }
                            commands.push(StaticImageVectorCommand::Polygon {
                                points,
                                stroke_color: state.passive_stroke_color(),
                                stroke_width: state.passive_stroke_width(&header, &coordinates),
                                stroke_style: state.stroke_style,
                                fill_rule: state.fill_rule,
                                fill_pattern: state.fill_pattern,
                                fill_color: state.fill_color,
                            });
                        }
                    }
                    _ => return None,
                }
            }
            EMR_POLYBEZIER => {
                let Some(raw_points) = parse_emf_raw_poly_points(data) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if !polybezier_point_count_is_valid(raw_points.len()) {
                    pos = record_end;
                    continue;
                }
                let points: Vec<_> = raw_points
                    .iter()
                    .map(|(x, y)| normalized_emf_point(*x, *y, &header, &coordinates))
                    .collect();
                if point_bounds_are_visible(&points) {
                    if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                        return None;
                    }
                    commands.push(StaticImageVectorCommand::Bezier {
                        points,
                        stroke_color: state.passive_stroke_color(),
                        stroke_width: state.passive_stroke_width(&header, &coordinates),
                        stroke_style: state.stroke_style,
                    });
                }
                current_position = *raw_points.last()?;
            }
            EMR_POLYBEZIERTO => {
                let Some(raw_points) = parse_emf_raw_poly_points(data) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if !polybezierto_point_count_is_valid(raw_points.len()) {
                    pos = record_end;
                    continue;
                }
                if let Some(path) = active_path.as_mut() {
                    path.bezier_to(&raw_points, &header, &coordinates)?;
                } else {
                    let mut points = Vec::with_capacity(raw_points.len().checked_add(1)?);
                    points.push(normalized_emf_point(
                        current_position.0,
                        current_position.1,
                        &header,
                        &coordinates,
                    ));
                    for (x, y) in &raw_points {
                        points.push(normalized_emf_point(*x, *y, &header, &coordinates));
                    }
                    if point_bounds_are_visible(&points) {
                        if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                            return None;
                        }
                        commands.push(StaticImageVectorCommand::Bezier {
                            points,
                            stroke_color: state.passive_stroke_color(),
                            stroke_width: state.passive_stroke_width(&header, &coordinates),
                            stroke_style: state.stroke_style,
                        });
                    }
                }
                current_position = *raw_points.last()?;
            }
            EMR_POLYGON | EMR_POLYLINE => {
                let Some(points) = parse_emf_poly_points(data, &header, &coordinates) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if record_type == EMR_POLYGON {
                    if points.len() >= 3 {
                        if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                            return None;
                        }
                        commands.push(StaticImageVectorCommand::Polygon {
                            points,
                            stroke_color: state.passive_stroke_color(),
                            stroke_width: state.passive_stroke_width(&header, &coordinates),
                            stroke_style: state.stroke_style,
                            fill_rule: state.fill_rule,
                            fill_pattern: state.fill_pattern,
                            fill_color: state.fill_color,
                        });
                    }
                } else if points.len() >= 2 {
                    if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                        return None;
                    }
                    commands.push(StaticImageVectorCommand::Polyline {
                        points,
                        stroke_color: state.passive_stroke_color(),
                        stroke_width: state.passive_stroke_width(&header, &coordinates),
                        stroke_style: state.stroke_style,
                    });
                }
            }
            EMR_POLYLINETO => {
                let Some(raw_points) = parse_emf_raw_poly_points(data) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if raw_points.is_empty() {
                    pos = record_end;
                    continue;
                }
                if let Some(path) = active_path.as_mut() {
                    path.polyline_to(&raw_points, &header, &coordinates)?;
                } else {
                    let mut points = Vec::with_capacity(raw_points.len().checked_add(1)?);
                    points.push(normalized_emf_point(
                        current_position.0,
                        current_position.1,
                        &header,
                        &coordinates,
                    ));
                    for (x, y) in &raw_points {
                        points.push(normalized_emf_point(*x, *y, &header, &coordinates));
                    }
                    if points
                        .windows(2)
                        .any(|pair| segment_is_visible(pair[0], pair[1]))
                    {
                        if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                            return None;
                        }
                        commands.push(StaticImageVectorCommand::Polyline {
                            points,
                            stroke_color: state.passive_stroke_color(),
                            stroke_width: state.passive_stroke_width(&header, &coordinates),
                            stroke_style: state.stroke_style,
                        });
                    }
                }
                current_position = *raw_points.last()?;
            }
            EMR_POLYGON16 | EMR_POLYLINE16 => {
                let Some(points) = parse_emf_poly16_points(data, &header, &coordinates) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if record_type == EMR_POLYGON16 {
                    if points.len() >= 3 {
                        if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                            return None;
                        }
                        commands.push(StaticImageVectorCommand::Polygon {
                            points,
                            stroke_color: state.passive_stroke_color(),
                            stroke_width: state.passive_stroke_width(&header, &coordinates),
                            stroke_style: state.stroke_style,
                            fill_rule: state.fill_rule,
                            fill_pattern: state.fill_pattern,
                            fill_color: state.fill_color,
                        });
                    }
                } else if points.len() >= 2 {
                    if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                        return None;
                    }
                    commands.push(StaticImageVectorCommand::Polyline {
                        points,
                        stroke_color: state.passive_stroke_color(),
                        stroke_width: state.passive_stroke_width(&header, &coordinates),
                        stroke_style: state.stroke_style,
                    });
                }
            }
            EMR_POLYBEZIER16 => {
                let Some(raw_points) = parse_emf_raw_poly16_points(data) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if !polybezier_point_count_is_valid(raw_points.len()) {
                    pos = record_end;
                    continue;
                }
                let points: Vec<_> = raw_points
                    .iter()
                    .map(|(x, y)| normalized_emf_point(*x, *y, &header, &coordinates))
                    .collect();
                if point_bounds_are_visible(&points) {
                    if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                        return None;
                    }
                    commands.push(StaticImageVectorCommand::Bezier {
                        points,
                        stroke_color: state.passive_stroke_color(),
                        stroke_width: state.passive_stroke_width(&header, &coordinates),
                        stroke_style: state.stroke_style,
                    });
                }
                current_position = *raw_points.last()?;
            }
            EMR_POLYBEZIERTO16 => {
                let Some(raw_points) = parse_emf_raw_poly16_points(data) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if !polybezierto_point_count_is_valid(raw_points.len()) {
                    pos = record_end;
                    continue;
                }
                if let Some(path) = active_path.as_mut() {
                    path.bezier_to(&raw_points, &header, &coordinates)?;
                } else {
                    let mut points = Vec::with_capacity(raw_points.len().checked_add(1)?);
                    points.push(normalized_emf_point(
                        current_position.0,
                        current_position.1,
                        &header,
                        &coordinates,
                    ));
                    for (x, y) in &raw_points {
                        points.push(normalized_emf_point(*x, *y, &header, &coordinates));
                    }
                    if point_bounds_are_visible(&points) {
                        if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                            return None;
                        }
                        commands.push(StaticImageVectorCommand::Bezier {
                            points,
                            stroke_color: state.passive_stroke_color(),
                            stroke_width: state.passive_stroke_width(&header, &coordinates),
                            stroke_style: state.stroke_style,
                        });
                    }
                }
                current_position = *raw_points.last()?;
            }
            EMR_POLYLINETO16 => {
                let Some(raw_points) = parse_emf_raw_poly16_points(data) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if raw_points.is_empty() {
                    pos = record_end;
                    continue;
                }
                if let Some(path) = active_path.as_mut() {
                    path.polyline_to(&raw_points, &header, &coordinates)?;
                } else {
                    let mut points = Vec::with_capacity(raw_points.len().checked_add(1)?);
                    points.push(normalized_emf_point(
                        current_position.0,
                        current_position.1,
                        &header,
                        &coordinates,
                    ));
                    for (x, y) in &raw_points {
                        points.push(normalized_emf_point(*x, *y, &header, &coordinates));
                    }
                    if points
                        .windows(2)
                        .any(|pair| segment_is_visible(pair[0], pair[1]))
                    {
                        if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                            return None;
                        }
                        commands.push(StaticImageVectorCommand::Polyline {
                            points,
                            stroke_color: state.passive_stroke_color(),
                            stroke_width: state.passive_stroke_width(&header, &coordinates),
                            stroke_style: state.stroke_style,
                        });
                    }
                }
                current_position = *raw_points.last()?;
            }
            EMR_POLYDRAW | EMR_POLYDRAW16 => {
                let parsed = if record_type == EMR_POLYDRAW {
                    let Some(parsed) = parse_emf_polydraw(
                        data,
                        &header,
                        &coordinates,
                        current_position,
                        state.passive_stroke_color(),
                        state.passive_stroke_width(&header, &coordinates),
                        state.stroke_style,
                    ) else {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                        pos = record_end;
                        continue;
                    };
                    parsed
                } else {
                    let Some(parsed) = parse_emf_polydraw16(
                        data,
                        &header,
                        &coordinates,
                        current_position,
                        state.passive_stroke_color(),
                        state.passive_stroke_width(&header, &coordinates),
                        state.stroke_style,
                    ) else {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                        pos = record_end;
                        continue;
                    };
                    parsed
                };
                if commands.len().checked_add(parsed.commands.len())? > MAX_PASSIVE_WMF_COMMANDS {
                    return None;
                }
                commands.extend(parsed.commands);
                current_position = parsed.end_position;
            }
            EMR_POLYPOLYGON | EMR_POLYPOLYLINE => {
                let Some(polygons) = parse_emf_poly_poly_points(data, &header, &coordinates) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if record_type == EMR_POLYPOLYGON {
                    if let Some(command) = emf_polypolygon_path_command(
                        polygons,
                        state.passive_stroke_color(),
                        state.passive_stroke_width(&header, &coordinates),
                        state.stroke_style,
                        state.fill_rule,
                        state.fill_pattern,
                        state.fill_color,
                    )? {
                        if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                            return None;
                        }
                        commands.push(command);
                    }
                } else {
                    for points in polygons {
                        if points
                            .windows(2)
                            .any(|pair| segment_is_visible(pair[0], pair[1]))
                        {
                            if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                                return None;
                            }
                            commands.push(StaticImageVectorCommand::Polyline {
                                points,
                                stroke_color: state.passive_stroke_color(),
                                stroke_width: state.passive_stroke_width(&header, &coordinates),
                                stroke_style: state.stroke_style,
                            });
                        }
                    }
                }
            }
            EMR_POLYPOLYGON16 | EMR_POLYPOLYLINE16 => {
                let Some(polygons) = parse_emf_poly_poly16_points(data, &header, &coordinates)
                else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if record_type == EMR_POLYPOLYGON16 {
                    if let Some(command) = emf_polypolygon_path_command(
                        polygons,
                        state.passive_stroke_color(),
                        state.passive_stroke_width(&header, &coordinates),
                        state.stroke_style,
                        state.fill_rule,
                        state.fill_pattern,
                        state.fill_color,
                    )? {
                        if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                            return None;
                        }
                        commands.push(command);
                    }
                } else {
                    for points in polygons {
                        if points
                            .windows(2)
                            .any(|pair| segment_is_visible(pair[0], pair[1]))
                        {
                            if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                                return None;
                            }
                            commands.push(StaticImageVectorCommand::Polyline {
                                points,
                                stroke_color: state.passive_stroke_color(),
                                stroke_width: state.passive_stroke_width(&header, &coordinates),
                                stroke_style: state.stroke_style,
                            });
                        }
                    }
                }
            }
            _ => {
                skipped_record_count = skipped_record_count.checked_add(1)?;
            }
        }

        pos = record_end;
    }

    if !reached_eof {
        return None;
    }
    let _ = active_path.take();
    if !saved_states.is_empty() {
        if commands.len().checked_add(saved_states.len())? > MAX_PASSIVE_WMF_COMMANDS {
            return None;
        }
        for _ in 0..saved_states.len() {
            commands.push(StaticImageVectorCommand::RestoreState);
        }
    }

    if commands.is_empty() && skipped_record_count > 0 {
        return None;
    }

    Some(ParsedEmfVector {
        width_px: header.width_px,
        height_px: header.height_px,
        frame_width_twips: header.frame_width_twips,
        frame_height_twips: header.frame_height_twips,
        skipped_record_count,
        has_valid_reference_device: header.has_valid_reference_device,
        commands,
    })
}

fn parse_emf_record_rect(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> Option<(f32, f32, f32, f32)> {
    if data.len() < 16 {
        return None;
    }
    let left = read_le_i32(data, 0)?;
    let top = read_le_i32(data, 4)?;
    let right = read_le_i32(data, 8)?;
    let bottom = read_le_i32(data, 12)?;
    normalized_emf_rect(left, top, right, bottom, header, coordinates)
}

fn parse_emf_record_rect_including_degenerate(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> Option<(f32, f32, f32, f32)> {
    if data.len() < 16 {
        return None;
    }
    let left = read_le_i32(data, 0)?;
    let top = read_le_i32(data, 4)?;
    let right = read_le_i32(data, 8)?;
    let bottom = read_le_i32(data, 12)?;
    let (x1, y1) = normalized_emf_point(left, top, header, coordinates);
    let (x2, y2) = normalized_emf_point(right, bottom, header, coordinates);
    Some((x1.min(x2), y1.min(y2), x1.max(x2), y1.max(y2)))
}

fn parse_emf_region_data_rects(
    data: &[u8],
    byte_count_offset: usize,
    region_offset: usize,
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> Option<Vec<(f32, f32, f32, f32)>> {
    let byte_count = usize::try_from(read_le_u32(data, byte_count_offset)?).ok()?;
    if byte_count < 32 {
        return None;
    }
    let region_end = region_offset.checked_add(byte_count)?;
    if region_end > data.len() {
        return None;
    }
    let region = data.get(region_offset..region_end)?;
    let header_size = usize::try_from(read_le_u32(region, 0)?).ok()?;
    let region_type = read_le_u32(region, 4)?;
    let rect_count = usize::try_from(read_le_u32(region, 8)?).ok()?;
    let region_size = usize::try_from(read_le_u32(region, 12)?).ok()?;
    if header_size != 32
        || region_type != 1
        || rect_count == 0
        || rect_count > MAX_PASSIVE_WMF_POINTS_PER_RECORD
    {
        return None;
    }
    let rect_bytes = rect_count.checked_mul(16)?;
    if region_size < rect_bytes || header_size.checked_add(rect_bytes)? > region.len() {
        return None;
    }

    let mut rects = Vec::with_capacity(rect_count);
    for index in 0..rect_count {
        let offset = header_size.checked_add(index.checked_mul(16)?)?;
        let left = read_le_i32(region, offset)?;
        let top = read_le_i32(region, offset.checked_add(4)?)?;
        let right = read_le_i32(region, offset.checked_add(8)?)?;
        let bottom = read_le_i32(region, offset.checked_add(12)?)?;
        rects.push(normalized_emf_rect(
            left,
            top,
            right,
            bottom,
            header,
            coordinates,
        )?);
    }
    Some(rects)
}

fn is_passive_empty_emf_region(
    data: &[u8],
    byte_count_offset: usize,
    region_offset: usize,
) -> bool {
    let Some(byte_count) =
        read_le_u32(data, byte_count_offset).and_then(|value| usize::try_from(value).ok())
    else {
        return false;
    };
    if byte_count < 32 {
        return false;
    }
    let Some(region_end) = region_offset.checked_add(byte_count) else {
        return false;
    };
    let Some(region) = data.get(region_offset..region_end) else {
        return false;
    };
    matches!(
        (
            read_le_u32(region, 0),
            read_le_u32(region, 4),
            read_le_u32(region, 8),
            read_le_u32(region, 12),
        ),
        (Some(32), Some(1), Some(0), Some(0))
    )
}

fn is_passive_empty_emf_clip_combine_noop(region_mode: u32) -> bool {
    matches!(region_mode, EMF_RGN_OR | EMF_RGN_XOR | EMF_RGN_DIFF)
}

fn is_passive_noop_emf_framergn(data: &[u8]) -> bool {
    data.len() >= 32
        && matches!(
            (read_le_i32(data, 24), read_le_i32(data, 28)),
            (Some(0), _) | (_, Some(0))
        )
}

fn emf_region_clip_path_command(
    rects: Vec<(f32, f32, f32, f32)>,
) -> Option<StaticImageVectorCommand> {
    let first = *rects.first()?;
    let mut segments = Vec::with_capacity(rects.len().checked_mul(5)?.saturating_sub(1));
    segments.push(StaticImageVectorPathSegment::LineTo(first.2, first.1));
    segments.push(StaticImageVectorPathSegment::LineTo(first.2, first.3));
    segments.push(StaticImageVectorPathSegment::LineTo(first.0, first.3));
    segments.push(StaticImageVectorPathSegment::LineTo(first.0, first.1));
    for (left, top, right, bottom) in rects.into_iter().skip(1) {
        segments.push(StaticImageVectorPathSegment::MoveTo(left, top));
        segments.push(StaticImageVectorPathSegment::LineTo(right, top));
        segments.push(StaticImageVectorPathSegment::LineTo(right, bottom));
        segments.push(StaticImageVectorPathSegment::LineTo(left, bottom));
        segments.push(StaticImageVectorPathSegment::LineTo(left, top));
    }
    Some(StaticImageVectorCommand::ClipPath {
        start: (first.0, first.1),
        segments,
        closed: false,
        fill_rule: StaticImageVectorFillRule::Winding,
    })
}

fn emf_exclude_clip_region_command(
    header: &ParsedEmfHeader,
    rects: Vec<(f32, f32, f32, f32)>,
) -> Option<StaticImageVectorCommand> {
    if rects.is_empty() || !rectangles_are_pairwise_non_overlapping(&rects) {
        return None;
    }
    let image_right = header.width_px.max(1) as f32;
    let image_bottom = header.height_px.max(1) as f32;
    let mut segments = Vec::with_capacity(rects.len().checked_mul(5)?.checked_add(4)?);
    segments.push(StaticImageVectorPathSegment::LineTo(image_right, 0.0));
    segments.push(StaticImageVectorPathSegment::LineTo(
        image_right,
        image_bottom,
    ));
    segments.push(StaticImageVectorPathSegment::LineTo(0.0, image_bottom));
    segments.push(StaticImageVectorPathSegment::LineTo(0.0, 0.0));
    for (left, top, right, bottom) in rects {
        segments.push(StaticImageVectorPathSegment::MoveTo(left, top));
        segments.push(StaticImageVectorPathSegment::LineTo(right, top));
        segments.push(StaticImageVectorPathSegment::LineTo(right, bottom));
        segments.push(StaticImageVectorPathSegment::LineTo(left, bottom));
        segments.push(StaticImageVectorPathSegment::LineTo(left, top));
    }
    Some(StaticImageVectorCommand::ClipPath {
        start: (0.0, 0.0),
        segments,
        closed: false,
        fill_rule: StaticImageVectorFillRule::Alternate,
    })
}

pub(in crate::rtf::parser) fn vector_diff_from_unpainted_clip_rect_command(
    commands: &[StaticImageVectorCommand],
    clip_start: usize,
    rects: Vec<(f32, f32, f32, f32)>,
) -> Option<Option<StaticImageVectorCommand>> {
    let outer = unpainted_clip_rect(commands, clip_start)?;

    let mut clipped_rects = Vec::new();
    for rect in rects {
        let clipped = (
            rect.0.max(outer.0),
            rect.1.max(outer.1),
            rect.2.min(outer.2),
            rect.3.min(outer.3),
        );
        if bounds_is_visible(clipped) {
            clipped_rects.push(clipped);
        }
    }
    if clipped_rects.is_empty() {
        return Some(None);
    }
    if !rectangles_are_pairwise_non_overlapping(&clipped_rects) {
        return None;
    }

    Some(Some(emf_exclude_rectangles_from_rect_command(
        outer,
        clipped_rects,
    )?))
}

fn vector_xor_inside_unpainted_clip_rect_command(
    commands: &[StaticImageVectorCommand],
    clip_start: usize,
    rects: Vec<(f32, f32, f32, f32)>,
) -> Option<Option<StaticImageVectorCommand>> {
    let outer = unpainted_clip_rect(commands, clip_start)?;
    let visible_rects: Vec<_> = rects
        .into_iter()
        .filter(|rect| bounds_is_visible(*rect))
        .collect();
    if visible_rects.is_empty() {
        return Some(None);
    }
    if !visible_rects
        .iter()
        .all(|rect| rectangle_is_inside_rect(*rect, outer))
        || !rectangles_are_pairwise_non_overlapping(&visible_rects)
    {
        return None;
    }

    Some(Some(emf_exclude_rectangles_from_rect_command(
        outer,
        visible_rects,
    )?))
}

fn vector_rects_are_inside_unpainted_clip_rect(
    commands: &[StaticImageVectorCommand],
    clip_start: usize,
    rects: &[(f32, f32, f32, f32)],
) -> Option<bool> {
    let outer = unpainted_clip_rect(commands, clip_start)?;
    Some(
        rects
            .iter()
            .all(|rect| !bounds_is_visible(*rect) || rectangle_is_inside_rect(*rect, outer)),
    )
}

fn vector_region_or_unpainted_clip_rect_command(
    commands: &[StaticImageVectorCommand],
    clip_start: usize,
    rects: &[(f32, f32, f32, f32)],
) -> Option<Option<StaticImageVectorCommand>> {
    let outer = unpainted_clip_rect(commands, clip_start)?;
    let visible_rects: Vec<_> = rects
        .iter()
        .copied()
        .filter(|rect| bounds_is_visible(*rect))
        .collect();
    if visible_rects.is_empty()
        || vector_rects_are_inside_unpainted_clip_rect(commands, clip_start, &visible_rects)?
    {
        return Some(None);
    }
    if !visible_rects
        .iter()
        .any(|rect| rectangle_is_inside_rect(outer, *rect))
    {
        return None;
    }
    if visible_rects.len() == 1 {
        let (left, top, right, bottom) = visible_rects[0];
        return Some(Some(StaticImageVectorCommand::ClipRect {
            left,
            top,
            right,
            bottom,
        }));
    }

    Some(Some(emf_region_clip_path_command(visible_rects)?))
}

fn emf_exclude_rectangles_from_rect_command(
    outer: (f32, f32, f32, f32),
    rects: Vec<(f32, f32, f32, f32)>,
) -> Option<StaticImageVectorCommand> {
    let mut segments = Vec::with_capacity(rects.len().checked_mul(5)?.checked_add(4)?);
    segments.push(StaticImageVectorPathSegment::LineTo(outer.2, outer.1));
    segments.push(StaticImageVectorPathSegment::LineTo(outer.2, outer.3));
    segments.push(StaticImageVectorPathSegment::LineTo(outer.0, outer.3));
    segments.push(StaticImageVectorPathSegment::LineTo(outer.0, outer.1));
    for (left, top, right, bottom) in rects {
        segments.push(StaticImageVectorPathSegment::MoveTo(left, top));
        segments.push(StaticImageVectorPathSegment::LineTo(right, top));
        segments.push(StaticImageVectorPathSegment::LineTo(right, bottom));
        segments.push(StaticImageVectorPathSegment::LineTo(left, bottom));
        segments.push(StaticImageVectorPathSegment::LineTo(left, top));
    }
    Some(StaticImageVectorCommand::ClipPath {
        start: (outer.0, outer.1),
        segments,
        closed: false,
        fill_rule: StaticImageVectorFillRule::Alternate,
    })
}

fn vector_diff_path_from_unpainted_clip_rect_command(
    commands: &[StaticImageVectorCommand],
    clip_start: usize,
    path: EmfPathBuilder,
) -> Option<Option<StaticImageVectorCommand>> {
    let outer = unpainted_clip_rect(commands, clip_start)?;
    if path.collecting {
        return None;
    }
    let (path_start, path_segments, path_closed) = path.path_segments()?;
    if !path_is_inside_rect(path_start, &path_segments, outer) {
        return None;
    }

    Some(Some(emf_exclude_path_from_rect_command(
        outer,
        path_start,
        path_segments,
        path_closed,
    )?))
}

fn vector_path_is_inside_unpainted_clip_rect(
    commands: &[StaticImageVectorCommand],
    clip_start: usize,
    path: &EmfPathBuilder,
) -> Option<bool> {
    let outer = unpainted_clip_rect(commands, clip_start)?;
    if path.collecting {
        return None;
    }
    let (path_start, path_segments, _) = path.path_segments()?;
    Some(path_is_inside_rect(path_start, &path_segments, outer))
}

fn vector_path_or_unpainted_clip_rect_command(
    commands: &[StaticImageVectorCommand],
    clip_start: usize,
    path: &EmfPathBuilder,
) -> Option<Option<StaticImageVectorCommand>> {
    if vector_path_is_inside_unpainted_clip_rect(commands, clip_start, path)? {
        return Some(None);
    }
    let outer = unpainted_clip_rect(commands, clip_start)?;
    if path.collecting {
        return None;
    }
    let (path_start, path_segments, path_closed) = path.path_segments()?;
    let path_rect = rectangular_path_bounds(path_start, &path_segments, path_closed)?;
    if !rectangle_is_inside_rect(outer, path_rect) {
        return None;
    }
    Some(Some(StaticImageVectorCommand::ClipRect {
        left: path_rect.0,
        top: path_rect.1,
        right: path_rect.2,
        bottom: path_rect.3,
    }))
}

fn unpainted_clip_rect(
    commands: &[StaticImageVectorCommand],
    clip_start: usize,
) -> Option<(f32, f32, f32, f32)> {
    if commands.len() != clip_start.checked_add(1)? {
        return None;
    }
    let StaticImageVectorCommand::ClipRect {
        left,
        top,
        right,
        bottom,
    } = commands.get(clip_start)?
    else {
        return None;
    };
    let outer = (*left, *top, *right, *bottom);
    if !bounds_is_visible(outer) {
        return None;
    }
    Some(outer)
}

fn emf_exclude_path_from_rect_command(
    outer: (f32, f32, f32, f32),
    path_start: (f32, f32),
    path_segments: Vec<StaticImageVectorPathSegment>,
    path_closed: bool,
) -> Option<StaticImageVectorCommand> {
    let mut segments = Vec::with_capacity(path_segments.len().checked_add(5)?);
    segments.push(StaticImageVectorPathSegment::LineTo(outer.2, outer.1));
    segments.push(StaticImageVectorPathSegment::LineTo(outer.2, outer.3));
    segments.push(StaticImageVectorPathSegment::LineTo(outer.0, outer.3));
    segments.push(StaticImageVectorPathSegment::LineTo(outer.0, outer.1));
    segments.push(StaticImageVectorPathSegment::MoveTo(
        path_start.0,
        path_start.1,
    ));
    segments.extend(path_segments);
    Some(StaticImageVectorCommand::ClipPath {
        start: (outer.0, outer.1),
        segments,
        closed: path_closed,
        fill_rule: StaticImageVectorFillRule::Alternate,
    })
}

fn path_is_inside_rect(
    start: (f32, f32),
    segments: &[StaticImageVectorPathSegment],
    rect: (f32, f32, f32, f32),
) -> bool {
    point_is_inside_rect(start, rect) && path_segments_are_inside_rect(segments, rect)
}

fn path_segments_are_inside_rect(
    segments: &[StaticImageVectorPathSegment],
    rect: (f32, f32, f32, f32),
) -> bool {
    segments.iter().all(|segment| match segment {
        StaticImageVectorPathSegment::MoveTo(x, y) | StaticImageVectorPathSegment::LineTo(x, y) => {
            point_is_inside_rect((*x, *y), rect)
        }
        StaticImageVectorPathSegment::CubicTo {
            control1,
            control2,
            end,
        } => {
            point_is_inside_rect(*control1, rect)
                && point_is_inside_rect(*control2, rect)
                && point_is_inside_rect(*end, rect)
        }
    })
}

fn point_is_inside_rect(point: (f32, f32), rect: (f32, f32, f32, f32)) -> bool {
    point.0 >= rect.0 && point.0 <= rect.2 && point.1 >= rect.1 && point.1 <= rect.3
}

fn rectangle_is_inside_rect(inner: (f32, f32, f32, f32), outer: (f32, f32, f32, f32)) -> bool {
    inner.0 >= outer.0 && inner.1 >= outer.1 && inner.2 <= outer.2 && inner.3 <= outer.3
}

fn rectangular_path_bounds(
    start: (f32, f32),
    segments: &[StaticImageVectorPathSegment],
    closed: bool,
) -> Option<(f32, f32, f32, f32)> {
    if !closed || segments.len() != 4 {
        return None;
    }
    let mut points = Vec::with_capacity(5);
    points.push(start);
    for segment in segments {
        let StaticImageVectorPathSegment::LineTo(x, y) = *segment else {
            return None;
        };
        points.push((x, y));
    }
    if points.last().copied()? != start {
        return None;
    }
    let mut unique = points[..4].to_vec();
    unique.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    unique.dedup();
    if unique.len() != 4 {
        return None;
    }
    let left = unique
        .iter()
        .map(|point| point.0)
        .fold(f32::INFINITY, f32::min);
    let top = unique
        .iter()
        .map(|point| point.1)
        .fold(f32::INFINITY, f32::min);
    let right = unique
        .iter()
        .map(|point| point.0)
        .fold(f32::NEG_INFINITY, f32::max);
    let bottom = unique
        .iter()
        .map(|point| point.1)
        .fold(f32::NEG_INFINITY, f32::max);
    if !bounds_is_visible((left, top, right, bottom)) {
        return None;
    }
    let expected = [(left, top), (left, bottom), (right, top), (right, bottom)];
    unique
        .iter()
        .all(|point| expected.contains(point))
        .then_some((left, top, right, bottom))
}

fn rectangles_are_pairwise_non_overlapping(rects: &[(f32, f32, f32, f32)]) -> bool {
    for (index, first) in rects.iter().enumerate() {
        if first.0 >= first.2 || first.1 >= first.3 {
            return false;
        }
        for second in &rects[index.saturating_add(1)..] {
            if first.0 < second.2 && first.2 > second.0 && first.1 < second.3 && first.3 > second.1
            {
                return false;
            }
        }
    }
    true
}

fn emf_exclude_clip_path_command(
    header: &ParsedEmfHeader,
    path: EmfPathBuilder,
) -> Option<StaticImageVectorCommand> {
    if path.collecting {
        return None;
    }
    let (path_start, path_segments, path_closed) = path.path_segments()?;
    let image_right = header.width_px.max(1) as f32;
    let image_bottom = header.height_px.max(1) as f32;
    let mut segments = Vec::with_capacity(path_segments.len().checked_add(5)?);
    segments.push(StaticImageVectorPathSegment::LineTo(image_right, 0.0));
    segments.push(StaticImageVectorPathSegment::LineTo(
        image_right,
        image_bottom,
    ));
    segments.push(StaticImageVectorPathSegment::LineTo(0.0, image_bottom));
    segments.push(StaticImageVectorPathSegment::LineTo(0.0, 0.0));
    segments.push(StaticImageVectorPathSegment::MoveTo(
        path_start.0,
        path_start.1,
    ));
    segments.extend(path_segments);
    Some(StaticImageVectorCommand::ClipPath {
        start: (0.0, 0.0),
        segments,
        closed: path_closed,
        fill_rule: StaticImageVectorFillRule::Alternate,
    })
}

fn push_emf_region_rectangles(
    commands: &mut Vec<StaticImageVectorCommand>,
    rects: Vec<(f32, f32, f32, f32)>,
    fill_color: Color,
    fill_pattern: ShadingPattern,
) -> Option<()> {
    if commands.len().checked_add(rects.len())? > MAX_PASSIVE_WMF_COMMANDS {
        return None;
    }
    let mut emitted = false;
    for (left, top, right, bottom) in rects {
        if bounds_is_visible((left, top, right, bottom)) {
            commands.push(StaticImageVectorCommand::Rectangle {
                left,
                top,
                right,
                bottom,
                stroke_color: None,
                stroke_width: 0.0,
                stroke_style: BorderStyle::Single,
                fill_pattern,
                fill_color: Some(fill_color),
            });
            emitted = true;
        }
    }
    if !emitted {
        return None;
    }
    Some(())
}

fn push_emf_region_frame_rectangles(
    commands: &mut Vec<StaticImageVectorCommand>,
    rects: Vec<(f32, f32, f32, f32)>,
    stroke_width: f32,
    stroke_height: f32,
    fill_color: Color,
    fill_pattern: ShadingPattern,
) -> Option<()> {
    let max_new_commands = rects.len().checked_mul(4)?;
    if commands.len().checked_add(max_new_commands)? > MAX_PASSIVE_WMF_COMMANDS {
        return None;
    }
    let mut emitted = false;
    for (left, top, right, bottom) in rects {
        let frame_width = stroke_width.min(right - left);
        let frame_height = stroke_height.min(bottom - top);
        let right_inner = (right - frame_width).max(left);
        let bottom_inner = (bottom - frame_height).max(top);
        let frame_rects = [
            (left, top, right, (top + frame_height).min(bottom)),
            (left, bottom_inner, right, bottom),
            (left, top, (left + frame_width).min(right), bottom),
            (right_inner, top, right, bottom),
        ];
        for (frame_left, frame_top, frame_right, frame_bottom) in frame_rects {
            if bounds_is_visible((frame_left, frame_top, frame_right, frame_bottom)) {
                commands.push(StaticImageVectorCommand::Rectangle {
                    left: frame_left,
                    top: frame_top,
                    right: frame_right,
                    bottom: frame_bottom,
                    stroke_color: None,
                    stroke_width: 0.0,
                    stroke_style: BorderStyle::Single,
                    fill_pattern,
                    fill_color: Some(fill_color),
                });
                emitted = true;
            }
        }
    }
    if !emitted {
        return None;
    }
    Some(())
}

fn normalized_emf_extent_width(
    width: i32,
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> Option<f32> {
    let end_x = coordinates.window_origin_x.checked_add(width)?;
    let (start, _) = normalized_emf_point(
        coordinates.window_origin_x,
        coordinates.window_origin_y,
        header,
        coordinates,
    );
    let (end, _) = normalized_emf_point(end_x, coordinates.window_origin_y, header, coordinates);
    let width = (end - start).abs();
    (width > 0.0).then_some(width)
}

fn normalized_emf_extent_height(
    height: i32,
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> Option<f32> {
    let end_y = coordinates.window_origin_y.checked_add(height)?;
    let (_, start) = normalized_emf_point(
        coordinates.window_origin_x,
        coordinates.window_origin_y,
        header,
        coordinates,
    );
    let (_, end) = normalized_emf_point(coordinates.window_origin_x, end_y, header, coordinates);
    let height = (end - start).abs();
    (height > 0.0).then_some(height)
}

fn parse_emf_anglearc_record(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
    current_position: (i32, i32),
) -> Option<(Vec<(f32, f32)>, (i32, i32))> {
    if data.len() < 20 {
        return None;
    }
    let center = (read_le_i32(data, 0)?, read_le_i32(data, 4)?);
    let radius = read_le_u32(data, 8)?;
    let start_angle = read_le_f32(data, 12)?;
    let sweep_angle = read_le_f32(data, 16)?;
    let raw_arc_points = sample_emf_anglearc_points(center, radius, start_angle, sweep_angle)?;
    let end_position = *raw_arc_points.last()?;
    let mut points = Vec::with_capacity(raw_arc_points.len().checked_add(1)?);
    points.push(normalized_emf_point(
        current_position.0,
        current_position.1,
        header,
        coordinates,
    ));
    points.extend(
        raw_arc_points
            .into_iter()
            .map(|(x, y)| normalized_emf_point(x, y, header, coordinates)),
    );
    Some((points, end_position))
}

fn sample_emf_anglearc_points(
    center: (i32, i32),
    radius: u32,
    start_angle: f32,
    sweep_angle: f32,
) -> Option<Vec<(i32, i32)>> {
    if !start_angle.is_finite() || !sweep_angle.is_finite() {
        return None;
    }
    if radius == 0 {
        return Some(vec![center]);
    }
    let sweep_angle = sweep_angle.clamp(-360.0, 360.0);
    let center_x = f64::from(center.0);
    let center_y = f64::from(center.1);
    let radius = f64::from(radius);
    let start = f64::from(start_angle).to_radians();
    let sweep = f64::from(sweep_angle).to_radians();
    let start_point = (
        f64_to_i32_rounded(center_x + (radius * start.cos()))?,
        f64_to_i32_rounded(center_y - (radius * start.sin()))?,
    );
    if sweep_angle.abs() < f32::EPSILON {
        return Some(vec![start_point]);
    }
    let segments = ((f64::from(sweep_angle.abs()) / 360.0) * MAX_PASSIVE_WMF_ARC_SEGMENTS as f64)
        .ceil()
        .clamp(4.0, MAX_PASSIVE_WMF_ARC_SEGMENTS as f64) as usize;
    let mut points = Vec::with_capacity(segments.checked_add(1)?);
    for idx in 0..=segments {
        let ratio = idx as f64 / segments as f64;
        let angle = start + (sweep * ratio);
        let x = center_x + (radius * angle.cos());
        let y = center_y - (radius * angle.sin());
        points.push((f64_to_i32_rounded(x)?, f64_to_i32_rounded(y)?));
    }
    Some(points)
}

fn normalized_emf_rect_center(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> Option<(f32, f32)> {
    if data.len() < 16 {
        return None;
    }
    let left = i64::from(read_le_i32(data, 0)?);
    let top = i64::from(read_le_i32(data, 4)?);
    let right = i64::from(read_le_i32(data, 8)?);
    let bottom = i64::from(read_le_i32(data, 12)?);
    let x = i32::try_from((left + right) / 2).ok()?;
    let y = i32::try_from((top + bottom) / 2).ok()?;
    Some(normalized_emf_point(x, y, header, coordinates))
}

fn parse_emf_arc_record(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
    clockwise: bool,
) -> Option<ParsedEmfArc> {
    if data.len() < 32 {
        return None;
    }
    let left = read_le_i32(data, 0)?;
    let top = read_le_i32(data, 4)?;
    let right = read_le_i32(data, 8)?;
    let bottom = read_le_i32(data, 12)?;
    let start = (read_le_i32(data, 16)?, read_le_i32(data, 20)?);
    let end = (read_le_i32(data, 24)?, read_le_i32(data, 28)?);
    let raw_points = sample_emf_arc_points(left, top, right, bottom, start, end, clockwise)?;
    let end_position = *raw_points.last()?;
    let points = raw_points
        .into_iter()
        .map(|(x, y)| normalized_emf_point(x, y, header, coordinates))
        .collect();
    Some(ParsedEmfArc {
        points,
        end_position,
    })
}

fn emf_arc_bounds_are_degenerate(data: &[u8]) -> Option<bool> {
    if data.len() < 16 {
        return None;
    }
    let left = read_le_i32(data, 0)?;
    let top = read_le_i32(data, 4)?;
    let right = read_le_i32(data, 8)?;
    let bottom = read_le_i32(data, 12)?;
    Some(left == right || top == bottom)
}

pub(in crate::rtf::parser) fn sample_emf_arc_points(
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    start: (i32, i32),
    end: (i32, i32),
    clockwise: bool,
) -> Option<Vec<(i32, i32)>> {
    let center_x = (f64::from(left) + f64::from(right)) * 0.5;
    let center_y = (f64::from(top) + f64::from(bottom)) * 0.5;
    let radius_x = (f64::from(right) - f64::from(left)).abs() * 0.5;
    let radius_y = (f64::from(bottom) - f64::from(top)).abs() * 0.5;
    if radius_x <= 0.0 || radius_y <= 0.0 {
        return None;
    }

    let start_angle = emf_arc_angle(center_x, center_y, radius_x, radius_y, start)?;
    let mut end_angle = emf_arc_angle(center_x, center_y, radius_x, radius_y, end)?;
    let full_ellipse = start == end;
    let two_pi = std::f64::consts::PI * 2.0;
    if full_ellipse {
        end_angle = if clockwise {
            start_angle - two_pi
        } else {
            start_angle + two_pi
        };
    } else if clockwise {
        while end_angle >= start_angle {
            end_angle -= two_pi;
        }
    } else {
        while end_angle <= start_angle {
            end_angle += two_pi;
        }
    }
    let span = end_angle - start_angle;
    let segments = ((span.abs() / two_pi) * MAX_PASSIVE_WMF_ARC_SEGMENTS as f64)
        .ceil()
        .clamp(4.0, MAX_PASSIVE_WMF_ARC_SEGMENTS as f64) as usize;
    let mut points = Vec::with_capacity(segments.checked_add(1)?);
    for idx in 0..=segments {
        let ratio = idx as f64 / segments as f64;
        let angle = start_angle + (span * ratio);
        let x = center_x + (radius_x * angle.cos());
        let y = center_y + (radius_y * angle.sin());
        points.push((f64_to_i32_rounded(x)?, f64_to_i32_rounded(y)?));
    }
    Some(points)
}

fn emf_arc_angle(
    center_x: f64,
    center_y: f64,
    radius_x: f64,
    radius_y: f64,
    point: (i32, i32),
) -> Option<f64> {
    let dx = f64::from(point.0) - center_x;
    let dy = f64::from(point.1) - center_y;
    if dx == 0.0 && dy == 0.0 {
        return None;
    }
    Some((dy / radius_y).atan2(dx / radius_x))
}

fn f64_to_i32_rounded(value: f64) -> Option<i32> {
    if !value.is_finite() {
        return None;
    }
    let rounded = value.round();
    if rounded < f64::from(i32::MIN) || rounded > f64::from(i32::MAX) {
        return None;
    }
    Some(rounded as i32)
}

fn parse_emf_exttextoutw(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> Option<Vec<ParsedEmfExtTextOut>> {
    parse_emf_exttextout(data, header, coordinates, true, None)
}

fn parse_emf_exttextouta(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
    font_charset: Option<i32>,
) -> Option<Vec<ParsedEmfExtTextOut>> {
    parse_emf_exttextout(data, header, coordinates, false, font_charset)
}

fn parse_emf_exttextout(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
    unicode: bool,
    font_charset: Option<i32>,
) -> Option<Vec<ParsedEmfExtTextOut>> {
    const EMF_ETO_OPAQUE: u32 = 0x0002;
    const EMF_ETO_CLIPPED: u32 = 0x0004;
    const EMF_ETO_GLYPH_INDEX: u32 = 0x0010;
    const EMF_ETO_NO_RECT: u32 = 0x0100;
    const EMF_ETO_PDY: u32 = 0x2000;
    const EMF_ETO_SUPPORTED_MASK: u32 =
        EMF_ETO_OPAQUE | EMF_ETO_CLIPPED | EMF_ETO_NO_RECT | EMF_ETO_PDY;

    if data.len() < 68 {
        return None;
    }
    let x = read_le_i32(data, 28)?;
    let y = read_le_i32(data, 32)?;
    let char_count = usize::try_from(read_le_u32(data, 36)?).ok()?;
    let max_char_count = if unicode {
        MAX_PASSIVE_WMF_TEXT_BYTES / 2
    } else {
        MAX_PASSIVE_WMF_TEXT_BYTES
    };
    if char_count > max_char_count {
        return None;
    }
    let record_string_offset = usize::try_from(read_le_u32(data, 40)?).ok()?;
    let options = read_le_u32(data, 44)?;
    if options & !EMF_ETO_SUPPORTED_MASK != 0 || options & EMF_ETO_GLYPH_INDEX != 0 {
        return None;
    }
    let bounds = if options & (EMF_ETO_OPAQUE | EMF_ETO_CLIPPED) != 0 {
        normalized_emf_rect(
            read_le_i32(data, 48)?,
            read_le_i32(data, 52)?,
            read_le_i32(data, 56)?,
            read_le_i32(data, 60)?,
            header,
            coordinates,
        )
    } else {
        None
    };
    if char_count == 0 {
        let (x, y) = normalized_emf_point(x, y, header, coordinates);
        return Some(vec![ParsedEmfExtTextOut {
            x,
            y,
            text: String::new(),
            opaque_bounds: if options & EMF_ETO_OPAQUE != 0 {
                bounds
            } else {
                None
            },
            clip_bounds: if options & EMF_ETO_CLIPPED != 0 {
                bounds
            } else {
                None
            },
        }]);
    }
    let byte_count = if unicode {
        char_count.checked_mul(2)?
    } else {
        char_count
    };
    let string_offset = record_string_offset.checked_sub(8)?;
    let string_end = string_offset.checked_add(byte_count)?;
    if string_offset < 68 || string_end > data.len() {
        return None;
    }
    let text = if unicode {
        sanitize_emf_utf16le_text_allow_empty(data.get(string_offset..string_end)?)?
    } else {
        sanitize_emf_ansi_text_allow_empty(data.get(string_offset..string_end)?, font_charset)?
    };
    let (x, y) = normalized_emf_point(x, y, header, coordinates);
    let text = ParsedEmfExtTextOut {
        x,
        y,
        text,
        opaque_bounds: if options & EMF_ETO_OPAQUE != 0 {
            bounds
        } else {
            None
        },
        clip_bounds: if options & EMF_ETO_CLIPPED != 0 {
            bounds
        } else {
            None
        },
    };
    let record_dx_offset = usize::try_from(read_le_u32(data, 64)?).ok()?;
    if text.text.is_empty() {
        return Some(vec![text]);
    }
    if record_dx_offset == 0 {
        return Some(vec![text]);
    }
    split_emf_text_object_by_dx(
        data,
        record_dx_offset,
        char_count,
        text,
        options & EMF_ETO_PDY != 0,
        header,
        coordinates,
    )
}

fn parse_emf_polytextout(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
    unicode: bool,
    font_charset: Option<i32>,
) -> Option<Vec<ParsedEmfExtTextOut>> {
    if data.len() < 32 {
        return None;
    }
    let text_count = usize::try_from(read_le_u32(data, 28)?).ok()?;
    if text_count > MAX_PASSIVE_WMF_POINTS_PER_RECORD {
        return None;
    }
    let text_objects_start = 32usize;
    let text_objects_bytes = text_count.checked_mul(40)?;
    let text_objects_end = text_objects_start.checked_add(text_objects_bytes)?;
    if text_objects_end > data.len() {
        return None;
    }

    let mut total_text_bytes = 0usize;
    let mut texts = Vec::with_capacity(text_count);
    for index in 0..text_count {
        let offset = text_objects_start.checked_add(index.checked_mul(40)?)?;
        let char_count = usize::try_from(read_le_u32(data, offset.checked_add(8)?)?).ok()?;
        let text_bytes = if unicode {
            char_count.checked_mul(2)?
        } else {
            char_count
        };
        total_text_bytes = total_text_bytes.checked_add(text_bytes)?;
        if total_text_bytes > MAX_PASSIVE_WMF_TEXT_BYTES {
            return None;
        }
        let mut text_items =
            parse_emf_text_objects(data, offset, header, coordinates, unicode, font_charset)?;
        texts.append(&mut text_items);
    }
    Some(texts)
}

fn parse_emf_text_objects(
    data: &[u8],
    offset: usize,
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
    unicode: bool,
    font_charset: Option<i32>,
) -> Option<Vec<ParsedEmfExtTextOut>> {
    const EMF_ETO_OPAQUE: u32 = 0x0002;
    const EMF_ETO_CLIPPED: u32 = 0x0004;
    const EMF_ETO_GLYPH_INDEX: u32 = 0x0010;
    const EMF_ETO_NO_RECT: u32 = 0x0100;
    const EMF_ETO_PDY: u32 = 0x2000;
    const EMF_ETO_SUPPORTED_MASK: u32 =
        EMF_ETO_OPAQUE | EMF_ETO_CLIPPED | EMF_ETO_NO_RECT | EMF_ETO_PDY;

    if data.len() < offset.checked_add(40)? {
        return None;
    }
    let x = read_le_i32(data, offset)?;
    let y = read_le_i32(data, offset.checked_add(4)?)?;
    let char_count = usize::try_from(read_le_u32(data, offset.checked_add(8)?)?).ok()?;
    let max_char_count = if unicode {
        MAX_PASSIVE_WMF_TEXT_BYTES / 2
    } else {
        MAX_PASSIVE_WMF_TEXT_BYTES
    };
    if char_count > max_char_count {
        return None;
    }
    let record_string_offset = usize::try_from(read_le_u32(data, offset.checked_add(12)?)?).ok()?;
    let options = read_le_u32(data, offset.checked_add(16)?)?;
    if options & !EMF_ETO_SUPPORTED_MASK != 0 || options & EMF_ETO_GLYPH_INDEX != 0 {
        return None;
    }
    let bounds = if options & (EMF_ETO_OPAQUE | EMF_ETO_CLIPPED) != 0 {
        normalized_emf_rect(
            read_le_i32(data, offset.checked_add(20)?)?,
            read_le_i32(data, offset.checked_add(24)?)?,
            read_le_i32(data, offset.checked_add(28)?)?,
            read_le_i32(data, offset.checked_add(32)?)?,
            header,
            coordinates,
        )
    } else {
        None
    };
    if char_count == 0 {
        let (x, y) = normalized_emf_point(x, y, header, coordinates);
        return Some(vec![ParsedEmfExtTextOut {
            x,
            y,
            text: String::new(),
            opaque_bounds: if options & EMF_ETO_OPAQUE != 0 {
                bounds
            } else {
                None
            },
            clip_bounds: if options & EMF_ETO_CLIPPED != 0 {
                bounds
            } else {
                None
            },
        }]);
    }
    let byte_count = if unicode {
        char_count.checked_mul(2)?
    } else {
        char_count
    };
    let string_offset = record_string_offset.checked_sub(8)?;
    let string_end = string_offset.checked_add(byte_count)?;
    if string_offset < offset.checked_add(40)? || string_end > data.len() {
        return None;
    }
    let text = if unicode {
        sanitize_emf_utf16le_text_allow_empty(data.get(string_offset..string_end)?)?
    } else {
        sanitize_emf_ansi_text_allow_empty(data.get(string_offset..string_end)?, font_charset)?
    };
    let (x, y) = normalized_emf_point(x, y, header, coordinates);
    let text = ParsedEmfExtTextOut {
        x,
        y,
        text,
        opaque_bounds: if options & EMF_ETO_OPAQUE != 0 {
            bounds
        } else {
            None
        },
        clip_bounds: if options & EMF_ETO_CLIPPED != 0 {
            bounds
        } else {
            None
        },
    };
    let record_dx_offset = usize::try_from(read_le_u32(data, offset.checked_add(36)?)?).ok()?;
    if text.text.is_empty() {
        return Some(vec![text]);
    }
    if record_dx_offset == 0 {
        return Some(vec![text]);
    }
    split_emf_text_object_by_dx(
        data,
        record_dx_offset,
        char_count,
        text,
        options & EMF_ETO_PDY != 0,
        header,
        coordinates,
    )
}

fn split_emf_text_object_by_dx(
    data: &[u8],
    record_dx_offset: usize,
    char_count: usize,
    text: ParsedEmfExtTextOut,
    has_pdy: bool,
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> Option<Vec<ParsedEmfExtTextOut>> {
    if char_count > MAX_PASSIVE_WMF_POINTS_PER_RECORD {
        return None;
    }
    let chars: Vec<char> = text.text.chars().collect();
    if chars.len() != char_count {
        return None;
    }
    let dx_offset = record_dx_offset.checked_sub(8)?;
    let spacing_values_per_char = if has_pdy { 2usize } else { 1usize };
    let spacing_stride = spacing_values_per_char.checked_mul(4)?;
    let dx_bytes = char_count.checked_mul(spacing_stride)?;
    let dx_end = dx_offset.checked_add(dx_bytes)?;
    if dx_offset < 32 || dx_end > data.len() {
        return None;
    }
    let mut x = text.x;
    let mut y = text.y;
    let mut items = Vec::with_capacity(char_count);
    for (index, ch) in chars.into_iter().enumerate() {
        let mut item = text.clone();
        item.x = x;
        item.y = y;
        item.text.clear();
        item.text.push(ch);
        if index != 0 {
            item.opaque_bounds = None;
        }
        items.push(item);
        let spacing_offset = dx_offset.checked_add(index.checked_mul(spacing_stride)?)?;
        let dx = read_le_i32(data, spacing_offset)?;
        x += map_emf_length_axis(
            dx,
            coordinates.window_extent_x,
            coordinates.viewport_extent_x,
            header.width_px,
        );
        if has_pdy {
            let dy = read_le_i32(data, spacing_offset.checked_add(4)?)?;
            y += map_emf_length_axis(
                dy,
                coordinates.window_extent_y,
                coordinates.viewport_extent_y,
                header.height_px,
            );
        }
    }
    Some(items)
}

fn push_emf_text_command(
    commands: &mut Vec<StaticImageVectorCommand>,
    text: ParsedEmfExtTextOut,
    state: &EmfDrawingState,
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> Option<()> {
    if let (Some((left, top, right, bottom)), Some(fill_color)) =
        (text.opaque_bounds, state.background_color)
    {
        let required_commands = if text.text.is_empty() { 1usize } else { 2usize };
        if commands.len().checked_add(required_commands)? > MAX_PASSIVE_WMF_COMMANDS {
            return None;
        }
        commands.push(StaticImageVectorCommand::Rectangle {
            left,
            top,
            right,
            bottom,
            stroke_color: None,
            stroke_width: 0.0,
            stroke_style: BorderStyle::Single,
            fill_pattern: ShadingPattern::None,
            fill_color: Some(fill_color),
        });
    } else if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
        return None;
    }
    if text.text.is_empty() {
        return Some(());
    }
    commands.push(StaticImageVectorCommand::Text {
        x: text.x,
        y: text.y,
        height: normalized_emf_text_height(state.font_height, header, coordinates),
        bold: false,
        vertical_scale_percent: 100,
        text: text.text,
        color: state.text_color,
        background_color: if text.opaque_bounds.is_none() {
            match state.text_background_mode {
                WmfTextBackgroundMode::Opaque => state.background_color,
                WmfTextBackgroundMode::Transparent => None,
            }
        } else {
            None
        },
        clip_bounds: text.clip_bounds.map(|(left, top, right, bottom)| {
            StaticImageVectorTextBounds {
                left,
                top,
                right,
                bottom,
            }
        }),
        character_extra: state.text_character_extra,
        word_extra: state.text_word_extra,
        horizontal_align: state.text_horizontal_align,
        vertical_align: state.text_vertical_align,
    });
    Some(())
}

fn sanitize_emf_utf16le_text_allow_empty(bytes: &[u8]) -> Option<String> {
    if bytes.len() % 2 != 0 || bytes.len() > MAX_PASSIVE_WMF_TEXT_BYTES {
        return None;
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();
    let mut text = String::new();
    for item in char::decode_utf16(units) {
        match item.ok()? {
            '\0' => {}
            '\t' | '\n' | '\r' => text.push(' '),
            ch if ch.is_control() => {}
            ch => text.push(ch),
        }
    }
    Some(text)
}

fn sanitize_emf_ansi_text_allow_empty(bytes: &[u8], font_charset: Option<i32>) -> Option<String> {
    if bytes.is_empty() || bytes.len() > MAX_PASSIVE_WMF_TEXT_BYTES {
        return None;
    }
    let code_page = font_charset
        .and_then(CodePage::from_font_charset)
        .unwrap_or(CodePage::Windows1252);
    let mut text = String::new();
    for ch in decode_raw_bytes(bytes, code_page).chars() {
        match ch {
            '\0' => {}
            '\t' | '\n' | '\r' => text.push(' '),
            decoded if decoded.is_control() => {}
            decoded => text.push(decoded),
        }
    }
    Some(text)
}

fn normalized_emf_text_height(
    font_height: Option<i32>,
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> f32 {
    font_height
        .and_then(|height| {
            let height = i32::try_from(height.unsigned_abs()).ok()?;
            normalized_emf_extent_height(height, header, coordinates)
        })
        .unwrap_or_else(|| header.height_px.max(1) as f32 / 12.0)
        .clamp(4.0, 48.0)
}

fn parse_emf_text_justification(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> Option<Option<f32>> {
    if data.len() < 8 {
        return None;
    }
    let break_extra = read_le_i32(data, 0)?;
    let break_count = read_le_i32(data, 4)?;
    if break_count < 0 {
        return Some(None);
    }
    if break_count == 0 || break_extra == 0 {
        return Some(Some(0.0));
    }
    let extra_per_break = break_extra.checked_div(break_count)?;
    let max_axis = i32::try_from(header.width_px.max(1)).ok()?;
    let max = map_emf_length_axis(
        max_axis,
        coordinates.window_extent_x,
        coordinates.viewport_extent_x,
        header.width_px,
    )
    .abs()
    .max(1.0);
    let extra = map_emf_length_axis(
        extra_per_break,
        coordinates.window_extent_x,
        coordinates.viewport_extent_x,
        header.width_px,
    );
    Some(Some(extra.clamp(-max, max)))
}

fn normalized_emf_text_character_extra(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> Option<f32> {
    if data.len() < 4 {
        return None;
    }
    let value = read_le_i32(data, 0)?;
    let max_axis = i32::try_from(header.width_px.max(1)).ok()?;
    let max = map_emf_length_axis(
        max_axis,
        coordinates.window_extent_x,
        coordinates.viewport_extent_x,
        header.width_px,
    )
    .abs()
    .max(1.0);
    let extra = map_emf_length_axis(
        value,
        coordinates.window_extent_x,
        coordinates.viewport_extent_x,
        header.width_px,
    );
    Some(extra.clamp(-max, max))
}

fn parse_emf_arc_direction(data: &[u8]) -> Option<Option<bool>> {
    if data.len() < 4 {
        return None;
    }
    match read_le_u32(data, 0)? {
        1 => Some(Some(false)),
        2 => Some(Some(true)),
        _ => Some(None),
    }
}

fn parse_passive_emf_world_transform(
    data: &[u8],
    offset: usize,
) -> Option<Option<EmfWorldTransform>> {
    let scale_x = read_le_f32(data, offset)?;
    let shear_y = read_le_f32(data, offset.checked_add(4)?)?;
    let shear_x = read_le_f32(data, offset.checked_add(8)?)?;
    let scale_y = read_le_f32(data, offset.checked_add(12)?)?;
    let translate_x = read_le_f32(data, offset.checked_add(16)?)?;
    let translate_y = read_le_f32(data, offset.checked_add(20)?)?;
    let values = [scale_x, shear_y, shear_x, scale_y, translate_x, translate_y];
    if !values.iter().all(|value| value.is_finite()) {
        return Some(None);
    }
    if shear_x != 0.0 || shear_y != 0.0 || scale_x == 0.0 || scale_y == 0.0 {
        return Some(None);
    }
    if scale_x.abs() > 64.0
        || scale_y.abs() > 64.0
        || translate_x.abs() > 1_000_000.0
        || translate_y.abs() > 1_000_000.0
    {
        return Some(None);
    }
    Some(Some(EmfWorldTransform {
        scale_x,
        scale_y,
        translate_x,
        translate_y,
    }))
}

fn parse_passive_emf_miter_limit(data: &[u8]) -> Option<f32> {
    let limit = read_le_f32(data, 0)?;
    if !limit.is_finite() || !(1.0..=256.0).contains(&limit) {
        return None;
    }
    Some(limit)
}

fn multiply_emf_world_transforms(
    left: EmfWorldTransform,
    right: EmfWorldTransform,
) -> Option<EmfWorldTransform> {
    let scale_x = left.scale_x * right.scale_x;
    let scale_y = left.scale_y * right.scale_y;
    let translate_x = left.scale_x * right.translate_x + left.translate_x;
    let translate_y = left.scale_y * right.translate_y + left.translate_y;
    let values = [scale_x, scale_y, translate_x, translate_y];
    if !values.iter().all(|value| value.is_finite())
        || scale_x == 0.0
        || scale_y == 0.0
        || scale_x.abs() > 64.0
        || scale_y.abs() > 64.0
        || translate_x.abs() > 1_000_000.0
        || translate_y.abs() > 1_000_000.0
    {
        return None;
    }
    Some(EmfWorldTransform {
        scale_x,
        scale_y,
        translate_x,
        translate_y,
    })
}

fn is_neutral_emf_color_adjustment(data: &[u8]) -> Option<bool> {
    Some(
        read_le_u16(data, 0)? == 24
            && read_le_u16(data, 2)? == 0
            && read_le_u16(data, 4)? == 0
            && read_le_u16(data, 6)? == 10_000
            && read_le_u16(data, 8)? == 10_000
            && read_le_u16(data, 10)? == 10_000
            && read_le_u16(data, 12)? == 0
            && read_le_u16(data, 14)? == 10_000
            && read_le_i16(data, 16)? == 0
            && read_le_i16(data, 18)? == 0
            && read_le_i16(data, 20)? == 0
            && read_le_i16(data, 22)? == 0,
    )
}

fn parse_emf_setpixelv_rect(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> Option<(f32, f32, f32, f32, Color)> {
    if data.len() < 12 {
        return None;
    }
    let x = read_le_i32(data, 0)?;
    let y = read_le_i32(data, 4)?;
    let color = color_from_colorref(data, 8)?;
    let (x, y) = normalized_emf_point(x, y, header, coordinates);
    let max_x = header.width_px.max(1) as f32;
    let max_y = header.height_px.max(1) as f32;
    Some((
        pixel_rect_start(x, max_x),
        pixel_rect_start(y, max_y),
        pixel_rect_end(x, max_x),
        pixel_rect_end(y, max_y),
        color,
    ))
}

fn parse_emf_offset_clip_region(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> Option<(f32, f32)> {
    if data.len() < 8 {
        return None;
    }
    let x = read_le_i32(data, 0)?;
    let y = read_le_i32(data, 4)?;
    Some((
        map_emf_length_axis(
            x,
            coordinates.window_extent_x,
            coordinates.viewport_extent_x,
            header.width_px,
        ),
        map_emf_length_axis(
            y,
            coordinates.window_extent_y,
            coordinates.viewport_extent_y,
            header.height_px,
        ),
    ))
}

fn is_passive_zero_emf_offset_clip_region(data: &[u8]) -> bool {
    data.len() >= 8
        && matches!(
            (read_le_i32(data, 0), read_le_i32(data, 4)),
            (Some(0), Some(0))
        )
}

pub(in crate::rtf::parser) fn vector_commands_are_only_clip_updates(
    commands: &[StaticImageVectorCommand],
) -> bool {
    !commands.is_empty()
        && commands.iter().all(|command| {
            matches!(
                command,
                StaticImageVectorCommand::ClipRect { .. }
                    | StaticImageVectorCommand::ClipPath { .. }
            )
        })
}

fn vector_command_is_passive_graphics_state(command: &StaticImageVectorCommand) -> bool {
    matches!(command, StaticImageVectorCommand::SetMiterLimit { .. })
}

fn vector_commands_have_replaceable_clip_after_paint(
    commands: &[StaticImageVectorCommand],
) -> bool {
    offsettable_clip_commands_after_paint(commands).is_some()
}

fn replace_emf_clip_scope(
    commands: &mut Vec<StaticImageVectorCommand>,
    replaceable_clip_command_start: &mut Option<usize>,
    clip_scope_command_start: &mut Option<usize>,
) -> Option<()> {
    if let Some(clip_start) = *replaceable_clip_command_start
        && vector_commands_are_only_clip_updates(&commands[clip_start..])
    {
        commands.truncate(clip_start);
        *replaceable_clip_command_start = None;
    } else if clip_scope_command_start.is_some() {
        if commands.len().checked_add(2)? > MAX_PASSIVE_WMF_COMMANDS {
            return None;
        }
        commands.push(StaticImageVectorCommand::RestoreState);
        commands.push(StaticImageVectorCommand::SaveState);
        *clip_scope_command_start = commands.len().checked_sub(1);
        *replaceable_clip_command_start = None;
    } else if let Some(clip_start) = *replaceable_clip_command_start
        && vector_commands_have_replaceable_clip_after_paint(&commands[clip_start..])
    {
        if commands.len().checked_add(3)? > MAX_PASSIVE_WMF_COMMANDS {
            return None;
        }
        commands.insert(clip_start, StaticImageVectorCommand::SaveState);
        commands.push(StaticImageVectorCommand::RestoreState);
        commands.push(StaticImageVectorCommand::SaveState);
        *clip_scope_command_start = commands.len().checked_sub(1);
        *replaceable_clip_command_start = None;
    } else {
        return None;
    }
    Some(())
}

fn initial_vector_clip_commands_then_paint_only(
    commands: &[StaticImageVectorCommand],
) -> Option<Vec<StaticImageVectorCommand>> {
    let mut saw_clip = false;
    let mut saw_paint = false;
    let mut clip_commands = Vec::new();
    for command in commands {
        match command {
            StaticImageVectorCommand::ClipRect { .. }
            | StaticImageVectorCommand::ClipPath { .. }
                if !saw_paint =>
            {
                saw_clip = true;
                clip_commands.push(command.clone());
            }
            StaticImageVectorCommand::ClipRect { .. }
            | StaticImageVectorCommand::ClipPath { .. } => {
                return None;
            }
            StaticImageVectorCommand::SaveState | StaticImageVectorCommand::RestoreState => {
                return None;
            }
            command if vector_command_is_passive_graphics_state(command) => {}
            _ => {
                if !saw_clip {
                    return None;
                }
                saw_paint = true;
            }
        }
    }

    (saw_clip && saw_paint).then_some(clip_commands)
}

pub(in crate::rtf::parser) fn offsettable_clip_commands_after_paint(
    commands: &[StaticImageVectorCommand],
) -> Option<Vec<StaticImageVectorCommand>> {
    initial_vector_clip_commands_then_paint_only(commands)
        .or_else(|| initial_vector_clip_commands_then_paint_then_clip_updates(commands))
}

fn initial_vector_clip_commands_then_paint_then_clip_updates(
    commands: &[StaticImageVectorCommand],
) -> Option<Vec<StaticImageVectorCommand>> {
    let mut saw_initial_clip = false;
    let mut saw_paint = false;
    let mut saw_trailing_clip = false;
    let mut clip_commands = Vec::new();
    for command in commands {
        match command {
            StaticImageVectorCommand::ClipRect { .. }
            | StaticImageVectorCommand::ClipPath { .. }
                if !saw_paint =>
            {
                saw_initial_clip = true;
                clip_commands.push(command.clone());
            }
            StaticImageVectorCommand::ClipRect { .. }
            | StaticImageVectorCommand::ClipPath { .. } => {
                saw_trailing_clip = true;
                clip_commands.push(command.clone());
            }
            StaticImageVectorCommand::SaveState | StaticImageVectorCommand::RestoreState => {
                return None;
            }
            command if vector_command_is_passive_graphics_state(command) => {}
            _ if saw_trailing_clip => return None,
            _ => {
                if !saw_initial_clip {
                    return None;
                }
                saw_paint = true;
            }
        }
    }

    (saw_initial_clip && saw_paint && saw_trailing_clip).then_some(clip_commands)
}

pub(in crate::rtf::parser) fn scoped_vector_clip_commands(
    commands: &[StaticImageVectorCommand],
    scope_start: usize,
) -> Option<Vec<StaticImageVectorCommand>> {
    if !matches!(
        commands.get(scope_start),
        Some(StaticImageVectorCommand::SaveState)
    ) {
        return None;
    }

    let scoped_commands = commands.get(scope_start.checked_add(1)?..)?;
    if vector_commands_are_only_clip_updates(scoped_commands) {
        return Some(scoped_commands.to_vec());
    }
    offsettable_clip_commands_after_paint(scoped_commands)
}

pub(in crate::rtf::parser) fn emf_commands_are_unpainted_clip_scope(
    commands: &[StaticImageVectorCommand],
) -> bool {
    commands.iter().all(|command| {
        matches!(
            command,
            StaticImageVectorCommand::ClipRect { .. }
                | StaticImageVectorCommand::ClipPath { .. }
                | StaticImageVectorCommand::SetMiterLimit { .. }
                | StaticImageVectorCommand::SaveState
                | StaticImageVectorCommand::RestoreState
        )
    })
}

fn parse_emf_passive_raster_transfer(
    data: &[u8],
    min_data_len: usize,
    raster_op_offset: usize,
    destination_position_offset: usize,
    destination_extent_offset: usize,
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
    selected_fill_color: Option<Color>,
    blank_destination: bool,
) -> Option<(f32, f32, f32, f32, Color)> {
    let raster_op = read_le_u32(data, raster_op_offset)?;
    let fill_color = match raster_op {
        WMF_BLACKNESS_RASTER_OP => Color {
            red: 0,
            green: 0,
            blue: 0,
        },
        WMF_WHITENESS_RASTER_OP => Color {
            red: 255,
            green: 255,
            blue: 255,
        },
        WMF_PATCOPY_RASTER_OP => selected_fill_color?,
        WMF_PATPAINT_RASTER_OP if selected_fill_color.is_some_and(is_white_color) => Color {
            red: 255,
            green: 255,
            blue: 255,
        },
        WMF_SRCERASE_RASTER_OP | WMF_NOTSRCERASE_RASTER_OP if blank_destination => Color {
            red: 0,
            green: 0,
            blue: 0,
        },
        WMF_DSTINVERT_RASTER_OP if blank_destination => Color {
            red: 0,
            green: 0,
            blue: 0,
        },
        WMF_MERGEPAINT_RASTER_OP | WMF_SRCPAINT_RASTER_OP | WMF_PATPAINT_RASTER_OP
            if blank_destination =>
        {
            Color {
                red: 255,
                green: 255,
                blue: 255,
            }
        }
        WMF_MERGECOPY_RASTER_OP if selected_fill_color == Some(Color::default()) => Color {
            red: 0,
            green: 0,
            blue: 0,
        },
        WMF_NOTPATCOPY_RASTER_OP => inverted_color(selected_fill_color?),
        WMF_PATINVERT_RASTER_OP if blank_destination => inverted_color(selected_fill_color?),
        _ => return None,
    };
    let (left, top, right, bottom) = parse_emf_raster_destination_bounds(
        data,
        min_data_len,
        destination_position_offset,
        destination_extent_offset,
        header,
        coordinates,
    )?;
    Some((left, top, right, bottom, fill_color))
}

fn parse_emf_raster_destination_bounds(
    data: &[u8],
    min_data_len: usize,
    destination_position_offset: usize,
    destination_extent_offset: usize,
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> Option<(f32, f32, f32, f32)> {
    if data.len() < min_data_len {
        return None;
    }
    let x = read_le_i32(data, destination_position_offset)?;
    let y = read_le_i32(data, destination_position_offset + 4)?;
    let width = i32::try_from(
        read_le_i32(data, destination_extent_offset)?
            .unsigned_abs()
            .max(1),
    )
    .unwrap_or(i32::MAX);
    let height = i32::try_from(
        read_le_i32(data, destination_extent_offset + 4)?
            .unsigned_abs()
            .max(1),
    )
    .unwrap_or(i32::MAX);
    let left_top = normalized_emf_point(x, y, header, coordinates);
    let right_bottom = normalized_emf_point(
        x.saturating_add(width),
        y.saturating_add(height),
        header,
        coordinates,
    );
    Some((
        left_top.0.min(right_bottom.0),
        left_top.1.min(right_bottom.1),
        left_top.0.max(right_bottom.0),
        left_top.1.max(right_bottom.1),
    ))
}

fn parse_emf_bitmap_blt_srcopy(
    data: &[u8],
    record_type: u32,
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
    selected_fill_color: Option<Color>,
    blank_destination: bool,
) -> Option<PassiveSourceRasterCommand> {
    const EMR_BITBLT: u32 = 76;
    const EMR_STRETCHBLT: u32 = 77;
    const DIB_RGB_COLORS: u32 = 0;

    if data.len() < 92 {
        return None;
    }
    let transfer_mode = passive_source_raster_transfer_mode(
        read_le_u32(data, 32)?,
        selected_fill_color,
        blank_destination,
    )?;

    let x_dest = read_le_i32(data, 16)?;
    let y_dest = read_le_i32(data, 20)?;
    let cx_dest = read_le_i32(data, 24)?;
    let cy_dest = read_le_i32(data, 28)?;

    if cx_dest == 0 || cy_dest == 0 {
        return None;
    }

    let width = i32::try_from(cx_dest.unsigned_abs().max(1)).unwrap_or(i32::MAX);
    let height = i32::try_from(cy_dest.unsigned_abs().max(1)).unwrap_or(i32::MAX);
    let left_top = normalized_emf_point(x_dest, y_dest, header, coordinates);
    let right_bottom = normalized_emf_point(
        x_dest.saturating_add(width),
        y_dest.saturating_add(height),
        header,
        coordinates,
    );
    let left = left_top.0.min(right_bottom.0);
    let top = left_top.1.min(right_bottom.1);
    let right = left_top.0.max(right_bottom.0);
    let bottom = left_top.1.max(right_bottom.1);
    if let Some(command) = source_ignoring_passive_source_raster_command(
        (left, top, right, bottom),
        read_le_u32(data, 32)?,
        selected_fill_color,
        blank_destination,
    ) {
        return Some(command);
    }
    let x_src = read_le_i32(data, 36)?;
    let y_src = read_le_i32(data, 40)?;
    let usage_src = read_le_u32(data, 72)?;
    let off_bmi_src = emf_record_offset_to_data_offset(read_le_u32(data, 76)?)?;
    let cb_bmi_src = usize::try_from(read_le_u32(data, 80)?).ok()?;
    let off_bits_src = emf_record_offset_to_data_offset(read_le_u32(data, 84)?)?;
    let cb_bits_src = usize::try_from(read_le_u32(data, 88)?).ok()?;
    let (cx_src, cy_src) = match record_type {
        EMR_BITBLT => (
            i32::try_from(cx_dest.unsigned_abs()).ok()?,
            i32::try_from(cy_dest.unsigned_abs()).ok()?,
        ),
        EMR_STRETCHBLT => {
            if data.len() < 100 {
                return None;
            }
            (read_le_i32(data, 92)?, read_le_i32(data, 96)?)
        }
        _ => return None,
    };
    if usage_src != DIB_RGB_COLORS || cx_src <= 0 || cy_src <= 0 {
        return None;
    }
    let dib_bytes =
        emf_stretchdibits_dib_bytes(data, off_bmi_src, cb_bmi_src, off_bits_src, cb_bits_src)?;
    let transfer = parse_vector_raster_dib_image(&dib_bytes, x_src, y_src, cx_src, cy_src, false)
        .and_then(|image| apply_passive_source_raster_transfer_mode(image, transfer_mode));
    if let Some(transfer) = transfer {
        return Some(passive_source_raster_command(
            left, top, right, bottom, transfer,
        ));
    }
    if let Some(command) = clipped_compressed_jpeg_source_raster_command(
        &dib_bytes,
        x_src,
        y_src,
        cx_src,
        cy_src,
        left,
        top,
        right,
        bottom,
        transfer_mode,
    ) {
        return Some(command);
    }

    None
}

fn parse_emf_stretchdibits_srcopy(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
    selected_fill_color: Option<Color>,
    blank_destination: bool,
) -> Option<PassiveSourceRasterCommand> {
    const DIB_RGB_COLORS: u32 = 0;

    if data.len() < 72 {
        return None;
    }
    let transfer_mode = passive_source_raster_transfer_mode(
        read_le_u32(data, 60)?,
        selected_fill_color,
        blank_destination,
    )?;
    let x_dest = read_le_i32(data, 16)?;
    let y_dest = read_le_i32(data, 20)?;
    let cx_dest = read_le_i32(data, 64)?;
    let cy_dest = read_le_i32(data, 68)?;

    if cx_dest == 0 || cy_dest == 0 {
        return None;
    }

    let width = i32::try_from(cx_dest.unsigned_abs().max(1)).unwrap_or(i32::MAX);
    let height = i32::try_from(cy_dest.unsigned_abs().max(1)).unwrap_or(i32::MAX);
    let left_top = normalized_emf_point(x_dest, y_dest, header, coordinates);
    let right_bottom = normalized_emf_point(
        x_dest.saturating_add(width),
        y_dest.saturating_add(height),
        header,
        coordinates,
    );
    let left = left_top.0.min(right_bottom.0);
    let top = left_top.1.min(right_bottom.1);
    let right = left_top.0.max(right_bottom.0);
    let bottom = left_top.1.max(right_bottom.1);
    if let Some(command) = source_ignoring_passive_source_raster_command(
        (left, top, right, bottom),
        read_le_u32(data, 60)?,
        selected_fill_color,
        blank_destination,
    ) {
        return Some(command);
    }
    let x_src = read_le_i32(data, 24)?;
    let y_src = read_le_i32(data, 28)?;
    let cx_src = read_le_i32(data, 32)?;
    let cy_src = read_le_i32(data, 36)?;
    let off_bmi_src = emf_record_offset_to_data_offset(read_le_u32(data, 40)?)?;
    let cb_bmi_src = usize::try_from(read_le_u32(data, 44)?).ok()?;
    let off_bits_src = emf_record_offset_to_data_offset(read_le_u32(data, 48)?)?;
    let cb_bits_src = usize::try_from(read_le_u32(data, 52)?).ok()?;
    let usage_src = read_le_u32(data, 56)?;
    if usage_src != DIB_RGB_COLORS || cx_src <= 0 || cy_src <= 0 {
        return None;
    }
    let dib_bytes =
        emf_stretchdibits_dib_bytes(data, off_bmi_src, cb_bmi_src, off_bits_src, cb_bits_src)?;
    let transfer = parse_vector_raster_dib_image(&dib_bytes, x_src, y_src, cx_src, cy_src, false)
        .and_then(|image| apply_passive_source_raster_transfer_mode(image, transfer_mode));
    if let Some(transfer) = transfer {
        return Some(passive_source_raster_command(
            left, top, right, bottom, transfer,
        ));
    }
    if let Some(command) = clipped_compressed_jpeg_source_raster_command(
        &dib_bytes,
        x_src,
        y_src,
        cx_src,
        cy_src,
        left,
        top,
        right,
        bottom,
        transfer_mode,
    ) {
        return Some(command);
    }

    None
}

fn parse_emf_setdibitstodevice_srcopy(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> Option<PassiveSourceRasterCommand> {
    const DIB_RGB_COLORS: u32 = 0;

    if data.len() < 68 {
        return None;
    }

    let x_dest = read_le_i32(data, 16)?;
    let y_dest = read_le_i32(data, 20)?;
    let x_src = read_le_i32(data, 24)?;
    let y_src = read_le_i32(data, 28)?;
    let cx_src = read_le_i32(data, 32)?;
    let cy_src = read_le_i32(data, 36)?;
    let off_bmi_src = emf_record_offset_to_data_offset(read_le_u32(data, 40)?)?;
    let cb_bmi_src = usize::try_from(read_le_u32(data, 44)?).ok()?;
    let off_bits_src = emf_record_offset_to_data_offset(read_le_u32(data, 48)?)?;
    let cb_bits_src = usize::try_from(read_le_u32(data, 52)?).ok()?;
    let usage_src = read_le_u32(data, 56)?;
    let start_scan = read_le_u32(data, 60)?;
    let scan_count = read_le_u32(data, 64)?;

    if cx_src <= 0 || cy_src <= 0 || usage_src != DIB_RGB_COLORS {
        return None;
    }
    if scan_count > cy_src.unsigned_abs() {
        return None;
    }

    let dib_bytes =
        emf_stretchdibits_dib_bytes(data, off_bmi_src, cb_bmi_src, off_bits_src, cb_bits_src)?;
    let width = i32::try_from(cx_src.unsigned_abs().max(1)).unwrap_or(i32::MAX);
    let height = i32::try_from(scan_count.max(1)).unwrap_or(i32::MAX);
    let left_top = normalized_emf_point(x_dest, y_dest, header, coordinates);
    let right_bottom = normalized_emf_point(
        x_dest.saturating_add(width),
        y_dest.saturating_add(height),
        header,
        coordinates,
    );
    let left = left_top.0.min(right_bottom.0);
    let top = left_top.1.min(right_bottom.1);
    let right = left_top.0.max(right_bottom.0);
    let bottom = left_top.1.max(right_bottom.1);
    if let Some(image) =
        parse_vector_raster_dib_scan_lines(&dib_bytes, x_src, y_src, cx_src, start_scan, scan_count)
    {
        return Some(PassiveSourceRasterCommand::Draw(
            StaticImageVectorCommand::RasterImage {
                left,
                top,
                right,
                bottom,
                image: StaticImageVectorRaster {
                    format: image.format,
                    width_px: image.width_px,
                    height_px: image.height_px,
                    bytes: image.bytes,
                    palette: image.palette,
                    alpha_mask: image.alpha_mask,
                    tone_adjustment: None,
                },
            },
        ));
    }

    let compressed = parse_compressed_dib_image_data(&dib_bytes, MAX_PASSIVE_VECTOR_RASTER_PIXELS)?;
    let scan_bottom = start_scan.checked_add(scan_count)?;
    if compressed.format != ImageFormat::Jpeg
        || scan_count == 0
        || scan_bottom > compressed.height_px
    {
        return None;
    }
    let scan_top_y = if compressed.top_down {
        start_scan
    } else {
        compressed.height_px.checked_sub(scan_bottom)?
    };
    let source_y = i32::try_from(scan_top_y).ok()?.checked_add(y_src)?;
    clipped_compressed_jpeg_source_raster_command(
        &dib_bytes,
        x_src,
        source_y,
        cx_src,
        i32::try_from(scan_count).ok()?,
        left,
        top,
        right,
        bottom,
        PassiveSourceRasterTransferMode::Copy,
    )
}

fn parse_emf_alphablend_passive(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> Option<StaticImageVectorCommand> {
    const AC_SRC_OVER: u8 = 0;
    const AC_SRC_ALPHA: u8 = 1;
    const DIB_RGB_COLORS: u32 = 0;

    if data.len() < 100 {
        return None;
    }

    let blend_op = *data.get(32)?;
    let blend_flags = *data.get(33)?;
    let source_constant_alpha = *data.get(34)?;
    let alpha_format = *data.get(35)?;
    if blend_op != AC_SRC_OVER || blend_flags != 0 || !matches!(alpha_format, 0 | AC_SRC_ALPHA) {
        return None;
    }

    let x_dest = read_le_i32(data, 16)?;
    let y_dest = read_le_i32(data, 20)?;
    let cx_dest = read_le_i32(data, 24)?;
    let cy_dest = read_le_i32(data, 28)?;
    let x_src = read_le_i32(data, 36)?;
    let y_src = read_le_i32(data, 40)?;
    let usage_src = read_le_u32(data, 72)?;
    let off_bmi_src = emf_record_offset_to_data_offset(read_le_u32(data, 76)?)?;
    let cb_bmi_src = usize::try_from(read_le_u32(data, 80)?).ok()?;
    let off_bits_src = emf_record_offset_to_data_offset(read_le_u32(data, 84)?)?;
    let cb_bits_src = usize::try_from(read_le_u32(data, 88)?).ok()?;
    let cx_src = read_le_i32(data, 92)?;
    let cy_src = read_le_i32(data, 96)?;

    if usage_src != DIB_RGB_COLORS || cx_dest == 0 || cy_dest == 0 || cx_src <= 0 || cy_src <= 0 {
        return None;
    }

    let dib_bytes =
        emf_stretchdibits_dib_bytes(data, off_bmi_src, cb_bmi_src, off_bits_src, cb_bits_src)?;
    let mut image = parse_vector_raster_dib_image(
        &dib_bytes,
        x_src,
        y_src,
        cx_src,
        cy_src,
        alpha_format == AC_SRC_ALPHA,
    )?;
    image.alpha_mask = alphablend_alpha_mask(
        image.width_px,
        image.height_px,
        source_constant_alpha,
        alpha_format == AC_SRC_ALPHA,
        image.alpha_mask,
    )?;

    let width = i32::try_from(cx_dest.unsigned_abs().max(1)).unwrap_or(i32::MAX);
    let height = i32::try_from(cy_dest.unsigned_abs().max(1)).unwrap_or(i32::MAX);
    let left_top = normalized_emf_point(x_dest, y_dest, header, coordinates);
    let right_bottom = normalized_emf_point(
        x_dest.saturating_add(width),
        y_dest.saturating_add(height),
        header,
        coordinates,
    );
    Some(StaticImageVectorCommand::RasterImage {
        left: left_top.0.min(right_bottom.0),
        top: left_top.1.min(right_bottom.1),
        right: left_top.0.max(right_bottom.0),
        bottom: left_top.1.max(right_bottom.1),
        image: StaticImageVectorRaster {
            format: image.format,
            width_px: image.width_px,
            height_px: image.height_px,
            bytes: image.bytes,
            palette: image.palette,
            alpha_mask: image.alpha_mask,
            tone_adjustment: None,
        },
    })
}

fn is_passive_noop_emf_alphablend(data: &[u8]) -> bool {
    const AC_SRC_OVER: u8 = 0;
    const AC_SRC_ALPHA: u8 = 1;

    let Some((&blend_op, rest)) = data.get(32..).and_then(|slice| slice.split_first()) else {
        return false;
    };
    let Some((&blend_flags, rest)) = rest.split_first() else {
        return false;
    };
    let Some((&source_constant_alpha, rest)) = rest.split_first() else {
        return false;
    };
    let Some((&alpha_format, _)) = rest.split_first() else {
        return false;
    };

    blend_op == AC_SRC_OVER
        && blend_flags == 0
        && source_constant_alpha == 0
        && matches!(alpha_format, 0 | AC_SRC_ALPHA)
}

fn is_passive_invisible_emf_bitmap_transfer(data: &[u8]) -> bool {
    is_passive_invisible_emf_raster_transfer(data, 24)
}

fn is_passive_noop_emf_bitmap_transfer_source_extent(data: &[u8]) -> bool {
    data.len() >= 100
        && matches!(
            (read_le_i32(data, 92), read_le_i32(data, 96)),
            (Some(0), _) | (_, Some(0))
        )
}

fn is_passive_invisible_emf_raster_transfer(data: &[u8], destination_extent_offset: usize) -> bool {
    let Some(width_offset) = destination_extent_offset.checked_add(4) else {
        return false;
    };
    data.len() >= width_offset.checked_add(4).unwrap_or(usize::MAX)
        && matches!(
            (
                read_le_i32(data, destination_extent_offset),
                read_le_i32(data, width_offset)
            ),
            (Some(0), _) | (_, Some(0))
        )
}

fn is_passive_noop_emf_setdibitstodevice(data: &[u8]) -> bool {
    data.len() >= 68
        && matches!(
            (read_le_i32(data, 32), read_le_u32(data, 64)),
            (Some(0), _) | (_, Some(0))
        )
}

fn is_passive_noop_emf_stretchblt_source_extent(data: &[u8]) -> bool {
    data.len() >= 100
        && matches!(
            (read_le_i32(data, 92), read_le_i32(data, 96)),
            (Some(0), _) | (_, Some(0))
        )
}

fn is_passive_noop_emf_stretchdibits_source_extent(data: &[u8]) -> bool {
    data.len() >= 40
        && matches!(
            (read_le_i32(data, 32), read_le_i32(data, 36)),
            (Some(0), _) | (_, Some(0))
        )
}

fn parse_emf_transparentblt_keyed(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> Option<PassiveSourceRasterCommand> {
    const DIB_RGB_COLORS: u32 = 0;

    if data.len() < 100 {
        return None;
    }

    let x_dest = read_le_i32(data, 16)?;
    let y_dest = read_le_i32(data, 20)?;
    let cx_dest = read_le_i32(data, 24)?;
    let cy_dest = read_le_i32(data, 28)?;
    let transparent = color_from_colorref(data, 32)?;
    let x_src = read_le_i32(data, 36)?;
    let y_src = read_le_i32(data, 40)?;
    let usage_src = read_le_u32(data, 72)?;
    let off_bmi_src = emf_record_offset_to_data_offset(read_le_u32(data, 76)?)?;
    let cb_bmi_src = usize::try_from(read_le_u32(data, 80)?).ok()?;
    let off_bits_src = emf_record_offset_to_data_offset(read_le_u32(data, 84)?)?;
    let cb_bits_src = usize::try_from(read_le_u32(data, 88)?).ok()?;
    let cx_src = read_le_i32(data, 92)?;
    let cy_src = read_le_i32(data, 96)?;

    if usage_src != DIB_RGB_COLORS || cx_dest == 0 || cy_dest == 0 || cx_src <= 0 || cy_src <= 0 {
        return None;
    }

    let dib_bytes =
        emf_stretchdibits_dib_bytes(data, off_bmi_src, cb_bmi_src, off_bits_src, cb_bits_src)?;
    let width = i32::try_from(cx_dest.unsigned_abs().max(1)).unwrap_or(i32::MAX);
    let height = i32::try_from(cy_dest.unsigned_abs().max(1)).unwrap_or(i32::MAX);
    let left_top = normalized_emf_point(x_dest, y_dest, header, coordinates);
    let right_bottom = normalized_emf_point(
        x_dest.saturating_add(width),
        y_dest.saturating_add(height),
        header,
        coordinates,
    );
    let left = left_top.0.min(right_bottom.0);
    let top = left_top.1.min(right_bottom.1);
    let right = left_top.0.max(right_bottom.0);
    let bottom = left_top.1.max(right_bottom.1);

    if let Some(mut image) =
        parse_vector_raster_dib_image(&dib_bytes, x_src, y_src, cx_src, cy_src, false)
    {
        image.alpha_mask = keyed_vector_raster_alpha_mask(
            image.format,
            &image.bytes,
            image.width_px,
            image.height_px,
            transparent,
            &image.palette,
            image.alpha_mask,
        )?;
        return Some(PassiveSourceRasterCommand::Draw(
            StaticImageVectorCommand::RasterImage {
                left,
                top,
                right,
                bottom,
                image: StaticImageVectorRaster {
                    format: image.format,
                    width_px: image.width_px,
                    height_px: image.height_px,
                    bytes: image.bytes,
                    palette: image.palette,
                    alpha_mask: image.alpha_mask,
                    tone_adjustment: None,
                },
            },
        ));
    }

    clipped_compressed_jpeg_transparentblt_command(
        &dib_bytes,
        x_src,
        y_src,
        cx_src,
        cy_src,
        left,
        top,
        right,
        bottom,
        transparent,
    )
}

#[allow(clippy::too_many_arguments)]
fn clipped_compressed_jpeg_transparentblt_command(
    bytes: &[u8],
    source_x: i32,
    source_y: i32,
    source_width: i32,
    source_height: i32,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    transparent: Color,
) -> Option<PassiveSourceRasterCommand> {
    if source_x < 0 || source_y < 0 || source_width <= 0 || source_height <= 0 {
        return None;
    }
    let mut image = parse_compressed_dib_image_data(bytes, MAX_PASSIVE_VECTOR_RASTER_PIXELS)?;
    if !matches!(
        image.format,
        ImageFormat::Jpeg
            | ImageFormat::JpegGrayscale
            | ImageFormat::JpegCmyk
            | ImageFormat::JpegInverted
            | ImageFormat::JpegGrayscaleInverted
            | ImageFormat::JpegCmykInverted
    ) {
        return None;
    }
    let source_x = u32::try_from(source_x).ok()?;
    let source_y = u32::try_from(source_y).ok()?;
    let source_width = u32::try_from(source_width).ok()?;
    let source_height = u32::try_from(source_height).ok()?;
    let source_right = source_x.checked_add(source_width)?;
    let source_bottom = source_y.checked_add(source_height)?;
    if source_right > image.width_px || source_bottom > image.height_px {
        return None;
    }
    if source_x == 0
        && source_y == 0
        && source_width == image.width_px
        && source_height == image.height_px
    {
        return None;
    }

    image.alpha_mask = keyed_vector_raster_alpha_mask(
        image.format,
        &image.bytes,
        image.width_px,
        image.height_px,
        transparent,
        &image.palette,
        image.alpha_mask,
    )?;

    let dest_width = right - left;
    let dest_height = bottom - top;
    if dest_width <= 0.0 || dest_height <= 0.0 {
        return None;
    }
    let scale_x = dest_width / source_width as f32;
    let scale_y = dest_height / source_height as f32;
    let full_left = left - (source_x as f32 * scale_x);
    let full_top = top - (source_y as f32 * scale_y);
    let full_right = full_left + (image.width_px as f32 * scale_x);
    let full_bottom = full_top + (image.height_px as f32 * scale_y);

    Some(PassiveSourceRasterCommand::DrawMany(vec![
        StaticImageVectorCommand::SaveState,
        StaticImageVectorCommand::ClipRect {
            left,
            top,
            right,
            bottom,
        },
        StaticImageVectorCommand::RasterImage {
            left: full_left,
            top: full_top,
            right: full_right,
            bottom: full_bottom,
            image: StaticImageVectorRaster {
                format: image.format,
                width_px: image.width_px,
                height_px: image.height_px,
                bytes: image.bytes,
                palette: image.palette,
                alpha_mask: image.alpha_mask,
                tone_adjustment: None,
            },
        },
        StaticImageVectorCommand::RestoreState,
    ]))
}

pub(in crate::rtf::parser) fn keyed_vector_raster_alpha_mask(
    format: ImageFormat,
    bytes: &[u8],
    width_px: u32,
    height_px: u32,
    transparent: Color,
    palette: &[u8],
    existing_mask: Option<StaticImageAlphaMask>,
) -> Option<Option<StaticImageAlphaMask>> {
    match format {
        ImageFormat::Rgb8 => keyed_rgb_alpha_mask(bytes, width_px, height_px, transparent),
        ImageFormat::Png => combine_keyed_alpha_mask(
            width_px,
            height_px,
            existing_mask,
            keyed_png_rgb_alpha_mask(bytes, width_px, height_px, transparent)?,
        ),
        ImageFormat::PngIndexed => combine_keyed_alpha_mask(
            width_px,
            height_px,
            existing_mask,
            keyed_png_indexed_alpha_mask(bytes, width_px, height_px, transparent, palette)?,
        ),
        ImageFormat::PngGrayscale => combine_keyed_alpha_mask(
            width_px,
            height_px,
            existing_mask,
            keyed_png_grayscale_alpha_mask(bytes, width_px, height_px, transparent)?,
        ),
        ImageFormat::Jpeg
        | ImageFormat::JpegGrayscale
        | ImageFormat::JpegCmyk
        | ImageFormat::JpegInverted
        | ImageFormat::JpegGrayscaleInverted
        | ImageFormat::JpegCmykInverted => {
            let keyed = keyed_jpeg_alpha_mask(bytes, width_px, height_px, format, transparent)
                .unwrap_or(None);
            combine_keyed_alpha_mask(width_px, height_px, existing_mask, keyed)
        }
        _ => None,
    }
}

fn keyed_jpeg_alpha_mask(
    bytes: &[u8],
    width_px: u32,
    height_px: u32,
    format: ImageFormat,
    transparent: Color,
) -> Option<Option<StaticImageAlphaMask>> {
    let rgb = decode_bounded_jpeg_to_rgb8(
        bytes,
        width_px,
        height_px,
        format,
        MAX_EXACT_JPEG_COLOR_MODE_BYTES,
        MAX_EXACT_JPEG_COLOR_MODE_PIXELS,
    )?;
    keyed_rgb_alpha_mask(&rgb, width_px, height_px, transparent)
}

fn keyed_rgb_alpha_mask(
    rgb: &[u8],
    width_px: u32,
    height_px: u32,
    transparent: Color,
) -> Option<Option<StaticImageAlphaMask>> {
    let width = usize::try_from(width_px).ok()?;
    let height = usize::try_from(height_px).ok()?;
    let pixels = width.checked_mul(height)?;
    if rgb.len() != pixels.checked_mul(3)? {
        return None;
    }

    let row_len = width.checked_add(1)?;
    let mut mask = Vec::with_capacity(row_len.checked_mul(height)?);
    let mut has_transparent_pixel = false;
    for row in rgb.chunks_exact(width.checked_mul(3)?) {
        mask.push(0);
        for pixel in row.chunks_exact(3) {
            let is_transparent = pixel[0] == transparent.red
                && pixel[1] == transparent.green
                && pixel[2] == transparent.blue;
            has_transparent_pixel |= is_transparent;
            mask.push(if is_transparent { 0 } else { u8::MAX });
        }
    }
    if !has_transparent_pixel {
        return Some(None);
    }
    Some(Some(StaticImageAlphaMask {
        bytes: miniz_oxide::deflate::compress_to_vec_zlib(&mask, 6),
    }))
}

fn keyed_png_rgb_alpha_mask(
    idat: &[u8],
    width_px: u32,
    height_px: u32,
    transparent: Color,
) -> Option<Option<StaticImageAlphaMask>> {
    let (width, height, row_len, mut scanlines) =
        unfiltered_png_scanlines(idat, width_px, height_px, 3)?;
    let output_row_len = width.checked_add(1)?;
    let mut mask = Vec::with_capacity(output_row_len.checked_mul(height)?);
    let mut has_transparent_pixel = false;
    for row in scanlines.chunks_exact_mut(row_len) {
        mask.push(0);
        for pixel in row[1..].chunks_exact(3) {
            let is_transparent = pixel[0] == transparent.red
                && pixel[1] == transparent.green
                && pixel[2] == transparent.blue;
            has_transparent_pixel |= is_transparent;
            mask.push(if is_transparent { 0 } else { u8::MAX });
        }
    }
    if !has_transparent_pixel {
        return Some(None);
    }
    Some(Some(StaticImageAlphaMask {
        bytes: miniz_oxide::deflate::compress_to_vec_zlib(&mask, 6),
    }))
}

fn keyed_png_indexed_alpha_mask(
    idat: &[u8],
    width_px: u32,
    height_px: u32,
    transparent: Color,
    palette: &[u8],
) -> Option<Option<StaticImageAlphaMask>> {
    let (width, height, row_len, mut scanlines) =
        unfiltered_png_scanlines(idat, width_px, height_px, 1)?;
    let output_row_len = width.checked_add(1)?;
    let mut mask = Vec::with_capacity(output_row_len.checked_mul(height)?);
    let mut has_transparent_pixel = false;
    for row in scanlines.chunks_exact_mut(row_len) {
        mask.push(0);
        for sample in &row[1..] {
            let palette_start = usize::from(*sample).checked_mul(3)?;
            let entry = palette.get(palette_start..palette_start.checked_add(3)?)?;
            let is_transparent = entry[0] == transparent.red
                && entry[1] == transparent.green
                && entry[2] == transparent.blue;
            has_transparent_pixel |= is_transparent;
            mask.push(if is_transparent { 0 } else { u8::MAX });
        }
    }
    if !has_transparent_pixel {
        return Some(None);
    }
    Some(Some(StaticImageAlphaMask {
        bytes: miniz_oxide::deflate::compress_to_vec_zlib(&mask, 6),
    }))
}

fn keyed_png_grayscale_alpha_mask(
    idat: &[u8],
    width_px: u32,
    height_px: u32,
    transparent: Color,
) -> Option<Option<StaticImageAlphaMask>> {
    if transparent.red != transparent.green || transparent.red != transparent.blue {
        return Some(None);
    }
    let (width, height, row_len, mut scanlines) =
        unfiltered_png_scanlines(idat, width_px, height_px, 1)?;
    let output_row_len = width.checked_add(1)?;
    let mut mask = Vec::with_capacity(output_row_len.checked_mul(height)?);
    let mut has_transparent_pixel = false;
    for row in scanlines.chunks_exact_mut(row_len) {
        mask.push(0);
        for sample in &row[1..] {
            let is_transparent = *sample == transparent.red;
            has_transparent_pixel |= is_transparent;
            mask.push(if is_transparent { 0 } else { u8::MAX });
        }
    }
    if !has_transparent_pixel {
        return Some(None);
    }
    Some(Some(StaticImageAlphaMask {
        bytes: miniz_oxide::deflate::compress_to_vec_zlib(&mask, 6),
    }))
}

fn combine_keyed_alpha_mask(
    width_px: u32,
    height_px: u32,
    existing_mask: Option<StaticImageAlphaMask>,
    keyed_mask: Option<StaticImageAlphaMask>,
) -> Option<Option<StaticImageAlphaMask>> {
    match (existing_mask, keyed_mask) {
        (None, keyed) => Some(keyed),
        (Some(existing), None) => Some(Some(existing)),
        (Some(existing), Some(keyed)) => {
            let width = usize::try_from(width_px).ok()?;
            let height = usize::try_from(height_px).ok()?;
            let row_len = width.checked_add(1)?;
            let expected_len = row_len.checked_mul(height)?;
            let mut existing_bytes = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(
                &existing.bytes,
                expected_len,
            )
            .ok()?;
            let keyed_bytes =
                miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&keyed.bytes, expected_len)
                    .ok()?;
            if existing_bytes.len() != expected_len || keyed_bytes.len() != expected_len {
                return None;
            }
            for (existing_row, keyed_row) in existing_bytes
                .chunks_exact_mut(row_len)
                .zip(keyed_bytes.chunks_exact(row_len))
            {
                if existing_row[0] != 0 || keyed_row[0] != 0 {
                    return None;
                }
                for (existing_alpha, keyed_alpha) in
                    existing_row[1..].iter_mut().zip(&keyed_row[1..])
                {
                    match *keyed_alpha {
                        0 => *existing_alpha = 0,
                        u8::MAX => {}
                        _ => return None,
                    }
                }
            }
            Some(Some(StaticImageAlphaMask {
                bytes: miniz_oxide::deflate::compress_to_vec_zlib(&existing_bytes, 6),
            }))
        }
    }
}

fn alphablend_alpha_mask(
    width_px: u32,
    height_px: u32,
    constant_alpha: u8,
    uses_source_alpha: bool,
    existing_mask: Option<StaticImageAlphaMask>,
) -> Option<Option<StaticImageAlphaMask>> {
    if uses_source_alpha {
        let mask = existing_mask?;
        if constant_alpha == u8::MAX {
            return Some(Some(mask));
        }
        return multiply_alpha_mask(width_px, height_px, constant_alpha, mask).map(Some);
    }

    if constant_alpha == u8::MAX {
        return Some(None);
    }
    constant_alpha_mask(width_px, height_px, constant_alpha).map(Some)
}

fn constant_alpha_mask(width_px: u32, height_px: u32, alpha: u8) -> Option<StaticImageAlphaMask> {
    let width = usize::try_from(width_px).ok()?;
    let height = usize::try_from(height_px).ok()?;
    let row_len = width.checked_add(1)?;
    let mask_len = row_len.checked_mul(height)?;
    let mut mask = Vec::with_capacity(mask_len);
    for _ in 0..height {
        mask.push(0);
        mask.resize(mask.len().checked_add(width)?, alpha);
    }
    Some(StaticImageAlphaMask {
        bytes: miniz_oxide::deflate::compress_to_vec_zlib(&mask, 6),
    })
}

pub(in crate::rtf::parser) fn alpha_samples_to_mask(
    alpha: &[u8],
    width_px: u32,
    height_px: u32,
) -> Option<StaticImageAlphaMask> {
    let width = usize::try_from(width_px).ok()?;
    let height = usize::try_from(height_px).ok()?;
    if alpha.len() != width.checked_mul(height)? {
        return None;
    }

    let row_len = width.checked_add(1)?;
    let mut mask = Vec::with_capacity(row_len.checked_mul(height)?);
    for row in alpha.chunks_exact(width) {
        mask.push(0);
        mask.extend_from_slice(row);
    }
    Some(StaticImageAlphaMask {
        bytes: miniz_oxide::deflate::compress_to_vec_zlib(&mask, 6),
    })
}

fn multiply_alpha_mask(
    width_px: u32,
    height_px: u32,
    constant_alpha: u8,
    mask: StaticImageAlphaMask,
) -> Option<StaticImageAlphaMask> {
    let width = usize::try_from(width_px).ok()?;
    let height = usize::try_from(height_px).ok()?;
    let row_len = width.checked_add(1)?;
    let expected_len = row_len.checked_mul(height)?;
    let mut bytes =
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&mask.bytes, expected_len).ok()?;
    if bytes.len() != expected_len {
        return None;
    }
    for row in bytes.chunks_exact_mut(row_len) {
        if row[0] != 0 {
            return None;
        }
        for alpha in &mut row[1..] {
            let multiplied = (u16::from(*alpha) * u16::from(constant_alpha) + 127) / 255;
            *alpha = u8::try_from(multiplied).ok()?;
        }
    }
    Some(StaticImageAlphaMask {
        bytes: miniz_oxide::deflate::compress_to_vec_zlib(&bytes, 6),
    })
}

fn emf_record_offset_to_data_offset(offset: u32) -> Option<usize> {
    usize::try_from(offset).ok()?.checked_sub(8)
}

fn emf_stretchdibits_dib_bytes(
    data: &[u8],
    off_bmi_src: usize,
    cb_bmi_src: usize,
    off_bits_src: usize,
    cb_bits_src: usize,
) -> Option<Vec<u8>> {
    let bmi_end = off_bmi_src.checked_add(cb_bmi_src)?;
    let bits_end = off_bits_src.checked_add(cb_bits_src)?;
    if bmi_end > data.len() || bits_end > data.len() || cb_bmi_src == 0 || cb_bits_src == 0 {
        return None;
    }
    if off_bmi_src == off_bits_src && cb_bmi_src == cb_bits_src {
        return Some(data.get(off_bmi_src..bmi_end)?.to_vec());
    }
    if bmi_end > off_bits_src {
        return None;
    }
    let mut dib = Vec::with_capacity(cb_bmi_src.checked_add(cb_bits_src)?);
    dib.extend_from_slice(data.get(off_bmi_src..bmi_end)?);
    dib.extend_from_slice(data.get(off_bits_src..bits_end)?);
    Some(dib)
}

fn parse_emf_pen_object(data: &[u8]) -> Option<(usize, EmfObject)> {
    if data.len() < 20 {
        return None;
    }
    let handle = usize::try_from(read_le_u32(data, 0)?).ok()?;
    let style = read_le_u32(data, 4)?;
    if !emf_pen_style_is_supported(style) {
        return Some((handle, EmfObject::Other));
    }
    let width = i32::try_from(read_le_i32(data, 8)?.unsigned_abs().max(1)).unwrap_or(i32::MAX);
    let color = color_from_colorref(data, 16)?;
    Some((
        handle,
        EmfObject::Pen {
            color: if (style & 0x000f) == 5 {
                None
            } else {
                Some(color)
            },
            width,
            style: wmf_pen_border_style((style & 0xffff) as u16),
        },
    ))
}

fn parse_emf_ext_pen_object(data: &[u8]) -> Option<(usize, EmfObject)> {
    const BS_NULL: u32 = 1;
    const BS_HATCHED: u32 = 2;

    if data.len() < 44 {
        return None;
    }
    let handle = usize::try_from(read_le_u32(data, 0)?).ok()?;
    let off_bmi = read_le_u32(data, 4)?;
    let cb_bmi = read_le_u32(data, 8)?;
    let off_bits = read_le_u32(data, 12)?;
    let cb_bits = read_le_u32(data, 16)?;

    let pen_style = read_le_u32(data, 20)?;
    if !emf_pen_style_is_supported(pen_style) {
        return Some((handle, EmfObject::Other));
    }
    let width = i32::try_from(read_le_u32(data, 24)?.max(1)).unwrap_or(i32::MAX);
    let brush_style = read_le_u32(data, 28)?;
    let color = color_from_colorref(data, 32)?;
    let style_entry_count = usize::try_from(read_le_u32(data, 40)?).ok()?;
    if style_entry_count > MAX_PASSIVE_WMF_POINTS_PER_RECORD {
        return None;
    }
    let entries_end = 44usize.checked_add(style_entry_count.checked_mul(4)?)?;
    if entries_end > data.len() {
        return None;
    }
    if !matches!(brush_style, 0 | BS_NULL | BS_HATCHED) {
        validate_emf_optional_record_payload(data, off_bmi, cb_bmi)?;
        validate_emf_optional_record_payload(data, off_bits, cb_bits)?;
    }

    let style = wmf_pen_border_style((pen_style & 0xffff) as u16);
    let color = if (pen_style & 0x000f) == 5 || brush_style == BS_NULL {
        None
    } else {
        Some(color)
    };

    Some((
        handle,
        EmfObject::Pen {
            color,
            width,
            style,
        },
    ))
}

fn emf_pen_style_is_supported(style: u32) -> bool {
    const PS_STYLE_MASK: u32 = 0x0000_000f;
    const PS_ENDCAP_MASK: u32 = 0x0000_0f00;
    const PS_JOIN_MASK: u32 = 0x0000_f000;
    const PS_TYPE_MASK: u32 = 0x000f_0000;

    matches!(style & PS_STYLE_MASK, 0..=6)
        && matches!(style & PS_ENDCAP_MASK, 0x0000 | 0x0100 | 0x0200)
        && matches!(style & PS_JOIN_MASK, 0x0000 | 0x1000 | 0x2000)
        && matches!(style & PS_TYPE_MASK, 0x0000 | 0x0001_0000)
        && (style & !(PS_STYLE_MASK | PS_ENDCAP_MASK | PS_JOIN_MASK | PS_TYPE_MASK)) == 0
}

fn emf_layout_is_supported_passive_noop(layout: u32) -> bool {
    const LAYOUT_LTR: u32 = 0x0000;
    const LAYOUT_BITMAPORIENTATIONPRESERVED: u32 = 0x0008;

    matches!(layout, LAYOUT_LTR | LAYOUT_BITMAPORIENTATIONPRESERVED)
}

fn validate_emf_optional_record_payload(
    data: &[u8],
    record_offset: u32,
    byte_count: u32,
) -> Option<()> {
    if record_offset == 0 && byte_count == 0 {
        return Some(());
    }
    if record_offset < 8 || byte_count == 0 {
        return None;
    }
    let data_offset = usize::try_from(record_offset).ok()?.checked_sub(8)?;
    let byte_count = usize::try_from(byte_count).ok()?;
    let end = data_offset.checked_add(byte_count)?;
    if end > data.len() {
        return None;
    }
    Some(())
}

fn parse_emf_ext_font_object(data: &[u8]) -> Option<(usize, EmfObject)> {
    const LOGFONTW_MIN_BYTES: usize = 28;
    const LF_HEIGHT_OFFSET: usize = 4;
    const LF_ESCAPEMENT_OFFSET: usize = 12;
    const LF_ORIENTATION_OFFSET: usize = 16;
    const LF_CHARSET_OFFSET: usize = 27;

    if data.len() < LF_HEIGHT_OFFSET.checked_add(LOGFONTW_MIN_BYTES)? {
        return None;
    }
    let handle = usize::try_from(read_le_u32(data, 0)?).ok()?;
    let height = read_le_i32(data, LF_HEIGHT_OFFSET)?;
    if height == 0 {
        return Some((handle, EmfObject::Other));
    }
    let escapement = read_le_i32(data, LF_ESCAPEMENT_OFFSET)?;
    let orientation = read_le_i32(data, LF_ORIENTATION_OFFSET)?;
    if escapement != 0 || orientation != 0 {
        return Some((handle, EmfObject::Other));
    }
    Some((
        handle,
        EmfObject::Font {
            height,
            charset: data.get(LF_CHARSET_OFFSET).copied().map(i32::from),
        },
    ))
}

fn parse_emf_comment_record(data: &[u8]) -> Option<()> {
    if data.len() < 4 {
        return None;
    }
    let byte_count = usize::try_from(read_le_u32(data, 0)?).ok()?;
    let end = 4usize.checked_add(byte_count)?;
    if end > data.len() {
        return None;
    }
    Some(())
}

fn parse_emf_brush_object(data: &[u8]) -> Option<(usize, EmfObject)> {
    if data.len() < 16 {
        return None;
    }
    let handle = usize::try_from(read_le_u32(data, 0)?).ok()?;
    let style = read_le_u32(data, 4)?;
    let color = color_from_colorref(data, 8)?;
    if !matches!(style, 0 | 1 | 2) {
        return Some((handle, EmfObject::Other));
    }
    let pattern = if style == 2 {
        wmf_hatch_shading_pattern((read_le_u32(data, 12)? & 0xffff) as u16)
    } else {
        ShadingPattern::None
    };
    Some((
        handle,
        EmfObject::Brush {
            color: if style == 1 { None } else { Some(color) },
            pattern,
        },
    ))
}

fn parse_emf_palette_object(data: &[u8]) -> Option<usize> {
    if data.len() < 8 {
        return None;
    }
    let handle = usize::try_from(read_le_u32(data, 0)?).ok()?;
    let entry_count = usize::from(read_le_u16(data, 6)?);
    let entries_end = 8usize.checked_add(entry_count.checked_mul(4)?)?;
    if entries_end > data.len() || entry_count > MAX_PASSIVE_WMF_OBJECTS {
        return None;
    }
    Some(handle)
}

fn validate_emf_set_palette_entries(data: &[u8]) -> Option<()> {
    if data.len() < 12 {
        return None;
    }
    read_le_u32(data, 0)?;
    read_le_u32(data, 4)?;
    let entry_count = usize::try_from(read_le_u32(data, 8)?).ok()?;
    let entries_end = 12usize.checked_add(entry_count.checked_mul(4)?)?;
    if entries_end > data.len() || entry_count > MAX_PASSIVE_WMF_OBJECTS {
        return None;
    }
    Some(())
}

fn store_emf_object(
    objects: &mut Vec<Option<EmfObject>>,
    handle: usize,
    object: EmfObject,
) -> Option<()> {
    if handle >= MAX_PASSIVE_WMF_OBJECTS {
        return None;
    }
    let required_len = handle.checked_add(1)?;
    if objects.len() < required_len {
        objects.resize(required_len, None);
    }
    objects[handle] = Some(object);
    Some(())
}

fn apply_emf_object(
    state: &mut EmfDrawingState,
    object: EmfObject,
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) {
    match object {
        EmfObject::Pen {
            color,
            width,
            style,
        } => {
            state.stroke_color = color;
            state.stroke_width_logical = width.max(1);
            state.stroke_width = normalized_emf_stroke_width(width, header, coordinates);
            state.stroke_style = style;
        }
        EmfObject::Brush { color, pattern } => {
            state.fill_color = color;
            state.fill_pattern = pattern;
        }
        EmfObject::Font { height, charset } => {
            state.font_height = Some(height);
            state.font_charset = charset;
        }
        EmfObject::Other => {}
    }
}

fn normalized_emf_stroke_width(
    width: i32,
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> f32 {
    let width = width.max(1);
    let x = map_emf_length_axis(
        width,
        coordinates.window_extent_x,
        coordinates.viewport_extent_x,
        header.width_px,
    ) * coordinates.world_transform.scale_x.abs();
    let y = map_emf_length_axis(
        width,
        coordinates.window_extent_y,
        coordinates.viewport_extent_y,
        header.height_px,
    ) * coordinates.world_transform.scale_y.abs();
    x.abs()
        .max(y.abs())
        .clamp(1.0, header.width_px.max(header.height_px) as f32)
}

fn restore_emf_saved_state(
    data: &[u8],
    saved_states: &mut Vec<EmfSavedState>,
) -> Option<RestoredEmfSavedState> {
    if data.len() < 4 {
        return None;
    }
    let relative = read_le_i32(data, 0)?;
    if relative >= 0 {
        return None;
    }
    let restore_count = usize::try_from(relative.unsigned_abs()).ok()?;
    if restore_count == 0 || restore_count > saved_states.len() {
        return None;
    }
    let mut restored = None;
    for _ in 0..restore_count {
        restored = saved_states.pop();
    }
    Some(RestoredEmfSavedState {
        state: restored?,
        restore_count,
    })
}

fn emf_stock_object(handle: u32) -> Option<EmfObject> {
    match handle {
        0x8000_0000 => Some(EmfObject::Brush {
            color: Some(Color {
                red: 255,
                green: 255,
                blue: 255,
            }),
            pattern: ShadingPattern::None,
        }),
        0x8000_0001 => Some(EmfObject::Brush {
            color: Some(Color {
                red: 192,
                green: 192,
                blue: 192,
            }),
            pattern: ShadingPattern::None,
        }),
        0x8000_0002 => Some(EmfObject::Brush {
            color: Some(Color {
                red: 128,
                green: 128,
                blue: 128,
            }),
            pattern: ShadingPattern::None,
        }),
        0x8000_0003 => Some(EmfObject::Brush {
            color: Some(Color {
                red: 64,
                green: 64,
                blue: 64,
            }),
            pattern: ShadingPattern::None,
        }),
        0x8000_0004 => Some(EmfObject::Brush {
            color: Some(Color::default()),
            pattern: ShadingPattern::None,
        }),
        0x8000_0005 => Some(EmfObject::Brush {
            color: None,
            pattern: ShadingPattern::None,
        }),
        0x8000_0006 => Some(EmfObject::Pen {
            color: Some(Color {
                red: 255,
                green: 255,
                blue: 255,
            }),
            width: 1,
            style: BorderStyle::Single,
        }),
        0x8000_0007 => Some(EmfObject::Pen {
            color: Some(Color::default()),
            width: 1,
            style: BorderStyle::Single,
        }),
        0x8000_0008 => Some(EmfObject::Pen {
            color: None,
            width: 1,
            style: BorderStyle::Single,
        }),
        0x8000_0012 => Some(EmfObject::Brush {
            color: Some(Color {
                red: 255,
                green: 255,
                blue: 255,
            }),
            pattern: ShadingPattern::None,
        }),
        0x8000_0013 => Some(EmfObject::Pen {
            color: Some(Color::default()),
            width: 1,
            style: BorderStyle::Single,
        }),
        _ => None,
    }
}

fn parse_emf_poly_points(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> Option<Vec<(f32, f32)>> {
    parse_emf_raw_poly_points(data).map(|points| {
        points
            .into_iter()
            .map(|(x, y)| normalized_emf_point(x, y, header, coordinates))
            .collect()
    })
}

fn parse_emf_raw_poly_points(data: &[u8]) -> Option<Vec<(i32, i32)>> {
    if data.len() < 20 {
        return None;
    }
    let count = usize::try_from(read_le_u32(data, 16)?).ok()?;
    if count > MAX_PASSIVE_WMF_POINTS_PER_RECORD {
        return None;
    }
    let points_start = 20usize;
    let points_bytes = count.checked_mul(8)?;
    let points_end = points_start.checked_add(points_bytes)?;
    if points_end > data.len() {
        return None;
    }

    let mut points = Vec::with_capacity(count);
    for idx in 0..count {
        let offset = points_start + (idx * 8);
        let x = read_le_i32(data, offset)?;
        let y = read_le_i32(data, offset + 4)?;
        points.push((x, y));
    }
    Some(points)
}

fn parse_emf_poly16_points(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> Option<Vec<(f32, f32)>> {
    parse_emf_raw_poly16_points(data).map(|points| {
        points
            .into_iter()
            .map(|(x, y)| normalized_emf_point(x, y, header, coordinates))
            .collect()
    })
}

fn parse_emf_raw_poly16_points(data: &[u8]) -> Option<Vec<(i32, i32)>> {
    if data.len() < 20 {
        return None;
    }
    let count = usize::try_from(read_le_u32(data, 16)?).ok()?;
    if count > MAX_PASSIVE_WMF_POINTS_PER_RECORD {
        return None;
    }
    let points_start = 20usize;
    let points_bytes = count.checked_mul(4)?;
    let points_end = points_start.checked_add(points_bytes)?;
    if points_end > data.len() {
        return None;
    }

    let mut points = Vec::with_capacity(count);
    for idx in 0..count {
        let offset = points_start + (idx * 4);
        let x = i32::from(read_le_i16(data, offset)?);
        let y = i32::from(read_le_i16(data, offset + 2)?);
        points.push((x, y));
    }
    Some(points)
}

fn parse_emf_polydraw(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
    current_position: (i32, i32),
    stroke_color: Option<Color>,
    stroke_width: f32,
    stroke_style: BorderStyle,
) -> Option<ParsedEmfPolyDraw> {
    let (points, types) = parse_emf_raw_polydraw(data, false)?;
    if emf_polydraw_has_incomplete_bezier_run(&types) {
        return Some(ParsedEmfPolyDraw {
            commands: Vec::new(),
            end_position: current_position,
        });
    }
    build_emf_polydraw_commands(
        &points,
        &types,
        header,
        coordinates,
        current_position,
        stroke_color,
        stroke_width,
        stroke_style,
    )
}

fn parse_emf_polydraw16(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
    current_position: (i32, i32),
    stroke_color: Option<Color>,
    stroke_width: f32,
    stroke_style: BorderStyle,
) -> Option<ParsedEmfPolyDraw> {
    let (points, types) = parse_emf_raw_polydraw(data, true)?;
    if emf_polydraw_has_incomplete_bezier_run(&types) {
        return Some(ParsedEmfPolyDraw {
            commands: Vec::new(),
            end_position: current_position,
        });
    }
    build_emf_polydraw_commands(
        &points,
        &types,
        header,
        coordinates,
        current_position,
        stroke_color,
        stroke_width,
        stroke_style,
    )
}

fn parse_emf_raw_polydraw(data: &[u8], compact_points: bool) -> Option<(Vec<(i32, i32)>, Vec<u8>)> {
    if data.len() < 20 {
        return None;
    }
    let count = usize::try_from(read_le_u32(data, 16)?).ok()?;
    if count > MAX_PASSIVE_WMF_POINTS_PER_RECORD {
        return None;
    }
    let point_size = if compact_points { 4usize } else { 8usize };
    let points_start = 20usize;
    let points_bytes = count.checked_mul(point_size)?;
    let types_start = points_start.checked_add(points_bytes)?;
    let types_end = types_start.checked_add(count)?;
    if types_end > data.len() {
        return None;
    }

    let mut points = Vec::with_capacity(count);
    for idx in 0..count {
        let offset = points_start + (idx * point_size);
        let point = if compact_points {
            (
                i32::from(read_le_i16(data, offset)?),
                i32::from(read_le_i16(data, offset + 2)?),
            )
        } else {
            (read_le_i32(data, offset)?, read_le_i32(data, offset + 4)?)
        };
        points.push(point);
    }
    Some((points, data[types_start..types_end].to_vec()))
}

fn emf_polydraw_has_incomplete_bezier_run(types: &[u8]) -> bool {
    const PT_CLOSEFIGURE: u8 = 0x01;
    const PT_BEZIERTO: u8 = 0x04;

    let mut idx = 0usize;
    while idx < types.len() {
        let base = types[idx] & !PT_CLOSEFIGURE;
        if base != PT_BEZIERTO {
            idx += 1;
            continue;
        }
        if idx + 2 >= types.len() {
            return true;
        }
        if (types[idx + 1] & !PT_CLOSEFIGURE) != PT_BEZIERTO
            || (types[idx + 2] & !PT_CLOSEFIGURE) != PT_BEZIERTO
        {
            return true;
        }
        idx += 3;
    }
    false
}

fn build_emf_polydraw_commands(
    points: &[(i32, i32)],
    types: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
    initial_position: (i32, i32),
    stroke_color: Option<Color>,
    stroke_width: f32,
    stroke_style: BorderStyle,
) -> Option<ParsedEmfPolyDraw> {
    const PT_CLOSEFIGURE: u8 = 0x01;
    const PT_LINETO: u8 = 0x02;
    const PT_BEZIERTO: u8 = 0x04;
    const PT_MOVETO: u8 = 0x06;

    if points.len() != types.len() {
        return None;
    }
    let mut commands = Vec::new();
    let mut current_position = initial_position;
    let mut current =
        normalized_emf_point(initial_position.0, initial_position.1, header, coordinates);
    let mut figure_start = current;
    let mut figure_start_position = initial_position;
    let mut line_points: Vec<(f32, f32)> = Vec::new();
    let mut idx = 0usize;
    while idx < points.len() {
        let point_type = types[idx];
        let close = point_type & PT_CLOSEFIGURE != 0;
        let base = point_type & !PT_CLOSEFIGURE;
        match base {
            PT_MOVETO => {
                if close {
                    return None;
                }
                flush_emf_polydraw_line(
                    &mut commands,
                    &mut line_points,
                    stroke_color,
                    stroke_width,
                    stroke_style,
                )?;
                current_position = points[idx];
                current = normalized_emf_point(
                    current_position.0,
                    current_position.1,
                    header,
                    coordinates,
                );
                figure_start = current;
                figure_start_position = current_position;
                idx += 1;
            }
            PT_LINETO => {
                if line_points.is_empty() {
                    line_points.push(current);
                }
                current_position = points[idx];
                current = normalized_emf_point(
                    current_position.0,
                    current_position.1,
                    header,
                    coordinates,
                );
                line_points.push(current);
                if close {
                    line_points.push(figure_start);
                    current = figure_start;
                    current_position = figure_start_position;
                }
                idx += 1;
            }
            PT_BEZIERTO => {
                flush_emf_polydraw_line(
                    &mut commands,
                    &mut line_points,
                    stroke_color,
                    stroke_width,
                    stroke_style,
                )?;
                if idx.checked_add(2)? >= points.len() {
                    return None;
                }
                let first_type = types[idx];
                let second_type = types[idx + 1];
                let third_type = types[idx + 2];
                if first_type & PT_CLOSEFIGURE != 0 || second_type & PT_CLOSEFIGURE != 0 {
                    return None;
                }
                if (first_type & !PT_CLOSEFIGURE) != PT_BEZIERTO
                    || (second_type & !PT_CLOSEFIGURE) != PT_BEZIERTO
                    || (third_type & !PT_CLOSEFIGURE) != PT_BEZIERTO
                {
                    return None;
                }
                let mut bezier_points = Vec::with_capacity(4);
                bezier_points.push(current);
                for point in &points[idx..idx + 3] {
                    bezier_points.push(normalized_emf_point(point.0, point.1, header, coordinates));
                }
                if point_bounds_are_visible(&bezier_points) {
                    commands.push(StaticImageVectorCommand::Bezier {
                        points: bezier_points,
                        stroke_color,
                        stroke_width,
                        stroke_style,
                    });
                }
                current_position = points[idx + 2];
                current = normalized_emf_point(
                    current_position.0,
                    current_position.1,
                    header,
                    coordinates,
                );
                if third_type & PT_CLOSEFIGURE != 0 {
                    let close_points = vec![current, figure_start];
                    if segment_is_visible(close_points[0], close_points[1]) {
                        commands.push(StaticImageVectorCommand::Polyline {
                            points: close_points,
                            stroke_color,
                            stroke_width,
                            stroke_style,
                        });
                    }
                    current = figure_start;
                    current_position = figure_start_position;
                }
                idx += 3;
            }
            _ => return None,
        }
    }
    flush_emf_polydraw_line(
        &mut commands,
        &mut line_points,
        stroke_color,
        stroke_width,
        stroke_style,
    )?;
    Some(ParsedEmfPolyDraw {
        commands,
        end_position: current_position,
    })
}

fn flush_emf_polydraw_line(
    commands: &mut Vec<StaticImageVectorCommand>,
    line_points: &mut Vec<(f32, f32)>,
    stroke_color: Option<Color>,
    stroke_width: f32,
    stroke_style: BorderStyle,
) -> Option<()> {
    if line_points.len() >= 2
        && line_points
            .windows(2)
            .any(|pair| segment_is_visible(pair[0], pair[1]))
    {
        commands.push(StaticImageVectorCommand::Polyline {
            points: std::mem::take(line_points),
            stroke_color,
            stroke_width,
            stroke_style,
        });
    } else {
        line_points.clear();
    }
    if commands.len() > MAX_PASSIVE_WMF_COMMANDS {
        return None;
    }
    Some(())
}

pub(in crate::rtf::parser) fn polybezier_point_count_is_valid(count: usize) -> bool {
    count >= 4 && (count - 1) % 3 == 0
}

pub(in crate::rtf::parser) fn polybezierto_point_count_is_valid(count: usize) -> bool {
    count >= 3 && count % 3 == 0
}

fn emf_polypolygon_path_command(
    polygons: Vec<Vec<(f32, f32)>>,
    stroke_color: Option<Color>,
    stroke_width: f32,
    stroke_style: BorderStyle,
    fill_rule: StaticImageVectorFillRule,
    fill_pattern: ShadingPattern,
    fill_color: Option<Color>,
) -> Option<Option<StaticImageVectorCommand>> {
    if fill_color.is_none() && stroke_color.is_none() {
        return Some(None);
    }

    let mut visible_polygons = polygons
        .into_iter()
        .filter(|points| points.len() >= 3 && point_bounds_are_visible(points));
    let Some(first_polygon) = visible_polygons.next() else {
        return Some(None);
    };
    let start = *first_polygon.first()?;
    let mut segments = Vec::new();
    append_emf_polypolygon_path_segments(&mut segments, &first_polygon, false)?;
    for polygon in visible_polygons {
        append_emf_polypolygon_path_segments(&mut segments, &polygon, true)?;
    }
    if segments.is_empty() {
        return Some(None);
    }

    Some(Some(StaticImageVectorCommand::Path {
        start,
        segments,
        closed: false,
        stroke_color,
        stroke_width,
        stroke_style,
        fill_rule,
        fill_pattern,
        fill_color,
    }))
}

fn append_emf_polypolygon_path_segments(
    segments: &mut Vec<StaticImageVectorPathSegment>,
    points: &[(f32, f32)],
    include_move_to: bool,
) -> Option<()> {
    let first = *points.first()?;
    if include_move_to {
        if segments.len() >= MAX_PASSIVE_WMF_POINTS_PER_RECORD {
            return None;
        }
        segments.push(StaticImageVectorPathSegment::MoveTo(first.0, first.1));
    }
    for point in points.iter().skip(1) {
        if segments.len() >= MAX_PASSIVE_WMF_POINTS_PER_RECORD {
            return None;
        }
        segments.push(StaticImageVectorPathSegment::LineTo(point.0, point.1));
    }
    if points.last().is_some_and(|last| *last != first) {
        if segments.len() >= MAX_PASSIVE_WMF_POINTS_PER_RECORD {
            return None;
        }
        segments.push(StaticImageVectorPathSegment::LineTo(first.0, first.1));
    }
    Some(())
}

fn emf_exclude_clip_rect_command(
    header: &ParsedEmfHeader,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
) -> Option<StaticImageVectorCommand> {
    let image_right = header.width_px.max(1) as f32;
    let image_bottom = header.height_px.max(1) as f32;
    Some(StaticImageVectorCommand::ClipPath {
        start: (0.0, 0.0),
        segments: vec![
            StaticImageVectorPathSegment::LineTo(image_right, 0.0),
            StaticImageVectorPathSegment::LineTo(image_right, image_bottom),
            StaticImageVectorPathSegment::LineTo(0.0, image_bottom),
            StaticImageVectorPathSegment::LineTo(0.0, 0.0),
            StaticImageVectorPathSegment::MoveTo(left, top),
            StaticImageVectorPathSegment::LineTo(right, top),
            StaticImageVectorPathSegment::LineTo(right, bottom),
            StaticImageVectorPathSegment::LineTo(left, bottom),
            StaticImageVectorPathSegment::LineTo(left, top),
        ],
        closed: false,
        fill_rule: StaticImageVectorFillRule::Alternate,
    })
}

fn parse_emf_poly_poly_points(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> Option<Vec<Vec<(f32, f32)>>> {
    if data.len() < 24 {
        return None;
    }
    let polygon_count = usize::try_from(read_le_u32(data, 16)?).ok()?;
    let point_count = usize::try_from(read_le_u32(data, 20)?).ok()?;
    if polygon_count == 0 && point_count != 0 {
        return None;
    }
    if polygon_count > MAX_PASSIVE_WMF_POINTS_PER_RECORD
        || point_count > MAX_PASSIVE_WMF_POINTS_PER_RECORD
    {
        return None;
    }
    let counts_start = 24usize;
    let counts_bytes = polygon_count.checked_mul(4)?;
    let points_start = counts_start.checked_add(counts_bytes)?;
    let points_bytes = point_count.checked_mul(8)?;
    let points_end = points_start.checked_add(points_bytes)?;
    if points_end > data.len() {
        return None;
    }

    let mut counts = Vec::with_capacity(polygon_count);
    let mut total_count = 0usize;
    for idx in 0..polygon_count {
        let count = usize::try_from(read_le_u32(data, counts_start + (idx * 4))?).ok()?;
        total_count = total_count.checked_add(count)?;
        if total_count > point_count {
            return None;
        }
        counts.push(count);
    }
    if total_count != point_count {
        return None;
    }

    let mut polygons = Vec::with_capacity(polygon_count);
    let mut point_index = 0usize;
    for count in counts {
        let mut points = Vec::with_capacity(count);
        for _ in 0..count {
            let offset = points_start + (point_index * 8);
            let x = read_le_i32(data, offset)?;
            let y = read_le_i32(data, offset + 4)?;
            points.push(normalized_emf_point(x, y, header, coordinates));
            point_index += 1;
        }
        polygons.push(points);
    }
    Some(polygons)
}

fn parse_emf_poly_poly16_points(
    data: &[u8],
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> Option<Vec<Vec<(f32, f32)>>> {
    if data.len() < 24 {
        return None;
    }
    let polygon_count = usize::try_from(read_le_u32(data, 16)?).ok()?;
    let point_count = usize::try_from(read_le_u32(data, 20)?).ok()?;
    if polygon_count == 0 && point_count != 0 {
        return None;
    }
    if polygon_count > MAX_PASSIVE_WMF_POINTS_PER_RECORD
        || point_count > MAX_PASSIVE_WMF_POINTS_PER_RECORD
    {
        return None;
    }
    let counts_start = 24usize;
    let counts_bytes = polygon_count.checked_mul(4)?;
    let points_start = counts_start.checked_add(counts_bytes)?;
    let points_bytes = point_count.checked_mul(4)?;
    let points_end = points_start.checked_add(points_bytes)?;
    if points_end > data.len() {
        return None;
    }

    let mut counts = Vec::with_capacity(polygon_count);
    let mut total_count = 0usize;
    for idx in 0..polygon_count {
        let count = usize::try_from(read_le_u32(data, counts_start + (idx * 4))?).ok()?;
        total_count = total_count.checked_add(count)?;
        if total_count > point_count {
            return None;
        }
        counts.push(count);
    }
    if total_count != point_count {
        return None;
    }

    let mut polygons = Vec::with_capacity(polygon_count);
    let mut point_index = 0usize;
    for count in counts {
        let mut points = Vec::with_capacity(count);
        for _ in 0..count {
            let offset = points_start + (point_index * 4);
            let x = i32::from(read_le_i16(data, offset)?);
            let y = i32::from(read_le_i16(data, offset + 2)?);
            points.push(normalized_emf_point(x, y, header, coordinates));
            point_index += 1;
        }
        polygons.push(points);
    }
    Some(polygons)
}

fn parse_emf_roundrect_corners(data: &[u8], width: f32, height: f32) -> Option<(f32, f32)> {
    if data.len() < 24 {
        return None;
    }
    let corner_width = read_le_i32(data, 16)?.unsigned_abs() as f32;
    let corner_height = read_le_i32(data, 20)?.unsigned_abs() as f32;
    Some((
        corner_width.clamp(0.0, width.max(0.0)),
        corner_height.clamp(0.0, height.max(0.0)),
    ))
}

fn parse_emf_raw_point_record(data: &[u8]) -> Option<(i32, i32)> {
    if data.len() < 8 {
        return None;
    }
    let x = read_le_i32(data, 0)?;
    let y = read_le_i32(data, 4)?;
    Some((x, y))
}

fn parse_emf_size_record(data: &[u8]) -> Option<(i32, i32)> {
    parse_emf_raw_point_record(data)
}

fn parse_emf_scale_record(data: &[u8]) -> Option<(i32, i32, i32, i32)> {
    if data.len() < 16 {
        return None;
    }
    Some((
        read_le_i32(data, 0)?,
        read_le_i32(data, 4)?,
        read_le_i32(data, 8)?,
        read_le_i32(data, 12)?,
    ))
}

fn scale_emf_extent(value: i32, numerator: i32, denominator: i32) -> Option<i32> {
    if value == 0 || numerator == 0 || denominator == 0 {
        return None;
    }
    let scaled = (i128::from(value) * i128::from(numerator)) / i128::from(denominator);
    i32::try_from(scaled).ok().filter(|value| *value != 0)
}

fn normalized_emf_rect(
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> Option<(f32, f32, f32, f32)> {
    let (x1, y1) = normalized_emf_point(left, top, header, coordinates);
    let (x2, y2) = normalized_emf_point(right, bottom, header, coordinates);
    let (x1, x2) = (x1.min(x2), x1.max(x2));
    let (y1, y2) = (y1.min(y2), y1.max(y2));
    if x2 <= x1 || y2 <= y1 {
        return None;
    }
    Some((x1, y1, x2, y2))
}

fn normalized_emf_point(
    x: i32,
    y: i32,
    header: &ParsedEmfHeader,
    coordinates: &EmfCoordinateState,
) -> (f32, f32) {
    let x = f64::from(x) * f64::from(coordinates.world_transform.scale_x)
        + f64::from(coordinates.world_transform.translate_x);
    let y = f64::from(y) * f64::from(coordinates.world_transform.scale_y)
        + f64::from(coordinates.world_transform.translate_y);
    let x = map_emf_axis(
        x,
        coordinates.window_origin_x,
        coordinates.window_extent_x,
        coordinates.viewport_origin_x,
        coordinates.viewport_extent_x,
        header.width_px,
    );
    let y = map_emf_axis(
        y,
        coordinates.window_origin_y,
        coordinates.window_extent_y,
        coordinates.viewport_origin_y,
        coordinates.viewport_extent_y,
        header.height_px,
    );
    (x, y)
}

fn map_emf_axis(
    value: f64,
    window_origin: i32,
    window_extent: i32,
    viewport_origin: i32,
    viewport_extent: i32,
    max: u32,
) -> f32 {
    let max = i128::from(max);
    if window_extent == 0 || viewport_extent == 0 {
        return 0.0;
    }
    let relative = value - f64::from(window_origin);
    let scaled = (relative * f64::from(viewport_extent)) / f64::from(window_extent);
    let device = f64::from(viewport_origin) + scaled;
    device.clamp(0.0, max as f64) as f32
}

fn map_emf_length_axis(value: i32, window_extent: i32, viewport_extent: i32, max: u32) -> f32 {
    if window_extent == 0 || viewport_extent == 0 || value == 0 {
        return 0.0;
    }
    let scaled = (i128::from(value) * i128::from(viewport_extent)) / i128::from(window_extent);
    let limit = i128::from(max.max(1));
    scaled.clamp(-limit, limit) as f32
}

fn signed_rect_extent(start: i32, end: i32) -> Option<u32> {
    let extent = i64::from(end) - i64::from(start);
    u32::try_from(extent.unsigned_abs())
        .ok()
        .filter(|value| *value > 0)
}

fn hundredth_mm_to_twips(value: Option<u32>) -> Option<i32> {
    let hundredth_mm = u128::from(value?);
    let twips = ((hundredth_mm * 72) + 63) / 127;
    i32::try_from(twips.max(1)).ok()
}

fn hundredth_mm_to_96dpi_px(value: Option<u32>) -> Option<u32> {
    let hundredth_mm = u128::from(value?);
    let px = ((hundredth_mm * 96) + 1270) / 2540;
    u32::try_from(px.max(1)).ok()
}

pub(in crate::rtf::parser) fn twips_to_96dpi_px(value: i32) -> u32 {
    let twips = u128::from(value.max(1) as u32);
    let px = ((twips * 96) + 720) / 1440;
    u32::try_from(px.max(1)).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::super::super::minimal_emf_with_bounds_and_frame;
    use super::*;

    #[test]
    fn rejects_an_explicit_point_limit_below_the_decoder_cap() {
        let emf = minimal_emf_with_bounds_and_frame(1, 1, 1, 1);
        assert!(
            parse_emf_vector_image_data_with_limits(
                &emf,
                VectorDecodeLimits {
                    max_points_per_record: 1,
                    max_raster_pixels: usize::MAX,
                },
            )
            .is_none()
        );
    }
}
