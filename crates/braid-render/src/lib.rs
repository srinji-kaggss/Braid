//! # braid-render — the IR→human direction (ADR-088 D12/D17, U2 #559)
//!
//! Two deterministic translations of a capsule for the human side of the
//! meeting layer: the **manifest** (the review object — D12) and the
//! **braid-graph DOT export** (D17's "translation/graph stuff").
//!
//! The manifest is BOUND to the capsule CID it was rendered from; the
//! widening classifier makes capability/effect growth a mechanical fact,
//! not a reviewer's impression (threat T12).

use braid_ir::term::EffectClass;
use braid_ir::{Capsule, Cid, ConfirmPolicy, TermRegistry};
use std::collections::BTreeSet;
use std::fmt;

/// The deterministic human-review object, derived from (capsule, registry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// The capsule this manifest renders — the binding (T4).
    pub capsule_cid: Cid,
    pub intent: String,
    pub ir_version: u32,
    pub vocab_version: u32,
    pub registry_cid: Cid,
    /// Sorted capability names.
    pub capabilities: Vec<String>,
    /// Sorted effect-class names present in the braid.
    pub effects: Vec<String>,
    pub irreversible_strands: u32,
    pub egress_strands: u32,
    pub strand_count: u32,
    pub total_cost: u64,
    pub budget: u64,
    pub confirm: ConfirmPolicy,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// A strand references a term the registry doesn't know — a manifest for
    /// an unverifiable capsule must not exist (fail-closed, L9).
    UnknownTerm {
        term: String,
        at: &'static str,
    },
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTerm { term, at } => write!(f, "unknown term `{term}` at {at}"),
        }
    }
}

impl std::error::Error for RenderError {}

fn effect_name(e: EffectClass) -> &'static str {
    match e {
        EffectClass::Pure => "pure",
        EffectClass::Read => "read",
        EffectClass::ReversibleWrite => "reversible-write",
        EffectClass::Irreversible => "irreversible",
        EffectClass::Egress => "egress",
    }
}

#[derive(Default)]
struct StrandStats {
    effects: BTreeSet<String>,
    irreversible: u32,
    egress: u32,
    total_cost: u64,
}

fn accumulate_strand_stat(
    registry: &TermRegistry,
    term: &str,
    stats: &mut StrandStats,
) -> Result<(), RenderError> {
    let spec = registry.get(term).ok_or_else(|| RenderError::UnknownTerm {
        term: term.to_string(),
        at: "manifest",
    })?;
    stats.effects.insert(effect_name(spec.effect).to_string());
    match spec.effect {
        EffectClass::Irreversible => stats.irreversible += 1,
        EffectClass::Egress => stats.egress += 1,
        _ => {}
    }
    stats.total_cost = stats.total_cost.saturating_add(spec.cost);
    Ok(())
}

fn sorted_capabilities(capsule: &Capsule) -> Vec<String> {
    let mut capabilities: Vec<String> = capsule.grants.iter().map(|c| c.to_string()).collect();
    capabilities.sort();
    capabilities
}

/// Derive the manifest. Same (capsule, registry) ⇒ same manifest, always.
pub fn manifest(capsule: &Capsule, registry: &TermRegistry) -> Result<Manifest, RenderError> {
    let mut stats = StrandStats::default();
    for s in &capsule.braid.strands {
        accumulate_strand_stat(registry, &s.term, &mut stats)?;
    }
    let capabilities = sorted_capabilities(capsule);
    Ok(Manifest {
        capsule_cid: capsule.cid(),
        intent: capsule.intent.clone(),
        ir_version: capsule.ir_version,
        vocab_version: capsule.vocab_version,
        registry_cid: capsule.registry_cid,
        capabilities,
        effects: stats.effects.into_iter().collect(),
        irreversible_strands: stats.irreversible,
        egress_strands: stats.egress,
        strand_count: capsule.braid.strands.len() as u32,
        total_cost: stats.total_cost,
        budget: capsule.budget,
        confirm: capsule.confirm,
        evidence: capsule.evidence.clone(),
    })
}

/// Escape control characters in a manifest field value so a single logical
/// field can NEVER produce more than one manifest line.
fn escape_field(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

fn format_list_field(items: &[String]) -> String {
    if items.is_empty() {
        "(none)".into()
    } else {
        items.iter().map(|item| escape_field(item)).collect::<Vec<_>>().join(", ")
    }
}

fn push_manifest_line(out: &mut String, k: &str, v: &str) {
    out.push_str(k);
    out.push_str(": ");
    out.push_str(v);
    out.push('\n');
}

fn render_header_lines(m: &Manifest, out: &mut String) {
    push_manifest_line(out, "capsule", &m.capsule_cid.to_hex());
    push_manifest_line(out, "intent", &escape_field(&m.intent));
    push_manifest_line(out, "ir_version", &m.ir_version.to_string());
    push_manifest_line(out, "vocab_version", &m.vocab_version.to_string());
    push_manifest_line(out, "registry", &m.registry_cid.to_hex());
}

fn render_strand_metrics(m: &Manifest, out: &mut String) {
    push_manifest_line(out, "capabilities", &format_list_field(&m.capabilities));
    push_manifest_line(out, "effects", &format_list_field(&m.effects));
    push_manifest_line(out, "irreversible_strands", &m.irreversible_strands.to_string());
    push_manifest_line(out, "egress_strands", &m.egress_strands.to_string());
}

fn render_policy_metrics(m: &Manifest, out: &mut String) {
    push_manifest_line(out, "strands", &m.strand_count.to_string());
    push_manifest_line(out, "cost", &format!("{} / budget {}", m.total_cost, m.budget));
    let confirm_str = match m.confirm {
        ConfirmPolicy::None => "none",
        ConfirmPolicy::HumanConfirm => "human-confirm",
    };
    push_manifest_line(out, "confirm", confirm_str);
    push_manifest_line(out, "evidence", &format_list_field(&m.evidence));
}

/// Deterministic text rendering — what a reviewer (or a PR diff) reads.
pub fn render_text(m: &Manifest) -> String {
    let mut out = String::new();
    render_header_lines(m, &mut out);
    render_strand_metrics(m, &mut out);
    render_policy_metrics(m, &mut out);
    out
}

// ───────────────────────────── widening diff ─────────────────────────────

/// Classification of one manifest change. Widenings are what CI gates on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeltaKind {
    Widening,
    Narrowing,
    Neutral,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delta {
    pub kind: DeltaKind,
    pub field: &'static str,
    pub detail: String,
}

fn diff_capabilities(old: &Manifest, new: &Manifest, deltas: &mut Vec<Delta>) {
    let old_set: BTreeSet<String> = old.capabilities.iter().cloned().collect();
    let new_set: BTreeSet<String> = new.capabilities.iter().cloned().collect();
    for added in new_set.difference(&old_set) {
        deltas.push(Delta {
            kind: DeltaKind::Widening,
            field: "capabilities",
            detail: format!("+{added}"),
        });
    }
    for removed in old_set.difference(&new_set) {
        deltas.push(Delta {
            kind: DeltaKind::Narrowing,
            field: "capabilities",
            detail: format!("-{removed}"),
        });
    }
}

fn diff_effects(old: &Manifest, new: &Manifest, deltas: &mut Vec<Delta>) {
    let old_set: BTreeSet<String> = old.effects.iter().cloned().collect();
    let new_set: BTreeSet<String> = new.effects.iter().cloned().collect();
    for added in new_set.difference(&old_set) {
        deltas.push(Delta {
            kind: DeltaKind::Widening,
            field: "effects",
            detail: format!("+{added}"),
        });
    }
    for removed in old_set.difference(&new_set) {
        deltas.push(Delta {
            kind: DeltaKind::Narrowing,
            field: "effects",
            detail: format!("-{removed}"),
        });
    }
}

fn diff_budget(old: &Manifest, new: &Manifest, deltas: &mut Vec<Delta>) {
    if new.budget > old.budget {
        deltas.push(Delta {
            kind: DeltaKind::Widening,
            field: "budget",
            detail: format!("{} -> {}", old.budget, new.budget),
        });
    } else if new.budget < old.budget {
        deltas.push(Delta {
            kind: DeltaKind::Narrowing,
            field: "budget",
            detail: format!("{} -> {}", old.budget, new.budget),
        });
    }
}

fn diff_confirm(old: &Manifest, new: &Manifest, deltas: &mut Vec<Delta>) {
    if old.confirm == ConfirmPolicy::HumanConfirm && new.confirm == ConfirmPolicy::None {
        let still_dangerous = new.irreversible_strands + new.egress_strands > 0;
        deltas.push(Delta {
            kind: if still_dangerous {
                DeltaKind::Widening
            } else {
                DeltaKind::Neutral
            },
            field: "confirm",
            detail: "human-confirm -> none".into(),
        });
    } else if old.confirm == ConfirmPolicy::None && new.confirm == ConfirmPolicy::HumanConfirm {
        deltas.push(Delta {
            kind: DeltaKind::Narrowing,
            field: "confirm",
            detail: "none -> human-confirm".into(),
        });
    }
}

fn diff_meta_fields(old: &Manifest, new: &Manifest, deltas: &mut Vec<Delta>) {
    if old.capsule_cid != new.capsule_cid {
        deltas.push(Delta {
            kind: DeltaKind::Neutral,
            field: "capsule",
            detail: format!("{} -> {}", old.capsule_cid.to_hex(), new.capsule_cid.to_hex()),
        });
    }
    if old.intent != new.intent {
        deltas.push(Delta {
            kind: DeltaKind::Neutral,
            field: "intent",
            detail: "changed".into(),
        });
    }
    if old.evidence != new.evidence {
        deltas.push(Delta {
            kind: DeltaKind::Neutral,
            field: "evidence",
            detail: "changed".into(),
        });
    }
}

fn diff_strand_counts(old: &Manifest, new: &Manifest, deltas: &mut Vec<Delta>) {
    if old.strand_count != new.strand_count {
        deltas.push(Delta {
            kind: DeltaKind::Neutral,
            field: "strands",
            detail: format!("{} -> {}", old.strand_count, new.strand_count),
        });
    }
    if old.total_cost != new.total_cost {
        deltas.push(Delta {
            kind: DeltaKind::Neutral,
            field: "cost",
            detail: format!("{} -> {}", old.total_cost, new.total_cost),
        });
    }
}

fn diff_effect_strand_counts(old: &Manifest, new: &Manifest, deltas: &mut Vec<Delta>) {
    if old.irreversible_strands != new.irreversible_strands {
        deltas.push(Delta {
            kind: DeltaKind::Neutral,
            field: "irreversible_strands",
            detail: format!(
                "{} -> {}",
                old.irreversible_strands, new.irreversible_strands
            ),
        });
    }
    if old.egress_strands != new.egress_strands {
        deltas.push(Delta {
            kind: DeltaKind::Neutral,
            field: "egress_strands",
            detail: format!("{} -> {}", old.egress_strands, new.egress_strands),
        });
    }
}

/// Mechanical widening classification (T12): authority and effect growth are
/// facts the gate computes, never impressions a tired reviewer forms.
pub fn manifest_diff(old: &Manifest, new: &Manifest) -> Vec<Delta> {
    let mut deltas = Vec::new();
    diff_meta_fields(old, new, &mut deltas);
    diff_strand_counts(old, new, &mut deltas);
    diff_effect_strand_counts(old, new, &mut deltas);
    diff_capabilities(old, new, &mut deltas);
    diff_effects(old, new, &mut deltas);
    diff_budget(old, new, &mut deltas);
    diff_confirm(old, new, &mut deltas);
    deltas
}

/// True iff the diff contains any widening (the CI gate's one-bit answer).
pub fn has_widening(deltas: &[Delta]) -> bool {
    deltas.iter().any(|d| d.kind == DeltaKind::Widening)
}

// ───────────────────────────── graph export ─────────────────────────────

fn render_dot_strand(
    out: &mut String,
    strand_idx: usize,
    term: &str,
    spec_effect: EffectClass,
    inputs: &[u32],
) {
    out.push_str(&format!(
        "  s{strand_idx} [label=\"{strand_idx}: {} [{}]\"];\n",
        term,
        effect_name(spec_effect)
    ));
    for &input_idx in inputs {
        out.push_str(&format!("  s{input_idx} -> s{strand_idx};\n"));
    }
}

fn render_dot_output_nodes(out: &mut String, outputs: &[u32]) {
    for &o in outputs {
        out.push_str(&format!("  s{o} [peripheries=2];\n"));
    }
}

/// Deterministic DOT export of the strand DAG (D17). Node label = index +
/// term id + effect class; edges follow value flow.
pub fn to_dot(capsule: &Capsule, registry: &TermRegistry) -> Result<String, RenderError> {
    let mut out = String::from("digraph braid {\n  rankdir=LR;\n");
    for (strand_idx, s) in capsule.braid.strands.iter().enumerate() {
        let spec = registry
            .get(&s.term)
            .ok_or_else(|| RenderError::UnknownTerm {
                term: s.term.clone(),
                at: "to_dot",
            })?;
        render_dot_strand(&mut out, strand_idx, &s.term, spec.effect, &s.inputs);
    }
    render_dot_output_nodes(&mut out, &capsule.braid.outputs);
    out.push_str("}\n");
    Ok(out)
}
