//! Bounded symbolic proof for pairwise `Choice` predicate disjointness.
//!
//! The v0 fragment is intentionally closed: constants, comparisons between a
//! canonical literal and one exact reference, reflexive comparisons, boolean
//! connectives, and completion-class atoms. Distinct-reference relations that
//! need relational solving return [`Disjointness::Unknown`]. Resource
//! exhaustion does the same; neither condition can become admission.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use braid_flow_ir::{CompletionClass, NodeKey, Predicate, ValueExpr};
use braid_ir::{Value, encode};

/// Version of the closed symbolic fragment implemented by this module.
pub const DISJOINTNESS_FRAGMENT_VERSION: u16 = 0;

pub const DISJOINTNESS_MAX_PREDICATE_NODES: usize = 16_384;
pub const DISJOINTNESS_MAX_PREDICATE_DEPTH: usize = 32;
pub const DISJOINTNESS_MAX_NORMAL_FORM_CLAUSES: usize = 4_096;
pub const DISJOINTNESS_MAX_NORMAL_FORM_ATOMS: usize = 65_536;
pub const DISJOINTNESS_MAX_WORK: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverLimit {
    PredicateNodes,
    PredicateDepth,
    NormalFormClauses,
    NormalFormAtoms,
    Work,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisjointnessUnknown {
    LimitExceeded {
        kind: SolverLimit,
        actual: usize,
        limit: usize,
    },
    UnsupportedReferenceRelation,
    IncomparableOrderedValues,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueBinding {
    pub expression: ValueExpr,
    pub value: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionWitness {
    Pending,
    ExecutedSuccess,
    SatisfiedWithoutExecution,
    Failure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionBinding {
    pub node: NodeKey,
    pub state: CompletionWitness,
}

/// A deterministic, minimal-for-the-supported-clause overlap witness.
///
/// Only expressions and completion nodes needed by the selected satisfiable
/// clause are included. The vectors are sorted by canonical expression/node
/// identity, so source operand ordering cannot perturb diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PredicateCounterexample {
    pub values: Vec<ValueBinding>,
    pub completions: Vec<CompletionBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Disjointness {
    Disjoint,
    Overlap(PredicateCounterexample),
    Unknown(DisjointnessUnknown),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NormalFormSize {
    clauses: usize,
    atoms: usize,
}

impl NormalFormSize {
    const TRUE: Self = Self {
        clauses: 1,
        atoms: 0,
    };
    const FALSE: Self = Self {
        clauses: 0,
        atoms: 0,
    };
    const ATOM: Self = Self {
        clauses: 1,
        atoms: 1,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Comparison {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl Comparison {
    const fn negated(self) -> Self {
        match self {
            Self::Eq => Self::Ne,
            Self::Ne => Self::Eq,
            Self::Lt => Self::Ge,
            Self::Le => Self::Gt,
            Self::Gt => Self::Le,
            Self::Ge => Self::Lt,
        }
    }

    const fn swapped(self) -> Self {
        match self {
            Self::Eq => Self::Eq,
            Self::Ne => Self::Ne,
            Self::Lt => Self::Gt,
            Self::Le => Self::Ge,
            Self::Gt => Self::Lt,
            Self::Ge => Self::Le,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Atom {
    Comparison {
        operation: Comparison,
        left: ValueExpr,
        right: ValueExpr,
    },
    Completion {
        node: NodeKey,
        class: CompletionClass,
        positive: bool,
    },
}

type Clause = Vec<Atom>;

/// Prove whether two predicates can both be true in the v0 symbolic fragment.
///
/// Preflight computes the maximum normal-form footprint without allocation.
/// Only a bounded, accepted footprint is materialized.
pub fn analyze_disjointness(left: &Predicate, right: &Predicate) -> Disjointness {
    match preflight_pair(left, right) {
        Ok(_) => analyze_preflighted(left, right),
        Err(reason) => Disjointness::Unknown(reason),
    }
}

pub(crate) fn preflight_pair(
    left: &Predicate,
    right: &Predicate,
) -> Result<usize, DisjointnessUnknown> {
    let mut nodes = 0usize;
    let left_size = measure(left, false, 1, &mut nodes)?;
    let right_size = measure(right, false, 1, &mut nodes)?;
    let pair = conjunction_size(left_size, right_size)?;
    let work = nodes
        .checked_add(pair.clauses)
        .and_then(|value| value.checked_add(pair.atoms))
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| limit(SolverLimit::Work, usize::MAX, DISJOINTNESS_MAX_WORK))?;
    if work > DISJOINTNESS_MAX_WORK {
        return Err(limit(SolverLimit::Work, work, DISJOINTNESS_MAX_WORK));
    }
    Ok(work)
}

pub(crate) fn analyze_preflighted(left: &Predicate, right: &Predicate) -> Disjointness {
    let left_clauses = normalize(left, false);
    let right_clauses = normalize(right, false);
    let clauses = conjunct_clauses(left_clauses, right_clauses);
    let mut first_unknown = None;

    for clause in clauses {
        match analyze_clause(&clause) {
            ClauseResult::Satisfiable(counterexample) => {
                return Disjointness::Overlap(counterexample);
            }
            ClauseResult::Unsatisfiable => {}
            ClauseResult::Unknown(reason) => {
                if first_unknown.is_none() {
                    first_unknown = Some(reason);
                }
            }
        }
    }

    match first_unknown {
        Some(reason) => Disjointness::Unknown(reason),
        None => Disjointness::Disjoint,
    }
}

fn measure(
    predicate: &Predicate,
    negated: bool,
    depth: usize,
    nodes: &mut usize,
) -> Result<NormalFormSize, DisjointnessUnknown> {
    if depth > DISJOINTNESS_MAX_PREDICATE_DEPTH {
        return Err(limit(
            SolverLimit::PredicateDepth,
            depth,
            DISJOINTNESS_MAX_PREDICATE_DEPTH,
        ));
    }
    *nodes = nodes.checked_add(1).ok_or_else(|| {
        limit(
            SolverLimit::PredicateNodes,
            usize::MAX,
            DISJOINTNESS_MAX_PREDICATE_NODES,
        )
    })?;
    if *nodes > DISJOINTNESS_MAX_PREDICATE_NODES {
        return Err(limit(
            SolverLimit::PredicateNodes,
            *nodes,
            DISJOINTNESS_MAX_PREDICATE_NODES,
        ));
    }

    match predicate {
        Predicate::Const(value) => {
            if *value ^ negated {
                Ok(NormalFormSize::TRUE)
            } else {
                Ok(NormalFormSize::FALSE)
            }
        }
        Predicate::Not(inner) => measure(inner, !negated, depth + 1, nodes),
        Predicate::And(items) | Predicate::Or(items) => {
            let conjunction = matches!(predicate, Predicate::And(_)) ^ negated;
            let mut size = if conjunction {
                NormalFormSize::TRUE
            } else {
                NormalFormSize::FALSE
            };
            for item in items {
                let child = measure(item, negated, depth + 1, nodes)?;
                size = if conjunction {
                    conjunction_size(size, child)?
                } else {
                    disjunction_size(size, child)?
                };
            }
            Ok(size)
        }
        Predicate::Eq(_, _)
        | Predicate::Ne(_, _)
        | Predicate::Lt(_, _)
        | Predicate::Le(_, _)
        | Predicate::Gt(_, _)
        | Predicate::Ge(_, _)
        | Predicate::HasCompletion { .. } => Ok(NormalFormSize::ATOM),
    }
}

fn conjunction_size(
    left: NormalFormSize,
    right: NormalFormSize,
) -> Result<NormalFormSize, DisjointnessUnknown> {
    let clauses = left
        .clauses
        .checked_mul(right.clauses)
        .ok_or_else(|| clauses_limit(usize::MAX))?;
    let atoms = left
        .atoms
        .checked_mul(right.clauses)
        .and_then(|first| {
            right
                .atoms
                .checked_mul(left.clauses)
                .and_then(|second| first.checked_add(second))
        })
        .ok_or_else(|| atoms_limit(usize::MAX))?;
    check_normal_form(clauses, atoms)
}

fn disjunction_size(
    left: NormalFormSize,
    right: NormalFormSize,
) -> Result<NormalFormSize, DisjointnessUnknown> {
    let clauses = left
        .clauses
        .checked_add(right.clauses)
        .ok_or_else(|| clauses_limit(usize::MAX))?;
    let atoms = left
        .atoms
        .checked_add(right.atoms)
        .ok_or_else(|| atoms_limit(usize::MAX))?;
    check_normal_form(clauses, atoms)
}

fn check_normal_form(clauses: usize, atoms: usize) -> Result<NormalFormSize, DisjointnessUnknown> {
    if clauses > DISJOINTNESS_MAX_NORMAL_FORM_CLAUSES {
        return Err(clauses_limit(clauses));
    }
    if atoms > DISJOINTNESS_MAX_NORMAL_FORM_ATOMS {
        return Err(atoms_limit(atoms));
    }
    Ok(NormalFormSize { clauses, atoms })
}

fn clauses_limit(actual: usize) -> DisjointnessUnknown {
    limit(
        SolverLimit::NormalFormClauses,
        actual,
        DISJOINTNESS_MAX_NORMAL_FORM_CLAUSES,
    )
}

fn atoms_limit(actual: usize) -> DisjointnessUnknown {
    limit(
        SolverLimit::NormalFormAtoms,
        actual,
        DISJOINTNESS_MAX_NORMAL_FORM_ATOMS,
    )
}

const fn limit(kind: SolverLimit, actual: usize, limit: usize) -> DisjointnessUnknown {
    DisjointnessUnknown::LimitExceeded {
        kind,
        actual,
        limit,
    }
}

fn normalize(predicate: &Predicate, negated: bool) -> Vec<Clause> {
    match predicate {
        Predicate::Const(value) => {
            if *value ^ negated {
                vec![Vec::new()]
            } else {
                Vec::new()
            }
        }
        Predicate::Not(inner) => normalize(inner, !negated),
        Predicate::And(items) | Predicate::Or(items) => {
            let conjunction = matches!(predicate, Predicate::And(_)) ^ negated;
            let mut clauses = if conjunction {
                vec![Vec::new()]
            } else {
                Vec::new()
            };
            for item in items {
                let child = normalize(item, negated);
                if conjunction {
                    clauses = conjunct_clauses(clauses, child);
                } else {
                    clauses.extend(child);
                }
            }
            canonicalize_clauses(&mut clauses);
            clauses
        }
        _ => vec![vec![normalize_atom(predicate, negated)]],
    }
}

fn conjunct_clauses(left: Vec<Clause>, right: Vec<Clause>) -> Vec<Clause> {
    if left.is_empty() || right.is_empty() {
        return Vec::new();
    }
    let mut output = Vec::with_capacity(left.len() * right.len());
    for left_clause in &left {
        for right_clause in &right {
            let mut clause = Vec::with_capacity(left_clause.len() + right_clause.len());
            clause.extend(left_clause.iter().cloned());
            clause.extend(right_clause.iter().cloned());
            canonicalize_clause(&mut clause);
            output.push(clause);
        }
    }
    canonicalize_clauses(&mut output);
    output
}

fn normalize_atom(predicate: &Predicate, negated: bool) -> Atom {
    let (operation, left, right) = match predicate {
        Predicate::Eq(left, right) => (Comparison::Eq, left, right),
        Predicate::Ne(left, right) => (Comparison::Ne, left, right),
        Predicate::Lt(left, right) => (Comparison::Lt, left, right),
        Predicate::Le(left, right) => (Comparison::Le, left, right),
        Predicate::Gt(left, right) => (Comparison::Gt, left, right),
        Predicate::Ge(left, right) => (Comparison::Ge, left, right),
        Predicate::HasCompletion { node, class } => {
            return Atom::Completion {
                node: node.clone(),
                class: *class,
                positive: !negated,
            };
        }
        Predicate::Const(_) | Predicate::And(_) | Predicate::Or(_) | Predicate::Not(_) => {
            unreachable!("boolean structure is normalized before atoms")
        }
    };
    let operation = if negated {
        operation.negated()
    } else {
        operation
    };
    let mut left = left.clone();
    let mut right = right.clone();
    if matches!(operation, Comparison::Eq | Comparison::Ne)
        && expression_key(&right) < expression_key(&left)
    {
        core::mem::swap(&mut left, &mut right);
    }
    Atom::Comparison {
        operation,
        left,
        right,
    }
}

fn canonicalize_clause(clause: &mut Clause) {
    clause.sort_by_cached_key(atom_key);
    clause.dedup();
}

fn canonicalize_clauses(clauses: &mut Vec<Clause>) {
    clauses.sort_by_cached_key(clause_key);
    clauses.dedup();
}

fn clause_key(clause: &Clause) -> Vec<u8> {
    let values = clause.iter().map(atom_canon).collect();
    encode(&Value::List(values))
}

fn atom_key(atom: &Atom) -> Vec<u8> {
    encode(&atom_canon(atom))
}

fn atom_canon(atom: &Atom) -> Value {
    match atom {
        Atom::Comparison {
            operation,
            left,
            right,
        } => Value::map(vec![(
            match operation {
                Comparison::Eq => "eq",
                Comparison::Ne => "ne",
                Comparison::Lt => "lt",
                Comparison::Le => "le",
                Comparison::Gt => "gt",
                Comparison::Ge => "ge",
            },
            Value::List(vec![expression_canon(left), expression_canon(right)]),
        )]),
        Atom::Completion {
            node,
            class,
            positive,
        } => Value::map(vec![(
            if *positive {
                "has_completion"
            } else {
                "not_completion"
            },
            Value::List(vec![
                Value::Text(node.to_string()),
                Value::Text(completion_name(*class).into()),
            ]),
        )]),
    }
}

fn expression_key(expression: &ValueExpr) -> Vec<u8> {
    encode(&expression_canon(expression))
}

fn expression_canon(expression: &ValueExpr) -> Value {
    match expression {
        ValueExpr::Literal(value) => Value::map(vec![("literal", value.clone())]),
        ValueExpr::RootInput(input) => {
            Value::map(vec![("root_input", Value::Text(input.to_string()))])
        }
        ValueExpr::NodeOutput(output) => Value::map(vec![(
            "node_output",
            Value::List(vec![
                Value::Text(output.node.to_string()),
                Value::Text(output.port.to_string()),
            ]),
        )]),
        ValueExpr::SnapshotFact(fact) => {
            Value::map(vec![("snapshot_fact", Value::Text(fact.to_string()))])
        }
    }
}

const fn completion_name(class: CompletionClass) -> &'static str {
    match class {
        CompletionClass::ExecutedSuccess => "executed_success",
        CompletionClass::SatisfiedWithoutExecution => "satisfied_without_execution",
        CompletionClass::Failure => "failure",
    }
}

enum ClauseResult {
    Satisfiable(PredicateCounterexample),
    Unsatisfiable,
    Unknown(DisjointnessUnknown),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApplyResult {
    Applied,
    Contradiction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrderedKind {
    Bool,
    Int,
    Text,
    Bytes,
}

#[derive(Debug, Clone)]
struct Bound {
    value: Value,
    inclusive: bool,
}

#[derive(Debug, Clone)]
struct ValueDomain {
    expression: ValueExpr,
    exact: Option<Value>,
    excluded: BTreeMap<Vec<u8>, Value>,
    requires_ordered: bool,
    kind: Option<OrderedKind>,
    lower: Option<Bound>,
    upper: Option<Bound>,
}

impl ValueDomain {
    fn new(expression: ValueExpr) -> Self {
        Self {
            expression,
            exact: None,
            excluded: BTreeMap::new(),
            requires_ordered: false,
            kind: None,
            lower: None,
            upper: None,
        }
    }

    fn apply(
        &mut self,
        operation: Comparison,
        literal: &Value,
    ) -> Result<ApplyResult, DisjointnessUnknown> {
        match operation {
            Comparison::Eq => {
                if let Some(exact) = &self.exact
                    && exact != literal
                {
                    return Ok(ApplyResult::Contradiction);
                }
                self.exact = Some(literal.clone());
            }
            Comparison::Ne => {
                self.excluded.insert(encode(literal), literal.clone());
            }
            Comparison::Lt | Comparison::Le | Comparison::Gt | Comparison::Ge => {
                let kind =
                    ordered_kind(literal).ok_or(DisjointnessUnknown::IncomparableOrderedValues)?;
                if let Some(existing) = self.kind
                    && existing != kind
                {
                    return Ok(ApplyResult::Contradiction);
                }
                self.kind = Some(kind);
                let bound = Bound {
                    value: literal.clone(),
                    inclusive: matches!(operation, Comparison::Le | Comparison::Ge),
                };
                if matches!(operation, Comparison::Lt | Comparison::Le) {
                    update_upper(&mut self.upper, bound);
                } else {
                    update_lower(&mut self.lower, bound);
                }
            }
        }
        if self.has_obvious_contradiction() {
            Ok(ApplyResult::Contradiction)
        } else {
            Ok(ApplyResult::Applied)
        }
    }

    fn has_obvious_contradiction(&self) -> bool {
        if let Some(exact) = &self.exact {
            if self.excluded.contains_key(&encode(exact)) {
                return true;
            }
            if let Some(kind) = self.kind
                && ordered_kind(exact) != Some(kind)
            {
                return true;
            }
            if self.requires_ordered && ordered_kind(exact).is_none() {
                return true;
            }
            return !within_bounds(exact, self.lower.as_ref(), self.upper.as_ref());
        }
        match (&self.lower, &self.upper) {
            (Some(lower), Some(upper)) => match compare_ordered(&lower.value, &upper.value) {
                Some(core::cmp::Ordering::Greater) => true,
                Some(core::cmp::Ordering::Equal) => !(lower.inclusive && upper.inclusive),
                Some(core::cmp::Ordering::Less) | None => false,
            },
            _ => false,
        }
    }

    fn witness(&self) -> Option<Value> {
        if self.has_obvious_contradiction() {
            return None;
        }
        if let Some(exact) = &self.exact {
            return Some(exact.clone());
        }
        if let Some(kind) = self.kind {
            let mut candidate = match &self.lower {
                Some(lower) if lower.inclusive => lower.value.clone(),
                Some(lower) => successor(&lower.value)?,
                None => minimum(kind),
            };
            loop {
                if !within_bounds(&candidate, self.lower.as_ref(), self.upper.as_ref()) {
                    return None;
                }
                if !self.excluded.contains_key(&encode(&candidate)) {
                    return Some(candidate);
                }
                candidate = successor(&candidate)?;
            }
        }

        if self.requires_ordered {
            return Some(Value::Bool(false));
        }

        for candidate in [Value::Bool(false), Value::Bool(true)] {
            if !self.excluded.contains_key(&encode(&candidate)) {
                return Some(candidate);
            }
        }
        for number in 0..=self.excluded.len() {
            let candidate = Value::Int(i64::try_from(number).ok()?);
            if !self.excluded.contains_key(&encode(&candidate)) {
                return Some(candidate);
            }
        }
        None
    }
}

fn analyze_clause(clause: &Clause) -> ClauseResult {
    let mut values: BTreeMap<Vec<u8>, ValueDomain> = BTreeMap::new();
    let mut completions: BTreeMap<NodeKey, u8> = BTreeMap::new();
    let mut first_unknown = None;

    for atom in clause {
        match atom {
            Atom::Comparison {
                operation,
                left,
                right,
            } => match apply_comparison(&mut values, *operation, left, right) {
                Ok(ApplyResult::Applied) => {}
                Ok(ApplyResult::Contradiction) => return ClauseResult::Unsatisfiable,
                Err(reason) => {
                    if first_unknown.is_none() {
                        first_unknown = Some(reason);
                    }
                }
            },
            Atom::Completion {
                node,
                class,
                positive,
            } => {
                let allowed = completions.entry(node.clone()).or_insert(0b1111);
                let bit = completion_bit(*class);
                if *positive {
                    *allowed &= bit;
                } else {
                    *allowed &= !bit;
                }
                if *allowed == 0 {
                    return ClauseResult::Unsatisfiable;
                }
            }
        }
    }

    if let Some(reason) = first_unknown {
        return ClauseResult::Unknown(reason);
    }

    let mut value_bindings = Vec::with_capacity(values.len());
    for domain in values.into_values() {
        let Some(value) = domain.witness() else {
            return ClauseResult::Unsatisfiable;
        };
        value_bindings.push(ValueBinding {
            expression: domain.expression,
            value,
        });
    }
    let completion_bindings = completions
        .into_iter()
        .map(|(node, allowed)| CompletionBinding {
            node,
            state: first_completion(allowed),
        })
        .collect();
    ClauseResult::Satisfiable(PredicateCounterexample {
        values: value_bindings,
        completions: completion_bindings,
    })
}

fn apply_comparison(
    domains: &mut BTreeMap<Vec<u8>, ValueDomain>,
    operation: Comparison,
    left: &ValueExpr,
    right: &ValueExpr,
) -> Result<ApplyResult, DisjointnessUnknown> {
    match (left, right) {
        (ValueExpr::Literal(left), ValueExpr::Literal(right)) => {
            evaluate_values(operation, left, right)
                .map(|value| {
                    if value {
                        ApplyResult::Applied
                    } else {
                        ApplyResult::Contradiction
                    }
                })
                .ok_or(DisjointnessUnknown::IncomparableOrderedValues)
        }
        (ValueExpr::Literal(literal), expression) => {
            apply_reference_literal(domains, operation.swapped(), expression, literal)
        }
        (expression, ValueExpr::Literal(literal)) => {
            apply_reference_literal(domains, operation, expression, literal)
        }
        (left, right) if expression_key(left) == expression_key(right) => {
            if matches!(operation, Comparison::Ne | Comparison::Lt | Comparison::Gt) {
                return Ok(ApplyResult::Contradiction);
            }
            let key = expression_key(left);
            let domain = domains
                .entry(key)
                .or_insert_with(|| ValueDomain::new(left.clone()));
            if matches!(operation, Comparison::Le | Comparison::Ge) {
                domain.requires_ordered = true;
            }
            Ok(if domain.has_obvious_contradiction() {
                ApplyResult::Contradiction
            } else {
                ApplyResult::Applied
            })
        }
        _ => Err(DisjointnessUnknown::UnsupportedReferenceRelation),
    }
}

fn apply_reference_literal(
    domains: &mut BTreeMap<Vec<u8>, ValueDomain>,
    operation: Comparison,
    expression: &ValueExpr,
    literal: &Value,
) -> Result<ApplyResult, DisjointnessUnknown> {
    if matches!(expression, ValueExpr::Literal(_)) {
        return Err(DisjointnessUnknown::UnsupportedReferenceRelation);
    }
    let key = expression_key(expression);
    domains
        .entry(key)
        .or_insert_with(|| ValueDomain::new(expression.clone()))
        .apply(operation, literal)
}

fn evaluate_values(operation: Comparison, left: &Value, right: &Value) -> Option<bool> {
    match operation {
        Comparison::Eq => Some(left == right),
        Comparison::Ne => Some(left != right),
        Comparison::Lt => compare_ordered(left, right).map(|order| order.is_lt()),
        Comparison::Le => compare_ordered(left, right).map(|order| order.is_le()),
        Comparison::Gt => compare_ordered(left, right).map(|order| order.is_gt()),
        Comparison::Ge => compare_ordered(left, right).map(|order| order.is_ge()),
    }
}

fn ordered_kind(value: &Value) -> Option<OrderedKind> {
    match value {
        Value::Bool(_) => Some(OrderedKind::Bool),
        Value::Int(_) => Some(OrderedKind::Int),
        Value::Text(_) => Some(OrderedKind::Text),
        Value::Bytes(_) => Some(OrderedKind::Bytes),
        Value::List(_) | Value::Map(_) => None,
    }
}

fn compare_ordered(left: &Value, right: &Value) -> Option<core::cmp::Ordering> {
    match (left, right) {
        (Value::Bool(left), Value::Bool(right)) => Some(left.cmp(right)),
        (Value::Int(left), Value::Int(right)) => Some(left.cmp(right)),
        (Value::Text(left), Value::Text(right)) => Some(left.cmp(right)),
        (Value::Bytes(left), Value::Bytes(right)) => Some(left.cmp(right)),
        _ => None,
    }
}

fn update_lower(current: &mut Option<Bound>, candidate: Bound) {
    match current {
        None => *current = Some(candidate),
        Some(existing) => match compare_ordered(&candidate.value, &existing.value) {
            Some(core::cmp::Ordering::Greater) => *existing = candidate,
            Some(core::cmp::Ordering::Equal) => existing.inclusive &= candidate.inclusive,
            Some(core::cmp::Ordering::Less) | None => {}
        },
    }
}

fn update_upper(current: &mut Option<Bound>, candidate: Bound) {
    match current {
        None => *current = Some(candidate),
        Some(existing) => match compare_ordered(&candidate.value, &existing.value) {
            Some(core::cmp::Ordering::Less) => *existing = candidate,
            Some(core::cmp::Ordering::Equal) => existing.inclusive &= candidate.inclusive,
            Some(core::cmp::Ordering::Greater) | None => {}
        },
    }
}

fn within_bounds(value: &Value, lower: Option<&Bound>, upper: Option<&Bound>) -> bool {
    if let Some(lower) = lower {
        match compare_ordered(value, &lower.value) {
            Some(core::cmp::Ordering::Less) | None => return false,
            Some(core::cmp::Ordering::Equal) if !lower.inclusive => return false,
            Some(core::cmp::Ordering::Equal | core::cmp::Ordering::Greater) => {}
        }
    }
    if let Some(upper) = upper {
        match compare_ordered(value, &upper.value) {
            Some(core::cmp::Ordering::Greater) | None => return false,
            Some(core::cmp::Ordering::Equal) if !upper.inclusive => return false,
            Some(core::cmp::Ordering::Equal | core::cmp::Ordering::Less) => {}
        }
    }
    true
}

fn minimum(kind: OrderedKind) -> Value {
    match kind {
        OrderedKind::Bool => Value::Bool(false),
        OrderedKind::Int => Value::Int(i64::MIN),
        OrderedKind::Text => Value::Text(String::new()),
        OrderedKind::Bytes => Value::Bytes(Vec::new()),
    }
}

fn successor(value: &Value) -> Option<Value> {
    match value {
        Value::Bool(false) => Some(Value::Bool(true)),
        Value::Bool(true) => None,
        Value::Int(value) => value.checked_add(1).map(Value::Int),
        Value::Text(value) => {
            let mut next = value.clone();
            next.push('\0');
            Some(Value::Text(next))
        }
        Value::Bytes(value) => {
            let mut next = value.clone();
            next.push(0);
            Some(Value::Bytes(next))
        }
        Value::List(_) | Value::Map(_) => None,
    }
}

const fn completion_bit(class: CompletionClass) -> u8 {
    match class {
        CompletionClass::ExecutedSuccess => 0b0010,
        CompletionClass::SatisfiedWithoutExecution => 0b0100,
        CompletionClass::Failure => 0b1000,
    }
}

const fn first_completion(allowed: u8) -> CompletionWitness {
    if allowed & 0b0001 != 0 {
        CompletionWitness::Pending
    } else if allowed & 0b0010 != 0 {
        CompletionWitness::ExecutedSuccess
    } else if allowed & 0b0100 != 0 {
        CompletionWitness::SatisfiedWithoutExecution
    } else {
        CompletionWitness::Failure
    }
}
