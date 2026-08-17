//! # braid-manifest — repo-manifest artifacts (W5)
//!
//! The org map ("pick up a repo, get the full summary of everything that is
//! ours") as a content-addressed, fail-closed artifact.
//!
//! ## Why a sibling format, not a capsule (Director decision 2026-08-16)
//! The plan says "Repo-manifest capsule", but strand payloads are valueless
//! (`Strand { term, inputs }` carries no operand), literal payloads are
//! D8-locked substrate work (DEBT_REGISTER.md), and the plan's non-goals
//! forbid changing admission semantics. So W5 ships a sibling validated
//! format with the SAME discipline as capsules: canonical CBOR bytes, BLAKE3
//! CID under a `lw.braid.*` domain, strict bijection-guarded decode,
//! validate-at-every-boundary. When the Strand-literal unit lands, this
//! artifact graduates to true capsule form — `validate()` is the migration
//! surface.
//!
//! ## Authority boundary
//! braid-verify remains the sole admission authority for CAPSULES. A repo
//! manifest is inventory metadata — no capabilities, no effects, nothing for
//! the verifier to admit — so this crate builds no second verifier. Every
//! consumer (`braid store put`, `braid catalog`, `braid summary`) shares this
//! one `validate()` and one codec: one concept, one implementation.

use braid_ir::canon::{decode_strict, encode, CanonError};
use braid_ir::cid::Cid;
use braid_ir::Value;
use serde::Deserialize;

use std::collections::BTreeMap;

/// Domain separator for repo-manifest CIDs — the same hashing discipline as
/// capsule/registry/project CIDs (D8/D11).
pub const MANIFEST_DOMAIN: &[u8] = b"lw.braid.repo-manifest.v1";

/// Closed archetype set (plan W5 line 70). An out-of-set value is a
/// validation error, never a stored UNKNOWN — the dimension contract has no
/// unknown member.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Archetype {
    WorkspaceCrate,
    SingleCrateApp,
    InfraGate,
    Docs,
}

impl Archetype {
    pub fn as_str(self) -> &'static str {
        match self {
            Archetype::WorkspaceCrate => "workspace-crate",
            Archetype::SingleCrateApp => "single-crate-app",
            Archetype::InfraGate => "infra-gate",
            Archetype::Docs => "docs",
        }
    }

    fn parse(s: &str) -> Option<Archetype> {
        match s {
            "workspace-crate" => Some(Archetype::WorkspaceCrate),
            "single-crate-app" => Some(Archetype::SingleCrateApp),
            "infra-gate" => Some(Archetype::InfraGate),
            "docs" => Some(Archetype::Docs),
            _ => None,
        }
    }
}

/// Closed CI-status set. `None` = the repo has no CI (evidence-backed: the
/// plan names nova and sentinel as no-CI). No UNKNOWN member exists — an
/// undocumented CI state blocks admission rather than rendering as UNKNOWN.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiStatus {
    Green,
    Red,
    None,
}

impl CiStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            CiStatus::Green => "green",
            CiStatus::Red => "red",
            CiStatus::None => "none",
        }
    }

    fn parse(s: &str) -> Option<CiStatus> {
        match s {
            "green" => Some(CiStatus::Green),
            "red" => Some(CiStatus::Red),
            "none" => Some(CiStatus::None),
            _ => None,
        }
    }
}

/// One repo's manifest. All 8 fields are required — optionality would create
/// UNKNOWN, and the W5 verify line is "no UNKNOWN fields".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoManifest {
    pub name: String,
    pub archetype: Archetype,
    pub owner: String,
    pub gate_version: String,
    pub ci_status: CiStatus,
    pub entry_docs: Vec<String>,
    pub canonical_commands: Vec<String>,
    pub local_ci: bool,
}

impl RepoManifest {
    /// Content address of the canonical bytes.
    pub fn cid(&self) -> Cid {
        Cid::compute(MANIFEST_DOMAIN, &self.to_bytes())
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        encode(&to_canon(self))
    }

    /// Strict parse: canonical bytes only (bijection-guarded), full shape
    /// validation — the same discipline as `Capsule::from_bytes`.
    pub fn from_bytes(bytes: &[u8]) -> Result<RepoManifest, ManifestError> {
        from_canon(&decode_strict(bytes).map_err(ManifestError::Canon)?)
    }
}

/// Every way a manifest fails closed. None of these produce an artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// The authored JSON did not parse, or had an unknown/missing field
    /// (serde names the offending field in the message).
    Parse(String),
    /// A value outside a closed enum (the dimension contract).
    BadEnum { field: &'static str, value: String },
    /// A required string is empty.
    EmptyField(&'static str),
    /// A required list is empty or contains an empty entry.
    EmptyList(&'static str),
    /// A string contains a character banned by the TSV machine-line contract
    /// (tab, newline, comma — comma is the join character).
    BannedChar { field: &'static str },
    /// The repo name is not a safe single path component / storage key.
    UnsafeName(String),
    /// Canonical-bytes decode failure (read side).
    Canon(CanonError),
    /// Shape violation on canonical decode (read side).
    Malformed(&'static str),
}

impl core::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ManifestError::Parse(m) => write!(f, "manifest parse error: {m}"),
            ManifestError::BadEnum { field, value } => {
                write!(
                    f,
                    "field `{field}` has value `{value}`, outside its closed set"
                )
            }
            ManifestError::EmptyField(k) => write!(f, "field `{k}` is empty"),
            ManifestError::EmptyList(k) => write!(f, "field `{k}` is empty or has an empty entry"),
            ManifestError::BannedChar { field } => write!(
                f,
                "field `{field}` contains tab, newline, or comma (banned by the \
                 machine-line contract)"
            ),
            ManifestError::UnsafeName(n) => write!(
                f,
                "name `{n}` is not a safe storage key (want a single path component: \
                 [A-Za-z0-9._-]+, no leading `.`)"
            ),
            ManifestError::Canon(e) => write!(f, "not canonical bytes: {e:?}"),
            ManifestError::Malformed(k) => write!(f, "canonical shape violation: {k}"),
        }
    }
}

impl std::error::Error for ManifestError {}

/// A safe storage key: one path component — `[A-Za-z0-9._-]+`, no leading
/// `.`, not `.` or `..`. Implies no `/`, `\`, NUL, control chars, or TSV
/// separators (tab/newline/comma are outside the alphabet).
pub fn safe_name_component(name: &str) -> bool {
    if name.is_empty() || name.starts_with('.') {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

/// No TSV separators in a free-text field — the comma-joined machine line
/// must round-trip losslessly.
fn no_tsv_separators(s: &str) -> bool {
    !s.chars().any(|c| c == '\t' || c == '\n' || c == ',')
}

/// The one contract check, shared by `validate` (author side) and `from_canon`
/// (read side): write invariant == read invariant.
fn enforce_contracts(m: &RepoManifest) -> Result<(), ManifestError> {
    if !safe_name_component(&m.name) {
        return Err(ManifestError::UnsafeName(m.name.clone()));
    }
    for (field, v) in [("owner", &m.owner), ("gate_version", &m.gate_version)] {
        if v.is_empty() {
            return Err(ManifestError::EmptyField(field));
        }
        if !no_tsv_separators(v) {
            return Err(ManifestError::BannedChar { field });
        }
    }
    for (field, list) in [
        ("entry_docs", &m.entry_docs),
        ("canonical_commands", &m.canonical_commands),
    ] {
        if list.is_empty() {
            return Err(ManifestError::EmptyList(field));
        }
        for item in list {
            if item.is_empty() {
                return Err(ManifestError::EmptyList(field));
            }
            if !no_tsv_separators(item) {
                return Err(ManifestError::BannedChar { field });
            }
        }
    }
    Ok(())
}

/// Authored JSON shape. `deny_unknown_fields` mirrors the capsule's
/// anti-smuggling discipline: an unrecognized key is a rejected author input,
/// never silently dropped (T3).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonManifest {
    name: String,
    archetype: String,
    owner: String,
    gate_version: String,
    ci_status: String,
    entry_docs: Vec<String>,
    canonical_commands: Vec<String>,
    local_ci: bool,
}

/// Validate an authored repo-manifest JSON document. Fail-closed: any
/// violation names the field, nothing is produced.
pub fn validate(json: &str) -> Result<RepoManifest, ManifestError> {
    let j: JsonManifest =
        serde_json::from_str(json).map_err(|e| ManifestError::Parse(e.to_string()))?;
    let archetype = Archetype::parse(&j.archetype).ok_or(ManifestError::BadEnum {
        field: "archetype",
        value: j.archetype.clone(),
    })?;
    let ci_status = CiStatus::parse(&j.ci_status).ok_or(ManifestError::BadEnum {
        field: "ci_status",
        value: j.ci_status.clone(),
    })?;
    let m = RepoManifest {
        name: j.name,
        archetype,
        owner: j.owner,
        gate_version: j.gate_version,
        ci_status,
        entry_docs: j.entry_docs,
        canonical_commands: j.canonical_commands,
        local_ci: j.local_ci,
    };
    enforce_contracts(&m)?;
    Ok(m)
}

/// Canonical form — fixed key set; the encoder re-sorts map keys at the
/// boundary (`canon::key_cmp`), so this is deterministic by construction.
pub fn to_canon(m: &RepoManifest) -> Value {
    let text_list =
        |items: &[String]| Value::List(items.iter().map(|s| Value::Text(s.clone())).collect());
    Value::map(vec![
        ("archetype", Value::Text(m.archetype.as_str().into())),
        ("canonical_commands", text_list(&m.canonical_commands)),
        ("ci_status", Value::Text(m.ci_status.as_str().into())),
        ("entry_docs", text_list(&m.entry_docs)),
        ("gate_version", Value::Text(m.gate_version.clone())),
        ("local_ci", Value::Bool(m.local_ci)),
        ("name", Value::Text(m.name.clone())),
        ("owner", Value::Text(m.owner.clone())),
    ])
}

/// Strict decode of the canonical form — the exact key universe, nothing
/// else (a smuggled field is a rejected byte form).
pub fn from_canon(v: &Value) -> Result<RepoManifest, ManifestError> {
    if !v.require_only_keys(&[
        "archetype",
        "canonical_commands",
        "ci_status",
        "entry_docs",
        "gate_version",
        "local_ci",
        "name",
        "owner",
    ]) {
        return Err(ManifestError::Malformed("repo-manifest: unknown field"));
    }
    let text = |k: &'static str| -> Result<String, ManifestError> {
        match v.get(k) {
            Some(Value::Text(s)) => Ok(s.clone()),
            _ => Err(ManifestError::Malformed(k)),
        }
    };
    let list = |k: &'static str| -> Result<Vec<String>, ManifestError> {
        match v.get(k) {
            Some(Value::List(items)) => items
                .iter()
                .map(|i| match i {
                    Value::Text(s) => Ok(s.clone()),
                    _ => Err(ManifestError::Malformed(k)),
                })
                .collect(),
            _ => Err(ManifestError::Malformed(k)),
        }
    };
    let local_ci = match v.get("local_ci") {
        Some(Value::Bool(b)) => *b,
        _ => return Err(ManifestError::Malformed("local_ci")),
    };
    let m = RepoManifest {
        archetype: Archetype::parse(&text("archetype")?)
            .ok_or(ManifestError::Malformed("archetype"))?,
        canonical_commands: list("canonical_commands")?,
        ci_status: CiStatus::parse(&text("ci_status")?)
            .ok_or(ManifestError::Malformed("ci_status"))?,
        entry_docs: list("entry_docs")?,
        gate_version: text("gate_version")?,
        local_ci,
        name: text("name")?,
        owner: text("owner")?,
    };
    enforce_contracts(&m)?;
    Ok(m)
}

/// One declared repo: its name and the pinned content address of its
/// admitted manifest. `cid == None` = declared but not yet admitted — the
/// catalog refuses to render until every declaration is pinned (no UNKNOWN,
/// no silent partial map).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryEntry {
    pub name: String,
    pub cid: Option<Cid>,
}

/// Parse the declared inventory — the org database, the ONE place the org
/// set is declared: a JSON object mapping repo name → pinned manifest CID
/// (64 hex chars) or `null` (declared, not yet admitted). Keys must be safe
/// name components. The pin is what makes the store tamper-evident: catalog
/// re-hashes every artifact and denies on any pin mismatch.
pub fn parse_inventory(json: &str) -> Result<Vec<InventoryEntry>, ManifestError> {
    let raw: BTreeMap<String, Option<String>> =
        serde_json::from_str(json).map_err(|e| ManifestError::Parse(e.to_string()))?;
    if raw.is_empty() {
        return Err(ManifestError::EmptyList("inventory"));
    }
    let mut out = Vec::with_capacity(raw.len());
    for (name, cid_hex) in raw {
        if !safe_name_component(&name) {
            return Err(ManifestError::UnsafeName(name));
        }
        let cid = match cid_hex {
            None => None,
            Some(h) => {
                let cid = Cid::from_hex(&h).ok_or_else(|| {
                    ManifestError::Parse(format!("inventory[{name}]: not a 64-char hex CID"))
                })?;
                Some(cid)
            }
        };
        out.push(InventoryEntry { name, cid });
    }
    Ok(out)
}
