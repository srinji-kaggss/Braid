//! # braid — the human-reconstructable CLI loop (ADR-088 G6/L7, U6 #2)
//!
//! `encode | decode | verify | render | diff` — the same admission loop the
//! Rust SDK drives, reachable by a human with no AI and no Rust toolchain
//! (scenario #12, threat T13). The CLI is NOT a second verifier: `encode`
//! routes author input through the `braid-sdk` Builder + the canonical
//! encoder, and `verify` calls the one `braid-verify` the SDK path uses. The
//! CLI adds zero authority and zero admission semantics.
//!
//! ## `encode` input — JSON-of-IR (D19, Director-selected 2026-06-14)
//!
//! A 1:1 data transcription of the IR — NOT a surface grammar (the PRD
//! non-goal D17 keeps gated). Grants are derived from the terms used;
//! `vocab_version`/`registry_cid`/`ir_version` come from the pinned registry,
//! never hand-typed. So the CLI path is byte-identical to the SDK path and
//! reproduces the pinned reference CIDs.
//!
//! ```json
//! {
//!   "intent": "Edit landing section and render preview (reversible)",
//!   "budget": 20,
//!   "confirm": "none",
//!   "evidence": ["fact.cid"],
//!   "strands": [
//!     {"term": "lit.entity", "inputs": []},
//!     {"term": "lit.text", "inputs": []},
//!     {"term": "cms.edit_section", "inputs": [0, 1]},
//!     {"term": "view.section", "inputs": [1]}
//!   ],
//!   "outputs": [2, 3]
//! }
//! ```
//! `budget`, `confirm`, `evidence` are optional (budget defaults to the
//! composed cost; a dangerous capsule with no `confirm` is refused at author
//! time, fail-closed). Unknown keys are rejected.

use std::process::ExitCode;

use braid_ir::registry_v0;
use braid_ir::{Capsule, ConfirmPolicy};
use braid_render::{has_widening, manifest, manifest_diff, render_text, DeltaKind};
use braid_sdk::Builder;
use braid_verify::{verify, Verdict};
use serde::{Deserialize, Serialize};

// ── JSON-of-IR author form (D19). `deny_unknown_fields` mirrors the IR's
//    require_only_keys anti-smuggling discipline: an unrecognized key is a
//    rejected author input, never silently dropped. ──

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonCapsule {
    intent: String,
    #[serde(default)]
    budget: Option<u64>,
    #[serde(default)]
    confirm: Option<String>,
    #[serde(default)]
    evidence: Vec<String>,
    strands: Vec<JsonStrand>,
    outputs: Vec<u32>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct JsonStrand {
    term: String,
    inputs: Vec<u32>,
}

#[derive(Serialize)]
struct JsonCapsuleOut {
    intent: String,
    budget: u64,
    confirm: String,
    evidence: Vec<String>,
    strands: Vec<JsonStrand>,
    outputs: Vec<u32>,
}

/// Operator error (usage / IO / malformed input / author-time reject). Distinct
/// from a *policy-negative* outcome (verifier Reject, diff Widening), which is
/// a successful run reporting a deny and uses exit code 1.
type CliResult = Result<(), String>;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = match args.first() {
        Some(c) => c.as_str(),
        None => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let rest = &args[1..];
    let outcome = match cmd {
        "encode" => cmd_encode(rest),
        "decode" => cmd_decode(rest),
        "verify" => cmd_verify(rest),
        "render" => cmd_render(rest),
        "diff" => cmd_diff(rest),
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        other => Err(format!("unknown command `{other}`\n\n{USAGE}")),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        // Policy-negative: a clean run that ends in a deny. Exit 1 so CI gates
        // and shell `&&` chains treat it as a failure, distinct from operator
        // error (exit 2).
        Err(e) if e == POLICY_NEGATIVE => ExitCode::from(1),
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::from(2)
        }
    }
}

/// Sentinel: the command ran correctly and the answer is "denied" (reject /
/// widening). Carried as an Err so `main` can map it to exit 1, but it is NOT
/// an operator error — the human-readable verdict was already printed.
const POLICY_NEGATIVE: &str = "\0policy-negative";

const USAGE: &str = "\
braid — machine-first capsule loop (ADR-088 U6)

USAGE:
    braid encode <input.json> [-o <out.braid>]   author JSON-of-IR -> canonical bytes (+ CID)
    braid decode <capsule.braid>                 canonical bytes -> JSON-of-IR (inverse of encode)
    braid verify <capsule.braid> [--grant <cap>] run the admission pipeline (exit 1 on Reject)
    braid render <capsule.braid>                 the human-review manifest
    braid diff <old.braid> <new.braid>           manifest delta (exit 1 on any Widening)

EXIT CODES: 0 ok · 1 policy-negative (Reject / Widening) · 2 operator error

`verify --grant` may repeat; it sets the ambient authority the principal holds.
Default ambient = the capsule's own declared grants (the happy-path check).";

// ───────────────────────────────── encode ─────────────────────────────────

fn cmd_encode(args: &[String]) -> CliResult {
    let mut input: Option<&str> = None;
    let mut out: Option<&str> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--out" => {
                out = Some(args.get(i + 1).ok_or("`-o` needs a path")?.as_str());
                i += 2;
            }
            p if !p.starts_with('-') && input.is_none() => {
                input = Some(p);
                i += 1;
            }
            other => return Err(format!("unexpected arg `{other}` (encode)")),
        }
    }
    let path = input.ok_or("encode needs <input.json>")?;
    let text = read_text(path)?;
    let jc: JsonCapsule =
        serde_json::from_str(&text).map_err(|e| format!("{path}: invalid JSON-of-IR: {e}"))?;

    let capsule = build_from_json(jc)?;
    let bytes = capsule.to_bytes();
    let cid = capsule.cid();

    match out {
        Some(p) => {
            std::fs::write(p, &bytes).map_err(|e| format!("write {p}: {e}"))?;
            // CID to stderr so stdout stays clean for piping; the artifact is
            // the file. A human can re-derive this CID from the bytes alone.
            eprintln!("wrote {p} ({} bytes)\ncid {}", bytes.len(), cid.to_hex());
        }
        None => {
            use std::io::Write;
            std::io::stdout()
                .write_all(&bytes)
                .map_err(|e| format!("stdout: {e}"))?;
            eprintln!("cid {}", cid.to_hex());
        }
    }
    Ok(())
}

/// Route JSON-of-IR through the SDK Builder (D19). The Builder derives grants,
/// pins the registry, and runs the author-time checks the verifier repeats —
/// so a CLI-authored capsule is byte-identical to an SDK-authored one.
fn build_from_json(jc: JsonCapsule) -> Result<Capsule, String> {
    let registry = registry_v0();
    let mut b = Builder::new(&registry, jc.intent);

    let mut handles = Vec::with_capacity(jc.strands.len());
    for (i, js) in jc.strands.iter().enumerate() {
        let mut inputs = Vec::with_capacity(js.inputs.len());
        for &idx in &js.inputs {
            // A handle exists only for an already-placed strand. A forward /
            // self reference is unrepresentable in the IR (acyclicity is
            // structural); reject it here with a clear author message instead
            // of indexing out of bounds.
            let h = handles.get(idx as usize).ok_or_else(|| {
                format!(
                    "strand {i} `{}` references input strand {idx}, which is not defined before it \
                     (inputs must reference strictly earlier strands)",
                    js.term
                )
            })?;
            inputs.push(*h);
        }
        let h = b
            .strand(&js.term, &inputs)
            .map_err(|e| format!("strand {i} `{}`: {e:?}", js.term))?;
        handles.push(h);
    }

    for &o in &jc.outputs {
        let h = handles
            .get(o as usize)
            .ok_or_else(|| format!("output references strand {o}, which does not exist"))?;
        b.output(*h);
    }
    if let Some(bud) = jc.budget {
        b.budget(bud);
    }
    if let Some(c) = &jc.confirm {
        b.confirm(parse_confirm(c)?);
    }
    for e in jc.evidence {
        b.evidence(e);
    }
    b.build().map_err(|e| format!("author-time reject: {e:?}"))
}

// ───────────────────────────────── decode ─────────────────────────────────

fn cmd_decode(args: &[String]) -> CliResult {
    let path = single_path(args, "decode")?;
    let capsule = read_capsule(path)?;
    let out = JsonCapsuleOut {
        intent: capsule.intent.clone(),
        budget: capsule.budget,
        confirm: confirm_str(capsule.confirm).to_string(),
        evidence: capsule.evidence.clone(),
        strands: capsule
            .braid
            .strands
            .iter()
            .map(|s| JsonStrand {
                term: s.term.clone(),
                inputs: s.inputs.clone(),
            })
            .collect(),
        outputs: capsule.braid.outputs.clone(),
    };
    let json = serde_json::to_string_pretty(&out).map_err(|e| format!("serialize: {e}"))?;
    println!("{json}");
    Ok(())
}

// ───────────────────────────────── verify ─────────────────────────────────

fn cmd_verify(args: &[String]) -> CliResult {
    let mut path: Option<&str> = None;
    let mut grants: Vec<braid_capability::Capability> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--grant" => {
                let name = args.get(i + 1).ok_or("`--grant` needs a capability")?;
                let cap = name
                    .parse()
                    .map_err(|_| format!("unknown capability `{name}`"))?;
                grants.push(cap);
                i += 2;
            }
            p if !p.starts_with('-') && path.is_none() => {
                path = Some(p);
                i += 1;
            }
            other => return Err(format!("unexpected arg `{other}` (verify)")),
        }
    }
    let path = path.ok_or("verify needs <capsule.braid>")?;
    let bytes = read_bytes(path)?;

    // Default ambient = the capsule's own declared grants (a principal
    // authorized for exactly what it requests) — the happy-path admission
    // check. `--grant` overrides to model a narrower (or wider) principal,
    // e.g. to demonstrate the attenuation reject.
    let ambient = if grants.is_empty() {
        read_capsule(path)?.grants
    } else {
        grants
    };

    let registry = registry_v0();
    match verify(&bytes, &registry, &ambient) {
        Verdict::Admit { capsule_cid } => {
            println!("ADMIT  cid {}", capsule_cid.to_hex());
            Ok(())
        }
        Verdict::Reject { stage, reason } => {
            println!("REJECT [{stage:?}] {reason}");
            Err(POLICY_NEGATIVE.to_string())
        }
    }
}

// ───────────────────────────────── render ─────────────────────────────────

fn cmd_render(args: &[String]) -> CliResult {
    let path = single_path(args, "render")?;
    let capsule = read_capsule(path)?;
    let registry = registry_v0();
    require_admit_for_review(path, &capsule, &registry)?;
    let m = manifest(&capsule, &registry).map_err(|e| format!("render: {e:?}"))?;
    print!("{}", render_text(&m));
    Ok(())
}

// ────────────────────────────────── diff ──────────────────────────────────

fn cmd_diff(args: &[String]) -> CliResult {
    if args.len() != 2 {
        return Err("diff needs <old.braid> <new.braid>".into());
    }
    let registry = registry_v0();
    let old = read_capsule(&args[0])?;
    let new = read_capsule(&args[1])?;
    require_admit_for_review(&args[0], &old, &registry)?;
    require_admit_for_review(&args[1], &new, &registry)?;
    let m_old = manifest(&old, &registry).map_err(|e| format!("render old: {e:?}"))?;
    let m_new = manifest(&new, &registry).map_err(|e| format!("render new: {e:?}"))?;
    let deltas = manifest_diff(&m_old, &m_new);

    if deltas.is_empty() {
        println!("no change");
        return Ok(());
    }
    for d in &deltas {
        let tag = match d.kind {
            DeltaKind::Widening => "WIDENING",
            DeltaKind::Narrowing => "narrowing",
            DeltaKind::Neutral => "neutral",
        };
        println!("{tag:9} {}: {}", d.field, d.detail);
    }
    // The CI gate's one-bit answer (T12): any widening fails the run.
    if has_widening(&deltas) {
        Err(POLICY_NEGATIVE.to_string())
    } else {
        Ok(())
    }
}

// ───────────────────────────────── helpers ─────────────────────────────────

fn parse_confirm(s: &str) -> Result<ConfirmPolicy, String> {
    match s {
        "none" => Ok(ConfirmPolicy::None),
        "human-confirm" => Ok(ConfirmPolicy::HumanConfirm),
        other => Err(format!(
            "invalid confirm `{other}` (expected `none` or `human-confirm`)"
        )),
    }
}

fn confirm_str(c: ConfirmPolicy) -> &'static str {
    match c {
        ConfirmPolicy::None => "none",
        ConfirmPolicy::HumanConfirm => "human-confirm",
    }
}

fn single_path<'a>(args: &'a [String], cmd: &str) -> Result<&'a str, String> {
    match args {
        [p] if !p.starts_with('-') => Ok(p.as_str()),
        _ => Err(format!("{cmd} needs exactly one <path>")),
    }
}

fn read_text(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))
}

fn read_bytes(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("read {path}: {e}"))
}

/// Strict-decode a capsule file (bijection-guarded canonical bytes only).
fn read_capsule(path: &str) -> Result<Capsule, String> {
    let bytes = read_bytes(path)?;
    Capsule::from_bytes(&bytes).map_err(|e| format!("{path}: not a canonical capsule: {e:?}"))
}

/// Human-review outputs are projections of admitted artifacts, not a parallel
/// path around the verifier. Use the capsule's declared grants as the ambient
/// authority, matching `verify`'s default happy-path check.
fn require_admit_for_review(
    path: &str,
    capsule: &Capsule,
    registry: &braid_ir::TermRegistry,
) -> CliResult {
    let bytes = capsule.to_bytes();
    match verify(&bytes, registry, &capsule.grants) {
        Verdict::Admit { .. } => Ok(()),
        Verdict::Reject { stage, reason } => Err(format!(
            "{path}: not admitted for review [{stage:?}] {reason}"
        )),
    }
}
