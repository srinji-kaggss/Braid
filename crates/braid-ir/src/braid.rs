//! The braid: an acyclic graph of strand invocations (PRD §4.2).
//!
//! Strands are strictly 0-indexed in topological order; strand
//! `i` may only take inputs from `[0..i)`. Forward references and cycles are
//! unrepresentable by construction.

use crate::term::RegistryError;
use crate::value::Value;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

/// One node in the braid graph: an invocation of a registered term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Strand {
    /// Identifier of the term in the capsule's pinned registry.
    pub term: String,
    /// Indices of the strands providing this term's inputs, in signature order.
    pub inputs: Vec<u32>,
}

/// The computation graph of a capsule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Braid {
    /// Strands in topological evaluation order.
    pub strands: Vec<Strand>,
    /// Indices of strands whose outputs constitute the braid's results.
    pub outputs: Vec<u32>,
}

fn decode_strand_input(value: &Value) -> Result<u32, RegistryError> {
    match value {
        Value::Int(n) if *n >= 0 && *n <= u32::MAX as i64 => Ok(*n as u32),
        _ => Err(RegistryError::Malformed {
            field: "strand input index",
            at: "Braid::decode_strand_input",
        }),
    }
}

fn check_strand_key_universe(strand_val: &Value) -> Result<(), RegistryError> {
    if !strand_val.require_only_keys(&["term", "inputs"]) {
        Err(RegistryError::Malformed {
            field: "strand: unknown field",
            at: "Braid::decode_single_strand",
        })
    } else {
        Ok(())
    }
}

fn extract_strand_term(strand_val: &Value) -> Result<String, RegistryError> {
    match strand_val.get_field("term") {
        Some(Value::Text(t)) => Ok(t.clone()),
        _ => Err(RegistryError::Malformed {
            field: "strand term",
            at: "Braid::decode_single_strand",
        }),
    }
}

fn extract_strand_inputs(strand_val: &Value) -> Result<Vec<u32>, RegistryError> {
    match strand_val.get_field("inputs") {
        Some(Value::List(idx)) => idx
            .iter()
            .map(decode_strand_input)
            .collect::<Result<Vec<_>, _>>(),
        _ => Err(RegistryError::Malformed {
            field: "strand inputs",
            at: "Braid::decode_single_strand",
        }),
    }
}

fn decode_single_strand(strand_val: &Value) -> Result<Strand, RegistryError> {
    check_strand_key_universe(strand_val)?;
    let term = extract_strand_term(strand_val)?;
    let inputs = extract_strand_inputs(strand_val)?;
    Ok(Strand { term, inputs })
}

fn decode_output_index(value: &Value) -> Result<u32, RegistryError> {
    match value {
        Value::Int(n) if *n >= 0 && *n <= u32::MAX as i64 => Ok(*n as u32),
        _ => Err(RegistryError::Malformed {
            field: "output index",
            at: "Braid::decode_output_index",
        }),
    }
}

fn decode_strands_list(v: &Value) -> Result<Vec<Strand>, RegistryError> {
    match v.get_field("strands") {
        Some(Value::List(items)) => items
            .iter()
            .map(decode_single_strand)
            .collect::<Result<Vec<_>, _>>(),
        _ => Err(RegistryError::Malformed {
            field: "strands",
            at: "Braid::decode_strands_list",
        }),
    }
}

fn decode_outputs_list(v: &Value) -> Result<Vec<u32>, RegistryError> {
    match v.get_field("outputs") {
        Some(Value::List(items)) => items
            .iter()
            .map(decode_output_index)
            .collect::<Result<Vec<_>, _>>(),
        _ => Err(RegistryError::Malformed {
            field: "outputs",
            at: "Braid::decode_outputs_list",
        }),
    }
}

fn check_braid_key_universe(v: &Value) -> Result<(), RegistryError> {
    if !v.require_only_keys(&["outputs", "strands"]) {
        Err(RegistryError::Malformed {
            field: "braid: unknown field",
            at: "Braid::from_canon",
        })
    } else {
        Ok(())
    }
}

fn check_input_index_bound(input_idx: u32, strand_idx: usize) -> Result<(), &'static str> {
    if input_idx as usize >= strand_idx {
        Err("strand input forward-reference or self-reference")
    } else {
        Ok(())
    }
}

fn validate_strand_inputs(strand_idx: usize, strand: &Strand) -> Result<(), &'static str> {
    for &input_idx in &strand.inputs {
        check_input_index_bound(input_idx, strand_idx)?;
    }
    Ok(())
}

fn check_output_index_bound(output_idx: u32, strand_count: usize) -> Result<(), &'static str> {
    if output_idx as usize >= strand_count {
        Err("braid output index out of range")
    } else {
        Ok(())
    }
}

fn validate_braid_outputs(strand_count: usize, outputs: &[u32]) -> Result<(), &'static str> {
    for &output_idx in outputs {
        check_output_index_bound(output_idx, strand_count)?;
    }
    Ok(())
}

impl Braid {
    /// Validates DAG topological ordering: strand `i` may only take inputs from `0..i`,
    /// and output indices must be within the strand count bounds.
    pub fn validate(&self) -> Result<(), &'static str> {
        for (strand_idx, strand) in self.strands.iter().enumerate() {
            validate_strand_inputs(strand_idx, strand)?;
        }
        validate_braid_outputs(self.strands.len(), &self.outputs)?;
        Ok(())
    }

    pub fn to_canon(&self) -> Value {
        let strands: Vec<Value> = self
            .strands
            .iter()
            .map(|s| {
                let inputs: Vec<Value> = s.inputs.iter().map(|&i| Value::Int(i as i64)).collect();
                Value::map(vec![
                    ("inputs", Value::List(inputs)),
                    ("term", Value::Text(s.term.clone())),
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
        check_braid_key_universe(v)?;
        let strands = decode_strands_list(v)?;
        let outputs = decode_outputs_list(v)?;
        Ok(Braid { strands, outputs })
    }
}
