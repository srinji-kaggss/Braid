//! Normalized Flow graph and fail-fast construction invariants.

use crate::FlowCid;
use crate::encode::flow_to_canon;
use crate::error::{FlowError, FlowResult, LimitKind};
use crate::literal::{MAX_LITERAL_BYTES, MAX_LITERAL_NODES, validate_literal};
use crate::predicate::Predicate;
use crate::symbol::{
    CostOrderRef, FlowName, InputKey, InvariantRef, NodeKey, PortKey, RelationRef,
};
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::ToString;
use alloc::vec::Vec;
use braid_ir::{Cid, TypeTag, Value, encode, type_tag_node_count};

pub const FLOW_VERSION: u16 = 0;
pub const FLOW_DOMAIN: &[u8] = b"lw.braid.flow.v0";

pub(crate) const HARD_MAX_SOURCE_NODES: u32 = 10_000;
pub(crate) const HARD_MAX_SOURCE_EDGES: u32 = 50_000;
const HARD_MAX_EXPANDED_NODES: u32 = 50_000;
const HARD_MAX_EXPANDED_EDGES: u32 = 250_000;
const HARD_MAX_PREDICATE_DEPTH: u32 = 32;
pub(crate) const HARD_MAX_PREDICATE_NODES: usize = 16_384;
pub(crate) const HARD_MAX_CHOICE_ARMS: usize = 128;
const HARD_MAX_PORTS: usize = 128;
// Provisional P1 resource ceiling; the ratified table does not yet name this
// collection independently.
pub(crate) const PROVISIONAL_MAX_JUSTIFICATION_REFERENCES: usize = 128;
pub(crate) const PROVISIONAL_MAX_JUSTIFICATION_REFERENCES_PER_FLOW: usize = 16_384;
// Aggregate graph work ceiling. Individual tags remain capped by braid-ir;
// this prevents many individually valid tags from multiplying without bound.
pub(crate) const PROVISIONAL_MAX_TYPE_TAG_NODES_PER_FLOW: usize = 262_144;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowBounds {
    pub max_nodes: u32,
    pub max_edges: u32,
    pub max_predicate_depth: u16,
    pub max_expanded_nodes: u32,
    pub max_expanded_edges: u32,
}

impl Default for FlowBounds {
    fn default() -> Self {
        Self {
            max_nodes: HARD_MAX_SOURCE_NODES,
            max_edges: HARD_MAX_SOURCE_EDGES,
            max_predicate_depth: HARD_MAX_PREDICATE_DEPTH as u16,
            max_expanded_nodes: HARD_MAX_EXPANDED_NODES,
            max_expanded_edges: HARD_MAX_EXPANDED_EDGES,
        }
    }
}

impl FlowBounds {
    pub(crate) fn validate(self) -> FlowResult<()> {
        check_positive_bound(
            LimitKind::SourceNodes,
            self.max_nodes,
            HARD_MAX_SOURCE_NODES,
        )?;
        check_bound(
            LimitKind::SourceEdges,
            self.max_edges,
            HARD_MAX_SOURCE_EDGES,
        )?;
        check_positive_bound(
            LimitKind::ExpandedNodes,
            self.max_expanded_nodes,
            HARD_MAX_EXPANDED_NODES,
        )?;
        check_bound(
            LimitKind::ExpandedEdges,
            self.max_expanded_edges,
            HARD_MAX_EXPANDED_EDGES,
        )?;
        check_positive_bound(
            LimitKind::PredicateDepth,
            u32::from(self.max_predicate_depth),
            HARD_MAX_PREDICATE_DEPTH,
        )
    }
}

fn check_bound(kind: LimitKind, requested: u32, hard_limit: u32) -> FlowResult<()> {
    if requested > hard_limit {
        Err(FlowError::InvalidBound {
            kind,
            requested,
            hard_limit,
            invariant: "INV-FLOW-004",
        })
    } else {
        Ok(())
    }
}

fn check_positive_bound(kind: LimitKind, requested: u32, hard_limit: u32) -> FlowResult<()> {
    if requested == 0 {
        Err(FlowError::InvalidBound {
            kind,
            requested,
            hard_limit,
            invariant: "INV-FLOW-004",
        })
    } else {
        check_bound(kind, requested, hard_limit)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowInput {
    pub key: InputKey,
    pub value_type: TypeTag,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OutputPort {
    pub node: NodeKey,
    pub port: PortKey,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct InputPort {
    pub node: NodeKey,
    pub port: PortKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompletionClass {
    ExecutedSuccess,
    SatisfiedWithoutExecution,
    Failure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueSource {
    Root(InputKey),
    Node(OutputPort),
    Literal(Value),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowEdge {
    Data {
        from: ValueSource,
        to: InputPort,
        value_type: TypeTag,
    },
    After {
        from: NodeKey,
        to: NodeKey,
        on: Vec<CompletionClass>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalOutcome {
    Success,
    Failure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceArm {
    pub when: Predicate,
    pub then: NodeKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowNodeKind {
    InvokeCapsule {
        capsule: Cid,
    },
    Choice {
        arms: Vec<ChoiceArm>,
        otherwise: NodeKey,
    },
    JoinAll,
    Terminal {
        outcome: TerminalOutcome,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JustificationDecl {
    pub needed_when: Predicate,
    pub satisfied_when: Predicate,
    pub guarantees: Vec<RelationRef>,
    pub preserves: Vec<InvariantRef>,
    pub cost_order: Option<CostOrderRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UrgencyClass {
    SafetyRecovery,
    Required,
    Diagnostic,
    Optimization,
    Cleanup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowNode {
    pub key: NodeKey,
    pub kind: FlowNodeKind,
    pub guard: Predicate,
    pub justification: Option<JustificationDecl>,
    pub urgency: UrgencyClass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowSpec {
    pub(crate) version: u16,
    pub(crate) name: FlowName,
    pub(crate) roots: Vec<FlowInput>,
    pub(crate) nodes: Vec<FlowNode>,
    pub(crate) edges: Vec<FlowEdge>,
    pub(crate) terminals: Vec<NodeKey>,
    pub(crate) bounds: FlowBounds,
}

impl FlowSpec {
    pub fn new(
        name: FlowName,
        roots: Vec<FlowInput>,
        nodes: Vec<FlowNode>,
        edges: Vec<FlowEdge>,
        terminals: Vec<NodeKey>,
        bounds: FlowBounds,
    ) -> FlowResult<Self> {
        preflight_counts(&roots, &nodes, &edges, &terminals, bounds)?;
        let mut flow = Self {
            version: FLOW_VERSION,
            name,
            roots,
            nodes,
            edges,
            terminals,
            bounds,
        };
        flow.validate_and_normalize()?;
        Ok(flow)
    }

    pub fn to_canon(&self) -> Value {
        flow_to_canon(self)
    }

    /// Strictly decode the canonical Flow wire and require a byte-identical
    /// semantic round trip. Unknown fields, unknown closed variants, source
    /// order, duplicate semantic entries, and non-minimal CBOR all fail
    /// closed rather than being normalized during decoding.
    pub fn from_bytes(bytes: &[u8]) -> FlowResult<Self> {
        crate::decode::decode_flow(bytes)
    }

    pub const fn version(&self) -> u16 {
        self.version
    }

    pub fn name(&self) -> &FlowName {
        &self.name
    }

    pub fn roots(&self) -> &[FlowInput] {
        &self.roots
    }

    pub fn nodes(&self) -> &[FlowNode] {
        &self.nodes
    }

    pub fn edges(&self) -> &[FlowEdge] {
        &self.edges
    }

    pub fn terminals(&self) -> &[NodeKey] {
        &self.terminals
    }

    pub const fn bounds(&self) -> FlowBounds {
        self.bounds
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        encode(&self.to_canon())
    }

    pub fn cid(&self) -> FlowCid {
        FlowCid::new(Cid::compute(FLOW_DOMAIN, &self.canonical_bytes()))
    }

    fn validate_and_normalize(&mut self) -> FlowResult<()> {
        self.roots.sort_by(|left, right| left.key.cmp(&right.key));
        reject_duplicate_roots(&self.roots)?;
        self.nodes.sort_by(|left, right| left.key.cmp(&right.key));
        reject_duplicate_nodes(&self.nodes)?;
        self.terminals.sort();
        reject_duplicate_terminals(&self.terminals)?;

        let node_keys: BTreeSet<NodeKey> = self.nodes.iter().map(|node| node.key.clone()).collect();
        let root_keys: BTreeSet<InputKey> =
            self.roots.iter().map(|root| root.key.clone()).collect();
        let mut remaining_literal_bytes = MAX_LITERAL_BYTES;
        let mut remaining_literal_nodes = MAX_LITERAL_NODES;
        let mut remaining_type_tag_nodes = PROVISIONAL_MAX_TYPE_TAG_NODES_PER_FLOW;
        validate_root_types(&self.roots, &mut remaining_type_tag_nodes)?;
        validate_terminals(&self.terminals, &node_keys)?;
        validate_nodes(
            &mut self.nodes,
            &node_keys,
            &self.bounds,
            &mut remaining_literal_bytes,
            &mut remaining_literal_nodes,
        )?;
        normalize_and_validate_edges(
            &mut self.edges,
            &node_keys,
            &root_keys,
            &mut remaining_literal_bytes,
            &mut remaining_literal_nodes,
            &mut remaining_type_tag_nodes,
        )?;
        reject_cycle(&self.nodes, &self.edges)?;
        crate::wire_depth::validate(self)?;
        Ok(())
    }
}

fn preflight_counts(
    roots: &[FlowInput],
    nodes: &[FlowNode],
    edges: &[FlowEdge],
    terminals: &[NodeKey],
    bounds: FlowBounds,
) -> FlowResult<()> {
    bounds.validate()?;
    if terminals.is_empty() {
        return Err(FlowError::EmptyCollection {
            field: "terminals",
            invariant: "INV-FLOW-014",
        });
    }
    check_count(
        LimitKind::SourceNodes,
        nodes.len(),
        bounds.max_nodes as usize,
    )?;
    check_count(
        LimitKind::ExpandedNodes,
        nodes.len(),
        bounds.max_expanded_nodes as usize,
    )?;
    check_count(LimitKind::Roots, roots.len(), bounds.max_nodes as usize)?;
    check_count(
        LimitKind::Terminals,
        terminals.len(),
        nodes.len().min(bounds.max_nodes as usize),
    )?;
    check_count(
        LimitKind::SourceEdges,
        edges.len(),
        bounds.max_edges as usize,
    )?;
    check_count(
        LimitKind::ExpandedEdges,
        edges.len(),
        bounds.max_expanded_edges as usize,
    )
}

fn check_count(kind: LimitKind, actual: usize, limit: usize) -> FlowResult<()> {
    if actual > limit {
        Err(FlowError::LimitExceeded {
            kind,
            actual,
            limit,
            invariant: "INV-FLOW-004",
        })
    } else {
        Ok(())
    }
}

fn reject_duplicate_roots(roots: &[FlowInput]) -> FlowResult<()> {
    for pair in roots.windows(2) {
        if pair[0].key == pair[1].key {
            return Err(FlowError::Duplicate {
                field: "root",
                key: pair[0].key.to_string(),
                invariant: "INV-FLOW-001",
            });
        }
    }
    Ok(())
}

fn validate_root_types(
    roots: &[FlowInput],
    remaining_type_tag_nodes: &mut usize,
) -> FlowResult<()> {
    for root in roots {
        check_type_tag(&root.value_type, "root type", remaining_type_tag_nodes)?;
    }
    Ok(())
}

fn reject_duplicate_nodes(nodes: &[FlowNode]) -> FlowResult<()> {
    for pair in nodes.windows(2) {
        if pair[0].key == pair[1].key {
            return Err(FlowError::Duplicate {
                field: "node",
                key: pair[0].key.to_string(),
                invariant: "INV-FLOW-001",
            });
        }
    }
    Ok(())
}

fn reject_duplicate_terminals(terminals: &[NodeKey]) -> FlowResult<()> {
    for pair in terminals.windows(2) {
        if pair[0] == pair[1] {
            return Err(FlowError::Duplicate {
                field: "terminal",
                key: pair[0].to_string(),
                invariant: "INV-FLOW-001",
            });
        }
    }
    Ok(())
}

fn validate_terminals(terminals: &[NodeKey], node_keys: &BTreeSet<NodeKey>) -> FlowResult<()> {
    for terminal in terminals {
        if !node_keys.contains(terminal) {
            return Err(FlowError::Unresolved {
                field: "terminal",
                key: terminal.to_string(),
                invariant: "INV-FLOW-002",
            });
        }
    }
    Ok(())
}

fn validate_nodes(
    nodes: &mut [FlowNode],
    node_keys: &BTreeSet<NodeKey>,
    bounds: &FlowBounds,
    remaining_literal_bytes: &mut usize,
    remaining_literal_nodes: &mut usize,
) -> FlowResult<()> {
    let mut remaining_predicate_nodes = HARD_MAX_PREDICATE_NODES;
    let mut remaining_justification_references = PROVISIONAL_MAX_JUSTIFICATION_REFERENCES_PER_FLOW;
    for node in nodes {
        node.guard.validate(
            bounds,
            &mut remaining_predicate_nodes,
            remaining_literal_bytes,
            remaining_literal_nodes,
        )?;
        validate_node_kind(
            node,
            node_keys,
            bounds,
            &mut remaining_predicate_nodes,
            remaining_literal_bytes,
            remaining_literal_nodes,
        )?;
        if let Some(justification) = &mut node.justification {
            validate_justification_shape(
                justification,
                bounds,
                &mut remaining_predicate_nodes,
                remaining_literal_bytes,
                remaining_literal_nodes,
                &mut remaining_justification_references,
            )?;
        }
    }
    Ok(())
}

fn validate_node_kind(
    node: &mut FlowNode,
    node_keys: &BTreeSet<NodeKey>,
    bounds: &FlowBounds,
    remaining_predicate_nodes: &mut usize,
    remaining_literal_bytes: &mut usize,
    remaining_literal_nodes: &mut usize,
) -> FlowResult<()> {
    if let FlowNodeKind::Choice { arms, otherwise } = &mut node.kind {
        if arms.is_empty() {
            return Err(FlowError::EmptyCollection {
                field: "choice arms",
                invariant: "INV-FLOW-005",
            });
        }
        check_count(LimitKind::ChoiceArms, arms.len(), HARD_MAX_CHOICE_ARMS)?;
        resolve_node(otherwise, node_keys, "choice otherwise")?;
        for arm in arms.iter_mut() {
            resolve_node(&arm.then, node_keys, "choice arm")?;
            arm.when.validate(
                bounds,
                remaining_predicate_nodes,
                remaining_literal_bytes,
                remaining_literal_nodes,
            )?;
        }
        arms.sort_by(|left, right| {
            let left_bytes = encode(&crate::encode::predicate_to_canon(&left.when));
            let right_bytes = encode(&crate::encode::predicate_to_canon(&right.when));
            left_bytes
                .cmp(&right_bytes)
                .then_with(|| left.then.cmp(&right.then))
        });
    }
    Ok(())
}

fn validate_justification_shape(
    justification: &mut JustificationDecl,
    bounds: &FlowBounds,
    remaining_predicate_nodes: &mut usize,
    remaining_literal_bytes: &mut usize,
    remaining_literal_nodes: &mut usize,
    remaining_references: &mut usize,
) -> FlowResult<()> {
    check_count(
        LimitKind::References,
        justification.guarantees.len(),
        PROVISIONAL_MAX_JUSTIFICATION_REFERENCES,
    )?;
    check_count(
        LimitKind::References,
        justification.preserves.len(),
        PROVISIONAL_MAX_JUSTIFICATION_REFERENCES,
    )?;
    justification.guarantees.sort();
    justification.guarantees.dedup();
    justification.preserves.sort();
    justification.preserves.dedup();
    let reference_count = justification
        .guarantees
        .len()
        .checked_add(justification.preserves.len())
        .ok_or(FlowError::ArithmeticOverflow {
            field: "justification reference count",
            invariant: "INV-FLOW-004",
        })?;
    *remaining_references =
        remaining_references
            .checked_sub(reference_count)
            .ok_or(FlowError::LimitExceeded {
                kind: LimitKind::References,
                actual: PROVISIONAL_MAX_JUSTIFICATION_REFERENCES_PER_FLOW + 1,
                limit: PROVISIONAL_MAX_JUSTIFICATION_REFERENCES_PER_FLOW,
                invariant: "INV-FLOW-004",
            })?;
    justification.needed_when.validate(
        bounds,
        remaining_predicate_nodes,
        remaining_literal_bytes,
        remaining_literal_nodes,
    )?;
    justification.satisfied_when.validate(
        bounds,
        remaining_predicate_nodes,
        remaining_literal_bytes,
        remaining_literal_nodes,
    )
}

fn resolve_node(
    key: &NodeKey,
    node_keys: &BTreeSet<NodeKey>,
    field: &'static str,
) -> FlowResult<()> {
    if node_keys.contains(key) {
        Ok(())
    } else {
        Err(FlowError::Unresolved {
            field,
            key: key.to_string(),
            invariant: "INV-FLOW-002",
        })
    }
}

fn normalize_and_validate_edges(
    edges: &mut [FlowEdge],
    node_keys: &BTreeSet<NodeKey>,
    root_keys: &BTreeSet<InputKey>,
    remaining_literal_bytes: &mut usize,
    remaining_literal_nodes: &mut usize,
    remaining_type_tag_nodes: &mut usize,
) -> FlowResult<()> {
    let mut input_ports: BTreeMap<NodeKey, BTreeSet<PortKey>> = BTreeMap::new();
    let mut output_ports: BTreeMap<NodeKey, BTreeSet<PortKey>> = BTreeMap::new();
    for edge in edges.iter_mut() {
        match edge {
            FlowEdge::Data {
                from,
                to,
                value_type,
            } => {
                check_type_tag(value_type, "data-edge type", remaining_type_tag_nodes)?;
                resolve_node(&to.node, node_keys, "data destination")?;
                record_port(&mut input_ports, &to.node, &to.port, "input port", true)?;
                match from {
                    ValueSource::Root(root) if !root_keys.contains(root) => {
                        return Err(FlowError::Unresolved {
                            field: "root source",
                            key: root.to_string(),
                            invariant: "INV-FLOW-002",
                        });
                    }
                    ValueSource::Node(output) => {
                        resolve_node(&output.node, node_keys, "data source")?;
                        record_port(
                            &mut output_ports,
                            &output.node,
                            &output.port,
                            "output port",
                            false,
                        )?;
                    }
                    ValueSource::Literal(value) => {
                        validate_literal(value, remaining_literal_bytes, remaining_literal_nodes)?;
                    }
                    ValueSource::Root(_) => {}
                }
            }
            FlowEdge::After { from, to, on } => {
                resolve_node(from, node_keys, "control source")?;
                resolve_node(to, node_keys, "control destination")?;
                if on.is_empty() {
                    return Err(FlowError::EmptyCollection {
                        field: "completion classes",
                        invariant: "INV-FLOW-013",
                    });
                }
                check_count(LimitKind::CompletionClasses, on.len(), 3)?;
                on.sort();
                on.dedup();
            }
        }
    }
    edges.sort_by_cached_key(|edge| encode(&crate::encode::edge_to_canon(edge)));
    for pair in edges.windows(2) {
        if pair[0] == pair[1] {
            return Err(FlowError::Duplicate {
                field: "edge",
                key: "canonical edge".to_string(),
                invariant: "INV-FLOW-001",
            });
        }
    }
    for node in node_keys {
        let input_count = input_ports.get(node).map_or(0, BTreeSet::len);
        let output_count = output_ports.get(node).map_or(0, BTreeSet::len);
        let total = input_count
            .checked_add(output_count)
            .ok_or(FlowError::ArithmeticOverflow {
                field: "port count",
                invariant: "INV-FLOW-004",
            })?;
        check_count(LimitKind::Ports, total, HARD_MAX_PORTS)?;
    }
    Ok(())
}

fn check_type_tag(
    value_type: &TypeTag,
    field: &'static str,
    remaining_type_tag_nodes: &mut usize,
) -> FlowResult<()> {
    let nodes = type_tag_node_count(value_type).map_err(|error| FlowError::InvalidTypeTag {
        field,
        error,
        invariant: "INV-FLOW-018",
    })?;
    *remaining_type_tag_nodes =
        remaining_type_tag_nodes
            .checked_sub(nodes)
            .ok_or(FlowError::LimitExceeded {
                kind: LimitKind::TypeTagNodes,
                actual: PROVISIONAL_MAX_TYPE_TAG_NODES_PER_FLOW + 1,
                limit: PROVISIONAL_MAX_TYPE_TAG_NODES_PER_FLOW,
                invariant: "INV-FLOW-004",
            })?;
    Ok(())
}

fn record_port(
    ports: &mut BTreeMap<NodeKey, BTreeSet<PortKey>>,
    node: &NodeKey,
    port: &PortKey,
    field: &'static str,
    reject_duplicate: bool,
) -> FlowResult<()> {
    let node_ports = ports.entry(node.clone()).or_default();
    let inserted = node_ports.insert(port.clone());
    if reject_duplicate && !inserted {
        return Err(FlowError::Duplicate {
            field,
            key: alloc::format!("{node}.{port}"),
            invariant: "INV-FLOW-012",
        });
    }
    check_count(LimitKind::Ports, node_ports.len(), HARD_MAX_PORTS)
}

fn reject_cycle(nodes: &[FlowNode], edges: &[FlowEdge]) -> FlowResult<()> {
    let mut index = BTreeMap::new();
    for (position, node) in nodes.iter().enumerate() {
        index.insert(node.key.clone(), position);
    }
    let mut adjacency = alloc::vec![Vec::<usize>::new(); nodes.len()];
    let mut indegree = alloc::vec![0usize; nodes.len()];
    for edge in edges {
        let endpoints = match edge {
            FlowEdge::Data {
                from: ValueSource::Node(output),
                to,
                ..
            } => Some((&output.node, &to.node)),
            FlowEdge::After { from, to, .. } => Some((from, to)),
            _ => None,
        };
        if let Some((from, to)) = endpoints {
            record_arc(&index, &mut adjacency, &mut indegree, from, to)?;
        }
    }
    for node in nodes {
        if let FlowNodeKind::Choice { arms, otherwise } = &node.kind {
            for arm in arms {
                record_arc(&index, &mut adjacency, &mut indegree, &node.key, &arm.then)?;
            }
            record_arc(&index, &mut adjacency, &mut indegree, &node.key, otherwise)?;
        }
    }
    let mut stack: Vec<usize> = indegree
        .iter()
        .enumerate()
        .filter_map(|(position, degree)| (*degree == 0).then_some(position))
        .collect();
    let mut visited = 0usize;
    while let Some(source) = stack.pop() {
        visited += 1;
        for destination in &adjacency[source] {
            indegree[*destination] -= 1;
            if indegree[*destination] == 0 {
                stack.push(*destination);
            }
        }
    }
    if visited == nodes.len() {
        Ok(())
    } else {
        Err(FlowError::Cycle {
            invariant: "INV-FLOW-003",
        })
    }
}

fn record_arc(
    index: &BTreeMap<NodeKey, usize>,
    adjacency: &mut [Vec<usize>],
    indegree: &mut [usize],
    from: &NodeKey,
    to: &NodeKey,
) -> FlowResult<()> {
    let source = index[from];
    let destination = index[to];
    adjacency[source].push(destination);
    indegree[destination] =
        indegree[destination]
            .checked_add(1)
            .ok_or(FlowError::ArithmeticOverflow {
                field: "node indegree",
                invariant: "INV-FLOW-003",
            })?;
    Ok(())
}
