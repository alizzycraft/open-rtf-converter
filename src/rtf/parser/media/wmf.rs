//! Bounded WMF parsing and passive vector-command recovery.

use super::super::*;
use super::emf::*;

#[derive(Debug)]
pub(in crate::rtf::parser) struct ParsedWmfVector {
    pub(in crate::rtf::parser) width_px: u32,
    pub(in crate::rtf::parser) height_px: u32,
    pub(in crate::rtf::parser) skipped_record_count: usize,
    pub(in crate::rtf::parser) commands: Vec<StaticImageVectorCommand>,
}

#[derive(Debug, Clone)]
pub(in crate::rtf::parser) struct ParsedWmfExtTextOut {
    pub(in crate::rtf::parser) x: f32,
    pub(in crate::rtf::parser) y: f32,
    pub(in crate::rtf::parser) text: String,
    pub(in crate::rtf::parser) opaque_bounds: Option<(f32, f32, f32, f32)>,
    pub(in crate::rtf::parser) clip_bounds: Option<(f32, f32, f32, f32)>,
}

#[derive(Debug, Copy, Clone)]
pub(in crate::rtf::parser) struct WmfHeaderInfo {
    pub(in crate::rtf::parser) header_start: usize,
    pub(in crate::rtf::parser) file_end: usize,
    pub(in crate::rtf::parser) window_origin_x: i32,
    pub(in crate::rtf::parser) window_origin_y: i32,
    pub(in crate::rtf::parser) window_width: i32,
    pub(in crate::rtf::parser) window_height: i32,
}

#[derive(Debug, Copy, Clone)]
pub(in crate::rtf::parser) enum WmfObject {
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
        bold: bool,
    },
    Other,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(in crate::rtf::parser) enum WmfTextBackgroundMode {
    Transparent,
    Opaque,
}

#[derive(Debug, Copy, Clone)]
pub(in crate::rtf::parser) struct WmfDrawingState {
    pub(in crate::rtf::parser) stroke_color: Option<Color>,
    pub(in crate::rtf::parser) stroke_width: f32,
    pub(in crate::rtf::parser) stroke_style: BorderStyle,
    pub(in crate::rtf::parser) stock_pen_selected: bool,
    pub(in crate::rtf::parser) fill_rule: StaticImageVectorFillRule,
    pub(in crate::rtf::parser) fill_color: Option<Color>,
    pub(in crate::rtf::parser) fill_pattern: ShadingPattern,
    pub(in crate::rtf::parser) text_color: Option<Color>,
    pub(in crate::rtf::parser) background_color: Option<Color>,
    pub(in crate::rtf::parser) text_background_mode: WmfTextBackgroundMode,
    pub(in crate::rtf::parser) text_character_extra: f32,
    pub(in crate::rtf::parser) text_word_extra: f32,
    pub(in crate::rtf::parser) font_height: Option<i32>,
    pub(in crate::rtf::parser) font_charset: Option<i32>,
    pub(in crate::rtf::parser) font_bold: bool,
    pub(in crate::rtf::parser) stroke_rop2: u16,
    pub(in crate::rtf::parser) text_horizontal_align: StaticImageTextHorizontalAlign,
    pub(in crate::rtf::parser) text_vertical_align: StaticImageTextVerticalAlign,
}

#[derive(Debug, Copy, Clone)]
pub(in crate::rtf::parser) struct WmfSavedDrawingState {
    pub(in crate::rtf::parser) state: WmfDrawingState,
    pub(in crate::rtf::parser) current_point: (f32, f32),
    pub(in crate::rtf::parser) window_origin_x: i32,
    pub(in crate::rtf::parser) window_origin_y: i32,
    pub(in crate::rtf::parser) viewport_origin_x: i32,
    pub(in crate::rtf::parser) viewport_origin_y: i32,
    pub(in crate::rtf::parser) viewport_width: i32,
    pub(in crate::rtf::parser) viewport_height: i32,
    pub(in crate::rtf::parser) viewport_extent_explicit: bool,
    pub(in crate::rtf::parser) window_width: i32,
    pub(in crate::rtf::parser) window_height: i32,
    pub(in crate::rtf::parser) clip_scope_command_start: Option<usize>,
}

#[derive(Debug, Copy, Clone)]
pub(in crate::rtf::parser) struct RestoredWmfSavedDrawingState {
    pub(in crate::rtf::parser) state: WmfSavedDrawingState,
    pub(in crate::rtf::parser) restore_count: usize,
}

impl Default for WmfDrawingState {
    fn default() -> Self {
        Self {
            stroke_color: Some(Color::default()),
            // Word's PDF playback expands the fresh GDI BLACK_PEN to about
            // 1.878 points in the common 200x100 -> 108x36 point WMF frame.
            // Preserve that measured stock-pen width in logical coordinates;
            // an explicitly selected CREATEPEN object replaces it below.
            stroke_width: WMF_WORD_STOCK_PEN_LOGICAL_WIDTH,
            stroke_style: BorderStyle::Single,
            stock_pen_selected: true,
            fill_rule: StaticImageVectorFillRule::Alternate,
            fill_color: None,
            fill_pattern: ShadingPattern::None,
            text_color: Some(Color::default()),
            background_color: Some(Color {
                red: 255,
                green: 255,
                blue: 255,
            }),
            // A fresh GDI device context starts in OPAQUE background mode.
            // Word preserves that stock state when exporting fontless WMF text.
            text_background_mode: WmfTextBackgroundMode::Opaque,
            text_character_extra: 0.0,
            text_word_extra: 0.0,
            font_height: None,
            font_charset: None,
            // A fresh metafile DC uses the stock SYSTEM_FONT. Word's passive
            // export maps that stock face to an Arial-compatible bold font.
            font_bold: true,
            stroke_rop2: WMF_ROP2_COPYPEN,
            text_horizontal_align: StaticImageTextHorizontalAlign::Left,
            text_vertical_align: StaticImageTextVerticalAlign::Top,
        }
    }
}

impl WmfDrawingState {
    pub(in crate::rtf::parser) fn passive_stroke_color(self) -> Option<Color> {
        match self.stroke_rop2 {
            WMF_ROP2_BLACK => Some(Color::default()),
            WMF_ROP2_NOTCOPYPEN => self.stroke_color.map(inverted_color),
            WMF_ROP2_NOP => None,
            WMF_ROP2_COPYPEN => self.stroke_color,
            WMF_ROP2_WHITE => Some(white_color()),
            _ => None,
        }
    }
}

pub(in crate::rtf::parser) const MAX_PASSIVE_WMF_RECORDS: usize = 512;
pub(in crate::rtf::parser) const MAX_PASSIVE_WMF_COMMANDS: usize = 256;
pub(in crate::rtf::parser) const MAX_PASSIVE_WMF_POINTS_PER_RECORD: usize = 128;
pub(in crate::rtf::parser) const MAX_PASSIVE_WMF_ARC_SEGMENTS: usize = 64;
pub(in crate::rtf::parser) const MAX_PASSIVE_WMF_TEXT_BYTES: usize = 512;
pub(in crate::rtf::parser) const MAX_PASSIVE_WMF_OBJECTS: usize = 256;
pub(in crate::rtf::parser) const MAX_PASSIVE_WMF_SAVED_STATES: usize = 32;
pub(in crate::rtf::parser) const WMF_WORD_STOCK_PEN_LOGICAL_WIDTH: f32 = 4.172_978;
pub(in crate::rtf::parser) const WMF_WORD_AUTHORED_PEN_SCALE: f32 =
    WMF_WORD_STOCK_PEN_LOGICAL_WIDTH / 4.0;
// Word exports a WMF SETPIXEL as a one-device-pixel horizontal stroke. In the
// common 200-unit WMF window that stroke spans 0.8577 points inside a 108-point
// picture, or 1.5883 normalized logical units. Keep the primitive bounded and
// independent from the selected GDI pen state.
pub(in crate::rtf::parser) const WMF_WORD_SETPIXEL_LOGICAL_LENGTH: f32 = 1.588_3;
pub(in crate::rtf::parser) const MAX_PASSIVE_VECTOR_RASTER_PIXELS: usize = 1_000_000;
pub(in crate::rtf::parser) const WMF_ETO_OPAQUE: u16 = 0x0002;
pub(in crate::rtf::parser) const WMF_ETO_CLIPPED: u16 = 0x0004;
pub(in crate::rtf::parser) const WMF_ETO_GLYPH_INDEX: u16 = 0x0010;
pub(in crate::rtf::parser) const WMF_ETO_NO_RECT: u16 = 0x0100;
pub(in crate::rtf::parser) const WMF_ETO_PDY: u16 = 0x2000;
pub(in crate::rtf::parser) const WMF_ETO_SUPPORTED_MASK: u16 =
    WMF_ETO_OPAQUE | WMF_ETO_CLIPPED | WMF_ETO_NO_RECT | WMF_ETO_PDY;
pub(in crate::rtf::parser) const WMF_BKMODE_TRANSPARENT: u16 = 1;
pub(in crate::rtf::parser) const WMF_BKMODE_OPAQUE: u16 = 2;
pub(in crate::rtf::parser) const WMF_POLYFILLMODE_ALTERNATE: u16 = 1;
pub(in crate::rtf::parser) const WMF_POLYFILLMODE_WINDING: u16 = 2;
pub(in crate::rtf::parser) const WMF_MAPMODE_TEXT: u16 = 1;
pub(in crate::rtf::parser) const WMF_MAPMODE_ISOTROPIC: u16 = 7;
pub(in crate::rtf::parser) const WMF_MAPMODE_ANISOTROPIC: u16 = 8;
pub(in crate::rtf::parser) const WMF_ROP2_BLACK: u16 = 1;
pub(in crate::rtf::parser) const WMF_ROP2_NOTCOPYPEN: u16 = 4;
pub(in crate::rtf::parser) const WMF_ROP2_NOP: u16 = 11;
pub(in crate::rtf::parser) const WMF_ROP2_COPYPEN: u16 = 13;
pub(in crate::rtf::parser) const WMF_ROP2_WHITE: u16 = 16;
pub(in crate::rtf::parser) const WMF_STRETCHMODE_BLACKONWHITE: u16 = 1;
pub(in crate::rtf::parser) const WMF_STRETCHMODE_WHITEONBLACK: u16 = 2;
pub(in crate::rtf::parser) const WMF_STRETCHMODE_COLORONCOLOR: u16 = 3;
pub(in crate::rtf::parser) const WMF_STRETCHMODE_HALFTONE: u16 = 4;
pub(in crate::rtf::parser) const WMF_TA_RIGHT: u16 = 0x0002;
pub(in crate::rtf::parser) const WMF_TA_CENTER: u16 = 0x0006;
pub(in crate::rtf::parser) const WMF_TA_BOTTOM: u16 = 0x0008;
pub(in crate::rtf::parser) const WMF_TA_BASELINE: u16 = 0x0018;
pub(in crate::rtf::parser) const WMF_TA_HORIZONTAL_MASK: u16 = 0x0006;
pub(in crate::rtf::parser) const WMF_TA_VERTICAL_MASK: u16 = 0x0018;
pub(in crate::rtf::parser) const WMF_TA_SUPPORTED_MASK: u16 =
    WMF_TA_HORIZONTAL_MASK | WMF_TA_VERTICAL_MASK;
pub(in crate::rtf::parser) const WMF_BLACKNESS_RASTER_OP: u32 = 0x0000_0042;
pub(in crate::rtf::parser) const WMF_DSTCOPY_RASTER_OP: u32 = 0x00aa_0029;
pub(in crate::rtf::parser) const WMF_DSTINVERT_RASTER_OP: u32 = 0x0055_0009;
pub(in crate::rtf::parser) const WMF_MERGEPAINT_RASTER_OP: u32 = 0x00bb_0226;
pub(in crate::rtf::parser) const WMF_MERGECOPY_RASTER_OP: u32 = 0x00c0_00ca;
pub(in crate::rtf::parser) const WMF_NOTSRCERASE_RASTER_OP: u32 = 0x0011_00a6;
pub(in crate::rtf::parser) const WMF_NOTSRCCOPY_RASTER_OP: u32 = 0x0033_0008;
pub(in crate::rtf::parser) const WMF_NOTPATCOPY_RASTER_OP: u32 = 0x000f_0001;
pub(in crate::rtf::parser) const WMF_PATCOPY_RASTER_OP: u32 = 0x00f0_0021;
pub(in crate::rtf::parser) const WMF_PATINVERT_RASTER_OP: u32 = 0x005a_0049;
pub(in crate::rtf::parser) const WMF_PATPAINT_RASTER_OP: u32 = 0x00fb_0a09;
pub(in crate::rtf::parser) const WMF_SRCAND_RASTER_OP: u32 = 0x0088_00c6;
pub(in crate::rtf::parser) const WMF_SRCCOPY_RASTER_OP: u32 = 0x00cc_0020;
pub(in crate::rtf::parser) const WMF_SRCERASE_RASTER_OP: u32 = 0x0044_0328;
pub(in crate::rtf::parser) const WMF_SRCINVERT_RASTER_OP: u32 = 0x0066_0046;
pub(in crate::rtf::parser) const WMF_SRCPAINT_RASTER_OP: u32 = 0x00ee_0086;
pub(in crate::rtf::parser) const WMF_WHITENESS_RASTER_OP: u32 = 0x00ff_0062;
pub(in crate::rtf::parser) const WMF_ESCAPE_MFCOMMENT: u16 = 0x000f;
pub(in crate::rtf::parser) const PLACEABLE_WMF_KEY: u32 = 0x9ac6_cdd7;
pub(in crate::rtf::parser) const PLACEABLE_WMF_HEADER_BYTES: usize = 22;
pub(in crate::rtf::parser) const EMF_RGN_AND: u32 = 1;
pub(in crate::rtf::parser) const EMF_RGN_OR: u32 = 2;
pub(in crate::rtf::parser) const EMF_RGN_XOR: u32 = 3;
pub(in crate::rtf::parser) const EMF_RGN_DIFF: u32 = 4;
pub(in crate::rtf::parser) const EMF_RGN_COPY: u32 = 5;

#[derive(Debug, Copy, Clone)]
pub(in crate::rtf::parser) struct WmfCoordinateMap {
    pub(in crate::rtf::parser) window_origin_x: i32,
    pub(in crate::rtf::parser) window_origin_y: i32,
    pub(in crate::rtf::parser) viewport_origin_x: i32,
    pub(in crate::rtf::parser) viewport_origin_y: i32,
    pub(in crate::rtf::parser) viewport_width: i32,
    pub(in crate::rtf::parser) viewport_height: i32,
    pub(in crate::rtf::parser) window_width: i32,
    pub(in crate::rtf::parser) window_height: i32,
    pub(in crate::rtf::parser) image_width: i32,
    pub(in crate::rtf::parser) image_height: i32,
}

impl WmfCoordinateMap {
    pub(in crate::rtf::parser) fn normalized_point(self, x: i32, y: i32) -> (f32, f32) {
        (
            self.normalized_axis(
                x,
                self.window_origin_x,
                self.window_width,
                self.viewport_origin_x,
                self.viewport_width,
                self.image_width,
            ),
            self.normalized_axis(
                y,
                self.window_origin_y,
                self.window_height,
                self.viewport_origin_y,
                self.viewport_height,
                self.image_height,
            ),
        )
    }

    pub(in crate::rtf::parser) fn normalized_axis(
        self,
        value: i32,
        window_origin: i32,
        window_extent: i32,
        viewport_origin: i32,
        viewport_extent: i32,
        image_extent: i32,
    ) -> f32 {
        let logical_extent = window_extent.max(1);
        let image_extent = image_extent.max(1);
        let viewport_extent = viewport_extent.clamp(-image_extent, image_extent);
        let logical = i128::from(value.saturating_sub(window_origin));
        let scaled = logical
            .saturating_mul(i128::from(viewport_extent))
            .saturating_div(i128::from(logical_extent));
        let device = i128::from(viewport_origin).saturating_add(scaled);
        device.clamp(0, i128::from(image_extent)) as f32
    }

    pub(in crate::rtf::parser) fn normalized_x_distance(self, value: i32) -> f32 {
        let logical_extent = self.window_width.max(1);
        let image_extent = self.image_width.max(1);
        let viewport_extent = self.viewport_width.clamp(-image_extent, image_extent);
        let scaled = i128::from(value)
            .saturating_mul(i128::from(viewport_extent))
            .saturating_div(i128::from(logical_extent));
        scaled.clamp(-i128::from(image_extent), i128::from(image_extent)) as f32
    }

    pub(in crate::rtf::parser) fn normalized_y_distance(self, value: i32) -> f32 {
        let logical_extent = self.window_height.max(1);
        let image_extent = self.image_height.max(1);
        let viewport_extent = self.viewport_height.clamp(-image_extent, image_extent);
        let scaled = i128::from(value)
            .saturating_mul(i128::from(viewport_extent))
            .saturating_div(i128::from(logical_extent));
        scaled.clamp(-i128::from(image_extent), i128::from(image_extent)) as f32
    }
}

pub(in crate::rtf::parser) fn crop_parsed_dib(
    dib: ParsedDib,
    source_x: i32,
    source_y: i32,
    source_width: i32,
    source_height: i32,
) -> Option<ParsedDib> {
    if source_x < 0 || source_y < 0 || source_width <= 0 || source_height <= 0 {
        return None;
    }
    let source_x = u32::try_from(source_x).ok()?;
    let source_y = u32::try_from(source_y).ok()?;
    let source_width = u32::try_from(source_width).ok()?;
    let source_height = u32::try_from(source_height).ok()?;
    let source_right = source_x.checked_add(source_width)?;
    let source_bottom = source_y.checked_add(source_height)?;
    if source_right > dib.width_px || source_bottom > dib.height_px {
        return None;
    }
    if source_x == 0
        && source_y == 0
        && source_width == dib.width_px
        && source_height == dib.height_px
    {
        return Some(dib);
    }

    let crop_pixels = usize::try_from(source_width)
        .ok()?
        .checked_mul(usize::try_from(source_height).ok()?)?;
    if crop_pixels == 0 || crop_pixels > MAX_PASSIVE_VECTOR_RASTER_PIXELS {
        return None;
    }
    let crop_len = crop_pixels.checked_mul(3)?;
    let mut rgb = Vec::with_capacity(crop_len);
    let mut alpha = dib.alpha.as_ref().map(|_| Vec::with_capacity(crop_pixels));
    let source_row_stride = usize::try_from(dib.width_px).ok()?.checked_mul(3)?;
    let source_alpha_row_stride = usize::try_from(dib.width_px).ok()?;
    let crop_row_bytes = usize::try_from(source_width).ok()?.checked_mul(3)?;
    let source_x_bytes = usize::try_from(source_x).ok()?.checked_mul(3)?;
    for y in source_y..source_bottom {
        let row_start = usize::try_from(y)
            .ok()?
            .checked_mul(source_row_stride)?
            .checked_add(source_x_bytes)?;
        let row_end = row_start.checked_add(crop_row_bytes)?;
        rgb.extend_from_slice(dib.rgb.get(row_start..row_end)?);
        if let (Some(source_alpha), Some(cropped_alpha)) = (dib.alpha.as_ref(), alpha.as_mut()) {
            let alpha_row_start = usize::try_from(y)
                .ok()?
                .checked_mul(source_alpha_row_stride)?
                .checked_add(usize::try_from(source_x).ok()?)?;
            let alpha_row_end = alpha_row_start.checked_add(usize::try_from(source_width).ok()?)?;
            cropped_alpha.extend_from_slice(source_alpha.get(alpha_row_start..alpha_row_end)?);
        }
    }

    Some(ParsedDib {
        width_px: source_width,
        height_px: source_height,
        rgb,
        alpha,
        top_down: dib.top_down,
    })
}

pub(in crate::rtf::parser) fn crop_parsed_dib_scan_lines(
    dib: ParsedDib,
    source_x: i32,
    source_y: i32,
    source_width: i32,
    start_scan: u32,
    scan_count: u32,
) -> Option<ParsedDib> {
    let scan_bottom = start_scan.checked_add(scan_count)?;
    if scan_count == 0 || scan_bottom > dib.height_px {
        return None;
    }
    let scan_top_y = if dib.top_down {
        start_scan
    } else {
        dib.height_px.checked_sub(scan_bottom)?
    };
    let crop_y = i32::try_from(scan_top_y).ok()?.checked_add(source_y)?;
    crop_parsed_dib(
        dib,
        source_x,
        crop_y,
        source_width,
        i32::try_from(scan_count).ok()?,
    )
}

pub(in crate::rtf::parser) fn crop_compressed_vector_raster_image(
    image: ParsedCompressedDibImage,
    source_x: i32,
    source_y: i32,
    source_width: i32,
    source_height: i32,
) -> Option<ParsedCompressedDibImage> {
    if source_x < 0 || source_y < 0 || source_width <= 0 || source_height <= 0 {
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
        return Some(image);
    }
    let crop_pixels = usize::try_from(source_width)
        .ok()?
        .checked_mul(usize::try_from(source_height).ok()?)?;
    if crop_pixels == 0 || crop_pixels > MAX_PASSIVE_VECTOR_RASTER_PIXELS {
        return None;
    }
    let components = match image.format {
        ImageFormat::Png => 3usize,
        ImageFormat::PngGrayscale | ImageFormat::PngIndexed => 1usize,
        _ => return None,
    };
    let cropped_bytes = crop_png_scanline_idat(
        &image.bytes,
        image.width_px,
        image.height_px,
        components,
        source_x,
        source_y,
        source_width,
        source_height,
    )?;
    let alpha_mask = match image.alpha_mask {
        Some(mask) => Some(StaticImageAlphaMask {
            bytes: crop_png_scanline_idat(
                &mask.bytes,
                image.width_px,
                image.height_px,
                1,
                source_x,
                source_y,
                source_width,
                source_height,
            )?,
        }),
        None => None,
    };
    Some(ParsedCompressedDibImage {
        width_px: source_width,
        height_px: source_height,
        format: image.format,
        bytes: cropped_bytes,
        palette: image.palette,
        alpha_mask,
        top_down: image.top_down,
    })
}

pub(in crate::rtf::parser) fn crop_png_scanline_idat(
    idat: &[u8],
    width_px: u32,
    height_px: u32,
    components: usize,
    source_x: u32,
    source_y: u32,
    source_width: u32,
    source_height: u32,
) -> Option<Vec<u8>> {
    let (width, _height, row_len, scanlines) =
        unfiltered_png_scanlines(idat, width_px, height_px, components)?;
    let crop_width = usize::try_from(source_width).ok()?;
    let crop_height = usize::try_from(source_height).ok()?;
    let crop_row_pixels_len = crop_width.checked_mul(components)?;
    let crop_row_len = crop_row_pixels_len.checked_add(1)?;
    let source_x_bytes = usize::try_from(source_x).ok()?.checked_mul(components)?;
    let mut output = Vec::with_capacity(crop_row_len.checked_mul(crop_height)?);
    for y in source_y..source_y.checked_add(source_height)? {
        let row_start = usize::try_from(y).ok()?.checked_mul(row_len)?;
        let pixel_start = row_start.checked_add(1)?.checked_add(source_x_bytes)?;
        let pixel_end = pixel_start.checked_add(crop_row_pixels_len)?;
        output.push(0);
        output.extend_from_slice(scanlines.get(pixel_start..pixel_end)?);
    }
    if width == 0 || output.len() != crop_row_len.checked_mul(crop_height)? {
        return None;
    }
    Some(miniz_oxide::deflate::compress_to_vec_zlib(&output, 6))
}

pub(in crate::rtf::parser) fn parse_vector_raster_dib_image(
    bytes: &[u8],
    source_x: i32,
    source_y: i32,
    source_width: i32,
    source_height: i32,
    preserve_uncompressed_alpha: bool,
) -> Option<ParsedVectorRasterImage> {
    if let Some(dib) = parse_dib_image_data(bytes, MAX_PASSIVE_VECTOR_RASTER_PIXELS) {
        let dib = crop_parsed_dib(dib, source_x, source_y, source_width, source_height)?;
        let pixels = usize::try_from(dib.width_px)
            .ok()?
            .checked_mul(usize::try_from(dib.height_px).ok()?)?;
        if pixels > MAX_PASSIVE_VECTOR_RASTER_PIXELS {
            return None;
        }
        return Some(ParsedVectorRasterImage {
            width_px: dib.width_px,
            height_px: dib.height_px,
            format: ImageFormat::Rgb8,
            bytes: dib.rgb,
            palette: Vec::new(),
            alpha_mask: if preserve_uncompressed_alpha {
                match dib.alpha.as_ref() {
                    Some(alpha) => Some(alpha_samples_to_mask(alpha, dib.width_px, dib.height_px)?),
                    None => None,
                }
            } else {
                None
            },
        });
    }

    let compressed = parse_compressed_dib_image_data(bytes, MAX_PASSIVE_VECTOR_RASTER_PIXELS)?;
    let compressed = crop_compressed_vector_raster_image(
        compressed,
        source_x,
        source_y,
        source_width,
        source_height,
    )?;
    Some(ParsedVectorRasterImage {
        width_px: compressed.width_px,
        height_px: compressed.height_px,
        format: compressed.format,
        bytes: compressed.bytes,
        palette: compressed.palette,
        alpha_mask: compressed.alpha_mask,
    })
}

pub(in crate::rtf::parser) fn parse_vector_raster_dib_scan_lines(
    bytes: &[u8],
    source_x: i32,
    source_y: i32,
    source_width: i32,
    start_scan: u32,
    scan_count: u32,
) -> Option<ParsedVectorRasterImage> {
    if let Some(dib) = parse_dib_image_data(bytes, MAX_PASSIVE_VECTOR_RASTER_PIXELS) {
        let dib = crop_parsed_dib_scan_lines(
            dib,
            source_x,
            source_y,
            source_width,
            start_scan,
            scan_count,
        )?;
        let pixels = usize::try_from(dib.width_px)
            .ok()?
            .checked_mul(usize::try_from(dib.height_px).ok()?)?;
        if pixels > MAX_PASSIVE_VECTOR_RASTER_PIXELS {
            return None;
        }
        return Some(ParsedVectorRasterImage {
            width_px: dib.width_px,
            height_px: dib.height_px,
            format: ImageFormat::Rgb8,
            bytes: dib.rgb,
            palette: Vec::new(),
            alpha_mask: None,
        });
    }

    let compressed = parse_compressed_dib_image_data(bytes, MAX_PASSIVE_VECTOR_RASTER_PIXELS)?;
    let scan_bottom = start_scan.checked_add(scan_count)?;
    if scan_count == 0 || scan_bottom > compressed.height_px {
        return None;
    }
    let scan_top_y = if compressed.top_down {
        start_scan
    } else {
        compressed.height_px.checked_sub(scan_bottom)?
    };
    let source_y = i32::try_from(scan_top_y).ok()?.checked_add(source_y)?;
    let compressed = crop_compressed_vector_raster_image(
        compressed,
        source_x,
        source_y,
        source_width,
        i32::try_from(scan_count).ok()?,
    )?;
    Some(ParsedVectorRasterImage {
        width_px: compressed.width_px,
        height_px: compressed.height_px,
        format: compressed.format,
        bytes: compressed.bytes,
        palette: compressed.palette,
        alpha_mask: compressed.alpha_mask,
    })
}

#[allow(clippy::too_many_arguments)]
pub(in crate::rtf::parser) fn clipped_compressed_jpeg_source_raster_command(
    bytes: &[u8],
    source_x: i32,
    source_y: i32,
    source_width: i32,
    source_height: i32,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    transfer_mode: PassiveSourceRasterTransferMode,
) -> Option<PassiveSourceRasterCommand> {
    if transfer_mode != PassiveSourceRasterTransferMode::Copy
        || source_x < 0
        || source_y < 0
        || source_width <= 0
        || source_height <= 0
    {
        return None;
    }
    let image = parse_compressed_dib_image_data(bytes, MAX_PASSIVE_VECTOR_RASTER_PIXELS)?;
    if !matches!(
        image.format,
        ImageFormat::Jpeg
            | ImageFormat::JpegGrayscale
            | ImageFormat::JpegCmyk
            | ImageFormat::JpegInverted
            | ImageFormat::JpegGrayscaleInverted
            | ImageFormat::JpegCmykInverted
    ) || image.alpha_mask.is_some()
    {
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
                alpha_mask: None,
                tone_adjustment: None,
            },
        },
        StaticImageVectorCommand::RestoreState,
    ]))
}

pub(in crate::rtf::parser) fn read_le_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    Some(u16::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

pub(in crate::rtf::parser) fn read_le_i16(bytes: &[u8], offset: usize) -> Option<i16> {
    let end = offset.checked_add(2)?;
    Some(i16::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

pub(in crate::rtf::parser) fn read_le_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

pub(in crate::rtf::parser) fn read_le_i32(bytes: &[u8], offset: usize) -> Option<i32> {
    let end = offset.checked_add(4)?;
    Some(i32::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

pub(in crate::rtf::parser) fn read_le_f32(bytes: &[u8], offset: usize) -> Option<f32> {
    let end = offset.checked_add(4)?;
    Some(f32::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

pub(in crate::rtf::parser) fn parse_wmf_vector_image_data_with_limits(
    bytes: &[u8],
    word_compatible_passive: bool,
    shape_picture_result: bool,
    limits: VectorDecodeLimits,
) -> Option<ParsedWmfVector> {
    if limits.max_points_per_record < MAX_PASSIVE_WMF_POINTS_PER_RECORD
        || limits.max_raster_pixels < MAX_PASSIVE_VECTOR_RASTER_PIXELS
    {
        return None;
    }
    let header = parse_wmf_header_info(bytes)?;
    let header_size_words = read_le_u16(bytes, header.header_start + 2)?;
    let mut pos = header
        .header_start
        .checked_add(usize::from(header_size_words).checked_mul(2)?)?;
    let mut record_count = 0usize;
    let mut window_origin_x = header.window_origin_x;
    let mut window_origin_y = header.window_origin_y;
    let mut viewport_origin_x = 0i32;
    let mut viewport_origin_y = 0i32;
    let mut viewport_width = header.window_width;
    let mut viewport_height = header.window_height;
    let mut viewport_extent_explicit = false;
    let mut window_width = header.window_width;
    let mut window_height = header.window_height;
    let mut image_width = header.window_width;
    let mut image_height = header.window_height;
    let mut objects: Vec<Option<WmfObject>> = Vec::new();
    let mut state = WmfDrawingState::default();
    let mut saved_states: Vec<WmfSavedDrawingState> = Vec::new();
    let mut commands = Vec::new();
    let mut skipped_record_count = 0usize;
    let mut word_ignored_patcopy_family = false;
    let mut current_point = (0.0f32, 0.0f32);
    let mut replaceable_clip_command_start: Option<usize> = None;
    let mut clip_scope_command_start: Option<usize> = None;
    let word_coordinate_recovery = word_compatible_passive && !shape_picture_result;

    while pos + 6 <= header.file_end {
        record_count = record_count.checked_add(1)?;
        if record_count > MAX_PASSIVE_WMF_RECORDS {
            return None;
        }
        let record_size_words = read_le_u32(bytes, pos)? as usize;
        let function = read_le_u16(bytes, pos + 4)?;
        if record_size_words < 3 {
            return None;
        }
        let record_size_bytes = record_size_words.checked_mul(2)?;
        let record_end = pos.checked_add(record_size_bytes)?;
        if record_end > header.file_end {
            return None;
        }
        let data = &bytes[pos + 6..record_end];
        let coordinates = WmfCoordinateMap {
            window_origin_x,
            window_origin_y,
            viewport_origin_x,
            viewport_origin_y,
            viewport_width,
            viewport_height,
            window_width,
            window_height,
            image_width,
            image_height,
        };
        match function {
            0x0000 => break,
            0x001e => {
                if saved_states.len() >= MAX_PASSIVE_WMF_SAVED_STATES {
                    return None;
                }
                if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                    return None;
                }
                saved_states.push(WmfSavedDrawingState {
                    state,
                    current_point,
                    window_origin_x,
                    window_origin_y,
                    viewport_origin_x,
                    viewport_origin_y,
                    viewport_width,
                    viewport_height,
                    viewport_extent_explicit,
                    window_width,
                    window_height,
                    clip_scope_command_start,
                });
                commands.push(StaticImageVectorCommand::SaveState);
                clip_scope_command_start = commands.len().checked_sub(1);
            }
            0x0127 => {
                let Some(restore_index) = read_le_i16(data, 0) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                let had_saved_states = !saved_states.is_empty();
                if let Some(restored) = restore_wmf_saved_state(&mut saved_states, restore_index) {
                    if commands.len().checked_add(restored.restore_count)?
                        > MAX_PASSIVE_WMF_COMMANDS
                    {
                        return None;
                    }
                    state = restored.state.state;
                    current_point = restored.state.current_point;
                    window_origin_x = restored.state.window_origin_x;
                    window_origin_y = restored.state.window_origin_y;
                    viewport_origin_x = restored.state.viewport_origin_x;
                    viewport_origin_y = restored.state.viewport_origin_y;
                    viewport_width = restored.state.viewport_width;
                    viewport_height = restored.state.viewport_height;
                    viewport_extent_explicit = restored.state.viewport_extent_explicit;
                    window_width = restored.state.window_width;
                    window_height = restored.state.window_height;
                    clip_scope_command_start = restored.state.clip_scope_command_start;
                    for _ in 0..restored.restore_count {
                        commands.push(StaticImageVectorCommand::RestoreState);
                    }
                    replaceable_clip_command_start = None;
                } else if had_saved_states {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            0x020b => match (read_le_i16(data, 0), read_le_i16(data, 2)) {
                (Some(y), Some(x)) => {
                    window_origin_x = i32::from(x);
                    window_origin_y = i32::from(y);
                }
                _ => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            0x020c => match (read_le_i16(data, 0), read_le_i16(data, 2)) {
                (Some(height), Some(width)) => {
                    window_width = i32::from(width.unsigned_abs().max(1));
                    window_height = i32::from(height.unsigned_abs().max(1));
                    image_width = window_width;
                    image_height = window_height;
                    if !viewport_extent_explicit {
                        viewport_width = window_width;
                        viewport_height = window_height;
                    }
                }
                _ => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            0x020e => match (read_le_i16(data, 0), read_le_i16(data, 2)) {
                (Some(height), Some(width)) => {
                    let next_width = i32::from(width).clamp(-image_width, image_width);
                    let next_height = i32::from(height).clamp(-image_height, image_height);
                    if next_width == 0 || next_height == 0 {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                    } else {
                        viewport_width = next_width;
                        viewport_height = next_height;
                        viewport_extent_explicit = true;
                    }
                }
                _ => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            0x020f => match (read_le_i16(data, 0), read_le_i16(data, 2)) {
                (Some(y), Some(x)) => {
                    let y_offset = i32::from(y);
                    let x_offset = i32::from(x);
                    window_origin_x = window_origin_x.checked_add(x_offset)?;
                    window_origin_y = window_origin_y.checked_add(y_offset)?;
                }
                _ => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            0x0410 => {
                match (
                    read_le_i16(data, 0),
                    read_le_i16(data, 2),
                    read_le_i16(data, 4),
                    read_le_i16(data, 6),
                ) {
                    (Some(y_denom), Some(y_num), Some(x_denom), Some(x_num))
                        if x_denom != 0 && y_denom != 0 =>
                    {
                        if let (Some(width), Some(height)) = (
                            scale_wmf_logical_extent(window_width, x_num, x_denom, image_width),
                            scale_wmf_logical_extent(window_height, y_num, y_denom, image_height),
                        ) {
                            window_width = width;
                            window_height = height;
                            if !viewport_extent_explicit {
                                viewport_width = image_width;
                                viewport_height = image_height;
                            }
                        } else {
                            skipped_record_count = skipped_record_count.checked_add(1)?;
                        }
                    }
                    _ => skipped_record_count = skipped_record_count.checked_add(1)?,
                }
            }
            0x0412 => {
                match (
                    read_le_i16(data, 0),
                    read_le_i16(data, 2),
                    read_le_i16(data, 4),
                    read_le_i16(data, 6),
                ) {
                    (Some(y_denom), Some(y_num), Some(x_denom), Some(x_num))
                        if x_denom != 0 && y_denom != 0 =>
                    {
                        if let (Some(width), Some(height)) = (
                            scale_wmf_extent(viewport_width, x_num, x_denom, image_width),
                            scale_wmf_extent(viewport_height, y_num, y_denom, image_height),
                        ) {
                            viewport_width = width;
                            viewport_height = height;
                            viewport_extent_explicit = true;
                        } else {
                            skipped_record_count = skipped_record_count.checked_add(1)?;
                        }
                    }
                    _ => skipped_record_count = skipped_record_count.checked_add(1)?,
                }
            }
            0x01f0 => {
                if let Some(handle) = read_le_u16(data, 0).map(usize::from) {
                    if let Some(object) = objects.get_mut(handle) {
                        *object = None;
                    } else {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                    }
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            0x0035 => {}
            0x0037 | 0x0436 => {
                if validate_wmf_palette_entries(data).is_none() {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            0x0139 => {
                if read_le_u16(data, 0).is_none() {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            0x0234 => {
                if read_le_u16(data, 0).is_none() {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            0x0231 => {
                if read_le_u32(data, 0).is_none() {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            0x00f7 => {
                if let Some(object) = parse_wmf_palette_object(data) {
                    store_wmf_object(&mut objects, object)?;
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            0x02fa => {
                if let Some(object) = parse_wmf_pen_object(data) {
                    store_wmf_object(&mut objects, object)?;
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            0x02fc => {
                if let Some(object) = parse_wmf_brush_object(data) {
                    store_wmf_object(&mut objects, object)?;
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            0x02fb => {
                if let Some(object) = parse_wmf_font_object(data) {
                    store_wmf_object(&mut objects, object)?;
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            0x02fd => store_wmf_object(&mut objects, WmfObject::Other)?,
            0x01f9 => {
                if let Some(object) = parse_wmf_pattern_brush_object(data) {
                    store_wmf_object(&mut objects, object)?;
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            0x0142 => {
                if let Some(object) = parse_wmf_dib_pattern_brush_object(data) {
                    store_wmf_object(&mut objects, object)?;
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            0x012d => {
                if let Some(handle) = read_le_u16(data, 0).map(usize::from) {
                    if let Some(object) = objects.get(handle).and_then(|object| *object) {
                        match object {
                            WmfObject::Pen {
                                color,
                                width,
                                style,
                            } => {
                                state.stroke_color = color;
                                state.stroke_width = normalized_wmf_stroke_width(
                                    if word_compatible_passive && !shape_picture_result {
                                        width.max(4)
                                    } else {
                                        width
                                    },
                                    window_width,
                                );
                                state.stroke_style = style;
                                state.stock_pen_selected = false;
                            }
                            WmfObject::Brush { color, pattern } => {
                                state.fill_color = color;
                                state.fill_pattern = if word_compatible_passive
                                    && pattern == ShadingPattern::Horizontal
                                {
                                    ShadingPattern::WordWmfBitmapHorizontal
                                } else {
                                    pattern
                                };
                            }
                            WmfObject::Font {
                                height,
                                charset,
                                bold,
                            } => {
                                state.font_height = Some(height);
                                state.font_charset = charset;
                                state.font_bold = bold;
                            }
                            WmfObject::Other => {
                                skipped_record_count = skipped_record_count.checked_add(1)?
                            }
                        }
                    } else {
                        skipped_record_count = skipped_record_count.checked_add(1)?;
                    }
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            0x0209 => match color_from_colorref(data, 0) {
                Some(color) => state.text_color = Some(color),
                None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            0x0201 => match color_from_colorref(data, 0) {
                Some(color) => state.background_color = Some(color),
                None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            0x0103 => match read_le_u16(data, 0) {
                Some(WMF_MAPMODE_TEXT | WMF_MAPMODE_ISOTROPIC | WMF_MAPMODE_ANISOTROPIC) => {}
                Some(_) | None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            0x0104 => match read_le_u16(data, 0) {
                Some(
                    mode @ (WMF_ROP2_BLACK | WMF_ROP2_NOTCOPYPEN | WMF_ROP2_NOP | WMF_ROP2_COPYPEN
                    | WMF_ROP2_WHITE),
                ) => {
                    state.stroke_rop2 = mode;
                }
                Some(_) | None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            0x0105 => {}
            0x0107 => match read_le_u16(data, 0) {
                Some(
                    WMF_STRETCHMODE_BLACKONWHITE
                    | WMF_STRETCHMODE_WHITEONBLACK
                    | WMF_STRETCHMODE_COLORONCOLOR
                    | WMF_STRETCHMODE_HALFTONE,
                ) => {}
                Some(_) | None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            0x020d => match (read_le_i16(data, 0), read_le_i16(data, 2)) {
                (Some(y), Some(x)) => {
                    viewport_origin_y =
                        i32::from(y).clamp(-window_height.max(1), window_height.max(1));
                    viewport_origin_x =
                        i32::from(x).clamp(-window_width.max(1), window_width.max(1));
                }
                _ => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            0x0211 => match (read_le_i16(data, 0), read_le_i16(data, 2)) {
                (Some(y), Some(x)) => {
                    let y_offset = i32::from(y);
                    let x_offset = i32::from(x);
                    viewport_origin_y = viewport_origin_y
                        .checked_add(y_offset)?
                        .clamp(-window_height.max(1), window_height.max(1));
                    viewport_origin_x = viewport_origin_x
                        .checked_add(x_offset)?
                        .clamp(-window_width.max(1), window_width.max(1));
                }
                _ => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            0x0102 => match read_le_u16(data, 0) {
                Some(WMF_BKMODE_TRANSPARENT) => {
                    state.text_background_mode = WmfTextBackgroundMode::Transparent
                }
                Some(WMF_BKMODE_OPAQUE) => {
                    state.text_background_mode = WmfTextBackgroundMode::Opaque
                }
                Some(_) | None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            0x0106 => match read_le_u16(data, 0) {
                Some(WMF_POLYFILLMODE_ALTERNATE) => {
                    state.fill_rule = StaticImageVectorFillRule::Alternate
                }
                Some(WMF_POLYFILLMODE_WINDING) => {
                    state.fill_rule = StaticImageVectorFillRule::Winding
                }
                Some(_) | None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            0x0108 => match normalized_wmf_text_character_extra(data, 0, window_width) {
                Some(extra) => state.text_character_extra = extra,
                None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            0x020a => match normalized_wmf_text_justification(data, window_width) {
                Some(extra) => state.text_word_extra = extra,
                None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            0x012e => {
                if let Some(mode) = read_le_u16(data, 0).map(u32::from) {
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
            0x0214 => match parse_wmf_yx_point(data, 0, coordinates) {
                Some(mut point) => {
                    if word_coordinate_recovery
                        && state.stock_pen_selected
                        && !viewport_extent_explicit
                    {
                        point = word_compatible_wmf_path_point(point, coordinates);
                    } else if word_coordinate_recovery && !viewport_extent_explicit {
                        point = word_compatible_wmf_custom_line_point(point, coordinates);
                    }
                    current_point = point;
                }
                None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            0x0213 => match parse_wmf_yx_point(data, 0, coordinates) {
                Some(mut end) => {
                    if word_coordinate_recovery
                        && state.stock_pen_selected
                        && !viewport_extent_explicit
                    {
                        end = word_compatible_wmf_path_point(end, coordinates);
                    } else if word_coordinate_recovery && !viewport_extent_explicit {
                        end = word_compatible_wmf_custom_line_point(end, coordinates);
                    }
                    if segment_is_visible(current_point, end) {
                        if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                            return None;
                        }
                        commands.push(StaticImageVectorCommand::Line {
                            x1: current_point.0,
                            y1: current_point.1,
                            x2: end.0,
                            y2: end.1,
                            stroke_color: state.passive_stroke_color(),
                            stroke_width: state.stroke_width,
                            stroke_style: state.stroke_style,
                        });
                    }
                    current_point = end;
                }
                None => skipped_record_count = skipped_record_count.checked_add(1)?,
            },
            0x0415 | 0x0416 => {
                if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                    return None;
                }
                let Some(mut bounds) = parse_wmf_bounds(data, coordinates) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if word_coordinate_recovery && state.stock_pen_selected && !viewport_extent_explicit
                {
                    bounds = word_compatible_wmf_box_bounds(bounds, coordinates);
                }
                let (left, top, right, bottom) = bounds;
                if function != 0x0416 && !bounds_is_visible((left, top, right, bottom)) {
                    if function == 0x0415 {
                        pos = record_end;
                        continue;
                    }
                    return None;
                }
                if function == 0x0416 {
                    commands.push(StaticImageVectorCommand::ClipRect {
                        left,
                        top,
                        right,
                        bottom,
                    });
                } else {
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
                        commands.push(wmf_exclude_clip_rect_command(
                            window_width,
                            window_height,
                            left,
                            top,
                            right,
                            bottom,
                        )?);
                    }
                }
                if replaceable_clip_command_start.is_none() {
                    replaceable_clip_command_start = commands.len().checked_sub(1);
                }
            }
            0x0220 => {
                if is_passive_zero_wmf_offset_clip_region(data) {
                    pos = record_end;
                    continue;
                }
                let Some((offset_x, offset_y)) =
                    parse_wmf_offset_clip_region(data, window_width, window_height)
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
            0x0324 | 0x0325 => {
                let Some(mut points) = parse_wmf_point_list(data, coordinates) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if word_coordinate_recovery && state.stock_pen_selected && !viewport_extent_explicit
                {
                    for point in &mut points {
                        *point = word_compatible_wmf_path_point(*point, coordinates);
                    }
                }
                if function == 0x0324 {
                    if points.len() >= 3 && point_bounds_are_visible(&points) {
                        if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                            return None;
                        }
                        commands.push(StaticImageVectorCommand::Polygon {
                            points,
                            stroke_color: state.passive_stroke_color(),
                            stroke_width: state.stroke_width,
                            stroke_style: state.stroke_style,
                            fill_rule: state.fill_rule,
                            fill_pattern: state.fill_pattern,
                            fill_color: state.fill_color,
                        });
                    }
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
                        stroke_width: state.stroke_width,
                        stroke_style: state.stroke_style,
                    });
                }
            }
            0x0538 => {
                let Some(mut polygons) = parse_wmf_polypolygon_list(data, coordinates) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if word_coordinate_recovery && !viewport_extent_explicit {
                    for polygon in &mut polygons {
                        for point in polygon {
                            *point = word_compatible_wmf_path_point(*point, coordinates);
                        }
                    }
                }
                if let Some(command) = wmf_polypolygon_command(
                    polygons,
                    state.passive_stroke_color(),
                    state.stroke_width,
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
            }
            0x1005 => {
                let Some(points) = parse_wmf_point_list(data, coordinates) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if !polybezier_point_count_is_valid(points.len()) {
                    pos = record_end;
                    continue;
                }
                if word_compatible_passive {
                    pos = record_end;
                    continue;
                }
                if points
                    .windows(2)
                    .any(|pair| segment_is_visible(pair[0], pair[1]))
                {
                    if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                        return None;
                    }
                    commands.push(StaticImageVectorCommand::Bezier {
                        points,
                        stroke_color: state.passive_stroke_color(),
                        stroke_width: state.stroke_width,
                        stroke_style: state.stroke_style,
                    });
                }
            }
            0x1004 => {
                let Some(control_points) = parse_wmf_point_list(data, coordinates) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if !polybezierto_point_count_is_valid(control_points.len()) {
                    pos = record_end;
                    continue;
                }
                if word_compatible_passive {
                    pos = record_end;
                    continue;
                }
                let new_current_point = *control_points.last()?;
                let mut points = Vec::with_capacity(control_points.len().checked_add(1)?);
                points.push(current_point);
                points.extend(control_points);
                if points
                    .windows(2)
                    .any(|pair| segment_is_visible(pair[0], pair[1]))
                {
                    if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                        return None;
                    }
                    commands.push(StaticImageVectorCommand::Bezier {
                        points,
                        stroke_color: state.passive_stroke_color(),
                        stroke_width: state.stroke_width,
                        stroke_style: state.stroke_style,
                    });
                }
                current_point = new_current_point;
            }
            0x041b | 0x0418 | 0x061c => {
                let Some(mut bounds) = (if function == 0x061c {
                    parse_wmf_bounds_at(data, 4, coordinates)
                } else {
                    parse_wmf_bounds(data, coordinates)
                }) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                if word_coordinate_recovery && !viewport_extent_explicit {
                    bounds = if function == 0x0418 {
                        word_compatible_wmf_ellipse_bounds(bounds, coordinates)
                    } else {
                        word_compatible_wmf_box_bounds(bounds, coordinates)
                    };
                }
                if bounds_is_visible(bounds) {
                    let (left, top, right, bottom) = bounds;
                    let command = if function == 0x041b {
                        StaticImageVectorCommand::Rectangle {
                            left,
                            top,
                            right,
                            bottom,
                            stroke_color: state.passive_stroke_color(),
                            stroke_width: state.stroke_width,
                            stroke_style: state.stroke_style,
                            fill_pattern: state.fill_pattern,
                            fill_color: state.fill_color,
                        }
                    } else if function == 0x0418 {
                        StaticImageVectorCommand::Ellipse {
                            left,
                            top,
                            right,
                            bottom,
                            stroke_color: state.passive_stroke_color(),
                            stroke_width: state.stroke_width,
                            stroke_style: state.stroke_style,
                            fill_pattern: state.fill_pattern,
                            fill_color: state.fill_color,
                        }
                    } else {
                        let (Some(mut corner_width), Some(mut corner_height)) = (
                            normalized_wmf_size(data, 2, window_width),
                            normalized_wmf_size(data, 0, window_height),
                        ) else {
                            skipped_record_count = skipped_record_count.checked_add(1)?;
                            pos = record_end;
                            continue;
                        };
                        if word_coordinate_recovery && !viewport_extent_explicit {
                            corner_width = corner_width * WMF_WORD_BOX_X_SCALE
                                + 2.0
                                    * coordinates.image_width.max(1) as f32
                                    * WMF_WORD_BOX_X_OFFSET_RATIO;
                            corner_height = corner_height * WMF_WORD_BOX_Y_SCALE
                                + 2.0
                                    * coordinates.image_height.max(1) as f32
                                    * WMF_WORD_BOX_Y_OFFSET_RATIO;
                        }
                        StaticImageVectorCommand::RoundedRectangle {
                            left,
                            top,
                            right,
                            bottom,
                            corner_width,
                            corner_height,
                            stroke_color: state.passive_stroke_color(),
                            stroke_width: state.stroke_width,
                            stroke_style: state.stroke_style,
                            fill_pattern: state.fill_pattern,
                            fill_color: state.fill_color,
                        }
                    };
                    if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                        return None;
                    }
                    commands.push(command);
                }
            }
            0x0817 | 0x081a | 0x0830 => {
                let Some((points, center)) = parse_wmf_arc_record(
                    data,
                    coordinates,
                    word_coordinate_recovery && !viewport_extent_explicit,
                    function != 0x0817,
                ) else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                    pos = record_end;
                    continue;
                };
                match function {
                    0x0817 => {
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
                                stroke_width: state.stroke_width,
                                stroke_style: state.stroke_style,
                            });
                        }
                    }
                    0x081a => {
                        let mut pie_points = Vec::with_capacity(points.len().checked_add(1)?);
                        pie_points.push(center);
                        pie_points.extend(points);
                        if pie_points.len() >= 3 && point_bounds_are_visible(&pie_points) {
                            if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                                return None;
                            }
                            commands.push(StaticImageVectorCommand::Polygon {
                                points: pie_points,
                                stroke_color: state.passive_stroke_color(),
                                stroke_width: state.stroke_width,
                                stroke_style: state.stroke_style,
                                fill_rule: state.fill_rule,
                                fill_pattern: state.fill_pattern,
                                fill_color: state.fill_color,
                            });
                        }
                    }
                    0x0830 => {
                        if points.len() >= 3 && point_bounds_are_visible(&points) {
                            if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                                return None;
                            }
                            commands.push(StaticImageVectorCommand::Polygon {
                                points,
                                stroke_color: state.passive_stroke_color(),
                                stroke_width: state.stroke_width,
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
            0x0521 => {
                if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                    return None;
                }
                if let Some((x, y, text)) = parse_wmf_textout(data, coordinates, state.font_charset)
                {
                    commands.push(StaticImageVectorCommand::Text {
                        x,
                        y,
                        height: normalized_wmf_text_height(state.font_height, window_height),
                        bold: state.font_bold,
                        vertical_scale_percent: if state.font_height.is_none() { 81 } else { 100 },
                        text,
                        color: state.text_color,
                        background_color: match state.text_background_mode {
                            WmfTextBackgroundMode::Opaque => state.background_color,
                            WmfTextBackgroundMode::Transparent => None,
                        },
                        clip_bounds: None,
                        character_extra: state.text_character_extra,
                        word_extra: state.text_word_extra,
                        horizontal_align: state.text_horizontal_align,
                        vertical_align: state.text_vertical_align,
                    });
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            0x0a32 => {
                if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                    return None;
                }
                if let Some(ext_text_items) =
                    parse_wmf_exttextout(data, coordinates, state.font_charset)
                {
                    let required_commands = ext_text_items
                        .iter()
                        .map(|ext_text| {
                            usize::from(
                                ext_text
                                    .opaque_bounds
                                    .is_some_and(|bounds| bounds_is_visible(bounds)),
                            ) + usize::from(!ext_text.text.is_empty())
                        })
                        .try_fold(0usize, |total, count| total.checked_add(count));
                    let Some(required_commands) = required_commands else {
                        return None;
                    };
                    if commands.len().saturating_add(required_commands) > MAX_PASSIVE_WMF_COMMANDS {
                        return None;
                    }
                    for ext_text in ext_text_items {
                        if let Some((left, top, right, bottom)) = ext_text.opaque_bounds
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
                                fill_color: state.background_color,
                            });
                        }
                        if !ext_text.text.is_empty() {
                            commands.push(StaticImageVectorCommand::Text {
                                x: ext_text.x,
                                y: ext_text.y,
                                height: normalized_wmf_text_height(
                                    state.font_height,
                                    window_height,
                                ),
                                bold: state.font_bold,
                                vertical_scale_percent: if state.font_height.is_none() {
                                    81
                                } else {
                                    100
                                },
                                color: state.text_color,
                                background_color: if ext_text.opaque_bounds.is_none() {
                                    match state.text_background_mode {
                                        WmfTextBackgroundMode::Opaque => state.background_color,
                                        WmfTextBackgroundMode::Transparent => None,
                                    }
                                } else {
                                    None
                                },
                                clip_bounds: ext_text.clip_bounds.map(
                                    |(left, top, right, bottom)| {
                                        crate::model::StaticImageVectorTextBounds {
                                            left,
                                            top,
                                            right,
                                            bottom,
                                        }
                                    },
                                ),
                                character_extra: if state.font_height.is_none()
                                    && state.text_character_extra == 0.0
                                    && state.text_word_extra == 0.0
                                    && ext_text.text.chars().count() > 1
                                {
                                    // Word's stock-font recovery for a fontless
                                    // META_EXTTEXTOUT advances each subsequent
                                    // glyph by one metafile frame width. The
                                    // renderer clips that advance to the passive
                                    // image frame, matching GDI export without
                                    // reading beyond the bounded record.
                                    window_width.max(1) as f32
                                } else {
                                    state.text_character_extra
                                },
                                text: ext_text.text,
                                word_extra: state.text_word_extra,
                                horizontal_align: state.text_horizontal_align,
                                vertical_align: state.text_vertical_align,
                            });
                        }
                    }
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            0x061d => {
                if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                    return None;
                }
                if is_passive_invisible_wmf_patblt(data) {
                } else if let Some((left, top, right, bottom, fill_color)) =
                    parse_wmf_patblt_passive_transfer(
                        data,
                        coordinates,
                        state.fill_color,
                        emf_commands_are_unpainted_clip_scope(&commands),
                    )
                {
                    let (left, top, right, bottom) = if word_coordinate_recovery
                        && !viewport_extent_explicit
                    {
                        word_compatible_wmf_patblt_bounds((left, top, right, bottom), coordinates)
                    } else {
                        (left, top, right, bottom)
                    };
                    if bounds_is_visible((left, top, right, bottom)) {
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
                    }
                }
            }
            0x0d33 => {
                if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                    return None;
                }
                if word_compatible_passive || is_passive_noop_wmf_setdibits_to_device(data) {
                } else if let Some(command) = parse_wmf_setdibits_to_device(data, coordinates) {
                    commands.push(command);
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            0x0922 | 0x0940 | 0x0b23 | 0x0b41 | 0x0f43 => {
                if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                    return None;
                }
                let destination_offset = match function {
                    0x0922 | 0x0940 => 8,
                    0x0b23 | 0x0b41 => 12,
                    0x0f43 => 14,
                    _ => unreachable!(),
                };
                let blank_destination = emf_commands_are_unpainted_clip_scope(&commands);
                if is_passive_invisible_wmf_blt(data, destination_offset) {
                } else if word_compatible_passive
                    && matches!(function, 0x0922 | 0x0b23 | 0x0b41 | 0x0f43)
                    && read_le_u32(data, 0)? == WMF_PATCOPY_RASTER_OP
                {
                    word_ignored_patcopy_family = true;
                } else if let Some(command) = match function {
                    0x0940 => parse_wmf_dibbitblt_srcopy(
                        data,
                        coordinates,
                        state.fill_color,
                        blank_destination,
                        word_compatible_passive,
                    ),
                    0x0b41 => parse_wmf_dibstretchblt_srcopy(
                        data,
                        coordinates,
                        state.fill_color,
                        blank_destination,
                    ),
                    0x0f43 => parse_wmf_stretchdib_srcopy(
                        data,
                        coordinates,
                        state.fill_color,
                        blank_destination,
                        word_compatible_passive,
                    ),
                    _ => None,
                } {
                    push_passive_source_raster_command(&mut commands, command)?;
                } else if let Some((mut left, mut top, mut right, mut bottom, fill_color)) =
                    parse_wmf_blt_passive_transfer(
                        data,
                        destination_offset,
                        coordinates,
                        state.fill_color,
                        blank_destination,
                    )
                {
                    if word_coordinate_recovery
                        && !viewport_extent_explicit
                        && function == 0x0940
                        && read_le_u32(data, 0)? == WMF_PATCOPY_RASTER_OP
                    {
                        (left, top, right, bottom) = word_compatible_wmf_dibbitblt_patcopy_bounds(
                            (left, top, right, bottom),
                            coordinates,
                        );
                    }
                    if bounds_is_visible((left, top, right, bottom)) {
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
                    }
                } else if let Some(bounds) =
                    parse_wmf_blt_destination_bounds(data, destination_offset, coordinates)
                    && apply_passive_solid_backdrop_raster_transfer(
                        &mut commands,
                        bounds,
                        read_le_u32(data, 0)?,
                        state.fill_color,
                    )
                {
                } else if is_passive_noop_raster_transfer(data, 0, state.fill_color) {
                } else if matches!(function, 0x0b23 | 0x0b41 | 0x0f43)
                    && is_passive_noop_wmf_stretch_source_extent(data, destination_offset)
                {
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            0x041f => {
                if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                    return None;
                }
                if let Some((x1, y1, x2, y2, color)) = parse_wmf_setpixel_line(
                    data,
                    coordinates,
                    word_coordinate_recovery && !viewport_extent_explicit,
                ) && x2 > x1
                {
                    let line = if word_coordinate_recovery && !viewport_extent_explicit {
                        StaticImageVectorCommand::SquareCappedLine {
                            x1,
                            y1,
                            x2,
                            y2,
                            stroke_color: Some(color),
                            stroke_width: WMF_WORD_STOCK_PEN_LOGICAL_WIDTH,
                            stroke_style: BorderStyle::Single,
                        }
                    } else {
                        StaticImageVectorCommand::Line {
                            x1,
                            y1,
                            x2,
                            y2,
                            stroke_color: Some(color),
                            stroke_width: WMF_WORD_STOCK_PEN_LOGICAL_WIDTH,
                            stroke_style: BorderStyle::Single,
                        }
                    };
                    commands.push(line);
                } else {
                    skipped_record_count = skipped_record_count.checked_add(1)?;
                }
            }
            0x0626 if wmf_escape_is_non_visual_comment(data) => {}
            _ => {
                skipped_record_count = skipped_record_count.checked_add(1)?;
            }
        }

        pos = record_end;
    }

    if !saved_states.is_empty() {
        if commands.len().checked_add(saved_states.len())? > MAX_PASSIVE_WMF_COMMANDS {
            return None;
        }
        for _ in 0..saved_states.len() {
            commands.push(StaticImageVectorCommand::RestoreState);
        }
    }

    if commands.is_empty() && skipped_record_count > 0 && !word_ignored_patcopy_family {
        return None;
    }
    if commands.is_empty() && word_ignored_patcopy_family {
        skipped_record_count = 0;
    }

    Some(ParsedWmfVector {
        width_px: u32::try_from(image_width.max(1)).ok()?,
        height_px: u32::try_from(image_height.max(1)).ok()?,
        skipped_record_count,
        commands,
    })
}

pub(in crate::rtf::parser) fn store_wmf_object(
    objects: &mut Vec<Option<WmfObject>>,
    object: WmfObject,
) -> Option<()> {
    if let Some(slot) = objects.iter_mut().find(|slot| slot.is_none()) {
        *slot = Some(object);
        return Some(());
    }
    if objects.len() >= MAX_PASSIVE_WMF_OBJECTS {
        return None;
    }
    objects.push(Some(object));
    Some(())
}

pub(in crate::rtf::parser) fn restore_wmf_saved_state(
    saved_states: &mut Vec<WmfSavedDrawingState>,
    restore_index: i16,
) -> Option<RestoredWmfSavedDrawingState> {
    if saved_states.is_empty() {
        return None;
    }
    let restore_count = if restore_index < 0 {
        usize::from(restore_index.unsigned_abs())
    } else {
        1
    };
    if restore_count == 0 || restore_count > saved_states.len() {
        return None;
    }
    let mut restored = None;
    for _ in 0..restore_count {
        restored = saved_states.pop();
    }
    Some(RestoredWmfSavedDrawingState {
        state: restored?,
        restore_count,
    })
}

pub(in crate::rtf::parser) fn wmf_exclude_clip_rect_command(
    window_width: i32,
    window_height: i32,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
) -> Option<StaticImageVectorCommand> {
    let image_right = window_width.max(1) as f32;
    let image_bottom = window_height.max(1) as f32;
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

pub(in crate::rtf::parser) fn parse_wmf_offset_clip_region(
    data: &[u8],
    window_width: i32,
    window_height: i32,
) -> Option<(f32, f32)> {
    if data.len() < 4 {
        return None;
    }
    let y = i32::from(read_le_i16(data, 0)?);
    let x = i32::from(read_le_i16(data, 2)?);
    Some((
        x.clamp(-window_width.max(1), window_width.max(1)) as f32,
        y.clamp(-window_height.max(1), window_height.max(1)) as f32,
    ))
}

pub(in crate::rtf::parser) fn is_passive_zero_wmf_offset_clip_region(data: &[u8]) -> bool {
    data.len() >= 4
        && matches!(
            (read_le_i16(data, 0), read_le_i16(data, 2)),
            (Some(0), Some(0))
        )
}

pub(in crate::rtf::parser) fn offset_vector_clip_command(
    command: &mut StaticImageVectorCommand,
    offset_x: f32,
    offset_y: f32,
) -> Option<()> {
    match command {
        StaticImageVectorCommand::ClipRect {
            left,
            top,
            right,
            bottom,
        } => {
            *left += offset_x;
            *right += offset_x;
            *top += offset_y;
            *bottom += offset_y;
        }
        StaticImageVectorCommand::ClipPath {
            start, segments, ..
        } => {
            start.0 += offset_x;
            start.1 += offset_y;
            for segment in segments {
                match segment {
                    StaticImageVectorPathSegment::MoveTo(x, y)
                    | StaticImageVectorPathSegment::LineTo(x, y) => {
                        *x += offset_x;
                        *y += offset_y;
                    }
                    StaticImageVectorPathSegment::CubicTo {
                        control1,
                        control2,
                        end,
                    } => {
                        control1.0 += offset_x;
                        control1.1 += offset_y;
                        control2.0 += offset_x;
                        control2.1 += offset_y;
                        end.0 += offset_x;
                        end.1 += offset_y;
                    }
                }
            }
        }
        _ => return None,
    }
    Some(())
}

pub(in crate::rtf::parser) fn parse_wmf_header_info(bytes: &[u8]) -> Option<WmfHeaderInfo> {
    let mut header_start = 0usize;
    let mut window_origin_x = 0i32;
    let mut window_origin_y = 0i32;
    let mut window_width = 1i32;
    let mut window_height = 1i32;

    if bytes.len() >= PLACEABLE_WMF_HEADER_BYTES + 18 && read_le_u32(bytes, 0)? == PLACEABLE_WMF_KEY
    {
        if !placeable_wmf_checksum_is_valid(bytes)? {
            return None;
        }
        let left = i32::from(read_le_i16(bytes, 6)?);
        let top = i32::from(read_le_i16(bytes, 8)?);
        let right = i32::from(read_le_i16(bytes, 10)?);
        let bottom = i32::from(read_le_i16(bytes, 12)?);
        let width = right.checked_sub(left)?.unsigned_abs().max(1);
        let height = bottom.checked_sub(top)?.unsigned_abs().max(1);
        window_origin_x = left.min(right);
        window_origin_y = top.min(bottom);
        window_width = i32::try_from(width).ok()?;
        window_height = i32::try_from(height).ok()?;
        header_start = PLACEABLE_WMF_HEADER_BYTES;
    }

    if bytes.len().checked_sub(header_start)? < 18 {
        return None;
    }
    let file_type = read_le_u16(bytes, header_start)?;
    let header_size_words = read_le_u16(bytes, header_start + 2)?;
    let version = read_le_u16(bytes, header_start + 4)?;
    let file_size_words = read_le_u32(bytes, header_start + 6)? as usize;
    if file_type != 1 || header_size_words != 9 || version != 0x0300 {
        return None;
    }
    let file_end = header_start.checked_add(file_size_words.checked_mul(2)?)?;
    if file_end > bytes.len() {
        return None;
    }
    Some(WmfHeaderInfo {
        header_start,
        file_end,
        window_origin_x,
        window_origin_y,
        window_width,
        window_height,
    })
}

pub(in crate::rtf::parser) fn placeable_wmf_checksum_is_valid(bytes: &[u8]) -> Option<bool> {
    if bytes.len() < PLACEABLE_WMF_HEADER_BYTES {
        return None;
    }
    let mut checksum = 0u16;
    for offset in (0..20).step_by(2) {
        checksum ^= read_le_u16(bytes, offset)?;
    }
    Some(checksum == read_le_u16(bytes, 20)?)
}

pub(in crate::rtf::parser) fn parse_wmf_pen_object(data: &[u8]) -> Option<WmfObject> {
    if data.len() < 10 {
        return None;
    }
    let style = read_le_u16(data, 0)?;
    if !wmf_pen_style_is_supported(style) {
        return Some(WmfObject::Other);
    }
    let width = i32::from(read_le_i16(data, 2)?.unsigned_abs().max(1));
    let color = color_from_colorref(data, 6)?;
    Some(WmfObject::Pen {
        color: if style == 5 { None } else { Some(color) },
        width,
        style: wmf_pen_border_style(style),
    })
}

pub(in crate::rtf::parser) fn wmf_pen_style_is_supported(style: u16) -> bool {
    matches!(style & 0x000f, 0..=6)
}

pub(in crate::rtf::parser) fn wmf_pen_border_style(style: u16) -> BorderStyle {
    match style & 0x000f {
        1 | 3 | 4 => BorderStyle::Dashed,
        2 => BorderStyle::Dotted,
        _ => BorderStyle::Single,
    }
}

pub(in crate::rtf::parser) fn parse_wmf_brush_object(data: &[u8]) -> Option<WmfObject> {
    if data.len() < 8 {
        return None;
    }
    let style = read_le_u16(data, 0)?;
    let color = color_from_colorref(data, 2)?;
    if !matches!(style, 0 | 1 | 2) {
        return Some(WmfObject::Other);
    }
    let pattern = if style == 2 {
        wmf_hatch_shading_pattern(read_le_u16(data, 6)?)
    } else {
        ShadingPattern::None
    };
    Some(WmfObject::Brush {
        color: if style == 1 { None } else { Some(color) },
        pattern,
    })
}

pub(in crate::rtf::parser) fn wmf_hatch_shading_pattern(hatch: u16) -> ShadingPattern {
    match hatch {
        0 => ShadingPattern::Horizontal,
        1 => ShadingPattern::Vertical,
        2 => ShadingPattern::ForwardDiagonal,
        3 => ShadingPattern::BackwardDiagonal,
        4 => ShadingPattern::Cross,
        5 => ShadingPattern::DiagonalCross,
        _ => ShadingPattern::None,
    }
}

pub(in crate::rtf::parser) fn parse_wmf_font_object(data: &[u8]) -> Option<WmfObject> {
    let height = i32::from(read_le_i16(data, 0)?);
    let bold = read_le_i16(data, 8).is_some_and(|weight| weight >= 600);
    let charset = data.get(13).copied().map(i32::from);
    Some(WmfObject::Font {
        height,
        charset,
        bold,
    })
}

pub(in crate::rtf::parser) fn parse_wmf_palette_object(data: &[u8]) -> Option<WmfObject> {
    read_le_u16(data, 0)?;
    let entry_count = usize::from(read_le_u16(data, 2)?);
    let entries_len = entry_count.checked_mul(4)?;
    let entries_end = 4usize.checked_add(entries_len)?;
    if entries_end > data.len() || entry_count > MAX_PASSIVE_WMF_OBJECTS {
        return None;
    }
    Some(WmfObject::Other)
}

pub(in crate::rtf::parser) fn validate_wmf_palette_entries(data: &[u8]) -> Option<()> {
    read_le_u16(data, 0)?;
    let entry_count = usize::from(read_le_u16(data, 2)?);
    let entries_len = entry_count.checked_mul(4)?;
    let entries_end = 4usize.checked_add(entries_len)?;
    if entries_end > data.len() || entry_count > MAX_PASSIVE_WMF_OBJECTS {
        return None;
    }
    Some(())
}

pub(in crate::rtf::parser) fn parse_wmf_pattern_brush_object(data: &[u8]) -> Option<WmfObject> {
    read_le_u16(data, 0)?;
    Some(WmfObject::Other)
}

pub(in crate::rtf::parser) fn parse_wmf_dib_pattern_brush_object(data: &[u8]) -> Option<WmfObject> {
    read_le_u16(data, 0)?;
    read_le_u16(data, 2)?;
    Some(WmfObject::Other)
}

pub(in crate::rtf::parser) fn wmf_text_horizontal_align(
    mode: u16,
) -> StaticImageTextHorizontalAlign {
    match mode & WMF_TA_HORIZONTAL_MASK {
        WMF_TA_RIGHT => StaticImageTextHorizontalAlign::Right,
        WMF_TA_CENTER => StaticImageTextHorizontalAlign::Center,
        _ => StaticImageTextHorizontalAlign::Left,
    }
}

pub(in crate::rtf::parser) fn supported_wmf_text_align_mode(mode: u32) -> Option<u16> {
    let mode = u16::try_from(mode).ok()?;
    if mode & !WMF_TA_SUPPORTED_MASK != 0 {
        return None;
    }
    if !matches!(
        mode & WMF_TA_HORIZONTAL_MASK,
        0 | WMF_TA_RIGHT | WMF_TA_CENTER
    ) {
        return None;
    }
    if !matches!(
        mode & WMF_TA_VERTICAL_MASK,
        0 | WMF_TA_BOTTOM | WMF_TA_BASELINE
    ) {
        return None;
    }
    Some(mode)
}

pub(in crate::rtf::parser) fn wmf_text_vertical_align(mode: u16) -> StaticImageTextVerticalAlign {
    match mode & WMF_TA_VERTICAL_MASK {
        WMF_TA_BOTTOM => StaticImageTextVerticalAlign::Bottom,
        WMF_TA_BASELINE => StaticImageTextVerticalAlign::Baseline,
        _ => StaticImageTextVerticalAlign::Top,
    }
}

pub(in crate::rtf::parser) fn parse_wmf_bounds(
    data: &[u8],
    coordinates: WmfCoordinateMap,
) -> Option<(f32, f32, f32, f32)> {
    parse_wmf_bounds_at(data, 0, coordinates)
}

pub(in crate::rtf::parser) const WMF_WORD_BOX_X_SCALE: f32 = 0.868_055_2;
pub(in crate::rtf::parser) const WMF_WORD_BOX_Y_SCALE: f32 = 0.826_064_7;
pub(in crate::rtf::parser) const WMF_WORD_BOX_X_OFFSET_RATIO: f32 = 0.012_400_32;
pub(in crate::rtf::parser) const WMF_WORD_BOX_Y_OFFSET_RATIO: f32 = 0.026_223_445;
pub(in crate::rtf::parser) const WMF_WORD_ELLIPSE_X_SCALE: f32 = 0.793_703;
pub(in crate::rtf::parser) const WMF_WORD_ELLIPSE_Y_SCALE: f32 = 0.806_6;
pub(in crate::rtf::parser) const WMF_WORD_ELLIPSE_X_OFFSET_RATIO: f32 = 0.095_215;
pub(in crate::rtf::parser) const WMF_WORD_ELLIPSE_Y_OFFSET_RATIO: f32 = 0.056_37;
pub(in crate::rtf::parser) const WMF_WORD_PATH_X_SCALE: f32 = 0.986_110_45;
pub(in crate::rtf::parser) const WMF_WORD_PATH_Y_SCALE: f32 = 1.068_494_8;
pub(in crate::rtf::parser) const WMF_WORD_PATH_X_OFFSET_RATIO: f32 = 0.042_261_876;
pub(in crate::rtf::parser) const WMF_WORD_PATH_Y_OFFSET_RATIO: f32 = 0.073_890_485;
pub(in crate::rtf::parser) const WMF_WORD_ARC_X_SCALE: f32 = 0.763_906;
pub(in crate::rtf::parser) const WMF_WORD_ARC_Y_SCALE: f32 = 0.770_984_35;
pub(in crate::rtf::parser) const WMF_WORD_OPEN_ARC_X_OFFSET_RATIO: f32 = 0.064_482_32;
pub(in crate::rtf::parser) const WMF_WORD_OPEN_ARC_Y_OFFSET_RATIO: f32 = 0.103_166_096;
pub(in crate::rtf::parser) const WMF_WORD_CLOSED_ARC_X_OFFSET_RATIO: f32 = 0.022_817_705;
pub(in crate::rtf::parser) const WMF_WORD_CLOSED_ARC_Y_OFFSET_RATIO: f32 = 0.031_731_483;
pub(in crate::rtf::parser) const WMF_WORD_PATBLT_X_SCALE: f32 = 0.992_042;
pub(in crate::rtf::parser) const WMF_WORD_PATBLT_Y_SCALE: f32 = 1.088_440_2;
pub(in crate::rtf::parser) const WMF_WORD_PATBLT_X_OFFSET_RATIO: f32 = -0.001_985_4;
pub(in crate::rtf::parser) const WMF_WORD_PATBLT_Y_OFFSET_RATIO: f32 = -0.006_810_675;
pub(in crate::rtf::parser) const WMF_WORD_DIBBITBLT_PATCOPY_X_SCALE: f32 = 0.283_445_27;
pub(in crate::rtf::parser) const WMF_WORD_DIBBITBLT_PATCOPY_Y_SCALE: f32 = 1.247_168_1;
pub(in crate::rtf::parser) const WMF_WORD_DIBBITBLT_PATCOPY_X_OFFSET_RATIO: f32 = -0.014_172_263;
pub(in crate::rtf::parser) const WMF_WORD_DIBBITBLT_PATCOPY_Y_OFFSET_RATIO: f32 = -0.140_592_43;

pub(in crate::rtf::parser) fn word_compatible_wmf_box_bounds(
    (left, top, right, bottom): (f32, f32, f32, f32),
    coordinates: WmfCoordinateMap,
) -> (f32, f32, f32, f32) {
    let x_offset = coordinates.image_width.max(1) as f32 * WMF_WORD_BOX_X_OFFSET_RATIO;
    let y_offset = coordinates.image_height.max(1) as f32 * WMF_WORD_BOX_Y_OFFSET_RATIO;
    (
        left * WMF_WORD_BOX_X_SCALE + x_offset,
        top * WMF_WORD_BOX_Y_SCALE + y_offset,
        right * WMF_WORD_BOX_X_SCALE + x_offset,
        bottom * WMF_WORD_BOX_Y_SCALE + y_offset,
    )
}

pub(in crate::rtf::parser) fn word_compatible_wmf_ellipse_bounds(
    (left, top, right, bottom): (f32, f32, f32, f32),
    coordinates: WmfCoordinateMap,
) -> (f32, f32, f32, f32) {
    let x_offset = coordinates.image_width.max(1) as f32 * WMF_WORD_ELLIPSE_X_OFFSET_RATIO;
    let y_offset = coordinates.image_height.max(1) as f32 * WMF_WORD_ELLIPSE_Y_OFFSET_RATIO;
    (
        left * WMF_WORD_ELLIPSE_X_SCALE + x_offset,
        top * WMF_WORD_ELLIPSE_Y_SCALE + y_offset,
        right * WMF_WORD_ELLIPSE_X_SCALE + x_offset,
        bottom * WMF_WORD_ELLIPSE_Y_SCALE + y_offset,
    )
}

pub(in crate::rtf::parser) fn word_compatible_wmf_path_point(
    (x, y): (f32, f32),
    coordinates: WmfCoordinateMap,
) -> (f32, f32) {
    (
        x * WMF_WORD_PATH_X_SCALE
            + coordinates.image_width.max(1) as f32 * WMF_WORD_PATH_X_OFFSET_RATIO,
        y * WMF_WORD_PATH_Y_SCALE
            + coordinates.image_height.max(1) as f32 * WMF_WORD_PATH_Y_OFFSET_RATIO,
    )
}

pub(in crate::rtf::parser) fn word_compatible_wmf_setpixel_point(
    (x, y): (f32, f32),
    coordinates: WmfCoordinateMap,
) -> (f32, f32) {
    // Four independently positioned points in Word's 200x100 SETPIXEL probe
    // expose a slightly different horizontal device grid from ordinary stock
    // paths. Preserve the already closer stock-path vertical recovery.
    const X_SCALE: f32 = 0.983_647_94;
    const X_OFFSET_RATIO: f32 = 0.041_673_873;
    (
        x * X_SCALE + coordinates.image_width.max(1) as f32 * X_OFFSET_RATIO,
        y * WMF_WORD_PATH_Y_SCALE
            + coordinates.image_height.max(1) as f32 * WMF_WORD_PATH_Y_OFFSET_RATIO,
    )
}

pub(in crate::rtf::parser) fn word_compatible_wmf_custom_line_point(
    (x, y): (f32, f32),
    coordinates: WmfCoordinateMap,
) -> (f32, f32) {
    // Word's direct custom-pen MoveTo/LineTo playback uses a milder device
    // transform than its stock-pen path recovery. The bounded affine fit is
    // backed by four independent coordinates across the dashed-line and
    // POLYBEZIERTO registered references.
    const X_SCALE: f32 = 0.988_626_96;
    const Y_SCALE: f32 = 1.073_481_4;
    const X_OFFSET_RATIO: f32 = -0.000_695_731_4;
    const Y_OFFSET_RATIO: f32 = 0.002_410_996_5;
    (
        x * X_SCALE + coordinates.image_width.max(1) as f32 * X_OFFSET_RATIO,
        y * Y_SCALE + coordinates.image_height.max(1) as f32 * Y_OFFSET_RATIO,
    )
}

pub(in crate::rtf::parser) fn parse_wmf_bounds_at(
    data: &[u8],
    offset: usize,
    coordinates: WmfCoordinateMap,
) -> Option<(f32, f32, f32, f32)> {
    if data.len() < offset.checked_add(8)? {
        return None;
    }
    let bottom = i32::from(read_le_i16(data, offset)?);
    let right = i32::from(read_le_i16(data, offset.checked_add(2)?)?);
    let top = i32::from(read_le_i16(data, offset.checked_add(4)?)?);
    let left = i32::from(read_le_i16(data, offset.checked_add(6)?)?);
    let (left, top) = coordinates.normalized_point(left, top);
    let (right, bottom) = coordinates.normalized_point(right, bottom);
    Some((
        left.min(right),
        top.min(bottom),
        left.max(right),
        top.max(bottom),
    ))
}

pub(in crate::rtf::parser) fn normalized_wmf_size(
    data: &[u8],
    offset: usize,
    extent: i32,
) -> Option<f32> {
    let value = i32::from(read_le_i16(data, offset)?.unsigned_abs());
    Some(value.clamp(0, extent.max(1)) as f32)
}

pub(in crate::rtf::parser) fn scale_wmf_extent(
    extent: i32,
    numerator: i16,
    denominator: i16,
    image_extent: i32,
) -> Option<i32> {
    let image_extent = image_extent.max(1);
    let scaled = i128::from(extent)
        .checked_mul(i128::from(numerator))?
        .checked_div(i128::from(denominator))?;
    let clamped = scaled.clamp(-i128::from(image_extent), i128::from(image_extent));
    if clamped == 0 {
        return None;
    }
    i32::try_from(clamped).ok()
}

pub(in crate::rtf::parser) fn scale_wmf_logical_extent(
    extent: i32,
    numerator: i16,
    denominator: i16,
    image_extent: i32,
) -> Option<i32> {
    let image_extent = image_extent.max(1);
    let max_extent = i128::from(image_extent).checked_mul(64)?;
    let scaled = i128::from(extent)
        .checked_mul(i128::from(numerator))?
        .checked_div(i128::from(denominator))?;
    let clamped = scaled.clamp(1, max_extent);
    i32::try_from(clamped).ok()
}

pub(in crate::rtf::parser) fn parse_wmf_patblt_passive_transfer(
    data: &[u8],
    coordinates: WmfCoordinateMap,
    selected_fill_color: Option<Color>,
    blank_destination: bool,
) -> Option<(f32, f32, f32, f32, Color)> {
    let fill_color = passive_wmf_raster_transfer_color(
        read_le_u32(data, 0)?,
        selected_fill_color,
        blank_destination,
    )?;
    if data.len() < 12 {
        return None;
    }
    let height = i32::from(read_le_i16(data, 4)?.unsigned_abs().max(1));
    let width = i32::from(read_le_i16(data, 6)?.unsigned_abs().max(1));
    let y = i32::from(read_le_i16(data, 8)?);
    let x = i32::from(read_le_i16(data, 10)?);
    let left_top = coordinates.normalized_point(x, y);
    let right_bottom =
        coordinates.normalized_point(x.saturating_add(width), y.saturating_add(height));
    Some((
        left_top.0.min(right_bottom.0),
        left_top.1.min(right_bottom.1),
        left_top.0.max(right_bottom.0),
        left_top.1.max(right_bottom.1),
        fill_color,
    ))
}

pub(in crate::rtf::parser) fn is_passive_invisible_wmf_patblt(data: &[u8]) -> bool {
    data.len() >= 12
        && matches!(
            (read_le_i16(data, 4), read_le_i16(data, 6)),
            (Some(0), _) | (_, Some(0))
        )
}

pub(in crate::rtf::parser) fn parse_wmf_blt_passive_transfer(
    data: &[u8],
    destination_offset: usize,
    coordinates: WmfCoordinateMap,
    selected_fill_color: Option<Color>,
    blank_destination: bool,
) -> Option<(f32, f32, f32, f32, Color)> {
    let fill_color = passive_wmf_raster_transfer_color(
        read_le_u32(data, 0)?,
        selected_fill_color,
        blank_destination,
    )?;
    let (left, top, right, bottom) =
        parse_wmf_blt_destination_bounds(data, destination_offset, coordinates)?;
    Some((left, top, right, bottom, fill_color))
}

pub(in crate::rtf::parser) fn parse_wmf_blt_destination_bounds(
    data: &[u8],
    destination_offset: usize,
    coordinates: WmfCoordinateMap,
) -> Option<(f32, f32, f32, f32)> {
    if data.len() < destination_offset.checked_add(8)? {
        return None;
    }
    let height = i32::from(read_le_i16(data, destination_offset)?.unsigned_abs().max(1));
    let width = i32::from(
        read_le_i16(data, destination_offset + 2)?
            .unsigned_abs()
            .max(1),
    );
    let y = i32::from(read_le_i16(data, destination_offset + 4)?);
    let x = i32::from(read_le_i16(data, destination_offset + 6)?);
    let left_top = coordinates.normalized_point(x, y);
    let right_bottom =
        coordinates.normalized_point(x.saturating_add(width), y.saturating_add(height));
    Some((
        left_top.0.min(right_bottom.0),
        left_top.1.min(right_bottom.1),
        left_top.0.max(right_bottom.0),
        left_top.1.max(right_bottom.1),
    ))
}

pub(in crate::rtf::parser) fn is_passive_invisible_wmf_blt(
    data: &[u8],
    destination_offset: usize,
) -> bool {
    data.len() >= destination_offset.saturating_add(4)
        && matches!(
            (
                read_le_i16(data, destination_offset),
                read_le_i16(data, destination_offset + 2)
            ),
            (Some(0), _) | (_, Some(0))
        )
}

pub(in crate::rtf::parser) fn is_passive_noop_wmf_stretch_source_extent(
    data: &[u8],
    destination_offset: usize,
) -> bool {
    let Some(source_extent_offset) = destination_offset.checked_sub(8) else {
        return false;
    };
    data.len() >= source_extent_offset.saturating_add(4)
        && matches!(
            (
                read_le_i16(data, source_extent_offset),
                read_le_i16(data, source_extent_offset + 2)
            ),
            (Some(0), _) | (_, Some(0))
        )
}

pub(in crate::rtf::parser) fn parse_wmf_stretchdib_srcopy(
    data: &[u8],
    coordinates: WmfCoordinateMap,
    selected_fill_color: Option<Color>,
    blank_destination: bool,
    word_compatible_passive: bool,
) -> Option<PassiveSourceRasterCommand> {
    const DIB_RGB_COLORS: u16 = 0;
    const DIB_HEADER_OFFSET: usize = 22;

    if data.len() < DIB_HEADER_OFFSET {
        return None;
    }
    let transfer_mode = passive_source_raster_transfer_mode(
        read_le_u32(data, 0)?,
        selected_fill_color,
        blank_destination,
    )?;
    if read_le_u16(data, 4)? != DIB_RGB_COLORS {
        return None;
    }

    let source_height = i32::from(read_le_i16(data, 6)?);
    let source_width = i32::from(read_le_i16(data, 8)?);
    let mut source_y = i32::from(read_le_i16(data, 10)?);
    let source_x = i32::from(read_le_i16(data, 12)?);
    if source_width <= 0 || source_height == 0 {
        return None;
    }
    let source_height_abs = i32::try_from(source_height.unsigned_abs()).ok()?;
    if word_compatible_passive && source_height > 0 {
        source_y = word_compatible_wmf_stretchdib_source_y(
            &data[DIB_HEADER_OFFSET..],
            source_y,
            source_height_abs,
        )?;
    }

    let destination_height = i32::from(read_le_i16(data, 14)?.unsigned_abs().max(1));
    let destination_width = i32::from(read_le_i16(data, 16)?.unsigned_abs().max(1));
    let destination_y = i32::from(read_le_i16(data, 18)?);
    let destination_x = i32::from(read_le_i16(data, 20)?);
    let destination_geometry =
        if word_compatible_passive && read_le_u32(data, 0)? == WMF_SRCCOPY_RASTER_OP {
            word_compatible_wmf_srccopy_raster_geometry(0.982_161_8, 1.071_441_3)
        } else {
            WmfSourceTransferGeometry::identity()
        };

    wmf_source_dib_transfer_command(
        &data[DIB_HEADER_OFFSET..],
        source_x,
        source_y,
        source_width,
        source_height_abs,
        destination_x,
        destination_y,
        destination_width,
        destination_height,
        coordinates,
        destination_geometry,
        transfer_mode,
    )
}

pub(in crate::rtf::parser) fn word_compatible_wmf_stretchdib_source_y(
    dib_bytes: &[u8],
    source_y: i32,
    source_height: i32,
) -> Option<i32> {
    const BITMAPCOREHEADER_SIZE: u32 = 12;
    const BITMAPINFOHEADER_SIZE: u32 = 40;

    let header_size = read_le_u32(dib_bytes, 0)?;
    let raw_bitmap_height = match header_size {
        BITMAPCOREHEADER_SIZE => i32::from(read_le_u16(dib_bytes, 6)?),
        BITMAPINFOHEADER_SIZE.. => {
            if usize::try_from(header_size).ok()? > dib_bytes.len() {
                return None;
            }
            read_le_i32(dib_bytes, 8)?
        }
        _ => return None,
    };

    if raw_bitmap_height > 0 {
        raw_bitmap_height
            .checked_sub(source_y)?
            .checked_sub(source_height)
    } else {
        Some(source_y)
    }
}

pub(in crate::rtf::parser) fn parse_wmf_dibbitblt_srcopy(
    data: &[u8],
    coordinates: WmfCoordinateMap,
    selected_fill_color: Option<Color>,
    blank_destination: bool,
    word_compatible_passive: bool,
) -> Option<PassiveSourceRasterCommand> {
    const DIB_HEADER_OFFSET: usize = 16;

    if data.len() < DIB_HEADER_OFFSET {
        return None;
    }
    let transfer_mode = passive_source_raster_transfer_mode(
        read_le_u32(data, 0)?,
        selected_fill_color,
        blank_destination,
    )?;
    let source_y = i32::from(read_le_i16(data, 4)?);
    let source_x = i32::from(read_le_i16(data, 6)?);

    let destination_height = i32::from(read_le_i16(data, 8)?.unsigned_abs().max(1));
    let destination_width = i32::from(read_le_i16(data, 10)?.unsigned_abs().max(1));
    let destination_y = i32::from(read_le_i16(data, 12)?);
    let destination_x = i32::from(read_le_i16(data, 14)?);
    let destination_geometry =
        if word_compatible_passive && read_le_u32(data, 0)? == WMF_SRCCOPY_RASTER_OP {
            let (bitmap_width, _) = bounded_wmf_dib_dimensions(&data[DIB_HEADER_OFFSET..])?;
            let x_extent_scale = (bitmap_width as f32 / destination_width as f32) * 0.793_652;
            word_compatible_wmf_srccopy_raster_geometry(x_extent_scale, 1.360_547_8)
        } else {
            WmfSourceTransferGeometry::identity()
        };

    wmf_source_dib_transfer_command(
        &data[DIB_HEADER_OFFSET..],
        source_x,
        source_y,
        destination_width,
        destination_height,
        destination_x,
        destination_y,
        destination_width,
        destination_height,
        coordinates,
        destination_geometry,
        transfer_mode,
    )
}

pub(in crate::rtf::parser) fn parse_wmf_dibstretchblt_srcopy(
    data: &[u8],
    coordinates: WmfCoordinateMap,
    selected_fill_color: Option<Color>,
    blank_destination: bool,
) -> Option<PassiveSourceRasterCommand> {
    const DIB_HEADER_OFFSET: usize = 20;

    if data.len() < DIB_HEADER_OFFSET {
        return None;
    }
    let transfer_mode = passive_source_raster_transfer_mode(
        read_le_u32(data, 0)?,
        selected_fill_color,
        blank_destination,
    )?;
    let source_height = i32::from(read_le_i16(data, 4)?);
    let source_width = i32::from(read_le_i16(data, 6)?);
    let source_y = i32::from(read_le_i16(data, 8)?);
    let source_x = i32::from(read_le_i16(data, 10)?);
    if source_width <= 0 || source_height == 0 {
        return None;
    }

    let destination_height = i32::from(read_le_i16(data, 12)?.unsigned_abs().max(1));
    let destination_width = i32::from(read_le_i16(data, 14)?.unsigned_abs().max(1));
    let destination_y = i32::from(read_le_i16(data, 16)?);
    let destination_x = i32::from(read_le_i16(data, 18)?);

    wmf_source_dib_transfer_command(
        &data[DIB_HEADER_OFFSET..],
        source_x,
        source_y,
        source_width,
        i32::try_from(source_height.unsigned_abs()).ok()?,
        destination_x,
        destination_y,
        destination_width,
        destination_height,
        coordinates,
        WmfSourceTransferGeometry::identity(),
        transfer_mode,
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::rtf::parser) fn wmf_source_dib_transfer_command(
    dib_bytes: &[u8],
    source_x: i32,
    source_y: i32,
    source_width: i32,
    source_height: i32,
    destination_x: i32,
    destination_y: i32,
    destination_width: i32,
    destination_height: i32,
    coordinates: WmfCoordinateMap,
    destination_geometry: WmfSourceTransferGeometry,
    transfer_mode: PassiveSourceRasterTransferMode,
) -> Option<PassiveSourceRasterCommand> {
    let bounds = scaled_wmf_source_transfer_bounds(
        destination_x,
        destination_y,
        destination_width,
        destination_height,
        coordinates,
        destination_geometry,
    )?;
    if let PassiveSourceRasterTransferMode::SourceIgnoring {
        raster_op,
        selected_fill_color,
        blank_destination,
    } = transfer_mode
    {
        return source_ignoring_passive_source_raster_command(
            bounds,
            raster_op,
            selected_fill_color,
            blank_destination,
        );
    }
    let transfer = parse_vector_raster_dib_image(
        dib_bytes,
        source_x,
        source_y,
        source_width,
        source_height,
        false,
    )
    .and_then(|image| apply_passive_source_raster_transfer_mode(image, transfer_mode));
    if let Some(transfer) = transfer {
        return Some(passive_source_raster_command(
            bounds.0, bounds.1, bounds.2, bounds.3, transfer,
        ));
    }
    clipped_compressed_jpeg_source_raster_command(
        dib_bytes,
        source_x,
        source_y,
        source_width,
        source_height,
        bounds.0,
        bounds.1,
        bounds.2,
        bounds.3,
        transfer_mode,
    )
}

#[derive(Debug, Copy, Clone)]
pub(in crate::rtf::parser) struct WmfSourceTransferGeometry {
    pub(in crate::rtf::parser) origin_x_scale: f32,
    pub(in crate::rtf::parser) origin_y_scale: f32,
    pub(in crate::rtf::parser) origin_x_offset_ratio: f32,
    pub(in crate::rtf::parser) origin_y_offset_ratio: f32,
    pub(in crate::rtf::parser) extent_x_scale: f32,
    pub(in crate::rtf::parser) extent_y_scale: f32,
}

impl WmfSourceTransferGeometry {
    const fn identity() -> Self {
        Self {
            origin_x_scale: 1.0,
            origin_y_scale: 1.0,
            origin_x_offset_ratio: 0.0,
            origin_y_offset_ratio: 0.0,
            extent_x_scale: 1.0,
            extent_y_scale: 1.0,
        }
    }
}

pub(in crate::rtf::parser) fn word_compatible_wmf_srccopy_raster_geometry(
    extent_x_scale: f32,
    extent_y_scale: f32,
) -> WmfSourceTransferGeometry {
    // Word's legacy source-raster playback uses a shared device transform for
    // destination origins. Record-specific extent behavior remains explicit at
    // each call site so it cannot affect unrelated raster operations.
    WmfSourceTransferGeometry {
        origin_x_scale: 0.952_366_9,
        origin_y_scale: 1.088_299_2,
        origin_x_offset_ratio: 0.003_969_973,
        origin_y_offset_ratio: -0.006_766_688,
        extent_x_scale,
        extent_y_scale,
    }
}

pub(in crate::rtf::parser) fn scaled_wmf_source_transfer_bounds(
    destination_x: i32,
    destination_y: i32,
    destination_width: i32,
    destination_height: i32,
    coordinates: WmfCoordinateMap,
    geometry: WmfSourceTransferGeometry,
) -> Option<(f32, f32, f32, f32)> {
    let (left, top, right, bottom) = wmf_source_transfer_bounds(
        destination_x,
        destination_y,
        destination_width,
        destination_height,
        coordinates,
    )?;
    let values = [
        geometry.origin_x_scale,
        geometry.origin_y_scale,
        geometry.origin_x_offset_ratio,
        geometry.origin_y_offset_ratio,
        geometry.extent_x_scale,
        geometry.extent_y_scale,
    ];
    if values.iter().any(|value| !value.is_finite())
        || geometry.origin_x_scale <= 0.0
        || geometry.origin_y_scale <= 0.0
        || geometry.extent_x_scale <= 0.0
        || geometry.extent_y_scale <= 0.0
    {
        return None;
    }
    let width = right - left;
    let height = bottom - top;
    let left = left * geometry.origin_x_scale
        + coordinates.image_width.max(1) as f32 * geometry.origin_x_offset_ratio;
    let top = top * geometry.origin_y_scale
        + coordinates.image_height.max(1) as f32 * geometry.origin_y_offset_ratio;
    let right = left + (width * geometry.extent_x_scale);
    let bottom = top + (height * geometry.extent_y_scale);
    bounds_is_visible((left, top, right, bottom)).then_some((left, top, right, bottom))
}

pub(in crate::rtf::parser) fn bounded_wmf_dib_dimensions(dib_bytes: &[u8]) -> Option<(u32, u32)> {
    const BITMAPCOREHEADER_SIZE: u32 = 12;
    const BITMAPINFOHEADER_SIZE: u32 = 40;

    let header_size = read_le_u32(dib_bytes, 0)?;
    let (width, height) = match header_size {
        BITMAPCOREHEADER_SIZE => (
            u32::from(read_le_u16(dib_bytes, 4)?),
            u32::from(read_le_u16(dib_bytes, 6)?),
        ),
        BITMAPINFOHEADER_SIZE.. => {
            if usize::try_from(header_size).ok()? > dib_bytes.len() {
                return None;
            }
            let width = read_le_i32(dib_bytes, 4)?;
            let height = read_le_i32(dib_bytes, 8)?;
            if width <= 0 || height == 0 {
                return None;
            }
            (u32::try_from(width).ok()?, height.unsigned_abs())
        }
        _ => return None,
    };
    let pixels = usize::try_from(width)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?;
    (width > 0 && height > 0 && pixels <= MAX_PASSIVE_VECTOR_RASTER_PIXELS)
        .then_some((width, height))
}

#[allow(clippy::too_many_arguments)]
pub(in crate::rtf::parser) fn wmf_source_transfer_bounds(
    destination_x: i32,
    destination_y: i32,
    destination_width: i32,
    destination_height: i32,
    coordinates: WmfCoordinateMap,
) -> Option<(f32, f32, f32, f32)> {
    let left_top = coordinates.normalized_point(destination_x, destination_y);
    let right_bottom = coordinates.normalized_point(
        destination_x.saturating_add(destination_width),
        destination_y.saturating_add(destination_height),
    );
    let left = left_top.0.min(right_bottom.0);
    let top = left_top.1.min(right_bottom.1);
    let right = left_top.0.max(right_bottom.0);
    let bottom = left_top.1.max(right_bottom.1);
    bounds_is_visible((left, top, right, bottom)).then_some((left, top, right, bottom))
}

pub(in crate::rtf::parser) fn parse_wmf_setdibits_to_device(
    data: &[u8],
    coordinates: WmfCoordinateMap,
) -> Option<StaticImageVectorCommand> {
    const DIB_RGB_COLORS: u16 = 0;
    const DIB_HEADER_OFFSET: usize = 18;

    if data.len() < DIB_HEADER_OFFSET || read_le_u16(data, 0)? != DIB_RGB_COLORS {
        return None;
    }

    let scan_count = read_le_u16(data, 2)?;
    let start_scan = read_le_u16(data, 4)?;
    let source_y = i32::from(read_le_i16(data, 6)?);
    let source_x = i32::from(read_le_i16(data, 8)?);
    let source_height = i32::from(read_le_i16(data, 10)?.unsigned_abs());
    let source_width = i32::from(read_le_i16(data, 12)?.unsigned_abs());
    let destination_y = i32::from(read_le_i16(data, 14)?);
    let destination_x = i32::from(read_le_i16(data, 16)?);

    if source_width == 0 || source_height == 0 {
        return None;
    }
    if u32::from(scan_count) > u32::try_from(source_height).ok()? {
        return None;
    }

    let image = parse_vector_raster_dib_scan_lines(
        &data[DIB_HEADER_OFFSET..],
        source_x,
        source_y,
        source_width,
        u32::from(start_scan),
        u32::from(scan_count),
    )?;

    wmf_raster_image_command(
        destination_x,
        destination_y,
        source_width,
        i32::from(scan_count.max(1)),
        coordinates,
        image,
    )
}

pub(in crate::rtf::parser) fn is_passive_noop_wmf_setdibits_to_device(data: &[u8]) -> bool {
    data.len() >= 14
        && matches!(
            (
                read_le_u16(data, 2),
                read_le_i16(data, 10),
                read_le_i16(data, 12)
            ),
            (Some(0), _, _) | (_, Some(0), _) | (_, _, Some(0))
        )
}

pub(in crate::rtf::parser) fn wmf_raster_image_command(
    destination_x: i32,
    destination_y: i32,
    destination_width: i32,
    destination_height: i32,
    coordinates: WmfCoordinateMap,
    image: ParsedVectorRasterImage,
) -> Option<StaticImageVectorCommand> {
    let left_top = coordinates.normalized_point(destination_x, destination_y);
    let right_bottom = coordinates.normalized_point(
        destination_x.saturating_add(destination_width),
        destination_y.saturating_add(destination_height),
    );
    let left = left_top.0.min(right_bottom.0);
    let top = left_top.1.min(right_bottom.1);
    let right = left_top.0.max(right_bottom.0);
    let bottom = left_top.1.max(right_bottom.1);
    if !bounds_is_visible((left, top, right, bottom)) {
        return None;
    }

    Some(StaticImageVectorCommand::RasterImage {
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
    })
}

pub(in crate::rtf::parser) fn passive_wmf_raster_transfer_color(
    raster_op: u32,
    selected_fill_color: Option<Color>,
    blank_destination: bool,
) -> Option<Color> {
    match raster_op {
        WMF_BLACKNESS_RASTER_OP => Some(Color {
            red: 0,
            green: 0,
            blue: 0,
        }),
        WMF_WHITENESS_RASTER_OP => Some(Color {
            red: 255,
            green: 255,
            blue: 255,
        }),
        WMF_PATCOPY_RASTER_OP => selected_fill_color,
        WMF_PATPAINT_RASTER_OP if selected_fill_color.is_some_and(is_white_color) => Some(Color {
            red: 255,
            green: 255,
            blue: 255,
        }),
        WMF_SRCERASE_RASTER_OP | WMF_NOTSRCERASE_RASTER_OP if blank_destination => Some(Color {
            red: 0,
            green: 0,
            blue: 0,
        }),
        WMF_DSTINVERT_RASTER_OP if blank_destination => Some(Color {
            red: 0,
            green: 0,
            blue: 0,
        }),
        WMF_MERGEPAINT_RASTER_OP | WMF_SRCPAINT_RASTER_OP | WMF_PATPAINT_RASTER_OP
            if blank_destination =>
        {
            Some(Color {
                red: 255,
                green: 255,
                blue: 255,
            })
        }
        WMF_MERGECOPY_RASTER_OP if selected_fill_color == Some(Color::default()) => Some(Color {
            red: 0,
            green: 0,
            blue: 0,
        }),
        WMF_NOTPATCOPY_RASTER_OP => selected_fill_color.map(inverted_color),
        WMF_PATINVERT_RASTER_OP if blank_destination => selected_fill_color.map(inverted_color),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn is_passive_noop_raster_transfer(
    data: &[u8],
    raster_op_offset: usize,
    selected_fill_color: Option<Color>,
) -> bool {
    match read_le_u32(data, raster_op_offset) {
        Some(WMF_DSTCOPY_RASTER_OP) => true,
        Some(WMF_PATINVERT_RASTER_OP) if selected_fill_color == Some(Color::default()) => true,
        _ => false,
    }
}

pub(in crate::rtf::parser) fn passive_source_raster_transfer_mode(
    raster_op: u32,
    selected_fill_color: Option<Color>,
    blank_destination: bool,
) -> Option<PassiveSourceRasterTransferMode> {
    match raster_op {
        WMF_SRCCOPY_RASTER_OP => Some(PassiveSourceRasterTransferMode::Copy),
        WMF_NOTSRCCOPY_RASTER_OP => Some(PassiveSourceRasterTransferMode::Invert),
        WMF_MERGECOPY_RASTER_OP => selected_fill_color.map(|color| {
            if is_white_color(color) {
                PassiveSourceRasterTransferMode::Copy
            } else if color == Color::default() {
                PassiveSourceRasterTransferMode::Solid(Color::default())
            } else {
                PassiveSourceRasterTransferMode::Mask(color)
            }
        }),
        WMF_NOTPATCOPY_RASTER_OP => Some(PassiveSourceRasterTransferMode::SourceIgnoring {
            raster_op,
            selected_fill_color,
            blank_destination,
        }),
        WMF_PATINVERT_RASTER_OP => Some(PassiveSourceRasterTransferMode::SourceIgnoring {
            raster_op,
            selected_fill_color,
            blank_destination,
        }),
        WMF_SRCAND_RASTER_OP if blank_destination => Some(PassiveSourceRasterTransferMode::Copy),
        WMF_SRCAND_RASTER_OP => {
            Some(PassiveSourceRasterTransferMode::SameBoundsBackdropAndIfSolidSource)
        }
        WMF_SRCINVERT_RASTER_OP if blank_destination => {
            Some(PassiveSourceRasterTransferMode::Invert)
        }
        WMF_SRCINVERT_RASTER_OP => {
            Some(PassiveSourceRasterTransferMode::SameBoundsBackdropXorIfSolidSource)
        }
        WMF_SRCERASE_RASTER_OP if blank_destination => Some(PassiveSourceRasterTransferMode::Copy),
        WMF_SRCERASE_RASTER_OP => {
            Some(PassiveSourceRasterTransferMode::SameBoundsBackdropSourceAndNotDestIfSolidSource)
        }
        WMF_NOTSRCERASE_RASTER_OP => {
            Some(PassiveSourceRasterTransferMode::SameBoundsBackdropNorIfSolidSource)
        }
        WMF_SRCPAINT_RASTER_OP => {
            Some(PassiveSourceRasterTransferMode::SameBoundsBackdropOrIfSolidSource)
        }
        WMF_MERGEPAINT_RASTER_OP => {
            Some(PassiveSourceRasterTransferMode::SameBoundsBackdropOrNotSourceIfSolidSource)
        }
        WMF_PATPAINT_RASTER_OP => selected_fill_color.map(|pattern| {
            PassiveSourceRasterTransferMode::SameBoundsBackdropOrPatternOrNotSourceIfSolidSource {
                pattern,
                blank_destination,
            }
        }),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn apply_passive_source_raster_transfer_mode(
    mut image: ParsedVectorRasterImage,
    mode: PassiveSourceRasterTransferMode,
) -> Option<PassiveSourceRasterTransfer> {
    match mode {
        PassiveSourceRasterTransferMode::Copy => {
            if let Some(source) = vector_raster_solid_color(&image) {
                Some(PassiveSourceRasterTransfer::Solid(source))
            } else {
                Some(PassiveSourceRasterTransfer::Image(image))
            }
        }
        PassiveSourceRasterTransferMode::Mask(color) => {
            if let Some(source) = vector_raster_solid_color(&image) {
                return Some(PassiveSourceRasterTransfer::Solid(and_color(source, color)));
            }
            mask_vector_raster_image(&mut image, color)?;
            Some(PassiveSourceRasterTransfer::Image(image))
        }
        PassiveSourceRasterTransferMode::Solid(color) => {
            Some(PassiveSourceRasterTransfer::Solid(color))
        }
        PassiveSourceRasterTransferMode::SourceIgnoring { .. } => None,
        PassiveSourceRasterTransferMode::SameBoundsBackdropAndIfSolidSource => {
            let source = vector_raster_solid_color(&image)?;
            if source == white_color() {
                Some(PassiveSourceRasterTransfer::Noop)
            } else if source == Color::default() {
                Some(PassiveSourceRasterTransfer::Solid(Color::default()))
            } else {
                Some(PassiveSourceRasterTransfer::SameBoundsBackdropAnd(source))
            }
        }
        PassiveSourceRasterTransferMode::SameBoundsBackdropNorIfSolidSource => {
            let source = vector_raster_solid_color(&image)?;
            if source == white_color() {
                Some(PassiveSourceRasterTransfer::Solid(Color::default()))
            } else {
                Some(PassiveSourceRasterTransfer::SameBoundsBackdropNor(source))
            }
        }
        PassiveSourceRasterTransferMode::SameBoundsBackdropOrIfSolidSource => {
            let source = vector_raster_solid_color(&image)?;
            if source == Color::default() {
                Some(PassiveSourceRasterTransfer::Noop)
            } else if source == white_color() {
                Some(PassiveSourceRasterTransfer::Solid(white_color()))
            } else {
                Some(PassiveSourceRasterTransfer::SameBoundsBackdropOr(source))
            }
        }
        PassiveSourceRasterTransferMode::SameBoundsBackdropOrNotSourceIfSolidSource => {
            let source = inverted_color(vector_raster_solid_color(&image)?);
            if source == Color::default() {
                Some(PassiveSourceRasterTransfer::Noop)
            } else if source == white_color() {
                Some(PassiveSourceRasterTransfer::Solid(white_color()))
            } else {
                Some(PassiveSourceRasterTransfer::SameBoundsBackdropOr(source))
            }
        }
        PassiveSourceRasterTransferMode::SameBoundsBackdropOrPatternOrNotSourceIfSolidSource {
            pattern,
            blank_destination,
        } => {
            let source = or_color(pattern, inverted_color(vector_raster_solid_color(&image)?));
            if source == white_color() {
                Some(PassiveSourceRasterTransfer::Solid(white_color()))
            } else if blank_destination {
                Some(PassiveSourceRasterTransfer::Solid(source))
            } else if source == Color::default() {
                Some(PassiveSourceRasterTransfer::Noop)
            } else {
                Some(PassiveSourceRasterTransfer::SameBoundsBackdropOr(source))
            }
        }
        PassiveSourceRasterTransferMode::SameBoundsBackdropSourceAndNotDestIfSolidSource => {
            let source = vector_raster_solid_color(&image)?;
            if source == Color::default() {
                Some(PassiveSourceRasterTransfer::Noop)
            } else {
                Some(PassiveSourceRasterTransfer::SameBoundsBackdropSourceAndNotDest(source))
            }
        }
        PassiveSourceRasterTransferMode::SameBoundsBackdropXorIfSolidSource => {
            let source = vector_raster_solid_color(&image)?;
            if source == Color::default() {
                Some(PassiveSourceRasterTransfer::Noop)
            } else {
                Some(PassiveSourceRasterTransfer::SameBoundsBackdropXor(source))
            }
        }
        PassiveSourceRasterTransferMode::Invert => {
            if let Some(source) = vector_raster_solid_color(&image) {
                Some(PassiveSourceRasterTransfer::Solid(inverted_color(source)))
            } else {
                match image.format {
                    ImageFormat::Jpeg => {
                        image.format = ImageFormat::JpegInverted;
                        Some(PassiveSourceRasterTransfer::Image(image))
                    }
                    ImageFormat::JpegGrayscale => {
                        image.format = ImageFormat::JpegGrayscaleInverted;
                        Some(PassiveSourceRasterTransfer::Image(image))
                    }
                    ImageFormat::JpegCmyk => {
                        image.format = ImageFormat::JpegCmykInverted;
                        Some(PassiveSourceRasterTransfer::Image(image))
                    }
                    ImageFormat::Rgb8 => {
                        if !image.palette.is_empty() {
                            return None;
                        }
                        for byte in &mut image.bytes {
                            *byte = u8::MAX.saturating_sub(*byte);
                        }
                        Some(PassiveSourceRasterTransfer::Image(image))
                    }
                    ImageFormat::Png => {
                        invert_png_scanline_bytes(
                            &mut image.bytes,
                            image.width_px,
                            image.height_px,
                            3,
                        )?;
                        Some(PassiveSourceRasterTransfer::Image(image))
                    }
                    ImageFormat::PngGrayscale => {
                        invert_png_scanline_bytes(
                            &mut image.bytes,
                            image.width_px,
                            image.height_px,
                            1,
                        )?;
                        Some(PassiveSourceRasterTransfer::Image(image))
                    }
                    ImageFormat::PngIndexed => {
                        invert_indexed_palette(&mut image.palette)?;
                        Some(PassiveSourceRasterTransfer::Image(image))
                    }
                    _ => None,
                }
            }
        }
    }
}

pub(in crate::rtf::parser) fn passive_source_raster_command(
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    transfer: PassiveSourceRasterTransfer,
) -> PassiveSourceRasterCommand {
    match transfer {
        PassiveSourceRasterTransfer::Image(image) => {
            PassiveSourceRasterCommand::Draw(StaticImageVectorCommand::RasterImage {
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
            })
        }
        PassiveSourceRasterTransfer::Solid(color) => {
            PassiveSourceRasterCommand::Draw(StaticImageVectorCommand::Rectangle {
                left,
                top,
                right,
                bottom,
                stroke_color: None,
                stroke_width: 0.0,
                stroke_style: BorderStyle::Single,
                fill_pattern: ShadingPattern::None,
                fill_color: Some(color),
            })
        }
        PassiveSourceRasterTransfer::SameBoundsBackdropAnd(color) => {
            PassiveSourceRasterCommand::ReduceSameBoundsBackdrop {
                bounds: (left, top, right, bottom),
                raster_op: WMF_SRCAND_RASTER_OP,
                selected_fill_color: Some(color),
            }
        }
        PassiveSourceRasterTransfer::SameBoundsBackdropNor(color) => {
            PassiveSourceRasterCommand::ReduceSameBoundsBackdrop {
                bounds: (left, top, right, bottom),
                raster_op: WMF_NOTSRCERASE_RASTER_OP,
                selected_fill_color: Some(color),
            }
        }
        PassiveSourceRasterTransfer::SameBoundsBackdropOr(color) => {
            PassiveSourceRasterCommand::ReduceSameBoundsBackdrop {
                bounds: (left, top, right, bottom),
                raster_op: WMF_SRCPAINT_RASTER_OP,
                selected_fill_color: Some(color),
            }
        }
        PassiveSourceRasterTransfer::SameBoundsBackdropSourceAndNotDest(color) => {
            PassiveSourceRasterCommand::ReduceSameBoundsBackdrop {
                bounds: (left, top, right, bottom),
                raster_op: WMF_SRCERASE_RASTER_OP,
                selected_fill_color: Some(color),
            }
        }
        PassiveSourceRasterTransfer::SameBoundsBackdropXor(color) => {
            PassiveSourceRasterCommand::ReduceSameBoundsBackdrop {
                bounds: (left, top, right, bottom),
                raster_op: WMF_SRCINVERT_RASTER_OP,
                selected_fill_color: Some(color),
            }
        }
        PassiveSourceRasterTransfer::Noop => PassiveSourceRasterCommand::Noop,
    }
}

pub(in crate::rtf::parser) fn push_passive_source_raster_command(
    commands: &mut Vec<StaticImageVectorCommand>,
    command: PassiveSourceRasterCommand,
) -> Option<()> {
    match command {
        PassiveSourceRasterCommand::Draw(command) => {
            if commands.len() >= MAX_PASSIVE_WMF_COMMANDS {
                return None;
            }
            commands.push(command);
        }
        PassiveSourceRasterCommand::DrawMany(mut new_commands) => {
            let new_len = commands.len().checked_add(new_commands.len())?;
            if new_len > MAX_PASSIVE_WMF_COMMANDS {
                return None;
            }
            commands.append(&mut new_commands);
        }
        PassiveSourceRasterCommand::ReduceSameBoundsBackdrop {
            bounds,
            raster_op,
            selected_fill_color,
        } => {
            if raster_op == WMF_SRCAND_RASTER_OP {
                let source_color = selected_fill_color?;
                if !apply_passive_same_bounds_srcand_source_transfer(commands, bounds, source_color)
                {
                    return None;
                }
            } else if raster_op == WMF_NOTSRCERASE_RASTER_OP {
                let source_color = selected_fill_color?;
                if !apply_passive_same_bounds_notsrcerase_source_transfer(
                    commands,
                    bounds,
                    source_color,
                ) {
                    return None;
                }
            } else if raster_op == WMF_SRCPAINT_RASTER_OP {
                let source_color = selected_fill_color?;
                if !apply_passive_same_bounds_srcpaint_source_transfer(
                    commands,
                    bounds,
                    source_color,
                ) {
                    return None;
                }
            } else if raster_op == WMF_SRCERASE_RASTER_OP {
                let source_color = selected_fill_color?;
                if !apply_passive_same_bounds_srcerase_source_transfer(
                    commands,
                    bounds,
                    source_color,
                ) {
                    return None;
                }
            } else if raster_op == WMF_SRCINVERT_RASTER_OP {
                let source_color = selected_fill_color?;
                if !apply_passive_same_bounds_srcinvert_source_transfer(
                    commands,
                    bounds,
                    source_color,
                ) {
                    return None;
                }
            } else if !apply_passive_solid_backdrop_raster_transfer(
                commands,
                bounds,
                raster_op,
                selected_fill_color,
            ) {
                return None;
            }
        }
        PassiveSourceRasterCommand::Noop => {}
    }
    Some(())
}

pub(in crate::rtf::parser) fn mask_vector_raster_image(
    image: &mut ParsedVectorRasterImage,
    color: Color,
) -> Option<()> {
    match image.format {
        ImageFormat::Rgb8 => {
            if !image.palette.is_empty() {
                return None;
            }
            for pixel in image.bytes.chunks_exact_mut(3) {
                pixel[0] &= color.red;
                pixel[1] &= color.green;
                pixel[2] &= color.blue;
            }
            Some(())
        }
        ImageFormat::Png => {
            mask_png_scanline_bytes(&mut image.bytes, image.width_px, image.height_px, color)
        }
        ImageFormat::PngGrayscale => {
            if color.red != color.green || color.red != color.blue {
                return None;
            }
            mask_png_grayscale_scanline_bytes(
                &mut image.bytes,
                image.width_px,
                image.height_px,
                color.red,
            )
        }
        ImageFormat::PngIndexed => {
            mask_indexed_palette(&mut image.palette, color)?;
            Some(())
        }
        _ => None,
    }
}

pub(in crate::rtf::parser) fn vector_raster_solid_color(
    image: &ParsedVectorRasterImage,
) -> Option<Color> {
    if image.alpha_mask.is_some() {
        return None;
    }
    match image.format {
        ImageFormat::Rgb8 => {
            if !image.palette.is_empty() {
                return None;
            }
            let width = usize::try_from(image.width_px).ok()?;
            let height = usize::try_from(image.height_px).ok()?;
            let expected_len = width.checked_mul(height)?.checked_mul(3)?;
            if expected_len == 0 || image.bytes.len() != expected_len {
                return None;
            }
            let first = image.bytes.get(0..3)?;
            let color = Color {
                red: first[0],
                green: first[1],
                blue: first[2],
            };
            image
                .bytes
                .chunks_exact(3)
                .all(|pixel| pixel == first)
                .then_some(color)
        }
        ImageFormat::Png => {
            let (_, _, row_len, scanlines) =
                unfiltered_png_scanlines(&image.bytes, image.width_px, image.height_px, 3)?;
            let first = scanlines
                .chunks_exact(row_len)
                .flat_map(|row| row[1..].chunks_exact(3))
                .next()?;
            let color = Color {
                red: first[0],
                green: first[1],
                blue: first[2],
            };
            scanlines
                .chunks_exact(row_len)
                .all(|row| row[1..].chunks_exact(3).all(|pixel| pixel == first))
                .then_some(color)
        }
        ImageFormat::PngGrayscale => {
            let (_, _, row_len, scanlines) =
                unfiltered_png_scanlines(&image.bytes, image.width_px, image.height_px, 1)?;
            let first = *scanlines
                .chunks_exact(row_len)
                .flat_map(|row| row[1..].iter())
                .next()?;
            let color = Color {
                red: first,
                green: first,
                blue: first,
            };
            scanlines
                .chunks_exact(row_len)
                .all(|row| row[1..].iter().all(|sample| *sample == first))
                .then_some(color)
        }
        ImageFormat::PngIndexed => {
            if image.palette.is_empty() || image.palette.len() % 3 != 0 {
                return None;
            }
            let (_, _, row_len, scanlines) =
                unfiltered_png_scanlines(&image.bytes, image.width_px, image.height_px, 1)?;
            let first_index = *scanlines
                .chunks_exact(row_len)
                .flat_map(|row| row[1..].iter())
                .next()?;
            let palette_start = usize::from(first_index).checked_mul(3)?;
            let entry = image
                .palette
                .get(palette_start..palette_start.saturating_add(3))?;
            let color = Color {
                red: entry[0],
                green: entry[1],
                blue: entry[2],
            };
            scanlines
                .chunks_exact(row_len)
                .all(|row| row[1..].iter().all(|sample| *sample == first_index))
                .then_some(color)
        }
        _ => None,
    }
}

pub(in crate::rtf::parser) fn mask_png_scanline_bytes(
    bytes: &mut Vec<u8>,
    width_px: u32,
    height_px: u32,
    color: Color,
) -> Option<()> {
    let (_, _, row_len, mut scanlines) = unfiltered_png_scanlines(bytes, width_px, height_px, 3)?;
    for row in scanlines.chunks_exact_mut(row_len) {
        for pixel in row[1..].chunks_exact_mut(3) {
            pixel[0] &= color.red;
            pixel[1] &= color.green;
            pixel[2] &= color.blue;
        }
    }
    *bytes = miniz_oxide::deflate::compress_to_vec_zlib(&scanlines, 6);
    Some(())
}

pub(in crate::rtf::parser) fn mask_png_grayscale_scanline_bytes(
    bytes: &mut Vec<u8>,
    width_px: u32,
    height_px: u32,
    mask: u8,
) -> Option<()> {
    let (_, _, row_len, mut scanlines) = unfiltered_png_scanlines(bytes, width_px, height_px, 1)?;
    for row in scanlines.chunks_exact_mut(row_len) {
        for sample in &mut row[1..] {
            *sample &= mask;
        }
    }
    *bytes = miniz_oxide::deflate::compress_to_vec_zlib(&scanlines, 6);
    Some(())
}

pub(in crate::rtf::parser) fn mask_indexed_palette(palette: &mut [u8], color: Color) -> Option<()> {
    if palette.is_empty() || palette.len() % 3 != 0 {
        return None;
    }
    for entry in palette.chunks_exact_mut(3) {
        entry[0] &= color.red;
        entry[1] &= color.green;
        entry[2] &= color.blue;
    }
    Some(())
}

pub(in crate::rtf::parser) fn invert_png_scanline_bytes(
    bytes: &mut Vec<u8>,
    width_px: u32,
    height_px: u32,
    components: usize,
) -> Option<()> {
    let (_, _, row_len, mut scanlines) =
        unfiltered_png_scanlines(bytes, width_px, height_px, components)?;
    for row in scanlines.chunks_exact_mut(row_len) {
        for sample in &mut row[1..] {
            *sample = u8::MAX.saturating_sub(*sample);
        }
    }
    *bytes = miniz_oxide::deflate::compress_to_vec_zlib(&scanlines, 6);
    Some(())
}

pub(in crate::rtf::parser) fn invert_indexed_palette(palette: &mut [u8]) -> Option<()> {
    if palette.is_empty() || palette.len() % 3 != 0 {
        return None;
    }
    for sample in palette {
        *sample = u8::MAX.saturating_sub(*sample);
    }
    Some(())
}

pub(in crate::rtf::parser) fn inverted_color(color: Color) -> Color {
    Color {
        red: u8::MAX.saturating_sub(color.red),
        green: u8::MAX.saturating_sub(color.green),
        blue: u8::MAX.saturating_sub(color.blue),
    }
}

pub(in crate::rtf::parser) fn xor_color(lhs: Color, rhs: Color) -> Color {
    Color {
        red: lhs.red ^ rhs.red,
        green: lhs.green ^ rhs.green,
        blue: lhs.blue ^ rhs.blue,
    }
}

pub(in crate::rtf::parser) fn and_color(lhs: Color, rhs: Color) -> Color {
    Color {
        red: lhs.red & rhs.red,
        green: lhs.green & rhs.green,
        blue: lhs.blue & rhs.blue,
    }
}

pub(in crate::rtf::parser) fn or_color(lhs: Color, rhs: Color) -> Color {
    Color {
        red: lhs.red | rhs.red,
        green: lhs.green | rhs.green,
        blue: lhs.blue | rhs.blue,
    }
}

pub(in crate::rtf::parser) fn nor_color(lhs: Color, rhs: Color) -> Color {
    inverted_color(or_color(lhs, rhs))
}

pub(in crate::rtf::parser) fn apply_passive_solid_backdrop_raster_transfer(
    commands: &mut [StaticImageVectorCommand],
    bounds: (f32, f32, f32, f32),
    raster_op: u32,
    selected_fill_color: Option<Color>,
) -> bool {
    let Some(StaticImageVectorCommand::Rectangle {
        left,
        top,
        right,
        bottom,
        stroke_color: None,
        fill_pattern: ShadingPattern::None,
        fill_color: Some(fill_color),
        ..
    }) = commands.last_mut()
    else {
        return false;
    };
    if (*left, *top, *right, *bottom) != bounds {
        return false;
    }

    *fill_color = match raster_op {
        WMF_DSTINVERT_RASTER_OP => inverted_color(*fill_color),
        WMF_PATINVERT_RASTER_OP => xor_color(*fill_color, selected_fill_color.unwrap_or_default()),
        _ => return false,
    };
    true
}

pub(in crate::rtf::parser) fn apply_passive_same_bounds_invert_fills(
    commands: &mut [StaticImageVectorCommand],
    rects: &[(f32, f32, f32, f32)],
) -> bool {
    if rects.is_empty() || rects.iter().any(|bounds| !bounds_is_visible(*bounds)) {
        return false;
    }
    let Some(command_start) = commands.len().checked_sub(rects.len()) else {
        return false;
    };

    for (command, bounds) in commands[command_start..].iter().zip(rects) {
        let StaticImageVectorCommand::Rectangle {
            left,
            top,
            right,
            bottom,
            stroke_color: None,
            fill_pattern: ShadingPattern::None,
            fill_color: Some(_),
            ..
        } = command
        else {
            return false;
        };
        if (*left, *top, *right, *bottom) != *bounds {
            return false;
        }
    }

    for command in &mut commands[command_start..] {
        if let StaticImageVectorCommand::Rectangle {
            fill_color: Some(fill_color),
            ..
        } = command
        {
            *fill_color = inverted_color(*fill_color);
        }
    }
    true
}

pub(in crate::rtf::parser) fn apply_passive_same_bounds_srcand_source_transfer(
    commands: &mut [StaticImageVectorCommand],
    bounds: (f32, f32, f32, f32),
    source_color: Color,
) -> bool {
    let Some(StaticImageVectorCommand::Rectangle {
        left,
        top,
        right,
        bottom,
        stroke_color: None,
        fill_pattern: ShadingPattern::None,
        fill_color: Some(fill_color),
        ..
    }) = commands.last_mut()
    else {
        return false;
    };
    if (*left, *top, *right, *bottom) != bounds {
        return false;
    }

    *fill_color = and_color(*fill_color, source_color);
    true
}

pub(in crate::rtf::parser) fn apply_passive_same_bounds_notsrcerase_source_transfer(
    commands: &mut [StaticImageVectorCommand],
    bounds: (f32, f32, f32, f32),
    source_color: Color,
) -> bool {
    let Some(StaticImageVectorCommand::Rectangle {
        left,
        top,
        right,
        bottom,
        stroke_color: None,
        fill_pattern: ShadingPattern::None,
        fill_color: Some(fill_color),
        ..
    }) = commands.last_mut()
    else {
        return false;
    };
    if (*left, *top, *right, *bottom) != bounds {
        return false;
    }

    *fill_color = nor_color(*fill_color, source_color);
    true
}

pub(in crate::rtf::parser) fn apply_passive_same_bounds_srcpaint_source_transfer(
    commands: &mut [StaticImageVectorCommand],
    bounds: (f32, f32, f32, f32),
    source_color: Color,
) -> bool {
    let Some(StaticImageVectorCommand::Rectangle {
        left,
        top,
        right,
        bottom,
        stroke_color: None,
        fill_pattern: ShadingPattern::None,
        fill_color: Some(fill_color),
        ..
    }) = commands.last_mut()
    else {
        return false;
    };
    if (*left, *top, *right, *bottom) != bounds {
        return false;
    }

    *fill_color = or_color(*fill_color, source_color);
    true
}

pub(in crate::rtf::parser) fn apply_passive_same_bounds_srcerase_source_transfer(
    commands: &mut [StaticImageVectorCommand],
    bounds: (f32, f32, f32, f32),
    source_color: Color,
) -> bool {
    let Some(StaticImageVectorCommand::Rectangle {
        left,
        top,
        right,
        bottom,
        stroke_color: None,
        fill_pattern: ShadingPattern::None,
        fill_color: Some(fill_color),
        ..
    }) = commands.last_mut()
    else {
        return false;
    };
    if (*left, *top, *right, *bottom) != bounds {
        return false;
    }

    *fill_color = and_color(source_color, inverted_color(*fill_color));
    true
}

pub(in crate::rtf::parser) fn apply_passive_same_bounds_srcinvert_source_transfer(
    commands: &mut [StaticImageVectorCommand],
    bounds: (f32, f32, f32, f32),
    source_color: Color,
) -> bool {
    let Some(StaticImageVectorCommand::Rectangle {
        left,
        top,
        right,
        bottom,
        stroke_color: None,
        fill_pattern: ShadingPattern::None,
        fill_color: Some(fill_color),
        ..
    }) = commands.last_mut()
    else {
        return false;
    };
    if (*left, *top, *right, *bottom) != bounds {
        return false;
    }

    *fill_color = xor_color(*fill_color, source_color);
    true
}

pub(in crate::rtf::parser) fn is_white_color(color: Color) -> bool {
    color == white_color()
}

pub(in crate::rtf::parser) fn white_color() -> Color {
    Color {
        red: u8::MAX,
        green: u8::MAX,
        blue: u8::MAX,
    }
}

pub(in crate::rtf::parser) fn parse_wmf_setpixel_line(
    data: &[u8],
    coordinates: WmfCoordinateMap,
    word_compatible_passive: bool,
) -> Option<(f32, f32, f32, f32, Color)> {
    if data.len() < 8 {
        return None;
    }
    let color = color_from_colorref(data, 0)?;
    let y = i32::from(read_le_i16(data, 4)?);
    let x = i32::from(read_le_i16(data, 6)?);
    let mut point = coordinates.normalized_point(x, y);
    // Four independent points in Word's common 200x100 SETPIXEL frame define
    // a bounded legacy device transform (maximum horizontal fit residual 0.15
    // points after display scaling). Larger
    // windows use a different quantized device grid, so keep their generic
    // bounded mapping until that grid has independent reference coverage.
    if word_compatible_passive && coordinates.image_width == 200 && coordinates.image_height == 100
    {
        point = word_compatible_wmf_setpixel_point(point, coordinates);
    }
    let (x, y) = point;
    let max_x = coordinates.image_width.max(1) as f32;
    let max_y = coordinates.image_height.max(1) as f32;
    let x1 = x.clamp(0.0, max_x);
    let y = y.clamp(0.0, max_y);
    let x2 = (x1 + WMF_WORD_SETPIXEL_LOGICAL_LENGTH).min(max_x);
    let (x1, x2) = if x2 > x1 {
        (x1, x2)
    } else {
        ((x1 - WMF_WORD_SETPIXEL_LOGICAL_LENGTH).max(0.0), x1)
    };
    Some((x1, y, x2, y, color))
}

pub(in crate::rtf::parser) fn parse_wmf_arc_record(
    data: &[u8],
    coordinates: WmfCoordinateMap,
    word_compatible_passive: bool,
    closed: bool,
) -> Option<(Vec<(f32, f32)>, (f32, f32))> {
    if data.len() < 16 {
        return None;
    }
    let end_y = i32::from(read_le_i16(data, 0)?);
    let end_x = i32::from(read_le_i16(data, 2)?);
    let start_y = i32::from(read_le_i16(data, 4)?);
    let start_x = i32::from(read_le_i16(data, 6)?);
    let bottom = i32::from(read_le_i16(data, 8)?);
    let right = i32::from(read_le_i16(data, 10)?);
    let top = i32::from(read_le_i16(data, 12)?);
    let left = i32::from(read_le_i16(data, 14)?);
    let raw_points = sample_emf_arc_points(
        left,
        top,
        right,
        bottom,
        (start_x, start_y),
        (end_x, end_y),
        word_compatible_passive,
    )?;
    let mut points = Vec::with_capacity(raw_points.len());
    for (x, y) in raw_points {
        let point = coordinates.normalized_point(x, y);
        points.push(if word_compatible_passive {
            word_compatible_wmf_arc_point(point, coordinates, closed)
        } else {
            point
        });
    }
    let mut center = coordinates.normalized_point(
        left.saturating_add(right) / 2,
        top.saturating_add(bottom) / 2,
    );
    if word_compatible_passive {
        center = word_compatible_wmf_arc_point(center, coordinates, closed);
    }
    Some((points, center))
}

pub(in crate::rtf::parser) fn word_compatible_wmf_arc_point(
    (x, y): (f32, f32),
    coordinates: WmfCoordinateMap,
    closed: bool,
) -> (f32, f32) {
    let (x_offset_ratio, y_offset_ratio) = if closed {
        (
            WMF_WORD_CLOSED_ARC_X_OFFSET_RATIO,
            WMF_WORD_CLOSED_ARC_Y_OFFSET_RATIO,
        )
    } else {
        (
            WMF_WORD_OPEN_ARC_X_OFFSET_RATIO,
            WMF_WORD_OPEN_ARC_Y_OFFSET_RATIO,
        )
    };
    (
        x * WMF_WORD_ARC_X_SCALE + coordinates.image_width.max(1) as f32 * x_offset_ratio,
        y * WMF_WORD_ARC_Y_SCALE + coordinates.image_height.max(1) as f32 * y_offset_ratio,
    )
}

pub(in crate::rtf::parser) fn word_compatible_wmf_patblt_bounds(
    bounds: (f32, f32, f32, f32),
    coordinates: WmfCoordinateMap,
) -> (f32, f32, f32, f32) {
    word_compatible_wmf_affine_bounds(
        bounds,
        coordinates,
        WMF_WORD_PATBLT_X_SCALE,
        WMF_WORD_PATBLT_Y_SCALE,
        WMF_WORD_PATBLT_X_OFFSET_RATIO,
        WMF_WORD_PATBLT_Y_OFFSET_RATIO,
    )
}

pub(in crate::rtf::parser) fn word_compatible_wmf_dibbitblt_patcopy_bounds(
    bounds: (f32, f32, f32, f32),
    coordinates: WmfCoordinateMap,
) -> (f32, f32, f32, f32) {
    word_compatible_wmf_affine_bounds(
        bounds,
        coordinates,
        WMF_WORD_DIBBITBLT_PATCOPY_X_SCALE,
        WMF_WORD_DIBBITBLT_PATCOPY_Y_SCALE,
        WMF_WORD_DIBBITBLT_PATCOPY_X_OFFSET_RATIO,
        WMF_WORD_DIBBITBLT_PATCOPY_Y_OFFSET_RATIO,
    )
}

pub(in crate::rtf::parser) fn word_compatible_wmf_affine_bounds(
    (left, top, right, bottom): (f32, f32, f32, f32),
    coordinates: WmfCoordinateMap,
    x_scale: f32,
    y_scale: f32,
    x_offset_ratio: f32,
    y_offset_ratio: f32,
) -> (f32, f32, f32, f32) {
    let x_offset = coordinates.image_width.max(1) as f32 * x_offset_ratio;
    let y_offset = coordinates.image_height.max(1) as f32 * y_offset_ratio;
    (
        left * x_scale + x_offset,
        top * y_scale + y_offset,
        right * x_scale + x_offset,
        bottom * y_scale + y_offset,
    )
}

pub(in crate::rtf::parser) fn wmf_escape_is_non_visual_comment(data: &[u8]) -> bool {
    if data.len() < 4 || read_le_u16(data, 0) != Some(WMF_ESCAPE_MFCOMMENT) {
        return false;
    }
    let Some(byte_count) = read_le_u16(data, 2).map(usize::from) else {
        return false;
    };
    byte_count <= data.len().saturating_sub(4)
}

pub(in crate::rtf::parser) fn parse_wmf_textout(
    data: &[u8],
    coordinates: WmfCoordinateMap,
    font_charset: Option<i32>,
) -> Option<(f32, f32, String)> {
    let byte_count = usize::from(read_le_u16(data, 0)?);
    if byte_count == 0 || byte_count > MAX_PASSIVE_WMF_TEXT_BYTES {
        return None;
    }
    let text_start = 2usize;
    let text_end = text_start.checked_add(byte_count)?;
    let coordinate_offset = text_start.checked_add(byte_count.next_multiple_of(2))?;
    if data.len() < coordinate_offset.checked_add(4)? {
        return None;
    }
    let text = sanitize_wmf_text_bytes(data.get(text_start..text_end)?, font_charset)?;
    let y = i32::from(read_le_i16(data, coordinate_offset)?);
    let x = i32::from(read_le_i16(data, coordinate_offset + 2)?);
    let (x, y) = coordinates.normalized_point(x, y);
    Some((x, y, text))
}

pub(in crate::rtf::parser) fn parse_wmf_exttextout(
    data: &[u8],
    coordinates: WmfCoordinateMap,
    font_charset: Option<i32>,
) -> Option<Vec<ParsedWmfExtTextOut>> {
    if data.len() < 8 {
        return None;
    }
    let y = i32::from(read_le_i16(data, 0)?);
    let x = i32::from(read_le_i16(data, 2)?);
    let byte_count = usize::try_from(read_le_i16(data, 4)?).ok()?;
    if byte_count > MAX_PASSIVE_WMF_TEXT_BYTES {
        return None;
    }
    let flags = read_le_u16(data, 6)?;
    if flags & !WMF_ETO_SUPPORTED_MASK != 0 || flags & WMF_ETO_GLYPH_INDEX != 0 {
        return None;
    }
    let bounds = if flags & (WMF_ETO_OPAQUE | WMF_ETO_CLIPPED) != 0 {
        Some(parse_wmf_bounds_at(data, 8, coordinates)?)
    } else {
        None
    };
    let text_start = if flags & (WMF_ETO_OPAQUE | WMF_ETO_CLIPPED) != 0 {
        16usize
    } else {
        8usize
    };
    let text_end = text_start.checked_add(byte_count)?;
    if data.len() < text_end {
        return None;
    }
    let text_data_end = text_start.checked_add(byte_count.next_multiple_of(2))?;
    if data.len() < text_data_end {
        return None;
    }
    let text = if byte_count == 0 {
        String::new()
    } else {
        sanitize_wmf_text_bytes_allow_empty(data.get(text_start..text_end)?, font_charset)?
    };
    let (x, y) = coordinates.normalized_point(x, y);
    let text = ParsedWmfExtTextOut {
        x,
        y,
        text,
        opaque_bounds: if flags & WMF_ETO_OPAQUE != 0 {
            bounds
        } else {
            None
        },
        clip_bounds: if flags & WMF_ETO_CLIPPED != 0 {
            bounds
        } else {
            None
        },
    };
    if text.text.is_empty() {
        return Some(vec![text]);
    }
    let has_pdy = flags & WMF_ETO_PDY != 0;
    let spacing_values_per_char = if has_pdy { 2usize } else { 1usize };
    let spacing_stride = spacing_values_per_char.checked_mul(2)?;
    let dx_bytes = byte_count.checked_mul(spacing_stride)?;
    if data.len().checked_sub(text_data_end)? != dx_bytes {
        return Some(vec![text]);
    }
    split_wmf_exttextout_by_dx(data, text_data_end, byte_count, text, has_pdy, coordinates)
}

pub(in crate::rtf::parser) fn split_wmf_exttextout_by_dx(
    data: &[u8],
    dx_offset: usize,
    byte_count: usize,
    text: ParsedWmfExtTextOut,
    has_pdy: bool,
    coordinates: WmfCoordinateMap,
) -> Option<Vec<ParsedWmfExtTextOut>> {
    if byte_count > MAX_PASSIVE_WMF_POINTS_PER_RECORD {
        return None;
    }
    let chars: Vec<char> = text.text.chars().collect();
    if chars.len() != byte_count {
        return None;
    }
    let spacing_values_per_char = if has_pdy { 2usize } else { 1usize };
    let spacing_stride = spacing_values_per_char.checked_mul(2)?;
    let dx_end = dx_offset.checked_add(byte_count.checked_mul(spacing_stride)?)?;
    if dx_end != data.len() {
        return None;
    }
    let mut x = text.x;
    let mut y = text.y;
    let mut items = Vec::with_capacity(byte_count);
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
        let dx = i32::from(read_le_i16(data, spacing_offset)?);
        x += coordinates.normalized_x_distance(dx);
        if has_pdy {
            let dy = i32::from(read_le_i16(data, spacing_offset.checked_add(2)?)?);
            y += coordinates.normalized_y_distance(dy);
        }
    }
    Some(items)
}

pub(in crate::rtf::parser) fn sanitize_wmf_text_bytes(
    bytes: &[u8],
    font_charset: Option<i32>,
) -> Option<String> {
    let text = sanitize_wmf_text_bytes_allow_empty(bytes, font_charset)?;
    (!text.is_empty()).then_some(text)
}

pub(in crate::rtf::parser) fn sanitize_wmf_text_bytes_allow_empty(
    bytes: &[u8],
    font_charset: Option<i32>,
) -> Option<String> {
    let code_page = font_charset
        .and_then(CodePage::from_font_charset)
        .unwrap_or(CodePage::Windows1252);
    let mut text = String::new();
    for ch in decode_raw_bytes(bytes, code_page).chars() {
        match ch {
            '\0' => {}
            '\t' | '\n' | '\r' => text.push(' '),
            ch if ch.is_control() => {}
            ch => text.push(ch),
        }
    }
    let text = text.trim_end().to_string();
    Some(text)
}

pub(in crate::rtf::parser) fn normalized_wmf_text_height(
    font_height: Option<i32>,
    window_height: i32,
) -> f32 {
    // GDI's stock SYSTEM_FONT is roughly 6.52 points in Word's ordinary
    // 36-point inline metafile frame. Keep that ratio in normalized source
    // coordinates so display scaling remains bounded and deterministic.
    let fallback = (window_height.max(1) as f32 / 5.52).clamp(4.0, 48.0);
    font_height
        .map(|height| height.unsigned_abs().max(1) as f32)
        .unwrap_or(fallback)
        .clamp(1.0, window_height.max(1) as f32)
}

pub(in crate::rtf::parser) fn normalized_wmf_stroke_width(width: i32, window_width: i32) -> f32 {
    (width.clamp(1, window_width.max(1)) as f32 * WMF_WORD_AUTHORED_PEN_SCALE)
        .min(window_width.max(1) as f32)
}

pub(in crate::rtf::parser) fn normalized_wmf_text_character_extra(
    data: &[u8],
    offset: usize,
    window_width: i32,
) -> Option<f32> {
    let value = i32::from(read_le_i16(data, offset)?);
    let max = window_width.max(1);
    Some(value.clamp(-max, max) as f32)
}

pub(in crate::rtf::parser) fn normalized_wmf_text_justification(
    data: &[u8],
    window_width: i32,
) -> Option<f32> {
    let break_count = i32::from(read_le_i16(data, 0)?);
    let break_extra = i32::from(read_le_i16(data, 2)?);
    if break_count <= 0 || break_extra == 0 {
        return Some(0.0);
    }
    let max = window_width.max(1) as f32;
    Some(((break_extra as f32) / (break_count as f32)).clamp(-max, max))
}

pub(in crate::rtf::parser) fn pixel_rect_start(value: f32, max: f32) -> f32 {
    if value >= max {
        (max - 1.0).max(0.0)
    } else {
        value.max(0.0)
    }
}

pub(in crate::rtf::parser) fn pixel_rect_end(value: f32, max: f32) -> f32 {
    if value >= max {
        max
    } else {
        (value + 1.0).min(max)
    }
}

pub(in crate::rtf::parser) fn parse_wmf_yx_point(
    data: &[u8],
    offset: usize,
    coordinates: WmfCoordinateMap,
) -> Option<(f32, f32)> {
    let y = i32::from(read_le_i16(data, offset)?);
    let x = i32::from(read_le_i16(data, offset.checked_add(2)?)?);
    Some(coordinates.normalized_point(x, y))
}

pub(in crate::rtf::parser) fn parse_wmf_xy_point(
    data: &[u8],
    offset: usize,
    coordinates: WmfCoordinateMap,
) -> Option<(f32, f32)> {
    let x = i32::from(read_le_i16(data, offset)?);
    let y = i32::from(read_le_i16(data, offset.checked_add(2)?)?);
    Some(coordinates.normalized_point(x, y))
}

pub(in crate::rtf::parser) fn parse_wmf_point_list(
    data: &[u8],
    coordinates: WmfCoordinateMap,
) -> Option<Vec<(f32, f32)>> {
    let count = usize::from(read_le_u16(data, 0)?);
    if count > MAX_PASSIVE_WMF_POINTS_PER_RECORD {
        return None;
    }
    let points_bytes = count.checked_mul(4)?;
    let required_len = 2usize.checked_add(points_bytes)?;
    if data.len() < required_len {
        return None;
    }
    let mut points = Vec::with_capacity(count);
    for index in 0..count {
        let offset = 2usize.checked_add(index.checked_mul(4)?)?;
        points.push(parse_wmf_xy_point(data, offset, coordinates)?);
    }
    Some(points)
}

pub(in crate::rtf::parser) fn parse_wmf_polypolygon_list(
    data: &[u8],
    coordinates: WmfCoordinateMap,
) -> Option<Vec<Vec<(f32, f32)>>> {
    let polygon_count = usize::from(read_le_u16(data, 0)?);
    if polygon_count > MAX_PASSIVE_WMF_POINTS_PER_RECORD {
        return None;
    }
    let counts_bytes = polygon_count.checked_mul(2)?;
    let mut counts = Vec::with_capacity(polygon_count);
    let mut total_points = 0usize;
    for index in 0..polygon_count {
        let offset = 2usize.checked_add(index.checked_mul(2)?)?;
        let count = usize::from(read_le_u16(data, offset)?);
        total_points = total_points.checked_add(count)?;
        if total_points > MAX_PASSIVE_WMF_POINTS_PER_RECORD {
            return None;
        }
        counts.push(count);
    }

    let points_start = 2usize.checked_add(counts_bytes)?;
    let points_bytes = total_points.checked_mul(4)?;
    let required_len = points_start.checked_add(points_bytes)?;
    if data.len() < required_len {
        return None;
    }

    let mut polygons = Vec::with_capacity(polygon_count);
    let mut point_index = 0usize;
    for count in counts {
        let mut points = Vec::with_capacity(count);
        for _ in 0..count {
            let offset = points_start.checked_add(point_index.checked_mul(4)?)?;
            points.push(parse_wmf_xy_point(data, offset, coordinates)?);
            point_index += 1;
        }
        polygons.push(points);
    }
    Some(polygons)
}

pub(in crate::rtf::parser) fn wmf_polypolygon_command(
    polygons: Vec<Vec<(f32, f32)>>,
    stroke_color: Option<Color>,
    stroke_width: f32,
    stroke_style: BorderStyle,
    fill_rule: StaticImageVectorFillRule,
    fill_pattern: ShadingPattern,
    fill_color: Option<Color>,
) -> Option<Option<StaticImageVectorCommand>> {
    if stroke_color.is_none() && fill_color.is_none() {
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
    append_wmf_polygon_path_segments(&mut segments, &first_polygon)?;
    for points in visible_polygons {
        let next_start = *points.first()?;
        segments.push(StaticImageVectorPathSegment::MoveTo(
            next_start.0,
            next_start.1,
        ));
        append_wmf_polygon_path_segments(&mut segments, &points)?;
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

pub(in crate::rtf::parser) fn append_wmf_polygon_path_segments(
    segments: &mut Vec<StaticImageVectorPathSegment>,
    points: &[(f32, f32)],
) -> Option<()> {
    let first = *points.first()?;
    for &(x, y) in points.iter().skip(1) {
        segments.push(StaticImageVectorPathSegment::LineTo(x, y));
    }
    segments.push(StaticImageVectorPathSegment::LineTo(first.0, first.1));
    Some(())
}

pub(in crate::rtf::parser) fn bounds_is_visible(
    (left, top, right, bottom): (f32, f32, f32, f32),
) -> bool {
    right - left >= 0.5 && bottom - top >= 0.5
}

pub(in crate::rtf::parser) fn segment_is_visible(
    (x1, y1): (f32, f32),
    (x2, y2): (f32, f32),
) -> bool {
    (x2 - x1).abs() >= 0.5 || (y2 - y1).abs() >= 0.5
}

pub(in crate::rtf::parser) fn point_bounds_are_visible(points: &[(f32, f32)]) -> bool {
    let Some((first_x, first_y)) = points.first().copied() else {
        return false;
    };
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (first_x, first_y, first_x, first_y);
    for (x, y) in points.iter().copied().skip(1) {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    bounds_is_visible((min_x, min_y, max_x, max_y))
}

pub(in crate::rtf::parser) fn color_from_colorref(data: &[u8], offset: usize) -> Option<Color> {
    Some(Color {
        red: *data.get(offset)?,
        green: *data.get(offset + 1)?,
        blue: *data.get(offset + 2)?,
    })
}

#[cfg(test)]
mod tests {
    use super::super::super::minimal_wmf_with_records;
    use super::*;

    #[test]
    fn rejects_an_explicit_raster_limit_below_the_decoder_cap() {
        let wmf = minimal_wmf_with_records(1, 1, &[]);
        assert!(
            parse_wmf_vector_image_data_with_limits(
                &wmf,
                false,
                false,
                VectorDecodeLimits {
                    max_points_per_record: usize::MAX,
                    max_raster_pixels: 1,
                },
            )
            .is_none()
        );
    }
}
