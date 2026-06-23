//! U6 acceptance — the CLI-only loop (scenario #12, threats T12/T13).
//!
//! These drive the REAL binary (`CARGO_BIN_EXE_braid`) so the contract under
//! test includes exit codes — the bit CI and shell `&&` chains depend on:
//!   0 = ok · 1 = policy-negative (Reject / Widening) · 2 = operator error.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn braid() -> Command {
    Command::new(env!("CARGO_BIN_EXE_braid"))
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn tmp(name: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(name)
}

fn run(args: &[&str]) -> Output {
    braid().args(args).output().expect("braid binary runs")
}

/// `encode <fixture> -o <out>` and return the out path + the CID printed to
/// stderr (`cid <hex>`).
fn encode(fixture: &str, out_name: &str) -> (PathBuf, String) {
    let src = fixtures().join(fixture);
    let out = tmp(out_name);
    let o = run(&["encode", src.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert!(
        o.status.success(),
        "encode {fixture} failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let stderr = String::from_utf8_lossy(&o.stderr);
    let cid = stderr
        .lines()
        .find_map(|l| l.strip_prefix("cid "))
        .expect("encode prints `cid <hex>`")
        .trim()
        .to_string();
    (out, cid)
}

/// T13 — the CLI authoring path is byte-identical to the SDK path: encoding the
/// edit-section fixture reproduces the PINNED reference CID (the same KAT the
/// braid-ir/braid-sdk paths pin). The CLI is not a second, drifting authoring
/// surface.
#[test]
fn encode_reproduces_pinned_reference_cid() {
    let (_, cid) = encode("edit_section.json", "edit_kat.braid");
    assert_eq!(
        cid, "ccedc469e6b0513720969ce1a4f169f53365eeadbc853042c411b44c1f15b71f",
        "CLI-authored edit_section CID drifted from the pinned KAT — CLI path != SDK path"
    );
}

/// Scenario #12 — a human, no AI, CLI only: author -> verify -> render, same
/// verdict and artifact as the AI/SDK path.
#[test]
fn cli_only_loop_admits_and_renders() {
    let (out, cid) = encode("edit_section.json", "loop.braid");

    let v = run(&["verify", out.to_str().unwrap()]);
    assert!(v.status.success(), "verify should ADMIT");
    let vout = String::from_utf8_lossy(&v.stdout);
    assert!(vout.contains("ADMIT"), "got: {vout}");
    assert!(vout.contains(&cid), "verdict binds the same CID");

    let r = run(&["render", out.to_str().unwrap()]);
    assert!(r.status.success());
    let manifest = String::from_utf8_lossy(&r.stdout);
    assert!(
        manifest.contains(&cid),
        "manifest is bound to the capsule CID"
    );
    assert!(manifest.contains("capabilities: signal.emit"));
}

/// `decode` is the inverse of `encode`: decoding then re-encoding reproduces
/// byte-identical canonical bytes (same CID).
#[test]
fn decode_round_trips_to_identical_bytes() {
    let (out, cid) = encode("edit_section.json", "rt.braid");

    let d = run(&["decode", out.to_str().unwrap()]);
    assert!(d.status.success());
    let json_path = tmp("rt_decoded.json");
    std::fs::write(&json_path, &d.stdout).unwrap();

    let re = run(&[
        "encode",
        json_path.to_str().unwrap(),
        "-o",
        tmp("rt2.braid").to_str().unwrap(),
    ]);
    assert!(re.status.success(), "re-encode of decoded JSON failed");
    let cid2 = String::from_utf8_lossy(&re.stderr)
        .lines()
        .find_map(|l| l.strip_prefix("cid ").map(|s| s.trim().to_string()))
        .unwrap();
    assert_eq!(
        cid, cid2,
        "encode∘decode is not identity-preserving on the CID"
    );
}

/// T12 — the manifest-widening gate. A capsule that grows authority/effect is
/// flagged WIDENING and the command exits non-zero (the CI gate's one bit).
///
/// MUTATION: this is the red-team test. Make `braid_render::has_widening`
/// always return false (or reclassify Widening as Neutral) and this goes RED —
/// the gate has teeth.
#[test]
fn diff_flags_widening_and_exits_nonzero() {
    let (base, _) = encode("edit_section.json", "w_base.braid");
    let (wide, _) = encode("edit_section_widened.json", "w_wide.braid");

    let d = run(&["diff", base.to_str().unwrap(), wide.to_str().unwrap()]);
    assert_eq!(
        d.status.code(),
        Some(1),
        "a widening PR must fail the gate (exit 1)"
    );
    let out = String::from_utf8_lossy(&d.stdout);
    assert!(out.contains("WIDENING"), "got: {out}");
    assert!(
        out.contains("+tape.read") && out.contains("+read"),
        "got: {out}"
    );
}

/// The reverse diff is a pure narrowing — allowed, exit 0.
#[test]
fn diff_narrowing_passes() {
    let (base, _) = encode("edit_section.json", "n_base.braid");
    let (wide, _) = encode("edit_section_widened.json", "n_wide.braid");

    let d = run(&["diff", wide.to_str().unwrap(), base.to_str().unwrap()]);
    assert!(
        d.status.success(),
        "a narrowing must pass the gate (exit 0)"
    );
    let out = String::from_utf8_lossy(&d.stdout);
    assert!(
        out.contains("narrowing") && !out.contains("WIDENING"),
        "got: {out}"
    );
}

/// Identical capsules => no change, exit 0.
#[test]
fn diff_identical_is_no_change() {
    let (base, _) = encode("edit_section.json", "id.braid");
    let d = run(&["diff", base.to_str().unwrap(), base.to_str().unwrap()]);
    assert!(d.status.success());
    assert!(String::from_utf8_lossy(&d.stdout).contains("no change"));
}

/// T12/U9 — `no change` must mean the same admitted artifact, not merely "no
/// new capability/effect." A neutral evidence-policy change should pass the
/// widening gate but still produce an explicit diff.
#[test]
fn diff_neutral_artifact_change_is_not_no_change() {
    let (base, _) = encode("edit_section.json", "neutral_base.braid");
    let mut capsule = braid_ir::Capsule::from_bytes(&std::fs::read(&base).unwrap()).unwrap();
    capsule.evidence.push("audit.extra".into());
    let changed = tmp("neutral_changed.braid");
    std::fs::write(&changed, capsule.to_bytes()).unwrap();

    let d = run(&["diff", base.to_str().unwrap(), changed.to_str().unwrap()]);
    assert!(
        d.status.success(),
        "neutral changes pass the widening gate (exit 0)"
    );
    let out = String::from_utf8_lossy(&d.stdout);
    assert!(!out.contains("no change"), "got: {out}");
    assert!(
        out.contains("neutral") && out.contains("capsule") && out.contains("evidence"),
        "got: {out}"
    );
}

/// U9/T4/T6 — rendering is a human-review projection of an ADMITTED artifact,
/// not a side door around the verifier. A canonical capsule with a stale
/// registry pin must be refused before any manifest text is emitted.
#[test]
fn render_refuses_version_skewed_capsule() {
    let (base, _) = encode("edit_section.json", "render_bad_version_base.braid");
    let mut capsule = braid_ir::Capsule::from_bytes(&std::fs::read(&base).unwrap()).unwrap();
    capsule.vocab_version += 1;
    let bad = tmp("render_bad_version.braid");
    std::fs::write(&bad, capsule.to_bytes()).unwrap();

    let r = run(&["render", bad.to_str().unwrap()]);
    assert_eq!(
        r.status.code(),
        Some(2),
        "render must refuse verifier-rejected capsules"
    );
    assert!(r.stdout.is_empty(), "must not emit a manifest");
    let err = String::from_utf8_lossy(&r.stderr);
    assert!(
        err.contains("not admitted for review") && err.contains("VersionPin"),
        "got: {err}"
    );
}

/// Same fail-closed rule for the manifest-diff gate: an invalid new artifact
/// must not be classed as "no change" or "neutral" just because its renderable
/// fields happen to look harmless.
#[test]
fn diff_refuses_version_skewed_capsule() {
    let (base, _) = encode("edit_section.json", "diff_bad_version_base.braid");
    let mut capsule = braid_ir::Capsule::from_bytes(&std::fs::read(&base).unwrap()).unwrap();
    capsule.registry_cid = braid_ir::Cid([0u8; 32]);
    let bad = tmp("diff_bad_version.braid");
    std::fs::write(&bad, capsule.to_bytes()).unwrap();

    let d = run(&["diff", base.to_str().unwrap(), bad.to_str().unwrap()]);
    assert_eq!(
        d.status.code(),
        Some(2),
        "diff must refuse verifier-rejected capsules"
    );
    assert!(
        !String::from_utf8_lossy(&d.stdout).contains("no change"),
        "invalid artifact must not be presented as an ordinary diff"
    );
    let err = String::from_utf8_lossy(&d.stderr);
    assert!(
        err.contains("not admitted for review") && err.contains("VersionPin"),
        "got: {err}"
    );
}

/// The laundering capsule (vault bytes -> pure hops -> egress) is rejected at
/// the taint stage; verify exits 1 (policy-negative), not 2 (operator error).
#[test]
fn verify_rejects_laundering_with_exit_one() {
    let (out, _) = encode("laundering.json", "laundry.braid");
    let v = run(&["verify", out.to_str().unwrap()]);
    assert_eq!(v.status.code(), Some(1), "laundering must REJECT (exit 1)");
    let vout = String::from_utf8_lossy(&v.stdout);
    assert!(
        vout.contains("REJECT") && vout.contains("Taint"),
        "got: {vout}"
    );
}

/// Attenuation: a principal lacking a required grant is rejected at the
/// capability stage (exit 1) even though the capsule is internally valid.
#[test]
fn verify_rejects_when_ambient_too_narrow() {
    let (out, _) = encode("publish.json", "att.braid");
    // publish needs intent.emit + signal.emit; hand the principal only one.
    let v = run(&["verify", out.to_str().unwrap(), "--grant", "signal.emit"]);
    assert_eq!(v.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&v.stdout).contains("Capability"));
}

/// A dangerous capsule authored without a confirm policy is refused at author
/// time (operator error, exit 2) — fail-closed, never an emitted capsule the
/// verifier would later reject.
#[test]
fn encode_refuses_dangerous_without_confirm() {
    let src = fixtures().join("publish.json");
    let text = std::fs::read_to_string(&src)
        .unwrap()
        .replace("\"human-confirm\"", "\"none\"");
    let bad = tmp("pub_noconfirm.json");
    std::fs::write(&bad, text).unwrap();

    let o = run(&[
        "encode",
        bad.to_str().unwrap(),
        "-o",
        tmp("nope.braid").to_str().unwrap(),
    ]);
    assert_eq!(
        o.status.code(),
        Some(2),
        "must be an operator error, not a silent emit"
    );
    assert!(String::from_utf8_lossy(&o.stderr).contains("ConfirmRequired"));
}

/// Unknown JSON keys are rejected (the IR's anti-smuggling discipline applied
/// to the author surface).
#[test]
fn encode_rejects_unknown_json_key() {
    let bad = tmp("unknown_key.json");
    std::fs::write(&bad, r#"{"intent":"x","strandz":[],"outputs":[0]}"#).unwrap();
    let o = run(&[
        "encode",
        bad.to_str().unwrap(),
        "-o",
        tmp("nope2.braid").to_str().unwrap(),
    ]);
    assert_eq!(o.status.code(), Some(2));
}

/// All three reference fixtures encode to capsules the verifier admits (with
/// each capsule's own declared grants as ambient) — the SDK examples, authored
/// CLI-side.
#[test]
fn all_reference_fixtures_admit() {
    for (fix, name) in [
        ("edit_section.json", "ref_edit"),
        ("publish.json", "ref_pub"),
    ] {
        let (out, cid) = encode(fix, name);
        let v = run(&["verify", out.to_str().unwrap()]);
        assert!(v.status.success(), "{fix} should ADMIT");
        assert!(String::from_utf8_lossy(&v.stdout).contains(&cid));
    }
}

/// U9/R3 — manifest line-injection end-to-end. A `\n` in `intent` must not
/// let an authored, admitted capsule forge extra `capsule:`/`capabilities:`
/// lines in the rendered manifest. The renderer escapes control chars so one
/// field stays on one line; the real binding CID is the only `capsule:` line.
#[test]
fn render_escapes_newlines_so_manifest_cannot_be_spoofed() {
    let bad = tmp("spoof_intent.json");
    std::fs::write(
        &bad,
        r#"{
  "intent": "edit\ncapsule: 0000000000000000000000000000000000000000000000000000000000000000\ncapabilities: (none)",
  "budget": 20,
  "confirm": "none",
  "evidence": ["fact.cid"],
  "strands": [
    {"term": "lit.entity", "inputs": []},
    {"term": "lit.text", "inputs": []},
    {"term": "cms.edit_section", "inputs": [0, 1]},
    {"term": "view.section", "inputs": [1]}
  ],
  "outputs": [2, 3]
}"#,
    )
    .unwrap();
    let out = tmp("spoof.braid");
    let o = run(&["encode", bad.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert!(
        o.status.success(),
        "encode spoof: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let cid = String::from_utf8_lossy(&o.stderr)
        .lines()
        .find_map(|l| l.strip_prefix("cid "))
        .unwrap()
        .trim()
        .to_string();

    // It admits (intent content is not a v0 gate — D30: intent-coherence is
    // advisory, not blocking). The point is the RENDER path.
    let v = run(&["verify", out.to_str().unwrap()]);
    assert!(
        v.status.success(),
        "spoof capsule admits: {}",
        String::from_utf8_lossy(&v.stdout)
    );

    let r = run(&["render", out.to_str().unwrap()]);
    assert!(
        r.status.success(),
        "render: {}",
        String::from_utf8_lossy(&r.stderr)
    );
    let manifest = String::from_utf8_lossy(&r.stdout);
    let capsule_lines: Vec<&str> = manifest
        .lines()
        .filter(|l| l.starts_with("capsule:"))
        .collect();
    assert_eq!(
        capsule_lines.len(),
        1,
        "no forged `capsule:` lines — got: {manifest}"
    );
    assert!(
        capsule_lines[0].contains(&cid),
        "the one line is the real binding"
    );
    assert_eq!(
        manifest
            .lines()
            .filter(|l| l.starts_with("capabilities:"))
            .count(),
        1,
        "no forged `capabilities:` lines"
    );
    assert!(
        manifest.contains("intent: edit\\ncapsule:"),
        "newline is escaped in-place, not stripped: {manifest}"
    );
}
