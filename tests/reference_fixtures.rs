use std::path::Path;

use lopdf::Document as PdfDocument;
use lopdf::content::Content;
use open_rtf_converter::pdf::audit_passive_pdf_bytes;
use open_rtf_converter::{ConvertOptions, convert_rtf_to_pdf};

const MANIFEST: &str = include_str!("../fixtures/reference/expected-policy.json");

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
        "fixtures/static-shape-wrap.rtf",
        "fixtures/shape-wrap-distance-passive.rtf",
        "fixtures/floating-table-positioning.rtf",
        "fixtures/office-math-passive.rtf",
        "fixtures/field-hyperlink-passive.rtf",
        "fixtures/line-numbering-passive.rtf",
        "fixtures/page-border-art-passive.rtf",
        "fixtures/character-typography-passive.rtf",
        "fixtures/section-grid-page-number-passive.rtf",
        "fixtures/table-spacing-autofit-passive.rtf",
        "fixtures/table-padding-passive.rtf",
        "fixtures/table-row-height-passive.rtf",
        "fixtures/table-cell-text-direction-passive.rtf",
        "fixtures/table-cell-vertical-align-passive.rtf",
        "fixtures/table-header-repeat-passive.rtf",
        "fixtures/table-row-keep-together-passive.rtf",
        "fixtures/table-row-keep-follow-passive.rtf",
        "fixtures/hyphenation-passive.rtf",
        "fixtures/form-field-shading-passive.rtf",
        "fixtures/note-placement-passive.rtf",
        "fixtures/shape-rotation-passive.rtf",
        "fixtures/section-columns-passive.rtf",
        "fixtures/tab-alignment-passive.rtf",
        "fixtures/legacy-textbox-passive.rtf",
        "fixtures/shading-patterns-passive.rtf",
        "fixtures/table-borders-passive.rtf",
        "fixtures/table-merged-cells-passive.rtf",
        "fixtures/old-style-list-passive.rtf",
        "fixtures/section-number-passive.rtf",
        "fixtures/picture-scaling-passive.rtf",
        "fixtures/associated-character-passive.rtf",
        "fixtures/font-family-hints-passive.rtf",
        "fixtures/section-pages-passive.rtf",
        "fixtures/header-footer-variants-passive.rtf",
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

    for category in [
        "must_match_closely",
        "acceptable_approximation",
        "intentional_security_difference",
    ] {
        assert!(
            MANIFEST.contains(&format!("\"category\": \"{category}\"")),
            "manifest must include category {category}"
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
fn reference_policy_fixtures_match_current_passive_converter_output() {
    for fixture in manifest_reference_fixtures() {
        let input = std::fs::read(&fixture.input).unwrap_or_else(|error| {
            panic!(
                "failed to read reference fixture {}: {error}",
                fixture.input
            )
        });
        let output = convert_rtf_to_pdf(
            &input,
            &ConvertOptions {
                diagnostics: true,
                ..ConvertOptions::default()
            },
        )
        .unwrap_or_else(|error| panic!("failed to convert {}: {error}", fixture.input));

        assert_eq!(
            output.pages, fixture.expected_pages,
            "{} should render the expected page count",
            fixture.input
        );
        audit_passive_pdf_bytes(&output.pdf).unwrap_or_else(|error| {
            panic!("{} emitted active PDF content: {error}", fixture.input)
        });

        let pdf = PdfDocument::load_mem(&output.pdf)
            .unwrap_or_else(|error| panic!("{} emitted invalid PDF: {error}", fixture.input));
        assert_eq!(
            pdf.get_pages().len(),
            fixture.expected_pages,
            "{} PDF page tree should match report",
            fixture.input
        );
        let rendered_text = decoded_pdf_text(&pdf);
        for expected in &fixture.must_preserve_text {
            assert!(
                rendered_text.contains(expected),
                "{} rendered PDF text did not contain {:?}; text was {:?}",
                fixture.input,
                expected,
                rendered_text
            );
        }
        for forbidden in &fixture.must_not_leak {
            let forbidden = forbidden.as_bytes();
            assert!(
                !output
                    .pdf
                    .windows(forbidden.len())
                    .any(|window| window == forbidden),
                "{} leaked forbidden source/control bytes {:?}",
                fixture.input,
                String::from_utf8_lossy(forbidden)
            );
        }
        for expected in &fixture.must_contain_pdf {
            let expected = expected.as_bytes();
            assert!(
                output
                    .pdf
                    .windows(expected.len())
                    .any(|window| window == expected),
                "{} rendered PDF did not contain expected passive marker {:?}",
                fixture.input,
                String::from_utf8_lossy(expected)
            );
        }
        for expected in &fixture.must_emit_diagnostics {
            assert!(
                output
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "{} did not emit expected diagnostic {:?}; diagnostics were {:?}",
                fixture.input,
                expected,
                output.diagnostics
            );
        }
    }
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

#[derive(Debug)]
struct ManifestReferenceFixture {
    input: String,
    expected_pages: usize,
    must_preserve_text: Vec<String>,
    must_not_leak: Vec<String>,
    must_contain_pdf: Vec<String>,
    must_emit_diagnostics: Vec<String>,
}

struct ReferenceFixture {
    input: &'static str,
    expected_pages: usize,
    must_preserve_text: &'static [&'static str],
    must_not_leak: &'static [&'static [u8]],
    must_contain_pdf: &'static [&'static [u8]],
    must_emit_diagnostics: &'static [&'static str],
}

fn manifest_reference_fixtures() -> Vec<ManifestReferenceFixture> {
    let fixtures = json_array_for_key(MANIFEST, "fixtures");
    split_json_objects(fixtures)
        .into_iter()
        .map(|object| ManifestReferenceFixture {
            input: json_string_for_key(object, "input"),
            expected_pages: json_usize_for_key(object, "expected_pages"),
            must_preserve_text: json_string_array_for_key(object, "must_preserve_text"),
            must_not_leak: json_string_array_for_key(object, "forbidden_pdf_markers"),
            must_contain_pdf: json_string_array_for_key(object, "expected_pdf_markers"),
            must_emit_diagnostics: json_string_array_for_key(object, "expected_diagnostics"),
        })
        .collect()
}

fn reference_fixtures() -> &'static [ReferenceFixture] {
    &[
        ReferenceFixture {
            input: "fixtures/simple.rtf",
            expected_pages: 2,
            must_preserve_text: &[
                "Hello from open-rtf-converter",
                "Centered paragraph with",
                "Second page text",
            ],
            must_not_leak: &[b"fonttbl", b"colortbl", b"/JavaScript", b"/EmbeddedFile"],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/table-ish.rtf",
            expected_pages: 1,
            must_preserve_text: &["Name", "Value", "Alpha", "Beta", "After table text"],
            must_not_leak: &[b"trowd", b"cellx", b"/JavaScript", b"/EmbeddedFile"],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/weird.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "visible text should survive",
                "Escaped braces: {sample}",
                "hex: ABC",
            ],
            must_not_leak: &[
                b"unknownDestination",
                b"madeup123",
                b"/JavaScript",
                b"/EmbeddedFile",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/object-result.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "Before object result.",
                "visible fallback",
                "After object result.",
            ],
            must_not_leak: &[
                b"objemb",
                b"objdata",
                b"414243",
                b"JavaScript",
                b"EmbeddedFile",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/png-alpha.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before alpha image.", "After alpha image."],
            must_not_leak: &[
                b"pngblip",
                b"IHDR",
                b"IDAT",
                b"IEND",
                b"/JavaScript",
                b"/EmbeddedFile",
            ],
            must_contain_pdf: &[b"/SMask"],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/png-trns.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "Before indexed transparency image.",
                "After indexed transparency image.",
            ],
            must_not_leak: &[
                b"pngblip",
                b"IHDR",
                b"PLTE",
                b"tRNS",
                b"IDAT",
                b"IEND",
                b"/JavaScript",
                b"/EmbeddedFile",
            ],
            must_contain_pdf: &[b"/SMask"],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/windows-bitmap.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "Before Windows bitmap image.",
                "After Windows bitmap image.",
            ],
            must_not_leak: &[
                b"wbitmap",
                b"wbmplanes",
                b"wbmbitspixel",
                b"wbmwidthbytes",
                b"ff0000ffffff0000ff00ff00",
                b"/JavaScript",
                b"/EmbeddedFile",
            ],
            must_contain_pdf: &[b"/Subtype /Image"],
            must_emit_diagnostics: &[
                "Windows bitmap picture rendered as bounded passive RGB image",
            ],
        },
        ReferenceFixture {
            input: "fixtures/static-shape-wrap.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "Before wrapped static shape.",
                "Wrapped static shape text should flow beside the passive frame",
                "After wrapped static shape.",
            ],
            must_not_leak: &[
                b"shpinst",
                b"shpleft",
                b"shptop",
                b"shpright",
                b"shpbottom",
                b"shpwr",
                b"shpwrk",
                b"shapeType",
                b"pFragments",
                b"hidden-static-wrap-reference-payload",
                b"/JavaScript",
                b"/EmbeddedFile",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "rendering bounded passive static drawing shape and stripping unsupported/active drawing properties",
            ],
        },
        ReferenceFixture {
            input: "fixtures/shape-wrap-distance-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "Before shape wrap distance.",
                "Wrapped paragraph should honor the passive shape wrap distances",
                "After shape wrap distance.",
            ],
            must_not_leak: &[
                b"shpinst",
                b"shpleft",
                b"shptop",
                b"shpright",
                b"shpbottom",
                b"shpwr",
                b"shpwrk",
                b"shapeType",
                b"dxWrapDistLeft",
                b"dxWrapDistRight",
                b"dyWrapDistTop",
                b"dyWrapDistBottom",
                b"pFragments",
                b"hostile-wrap-distance-reference-payload",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "rendering bounded passive static drawing shape and stripping unsupported/active drawing properties",
            ],
        },
        ReferenceFixture {
            input: "fixtures/floating-table-positioning.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "Before floating table.",
                "Floating left",
                "Floating right",
                "Follow left",
                "Follow right",
                "After floating table.",
            ],
            must_not_leak: &[
                b"tabsnoovrlp",
                b"trgaph",
                b"trleft",
                b"tphmrg",
                b"posx",
                b"pvmrg",
                b"posy",
                b"tdfrmtxt",
                b"cellx",
                b"trowd",
                b"/JavaScript",
                b"/EmbeddedFile",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "floating table no-overlap \\tabsnoovrlp captured as bounded passive row exclusion",
                "floating table horizontal position interpreted as bounded passive row offset",
                "floating table vertical position interpreted as bounded passive row offset",
                "floating table wrap distance interpreted as bounded passive row margin",
            ],
        },
        ReferenceFixture {
            input: "fixtures/office-math-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before math.", "E=mc2", "x+1", "y", "after math."],
            must_not_leak: &[
                b"mmath",
                b"moMath",
                b"mtext",
                b"msup",
                b"mnum",
                b"mden",
                b"xmlopen",
                b"hidden-office-math-reference-payload",
                b"objdata",
                b"414243",
                b"/JavaScript",
                b"/EmbeddedFile",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "Office math rendered as bounded passive math text",
                "active content removed: OLE object before safe model normalization",
            ],
        },
        ReferenceFixture {
            input: "fixtures/field-hyperlink-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "Before field.",
                "Visible safe link text",
                "after link.",
                "[Field removed: no passive result]",
                "after external field.",
            ],
            must_not_leak: &[
                b"fldinst",
                b"fldrslt",
                b"HYPERLINK",
                b"INCLUDEPICTURE",
                b"example.com",
                b"active?token=414243",
                b"pixel.png",
                b"/URI",
                b"/Annots",
                b"/Action",
                b"/JavaScript",
                b"/EmbeddedFile",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "rendering stored result for field HYPERLINK without executing field instruction",
                "external field INCLUDEPICTURE rendered as passive placeholder without fetching external resource",
            ],
        },
        ReferenceFixture {
            input: "fixtures/line-numbering-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "1Numbered first line.",
                "2Numbered second line.",
                "3Numbered third line.",
            ],
            must_not_leak: &[
                b"linemod",
                b"linex",
                b"lineppage",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &["line numbering rendered as bounded passive margin text"],
        },
        ReferenceFixture {
            input: "fixtures/page-border-art-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Body inside passive page border fallback."],
            must_not_leak: &[
                b"pgbrdrt",
                b"brdrart",
                b"brdrw80",
                b"brsp240",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/AcroForm",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "page border art rendered as bounded passive line border fallback",
            ],
        },
        ReferenceFixture {
            input: "fixtures/character-typography-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "Before typography.",
                "Visible styled typography",
                "After typography.",
            ],
            must_not_leak: &[
                b"kerning2",
                b"charscalex125",
                b"fittext720",
                b"objdata",
                b"414243",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "character kerning rendered as bounded passive pair spacing",
                "character fit-text rendered as bounded passive horizontal scaling",
                "active content removed: OLE object before safe model normalization",
            ],
        },
        ReferenceFixture {
            input: "fixtures/section-grid-page-number-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Page 1", "Visible section grid body.", "Second grid line."],
            must_not_leak: &[
                b"pgnx",
                b"pgny",
                b"sectlinegrid",
                b"sectdefaultcl",
                b"sectexpand",
                b"sectspecifycl",
                b"sectspecifyl",
                b"chpgn",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "page number position rendered as bounded passive header/footer coordinates",
                "section line grid applied as bounded passive paragraph line pitch",
                "section default text grid cleared bounded passive paragraph line pitch",
                "section text grid interpreted through bounded passive paragraph layout",
            ],
        },
        ReferenceFixture {
            input: "fixtures/table-spacing-autofit-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "Before table spacing.",
                "Unit left",
                "Unit right",
                "After table spacing.",
            ],
            must_not_leak: &[
                b"trowd",
                b"trautofit",
                b"trspdl",
                b"trspdfl",
                b"trspdr",
                b"trspdfr",
                b"trspdt",
                b"trspdft",
                b"trspdb",
                b"trspdfb",
                b"clspdl",
                b"clspdfl",
                b"clspdr",
                b"clspdfr",
                b"cellx",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "table row autofit interpreted through bounded passive table width layout",
                "table cell spacing rendered as bounded passive border gaps",
            ],
        },
        ReferenceFixture {
            input: "fixtures/table-padding-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "Before table padding.",
                "Row padded",
                "Cell padded",
                "After table padding.",
            ],
            must_not_leak: &[
                b"trowd",
                b"trgaph",
                b"trpaddfl",
                b"trpaddfr",
                b"trpaddft",
                b"trpaddfb",
                b"trpaddl",
                b"trpaddr",
                b"trpaddt",
                b"trpaddb",
                b"clpadfl",
                b"clpadfr",
                b"clpadft",
                b"clpadfb",
                b"clpadl",
                b"clpadr",
                b"clpadt",
                b"clpadb",
                b"cellx",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "table padding and spacing units interpreted through bounded twip layout",
            ],
        },
        ReferenceFixture {
            input: "fixtures/table-row-height-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "Before table row height.",
                "Minimum row",
                "Normal neighbor",
                "Exact row first line",
                "Exact neighbor",
                "After table row height.",
            ],
            must_not_leak: &[
                b"colortbl",
                b"red240",
                b"trowd",
                b"trgaph",
                b"trrh",
                b"clcbpat",
                b"clbrdrt",
                b"clbrdrl",
                b"clbrdrb",
                b"clbrdrr",
                b"brdrw",
                b"cellx",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/table-cell-text-direction-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "Before text direction table.",
                "Vertical ABC",
                "Alias vertical",
                "Bottom top XY",
                "Flat text",
                "After text direction table.",
            ],
            must_not_leak: &[
                b"trowd",
                b"trgaph",
                b"trrh",
                b"cltxtbrl",
                b"cltxtbrlv",
                b"cltxbtlr",
                b"cltxlrtb",
                b"clbrdrt",
                b"clbrdrl",
                b"clbrdrb",
                b"clbrdrr",
                b"brdrw",
                b"cellx",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/table-cell-vertical-align-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "Before vertical align table.",
                "Top aligned",
                "Center aligned",
                "Bottom aligned",
                "After vertical align table.",
            ],
            must_not_leak: &[
                b"trowd",
                b"trgaph",
                b"trrh",
                b"clvertalt",
                b"clvertalc",
                b"clvertalb",
                b"clbrdrt",
                b"clbrdrl",
                b"clbrdrb",
                b"clbrdrr",
                b"brdrw",
                b"cellx",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/table-header-repeat-passive.rtf",
            expected_pages: 5,
            must_preserve_text: &[
                "Before repeating table header.",
                "Header row",
                "Body row 01",
                "Body row 18",
                "Body row 36",
                "After repeating table header.",
            ],
            must_not_leak: &[
                b"trowd",
                b"trhdr",
                b"trhdr0",
                b"trrh",
                b"clcbpat",
                b"cellx",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/table-row-keep-together-passive.rtf",
            expected_pages: 2,
            must_preserve_text: &[
                "Before keep-together table.",
                "Filler row 1",
                "Filler row 3",
                "Kept tall row first line",
                "Kept tall row second line",
                "Normal row",
                "After keep-together table.",
            ],
            must_not_leak: &[
                b"trowd",
                b"trrh",
                b"trkeep",
                b"trkeep0",
                b"cellx",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/table-row-keep-follow-passive.rtf",
            expected_pages: 2,
            must_preserve_text: &[
                "Before keep-follow table.",
                "Filler row 1",
                "Filler row 3",
                "Keep follow row",
                "Follower row",
                "After keep-follow table.",
            ],
            must_not_leak: &[
                b"trowd",
                b"trrh",
                b"trkeepfollow",
                b"trkeepfollow0",
                b"cellx",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/hyphenation-passive.rtf",
            expected_pages: 5,
            must_preserve_text: &[
                "Antidisestabli-",
                "shmentarian-",
                "ism",
                "Characteristically",
            ],
            must_not_leak: &[
                b"hyphauto",
                b"hyphpar",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/AcroForm",
            ],
            must_contain_pdf: &[b"-"],
            must_emit_diagnostics: &[
                "document hyphenation rendered as bounded passive soft hyphenation",
                "paragraph hyphenation rendered as bounded passive soft hyphenation",
            ],
        },
        ReferenceFixture {
            input: "fixtures/form-field-shading-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before form field.", "Visible value", "After form field."],
            must_not_leak: &[
                b"formshade",
                b"FORMTEXT",
                b"formfield",
                b"HiddenName",
                b"HiddenDefault",
                b"launch.exe",
                b"datafield",
                b"414243",
                b"/AcroForm",
                b"/Widget",
                b"/AA",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "form-field shading rendered as bounded passive fill",
                "rendering stored result for passive form field FORMTEXT without creating PDF form actions",
            ],
        },
        ReferenceFixture {
            input: "fixtures/note-placement-passive.rtf",
            expected_pages: 3,
            must_preserve_text: &[
                "Body with footnote",
                "Footnote text",
                "and endnote",
                "Endnote text",
                "Next section body.",
            ],
            must_not_leak: &[
                b"ftnbj",
                b"endnhere",
                b"chftn",
                b"footnote",
                b"endnote",
                b"objdata",
                b"414243",
                b"444546",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/AcroForm",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "footnotes placed at passive page bottom without active note behavior",
                "endnotes placed at passive section boundary without active note behavior",
                "active content removed: OLE object before safe model normalization",
            ],
        },
        ReferenceFixture {
            input: "fixtures/shape-rotation-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before tilted shape.", "After tilted shape."],
            must_not_leak: &[
                b"shpinst",
                b"shpleft",
                b"shptop",
                b"shpright",
                b"shpbottom",
                b"shapeType",
                b"rotation",
                b"pFragments",
                b"hidden-rotated-shape-reference-payload",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "rendering bounded passive static drawing shape and stripping unsupported/active drawing properties",
                "shape rotation rendered as bounded passive static geometry",
            ],
        },
        ReferenceFixture {
            input: "fixtures/section-columns-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Left rail text.", "Right rail text."],
            must_not_leak: &[
                b"colsx",
                b"linebetcol",
                b"colno",
                b"colw",
                b"colsr",
                b"column",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/tab-alignment-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Left label", "9", "Decimal value", "12.3"],
            must_not_leak: &[
                b"tqr",
                b"tqdec",
                b"tldot",
                b"tx1440",
                b"tx2160",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/legacy-textbox-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before legacy box.", "Legacy box text", "After legacy box."],
            must_not_leak: &[
                b"dobx",
                b"dobxcolumn",
                b"doby",
                b"dobypara",
                b"dodhgt",
                b"dpxsize",
                b"dpysize",
                b"dptxbx",
                b"dptxbxtext",
                b"objemb",
                b"objclass",
                b"objdata",
                b"Word.Picture.8",
                b"414243",
                b"dpptx",
                b"hidden-legacy-textbox-coordinate-payload",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "active content removed: OLE object before safe model normalization",
            ],
        },
        ReferenceFixture {
            input: "fixtures/shading-patterns-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "Patterned paragraph.",
                "Row shade",
                "Cell shade",
                "After shading.",
            ],
            must_not_leak: &[
                b"colortbl",
                b"red255",
                b"green0",
                b"blue255",
                b"cbpat",
                b"cfpat",
                b"shading2500",
                b"bghoriz",
                b"trcbpat",
                b"trcfpat",
                b"trshdng",
                b"trbgvert",
                b"clcbpat",
                b"clcfpat",
                b"clshdng",
                b"clbgdkdcross",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/table-borders-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "Before table borders.",
                "Top left",
                "Diagonal cell",
                "Right cell",
                "After table borders.",
            ],
            must_not_leak: &[
                b"colortbl",
                b"red255",
                b"blue255",
                b"trowd",
                b"trbrdrt",
                b"trbrdrh",
                b"trbrdrv",
                b"clbrdrl",
                b"clbrdrt",
                b"cldglu",
                b"cldgll",
                b"brdrdb",
                b"brdrdash",
                b"brdrw",
                b"brdrcf",
                b"cellx",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/table-merged-cells-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "Before merged table.",
                "Horizontal merged",
                "Right top",
                "Vertical merged",
                "Middle",
                "Right middle",
                "Bottom middle",
                "Right bottom",
                "After merged table.",
            ],
            must_not_leak: &[
                b"colortbl",
                b"red220",
                b"trowd",
                b"trgaph",
                b"trrh",
                b"clmgf",
                b"clmrg",
                b"clvmgf",
                b"clvmrg",
                b"clcbpat",
                b"clbrdrt",
                b"clbrdrl",
                b"clbrdrb",
                b"clbrdrr",
                b"brdrw",
                b"cellx",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/old-style-list-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "Before old list.",
                "3.Indented decimal item",
                "IV.Upper roman item",
                "After old list.",
            ],
            must_not_leak: &[
                b"pnindent",
                b"pnhang",
                b"pnsp",
                b"pnstart",
                b"pndec",
                b"pnucrm",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/section-number-passive.rtf",
            expected_pages: 2,
            must_preserve_text: &["Part 1", "Part 2", "Part 3"],
            must_not_leak: &[
                b"sectnum",
                b"fldinst",
                b"SECTION",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/picture-scaling-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before scaled image.", "After scaled image."],
            must_not_leak: &[
                b"pngblip",
                b"picw2",
                b"pich1",
                b"picwgoal",
                b"pichgoal",
                b"picscalex",
                b"picscaley",
                b"piccropl",
                b"piccropt",
                b"piccropr",
                b"piccropb",
                b"IHDR",
                b"IDAT",
                b"IEND",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[b"/Subtype /Image"],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/associated-character-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "Before associated text.",
                "Associated styled text",
                "Plain after associated text.",
            ],
            must_not_leak: &[
                b"fonttbl",
                b"colortbl",
                b"loch",
                b"acf1",
                b"aul",
                b"aexpnd4",
                b"aup6",
                b"objdata",
                b"414243",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
                b"/URI",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "active content removed: OLE object before safe model normalization",
            ],
        },
        ReferenceFixture {
            input: "fixtures/font-family-hints-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "Roman family text.",
                "Modern family text.",
                "Theme heading text.",
                "Theme body text.",
            ],
            must_not_leak: &[
                b"fonttbl",
                b"froman",
                b"fmodern",
                b"flomajor",
                b"fhiminor",
                b"Mystery Serif",
                b"Mystery Mono",
                b"Mystery Heading",
                b"Mystery Body",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/section-pages-passive.rtf",
            expected_pages: 3,
            must_preserve_text: &[
                "First section pages 2",
                "First section continues.",
                "Second section pages 1",
            ],
            must_not_leak: &[
                b"fldinst",
                b"SECTIONPAGES",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/header-footer-variants-passive.rtf",
            expected_pages: 3,
            must_preserve_text: &[
                "First header",
                "Even header",
                "Odd header",
                "First footer",
                "Even footer",
                "Odd footer",
                "Page one body.",
                "Page two body.",
                "Page three body.",
            ],
            must_not_leak: &[
                b"headerf",
                b"headerl",
                b"headerr",
                b"footerf",
                b"footerl",
                b"footerr",
                b"titlepg",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/background-shape-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Body over passive background."],
            must_not_leak: &[
                b"background",
                b"Hidden background text",
                b"shpinst",
                b"shapeType",
                b"fillColor",
                b"pFragments",
                b"hostile-background-reference-payload",
                b"[Shape skipped",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/shape-z-order-text-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &[
                "Before z-order shape.",
                "Layered Box",
                "After z-order shape.",
            ],
            must_not_leak: &[
                b"shpinst",
                b"shpz",
                b"shapeType",
                b"pFragments",
                b"hidden-z-order-reference-payload",
                b"shptxt",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "shape z-order rendered through bounded passive drawing order",
            ],
        },
        ReferenceFixture {
            input: "fixtures/shape-arrowheads-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before arrow line.", "After arrow line."],
            must_not_leak: &[
                b"dpline",
                b"dpx360",
                b"dpy480",
                b"dpxsize1440",
                b"dpysize720",
                b"lineStartArrowhead",
                b"lineEndArrowhead",
                b"triangle",
                b"pFragments",
                b"hostile-arrowhead-reference-payload",
                b"[Shape skipped",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/shape-line-dashing-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before dashed line.", "After dashed line."],
            must_not_leak: &[
                b"dpline",
                b"dpx360",
                b"dpy480",
                b"dpxsize1440",
                b"dpysize720",
                b"lineDashing",
                b"dashDot",
                b"pFragments",
                b"hostile-line-dashing-reference-payload",
                b"[Shape skipped",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/shape-line-cap-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before capped line.", "After capped line."],
            must_not_leak: &[
                b"dpline",
                b"dpx360",
                b"dpy480",
                b"dpxsize1440",
                b"dpysize720",
                b"lineEndCap",
                b"round",
                b"pFragments",
                b"hostile-line-cap-reference-payload",
                b"[Shape skipped",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "rendering bounded passive static drawing shape and stripping unsupported/active drawing properties",
            ],
        },
        ReferenceFixture {
            input: "fixtures/shape-line-join-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before joined line.", "After joined line."],
            must_not_leak: &[
                b"dppolyline",
                b"dpptx360",
                b"dppty480",
                b"dpptx1440",
                b"dppty1200",
                b"lineWidth",
                b"lineJoinStyle",
                b"round",
                b"pFragments",
                b"hostile-line-join-reference-payload",
                b"[Shape skipped",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "rendering bounded passive static drawing shape and stripping unsupported/active drawing properties",
            ],
        },
        ReferenceFixture {
            input: "fixtures/shape-pattern-fill-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before pattern fill.", "After pattern fill."],
            must_not_leak: &[
                b"shpinst",
                b"shapeType",
                b"fillType",
                b"msoFillPattern",
                b"fillColor",
                b"pFragments",
                b"hostile-pattern-fill-reference-payload",
                b"[Shape skipped",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "rendering bounded passive static drawing shape and stripping unsupported/active drawing properties",
            ],
        },
        ReferenceFixture {
            input: "fixtures/shape-gradient-fill-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before gradient fill.", "After gradient fill."],
            must_not_leak: &[
                b"shpinst",
                b"shapeType",
                b"fillType",
                b"msofillShade",
                b"fillColor",
                b"fillBackColor",
                b"16711680",
                b"[Shape skipped",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[],
        },
        ReferenceFixture {
            input: "fixtures/shape-shadow-opacity-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before shadow shape.", "After shadow shape."],
            must_not_leak: &[
                b"shpinst",
                b"shapeType",
                b"fillColor",
                b"fShadow",
                b"shadowColor",
                b"shadowOpacity",
                b"32768",
                b"pFragments",
                b"hostile-shadow-opacity-reference-payload",
                b"[Shape skipped",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "rendering bounded passive static drawing shape and stripping unsupported/active drawing properties",
            ],
        },
        ReferenceFixture {
            input: "fixtures/shape-fill-fore-color-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before fill color.", "After fill color."],
            must_not_leak: &[
                b"shpinst",
                b"shapeType",
                b"fillForeColor",
                b"pFragments",
                b"hostile-fill-fore-color-reference-payload",
                b"[Shape skipped",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "rendering bounded passive static drawing shape and stripping unsupported/active drawing properties",
            ],
        },
        ReferenceFixture {
            input: "fixtures/shape-line-fore-color-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before stroke color.", "After stroke color."],
            must_not_leak: &[
                b"shpinst",
                b"shapeType",
                b"lineForeColor",
                b"pFragments",
                b"hostile-line-fore-color-reference-payload",
                b"[Shape skipped",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "rendering bounded passive static drawing shape and stripping unsupported/active drawing properties",
            ],
        },
        ReferenceFixture {
            input: "fixtures/shape-fill-back-color-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before back fill.", "After back fill."],
            must_not_leak: &[
                b"shpinst",
                b"shapeType",
                b"fillBackColor",
                b"pFragments",
                b"hostile-fill-back-color-reference-payload",
                b"[Shape skipped",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "rendering bounded passive static drawing shape and stripping unsupported/active drawing properties",
            ],
        },
        ReferenceFixture {
            input: "fixtures/shape-line-back-color-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before back stroke.", "After back stroke."],
            must_not_leak: &[
                b"shpinst",
                b"shapeType",
                b"lineBackColor",
                b"pFragments",
                b"hostile-line-back-color-reference-payload",
                b"[Shape skipped",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "rendering bounded passive static drawing shape and stripping unsupported/active drawing properties",
            ],
        },
        ReferenceFixture {
            input: "fixtures/shape-fill-flag-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before fill flag.", "After fill flag."],
            must_not_leak: &[
                b"shpinst",
                b"shapeType",
                b"fillColor",
                b"fFilled",
                b"pFragments",
                b"hostile-fill-flag-reference-payload",
                b"[Shape skipped",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "rendering bounded passive static drawing shape and stripping unsupported/active drawing properties",
            ],
        },
        ReferenceFixture {
            input: "fixtures/shape-line-flag-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before line flag.", "After line flag."],
            must_not_leak: &[
                b"shpinst",
                b"shapeType",
                b"fLine",
                b"pFragments",
                b"hostile-line-flag-reference-payload",
                b"[Shape skipped",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "rendering bounded passive static drawing shape and stripping unsupported/active drawing properties",
            ],
        },
        ReferenceFixture {
            input: "fixtures/shape-disabled-fill-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before disabled fill.", "After disabled fill."],
            must_not_leak: &[
                b"shpinst",
                b"shapeType",
                b"fFilled",
                b"fillColor",
                b"pFragments",
                b"hostile-disabled-fill-reference-payload",
                b"[Shape skipped",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "rendering bounded passive static drawing shape and stripping unsupported/active drawing properties",
            ],
        },
        ReferenceFixture {
            input: "fixtures/shape-disabled-line-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before disabled line.", "After disabled line."],
            must_not_leak: &[
                b"shpinst",
                b"shapeType",
                b"fLine",
                b"lineWidth",
                b"pFragments",
                b"hostile-disabled-line-reference-payload",
                b"[Shape skipped",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "rendering bounded passive static drawing shape and stripping unsupported/active drawing properties",
            ],
        },
        ReferenceFixture {
            input: "fixtures/shape-hidden-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before hidden shape.", "After hidden shape."],
            must_not_leak: &[
                b"shpinst",
                b"shapeType",
                b"fHidden",
                b"fillColor",
                b"lineWidth",
                b"pFragments",
                b"hostile-hidden-shape-reference-payload",
                b"Hidden shape text must not render.",
                b"[Shape skipped",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &["hidden shape stripped before safe model normalization"],
        },
        ReferenceFixture {
            input: "fixtures/shape-flip-passive.rtf",
            expected_pages: 1,
            must_preserve_text: &["Before flipped shape.", "After flipped shape."],
            must_not_leak: &[
                b"shpinst",
                b"shapeType",
                b"fFlipH",
                b"fFlipV",
                b"lineColor",
                b"lineWidth",
                b"pFragments",
                b"hostile-flipped-shape-reference-payload",
                b"[Shape skipped",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
                b"/RichMedia",
                b"/AcroForm",
                b"/Annots",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "rendering bounded passive static drawing shape and stripping unsupported/active drawing properties",
            ],
        },
        ReferenceFixture {
            input: "docs/sample.rtf",
            expected_pages: 2,
            must_preserve_text: &[
                "It is an example test rtf-file to RTF2XML bean for testing",
                "Simple table",
                "Here are some special characters",
                "At last you can see an image",
            ],
            must_not_leak: &[
                b"objdata",
                b"Word.Picture.8",
                b"METAFILEPICT",
                b"shppict",
                b"shprslt",
                b"wmetafile8",
                b"/JavaScript",
                b"/EmbeddedFile",
                b"/Launch",
                b"/OpenAction",
            ],
            must_contain_pdf: &[],
            must_emit_diagnostics: &[
                "rendering shape picture result with bounded passive shape frame",
                "ignoring duplicate embedded object alternate after passive shape result",
                "active content removed: object metadata in skipped destination",
                "ignoring duplicate shape result fallback after passive primary shape result",
            ],
        },
    ]
}

fn decoded_pdf_text(pdf: &PdfDocument) -> String {
    let mut output = String::new();
    for page_id in pdf.get_pages().values() {
        let content = pdf
            .get_and_decode_page_content(*page_id)
            .expect("page content should decode");
        output.push_str(&content_text(&content));
        output.push('\n');
    }
    output
}

fn content_text(content: &Content) -> String {
    let mut text = String::new();
    for operation in &content.operations {
        match operation.operator.as_ref() {
            "Tj" | "'" | "\"" => {
                for operand in &operation.operands {
                    if let Ok(bytes) = operand.as_str() {
                        text.push_str(&decode_pdf_text_bytes(bytes));
                    }
                }
            }
            "TJ" => {
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
            _ => {}
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

fn json_usize_for_key(object: &str, key: &str) -> usize {
    let marker = format!("\"{key}\":");
    let start = object
        .find(&marker)
        .unwrap_or_else(|| panic!("missing numeric key {key}"))
        + marker.len();
    let value = object[start..]
        .trim_start()
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    value
        .parse()
        .unwrap_or_else(|error| panic!("invalid numeric key {key}: {error}"))
}

fn json_string_for_key(object: &str, key: &str) -> String {
    let marker = format!("\"{key}\":");
    let start = object
        .find(&marker)
        .unwrap_or_else(|| panic!("missing string key {key}"))
        + marker.len();
    let value = object[start..].trim_start();
    let (decoded, _) = parse_json_string(value)
        .unwrap_or_else(|| panic!("invalid JSON string for key {key}: {value:?}"));
    decoded
}

fn json_string_array_for_key(object: &str, key: &str) -> Vec<String> {
    let array = json_array_for_key(object, key);
    let mut values = Vec::new();
    let mut rest = array.trim();
    while !rest.is_empty() {
        rest = rest.trim_start();
        if let Some(after_comma) = rest.strip_prefix(',') {
            rest = after_comma;
            continue;
        }
        let Some((value, consumed)) = parse_json_string(rest) else {
            panic!("invalid JSON string array item for key {key}: {rest:?}");
        };
        values.push(value);
        rest = &rest[consumed..];
    }
    values
}

fn json_array_for_key<'a>(object: &'a str, key: &str) -> &'a str {
    let marker = format!("\"{key}\":");
    let start = object
        .find(&marker)
        .unwrap_or_else(|| panic!("missing array key {key}"))
        + marker.len();
    let after_marker = object[start..].trim_start();
    let open = after_marker
        .find('[')
        .unwrap_or_else(|| panic!("missing array open for key {key}"));
    let array_start = open + 1;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (idx, ch) in after_marker[array_start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '[' => depth += 1,
            ']' if depth == 0 => return &after_marker[array_start..array_start + idx],
            ']' => depth -= 1,
            _ => {}
        }
    }
    panic!("unterminated array for key {key}")
}

fn split_json_objects(array: &str) -> Vec<&str> {
    let mut objects = Vec::new();
    let mut object_start = None;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (idx, ch) in array.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    object_start = Some(idx);
                }
                depth += 1;
            }
            '}' => {
                depth = depth.checked_sub(1).expect("unexpected JSON object close");
                if depth == 0 {
                    let start = object_start.take().expect("missing JSON object start");
                    objects.push(&array[start..=idx]);
                }
            }
            _ => {}
        }
    }
    objects
}

fn parse_json_string(value: &str) -> Option<(String, usize)> {
    let mut chars = value.char_indices();
    if chars.next()?.1 != '"' {
        return None;
    }
    let mut output = String::new();
    let mut escaped = false;
    for (idx, ch) in chars {
        if escaped {
            let decoded = match ch {
                '"' => '"',
                '\\' => '\\',
                '/' => '/',
                'b' => '\u{0008}',
                'f' => '\u{000c}',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                _ => return None,
            };
            output.push(decoded);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some((output, idx + ch.len_utf8()));
        } else {
            output.push(ch);
        }
    }
    None
}
