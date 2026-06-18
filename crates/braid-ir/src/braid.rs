//! The braid: a DAG of strands (PRD §4.2).
//!
//! //why inputs reference strictly SMALLER strand indices: acyclicity is then
//! structural — a cycle is unrepresentable, not detected-after. The index
//! order is also the (one) topological order every fold uses, so taint and
//! type checks are single forward passes.

use crate::term::RegistryError;
use crate::value::Value;

/// One term application. `inputs[i]` is the index of the strand whose output
/// feeds this strand's i-th parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Strand {
    pub term: String,
    pub inputs: Vec<u32>,
}

/// A DAG of strands plus its designated outputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Braid {
    pub strands: Vec<Strand>,
    pub outputs: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BraidError {
    /// An input/output references a strand index ≥ its own position / len.
    ForwardReference {
        strand: usize,
        index: u32,
    },
    OutputOutOfRange(u32),
    Empty,
    NoOutputs,
}

impl Braid {
    /// Structural validation (verifier Structure stage re-runs this on its
    /// own decode — shared types, independent bytes).
    pub fn validate(&self) -> Result<(), BraidError> {
        if self.strands.is_empty() {
            return Err(BraidError::Empty);
        }
        for (i, s) in self.strands.iter().enumerate() {
            for &inp in &s.inputs {
                if inp as usize >= i {
                    return Err(BraidError::ForwardReference {
                        strand: i,
                        index: inp,
                    });
                }
            }
        }
        if self.outputs.is_empty() {
            return Err(BraidError::NoOutputs);
        }
        for &o in &self.outputs {
            if o as usize >= self.strands.len() {
                return Err(BraidError::OutputOutOfRange(o));
            }
        }
        Ok(())
    }

    pub fn to_canon(&self) -> Value {
        let strands: Vec<Value> = self
            .strands
            .iter()
            .map(|s| {
                Value::map(vec![
                    ("term", Value::Text(s.term.clone())),
                    (
                        "inputs",
                        Value::List(s.inputs.iter().map(|&i| Value::Int(i as i64)).collect()),
                    ),
                ])
            })
            .collect();
        Value::map(vec![
            (
                "outputs",
                Value::List(self.outputs.iter().map(|&o| Value::Int(o as i64)).collect()),
            ),
            ("strands", Value::List(strands)),
        ])
    }

    pub fn from_canon(v: &Value) -> Result<Self, RegistryError> {
        if !v.require_only_keys(&["outputs", "strands"]) {
            return Err(RegistryError::Malformed("braid: unknown field"));
        }
        let strands = match v.get("strands") {
            Some(Value::List(items)) => items
                .iter()
                .map(|s| {
                    if !s.require_only_keys(&["term", "inputs"]) {
                        return Err(RegistryError::Malformed("strand: unknown field"));
                    }
                    let term = match s.get("term") {
                        Some(Value::Text(t)) => t.clone(),
                        _ => return Err(RegistryError::Malformed("strand term")),
                    };
                    let inputs = match s.get("inputs") {
                        Some(Value::List(idx)) => idx
                            .iter()
                            .map(|i| match i {
                                Value::Int(n) if *n >= 0 && *n <= u32::MAX as i64 => Ok(*n as u32),
                                _ => Err(RegistryError::Malformed("strand input index")),
                            })
                            .collect::<Result<Vec<_>, _>>()?,
                        _ => return Err(RegistryError::Malformed("strand inputs")),
                    };
                    Ok(Strand { term, inputs })
                })
                .collect::<Result<Vec<_>, _>>()?,
            _ => return Err(RegistryError::Malformed("strands")),
        };
        let outputs = match v.get("outputs") {
            Some(Value::List(items)) => items
                .iter()
                .map(|i| match i {
                    Value::Int(n) if *n >= 0 && *n <= u32::MAX as i64 => Ok(*n as u32),
                    _ => Err(RegistryError::Malformed("output index")),
                })
                .collect::<Result<Vec<_>, _>>()?,
            _ => return Err(RegistryError::Malformed("outputs")),
        };
        Ok(Braid { strands, outputs })
    }
}
