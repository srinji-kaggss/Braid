//! # braid-run — deterministic DAG interpreter and execution engine (WS-1, PB-02)
//!
//! Executes admitted Braid capsules in topological order.
//!
//! Invariants enforced:
//! - **INV-RUN-TOPO:** Strands are executed sequentially in DAG order; forward references are rejected.
//! - **INV-RUN-CAP-GATED:** Any effectful term requiring a capability is refused unless granted in `capsule.grants`.
//! - **INV-RUN-BUDGET:** Total unit cost cannot exceed `capsule.budget`.
//! - **INV-RUN-CONFIRM:** Irreversible/Egress effects require explicit `ConfirmPolicy` authorization.
//! - **INV-RUN-JOURNAL:** All executions emit an append-only, content-addressed `Journal` witness.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

extern crate alloc;

use braid_capability::Capability;
use braid_ir::braid::Strand;
use braid_ir::term::EffectClass;
use braid_ir::{Capsule, Cid, ConfirmPolicy, IR_VERSION, TermRegistry, TermSpec, TypeTag, Value};

/// Why a capsule execution failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    /// The capsule exceeded its declared execution budget.
    BudgetExhausted {
        /// Declared maximum budget.
        budget: u64,
        /// Attempted consumption.
        consumed: u64,
        /// Source location of the error.
        at: &'static str,
    },
    /// An input reference pointed to an un-evaluated strand or invalid index.
    InvalidInputReference {
        /// The strand being evaluated.
        strand: usize,
        /// The invalid input index requested.
        input_index: u32,
        /// Source location of the error.
        at: &'static str,
    },
    /// The host dispatcher failed to evaluate the term.
    HostError {
        /// Error message from the host.
        message: String,
        /// Source location of the error.
        at: &'static str,
    },
    /// A required capability was not granted to the capsule.
    MissingCapability {
        /// Missing capability.
        capability: Capability,
        /// Source location of the error.
        at: &'static str,
    },
    /// The term produced a value that does not conform to its declared TypeTag.
    TypeMismatch {
        /// Expected type tag.
        expected: TypeTag,
        /// Actual runtime value description.
        actual: String,
        /// Source location of the error.
        at: &'static str,
    },
    /// An effect was invoked without the required confirmation policy.
    UnconfirmedEffect {
        /// The offending term ID.
        term: String,
        /// The unconfirmed effect class.
        effect: EffectClass,
        /// Source location of the error.
        at: &'static str,
    },
    /// The term was not found in the term registry.
    UnknownTerm {
        /// Unknown term identifier.
        term: String,
        /// Source location of the error.
        at: &'static str,
    },
    /// The number of provided inputs does not match the term specification.
    ArityMismatch {
        /// The term ID.
        term: String,
        /// Expected arity.
        expected: usize,
        /// Actual arity.
        actual: usize,
        /// Source location of the error.
        at: &'static str,
    },
    /// Output index out of bounds.
    InvalidOutputReference {
        /// Out-of-bounds output index.
        index: u32,
        /// Source location of the error.
        at: &'static str,
    },
    /// Capsule failed defense-in-depth header verification.
    InvalidCapsuleHeader {
        /// Reason for header invalidity.
        reason: String,
        /// Source location of the error.
        at: &'static str,
    },
}

fn fmt_budget_err(f: &mut fmt::Formatter<'_>, at: &str, consumed: u64, budget: u64) -> fmt::Result {
    write!(
        f,
        "execution budget exhausted at {at}: {consumed} > {budget}"
    )
}

fn fmt_input_ref_err(
    f: &mut fmt::Formatter<'_>,
    at: &str,
    strand: usize,
    input_index: u32,
) -> fmt::Result {
    write!(
        f,
        "strand {strand} references non-preceding input {input_index} at {at}"
    )
}

fn fmt_type_mismatch_err(
    f: &mut fmt::Formatter<'_>,
    at: &str,
    expected: &TypeTag,
    actual: &str,
) -> fmt::Result {
    write!(
        f,
        "type mismatch at {at}: expected {expected:?}, got {actual}"
    )
}

fn fmt_effect_err(
    f: &mut fmt::Formatter<'_>,
    at: &str,
    term: &str,
    effect: EffectClass,
) -> fmt::Result {
    write!(
        f,
        "term '{term}' with effect {effect:?} requires confirmation at {at}"
    )
}

fn fmt_arity_err(
    f: &mut fmt::Formatter<'_>,
    at: &str,
    term: &str,
    expected: usize,
    actual: usize,
) -> fmt::Result {
    write!(
        f,
        "arity mismatch for '{term}' at {at}: expected {expected}, got {actual}"
    )
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BudgetExhausted {
                budget,
                consumed,
                at,
            } => fmt_budget_err(f, at, *consumed, *budget),
            Self::InvalidInputReference {
                strand,
                input_index,
                at,
            } => fmt_input_ref_err(f, at, *strand, *input_index),
            Self::HostError { message, at } => write!(f, "host error at {at}: {message}"),
            Self::MissingCapability { capability, at } => {
                write!(f, "missing capability {capability:?} at {at}")
            }
            Self::TypeMismatch {
                expected,
                actual,
                at,
            } => fmt_type_mismatch_err(f, at, expected, actual),
            Self::UnconfirmedEffect { term, effect, at } => fmt_effect_err(f, at, term, *effect),
            Self::UnknownTerm { term, at } => write!(f, "unknown term '{term}' at {at}"),
            Self::ArityMismatch {
                term,
                expected,
                actual,
                at,
            } => fmt_arity_err(f, at, term, *expected, *actual),
            Self::InvalidOutputReference { index, at } => {
                write!(f, "invalid output strand {index} at {at}")
            }
            Self::InvalidCapsuleHeader { reason, at } => {
                write!(f, "invalid capsule header at {at}: {reason}")
            }
        }
    }
}

impl core::error::Error for ExecutionError {}

/// An individual strand execution receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalEntry {
    /// The 0-based topological index of the strand.
    pub strand_index: usize,
    /// The term identifier that was executed.
    pub term: String,
    /// The effect class of the term.
    pub effect: EffectClass,
    /// The cost charged for this strand.
    pub cost: u64,
    /// The resulting value produced by this strand.
    pub output: Value,
}

/// The complete execution journal witness of a capsule run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Journal {
    /// The content-addressed CID of the executed capsule.
    pub capsule_cid: Cid,
    /// Chronological list of strand execution entries.
    pub entries: Vec<JournalEntry>,
    /// The final evaluated output values of the capsule.
    pub outputs: Vec<Value>,
    /// Total cost units consumed.
    pub total_cost: u64,
}

/// The Host environment contract for executing registered Braid terms.
pub trait Host {
    /// Executes a registered term on the host.
    ///
    /// - `term_id`: The identifier of the term being invoked.
    /// - `inputs`: Evaluated argument values from upstream strands.
    /// - `spec`: The term's registered specification from the TermRegistry.
    fn call(
        &mut self,
        term_id: &str,
        inputs: &[Value],
        spec: &TermSpec,
    ) -> Result<Value, ExecutionError>;
}

fn verify_capsule_headers(
    capsule: &Capsule,
    registry: &TermRegistry,
) -> Result<(), ExecutionError> {
    if capsule.ir_version != IR_VERSION {
        Err(ExecutionError::InvalidCapsuleHeader {
            reason: "ir_version mismatch".into(),
            at: "execute_capsule",
        })
    } else if capsule.registry_cid != registry.cid() {
        Err(ExecutionError::InvalidCapsuleHeader {
            reason: "registry_cid mismatch".into(),
            at: "execute_capsule",
        })
    } else {
        Ok(())
    }
}

fn charge_strand_budget(consumed: &mut u64, cost: u64, budget: u64) -> Result<(), ExecutionError> {
    let next_consumed = consumed
        .checked_add(cost)
        .ok_or(ExecutionError::BudgetExhausted {
            budget,
            consumed: u64::MAX,
            at: "execute_capsule::budget",
        })?;
    if next_consumed > budget {
        Err(ExecutionError::BudgetExhausted {
            budget,
            consumed: next_consumed,
            at: "execute_capsule::budget",
        })
    } else {
        *consumed = next_consumed;
        Ok(())
    }
}

fn ensure_capability_available(
    required_cap: &Capability,
    grants: &[Capability],
) -> Result<(), ExecutionError> {
    if !grants.contains(required_cap) {
        Err(ExecutionError::MissingCapability {
            capability: required_cap.clone(),
            at: "execute_capsule::capability",
        })
    } else {
        Ok(())
    }
}

fn verify_strand_capability(spec: &TermSpec, grants: &[Capability]) -> Result<(), ExecutionError> {
    if let Some(ref required_cap) = spec.capability {
        ensure_capability_available(required_cap, grants)?;
    }
    Ok(())
}

fn verify_strand_confirmation(
    spec: &TermSpec,
    confirm: ConfirmPolicy,
    term: &str,
) -> Result<(), ExecutionError> {
    if matches!(spec.effect, EffectClass::Irreversible | EffectClass::Egress)
        && matches!(confirm, ConfirmPolicy::None)
    {
        Err(ExecutionError::UnconfirmedEffect {
            term: term.to_string(),
            effect: spec.effect,
            at: "execute_capsule::confirmation",
        })
    } else {
        Ok(())
    }
}

fn check_strand_arity(term: &str, expected: usize, actual: usize) -> Result<(), ExecutionError> {
    if expected != actual {
        Err(ExecutionError::ArityMismatch {
            term: term.to_string(),
            expected,
            actual,
            at: "execute_capsule::inputs",
        })
    } else {
        Ok(())
    }
}

fn check_input_index(
    strand_idx: usize,
    in_pos: usize,
    evaluated_len: usize,
    in_idx: u32,
) -> Result<(), ExecutionError> {
    if in_pos >= strand_idx || in_pos >= evaluated_len {
        Err(ExecutionError::InvalidInputReference {
            strand: strand_idx,
            input_index: in_idx,
            at: "execute_capsule::inputs",
        })
    } else {
        Ok(())
    }
}

fn resolve_strand_inputs(
    strand: &Strand,
    strand_idx: usize,
    spec: &TermSpec,
    evaluated: &[Value],
) -> Result<Vec<Value>, ExecutionError> {
    check_strand_arity(&strand.term, spec.inputs.len(), strand.inputs.len())?;
    let mut input_vals = Vec::with_capacity(strand.inputs.len());
    for &in_idx in &strand.inputs {
        let in_pos = in_idx as usize;
        check_input_index(strand_idx, in_pos, evaluated.len(), in_idx)?;
        input_vals.push(evaluated[in_pos].clone());
    }
    Ok(input_vals)
}

fn pre_check_strand<'a>(
    strand: &'a Strand,
    registry: &'a TermRegistry,
    capsule: &Capsule,
    consumed_budget: &mut u64,
) -> Result<&'a TermSpec, ExecutionError> {
    let spec = registry
        .get(&strand.term)
        .ok_or_else(|| ExecutionError::UnknownTerm {
            term: strand.term.clone(),
            at: "execute_capsule::spec",
        })?;

    charge_strand_budget(consumed_budget, spec.cost, capsule.budget)?;
    verify_strand_capability(spec, &capsule.grants)?;
    verify_strand_confirmation(spec, capsule.confirm, &strand.term)?;
    Ok(spec)
}

fn evaluate_strand(
    strand: &Strand,
    strand_idx: usize,
    registry: &TermRegistry,
    capsule: &Capsule,
    evaluated: &[Value],
    consumed_budget: &mut u64,
    host: &mut impl Host,
) -> Result<(JournalEntry, Value), ExecutionError> {
    let spec = pre_check_strand(strand, registry, capsule, consumed_budget)?;
    let input_vals = resolve_strand_inputs(strand, strand_idx, spec, evaluated)?;
    let out_val = host.call(&strand.term, &input_vals, spec)?;
    validate_type_tag(&out_val, &spec.output)?;

    let entry = JournalEntry {
        strand_index: strand_idx,
        term: strand.term.clone(),
        effect: spec.effect,
        cost: spec.cost,
        output: out_val.clone(),
    };
    Ok((entry, out_val))
}

fn check_output_index(
    out_pos: usize,
    evaluated_len: usize,
    out_idx: u32,
) -> Result<(), ExecutionError> {
    if out_pos >= evaluated_len {
        Err(ExecutionError::InvalidOutputReference {
            index: out_idx,
            at: "execute_capsule::outputs",
        })
    } else {
        Ok(())
    }
}

fn gather_capsule_outputs(
    output_indices: &[u32],
    evaluated: &[Value],
) -> Result<Vec<Value>, ExecutionError> {
    let mut outputs = Vec::with_capacity(output_indices.len());
    for &out_idx in output_indices {
        let out_pos = out_idx as usize;
        check_output_index(out_pos, evaluated.len(), out_idx)?;
        outputs.push(evaluated[out_pos].clone());
    }
    Ok(outputs)
}

/// Executes an admitted Braid capsule against a registry and host dispatcher.
pub fn execute_capsule(
    capsule: &Capsule,
    registry: &TermRegistry,
    host: &mut impl Host,
) -> Result<Journal, ExecutionError> {
    verify_capsule_headers(capsule, registry)?;

    let mut evaluated: Vec<Value> = Vec::with_capacity(capsule.braid.strands.len());
    let mut entries: Vec<JournalEntry> = Vec::with_capacity(capsule.braid.strands.len());
    let mut consumed_budget: u64 = 0;

    for (strand_idx, strand) in capsule.braid.strands.iter().enumerate() {
        let (entry, out_val) = evaluate_strand(
            strand,
            strand_idx,
            registry,
            capsule,
            &evaluated,
            &mut consumed_budget,
            host,
        )?;
        entries.push(entry);
        evaluated.push(out_val);
    }

    let outputs = gather_capsule_outputs(&capsule.braid.outputs, &evaluated)?;
    Ok(Journal {
        capsule_cid: capsule.cid(),
        entries,
        outputs,
        total_cost: consumed_budget,
    })
}

fn validate_cid_bytes(b: &[u8]) -> Result<(), ExecutionError> {
    if b.len() == 32 {
        Ok(())
    } else {
        Err(ExecutionError::TypeMismatch {
            expected: TypeTag::Cid,
            actual: format!("Bytes(len={})", b.len()),
            at: "validate_type_tag::cid",
        })
    }
}

fn check_opaque_tag(
    tag: &str,
    expected_label: &str,
    expected: &TypeTag,
) -> Result<(), ExecutionError> {
    if tag != expected_label {
        Err(ExecutionError::TypeMismatch {
            expected: expected.clone(),
            actual: format!("Opaque({tag})"),
            at: "validate_type_tag::opaque",
        })
    } else {
        Ok(())
    }
}

fn validate_opaque_map(
    map: &alloc::collections::BTreeMap<String, Value>,
    expected: &TypeTag,
    expected_label: &str,
) -> Result<(), ExecutionError> {
    if let Some(Value::Text(tag)) = map.get("__type") {
        check_opaque_tag(tag, expected_label, expected)?;
    }
    Ok(())
}

fn validate_opaque_value(
    value: &Value,
    expected: &TypeTag,
    expected_label: &str,
) -> Result<(), ExecutionError> {
    match value {
        Value::Map(m) => validate_opaque_map(m, expected, expected_label),
        Value::Bytes(_) => Ok(()),
        actual_val => Err(ExecutionError::TypeMismatch {
            expected: expected.clone(),
            actual: format!("{actual_val:?}"),
            at: "validate_type_tag::opaque",
        }),
    }
}

fn validate_scalar_type(value: &Value, expected: &TypeTag) -> Option<Result<(), ExecutionError>> {
    match (value, expected) {
        (Value::Bool(_), TypeTag::Bool) => Some(Ok(())),
        (Value::Int(_), TypeTag::Int) => Some(Ok(())),
        (Value::Bytes(_), TypeTag::Bytes) => Some(Ok(())),
        (Value::Text(_), TypeTag::Text) => Some(Ok(())),
        (Value::Bytes(b), TypeTag::Cid) => Some(validate_cid_bytes(b)),
        _ => None,
    }
}

/// Recursively validates that a runtime `Value` conforms strictly to its `TypeTag`.
pub fn validate_type_tag(value: &Value, expected: &TypeTag) -> Result<(), ExecutionError> {
    if let Some(res) = validate_scalar_type(value, expected) {
        return res;
    }
    match (value, expected) {
        (Value::List(items), TypeTag::List(inner)) => {
            for item in items {
                validate_type_tag(item, inner)?;
            }
            Ok(())
        }
        (val, TypeTag::Opaque(expected_label, _args)) => {
            validate_opaque_value(val, expected, expected_label)
        }
        (actual_val, expected_tag) => Err(ExecutionError::TypeMismatch {
            expected: expected_tag.clone(),
            actual: format!("{actual_val:?}"),
            at: "validate_type_tag",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_error_display() {
        let err = ExecutionError::HostError {
            message: "simulated host failure".into(),
            at: "test",
        };
        assert!(err.to_string().contains("simulated host failure"));
    }
}
