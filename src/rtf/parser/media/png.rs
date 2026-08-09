//! PNG validation, bounded inflation, and normalization to renderer-supported forms.

use crate::model::{ImageFormat, StaticImageAlphaMask};

use super::MediaDecodeError;

pub(in crate::rtf::parser) fn inflate_zlib_png_scanlines(
    bytes: &[u8],
    expected_len: usize,
) -> Option<Vec<u8>> {
    if let Some(output) = inflate_zlib_stored_blocks(bytes) {
        return (output.len() == expected_len).then_some(output);
    }
    match miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(bytes, expected_len) {
        Ok(output) if output.len() == expected_len => Some(output),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn unfilter_png_scanlines(
    scanlines: &mut [u8],
    row_len: usize,
    bpp: usize,
) -> bool {
    if row_len <= 1 || bpp == 0 || scanlines.len() % row_len != 0 {
        return false;
    }
    let pixel_len = row_len - 1;
    let mut previous = vec![0u8; pixel_len];
    for row in scanlines.chunks_exact_mut(row_len) {
        let filter = row[0];
        {
            let pixels = &mut row[1..];
            match filter {
                0 => {}
                1 => {
                    for index in 0..pixel_len {
                        let left = if index >= bpp { pixels[index - bpp] } else { 0 };
                        pixels[index] = pixels[index].wrapping_add(left);
                    }
                }
                2 => {
                    for index in 0..pixel_len {
                        pixels[index] = pixels[index].wrapping_add(previous[index]);
                    }
                }
                3 => {
                    for index in 0..pixel_len {
                        let left = if index >= bpp { pixels[index - bpp] } else { 0 };
                        let up = previous[index];
                        let average = ((u16::from(left) + u16::from(up)) / 2) as u8;
                        pixels[index] = pixels[index].wrapping_add(average);
                    }
                }
                4 => {
                    for index in 0..pixel_len {
                        let left = if index >= bpp { pixels[index - bpp] } else { 0 };
                        let up = previous[index];
                        let upper_left = if index >= bpp {
                            previous[index - bpp]
                        } else {
                            0
                        };
                        pixels[index] =
                            pixels[index].wrapping_add(paeth_predictor(left, up, upper_left));
                    }
                }
                _ => return false,
            }
            previous.copy_from_slice(pixels);
        }
        row[0] = 0;
    }
    true
}

fn paeth_predictor(left: u8, up: u8, upper_left: u8) -> u8 {
    let left = i32::from(left);
    let up = i32::from(up);
    let upper_left = i32::from(upper_left);
    let prediction = left + up - upper_left;
    let left_distance = (prediction - left).abs();
    let up_distance = (prediction - up).abs();
    let upper_left_distance = (prediction - upper_left).abs();
    if left_distance <= up_distance && left_distance <= upper_left_distance {
        left as u8
    } else if up_distance <= upper_left_distance {
        up as u8
    } else {
        upper_left as u8
    }
}

pub(in crate::rtf::parser) fn inflate_zlib_stored_blocks(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < 6 {
        return None;
    }
    let cmf = bytes[0];
    let flg = bytes[1];
    if cmf & 0x0f != 8 || ((u16::from(cmf) << 8) | u16::from(flg)) % 31 != 0 {
        return None;
    }
    if flg & 0x20 != 0 {
        return None;
    }
    let expected_adler = u32::from_be_bytes(bytes[bytes.len() - 4..].try_into().ok()?);
    let mut pos = 2usize;
    let end = bytes.len() - 4;
    let mut output = Vec::new();
    loop {
        if pos >= end {
            return None;
        }
        let header = bytes[pos];
        pos += 1;
        let is_final = header & 0x01 != 0;
        if header & 0x06 != 0 {
            return None;
        }
        if pos.checked_add(4)? > end {
            return None;
        }
        let len = u16::from_le_bytes([bytes[pos], bytes[pos + 1]]) as usize;
        let nlen = u16::from_le_bytes([bytes[pos + 2], bytes[pos + 3]]);
        if nlen != !(len as u16) {
            return None;
        }
        pos += 4;
        let data_end = pos.checked_add(len)?;
        if data_end > end {
            return None;
        }
        output.extend_from_slice(&bytes[pos..data_end]);
        pos = data_end;
        if is_final {
            if pos != end || adler32(&output) != expected_adler {
                return None;
            }
            return Some(output);
        }
    }
}

#[cfg(test)]
pub(in crate::rtf::parser) fn deflate_zlib_stored_blocks(data: &[u8]) -> Vec<u8> {
    let mut output = vec![0x78, 0x01];
    let mut remaining = data;
    while !remaining.is_empty() {
        let len = remaining.len().min(u16::MAX as usize);
        let is_final = len == remaining.len();
        output.push(if is_final { 0x01 } else { 0x00 });
        let len_u16 = len as u16;
        output.extend_from_slice(&len_u16.to_le_bytes());
        output.extend_from_slice(&(!len_u16).to_le_bytes());
        output.extend_from_slice(&remaining[..len]);
        remaining = &remaining[len..];
    }
    if data.is_empty() {
        output.extend_from_slice(&[0x01, 0x00, 0x00, 0xff, 0xff]);
    }
    output.extend_from_slice(&adler32(data).to_be_bytes());
    output
}

fn adler32(data: &[u8]) -> u32 {
    const MOD_ADLER: u32 = 65_521;
    let mut a = 1u32;
    let mut b = 0u32;
    for byte in data {
        a = (a + u32::from(*byte)) % MOD_ADLER;
        b = (b + a) % MOD_ADLER;
    }
    (b << 16) | a
}

#[derive(Debug)]
pub(in crate::rtf::parser) struct ParsedPng {
    pub(in crate::rtf::parser) width_px: u32,
    pub(in crate::rtf::parser) height_px: u32,
    pub(in crate::rtf::parser) format: ImageFormat,
    pub(in crate::rtf::parser) idat: Vec<u8>,
    pub(in crate::rtf::parser) palette: Vec<u8>,
    pub(in crate::rtf::parser) alpha_mask: Option<StaticImageAlphaMask>,
}

pub(in crate::rtf::parser) fn parse_png_image_data(
    bytes: &[u8],
    max_pixels: usize,
) -> Result<ParsedPng, MediaDecodeError> {
    let (width_px, height_px) = png_dimensions(bytes).ok_or(MediaDecodeError::Malformed)?;
    let pixel_count = usize::try_from(width_px)
        .ok()
        .and_then(|width| {
            usize::try_from(height_px)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or(MediaDecodeError::PixelLimitExceeded {
            width_px,
            height_px,
        })?;
    if pixel_count > max_pixels {
        return Err(MediaDecodeError::PixelLimitExceeded {
            width_px,
            height_px,
        });
    }
    parse_png_image_data_inner(bytes).ok_or(MediaDecodeError::Malformed)
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.get(..8)? != PNG_SIGNATURE || bytes.get(12..16)? != b"IHDR" {
        return None;
    }
    let width_px = u32::from_be_bytes(bytes.get(16..20)?.try_into().ok()?);
    let height_px = u32::from_be_bytes(bytes.get(20..24)?.try_into().ok()?);
    (width_px > 0 && height_px > 0).then_some((width_px, height_px))
}

fn parse_png_image_data_inner(bytes: &[u8]) -> Option<ParsedPng> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < PNG_SIGNATURE.len() || &bytes[..8] != PNG_SIGNATURE {
        return None;
    }

    let mut pos = PNG_SIGNATURE.len();
    let mut width_px = 0;
    let mut height_px = 0;
    let mut bit_depth = 0;
    let mut color_type = 0;
    let mut interlace_method = 0;
    let mut format = None;
    let mut saw_ihdr = false;
    let mut saw_plte = false;
    let mut saw_trns = false;
    let mut saw_iend = false;
    let mut idat = Vec::new();
    let mut palette = Vec::new();
    let mut transparency = None;

    while pos.checked_add(12)? <= bytes.len() {
        let len = u32::from_be_bytes(bytes[pos..pos + 4].try_into().ok()?) as usize;
        let chunk_type = &bytes[pos + 4..pos + 8];
        let data_start = pos + 8;
        let data_end = data_start.checked_add(len)?;
        let next = data_end.checked_add(4)?;
        if next > bytes.len() {
            return None;
        }
        let data = &bytes[data_start..data_end];

        match chunk_type {
            b"IHDR" => {
                if saw_ihdr || len != 13 || pos != PNG_SIGNATURE.len() {
                    return None;
                }
                width_px = u32::from_be_bytes(data[0..4].try_into().ok()?);
                height_px = u32::from_be_bytes(data[4..8].try_into().ok()?);
                bit_depth = data[8];
                color_type = data[9];
                let compression = data[10];
                let filter = data[11];
                let interlace = data[12];
                interlace_method = interlace;
                let supported_bit_depth = match color_type {
                    0 => matches!(bit_depth, 1 | 2 | 4 | 8 | 16),
                    2 | 4 | 6 => matches!(bit_depth, 8 | 16),
                    3 => matches!(bit_depth, 1 | 2 | 4 | 8),
                    _ => false,
                };
                if width_px == 0
                    || height_px == 0
                    || !supported_bit_depth
                    || compression != 0
                    || filter != 0
                    || !matches!(interlace, 0 | 1)
                {
                    return None;
                }
                format = Some(match color_type {
                    0 => ImageFormat::PngGrayscale,
                    2 => ImageFormat::Png,
                    3 => ImageFormat::PngIndexed,
                    4 => ImageFormat::PngGrayscale,
                    6 => ImageFormat::Png,
                    _ => return None,
                });
                saw_ihdr = true;
            }
            b"PLTE" => {
                if !saw_ihdr || saw_plte || saw_trns || saw_iend || !idat.is_empty() {
                    return None;
                }
                if len == 0 || len % 3 != 0 || len > 256 * 3 {
                    return None;
                }
                palette.extend_from_slice(data);
                saw_plte = true;
            }
            b"tRNS" => {
                if !saw_ihdr || saw_trns || saw_iend || !idat.is_empty() {
                    return None;
                }
                transparency = Some(match color_type {
                    0 => {
                        if len != 2 || bit_depth != 8 {
                            return None;
                        }
                        PngTransparency::Grayscale(u16::from_be_bytes(data.try_into().ok()?))
                    }
                    2 => {
                        if len != 6 || bit_depth != 8 {
                            return None;
                        }
                        PngTransparency::Rgb {
                            red: u16::from_be_bytes(data[0..2].try_into().ok()?),
                            green: u16::from_be_bytes(data[2..4].try_into().ok()?),
                            blue: u16::from_be_bytes(data[4..6].try_into().ok()?),
                        }
                    }
                    3 => {
                        let palette_entries = palette.len() / 3;
                        if !saw_plte || len == 0 || len > palette_entries {
                            return None;
                        }
                        PngTransparency::Indexed(data.to_vec())
                    }
                    _ => return None,
                });
                saw_trns = true;
            }
            b"IDAT" => {
                if !saw_ihdr || saw_iend {
                    return None;
                }
                if matches!(format, Some(ImageFormat::PngIndexed)) && !saw_plte {
                    return None;
                }
                idat.extend_from_slice(data);
            }
            b"IEND" => {
                if len != 0 || !saw_ihdr {
                    return None;
                }
                saw_iend = true;
                break;
            }
            _ => {}
        }

        pos = next;
    }

    if !saw_ihdr || !saw_iend || idat.is_empty() {
        return None;
    }
    if matches!(format, Some(ImageFormat::PngIndexed)) && palette.is_empty() {
        return None;
    }
    let format = format?;
    let mut alpha_mask = None;
    let idat = if interlace_method == 1 {
        if matches!(bit_depth, 1 | 2 | 4) {
            normalize_adam7_low_bit_depth_png_idat(
                &idat,
                width_px,
                height_px,
                bit_depth,
                format,
                palette.len() / 3,
            )?
        } else {
            let source_pixel_bytes = png_pixel_bytes(bit_depth, color_type)?;
            let deinterlaced =
                normalize_adam7_png_idat(&idat, width_px, height_px, source_pixel_bytes)?;
            match (bit_depth, color_type) {
                (8, 4 | 6) => {
                    let normalized =
                        normalize_alpha_png_idat(&deinterlaced, width_px, height_px, color_type)?;
                    alpha_mask = Some(normalized.alpha_mask);
                    normalized.idat
                }
                (8, 0 | 2 | 3) => deinterlaced,
                (16, 0 | 2 | 4 | 6) => {
                    let normalized =
                        normalize_16_bit_png_idat(&deinterlaced, width_px, height_px, color_type)?;
                    alpha_mask = normalized.alpha_mask;
                    normalized.idat
                }
                _ => return None,
            }
        }
    } else {
        match (bit_depth, color_type) {
            (8, 4 | 6) => {
                let normalized = normalize_alpha_png_idat(&idat, width_px, height_px, color_type)?;
                alpha_mask = Some(normalized.alpha_mask);
                normalized.idat
            }
            (8, _) => idat,
            (16, 0 | 2 | 4 | 6) => {
                let normalized = normalize_16_bit_png_idat(&idat, width_px, height_px, color_type)?;
                alpha_mask = normalized.alpha_mask;
                normalized.idat
            }
            _ => normalize_low_bit_depth_png_idat(
                &idat,
                width_px,
                height_px,
                bit_depth,
                format,
                palette.len() / 3,
            )?,
        }
    };
    if let Some(transparency) = transparency {
        alpha_mask = Some(png_transparency_alpha_mask(
            &idat,
            width_px,
            height_px,
            bit_depth,
            format,
            transparency,
        )?);
    }

    Some(ParsedPng {
        width_px,
        height_px,
        format,
        idat,
        palette,
        alpha_mask,
    })
}

pub(in crate::rtf::parser) const PNG_ADAM7_PASSES: [(usize, usize, usize, usize); 7] = [
    (0, 0, 8, 8),
    (4, 0, 8, 8),
    (0, 4, 4, 8),
    (2, 0, 4, 4),
    (0, 2, 2, 4),
    (1, 0, 2, 2),
    (0, 1, 1, 2),
];

fn normalize_adam7_png_idat(
    idat: &[u8],
    width_px: u32,
    height_px: u32,
    pixel_bytes: usize,
) -> Option<Vec<u8>> {
    let width = usize::try_from(width_px).ok()?;
    let height = usize::try_from(height_px).ok()?;
    let output_row_pixels_len = width.checked_mul(pixel_bytes)?;
    let output_row_len = output_row_pixels_len.checked_add(1)?;
    let output_len = output_row_len.checked_mul(height)?;
    let expected_source_len = adam7_expected_scanline_len(width, height, pixel_bytes)?;
    let mut source = inflate_zlib_png_scanlines(idat, expected_source_len)?;

    let mut output = vec![0u8; output_len];
    for row in output.chunks_exact_mut(output_row_len) {
        row[0] = 0;
    }

    let mut source_pos = 0usize;
    for (start_x, start_y, step_x, step_y) in PNG_ADAM7_PASSES {
        let pass_width = adam7_pass_dimension(width, start_x, step_x);
        let pass_height = adam7_pass_dimension(height, start_y, step_y);
        if pass_width == 0 || pass_height == 0 {
            continue;
        }
        let pass_pixels_len = pass_width.checked_mul(pixel_bytes)?;
        let pass_row_len = pass_pixels_len.checked_add(1)?;
        let pass_len = pass_row_len.checked_mul(pass_height)?;
        let pass_end = source_pos.checked_add(pass_len)?;
        let pass = source.get_mut(source_pos..pass_end)?;
        if !unfilter_png_scanlines(pass, pass_row_len, pixel_bytes) {
            return None;
        }
        for pass_y in 0..pass_height {
            let output_y = start_y.checked_add(pass_y.checked_mul(step_y)?)?;
            let source_row_start = pass_y.checked_mul(pass_row_len)?.checked_add(1)?;
            let source_row = pass.get(source_row_start..source_row_start + pass_pixels_len)?;
            for pass_x in 0..pass_width {
                let output_x = start_x.checked_add(pass_x.checked_mul(step_x)?)?;
                let source_pixel_start = pass_x.checked_mul(pixel_bytes)?;
                let output_pixel_start = output_y
                    .checked_mul(output_row_len)?
                    .checked_add(1)?
                    .checked_add(output_x.checked_mul(pixel_bytes)?)?;
                output[output_pixel_start..output_pixel_start + pixel_bytes].copy_from_slice(
                    &source_row[source_pixel_start..source_pixel_start + pixel_bytes],
                );
            }
        }
        source_pos = pass_end;
    }

    if source_pos != source.len() {
        return None;
    }
    Some(miniz_oxide::deflate::compress_to_vec_zlib(&output, 6))
}

fn normalize_adam7_low_bit_depth_png_idat(
    idat: &[u8],
    width_px: u32,
    height_px: u32,
    bit_depth: u8,
    format: ImageFormat,
    palette_entries: usize,
) -> Option<Vec<u8>> {
    if !matches!(bit_depth, 1 | 2 | 4)
        || !matches!(format, ImageFormat::PngGrayscale | ImageFormat::PngIndexed)
    {
        return None;
    }
    let width = usize::try_from(width_px).ok()?;
    let height = usize::try_from(height_px).ok()?;
    let expected_source_len = adam7_expected_packed_scanline_len(width, height, bit_depth)?;
    let mut source = inflate_zlib_png_scanlines(idat, expected_source_len)?;

    let output_row_len = width.checked_add(1)?;
    let output_len = output_row_len.checked_mul(height)?;
    let mut output = vec![0u8; output_len];
    let max_sample = (1u16 << bit_depth) - 1;
    let mut source_pos = 0usize;
    for (start_x, start_y, step_x, step_y) in PNG_ADAM7_PASSES {
        let pass_width = adam7_pass_dimension(width, start_x, step_x);
        let pass_height = adam7_pass_dimension(height, start_y, step_y);
        if pass_width == 0 || pass_height == 0 {
            continue;
        }
        let pass_row_len = adam7_packed_pass_row_len(pass_width, bit_depth)?;
        let pass_len = pass_row_len.checked_mul(pass_height)?;
        let pass_end = source_pos.checked_add(pass_len)?;
        let pass = source.get_mut(source_pos..pass_end)?;
        if !unfilter_png_scanlines(pass, pass_row_len, 1) {
            return None;
        }
        for pass_y in 0..pass_height {
            let output_y = start_y.checked_add(pass_y.checked_mul(step_y)?)?;
            let source_row_start = pass_y.checked_mul(pass_row_len)?.checked_add(1)?;
            let source_row = pass.get(source_row_start..source_row_start + (pass_row_len - 1))?;
            for pass_x in 0..pass_width {
                let sample = packed_png_sample(source_row, pass_x, bit_depth)?;
                let value = match format {
                    ImageFormat::PngIndexed => {
                        if usize::from(sample) >= palette_entries {
                            return None;
                        }
                        sample
                    }
                    ImageFormat::PngGrayscale => ((u16::from(sample) * 255) / max_sample) as u8,
                    _ => return None,
                };
                let output_x = start_x.checked_add(pass_x.checked_mul(step_x)?)?;
                let output_index = output_y
                    .checked_mul(output_row_len)?
                    .checked_add(1)?
                    .checked_add(output_x)?;
                output[output_index] = value;
            }
        }
        source_pos = pass_end;
    }

    if source_pos != source.len() {
        return None;
    }
    Some(miniz_oxide::deflate::compress_to_vec_zlib(&output, 6))
}

pub(in crate::rtf::parser) fn png_pixel_bytes(bit_depth: u8, color_type: u8) -> Option<usize> {
    let components = match color_type {
        0 | 3 => 1usize,
        2 => 3usize,
        4 => 2usize,
        6 => 4usize,
        _ => return None,
    };
    components.checked_mul(usize::from(bit_depth).checked_div(8)?)
}

fn adam7_expected_scanline_len(width: usize, height: usize, components: usize) -> Option<usize> {
    let mut total = 0usize;
    for (start_x, start_y, step_x, step_y) in PNG_ADAM7_PASSES {
        let pass_width = adam7_pass_dimension(width, start_x, step_x);
        let pass_height = adam7_pass_dimension(height, start_y, step_y);
        if pass_width == 0 || pass_height == 0 {
            continue;
        }
        let row_pixels_len = pass_width.checked_mul(components)?;
        let row_len = row_pixels_len.checked_add(1)?;
        total = total.checked_add(row_len.checked_mul(pass_height)?)?;
    }
    Some(total)
}

fn adam7_expected_packed_scanline_len(width: usize, height: usize, bit_depth: u8) -> Option<usize> {
    let mut total = 0usize;
    for (start_x, start_y, step_x, step_y) in PNG_ADAM7_PASSES {
        let pass_width = adam7_pass_dimension(width, start_x, step_x);
        let pass_height = adam7_pass_dimension(height, start_y, step_y);
        if pass_width == 0 || pass_height == 0 {
            continue;
        }
        let row_len = adam7_packed_pass_row_len(pass_width, bit_depth)?;
        total = total.checked_add(row_len.checked_mul(pass_height)?)?;
    }
    Some(total)
}

fn adam7_packed_pass_row_len(width: usize, bit_depth: u8) -> Option<usize> {
    let row_bits = width.checked_mul(usize::from(bit_depth))?;
    row_bits.div_ceil(8).checked_add(1)
}

pub(in crate::rtf::parser) fn adam7_pass_dimension(
    size: usize,
    start: usize,
    step: usize,
) -> usize {
    if size <= start {
        0
    } else {
        ((size - 1 - start) / step) + 1
    }
}

struct NormalizedPngData {
    idat: Vec<u8>,
    alpha_mask: Option<StaticImageAlphaMask>,
}

struct NormalizedAlphaPngData {
    idat: Vec<u8>,
    alpha_mask: StaticImageAlphaMask,
}

fn normalize_16_bit_png_idat(
    idat: &[u8],
    width_px: u32,
    height_px: u32,
    color_type: u8,
) -> Option<NormalizedPngData> {
    let (source_components, output_components) = match color_type {
        0 => (1usize, 1usize),
        2 => (3usize, 3usize),
        4 => (2usize, 1usize),
        6 => (4usize, 3usize),
        _ => return None,
    };
    let width = usize::try_from(width_px).ok()?;
    let height = usize::try_from(height_px).ok()?;
    let source_pixel_bytes = source_components.checked_mul(2)?;
    let source_pixels_len = width.checked_mul(source_pixel_bytes)?;
    let source_row_len = source_pixels_len.checked_add(1)?;
    let expected_source_len = source_row_len.checked_mul(height)?;
    let mut scanlines = inflate_zlib_png_scanlines(idat, expected_source_len)?;
    if !unfilter_png_scanlines(&mut scanlines, source_row_len, source_pixel_bytes) {
        return None;
    }

    let output_pixels_len = width.checked_mul(output_components)?;
    let output_row_len = output_pixels_len.checked_add(1)?;
    let output_len = output_row_len.checked_mul(height)?;
    let mut output = Vec::with_capacity(output_len);
    let mut alpha = if matches!(color_type, 4 | 6) {
        Some(Vec::with_capacity(
            width.checked_add(1)?.checked_mul(height)?,
        ))
    } else {
        None
    };
    for row in scanlines.chunks_exact(source_row_len) {
        output.push(0);
        if let Some(alpha) = alpha.as_mut() {
            alpha.push(0);
        }
        let pixels = &row[1..];
        match color_type {
            0 => {
                for sample in pixels.chunks_exact(2) {
                    output.push(scale_png_16_to_8(u16::from_be_bytes([
                        sample[0], sample[1],
                    ])));
                }
            }
            2 => {
                for pixel in pixels.chunks_exact(6) {
                    output.push(scale_png_16_to_8(u16::from_be_bytes([pixel[0], pixel[1]])));
                    output.push(scale_png_16_to_8(u16::from_be_bytes([pixel[2], pixel[3]])));
                    output.push(scale_png_16_to_8(u16::from_be_bytes([pixel[4], pixel[5]])));
                }
            }
            4 => {
                for pixel in pixels.chunks_exact(4) {
                    let gray = u16::from_be_bytes([pixel[0], pixel[1]]);
                    let alpha_sample = u16::from_be_bytes([pixel[2], pixel[3]]);
                    output.push(scale_png_16_to_8(gray));
                    if let Some(mask) = alpha.as_mut() {
                        mask.push(scale_png_16_to_8(alpha_sample));
                    }
                }
            }
            6 => {
                for pixel in pixels.chunks_exact(8) {
                    let red = u16::from_be_bytes([pixel[0], pixel[1]]);
                    let green = u16::from_be_bytes([pixel[2], pixel[3]]);
                    let blue = u16::from_be_bytes([pixel[4], pixel[5]]);
                    let alpha_sample = u16::from_be_bytes([pixel[6], pixel[7]]);
                    output.push(scale_png_16_to_8(red));
                    output.push(scale_png_16_to_8(green));
                    output.push(scale_png_16_to_8(blue));
                    if let Some(mask) = alpha.as_mut() {
                        mask.push(scale_png_16_to_8(alpha_sample));
                    }
                }
            }
            _ => return None,
        }
    }
    Some(NormalizedPngData {
        idat: miniz_oxide::deflate::compress_to_vec_zlib(&output, 6),
        alpha_mask: alpha.map(|bytes| StaticImageAlphaMask {
            bytes: miniz_oxide::deflate::compress_to_vec_zlib(&bytes, 6),
        }),
    })
}

pub(in crate::rtf::parser) fn scale_png_16_to_8(value: u16) -> u8 {
    ((u32::from(value) * 255 + 32_767) / 65_535) as u8
}

#[derive(Debug)]
enum PngTransparency {
    Grayscale(u16),
    Rgb { red: u16, green: u16, blue: u16 },
    Indexed(Vec<u8>),
}

fn png_transparency_alpha_mask(
    idat: &[u8],
    width_px: u32,
    height_px: u32,
    bit_depth: u8,
    format: ImageFormat,
    transparency: PngTransparency,
) -> Option<StaticImageAlphaMask> {
    match (format, transparency) {
        (ImageFormat::PngIndexed, PngTransparency::Indexed(alpha_values)) => {
            indexed_png_transparency_alpha_mask(idat, width_px, height_px, &alpha_values)
        }
        (ImageFormat::PngGrayscale, PngTransparency::Grayscale(gray)) if bit_depth == 8 => {
            if gray > u16::from(u8::MAX) {
                return None;
            }
            grayscale_png_transparency_alpha_mask(idat, width_px, height_px, gray as u8)
        }
        (ImageFormat::Png, PngTransparency::Rgb { red, green, blue }) if bit_depth == 8 => {
            if red > u16::from(u8::MAX) || green > u16::from(u8::MAX) || blue > u16::from(u8::MAX) {
                return None;
            }
            rgb_png_transparency_alpha_mask(
                idat,
                width_px,
                height_px,
                red as u8,
                green as u8,
                blue as u8,
            )
        }
        _ => None,
    }
}

fn indexed_png_transparency_alpha_mask(
    idat: &[u8],
    width_px: u32,
    height_px: u32,
    alpha_values: &[u8],
) -> Option<StaticImageAlphaMask> {
    let (width, height, row_len, mut scanlines) =
        unfiltered_png_scanlines(idat, width_px, height_px, 1)?;
    let output_row_len = width.checked_add(1)?;
    let output_len = output_row_len.checked_mul(height)?;
    let mut output = Vec::with_capacity(output_len);
    for row in scanlines.chunks_exact_mut(row_len) {
        output.push(0);
        for sample in &row[1..] {
            output.push(
                alpha_values
                    .get(usize::from(*sample))
                    .copied()
                    .unwrap_or(u8::MAX),
            );
        }
    }
    Some(StaticImageAlphaMask {
        bytes: miniz_oxide::deflate::compress_to_vec_zlib(&output, 6),
    })
}

fn grayscale_png_transparency_alpha_mask(
    idat: &[u8],
    width_px: u32,
    height_px: u32,
    transparent_gray: u8,
) -> Option<StaticImageAlphaMask> {
    let (width, height, row_len, mut scanlines) =
        unfiltered_png_scanlines(idat, width_px, height_px, 1)?;
    let output_row_len = width.checked_add(1)?;
    let output_len = output_row_len.checked_mul(height)?;
    let mut output = Vec::with_capacity(output_len);
    for row in scanlines.chunks_exact_mut(row_len) {
        output.push(0);
        for sample in &row[1..] {
            output.push(if *sample == transparent_gray {
                0
            } else {
                u8::MAX
            });
        }
    }
    Some(StaticImageAlphaMask {
        bytes: miniz_oxide::deflate::compress_to_vec_zlib(&output, 6),
    })
}

fn rgb_png_transparency_alpha_mask(
    idat: &[u8],
    width_px: u32,
    height_px: u32,
    transparent_red: u8,
    transparent_green: u8,
    transparent_blue: u8,
) -> Option<StaticImageAlphaMask> {
    let (width, height, row_len, mut scanlines) =
        unfiltered_png_scanlines(idat, width_px, height_px, 3)?;
    let output_row_len = width.checked_add(1)?;
    let output_len = output_row_len.checked_mul(height)?;
    let mut output = Vec::with_capacity(output_len);
    for row in scanlines.chunks_exact_mut(row_len) {
        output.push(0);
        for pixel in row[1..].chunks_exact(3) {
            output.push(
                if pixel[0] == transparent_red
                    && pixel[1] == transparent_green
                    && pixel[2] == transparent_blue
                {
                    0
                } else {
                    u8::MAX
                },
            );
        }
    }
    Some(StaticImageAlphaMask {
        bytes: miniz_oxide::deflate::compress_to_vec_zlib(&output, 6),
    })
}

pub(in crate::rtf::parser) fn unfiltered_png_scanlines(
    idat: &[u8],
    width_px: u32,
    height_px: u32,
    components: usize,
) -> Option<(usize, usize, usize, Vec<u8>)> {
    let width = usize::try_from(width_px).ok()?;
    let height = usize::try_from(height_px).ok()?;
    let row_pixels_len = width.checked_mul(components)?;
    let row_len = row_pixels_len.checked_add(1)?;
    let expected_len = row_len.checked_mul(height)?;
    let mut scanlines = inflate_zlib_png_scanlines(idat, expected_len)?;
    if !unfilter_png_scanlines(&mut scanlines, row_len, components) {
        return None;
    }
    Some((width, height, row_len, scanlines))
}

fn normalize_alpha_png_idat(
    idat: &[u8],
    width_px: u32,
    height_px: u32,
    color_type: u8,
) -> Option<NormalizedAlphaPngData> {
    let (source_components, output_components) = match color_type {
        4 => (2usize, 1usize),
        6 => (4usize, 3usize),
        _ => return None,
    };
    let width = usize::try_from(width_px).ok()?;
    let height = usize::try_from(height_px).ok()?;
    let source_pixels_len = width.checked_mul(source_components)?;
    let source_row_len = source_pixels_len.checked_add(1)?;
    let expected_source_len = source_row_len.checked_mul(height)?;
    let mut scanlines = inflate_zlib_png_scanlines(idat, expected_source_len)?;
    if !unfilter_png_scanlines(&mut scanlines, source_row_len, source_components) {
        return None;
    }

    let output_pixels_len = width.checked_mul(output_components)?;
    let output_row_len = output_pixels_len.checked_add(1)?;
    let output_len = output_row_len.checked_mul(height)?;
    let mut output = Vec::with_capacity(output_len);
    let alpha_row_len = width.checked_add(1)?;
    let alpha_len = alpha_row_len.checked_mul(height)?;
    let mut alpha_mask = Vec::with_capacity(alpha_len);
    for row in scanlines.chunks_exact(source_row_len) {
        output.push(0);
        alpha_mask.push(0);
        let pixels = &row[1..];
        match color_type {
            4 => {
                for pixel in pixels.chunks_exact(2) {
                    output.push(pixel[0]);
                    alpha_mask.push(pixel[1]);
                }
            }
            6 => {
                for pixel in pixels.chunks_exact(4) {
                    output.push(pixel[0]);
                    output.push(pixel[1]);
                    output.push(pixel[2]);
                    alpha_mask.push(pixel[3]);
                }
            }
            _ => return None,
        }
    }
    Some(NormalizedAlphaPngData {
        idat: miniz_oxide::deflate::compress_to_vec_zlib(&output, 6),
        alpha_mask: StaticImageAlphaMask {
            bytes: miniz_oxide::deflate::compress_to_vec_zlib(&alpha_mask, 6),
        },
    })
}

fn normalize_low_bit_depth_png_idat(
    idat: &[u8],
    width_px: u32,
    height_px: u32,
    bit_depth: u8,
    format: ImageFormat,
    palette_entries: usize,
) -> Option<Vec<u8>> {
    if !matches!(bit_depth, 1 | 2 | 4)
        || !matches!(format, ImageFormat::PngGrayscale | ImageFormat::PngIndexed)
    {
        return None;
    }
    let width = usize::try_from(width_px).ok()?;
    let height = usize::try_from(height_px).ok()?;
    let row_bits = width.checked_mul(usize::from(bit_depth))?;
    let packed_row_len = row_bits.div_ceil(8);
    let source_row_len = packed_row_len.checked_add(1)?;
    let expected_source_len = source_row_len.checked_mul(height)?;
    let mut scanlines = inflate_zlib_png_scanlines(idat, expected_source_len)?;
    if !unfilter_png_scanlines(&mut scanlines, source_row_len, 1) {
        return None;
    }

    let output_row_len = width.checked_add(1)?;
    let output_len = output_row_len.checked_mul(height)?;
    let mut output = Vec::with_capacity(output_len);
    let max_sample = (1u16 << bit_depth) - 1;
    for row in scanlines.chunks_exact(source_row_len) {
        output.push(0);
        let packed = &row[1..];
        for index in 0..width {
            let sample = packed_png_sample(packed, index, bit_depth)?;
            let value = match format {
                ImageFormat::PngIndexed => {
                    if usize::from(sample) >= palette_entries {
                        return None;
                    }
                    sample
                }
                ImageFormat::PngGrayscale => ((u16::from(sample) * 255) / max_sample) as u8,
                _ => return None,
            };
            output.push(value);
        }
    }
    Some(miniz_oxide::deflate::compress_to_vec_zlib(&output, 6))
}

fn packed_png_sample(row: &[u8], index: usize, bit_depth: u8) -> Option<u8> {
    let bits_per_sample = usize::from(bit_depth);
    let samples_per_byte = 8usize.checked_div(bits_per_sample)?;
    let byte = *row.get(index.checked_div(samples_per_byte)?)?;
    let bit_offset = index
        .checked_rem(samples_per_byte)?
        .checked_mul(bits_per_sample)?;
    let shift = 8usize
        .checked_sub(bits_per_sample)?
        .checked_sub(bit_offset)?;
    let mask = (1u16 << bit_depth) - 1;
    Some(((u16::from(byte) >> shift) & mask) as u8)
}

#[cfg(test)]
mod tests {
    use super::super::super::minimal_rgb_png_with_dimensions;
    use super::*;

    #[test]
    fn rejects_dimensions_before_inflation_above_the_explicit_pixel_limit() {
        let png = minimal_rgb_png_with_dimensions(2, 1);
        assert_eq!(
            parse_png_image_data(&png, 1).unwrap_err(),
            MediaDecodeError::PixelLimitExceeded {
                width_px: 2,
                height_px: 1,
            }
        );
    }
}
