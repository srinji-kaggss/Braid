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
    UnknownTerm(String),
}

fn effect_name(e: EffectClass) -> &'static str {
    match e {
        EffectClass::Pure => "pure",
        EffectClass::Read => "read",
        EffectClass::ReversibleWrite => "reversible-write",
        EffectClass::Irreversible => "irreversible",
        EffectClass::Egress => "egress",
    }
}

/// Derive the manifest. Same (capsule, registry) ⇒ same manifest, always.
pub fn manifest(capsule: &Capsule, registry: &TermRegistry) -> Result<Manifest, RenderError> {
    let mut effects = BTreeSet::new();
    let mut irreversible = 0u32;
    let mut egress = 0u32;
    let mut total_cost = 0u64;
    for s in &capsule.braid.strands {
        let spec = registry
            .get(&s.term)
            .ok_or_else(|| RenderError::UnknownTerm(s.term.clone()))?;
        effects.insert(effect_name(spec.effect).to_string());
        match spec.effect {
            EffectClass::Irreversible => irreversible += 1,
            EffectClass::Egress => egress += 1,
            _ => {}
        }
        total_cost = total_cost.saturating_add(spec.cost);
    }
    let mut capabilities: Vec<String> = capsule.grants.iter().map(|c| c.to_string()).collect();
    capabilities.sort();
    Ok(Manifest {
        capsule_cid: capsule.cid(),
        intent: capsule.intent.clone(),
        ir_version: capsule.ir_version,
        vocab_version: capsule.vocab_version,
        registry_cid: capsule.registry_cid,
        capabilities,
        effects: effects.into_iter().collect(),
        irreversible_strands: irreversible,
        egress_strands: egress,
        strand_count: capsule.braid.strands.len() as u32,
        total_cost,
        budget: capsule.budget,
        confirm: capsule.confirm,
        evidence: capsule.evidence.clone(),
    })
}

/// Escape control characters in a manifest field value so a single logical
/// field can NEVER produce more than one manifest line (threat R3: the
/// manifest is the review object; a `\n` in `intent`/`evidence`/etc. would
/// inject forged `capsule:`/`capabilities:` lines that a scanning reviewer
/// could mistake for the real binding). The manifest is line-oriented
/// `key: value`; the only raw newlines in the output are the ones this
/// emitter inserts between fields. Backslash is escaped first so the mapping
/// is unambiguous and reversible.
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

/// Deterministic text rendering — what a reviewer (or a PR diff) reads.
pub fn render_text(m: &Manifest) -> String {
    let mut out = String::new();
    let mut line = |k: &str, v: String| {
        out.push_str(k);
        out.push_str(": ");
        out.push_str(&v);
        out.push('\n');
    };
    line("capsule", m.capsule_cid.to_hex());
    line("intent", escape_field(&m.intent));
    line("ir_version", m.ir_version.to_string());
    line("vocab_version", m.vocab_version.to_string());
    line("registry", m.registry_cid.to_hex());
    line(
        "capabilities",
        if m.capabilities.is_empty() {
            "(none)".into()
        } else {
            m.capabilities
                .iter()
                .map(|c| escape_field(c))
                .collect::<Vec<_>>()
                .join(", ")
        },
    );
    line(
        "effects",
        m.effects
            .iter()
            .map(|e| escape_field(e))
            .collect::<Vec<_>>()
            .join(", "),
    );
    line("irreversible_strands", m.irreversible_strands.to_string());
    line("egress_strands", m.egress_strands.to_string());
    line("strands", m.strand_count.to_string());
    line("cost", format!("{} / budget {}", m.total_cost, m.budget));
    line(
        "confirm",
        match m.confirm {
            ConfirmPolicy::None => "none".into(),
            ConfirmPolicy::HumanConfirm => "human-confirm".into(),
        },
    );
    line(
        "evidence",
        if m.evidence.is_empty() {
            "(none)".into()
        } else {
            m.evidence
                .iter()
                .map(|e| escape_field(e))
                .collect::<Vec<_>>()
                .join(", ")
        },
    );
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

/// Mechanical widening classification (T12): authority and effect growth are
/// facts the gate computes, never impressions a tired reviewer forms.
pub fn manifest_diff(old: &Manifest, new: &Manifest) -> Vec<Delta> {
    let mut deltas = Vec::new();
    let set = |v: &[String]| -> BTreeSet<String> { v.iter().cloned().collect() };

    if old.capsule_cid != new.capsule_cid {
        deltas.push(Delta {
            kind: DeltaKind::Neutral,
            field: "capsule",
            detail: format!(
                "{} -> {}",
                old.capsule_cid.to_hex(),
                new.capsule_cid.to_hex()
            ),
        });
    }

    let (old_caps, new_caps) = (set(&old.capabilities), set(&new.capabilities));
    for added in new_caps.difference(&old_caps) {
        deltas.push(Delta {
            kind: DeltaKind::Widening,
            field: "capabilities",
            detail: format!("+{added}"),
        });
    }
    for removed in old_caps.difference(&new_caps) {
        deltas.push(Delta {
            kind: DeltaKind::Narrowing,
            field: "capabilities",
            detail: format!("-{removed}"),
        });
    }

    let (old_fx, new_fx) = (set(&old.effects), set(&new.effects));
    for added in new_fx.difference(&old_fx) {
        deltas.push(Delta {
            kind: DeltaKind::Widening,
            field: "effects",
            detail: format!("+{added}"),
        });
    }
    for removed in old_fx.difference(&new_fx) {
        deltas.push(Delta {
            kind: DeltaKind::Narrowing,
            field: "effects",
            detail: format!("-{removed}"),
        });
    }

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

    // Dropping human confirmation is the sharpest widening there is — but
    // only while the NEW capsule still contains dangerous strands; a capsule
    // that no longer has anything to confirm hasn't widened by not asking.
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

    if old.intent != new.intent {
        deltas.push(Delta {
            kind: DeltaKind::Neutral,
            field: "intent",
            detail: "changed".into(),
        });
    }
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
    if old.evidence != new.evidence {
        deltas.push(Delta {
            kind: DeltaKind::Neutral,
            field: "evidence",
            detail: "changed".into(),
        });
    }
    deltas
}

/// True iff the diff contains any widening (the CI gate's one-bit answer).
pub fn has_widening(deltas: &[Delta]) -> bool {
    deltas.iter().any(|d| d.kind == DeltaKind::Widening)
}

// ───────────────────────────── graph export ─────────────────────────────

/// Deterministic DOT export of the strand DAG (D17). Node label = index +
/// term id + effect class; edges follow value flow.
pub fn to_dot(capsule: &Capsule, registry: &TermRegistry) -> Result<String, RenderError> {
    let mut out = String::from("digraph braid {\n  rankdir=LR;\n");
    for (i, s) in capsule.braid.strands.iter().enumerate() {
        let spec = registry
            .get(&s.term)
            .ok_or_else(|| RenderError::UnknownTerm(s.term.clone()))?;
        out.push_str(&format!(
            "  s{i} [label=\"{i}: {} [{}]\"];\n",
            s.term,
            effect_name(spec.effect)
        ));
        for &input_idx in &s.inputs {
            out.push_str(&format!("  s{input_idx} -> s{i};\n"));
        }
    }
    for &o in &capsule.braid.outputs {
        out.push_str(&format!("  s{o} [peripheries=2];\n"));
    }
    out.push_str("}\n");
    Ok(out)
}
