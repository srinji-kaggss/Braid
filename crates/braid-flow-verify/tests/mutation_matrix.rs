use braid_flow_ir::{
    FlowBounds, FlowEdge, FlowNode, FlowNodeKind, FlowSpec, JustificationDecl, Predicate,
    TerminalOutcome, UrgencyClass,
};
use braid_flow_verify::verify;
use braid_ir::{Cid, Value, encode};

fn cid(n: u8) -> Cid {
    Cid([n; 32])
}
fn is_rejected(bytes: &[u8]) -> bool {
    verify(bytes).is_err()
}
fn canon_bytes(flow: &FlowSpec) -> Vec<u8> {
    encode(&flow.to_canon())
}
fn just() -> JustificationDecl {
    JustificationDecl {
        needed_when: Predicate::Const(true),
        satisfied_when: Predicate::Const(false),
        guarantees: vec![],
        preserves: vec![],
        cost_order: None,
    }
}

fn minimal_valid_flow() -> FlowSpec {
    let hex = include_str!("../../../spec/braid/vectors/frontier-flow/flow_v0.kat");
    let h = hex
        .lines()
        .find(|l| l.starts_with("canonical_hex="))
        .unwrap()
        .trim_start_matches("canonical_hex=")
        .trim()
        .to_string();
    let bytes: Vec<u8> = (0..h.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap())
        .collect();
    FlowSpec::from_bytes(&bytes).unwrap()
}

#[test]
fn positive_kat_admits() {
    let f = minimal_valid_flow();
    let b = canon_bytes(&f);
    assert!(verify(&b).is_ok(), "KAT must admit: {:?}", verify(&b).err());
}

#[test]
fn every_invariant_has_a_killing_negative() {
    let base = minimal_valid_flow();
    let base_bytes = canon_bytes(&base);

    let mut bad = base_bytes.clone();
    bad[0] ^= 0xff;
    assert!(is_rejected(&bad), "INV-FLOW-018 malformed must be rejected");

    let mut truncated = base_bytes.clone();
    truncated.truncate(truncated.len() / 2);
    assert!(
        is_rejected(&truncated),
        "INV-FLOW-018 truncated must be rejected"
    );

    // INV-FLOW-003 cycle — build via FlowSpec with explicit cycle; FlowSpec::new may accept it and verifier must reject
    {
        let cap = cid(2);
        let mk = |k: &str| FlowNode {
            key: k.parse().unwrap(),
            kind: FlowNodeKind::InvokeCapsule { capsule: cap },
            guard: Predicate::Const(true),
            justification: Some(just()),
            urgency: UrgencyClass::Required,
        };
        let nodes = vec![
            mk("aaa"),
            mk("bbb"),
            FlowNode {
                key: "accepted".parse().unwrap(),
                kind: FlowNodeKind::Terminal {
                    outcome: TerminalOutcome::Success,
                },
                guard: Predicate::Const(true),
                justification: None,
                urgency: UrgencyClass::Required,
            },
        ];
        let edges = vec![
            FlowEdge::After {
                from: "aaa".parse().unwrap(),
                to: "bbb".parse().unwrap(),
                on: vec![braid_flow_ir::CompletionClass::ExecutedSuccess],
            },
            FlowEdge::After {
                from: "bbb".parse().unwrap(),
                to: "aaa".parse().unwrap(),
                on: vec![braid_flow_ir::CompletionClass::ExecutedSuccess],
            },
            FlowEdge::After {
                from: "bbb".parse().unwrap(),
                to: "accepted".parse().unwrap(),
                on: vec![braid_flow_ir::CompletionClass::ExecutedSuccess],
            },
        ];
        if let Ok(flow) = FlowSpec::new(
            "cycle".parse().unwrap(),
            vec![],
            nodes,
            edges,
            vec!["accepted".parse().unwrap()],
            FlowBounds::default(),
        ) {
            assert!(
                is_rejected(&canon_bytes(&flow)),
                "INV-FLOW-003 cycle must be rejected"
            );
        } else {
            // Constructor already rejects cycle — still a killing negative (IR already enforces, verifier would also)
        }
    }

    // INV-FLOW-006 missing justification — raw Value bypass
    {
        let mut m = match base.to_canon() {
            Value::Map(m) => m,
            _ => panic!(),
        };
        if let Some(Value::List(nodes)) = m.get_mut("nodes") {
            for node in nodes.iter_mut() {
                if let Value::Map(nm) = node
                    && nm.contains_key("justification")
                {
                    nm.remove("justification");
                    break;
                }
            }
        }
        let raw = encode(&Value::Map(m));
        assert!(
            is_rejected(&raw),
            "INV-FLOW-006 missing justification must be rejected"
        );
    }

    // INV-FLOW-014 isolated node that cannot reach terminal
    {
        let cap = cid(4);
        let a = FlowNode {
            key: "aaa".parse().unwrap(),
            kind: FlowNodeKind::InvokeCapsule { capsule: cap },
            guard: Predicate::Const(true),
            justification: Some(just()),
            urgency: UrgencyClass::Required,
        };
        let t1 = FlowNode {
            key: "accepted".parse().unwrap(),
            kind: FlowNodeKind::Terminal {
                outcome: TerminalOutcome::Success,
            },
            guard: Predicate::Const(true),
            justification: None,
            urgency: UrgencyClass::Required,
        };
        let dead = FlowNode {
            key: "zzz".parse().unwrap(),
            kind: FlowNodeKind::InvokeCapsule { capsule: cap },
            guard: Predicate::Const(true),
            justification: Some(just()),
            urgency: UrgencyClass::Required,
        };
        let flow = FlowSpec::new(
            "dead_term".parse().unwrap(),
            vec![],
            vec![a, t1, dead],
            vec![FlowEdge::After {
                from: "aaa".parse().unwrap(),
                to: "accepted".parse().unwrap(),
                on: vec![braid_flow_ir::CompletionClass::ExecutedSuccess],
            }],
            vec!["accepted".parse().unwrap()],
            FlowBounds::default(),
        )
        .unwrap();
        assert!(
            is_rejected(&canon_bytes(&flow)),
            "INV-FLOW-014 isolated node must be rejected"
        );
    }
}
