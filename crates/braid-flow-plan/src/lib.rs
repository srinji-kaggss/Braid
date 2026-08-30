//! Deterministic snapshot-bound frontier planner — P3 #59.
//!
//! * `snapshot` — immutable, content-addressed fact bindings.
//! * `eval`     — three-valued, total predicate evaluation.
//! * `plan`     — satiation-first derivation, ready antichain, stable
//!   sequential selection, Plan CID, reverse-dep index.

#![forbid(unsafe_code)]
#![deny(warnings)]

extern crate alloc;

pub mod eval;
pub mod plan;
pub mod snapshot;

pub use eval::{CompletionKind, CompletionMap, eval_predicate};
pub use plan::{
    FlowPlan, PLANNER_VERSION, PlanError, PlanStep, PlanStepKind, PlanningContext, ReverseDeps,
    SatiatedTransition, plan,
};
pub use snapshot::{FlowSnapshot, MissingEvidence, ProofState};
