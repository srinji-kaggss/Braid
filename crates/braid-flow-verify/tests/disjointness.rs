use braid_flow_ir::{
    CompletionClass, FactRef, FlowBounds, FlowName, FlowNode, FlowNodeKind, FlowSpec, Predicate,
    TerminalOutcome, UrgencyClass, ValueExpr,
};
use braid_flow_verify::{
    CompletionWitness, Disjointness, DisjointnessUnknown, FlowVerifyError, SolverLimit,
    analyze_disjointness, verify,
};
use braid_ir::Value;
use proptest::prelude::*;

fn fact(name: &str) -> ValueExpr {
    ValueExpr::SnapshotFact(FactRef::new(name).unwrap())
}

fn int(value: i64) -> ValueExpr {
    ValueExpr::Literal(Value::Int(value))
}

fn predicate_for(operation: u8, value: i64) -> Predicate {
    let left = fact("choice.value");
    let right = int(value);
    match operation {
        0 => Predicate::Eq(left, right),
        1 => Predicate::Ne(left, right),
        2 => Predicate::Lt(left, right),
        3 => Predicate::Le(left, right),
        4 => Predicate::Gt(left, right),
        5 => Predicate::Ge(left, right),
        _ => unreachable!(),
    }
}

fn eval(predicate: &Predicate, value: i64) -> bool {
    match predicate {
        Predicate::Const(value) => *value,
        Predicate::Eq(left, right) => eval_values(left, right, value, |a, b| a == b),
        Predicate::Ne(left, right) => eval_values(left, right, value, |a, b| a != b),
        Predicate::Lt(left, right) => eval_values(left, right, value, |a, b| a < b),
        Predicate::Le(left, right) => eval_values(left, right, value, |a, b| a <= b),
        Predicate::Gt(left, right) => eval_values(left, right, value, |a, b| a > b),
        Predicate::Ge(left, right) => eval_values(left, right, value, |a, b| a >= b),
        Predicate::And(items) => items.iter().all(|item| eval(item, value)),
        Predicate::Or(items) => items.iter().any(|item| eval(item, value)),
        Predicate::Not(inner) => !eval(inner, value),
        Predicate::HasCompletion { .. } => unreachable!("property grammar has no completions"),
    }
}

fn eval_values(
    left: &ValueExpr,
    right: &ValueExpr,
    value: i64,
    compare: impl FnOnce(i64, i64) -> bool,
) -> bool {
    compare(eval_expr(left, value), eval_expr(right, value))
}

fn eval_expr(expression: &ValueExpr, value: i64) -> i64 {
    match expression {
        ValueExpr::SnapshotFact(_) => value,
        ValueExpr::Literal(Value::Int(value)) => *value,
        _ => unreachable!("property grammar is integer fact/literal only"),
    }
}

fn predicate_strategy() -> impl Strategy<Value = Predicate> {
    let leaf = prop_oneof![
        any::<bool>().prop_map(Predicate::Const),
        (0u8..6, -1i64..=1).prop_map(|(operation, value)| predicate_for(operation, value)),
    ];
    leaf.prop_recursive(3, 24, 3, |inner| {
        prop_oneof![
            inner
                .clone()
                .prop_map(|value| Predicate::Not(Box::new(value))),
            prop::collection::vec(inner.clone(), 1..=3).prop_map(Predicate::And),
            prop::collection::vec(inner, 1..=3).prop_map(Predicate::Or),
        ]
    })
}

proptest! {
    #[test]
    fn solver_matches_exhaustive_finite_domain(
        left in predicate_strategy(),
        right in predicate_strategy(),
    ) {
        let overlap = (-2..=2).any(|value| eval(&left, value) && eval(&right, value));
        match analyze_disjointness(&left, &right) {
            Disjointness::Disjoint => prop_assert!(!overlap),
            Disjointness::Overlap(counterexample) => {
                prop_assert!(overlap);
                if let Some(binding) = counterexample.values.first()
                    && let Value::Int(value) = binding.value
                {
                    prop_assert!(eval(&left, value));
                    prop_assert!(eval(&right, value));
                }
            }
            Disjointness::Unknown(reason) => {
                prop_assert!(false, "supported finite fixture returned Unknown: {reason:?}");
            }
        }
    }
}

#[test]
fn numeric_boundaries_prove_disjointness_and_overlap() {
    let below_zero = Predicate::Lt(fact("choice.value"), int(0));
    let zero_or_above = Predicate::Ge(fact("choice.value"), int(0));
    assert_eq!(
        analyze_disjointness(&below_zero, &zero_or_above),
        Disjointness::Disjoint
    );

    let at_most_two = Predicate::Le(fact("choice.value"), int(2));
    let result = analyze_disjointness(&zero_or_above, &at_most_two);
    let Disjointness::Overlap(counterexample) = result else {
        panic!("bounded interval must overlap: {result:?}");
    };
    assert_eq!(counterexample.values.len(), 1);
    assert_eq!(counterexample.values[0].value, Value::Int(0));
}

#[test]
fn de_morgan_and_operand_order_have_identical_witnesses() {
    let outside = Predicate::Or(vec![
        Predicate::Lt(fact("choice.value"), int(0)),
        Predicate::Gt(fact("choice.value"), int(10)),
    ]);
    let de_morgan = Predicate::Not(Box::new(outside));
    let direct = Predicate::And(vec![
        Predicate::Ge(fact("choice.value"), int(0)),
        Predicate::Le(fact("choice.value"), int(10)),
    ]);
    assert_eq!(
        analyze_disjointness(&de_morgan, &Predicate::Const(true)),
        analyze_disjointness(&direct, &Predicate::Const(true))
    );

    let normal = Predicate::Eq(fact("choice.value"), int(7));
    let swapped = Predicate::Eq(int(7), fact("choice.value"));
    assert_eq!(
        analyze_disjointness(&normal, &Predicate::Const(true)),
        analyze_disjointness(&swapped, &Predicate::Const(true))
    );
}

#[test]
fn completion_atoms_use_a_closed_four_state_witness_domain() {
    let success = Predicate::HasCompletion {
        node: "build".parse().unwrap(),
        class: CompletionClass::ExecutedSuccess,
    };
    let failure = Predicate::HasCompletion {
        node: "build".parse().unwrap(),
        class: CompletionClass::Failure,
    };
    assert_eq!(
        analyze_disjointness(&success, &failure),
        Disjointness::Disjoint
    );
    let Disjointness::Overlap(counterexample) = analyze_disjointness(&success, &success) else {
        panic!("identical completion predicates must overlap");
    };
    assert_eq!(counterexample.completions.len(), 1);
    assert_eq!(
        counterexample.completions[0].state,
        CompletionWitness::ExecutedSuccess
    );
}

#[test]
fn unsupported_reference_relations_are_unknown_not_proven() {
    let relation = Predicate::Eq(fact("choice.left"), fact("choice.right"));
    assert_eq!(
        analyze_disjointness(&relation, &Predicate::Const(true)),
        Disjointness::Unknown(DisjointnessUnknown::UnsupportedReferenceRelation)
    );
}

#[test]
fn reflexive_relations_keep_concrete_type_constraints() {
    let reference = fact("choice.value");
    let reflexive = Predicate::Eq(reference.clone(), reference.clone());
    let text = Predicate::Eq(
        reference.clone(),
        ValueExpr::Literal(Value::Text("stable".into())),
    );
    let Disjointness::Overlap(counterexample) = analyze_disjointness(&reflexive, &text) else {
        panic!("reflexive equality must preserve the text witness");
    };
    assert_eq!(counterexample.values[0].value, Value::Text("stable".into()));

    let ordered_reflexive = Predicate::Le(reference.clone(), reference.clone());
    let list = Predicate::Eq(
        reference,
        ValueExpr::Literal(Value::List(vec![Value::Int(1)])),
    );
    assert_eq!(
        analyze_disjointness(&ordered_reflexive, &list),
        Disjointness::Disjoint
    );
}

#[test]
fn normal_form_budget_refuses_before_materialization() {
    let arms = (0..=4_096)
        .map(|value| Predicate::Eq(fact("choice.value"), int(value)))
        .collect();
    let result = analyze_disjointness(&Predicate::Or(arms), &Predicate::Const(true));
    assert!(matches!(
        result,
        Disjointness::Unknown(DisjointnessUnknown::LimitExceeded {
            kind: SolverLimit::NormalFormClauses,
            actual: 4_097,
            limit: 4_096,
        })
    ));
}

#[test]
fn predicate_node_and_depth_budgets_refuse_before_normalization() {
    let too_many_nodes = Predicate::And(vec![Predicate::Const(true); 16_384]);
    assert!(matches!(
        analyze_disjointness(&too_many_nodes, &Predicate::Const(true)),
        Disjointness::Unknown(DisjointnessUnknown::LimitExceeded {
            kind: SolverLimit::PredicateNodes,
            actual: 16_385,
            limit: 16_384,
        })
    ));

    let mut too_deep = Predicate::Const(true);
    for _ in 0..32 {
        too_deep = Predicate::Not(Box::new(too_deep));
    }
    assert!(matches!(
        analyze_disjointness(&too_deep, &Predicate::Const(true)),
        Disjointness::Unknown(DisjointnessUnknown::LimitExceeded {
            kind: SolverLimit::PredicateDepth,
            actual: 33,
            limit: 32,
        })
    ));
}

#[test]
fn aggregate_pair_work_budget_blocks_large_choices() {
    let mut arms = Vec::new();
    let mut nodes = Vec::new();
    let mut terminals = Vec::new();
    for arm_index in 0..128 {
        let target = format!("target{arm_index:03}");
        let predicate = Predicate::Or(
            (0..16)
                .map(|offset| {
                    Predicate::Eq(
                        fact("choice.value"),
                        int(i64::from(arm_index * 100 + offset)),
                    )
                })
                .collect(),
        );
        arms.push(braid_flow_ir::ChoiceArm {
            when: predicate,
            then: target.parse().unwrap(),
        });
        nodes.push(terminal(&target));
        terminals.push(target.parse().unwrap());
    }
    nodes.push(FlowNode {
        key: "choose".parse().unwrap(),
        kind: FlowNodeKind::Choice {
            arms,
            otherwise: "fallback".parse().unwrap(),
        },
        guard: Predicate::Const(true),
        justification: None,
        urgency: UrgencyClass::Required,
    });
    nodes.push(terminal("fallback"));
    terminals.push("fallback".parse().unwrap());
    let flow = FlowSpec::new(
        FlowName::new("choice-work-budget").unwrap(),
        vec![],
        nodes,
        vec![],
        terminals,
        FlowBounds::default(),
    )
    .unwrap();

    assert!(matches!(
        verify(&flow.canonical_bytes()),
        Err(FlowVerifyError::ChoiceDisjointnessUnknown {
            reason: DisjointnessUnknown::LimitExceeded {
                kind: SolverLimit::Work,
                ..
            },
            ..
        })
    ));
}

fn terminal(name: &str) -> FlowNode {
    FlowNode {
        key: name.parse().unwrap(),
        kind: FlowNodeKind::Terminal {
            outcome: TerminalOutcome::Success,
        },
        guard: Predicate::Const(true),
        justification: None,
        urgency: UrgencyClass::Required,
    }
}

fn choice_flow(left: Predicate, right: Predicate) -> FlowSpec {
    FlowSpec::new(
        FlowName::new("choice-proof").unwrap(),
        vec![],
        vec![
            FlowNode {
                key: "choose".parse().unwrap(),
                kind: FlowNodeKind::Choice {
                    arms: vec![
                        braid_flow_ir::ChoiceArm {
                            when: left,
                            then: "left".parse().unwrap(),
                        },
                        braid_flow_ir::ChoiceArm {
                            when: right,
                            then: "right".parse().unwrap(),
                        },
                    ],
                    otherwise: "fallback".parse().unwrap(),
                },
                guard: Predicate::Const(true),
                justification: None,
                urgency: UrgencyClass::Required,
            },
            terminal("left"),
            terminal("right"),
            terminal("fallback"),
        ],
        vec![],
        vec![
            "left".parse().unwrap(),
            "right".parse().unwrap(),
            "fallback".parse().unwrap(),
        ],
        FlowBounds::default(),
    )
    .unwrap()
}

#[test]
fn mutation_removing_pair_checks_cannot_hide_identical_predicates() {
    let predicate = Predicate::Eq(fact("choice.value"), int(1));
    let flow = choice_flow(predicate.clone(), predicate);
    let error = match verify(&flow.canonical_bytes()) {
        Ok(_) => panic!("overlapping predicates must not admit"),
        Err(error) => error,
    };
    let FlowVerifyError::ChoiceNotDisjoint { overlap, .. } = error else {
        panic!("expected overlap refusal, got {error:?}");
    };
    assert_eq!(overlap.choice, "choose");
    assert_ne!(overlap.left_target, overlap.right_target);
    assert_eq!(overlap.counterexample.values.len(), 1);
}

#[test]
fn demonstrably_disjoint_choice_is_admitted() {
    let flow = choice_flow(
        Predicate::Lt(fact("choice.value"), int(0)),
        Predicate::Ge(fact("choice.value"), int(0)),
    );
    verify(&flow.canonical_bytes()).unwrap();
}

#[test]
fn mutation_treating_unknown_as_disjoint_cannot_admit() {
    let flow = choice_flow(
        Predicate::Eq(fact("choice.left"), fact("choice.right")),
        Predicate::Const(true),
    );
    assert!(matches!(
        verify(&flow.canonical_bytes()),
        Err(FlowVerifyError::ChoiceDisjointnessUnknown {
            reason: DisjointnessUnknown::UnsupportedReferenceRelation,
            ..
        })
    ));
}
