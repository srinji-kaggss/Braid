//! Canonical inter-capsule Flow IR (ADR-099, P1 #57).
//!
//! A Flow says which already-admitted capsules may compose and carries the
//! deterministic justification declaration that answers why each invocation
//! should exist. It does not authorize, schedule, execute, retry, or persist
//! work. Those are separate kernel, planner, and Forge boundaries.
//!
//! The admitted-compute triad is represented without duplicating authority:
//!
//! 1. language/structural safety is inherited from the referenced admitted
//!    capsule and independently verified Flow shape;
//! 2. capability remains owned by the kernel and is derived from that capsule;
//! 3. justification is an explicit, closed declaration on the invocation.
//!
//! P1 validates and normalizes local identity shape. Predicate reference/type
//! resolution and justification admission deliberately remain independent P2
//! verifier work; constructing a `FlowSpec` is not an admission verdict.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod decode;
mod encode;
mod error;
mod flow;
mod justification_gate;
mod literal;
mod predicate;
mod preflight;
mod symbol;
mod wire_depth;

pub use crate::error::{FlowError, FlowResult, IdentifierError, LimitKind};
pub use crate::flow::{
    ChoiceArm, CompletionClass, FLOW_DOMAIN, FLOW_VERSION, FlowBounds, FlowEdge, FlowInput,
    FlowNode, FlowNodeKind, FlowSpec, InputPort, JustificationDecl, OutputPort, TerminalOutcome,
    UrgencyClass, ValueSource,
};
pub use crate::predicate::{Predicate, ValueExpr};
pub use crate::justification_gate::JustificationGate;
pub use crate::symbol::{
    CostOrderRef, FactRef, FlowName, InputKey, InvariantRef, NodeKey, PortKey, RelationRef,
};

/// A domain-safe wrapper around Braid's sole content-identity authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FlowCid(braid_ir::Cid);

impl FlowCid {
    pub(crate) const fn new(cid: braid_ir::Cid) -> Self {
        Self(cid)
    }

    pub const fn get(self) -> braid_ir::Cid {
        self.0
    }
}
