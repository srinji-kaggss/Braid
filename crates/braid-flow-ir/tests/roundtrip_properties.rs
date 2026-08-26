mod common;

use braid_flow_ir::{
    ChoiceArm, FlowBounds, FlowEdge, FlowInput, FlowName, FlowNode, FlowNodeKind, FlowSpec,
    InputPort, PortKey, Predicate, UrgencyClass, ValueExpr, ValueSource,
};
use braid_ir::{TypeTag, Value};
use proptest::prelude::*;
use std::collections::BTreeMap;

fn rank(seed: u64, label: &str) -> u64 {
    label.bytes().fold(seed, |state, byte| {
        state
            .rotate_left(7)
            .wrapping_mul(0x9e37_79b9_7f4a_7c15)
            .wrapping_add(u64::from(byte))
    })
}

fn permuted_flow(seed: u64) -> FlowSpec {
    let base = common::flow_with_orders(false);
    let mut roots = base.roots().to_vec();
    let mut nodes = base.nodes().to_vec();
    let mut edges = base.edges().to_vec();
    roots.sort_by_key(|root| rank(seed, root.key.as_str()));
    nodes.sort_by_key(|node| rank(seed.rotate_left(13), node.key.as_str()));
    edges.sort_by_key(|edge| rank(seed.rotate_left(29), &format!("{edge:?}")));
    FlowSpec::new(
        base.name().clone(),
        roots,
        nodes,
        edges,
        base.terminals().to_vec(),
        base.bounds(),
    )
    .unwrap()
}

fn literal_strategy() -> BoxedStrategy<Value> {
    let leaf = prop_oneof![
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(Value::Int),
        proptest::collection::vec(any::<u8>(), 0..16).prop_map(Value::Bytes),
        "[a-z0-9]{0,16}".prop_map(Value::Text),
    ];
    leaf.prop_recursive(4, 64, 4, |inner| {
        prop_oneof![
            proptest::collection::vec(inner.clone(), 0..4).prop_map(Value::List),
            proptest::collection::vec((0u8..8, inner), 0..4).prop_map(|entries| {
                Value::Map(
                    entries
                        .into_iter()
                        .map(|(key, value)| (format!("k{key}"), value))
                        .collect::<BTreeMap<_, _>>(),
                )
            }),
        ]
    })
    .boxed()
}

fn type_strategy() -> BoxedStrategy<TypeTag> {
    prop_oneof![
        Just(TypeTag::Bool),
        Just(TypeTag::Int),
        Just(TypeTag::Bytes),
        Just(TypeTag::Text),
        Just(TypeTag::Cid),
    ]
    .prop_recursive(4, 64, 4, |inner| {
        prop_oneof![
            inner.clone().prop_map(|item| TypeTag::List(Box::new(item))),
            (0u8..8, proptest::collection::vec(inner, 0..4))
                .prop_map(|(label, arguments)| TypeTag::Opaque(format!("type.{label}"), arguments)),
        ]
    })
    .boxed()
}

fn predicate_strategy() -> BoxedStrategy<Predicate> {
    let comparison =
        (0u8..6, literal_strategy(), literal_strategy()).prop_map(|(variant, left, right)| {
            let left = ValueExpr::Literal(left);
            let right = ValueExpr::Literal(right);
            match variant {
                0 => Predicate::Eq(left, right),
                1 => Predicate::Ne(left, right),
                2 => Predicate::Lt(left, right),
                3 => Predicate::Le(left, right),
                4 => Predicate::Gt(left, right),
                _ => Predicate::Ge(left, right),
            }
        });
    prop_oneof![any::<bool>().prop_map(Predicate::Const), comparison]
        .prop_recursive(4, 64, 4, |inner| {
            prop_oneof![
                inner
                    .clone()
                    .prop_map(|item| Predicate::Not(Box::new(item))),
                proptest::collection::vec(inner.clone(), 1..4).prop_map(Predicate::And),
                proptest::collection::vec(inner, 1..4).prop_map(Predicate::Or),
            ]
        })
        .boxed()
}

fn generated_flow(value_type: TypeTag, predicate: Predicate, literal: Value) -> FlowSpec {
    FlowSpec::new(
        FlowName::new("generated").unwrap(),
        vec![FlowInput {
            key: braid_flow_ir::InputKey::new("source").unwrap(),
            value_type,
        }],
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
        vec![FlowEdge::Data {
            from: ValueSource::Literal(literal),
            to: InputPort {
                node: common::key("selected"),
                port: PortKey::new("value").unwrap(),
            },
            value_type: TypeTag::Bool,
        }],
        vec![common::key("selected"), common::key("fallback")],
        FlowBounds::default(),
    )
    .unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn canonical_flow_bytes_decode_bijectively(seed in any::<u64>()) {
        let flow = permuted_flow(seed);
        let bytes = flow.canonical_bytes();
        let decoded = FlowSpec::from_bytes(&bytes).unwrap();

        prop_assert_eq!(decoded.canonical_bytes(), bytes);
        prop_assert_eq!(decoded, flow);
    }

    #[test]
    fn generated_recursive_asts_decode_bijectively(
        value_type in type_strategy(),
        predicate in predicate_strategy(),
        literal in literal_strategy(),
    ) {
        let flow = generated_flow(value_type, predicate, literal);
        let bytes = flow.canonical_bytes();
        let decoded = FlowSpec::from_bytes(&bytes).unwrap();

        prop_assert_eq!(decoded.canonical_bytes(), bytes);
        prop_assert_eq!(decoded, flow);
    }

    #[test]
    fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..4096)) {
        drop(FlowSpec::from_bytes(&bytes));
    }

    #[test]
    fn generated_declaration_orders_cannot_change_identity(
        left_seed in any::<u64>(),
        right_seed in any::<u64>(),
    ) {
        let left = permuted_flow(left_seed);
        let right = permuted_flow(right_seed);

        prop_assert_eq!(left.canonical_bytes(), right.canonical_bytes());
        prop_assert_eq!(left.cid(), right.cid());
    }

    #[test]
    fn any_single_capsule_bit_changes_flow_identity(bit in 0usize..256) {
        let original = common::flow_with_orders(false);
        let mut nodes = original.nodes().to_vec();
        let invoke = nodes
            .iter_mut()
            .find(|node| node.key.as_str() == "scope")
            .unwrap();
        let FlowNodeKind::InvokeCapsule { capsule } = &invoke.kind else {
            panic!("scope is an invocation")
        };
        let mut changed = capsule.0;
        changed[bit / 8] ^= 1 << (bit % 8);
        invoke.kind = FlowNodeKind::InvokeCapsule {
            capsule: braid_ir::Cid(changed),
        };
        let changed = FlowSpec::new(
            original.name().clone(),
            original.roots().to_vec(),
            nodes,
            original.edges().to_vec(),
            original.terminals().to_vec(),
            original.bounds(),
        )
        .unwrap();

        prop_assert_ne!(original.cid(), changed.cid());
    }
}
