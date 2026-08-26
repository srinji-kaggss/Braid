mod common;

use braid_flow_ir::{
    ChoiceArm, CompletionClass, CostOrderRef, FactRef, FlowBounds, FlowEdge, FlowError, FlowInput,
    FlowName, FlowNode, FlowNodeKind, FlowSpec, InputKey, InputPort, InvariantRef,
    JustificationDecl, OutputPort, PortKey, Predicate, RelationRef, TerminalOutcome, UrgencyClass,
    ValueExpr, ValueSource,
};
use braid_ir::{Cid, TypeTag, Value, encode};

fn full_variant_flow() -> FlowSpec {
    let composite_type = TypeTag::Opaque(
        "lw.example.composite".into(),
        vec![
            TypeTag::Bool,
            TypeTag::Int,
            TypeTag::Bytes,
            TypeTag::Text,
            TypeTag::Cid,
            TypeTag::List(Box::new(TypeTag::Opaque("lw.example.item".into(), vec![]))),
        ],
    );
    let output = OutputPort {
        node: common::key("produce"),
        port: PortKey::new("result").unwrap(),
    };
    let comparisons = vec![
        Predicate::Eq(
            ValueExpr::RootInput(InputKey::new("source").unwrap()),
            ValueExpr::Literal(Value::Text("ready".into())),
        ),
        Predicate::Ne(
            ValueExpr::SnapshotFact(FactRef::new("state.old").unwrap()),
            ValueExpr::Literal(Value::Bool(true)),
        ),
        Predicate::Lt(
            ValueExpr::Literal(Value::Int(1)),
            ValueExpr::Literal(Value::Int(2)),
        ),
        Predicate::Le(
            ValueExpr::Literal(Value::Int(2)),
            ValueExpr::Literal(Value::Int(2)),
        ),
        Predicate::Gt(
            ValueExpr::NodeOutput(output.clone()),
            ValueExpr::Literal(Value::Int(0)),
        ),
        Predicate::Ge(
            ValueExpr::Literal(Value::Int(3)),
            ValueExpr::Literal(Value::Int(2)),
        ),
        Predicate::Not(Box::new(Predicate::Const(false))),
        Predicate::Or(vec![
            Predicate::Const(false),
            Predicate::HasCompletion {
                node: common::key("produce"),
                class: CompletionClass::ExecutedSuccess,
            },
        ]),
    ];
    let justification = JustificationDecl {
        needed_when: Predicate::And(comparisons),
        satisfied_when: Predicate::HasCompletion {
            node: common::key("produce"),
            class: CompletionClass::SatisfiedWithoutExecution,
        },
        guarantees: vec![RelationRef::new("relation.output.exists").unwrap()],
        preserves: vec![InvariantRef::new("invariant.source.stable").unwrap()],
        cost_order: Some(CostOrderRef::new("cost.lower.first").unwrap()),
    };
    let nodes = vec![
        FlowNode {
            key: common::key("produce"),
            kind: FlowNodeKind::InvokeCapsule {
                capsule: Cid([7; 32]),
            },
            guard: Predicate::Const(true),
            justification: Some(justification),
            urgency: UrgencyClass::SafetyRecovery,
        },
        FlowNode {
            key: common::key("join"),
            kind: FlowNodeKind::JoinAll,
            guard: Predicate::Const(true),
            justification: None,
            urgency: UrgencyClass::Diagnostic,
        },
        FlowNode {
            key: common::key("choose"),
            kind: FlowNodeKind::Choice {
                arms: vec![ChoiceArm {
                    when: Predicate::Const(false),
                    then: common::key("accepted"),
                }],
                otherwise: common::key("rejected"),
            },
            guard: Predicate::Const(true),
            justification: None,
            urgency: UrgencyClass::Optimization,
        },
        FlowNode {
            key: common::key("accepted"),
            kind: FlowNodeKind::Terminal {
                outcome: TerminalOutcome::Success,
            },
            guard: Predicate::Const(true),
            justification: None,
            urgency: UrgencyClass::Required,
        },
        FlowNode {
            key: common::key("rejected"),
            kind: FlowNodeKind::Terminal {
                outcome: TerminalOutcome::Failure,
            },
            guard: Predicate::Const(true),
            justification: None,
            urgency: UrgencyClass::Cleanup,
        },
    ];
    let edges = vec![
        FlowEdge::Data {
            from: ValueSource::Root(InputKey::new("source").unwrap()),
            to: InputPort {
                node: common::key("produce"),
                port: PortKey::new("source").unwrap(),
            },
            value_type: composite_type.clone(),
        },
        FlowEdge::Data {
            from: ValueSource::Node(output),
            to: InputPort {
                node: common::key("join"),
                port: PortKey::new("generated").unwrap(),
            },
            value_type: TypeTag::Int,
        },
        FlowEdge::Data {
            from: ValueSource::Literal(Value::Bytes(vec![1, 2, 3])),
            to: InputPort {
                node: common::key("join"),
                port: PortKey::new("literal").unwrap(),
            },
            value_type: TypeTag::Bytes,
        },
        FlowEdge::After {
            from: common::key("produce"),
            to: common::key("join"),
            on: vec![
                CompletionClass::ExecutedSuccess,
                CompletionClass::SatisfiedWithoutExecution,
                CompletionClass::Failure,
            ],
        },
        common::after("join", "choose"),
    ];
    FlowSpec::new(
        FlowName::new("all-closed-variants").unwrap(),
        vec![FlowInput {
            key: InputKey::new("source").unwrap(),
            value_type: composite_type,
        }],
        nodes,
        edges,
        vec![common::key("accepted"), common::key("rejected")],
        FlowBounds::default(),
    )
    .unwrap()
}

fn canonical_value(flow: &FlowSpec) -> Value {
    braid_ir::decode_strict(&flow.canonical_bytes()).unwrap()
}

#[test]
fn every_closed_variant_round_trips_exactly() {
    let original = full_variant_flow();
    let decoded = FlowSpec::from_bytes(&original.canonical_bytes()).unwrap();

    assert_eq!(decoded, original);
    assert_eq!(decoded.cid(), original.cid());
}

#[test]
fn known_answer_vector_decodes_to_the_pinned_identity() {
    let expected = common::flow_with_orders(false);
    let decoded = FlowSpec::from_bytes(&expected.canonical_bytes()).unwrap();

    assert_eq!(decoded.canonical_bytes(), expected.canonical_bytes());
    assert_eq!(decoded.cid(), expected.cid());
}

#[test]
fn unknown_top_level_field_is_rejected() {
    let flow = full_variant_flow();
    let mut value = canonical_value(&flow);
    let Value::Map(fields) = &mut value else {
        panic!("Flow wire is a map")
    };
    fields.insert("future".into(), Value::Bool(true));

    assert!(matches!(
        FlowSpec::from_bytes(&encode(&value)),
        Err(FlowError::Malformed { field: "flow", .. })
    ));
}

#[test]
fn unknown_nested_field_is_rejected() {
    let flow = full_variant_flow();
    let mut value = canonical_value(&flow);
    let Value::Map(flow_fields) = &mut value else {
        panic!("Flow wire is a map")
    };
    let Value::List(nodes) = flow_fields.get_mut("nodes").unwrap() else {
        panic!("nodes is a list")
    };
    let Value::Map(node) = nodes.first_mut().unwrap() else {
        panic!("node is a map")
    };
    node.insert("future".into(), Value::Bool(true));

    assert!(matches!(
        FlowSpec::from_bytes(&encode(&value)),
        Err(FlowError::Malformed { field: "node", .. })
    ));
}

#[test]
fn semantic_source_order_is_rejected_on_the_wire() {
    let flow = full_variant_flow();
    let mut value = canonical_value(&flow);
    let Value::Map(flow_fields) = &mut value else {
        panic!("Flow wire is a map")
    };
    let Value::List(nodes) = flow_fields.get_mut("nodes").unwrap() else {
        panic!("nodes is a list")
    };
    nodes.reverse();

    assert!(matches!(
        FlowSpec::from_bytes(&encode(&value)),
        Err(FlowError::NonBijective { .. })
    ));
}

#[test]
fn duplicate_semantic_reference_is_not_silently_normalized() {
    let flow = full_variant_flow();
    let mut value = canonical_value(&flow);
    let Value::Map(flow_fields) = &mut value else {
        panic!("Flow wire is a map")
    };
    let Value::List(nodes) = flow_fields.get_mut("nodes").unwrap() else {
        panic!("nodes is a list")
    };
    let Value::Map(produce) = nodes
        .iter_mut()
        .find(|node| node.get_field("key") == Some(&Value::Text("produce".into())))
        .unwrap()
    else {
        panic!("node is a map")
    };
    let Value::Map(justification) = produce.get_mut("justification").unwrap() else {
        panic!("justification is a map")
    };
    let Value::List(guarantees) = justification.get_mut("guarantees").unwrap() else {
        panic!("guarantees is a list")
    };
    guarantees.push(guarantees[0].clone());

    assert!(matches!(
        FlowSpec::from_bytes(&encode(&value)),
        Err(FlowError::NonBijective { .. })
    ));
}

#[test]
fn recursively_noncanonical_predicate_operands_are_rejected() {
    let flow = full_variant_flow();
    let mut value = canonical_value(&flow);
    let Value::Map(flow_fields) = &mut value else {
        panic!("Flow wire is a map")
    };
    let Value::List(nodes) = flow_fields.get_mut("nodes").unwrap() else {
        panic!("nodes is a list")
    };
    let Value::Map(produce) = nodes
        .iter_mut()
        .find(|node| node.get_field("key") == Some(&Value::Text("produce".into())))
        .unwrap()
    else {
        panic!("node is a map")
    };
    let Value::Map(justification) = produce.get_mut("justification").unwrap() else {
        panic!("justification is a map")
    };
    let Value::Map(needed_when) = justification.get_mut("needed_when").unwrap() else {
        panic!("needed_when is a predicate map")
    };
    let Value::List(operands) = needed_when.get_mut("and").unwrap() else {
        panic!("and payload is a list")
    };
    operands.reverse();
    operands.push(operands[0].clone());

    assert!(matches!(
        FlowSpec::from_bytes(&encode(&value)),
        Err(FlowError::NonBijective { .. })
    ));
}

#[test]
fn non_minimal_cbor_is_rejected_before_schema_projection() {
    let error = FlowSpec::from_bytes(&[0x18, 0x00]).unwrap_err();

    assert!(matches!(error, FlowError::Canon(_)));
}

#[test]
fn unknown_closed_variant_is_rejected() {
    let flow = full_variant_flow();
    let mut value = canonical_value(&flow);
    let Value::Map(flow_fields) = &mut value else {
        panic!("Flow wire is a map")
    };
    let Value::List(nodes) = flow_fields.get_mut("nodes").unwrap() else {
        panic!("nodes is a list")
    };
    let Value::Map(node) = nodes.first_mut().unwrap() else {
        panic!("node is a map")
    };
    node.insert("urgency".into(), Value::Text("whatever".into()));

    assert!(matches!(
        FlowSpec::from_bytes(&encode(&value)),
        Err(FlowError::Malformed {
            field: "urgency",
            ..
        })
    ));
}
