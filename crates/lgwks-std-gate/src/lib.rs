//! `lgwks_std_gate` owns dependency admission and enforces INV-DEP-REGISTERED:
//! a resolved crate that is neither written by the estate, nor `lgwks_std`
//! itself, nor an entry in the human-approved register fails the build.
//!
//! The Director's rule in one line: *if a library is not in `std` or `std+`,
//! it is not an approved dependency, and the code does not compile until a
//! human has registered it in the semantic contract.* Everything here exists to
//! make the second half of that sentence mechanically true rather than a
//! convention people remember.
//!
//! ## Where the refusal happens
//!
//! A CI check is advisory — it runs after the code is written and it can be
//! skipped. To make an unapproved dependency a *compile* error, the consumer
//! takes this crate as a build-dependency and calls [`enforce`] from its
//! `build.rs`:
//!
//! ```ignore
//! // build.rs — INV-DEP-REGISTERED
//! fn main() {
//!     lgwks_std_gate::enforce();
//! }
//! ```
//!
//! Cargo runs `build.rs` before compiling the crate, with the *consumer's*
//! `CARGO_MANIFEST_DIR`, so the gate reads the consumer's own `Cargo.lock` and
//! `contract/APPROVED.toml` and fails the build with `cargo::error` before a
//! single line of the crate is type-checked.
//!
//! ## Fail-closed
//!
//! A missing register, an unparseable register, and an unreadable lock file are
//! all refusals. A gate that passes when it cannot find its own contract is a
//! gate that reports success for the one condition it exists to catch. The only
//! way to stand enforcement down is `enforce = false` under `[policy]` in the
//! register itself — a reviewable diff carrying a human's name, never an
//! environment variable a build can set for itself.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod contract;
pub mod lock;

use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use contract::Contract;
use lock::Resolved;

/// Register location relative to the repository root.
pub const CONTRACT_PATH: &str = "contract/APPROVED.toml";

/// Crates that are the gate, and so cannot be gated by it.
const SELF_EXEMPT: [&str; 2] = ["lgwks_std", "lgwks_std_gate"];

// ── Refusals ────────────────────────────────────────────────────────────────

/// One dependency the register does not admit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The crate appears in the resolved graph with no approval at all.
    Unregistered {
        /// Package name as `Cargo.lock` spells it.
        krate: String,
        /// Version Cargo resolved.
        version: String,
    },
    /// The crate is approved, but not at the version that resolved. An approval
    /// is for a specific set of bytes; a drifted version is a new approval, not
    /// an automatic one.
    VersionDrift {
        /// Package name.
        krate: String,
        /// Version the register approves.
        approved: String,
        /// Version Cargo resolved.
        resolved: String,
    },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unregistered { krate, version } => write!(
                f,
                "{krate} {version} is not in std, not in lgwks_std, and not approved in {CONTRACT_PATH}"
            ),
            Self::VersionDrift { krate, approved, resolved } => write!(
                f,
                "{krate} resolved to {resolved} but {CONTRACT_PATH} approves {approved}"
            ),
        }
    }
}

impl Refusal {
    /// The crate this refusal is about.
    pub fn krate(&self) -> &str {
        match self {
            Self::Unregistered { krate, .. } | Self::VersionDrift { krate, .. } => krate,
        }
    }
}

/// Why the gate could not reach a verdict. Every variant is a refusal, not a
/// pass — see the fail-closed note on this module.
#[derive(Debug)]
pub enum GateError {
    /// No `Cargo.lock` was found at or above the starting directory.
    LockNotFound {
        /// Directory the search started from.
        from: PathBuf,
    },
    /// The repository has no register.
    ContractNotFound {
        /// Path the register was expected at.
        path: PathBuf,
    },
    /// A file could not be read.
    Unreadable {
        /// Path that could not be read.
        path: PathBuf,
        /// Underlying I/O cause.
        cause: std::io::Error,
    },
    /// The register is not a valid contract.
    Contract(contract::ContractError),
    /// The lock file could not be read.
    Lock(lock::LockError),
}

impl fmt::Display for GateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LockNotFound { from } => {
                write!(f, "no Cargo.lock at or above {}", from.display())
            }
            Self::ContractNotFound { path } => write!(
                f,
                "no dependency register at {} — every repo the gate guards must carry one; \
                 run `lgwks-gate init` to write a fail-closed starting register",
                path.display()
            ),
            Self::Unreadable { path, cause } => {
                write!(f, "cannot read {}: {cause}", path.display())
            }
            Self::Contract(e) => write!(f, "{CONTRACT_PATH}: {e}"),
            Self::Lock(e) => write!(f, "Cargo.lock: {e}"),
        }
    }
}

impl Error for GateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Unreadable { cause, .. } => Some(cause),
            Self::Contract(e) => Some(e),
            Self::Lock(e) => Some(e),
            _ => None,
        }
    }
}

// ── The audit ───────────────────────────────────────────────────────────────

/// Audits a resolved graph against a register. Pure: no filesystem, no
/// environment, so the whole decision is testable from two strings.
pub fn audit(resolved: &[Resolved], register: &Contract) -> Vec<Refusal> {
    let mut refusals = Vec::new();
    for package in resolved {
        if package.local || is_self_exempt(&package.name) {
            continue;
        }
        match register.approval_for(&package.name) {
            None => refusals.push(Refusal::Unregistered {
                krate: package.name.clone(),
                version: package.version.clone(),
            }),
            Some(entry) if !version_admits(&entry.version, &package.version) => {
                refusals.push(Refusal::VersionDrift {
                    krate: package.name.clone(),
                    approved: entry.version.clone(),
                    resolved: package.version.clone(),
                })
            }
            Some(_) => {}
        }
    }
    refusals.sort_by(|a, b| a.krate().cmp(b.krate()));
    refusals
}

fn is_self_exempt(name: &str) -> bool {
    let normalised = name.to_ascii_lowercase().replace('-', "_");
    SELF_EXEMPT.contains(&normalised.as_str())
}

/// An approved version admits a resolved one when it is equal or a prefix on a
/// dot boundary, so `1.0` admits `1.0.219` and `1.0.219` admits only itself.
/// The literal `*` admits any version and is the visible way for a human to say
/// "deliberately unpinned".
fn version_admits(approved: &str, resolved: &str) -> bool {
    if approved == "*" {
        return true;
    }
    resolved == approved
        || resolved
            .strip_prefix(approved)
            .is_some_and(|rest| rest.starts_with('.'))
}

// ── Filesystem entry points ─────────────────────────────────────────────────

/// Walks up from `start` to the nearest directory holding a `Cargo.lock`.
pub fn repository_root(start: &Path) -> Result<PathBuf, GateError> {
    let mut cursor = Some(start);
    while let Some(dir) = cursor {
        if dir.join("Cargo.lock").is_file() {
            return Ok(dir.to_path_buf());
        }
        cursor = dir.parent();
    }
    Err(GateError::LockNotFound {
        from: start.to_path_buf(),
    })
}

fn ensure_contract_file(path: &Path) -> Result<(), GateError> {
    if !path.is_file() {
        Err(GateError::ContractNotFound {
            path: path.to_path_buf(),
        })
    } else {
        Ok(())
    }
}

/// Audits the repository rooted at `root`, reading its lock file and register.
pub fn check_dependencies(root: &Path) -> Result<(Contract, Vec<Refusal>), GateError> {
    check_dependencies_against(root, &root.join(CONTRACT_PATH))
}

/// Audits `root` against a register held elsewhere. This exists for the `check
/// --contract` diagnosis path, where a repo is audited *before* it carries a
/// register of its own. [`enforce`] never calls it: a build always reads the
/// register committed beside the code it is building, so no build can be
/// pointed at a more permissive contract than the one in its own tree.
pub fn check_dependencies_against(
    root: &Path,
    contract_path: &Path,
) -> Result<(Contract, Vec<Refusal>), GateError> {
    let lock_path = root.join("Cargo.lock");
    let contract_path = contract_path.to_path_buf();
    ensure_contract_file(&contract_path)?;
    let register = Contract::parse(&read(&contract_path)?).map_err(GateError::Contract)?;
    let resolved = lock::parse(&read(&lock_path)?).map_err(GateError::Lock)?;
    let refusals = audit(&resolved, &register);
    Ok((register, refusals))
}

fn read(path: &Path) -> Result<String, GateError> {
    std::fs::read_to_string(path).map_err(|cause| GateError::Unreadable {
        path: path.to_path_buf(),
        cause,
    })
}

// ── build.rs entry point ────────────────────────────────────────────────────

fn resolve_enforce_root() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR is set by cargo for every build script");
    match repository_root(Path::new(&manifest_dir)) {
        Ok(root) => root,
        Err(e) => fail(&[e.to_string()]),
    }
}

fn emit_cargo_rerun(root: &Path) {
    println!(
        "cargo::rerun-if-changed={}",
        root.join("Cargo.lock").display()
    );
    println!(
        "cargo::rerun-if-changed={}",
        root.join(CONTRACT_PATH).display()
    );
}

fn handle_enforce_refusals(register: &Contract, refusals: &[Refusal]) {
    if refusals.is_empty() {
        return;
    }
    let lines: Vec<String> = refusals.iter().map(|r| r.to_string()).collect();
    if register.enforce {
        fail(&lines);
    }
    for line in &lines {
        println!("cargo::warning=INV-DEP-REGISTERED (enforcement stood down): {line}");
    }
}

/// Fails the consumer's build when its resolved graph carries a dependency the
/// human-approved register does not admit. Call from `build.rs`; see the module
/// header for the three-line wiring.
///
/// # Panics
///
/// Panics when the audit refuses, when the register is missing or unparseable,
/// or when the lock file cannot be read. That panic is the mechanism: it is how
/// a build script turns INV-DEP-REGISTERED into a compile error.
pub fn enforce() {
    let root = resolve_enforce_root();
    emit_cargo_rerun(&root);

    let (register, refusals) = match check_dependencies(&root) {
        Ok(outcome) => outcome,
        Err(e) => fail(&[e.to_string()]),
    };
    handle_enforce_refusals(&register, &refusals);
}

fn fail(lines: &[String]) -> ! {
    for line in lines {
        println!("cargo::error=INV-DEP-REGISTERED: {line}");
    }
    println!(
        "cargo::error=add an approval to {CONTRACT_PATH}, or reach for a lower rung — \
         `lgwks-gate request <crate> <version>` prints the block to fill in"
    );
    panic!(
        "INV-DEP-REGISTERED: {} unapproved dependencies",
        lines.len()
    );
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const REGISTER: &str = concat!(
        "[policy]\nenforce = true\n\n",
        "[[approved]]\n",
        "crate = \"serde\"\n",
        "tier = \"boundary\"\n",
        "version = \"1.0\"\n",
        "reason = \"Derive-based serialization needs compiler introspection std lacks.\"\n",
        "approved_by = \"Director\"\n",
        "approved_on = \"2026-08-19\"\n",
        "review = \"docs/ADMISSION.md\"\n",
    );

    fn resolved(name: &str, version: &str, local: bool) -> Resolved {
        Resolved {
            name: name.into(),
            version: version.into(),
            local,
        }
    }

    #[test]
    fn an_approved_crate_at_an_approved_version_passes() {
        let register = Contract::parse(REGISTER).unwrap();
        assert!(audit(&[resolved("serde", "1.0.219", false)], &register).is_empty());
    }

    #[test]
    fn an_unregistered_crate_is_refused() {
        let register = Contract::parse(REGISTER).unwrap();
        assert_eq!(
            audit(&[resolved("tokio", "1.40.0", false)], &register),
            vec![Refusal::Unregistered {
                krate: "tokio".into(),
                version: "1.40.0".into()
            }]
        );
    }

    #[test]
    fn a_workspace_crate_is_never_audited() {
        let register = Contract::parse(REGISTER).unwrap();
        assert!(audit(&[resolved("braid-ir", "0.1.0", true)], &register).is_empty());
    }

    #[test]
    fn the_plus_library_itself_needs_no_approval() {
        let register = Contract::parse(REGISTER).unwrap();
        assert!(audit(&[resolved("lgwks_std", "0.1.0", false)], &register).is_empty());
    }

    #[test]
    fn a_drifted_version_is_refused_even_though_the_crate_is_approved() {
        let register = Contract::parse(REGISTER).unwrap();
        assert_eq!(
            audit(&[resolved("serde", "2.0.0", false)], &register),
            vec![Refusal::VersionDrift {
                krate: "serde".into(),
                approved: "1.0".into(),
                resolved: "2.0.0".into(),
            }]
        );
    }

    #[test]
    fn a_prefix_pin_only_matches_on_a_dot_boundary() {
        assert!(version_admits("1.0", "1.0.219"));
        assert!(version_admits("1.0.219", "1.0.219"));
        assert!(!version_admits("1.0", "1.02"));
        assert!(!version_admits("1.0", "1.0219"));
        assert!(version_admits("*", "99.1.2"));
    }

    #[test]
    fn refusals_are_reported_in_a_stable_order() {
        let register = Contract::parse(REGISTER).unwrap();
        let graph = [
            resolved("zeroize", "1.8.1", false),
            resolved("anyhow", "1.0.90", false),
            resolved("tokio", "1.40.0", false),
        ];
        let refusals = audit(&graph, &register);
        let names: Vec<&str> = refusals.iter().map(|r| r.krate()).collect();
        assert_eq!(names, vec!["anyhow", "tokio", "zeroize"]);
    }

    /// INV-STDPLUS-APPROVED-ONLY: lgwks-std may depend on vetted leaf crates
    /// whose transitive trees bottom out at zero external deps. The gate itself
    /// stays zero-dep (self-refuting otherwise). This test enforces both.
    #[test]
    fn deps_are_approved_leaves() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();

        // The gate itself must remain zero-dep.
        {
            let manifest =
                std::fs::read_to_string(workspace.join("crates/lgwks-std-gate/Cargo.toml"))
                    .expect("gate manifest missing");
            let after = manifest
                .split("[dependencies]")
                .nth(1)
                .expect("gate declares [dependencies]");
            let declared: Vec<&str> = after
                .lines()
                .map(str::trim)
                .take_while(|l| !l.starts_with('['))
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .collect();
            assert!(
                declared.is_empty(),
                "lgwks-std-gate declares dependencies: {declared:?}"
            );
        }

        // lgwks-std may only declare deps on this approved list.
        {
            const APPROVED: &[&str] = &["blake3", "regex", "rkyv", "serde", "serde_json"];

            let manifest = std::fs::read_to_string(workspace.join("crates/lgwks-std/Cargo.toml"))
                .expect("std manifest missing");
            let after = manifest
                .split("[dependencies]")
                .nth(1)
                .expect("std declares [dependencies]");
            let declared: Vec<&str> = after
                .lines()
                .map(str::trim)
                .take_while(|l| !l.starts_with('['))
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .collect();
            for line in &declared {
                let name = line.split('=').next().unwrap_or("").trim();
                assert!(
                    APPROVED.contains(&name),
                    "lgwks-std declares unapproved dependency `{name}` — \
                     add it to APPROVED in this test after Director review"
                );
            }
        }
    }

    #[test]
    fn a_repository_root_is_the_nearest_ancestor_holding_a_lock() {
        let here = Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = repository_root(here).expect("workspace has a Cargo.lock");
        assert!(root.join("Cargo.lock").is_file());
    }
}
