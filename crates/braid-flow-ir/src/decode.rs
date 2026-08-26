//! Strict semantic decoder for the canonical Flow wire.
//!
//! This is the authoring-side decoder for P1. P2 deliberately supplies its
//! own decoder so producer and admission do not share a normalization path.

use crate::error::{FlowError, FlowResult, IdentifierError, LimitKind};
use crate::flow::{
    ChoiceArm, CompletionClass, FLOW_VERSION, FlowBounds, FlowEdge, FlowInput, FlowNode,
    FlowNodeKind, FlowSpec, HARD_MAX_CHOICE_ARMS, HARD_MAX_PREDICATE_NODES, InputPort,
    JustificationDecl, OutputPort, PROVISIONAL_MAX_JUSTIFICATION_REFERENCES, TerminalOutcome,
    UrgencyClass, ValueSource,
};
use crate::predicate::{Predicate, ValueExpr};
use crate::symbol::{
    CostOrderRef, FactRef, FlowName, InputKey, InvariantRef, NodeKey, PortKey, RelationRef,
};
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use braid_ir::{Cid, TypeTag, Value, decode_strict};
use core::str::FromStr;

const INV_SHAPE: &str = "INV-FLOW-018";

pub(crate) fn decode_flow(bytes: &[u8]) -> FlowResult<FlowSpec> {
    crate::preflight::validate(bytes)?;
    let value = decode_strict(bytes)?;
    let flow = decode_flow_value(&value)?;
    if flow.canonical_bytes() != bytes {
        return Err(FlowError::NonBijective {
            invariant: INV_SHAPE,
        });
    }
    Ok(flow)
}

fn malformed(field: &'static str) -> FlowError {
    FlowError::Malformed {
        field,
        invariant: INV_SHAPE,
    }
}

fn as_map<'a>(
    value: &'a Value,
    allowed: &[&str],
    field: &'static str,
) -> FlowResult<&'a BTreeMap<String, Value>> {
    let Value::Map(items) = value else {
        return Err(malformed(field));
    };
    if items.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(malformed(field));
    }
    Ok(items)
}

fn required<'a>(map: &'a BTreeMap<String, Value>, key: &'static str) -> FlowResult<&'a Value> {
    map.get(key).ok_or_else(|| malformed(key))
}

fn as_list<'a>(value: &'a Value, field: &'static str) -> FlowResult<&'a [Value]> {
    match value {
        Value::List(items) => Ok(items),
        _ => Err(malformed(field)),
    }
}

fn as_text<'a>(value: &'a Value, field: &'static str) -> FlowResult<&'a str> {
    match value {
        Value::Text(text) => Ok(text),
        _ => Err(malformed(field)),
    }
}

fn as_u32(value: &Value, field: &'static str) -> FlowResult<u32> {
    match value {
        Value::Int(number) => u32::try_from(*number).map_err(|_| malformed(field)),
        _ => Err(malformed(field)),
    }
}

fn as_u16(value: &Value, field: &'static str) -> FlowResult<u16> {
    match value {
        Value::Int(number) => u16::try_from(*number).map_err(|_| malformed(field)),
        _ => Err(malformed(field)),
    }
}

fn symbol<T>(value: &Value, field: &'static str) -> FlowResult<T>
where
    T: FromStr<Err = IdentifierError>,
{
    as_text(value, field)?.parse().map_err(FlowError::from)
}

fn single_variant<'a>(
    value: &'a Value,
    allowed: &[&str],
    field: &'static str,
) -> FlowResult<(&'a str, &'a Value)> {
    let map = as_map(value, allowed, field)?;
    if map.len() != 1 {
        return Err(malformed(field));
    }
    let (key, payload) = map.iter().next().ok_or_else(|| malformed(field))?;
    Ok((key.as_str(), payload))
}

fn decode_flow_value(value: &Value) -> FlowResult<FlowSpec> {
    let map = as_map(
        value,
        &[
            "name",
            "roots",
            "nodes",
            "edges",
            "terminals",
            "bounds",
            "version",
        ],
        "flow",
    )?;
    let version = as_u16(required(map, "version")?, "version")?;
    if version != FLOW_VERSION {
        return Err(FlowError::UnsupportedVersion {
            found: version,
            expected: FLOW_VERSION,
            invariant: INV_SHAPE,
        });
    }
    let name = symbol::<FlowName>(required(map, "name")?, "name")?;
    let bounds = decode_bounds(required(map, "bounds")?)?;
    bounds.validate()?;
    let roots = decode_list(
        required(map, "roots")?,
        "roots",
        bounds.max_nodes as usize,
        LimitKind::Roots,
        decode_root,
    )?;
    let nodes = decode_list(
        required(map, "nodes")?,
        "nodes",
        bounds.max_nodes as usize,
        LimitKind::SourceNodes,
        decode_node,
    )?;
    let edges = decode_list(
        required(map, "edges")?,
        "edges",
        bounds.max_edges as usize,
        LimitKind::SourceEdges,
        decode_edge,
    )?;
    let terminals = decode_list(
        required(map, "terminals")?,
        "terminals",
        bounds.max_nodes as usize,
        LimitKind::Terminals,
        |item| symbol::<NodeKey>(item, "terminal"),
    )?;
    FlowSpec::new(name, roots, nodes, edges, terminals, bounds)
}

fn decode_list<T>(
    value: &Value,
    field: &'static str,
    limit: usize,
    kind: LimitKind,
    decode_item: fn(&Value) -> FlowResult<T>,
) -> FlowResult<Vec<T>> {
    let items = as_list(value, field)?;
    if items.len() > limit {
        return Err(FlowError::LimitExceeded {
            kind,
            actual: items.len(),
            limit,
            invariant: "INV-FLOW-004",
        });
    }
    let mut decoded = Vec::with_capacity(items.len());
    for item in items {
        decoded.push(decode_item(item)?);
    }
    Ok(decoded)
}

fn decode_root(value: &Value) -> FlowResult<FlowInput> {
    let map = as_map(value, &["key", "type"], "root")?;
    Ok(FlowInput {
        key: symbol::<InputKey>(required(map, "key")?, "root key")?,
        value_type: decode_type(required(map, "type")?)?,
    })
}

fn decode_bounds(value: &Value) -> FlowResult<FlowBounds> {
    let map = as_map(
        value,
        &[
            "max_nodes",
            "max_edges",
            "max_predicate_depth",
            "max_expanded_nodes",
            "max_expanded_edges",
        ],
        "bounds",
    )?;
    Ok(FlowBounds {
        max_nodes: as_u32(required(map, "max_nodes")?, "max_nodes")?,
        max_edges: as_u32(required(map, "max_edges")?, "max_edges")?,
        max_predicate_depth: as_u16(required(map, "max_predicate_depth")?, "max_predicate_depth")?,
        max_expanded_nodes: as_u32(required(map, "max_expanded_nodes")?, "max_expanded_nodes")?,
        max_expanded_edges: as_u32(required(map, "max_expanded_edges")?, "max_expanded_edges")?,
    })
}

fn decode_type(value: &Value) -> FlowResult<TypeTag> {
    let (variant, payload) = single_variant(value, &["primitive", "opaque", "list"], "type")?;
    match variant {
        "primitive" => match as_text(payload, "primitive type")? {
            "bool" => Ok(TypeTag::Bool),
            "int" => Ok(TypeTag::Int),
            "bytes" => Ok(TypeTag::Bytes),
            "text" => Ok(TypeTag::Text),
            "cid" => Ok(TypeTag::Cid),
            _ => Err(malformed("primitive type")),
        },
        "opaque" => decode_opaque_type(payload),
        "list" => Ok(TypeTag::List(Box::new(decode_type(payload)?))),
        _ => Err(malformed("type")),
    }
}

fn decode_opaque_type(value: &Value) -> FlowResult<TypeTag> {
    let map = as_map(value, &["label", "arguments"], "opaque type")?;
    let label = as_text(required(map, "label")?, "opaque label")?.into();
    let arguments = decode_list(
        required(map, "arguments")?,
        "opaque arguments",
        128,
        LimitKind::TypeTagNodes,
        decode_type,
    )?;
    Ok(TypeTag::Opaque(label, arguments))
}

fn decode_node(value: &Value) -> FlowResult<FlowNode> {
    let map = as_map(
        value,
        &["key", "kind", "guard", "justification", "urgency"],
        "node",
    )?;
    Ok(FlowNode {
        key: symbol::<NodeKey>(required(map, "key")?, "node key")?,
        kind: decode_node_kind(required(map, "kind")?)?,
        guard: decode_predicate(required(map, "guard")?)?,
        justification: map
            .get("justification")
            .map(decode_justification)
            .transpose()?,
        urgency: decode_urgency(required(map, "urgency")?)?,
    })
}

fn decode_node_kind(value: &Value) -> FlowResult<FlowNodeKind> {
    if matches!(value, Value::Text(text) if text == "join_all") {
        return Ok(FlowNodeKind::JoinAll);
    }
    let (variant, payload) = single_variant(
        value,
        &["invoke_capsule", "choice", "terminal"],
        "node kind",
    )?;
    match variant {
        "invoke_capsule" => Ok(FlowNodeKind::InvokeCapsule {
            capsule: decode_cid(payload)?,
        }),
        "choice" => decode_choice(payload),
        "terminal" => Ok(FlowNodeKind::Terminal {
            outcome: decode_terminal_outcome(payload)?,
        }),
        _ => Err(malformed("node kind")),
    }
}

fn decode_cid(value: &Value) -> FlowResult<Cid> {
    let Value::Bytes(bytes) = value else {
        return Err(malformed("capsule cid"));
    };
    let bytes: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| malformed("capsule cid"))?;
    Ok(Cid(bytes))
}

fn decode_choice(value: &Value) -> FlowResult<FlowNodeKind> {
    let map = as_map(value, &["arms", "otherwise"], "choice")?;
    Ok(FlowNodeKind::Choice {
        arms: decode_list(
            required(map, "arms")?,
            "choice arms",
            HARD_MAX_CHOICE_ARMS,
            LimitKind::ChoiceArms,
            decode_choice_arm,
        )?,
        otherwise: symbol::<NodeKey>(required(map, "otherwise")?, "choice otherwise")?,
    })
}

fn decode_choice_arm(value: &Value) -> FlowResult<ChoiceArm> {
    let map = as_map(value, &["then", "when"], "choice arm")?;
    Ok(ChoiceArm {
        when: decode_predicate(required(map, "when")?)?,
        then: symbol::<NodeKey>(required(map, "then")?, "choice target")?,
    })
}

fn decode_terminal_outcome(value: &Value) -> FlowResult<TerminalOutcome> {
    match as_text(value, "terminal outcome")? {
        "success" => Ok(TerminalOutcome::Success),
        "failure" => Ok(TerminalOutcome::Failure),
        _ => Err(malformed("terminal outcome")),
    }
}

fn decode_urgency(value: &Value) -> FlowResult<UrgencyClass> {
    match as_text(value, "urgency")? {
        "safety_recovery" => Ok(UrgencyClass::SafetyRecovery),
        "required" => Ok(UrgencyClass::Required),
        "diagnostic" => Ok(UrgencyClass::Diagnostic),
        "optimization" => Ok(UrgencyClass::Optimization),
        "cleanup" => Ok(UrgencyClass::Cleanup),
        _ => Err(malformed("urgency")),
    }
}

fn decode_justification(value: &Value) -> FlowResult<JustificationDecl> {
    let map = as_map(
        value,
        &[
            "needed_when",
            "satisfied_when",
            "guarantees",
            "preserves",
            "cost_order",
        ],
        "justification",
    )?;
    Ok(JustificationDecl {
        needed_when: decode_predicate(required(map, "needed_when")?)?,
        satisfied_when: decode_predicate(required(map, "satisfied_when")?)?,
        guarantees: decode_symbols::<RelationRef>(
            required(map, "guarantees")?,
            "guarantees",
            PROVISIONAL_MAX_JUSTIFICATION_REFERENCES,
        )?,
        preserves: decode_symbols::<InvariantRef>(
            required(map, "preserves")?,
            "preserves",
            PROVISIONAL_MAX_JUSTIFICATION_REFERENCES,
        )?,
        cost_order: map
            .get("cost_order")
            .map(|item| symbol::<CostOrderRef>(item, "cost_order"))
            .transpose()?,
    })
}

fn decode_symbols<T>(value: &Value, field: &'static str, limit: usize) -> FlowResult<Vec<T>>
where
    T: FromStr<Err = IdentifierError>,
{
    let items = as_list(value, field)?;
    if items.len() > limit {
        return Err(FlowError::LimitExceeded {
            kind: LimitKind::References,
            actual: items.len(),
            limit,
            invariant: "INV-FLOW-004",
        });
    }
    let mut decoded = Vec::with_capacity(items.len());
    for item in items {
        decoded.push(symbol::<T>(item, field)?);
    }
    Ok(decoded)
}

fn decode_edge(value: &Value) -> FlowResult<FlowEdge> {
    let (variant, payload) = single_variant(value, &["data", "after"], "edge")?;
    match variant {
        "data" => decode_data_edge(payload),
        "after" => decode_after_edge(payload),
        _ => Err(malformed("edge")),
    }
}

fn decode_data_edge(value: &Value) -> FlowResult<FlowEdge> {
    let map = as_map(value, &["from", "to", "type"], "data edge")?;
    Ok(FlowEdge::Data {
        from: decode_value_source(required(map, "from")?)?,
        to: decode_input_port(required(map, "to")?)?,
        value_type: decode_type(required(map, "type")?)?,
    })
}

fn decode_after_edge(value: &Value) -> FlowResult<FlowEdge> {
    let map = as_map(value, &["from", "to", "on"], "after edge")?;
    Ok(FlowEdge::After {
        from: symbol::<NodeKey>(required(map, "from")?, "after source")?,
        to: symbol::<NodeKey>(required(map, "to")?, "after destination")?,
        on: decode_list(
            required(map, "on")?,
            "completion classes",
            3,
            LimitKind::CompletionClasses,
            decode_completion,
        )?,
    })
}

fn decode_value_source(value: &Value) -> FlowResult<ValueSource> {
    let (variant, payload) = single_variant(value, &["root", "node", "literal"], "value source")?;
    match variant {
        "root" => Ok(ValueSource::Root(symbol::<InputKey>(
            payload,
            "root source",
        )?)),
        "node" => Ok(ValueSource::Node(decode_output_port(payload)?)),
        "literal" => Ok(ValueSource::Literal(payload.clone())),
        _ => Err(malformed("value source")),
    }
}

fn decode_input_port(value: &Value) -> FlowResult<InputPort> {
    let (node, port) = decode_port_tuple(value, "input port")?;
    Ok(InputPort { node, port })
}

fn decode_output_port(value: &Value) -> FlowResult<OutputPort> {
    let (node, port) = decode_port_tuple(value, "output port")?;
    Ok(OutputPort { node, port })
}

fn decode_port_tuple(value: &Value, field: &'static str) -> FlowResult<(NodeKey, PortKey)> {
    let items = as_list(value, field)?;
    let [node, port] = items else {
        return Err(malformed(field));
    };
    Ok((
        symbol::<NodeKey>(node, field)?,
        symbol::<PortKey>(port, field)?,
    ))
}

fn decode_completion(value: &Value) -> FlowResult<CompletionClass> {
    match as_text(value, "completion class")? {
        "executed_success" => Ok(CompletionClass::ExecutedSuccess),
        "satisfied_without_execution" => Ok(CompletionClass::SatisfiedWithoutExecution),
        "failure" => Ok(CompletionClass::Failure),
        _ => Err(malformed("completion class")),
    }
}

fn decode_predicate(value: &Value) -> FlowResult<Predicate> {
    let (variant, payload) = single_variant(
        value,
        &[
            "const",
            "eq",
            "ne",
            "lt",
            "le",
            "gt",
            "ge",
            "and",
            "or",
            "not",
            "has_completion",
        ],
        "predicate",
    )?;
    match variant {
        "const" => decode_const(payload),
        "eq" => decode_comparison(payload, Predicate::Eq),
        "ne" => decode_comparison(payload, Predicate::Ne),
        "lt" => decode_comparison(payload, Predicate::Lt),
        "le" => decode_comparison(payload, Predicate::Le),
        "gt" => decode_comparison(payload, Predicate::Gt),
        "ge" => decode_comparison(payload, Predicate::Ge),
        "and" => Ok(Predicate::And(decode_predicates(payload, "and")?)),
        "or" => Ok(Predicate::Or(decode_predicates(payload, "or")?)),
        "not" => Ok(Predicate::Not(Box::new(decode_predicate(payload)?))),
        "has_completion" => decode_has_completion(payload),
        _ => Err(malformed("predicate")),
    }
}

fn decode_const(value: &Value) -> FlowResult<Predicate> {
    match value {
        Value::Bool(flag) => Ok(Predicate::Const(*flag)),
        _ => Err(malformed("const predicate")),
    }
}

fn decode_comparison(
    value: &Value,
    constructor: fn(ValueExpr, ValueExpr) -> Predicate,
) -> FlowResult<Predicate> {
    let items = as_list(value, "comparison")?;
    let [left, right] = items else {
        return Err(malformed("comparison"));
    };
    Ok(constructor(
        decode_value_expr(left)?,
        decode_value_expr(right)?,
    ))
}

fn decode_predicates(value: &Value, field: &'static str) -> FlowResult<Vec<Predicate>> {
    decode_list(
        value,
        field,
        HARD_MAX_PREDICATE_NODES,
        LimitKind::PredicateNodes,
        decode_predicate,
    )
}

fn decode_has_completion(value: &Value) -> FlowResult<Predicate> {
    let items = as_list(value, "has_completion")?;
    let [node, class] = items else {
        return Err(malformed("has_completion"));
    };
    Ok(Predicate::HasCompletion {
        node: symbol::<NodeKey>(node, "completion node")?,
        class: decode_completion(class)?,
    })
}

fn decode_value_expr(value: &Value) -> FlowResult<ValueExpr> {
    let (variant, payload) = single_variant(
        value,
        &["literal", "root_input", "node_output", "snapshot_fact"],
        "value expression",
    )?;
    match variant {
        "literal" => Ok(ValueExpr::Literal(payload.clone())),
        "root_input" => Ok(ValueExpr::RootInput(symbol::<InputKey>(
            payload,
            "root input",
        )?)),
        "node_output" => Ok(ValueExpr::NodeOutput(decode_output_port(payload)?)),
        "snapshot_fact" => Ok(ValueExpr::SnapshotFact(symbol::<FactRef>(
            payload,
            "snapshot fact",
        )?)),
        _ => Err(malformed("value expression")),
    }
}
