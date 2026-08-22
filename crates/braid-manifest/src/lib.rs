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
use std::fmt;

/// Domain separator for repo-manifest CIDs — the same hashing discipline as
/// capsule/registry/project CIDs (D8/D11).
pub const MANIFEST_DOMAIN: &[u8] = b"lw.braid.repo-manifest.v1";

/// Closed archetype set (plan W5 line 70). An out-of-set value is a
/// validation error, never a stored UNKNOWN — the dimension contract has no
/// unknown member.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Archetype {
    /// Standard multi-crate workspace member.
    WorkspaceCrate,
    /// Standalone application crate.
    SingleCrateApp,
    /// Infrastructure security gate tool.
    InfraGate,
    /// Documentation-only repository.
    Docs,
}

impl Archetype {
    /// Returns the canonical kebab-case identifier for this archetype.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WorkspaceCrate => "workspace-crate",
            Self::SingleCrateApp => "single-crate-app",
            Self::InfraGate => "infra-gate",
            Self::Docs => "docs",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "workspace-crate" => Some(Self::WorkspaceCrate),
            "single-crate-app" => Some(Self::SingleCrateApp),
            "infra-gate" => Some(Self::InfraGate),
            "docs" => Some(Self::Docs),
            _ => None,
        }
    }
}

/// Closed CI-status set. `None` = the repo has no CI (evidence-backed: the
/// plan names nova and sentinel as no-CI). No UNKNOWN member exists — an
/// undocumented CI state blocks admission rather than rendering as UNKNOWN.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiStatus {
    /// Continuous integration builds cleanly.
    Green,
    /// Continuous integration is failing.
    Red,
    /// Repository has no automated CI configured.
    None,
}

impl CiStatus {
    /// Returns the canonical lowercase string identifier for this CI status.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Green => "green",
            Self::Red => "red",
            Self::None => "none",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "green" => Some(Self::Green),
            "red" => Some(Self::Red),
            "none" => Some(Self::None),
            _ => None,
        }
    }
}

/// One repo's manifest. All 8 fields are required — optionality would create
/// UNKNOWN, and the W5 verify line is "no UNKNOWN fields".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoManifest {
    /// Unique repository name.
    pub name: String,
    /// Architectural archetype classification.
    pub archetype: Archetype,
    /// Owning team or individual.
    pub owner: String,
    /// Minimum required security gate version.
    pub gate_version: String,
    /// Status of continuous integration.
    pub ci_status: CiStatus,
    /// Primary entry documentation links.
    pub entry_docs: Vec<String>,
    /// Standard reproduction commands.
    pub canonical_commands: Vec<String>,
    /// Whether local pre-commit CI runs.
    pub local_ci: bool,
}

impl RepoManifest {
    /// Content address of the canonical bytes.
    pub fn cid(&self) -> Cid {
        Cid::compute(MANIFEST_DOMAIN, &self.to_bytes())
    }

    /// Encodes the repository manifest into canonical CBOR byte representation.
    pub fn to_bytes(&self) -> Vec<u8> {
        encode(&to_canon(self))
    }

    /// Strict parse: canonical bytes only (bijection-guarded), full shape
    /// validation — the same discipline as `Capsule::from_bytes`.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ManifestError> {
        let val = decode_strict(bytes).map_err(|error| ManifestError::Canon {
            error,
            at: "RepoManifest::from_bytes",
        })?;
        from_canon(&val)
    }
}

/// Every way a manifest fails closed. None of these produce an artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// The authored JSON did not parse, or had an unknown/missing field.
    Parse {
        /// Parse error message.
        message: String,
        /// Source location of the error.
        at: &'static str,
    },
    /// A value outside a closed enum (the dimension contract).
    BadEnum {
        /// The field name.
        field: &'static str,
        /// The invalid value provided.
        value: String,
        /// Source location of the error.
        at: &'static str,
    },
    /// A required string is empty.
    EmptyField {
        /// The empty field name.
        field: &'static str,
        /// Source location of the error.
        at: &'static str,
    },
    /// A required list is empty or contains an empty entry.
    EmptyList {
        /// The list field name.
        field: &'static str,
        /// Source location of the error.
        at: &'static str,
    },
    /// A string contains a character banned by the TSV machine-line contract.
    BannedChar {
        /// The offending field name.
        field: &'static str,
        /// Source location of the error.
        at: &'static str,
    },
    /// The repo name is not a safe single path component / storage key.
    UnsafeName {
        /// The unsafe name string.
        name: String,
        /// Source location of the error.
        at: &'static str,
    },
    /// Canonical-bytes decode failure (read side).
    Canon {
        /// The underlying canonical codec error.
        error: CanonError,
        /// Source location of the error.
        at: &'static str,
    },
    /// Shape violation on canonical decode (read side).
    Malformed {
        /// The malformed field description.
        field: &'static str,
        /// Source location of the error.
        at: &'static str,
    },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse { message, at } => write!(f, "manifest parse error at {at}: {message}"),
            Self::BadEnum { field, value, at } => {
                write!(f, "field `{field}` has invalid value `{value}` at {at}")
            }
            Self::EmptyField { field, at } => write!(f, "field `{field}` is empty at {at}"),
            Self::EmptyList { field, at } => write!(f, "field `{field}` list is empty at {at}"),
            Self::BannedChar { field, at } => write!(f, "field `{field}` contains banned characters at {at}"),
            Self::UnsafeName { name, at } => write!(f, "name `{name}` is not a safe path key at {at}"),
            Self::Canon { error, at } => write!(f, "canonical decoding failed at {at}: {error:?}"),
            Self::Malformed { field, at } => write!(f, "canonical shape violation on `{field}` at {at}"),
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

fn validate_safe_name(name: &str) -> Result<(), ManifestError> {
    if !safe_name_component(name) {
        Err(ManifestError::UnsafeName {
            name: name.to_string(),
            at: "enforce_contracts",
        })
    } else {
        Ok(())
    }
}

fn validate_single_field(field: &'static str, v: &str) -> Result<(), ManifestError> {
    if v.is_empty() {
        Err(ManifestError::EmptyField {
            field,
            at: "enforce_contracts",
        })
    } else if !no_tsv_separators(v) {
        Err(ManifestError::BannedChar {
            field,
            at: "enforce_contracts",
        })
    } else {
        Ok(())
    }
}

fn validate_list_item(field: &'static str, item: &str) -> Result<(), ManifestError> {
    if item.is_empty() {
        Err(ManifestError::EmptyList {
            field,
            at: "enforce_contracts",
        })
    } else if !no_tsv_separators(item) {
        Err(ManifestError::BannedChar {
            field,
            at: "enforce_contracts",
        })
    } else {
        Ok(())
    }
}

fn check_list_not_empty(field: &'static str, is_empty: bool) -> Result<(), ManifestError> {
    if is_empty {
        Err(ManifestError::EmptyList {
            field,
            at: "enforce_contracts",
        })
    } else {
        Ok(())
    }
}

fn validate_string_list(field: &'static str, list: &[String]) -> Result<(), ManifestError> {
    check_list_not_empty(field, list.is_empty())?;
    for item in list {
        validate_list_item(field, item)?;
    }
    Ok(())
}

/// The one contract check, shared by `validate` (author side) and `from_canon`
/// (read side): write invariant == read invariant.
fn enforce_contracts(manifest: &RepoManifest) -> Result<(), ManifestError> {
    validate_safe_name(&manifest.name)?;
    validate_single_field("owner", &manifest.owner)?;
    validate_single_field("gate_version", &manifest.gate_version)?;
    validate_string_list("entry_docs", &manifest.entry_docs)?;
    validate_string_list("canonical_commands", &manifest.canonical_commands)?;
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

fn parse_archetype_field(val: &str) -> Result<Archetype, ManifestError> {
    Archetype::parse(val).ok_or_else(|| ManifestError::BadEnum {
        field: "archetype",
        value: val.to_string(),
        at: "validate",
    })
}

fn parse_ci_status_field(val: &str) -> Result<CiStatus, ManifestError> {
    CiStatus::parse(val).ok_or_else(|| ManifestError::BadEnum {
        field: "ci_status",
        value: val.to_string(),
        at: "validate",
    })
}

/// Validate an authored repo-manifest JSON document. Fail-closed: any
/// violation names the field, nothing is produced.
pub fn validate(json: &str) -> Result<RepoManifest, ManifestError> {
    let j: JsonManifest = serde_json::from_str(json).map_err(|e| ManifestError::Parse {
        message: e.to_string(),
        at: "validate",
    })?;
    let archetype = parse_archetype_field(&j.archetype)?;
    let ci_status = parse_ci_status_field(&j.ci_status)?;
    let manifest = RepoManifest {
        name: j.name,
        archetype,
        owner: j.owner,
        gate_version: j.gate_version,
        ci_status,
        entry_docs: j.entry_docs,
        canonical_commands: j.canonical_commands,
        local_ci: j.local_ci,
    };
    enforce_contracts(&manifest)?;
    Ok(manifest)
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

fn decode_text_field(v: &Value, k: &'static str) -> Result<String, ManifestError> {
    match v.get_field(k) {
        Some(Value::Text(s)) => Ok(s.clone()),
        _ => Err(ManifestError::Malformed {
            field: k,
            at: "from_canon",
        }),
    }
}

fn extract_text_item(i: &Value, k: &'static str) -> Result<String, ManifestError> {
    match i {
        Value::Text(s) => Ok(s.clone()),
        _ => Err(ManifestError::Malformed {
            field: k,
            at: "from_canon",
        }),
    }
}

fn decode_list_field(v: &Value, k: &'static str) -> Result<Vec<String>, ManifestError> {
    match v.get_field(k) {
        Some(Value::List(items)) => {
            let mut list = Vec::with_capacity(items.len());
            for item in items {
                list.push(extract_text_item(item, k)?);
            }
            Ok(list)
        }
        _ => Err(ManifestError::Malformed {
            field: k,
            at: "from_canon",
        }),
    }
}

fn decode_bool_field(v: &Value, k: &'static str) -> Result<bool, ManifestError> {
    match v.get_field(k) {
        Some(Value::Bool(b)) => Ok(*b),
        _ => Err(ManifestError::Malformed {
            field: k,
            at: "from_canon",
        }),
    }
}

fn check_canon_keys(v: &Value) -> Result<(), ManifestError> {
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
        Err(ManifestError::Malformed {
            field: "repo-manifest: unknown field",
            at: "from_canon",
        })
    } else {
        Ok(())
    }
}

fn decode_manifest_archetype(v: &Value) -> Result<Archetype, ManifestError> {
    let arch_str = decode_text_field(v, "archetype")?;
    Archetype::parse(&arch_str).ok_or(ManifestError::Malformed {
        field: "archetype",
        at: "from_canon",
    })
}

fn decode_manifest_ci_status(v: &Value) -> Result<CiStatus, ManifestError> {
    let ci_str = decode_text_field(v, "ci_status")?;
    CiStatus::parse(&ci_str).ok_or(ManifestError::Malformed {
        field: "ci_status",
        at: "from_canon",
    })
}

fn decode_manifest_metadata(
    v: &Value,
) -> Result<(String, bool, String, String), ManifestError> {
    let gate_version = decode_text_field(v, "gate_version")?;
    let local_ci = decode_bool_field(v, "local_ci")?;
    let name = decode_text_field(v, "name")?;
    let owner = decode_text_field(v, "owner")?;
    Ok((gate_version, local_ci, name, owner))
}

fn decode_manifest_lists(
    v: &Value,
) -> Result<(Vec<String>, Vec<String>), ManifestError> {
    let canonical_commands = decode_list_field(v, "canonical_commands")?;
    let entry_docs = decode_list_field(v, "entry_docs")?;
    Ok((canonical_commands, entry_docs))
}

/// Strict decode of the canonical form — the exact key universe, nothing
/// else (a smuggled field is a rejected byte form).
pub fn from_canon(v: &Value) -> Result<RepoManifest, ManifestError> {
    check_canon_keys(v)?;
    let archetype = decode_manifest_archetype(v)?;
    let ci_status = decode_manifest_ci_status(v)?;
    let (canonical_commands, entry_docs) = decode_manifest_lists(v)?;
    let (gate_version, local_ci, name, owner) = decode_manifest_metadata(v)?;
    let manifest = RepoManifest {
        archetype,
        canonical_commands,
        ci_status,
        entry_docs,
        gate_version,
        local_ci,
        name,
        owner,
    };
    enforce_contracts(&manifest)?;
    Ok(manifest)
}

/// One declared repo: its name and the pinned content address of its
/// admitted manifest. `cid == None` = declared but not yet admitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryEntry {
    /// Repository name.
    pub name: String,
    /// Pinned manifest CID if admitted.
    pub cid: Option<Cid>,
}

fn parse_inventory_cid(name: &str, h: &str) -> Result<Cid, ManifestError> {
    Cid::from_hex(h).ok_or_else(|| ManifestError::Parse {
        message: format!("inventory[{name}]: not a 64-char hex CID"),
        at: "parse_inventory",
    })
}

fn parse_inventory_item(
    name: String,
    cid_hex: Option<String>,
) -> Result<InventoryEntry, ManifestError> {
    validate_safe_name(&name)?;
    let cid = match cid_hex {
        None => None,
        Some(h) => Some(parse_inventory_cid(&name, &h)?),
    };
    Ok(InventoryEntry { name, cid })
}

fn check_raw_not_empty(is_empty: bool) -> Result<(), ManifestError> {
    if is_empty {
        Err(ManifestError::EmptyList {
            field: "inventory",
            at: "parse_inventory",
        })
    } else {
        Ok(())
    }
}

/// Parse the declared inventory — the org database, the ONE place the org
/// set is declared.
pub fn parse_inventory(json: &str) -> Result<Vec<InventoryEntry>, ManifestError> {
    let raw: BTreeMap<String, Option<String>> =
        serde_json::from_str(json).map_err(|e| ManifestError::Parse {
            message: e.to_string(),
            at: "parse_inventory",
        })?;
    check_raw_not_empty(raw.is_empty())?;
    let mut out = Vec::with_capacity(raw.len());
    for (name, cid_hex) in raw {
        let entry = parse_inventory_item(name, cid_hex)?;
        out.push(entry);
    }
    Ok(out)
}
