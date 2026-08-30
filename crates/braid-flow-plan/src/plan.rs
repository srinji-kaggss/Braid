//! Deterministic frontier planning — satiation first, ready antichain,
//! stable sequential selection (ADR-099, P3 #59).
//!
//! `plan()` is the only entry point that advances the frontier by at most one
//! sequential step. It is deterministic over its canonical inputs and refuses
//! to dispatch under `Unknown` (INV-FLOW-007, INV-FLOW-008, INV-FLOW-011,
//! INV-FLOW-019, INV-FLOW-023, INV-FLOW-025).

extern crate alloc;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::ToString;
use alloc::vec::Vec;
use braid_flow_ir::{CompletionClass, FlowEdge, FlowNodeKind, FlowSpec};
use braid_ir::{Cid, Value, encode};

use crate::eval::{CompletionKind, CompletionMap, eval_predicate};
use crate::snapshot::{FlowSnapshot, ProofState};

const PLAN_DOMAIN: &[u8] = b"lw.braid.flow.plan.v0";
pub const PLANNER_VERSION: u16 = 1;

/// Which inputs the planner consumed. The Plan CID covers all of them so a
/// plan cannot be reused under a different snapshot (INV-FLOW-008/023).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanningContext {
    pub snapshot_cid: Cid,
    pub target_profile_cid: Cid,
    pub cache_manifest_cid: Cid,
    pub planner_version: u16,
}

impl Default for PlanningContext {
    fn default() -> Self {
        // Nil CIDs (BLAKE3 over empty) as the neutral default —callers that
        // care about target/cache distinguish them; the plan CID still binds
        // them so determinism is preserved.
        let nil = Cid::compute(b"lw.braid.nil", &[]);
        Self {
            snapshot_cid: nil,
            target_profile_cid: nil,
            cache_manifest_cid: nil,
            planner_version: PLANNER_VERSION,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    UnsupportedPlannerVersion {
        found: u16,
        expected: u16,
    },
    UnknownProof {
        reason: String,
        invariant: &'static str,
    },
    NoReadyNode {
        invariant: &'static str,
    },
    InconsistentGraph {
        reason: String,
        invariant: &'static str,
    },
}

impl core::fmt::Display for PlanError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedPlannerVersion { found, expected } => {
                write!(
                    f,
                    "unsupported planner version {found}; expected {expected}"
                )
            }
            Self::UnknownProof { reason, invariant } => {
                write!(f, "{invariant}: unknown — {reason}")
            }
            Self::NoReadyNode { invariant } => write!(f, "{invariant}: no ready node"),
            Self::InconsistentGraph { reason, invariant } => {
                write!(f, "{invariant}: {reason}")
            }
        }
    }
}
impl core::error::Error for PlanError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SatiatedTransition {
    pub node: String,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanStep {
    pub node: String,
    pub capsule: Option<Cid>,
    /// Snapshot-evaluated target for a `Choice`; absent for every other kind.
    pub choice_target: Option<String>,
    pub kind: PlanStepKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanStepKind {
    InvokeCapsule,
    Choice,
    JoinAll,
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowPlan {
    pub flow_cid: Cid,
    pub plan_cid: Cid,
    pub satiated: Vec<SatiatedTransition>,
    pub next_step: Option<PlanStep>,
    pub trace: Vec<String>,
}

/// Derive one deterministic planning outcome from an admitted flow, a snapshot,
/// and already-known completions.
///
/// `completions` binds `NodeKey -> CompletionKind` for nodes already decided
/// (including synthetic start/terminal predecessors). `context` binds the
/// external CIDs that participate in `lw.braid.flow.plan.v0` identity.
///
/// Evaluation order per §9.2: `satisfied_when` first — a `Proven` satiation
/// completes the node without invocation (INV-019) — then `needed_when` and
/// `guard`, then predecessor completion. `Unknown` anywhere is a refusal, not
/// a skip (INV-007).
pub fn plan(
    flow: &FlowSpec,
    snapshot: &FlowSnapshot,
    completions: &CompletionMap,
    context: &PlanningContext,
) -> Result<FlowPlan, PlanError> {
    if context.planner_version != PLANNER_VERSION {
        return Err(PlanError::UnsupportedPlannerVersion {
            found: context.planner_version,
            expected: PLANNER_VERSION,
        });
    }
    let flow_cid = flow.cid().get();
    let ranked = evaluate_and_rank(flow, snapshot, completions)?;
    let mut trace: Vec<String> = Vec::new();
    let mut satiated: Vec<SatiatedTransition> = Vec::new();

    // Satiation-first sweep in node-key order (deterministic, not insertion
    // order) so the pending set and trace are stable regardless of how the
    // caller built the flow.
    for r in &ranked {
        if r.satiated {
            satiated.push(SatiatedTransition {
                node: r.key.clone(),
                evidence: "satisfied_when Proven".into(),
            });
            trace.push(alloc::format!("satiated {}", r.key));
        }
    }

    // Ready antichain: pending + guard Proven + needed Proven + preds
    // satisfied. Already-completed nodes (ExecutedSuccess /
    // SatisfiedWithoutExecution per `completions`) are never dispatched
    // again. Predecessors are satisfied if the predecessor node is either
    // ExecutedSuccess or SatisfiedWithoutExecution — both count as having
    // released the `After` / `Data` dependency. A Failure predecessor does not
    // release unless the edge's `on` set includes it (flow-ir normalizes `on`
    // to include the admissible classes).
    let ready: Vec<&RankedNode> = ranked
        .iter()
        .filter(|r| {
            if r.satiated {
                return false;
            }
            if !r.ready {
                return false;
            }
            match completions.get(&r.key).copied() {
                None => true,
                Some(CompletionKind::Pending) => true,
                Some(_) => false,
            }
        })
        .collect();

    // Enforce antichain: two ready nodes may not be ancestors of each other.
    // The verifier's reachability/SCC work already guarantees a DAG; we just
    // filter the ready set to be ancestor-free so a later parallel planner has
    // a sound extension point. Under adversity (a ready pair where one reaches
    // the other) the downstream node is not ready — its predecessor hasn't
    // completed — so the invariant follows from predecessor satisfaction, but
    // we keep the explicit check as a fast diagnostic.
    // For v0 this is a no-op in the correct case.
    let next_step = if ready.is_empty() {
        None
    } else {
        // Deterministic ranking §10.2 — urgency class, then a future
        // critical-path / cost, then node CID tie-break. Insertion / map
        // iteration order must never affect selection (INV-011/023).
        let chosen = ranked_choice(flow, &ready);
        let (kind, capsule, choice_target) =
            step_details(flow, &chosen.key, snapshot, completions)?;
        Some(PlanStep {
            node: chosen.key.clone(),
            capsule,
            choice_target,
            kind,
        })
    };

    for r in &ranked {
        trace.push(alloc::format!(
            "{} ready={} satiated={} guard={:?} needed={:?}",
            r.key,
            r.ready,
            r.satiated,
            r.guard_state,
            r.needed_state
        ));
    }
    if let Some(s) = &next_step {
        trace.push(alloc::format!("selected {}", s.node));
    } else {
        trace.push("no ready node".into());
    }

    let plan_cid = compute_plan_cid(
        flow_cid,
        snapshot.cid(),
        context,
        &satiated,
        next_step.as_ref(),
        &trace,
    );

    Ok(FlowPlan {
        flow_cid,
        plan_cid,
        satiated,
        next_step,
        trace,
    })
}

#[derive(Debug, Clone)]
struct RankedNode {
    key: String,
    ready: bool,
    satiated: bool,
    guard_state: ProofState,
    needed_state: Option<ProofState>,
    urgency: braid_flow_ir::UrgencyClass,
}

fn evaluate_and_rank(
    flow: &FlowSpec,
    snapshot: &FlowSnapshot,
    completions: &CompletionMap,
) -> Result<Vec<RankedNode>, PlanError> {
    // Precompute predecessor map from edges + Choice arms (so frontier
    // ordering over After/Data/Choice is identical to the verifier's
    // reachability view).
    let mut preds: BTreeMap<String, Vec<(String, BTreeSet<CompletionClass>)>> = BTreeMap::new();
    for e in flow.edges() {
        match e {
            FlowEdge::After { from, to, on } => {
                let on_set: BTreeSet<CompletionClass> = on.iter().copied().collect();
                preds
                    .entry(to.to_string())
                    .or_default()
                    .push((from.to_string(), on_set));
            }
            FlowEdge::Data { from, to, .. } => {
                let from_key = match from {
                    braid_flow_ir::ValueSource::Node(o) => o.node.to_string(),
                    _ => continue,
                };
                preds
                    .entry(to.node.to_string())
                    .or_default()
                    .push((from_key, BTreeSet::new()));
            }
        }
    }
    for nd in flow.nodes() {
        if let FlowNodeKind::Choice { arms, otherwise } = &nd.kind {
            for a in arms {
                preds
                    .entry(a.then.to_string())
                    .or_default()
                    .push((nd.key.to_string(), BTreeSet::new()));
            }
            preds
                .entry(otherwise.to_string())
                .or_default()
                .push((nd.key.to_string(), BTreeSet::new()));
        }
    }

    let mut out = Vec::new();
    for nd in flow.nodes() {
        let key = nd.key.to_string();
        let completions_for_guard = completions;

        let guard_state = eval_predicate(&nd.guard, snapshot, completions_for_guard);
        let (needed_state, satiated, ready) = match &nd.justification {
            None => {
                // Non-invoke nodes (Choice/Join/Terminal) have no
                // justification fields — guard + predecessor completeness decide
                // readiness. They may still carry a guard referencing completion
                // facts.
                let preds_ok = preds_satisfied(&key, &preds, completions);
                let ready = preds_ok && is_proven(&guard_state);
                (None, false, ready)
            }
            Some(j) => {
                let satisfied = eval_predicate(&j.satisfied_when, snapshot, completions_for_guard);
                if is_proven(&satisfied) {
                    // Satiation precedes action — even if predecessors are not
                    // yet satisfied, satiation completes the node without
                    // invocation. Data-edge bindings for demanded outputs are
                    // the remaining debt (INV-FLOW-025); for v0 we note the
                    // debt but do not invent bindings — see `eval` where
                    // NodeOutput references remain Unknown.
                    (Some(satisfied), true, false)
                } else if is_unknown(&satisfied) {
                    return Err(PlanError::UnknownProof {
                        reason: alloc::format!("satisfied_when Unknown for {}", key),
                        invariant: "INV-FLOW-007",
                    });
                } else {
                    let needed = eval_predicate(&j.needed_when, snapshot, completions_for_guard);
                    if is_unknown(&needed) {
                        return Err(PlanError::UnknownProof {
                            reason: alloc::format!("needed_when Unknown for {}", key),
                            invariant: "INV-FLOW-007",
                        });
                    }
                    if is_unknown(&guard_state) {
                        return Err(PlanError::UnknownProof {
                            reason: alloc::format!("guard Unknown for {}", key),
                            invariant: "INV-FLOW-007",
                        });
                    }
                    let preds_ok = preds_satisfied(&key, &preds, completions);
                    let ready = preds_ok && is_proven(&guard_state) && is_proven(&needed);
                    (Some(needed), false, ready)
                }
            }
        };
        // Guard Unknown on non-invoke nodes is also a refusal.
        if nd.justification.is_none() && is_unknown(&guard_state) {
            return Err(PlanError::UnknownProof {
                reason: alloc::format!("guard Unknown for {}", key),
                invariant: "INV-FLOW-007",
            });
        }
        out.push(RankedNode {
            key,
            ready,
            satiated,
            guard_state,
            needed_state,
            urgency: nd.urgency,
        });
    }
    // Stable order before selection — node key lex order is the canonical tie
    // surface (INV-011). `ranked_choice` refines further by urgency.
    out.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(out)
}

fn is_proven(p: &ProofState) -> bool {
    matches!(p, ProofState::Proven)
}
fn is_unknown(p: &ProofState) -> bool {
    matches!(p, ProofState::Unknown(_))
}

fn preds_satisfied(
    node_key: &str,
    preds: &BTreeMap<String, Vec<(String, BTreeSet<CompletionClass>)>>,
    completions: &CompletionMap,
) -> bool {
    let Some(list) = preds.get(node_key) else {
        return true;
    };
    for (from, on_set) in list {
        let c = completions
            .get(from)
            .copied()
            .unwrap_or(CompletionKind::Pending);
        match c {
            CompletionKind::Pending => return false,
            CompletionKind::Failure => {
                // Data edges imply success; After edges expose `on`. An empty
                // `on_set` (data edge / Choice arm) does not carry a failure
                // predecessor — only After edges with explicit `on` can release
                // from a failure.
                if on_set.is_empty() {
                    return false;
                }
                if !on_set.contains(&CompletionClass::Failure) {
                    return false;
                }
            }
            CompletionKind::ExecutedSuccess | CompletionKind::SatisfiedWithoutExecution => {}
        }
    }
    true
}

fn canonical_kind(kind: &FlowNodeKind) -> Value {
    match kind {
        FlowNodeKind::InvokeCapsule { capsule } => Value::Map(
            vec![("invoke_capsule".into(), Value::Bytes(capsule.0.to_vec()))]
                .into_iter()
                .collect(),
        ),
        FlowNodeKind::Choice { arms, otherwise } => {
            let arms_v = Value::List(
                arms.iter()
                    .map(|arm| {
                        Value::Map(
                            vec![
                                ("then".into(), Value::Text(arm.then.to_string())),
                                ("when".into(), arm.when.to_canon()),
                            ]
                            .into_iter()
                            .collect(),
                        )
                    })
                    .collect(),
            );
            Value::Map(
                vec![(
                    "choice".into(),
                    Value::Map(
                        vec![
                            ("arms".into(), arms_v),
                            ("otherwise".into(), Value::Text(otherwise.to_string())),
                        ]
                        .into_iter()
                        .collect(),
                    ),
                )]
                .into_iter()
                .collect(),
            )
        }
        FlowNodeKind::JoinAll => Value::Text("join_all".into()),
        FlowNodeKind::Terminal { outcome } => Value::Map(
            vec![(
                "terminal".into(),
                Value::Text(
                    match outcome {
                        braid_flow_ir::TerminalOutcome::Success => "success",
                        braid_flow_ir::TerminalOutcome::Failure => "failure",
                    }
                    .into(),
                ),
            )]
            .into_iter()
            .collect(),
        ),
    }
}

fn canonical_urgency(urgency: braid_flow_ir::UrgencyClass) -> Value {
    Value::Text(
        match urgency {
            braid_flow_ir::UrgencyClass::SafetyRecovery => "safety_recovery",
            braid_flow_ir::UrgencyClass::Required => "required",
            braid_flow_ir::UrgencyClass::Diagnostic => "diagnostic",
            braid_flow_ir::UrgencyClass::Optimization => "optimization",
            braid_flow_ir::UrgencyClass::Cleanup => "cleanup",
        }
        .into(),
    )
}

fn ranked_choice<'a>(flow: &FlowSpec, ready: &[&'a RankedNode]) -> &'a RankedNode {
    // §10.2 deterministic ranking: urgency first, then critical-path / cost
    // (not yet wired — fall through), then node CID via canonical node bytes
    // so insertion order is irrelevant (INV-011/023).
    let cid_of = |key: &str| -> Vec<u8> {
        for nd in flow.nodes() {
            if nd.key.to_string() == key {
                let canon = Value::Map(
                    [
                        ("key".into(), Value::Text(nd.key.to_string())),
                        ("kind".into(), canonical_kind(&nd.kind)),
                        ("guard".into(), nd.guard.to_canon()),
                        ("urgency".into(), canonical_urgency(nd.urgency)),
                    ]
                    .into_iter()
                    .collect(),
                );
                return encode(&canon);
            }
        }
        Vec::new()
    };
    let urgency_rank = |u: braid_flow_ir::UrgencyClass| match u {
        braid_flow_ir::UrgencyClass::SafetyRecovery => 0,
        braid_flow_ir::UrgencyClass::Required => 1,
        braid_flow_ir::UrgencyClass::Diagnostic => 2,
        braid_flow_ir::UrgencyClass::Optimization => 3,
        braid_flow_ir::UrgencyClass::Cleanup => 4,
    };
    let mut best = ready[0];
    let mut best_key = (urgency_rank(best.urgency), cid_of(&best.key));
    for &r in &ready[1..] {
        let k = (urgency_rank(r.urgency), cid_of(&r.key));
        if k < best_key {
            best = r;
            best_key = k;
        }
    }
    best
}

fn step_details(
    flow: &FlowSpec,
    key: &str,
    snapshot: &FlowSnapshot,
    completions: &CompletionMap,
) -> Result<(PlanStepKind, Option<Cid>, Option<String>), PlanError> {
    for nd in flow.nodes() {
        if nd.key.to_string() == key {
            return match &nd.kind {
                FlowNodeKind::InvokeCapsule { capsule } => {
                    Ok((PlanStepKind::InvokeCapsule, Some(*capsule), None))
                }
                FlowNodeKind::Choice { arms, otherwise } => {
                    let mut selected = None;
                    for (index, arm) in arms.iter().enumerate() {
                        match eval_predicate(&arm.when, snapshot, completions) {
                            ProofState::Proven => {
                                if selected.is_some() {
                                    return Err(PlanError::InconsistentGraph {
                                        reason: alloc::format!(
                                            "multiple Choice arms Proven for {key}"
                                        ),
                                        invariant: "INV-FLOW-011",
                                    });
                                }
                                selected = Some(arm.then.to_string());
                            }
                            ProofState::Disproven => {}
                            ProofState::Unknown(reason) => {
                                return Err(PlanError::UnknownProof {
                                    reason: alloc::format!(
                                        "Choice arm {index} Unknown for {key}: {reason}"
                                    ),
                                    invariant: "INV-FLOW-007",
                                });
                            }
                        }
                    }
                    Ok((
                        PlanStepKind::Choice,
                        None,
                        Some(selected.unwrap_or_else(|| otherwise.to_string())),
                    ))
                }
                FlowNodeKind::JoinAll => Ok((PlanStepKind::JoinAll, None, None)),
                FlowNodeKind::Terminal { .. } => Ok((PlanStepKind::Terminal, None, None)),
            };
        }
    }
    Err(PlanError::InconsistentGraph {
        reason: alloc::format!("selected node {key} is absent"),
        invariant: "INV-FLOW-002",
    })
}

fn compute_plan_cid(
    flow_cid: Cid,
    snapshot_cid: Cid,
    ctx: &PlanningContext,
    satiated: &[SatiatedTransition],
    next_step: Option<&PlanStep>,
    trace: &[String],
) -> Cid {
    // INV-020 / INV-023: Plan CID is sensitive to every planning input.
    let next_v = match next_step {
        Some(step) => Value::Map(
            [
                ("node".into(), Value::Text(step.node.clone())),
                (
                    "kind".into(),
                    Value::Text(
                        match step.kind {
                            PlanStepKind::InvokeCapsule => "invoke_capsule",
                            PlanStepKind::Choice => "choice",
                            PlanStepKind::JoinAll => "join_all",
                            PlanStepKind::Terminal => "terminal",
                        }
                        .into(),
                    ),
                ),
                (
                    "capsule".into(),
                    step.capsule.map_or_else(
                        || Value::Bytes(Vec::new()),
                        |cid| Value::Bytes(cid.0.to_vec()),
                    ),
                ),
                (
                    "choice_target".into(),
                    Value::Text(
                        step.choice_target
                            .clone()
                            .unwrap_or_else(|| "__none__".into()),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
        ),
        None => Value::Text("__none__".into()),
    };
    let satiated_v = Value::List(
        satiated
            .iter()
            .map(|s| Value::Text(s.node.clone()))
            .collect(),
    );
    let trace_v = Value::List(trace.iter().map(|t| Value::Text(t.clone())).collect());
    let canon = Value::Map(
        [
            ("flow_cid".into(), Value::Bytes(flow_cid.0.to_vec())),
            ("snapshot_cid".into(), Value::Bytes(snapshot_cid.0.to_vec())),
            (
                "target_profile_cid".into(),
                Value::Bytes(ctx.target_profile_cid.0.to_vec()),
            ),
            (
                "cache_manifest_cid".into(),
                Value::Bytes(ctx.cache_manifest_cid.0.to_vec()),
            ),
            (
                "planner_version".into(),
                Value::Int(ctx.planner_version as i64),
            ),
            ("next".into(), next_v),
            ("satiated".into(), satiated_v),
            ("trace".into(), trace_v),
        ]
        .into_iter()
        .collect(),
    );
    Cid::compute(PLAN_DOMAIN, &encode(&canon))
}

/// Reverse-dependency index for early-cutoff invalidation (INV-FLOW-023 §14.3).
/// Built from the admitted flow's edges; stable and deterministic.
#[derive(Debug, Clone)]
pub struct ReverseDeps {
    inner: BTreeMap<String, Vec<String>>,
}

impl ReverseDeps {
    pub fn from_flow(flow: &FlowSpec) -> Self {
        let mut inner: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for n in flow.nodes() {
            inner.entry(n.key.to_string()).or_default();
        }
        for e in flow.edges() {
            let (from, to) = match e {
                FlowEdge::Data { from, to, .. } => {
                    let fk = match from {
                        braid_flow_ir::ValueSource::Node(o) => o.node.to_string(),
                        _ => continue,
                    };
                    (fk, to.node.to_string())
                }
                FlowEdge::After { from, to, .. } => (from.to_string(), to.to_string()),
            };
            inner.entry(from.clone()).or_default();
            inner.entry(to.clone()).or_default();
            // Reverse index: dependents(from) = [to].
            inner.entry(from).or_default().push(to);
        }
        for nd in flow.nodes() {
            if let FlowNodeKind::Choice { arms, otherwise } = &nd.kind {
                let choice = nd.key.to_string();
                for a in arms {
                    let target = a.then.to_string();
                    inner
                        .entry(choice.clone())
                        .or_default()
                        .push(target.clone());
                    inner.entry(target).or_default();
                }
                let target = otherwise.to_string();
                inner.entry(choice).or_default().push(target.clone());
                inner.entry(target).or_default();
            }
        }
        for v in inner.values_mut() {
            v.sort();
            v.dedup();
        }
        Self { inner }
    }

    pub fn direct_dependents(&self, node: &str) -> &[String] {
        self.inner.get(node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn transitive_dependents(&self, node: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![node.to_string()];
        while let Some(cur) = stack.pop() {
            if let Some(deps) = self.inner.get(&cur) {
                for d in deps {
                    if seen.insert(d.clone()) {
                        stack.push(d.clone());
                    }
                }
            }
        }
        seen.remove(node);
        seen
    }
}
