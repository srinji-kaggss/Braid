//! Unified program graph kernel.
//!
//! This module introduces one semantic graph over operations and typed edges.
//! Existing `Braid` and `FlowSpec` remain compatibility forms during migration;
//! this kernel does not change their wire encodings or CIDs.

use crate::term::TypeTag;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(String);

impl NodeId {
    pub fn new(value: impl Into<String>) -> Result<Self, GraphError> {
        let value = value.into();
        if value.is_empty() {
            return Err(GraphError::InvalidNodeId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum GraphNodeKind {
    InvokeTerm { term: String },
    InvokeCapsule { cid: [u8; 32] },
    Choice,
    Join,
    TerminalSuccess,
    TerminalFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GraphNode {
    pub id: NodeId,
    pub kind: GraphNodeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OutputPort {
    pub node: NodeId,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct InputPort {
    pub node: NodeId,
    pub port: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompletionClass {
    Success,
    Satisfied,
    Failure,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum GraphEdge {
    Data {
        from: OutputPort,
        to: InputPort,
        value_type: TypeTag,
    },
    Control {
        from: NodeId,
        to: NodeId,
        on: Vec<CompletionClass>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramGraph {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
    outputs: Vec<OutputPort>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlFlowGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyDag {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    InvalidNodeId,
    DuplicateNode { node: String },
    MissingEndpoint { node: String },
    SelfEdge { node: String },
    DuplicateEdge,
    ControlCycle,
    DataCycle,
}

impl ProgramGraph {
    pub fn new(
        mut nodes: Vec<GraphNode>,
        mut edges: Vec<GraphEdge>,
        mut outputs: Vec<OutputPort>,
    ) -> Result<Self, GraphError> {
        nodes.sort();
        edges.sort();
        outputs.sort();

        validate_unique_nodes(&nodes)?;
        validate_endpoints(&nodes, &edges, &outputs)?;
        validate_edges(&edges)?;
        reject_cycle(&nodes, &edges, EdgeClass::Control)?;
        reject_cycle(&nodes, &edges, EdgeClass::Data)?;

        Ok(Self {
            nodes,
            edges,
            outputs,
        })
    }

    pub fn nodes(&self) -> &[GraphNode] {
        &self.nodes
    }

    pub fn edges(&self) -> &[GraphEdge] {
        &self.edges
    }

    pub fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }

    pub fn cfg(&self) -> ControlFlowGraph {
        let edges = self
            .edges
            .iter()
            .filter(|edge| matches!(edge, GraphEdge::Control { .. }))
            .cloned()
            .collect();
        ControlFlowGraph {
            nodes: self.nodes.clone(),
            edges,
        }
    }

    pub fn dag(&self) -> DependencyDag {
        let edges = self
            .edges
            .iter()
            .filter(|edge| matches!(edge, GraphEdge::Data { .. }))
            .cloned()
            .collect();
        DependencyDag {
            nodes: self.nodes.clone(),
            edges,
        }
    }
}

fn validate_unique_nodes(nodes: &[GraphNode]) -> Result<(), GraphError> {
    let mut previous: Option<&NodeId> = None;
    for node in nodes {
        if previous == Some(&node.id) {
            return Err(GraphError::DuplicateNode {
                node: node.id.as_str().into(),
            });
        }
        previous = Some(&node.id);
    }
    Ok(())
}

fn validate_endpoints(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
    outputs: &[OutputPort],
) -> Result<(), GraphError> {
    let ids: BTreeSet<NodeId> = nodes.iter().map(|node| node.id.clone()).collect();
    for edge in edges {
        match edge {
            GraphEdge::Data { from, to, .. } => {
                ensure_endpoint(&ids, &from.node)?;
                ensure_endpoint(&ids, &to.node)?;
                if from.node == to.node {
                    return Err(GraphError::SelfEdge {
                        node: from.node.as_str().into(),
                    });
                }
            }
            GraphEdge::Control { from, to, .. } => {
                ensure_endpoint(&ids, from)?;
                ensure_endpoint(&ids, to)?;
                if from == to {
                    return Err(GraphError::SelfEdge {
                        node: from.as_str().into(),
                    });
                }
            }
        }
    }
    for output in outputs {
        ensure_endpoint(&ids, &output.node)?;
    }
    Ok(())
}

fn ensure_endpoint(ids: &BTreeSet<NodeId>, node: &NodeId) -> Result<(), GraphError> {
    if ids.contains(node) {
        Ok(())
    } else {
        Err(GraphError::MissingEndpoint {
            node: node.as_str().into(),
        })
    }
}

fn validate_edges(edges: &[GraphEdge]) -> Result<(), GraphError> {
    for pair in edges.windows(2) {
        if pair[0] == pair[1] {
            return Err(GraphError::DuplicateEdge);
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum EdgeClass {
    Control,
    Data,
}

fn reject_cycle(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
    class: EdgeClass,
) -> Result<(), GraphError> {
    let mut indegree: BTreeMap<NodeId, usize> =
        nodes.iter().map(|node| (node.id.clone(), 0usize)).collect();
    let mut adjacency: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();

    for edge in edges {
        let endpoints = match (class, edge) {
            (EdgeClass::Control, GraphEdge::Control { from, to, .. }) => {
                Some((from.clone(), to.clone()))
            }
            (EdgeClass::Data, GraphEdge::Data { from, to, .. }) => {
                Some((from.node.clone(), to.node.clone()))
            }
            _ => None,
        };
        if let Some((from, to)) = endpoints {
            adjacency.entry(from).or_default().push(to.clone());
            *indegree.get_mut(&to).expect("endpoint validated") += 1;
        }
    }

    let mut ready: BTreeSet<NodeId> = indegree
        .iter()
        .filter_map(|(node, degree)| (*degree == 0).then_some(node.clone()))
        .collect();
    let mut visited = 0usize;

    while let Some(node) = ready.pop_first() {
        visited += 1;
        if let Some(next) = adjacency.get(&node) {
            for target in next {
                let degree = indegree.get_mut(target).expect("endpoint validated");
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(target.clone());
                }
            }
        }
    }

    if visited == nodes.len() {
        Ok(())
    } else {
        match class {
            EdgeClass::Control => Err(GraphError::ControlCycle),
            EdgeClass::Data => Err(GraphError::DataCycle),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn node(id: &str) -> GraphNode {
        GraphNode {
            id: NodeId::new(id).unwrap(),
            kind: GraphNodeKind::Join,
        }
    }

    #[test]
    fn deterministic_ordering_is_insertion_independent() {
        let a = node("a");
        let b = node("b");
        let edge = GraphEdge::Control {
            from: a.id.clone(),
            to: b.id.clone(),
            on: vec![CompletionClass::Success],
        };
        let left =
            ProgramGraph::new(vec![b.clone(), a.clone()], vec![edge.clone()], vec![]).unwrap();
        let right = ProgramGraph::new(vec![a, b], vec![edge], vec![]).unwrap();
        assert_eq!(left, right);
    }

    #[test]
    fn duplicate_nodes_fail_closed() {
        let err = ProgramGraph::new(vec![node("a"), node("a")], vec![], vec![]).unwrap_err();
        assert!(matches!(err, GraphError::DuplicateNode { .. }));
    }

    #[test]
    fn missing_endpoint_fails_closed() {
        let edge = GraphEdge::Control {
            from: NodeId::new("a").unwrap(),
            to: NodeId::new("missing").unwrap(),
            on: vec![CompletionClass::Success],
        };
        let err = ProgramGraph::new(vec![node("a")], vec![edge], vec![]).unwrap_err();
        assert!(matches!(err, GraphError::MissingEndpoint { .. }));
    }

    #[test]
    fn duplicate_edges_fail_closed() {
        let edge = GraphEdge::Control {
            from: NodeId::new("a").unwrap(),
            to: NodeId::new("b").unwrap(),
            on: vec![CompletionClass::Success],
        };
        let err = ProgramGraph::new(vec![node("a"), node("b")], vec![edge.clone(), edge], vec![])
            .unwrap_err();
        assert_eq!(err, GraphError::DuplicateEdge);
    }

    #[test]
    fn control_cycle_fails_closed() {
        let e1 = GraphEdge::Control {
            from: NodeId::new("a").unwrap(),
            to: NodeId::new("b").unwrap(),
            on: vec![CompletionClass::Success],
        };
        let e2 = GraphEdge::Control {
            from: NodeId::new("b").unwrap(),
            to: NodeId::new("a").unwrap(),
            on: vec![CompletionClass::Success],
        };
        let err = ProgramGraph::new(vec![node("a"), node("b")], vec![e1, e2], vec![]).unwrap_err();
        assert_eq!(err, GraphError::ControlCycle);
    }

    #[test]
    fn data_cycle_fails_closed() {
        let e1 = GraphEdge::Data {
            from: OutputPort {
                node: NodeId::new("a").unwrap(),
                port: 0,
            },
            to: InputPort {
                node: NodeId::new("b").unwrap(),
                port: 0,
            },
            value_type: TypeTag::Int,
        };
        let e2 = GraphEdge::Data {
            from: OutputPort {
                node: NodeId::new("b").unwrap(),
                port: 0,
            },
            to: InputPort {
                node: NodeId::new("a").unwrap(),
                port: 0,
            },
            value_type: TypeTag::Int,
        };
        let err = ProgramGraph::new(vec![node("a"), node("b")], vec![e1, e2], vec![]).unwrap_err();
        assert_eq!(err, GraphError::DataCycle);
    }

    #[test]
    fn cfg_and_dag_are_disjoint_views() {
        let control = GraphEdge::Control {
            from: NodeId::new("a").unwrap(),
            to: NodeId::new("b").unwrap(),
            on: vec![CompletionClass::Success],
        };
        let data = GraphEdge::Data {
            from: OutputPort {
                node: NodeId::new("a").unwrap(),
                port: 0,
            },
            to: InputPort {
                node: NodeId::new("b").unwrap(),
                port: 0,
            },
            value_type: TypeTag::Int,
        };
        let graph =
            ProgramGraph::new(vec![node("a"), node("b")], vec![control, data], vec![]).unwrap();
        assert_eq!(graph.cfg().edges.len(), 1);
        assert_eq!(graph.dag().edges.len(), 1);
        assert!(matches!(graph.cfg().edges[0], GraphEdge::Control { .. }));
        assert!(matches!(graph.dag().edges[0], GraphEdge::Data { .. }));
    }
}
