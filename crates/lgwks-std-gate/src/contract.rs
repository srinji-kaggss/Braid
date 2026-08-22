//! `contract` owns the human-approved dependency register and enforces
//! INV-APPROVAL-IS-SEMANTIC: an entry is not an approval unless it says *what
//! the standard library cannot do*. A name on a list is a whitelist; a name
//! with a reason, a pin, an approver, a date, and a link to the evidence is a
//! contract, and only the second one is admissible here.
//!
//! The register lives at `contract/APPROVED.toml` in the repo being gated. It
//! is valid TOML so an editor or a human can read it, but it is parsed by a
//! line-oriented reader in this module rather than by a TOML crate — taking a
//! dependency in order to police dependencies would be self-refuting. The
//! reader refuses any line it does not recognise instead of skipping it, so a
//! typo cannot quietly become an unenforced entry.
//!
//! Approval is a diff. There is no command that adds an entry: a human writes
//! the block and commits it, which is what makes the approval reviewable and
//! attributable. `lgwks-gate request` only prints the block to be filled in.

use std::error::Error;
use std::fmt;

// ── The register ────────────────────────────────────────────────────────────

/// Where an approved crate sits in the ladder. Only two tiers are admissible:
/// ELIMINATE and CONSOLIDATE crates do not get entries, they get an
/// `lgwks_std` module, and an entry claiming either tier is a category error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// Out of scope for reimplementation — kept as a direct dependency.
    Boundary,
    /// Kept as audited upstream source under `vendor/`, not as a registry edge.
    Vendor,
}

impl Tier {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "boundary" => Some(Self::Boundary),
            "vendor" => Some(Self::Vendor),
            _ => None,
        }
    }
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Boundary => "boundary",
            Self::Vendor => "vendor",
        })
    }
}

/// One approved dependency, with the evidence that justified it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Package name as `Cargo.lock` spells it.
    pub krate: String,
    /// Which tier the approval sits in.
    pub tier: Tier,
    /// Approved version. Matching is prefix-on-a-dot-boundary, so `1.0` admits
    /// `1.0.219` and `1.0.219` admits only itself — the author picks the
    /// strictness by how much of the version they write down.
    pub version: String,
    /// One sentence naming what the standard library cannot do.
    pub reason: String,
    /// The human who approved it.
    pub approved_by: String,
    /// ISO date of approval.
    pub approved_on: String,
    /// Path or URL to the evidence behind the approval.
    pub review: String,
    /// Line where the entry opened, for diagnosis.
    pub line: usize,
}

/// The parsed register.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contract {
    /// When false, refusals are reported as warnings instead of failing the
    /// build. Adoption-only: flipping it is a reviewable diff in the register
    /// itself, never an environment variable a process can set for itself.
    pub enforce: bool,
    /// Every approved dependency.
    pub entries: Vec<Entry>,
}

// ── Errors ──────────────────────────────────────────────────────────────────

/// Why a register is not a contract. Each variant carries the line so the
/// message can be pasted straight into an editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractError {
    /// A line matched neither a section header, a key/value pair, nor a comment.
    Malformed {
        /// One-based line number.
        line: usize,
        /// The offending line, trimmed.
        text: String,
    },
    /// A key appeared that the schema does not define.
    UnknownKey {
        /// One-based line number.
        line: usize,
        /// The offending key.
        key: String,
    },
    /// A key/value pair appeared before any section header.
    OrphanKey {
        /// One-based line number.
        line: usize,
        /// The offending key.
        key: String,
    },
    /// A required field was absent from an entry.
    MissingField {
        /// The entry's crate name, or `<unnamed>`.
        krate: String,
        /// The absent field.
        field: &'static str,
    },
    /// `tier` held a value outside `boundary` / `vendor`.
    BadTier {
        /// One-based line number.
        line: usize,
        /// The offending value.
        value: String,
    },
    /// `approved_on` was not an ISO `YYYY-MM-DD` date.
    BadDate {
        /// The entry's crate name.
        krate: String,
        /// The offending value.
        value: String,
    },
    /// `reason` did not name what the standard library cannot do. A reason must
    /// be a sentence — at least four words, at least 24 characters, ending in a
    /// full stop — and must not merely restate the crate's name.
    ThinReason {
        /// The entry's crate name.
        krate: String,
    },
    /// The same crate was approved twice.
    DuplicateEntry {
        /// The repeated crate name.
        krate: String,
        /// One-based line where the duplicate opened.
        line: usize,
    },
}

impl fmt::Display for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { line, text } => {
                write!(f, "line {line}: cannot parse {text:?}")
            }
            Self::UnknownKey { line, key } => {
                write!(f, "line {line}: unknown key {key:?}")
            }
            Self::OrphanKey { line, key } => {
                write!(f, "line {line}: key {key:?} appears before any section header")
            }
            Self::MissingField { krate, field } => {
                write!(f, "approval for {krate:?} is missing required field {field:?}")
            }
            Self::BadTier { line, value } => {
                write!(f, "line {line}: tier {value:?} is not 'boundary' or 'vendor'")
            }
            Self::BadDate { krate, value } => {
                write!(f, "approval for {krate:?} has approved_on {value:?}, want YYYY-MM-DD")
            }
            Self::ThinReason { krate } => write!(
                f,
                "approval for {krate:?} needs a reason naming what std cannot do — \
                 a sentence of four or more words ending in a full stop"
            ),
            Self::DuplicateEntry { krate, line } => {
                write!(f, "line {line}: {krate:?} is already approved")
            }
        }
    }
}

impl Error for ContractError {}

// ── Parsing ─────────────────────────────────────────────────────────────────

const REQUIRED: [&str; 7] =
    ["crate", "tier", "version", "reason", "approved_by", "approved_on", "review"];

#[derive(Default)]
struct Draft {
    line: usize,
    fields: Vec<(&'static str, String)>,
}

impl Draft {
    fn get(&self, key: &str) -> Option<&str> {
        self.fields.iter().find(|(k, _)| *k == key).map(|(_, v)| v.as_str())
    }
}

enum Section {
    None,
    Policy,
    Approved,
}

impl Contract {
    /// Parses a register. Fail-closed: an unrecognised line is an error, not a
    /// line to skip.
    pub fn parse(text: &str) -> Result<Self, ContractError> {
        let mut enforce = true;
        let mut drafts: Vec<Draft> = Vec::new();
        let mut section = Section::None;

        for (index, raw) in text.lines().enumerate() {
            let line_no = index + 1;
            let line = strip_comment(raw).trim();
            if line.is_empty() {
                continue;
            }
            match line {
                "[policy]" => {
                    section = Section::Policy;
                    continue;
                }
                "[[approved]]" => {
                    section = Section::Approved;
                    drafts.push(Draft { line: line_no, fields: Vec::new() });
                    continue;
                }
                _ if line.starts_with('[') => {
                    return Err(ContractError::Malformed { line: line_no, text: line.to_string() })
                }
                _ => {}
            }

            let (key, value) = split_pair(line)
                .ok_or_else(|| ContractError::Malformed { line: line_no, text: line.to_string() })?;

            match section {
                Section::None => {
                    return Err(ContractError::OrphanKey { line: line_no, key: key.to_string() })
                }
                Section::Policy => match key {
                    "enforce" => enforce = value == "true",
                    _ => {
                        return Err(ContractError::UnknownKey {
                            line: line_no,
                            key: key.to_string(),
                        })
                    }
                },
                Section::Approved => {
                    let known = REQUIRED.iter().find(|k| **k == key).ok_or_else(|| {
                        ContractError::UnknownKey { line: line_no, key: key.to_string() }
                    })?;
                    let draft = drafts.last_mut().expect("approved section implies a draft");
                    draft.fields.push((known, unquote(value).to_string()));
                }
            }
        }

        let mut entries: Vec<Entry> = Vec::new();
        for draft in &drafts {
            let entry = build(draft)?;
            if entries.iter().any(|e| normalise(&e.krate) == normalise(&entry.krate)) {
                return Err(ContractError::DuplicateEntry {
                    krate: entry.krate,
                    line: draft.line,
                });
            }
            entries.push(entry);
        }
        Ok(Self { enforce, entries })
    }

    /// Finds the approval for a resolved package, tolerating `-`/`_` spelling
    /// drift between a manifest and a lock file.
    pub fn approval_for(&self, krate: &str) -> Option<&Entry> {
        let wanted = normalise(krate);
        self.entries.iter().find(|e| normalise(&e.krate) == wanted)
    }
}

fn build(draft: &Draft) -> Result<Entry, ContractError> {
    let krate = draft.get("crate").unwrap_or("<unnamed>").to_string();
    for field in REQUIRED {
        let present = draft.get(field).is_some_and(|v| !v.trim().is_empty());
        if !present {
            return Err(ContractError::MissingField { krate, field });
        }
    }
    let tier_text = draft.get("tier").expect("checked above");
    let tier = Tier::parse(tier_text)
        .ok_or_else(|| ContractError::BadTier { line: draft.line, value: tier_text.to_string() })?;
    let approved_on = draft.get("approved_on").expect("checked above").to_string();
    if !is_iso_date(&approved_on) {
        return Err(ContractError::BadDate { krate, value: approved_on });
    }
    let reason = draft.get("reason").expect("checked above").to_string();
    if !is_a_sentence(&reason, &krate) {
        return Err(ContractError::ThinReason { krate });
    }
    Ok(Entry {
        krate,
        tier,
        version: draft.get("version").expect("checked above").to_string(),
        reason,
        approved_by: draft.get("approved_by").expect("checked above").to_string(),
        approved_on,
        review: draft.get("review").expect("checked above").to_string(),
        line: draft.line,
    })
}

// ── Field rules ─────────────────────────────────────────────────────────────

/// A reason has to be a sentence that says something. The floor is deliberately
/// low enough to pass any honest justification and high enough to fail
/// `reason = "needed"` or `reason = "serde"`.
fn is_a_sentence(reason: &str, krate: &str) -> bool {
    let trimmed = reason.trim();
    if trimmed.len() < 24 || !trimmed.ends_with('.') {
        return false;
    }
    if trimmed.split_whitespace().count() < 4 {
        return false;
    }
    normalise(trimmed.trim_end_matches('.')) != normalise(krate)
}

fn is_iso_date(value: &str) -> bool {
    let b = value.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && [0, 1, 2, 3, 5, 6, 8, 9].iter().all(|&i| b[i].is_ascii_digit())
}

/// Cargo treats `-` and `_` as interchangeable in package names; so does this.
fn normalise(name: &str) -> String {
    name.trim().to_ascii_lowercase().replace('-', "_")
}

// ── Line reading ────────────────────────────────────────────────────────────

/// Drops a trailing `#` comment, respecting quotes so a `#` inside a reason or
/// a review URL survives.
fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_quotes = false;
    let mut escaped = false;
    for (i, &b) in bytes.iter().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        match b {
            b'\\' if in_quotes => escaped = true,
            b'"' => in_quotes = !in_quotes,
            b'#' if !in_quotes => return &line[..i],
            _ => {}
        }
    }
    line
}

fn split_pair(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once('=')?;
    let key = key.trim();
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some((key, value.trim()))
}

fn unquote(value: &str) -> &str {
    value.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(value)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(extra: &str) -> String {
        format!(
            concat!(
                "[[approved]]\n",
                "crate = \"serde\"\n",
                "tier = \"boundary\"\n",
                "version = \"1.0\"\n",
                "reason = \"Derive-based serialization needs compiler introspection std does not expose.\"\n",
                "approved_by = \"Director\"\n",
                "approved_on = \"2026-08-19\"\n",
                "review = \"docs/ADMISSION.md\"\n",
                "{}"
            ),
            extra
        )
    }

    #[test]
    fn a_complete_entry_parses() {
        let contract = Contract::parse(&entry("")).unwrap();
        assert!(contract.enforce);
        assert_eq!(contract.entries.len(), 1);
        assert_eq!(contract.entries[0].tier, Tier::Boundary);
        assert_eq!(contract.entries[0].version, "1.0");
    }

    #[test]
    fn enforcement_defaults_to_on_when_no_policy_is_written() {
        assert!(Contract::parse(&entry("")).unwrap().enforce);
    }

    #[test]
    fn policy_can_stand_enforcement_down_for_adoption() {
        let text = format!("[policy]\nenforce = false\n\n{}", entry(""));
        assert!(!Contract::parse(&text).unwrap().enforce);
    }

    #[test]
    fn a_missing_field_is_refused() {
        let text = "[[approved]]\ncrate = \"serde\"\ntier = \"boundary\"\n";
        assert_eq!(
            Contract::parse(text),
            Err(ContractError::MissingField { krate: "serde".into(), field: "version" })
        );
    }

    #[test]
    fn a_reason_that_restates_the_crate_name_is_refused() {
        let text = "[[approved]]\ncrate = \"serde\"\ntier = \"boundary\"\nversion = \"1\"\n\
                    reason = \"serde\"\napproved_by = \"D\"\napproved_on = \"2026-08-19\"\n\
                    review = \"x\"\n";
        assert_eq!(Contract::parse(text), Err(ContractError::ThinReason { krate: "serde".into() }));
    }

    #[test]
    fn a_reason_that_is_not_a_sentence_is_refused() {
        let text = "[[approved]]\ncrate = \"serde\"\ntier = \"boundary\"\nversion = \"1\"\n\
                    reason = \"needed for the thing\"\napproved_by = \"D\"\n\
                    approved_on = \"2026-08-19\"\nreview = \"x\"\n";
        assert_eq!(Contract::parse(text), Err(ContractError::ThinReason { krate: "serde".into() }));
    }

    #[test]
    fn an_eliminate_tier_entry_is_a_category_error() {
        let text = entry("").replace("boundary", "eliminate");
        assert!(matches!(Contract::parse(&text), Err(ContractError::BadTier { .. })));
    }

    #[test]
    fn a_malformed_date_is_refused() {
        let text = entry("").replace("2026-08-19", "19/08/2026");
        assert!(matches!(Contract::parse(&text), Err(ContractError::BadDate { .. })));
    }

    #[test]
    fn an_unknown_key_is_refused_rather_than_skipped() {
        let text = entry("notes = \"whatever\"\n");
        assert_eq!(
            Contract::parse(&text),
            Err(ContractError::UnknownKey { line: 9, key: "notes".into() })
        );
    }

    #[test]
    fn a_duplicate_approval_is_refused() {
        let text = format!("{}\n{}", entry(""), entry(""));
        assert!(matches!(Contract::parse(&text), Err(ContractError::DuplicateEntry { .. })));
    }

    #[test]
    fn a_key_before_any_section_is_refused() {
        assert_eq!(
            Contract::parse("crate = \"serde\"\n"),
            Err(ContractError::OrphanKey { line: 1, key: "crate".into() })
        );
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let text = format!("# header comment\n\n{}", entry(""));
        assert_eq!(Contract::parse(&text).unwrap().entries.len(), 1);
    }

    #[test]
    fn a_hash_inside_a_quoted_value_survives() {
        let text = entry("").replace("docs/ADMISSION.md", "docs/ADMISSION.md#tiers");
        assert_eq!(Contract::parse(&text).unwrap().entries[0].review, "docs/ADMISSION.md#tiers");
    }

    #[test]
    fn lookup_tolerates_hyphen_underscore_drift() {
        let text = entry("").replace("\"serde\"", "\"serde-json\"");
        let contract = Contract::parse(&text).unwrap();
        assert!(contract.approval_for("serde_json").is_some());
    }
}
