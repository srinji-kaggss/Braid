//! W5 integration at the real interface: the `braid` binary itself — the
//! same surface a fresh agent gets. Covers the golden org map, the
//! fail-closed negative matrix, and cross-command machine-line identity.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_braid")
}

fn vectors() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/braid/vectors/w5")
}

/// A fresh per-test store root under the system temp dir.
fn tmp_store(test: &str) -> PathBuf {
    std::env::temp_dir().join(format!("braid-w5-{test}-{}", std::process::id()))
}

fn run(args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .output()
        .expect("braid binary must run")
}

fn stderr_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// Seed a store with every fixture manifest, then install the declared
/// inventory — all through the binary itself (no side-channel writes).
fn seed(store: &Path) {
    let manifests = vectors().join("manifests");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&manifests)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    for f in files {
        let out = run(&[
            "store",
            "put",
            f.to_str().unwrap(),
            "--store",
            store.to_str().unwrap(),
        ]);
        assert!(
            out.status.success(),
            "put {} failed: {}",
            f.display(),
            stderr_of(&out)
        );
    }
    std::fs::copy(
        vectors().join("inventory.json"),
        store.join("inventory.json"),
    )
    .unwrap();
}

// ───────────────────────────── the golden org map ─────────────────────────────

#[test]
fn catalog_covers_all_declared_repos_with_no_unknown_and_matches_golden() {
    let store = tmp_store("golden");
    seed(&store);

    let out = run(&["catalog", "--store", store.to_str().unwrap()]);
    assert!(out.status.success(), "catalog failed: {}", stderr_of(&out));
    let stdout = String::from_utf8(out.stdout).unwrap();

    // The full map: 10 repos, no UNKNOWN anywhere.
    assert!(!stdout.contains("UNKNOWN"), "UNKNOWN leaked into the map");
    let machine = stdout.split("---\n").nth(1).expect("machine block");
    let lines: Vec<&str> = machine.lines().collect();
    assert_eq!(lines.len(), 10, "expected 10 machine lines:\n{stdout}");
    for line in &lines {
        assert_eq!(
            line.split('\t').count(),
            9,
            "machine line must be exactly 9 TSV fields: {line:?}"
        );
    }
    // Sorted by name.
    let names: Vec<&str> = lines
        .iter()
        .map(|l| l.split('\t').next().unwrap())
        .collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted);

    // Byte-identical to the committed golden vector (determinism proof).
    let golden =
        std::fs::read_to_string(vectors().join("catalog.golden")).expect("catalog.golden fixture");
    assert_eq!(
        stdout, golden,
        "catalog output drifted from the golden vector"
    );
}

// ───────────────────────────── fail-closed reads ─────────────────────────────

#[test]
fn catalog_without_inventory_fails_closed() {
    let store = tmp_store("no-inventory");
    std::fs::create_dir_all(&store).unwrap();
    let out = run(&[
        "store",
        "put",
        vectors().join("manifests/braid.json").to_str().unwrap(),
        "--store",
        store.to_str().unwrap(),
    ]);
    assert!(out.status.success(), "put failed: {}", stderr_of(&out));

    let out = run(&["catalog", "--store", store.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty(), "no rows may be emitted");
    assert!(
        stderr_of(&out).contains("no inventory declared"),
        "stderr must name the missing inventory: {}",
        stderr_of(&out)
    );
}

#[test]
fn catalog_denies_on_inventory_mismatch() {
    let store = tmp_store("mismatch");
    seed(&store);
    // Missing repo: remove one manifest.
    std::fs::remove_file(store.join("moo.manifest")).unwrap();
    let out = run(&["catalog", "--store", store.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert!(
        stderr_of(&out).contains("missing from store: moo"),
        "stderr: {}",
        stderr_of(&out)
    );

    // Undeclared repo: an edited document whose name is not in the inventory
    // must be denied by the declared-set gate.
    let edited = vectors().join("manifests/braid.json");
    let mut doc: lgwks_std::json::Value =
        lgwks_std::json::from_str(&std::fs::read_to_string(&edited).unwrap()).unwrap();
    doc["name"] = "mystery-repo".into();
    let dir = std::env::temp_dir().join(format!("braid-w5-undeclared-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("mystery.json");
    std::fs::write(&p, doc.to_string()).unwrap();
    let out = run(&[
        "store",
        "put",
        p.to_str().unwrap(),
        "--store",
        store.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(1), "undeclared put must be denied");
    assert!(
        stderr_of(&out).contains("not in the declared inventory"),
        "stderr: {}",
        stderr_of(&out)
    );
}

#[test]
fn tampered_artifact_denies_catalog_with_no_rows() {
    let store = tmp_store("tampered");
    seed(&store);
    // Flip one byte in a stored artifact.
    let path = store.join("wwfd.manifest");
    let mut bytes = std::fs::read(&path).unwrap();
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0x01;
    std::fs::write(&path, &bytes).unwrap();

    let out = run(&["catalog", "--store", store.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty(), "no partial rows");
    assert!(
        stderr_of(&out).contains("wwfd.manifest"),
        "stderr must name the bad artifact: {}",
        stderr_of(&out)
    );
}

#[test]
fn renamed_file_key_mismatch_is_denied() {
    let store = tmp_store("key-mismatch");
    seed(&store);
    // A manifest stored under a different key: the stored name must equal
    // the filename — a rename is a tampered artifact.
    std::fs::copy(store.join("braid.manifest"), store.join("evil.manifest")).unwrap();
    let out = run(&["catalog", "--store", store.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert!(
        stderr_of(&out).contains("does not match its key"),
        "stderr: {}",
        stderr_of(&out)
    );
}

#[test]
fn missing_store_is_operator_error_not_a_denial() {
    let out = run(&["catalog", "--store", "/nonexistent-braid-store-w5"]);
    assert_eq!(out.status.code(), Some(2), "missing path = operator error");
    assert!(stderr_of(&out).contains("does not exist"));
}

// ───────────────────────────── summary + put ─────────────────────────────

#[test]
fn summary_line_is_byte_identical_to_catalog_line() {
    let store = tmp_store("summary");
    seed(&store);
    let cat = run(&["catalog", "--store", store.to_str().unwrap()]);
    let cat_stdout = String::from_utf8(cat.stdout).unwrap();
    let cat_line = cat_stdout
        .split("---\n")
        .nth(1)
        .unwrap()
        .lines()
        .find(|l| l.starts_with("keel\t"))
        .expect("keel machine line");

    let sum = run(&["summary", "keel", "--store", store.to_str().unwrap()]);
    assert!(sum.status.success(), "summary failed: {}", stderr_of(&sum));
    let sum_stdout = String::from_utf8(sum.stdout).unwrap();
    assert!(
        sum_stdout.lines().any(|l| l == cat_line),
        "summary must emit the byte-identical machine line:\n{sum_stdout}"
    );
    assert!(sum_stdout.contains("name:         keel"));
}

#[test]
fn summary_unknown_repo_is_denied_and_usage_is_exit_2() {
    let store = tmp_store("summary-negative");
    seed(&store);
    let out = run(&["summary", "nope", "--store", store.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr_of(&out).contains("unknown repo `nope`"));

    let out = run(&["summary"]);
    assert_eq!(out.status.code(), Some(2));
    let out = run(&["summary", "a", "b"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn put_rejects_out_of_contract_and_writes_nothing() {
    let store = tmp_store("put-negative");
    for fixture in ["bad-archetype.json", "traversal-name.json"] {
        let out = run(&[
            "store",
            "put",
            vectors().join("negative").join(fixture).to_str().unwrap(),
            "--store",
            store.to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(1), "{fixture} must be denied");
        assert!(
            !store.exists(),
            "a rejected manifest must not create the store ({fixture})"
        );
    }
}

#[test]
fn duplicate_put_is_denied_then_replace_updates() {
    let store = tmp_store("replace");
    std::fs::create_dir_all(&store).unwrap();
    let p = vectors().join("manifests/braid.json");
    let first = run(&[
        "store",
        "put",
        p.to_str().unwrap(),
        "--store",
        store.to_str().unwrap(),
    ]);
    assert!(first.status.success());
    let first_cid: String = String::from_utf8(first.stdout)
        .unwrap()
        .split("cid ")
        .nth(1)
        .unwrap()
        .trim()
        .to_string();

    let dup = run(&[
        "store",
        "put",
        p.to_str().unwrap(),
        "--store",
        store.to_str().unwrap(),
    ]);
    assert_eq!(dup.status.code(), Some(1), "duplicate must be denied");
    assert!(stderr_of(&dup).contains("already stored"));

    // An edited document replaces and moves the CID.
    let dir = std::env::temp_dir().join(format!("braid-w5-replace-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let edited = dir.join("braid.json");
    let mut doc: lgwks_std::json::Value =
        lgwks_std::json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    doc["owner"] = "Director (updated)".into();
    std::fs::write(&edited, doc.to_string()).unwrap();

    let repl = run(&[
        "store",
        "put",
        edited.to_str().unwrap(),
        "--store",
        store.to_str().unwrap(),
        "--replace",
    ]);
    assert!(
        repl.status.success(),
        "replace failed: {}",
        stderr_of(&repl)
    );
    let new_cid: String = String::from_utf8(repl.stdout)
        .unwrap()
        .split("cid ")
        .nth(1)
        .unwrap()
        .trim()
        .to_string();
    assert_ne!(first_cid, new_cid, "an edited manifest must move the CID");
}

#[test]
fn put_creates_the_store_root_on_first_run() {
    let store = tmp_store("first-run").join("nested").join("store");
    let out = run(&[
        "store",
        "put",
        vectors().join("manifests/braid.json").to_str().unwrap(),
        "--store",
        store.to_str().unwrap(),
    ]);
    assert!(
        out.status.success(),
        "first put failed: {}",
        stderr_of(&out)
    );
    assert!(store.join("braid.manifest").exists());
}
