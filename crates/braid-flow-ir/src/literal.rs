//! Allocation-free validation for canonical literal values carried by a Flow.

use crate::error::{FlowError, FlowResult, LimitKind};
use braid_ir::Value;

// Conservative builder-side mirror of the ratified 16 MiB source envelope.
// The exact semantic-literal ceiling remains a v0 ratification item.
pub(crate) const MAX_LITERAL_BYTES: usize = 16 * 1024 * 1024;
// A flat one-byte literal can otherwise amplify into millions of `Value`
// objects during decoding while remaining inside the byte envelope.
pub(crate) const MAX_LITERAL_NODES: usize = 262_144;

pub(crate) fn validate_literal(
    value: &Value,
    remaining_bytes: &mut usize,
    remaining_nodes: &mut usize,
) -> FlowResult<()> {
    validate_at(value, 0, remaining_bytes, remaining_nodes)
}

fn validate_at(
    value: &Value,
    depth: usize,
    remaining_bytes: &mut usize,
    remaining_nodes: &mut usize,
) -> FlowResult<()> {
    if depth > braid_ir::canon::MAX_DEPTH {
        return Err(FlowError::LimitExceeded {
            kind: LimitKind::LiteralDepth,
            actual: depth,
            limit: braid_ir::canon::MAX_DEPTH,
            invariant: "INV-FLOW-004",
        });
    }
    *remaining_nodes = remaining_nodes
        .checked_sub(1)
        .ok_or(FlowError::LimitExceeded {
            kind: LimitKind::LiteralNodes,
            actual: MAX_LITERAL_NODES + 1,
            limit: MAX_LITERAL_NODES,
            invariant: "INV-FLOW-004",
        })?;

    match value {
        Value::Bool(_) => consume(1, remaining_bytes)?,
        Value::Int(value) => consume(integer_len(*value), remaining_bytes)?,
        Value::Bytes(bytes) => {
            consume(head_len(bytes.len()), remaining_bytes)?;
            consume(bytes.len(), remaining_bytes)?;
        }
        Value::Text(text) => {
            consume(head_len(text.len()), remaining_bytes)?;
            consume(text.len(), remaining_bytes)?;
        }
        Value::List(items) => {
            consume(head_len(items.len()), remaining_bytes)?;
            for item in items {
                validate_at(item, depth + 1, remaining_bytes, remaining_nodes)?;
            }
        }
        Value::Map(entries) => {
            consume(head_len(entries.len()), remaining_bytes)?;
            for (key, item) in entries {
                consume(head_len(key.len()), remaining_bytes)?;
                consume(key.len(), remaining_bytes)?;
                validate_at(item, depth + 1, remaining_bytes, remaining_nodes)?;
            }
        }
    }
    Ok(())
}

fn integer_len(value: i64) -> usize {
    let argument = if value >= 0 {
        value as u64
    } else {
        (-1i128 - i128::from(value)) as u64
    };
    head_len_u64(argument)
}

fn head_len(value: usize) -> usize {
    head_len_u64(value as u64)
}

const fn head_len_u64(value: u64) -> usize {
    match value {
        0..=23 => 1,
        24..=0xff => 2,
        0x100..=0xffff => 3,
        0x1_0000..=0xffff_ffff => 5,
        _ => 9,
    }
}

fn consume(amount: usize, remaining_bytes: &mut usize) -> FlowResult<()> {
    *remaining_bytes = remaining_bytes
        .checked_sub(amount)
        .ok_or(FlowError::LimitExceeded {
            kind: LimitKind::LiteralBytes,
            actual: MAX_LITERAL_BYTES + 1,
            limit: MAX_LITERAL_BYTES,
            invariant: "INV-FLOW-004",
        })?;
    Ok(())
}
