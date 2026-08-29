//! Snapshot binding for frontier planning (ADR-099, P3 #59).
//!
//! v0 carries an opaque, content-addressed fact map. The planner never invents
//! facts — only `Proven` from present evidence authorizes work. A future
//! increment may replace the map with a Logical-DB transaction handle, but the
//! CID binding and stale-proof rejection stay.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use braid_flow_ir::FactRef;
use braid_ir::{Cid, Value, encode};

/// Closed three-valued result for trusted predicates and proof obligations.
///
/// Only `Proven` authorizes execution or satiation. `Unknown` fails closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofState {
    Proven,
    Disproven,
    Unknown(MissingEvidence),
}

/// Opaque reason a predicate could not be closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingEvidence(pub alloc::string::String);

impl core::fmt::Display for MissingEvidence {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for MissingEvidence {}

/// Immutable, content-addressed fact snapshot that every justification proof
/// binds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowSnapshot {
    facts: BTreeMap<String, Value>,
    cid: Cid,
}

const SNAPSHOT_DOMAIN: &[u8] = b"lw.braid.flow.snapshot.v0";

impl FlowSnapshot {
    /// Build a snapshot from an explicit fact map. The CID is deterministic
    /// over the canonical encoding of the sorted map — insertion order cannot
    /// affect identity (INV-FLOW-021, INV-FLOW-023).
    pub fn new(facts: BTreeMap<String, Value>) -> Self {
        let cid = Self::compute_cid(&facts);
        Self { facts, cid }
    }

    /// Convenience for the small, test-only flow snapshots that address facts
    /// by the string form of `FactRef` (e.g. `"scope.code_changed"`).
    pub fn from_pairs(pairs: Vec<(FactRef, Value)>) -> Self {
        let map: BTreeMap<String, Value> =
            pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        Self::new(map)
    }

    pub fn cid(&self) -> Cid {
        self.cid
    }

    pub fn get(&self, fact: &FactRef) -> Option<&Value> {
        self.facts.get(&fact.to_string())
    }

    pub fn get_by_str(&self, key: &str) -> Option<&Value> {
        self.facts.get(key)
    }

    /// Canonical snapshot bytes are part of the Plan CID.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let m = Value::Map(
            self.facts
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        );
        encode(&m)
    }

    fn compute_cid(facts: &BTreeMap<String, Value>) -> Cid {
        let m = Value::Map(facts.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
        Cid::compute(SNAPSHOT_DOMAIN, &encode(&m))
    }
}
