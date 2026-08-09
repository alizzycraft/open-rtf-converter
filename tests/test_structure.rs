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

fn rust_sources_below(relative_directory: &str) -> Vec<(PathBuf, String)> {
    fn collect(directory: &Path, sources: &mut Vec<(PathBuf, String)>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(&path, sources);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                sources.push((path.clone(), std::fs::read_to_string(path).unwrap()));
            }
        }
    }

    let mut sources = Vec::new();
    collect(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_directory),
        &mut sources,
    );
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    sources
}

#[test]
fn decomposed_targets_preserve_the_recorded_test_inventory() {
    assert_eq!(module_test_count("src/security_tests"), 1_044);
    assert_eq!(module_test_count("tests/conversion"), 46);
    assert_eq!(module_test_count("tests/reference"), 4);
    assert_eq!(module_test_count("src/rtf/parser_tests"), 845);
    assert_eq!(module_test_count("src/rtf/parser/media"), 5);
    assert_eq!(module_test_count("src/layout_tests"), 319);
    assert_eq!(module_test_count("src/pdf_tests"), 83);
    assert_eq!(
        test_attribute_count(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/pdf/audit_tests.rs")
        ),
        18
    );

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for relative_path in ["tests/conversion.rs", "tests/reference_fixtures.rs"] {
        assert_eq!(
            test_attribute_count(&root.join(relative_path)),
            0,
            "{relative_path} should remain an orchestration and shared-support root"
        );
    }
    assert_eq!(
        test_attribute_count(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/security.rs")),
        2
    );
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

#[test]
fn production_pdf_writer_has_one_plan_backed_resource_path() {
    let writer = production_source("src/pdf/writer.rs");

    for retired in [
        "try_render_pdf_with_font_provider",
        "collect_page_used_font_indexes",
        "collect_supplied_pdf_fonts",
        "collect_page_supplied_font_indexes",
        "collect_image_refs",
        "count_image_xobjects",
    ] {
        assert!(
            !writer.contains(retired),
            "PDF writer still contains duplicate resource path {retired}"
        );
    }
    assert!(writer.contains("fn try_render_pdf_plan("));
    assert!(writer.contains("render_pdf_plan(&plan)"));
}

#[test]
fn normal_integration_tests_use_only_the_supported_crate_facade() {
    for (path, source) in rust_sources_below("tests") {
        for module in ["rtf", "model", "layout", "pdf"] {
            let forbidden = format!("open_rtf_converter::{module}");
            assert!(
                !source.contains(&forbidden),
                "{} imports raw implementation module {module}",
                path.display()
            );
        }
    }
}

#[test]
fn fuzz_targets_use_only_explicit_non_default_hooks_and_the_facade() {
    let manifest_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let package_manifest = std::fs::read_to_string(manifest_root.join("Cargo.toml")).unwrap();
    let fuzz_manifest = std::fs::read_to_string(manifest_root.join("fuzz/Cargo.toml")).unwrap();
    assert!(package_manifest.contains("fuzzing = []"));
    assert!(!package_manifest.contains("default = [\"cli\", \"fuzzing\"]"));
    assert!(fuzz_manifest.contains("features = [\"fuzzing\"]"));

    for (path, source) in rust_sources_below("fuzz/fuzz_targets") {
        for module in ["rtf", "model", "layout", "pdf"] {
            let forbidden = format!("open_rtf_converter::{module}");
            assert!(
                !source.contains(&forbidden),
                "{} bypasses fuzz hooks through raw module {module}",
                path.display()
            );
        }
    }
}
