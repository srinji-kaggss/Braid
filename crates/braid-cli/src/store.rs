//! The braid store (W5): name-addressed repo-manifest artifacts plus the
//! declared inventory.
//!
//! - `put` validates then installs — validate-before-write, nothing touches
//!   disk on a contract violation.
//! - `catalog`/`summary` strict-decode + re-validate every artifact before
//!   emitting a single row: read invariant == write invariant, and no
//!   partial output (one bad artifact fails the whole read, named).
//! - Completeness is provable only against the declared inventory; without
//!   one, catalog fails closed ("full summary of everything that is ours"
//!   cannot be claimed from a store that does not declare the set).
//!
//! No second verifier is built anywhere: repo manifests are inventory
//! metadata, and braid-verify remains the sole admission authority for
//! capsules (D9). This module adds zero authority.
//!
//! Exit discipline (documented in docs/authoring-cli.md): missing/unreadable
//! paths are operator errors (exit 2, matching the family precedent);
//! contract violations, unknown repos, duplicates, tampering, and inventory
//! mismatches are fail-closed denials (exit 1); misuse is usage (exit 2).

use std::path::{Path, PathBuf};

use braid_ir::Cid;
use braid_manifest::{self, RepoManifest};

/// Default store root, relative to `$HOME`. ONE constant — the single point
/// of correction if the org store lives elsewhere.
pub const DEFAULT_STORE_DIR: &str = ".local/share/braid/store";
pub const INVENTORY_FILE: &str = "inventory.json";
pub const MANIFEST_SUFFIX: &str = ".manifest";

/// Sentinel prefix: the command ran correctly and the answer is "denied"
/// (fail-closed). `main` maps it to exit 1, distinct from operator error (2).
pub const FAIL_CLOSED: &str = "\0fail-closed";

/// Wrap a fail-closed denial so `main` can map it to exit 1.
pub fn denied(msg: String) -> String {
    format!("{FAIL_CLOSED}{msg}")
}

/// Resolve the store root: `--store` wins; otherwise `$HOME` + default.
pub fn store_root(flag: Option<&str>) -> Result<PathBuf, String> {
    match flag {
        Some(p) => Ok(PathBuf::from(p)),
        None => {
            let home = std::env::var_os("HOME").ok_or("HOME is not set; pass --store")?;
            Ok(PathBuf::from(home).join(DEFAULT_STORE_DIR))
        }
    }
}

type CliResult = Result<(), String>;

// ───────────────────────────────── store ─────────────────────────────────

/// `braid store put <repo-manifest.json> [--store <dir>] [--replace]`
pub fn cmd_store(args: &[String]) -> CliResult {
    let Some((sub, rest)) = args.split_first() else {
        return Err(
            "usage: braid store put <repo-manifest.json> [--store <dir>] [--replace]".into(),
        );
    };
    match sub.as_str() {
        "put" => cmd_store_put(rest),
        other => Err(format!(
            "unknown store subcommand `{other}` (only `put` exists)"
        )),
    }
}

fn cmd_store_put(args: &[String]) -> CliResult {
    let mut store_flag: Option<&str> = None;
    let mut replace = false;
    let mut path: Option<&str> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--store" => {
                store_flag = Some(args.get(i + 1).ok_or("`--store` needs a path")?);
                i += 2;
            }
            "--replace" => {
                replace = true;
                i += 1;
            }
            p if !p.starts_with('-') && path.is_none() => {
                path = Some(p);
                i += 1;
            }
            other => return Err(format!("unexpected arg `{other}` (store put)")),
        }
    }
    let path = path.ok_or("store put needs <repo-manifest.json>")?;
    // Missing/unreadable input is an operator error (exit 2).
    let json = std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
    // Validate BEFORE any filesystem mutation — a contract violation leaves
    // the store byte-identical.
    let manifest = braid_manifest::validate(&json).map_err(|e| denied(format!("{path}: {e}")))?;
    let root = store_root(store_flag)?;
    // First run creates the store root (documented; tested).
    std::fs::create_dir_all(&root).map_err(|e| format!("create store {}: {e}", root.display()))?;
    install(&root, &manifest, replace)?;
    println!(
        "stored {:<32} cid {}",
        manifest.name,
        manifest.cid().to_hex()
    );
    Ok(())
}

fn install(root: &Path, m: &RepoManifest, replace: bool) -> CliResult {
    // Declared-inventory gate: once an inventory exists, only declared repos
    // may enter the store.
    let inv_path = root.join(INVENTORY_FILE);
    if inv_path.exists() {
        let inv = std::fs::read_to_string(&inv_path)
            .map_err(|e| format!("read {}: {e}", inv_path.display()))?;
        let declared = braid_manifest::parse_inventory(&inv)
            .map_err(|e| denied(format!("{}: {e}", inv_path.display())))?;
        if !declared.iter().any(|d| d.name == m.name) {
            return Err(denied(format!(
                "repo `{}` is not in the declared inventory {}",
                m.name,
                inv_path.display()
            )));
        }
    }
    // Duplicate + case-insensitive collision detection (APFS folds case:
    // `Braid` vs `braid` is one file). Never silent last-write-wins.
    for entry in
        std::fs::read_dir(root).map_err(|e| format!("read store {}: {e}", root.display()))?
    {
        let entry = entry.map_err(|e| format!("read store entry: {e}"))?;
        let fname = entry.file_name();
        let fname = fname.to_str().unwrap_or_default();
        let Some(stem) = fname.strip_suffix(MANIFEST_SUFFIX) else {
            continue;
        };
        if !stem.eq_ignore_ascii_case(&m.name) {
            continue;
        }
        let existing_bytes = std::fs::read(entry.path())
            .map_err(|e| format!("read {}: {e}", entry.path().display()))?;
        let existing = RepoManifest::from_bytes(&existing_bytes).map_err(|e| {
            denied(format!(
                "store corruption at {}: {e}",
                entry.path().display()
            ))
        })?;
        if stem != m.name {
            return Err(denied(format!(
                "case-insensitive name collision with `{stem}` (cid {}); rename the repo",
                existing.cid().to_hex()
            )));
        }
        if !replace {
            return Err(denied(format!(
                "`{}` is already stored (cid {}) — pass --replace to overwrite",
                m.name,
                existing.cid().to_hex()
            )));
        }
    }
    let dest = root.join(format!("{}{}", m.name, MANIFEST_SUFFIX));
    std::fs::write(&dest, m.to_bytes()).map_err(|e| format!("write {}: {e}", dest.display()))?;
    // If an inventory pin exists for this repo and moved, the catalog now
    // denies until the new cid is recorded — an org-metadata change is a
    // recorded event in the org database, never silent drift.
    let inv_path = root.join(INVENTORY_FILE);
    if inv_path.exists()
        && let Ok(inv) = std::fs::read_to_string(&inv_path)
        && let Ok(declared) = braid_manifest::parse_inventory(&inv)
        && let Some(entry) = declared.iter().find(|d| d.name == m.name)
        && entry.cid != Some(m.cid())
    {
        eprintln!(
            "note: the inventory pin for `{}` is now stale — record its new \
             cid there; `braid catalog` denies until re-pinned",
            m.name
        );
    }
    Ok(())
}

// ───────────────────────────────── catalog ─────────────────────────────────

/// `braid catalog [--store <dir>]`
pub fn cmd_catalog(args: &[String]) -> CliResult {
    let flag = store_flag_only(args, "catalog")?;
    let root = store_root(flag)?;
    require_store_exists(&root)?;
    let entries = read_store(&root)?;
    print!("{}", render_catalog(&entries));
    Ok(())
}

/// `braid summary <repo> [--store <dir>]`
pub fn cmd_summary(args: &[String]) -> CliResult {
    let mut flag: Option<&str> = None;
    let mut repo: Option<&str> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--store" => {
                flag = Some(args.get(i + 1).ok_or("`--store` needs a path")?);
                i += 2;
            }
            p if !p.starts_with('-') && repo.is_none() => {
                repo = Some(p);
                i += 1;
            }
            other => return Err(format!("unexpected arg `{other}` (summary)")),
        }
    }
    let Some(repo) = repo else {
        return Err("usage: braid summary <repo> [--store <dir>]".into());
    };
    let root = store_root(flag)?;
    require_store_exists(&root)?;
    let entries = read_store(&root)?;
    let Some((m, cid)) = entries.iter().find(|(m, _)| m.name == repo) else {
        return Err(denied(format!(
            "unknown repo `{repo}` — run `braid catalog` for the declared set"
        )));
    };
    // Same emitter as catalog: the machine line is byte-identical across
    // both commands (one parser for consumers).
    print!("{}\n{}", human_block(m, cid), tsv_line(m, cid));
    Ok(())
}

fn store_flag_only<'a>(args: &'a [String], cmd: &'a str) -> Result<Option<&'a str>, String> {
    let mut flag: Option<&str> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--store" => {
                flag = Some(args.get(i + 1).ok_or("`--store` needs a path")?);
                i += 2;
            }
            other => return Err(format!("unexpected arg `{other}` ({cmd})")),
        }
    }
    Ok(flag)
}

/// Missing store dir is an operator error (exit 2, family precedent); the
/// message names the resolved path.
fn require_store_exists(root: &Path) -> CliResult {
    if !root.exists() {
        return Err(format!("store {} does not exist", root.display()));
    }
    Ok(())
}

/// Read + validate the whole store, fail-closed on the first bad artifact.
/// Tamper-evidence: every declared entry pins a CID, and every artifact's
/// content address is recomputed on read — a pinned/stored mismatch is a
/// denial naming both. Completeness: the store's manifest set must EXACTLY
/// equal the declared inventory — a missing repo, an undeclared one, or an
/// unpinned declaration is a denial naming the sides, never a silent partial
/// map.
fn read_store(root: &Path) -> Result<Vec<(RepoManifest, Cid)>, String> {
    let inv_path = root.join(INVENTORY_FILE);
    let declared = match std::fs::read_to_string(&inv_path) {
        Ok(s) => braid_manifest::parse_inventory(&s)
            .map_err(|e| denied(format!("{}: {e}", inv_path.display())))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(denied(format!(
                "no inventory declared at {} — completeness cannot be verified",
                inv_path.display()
            )));
        }
        Err(e) => return Err(format!("read {}: {e}", inv_path.display())),
    };

    let mut found: Vec<(RepoManifest, Cid)> = Vec::new();
    for entry in
        std::fs::read_dir(root).map_err(|e| format!("read store {}: {e}", root.display()))?
    {
        let entry = entry.map_err(|e| format!("read store entry: {e}"))?;
        let fname = entry.file_name();
        let fname = fname
            .to_str()
            .ok_or_else(|| denied("store entry name is not valid UTF-8".into()))?;
        if fname == INVENTORY_FILE {
            continue;
        }
        let Some(stem) = fname.strip_suffix(MANIFEST_SUFFIX) else {
            return Err(denied(format!(
                "undeclared store file `{fname}` — the store holds only manifests and {INVENTORY_FILE}"
            )));
        };
        if !braid_manifest::safe_name_component(stem) {
            return Err(denied(format!("unsafe store entry `{fname}`")));
        }
        let bytes = std::fs::read(entry.path())
            .map_err(|e| format!("read {}: {e}", entry.path().display()))?;
        let m = RepoManifest::from_bytes(&bytes)
            .map_err(|e| denied(format!("{fname}: fails validation: {e}")))?;
        // The storage key must equal the stored name — a renamed file is a
        // tampered artifact.
        if m.name != stem {
            return Err(denied(format!(
                "{fname}: stored name `{}` does not match its key",
                m.name
            )));
        }
        let cid = m.cid();
        found.push((m, cid));
    }

    // Every store artifact must be declared.
    let extra: Vec<String> = found
        .iter()
        .filter(|(m, _)| !declared.iter().any(|d| d.name == m.name))
        .map(|(m, _)| m.name.clone())
        .collect();
    if !extra.is_empty() {
        return Err(denied(format!(
            "store does not match the declared inventory — in store but undeclared: {}",
            extra.join(", ")
        )));
    }
    // Every declaration must be present, pinned, and content-matching.
    let mut missing: Vec<String> = Vec::new();
    for d in &declared {
        match found.iter().find(|(m, _)| m.name == d.name) {
            None => missing.push(format!("{} (not admitted)", d.name)),
            Some((_, actual)) => match d.cid {
                None => {
                    return Err(denied(format!(
                        "`{}` is declared but not pinned — record its cid in {} \
                         (`braid store put` prints it); the catalog denies until re-pinned",
                        d.name,
                        inv_path.display()
                    )));
                }
                Some(pinned) if pinned != *actual => {
                    return Err(denied(format!(
                        "{}.manifest: tampered or stale — pinned {} but stored {}",
                        d.name,
                        pinned.to_hex(),
                        actual.to_hex()
                    )));
                }
                Some(_) => {}
            },
        }
    }
    if !missing.is_empty() {
        return Err(denied(format!(
            "store does not match the declared inventory — missing from store: {}",
            missing.join(", ")
        )));
    }
    found.sort_by(|a, b| a.0.name.cmp(&b.0.name));
    Ok(found)
}

// ───────────────────────────────── render ─────────────────────────────────

/// One repo, all 8 fields + CID, fixed order. Every field is required at
/// admission, so nothing here can render as UNKNOWN.
fn human_block(m: &RepoManifest, cid: &Cid) -> String {
    format!(
        "name:         {}\narchetype:    {}\nowner:        {}\ngate_version: {}\nci_status:    {}\nentry_docs:   {}\ncommands:     {}\nlocal_ci:     {}\ncid:          {}",
        m.name,
        m.archetype.as_str(),
        m.owner,
        m.gate_version,
        m.ci_status.as_str(),
        m.entry_docs.join(", "),
        m.canonical_commands.join(", "),
        if m.local_ci { "yes" } else { "no" },
        cid.to_hex(),
    )
}

/// The stable machine line — 9 TSV fields; comma-joined lists are lossless
/// because commas are banned in authored strings. `cid8` = first 8 hex chars
/// of the manifest's BLAKE3 CID.
fn tsv_line(m: &RepoManifest, cid: &Cid) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        m.name,
        m.archetype.as_str(),
        m.owner,
        m.gate_version,
        m.ci_status.as_str(),
        m.entry_docs.join(","),
        m.canonical_commands.join(","),
        if m.local_ci { "yes" } else { "no" },
        &cid.to_hex()[..8],
    )
}

/// Human blocks (sorted by name), a `---` marker, then one machine line per
/// repo. Deterministic: no timestamps, env, or cwd; golden-vector pinned.
fn render_catalog(entries: &[(RepoManifest, Cid)]) -> String {
    let mut out = String::new();
    for (i, (m, cid)) in entries.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&human_block(m, cid));
    }
    out.push_str("\n---\n");
    for (m, cid) in entries {
        out.push_str(&tsv_line(m, cid));
        out.push('\n');
    }
    out
}
