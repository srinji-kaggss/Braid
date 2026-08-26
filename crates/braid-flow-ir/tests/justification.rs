mod common;

use braid_flow_ir::{
    ChoiceArm, CompletionClass, FlowBounds, FlowEdge, FlowError, FlowName, FlowNode, FlowNodeKind,
    FlowSpec, Predicate, RelationRef, UrgencyClass,
};

fn one_invocation(node: braid_flow_ir::FlowNode) -> Result<FlowSpec, FlowError> {
    FlowSpec::new(
        FlowName::new("justification-test").unwrap(),
        vec![braid_flow_ir::FlowInput {
            key: braid_flow_ir::InputKey::new("input").unwrap(),
            value_type: braid_ir::TypeTag::Bool,
        }],
        vec![node, common::terminal("done")],
        vec![FlowEdge::After {
            from: common::key("work"),
            to: common::key("done"),
            on: vec![CompletionClass::ExecutedSuccess],
        }],
        vec![common::key("done")],
        FlowBounds::default(),
    )
}

#[test]
fn p1_does_not_guess_materiality_or_inherited_justification() {
    let mut node = common::invocation("work", 1);
    node.justification = None;

    assert!(one_invocation(node).is_ok());
}

#[test]
fn justification_is_identity_bearing() {
    let first = one_invocation(common::invocation("work", 1)).unwrap();
    let mut changed_node = common::invocation("work", 1);
    changed_node.justification.as_mut().unwrap().guarantees =
        vec![RelationRef::new("guarantee.different").unwrap()];
    let changed = one_invocation(changed_node).unwrap();

    assert_ne!(first.cid(), changed.cid());
}

#[test]
fn p1_preserves_vacuity_for_p2_to_decide_with_capsule_context() {
    let mut node = common::invocation("work", 1);
    node.justification.as_mut().unwrap().satisfied_when = Predicate::Const(true);

    assert!(one_invocation(node).is_ok());
}

#[test]
fn p1_preserves_incomplete_declarations_for_fail_closed_p2_admission() {
    let mut node = common::invocation("work", 1);
    node.justification.as_mut().unwrap().guarantees.clear();

    assert!(one_invocation(node).is_ok());
}

#[test]
fn cycles_are_rejected_even_when_both_nodes_are_individually_valid() {
    let nodes = vec![common::invocation("a", 1), common::invocation("b", 2)];
    let edges = vec![common::after("a", "b"), common::after("b", "a")];
    let error = FlowSpec::new(
        FlowName::new("cycle").unwrap(),
        vec![braid_flow_ir::FlowInput {
            key: braid_flow_ir::InputKey::new("input").unwrap(),
            value_type: braid_ir::TypeTag::Bool,
        }],
        nodes,
        edges,
        vec![common::key("b")],
        FlowBounds::default(),
    )
    .unwrap_err();

    assert!(matches!(error, FlowError::Cycle { .. }));
}

#[test]
fn a_cycle_hidden_in_choice_targets_is_still_a_cycle() {
    let choice = |name: &str, target: &str, otherwise: &str| FlowNode {
        key: common::key(name),
        kind: FlowNodeKind::Choice {
            arms: vec![ChoiceArm {
                when: Predicate::Const(false),
                then: common::key(target),
            }],
            otherwise: common::key(otherwise),
        },
        guard: Predicate::Const(true),
        justification: None,
        urgency: UrgencyClass::Required,
    };
    let error = FlowSpec::new(
        FlowName::new("choice-cycle").unwrap(),
        vec![braid_flow_ir::FlowInput {
            key: braid_flow_ir::InputKey::new("input").unwrap(),
            value_type: braid_ir::TypeTag::Bool,
        }],
        vec![
            choice("a", "b", "done"),
            choice("b", "a", "done"),
            common::terminal("done"),
        ],
        vec![],
        vec![common::key("done")],
        FlowBounds::default(),
    )
    .unwrap_err();

    assert!(matches!(error, FlowError::Cycle { .. }));
}
