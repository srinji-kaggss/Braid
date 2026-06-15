//! U8 — the Day-0 CMS reference workflow (D16 "landing page as first full
//! port"), buildable slice: the **author → admit → render** legs of three
//! demo-port CMS actions (modeled on the kernel's `blueprints/afternow-port/`
//! landing surface), driven through the human-reconstructable CLI binary
//! exactly like a human or CI would.
//!
//! What this proves now (no kernel runtime required):
//!   - the real landing-surface verbs (`cms.edit_section`, `cms.publish`,
//!     `proj.listing`) admit and render with the expected effect/capability
//!     posture;
//!   - the irreversible publish is admissible only with a confirm policy, and a
//!     publish authored WITHOUT one is refused at author time (fail-closed);
//!   - each capsule's CID is pinned, so any drift in the authoring/canonical
//!     path turns this RED (T13 / scenario #13 discipline).
//!
//! What is DEFERRED behind the U7/kernel-WASM seam (NOT exercised here, tracked
//! in #6): actual execution, on-tape fact journaling, and scenario #3's
//! *runtime* confirmation-hash-mismatch reject. The seam is: capsule CID →
//! kernel runtime load → manifest re-derivation (refuse-on-mismatch, T4) + fact
//! journal.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn braid() -> Command {
    Command::new(env!("CARGO_BIN_EXE_braid"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/demo-port")
        .join(name)
}

fn tmp(name: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(name)
}

fn run(args: &[&str]) -> Output {
    braid().args(args).output().expect("braid binary runs")
}

/// `encode <fixture> -o <out>` → (out path, CID printed as `cid <hex>` on stderr).
fn encode(fixture_name: &str, out_name: &str) -> (PathBuf, String) {
    let src = fixture(fixture_name);
    let out = tmp(out_name);
    let o = run(&["encode", src.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert!(
        o.status.success(),
        "encode {fixture_name} failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let cid = String::from_utf8_lossy(&o.stderr)
        .lines()
        .find_map(|l| l.strip_prefix("cid ").map(|s| s.trim().to_string()))
        .expect("encode prints `cid <hex>`");
    (out, cid)
}

/// Pinned CIDs for the three admitted demo-port actions. These are the
/// content-addresses of the canonical bytes; a change here means the authoring
/// or encoding path moved — investigate before re-pinning.
const CID_EDIT_HOME_HERO: &str = "e160ad02e081e856f18815957246ff224af333bd3dcc44411609dd8ad1227ac2";
const CID_PUBLISH_SERVICES: &str =
    "5bb1c3b90f78ad832c628617bfe4d0079b395458400feaf1ec430e0abb65fbf3";
const CID_RENDER_WORK_LISTING: &str =
    "3e9816f1fa77e75ada2b25bcbba82bc0c31248edf8198d89944dc7e882f15f4a";

/// Scenario #1 — AI authors an "edit page section" capsule (reversible, local):
/// admitted; the manifest shows no egress and no irreversible effect.
#[test]
fn edit_home_hero_admits_reversible_local() {
    let (out, cid) = encode("edit-home-hero.json", "dp_edit.braid");
    assert_eq!(cid, CID_EDIT_HOME_HERO, "edit-home-hero CID drifted");

    let v = run(&["verify", out.to_str().unwrap()]);
    assert!(v.status.success(), "edit-home-hero must ADMIT");
    let vout = String::from_utf8_lossy(&v.stdout);
    assert!(vout.contains("ADMIT") && vout.contains(&cid), "got: {vout}");

    let m = String::from_utf8_lossy(&run(&["render", out.to_str().unwrap()]).stdout).into_owned();
    assert!(
        m.contains("irreversible_strands: 0"),
        "no irreversible: {m}"
    );
    assert!(m.contains("egress_strands: 0"), "no egress: {m}");
    assert!(
        m.contains("capabilities: signal.emit"),
        "reversible-write cap only: {m}"
    );
    assert!(m.contains("confirm: none"), "no confirmation needed: {m}");
}

/// The real publish action: edit + `cms.publish` (irreversible) with a human
/// confirm policy is admissible (the confirm-present path of scenarios #2/#3).
/// The manifest surfaces the irreversible strand and the escalated grant.
#[test]
fn publish_services_admits_with_confirm() {
    let (out, cid) = encode("publish-services.json", "dp_pub.braid");
    assert_eq!(cid, CID_PUBLISH_SERVICES, "publish-services CID drifted");

    let v = run(&["verify", out.to_str().unwrap()]);
    assert!(
        v.status.success(),
        "publish-services (human-confirm) must ADMIT"
    );
    assert!(String::from_utf8_lossy(&v.stdout).contains("ADMIT"));

    let m = String::from_utf8_lossy(&run(&["render", out.to_str().unwrap()]).stdout).into_owned();
    assert!(
        m.contains("irreversible_strands: 1"),
        "publish is irreversible: {m}"
    );
    assert!(
        m.contains("intent.emit") && m.contains("signal.emit"),
        "escalated grants: {m}"
    );
    assert!(
        m.contains("confirm: human-confirm"),
        "confirm declared: {m}"
    );
}

/// A pure projection read of the work case-study listing: admitted, read-only —
/// no writes, no irreversible, no egress.
#[test]
fn render_work_listing_admits_read_only() {
    let (out, cid) = encode("render-work-listing.json", "dp_list.braid");
    assert_eq!(
        cid, CID_RENDER_WORK_LISTING,
        "render-work-listing CID drifted"
    );

    let v = run(&["verify", out.to_str().unwrap()]);
    assert!(v.status.success(), "render-work-listing must ADMIT");

    let m = String::from_utf8_lossy(&run(&["render", out.to_str().unwrap()]).stdout).into_owned();
    assert!(
        m.contains("capabilities: tape.read"),
        "projection read cap: {m}"
    );
    assert!(
        m.contains("irreversible_strands: 0") && m.contains("egress_strands: 0"),
        "{m}"
    );
    // The effect set is exactly a pure projection read — no write/publish effect.
    assert!(
        m.contains("effects: pure, read"),
        "read-only effect set: {m}"
    );
}

/// The escalation probe: publishing WITHOUT a confirm policy is an irreversible
/// effect with no human in the loop — refused at AUTHOR time (operator error,
/// exit 2), never emitted as a capsule the verifier would later reject.
#[test]
fn publish_without_confirm_is_refused_at_author_time() {
    let src = fixture("publish-services-noconfirm.json");
    let o = run(&[
        "encode",
        src.to_str().unwrap(),
        "-o",
        tmp("dp_nc.braid").to_str().unwrap(),
    ]);
    assert_eq!(
        o.status.code(),
        Some(2),
        "must be a fail-closed author refusal, not a silent emit"
    );
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("ConfirmRequired"),
        "got: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}
