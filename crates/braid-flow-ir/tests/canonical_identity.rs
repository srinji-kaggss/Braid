mod common;

use braid_flow_ir::FlowNodeKind;
use std::fmt::Write;

fn to_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").unwrap();
    }
    output
}

#[test]
fn declaration_order_is_not_identity() {
    let forward = common::flow_with_orders(false);
    let reverse = common::flow_with_orders(true);

    assert_eq!(forward.canonical_bytes(), reverse.canonical_bytes());
    assert_eq!(forward.cid(), reverse.cid());
}

#[test]
fn one_semantic_bit_changes_flow_identity() {
    let original = common::flow_with_orders(false);
    let mut nodes = original.nodes().to_vec();
    let scope = nodes
        .iter_mut()
        .find(|node| node.key.as_str() == "scope")
        .unwrap();
    let FlowNodeKind::InvokeCapsule { capsule } = &scope.kind else {
        panic!("scope is an invocation")
    };
    let mut changed_capsule = capsule.0;
    changed_capsule[0] ^= 1;
    scope.kind = FlowNodeKind::InvokeCapsule {
        capsule: braid_ir::Cid(changed_capsule),
    };
    let changed = braid_flow_ir::FlowSpec::new(
        original.name().clone(),
        original.roots().to_vec(),
        nodes,
        original.edges().to_vec(),
        original.terminals().to_vec(),
        original.bounds(),
    )
    .unwrap();

    assert_ne!(original.cid(), changed.cid());
}

#[test]
fn flow_v0_known_answer_is_pinned() {
    let flow = common::flow_with_orders(false);
    let vector = include_str!("../../../spec/braid/vectors/frontier-flow/flow_v0.kat");
    let expected_bytes = vector
        .lines()
        .find_map(|line| line.strip_prefix("canonical_hex="))
        .expect("flow_v0.kat carries canonical bytes");
    let expected = vector
        .lines()
        .find_map(|line| line.strip_prefix("cid="))
        .expect("flow_v0.kat carries cid");
    assert_eq!(to_hex(&flow.canonical_bytes()), expected_bytes);
    assert_eq!(flow.cid().get().to_hex(), expected);
}
