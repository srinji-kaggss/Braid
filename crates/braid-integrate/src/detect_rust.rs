//! Rust detectors — thin wrappers over `lgwks_std_gate` + Cargo manifest scans.
//!
//! These fire when `Cargo.toml` is present. They reuse the gate's
//! direct-edge ownership audit and add the replacement + scheduler hints that
//! the polyglot pass does not.

use std::path::Path;

use crate::scan::Scan;
use crate::{Finding, Severity};

/// All Rust detectors.
pub fn detect(root: &Path, scan: &Scan) -> Vec<Finding> {
    let mut out = Vec::new();
    out.extend(gate_findings(root));
    out.extend(cargo_replace_findings(scan));
    out.extend(rust_sched_findings(scan));
    out
}

fn gate_findings(root: &Path) -> Vec<Finding> {
    let lock_path = root.join("Cargo.lock");
    if !lock_path.is_file() {
        return Vec::new();
    }
    let contract_path = root.join("contract/APPROVED.toml");
    if !contract_path.is_file() {
        return vec![Finding {
            id: "GATE-NO-CONTRACT",
            title: "No contract/APPROVED.toml — gate will refuse this repo".to_string(),
            rationale:
                "The estate gate (lgwks_std_gate) is fail-closed when the register is missing. Run `lgwks-gate init` to write a starting register."
                    .to_string(),
            evidence: vec![format!("missing {}", contract_path.display())],
            maps_to: "lgwks_std_gate — contract/APPROVED.toml (policy.enforce, [[approved]] entries)"
                .to_string(),
            severity: Severity::Warn,
        }];
    }
    match lgwks_std_gate::check_dependencies(root) {
        Ok((_, refusals)) if refusals.is_empty() => Vec::new(),
        Ok((_, refusals)) => refusals
            .into_iter()
            .map(|r| {
                let (id, title) = match &r {
                    lgwks_std_gate::Refusal::ForeignWorkspaceMember { .. } => (
                        "GATE-FOREIGN-WORKSPACE-COPY",
                        format!("Copied foreign crate: {r}"),
                    ),
                    lgwks_std_gate::Refusal::UnregisteredEdge { .. } => (
                        "GATE-UNOWNED-EDGE",
                        format!("Unowned dependency edge: {r}"),
                    ),
                    lgwks_std_gate::Refusal::ConsumerNotAllowed { .. } => (
                        "GATE-CONSUMER-BYPASS",
                        format!("Dependency owner bypass: {r}"),
                    ),
                    lgwks_std_gate::Refusal::RequirementDrift { .. } => (
                        "GATE-REQUIREMENT-DRIFT",
                        format!("Dependency requirement drift: {r}"),
                    ),
                    lgwks_std_gate::Refusal::SourceDrift { .. } => (
                        "GATE-SOURCE-DRIFT",
                        format!("Dependency source drift: {r}"),
                    ),
                    lgwks_std_gate::Refusal::KindNotAllowed { .. } => (
                        "GATE-KIND-BYPASS",
                        format!("Dependency kind bypass: {r}"),
                    ),
                    lgwks_std_gate::Refusal::UnusedApproval { .. } => (
                        "GATE-UNUSED-APPROVAL",
                        format!("Stale dependency authority: {r}"),
                    ),
                };
                Finding {
                    id,
                    title,
                    rationale: "The gate requires each authored external edge to name its semantic owner, capability, source, requirement, allowed consumer, and dependency kind."
                        .to_string(),
                    evidence: vec![r.to_string()],
                    maps_to: "lgwks_std_gate::audit_direct — consolidate through the owner or amend contract/APPROVED.toml"
                        .to_string(),
                    severity: Severity::Warn,
                }
            })
            .collect(),
        Err(e) => vec![Finding {
            id: "GATE-ERROR",
            title: format!("Gate check failed: {e}"),
            rationale: "The gate could not reach a verdict — treated as a refusal (fail-closed)."
                .to_string(),
            evidence: vec![e.to_string()],
            maps_to: "lgwks_std_gate::check_dependencies".to_string(),
            severity: Severity::Warn,
        }],
    }
}

fn cargo_replace_findings(scan: &Scan) -> Vec<Finding> {
    // If the repo already depends on lgwks_std, the RS-level replacements
    // are redundant — the polyglot layer already suppresses its twin.
    // Suppress here too so a post-adoption repo does not keep nagging.
    let cargo_joined = scan.cargo_dep_lines.join("\n").to_ascii_lowercase();
    if cargo_joined.contains("lgwks_std") {
        return Vec::new();
    }
    // lgwks_std subsumes these crates — suggest feature flags.
    let subsume: &[(&str, &str, &str, &str)] = &[
        ("hex", "STD-RS-HEX", "hex crate", "lgwks_std::hex (core)"),
        (
            "base64",
            "STD-RS-BASE64",
            "base64 / percent-encoding",
            "lgwks_std::encoding (core)",
        ),
        (
            "percent-encoding",
            "STD-RS-BASE64",
            "base64 / percent-encoding",
            "lgwks_std::encoding (core)",
        ),
        ("walkdir", "STD-RS-FS", "walkdir", "lgwks_std::fs (core)"),
        (
            "glob ",
            "STD-RS-GLOB",
            "glob crate",
            "lgwks_std::glob (core)",
        ),
        (
            "uuid",
            "STD-RS-UUID",
            "uuid",
            "lgwks_std::id (feature random)",
        ),
        ("chrono", "STD-RS-TIME", "chrono", "lgwks_std::time (core)"),
        (
            "regex",
            "STD-RS-PATTERN",
            "regex",
            "lgwks_std::pattern (feature pattern)",
        ),
        (
            "serde_json",
            "STD-RS-JSON",
            "serde_json",
            "lgwks_std::json (feature json)",
        ),
        (
            "rkyv",
            "STD-RS-WIRE",
            "rkyv",
            "lgwks_std::wire (feature wire)",
        ),
        (
            "blake3",
            "STD-RS-HASH",
            "blake3",
            "lgwks_std::hash (feature hash)",
        ),
    ];
    let cargo = scan.cargo_dep_lines.join("\n").to_ascii_lowercase();
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for (needle, id, label, maps_to) in subsume {
        if cargo.contains(needle) && seen.insert(id) {
            out.push(Finding {
                id,
                title: format!("Cargo dep `{label}` → {maps_to}"),
                rationale: format!(
                    "{label} is subsumed by lgwks_std — one feature flag, no extra crate."
                ),
                evidence: scan
                    .cargo_dep_lines
                    .iter()
                    .filter(|l| l.to_ascii_lowercase().contains(needle))
                    .cloned()
                    .collect(),
                maps_to: (*maps_to).to_string(),
                severity: Severity::Info,
            });
        }
    }
    out
}

fn is_toolkit_or_fixture_rel(rel: &str) -> bool {
    rel.starts_with("crates/lgwks-std/")
        || rel.starts_with("crates/lgwks-std-gate/")
        || rel.starts_with("crates/lgwks-bot/")
        || rel.contains("braid-integrate/fixtures/")
        || rel.starts_with("docs/")
        || rel.starts_with("calibration/")
}

fn rust_sched_findings(scan: &Scan) -> Vec<Finding> {
    let hits: Vec<String> = scan
        .files
        .iter()
        .filter(|f| {
            f.ext == "rs"
                && f.sched_hit
                && !f.rel.contains("braid-integrate/src/")
                && !is_toolkit_or_fixture_rel(&f.rel)
        })
        .map(|f| format!("{} (tokio::spawn/cron hint)", f.rel))
        .collect();
    if hits.is_empty() {
        return Vec::new();
    }
    vec![Finding {
        id: "BOT-RS-SCHED",
        title: "Rust scheduler seam → lgwks_bot Observe+Execute".to_string(),
        rationale:
            "tokio::spawn / cron-like calls map to lgwks_bot Bot::builder().observe(...).on(...).build(&GrantSet) + tick()."
                .to_string(),
        evidence: hits,
        maps_to: "lgwks_bot Bot + domain::flow (caps bot.net / bot.sys as needed)".to_string(),
        severity: Severity::Info,
    }]
}
