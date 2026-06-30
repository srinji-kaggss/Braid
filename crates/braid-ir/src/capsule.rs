//! The capsule: the admitted artifact (PRD §4.3). Canonical bytes → CID; the
//! CID is what admission, the manifest, and the runtime all bind to.

use crate::braid::Braid;
use crate::canon::{self, CanonError};
use crate::cid::{Cid, CAPSULE_DOMAIN};
use crate::term::RegistryError;
use crate::value::Value;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use braid_capability::Capability;
use core::str::FromStr;

/// Braid IR version. Bumped on ANY change to capsule/braid/registry canonical
/// shape; admission refuses a mismatch (D11). Pinned by `tests/kat.rs`.
pub const IR_VERSION: u32 = 0;

/// Confirmation policy (T10 static half; the runtime payload-hash binding is
/// the U7 half).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmPolicy {
    None,
    HumanConfirm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capsule {
    pub ir_version: u32,
    pub vocab_version: u32,
    /// Content address of the EXACT registry this capsule was authored
    /// against (T6) — not a version number, the bytes.
    pub registry_cid: Cid,
    /// Declared purpose (rendered verbatim in the manifest).
    pub intent: String,
    /// Requested capability set. Canonical form: sorted by capability name,
    /// no duplicates (grant-order malleability is a rejected byte form).
    pub grants: Vec<Capability>,
    pub braid: Braid,
    /// Total cost budget (abstract units; T7 static half).
    pub budget: u64,
    pub confirm: ConfirmPolicy,
    /// Evidence keys to retain on execution (journaled by the runtime).
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapsuleError {
    Canon(CanonError),
    Malformed(&'static str),
}

impl From<CanonError> for CapsuleError {
    fn from(e: CanonError) -> Self {
        CapsuleError::Canon(e)
    }
}

impl From<RegistryError> for CapsuleError {
    fn from(_: RegistryError) -> Self {
        CapsuleError::Malformed("braid")
    }
}

impl Capsule {
    pub fn to_canon(&self) -> Value {
        Value::map(vec![
            ("braid", self.braid.to_canon()),
            ("budget", Value::Int(self.budget as i64)),
            (
                "grants",
                Value::List(
                    self.grants
                        .iter()
                        .map(|c| Value::Text(c.to_string()))
                        .collect(),
                ),
            ),
            ("intent", Value::Text(self.intent.clone())),
            (
                "confirm",
                Value::Text(
                    match self.confirm {
                        ConfirmPolicy::None => "none",
                        ConfirmPolicy::HumanConfirm => "human-confirm",
                    }
                    .into(),
                ),
            ),
            (
                "evidence",
                Value::List(
                    self.evidence
                        .iter()
                        .map(|e| Value::Text(e.clone()))
                        .collect(),
                ),
            ),
            ("ir_version", Value::Int(self.ir_version as i64)),
            ("registry_cid", Value::Bytes(self.registry_cid.0.to_vec())),
            ("vocab_version", Value::Int(self.vocab_version as i64)),
        ])
    }

    pub fn from_canon(v: &Value) -> Result<Self, CapsuleError> {
        let u32_field = |key: &'static str| -> Result<u32, CapsuleError> {
            match v.get(key) {
                Some(Value::Int(n)) if *n >= 0 && *n <= u32::MAX as i64 => Ok(*n as u32),
                _ => Err(CapsuleError::Malformed(key)),
            }
        };
        let ir_version = u32_field("ir_version")?;
        let vocab_version = u32_field("vocab_version")?;
        let registry_cid = match v.get("registry_cid") {
            Some(Value::Bytes(b)) if b.len() == 32 => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(b);
                Cid(arr)
            }
            _ => return Err(CapsuleError::Malformed("registry_cid")),
        };
        let intent = match v.get("intent") {
            Some(Value::Text(s)) => s.clone(),
            _ => return Err(CapsuleError::Malformed("intent")),
        };
        let grants = match v.get("grants") {
            Some(Value::List(items)) => {
                let mut out: Vec<Capability> = Vec::with_capacity(items.len());
                let mut prev: Option<String> = None;
                for it in items {
                    let name = match it {
                        Value::Text(s) => s.clone(),
                        _ => return Err(CapsuleError::Malformed("grant")),
                    };
                    // Canonical: strictly increasing names ⇒ sorted + deduped.
                    if let Some(p) = &prev {
                        if p.as_str() >= name.as_str() {
                            return Err(CapsuleError::Malformed("grant order"));
                        }
                    }
                    let cap = Capability::from_str(&name)
                        .map_err(|_| CapsuleError::Malformed("unknown grant"))?;
                    prev = Some(name);
                    out.push(cap);
                }
                out
            }
            _ => return Err(CapsuleError::Malformed("grants")),
        };
        let braid = match v.get("braid") {
            Some(b) => Braid::from_canon(b)?,
            None => return Err(CapsuleError::Malformed("braid")),
        };
        let budget = match v.get("budget") {
            Some(Value::Int(n)) if *n >= 0 => *n as u64,
            _ => return Err(CapsuleError::Malformed("budget")),
        };
        let confirm = match v.get("confirm") {
            Some(Value::Text(s)) => match s.as_str() {
                "none" => ConfirmPolicy::None,
                "human-confirm" => ConfirmPolicy::HumanConfirm,
                _ => return Err(CapsuleError::Malformed("confirm")),
            },
            _ => return Err(CapsuleError::Malformed("confirm")),
        };
        let evidence = match v.get("evidence") {
            Some(Value::List(items)) => items
                .iter()
                .map(|i| match i {
                    Value::Text(s) => Ok(s.clone()),
                    _ => Err(CapsuleError::Malformed("evidence")),
                })
                .collect::<Result<Vec<_>, _>>()?,
            _ => return Err(CapsuleError::Malformed("evidence")),
        };
        // Exactly the known fields — an extra key is a rejected byte form,
        // not ignorable padding (T3: unknown fields are a smuggling channel).
        // Allowlist (not a bare count) so a new field can never silently widen
        // the gate; the required keys above already enforce presence.
        if !v.require_only_keys(&[
            "braid",
            "budget",
            "grants",
            "intent",
            "confirm",
            "evidence",
            "ir_version",
            "registry_cid",
            "vocab_version",
        ]) {
            return Err(CapsuleError::Malformed("capsule: unknown field"));
        }
        Ok(Capsule {
            ir_version,
            vocab_version,
            registry_cid,
            intent,
            grants,
            braid,
            budget,
            confirm,
            evidence,
        })
    }

    /// Canonical bytes (one capsule ⇒ one byte string).
    pub fn to_bytes(&self) -> Vec<u8> {
        canon::encode(&self.to_canon())
    }

    /// Strict parse: canonical bytes only (bijection-guarded), full shape
    /// validation. The authoring-side mirror of the verifier's stage 1.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CapsuleError> {
        let v = canon::decode_strict(bytes)?;
        Self::from_canon(&v)
    }

    /// Content address under [`CAPSULE_DOMAIN`].
    pub fn cid(&self) -> Cid {
        Cid::compute(CAPSULE_DOMAIN, &self.to_bytes())
    }
}
