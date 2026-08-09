use super::*;

#[test]
fn fields_render_result_without_executing_instruction() {
    let parsed = parse_rtf_bytes(&field_with_link_result()).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("visible link"));
    assert!(!text.contains("HYPERLINK"));
    assert!(!text.contains("https://example.com"));
}

#[test]
fn advance_field_offsets_passively_without_instruction_or_marker_pdf_leakage() {
    let input = br#"{\rtf1 Before {\field{\*\fldinst ADVANCE \\r 240 \\d 120}}After\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert!(text.contains(PASSIVE_ADVANCE_MARKER));
    let advance = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Paragraph(paragraph) => {
                paragraph
                    .runs
                    .iter()
                    .find_map(|run| match run.dynamic_field {
                        Some(InlineKind::LayoutAdvance(advance)) => Some(advance),
                        _ => None,
                    })
            }
            _ => None,
        })
        .expect("passive advance marker");
    assert_eq!(advance.horizontal_twips, 240);
    assert_eq!(advance.vertical_twips, -120);
    for forbidden in ["ADVANCE", "fldinst", "\\r", "\\d"] {
        assert!(
            !text.contains(forbidden),
            "ADVANCE field instruction leaked to normalized text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "layout field ADVANCE interpreted as bounded passive horizontal cursor advance",
        )
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("layout field ADVANCE interpreted as bounded passive vertical cursor advance")
    }));

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(!rendered_text.contains(PASSIVE_ADVANCE_MARKER));
    for forbidden in [
        b"ADVANCE".as_slice(),
        b"fldinst",
        PASSIVE_ADVANCE_MARKER.as_bytes(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "ADVANCE field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn field_instruction_nested_model_content_does_not_escape_to_visible_output() {
    let input = br#"{\rtf1 Before {\field{\*\fldinst HYPERLINK "https://example.com" {\pict\wmetafile8\picwgoal720\pichgoal720 01020304}{\object\objw2160\objh720\objdata 4142432f4a617661536372697074}{\shp{\shpinst\shpleft720\shptop720\shpright2160\shpbottom1440{\sp{\sn shapeType}{\sv 1}}}{\shptxt Hidden shape text\par}}}} After\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before"));
    assert!(text.contains("[Field removed: no passive result]"));
    assert!(text.contains("After"));
    assert!(
        !parsed
            .document
            .blocks
            .iter()
            .any(|block| matches!(block, Block::Image(_) | Block::Shape(_))),
        "field instruction nested model content escaped into document blocks: {:?}",
        parsed.document.blocks
    );
    for forbidden in [
        "HYPERLINK",
        "https://example.com",
        "pict",
        "wmetafile",
        "object",
        "objdata",
        "414243",
        "JavaScript",
        "shapeType",
        "Hidden shape text",
    ] {
        assert!(
            !text.contains(forbidden),
            "field instruction nested content leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("[Field removed: no passive result]"));
    assert!(rendered_text.contains("After"));
    for forbidden in [
        b"HYPERLINK".as_slice(),
        b"https://example.com",
        b"pict",
        b"wmetafile",
        b"Image skipped",
        b"Embedded object removed",
        b"object",
        b"objdata",
        b"414243",
        b"JavaScript",
        b"shapeType",
        b"Hidden shape text",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "field instruction nested content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn stored_field_result_survives_when_instruction_contains_nested_active_payload() {
    let input = br#"{\rtf1 Before {\field{\*\fldinst HYPERLINK "https://example.com" {\pict\wmetafile8\picwgoal720\pichgoal720 01020304}{\object\objw2160\objh720\objdata 4142432f4a617661536372697074}{\shp{\shpinst\shpleft720\shptop720\shpright2160\shpbottom1440{\sp{\sn shapeType}{\sv 1}}}{\shptxt Hidden shape text\par}}}{\fldrslt visible link}} After\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before visible link After"));
    assert!(!text.contains("[Field removed: no passive result]"));
    assert!(
        !parsed
            .document
            .blocks
            .iter()
            .any(|block| matches!(block, Block::Image(_) | Block::Shape(_))),
        "field instruction nested model content escaped despite stored result: {:?}",
        parsed.document.blocks
    );
    for forbidden in [
        "HYPERLINK",
        "https://example.com",
        "pict",
        "wmetafile",
        "object",
        "objdata",
        "414243",
        "JavaScript",
        "shapeType",
        "Hidden shape text",
    ] {
        assert!(
            !text.contains(forbidden),
            "stored field leaked nested instruction content to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before visible link After"));
    assert!(!rendered_text.contains("[Field removed: no passive result]"));
    for forbidden in [
        b"HYPERLINK".as_slice(),
        b"https://example.com",
        b"pict",
        b"wmetafile",
        b"Image skipped",
        b"Embedded object removed",
        b"object",
        b"objdata",
        b"414243",
        b"JavaScript",
        b"shapeType",
        b"Hidden shape text",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "stored field leaked nested instruction content to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn field_instruction_nested_structural_destinations_do_not_mutate_safe_model() {
    let input = br#"{\rtf1 Before {\field{\*\fldinst HYPERLINK "https://example.com" {\fonttbl{\f77 HiddenFont;}}{\colortbl;\red1\green2\blue3;}{\stylesheet{\s9 HiddenStyle;}}{\*\bkmkstart HiddenBookmark}Hidden bookmark payload{\*\bkmkend HiddenBookmark}{\*\listtable{\list\listtemplateid1{\listlevel\levelnfc23{\leveltext\'01?;}{\levelnumbers;}}\listid99}}}} ref {\field{\*\fldinst REF HiddenBookmark}} style {\field{\*\fldinst STYLEREF HiddenStyle}} After\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before"));
    assert!(text.contains("ref"));
    assert!(text.contains("style"));
    assert!(text.contains("After"));
    assert!(text.contains("[Field removed: no passive result]"));
    assert!(
        parsed
            .document
            .fonts
            .iter()
            .all(|font| font.name != "HiddenFont"),
        "field instruction font table mutated normalized fonts: {:?}",
        parsed.document.fonts
    );
    assert!(
        parsed
            .document
            .colors
            .iter()
            .all(|color| { !(color.red == 1 && color.green == 2 && color.blue == 3) }),
        "field instruction color table mutated normalized colors: {:?}",
        parsed.document.colors
    );
    for forbidden in [
        "HYPERLINK",
        "https://example.com",
        "HiddenFont",
        "HiddenStyle",
        "HiddenBookmark",
        "Hidden bookmark payload",
        "listtemplateid",
        "leveltext",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "field instruction structural content leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("ref"));
    assert!(rendered_text.contains("style"));
    assert!(rendered_text.contains("After"));
    assert!(rendered_text.contains("[Field removed: no passive result]"));
    for forbidden in [
        b"HYPERLINK".as_slice(),
        b"https://example.com",
        b"HiddenFont",
        b"HiddenStyle",
        b"HiddenBookmark",
        b"Hidden bookmark payload",
        b"listtemplateid",
        b"leveltext",
        b"fldinst",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "field instruction structural content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn field_status_flags_do_not_update_execute_or_leak_into_passive_pdf() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field",
        "\\",
        "flddirty",
        "\\",
        "fldedit",
        "\\",
        "fldlock",
        "\\",
        "fldpriv{",
        "\\",
        "*",
        "\\",
        "fldinst HYPERLINK \"https://example.com/locked\"}{",
        "\\",
        "fldrslt Stored locked link}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before Stored locked link After"));
    assert!(!text.contains("HYPERLINK"));
    assert!(!text.contains("https://example.com"));
    for forbidden in ["flddirty", "fldedit", "fldlock", "fldpriv"] {
        assert!(
            !text.contains(forbidden),
            "field status flag leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "field status flags should not be unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            parse_options: RtfParseOptions {
                pdf_link_policy: PdfLinkPolicy::AllowSanitizedHttpLinks,
                ..RtfParseOptions::default()
            },
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before Stored locked link After"));
    for forbidden in [
        b"flddirty".as_slice(),
        b"fldedit",
        b"fldlock",
        b"fldpriv",
        b"HYPERLINK",
        b"https://example.com",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "field status flag or active PDF content leaked: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn header_field_results_and_placeholders_render_passively_without_body_flow_or_instruction_leakage()
{
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "header Logo {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst HYPERLINK \"https://example.com/click\"}{",
        "\\",
        "fldrslt Stored link}} {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst INCLUDEPICTURE \"https://example.com/pixel.png\"}}",
        "\\",
        "par} Body",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let body_text = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(
                paragraph
                    .runs
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<String>(),
            ),
            Block::Placeholder(text) => Some(text.clone()),
            _ => None,
        })
        .collect::<String>();

    assert!(text.contains("Logo"));
    assert!(text.contains("Stored link"));
    assert!(text.contains("[Field removed: no passive result]"));
    assert_eq!(body_text.trim(), "Body");
    for forbidden in [
        "HYPERLINK",
        "INCLUDEPICTURE",
        "https://example.com",
        "fldinst",
        "fldrslt",
    ] {
        assert!(
            !text.contains(forbidden),
            "field instruction leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Logo"));
    assert!(rendered_text.contains("Stored link"));
    assert!(rendered_text.contains("[Field removed: no passive result]"));
    assert!(rendered_text.contains("Body"));
    for forbidden in [
        b"HYPERLINK".as_slice(),
        b"INCLUDEPICTURE",
        b"https://example.com",
        b"fldinst",
        b"fldrslt",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "field instruction or active PDF content leaked: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn docproperty_fields_render_metadata_without_leaking_nested_active_content() {
    let input = br#"{\rtf1{\info{\title Safe title {\title Nested overwrite}{\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt Hidden link}} tail}{\author Alice}{\doccomm Hidden comment}}Doc {\field{\*\fldinst DOCPROPERTY Title}} / {\field{\*\fldinst DOCPROPERTY Author}}\par}"#.to_vec();
    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Doc Safe title"));
    assert!(rendered_text.contains("tail / Alice"));
    for forbidden in [
        b"DOCPROPERTY".as_slice(),
        b"HYPERLINK",
        b"https://example.com",
        b"Nested overwrite",
        b"Hidden link",
        b"Hidden comment",
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "document property field leaked active PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn docproperty_timestamp_aliases_render_metadata_without_instruction_or_payload_leakage() {
    let input = br#"{\rtf1{\info{\creatim\yr2024\mo7\dy5\hr14\min30\sec9}{\revtim\yr2025\mo1\dy2\hr9\min4\sec5}{\printim\yr2026\mo12\dy31}}Created {\field{\*\fldinst DOCPROPERTY "Creation Date" \\@ "MMMM d, yyyy"}} saved {\field{\*\fldinst DOCPROPERTY "Last Save Time" \\@ "yyyy-MM-dd h:mm A/P"}} printed {\field{\*\fldinst DOCPROPERTY "Last Printed"}} malformed {\field{\*\fldinst DOCPROPERTY "Last Saved Date" \\@ "bad-token"}}\par}"#.to_vec();
    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Created July 5, 2024"));
    assert!(rendered_text.contains("saved 2025-01-02 9:04 A"));
    assert!(rendered_text.contains("printed 2026-12-31 00:00:00"));
    assert!(rendered_text.contains("malformed [Field removed: no passive result]"));
    for forbidden in [
        b"DOCPROPERTY".as_slice(),
        b"Creation Date",
        b"Last Save Time",
        b"Last Printed",
        b"Last Saved Date",
        b"bad-token",
        b"creatim",
        b"revtim",
        b"printim",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "DOCPROPERTY timestamp alias leaked active PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn docproperty_stat_aliases_resolve_layout_stats_without_instruction_leakage() {
    let input = br#"{\rtf1 Alpha beta\page Gamma stats pages {\field{\*\fldinst DOCPROPERTY "Number of Pages"}} words {\field{\*\fldinst DOCPROPERTY "Number of Words"}} chars {\field{\*\fldinst DOCPROPERTY "Number of Characters"}} spaces {\field{\*\fldinst DOCPROPERTY "Number of Characters With Spaces"}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains(TOTAL_PAGES_MARKER));
    assert!(text.contains(DOCUMENT_WORDS_MARKER));
    assert!(text.contains(DOCUMENT_CHARS_WITH_SPACES_MARKER));
    for forbidden in [
        "DOCPROPERTY",
        "Number of Pages",
        "Number of Words",
        "Number of Characters",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "DOCPROPERTY stat alias leaked instruction text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 2);
    let page_texts = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| {
            let content = parsed_pdf
                .get_and_decode_page_content(*page_id)
                .expect("page content");
            decoded_pdf_text(&content)
        })
        .collect::<Vec<_>>();
    let rendered_text = page_texts.join("\n");

    assert!(
        rendered_text.contains("stats pages 2 words"),
        "decoded PDF text did not resolve DOCPROPERTY page count: {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("chars"),
        "decoded PDF text omitted DOCPROPERTY character stat label: {rendered_text:?}"
    );
    for forbidden in [
        b"DOCPROPERTY".as_slice(),
        b"Number of Pages",
        b"Number of Words",
        b"Number of Characters",
        b"fldinst",
        TOTAL_PAGES_MARKER.as_bytes(),
        DOCUMENT_WORDS_MARKER.as_bytes(),
        DOCUMENT_CHARS_MARKER.as_bytes(),
        DOCUMENT_CHARS_WITH_SPACES_MARKER.as_bytes(),
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "DOCPROPERTY stat alias leaked active/internal PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn stored_docproperty_stat_metadata_renders_passively_without_control_leakage() {
    let input = br##"{\rtf1{\info\nofpages7\nofwords1234\nofchars5678\nofcharsws6789}Stats pages {\field{\*\fldinst DOCPROPERTY "Number of Pages" \\# "000"}} words {\field{\*\fldinst DOCPROPERTY "Number of Words" \\# "#,##0"}} chars {\field{\*\fldinst INFO "Number of Characters"}} spaces {\field{\*\fldinst INFO "Number of Characters With Spaces"}}\par}"##.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains("Stats pages 007 words 1,234 chars 5678 spaces 6789"),
        "stored metadata stats were not rendered passively: {text:?}"
    );
    for forbidden in [
        "DOCPROPERTY",
        "INFO",
        "Number of Pages",
        "Number of Words",
        "Number of Characters",
        "nofpages",
        "nofwords",
        "nofchars",
        "nofcharsws",
        "fldinst",
        TOTAL_PAGES_MARKER,
        DOCUMENT_WORDS_MARKER,
        DOCUMENT_CHARS_MARKER,
        DOCUMENT_CHARS_WITH_SPACES_MARKER,
    ] {
        assert!(
            !text.contains(forbidden),
            "stored stat metadata leaked unsafe text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.contains("Stats pages 007 words 1,234 chars 5678 spaces 6789"),
        "stored metadata stats did not reach passive PDF text: {rendered_text:?}"
    );
    for forbidden in [
        b"DOCPROPERTY".as_slice(),
        b"Number of Pages",
        b"Number of Words",
        b"Number of Characters",
        b"nofpages",
        b"nofwords",
        b"nofchars",
        b"nofcharsws",
        b"fldinst",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden stored stat metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn docproperty_numeric_aliases_render_metadata_without_instruction_leakage() {
    let input = br##"{\rtf1{\info{\version42}{\edmins17}\nofparas3\noflines12\nofbytes4096}Revision {\field{\*\fldinst DOCPROPERTY "Revision Number" \\* ROMAN}} edit {\field{\*\fldinst DOCPROPERTY "Total Editing Time" \\# "0000"}} paragraphs {\field{\*\fldinst DOCPROPERTY "Number of Paragraphs"}} lines {\field{\*\fldinst DOCPROPERTY "Number of Lines" \\# "000"}} bytes {\field{\*\fldinst DOCPROPERTY "Number of Bytes" \\# "#,##0"}} missing {\field{\*\fldinst DOCPROPERTY "Editing Time"}}\par}"##.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Revision XLII edit 0017 paragraphs 3 lines 012 bytes 4,096 missing 17"));
    for forbidden in [
        "DOCPROPERTY",
        "Revision Number",
        "Total Editing Time",
        "Number of Paragraphs",
        "Number of Lines",
        "Number of Bytes",
        "Editing Time",
        "version",
        "edmins",
        "nofparas",
        "noflines",
        "nofbytes",
        "fldinst",
        "[Field removed",
    ] {
        assert!(
            !text.contains(forbidden),
            "DOCPROPERTY numeric alias leaked instruction text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text
            .contains("Revision XLII edit 0017 paragraphs 3 lines 012 bytes 4,096 missing 17"),
        "decoded PDF text did not contain passive numeric DOCPROPERTY values: {rendered_text:?}"
    );
    for forbidden in [
        b"DOCPROPERTY".as_slice(),
        b"Revision Number",
        b"Total Editing Time",
        b"Number of Paragraphs",
        b"Number of Lines",
        b"Number of Bytes",
        b"Editing Time",
        b"version",
        b"edmins",
        b"nofparas",
        b"noflines",
        b"nofbytes",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "DOCPROPERTY numeric alias leaked active/internal PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn shortcut_document_property_fields_render_metadata_without_instruction_leakage() {
    let input = br#"{\rtf1{\info{\title Safe title {\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt Hidden link}} tail}{\author Alice}{\operator Bob}{\doccomm Comment text}}Doc {\field{\*\fldinst TITLE}} / {\field{\*\fldinst AUTHOR \\* Upper}} / {\field{\*\fldinst LASTSAVEDBY}} / {\field{\*\fldinst COMMENTS}}\par}"#.to_vec();
    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Doc Safe title"));
    assert!(rendered_text.contains("tail / ALICE / Bob / Comment text"));
    for forbidden in [
        b"TITLE".as_slice(),
        b"AUTHOR",
        b"LASTSAVEDBY",
        b"COMMENTS",
        b"HYPERLINK",
        b"https://example.com",
        b"Hidden link",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "shortcut document property field leaked active PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn extended_shortcut_document_property_fields_render_metadata_without_instruction_leakage() {
    let input = br#"{\rtf1{\info{\manager Alice Manager}{\company Contoso \u937? {\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt Hidden link}} tail}{\category Internal Use}{\doccomm Hidden comment}}Mgr {\field{\*\fldinst MANAGER}} / {\field{\*\fldinst COMPANY \\* Upper}} / {\field{\*\fldinst CATEGORY}}\par}"#.to_vec();
    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Mgr Alice Manager / CONTOSO"));
    assert!(rendered_text.contains("TAIL / Internal Use"));
    for forbidden in [
        b"MANAGER".as_slice(),
        b"COMPANY",
        b"CATEGORY",
        b"HYPERLINK",
        b"https://example.com",
        b"Hidden link",
        b"Hidden comment",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "extended shortcut document property field leaked active PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn info_fields_render_metadata_without_instruction_or_active_payload_leakage() {
    let input = br#"{\rtf1{\info{\title Safe title {\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt Hidden link}} tail}{\author Alice}{\operator Bob}{\doccomm Comment text}{\hlinkbase https://example.test/base}}Doc {\field{\*\fldinst INFO Title}} / {\field{\*\fldinst INFO Author \\* Upper}} / {\field{\*\fldinst INFO "Last Author"}} / {\field{\*\fldinst INFO "Last Modified By" \\* Upper}} / {\field{\*\fldinst INFO Comments}} / {\field{\*\fldinst INFO "Hyperlink Base"}} / {\field{\*\fldinst INFO Filename}}\par}"#.to_vec();
    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Doc Safe title"));
    assert!(
        rendered_text
            .contains("tail / ALICE / Bob / BOB / Comment text / https://example.test/base")
    );
    assert!(rendered_text.contains("[Field removed: no passive result]"));
    for forbidden in [
        b"INFO".as_slice(),
        b"Last Author",
        b"Last Modified By",
        b"Hyperlink Base",
        b"hlinkbase",
        b"Filename",
        b"HYPERLINK",
        b"https://example.com",
        b"Hidden link",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "INFO field leaked active PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn docproperty_fields_render_custom_properties_without_leaking_linked_or_active_content() {
    let input = br#"{\rtf1{\*\userprops{\propname Client Name}{\proptype30}{\staticval Contoso {\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt Hidden link}} tail}{\linkval Hidden linked value}}Client {\field{\*\fldinst DOCPROPERTY "Client Name"}}\par}"#.to_vec();
    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Client Contoso"));
    assert!(rendered_text.contains("tail"));
    for forbidden in [
        b"DOCPROPERTY".as_slice(),
        b"Client Name",
        b"HYPERLINK",
        b"https://example.com",
        b"Hidden link",
        b"Hidden linked value",
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "custom document property field leaked active PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn metadata_field_values_reject_active_markers_before_pdf_output() {
    let input = br#"{\rtf1{\*\userprops{\propname Client Name}{\proptype30}{\staticval PAYLOAD /JavaScript}}{\*\docvar {Client Var}{PAYLOAD /EmbeddedFile}}Property {\field{\*\fldinst DOCPROPERTY "Client Name"}} variable {\field{\*\fldinst DOCVARIABLE "Client Var"}}\par}"#.to_vec();
    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Property [Field removed: no passive result]"));
    assert!(rendered_text.contains("variable [Field removed: no passive result]"));
    for forbidden in [
        b"PAYLOAD".as_slice(),
        b"Client Name",
        b"Client Var",
        b"DOCPROPERTY",
        b"DOCVARIABLE",
        b"staticval",
        b"docvar",
        b"fldinst",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "metadata field active marker content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn metadata_field_values_reject_form_and_signature_markers_before_pdf_output() {
    let input = br#"{\rtf1{\*\userprops{\propname Client Name}{\proptype30}{\staticval PAYLOAD /FDF /NeedAppearances /XDP /XFA}}{\*\docvar {Client Var}{PAYLOAD /SigFlags /DocMDP}}Property {\field{\*\fldinst DOCPROPERTY "Client Name"}} variable {\field{\*\fldinst DOCVARIABLE "Client Var"}}\par}"#.to_vec();
    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Property [Field removed: no passive result]"));
    assert!(rendered_text.contains("variable [Field removed: no passive result]"));
    for forbidden in [
        b"PAYLOAD".as_slice(),
        b"Client Name",
        b"Client Var",
        b"DOCPROPERTY",
        b"DOCVARIABLE",
        b"staticval",
        b"docvar",
        b"fldinst",
        b"/FDF",
        b"/NeedAppearances",
        b"/XDP",
        b"/XFA",
        b"/SigFlags",
        b"/DocMDP",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "metadata field form/signature marker content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
    audit_passive_pdf_bytes(&output.pdf).unwrap();
}

#[test]
fn docvariable_fields_render_metadata_without_leaking_nested_active_content() {
    let input = br#"{\rtf1{\*\docvar {Client Name}{Contoso {\field{\*\fldinst HYPERLINK "https://example.com/docvar"}{\fldrslt Hidden link}} tail}}Client {\field{\*\fldinst DOCVARIABLE "Client Name"}} missing {\field{\*\fldinst DOCVARIABLE Missing}}\par}"#.to_vec();
    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Client Contoso"));
    assert!(rendered_text.contains("tail"));
    assert!(rendered_text.contains("missing [Field removed: no passive result]"));
    for forbidden in [
        b"DOCVARIABLE".as_slice(),
        b"docvar",
        b"Client Name",
        b"HYPERLINK",
        b"https://example.com",
        b"Hidden link",
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "document variable field leaked active PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn hex_docvariable_metadata_renders_without_raw_or_active_pdf_leakage() {
    let input = br#"{\rtf1{\*\docvar {Client}{Con\'74oso {\field{\*\fldinst HYPERLINK "https://example.com/docvar-hex"}{\fldrslt Hidden link}} ta\'69l}}Client {\field{\*\fldinst DOCVARIABLE Client}}\par}"#.to_vec();
    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.contains("Client Contoso  tail"),
        "hex-escaped document variable text should render passively: {rendered_text:?}"
    );
    for forbidden in [
        b"DOCVARIABLE".as_slice(),
        b"docvar",
        b"fldinst",
        b"HYPERLINK",
        b"https://example.com",
        b"Hidden link",
        b"\\'74",
        b"\\'69",
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "hex document variable field leaked active PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn date_fields_use_stored_results_and_never_update_or_leak_instructions() {
    let stored = parse_rtf_bytes(&rtf(&[
        "{",
        "\\",
        "rtf1 Created {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst DATE \\\\@ \"current-date-sentinel\"}{",
        "\\",
        "fldrslt Stored visible date}}",
        "\\",
        "par}",
    ]))
    .unwrap();
    let stored_text = collect_text(&stored.document);
    assert!(stored_text.contains("Created Stored visible date"));
    assert!(!stored_text.contains("DATE"));
    assert!(!stored_text.contains("current-date-sentinel"));
    assert!(!stored_text.contains("fldinst"));
    assert!(!stored_text.contains("[Field removed"));

    let resultless = parse_rtf_bytes(&rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst DATE \\\\@ \"current-date-sentinel\"}} After",
        "\\",
        "par}",
    ]))
    .unwrap();
    let resultless_text = collect_text(&resultless.document);
    assert!(resultless_text.contains("Before"));
    assert!(resultless_text.contains("Before  After"));
    assert!(!resultless_text.contains("[Field removed: no passive result]"));
    assert!(resultless_text.contains("After"));
    assert!(!resultless_text.contains("DATE"));
    assert!(!resultless_text.contains("current-date-sentinel"));
    assert!(resultless.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "resultless field DATE omitted for Word-compatible passive rendering without dynamic evaluation",
        )
    }));
    assert!(resultless.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("dynamic field DATE removed without reading converter clock")
    }));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("date-field.rtf");
    let output_path = dir.path().join("date-field.pdf");
    fs::write(
        &input_path,
        rtf(&[
            "{",
            "\\",
            "rtf1 Before {",
            "\\",
            "field{",
            "\\",
            "*",
            "\\",
            "fldinst DATE \\\\@ \"current-date-sentinel\"}} After",
            "\\",
            "par}",
        ]),
    )
    .unwrap();
    convert_rtf_file_to_pdf(&input_path, &output_path, &ConvertOptions::default()).unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"fldinst".as_slice(),
        b"current-date-sentinel",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden date-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn metadata_timestamp_fields_render_passively_without_instruction_or_metadata_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "info{",
        "\\",
        "creatim",
        "\\",
        "yr2024",
        "\\",
        "mo7",
        "\\",
        "dy5",
        "\\",
        "hr14",
        "\\",
        "min30",
        "\\",
        "sec9}{",
        "\\",
        "revtim",
        "\\",
        "yr2025",
        "\\",
        "mo1",
        "\\",
        "dy2",
        "\\",
        "hr9",
        "\\",
        "min4",
        "\\",
        "sec5}}Created {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst CREATEDATE \\\\@ \"dddd, MMMM d, yyyy 'at' h:mm A/P\"}} saved {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SAVEDATE \\\\@ \"ddd yyyy-MM-dd h:mm a/p\"}} dynamic {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst DATE \\\\@ \"current-date-sentinel\"}}",
        "\\",
        "par}",
    ]);
    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            parse_options: RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Created Friday, July 5, 2024 at 2:30 P"));
    assert!(rendered_text.contains("saved Thu 2025-01-02 9:04 a"));
    assert!(rendered_text.contains("[Field removed: no passive result]"));
    for forbidden in [
        b"fldinst".as_slice(),
        b"CREATEDATE",
        b"SAVEDATE",
        b"current-date-sentinel",
        b"creatim",
        b"revtim",
        b"yr2024",
        b"MMMM",
        b"yyyy",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden metadata timestamp content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_page_count_fields_render_without_executing_field_instruction() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Page {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst PAGE}} of {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst NUMPAGES}}",
        "\\",
        "page second page",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains(PAGE_NUMBER_MARKER));
    assert!(text.contains(TOTAL_PAGES_MARKER));
    assert!(!text.contains("fldinst"));
    assert!(!text.contains("PAGE"));
    assert!(!text.contains("NUMPAGES"));
    assert!(!text.contains("[Field removed"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("page-count-field.rtf");
    let output_path = dir.path().join("page-count-field.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"fldinst".as_slice(),
        b"NUMPAGES",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn resultless_document_stat_fields_render_passive_counts_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 One two. Three Words:{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst NUMWORDS}} Chars:{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst NUMCHARS}} WS:{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst NUMCHARSWS}}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains(DOCUMENT_WORDS_MARKER));
    assert!(text.contains(DOCUMENT_CHARS_MARKER));
    assert!(text.contains(DOCUMENT_CHARS_WITH_SPACES_MARKER));
    for forbidden in [
        "NUMWORDS",
        "NUMCHARS",
        "NUMCHARSWS",
        "fldinst",
        "[Field removed",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden document stat field content leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("One two. Three Words:6 Chars:27 WS:32"),
        "decoded PDF text did not contain passive document stat counts: {rendered_text:?}"
    );
    for forbidden in [
        b"NUMWORDS".as_slice(),
        b"NUMCHARS",
        b"NUMCHARSWS",
        b"fldinst",
        DOCUMENT_WORDS_MARKER.as_bytes(),
        DOCUMENT_CHARS_MARKER.as_bytes(),
        DOCUMENT_CHARS_WITH_SPACES_MARKER.as_bytes(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden document stat field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn document_stat_fields_ignore_unused_header_footer_variants() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "headerf HiddenFirst",
        "\\",
        "par}{",
        "\\",
        "headerl HiddenEven",
        "\\",
        "par}{",
        "\\",
        "footerf HiddenFooterFirst",
        "\\",
        "par}{",
        "\\",
        "footerl HiddenFooterEven",
        "\\",
        "par}Body Words:{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst NUMWORDS}}",
        "\\",
        "par}",
    ]);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.contains("Body Words:2"),
        "unused first/even header/footer variants should not change NUMWORDS; got {rendered_text:?}"
    );
    assert!(
        !rendered_text.contains("Body Words:6"),
        "unused header/footer variant text changed passive NUMWORDS: {rendered_text:?}"
    );
    for forbidden in [
        b"HiddenFirst".as_slice(),
        b"HiddenEven",
        b"HiddenFooterFirst",
        b"HiddenFooterEven",
        b"NUMWORDS",
        b"fldinst",
        b"headerf",
        b"headerl",
        b"footerf",
        b"footerl",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "unused header/footer stat field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn document_stat_fields_ignore_unused_later_section_even_variants() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 First",
        "\\",
        "par",
        "\\",
        "sbkodd",
        "\\",
        "sect",
        "\\",
        "sectd{",
        "\\",
        "headerl HiddenSectionEven",
        "\\",
        "par}{",
        "\\",
        "footerl HiddenSectionFooterEven",
        "\\",
        "par}Second Words:{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst NUMWORDS}}",
        "\\",
        "par}",
    ]);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = parsed_pdf
        .get_pages()
        .values()
        .last()
        .copied()
        .expect("last page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.contains("Second Words:3"),
        "unused later-section even variants should not change NUMWORDS; got {rendered_text:?}"
    );
    assert!(
        !rendered_text.contains("Second Words:5"),
        "unused later-section even variant text changed passive NUMWORDS: {rendered_text:?}"
    );
    for forbidden in [
        b"HiddenSectionEven".as_slice(),
        b"HiddenSectionFooterEven",
        b"NUMWORDS",
        b"fldinst",
        b"headerl",
        b"footerl",
        b"sbkodd",
        b"sectd",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "unused later-section stat field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn document_stat_fields_count_visible_section_fallback_header_footer_variants() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "headerf Fallback First Header",
        "\\",
        "par}{",
        "\\",
        "footerf Fallback First Footer",
        "\\",
        "par}First",
        "\\",
        "par",
        "\\",
        "sect",
        "\\",
        "sectd",
        "\\",
        "titlepg Second Words:{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst NUMWORDS}}",
        "\\",
        "par}",
    ]);

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = parsed_pdf
        .get_pages()
        .values()
        .last()
        .copied()
        .expect("last page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.contains("Fallback First Header"),
        "section title page should render document first-header fallback; got {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("Fallback First Footer"),
        "section title page should render document first-footer fallback; got {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("Second Words:9"),
        "visible section fallback header/footer variants should count in NUMWORDS; got {rendered_text:?}"
    );
    assert!(
        !rendered_text.contains("Second Words:3"),
        "visible section fallback header/footer variants were omitted from NUMWORDS: {rendered_text:?}"
    );
    for forbidden in [
        b"NUMWORDS".as_slice(),
        b"fldinst",
        b"headerf",
        b"footerf",
        b"titlepg",
        b"sectd",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "section fallback stat field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn document_stat_fields_count_visible_header_shape_text() {
    let input = br#"{\rtf1{\header{\shp{\shpinst\shpleft720\shptop720\shpright4320\shpbottom1800{\sp{\sn shapeType}{\sv 1}}{\sp{\sn pFragments}{\sv hostile-shape-stat-payload}}}{\shptxt Header Box\par}}\par}Body Words:{\field{\*\fldinst NUMWORDS}}\par}"#.to_vec();

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.contains("Header Box"),
        "header shape text should render visibly; got {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("Body Words:4"),
        "visible header shape text should count in NUMWORDS; got {rendered_text:?}"
    );
    assert!(
        !rendered_text.contains("Body Words:2"),
        "visible header shape text was omitted from NUMWORDS: {rendered_text:?}"
    );
    for forbidden in [
        b"NUMWORDS".as_slice(),
        b"fldinst",
        b"shpinst",
        b"shapeType",
        b"pFragments",
        b"hostile-shape-stat-payload",
        b"shptxt",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "header shape stat field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn document_stat_fields_count_visible_background_shape_text() {
    let input = br#"{\rtf1{\background{\shp{\shpinst\shpleft720\shptop720\shpright4320\shpbottom1800{\sp{\sn shapeType}{\sv 1}}{\sp{\sn pFragments}{\sv hostile-background-stat-payload}}}{\shptxt Water Mark\par}}}Body Words:{\field{\*\fldinst NUMWORDS}}\par}"#.to_vec();

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.contains("Water Mark"),
        "background shape text should render visibly; got {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("Body Words:4"),
        "visible background shape text should count in NUMWORDS; got {rendered_text:?}"
    );
    assert!(
        !rendered_text.contains("Body Words:2"),
        "visible background shape text was omitted from NUMWORDS: {rendered_text:?}"
    );
    for forbidden in [
        b"NUMWORDS".as_slice(),
        b"fldinst",
        b"background",
        b"shpinst",
        b"shapeType",
        b"pFragments",
        b"hostile-background-stat-payload",
        b"shptxt",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "background shape stat field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn document_stat_fields_count_visible_image_placeholder_text() {
    let input = br"{\rtf1{\pict\emfblip\picw100\pich50\picwgoal2160\pichgoal720 41424344}Body Words:{\field{\*\fldinst NUMWORDS}}\par}".to_vec();

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.contains("Image skipped"),
        "unsupported image placeholder text should render visibly; got {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("Body Words:4"),
        "visible image placeholder text should count in NUMWORDS; got {rendered_text:?}"
    );
    assert!(
        !rendered_text.contains("Body Words:2"),
        "visible image placeholder text was omitted from NUMWORDS: {rendered_text:?}"
    );
    for forbidden in [
        b"NUMWORDS".as_slice(),
        b"fldinst",
        b"pict",
        b"emfblip",
        b"41424344",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "image placeholder stat field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn document_stat_fields_count_visible_wmf_vector_text() {
    let wmf_hex = concat!(
        "0100090000032d00000001000c0000000000",
        "050000000c026400c800",
        "050000000902ff000000",
        "0c000000fb02f4ff00000000000000000000000000000000",
        "040000002d010000",
        "0700000021050200486914002800",
        "030000000000",
    );
    let input = format!(
        "{{\\rtf1{{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}}Body Words:{{\\field{{\\*\\fldinst NUMWORDS}}}}\\par}}"
    )
    .into_bytes();

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.contains("Hi"),
        "visible WMF vector text should render in passive PDF text; got {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("Body Words:3"),
        "visible WMF vector text should count in NUMWORDS; got {rendered_text:?}"
    );
    assert!(
        !rendered_text.contains("Body Words:2"),
        "visible WMF vector text was omitted from NUMWORDS: {rendered_text:?}"
    );
    for forbidden in [
        b"NUMWORDS".as_slice(),
        b"fldinst",
        b"pict",
        b"wmetafile",
        b"0521",
        b"fb02",
        b"ff0000",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "WMF vector stat field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn section_page_count_fields_render_passively_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 S1 page {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SECTIONPAGES}}",
        "\\",
        "page S1 second page",
        "\\",
        "sect S2 page {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SECTIONPAGES}}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains(SECTION_PAGES_MARKER));
    assert!(!text.contains("fldinst"));
    assert!(!text.contains("SECTIONPAGES"));
    assert!(!text.contains("[Field removed"));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 3);
    let rendered_text = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| {
            let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
            decoded_pdf_text(&content)
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered_text.contains("S1 page 2"),
        "first section page count should resolve to 2 in visible PDF text: {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("S2 page 1"),
        "second section page count should resolve to 1 in visible PDF text: {rendered_text:?}"
    );
    for forbidden in [
        b"fldinst".as_slice(),
        b"SECTIONPAGES",
        SECTION_PAGES_MARKER.as_bytes(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden section-page field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_seq_fields_render_deterministic_passive_counters_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Figure {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SEQ Figure}} and Figure {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SEQ Figure}} Table {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SEQ Table}}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Figure 1 and Figure 2 Table 1"));
    assert!(!text.contains("SEQ"));
    assert!(!text.contains("fldinst"));
    assert!(!text.contains("[Field removed"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field SEQ without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Figure 1 and Figure 2 Table 1"),
        "decoded PDF text did not contain passive SEQ counters: {rendered_text:?}"
    );
    for forbidden in [
        b"SEQ".as_slice(),
        b"fldinst",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden sequence-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_mergefields_render_passive_placeholders_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Dear {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst MERGEFIELD FirstName \\\\* MERGEFORMAT}} {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst MERGEFIELD \"Last Name\" \\\\b \"before\"}}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Dear \u{00ab}FirstName\u{00bb} \u{00ab}Last Name\u{00bb}"));
    assert!(!text.contains("MERGEFIELD"));
    assert!(!text.contains("MERGEFORMAT"));
    assert!(!text.contains("fldinst"));
    assert!(!text.contains("before"));
    assert!(!text.contains("[Field removed"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field MERGEFIELD without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Dear")
            && rendered_text.contains("FirstName")
            && rendered_text.contains("Last Name"),
        "decoded PDF text did not contain passive MERGEFIELD placeholders: {rendered_text:?}"
    );
    for forbidden in [
        b"MERGEFIELD".as_slice(),
        b"MERGEFORMAT",
        b"fldinst",
        b"before",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden merge-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_autonum_fields_render_bounded_passive_numbers_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Clause {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst AUTONUM}}. Legal {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst AUTONUMLGL \\\\s 1}}. Outline {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst AUTONUMOUT}}.",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let visible_text = text.replace(PASSIVE_ADVANCE_MARKER, "");

    assert!(visible_text.contains("Clause 1.. Legal 11. Outline 1.."));
    assert_eq!(text.matches(PASSIVE_ADVANCE_MARKER).count(), 3);
    assert!(!text.contains("AUTONUM"));
    assert!(!text.contains("AUTONUMLGL"));
    assert!(!text.contains("AUTONUMOUT"));
    assert!(!text.contains("fldinst"));
    assert!(!text.contains("\\s"));
    assert!(!text.contains("[Field removed"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field AUTONUM without executing field instruction")
    }));

    let strict = parse_rtf_bytes_with_options(
        &input,
        &RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        },
    )
    .unwrap();
    assert!(collect_text(&strict.document).contains("Clause 1. Legal 2. Outline 3."));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Clause 1.. Legal 11. Outline 1.."),
        "decoded PDF text did not contain passive AUTONUM values: {rendered_text:?}"
    );
    for forbidden in [
        b"AUTONUM".as_slice(),
        b"AUTONUMLGL",
        b"AUTONUMOUT",
        b"fldinst",
        PASSIVE_ADVANCE_MARKER.as_bytes(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden autonum-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn structural_metadata_fields_do_not_emit_visible_placeholders_or_payloads() {
    let input = br#"{\rtf1{\fonttbl{\f0 Arial;}{\field{\*\fldinst HYPERLINK "https://hidden.example/font"}}}{\stylesheet{\s1 Visible Style;}{\field{\*\fldinst HYPERLINK "https://hidden.example/style"}}}{\colortbl;\red0\green0\blue0;{\field{\*\fldinst HYPERLINK "https://hidden.example/color"}}}\f0 Visible body\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains("Visible body"),
        "decoded text did not contain visible body text: {text:?}"
    );
    for forbidden in [
        "[Field removed",
        "HYPERLINK",
        "hidden.example",
        "fldinst",
        "fonttbl",
        "stylesheet",
        "colortbl",
    ] {
        assert!(
            !text.contains(forbidden),
            "structural metadata field leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let rendered_text = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| {
            let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
            decoded_pdf_text(&content)
        })
        .collect::<String>();

    assert!(
        rendered_text.contains("Visible body"),
        "decoded PDF text did not contain visible body text: {rendered_text:?}"
    );
    for forbidden in [
        b"[Field removed".as_slice(),
        b"HYPERLINK",
        b"hidden.example",
        b"fldinst",
        b"fonttbl",
        b"stylesheet",
        b"colortbl",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/AcroForm",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "structural metadata field leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn metadata_resultless_fields_do_not_emit_visible_placeholders_or_payloads() {
    let input = br#"{\rtf1{\info{\title Hidden title {\field{\*\fldinst INCLUDEPICTURE "https://hidden.example/payload.png"}}}}Visible body\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert_eq!(text, "Visible body");
    for forbidden in [
        "[Field removed",
        "INCLUDEPICTURE",
        "hidden.example",
        "payload.png",
        "Hidden title",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "metadata resultless field leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible body"));
    for forbidden in [
        b"[Field removed".as_slice(),
        b"INCLUDEPICTURE",
        b"hidden.example",
        b"payload.png",
        b"Hidden title",
        b"fldinst",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "metadata resultless field leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn structural_metadata_bookmarks_do_not_feed_fields_or_pdf_anchors() {
    let input = br#"{\rtf1{\fonttbl{\f0 Arial;}{\*\bkmkstart HiddenFontMark}Hidden font payload{\*\bkmkend HiddenFontMark}}{\stylesheet{\s1 Visible Style;}{\*\bkmkstart HiddenStyleMark}Hidden style payload{\*\bkmkend HiddenStyleMark}}\f0 Body {\*\bkmkstart VisibleMark}Marked text{\*\bkmkend VisibleMark} visible {\field{\*\fldinst REF VisibleMark}} hidden {\field{\*\fldinst REF HiddenFontMark}} style {\field{\*\fldinst REF HiddenStyleMark}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = strip_bookmark_page_markers(&collect_text(&parsed.document));

    assert!(
        text.contains("Body Marked text visible Marked text hidden [Field removed: no passive result] style [Field removed: no passive result]"),
        "decoded text did not preserve visible bookmark or placeholder hidden metadata refs: {text:?}"
    );
    for forbidden in [
        "Hidden font payload",
        "Hidden style payload",
        "HiddenFontMark",
        "HiddenStyleMark",
        "bkmkstart",
        "bkmkend",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "structural metadata bookmark leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let rendered_text = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| {
            let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
            decoded_pdf_text(&content)
        })
        .collect::<String>();

    assert!(
        rendered_text.contains("Body Marked text visible Marked text hidden [Field removed: no passive result] style [Field removed: no passive result]"),
        "decoded PDF text did not preserve visible bookmark or placeholder hidden metadata refs: {rendered_text:?}"
    );
    for forbidden in [
        b"Hidden font payload".as_slice(),
        b"Hidden style payload",
        b"HiddenFontMark",
        b"HiddenStyleMark",
        b"bkmkstart",
        b"bkmkend",
        b"fldinst",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/AcroForm",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "structural metadata bookmark leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_ref_fields_render_closed_bookmark_text_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "*",
        "\\",
        "bkmkstart SafeBookmark}Visible referenced text{",
        "\\",
        "*",
        "\\",
        "bkmkend SafeBookmark} copy {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst REF SafeBookmark \\\\h}}.",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let visible_text = strip_bookmark_page_markers(&text);

    assert!(visible_text.contains("Before Visible referenced text copy Visible referenced text."));
    for forbidden in [
        "REF",
        "SafeBookmark",
        "bkmkstart",
        "bkmkend",
        "fldinst",
        "\\h",
        "[Field removed",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden REF/bookmark content leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field REF without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Before Visible referenced text copy Visible referenced text."),
        "decoded PDF text did not contain passive REF value: {rendered_text:?}"
    );
    for forbidden in [
        b"REF".as_slice(),
        b"SafeBookmark",
        b"bkmkstart",
        b"bkmkend",
        b"fldinst",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden REF/bookmark content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_ref_fields_resolve_unicode_bookmark_names_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "*",
        "\\",
        "bkmkstart Code",
        "\\",
        "u65?Bookmark}Visible referenced text{",
        "\\",
        "*",
        "\\",
        "bkmkend Code",
        "\\",
        "u65?Bookmark} copy {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst REF \"Code",
        "\\",
        "u65?Bookmark\"}}.",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let visible_text = strip_bookmark_page_markers(&text);

    assert!(visible_text.contains("Before Visible referenced text copy Visible referenced text."));
    for forbidden in [
        "REF",
        "CodeABookmark",
        "u65",
        "?",
        "bkmkstart",
        "bkmkend",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden Unicode REF/bookmark content leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.contains("Before Visible referenced text copy Visible referenced text."),
        "decoded PDF text did not contain Unicode passive REF value: {rendered_text:?}"
    );
    for forbidden in [
        b"REF".as_slice(),
        b"CodeABookmark",
        b"u65",
        b"bkmkstart",
        b"bkmkend",
        b"fldinst",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden Unicode REF/bookmark content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_set_fields_feed_refs_without_instruction_or_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Output {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SET HiddenKey \"Contoso\"}}{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst REF HiddenKey}} reject {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SET BadKey \"Hidden value\" HYPERLINK \"https://example.com/set\"}}{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst REF BadKey}}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Output Contoso reject [Field removed: no passive result]"));
    for forbidden in [
        "SET",
        "HiddenKey",
        "BadKey",
        "Hidden value",
        "HYPERLINK",
        "https://example.com/set",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "SET field leaked unsafe text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Output Contoso reject [Field removed: no passive result]"),
        "decoded PDF text did not contain passive SET/REF value: {rendered_text:?}"
    );
    for forbidden in [
        b"SET".as_slice(),
        b"REF",
        b"HiddenKey",
        b"BadKey",
        b"Hidden value",
        b"HYPERLINK",
        b"https://example.com/set",
        b"fldinst",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden SET-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn set_field_values_reject_active_markers_before_ref_pdf_output() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Output {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SET Safe \"Visible value\"}} safe {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst REF Safe}} unsafe {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SET Unsafe \"PAYLOAD /JavaScript\"}} ref {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst REF Unsafe}}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains("Output  safe Visible value unsafe  ref [Field removed: no passive result]"),
        "unsafe SET value should not seed REF output: {text:?}"
    );
    for forbidden in [
        "PAYLOAD",
        "/JavaScript",
        "SET Unsafe",
        "REF Unsafe",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "unsafe SET/REF content leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("safe Visible value unsafe  ref"));
    assert!(rendered_text.contains("[Field removed: no passive result]"));
    for forbidden in [
        b"PAYLOAD".as_slice(),
        b"SET Unsafe",
        b"REF Unsafe",
        b"fldinst",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "unsafe SET/REF content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_styleref_fields_render_safe_prior_style_text_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "stylesheet{",
        "\\",
        "s1 TitleStyle;}}{",
        "\\",
        "s1 Visible title {",
        "\\",
        "*",
        "\\",
        "unknown{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst HYPERLINK \"https://example.com/styleref\"}{",
        "\\",
        "fldrslt Hidden link}}}",
        "\\",
        "par}{",
        "\\",
        "pard Ref {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst STYLEREF TitleStyle \\\\* Upper}}",
        "\\",
        "par}}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible title"));
    assert!(text.contains("Ref VISIBLE TITLE"));
    for forbidden in [
        "STYLEREF",
        "TitleStyle",
        "HYPERLINK",
        "https://example.com",
        "Hidden link",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "STYLEREF leaked unsafe text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Visible title"));
    assert!(
        rendered_text.contains("Ref VISIBLE TITLE"),
        "decoded PDF text did not contain passive STYLEREF value: {rendered_text:?}"
    );
    for forbidden in [
        b"STYLEREF".as_slice(),
        b"TitleStyle",
        b"HYPERLINK",
        b"https://example.com",
        b"Hidden link",
        b"fldinst",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden STYLEREF content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_styleref_fields_resolve_unicode_style_names_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "stylesheet{",
        "\\",
        "s1 Code ",
        "\\",
        "u65? Style;}}{",
        "\\",
        "s1 Visible title",
        "\\",
        "par}{",
        "\\",
        "pard Ref {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst STYLEREF \"Code ",
        "\\",
        "u65? Style\" \\\\* Upper}}",
        "\\",
        "par}}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible title"));
    assert!(text.contains("Ref VISIBLE TITLE"));
    for forbidden in [
        "STYLEREF",
        "Code A Style",
        "u65",
        "?",
        "fldinst",
        "stylesheet",
    ] {
        assert!(
            !text.contains(forbidden),
            "Unicode STYLEREF leaked unsafe text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.contains("Ref VISIBLE TITLE"),
        "decoded PDF text did not contain Unicode passive STYLEREF value: {rendered_text:?}"
    );
    for forbidden in [
        b"STYLEREF".as_slice(),
        b"Code A Style",
        b"u65",
        b"fldinst",
        b"stylesheet",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden Unicode STYLEREF content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_styleref_fields_resolve_hex_style_names_without_control_leakage() {
    let input = br#"{\rtf1{\stylesheet{\s1 Cod\'65 Style;}}\s1 Visible title\par\pard Ref {\field{\*\fldinst STYLEREF "Cod\'65 Style" \\* Upper}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible title"));
    assert!(text.contains("Ref VISIBLE TITLE"));
    for forbidden in ["STYLEREF", "Code Style", "\\'65", "fldinst", "stylesheet"] {
        assert!(
            !text.contains(forbidden),
            "hex STYLEREF leaked unsafe text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.contains("Ref VISIBLE TITLE"),
        "decoded PDF text did not contain hex-style-name passive STYLEREF value: {rendered_text:?}"
    );
    for forbidden in [
        b"STYLEREF".as_slice(),
        b"Code Style",
        b"Cod\\'65",
        b"fldinst",
        b"stylesheet",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden hex STYLEREF content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_noteref_fields_render_bookmarked_note_reference_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Note {",
        "\\",
        "*",
        "\\",
        "bkmkstart NoteRef}",
        "\\",
        "chftn{",
        "\\",
        "*",
        "\\",
        "bkmkend NoteRef}{",
        "\\",
        "footnote Footnote text {",
        "\\",
        "*",
        "\\",
        "unknown{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst HYPERLINK \"https://example.com/hidden\"}}",
        "\\",
        "par} again {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst NOTEREF NoteRef \\\\* ROMAN}}.",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst NOTEREF Missing}}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let visible_text = strip_bookmark_page_markers(&text);

    assert!(
        visible_text.contains(&format!("Note {}", footnote_reference_marker(0))),
        "text was {text:?}"
    );
    assert!(visible_text.contains("again I."), "text was {text:?}");
    assert_eq!(
        text.matches("[Field removed: no passive result]").count(),
        1
    );
    for forbidden in [
        "NOTEREF",
        "NoteRef",
        "bkmkstart",
        "bkmkend",
        "fldinst",
        "HYPERLINK",
        "https://example.com",
        "unknown",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden NOTEREF/bookmark content leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field NOTEREF without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let rendered_text = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| {
            let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
            decoded_pdf_text(&content)
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered_text.contains("Note 1"),
        "decoded PDF text did not contain bookmarked note reference: {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("again I."),
        "decoded PDF text did not contain passive NOTEREF value: {rendered_text:?}"
    );
    for forbidden in [
        b"NOTEREF".as_slice(),
        b"NoteRef",
        b"bkmkstart",
        b"bkmkend",
        b"fldinst",
        b"HYPERLINK",
        b"https://example.com",
        b"Hidden link",
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden NOTEREF/bookmark content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_pageref_fields_resolve_bookmark_page_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 See target on page {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst PAGEREF TargetBookmark \\\\h}}.",
        "\\",
        "page {",
        "\\",
        "*",
        "\\",
        "bkmkstart TargetBookmark}Target text{",
        "\\",
        "*",
        "\\",
        "bkmkend TargetBookmark}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains(BOOKMARK_PAGE_REF_MARKER));
    assert!(text.contains(BOOKMARK_PAGE_ANCHOR_MARKER));
    for forbidden in [
        "PAGEREF",
        "TargetBookmark",
        "bkmkstart",
        "bkmkend",
        "fldinst",
        "\\h",
        "[Field removed",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden PAGEREF/bookmark content leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field PAGEREF without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 2);
    let rendered_text = parsed_pdf
        .get_pages()
        .values()
        .map(|page_id| {
            let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
            decoded_pdf_text(&content)
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered_text.contains("See target on page 2."),
        "decoded PDF text did not contain passive PAGEREF value: {rendered_text:?}"
    );
    assert!(rendered_text.contains("Target text"));
    for forbidden in [
        b"PAGEREF".as_slice(),
        b"TargetBookmark",
        b"bkmkstart",
        b"bkmkend",
        b"fldinst",
        BOOKMARK_PAGE_REF_MARKER.as_bytes(),
        BOOKMARK_PAGE_ANCHOR_MARKER.as_bytes(),
        BOOKMARK_PAGE_MARKER_END.as_bytes(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden PAGEREF/bookmark content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_formula_fields_render_bounded_passive_arithmetic_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Total {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = (6 + 4) * 3 \\\\# \"0\"}} and delta {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = -8 + 3 * 2}} plus {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = +5}} compare {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = 2 + 2 = 4}} neq {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = 3 <> 3}} le {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = 5 <= 5 \\\\* ROMAN}} badcmp {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = 1 < 2 < 3}} true {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = TRUE() + 4}} false {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = NOT(FALSE())}} badboolfn {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = TRUE(1)}} nested {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = AND(1 < 2, TRUE)}} badnested {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = AND(1 < 2 < 3, TRUE)}} iffn {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = IF(2 > 1; 7 + 3; 4)}} badif {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = IF(1 < 2 < 3; 7; 0)}} badconst {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = MAYBE}} sum {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = SUM(1, 2 + 3, (4 * 5)) \\\\# \"000\"}} semisum {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = SUM(1; 2 + 3; (4 * 5)) \\\\# \"000\"}} badsemi {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = SUM(1;)}} min {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = MIN(9, 2 + 3, -4)}} max {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = MAX(9, 2 + 3, -4) \\\\* ROMAN}} abs {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = ABS(-8 + 3) \\\\# \"00\"}} malformed {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = ABS(1,2)}} product {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = PRODUCT(2, 3 + 1, -5)}} power {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = POWER(2 + 1, 4)}} exp {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = 2 * 3 ^ 2}} badexp {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = 2 ^ -1}} mod {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = MOD(17 + 3, 6)}} modzero {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = MOD(5,0)}} quotient {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = QUOTIENT(17 + 3, 6)}} badquotient {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = QUOTIENT(5,0)}} round {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = ROUND(2 + 3,0)}} count {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = COUNT(2, 3 + 1, -5)}} n {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = N(7 + 3) \\\\# \"000\"}} badn {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = N(1,2)}} even {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = EVEN(5)}} odd {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = ODD(-4) \\\\# \"00\"}} badeven {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = EVEN(9223372036854775807)}} sign {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = SIGN(-8 + 3)}} int {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = INT(17 + 3)}} sqrt {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = SQRT(9 + 7)}} and {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = AND(1, 2 + 3, -5)}} or {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = OR(0, 4 - 4, 7)}} not {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = NOT(4 - 4)}} xor {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = XOR(1, 0, 4 - 4)}} average {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = AVERAGE(2, 3 + 1, 6)}} unsupported {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = MEDIAN(2,0)}}.",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains(
        "Total 30 and delta -2 plus 5 compare 1 neq 0 le I badcmp [Field removed: no passive result] true 5 false 1 badboolfn [Field removed: no passive result] nested 1 badnested [Field removed: no passive result] iffn 10 badif [Field removed: no passive result] badconst [Field removed: no passive result] sum 026 semisum 026 badsemi [Field removed: no passive result] min -4 max IX abs 05 malformed [Field removed: no passive result] product -40 power 81 exp 18 badexp [Field removed: no passive result] mod 2 modzero [Field removed: no passive result] quotient 3 badquotient [Field removed: no passive result] round 5 count 3 n 010 badn [Field removed: no passive result] even 6 odd -05 badeven [Field removed: no passive result] sign -1 int 20 sqrt 4 and 1 or 1 not 1 xor 1 average 4 unsupported [Field removed: no passive result]."
    ));
    for forbidden in [
        "fldinst",
        "\\#",
        "\"0\"",
        "\"000\"",
        "\"00\"",
        "(6 + 4)",
        "3 * 2",
        "SUM",
        "1;",
        "MIN",
        "MAX",
        "ABS",
        "PRODUCT",
        "POWER",
        "2 * 3 ^ 2",
        "2 ^ -1",
        "MOD",
        "QUOTIENT",
        "ROUND",
        "COUNT",
        "N(",
        "EVEN",
        "ODD",
        "9223372036854775807",
        "SIGN",
        "INT",
        "SQRT",
        "AND",
        "OR",
        "NOT",
        "IF",
        "XOR",
        "AVERAGE",
        "MEDIAN",
        "ROMAN",
        "2 + 3",
        "4 * 5",
        "-8 + 3",
        "3 + 1",
        "2 + 1",
        "+5",
        "2 + 2",
        "3 <> 3",
        "5 <= 5",
        "1 < 2 < 3",
        "AND(1 < 2",
        "AND(1 < 2 < 3",
        "IF(2 > 1",
        "IF(1 < 2 < 3",
        "TRUE",
        "FALSE",
        "TRUE()",
        "FALSE()",
        "TRUE(1)",
        "MAYBE",
        "17 + 3",
        "9 + 7",
        "4 - 4",
        "5,0",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden formula-field content leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field FORMULA without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains(
            "Total 30 and delta -2 plus 5 compare 1 neq 0 le I badcmp [Field removed: no passive result] true 5 false 1 badboolfn [Field removed: no passive result] nested 1 badnested [Field removed: no passive result] iffn 10 badif [Field removed: no passive result] badconst [Field removed: no passive result] sum 026 semisum 026 badsemi [Field removed: no passive result] min -4 max IX abs 05 malformed [Field removed: no passive result] product -40 power 81 exp 18 badexp [Field removed: no passive result] mod 2 modzero [Field removed: no passive result] quotient 3 badquotient [Field removed: no passive result] round 5 count 3 n 010 badn [Field removed: no passive result] even 6 odd -05 badeven [Field removed: no passive result] sign -1 int 20 sqrt 4 and 1 or 1 not 1 xor 1 average 4 unsupported [Field removed: no passive result]."
        ),
        "decoded PDF text did not contain passive formula values: {rendered_text:?}"
    );
    for forbidden in [
        b"fldinst".as_slice(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden formula-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_formula_fields_resolve_passive_references_without_instruction_leakage() {
    let input = br#"{\rtf1{\field{\*\fldinst SET Units "7"}}Units {\field{\*\fldinst = Units * 3 \\# "00"}} {\*\bkmkstart Rate}4{\*\bkmkend Rate} rate {\field{\*\fldinst = Rate + 2}} bad {\field{\*\fldinst = Missing + 1}}{\field{\*\fldinst SET Bad "PAYLOAD /JavaScript"}} unsafe {\field{\*\fldinst = Bad + 1}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = strip_bookmark_page_markers(&collect_text(&parsed.document));

    assert!(
        text.contains(
            "Units 21 4 rate 6 bad [Field removed: no passive result] unsafe [Field removed: no passive result]"
        ),
        "normalized text did not contain passive formula reference values: {text:?}"
    );
    for forbidden in [
        "SET",
        "Units * 3",
        "Rate + 2",
        "Missing + 1",
        "Bad + 1",
        "PAYLOAD",
        "/JavaScript",
        "\\#",
        "\"00\"",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "formula reference field leaked unsafe text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let searchable_pdf = pdf_bytes_excluding_embedded_font_programs(&output.pdf);
    assert!(
        rendered_text.contains(
            "Units 21 4 rate 6 bad [Field removed: no passive result] unsafe [Field removed: no passive result]"
        ),
        "decoded PDF text did not contain passive formula reference values: {rendered_text:?}"
    );
    for forbidden in [
        b"SET".as_slice(),
        b"Units * 3",
        b"Rate + 2",
        b"Missing + 1",
        b"Bad + 1",
        b"PAYLOAD",
        b"/JavaScript",
        b"\\#",
        b"\"00\"",
        b"fldinst",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
        b"/AA",
        b"/AcroForm",
        b"/Widget",
        b"/URI",
    ] {
        assert!(
            !searchable_pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "formula reference field leaked unsafe PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_eq_fraction_fields_render_passively_without_instruction_leakage() {
    let input = br"{\rtf1 Equation {\field{\*\fldinst EQ \f(1,2)}} and escaped {\field{\*\fldinst EQ \\f(alpha,beta)}}\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains("Equation (1,2) and escaped alpha\u{2044}beta"),
        "normalized EQ text was {text:?}"
    );
    for forbidden in ["EQ", "fldinst", "\\f", "(alpha,beta)", "[Field removed"] {
        assert!(
            !text.contains(forbidden),
            "forbidden EQ field content leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field EQ without executing field instruction")
    }));

    let strict = parse_rtf_bytes_with_options(
        &input,
        &RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        },
    )
    .unwrap();
    let strict_text = collect_text(&strict.document);
    assert!(
        strict_text.contains("Equation 1\u{2044}2 and escaped alpha\u{2044}beta"),
        "strict EQ text was {strict_text:?}"
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let searchable_pdf = pdf_bytes_excluding_embedded_font_programs(&output.pdf);
    assert!(rendered_text.contains("Equation (1,2) and escaped alpha"));
    assert!(rendered_text.contains("beta"));
    let alpha_position =
        pdf_first_text_position_for_text(&content, "alpha").expect("escaped EQ numerator position");
    let beta_position = pdf_first_text_position_for_text(&content, "beta")
        .expect("escaped EQ denominator position");
    assert!(
        alpha_position.1 > beta_position.1,
        "escaped EQ fraction numerator should render above denominator: numerator={alpha_position:?}, denominator={beta_position:?}"
    );
    assert!(
        pdf_text_font_names(&content)
            .iter()
            .any(|name| name.starts_with(b"TF")),
        "escaped EQ fraction should use a passive supplied font"
    );
    for forbidden in [
        b"fldinst".as_slice(),
        b"\\f",
        b"(alpha,beta)",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !searchable_pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden EQ field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_eq_root_fields_render_passively_without_instruction_leakage() {
    let input =
        br"{\rtf1 Roots {\field{\*\fldinst EQ \\r(x+1)}} cube {\field{\*\fldinst EQ \\r(3,y)}}\par}"
            .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains("Roots \u{221a}x+1 cube 3\u{221a}y"),
        "normalized EQ root text was {text:?}"
    );
    for forbidden in ["EQ", "fldinst", "\\r", "(x+1)", "(3,y)", "[Field removed"] {
        assert!(
            !text.contains(forbidden),
            "forbidden EQ root field content leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field EQ without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(rendered_text.contains("Roots "));
    assert!(rendered_text.contains("x+1 cube 3"));
    assert!(rendered_text.contains("y"));
    let degree_position =
        pdf_first_text_position_for_text(&content, "3").expect("EQ root degree position");
    let radicand_position =
        pdf_first_text_position_for_text(&content, "y").expect("EQ root radicand position");
    assert!(
        degree_position.1 > radicand_position.1,
        "EQ indexed root degree should render above radicand: degree={degree_position:?}, radicand={radicand_position:?}"
    );
    assert!(
        pdf_text_font_names(&content)
            .iter()
            .any(|name| name.starts_with(b"TF")),
        "EQ roots should use a passive supplied font"
    );
    assert!(
        content.operations.windows(3).any(|operations| {
            operations[0].operator == "m"
                && operations[1].operator == "l"
                && operations[2].operator == "S"
        }),
        "EQ root radicands should render with passive overbar strokes"
    );
    for forbidden in [
        b"fldinst".as_slice(),
        b"(3,y)",
        b"\\r(x+1)",
        b"\\r(3,y)",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden EQ root field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_eq_fields_reject_active_pdf_marker_components_before_pdf() {
    let input = br#"{\rtf1 frac {\field{\*\fldinst EQ \f(/JavaScript,2)}} root {\field{\*\fldinst EQ \r(3,/EmbeddedFile)}} ok {\field{\*\fldinst EQ \f(1,2)}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains(
            "frac [Field removed: no passive result] root [Field removed: no passive result] ok (1,2)"
        ),
        "normalized text did not contain passive EQ placeholders: {text:?}"
    );
    for forbidden in [
        "EQ",
        "\\f",
        "\\r",
        "/JavaScript",
        "/EmbeddedFile",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "active marker EQ component leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let searchable_pdf = pdf_bytes_excluding_embedded_font_programs(&output.pdf);
    assert!(
        rendered_text.contains(
            "frac [Field removed: no passive result] root [Field removed: no passive result] ok (1,2)"
        ),
        "decoded PDF text did not contain passive EQ placeholders: {rendered_text:?}"
    );
    for forbidden in [
        b"fldinst".as_slice(),
        b"\\f",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/URI",
    ] {
        assert!(
            !searchable_pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "active marker EQ component leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_if_fields_render_passive_branches_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Status {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst IF 5 > 3 \"Greater\" \"Lower\"}} and {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst IF \"Alpha\" = \"Beta\" \"Match\" \"Different\"}}.",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let visible_text = text.replace(PASSIVE_ADVANCE_MARKER, "");

    assert!(visible_text.contains("Status Greater and Different."));
    for forbidden in [
        "IF",
        "Alpha",
        "Beta",
        "fldinst",
        "Greater\"",
        "Lower\"",
        "[Field removed",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden IF field content leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field IF without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Status Greater and Different."),
        "decoded PDF text did not contain passive IF values: {rendered_text:?}"
    );
    for forbidden in [
        b"Alpha".as_slice(),
        b"Beta",
        b"fldinst",
        b"Greater\"",
        b"Lower\"",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden IF field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_if_fields_resolve_passive_references_without_instruction_leakage() {
    let input = br#"{\rtf1{\field{\*\fldinst SET Units "7"}}If {\field{\*\fldinst IF Units > 5 "High" "Low"}} {\*\bkmkstart Rate}4{\*\bkmkend Rate} rate {\field{\*\fldinst IF Rate = 4 "Match" "Miss" \\* Upper}} missing {\field{\*\fldinst IF Missing = 1 "Yes" "No"}} unsafe {\field{\*\fldinst SET Bad "PAYLOAD /JavaScript"}}{\field{\*\fldinst IF Bad = 1 "Yes" "No"}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = strip_bookmark_page_markers(&collect_text(&parsed.document));
    let visible_text = text.replace(PASSIVE_ADVANCE_MARKER, "");

    assert!(
        visible_text.contains("If High 4 rate MATCH missing No unsafe No"),
        "text did not contain passive IF reference values: {text:?}"
    );
    for forbidden in [
        "SET",
        "IF",
        "Units > 5",
        "Rate = 4",
        "Missing = 1",
        "Bad = 1",
        "PAYLOAD",
        "/JavaScript",
        "fldinst",
        "[Field removed",
    ] {
        assert!(
            !text.contains(forbidden),
            "referenced IF field leaked unsafe text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("If High 4 rate MATCH missing No unsafe No"),
        "decoded PDF text did not contain passive IF reference values: {rendered_text:?}"
    );
    for forbidden in [
        b"SET".as_slice(),
        b"Units > 5",
        b"Rate = 4",
        b"Missing = 1",
        b"Bad = 1",
        b"PAYLOAD",
        b"/JavaScript",
        b"fldinst",
        b"[Field removed",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
        b"/AA",
        b"/AcroForm",
        b"/Widget",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "referenced IF field leaked unsafe PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_compare_fields_render_bounded_passive_values_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Compare {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst COMPARE 5 > 3}} and {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst COMPARE \"Alpha\" = \"Beta\"}} and {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst COMPARE 3 <= 3 \\\\* ROMAN}} and malformed {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst COMPARE 1 = 1 HIDDEN-TRAIL}}.",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let visible_text = text.replace(PASSIVE_ADVANCE_MARKER, "");

    assert!(
        visible_text
            .contains("Compare 1 and 0 and I and malformed [Field removed: no passive result]."),
        "text was {text:?}"
    );
    for forbidden in [
        "COMPARE",
        "Alpha",
        "Beta",
        "HIDDEN-TRAIL",
        "fldinst",
        "ROMAN",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden COMPARE field content leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field COMPARE without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text
            .contains("Compare 1 and 0 and I and malformed [Field removed: no passive result]."),
        "decoded PDF text did not contain passive COMPARE values: {rendered_text:?}"
    );
    for forbidden in [
        b"COMPARE".as_slice(),
        b"Alpha",
        b"Beta",
        b"HIDDEN-TRAIL",
        b"fldinst",
        b"ROMAN",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden COMPARE field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_compare_fields_resolve_passive_references_without_instruction_leakage() {
    let input = br#"{\rtf1{\field{\*\fldinst SET Units "7"}}Compare {\field{\*\fldinst COMPARE Units > 5}} {\*\bkmkstart Rate}4{\*\bkmkend Rate} rate {\field{\*\fldinst COMPARE Rate = 4 \\* ROMAN}} missing {\field{\*\fldinst COMPARE Missing = 1}} unsafe {\field{\*\fldinst SET Bad "PAYLOAD /JavaScript"}}{\field{\*\fldinst COMPARE Bad = 1}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = strip_bookmark_page_markers(&collect_text(&parsed.document));
    let visible_text = text.replace(PASSIVE_ADVANCE_MARKER, "");

    assert!(
        visible_text.contains("Compare 1 4 rate I missing 0 unsafe 0"),
        "text did not contain passive COMPARE reference values: {text:?}"
    );
    for forbidden in [
        "SET",
        "COMPARE",
        "Units > 5",
        "Rate = 4",
        "Missing = 1",
        "Bad = 1",
        "PAYLOAD",
        "/JavaScript",
        "ROMAN",
        "fldinst",
        "[Field removed",
    ] {
        assert!(
            !text.contains(forbidden),
            "referenced COMPARE field leaked unsafe text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Compare 1 4 rate I missing 0 unsafe 0"),
        "decoded PDF text did not contain passive COMPARE reference values: {rendered_text:?}"
    );
    for forbidden in [
        b"SET".as_slice(),
        b"COMPARE",
        b"Units > 5",
        b"Rate = 4",
        b"Missing = 1",
        b"Bad = 1",
        b"PAYLOAD",
        b"/JavaScript",
        b"ROMAN",
        b"fldinst",
        b"[Field removed",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
        b"/AA",
        b"/AcroForm",
        b"/Widget",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "referenced COMPARE field leaked unsafe PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_field_case_switches_render_passively_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Values {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst QUOTE \"mixed Case\" \\\\* Upper}} and {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst QUOTE \"MIXED Case\" \\\\* Lower \\\\* FirstCap}} and {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst IF 1 = 1 \"checked status\" \"other\" \\\\* Caps}}.",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let visible_text = text.replace(PASSIVE_ADVANCE_MARKER, "");

    assert!(
        visible_text.contains("Values MIXED CASE and Mixed case and Checked Status."),
        "normalized field switch text was {text:?}"
    );
    for forbidden in [
        "QUOTE",
        "IF",
        "fldinst",
        "Upper",
        "Lower",
        "FirstCap",
        "Caps",
        "checked status\"",
        "[Field removed",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden field case-switch content leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field QUOTE without executing field instruction")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field IF without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Values MIXED CASE and Mixed case and Checked Status."),
        "decoded PDF text did not contain passive field case-switch values: {rendered_text:?}"
    );
    for forbidden in [
        b"QUOTE".as_slice(),
        b"fldinst",
        b"Upper",
        b"Lower",
        b"FirstCap",
        b"checked status\"",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden field case-switch content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_field_number_switches_render_passively_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Values {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SEQ Figure \\\\r 4 \\\\* ROMAN}} {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SEQ Figure \\\\* roman}} {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = 27 \\\\* alphabetic}} {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = 27 \\\\* ALPHABETIC}} {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = 255 \\\\* Hex}} {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst IF 1 = 1 \"7\" \"0\" \\\\* Ordinal}}.",
        " {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = 42 \\\\* CardText \\\\* MERGEFORMAT}} {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = 513 \\\\* OrdText \\\\* CHARFORMAT}}.",
        " {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = 42 \\\\* DollarText}}.",
        " {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst = 9 \\\\* ArabicDash}}.",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let visible_text = text.replace(PASSIVE_ADVANCE_MARKER, "");

    assert!(
        visible_text.contains(
            "Values IV v aa AA FF 7th. forty-two five hundred thirteenth. forty-two and 00/100. - 9 -."
        ),
        "normalized field number-switch text was {text:?}"
    );
    for forbidden in [
        "SEQ",
        "IF",
        "fldinst",
        "\\r",
        "Figure",
        "ROMAN",
        "roman",
        "alphabetic",
        "ALPHABETIC",
        "Ordinal",
        "Hex",
        "CardText",
        "OrdText",
        "DollarText",
        "ArabicDash",
        "MERGEFORMAT",
        "CHARFORMAT",
        "[Field removed",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden field number-switch content leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field SEQ without executing field instruction")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field FORMULA without executing field instruction")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field IF without executing field instruction")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains(
            "Values IV v aa AA FF 7th. forty-two five hundred thirteenth. forty-two and 00/100. - 9 -."
        ),
        "decoded PDF text did not contain passive field number-switch values: {rendered_text:?}"
    );
    for forbidden in [
        b"SEQ".as_slice(),
        b"fldinst",
        b"Figure",
        b"ROMAN",
        b"roman",
        b"alphabetic",
        b"ALPHABETIC",
        b"Ordinal",
        b"Hex",
        b"CardText",
        b"OrdText",
        b"DollarText",
        b"ArabicDash",
        b"MERGEFORMAT",
        b"CHARFORMAT",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden field number-switch content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_quote_fields_render_without_executing_field_instruction() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst QUOTE \"Visible quoted text\"}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let visible_text = text.replace(PASSIVE_ADVANCE_MARKER, "");

    assert!(visible_text.contains("Before Visible quoted text After"));
    assert!(!text.contains("QUOTE"));
    assert!(!text.contains("fldinst"));
    assert!(!text.contains("[Field removed"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field QUOTE without executing field instruction")
    }));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("quote-field.rtf");
    let output_path = dir.path().join("quote-field.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(
        decoded_pdf_text(&content).contains("Before Visible quoted text After"),
        "decoded PDF text did not contain passive QUOTE result"
    );
    for forbidden in [
        b"QUOTE".as_slice(),
        b"fldinst",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        PASSIVE_ADVANCE_MARKER.as_bytes(),
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden quote-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_quote_field_instruction_unicode_renders_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst QUOTE \"Code ",
        "\\",
        "u65? value\"}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let visible_text = text.replace(PASSIVE_ADVANCE_MARKER, "");

    assert!(visible_text.contains("Before Code A value After"));
    for forbidden in ["QUOTE", "fldinst", "u65", "? value"] {
        assert!(
            !text.contains(forbidden),
            "forbidden Unicode quote-field content leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(
        rendered_text.contains("Before Code A value After"),
        "decoded PDF text did not contain Unicode passive QUOTE result: {rendered_text:?}"
    );
    for forbidden in [
        b"QUOTE".as_slice(),
        b"fldinst",
        b"u65",
        b"? value",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden Unicode quote-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_instruction_literal_fields_reject_active_pdf_markers_before_pdf() {
    let input = br#"{\rtf1 quote {\field{\*\fldinst QUOTE "/JavaScript"}} if {\field{\*\fldinst IF 1 = 1 "/EmbeddedFile" "Safe"}} macro {\field{\*\fldinst MACROBUTTON LaunchPayload "/Launch"}} go {\field{\*\fldinst GOTOBUTTON Target "/OpenAction"}} merge {\field{\*\fldinst MERGEFIELD "/URI"}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let visible_text = text.replace(PASSIVE_ADVANCE_MARKER, "");

    assert!(
        visible_text.contains(
            "quote [Field removed: no passive result] if [Field removed: no passive result] macro [Field removed: no passive result] go [Field removed: no passive result] merge [Field removed: no passive result]"
        ),
        "normalized text did not contain passive placeholders: {text:?}"
    );
    for forbidden in [
        "QUOTE",
        "IF",
        "MACROBUTTON",
        "GOTOBUTTON",
        "MERGEFIELD",
        "/JavaScript",
        "/EmbeddedFile",
        "/Launch",
        "/OpenAction",
        "/URI",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "active marker field literal leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains(
            "quote [Field removed: no passive result] if [Field removed: no passive result] macro [Field removed: no passive result] go [Field removed: no passive result] merge [Field removed: no passive result]"
        ),
        "decoded PDF text did not contain passive placeholders: {rendered_text:?}"
    );
    for forbidden in [
        b"QUOTE".as_slice(),
        b"MACROBUTTON",
        b"GOTOBUTTON",
        b"MERGEFIELD",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/URI",
        b"fldinst",
        PASSIVE_ADVANCE_MARKER.as_bytes(),
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "active marker field literal leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_macrobutton_fields_render_without_executing_macro() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst MACROBUTTON LaunchPayload Visible button text}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let visible_text = text.replace(PASSIVE_ADVANCE_MARKER, "");

    assert!(visible_text.contains("Before Visible button text After"));
    assert!(!text.contains("MACROBUTTON"));
    assert!(!text.contains("LaunchPayload"));
    assert!(!text.contains("fldinst"));
    assert!(!text.contains("[Field removed"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive field MACROBUTTON without executing field instruction")
    }));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("macrobutton-field.rtf");
    let output_path = dir.path().join("macrobutton-field.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(decoded_pdf_text(&content).contains("Before Visible button text After"));
    for forbidden in [
        b"MACROBUTTON".as_slice(),
        b"LaunchPayload",
        b"fldinst",
        b"/AA",
        b"/AcroForm",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        PASSIVE_ADVANCE_MARKER.as_bytes(),
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden macrobutton-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_symbol_fields_render_without_executing_field_instruction() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SYMBOL 65}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before A After"));
    assert!(!text.contains("SYMBOL"));
    assert!(!text.contains("fldinst"));
    assert!(!text.contains("[Field removed"));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("rendering passive field SYMBOL without executing field instruction")
    }));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("symbol-field.rtf");
    let output_path = dir.path().join("symbol-field.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(decoded_pdf_text(&content).contains("Before A After"));
    for forbidden in [
        b"SYMBOL".as_slice(),
        b"fldinst",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden symbol-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_symbol_field_cannot_inject_reserved_private_use_semantics() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SYMBOL 983040}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before \u{fffd} After"));
    assert!(!text.contains(PAGE_NUMBER_MARKER));
    assert!(!text.contains("SYMBOL"));
    assert!(!text.contains("983040"));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(!rendered_text.contains("Before 1 After"));
    for forbidden in [
        b"SYMBOL".as_slice(),
        b"983040",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/AcroForm",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden hostile symbol-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn former_inline_protocol_scalars_cannot_trigger_typed_semantics() {
    for scalar in [
        0xF0003_u32, // pending note
        0xF0005,     // bookmark anchor
        0xF0006,     // bookmark reference
        0xF0007,     // former bookmark terminator
        0xF000B,     // layout advance
        0xF000C,     // footnote reference
        0xF000D,     // former footnote terminator
        0xF000E,     // endnote reference
        0xF000F,     // former endnote terminator
        0xF0010,     // nested-table anchor
        0xF1000,     // first former inline-image marker
    ] {
        let input =
            format!("{{\\rtf1 Before {{\\field{{\\*\\fldinst SYMBOL {scalar}}}}} After\\par}}");
        let parsed = parse_rtf_bytes(input.as_bytes()).expect("reserved scalar remains parseable");

        assert_eq!(collect_text(&parsed.document), "Before \u{fffd} After");
        assert!(parsed.document.inline_images.is_empty());
        assert!(parsed.document.footnotes.is_empty());
        assert!(parsed.document.endnotes.is_empty());
        for block in &parsed.document.blocks {
            if let Block::Paragraph(paragraph) = block {
                assert!(
                    paragraph.runs.iter().all(|run| run.dynamic_field.is_none()),
                    "U+{scalar:05X} created typed inline semantics"
                );
            }
        }
    }
}

#[test]
fn resultless_external_fields_are_placeholdered_without_fetching() {
    let parsed = parse_rtf_bytes(&rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst INCLUDEPICTURE \"https://example.com/pixel.png\"}} After}",
    ]))
    .unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before"));
    assert!(text.contains("[Field removed: no passive result]"));
    assert!(text.contains("After"));
    assert!(!text.contains("INCLUDEPICTURE"));
    assert!(!text.contains("https://example.com"));

    let strip_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Strip,
        ..RtfParseOptions::default()
    };
    let stripped = parse_rtf_bytes_with_options(
        &rtf(&[
            "{",
            "\\",
            "rtf1 Before {",
            "\\",
            "field{",
            "\\",
            "*",
            "\\",
            "fldinst INCLUDEPICTURE \"https://example.com/pixel.png\"}} After}",
        ]),
        &strip_options,
    )
    .unwrap();
    let stripped_text = collect_text(&stripped.document);
    assert!(!stripped_text.contains("[Field removed"));
    assert!(!stripped_text.contains("https://example.com"));
}

#[test]
fn resultless_active_external_fields_do_not_fetch_or_leak_to_pdf() {
    let input = br#"{\rtf1 Visible before
{\field{\*\fldinst INCLUDEPICTURE "https://example.com/pixel.png"}}
{\field{\*\fldinst INCLUDETEXT "https://example.com/doc.rtf"}}
{\field{\*\fldinst HYPERLINK "https://example.com/click"}}
{\field{\*\fldinst LINK Word.Document.8 "https://example.com/doc.doc"}}
{\field{\*\fldinst DDEAUTO Excel "Sheet1" "R1C1"}}
{\field{\*\fldinst DDE Excel "Sheet1" "R2C2"}}
{\field{\*\fldinst IMPORT "https://example.com/data"}}
{\field{\*\fldinst DATABASE \d "https://example.com/db" \s "SELECT * FROM Hidden"}}
visible after\par}"#
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("visible after"));
    assert_eq!(
        text.matches("[Field removed: no passive result]").count(),
        8
    );
    for name in [
        "INCLUDEPICTURE",
        "INCLUDETEXT",
        "HYPERLINK",
        "LINK",
        "DDEAUTO",
        "DDE",
        "IMPORT",
        "DATABASE",
    ] {
        assert!(
            parsed.diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
                &format!(
                    "external field {name} rendered as passive placeholder without fetching external resource"
                )
            )),
            "missing passive external-field diagnostic for {name}: {:?}",
            parsed.diagnostics
        );
    }
    for forbidden in [
        "INCLUDEPICTURE",
        "INCLUDETEXT",
        "HYPERLINK",
        "LINK",
        "DDEAUTO",
        "DDE",
        "IMPORT",
        "DATABASE",
        "example.com",
        "SELECT *",
        "Sheet1",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden active external field leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("active-external-fields.rtf");
    let output_path = dir.path().join("active-external-fields.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("visible after"));
    assert!(rendered_text.contains("[Field removed: no passive result]"));
    for forbidden in [
        b"INCLUDEPICTURE".as_slice(),
        b"INCLUDETEXT",
        b"HYPERLINK",
        b"LINK",
        b"DDEAUTO",
        b"DDE",
        b"IMPORT",
        b"DATABASE",
        b"example.com",
        b"SELECT *",
        b"Sheet1",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden active external field leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_generated_reference_fields_do_not_build_or_leak_to_pdf() {
    let input = br#"{\rtf1 Visible before
{\field{\*\fldinst TOC \o "1-3" \h \z \u}}
index {\field{\*\fldinst INDEX \c "2" \e "Hidden separator"}}
citation {\field{\*\fldinst CITATION HiddenSource \l 1033}}
bib {\field{\*\fldinst BIBLIOGRAPHY}}
toa {\field{\*\fldinst TOA \c 1}}
visible after\par}"#
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("visible after"));
    assert!(text.contains("index [Field removed: no passive result]"));
    assert!(text.contains("citation [Field removed: no passive result]"));
    assert!(text.contains("bib [Field removed: no passive result]"));
    assert!(text.contains("toa [Field removed: no passive result]"));
    assert_eq!(
        text.matches("[Field removed: no passive result]").count(),
        5
    );
    for forbidden in [
        "TOC",
        "INDEX",
        "CITATION",
        "BIBLIOGRAPHY",
        "TOA",
        "HiddenSource",
        "Hidden separator",
        "fldinst",
        "\\o",
        "\\h",
        "\\z",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden generated field leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("visible after"));
    assert!(rendered_text.contains("[Field removed: no passive result]"));
    for name in ["TOC", "INDEX", "CITATION", "BIBLIOGRAPHY", "TOA"] {
        assert!(
            output.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(&format!(
                    "generated field {name} rendered as passive placeholder without synthesizing generated field output"
                ))
            }),
            "missing generated field diagnostic for {name}"
        );
    }
    assert!(
        output
            .diagnostics
            .iter()
            .all(|diagnostic| { !diagnostic.message.contains("generated field TOC removed") })
    );
    for forbidden in [
        b"TOC".as_slice(),
        b"INDEX",
        b"CITATION",
        b"BIBLIOGRAPHY",
        b"TOA",
        b"HiddenSource",
        b"Hidden separator",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden generated field leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_template_barcode_equation_embed_and_gotobutton_fields_stay_passive() {
    let input = br#"{\rtf1 Visible before
{\field{\*\fldinst AUTOTEXT HiddenBlock}}
list {\field{\*\fldinst AUTOTEXTLIST "Hidden menu" \s HiddenStyle}}
barcode {\field{\*\fldinst BARCODE "Hidden address" QR \h 720}}
display {\field{\*\fldinst DISPLAYBARCODE "Hidden code" QR}}
mergebarcode {\field{\*\fldinst MERGEBARCODE "Hidden merge value" QR \h 720}}
eq {\field{\*\fldinst EQ \f(1,2)}}
embed {\field{\*\fldinst EMBED Word.Document.8}}
go {\field{\*\fldinst GOTOBUTTON HiddenBookmark "Visible jump"}}
visible after\par}"#
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("visible after"));
    assert!(text.contains("eq (1,2)"));
    assert!(text.contains("go \"Visible jump\""));
    assert_eq!(
        text.matches("[Field removed: no passive result]").count(),
        6
    );
    let strict = parse_rtf_bytes_with_options(
        &input,
        &RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        },
    )
    .unwrap();
    let strict_text = collect_text(&strict.document);
    assert!(strict_text.contains("eq 1\u{2044}2"));
    assert!(strict_text.contains("go Visible jump"));
    assert!(!strict_text.contains("go \"Visible jump\""));
    for forbidden in [
        "AUTOTEXT",
        "AUTOTEXTLIST",
        "BARCODE",
        "DISPLAYBARCODE",
        "MERGEBARCODE",
        "EMBED",
        "EQ",
        "GOTOBUTTON",
        "HiddenBlock",
        "Hidden menu",
        "HiddenStyle",
        "Hidden address",
        "Hidden code",
        "Hidden merge value",
        "HiddenBookmark",
        "Word.Document.8",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden template/generated/embedded field leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("visible after"));
    assert!(rendered_text.contains("eq (1,2)"));
    assert!(rendered_text.contains("go \"Visible jump\""));
    assert!(rendered_text.contains("[Field removed: no passive result]"));
    for name in ["AUTOTEXT", "AUTOTEXTLIST"] {
        assert!(
            output.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(&format!(
                    "building-block field {name} rendered as passive placeholder without reading template storage"
                ))
            }),
            "missing building-block diagnostic for {name}"
        );
    }
    for name in ["BARCODE", "DISPLAYBARCODE", "MERGEBARCODE"] {
        assert!(
            output.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(&format!(
                    "generated field {name} rendered as passive placeholder without synthesizing generated field output"
                ))
            }),
            "missing generated field diagnostic for {name}"
        );
    }
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "embedded object field EMBED rendered as passive placeholder without activating embedded object",
        )
    }));
    for forbidden in [
        b"AUTOTEXT".as_slice(),
        b"AUTOTEXTLIST",
        b"BARCODE",
        b"DISPLAYBARCODE",
        b"MERGEBARCODE",
        b"EMBED",
        b"GOTOBUTTON",
        b"HiddenBlock",
        b"Hidden menu",
        b"HiddenStyle",
        b"Hidden address",
        b"Hidden code",
        b"Hidden merge value",
        b"HiddenBookmark",
        b"Word.Document.8",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden template/generated/embedded field leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn shape_fields_render_stored_result_without_synthesizing_canvas_or_leaking_to_pdf() {
    let input = br#"{\rtf1 Visible before
stored {\field{\*\fldinst SHAPE \* MERGEFORMAT}{\fldrslt Visible drawing fallback}}
resultless {\field{\*\fldinst SHAPE \* MERGEFORMAT}}
visible after\par}"#
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let visible_text = text.replace(PASSIVE_ADVANCE_MARKER, "");

    assert!(visible_text.contains("Visible before"));
    assert!(visible_text.contains("stored Visible drawing fallback"));
    assert!(visible_text.contains("resultless [Field removed: no passive result]"));
    assert!(visible_text.contains("visible after"));
    for forbidden in ["SHAPE", "MERGEFORMAT", "fldinst", "fldrslt"] {
        assert!(
            !text.contains(forbidden),
            "forbidden SHAPE field content leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("stored Visible drawing fallback"));
    assert!(rendered_text.contains("resultless [Field removed: no passive result]"));
    assert!(rendered_text.contains("visible after"));
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "drawing field SHAPE rendered as passive placeholder without synthesizing drawing canvas output",
        )
    }));
    for forbidden in [
        b"SHAPE".as_slice(),
        b"MERGEFORMAT",
        b"fldinst",
        b"fldrslt",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        PASSIVE_ADVANCE_MARKER.as_bytes(),
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden SHAPE field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_active_control_fields_do_not_create_controls_or_leak_to_pdf() {
    let input = br#"{\rtf1 Visible before
control {\field{\*\fldinst CONTROL Forms.CommandButton.1 "Hidden caption"}}
checkbox {\field{\*\fldinst HTMLCHECKBOX "secret-name" checked}}
input {\field{\*\fldinst HTMLINPUT "password" value="secret"}}
select {\field{\*\fldinst HTMLSELECT "Secret option"}}
visible after\par}"#
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("visible after"));
    assert_eq!(
        text.matches("[Field removed: no passive result]").count(),
        4
    );
    for forbidden in [
        "CONTROL",
        "HTMLCHECKBOX",
        "HTMLINPUT",
        "HTMLSELECT",
        "Forms.CommandButton",
        "Hidden caption",
        "secret-name",
        "password",
        "value=",
        "Secret option",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden active control field leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("visible after"));
    assert!(rendered_text.contains("[Field removed: no passive result]"));
    for name in ["CONTROL", "HTMLCHECKBOX", "HTMLINPUT", "HTMLSELECT"] {
        assert!(
            output.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(&format!(
                    "active control field {name} rendered as passive placeholder without creating active form or control content"
                ))
            }),
            "missing active control diagnostic for {name}"
        );
    }
    assert!(output.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("active control field CONTROL removed")
    }));
    for forbidden in [
        b"CONTROL".as_slice(),
        b"HTMLCHECKBOX",
        b"HTMLINPUT",
        b"HTMLSELECT",
        b"Forms.CommandButton",
        b"Hidden caption",
        b"secret-name",
        b"password",
        b"value=",
        b"Secret option",
        b"fldinst",
        b"/AcroForm",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden active control field leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_environment_fields_do_not_expose_host_state_to_pdf() {
    let input = br#"{\rtf1 Visible before
{\field{\*\fldinst FILENAME \p}}
{\field{\*\fldinst FILESIZE}}
{\field{\*\fldinst TEMPLATE}}
{\field{\*\fldinst USERNAME}}
{\field{\*\fldinst USERINITIALS}}
{\field{\*\fldinst USERADDRESS}}
visible after\par}"#
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("visible after"));
    assert_eq!(
        text.matches("[Field removed: no passive result]").count(),
        5
    );
    for forbidden in [
        "FILENAME",
        "FILESIZE",
        "TEMPLATE",
        "USERNAME",
        "USERINITIALS",
        "USERADDRESS",
        "Users\\",
        "open-rtf-converter",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden environment field leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("visible after"));
    assert!(rendered_text.contains("[Field removed: no passive result]"));
    for name in [
        "FILENAME",
        "TEMPLATE",
        "USERNAME",
        "USERINITIALS",
        "USERADDRESS",
    ] {
        assert!(
            output.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(&format!(
                    "environment field {name} rendered as passive placeholder without exposing converter host state"
                ))
            }),
            "missing environment field diagnostic for {name}"
        );
    }
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("resultless FILESIZE suppressed for Word-compatible passive rendering")
    }));
    assert!(output.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("environment field FILENAME removed")
    }));
    for forbidden in [
        b"FILENAME".as_slice(),
        b"FILESIZE",
        b"TEMPLATE",
        b"USERNAME",
        b"USERINITIALS",
        b"USERADDRESS",
        b"Users\\",
        b"open-rtf-converter",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden environment field leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn strict_spec_filesize_field_renders_metadata_without_host_state_or_instruction_leakage() {
    let input = br##"{\rtf1{\info\nofbytes4096}Visible before size {\field{\*\fldinst FILESIZE \\# "#,##0"}} path {\field{\*\fldinst FILENAME \p}} visible after\par}"##.to_vec();
    let strict_parse_options = RtfParseOptions {
        compatibility_mode: CompatibilityMode::StrictSpec,
        ..RtfParseOptions::default()
    };
    let parsed = parse_rtf_bytes_with_options(&input, &strict_parse_options).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains(
        "Visible before size 4,096 path [Field removed: no passive result] visible after"
    ));
    for forbidden in [
        "FILESIZE",
        "FILENAME",
        "nofbytes",
        "Users\\",
        "open-rtf-converter",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden FILESIZE field content leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            parse_options: strict_parse_options,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains(
        "Visible before size 4,096 path [Field removed: no passive result] visible after"
    ));
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "environment field FILENAME rendered as passive placeholder without exposing converter host state",
        )
    }));
    for forbidden in [
        b"FILESIZE".as_slice(),
        b"FILENAME",
        b"nofbytes",
        b"Users\\",
        b"open-rtf-converter",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden FILESIZE field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_layout_and_clock_fields_do_not_leak_to_pdf() {
    let input = br#"{\rtf1 Visible before
{\field{\*\fldinst ADVANCE \\r 240 \\d 120}}
after advance
date {\field{\*\fldinst DATE \\@ "yyyy-MM-dd host-sentinel"}}
time {\field{\*\fldinst TIME \\@ "HH:mm host-sentinel"}}
visible after\par}"#
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("after advance"));
    assert!(text.contains("date "));
    assert!(text.contains("time "));
    assert!(!text.contains("[Field removed: no passive result]"));
    assert!(text.contains("visible after"));
    for forbidden in [
        "ADVANCE",
        "DATE",
        "TIME",
        "host-sentinel",
        "yyyy-MM-dd",
        "HH:mm",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden layout/clock field leaked to text: {forbidden}"
        );
    }
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "layout field ADVANCE interpreted as bounded passive horizontal cursor advance",
        )
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("layout field ADVANCE interpreted as bounded passive vertical cursor advance")
    }));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("ADVANCE stripped without applying cursor positioning")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("after advance"));
    assert!(rendered_text.contains("date "));
    assert!(rendered_text.contains("time "));
    assert!(!rendered_text.contains("[Field removed: no passive result]"));
    assert!(rendered_text.contains("visible after"));
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "layout field ADVANCE interpreted as bounded passive horizontal cursor advance",
        )
    }));
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("layout field ADVANCE interpreted as bounded passive vertical cursor advance")
    }));
    assert!(output.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("ADVANCE stripped without applying cursor positioning")
    }));
    for name in ["DATE", "TIME"] {
        assert!(
            output.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(&format!(
                    "resultless field {name} omitted for Word-compatible passive rendering without dynamic evaluation"
                ))
            }),
            "missing dynamic clock diagnostic for {name}"
        );
    }
    assert!(output.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("dynamic field DATE removed without reading converter clock")
    }));
    for forbidden in [
        b"ADVANCE".as_slice(),
        b"DATE",
        b"TIME",
        b"host-sentinel",
        b"yyyy-MM-dd",
        b"HH:mm",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden layout/clock field leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_prompt_fields_do_not_request_input_or_leak_to_pdf() {
    let input = br#"{\rtf1 Visible before
{\field{\*\fldinst ASK Client "Hidden prompt" \d "Hidden default"}}
ask-ref {\field{\*\fldinst REF Client}}
fill {\field{\*\fldinst FILLIN "Secret prompt" \d "Secret default"}}
unsafe-fill {\field{\*\fldinst FILLIN "Unsafe prompt" \d "/JavaScript"}}
visible after\par}"#
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("ask-ref [Field removed: no passive result]"));
    assert!(text.contains("fill Secret default"));
    assert!(text.contains("unsafe-fill [Field removed: no passive result]"));
    assert!(text.contains("visible after"));
    for forbidden in [
        "ASK",
        "FILLIN",
        "Client",
        "Hidden prompt",
        "Hidden default",
        "Secret prompt",
        "Unsafe prompt",
        "/JavaScript",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden prompt field leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("ask-ref [Field removed: no passive result]"));
    assert!(rendered_text.contains("fill Secret default"));
    assert!(rendered_text.contains("unsafe-fill [Field removed: no passive result]"));
    assert!(rendered_text.contains("visible after"));
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "prompt field FILLIN rendered from passive default without requesting user input",
        )
    }));
    assert!(output.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("prompt field FILLIN removed without requesting user input")
    }));
    for forbidden in [
        b"ASK".as_slice(),
        b"FILLIN",
        b"Client",
        b"Hidden prompt",
        b"Hidden default",
        b"Secret prompt",
        b"Unsafe prompt",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden prompt field leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_mail_merge_flow_fields_do_not_execute_or_leak_to_pdf() {
    let input = br#"{\rtf1 Visible before
{\field{\*\fldinst NEXT}}
nextif {\field{\*\fldinst NEXTIF "City" = "Paris"}}
skip {\field{\*\fldinst SKIPIF "Status" = "Hidden"}}
record {\field{\*\fldinst MERGEREC}}
seq {\field{\*\fldinst MERGESEQ}}
name {\field{\*\fldinst MERGEFIELD Client}}
visible after\par}"#
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("nextif"));
    assert!(text.contains("skip"));
    assert!(text.contains("record [Field removed: no passive result]"));
    assert!(text.contains("seq [Field removed: no passive result]"));
    assert!(text.contains("name \u{00ab}Client\u{00bb}"));
    assert!(text.contains("visible after"));
    for forbidden in [
        "NEXT",
        "NEXTIF",
        "SKIPIF",
        "MERGEREC",
        "MERGESEQ",
        "MERGEFIELD",
        "City",
        "Paris",
        "Status",
        "Hidden",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden mail-merge field leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("record [Field removed: no passive result]"));
    assert!(rendered_text.contains("seq [Field removed: no passive result]"));
    assert!(rendered_text.contains("name"));
    assert!(rendered_text.contains("Client"));
    assert!(rendered_text.contains("visible after"));
    for name in ["MERGEREC", "MERGESEQ"] {
        assert!(
            output.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(&format!(
                    "mail-merge field {name} rendered as passive placeholder without executing merge data source"
                ))
            }),
            "missing mail-merge diagnostic for {name}"
        );
    }
    for forbidden in [
        b"NEXT".as_slice(),
        b"NEXTIF",
        b"SKIPIF",
        b"MERGEREC",
        b"MERGESEQ",
        b"MERGEFIELD",
        b"City",
        b"Paris",
        b"Status",
        b"Hidden",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden mail-merge field leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_mail_merge_address_fields_do_not_execute_or_leak_to_pdf() {
    let input = br#"{\rtf1 Visible before
address {\field{\*\fldinst ADDRESSBLOCK \f "<<_COMPANY_>>\r<<_ADDRESS1_>>" \l 1033}}
greeting {\field{\*\fldinst GREETINGLINE \f "<<_TITLE0_>> <<_LAST0_>>" \e "Dear Sir or Madam,"}}
unsafe-greeting {\field{\*\fldinst GREETINGLINE \e "/JavaScript"}}
visible after\par}"#
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("address [Field removed: no passive result]"));
    assert!(text.contains("greeting Dear Sir or Madam,"));
    assert!(text.contains("unsafe-greeting [Field removed: no passive result]"));
    assert!(text.contains("visible after"));
    for forbidden in [
        "ADDRESSBLOCK",
        "GREETINGLINE",
        "_COMPANY_",
        "_ADDRESS1_",
        "_TITLE0_",
        "_LAST0_",
        "/JavaScript",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden mail-merge address field leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("address [Field removed: no passive result]"));
    assert!(rendered_text.contains("greeting Dear Sir or Madam,"));
    assert!(rendered_text.contains("unsafe-greeting [Field removed: no passive result]"));
    assert!(rendered_text.contains("visible after"));
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "mail-merge field GREETINGLINE rendered from passive fallback without executing merge data source",
        )
    }));
    for name in ["ADDRESSBLOCK"] {
        assert!(
            output.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(&format!(
                    "mail-merge field {name} rendered as passive placeholder without executing merge data source"
                ))
            }),
            "missing mail-merge diagnostic for {name}"
        );
    }
    assert!(output.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("mail-merge field ADDRESSBLOCK removed")
    }));
    for forbidden in [
        b"ADDRESSBLOCK".as_slice(),
        b"GREETINGLINE",
        b"_COMPANY_",
        b"_ADDRESS1_",
        b"_TITLE0_",
        b"_LAST0_",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden mail-merge address field leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_private_and_print_fields_are_stripped_without_pdf_leakage() {
    let input = br#"{\rtf1 Visible before
{\field{\*\fldinst ADDIN "Hidden addin payload"}}
addin {\field{\*\fldinst PRIVATE "414243 hidden private"}}
private {\field{\*\fldinst PRINT "hidden printer command"}}
print {\field{\*\fldinst RD "C:\\secret\\source.rtf"}}
visible after\par}"#
        .to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("addin"));
    assert!(text.contains("private"));
    assert!(text.contains("print"));
    assert!(text.contains("visible after"));
    assert!(!text.contains("[Field removed"));
    for forbidden in [
        "ADDIN",
        "PRIVATE",
        "PRINT",
        "RD",
        "Hidden addin payload",
        "414243",
        "hidden private",
        "hidden printer command",
        "secret",
        "source.rtf",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden hidden active/storage field leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("addin"));
    assert!(rendered_text.contains("private"));
    assert!(rendered_text.contains("print"));
    assert!(rendered_text.contains("visible after"));
    assert!(!rendered_text.contains("[Field removed"));
    for forbidden in [
        b"ADDIN".as_slice(),
        b"PRIVATE",
        b"PRINT",
        b"Hidden addin payload",
        b"414243",
        b"hidden private",
        b"hidden printer command",
        b"secret",
        b"source.rtf",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden hidden active/storage field leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn edit_time_field_uses_passive_metadata_without_pdf_leakage() {
    let input = br#"{\rtf1{\info{\edmins42}{\title Hidden title {\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt Hidden link}}}}Edit {\field{\*\fldinst EDITTIME \\# "0000"}} dynamic {\field{\*\fldinst TIME}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);

    assert!(text.contains("Edit 0042"));
    assert!(text.contains("dynamic [Field removed: no passive result]"));
    for forbidden in [
        "EDITTIME",
        "edmins",
        "TIME",
        "HYPERLINK",
        "https://example.com",
        "Hidden title",
        "Hidden link",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden edit-time metadata leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("edit-time-field.rtf");
    let output_path = dir.path().join("edit-time-field.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            parse_options: RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Edit 0042"));
    assert!(rendered_text.contains("dynamic [Field removed: no passive result]"));
    for forbidden in [
        b"EDITTIME".as_slice(),
        b"edmins",
        b"HYPERLINK",
        b"https://example.com",
        b"Hidden title",
        b"Hidden link",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden edit-time metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn revision_number_field_uses_passive_metadata_without_pdf_leakage() {
    let input = br#"{\rtf1{\info{\version12}{\subject Hidden subject {\field{\*\fldinst HYPERLINK "https://example.com/revision"}{\fldrslt Hidden link}}}}Revision {\field{\*\fldinst REVNUM \\# "0000"}} dynamic {\field{\*\fldinst DATE}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);

    assert!(text.contains("Revision 0012"));
    assert!(text.contains("dynamic [Field removed: no passive result]"));
    for forbidden in [
        "REVNUM",
        "version",
        "DATE",
        "HYPERLINK",
        "https://example.com",
        "Hidden subject",
        "Hidden link",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden revision metadata leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("revision-number-field.rtf");
    let output_path = dir.path().join("revision-number-field.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            parse_options: RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Revision 0012"));
    assert!(rendered_text.contains("dynamic [Field removed: no passive result]"));
    for forbidden in [
        b"REVNUM".as_slice(),
        b"version",
        b"HYPERLINK",
        b"https://example.com",
        b"Hidden subject",
        b"Hidden link",
        b"fldinst",
        b"/Action",
        b"/Annots",
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden revision metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_index_entry_fields_are_stripped_without_pdf_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Visible before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst XE \"https://example.com/hidden-index\"}} visible after {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst TC \"Hidden TOC entry\"}}{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst TA \"Hidden authority entry\"}}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before"));
    assert!(text.contains("visible after"));
    assert!(!text.contains("[Field removed"));
    for forbidden in [
        "https://example.com",
        "Hidden TOC",
        "Hidden authority",
        "fldinst",
        "XE",
        "TC",
        "TA",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden non-visible field content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("index-entry-fields.rtf");
    let output_path = dir.path().join("index-entry-fields.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible before"));
    assert!(rendered_text.contains("visible after"));
    for forbidden in [
        b"https://example.com".as_slice(),
        b"Hidden TOC",
        b"Hidden authority",
        b"fldinst",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden non-visible field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn form_field_metadata_does_not_create_pdf_forms_or_leak_payloads() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst FORMTEXT}{",
        "\\",
        "formfield{",
        "\\",
        "fftype0}{",
        "\\",
        "ffname HiddenName}{",
        "\\",
        "ffdeftext HiddenDefault}{",
        "\\",
        "ffentrymcr launch.exe}{",
        "\\",
        "ffexitmcr https://example.com/macro}{",
        "\\",
        "datafield 414243}}{",
        "\\",
        "fldrslt Visible value}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before Visible value After"));
    for forbidden in [
        "FORMTEXT",
        "HiddenName",
        "HiddenDefault",
        "launch.exe",
        "https://example.com",
        "414243",
        "ffentrymcr",
        "ffexitmcr",
        "datafield",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden form-field metadata leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("form-field-metadata.rtf");
    let output_path = dir.path().join("form-field-metadata.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(decoded_pdf_text(&content).contains("Before Visible value After"));
    for forbidden in [
        b"FORMTEXT".as_slice(),
        b"HiddenName",
        b"HiddenDefault",
        b"launch.exe",
        b"https://example.com",
        b"414243",
        b"ffentrymcr",
        b"ffexitmcr",
        b"datafield",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden form-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn form_field_macros_obey_reject_policy_with_word_compatible_blank_defaults() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst FORMTEXT}{",
        "\\",
        "formfield{",
        "\\",
        "fftype0}{",
        "\\",
        "ffdeftext Passive default}{",
        "\\",
        "ffentrymcr launch.exe}{",
        "\\",
        "ffexitmcr https://example.com/macro}}} After",
        "\\",
        "par}",
    ]);

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Before  After"));
    assert!(!text.contains("launch.exe"));
    assert!(!text.contains("https://example.com/macro"));

    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };
    assert!(matches!(
        parse_rtf_bytes_with_options(&input, &reject_options),
        Err(ParseError::ActiveContentRejected { feature, .. }) if feature == "field instruction"
    ));
    assert!(matches!(
        parse_rtf_bytes_with_options(
            br"{\rtf1 Visible{\formfield{\ffentrymcr launch.exe}} body\par}",
            &reject_options,
        ),
        Err(ParseError::ActiveContentRejected { feature, .. }) if feature == "form field macro"
    ));
}

#[test]
fn resultless_form_text_is_blank_in_word_compatible_pdf_without_metadata() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst FORMTEXT}{",
        "\\",
        "formfield{",
        "\\",
        "fftype0}{",
        "\\",
        "ffname HiddenName}{",
        "\\",
        "ffdeftext Default value}{",
        "\\",
        "ffentrymcr launch.exe}{",
        "\\",
        "ffexitmcr https://example.com/macro}{",
        "\\",
        "datafield 414243}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before  After"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("form field FORMTEXT is editor-only and omitted from passive print output")
    }));
    for forbidden in [
        "FORMTEXT",
        "HiddenName",
        "launch.exe",
        "https://example.com",
        "414243",
        "ffentrymcr",
        "ffexitmcr",
        "datafield",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden form-text metadata leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("form-text-default.rtf");
    let output_path = dir.path().join("form-text-default.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(decoded_pdf_text(&content).contains("Before  After"));
    for forbidden in [
        b"FORMTEXT".as_slice(),
        b"HiddenName",
        b"launch.exe",
        b"https://example.com",
        b"414243",
        b"ffentrymcr",
        b"ffexitmcr",
        b"datafield",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden form-text content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn formshade_is_omitted_from_stored_form_field_result_in_word_compatible_output() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "formshade Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst FORMTEXT}{",
        "\\",
        "formfield{",
        "\\",
        "fftype0}{",
        "\\",
        "ffname HiddenName}{",
        "\\",
        "ffdeftext HiddenDefault}{",
        "\\",
        "ffentrymcr launch.exe}{",
        "\\",
        "datafield 414243}}{",
        "\\",
        "fldrslt Visible value}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        other => panic!("expected paragraph, got {other:?}"),
    };
    let form_run = paragraph
        .runs
        .iter()
        .find(|run| run.text.contains("Visible value"))
        .expect("stored form result run");

    assert!(text.contains("Before Visible value After"));
    assert!(
        !form_run.style.form_field_shading,
        "stored form-field result should omit editor-only shading in print output"
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("form-field shading is editor-only and omitted from passive print output")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "rendering stored result for passive form field FORMTEXT without creating PDF form actions",
        )
    }));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic.message.contains(
            "rendering stored result for field FORMTEXT without executing field instruction",
        )
    }));
    for forbidden in [
        "FORMTEXT",
        "HiddenName",
        "HiddenDefault",
        "launch.exe",
        "414243",
        "formshade",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden stored formshade metadata leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(decoded_pdf_text(&content).contains("Before Visible value After"));
    assert!(
        content
            .operations
            .iter()
            .all(|operation| operation.operator != "re")
    );
    for forbidden in [
        b"FORMTEXT".as_slice(),
        b"HiddenName",
        b"HiddenDefault",
        b"launch.exe",
        b"414243",
        b"formshade",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden stored formshade content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_form_checkbox_is_blank_in_word_compatible_pdf_without_metadata() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst FORMCHECKBOX}{",
        "\\",
        "formfield{",
        "\\",
        "fftype1}{",
        "\\",
        "ffname HiddenName}{",
        "\\",
        "ffdefres0}{",
        "\\",
        "ffres1}{",
        "\\",
        "ffentrymcr launch.exe}{",
        "\\",
        "datafield 414243}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before  After"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive form field FORMCHECKBOX without creating PDF form actions")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("form checkbox is editor-only and omitted from passive print output")
    }));
    for forbidden in [
        "FORMCHECKBOX",
        "HiddenName",
        "launch.exe",
        "414243",
        "ffentrymcr",
        "datafield",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden checkbox form-field metadata leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("form-checkbox.rtf");
    let output_path = dir.path().join("form-checkbox.pdf");
    fs::write(&input_path, &input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    let stroke_count = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "S")
        .count();

    assert!(rendered_text.contains("Before "));
    assert!(rendered_text.contains(" After"));
    assert_eq!(stroke_count, 0, "Word-compatible checkbox should not print");
    let strict_output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            parse_options: RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let strict_pdf = PdfDocument::load_mem(&strict_output.pdf).unwrap();
    let strict_page_id = *strict_pdf.get_pages().values().next().expect("strict page");
    let strict_content = strict_pdf
        .get_and_decode_page_content(strict_page_id)
        .unwrap();
    assert_passive_checkbox_vectors_without_zapf(
        &strict_output.pdf,
        &strict_content,
        "StrictSpec checked form checkbox",
    );
    for forbidden in [
        b"FORMCHECKBOX".as_slice(),
        b"HiddenName",
        b"launch.exe",
        b"414243",
        b"ffentrymcr",
        b"datafield",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden checkbox form-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn unchecked_form_checkbox_is_blank_in_word_compatible_pdf() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst FORMCHECKBOX}{",
        "\\",
        "formfield{",
        "\\",
        "fftype1}{",
        "\\",
        "ffname HiddenUnchecked}{",
        "\\",
        "ffres0}{",
        "\\",
        "ffentrymcr launch.exe}{",
        "\\",
        "datafield 414243}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before  After"));
    for forbidden in [
        "FORMCHECKBOX",
        "HiddenUnchecked",
        "launch.exe",
        "414243",
        "ffentrymcr",
        "datafield",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden unchecked checkbox metadata leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("unchecked-form-checkbox.rtf");
    let output_path = dir.path().join("unchecked-form-checkbox.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before "));
    assert!(rendered_text.contains(" After"));
    assert!(
        content
            .operations
            .iter()
            .all(|operation| operation.operator != "S")
    );
    for forbidden in [
        b"FORMCHECKBOX".as_slice(),
        b"HiddenUnchecked",
        b"launch.exe",
        b"414243",
        b"ffentrymcr",
        b"datafield",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden unchecked checkbox content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn sized_form_checkbox_is_blank_in_word_compatible_pdf_without_metadata() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "fs20 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst FORMCHECKBOX}{",
        "\\",
        "formfield{",
        "\\",
        "fftype1}{",
        "\\",
        "ffres1}{",
        "\\",
        "ffhps40}{",
        "\\",
        "ffentrymcr launch.exe}{",
        "\\",
        "datafield 414243}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let before_style = run_style_for_text(&parsed.document, "Before  After").expect("ambient run");

    assert!(text.contains("Before  After"));
    assert_eq!(before_style.font_size_half_points, 20);
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rendering passive form field FORMCHECKBOX without creating PDF form actions")
    }));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before "));
    assert!(rendered_text.contains(" After"));
    assert!(
        content
            .operations
            .iter()
            .all(|operation| operation.operator != "S")
    );
    for forbidden in [
        b"FORMCHECKBOX".as_slice(),
        b"ffhps",
        b"launch.exe",
        b"414243",
        b"ffentrymcr",
        b"datafield",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden sized checkbox content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_form_dropdown_is_blank_in_word_compatible_pdf_without_metadata() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst FORMDROPDOWN}{",
        "\\",
        "formfield{",
        "\\",
        "fftype2}{",
        "\\",
        "ffname HiddenName}{",
        "\\",
        "ffdefres0}{",
        "\\",
        "ffres1}{",
        "\\",
        "*",
        "\\",
        "ffl First choice}{",
        "\\",
        "*",
        "\\",
        "ffl Second choice}{",
        "\\",
        "ffentrymcr launch.exe}{",
        "\\",
        "datafield 414243}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before  After"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "form field FORMDROPDOWN is editor-only and omitted from passive print output",
        )
    }));
    for forbidden in [
        "FORMDROPDOWN",
        "HiddenName",
        "First choice",
        "launch.exe",
        "414243",
        "ffentrymcr",
        "datafield",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden dropdown form-field metadata leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("form-dropdown.rtf");
    let output_path = dir.path().join("form-dropdown.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();

    assert!(decoded_pdf_text(&content).contains("Before  After"));
    for forbidden in [
        b"FORMDROPDOWN".as_slice(),
        b"HiddenName",
        b"First choice",
        b"launch.exe",
        b"414243",
        b"ffentrymcr",
        b"datafield",
        b"/AcroForm",
        b"/Widget",
        b"/AA",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden dropdown form-field content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn mail_merge_metadata_does_not_reach_text_or_pdf() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Visible before {",
        "\\",
        "mailmerge{",
        "\\",
        "mmconnectstr Provider=SQLOLEDB;Password=secret;Data Source=https://example.com/db}{",
        "\\",
        "mmdatasource C:",
        "\\",
        "remote",
        "\\",
        "contacts.mdb}{",
        "\\",
        "mmquery SELECT * FROM Contacts WHERE Url='https://example.com/query'}{",
        "\\",
        "mmodsoudl https://example.com/source.udl}{",
        "\\",
        "object",
        "\\",
        "objdata ",
        payload_hex(),
        "}} visible after",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before  visible after"));
    for forbidden in [
        "Provider=SQLOLEDB",
        "Password=secret",
        "Data Source",
        "contacts.mdb",
        "SELECT *",
        "example.com",
        payload_hex(),
        "[Embedded object removed]",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden mail merge content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("mail-merge-metadata.rtf");
    let output_path = dir.path().join("mail-merge-metadata.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    assert!(PdfDocument::load_mem(&pdf).is_ok());
    for forbidden in [
        b"Provider=SQLOLEDB".as_slice(),
        b"Password=secret",
        b"Data Source",
        b"contacts.mdb",
        b"SELECT *",
        b"example.com",
        payload_hex().as_bytes(),
        b"/URI",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden mail merge content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn mail_merge_metadata_obeys_reject_policy() {
    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };

    assert!(matches!(
        parse_rtf_bytes_with_options(
            br"{\rtf1 Visible{\mailmerge{\mmconnectstr Provider=SQLOLEDB;Password=secret}} body\par}",
            &reject_options,
        ),
        Err(ParseError::ActiveContentRejected { feature, .. }) if feature == "mail merge data source"
    ));
}

#[test]
fn bitfields_dib_picture_renders_passively_without_payload_leakage() {
    let image_hex = bytes_to_hex(&minimal_16bit_bitfields_dib_with_dimensions(2, 1));
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 before {",
        "\\",
        "pict",
        "\\",
        "dibitmap0",
        "\\",
        "picwgoal720",
        "\\",
        "pichgoal720 ",
        image_hex.as_str(),
        "} after",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("before"));
    assert!(text.contains("after"));
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            open_rtf_converter::model::Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("bitfields DIB image block");
    assert_eq!(image.format, open_rtf_converter::model::ImageFormat::Rgb8);
    assert_eq!(image.width_px, 2);
    assert_eq!(image.height_px, 1);
    assert_eq!(image.bytes, vec![255, 0, 0, 0, 255, 0]);
    assert!(!text.contains("dibitmap"));

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    assert_eq!(parsed_pdf.get_pages().len(), 1);
    assert!(
        output
            .pdf
            .windows(b"/Subtype /Image".len())
            .any(|window| window == b"/Subtype /Image")
    );
    for forbidden in [
        b"dibitmap".as_slice(),
        image_hex.as_bytes(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden bitfields DIB content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_emf_transform_state_records_count_as_partial_without_payload_leakage() {
    for (record_type, label) in [
        (35, "EMF-SETWORLDTRANSFORM-MALFORMED"),
        (36, "EMF-MODIFYWORLDTRANSFORM-MALFORMED"),
        (115, "EMF-SETLAYOUT-MALFORMED"),
    ] {
        let records = [
            emf_unknown_record(record_type),
            emf_rect_record(43, 20, 10, 80, 50),
        ];
        let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
        let emf_hex = bytes_to_hex(&emf);
        let input =
            format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
        let parsed = parse_rtf_bytes(&input).unwrap();
        let text = collect_text(&parsed.document);
        let image = parsed
            .document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Image(image) => Some(image),
                _ => None,
            })
            .expect("malformed EMF transform/layout partial vector preview");

        assert!(text.contains("before"));
        assert!(text.contains("after"));
        assert_eq!(image.format, ImageFormat::WmfVector, "{label}");
        assert!(image.bytes.is_empty(), "{label}");
        assert!(
            image
                .vector_commands
                .iter()
                .any(|command| matches!(command, StaticImageVectorCommand::Rectangle { .. })),
            "safe drawing after {label} should stay visible"
        );
        for forbidden in ["emfblip", "JavaScript", "EmbeddedFile", "Launch"] {
            assert!(
                !text.contains(forbidden),
                "malformed EMF transform/layout payload/control leaked to normalized text: {forbidden}"
            );
        }

        let output = convert_rtf_to_pdf(
            &input,
            &ConvertOptions {
                diagnostics: true,
                ..ConvertOptions::browser_safe_defaults()
            },
        )
        .unwrap();
        assert!(
            output.diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("1 unsupported record(s) skipped")),
            "{label} should produce skipped-record diagnostics: {:?}",
            output.diagnostics
        );
        let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
        let page_id = *parsed_pdf.get_pages().values().next().expect("page");
        let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
        let rendered_text = decoded_pdf_text(&content);

        assert!(rendered_text.contains("before"));
        assert!(rendered_text.contains("after"));
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "re"),
            "safe drawing after {label} should render passively"
        );
        for forbidden in [
            b"emfblip".as_slice(),
            emf_hex.as_bytes(),
            b"/JavaScript",
            b"/EmbeddedFile",
            b"/Subtype /Image",
            b"/Launch",
            b"/OpenAction",
            b"/RichMedia",
        ] {
            assert!(
                !output
                    .pdf
                    .windows(forbidden.len())
                    .any(|window| window == forbidden),
                "malformed EMF transform/layout payload leaked to PDF: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn emf_identity_setworldtransform_is_passive_state_without_payload_leakage() {
    let mut mode = emf_xform_record(35, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    mode.extend_from_slice(b"EMF-SETWORLDTRANSFORM-IDENTITY /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("passive EMF identity world transform vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "EMF-SETWORLDTRANSFORM-IDENTITY",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF SETWORLDTRANSFORM payload/control leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after identity EMF SETWORLDTRANSFORM should render a passive PDF rectangle"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETWORLDTRANSFORM-IDENTITY",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Subtype /Image",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "EMF SETWORLDTRANSFORM payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn scale_translate_emf_setworldtransform_maps_passive_geometry_without_payload_leakage() {
    let mut mode = emf_xform_record(35, [2.0, 0.0, 0.0, 2.0, 10.0, 5.0]);
    mode.extend_from_slice(b"EMF-SETWORLDTRANSFORM-SCALE /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("passive EMF transformed vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 10.0,
            top: 5.0,
            right: 160.0,
            bottom: 80.0,
            ..
        }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "EMF-SETWORLDTRANSFORM-SCALE",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF SETWORLDTRANSFORM payload/control leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    assert!(output.diagnostics.is_empty());
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "transformed EMF SETWORLDTRANSFORM should render a passive PDF rectangle"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETWORLDTRANSFORM-SCALE",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Subtype /Image",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "EMF SETWORLDTRANSFORM scale payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn scale_emf_setworldtransform_maps_passive_pen_width_without_payload_leakage() {
    let mut mode = emf_xform_record(35, [2.0, 0.0, 0.0, 2.0, 0.0, 0.0]);
    mode.extend_from_slice(b"EMF-SETWORLDTRANSFORM-PENWIDTH /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [
        mode,
        emf_create_pen_record(
            2,
            0,
            3,
            Color {
                red: 20,
                green: 40,
                blue: 60,
            },
        ),
        emf_select_object_record(2),
        emf_point_record(27, 10, 10),
        emf_point_record(54, 70, 10),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("passive EMF transformed pen-width vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Line {
            x1: 20.0,
            y1: 20.0,
            x2: 140.0,
            y2: 20.0,
            stroke_width: 6.0,
            ..
        }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "EMF-SETWORLDTRANSFORM-PENWIDTH",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF SETWORLDTRANSFORM pen-width payload/control leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    assert!(output.diagnostics.is_empty());
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETWORLDTRANSFORM-PENWIDTH",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Subtype /Image",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "EMF SETWORLDTRANSFORM pen-width payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn later_emf_setworldtransform_rescales_selected_pen_width_without_payload_leakage() {
    let mut mode = emf_xform_record(35, [2.0, 0.0, 0.0, 2.0, 0.0, 0.0]);
    mode.extend_from_slice(b"EMF-SETWORLDTRANSFORM-LATE-PENWIDTH /JavaScript /EmbeddedFile");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [
        emf_create_pen_record(
            2,
            0,
            3,
            Color {
                red: 20,
                green: 40,
                blue: 60,
            },
        ),
        emf_select_object_record(2),
        mode,
        emf_point_record(27, 10, 10),
        emf_point_record(54, 70, 10),
    ];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("passive EMF late transformed pen-width vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Line {
            x1: 20.0,
            y1: 20.0,
            x2: 140.0,
            y2: 20.0,
            stroke_width: 6.0,
            ..
        }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "EMF-SETWORLDTRANSFORM-LATE-PENWIDTH",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "late EMF SETWORLDTRANSFORM pen-width payload/control leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    assert!(output.diagnostics.is_empty());
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETWORLDTRANSFORM-LATE-PENWIDTH",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Subtype /Image",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "late EMF SETWORLDTRANSFORM pen-width payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn shear_emf_setworldtransform_stays_partial_without_payload_leakage() {
    let mut mode = emf_xform_record(35, [1.0, 0.25, 0.0, 1.0, 0.0, 0.0]);
    mode.extend_from_slice(b"EMF-SETWORLDTRANSFORM-SHEAR /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("1 unsupported record(s) skipped")
    }));
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-SETWORLDTRANSFORM-SHEAR",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Subtype /Image",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "unsupported EMF SETWORLDTRANSFORM shear payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_emf_setworldtransform_becomes_passive_placeholder() {
    let mut mode = emf_unknown_record(35);
    write_test_le_u32(&mut mode, 4, 8);
    let records = [mode];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {}}} after\\par}}",
        bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("malformed EMF world transform placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn emf_identity_modifyworldtransform_is_passive_state_without_payload_leakage() {
    let mut mode = emf_modifyworldtransform_record(1, [2.0, 0.0, 0.0, 2.0, 3.0, 4.0]);
    mode.extend_from_slice(b"EMF-MODIFYWORLDTRANSFORM-IDENTITY /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("passive EMF identity modify world transform vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle { .. }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "EMF-MODIFYWORLDTRANSFORM-IDENTITY",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF MODIFYWORLDTRANSFORM payload/control leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "drawing after EMF MODIFYWORLDTRANSFORM MWT_IDENTITY should render a passive PDF rectangle"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-MODIFYWORLDTRANSFORM-IDENTITY",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Subtype /Image",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "EMF MODIFYWORLDTRANSFORM payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn set_mode_emf_modifyworldtransform_maps_passive_geometry_without_payload_leakage() {
    let mut mode = emf_modifyworldtransform_record(4, [2.0, 0.0, 0.0, 2.0, 10.0, 5.0]);
    mode.extend_from_slice(b"EMF-MODIFYWORLDTRANSFORM-SET /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("passive EMF modify-world-transform vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 10.0,
            top: 5.0,
            right: 160.0,
            bottom: 80.0,
            ..
        }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "EMF-MODIFYWORLDTRANSFORM-SET",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF MODIFYWORLDTRANSFORM payload/control leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    assert!(output.diagnostics.is_empty());
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "EMF MODIFYWORLDTRANSFORM MWT_SET should render a passive PDF rectangle"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-MODIFYWORLDTRANSFORM-SET",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Subtype /Image",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "EMF MODIFYWORLDTRANSFORM MWT_SET payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn left_multiply_emf_modifyworldtransform_composes_passive_geometry_without_payload_leakage() {
    let set = emf_xform_record(35, [2.0, 0.0, 0.0, 2.0, 10.0, 5.0]);
    let mut multiply = emf_modifyworldtransform_record(2, [3.0, 0.0, 0.0, 4.0, 7.0, 11.0]);
    multiply.extend_from_slice(b"EMF-MODIFYWORLDTRANSFORM-LEFT /JavaScript /EmbeddedFile /Launch");
    multiply.resize(multiply.len().next_multiple_of(4), 0);
    let multiply_len = multiply.len() as u32;
    write_test_le_u32(&mut multiply, 4, multiply_len);
    let records = [set, multiply, emf_rect_record(43, 0, 0, 10, 10)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("passive EMF left-multiplied modify-world-transform vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 37.0,
            top: 31.0,
            right: 97.0,
            bottom: 80.0,
            ..
        }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "EMF-MODIFYWORLDTRANSFORM-LEFT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF left multiply transform payload/control leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    assert!(output.diagnostics.is_empty());
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-MODIFYWORLDTRANSFORM-LEFT",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Subtype /Image",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "EMF left multiply transform payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn right_multiply_emf_modifyworldtransform_composes_passive_geometry_without_payload_leakage() {
    let set = emf_xform_record(35, [2.0, 0.0, 0.0, 2.0, 10.0, 5.0]);
    let mut multiply = emf_modifyworldtransform_record(3, [3.0, 0.0, 0.0, 4.0, 7.0, 11.0]);
    multiply.extend_from_slice(b"EMF-MODIFYWORLDTRANSFORM-RIGHT /JavaScript /EmbeddedFile /Launch");
    multiply.resize(multiply.len().next_multiple_of(4), 0);
    let multiply_len = multiply.len() as u32;
    write_test_le_u32(&mut multiply, 4, multiply_len);
    let records = [set, multiply, emf_rect_record(43, 0, 0, 10, 10)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("passive EMF right-multiplied modify-world-transform vector image");

    assert!(text.contains("before"));
    assert!(text.contains("after"));
    assert_eq!(image.format, ImageFormat::WmfVector);
    assert!(image.bytes.is_empty());
    assert_eq!(image.vector_commands.len(), 1);
    assert!(matches!(
        image.vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            left: 24.0,
            top: 27.0,
            right: 84.0,
            bottom: 80.0,
            ..
        }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "EMF-MODIFYWORLDTRANSFORM-RIGHT",
        "JavaScript",
        "EmbeddedFile",
        "Launch",
    ] {
        assert!(
            !text.contains(forbidden),
            "EMF right multiply transform payload/control leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    assert!(output.diagnostics.is_empty());
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-MODIFYWORLDTRANSFORM-RIGHT",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Subtype /Image",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "EMF right multiply transform payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn shear_emf_modifyworldtransform_stays_partial_without_payload_leakage() {
    let mut mode = emf_modifyworldtransform_record(4, [1.0, 0.25, 0.0, 1.0, 0.0, 0.0]);
    mode.extend_from_slice(b"EMF-MODIFYWORLDTRANSFORM-SHEAR /JavaScript /EmbeddedFile /Launch");
    mode.resize(mode.len().next_multiple_of(4), 0);
    let mode_len = mode.len() as u32;
    write_test_le_u32(&mut mode, 4, mode_len);
    let records = [mode, emf_rect_record(43, 0, 0, 80, 40)];
    let emf = minimal_emf_with_records(160, 80, 2540, 1270, &records);
    let emf_hex = bytes_to_hex(&emf);
    let input = format!("{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} after\\par}}").into_bytes();
    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("1 unsupported record(s) skipped")
    }));
    for forbidden in [
        b"emfblip".as_slice(),
        emf_hex.as_bytes(),
        b"EMF-MODIFYWORLDTRANSFORM-SHEAR",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Subtype /Image",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "unsupported EMF MODIFYWORLDTRANSFORM shear payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn malformed_emf_modifyworldtransform_becomes_passive_placeholder() {
    let mut mode = emf_unknown_record(36);
    write_test_le_u32(&mut mode, 4, 8);
    let records = [mode];
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {}}} after\\par}}",
        bytes_to_hex(&minimal_emf_with_records(160, 80, 2540, 1270, &records))
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let image = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .expect("malformed EMF modify world transform placeholder");

    assert_eq!(image.format, ImageFormat::Placeholder);
    assert!(image.bytes.is_empty());
    assert!(image.vector_commands.is_empty());
}

#[test]
fn bitfields_dib_vector_rasters_render_as_passive_images_without_payload_leakage() {
    let mut emf_dib =
        minimal_32bit_bitfields_dib_with_rgba_pixels(2, 1, &[[255, 0, 0, 64], [0, 255, 0, 192]]);
    emf_dib.extend_from_slice(b"TRAILING-EMF-BITFIELDS-DIB /JavaScript");
    let mut wmf_dib = minimal_16bit_bitfields_dib_with_dimensions(2, 1);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-BITFIELDS-DIB /EmbeddedFile");
    let emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[emf_stretchdibits_dib_record(
            20,
            15,
            60,
            30,
            0x00cc_0020,
            &emf_dib,
        )],
    );
    let wmf = minimal_wmf_with_records(
        200,
        100,
        &[wmf_stretchdib_dib_record(
            35,
            25,
            80,
            40,
            0x00cc_0020,
            &wmf_dib,
        )],
    );
    let emf_hex = bytes_to_hex(&emf);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} middle {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let images: Vec<_> = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Image(image) => Some(image),
            _ => None,
        })
        .collect();

    assert!(text.contains("before"));
    assert!(text.contains("middle"));
    assert!(text.contains("after"));
    assert_eq!(images.len(), 2);
    for image in &images {
        assert_eq!(image.format, ImageFormat::WmfVector);
        assert!(image.bytes.is_empty());
        assert_eq!(image.vector_commands.len(), 1);
    }
    assert!(matches!(
        &images[0].vector_commands[0],
        StaticImageVectorCommand::RasterImage { image, .. }
            if image.format == ImageFormat::Rgb8
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == vec![255, 0, 0, 0, 255, 0]
                && image.alpha_mask.is_none()
    ));
    assert!(matches!(
        &images[1].vector_commands[0],
        StaticImageVectorCommand::RasterImage { image, .. }
            if image.format == ImageFormat::Rgb8
                && image.width_px == 2
                && image.height_px == 1
                && image.bytes == vec![255, 0, 0, 0, 255, 0]
                && image.alpha_mask.is_none()
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-BITFIELDS-DIB",
        "TRAILING-WMF-BITFIELDS-DIB",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "bitfields DIB payload/control leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("middle"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "Do")
            .count()
            >= 2,
        "bitfields DIB vectors should render passive image XObjects"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-BITFIELDS-DIB",
        b"TRAILING-WMF-BITFIELDS-DIB",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "bitfields DIB payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn modern_shape_text_renders_passively_without_property_or_field_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before",
        "\\",
        "par{",
        "\\",
        "shp{",
        "\\",
        "shpinst{",
        "\\",
        "sp{",
        "\\",
        "sn pFragments}{",
        "\\",
        "sv hostile-shape-property}}{",
        "\\",
        "shptxt Modern box text {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst HYPERLINK \"https://example.com/box\"}{",
        "\\",
        "fldrslt safe link}}",
        "\\",
        "par}}}After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before"));
    assert!(text.contains("Modern box text safe link"));
    assert!(text.contains("After"));
    for forbidden in [
        "shpinst",
        "pFragments",
        "hostile-shape-property",
        "shptxt",
        "HYPERLINK",
        "https://example.com/box",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden modern shape-text content leaked to text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("Modern box text safe link"));
    assert!(rendered_text.contains("After"));
    for forbidden in [
        b"shpinst".as_slice(),
        b"pFragments",
        b"hostile-shape-property",
        b"shptxt",
        b"HYPERLINK",
        b"https://example.com/box",
        b"/Action",
        b"/Annots",
        b"/JavaScript",
        b"/Launch",
        b"/OpenAction",
        b"/RichMedia",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden modern shape-text content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}
