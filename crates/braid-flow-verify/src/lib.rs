//! Independent Flow graph admission verifier — P2 #60.
//!
//! Own decoder (preflight + projection) so producer ≠ admission.
//! Fail-closed typed refusals per INV-FLOW-*.

#![forbid(unsafe_code)]
#![deny(warnings)]

extern crate alloc;

pub mod decode;
pub mod disjoint;
pub mod error;
pub mod verify;

pub use disjoint::{
    CompletionBinding, CompletionWitness, DISJOINTNESS_FRAGMENT_VERSION,
    DISJOINTNESS_MAX_NORMAL_FORM_ATOMS, DISJOINTNESS_MAX_NORMAL_FORM_CLAUSES,
    DISJOINTNESS_MAX_PREDICATE_DEPTH, DISJOINTNESS_MAX_PREDICATE_NODES, DISJOINTNESS_MAX_WORK,
    Disjointness, DisjointnessUnknown, PredicateCounterexample, SolverLimit, ValueBinding,
    analyze_disjointness,
};
pub use error::{ChoiceOverlap, FlowVerifyError, VerifyResult};
pub use verify::{AdmittedFlow, verify};
