//! P0.1 interop gate — the third axis of the Flow triad.
//!
//! The material persistence boundary is exactly `Read | Write`. The first two
//! axes are `Safe` (structural admission) and `Authorized` (kernel capability).
//! The third axis is `Justified`, which remains a loud future gate in v0.
//!
//! `JustificationGate::Deferred` MUST be visible in manifests, receipts, and
//! diagnostics and MUST never serialize as `Enforced`. A future protocol
//! version can require `Enforced` for selected effect classes without changing
//! the Read|Write seam (issue #63).

use crate::error::{FlowError, FlowResult};
use alloc::string::{String, ToString};
use alloc::vec;
use braid_ir::Value;

const MAX_REASON_BYTES: usize = 256;
const INV_JUSTIFICATION_GATE: &str = "INV-FLOW-019";

/// The v0 justification gate for a material effect.
///
/// `Enforced` means the invocation carries a proven justification for the
/// current snapshot/state. `Deferred` means the estate cannot prove
/// justification generally yet — the gate is present, explicit, and blocks a
/// future `Enforced` requirement, but does not claim proof today.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JustificationGate {
    /// Proven justification present — the effect is justified at this version.
    Enforced,
    /// Justification is a future gate — the reason and the version that will
    /// require enforcement are recorded, and no code may coerce this into
    /// `Enforced`.
    Deferred {
        reason: String,
        required_by_version: Option<u16>,
    },
}

impl JustificationGate {
    /// Validate the deferred reason shape.
    fn validate_reason(reason: &str) -> FlowResult<()> {
        if reason.is_empty() || reason.len() > MAX_REASON_BYTES {
            return Err(FlowError::Malformed {
                field: "justification_gate.reason",
                invariant: INV_JUSTIFICATION_GATE,
            });
        }
        if reason.chars().any(|c| c.is_control()) {
            return Err(FlowError::Malformed {
                field: "justification_gate.reason",
                invariant: INV_JUSTIFICATION_GATE,
            });
        }
        Ok(())
    }

    /// Canonical `Value` encoding. The two variants are disjoint closed shapes;
    /// `Deferred` never produces the `enforced` wire and vice versa.
    pub fn to_canon(&self) -> Value {
        match self {
            Self::Enforced => Value::map(vec![("enforced", Value::Bool(true))]),
            Self::Deferred {
                reason,
                required_by_version,
            } => {
                let mut inner = vec![("reason", Value::Text(reason.clone()))];
                if let Some(version) = required_by_version {
                    inner.push(("required_by_version", Value::Int(i64::from(*version))));
                }
                Value::map(vec![("deferred", Value::map(inner))])
            }
        }
    }

    /// Strict decode — unknown fields, wrong types, or a `Deferred` masquerading
    /// as `Enforced` fail closed. Mixing both top-level keys also fails.
    pub fn from_canon(value: &Value) -> FlowResult<Self> {
        let Value::Map(items) = value else {
            return Err(FlowError::Malformed {
                field: "justification_gate",
                invariant: INV_JUSTIFICATION_GATE,
            });
        };
        if items.len() != 1 {
            return Err(FlowError::Malformed {
                field: "justification_gate",
                invariant: INV_JUSTIFICATION_GATE,
            });
        }
        if let Some(marker) = items.get("enforced") {
            if *marker != Value::Bool(true) {
                return Err(FlowError::Malformed {
                    field: "justification_gate.enforced",
                    invariant: INV_JUSTIFICATION_GATE,
                });
            }
            return Ok(Self::Enforced);
        }
        if let Some(deferred) = items.get("deferred") {
            let Value::Map(fields) = deferred else {
                return Err(FlowError::Malformed {
                    field: "justification_gate.deferred",
                    invariant: INV_JUSTIFICATION_GATE,
                });
            };
            if fields.len() > 2 || fields.is_empty() {
                return Err(FlowError::Malformed {
                    field: "justification_gate.deferred",
                    invariant: INV_JUSTIFICATION_GATE,
                });
            }
            if fields
                .keys()
                .any(|k| k != "reason" && k != "required_by_version")
            {
                return Err(FlowError::Malformed {
                    field: "justification_gate.deferred",
                    invariant: INV_JUSTIFICATION_GATE,
                });
            }
            let Value::Text(reason) = fields.get("reason").ok_or(FlowError::Malformed {
                field: "justification_gate.deferred.reason",
                invariant: INV_JUSTIFICATION_GATE,
            })?
            else {
                return Err(FlowError::Malformed {
                    field: "justification_gate.deferred.reason",
                    invariant: INV_JUSTIFICATION_GATE,
                });
            };
            Self::validate_reason(reason)?;
            let required_by_version = match fields.get("required_by_version") {
                None => None,
                Some(Value::Int(number)) => {
                    let version = u16::try_from(*number).map_err(|_| FlowError::Malformed {
                        field: "justification_gate.deferred.required_by_version",
                        invariant: INV_JUSTIFICATION_GATE,
                    })?;
                    Some(version)
                }
                Some(_) => {
                    return Err(FlowError::Malformed {
                        field: "justification_gate.deferred.required_by_version",
                        invariant: INV_JUSTIFICATION_GATE,
                    });
                }
            };
            return Ok(Self::Deferred {
                reason: reason.clone(),
                required_by_version,
            });
        }
        Err(FlowError::Malformed {
            field: "justification_gate",
            invariant: INV_JUSTIFICATION_GATE,
        })
    }

    /// Human diagnostic — never used as wire identity.
    pub fn diagnostic(&self) -> String {
        match self {
            Self::Enforced => "justification_gate=enforced".into(),
            Self::Deferred {
                reason,
                required_by_version,
            } => {
                let version = required_by_version
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "(none)".into());
                alloc::format!(
                    "justification_gate=deferred reason=\"{}\" required_by_version={}",
                    reason,
                    version
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use braid_ir::{Value, decode_strict, encode};

    #[test]
    fn enforced_round_trip() {
        let gate = JustificationGate::Enforced;
        let canon = gate.to_canon();
        let decoded = JustificationGate::from_canon(&canon).expect("enforced round trip");
        assert_eq!(gate, decoded);
        assert_eq!(canon, Value::map(vec![("enforced", Value::Bool(true))]));
    }

    #[test]
    fn deferred_round_trip_without_version() {
        let gate = JustificationGate::Deferred {
            reason: "estate cannot prove generally".into(),
            required_by_version: None,
        };
        let canon = gate.to_canon();
        let decoded = JustificationGate::from_canon(&canon).expect("deferred round trip");
        assert_eq!(gate, decoded);
    }

    #[test]
    fn deferred_round_trip_with_version() {
        let gate = JustificationGate::Deferred {
            reason: "needs provenance v2".into(),
            required_by_version: Some(2),
        };
        let canon = gate.to_canon();
        let decoded = JustificationGate::from_canon(&canon).expect("deferred with version");
        assert_eq!(gate, decoded);
    }

    #[test]
    fn deferred_never_serializes_as_enforced() {
        let deferred = JustificationGate::Deferred {
            reason: "loud future gate".into(),
            required_by_version: Some(3),
        };
        let canon = deferred.to_canon();
        assert_ne!(canon, Value::map(vec![("enforced", Value::Bool(true))]));
        let Value::Map(items) = &canon else {
            panic!("expected map");
        };
        assert!(items.contains_key("deferred"));
        assert!(!items.contains_key("enforced"));
        let decoded = JustificationGate::from_canon(&canon).unwrap();
        assert_eq!(decoded, deferred);
        assert_ne!(decoded, JustificationGate::Enforced);
    }

    #[test]
    fn enforced_never_decodes_as_deferred() {
        let enforced = JustificationGate::Enforced;
        let canon = enforced.to_canon();
        let decoded = JustificationGate::from_canon(&canon).unwrap();
        assert_eq!(decoded, JustificationGate::Enforced);
        assert_ne!(
            decoded,
            JustificationGate::Deferred {
                reason: "x".into(),
                required_by_version: None
            }
        );
    }

    #[test]
    fn unknown_variant_rejects() {
        let bad = Value::map(vec![("unknown_variant", Value::Bool(true))]);
        assert!(JustificationGate::from_canon(&bad).is_err());
    }

    #[test]
    fn mixed_keys_reject() {
        let bad = Value::map(vec![
            ("enforced", Value::Bool(true)),
            (
                "deferred",
                Value::map(vec![("reason", Value::Text("x".into()))]),
            ),
        ]);
        assert!(JustificationGate::from_canon(&bad).is_err());
    }

    #[test]
    fn empty_reason_rejects() {
        let bad = JustificationGate::Deferred {
            reason: "".into(),
            required_by_version: None,
        }
        .to_canon();
        assert!(JustificationGate::from_canon(&bad).is_err());
    }

    #[test]
    fn canon_bytes_bijection_guarded() {
        let gate = JustificationGate::Deferred {
            reason: "bijection check".into(),
            required_by_version: Some(5),
        };
        let canon = gate.to_canon();
        let bytes = encode(&canon);
        let decoded_value = decode_strict(&bytes).expect("decode_strict");
        assert_eq!(canon, decoded_value);
        let re_decoded = JustificationGate::from_canon(&decoded_value).unwrap();
        assert_eq!(gate, re_decoded);
    }

    #[test]
    fn diagnostic_contains_reason() {
        let gate = JustificationGate::Deferred {
            reason: "needs provenance v2".into(),
            required_by_version: Some(2),
        };
        let diag = gate.diagnostic();
        assert!(diag.contains("deferred"));
        assert!(diag.contains("needs provenance v2"));
        assert!(diag.contains("2"));
        let enforced_diag = JustificationGate::Enforced.diagnostic();
        assert!(enforced_diag.contains("enforced"));
        assert!(!enforced_diag.contains("deferred"));
    }
}
