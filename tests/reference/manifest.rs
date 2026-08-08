use super::*;

#[test]
fn word_reference_policy_manifest_covers_existing_visual_fixtures() {
    assert!(MANIFEST.contains("\"schema\": 1"));
    assert!(
        MANIFEST.contains("development references only"),
        "manifest should document that Word is not a production dependency"
    );

    for fixture in [
        "fixtures/simple.rtf",
        "fixtures/table-ish.rtf",
        "fixtures/weird.rtf",
        "fixtures/object-result.rtf",
        "fixtures/png-alpha.rtf",
        "fixtures/png-trns.rtf",
        "fixtures/windows-bitmap.rtf",
        "fixtures/dib-grayscale-passive.rtf",
        "fixtures/wmf-vector-passive.rtf",
        "fixtures/wmf-text-passive.rtf",
        "fixtures/wmf-cliprect-passive.rtf",
        "fixtures/wmf-hatch-brush-passive.rtf",
        "fixtures/wmf-dashed-pen-passive.rtf",
        "fixtures/wmf-arc-family-passive.rtf",
        "fixtures/wmf-polybezier-passive.rtf",
        "fixtures/wmf-polybezierto-passive.rtf",
        "fixtures/wmf-polypolygon-passive.rtf",
        "fixtures/wmf-setpixel-passive.rtf",
        "fixtures/wmf-roundrect-passive.rtf",
        "fixtures/wmf-line-poly-paths-passive.rtf",
        "fixtures/wmf-patblt-passive.rtf",
        "fixtures/wmf-stretchblt-patcopy-passive.rtf",
        "fixtures/wmf-bitblt-patcopy-passive.rtf",
        "fixtures/wmf-dibbitblt-patcopy-passive.rtf",
        "fixtures/wmf-dibstretchblt-patcopy-passive.rtf",
        "fixtures/wmf-stretchdib-patcopy-passive.rtf",
        "fixtures/wmf-srccopy-stretchdib-passive.rtf",
        "fixtures/wmf-setdib-to-dev-passive.rtf",
        "fixtures/wmf-srccopy-dib-blits-passive.rtf",
        "fixtures/wmf-srccopy-cropped-dib-passive.rtf",
        "fixtures/wmf-srccopy-cropped-jpeg-dib-passive.rtf",
        "fixtures/emf-vector-passive.rtf",
        "fixtures/emf-text-passive.rtf",
        "fixtures/emf-cliprect-passive.rtf",
        "fixtures/emf-hatch-brush-passive.rtf",
        "fixtures/emf-arc-family-passive.rtf",
        "fixtures/emf-roundrect-passive.rtf",
        "fixtures/emf-setpixelv-passive.rtf",
        "fixtures/emf-polyline-polygon-passive.rtf",
        "fixtures/emf-polypolygon-passive.rtf",
        "fixtures/emf-bezier-passive.rtf",
        "fixtures/emf-polydraw-passive.rtf",
        "fixtures/emf-anglearc-passive.rtf",
        "fixtures/emf-savedc-restoredc-passive.rtf",
        "fixtures/static-shape-wrap.rtf",
        "fixtures/shape-wrap-distance-passive.rtf",
        "fixtures/floating-table-positioning.rtf",
        "fixtures/office-math-passive.rtf",
        "fixtures/office-math-structures-passive.rtf",
        "fixtures/office-math-flow-passive.rtf",
        "fixtures/office-math-limits-arrays-passive.rtf",
        "fixtures/office-math-nary-visible-passive.rtf",
        "fixtures/field-hyperlink-passive.rtf",
        "fixtures/generated-reference-fields-passive.rtf",
        "fixtures/mail-merge-prompt-fields-passive.rtf",
        "fixtures/template-barcode-embed-fields-passive.rtf",
        "fixtures/environment-layout-clock-fields-passive.rtf",
        "fixtures/nonvisible-storage-fields-passive.rtf",
        "fixtures/document-property-fields-passive.rtf",
        "fixtures/info-custom-docvariable-fields-passive.rtf",
        "fixtures/generated-counter-fields-passive.rtf",
        "fixtures/cross-reference-fields-passive.rtf",
        "fixtures/styleref-resultless-passive.rtf",
        "fixtures/styleref-multilevel-passive.rtf",
        "fixtures/literal-control-drawing-fields-passive.rtf",
        "fixtures/timestamp-stat-fields-passive.rtf",
        "fixtures/line-numbering-passive.rtf",
        "fixtures/page-border-art-passive.rtf",
        "fixtures/page-border-scope-passive.rtf",
        "fixtures/character-typography-passive.rtf",
        "fixtures/character-spacing-units-passive.rtf",
        "fixtures/character-scaling-spacing-passive.rtf",
        "fixtures/scaled-justification-passive.rtf",
        "fixtures/distributed-run-boundary-passive.rtf",
        "fixtures/section-grid-page-number-passive.rtf",
        "fixtures/page-number-format-restart-passive.rtf",
        "fixtures/table-spacing-autofit-passive.rtf",
        "fixtures/table-padding-passive.rtf",
        "fixtures/table-row-height-passive.rtf",
        "fixtures/table-cell-text-direction-passive.rtf",
        "fixtures/table-cell-vertical-align-passive.rtf",
        "fixtures/table-header-repeat-passive.rtf",
        "fixtures/tall-table-row-split-passive.rtf",
        "fixtures/table-row-keep-together-passive.rtf",
        "fixtures/table-row-keep-follow-passive.rtf",
        "fixtures/paragraph-keep-pagination-passive.rtf",
        "fixtures/widow-control-passive.rtf",
        "fixtures/no-wrap-passive.rtf",
        "fixtures/page-break-before-passive.rtf",
        "fixtures/section-break-kinds-passive.rtf",
        "fixtures/odd-section-break-passive.rtf",
        "fixtures/even-section-break-passive.rtf",
        "fixtures/page-vertical-alignment-passive.rtf",
        "fixtures/page-gutter-passive.rtf",
        "fixtures/landscape-zero-geometry-passive.rtf",
        "fixtures/alignment-direction-passive.rtf",
        "fixtures/paragraph-indent-aliases-passive.rtf",
        "fixtures/paragraph-spacing-passive.rtf",
        "fixtures/line-spacing-passive.rtf",
        "fixtures/soft-breaks-passive.rtf",
        "fixtures/drop-cap-passive.rtf",
        "fixtures/framed-drop-cap-passive.rtf",
        "fixtures/framed-drop-cap-derived-dimensions-passive.rtf",
        "fixtures/text-effects-passive.rtf",
        "fixtures/text-decoration-passive.rtf",
        "fixtures/character-highlight-border-passive.rtf",
        "fixtures/hyphenation-passive.rtf",
        "fixtures/form-field-shading-passive.rtf",
        "fixtures/note-placement-passive.rtf",
        "fixtures/note-restart-numbering-passive.rtf",
        "fixtures/shape-rotation-passive.rtf",
        "fixtures/section-columns-passive.rtf",
        "fixtures/column-section-break-passive.rtf",
        "fixtures/tab-alignment-passive.rtf",
        "fixtures/tab-leader-spacing-passive.rtf",
        "fixtures/tab-leader-variants-passive.rtf",
        "fixtures/legacy-textbox-passive.rtf",
        "fixtures/shading-patterns-passive.rtf",
        "fixtures/paragraph-borders-passive.rtf",
        "fixtures/table-borders-passive.rtf",
        "fixtures/carried-table-row-definitions-passive.rtf",
        "fixtures/intbl-zero-body-flow-passive.rtf",
        "fixtures/table-merged-cells-passive.rtf",
        "fixtures/nested-table-flatten-passive.rtf",
        "fixtures/old-style-list-passive.rtf",
        "fixtures/section-number-passive.rtf",
        "fixtures/picture-scaling-passive.rtf",
        "fixtures/picture-adjustments-passive.rtf",
        "fixtures/jpeg-trailing-payload-passive.rtf",
        "fixtures/associated-character-passive.rtf",
        "fixtures/font-family-hints-passive.rtf",
        "fixtures/font-charset-greek-passive.rtf",
        "fixtures/font-charset-cyrillic-passive.rtf",
        "fixtures/font-charset-central-european-passive.rtf",
        "fixtures/font-charset-turkish-passive.rtf",
        "fixtures/font-charset-baltic-passive.rtf",
        "fixtures/font-code-page-passive.rtf",
        "fixtures/font-charset-legacy-code-page-passive.rtf",
        "fixtures/zero-width-formatting-passive.rtf",
        "fixtures/symbol-font-passive.rtf",
        "fixtures/symbol-field-passive.rtf",
        "fixtures/pntext-symbol-list-passive.rtf",
        "fixtures/wingdings-checkbox-passive.rtf",
        "fixtures/wingdings2-checkbox-passive.rtf",
        "fixtures/wingdings3-arrows-passive.rtf",
        "fixtures/webdings-checkbox-passive.rtf",
        "fixtures/segoe-ui-symbol-checkbox-passive.rtf",
        "fixtures/form-checkbox-passive.rtf",
        "fixtures/form-text-dropdown-passive.rtf",
        "fixtures/active-control-fields-passive.rtf",
        "fixtures/document-protection-passive.rtf",
        "fixtures/mailmerge-object-metadata-passive.rtf",
        "fixtures/private-metadata-passive.rtf",
        "fixtures/review-bookmark-metadata-passive.rtf",
        "fixtures/hidden-animation-passive.rtf",
        "fixtures/font-template-metadata-passive.rtf",
        "fixtures/macro-script-metadata-passive.rtf",
        "fixtures/html-xml-metadata-passive.rtf",
        "fixtures/word-metadata-session-passive.rtf",
        "fixtures/stylesheet-metadata-passive.rtf",
        "fixtures/structural-metadata-passive.rtf",
        "fixtures/word-grid-outline-metadata-passive.rtf",
        "fixtures/stylesheet-semantics-passive.rtf",
        "fixtures/word-layout-compatibility-passive.rtf",
        "fixtures/section-pages-passive.rtf",
        "fixtures/header-footer-variants-passive.rtf",
        "fixtures/header-footer-distance-passive.rtf",
        "fixtures/background-shape-passive.rtf",
        "fixtures/shape-z-order-text-passive.rtf",
        "fixtures/shape-arrowheads-passive.rtf",
        "fixtures/shape-line-dashing-passive.rtf",
        "fixtures/shape-line-cap-passive.rtf",
        "fixtures/shape-line-join-passive.rtf",
        "fixtures/shape-pattern-fill-passive.rtf",
        "fixtures/shape-gradient-fill-passive.rtf",
        "fixtures/shape-shadow-opacity-passive.rtf",
        "fixtures/shape-fill-fore-color-passive.rtf",
        "fixtures/shape-line-fore-color-passive.rtf",
        "fixtures/shape-fill-back-color-passive.rtf",
        "fixtures/shape-line-back-color-passive.rtf",
        "fixtures/shape-fill-flag-passive.rtf",
        "fixtures/shape-line-flag-passive.rtf",
        "fixtures/shape-disabled-fill-passive.rtf",
        "fixtures/shape-disabled-line-passive.rtf",
        "fixtures/shape-hidden-passive.rtf",
        "fixtures/shape-flip-passive.rtf",
        "docs/sample.rtf",
    ] {
        assert!(
            Path::new(fixture).is_file(),
            "manifest references missing fixture {fixture}"
        );
        assert!(
            MANIFEST.contains(&format!("\"input\": \"{fixture}\"")),
            "manifest must classify {fixture}"
        );
    }

    for category in ["must_match_closely", "intentional_security_difference"] {
        assert!(
            MANIFEST.contains(&format!("\"category\": \"{category}\"")),
            "manifest must include category {category}"
        );
    }
    for line in MANIFEST
        .lines()
        .filter(|line| line.contains("\"category\":"))
    {
        assert!(
            [
                "must_match_closely",
                "acceptable_approximation",
                "intentional_security_difference",
            ]
            .iter()
            .any(|category| line.contains(&format!("\"{category}\""))),
            "manifest contains unknown category: {line}"
        );
    }

    assert!(
        MANIFEST.contains("\"word_reference_status\": \"pending_word_export\""),
        "current fixtures should explicitly mark missing Word references instead of implying coverage"
    );
    assert!(
        MANIFEST.contains("\"word_reference_pdf\": null"),
        "missing Word reference PDFs should be explicit"
    );
    assert!(
        MANIFEST.contains("\"intentional_security_differences\""),
        "security-sensitive fixtures must document intentional Word differences"
    );
    assert!(
        MANIFEST.contains("\"known_gaps\""),
        "visual fixtures must track missing comparison evidence"
    );
    assert_eq!(
        MANIFEST.matches("\"expected_diagnostics\"").count(),
        reference_fixtures().len(),
        "each manifest fixture should explicitly document expected diagnostics"
    );
    assert_eq!(
        MANIFEST.matches("\"forbidden_pdf_markers\"").count(),
        reference_fixtures().len(),
        "each manifest fixture should explicitly document source/control/PDF markers that must not reach PDF bytes"
    );
    assert_eq!(
        MANIFEST.matches("\"expected_pdf_markers\"").count(),
        reference_fixtures().len(),
        "each manifest fixture should explicitly document passive PDF markers required by the executable fixture gate"
    );
}

#[test]
fn available_word_reference_pdfs_match_manifest_contract() {
    let mut failures = Vec::new();
    for fixture in manifest_reference_fixtures() {
        match fixture.word_reference_status.as_str() {
            "pending_word_export" => assert!(
                fixture.word_reference_pdf.is_none(),
                "{} cannot mark a Word reference pending while naming a PDF",
                fixture.input
            ),
            "available" => {
                let Some(path) = fixture.word_reference_pdf.as_deref() else {
                    failures.push(format!("{} marks a missing Word reference", fixture.input));
                    continue;
                };
                if !Path::new(path).is_file() {
                    failures.push(format!(
                        "{} names missing Word reference PDF {}",
                        fixture.input, path
                    ));
                    continue;
                }
                let Ok(pdf) = PdfDocument::load(path) else {
                    failures.push(format!("{} Word reference PDF is invalid", fixture.input));
                    continue;
                };
                let page_count = pdf.get_pages().len();
                if page_count != fixture.word_expected_pages {
                    failures.push(format!(
                        "{} Word reference page count drifted: expected {}, got {}",
                        fixture.input, fixture.word_expected_pages, page_count
                    ));
                }
                let rendered_text = normalize_pdf_contract_text(&decoded_pdf_text(&pdf));
                for expected in &fixture.word_must_preserve_text {
                    let expected = normalize_pdf_contract_text(expected);
                    if !pdf_contract_text_contains(&rendered_text, &expected) {
                        failures.push(format!(
                            "{} Word reference PDF did not contain {:?}",
                            fixture.input, expected
                        ));
                    }
                }
            }
            status => panic!(
                "{} has unsupported word_reference_status {:?}",
                fixture.input, status
            ),
        }
    }
    assert!(
        failures.is_empty(),
        "Word reference contract failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn executable_reference_fixtures_follow_manifest_policy_entries() {
    let manifest_fixtures = manifest_reference_fixtures();
    assert_eq!(
        manifest_fixtures.len(),
        reference_fixtures().len(),
        "executable fixture count should stay in lockstep with manifest policy entries"
    );

    for fixture in reference_fixtures() {
        let manifest = manifest_fixtures
            .iter()
            .find(|manifest| manifest.input == fixture.input)
            .unwrap_or_else(|| panic!("manifest missing executable fixture {}", fixture.input));
        assert_eq!(
            manifest.expected_pages, fixture.expected_pages,
            "{} expected_pages drifted between manifest and executable gate",
            fixture.input
        );
        assert_eq!(
            manifest.must_preserve_text,
            fixture
                .must_preserve_text
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>(),
            "{} must_preserve_text drifted between manifest and executable gate",
            fixture.input
        );
        assert_eq!(
            manifest.must_not_leak,
            fixture
                .must_not_leak
                .iter()
                .map(|marker| String::from_utf8_lossy(marker).into_owned())
                .collect::<Vec<_>>(),
            "{} forbidden_pdf_markers drifted between manifest and executable gate",
            fixture.input
        );
        assert_eq!(
            manifest.must_contain_pdf,
            fixture
                .must_contain_pdf
                .iter()
                .map(|marker| String::from_utf8_lossy(marker).into_owned())
                .collect::<Vec<_>>(),
            "{} expected_pdf_markers drifted between manifest and executable gate",
            fixture.input
        );
        assert_eq!(
            manifest.must_emit_diagnostics,
            fixture
                .must_emit_diagnostics
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>(),
            "{} expected_diagnostics drifted between manifest and executable gate",
            fixture.input
        );
    }
}
