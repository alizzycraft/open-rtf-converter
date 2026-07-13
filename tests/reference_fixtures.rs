use std::path::Path;

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
}
