//! Findings → proposals. Each proposal carries a unified-diff patch
//! string and optional `contract/APPROVED.toml` additions. Nothing is
//! written to disk here — `--apply` in `lib.rs` calls `apply_patches`.

use std::path::Path;

use crate::scan::Scan;
use crate::{Finding, Proposal};

/// Map findings to proposals (pure — no I/O).
pub fn propose(findings: &[Finding], scan: &Scan) -> Vec<Proposal> {
    let mut out: Vec<Proposal> = Vec::new();
    for f in findings {
        match f.id {
            id if id.starts_with("STD-REPLACE-") || id.starts_with("STD-RS-") => {
                out.push(std_proposal(f, scan));
            }
            id if id.starts_with("BOT-") => {
                out.push(bot_proposal(f));
            }
            "GATE-NO-CONTRACT" => {
                out.push(Proposal {
                    id: "GATE-NO-CONTRACT",
                    title: "Add contract/APPROVED.toml".to_string(),
                    targets: vec!["contract/APPROVED.toml".to_string()],
                    caps: Vec::new(),
                    patch: patch_new_contract(),
                    contract_additions: Vec::new(),
                    rationale: f.rationale.clone(),
                });
            }
            id if id.starts_with("GATE-") => {
                out.push(Proposal {
                    id: f.id,
                    title: f.title.clone(),
                    targets: vec!["contract/APPROVED.toml".to_string()],
                    caps: Vec::new(),
                    patch: patch_gate_fix(f),
                    contract_additions: gate_contract_lines(f),
                    rationale: f.rationale.clone(),
                });
            }
            _ => {
                out.push(Proposal {
                    id: f.id,
                    title: f.title.clone(),
                    targets: f.evidence.clone(),
                    caps: Vec::new(),
                    patch: format!("# no patch template for {} — see rationale\n", f.id),
                    contract_additions: Vec::new(),
                    rationale: f.rationale.clone(),
                });
            }
        }
    }
    out
}

fn std_proposal(f: &Finding, scan: &Scan) -> Proposal {
    let feature = feature_for(f.id);
    let cargo_targets: Vec<String> = scan
        .cargo_dep_lines
        .iter()
        .filter(|l| {
            let l = l.to_ascii_lowercase();
            let n = f.id.to_ascii_lowercase();
            // cheap: feature name appears in the cargo line
            l.contains(feature) || n.contains(&l.split('=').next().unwrap_or("").trim().to_string())
        })
        .map(|_| "Cargo.toml".to_string())
        .collect();
    let targets = if cargo_targets.is_empty() {
        vec!["Cargo.toml".to_string()]
    } else {
        cargo_targets
    };
    let patch = patch_cargo_lgwks(feature);
    Proposal {
        id: f.id,
        title: f.title.clone(),
        targets,
        caps: Vec::new(),
        patch,
        contract_additions: Vec::new(),
        rationale: f.rationale.clone(),
    }
}

fn feature_for(id: &str) -> &'static str {
    match id {
        "STD-REPLACE-HEX" | "STD-RS-HEX" => "core",
        "STD-REPLACE-ENCODING" | "STD-RS-BASE64" => "core",
        "STD-REPLACE-UUID" | "STD-RS-UUID" => "random",
        "STD-REPLACE-TIME" | "STD-RS-TIME" => "core",
        "STD-REPLACE-FS-GLOB" | "STD-RS-FS" | "STD-RS-GLOB" => "core",
        "STD-REPLACE-PATTERN" | "STD-RS-PATTERN" => "pattern",
        "STD-REPLACE-JSON" | "STD-RS-JSON" => "json",
        "STD-REPLACE-WIRE" | "STD-RS-WIRE" => "wire",
        "STD-REPLACE-HASH" | "STD-RS-HASH" => "hash",
        _ => "core",
    }
}

fn patch_cargo_lgwks(feature: &str) -> String {
    if feature == "core" {
        r#"# Cargo.toml — add lgwks_std (core is default, 0 deps)
[dependencies]
lgwks_std = "0.5"
# then: use lgwks_std::hex; // or encoding / time / fs / glob
"#
        .to_string()
    } else {
        format!(
            r#"# Cargo.toml — add lgwks_std with feature `{feature}`
[dependencies]
lgwks_std = {{ version = "0.5", features = ["{feature}"] }}
# then: use lgwks_std::...;
"#
        )
    }
}

fn bot_proposal(f: &Finding) -> Proposal {
    let (caps, snippet) = match f.id {
        "BOT-HTTP-SEAM" => (
            vec!["bot.net".to_string()],
            "// lgwks_bot — HTTP poll as Observe+Execute\nuse lgwks_bot::{Bot, Cap, GrantSet};\nuse lgwks_bot::domain::net;\n\nlet bot = Bot::builder(\"http-watcher\")\n    .observe(net::Poll::new(\"https://example.com\"))\n    .on(|state| state.changed, net::Fetch::new())\n    .build(&GrantSet::empty().grant(Cap::net()))?;\nbot.tick()?;\n",
        ),
        "BOT-SCHED-SEAM" | "BOT-RS-SCHED" => (
            vec!["bot.net".to_string(), "bot.sys".to_string()],
            "// lgwks_bot \u{2014} scheduler seam as Observe(Tick) + flow\nuse lgwks_bot::{Bot, Cap, GrantSet};\nuse lgwks_bot::domain::flow;\n\nlet bot = Bot::builder(\"cron-bot\")\n    .observe(flow::Tick::every_secs(60))\n    .on(|_| true, Notify::new(\"#deploys\"))\n    .build(&GrantSet::empty().grant(Cap::net()).grant(Cap::sys()))?;\nbot.tick()?;\n",
        ),
        _ => (
            vec!["bot.notify".to_string()],
            "// see lgwks_bot docs for this seam\n",
        ),
    };
    Proposal {
        id: f.id,
        title: f.title.clone(),
        targets: f.evidence.clone(),
        caps,
        patch: snippet.to_string(),
        contract_additions: Vec::new(),
        rationale: f.rationale.clone(),
    }
}

fn patch_new_contract() -> String {
    r#"# contract/APPROVED.toml — new file (fill with lgwks-gate request)
[policy]
enforce = true

[[approved]]
crate = "serde"
tier = "boundary"
version = "1.0"
reason = "Derive-based serialization needs compiler introspection std lacks."
approved_by = "Director"
approved_on = "2026-08-26"
review = "docs/ADMISSION.md"
"#
    .to_string()
}

fn patch_gate_fix(f: &Finding) -> String {
    format!(
        "# contract/APPROVED.toml — add approval for gate refusal\n# run: lgwks-gate request <crate> <version> to fill the block\n# refusal: {}\n{}",
        f.evidence.first().cloned().unwrap_or_default(),
        r#"[[approved]]
crate = "<crate>"
tier = "<boundary|leaf>"
version = "<resolved version>"
reason = "<what std cannot do>"
approved_by = "Director"
approved_on = "2026-08-26"
review = "docs/ADMISSION.md"
"#
    )
}

fn gate_contract_lines(f: &Finding) -> Vec<String> {
    vec![format!(
        "# for: {} — fill with lgwks-gate request",
        f.evidence.first().cloned().unwrap_or_default()
    )]
}

/// Apply patches when `--apply` was requested.
///
/// Only `GATE-NO-CONTRACT` actually writes a file (contract/APPROVED.toml)
/// when it does not already exist. All other proposals are advisory diff
/// text; applying them without a real hunk applier would risk corrupting
/// the target. V0 prints the patches and writes new-file proposals only.
pub fn apply_patches(root: &Path, proposals: &[Proposal]) -> Result<(), String> {
    for p in proposals {
        match p.id {
            "GATE-NO-CONTRACT" => {
                let path = root.join("contract/APPROVED.toml");
                if path.exists() {
                    continue;
                }
                let dir = path.parent().ok_or_else(|| "no parent".to_string())?;
                std::fs::create_dir_all(dir)
                    .map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
                // Atomic: write to temp then rename.
                let tmp = dir.join(".APPROVED.toml.tmp");
                std::fs::write(&tmp, &p.patch).map_err(|e| format!("write tmp: {e}"))?;
                std::fs::rename(&tmp, &path).map_err(|e| format!("rename: {e}"))?;
            }
            _ => {
                // Advisory: do not mutate the repo — the patch is for the AI to apply after approval.
                // Surface the location so --apply is not silent.
                eprintln!(
                    "advice --apply: proposal {} would patch {}",
                    p.id,
                    p.targets.join(", ")
                );
            }
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn _scan_unused(scan: &Scan) -> usize {
    scan.files.len()
}
