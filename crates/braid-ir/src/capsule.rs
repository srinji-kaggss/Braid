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
    Malformed { field: &'static str, at: &'static str },
}

impl core::fmt::Display for CapsuleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Canon(err) => write!(f, "canon error: {err:?}"),
            Self::Malformed { field, at } => write!(f, "malformed {field} at {at}"),
        }
    }
}

impl core::error::Error for CapsuleError {}

impl From<CanonError> for CapsuleError {
    fn from(e: CanonError) -> Self {
        CapsuleError::Canon(e)
    }
}

impl From<RegistryError> for CapsuleError {
    fn from(_: RegistryError) -> Self {
        CapsuleError::Malformed {
            field: "braid",
            at: "Capsule::from_canon",
        }
    }
}

fn decode_u32_field(v: &Value, key: &'static str) -> Result<u32, CapsuleError> {
    match v.get_field(key) {
        Some(Value::Int(n)) if *n >= 0 && *n <= u32::MAX as i64 => Ok(*n as u32),
        _ => Err(CapsuleError::Malformed {
            field: key,
            at: "decode_u32_field",
        }),
    }
}

fn decode_registry_cid(v: &Value) -> Result<Cid, CapsuleError> {
    match v.get_field("registry_cid") {
        Some(Value::Bytes(b)) if b.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(b);
            Ok(Cid(arr))
        }
        _ => Err(CapsuleError::Malformed {
            field: "registry_cid",
            at: "decode_registry_cid",
        }),
    }
}

fn decode_intent(v: &Value) -> Result<String, CapsuleError> {
    match v.get_field("intent") {
        Some(Value::Text(s)) => Ok(s.clone()),
        _ => Err(CapsuleError::Malformed {
            field: "intent",
            at: "decode_intent",
        }),
    }
}

fn check_grant_ordering(prev: &Option<String>, name: &str) -> Result<(), CapsuleError> {
    if let Some(p) = prev.as_ref() {
        if p.as_str() >= name {
            Err(CapsuleError::Malformed {
                field: "grant order",
                at: "decode_single_grant",
            })
        } else {
            Ok(())
        }
    } else {
        Ok(())
    }
}

fn extract_grant_name(item: &Value) -> Result<String, CapsuleError> {
    match item {
        Value::Text(s) => Ok(s.clone()),
        _ => Err(CapsuleError::Malformed {
            field: "grant",
            at: "decode_single_grant",
        }),
    }
}

fn parse_grant_capability(name: &str) -> Result<Capability, CapsuleError> {
    match Capability::from_str(name) {
        Ok(c) => Ok(c),
        Err(_err) => Err(CapsuleError::Malformed {
            field: "unknown grant",
            at: "decode_single_grant",
        }),
    }
}

fn decode_single_grant(item: &Value, prev: &mut Option<String>) -> Result<Capability, CapsuleError> {
    let name = extract_grant_name(item)?;
    check_grant_ordering(prev, &name)?;
    let cap = parse_grant_capability(&name)?;
    *prev = Some(name);
    Ok(cap)
}

fn decode_grants(v: &Value) -> Result<Vec<Capability>, CapsuleError> {
    match v.get_field("grants") {
        Some(Value::List(items)) => {
            let mut out = Vec::with_capacity(items.len());
            let mut prev = None;
            for item in items {
                out.push(decode_single_grant(item, &mut prev)?);
            }
            Ok(out)
        }
        _ => Err(CapsuleError::Malformed {
            field: "grants",
            at: "decode_grants",
        }),
    }
}

fn decode_braid(v: &Value) -> Result<Braid, CapsuleError> {
    match v.get_field("braid") {
        Some(b) => Ok(Braid::from_canon(b)?),
        None => Err(CapsuleError::Malformed {
            field: "braid",
            at: "decode_braid",
        }),
    }
}

fn decode_budget(v: &Value) -> Result<u64, CapsuleError> {
    match v.get_field("budget") {
        Some(Value::Int(n)) if *n >= 0 => Ok(*n as u64),
        _ => Err(CapsuleError::Malformed {
            field: "budget",
            at: "decode_budget",
        }),
    }
}

fn decode_confirm(v: &Value) -> Result<ConfirmPolicy, CapsuleError> {
    match v.get_field("confirm") {
        Some(Value::Text(s)) => match s.as_str() {
            "none" => Ok(ConfirmPolicy::None),
            "human-confirm" => Ok(ConfirmPolicy::HumanConfirm),
            _ => Err(CapsuleError::Malformed {
                field: "confirm",
                at: "decode_confirm",
            }),
        },
        _ => Err(CapsuleError::Malformed {
            field: "confirm",
            at: "decode_confirm",
        }),
    }
}

fn decode_evidence(v: &Value) -> Result<Vec<String>, CapsuleError> {
    match v.get_field("evidence") {
        Some(Value::List(items)) => items
            .iter()
            .map(|i| match i {
                Value::Text(s) => Ok(s.clone()),
                _ => Err(CapsuleError::Malformed {
                    field: "evidence",
                    at: "decode_evidence",
                }),
            })
            .collect::<Result<Vec<_>, _>>(),
        _ => Err(CapsuleError::Malformed {
            field: "evidence",
            at: "decode_evidence",
        }),
    }
}

fn decode_capsule_meta(v: &Value) -> Result<(u32, u32, Cid, String), CapsuleError> {
    let ir_version = decode_u32_field(v, "ir_version")?;
    let vocab_version = decode_u32_field(v, "vocab_version")?;
    let registry_cid = decode_registry_cid(v)?;
    let intent = decode_intent(v)?;
    Ok((ir_version, vocab_version, registry_cid, intent))
}

fn decode_capsule_execution(
    v: &Value,
) -> Result<(Vec<Capability>, Braid, u64, ConfirmPolicy, Vec<String>), CapsuleError> {
    let grants = decode_grants(v)?;
    let braid = decode_braid(v)?;
    let budget = decode_budget(v)?;
    let confirm = decode_confirm(v)?;
    let evidence = decode_evidence(v)?;
    Ok((grants, braid, budget, confirm, evidence))
}

fn check_capsule_key_universe(v: &Value) -> Result<(), CapsuleError> {
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
        Err(CapsuleError::Malformed {
            field: "capsule: unknown field",
            at: "Capsule::from_canon",
        })
    } else {
        Ok(())
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
        check_capsule_key_universe(v)?;
        let (ir_version, vocab_version, registry_cid, intent) = decode_capsule_meta(v)?;
        let (grants, braid, budget, confirm, evidence) = decode_capsule_execution(v)?;

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
        let canon_val = canon::decode_strict(bytes)?;
        Self::from_canon(&canon_val)
    }

    /// Content address under [`CAPSULE_DOMAIN`].
    pub fn cid(&self) -> Cid {
        Cid::compute(CAPSULE_DOMAIN, &self.to_bytes())
    }
}
