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
<< /AF [4 0 R] /Perms << /DocMDP 7 0 R /FieldMDP 18 0 R /UR3 19 0 R >> /Encrypt 8 0 R /Collection << >> >>
endobj
7 0 obj
<< /S /GoTo /D [2 0 R /Fit] /Next << /S /Named /N /Print >> >>
endobj
8 0 obj
<< /S /ResetForm /FDF << /Fields [] >> >>
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
16 0 obj
<< /SigFlags 3 /Fields [17 0 R] /NeedAppearances true /PageMode /UseAttachments >>
endobj
17 0 obj
<< /FT /Sig /T (signature) >>
endobj
18 0 obj
<< /Type /Sig /Cert (certificate-bytes) >>
endobj
19 0 obj
<< /DSS << /VRI << /ABCDEF << >> >> >> >>
endobj
20 0 obj
<< /XFA 21 0 R /XDP <3c7864703e3c2f7864703e> >>
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
        assert!(error.issues.iter().any(|issue| issue.token == "/FDF"));
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
        assert!(error.issues.iter().any(|issue| issue.token == "/Cert"));
        assert!(error.issues.iter().any(|issue| issue.token == "/DocMDP"));
        assert!(error.issues.iter().any(|issue| issue.token == "/DSS"));
        assert!(error.issues.iter().any(|issue| issue.token == "/FieldMDP"));
        assert!(error.issues.iter().any(|issue| issue.token == "/GoTo"));
        assert!(error.issues.iter().any(|issue| issue.token == "/Named"));
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.token == "/NeedAppearances")
        );
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
        assert!(error.issues.iter().any(|issue| issue.token == "/Sig"));
        assert!(error.issues.iter().any(|issue| issue.token == "/SigFlags"));
        assert!(error.issues.iter().any(|issue| issue.token == "/Trans"));
        assert!(error.issues.iter().any(|issue| issue.token == "/UR3"));
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.token == "/UseAttachments")
        );
        assert!(error.issues.iter().any(|issue| issue.token == "/VRI"));
        assert!(error.issues.iter().any(|issue| issue.token == "/XDP"));
        assert!(error.issues.iter().any(|issue| issue.token == "/XFA"));
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
<< /Names << /Embedded#46iles [(payload.bin) 3 0 R] >> /#45F << /F 3 0 R >> /#46DF << >> /Need#41ppearances true >>
endobj
3 0 obj
<< /Type /Embedded#46ile /AF#52elationship /Data /#58FA 4 0 R /#58DP <3c7864703e3c2f7864703e> >>
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
            "/FDF",
            "/EmbeddedFile",
            "/AFRelationship",
            "/NeedAppearances",
            "/XFA",
            "/XDP",
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
    fn passive_pdf_audit_ignores_fake_lengths_inside_dictionary_strings() {
        let pdf = b"%PDF-1.7
1 0 obj
<< /Info (<< /Length 12 >>) >>
stream
BT (visible text) Tj ET
endstream
endobj
%%EOF";

        let error = audit_passive_pdf_bytes(pdf)
            .expect_err("string content must not provide stream audit bounds");

        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.token == MISSING_STREAM_LENGTH_TOKEN),
            "issues were {:?}",
            error.issues
        );
    }

    #[test]
    fn passive_pdf_audit_ignores_fake_lengths_inside_dictionary_arrays() {
        let pdf = b"%PDF-1.7
1 0 obj
<< /Info [/Length 12 <2f4c656e677468203132>] >>
stream
BT (visible text) Tj ET
endstream
endobj
%%EOF";

        let error =
            audit_passive_pdf_bytes(pdf).expect_err("array content must not provide stream bounds");

        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.token == MISSING_STREAM_LENGTH_TOKEN),
            "issues were {:?}",
            error.issues
        );
    }

    #[test]
    fn passive_pdf_audit_accepts_stream_length_after_comments() {
        let stream = b"BT (visible text) Tj ET\n";
        let mut pdf = format!(
            "%PDF-1.7\n1 0 obj\n<< /Info (<< /Length 999 >>) /Length % harmless comment\n{} >>\nstream\n",
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
    fn passive_pdf_audit_rejects_missing_direct_stream_length() {
        let pdf = b"%PDF-1.7
1 0 obj
<< >>
stream
BT (visible text) Tj ET
endstream
endobj
%%EOF";

        let error = audit_passive_pdf_bytes(pdf)
            .expect_err("stream without direct length has ambiguous passive audit bounds");

        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.token == MISSING_STREAM_LENGTH_TOKEN),
            "issues were {:?}",
            error.issues
        );
    }

    #[test]
    fn passive_pdf_audit_rejects_duplicate_direct_stream_length() {
        let pdf = b"%PDF-1.7
1 0 obj
<< /Length 0 /Length 22 >>
stream
BT (visible text) Tj ET
endstream
endobj
%%EOF";

        let error = audit_passive_pdf_bytes(pdf)
            .expect_err("duplicate direct stream lengths have ambiguous passive audit bounds");

        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.token == DUPLICATE_STREAM_LENGTH_TOKEN),
            "issues were {:?}",
            error.issues
        );
    }

    #[test]
    fn passive_pdf_audit_rejects_indirect_stream_length() {
        let pdf = b"%PDF-1.7
1 0 obj
<< /Length 2 0 R >>
stream
BT (visible text) Tj ET
endstream
endobj
2 0 obj
22
endobj
%%EOF";

        let error = audit_passive_pdf_bytes(pdf)
            .expect_err("indirect stream length has ambiguous passive audit bounds");

        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.token == MISSING_STREAM_LENGTH_TOKEN),
            "issues were {:?}",
            error.issues
        );
    }

    #[test]
    fn passive_pdf_audit_rejects_malformed_stream_length_token() {
        let pdf = b"%PDF-1.7
1 0 obj
<< /Length 12abc >>
stream
BT (visible text) Tj ET
endstream
endobj
%%EOF";

        let error = audit_passive_pdf_bytes(pdf)
            .expect_err("malformed stream length has ambiguous passive audit bounds");

        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.token == MISSING_STREAM_LENGTH_TOKEN),
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
