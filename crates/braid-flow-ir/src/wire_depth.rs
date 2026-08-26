//! Exact, allocation-free mirror of the canonical Flow value's nesting.
//!
//! Semantic recursion limits are not enough: Flow's maps and lists add wire
//! levels around predicates, literals, and types. This walk keeps the builder
//! and strict decoder on one depth contract without materializing a second
//! `Value` tree.

use crate::error::{FlowError, FlowResult, LimitKind};
use crate::flow::{FlowEdge, FlowNodeKind, FlowSpec, JustificationDecl, ValueSource};
use crate::predicate::{Predicate, ValueExpr};
use braid_ir::{TypeTag, Value};

const LIMIT: usize = braid_ir::canon::MAX_DEPTH;

pub(crate) fn validate(flow: &FlowSpec) -> FlowResult<()> {
    check(0)?; // Flow map.
    check(1)?; // name, roots/nodes/edges/terminals lists, bounds map, version.

    for root in &flow.roots {
        check(2)?; // root map
        check(3)?; // key and type
        type_tag(&root.value_type, 3)?;
    }
    for node in &flow.nodes {
        check(2)?; // node map
        check(3)?; // node fields
        node_kind(&node.kind, 3)?;
        predicate(&node.guard, 3)?;
        if let Some(justification) = &node.justification {
            justification_depth(justification, 3)?;
        }
    }
    for edge in &flow.edges {
        edge_depth(edge, 2)?;
    }
    check(2)?; // terminal list items and bounds scalar values.
    Ok(())
}

fn check(depth: usize) -> FlowResult<()> {
    if depth > LIMIT {
        Err(FlowError::LimitExceeded {
            kind: LimitKind::CanonicalDepth,
            actual: depth,
            limit: LIMIT,
            invariant: "INV-FLOW-018",
        })
    } else {
        Ok(())
    }
}

fn type_tag(value_type: &TypeTag, depth: usize) -> FlowResult<()> {
    check(depth)?; // variant map
    match value_type {
        TypeTag::Bool | TypeTag::Int | TypeTag::Bytes | TypeTag::Text | TypeTag::Cid => {
            check(depth + 1)
        }
        TypeTag::List(inner) => type_tag(inner, depth + 1),
        TypeTag::Opaque(_, arguments) => {
            check(depth + 1)?; // opaque payload map
            check(depth + 2)?; // label and arguments list
            for argument in arguments {
                type_tag(argument, depth + 3)?;
            }
            Ok(())
        }
    }
}

fn node_kind(kind: &FlowNodeKind, depth: usize) -> FlowResult<()> {
    check(depth)?;
    match kind {
        FlowNodeKind::InvokeCapsule { .. } | FlowNodeKind::Terminal { .. } => check(depth + 1),
        FlowNodeKind::JoinAll => Ok(()),
        FlowNodeKind::Choice { arms, .. } => {
            check(depth + 1)?; // choice payload map
            check(depth + 2)?; // arms list and otherwise
            for arm in arms {
                check(depth + 3)?; // arm map
                check(depth + 4)?; // then and when
                predicate(&arm.when, depth + 4)?;
            }
            Ok(())
        }
    }
}

fn justification_depth(value: &JustificationDecl, depth: usize) -> FlowResult<()> {
    check(depth)?; // justification map
    check(depth + 1)?; // predicates, reference lists, optional cost order
    predicate(&value.needed_when, depth + 1)?;
    predicate(&value.satisfied_when, depth + 1)?;
    if !value.guarantees.is_empty() || !value.preserves.is_empty() {
        check(depth + 2)?; // reference list items
    }
    Ok(())
}

fn edge_depth(edge: &FlowEdge, depth: usize) -> FlowResult<()> {
    check(depth)?; // edge variant map
    check(depth + 1)?; // variant payload map
    match edge {
        FlowEdge::Data {
            from, value_type, ..
        } => {
            check(depth + 2)?; // from map, to list, type map
            value_source(from, depth + 2)?;
            check(depth + 3)?; // to tuple items
            type_tag(value_type, depth + 2)
        }
        FlowEdge::After { .. } => {
            check(depth + 2)?; // from, to, on list
            check(depth + 3) // completion list items
        }
    }
}

fn value_source(source: &ValueSource, depth: usize) -> FlowResult<()> {
    check(depth)?; // source variant map
    match source {
        ValueSource::Root(_) => check(depth + 1),
        ValueSource::Node(_) => {
            check(depth + 1)?; // tuple list
            check(depth + 2) // tuple items
        }
        ValueSource::Literal(value) => literal(value, depth + 1),
    }
}

fn predicate(value: &Predicate, depth: usize) -> FlowResult<()> {
    check(depth)?; // predicate variant map
    match value {
        Predicate::Const(_) => check(depth + 1),
        Predicate::Eq(left, right)
        | Predicate::Ne(left, right)
        | Predicate::Lt(left, right)
        | Predicate::Le(left, right)
        | Predicate::Gt(left, right)
        | Predicate::Ge(left, right) => {
            check(depth + 1)?; // comparison list
            value_expr(left, depth + 2)?;
            value_expr(right, depth + 2)
        }
        Predicate::And(items) | Predicate::Or(items) => {
            check(depth + 1)?; // operands list
            for item in items {
                predicate(item, depth + 2)?;
            }
            Ok(())
        }
        Predicate::Not(inner) => predicate(inner, depth + 1),
        Predicate::HasCompletion { .. } => {
            check(depth + 1)?; // tuple list
            check(depth + 2) // tuple items
        }
    }
}

fn value_expr(value: &ValueExpr, depth: usize) -> FlowResult<()> {
    check(depth)?; // expression variant map
    match value {
        ValueExpr::Literal(value) => literal(value, depth + 1),
        ValueExpr::RootInput(_) | ValueExpr::SnapshotFact(_) => check(depth + 1),
        ValueExpr::NodeOutput(_) => {
            check(depth + 1)?; // tuple list
            check(depth + 2) // tuple items
        }
    }
}

fn literal(value: &Value, depth: usize) -> FlowResult<()> {
    check(depth)?;
    match value {
        Value::List(items) => {
            for item in items {
                literal(item, depth + 1)?;
            }
        }
        Value::Map(entries) => {
            for item in entries.values() {
                literal(item, depth + 1)?;
            }
        }
        Value::Bool(_) | Value::Int(_) | Value::Bytes(_) | Value::Text(_) => {}
    }
    Ok(())
}
