//! Built-in and supplied font mapping, encoding, CMaps, CID maps, and font objects.

use super::*;

pub(in crate::pdf) fn font_name_for_style(
    family: PdfFontFamily,
    style: &CharacterStyle,
) -> Name<'static> {
    Name(font_resource_for_style(family, style))
}

pub(in crate::pdf) fn font_resource_for_style(
    family: PdfFontFamily,
    style: &CharacterStyle,
) -> &'static [u8] {
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

pub(in crate::pdf) fn passive_to_unicode_cmap(
    name: &'static [u8],
    mappings: &[(u8, char)],
) -> Vec<u8> {
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
    standards_compliant_to_unicode_cmap(cmap.finish().into_vec())
}

pub(in crate::pdf) fn standards_compliant_to_unicode_cmap(mut cmap: Vec<u8>) -> Vec<u8> {
    // pdf-writer 0.15 emits a generic CMapType 0 declaration, while the PDF
    // specification requires a ToUnicode CMap to declare CMapType 2.
    const GENERIC_DECLARATION: &[u8] = b"/CMapType 0 def";
    const TO_UNICODE_DECLARATION: &[u8] = b"/CMapType 2 def";
    let declaration = cmap
        .windows(GENERIC_DECLARATION.len())
        .position(|window| window == GENERIC_DECLARATION)
        .expect("pdf-writer UnicodeCmap declaration");
    cmap[declaration..declaration + GENERIC_DECLARATION.len()]
        .copy_from_slice(TO_UNICODE_DECLARATION);
    cmap
}

pub(in crate::pdf) fn write_supplied_pdf_fonts(
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
        let Ok(face) = Face::parse(asset.bytes(), 0) else {
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
                .cid_to_gid_map_stream(supplied.cid_to_gid_map_ref);
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
        if asset.style().italic {
            flags |= FontFlags::ITALIC;
        }
        if asset.style().bold {
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
            .italic_angle(if asset.style().italic { -12.0 } else { 0.0 })
            .ascent(scale(face.ascender()))
            .descent(scale(face.descender()))
            .cap_height(scale(face.ascender()))
            .stem_v(if asset.style().bold { 120.0 } else { 80.0 })
            .font_file2(supplied.font_file_ref);

        {
            let compressed_font = compress_to_vec_zlib(asset.bytes(), 6);
            let mut font_file = pdf.stream(supplied.font_file_ref, &compressed_font);
            font_file.filter(Filter::FlateDecode);
            font_file.pair(Name(b"Length1"), asset.bytes().len() as i32);
        }

        pdf.stream(
            supplied.cid_to_gid_map_ref,
            &supplied_cid_to_gid_map(&supplied.used_glyphs),
        );

        let mut cmap =
            UnicodeCmap::<u16>::new(Name(&supplied.base_name), supplied_cid_system_info());
        for glyph in &supplied.used_glyphs {
            cmap.pair(glyph.cid, glyph.unicode);
        }
        let cmap = standards_compliant_to_unicode_cmap(cmap.finish().into_vec());
        pdf.stream(supplied.to_unicode_ref, &cmap);
    }
}

pub(in crate::pdf) fn supplied_cid_to_gid_map(glyphs: &[SuppliedGlyph]) -> Vec<u8> {
    let max_cid = glyphs
        .iter()
        .map(|glyph| usize::from(glyph.cid))
        .max()
        .unwrap_or(0);
    let mut map = vec![0; (max_cid + 1) * 2];
    for glyph in glyphs {
        let offset = usize::from(glyph.cid) * 2;
        map[offset..offset + 2].copy_from_slice(&glyph.gid.to_be_bytes());
    }
    map
}

pub(in crate::pdf) fn supplied_cid_system_info() -> SystemInfo<'static> {
    SystemInfo {
        registry: Str(b"Adobe"),
        ordering: Str(b"Identity"),
        supplement: 0,
    }
}
