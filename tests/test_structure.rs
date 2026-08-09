use std::path::{Path, PathBuf};

fn test_attribute_count(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
        .lines()
        .filter(|line| line.trim() == "#[test]")
        .count()
}

fn module_test_count(relative_directory: &str) -> usize {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_directory);
    let mut files = std::fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect::<Vec<_>>();
    files.sort();
    files.iter().map(|path| test_attribute_count(path)).sum()
}

fn production_source(relative_path: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    source
        .split_once("#[cfg(test)]")
        .map_or(source.as_str(), |(production, _)| production)
        .to_owned()
}

#[test]
fn decomposed_targets_preserve_the_recorded_test_inventory() {
    assert_eq!(module_test_count("tests/security"), 1_044);
    assert_eq!(module_test_count("tests/conversion"), 46);
    assert_eq!(module_test_count("tests/reference"), 4);
    assert_eq!(module_test_count("src/rtf/parser_tests"), 845);
    assert_eq!(module_test_count("src/rtf/parser/media"), 5);
    assert_eq!(module_test_count("src/layout_tests"), 319);
    assert_eq!(module_test_count("src/pdf_tests"), 80);
    assert_eq!(
        test_attribute_count(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/pdf/audit_tests.rs")
        ),
        18
    );

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for relative_path in [
        "tests/security.rs",
        "tests/conversion.rs",
        "tests/reference_fixtures.rs",
    ] {
        assert_eq!(
            test_attribute_count(&root.join(relative_path)),
            0,
            "{relative_path} should remain an orchestration and shared-support root"
        );
    }
}

#[test]
fn production_model_pipeline_has_no_semantic_sentinel_protocols() {
    let source = [
        production_source("src/model.rs"),
        production_source("src/rtf/parser.rs"),
        production_source("src/rtf/parser/fields/formatting.rs"),
        production_source("src/rtf/parser/fields/formula.rs"),
        production_source("src/layout.rs"),
        production_source("src/layout/late_fields.rs"),
    ]
    .join("\n");

    for forbidden in [
        "BOOKMARK_PAGE_ANCHOR_MARKER",
        "BOOKMARK_PAGE_REF_MARKER",
        "BOOKMARK_PAGE_MARKER_END",
        "PASSIVE_ADVANCE_MARKER",
        "NESTED_TABLE_ANCHOR_MARKER",
        "INLINE_IMAGE_MARKER_BASE",
        "FOOTNOTE_REFERENCE_MARKER",
        "ENDNOTE_REFERENCE_MARKER",
        "PENDING_NOTE_REFERENCE_MARKER",
        "PASSIVE_MATH_FRACTION_RULE_MARKER",
        "PASSIVE_MATH_STACK_ANCHOR_MARKER",
        "PASSIVE_MATH_INLINE_LEAD_MARKER",
        "contains_internal_marker",
        "sanitize_internal_markers",
        "remove_internal_stat_markers",
    ] {
        assert!(
            !source.contains(forbidden),
            "production source still contains obsolete sentinel protocol {forbidden}"
        );
    }
}
