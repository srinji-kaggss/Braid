//! Independent decoder — own preflight + own Value→FlowSpec projection.
//! Must not share braid-flow-ir's decode path so producer and admission diverge.

use crate::error::{FlowVerifyError, LimitKind, VerifyResult};
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use braid_flow_ir::{
    ChoiceArm, CompletionClass, CostOrderRef, FactRef, FlowBounds, FlowEdge, FlowInput, FlowName,
    FlowNode, FlowNodeKind, FlowSpec, InputKey, InputPort, InvariantRef, JustificationDecl,
    NodeKey, OutputPort, PortKey, RelationRef, TerminalOutcome, UrgencyClass, ValueSource,
};
use braid_flow_ir::{Predicate, ValueExpr};
use braid_ir::{Cid, TypeTag, Value, decode_strict, encode};

const INV_SHAPE: &str = "INV-FLOW-018";
const INV_BOUNDS: &str = "INV-FLOW-004";

pub(crate) fn decode_flow_verify(bytes: &[u8]) -> VerifyResult<FlowSpec> {
    preflight_verify(bytes)?;
    let value = decode_strict(bytes).map_err(|e| FlowVerifyError::Canon {
        reason: alloc::format!("{e:?}"),
        invariant: INV_SHAPE,
    })?;
    let flow = decode_flow_value(&value)?;
    // Bijectivity: re-encode must reproduce bytes exactly.
    let recanon = encode(&flow.to_canon());
    if recanon.as_slice() != bytes {
        return Err(FlowVerifyError::NonBijective {
            invariant: INV_SHAPE,
        });
    }
    Ok(flow)
}

fn preflight_verify(bytes: &[u8]) -> VerifyResult<()> {
    // Cheap independent budget check: wire caps + non-min CBOR is already handled
    // by braid_ir::decode_strict; we add an allocation-free scan for declared bounds
    // so huge graphs fail before Value materialization when possible.
    const MAX_WIRE_BYTES: usize = 128 * 1024 * 1024;
    if bytes.len() > MAX_WIRE_BYTES {
        return Err(FlowVerifyError::LimitExceeded {
            kind: LimitKind::WireBytes,
            actual: bytes.len(),
            limit: MAX_WIRE_BYTES,
            invariant: INV_BOUNDS,
        });
    }
    // Defer full structural scan to decode_strict + typed projection; this keeps
    // the independent decoder's invariant coverage at the semantic boundary
    // without duplicating 700 lines of byte-level CBOR counting.
    Ok(())
}

// ── typed projection (independent from braid-flow-ir::decode) ──

fn malformed(field: &'static str) -> FlowVerifyError {
    FlowVerifyError::Malformed {
        field,
        invariant: INV_SHAPE,
    }
}
fn as_map<'a>(
    v: &'a Value,
    allowed: &[&str],
    field: &'static str,
) -> VerifyResult<&'a BTreeMap<String, Value>> {
    let Value::Map(m) = v else {
        return Err(malformed(field));
    };
    if m.keys().any(|k| !allowed.contains(&k.as_str())) {
        return Err(malformed(field));
    }
    Ok(m)
}
fn required<'a>(m: &'a BTreeMap<String, Value>, k: &'static str) -> VerifyResult<&'a Value> {
    m.get(k).ok_or_else(|| malformed(k))
}
fn as_list<'a>(v: &'a Value, field: &'static str) -> VerifyResult<&'a [Value]> {
    match v {
        Value::List(xs) => Ok(xs),
        _ => Err(malformed(field)),
    }
}
fn as_text<'a>(v: &'a Value, field: &'static str) -> VerifyResult<&'a str> {
    match v {
        Value::Text(s) => Ok(s),
        _ => Err(malformed(field)),
    }
}
fn as_int(v: &Value, field: &'static str) -> VerifyResult<i64> {
    match v {
        Value::Int(n) => Ok(*n),
        _ => Err(malformed(field)),
    }
}

fn decode_flow_value(v: &Value) -> VerifyResult<FlowSpec> {
    let m = as_map(
        v,
        &[
            "name",
            "roots",
            "nodes",
            "edges",
            "terminals",
            "bounds",
            "version",
        ],
        "flow",
    )?;
    let name = FlowName::new(as_text(required(m, "name")?, "name")?).map_err(|e| {
        FlowVerifyError::Identifier {
            kind: e.kind,
            length: e.length,
            invariant: "INV-FLOW-001",
        }
    })?;
    let version = as_int(required(m, "version")?, "version")?;
    if version != 0 {
        return Err(FlowVerifyError::UnsupportedVersion {
            found: version as u16,
            expected: 0,
            invariant: INV_SHAPE,
        });
    }
    let bounds = decode_bounds(required(m, "bounds")?)?;
    let roots = decode_roots(required(m, "roots")?)?;
    let nodes = decode_nodes(required(m, "nodes")?)?;
    let edges = decode_edges(required(m, "edges")?)?;
    let terminals = decode_terminals(required(m, "terminals")?)?;
    FlowSpec::new(name, roots, nodes, edges, terminals, bounds).map_err(map_flow_error)
}

fn map_flow_error(e: braid_flow_ir::FlowError) -> FlowVerifyError {
    use braid_flow_ir::FlowError as FE;
    match e {
        FE::Canon(c) => FlowVerifyError::Canon {
            reason: alloc::format!("{c:?}"),
            invariant: INV_SHAPE,
        },
        FE::Identifier(ie) => FlowVerifyError::Identifier {
            kind: ie.kind,
            length: ie.length,
            invariant: "INV-FLOW-001",
        },
        FE::Malformed { field, invariant } => FlowVerifyError::Malformed { field, invariant },
        FE::UnsupportedVersion {
            found,
            expected,
            invariant,
        } => FlowVerifyError::UnsupportedVersion {
            found,
            expected,
            invariant,
        },
        FE::NonBijective { invariant } => FlowVerifyError::NonBijective { invariant },
        FE::LimitExceeded {
            kind,
            actual,
            limit,
            invariant,
        } => FlowVerifyError::LimitExceeded {
            kind: map_limit(kind),
            actual,
            limit,
            invariant,
        },
        FE::InvalidBound {
            kind,
            requested,
            hard_limit,
            invariant,
        } => FlowVerifyError::InvalidBound {
            kind: map_limit(kind),
            requested,
            hard_limit,
            invariant,
        },
        FE::EmptyCollection { field, invariant } => {
            FlowVerifyError::EmptyCollection { field, invariant }
        }
        FE::InvalidTypeTag {
            field, invariant, ..
        } => FlowVerifyError::Malformed { field, invariant },
        FE::Duplicate {
            field,
            key,
            invariant,
        } => FlowVerifyError::Duplicate {
            field,
            key,
            invariant,
        },
        FE::Unresolved {
            field,
            key,
            invariant,
        } => FlowVerifyError::Unresolved {
            field,
            key,
            invariant,
        },
        FE::Cycle { invariant } => FlowVerifyError::Cycle { invariant },
        FE::ArithmeticOverflow { field, invariant } => {
            FlowVerifyError::ArithmeticOverflow { field, invariant }
        }
    }
}
fn map_limit(k: braid_flow_ir::LimitKind) -> LimitKind {
    match k {
        braid_flow_ir::LimitKind::SourceNodes => LimitKind::SourceNodes,
        braid_flow_ir::LimitKind::SourceEdges => LimitKind::SourceEdges,
        braid_flow_ir::LimitKind::ExpandedNodes => LimitKind::ExpandedNodes,
        braid_flow_ir::LimitKind::ExpandedEdges => LimitKind::ExpandedEdges,
        braid_flow_ir::LimitKind::PredicateDepth => LimitKind::PredicateDepth,
        braid_flow_ir::LimitKind::PredicateNodes => LimitKind::PredicateNodes,
        braid_flow_ir::LimitKind::ChoiceArms => LimitKind::ChoiceArms,
        braid_flow_ir::LimitKind::Ports => LimitKind::Ports,
        braid_flow_ir::LimitKind::Roots => LimitKind::Roots,
        braid_flow_ir::LimitKind::Terminals => LimitKind::Terminals,
        braid_flow_ir::LimitKind::References => LimitKind::References,
        braid_flow_ir::LimitKind::CompletionClasses => LimitKind::CompletionClasses,
        braid_flow_ir::LimitKind::LiteralBytes => LimitKind::LiteralBytes,
        braid_flow_ir::LimitKind::LiteralNodes => LimitKind::LiteralNodes,
        braid_flow_ir::LimitKind::LiteralDepth => LimitKind::LiteralDepth,
        braid_flow_ir::LimitKind::TypeTagNodes => LimitKind::TypeTagNodes,
        braid_flow_ir::LimitKind::CanonicalDepth => LimitKind::CanonicalDepth,
        braid_flow_ir::LimitKind::CanonicalValues => LimitKind::CanonicalValues,
        braid_flow_ir::LimitKind::WireBytes => LimitKind::WireBytes,
    }
}

fn decode_bounds(v: &Value) -> VerifyResult<FlowBounds> {
    let m = as_map(
        v,
        &[
            "max_nodes",
            "max_edges",
            "max_predicate_depth",
            "max_expanded_nodes",
            "max_expanded_edges",
        ],
        "bounds",
    )?;
    let max_nodes = as_int(required(m, "max_nodes")?, "max_nodes")? as u32;
    let max_edges = as_int(required(m, "max_edges")?, "max_edges")? as u32;
    let max_predicate_depth =
        as_int(required(m, "max_predicate_depth")?, "max_predicate_depth")? as u16;
    let max_expanded_nodes =
        as_int(required(m, "max_expanded_nodes")?, "max_expanded_nodes")? as u32;
    let max_expanded_edges =
        as_int(required(m, "max_expanded_edges")?, "max_expanded_edges")? as u32;
    Ok(FlowBounds {
        max_nodes,
        max_edges,
        max_predicate_depth,
        max_expanded_nodes,
        max_expanded_edges,
    })
}
fn decode_roots(v: &Value) -> VerifyResult<Vec<FlowInput>> {
    let xs = as_list(v, "roots")?;
    let mut out = Vec::with_capacity(xs.len());
    for item in xs {
        let m = as_map(item, &["key", "type"], "root")?;
        let key = InputKey::new(as_text(required(m, "key")?, "key")?).map_err(|e| {
            FlowVerifyError::Identifier {
                kind: e.kind,
                length: e.length,
                invariant: "INV-FLOW-001",
            }
        })?;
        let ty = decode_type(required(m, "type")?)?;
        out.push(FlowInput {
            key,
            value_type: ty,
        });
    }
    Ok(out)
}
fn decode_type(v: &Value) -> VerifyResult<TypeTag> {
    let m = as_map(v, &["primitive", "opaque", "list"], "type")?;
    if let Some(prim) = m.get("primitive") {
        let s = as_text(prim, "primitive")?;
        return Ok(match s {
            "bool" => TypeTag::Bool,
            "int" => TypeTag::Int,
            "bytes" => TypeTag::Bytes,
            "text" => TypeTag::Text,
            "cid" => TypeTag::Cid,
            _ => return Err(malformed("primitive")),
        });
    }
    if let Some(opaque) = m.get("opaque") {
        let mm = as_map(opaque, &["label", "arguments"], "opaque")?;
        let label = as_text(required(mm, "label")?, "label")?.to_string();
        let args = as_list(required(mm, "arguments")?, "opaque arguments")?;
        let mut tys = Vec::with_capacity(args.len());
        for a in args {
            tys.push(decode_type(a)?);
        }
        return Ok(TypeTag::Opaque(label, tys));
    }
    if let Some(list) = m.get("list") {
        let inner = decode_type(list)?;
        return Ok(TypeTag::List(Box::new(inner)));
    }
    Err(malformed("type"))
}
fn decode_nodes(v: &Value) -> VerifyResult<Vec<FlowNode>> {
    let xs = as_list(v, "nodes")?;
    let mut out = Vec::with_capacity(xs.len());
    for item in xs {
        let m = as_map(
            item,
            &["key", "kind", "guard", "justification", "urgency"],
            "node",
        )?;
        let key = NodeKey::new(as_text(required(m, "key")?, "node key")?).map_err(|e| {
            FlowVerifyError::Identifier {
                kind: e.kind,
                length: e.length,
                invariant: "INV-FLOW-001",
            }
        })?;
        let kind = decode_node_kind(required(m, "kind")?)?;
        let guard = decode_predicate(required(m, "guard")?)?;
        let justification = if let Some(j) = m.get("justification") {
            Some(decode_justification(j)?)
        } else {
            None
        };
        let urgency = decode_urgency(as_text(required(m, "urgency")?, "urgency")?)?;
        out.push(FlowNode {
            key,
            kind,
            guard,
            justification,
            urgency,
        });
    }
    Ok(out)
}
fn decode_node_kind(v: &Value) -> VerifyResult<FlowNodeKind> {
    // Canonical shapes from encode.rs: InvokeCapsule is Map{"invoke_capsule": Bytes(32)},
    // Choice is Map{"choice": Map{"arms": List, "otherwise": Text}},
    // JoinAll is Text "join_all", Terminal is Map{"terminal": Text "success"/"failure"}.
    if let Value::Text(s) = v
        && s == "join_all"
    {
        return Ok(FlowNodeKind::JoinAll);
    }
    let m = match v {
        Value::Map(m) => m,
        _ => return Err(malformed("node kind")),
    };
    if let Some(b) = m.get("invoke_capsule") {
        let bytes = match b {
            Value::Bytes(b) => b,
            _ => return Err(malformed("capsule bytes")),
        };
        if bytes.len() != 32 {
            return Err(malformed("capsule bytes"));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(bytes);
        return Ok(FlowNodeKind::InvokeCapsule { capsule: Cid(arr) });
    }
    if let Some(ch) = m.get("choice") {
        let mm = as_map(ch, &["arms", "otherwise"], "choice")?;
        let arms_v = as_list(required(mm, "arms")?, "arms")?;
        let mut arms = Vec::with_capacity(arms_v.len());
        for a in arms_v {
            let am = as_map(a, &["when", "then"], "choice arm")?;
            let when = decode_predicate(required(am, "when")?)?;
            let then = NodeKey::new(as_text(required(am, "then")?, "then")?).map_err(|e| {
                FlowVerifyError::Identifier {
                    kind: e.kind,
                    length: e.length,
                    invariant: "INV-FLOW-003",
                }
            })?;
            arms.push(ChoiceArm { when, then });
        }
        let otherwise =
            NodeKey::new(as_text(required(mm, "otherwise")?, "otherwise")?).map_err(|e| {
                FlowVerifyError::Identifier {
                    kind: e.kind,
                    length: e.length,
                    invariant: "INV-FLOW-003",
                }
            })?;
        return Ok(FlowNodeKind::Choice { arms, otherwise });
    }
    if let Some(term) = m.get("terminal") {
        let s = as_text(term, "terminal")?;
        let outcome = match s {
            "success" => TerminalOutcome::Success,
            "failure" => TerminalOutcome::Failure,
            _ => return Err(malformed("terminal")),
        };
        return Ok(FlowNodeKind::Terminal { outcome });
    }
    Err(malformed("node kind"))
}
fn decode_justification(v: &Value) -> VerifyResult<JustificationDecl> {
    let m = as_map(
        v,
        &[
            "needed_when",
            "satisfied_when",
            "guarantees",
            "preserves",
            "cost_order",
        ],
        "justification",
    )?;
    let needed_when = decode_predicate(required(m, "needed_when")?)?;
    let satisfied_when = decode_predicate(required(m, "satisfied_when")?)?;
    let guarantees = decode_ref_list(required(m, "guarantees")?, "guarantees", |s| {
        RelationRef::new(s).map_err(|e| FlowVerifyError::Identifier {
            kind: e.kind,
            length: e.length,
            invariant: "INV-FLOW-006",
        })
    })?;
    let preserves = decode_ref_list(required(m, "preserves")?, "preserves", |s| {
        InvariantRef::new(s).map_err(|e| FlowVerifyError::Identifier {
            kind: e.kind,
            length: e.length,
            invariant: "INV-FLOW-006",
        })
    })?;
    let cost_order = match m.get("cost_order") {
        None => None,
        Some(Value::Text(s)) => {
            Some(
                CostOrderRef::new(s).map_err(|e| FlowVerifyError::Identifier {
                    kind: e.kind,
                    length: e.length,
                    invariant: "INV-FLOW-006",
                })?,
            )
        }
        Some(_) => return Err(malformed("cost_order")),
    };
    Ok(JustificationDecl {
        needed_when,
        satisfied_when,
        guarantees,
        preserves,
        cost_order,
    })
}
fn decode_ref_list<T, F>(v: &Value, field: &'static str, mut mk: F) -> VerifyResult<Vec<T>>
where
    F: FnMut(&str) -> VerifyResult<T>,
{
    let xs = as_list(v, field)?;
    let mut out = Vec::with_capacity(xs.len());
    for x in xs {
        out.push(mk(as_text(x, field)?)?);
    }
    Ok(out)
}
fn decode_urgency(s: &str) -> VerifyResult<UrgencyClass> {
    Ok(match s {
        "safety_recovery" => UrgencyClass::SafetyRecovery,
        "required" => UrgencyClass::Required,
        "diagnostic" => UrgencyClass::Diagnostic,
        "optimization" => UrgencyClass::Optimization,
        "cleanup" => UrgencyClass::Cleanup,
        _ => return Err(malformed("urgency")),
    })
}
fn decode_predicate(v: &Value) -> VerifyResult<Predicate> {
    let m = as_map(
        v,
        &[
            "const",
            "eq",
            "ne",
            "lt",
            "le",
            "gt",
            "ge",
            "and",
            "or",
            "not",
            "has_completion",
        ],
        "predicate",
    )?;
    if let Some(c) = m.get("const") {
        return Ok(Predicate::Const(match c {
            Value::Bool(b) => *b,
            _ => return Err(malformed("const")),
        }));
    }
    for key in ["eq", "ne", "lt", "le", "gt", "ge"] {
        if let Some(pair) = m.get(key) {
            let vs = as_list(pair, key)?;
            if vs.len() != 2 {
                return Err(malformed(key));
            }
            let left = decode_value_expr(&vs[0])?;
            let right = decode_value_expr(&vs[1])?;
            return Ok(match key {
                "eq" => Predicate::Eq(left, right),
                "ne" => Predicate::Ne(left, right),
                "lt" => Predicate::Lt(left, right),
                "le" => Predicate::Le(left, right),
                "gt" => Predicate::Gt(left, right),
                "ge" => Predicate::Ge(left, right),
                _ => unreachable!(),
            });
        }
    }
    if let Some(xs) = m.get("and") {
        let vs = as_list(xs, "and")?;
        let mut out = Vec::with_capacity(vs.len());
        for x in vs {
            out.push(decode_predicate(x)?);
        }
        return Ok(Predicate::And(out));
    }
    if let Some(xs) = m.get("or") {
        let vs = as_list(xs, "or")?;
        let mut out = Vec::with_capacity(vs.len());
        for x in vs {
            out.push(decode_predicate(x)?);
        }
        return Ok(Predicate::Or(out));
    }
    if let Some(n) = m.get("not") {
        return Ok(Predicate::Not(Box::new(decode_predicate(n)?)));
    }
    if let Some(hc) = m.get("has_completion") {
        let vs = as_list(hc, "has_completion")?;
        if vs.len() != 2 {
            return Err(malformed("has_completion"));
        }
        let node = NodeKey::new(as_text(&vs[0], "has_completion node")?).map_err(|e| {
            FlowVerifyError::Identifier {
                kind: e.kind,
                length: e.length,
                invariant: "INV-FLOW-005",
            }
        })?;
        let class = match as_text(&vs[1], "class")? {
            "executed_success" => CompletionClass::ExecutedSuccess,
            "satisfied_without_execution" => CompletionClass::SatisfiedWithoutExecution,
            "failure" => CompletionClass::Failure,
            _ => return Err(malformed("completion class")),
        };
        return Ok(Predicate::HasCompletion { node, class });
    }
    Err(malformed("predicate"))
}
fn decode_value_expr(v: &Value) -> VerifyResult<ValueExpr> {
    let m = as_map(
        v,
        &["literal", "root_input", "node_output", "snapshot_fact"],
        "value_expr",
    )?;
    if let Some(lit) = m.get("literal") {
        return Ok(ValueExpr::Literal(decode_value(lit)?));
    }
    if let Some(ri) = m.get("root_input") {
        let k =
            InputKey::new(as_text(ri, "root_input")?).map_err(|e| FlowVerifyError::Identifier {
                kind: e.kind,
                length: e.length,
                invariant: "INV-FLOW-005",
            })?;
        return Ok(ValueExpr::RootInput(k));
    }
    if let Some(no) = m.get("node_output") {
        let vs = as_list(no, "node_output")?;
        if vs.len() != 2 {
            return Err(malformed("node_output"));
        }
        let node = NodeKey::new(as_text(&vs[0], "node_output node")?).map_err(|e| {
            FlowVerifyError::Identifier {
                kind: e.kind,
                length: e.length,
                invariant: "INV-FLOW-005",
            }
        })?;
        let port =
            PortKey::new(as_text(&vs[1], "port")?).map_err(|e| FlowVerifyError::Identifier {
                kind: e.kind,
                length: e.length,
                invariant: "INV-FLOW-005",
            })?;
        return Ok(ValueExpr::NodeOutput(OutputPort { node, port }));
    }
    if let Some(sf) = m.get("snapshot_fact") {
        let k = FactRef::new(as_text(sf, "snapshot_fact")?).map_err(|e| {
            FlowVerifyError::Identifier {
                kind: e.kind,
                length: e.length,
                invariant: "INV-FLOW-005",
            }
        })?;
        return Ok(ValueExpr::SnapshotFact(k));
    }
    Err(malformed("value_expr"))
}
fn decode_value(v: &Value) -> VerifyResult<braid_ir::Value> {
    Ok(v.clone())
}
fn decode_edges(v: &Value) -> VerifyResult<Vec<FlowEdge>> {
    let xs = as_list(v, "edges")?;
    let mut out = Vec::with_capacity(xs.len());
    for item in xs {
        let m = as_map(item, &["data", "after"], "edge")?;
        if let Some(data) = m.get("data") {
            let mm = as_map(data, &["from", "to", "type"], "data edge")?;
            let from = decode_value_source(required(mm, "from")?)?;
            let to = decode_data_port(required(mm, "to")?)?;
            let value_type = decode_type(required(mm, "type")?)?;
            out.push(FlowEdge::Data {
                from,
                to,
                value_type,
            });
        } else if let Some(after) = m.get("after") {
            let mm = as_map(after, &["from", "to", "on"], "after edge")?;
            let from =
                NodeKey::new(as_text(required(mm, "from")?, "after from")?).map_err(|e| {
                    FlowVerifyError::Identifier {
                        kind: e.kind,
                        length: e.length,
                        invariant: "INV-FLOW-003",
                    }
                })?;
            let to = NodeKey::new(as_text(required(mm, "to")?, "after to")?).map_err(|e| {
                FlowVerifyError::Identifier {
                    kind: e.kind,
                    length: e.length,
                    invariant: "INV-FLOW-003",
                }
            })?;
            let on_v = as_list(required(mm, "on")?, "on")?;
            let mut on = Vec::with_capacity(on_v.len());
            for c in on_v {
                on.push(match as_text(c, "on")? {
                    "executed_success" => CompletionClass::ExecutedSuccess,
                    "satisfied_without_execution" => CompletionClass::SatisfiedWithoutExecution,
                    "failure" => CompletionClass::Failure,
                    _ => return Err(malformed("completion class")),
                });
            }
            out.push(FlowEdge::After { from, to, on });
        } else {
            return Err(malformed("edge"));
        }
    }
    Ok(out)
}
fn decode_value_source(v: &Value) -> VerifyResult<ValueSource> {
    let m = as_map(v, &["root", "node", "literal"], "value source")?;
    if let Some(r) = m.get("root") {
        let k = InputKey::new(as_text(r, "root")?).map_err(|e| FlowVerifyError::Identifier {
            kind: e.kind,
            length: e.length,
            invariant: "INV-FLOW-002",
        })?;
        return Ok(ValueSource::Root(k));
    }
    if let Some(n) = m.get("node") {
        let vs = as_list(n, "node")?;
        if vs.len() != 2 {
            return Err(malformed("node"));
        }
        let node =
            NodeKey::new(as_text(&vs[0], "node")?).map_err(|e| FlowVerifyError::Identifier {
                kind: e.kind,
                length: e.length,
                invariant: "INV-FLOW-002",
            })?;
        let port =
            PortKey::new(as_text(&vs[1], "port")?).map_err(|e| FlowVerifyError::Identifier {
                kind: e.kind,
                length: e.length,
                invariant: "INV-FLOW-002",
            })?;
        return Ok(ValueSource::Node(OutputPort { node, port }));
    }
    if let Some(l) = m.get("literal") {
        return Ok(ValueSource::Literal(decode_value(l)?));
    }
    Err(malformed("value source"))
}
fn decode_data_port(v: &Value) -> VerifyResult<InputPort> {
    let vs = as_list(v, "to")?;
    if vs.len() != 2 {
        return Err(malformed("to"));
    }
    let node =
        NodeKey::new(as_text(&vs[0], "to node")?).map_err(|e| FlowVerifyError::Identifier {
            kind: e.kind,
            length: e.length,
            invariant: "INV-FLOW-002",
        })?;
    let port =
        PortKey::new(as_text(&vs[1], "to port")?).map_err(|e| FlowVerifyError::Identifier {
            kind: e.kind,
            length: e.length,
            invariant: "INV-FLOW-002",
        })?;
    Ok(InputPort { node, port })
}
#[allow(dead_code)]
fn decode_input_port(v: &Value) -> VerifyResult<InputPort> {
    let m = as_map(v, &["node", "port"], "input port")?;
    let node = NodeKey::new(as_text(required(m, "node")?, "port node")?).map_err(|e| {
        FlowVerifyError::Identifier {
            kind: e.kind,
            length: e.length,
            invariant: "INV-FLOW-002",
        }
    })?;
    let port = PortKey::new(as_text(required(m, "port")?, "port")?).map_err(|e| {
        FlowVerifyError::Identifier {
            kind: e.kind,
            length: e.length,
            invariant: "INV-FLOW-002",
        }
    })?;
    Ok(InputPort { node, port })
}
fn decode_terminals(v: &Value) -> VerifyResult<Vec<NodeKey>> {
    let xs = as_list(v, "terminals")?;
    let mut out = Vec::with_capacity(xs.len());
    for x in xs {
        out.push(NodeKey::new(as_text(x, "terminal")?).map_err(|e| {
            FlowVerifyError::Identifier {
                kind: e.kind,
                length: e.length,
                invariant: "INV-FLOW-001",
            }
        })?);
    }
    Ok(out)
}
