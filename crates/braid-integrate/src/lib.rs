//! Library surface for the Braid integration advisor.
//!
//! `braid-integrate` graphs an arbitrary repo (file inventory, language
//! signals, import lines, manifest deps) and proposes where `lgwks_std`
//! and `lgwks_bot` slot in. It is an **advisor**, not a second verifier:
//! it carries zero admission semantics and never calls `braid-verify`.
//! Output is suggests-for-approval — read-only unless `--apply`, so an
//! AI on the receiving end can paste `--json` and act without re-scanning.
//!
//! The process entrypoint lives in `main.rs` (`braid_runtime::entrypoint`).
//! This crate owns the dispatch and the report types so both are testable
//! without a real process.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod detect_polyglot;
mod detect_rust;
mod propose;
mod render;
mod scan;

use std::process::ExitCode;

use scan::{Mode, Scan};

/// Advisory finding about one integration seam.
#[derive(Debug, Clone)]
pub struct Finding {
    /// Stable identifier, e.g. `STD-REPLACE-HEX`.
    pub id: &'static str,
    /// Human title.
    pub title: String,
    /// One-line why.
    pub rationale: String,
    /// `file:line evidence` strings.
    pub evidence: Vec<String>,
    /// What this maps to (`lgwks_std::hex`, `lgwks_bot domain::net`, …).
    pub maps_to: String,
    /// Severity hint for the report.
    pub severity: Severity,
}

/// Severity hint — advisory, not policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Informational — a replacement is available.
    Info,
    /// Worth acting on soon — drift, scheduler seam.
    Warn,
}

/// One proposed patch.
#[derive(Debug, Clone)]
pub struct Proposal {
    /// Mirrors the finding `id`.
    pub id: &'static str,
    /// Title.
    pub title: String,
    /// Target files (relative to repo root).
    pub targets: Vec<String>,
    /// Caps required if this is a `lgwks_bot` chain.
    pub caps: Vec<String>,
    /// Unified diff text — never applied without `--apply`.
    pub patch: String,
    /// Lines to add to `contract/APPROVED.toml` (empty when not needed).
    pub contract_additions: Vec<String>,
    /// Why this proposal.
    pub rationale: String,
}

/// Full advisor output — the AI's contract.
#[derive(Debug, Clone)]
pub struct Report {
    /// Absolute repo path that was scanned.
    pub repo: String,
    /// Detected mode.
    pub mode: Mode,
    /// Languages present (file extensions + manifest signals).
    pub languages: Vec<String>,
    /// Scan summary (counts, imports).
    pub scan: Scan,
    /// Findings that drove proposals.
    pub findings: Vec<Finding>,
    /// Patch hunks (strings; applied only with `--apply`).
    pub proposals: Vec<Proposal>,
    /// Next-step lines (human + AI).
    pub next_steps: Vec<String>,
}

const USAGE: &str = "\
braid-integrate — advisor that graphs a repo and proposes lgwks_std / lgwks_bot seams

USAGE:
    braid-integrate <repo-path> [--json] [--apply] [--verbose]
    braid-integrate -h | --help | help

ARGS:
    <repo-path>   Path to the repository to inspect

FLAGS:
    --json        Emit the machine contract (single JSON object) to stdout
    --apply       Write suggested patches to disk (otherwise read-only)
    --verbose     Include per-file scan detail in human output
    -h, --help    Print this help

EXIT CODES: 0 plan emitted · 1 repo uninspectable · 2 operator error
";

type CliResult = Result<(), String>;

/// Dispatch for `braid_runtime::entrypoint`. Testable without a process.
pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(|s| s.as_str()) {
        None => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
        Some("-h") | Some("--help") | Some("help") => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        _ => {}
    }
    let parsed = match parse_args(args) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match do_run(parsed) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) if e == UNINSPECTABLE => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::from(2)
        }
    }
}

const UNINSPECTABLE: &str = "__uninspectable__";

struct Parsed {
    repo: String,
    json: bool,
    apply: bool,
    verbose: bool,
}

fn parse_args(args: &[String]) -> Result<Parsed, String> {
    let mut repo: Option<String> = None;
    let mut json = false;
    let mut apply = false;
    let mut verbose = false;
    for a in args {
        match a.as_str() {
            "--json" => json = true,
            "--apply" => apply = true,
            "--verbose" => verbose = true,
            s if s.starts_with('-') => return Err(format!("unknown flag `{s}`")),
            s => {
                if repo.is_some() {
                    return Err(format!("unexpected arg `{s}`"));
                }
                repo = Some(s.to_string());
            }
        }
    }
    let repo = repo.ok_or_else(|| "missing <repo-path>".to_string())?;
    Ok(Parsed {
        repo,
        json,
        apply,
        verbose,
    })
}

fn do_run(p: Parsed) -> CliResult {
    use std::path::Path;
    let root = Path::new(&p.repo);
    if !root.exists() {
        return Err(format!("no such path: {}", p.repo));
    }
    if !root.is_dir() {
        return Err(format!("not a directory: {}", p.repo));
    }
    let scan = scan::scan_repo(root).map_err(|e| format!("scan: {e}"))?;
    if scan.files.is_empty() {
        return Err(UNINSPECTABLE.to_string());
    }
    let mode = scan::detect_mode(root, &scan);
    let mut findings = Vec::new();
    findings.extend(detect_polyglot::detect(&scan));
    if mode == Mode::Rust || root.join("Cargo.toml").exists() {
        findings.extend(detect_rust::detect(root, &scan));
    }
    let proposals = propose::propose(&findings, &scan);
    let next_steps = next_steps_for(&findings, &proposals);
    let report = Report {
        repo: root
            .canonicalize()
            .unwrap_or_else(|_| root.to_path_buf())
            .display()
            .to_string(),
        mode,
        languages: scan.languages.clone(),
        scan,
        findings,
        proposals,
        next_steps,
    };
    if p.apply {
        propose::apply_patches(root, &report.proposals).map_err(|e| format!("apply: {e}"))?;
    }
    if p.json {
        let json = render::to_json(&report).map_err(|e| format!("render json: {e}"))?;
        println!("{json}");
    } else {
        print!("{}", render::to_text(&report, p.verbose));
    }
    Ok(())
}

fn next_steps_for(findings: &[Finding], proposals: &[Proposal]) -> Vec<String> {
    if proposals.is_empty() && findings.is_empty() {
        return vec![
            "No actionable seams detected — nothing to apply.".to_string(),
            "Re-run with --verbose for per-file scan detail.".to_string(),
        ];
    }
    let mut steps = Vec::new();
    let has_std = proposals.iter().any(|p| p.id.starts_with("STD-"));
    let has_bot = proposals.iter().any(|p| p.id.starts_with("BOT-"));
    let has_gate = proposals.iter().any(|p| p.id.starts_with("GATE-"));
    if has_std {
        steps.push(
            "Review STD-REPLACE-* proposals, then run with --apply to write Cargo.toml / feature updates."
                .to_string(),
        );
    }
    if has_bot {
        steps.push(
            "Review BOT-* proposals — each lists required caps (bot.net / bot.fs / bot.notify); add grants to BotSpec/GrantSet."
                .to_string(),
        );
    }
    if has_gate {
        steps.push(
            "Run `lgwks-gate check` / fill contract/APPROVED.toml entries from proposals[*].contract_additions."
                .to_string(),
        );
    }
    if !has_std && !has_bot && !has_gate {
        steps.push("Review findings above, then run with --apply after approval.".to_string());
    }
    steps.push("Paste --json output into the next AI session — it carries file:line evidence + patch hunks + caps.".to_string());
    steps
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    struct TestDir(std::path::PathBuf);

    fn remove_test_dir(path: &Path) {
        match std::fs::remove_dir_all(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("remove test directory {}: {error}", path.display()),
        }
    }

    impl TestDir {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("braid-integrate-{name}-{}", std::process::id()));
            remove_test_dir(&path);
            std::fs::create_dir_all(&path).expect("create isolated test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            remove_test_dir(&self.0);
        }
    }

    #[test]
    fn help_exits_zero() {
        assert_eq!(run(&["--help".to_string()]), ExitCode::SUCCESS);
    }

    #[test]
    fn missing_arg_is_operator_error() {
        assert_eq!(run(&[]), ExitCode::from(2));
    }

    #[test]
    fn unknown_flag_is_operator_error() {
        assert_eq!(
            run(&["/tmp".to_string(), "--bogus".to_string()]),
            ExitCode::from(2)
        );
    }

    #[test]
    fn nonexistent_path_is_operator_error() {
        assert_eq!(run(&["/no/such/path/xyzzy".to_string()]), ExitCode::from(2));
    }

    #[test]
    fn read_only_by_default_leaves_repo_untouched() {
        let tmp = TestDir::new("read-only");
        let root = tmp.path();
        std::fs::write(
            root.join("package.json"),
            r#"{"name":"t","dependencies":{"uuid":"^9.0.0"}}"#,
        )
        .unwrap();
        std::fs::write(root.join("index.js"), "import { v4 } from \"uuid\";\n").unwrap();
        let before = dir_snapshot(root);
        let code = run(&[root.display().to_string()]);
        assert_eq!(code, ExitCode::SUCCESS);
        let after = dir_snapshot(root);
        assert_eq!(
            before, after,
            "advisor without --apply must not mutate the repo"
        );
    }

    #[test]
    fn json_output_is_parseable_and_carries_evidence() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/js-mini");
        if !fixtures.is_file() && !fixtures.is_dir() {
            return;
        }
        let code = run(&[fixtures.display().to_string(), "--json".to_string()]);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn js_mini_fixture_emits_expected_findings() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/js-mini");
        if !fixtures.is_dir() {
            return;
        }
        let scan = scan::scan_repo(&fixtures).expect("scan js-mini");
        let findings = detect_polyglot::detect(&scan);
        let ids: std::collections::BTreeSet<&str> = findings.iter().map(|f| f.id).collect();
        assert!(
            ids.contains("STD-REPLACE-UUID"),
            "js-mini should flag uuid, got {ids:?}"
        );
        assert!(
            ids.contains("BOT-HTTP-SEAM"),
            "js-mini should flag http seam, got {ids:?}"
        );
        assert!(
            ids.contains("BOT-SCHED-SEAM"),
            "js-mini should flag sched seam, got {ids:?}"
        );
    }

    #[test]
    fn rs_mini_fixture_emits_expected_findings() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/rs-mini");
        if !fixtures.is_dir() {
            return;
        }
        let scan = scan::scan_repo(&fixtures).expect("scan rs-mini");
        let mode = scan::detect_mode(&fixtures, &scan);
        assert_eq!(mode, scan::Mode::Rust);
        let mut findings = detect_polyglot::detect(&scan);
        findings.extend(detect_rust::detect(&fixtures, &scan));
        let ids: std::collections::BTreeSet<&str> = findings.iter().map(|f| f.id).collect();
        assert!(
            ids.contains("STD-REPLACE-HEX"),
            "rs-mini should flag hex, got {ids:?}"
        );
        assert!(
            ids.contains("STD-RS-HEX"),
            "rs-mini should flag STD-RS-HEX, got {ids:?}"
        );
    }

    fn dir_snapshot(root: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
        let mut out = std::collections::BTreeMap::new();
        collect(root, root, &mut out);
        out
    }

    fn collect(root: &Path, dir: &Path, out: &mut std::collections::BTreeMap<String, Vec<u8>>) {
        for ent in std::fs::read_dir(dir).unwrap() {
            let ent = ent.unwrap();
            let p = ent.path();
            if ent.file_type().unwrap().is_dir() {
                collect(root, &p, out);
            } else {
                let rel = p.strip_prefix(root).unwrap().display().to_string();
                out.insert(rel, std::fs::read(&p).unwrap_or_default());
            }
        }
    }
}
