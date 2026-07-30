use std::collections::BTreeMap;

const MANIFEST: &str = include_str!("../Cargo.toml");

#[test]
fn core_dependencies_keep_default_features_disabled_for_wasm() {
    let dependencies = manifest_table("dependencies");
    let mut missing = Vec::new();
    for (name, spec) in dependencies {
        if !spec.contains("default-features = false") {
            missing.push(name);
        }
    }

    assert!(
        missing.is_empty(),
        "core dependencies must opt out of default features for WASM-safe builds: {missing:?}"
    );
}

#[test]
fn core_dependencies_do_not_add_known_native_or_process_surface_crates() {
    let dependencies = manifest_table("dependencies");
    let forbidden = [
        "font-kit",
        "fontconfig",
        "native-tls",
        "openssl",
        "reqwest",
        "rustls",
        "tokio",
        "ureq",
    ];
    let present = forbidden
        .into_iter()
        .filter(|name| dependencies.contains_key(*name))
        .collect::<Vec<_>>();

    assert!(
        present.is_empty(),
        "core dependencies must stay memory-only/browser-safe; move native/network crates out of core: {present:?}"
    );
}

fn manifest_table(name: &str) -> BTreeMap<String, String> {
    let mut entries = BTreeMap::new();
    let mut in_table = false;
    let mut current_name: Option<String> = None;
    let mut current_spec = String::new();

    for raw_line in MANIFEST.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') {
            flush_entry(&mut entries, &mut current_name, &mut current_spec);
            in_table = line == format!("[{name}]");
            continue;
        }
        if !in_table || line.is_empty() || line.starts_with('#') {
            continue;
        }

        if current_name.is_some() {
            current_spec.push(' ');
            current_spec.push_str(line);
            if line.ends_with('}') || line.ends_with(']') {
                flush_entry(&mut entries, &mut current_name, &mut current_spec);
            }
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_string();
        let value = value.trim();
        if value.starts_with('{') && !value.ends_with('}') {
            current_name = Some(key);
            current_spec.push_str(value);
        } else {
            entries.insert(key, value.to_string());
        }
    }

    flush_entry(&mut entries, &mut current_name, &mut current_spec);
    entries
}

fn flush_entry(
    entries: &mut BTreeMap<String, String>,
    current_name: &mut Option<String>,
    current_spec: &mut String,
) {
    if let Some(name) = current_name.take() {
        entries.insert(name, std::mem::take(current_spec));
    }
}
