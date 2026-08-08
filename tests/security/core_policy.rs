use super::*;

#[test]
fn ignored_pn_metadata_after_explicit_markers_does_not_leak_or_override_visible_marker() {
    let input = br"{\rtf1{\fonttbl{\f0 Arial;}{\f1\fcharset2 Symbol;}}{\*\pnseclvl1\pnucrm\pnstart1{\pntxta .}}Title text\par{\pntext\pard\plain\f1 \'b7\tab}{\*\pn\pnlvlblt\pnf1{\pntxtb \'b7}}\pard\fi-360\li360\tx360 Bullet item\par{\pntext I.\tab}{\*\pn\pnucrm{\pntxta .}}\pard\fi-720\li720\tx720 Roman item\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Title text"));
    assert!(text.contains("Bullet item"));
    assert!(text.contains("Roman item"));
    assert!(!text.contains("\u{2022}"));
    assert!(!text.contains("I.\t"));
    assert!(!text.contains(".Title"));
    assert!(!text.contains("\u{2022}\u{2022}"));
    assert!(!text.contains("I.\t.Roman"));
    for forbidden in [
        "pntext", "pntxtb", "pntxta", "pnlvlblt", "pnseclvl", "pnucrm", "fonttbl",
    ] {
        assert!(
            !text.contains(forbidden),
            "ignored list metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "ignored list metadata should not be unsupported: {:?}",
        parsed.diagnostics
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
    assert!(
        rendered_text.contains("Bullet item"),
        "explicit Symbol bullet should remain visible in passive PDF text: {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("Title text"),
        "decoded PDF text did not contain title text: {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("Bullet item"),
        "decoded PDF text did not contain bullet item text: {rendered_text:?}"
    );
    assert!(!rendered_text.contains("I.Roman item"));
    assert!(
        !rendered_text.contains("I..Roman"),
        "ignored pn suffix metadata should not become visible punctuation: {rendered_text:?}"
    );
    assert!(
        !rendered_text.contains(".Title"),
        "ignored pn section-level metadata should not prefix body text: {rendered_text:?}"
    );

    for forbidden in [
        b"pntext".as_slice(),
        b"pntxtb",
        b"pntxta",
        b"pnlvlblt",
        b"pnseclvl",
        b"pnucrm",
        b"fonttbl",
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
            "ignored list metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn hyperlink_stored_results_render_as_inert_pdf_text_under_all_current_link_policies() {
    for policy in [
        PdfLinkPolicy::RenderVisibleTextOnly,
        PdfLinkPolicy::DisableAll,
        PdfLinkPolicy::AllowSanitizedHttpLinks,
    ] {
        let output = convert_rtf_to_pdf(
            &field_with_link_result(),
            &ConvertOptions {
                diagnostics: true,
                parse_options: RtfParseOptions {
                    pdf_link_policy: policy,
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

        assert!(rendered_text.contains("visible link"));
        for forbidden in [
            b"HYPERLINK".as_slice(),
            b"https://example.com",
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
                "hyperlink field leaked active PDF content under {policy:?}: {:?}",
                String::from_utf8_lossy(forbidden)
            );
        }
    }
}

#[test]
fn info_metadata_aliases_render_passively_without_instruction_leakage() {
    let input = br##"{\rtf1{\info{\creatim\yr2024\mo7\dy5\hr14\min30}{\revtim\yr2025\mo1\dy2}{\version9}{\edmins17}\nofparas3\noflines12\nofbytes4096}Alpha beta Gamma info created {\field{\*\fldinst INFO "Creation Date" \\@ "yyyy-MM-dd"}} saved {\field{\*\fldinst INFO "Last Save Time"}} revision {\field{\*\fldinst INFO "Revision Number" \\* ROMAN}} edit {\field{\*\fldinst INFO "Total Editing Time" \\# "0000"}} words {\field{\*\fldinst INFO "Number of Words"}} paragraphs {\field{\*\fldinst INFO "Number of Paragraphs"}} lines {\field{\*\fldinst INFO "Number of Lines" \\# "000"}} bytes {\field{\*\fldinst INFO "Number of Bytes" \\# "#,##0"}} file {\field{\*\fldinst INFO Filename}}\par}"##.to_vec();
    let output = convert_rtf_to_pdf(&input, &ConvertOptions::browser_safe_defaults()).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("created 2024-07-05"));
    assert!(rendered_text.contains("saved 2025-01-02 00:00:00"));
    assert!(rendered_text.contains("revision IX edit 0017"));
    assert!(rendered_text.contains("paragraphs 3 lines 012"));
    assert!(rendered_text.contains("bytes 4,096"));
    assert!(rendered_text.contains("words"));
    assert!(rendered_text.contains("[Field removed: no passive result]"));
    for forbidden in [
        "INFO",
        "Creation Date",
        "Last Save Time",
        "Revision Number",
        "Total Editing Time",
        "Number of Words",
        "Number of Paragraphs",
        "Number of Lines",
        "Number of Bytes",
        "Filename",
        "creatim",
        "revtim",
        "version",
        "edmins",
        "nofparas",
        "noflines",
        "nofbytes",
        "fldinst",
    ] {
        assert!(
            !rendered_text.contains(forbidden),
            "INFO metadata alias leaked instruction text: {forbidden}"
        );
    }
    for forbidden in [
        b"INFO".as_slice(),
        b"nofparas",
        b"noflines",
        b"nofbytes",
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
            "INFO metadata alias leaked active PDF content: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn linked_custom_property_values_are_never_rendered_or_preserved() {
    let input = br#"{\rtf1{\*\userprops{\propname Client Name}{\proptype30}{\staticval Contoso}{\linkval C:\\secret\\linked-source.docm /JavaScript /EmbeddedFile}}Client {\field{\*\fldinst DOCPROPERTY "Client Name"}}\par}"#.to_vec();
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

    assert!(rendered_text.contains("Client Contoso"));
    for forbidden in [
        b"linked-source.docm".as_slice(),
        b"secret",
        b"linkval",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "linked custom property value leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }

    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };
    for linked_property_input in [
        br"{\rtf1{\linkval C:\\secret\\linked-source.docm} visible\par}".as_slice(),
        br"{\rtf1{\*\userprops{\propname Client}{\linkval C:\\secret\\linked-source.docm}} visible\par}",
        br"{\rtf1{\*\unknown{\linkval C:\\secret\\linked-source.docm}} visible\par}",
    ] {
        assert!(matches!(
            parse_rtf_bytes_with_options(linked_property_input, &reject_options),
            Err(ParseError::ActiveContentRejected { feature, .. })
                if feature == "linked custom property value"
        ));
    }
}

#[test]
fn resultless_seq_switches_render_passive_word_visible_counters_without_instruction_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Figure {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SEQ Figure \\\\r 4}} then {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SEQ Figure}} repeat {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SEQ Figure \\\\c}} hidden {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SEQ Figure \\\\h}} next {",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst SEQ Figure}}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Figure 4 then 5 repeat 5 hidden  next 7"));
    assert!(!text.contains("\\r"));
    assert!(!text.contains("\\c"));
    assert!(!text.contains("\\h"));
    assert!(!text.contains("SEQ"));
    assert!(!text.contains("fldinst"));
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
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);
    assert!(
        rendered_text.contains("Figure 4 then 5 repeat 5 hidden  next 7"),
        "decoded PDF text did not contain passive SEQ switch results: {rendered_text:?}"
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
            "forbidden sequence-switch content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn resultless_styleref_number_context_switches_render_without_payload_leakage() {
    let input = br#"{\rtf1{\stylesheet{\s1 TitleStyle;}}{\*\listtable{\list{\listlevel\levelnfc0{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\s1\ls1\ilvl0 Numbered title\par\pard Full {\field{\*\fldinst STYLEREF TitleStyle \\w}} relative {\field{\*\fldinst STYLEREF TitleStyle \\r}} malformed {\field{\*\fldinst STYLEREF TitleStyle \\nfoo}}\par}"#.to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(
        text.contains(
            "0.\tNumbered titleFull 0 relative 0 malformed [Field removed: no passive result]"
        ),
        "text was {text:?}"
    );
    for forbidden in [
        "STYLEREF",
        "TitleStyle",
        "nfoo",
        "listtable",
        "leveltext",
        "levelnumbers",
        "fldinst",
    ] {
        assert!(
            !text.contains(forbidden),
            "STYLEREF number-context content leaked to text: {forbidden}"
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
        rendered_text.contains("Full 0 relative 0 malformed [Field removed: no passive result]"),
        "decoded PDF text did not contain passive STYLEREF number contexts: {rendered_text:?}"
    );
    for forbidden in [
        b"STYLEREF".as_slice(),
        b"TitleStyle",
        b"nfoo",
        b"listtable",
        b"leveltext",
        b"levelnumbers",
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
            "forbidden STYLEREF number-context content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn word_multilevel_styleref_switches_retain_authored_parent_context_passively() {
    let input = include_bytes!("../../fixtures/styleref-multilevel-passive.rtf");
    let parsed = parse_rtf_bytes(input).unwrap();
    let text = collect_text(&parsed.document);
    for expected in [
        "1.1.1.\tLeaf target",
        "Number 1.1.1 relative 1.1.1 full 1.1.1",
        "1.1.2.\tContext number 1.1.1 relative 1.1.1 full 1.1.1",
    ] {
        assert!(
            text.contains(expected),
            "Word-compatible multilevel STYLEREF text omitted {expected:?}: {text:?}"
        );
    }

    let strict = parse_rtf_bytes_with_options(
        input,
        &RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        },
    )
    .unwrap();
    let strict_text = collect_text(&strict.document);
    assert!(
        strict_text.contains("Number 1.1.1. relative 1.1.1 full 1.1.1."),
        "StrictSpec should retain authored list delimiters: {strict_text:?}"
    );

    let output = convert_rtf_to_pdf(
        input,
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
        rendered_text.contains("Number 1.1.1 relative 1.1.1 full 1.1.1"),
        "decoded PDF omitted passive multilevel STYLEREF context: {rendered_text:?}"
    );
    for forbidden in [
        b"STYLEREF".as_slice(),
        b"TargetStyle",
        b"ContextStyle",
        b"listtable",
        b"leveltext",
        b"levelnumbers",
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
            "forbidden multilevel STYLEREF content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn index_entry_and_subdocument_metadata_are_stripped_without_pdf_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 visible before {",
        "\\",
        "txe hidden index entry {",
        "\\",
        "object",
        "\\",
        "objdata 414243}} ",
        "\\",
        "subdocument12 visible after",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("visible before"));
    assert!(text.contains("visible after"));
    for forbidden in [
        "txe",
        "hidden index entry",
        "subdocument",
        "objdata",
        "414243",
    ] {
        assert!(
            !text.contains(forbidden),
            "index/subdocument metadata leaked to normalized text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unsupported RTF control")
            && !diagnostic.message.contains("unknown RTF destination")),
        "index/subdocument metadata should be known and stripped: {:?}",
        parsed.diagnostics
    );

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

    assert!(rendered_text.contains("visible before"));
    assert!(rendered_text.contains("visible after"));
    for forbidden in [
        b"txe".as_slice(),
        b"hidden index entry",
        b"subdocument",
        b"objdata",
        b"414243",
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
            "index/subdocument metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn formshade_is_omitted_from_word_compatible_print_output_without_pdf_form() {
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
        "ffdeftext Default value}{",
        "\\",
        "ffentrymcr launch.exe}{",
        "\\",
        "datafield 414243}}} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        other => panic!("expected paragraph, got {other:?}"),
    };
    assert!(text.contains("Before  After"));
    assert!(
        paragraph
            .runs
            .iter()
            .all(|run| !run.style.form_field_shading)
    );
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("form-field shading approximated")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("form-field shading is editor-only and omitted from passive print output")
    }));
    for forbidden in [
        "FORMTEXT",
        "HiddenName",
        "launch.exe",
        "414243",
        "formshade",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden formshade metadata leaked to text: {forbidden}"
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

    assert!(rendered_text.contains("Before  After"));
    assert!(
        content
            .operations
            .iter()
            .all(|operation| operation.operator != "re")
    );
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
    assert!(decoded_pdf_text(&strict_content).contains("Before Default value After"));
    assert!(
        strict_content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "StrictSpec should retain bounded passive form-field shading"
    );
    for forbidden in [
        b"FORMTEXT".as_slice(),
        b"HiddenName",
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
            "forbidden formshade content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn document_protection_metadata_does_not_reach_text_or_pdf() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "formprot",
        "\\",
        "revprot",
        "\\",
        "annotprot{",
        "\\",
        "passwordhash DEADBEEFCAFE0123456789ABCDEF0123}Visible protected body",
        "\\",
        "par",
        "\\",
        "passwordhash AABBCCDDEEFF Inline body",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes_strict(&input);
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible protected body"));
    assert!(text.contains("Inline body"));
    for forbidden in [
        "passwordhash",
        "DEADBEEF",
        "AABBCC",
        "formprot",
        "revprot",
        "annotprot",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden document protection metadata leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("document-protection.rtf");
    let output_path = dir.path().join("document-protection.pdf");
    fs::write(&input_path, input).unwrap();
    convert_rtf_file_to_pdf(
        &input_path,
        &output_path,
        &ConvertOptions {
            diagnostics: true,
            parse_options: RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    let pdf = fs::read(&output_path).unwrap();
    let parsed_pdf = PdfDocument::load_mem(&pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Visible protected body"));
    assert!(rendered_text.contains("Inline body"));
    for forbidden in [
        b"passwordhash".as_slice(),
        b"DEADBEEF",
        b"AABBCC",
        b"formprot",
        b"revprot",
        b"annotprot",
        b"/AcroForm",
        b"/Encrypt",
        b"/Perms",
        b"/OpenAction",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden document protection content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn document_protection_password_hash_obeys_reject_policy() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "passwordhash AABBCCDDEEFF",
        "\\",
        "par Visible protected body",
        "\\",
        "par}",
    ]);
    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };

    assert!(matches!(
        parse_rtf_bytes_with_options(&input, &reject_options),
        Err(ParseError::ActiveContentRejected { feature, .. })
            if feature == "document protection password hash"
    ));
}

#[test]
fn dynamic_date_time_controls_do_not_evaluate_or_leak_to_pdf() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Before ",
        "\\",
        "chdate ",
        "\\",
        "chtime ",
        "\\",
        "chdpa ",
        "\\",
        "chdpl After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before"));
    assert!(text.contains("[Dynamic date/time removed]"));
    assert!(text.contains("After"));
    assert!(!text.contains("chdate"));
    assert!(!text.contains("chtime"));
    assert!(!text.contains("chdpa"));
    assert!(!text.contains("chdpl"));

    let strip_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Strip,
        ..RtfParseOptions::default()
    };
    let stripped = parse_rtf_bytes_with_options(&input, &strip_options).unwrap();
    let stripped_text = collect_text(&stripped.document);
    assert!(stripped_text.contains("Before"));
    assert!(stripped_text.contains("After"));
    assert!(!stripped_text.contains("[Dynamic date/time removed]"));

    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };
    assert!(matches!(
        parse_rtf_bytes_with_options(&input, &reject_options),
        Err(ParseError::ActiveContentRejected { feature, .. })
            if feature == "dynamic date/time control"
    ));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("dynamic-date-time.rtf");
    let output_path = dir.path().join("dynamic-date-time.pdf");
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

    assert!(rendered_text.contains("[Dynamic date/time removed]"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "dynamic date/time control chdate rendered as passive placeholder without evaluating current time",
        )
    }));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("dynamic date/time control chdate placeholdered")
    }));
    for forbidden in [
        b"chdate".as_slice(),
        b"chtime",
        b"chdpa",
        b"chdpl",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden dynamic date/time content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn encapsulated_html_metadata_does_not_warn_or_reach_text_or_pdf() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "fromhtml1",
        "\\",
        "htmlrtf1",
        "\\",
        "b",
        "\\",
        "htmlrtf0 Visible body{",
        "\\",
        "*",
        "\\",
        "htmltag <p onclick=\"launch.exe\">Hidden HTML</p>}{",
        "\\",
        "htmltag <script>414243</script>}{",
        "\\",
        "htmlbase https://example.com/base/}{",
        "\\",
        "*",
        "\\",
        "htmltag {",
        "\\",
        "object",
        "\\",
        "objdata 414243}} after",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible body after"));
    for forbidden in [
        "Hidden HTML",
        "script",
        "onclick",
        "launch.exe",
        "htmltag",
        "htmlbase",
        "htmlrtf",
        "fromhtml",
        "https://example.com",
        "objdata",
        "414243",
    ] {
        assert!(
            !text.contains(forbidden),
            "encapsulated HTML metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "encapsulated HTML wrapper controls should be classified, got {:?}",
        parsed.diagnostics
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

    assert!(rendered_text.contains("Visible body after"));
    for forbidden in [
        b"Hidden HTML".as_slice(),
        b"onclick",
        b"launch.exe",
        b"htmltag",
        b"htmlbase",
        b"htmlrtf",
        b"fromhtml",
        b"https://example.com",
        b"objdata",
        b"414243",
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
            "encapsulated HTML metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn encapsulated_html_metadata_obeys_reject_policy() {
    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };

    for input in [
        br"{\rtf1{\*\htmltag <script>launch.exe</script>}Visible body\par}".as_slice(),
        br"{\rtf1{\htmlbase https://example.com/base/}Visible body\par}".as_slice(),
        br#"{\rtf1{\info{\htmltag <p onclick="launch.exe">Hidden</p>}}Visible body\par}"#
            .as_slice(),
        br"{\rtf1{\*\unknown{\htmltag <script>414243</script>}}Visible body\par}".as_slice(),
    ] {
        assert!(matches!(
            parse_rtf_bytes_with_options(input, &reject_options),
            Err(ParseError::ActiveContentRejected { feature, .. })
                if feature == "encapsulated HTML metadata"
        ));
    }
}

#[test]
fn private_metadata_destinations_do_not_reach_text_or_pdf() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Visible before {",
        "\\",
        "private Hidden private payload /JavaScript {",
        "\\",
        "object",
        "\\",
        "objdata ",
        payload_hex(),
        "}{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst HYPERLINK \"https://example.com/private\"}{",
        "\\",
        "fldrslt Hidden link}}} private word after",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before  private word after"));
    for forbidden in [
        "Hidden private payload",
        "JavaScript",
        "object",
        "objdata",
        payload_hex(),
        "HYPERLINK",
        "https://example.com/private",
        "Hidden link",
    ] {
        assert!(
            !text.contains(forbidden),
            "private metadata leaked to text: {forbidden}"
        );
    }
    for expected in [
        "active content removed: private metadata before safe model normalization",
        "active content removed: object payload in metadata before safe model normalization",
        "active content removed: field instruction in metadata before safe model normalization",
    ] {
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "missing private metadata diagnostic {expected:?}: {:?}",
            parsed.diagnostics
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "private metadata should not be unsupported: {:?}",
        parsed.diagnostics
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

    assert!(rendered_text.contains("Visible before  private word after"));
    for forbidden in [
        b"Hidden private payload".as_slice(),
        b"object",
        b"objdata",
        payload_hex().as_bytes(),
        b"HYPERLINK",
        b"https://example.com/private",
        b"Hidden link",
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
            "private metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn private_metadata_obeys_reject_policy() {
    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };

    for (input, expected_feature) in [
        (
            br"{\rtf1{\private Hidden private payload} Visible\par}".as_slice(),
            "private metadata",
        ),
        (
            br"{\rtf1{\info{\private Hidden private payload}} Visible\par}".as_slice(),
            "private metadata in metadata",
        ),
        (
            br"{\rtf1{\*\unknown{\private Hidden private payload}} Visible\par}".as_slice(),
            "private metadata in skipped destination",
        ),
    ] {
        assert!(
            matches!(
                parse_rtf_bytes_with_options(input, &reject_options),
                Err(ParseError::ActiveContentRejected { feature, .. })
                    if feature == expected_feature
            ),
            "expected {expected_feature:?} rejection"
        );
    }
}

#[test]
fn review_bookmark_and_annotation_payloads_do_not_reach_text_or_pdf() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 Visible before {",
        "\\",
        "annotation Hidden comment {",
        "\\",
        "object",
        "\\",
        "objdata ",
        payload_hex(),
        "}{",
        "\\",
        "result Hidden result}{",
        "\\",
        "field{",
        "\\",
        "*",
        "\\",
        "fldinst HYPERLINK \"https://example.com/comment\"}{",
        "\\",
        "fldrslt Hidden link}}}{",
        "\\",
        "bkmkstart SecretBookmark}{",
        "\\",
        "*",
        "\\",
        "bkmkend SecretBookmark}{",
        "\\",
        "deleted Deleted text} visible after",
        " {",
        "\\",
        "revised",
        "\\",
        "revauth1",
        "\\",
        "revdttm123456789",
        "\\",
        "insrsid42 Inserted text}",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible before visible after Inserted text"));
    for forbidden in [
        "Hidden comment",
        "Hidden result",
        "Hidden link",
        "SecretBookmark",
        "Deleted text",
        "revised",
        "revauth",
        "revdttm",
        "insrsid",
        "https://example.com/comment",
        payload_hex(),
        "[Embedded object removed]",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden review/bookmark content leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("review-bookmark.rtf");
    let output_path = dir.path().join("review-bookmark.pdf");
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
        b"Hidden comment".as_slice(),
        b"Hidden result",
        b"Hidden link",
        b"SecretBookmark",
        b"Deleted text",
        b"revised",
        b"revauth",
        b"revdttm",
        b"insrsid",
        b"https://example.com/comment",
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
            "forbidden review/bookmark content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn character_animation_controls_render_static_text_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "animtext5 Animated text",
        "\\",
        "animtext0 Static text",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Animated textStatic text"));
    assert!(!text.contains("animtext"));
    assert!(
        parsed.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("character animation stripped for passive static PDF output")),
        "missing character animation diagnostic: {:?}",
        parsed.diagnostics
    );
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "animtext should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::default()
        },
    )
    .unwrap();
    assert!(
        output.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("character animation stripped for passive static PDF output")),
        "conversion diagnostics should include character animation warning: {:?}",
        output.diagnostics
    );
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let page_id = *parsed_pdf.get_pages().values().next().expect("page");
    let content = parsed_pdf.get_and_decode_page_content(page_id).unwrap();
    let rendered_text = decoded_pdf_text(&content);

    assert!(rendered_text.contains("Animated textStatic text"));
    for forbidden in [
        b"animtext".as_slice(),
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
            "forbidden character animation content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn annotation_metadata_obeys_reject_policy() {
    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };

    assert!(matches!(
        parse_rtf_bytes_with_options(
            br"{\rtf1 Visible{\annotation Hidden comment {\object\objdata 414243}} body\par}",
            &reject_options,
        ),
        Err(ParseError::ActiveContentRejected { feature, .. }) if feature == "annotation metadata"
    ));
}

#[test]
fn associated_character_properties_render_passively_without_control_leakage() {
    let input = br"{\rtf1{\colortbl;\red255\green0\blue0;}\loch\ab\ai\acf1\aul\aexpnd4\aup6 Associated styled{\object\objdata 414243}\par Plain\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Associated styled"));
    assert!(text.contains("Plain"));
    let paragraph = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .expect("paragraph");
    let styled = paragraph
        .runs
        .iter()
        .find(|run| run.text.contains("Associated styled"))
        .expect("associated styled run");
    assert!(styled.style.bold);
    assert!(styled.style.italic);
    assert_eq!(styled.style.color_index, 1);
    assert_eq!(styled.style.underline, UnderlineStyle::Thick);
    assert_eq!(styled.style.character_spacing_twips, 0);
    assert_eq!(styled.style.baseline_shift_half_points, 0);
    let strict = parse_rtf_bytes_with_options(
        &input,
        &RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        },
    )
    .unwrap();
    let strict_style = run_style_for_text(&strict.document, "Associated styled")
        .expect("StrictSpec associated style");
    assert_eq!(strict_style.character_spacing_twips, 20);
    assert_eq!(strict_style.baseline_shift_half_points, 6);
    assert_eq!(strict_style.underline, UnderlineStyle::Single);
    for forbidden in ["acf1", "aul", "aexpnd4", "aup6", "objdata", "414243"] {
        assert!(
            !text.contains(forbidden),
            "forbidden associated character content leaked to text: {forbidden}"
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
    assert!(decoded_pdf_text(&content).contains("Associated styled"));
    for forbidden in [
        b"acf1".as_slice(),
        b"aul",
        b"aexpnd4",
        b"aup6",
        b"objdata",
        b"414243",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden associated character content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn ignorable_marker_before_associated_character_control_preserves_visible_text_without_leakage() {
    let input = br"{\rtf1{\colortbl;\red255\green0\blue0;}{\*\ab\ai\acf1\aul Visible alias text{\object\objdata 414243}}\par After\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Visible alias text"));
    assert!(text.contains("After"));
    for forbidden in ["ab", "ai", "acf1", "aul", "objdata", "414243"] {
        assert!(
            !text.contains(forbidden),
            "forbidden associated alias content leaked to text: {forbidden}"
        );
    }

    let paragraph = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .expect("paragraph");
    let visible = paragraph
        .runs
        .iter()
        .find(|run| run.text.contains("Visible alias text"))
        .expect("visible associated alias run");
    assert!(visible.style.bold);
    assert!(visible.style.italic);
    assert_eq!(visible.style.color_index, 1);
    assert_eq!(visible.style.underline, UnderlineStyle::Single);

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
        rendered_text.contains("Visible alias text") && rendered_text.contains("After"),
        "decoded PDF text missing visible associated alias text: {rendered_text:?}"
    );

    for forbidden in [
        b"acf1".as_slice(),
        b"aul",
        b"objdata",
        b"414243",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden associated alias content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn ignorable_marker_before_passive_character_controls_preserves_visible_text_without_leakage() {
    let input = br"{\rtf1{\colortbl;\red0\green0\blue0;\red255\green0\blue0;}{\*\uldb\ulc2\outl\shad\accdot\expndtw40\kerning2\charscalex125\fittext720 Visible styled text{\object\objdata 414243}}\par After\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Visible styled text"));
    assert!(text.contains("After"));
    for forbidden in [
        "uldb",
        "ulc2",
        "outl",
        "shad",
        "accdot",
        "expndtw40",
        "kerning2",
        "charscalex125",
        "fittext720",
        "objdata",
        "414243",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden passive character control leaked to text: {forbidden}"
        );
    }

    let paragraph = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .expect("paragraph");
    let visible = paragraph
        .runs
        .iter()
        .find(|run| run.text.contains("Visible styled text"))
        .expect("visible passive style run");
    assert_eq!(visible.style.underline, UnderlineStyle::Double);
    assert_eq!(visible.style.underline_color_index, Some(2));
    assert!(visible.style.outline);
    assert!(visible.style.shadow);
    assert_eq!(
        visible.style.emphasis_mark,
        open_rtf_converter::model::CharacterEmphasisMark::Dot
    );
    assert_eq!(visible.style.character_spacing_twips, 40);
    assert_eq!(visible.style.character_kerning_half_points, 2);
    assert_eq!(visible.style.character_scaling_percent, 125);
    assert_eq!(visible.style.fit_text_width_twips, Some(720));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("character fit-text rendered as bounded passive horizontal scaling")
    }));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "passive character controls should not be unsupported: {:?}",
        parsed.diagnostics
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
    assert!(
        ["Visible", "styled", "text", "After"]
            .into_iter()
            .all(|expected| rendered_text.contains(expected)),
        "decoded PDF text missing visible passive styled text: {rendered_text:?}"
    );

    for forbidden in [
        b"uldb".as_slice(),
        b"ulc2",
        b"outl",
        b"shad",
        b"accdot",
        b"expndtw40",
        b"kerning2",
        b"charscalex125",
        b"fittext720",
        b"objdata",
        b"414243",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden passive character control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn emphasis_mark_controls_render_passive_overlays_without_control_leakage() {
    let input = br"{\rtf1\accdot Dot text \acccomma Comma text \accnone Plain text{\object\objdata 414243}\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Dot text"));
    assert!(text.contains("Comma text"));
    assert!(text.contains("Plain text"));
    let paragraph = parsed
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .expect("paragraph");
    let style_for = |needle: &str| {
        paragraph
            .runs
            .iter()
            .find(|run| run.text.contains(needle))
            .unwrap_or_else(|| panic!("missing run containing {needle}"))
            .style
            .emphasis_mark
    };
    assert_eq!(style_for("Dot text"), CharacterEmphasisMark::Dot);
    assert_eq!(style_for("Comma text"), CharacterEmphasisMark::Comma);
    assert_eq!(style_for("Plain text"), CharacterEmphasisMark::None);
    for forbidden in ["accdot", "acccomma", "accnone", "objdata", "414243"] {
        assert!(
            !text.contains(forbidden),
            "forbidden emphasis mark content leaked to text: {forbidden}"
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
    assert!(rendered_text.contains("Dot text"));
    assert!(rendered_text.contains("Comma text"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "re"),
        "dot emphasis should render as passive filled vector marks"
    );
    assert!(
        content.operations.windows(3).any(|operations| {
            operations[0].operator == "m"
                && operations[1].operator == "l"
                && operations[2].operator == "S"
        }),
        "comma emphasis should render as passive stroked vector marks"
    );
    for forbidden in [
        b"accdot".as_slice(),
        b"acccomma",
        b"accnone",
        b"objdata",
        b"414243",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/URI",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden emphasis mark content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn stylesheet_inheritance_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "stylesheet{",
        "\\",
        "s2",
        "\\",
        "sbasedon1",
        "\\",
        "i Child;}{",
        "\\",
        "s1",
        "\\",
        "qc",
        "\\",
        "b Base;}}",
        "\\",
        "s2 Styled",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Styled"));
    assert!(!text.contains("Child"));
    assert!(!text.contains("sbasedon"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("stylesheet-inheritance.rtf");
    let output_path = dir.path().join("stylesheet-inheritance.pdf");
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
        b"stylesheet".as_slice(),
        b"sbasedon",
        b"Child",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn missing_word_normal_style_zero_renders_without_control_leakage() {
    let input = br"{\rtf1{\stylesheet{\s1\b Heading;}}\s1 Bold\par\s0 Normal\par}".to_vec();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Bold"));
    assert!(text.contains("Normal"));
    for forbidden in ["stylesheet", "Heading", "s0", "s1"] {
        assert!(
            !text.contains(forbidden),
            "forbidden style metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unknown RTF style index 0")),
        "missing Normal style 0 should not warn: {:?}",
        parsed.diagnostics
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

    assert!(
        rendered_text.contains("Bold"),
        "decoded PDF text missing styled text: {rendered_text:?}"
    );
    assert!(
        rendered_text.contains("Normal"),
        "decoded PDF text missing Normal style text: {rendered_text:?}"
    );
    for forbidden in [
        b"stylesheet".as_slice(),
        b"Heading",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden missing-style content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn stylesheet_next_style_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "stylesheet{",
        "\\",
        "s1",
        "\\",
        "snext2",
        "\\",
        "b Heading;}{",
        "\\",
        "s2",
        "\\",
        "qc",
        "\\",
        "i Body;}}",
        "\\",
        "s1 Heading text",
        "\\",
        "par Body text",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Heading text"));
    assert!(text.contains("Body text"));
    assert!(!text.contains("snext"));
    assert!(!text.contains("Body;"));

    let first = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    let second = match &parsed.document.blocks[1] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    assert!(!first.runs[0].style.bold);
    assert!(!first.runs[0].style.italic);
    assert_eq!(second.style.alignment, Alignment::Left);
    assert!(!second.runs[0].style.bold);
    assert!(!second.runs[0].style.italic);

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("stylesheet-next-style.rtf");
    let output_path = dir.path().join("stylesheet-next-style.pdf");
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
    assert!(rendered_text.contains("Heading text"));
    assert!(rendered_text.contains("Body text"));
    for forbidden in [
        b"stylesheet".as_slice(),
        b"snext",
        b"Heading;",
        b"Body;",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn stylesheet_character_style_preserves_direct_formatting_without_metadata_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Arial;}{",
        "\\",
        "f1 Courier New;}}{",
        "\\",
        "stylesheet{",
        "\\",
        "cs5",
        "\\",
        "b Emphasis;}}",
        "\\",
        "f1",
        "\\",
        "i Direct ",
        "\\",
        "cs5 Direct and styled",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Direct Direct and styled"));
    assert!(!text.contains("Emphasis"));
    assert!(!text.contains("cs5"));

    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected paragraph"),
    };
    assert!(paragraph.runs.iter().all(|run| run.style.font_index == 1));
    assert!(paragraph.runs.iter().all(|run| run.style.italic));
    assert!(paragraph.runs.iter().all(|run| !run.style.bold));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("stylesheet-character-style-additive.rtf");
    let output_path = dir.path().join("stylesheet-character-style-additive.pdf");
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
    assert!(rendered_text.contains("Direct Direct and styled"));
    for forbidden in [
        b"stylesheet".as_slice(),
        b"cs5",
        b"Emphasis",
        b"fonttbl",
        b"Courier New",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "{} leaked into PDF",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn word_stylesheet_metadata_controls_do_not_warn_or_leak_to_pdf() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0 Calibri;}}{",
        "\\",
        "stylesheet{",
        "\\",
        "s0",
        "\\",
        "ql",
        "\\",
        "widctlpar",
        "\\",
        "snext0",
        "\\",
        "ssemihidden",
        "\\",
        "sqformat",
        "\\",
        "spriority0 Normal;}{",
        "\\",
        "s1",
        "\\",
        "sbasedon0",
        "\\",
        "slink15",
        "\\",
        "slocked",
        "\\",
        "sautoupd",
        "\\",
        "styrsid123",
        "\\",
        "b",
        "\\",
        "qc Heading 1;}{",
        "\\",
        "cs15",
        "\\",
        "sadditive",
        "\\",
        "ssemihidden",
        "\\",
        "i Linked Char;}{",
        "\\",
        "*",
        "\\",
        "latentstyles",
        "\\",
        "lsdstimax376",
        "\\",
        "lsdlockeddef0",
        "\\",
        "lsdsemihiddendef1{",
        "\\",
        "lsdlockedexcept Normal;}}}",
        "\\",
        "pard",
        "\\",
        "s1 Heading",
        "\\",
        "par ",
        "\\",
        "cs15 linked text",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Heading"));
    assert!(text.contains("linked text"));
    assert!(!text.contains("Heading 1"));
    assert!(!text.contains("Linked Char"));
    for forbidden in [
        "ssemihidden",
        "sqformat",
        "spriority",
        "slink",
        "slocked",
        "sautoupd",
        "styrsid",
        "latentstyles",
        "lsdstimax",
    ] {
        assert!(
            !text.contains(forbidden),
            "stylesheet metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "Word stylesheet metadata should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );
    let heading = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected heading paragraph"),
    };
    assert_eq!(heading.style.alignment, Alignment::Left);
    assert!(!heading.runs[0].style.bold);
    let linked = match &parsed.document.blocks[1] {
        Block::Paragraph(paragraph) => paragraph,
        _ => panic!("expected linked character-style paragraph"),
    };
    assert!(!linked.runs[0].style.italic);

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
    assert!(rendered_text.contains("Heading"));
    assert!(rendered_text.contains("linked text"));
    for forbidden in [
        b"stylesheet".as_slice(),
        b"ssemihidden",
        b"sqformat",
        b"spriority",
        b"slink",
        b"slocked",
        b"sautoupd",
        b"styrsid",
        b"latentstyles",
        b"lsdstimax",
        b"Heading 1",
        b"Linked Char",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "stylesheet metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn latent_styles_metadata_does_not_warn_or_leak_payloads() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "stylesheet{",
        "\\",
        "s0 Normal;}}{",
        "\\",
        "latentstyles",
        "\\",
        "lsdstimax376",
        "\\",
        "lsdlockeddef0",
        "\\",
        "lsdpriority99",
        "\\",
        "lsdqformat1",
        "\\",
        "lsdunhideused1{",
        "\\",
        "lsdsemihiddendef1 Hidden latent style metadata}{",
        "\\",
        "object",
        "\\",
        "objdata 414243}}Visible",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Visible"));
    for forbidden in [
        "latentstyles",
        "lsdstimax",
        "lsdlockeddef",
        "lsdpriority",
        "lsdqformat",
        "lsdunhideused",
        "lsdsemihiddendef",
        "Hidden latent style metadata",
        "objdata",
        "414243",
    ] {
        assert!(
            !text.contains(forbidden),
            "latent styles metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| {
            !diagnostic.message.contains("unsupported RTF control")
                && !diagnostic.message.contains("unknown RTF destination")
        }),
        "latent styles metadata should be classified, got {:?}",
        parsed.diagnostics
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

    assert!(rendered_text.contains("Visible"));
    for forbidden in [
        b"latentstyles".as_slice(),
        b"lsdstimax",
        b"lsdlockeddef",
        b"lsdpriority",
        b"lsdqformat",
        b"lsdunhideused",
        b"lsdsemihiddendef",
        b"Hidden latent style metadata",
        b"objdata",
        b"414243",
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
            "latent styles metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn zero_width_formatting_controls_render_without_visible_glyph_or_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 A",
        "\\",
        "zwbo B",
        "\\",
        "zwnbo C",
        "\\",
        "zwnj D",
        "\\",
        "zwj E",
        "\\",
        "ltrmark L",
        "\\",
        "rtlmark R",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("A\u{200b}B\u{feff}C\u{200c}D\u{200d}E\u{200e}L\u{200f}R"));
    for forbidden in ["zwbo", "zwnbo", "zwnj", "zwj", "ltrmark", "rtlmark"] {
        assert!(
            !text.contains(forbidden),
            "forbidden zero-width control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("zero-width-formatting.rtf");
    let output_path = dir.path().join("zero-width-formatting.pdf");
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
        b"zwbo".as_slice(),
        b"zwnbo",
        b"zwnj",
        b"zwj",
        b"ltrmark",
        b"rtlmark",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden zero-width formatting content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn common_word_metadata_controls_do_not_warn_or_leak_to_pdf() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1{",
        "\\",
        "fonttbl{",
        "\\",
        "f0",
        "\\",
        "fswiss Calibri;}{",
        "\\",
        "f1",
        "\\",
        "froman Times New Roman;}}",
        "\\",
        "pard",
        "\\",
        "adeflang1025",
        "\\",
        "nobrkwrptbl",
        "\\",
        "sprstsp",
        "\\",
        "doctype0",
        "\\",
        "oldas",
        "\\",
        "donotembedsysfont",
        "\\",
        "donotembedlingdata",
        "\\",
        "trackmoves0",
        "\\",
        "trackformatting1",
        "\\",
        "validatexml0",
        "\\",
        "showxmlerrors0",
        "\\",
        "saveinvalidxml0",
        "\\",
        "usenormstyforlist",
        "\\",
        "lang1033",
        "\\",
        "loch",
        "\\",
        "af0",
        "\\",
        "hich",
        "\\",
        "af0",
        "\\",
        "dbch",
        "\\",
        "af1",
        "\\",
        "ltrch",
        "\\",
        "kerning2",
        "\\",
        "hyphpar0",
        "\\",
        "adjustright Word metadata text",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Word metadata text"));
    for forbidden in [
        "lang1033",
        "loch",
        "hich",
        "dbch",
        "af0",
        "af1",
        "ltrch",
        "kerning2",
        "hyphpar0",
        "adjustright",
        "adeflang",
        "nobrkwrptbl",
        "sprstsp",
        "doctype",
        "oldas",
        "donotembedsysfont",
        "donotembedlingdata",
        "trackmoves",
        "trackformatting",
        "validatexml",
        "showxmlerrors",
        "saveinvalidxml",
        "usenormstyforlist",
    ] {
        assert!(
            !text.contains(forbidden),
            "Word metadata control leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "common Word metadata should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("character kerning rendered as bounded passive pair spacing")
    }));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("character kerning approximated")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("paragraph hyphenation disabled")
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
    assert!(decoded_pdf_text(&content).contains("Word metadata text"));
    assert!(
        content
            .operations
            .iter()
            .any(|operation| operation.operator == "TJ" || operation.operator == "Tj"),
        "kerning should render as passive positioned text"
    );
    for forbidden in [
        b"lang1033".as_slice(),
        b"adjustright",
        b"adeflang",
        b"nobrkwrptbl",
        b"sprstsp",
        b"doctype",
        b"oldas",
        b"donotembedsysfont",
        b"donotembedlingdata",
        b"trackmoves",
        b"trackformatting",
        b"validatexml",
        b"showxmlerrors",
        b"saveinvalidxml",
        b"usenormstyforlist",
        b"kerning",
        b"hyphpar",
        b"loch",
        b"hich",
        b"dbch",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Word metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn word_session_and_layout_flags_are_classified_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "deff0",
        "\\",
        "deflang1033",
        "\\",
        "deflangfe1033",
        "\\",
        "nouicompat",
        "\\",
        "rsidroot123456{",
        "\\",
        "*",
        "\\",
        "rsidtbl ",
        "\\",
        "rsid123456{",
        "\\",
        "object",
        "\\",
        "objdata 414243}}",
        "\\",
        "rsid123456",
        "\\",
        "insrsid123456",
        "\\",
        "delrsid789012",
        "\\",
        "vertdoc",
        "\\",
        "wraptrsp",
        "\\",
        "sprsspbf",
        "\\",
        "sprsbsp Visible body",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Visible body"));
    for forbidden in [
        "rsid", "rsidtbl", "insrsid", "delrsid", "vertdoc", "wraptrsp", "sprsspbf", "sprsbsp",
        "objdata", "414243",
    ] {
        assert!(
            !text.contains(forbidden),
            "Word session/layout control leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unsupported RTF control")
            && !diagnostic.message.contains("unknown RTF destination")),
        "Word session/layout flags should not be unknown or unsupported: {:?}",
        parsed.diagnostics
    );
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("vertical document layout approximated")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "vertical document layout rendered through bounded passive horizontal fallback",
        )
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "Word typography compatibility option interpreted through bounded passive layout",
        )
    }));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("Word typography compatibility option approximated")
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
    assert!(decoded_pdf_text(&content).contains("Visible body"));
    for forbidden in [
        b"rsid".as_slice(),
        b"rsidtbl",
        b"insrsid",
        b"delrsid",
        b"vertdoc",
        b"wraptrsp",
        b"sprsspbf",
        b"sprsbsp",
        b"objdata",
        b"414243",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Word session/layout payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn custom_xml_markup_preserves_visible_text_without_metadata_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "deff0{",
        "\\",
        "*",
        "\\",
        "xmlnstbl {",
        "\\",
        "xmlns1 http://schemas.example/payload}}{",
        "\\",
        "*",
        "\\",
        "datastore 414243 {",
        "\\",
        "object",
        "\\",
        "objdata 414243}}{",
        "\\",
        "*",
        "\\",
        "themedata 504b0304}{",
        "\\",
        "*",
        "\\",
        "colorschememapping 414243}",
        "Before {",
        "\\",
        "xmlopen{",
        "\\",
        "xmlattrname secret}{",
        "\\",
        "xmlattrvalue 414243}tagged ",
        "\\",
        "xmlclose} After",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert_eq!(text.matches(PASSIVE_ADVANCE_MARKER).count(), 1);
    let marker_spacing = parsed.document.blocks.iter().find_map(|block| match block {
        Block::Paragraph(paragraph) => paragraph
            .runs
            .iter()
            .find(|run| run.text == PASSIVE_ADVANCE_MARKER)
            .map(|run| run.style.character_spacing_twips),
        _ => None,
    });
    assert_eq!(marker_spacing, Some(82));
    let calibri = parse_rtf_bytes(
        br"{\rtf1{\fonttbl{\f0 Calibri;}}\f0 Before {\xmlopen tagged \xmlclose} After}",
    )
    .unwrap();
    let calibri_marker_spacing = calibri
        .document
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Paragraph(paragraph) => paragraph
                .runs
                .iter()
                .find(|run| run.text == PASSIVE_ADVANCE_MARKER)
                .map(|run| run.style.character_spacing_twips),
            _ => None,
        });
    assert_eq!(calibri_marker_spacing, Some(144));
    let visible_text = text.replace(PASSIVE_ADVANCE_MARKER, "");
    assert!(
        visible_text.contains("Before tagged  After")
            || visible_text.contains("Before tagged After"),
        "custom XML visible text was not preserved: {visible_text:?}"
    );
    let strict_text = collect_text(&parse_rtf_bytes_strict(&input).document);
    assert!(!strict_text.contains(PASSIVE_ADVANCE_MARKER));
    for forbidden in [
        "xmlopen",
        "xmlclose",
        "xmlattrname",
        "xmlattrvalue",
        "xmlnstbl",
        "datastore",
        "themedata",
        "colorschememapping",
        "secret",
        "414243",
        "objdata",
    ] {
        assert!(
            !text.contains(forbidden),
            "custom XML metadata leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed.diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("unsupported RTF control")
            && !diagnostic.message.contains("unknown RTF destination")),
        "custom XML controls should not be unknown or unsupported: {:?}",
        parsed.diagnostics
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
    assert!(rendered_text.contains("Before tagged"));
    assert!(rendered_text.contains("After"));
    for forbidden in [
        b"xmlopen".as_slice(),
        b"xmlclose",
        b"xmlattrname",
        b"xmlattrvalue",
        b"xmlnstbl",
        b"datastore",
        b"themedata",
        b"colorschememapping",
        b"secret",
        b"414243",
        b"objdata",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "custom XML metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn opaque_metadata_payloads_obey_reject_policy() {
    let reject_options = RtfParseOptions {
        active_content_policy: ActiveContentPolicy::Reject,
        ..RtfParseOptions::default()
    };

    for (input, expected_feature) in [
        (
            br"{\rtf1{\*\datastore 414243}Visible body\par}".as_slice(),
            "custom XML data store",
        ),
        (
            br"{\rtf1{\*\themedata 504b0304}Visible body\par}".as_slice(),
            "Office theme data",
        ),
        (
            br"{\rtf1{\info{\hlinkbase https://example.com/base/}}Visible body\par}".as_slice(),
            "hyperlink base",
        ),
        (
            br"{\rtf1{\*\colorschememapping 414243}Visible body\par}".as_slice(),
            "Office color scheme mapping",
        ),
        (
            br"{\rtf1{\*\xmlnstbl {\xmlns1 http://schemas.example/payload}}Visible body\par}"
                .as_slice(),
            "custom XML namespace table",
        ),
        (
            br"{\rtf1{\xmlattrvalue 414243}Visible body\par}".as_slice(),
            "custom XML attribute metadata",
        ),
        (
            br"{\rtf1 Before {\xmlopen tagged\par} After\par}".as_slice(),
            "custom XML markup",
        ),
        (
            br"{\rtf1{\info{\title Hidden {\xmlopen tagged}}}Visible body\par}".as_slice(),
            "custom XML markup",
        ),
        (
            br"{\rtf1{\*\unknown{\xmlclose hidden}}Visible body\par}".as_slice(),
            "custom XML markup",
        ),
        (
            br"{\rtf1{\datafield 414243}Visible body\par}".as_slice(),
            "form field data payload",
        ),
    ] {
        assert!(matches!(
            parse_rtf_bytes_with_options(input, &reject_options),
            Err(ParseError::ActiveContentRejected { feature, .. })
                if feature == expected_feature
        ));
    }
}

#[test]
fn word_layout_compatibility_controls_are_classified_without_payload_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "ansi",
        "\\",
        "deff0",
        "\\",
        "themelang1033",
        "\\",
        "themelangfe2052",
        "\\",
        "themelangcs1025",
        "\\",
        "dghspace180",
        "\\",
        "dgvspace180",
        "\\",
        "dghorigin1701",
        "\\",
        "dgvorigin1984",
        "\\",
        "dghshow1",
        "\\",
        "dgvshow1",
        "\\",
        "viewbksp1",
        "\\",
        "viewnobound1",
        "\\",
        "formdisp",
        "\\",
        "rempersonalinfo",
        "\\",
        "jexpand",
        "\\",
        "jcompress",
        "\\",
        "jclisttab",
        "\\",
        "asianbrkrule",
        "\\",
        "dxfrtext180",
        "\\",
        "dfrmtxtx240",
        "\\",
        "dfrmtxty120",
        "\\",
        "absw1440",
        "\\",
        "absh720",
        "\\",
        "faauto",
        "\\",
        "twoinone",
        "\\",
        "horzvert",
        "\\",
        "aspalpha",
        "\\",
        "aspnum",
        "\\",
        "nogrowautofit",
        "\\",
        "lytexcttp",
        "\\",
        "lytprtmet",
        "\\",
        "noextrasprl",
        "\\",
        "notcvasp",
        "\\",
        "notvatxbx",
        "\\",
        "expshrtn",
        "\\",
        "useltbaln",
        "\\",
        "htmautsp",
        "\\",
        "nooverflow",
        "\\",
        "wpjst",
        "\\",
        "wpbrdr",
        "\\",
        "otblrul",
        " Visible compatibility text",
        "\\",
        "par}",
    ]);

    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Visible compatibility text"));
    assert!(parsed.document.blocks.iter().all(|block| {
        !matches!(block, Block::Paragraph(paragraph) if !paragraph.style.two_in_one)
    }));
    let layout = LayoutEngine::layout(&parsed.document);
    let fragments = layout.pages[0]
        .items
        .iter()
        .filter_map(|item| match item {
            LayoutItem::Text(fragment) => Some(fragment),
            _ => None,
        })
        .collect::<Vec<_>>();
    let first = fragments
        .iter()
        .find(|fragment| fragment.text == "Visible ")
        .expect("two-in-one first row");
    let second = fragments
        .iter()
        .find(|fragment| fragment.text.starts_with("compatibility"))
        .expect("two-in-one second row");
    assert!((first.style.font_size_points() - 6.0).abs() < 0.01);
    assert!((second.style.font_size_points() - 6.0).abs() < 0.01);
    assert!((second.baseline_y - first.baseline_y).abs() > 6.0);
    assert!((second.baseline_y - first.baseline_y).abs() < 8.0);
    for forbidden in [
        "themelang",
        "dghspace",
        "dgvspace",
        "dghorigin",
        "dgvorigin",
        "dghshow",
        "dgvshow",
        "viewbksp",
        "viewnobound",
        "formdisp",
        "rempersonalinfo",
        "jexpand",
        "jcompress",
        "jclisttab",
        "asianbrkrule",
        "dxfrtext",
        "dfrmtxtx",
        "dfrmtxty",
        "absw",
        "absh",
        "faauto",
        "twoinone",
        "horzvert",
        "aspalpha",
        "aspnum",
        "nogrowautofit",
        "lytexcttp",
        "lytprtmet",
        "noextrasprl",
        "notcvasp",
        "notvatxbx",
        "expshrtn",
        "useltbaln",
        "htmautsp",
        "nooverflow",
        "wpjst",
        "wpbrdr",
        "otblrul",
    ] {
        assert!(
            !text.contains(forbidden),
            "Word compatibility control leaked to text: {forbidden}"
        );
    }
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "Word compatibility controls should not be reported as unsupported: {:?}",
        parsed.diagnostics
    );
    for expected in [
        "Japanese text justification interpreted through bounded passive line layout",
        "Asian line-breaking rule interpreted through bounded passive Unicode line layout",
        "East Asian font alignment interpreted through bounded passive font metrics",
        "East Asian inline layout rendered through bounded passive horizontal fallback",
        "Word auto-spacing compatibility interpreted through bounded passive text layout",
        "paragraph wrap distance interpreted through bounded passive flow layout",
        "paragraph frame dimensions interpreted through bounded passive flow layout",
        "Word typography compatibility option interpreted through bounded passive layout",
    ] {
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "missing diagnostic: {expected}; diagnostics were {:?}",
            parsed.diagnostics
        );
    }
    for stale in [
        "Japanese text justification approximated",
        "Asian line-breaking rule approximated",
        "Word typography compatibility option approximated",
    ] {
        assert!(
            parsed
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains(stale)),
            "stale diagnostic present: {stale}; diagnostics were {:?}",
            parsed.diagnostics
        );
    }
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("table autofit growth compatibility approximated")
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("table autofit growth disabled through bounded passive authored widths")
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
    assert!(decoded_pdf_text(&content).contains("Visible compatibility text"));
    for forbidden in [
        b"themelang".as_slice(),
        b"dghspace",
        b"dgvspace",
        b"dghorigin",
        b"dgvorigin",
        b"dghshow",
        b"dgvshow",
        b"viewbksp",
        b"viewnobound",
        b"formdisp",
        b"rempersonalinfo",
        b"jexpand",
        b"jcompress",
        b"jclisttab",
        b"asianbrkrule",
        b"dxfrtext",
        b"dfrmtxtx",
        b"dfrmtxty",
        b"absw",
        b"absh",
        b"faauto",
        b"twoinone",
        b"horzvert",
        b"aspalpha",
        b"aspnum",
        b"nogrowautofit",
        b"lytexcttp",
        b"lytprtmet",
        b"noextrasprl",
        b"notcvasp",
        b"notvatxbx",
        b"expshrtn",
        b"useltbaln",
        b"htmautsp",
        b"nooverflow",
        b"wpjst",
        b"wpbrdr",
        b"otblrul",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "Word compatibility content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn document_hyphenation_default_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw3000",
        "\\",
        "margl720",
        "\\",
        "margr720",
        "\\",
        "hyphauto",
        "\\",
        "pard Antidisestablishmentarianism",
        "\\",
        "par",
        "\\",
        "pard Short",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let first = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        other => panic!("expected first paragraph, got {other:?}"),
    };
    let second = match &parsed.document.blocks[1] {
        Block::Paragraph(paragraph) => paragraph,
        other => panic!("expected second paragraph, got {other:?}"),
    };

    assert!(first.style.auto_hyphenation);
    assert!(second.style.auto_hyphenation);
    assert!(text.contains("AntidisestablishmentarianismShort"));
    assert!(!text.contains("hyphauto"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("document hyphenation rendered as bounded passive soft hyphenation")
    }));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("document hyphenation approximated")
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

    assert!(rendered_text.contains("Antidisestablishmentarianism"));
    assert!(rendered_text.contains("Short"));
    assert!(!rendered_text.contains('-'));
    for forbidden in [
        b"hyphauto".as_slice(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden document hyphenation content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn capital_word_hyphenation_control_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw3000",
        "\\",
        "margl720",
        "\\",
        "margr720",
        "\\",
        "hyphauto",
        "\\",
        "hyphcaps0",
        "\\",
        "pard ANTIDISESTABLISHMENTARIANISM",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        other => panic!("expected paragraph, got {other:?}"),
    };

    assert!(paragraph.style.auto_hyphenation);
    assert!(!paragraph.style.hyphenate_caps);
    assert!(text.contains("ANTIDISESTABLISHMENTARIANISM"));
    assert!(!text.contains("hyphcaps"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("document hyphenation rendered as bounded passive soft hyphenation")
    }));
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("document hyphenation approximated")
    }));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| { !diagnostic.message.contains("capitalized word hyphenation") })
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

    assert_eq!(
        rendered_text, "ANTIDISESTABLISHMENTARIANISM",
        "emergency wrapping should preserve all-caps text without synthetic punctuation: {rendered_text:?}"
    );
    for forbidden in [
        b"hyphcaps".as_slice(),
        b"hyphauto",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden capital-word hyphenation content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn hyphenation_zone_renders_passively_without_control_leakage() {
    let word = "Antidisestablishment";
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw3000",
        "\\",
        "margl720",
        "\\",
        "margr720",
        "\\",
        "hyphauto",
        "\\",
        "hyphhotz0",
        "\\",
        "pard ",
        word,
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let paragraph = match &parsed.document.blocks[0] {
        Block::Paragraph(paragraph) => paragraph,
        other => panic!("expected paragraph, got {other:?}"),
    };

    assert!(paragraph.style.auto_hyphenation);
    assert_eq!(paragraph.style.hyphenation_zone_twips, 0);
    assert!(text.contains(word));
    assert!(!text.contains("hyphhotz"));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("hyphenation zone applied to bounded passive hyphenation")
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

    assert_eq!(rendered_text, word);
    assert!(!rendered_text.contains('-'));
    for forbidden in [
        b"hyphhotz".as_slice(),
        b"hyphauto",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden hyphenation-zone content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn nonbreaking_hyphen_renders_passively_without_control_leakage() {
    let input = rtf(&["{", "\\", "rtf1 Before A", "\\", "_B after", "\\", "par}"]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before A\u{2011}B after"));
    assert!(!text.contains("\\_"));

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

    assert!(rendered_text.contains("Before A\u{2011}B after"));
    for forbidden in [
        b"\\_".as_slice(),
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
    ] {
        assert!(
            !searchable_pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden nonbreaking hyphen content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn flat_color_alpha_srccopy_and_notsrccopy_sources_keep_passive_masks_without_payload_leakage() {
    let mut emf_png = minimal_rgba_png(&[[12, 34, 56, 64], [12, 34, 56, 192]]);
    emf_png.extend_from_slice(b"TRAILING-EMF-ALPHA-SRCCOPY-PNG /JavaScript");
    let mut emf_dib = minimal_compressed_dib_with_payload(2, 1, 5, &emf_png);
    emf_dib.extend_from_slice(b"TRAILING-EMF-ALPHA-SRCCOPY-DIB /OpenAction");
    let mut wmf_png = minimal_rgba_png(&[[12, 34, 56, 64], [12, 34, 56, 192]]);
    wmf_png.extend_from_slice(b"TRAILING-WMF-ALPHA-NOTSRCCOPY-PNG /EmbeddedFile");
    let mut wmf_dib = minimal_compressed_dib_with_payload(2, 1, 5, &wmf_png);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-ALPHA-NOTSRCCOPY-DIB /RichMedia");
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
        &[
            wmf_set_window_ext_record(200, 100),
            wmf_stretchdib_dib_record(15, 25, 80, 40, 0x0033_0008, &wmf_dib),
        ],
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
        assert!(matches!(
            &image.vector_commands[0],
            StaticImageVectorCommand::RasterImage { image, .. }
                if image.format == ImageFormat::Png
                    && image.alpha_mask.is_some()
        ));
    }
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-ALPHA-SRCCOPY-PNG",
        "TRAILING-EMF-ALPHA-SRCCOPY-DIB",
        "TRAILING-WMF-ALPHA-NOTSRCCOPY-PNG",
        "TRAILING-WMF-ALPHA-NOTSRCCOPY-DIB",
        "JavaScript",
        "OpenAction",
        "EmbeddedFile",
        "RichMedia",
    ] {
        assert!(
            !text.contains(forbidden),
            "flat-color alpha SRCCOPY/NOTSRCCOPY payload/control leaked to text: {forbidden}"
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
        "flat-color alpha sources should render passive image XObjects"
    );
    assert!(
        output
            .pdf
            .windows(b"/SMask".len())
            .any(|window| window == b"/SMask"),
        "flat-color alpha sources should preserve passive PDF soft masks"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-ALPHA-SRCCOPY-PNG",
        b"TRAILING-EMF-ALPHA-SRCCOPY-DIB",
        b"TRAILING-WMF-ALPHA-NOTSRCCOPY-PNG",
        b"TRAILING-WMF-ALPHA-NOTSRCCOPY-DIB",
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
            "flat-color alpha SRCCOPY/NOTSRCCOPY payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn strict_spec_same_bounds_srcpaint_mergepaint_solid_sources_reduce_without_payload_leakage() {
    let mut emf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]);
    emf_dib.extend_from_slice(b"TRAILING-EMF-SAME-BOUNDS-SRCPAINT-SOLID-DIB /JavaScript");
    let mut wmf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[170, 170, 170], [170, 170, 170]]);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-SAME-BOUNDS-MERGEPAINT-SOLID-DIB /EmbeddedFile");
    let fill_color = Color {
        red: 100,
        green: 30,
        blue: 200,
    };
    let emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[
            emf_create_brush_record(3, 0, fill_color, 0),
            emf_select_object_record(3),
            emf_bitblt_record(20, 15, 60, 30, 0x00f0_0021, b"EMF-PATCOPY-FILL"),
            emf_stretchdibits_dib_record(20, 15, 60, 30, 0x00ee_0086, &emf_dib),
        ],
    );
    let wmf = minimal_wmf_with_records(
        200,
        100,
        &[
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(fill_color),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(15, 25, 80, 40, 0x00f0_0021, b"WMF-PATCOPY-FILL"),
            wmf_stretchdib_dib_record(15, 25, 80, 40, 0x00bb_0226, &wmf_dib),
        ],
    );
    let emf_hex = bytes_to_hex(&emf);
    let wmf_hex = bytes_to_hex(&wmf);
    let input = format!(
        "{{\\rtf1 before {{\\pict\\emfblip {emf_hex}}} middle {{\\pict\\wmetafile8\\picw200\\pich100\\picwgoal2160\\pichgoal720 {wmf_hex}}} after\\par}}"
    )
    .into_bytes();
    let parsed = parse_rtf_bytes_strict(&input);
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
    assert_eq!(images[0].vector_commands.len(), 1);
    assert_eq!(images[1].vector_commands.len(), 1);
    assert!(matches!(
        images[0].vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            fill_color: Some(Color {
                red: 238,
                green: 190,
                blue: 234
            }),
            ..
        }
    ));
    assert!(matches!(
        images[1].vector_commands[0],
        StaticImageVectorCommand::Rectangle {
            fill_color: Some(Color {
                red: 117,
                green: 95,
                blue: 221
            }),
            ..
        }
    ));
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-SAME-BOUNDS-SRCPAINT-SOLID-DIB",
        "TRAILING-WMF-SAME-BOUNDS-MERGEPAINT-SOLID-DIB",
        "EMF-PATCOPY-FILL",
        "WMF-PATCOPY-FILL",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "same-bounds source OR payload/control leaked to normalized text: {forbidden}"
        );
    }

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
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

    assert!(rendered_text.contains("before"));
    assert!(rendered_text.contains("middle"));
    assert!(rendered_text.contains("after"));
    assert!(
        content
            .operations
            .iter()
            .filter(|operation| operation.operator == "re")
            .count()
            >= 2,
        "same-bounds source OR reductions should render passive PDF rectangles"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-SAME-BOUNDS-SRCPAINT-SOLID-DIB",
        b"TRAILING-WMF-SAME-BOUNDS-MERGEPAINT-SOLID-DIB",
        b"EMF-PATCOPY-FILL",
        b"WMF-PATCOPY-FILL",
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
            "same-bounds source OR payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn source_backed_notpatcopy_renders_inverted_brush_without_payload_leakage() {
    let mut emf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[255, 63, 170], [15, 240, 85]]);
    emf_dib.extend_from_slice(b"TRAILING-EMF-NOTPATCOPY-DIB /JavaScript");
    let mut wmf_dib = minimal_24bit_dib_with_rgb_pixels(2, 1, &[[255, 63, 170], [15, 240, 85]]);
    wmf_dib.extend_from_slice(b"TRAILING-WMF-NOTPATCOPY-DIB /EmbeddedFile");
    let brush_color = Color {
        red: 20,
        green: 80,
        blue: 140,
    };
    let emf = minimal_emf_with_records(
        160,
        80,
        2540,
        1270,
        &[
            emf_create_brush_record(3, 0, brush_color, 0),
            emf_select_object_record(3),
            emf_bitblt_record(20, 15, 60, 30, 0x00f0_0021, b"EMF-PATCOPY-FILL"),
            emf_stretchdibits_dib_record(20, 15, 60, 30, 0x000f_0001, &emf_dib),
        ],
    );
    let wmf = minimal_wmf_with_records(
        200,
        100,
        &[
            wmf_set_window_ext_record(200, 100),
            wmf_create_brush_record(brush_color),
            wmf_select_object_record(0),
            wmf_dibbitblt_record(15, 25, 80, 40, 0x00f0_0021, b"WMF-PATCOPY-FILL"),
            wmf_stretchdib_dib_record(15, 25, 80, 40, 0x000f_0001, &wmf_dib),
        ],
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
    assert_eq!(images[0].vector_commands.len(), 2);
    assert_eq!(images[1].vector_commands.len(), 2);
    for image in &images {
        assert!(matches!(
            image.vector_commands[1],
            StaticImageVectorCommand::Rectangle {
                fill_color: Some(Color {
                    red: 235,
                    green: 175,
                    blue: 115
                }),
                ..
            }
        ));
    }
    assert_eq!(parsed.diagnostics.len(), 0);
    for forbidden in [
        "emfblip",
        "wmetafile",
        "TRAILING-EMF-NOTPATCOPY-DIB",
        "TRAILING-WMF-NOTPATCOPY-DIB",
        "EMF-PATCOPY-FILL",
        "WMF-PATCOPY-FILL",
        "JavaScript",
        "EmbeddedFile",
    ] {
        assert!(
            !text.contains(forbidden),
            "source-backed NOTPATCOPY payload/control leaked to normalized text: {forbidden}"
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
            .filter(|operation| operation.operator == "re")
            .count()
            >= 4,
        "source-backed NOTPATCOPY should render passive PDF rectangles"
    );
    for forbidden in [
        b"emfblip".as_slice(),
        b"wmetafile",
        emf_hex.as_bytes(),
        wmf_hex.as_bytes(),
        b"TRAILING-EMF-NOTPATCOPY-DIB",
        b"TRAILING-WMF-NOTPATCOPY-DIB",
        b"EMF-PATCOPY-FILL",
        b"WMF-PATCOPY-FILL",
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
            "source-backed NOTPATCOPY payload leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn landscape_zero_controls_keep_portrait_pdf_geometry_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "landscape0",
        "\\",
        "paperw12240",
        "\\",
        "paperh15840 Portrait body",
        "\\",
        "page portrait page two",
        "\\",
        "sect",
        "\\",
        "sectd",
        "\\",
        "lndscpsxn0",
        "\\",
        "pgwsxn12240",
        "\\",
        "pghsxn15840 Later section",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Portrait body"));
    assert!(text.contains("Later section"));
    assert!(!parsed.document.page.landscape);
    assert_eq!(parsed.document.page.width_twips, 12_240);
    assert_eq!(parsed.document.page.height_twips, 15_840);
    for forbidden in [
        "landscape",
        "lndscpsxn",
        "paperw",
        "paperh",
        "pgwsxn",
        "pghsxn",
    ] {
        assert!(
            !text.contains(forbidden),
            "orientation/page geometry control leaked to text: {forbidden}"
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
    assert_eq!(parsed_pdf.get_pages().len(), 3);
    for page_id in parsed_pdf.get_pages().values() {
        let (width, height) = pdf_page_media_box_size(&parsed_pdf, *page_id);
        assert_eq!((width.round() as i32, height.round() as i32), (612, 792));
    }
    for forbidden in [
        b"landscape".as_slice(),
        b"lndscpsxn",
        b"paperw",
        b"paperh",
        b"pgwsxn",
        b"pghsxn",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
        b"/AcroForm",
    ] {
        assert!(
            !output
                .pdf
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "orientation/page geometry control leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
    audit_passive_pdf_bytes(&output.pdf).unwrap();
}

#[test]
fn distributed_alignment_controls_render_as_passive_spaced_text() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "paperw5000",
        "\\",
        "margl720",
        "\\",
        "margr720",
        "\\",
        "qd distributed one two three four five six seven eight",
        "\\",
        "par",
        "\\",
        "qk thai distributed one two three four five six seven eight",
        "\\",
        "par",
        "\\",
        "qt thai justified one two three four five six seven eight",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("distributed one"));
    assert!(text.contains("thai distributed"));
    assert!(text.contains("thai justified"));
    for forbidden in ["qd", "qk", "qt"] {
        assert!(
            !text.contains(forbidden),
            "forbidden distributed-alignment control leaked to text: {forbidden}"
        );
    }
    let paragraphs = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            open_rtf_converter::model::Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(paragraphs[0].style.alignment, Alignment::Distributed);
    assert_eq!(paragraphs[1].style.alignment, Alignment::Justified);
    assert_eq!(paragraphs[2].style.alignment, Alignment::Justified);
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "distributed alignment controls should not be unsupported: {:?}",
        parsed.diagnostics
    );
    let dir = tempdir().unwrap();
    let input_path = dir.path().join("distributed-alignment.rtf");
    let output_path = dir.path().join("distributed-alignment.pdf");
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
        b"\\qd".as_slice(),
        b"\\qk",
        b"\\qt",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden distributed-alignment content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn widow_control_renders_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "widowctrl",
        "\\",
        "pard Document default",
        "\\",
        "line Carries widow control",
        "\\",
        "par",
        "\\",
        "widctlpar Alpha",
        "\\",
        "line Beta",
        "\\",
        "line Gamma",
        "\\",
        "par",
        "\\",
        "nowidctlpar Plain",
        "\\",
        "par",
        "\\",
        "widctlpar Restored",
        "\\",
        "par",
        "\\",
        "nowidowctrl Alias plain",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Document default\nCarries widow control"));
    assert!(text.contains("Alpha\nBeta\nGamma"));
    assert!(text.contains("Plain"));
    assert!(text.contains("Restored"));
    assert!(text.contains("Alias plain"));
    assert!(!text.contains("widowctrl"));
    assert!(!text.contains("widctlpar"));
    assert!(!text.contains("nowidctlpar"));
    assert!(!text.contains("nowidowctrl"));
    let open_rtf_converter::model::Block::Paragraph(first) = &parsed.document.blocks[0] else {
        panic!("expected first paragraph");
    };
    let open_rtf_converter::model::Block::Paragraph(third) = &parsed.document.blocks[2] else {
        panic!("expected third paragraph");
    };
    let open_rtf_converter::model::Block::Paragraph(fourth) = &parsed.document.blocks[3] else {
        panic!("expected fourth paragraph");
    };
    let open_rtf_converter::model::Block::Paragraph(fifth) = &parsed.document.blocks[4] else {
        panic!("expected fifth paragraph");
    };
    assert!(first.style.widow_control);
    assert!(!third.style.widow_control);
    assert!(fourth.style.widow_control);
    assert!(!fifth.style.widow_control);
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
        "widow-control aliases should not be unsupported: {:?}",
        parsed.diagnostics
    );

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("widow-control.rtf");
    let output_path = dir.path().join("widow-control.pdf");
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
        b"widowctrl".as_slice(),
        b"widctlpar",
        b"nowidctlpar",
        b"nowidowctrl",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden widow-control content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn relief_text_stays_passive_pdf_text_state() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 plain ",
        "\\",
        "embo emboss ",
        "\\",
        "impr engrave ",
        "\\",
        "impr0 normal",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("plain"));
    assert!(text.contains("emboss"));
    assert!(text.contains("engrave"));
    assert!(text.contains("normal"));
    let Block::Paragraph(paragraph) = &parsed.document.blocks[0] else {
        panic!("expected relief paragraph");
    };
    let relief_for = |text: &str| {
        paragraph
            .runs
            .iter()
            .find(|run| run.text.contains(text))
            .map(|run| run.style.relief)
            .expect("expected relief run")
    };
    assert_eq!(relief_for("emboss"), TextRelief::Emboss);
    assert_eq!(relief_for("engrave"), TextRelief::Emboss);
    assert_eq!(relief_for("normal"), TextRelief::Emboss);

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("relief.rtf");
    let output_path = dir.path().join("relief.pdf");
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
    for forbidden in [b"/JavaScript".as_slice(), b"/EmbeddedFile", b"/Launch"] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden)
        );
    }
}

#[test]
fn small_caps_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 ",
        "\\",
        "scaps Mix",
        "\\",
        "scaps0 normal",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    assert!(text.contains("Mix"));
    assert!(text.contains("normal"));
    assert!(!text.contains("scaps"));

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("small-caps.rtf");
    let output_path = dir.path().join("small-caps.pdf");
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

    assert!(decoded_pdf_text(&content).contains("MIX"));
    assert!(content.operations.iter().any(|operation| {
        operation.operator == "Tf" && format!("{:?}", operation.operands).contains("8.5")
    }));
    for forbidden in [
        b"scaps".as_slice(),
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
fn drop_cap_controls_render_passively_without_control_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1",
        "\\",
        "dropcapli3",
        "\\",
        "dropcapt1 Dropped paragraph",
        "\\",
        "par",
        "\\",
        "pard Plain paragraph",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let paragraphs = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(text.contains("Dropped paragraph"));
    assert!(text.contains("Plain paragraph"));
    assert_eq!(paragraphs[0].style.drop_cap_lines, 0);
    assert_eq!(paragraphs[1].style.drop_cap_lines, 0);
    for forbidden in ["dropcapli", "dropcapt"] {
        assert!(
            !text.contains(forbidden),
            "forbidden drop-cap control leaked to text: {forbidden}"
        );
    }

    let dir = tempdir().unwrap();
    let input_path = dir.path().join("drop-cap.rtf");
    let output_path = dir.path().join("drop-cap.pdf");
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

    assert!(rendered_text.contains("Dropped paragraph"));
    assert!(rendered_text.contains("Plain paragraph"));
    for forbidden in [
        b"dropcapli".as_slice(),
        b"dropcapt",
        b"/JavaScript",
        b"/EmbeddedFile",
        b"/Launch",
        b"/OpenAction",
    ] {
        assert!(
            !pdf.windows(forbidden.len())
                .any(|window| window == forbidden),
            "forbidden drop-cap content leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn unknown_non_ignorable_destinations_are_skipped_without_pdf_leakage() {
    let input = rtf(&[
        "{",
        "\\",
        "rtf1 visible {",
        "\\",
        "unknown Hidden text {",
        "\\",
        "object",
        "\\",
        "objdata ",
        payload_hex(),
        " ",
        "\\",
        "bin5 ABCDE",
        " /JavaScript /EmbeddedFile}} after",
        "\\",
        "par}",
    ]);
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("visible  after"));
    for forbidden in [
        "Hidden text",
        payload_hex(),
        "unknown",
        "objdata",
        "ABCDE",
        "JavaScript",
        "EmbeddedFile",
        "[Embedded object removed]",
    ] {
        assert!(
            !text.contains(forbidden),
            "unknown destination leaked to text: {forbidden}"
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

    assert!(rendered_text.contains("visible  after"));
    for forbidden in [
        b"Hidden text".as_slice(),
        payload_hex().as_bytes(),
        b"unknown",
        b"objdata",
        b"ABCDE",
        b"Embedded object removed",
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
            "unknown destination leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn ignorable_marker_before_formatting_control_does_not_hide_visible_text() {
    let input = br"{\rtf1 Before {\*\b Bold {\*\i italic}} After {\*\unknown Hidden {\object\objdata 414243}} visible\par}";
    let parsed = parse_rtf_bytes(input).unwrap();
    let text = collect_text(&parsed.document);

    assert!(text.contains("Before"));
    assert!(text.contains("Bold"));
    assert!(text.contains("italic"));
    assert!(text.contains("After"));
    assert!(text.contains("visible"));
    assert!(!text.contains("Hidden"));
    assert!(!text.contains("414243"));
    assert!(
        parsed.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "ignorable destination marker before a non-destination control was ignored",
            )
        }),
        "missing non-destination ignorable marker diagnostic: {:?}",
        parsed.diagnostics
    );

    let output = convert_rtf_to_pdf(
        input,
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
    assert!(rendered_text.contains("Bold"));
    assert!(rendered_text.contains("italic"));
    assert!(rendered_text.contains("After"));
    assert!(rendered_text.contains("visible"));
    for forbidden in [
        b"Hidden".as_slice(),
        b"414243",
        b"objdata",
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
            "non-destination ignorable marker leaked hidden payload to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_arc_double_brace_and_double_bracket_render_passively_without_payload_leakage() {
    let mut input = String::from("{\\rtf1 Before\\par");
    for (shape_type, payload) in [
        (25, "hidden-arc-payload"),
        (26, "hidden-double-bracket-payload"),
        (27, "hidden-double-brace-payload"),
    ] {
        input.push_str(&format!(
            "{{\\shp{{\\*\\shpinst\\shpleft720\\shptop720\\shpright3600\\shpbottom1800{{\\sp{{\\sn shapeType}}{{\\sv {shape_type}}}}}{{\\sp{{\\sn fillColor}}{{\\sv 65535}}}}{{\\sp{{\\sn lineColor}}{{\\sv 16711680}}}}{{\\sp{{\\sn pFragments}}{{\\sv {payload}}}}}}}}}"
        ));
    }
    input.push_str("After\\par}");
    let input = input.into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let shapes = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shapes.len(), 3);
    assert_eq!(shapes[0].kind, StaticShapeKind::Polyline);
    assert_eq!(shapes[0].points.len(), 49);
    assert_eq!(shapes[1].kind, StaticShapeKind::Polygon);
    assert_eq!(shapes[1].points.len(), 12);
    assert_eq!(shapes[2].kind, StaticShapeKind::Polygon);
    assert_eq!(shapes[2].points.len(), 52);
    for shape in &shapes {
        assert!(
            shape.points.iter().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "arc/double-brace/double-bracket points must stay inside the passive shape frame"
        );
    }
    assert_eq!(shapes[0].points[0].x_twips, 0);
    assert_eq!(shapes[0].points[0].y_twips, shapes[0].height_twips);
    assert_eq!(
        shapes[0].points.last().unwrap().x_twips,
        shapes[0].width_twips
    );
    assert_eq!(
        shapes[0].points.last().unwrap().y_twips,
        shapes[0].height_twips
    );
    assert!(
        shapes[0]
            .points
            .iter()
            .filter(|point| point.y_twips <= shapes[0].height_twips / 8)
            .count()
            >= 10,
        "arc should preserve dense bounded passive samples near the top of the curve"
    );
    assert!(
        shapes[2]
            .points
            .iter()
            .filter(|point| {
                point.x_twips <= shapes[2].width_twips / 8
                    && point.y_twips >= (shapes[2].height_twips * 2) / 5
                    && point.y_twips <= (shapes[2].height_twips * 3) / 5
            })
            .count()
            >= 3,
        "double brace should preserve the bounded passive left waist curve"
    );
    assert!(
        shapes[2]
            .points
            .iter()
            .filter(|point| {
                point.x_twips >= shapes[2].width_twips - (shapes[2].width_twips / 8)
                    && point.y_twips >= (shapes[2].height_twips * 2) / 5
                    && point.y_twips <= (shapes[2].height_twips * 3) / 5
            })
            .count()
            >= 3,
        "double brace should preserve the bounded passive right waist curve"
    );
    assert!(
        shapes[2]
            .points
            .iter()
            .filter(|point| {
                point.y_twips <= shapes[2].height_twips / 5
                    || point.y_twips >= shapes[2].height_twips - (shapes[2].height_twips / 5)
            })
            .count()
            >= 16,
        "double brace should preserve bounded passive upper and lower shoulder curves"
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "hidden-arc-payload",
        "hidden-double-bracket-payload",
        "hidden-double-brace-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "arc/double shape metadata leaked to normalized text: {forbidden}"
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
    let mut rendered_text = String::new();
    let mut saw_polyline_stroke = false;
    let mut passive_shape_paints = 0;
    for page_id in parsed_pdf.get_pages().values() {
        let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
        rendered_text.push_str(&decoded_pdf_text(&content));
        saw_polyline_stroke |= content
            .operations
            .iter()
            .any(|operation| operation.operator == "S");
        passive_shape_paints += content
            .operations
            .iter()
            .filter(|operation| operation.operator == "B")
            .count();
    }

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        saw_polyline_stroke,
        "arc should render as a passive stroked polyline"
    );
    assert!(
        passive_shape_paints >= 2,
        "double brace/bracket shapes should render passive fill/stroke paths"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"hidden-arc-payload",
        b"hidden-double-bracket-payload",
        b"hidden-double-brace-payload",
        b"[Shape skipped",
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
            "arc/double shape metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_flowchart_primitives_render_passively_without_payload_leakage() {
    let mut input = String::from("{\\rtf1 Before\\par");
    for (shape_type, payload) in [
        (61, "flowchart-process-payload"),
        (62, "flowchart-alternate-process-payload"),
        (63, "flowchart-decision-payload"),
        (64, "flowchart-data-payload"),
        (69, "flowchart-terminator-payload"),
        (70, "flowchart-preparation-payload"),
        (71, "flowchart-manual-input-payload"),
        (72, "flowchart-manual-operation-payload"),
        (73, "flowchart-connector-payload"),
        (74, "flowchart-offpage-connector-payload"),
        (81, "flowchart-extract-payload"),
        (82, "flowchart-merge-payload"),
    ] {
        input.push_str(&format!(
            "{{\\shp{{\\*\\shpinst\\shpleft720\\shptop720\\shpright2880\\shpbottom1080{{\\sp{{\\sn shapeType}}{{\\sv {shape_type}}}}}{{\\sp{{\\sn fillColor}}{{\\sv 65280}}}}{{\\sp{{\\sn lineColor}}{{\\sv 16711680}}}}{{\\sp{{\\sn pFragments}}{{\\sv {payload}}}}}}}}}"
        ));
    }
    input.push_str("After\\par}");
    let input = input.into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let shapes = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shapes.len(), 12);
    assert_eq!(shapes[0].kind, StaticShapeKind::Rectangle);
    assert_eq!(shapes[1].kind, StaticShapeKind::RoundedRectangle);
    assert_eq!(shapes[2].kind, StaticShapeKind::Polygon);
    assert_eq!(shapes[2].points.len(), 4);
    assert_eq!(shapes[3].kind, StaticShapeKind::Polygon);
    assert_eq!(shapes[3].points.len(), 4);
    assert_eq!(shapes[4].kind, StaticShapeKind::RoundedRectangle);
    assert_eq!(shapes[5].kind, StaticShapeKind::Polygon);
    assert_eq!(shapes[5].points.len(), 6);
    assert_eq!(shapes[6].kind, StaticShapeKind::Polygon);
    assert_eq!(shapes[6].points.len(), 4);
    assert_eq!(shapes[7].kind, StaticShapeKind::Polygon);
    assert_eq!(shapes[7].points.len(), 4);
    assert_eq!(shapes[8].kind, StaticShapeKind::Ellipse);
    assert_eq!(shapes[9].kind, StaticShapeKind::Polygon);
    assert_eq!(shapes[9].points.len(), 5);
    assert_eq!(shapes[10].kind, StaticShapeKind::Polygon);
    assert_eq!(shapes[10].points.len(), 3);
    assert_eq!(shapes[11].kind, StaticShapeKind::Polygon);
    assert_eq!(shapes[11].points.len(), 3);
    assert_eq!(shapes[10].points[0].x_twips, shapes[10].width_twips / 2);
    assert_eq!(shapes[10].points[0].y_twips, 0);
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "flowchart-process-payload",
        "flowchart-alternate-process-payload",
        "flowchart-decision-payload",
        "flowchart-data-payload",
        "flowchart-terminator-payload",
        "flowchart-preparation-payload",
        "flowchart-manual-input-payload",
        "flowchart-manual-operation-payload",
        "flowchart-connector-payload",
        "flowchart-offpage-connector-payload",
        "flowchart-extract-payload",
        "flowchart-merge-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "flowchart metadata leaked to normalized text: {forbidden}"
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
    let mut rendered_text = String::new();
    let mut saw_passive_shape_paint = false;
    for page_id in parsed_pdf.get_pages().values() {
        let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
        rendered_text.push_str(&decoded_pdf_text(&content));
        saw_passive_shape_paint |= content
            .operations
            .iter()
            .any(|operation| operation.operator == "B");
    }

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        saw_passive_shape_paint,
        "flowchart primitives should render passive fill/stroke paths"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"flowchart-process-payload",
        b"flowchart-alternate-process-payload",
        b"flowchart-decision-payload",
        b"flowchart-data-payload",
        b"flowchart-terminator-payload",
        b"flowchart-preparation-payload",
        b"flowchart-manual-input-payload",
        b"flowchart-manual-operation-payload",
        b"flowchart-connector-payload",
        b"flowchart-offpage-connector-payload",
        b"flowchart-extract-payload",
        b"flowchart-merge-payload",
        b"[Shape skipped",
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
            "flowchart metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}

#[test]
fn office_flowchart_document_variants_render_passively_without_payload_leakage() {
    let mut input = String::from("{\\rtf1 Before\\par");
    for (shape_type, payload) in [
        (65, "flowchart-predefined-process-payload"),
        (66, "flowchart-internal-storage-payload"),
        (67, "flowchart-document-payload"),
        (68, "flowchart-multidocument-payload"),
    ] {
        input.push_str(&format!(
            "{{\\shp{{\\*\\shpinst\\shpleft720\\shptop720\\shpright2880\\shpbottom1440{{\\sp{{\\sn shapeType}}{{\\sv {shape_type}}}}}{{\\sp{{\\sn fillColor}}{{\\sv 65280}}}}{{\\sp{{\\sn lineColor}}{{\\sv 16711680}}}}{{\\sp{{\\sn pFragments}}{{\\sv {payload}}}}}}}}}"
        ));
    }
    input.push_str("After\\par}");
    let input = input.into_bytes();
    let parsed = parse_rtf_bytes(&input).unwrap();
    let text = collect_text(&parsed.document);
    let shapes = parsed
        .document
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Shape(shape) => Some(shape),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert_eq!(shapes.len(), 4);
    for shape in &shapes {
        assert_eq!(shape.kind, StaticShapeKind::Polygon);
        assert!(
            shape.points.iter().all(|point| {
                point.x_twips >= 0
                    && point.x_twips <= shape.width_twips
                    && point.y_twips >= 0
                    && point.y_twips <= shape.height_twips
            }),
            "flowchart document shape points must stay inside passive frame: {shape:?}"
        );
    }
    assert_eq!(shapes[0].points.len(), 4);
    assert_eq!(shapes[1].points.len(), 4);
    assert_eq!(shapes[2].points.len(), 27);
    assert_eq!(shapes[3].points.len(), 35);
    assert_eq!(
        shapes[0].overlay_paths.len(),
        2,
        "predefined-process flowchart should preserve Word-visible internal vertical lines"
    );
    assert_eq!(
        shapes[1].overlay_paths.len(),
        2,
        "internal-storage flowchart should preserve Word-visible internal guide lines"
    );
    assert_eq!(
        shapes[3].overlay_paths.len(),
        2,
        "multidocument flowchart should preserve Word-visible stacked-page edges"
    );
    assert!(shapes[0].overlay_paths.iter().all(|path| path.len() == 2));
    assert!(shapes[1].overlay_paths.iter().all(|path| path.len() == 2));
    assert!(shapes[3].overlay_paths.iter().all(|path| path.len() == 2));
    assert!(
        shapes[3].overlay_paths.iter().flatten().all(|point| {
            point.x_twips >= 0
                && point.x_twips <= shapes[3].width_twips
                && point.y_twips >= 0
                && point.y_twips <= shapes[3].height_twips
        }),
        "flowchart multidocument overlay edges must stay inside passive frame"
    );
    assert_eq!(
        shapes[2].points[2].y_twips,
        (shapes[2].height_twips * 4) / 5
    );
    assert!(
        shapes[2]
            .points
            .iter()
            .filter(|point| point.y_twips > (shapes[2].height_twips * 4) / 5)
            .count()
            >= 12,
        "flowchart document should preserve a bounded passive wavy bottom edge"
    );
    assert!(
        shapes[3]
            .points
            .iter()
            .filter(|point| point.y_twips > (shapes[3].height_twips * 4) / 5)
            .count()
            >= 12,
        "flowchart multidocument should preserve bounded passive wavy page edges"
    );
    assert!(
        shapes[3].points.iter().any(|point| {
            point.x_twips == shapes[3].width_twips / 6
                && point.y_twips == shapes[3].height_twips / 5
        }),
        "flowchart multidocument should preserve the stacked-page offset corner"
    );
    for forbidden in [
        "shapeType",
        "fillColor",
        "lineColor",
        "pFragments",
        "flowchart-predefined-process-payload",
        "flowchart-internal-storage-payload",
        "flowchart-document-payload",
        "flowchart-multidocument-payload",
        "[Shape skipped",
    ] {
        assert!(
            !text.contains(forbidden),
            "flowchart document metadata leaked to normalized text: {forbidden}"
        );
    }

    let layout = LayoutEngine::layout(&parsed.document);
    let overlay_path_count = layout
        .pages
        .iter()
        .flat_map(|page| page.items.iter())
        .filter_map(|item| match item {
            LayoutItem::Polygon { overlay_paths, .. } => Some(overlay_paths.len()),
            _ => None,
        })
        .sum::<usize>();
    assert!(
        overlay_path_count >= 6,
        "flowchart internal guide lines and stacked-page edges should survive into passive layout"
    );

    let output = convert_rtf_to_pdf(
        &input,
        &ConvertOptions {
            diagnostics: true,
            ..ConvertOptions::browser_safe_defaults()
        },
    )
    .unwrap();
    let parsed_pdf = PdfDocument::load_mem(&output.pdf).unwrap();
    let mut rendered_text = String::new();
    let mut passive_shape_paints = 0usize;
    let mut passive_overlay_strokes = 0usize;
    for page_id in parsed_pdf.get_pages().values() {
        let content = parsed_pdf.get_and_decode_page_content(*page_id).unwrap();
        rendered_text.push_str(&decoded_pdf_text(&content));
        passive_shape_paints += content
            .operations
            .iter()
            .filter(|operation| operation.operator == "B")
            .count();
        passive_overlay_strokes += content
            .operations
            .iter()
            .filter(|operation| operation.operator == "S")
            .count();
    }

    assert!(rendered_text.contains("Before"));
    assert!(rendered_text.contains("After"));
    assert!(
        passive_shape_paints >= 4,
        "flowchart document variants should render passive fill/stroke paths"
    );
    assert!(
        passive_overlay_strokes >= 6,
        "flowchart guide lines and stacked-page edges should render as passive strokes"
    );
    for forbidden in [
        b"shapeType".as_slice(),
        b"fillColor",
        b"lineColor",
        b"pFragments",
        b"flowchart-predefined-process-payload",
        b"flowchart-internal-storage-payload",
        b"flowchart-document-payload",
        b"flowchart-multidocument-payload",
        b"[Shape skipped",
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
            "flowchart document metadata leaked to PDF: {:?}",
            String::from_utf8_lossy(forbidden)
        );
    }
}
