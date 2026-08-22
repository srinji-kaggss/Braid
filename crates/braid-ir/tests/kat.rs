//! Known-answer tests (D8): the canonical encoding has a pinned answer from
//! the first byte it ever produced. A KAT failure means the byte form moved —
//! that is an IR_VERSION event, never a test to "update and move on".
//!
//! Vector file: `spec/braid/vectors/capsule_v0.kat` (the extraction-ready
//! spec home owns the vectors; both this crate and braid-verify consume them).

use braid_ir::{Capsule, IR_VERSION};
use braid_vocab_cms::{edit_section_capsule, registry_v0};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

fn vectors_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/braid/vectors/capsule_v0.kat")
}

fn read_vectors() -> BTreeMap<String, String> {
    let raw = fs::read_to_string(vectors_path()).expect(
        "spec/braid/vectors/capsule_v0.kat missing — vectors are part of the spec, regenerate ONLY on a conscious IR_VERSION bump",
    );
    raw.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let (k, v) = l.split_once('=').expect("kat line is key=value");
            (k.trim().to_string(), v.trim().to_string())
        })
        .collect()
}

#[test]
fn ir_version_is_pinned() {
    // A bump must be conscious (D11): change this assertion only together
    // with new vectors and a register entry.
    assert_eq!(IR_VERSION, 0);
}

#[test]
fn registry_v0_cid_known_answer() {
    let v = read_vectors();
    assert_eq!(
        registry_v0().cid().to_hex(),
        v["registry_cid_hex"],
        "registry canonical bytes moved — vocabulary/IR version event"
    );
}

#[test]
fn capsule_bytes_known_answer() {
    let v = read_vectors();
    let capsule = edit_section_capsule();
    assert_eq!(
        lgwks_std::hex::encode(capsule.to_bytes()),
        v["capsule_bytes_hex"],
        "capsule canonical bytes moved — IR_VERSION event"
    );
}

#[test]
fn capsule_cid_known_answer() {
    let v = read_vectors();
    let capsule = edit_section_capsule();
    assert_eq!(capsule.cid().to_hex(), v["capsule_cid_hex"]);
}

#[test]
fn capsule_round_trips_from_pinned_bytes() {
    let v = read_vectors();
    let bytes = lgwks_std::hex::decode(&v["capsule_bytes_hex"]).unwrap();
    let parsed = Capsule::from_bytes(&bytes).expect("pinned bytes are canonical");
    assert_eq!(parsed, edit_section_capsule());
}

/// Generator: `cargo test -p braid-ir --test kat -- --ignored --nocapture`
/// prints the current values. Used ONCE per conscious version bump.
#[test]
#[ignore]
fn print_kat_values() {
    let capsule = edit_section_capsule();
    println!("# Braid v0 known-answer vectors — ADR-088 D8.");
    println!("# Regenerating these is an IR_VERSION event (D11), never routine.");
    println!("registry_cid_hex = {}", registry_v0().cid().to_hex());
    println!("capsule_bytes_hex = {}", lgwks_std::hex::encode(capsule.to_bytes()));
    println!("capsule_cid_hex = {}", capsule.cid().to_hex());
}
