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
use braid_ir::term::EffectClass;
use braid_ir::{Capsule, Cid, ConfirmPolicy, TermRegistry, TermSpec, TypeTag, Value, IR_VERSION};

/// Why a capsule execution failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    /// The capsule exceeded its declared execution budget.
    BudgetExhausted {
        /// Declared maximum budget.
        budget: u64,
        /// Attempted consumption.
        consumed: u64,
    },
    /// An input reference pointed to an un-evaluated strand or invalid index.
    InvalidInputReference {
        /// The strand being evaluated.
        strand: usize,
        /// The invalid input index requested.
        input_index: u32,
    },
    /// The host dispatcher failed to evaluate the term.
    HostError(String),
    /// A required capability was not granted to the capsule.
    MissingCapability(Capability),
    /// The term produced a value that does not conform to its declared TypeTag.
    TypeMismatch {
        /// Expected type tag.
        expected: TypeTag,
        /// Actual runtime value description.
        actual: String,
    },
    /// An effect was invoked without the required confirmation policy.
    UnconfirmedEffect {
        /// The offending term ID.
        term: String,
        /// The unconfirmed effect class.
        effect: EffectClass,
    },
    /// The term was not found in the term registry.
    UnknownTerm(String),
    /// The number of provided inputs does not match the term specification.
    ArityMismatch {
        /// The term ID.
        term: String,
        /// Expected arity.
        expected: usize,
        /// Actual arity.
        actual: usize,
    },
    /// Output index out of bounds.
    InvalidOutputReference(u32),
    /// Capsule failed defense-in-depth header verification.
    InvalidCapsuleHeader(String),
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BudgetExhausted { budget, consumed } => {
                write!(f, "execution budget exhausted: {consumed} > {budget}")
            }
            Self::InvalidInputReference {
                strand,
                input_index,
            } => write!(
                f,
                "strand {strand} references non-preceding or out-of-bounds input {input_index}"
            ),
            Self::HostError(msg) => write!(f, "host error: {msg}"),
            Self::MissingCapability(cap) => write!(f, "missing granted capability: {cap:?}"),
            Self::TypeMismatch { expected, actual } => {
                write!(f, "type mismatch: expected {expected:?}, got {actual}")
            }
            Self::UnconfirmedEffect { term, effect } => write!(
                f,
                "term '{term}' with effect {effect:?} requires confirmation policy"
            ),
            Self::UnknownTerm(term) => write!(f, "unknown term: {term}"),
            Self::ArityMismatch {
                term,
                expected,
                actual,
            } => write!(
                f,
                "arity mismatch for '{term}': expected {expected} inputs, got {actual}"
            ),
            Self::InvalidOutputReference(idx) => {
                write!(f, "capsule output references out-of-bounds strand {idx}")
            }
            Self::InvalidCapsuleHeader(reason) => {
                write!(f, "invalid capsule header: {reason}")
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

/// Executes an admitted Braid capsule against a registry and host dispatcher.
pub fn execute(
    capsule: &Capsule,
    registry: &TermRegistry,
    host: &mut impl Host,
) -> Result<Journal, ExecutionError> {
    // Defense-in-depth header verification
    if capsule.ir_version != IR_VERSION {
        return Err(ExecutionError::InvalidCapsuleHeader("ir_version mismatch".into()));
    }
    if capsule.registry_cid != registry.cid() {
        return Err(ExecutionError::InvalidCapsuleHeader("registry_cid mismatch".into()));
    }

    let mut evaluated: Vec<Value> = Vec::with_capacity(capsule.braid.strands.len());
    let mut entries: Vec<JournalEntry> = Vec::with_capacity(capsule.braid.strands.len());
    let mut consumed_budget: u64 = 0;

    for (strand_idx, strand) in capsule.braid.strands.iter().enumerate() {
        let spec = registry
            .get(&strand.term)
            .ok_or_else(|| ExecutionError::UnknownTerm(strand.term.clone()))?;

        // 1. Check budget.
        consumed_budget = consumed_budget
            .checked_add(spec.cost)
            .ok_or(ExecutionError::BudgetExhausted {
                budget: capsule.budget,
                consumed: u64::MAX,
            })?;
        if consumed_budget > capsule.budget {
            return Err(ExecutionError::BudgetExhausted {
                budget: capsule.budget,
                consumed: consumed_budget,
            });
        }

        // 2. Gate capabilities.
        if let Some(ref required_cap) = spec.capability {
            if !capsule.grants.contains(required_cap) {
                return Err(ExecutionError::MissingCapability(required_cap.clone()));
            }
        }

        // 3. Check confirmation policy for irreversible or egress effects.
        if matches!(spec.effect, EffectClass::Irreversible | EffectClass::Egress)
            && matches!(capsule.confirm, ConfirmPolicy::None)
        {
            return Err(ExecutionError::UnconfirmedEffect {
                term: strand.term.clone(),
                effect: spec.effect,
            });
        }

        // 4. Resolve inputs in DAG order (strictly preceding strands).
        if strand.inputs.len() != spec.inputs.len() {
            return Err(ExecutionError::ArityMismatch {
                term: strand.term.clone(),
                expected: spec.inputs.len(),
                actual: strand.inputs.len(),
            });
        }

        let mut input_vals = Vec::with_capacity(strand.inputs.len());
        for &in_idx in &strand.inputs {
            let in_pos = in_idx as usize;
            if in_pos >= strand_idx || in_pos >= evaluated.len() {
                return Err(ExecutionError::InvalidInputReference {
                    strand: strand_idx,
                    input_index: in_idx,
                });
            }
            input_vals.push(evaluated[in_pos].clone());
        }

        // 5. Dispatch to host.
        let out_val = host.call(&strand.term, &input_vals, spec)?;

        // 6. Validate output against expected type tag (deep recursive check).
        validate_type_tag(&out_val, &spec.output)?;

        // 7. Record to evaluated cache and journal.
        entries.push(JournalEntry {
            strand_index: strand_idx,
            term: strand.term.clone(),
            effect: spec.effect,
            cost: spec.cost,
            output: out_val.clone(),
        });
        evaluated.push(out_val);
    }

    // Gather final outputs.
    let mut outputs = Vec::with_capacity(capsule.braid.outputs.len());
    for &out_idx in &capsule.braid.outputs {
        let out_pos = out_idx as usize;
        if out_pos >= evaluated.len() {
            return Err(ExecutionError::InvalidOutputReference(out_idx));
        }
        outputs.push(evaluated[out_pos].clone());
    }

    Ok(Journal {
        capsule_cid: capsule.cid(),
        entries,
        outputs,
        total_cost: consumed_budget,
    })
}

/// Recursively validates that a runtime `Value` conforms strictly to its `TypeTag`.
pub fn validate_type_tag(value: &Value, expected: &TypeTag) -> Result<(), ExecutionError> {
    match (value, expected) {
        (Value::Bool(_), TypeTag::Bool) => Ok(()),
        (Value::Int(_), TypeTag::Int) => Ok(()),
        (Value::Bytes(_), TypeTag::Bytes) => Ok(()),
        (Value::Text(_), TypeTag::Text) => Ok(()),
        (Value::Bytes(b), TypeTag::Cid) => {
            if b.len() == 32 {
                Ok(())
            } else {
                Err(ExecutionError::TypeMismatch {
                    expected: TypeTag::Cid,
                    actual: format!("Bytes(len={})", b.len()),
                })
            }
        }
        (Value::List(items), TypeTag::List(inner)) => {
            for item in items {
                validate_type_tag(item, inner)?;
            }
            Ok(())
        }
        (val, TypeTag::Opaque(expected_label, _args)) => {
            match val {
                Value::Map(m) => {
                    if let Some(Value::Text(tag)) = m.get("__type") {
                        if tag != expected_label {
                            return Err(ExecutionError::TypeMismatch {
                                expected: expected.clone(),
                                actual: format!("Opaque({tag})"),
                            });
                        }
                    }
                    Ok(())
                }
                Value::Bytes(_) => Ok(()),
                actual_val => Err(ExecutionError::TypeMismatch {
                    expected: expected.clone(),
                    actual: format!("{actual_val:?}"),
                }),
            }
        }
        (actual_val, expected_tag) => Err(ExecutionError::TypeMismatch {
            expected: expected_tag.clone(),
            actual: format!("{actual_val:?}"),
        }),
    }
}

/// A mock host for testing and pure term dispatch.
pub struct MockHost {
    /// Invocations logged during execution.
    pub calls: Vec<(String, Vec<Value>)>,
}

impl Default for MockHost {
    fn default() -> Self {
        Self::new()
    }
}

impl MockHost {
    /// Creates an empty MockHost.
    pub fn new() -> Self {
        Self { calls: Vec::new() }
    }
}

impl Host for MockHost {
    fn call(
        &mut self,
        term_id: &str,
        inputs: &[Value],
        _spec: &TermSpec,
    ) -> Result<Value, ExecutionError> {
        self.calls.push((term_id.to_string(), inputs.to_vec()));

        // Built-in checked arithmetic behaviors for mock math terms:
        match term_id {
            "math.add" => {
                if let (Some(Value::Int(a)), Some(Value::Int(b))) = (inputs.first(), inputs.get(1)) {
                    a.checked_add(*b)
                        .map(Value::Int)
                        .ok_or_else(|| ExecutionError::HostError("math.add overflow".to_string()))
                } else {
                    Err(ExecutionError::HostError("math.add expects two Ints".to_string()))
                }
            }
            "math.mul" => {
                if let (Some(Value::Int(a)), Some(Value::Int(b))) = (inputs.first(), inputs.get(1)) {
                    a.checked_mul(*b)
                        .map(Value::Int)
                        .ok_or_else(|| ExecutionError::HostError("math.mul overflow".to_string()))
                } else {
                    Err(ExecutionError::HostError("math.mul expects two Ints".to_string()))
                }
            }
            "text.concat" => {
                if let (Some(Value::Text(a)), Some(Value::Text(b))) = (inputs.first(), inputs.get(1)) {
                    Ok(Value::Text(format!("{a}{b}")))
                } else {
                    Err(ExecutionError::HostError("text.concat expects two Texts".to_string()))
                }
            }
            "bool.not" => {
                if let Some(Value::Bool(b)) = inputs.first() {
                    Ok(Value::Bool(!b))
                } else {
                    Err(ExecutionError::HostError("bool.not expects a Bool".to_string()))
                }
            }
            _ => {
                // If it's a literal or opaque mock term, return default matching or unit
                Ok(Value::Bool(true))
            }
        }
    }
}
