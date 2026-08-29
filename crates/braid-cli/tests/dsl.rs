//! Black-box Braid DSL journey: source -> canonical bytes -> independent
//! admission, with JSON-of-IR parity and widening visibility.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn braid() -> Command {
    Command::new(env!("CARGO_BIN_EXE_braid"))
}

fn dsl_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../braid-elaborate-dsl/tests/fixtures")
        .join(name)
}

fn tmp(name: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(name)
}

fn run(args: &[&str]) -> Output {
    braid().args(args).output().expect("braid binary runs")
}

fn compile_and_compare(name: &str, stem: &str, expected_cid: &str) -> PathBuf {
    let source = dsl_fixture(name);
    let dsl_bytes = tmp(&format!("{stem}.braid"));
    let json = tmp(&format!("{stem}.json"));
    let json_bytes = tmp(&format!("{stem}-json.braid"));
    let output = run(&[
        "dsl",
        "compile",
        source.to_str().unwrap(),
        "-o",
        dsl_bytes.to_str().unwrap(),
        "--emit-json",
        json.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "DSL compile failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(expected_cid),
        "compile receipt did not contain pinned CID"
    );

    let encoded = run(&[
        "encode",
        json.to_str().unwrap(),
        "-o",
        json_bytes.to_str().unwrap(),
    ]);
    assert!(
        encoded.status.success(),
        "JSON parity encode failed: {}",
        String::from_utf8_lossy(&encoded.stderr)
    );
    assert_eq!(
        std::fs::read(&dsl_bytes).unwrap(),
        std::fs::read(&json_bytes).unwrap(),
        "DSL and JSON-of-IR paths must emit identical canonical bytes"
    );
    dsl_bytes
}

#[test]
fn three_demo_port_sources_match_json_transport_and_pinned_cids() {
    let cases = [
        (
            "edit-home-hero.brd",
            "dsl-edit",
            "26f6162e1a5fb5f6e3a46724a991a8b4dc48e08c223016fb86c3c8de38594226",
        ),
        (
            "publish-services.brd",
            "dsl-publish",
            "195a7455e56b42dde32a79218c1b675420bc2de310184d16413a027d6261f33a",
        ),
        (
            "render-work-listing.brd",
            "dsl-listing",
            "43ed11f1d863a214ca4ac51bbfaa5078166ddcc228fda717c25fa358bf8da3a6",
        ),
    ];
    for (name, stem, cid) in cases {
        let artifact = compile_and_compare(name, stem, cid);
        let verified = run(&["verify", artifact.to_str().unwrap()]);
        assert!(verified.status.success());
        assert!(String::from_utf8_lossy(&verified.stdout).contains("ADMIT"));
    }
}

#[test]
fn source_capability_change_is_a_visible_widening() {
    let edit = compile_and_compare(
        "edit-home-hero.brd",
        "dsl-diff-edit",
        "26f6162e1a5fb5f6e3a46724a991a8b4dc48e08c223016fb86c3c8de38594226",
    );
    let publish = compile_and_compare(
        "publish-services.brd",
        "dsl-diff-publish",
        "195a7455e56b42dde32a79218c1b675420bc2de310184d16413a027d6261f33a",
    );
    let diff = run(&["diff", edit.to_str().unwrap(), publish.to_str().unwrap()]);
    assert_eq!(diff.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&diff.stdout);
    assert!(stdout.contains("WIDENING"), "got: {stdout}");
    assert!(stdout.contains("intent.emit"), "got: {stdout}");
}

#[test]
fn failed_source_writes_no_artifact() {
    let source_path = tmp("dsl-unconfirmed.brd");
    let artifact_path = tmp("dsl-unconfirmed.braid");
    let source = std::fs::read_to_string(dsl_fixture("publish-services.brd"))
        .unwrap()
        .replace("confirm human", "confirm none");
    std::fs::write(&source_path, source).unwrap();
    drop(std::fs::remove_file(&artifact_path));

    let output = run(&[
        "dsl",
        "compile",
        source_path.to_str().unwrap(),
        "-o",
        artifact_path.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(!artifact_path.exists(), "failed compile wrote an artifact");
    assert!(String::from_utf8_lossy(&output.stderr).contains("BRD014_BUILD_REJECTED"));
}

#[test]
fn public_help_exposes_the_dsl_entrypoint() {
    let output = run(&["--help"]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("braid dsl compile"));
}
