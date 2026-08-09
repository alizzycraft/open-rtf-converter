use std::fs;
#[cfg(not(feature = "cli"))]
use std::path::Path;

use jpeg_decoder::{Decoder as TestJpegDecoder, PixelFormat as TestJpegPixelFormat};
use lopdf::{Document as PdfDocument, Object as PdfObject};
use open_rtf_converter::audit_passive_pdf_bytes;
#[cfg(feature = "cli")]
use open_rtf_converter::convert_rtf_file_to_pdf;
use open_rtf_converter::layout::{LayoutEngine, LayoutItem, LineJoin, LineStyle};
use open_rtf_converter::model::{
    Alignment, Block, BorderStyle, CharacterEmphasisMark, CharacterStyle, Color, DynamicField,
    EndnotePlacement, FontFamilyHint, FontPitch, ImageFormat, ImageToneAdjustment, InlineKind,
    MathInline, PageVerticalAlignment, ParagraphFrameVerticalAlignment, PassiveMathFractionPart,
    PassiveMathLimitPart, ShadingPattern, StaticImageTextHorizontalAlign,
    StaticImageTextVerticalAlign, StaticImageVectorCommand, StaticImageVectorFillRule,
    StaticImageVectorPathSegment, StaticImageWrapSide, StaticShapeArrowhead,
    StaticShapeHorizontalAnchor, StaticShapeKind, StaticShapeLineCap, StaticShapeLineJoin,
    StaticShapeTextVerticalAnchor, StaticShapeVerticalAnchor, TabAlignment, TableCellTextDirection,
    TableRowAlignment, TextRelief, UnderlineStyle,
};

const PAGE_NUMBER_MARKER: &str = "\u{f0000}";
const TOTAL_PAGES_MARKER: &str = "\u{f0001}";
const SECTION_NUMBER_MARKER: &str = "\u{f0002}";
const PENDING_NOTE_REFERENCE_MARKER: &str = "\u{f0003}";
const SECTION_PAGES_MARKER: &str = "\u{f0004}";
const DOCUMENT_WORDS_MARKER: &str = "\u{f0008}";
const DOCUMENT_CHARS_MARKER: &str = "\u{f0009}";
const DOCUMENT_CHARS_WITH_SPACES_MARKER: &str = "\u{f000a}";
const BOOKMARK_PAGE_ANCHOR_MARKER: &str = "\u{f0005}";
const BOOKMARK_PAGE_REF_MARKER: &str = "\u{f0006}";
const BOOKMARK_PAGE_MARKER_END: &str = "\u{f0007}";
const PASSIVE_ADVANCE_MARKER: &str = "\u{f000b}";
const FOOTNOTE_REFERENCE_MARKER: &str = "\u{f000c}";
const FOOTNOTE_REFERENCE_MARKER_END: &str = "\u{f000d}";
const ENDNOTE_REFERENCE_MARKER: &str = "\u{f000e}";
const ENDNOTE_REFERENCE_MARKER_END: &str = "\u{f000f}";
const NESTED_TABLE_ANCHOR_MARKER: &str = "\u{f0010}";
const PASSIVE_MATH_FRACTION_RULE_MARKER: &str = "\u{f0011}";
const PASSIVE_MATH_STACK_ANCHOR_MARKER: &str = "\u{f0012}";
const PASSIVE_MATH_INLINE_LEAD_MARKER: &str = "\u{f0013}";
const INLINE_IMAGE_MARKER_BASE: u32 = 0xF1000;

fn inline_image_marker(index: usize) -> Option<char> {
    let codepoint = INLINE_IMAGE_MARKER_BASE.checked_add(u32::try_from(index).ok()?)?;
    char::from_u32(codepoint).filter(|ch| *ch <= '\u{ffffd}')
}

use open_rtf_converter::rtf::{
    LexError, ParseError, parse_rtf_bytes, parse_rtf_bytes_with_options,
};
use open_rtf_converter::{
    ActiveContentPolicy, CompatibilityMode, ConvertOptions, Diagnostic, FontAsset, FontAssetStyle,
    PdfLinkPolicy, RtfLimits, RtfParseOptions, convert_rtf_to_pdf,
};
#[cfg(not(feature = "cli"))]
use open_rtf_converter::{ConvertError, ConvertReport};
use tempfile::tempdir;

#[cfg(not(feature = "cli"))]
fn convert_rtf_file_to_pdf(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &ConvertOptions,
) -> Result<ConvertReport, ConvertError> {
    let input = fs::read(input).expect("test fixture should be readable");
    let converted = convert_rtf_to_pdf(&input, options)?;
    fs::write(output, &converted.pdf).expect("test output should be writable");
    Ok(ConvertReport {
        diagnostics: converted.diagnostics,
        pages: converted.pages,
    })
}

fn footnote_reference_marker(index: usize) -> String {
    format!("{FOOTNOTE_REFERENCE_MARKER}{index}{FOOTNOTE_REFERENCE_MARKER_END}")
}

fn endnote_reference_marker(index: usize) -> String {
    format!("{ENDNOTE_REFERENCE_MARKER}{index}{ENDNOTE_REFERENCE_MARKER_END}")
}

fn parse_rtf_bytes_strict(input: &[u8]) -> open_rtf_converter::rtf::ParseOutput {
    parse_rtf_bytes_with_options(
        input,
        &RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        },
    )
    .expect("strict-spec RTF parse")
}

fn pdf_bytes_excluding_embedded_font_programs(pdf: &[u8]) -> Vec<u8> {
    let document = PdfDocument::load_mem(pdf).expect("valid generated PDF");
    let mut font_stream_ids = std::collections::BTreeSet::new();
    for object in document.objects.values() {
        let dictionary = match object {
            PdfObject::Dictionary(dictionary) => Some(dictionary),
            PdfObject::Stream(stream) => Some(&stream.dict),
            _ => None,
        };
        let Some(dictionary) = dictionary else {
            continue;
        };
        for key in [b"FontFile".as_slice(), b"FontFile2", b"FontFile3"] {
            if let Ok(PdfObject::Reference(id)) = dictionary.get(key) {
                font_stream_ids.insert(*id);
            }
        }
    }

    let mut searchable = Vec::new();
    for (id, object) in &document.objects {
        match object {
            PdfObject::Stream(stream) if !font_stream_ids.contains(id) => {
                searchable.extend_from_slice(format!("{:?}", stream.dict).as_bytes());
                searchable.extend_from_slice(
                    &stream
                        .decompressed_content()
                        .unwrap_or_else(|_| stream.content.clone()),
                );
            }
            PdfObject::Stream(_) => {}
            other => searchable.extend_from_slice(format!("{other:?}").as_bytes()),
        }
        searchable.push(b'\n');
    }
    searchable
}

fn object_with_payload(with_result: bool) -> Vec<u8> {
    if with_result {
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "object{",
            "\\",
            "objdata ",
            payload_hex(),
            "}{",
            "\\",
            "result visible fallback}}}",
        ])
    } else {
        rtf(&[
            "{",
            "\\",
            "rtf1{",
            "\\",
            "object",
            "\\",
            "objocx",
            "\\",
            "objdata ",
            payload_hex(),
            "}}",
        ])
    }
}

fn field_with_link_result() -> Vec<u8> {
    rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst HYPER",
        "LINK \"https://example.com\"}{",
        "\\",
        "fldrslt visible link}}}",
    ])
}

fn payload_hex() -> &'static str {
    "414243"
}

fn rtf(parts: &[&str]) -> Vec<u8> {
    let mut output = Vec::new();
    for part in parts {
        output.extend_from_slice(part.as_bytes());
    }
    output
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
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

fn valid_rgb_jpeg_1x1() -> Vec<u8> {
    const JPEG_HEX: &str = concat!(
        "ffd8ffe000104a46494600010101006000600000ffdb0043000302020302020303030304030304050805050404050a070706080c0a0c0c0b0a0b0b0d0e12100d0e110e0b0b1016101113141515150c0f171816141812141514",
        "ffdb00430103040405040509050509140d0b0d1414141414141414141414141414141414141414141414141414141414141414141414141414141414141414141414141414141414141414141414",
        "ffc00011080001000103012200021101031101ffc4001f0000010501010101010100000000000000000102030405060708090a0bffc400b5100002010303020403050504040000017d01020300041105122131410613516107227114328191a1082342b1c11552d1f02433627282090a161718191a25262728292a3435363738393a434445464748494a535455565758595a636465666768696a737475767778797a838485868788898a92939495969798999aa2a3a4a5a6a7a8a9aab2b3b4b5b6b7b8b9bac2c3c4c5c6c7c8c9cad2d3d4d5d6d7d8d9dae1e2e3e4e5e6e7e8e9eaf1f2f3f4f5f6f7f8f9fa",
        "ffc4001f0100030101010101010101010000000000000102030405060708090a0bffc400b51100020102040403040705040400010277000102031104052131061241510761711322328108144291a1b1c109233352f0156272d10a162434e125f11718191a262728292a35363738393a434445464748494a535455565758595a636465666768696a737475767778797a82838485868788898a92939495969798999aa2a3a4a5a6a7a8a9aab2b3b4b5b6b7b8b9bac2c3c4c5c6c7c8c9cad2d3d4d5d6d7d8d9dae2e3e4e5e6e7e8e9eaf2f3f4f5f6f7f8f9fa",
        "ffda000c03010002110311003f00f9d28a28af8a3fbacfffd9"
    );
    hex_bytes(JPEG_HEX)
}

fn valid_rgb_jpeg_1x1_color() -> Color {
    let jpeg = valid_rgb_jpeg_1x1();
    let mut decoder = TestJpegDecoder::new(jpeg.as_slice());
    let decoded = decoder.decode().expect("valid RGB JPEG should decode");
    let info = decoder.info().expect("valid RGB JPEG should report info");
    assert_eq!(info.width, 1);
    assert_eq!(info.height, 1);
    match info.pixel_format {
        TestJpegPixelFormat::RGB24 => {
            assert_eq!(decoded.len(), 3);
            Color {
                red: decoded[0],
                green: decoded[1],
                blue: decoded[2],
            }
        }
        other => panic!("valid RGB JPEG should decode as RGB24, got {other:?}"),
    }
}

fn minimal_jpeg_with_components(width: u16, height: u16, components: u8) -> Vec<u8> {
    let template = match components {
        1 => include_str!("../fixtures/test-images/valid-grayscale-1x1.jpg.hex"),
        3 => include_str!("../fixtures/test-images/valid-rgb-1x1.jpg.hex"),
        4 => include_str!("../fixtures/test-images/valid-cmyk-1x1.jpg.hex"),
        _ => return minimal_jpeg_frame_header_with_components(width, height, components),
    };
    let mut jpeg = hex_bytes(template.trim());
    let sof = jpeg
        .windows(2)
        .position(|window| window == [0xff, 0xc0])
        .expect("test JPEG SOF0 marker");
    jpeg[sof + 5..sof + 7].copy_from_slice(&height.to_be_bytes());
    jpeg[sof + 7..sof + 9].copy_from_slice(&width.to_be_bytes());
    jpeg
}

fn minimal_jpeg_frame_header_with_components(width: u16, height: u16, components: u8) -> Vec<u8> {
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

fn jpeg_with_large_ignored_app_segments(jpeg: Vec<u8>) -> Vec<u8> {
    const APP_PAYLOAD_LEN: usize = (u16::MAX as usize) - 2;
    const APP_SEGMENT_COUNT: usize = 4;

    assert!(jpeg.starts_with(&[0xff, 0xd8]));
    let app_bytes = APP_SEGMENT_COUNT * (APP_PAYLOAD_LEN + 4);
    let mut output = Vec::with_capacity(jpeg.len() + app_bytes);
    output.extend_from_slice(&jpeg[..2]);
    for _ in 0..APP_SEGMENT_COUNT {
        output.extend_from_slice(&[0xff, 0xef]);
        output.extend_from_slice(&u16::MAX.to_be_bytes());
        output.resize(output.len() + APP_PAYLOAD_LEN, 0);
    }
    output.extend_from_slice(&jpeg[2..]);
    output
}

fn hex_bytes(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0);
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
        .collect()
}

fn minimal_emf_with_bounds_and_frame(
    width_px: i32,
    height_px: i32,
    frame_width_hundredth_mm: i32,
    frame_height_hundredth_mm: i32,
) -> Vec<u8> {
    minimal_emf_with_records(
        width_px,
        height_px,
        frame_width_hundredth_mm,
        frame_height_hundredth_mm,
        &[],
    )
}

fn minimal_emf_with_records(
    width_px: i32,
    height_px: i32,
    frame_width_hundredth_mm: i32,
    frame_height_hundredth_mm: i32,
    records: &[Vec<u8>],
) -> Vec<u8> {
    let mut emf = vec![0; 88];
    write_test_le_u32(&mut emf, 0, 1);
    write_test_le_u32(&mut emf, 4, 88);
    write_test_le_i32(&mut emf, 8, 0);
    write_test_le_i32(&mut emf, 12, 0);
    write_test_le_i32(&mut emf, 16, width_px);
    write_test_le_i32(&mut emf, 20, height_px);
    write_test_le_i32(&mut emf, 24, 0);
    write_test_le_i32(&mut emf, 28, 0);
    write_test_le_i32(&mut emf, 32, frame_width_hundredth_mm);
    write_test_le_i32(&mut emf, 36, frame_height_hundredth_mm);
    write_test_le_u32(&mut emf, 40, 0x464d_4520);
    write_test_le_u32(&mut emf, 44, 0x0001_0000);
    write_test_le_i32(&mut emf, 72, 1920);
    write_test_le_i32(&mut emf, 76, 1080);
    write_test_le_i32(&mut emf, 80, 508);
    write_test_le_i32(&mut emf, 84, 286);
    for record in records {
        emf.extend_from_slice(record);
    }
    if !records.is_empty() {
        emf.extend_from_slice(&emf_eof_record());
    }
    let byte_count = emf.len() as u32;
    let eof_record_count = if records.is_empty() { 0 } else { 1 };
    write_test_le_u32(&mut emf, 52, 1 + records.len() as u32 + eof_record_count);
    write_test_le_u32(&mut emf, 48, byte_count);
    emf
}

fn minimal_wmf_with_records(width: i16, height: i16, records: &[Vec<u8>]) -> Vec<u8> {
    let mut wmf = vec![0; 18];
    write_test_le_u16(&mut wmf, 0, 1);
    write_test_le_u16(&mut wmf, 2, 9);
    write_test_le_u16(&mut wmf, 4, 0x0300);

    let set_window_ext = wmf_set_window_ext_record(width, height);
    let eof = wmf_eof_record();
    let mut max_record_words = set_window_ext.len() / 2;
    wmf.extend_from_slice(&set_window_ext);
    for record in records {
        max_record_words = max_record_words.max(record.len() / 2);
        wmf.extend_from_slice(record);
    }
    max_record_words = max_record_words.max(eof.len() / 2);
    wmf.extend_from_slice(&eof);

    let file_size_words = (wmf.len() / 2) as u32;
    write_test_le_u32(&mut wmf, 6, file_size_words);
    write_test_le_u16(&mut wmf, 10, 0);
    write_test_le_u32(&mut wmf, 12, max_record_words as u32);
    write_test_le_u16(&mut wmf, 16, 0);
    wmf
}

fn wmf_set_window_ext_record(width: i16, height: i16) -> Vec<u8> {
    let mut record = vec![0; 10];
    write_test_le_u32(&mut record, 0, 5);
    write_test_le_u16(&mut record, 4, 0x020c);
    write_test_le_i16(&mut record, 6, height);
    write_test_le_i16(&mut record, 8, width);
    record
}

fn wmf_function_record(function: u16) -> Vec<u8> {
    let mut record = vec![0; 6];
    write_test_le_u32(&mut record, 0, 3);
    write_test_le_u16(&mut record, 4, function);
    record
}

fn wmf_u16_record(function: u16, value: u16) -> Vec<u8> {
    let mut record = vec![0; 8];
    write_test_le_u32(&mut record, 0, 4);
    write_test_le_u16(&mut record, 4, function);
    write_test_le_u16(&mut record, 6, value);
    record
}

fn wmf_u32_record(function: u16, value: u32) -> Vec<u8> {
    let mut record = vec![0; 10];
    write_test_le_u32(&mut record, 0, 5);
    write_test_le_u16(&mut record, 4, function);
    write_test_le_u32(&mut record, 6, value);
    record
}

fn wmf_i16_pair_record(function: u16, first: i16, second: i16) -> Vec<u8> {
    let mut record = vec![0; 10];
    write_test_le_u32(&mut record, 0, 5);
    write_test_le_u16(&mut record, 4, function);
    write_test_le_i16(&mut record, 6, first);
    write_test_le_i16(&mut record, 8, second);
    record
}

fn wmf_scale_record(function: u16, y_denom: i16, y_num: i16, x_denom: i16, x_num: i16) -> Vec<u8> {
    let mut record = vec![0; 14];
    write_test_le_u32(&mut record, 0, 7);
    write_test_le_u16(&mut record, 4, function);
    write_test_le_i16(&mut record, 6, y_denom);
    write_test_le_i16(&mut record, 8, y_num);
    write_test_le_i16(&mut record, 10, x_denom);
    write_test_le_i16(&mut record, 12, x_num);
    record
}

fn wmf_create_palette_record(entries: &[(u8, u8, u8)]) -> Vec<u8> {
    let mut record = vec![0; 10 + entries.len() * 4];
    let record_size_words = (record.len() / 2) as u32;
    write_test_le_u32(&mut record, 0, record_size_words);
    write_test_le_u16(&mut record, 4, 0x00f7);
    write_test_le_u16(&mut record, 6, 0x0300);
    write_test_le_u16(&mut record, 8, entries.len() as u16);
    for (index, &(red, green, blue)) in entries.iter().enumerate() {
        let offset = 10 + index * 4;
        record[offset] = red;
        record[offset + 1] = green;
        record[offset + 2] = blue;
        record[offset + 3] = 0;
    }
    record
}

fn wmf_palette_entries_record(
    function: u16,
    start_index: u16,
    entries: &[(u8, u8, u8)],
) -> Vec<u8> {
    let mut record = vec![0; 10 + entries.len() * 4];
    let record_size_words = (record.len() / 2) as u32;
    write_test_le_u32(&mut record, 0, record_size_words);
    write_test_le_u16(&mut record, 4, function);
    write_test_le_u16(&mut record, 6, start_index);
    write_test_le_u16(&mut record, 8, entries.len() as u16);
    for (index, &(red, green, blue)) in entries.iter().enumerate() {
        let offset = 10 + index * 4;
        record[offset] = red;
        record[offset + 1] = green;
        record[offset + 2] = blue;
        record[offset + 3] = 0;
    }
    record
}

fn wmf_bounds_record(function: u16, left: i16, top: i16, right: i16, bottom: i16) -> Vec<u8> {
    let mut record = vec![0; 14];
    write_test_le_u32(&mut record, 0, 7);
    write_test_le_u16(&mut record, 4, function);
    write_test_le_i16(&mut record, 6, bottom);
    write_test_le_i16(&mut record, 8, right);
    write_test_le_i16(&mut record, 10, top);
    write_test_le_i16(&mut record, 12, left);
    record
}

fn wmf_yx_record(function: u16, y: i16, x: i16) -> Vec<u8> {
    let mut record = vec![0; 10];
    write_test_le_u32(&mut record, 0, 5);
    write_test_le_u16(&mut record, 4, function);
    write_test_le_i16(&mut record, 6, y);
    write_test_le_i16(&mut record, 8, x);
    record
}

fn wmf_point_list_record(function: u16, points: &[(i16, i16)]) -> Vec<u8> {
    let size_words = 4 + (points.len() * 2);
    let mut record = vec![0; size_words * 2];
    write_test_le_u32(&mut record, 0, size_words as u32);
    write_test_le_u16(&mut record, 4, function);
    write_test_le_u16(&mut record, 6, points.len() as u16);
    for (idx, (x, y)) in points.iter().enumerate() {
        let offset = 8 + (idx * 4);
        write_test_le_i16(&mut record, offset, *y);
        write_test_le_i16(&mut record, offset + 2, *x);
    }
    record
}

fn wmf_exttextout_record(
    x: i16,
    y: i16,
    text: &str,
    flags: u16,
    bounds: Option<(i16, i16, i16, i16)>,
) -> Vec<u8> {
    let text_bytes = text.as_bytes();
    let has_bounds = flags & 0x0006 != 0;
    let bounds_words = if has_bounds { 4usize } else { 0usize };
    let text_words = text_bytes.len().next_multiple_of(2) / 2;
    let size_words = 7 + bounds_words + text_words;
    let mut record = vec![0; size_words * 2];
    write_test_le_u32(&mut record, 0, size_words as u32);
    write_test_le_u16(&mut record, 4, 0x0a32);
    write_test_le_i16(&mut record, 6, y);
    write_test_le_i16(&mut record, 8, x);
    write_test_le_i16(&mut record, 10, text_bytes.len() as i16);
    write_test_le_u16(&mut record, 12, flags);
    let text_offset = if has_bounds {
        let (left, top, right, bottom) = bounds.unwrap_or((0, 0, 0, 0));
        write_test_le_i16(&mut record, 14, bottom);
        write_test_le_i16(&mut record, 16, right);
        write_test_le_i16(&mut record, 18, top);
        write_test_le_i16(&mut record, 20, left);
        22usize
    } else {
        14usize
    };
    record[text_offset..text_offset + text_bytes.len()].copy_from_slice(text_bytes);
    record
}

fn wmf_textout_record(x: i16, y: i16, text: &[u8]) -> Vec<u8> {
    let text_words = text.len().next_multiple_of(2) / 2;
    let size_words = 5 + text_words + 2;
    let mut record = vec![0; size_words * 2];
    write_test_le_u32(&mut record, 0, size_words as u32);
    write_test_le_u16(&mut record, 4, 0x0521);
    write_test_le_u16(&mut record, 6, text.len() as u16);
    record[8..8 + text.len()].copy_from_slice(text);
    let coordinate_offset = 8 + text.len().next_multiple_of(2);
    write_test_le_i16(&mut record, coordinate_offset, y);
    write_test_le_i16(&mut record, coordinate_offset + 2, x);
    record
}

fn wmf_exttextout_dx_record(
    x: i16,
    y: i16,
    text: &str,
    flags: u16,
    bounds: Option<(i16, i16, i16, i16)>,
    dx: &[i16],
) -> Vec<u8> {
    let text_bytes = text.as_bytes();
    assert_eq!(text_bytes.len(), dx.len());
    let has_bounds = flags & 0x0006 != 0;
    let bounds_words = if has_bounds { 4usize } else { 0usize };
    let text_words = text_bytes.len().next_multiple_of(2) / 2;
    let size_words = 7 + bounds_words + text_words + dx.len();
    let mut record = vec![0; size_words * 2];
    write_test_le_u32(&mut record, 0, size_words as u32);
    write_test_le_u16(&mut record, 4, 0x0a32);
    write_test_le_i16(&mut record, 6, y);
    write_test_le_i16(&mut record, 8, x);
    write_test_le_i16(&mut record, 10, text_bytes.len() as i16);
    write_test_le_u16(&mut record, 12, flags);
    let text_offset = if has_bounds {
        let (left, top, right, bottom) = bounds.unwrap_or((0, 0, 0, 0));
        write_test_le_i16(&mut record, 14, bottom);
        write_test_le_i16(&mut record, 16, right);
        write_test_le_i16(&mut record, 18, top);
        write_test_le_i16(&mut record, 20, left);
        22usize
    } else {
        14usize
    };
    record[text_offset..text_offset + text_bytes.len()].copy_from_slice(text_bytes);
    let dx_offset = text_offset + text_bytes.len().next_multiple_of(2);
    for (index, advance) in dx.iter().enumerate() {
        write_test_le_i16(&mut record, dx_offset + index * 2, *advance);
    }
    record
}

fn wmf_exttextout_pdy_record(
    x: i16,
    y: i16,
    text: &str,
    flags: u16,
    bounds: Option<(i16, i16, i16, i16)>,
    advances: &[(i16, i16)],
) -> Vec<u8> {
    let text_bytes = text.as_bytes();
    assert_eq!(text_bytes.len(), advances.len());
    let has_bounds = flags & 0x0006 != 0;
    let bounds_words = if has_bounds { 4usize } else { 0usize };
    let text_words = text_bytes.len().next_multiple_of(2) / 2;
    let size_words = 7 + bounds_words + text_words + (advances.len() * 2);
    let mut record = vec![0; size_words * 2];
    write_test_le_u32(&mut record, 0, size_words as u32);
    write_test_le_u16(&mut record, 4, 0x0a32);
    write_test_le_i16(&mut record, 6, y);
    write_test_le_i16(&mut record, 8, x);
    write_test_le_i16(&mut record, 10, text_bytes.len() as i16);
    write_test_le_u16(&mut record, 12, flags);
    let text_offset = if has_bounds {
        let (left, top, right, bottom) = bounds.unwrap_or((0, 0, 0, 0));
        write_test_le_i16(&mut record, 14, bottom);
        write_test_le_i16(&mut record, 16, right);
        write_test_le_i16(&mut record, 18, top);
        write_test_le_i16(&mut record, 20, left);
        22usize
    } else {
        14usize
    };
    record[text_offset..text_offset + text_bytes.len()].copy_from_slice(text_bytes);
    let dx_offset = text_offset + text_bytes.len().next_multiple_of(2);
    for (index, (dx, dy)) in advances.iter().enumerate() {
        let offset = dx_offset + index * 4;
        write_test_le_i16(&mut record, offset, *dx);
        write_test_le_i16(&mut record, offset + 2, *dy);
    }
    record
}

fn wmf_create_brush_record(color: Color) -> Vec<u8> {
    let mut record = vec![0; 14];
    write_test_le_u32(&mut record, 0, 7);
    write_test_le_u16(&mut record, 4, 0x02fc);
    write_test_le_u16(&mut record, 6, 0);
    record[8] = color.red;
    record[9] = color.green;
    record[10] = color.blue;
    write_test_le_u16(&mut record, 12, 0);
    record
}

fn wmf_create_pen_record(style: u16, width: i16, color: Color) -> Vec<u8> {
    let mut record = vec![0; 16];
    write_test_le_u32(&mut record, 0, 8);
    write_test_le_u16(&mut record, 4, 0x02fa);
    write_test_le_u16(&mut record, 6, style);
    write_test_le_i16(&mut record, 8, width);
    write_test_le_i16(&mut record, 10, 0);
    record[12] = color.red;
    record[13] = color.green;
    record[14] = color.blue;
    record
}

fn wmf_select_object_record(handle: u16) -> Vec<u8> {
    let mut record = vec![0; 8];
    write_test_le_u32(&mut record, 0, 4);
    write_test_le_u16(&mut record, 4, 0x012d);
    write_test_le_u16(&mut record, 6, handle);
    record
}

fn wmf_patblt_record(x: i16, y: i16, width: i16, height: i16, raster_op: u32) -> Vec<u8> {
    let mut record = vec![0; 18];
    write_test_le_u32(&mut record, 0, 9);
    write_test_le_u16(&mut record, 4, 0x061d);
    write_test_le_u32(&mut record, 6, raster_op);
    write_test_le_i16(&mut record, 10, height);
    write_test_le_i16(&mut record, 12, width);
    write_test_le_i16(&mut record, 14, y);
    write_test_le_i16(&mut record, 16, x);
    record
}

fn wmf_bitblt_record(
    x: i16,
    y: i16,
    width: i16,
    height: i16,
    raster_op: u32,
    payload: &[u8],
) -> Vec<u8> {
    let size = (6 + 16 + payload.len()).next_multiple_of(2);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, (size / 2) as u32);
    write_test_le_u16(&mut record, 4, 0x0922);
    write_test_le_u32(&mut record, 6, raster_op);
    write_test_le_i16(&mut record, 10, 0);
    write_test_le_i16(&mut record, 12, 0);
    write_test_le_i16(&mut record, 14, height);
    write_test_le_i16(&mut record, 16, width);
    write_test_le_i16(&mut record, 18, y);
    write_test_le_i16(&mut record, 20, x);
    record[22..22 + payload.len()].copy_from_slice(payload);
    record
}

fn wmf_stretchblt_record(
    x: i16,
    y: i16,
    width: i16,
    height: i16,
    raster_op: u32,
    payload: &[u8],
) -> Vec<u8> {
    let size = (6 + 20 + payload.len()).next_multiple_of(2);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, (size / 2) as u32);
    write_test_le_u16(&mut record, 4, 0x0b23);
    write_test_le_u32(&mut record, 6, raster_op);
    write_test_le_i16(&mut record, 10, 0);
    write_test_le_i16(&mut record, 12, 0);
    write_test_le_i16(&mut record, 14, 0);
    write_test_le_i16(&mut record, 16, 0);
    write_test_le_i16(&mut record, 18, height);
    write_test_le_i16(&mut record, 20, width);
    write_test_le_i16(&mut record, 22, y);
    write_test_le_i16(&mut record, 24, x);
    record[26..26 + payload.len()].copy_from_slice(payload);
    record
}

fn wmf_stretchdib_dib_record(
    x: i16,
    y: i16,
    width: i16,
    height: i16,
    raster_op: u32,
    dib: &[u8],
) -> Vec<u8> {
    let source_width = if dib.len() >= 12 {
        i32::from_le_bytes(dib[4..8].try_into().unwrap()).clamp(1, i32::from(i16::MAX)) as i16
    } else {
        width
    };
    let source_height = if dib.len() >= 12 {
        i32::from_le_bytes(dib[8..12].try_into().unwrap())
            .unsigned_abs()
            .clamp(1, i16::MAX as u32) as i16
    } else {
        height
    };
    let size = (6 + 22 + dib.len()).next_multiple_of(2);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, (size / 2) as u32);
    write_test_le_u16(&mut record, 4, 0x0f43);
    write_test_le_u32(&mut record, 6, raster_op);
    write_test_le_u16(&mut record, 10, 0);
    write_test_le_i16(&mut record, 12, source_height);
    write_test_le_i16(&mut record, 14, source_width);
    write_test_le_i16(&mut record, 16, 0);
    write_test_le_i16(&mut record, 18, 0);
    write_test_le_i16(&mut record, 20, height);
    write_test_le_i16(&mut record, 22, width);
    write_test_le_i16(&mut record, 24, y);
    write_test_le_i16(&mut record, 26, x);
    record[28..28 + dib.len()].copy_from_slice(dib);
    record
}

#[allow(clippy::too_many_arguments)]
fn wmf_stretchdib_dib_record_with_source(
    x: i16,
    y: i16,
    width: i16,
    height: i16,
    source_x: i16,
    source_y: i16,
    source_width: i16,
    source_height: i16,
    dib: &[u8],
) -> Vec<u8> {
    let mut record = wmf_stretchdib_dib_record(x, y, width, height, 0x00cc_0020, dib);
    write_test_le_i16(&mut record, 12, source_height);
    write_test_le_i16(&mut record, 14, source_width);
    write_test_le_i16(&mut record, 16, source_y);
    write_test_le_i16(&mut record, 18, source_x);
    record
}

fn wmf_setdib_to_dev_record(
    x: i16,
    y: i16,
    width: i16,
    height: i16,
    start_scan: u16,
    scan_count: u16,
    dib: &[u8],
) -> Vec<u8> {
    let size = (6 + 18 + dib.len()).next_multiple_of(2);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, (size / 2) as u32);
    write_test_le_u16(&mut record, 4, 0x0d33);
    write_test_le_u16(&mut record, 6, 0);
    write_test_le_u16(&mut record, 8, scan_count);
    write_test_le_u16(&mut record, 10, start_scan);
    write_test_le_i16(&mut record, 12, 0);
    write_test_le_i16(&mut record, 14, 0);
    write_test_le_i16(&mut record, 16, height);
    write_test_le_i16(&mut record, 18, width);
    write_test_le_i16(&mut record, 20, y);
    write_test_le_i16(&mut record, 22, x);
    record[24..24 + dib.len()].copy_from_slice(dib);
    record
}

fn wmf_dibbitblt_record(
    x: i16,
    y: i16,
    width: i16,
    height: i16,
    raster_op: u32,
    dib: &[u8],
) -> Vec<u8> {
    let size = (6 + 16 + dib.len()).next_multiple_of(2);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, (size / 2) as u32);
    write_test_le_u16(&mut record, 4, 0x0940);
    write_test_le_u32(&mut record, 6, raster_op);
    write_test_le_i16(&mut record, 10, 0);
    write_test_le_i16(&mut record, 12, 0);
    write_test_le_i16(&mut record, 14, height);
    write_test_le_i16(&mut record, 16, width);
    write_test_le_i16(&mut record, 18, y);
    write_test_le_i16(&mut record, 20, x);
    record[22..22 + dib.len()].copy_from_slice(dib);
    record
}

fn wmf_dibbitblt_record_with_source(
    x: i16,
    y: i16,
    width: i16,
    height: i16,
    source_x: i16,
    source_y: i16,
    dib: &[u8],
) -> Vec<u8> {
    let mut record = wmf_dibbitblt_record(x, y, width, height, 0x00cc_0020, dib);
    write_test_le_i16(&mut record, 10, source_y);
    write_test_le_i16(&mut record, 12, source_x);
    record
}

fn wmf_dibstretchblt_record(
    x: i16,
    y: i16,
    width: i16,
    height: i16,
    raster_op: u32,
    dib: &[u8],
) -> Vec<u8> {
    let source_width = if dib.len() >= 12 {
        i32::from_le_bytes(dib[4..8].try_into().unwrap()).clamp(1, i32::from(i16::MAX)) as i16
    } else {
        width
    };
    let source_height = if dib.len() >= 12 {
        i32::from_le_bytes(dib[8..12].try_into().unwrap())
            .unsigned_abs()
            .clamp(1, i16::MAX as u32) as i16
    } else {
        height
    };
    let size = (6 + 20 + dib.len()).next_multiple_of(2);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, (size / 2) as u32);
    write_test_le_u16(&mut record, 4, 0x0b41);
    write_test_le_u32(&mut record, 6, raster_op);
    write_test_le_i16(&mut record, 10, source_height);
    write_test_le_i16(&mut record, 12, source_width);
    write_test_le_i16(&mut record, 14, 0);
    write_test_le_i16(&mut record, 16, 0);
    write_test_le_i16(&mut record, 18, height);
    write_test_le_i16(&mut record, 20, width);
    write_test_le_i16(&mut record, 22, y);
    write_test_le_i16(&mut record, 24, x);
    record[26..26 + dib.len()].copy_from_slice(dib);
    record
}

#[allow(clippy::too_many_arguments)]
fn wmf_dibstretchblt_record_with_source(
    x: i16,
    y: i16,
    width: i16,
    height: i16,
    source_x: i16,
    source_y: i16,
    source_width: i16,
    source_height: i16,
    dib: &[u8],
) -> Vec<u8> {
    let mut record = wmf_dibstretchblt_record(x, y, width, height, 0x00cc_0020, dib);
    write_test_le_i16(&mut record, 10, source_height);
    write_test_le_i16(&mut record, 12, source_width);
    write_test_le_i16(&mut record, 14, source_y);
    write_test_le_i16(&mut record, 16, source_x);
    record
}

fn wmf_eof_record() -> Vec<u8> {
    let mut record = vec![0; 6];
    write_test_le_u32(&mut record, 0, 3);
    write_test_le_u16(&mut record, 4, 0);
    record
}

fn emf_rect_record(record_type: u32, left: i32, top: i32, right: i32, bottom: i32) -> Vec<u8> {
    let mut record = vec![0; 24];
    write_test_le_u32(&mut record, 0, record_type);
    write_test_le_u32(&mut record, 4, 24);
    write_test_le_i32(&mut record, 8, left);
    write_test_le_i32(&mut record, 12, top);
    write_test_le_i32(&mut record, 16, right);
    write_test_le_i32(&mut record, 20, bottom);
    record
}

fn emf_roundrect_record(
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    corner_width: i32,
    corner_height: i32,
) -> Vec<u8> {
    let mut record = vec![0; 32];
    write_test_le_u32(&mut record, 0, 44);
    write_test_le_u32(&mut record, 4, 32);
    write_test_le_i32(&mut record, 8, left);
    write_test_le_i32(&mut record, 12, top);
    write_test_le_i32(&mut record, 16, right);
    write_test_le_i32(&mut record, 20, bottom);
    write_test_le_i32(&mut record, 24, corner_width);
    write_test_le_i32(&mut record, 28, corner_height);
    record
}

fn emf_create_pen_record(handle: u32, style: u32, width: i32, color: Color) -> Vec<u8> {
    let mut record = vec![0; 28];
    write_test_le_u32(&mut record, 0, 38);
    write_test_le_u32(&mut record, 4, 28);
    write_test_le_u32(&mut record, 8, handle);
    write_test_le_u32(&mut record, 12, style);
    write_test_le_i32(&mut record, 16, width);
    write_test_le_i32(&mut record, 20, 0);
    record[24] = color.red;
    record[25] = color.green;
    record[26] = color.blue;
    record
}

fn emf_extcreatepen_record(
    handle: u32,
    pen_style: u32,
    width: u32,
    brush_style: u32,
    color: Color,
    optional_payload: &[u8],
) -> Vec<u8> {
    let payload_offset = 52usize;
    let size = (payload_offset + optional_payload.len()).next_multiple_of(4);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, 95);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_u32(&mut record, 8, handle);
    if !optional_payload.is_empty() {
        write_test_le_u32(&mut record, 12, payload_offset as u32);
        write_test_le_u32(&mut record, 16, optional_payload.len() as u32);
        record[payload_offset..payload_offset + optional_payload.len()]
            .copy_from_slice(optional_payload);
    }
    write_test_le_u32(&mut record, 28, pen_style);
    write_test_le_u32(&mut record, 32, width);
    write_test_le_u32(&mut record, 36, brush_style);
    record[40] = color.red;
    record[41] = color.green;
    record[42] = color.blue;
    write_test_le_u32(&mut record, 48, 0);
    record
}

fn emf_create_brush_record(handle: u32, style: u32, color: Color, hatch: u32) -> Vec<u8> {
    let mut record = vec![0; 24];
    write_test_le_u32(&mut record, 0, 39);
    write_test_le_u32(&mut record, 4, 24);
    write_test_le_u32(&mut record, 8, handle);
    write_test_le_u32(&mut record, 12, style);
    record[16] = color.red;
    record[17] = color.green;
    record[18] = color.blue;
    write_test_le_u32(&mut record, 20, hatch);
    record
}

fn emf_create_palette_record(handle: u32, entries: &[(u8, u8, u8)]) -> Vec<u8> {
    let size = 16 + (entries.len() * 4);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, 49);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_u32(&mut record, 8, handle);
    write_test_le_u16(&mut record, 12, 0x0300);
    write_test_le_u16(&mut record, 14, entries.len() as u16);
    for (index, (red, green, blue)) in entries.iter().copied().enumerate() {
        let offset = 16 + (index * 4);
        record[offset] = red;
        record[offset + 1] = green;
        record[offset + 2] = blue;
    }
    record
}

fn emf_set_palette_entries_record(
    handle: u32,
    start_index: u32,
    entries: &[(u8, u8, u8)],
) -> Vec<u8> {
    let size = 20 + (entries.len() * 4);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, 50);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_u32(&mut record, 8, handle);
    write_test_le_u32(&mut record, 12, start_index);
    write_test_le_u32(&mut record, 16, entries.len() as u32);
    for (index, (red, green, blue)) in entries.iter().copied().enumerate() {
        let offset = 20 + (index * 4);
        record[offset] = red;
        record[offset + 1] = green;
        record[offset + 2] = blue;
    }
    record
}

fn emf_fillrgn_record(brush_handle: u32, rects: &[(i32, i32, i32, i32)]) -> Vec<u8> {
    let region_size = 32 + (rects.len() * 16);
    let size = 32 + region_size;
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, 71);
    write_test_le_u32(&mut record, 4, size as u32);
    let (left, top, right, bottom) = rects.first().copied().unwrap_or((0, 0, 0, 0));
    write_test_le_i32(&mut record, 8, left);
    write_test_le_i32(&mut record, 12, top);
    write_test_le_i32(&mut record, 16, right);
    write_test_le_i32(&mut record, 20, bottom);
    write_test_le_u32(&mut record, 24, region_size as u32);
    write_test_le_u32(&mut record, 28, brush_handle);
    write_test_le_u32(&mut record, 32, 32);
    write_test_le_u32(&mut record, 36, 1);
    write_test_le_u32(&mut record, 40, rects.len() as u32);
    write_test_le_u32(&mut record, 44, (rects.len() * 16) as u32);
    write_test_le_i32(&mut record, 48, left);
    write_test_le_i32(&mut record, 52, top);
    write_test_le_i32(&mut record, 56, right);
    write_test_le_i32(&mut record, 60, bottom);
    for (index, (left, top, right, bottom)) in rects.iter().copied().enumerate() {
        let offset = 64 + (index * 16);
        write_test_le_i32(&mut record, offset, left);
        write_test_le_i32(&mut record, offset + 4, top);
        write_test_le_i32(&mut record, offset + 8, right);
        write_test_le_i32(&mut record, offset + 12, bottom);
    }
    record
}

fn emf_framergn_record(
    brush_handle: u32,
    stroke_width: i32,
    stroke_height: i32,
    rects: &[(i32, i32, i32, i32)],
) -> Vec<u8> {
    let region_size = 32 + (rects.len() * 16);
    let size = 40 + region_size;
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, 72);
    write_test_le_u32(&mut record, 4, size as u32);
    let (left, top, right, bottom) = rects.first().copied().unwrap_or((0, 0, 0, 0));
    write_test_le_i32(&mut record, 8, left);
    write_test_le_i32(&mut record, 12, top);
    write_test_le_i32(&mut record, 16, right);
    write_test_le_i32(&mut record, 20, bottom);
    write_test_le_u32(&mut record, 24, region_size as u32);
    write_test_le_u32(&mut record, 28, brush_handle);
    write_test_le_i32(&mut record, 32, stroke_width);
    write_test_le_i32(&mut record, 36, stroke_height);
    write_test_le_u32(&mut record, 40, 32);
    write_test_le_u32(&mut record, 44, 1);
    write_test_le_u32(&mut record, 48, rects.len() as u32);
    write_test_le_u32(&mut record, 52, (rects.len() * 16) as u32);
    write_test_le_i32(&mut record, 56, left);
    write_test_le_i32(&mut record, 60, top);
    write_test_le_i32(&mut record, 64, right);
    write_test_le_i32(&mut record, 68, bottom);
    for (index, (left, top, right, bottom)) in rects.iter().copied().enumerate() {
        let offset = 72 + (index * 16);
        write_test_le_i32(&mut record, offset, left);
        write_test_le_i32(&mut record, offset + 4, top);
        write_test_le_i32(&mut record, offset + 8, right);
        write_test_le_i32(&mut record, offset + 12, bottom);
    }
    record
}

fn emf_extselectcliprgn_record(region_mode: u32, rects: &[(i32, i32, i32, i32)]) -> Vec<u8> {
    let region_size = 32 + (rects.len() * 16);
    let size = 16 + region_size;
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, 75);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_u32(&mut record, 8, region_size as u32);
    write_test_le_u32(&mut record, 12, region_mode);
    let (left, top, right, bottom) = rects.first().copied().unwrap_or((0, 0, 0, 0));
    write_test_le_u32(&mut record, 16, 32);
    write_test_le_u32(&mut record, 20, 1);
    write_test_le_u32(&mut record, 24, rects.len() as u32);
    write_test_le_u32(&mut record, 28, (rects.len() * 16) as u32);
    write_test_le_i32(&mut record, 32, left);
    write_test_le_i32(&mut record, 36, top);
    write_test_le_i32(&mut record, 40, right);
    write_test_le_i32(&mut record, 44, bottom);
    for (index, (left, top, right, bottom)) in rects.iter().copied().enumerate() {
        let offset = 48 + (index * 16);
        write_test_le_i32(&mut record, offset, left);
        write_test_le_i32(&mut record, offset + 4, top);
        write_test_le_i32(&mut record, offset + 8, right);
        write_test_le_i32(&mut record, offset + 12, bottom);
    }
    record
}

fn emf_paintrgn_record(rects: &[(i32, i32, i32, i32)]) -> Vec<u8> {
    emf_region_only_record(74, rects)
}

fn emf_invertrgn_record(rects: &[(i32, i32, i32, i32)]) -> Vec<u8> {
    emf_region_only_record(73, rects)
}

fn emf_region_only_record(record_type: u32, rects: &[(i32, i32, i32, i32)]) -> Vec<u8> {
    let region_size = 32 + (rects.len() * 16);
    let size = 28 + region_size;
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, record_type);
    write_test_le_u32(&mut record, 4, size as u32);
    let (left, top, right, bottom) = rects.first().copied().unwrap_or((0, 0, 0, 0));
    write_test_le_i32(&mut record, 8, left);
    write_test_le_i32(&mut record, 12, top);
    write_test_le_i32(&mut record, 16, right);
    write_test_le_i32(&mut record, 20, bottom);
    write_test_le_u32(&mut record, 24, region_size as u32);
    write_test_le_u32(&mut record, 28, 32);
    write_test_le_u32(&mut record, 32, 1);
    write_test_le_u32(&mut record, 36, rects.len() as u32);
    write_test_le_u32(&mut record, 40, (rects.len() * 16) as u32);
    write_test_le_i32(&mut record, 44, left);
    write_test_le_i32(&mut record, 48, top);
    write_test_le_i32(&mut record, 52, right);
    write_test_le_i32(&mut record, 56, bottom);
    for (index, (left, top, right, bottom)) in rects.iter().copied().enumerate() {
        let offset = 60 + (index * 16);
        write_test_le_i32(&mut record, offset, left);
        write_test_le_i32(&mut record, offset + 4, top);
        write_test_le_i32(&mut record, offset + 8, right);
        write_test_le_i32(&mut record, offset + 12, bottom);
    }
    record
}

fn emf_select_object_record(handle: u32) -> Vec<u8> {
    let mut record = vec![0; 12];
    write_test_le_u32(&mut record, 0, 37);
    write_test_le_u32(&mut record, 4, 12);
    write_test_le_u32(&mut record, 8, handle);
    record
}

fn emf_u32_record(record_type: u32, value: u32) -> Vec<u8> {
    let mut record = vec![0; 12];
    write_test_le_u32(&mut record, 0, record_type);
    write_test_le_u32(&mut record, 4, 12);
    write_test_le_u32(&mut record, 8, value);
    record
}

fn emf_i32_record(record_type: u32, value: i32) -> Vec<u8> {
    let mut record = vec![0; 12];
    write_test_le_u32(&mut record, 0, record_type);
    write_test_le_u32(&mut record, 4, 12);
    write_test_le_i32(&mut record, 8, value);
    record
}

fn emf_f32_record(record_type: u32, value: f32) -> Vec<u8> {
    let mut record = vec![0; 12];
    write_test_le_u32(&mut record, 0, record_type);
    write_test_le_u32(&mut record, 4, 12);
    write_test_le_f32(&mut record, 8, value);
    record
}

fn emf_color_adjustment_record(values: u16, gamma: u16, contrast: i16, brightness: i16) -> Vec<u8> {
    let mut record = vec![0; 32];
    write_test_le_u32(&mut record, 0, 23);
    write_test_le_u32(&mut record, 4, 32);
    write_test_le_u16(&mut record, 8, 24);
    write_test_le_u16(&mut record, 10, values);
    write_test_le_u16(&mut record, 12, 0);
    write_test_le_u16(&mut record, 14, gamma);
    write_test_le_u16(&mut record, 16, gamma);
    write_test_le_u16(&mut record, 18, gamma);
    write_test_le_u16(&mut record, 20, 0);
    write_test_le_u16(&mut record, 22, 10_000);
    write_test_le_i16(&mut record, 24, contrast);
    write_test_le_i16(&mut record, 26, brightness);
    write_test_le_i16(&mut record, 28, 0);
    write_test_le_i16(&mut record, 30, 0);
    record
}

fn emf_i32_pair_record(record_type: u32, first: i32, second: i32) -> Vec<u8> {
    let mut record = vec![0; 16];
    write_test_le_u32(&mut record, 0, record_type);
    write_test_le_u32(&mut record, 4, 16);
    write_test_le_i32(&mut record, 8, first);
    write_test_le_i32(&mut record, 12, second);
    record
}

fn emf_xform_record(record_type: u32, values: [f32; 6]) -> Vec<u8> {
    let mut record = vec![0; 32];
    write_test_le_u32(&mut record, 0, record_type);
    write_test_le_u32(&mut record, 4, 32);
    for (index, value) in values.into_iter().enumerate() {
        write_test_le_f32(&mut record, 8 + index * 4, value);
    }
    record
}

fn emf_modifyworldtransform_record(mode: u32, values: [f32; 6]) -> Vec<u8> {
    let mut record = emf_xform_record(36, values);
    record.resize(36, 0);
    write_test_le_u32(&mut record, 4, 36);
    write_test_le_u32(&mut record, 32, mode);
    record
}

fn emf_exttextoutw_record(
    x: i32,
    y: i32,
    text: &str,
    options: u32,
    bounds: Option<(i32, i32, i32, i32)>,
    glyph_index: bool,
) -> Vec<u8> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let text_bytes = units.len() * 2;
    let size = (76usize + text_bytes).next_multiple_of(4);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, 84);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_i32(&mut record, 8, 0);
    write_test_le_i32(&mut record, 12, 0);
    write_test_le_i32(&mut record, 16, 160);
    write_test_le_i32(&mut record, 20, 80);
    write_test_le_u32(&mut record, 24, 1);
    write_test_le_u32(&mut record, 28, 0x3f80_0000);
    write_test_le_u32(&mut record, 32, 0x3f80_0000);
    write_test_le_i32(&mut record, 36, x);
    write_test_le_i32(&mut record, 40, y);
    write_test_le_u32(&mut record, 44, units.len() as u32);
    write_test_le_u32(&mut record, 48, 76);
    write_test_le_u32(
        &mut record,
        52,
        if glyph_index {
            options | 0x0010
        } else {
            options
        },
    );
    let (left, top, right, bottom) = bounds.unwrap_or((0, 0, 0, 0));
    write_test_le_i32(&mut record, 56, left);
    write_test_le_i32(&mut record, 60, top);
    write_test_le_i32(&mut record, 64, right);
    write_test_le_i32(&mut record, 68, bottom);
    write_test_le_u32(&mut record, 72, 0);
    for (idx, unit) in units.iter().enumerate() {
        let offset = 76 + (idx * 2);
        record[offset..offset + 2].copy_from_slice(&unit.to_le_bytes());
    }
    record
}

fn emf_exttextoutw_dx_record(x: i32, y: i32, text: &str, dx: &[i32]) -> Vec<u8> {
    let units: Vec<u16> = text.encode_utf16().collect();
    assert_eq!(dx.len(), units.len());
    let text_bytes = units.len() * 2;
    let string_offset = 76usize;
    let dx_offset = string_offset + text_bytes;
    let size = (dx_offset + (dx.len() * 4)).next_multiple_of(4);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, 84);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_i32(&mut record, 8, 0);
    write_test_le_i32(&mut record, 12, 0);
    write_test_le_i32(&mut record, 16, 160);
    write_test_le_i32(&mut record, 20, 80);
    write_test_le_u32(&mut record, 24, 1);
    write_test_le_u32(&mut record, 28, 0x3f80_0000);
    write_test_le_u32(&mut record, 32, 0x3f80_0000);
    write_test_le_i32(&mut record, 36, x);
    write_test_le_i32(&mut record, 40, y);
    write_test_le_u32(&mut record, 44, units.len() as u32);
    write_test_le_u32(&mut record, 48, string_offset as u32);
    write_test_le_u32(&mut record, 72, dx_offset as u32);
    for (idx, unit) in units.iter().enumerate() {
        let offset = string_offset + (idx * 2);
        record[offset..offset + 2].copy_from_slice(&unit.to_le_bytes());
    }
    for (idx, value) in dx.iter().enumerate() {
        write_test_le_i32(&mut record, dx_offset + (idx * 4), *value);
    }
    record
}

fn emf_exttextoutw_pdy_record(x: i32, y: i32, text: &str, advances: &[(i32, i32)]) -> Vec<u8> {
    let units: Vec<u16> = text.encode_utf16().collect();
    assert_eq!(advances.len(), units.len());
    let text_bytes = units.len() * 2;
    let string_offset = 76usize;
    let dx_offset = string_offset + text_bytes;
    let size = (dx_offset + (advances.len() * 8)).next_multiple_of(4);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, 84);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_i32(&mut record, 8, 0);
    write_test_le_i32(&mut record, 12, 0);
    write_test_le_i32(&mut record, 16, 160);
    write_test_le_i32(&mut record, 20, 80);
    write_test_le_u32(&mut record, 24, 1);
    write_test_le_u32(&mut record, 28, 0x3f80_0000);
    write_test_le_u32(&mut record, 32, 0x3f80_0000);
    write_test_le_i32(&mut record, 36, x);
    write_test_le_i32(&mut record, 40, y);
    write_test_le_u32(&mut record, 44, units.len() as u32);
    write_test_le_u32(&mut record, 48, string_offset as u32);
    write_test_le_u32(&mut record, 52, 0x2000);
    write_test_le_u32(&mut record, 72, dx_offset as u32);
    for (idx, unit) in units.iter().enumerate() {
        let offset = string_offset + (idx * 2);
        record[offset..offset + 2].copy_from_slice(&unit.to_le_bytes());
    }
    for (idx, (dx, dy)) in advances.iter().enumerate() {
        let offset = dx_offset + (idx * 8);
        write_test_le_i32(&mut record, offset, *dx);
        write_test_le_i32(&mut record, offset + 4, *dy);
    }
    record
}

fn emf_extcreatefontindirectw_record(
    handle: u32,
    height: i32,
    escapement: i32,
    orientation: i32,
    charset: u8,
) -> Vec<u8> {
    let mut record = vec![0; 104];
    write_test_le_u32(&mut record, 0, 82);
    write_test_le_u32(&mut record, 4, 104);
    write_test_le_u32(&mut record, 8, handle);
    write_test_le_i32(&mut record, 12, height);
    write_test_le_i32(&mut record, 20, escapement);
    write_test_le_i32(&mut record, 24, orientation);
    record[35] = charset;
    record
}

fn emf_exttextouta_record(
    x: i32,
    y: i32,
    text: &[u8],
    options: u32,
    bounds: Option<(i32, i32, i32, i32)>,
    glyph_index: bool,
) -> Vec<u8> {
    let size = (76usize + text.len()).next_multiple_of(4);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, 83);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_i32(&mut record, 8, 0);
    write_test_le_i32(&mut record, 12, 0);
    write_test_le_i32(&mut record, 16, 160);
    write_test_le_i32(&mut record, 20, 80);
    write_test_le_u32(&mut record, 24, 1);
    write_test_le_u32(&mut record, 28, 0x3f80_0000);
    write_test_le_u32(&mut record, 32, 0x3f80_0000);
    write_test_le_i32(&mut record, 36, x);
    write_test_le_i32(&mut record, 40, y);
    write_test_le_u32(&mut record, 44, text.len() as u32);
    write_test_le_u32(&mut record, 48, 76);
    write_test_le_u32(
        &mut record,
        52,
        if glyph_index {
            options | 0x0010
        } else {
            options
        },
    );
    let (left, top, right, bottom) = bounds.unwrap_or((0, 0, 0, 0));
    write_test_le_i32(&mut record, 56, left);
    write_test_le_i32(&mut record, 60, top);
    write_test_le_i32(&mut record, 64, right);
    write_test_le_i32(&mut record, 68, bottom);
    write_test_le_u32(&mut record, 72, 0);
    record[76..76 + text.len()].copy_from_slice(text);
    record
}

fn emf_polytextoutw_record(
    items: &[(i32, i32, &str, u32, Option<(i32, i32, i32, i32)>)],
) -> Vec<u8> {
    let encoded: Vec<(i32, i32, Vec<u8>, usize, u32, Option<(i32, i32, i32, i32)>)> = items
        .iter()
        .map(|(x, y, text, options, bounds)| {
            let bytes: Vec<u8> = text
                .encode_utf16()
                .flat_map(|unit| unit.to_le_bytes())
                .collect();
            (
                *x,
                *y,
                bytes,
                text.encode_utf16().count(),
                *options,
                *bounds,
            )
        })
        .collect();
    emf_polytextout_record(97, &encoded)
}

fn emf_polytextouta_record(
    items: &[(i32, i32, &[u8], u32, Option<(i32, i32, i32, i32)>)],
) -> Vec<u8> {
    let encoded: Vec<(i32, i32, Vec<u8>, usize, u32, Option<(i32, i32, i32, i32)>)> = items
        .iter()
        .map(|(x, y, text, options, bounds)| (*x, *y, text.to_vec(), text.len(), *options, *bounds))
        .collect();
    emf_polytextout_record(96, &encoded)
}

fn emf_polytextoutw_dx_record(x: i32, y: i32, text: &str, dx: &[i32]) -> Vec<u8> {
    let bytes: Vec<u8> = text
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect();
    let char_count = text.encode_utf16().count();
    assert_eq!(dx.len(), char_count);
    let text_objects_start = 40usize;
    let string_offset = text_objects_start + 40;
    let dx_offset = string_offset + bytes.len();
    let size = (dx_offset + (dx.len() * 4)).next_multiple_of(4);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, 97);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_i32(&mut record, 8, 0);
    write_test_le_i32(&mut record, 12, 0);
    write_test_le_i32(&mut record, 16, 160);
    write_test_le_i32(&mut record, 20, 80);
    write_test_le_u32(&mut record, 24, 1);
    write_test_le_u32(&mut record, 28, 0x3f80_0000);
    write_test_le_u32(&mut record, 32, 0x3f80_0000);
    write_test_le_u32(&mut record, 36, 1);
    write_test_le_i32(&mut record, text_objects_start, x);
    write_test_le_i32(&mut record, text_objects_start + 4, y);
    write_test_le_u32(&mut record, text_objects_start + 8, char_count as u32);
    write_test_le_u32(&mut record, text_objects_start + 12, string_offset as u32);
    write_test_le_u32(&mut record, text_objects_start + 16, 0);
    write_test_le_u32(&mut record, text_objects_start + 36, dx_offset as u32);
    record[string_offset..string_offset + bytes.len()].copy_from_slice(&bytes);
    for (index, value) in dx.iter().enumerate() {
        write_test_le_i32(&mut record, dx_offset + (index * 4), *value);
    }
    record
}

fn emf_polytextoutw_pdy_record(x: i32, y: i32, text: &str, advances: &[(i32, i32)]) -> Vec<u8> {
    let bytes: Vec<u8> = text
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect();
    let char_count = text.encode_utf16().count();
    assert_eq!(advances.len(), char_count);
    let text_objects_start = 40usize;
    let string_offset = text_objects_start + 40;
    let dx_offset = string_offset + bytes.len();
    let size = (dx_offset + (advances.len() * 8)).next_multiple_of(4);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, 97);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_i32(&mut record, 8, 0);
    write_test_le_i32(&mut record, 12, 0);
    write_test_le_i32(&mut record, 16, 160);
    write_test_le_i32(&mut record, 20, 80);
    write_test_le_u32(&mut record, 24, 1);
    write_test_le_u32(&mut record, 28, 0x3f80_0000);
    write_test_le_u32(&mut record, 32, 0x3f80_0000);
    write_test_le_u32(&mut record, 36, 1);
    write_test_le_i32(&mut record, text_objects_start, x);
    write_test_le_i32(&mut record, text_objects_start + 4, y);
    write_test_le_u32(&mut record, text_objects_start + 8, char_count as u32);
    write_test_le_u32(&mut record, text_objects_start + 12, string_offset as u32);
    write_test_le_u32(&mut record, text_objects_start + 16, 0x2000);
    write_test_le_u32(&mut record, text_objects_start + 36, dx_offset as u32);
    record[string_offset..string_offset + bytes.len()].copy_from_slice(&bytes);
    for (index, (dx, dy)) in advances.iter().enumerate() {
        let offset = dx_offset + (index * 8);
        write_test_le_i32(&mut record, offset, *dx);
        write_test_le_i32(&mut record, offset + 4, *dy);
    }
    record
}

fn emf_polytextouta_dx_record(x: i32, y: i32, text: &[u8], dx: &[i32]) -> Vec<u8> {
    assert_eq!(dx.len(), text.len());
    let text_objects_start = 40usize;
    let string_offset = text_objects_start + 40;
    let dx_offset = string_offset + text.len();
    let size = (dx_offset + (dx.len() * 4)).next_multiple_of(4);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, 96);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_i32(&mut record, 8, 0);
    write_test_le_i32(&mut record, 12, 0);
    write_test_le_i32(&mut record, 16, 160);
    write_test_le_i32(&mut record, 20, 80);
    write_test_le_u32(&mut record, 24, 1);
    write_test_le_u32(&mut record, 28, 0x3f80_0000);
    write_test_le_u32(&mut record, 32, 0x3f80_0000);
    write_test_le_u32(&mut record, 36, 1);
    write_test_le_i32(&mut record, text_objects_start, x);
    write_test_le_i32(&mut record, text_objects_start + 4, y);
    write_test_le_u32(&mut record, text_objects_start + 8, text.len() as u32);
    write_test_le_u32(&mut record, text_objects_start + 12, string_offset as u32);
    write_test_le_u32(&mut record, text_objects_start + 16, 0);
    write_test_le_u32(&mut record, text_objects_start + 36, dx_offset as u32);
    record[string_offset..string_offset + text.len()].copy_from_slice(text);
    for (index, value) in dx.iter().enumerate() {
        write_test_le_i32(&mut record, dx_offset + (index * 4), *value);
    }
    record
}

fn emf_polytextout_record(
    record_type: u32,
    items: &[(i32, i32, Vec<u8>, usize, u32, Option<(i32, i32, i32, i32)>)],
) -> Vec<u8> {
    let text_objects_start = 40usize;
    let strings_start = text_objects_start + (items.len() * 40);
    let text_bytes: usize = items.iter().map(|(_, _, bytes, _, _, _)| bytes.len()).sum();
    let size = (strings_start + text_bytes).next_multiple_of(4);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, record_type);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_i32(&mut record, 8, 0);
    write_test_le_i32(&mut record, 12, 0);
    write_test_le_i32(&mut record, 16, 160);
    write_test_le_i32(&mut record, 20, 80);
    write_test_le_u32(&mut record, 24, 1);
    write_test_le_u32(&mut record, 28, 0x3f80_0000);
    write_test_le_u32(&mut record, 32, 0x3f80_0000);
    write_test_le_u32(&mut record, 36, items.len() as u32);

    let mut string_offset = strings_start;
    for (index, (x, y, bytes, char_count, options, bounds)) in items.iter().enumerate() {
        let object_offset = text_objects_start + (index * 40);
        write_test_le_i32(&mut record, object_offset, *x);
        write_test_le_i32(&mut record, object_offset + 4, *y);
        write_test_le_u32(&mut record, object_offset + 8, *char_count as u32);
        write_test_le_u32(&mut record, object_offset + 12, string_offset as u32);
        write_test_le_u32(&mut record, object_offset + 16, *options);
        let (left, top, right, bottom) = bounds.unwrap_or((0, 0, 0, 0));
        write_test_le_i32(&mut record, object_offset + 20, left);
        write_test_le_i32(&mut record, object_offset + 24, top);
        write_test_le_i32(&mut record, object_offset + 28, right);
        write_test_le_i32(&mut record, object_offset + 32, bottom);
        write_test_le_u32(&mut record, object_offset + 36, 0);
        record[string_offset..string_offset + bytes.len()].copy_from_slice(bytes);
        string_offset += bytes.len();
    }
    record
}

fn emf_poly_record(record_type: u32, points: &[(i32, i32)]) -> Vec<u8> {
    let size = 28 + (points.len() * 8);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, record_type);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_i32(&mut record, 8, 0);
    write_test_le_i32(&mut record, 12, 0);
    write_test_le_i32(&mut record, 16, 160);
    write_test_le_i32(&mut record, 20, 80);
    write_test_le_u32(&mut record, 24, points.len() as u32);
    for (idx, (x, y)) in points.iter().enumerate() {
        let offset = 28 + (idx * 8);
        write_test_le_i32(&mut record, offset, *x);
        write_test_le_i32(&mut record, offset + 4, *y);
    }
    record
}

fn emf_poly16_record(record_type: u32, points: &[(i16, i16)]) -> Vec<u8> {
    let size = 28 + (points.len() * 4);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, record_type);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_i32(&mut record, 8, 0);
    write_test_le_i32(&mut record, 12, 0);
    write_test_le_i32(&mut record, 16, 160);
    write_test_le_i32(&mut record, 20, 80);
    write_test_le_u32(&mut record, 24, points.len() as u32);
    for (idx, (x, y)) in points.iter().enumerate() {
        let offset = 28 + (idx * 4);
        write_test_le_i16(&mut record, offset, *x);
        write_test_le_i16(&mut record, offset + 2, *y);
    }
    record
}

fn emf_poly_poly_record(record_type: u32, polygons: &[Vec<(i32, i32)>]) -> Vec<u8> {
    let counts: Vec<usize> = polygons.iter().map(Vec::len).collect();
    let points: Vec<(i32, i32)> = polygons
        .iter()
        .flat_map(|points| points.iter().copied())
        .collect();
    let counts: Vec<u32> = counts.into_iter().map(|count| count as u32).collect();
    emf_poly_poly_record_with_counts(record_type, &counts, &points)
}

fn emf_poly_poly_record_with_counts(
    record_type: u32,
    counts: &[u32],
    points: &[(i32, i32)],
) -> Vec<u8> {
    let size = 32 + (counts.len() * 4) + (points.len() * 8);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, record_type);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_i32(&mut record, 8, 0);
    write_test_le_i32(&mut record, 12, 0);
    write_test_le_i32(&mut record, 16, 160);
    write_test_le_i32(&mut record, 20, 80);
    write_test_le_u32(&mut record, 24, counts.len() as u32);
    write_test_le_u32(&mut record, 28, points.len() as u32);
    for (idx, count) in counts.iter().enumerate() {
        write_test_le_u32(&mut record, 32 + (idx * 4), *count);
    }
    let points_start = 32 + (counts.len() * 4);
    for (idx, (x, y)) in points.iter().enumerate() {
        let offset = points_start + (idx * 8);
        write_test_le_i32(&mut record, offset, *x);
        write_test_le_i32(&mut record, offset + 4, *y);
    }
    record
}

fn emf_poly_poly16_record(record_type: u32, polygons: &[Vec<(i16, i16)>]) -> Vec<u8> {
    let counts: Vec<usize> = polygons.iter().map(Vec::len).collect();
    let points: Vec<(i16, i16)> = polygons
        .iter()
        .flat_map(|points| points.iter().copied())
        .collect();
    let counts: Vec<u32> = counts.into_iter().map(|count| count as u32).collect();
    emf_poly_poly16_record_with_counts(record_type, &counts, &points)
}

fn emf_poly_poly16_record_with_counts(
    record_type: u32,
    counts: &[u32],
    points: &[(i16, i16)],
) -> Vec<u8> {
    let size = 32 + (counts.len() * 4) + (points.len() * 4);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, record_type);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_i32(&mut record, 8, 0);
    write_test_le_i32(&mut record, 12, 0);
    write_test_le_i32(&mut record, 16, 160);
    write_test_le_i32(&mut record, 20, 80);
    write_test_le_u32(&mut record, 24, counts.len() as u32);
    write_test_le_u32(&mut record, 28, points.len() as u32);
    for (idx, count) in counts.iter().enumerate() {
        write_test_le_u32(&mut record, 32 + (idx * 4), *count);
    }
    let points_start = 32 + (counts.len() * 4);
    for (idx, (x, y)) in points.iter().enumerate() {
        let offset = points_start + (idx * 4);
        write_test_le_i16(&mut record, offset, *x);
        write_test_le_i16(&mut record, offset + 2, *y);
    }
    record
}

fn emf_setpixelv_record(x: i32, y: i32, color: Color) -> Vec<u8> {
    let mut record = vec![0; 20];
    write_test_le_u32(&mut record, 0, 15);
    write_test_le_u32(&mut record, 4, 20);
    write_test_le_i32(&mut record, 8, x);
    write_test_le_i32(&mut record, 12, y);
    record[16] = color.red;
    record[17] = color.green;
    record[18] = color.blue;
    record
}

fn emf_bitblt_record(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    raster_op: u32,
    payload: &[u8],
) -> Vec<u8> {
    let payload_len = payload.len().next_multiple_of(4);
    let size = 48 + payload_len;
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, 76);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_i32(&mut record, 8, x);
    write_test_le_i32(&mut record, 12, y);
    write_test_le_i32(&mut record, 16, x.saturating_add(width));
    write_test_le_i32(&mut record, 20, y.saturating_add(height));
    write_test_le_i32(&mut record, 24, x);
    write_test_le_i32(&mut record, 28, y);
    write_test_le_i32(&mut record, 32, width);
    write_test_le_i32(&mut record, 36, height);
    write_test_le_u32(&mut record, 40, raster_op);
    record[48..48 + payload.len()].copy_from_slice(payload);
    record
}

fn emf_bitblt_dib_record(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    raster_op: u32,
    dib: &[u8],
) -> Vec<u8> {
    let payload_len = dib.len().next_multiple_of(4);
    let size = 100 + payload_len;
    let mut record = emf_bitblt_record(x, y, width, height, raster_op, &[]);
    record.resize(size, 0);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_u32(&mut record, 84, 100);
    write_test_le_u32(&mut record, 88, payload_len as u32);
    write_test_le_u32(&mut record, 92, 100);
    write_test_le_u32(&mut record, 96, payload_len as u32);
    record[100..100 + dib.len()].copy_from_slice(dib);
    record
}

fn emf_stretchblt_record(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    raster_op: u32,
    payload: &[u8],
) -> Vec<u8> {
    let payload_len = payload.len().next_multiple_of(4);
    let size = 108 + payload_len;
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, 77);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_i32(&mut record, 8, x);
    write_test_le_i32(&mut record, 12, y);
    write_test_le_i32(&mut record, 16, x.saturating_add(width));
    write_test_le_i32(&mut record, 20, y.saturating_add(height));
    write_test_le_i32(&mut record, 24, x);
    write_test_le_i32(&mut record, 28, y);
    write_test_le_i32(&mut record, 32, width);
    write_test_le_i32(&mut record, 36, height);
    write_test_le_u32(&mut record, 40, raster_op);
    write_test_le_i32(&mut record, 100, width);
    write_test_le_i32(&mut record, 104, height);
    record[108..108 + payload.len()].copy_from_slice(payload);
    record
}

fn emf_stretchblt_dib_record(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    raster_op: u32,
    dib: &[u8],
) -> Vec<u8> {
    let payload_len = dib.len().next_multiple_of(4);
    let mut record = emf_stretchblt_record(x, y, width, height, raster_op, &[]);
    record.resize(108 + payload_len, 0);
    write_test_le_u32(&mut record, 4, (108 + payload_len) as u32);
    write_test_le_u32(&mut record, 84, 108);
    write_test_le_u32(&mut record, 88, payload_len as u32);
    write_test_le_u32(&mut record, 92, 108);
    write_test_le_u32(&mut record, 96, payload_len as u32);
    if dib.len() >= 12 {
        let source_width = i32::from_le_bytes(dib[4..8].try_into().unwrap());
        let source_height =
            i32::from_le_bytes(dib[8..12].try_into().unwrap()).unsigned_abs() as i32;
        write_test_le_i32(&mut record, 100, source_width);
        write_test_le_i32(&mut record, 104, source_height);
    }
    record[108..108 + dib.len()].copy_from_slice(dib);
    record
}

fn emf_stretchdibits_record(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    raster_op: u32,
    payload: &[u8],
) -> Vec<u8> {
    let payload_len = payload.len().next_multiple_of(4);
    let size = 80 + payload_len;
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, 81);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_i32(&mut record, 8, x);
    write_test_le_i32(&mut record, 12, y);
    write_test_le_i32(&mut record, 16, x.saturating_add(width));
    write_test_le_i32(&mut record, 20, y.saturating_add(height));
    write_test_le_i32(&mut record, 24, x);
    write_test_le_i32(&mut record, 28, y);
    write_test_le_i32(&mut record, 32, 0);
    write_test_le_i32(&mut record, 36, 0);
    write_test_le_i32(&mut record, 40, width);
    write_test_le_i32(&mut record, 44, height);
    write_test_le_u32(&mut record, 48, 80);
    write_test_le_u32(&mut record, 52, payload_len as u32);
    write_test_le_u32(&mut record, 56, 80);
    write_test_le_u32(&mut record, 60, payload_len as u32);
    write_test_le_u32(&mut record, 68, raster_op);
    write_test_le_i32(&mut record, 72, width);
    write_test_le_i32(&mut record, 76, height);
    record[80..80 + payload.len()].copy_from_slice(payload);
    record
}

fn emf_stretchdibits_dib_record(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    raster_op: u32,
    dib: &[u8],
) -> Vec<u8> {
    let mut record = emf_stretchdibits_record(x, y, width, height, raster_op, dib);
    if dib.len() >= 12 {
        let source_width = i32::from_le_bytes(dib[4..8].try_into().unwrap());
        let source_height =
            i32::from_le_bytes(dib[8..12].try_into().unwrap()).unsigned_abs() as i32;
        write_test_le_i32(&mut record, 40, source_width);
        write_test_le_i32(&mut record, 44, source_height);
    }
    record
}

fn emf_setdibitstodevice_dib_record(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    start_scan: u32,
    scan_count: u32,
    dib: &[u8],
) -> Vec<u8> {
    let payload_len = dib.len().next_multiple_of(4);
    let size = 76 + payload_len;
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, 80);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_i32(&mut record, 8, x);
    write_test_le_i32(&mut record, 12, y);
    write_test_le_i32(&mut record, 16, x.saturating_add(width));
    write_test_le_i32(&mut record, 20, y.saturating_add(height));
    write_test_le_i32(&mut record, 24, x);
    write_test_le_i32(&mut record, 28, y);
    write_test_le_i32(&mut record, 32, 0);
    write_test_le_i32(&mut record, 36, 0);
    write_test_le_i32(&mut record, 40, width);
    write_test_le_i32(&mut record, 44, height);
    write_test_le_u32(&mut record, 48, 76);
    write_test_le_u32(&mut record, 52, payload_len as u32);
    write_test_le_u32(&mut record, 56, 76);
    write_test_le_u32(&mut record, 60, payload_len as u32);
    write_test_le_u32(&mut record, 68, start_scan);
    write_test_le_u32(&mut record, 72, scan_count);
    record[76..76 + dib.len()].copy_from_slice(dib);
    record
}

#[allow(clippy::too_many_arguments)]
fn emf_alphablend_dib_record(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    source_constant_alpha: u8,
    alpha_format: u8,
    dib: &[u8],
) -> Vec<u8> {
    let payload_len = dib.len().next_multiple_of(4);
    let size = 108 + payload_len;
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, 114);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_i32(&mut record, 8, x);
    write_test_le_i32(&mut record, 12, y);
    write_test_le_i32(&mut record, 16, x.saturating_add(width));
    write_test_le_i32(&mut record, 20, y.saturating_add(height));
    write_test_le_i32(&mut record, 24, x);
    write_test_le_i32(&mut record, 28, y);
    write_test_le_i32(&mut record, 32, width);
    write_test_le_i32(&mut record, 36, height);
    record[40] = 0;
    record[41] = 0;
    record[42] = source_constant_alpha;
    record[43] = alpha_format;
    write_test_le_u32(&mut record, 80, 0);
    write_test_le_u32(&mut record, 84, 108);
    write_test_le_u32(&mut record, 88, payload_len as u32);
    write_test_le_u32(&mut record, 92, 108);
    write_test_le_u32(&mut record, 96, payload_len as u32);
    if dib.len() >= 12 {
        let source_width = i32::from_le_bytes(dib[4..8].try_into().unwrap());
        let source_height =
            i32::from_le_bytes(dib[8..12].try_into().unwrap()).unsigned_abs() as i32;
        write_test_le_i32(&mut record, 100, source_width);
        write_test_le_i32(&mut record, 104, source_height);
    }
    record[108..108 + dib.len()].copy_from_slice(dib);
    record
}

fn emf_transparentblt_dib_record(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    transparent: Color,
    dib: &[u8],
) -> Vec<u8> {
    let payload_len = dib.len().next_multiple_of(4);
    let size = 108 + payload_len;
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, 116);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_i32(&mut record, 8, x);
    write_test_le_i32(&mut record, 12, y);
    write_test_le_i32(&mut record, 16, x.saturating_add(width));
    write_test_le_i32(&mut record, 20, y.saturating_add(height));
    write_test_le_i32(&mut record, 24, x);
    write_test_le_i32(&mut record, 28, y);
    write_test_le_i32(&mut record, 32, width);
    write_test_le_i32(&mut record, 36, height);
    write_test_le_u32(
        &mut record,
        40,
        u32::from(transparent.red)
            | (u32::from(transparent.green) << 8)
            | (u32::from(transparent.blue) << 16),
    );
    write_test_le_u32(&mut record, 80, 0);
    write_test_le_u32(&mut record, 84, 108);
    write_test_le_u32(&mut record, 88, payload_len as u32);
    write_test_le_u32(&mut record, 92, 108);
    write_test_le_u32(&mut record, 96, payload_len as u32);
    if dib.len() >= 12 {
        let source_width = i32::from_le_bytes(dib[4..8].try_into().unwrap());
        let source_height =
            i32::from_le_bytes(dib[8..12].try_into().unwrap()).unsigned_abs() as i32;
        write_test_le_i32(&mut record, 100, source_width);
        write_test_le_i32(&mut record, 104, source_height);
    }
    record[108..108 + dib.len()].copy_from_slice(dib);
    record
}

fn emf_point_record(record_type: u32, x: i32, y: i32) -> Vec<u8> {
    let mut record = vec![0; 16];
    write_test_le_u32(&mut record, 0, record_type);
    write_test_le_u32(&mut record, 4, 16);
    write_test_le_i32(&mut record, 8, x);
    write_test_le_i32(&mut record, 12, y);
    record
}

fn emf_size_record(record_type: u32, width: i32, height: i32) -> Vec<u8> {
    emf_point_record(record_type, width, height)
}

fn emf_scale_record(
    record_type: u32,
    x_num: i32,
    x_denom: i32,
    y_num: i32,
    y_denom: i32,
) -> Vec<u8> {
    let mut record = vec![0; 24];
    write_test_le_u32(&mut record, 0, record_type);
    write_test_le_u32(&mut record, 4, 24);
    write_test_le_i32(&mut record, 8, x_num);
    write_test_le_i32(&mut record, 12, x_denom);
    write_test_le_i32(&mut record, 16, y_num);
    write_test_le_i32(&mut record, 20, y_denom);
    record
}

fn emf_arc_record(
    record_type: u32,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
) -> Vec<u8> {
    let mut record = vec![0; 40];
    write_test_le_u32(&mut record, 0, record_type);
    write_test_le_u32(&mut record, 4, 40);
    write_test_le_i32(&mut record, 8, left);
    write_test_le_i32(&mut record, 12, top);
    write_test_le_i32(&mut record, 16, right);
    write_test_le_i32(&mut record, 20, bottom);
    write_test_le_i32(&mut record, 24, start_x);
    write_test_le_i32(&mut record, 28, start_y);
    write_test_le_i32(&mut record, 32, end_x);
    write_test_le_i32(&mut record, 36, end_y);
    record
}

fn emf_polydraw_record(record_type: u32, points: &[(i32, i32)], types: &[u8]) -> Vec<u8> {
    assert_eq!(points.len(), types.len());
    let size = (28 + (points.len() * 8) + types.len()).next_multiple_of(4);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, record_type);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_i32(&mut record, 8, 0);
    write_test_le_i32(&mut record, 12, 0);
    write_test_le_i32(&mut record, 16, 160);
    write_test_le_i32(&mut record, 20, 80);
    write_test_le_u32(&mut record, 24, points.len() as u32);
    let points_start = 28;
    for (idx, (x, y)) in points.iter().enumerate() {
        let offset = points_start + (idx * 8);
        write_test_le_i32(&mut record, offset, *x);
        write_test_le_i32(&mut record, offset + 4, *y);
    }
    let types_start = points_start + (points.len() * 8);
    record[types_start..types_start + types.len()].copy_from_slice(types);
    record
}

fn emf_polydraw16_record(record_type: u32, points: &[(i16, i16)], types: &[u8]) -> Vec<u8> {
    assert_eq!(points.len(), types.len());
    let size = (28 + (points.len() * 4) + types.len()).next_multiple_of(4);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, record_type);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_i32(&mut record, 8, 0);
    write_test_le_i32(&mut record, 12, 0);
    write_test_le_i32(&mut record, 16, 160);
    write_test_le_i32(&mut record, 20, 80);
    write_test_le_u32(&mut record, 24, points.len() as u32);
    let points_start = 28;
    for (idx, (x, y)) in points.iter().enumerate() {
        let offset = points_start + (idx * 4);
        write_test_le_i16(&mut record, offset, *x);
        write_test_le_i16(&mut record, offset + 2, *y);
    }
    let types_start = points_start + (points.len() * 4);
    record[types_start..types_start + types.len()].copy_from_slice(types);
    record
}

fn emf_anglearc_record(
    center_x: i32,
    center_y: i32,
    radius: u32,
    start_angle: f32,
    sweep_angle: f32,
) -> Vec<u8> {
    let mut record = vec![0; 28];
    write_test_le_u32(&mut record, 0, 41);
    write_test_le_u32(&mut record, 4, 28);
    write_test_le_i32(&mut record, 8, center_x);
    write_test_le_i32(&mut record, 12, center_y);
    write_test_le_u32(&mut record, 16, radius);
    write_test_le_f32(&mut record, 20, start_angle);
    write_test_le_f32(&mut record, 24, sweep_angle);
    record
}

fn emf_comment_record(payload: &[u8]) -> Vec<u8> {
    let size = (12usize + payload.len()).next_multiple_of(4);
    let mut record = vec![0; size];
    write_test_le_u32(&mut record, 0, 70);
    write_test_le_u32(&mut record, 4, size as u32);
    write_test_le_u32(&mut record, 8, payload.len() as u32);
    record[12..12 + payload.len()].copy_from_slice(payload);
    record
}

fn emf_unknown_record(record_type: u32) -> Vec<u8> {
    let mut record = vec![0; 8];
    write_test_le_u32(&mut record, 0, record_type);
    write_test_le_u32(&mut record, 4, 8);
    record
}

fn emf_eof_record() -> Vec<u8> {
    let mut record = vec![0; 20];
    write_test_le_u32(&mut record, 0, 14);
    write_test_le_u32(&mut record, 4, 20);
    record
}

fn write_test_le_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_test_le_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_test_le_i32(bytes: &mut [u8], offset: usize, value: i32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_test_le_i16(bytes: &mut [u8], offset: usize, value: i16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_test_le_f32(bytes: &mut [u8], offset: usize, value: f32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn minimal_grayscale_png_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 0, 0, 0, 0]);
    push_png_chunk(&mut png, b"IHDR", &ihdr);

    let idat = [
        0x78, 0x01, 0x01, 0x02, 0x00, 0xfd, 0xff, 0x00, 0x80, 0x00, 0x81, 0x00, 0x81,
    ];
    push_png_chunk(&mut png, b"IDAT", &idat);
    push_png_chunk(&mut png, b"IEND", &[]);
    png
}

fn minimal_grayscale_png(samples: &[u8]) -> Vec<u8> {
    let width = u32::try_from(samples.len()).expect("test PNG width fits u32");
    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&1u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 0, 0, 0, 0]);
    push_png_chunk(&mut png, b"IHDR", &ihdr);

    let mut scanline = Vec::with_capacity(1 + samples.len());
    scanline.push(0);
    scanline.extend_from_slice(samples);
    let idat = miniz_oxide::deflate::compress_to_vec_zlib(&scanline, 6);
    push_png_chunk(&mut png, b"IDAT", &idat);
    push_png_chunk(&mut png, b"IEND", &[]);
    png
}

fn minimal_indexed_png_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 3, 0, 0, 0]);
    push_png_chunk(&mut png, b"IHDR", &ihdr);
    push_png_chunk(&mut png, b"PLTE", &[255, 0, 0, 0, 255, 0]);

    let idat = [
        0x78, 0x01, 0x01, 0x02, 0x00, 0xfd, 0xff, 0x00, 0x01, 0x00, 0x02, 0x00, 0x02,
    ];
    push_png_chunk(&mut png, b"IDAT", &idat);
    push_png_chunk(&mut png, b"IEND", &[]);
    png
}

fn minimal_indexed_png_with_samples(samples: &[u8]) -> Vec<u8> {
    let width = u32::try_from(samples.len()).expect("test PNG width fits u32");
    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&1u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 3, 0, 0, 0]);
    push_png_chunk(&mut png, b"IHDR", &ihdr);
    push_png_chunk(&mut png, b"PLTE", &[255, 0, 0, 0, 255, 0]);

    let mut scanline = Vec::with_capacity(1 + samples.len());
    scanline.push(0);
    scanline.extend_from_slice(samples);
    let idat = miniz_oxide::deflate::compress_to_vec_zlib(&scanline, 6);
    push_png_chunk(&mut png, b"IDAT", &idat);
    push_png_chunk(&mut png, b"IEND", &[]);
    png
}

fn minimal_rgb_png(pixels: &[[u8; 3]]) -> Vec<u8> {
    let width = u32::try_from(pixels.len()).expect("test PNG width fits u32");
    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&1u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    push_png_chunk(&mut png, b"IHDR", &ihdr);

    let mut scanline = Vec::with_capacity(1 + pixels.len() * 3);
    scanline.push(0);
    for pixel in pixels {
        scanline.extend_from_slice(pixel);
    }
    let idat = miniz_oxide::deflate::compress_to_vec_zlib(&scanline, 6);
    push_png_chunk(&mut png, b"IDAT", &idat);
    push_png_chunk(&mut png, b"IEND", &[]);
    png
}

fn minimal_rgb_png_with_rows(width: u32, height: u32, pixels: &[[u8; 3]]) -> Vec<u8> {
    assert_eq!(pixels.len(), (width as usize) * (height as usize));
    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    push_png_chunk(&mut png, b"IHDR", &ihdr);

    let mut scanlines = Vec::with_capacity((height as usize) * (1 + (width as usize * 3)));
    for row in pixels.chunks(width as usize) {
        scanlines.push(0);
        for pixel in row {
            scanlines.extend_from_slice(pixel);
        }
    }
    let idat = miniz_oxide::deflate::compress_to_vec_zlib(&scanlines, 6);
    push_png_chunk(&mut png, b"IDAT", &idat);
    push_png_chunk(&mut png, b"IEND", &[]);
    png
}

fn minimal_rgba_png(pixels: &[[u8; 4]]) -> Vec<u8> {
    let width = u32::try_from(pixels.len()).expect("test PNG width fits u32");
    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&1u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    push_png_chunk(&mut png, b"IHDR", &ihdr);

    let mut scanline = Vec::with_capacity(1 + pixels.len() * 4);
    scanline.push(0);
    for pixel in pixels {
        scanline.extend_from_slice(pixel);
    }
    let idat = miniz_oxide::deflate::compress_to_vec_zlib(&scanline, 6);
    push_png_chunk(&mut png, b"IDAT", &idat);
    push_png_chunk(&mut png, b"IEND", &[]);
    png
}

fn push_png_chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(kind);
    png.extend_from_slice(data);
    png.extend_from_slice(&0u32.to_be_bytes());
}

fn minimal_24bit_dib_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    let pixels: Vec<[u8; 3]> = (0..height)
        .flat_map(|_| (0..width).map(|x| if x % 2 == 0 { [255, 0, 0] } else { [0, 255, 0] }))
        .collect();
    minimal_24bit_dib_with_rgb_pixels(width, height, &pixels)
}

fn minimal_24bit_dib_with_rgb_pixels(width: u32, height: u32, pixels: &[[u8; 3]]) -> Vec<u8> {
    assert_eq!(pixels.len(), (width as usize) * (height as usize));
    let row_stride = ((width as usize * 3) + 3) / 4 * 4;
    let pixel_bytes = row_stride * height as usize;
    let mut dib = Vec::with_capacity(40 + pixel_bytes);
    dib.extend_from_slice(&40u32.to_le_bytes());
    dib.extend_from_slice(&(width as i32).to_le_bytes());
    dib.extend_from_slice(&(height as i32).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&24u16.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());

    for y in (0..height).rev() {
        let mut row = Vec::with_capacity(row_stride);
        for x in 0..width {
            let [red, green, blue] = pixels[(y as usize * width as usize) + x as usize];
            row.extend_from_slice(&[blue, green, red]);
        }
        row.resize(row_stride, 0);
        dib.extend_from_slice(&row);
    }
    dib
}

fn minimal_32bit_dib_with_rgba_pixels(width: u32, height: u32, pixels: &[[u8; 4]]) -> Vec<u8> {
    assert_eq!(pixels.len(), (width as usize) * (height as usize));
    let row_stride = width as usize * 4;
    let pixel_bytes = row_stride * height as usize;
    let mut dib = Vec::with_capacity(40 + pixel_bytes);
    dib.extend_from_slice(&40u32.to_le_bytes());
    dib.extend_from_slice(&(width as i32).to_le_bytes());
    dib.extend_from_slice(&(height as i32).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&32u16.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());

    for y in (0..height).rev() {
        for x in 0..width {
            let [red, green, blue, alpha] = pixels[(y as usize * width as usize) + x as usize];
            dib.extend_from_slice(&[blue, green, red, alpha]);
        }
    }
    dib
}

fn minimal_32bit_bitfields_dib_with_rgba_pixels(
    width: u32,
    height: u32,
    pixels: &[[u8; 4]],
) -> Vec<u8> {
    assert_eq!(pixels.len(), (width as usize) * (height as usize));
    let row_stride = width as usize * 4;
    let pixel_bytes = row_stride * height as usize;
    let mut dib = Vec::with_capacity(56 + pixel_bytes);
    dib.extend_from_slice(&56u32.to_le_bytes());
    dib.extend_from_slice(&(width as i32).to_le_bytes());
    dib.extend_from_slice(&(height as i32).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&32u16.to_le_bytes());
    dib.extend_from_slice(&3u32.to_le_bytes());
    dib.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&0x00ff_0000u32.to_le_bytes());
    dib.extend_from_slice(&0x0000_ff00u32.to_le_bytes());
    dib.extend_from_slice(&0x0000_00ffu32.to_le_bytes());
    dib.extend_from_slice(&0xff00_0000u32.to_le_bytes());

    for y in (0..height).rev() {
        for x in 0..width {
            let [red, green, blue, alpha] = pixels[(y as usize * width as usize) + x as usize];
            let pixel = (u32::from(alpha) << 24)
                | (u32::from(red) << 16)
                | (u32::from(green) << 8)
                | u32::from(blue);
            dib.extend_from_slice(&pixel.to_le_bytes());
        }
    }
    dib
}

fn minimal_32bit_alpha_bitfields_dib_with_rgba_pixels(
    width: u32,
    height: u32,
    pixels: &[[u8; 4]],
) -> Vec<u8> {
    assert_eq!(pixels.len(), (width as usize) * (height as usize));
    let row_stride = width as usize * 4;
    let pixel_bytes = row_stride * height as usize;
    let mut dib = Vec::with_capacity(56 + pixel_bytes);
    dib.extend_from_slice(&40u32.to_le_bytes());
    dib.extend_from_slice(&(width as i32).to_le_bytes());
    dib.extend_from_slice(&(height as i32).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&32u16.to_le_bytes());
    dib.extend_from_slice(&6u32.to_le_bytes());
    dib.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&0x00ff_0000u32.to_le_bytes());
    dib.extend_from_slice(&0x0000_ff00u32.to_le_bytes());
    dib.extend_from_slice(&0x0000_00ffu32.to_le_bytes());
    dib.extend_from_slice(&0xff00_0000u32.to_le_bytes());

    for y in (0..height).rev() {
        for x in 0..width {
            let [red, green, blue, alpha] = pixels[(y as usize * width as usize) + x as usize];
            let pixel = (u32::from(alpha) << 24)
                | (u32::from(red) << 16)
                | (u32::from(green) << 8)
                | u32::from(blue);
            dib.extend_from_slice(&pixel.to_le_bytes());
        }
    }
    dib
}

fn minimal_compressed_dib_with_payload(
    width: u32,
    height: u32,
    compression: u32,
    payload: &[u8],
) -> Vec<u8> {
    let payload_len = u32::try_from(payload.len()).expect("test payload fits in DIB header");
    let mut dib = Vec::with_capacity(40 + payload.len());
    dib.extend_from_slice(&40u32.to_le_bytes());
    dib.extend_from_slice(&(width as i32).to_le_bytes());
    dib.extend_from_slice(&(height as i32).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&0u16.to_le_bytes());
    dib.extend_from_slice(&compression.to_le_bytes());
    dib.extend_from_slice(&payload_len.to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(payload);
    dib
}

fn minimal_16bit_dib_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    let row_stride = ((width as usize * 16).div_ceil(32)) * 4;
    let pixel_bytes = row_stride * height as usize;
    let mut dib = Vec::with_capacity(40 + pixel_bytes);
    dib.extend_from_slice(&40u32.to_le_bytes());
    dib.extend_from_slice(&(width as i32).to_le_bytes());
    dib.extend_from_slice(&(height as i32).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&16u16.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());

    for _ in 0..height {
        let mut row = Vec::with_capacity(row_stride);
        for x in 0..width {
            let pixel = if x % 2 == 0 { 0x7c00u16 } else { 0x03e0u16 };
            row.extend_from_slice(&pixel.to_le_bytes());
        }
        row.resize(row_stride, 0);
        dib.extend_from_slice(&row);
    }
    dib
}

fn minimal_16bit_bitfields_dib_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    let row_stride = ((width as usize * 16).div_ceil(32)) * 4;
    let pixel_bytes = row_stride * height as usize;
    let mut dib = Vec::with_capacity(52 + pixel_bytes);
    dib.extend_from_slice(&40u32.to_le_bytes());
    dib.extend_from_slice(&(width as i32).to_le_bytes());
    dib.extend_from_slice(&(height as i32).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&16u16.to_le_bytes());
    dib.extend_from_slice(&3u32.to_le_bytes());
    dib.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&0xf800u32.to_le_bytes());
    dib.extend_from_slice(&0x07e0u32.to_le_bytes());
    dib.extend_from_slice(&0x001fu32.to_le_bytes());

    for _ in 0..height {
        let mut row = Vec::with_capacity(row_stride);
        for x in 0..width {
            let pixel = if x % 2 == 0 { 0xf800u16 } else { 0x07e0u16 };
            row.extend_from_slice(&pixel.to_le_bytes());
        }
        row.resize(row_stride, 0);
        dib.extend_from_slice(&row);
    }
    dib
}

fn minimal_16bit_alpha_bitfields_dib_with_rgba_pixels(
    width: u32,
    height: u32,
    pixels: &[[u8; 4]],
) -> Vec<u8> {
    assert_eq!(pixels.len(), (width as usize) * (height as usize));
    let row_stride = ((width as usize * 16).div_ceil(32)) * 4;
    let pixel_bytes = row_stride * height as usize;
    let mut dib = Vec::with_capacity(56 + pixel_bytes);
    dib.extend_from_slice(&40u32.to_le_bytes());
    dib.extend_from_slice(&(width as i32).to_le_bytes());
    dib.extend_from_slice(&(height as i32).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&16u16.to_le_bytes());
    dib.extend_from_slice(&6u32.to_le_bytes());
    dib.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&0x7c00u32.to_le_bytes());
    dib.extend_from_slice(&0x03e0u32.to_le_bytes());
    dib.extend_from_slice(&0x001fu32.to_le_bytes());
    dib.extend_from_slice(&0x8000u32.to_le_bytes());

    for y in (0..height).rev() {
        let mut row = Vec::with_capacity(row_stride);
        for x in 0..width {
            let [red, green, blue, alpha] = pixels[(y as usize * width as usize) + x as usize];
            let pixel = (if alpha >= 128 { 0x8000u16 } else { 0 })
                | (u16::from(red >> 3) << 10)
                | (u16::from(green >> 3) << 5)
                | u16::from(blue >> 3);
            row.extend_from_slice(&pixel.to_le_bytes());
        }
        row.resize(row_stride, 0);
        dib.extend_from_slice(&row);
    }
    dib
}

fn minimal_8bit_dib_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    minimal_indexed_dib_with_dimensions(width, height, 8)
}

fn minimal_rle8_dib_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    assert_eq!(width, 3);
    assert_eq!(height, 1);
    let compressed = [0, 3, 0, 1, 0, 0, 0, 1];
    let mut dib = Vec::with_capacity(40 + 8 + compressed.len());
    dib.extend_from_slice(&40u32.to_le_bytes());
    dib.extend_from_slice(&(width as i32).to_le_bytes());
    dib.extend_from_slice(&(height as i32).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&8u16.to_le_bytes());
    dib.extend_from_slice(&1u32.to_le_bytes());
    dib.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&2u32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&[0, 0, 255, 0]);
    dib.extend_from_slice(&[0, 255, 0, 0]);
    dib.extend_from_slice(&compressed);
    dib
}

fn minimal_rle4_dib_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    assert_eq!(width, 3);
    assert_eq!(height, 1);
    let compressed = [0, 3, 0x01, 0, 0, 1];
    let mut dib = Vec::with_capacity(40 + 8 + compressed.len());
    dib.extend_from_slice(&40u32.to_le_bytes());
    dib.extend_from_slice(&(width as i32).to_le_bytes());
    dib.extend_from_slice(&(height as i32).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&4u16.to_le_bytes());
    dib.extend_from_slice(&2u32.to_le_bytes());
    dib.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&2u32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&[0, 0, 255, 0]);
    dib.extend_from_slice(&[0, 255, 0, 0]);
    dib.extend_from_slice(&compressed);
    dib
}

fn minimal_4bit_dib_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    minimal_indexed_dib_with_dimensions(width, height, 4)
}

fn minimal_4bit_bitmap_core_dib_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    let row_stride = ((width as usize * 4).div_ceil(32)) * 4;
    let pixel_bytes = row_stride * height as usize;
    let palette_entries = 16u32;
    let mut dib = Vec::with_capacity(12 + (palette_entries as usize * 3) + pixel_bytes);
    dib.extend_from_slice(&12u32.to_le_bytes());
    dib.extend_from_slice(&(width as u16).to_le_bytes());
    dib.extend_from_slice(&(height as u16).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&4u16.to_le_bytes());
    dib.extend_from_slice(&[0, 0, 255]);
    dib.extend_from_slice(&[0, 255, 0]);
    for _ in 2..palette_entries {
        dib.extend_from_slice(&[0, 0, 0]);
    }

    for _ in 0..height {
        let mut row = indexed_dib_test_row(width, 4);
        row.resize(row_stride, 0);
        dib.extend_from_slice(&row);
    }
    dib
}

fn minimal_indexed_dib_with_dimensions(width: u32, height: u32, bits_per_pixel: u16) -> Vec<u8> {
    let row_stride = ((width as usize * usize::from(bits_per_pixel)).div_ceil(32)) * 4;
    let pixel_bytes = row_stride * height as usize;
    let palette_entries = 2u32;
    let mut dib = Vec::with_capacity(40 + (palette_entries as usize * 4) + pixel_bytes);
    dib.extend_from_slice(&40u32.to_le_bytes());
    dib.extend_from_slice(&(width as i32).to_le_bytes());
    dib.extend_from_slice(&(height as i32).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&bits_per_pixel.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&0i32.to_le_bytes());
    dib.extend_from_slice(&palette_entries.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&[0, 0, 255, 0]);
    dib.extend_from_slice(&[0, 255, 0, 0]);

    for _ in 0..height {
        let mut row = indexed_dib_test_row(width, bits_per_pixel);
        row.resize(row_stride, 0);
        dib.extend_from_slice(&row);
    }
    dib
}

fn indexed_dib_test_row(width: u32, bits_per_pixel: u16) -> Vec<u8> {
    match bits_per_pixel {
        4 => {
            let mut row = Vec::new();
            let mut high = None;
            for x in 0..width {
                let index = (x % 2) as u8;
                if let Some(high_index) = high.take() {
                    row.push((high_index << 4) | index);
                } else {
                    high = Some(index);
                }
            }
            if let Some(high_index) = high {
                row.push(high_index << 4);
            }
            row
        }
        8 => (0..width).map(|x| (x % 2) as u8).collect(),
        _ => Vec::new(),
    }
}

fn collect_text(document: &open_rtf_converter::model::Document) -> String {
    let mut text = String::new();
    for paragraph in &document.header {
        append_paragraph_text(&mut text, paragraph);
    }
    for paragraph in &document.first_page_header {
        append_paragraph_text(&mut text, paragraph);
    }
    for paragraph in &document.even_page_header {
        append_paragraph_text(&mut text, paragraph);
    }
    for paragraph in &document.footer {
        append_paragraph_text(&mut text, paragraph);
    }
    for paragraph in &document.first_page_footer {
        append_paragraph_text(&mut text, paragraph);
    }
    for paragraph in &document.even_page_footer {
        append_paragraph_text(&mut text, paragraph);
    }
    for paragraph in &document.footnotes {
        append_paragraph_text(&mut text, paragraph);
    }
    for paragraph in &document.endnotes {
        append_paragraph_text(&mut text, paragraph);
    }
    for block in &document.blocks {
        match block {
            open_rtf_converter::model::Block::Paragraph(paragraph) => {
                append_paragraph_text(&mut text, paragraph);
            }
            open_rtf_converter::model::Block::Table(table) => {
                for row in &table.rows {
                    for cell in &row.cells {
                        for paragraph in &cell.paragraphs {
                            append_paragraph_text(&mut text, paragraph);
                        }
                    }
                }
            }
            open_rtf_converter::model::Block::Placeholder(value) => text.push_str(value),
            open_rtf_converter::model::Block::Shape(shape) => {
                for paragraph in &shape.text {
                    append_paragraph_text(&mut text, paragraph);
                }
            }
            open_rtf_converter::model::Block::Image(_) => {}
            open_rtf_converter::model::Block::SectionSettings(settings) => {
                for paragraph in &settings.header {
                    append_paragraph_text(&mut text, paragraph);
                }
                for paragraph in &settings.first_page_header {
                    append_paragraph_text(&mut text, paragraph);
                }
                for paragraph in &settings.even_page_header {
                    append_paragraph_text(&mut text, paragraph);
                }
                for paragraph in &settings.footer {
                    append_paragraph_text(&mut text, paragraph);
                }
                for paragraph in &settings.first_page_footer {
                    append_paragraph_text(&mut text, paragraph);
                }
                for paragraph in &settings.even_page_footer {
                    append_paragraph_text(&mut text, paragraph);
                }
            }
            open_rtf_converter::model::Block::PageBreak
            | open_rtf_converter::model::Block::ColumnBreak
            | open_rtf_converter::model::Block::ContinuousSectionBreak
            | open_rtf_converter::model::Block::SectionBreak
            | open_rtf_converter::model::Block::EvenPageSectionBreak
            | open_rtf_converter::model::Block::OddPageSectionBreak => {}
        }
    }
    text
}

fn assert_passive_checkbox_vectors_without_zapf(
    pdf: &[u8],
    content: &lopdf::content::Content,
    context: &str,
) {
    assert!(
        !pdf.windows(b"/BaseFont /ZapfDingbats".len())
            .any(|window| window == b"/BaseFont /ZapfDingbats"),
        "{context} should not require a viewer ZapfDingbats font"
    );
    assert!(
        pdf_text_bytes_for_font(content, b"F14").is_empty(),
        "{context} should not emit checkbox glyphs as ZapfDingbats text bytes"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "{context} should render checkbox boxes as passive vector rectangles"
    );
    assert!(
        content
            .operations
            .iter()
            .any(|operation| matches!(operation.operator.as_str(), "S" | "f" | "f*")),
        "{context} should render checkbox marks as passive vector paths"
    );
}

fn run_style_for_text<'a>(
    document: &'a open_rtf_converter::model::Document,
    exact_text: &str,
) -> Option<&'a CharacterStyle> {
    for paragraph in &document.header {
        if let Some(style) = paragraph_run_style_for_text(paragraph, exact_text) {
            return Some(style);
        }
    }
    for paragraph in &document.first_page_header {
        if let Some(style) = paragraph_run_style_for_text(paragraph, exact_text) {
            return Some(style);
        }
    }
    for paragraph in &document.even_page_header {
        if let Some(style) = paragraph_run_style_for_text(paragraph, exact_text) {
            return Some(style);
        }
    }
    for paragraph in &document.footer {
        if let Some(style) = paragraph_run_style_for_text(paragraph, exact_text) {
            return Some(style);
        }
    }
    for paragraph in &document.first_page_footer {
        if let Some(style) = paragraph_run_style_for_text(paragraph, exact_text) {
            return Some(style);
        }
    }
    for paragraph in &document.even_page_footer {
        if let Some(style) = paragraph_run_style_for_text(paragraph, exact_text) {
            return Some(style);
        }
    }
    for paragraph in &document.footnotes {
        if let Some(style) = paragraph_run_style_for_text(paragraph, exact_text) {
            return Some(style);
        }
    }
    for paragraph in &document.endnotes {
        if let Some(style) = paragraph_run_style_for_text(paragraph, exact_text) {
            return Some(style);
        }
    }
    for block in &document.blocks {
        if let Some(style) = block_run_style_for_text(block, exact_text) {
            return Some(style);
        }
    }
    None
}

fn block_run_style_for_text<'a>(block: &'a Block, exact_text: &str) -> Option<&'a CharacterStyle> {
    match block {
        Block::Paragraph(paragraph) => paragraph_run_style_for_text(paragraph, exact_text),
        Block::Table(table) => {
            for row in &table.rows {
                for cell in &row.cells {
                    for paragraph in &cell.paragraphs {
                        if let Some(style) = paragraph_run_style_for_text(paragraph, exact_text) {
                            return Some(style);
                        }
                    }
                }
            }
            None
        }
        Block::Shape(shape) => {
            for paragraph in &shape.text {
                if let Some(style) = paragraph_run_style_for_text(paragraph, exact_text) {
                    return Some(style);
                }
            }
            None
        }
        Block::SectionSettings(settings) => {
            for paragraph in settings
                .header
                .iter()
                .chain(&settings.first_page_header)
                .chain(&settings.even_page_header)
                .chain(&settings.footer)
                .chain(&settings.first_page_footer)
                .chain(&settings.even_page_footer)
            {
                if let Some(style) = paragraph_run_style_for_text(paragraph, exact_text) {
                    return Some(style);
                }
            }
            None
        }
        Block::Placeholder(_)
        | Block::Image(_)
        | Block::PageBreak
        | Block::ColumnBreak
        | Block::ContinuousSectionBreak
        | Block::SectionBreak
        | Block::EvenPageSectionBreak
        | Block::OddPageSectionBreak => None,
    }
}

fn paragraph_run_style_for_text<'a>(
    paragraph: &'a open_rtf_converter::model::Paragraph,
    exact_text: &str,
) -> Option<&'a CharacterStyle> {
    paragraph.runs.iter().find_map(|run| {
        let matches = match run.dynamic_field {
            Some(InlineKind::LayoutAdvance(_)) => exact_text == PASSIVE_ADVANCE_MARKER,
            Some(InlineKind::InlineImage(id)) => {
                inline_image_marker(id.get()).is_some_and(|marker| exact_text == marker.to_string())
            }
            Some(InlineKind::FootnoteReference(id)) => {
                exact_text == footnote_reference_marker(id.get())
            }
            Some(InlineKind::EndnoteReference(id)) => {
                exact_text == endnote_reference_marker(id.get())
            }
            _ => run.text == exact_text,
        };
        matches.then_some(&run.style)
    })
}

fn append_paragraph_text(text: &mut String, paragraph: &open_rtf_converter::model::Paragraph) {
    for run in &paragraph.runs {
        match run.dynamic_field {
            Some(InlineKind::DynamicField(field)) => text.push_str(match field {
                DynamicField::PageNumber => PAGE_NUMBER_MARKER,
                DynamicField::TotalPages => TOTAL_PAGES_MARKER,
                DynamicField::SectionNumber => SECTION_NUMBER_MARKER,
                DynamicField::SectionPages => SECTION_PAGES_MARKER,
                DynamicField::DocumentWords => DOCUMENT_WORDS_MARKER,
                DynamicField::DocumentCharacters => DOCUMENT_CHARS_MARKER,
                DynamicField::DocumentCharactersWithSpaces => DOCUMENT_CHARS_WITH_SPACES_MARKER,
            }),
            Some(InlineKind::InlineImage(id)) => {
                if let Some(marker) = inline_image_marker(id.get()) {
                    text.push(marker);
                }
            }
            Some(InlineKind::PendingNoteReference) => text.push_str(PENDING_NOTE_REFERENCE_MARKER),
            Some(InlineKind::FootnoteReference(id)) => {
                text.push_str(&footnote_reference_marker(id.get()))
            }
            Some(InlineKind::EndnoteReference(id)) => {
                text.push_str(&endnote_reference_marker(id.get()))
            }
            Some(InlineKind::BookmarkPageAnchor(id)) => text.push_str(&format!(
                "{BOOKMARK_PAGE_ANCHOR_MARKER}{}{BOOKMARK_PAGE_MARKER_END}",
                id.get()
            )),
            Some(InlineKind::BookmarkPageReference(id)) => text.push_str(&format!(
                "{BOOKMARK_PAGE_REF_MARKER}{}{BOOKMARK_PAGE_MARKER_END}",
                id.get()
            )),
            Some(InlineKind::LayoutAdvance(_)) => text.push_str(PASSIVE_ADVANCE_MARKER),
            Some(InlineKind::Math(kind)) => text.push_str(match kind {
                MathInline::FractionRule => PASSIVE_MATH_FRACTION_RULE_MARKER,
                MathInline::StackAnchor => PASSIVE_MATH_STACK_ANCHOR_MARKER,
                MathInline::InlineLead => PASSIVE_MATH_INLINE_LEAD_MARKER,
            }),
            Some(InlineKind::NestedTableAnchor) => text.push_str(NESTED_TABLE_ANCHOR_MARKER),
            None => text.push_str(&run.text),
        }
    }
}

fn strip_bookmark_page_markers(text: &str) -> String {
    let mut stripped = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(marker_start) = rest
        .find(BOOKMARK_PAGE_ANCHOR_MARKER)
        .into_iter()
        .chain(rest.find(BOOKMARK_PAGE_REF_MARKER))
        .min()
    {
        stripped.push_str(&rest[..marker_start]);
        let marker_tail = &rest[marker_start..];
        let Some(marker_end) = marker_tail.find(BOOKMARK_PAGE_MARKER_END) else {
            rest = &rest[marker_start + BOOKMARK_PAGE_ANCHOR_MARKER.len()..];
            continue;
        };
        rest = &marker_tail[marker_end + BOOKMARK_PAGE_MARKER_END.len()..];
    }
    stripped.push_str(rest);
    stripped
}

fn decoded_pdf_text(content: &lopdf::content::Content) -> String {
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

fn pdf_first_text_position_for_text(
    content: &lopdf::content::Content,
    needle: &str,
) -> Option<(f32, f32)> {
    let mut position = None;
    for operation in &content.operations {
        if operation.operator == "Td" {
            let x = operation.operands.first().and_then(pdf_operand_number)?;
            let y = operation.operands.get(1).and_then(pdf_operand_number)?;
            position = Some((x, y));
        } else if operation.operator == "Tj" {
            if operation.operands.iter().any(|operand| {
                operand
                    .as_str()
                    .is_ok_and(|bytes| decode_pdf_text_bytes(bytes).contains(needle))
            }) {
                return position;
            }
        } else if operation.operator == "TJ"
            && operation.operands.iter().any(|operand| {
                operand.as_array().is_ok_and(|items| {
                    items.iter().any(|item| {
                        item.as_str()
                            .is_ok_and(|bytes| decode_pdf_text_bytes(bytes).contains(needle))
                    })
                })
            })
        {
            return position;
        }
    }
    None
}

fn pdf_text_font_names(content: &lopdf::content::Content) -> Vec<Vec<u8>> {
    content
        .operations
        .iter()
        .filter(|operation| operation.operator == "Tf")
        .filter_map(|operation| operation.operands.first())
        .filter_map(|operand| operand.as_name().ok().map(|name| name.to_vec()))
        .collect()
}

fn pdf_text_bytes_for_font(content: &lopdf::content::Content, font_name: &[u8]) -> Vec<u8> {
    let mut current_font: Option<Vec<u8>> = None;
    let mut output = Vec::new();
    for operation in &content.operations {
        if operation.operator == "Tf" {
            current_font = operation
                .operands
                .first()
                .and_then(|operand| operand.as_name().ok().map(|name| name.to_vec()));
        } else if operation.operator == "Tj" && current_font.as_deref() == Some(font_name) {
            for operand in &operation.operands {
                if let Ok(bytes) = operand.as_str() {
                    output.extend_from_slice(bytes);
                }
            }
        } else if operation.operator == "TJ" && current_font.as_deref() == Some(font_name) {
            for operand in &operation.operands {
                if let Ok(items) = operand.as_array() {
                    for item in items {
                        if let Ok(bytes) = item.as_str() {
                            output.extend_from_slice(bytes);
                        }
                    }
                }
            }
        }
    }
    output
}

fn pdf_image_transform_before_first_do(
    content: &lopdf::content::Content,
) -> Option<&lopdf::content::Operation> {
    let do_pos = content
        .operations
        .iter()
        .position(|operation| operation.operator == "Do")?;
    content.operations[..do_pos]
        .iter()
        .rfind(|operation| operation.operator == "cm")
}

fn pdf_operand_number(object: &lopdf::Object) -> Option<f32> {
    match object {
        lopdf::Object::Integer(value) => Some(*value as f32),
        lopdf::Object::Real(value) => Some(*value),
        _ => None,
    }
}

fn pdf_page_media_box_size(pdf: &PdfDocument, page_id: lopdf::ObjectId) -> (f32, f32) {
    let page = pdf
        .get_object(page_id)
        .expect("page object")
        .as_dict()
        .expect("page dictionary");
    let media_box = page
        .get(b"MediaBox")
        .expect("page media box")
        .as_array()
        .expect("media box array");
    let left = media_box
        .first()
        .and_then(pdf_operand_number)
        .expect("media box left");
    let bottom = media_box
        .get(1)
        .and_then(pdf_operand_number)
        .expect("media box bottom");
    let right = media_box
        .get(2)
        .and_then(pdf_operand_number)
        .expect("media box right");
    let top = media_box
        .get(3)
        .and_then(pdf_operand_number)
        .expect("media box top");
    (right - left, top - bottom)
}

fn has_wmf_preview_warning(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("WMF picture rendered as bounded passive vector preview")
    })
}

fn assert_no_wmf_preview_warning(diagnostics: &[Diagnostic]) {
    assert!(
        !has_wmf_preview_warning(diagnostics),
        "fully handled WMF should not be reported as a partial passive preview: {diagnostics:?}"
    );
}

// Test functions live in domain modules; shared fixtures and helpers remain
// in this target root so this structural split does not alter assertions.
#[path = "security_tests/active_content.rs"]
mod active_content;
#[path = "security_tests/core_policy.rs"]
mod core_policy;
#[path = "security_tests/emf.rs"]
mod emf;
#[path = "security_tests/fields.rs"]
mod fields;
#[path = "security_tests/fonts_text.rs"]
mod fonts_text;
#[path = "security_tests/limits_malformed.rs"]
mod limits_malformed;
#[path = "security_tests/lists.rs"]
mod lists;
#[path = "security_tests/math.rs"]
mod math;
#[path = "security_tests/notes.rs"]
mod notes;
#[path = "security_tests/page_layout.rs"]
mod page_layout;
#[path = "security_tests/raster_images.rs"]
mod raster_images;
#[path = "security_tests/shapes.rs"]
mod shapes;
#[path = "security_tests/tables.rs"]
mod tables;
#[path = "security_tests/wmf.rs"]
mod wmf;
