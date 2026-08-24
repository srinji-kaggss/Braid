//! # braid-project — multi-capsule project toolchain (U13, D-TOOLCHAIN)
//!
//! A **project** is a set of named capsules. This crate reads a project
//! manifest (JSON), elaborates every capsule's JS source through
//! `braid-elaborate-js`, admits each through the **one** `braid-verify`, and —
//! only if *all* admit — emits a deterministic **project CID**. It is the first
//! step toward a `braid build` for projects of many capsules; it is a
//! consumer/toolchain crate, **not** trust-base: it adds zero authority and
//! builds no second verifier.
//!
//! ## Manifest
//! ```json
//! { "name": "demo", "capsules": [ { "name": "greeting", "source": "\"hi\" + \"!\"" } ] }
//! ```
//!
//! ## Anti-dredging posture (cross-capsule)
//! The dredging risk a *project* adds over a single capsule is **aggregation**:
//! a build step that quietly pools authority, wires capsules together, or
//! admits partially. This toolchain refuses all three by construction:
//! - **No authority aggregation.** Each capsule is elaborated and verified
//!   *independently under the empty ambient set* — exactly as it would be alone
//!   (`build` calls the same `elaborate_and_admit`). A capsule's CID inside a
//!   project equals its standalone CID; the project never rewrites or re-wires
//!   it, so no capsule gains authority from its neighbours (T1/T5).
//! - **Fail-closed, never partial.** One capsule that fails to elaborate or
//!   that the verifier rejects fails the **whole** build, naming the capsule.
//!   There is no partial-admit surface to exploit.
//! - **No shadowing.** Duplicate capsule names are rejected — a second capsule
//!   cannot hide behind a name already taken.

use std::collections::BTreeSet;

use braid_elaborate_js::{ElabError, elaborate_and_admit};
use braid_ir::{Capsule, Cid};
use braid_verify::Verdict;
use serde::Deserialize;

pub mod cli;

/// Re-exported so the CLI and tooling name the emission type without a direct
/// dependency on the elaborator crate.
pub use braid_vocab_rust::RustCrate;

/// Domain separator for the project CID — BLAKE3 under `lw.braid.*`, the same
/// hashing discipline as the substrate's capsule/registry CIDs (D8/D11). A
/// project CID is a *build-tool* artifact (reproducibility anchor), not a
/// verified capsule.
const PROJECT_DOMAIN: &[u8] = b"lw.braid.project.v1";

/// A project manifest.
#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    pub name: String,
    pub capsules: Vec<CapsuleSource>,
}

/// One named capsule's source.
#[derive(Debug, Clone, Deserialize)]
pub struct CapsuleSource {
    pub name: String,
    pub source: String,
}

/// Every way a build fails closed. None yields a partial report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectError {
    /// A manifest with no capsules.
    Empty {
        /// Source location of the error.
        at: &'static str,
    },
    /// Two capsules share a name (no shadowing).
    DuplicateName {
        /// The duplicated name.
        name: String,
        /// Source location of the error.
        at: &'static str,
    },
    /// The manifest JSON did not parse.
    Parse {
        /// Parse failure description.
        message: String,
        /// Source location of the error.
        at: &'static str,
    },
    /// A capsule's source failed to elaborate (carries the frontend error).
    CapsuleElaboration {
        /// Capsule name that failed.
        name: String,
        /// The underlying frontend error.
        error: ElabError,
        /// Source location of the error.
        at: &'static str,
    },
    /// A capsule elaborated but the verifier rejected it.
    CapsuleRejected {
        /// Capsule name that was rejected.
        name: String,
        /// Rejection reason from the verifier.
        reason: String,
        /// Source location of the error.
        at: &'static str,
    },
}

impl core::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Empty { at } => write!(f, "project has no capsules at {at}"),
            Self::DuplicateName { name, at } => {
                write!(f, "duplicate capsule name `{name}` at {at}")
            }
            Self::Parse { message, at } => write!(f, "manifest parse error at {at}: {message}"),
            Self::CapsuleElaboration { name, error, at } => {
                write!(f, "capsule `{name}` failed elaboration at {at}: {error}")
            }
            Self::CapsuleRejected { name, reason, at } => {
                write!(f, "capsule `{name}` rejected by verifier at {at}: {reason}")
            }
        }
    }
}

impl std::error::Error for ProjectError {}

/// One built, admitted capsule.
#[derive(Debug)]
pub struct CapsuleEntry {
    pub name: String,
    pub capsule: Capsule,
    pub cid: Cid,
}

/// The result of a successful build: every capsule admitted.
#[derive(Debug)]
pub struct BuildReport {
    pub name: String,
    pub project_cid: Cid,
    pub entries: Vec<CapsuleEntry>,
}

/// Deserializes a [`Project`] schema structure from a JSON string slice.
pub fn parse_project(json: &str) -> Result<Project, ProjectError> {
    lgwks_std::json::from_str(json).map_err(|json_err| ProjectError::Parse {
        message: json_err.to_string(),
        at: "parse_project",
    })
}

fn check_project_non_empty(capsules: &[CapsuleSource]) -> Result<(), ProjectError> {
    if capsules.is_empty() {
        Err(ProjectError::Empty { at: "build" })
    } else {
        Ok(())
    }
}

fn check_name_unique<'a>(seen: &mut BTreeSet<&'a str>, name: &'a str) -> Result<(), ProjectError> {
    if !seen.insert(name) {
        Err(ProjectError::DuplicateName {
            name: name.to_string(),
            at: "build::shadowing",
        })
    } else {
        Ok(())
    }
}

fn check_no_duplicate_names(capsules: &[CapsuleSource]) -> Result<(), ProjectError> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for item in capsules {
        check_name_unique(&mut seen, item.name.as_str())?;
    }
    Ok(())
}

fn verify_capsule_verdict(name: &str, verdict: &Verdict) -> Result<(), ProjectError> {
    match verdict {
        Verdict::Admit { .. } => Ok(()),
        Verdict::Reject { stage, reason } => Err(ProjectError::CapsuleRejected {
            name: name.to_string(),
            reason: format!("{stage:?}: {reason}"),
            at: "build::verify",
        }),
    }
}

fn build_single_capsule(c: &CapsuleSource) -> Result<CapsuleEntry, ProjectError> {
    let elaborated =
        elaborate_and_admit(&c.source).map_err(|error| ProjectError::CapsuleElaboration {
            name: c.name.clone(),
            error,
            at: "build::elaborate",
        })?;
    verify_capsule_verdict(&c.name, &elaborated.verdict)?;
    let cid = elaborated.capsule.cid();
    Ok(CapsuleEntry {
        name: c.name.clone(),
        capsule: elaborated.capsule,
        cid,
    })
}

/// Build a project: elaborate + admit every capsule, fail-closed on the first
/// failure, then compute the project CID. Each capsule is verified
/// independently under the empty ambient set — the project pools no authority.
pub fn build(project: &Project) -> Result<BuildReport, ProjectError> {
    check_project_non_empty(&project.capsules)?;
    check_no_duplicate_names(&project.capsules)?;

    let mut entries = Vec::with_capacity(project.capsules.len());
    for item in &project.capsules {
        let entry = build_single_capsule(item)?;
        entries.push(entry);
    }

    let project_cid = compute_project_cid(&entries);
    Ok(BuildReport {
        name: project.name.clone(),
        project_cid,
        entries,
    })
}

/// Builds a project from a raw JSON manifest string slice.
pub fn build_from_json(json: &str) -> Result<BuildReport, ProjectError> {
    let project = parse_project(json)?;
    build(&project)
}

fn elaborate_entry_to_rust(
    registry: &braid_ir::TermRegistry,
    entry: &CapsuleEntry,
) -> Result<(String, RustCrate), ProjectError> {
    let rust_crate = braid_vocab_rust::elaborate(registry, &entry.capsule).map_err(|elab_err| {
        ProjectError::CapsuleElaboration {
            name: entry.name.clone(),
            error: ElabError::Build(elab_err.to_string()),
            at: "build_rust",
        }
    })?;
    Ok((entry.name.clone(), rust_crate))
}

/// Elaborates every capsule in the project to a [`RustCrate`].
pub fn build_rust(project: &Project) -> Result<Vec<(String, RustCrate)>, ProjectError> {
    let report = build(project)?;
    let registry = braid_vocab_js::registry_v0();
    let mut out = Vec::with_capacity(report.entries.len());
    for entry in &report.entries {
        let pair = elaborate_entry_to_rust(&registry, entry)?;
        out.push(pair);
    }
    Ok(out)
}

fn format_entry_row(entry: &CapsuleEntry) -> String {
    format!("{}\u{0}{}", entry.name, entry.cid.to_hex())
}

/// The project CID: order-independent over the `(name, capsule_cid)` set.
fn compute_project_cid(entries: &[CapsuleEntry]) -> Cid {
    let mut rows: Vec<String> = entries.iter().map(format_entry_row).collect();
    rows.sort();
    let joined = rows.join("\n");
    Cid::compute(PROJECT_DOMAIN, joined.as_bytes())
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_MANIFEST: &str = r#"{
        "name": "demo",
        "capsules": [
            { "name": "a", "source": "1 + 2" },
            { "name": "b", "source": "\"hi\" + \"!\"" }
        ]
    }"#;

    #[test]
    fn valid_project_builds_and_hashes() {
        let p = parse_project(VALID_MANIFEST).unwrap();
        let report = build(&p).unwrap();
        assert_eq!(report.name, "demo");
        assert_eq!(report.entries.len(), 2);
        assert_eq!(report.entries[0].name, "a");
        assert_eq!(report.entries[1].name, "b");
    }

    #[test]
    fn empty_project_refused() {
        let p = parse_project(r#"{ "name": "e", "capsules": [] }"#).unwrap();
        assert!(matches!(build(&p), Err(ProjectError::Empty { .. })));
    }

    #[test]
    fn duplicate_capsule_name_refused() {
        let json = r#"{
            "name": "dup",
            "capsules": [
                { "name": "a", "source": "1" },
                { "name": "a", "source": "2" }
            ]
        }"#;
        let p = parse_project(json).unwrap();
        assert!(matches!(build(&p), Err(ProjectError::DuplicateName { .. })));
    }

    #[test]
    fn invalid_capsule_fails_whole_build() {
        let json = r#"{
            "name": "bad",
            "capsules": [
                { "name": "good", "source": "1 + 2" },
                { "name": "broken", "source": "this is not javascript" }
            ]
        }"#;
        let p = parse_project(json).unwrap();
        assert!(matches!(
            build(&p),
            Err(ProjectError::CapsuleElaboration { .. })
        ));
    }

    #[test]
    fn declaration_order_does_not_affect_project_cid() {
        let p1 = parse_project(
            r#"{ "name": "x", "capsules": [ { "name": "a", "source": "1" }, { "name": "b", "source": "2" } ] }"#,
        )
        .unwrap();
        let p2 = parse_project(
            r#"{ "name": "x", "capsules": [ { "name": "b", "source": "2" }, { "name": "a", "source": "1" } ] }"#,
        )
        .unwrap();
        assert_eq!(
            build(&p1).unwrap().project_cid,
            build(&p2).unwrap().project_cid
        );
    }
}
