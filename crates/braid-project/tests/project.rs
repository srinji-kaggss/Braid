//! U13 — multi-capsule build, end to end, with the cross-capsule anti-dredging
//! guarantees pinned. Nothing mocked; capsules go through the real frontend and
//! the one verifier.

use braid_elaborate_js::elaborate_js;
use braid_project::{build, build_from_json, parse_project, ProjectError};

const DEMO: &str = include_str!("fixtures/demo.json");

#[test]
fn build_admits_all_capsules() {
    let report = build_from_json(DEMO).expect("demo builds");
    assert_eq!(report.name, "demo");
    let names: Vec<&str> = report.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["greeting", "math", "check"]);
}

/// Anti-dredging — NO authority aggregation (T1/T5): a capsule's CID inside the
/// project is byte-identical to its STANDALONE elaboration. The project does
/// not rewrite, re-wire, or pool anything; building together grants no capsule
/// authority it would not have alone. Mutation-proof: if `build` ever threaded
/// cross-capsule state into a capsule, its CID would diverge from the standalone
/// one and this trips.
#[test]
fn project_does_not_rewrite_or_aggregate() {
    let project = parse_project(DEMO).expect("parses");
    let report = build(&project).expect("builds");
    for (entry, src) in report.entries.iter().zip(project.capsules.iter()) {
        let standalone = elaborate_js(&src.source).expect("standalone elaborates");
        assert_eq!(
            entry.cid.to_hex(),
            standalone.cid().to_hex(),
            "capsule `{}` was rewritten by the project build",
            entry.name
        );
        // Every capsule requests zero authority — a pure project pools nothing.
        assert!(
            entry.capsule.grants.is_empty(),
            "capsule `{}` unexpectedly holds grants",
            entry.name
        );
    }
}

/// Anti-dredging — context/spec drift: the project CID is pinned. It moves only
/// when a capsule's source or name changes — a recorded, reproducible event.
#[test]
fn project_cid_is_pinned() {
    let report = build_from_json(DEMO).expect("builds");
    assert_eq!(
        report.project_cid.to_hex(),
        "7071dd369d6e982374f1734d2c69675697bae9f811674256aa7e1d0e23546d33"
    );
}

/// The project CID is order-independent (a project is a *set* of named
/// capsules): reordering the manifest yields the same CID...
#[test]
fn project_cid_is_reorder_invariant() {
    let reordered = r#"{ "name": "demo", "capsules": [
        { "name": "check",    "source": "1 < 2 && true" },
        { "name": "math",     "source": "1 + 2 * 3" },
        { "name": "greeting", "source": "\"hello\" + \"world\"" }
    ] }"#;
    assert_eq!(
        build_from_json(reordered).unwrap().project_cid.to_hex(),
        build_from_json(DEMO).unwrap().project_cid.to_hex(),
    );
}

/// ...but changing any capsule's source changes it (anti-Goodhart: the pin has
/// teeth — it is a real function of content, not a constant).
#[test]
fn project_cid_tracks_content() {
    let changed = r#"{ "name": "demo", "capsules": [
        { "name": "greeting", "source": "\"hello\" + \"world\"" },
        { "name": "math",     "source": "1 + 2 * 4" },
        { "name": "check",    "source": "1 < 2 && true" }
    ] }"#;
    assert_ne!(
        build_from_json(changed).unwrap().project_cid.to_hex(),
        build_from_json(DEMO).unwrap().project_cid.to_hex(),
    );
}

/// Anti-dredging — fail-closed, never partial: one bad capsule fails the WHOLE
/// build, naming the offender. There is no partial-admit report to exploit.
#[test]
fn one_bad_capsule_fails_the_whole_build() {
    let bad = r#"{ "name": "demo", "capsules": [
        { "name": "ok",  "source": "1 + 2" },
        { "name": "bad", "source": "\"a\" + 1" }
    ] }"#;
    match build_from_json(bad) {
        Err(ProjectError::CapsuleElaboration { name, .. }) => assert_eq!(name, "bad"),
        other => panic!("expected the build to fail on `bad`, got {other:?}"),
    }
}

/// Anti-dredging — no shadowing: a duplicate name cannot hide a second capsule.
#[test]
fn duplicate_names_are_rejected() {
    let dup = r#"{ "name": "demo", "capsules": [
        { "name": "x", "source": "1 + 2" },
        { "name": "x", "source": "3 + 4" }
    ] }"#;
    assert!(matches!(
        build_from_json(dup),
        Err(ProjectError::DuplicateName(n)) if n == "x"
    ));
}

#[test]
fn empty_and_malformed_manifests_fail_closed() {
    assert!(matches!(
        build_from_json(r#"{ "name": "e", "capsules": [] }"#),
        Err(ProjectError::Empty)
    ));
    assert!(matches!(
        build_from_json("not json"),
        Err(ProjectError::Parse(_))
    ));
}
