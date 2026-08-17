//! W1 acceptance at the toolchain seam: `--target rust` emission is
//! deterministic, fail-closed, and the emitted crate compiles.

use braid_project::{build_rust, parse_project};

const MANIFEST: &str =
    r#"{ "name": "demo", "capsules": [ { "name": "greeting", "source": "\"hi\" + \"!\"" } ] }"#;

#[test]
fn rust_target_emits_one_crate_per_capsule() {
    let project = parse_project(MANIFEST).unwrap();
    let crates = build_rust(&project).unwrap();
    assert_eq!(crates.len(), 1);
    assert_eq!(crates[0].0, "greeting");
}

#[test]
fn rust_target_is_deterministic() {
    let a = build_rust(&parse_project(MANIFEST).unwrap()).unwrap();
    let b = build_rust(&parse_project(MANIFEST).unwrap()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn emitted_crate_compiles() {
    let crates = build_rust(&parse_project(MANIFEST).unwrap()).unwrap();
    let dir = std::env::temp_dir().join(format!("braid-project-rust-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("lib.rs");
    std::fs::write(&path, &crates[0].1.lib_rs).unwrap();
    let out = std::process::Command::new("rustc")
        .args(["--crate-type", "lib", "--edition", "2021"])
        .arg(&path)
        .output()
        .expect("rustc must be on PATH");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.status.success(),
        "emitted lib.rs failed to compile:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn rejected_source_fails_the_whole_rust_build() {
    // "1" + "2" elaborates fine; a bad source fails build_rust entirely,
    // not partially.
    let bad = r#"{ "name": "demo", "capsules": [ { "name": "ok", "source": "\"a\" + \"b\"" }, { "name": "broken", "source": "1 + " } ] }"#;
    assert!(build_rust(&parse_project(bad).unwrap()).is_err());
}
