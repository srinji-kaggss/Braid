//! T15 — the extraction covenant, machine-enforced (ADR-088 D3/D5): braid
//! crates depend ONLY on the declared kernel contracts. The pattern is the
//! kernel's `test_module_boundary_contract.rs`: a structural scan, so the
//! boundary cannot erode silently.

use std::fs;
use std::path::{Path, PathBuf};

fn crates_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

const BRAID_CRATES: &[&str] = &["braid-ir", "braid-verify", "braid-render"];

/// Runtime dependency allowlist per crate (Cargo.toml `[dependencies]`).
/// `braid-capability` is the vendored kernel capability contract — the single
/// type that crosses the D3 boundary (see crates/braid-capability).
fn allowed_deps(krate: &str) -> &'static [&'static str] {
    match krate {
        "braid-ir" => &["blake3", "braid-capability"],
        "braid-verify" => &["blake3", "braid-ir", "braid-capability"],
        "braid-render" => &["braid-ir", "braid-capability"],
        _ => unreachable!(),
    }
}

/// Dev-dependency allowlist (tests only; never linked into the artifact).
/// `braid-vocab-cms` is a vocabulary package used by the substrate's tests
/// as a concrete registry to exercise the codec/registry — it is NOT a
/// runtime dep of the substrate (D31: vocabularies are consumer-side).
const ALLOWED_DEV: &[&str] = &[
    "hex",
    "proptest",
    "braid-ir",
    "braid-render",
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

fn toml_section_keys(toml: &str, section: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut in_section = false;
    for line in toml.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_section = l == format!("[{section}]");
            continue;
        }
        if in_section && !l.is_empty() && !l.starts_with('#') {
            if let Some((k, _)) = l.split_once('=') {
                keys.push(k.trim().to_string());
            }
        }
    }
    keys
}

#[test]
fn cargo_dependencies_are_allowlisted() {
    for krate in BRAID_CRATES {
        let manifest = fs::read_to_string(crates_dir().join(krate).join("Cargo.toml"))
            .unwrap_or_else(|_| panic!("{krate}/Cargo.toml readable"));
        for dep in toml_section_keys(&manifest, "dependencies") {
            assert!(
                allowed_deps(krate).contains(&dep.as_str()),
                "{krate}: runtime dependency `{dep}` is outside the ADR-088 D3 boundary — \
                 extend the spec (Director) before extending the deps"
            );
        }
        for dep in toml_section_keys(&manifest, "dev-dependencies") {
            assert!(
                ALLOWED_DEV.contains(&dep.as_str()),
                "{krate}: dev-dependency `{dep}` is not allowlisted"
            );
        }
    }
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                rust_files(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
}

#[test]
fn src_use_statements_stay_inside_the_boundary() {
    for krate in BRAID_CRATES {
        let mut files = Vec::new();
        rust_files(&crates_dir().join(krate).join("src"), &mut files);
        assert!(!files.is_empty(), "{krate}/src has sources");
        for f in files {
            let src = fs::read_to_string(&f).unwrap();
            for (ln, line) in src.lines().enumerate() {
                let t = line.trim();
                let Some(rest) = t
                    .strip_prefix("use ")
                    .or_else(|| t.strip_prefix("pub use "))
                else {
                    continue;
                };
                let root: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                // Uppercase roots are type-relative paths (`use EffectClass::*`
                // inside a fn) — already covered by the import that brought the
                // type in. Crate roots are snake_case by Rust convention.
                let type_relative = root.chars().next().is_some_and(|c| c.is_uppercase());
                assert!(
                    type_relative || ALLOWED_USE_ROOTS.contains(&root.as_str()),
                    "{}:{}: `use {root}…` crosses the D3 boundary",
                    f.display(),
                    ln + 1
                );
            }
        }
    }
}
