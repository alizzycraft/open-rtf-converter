//! JPEG structure validation and explicitly bounded RGB decoding.

use jpeg_decoder::{Decoder as JpegDecoder, PixelFormat as JpegPixelFormat};

use crate::model::ImageFormat;

use super::MediaDecodeError;

#[derive(Debug)]
pub(in crate::rtf::parser) struct ParsedJpeg {
    pub(in crate::rtf::parser) width_px: u32,
    pub(in crate::rtf::parser) height_px: u32,
    pub(in crate::rtf::parser) format: ImageFormat,
    pub(in crate::rtf::parser) data_end: usize,
}

pub(in crate::rtf::parser) fn parse_jpeg_image_data(
    bytes: &[u8],
    max_pixels: usize,
) -> Result<ParsedJpeg, MediaDecodeError> {
    let parsed = parse_jpeg_image_data_inner(bytes).ok_or(MediaDecodeError::Malformed)?;
    let pixel_count = usize::try_from(parsed.width_px)
        .ok()
        .and_then(|width| {
            usize::try_from(parsed.height_px)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or(MediaDecodeError::PixelLimitExceeded {
            width_px: parsed.width_px,
            height_px: parsed.height_px,
        })?;
    if pixel_count > max_pixels {
        return Err(MediaDecodeError::PixelLimitExceeded {
            width_px: parsed.width_px,
            height_px: parsed.height_px,
        });
    }
    Ok(parsed)
}

fn parse_jpeg_image_data_inner(bytes: &[u8]) -> Option<ParsedJpeg> {
    if bytes.len() < 4 || bytes[0] != 0xff || bytes[1] != 0xd8 {
        return None;
    }
    let data_end = jpeg_eoi_end(bytes)?;

    let mut parsed_frame = None;
    let mut pos = 2;
    while pos + 3 < bytes.len() {
        while pos < bytes.len() && bytes[pos] != 0xff {
            pos += 1;
        }
        while pos < bytes.len() && bytes[pos] == 0xff {
            pos += 1;
        }
        if pos >= bytes.len() {
            return None;
        }

        let marker = bytes[pos];
        pos += 1;
        if marker == 0xd9 {
            return None;
        }
        if marker == 0xda {
            return parsed_frame;
        }
        if is_standalone_jpeg_marker(marker) {
            continue;
        }
        if pos + 1 >= bytes.len() {
            return None;
        }

        let segment_len = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as usize;
        if segment_len < 2 || pos + segment_len > bytes.len() {
            return None;
        }

        if is_jpeg_start_of_frame(marker) {
            if segment_len < 8 {
                return None;
            }
            let height = u16::from_be_bytes([bytes[pos + 3], bytes[pos + 4]]) as u32;
            let width = u16::from_be_bytes([bytes[pos + 5], bytes[pos + 6]]) as u32;
            let components = bytes[pos + 7];
            let expected_segment_len =
                8usize.checked_add(usize::from(components).checked_mul(3)?)?;
            if segment_len < expected_segment_len {
                return None;
            }
            if width == 0 || height == 0 {
                return None;
            }
            let format = match components {
                1 => ImageFormat::JpegGrayscale,
                3 => ImageFormat::Jpeg,
                4 => ImageFormat::JpegCmyk,
                _ => return None,
            };
            parsed_frame = Some(ParsedJpeg {
                width_px: width,
                height_px: height,
                format,
                data_end,
            });
        }

        pos += segment_len;
    }

    None
}

pub(in crate::rtf::parser) fn jpeg_image_data_is_decodable(
    bytes: &[u8],
    jpeg: &ParsedJpeg,
) -> bool {
    let Some(encoded) = bytes.get(..jpeg.data_end) else {
        return false;
    };
    let mut decoder = JpegDecoder::new(encoded);
    let Ok(decoded) = decoder.decode() else {
        return false;
    };
    let Some(info) = decoder.info() else {
        return false;
    };
    if u32::from(info.width) != jpeg.width_px || u32::from(info.height) != jpeg.height_px {
        return false;
    }
    let Some(pixel_count) = usize::from(info.width).checked_mul(usize::from(info.height)) else {
        return false;
    };
    let expected_components = match info.pixel_format {
        JpegPixelFormat::L8 => 1,
        JpegPixelFormat::RGB24 => 3,
        JpegPixelFormat::CMYK32 => 4,
        _ => return false,
    };
    pixel_count
        .checked_mul(expected_components)
        .is_some_and(|expected_len| decoded.len() == expected_len)
}

fn jpeg_eoi_end(bytes: &[u8]) -> Option<usize> {
    let mut pos = 2;
    while pos < bytes.len() {
        while pos < bytes.len() && bytes[pos] != 0xff {
            pos += 1;
        }
        while pos < bytes.len() && bytes[pos] == 0xff {
            pos += 1;
        }
        if pos >= bytes.len() {
            return None;
        }

        let marker = bytes[pos];
        pos += 1;
        if marker == 0xd9 {
            return Some(pos);
        }
        if marker == 0xda {
            if pos + 1 >= bytes.len() {
                return None;
            }
            let segment_len = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as usize;
            if segment_len < 2 || pos.checked_add(segment_len)? > bytes.len() {
                return None;
            }
            pos += segment_len;
            while pos + 1 < bytes.len() {
                if bytes[pos] == 0xff {
                    let marker = bytes[pos + 1];
                    if marker == 0x00 {
                        pos += 2;
                        continue;
                    }
                    if marker == 0xd9 {
                        return Some(pos + 2);
                    }
                }
                pos += 1;
            }
            return None;
        }
        if is_standalone_jpeg_marker(marker) {
            continue;
        }
        if pos + 1 >= bytes.len() {
            return None;
        }
        let segment_len = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as usize;
        if segment_len < 2 || pos.checked_add(segment_len)? > bytes.len() {
            return None;
        }
        pos += segment_len;
    }
    None
}

pub(in crate::rtf::parser) fn decode_bounded_jpeg_to_rgb8(
    bytes: &[u8],
    width_px: u32,
    height_px: u32,
    format: ImageFormat,
    max_encoded_bytes: usize,
    max_pixels: u64,
) -> Option<Vec<u8>> {
    let pixel_count_u64 = u64::from(width_px).checked_mul(u64::from(height_px))?;
    if bytes.len() > max_encoded_bytes || pixel_count_u64 > max_pixels {
        return None;
    }
    let mut decoder = JpegDecoder::new(bytes);
    let decoded = decoder.decode().ok()?;
    let info = decoder.info()?;
    if u32::from(info.width) != width_px || u32::from(info.height) != height_px {
        return None;
    }
    let pixel_count = usize::from(info.width).checked_mul(usize::from(info.height))?;
    let expected_rgb_len = pixel_count.checked_mul(3)?;
    let inverted = matches!(
        format,
        ImageFormat::JpegInverted
            | ImageFormat::JpegGrayscaleInverted
            | ImageFormat::JpegCmykInverted
    );
    let mut rgb = Vec::with_capacity(expected_rgb_len);
    match info.pixel_format {
        JpegPixelFormat::RGB24 if decoded.len() == expected_rgb_len => {
            rgb.extend_from_slice(&decoded);
        }
        JpegPixelFormat::L8 if decoded.len() == pixel_count => {
            for sample in decoded {
                rgb.extend_from_slice(&[sample, sample, sample]);
            }
        }
        JpegPixelFormat::CMYK32 if decoded.len() == pixel_count.saturating_mul(4) => {
            for pixel in decoded.chunks_exact(4) {
                let cyan = u16::from(pixel[0]);
                let magenta = u16::from(pixel[1]);
                let yellow = u16::from(pixel[2]);
                let black = u16::from(pixel[3]);
                rgb.push((((255 - cyan) * (255 - black) + 127) / 255) as u8);
                rgb.push((((255 - magenta) * (255 - black) + 127) / 255) as u8);
                rgb.push((((255 - yellow) * (255 - black) + 127) / 255) as u8);
            }
        }
        _ => return None,
    }
    if inverted {
        for component in &mut rgb {
            *component = 255 - *component;
        }
    }
    Some(rgb)
}

fn is_standalone_jpeg_marker(marker: u8) -> bool {
    marker == 0x01 || (0xd0..=0xd7).contains(&marker)
}

fn is_jpeg_start_of_frame(marker: u8) -> bool {
    matches!(
        marker,
        0xc0 | 0xc1 | 0xc2 | 0xc3 | 0xc5 | 0xc6 | 0xc7 | 0xc9 | 0xca | 0xcb | 0xcd | 0xce | 0xcf
    )
}

#[cfg(test)]
mod tests {
    use super::super::super::minimal_jpeg_with_dimensions;
    use super::*;

    #[test]
    fn rejects_dimensions_above_the_explicit_pixel_limit() {
        let jpeg = minimal_jpeg_with_dimensions(2, 1);
        assert_eq!(
            parse_jpeg_image_data(&jpeg, 1).unwrap_err(),
            MediaDecodeError::PixelLimitExceeded {
                width_px: 2,
                height_px: 1,
            }
        );
    }
}
