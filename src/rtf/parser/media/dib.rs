//! Explicitly pixel-bounded DIB and Windows bitmap decoding and normalization.

use crate::model::{ImageFormat, StaticImageAlphaMask};

use super::super::{read_le_i32, read_le_u16, read_le_u32};
use super::jpeg::{jpeg_image_data_is_decodable, parse_jpeg_image_data};
use super::png::parse_png_image_data;

#[derive(Debug)]
pub(in crate::rtf::parser) struct ParsedDib {
    pub(in crate::rtf::parser) width_px: u32,
    pub(in crate::rtf::parser) height_px: u32,
    pub(in crate::rtf::parser) rgb: Vec<u8>,
    pub(in crate::rtf::parser) alpha: Option<Vec<u8>>,
    pub(in crate::rtf::parser) top_down: bool,
}

#[derive(Debug)]
pub(in crate::rtf::parser) struct ParsedWindowsBitmap {
    pub(in crate::rtf::parser) width_px: u32,
    pub(in crate::rtf::parser) height_px: u32,
    pub(in crate::rtf::parser) rgb: Vec<u8>,
}

#[derive(Debug)]
pub(in crate::rtf::parser) struct ParsedCompressedDibImage {
    pub(in crate::rtf::parser) width_px: u32,
    pub(in crate::rtf::parser) height_px: u32,
    pub(in crate::rtf::parser) format: ImageFormat,
    pub(in crate::rtf::parser) bytes: Vec<u8>,
    pub(in crate::rtf::parser) palette: Vec<u8>,
    pub(in crate::rtf::parser) alpha_mask: Option<StaticImageAlphaMask>,
    pub(in crate::rtf::parser) top_down: bool,
}

#[derive(Debug, Copy, Clone)]
struct DibColorMasks {
    red: u32,
    green: u32,
    blue: u32,
    alpha: Option<u32>,
}

impl DibColorMasks {
    fn are_supported(self) -> bool {
        if self.red == 0
            || self.green == 0
            || self.blue == 0
            || self.red & self.green != 0
            || self.red & self.blue != 0
            || self.green & self.blue != 0
        {
            return false;
        }
        let Some(alpha) = self.alpha else {
            return true;
        };
        alpha != 0 && self.red & alpha == 0 && self.green & alpha == 0 && self.blue & alpha == 0
    }
}

pub(in crate::rtf::parser) fn parse_dib_image_data(
    bytes: &[u8],
    max_pixels: usize,
) -> Option<ParsedDib> {
    const BITMAPCOREHEADER_SIZE: usize = 12;
    const BITMAPINFOHEADER_SIZE: usize = 40;
    const BI_RGB: u32 = 0;
    const BI_RLE8: u32 = 1;
    const BI_RLE4: u32 = 2;
    const BI_BITFIELDS: u32 = 3;
    const BI_ALPHABITFIELDS: u32 = 6;

    if bytes.len() < 4 {
        return None;
    }

    let header_size = read_le_u32(bytes, 0)? as usize;
    if header_size == BITMAPCOREHEADER_SIZE {
        return parse_bitmap_core_dib_image_data(bytes, max_pixels);
    }
    if header_size < BITMAPINFOHEADER_SIZE || header_size > bytes.len() {
        return None;
    }

    let width = read_le_i32(bytes, 4)?;
    let raw_height = read_le_i32(bytes, 8)?;
    let planes = read_le_u16(bytes, 12)?;
    let bits_per_pixel = read_le_u16(bytes, 14)?;
    let compression = read_le_u32(bytes, 16)?;
    if width <= 0 || raw_height == 0 || planes != 1 {
        return None;
    }
    let uses_bitfields = matches!(compression, BI_BITFIELDS | BI_ALPHABITFIELDS)
        && matches!(bits_per_pixel, 16 | 32);
    let uses_rle8 = compression == BI_RLE8 && bits_per_pixel == 8 && raw_height > 0;
    let uses_rle4 = compression == BI_RLE4 && bits_per_pixel == 4 && raw_height > 0;
    if compression != BI_RGB && !uses_bitfields && !uses_rle8 && !uses_rle4 {
        return None;
    }

    let width_px = u32::try_from(width).ok()?;
    let height_px = raw_height.unsigned_abs();
    let pixels = usize::try_from(width_px)
        .ok()?
        .checked_mul(usize::try_from(height_px).ok()?)?;
    if pixels == 0 || pixels > max_pixels {
        return None;
    }

    let width = usize::try_from(width_px).ok()?;
    let colors_used = read_le_u32(bytes, 32)?;
    let color_masks = if uses_bitfields {
        let mask_offset = if header_size == BITMAPINFOHEADER_SIZE {
            BITMAPINFOHEADER_SIZE
        } else if header_size >= BITMAPINFOHEADER_SIZE + 12 {
            BITMAPINFOHEADER_SIZE
        } else {
            return None;
        };
        let has_alpha_mask = matches!(bits_per_pixel, 16 | 32)
            && (compression == BI_ALPHABITFIELDS || header_size >= 56);
        let masks = DibColorMasks {
            red: read_le_u32(bytes, mask_offset)?,
            green: read_le_u32(bytes, mask_offset + 4)?,
            blue: read_le_u32(bytes, mask_offset + 8)?,
            alpha: if has_alpha_mask {
                Some(read_le_u32(bytes, mask_offset + 12)?)
            } else {
                None
            },
        };
        if !masks.are_supported() {
            return None;
        }
        Some(masks)
    } else {
        None
    };
    let bitfield_mask_bytes = if uses_bitfields && header_size == BITMAPINFOHEADER_SIZE {
        if compression == BI_ALPHABITFIELDS && matches!(bits_per_pixel, 16 | 32) {
            16
        } else {
            12
        }
    } else {
        0
    };
    let pixel_data_start = header_size.checked_add(bitfield_mask_bytes)?;
    if pixel_data_start > bytes.len() {
        return None;
    }

    if uses_rle8 {
        return parse_rle8_dib_pixels(bytes, width_px, height_px, header_size, colors_used);
    }
    if uses_rle4 {
        return parse_rle4_dib_pixels(bytes, width_px, height_px, header_size, colors_used);
    }

    let (row_stride, pixel_start, palette_entries) = match bits_per_pixel {
        1 | 4 | 8 => {
            if color_masks.is_some() {
                return None;
            }
            let palette_capacity = 1usize.checked_shl(u32::from(bits_per_pixel))?;
            let palette_entries = if colors_used == 0 {
                palette_capacity
            } else {
                usize::try_from(colors_used).ok()?
            };
            if palette_entries == 0 || palette_entries > palette_capacity {
                return None;
            }
            let palette_bytes = palette_entries.checked_mul(4)?;
            let palette_end = pixel_data_start.checked_add(palette_bytes)?;
            if palette_end > bytes.len() {
                return None;
            }
            let row_bits = width.checked_mul(usize::from(bits_per_pixel))?;
            let row_stride = row_bits.checked_add(31)?.checked_div(32)?.checked_mul(4)?;
            (row_stride, palette_end, Some(palette_entries))
        }
        16 => {
            let row_bits = width.checked_mul(16)?;
            let row_stride = row_bits.checked_add(31)?.checked_div(32)?.checked_mul(4)?;
            (row_stride, pixel_data_start, None)
        }
        24 => {
            if color_masks.is_some() {
                return None;
            }
            let unpadded_row_bytes = width.checked_mul(3)?;
            let row_stride = unpadded_row_bytes
                .checked_add(3)?
                .checked_div(4)?
                .checked_mul(4)?;
            (row_stride, pixel_data_start, None)
        }
        32 => {
            let row_stride = width.checked_mul(4)?;
            (row_stride, pixel_data_start, None)
        }
        _ => return None,
    };
    let pixel_bytes = row_stride.checked_mul(usize::try_from(height_px).ok()?)?;
    let pixel_end = pixel_start.checked_add(pixel_bytes)?;
    if pixel_end > bytes.len() {
        return None;
    }

    decode_uncompressed_dib_pixels(
        bytes,
        width_px,
        height_px,
        bits_per_pixel,
        row_stride,
        pixel_start,
        palette_entries,
        header_size,
        4,
        raw_height < 0,
        color_masks,
    )
}

pub(in crate::rtf::parser) fn parse_windows_bitmap_image_data(
    bytes: &[u8],
    width_px_hint: Option<u32>,
    height_px_hint: Option<u32>,
    planes: Option<u16>,
    bits_per_pixel: Option<u16>,
    width_bytes: Option<usize>,
    max_pixels: usize,
) -> Option<ParsedWindowsBitmap> {
    let width_px = width_px_hint?;
    let height_px = height_px_hint?;
    let planes = planes.unwrap_or(1);
    let bits_per_pixel = bits_per_pixel?;
    if planes != 1 || !matches!(bits_per_pixel, 1 | 24 | 32) {
        return None;
    }

    let width = usize::try_from(width_px).ok()?;
    let height = usize::try_from(height_px).ok()?;
    let pixels = width.checked_mul(height)?;
    if pixels == 0 || pixels > max_pixels {
        return None;
    }
    let minimum_row_bytes = match bits_per_pixel {
        1 => width.checked_add(7)?.checked_div(8)?,
        24 => width.checked_mul(3)?,
        32 => width.checked_mul(4)?,
        _ => return None,
    };
    let row_stride = width_bytes
        .unwrap_or_else(|| word_aligned_bitmap_row_stride(width, bits_per_pixel).unwrap_or(0));
    if row_stride < minimum_row_bytes {
        return None;
    }
    let required_bytes = row_stride.checked_mul(height)?;
    if bytes.len() < required_bytes {
        return None;
    }

    let mut rgb = vec![0; pixels.checked_mul(3)?];
    for output_y in 0..height {
        let source_y = height - 1 - output_y;
        let source_row = source_y.checked_mul(row_stride)?;
        let output_row = output_y.checked_mul(width)?.checked_mul(3)?;
        for x in 0..width {
            let output = output_row.checked_add(x.checked_mul(3)?)?;
            match bits_per_pixel {
                1 => {
                    let byte = *bytes.get(source_row.checked_add(x / 8)?)?;
                    let bit = (byte >> (7 - (x % 8))) & 0x01;
                    let sample = if bit == 0 { 0 } else { 255 };
                    rgb[output] = sample;
                    rgb[output + 1] = sample;
                    rgb[output + 2] = sample;
                }
                24 => {
                    let source = source_row.checked_add(x.checked_mul(3)?)?;
                    rgb[output] = *bytes.get(source + 2)?;
                    rgb[output + 1] = *bytes.get(source + 1)?;
                    rgb[output + 2] = *bytes.get(source)?;
                }
                32 => {
                    let source = source_row.checked_add(x.checked_mul(4)?)?;
                    rgb[output] = *bytes.get(source + 2)?;
                    rgb[output + 1] = *bytes.get(source + 1)?;
                    rgb[output + 2] = *bytes.get(source)?;
                }
                _ => return None,
            }
        }
    }

    Some(ParsedWindowsBitmap {
        width_px,
        height_px,
        rgb,
    })
}

fn word_aligned_bitmap_row_stride(width: usize, bits_per_pixel: u16) -> Option<usize> {
    width
        .checked_mul(usize::from(bits_per_pixel))?
        .checked_add(15)?
        .checked_div(16)?
        .checked_mul(2)
}

pub(in crate::rtf::parser) fn parse_compressed_dib_image_data(
    bytes: &[u8],
    max_pixels: usize,
) -> Option<ParsedCompressedDibImage> {
    const BITMAPINFOHEADER_SIZE: usize = 40;
    const BI_JPEG: u32 = 4;
    const BI_PNG: u32 = 5;

    if bytes.len() < BITMAPINFOHEADER_SIZE {
        return None;
    }
    let header_size = read_le_u32(bytes, 0)? as usize;
    if header_size < BITMAPINFOHEADER_SIZE || header_size > bytes.len() {
        return None;
    }
    let width = read_le_i32(bytes, 4)?;
    let raw_height = read_le_i32(bytes, 8)?;
    let planes = read_le_u16(bytes, 12)?;
    let compression = read_le_u32(bytes, 16)?;
    if width <= 0 || raw_height == 0 || planes != 1 || !matches!(compression, BI_JPEG | BI_PNG) {
        return None;
    }
    let payload_len = usize::try_from(read_le_u32(bytes, 20)?).ok()?;
    if payload_len == 0 {
        return None;
    }
    let payload_end = header_size.checked_add(payload_len)?;
    let payload = bytes.get(header_size..payload_end)?;
    let width_px = u32::try_from(width).ok()?;
    let height_px = raw_height.unsigned_abs();
    let pixels = usize::try_from(width_px)
        .ok()?
        .checked_mul(usize::try_from(height_px).ok()?)?;
    if pixels == 0 || pixels > max_pixels {
        return None;
    }

    match compression {
        BI_JPEG => {
            let jpeg = parse_jpeg_image_data(payload, max_pixels).ok()?;
            if jpeg.width_px != width_px || jpeg.height_px != height_px {
                return None;
            }
            if !jpeg_image_data_is_decodable(payload, &jpeg) {
                return None;
            }
            Some(ParsedCompressedDibImage {
                width_px,
                height_px,
                format: jpeg.format,
                bytes: payload[..jpeg.data_end].to_vec(),
                palette: Vec::new(),
                alpha_mask: None,
                top_down: raw_height < 0,
            })
        }
        BI_PNG => {
            let png = parse_png_image_data(payload, max_pixels).ok()?;
            if png.width_px != width_px || png.height_px != height_px {
                return None;
            }
            Some(ParsedCompressedDibImage {
                width_px,
                height_px,
                format: png.format,
                bytes: png.idat,
                palette: png.palette,
                alpha_mask: png.alpha_mask,
                top_down: raw_height < 0,
            })
        }
        _ => None,
    }
}

fn parse_bitmap_core_dib_image_data(bytes: &[u8], max_pixels: usize) -> Option<ParsedDib> {
    const BITMAPCOREHEADER_SIZE: usize = 12;

    if bytes.len() < BITMAPCOREHEADER_SIZE {
        return None;
    }

    let width_px = u32::from(read_le_u16(bytes, 4)?);
    let height_px = u32::from(read_le_u16(bytes, 6)?);
    let planes = read_le_u16(bytes, 8)?;
    let bits_per_pixel = read_le_u16(bytes, 10)?;
    if width_px == 0 || height_px == 0 || planes != 1 {
        return None;
    }

    let pixels = usize::try_from(width_px)
        .ok()?
        .checked_mul(usize::try_from(height_px).ok()?)?;
    if pixels == 0 || pixels > max_pixels {
        return None;
    }

    let width = usize::try_from(width_px).ok()?;
    let height = usize::try_from(height_px).ok()?;
    let (row_stride, pixel_start, palette_entries) =
        dib_layout_for_uncompressed_bitmap(width, bits_per_pixel, BITMAPCOREHEADER_SIZE, 3)?;
    let pixel_bytes = row_stride.checked_mul(height)?;
    let pixel_end = pixel_start.checked_add(pixel_bytes)?;
    if pixel_end > bytes.len() {
        return None;
    }

    decode_uncompressed_dib_pixels(
        bytes,
        width_px,
        height_px,
        bits_per_pixel,
        row_stride,
        pixel_start,
        palette_entries,
        BITMAPCOREHEADER_SIZE,
        3,
        false,
        None,
    )
}

fn dib_layout_for_uncompressed_bitmap(
    width: usize,
    bits_per_pixel: u16,
    header_size: usize,
    palette_entry_size: usize,
) -> Option<(usize, usize, Option<usize>)> {
    match bits_per_pixel {
        1 | 4 | 8 => {
            let palette_entries = 1usize.checked_shl(u32::from(bits_per_pixel))?;
            let palette_bytes = palette_entries.checked_mul(palette_entry_size)?;
            let palette_end = header_size.checked_add(palette_bytes)?;
            let row_bits = width.checked_mul(usize::from(bits_per_pixel))?;
            let row_stride = row_bits.checked_add(31)?.checked_div(32)?.checked_mul(4)?;
            Some((row_stride, palette_end, Some(palette_entries)))
        }
        24 => {
            let unpadded_row_bytes = width.checked_mul(3)?;
            let row_stride = unpadded_row_bytes
                .checked_add(3)?
                .checked_div(4)?
                .checked_mul(4)?;
            Some((row_stride, header_size, None))
        }
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn decode_uncompressed_dib_pixels(
    bytes: &[u8],
    width_px: u32,
    height_px: u32,
    bits_per_pixel: u16,
    row_stride: usize,
    pixel_start: usize,
    palette_entries: Option<usize>,
    palette_start: usize,
    palette_entry_size: usize,
    top_down: bool,
    color_masks: Option<DibColorMasks>,
) -> Option<ParsedDib> {
    let width = usize::try_from(width_px).ok()?;
    let height = usize::try_from(height_px).ok()?;
    let pixels = width.checked_mul(height)?;
    let output_len = pixels.checked_mul(3)?;
    let mut rgb = vec![0; output_len];
    let mut alpha = if (bits_per_pixel == 32 && color_masks.is_none())
        || color_masks.and_then(|masks| masks.alpha).is_some()
    {
        Some(vec![0; pixels])
    } else {
        None
    };

    for output_y in 0..height {
        let source_y = if top_down {
            output_y
        } else {
            height - 1 - output_y
        };
        let source_row = pixel_start.checked_add(source_y.checked_mul(row_stride)?)?;
        let output_row = output_y.checked_mul(width)?.checked_mul(3)?;
        for x in 0..width {
            let output = output_row.checked_add(x.checked_mul(3)?)?;
            match bits_per_pixel {
                1 => {
                    let source = source_row.checked_add(x / 8)?;
                    let byte = *bytes.get(source)?;
                    let shift = 7 - (x % 8);
                    let palette_index = usize::from((byte >> shift) & 0x01);
                    copy_dib_palette_color(
                        bytes,
                        palette_start,
                        palette_entries?,
                        palette_entry_size,
                        palette_index,
                        &mut rgb[output..output + 3],
                    )?;
                }
                4 => {
                    let source = source_row.checked_add(x / 2)?;
                    let byte = *bytes.get(source)?;
                    let palette_index = if x % 2 == 0 {
                        usize::from(byte >> 4)
                    } else {
                        usize::from(byte & 0x0f)
                    };
                    copy_dib_palette_color(
                        bytes,
                        palette_start,
                        palette_entries?,
                        palette_entry_size,
                        palette_index,
                        &mut rgb[output..output + 3],
                    )?;
                }
                8 => {
                    let source = source_row.checked_add(x)?;
                    let palette_index = usize::from(*bytes.get(source)?);
                    copy_dib_palette_color(
                        bytes,
                        palette_start,
                        palette_entries?,
                        palette_entry_size,
                        palette_index,
                        &mut rgb[output..output + 3],
                    )?;
                }
                16 => {
                    let source = source_row.checked_add(x.checked_mul(2)?)?;
                    let pixel = read_le_u16(bytes, source)?;
                    if let Some(masks) = color_masks {
                        rgb[output] = expand_masked_color_channel(u32::from(pixel), masks.red)?;
                        rgb[output + 1] =
                            expand_masked_color_channel(u32::from(pixel), masks.green)?;
                        rgb[output + 2] =
                            expand_masked_color_channel(u32::from(pixel), masks.blue)?;
                        if let (Some(alpha_mask), Some(alpha)) = (alpha.as_mut(), masks.alpha) {
                            alpha_mask[output_y.checked_mul(width)?.checked_add(x)?] =
                                expand_masked_color_channel(u32::from(pixel), alpha)?;
                        }
                    } else {
                        let red = ((pixel >> 10) & 0x1f) as u8;
                        let green = ((pixel >> 5) & 0x1f) as u8;
                        let blue = (pixel & 0x1f) as u8;
                        rgb[output] = expand_5bit_color_channel(red);
                        rgb[output + 1] = expand_5bit_color_channel(green);
                        rgb[output + 2] = expand_5bit_color_channel(blue);
                    }
                }
                24 => {
                    let bytes_per_pixel = usize::from(bits_per_pixel / 8);
                    let source = source_row.checked_add(x.checked_mul(bytes_per_pixel)?)?;
                    rgb[output] = bytes[source + 2];
                    rgb[output + 1] = bytes[source + 1];
                    rgb[output + 2] = bytes[source];
                }
                32 => {
                    let source = source_row.checked_add(x.checked_mul(4)?)?;
                    if let Some(masks) = color_masks {
                        let pixel = read_le_u32(bytes, source)?;
                        rgb[output] = expand_masked_color_channel(pixel, masks.red)?;
                        rgb[output + 1] = expand_masked_color_channel(pixel, masks.green)?;
                        rgb[output + 2] = expand_masked_color_channel(pixel, masks.blue)?;
                        if let (Some(alpha_mask), Some(alpha)) = (alpha.as_mut(), masks.alpha) {
                            alpha_mask[output_y.checked_mul(width)?.checked_add(x)?] =
                                expand_masked_color_channel(pixel, alpha)?;
                        }
                    } else {
                        rgb[output] = bytes[source + 2];
                        rgb[output + 1] = bytes[source + 1];
                        rgb[output + 2] = bytes[source];
                        if let Some(alpha) = alpha.as_mut() {
                            alpha[output_y.checked_mul(width)?.checked_add(x)?] = bytes[source + 3];
                        }
                    }
                }
                _ => return None,
            }
        }
    }

    Some(ParsedDib {
        width_px,
        height_px,
        rgb,
        alpha,
        top_down,
    })
}

fn parse_rle8_dib_pixels(
    bytes: &[u8],
    width_px: u32,
    height_px: u32,
    header_size: usize,
    colors_used: u32,
) -> Option<ParsedDib> {
    let palette_entries = if colors_used == 0 {
        256usize
    } else {
        usize::try_from(colors_used).ok()?
    };
    if palette_entries == 0 || palette_entries > 256 {
        return None;
    }
    let palette_bytes = palette_entries.checked_mul(4)?;
    let pixel_start = header_size.checked_add(palette_bytes)?;
    if pixel_start > bytes.len() {
        return None;
    }

    let width = usize::try_from(width_px).ok()?;
    let height = usize::try_from(height_px).ok()?;
    let pixels = width.checked_mul(height)?;
    let output_len = pixels.checked_mul(3)?;
    let mut rgb = vec![0; output_len];
    let mut x = 0usize;
    let mut y_from_bottom = 0usize;
    let mut pos = pixel_start;
    let mut reached_eof = false;

    while pos < bytes.len() && !reached_eof {
        let count = *bytes.get(pos)?;
        let value = *bytes.get(pos + 1)?;
        pos = pos.checked_add(2)?;
        if count > 0 {
            let run = usize::from(count);
            if x.checked_add(run)? > width || y_from_bottom >= height {
                return None;
            }
            for _ in 0..run {
                write_rle_output_pixel(
                    bytes,
                    header_size,
                    palette_entries,
                    width,
                    height,
                    x,
                    y_from_bottom,
                    value,
                    &mut rgb,
                )?;
                x = x.checked_add(1)?;
            }
            continue;
        }

        match value {
            0 => {
                x = 0;
                y_from_bottom = y_from_bottom.checked_add(1)?;
                if y_from_bottom > height {
                    return None;
                }
            }
            1 => reached_eof = true,
            2 => {
                let dx = usize::from(*bytes.get(pos)?);
                let dy = usize::from(*bytes.get(pos + 1)?);
                pos = pos.checked_add(2)?;
                x = x.checked_add(dx)?;
                y_from_bottom = y_from_bottom.checked_add(dy)?;
                if x > width || y_from_bottom >= height {
                    return None;
                }
            }
            absolute_count => {
                let literal_count = usize::from(absolute_count);
                if literal_count < 3
                    || x.checked_add(literal_count)? > width
                    || y_from_bottom >= height
                {
                    return None;
                }
                let literal_end = pos.checked_add(literal_count)?;
                let literals = bytes.get(pos..literal_end)?;
                for &palette_index in literals {
                    write_rle_output_pixel(
                        bytes,
                        header_size,
                        palette_entries,
                        width,
                        height,
                        x,
                        y_from_bottom,
                        palette_index,
                        &mut rgb,
                    )?;
                    x = x.checked_add(1)?;
                }
                pos = literal_end;
                if literal_count % 2 == 1 {
                    pos = pos.checked_add(1)?;
                    bytes.get(pos - 1)?;
                }
            }
        }
    }

    if !reached_eof {
        return None;
    }

    Some(ParsedDib {
        width_px,
        height_px,
        rgb,
        alpha: None,
        top_down: false,
    })
}

fn parse_rle4_dib_pixels(
    bytes: &[u8],
    width_px: u32,
    height_px: u32,
    header_size: usize,
    colors_used: u32,
) -> Option<ParsedDib> {
    let palette_entries = if colors_used == 0 {
        16usize
    } else {
        usize::try_from(colors_used).ok()?
    };
    if palette_entries == 0 || palette_entries > 16 {
        return None;
    }
    let palette_bytes = palette_entries.checked_mul(4)?;
    let pixel_start = header_size.checked_add(palette_bytes)?;
    if pixel_start > bytes.len() {
        return None;
    }

    let width = usize::try_from(width_px).ok()?;
    let height = usize::try_from(height_px).ok()?;
    let pixels = width.checked_mul(height)?;
    let output_len = pixels.checked_mul(3)?;
    let mut rgb = vec![0; output_len];
    let mut x = 0usize;
    let mut y_from_bottom = 0usize;
    let mut pos = pixel_start;
    let mut reached_eof = false;

    while pos < bytes.len() && !reached_eof {
        let count = *bytes.get(pos)?;
        let value = *bytes.get(pos + 1)?;
        pos = pos.checked_add(2)?;
        if count > 0 {
            let run = usize::from(count);
            if x.checked_add(run)? > width || y_from_bottom >= height {
                return None;
            }
            let indexes = [value >> 4, value & 0x0f];
            for index in 0..run {
                write_rle_output_pixel(
                    bytes,
                    header_size,
                    palette_entries,
                    width,
                    height,
                    x,
                    y_from_bottom,
                    indexes[index % 2],
                    &mut rgb,
                )?;
                x = x.checked_add(1)?;
            }
            continue;
        }

        match value {
            0 => {
                x = 0;
                y_from_bottom = y_from_bottom.checked_add(1)?;
                if y_from_bottom > height {
                    return None;
                }
            }
            1 => reached_eof = true,
            2 => {
                let dx = usize::from(*bytes.get(pos)?);
                let dy = usize::from(*bytes.get(pos + 1)?);
                pos = pos.checked_add(2)?;
                x = x.checked_add(dx)?;
                y_from_bottom = y_from_bottom.checked_add(dy)?;
                if x > width || y_from_bottom >= height {
                    return None;
                }
            }
            absolute_count => {
                let literal_count = usize::from(absolute_count);
                if literal_count < 3
                    || x.checked_add(literal_count)? > width
                    || y_from_bottom >= height
                {
                    return None;
                }
                let literal_bytes = literal_count.checked_add(1)?.checked_div(2)?;
                let literal_end = pos.checked_add(literal_bytes)?;
                let literals = bytes.get(pos..literal_end)?;
                for index in 0..literal_count {
                    let byte = *literals.get(index / 2)?;
                    let palette_index = if index % 2 == 0 {
                        byte >> 4
                    } else {
                        byte & 0x0f
                    };
                    write_rle_output_pixel(
                        bytes,
                        header_size,
                        palette_entries,
                        width,
                        height,
                        x,
                        y_from_bottom,
                        palette_index,
                        &mut rgb,
                    )?;
                    x = x.checked_add(1)?;
                }
                pos = literal_end;
                if literal_bytes % 2 == 1 {
                    pos = pos.checked_add(1)?;
                    bytes.get(pos - 1)?;
                }
            }
        }
    }

    if !reached_eof {
        return None;
    }

    Some(ParsedDib {
        width_px,
        height_px,
        rgb,
        alpha: None,
        top_down: false,
    })
}

#[allow(clippy::too_many_arguments)]
fn write_rle_output_pixel(
    bytes: &[u8],
    palette_start: usize,
    palette_entries: usize,
    width: usize,
    height: usize,
    x: usize,
    y_from_bottom: usize,
    palette_index: u8,
    rgb: &mut [u8],
) -> Option<()> {
    let output_y = height.checked_sub(1)?.checked_sub(y_from_bottom)?;
    let output = output_y
        .checked_mul(width)?
        .checked_add(x)?
        .checked_mul(3)?;
    copy_dib_palette_color(
        bytes,
        palette_start,
        palette_entries,
        4,
        usize::from(palette_index),
        rgb.get_mut(output..output + 3)?,
    )
}

fn copy_dib_palette_color(
    bytes: &[u8],
    palette_start: usize,
    palette_entries: usize,
    palette_entry_size: usize,
    palette_index: usize,
    output: &mut [u8],
) -> Option<()> {
    if palette_index >= palette_entries || output.len() < 3 {
        return None;
    }
    if palette_entry_size < 3 {
        return None;
    }
    let palette = palette_start.checked_add(palette_index.checked_mul(palette_entry_size)?)?;
    output[0] = *bytes.get(palette + 2)?;
    output[1] = *bytes.get(palette + 1)?;
    output[2] = *bytes.get(palette)?;
    Some(())
}

fn expand_5bit_color_channel(value: u8) -> u8 {
    (value << 3) | (value >> 2)
}

fn expand_masked_color_channel(pixel: u32, mask: u32) -> Option<u8> {
    if mask == 0 {
        return None;
    }
    let shift = mask.trailing_zeros();
    let raw = (pixel & mask) >> shift;
    let max = mask >> shift;
    if max == 0 {
        return None;
    }
    let scaled = raw
        .checked_mul(255)?
        .checked_add(max / 2)?
        .checked_div(max)?;
    u8::try_from(scaled).ok()
}

#[cfg(test)]
mod tests {
    use super::super::super::minimal_24bit_dib_with_dimensions;
    use super::*;

    #[test]
    fn rejects_dimensions_above_the_explicit_pixel_limit() {
        let dib = minimal_24bit_dib_with_dimensions(2, 1);
        assert!(parse_dib_image_data(&dib, 1).is_none());
    }
}
