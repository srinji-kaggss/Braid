use braid_flow_ir::{
    CompletionClass, FactRef, FlowBounds, FlowEdge, FlowInput, FlowName, FlowNode, FlowNodeKind,
    FlowSpec, InputKey, InputPort, JustificationDecl, NodeKey, PortKey, Predicate, RelationRef,
    TerminalOutcome, UrgencyClass, ValueExpr, ValueSource,
};
use braid_flow_plan::{
    CompletionKind, FlowSnapshot, PlanError, PlanningContext, ReverseDeps, plan,
};
use braid_ir::{Cid, TypeTag, Value};
use std::collections::BTreeMap;

fn key(v: &str) -> NodeKey {
    NodeKey::new(v).unwrap()
}
fn invocation(name: &str, byte: u8) -> FlowNode {
    FlowNode {
        key: key(name),
        kind: FlowNodeKind::InvokeCapsule {
            capsule: Cid([byte; 32]),
        },
        guard: Predicate::Const(true),
        justification: Some(justification(name)),
        urgency: UrgencyClass::Required,
    }
}
fn justification(name: &str) -> JustificationDecl {
    JustificationDecl {
        needed_when: Predicate::Eq(
            ValueExpr::SnapshotFact(FactRef::new(&format!("need.{name}")).unwrap()),
            ValueExpr::Literal(Value::Bool(true)),
        ),
        satisfied_when: Predicate::Eq(
            ValueExpr::SnapshotFact(FactRef::new(&format!("done.{name}")).unwrap()),
            ValueExpr::Literal(Value::Bool(true)),
        ),
        guarantees: vec![RelationRef::new(&format!("guarantee.{name}")).unwrap()],
        preserves: vec![],
        cost_order: None,
    }
}
fn terminal(name: &str) -> FlowNode {
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

fn choice_flow() -> FlowSpec {
    FlowSpec::new(
        FlowName::new("choice-plan").unwrap(),
        vec![],
        vec![
            FlowNode {
                key: key("choose"),
                kind: FlowNodeKind::Choice {
                    arms: vec![
                        braid_flow_ir::ChoiceArm {
                            when: Predicate::Eq(
                                ValueExpr::SnapshotFact(FactRef::new("choice.value").unwrap()),
                                ValueExpr::Literal(Value::Int(1)),
                            ),
                            then: key("one"),
                        },
                        braid_flow_ir::ChoiceArm {
                            when: Predicate::Eq(
                                ValueExpr::SnapshotFact(FactRef::new("choice.value").unwrap()),
                                ValueExpr::Literal(Value::Int(2)),
                            ),
                            then: key("two"),
                        },
                    ],
                    otherwise: key("other"),
                },
                guard: Predicate::Const(true),
                justification: None,
                urgency: UrgencyClass::Required,
            },
            terminal("one"),
            terminal("two"),
            terminal("other"),
        ],
        vec![],
        vec![key("one"), key("two"), key("other")],
        FlowBounds::default(),
    )
    .unwrap()
}
fn after(from: &str, to: &str) -> FlowEdge {
    FlowEdge::After {
        from: key(from),
        to: key(to),
        on: vec![
            CompletionClass::SatisfiedWithoutExecution,
            CompletionClass::ExecutedSuccess,
        ],
    }
}
fn root_edge(root: &str, node: &str) -> FlowEdge {
    FlowEdge::Data {
        from: ValueSource::Root(InputKey::new(root).unwrap()),
        to: InputPort {
            node: key(node),
            port: PortKey::new("input").unwrap(),
        },
        value_type: TypeTag::Bool,
    }
}

fn simple_dag() -> FlowSpec {
    FlowSpec::new(
        FlowName::new("braid-ci").unwrap(),
        vec![
            FlowInput {
                key: InputKey::new("source.a").unwrap(),
                value_type: TypeTag::Bool,
            },
            FlowInput {
                key: InputKey::new("source.b").unwrap(),
                value_type: TypeTag::Bool,
            },
        ],
        vec![
            invocation("scope", 1),
            invocation("build", 2),
            terminal("accepted"),
        ],
        vec![
            root_edge("source.a", "scope"),
            root_edge("source.b", "build"),
            after("scope", "build"),
            after("build", "accepted"),
        ],
        vec![key("accepted")],
        FlowBounds::default(),
    )
    .unwrap()
}

// helpers: snapshot where need.X=true / done.X flags drive §9.2
fn snap(pairs: Vec<(&str, bool)>) -> FlowSnapshot {
    let map: BTreeMap<String, Value> = pairs
        .into_iter()
        .map(|(k, v)| (k.to_string(), Value::Bool(v)))
        .collect();
    FlowSnapshot::new(map)
}

#[test]
fn satiation_precedes_action_under_determininistic_inputs() {
    let flow = simple_dag();
    // build is already satisfied (done.build=true) -> satiated, no ready node
    let snapshot = snap(vec![
        ("need.scope", true),
        ("done.scope", false),
        ("need.build", true),
        ("done.build", true),
    ]);
    let mut comp = BTreeMap::new();
    comp.insert("scope".into(), CompletionKind::ExecutedSuccess);
    let ctx = PlanningContext::default();
    let out = plan(&flow, &snapshot, &comp, &ctx).unwrap();
    assert!(out.satiated.iter().any(|s| s.node == "build"));
    // Satiated nodes must never be dispatched — next_step is either None or a
    // different node.
    if let Some(s) = &out.next_step {
        assert_ne!(s.node, "build", "satiated node dispatched {s:?}");
    }
}

#[test]
fn ready_frontier_selects_only_with_proven_needed_and_preds_satisfied() {
    let flow = simple_dag();
    // scope is ready (no preds), build is not yet ready (scope pending)
    let snapshot = snap(vec![
        ("need.scope", true),
        ("done.scope", false),
        ("need.build", true),
        ("done.build", false),
    ]);
    let comp = BTreeMap::new();
    let ctx = PlanningContext::default();
    let out = plan(&flow, &snapshot, &comp, &ctx).unwrap();
    assert_eq!(out.next_step.as_ref().unwrap().node, "scope");
}

#[test]
fn unknown_fails_closed() {
    let flow = simple_dag();
    // need.build references unknown fact -> Unknown refusal
    let snapshot = snap(vec![("need.scope", true), ("done.scope", false)]);
    let mut comp = BTreeMap::new();
    comp.insert("scope".into(), CompletionKind::ExecutedSuccess);
    let ctx = PlanningContext::default();
    let err = plan(&flow, &snapshot, &comp, &ctx).unwrap_err();
    assert!(matches!(err, PlanError::UnknownProof { .. }), "got {err:?}");
}

#[test]
fn insertion_order_cannot_alter_plan_choice() {
    // Build two flows with the same semantic content but reversed declaration order;
    // the plan must be byte-identical (INV-FLOW-021/023).
    let roots_a = vec![
        FlowInput {
            key: InputKey::new("source.a").unwrap(),
            value_type: TypeTag::Bool,
        },
        FlowInput {
            key: InputKey::new("source.b").unwrap(),
            value_type: TypeTag::Bool,
        },
    ];
    let nodes_a = vec![
        invocation("scope", 1),
        invocation("build", 2),
        terminal("accepted"),
    ];
    let edges_a = vec![
        root_edge("source.a", "scope"),
        root_edge("source.b", "build"),
        after("scope", "build"),
        after("build", "accepted"),
    ];
    let nodes_b = vec![
        invocation("build", 2),
        invocation("scope", 1),
        terminal("accepted"),
    ];
    let edges_b = vec![
        root_edge("source.b", "build"),
        root_edge("source.a", "scope"),
        after("build", "accepted"),
        after("scope", "build"),
    ];
    let fa = FlowSpec::new(
        FlowName::new("braid-ci").unwrap(),
        roots_a,
        nodes_a,
        edges_a,
        vec![key("accepted")],
        FlowBounds::default(),
    )
    .unwrap();
    let fb = FlowSpec::new(
        FlowName::new("braid-ci").unwrap(),
        vec![
            FlowInput {
                key: InputKey::new("source.b").unwrap(),
                value_type: TypeTag::Bool,
            },
            FlowInput {
                key: InputKey::new("source.a").unwrap(),
                value_type: TypeTag::Bool,
            },
        ],
        nodes_b,
        edges_b,
        vec![key("accepted")],
        FlowBounds::default(),
    )
    .unwrap();
    assert_eq!(
        fa.cid(),
        fb.cid(),
        "flow CIDs must match regardless of declaration order"
    );
    let snapshot = snap(vec![
        ("need.scope", true),
        ("done.scope", false),
        ("need.build", true),
        ("done.build", false),
    ]);
    let comp = BTreeMap::new();
    let ctx = PlanningContext::default();
    let pa = plan(&fa, &snapshot, &comp, &ctx).unwrap();
    let pb = plan(&fb, &snapshot, &comp, &ctx).unwrap();
    assert_eq!(pa.plan_cid, pb.plan_cid, "plan CIDs must match");
    assert_eq!(pa.next_step, pb.next_step);
}

#[test]
fn plan_cid_sensitive_to_snapshot() {
    let flow = simple_dag();
    let sa = snap(vec![
        ("need.scope", true),
        ("done.scope", false),
        ("need.build", true),
        ("done.build", false),
    ]);
    let sb = snap(vec![
        ("need.scope", true),
        ("done.scope", true),
        ("need.build", true),
        ("done.build", false),
    ]);
    let ctx = PlanningContext::default();
    let pa = plan(&flow, &sa, &BTreeMap::new(), &ctx).unwrap();
    let pb = plan(&flow, &sb, &BTreeMap::new(), &ctx).unwrap();
    assert_ne!(
        pa.plan_cid, pb.plan_cid,
        "INV-FLOW-008/023: different snapshot -> different plan CID"
    );
    assert_ne!(sa.cid(), sb.cid());
}

#[test]
fn urgency_ranking_is_deterministic() {
    // Two ready siblings at the same predecessor level — the SafetyRecovery one
    // must be chosen first regardless of key order.
    let scope = invocation("scope", 1);
    let mut build = invocation("build", 2);
    build.urgency = UrgencyClass::SafetyRecovery;
    let mut other = invocation("other", 3);
    other.urgency = UrgencyClass::Required;
    let flow = FlowSpec::new(
        FlowName::new("rank-test").unwrap(),
        vec![FlowInput {
            key: InputKey::new("source.a").unwrap(),
            value_type: TypeTag::Bool,
        }],
        vec![scope, build, other, terminal("accepted")],
        vec![
            root_edge("source.a", "scope"),
            root_edge("source.a", "build"),
            root_edge("source.a", "other"),
            after("scope", "accepted"),
            after("build", "accepted"),
            after("other", "accepted"),
        ],
        vec![key("accepted")],
        FlowBounds::default(),
    )
    .unwrap();
    let snapshot = snap(vec![
        ("need.scope", true),
        ("done.scope", false),
        ("need.build", true),
        ("done.build", false),
        ("need.other", true),
        ("done.other", false),
    ]);
    let ctx = PlanningContext::default();
    let out = plan(&flow, &snapshot, &BTreeMap::new(), &ctx).unwrap();
    assert_eq!(out.next_step.as_ref().unwrap().node, "build");
}

#[test]
fn reverse_deps_are_stable_and_cover_edges() {
    let flow = simple_dag();
    let rev = ReverseDeps::from_flow(&flow);
    // build depends on scope via After edge -> scope is a dependency of build
    // ReverseDeps tracks dependents: scope -> [build], build -> [accepted]
    let _deps_of_build = rev.direct_dependents("build");
    // build's direct dependents include accepted via edge;
    // but transitive from scope should reach accepted
    let trans = rev.transitive_dependents("scope");
    assert!(trans.contains("build"));
}

#[test]
fn stale_snapshot_must_not_satiate_without_fresh_evidence() {
    // The plan bound a snapshot where done.build=true (satiated). Reusing that
    // derived plan under a newer snapshot where done.build=false would be a
    // stale-proof consume — Plan CID divergence is the refusal.
    let flow = simple_dag();
    let fresh_snapshot = snap(vec![
        ("need.build", true),
        ("done.build", false),
        ("need.scope", true),
        ("done.scope", true),
    ]);
    let stale_snapshot = snap(vec![
        ("need.build", true),
        ("done.build", true),
        ("need.scope", true),
        ("done.scope", true),
    ]);
    let mut comp = BTreeMap::new();
    comp.insert("scope".into(), CompletionKind::ExecutedSuccess);
    let ctx = PlanningContext::default();
    let stale = plan(&flow, &stale_snapshot, &comp, &ctx).unwrap();
    let fresh = plan(&flow, &fresh_snapshot, &comp, &ctx).unwrap();
    assert!(stale.satiated.iter().any(|s| s.node == "build"));
    assert!(fresh.satiated.iter().all(|s| s.node != "build"));
    assert_ne!(stale.plan_cid, fresh.plan_cid);
    // cannot reuse stale evidence under fresh snapshot — the CIDs already diverge
    assert_ne!(stale_snapshot.cid(), fresh_snapshot.cid());
}

#[test]
fn choice_selects_one_snapshot_bound_target_or_otherwise() {
    let flow = choice_flow();
    let context = PlanningContext::default();
    for (value, expected) in [(1, "one"), (2, "two"), (3, "other")] {
        let snapshot = FlowSnapshot::new(
            [("choice.value".into(), Value::Int(value))]
                .into_iter()
                .collect(),
        );
        let output = plan(&flow, &snapshot, &BTreeMap::new(), &context).unwrap();
        let step = output.next_step.expect("choice is the ready root");
        assert_eq!(step.kind, braid_flow_plan::PlanStepKind::Choice);
        assert_eq!(step.choice_target.as_deref(), Some(expected));
    }
}

#[test]
fn unknown_choice_arm_defers_instead_of_falling_through() {
    let flow = choice_flow();
    let error = plan(
        &flow,
        &FlowSnapshot::new(BTreeMap::new()),
        &BTreeMap::new(),
        &PlanningContext::default(),
    )
    .unwrap_err();
    assert!(matches!(error, PlanError::UnknownProof { .. }));
    assert!(error.to_string().contains("Choice arm"));
}

#[test]
fn mutation_removing_snapshot_binding_changes_choice_plan_cid() {
    let flow = choice_flow();
    let context = PlanningContext::default();
    let first = FlowSnapshot::new(
        [
            ("choice.value".into(), Value::Int(1)),
            ("irrelevant.audit".into(), Value::Bool(false)),
        ]
        .into_iter()
        .collect(),
    );
    let second = FlowSnapshot::new(
        [
            ("choice.value".into(), Value::Int(1)),
            ("irrelevant.audit".into(), Value::Bool(true)),
        ]
        .into_iter()
        .collect(),
    );
    let first_plan = plan(&flow, &first, &BTreeMap::new(), &context).unwrap();
    let second_plan = plan(&flow, &second, &BTreeMap::new(), &context).unwrap();
    assert_eq!(
        first_plan.next_step.as_ref().unwrap().choice_target,
        second_plan.next_step.as_ref().unwrap().choice_target
    );
    assert_ne!(first.cid(), second.cid());
    assert_ne!(first_plan.plan_cid, second_plan.plan_cid);
}

#[test]
fn stale_planner_versions_are_rejected_instead_of_aliasing_plan_identity() {
    let flow = simple_dag();
    let snapshot = snap(vec![
        ("need.scope", true),
        ("done.scope", false),
        ("need.build", true),
        ("done.build", false),
    ]);
    let context = PlanningContext {
        planner_version: 0,
        ..PlanningContext::default()
    };
    assert!(matches!(
        plan(&flow, &snapshot, &BTreeMap::new(), &context),
        Err(PlanError::UnsupportedPlannerVersion {
            found: 0,
            expected: 1,
        })
    ));
}
