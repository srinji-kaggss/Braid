//! # braid-sdk — typed capsule authoring (ADR-088 D5 "rust day 1", U10)
//!
//! The ergonomic Rust path for constructing Braid capsules. It is a *builder*,
//! not a second verifier: it performs the author-time structural checks the
//! verifier will repeat (type, arity, grant coverage, budget) so an agent gets
//! a typed error at construction instead of a `Reject` later — but the
//! verifier remains the sole authority (D9: the SDK never decides admission).
//!
//! Two invariants the SDK makes *unrepresentable* (stronger than a check):
//! - **No forward reference / no cycle**: [`Strand`] handles are returned by
//!   [`Builder::strand`] and can only reference already-added strands, so the
//!   DAG's index order — the verifier's topological order — holds by
//!   construction.
//! - **No undeclared capability**: grants are *collected* from the terms used,
//!   not declared separately, so a capsule cannot use a capability it failed
//!   to request (the omission that scenario #4b rejects can't be authored).
//!
//! Boundary (D3): depends only on `braid-ir` + `braid-capability`.

use braid_ir::braid::{Braid, Strand as IrStrand};
use braid_ir::term::TypeTag;
use braid_ir::{Capsule, ConfirmPolicy, EffectClass, TermRegistry, IR_VERSION};
use braid_capability::Capability;

/// A typed reference to a strand already placed in the braid. Carries its
/// output type so wiring is type-checked the moment it is used as an input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Strand {
    index: u32,
    // //why store the type here and not look it up: the handle is the only way
    // to reference a strand, so carrying the type makes a mis-wire a
    // compile-adjacent error at the call site, not a deferred verdict.
    ty: TypeTagId,
}

impl Strand {
    pub fn index(&self) -> u32 {
        self.index
    }
}

/// Interned type tag (Copy) so [`Strand`] stays `Copy` — `TypeTag` is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TypeTagId(usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    UnknownTerm(String),
    Arity { term: String, expected: usize, got: usize },
    TypeMismatch { term: String, slot: usize, expected: TypeTag, got: TypeTag },
    NoOutputs,
    /// A declared budget below the composed cost (the SDK refuses to author an
    /// over-budget capsule rather than emit one the verifier will reject).
    BudgetTooLow { needed: u64, set: u64 },
    /// Irreversible/egress term used without a confirm policy set.
    ConfirmRequired,
}

/// Capsule builder bound to a registry.
pub struct Builder<'r> {
    registry: &'r TermRegistry,
    intent: String,
    strands: Vec<IrStrand>,
    outputs: Vec<u32>,
    /// Parallel to `strands`: the interned output type of each.
    out_types: Vec<TypeTag>,
    type_interner: Vec<TypeTag>,
    // //why a Vec + membership check, not a set: `Capability` is not `Ord`
    // (kernel contract — D3 forbids us adding a derive to it), so we dedup by
    // the protocol-stable name and sort on emit to hit the canonical order.
    grants: Vec<Capability>,
    cost: u64,
    has_dangerous: bool,
    budget: Option<u64>,
    confirm: Option<ConfirmPolicy>,
    evidence: Vec<String>,
}

impl<'r> Builder<'r> {
    pub fn new(registry: &'r TermRegistry, intent: impl Into<String>) -> Self {
        Builder {
            registry,
            intent: intent.into(),
            strands: Vec::new(),
            outputs: Vec::new(),
            out_types: Vec::new(),
            type_interner: Vec::new(),
            grants: Vec::new(),
            cost: 0,
            has_dangerous: false,
            budget: None,
            confirm: None,
            evidence: Vec::new(),
        }
    }

    fn intern(&mut self, ty: &TypeTag) -> TypeTagId {
        if let Some(i) = self.type_interner.iter().position(|t| t == ty) {
            TypeTagId(i)
        } else {
            self.type_interner.push(ty.clone());
            TypeTagId(self.type_interner.len() - 1)
        }
    }

    /// Place a strand, type- and arity-checking its inputs against the
    /// registry. Returns a handle usable as input to later strands.
    pub fn strand(&mut self, term_id: &str, inputs: &[Strand]) -> Result<Strand, BuildError> {
        let spec = self
            .registry
            .get(term_id)
            .ok_or_else(|| BuildError::UnknownTerm(term_id.to_string()))?;
        if spec.inputs.len() != inputs.len() {
            return Err(BuildError::Arity {
                term: term_id.to_string(),
                expected: spec.inputs.len(),
                got: inputs.len(),
            });
        }
        for (slot, (h, expected)) in inputs.iter().zip(spec.inputs.iter()).enumerate() {
            let got = &self.type_interner[h.ty.0];
            if got != expected {
                return Err(BuildError::TypeMismatch {
                    term: term_id.to_string(),
                    slot,
                    expected: expected.clone(),
                    got: got.clone(),
                });
            }
        }
        if let Some(cap) = &spec.capability {
            if !self.grants.iter().any(|g| g.to_string() == cap.to_string()) {
                self.grants.push(cap.clone());
            }
        }
        if matches!(spec.effect, EffectClass::Irreversible | EffectClass::Egress) {
            self.has_dangerous = true;
        }
        self.cost = self.cost.saturating_add(spec.cost);

        let index = self.strands.len() as u32;
        let ty = self.intern(&spec.output);
        self.strands.push(IrStrand {
            term: term_id.to_string(),
            inputs: inputs.iter().map(|h| h.index).collect(),
        });
        self.out_types.push(self.type_interner[ty.0].clone());
        Ok(Strand { index, ty })
    }

    /// Mark a strand as a capsule output.
    pub fn output(&mut self, s: Strand) -> &mut Self {
        self.outputs.push(s.index);
        self
    }

    pub fn budget(&mut self, b: u64) -> &mut Self {
        self.budget = Some(b);
        self
    }

    /// Size the budget exactly to the composed cost (convenience for tight
    /// authoring; the verifier still checks).
    pub fn budget_tight(&mut self) -> &mut Self {
        self.budget = Some(self.cost);
        self
    }

    pub fn confirm(&mut self, c: ConfirmPolicy) -> &mut Self {
        self.confirm = Some(c);
        self
    }

    pub fn evidence(&mut self, key: impl Into<String>) -> &mut Self {
        self.evidence.push(key.into());
        self
    }

    /// Finalize. Grants are emitted sorted+deduped (canonical order); a
    /// dangerous capsule without `HumanConfirm` is refused at author time.
    pub fn build(self) -> Result<Capsule, BuildError> {
        if self.outputs.is_empty() {
            return Err(BuildError::NoOutputs);
        }
        let budget = self.budget.unwrap_or(self.cost);
        if budget < self.cost {
            return Err(BuildError::BudgetTooLow { needed: self.cost, set: budget });
        }
        let confirm = match self.confirm {
            Some(c) => c,
            None if self.has_dangerous => return Err(BuildError::ConfirmRequired),
            None => ConfirmPolicy::None,
        };
        if self.has_dangerous && confirm != ConfirmPolicy::HumanConfirm {
            return Err(BuildError::ConfirmRequired);
        }
        let mut grants = self.grants;
        grants.sort_by_key(|c| c.to_string());

        Ok(Capsule {
            ir_version: IR_VERSION,
            vocab_version: self.registry.vocab_version,
            registry_cid: self.registry.cid(),
            intent: self.intent,
            grants,
            braid: Braid {
                strands: self.strands,
                outputs: self.outputs,
            },
            budget,
            confirm,
            evidence: self.evidence,
        })
    }
}
