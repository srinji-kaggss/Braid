//! Predicate evaluator over a `FlowSnapshot` — three-valued, total,
//! deterministic, side-effect-free (INV-FLOW-005, INV-FLOW-007).

use braid_flow_ir::{CompletionClass, Predicate, ValueExpr};
use braid_ir::Value;

use crate::snapshot::{FlowSnapshot, MissingEvidence, ProofState};

/// Closed completion state for planned nodes inside this planner — no shared
/// kernel authority is minted (D10, ADR-099).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Pending,
    ExecutedSuccess,
    SatisfiedWithoutExecution,
    Failure,
}

impl CompletionKind {
    fn from_class(c: CompletionClass) -> CompletionKind {
        match c {
            CompletionClass::ExecutedSuccess => CompletionKind::ExecutedSuccess,
            CompletionClass::SatisfiedWithoutExecution => CompletionKind::SatisfiedWithoutExecution,
            CompletionClass::Failure => CompletionKind::Failure,
        }
    }
}

/// What the planner already knows about each node's completion before this
/// step. `Pending` means undecided — predecessor completion drives the ready
/// antichain, not an implicit "not yet run" default.
pub type CompletionMap = alloc::collections::BTreeMap<String, CompletionKind>;

pub fn eval_predicate(
    pred: &Predicate,
    snap: &FlowSnapshot,
    completions: &CompletionMap,
) -> ProofState {
    match pred {
        Predicate::Const(true) => ProofState::Proven,
        Predicate::Const(false) => ProofState::Disproven,
        Predicate::Eq(a, b) => eval_cmp(a, b, snap, |l, r| {
            if l == r {
                ProofState::Proven
            } else {
                ProofState::Disproven
            }
        }),
        Predicate::Ne(a, b) => eval_cmp(a, b, snap, |l, r| {
            if l != r {
                ProofState::Proven
            } else {
                ProofState::Disproven
            }
        }),
        Predicate::Lt(a, b) => eval_cmp(a, b, snap, |l, r| match compare_ordered(l, r) {
            Some(core::cmp::Ordering::Less) => ProofState::Proven,
            Some(_) => ProofState::Disproven,
            None => ProofState::Unknown(MissingEvidence("incomparable types for Lt".into())),
        }),
        Predicate::Le(a, b) => eval_cmp(a, b, snap, |l, r| match compare_ordered(l, r) {
            Some(core::cmp::Ordering::Less) | Some(core::cmp::Ordering::Equal) => {
                ProofState::Proven
            }
            Some(_) => ProofState::Disproven,
            None => ProofState::Unknown(MissingEvidence("incomparable types for Le".into())),
        }),
        Predicate::Gt(a, b) => eval_cmp(a, b, snap, |l, r| match compare_ordered(l, r) {
            Some(core::cmp::Ordering::Greater) => ProofState::Proven,
            Some(_) => ProofState::Disproven,
            None => ProofState::Unknown(MissingEvidence("incomparable types for Gt".into())),
        }),
        Predicate::Ge(a, b) => eval_cmp(a, b, snap, |l, r| match compare_ordered(l, r) {
            Some(core::cmp::Ordering::Greater) | Some(core::cmp::Ordering::Equal) => {
                ProofState::Proven
            }
            Some(_) => ProofState::Disproven,
            None => ProofState::Unknown(MissingEvidence("incomparable types for Ge".into())),
        }),
        Predicate::And(items) => eval_and(items, snap, completions),
        Predicate::Or(items) => eval_or(items, snap, completions),
        Predicate::Not(inner) => eval_not(inner, snap, completions),
        Predicate::HasCompletion { node, class } => eval_has_completion(node, *class, completions),
    }
}

fn eval_cmp<F>(left: &ValueExpr, right: &ValueExpr, snap: &FlowSnapshot, cmp: F) -> ProofState
where
    F: Fn(&Value, &Value) -> ProofState,
{
    let l = resolve_value_expr(left, snap);
    let r = resolve_value_expr(right, snap);
    match (l, r) {
        (Ok(lv), Ok(rv)) => cmp(&lv, &rv),
        (Err(m), _) | (_, Err(m)) => ProofState::Unknown(m),
    }
}

fn compare_ordered(left: &Value, right: &Value) -> Option<core::cmp::Ordering> {
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => Some(a.cmp(b)),
        (Value::Bool(a), Value::Bool(b)) => Some(a.cmp(b)),
        (Value::Text(a), Value::Text(b)) => Some(a.cmp(b)),
        (Value::Bytes(a), Value::Bytes(b)) => Some(a.cmp(b)),
        _ => None,
    }
}

fn resolve_value_expr(expr: &ValueExpr, snap: &FlowSnapshot) -> Result<Value, MissingEvidence> {
    match expr {
        ValueExpr::Literal(v) => Ok(v.clone()),
        ValueExpr::SnapshotFact(f) => snap
            .get(f)
            .cloned()
            .ok_or_else(|| MissingEvidence(alloc::format!("missing fact {}", f))),
        // RootInput and NodeOutput have no binding inside the snapshot-bound
        // predicate evaluator — a future Logical-DB binding would close them.
        // For v0 they remain Unknown so the planner fails closed rather than
        // inventing data outputs (INV-FLOW-025).
        ValueExpr::RootInput(k) => Err(MissingEvidence(alloc::format!(
            "root input {} not bound in snapshot evaluator",
            k
        ))),
        ValueExpr::NodeOutput(o) => Err(MissingEvidence(alloc::format!(
            "node output {}.{} not bound in snapshot evaluator",
            o.node,
            o.port
        ))),
    }
}

fn eval_and(items: &[Predicate], snap: &FlowSnapshot, completions: &CompletionMap) -> ProofState {
    let mut saw_unknown: Option<MissingEvidence> = None;
    for p in items {
        match eval_predicate(p, snap, completions) {
            ProofState::Disproven => return ProofState::Disproven,
            ProofState::Unknown(m) => {
                if saw_unknown.is_none() {
                    saw_unknown = Some(m);
                }
            }
            ProofState::Proven => {}
        }
    }
    match saw_unknown {
        Some(m) => ProofState::Unknown(m),
        None => ProofState::Proven,
    }
}

fn eval_or(items: &[Predicate], snap: &FlowSnapshot, completions: &CompletionMap) -> ProofState {
    let mut saw_unknown: Option<MissingEvidence> = None;
    for p in items {
        match eval_predicate(p, snap, completions) {
            ProofState::Proven => return ProofState::Proven,
            ProofState::Unknown(m) => {
                if saw_unknown.is_none() {
                    saw_unknown = Some(m);
                }
            }
            ProofState::Disproven => {}
        }
    }
    match saw_unknown {
        Some(m) => ProofState::Unknown(m),
        None => ProofState::Disproven,
    }
}

fn eval_not(inner: &Predicate, snap: &FlowSnapshot, completions: &CompletionMap) -> ProofState {
    match eval_predicate(inner, snap, completions) {
        ProofState::Proven => ProofState::Disproven,
        ProofState::Disproven => ProofState::Proven,
        ProofState::Unknown(m) => ProofState::Unknown(m),
    }
}

fn eval_has_completion(
    node: &braid_flow_ir::NodeKey,
    class: CompletionClass,
    completions: &CompletionMap,
) -> ProofState {
    let key = node.to_string();
    match completions.get(&key) {
        None => ProofState::Unknown(MissingEvidence(alloc::format!(
            "completion of {} unknown",
            key
        ))),
        Some(got) => {
            if *got == CompletionKind::from_class(class) {
                ProofState::Proven
            } else {
                ProofState::Disproven
            }
        }
    }
}
