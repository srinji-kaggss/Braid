//! The closed term registry (PRD §4.1): the "little api-type math the AI can
//! chain". Every term declares its full signature — types, capability, effect
//! class, exposure, cost — and the registry is content-addressed so a capsule
//! pins the EXACT alphabet it was authored against (T6).

use crate::canon;
use crate::cid::{Cid, REGISTRY_DOMAIN};
use crate::value::Value;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use braid_capability::Capability;
use core::str::FromStr;

/// The closed type universe of strand wiring. No interpretable-code type
/// exists (T1) and no float type exists (T8) — by construction, not by check.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TypeTag {
    Bool,
    /// Fixed-point integer (term-declared scaling).
    Int,
    Bytes,
    Text,
    Cid,
    /// A vocabulary-defined domain type.
    Opaque(String, Vec<TypeTag>),
    List(Box<TypeTag>),
}

const MAX_TYPE_TAG_DEPTH: usize = 32;
const MAX_TYPE_TAG_ARGUMENTS: usize = 128;
const MAX_TYPE_TAG_NODES: usize = 16_384;
const MAX_TYPE_TAG_LABEL_BYTES: usize = 128;

/// A structurally unbounded type declaration is rejected before a downstream
/// identity domain projects it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeTagError {
    OpaqueLabelTooLong { length: usize },
    NestingTooDeep { depth: usize },
    TooManyArguments { count: usize },
    TooManyNodes { count: usize },
}

impl core::fmt::Display for TypeTagError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::OpaqueLabelTooLong { length } => {
                write!(f, "opaque label has {length} bytes; maximum is 128")
            }
            Self::NestingTooDeep { depth } => write!(f, "type nesting depth {depth} exceeds 32"),
            Self::TooManyArguments { count } => {
                write!(f, "opaque type has {count} arguments; maximum is 128")
            }
            Self::TooManyNodes { count } => {
                write!(f, "type has at least {count} nodes; maximum is 16384")
            }
        }
    }
}

impl core::error::Error for TypeTagError {}

/// Effect classification of a term (PRD §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EffectClass {
    Pure,
    Read,
    ReversibleWrite,
    Irreversible,
    Egress,
}

/// Exposure/taint lattice (threat T5). Strictly ordered; folds by `max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Exposure {
    Public = 0,
    Internal = 1,
    Confidential = 2,
    Vault = 3,
}

/// One registered term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermSpec {
    /// Stable dotted id, e.g. `view.section`.
    pub id: String,
    pub inputs: Vec<TypeTag>,
    pub output: TypeTag,
    /// `None` ⇔ `effect == Pure` (enforced at insert).
    pub capability: Option<Capability>,
    pub effect: EffectClass,
    /// Exposure this term's output introduces (before folding inputs in).
    pub source_exposure: Exposure,
    /// For `Irreversible`/`Egress` terms: the maximum folded input exposure
    /// the term may consume (the path-level gate's per-sink ceiling — T5).
    pub egress_ceiling: Option<Exposure>,
    /// Abstract worst-case cost units (budget composition — T7 static half).
    pub cost: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// Pure terms carry no capability; effectful terms must carry one.
    CapabilityEffectMismatch { term: String, at: &'static str },
    /// Irreversible/Egress terms must declare an egress ceiling.
    MissingCeiling { term: String, at: &'static str },
    /// Duplicate term identifier.
    DuplicateTerm { term: String, at: &'static str },
    /// Canonical-form violation while decoding a registry value.
    Malformed {
        field: &'static str,
        at: &'static str,
    },
}

impl core::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::CapabilityEffectMismatch { term, at } => {
                write!(f, "capability effect mismatch on {term} at {at}")
            }
            Self::MissingCeiling { term, at } => {
                write!(f, "missing ceiling on {term} at {at}")
            }
            Self::DuplicateTerm { term, at } => {
                write!(f, "duplicate term {term} at {at}")
            }
            Self::Malformed { field, at } => {
                write!(f, "malformed {field} at {at}")
            }
        }
    }
}

impl core::error::Error for RegistryError {}

/// The closed, versioned term registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermRegistry {
    /// Pinned to `canvas_syscall::vocabulary::VOCABULARY_VERSION` by
    /// `tests/vocab_binding.rs` (D11): a kernel vocabulary bump is a Braid
    /// registry event, never silent drift.
    pub vocab_version: u32,
    terms: BTreeMap<String, TermSpec>,
}

fn check_capability_effect_alignment(spec: &TermSpec) -> Result<(), RegistryError> {
    let is_pure = spec.effect == EffectClass::Pure;
    if is_pure == spec.capability.is_some() {
        Err(RegistryError::CapabilityEffectMismatch {
            term: spec.id.clone(),
            at: "TermRegistry::insert",
        })
    } else {
        Ok(())
    }
}

fn check_egress_ceiling_presence(spec: &TermSpec) -> Result<(), RegistryError> {
    if matches!(spec.effect, EffectClass::Irreversible | EffectClass::Egress)
        && spec.egress_ceiling.is_none()
    {
        Err(RegistryError::MissingCeiling {
            term: spec.id.clone(),
            at: "TermRegistry::insert",
        })
    } else {
        Ok(())
    }
}

fn check_no_duplicate_term(
    terms: &BTreeMap<String, TermSpec>,
    spec: &TermSpec,
) -> Result<(), RegistryError> {
    if terms.contains_key(&spec.id) {
        Err(RegistryError::DuplicateTerm {
            term: spec.id.clone(),
            at: "TermRegistry::insert",
        })
    } else {
        Ok(())
    }
}

fn check_registry_key_universe(v: &Value) -> Result<(), RegistryError> {
    if !v.require_only_keys(&["terms", "vocab_version"]) {
        Err(RegistryError::Malformed {
            field: "registry: unknown field",
            at: "TermRegistry::from_canon",
        })
    } else {
        Ok(())
    }
}

fn check_term_ordering(prev: &Option<String>, id: &str) -> Result<(), RegistryError> {
    if let Some(p) = prev {
        if p.as_str() >= id {
            Err(RegistryError::Malformed {
                field: "term order",
                at: "TermRegistry::from_canon",
            })
        } else {
            Ok(())
        }
    } else {
        Ok(())
    }
}

fn decode_and_insert_term(
    reg: &mut TermRegistry,
    prev: &mut Option<String>,
    it: &Value,
) -> Result<(), RegistryError> {
    let spec = term_from_canon(it)?;
    check_term_ordering(prev, &spec.id)?;
    *prev = Some(spec.id.clone());
    reg.insert(spec)
}

impl TermRegistry {
    pub fn new(vocab_version: u32) -> Self {
        TermRegistry {
            vocab_version,
            terms: BTreeMap::new(),
        }
    }

    /// Insert with the registry invariants enforced (fail-closed at
    /// construction — an illegal TermSpec is unrepresentable in a registry).
    pub fn insert(&mut self, spec: TermSpec) -> Result<(), RegistryError> {
        check_capability_effect_alignment(&spec)?;
        check_egress_ceiling_presence(&spec)?;
        check_no_duplicate_term(&self.terms, &spec)?;
        self.terms.insert(spec.id.clone(), spec);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&TermSpec> {
        self.terms.get(id)
    }

    pub fn terms(&self) -> impl Iterator<Item = &TermSpec> {
        self.terms.values()
    }

    pub fn len(&self) -> usize {
        self.terms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// Content address of the registry's canonical encoding.
    pub fn cid(&self) -> Cid {
        Cid::compute(REGISTRY_DOMAIN, &canon::encode(&self.to_canon()))
    }

    pub fn to_canon(&self) -> Value {
        let terms: Vec<Value> = self.terms.values().map(term_to_canon).collect();
        Value::map(vec![
            ("terms", Value::List(terms)),
            ("vocab_version", Value::Int(self.vocab_version as i64)),
        ])
    }

    pub fn from_canon(v: &Value) -> Result<Self, RegistryError> {
        check_registry_key_universe(v)?;
        let vocab = decode_vocab_version(v)?;
        let mut reg = TermRegistry::new(vocab);
        let items = decode_terms_value_list(v)?;
        let mut prev: Option<String> = None;
        for it in items {
            decode_and_insert_term(&mut reg, &mut prev, it)?;
        }
        Ok(reg)
    }
}

fn decode_vocab_version(v: &Value) -> Result<u32, RegistryError> {
    match v.get_field("vocab_version") {
        Some(Value::Int(n)) if *n >= 0 && *n <= u32::MAX as i64 => Ok(*n as u32),
        _ => Err(RegistryError::Malformed {
            field: "vocab_version",
            at: "decode_vocab_version",
        }),
    }
}

fn decode_terms_value_list(v: &Value) -> Result<&Vec<Value>, RegistryError> {
    match v.get_field("terms") {
        Some(Value::List(items)) => Ok(items),
        _ => Err(RegistryError::Malformed {
            field: "terms",
            at: "decode_terms_value_list",
        }),
    }
}

// ── leaf encodings ──

/// Bound recursive and variable-sized type structure before canonical
/// projection. This does not narrow legacy opaque-label semantics.
pub fn validate_type_tag(value_type: &TypeTag) -> Result<(), TypeTagError> {
    type_tag_node_count(value_type).map(|_| ())
}

/// Validate one type and return its structural work units. Downstream graph
/// domains use this to enforce an aggregate budget across many valid tags.
pub fn type_tag_node_count(value_type: &TypeTag) -> Result<usize, TypeTagError> {
    let mut remaining_nodes = MAX_TYPE_TAG_NODES;
    validate_type_tag_at(value_type, 1, &mut remaining_nodes)?;
    Ok(MAX_TYPE_TAG_NODES - remaining_nodes)
}

fn validate_type_tag_at(
    value_type: &TypeTag,
    depth: usize,
    remaining_nodes: &mut usize,
) -> Result<(), TypeTagError> {
    if depth > MAX_TYPE_TAG_DEPTH {
        return Err(TypeTagError::NestingTooDeep { depth });
    }
    if *remaining_nodes == 0 {
        return Err(TypeTagError::TooManyNodes {
            count: MAX_TYPE_TAG_NODES + 1,
        });
    }
    *remaining_nodes -= 1;
    match value_type {
        TypeTag::Opaque(label, arguments) => {
            if label.len() > MAX_TYPE_TAG_LABEL_BYTES {
                return Err(TypeTagError::OpaqueLabelTooLong {
                    length: label.len(),
                });
            }
            if arguments.len() > MAX_TYPE_TAG_ARGUMENTS {
                return Err(TypeTagError::TooManyArguments {
                    count: arguments.len(),
                });
            }
            for argument in arguments {
                validate_type_tag_at(argument, depth + 1, remaining_nodes)?;
            }
        }
        TypeTag::List(inner) => validate_type_tag_at(inner, depth + 1, remaining_nodes)?,
        TypeTag::Bool | TypeTag::Int | TypeTag::Bytes | TypeTag::Text | TypeTag::Cid => {}
    }
    Ok(())
}

/// Project a type into the one canonical textual form used inside Braid's
/// identity-bearing values. Downstream IR crates must call this function
/// rather than re-derive the type encoding.
pub fn type_tag_to_text(t: &TypeTag) -> String {
    match t {
        TypeTag::Bool => "bool".into(),
        TypeTag::Int => "int".into(),
        TypeTag::Bytes => "bytes".into(),
        TypeTag::Text => "text".into(),
        TypeTag::Cid => "cid".into(),
        TypeTag::Opaque(label, args) => {
            if args.is_empty() {
                label.clone()
            } else {
                format!(
                    "{}<{}>",
                    label,
                    args.iter()
                        .map(type_tag_to_text)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
        TypeTag::List(inner) => format!("list<{}>", type_tag_to_text(inner)),
    }
}

fn parse_opaque_args(args_str: &str) -> Result<Vec<TypeTag>, RegistryError> {
    if args_str.is_empty() {
        Ok(Vec::new())
    } else {
        args_str
            .split(',')
            .map(|a| type_from_text(a.trim()))
            .collect()
    }
}

fn parse_compound_type(s: &str) -> Result<TypeTag, RegistryError> {
    if let Some(inner) = s.strip_prefix("list<").and_then(|r| r.strip_suffix('>')) {
        Ok(TypeTag::List(Box::new(type_from_text(inner)?)))
    } else if let Some(open) = s.find('<') {
        let label = s[..open].to_string();
        let args_str = &s[open + 1..s.len() - 1];
        let args = parse_opaque_args(args_str)?;
        Ok(TypeTag::Opaque(label, args))
    } else {
        Ok(TypeTag::Opaque(s.to_string(), Vec::new()))
    }
}

fn type_from_text(s: &str) -> Result<TypeTag, RegistryError> {
    match s {
        "bool" => Ok(TypeTag::Bool),
        "int" => Ok(TypeTag::Int),
        "bytes" => Ok(TypeTag::Bytes),
        "text" => Ok(TypeTag::Text),
        "cid" => Ok(TypeTag::Cid),
        _ => parse_compound_type(s),
    }
}

pub(crate) fn effect_to_text(e: EffectClass) -> &'static str {
    match e {
        EffectClass::Pure => "pure",
        EffectClass::Read => "read",
        EffectClass::ReversibleWrite => "reversible-write",
        EffectClass::Irreversible => "irreversible",
        EffectClass::Egress => "egress",
    }
}

fn effect_from_text(s: &str) -> Result<EffectClass, RegistryError> {
    match s {
        "pure" => Ok(EffectClass::Pure),
        "read" => Ok(EffectClass::Read),
        "reversible-write" => Ok(EffectClass::ReversibleWrite),
        "irreversible" => Ok(EffectClass::Irreversible),
        "egress" => Ok(EffectClass::Egress),
        _ => Err(RegistryError::Malformed {
            field: "effect class",
            at: "effect_from_text",
        }),
    }
}

pub(crate) fn exposure_to_int(x: Exposure) -> i64 {
    x as i64
}

fn exposure_from_int(n: i64) -> Result<Exposure, RegistryError> {
    match n {
        0 => Ok(Exposure::Public),
        1 => Ok(Exposure::Internal),
        2 => Ok(Exposure::Confidential),
        3 => Ok(Exposure::Vault),
        _ => Err(RegistryError::Malformed {
            field: "exposure",
            at: "exposure_from_int",
        }),
    }
}

fn term_to_canon(t: &TermSpec) -> Value {
    let mut fields = vec![
        ("id", Value::Text(t.id.clone())),
        (
            "inputs",
            Value::List(
                t.inputs
                    .iter()
                    .map(|i| Value::Text(type_tag_to_text(i)))
                    .collect(),
            ),
        ),
        ("output", Value::Text(type_tag_to_text(&t.output))),
        ("effect", Value::Text(effect_to_text(t.effect).into())),
        ("exposure", Value::Int(exposure_to_int(t.source_exposure))),
        ("cost", Value::Int(t.cost as i64)),
    ];
    if let Some(cap) = &t.capability {
        fields.push(("capability", Value::Text(cap.to_string())));
    }
    if let Some(c) = t.egress_ceiling {
        fields.push(("ceiling", Value::Int(exposure_to_int(c))));
    }
    Value::map(fields)
}

fn decode_term_id(v: &Value) -> Result<String, RegistryError> {
    match v.get_field("id") {
        Some(Value::Text(s)) => Ok(s.clone()),
        _ => Err(RegistryError::Malformed {
            field: "term id",
            at: "decode_term_id",
        }),
    }
}

fn decode_term_inputs(v: &Value) -> Result<Vec<TypeTag>, RegistryError> {
    match v.get_field("inputs") {
        Some(Value::List(items)) => items
            .iter()
            .map(|i| match i {
                Value::Text(s) => type_from_text(s),
                _ => Err(RegistryError::Malformed {
                    field: "input type",
                    at: "decode_term_inputs",
                }),
            })
            .collect::<Result<Vec<_>, _>>(),
        _ => Err(RegistryError::Malformed {
            field: "inputs",
            at: "decode_term_inputs",
        }),
    }
}

fn decode_term_output(v: &Value) -> Result<TypeTag, RegistryError> {
    match v.get_field("output") {
        Some(Value::Text(s)) => type_from_text(s),
        _ => Err(RegistryError::Malformed {
            field: "output",
            at: "decode_term_output",
        }),
    }
}

fn decode_term_effect(v: &Value) -> Result<EffectClass, RegistryError> {
    match v.get_field("effect") {
        Some(Value::Text(s)) => effect_from_text(s),
        _ => Err(RegistryError::Malformed {
            field: "effect",
            at: "decode_term_effect",
        }),
    }
}

fn decode_term_exposure(v: &Value) -> Result<Exposure, RegistryError> {
    match v.get_field("exposure") {
        Some(Value::Int(n)) => exposure_from_int(*n),
        _ => Err(RegistryError::Malformed {
            field: "exposure",
            at: "decode_term_exposure",
        }),
    }
}

fn decode_term_cost(v: &Value) -> Result<u64, RegistryError> {
    match v.get_field("cost") {
        Some(Value::Int(n)) if *n >= 0 => Ok(*n as u64),
        _ => Err(RegistryError::Malformed {
            field: "cost",
            at: "decode_term_cost",
        }),
    }
}

fn decode_term_capability(v: &Value) -> Result<Option<Capability>, RegistryError> {
    match v.get_field("capability") {
        None => Ok(None),
        Some(Value::Text(s)) => match Capability::from_str(s) {
            Ok(cap) => Ok(Some(cap)),
            Err(_err) => Err(RegistryError::Malformed {
                field: "unknown capability",
                at: "decode_term_capability",
            }),
        },
        Some(_) => Err(RegistryError::Malformed {
            field: "capability",
            at: "decode_term_capability",
        }),
    }
}

fn decode_term_ceiling(v: &Value) -> Result<Option<Exposure>, RegistryError> {
    match v.get_field("ceiling") {
        None => Ok(None),
        Some(Value::Int(n)) => exposure_from_int(*n).map(Some),
        Some(_) => Err(RegistryError::Malformed {
            field: "ceiling",
            at: "decode_term_ceiling",
        }),
    }
}

fn check_term_key_universe(v: &Value) -> Result<(), RegistryError> {
    if !v.require_only_keys(&[
        "id",
        "inputs",
        "output",
        "effect",
        "exposure",
        "cost",
        "capability",
        "ceiling",
    ]) {
        Err(RegistryError::Malformed {
            field: "term: unknown field",
            at: "term_from_canon",
        })
    } else {
        Ok(())
    }
}

fn decode_term_core(v: &Value) -> Result<(String, Vec<TypeTag>, TypeTag), RegistryError> {
    let id = decode_term_id(v)?;
    let inputs = decode_term_inputs(v)?;
    let output = decode_term_output(v)?;
    Ok((id, inputs, output))
}

type PolicyFields = (
    EffectClass,
    Exposure,
    u64,
    Option<Capability>,
    Option<Exposure>,
);

fn decode_term_policy(v: &Value) -> Result<PolicyFields, RegistryError> {
    let effect = decode_term_effect(v)?;
    let source_exposure = decode_term_exposure(v)?;
    let cost = decode_term_cost(v)?;
    let capability = decode_term_capability(v)?;
    let egress_ceiling = decode_term_ceiling(v)?;
    Ok((effect, source_exposure, cost, capability, egress_ceiling))
}

fn term_from_canon(v: &Value) -> Result<TermSpec, RegistryError> {
    check_term_key_universe(v)?;
    let (id, inputs, output) = decode_term_core(v)?;
    let (effect, source_exposure, cost, capability, egress_ceiling) = decode_term_policy(v)?;

    Ok(TermSpec {
        id,
        inputs,
        output,
        capability,
        effect,
        source_exposure,
        egress_ceiling,
        cost,
    })
}
