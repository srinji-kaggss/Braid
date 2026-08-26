//! # braid-ir — the Braid typed term-graph IR (ADR-088, `spec/braid/`, U1 #558)
//!
//! The canonical form of a Braid program is **data**: a content-addressed,
//! canonically-encoded capsule wrapping a DAG of typed term applications.
//! This crate owns the type universe, canonical encoding, content addressing,
//! closed registry shape, capsule artifact, compact admission algebra, and the
//! registry-scoped token projection used after independent verification.
//!
//! //why no serde/ciborium: the canonical byte form is a *security surface*
//! (admission hashes it). A third-party serializer's normalization choices
//! would become unauditable parts of the trust base; the subset we need is
//! small enough to own outright, and `braid-verify` must be able to implement
//! an INDEPENDENT decoder (D9) against a spec we control.
//!
//! //why no domain vocabulary lives here (D31): Braid is a *global* IR. A
//! consumer pulls `braid-ir` for the substrate (types, encoding, CID,
//! registry shape, capsule) and a vocabulary package (`braid-vocab-cms`,
//! `braid-vocab-js`, …) for the term alphabet + capability space. Baking a
//! domain vocabulary into the substrate would force every consumer to fork.
//!
//! Boundary covenant (D3): this crate depends only on `blake3` and
//! `braid-capability`. Enforced by `tests/boundary_conformance.rs`.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod admission;
pub mod braid;
pub mod canon;
pub mod capsule;
pub mod cid;
pub mod term;
pub mod token;
pub mod value;

pub use crate::admission::{
    AdmissionAxis, AdmissionEncodingError, AdmissionTriad, InvocationDecision, ProofState,
};
pub use crate::braid::{Braid, Strand};
pub use crate::canon::{CanonError, decode_strict, encode};
pub use crate::capsule::{Capsule, ConfirmPolicy, IR_VERSION};
pub use crate::cid::{CAPSULE_DOMAIN, Cid, REGISTRY_DOMAIN};
pub use crate::term::{
    EffectClass, Exposure, TermRegistry, TermSpec, TypeTag, TypeTagError, type_tag_node_count,
    type_tag_to_text, validate_type_tag,
};
pub use crate::token::{TermTable, TermToken, TokenError, TokenOp, TokenProgram};
pub use crate::value::Value;
