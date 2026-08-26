//! Fail-closed admission stages.
use crate::decode::decode_flow_verify;
use crate::error::{FlowVerifyError, VerifyResult};
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::ToString;
use alloc::vec::Vec;
use braid_flow_ir::{FlowNodeKind, FlowSpec};

pub struct AdmittedFlow {
    pub flow: FlowSpec,
    pub flow_cid: braid_ir::Cid,
}

pub fn verify(bytes: &[u8]) -> VerifyResult<AdmittedFlow> {
    let flow = decode_flow_verify(bytes)?;
    check_acyclic(&flow)?;
    check_reachability(&flow)?;
    check_choice(&flow)?;
    check_join(&flow)?;
    check_terminals(&flow)?;
    check_justification(&flow)?;
    let flow_cid = braid_ir::Cid::compute(braid_flow_ir::FLOW_DOMAIN, bytes);
    Ok(AdmittedFlow { flow, flow_cid })
}

fn check_acyclic(flow: &FlowSpec) -> VerifyResult<()> {
    let n = flow.nodes().len();
    let idx: BTreeMap<String, usize> = flow
        .nodes()
        .iter()
        .enumerate()
        .map(|(i, nd)| (nd.key.to_string(), i))
        .collect();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for e in flow.edges() {
        match e {
            braid_flow_ir::FlowEdge::After { from, to, .. } => {
                if let (Some(&u), Some(&v)) = (idx.get(&from.to_string()), idx.get(&to.to_string()))
                {
                    adj[u].push(v);
                }
            }
            braid_flow_ir::FlowEdge::Data { from, to, .. } => {
                let src = match from {
                    braid_flow_ir::ValueSource::Node(o) => o.node.to_string(),
                    _ => continue,
                };
                let dst = to.node.to_string();
                if let (Some(&u), Some(&v)) = (idx.get(&src), idx.get(&dst)) {
                    adj[u].push(v);
                }
            }
        }
    }
    let mut state = vec![0u8; n];
    fn dfs(u: usize, adj: &[Vec<usize>], state: &mut [u8]) -> bool {
        state[u] = 1;
        for &v in &adj[u] {
            if state[v] == 1 {
                return true;
            }
            if state[v] == 0 && dfs(v, adj, state) {
                return true;
            }
        }
        state[u] = 2;
        false
    }
    for i in 0..n {
        if state[i] == 0 && dfs(i, &adj, &mut state) {
            return Err(FlowVerifyError::Cycle {
                invariant: "INV-FLOW-003",
            });
        }
    }
    Ok(())
}
fn check_reachability(flow: &FlowSpec) -> VerifyResult<()> {
    // Every node must be on some path from synthetic start (roots/data predecessors) to a terminal.
    // Minimal v0: every node has at least one incoming or is referenced; and every node can reach a terminal via after/data edges.
    let idx: BTreeMap<String, usize> = flow
        .nodes()
        .iter()
        .enumerate()
        .map(|(i, nd)| (nd.key.to_string(), i))
        .collect();
    let n = flow.nodes().len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut rev: Vec<Vec<usize>> = vec![Vec::new(); n];
    for e in flow.edges() {
        let (s, d) = match e {
            braid_flow_ir::FlowEdge::After { from, to, .. } => (from.to_string(), to.to_string()),
            braid_flow_ir::FlowEdge::Data { from, to, .. } => {
                let s = match from {
                    braid_flow_ir::ValueSource::Node(o) => o.node.to_string(),
                    _ => continue,
                };
                (s, to.node.to_string())
            }
        };
        if let (Some(&u), Some(&v)) = (idx.get(&s), idx.get(&d)) {
            adj[u].push(v);
            rev[v].push(u);
        }
    }
    // Include Choice arms as edges for reachability
    for (i, nd) in flow.nodes().iter().enumerate() {
        if let FlowNodeKind::Choice { arms, otherwise } = &nd.kind {
            for a in arms {
                if let Some(&v) = idx.get(&a.then.to_string()) {
                    adj[i].push(v);
                    rev[v].push(i);
                }
            }
            if let Some(&v) = idx.get(&otherwise.to_string()) {
                adj[i].push(v);
                rev[v].push(i);
            }
        }
    }
    // Can-reach-terminal (reverse BFS from terminals)
    let mut can_reach_term = vec![false; n];
    let mut q: Vec<usize> = flow
        .terminals()
        .iter()
        .filter_map(|k| idx.get(&k.to_string()).copied())
        .collect();
    for &t in &q {
        can_reach_term[t] = true;
    }
    let mut qi = 0;
    while qi < q.len() {
        let u = q[qi];
        qi += 1;
        for &p in &rev[u] {
            if !can_reach_term[p] {
                can_reach_term[p] = true;
                q.push(p);
            }
        }
    }
    for can in &can_reach_term {
        if !*can {
            return Err(FlowVerifyError::TerminalSoundness {
                invariant: "INV-FLOW-014",
            });
        }
    }
    Ok(())
}
fn check_choice(flow: &FlowSpec) -> VerifyResult<()> {
    for nd in flow.nodes() {
        if let FlowNodeKind::Choice { arms, otherwise: _ } = &nd.kind {
            if arms.is_empty() {
                return Err(FlowVerifyError::ChoiceNotTotal {
                    invariant: "INV-FLOW-011",
                });
            }
            // Bounded disjointness: syntactic check — distinct `then` targets.
            let mut seen = BTreeSet::new();
            for a in arms {
                if !seen.insert(a.then.to_string()) {
                    return Err(FlowVerifyError::ChoiceNotDisjoint {
                        invariant: "INV-FLOW-011",
                    });
                }
            }
        }
    }
    Ok(())
}
fn check_join(flow: &FlowSpec) -> VerifyResult<()> {
    for nd in flow.nodes() {
        if matches!(nd.kind, FlowNodeKind::JoinAll) {
            let preds = flow
                .edges()
                .iter()
                .filter(|e| match e {
                    braid_flow_ir::FlowEdge::After { to, .. } => {
                        to.to_string() == nd.key.to_string()
                    }
                    braid_flow_ir::FlowEdge::Data { to, .. } => {
                        to.node.to_string() == nd.key.to_string()
                    }
                })
                .count();
            if preds == 0 {
                return Err(FlowVerifyError::JoinCardinality {
                    invariant: "INV-FLOW-013",
                });
            }
        }
    }
    Ok(())
}
fn check_terminals(flow: &FlowSpec) -> VerifyResult<()> {
    let keys: BTreeSet<String> = flow.nodes().iter().map(|n| n.key.to_string()).collect();
    for t in flow.terminals() {
        if !keys.contains(&t.to_string()) {
            return Err(FlowVerifyError::Unresolved {
                field: "terminal",
                key: t.to_string(),
                invariant: "INV-FLOW-014",
            });
        }
    }
    if flow.terminals().is_empty() {
        return Err(FlowVerifyError::EmptyCollection {
            field: "terminals",
            invariant: "INV-FLOW-014",
        });
    }
    Ok(())
}
fn check_justification(flow: &FlowSpec) -> VerifyResult<()> {
    for nd in flow.nodes() {
        if matches!(nd.kind, FlowNodeKind::InvokeCapsule { .. }) && nd.justification.is_none() {
            return Err(FlowVerifyError::JustificationIncomplete {
                field: "justification",
                invariant: "INV-FLOW-006",
            });
        }
    }
    Ok(())
}
