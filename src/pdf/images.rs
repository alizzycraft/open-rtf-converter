//! Raster image masks, XObjects, color spaces, and compression decisions.

use super::*;

pub(in crate::pdf) fn write_image_alpha_mask(
    pdf: &mut Pdf,
    alpha_mask_id: Ref,
    alpha_mask: &crate::model::StaticImageAlphaMask,
    width_px: u32,
    height_px: u32,
) {
    let mut mask = pdf.image_xobject(alpha_mask_id, &alpha_mask.bytes);
    mask.width(width_px as i32);
    mask.height(height_px as i32);
    mask.color_space().device_gray();
    mask.bits_per_component(8);
    mask.filter(Filter::FlateDecode);
    mask.decode_parms()
        .predictor(Predictor::PngOptimum)
        .colors(1)
        .bits_per_component(8)
        .columns(width_px as i32);
}

pub(in crate::pdf) fn write_passive_image_xobject(
    pdf: &mut Pdf,
    image_id: Ref,
    format: ImageFormat,
    width_px: u32,
    height_px: u32,
    bytes: &[u8],
    palette: &[u8],
    tone_adjustment: Option<ImageToneAdjustment>,
    alpha_mask_id: Option<Ref>,
) {
    match format {
        ImageFormat::Jpeg
        | ImageFormat::JpegInverted
        | ImageFormat::JpegPassiveGrayscale
        | ImageFormat::JpegPassiveBilevel
        | ImageFormat::JpegGrayscale
        | ImageFormat::JpegGrayscaleInverted
        | ImageFormat::JpegCmyk
        | ImageFormat::JpegCmykInverted
        | ImageFormat::JpegCmykPassiveGrayscale
        | ImageFormat::JpegCmykPassiveBilevel => {
            let mut image = pdf.image_xobject(image_id, bytes);
            image.width(width_px as i32);
            image.height(height_px as i32);
            match format {
                ImageFormat::Jpeg
                | ImageFormat::JpegInverted
                | ImageFormat::JpegPassiveGrayscale
                | ImageFormat::JpegPassiveBilevel => image.color_space().device_rgb(),
                ImageFormat::JpegGrayscale | ImageFormat::JpegGrayscaleInverted => {
                    image.color_space().device_gray()
                }
                ImageFormat::JpegCmyk
                | ImageFormat::JpegCmykInverted
                | ImageFormat::JpegCmykPassiveGrayscale
                | ImageFormat::JpegCmykPassiveBilevel => image.color_space().device_cmyk(),
                _ => image.color_space().device_rgb(),
            }
            image.bits_per_component(8);
            image.filter(Filter::DctDecode);
            match (format, tone_adjustment) {
                (ImageFormat::Jpeg | ImageFormat::JpegPassiveGrayscale, Some(tone_adjustment)) => {
                    image.decode([
                        tone_adjustment.decode_low,
                        tone_adjustment.decode_high,
                        tone_adjustment.decode_low,
                        tone_adjustment.decode_high,
                        tone_adjustment.decode_low,
                        tone_adjustment.decode_high,
                    ]);
                }
                (
                    ImageFormat::JpegGrayscale | ImageFormat::JpegGrayscaleInverted,
                    Some(tone_adjustment),
                ) => {
                    image.decode([tone_adjustment.decode_low, tone_adjustment.decode_high]);
                }
                (
                    ImageFormat::JpegCmyk
                    | ImageFormat::JpegCmykInverted
                    | ImageFormat::JpegCmykPassiveGrayscale,
                    Some(tone_adjustment),
                ) => {
                    image.decode([
                        tone_adjustment.decode_low,
                        tone_adjustment.decode_high,
                        tone_adjustment.decode_low,
                        tone_adjustment.decode_high,
                        tone_adjustment.decode_low,
                        tone_adjustment.decode_high,
                        tone_adjustment.decode_low,
                        tone_adjustment.decode_high,
                    ]);
                }
                (ImageFormat::JpegInverted, Some(tone_adjustment)) => {
                    image.decode([
                        tone_adjustment.decode_low,
                        tone_adjustment.decode_high,
                        tone_adjustment.decode_low,
                        tone_adjustment.decode_high,
                        tone_adjustment.decode_low,
                        tone_adjustment.decode_high,
                    ]);
                }
                (ImageFormat::JpegInverted | ImageFormat::JpegPassiveBilevel, _) => {
                    image.decode([1.0, 0.0, 1.0, 0.0, 1.0, 0.0]);
                }
                (ImageFormat::JpegGrayscaleInverted, _) => {
                    image.decode([1.0, 0.0]);
                }
                (ImageFormat::JpegCmykInverted | ImageFormat::JpegCmykPassiveBilevel, _) => {
                    image.decode([1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0]);
                }
                _ => {}
            }
            if let Some(alpha_mask_id) = alpha_mask_id {
                image.s_mask(alpha_mask_id);
            }
        }
        ImageFormat::Png | ImageFormat::PngGrayscale | ImageFormat::PngIndexed => {
            let mut image = pdf.image_xobject(image_id, bytes);
            image.width(width_px as i32);
            image.height(height_px as i32);
            let color_components = match format {
                ImageFormat::Png => {
                    image.color_space().device_rgb();
                    3
                }
                ImageFormat::PngGrayscale => {
                    image.color_space().device_gray();
                    1
                }
                ImageFormat::PngIndexed => {
                    let hival = palette
                        .len()
                        .checked_div(3)
                        .and_then(|entries| entries.checked_sub(1))
                        .unwrap_or(0) as i32;
                    image
                        .color_space()
                        .indexed(Name(b"DeviceRGB"), hival, palette);
                    1
                }
                _ => {
                    image.color_space().device_rgb();
                    3
                }
            };
            image.bits_per_component(8);
            image.filter(Filter::FlateDecode);
            image
                .decode_parms()
                .predictor(Predictor::PngOptimum)
                .colors(color_components)
                .bits_per_component(8)
                .columns(width_px as i32);
            if let Some(alpha_mask_id) = alpha_mask_id {
                image.s_mask(alpha_mask_id);
            }
        }
        ImageFormat::Rgb8 => {
            write_rgb8_image_xobject(pdf, image_id, width_px, height_px, bytes, alpha_mask_id);
        }
        ImageFormat::WmfVector | ImageFormat::Placeholder => {}
    }
}

pub(in crate::pdf) fn write_rgb8_image_xobject(
    pdf: &mut Pdf,
    image_id: Ref,
    width_px: u32,
    height_px: u32,
    bytes: &[u8],
    alpha_mask_id: Option<Ref>,
) {
    let mut image = pdf.image_xobject(image_id, bytes);
    image.width(width_px as i32);
    image.height(height_px as i32);
    image.color_space().device_rgb();
    image.bits_per_component(8);
    if let Some(alpha_mask_id) = alpha_mask_id {
        image.s_mask(alpha_mask_id);
    }
}

pub(in crate::pdf) fn draw_image_layout_item(
    content: &mut Content,
    fragment: &crate::layout::ImageFragment,
    page_images: &[PdfImageRef],
    image_ref_cursor: &mut usize,
) {
    if fragment.image.format == ImageFormat::Placeholder {
        draw_passive_image_placeholder(content, fragment);
        return;
    }
    if fragment.image.format == ImageFormat::WmfVector {
        draw_passive_wmf_vector_image(content, fragment, page_images, image_ref_cursor);
        return;
    }
    if !image_format_uses_xobject(fragment.image.format) {
        return;
    }
    let Some(image_ref) = page_images.get(*image_ref_cursor) else {
        return;
    };
    *image_ref_cursor = (*image_ref_cursor).saturating_add(1);
    let draw = image_draw_rect(fragment);
    if draw.fully_cropped {
        return;
    }
    content.save_state();
    if draw.clipped {
        content.rect(fragment.x, fragment.y, fragment.width, fragment.height);
        content.clip_nonzero();
        content.end_path();
    }
    if image_format_uses_passive_jpeg_luminosity_blend(fragment.image.format) {
        content.set_fill_gray(1.0);
        content.rect(fragment.x, fragment.y, fragment.width, fragment.height);
        content.fill_nonzero();
        content.set_parameters(Name(PASSIVE_JPEG_LUMINOSITY_GS));
    }
    content.transform([draw.width, 0.0, 0.0, draw.height, draw.x, draw.y]);
    content.x_object(Name(&image_ref.name));
    content.restore_state();
}

#[derive(Debug, Copy, Clone)]
pub(in crate::pdf) struct ImageDrawRect {
    pub(in crate::pdf) x: f32,
    pub(in crate::pdf) y: f32,
    pub(in crate::pdf) width: f32,
    pub(in crate::pdf) height: f32,
    pub(in crate::pdf) clipped: bool,
    pub(in crate::pdf) fully_cropped: bool,
}

pub(in crate::pdf) fn image_draw_rect(fragment: &crate::layout::ImageFragment) -> ImageDrawRect {
    let natural_width = fragment
        .image
        .natural_width_px_hint
        .unwrap_or(fragment.image.width_px)
        .max(1) as f32
        * 0.75;
    let natural_height = fragment
        .image
        .natural_height_px_hint
        .unwrap_or(fragment.image.height_px)
        .max(1) as f32
        * 0.75;
    let (crop_basis_width, crop_basis_height) = match (
        fragment.image.display_width_twips,
        fragment.image.display_height_twips,
    ) {
        (Some(width), Some(height)) => (twips_to_points(width), twips_to_points(height)),
        (Some(width), None) => {
            let width = twips_to_points(width);
            (width, width * natural_height / natural_width)
        }
        (None, Some(height)) => {
            let height = twips_to_points(height);
            (height * natural_width / natural_height, height)
        }
        (None, None) => (natural_width, natural_height),
    };
    let crop = fragment.image.crop;
    let fully_cropped = twips_to_points(
        crop.left_twips
            .max(0)
            .saturating_add(crop.right_twips.max(0)),
    ) >= natural_width
        || twips_to_points(
            crop.top_twips
                .max(0)
                .saturating_add(crop.bottom_twips.max(0)),
        ) >= natural_height;
    let mut left = twips_to_points(crop.left_twips.max(0)) / crop_basis_width.max(1.0);
    let mut right = twips_to_points(crop.right_twips.max(0)) / crop_basis_width.max(1.0);
    let mut top = twips_to_points(crop.top_twips.max(0)) / crop_basis_height.max(1.0);
    let mut bottom = twips_to_points(crop.bottom_twips.max(0)) / crop_basis_height.max(1.0);
    clamp_crop_pair(&mut left, &mut right);
    clamp_crop_pair(&mut top, &mut bottom);

    let visible_width = (1.0 - left - right).max(0.05);
    let visible_height = (1.0 - top - bottom).max(0.05);
    let width = fragment.width / visible_width;
    let height = fragment.height / visible_height;

    ImageDrawRect {
        x: fragment.x - (width * left),
        y: fragment.y - (height * bottom),
        width,
        height,
        clipped: left > 0.0 || right > 0.0 || top > 0.0 || bottom > 0.0,
        fully_cropped,
    }
}

pub(in crate::pdf) fn clamp_crop_pair(first: &mut f32, second: &mut f32) {
    let total = *first + *second;
    if total > 0.95 {
        let scale = 0.95 / total;
        *first *= scale;
        *second *= scale;
    }
}

const PASSIVE_IMAGE_PLACEHOLDER_LABEL: &str = "Image skipped";
const PASSIVE_IMAGE_PLACEHOLDER_LABEL_FONT_SIZE: f32 = 9.0;
const PASSIVE_IMAGE_PLACEHOLDER_LABEL_PADDING: f32 = 6.0;

pub(in crate::pdf) fn passive_image_placeholder_label_fits(width: f32, height: f32) -> bool {
    height >= 24.0
        && width
            >= estimated_passive_vector_text_width(
                PASSIVE_IMAGE_PLACEHOLDER_LABEL,
                PASSIVE_IMAGE_PLACEHOLDER_LABEL_FONT_SIZE,
            ) + (PASSIVE_IMAGE_PLACEHOLDER_LABEL_PADDING * 2.0)
}

pub(in crate::pdf) fn draw_passive_image_placeholder(
    content: &mut Content,
    fragment: &crate::layout::ImageFragment,
) {
    let width = fragment.width.max(1.0);
    let height = fragment.height.max(1.0);
    set_fill_color(
        content,
        PdfColor {
            red: 0.96,
            green: 0.96,
            blue: 0.96,
        },
    );
    content.rect(fragment.x, fragment.y, width, height);
    content.fill_nonzero();
    set_stroke_color(
        content,
        PdfColor {
            red: 0.45,
            green: 0.45,
            blue: 0.45,
        },
    );
    content.set_line_width(0.75);
    content.rect(fragment.x, fragment.y, width, height);
    content.stroke();
    if width > 24.0 && height > 24.0 {
        stroke_line(
            content,
            fragment.x,
            fragment.y,
            fragment.x + width,
            fragment.y + height,
            0.5,
        );
        stroke_line(
            content,
            fragment.x,
            fragment.y + height,
            fragment.x + width,
            fragment.y,
            0.5,
        );
    }
    if !passive_image_placeholder_label_fits(width, height) {
        return;
    }

    let mut style = CharacterStyle::default();
    style.font_size_half_points = (PASSIVE_IMAGE_PLACEHOLDER_LABEL_FONT_SIZE * 2.0) as i32;
    let label = PASSIVE_IMAGE_PLACEHOLDER_LABEL;
    let encoded = encode_pdf_text_for_font(label, PdfFontFamily::Helvetica);
    set_fill_color(
        content,
        PdfColor {
            red: 0.25,
            green: 0.25,
            blue: 0.25,
        },
    );
    write_text_fragment(
        content,
        label,
        font_name_for_style(PdfFontFamily::Helvetica, &style).0,
        Some(PdfFontFamily::Helvetica),
        None,
        &style,
        0.0,
        fragment.x + PASSIVE_IMAGE_PLACEHOLDER_LABEL_PADDING,
        fragment.y + (height / 2.0) - 3.0,
        TextRotation::None,
        &encoded,
        TextRenderingMode::Fill,
    );
}
