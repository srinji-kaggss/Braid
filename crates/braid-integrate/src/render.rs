//! Report rendering — human text + machine JSON (the AI's contract).
//!
//! The JSON is the durable surface. `to_json` is the only place that
//! defines its shape; `to_text` is a convenience.

use crate::Report;
use crate::scan::Mode;

// ── JSON — the AI's contract ─────────────────────────────────────────────

/// Serialize the report to the machine contract JSON.
pub fn to_json(report: &Report) -> Result<String, String> {
    let v = serde_json::json!({
        "repo": report.repo,
        "mode": match report.mode { Mode::Rust => "rust", Mode::Polyglot => "polyglot" },
        "languages": report.languages,
        "graph": {
            "files": report.scan.files.len(),
            "by_ext": report.scan.by_ext,
            "manifests": report.scan.manifests,
            "imports": report.scan.files.iter().map(|f| serde_json::json!({
                "file": f.rel,
                "imports": f.imports,
                "sched_hit": f.sched_hit,
                "http_hit": f.http_hit
            })).collect::<Vec<_>>(),
            "cargo_dep_lines": report.scan.cargo_dep_lines,
            "npm_deps": report.scan.npm_deps,
            "py_deps": report.scan.py_deps,
            "go_deps": report.scan.go_deps,
        },
        "findings": report.findings.iter().map(|f| serde_json::json!({
            "id": f.id,
            "title": f.title,
            "rationale": f.rationale,
            "evidence": f.evidence,
            "maps_to": f.maps_to,
            "severity": match f.severity { crate::Severity::Info => "info", crate::Severity::Warn => "warn" },
        })).collect::<Vec<_>>(),
        "proposals": report.proposals.iter().map(|p| serde_json::json!({
            "id": p.id,
            "title": p.title,
            "targets": p.targets,
            "caps": p.caps,
            "patch": p.patch,
            "contract_additions": p.contract_additions,
            "rationale": p.rationale,
        })).collect::<Vec<_>>(),
        "next_steps": report.next_steps,
    });
    serde_json::to_string_pretty(&v).map_err(|e| e.to_string())
}

// ── Human text ───────────────────────────────────────────────────────────

/// Human-readable report.
pub fn to_text(report: &Report, verbose: bool) -> String {
    let mut s = String::new();
    s.push_str(&format!("braid-integrate — {}\n", report.repo));
    s.push_str(&format!(
        "mode: {}  languages: {}\n",
        match report.mode {
            Mode::Rust => "rust",
            Mode::Polyglot => "polyglot",
        },
        if report.languages.is_empty() {
            "(none)".to_string()
        } else {
            report.languages.join(", ")
        }
    ));
    s.push_str(&format!(
        "graph: {} files  by_ext: {}\n",
        report.scan.files.len(),
        by_ext_str(&report.scan.by_ext)
    ));
    if !report.scan.manifests.is_empty() {
        s.push_str(&format!(
            "  manifests: {}\n",
            report.scan.manifests.join(", ")
        ));
    }
    s.push_str(&format!(
        "  findings: {}  proposals: {}\n",
        report.findings.len(),
        report.proposals.len()
    ));
    s.push('\n');
    if report.findings.is_empty() {
        s.push_str("No actionable seams detected.\n");
    } else {
        s.push_str("Findings:\n");
        for f in &report.findings {
            s.push_str(&format!(
                "  [{}] {} ({})\n    {}\n    evidence: {}\n    maps_to: {}\n",
                f.id,
                f.title,
                match f.severity {
                    crate::Severity::Info => "info",
                    crate::Severity::Warn => "warn",
                },
                f.rationale,
                if f.evidence.is_empty() {
                    "(none)".to_string()
                } else {
                    f.evidence.join("; ")
                },
                f.maps_to
            ));
        }
    }
    s.push('\n');
    if !report.proposals.is_empty() {
        s.push_str("Proposals (patches — apply with --apply after approval):\n");
        for p in &report.proposals {
            s.push_str(&format!(
                "  [{}] {} -> {}\n    caps: {}\n",
                p.id,
                p.title,
                p.targets.join(", "),
                if p.caps.is_empty() {
                    "(none)".to_string()
                } else {
                    p.caps.join(", ")
                }
            ));
            for line in p.patch.lines().take(12) {
                s.push_str(&format!("    {line}\n"));
            }
            if p.patch.lines().count() > 12 {
                s.push_str("    …\n");
            }
        }
        s.push('\n');
    }
    s.push_str("Next steps:\n");
    for (i, step) in report.next_steps.iter().enumerate() {
        s.push_str(&format!("  {}. {step}\n", i + 1));
    }
    if verbose && !report.scan.files.is_empty() {
        s.push_str("\nScan detail (--verbose):\n");
        for f in &report.scan.files {
            s.push_str(&format!(
                "  {}  ext={}  imports={}  http={} sched={}\n",
                f.rel,
                f.ext,
                f.imports.len(),
                f.http_hit,
                f.sched_hit
            ));
            for imp in &f.imports {
                s.push_str(&format!("    {imp}\n"));
            }
        }
    }
    s
}

fn by_ext_str(m: &std::collections::BTreeMap<String, usize>) -> String {
    if m.is_empty() {
        return "(none)".to_string();
    }
    m.iter()
        .map(|(k, v)| format!("{k}:{v}"))
        .collect::<Vec<_>>()
        .join(" ")
}
