//! # braid-ir — the Braid typed term-graph IR (ADR-088, `spec/braid/`, U1 #558)
//!
//! The canonical form of a Braid program is **data**: a content-addressed,
//! canonically-encoded capsule wrapping a DAG of typed term applications.
//! This crate owns the type universe, the canonical encoding (with bijection
//! guard — threat T3), content addressing (D8 hash discipline), the closed
//! term registry shape, and the capsule artifact.
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
//! domain vocabulary into the substrate would force every consumer to fork
//! to escape it — the "good enough to not be a fork" failure. The substrate
//! is registry-parametric: `verify` and the SDK take any `TermRegistry`.
//!
//! Boundary covenant (D3): this crate depends only on `blake3` and
//! `braid-capability`. Enforced by `tests/boundary_conformance.rs`.

pub mod braid;
pub mod canon;
pub mod capsule;
pub mod cid;
pub mod term;
pub mod value;

pub use crate::braid::{Braid, Strand};
pub use crate::canon::{decode_strict, encode, CanonError};
pub use crate::capsule::{Capsule, ConfirmPolicy, IR_VERSION};
pub use crate::cid::Cid;
pub use crate::term::{EffectClass, Exposure, TermRegistry, TermSpec, TypeTag};
pub use crate::value::Value;
