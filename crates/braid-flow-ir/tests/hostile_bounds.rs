mod common;

use braid_flow_ir::{
    ChoiceArm, FlowBounds, FlowEdge, FlowError, FlowInput, FlowName, FlowNode, FlowNodeKind,
    FlowSpec, InputPort, LimitKind, PortKey, Predicate, UrgencyClass, ValueExpr, ValueSource,
};
use braid_ir::{TypeTag, TypeTagError, Value};

fn try_flow_with_root_type(value_type: TypeTag) -> Result<FlowSpec, FlowError> {
    FlowSpec::new(
        FlowName::new("type-identity").unwrap(),
        vec![FlowInput {
            key: braid_flow_ir::InputKey::new("input").unwrap(),
            value_type,
        }],
        vec![common::terminal("done")],
        vec![],
        vec![common::key("done")],
        FlowBounds::default(),
    )
}

fn flow_with_root_type(value_type: TypeTag) -> FlowSpec {
    try_flow_with_root_type(value_type).unwrap()
}

fn flow_with_literal(value: Value) -> Result<FlowSpec, FlowError> {
    FlowSpec::new(
        FlowName::new("literal-bound").unwrap(),
        vec![FlowInput {
            key: braid_flow_ir::InputKey::new("input").unwrap(),
            value_type: TypeTag::Bool,
        }],
        vec![common::terminal("done")],
        vec![FlowEdge::Data {
            from: ValueSource::Literal(value),
            to: InputPort {
                node: common::key("done"),
                port: PortKey::new("input").unwrap(),
            },
            value_type: TypeTag::Bool,
        }],
        vec![common::key("done")],
        FlowBounds::default(),
    )
}

fn nested_literal(depth: usize) -> Value {
    let mut value = Value::Bool(true);
    for _ in 0..depth {
        value = Value::List(vec![value]);
    }
    value
}

fn nested_and(depth: usize) -> Predicate {
    assert!(depth > 0);
    let mut predicate = Predicate::Const(true);
    for _ in 1..depth {
        predicate = Predicate::And(vec![predicate]);
    }
    predicate
}

fn choice_flow(predicate: Predicate) -> Result<FlowSpec, FlowError> {
    FlowSpec::new(
        FlowName::new("wire-depth").unwrap(),
        vec![],
        vec![
            FlowNode {
                key: common::key("choose"),
                kind: FlowNodeKind::Choice {
                    arms: vec![ChoiceArm {
                        when: predicate,
                        then: common::key("selected"),
                    }],
                    otherwise: common::key("fallback"),
                },
                guard: Predicate::Const(true),
                justification: None,
                urgency: UrgencyClass::Required,
            },
            common::terminal("selected"),
            common::terminal("fallback"),
        ],
        vec![],
        vec![common::key("fallback"), common::key("selected")],
        FlowBounds::default(),
    )
}

fn opaque_chain(nodes: usize) -> TypeTag {
    assert!(nodes > 0);
    let mut value_type = TypeTag::Bool;
    for _ in 1..nodes {
        value_type = TypeTag::Opaque("layer".into(), vec![value_type]);
    }
    value_type
}

#[test]
fn rejects_before_allocation() {
    let bounds = FlowBounds {
        max_nodes: 1,
        ..FlowBounds::default()
    };
    let nodes = vec![common::invocation("first", 1), common::terminal("done")];

    let error = FlowSpec::new(
        FlowName::new("bounded").unwrap(),
        vec![braid_flow_ir::FlowInput {
            key: braid_flow_ir::InputKey::new("input").unwrap(),
            value_type: braid_ir::TypeTag::Bool,
        }],
        nodes,
        vec![],
        vec![common::key("done")],
        bounds,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        FlowError::LimitExceeded {
            kind: LimitKind::SourceNodes,
            actual: 2,
            limit: 1,
            ..
        }
    ));
}

#[test]
fn rejects_a_declared_bound_above_the_protocol_ceiling() {
    let bounds = FlowBounds {
        max_nodes: 10_001,
        ..FlowBounds::default()
    };
    let error = FlowSpec::new(
        FlowName::new("bounded").unwrap(),
        vec![braid_flow_ir::FlowInput {
            key: braid_flow_ir::InputKey::new("input").unwrap(),
            value_type: braid_ir::TypeTag::Bool,
        }],
        vec![common::terminal("done")],
        vec![],
        vec![common::key("done")],
        bounds,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        FlowError::InvalidBound {
            kind: LimitKind::SourceNodes,
            requested: 10_001,
            hard_limit: 10_000,
            ..
        }
    ));
}

#[test]
fn rootless_edgeless_flow_accepts_zero_edge_ceilings() {
    let bounds = FlowBounds {
        max_edges: 0,
        max_expanded_edges: 0,
        ..FlowBounds::default()
    };
    let flow = FlowSpec::new(
        FlowName::new("closed").unwrap(),
        vec![],
        vec![common::terminal("done")],
        vec![],
        vec![common::key("done")],
        bounds,
    )
    .unwrap();

    assert!(flow.roots().is_empty());
    assert!(flow.edges().is_empty());
}

#[test]
fn structurally_distinct_type_tags_cannot_alias_one_flow_cid() {
    let primitive = flow_with_root_type(TypeTag::Bool);
    let nominal = flow_with_root_type(TypeTag::Opaque("bool".into(), vec![]));
    assert_ne!(primitive.cid(), nominal.cid());

    let structural = flow_with_root_type(TypeTag::List(Box::new(TypeTag::Bool)));
    let nominal = flow_with_root_type(TypeTag::Opaque("list".into(), vec![TypeTag::Bool]));
    assert_ne!(structural.cid(), nominal.cid());
}

#[test]
fn rejects_type_tags_over_the_total_node_budget() {
    let branch = TypeTag::Opaque("branch".into(), vec![TypeTag::Bool; 128]);
    let value_type = TypeTag::Opaque("root".into(), vec![branch; 128]);
    let error = try_flow_with_root_type(value_type).unwrap_err();

    assert!(matches!(
        error,
        FlowError::InvalidTypeTag {
            error: TypeTagError::TooManyNodes { count: 16_385 },
            ..
        }
    ));
}

#[test]
fn rejects_aggregate_type_tag_work_across_the_flow() {
    let value_type = TypeTag::Opaque("branch".into(), vec![TypeTag::Bool; 128]);
    let roots = (0..2_033)
        .map(|index| FlowInput {
            key: braid_flow_ir::InputKey::new(&format!("input.{index}")).unwrap(),
            value_type: value_type.clone(),
        })
        .collect();
    let error = FlowSpec::new(
        FlowName::new("aggregate-types").unwrap(),
        roots,
        vec![common::terminal("done")],
        vec![],
        vec![common::key("done")],
        FlowBounds::default(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        FlowError::LimitExceeded {
            kind: LimitKind::TypeTagNodes,
            limit: 262_144,
            ..
        }
    ));
}

#[test]
fn rejects_literal_nesting_beyond_the_canonical_decoder_limit() {
    let mut value = Value::Bool(true);
    for _ in 0..=braid_ir::canon::MAX_DEPTH {
        value = Value::List(vec![value]);
    }
    let error = flow_with_literal(value).unwrap_err();

    assert!(matches!(
        error,
        FlowError::LimitExceeded {
            kind: LimitKind::LiteralDepth,
            actual: 65,
            limit: 64,
            ..
        }
    ));
}

#[test]
fn rejects_literal_canonical_bytes_over_the_global_budget() {
    let value = Value::Bytes(vec![0; 16 * 1024 * 1024]);
    let error = flow_with_literal(value).unwrap_err();

    assert!(matches!(
        error,
        FlowError::LimitExceeded {
            kind: LimitKind::LiteralBytes,
            limit: 16_777_216,
            ..
        }
    ));
}

#[test]
fn rejects_flat_literal_value_amplification() {
    let value = Value::List(vec![Value::Bool(true); 262_144]);
    let error = flow_with_literal(value).unwrap_err();

    assert!(matches!(
        error,
        FlowError::LimitExceeded {
            kind: LimitKind::LiteralNodes,
            actual: 262_145,
            limit: 262_144,
            ..
        }
    ));
}

#[test]
fn canonical_outer_count_is_refused_by_preflight() {
    let mut bytes = common::flow_with_orders(false).canonical_bytes();
    let marker = [0x65, b'r', b'o', b'o', b't', b's', 0x82];
    let position = bytes
        .windows(marker.len())
        .position(|window| window == marker)
        .unwrap()
        + marker.len()
        - 1;
    bytes.splice(position..=position, [0x99, 0x27, 0x11]);

    assert!(matches!(
        FlowSpec::from_bytes(&bytes),
        Err(FlowError::LimitExceeded {
            kind: LimitKind::Roots,
            actual: 10_001,
            limit: 10_000,
            ..
        })
    ));
}

#[test]
fn declared_count_bound_is_refused_before_semantic_allocation() {
    let flow = common::flow_with_orders(false);
    let mut value = flow.to_canon();
    let Value::Map(fields) = &mut value else {
        panic!("Flow is a map")
    };
    let Value::Map(bounds) = fields.get_mut("bounds").unwrap() else {
        panic!("bounds is a map")
    };
    bounds.insert("max_nodes".into(), Value::Int(2));
    let bytes = braid_ir::encode(&value);

    assert!(matches!(
        FlowSpec::from_bytes(&bytes),
        Err(FlowError::LimitExceeded {
            kind: LimitKind::SourceNodes,
            actual: 3,
            limit: 2,
            ..
        })
    ));
}

#[test]
fn aggregate_references_are_refused_by_raw_preflight() {
    let flow = common::flow_with_orders(false);
    let mut value = flow.to_canon();
    let Value::Map(fields) = &mut value else {
        panic!("Flow is a map")
    };
    let Value::List(nodes) = fields.get_mut("nodes").unwrap() else {
        panic!("nodes is a list")
    };
    let mut template = nodes
        .iter()
        .find(|node| node.get_field("justification").is_some())
        .unwrap()
        .clone();
    let Value::Map(node) = &mut template else {
        panic!("node is a map")
    };
    let Value::Map(justification) = node.get_mut("justification").unwrap() else {
        panic!("justification is a map")
    };
    let references = vec![Value::Text("relation.x".into()); 128];
    justification.insert("guarantees".into(), Value::List(references.clone()));
    justification.insert("preserves".into(), Value::List(references));
    *nodes = vec![template; 65];

    assert!(matches!(
        FlowSpec::from_bytes(&braid_ir::encode(&value)),
        Err(FlowError::LimitExceeded {
            kind: LimitKind::References,
            actual: 16_512,
            limit: 16_384,
            ..
        })
    ));
}

#[test]
fn aggregate_literal_nodes_are_refused_by_raw_preflight() {
    let flow = flow_with_literal(Value::Bool(true)).unwrap();
    let mut value = flow.to_canon();
    let Value::Map(fields) = &mut value else {
        panic!("Flow is a map")
    };
    let Value::List(edges) = fields.get_mut("edges").unwrap() else {
        panic!("edges is a list")
    };
    let Value::Map(edge) = edges.first_mut().unwrap() else {
        panic!("edge is a map")
    };
    let Value::Map(data) = edge.get_mut("data").unwrap() else {
        panic!("data is a map")
    };
    let Value::Map(source) = data.get_mut("from").unwrap() else {
        panic!("source is a map")
    };
    source.insert(
        "literal".into(),
        Value::List(vec![Value::Bool(true); 100_000]),
    );
    *edges = vec![Value::Map(edge.clone()); 3];

    assert!(matches!(
        FlowSpec::from_bytes(&braid_ir::encode(&value)),
        Err(FlowError::LimitExceeded {
            kind: LimitKind::LiteralNodes,
            actual: 262_145,
            limit: 262_144,
            ..
        })
    ));
}

#[test]
fn aggregate_type_nodes_are_refused_by_raw_preflight() {
    let value_type = TypeTag::Opaque("branch".into(), vec![TypeTag::Bool; 128]);
    let flow = flow_with_root_type(value_type);
    let mut value = flow.to_canon();
    let Value::Map(fields) = &mut value else {
        panic!("Flow is a map")
    };
    let Value::List(roots) = fields.get_mut("roots").unwrap() else {
        panic!("roots is a list")
    };
    *roots = vec![roots[0].clone(); 2_033];

    assert!(matches!(
        FlowSpec::from_bytes(&braid_ir::encode(&value)),
        Err(FlowError::LimitExceeded {
            kind: LimitKind::TypeTagNodes,
            actual: 262_145,
            limit: 262_144,
            ..
        })
    ));
}

#[test]
fn aggregate_predicate_nodes_are_refused_by_raw_preflight() {
    let comparisons = (0..1_000)
        .map(|number| {
            Predicate::Eq(
                ValueExpr::Literal(Value::Int(number)),
                ValueExpr::Literal(Value::Int(number + 1)),
            )
        })
        .collect();
    let mut terminal = common::terminal("done");
    terminal.guard = Predicate::And(comparisons);
    let flow = FlowSpec::new(
        FlowName::new("predicate-preflight").unwrap(),
        vec![],
        vec![terminal],
        vec![],
        vec![common::key("done")],
        FlowBounds::default(),
    )
    .unwrap();
    let mut value = flow.to_canon();
    let Value::Map(fields) = &mut value else {
        panic!("Flow is a map")
    };
    let Value::List(nodes) = fields.get_mut("nodes").unwrap() else {
        panic!("nodes is a list")
    };
    *nodes = vec![nodes[0].clone(); 17];

    assert!(matches!(
        FlowSpec::from_bytes(&braid_ir::encode(&value)),
        Err(FlowError::LimitExceeded {
            kind: LimitKind::PredicateNodes,
            actual: 16_385,
            limit: 16_384,
            ..
        })
    ));
}

#[test]
fn oversized_completion_list_is_refused_by_raw_preflight() {
    let flow = common::flow_with_orders(false);
    let mut value = flow.to_canon();
    let Value::Map(fields) = &mut value else {
        panic!("Flow is a map")
    };
    let Value::List(edges) = fields.get_mut("edges").unwrap() else {
        panic!("edges is a list")
    };
    let Value::Map(after) = edges
        .iter_mut()
        .find_map(|edge| match edge {
            Value::Map(outer) => outer.get_mut("after"),
            _ => None,
        })
        .unwrap()
    else {
        panic!("after payload is a map")
    };
    after.insert("on".into(), Value::List(vec![Value::Bool(true); 4]));

    assert!(matches!(
        FlowSpec::from_bytes(&braid_ir::encode(&value)),
        Err(FlowError::LimitExceeded {
            kind: LimitKind::CompletionClasses,
            actual: 4,
            limit: 3,
            ..
        })
    ));
}

#[test]
fn oversized_port_tuple_is_refused_by_raw_preflight() {
    let flow = common::flow_with_orders(false);
    let mut value = flow.to_canon();
    let Value::Map(fields) = &mut value else {
        panic!("Flow is a map")
    };
    let Value::List(edges) = fields.get_mut("edges").unwrap() else {
        panic!("edges is a list")
    };
    let Value::Map(data) = edges
        .iter_mut()
        .find_map(|edge| match edge {
            Value::Map(outer) => outer.get_mut("data"),
            _ => None,
        })
        .unwrap()
    else {
        panic!("data payload is a map")
    };
    data.insert(
        "to".into(),
        Value::List(vec![
            Value::Text("scope".into()),
            Value::Text("input".into()),
            Value::Text("extra".into()),
        ]),
    );

    assert!(matches!(
        FlowSpec::from_bytes(&braid_ir::encode(&value)),
        Err(FlowError::LimitExceeded {
            kind: LimitKind::CanonicalValues,
            actual: 3,
            limit: 2,
            ..
        })
    ));
}

#[test]
fn maximum_projected_choice_literal_depth_round_trips() {
    let predicate = Predicate::Eq(
        ValueExpr::Literal(nested_literal(54)),
        ValueExpr::Literal(Value::Bool(true)),
    );
    let flow = choice_flow(predicate).unwrap();
    assert_eq!(FlowSpec::from_bytes(&flow.canonical_bytes()).unwrap(), flow);

    let too_deep = Predicate::Eq(
        ValueExpr::Literal(nested_literal(55)),
        ValueExpr::Literal(Value::Bool(true)),
    );
    assert!(matches!(
        choice_flow(too_deep),
        Err(FlowError::LimitExceeded {
            kind: LimitKind::CanonicalDepth,
            actual: 65,
            limit: 64,
            ..
        })
    ));
}

#[test]
fn maximum_projected_choice_predicate_depth_round_trips() {
    let flow = choice_flow(nested_and(29)).unwrap();
    assert_eq!(FlowSpec::from_bytes(&flow.canonical_bytes()).unwrap(), flow);

    assert!(matches!(
        choice_flow(nested_and(30)),
        Err(FlowError::LimitExceeded {
            kind: LimitKind::CanonicalDepth,
            actual: 65,
            limit: 64,
            ..
        })
    ));
}

#[test]
fn maximum_projected_opaque_type_depth_round_trips() {
    let flow = flow_with_root_type(opaque_chain(21));
    assert_eq!(FlowSpec::from_bytes(&flow.canonical_bytes()).unwrap(), flow);

    assert!(matches!(
        try_flow_with_root_type(opaque_chain(22)),
        Err(FlowError::LimitExceeded {
            kind: LimitKind::CanonicalDepth,
            actual: 65,
            limit: 64,
            ..
        })
    ));
}
