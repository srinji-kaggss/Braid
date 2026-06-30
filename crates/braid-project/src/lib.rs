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

use braid_elaborate_js::{elaborate_and_admit, ElabError};
use braid_ir::{Capsule, Cid};
use braid_verify::Verdict;
use serde::Deserialize;

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
    Empty,
    /// Two capsules share a name (no shadowing).
    DuplicateName(String),
    /// The manifest JSON did not parse.
    Parse(String),
    /// A capsule's source failed to elaborate (carries the frontend error).
    CapsuleElaboration { name: String, error: ElabError },
    /// A capsule elaborated but the verifier rejected it.
    CapsuleRejected { name: String, reason: String },
}

impl core::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ProjectError::Empty => f.write_str("project has no capsules"),
            ProjectError::DuplicateName(n) => write!(f, "duplicate capsule name `{n}`"),
            ProjectError::Parse(m) => write!(f, "manifest parse error: {m}"),
            ProjectError::CapsuleElaboration { name, error } => {
                write!(f, "capsule `{name}`: {error}")
            }
            ProjectError::CapsuleRejected { name, reason } => {
                write!(f, "capsule `{name}` rejected by the verifier: {reason}")
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

/// Parse a project manifest from JSON.
pub fn parse_project(json: &str) -> Result<Project, ProjectError> {
    serde_json::from_str(json).map_err(|e| ProjectError::Parse(e.to_string()))
}

/// Build a project: elaborate + admit every capsule, fail-closed on the first
/// failure, then compute the project CID. Each capsule is verified
/// independently under the empty ambient set — the project pools no authority.
pub fn build(project: &Project) -> Result<BuildReport, ProjectError> {
    if project.capsules.is_empty() {
        return Err(ProjectError::Empty);
    }

    // No shadowing: a duplicate name could hide a second capsule behind one the
    // reviewer already approved.
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for c in &project.capsules {
        if !seen.insert(c.name.as_str()) {
            return Err(ProjectError::DuplicateName(c.name.clone()));
        }
    }

    let mut entries = Vec::with_capacity(project.capsules.len());
    for c in &project.capsules {
        let e =
            elaborate_and_admit(&c.source).map_err(|error| ProjectError::CapsuleElaboration {
                name: c.name.clone(),
                error,
            })?;
        match &e.verdict {
            Verdict::Admit { .. } => {}
            Verdict::Reject { stage, reason } => {
                return Err(ProjectError::CapsuleRejected {
                    name: c.name.clone(),
                    reason: format!("{stage:?}: {reason}"),
                });
            }
        }
        let cid = e.capsule.cid();
        entries.push(CapsuleEntry {
            name: c.name.clone(),
            capsule: e.capsule,
            cid,
        });
    }

    let project_cid = compute_project_cid(&entries);
    Ok(BuildReport {
        name: project.name.clone(),
        project_cid,
        entries,
    })
}

/// Convenience: parse + build in one call.
pub fn build_from_json(json: &str) -> Result<BuildReport, ProjectError> {
    build(&parse_project(json)?)
}

/// The project CID: order-independent over the `(name, capsule_cid)` set. Names
/// are unique (guarded), each row is unambiguously framed (NUL between name and
/// CID), the set is sorted, then hashed under the project domain. Reordering
/// capsules in the manifest does not change it; changing any capsule's source
/// (hence its CID) or its name does.
fn compute_project_cid(entries: &[CapsuleEntry]) -> Cid {
    let mut rows: Vec<String> = entries
        .iter()
        .map(|e| format!("{}\u{0}{}", e.name, e.cid.to_hex()))
        .collect();
    rows.sort();
    Cid::compute(PROJECT_DOMAIN, rows.join("\n").as_bytes())
}
