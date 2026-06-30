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
///
/// //why an `Opaque` variant (D31): a *global* IR must let each vocabulary
/// package declare its own domain types (CMS `entity`/`directive`, JS
/// `function`/`record`, …) without a core edit per language. The dotted
/// label is the protocol-stable identity (compared by `(label, args)`); it
/// is what canonical encoding serializes, so a rename is CID-breaking. The
/// core keeps only the language-neutral atoms (`Bool`/`Int`/`Bytes`/`Text`/
/// `Cid`/`List`); every domain type is a vocabulary-owned `Opaque`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeTag {
    Bool,
    /// Fixed-point integer (term-declared scaling).
    Int,
    Bytes,
    Text,
    Cid,
    /// A vocabulary-defined domain type: a dotted label plus optional type
    /// arguments (e.g. `Opaque("cms.entity", [])`, `Opaque("js.record", [Text])`).
    /// The label is the identity; the verifier compares `(label, args)` by
    /// structural equality. No core type lives here — `Entity`/`Directive`
    /// are now `Opaque("cms.entity", [])` / `Opaque("cms.directive", [])` in
    /// the `braid-vocab-cms` package.
    Opaque(String, Vec<TypeTag>),
    List(Box<TypeTag>),
}

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
    CapabilityEffectMismatch(String),
    /// Irreversible/Egress terms must declare an egress ceiling.
    MissingCeiling(String),
    DuplicateTerm(String),
    /// Canonical-form violation while decoding a registry value.
    Malformed(&'static str),
}

/// The closed, versioned term registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermRegistry {
    /// Pinned to `canvas_syscall::vocabulary::VOCABULARY_VERSION` by
    /// `tests/vocab_binding.rs` (D11): a kernel vocabulary bump is a Braid
    /// registry event, never silent drift.
    pub vocab_version: u32,
    terms: BTreeMap<String, TermSpec>,
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
        // Pure ⇔ no capability: a pure term with authority, or an effectful
        // term without one, is unrepresentable in a registry.
        let is_pure = spec.effect == EffectClass::Pure;
        if is_pure == spec.capability.is_some() {
            return Err(RegistryError::CapabilityEffectMismatch(spec.id));
        }
        if matches!(spec.effect, EffectClass::Irreversible | EffectClass::Egress)
            && spec.egress_ceiling.is_none()
        {
            return Err(RegistryError::MissingCeiling(spec.id));
        }
        if self.terms.contains_key(&spec.id) {
            return Err(RegistryError::DuplicateTerm(spec.id));
        }
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

    // ── canonical (de)serialization ──

    pub fn to_canon(&self) -> Value {
        let terms: Vec<Value> = self.terms.values().map(term_to_canon).collect();
        Value::map(vec![
            ("terms", Value::List(terms)),
            ("vocab_version", Value::Int(self.vocab_version as i64)),
        ])
    }

    pub fn from_canon(v: &Value) -> Result<Self, RegistryError> {
        if !v.require_only_keys(&["terms", "vocab_version"]) {
            return Err(RegistryError::Malformed("registry: unknown field"));
        }
        let vocab = match v.get("vocab_version") {
            Some(Value::Int(n)) if *n >= 0 && *n <= u32::MAX as i64 => *n as u32,
            _ => return Err(RegistryError::Malformed("vocab_version")),
        };
        let mut reg = TermRegistry::new(vocab);
        let items = match v.get("terms") {
            Some(Value::List(items)) => items,
            _ => return Err(RegistryError::Malformed("terms")),
        };
        let mut prev: Option<String> = None;
        for it in items {
            let spec = term_from_canon(it)?;
            // Canonical form: terms sorted by id, no duplicates (one registry,
            // one byte form — T3 applies to the registry too).
            if let Some(p) = &prev {
                if p.as_str() >= spec.id.as_str() {
                    return Err(RegistryError::Malformed("term order"));
                }
            }
            prev = Some(spec.id.clone());
            reg.insert(spec)?;
        }
        Ok(reg)
    }
}

// ── leaf encodings ──

fn type_to_text(t: &TypeTag) -> String {
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
                    args.iter().map(type_to_text).collect::<Vec<_>>().join(", ")
                )
            }
        }
        TypeTag::List(inner) => format!("list<{}>", type_to_text(inner)),
    }
}

fn type_from_text(s: &str) -> Result<TypeTag, RegistryError> {
    Ok(match s {
        "bool" => TypeTag::Bool,
        "int" => TypeTag::Int,
        "bytes" => TypeTag::Bytes,
        "text" => TypeTag::Text,
        "cid" => TypeTag::Cid,
        _ => {
            // `list<...>` is the only core parameterized form; everything else
            // is a vocabulary `Opaque`. A label with `<...>` carries type args.
            if let Some(inner) = s.strip_prefix("list<").and_then(|r| r.strip_suffix('>')) {
                TypeTag::List(Box::new(type_from_text(inner)?))
            } else if let Some(open) = s.find('<') {
                let label = s[..open].to_string();
                let args_str = &s[open + 1..s.len() - 1];
                let args = if args_str.is_empty() {
                    Vec::new()
                } else {
                    args_str
                        .split(',')
                        .map(|a| type_from_text(a.trim()))
                        .collect::<Result<Vec<_>, _>>()?
                };
                TypeTag::Opaque(label, args)
            } else {
                TypeTag::Opaque(s.to_string(), Vec::new())
            }
        }
    })
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
    Ok(match s {
        "pure" => EffectClass::Pure,
        "read" => EffectClass::Read,
        "reversible-write" => EffectClass::ReversibleWrite,
        "irreversible" => EffectClass::Irreversible,
        "egress" => EffectClass::Egress,
        _ => return Err(RegistryError::Malformed("effect class")),
    })
}

pub(crate) fn exposure_to_int(x: Exposure) -> i64 {
    x as i64
}

fn exposure_from_int(n: i64) -> Result<Exposure, RegistryError> {
    Ok(match n {
        0 => Exposure::Public,
        1 => Exposure::Internal,
        2 => Exposure::Confidential,
        3 => Exposure::Vault,
        _ => return Err(RegistryError::Malformed("exposure")),
    })
}

fn term_to_canon(t: &TermSpec) -> Value {
    let mut fields = vec![
        ("id", Value::Text(t.id.clone())),
        (
            "inputs",
            Value::List(
                t.inputs
                    .iter()
                    .map(|i| Value::Text(type_to_text(i)))
                    .collect(),
            ),
        ),
        ("output", Value::Text(type_to_text(&t.output))),
        ("effect", Value::Text(effect_to_text(t.effect).into())),
        ("exposure", Value::Int(exposure_to_int(t.source_exposure))),
        ("cost", Value::Int(t.cost as i64)),
    ];
    // //why Display-name and not a numeric discriminant: the strum serialize
    // names ("tape.read") are the protocol-stable identifiers; enum ordinals
    // would silently re-map on any variant reorder.
    if let Some(cap) = &t.capability {
        fields.push(("capability", Value::Text(cap.to_string())));
    }
    if let Some(c) = t.egress_ceiling {
        fields.push(("ceiling", Value::Int(exposure_to_int(c))));
    }
    Value::map(fields)
}

fn term_from_canon(v: &Value) -> Result<TermSpec, RegistryError> {
    // `capability` and `ceiling` are optional; the rest required. The allowed
    // set is the exhaustive key universe — any other key is a smuggled field.
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
        return Err(RegistryError::Malformed("term: unknown field"));
    }
    let id = match v.get("id") {
        Some(Value::Text(s)) => s.clone(),
        _ => return Err(RegistryError::Malformed("term id")),
    };
    let inputs = match v.get("inputs") {
        Some(Value::List(items)) => items
            .iter()
            .map(|i| match i {
                Value::Text(s) => type_from_text(s),
                _ => Err(RegistryError::Malformed("input type")),
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err(RegistryError::Malformed("inputs")),
    };
    let output = match v.get("output") {
        Some(Value::Text(s)) => type_from_text(s)?,
        _ => return Err(RegistryError::Malformed("output")),
    };
    let effect = match v.get("effect") {
        Some(Value::Text(s)) => effect_from_text(s)?,
        _ => return Err(RegistryError::Malformed("effect")),
    };
    let source_exposure = match v.get("exposure") {
        Some(Value::Int(n)) => exposure_from_int(*n)?,
        _ => return Err(RegistryError::Malformed("exposure")),
    };
    let cost = match v.get("cost") {
        Some(Value::Int(n)) if *n >= 0 => *n as u64,
        _ => return Err(RegistryError::Malformed("cost")),
    };
    let capability = match v.get("capability") {
        None => None,
        Some(Value::Text(s)) => Some(
            Capability::from_str(s).map_err(|_| RegistryError::Malformed("unknown capability"))?,
        ),
        Some(_) => return Err(RegistryError::Malformed("capability")),
    };
    let egress_ceiling = match v.get("ceiling") {
        None => None,
        Some(Value::Int(n)) => Some(exposure_from_int(*n)?),
        Some(_) => return Err(RegistryError::Malformed("ceiling")),
    };
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
