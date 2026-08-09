//! PDF rendering and serialization for laid-out documents.
//!
//! Passive-output auditing is isolated in [`audit`] and re-exported here so
//! rendering and policy verification can evolve independently.

mod audit;
pub use audit::{PassivePdfError, PassivePdfIssue, audit_passive_pdf_bytes};

use miniz_oxide::deflate::compress_to_vec_zlib;
use pdf_writer::types::{
    BlendMode, CidFontType, FontFlags, LineCapStyle, LineJoinStyle, Predictor, SystemInfo,
    TextRenderingMode, UnicodeCmap,
};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str};
use ttf_parser::{Face, name_id};

use crate::fonts::{FontAsset, FontProvider, font_asset_pair_kerning_points};
use crate::layout::{
    LayoutDocument, LayoutItem, LineCap, LineJoin, LineStyle, PdfColor, PdfFontFamily,
    TextFragment, TextRotation, passive_pair_kerning_points, passive_supplied_font_fallback_name,
    source_dingbat_advance_points, source_word_compatible_advance_points,
    source_word_compatible_pair_kerning_points, style_uses_passive_kerning, twips_to_points,
};
use crate::model::{
    BorderStyle, CharacterEmphasisMark, CharacterStyle, ImageFormat, ImageToneAdjustment,
    ShadingPattern, StaticImageTextHorizontalAlign, StaticImageTextVerticalAlign,
    StaticImageVectorCommand, StaticImageVectorFillRule, StaticImageVectorPathSegment, TextRelief,
    UnderlineStyle,
};

const HELVETICA_REGULAR: &[u8] = b"F1";
const HELVETICA_BOLD: &[u8] = b"F2";
const HELVETICA_ITALIC: &[u8] = b"F3";
const HELVETICA_BOLD_ITALIC: &[u8] = b"F4";
const COURIER_REGULAR: &[u8] = b"F5";
const COURIER_BOLD: &[u8] = b"F6";
const COURIER_ITALIC: &[u8] = b"F7";
const COURIER_BOLD_ITALIC: &[u8] = b"F8";
const TIMES_REGULAR: &[u8] = b"F9";
const TIMES_BOLD: &[u8] = b"F10";
const TIMES_ITALIC: &[u8] = b"F11";
const TIMES_BOLD_ITALIC: &[u8] = b"F12";
const SYMBOL_REGULAR: &[u8] = b"F13";
const ZAPF_DINGBATS_REGULAR: &[u8] = b"F14";
const PASSIVE_JPEG_LUMINOSITY_GS: &[u8] = b"GSImageLuminosity";
const PASSIVE_SHAPE_ALPHA_PREFIX: &[u8] = b"GSShapeAlpha";

const BUILTIN_FONTS: [(&[u8], &[u8]); 14] = [
    (HELVETICA_REGULAR, b"Helvetica"),
    (HELVETICA_BOLD, b"Helvetica-Bold"),
    (HELVETICA_ITALIC, b"Helvetica-Oblique"),
    (HELVETICA_BOLD_ITALIC, b"Helvetica-BoldOblique"),
    (COURIER_REGULAR, b"Courier"),
    (COURIER_BOLD, b"Courier-Bold"),
    (COURIER_ITALIC, b"Courier-Oblique"),
    (COURIER_BOLD_ITALIC, b"Courier-BoldOblique"),
    (TIMES_REGULAR, b"Times-Roman"),
    (TIMES_BOLD, b"Times-Bold"),
    (TIMES_ITALIC, b"Times-Italic"),
    (TIMES_BOLD_ITALIC, b"Times-BoldItalic"),
    (SYMBOL_REGULAR, b"Symbol"),
    (ZAPF_DINGBATS_REGULAR, b"ZapfDingbats"),
];

const PASSIVE_SYMBOL_TO_UNICODE: &[(u8, char)] = &[
    (b'A', '\u{0391}'),
    (b'B', '\u{0392}'),
    (b'C', '\u{03a7}'),
    (b'D', '\u{0394}'),
    (b'E', '\u{0395}'),
    (b'F', '\u{03a6}'),
    (b'G', '\u{0393}'),
    (b'H', '\u{0397}'),
    (b'I', '\u{0399}'),
    (b'J', '\u{03d1}'),
    (b'K', '\u{039a}'),
    (b'L', '\u{039b}'),
    (b'M', '\u{039c}'),
    (b'N', '\u{039d}'),
    (b'O', '\u{039f}'),
    (b'P', '\u{03a0}'),
    (b'Q', '\u{0398}'),
    (b'R', '\u{03a1}'),
    (b'S', '\u{03a3}'),
    (b'T', '\u{03a4}'),
    (b'U', '\u{03a5}'),
    (b'V', '\u{03c2}'),
    (b'W', '\u{03a9}'),
    (b'X', '\u{039e}'),
    (b'Y', '\u{03a8}'),
    (b'Z', '\u{0396}'),
    (b'a', '\u{03b1}'),
    (b'b', '\u{03b2}'),
    (b'c', '\u{03c7}'),
    (b'd', '\u{03b4}'),
    (b'e', '\u{03b5}'),
    (b'f', '\u{03c6}'),
    (b'g', '\u{03b3}'),
    (b'h', '\u{03b7}'),
    (b'i', '\u{03b9}'),
    (b'j', '\u{03d5}'),
    (b'k', '\u{03ba}'),
    (b'l', '\u{03bb}'),
    (b'm', '\u{03bc}'),
    (b'n', '\u{03bd}'),
    (b'o', '\u{03bf}'),
    (b'p', '\u{03c0}'),
    (b'q', '\u{03b8}'),
    (b'r', '\u{03c1}'),
    (b's', '\u{03c3}'),
    (b't', '\u{03c4}'),
    (b'u', '\u{03c5}'),
    (b'v', '\u{03d6}'),
    (b'w', '\u{03c9}'),
    (b'x', '\u{03be}'),
    (b'y', '\u{03c8}'),
    (b'z', '\u{03b6}'),
    (b' ', ' '),
    (b'"', '\u{2200}'),
    (b'$', '\u{2203}'),
    (b'\'', '\u{220b}'),
    (b'*', '\u{2217}'),
    (b'-', '\u{2212}'),
    (b'@', '\u{2245}'),
    (b'\\', '\u{2234}'),
    (b'^', '\u{22a5}'),
    (b'~', '\u{223c}'),
    (0xa1, '\u{03d2}'),
    (0xa2, '\u{2032}'),
    (0xa3, '\u{2264}'),
    (0xa4, '\u{2044}'),
    (0xa5, '\u{221e}'),
    (0xa6, '\u{0192}'),
    (0xa7, '\u{2663}'),
    (0xa8, '\u{2666}'),
    (0xa9, '\u{2665}'),
    (0xaa, '\u{2660}'),
    (0xab, '\u{2194}'),
    (0xac, '\u{2190}'),
    (0xad, '\u{2191}'),
    (0xae, '\u{2192}'),
    (0xaf, '\u{2193}'),
    (0xb0, '\u{00b0}'),
    (0xb1, '\u{00b1}'),
    (0xb2, '\u{2033}'),
    (0xb3, '\u{2265}'),
    (0xb4, '\u{00d7}'),
    (0xb5, '\u{221d}'),
    (0xb6, '\u{2202}'),
    (0xb7, '\u{2022}'),
    (0xb8, '\u{00f7}'),
    (0xb9, '\u{2260}'),
    (0xba, '\u{2261}'),
    (0xbb, '\u{2248}'),
    (0xbc, '\u{2026}'),
    (0xbd, '\u{23d0}'),
    (0xbe, '\u{23af}'),
    (0xbf, '\u{21b5}'),
    (0xc0, '\u{2135}'),
    (0xc1, '\u{2111}'),
    (0xc2, '\u{211c}'),
    (0xc3, '\u{2118}'),
    (0xc4, '\u{2297}'),
    (0xc5, '\u{2295}'),
    (0xc6, '\u{2205}'),
    (0xc7, '\u{2229}'),
    (0xc8, '\u{222a}'),
    (0xc9, '\u{2283}'),
    (0xca, '\u{2287}'),
    (0xcb, '\u{2284}'),
    (0xcc, '\u{2282}'),
    (0xcd, '\u{2286}'),
    (0xce, '\u{2208}'),
    (0xcf, '\u{2209}'),
    (0xd0, '\u{2220}'),
    (0xd1, '\u{2207}'),
    (0xd2, '\u{00ae}'),
    (0xd3, '\u{00a9}'),
    (0xd4, '\u{2122}'),
    (0xd5, '\u{220f}'),
    (0xd6, '\u{221a}'),
    (0xd7, '\u{22c5}'),
    (0xd8, '\u{00ac}'),
    (0xd9, '\u{2227}'),
    (0xda, '\u{2228}'),
    (0xdb, '\u{21d4}'),
    (0xdc, '\u{21d0}'),
    (0xdd, '\u{21d1}'),
    (0xde, '\u{21d2}'),
    (0xdf, '\u{21d3}'),
    (0xe0, '\u{25ca}'),
    (0xe1, '\u{2329}'),
    (0xe5, '\u{2211}'),
    (0xe6, '\u{239b}'),
    (0xe7, '\u{239c}'),
    (0xe8, '\u{239d}'),
    (0xe9, '\u{23a1}'),
    (0xea, '\u{23a2}'),
    (0xeb, '\u{23a3}'),
    (0xec, '\u{23a7}'),
    (0xed, '\u{23a8}'),
    (0xee, '\u{23a9}'),
    (0xef, '\u{23aa}'),
    (0xf0, '\u{20ac}'),
    (0xf1, '\u{232a}'),
    (0xf2, '\u{222b}'),
    (0xf3, '\u{2320}'),
    (0xf4, '\u{23ae}'),
    (0xf5, '\u{2321}'),
    (0xf7, '\u{239e}'),
    (0xf8, '\u{239f}'),
    (0xf9, '\u{23a0}'),
    (0xfa, '\u{23a4}'),
    (0xfb, '\u{23a5}'),
    (0xfc, '\u{23a6}'),
    (0xfd, '\u{23ab}'),
    (0xfe, '\u{23ac}'),
    (0xff, '\u{23ad}'),
];

const PASSIVE_ZAPF_DINGBATS_TO_UNICODE: &[(u8, char)] = &[
    (b' ', ' '),
    (b'q', '\u{2610}'),
    (b'J', '\u{263a}'),
    (b'3', '\u{2713}'),
    (b'7', '\u{2717}'),
];

#[derive(Debug, Clone, Default)]
struct ExtendedLatinUsage {
    entries: Vec<ExtendedLatinEntry>,
}

impl ExtendedLatinUsage {
    fn add(&mut self, entry: ExtendedLatinEntry) {
        if !self
            .entries
            .iter()
            .any(|existing| existing.byte == entry.byte)
        {
            self.entries.push(entry);
        }
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn to_unicode_entries(&self) -> Vec<(u8, char)> {
        self.entries
            .iter()
            .map(|entry| (entry.byte, entry.unicode))
            .collect()
    }
}

#[derive(Debug, Copy, Clone)]
struct ExtendedLatinEntry {
    byte: u8,
    unicode: char,
    glyph_name: &'static [u8],
}

#[derive(Debug, Clone)]
struct SuppliedPdfFont {
    asset_index: usize,
    resource_name: Vec<u8>,
    base_name: Vec<u8>,
    type0_ref: Ref,
    cid_ref: Ref,
    descriptor_ref: Ref,
    font_file_ref: Ref,
    cid_to_gid_map_ref: Ref,
    to_unicode_ref: Ref,
    used_glyphs: Vec<SuppliedGlyph>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
struct SuppliedGlyph {
    cid: u16,
    gid: u16,
    unicode: char,
    width: f32,
}

#[derive(Debug, Clone)]
struct SuppliedTextEncoding {
    font_index: usize,
    encoded: Vec<u8>,
    positioning_points: Vec<f32>,
}

mod fonts;
mod images;
mod plan;
mod text;
mod vector;
mod writer;

use fonts::*;
use images::*;
use plan::*;
pub(crate) use text::passive_base14_covers_latin_extended_char;
use text::*;
use vector::*;
pub use writer::{estimate_passive_pdf_object_count, render_pdf, render_pdf_with_font_provider};
#[cfg(test)]
mod tests {
    use crate::fonts::{FontAsset, FontAssetStyle, FontProvider};
    use crate::layout::{DrawingFragment, LayoutEngine, LayoutItem};
    use crate::model::{
        Alignment, Block, BorderStyle, CharacterStyle, Color, Document, DynamicField, FontDef,
        FontFamilyHint, FontPitch, ImageCrop, ImageFormat, ImageToneAdjustment, InlineKind,
        PageSettings, Paragraph, ParagraphStyle, Run, StaticImage, StaticImageAlphaMask,
        StaticShape, StaticShapeArrowhead, StaticShapeKind, StaticShapeLineCap,
        StaticShapeLineJoin, StaticShapeTextVerticalAnchor, Table, TableCell, TableCellBorder,
        TableRow, TableRowWrapMargins, UnderlineStyle,
    };
    use crate::rtf::parse_rtf;
    use lopdf::{Dictionary, Object};

    use super::*;

    fn page_font_resource_names(pdf: &[u8], page_index: usize) -> Vec<Vec<u8>> {
        let parsed = lopdf::Document::load_mem(pdf).unwrap();
        let page_id = *parsed
            .get_pages()
            .values()
            .nth(page_index)
            .expect("page index");
        let page = parsed.get_object(page_id).unwrap().as_dict().unwrap();
        let resources = pdf_object_dict(&parsed, page.get(b"Resources").unwrap());
        let fonts = pdf_object_dict(&parsed, resources.get(b"Font").unwrap());
        fonts.iter().map(|(name, _object)| name.to_vec()).collect()
    }

    fn pdf_object_dict<'a>(parsed: &'a lopdf::Document, object: &'a Object) -> &'a Dictionary {
        match object {
            Object::Reference(id) => parsed.get_object(*id).unwrap().as_dict().unwrap(),
            _ => object.as_dict().unwrap(),
        }
    }

    fn minimal_jpeg_with_dimensions(width: u16, height: u16) -> Vec<u8> {
        minimal_jpeg_with_components(width, height, 3)
    }

    fn minimal_grayscale_jpeg_with_dimensions(width: u16, height: u16) -> Vec<u8> {
        minimal_jpeg_with_components(width, height, 1)
    }

    fn minimal_cmyk_jpeg_with_dimensions(width: u16, height: u16) -> Vec<u8> {
        minimal_jpeg_with_components(width, height, 4)
    }

    fn minimal_jpeg_with_components(width: u16, height: u16, components: u8) -> Vec<u8> {
        let [height_hi, height_lo] = height.to_be_bytes();
        let [width_hi, width_lo] = width.to_be_bytes();
        let segment_len = 8 + u16::from(components) * 3;
        let [segment_hi, segment_lo] = segment_len.to_be_bytes();
        let mut jpeg = vec![
            0xff, 0xd8, 0xff, 0xc0, segment_hi, segment_lo, 0x08, height_hi, height_lo, width_hi,
            width_lo, components,
        ];
        for component in 1..=components {
            jpeg.extend_from_slice(&[component, 0x11, 0x00]);
        }
        jpeg.extend_from_slice(&[0xff, 0xd9]);
        jpeg
    }

    fn minimal_png_idat_for_1x1_rgb() -> Vec<u8> {
        vec![
            0x78, 0x01, 0x01, 0x04, 0x00, 0xfb, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00,
            0x01,
        ]
    }

    fn minimal_png_idat_for_1x1_indexed() -> Vec<u8> {
        vec![
            0x78, 0x01, 0x01, 0x02, 0x00, 0xfd, 0xff, 0x00, 0x01, 0x00, 0x02, 0x00, 0x02,
        ]
    }

    fn pdf_number(object: lopdf::Object) -> Option<f32> {
        match object {
            lopdf::Object::Integer(value) => Some(value as f32),
            lopdf::Object::Real(value) => Some(value),
            _ => None,
        }
    }

    fn content_text(content: &lopdf::content::Content) -> String {
        let mut text = String::new();
        for operation in &content.operations {
            if operation.operator == "Tj" {
                for operand in &operation.operands {
                    if let Ok(bytes) = operand.as_str() {
                        text.push_str(&decode_pdf_text_bytes(bytes));
                    }
                }
            } else if operation.operator == "TJ" {
                for operand in &operation.operands {
                    if let Ok(items) = operand.as_array() {
                        for item in items {
                            if let Ok(bytes) = item.as_str() {
                                text.push_str(&decode_pdf_text_bytes(bytes));
                            }
                        }
                    }
                }
            }
        }
        text
    }

    fn decode_pdf_text_bytes(bytes: &[u8]) -> String {
        if bytes_look_like_utf16be_cids(bytes) {
            let utf16 = bytes
                .chunks_exact(2)
                .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
                .collect::<Vec<_>>();
            if let Ok(decoded) = String::from_utf16(&utf16) {
                return decoded;
            }
        }
        String::from_utf8_lossy(bytes).into_owned()
    }

    fn bytes_look_like_utf16be_cids(bytes: &[u8]) -> bool {
        if bytes.len() < 2 || bytes.len() % 2 != 0 {
            return false;
        }
        let chunks = bytes.len() / 2;
        let zero_high_bytes = bytes.chunks_exact(2).filter(|chunk| chunk[0] == 0).count();
        zero_high_bytes * 2 >= chunks
    }

    fn content_bytes(content: &lopdf::content::Content) -> Vec<u8> {
        let mut text = Vec::new();
        for operation in &content.operations {
            if operation.operator == "Tj" {
                for operand in &operation.operands {
                    if let Ok(bytes) = operand.as_str() {
                        text.extend_from_slice(bytes);
                    }
                }
            } else if operation.operator == "TJ" {
                for operand in &operation.operands {
                    if let Ok(items) = operand.as_array() {
                        for item in items {
                            if let Ok(bytes) = item.as_str() {
                                text.extend_from_slice(bytes);
                            }
                        }
                    }
                }
            }
        }
        text
    }

    fn content_bytes_for_font(content: &lopdf::content::Content, font_name: &[u8]) -> Vec<u8> {
        let mut current_font: Option<Vec<u8>> = None;
        let mut text = Vec::new();
        for operation in &content.operations {
            if operation.operator == "Tf" {
                current_font = operation
                    .operands
                    .first()
                    .and_then(|operand| operand.as_name().ok().map(|name| name.to_vec()));
            } else if operation.operator == "Tj" && current_font.as_deref() == Some(font_name) {
                for operand in &operation.operands {
                    if let Ok(bytes) = operand.as_str() {
                        text.extend_from_slice(bytes);
                    }
                }
            } else if operation.operator == "TJ" && current_font.as_deref() == Some(font_name) {
                for operand in &operation.operands {
                    if let Ok(items) = operand.as_array() {
                        for item in items {
                            if let Ok(bytes) = item.as_str() {
                                text.extend_from_slice(bytes);
                            }
                        }
                    }
                }
            }
        }
        text
    }

    // Verbatim tests are grouped by subsystem; shared fixtures remain above.
    mod fonts {
        use super::*;
        include!("pdf_tests/fonts.rs");
    }
    mod images {
        use super::*;
        include!("pdf_tests/images.rs");
    }
    mod text {
        use super::*;
        include!("pdf_tests/text.rs");
    }
    mod vector {
        use super::*;
        include!("pdf_tests/vector.rs");
    }
    mod writer {
        use super::*;
        include!("pdf_tests/writer.rs");
    }
}
