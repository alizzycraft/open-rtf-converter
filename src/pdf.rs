use std::error::Error;
use std::fmt;

use pdf_writer::types::{
    CidFontType, FontFlags, Predictor, SystemInfo, TextRenderingMode, UnicodeCmap,
};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str};
use ttf_parser::{Face, name_id};

use crate::fonts::{FontAsset, FontProvider};
use crate::layout::{
    LayoutDocument, LayoutItem, LineStyle, PdfColor, PdfFontFamily, TextFragment,
    passive_pair_kerning_points, style_uses_passive_kerning, twips_to_points,
};
use crate::model::{
    BorderStyle, CharacterEmphasisMark, CharacterStyle, ImageFormat, ShadingPattern,
    StaticImageTextHorizontalAlign, StaticImageTextVerticalAlign, StaticImageVectorCommand,
    StaticImageVectorFillRule, TextRelief, UnderlineStyle,
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

const ACTIVE_PDF_NAME_TOKENS: &[(&[u8], &str)] = &[
    (b"/3D", "/3D"),
    (b"/A", "/A"),
    (b"/AA", "/AA"),
    (b"/AF", "/AF"),
    (b"/AFRelationship", "/AFRelationship"),
    (b"/AcroForm", "/AcroForm"),
    (b"/Action", "/Action"),
    (b"/Annot", "/Annot"),
    (b"/Annots", "/Annots"),
    (b"/Collection", "/Collection"),
    (b"/EF", "/EF"),
    (b"/EmbeddedFile", "/EmbeddedFile"),
    (b"/EmbeddedFiles", "/EmbeddedFiles"),
    (b"/Encrypt", "/Encrypt"),
    (b"/FileAttachment", "/FileAttachment"),
    (b"/Filespec", "/Filespec"),
    (b"/GoTo", "/GoTo"),
    (b"/GoTo3DView", "/GoTo3DView"),
    (b"/GoToE", "/GoToE"),
    (b"/GoToR", "/GoToR"),
    (b"/Hide", "/Hide"),
    (b"/ImportData", "/ImportData"),
    (b"/JavaScript", "/JavaScript"),
    (b"/JS", "/JS"),
    (b"/Launch", "/Launch"),
    (b"/Link", "/Link"),
    (b"/Movie", "/Movie"),
    (b"/Names", "/Names"),
    (b"/Named", "/Named"),
    (b"/Next", "/Next"),
    (b"/ObjStm", "/ObjStm"),
    (b"/OpenAction", "/OpenAction"),
    (b"/Perms", "/Perms"),
    (b"/Rendition", "/Rendition"),
    (b"/ResetForm", "/ResetForm"),
    (b"/RichMedia", "/RichMedia"),
    (b"/Screen", "/Screen"),
    (b"/SetOCGState", "/SetOCGState"),
    (b"/Sound", "/Sound"),
    (b"/SubmitForm", "/SubmitForm"),
    (b"/Trans", "/Trans"),
    (b"/URI", "/URI"),
    (b"/Widget", "/Widget"),
    (b"/XRef", "/XRef"),
    (b"/XFA", "/XFA"),
];
const MAX_AUDITED_PDF_NAME_BYTES: usize = 128;
const OVERLONG_PDF_NAME_TOKEN: &str = "<overlong PDF name>";
const MALFORMED_PDF_NAME_ESCAPE_TOKEN: &str = "<malformed PDF name escape>";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassivePdfIssue {
    pub token: &'static str,
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassivePdfError {
    pub issues: Vec<PassivePdfIssue>,
}

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
    to_unicode_ref: Ref,
    used_glyphs: Vec<SuppliedGlyph>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
struct SuppliedGlyph {
    cid: u16,
    unicode: char,
    width: f32,
}

#[derive(Debug, Clone)]
struct SuppliedTextEncoding {
    font_index: usize,
    encoded: Vec<u8>,
}

impl fmt::Display for PassivePdfError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(issue) = self.issues.first() {
            write!(
                formatter,
                "passive PDF audit found forbidden token {} at byte {}",
                issue.token, issue.offset
            )
        } else {
            formatter.write_str("passive PDF audit failed")
        }
    }
}

impl Error for PassivePdfError {}

pub fn audit_passive_pdf_bytes(pdf: &[u8]) -> Result<(), PassivePdfError> {
    let mut issues = Vec::new();
    let mut offset = 0;

    while offset < pdf.len() {
        if is_stream_marker_at(pdf, offset) {
            let Some(stream_start) = stream_data_start(pdf, offset) else {
                issues.push(PassivePdfIssue {
                    token: "<malformed stream>",
                    offset,
                });
                break;
            };
            let stream_end = match stream_end_from_declared_length(pdf, offset, stream_start) {
                Some(DeclaredStreamEnd::Found(stream_end)) => stream_end,
                Some(DeclaredStreamEnd::Malformed) => {
                    issues.push(PassivePdfIssue {
                        token: "<malformed stream length>",
                        offset,
                    });
                    break;
                }
                None => {
                    let Some(stream_end) = find_endstream_marker(pdf, stream_start) else {
                        issues.push(PassivePdfIssue {
                            token: "<unterminated stream>",
                            offset,
                        });
                        break;
                    };
                    stream_end
                }
            };
            offset = stream_end + b"endstream".len();
            continue;
        }

        if pdf[offset] == b'/' {
            if let Some(parsed_name) = parse_pdf_name_token(pdf, offset) {
                if parsed_name.truncated {
                    issues.push(PassivePdfIssue {
                        token: OVERLONG_PDF_NAME_TOKEN,
                        offset,
                    });
                }
                if parsed_name.malformed_escape {
                    issues.push(PassivePdfIssue {
                        token: MALFORMED_PDF_NAME_ESCAPE_TOKEN,
                        offset,
                    });
                }
                for (token, label) in ACTIVE_PDF_NAME_TOKENS {
                    if parsed_name.name == *token {
                        issues.push(PassivePdfIssue {
                            token: label,
                            offset,
                        });
                    }
                }
                offset = parsed_name.end;
                continue;
            } else {
                for (token, label) in ACTIVE_PDF_NAME_TOKENS {
                    if pdf[offset..].starts_with(token)
                        && is_pdf_name_boundary(pdf.get(offset + token.len()).copied())
                    {
                        issues.push(PassivePdfIssue {
                            token: label,
                            offset,
                        });
                    }
                }
            }
        }

        offset += 1;
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(PassivePdfError { issues })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedPdfName {
    name: Vec<u8>,
    end: usize,
    truncated: bool,
    malformed_escape: bool,
}

fn parse_pdf_name_token(pdf: &[u8], offset: usize) -> Option<ParsedPdfName> {
    if pdf.get(offset).copied()? != b'/' {
        return None;
    }

    let mut name = Vec::new();
    name.push(b'/');
    let mut truncated = false;
    let mut malformed_escape = false;
    let mut pos = offset + 1;
    while pos < pdf.len() && !is_pdf_name_boundary(Some(pdf[pos])) {
        if pdf[pos] == b'#' {
            if pos + 2 < pdf.len()
                && let (Some(high), Some(low)) = (hex_value(pdf[pos + 1]), hex_value(pdf[pos + 2]))
            {
                truncated |= push_audited_pdf_name_byte(&mut name, (high << 4) | low);
                pos += 3;
                continue;
            }
            malformed_escape = true;
        }

        truncated |= push_audited_pdf_name_byte(&mut name, pdf[pos]);
        pos += 1;
    }

    Some(ParsedPdfName {
        name,
        end: pos,
        truncated,
        malformed_escape,
    })
}

fn push_audited_pdf_name_byte(name: &mut Vec<u8>, byte: u8) -> bool {
    if name.len() < MAX_AUDITED_PDF_NAME_BYTES {
        name.push(byte);
        false
    } else {
        true
    }
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub fn render_pdf(layout: &LayoutDocument) -> Vec<u8> {
    render_pdf_with_font_provider(layout, None)
}

pub fn estimate_passive_pdf_object_count(
    layout: &LayoutDocument,
    font_provider: Option<&FontProvider>,
) -> usize {
    let page_used_font_indexes = collect_page_used_font_indexes(layout, font_provider);
    let used_font_indexes = collect_used_font_indexes(&page_used_font_indexes);
    let extended_latin_usage = collect_extended_latin_font_usage(layout);
    2usize
        .saturating_add(layout.pages.len().saturating_mul(2))
        .saturating_add(used_font_indexes.len())
        .saturating_add(usize::from(
            font_index_for_resource(SYMBOL_REGULAR)
                .is_some_and(|index| used_font_indexes.contains(&index)),
        ))
        .saturating_add(usize::from(
            font_index_for_resource(ZAPF_DINGBATS_REGULAR)
                .is_some_and(|index| used_font_indexes.contains(&index)),
        ))
        .saturating_add(
            extended_latin_usage
                .iter()
                .filter(|usage| !usage.is_empty())
                .count(),
        )
        .saturating_add(
            collect_used_supplied_font_asset_indexes(layout, font_provider)
                .len()
                .saturating_mul(5),
        )
        .saturating_add(count_image_xobjects(layout))
}

pub fn render_pdf_with_font_provider(
    layout: &LayoutDocument,
    font_provider: Option<&FontProvider>,
) -> Vec<u8> {
    let mut pdf = Pdf::new();
    let catalog_id = Ref::new(1);
    let page_tree_id = Ref::new(2);
    let first_page_id = 3;
    let first_content_id = first_page_id + layout.pages.len() as i32;
    let first_font_id = first_content_id + layout.pages.len() as i32;
    let page_used_font_indexes = collect_page_used_font_indexes(layout, font_provider);
    let used_font_indexes = collect_used_font_indexes(&page_used_font_indexes);
    let mut next_object_id = first_font_id;
    let mut font_refs = [None; 14];
    for font_idx in &used_font_indexes {
        font_refs[*font_idx] = Some(Ref::new(next_object_id));
        next_object_id += 1;
    }
    let symbol_to_unicode_ref = font_index_for_resource(SYMBOL_REGULAR)
        .and_then(|index| font_refs[index])
        .map(|_| next_ref(&mut next_object_id));
    let zapf_dingbats_to_unicode_ref = font_index_for_resource(ZAPF_DINGBATS_REGULAR)
        .and_then(|index| font_refs[index])
        .map(|_| next_ref(&mut next_object_id));
    let extended_latin_usage = collect_extended_latin_font_usage(layout);
    let mut extended_latin_to_unicode_refs = [None; 14];
    for (font_idx, usage) in extended_latin_usage.iter().enumerate() {
        if !usage.is_empty() {
            extended_latin_to_unicode_refs[font_idx] = Some(next_ref(&mut next_object_id));
        }
    }
    let supplied_fonts = collect_supplied_pdf_fonts(layout, font_provider, &mut next_object_id);
    let page_supplied_font_indexes =
        collect_page_supplied_font_indexes(layout, font_provider, &supplied_fonts);
    let first_image_id = next_object_id;

    let page_refs = (0..layout.pages.len())
        .map(|idx| Ref::new(first_page_id + idx as i32))
        .collect::<Vec<_>>();
    let image_refs = collect_image_refs(layout, first_image_id);

    pdf.catalog(catalog_id).pages(page_tree_id);
    pdf.pages(page_tree_id)
        .kids(page_refs.iter().copied())
        .count(page_refs.len() as i32);

    for (idx, page) in layout.pages.iter().enumerate() {
        let page_id = page_refs[idx];
        let content_id = Ref::new(first_content_id + idx as i32);

        let mut page_writer = pdf.page(page_id);
        page_writer.parent(page_tree_id);
        page_writer.media_box(Rect::new(0.0, 0.0, page.width, page.height));
        page_writer.contents(content_id);
        {
            let mut resources = page_writer.resources();
            {
                let mut fonts = resources.fonts();
                for font_idx in &page_used_font_indexes[idx] {
                    let (resource_name, _base_font) = BUILTIN_FONTS[*font_idx];
                    if let Some(font_ref) = font_refs[*font_idx] {
                        fonts.pair(Name(resource_name), font_ref);
                    }
                }
                for supplied_idx in &page_supplied_font_indexes[idx] {
                    let supplied = &supplied_fonts[*supplied_idx];
                    fonts.pair(Name(&supplied.resource_name), supplied.type0_ref);
                }
            }
            if let Some(page_images) = image_refs.get(idx)
                && !page_images.is_empty()
            {
                let mut x_objects = resources.x_objects();
                for image in page_images {
                    x_objects.pair(Name(&image.name), image.id);
                }
            }
        }
        page_writer.finish();

        let mut content = Content::new();
        for (item_idx, item) in page.items.iter().enumerate() {
            match item {
                LayoutItem::Highlight {
                    x,
                    y,
                    width,
                    height,
                    color,
                } => {
                    set_fill_color(&mut content, *color);
                    content.rect(*x, *y, *width, *height);
                    content.fill_nonzero();
                }
                LayoutItem::Text(fragment) => {
                    draw_text_layout_item(
                        &mut content,
                        fragment,
                        layout,
                        font_provider,
                        &supplied_fonts,
                    );
                }
                LayoutItem::Underline {
                    x,
                    y,
                    width,
                    color,
                    style,
                } => {
                    draw_underline(&mut content, *x, *y, *width, *color, *style);
                }
                LayoutItem::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    width,
                    color,
                    style,
                } => {
                    draw_passive_line(&mut content, *x1, *y1, *x2, *y2, *width, *color, *style);
                }
                LayoutItem::Ellipse {
                    x,
                    y,
                    width,
                    height,
                    stroke_width,
                    stroke_color,
                    stroke_style,
                    fill_color,
                } => {
                    draw_passive_ellipse(
                        &mut content,
                        *x,
                        *y,
                        *width,
                        *height,
                        *stroke_width,
                        *stroke_color,
                        *stroke_style,
                        *fill_color,
                    );
                }
                LayoutItem::RoundedRectangle {
                    x,
                    y,
                    width,
                    height,
                    radius,
                    stroke_width,
                    stroke_color,
                    stroke_style,
                    fill_color,
                } => {
                    draw_passive_rounded_rectangle(
                        &mut content,
                        *x,
                        *y,
                        *width,
                        *height,
                        *radius,
                        *stroke_width,
                        *stroke_color,
                        *stroke_style,
                        *fill_color,
                    );
                }
                LayoutItem::Polygon {
                    points,
                    stroke_width,
                    stroke_color,
                    stroke_style,
                    fill_color,
                } => {
                    draw_passive_polygon(
                        &mut content,
                        points,
                        *stroke_width,
                        *stroke_color,
                        *stroke_style,
                        StaticImageVectorFillRule::Winding,
                        *fill_color,
                    );
                }
                LayoutItem::Drawing(fragment) => match fragment.item.as_ref() {
                    LayoutItem::Highlight {
                        x,
                        y,
                        width,
                        height,
                        color,
                    } => {
                        set_fill_color(&mut content, *color);
                        content.rect(*x, *y, *width, *height);
                        content.fill_nonzero();
                    }
                    LayoutItem::Line {
                        x1,
                        y1,
                        x2,
                        y2,
                        width,
                        color,
                        style,
                    } => {
                        draw_passive_line(&mut content, *x1, *y1, *x2, *y2, *width, *color, *style);
                    }
                    LayoutItem::Ellipse {
                        x,
                        y,
                        width,
                        height,
                        stroke_width,
                        stroke_color,
                        stroke_style,
                        fill_color,
                    } => {
                        draw_passive_ellipse(
                            &mut content,
                            *x,
                            *y,
                            *width,
                            *height,
                            *stroke_width,
                            *stroke_color,
                            *stroke_style,
                            *fill_color,
                        );
                    }
                    LayoutItem::RoundedRectangle {
                        x,
                        y,
                        width,
                        height,
                        radius,
                        stroke_width,
                        stroke_color,
                        stroke_style,
                        fill_color,
                    } => {
                        draw_passive_rounded_rectangle(
                            &mut content,
                            *x,
                            *y,
                            *width,
                            *height,
                            *radius,
                            *stroke_width,
                            *stroke_color,
                            *stroke_style,
                            *fill_color,
                        );
                    }
                    LayoutItem::Polygon {
                        points,
                        stroke_width,
                        stroke_color,
                        stroke_style,
                        fill_color,
                    } => {
                        draw_passive_polygon(
                            &mut content,
                            points,
                            *stroke_width,
                            *stroke_color,
                            *stroke_style,
                            StaticImageVectorFillRule::Winding,
                            *fill_color,
                        );
                    }
                    LayoutItem::Text(fragment) => {
                        draw_text_layout_item(
                            &mut content,
                            fragment,
                            layout,
                            font_provider,
                            &supplied_fonts,
                        );
                    }
                    _ => {}
                },
                LayoutItem::Image(fragment) => {
                    if fragment.image.format == ImageFormat::Placeholder {
                        draw_passive_image_placeholder(&mut content, fragment);
                        continue;
                    }
                    if fragment.image.format == ImageFormat::WmfVector {
                        draw_passive_wmf_vector_image(&mut content, fragment);
                        continue;
                    }
                    if let Some(image_ref) = image_refs.get(idx).and_then(|page_images| {
                        page_images
                            .iter()
                            .find(|image| image.item_index == item_idx)
                    }) {
                        let draw = image_draw_rect(fragment);
                        content.save_state();
                        if draw.clipped {
                            content.rect(fragment.x, fragment.y, fragment.width, fragment.height);
                            content.clip_nonzero();
                            content.end_path();
                        }
                        content.transform([draw.width, 0.0, 0.0, draw.height, draw.x, draw.y]);
                        content.x_object(Name(&image_ref.name));
                        content.restore_state();
                    }
                }
            }
        }
        pdf.stream(content_id, &content.finish());
    }

    for font_idx in used_font_indexes {
        let Some(font_ref) = font_refs[font_idx] else {
            continue;
        };
        let (_resource_name, base_font) = BUILTIN_FONTS[font_idx];
        let mut font = pdf.type1_font(font_ref);
        font.base_font(Name(base_font));
        if base_font == b"Symbol" {
            if let Some(to_unicode_ref) = symbol_to_unicode_ref {
                font.to_unicode(to_unicode_ref);
            }
        } else if base_font == b"ZapfDingbats" {
            if let Some(to_unicode_ref) = zapf_dingbats_to_unicode_ref {
                font.to_unicode(to_unicode_ref);
            }
        } else if !extended_latin_usage[font_idx].is_empty() {
            {
                let mut encoding = font.encoding_custom();
                encoding.base_encoding(Name(b"WinAnsiEncoding"));
                let mut differences = encoding.differences();
                for entry in &extended_latin_usage[font_idx].entries {
                    differences.consecutive(entry.byte, [Name(entry.glyph_name)]);
                }
            }
            if let Some(to_unicode_ref) = extended_latin_to_unicode_refs[font_idx] {
                font.to_unicode(to_unicode_ref);
            }
        } else {
            font.encoding_predefined(Name(b"WinAnsiEncoding"));
        }
    }

    if let Some(to_unicode_ref) = symbol_to_unicode_ref {
        let symbol_to_unicode =
            passive_to_unicode_cmap(b"OpenRtfConverter-Symbol", PASSIVE_SYMBOL_TO_UNICODE);
        pdf.stream(to_unicode_ref, &symbol_to_unicode);
    }
    if let Some(to_unicode_ref) = zapf_dingbats_to_unicode_ref {
        let zapf_dingbats_to_unicode = passive_to_unicode_cmap(
            b"OpenRtfConverter-ZapfDingbats",
            PASSIVE_ZAPF_DINGBATS_TO_UNICODE,
        );
        pdf.stream(to_unicode_ref, &zapf_dingbats_to_unicode);
    }
    for (font_idx, to_unicode_ref) in extended_latin_to_unicode_refs.iter().enumerate() {
        let Some(to_unicode_ref) = *to_unicode_ref else {
            continue;
        };
        let entries = extended_latin_usage[font_idx].to_unicode_entries();
        let extended_latin_to_unicode =
            passive_to_unicode_cmap(b"OpenRtfConverter-ExtendedLatin", &entries);
        pdf.stream(to_unicode_ref, &extended_latin_to_unicode);
    }
    write_supplied_pdf_fonts(&mut pdf, font_provider, &supplied_fonts);

    for (page_idx, page) in layout.pages.iter().enumerate() {
        for (item_idx, item) in page.items.iter().enumerate() {
            let LayoutItem::Image(fragment) = item else {
                continue;
            };
            let Some(image_ref) = image_refs.get(page_idx).and_then(|page_images| {
                page_images
                    .iter()
                    .find(|image| image.item_index == item_idx)
            }) else {
                continue;
            };
            match fragment.image.format {
                ImageFormat::Jpeg | ImageFormat::JpegGrayscale | ImageFormat::JpegCmyk => {
                    let mut image = pdf.image_xobject(image_ref.id, &fragment.image.bytes);
                    image.width(fragment.image.width_px as i32);
                    image.height(fragment.image.height_px as i32);
                    match fragment.image.format {
                        ImageFormat::Jpeg => image.color_space().device_rgb(),
                        ImageFormat::JpegGrayscale => image.color_space().device_gray(),
                        ImageFormat::JpegCmyk => image.color_space().device_cmyk(),
                        _ => image.color_space().device_rgb(),
                    }
                    image.bits_per_component(8);
                    image.filter(Filter::DctDecode);
                }
                ImageFormat::Png | ImageFormat::PngGrayscale | ImageFormat::PngIndexed => {
                    let mut image = pdf.image_xobject(image_ref.id, &fragment.image.bytes);
                    image.width(fragment.image.width_px as i32);
                    image.height(fragment.image.height_px as i32);
                    let color_components = match fragment.image.format {
                        ImageFormat::Png => {
                            image.color_space().device_rgb();
                            3
                        }
                        ImageFormat::PngGrayscale => {
                            image.color_space().device_gray();
                            1
                        }
                        ImageFormat::PngIndexed => {
                            let hival = fragment
                                .image
                                .palette
                                .len()
                                .checked_div(3)
                                .and_then(|entries| entries.checked_sub(1))
                                .unwrap_or(0) as i32;
                            image.color_space().indexed(
                                Name(b"DeviceRGB"),
                                hival,
                                &fragment.image.palette,
                            );
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
                        .columns(fragment.image.width_px as i32);
                }
                ImageFormat::Rgb8 => {
                    let mut image = pdf.image_xobject(image_ref.id, &fragment.image.bytes);
                    image.width(fragment.image.width_px as i32);
                    image.height(fragment.image.height_px as i32);
                    image.color_space().device_rgb();
                    image.bits_per_component(8);
                }
                ImageFormat::WmfVector | ImageFormat::Placeholder => {}
            }
        }
    }

    pdf.finish()
}

fn draw_text_layout_item(
    content: &mut Content,
    fragment: &TextFragment,
    layout: &LayoutDocument,
    font_provider: Option<&FontProvider>,
    supplied_fonts: &[SuppliedPdfFont],
) {
    if text_fragment_renders_as_passive_vector_only(fragment) {
        draw_passive_vector_only_dingbat_text(content, fragment);
        return;
    }
    let supplied_encoding =
        encode_supplied_text_fragment(fragment, layout, font_provider, supplied_fonts);
    let mut base_encoded = Vec::new();
    let font_resource = if let Some(supplied) = supplied_encoding.as_ref() {
        supplied_fonts[supplied.font_index].resource_name.as_slice()
    } else {
        base_encoded = encode_pdf_text_for_font(&fragment.text, fragment.font_family);
        font_name_for_style(fragment.font_family, &fragment.style).0
    };
    let passive_kerning_family = supplied_encoding.is_none().then_some(fragment.font_family);
    let encoded = supplied_encoding
        .as_ref()
        .map(|supplied| supplied.encoded.as_slice())
        .unwrap_or(base_encoded.as_slice());
    if fragment.style.shadow {
        set_fill_color(content, shadow_color(fragment.color));
        write_text_fragment(
            content,
            &fragment.text,
            font_resource,
            passive_kerning_family,
            &fragment.style,
            fragment.word_spacing,
            fragment.x + shadow_offset(&fragment.style),
            fragment.baseline_y - shadow_offset(&fragment.style),
            encoded,
            TextRenderingMode::Fill,
        );
    }
    if fragment.style.relief != TextRelief::None {
        let offset = relief_offset(&fragment.style);
        let (first_color, first_dx, first_dy, second_color, second_dx, second_dy) =
            relief_layers(fragment.color, fragment.style.relief, offset);
        set_fill_color(content, first_color);
        write_text_fragment(
            content,
            &fragment.text,
            font_resource,
            passive_kerning_family,
            &fragment.style,
            fragment.word_spacing,
            fragment.x + first_dx,
            fragment.baseline_y + first_dy,
            encoded,
            TextRenderingMode::Fill,
        );
        set_fill_color(content, second_color);
        write_text_fragment(
            content,
            &fragment.text,
            font_resource,
            passive_kerning_family,
            &fragment.style,
            fragment.word_spacing,
            fragment.x + second_dx,
            fragment.baseline_y + second_dy,
            encoded,
            TextRenderingMode::Fill,
        );
    }
    set_fill_color(content, fragment.color);
    if fragment.style.outline {
        set_stroke_color(content, fragment.color);
        content.set_line_width((fragment.style.font_size_points() * 0.035).clamp(0.25, 1.25));
    }
    write_text_fragment(
        content,
        &fragment.text,
        font_resource,
        passive_kerning_family,
        &fragment.style,
        fragment.word_spacing,
        fragment.x,
        fragment.baseline_y,
        encoded,
        if fragment.style.outline {
            TextRenderingMode::Stroke
        } else {
            TextRenderingMode::Fill
        },
    );
    draw_passive_text_overlays(content, fragment);
}

fn next_ref(next_object_id: &mut i32) -> Ref {
    let reference = Ref::new(*next_object_id);
    *next_object_id += 1;
    reference
}

fn collect_page_used_font_indexes(
    layout: &LayoutDocument,
    font_provider: Option<&FontProvider>,
) -> Vec<Vec<usize>> {
    layout
        .pages
        .iter()
        .map(|page| collect_used_font_indexes_for_page(layout, page, font_provider))
        .collect()
}

fn collect_used_font_indexes(page_used_font_indexes: &[Vec<usize>]) -> Vec<usize> {
    let mut used = [false; 14];
    for page_fonts in page_used_font_indexes {
        for font_idx in page_fonts {
            used[*font_idx] = true;
        }
    }
    used_font_index_list(&used)
}

fn collect_used_font_indexes_for_page(
    layout: &LayoutDocument,
    page: &crate::layout::LayoutPage,
    font_provider: Option<&FontProvider>,
) -> Vec<usize> {
    let mut used = [false; 14];
    for_each_layout_text_fragment_on_page(page, &mut |fragment| {
        if text_fragment_renders_as_passive_vector_only(fragment) {
            return;
        }
        if font_provider
            .and_then(|provider| supplied_text_encoding_parts(fragment, layout, provider))
            .is_some()
        {
            return;
        }
        mark_used_font_resource(
            &mut used,
            font_resource_for_style(fragment.font_family, &fragment.style),
        );
    });
    for item in &page.items {
        match item {
            LayoutItem::Image(fragment)
                if fragment.image.format == ImageFormat::Placeholder
                    && fragment.width >= 96.0
                    && fragment.height >= 24.0 =>
            {
                mark_used_font_resource(&mut used, HELVETICA_REGULAR);
            }
            LayoutItem::Image(fragment)
                if fragment
                    .image
                    .vector_commands
                    .iter()
                    .any(|command| matches!(command, StaticImageVectorCommand::Text { .. })) =>
            {
                mark_used_font_resource(&mut used, HELVETICA_REGULAR);
            }
            _ => {}
        }
    }
    used_font_index_list(&used)
}

fn for_each_layout_text_fragment_on_page<F>(page: &crate::layout::LayoutPage, callback: &mut F)
where
    F: FnMut(&TextFragment),
{
    for item in &page.items {
        for_each_layout_item_text_fragment(item, callback);
    }
}

fn for_each_layout_item_text_fragment<F>(item: &LayoutItem, callback: &mut F)
where
    F: FnMut(&TextFragment),
{
    match item {
        LayoutItem::Text(fragment) => callback(fragment),
        LayoutItem::Drawing(fragment) => {
            for_each_layout_item_text_fragment(&fragment.item, callback)
        }
        LayoutItem::Highlight { .. }
        | LayoutItem::Underline { .. }
        | LayoutItem::Line { .. }
        | LayoutItem::Ellipse { .. }
        | LayoutItem::RoundedRectangle { .. }
        | LayoutItem::Polygon { .. }
        | LayoutItem::Image(_) => {}
    }
}

fn collect_supplied_pdf_fonts(
    layout: &LayoutDocument,
    font_provider: Option<&FontProvider>,
    next_object_id: &mut i32,
) -> Vec<SuppliedPdfFont> {
    let Some(font_provider) = font_provider else {
        return Vec::new();
    };
    let mut fonts = Vec::<SuppliedPdfFont>::new();
    for page in &layout.pages {
        for_each_layout_text_fragment_on_page(page, &mut |fragment| {
            let Some((asset_index, glyphs, _encoded)) =
                supplied_text_encoding_parts(fragment, layout, font_provider)
            else {
                return;
            };
            let font_index = if let Some(index) = fonts
                .iter()
                .position(|font| font.asset_index == asset_index)
            {
                index
            } else {
                let index = fonts.len();
                let base_name = font_provider
                    .assets
                    .get(asset_index)
                    .map(|asset| supplied_pdf_font_base_name(asset, index))
                    .unwrap_or_else(|| supplied_pdf_font_fallback_base_name(index));
                fonts.push(SuppliedPdfFont {
                    asset_index,
                    resource_name: format!("TF{}", index + 1).into_bytes(),
                    base_name,
                    type0_ref: next_ref(next_object_id),
                    cid_ref: next_ref(next_object_id),
                    descriptor_ref: next_ref(next_object_id),
                    font_file_ref: next_ref(next_object_id),
                    to_unicode_ref: next_ref(next_object_id),
                    used_glyphs: Vec::new(),
                });
                index
            };
            for glyph in glyphs {
                add_supplied_glyph(&mut fonts[font_index].used_glyphs, glyph);
            }
        });
    }
    fonts
}

fn collect_used_supplied_font_asset_indexes(
    layout: &LayoutDocument,
    font_provider: Option<&FontProvider>,
) -> Vec<usize> {
    let Some(font_provider) = font_provider else {
        return Vec::new();
    };
    let mut indexes = Vec::<usize>::new();
    for page in &layout.pages {
        for_each_layout_text_fragment_on_page(page, &mut |fragment| {
            let Some((asset_index, _glyphs, _encoded)) =
                supplied_text_encoding_parts(fragment, layout, font_provider)
            else {
                return;
            };
            if !indexes.contains(&asset_index) {
                indexes.push(asset_index);
            }
        });
    }
    indexes
}

fn collect_page_supplied_font_indexes(
    layout: &LayoutDocument,
    font_provider: Option<&FontProvider>,
    supplied_fonts: &[SuppliedPdfFont],
) -> Vec<Vec<usize>> {
    layout
        .pages
        .iter()
        .map(|page| {
            let mut indexes = Vec::new();
            for_each_layout_text_fragment_on_page(page, &mut |fragment| {
                let Some(encoding) =
                    encode_supplied_text_fragment(fragment, layout, font_provider, supplied_fonts)
                else {
                    return;
                };
                if !indexes.contains(&encoding.font_index) {
                    indexes.push(encoding.font_index);
                }
            });
            indexes
        })
        .collect()
}

fn collect_extended_latin_font_usage(layout: &LayoutDocument) -> [ExtendedLatinUsage; 14] {
    let mut usage: [ExtendedLatinUsage; 14] =
        std::array::from_fn(|_| ExtendedLatinUsage::default());
    for page in &layout.pages {
        for_each_layout_text_fragment_on_page(page, &mut |fragment| {
            let resource = font_resource_for_style(fragment.font_family, &fragment.style);
            let Some(font_idx) = font_index_for_resource(resource) else {
                return;
            };
            if !is_normal_text_font_index(font_idx) {
                return;
            }
            for ch in fragment.text.chars() {
                if let Some(entry) = extended_latin_entry_for_char(ch) {
                    usage[font_idx].add(entry);
                }
            }
        });
    }
    usage
}

fn encode_supplied_text_fragment(
    fragment: &TextFragment,
    layout: &LayoutDocument,
    font_provider: Option<&FontProvider>,
    supplied_fonts: &[SuppliedPdfFont],
) -> Option<SuppliedTextEncoding> {
    let font_provider = font_provider?;
    let (asset_index, _glyphs, encoded) =
        supplied_text_encoding_parts(fragment, layout, font_provider)?;
    let font_index = supplied_fonts
        .iter()
        .position(|font| font.asset_index == asset_index)?;
    Some(SuppliedTextEncoding {
        font_index,
        encoded,
    })
}

fn supplied_text_encoding_parts(
    fragment: &TextFragment,
    layout: &LayoutDocument,
    font_provider: &FontProvider,
) -> Option<(usize, Vec<SuppliedGlyph>, Vec<u8>)> {
    if fragment.text.is_empty() {
        return None;
    }
    let source_font = layout
        .fonts
        .iter()
        .find(|font| font.index == fragment.style.font_index)?;
    font_provider
        .assets
        .iter()
        .enumerate()
        .filter(|(_, asset)| supplied_font_asset_matches_font(asset, source_font))
        .filter_map(|(asset_index, asset)| {
            let (glyphs, encoded) = encode_text_with_font_asset(&fragment.text, asset)?;
            (!glyphs.is_empty()).then(|| {
                (
                    supplied_font_style_mismatch_score(asset.style, &fragment.style),
                    asset_index,
                    glyphs,
                    encoded,
                )
            })
        })
        .min_by_key(|(score, asset_index, _, _)| (*score, *asset_index))
        .map(|(_, asset_index, glyphs, encoded)| (asset_index, glyphs, encoded))
}

fn supplied_font_asset_matches_font(asset: &FontAsset, font: &crate::model::FontDef) -> bool {
    asset.matches_family(&font.name)
        || font
            .alternate_name
            .as_deref()
            .is_some_and(|alternate| asset.matches_family(alternate))
}

fn supplied_pdf_font_base_name(asset: &FontAsset, font_index: usize) -> Vec<u8> {
    let Some(name) = supplied_pdf_font_postscript_name(asset) else {
        return supplied_pdf_font_fallback_base_name(font_index);
    };
    let sanitized = sanitize_pdf_font_name(&name, 96);
    if sanitized.is_empty() {
        supplied_pdf_font_fallback_base_name(font_index)
    } else {
        format!("ORTF{:02}+{sanitized}", font_index + 1).into_bytes()
    }
}

fn supplied_pdf_font_fallback_base_name(font_index: usize) -> Vec<u8> {
    format!("ORTFSuppliedFont{}", font_index + 1).into_bytes()
}

fn supplied_pdf_font_postscript_name(asset: &FontAsset) -> Option<String> {
    let face = Face::parse(&asset.bytes, 0).ok()?;
    for name in face.names() {
        if name.name_id != name_id::POST_SCRIPT_NAME {
            continue;
        }
        let Some(value) = font_name_to_string(&name) else {
            continue;
        };
        if !value.trim().is_empty() {
            return Some(value);
        }
    }
    None
}

fn font_name_to_string(name: &ttf_parser::name::Name<'_>) -> Option<String> {
    if !name.is_unicode() || name.name.len() % 2 != 0 {
        return None;
    }
    let utf16 = name
        .name
        .chunks_exact(2)
        .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&utf16).ok()
}

fn sanitize_pdf_font_name(value: &str, max_len: usize) -> String {
    let mut output = String::new();
    for ch in value.chars() {
        if output.len() >= max_len {
            break;
        }
        let byte = if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '+') {
            ch as u8
        } else {
            b'-'
        };
        if output.len() + 1 > max_len {
            break;
        }
        output.push(byte as char);
    }
    output.trim_matches('-').to_string()
}

fn supplied_font_style_mismatch_score(
    asset_style: crate::fonts::FontAssetStyle,
    run_style: &CharacterStyle,
) -> u8 {
    u8::from(asset_style.bold != run_style.bold) + u8::from(asset_style.italic != run_style.italic)
}

fn encode_text_with_font_asset(
    text: &str,
    asset: &FontAsset,
) -> Option<(Vec<SuppliedGlyph>, Vec<u8>)> {
    let face = Face::parse(&asset.bytes, 0).ok()?;
    let units_per_em = f32::from(face.units_per_em()).max(1.0);
    let mut glyphs = Vec::new();
    let mut encoded = Vec::new();
    for ch in text.chars() {
        if is_zero_width_pdf_char(ch) {
            continue;
        }
        let glyph_id = face.glyph_index(ch)?;
        if glyph_id.0 == 0 {
            return None;
        }
        let advance = face.glyph_hor_advance(glyph_id)?;
        let cid = glyph_id.0;
        encoded.extend_from_slice(&cid.to_be_bytes());
        glyphs.push(SuppliedGlyph {
            cid,
            unicode: ch,
            width: f32::from(advance) * 1000.0 / units_per_em,
        });
    }
    Some((glyphs, encoded))
}

fn add_supplied_glyph(glyphs: &mut Vec<SuppliedGlyph>, glyph: SuppliedGlyph) {
    if glyphs.iter().any(|existing| existing.cid == glyph.cid) {
        return;
    }
    glyphs.push(glyph);
    glyphs.sort_by_key(|glyph| glyph.cid);
}

fn is_normal_text_font_index(font_idx: usize) -> bool {
    let (_resource_name, base_font) = BUILTIN_FONTS[font_idx];
    base_font != b"Symbol" && base_font != b"ZapfDingbats"
}

fn used_font_index_list(used: &[bool; 14]) -> Vec<usize> {
    used.iter()
        .enumerate()
        .filter_map(|(idx, is_used)| is_used.then_some(idx))
        .collect()
}

fn mark_used_font_resource(used: &mut [bool; 14], resource_name: &[u8]) {
    if let Some(index) = font_index_for_resource(resource_name) {
        used[index] = true;
    }
}

fn font_index_for_resource(resource_name: &[u8]) -> Option<usize> {
    BUILTIN_FONTS
        .iter()
        .position(|(candidate, _base_font)| *candidate == resource_name)
}

#[derive(Debug, Copy, Clone)]
struct ImageDrawRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    clipped: bool,
}

fn image_draw_rect(fragment: &crate::layout::ImageFragment) -> ImageDrawRect {
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
    let crop = fragment.image.crop;
    let mut left = twips_to_points(crop.left_twips.max(0)) / natural_width;
    let mut right = twips_to_points(crop.right_twips.max(0)) / natural_width;
    let mut top = twips_to_points(crop.top_twips.max(0)) / natural_height;
    let mut bottom = twips_to_points(crop.bottom_twips.max(0)) / natural_height;
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
    }
}

fn clamp_crop_pair(first: &mut f32, second: &mut f32) {
    let total = *first + *second;
    if total > 0.95 {
        let scale = 0.95 / total;
        *first *= scale;
        *second *= scale;
    }
}

#[derive(Debug, Clone)]
struct PdfImageRef {
    item_index: usize,
    name: Vec<u8>,
    id: Ref,
}

fn collect_image_refs(layout: &LayoutDocument, first_image_id: i32) -> Vec<Vec<PdfImageRef>> {
    let mut next_id = first_image_id;
    layout
        .pages
        .iter()
        .map(|page| {
            page.items
                .iter()
                .enumerate()
                .filter_map(|(idx, item)| {
                    if matches!(item, LayoutItem::Image(fragment) if image_format_uses_xobject(fragment.image.format))
                    {
                        let id = Ref::new(next_id);
                        let name = format!("Im{}", next_id - first_image_id + 1).into_bytes();
                        next_id += 1;
                        Some(PdfImageRef {
                            item_index: idx,
                            name,
                            id,
                        })
                    } else {
                        None
                    }
                })
                .collect()
        })
        .collect()
}

fn image_format_uses_xobject(format: ImageFormat) -> bool {
    matches!(
        format,
        ImageFormat::Jpeg
            | ImageFormat::JpegGrayscale
            | ImageFormat::JpegCmyk
            | ImageFormat::Png
            | ImageFormat::PngGrayscale
            | ImageFormat::PngIndexed
            | ImageFormat::Rgb8
    )
}

fn count_image_xobjects(layout: &LayoutDocument) -> usize {
    layout
        .pages
        .iter()
        .map(|page| {
            page.items
                .iter()
                .filter(|item| {
                    matches!(item, LayoutItem::Image(fragment) if image_format_uses_xobject(fragment.image.format))
                })
                .count()
        })
        .sum()
}

fn draw_passive_wmf_vector_image(content: &mut Content, fragment: &crate::layout::ImageFragment) {
    let draw = image_draw_rect(fragment);
    let source_width = fragment.image.width_px.max(1) as f32;
    let source_height = fragment.image.height_px.max(1) as f32;

    content.save_state();
    content.rect(fragment.x, fragment.y, fragment.width, fragment.height);
    content.clip_nonzero();
    content.end_path();
    for command in &fragment.image.vector_commands {
        match command {
            StaticImageVectorCommand::Line {
                x1,
                y1,
                x2,
                y2,
                stroke_color,
                stroke_width,
                stroke_style,
            } => {
                let stroke_width =
                    vector_command_stroke_width(draw, source_width, source_height, *stroke_width);
                let stroke_style = vector_command_line_style(*stroke_style);
                draw_passive_vector_line(
                    content,
                    vector_command_point(draw, source_width, source_height, *x1, *y1),
                    vector_command_point(draw, source_width, source_height, *x2, *y2),
                    *stroke_color,
                    stroke_width,
                    stroke_style,
                );
            }
            StaticImageVectorCommand::Polyline {
                points,
                stroke_color,
                stroke_width,
                stroke_style,
            } => {
                let points = vector_command_points(draw, source_width, source_height, points);
                let stroke_width =
                    vector_command_stroke_width(draw, source_width, source_height, *stroke_width);
                let stroke_style = vector_command_line_style(*stroke_style);
                draw_passive_vector_polyline(
                    content,
                    &points,
                    *stroke_color,
                    stroke_width,
                    stroke_style,
                );
            }
            StaticImageVectorCommand::Polygon {
                points,
                stroke_color,
                stroke_width,
                stroke_style,
                fill_rule,
                fill_pattern,
                fill_color,
            } => {
                let points = vector_command_points(draw, source_width, source_height, points);
                let stroke_width =
                    vector_command_stroke_width(draw, source_width, source_height, *stroke_width);
                let stroke_style = vector_command_line_style(*stroke_style);
                draw_passive_vector_polygon(
                    content,
                    &points,
                    *stroke_color,
                    stroke_width,
                    stroke_style,
                    *fill_rule,
                    *fill_pattern,
                    *fill_color,
                );
            }
            StaticImageVectorCommand::Rectangle {
                left,
                top,
                right,
                bottom,
                stroke_color,
                stroke_width,
                stroke_style,
                fill_pattern,
                fill_color,
            } => {
                let rect = vector_command_rect(
                    draw,
                    source_width,
                    source_height,
                    *left,
                    *top,
                    *right,
                    *bottom,
                );
                let stroke_width =
                    vector_command_stroke_width(draw, source_width, source_height, *stroke_width);
                let stroke_style = vector_command_line_style(*stroke_style);
                draw_passive_vector_rectangle(
                    content,
                    rect,
                    *stroke_color,
                    stroke_width,
                    stroke_style,
                    *fill_pattern,
                    *fill_color,
                );
            }
            StaticImageVectorCommand::RoundedRectangle {
                left,
                top,
                right,
                bottom,
                corner_width,
                corner_height,
                stroke_color,
                stroke_width,
                stroke_style,
                fill_pattern,
                fill_color,
            } => {
                let rect = vector_command_rect(
                    draw,
                    source_width,
                    source_height,
                    *left,
                    *top,
                    *right,
                    *bottom,
                );
                let corner_width = (*corner_width / source_width) * draw.width;
                let corner_height = (*corner_height / source_height) * draw.height;
                let stroke_width =
                    vector_command_stroke_width(draw, source_width, source_height, *stroke_width);
                let stroke_style = vector_command_line_style(*stroke_style);
                draw_passive_vector_rounded_rectangle(
                    content,
                    rect,
                    corner_width,
                    corner_height,
                    *stroke_color,
                    stroke_width,
                    stroke_style,
                    *fill_pattern,
                    *fill_color,
                );
            }
            StaticImageVectorCommand::Ellipse {
                left,
                top,
                right,
                bottom,
                stroke_color,
                stroke_width,
                stroke_style,
                fill_pattern,
                fill_color,
            } => {
                let rect = vector_command_rect(
                    draw,
                    source_width,
                    source_height,
                    *left,
                    *top,
                    *right,
                    *bottom,
                );
                let stroke_width =
                    vector_command_stroke_width(draw, source_width, source_height, *stroke_width);
                let stroke_style = vector_command_line_style(*stroke_style);
                draw_passive_vector_ellipse(
                    content,
                    rect,
                    *stroke_color,
                    stroke_width,
                    stroke_style,
                    *fill_pattern,
                    *fill_color,
                );
            }
            StaticImageVectorCommand::Text {
                x,
                y,
                height,
                text,
                color,
                background_color,
                clip_bounds,
                character_extra,
                horizontal_align,
                vertical_align,
            } => {
                let point = vector_command_point(draw, source_width, source_height, *x, *y);
                let font_size = ((*height / source_height) * draw.height).clamp(4.0, 72.0);
                let character_extra = ((*character_extra / source_width) * draw.width)
                    .clamp(-font_size, font_size * 4.0);
                let clip_rect = clip_bounds.map(|bounds| {
                    vector_command_rect(
                        draw,
                        source_width,
                        source_height,
                        bounds.left,
                        bounds.top,
                        bounds.right,
                        bounds.bottom,
                    )
                });
                draw_passive_vector_text(
                    content,
                    point,
                    font_size,
                    text,
                    *color,
                    *background_color,
                    clip_rect,
                    character_extra,
                    *horizontal_align,
                    *vertical_align,
                );
            }
        }
    }
    content.restore_state();
}

fn vector_command_points(
    draw: ImageDrawRect,
    source_width: f32,
    source_height: f32,
    points: &[(f32, f32)],
) -> Vec<crate::layout::LayoutPoint> {
    points
        .iter()
        .map(|(x, y)| vector_command_point(draw, source_width, source_height, *x, *y))
        .collect()
}

fn vector_command_point(
    draw: ImageDrawRect,
    source_width: f32,
    source_height: f32,
    x: f32,
    y: f32,
) -> crate::layout::LayoutPoint {
    crate::layout::LayoutPoint {
        x: draw.x + (x / source_width) * draw.width,
        y: draw.y + draw.height - (y / source_height) * draw.height,
    }
}

#[derive(Debug, Copy, Clone)]
struct VectorDrawRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

fn vector_command_rect(
    draw: ImageDrawRect,
    source_width: f32,
    source_height: f32,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
) -> VectorDrawRect {
    let x = draw.x + (left / source_width) * draw.width;
    let width = ((right - left) / source_width * draw.width).max(0.1);
    let y = draw.y + draw.height - (bottom / source_height) * draw.height;
    let height = ((bottom - top) / source_height * draw.height).max(0.1);
    VectorDrawRect {
        x,
        y,
        width,
        height,
    }
}

fn vector_command_stroke_width(
    draw: ImageDrawRect,
    source_width: f32,
    source_height: f32,
    width: f32,
) -> f32 {
    let width = width.max(0.0);
    if width == 0.0 {
        return 0.0;
    }
    let x_width = (width / source_width.max(1.0)) * draw.width;
    let y_width = (width / source_height.max(1.0)) * draw.height;
    ((x_width + y_width) * 0.5).clamp(0.25, 24.0)
}

fn vector_command_line_style(style: BorderStyle) -> LineStyle {
    match style {
        BorderStyle::Dotted => LineStyle::Dotted,
        BorderStyle::Dashed => LineStyle::Dashed,
        _ => LineStyle::Solid,
    }
}

fn draw_passive_vector_line(
    content: &mut Content,
    from: crate::layout::LayoutPoint,
    to: crate::layout::LayoutPoint,
    stroke_color: Option<crate::model::Color>,
    stroke_width: f32,
    stroke_style: LineStyle,
) {
    let Some(color) = stroke_color else {
        return;
    };
    if stroke_width <= 0.0 {
        return;
    }
    draw_passive_line(
        content,
        from.x,
        from.y,
        to.x,
        to.y,
        stroke_width,
        pdf_color_from_model(color),
        stroke_style,
    );
}

fn draw_passive_vector_polyline(
    content: &mut Content,
    points: &[crate::layout::LayoutPoint],
    stroke_color: Option<crate::model::Color>,
    stroke_width: f32,
    stroke_style: LineStyle,
) {
    if points.len() < 2 {
        return;
    }
    for pair in points.windows(2) {
        draw_passive_vector_line(
            content,
            pair[0],
            pair[1],
            stroke_color,
            stroke_width,
            stroke_style,
        );
    }
}

fn draw_passive_vector_polygon(
    content: &mut Content,
    points: &[crate::layout::LayoutPoint],
    stroke_color: Option<crate::model::Color>,
    stroke_width: f32,
    stroke_style: LineStyle,
    fill_rule: StaticImageVectorFillRule,
    fill_pattern: ShadingPattern,
    fill_color: Option<crate::model::Color>,
) {
    if points.len() < 3 {
        return;
    }
    if fill_pattern != ShadingPattern::None
        && let Some(fill_color) = fill_color
    {
        draw_passive_hatch_polygon(
            content,
            points,
            fill_rule,
            fill_pattern,
            pdf_color_from_model(fill_color),
        );
        if stroke_color.is_none() || stroke_width <= 0.0 {
            return;
        }
        draw_passive_polygon(
            content,
            points,
            stroke_width,
            stroke_color.map(pdf_color_from_model).unwrap_or(PdfColor {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
            }),
            stroke_style,
            fill_rule,
            None,
        );
        return;
    }
    draw_passive_polygon(
        content,
        points,
        stroke_color.map(|_| stroke_width).unwrap_or(0.0),
        stroke_color.map(pdf_color_from_model).unwrap_or(PdfColor {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
        }),
        stroke_style,
        fill_rule,
        fill_color.map(pdf_color_from_model),
    );
}

fn draw_passive_vector_rectangle(
    content: &mut Content,
    rect: VectorDrawRect,
    stroke_color: Option<crate::model::Color>,
    stroke_width: f32,
    stroke_style: LineStyle,
    fill_pattern: ShadingPattern,
    fill_color: Option<crate::model::Color>,
) {
    if fill_color.is_none() && stroke_color.is_none() {
        return;
    }
    if let Some(color) = fill_color
        && fill_pattern == ShadingPattern::None
    {
        set_fill_color(content, pdf_color_from_model(color));
    } else if let Some(color) = fill_color {
        draw_passive_hatch_rect(content, rect, fill_pattern, pdf_color_from_model(color));
    }
    if let Some(color) = stroke_color {
        set_stroke_color(content, pdf_color_from_model(color));
        content.set_line_width(stroke_width.max(0.25));
        set_passive_path_stroke_style(content, stroke_width, stroke_style);
    }
    content.rect(rect.x, rect.y, rect.width, rect.height);
    match (fill_color, stroke_color, fill_pattern) {
        (Some(_), Some(_), ShadingPattern::None) => {
            content.fill_nonzero_and_stroke();
        }
        (Some(_), None, ShadingPattern::None) => {
            content.fill_nonzero();
        }
        (_, Some(_), _) => {
            content.stroke();
        }
        _ => {
            content.end_path();
        }
    }
}

fn draw_passive_hatch_rect(
    content: &mut Content,
    rect: VectorDrawRect,
    pattern: ShadingPattern,
    color: PdfColor,
) {
    if pattern == ShadingPattern::None || rect.width <= 0.5 || rect.height <= 0.5 {
        return;
    }
    content.save_state();
    content.rect(rect.x, rect.y, rect.width, rect.height);
    content.clip_nonzero();
    content.end_path();
    draw_passive_hatch_lines(content, rect, pattern, color);
    content.restore_state();
}

fn draw_passive_hatch_polygon(
    content: &mut Content,
    points: &[crate::layout::LayoutPoint],
    fill_rule: StaticImageVectorFillRule,
    pattern: ShadingPattern,
    color: PdfColor,
) {
    let Some(bounds) = vector_points_bounds(points) else {
        return;
    };
    if pattern == ShadingPattern::None || bounds.width <= 0.5 || bounds.height <= 0.5 {
        return;
    }
    content.save_state();
    append_passive_polygon_path(content, points);
    match fill_rule {
        StaticImageVectorFillRule::Alternate => {
            content.clip_even_odd();
        }
        StaticImageVectorFillRule::Winding => {
            content.clip_nonzero();
        }
    }
    content.end_path();
    draw_passive_hatch_lines(content, bounds, pattern, color);
    content.restore_state();
}

fn draw_passive_hatch_rounded_rectangle(
    content: &mut Content,
    rect: VectorDrawRect,
    radius: f32,
    pattern: ShadingPattern,
    color: PdfColor,
) {
    if pattern == ShadingPattern::None || rect.width <= 0.5 || rect.height <= 0.5 {
        return;
    }
    content.save_state();
    append_passive_rounded_rectangle_path(content, rect.x, rect.y, rect.width, rect.height, radius);
    content.clip_nonzero();
    content.end_path();
    draw_passive_hatch_lines(content, rect, pattern, color);
    content.restore_state();
}

fn draw_passive_hatch_ellipse(
    content: &mut Content,
    rect: VectorDrawRect,
    pattern: ShadingPattern,
    color: PdfColor,
) {
    if pattern == ShadingPattern::None || rect.width <= 0.5 || rect.height <= 0.5 {
        return;
    }
    content.save_state();
    append_passive_ellipse_path(content, rect.x, rect.y, rect.width, rect.height);
    content.clip_nonzero();
    content.end_path();
    draw_passive_hatch_lines(content, rect, pattern, color);
    content.restore_state();
}

fn draw_passive_hatch_lines(
    content: &mut Content,
    rect: VectorDrawRect,
    pattern: ShadingPattern,
    color: PdfColor,
) {
    set_stroke_color(content, color);
    content.set_line_width(0.35);

    let spacing = match pattern {
        ShadingPattern::DarkHorizontal
        | ShadingPattern::DarkVertical
        | ShadingPattern::DarkForwardDiagonal
        | ShadingPattern::DarkBackwardDiagonal
        | ShadingPattern::DarkCross
        | ShadingPattern::DarkDiagonalCross => 2.5,
        _ => 4.0,
    };
    match pattern {
        ShadingPattern::Horizontal | ShadingPattern::DarkHorizontal => {
            draw_passive_horizontal_hatch_lines(content, rect, spacing);
        }
        ShadingPattern::Vertical | ShadingPattern::DarkVertical => {
            draw_passive_vertical_hatch_lines(content, rect, spacing);
        }
        ShadingPattern::ForwardDiagonal | ShadingPattern::DarkForwardDiagonal => {
            draw_passive_forward_diagonal_hatch_lines(content, rect, spacing);
        }
        ShadingPattern::BackwardDiagonal | ShadingPattern::DarkBackwardDiagonal => {
            draw_passive_backward_diagonal_hatch_lines(content, rect, spacing);
        }
        ShadingPattern::Cross | ShadingPattern::DarkCross => {
            draw_passive_horizontal_hatch_lines(content, rect, spacing);
            draw_passive_vertical_hatch_lines(content, rect, spacing);
        }
        ShadingPattern::DiagonalCross | ShadingPattern::DarkDiagonalCross => {
            draw_passive_forward_diagonal_hatch_lines(content, rect, spacing);
            draw_passive_backward_diagonal_hatch_lines(content, rect, spacing);
        }
        ShadingPattern::None => {}
    }
}

fn draw_passive_horizontal_hatch_lines(content: &mut Content, rect: VectorDrawRect, spacing: f32) {
    let mut cursor = rect.y + spacing;
    let max_y = rect.y + rect.height;
    while cursor < max_y {
        stroke_line(content, rect.x, cursor, rect.x + rect.width, cursor, 0.35);
        cursor += spacing;
    }
}

fn draw_passive_vertical_hatch_lines(content: &mut Content, rect: VectorDrawRect, spacing: f32) {
    let mut cursor = rect.x + spacing;
    let max_x = rect.x + rect.width;
    while cursor < max_x {
        stroke_line(content, cursor, rect.y, cursor, rect.y + rect.height, 0.35);
        cursor += spacing;
    }
}

fn draw_passive_forward_diagonal_hatch_lines(
    content: &mut Content,
    rect: VectorDrawRect,
    spacing: f32,
) {
    let mut offset = spacing;
    while offset < rect.width {
        let length = (rect.width - offset).min(rect.height);
        stroke_line(
            content,
            rect.x + offset,
            rect.y,
            rect.x + offset + length,
            rect.y + length,
            0.35,
        );
        offset += spacing;
    }

    let mut offset = spacing;
    while offset < rect.height {
        let length = rect.width.min(rect.height - offset);
        stroke_line(
            content,
            rect.x,
            rect.y + offset,
            rect.x + length,
            rect.y + offset + length,
            0.35,
        );
        offset += spacing;
    }
}

fn draw_passive_backward_diagonal_hatch_lines(
    content: &mut Content,
    rect: VectorDrawRect,
    spacing: f32,
) {
    let mut offset = spacing;
    while offset < rect.height {
        let length = rect.width.min(offset);
        stroke_line(
            content,
            rect.x,
            rect.y + offset,
            rect.x + length,
            rect.y + offset - length,
            0.35,
        );
        offset += spacing;
    }

    let mut offset = spacing;
    while offset < rect.width {
        let length = (rect.width - offset).min(rect.height);
        stroke_line(
            content,
            rect.x + offset,
            rect.y + rect.height,
            rect.x + offset + length,
            rect.y + rect.height - length,
            0.35,
        );
        offset += spacing;
    }
}

fn draw_passive_vector_rounded_rectangle(
    content: &mut Content,
    rect: VectorDrawRect,
    corner_width: f32,
    corner_height: f32,
    stroke_color: Option<crate::model::Color>,
    stroke_width: f32,
    stroke_style: LineStyle,
    fill_pattern: ShadingPattern,
    fill_color: Option<crate::model::Color>,
) {
    let radius = (corner_width.min(corner_height) * 0.5).max(0.1);
    if fill_pattern != ShadingPattern::None
        && let Some(fill_color) = fill_color
    {
        draw_passive_hatch_rounded_rectangle(
            content,
            rect,
            radius,
            fill_pattern,
            pdf_color_from_model(fill_color),
        );
        if stroke_color.is_none() || stroke_width <= 0.0 {
            return;
        }
        draw_passive_rounded_rectangle(
            content,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            radius,
            stroke_width,
            stroke_color.map(pdf_color_from_model).unwrap_or(PdfColor {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
            }),
            stroke_style,
            None,
        );
        return;
    }
    draw_passive_rounded_rectangle(
        content,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        radius,
        stroke_color.map(|_| stroke_width).unwrap_or(0.0),
        stroke_color.map(pdf_color_from_model).unwrap_or(PdfColor {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
        }),
        stroke_style,
        fill_color.map(pdf_color_from_model),
    );
}

fn draw_passive_vector_ellipse(
    content: &mut Content,
    rect: VectorDrawRect,
    stroke_color: Option<crate::model::Color>,
    stroke_width: f32,
    stroke_style: LineStyle,
    fill_pattern: ShadingPattern,
    fill_color: Option<crate::model::Color>,
) {
    if fill_pattern != ShadingPattern::None
        && let Some(fill_color) = fill_color
    {
        draw_passive_hatch_ellipse(
            content,
            rect,
            fill_pattern,
            pdf_color_from_model(fill_color),
        );
        if stroke_color.is_none() || stroke_width <= 0.0 {
            return;
        }
        draw_passive_ellipse(
            content,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            stroke_width,
            stroke_color.map(pdf_color_from_model).unwrap_or(PdfColor {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
            }),
            stroke_style,
            None,
        );
        return;
    }
    draw_passive_ellipse(
        content,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        stroke_color.map(|_| stroke_width).unwrap_or(0.0),
        stroke_color.map(pdf_color_from_model).unwrap_or(PdfColor {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
        }),
        stroke_style,
        fill_color.map(pdf_color_from_model),
    );
}

fn draw_passive_vector_text(
    content: &mut Content,
    point: crate::layout::LayoutPoint,
    font_size: f32,
    text: &str,
    color: Option<crate::model::Color>,
    background_color: Option<crate::model::Color>,
    clip_rect: Option<VectorDrawRect>,
    character_extra: f32,
    horizontal_align: StaticImageTextHorizontalAlign,
    vertical_align: StaticImageTextVerticalAlign,
) {
    if text.is_empty() {
        return;
    }
    if let Some(rect) = clip_rect {
        content.save_state();
        content.rect(rect.x, rect.y, rect.width, rect.height);
        content.clip_nonzero();
        content.end_path();
    }
    let mut style = CharacterStyle::default();
    style.font_size_half_points = (font_size * 2.0).round().clamp(1.0, 144.0) as i32;
    style.character_spacing_twips = (character_extra * 20.0).round().clamp(-1440.0, 5760.0) as i32;
    let metrics = passive_vector_text_metrics(
        point,
        font_size,
        text,
        character_extra,
        horizontal_align,
        vertical_align,
    );
    if let Some(background_color) = background_color {
        draw_passive_vector_rectangle(
            content,
            VectorDrawRect {
                x: metrics.x,
                y: metrics.bottom_y,
                width: metrics.width,
                height: metrics.top_y - metrics.bottom_y,
            },
            None,
            0.0,
            LineStyle::Solid,
            ShadingPattern::None,
            Some(background_color),
        );
    }
    set_fill_color(
        content,
        color.map(pdf_color_from_model).unwrap_or(PdfColor {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
        }),
    );
    let encoded = encode_pdf_text_for_font(text, PdfFontFamily::Helvetica);
    write_text_fragment(
        content,
        text,
        HELVETICA_REGULAR,
        Some(PdfFontFamily::Helvetica),
        &style,
        0.0,
        metrics.x,
        metrics.baseline_y,
        &encoded,
        TextRenderingMode::Fill,
    );
    if clip_rect.is_some() {
        content.restore_state();
    }
}

#[derive(Debug, Copy, Clone)]
struct PassiveVectorTextMetrics {
    x: f32,
    baseline_y: f32,
    top_y: f32,
    bottom_y: f32,
    width: f32,
}

fn passive_vector_text_metrics(
    point: crate::layout::LayoutPoint,
    font_size: f32,
    text: &str,
    character_extra: f32,
    horizontal_align: StaticImageTextHorizontalAlign,
    vertical_align: StaticImageTextVerticalAlign,
) -> PassiveVectorTextMetrics {
    let text_width = estimated_passive_vector_text_width(text, font_size)
        + passive_vector_text_extra_width(text, character_extra);
    let x = match horizontal_align {
        StaticImageTextHorizontalAlign::Left => point.x,
        StaticImageTextHorizontalAlign::Center => point.x - (text_width / 2.0),
        StaticImageTextHorizontalAlign::Right => point.x - text_width,
    };
    let (baseline_y, top_y, bottom_y) = match vertical_align {
        StaticImageTextVerticalAlign::Top => {
            let baseline_y = point.y - font_size;
            (baseline_y, point.y, baseline_y - (font_size * 0.2))
        }
        StaticImageTextVerticalAlign::Baseline => (
            point.y,
            point.y + (font_size * 0.8),
            point.y - (font_size * 0.2),
        ),
        StaticImageTextVerticalAlign::Bottom => {
            let baseline_y = point.y + (font_size * 0.2);
            (baseline_y, baseline_y + (font_size * 0.8), point.y)
        }
    };
    PassiveVectorTextMetrics {
        x,
        baseline_y,
        top_y,
        bottom_y,
        width: text_width.max(font_size * 0.25),
    }
}

fn passive_vector_text_extra_width(text: &str, character_extra: f32) -> f32 {
    if character_extra == 0.0 {
        return 0.0;
    }
    let visible_count = text.chars().filter(|ch| !ch.is_control()).count();
    if visible_count <= 1 {
        0.0
    } else {
        character_extra * visible_count.saturating_sub(1) as f32
    }
}

fn estimated_passive_vector_text_width(text: &str, font_size: f32) -> f32 {
    text.chars()
        .map(|ch| {
            if ch.is_whitespace() {
                0.28
            } else if matches!(ch, 'i' | 'l' | 'I' | '.' | ',' | ':' | ';' | '!' | '|') {
                0.28
            } else if matches!(ch, 'm' | 'w' | 'M' | 'W') {
                0.85
            } else {
                0.55
            }
        })
        .sum::<f32>()
        * font_size
}

fn pdf_color_from_model(color: crate::model::Color) -> PdfColor {
    PdfColor {
        red: color.red as f32 / 255.0,
        green: color.green as f32 / 255.0,
        blue: color.blue as f32 / 255.0,
    }
}

fn draw_passive_image_placeholder(content: &mut Content, fragment: &crate::layout::ImageFragment) {
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
    if width < 96.0 || height < 24.0 {
        return;
    }

    let mut style = CharacterStyle::default();
    style.font_size_half_points = 18;
    let label = "Image skipped";
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
        &style,
        0.0,
        fragment.x + 6.0,
        fragment.y + (height / 2.0) - 3.0,
        &encoded,
        TextRenderingMode::Fill,
    );
}

fn is_stream_marker_at(pdf: &[u8], offset: usize) -> bool {
    pdf[offset..].starts_with(b"stream")
        && offset
            .checked_sub(1)
            .and_then(|idx| pdf.get(idx))
            .is_some_and(|byte| is_pdf_whitespace(*byte))
        && stream_marker_follows_dictionary(pdf, offset)
        && pdf
            .get(offset + b"stream".len())
            .is_some_and(|byte| matches!(*byte, b'\r' | b'\n'))
}

fn stream_marker_follows_dictionary(pdf: &[u8], stream_offset: usize) -> bool {
    let Some(first_trailing_whitespace) = stream_offset.checked_sub(1) else {
        return false;
    };
    let mut cursor = first_trailing_whitespace;
    while let Some(byte) = pdf.get(cursor).copied() {
        if !is_pdf_whitespace(byte) {
            break;
        }
        let Some(previous) = cursor.checked_sub(1) else {
            return false;
        };
        cursor = previous;
    }

    pdf.get(cursor).copied() == Some(b'>')
        && cursor.checked_sub(1).and_then(|idx| pdf.get(idx)).copied() == Some(b'>')
}

fn stream_data_start(pdf: &[u8], stream_offset: usize) -> Option<usize> {
    let after_marker = stream_offset.checked_add(b"stream".len())?;
    match pdf.get(after_marker).copied()? {
        b'\r' if pdf.get(after_marker + 1).copied() == Some(b'\n') => Some(after_marker + 2),
        b'\r' | b'\n' => Some(after_marker + 1),
        _ => None,
    }
}

enum DeclaredStreamEnd {
    Found(usize),
    Malformed,
}

fn stream_end_from_declared_length(
    pdf: &[u8],
    stream_offset: usize,
    stream_start: usize,
) -> Option<DeclaredStreamEnd> {
    let (dictionary_start, dictionary_end) = stream_dictionary_bounds(pdf, stream_offset)?;
    let declared_length = pdf_dictionary_direct_length(pdf, dictionary_start, dictionary_end)?;
    let Some(stream_data_end) = stream_start.checked_add(declared_length) else {
        return Some(DeclaredStreamEnd::Malformed);
    };
    let Some(endstream_offset) = stream_end_marker_after_declared_data(pdf, stream_data_end) else {
        return Some(DeclaredStreamEnd::Malformed);
    };
    Some(DeclaredStreamEnd::Found(endstream_offset))
}

fn stream_dictionary_bounds(pdf: &[u8], stream_offset: usize) -> Option<(usize, usize)> {
    let mut cursor = stream_offset.checked_sub(1)?;
    while is_pdf_whitespace(*pdf.get(cursor)?) {
        cursor = cursor.checked_sub(1)?;
    }
    if pdf.get(cursor).copied() != Some(b'>')
        || cursor.checked_sub(1).and_then(|idx| pdf.get(idx)).copied() != Some(b'>')
    {
        return None;
    }
    let dictionary_end = cursor + 1;
    let mut depth = 0usize;
    let mut idx = cursor.checked_sub(2)?;
    loop {
        if pdf.get(idx).copied() == Some(b'>')
            && idx
                .checked_sub(1)
                .and_then(|previous| pdf.get(previous))
                .copied()
                == Some(b'>')
        {
            depth = depth.checked_add(1)?;
            idx = idx.checked_sub(2)?;
            continue;
        }
        if pdf.get(idx).copied() == Some(b'<') && pdf.get(idx + 1).copied() == Some(b'<') {
            if depth == 0 {
                return Some((idx, dictionary_end));
            }
            depth -= 1;
            idx = idx.checked_sub(1)?;
            continue;
        }
        let Some(previous) = idx.checked_sub(1) else {
            break;
        };
        idx = previous;
    }
    None
}

fn pdf_dictionary_direct_length(
    pdf: &[u8],
    dictionary_start: usize,
    dictionary_end: usize,
) -> Option<usize> {
    let mut offset = dictionary_start.checked_add(2)?;
    let mut depth = 0usize;
    while offset < dictionary_end {
        if pdf.get(offset).copied() == Some(b'<') && pdf.get(offset + 1).copied() == Some(b'<') {
            depth = depth.checked_add(1)?;
            offset += 2;
            continue;
        }
        if pdf.get(offset).copied() == Some(b'>') && pdf.get(offset + 1).copied() == Some(b'>') {
            if depth == 0 {
                break;
            }
            depth -= 1;
            offset += 2;
            continue;
        }
        if depth == 0
            && pdf[offset] == b'/'
            && let Some(parsed_name) = parse_pdf_name_token(pdf, offset)
        {
            if parsed_name.name == b"/Length" {
                return parse_pdf_unsigned_integer(pdf, parsed_name.end, dictionary_end);
            }
            offset = parsed_name.end;
            continue;
        }
        offset += 1;
    }
    None
}

fn parse_pdf_unsigned_integer(pdf: &[u8], mut offset: usize, limit: usize) -> Option<usize> {
    while offset < limit && is_pdf_whitespace(*pdf.get(offset)?) {
        offset += 1;
    }
    let mut value = 0usize;
    let mut digits = 0usize;
    while offset < limit {
        let byte = *pdf.get(offset)?;
        if !byte.is_ascii_digit() {
            break;
        }
        value = value
            .checked_mul(10)?
            .checked_add(usize::from(byte - b'0'))?;
        digits += 1;
        offset += 1;
    }
    if digits == 0 { None } else { Some(value) }
}

fn stream_end_marker_after_declared_data(pdf: &[u8], stream_data_end: usize) -> Option<usize> {
    let mut marker_offset = stream_data_end;
    match pdf.get(marker_offset).copied() {
        Some(b'\r') if pdf.get(marker_offset + 1).copied() == Some(b'\n') => {
            marker_offset += 2;
        }
        Some(b'\r' | b'\n') => {
            marker_offset += 1;
        }
        _ => {}
    }
    if pdf
        .get(marker_offset..)
        .is_some_and(|tail| tail.starts_with(b"endstream"))
        && is_pdf_whitespace_or_eof(pdf.get(marker_offset + b"endstream".len()).copied())
    {
        Some(marker_offset)
    } else {
        None
    }
}

fn find_endstream_marker(pdf: &[u8], stream_start: usize) -> Option<usize> {
    let mut offset = stream_start;
    while offset < pdf.len() {
        if pdf[offset..].starts_with(b"endstream")
            && offset
                .checked_sub(1)
                .and_then(|idx| pdf.get(idx))
                .is_some_and(|byte| is_pdf_whitespace(*byte))
            && is_pdf_whitespace_or_eof(pdf.get(offset + b"endstream".len()).copied())
        {
            return Some(offset);
        }
        offset += 1;
    }
    None
}

fn is_pdf_name_boundary(byte: Option<u8>) -> bool {
    byte.is_none_or(|byte| {
        is_pdf_whitespace(byte)
            || matches!(byte, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'/' | b'%')
    })
}

fn is_pdf_whitespace_or_eof(byte: Option<u8>) -> bool {
    byte.is_none_or(is_pdf_whitespace)
}

fn is_pdf_whitespace(byte: u8) -> bool {
    matches!(byte, b'\0' | b'\t' | b'\n' | b'\x0c' | b'\r' | b' ')
}

fn font_name_for_style(family: PdfFontFamily, style: &CharacterStyle) -> Name<'static> {
    Name(font_resource_for_style(family, style))
}

fn font_resource_for_style(family: PdfFontFamily, style: &CharacterStyle) -> &'static [u8] {
    match (family, style.bold, style.italic) {
        (PdfFontFamily::Helvetica, true, true) => HELVETICA_BOLD_ITALIC,
        (PdfFontFamily::Helvetica, true, false) => HELVETICA_BOLD,
        (PdfFontFamily::Helvetica, false, true) => HELVETICA_ITALIC,
        (PdfFontFamily::Helvetica, false, false) => HELVETICA_REGULAR,
        (PdfFontFamily::Courier, true, true) => COURIER_BOLD_ITALIC,
        (PdfFontFamily::Courier, true, false) => COURIER_BOLD,
        (PdfFontFamily::Courier, false, true) => COURIER_ITALIC,
        (PdfFontFamily::Courier, false, false) => COURIER_REGULAR,
        (PdfFontFamily::Times, true, true) => TIMES_BOLD_ITALIC,
        (PdfFontFamily::Times, true, false) => TIMES_BOLD,
        (PdfFontFamily::Times, false, true) => TIMES_ITALIC,
        (PdfFontFamily::Times, false, false) => TIMES_REGULAR,
        (PdfFontFamily::Symbol, _, _) => SYMBOL_REGULAR,
        (PdfFontFamily::ZapfDingbats, _, _) => ZAPF_DINGBATS_REGULAR,
    }
}

fn passive_to_unicode_cmap(name: &'static [u8], mappings: &[(u8, char)]) -> Vec<u8> {
    let mut cmap = UnicodeCmap::<u8>::new(
        Name(name),
        SystemInfo {
            registry: Str(b"Adobe"),
            ordering: Str(b"UCS"),
            supplement: 0,
        },
    );
    for (glyph, unicode) in mappings {
        cmap.pair(*glyph, *unicode);
    }
    cmap.finish().into_vec()
}

fn write_supplied_pdf_fonts(
    pdf: &mut Pdf,
    font_provider: Option<&FontProvider>,
    supplied_fonts: &[SuppliedPdfFont],
) {
    let Some(font_provider) = font_provider else {
        return;
    };
    for supplied in supplied_fonts {
        let Some(asset) = font_provider.assets.get(supplied.asset_index) else {
            continue;
        };
        let Ok(face) = Face::parse(&asset.bytes, 0) else {
            continue;
        };
        let system_info = supplied_cid_system_info();

        pdf.type0_font(supplied.type0_ref)
            .base_font(Name(&supplied.base_name))
            .encoding_predefined(Name(b"Identity-H"))
            .descendant_font(supplied.cid_ref)
            .to_unicode(supplied.to_unicode_ref);

        {
            let mut cid_font = pdf.cid_font(supplied.cid_ref);
            cid_font
                .subtype(CidFontType::Type2)
                .base_font(Name(&supplied.base_name))
                .system_info(system_info)
                .font_descriptor(supplied.descriptor_ref)
                .cid_to_gid_map_predefined(Name(b"Identity"));
            if let Some(default_width) = supplied.used_glyphs.first().map(|glyph| glyph.width) {
                cid_font.default_width(default_width);
            }
            {
                let mut widths = cid_font.widths();
                for glyph in &supplied.used_glyphs {
                    widths.same(glyph.cid, glyph.cid, glyph.width);
                }
            }
        }

        let units_per_em = f32::from(face.units_per_em()).max(1.0);
        let scale = |value: i16| f32::from(value) * 1000.0 / units_per_em;
        let bbox = face.global_bounding_box();
        let mut flags = FontFlags::NON_SYMBOLIC;
        if asset.style.italic {
            flags |= FontFlags::ITALIC;
        }
        if asset.style.bold {
            flags |= FontFlags::FORCE_BOLD;
        }
        pdf.font_descriptor(supplied.descriptor_ref)
            .name(Name(&supplied.base_name))
            .flags(flags)
            .bbox(Rect::new(
                scale(bbox.x_min),
                scale(bbox.y_min),
                scale(bbox.x_max),
                scale(bbox.y_max),
            ))
            .italic_angle(if asset.style.italic { -12.0 } else { 0.0 })
            .ascent(scale(face.ascender()))
            .descent(scale(face.descender()))
            .cap_height(scale(face.ascender()))
            .stem_v(if asset.style.bold { 120.0 } else { 80.0 })
            .font_file2(supplied.font_file_ref);

        {
            let mut font_file = pdf.stream(supplied.font_file_ref, &asset.bytes);
            font_file.pair(Name(b"Length1"), asset.bytes.len() as i32);
        }

        let mut cmap =
            UnicodeCmap::<u16>::new(Name(&supplied.base_name), supplied_cid_system_info());
        for glyph in &supplied.used_glyphs {
            cmap.pair(glyph.cid, glyph.unicode);
        }
        pdf.stream(supplied.to_unicode_ref, &cmap.finish());
    }
}

fn supplied_cid_system_info() -> SystemInfo<'static> {
    SystemInfo {
        registry: Str(b"Adobe"),
        ordering: Str(b"Identity"),
        supplement: 0,
    }
}

fn draw_underline(
    content: &mut Content,
    x: f32,
    y: f32,
    width: f32,
    color: PdfColor,
    style: UnderlineStyle,
) {
    if width <= 0.0 || style == UnderlineStyle::None {
        return;
    }

    content.save_state();
    set_stroke_color(content, color);

    match style {
        UnderlineStyle::None => {}
        UnderlineStyle::Single | UnderlineStyle::Words => {
            stroke_line(content, x, y, x + width, y, 0.5)
        }
        UnderlineStyle::Double => {
            stroke_line(content, x, y, x + width, y, 0.5);
            stroke_line(content, x, y - 2.0, x + width, y - 2.0, 0.5);
        }
        UnderlineStyle::Thick => stroke_line(content, x, y, x + width, y, 1.2),
        UnderlineStyle::Dotted => {
            content.set_dash_pattern([0.5, 2.0], 0.0);
            stroke_line(content, x, y, x + width, y, 0.8);
        }
        UnderlineStyle::Dashed => {
            content.set_dash_pattern([3.0, 2.0], 0.0);
            stroke_line(content, x, y, x + width, y, 0.5);
        }
        UnderlineStyle::Wave => stroke_wave_underline(content, x, y, width),
    }

    content.restore_state();
}

#[allow(clippy::too_many_arguments)]
fn draw_passive_line(
    content: &mut Content,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    width: f32,
    color: PdfColor,
    style: LineStyle,
) {
    content.save_state();
    set_stroke_color(content, color);
    match style {
        LineStyle::Solid => stroke_line(content, x1, y1, x2, y2, width),
        LineStyle::Dotted => {
            let dot = width.max(0.5);
            content.set_dash_pattern([dot, dot * 2.0], 0.0);
            stroke_line(content, x1, y1, x2, y2, width);
        }
        LineStyle::Dashed => {
            let dash = (width * 3.0).max(3.0);
            content.set_dash_pattern([dash, dash * 0.75], 0.0);
            stroke_line(content, x1, y1, x2, y2, width);
        }
        LineStyle::Double => draw_double_line(content, x1, y1, x2, y2, width),
        LineStyle::Wavy => stroke_wave_line(content, x1, y1, x2, y2, width),
    }
    content.restore_state();
}

fn set_passive_path_stroke_style(content: &mut Content, width: f32, style: LineStyle) {
    match style {
        LineStyle::Solid | LineStyle::Double | LineStyle::Wavy => {}
        LineStyle::Dotted => {
            let dot = width.max(0.5);
            content.set_dash_pattern([dot, dot * 2.0], 0.0);
        }
        LineStyle::Dashed => {
            let dash = (width * 3.0).max(3.0);
            content.set_dash_pattern([dash, dash * 0.75], 0.0);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_passive_ellipse(
    content: &mut Content,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    stroke_width: f32,
    stroke_color: PdfColor,
    stroke_style: LineStyle,
    fill_color: Option<PdfColor>,
) {
    if stroke_width <= 0.0 && fill_color.is_none() {
        return;
    }

    let has_stroke = stroke_width > 0.0;
    content.save_state();
    if has_stroke {
        content.set_line_width(stroke_width.max(0.25));
        set_stroke_color(content, stroke_color);
        set_passive_path_stroke_style(content, stroke_width, stroke_style);
    }
    if let Some(fill_color) = fill_color {
        set_fill_color(content, fill_color);
    }
    append_passive_ellipse_path(content, x, y, width, height);
    if fill_color.is_some() && has_stroke {
        content.fill_nonzero_and_stroke();
    } else if fill_color.is_some() {
        content.fill_nonzero();
    } else {
        content.stroke();
    }
    content.restore_state();
}

#[allow(clippy::too_many_arguments)]
fn draw_passive_rounded_rectangle(
    content: &mut Content,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    stroke_width: f32,
    stroke_color: PdfColor,
    stroke_style: LineStyle,
    fill_color: Option<PdfColor>,
) {
    if stroke_width <= 0.0 && fill_color.is_none() {
        return;
    }

    let has_stroke = stroke_width > 0.0;
    content.save_state();
    if has_stroke {
        content.set_line_width(stroke_width.max(0.25));
        set_stroke_color(content, stroke_color);
        set_passive_path_stroke_style(content, stroke_width, stroke_style);
    }
    if let Some(fill_color) = fill_color {
        set_fill_color(content, fill_color);
    }
    append_passive_rounded_rectangle_path(content, x, y, width, height, radius);
    if fill_color.is_some() && has_stroke {
        content.fill_nonzero_and_stroke();
    } else if fill_color.is_some() {
        content.fill_nonzero();
    } else {
        content.stroke();
    }
    content.restore_state();
}

fn append_passive_ellipse_path(content: &mut Content, x: f32, y: f32, width: f32, height: f32) {
    const KAPPA: f32 = 0.552_284_8;

    let width = width.max(0.1);
    let height = height.max(0.1);
    let rx = width / 2.0;
    let ry = height / 2.0;
    let cx = x + rx;
    let cy = y + ry;
    let dx = rx * KAPPA;
    let dy = ry * KAPPA;

    content.move_to(cx + rx, cy);
    content.cubic_to(cx + rx, cy + dy, cx + dx, cy + ry, cx, cy + ry);
    content.cubic_to(cx - dx, cy + ry, cx - rx, cy + dy, cx - rx, cy);
    content.cubic_to(cx - rx, cy - dy, cx - dx, cy - ry, cx, cy - ry);
    content.cubic_to(cx + dx, cy - ry, cx + rx, cy - dy, cx + rx, cy);
    content.close_path();
}

fn append_passive_rounded_rectangle_path(
    content: &mut Content,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
) {
    const KAPPA: f32 = 0.552_284_8;

    let width = width.max(0.1);
    let height = height.max(0.1);
    let radius = radius.clamp(0.1, width.min(height) / 2.0);
    let control = radius * KAPPA;
    let right = x + width;
    let top = y + height;

    content.move_to(x + radius, y);
    content.line_to(right - radius, y);
    content.cubic_to(
        right - radius + control,
        y,
        right,
        y + radius - control,
        right,
        y + radius,
    );
    content.line_to(right, top - radius);
    content.cubic_to(
        right,
        top - radius + control,
        right - radius + control,
        top,
        right - radius,
        top,
    );
    content.line_to(x + radius, top);
    content.cubic_to(
        x + radius - control,
        top,
        x,
        top - radius + control,
        x,
        top - radius,
    );
    content.line_to(x, y + radius);
    content.cubic_to(
        x,
        y + radius - control,
        x + radius - control,
        y,
        x + radius,
        y,
    );
    content.close_path();
}

fn append_passive_polygon_path(content: &mut Content, points: &[crate::layout::LayoutPoint]) {
    let Some(first) = points.first() else {
        return;
    };
    content.move_to(first.x, first.y);
    for point in points.iter().skip(1) {
        content.line_to(point.x, point.y);
    }
    content.close_path();
}

fn vector_points_bounds(points: &[crate::layout::LayoutPoint]) -> Option<VectorDrawRect> {
    let first = points.first()?;
    let mut min_x = first.x;
    let mut max_x = first.x;
    let mut min_y = first.y;
    let mut max_y = first.y;
    for point in points.iter().skip(1) {
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
    }
    Some(VectorDrawRect {
        x: min_x,
        y: min_y,
        width: (max_x - min_x).max(0.1),
        height: (max_y - min_y).max(0.1),
    })
}

fn draw_passive_polygon(
    content: &mut Content,
    points: &[crate::layout::LayoutPoint],
    stroke_width: f32,
    stroke_color: PdfColor,
    stroke_style: LineStyle,
    fill_rule: StaticImageVectorFillRule,
    fill_color: Option<PdfColor>,
) {
    let Some(first) = points.first() else {
        return;
    };
    if stroke_width <= 0.0 && fill_color.is_none() {
        return;
    }

    let has_stroke = stroke_width > 0.0;
    content.save_state();
    if has_stroke {
        content.set_line_width(stroke_width.max(0.25));
        set_stroke_color(content, stroke_color);
        set_passive_path_stroke_style(content, stroke_width, stroke_style);
    }
    if let Some(fill_color) = fill_color {
        set_fill_color(content, fill_color);
    }
    content.move_to(first.x, first.y);
    for point in points.iter().skip(1) {
        content.line_to(point.x, point.y);
    }
    content.close_path();
    if fill_color.is_some() && has_stroke {
        match fill_rule {
            StaticImageVectorFillRule::Alternate => {
                content.fill_even_odd_and_stroke();
            }
            StaticImageVectorFillRule::Winding => {
                content.fill_nonzero_and_stroke();
            }
        }
    } else if fill_color.is_some() {
        match fill_rule {
            StaticImageVectorFillRule::Alternate => {
                content.fill_even_odd();
            }
            StaticImageVectorFillRule::Winding => {
                content.fill_nonzero();
            }
        }
    } else {
        content.stroke();
    }
    content.restore_state();
}

fn draw_passive_text_overlays(content: &mut Content, fragment: &TextFragment) {
    draw_passive_emphasis_marks(content, fragment);
    draw_passive_checkbox_overlays(content, fragment);
    draw_passive_dingbat_overlays(content, fragment);
}

fn text_fragment_renders_as_passive_vector_only(fragment: &TextFragment) -> bool {
    if fragment.font_family != PdfFontFamily::ZapfDingbats
        || fragment.style.shadow
        || fragment.style.outline
        || fragment.style.relief != TextRelief::None
        || fragment.text.is_empty()
    {
        return false;
    }

    let mut has_passive_content = false;
    for ch in fragment
        .text
        .chars()
        .filter(|ch| !is_zero_width_pdf_char(*ch))
    {
        if ch.is_whitespace() {
            has_passive_content = true;
            continue;
        }
        if is_passive_vector_only_dingbat_char(ch) {
            has_passive_content = true;
        } else {
            return false;
        }
    }
    has_passive_content
}

fn is_passive_vector_only_dingbat_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{25a1}'
            | '\u{2610}'
            | '\u{2611}'
            | '\u{2612}'
            | '\u{263a}'
            | '\u{2713}'
            | '\u{2714}'
            | '\u{2717}'
            | '\u{2751}'
    )
}

fn draw_passive_vector_only_dingbat_text(content: &mut Content, fragment: &TextFragment) {
    let font_size = fragment.style.font_size_points();
    let horizontal_scale = fragment.style.horizontal_scale();
    let character_spacing = twips_to_points(fragment.style.character_spacing_twips);
    let visible_count = fragment
        .text
        .chars()
        .filter(|ch| !is_zero_width_pdf_char(*ch))
        .count();
    let mut visible_index = 0usize;
    let mut cursor = fragment.x;

    content.save_state();
    set_stroke_color(content, fragment.color);
    set_fill_color(content, fragment.color);
    content.set_line_width((font_size * 0.075).clamp(0.5, 1.25));
    for ch in fragment.text.chars() {
        match ch {
            '\u{25a1}' | '\u{2610}' | '\u{2751}' => {
                draw_passive_checkbox_box(content, cursor, fragment.baseline_y, font_size)
            }
            '\u{2611}' => {
                draw_passive_checkbox_box(content, cursor, fragment.baseline_y, font_size);
                draw_passive_checkbox_tick(content, cursor, fragment.baseline_y, font_size);
            }
            '\u{2612}' => {
                draw_passive_checkbox_box(content, cursor, fragment.baseline_y, font_size);
                draw_passive_checkbox_x(content, cursor, fragment.baseline_y, font_size);
            }
            '\u{2713}' | '\u{2714}' => {
                draw_passive_checkbox_tick(content, cursor, fragment.baseline_y, font_size)
            }
            '\u{2717}' => draw_passive_checkbox_x(content, cursor, fragment.baseline_y, font_size),
            '\u{263a}' => draw_passive_smiley(content, cursor, fragment.baseline_y, font_size),
            _ => {}
        }
        if !is_zero_width_pdf_char(ch) {
            visible_index += 1;
            let mut advance = pdf_base_glyph_advance(ch, fragment.font_family, &fragment.style);
            if ch == ' ' || ch == '\u{00a0}' {
                advance += fragment.word_spacing;
            }
            if visible_index < visible_count {
                advance += character_spacing;
            }
            cursor += advance * horizontal_scale;
        }
    }
    content.restore_state();
}

fn draw_passive_emphasis_marks(content: &mut Content, fragment: &TextFragment) {
    if fragment.style.emphasis_mark == CharacterEmphasisMark::None {
        return;
    }

    let font_size = fragment.style.font_size_points();
    let horizontal_scale = fragment.style.horizontal_scale();
    let character_spacing = twips_to_points(fragment.style.character_spacing_twips);
    let visible_count = fragment
        .text
        .chars()
        .filter(|ch| !is_zero_width_pdf_char(*ch))
        .count();
    if visible_count == 0 {
        return;
    }

    let mut visible_index = 0usize;
    let mut cursor = fragment.x;

    content.save_state();
    set_fill_color(content, fragment.color);
    set_stroke_color(content, fragment.color);
    content.set_line_width((font_size * 0.06).clamp(0.35, 1.0));
    for ch in fragment.text.chars() {
        if !is_zero_width_pdf_char(ch) {
            visible_index += 1;
        }
        if !is_zero_width_pdf_char(ch) && !ch.is_whitespace() {
            let advance = pdf_base_glyph_advance(ch, fragment.font_family, &fragment.style);
            let center_x = cursor + (advance * horizontal_scale * 0.5);
            draw_passive_emphasis_mark(
                content,
                fragment.style.emphasis_mark,
                center_x,
                fragment.baseline_y,
                font_size,
            );
        }
        if !is_zero_width_pdf_char(ch) {
            let mut advance = pdf_base_glyph_advance(ch, fragment.font_family, &fragment.style);
            if ch == ' ' || ch == '\u{00a0}' {
                advance += fragment.word_spacing;
            }
            if visible_index < visible_count {
                advance += character_spacing;
            }
            cursor += advance * horizontal_scale;
        }
    }
    content.restore_state();
}

fn draw_passive_emphasis_mark(
    content: &mut Content,
    mark: CharacterEmphasisMark,
    center_x: f32,
    baseline_y: f32,
    font_size: f32,
) {
    let mark_y = baseline_y + font_size * 0.82;
    match mark {
        CharacterEmphasisMark::None => {}
        CharacterEmphasisMark::Dot => {
            let radius = (font_size * 0.065).clamp(0.45, 1.25);
            content.rect(
                center_x - radius,
                mark_y - radius,
                radius * 2.0,
                radius * 2.0,
            );
            content.fill_nonzero();
        }
        CharacterEmphasisMark::Comma => {
            let height = (font_size * 0.22).clamp(1.0, 4.0);
            content.move_to(center_x, mark_y);
            content.line_to(center_x - height * 0.25, mark_y - height);
            content.stroke();
        }
    }
}

fn draw_passive_checkbox_overlays(content: &mut Content, fragment: &TextFragment) {
    if fragment.font_family != PdfFontFamily::ZapfDingbats
        || (!fragment.text.contains('\u{2611}') && !fragment.text.contains('\u{2612}'))
    {
        return;
    }

    let font_size = fragment.style.font_size_points();
    let horizontal_scale = fragment.style.horizontal_scale();
    let character_spacing = twips_to_points(fragment.style.character_spacing_twips);
    let visible_count = fragment
        .text
        .chars()
        .filter(|ch| !is_zero_width_pdf_char(*ch))
        .count();
    let mut visible_index = 0usize;
    let mut cursor = fragment.x;

    content.save_state();
    set_stroke_color(content, fragment.color);
    content.set_line_width((font_size * 0.075).clamp(0.5, 1.25));
    for ch in fragment.text.chars() {
        match ch {
            '\u{2611}' => {
                draw_passive_checkbox_tick(content, cursor, fragment.baseline_y, font_size)
            }
            '\u{2612}' => draw_passive_checkbox_x(content, cursor, fragment.baseline_y, font_size),
            _ => {}
        }
        if !is_zero_width_pdf_char(ch) {
            visible_index += 1;
            let mut advance = pdf_base_glyph_advance(ch, fragment.font_family, &fragment.style);
            if ch == ' ' || ch == '\u{00a0}' {
                advance += fragment.word_spacing;
            }
            if visible_index < visible_count {
                advance += character_spacing;
            }
            cursor += advance * horizontal_scale;
        }
    }
    content.restore_state();
}

fn draw_passive_checkbox_tick(
    content: &mut Content,
    glyph_x: f32,
    baseline_y: f32,
    font_size: f32,
) {
    let box_size = font_size * 0.62;
    let left = glyph_x + font_size * 0.04;
    let bottom = baseline_y + font_size * 0.04;

    content.move_to(left + box_size * 0.22, bottom + box_size * 0.48);
    content.line_to(left + box_size * 0.42, bottom + box_size * 0.25);
    content.line_to(left + box_size * 0.82, bottom + box_size * 0.78);
    content.stroke();
}

fn draw_passive_checkbox_box(content: &mut Content, glyph_x: f32, baseline_y: f32, font_size: f32) {
    let box_size = font_size * 0.62;
    let left = glyph_x + font_size * 0.04;
    let bottom = baseline_y + font_size * 0.04;

    content.rect(left, bottom, box_size, box_size);
    content.stroke();
}

fn draw_passive_checkbox_x(content: &mut Content, glyph_x: f32, baseline_y: f32, font_size: f32) {
    let box_size = font_size * 0.62;
    let left = glyph_x + font_size * 0.04;
    let bottom = baseline_y + font_size * 0.04;

    content.move_to(left + box_size * 0.24, bottom + box_size * 0.24);
    content.line_to(left + box_size * 0.76, bottom + box_size * 0.76);
    content.move_to(left + box_size * 0.76, bottom + box_size * 0.24);
    content.line_to(left + box_size * 0.24, bottom + box_size * 0.76);
    content.stroke();
}

fn draw_passive_dingbat_overlays(content: &mut Content, fragment: &TextFragment) {
    if fragment.font_family != PdfFontFamily::ZapfDingbats || !fragment.text.contains('\u{263a}') {
        return;
    }

    let font_size = fragment.style.font_size_points();
    let horizontal_scale = fragment.style.horizontal_scale();
    let character_spacing = twips_to_points(fragment.style.character_spacing_twips);
    let visible_count = fragment
        .text
        .chars()
        .filter(|ch| !is_zero_width_pdf_char(*ch))
        .count();
    let mut visible_index = 0usize;
    let mut cursor = fragment.x;

    content.save_state();
    set_stroke_color(content, fragment.color);
    set_fill_color(content, fragment.color);
    content.set_line_width((font_size * 0.055).clamp(0.45, 1.0));
    for ch in fragment.text.chars() {
        if ch == '\u{263a}' {
            draw_passive_smiley(content, cursor, fragment.baseline_y, font_size);
        }
        if !is_zero_width_pdf_char(ch) {
            visible_index += 1;
            let mut advance = pdf_base_glyph_advance(ch, fragment.font_family, &fragment.style);
            if ch == ' ' || ch == '\u{00a0}' {
                advance += fragment.word_spacing;
            }
            if visible_index < visible_count {
                advance += character_spacing;
            }
            cursor += advance * horizontal_scale;
        }
    }
    content.restore_state();
}

fn draw_passive_smiley(content: &mut Content, glyph_x: f32, baseline_y: f32, font_size: f32) {
    let radius = font_size * 0.28;
    let center_x = glyph_x + font_size * 0.27;
    let center_y = baseline_y + font_size * 0.34;
    draw_passive_circle(content, center_x, center_y, radius);

    let eye_radius = (font_size * 0.035).clamp(0.25, 0.7);
    draw_passive_filled_circle(
        content,
        center_x - radius * 0.35,
        center_y + radius * 0.25,
        eye_radius,
    );
    draw_passive_filled_circle(
        content,
        center_x + radius * 0.35,
        center_y + radius * 0.25,
        eye_radius,
    );

    content.move_to(center_x - radius * 0.48, center_y - radius * 0.12);
    content.cubic_to(
        center_x - radius * 0.26,
        center_y - radius * 0.48,
        center_x + radius * 0.26,
        center_y - radius * 0.48,
        center_x + radius * 0.48,
        center_y - radius * 0.12,
    );
    content.stroke();
}

fn draw_passive_circle(content: &mut Content, cx: f32, cy: f32, radius: f32) {
    add_passive_circle_path(content, cx, cy, radius);
    content.stroke();
}

fn draw_passive_filled_circle(content: &mut Content, cx: f32, cy: f32, radius: f32) {
    add_passive_circle_path(content, cx, cy, radius);
    content.fill_nonzero();
}

fn add_passive_circle_path(content: &mut Content, cx: f32, cy: f32, radius: f32) {
    const KAPPA: f32 = 0.552_284_8;
    let control = radius * KAPPA;
    content.move_to(cx + radius, cy);
    content.cubic_to(
        cx + radius,
        cy + control,
        cx + control,
        cy + radius,
        cx,
        cy + radius,
    );
    content.cubic_to(
        cx - control,
        cy + radius,
        cx - radius,
        cy + control,
        cx - radius,
        cy,
    );
    content.cubic_to(
        cx - radius,
        cy - control,
        cx - control,
        cy - radius,
        cx,
        cy - radius,
    );
    content.cubic_to(
        cx + control,
        cy - radius,
        cx + radius,
        cy - control,
        cx + radius,
        cy,
    );
    content.close_path();
}

fn pdf_base_glyph_advance(ch: char, family: PdfFontFamily, style: &CharacterStyle) -> f32 {
    let size = style.font_size_points();
    match family {
        PdfFontFamily::Courier => size * 0.6,
        PdfFontFamily::Times if matches!(ch, 'i' | 'l' | '!' | '.' | ',' | ':' | ';' | '\'') => {
            size * 0.22
        }
        PdfFontFamily::Helvetica
            if matches!(ch, 'i' | 'l' | '!' | '.' | ',' | ':' | ';' | '\'') =>
        {
            size * 0.25
        }
        PdfFontFamily::Times if matches!(ch, 'm' | 'w' | 'M' | 'W') => size * 0.72,
        PdfFontFamily::Helvetica if matches!(ch, 'm' | 'w' | 'M' | 'W') => size * 0.78,
        _ if matches!(ch, ' ' | '\u{00a0}') => size * 0.28,
        _ if is_zero_width_pdf_char(ch) => 0.0,
        PdfFontFamily::Helvetica if style.bold => size * 0.56,
        PdfFontFamily::Helvetica => size * 0.52,
        PdfFontFamily::Times if style.bold => size * 0.54,
        PdfFontFamily::Times => size * 0.48,
        PdfFontFamily::Symbol | PdfFontFamily::ZapfDingbats => size * 0.54,
    }
}

fn is_zero_width_pdf_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{00ad}' | '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{200e}' | '\u{200f}' | '\u{feff}'
    )
}

fn draw_double_line(content: &mut Content, x1: f32, y1: f32, x2: f32, y2: f32, width: f32) {
    let stroke_width = (width * 0.35).clamp(0.25, width.max(0.25));
    let offset = (width * 0.55).max(1.0);
    if (y1 - y2).abs() < 0.01 {
        stroke_line(content, x1, y1 + offset, x2, y2 + offset, stroke_width);
        stroke_line(content, x1, y1 - offset, x2, y2 - offset, stroke_width);
    } else if (x1 - x2).abs() < 0.01 {
        stroke_line(content, x1 + offset, y1, x2 + offset, y2, stroke_width);
        stroke_line(content, x1 - offset, y1, x2 - offset, y2, stroke_width);
    } else {
        stroke_line(content, x1, y1, x2, y2, width);
    }
}

fn stroke_line(content: &mut Content, x1: f32, y1: f32, x2: f32, y2: f32, width: f32) {
    content.set_line_width(width);
    content.move_to(x1, y1);
    content.line_to(x2, y2);
    content.stroke();
}

fn stroke_wave_underline(content: &mut Content, x: f32, y: f32, width: f32) {
    content.set_line_width(0.5);
    let step = 2.0;
    let amplitude = 1.0;
    let mut cursor = 0.0;
    content.move_to(x, y);
    while cursor < width {
        let next = (cursor + step).min(width);
        let segment_index = (cursor / step) as i32;
        let next_y = if segment_index % 2 == 0 {
            y - amplitude
        } else {
            y + amplitude
        };
        content.line_to(x + next, next_y);
        cursor = next;
    }
    content.stroke();
}

fn stroke_wave_line(content: &mut Content, x1: f32, y1: f32, x2: f32, y2: f32, width: f32) {
    content.set_line_width(width.max(0.5));
    let step = (width * 2.0).clamp(2.0, 6.0);
    let amplitude = width.clamp(1.0, 3.0);

    if (y1 - y2).abs() < 0.01 {
        let direction = if x2 >= x1 { 1.0 } else { -1.0 };
        let total = (x2 - x1).abs();
        let mut cursor = 0.0;
        content.move_to(x1, y1);
        while cursor < total {
            let next = (cursor + step).min(total);
            let segment_index = (cursor / step) as i32;
            let next_y = if segment_index % 2 == 0 {
                y1 - amplitude
            } else {
                y1 + amplitude
            };
            content.line_to(x1 + (next * direction), next_y);
            cursor = next;
        }
        content.stroke();
    } else if (x1 - x2).abs() < 0.01 {
        let direction = if y2 >= y1 { 1.0 } else { -1.0 };
        let total = (y2 - y1).abs();
        let mut cursor = 0.0;
        content.move_to(x1, y1);
        while cursor < total {
            let next = (cursor + step).min(total);
            let segment_index = (cursor / step) as i32;
            let next_x = if segment_index % 2 == 0 {
                x1 - amplitude
            } else {
                x1 + amplitude
            };
            content.line_to(next_x, y1 + (next * direction));
            cursor = next;
        }
        content.stroke();
    } else {
        stroke_line(content, x1, y1, x2, y2, width);
    }
}

fn encode_pdf_text(text: &str) -> Vec<u8> {
    text.chars().map(encode_win_ansi_char).collect()
}

fn encode_pdf_text_for_font(text: &str, family: PdfFontFamily) -> Vec<u8> {
    match family {
        PdfFontFamily::Symbol => text.chars().map(encode_symbol_char).collect(),
        PdfFontFamily::ZapfDingbats => text.chars().map(encode_zapf_dingbats_char).collect(),
        PdfFontFamily::Helvetica | PdfFontFamily::Courier | PdfFontFamily::Times => {
            encode_pdf_text(text)
        }
    }
}

fn encode_symbol_char(ch: char) -> u8 {
    match ch {
        '\u{0391}' => b'A',
        '\u{0392}' => b'B',
        '\u{03a7}' => b'C',
        '\u{0394}' => b'D',
        '\u{0395}' => b'E',
        '\u{03a6}' => b'F',
        '\u{0393}' => b'G',
        '\u{0397}' => b'H',
        '\u{0399}' => b'I',
        '\u{039a}' => b'K',
        '\u{039b}' => b'L',
        '\u{039c}' => b'M',
        '\u{039d}' => b'N',
        '\u{039f}' => b'O',
        '\u{03a0}' => b'P',
        '\u{0398}' => b'Q',
        '\u{03a1}' => b'R',
        '\u{03a3}' => b'S',
        '\u{03a4}' => b'T',
        '\u{03a5}' => b'U',
        '\u{03a9}' => b'W',
        '\u{039e}' => b'X',
        '\u{03a8}' => b'Y',
        '\u{0396}' => b'Z',
        '\u{03b1}' => b'a',
        '\u{03b2}' => b'b',
        '\u{03c7}' => b'c',
        '\u{03b4}' => b'd',
        '\u{03b5}' => b'e',
        '\u{03c6}' => b'f',
        '\u{03b3}' => b'g',
        '\u{03b7}' => b'h',
        '\u{03b9}' => b'i',
        '\u{03d5}' => b'j',
        '\u{03ba}' => b'k',
        '\u{03bb}' => b'l',
        '\u{03bc}' => b'm',
        '\u{03bd}' => b'n',
        '\u{03bf}' => b'o',
        '\u{03c0}' => b'p',
        '\u{03b8}' => b'q',
        '\u{03c1}' => b'r',
        '\u{03c3}' => b's',
        '\u{03c4}' => b't',
        '\u{03c5}' => b'u',
        '\u{03d6}' => b'v',
        '\u{03c9}' => b'w',
        '\u{03be}' => b'x',
        '\u{03c8}' => b'y',
        '\u{03b6}' => b'z',
        '\u{2200}' => b'"',
        '\u{2203}' => b'$',
        '\u{220b}' => b'\'',
        '\u{2217}' => b'*',
        '\u{2245}' => b'@',
        '\u{2206}' => b'D',
        '\u{03d1}' => b'J',
        '\u{03c2}' => b'V',
        '\u{2126}' => b'W',
        '\u{00b5}' => b'm',
        '\u{2234}' => b'\\',
        '\u{22a5}' => b'^',
        '\u{223c}' => b'~',
        '\u{03d2}' => 0xa1,
        '\u{2032}' => 0xa2,
        '\u{2022}' => 0xb7,
        '\u{2264}' => 0xa3,
        '\u{2044}' | '\u{2215}' => 0xa4,
        '\u{221e}' => 0xa5,
        '\u{0192}' => 0xa6,
        '\u{2663}' => 0xa7,
        '\u{2666}' => 0xa8,
        '\u{2665}' => 0xa9,
        '\u{2660}' => 0xaa,
        '\u{2194}' => 0xab,
        '\u{00b0}' => 0xb0,
        '\u{00b1}' => 0xb1,
        '\u{2033}' => 0xb2,
        '\u{2265}' => 0xb3,
        '\u{00d7}' => 0xb4,
        '\u{221d}' => 0xb5,
        '\u{2202}' => 0xb6,
        '\u{2212}' => 0x2d,
        '\u{00f7}' => 0xb8,
        '\u{2260}' => 0xb9,
        '\u{2261}' => 0xba,
        '\u{2248}' => 0xbb,
        '\u{2026}' => 0xbc,
        '\u{21b5}' => 0xbf,
        '\u{2135}' => 0xc0,
        '\u{2111}' => 0xc1,
        '\u{211c}' => 0xc2,
        '\u{2118}' => 0xc3,
        '\u{2297}' => 0xc4,
        '\u{2295}' => 0xc5,
        '\u{2205}' => 0xc6,
        '\u{2229}' => 0xc7,
        '\u{222a}' => 0xc8,
        '\u{2283}' => 0xc9,
        '\u{2287}' => 0xca,
        '\u{2284}' => 0xcb,
        '\u{2282}' => 0xcc,
        '\u{2286}' => 0xcd,
        '\u{2208}' => 0xce,
        '\u{2209}' => 0xcf,
        '\u{2220}' => 0xd0,
        '\u{2207}' => 0xd1,
        '\u{00ae}' => 0xd2,
        '\u{00a9}' => 0xd3,
        '\u{2122}' => 0xd4,
        '\u{220f}' => 0xd5,
        '\u{2211}' => 0xe5,
        '\u{221a}' => 0xd6,
        '\u{22c5}' => 0xd7,
        '\u{00ac}' => 0xd8,
        '\u{2227}' => 0xd9,
        '\u{2228}' => 0xda,
        '\u{21d4}' => 0xdb,
        '\u{21d0}' => 0xdc,
        '\u{21d1}' => 0xdd,
        '\u{21d2}' => 0xde,
        '\u{21d3}' => 0xdf,
        '\u{25ca}' => 0xe0,
        '\u{2329}' => 0xe1,
        '\u{232a}' => 0xf1,
        '\u{239b}' => 0xe6,
        '\u{239c}' => 0xe7,
        '\u{239d}' => 0xe8,
        '\u{23a1}' => 0xe9,
        '\u{23a2}' => 0xea,
        '\u{23a3}' => 0xeb,
        '\u{23a7}' => 0xec,
        '\u{23a8}' => 0xed,
        '\u{23a9}' => 0xee,
        '\u{23aa}' => 0xef,
        '\u{20ac}' => 0xf0,
        '\u{222b}' => 0xf2,
        '\u{2320}' => 0xf3,
        '\u{23af}' => 0xbe,
        '\u{2321}' => 0xf5,
        '\u{23ae}' => 0xf4,
        '\u{239e}' => 0xf7,
        '\u{239f}' => 0xf8,
        '\u{23a0}' => 0xf9,
        '\u{23a4}' => 0xfa,
        '\u{23a5}' => 0xfb,
        '\u{23a6}' => 0xfc,
        '\u{23ab}' => 0xfd,
        '\u{23ac}' => 0xfe,
        '\u{23ad}' => 0xff,
        '\u{23d0}' => 0xbd,
        '\u{2190}' => 0xac,
        '\u{2191}' => 0xad,
        '\u{2192}' => 0xae,
        '\u{2193}' => 0xaf,
        ' ' => b' ',
        ch if ch.is_ascii() => ch as u8,
        _ => b'?',
    }
}

fn encode_zapf_dingbats_char(ch: char) -> u8 {
    match ch {
        '\u{25a1}' | '\u{2610}' | '\u{2611}' | '\u{2612}' | '\u{2751}' => b'q',
        '\u{263a}' => b'J',
        '\u{2713}' | '\u{2714}' => b'3',
        '\u{2717}' => b'7',
        ' ' => b' ',
        ch if ch.is_ascii() => ch as u8,
        _ => b'?',
    }
}

fn encode_win_ansi_char(ch: char) -> u8 {
    match ch {
        '\u{0100}' => 0xc2,
        '\u{0101}' => 0xe2,
        '\u{0104}' => 0xc0,
        '\u{0105}' => 0xe0,
        '\u{0106}' => 0xc3,
        '\u{0107}' => 0xe3,
        '\u{010c}' => 0xc8,
        '\u{010d}' => 0xe8,
        '\u{0112}' => 0xc7,
        '\u{0113}' => 0xe7,
        '\u{0116}' => 0xcb,
        '\u{0117}' => 0xeb,
        '\u{0118}' => 0xc6,
        '\u{0119}' => 0xe6,
        '\u{0150}' => 0x81,
        '\u{0151}' => 0x8d,
        '\u{0170}' => 0x8f,
        '\u{0171}' => 0x90,
        '\u{011e}' => 0xd0,
        '\u{0130}' => 0xdd,
        '\u{015e}' => 0xde,
        '\u{011f}' => 0xf0,
        '\u{0131}' => 0xfd,
        '\u{015f}' => 0xfe,
        '\u{0122}' => 0xcc,
        '\u{0123}' => 0xec,
        '\u{012a}' => 0xce,
        '\u{012b}' => 0xee,
        '\u{012e}' => 0xc1,
        '\u{012f}' => 0xe1,
        '\u{0136}' => 0xcd,
        '\u{0137}' => 0xed,
        '\u{013b}' => 0xcf,
        '\u{013c}' => 0xef,
        '\u{0141}' => 0xd9,
        '\u{0142}' => 0xf9,
        '\u{0143}' => 0xd1,
        '\u{0144}' => 0xf1,
        '\u{0145}' => 0xd2,
        '\u{0146}' => 0xf2,
        '\u{014c}' => 0xd4,
        '\u{014d}' => 0xf4,
        '\u{0156}' => 0xaa,
        '\u{0157}' => 0xba,
        '\u{015a}' => 0xda,
        '\u{015b}' => 0xfa,
        '\u{016a}' => 0xdb,
        '\u{016b}' => 0xfb,
        '\u{0172}' => 0xd8,
        '\u{0173}' => 0xf8,
        '\u{0179}' => 0xca,
        '\u{017a}' => 0xea,
        '\u{017b}' => 0x8c,
        '\u{017c}' => 0x9c,
        '\u{20ac}' => 0x80,
        '\u{201a}' => 0x82,
        '\u{0192}' => 0x83,
        '\u{201e}' => 0x84,
        '\u{2026}' => 0x85,
        '\u{2020}' => 0x86,
        '\u{2021}' => 0x87,
        '\u{02c6}' => 0x88,
        '\u{2030}' => 0x89,
        '\u{0160}' => 0x8a,
        '\u{2039}' => 0x8b,
        '\u{0152}' => 0x8c,
        '\u{017d}' => 0x8e,
        '\u{2018}' => 0x91,
        '\u{2019}' => 0x92,
        '\u{201c}' => 0x93,
        '\u{201d}' => 0x94,
        '\u{2022}' => 0x95,
        '\u{2013}' => 0x96,
        '\u{2014}' => 0x97,
        '\u{02dc}' => 0x98,
        '\u{2122}' => 0x99,
        '\u{0161}' => 0x9a,
        '\u{203a}' => 0x9b,
        '\u{0153}' => 0x9c,
        '\u{017e}' => 0x9e,
        '\u{0178}' => 0x9f,
        '\u{00a0}' => 0xa0,
        '\u{00ad}' => 0xad,
        '\u{00a1}'..='\u{00ff}' => ch as u8,
        '\u{2011}' => b'-',
        '\u{2002}' | '\u{2003}' | '\u{2005}' => b' ',
        ch if ch.is_ascii() => ch as u8,
        _ => b'?',
    }
}

fn extended_latin_entry_for_char(ch: char) -> Option<ExtendedLatinEntry> {
    let (byte, glyph_name) = match ch {
        '\u{0100}' => (0xc2, b"Amacron".as_slice()),
        '\u{0101}' => (0xe2, b"amacron".as_slice()),
        '\u{0104}' => (0xc0, b"Aogonek".as_slice()),
        '\u{0105}' => (0xe0, b"aogonek".as_slice()),
        '\u{0106}' => (0xc3, b"Cacute".as_slice()),
        '\u{0107}' => (0xe3, b"cacute".as_slice()),
        '\u{010c}' => (0xc8, b"Ccaron".as_slice()),
        '\u{010d}' => (0xe8, b"ccaron".as_slice()),
        '\u{0112}' => (0xc7, b"Emacron".as_slice()),
        '\u{0113}' => (0xe7, b"emacron".as_slice()),
        '\u{0116}' => (0xcb, b"Edotaccent".as_slice()),
        '\u{0117}' => (0xeb, b"edotaccent".as_slice()),
        '\u{0118}' => (0xc6, b"Eogonek".as_slice()),
        '\u{0119}' => (0xe6, b"eogonek".as_slice()),
        '\u{011e}' => (0xd0, b"Gbreve".as_slice()),
        '\u{011f}' => (0xf0, b"gbreve".as_slice()),
        '\u{0122}' => (0xcc, b"Gcommaaccent".as_slice()),
        '\u{0123}' => (0xec, b"gcommaaccent".as_slice()),
        '\u{012a}' => (0xce, b"Imacron".as_slice()),
        '\u{012b}' => (0xee, b"imacron".as_slice()),
        '\u{012e}' => (0xc1, b"Iogonek".as_slice()),
        '\u{012f}' => (0xe1, b"iogonek".as_slice()),
        '\u{0130}' => (0xdd, b"Idotaccent".as_slice()),
        '\u{0131}' => (0xfd, b"dotlessi".as_slice()),
        '\u{0136}' => (0xcd, b"Kcommaaccent".as_slice()),
        '\u{0137}' => (0xed, b"kcommaaccent".as_slice()),
        '\u{013b}' => (0xcf, b"Lcommaaccent".as_slice()),
        '\u{013c}' => (0xef, b"lcommaaccent".as_slice()),
        '\u{0141}' => (0xd9, b"Lslash".as_slice()),
        '\u{0142}' => (0xf9, b"lslash".as_slice()),
        '\u{0143}' => (0xd1, b"Nacute".as_slice()),
        '\u{0144}' => (0xf1, b"nacute".as_slice()),
        '\u{0145}' => (0xd2, b"Ncommaaccent".as_slice()),
        '\u{0146}' => (0xf2, b"ncommaaccent".as_slice()),
        '\u{014c}' => (0xd4, b"Omacron".as_slice()),
        '\u{014d}' => (0xf4, b"omacron".as_slice()),
        '\u{0150}' => (0x81, b"Ohungarumlaut".as_slice()),
        '\u{0151}' => (0x8d, b"ohungarumlaut".as_slice()),
        '\u{0156}' => (0xaa, b"Rcommaaccent".as_slice()),
        '\u{0157}' => (0xba, b"rcommaaccent".as_slice()),
        '\u{015a}' => (0xda, b"Sacute".as_slice()),
        '\u{015b}' => (0xfa, b"sacute".as_slice()),
        '\u{015e}' => (0xde, b"Scedilla".as_slice()),
        '\u{015f}' => (0xfe, b"scedilla".as_slice()),
        '\u{016a}' => (0xdb, b"Umacron".as_slice()),
        '\u{016b}' => (0xfb, b"umacron".as_slice()),
        '\u{0170}' => (0x8f, b"Uhungarumlaut".as_slice()),
        '\u{0171}' => (0x90, b"uhungarumlaut".as_slice()),
        '\u{0172}' => (0xd8, b"Uogonek".as_slice()),
        '\u{0173}' => (0xf8, b"uogonek".as_slice()),
        '\u{0179}' => (0xca, b"Zacute".as_slice()),
        '\u{017a}' => (0xea, b"zacute".as_slice()),
        '\u{017b}' => (0x8c, b"Zdotaccent".as_slice()),
        '\u{017c}' => (0x9c, b"zdotaccent".as_slice()),
        _ => return None,
    };
    Some(ExtendedLatinEntry {
        byte,
        unicode: ch,
        glyph_name,
    })
}

fn set_fill_color(content: &mut Content, color: PdfColor) {
    content.set_fill_rgb(color.red, color.green, color.blue);
}

fn set_stroke_color(content: &mut Content, color: PdfColor) {
    content.set_stroke_rgb(color.red, color.green, color.blue);
}

fn write_text_fragment(
    content: &mut Content,
    text: &str,
    font_resource: &[u8],
    passive_kerning_family: Option<PdfFontFamily>,
    style: &CharacterStyle,
    word_spacing: f32,
    x: f32,
    baseline_y: f32,
    encoded: &[u8],
    rendering_mode: TextRenderingMode,
) {
    content.begin_text();
    content.set_font(Name(font_resource), style.font_size_points());
    if rendering_mode != TextRenderingMode::Fill {
        content.set_text_rendering_mode(rendering_mode);
    }
    content.set_word_spacing(word_spacing);
    content.set_char_spacing(twips_to_points(style.character_spacing_twips));
    content.set_horizontal_scaling(style.character_scaling_percent as f32);
    content.next_line(x, baseline_y);
    if let Some(font_family) = passive_kerning_family
        && style_uses_passive_kerning(style)
        && text_has_passive_kerning(text, font_family, style)
    {
        write_positioned_text(content, text, font_family, style);
    } else {
        content.show(Str(encoded));
    }
    content.set_horizontal_scaling(100.0);
    content.set_char_spacing(0.0);
    content.set_word_spacing(0.0);
    if rendering_mode != TextRenderingMode::Fill {
        content.set_text_rendering_mode(TextRenderingMode::Fill);
    }
    content.end_text();
}

fn text_has_passive_kerning(
    text: &str,
    font_family: PdfFontFamily,
    style: &CharacterStyle,
) -> bool {
    let mut previous = None;
    for ch in text.chars().filter(|ch| !is_zero_width_pdf_char(*ch)) {
        if let Some(left) = previous
            && passive_pair_kerning_points(left, ch, font_family, style) != 0.0
        {
            return true;
        }
        previous = Some(ch);
    }
    false
}

fn write_positioned_text(
    content: &mut Content,
    text: &str,
    font_family: PdfFontFamily,
    style: &CharacterStyle,
) {
    let mut positioned = content.show_positioned();
    let mut items = positioned.items();
    let mut previous = None;
    let font_size = style.font_size_points().max(1.0);

    for ch in text.chars().filter(|ch| !is_zero_width_pdf_char(*ch)) {
        if let Some(left) = previous {
            let adjustment = passive_pair_kerning_points(left, ch, font_family, style);
            if adjustment != 0.0 {
                items.adjust((-adjustment * 1000.0 / font_size).clamp(-1000.0, 1000.0));
            }
        }
        let mut buffer = [0; 4];
        let encoded = encode_pdf_text_for_font(ch.encode_utf8(&mut buffer), font_family);
        items.show(Str(&encoded));
        previous = Some(ch);
    }
}

fn shadow_offset(style: &CharacterStyle) -> f32 {
    (style.font_size_points() * 0.08).clamp(0.75, 2.5)
}

fn shadow_color(color: PdfColor) -> PdfColor {
    PdfColor {
        red: color.red * 0.35,
        green: color.green * 0.35,
        blue: color.blue * 0.35,
    }
}

fn relief_offset(style: &CharacterStyle) -> f32 {
    (style.font_size_points() * 0.045).clamp(0.5, 1.5)
}

fn relief_layers(
    color: PdfColor,
    relief: TextRelief,
    offset: f32,
) -> (PdfColor, f32, f32, PdfColor, f32, f32) {
    let light = lighten_color(color);
    let dark = shadow_color(color);
    match relief {
        TextRelief::None => (color, 0.0, 0.0, color, 0.0, 0.0),
        TextRelief::Emboss => (light, -offset, offset, dark, offset, -offset),
        TextRelief::Engrave => (dark, -offset, offset, light, offset, -offset),
    }
}

fn lighten_color(color: PdfColor) -> PdfColor {
    PdfColor {
        red: 1.0 - ((1.0 - color.red) * 0.25),
        green: 1.0 - ((1.0 - color.green) * 0.25),
        blue: 1.0 - ((1.0 - color.blue) * 0.25),
    }
}

#[cfg(test)]
mod tests {
    use crate::fonts::{FontAsset, FontAssetStyle, FontProvider};
    use crate::layout::LayoutEngine;
    use crate::model::{
        Alignment, Block, BorderStyle, CharacterStyle, Color, Document, FontDef, FontFamilyHint,
        FontPitch, ImageCrop, ImageFormat, PAGE_NUMBER_MARKER, PageSettings, Paragraph,
        ParagraphStyle, Run, SECTION_NUMBER_MARKER, StaticImage, StaticShape, StaticShapeArrowhead,
        StaticShapeKind, TOTAL_PAGES_MARKER, Table, TableCell, TableCellBorder, TableRow,
        UnderlineStyle,
    };
    use lopdf::{Dictionary, Object};

    use super::*;

    #[test]
    fn writes_valid_pdf_bytes() {
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Hello PDF".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
    }

    #[test]
    fn styled_supplied_font_assets_are_selected_by_run_style() {
        let mut document = Document {
            fonts: vec![FontDef {
                index: 0,
                name: "Tuffy".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Variable,
            }],
            ..Document::default()
        };
        let mut regular_style = CharacterStyle::default();
        regular_style.font_index = 0;
        let mut bold_style = regular_style.clone();
        bold_style.bold = true;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: ParagraphStyle::default(),
            runs: vec![
                Run {
                    text: "Regular".to_string(),
                    style: regular_style,
                },
                Run {
                    text: " Bold".to_string(),
                    style: bold_style,
                },
            ],
        })];
        let provider = FontProvider {
            assets: vec![
                FontAsset {
                    family_names: vec!["Tuffy".to_string()],
                    style: FontAssetStyle::default(),
                    bytes: include_bytes!("../fixtures/fonts/Tuffy.ttf").to_vec(),
                },
                FontAsset {
                    family_names: vec!["Tuffy".to_string()],
                    style: FontAssetStyle {
                        bold: true,
                        italic: false,
                    },
                    bytes: include_bytes!("../fixtures/fonts/Tuffy.ttf").to_vec(),
                },
            ],
            limits: Default::default(),
        };

        let layout = LayoutEngine::layout_with_font_provider(&document, Some(&provider));
        let pdf = render_pdf_with_font_provider(&layout, Some(&provider));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();
        let page_fonts = page_font_resource_names(&pdf, 0);

        assert!(page_fonts.iter().any(|name| name == b"TF1"));
        assert!(page_fonts.iter().any(|name| name == b"TF2"));
        assert!(!content_bytes_for_font(&content, b"TF1").is_empty());
        assert!(!content_bytes_for_font(&content, b"TF2").is_empty());
        assert!(
            pdf.windows(b"ORTF01+Tuffy".len())
                .any(|window| window == b"ORTF01+Tuffy")
        );
        assert!(
            pdf.windows(b"ORTF02+Tuffy".len())
                .any(|window| window == b"ORTF02+Tuffy")
        );
        assert!(
            !pdf.windows(b"ORTFSuppliedFont".len())
                .any(|window| window == b"ORTFSuppliedFont")
        );
        assert!(
            pdf.windows(b"/FontFile2".len())
                .any(|window| window == b"/FontFile2")
        );
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn estimated_pdf_object_count_matches_rendered_supplied_font_and_image_objects() {
        let mut document = Document {
            fonts: vec![FontDef {
                index: 0,
                name: "Tuffy".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Variable,
            }],
            ..Document::default()
        };
        let mut style = CharacterStyle::default();
        style.font_index = 0;
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: ParagraphStyle::default(),
                runs: vec![Run {
                    text: "Supplied font".to_string(),
                    style,
                }],
            }),
            Block::Image(StaticImage {
                format: ImageFormat::Rgb8,
                bytes: vec![255, 0, 0, 0, 0, 255],
                palette: Vec::new(),
                vector_commands: Vec::new(),
                width_px: 2,
                height_px: 1,
                natural_width_px_hint: None,
                natural_height_px_hint: None,
                display_width_twips: Some(720),
                display_height_twips: Some(720),
                scale_x_percent: None,
                scale_y_percent: None,
                crop: ImageCrop::default(),
                placement: None,
            }),
        ];
        let provider = FontProvider {
            assets: vec![FontAsset {
                family_names: vec!["Tuffy".to_string()],
                style: FontAssetStyle::default(),
                bytes: include_bytes!("../fixtures/fonts/Tuffy.ttf").to_vec(),
            }],
            limits: Default::default(),
        };

        let layout = LayoutEngine::layout_with_font_provider(&document, Some(&provider));
        let estimated = estimate_passive_pdf_object_count(&layout, Some(&provider));
        let pdf = render_pdf_with_font_provider(&layout, Some(&provider));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();

        assert_eq!(estimated, parsed.objects.len());
        assert_eq!(estimated, 10);
        assert!(
            pdf.windows(b"/FontFile2".len())
                .any(|window| window == b"/FontFile2")
        );
        assert!(
            pdf.windows(b"/Subtype /Image".len())
                .any(|window| window == b"/Subtype /Image")
        );
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn supplied_pdf_font_names_are_metadata_derived_and_sanitized() {
        let asset = FontAsset {
            family_names: vec!["Tuffy".to_string()],
            style: FontAssetStyle::default(),
            bytes: include_bytes!("../fixtures/fonts/Tuffy.ttf").to_vec(),
        };

        assert_eq!(
            supplied_pdf_font_base_name(&asset, 2),
            b"ORTF03+Tuffy".to_vec()
        );
        assert_eq!(
            sanitize_pdf_font_name("Bad Font/Name#1", 64),
            "Bad-Font-Name-1"
        );
        assert_eq!(sanitize_pdf_font_name(" --- ", 64), "");
    }

    #[test]
    fn unknown_builtin_font_resource_lookup_is_non_panicking() {
        assert_eq!(font_index_for_resource(b"UnknownFontResource"), None);
        assert!(font_index_for_resource(HELVETICA_REGULAR).is_some());
    }

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

    #[test]
    fn omits_unused_pdf_base_font_resources() {
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Plain Helvetica text".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);

        assert!(
            pdf.windows(b"/BaseFont /Helvetica".len())
                .any(|window| window == b"/BaseFont /Helvetica")
        );
        for forbidden in [
            b"/BaseFont /Helvetica-Bold".as_slice(),
            b"/BaseFont /Courier",
            b"/BaseFont /Times-Roman",
            b"/BaseFont /Symbol",
            b"/BaseFont /ZapfDingbats",
            b"OpenRtfConverter-Symbol",
            b"OpenRtfConverter-ZapfDingbats",
        ] {
            assert!(
                !pdf.windows(forbidden.len())
                    .any(|window| window == forbidden),
                "unused font resource leaked into PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn page_resources_omit_fonts_used_only_on_other_pages() {
        let mut document = Document::default();
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Plain page".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::PageBreak,
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "\u{03b1}".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let first_page_fonts = page_font_resource_names(&pdf, 0);
        let second_page_fonts = page_font_resource_names(&pdf, 1);

        assert!(first_page_fonts.iter().any(|name| name == b"F1"));
        assert!(!first_page_fonts.iter().any(|name| name == b"F13"));
        assert!(second_page_fonts.iter().any(|name| name == b"F13"));
        assert!(!second_page_fonts.iter().any(|name| name == b"F1"));
        assert!(
            pdf.windows(b"/BaseFont /Symbol".len())
                .any(|window| window == b"/BaseFont /Symbol"),
            "shared font object should still be emitted when used by any page"
        );
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn symbol_font_includes_passive_to_unicode_map_while_checkboxes_render_as_vectors() {
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "alpha \u{03b1} check \u{2611}".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(
            pdf.windows(b"/BaseFont /Symbol".len())
                .any(|window| window == b"/BaseFont /Symbol")
        );
        assert!(
            !pdf.windows(b"/BaseFont /ZapfDingbats".len())
                .any(|window| window == b"/BaseFont /ZapfDingbats"),
            "passive checkbox vector path should not require a ZapfDingbats font resource"
        );
        assert!(
            pdf.windows(b"/ToUnicode".len())
                .filter(|window| *window == b"/ToUnicode")
                .count()
                >= 1
        );
        assert!(
            pdf.windows(b"<61> <03B1>".len())
                .any(|window| window == b"<61> <03B1>"),
            "Symbol alpha byte must map back to Unicode alpha"
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "re"),
            "checkbox should render as passive vector geometry"
        );
    }

    #[test]
    fn smiley_dingbat_can_render_as_passive_vector_without_zapf_font_resource() {
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "\u{263a}".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(
            !pdf.windows(b"/BaseFont /ZapfDingbats".len())
                .any(|window| window == b"/BaseFont /ZapfDingbats"),
            "vector-only smiley should not require a viewer ZapfDingbats font"
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "c"),
            "vector-only smiley should draw passive Bezier paths"
        );
        assert!(content_bytes_for_font(&content, b"F14").is_empty());
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn passive_pdf_audit_rejects_active_names_outside_streams() {
        let pdf = b"%PDF-1.7
1 0 obj
<< /Type /Catalog /OpenAction << /S /JavaScript /JS (app.alert) >> >>
2 0 obj
<< /Type /Page /Annots [3 0 R] >>
endobj
3 0 obj
<< /Type /Annot /Subtype /Widget /A << /S /URI /URI (https://example.com) >> >>
endobj
4 0 obj
<< /Type /Filespec /AFRelationship /Data /EF << /F 5 0 R >> >>
endobj
5 0 obj
<< /Type /EmbeddedFile /Subtype /FileAttachment >>
endobj
6 0 obj
<< /AF [4 0 R] /Perms << /DocMDP 7 0 R >> /Encrypt 8 0 R /Collection << >> >>
endobj
7 0 obj
<< /S /GoTo /D [2 0 R /Fit] /Next << /S /Named /N /Print >> >>
endobj
8 0 obj
<< /S /ResetForm >>
endobj
9 0 obj
<< /S /Rendition /OP 0 >>
endobj
10 0 obj
<< /S /SetOCGState /State [/Toggle 1 0 R] >>
endobj
11 0 obj
<< /S /Trans /Trans << /S /Dissolve >> >>
endobj
12 0 obj
<< /S /GoTo3DView /TA 1 0 R /V /Default >>
endobj
13 0 obj
<< /Type /ObjStm /N 1 /First 8 /Length 16 >>
endobj
14 0 obj
<< /Type /XRef /Size 14 /Length 0 >>
endobj
15 0 obj
<< /Names << /EmbeddedFiles [(payload.bin) 4 0 R] >> >>
endobj
%%EOF";

        let error = audit_passive_pdf_bytes(pdf).expect_err("active PDF names must be rejected");

        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.token == "/OpenAction")
        );
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.token == "/JavaScript")
        );
        assert!(error.issues.iter().any(|issue| issue.token == "/Annots"));
        assert!(error.issues.iter().any(|issue| issue.token == "/Widget"));
        assert!(error.issues.iter().any(|issue| issue.token == "/A"));
        assert!(error.issues.iter().any(|issue| issue.token == "/URI"));
        assert!(error.issues.iter().any(|issue| issue.token == "/AF"));
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.token == "/AFRelationship")
        );
        assert!(error.issues.iter().any(|issue| issue.token == "/EF"));
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.token == "/FileAttachment")
        );
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.token == "/EmbeddedFiles")
        );
        assert!(error.issues.iter().any(|issue| issue.token == "/Encrypt"));
        assert!(error.issues.iter().any(|issue| issue.token == "/Perms"));
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.token == "/Collection")
        );
        assert!(error.issues.iter().any(|issue| issue.token == "/GoTo"));
        assert!(error.issues.iter().any(|issue| issue.token == "/Named"));
        assert!(error.issues.iter().any(|issue| issue.token == "/Next"));
        assert!(error.issues.iter().any(|issue| issue.token == "/ObjStm"));
        assert!(error.issues.iter().any(|issue| issue.token == "/XRef"));
        assert!(error.issues.iter().any(|issue| issue.token == "/ResetForm"));
        assert!(error.issues.iter().any(|issue| issue.token == "/Rendition"));
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.token == "/SetOCGState")
        );
        assert!(error.issues.iter().any(|issue| issue.token == "/Trans"));
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.token == "/GoTo3DView")
        );
    }

    #[test]
    fn passive_pdf_audit_decodes_escaped_active_names_outside_streams() {
        let pdf = b"%PDF-1.7
1 0 obj
<< /Type /Catalog /Open#41ction << /#41 << /S /Java#53cript /J#53 (app.alert) >> /#4Eext << /S /Named >> >> >>
endobj
2 0 obj
<< /Names << /Embedded#46iles [(payload.bin) 3 0 R] >> /#45F << /F 3 0 R >> >>
endobj
3 0 obj
<< /Type /Embedded#46ile /AF#52elationship /Data >>
endobj
%%EOF";

        let error =
            audit_passive_pdf_bytes(pdf).expect_err("escaped active PDF names must be rejected");

        for expected in [
            "/OpenAction",
            "/A",
            "/JavaScript",
            "/JS",
            "/Next",
            "/EmbeddedFiles",
            "/EF",
            "/EmbeddedFile",
            "/AFRelationship",
        ] {
            assert!(
                error.issues.iter().any(|issue| issue.token == expected),
                "missing decoded active PDF name {expected}: {:?}",
                error.issues
            );
        }
    }

    #[test]
    fn passive_pdf_audit_rejects_overlong_names_outside_streams() {
        let mut pdf = b"%PDF-1.7
1 0 obj
<< "
        .to_vec();
        pdf.push(b'/');
        pdf.extend(std::iter::repeat_n(b'A', MAX_AUDITED_PDF_NAME_BYTES));
        pdf.extend_from_slice(
            b" /Type /Catalog >>
endobj
%%EOF",
        );

        let error = audit_passive_pdf_bytes(&pdf).expect_err("overlong PDF names must be rejected");

        assert!(error.issues.iter().any(|issue| {
            issue.token == OVERLONG_PDF_NAME_TOKEN
                && issue.offset == b"%PDF-1.7\n1 0 obj\n<< ".len()
        }));
    }

    #[test]
    fn passive_pdf_audit_rejects_overlong_escaped_names_outside_streams() {
        let mut pdf = b"%PDF-1.7
1 0 obj
<< /"
            .to_vec();
        for _ in 0..MAX_AUDITED_PDF_NAME_BYTES {
            pdf.extend_from_slice(b"#41");
        }
        pdf.extend_from_slice(
            b" /Type /Catalog >>
endobj
%%EOF",
        );

        let error =
            audit_passive_pdf_bytes(&pdf).expect_err("overlong escaped PDF names must be rejected");

        assert!(error.issues.iter().any(|issue| {
            issue.token == OVERLONG_PDF_NAME_TOKEN
                && issue.offset == b"%PDF-1.7\n1 0 obj\n<< ".len()
        }));
    }

    #[test]
    fn passive_pdf_audit_rejects_malformed_name_escapes_outside_streams() {
        let pdf = b"%PDF-1.7
1 0 obj
<< /Open#4Gction true /Java#5 >>
endobj
%%EOF";

        let error =
            audit_passive_pdf_bytes(pdf).expect_err("malformed PDF name escapes must be rejected");

        let malformed = error
            .issues
            .iter()
            .filter(|issue| issue.token == MALFORMED_PDF_NAME_ESCAPE_TOKEN)
            .count();
        assert_eq!(malformed, 2, "issues were {:?}", error.issues);
    }

    #[test]
    fn passive_pdf_audit_ignores_visible_words_inside_content_streams() {
        let stream =
            b"BT (/A /Next /JavaScript /Launch /URI /Annots /Widget /ObjStm /XRef) Tj ET\n";
        let mut pdf = format!(
            "%PDF-1.7\n1 0 obj\n<< /Length {} >>\nstream\n",
            stream.len()
        )
        .into_bytes();
        pdf.extend_from_slice(stream);
        pdf.extend_from_slice(b"endstream\nendobj\n%%EOF");

        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn passive_pdf_audit_uses_declared_stream_length_before_marker_words() {
        let stream = b"BT (visible endstream /JavaScript /OpenAction text) Tj ET\n";
        let mut pdf = format!(
            "%PDF-1.7\n1 0 obj\n<< /Length {} >>\nstream\n",
            stream.len()
        )
        .into_bytes();
        pdf.extend_from_slice(stream);
        pdf.extend_from_slice(b"endstream\nendobj\n%%EOF");

        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn passive_pdf_audit_uses_top_level_stream_length() {
        let stream = b"BT (visible endstream /JavaScript text) Tj ET\n";
        let mut pdf = format!(
            "%PDF-1.7\n1 0 obj\n<< /DecodeParms << /Length 0 >> /Length {} >>\nstream\n",
            stream.len()
        )
        .into_bytes();
        pdf.extend_from_slice(stream);
        pdf.extend_from_slice(b"endstream\nendobj\n%%EOF");

        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn passive_pdf_audit_rejects_mismatched_direct_stream_length() {
        let pdf = b"%PDF-1.7
1 0 obj
<< /Length 0 >>
stream
/JavaScript
endstream
endobj
%%EOF";

        let error =
            audit_passive_pdf_bytes(pdf).expect_err("mismatched direct stream length is malformed");

        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.token == "<malformed stream length>"),
            "issues were {:?}",
            error.issues
        );
    }

    #[test]
    fn passive_pdf_audit_rejects_active_names_after_forged_stream_marker() {
        let pdf = b"%PDF-1.7
stream
/JavaScript
endstream
%%EOF";

        let error = audit_passive_pdf_bytes(pdf)
            .expect_err("forged stream marker must not hide active PDF names");

        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.token == "/JavaScript"),
            "issues were {:?}",
            error.issues
        );
    }

    #[test]
    fn passive_pdf_audit_ignores_escaped_visible_words_inside_content_streams() {
        let stream = b"BT (/Java#53cript /Launch /Embedded#46iles /Open#41ction) Tj ET\n";
        let mut pdf = format!(
            "%PDF-1.7\n1 0 obj\n<< /Length {} >>\nstream\n",
            stream.len()
        )
        .into_bytes();
        pdf.extend_from_slice(stream);
        pdf.extend_from_slice(b"endstream\nendobj\n%%EOF");

        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn writes_landscape_media_box() {
        let mut document = Document::default();
        document.page.width_twips = 15_840;
        document.page.height_twips = 12_240;
        document.page.landscape = true;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Landscape PDF".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let page = parsed.get_object(page_id).unwrap().as_dict().unwrap();
        let media_box = page.get(b"MediaBox").unwrap().as_array().unwrap();

        assert_eq!(pdf_number(media_box[2].clone()), Some(792.0));
        assert_eq!(pdf_number(media_box[3].clone()), Some(612.0));
    }

    #[test]
    fn writes_section_specific_media_boxes() {
        let mut document = Document::default();
        let mut second_page = PageSettings::default();
        second_page.width_twips = 10_080;
        second_page.height_twips = 7_200;
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "First page".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::SectionBreak,
            Block::SectionSettings(second_page),
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Second page".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_ids = parsed.get_pages().values().copied().collect::<Vec<_>>();
        let first = parsed.get_object(page_ids[0]).unwrap().as_dict().unwrap();
        let second = parsed.get_object(page_ids[1]).unwrap().as_dict().unwrap();
        let first_media_box = first.get(b"MediaBox").unwrap().as_array().unwrap();
        let second_media_box = second.get(b"MediaBox").unwrap().as_array().unwrap();

        assert_eq!(pdf_number(first_media_box[2].clone()), Some(612.0));
        assert_eq!(pdf_number(first_media_box[3].clone()), Some(792.0));
        assert_eq!(pdf_number(second_media_box[2].clone()), Some(504.0));
        assert_eq!(pdf_number(second_media_box[3].clone()), Some(360.0));
    }

    #[test]
    fn writes_column_layout_as_passive_text_and_rules() {
        let mut document = Document::default();
        document.page.column_count = 2;
        document.page.line_between_columns = true;
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Left".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::ColumnBreak,
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Right".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(parsed.get_pages().len(), 1);
        assert_eq!(content_text(&content), "LeftRight");
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "m")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
        assert!(!pdf.windows(b"/URI".len()).any(|window| window == b"/URI"));
    }

    #[test]
    fn writes_paragraph_borders_as_passive_lines() {
        let mut document = Document::default();
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 80,
            color_index: None,
            ..TableCellBorder::default()
        };
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "Bordered".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Bordered");
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "S")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
        assert!(!pdf.windows(b"/URI".len()).any(|window| window == b"/URI"));
    }

    #[test]
    fn writes_character_border_as_passive_strokes() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
        ];
        let mut style = CharacterStyle::default();
        style.border = TableCellBorder {
            visible: true,
            width_twips: 80,
            color_index: Some(1),
            ..TableCellBorder::default()
        };
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Boxed".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Boxed");
        assert!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "S")
                .count()
                >= 4
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "RG"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 1.0)
                && pdf_number(operation.operands[1].clone()).is_some_and(|value| value == 0.0)
                && pdf_number(operation.operands[2].clone()).is_some_and(|value| value == 0.0)
        }));
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
        assert!(!pdf.windows(b"/URI".len()).any(|window| window == b"/URI"));
    }

    #[test]
    fn writes_ellipse_shape_as_passive_bezier_path() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::Ellipse,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            z_order: 0,
            below_text: false,
            horizontal_anchor: crate::model::StaticShapeHorizontalAnchor::Column,
            vertical_anchor: crate::model::StaticShapeVerticalAnchor::Paragraph,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            stroke_width_twips: 20,
            stroke_color: Color {
                red: 255,
                green: 128,
                blue: 0,
            },
            stroke_style: BorderStyle::Single,
            fill_color: Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            }),
            text: Vec::new(),
            points: Vec::new(),
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "c")
                .count(),
            4
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "B")
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "RG"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 1.0)
                && pdf_number(operation.operands[1].clone())
                    .is_some_and(|value| (value - 0.5019608).abs() < 0.001)
                && pdf_number(operation.operands[2].clone()).is_some_and(|value| value == 0.0)
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone())
                    .is_some_and(|value| (value - (10.0 / 255.0)).abs() < 0.001)
                && pdf_number(operation.operands[1].clone())
                    .is_some_and(|value| (value - (20.0 / 255.0)).abs() < 0.001)
                && pdf_number(operation.operands[2].clone())
                    .is_some_and(|value| (value - (30.0 / 255.0)).abs() < 0.001)
        }));
        for forbidden in [
            b"/JavaScript".as_slice(),
            b"/EmbeddedFile",
            b"/Launch",
            b"/OpenAction",
            b"/RichMedia",
        ] {
            assert!(
                !pdf.windows(forbidden.len())
                    .any(|window| window == forbidden)
            );
        }
    }

    #[test]
    fn writes_rounded_rectangle_shape_as_passive_bezier_path() {
        let mut document = Document::default();
        document.blocks = vec![Block::Shape(StaticShape {
            kind: StaticShapeKind::RoundedRectangle,
            left_twips: 360,
            top_twips: 240,
            width_twips: 1440,
            height_twips: 720,
            z_order: 0,
            below_text: false,
            horizontal_anchor: crate::model::StaticShapeHorizontalAnchor::Column,
            vertical_anchor: crate::model::StaticShapeVerticalAnchor::Paragraph,
            flip_horizontal: false,
            flip_vertical: false,
            start_arrowhead: StaticShapeArrowhead::None,
            end_arrowhead: StaticShapeArrowhead::None,
            stroke_width_twips: 20,
            stroke_color: Color {
                red: 255,
                green: 128,
                blue: 0,
            },
            stroke_style: BorderStyle::Single,
            fill_color: Some(Color {
                red: 10,
                green: 20,
                blue: 30,
            }),
            text: Vec::new(),
            points: Vec::new(),
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "c")
                .count(),
            4
        );
        assert!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "l")
                .count()
                >= 4
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "B")
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "RG"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 1.0)
                && pdf_number(operation.operands[1].clone())
                    .is_some_and(|value| (value - 0.5019608).abs() < 0.001)
                && pdf_number(operation.operands[2].clone()).is_some_and(|value| value == 0.0)
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone())
                    .is_some_and(|value| (value - (10.0 / 255.0)).abs() < 0.001)
                && pdf_number(operation.operands[1].clone())
                    .is_some_and(|value| (value - (20.0 / 255.0)).abs() < 0.001)
                && pdf_number(operation.operands[2].clone())
                    .is_some_and(|value| (value - (30.0 / 255.0)).abs() < 0.001)
        }));
        for forbidden in [
            b"/JavaScript".as_slice(),
            b"/EmbeddedFile",
            b"/Launch",
            b"/OpenAction",
            b"/RichMedia",
        ] {
            assert!(
                !pdf.windows(forbidden.len())
                    .any(|window| window == forbidden)
            );
        }
    }

    #[test]
    fn writes_border_styles_as_passive_pdf_strokes() {
        let mut document = Document::default();
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 60,
            color_index: None,
            style: BorderStyle::Double,
            ..TableCellBorder::default()
        };
        let mut character_style = CharacterStyle::default();
        character_style.border = TableCellBorder {
            visible: true,
            width_twips: 40,
            color_index: None,
            style: BorderStyle::Dashed,
            ..TableCellBorder::default()
        };
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "Styled".to_string(),
                style: character_style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Styled");
        assert!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "S")
                .count()
                >= 6
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "d")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_wavy_borders_as_passive_path_strokes() {
        let mut document = Document::default();
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.borders.bottom = TableCellBorder {
            visible: true,
            width_twips: 80,
            color_index: None,
            style: BorderStyle::Wavy,
            ..TableCellBorder::default()
        };
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "Wavy".to_string(),
                style: CharacterStyle::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Wavy");
        assert!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "l")
                .count()
                > 2
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
        assert!(!pdf.windows(b"/URI".len()).any(|window| window == b"/URI"));
    }

    #[test]
    fn writes_passive_jpeg_image_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Jpeg,
            bytes: minimal_jpeg_with_dimensions(1, 1),
            palette: Vec::new(),
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/Subtype /Image".len())
                .any(|window| window == b"/Subtype /Image")
        );
        assert!(
            pdf.windows(b"/DCTDecode".len())
                .any(|window| window == b"/DCTDecode")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_passive_grayscale_jpeg_image_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::JpegGrayscale,
            bytes: minimal_grayscale_jpeg_with_dimensions(1, 1),
            palette: Vec::new(),
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/ColorSpace /DeviceGray".len())
                .any(|window| window == b"/ColorSpace /DeviceGray")
        );
        assert!(
            pdf.windows(b"/DCTDecode".len())
                .any(|window| window == b"/DCTDecode")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_passive_cmyk_jpeg_image_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::JpegCmyk,
            bytes: minimal_cmyk_jpeg_with_dimensions(1, 1),
            palette: Vec::new(),
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/ColorSpace /DeviceCMYK".len())
                .any(|window| window == b"/ColorSpace /DeviceCMYK")
        );
        assert!(
            pdf.windows(b"/DCTDecode".len())
                .any(|window| window == b"/DCTDecode")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_passive_png_image_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Png,
            bytes: minimal_png_idat_for_1x1_rgb(),
            palette: Vec::new(),
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/Subtype /Image".len())
                .any(|window| window == b"/Subtype /Image")
        );
        assert!(
            pdf.windows(b"/FlateDecode".len())
                .any(|window| window == b"/FlateDecode")
        );
        assert!(
            pdf.windows(b"/Predictor 15".len())
                .any(|window| window == b"/Predictor 15")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_passive_indexed_png_image_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::PngIndexed,
            bytes: minimal_png_idat_for_1x1_indexed(),
            palette: vec![255, 0, 0, 0, 255, 0],
            vector_commands: Vec::new(),
            width_px: 1,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/Subtype /Image".len())
                .any(|window| window == b"/Subtype /Image")
        );
        assert!(
            pdf.windows(b"/Indexed /DeviceRGB 1".len())
                .any(|window| window == b"/Indexed /DeviceRGB 1")
        );
        assert!(
            pdf.windows(b"/FlateDecode".len())
                .any(|window| window == b"/FlateDecode")
        );
        assert!(
            pdf.windows(b"/Colors 1".len())
                .any(|window| window == b"/Colors 1")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_passive_raw_rgb_image_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Rgb8,
            bytes: vec![255, 0, 0, 0, 255, 0],
            palette: Vec::new(),
            vector_commands: Vec::new(),
            width_px: 2,
            height_px: 1,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(720),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop::default(),
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            pdf.windows(b"/Subtype /Image".len())
                .any(|window| window == b"/Subtype /Image")
        );
        assert!(
            !pdf.windows(b"/DCTDecode".len())
                .any(|window| window == b"/DCTDecode")
        );
        assert!(
            !pdf.windows(b"/Predictor".len())
                .any(|window| window == b"/Predictor")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
    }

    #[test]
    fn writes_cropped_image_as_passive_clip_and_xobject() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Jpeg,
            bytes: minimal_jpeg_with_dimensions(100, 100),
            palette: Vec::new(),
            vector_commands: Vec::new(),
            width_px: 100,
            height_px: 100,
            natural_width_px_hint: None,
            natural_height_px_hint: None,
            display_width_twips: Some(1440),
            display_height_twips: Some(1440),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop {
                left_twips: 240,
                top_twips: 0,
                right_twips: 0,
                bottom_twips: 0,
            },
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "W")
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "Do")
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
    }

    #[test]
    fn crops_image_against_passive_natural_size_hints() {
        let mut document = Document::default();
        document.blocks = vec![Block::Image(StaticImage {
            format: ImageFormat::Jpeg,
            bytes: minimal_jpeg_with_dimensions(2, 1),
            palette: Vec::new(),
            vector_commands: Vec::new(),
            width_px: 2,
            height_px: 1,
            natural_width_px_hint: Some(80),
            natural_height_px_hint: Some(40),
            display_width_twips: Some(1440),
            display_height_twips: Some(720),
            scale_x_percent: None,
            scale_y_percent: None,
            crop: ImageCrop {
                left_twips: 720,
                top_twips: 0,
                right_twips: 0,
                bottom_twips: 0,
            },
            placement: None,
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();
        let do_pos = content
            .operations
            .iter()
            .position(|operation| operation.operator == "Do")
            .expect("image draw operation");
        let image_transform = content.operations[..do_pos]
            .iter()
            .rfind(|operation| operation.operator == "cm")
            .expect("image transform");

        assert!(
            pdf_number(image_transform.operands[0].clone())
                .is_some_and(|value| (value - 180.0).abs() < 0.01),
            "left crop should use passive natural width hint, got {:?}",
            image_transform.operands
        );
        assert!(
            pdf_number(image_transform.operands[3].clone())
                .is_some_and(|value| (value - 36.0).abs() < 0.01),
            "vertical image scale should remain the displayed height, got {:?}",
            image_transform.operands
        );
        assert!(
            !pdf.windows(b"/JavaScript".len())
                .any(|window| window == b"/JavaScript")
        );
        assert!(
            !pdf.windows(b"/EmbeddedFile".len())
                .any(|window| window == b"/EmbeddedFile")
        );
    }

    #[test]
    fn writes_text_color_and_highlight_as_passive_drawing() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
            Color {
                red: 0,
                green: 255,
                blue: 0,
            },
        ];
        let mut style = CharacterStyle::default();
        style.color_index = 1;
        style.highlight_index = Some(2);
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Marked".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && format!("{:?}", operation.operands) == "[0, 1, 0]"
        }));
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "re")
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "f")
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && format!("{:?}", operation.operands) == "[1, 0, 0]"
        }));
    }

    #[test]
    fn writes_character_shading_intensity_as_passive_fill() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
        ];
        let mut style = CharacterStyle::default();
        style.highlight_index = Some(1);
        style.highlight_shading_basis_points = 5_000;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Tinted".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Tinted");
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 1.0)
                && pdf_number(operation.operands[1].clone()).is_some_and(|value| value == 0.5)
                && pdf_number(operation.operands[2].clone()).is_some_and(|value| value == 0.5)
        }));
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "re")
        );
    }

    #[test]
    fn writes_courier_font_resource_for_monospace_rtf_font() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Helvetica".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Courier New".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Modern,
                pitch: FontPitch::Default,
            },
        ];
        let mut style = CharacterStyle::default();
        style.font_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Mono".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(
            pdf.windows(b"/BaseFont /Courier".len())
                .any(|window| window == b"/BaseFont /Courier")
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tf" && format!("{:?}", operation.operands).contains("/F5")
        }));
    }

    #[test]
    fn writes_base14_font_resources_from_rtf_family_hints() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Helvetica".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Mystery Serif".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Roman,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 2,
                name: "Mystery Mono".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Modern,
                pitch: FontPitch::Default,
            },
        ];
        let mut roman_style = CharacterStyle::default();
        roman_style.font_index = 1;
        let mut modern_style = CharacterStyle::default();
        modern_style.font_index = 2;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    text: "Roman".to_string(),
                    style: roman_style,
                },
                Run {
                    text: "Modern".to_string(),
                    style: modern_style,
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tf" && format!("{:?}", operation.operands).contains("/F9")
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tf" && format!("{:?}", operation.operands).contains("/F5")
        }));
        for forbidden in [b"/JavaScript".as_slice(), b"/EmbeddedFile", b"/Launch"] {
            assert!(
                !pdf.windows(forbidden.len())
                    .any(|window| window == forbidden)
            );
        }
    }

    #[test]
    fn writes_courier_font_resource_for_fixed_pitch_font_hint() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Helvetica".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Mystery Fixed".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Nil,
                pitch: FontPitch::Fixed,
            },
        ];
        let mut style = CharacterStyle::default();
        style.font_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Fixed".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tf" && format!("{:?}", operation.operands).contains("/F5")
        }));
        assert!(
            !pdf.windows(b"Mystery Fixed".len())
                .any(|window| window == b"Mystery Fixed")
        );
    }

    #[test]
    fn writes_table_cell_shading_as_passive_fill() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 220,
                green: 230,
                blue: 240,
            },
        ];
        document.blocks = vec![Block::Table(Table {
            column_widths_twips: vec![1440],
            borders_visible: true,
            preserve_authored_widths: false,
            rows: vec![TableRow {
                height_twips: None,
                left_offset_twips: 0,
                cell_gap_twips: 60,
                alignment: crate::model::TableRowAlignment::Left,
                repeat_header: false,
                keep_together: false,
                cells: vec![TableCell {
                    shading_color_index: Some(1),
                    shading_basis_points: 10_000,
                    shading_pattern: crate::model::ShadingPattern::None,
                    padding: crate::model::TableCellPadding::default(),
                    spacing: Default::default(),
                    borders: crate::model::TableCellBorders::default(),
                    fit_text: false,
                    vertical_align: crate::model::TableCellVerticalAlign::Top,
                    horizontal_merge: crate::model::TableCellHorizontalMerge::None,
                    vertical_merge: crate::model::TableCellVerticalMerge::None,
                    paragraphs: vec![Paragraph {
                        style: Default::default(),
                        runs: vec![Run {
                            text: "Shaded".to_string(),
                            style: Default::default(),
                        }],
                    }],
                }],
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && format!("{:?}", operation.operands).contains("0.8627451")
        }));
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "re")
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "f")
        );
        assert!(content_text(&content).contains("Shaded"));
    }

    #[test]
    fn writes_caps_as_uppercase_text() {
        let mut document = Document::default();
        let mut style = CharacterStyle::default();
        style.all_caps = true;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Mixed case".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "MIXED CASE");
    }

    #[test]
    fn writes_small_caps_as_passive_scaled_uppercase_text() {
        let mut document = Document::default();
        let mut style = CharacterStyle::default();
        style.small_caps = true;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Mix".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "MIX");
        assert!(
            content.operations.iter().any(|operation| {
                operation.operator == "Tf" && format!("{:?}", operation.operands).contains("8.5")
            }),
            "lowercase small-caps glyphs should be emitted with a reduced passive font size"
        );
    }

    #[test]
    fn writes_word_underline_variants_as_passive_strokes() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 0,
                green: 0,
                blue: 0,
            },
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
        ];
        let mut double_style = CharacterStyle::default();
        double_style.underline = UnderlineStyle::Double;
        double_style.underline_color_index = Some(2);
        let mut dotted_style = CharacterStyle::default();
        dotted_style.underline = UnderlineStyle::Dotted;
        let mut dashed_style = CharacterStyle::default();
        dashed_style.underline = UnderlineStyle::Dashed;
        let mut wave_style = CharacterStyle::default();
        wave_style.underline = UnderlineStyle::Wave;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    text: "double ".to_string(),
                    style: double_style,
                },
                Run {
                    text: "dotted ".to_string(),
                    style: dotted_style,
                },
                Run {
                    text: "dashed ".to_string(),
                    style: dashed_style,
                },
                Run {
                    text: "wave".to_string(),
                    style: wave_style,
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "double dotted dashed wave");
        assert!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "S")
                .count()
                >= 5
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "d")
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "RG"
                && operation.operands.len() == 3
                && format!("{:?}", operation.operands).contains('1')
        }));
    }

    #[test]
    fn writes_optional_hyphen_only_when_wrapping_uses_it() {
        let mut wide_document = Document::default();
        wide_document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Alpha\u{00ad}Beta".to_string(),
                style: Default::default(),
            }],
        })];
        let wide_layout = LayoutEngine::layout(&wide_document);
        let wide_pdf = render_pdf(&wide_layout);
        let wide_parsed = lopdf::Document::load_mem(&wide_pdf).unwrap();
        let wide_page_id = *wide_parsed.get_pages().values().next().expect("page");
        let wide_content = wide_parsed
            .get_and_decode_page_content(wide_page_id)
            .unwrap();
        assert_eq!(content_text(&wide_content), "AlphaBeta");

        let mut narrow_style = ParagraphStyle::default();
        narrow_style.right_indent_twips = 8_400;
        let mut narrow_document = Document::default();
        narrow_document.blocks = vec![Block::Paragraph(Paragraph {
            style: narrow_style,
            runs: vec![Run {
                text: "Alpha\u{00ad}Beta".to_string(),
                style: Default::default(),
            }],
        })];
        let narrow_layout = LayoutEngine::layout(&narrow_document);
        let narrow_pdf = render_pdf(&narrow_layout);
        let narrow_parsed = lopdf::Document::load_mem(&narrow_pdf).unwrap();
        let narrow_page_id = *narrow_parsed.get_pages().values().next().expect("page");
        let narrow_content = narrow_parsed
            .get_and_decode_page_content(narrow_page_id)
            .unwrap();
        assert_eq!(content_text(&narrow_content), "Alpha-Beta");
    }

    #[test]
    fn does_not_write_hidden_runs_to_pdf_text() {
        let mut hidden_style = CharacterStyle::default();
        hidden_style.hidden = true;
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![
                Run {
                    text: "Visible".to_string(),
                    style: Default::default(),
                },
                Run {
                    text: "Hidden".to_string(),
                    style: hidden_style,
                },
                Run {
                    text: "Shown".to_string(),
                    style: Default::default(),
                },
            ],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "VisibleShown");
        assert!(
            !pdf.windows(b"Hidden".len())
                .any(|window| window == b"Hidden")
        );
    }

    #[test]
    fn writes_bounded_font_size_to_pdf_text_state() {
        let mut style = CharacterStyle::default();
        style.font_size_half_points = 96;
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Bounded".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Bounded");
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tf"
                && operation.operands.len() == 2
                && format!("{:?}", operation.operands[1]).contains("48")
        }));
    }

    #[test]
    fn writes_character_spacing_as_passive_pdf_text_state() {
        let mut style = CharacterStyle::default();
        style.character_spacing_twips = 200;
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Wide".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Wide");
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tc"
                && operation.operands.len() == 1
                && format!("{:?}", operation.operands[0]).contains("10")
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tc"
                && operation.operands.len() == 1
                && format!("{:?}", operation.operands[0]).contains('0')
        }));
    }

    #[test]
    fn writes_character_scaling_as_passive_pdf_text_state() {
        let mut style = CharacterStyle::default();
        style.character_scaling_percent = 150;
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Scaled".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Scaled");
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tz"
                && operation.operands.len() == 1
                && format!("{:?}", operation.operands[0]).contains("150")
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tz"
                && operation.operands.len() == 1
                && format!("{:?}", operation.operands[0]).contains("100")
        }));
    }

    #[test]
    fn writes_justified_word_spacing_as_passive_pdf_text_state() {
        let mut paragraph_style = ParagraphStyle::default();
        paragraph_style.alignment = Alignment::Justified;
        let mut document = Document::default();
        document.page.width_twips = 5_000;
        document.page.margin_left_twips = 720;
        document.page.margin_right_twips = 720;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: paragraph_style,
            runs: vec![Run {
                text: "one two three four five six seven eight nine ten".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(content_text(&content).contains("one"));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tw"
                && operation.operands.len() == 1
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value > 0.0)
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tw"
                && operation.operands.len() == 1
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 0.0)
        }));
    }

    #[test]
    fn writes_outline_text_as_passive_pdf_text_state() {
        let mut style = CharacterStyle::default();
        style.outline = true;
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Outline".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "Outline");
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tr"
                && operation.operands.len() == 1
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 1.0)
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "Tr"
                && operation.operands.len() == 1
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 0.0)
        }));
    }

    #[test]
    fn writes_shadow_text_as_passive_offset_text() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 255,
                green: 0,
                blue: 0,
            },
        ];
        let mut style = CharacterStyle::default();
        style.shadow = true;
        style.color_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Shadow".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "ShadowShadow");
        assert_eq!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "Tj")
                .count(),
            2
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone())
                    .is_some_and(|value| (value - 0.35).abs() < 0.001)
                && pdf_number(operation.operands[1].clone()).is_some_and(|value| value == 0.0)
                && pdf_number(operation.operands[2].clone()).is_some_and(|value| value == 0.0)
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 1.0)
                && pdf_number(operation.operands[1].clone()).is_some_and(|value| value == 0.0)
                && pdf_number(operation.operands[2].clone()).is_some_and(|value| value == 0.0)
        }));
    }

    #[test]
    fn writes_relief_text_as_passive_offset_text_layers() {
        let mut document = Document::default();
        document.colors = vec![
            Color::default(),
            Color {
                red: 0,
                green: 0,
                blue: 255,
            },
        ];
        let mut style = CharacterStyle::default();
        style.relief = TextRelief::Emboss;
        style.color_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Relief".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert_eq!(content_text(&content), "ReliefReliefRelief");
        assert_eq!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "Tj")
                .count(),
            3
        );
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 0.75)
                && pdf_number(operation.operands[1].clone()).is_some_and(|value| value == 0.75)
                && pdf_number(operation.operands[2].clone()).is_some_and(|value| value == 1.0)
        }));
        assert!(content.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && pdf_number(operation.operands[0].clone()).is_some_and(|value| value == 0.0)
                && pdf_number(operation.operands[1].clone()).is_some_and(|value| value == 0.0)
                && pdf_number(operation.operands[2].clone())
                    .is_some_and(|value| (value - 0.35).abs() < 0.001)
        }));
    }

    #[test]
    fn encodes_word_symbol_text_as_safe_pdf_fallbacks() {
        let encoded = encode_pdf_text(
            "\u{2018}q\u{2019} \u{201c}qq\u{201d} \u{2014}\u{2013}\u{2011}\u{00ad} \u{2022} \u{00a0}\u{2002}\u{2003}\u{2005}",
        );

        assert_eq!(
            encoded,
            vec![
                0x91, b'q', 0x92, b' ', 0x93, b'q', b'q', 0x94, b' ', 0x97, 0x96, b'-', 0xad, b' ',
                0x95, b' ', 0xa0, b' ', b' ', b' '
            ]
        );
    }

    #[test]
    fn writes_winansi_font_encoding_and_text_bytes() {
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "\u{201c}Quote\u{201d} \u{2014} \u{2022}".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(
            pdf.windows(b"/Encoding /WinAnsiEncoding".len())
                .any(|window| window == b"/Encoding /WinAnsiEncoding")
        );
        assert_eq!(content_bytes(&content), b"\x93Quote\x94 \x97 \x95");
    }

    #[test]
    fn writes_symbol_font_encoding_for_symbol_charset_text() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Helvetica".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Symbol".to_string(),
                alternate_name: None,
                charset: Some(2),
                code_page: None,
                family: FontFamilyHint::Tech,
                pitch: FontPitch::Default,
            },
        ];
        let mut style = CharacterStyle::default();
        style.font_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "\u{03b1}\u{03b2} \u{2022}".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(
            pdf.windows(b"/BaseFont /Symbol".len())
                .any(|window| window == b"/BaseFont /Symbol")
        );
        assert_eq!(content_bytes_for_font(&content, b"F13"), b"ab ");
        assert_eq!(content_bytes_for_font(&content, b"F1"), b"\x95");
    }

    #[test]
    fn supplied_font_assets_can_cover_symbol_family_text() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Helvetica".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "Symbol".to_string(),
                alternate_name: None,
                charset: Some(2),
                code_page: None,
                family: FontFamilyHint::Tech,
                pitch: FontPitch::Default,
            },
        ];
        let mut style = CharacterStyle::default();
        style.font_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "\u{03b1}\u{03b2} ".to_string(),
                style,
            }],
        })];
        let provider = FontProvider {
            assets: vec![FontAsset {
                family_names: vec!["Symbol".to_string()],
                style: FontAssetStyle::default(),
                bytes: include_bytes!("../fixtures/fonts/Tuffy.ttf").to_vec(),
            }],
            limits: Default::default(),
        };

        let layout = LayoutEngine::layout_with_font_provider(&document, Some(&provider));
        let pdf = render_pdf_with_font_provider(&layout, Some(&provider));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();
        let page_fonts = page_font_resource_names(&pdf, 0);

        assert!(
            pdf.windows(b"/Subtype /Type0".len())
                .any(|window| window == b"/Subtype /Type0")
        );
        assert!(
            pdf.windows(b"/FontFile2".len())
                .any(|window| window == b"/FontFile2")
        );
        assert!(!content_bytes_for_font(&content, b"TF1").is_empty());
        assert!(content_bytes_for_font(&content, b"F13").is_empty());
        assert!(page_fonts.iter().any(|name| name == b"TF1"));
        assert!(!page_fonts.iter().any(|name| name == b"F13"));
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn supplied_font_assets_can_cover_zapf_dingbats_family_text() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Helvetica".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "ZapfDingbats".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Tech,
                pitch: FontPitch::Default,
            },
        ];
        let mut style = CharacterStyle::default();
        style.font_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "ABC".to_string(),
                style,
            }],
        })];
        let provider = FontProvider {
            assets: vec![FontAsset {
                family_names: vec!["ZapfDingbats".to_string()],
                style: FontAssetStyle::default(),
                bytes: include_bytes!("../fixtures/fonts/Tuffy.ttf").to_vec(),
            }],
            limits: Default::default(),
        };

        let layout = LayoutEngine::layout_with_font_provider(&document, Some(&provider));
        let pdf = render_pdf_with_font_provider(&layout, Some(&provider));
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();
        let page_fonts = page_font_resource_names(&pdf, 0);

        assert!(!content_bytes_for_font(&content, b"TF1").is_empty());
        assert!(content_bytes_for_font(&content, b"F14").is_empty());
        assert!(page_fonts.iter().any(|name| name == b"TF1"));
        assert!(!page_fonts.iter().any(|name| name == b"F14"));
        audit_passive_pdf_bytes(&pdf).unwrap();
    }

    #[test]
    fn writes_checkbox_dingbats_as_passive_vectors() {
        let mut document = Document::default();
        document.fonts = vec![
            FontDef {
                index: 0,
                name: "Helvetica".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Swiss,
                pitch: FontPitch::Default,
            },
            FontDef {
                index: 1,
                name: "ZapfDingbats".to_string(),
                alternate_name: None,
                charset: None,
                code_page: None,
                family: FontFamilyHint::Tech,
                pitch: FontPitch::Default,
            },
        ];
        let mut style = CharacterStyle::default();
        style.font_index = 1;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "\u{25a1}\u{2610}\u{2611}\u{2612}\u{2713}\u{2717}".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();

        assert!(
            !pdf.windows(b"/BaseFont /ZapfDingbats".len())
                .any(|window| window == b"/BaseFont /ZapfDingbats"),
            "supported checkbox dingbats should not require a viewer ZapfDingbats font"
        );
        assert!(content_bytes_for_font(&content, b"F14").is_empty());
        assert!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "re")
                .count()
                >= 4,
            "checkbox boxes should render as passive rectangles"
        );
        assert!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "m")
                .count()
                >= 3
        );
        assert!(
            content
                .operations
                .iter()
                .filter(|operation| operation.operator == "l")
                .count()
                >= 4
        );
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "S")
        );
    }

    #[test]
    fn writes_passive_kerning_as_positioned_text_without_control_payload() {
        let mut document = Document::default();
        let mut style = CharacterStyle {
            character_kerning_half_points: 2,
            ..Default::default()
        };
        style.font_size_half_points = 24;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "AV".to_string(),
                style,
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_id = *parsed.get_pages().values().next().expect("page");
        let content = parsed.get_and_decode_page_content(page_id).unwrap();
        let positioned = content
            .operations
            .iter()
            .find(|operation| operation.operator == "TJ")
            .expect("kerning should use positioned text");

        assert_eq!(content_bytes(&content), b"AV");
        assert!(
            positioned.operands.iter().any(|operand| {
                operand.as_array().is_ok_and(|items| {
                    items.iter().any(|item| {
                        matches!(item, lopdf::Object::Integer(value) if *value > 0)
                            || matches!(item, lopdf::Object::Real(value) if *value > 0.0)
                    })
                })
            }),
            "positioned text should include a positive kerning adjustment"
        );
        for forbidden in [b"kerning".as_slice(), b"/JavaScript", b"/EmbeddedFile"] {
            assert!(
                !pdf.windows(forbidden.len())
                    .any(|window| window == forbidden)
            );
        }
    }

    #[test]
    fn writes_resolved_page_numbers_without_internal_marker() {
        let mut document = Document::default();
        document.header = vec![Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: format!("{PAGE_NUMBER_MARKER} of {TOTAL_PAGES_MARKER}"),
                style: Default::default(),
            }],
        }];
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "First".to_string(),
                    style: Default::default(),
                }],
            }),
            Block::PageBreak,
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: "Second".to_string(),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_ids = parsed.get_pages().values().copied().collect::<Vec<_>>();
        let first_text = content_text(&parsed.get_and_decode_page_content(page_ids[0]).unwrap());
        let second_text = content_text(&parsed.get_and_decode_page_content(page_ids[1]).unwrap());

        assert!(first_text.contains("1 of 2"));
        assert!(second_text.contains("2 of 2"));
        assert!(!first_text.contains(PAGE_NUMBER_MARKER));
        assert!(!second_text.contains(PAGE_NUMBER_MARKER));
        assert!(!first_text.contains(TOTAL_PAGES_MARKER));
        assert!(!second_text.contains(TOTAL_PAGES_MARKER));
    }

    #[test]
    fn writes_resolved_section_numbers_without_internal_marker() {
        let mut document = Document::default();
        document.blocks = vec![
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: format!("Section {SECTION_NUMBER_MARKER}"),
                    style: Default::default(),
                }],
            }),
            Block::ContinuousSectionBreak,
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: format!("Section {SECTION_NUMBER_MARKER}"),
                    style: Default::default(),
                }],
            }),
            Block::SectionBreak,
            Block::Paragraph(Paragraph {
                style: Default::default(),
                runs: vec![Run {
                    text: format!("Section {SECTION_NUMBER_MARKER}"),
                    style: Default::default(),
                }],
            }),
        ];

        let layout = LayoutEngine::layout(&document);
        let pdf = render_pdf(&layout);
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let page_ids = parsed.get_pages().values().copied().collect::<Vec<_>>();
        let first_text = content_text(&parsed.get_and_decode_page_content(page_ids[0]).unwrap());
        let second_text = content_text(&parsed.get_and_decode_page_content(page_ids[1]).unwrap());

        assert!(first_text.contains("Section 1"));
        assert!(first_text.contains("Section 2"));
        assert!(second_text.contains("Section 3"));
        assert!(!first_text.contains(SECTION_NUMBER_MARKER));
        assert!(!second_text.contains(SECTION_NUMBER_MARKER));
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
                        text.push_str(&String::from_utf8_lossy(bytes));
                    }
                }
            } else if operation.operator == "TJ" {
                for operand in &operation.operands {
                    if let Ok(items) = operand.as_array() {
                        for item in items {
                            if let Ok(bytes) = item.as_str() {
                                text.push_str(&String::from_utf8_lossy(bytes));
                            }
                        }
                    }
                }
            }
        }
        text
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
}
