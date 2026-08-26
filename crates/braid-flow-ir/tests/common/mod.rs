#![allow(dead_code)]

use braid_flow_ir::{
    CompletionClass, FactRef, FlowBounds, FlowEdge, FlowInput, FlowName, FlowNode, FlowNodeKind,
    FlowSpec, InputKey, InputPort, JustificationDecl, NodeKey, PortKey, Predicate, RelationRef,
    TerminalOutcome, UrgencyClass, ValueExpr, ValueSource,
};
use braid_ir::{Cid, TypeTag, Value};

pub fn key(value: &str) -> NodeKey {
    NodeKey::new(value).unwrap()
}

pub fn invocation(name: &str, capsule_byte: u8) -> FlowNode {
    FlowNode {
        key: key(name),
        kind: FlowNodeKind::InvokeCapsule {
            capsule: Cid([capsule_byte; 32]),
        },
        guard: Predicate::Const(true),
        justification: Some(justification(name)),
        urgency: UrgencyClass::Required,
    }
}

pub fn justification(name: &str) -> JustificationDecl {
    let need = format!("need.{name}");
    let done = format!("done.{name}");
    let guarantee = format!("guarantee.{name}");
    JustificationDecl {
        needed_when: Predicate::Eq(
            ValueExpr::SnapshotFact(FactRef::new(&need).unwrap()),
            ValueExpr::Literal(Value::Bool(true)),
        ),
        satisfied_when: Predicate::Eq(
            ValueExpr::SnapshotFact(FactRef::new(&done).unwrap()),
            ValueExpr::Literal(Value::Bool(true)),
        ),
        guarantees: vec![RelationRef::new(&guarantee).unwrap()],
        preserves: vec![],
        cost_order: None,
    }
}

pub fn terminal(name: &str) -> FlowNode {
    FlowNode {
        key: key(name),
        kind: FlowNodeKind::Terminal {
            outcome: TerminalOutcome::Success,
        },
        guard: Predicate::Const(true),
        justification: None,
        urgency: UrgencyClass::Required,
    }
}

pub fn after(from: &str, to: &str) -> FlowEdge {
    FlowEdge::After {
        from: key(from),
        to: key(to),
        on: vec![
            CompletionClass::SatisfiedWithoutExecution,
            CompletionClass::ExecutedSuccess,
        ],
    }
}

pub fn root_edge(root: &str, node: &str) -> FlowEdge {
    FlowEdge::Data {
        from: ValueSource::Root(InputKey::new(root).unwrap()),
        to: InputPort {
            node: key(node),
            port: PortKey::new("input").unwrap(),
        },
        value_type: TypeTag::Bool,
    }
}

pub fn flow_with_orders(reverse: bool) -> FlowSpec {
    let mut roots = vec![
        FlowInput {
            key: InputKey::new("source.a").unwrap(),
            value_type: TypeTag::Bool,
        },
        FlowInput {
            key: InputKey::new("source.b").unwrap(),
            value_type: TypeTag::Bool,
        },
    ];
    let mut nodes = vec![
        invocation("scope", 1),
        invocation("build", 2),
        terminal("accepted"),
    ];
    let mut edges = vec![
        root_edge("source.a", "scope"),
        root_edge("source.b", "build"),
        after("scope", "build"),
        after("build", "accepted"),
    ];
    if reverse {
        roots.reverse();
        nodes.reverse();
        edges.reverse();
    }
    FlowSpec::new(
        FlowName::new("braid-ci").unwrap(),
        roots,
        nodes,
        edges,
        vec![key("accepted")],
        FlowBounds::default(),
    )
    .unwrap()
}
