//! T15 — the extraction covenant, machine-enforced (ADR-088 D3/D5): braid
//! crates depend ONLY on the declared kernel contracts. The pattern is the
//! kernel's `test_module_boundary_contract.rs`: a structural scan, so the
//! boundary cannot erode silently.

use std::fs;
use std::path::{Path, PathBuf};

fn crates_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

const BRAID_CRATES: &[&str] = &["braid-ir", "braid-verify", "braid-render", "braid-sdk"];

/// Runtime dependency allowlist per crate (Cargo.toml `[dependencies]`).
/// `braid-capability` is the vendored kernel capability contract — the single
/// type that crosses the D3 boundary (see crates/braid-capability).
fn allowed_deps(krate: &str) -> &'static [&'static str] {
    match krate {
        "braid-ir" => &["blake3", "braid-capability"],
        "braid-verify" => &["blake3", "braid-ir", "braid-capability"],
        "braid-render" => &["braid-ir", "braid-capability"],
        "braid-sdk" => &["braid-ir", "braid-capability"],
        _ => unreachable!(),
    }
}

/// Dev-dependency allowlist (tests only; never linked into the artifact).
/// `braid-vocab-cms` is a vocabulary package used by the substrate's tests
/// as a concrete registry to exercise the codec/registry. `serde_json` is
/// test-only parsing of the pre-validated RFC 8949 / BLAKE3 calibration
/// corpora (D-FLIGHT) — never linked into the artifact.
const ALLOWED_DEV: &[&str] = &[
    "lgwks_std",
    "proptest",
    "serde_json",
    "braid-ir",
    "braid-render",
    "braid-verify",
    "braid-vocab-cms",
];

/// First path segment allowlist for `use` statements in src/**.
const ALLOWED_USE_ROOTS: &[&str] = &[
    "crate",
    "super",
    "self",
    "std",
    "core",
    "alloc",
    "blake3",
    "braid_capability",
    "braid_ir",
];

fn parse_section_header(line: &str, section: &str) -> Option<bool> {
    if line.starts_with('[') {
        Some(line == format!("[{section}]"))
    } else {
        None
    }
}

fn parse_key_value(line: &str) -> Option<String> {
    if !line.is_empty() && !line.starts_with('#') {
        line.split_once('=').map(|(key, _)| key.trim().to_string())
    } else {
        None
    }
}

fn scan_toml_lines(toml: &str, section: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut in_section = false;
    for line in toml.lines() {
        let line_text = line.trim();
        if let Some(matches_section) = parse_section_header(line_text, section) {
            in_section = matches_section;
            continue;
        }
        if in_section && let Some(key_name) = parse_key_value(line_text) {
            keys.push(key_name);
        }
    }
    keys
}

fn toml_section_keys(toml: &str, section: &str) -> Vec<String> {
    scan_toml_lines(toml, section)
}

fn check_crate_runtime_dependencies(krate: &str, manifest: &str) {
    for dep in toml_section_keys(manifest, "dependencies") {
        assert!(
            allowed_deps(krate).contains(&dep.as_str()),
            "{krate}: runtime dependency `{dep}` is outside the ADR-088 D3 boundary — \
             extend the spec (Director) before extending the deps"
        );
    }
}

fn check_crate_dev_dependencies(krate: &str, manifest: &str) {
    for dep in toml_section_keys(manifest, "dev-dependencies") {
        assert!(
            ALLOWED_DEV.contains(&dep.as_str()),
            "{krate}: dev-dependency `{dep}` is not allowlisted"
        );
    }
}

#[test]
fn cargo_dependencies_are_allowlisted() {
    for krate in BRAID_CRATES {
        let manifest = fs::read_to_string(crates_dir().join(krate).join("Cargo.toml"))
            .unwrap_or_else(|_| panic!("{krate}/Cargo.toml readable"));
        check_crate_runtime_dependencies(krate, &manifest);
        check_crate_dev_dependencies(krate, &manifest);
    }
}

fn visit_fs_entry(entry_path: PathBuf, out: &mut Vec<PathBuf>) {
    if entry_path.is_dir() {
        rust_files(&entry_path, out);
    } else if entry_path.extension().is_some_and(|ext| ext == "rs") {
        out.push(entry_path);
    }
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            visit_fs_entry(entry.path(), out);
        }
    }
}

fn extract_use_root(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let rest = trimmed
        .strip_prefix("use ")
        .or_else(|| trimmed.strip_prefix("pub use "))?;
    let root: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    Some(root)
}

fn check_single_use_statement(file_path: &Path, line_num: usize, line: &str) {
    if let Some(root) = extract_use_root(line) {
        let type_relative = root.chars().next().is_some_and(|c| c.is_uppercase());
        assert!(
            type_relative || ALLOWED_USE_ROOTS.contains(&root.as_str()),
            "{}:{}: `use {root}…` crosses the D3 boundary",
            file_path.display(),
            line_num + 1
        );
    }
}

#[test]
fn src_use_statements_stay_inside_the_boundary() {
    for krate in BRAID_CRATES {
        let mut files = Vec::new();
        rust_files(&crates_dir().join(krate).join("src"), &mut files);
        assert!(!files.is_empty(), "{krate}/src has sources");
        for file_path in files {
            let src = fs::read_to_string(&file_path).unwrap();
            for (line_num, line) in src.lines().enumerate() {
                check_single_use_statement(&file_path, line_num, line);
            }
        }
    }
}
