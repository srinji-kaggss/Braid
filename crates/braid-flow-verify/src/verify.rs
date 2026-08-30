//! Fail-closed admission stages.

use crate::decode::decode_flow_verify;
use crate::disjoint::{
    DISJOINTNESS_MAX_WORK, Disjointness, DisjointnessUnknown, analyze_preflighted, preflight_pair,
};
use crate::error::{ChoiceOverlap, FlowVerifyError, VerifyResult};
use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use braid_flow_ir::{FlowEdge, FlowNodeKind, FlowSpec, ValueSource};

type NodeId = u32;
type Edge = (NodeId, NodeId);

/// Independently admitted Flow plus its exact content identity.
pub struct AdmittedFlow {
    pub flow: FlowSpec,
    pub flow_cid: braid_ir::Cid,
}

pub fn verify(bytes: &[u8]) -> VerifyResult<AdmittedFlow> {
    let flow = decode_flow_verify(bytes)?;
    let graph = CompactGraph::build(&flow)?;
    check_acyclic(&graph)?;
    check_reachability(&flow, &graph)?;
    check_choice(&flow)?;
    check_join(&flow, &graph)?;
    check_terminals(&flow)?;
    check_justification(&flow)?;
    let flow_cid = braid_ir::Cid::compute(braid_flow_ir::FLOW_DOMAIN, bytes);
    Ok(AdmittedFlow { flow, flow_cid })
}

fn arithmetic_overflow(field: &'static str) -> FlowVerifyError {
    FlowVerifyError::ArithmeticOverflow {
        field,
        invariant: "INV-FLOW-004",
    }
}

fn resolve_node(flow: &FlowSpec, key: &str, field: &'static str) -> VerifyResult<NodeId> {
    let index = flow
        .nodes()
        .binary_search_by(|node| node.key.as_str().cmp(key))
        .map_err(|_| FlowVerifyError::Unresolved {
            field,
            key: key.to_string(),
            invariant: "INV-FLOW-002",
        })?;
    u32::try_from(index).map_err(|_| arithmetic_overflow("node index"))
}

fn choice_edge_count(flow: &FlowSpec) -> VerifyResult<usize> {
    flow.nodes().iter().try_fold(0usize, |count, node| {
        let added = match &node.kind {
            FlowNodeKind::Choice { arms, .. } => arms
                .len()
                .checked_add(1)
                .ok_or_else(|| arithmetic_overflow("choice edge count"))?,
            _ => 0,
        };
        count
            .checked_add(added)
            .ok_or_else(|| arithmetic_overflow("choice edge count"))
    })
}

fn collect_edges(flow: &FlowSpec) -> VerifyResult<Vec<Edge>> {
    let capacity = flow
        .edges()
        .len()
        .checked_add(choice_edge_count(flow)?)
        .ok_or_else(|| arithmetic_overflow("graph edge capacity"))?;
    let mut edges = Vec::with_capacity(capacity);

    for edge in flow.edges() {
        match edge {
            FlowEdge::After { from, to, .. } => {
                edges.push((
                    resolve_node(flow, from.as_str(), "after.from")?,
                    resolve_node(flow, to.as_str(), "after.to")?,
                ));
            }
            FlowEdge::Data { from, to, .. } => {
                let destination = resolve_node(flow, to.node.as_str(), "data.to")?;
                match from {
                    ValueSource::Node(output) => edges.push((
                        resolve_node(flow, output.node.as_str(), "data.from")?,
                        destination,
                    )),
                    ValueSource::Root(_) | ValueSource::Literal(_) => {}
                }
            }
        }
    }

    for (source_index, node) in flow.nodes().iter().enumerate() {
        let source =
            u32::try_from(source_index).map_err(|_| arithmetic_overflow("choice source"))?;
        if let FlowNodeKind::Choice { arms, otherwise } = &node.kind {
            for arm in arms {
                edges.push((
                    source,
                    resolve_node(flow, arm.then.as_str(), "choice.then")?,
                ));
            }
            edges.push((
                source,
                resolve_node(flow, otherwise.as_str(), "choice.otherwise")?,
            ));
        }
    }

    edges.sort_unstable();
    edges.dedup();
    Ok(edges)
}

/// Compressed-sparse-row adjacency using u32 node IDs and offsets.
///
/// Flow's hard bounds are far below u32 limits. This avoids one heap
/// allocation per node (`Vec<Vec<usize>>`) and halves edge storage relative to
/// host-sized `usize` pairs on 64-bit machines.
struct Csr {
    offsets: Vec<u32>,
    targets: Vec<NodeId>,
}

impl Csr {
    fn from_sorted(node_count: usize, edges: &[Edge]) -> VerifyResult<Self> {
        let offset_count = node_count
            .checked_add(1)
            .ok_or_else(|| arithmetic_overflow("csr offsets"))?;
        let mut offsets = vec![0u32; offset_count];

        for &(source, _) in edges {
            let slot = offsets
                .get_mut(source as usize + 1)
                .ok_or_else(|| arithmetic_overflow("csr source"))?;
            *slot = slot
                .checked_add(1)
                .ok_or_else(|| arithmetic_overflow("csr degree"))?;
        }

        for index in 1..offsets.len() {
            offsets[index] = offsets[index]
                .checked_add(offsets[index - 1])
                .ok_or_else(|| arithmetic_overflow("csr prefix sum"))?;
        }

        let targets = edges.iter().map(|&(_, target)| target).collect();
        Ok(Self { offsets, targets })
    }

    fn neighbors(&self, node: NodeId) -> &[NodeId] {
        let index = node as usize;
        let start = self.offsets[index] as usize;
        let end = self.offsets[index + 1] as usize;
        &self.targets[start..end]
    }
}

struct CompactGraph {
    node_count: NodeId,
    forward: Csr,
    reverse: Csr,
}

impl CompactGraph {
    fn build(flow: &FlowSpec) -> VerifyResult<Self> {
        let node_count =
            u32::try_from(flow.nodes().len()).map_err(|_| arithmetic_overflow("node count"))?;
        let mut edges = collect_edges(flow)?;
        let forward = Csr::from_sorted(flow.nodes().len(), &edges)?;

        // Reuse the same edge arena to build the reverse CSR. This keeps peak
        // memory to one pair arena plus the two compact target arrays.
        for edge in &mut edges {
            *edge = (edge.1, edge.0);
        }
        edges.sort_unstable();
        edges.dedup();
        let reverse = Csr::from_sorted(flow.nodes().len(), &edges)?;

        Ok(Self {
            node_count,
            forward,
            reverse,
        })
    }
}

fn initial_indegree(graph: &CompactGraph) -> VerifyResult<Vec<u32>> {
    (0..graph.node_count)
        .map(|node| {
            u32::try_from(graph.reverse.neighbors(node).len())
                .map_err(|_| arithmetic_overflow("node indegree"))
        })
        .collect()
}

fn check_acyclic(graph: &CompactGraph) -> VerifyResult<()> {
    let mut indegree = initial_indegree(graph)?;
    let mut queue = Vec::with_capacity(graph.node_count as usize);
    for node in 0..graph.node_count {
        if indegree[node as usize] == 0 {
            queue.push(node);
        }
    }

    let mut cursor = 0usize;
    while cursor < queue.len() {
        let node = queue[cursor];
        cursor += 1;
        for &target in graph.forward.neighbors(node) {
            let degree = &mut indegree[target as usize];
            *degree = degree
                .checked_sub(1)
                .ok_or_else(|| arithmetic_overflow("kahn indegree"))?;
            if *degree == 0 {
                queue.push(target);
            }
        }
    }

    if queue.len() == graph.node_count as usize {
        Ok(())
    } else {
        Err(FlowVerifyError::Cycle {
            invariant: "INV-FLOW-003",
        })
    }
}

fn mark_reachable(adjacency: &Csr, node_count: NodeId, seeds: &[NodeId]) -> Vec<bool> {
    let mut reached = vec![false; node_count as usize];
    let mut queue = Vec::with_capacity(node_count as usize);
    for &seed in seeds {
        if !reached[seed as usize] {
            reached[seed as usize] = true;
            queue.push(seed);
        }
    }

    let mut cursor = 0usize;
    while cursor < queue.len() {
        let node = queue[cursor];
        cursor += 1;
        for &target in adjacency.neighbors(node) {
            if !reached[target as usize] {
                reached[target as usize] = true;
                queue.push(target);
            }
        }
    }
    reached
}

fn terminal_nodes(flow: &FlowSpec) -> VerifyResult<Vec<NodeId>> {
    flow.terminals()
        .iter()
        .map(|terminal| resolve_node(flow, terminal.as_str(), "terminal"))
        .collect()
}

fn check_reachability(flow: &FlowSpec, graph: &CompactGraph) -> VerifyResult<()> {
    // After acyclicity is proven, every node is reachable from at least one
    // zero-indegree node; the synthetic start connects exactly those roots.
    // The non-trivial half is proving that every node can reach a declared
    // terminal.
    let to_terminal = mark_reachable(&graph.reverse, graph.node_count, &terminal_nodes(flow)?);

    if to_terminal.iter().all(|reached| *reached) {
        Ok(())
    } else {
        Err(FlowVerifyError::TerminalSoundness {
            invariant: "INV-FLOW-014",
        })
    }
}

fn check_choice(flow: &FlowSpec) -> VerifyResult<()> {
    preflight_choice_solver(flow)?;
    for node in flow.nodes() {
        if let FlowNodeKind::Choice { arms, .. } = &node.kind {
            if arms.is_empty() {
                return Err(FlowVerifyError::ChoiceNotTotal {
                    invariant: "INV-FLOW-011",
                });
            }

            for (index, arm) in arms.iter().enumerate() {
                for (prior_index, prior) in arms[..index].iter().enumerate() {
                    if prior.then == arm.then {
                        return Err(FlowVerifyError::DuplicateChoiceTarget {
                            choice: node.key.to_string(),
                            left_arm: prior_index,
                            right_arm: index,
                            target: arm.then.to_string(),
                            invariant: "INV-FLOW-011",
                        });
                    }
                    match analyze_preflighted(&prior.when, &arm.when) {
                        Disjointness::Disjoint => {}
                        Disjointness::Overlap(counterexample) => {
                            return Err(FlowVerifyError::ChoiceNotDisjoint {
                                overlap: Box::new(ChoiceOverlap {
                                    choice: node.key.to_string(),
                                    left_arm: prior_index,
                                    right_arm: index,
                                    left_target: prior.then.to_string(),
                                    right_target: arm.then.to_string(),
                                    counterexample,
                                }),
                                invariant: "INV-FLOW-011",
                            });
                        }
                        Disjointness::Unknown(reason) => {
                            return Err(FlowVerifyError::ChoiceDisjointnessUnknown {
                                choice: node.key.to_string(),
                                left_arm: prior_index,
                                right_arm: index,
                                reason,
                                invariant: "INV-FLOW-007",
                            });
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn preflight_choice_solver(flow: &FlowSpec) -> VerifyResult<()> {
    let mut work = 0usize;
    for node in flow.nodes() {
        if let FlowNodeKind::Choice { arms, .. } = &node.kind {
            for (right_index, right) in arms.iter().enumerate() {
                for (left_index, left) in arms[..right_index].iter().enumerate() {
                    let pair_work = preflight_pair(&left.when, &right.when).map_err(|reason| {
                        FlowVerifyError::ChoiceDisjointnessUnknown {
                            choice: node.key.to_string(),
                            left_arm: left_index,
                            right_arm: right_index,
                            reason,
                            invariant: "INV-FLOW-007",
                        }
                    })?;
                    work = work.checked_add(pair_work).ok_or_else(|| {
                        choice_work_unknown(
                            node.key.to_string(),
                            left_index,
                            right_index,
                            usize::MAX,
                        )
                    })?;
                    if work > DISJOINTNESS_MAX_WORK {
                        return Err(choice_work_unknown(
                            node.key.to_string(),
                            left_index,
                            right_index,
                            work,
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn choice_work_unknown(
    choice: alloc::string::String,
    left_arm: usize,
    right_arm: usize,
    actual: usize,
) -> FlowVerifyError {
    FlowVerifyError::ChoiceDisjointnessUnknown {
        choice,
        left_arm,
        right_arm,
        reason: DisjointnessUnknown::LimitExceeded {
            kind: crate::disjoint::SolverLimit::Work,
            actual,
            limit: DISJOINTNESS_MAX_WORK,
        },
        invariant: "INV-FLOW-007",
    }
}

fn check_join(flow: &FlowSpec, graph: &CompactGraph) -> VerifyResult<()> {
    for (index, node) in flow.nodes().iter().enumerate() {
        if matches!(node.kind, FlowNodeKind::JoinAll) {
            let node_id = u32::try_from(index).map_err(|_| arithmetic_overflow("join index"))?;
            if graph.reverse.neighbors(node_id).is_empty() {
                return Err(FlowVerifyError::JoinCardinality {
                    invariant: "INV-FLOW-013",
                });
            }
        }
    }
    Ok(())
}

fn check_terminals(flow: &FlowSpec) -> VerifyResult<()> {
    if flow.terminals().is_empty() {
        return Err(FlowVerifyError::EmptyCollection {
            field: "terminals",
            invariant: "INV-FLOW-014",
        });
    }
    for terminal in flow.terminals() {
        resolve_node(flow, terminal.as_str(), "terminal")?;
    }
    Ok(())
}

fn check_justification(flow: &FlowSpec) -> VerifyResult<()> {
    for node in flow.nodes() {
        if matches!(node.kind, FlowNodeKind::InvokeCapsule { .. }) && node.justification.is_none() {
            return Err(FlowVerifyError::JustificationIncomplete {
                field: "justification",
                invariant: "INV-FLOW-006",
            });
        }
    }
    Ok(())
}
