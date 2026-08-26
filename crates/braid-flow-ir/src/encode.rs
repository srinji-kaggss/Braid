//! Projection into `braid_ir::Value`. The sole canonical encoder remains
//! `braid_ir::encode`; this module only defines Flow's semantic value shape.

use crate::flow::{
    ChoiceArm, CompletionClass, FlowEdge, FlowNode, FlowNodeKind, FlowSpec, JustificationDecl,
    TerminalOutcome, UrgencyClass, ValueSource,
};
use crate::predicate::{Predicate, ValueExpr};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use braid_ir::{TypeTag, Value, encode};

pub(crate) fn flow_to_canon(flow: &FlowSpec) -> Value {
    Value::map(vec![
        ("name", Value::Text(flow.name.to_string())),
        (
            "roots",
            Value::List(flow.roots.iter().map(root_to_canon).collect()),
        ),
        (
            "nodes",
            Value::List(flow.nodes.iter().map(node_to_canon).collect()),
        ),
        (
            "edges",
            Value::List(flow.edges.iter().map(edge_to_canon).collect()),
        ),
        (
            "terminals",
            Value::List(
                flow.terminals
                    .iter()
                    .map(|key| Value::Text(key.to_string()))
                    .collect(),
            ),
        ),
        ("bounds", bounds_to_canon(flow)),
        ("version", Value::Int(i64::from(flow.version))),
    ])
}

fn root_to_canon(root: &crate::flow::FlowInput) -> Value {
    Value::map(vec![
        ("key", Value::Text(root.key.to_string())),
        ("type", type_to_canon(&root.value_type)),
    ])
}

fn bounds_to_canon(flow: &FlowSpec) -> Value {
    Value::map(vec![
        ("max_nodes", Value::Int(i64::from(flow.bounds.max_nodes))),
        ("max_edges", Value::Int(i64::from(flow.bounds.max_edges))),
        (
            "max_predicate_depth",
            Value::Int(i64::from(flow.bounds.max_predicate_depth)),
        ),
        (
            "max_expanded_nodes",
            Value::Int(i64::from(flow.bounds.max_expanded_nodes)),
        ),
        (
            "max_expanded_edges",
            Value::Int(i64::from(flow.bounds.max_expanded_edges)),
        ),
    ])
}

fn type_to_canon(value_type: &TypeTag) -> Value {
    match value_type {
        TypeTag::Bool => Value::map(vec![("primitive", Value::Text("bool".into()))]),
        TypeTag::Int => Value::map(vec![("primitive", Value::Text("int".into()))]),
        TypeTag::Bytes => Value::map(vec![("primitive", Value::Text("bytes".into()))]),
        TypeTag::Text => Value::map(vec![("primitive", Value::Text("text".into()))]),
        TypeTag::Cid => Value::map(vec![("primitive", Value::Text("cid".into()))]),
        TypeTag::Opaque(label, arguments) => Value::map(vec![(
            "opaque",
            Value::map(vec![
                ("label", Value::Text(label.clone())),
                (
                    "arguments",
                    Value::List(arguments.iter().map(type_to_canon).collect()),
                ),
            ]),
        )]),
        TypeTag::List(inner) => Value::map(vec![("list", type_to_canon(inner))]),
    }
}

fn node_to_canon(node: &FlowNode) -> Value {
    let mut fields = vec![
        ("key", Value::Text(node.key.to_string())),
        ("kind", node_kind_to_canon(&node.kind)),
        ("guard", predicate_to_canon(&node.guard)),
        ("urgency", urgency_to_canon(node.urgency)),
    ];
    if let Some(justification) = &node.justification {
        fields.push(("justification", justification_to_canon(justification)));
    }
    Value::map(fields)
}

fn node_kind_to_canon(kind: &FlowNodeKind) -> Value {
    match kind {
        FlowNodeKind::InvokeCapsule { capsule } => {
            Value::map(vec![("invoke_capsule", Value::Bytes(capsule.0.to_vec()))])
        }
        FlowNodeKind::Choice { arms, otherwise } => Value::map(vec![(
            "choice",
            Value::map(vec![
                (
                    "arms",
                    Value::List(arms.iter().map(choice_arm_to_canon).collect()),
                ),
                ("otherwise", Value::Text(otherwise.to_string())),
            ]),
        )]),
        FlowNodeKind::JoinAll => Value::Text("join_all".into()),
        FlowNodeKind::Terminal { outcome } => Value::map(vec![(
            "terminal",
            Value::Text(
                match outcome {
                    TerminalOutcome::Success => "success",
                    TerminalOutcome::Failure => "failure",
                }
                .into(),
            ),
        )]),
    }
}

fn choice_arm_to_canon(arm: &ChoiceArm) -> Value {
    Value::map(vec![
        ("then", Value::Text(arm.then.to_string())),
        ("when", predicate_to_canon(&arm.when)),
    ])
}

fn justification_to_canon(justification: &JustificationDecl) -> Value {
    let mut fields = vec![
        (
            "needed_when",
            predicate_to_canon(&justification.needed_when),
        ),
        (
            "satisfied_when",
            predicate_to_canon(&justification.satisfied_when),
        ),
        (
            "guarantees",
            Value::List(
                justification
                    .guarantees
                    .iter()
                    .map(|item| Value::Text(item.to_string()))
                    .collect(),
            ),
        ),
        (
            "preserves",
            Value::List(
                justification
                    .preserves
                    .iter()
                    .map(|item| Value::Text(item.to_string()))
                    .collect(),
            ),
        ),
    ];
    if let Some(cost_order) = &justification.cost_order {
        fields.push(("cost_order", Value::Text(cost_order.to_string())));
    }
    Value::map(fields)
}

fn urgency_to_canon(urgency: UrgencyClass) -> Value {
    Value::Text(
        match urgency {
            UrgencyClass::SafetyRecovery => "safety_recovery",
            UrgencyClass::Required => "required",
            UrgencyClass::Diagnostic => "diagnostic",
            UrgencyClass::Optimization => "optimization",
            UrgencyClass::Cleanup => "cleanup",
        }
        .into(),
    )
}

pub(crate) fn edge_to_canon(edge: &FlowEdge) -> Value {
    match edge {
        FlowEdge::Data {
            from,
            to,
            value_type,
        } => Value::map(vec![(
            "data",
            Value::map(vec![
                ("from", value_source_to_canon(from)),
                (
                    "to",
                    Value::List(vec![
                        Value::Text(to.node.to_string()),
                        Value::Text(to.port.to_string()),
                    ]),
                ),
                ("type", type_to_canon(value_type)),
            ]),
        )]),
        FlowEdge::After { from, to, on } => Value::map(vec![(
            "after",
            Value::map(vec![
                ("from", Value::Text(from.to_string())),
                ("to", Value::Text(to.to_string())),
                (
                    "on",
                    Value::List(on.iter().map(|class| completion_to_canon(*class)).collect()),
                ),
            ]),
        )]),
    }
}

fn value_source_to_canon(source: &ValueSource) -> Value {
    match source {
        ValueSource::Root(root) => Value::map(vec![("root", Value::Text(root.to_string()))]),
        ValueSource::Node(output) => Value::map(vec![(
            "node",
            Value::List(vec![
                Value::Text(output.node.to_string()),
                Value::Text(output.port.to_string()),
            ]),
        )]),
        ValueSource::Literal(value) => Value::map(vec![("literal", value.clone())]),
    }
}

fn completion_to_canon(class: CompletionClass) -> Value {
    Value::Text(
        match class {
            CompletionClass::ExecutedSuccess => "executed_success",
            CompletionClass::SatisfiedWithoutExecution => "satisfied_without_execution",
            CompletionClass::Failure => "failure",
        }
        .into(),
    )
}

pub(crate) fn predicate_to_canon(predicate: &Predicate) -> Value {
    match predicate {
        Predicate::Const(value) => Value::map(vec![("const", Value::Bool(*value))]),
        Predicate::Eq(left, right) => comparison_to_canon("eq", left, right),
        Predicate::Ne(left, right) => comparison_to_canon("ne", left, right),
        Predicate::Lt(left, right) => comparison_to_canon("lt", left, right),
        Predicate::Le(left, right) => comparison_to_canon("le", left, right),
        Predicate::Gt(left, right) => comparison_to_canon("gt", left, right),
        Predicate::Ge(left, right) => comparison_to_canon("ge", left, right),
        Predicate::And(items) => boolean_list_to_canon("and", items),
        Predicate::Or(items) => boolean_list_to_canon("or", items),
        Predicate::Not(inner) => Value::map(vec![("not", predicate_to_canon(inner))]),
        Predicate::HasCompletion { node, class } => Value::map(vec![(
            "has_completion",
            Value::List(vec![
                Value::Text(node.to_string()),
                completion_to_canon(*class),
            ]),
        )]),
    }
}

fn comparison_to_canon(kind: &'static str, left: &ValueExpr, right: &ValueExpr) -> Value {
    Value::map(vec![(
        kind,
        Value::List(vec![value_expr_to_canon(left), value_expr_to_canon(right)]),
    )])
}

fn boolean_list_to_canon(kind: &'static str, items: &[Predicate]) -> Value {
    let mut canonical: Vec<Value> = items.iter().map(predicate_to_canon).collect();
    canonical.sort_by_cached_key(encode);
    canonical.dedup();
    Value::map(vec![(kind, Value::List(canonical))])
}

fn value_expr_to_canon(expression: &ValueExpr) -> Value {
    match expression {
        ValueExpr::Literal(value) => Value::map(vec![("literal", value.clone())]),
        ValueExpr::RootInput(input) => {
            Value::map(vec![("root_input", Value::Text(input.to_string()))])
        }
        ValueExpr::NodeOutput(output) => Value::map(vec![(
            "node_output",
            Value::List(vec![
                Value::Text(output.node.to_string()),
                Value::Text(output.port.to_string()),
            ]),
        )]),
        ValueExpr::SnapshotFact(fact) => {
            Value::map(vec![("snapshot_fact", Value::Text(fact.to_string()))])
        }
    }
}
