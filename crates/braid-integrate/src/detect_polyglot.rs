//! Polyglot detectors — always-on, language-agnostic.
//!
//! Each detector is a pure `fn(&Scan) -> Vec<Finding>` over the scan
//! inventory. They map existing deps / import seams to the `lgwks_std`
//! feature table and to `lgwks_bot` domain/cap vocabulary.

use crate::scan::Scan;
use crate::{Finding, Severity};

// ── STD-REPLACE — one per lgwks_std feature family ───────────────────────

fn std_replace_findings(scan: &Scan) -> Vec<Finding> {
    let mut out = Vec::new();
    let npm = scan.npm_deps.join(" ").to_ascii_lowercase();
    let py = scan.py_deps.join(" ").to_ascii_lowercase();
    let go = scan.go_deps.join(" ").to_ascii_lowercase();
    let imports = scan.all_imports.join("\n").to_ascii_lowercase();
    let cargo = scan.cargo_dep_lines.join("\n").to_ascii_lowercase();
    let already_lgwks = cargo.contains("lgwks_std") || imports.contains("lgwks_std");

    // hex — suppressed when the repo already uses lgwks_std (estate self-scan).
    if !already_lgwks
        && (npm.contains("hex")
            || py.contains("hex")
            || imports.contains("hex")
            || cargo.contains("hex ")
            || cargo.contains("hex\""))
    {
        out.push(Finding {
            id: "STD-REPLACE-HEX",
            title: "Replace hex crate / hex util with lgwks_std::hex".to_string(),
            rationale: "lgwks_std::hex is zero-dep (core) — same API, no extra crate.".to_string(),
            evidence: evidence_for(scan, &["hex"]),
            maps_to:
                "lgwks_std::hex (core, 0 deps) — lgwks_std = { version = \"0.5\", features = [] }"
                    .to_string(),
            severity: Severity::Info,
        });
    }
    // base64 / encoding
    if npm.contains("base64")
        || imports.contains("base64")
        || cargo.contains("base64")
        || cargo.contains("percent-encoding")
    {
        out.push(Finding {
            id: "STD-REPLACE-ENCODING",
            title: "Replace base64 / percent-encoding with lgwks_std::encoding".to_string(),
            rationale: "lgwks_std::encoding (core) covers base64 + percent-encoding.".to_string(),
            evidence: evidence_for(scan, &["base64", "percent-encoding", "percent_encoding"]),
            maps_to: "lgwks_std::encoding (core)".to_string(),
            severity: Severity::Info,
        });
    }
    // uuid / random — evidence excludes toolkit/fixtures/docs.
    {
        let mut hits = evidence_for(scan, &["uuid"]);
        hits.retain(|e| !is_toolkit_or_fixture_file(e));
        if !hits.is_empty()
            && (npm.contains("uuid")
                || py.contains("uuid")
                || cargo.contains("uuid")
                || imports.contains("uuid"))
        {
            out.push(Finding {
                id: "STD-REPLACE-UUID",
                title: "Replace uuid with lgwks_std::id::Uuid".to_string(),
                rationale:
                    "lgwks_std::id (feature random) wraps getrandom, zero extra deps in std mode."
                        .to_string(),
                evidence: hits,
                maps_to:
                    "lgwks_std::id — lgwks_std = { version = \"0.5\", features = [\"random\"] }"
                        .to_string(),
                severity: Severity::Info,
            });
        }
    }
    // time / chrono — only when an external crate is actually present.
    // `std::time` is the stdlib, not a replaceable dep; require an explicit
    // `chrono` or `time =` cargo line.
    if npm.contains("chrono")
        || cargo.contains("chrono")
        || cargo.contains("\ntime =")
        || cargo.contains("\ntime=")
        || cargo.contains("\"time\"")
        || py.contains("chrono")
    {
        out.push(Finding {
            id: "STD-REPLACE-TIME",
            title: "Replace chrono/time with lgwks_std::time".to_string(),
            rationale: "lgwks_std::time (core) is RFC3339 + calendar math, no chrono dep."
                .to_string(),
            evidence: evidence_for(scan, &["chrono", "time::", "time ="]),
            maps_to: "lgwks_std::time (core)".to_string(),
            severity: Severity::Info,
        });
    }
    // glob / walkdir / fs
    if cargo.contains("walkdir")
        || cargo.contains("glob ")
        || npm.contains("glob")
        || imports.contains("walkdir")
    {
        out.push(Finding {
            id: "STD-REPLACE-FS-GLOB",
            title: "Replace walkdir/glob with lgwks_std::fs / lgwks_std::glob".to_string(),
            rationale: "lgwks_std::fs + lgwks_std::glob are zero-dep and sandbox-aware."
                .to_string(),
            evidence: evidence_for(scan, &["walkdir", "glob"]),
            maps_to: "lgwks_std::fs, lgwks_std::glob (core)".to_string(),
            severity: Severity::Info,
        });
    }
    // regex / pattern
    if cargo.contains("regex ") || npm.contains("regex") || py.contains("regex") {
        out.push(Finding {
            id: "STD-REPLACE-PATTERN",
            title: "Replace regex with lgwks_std::pattern::Regex".to_string(),
            rationale: "lgwks_std::pattern (feature pattern) is regex with linear-time guarantee."
                .to_string(),
            evidence: evidence_for(scan, &["regex"]),
            maps_to: "lgwks_std::pattern — features = [\"pattern\"]".to_string(),
            severity: Severity::Info,
        });
    }
    // serde_json / json — suppress if lgwks_std already has feature json
    let has_json_feature =
        cargo.contains("lgwks_std") && (cargo.contains("\"json\"") || cargo.contains("'json'"));
    if !has_json_feature
        && (cargo.contains("serde_json") || npm.contains("serde") || imports.contains("serde_json"))
    {
        out.push(Finding {
            id: "STD-REPLACE-JSON",
            title: "Replace serde_json wiring with lgwks_std::json".to_string(),
            rationale:
                "lgwks_std::json (feature json) re-exports serde_json with a single feature flag."
                    .to_string(),
            evidence: evidence_for(scan, &["serde_json", "serde-json"]),
            maps_to: "lgwks_std::json — features = [\"json\"]".to_string(),
            severity: Severity::Info,
        });
    }
    // rkyv / wire — suppress if lgwks_std already has feature wire; evidence excludes toolkit.
    let has_wire_feature =
        cargo.contains("lgwks_std") && (cargo.contains("\"wire\"") || cargo.contains("'wire'"));
    if !has_wire_feature && (cargo.contains("rkyv") || imports.contains("rkyv")) {
        let mut ev = evidence_for(scan, &["rkyv"]);
        ev.retain(|e| !is_toolkit_or_fixture_file(e));
        if !ev.is_empty() || cargo.contains("rkyv") {
            out.push(Finding {
                id: "STD-REPLACE-WIRE",
                title: "Replace rkyv wiring with lgwks_std::wire".to_string(),
                rationale: "lgwks_std::wire (feature wire) wraps rkyv for zero-copy binary."
                    .to_string(),
                evidence: if ev.is_empty() {
                    evidence_for(scan, &["rkyv"])
                } else {
                    ev
                },
                maps_to: "lgwks_std::wire — features = [\"wire\"]".to_string(),
                severity: Severity::Info,
            });
        }
    }
    // blake3 / hash — suppress if lgwks_std already has feature hash.
    // Evidence excludes markdown prose (e.g. calibration/FLIGHT_HOURS.md mentioning BLAKE3).
    let has_hash_feature =
        cargo.contains("lgwks_std") && (cargo.contains("\"hash\"") || cargo.contains("'hash'"));
    if !has_hash_feature
        && (cargo.contains("blake3") || cargo.contains("sha2") || npm.contains("blake3"))
    {
        let mut ev = evidence_for(scan, &["blake3", "sha2"]);
        ev.retain(|e| {
            !e.starts_with("calibration/")
                && !e.starts_with("docs/")
                && !is_toolkit_or_fixture_file(e)
        });
        // Only surface if there is non-doc evidence or an explicit cargo dep.
        if !ev.is_empty() || cargo.contains("blake3") || cargo.contains("sha2") {
            if ev.is_empty() {
                ev = evidence_for(scan, &["blake3", "sha2"]);
            }
            out.push(Finding {
                id: "STD-REPLACE-HASH",
                title: "Replace blake3/sha2 with lgwks_std::hash".to_string(),
                rationale: "lgwks_std::hash (feature hash) is BLAKE3, 3 zero-dep leaves."
                    .to_string(),
                evidence: ev,
                maps_to: "lgwks_std::hash — features = [\"hash\"]".to_string(),
                severity: Severity::Info,
            });
        }
    }
    drop(go);
    out
}

fn evidence_for(scan: &Scan, needles: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for f in &scan.files {
        for imp in &f.imports {
            let l = imp.to_ascii_lowercase();
            if needles.iter().any(|n| l.contains(&n.to_ascii_lowercase())) {
                out.push(format!("{}: `{}`", f.rel, imp));
                break;
            }
        }
    }
    // Also surface Cargo.toml / manifest lines that triggered the finding.
    for line in &scan.cargo_dep_lines {
        let l = line.to_ascii_lowercase();
        if needles.iter().any(|n| l.contains(&n.to_ascii_lowercase())) {
            out.push(format!("Cargo.toml: `{}`", line));
        }
    }
    for dep in &scan.npm_deps {
        let l = dep.to_ascii_lowercase();
        if needles.iter().any(|n| l.contains(&n.to_ascii_lowercase())) {
            out.push(format!("package.json: dep `{dep}`"));
        }
    }
    out
}

// ── HTTP / scheduler / secret seams → lgwks_bot ──────────────────────────

fn is_advisor_template_file(rel: &str) -> bool {
    rel.contains("braid-integrate/src/")
}

fn is_toolkit_or_fixture_file(rel: &str) -> bool {
    rel.starts_with("crates/lgwks-std/")
        || rel.starts_with("crates/lgwks-std-gate/")
        || rel.starts_with("crates/lgwks-bot/")
        || rel.contains("braid-integrate/fixtures/")
        || rel.starts_with("docs/")
        || rel.starts_with("calibration/")
}

fn bot_seams(scan: &Scan) -> Vec<Finding> {
    let mut out = Vec::new();
    let http_files: Vec<&str> = scan
        .files
        .iter()
        .filter(|f| {
            f.http_hit && !is_advisor_template_file(&f.rel) && !is_toolkit_or_fixture_file(&f.rel)
        })
        .map(|f| f.rel.as_str())
        .collect();
    if !http_files.is_empty() {
        out.push(Finding {
            id: "BOT-HTTP-SEAM",
            title: "HTTP client seam → lgwks_bot domain::net".to_string(),
            rationale: "Polling/fetching HTTP resources can be modeled as Observe/Execute over domain::net (cap bot.net)."
                .to_string(),
            evidence: http_files.into_iter().map(|p| p.to_string()).collect(),
            maps_to: "lgwks_bot domain::net + domain::gh (Observe, Execute, Query; cap bot.net)"
                .to_string(),
            severity: Severity::Info,
        });
    }
    let sched_files: Vec<&str> = scan
        .files
        .iter()
        .filter(|f| {
            f.sched_hit && !is_advisor_template_file(&f.rel) && !is_toolkit_or_fixture_file(&f.rel)
        })
        .map(|f| f.rel.as_str())
        .collect();
    if !sched_files.is_empty() {
        out.push(Finding {
            id: "BOT-SCHED-SEAM",
            title: "Scheduler/cron seam → lgwks_bot flow + Observe".to_string(),
            rationale: "cron/setInterval/tokio::spawn schedulers map to Observe(Tick) + domain::flow (pipeline / branch / fan-out)."
                .to_string(),
            evidence: sched_files.into_iter().map(|p| p.to_string()).collect(),
            maps_to: "lgwks_bot domain::flow + Observe tick (caps bot.net / bot.sys as needed)"
                .to_string(),
            severity: Severity::Warn,
        });
    }
    // .env / config seam (from file names, not imports) — exclude toolkit/fixtures/docs.
    let secret_hits: Vec<String> = scan
        .files
        .iter()
        .filter(|f| {
            (f.rel.contains(".env") || f.rel.contains("config"))
                && !is_toolkit_or_fixture_file(&f.rel)
        })
        .map(|f| f.rel.clone())
        .collect();
    if !secret_hits.is_empty() {
        out.push(Finding {
            id: "BOT-SECRET-SEAM",
            title: "Config/secret seam — consider caps + contract posture".to_string(),
            rationale: "Files touching .env/config suggest env-gated behavior that should be capability-gated in a bot."
                .to_string(),
            evidence: secret_hits,
            maps_to: "lgwks_bot Cap + GrantSet; contract/APPROVED.toml for Rust deps".to_string(),
            severity: Severity::Info,
        });
    }
    out
}

/// All polyglot detectors.
pub fn detect(scan: &Scan) -> Vec<Finding> {
    let mut out = Vec::new();
    out.extend(std_replace_findings(scan));
    out.extend(bot_seams(scan));
    out
}
