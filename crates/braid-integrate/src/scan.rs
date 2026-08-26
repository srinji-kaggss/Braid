//! Repo scan — file inventory, language signals, import lines, manifest deps.
//!
//! The "graph" in `braid-integrate` is this module's output: a file
//! inventory plus per-file import lines. No per-language parser, no
//! tree-sitter — v0 deliberately stays at import-line regex and manifest
//! scans so the detector set proves the advisor loop before precision
//! work.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Language/mode of the target repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Rust workspace (Cargo.toml present).
    Rust,
    /// Anything else — polyglot heuristics.
    Polyglot,
}

/// One file's scan slice.
#[derive(Debug, Clone)]
pub struct ScannedFile {
    /// Path relative to repo root, with `/` separators.
    pub rel: String,
    /// Lowercased extension without dot (e.g. `rs`, `ts`, `py`).
    pub ext: String,
    /// Import-like lines (`import`, `require`, `use`, `from`, `include`).
    pub imports: Vec<String>,
    /// Whether the file looks like it contains a scheduler/cron seam.
    pub sched_hit: bool,
    /// Whether the file looks like an HTTP/client seam.
    pub http_hit: bool,
}

/// Whole-repo scan summary — the graph the AI reads.
#[derive(Debug, Clone)]
pub struct Scan {
    /// Absolute repo root that was scanned.
    pub root: String,
    /// Per-file slices.
    pub files: Vec<ScannedFile>,
    /// `ext -> count` (e.g. `rs -> 40`).
    pub by_ext: BTreeMap<String, usize>,
    /// Distinct language signals (extensions + manifest presence).
    pub languages: Vec<String>,
    /// Raw import strings across all files (for mode detection).
    pub all_imports: Vec<String>,
    /// Manifest signals present in the repo.
    pub manifests: Vec<String>,
    /// Cargo.toml dep lines (raw) when present.
    pub cargo_dep_lines: Vec<String>,
    /// package.json dep names when present.
    pub npm_deps: Vec<String>,
    /// pyproject / requirements dep names when present.
    pub py_deps: Vec<String>,
    /// go.mod module + requires when present.
    pub go_deps: Vec<String>,
}

/// Scan `root` and return the inventory + import graph.
pub fn scan_repo(root: &Path) -> Result<Scan, String> {
    let root_str = root.display().to_string();
    let mut files: Vec<ScannedFile> = Vec::new();
    walk(root, root, &mut files)?;
    let mut by_ext: BTreeMap<String, usize> = BTreeMap::new();
    let mut all_imports: Vec<String> = Vec::new();
    for f in &files {
        *by_ext.entry(f.ext.clone()).or_default() += 1;
        all_imports.extend(f.imports.clone());
    }
    let mut manifests = Vec::new();
    let mut cargo_dep_lines = Vec::new();
    let mut npm_deps = Vec::new();
    let mut py_deps = Vec::new();
    let mut go_deps = Vec::new();
    if let Ok(s) = std::fs::read_to_string(root.join("Cargo.toml")) {
        manifests.push("Cargo.toml".to_string());
        cargo_dep_lines = scan_cargo_dep_lines(&s);
    }
    // Workspace members: scan one level of crate Cargo.toml files so top-level
    // [workspace.dependencies] does not misreport the real feature posture.
    // Skip the toolkit's own crates — their optional deps are the implementation,
    // not consumer opportunities (Braid self-scan would otherwise flag its own blake3).
    if cargo_dep_lines.is_empty() {
        for dir in ["crates", "packages", "apps", "services", "libs"] {
            let base = root.join(dir);
            if !base.is_dir() {
                continue;
            }
            if let Ok(entries) = std::fs::read_dir(&base) {
                for ent in entries.flatten() {
                    let name = ent.file_name().to_string_lossy().to_string();
                    if name.starts_with("lgwks-") || name.starts_with("lgwks_") {
                        continue;
                    }
                    let cargo = ent.path().join("Cargo.toml");
                    if let Ok(s) = std::fs::read_to_string(&cargo) {
                        cargo_dep_lines.extend(scan_cargo_dep_lines(&s));
                    }
                }
            }
        }
    }
    if let Ok(s) = std::fs::read_to_string(root.join("package.json")) {
        manifests.push("package.json".to_string());
        npm_deps = scan_npm_deps(&s);
    }
    if root.join("pyproject.toml").exists() || root.join("requirements.txt").exists() {
        if let Ok(s) = std::fs::read_to_string(root.join("pyproject.toml")) {
            manifests.push("pyproject.toml".to_string());
            py_deps.extend(scan_pyproject_deps(&s));
        }
        if let Ok(s) = std::fs::read_to_string(root.join("requirements.txt")) {
            if !manifests.contains(&"requirements.txt".to_string()) {
                manifests.push("requirements.txt".to_string());
            }
            py_deps.extend(scan_requirements_deps(&s));
        }
    }
    if let Ok(s) = std::fs::read_to_string(root.join("go.mod")) {
        manifests.push("go.mod".to_string());
        go_deps = scan_go_deps(&s);
    }
    // Language set from extensions + manifest signals.
    let mut lang_set: BTreeSet<String> = BTreeSet::new();
    for ext in by_ext.keys() {
        lang_set.insert(ext.clone());
    }
    for m in &manifests {
        match m.as_str() {
            "Cargo.toml" => {
                lang_set.insert("rs".to_string());
            }
            "package.json" => {
                lang_set.insert("js".to_string());
            }
            "pyproject.toml" | "requirements.txt" => {
                lang_set.insert("py".to_string());
            }
            "go.mod" => {
                lang_set.insert("go".to_string());
            }
            _ => {}
        }
    }
    let languages = lang_set.into_iter().collect();
    Ok(Scan {
        root: root_str,
        files,
        by_ext,
        languages,
        all_imports,
        manifests,
        cargo_dep_lines,
        npm_deps,
        py_deps,
        go_deps,
    })
}

/// Autodetect advisor mode from the scan + on-disk signals.
pub fn detect_mode(root: &Path, scan: &Scan) -> Mode {
    if root.join("Cargo.toml").exists() {
        return Mode::Rust;
    }
    if root.join("contract/APPROVED.toml").exists() {
        return Mode::Rust;
    }
    let has_braid = scan
        .all_imports
        .iter()
        .any(|i| i.contains("braid-") || i.contains("lgwks"));
    if has_braid {
        return Mode::Rust;
    }
    Mode::Polyglot
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<ScannedFile>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("read_dir {}: {e}", dir.display()))?;
    for ent in entries {
        let ent = ent.map_err(|e| format!("entry: {e}"))?;
        let path = ent.path();
        let name = ent.file_name().to_string_lossy().to_string();
        if name.starts_with('.')
            || name == "target"
            || name == "node_modules"
            || name == "__pycache__"
        {
            continue;
        }
        let ft = ent.file_type().map_err(|e| format!("file_type: {e}"))?;
        if ft.is_dir() {
            walk(root, &path, out)?;
            continue;
        }
        if !ft.is_file() {
            continue;
        }
        // Only scan text-ish files.
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let interesting = matches!(
            ext.as_str(),
            "rs" | "ts"
                | "js"
                | "tsx"
                | "jsx"
                | "py"
                | "go"
                | "toml"
                | "json"
                | "yaml"
                | "yml"
                | "env"
                | "md"
                | "sh"
        );
        if !interesting {
            continue;
        }
        // Skip very large files (generated).
        let meta =
            std::fs::metadata(&path).map_err(|e| format!("metadata {}: {e}", path.display()))?;
        if meta.len() > 512 * 1024 {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .display()
            .to_string()
            .replace('\\', "/");
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let mut imports: Vec<String> = Vec::new();
        let mut sched_hit = false;
        let mut http_hit = false;
        for line in text.lines() {
            let t = line.trim();
            if is_import_line(t) {
                imports.push(t.to_string());
            }
            if !sched_hit && is_sched_line(t) {
                sched_hit = true;
            }
            if !http_hit && is_http_line(t) {
                http_hit = true;
            }
        }
        // Keep files that carry at least one signal or are manifest-adjacent.
        let keep = !imports.is_empty()
            || sched_hit
            || http_hit
            || matches!(ext.as_str(), "rs" | "ts" | "js" | "tsx" | "py" | "go");
        if !keep {
            continue;
        }
        out.push(ScannedFile {
            rel,
            ext,
            imports,
            sched_hit,
            http_hit,
        });
    }
    Ok(())
}

fn is_import_line(t: &str) -> bool {
    t.starts_with("import ")
        || t.starts_with("from ")
        || t.starts_with("require(")
        || t.starts_with("use ")
        || t.starts_with("mod ")
        || t.starts_with("pub use ")
        || t.starts_with("include")
        || t.starts_with("export ")
        || (t.contains("from \"") && t.contains("import"))
        || (t.contains("from '") && t.contains("import"))
}

fn is_sched_line(t: &str) -> bool {
    let l = t.to_ascii_lowercase();
    l.contains("cron")
        || l.contains("setinterval")
        || l.contains("set_interval")
        || l.contains("apscheduler")
        || l.contains("celery") && l.contains("beat")
        || l.contains("tokio::spawn")
        || l.contains("spawn_blocking")
}

fn is_http_line(t: &str) -> bool {
    let l = t.to_ascii_lowercase();
    l.contains("axios")
        || l.contains("fetch(")
        || l.contains("reqwest")
        || l.contains("hyper")
        || l.contains("requests.")
        || l.contains("net/http")
        || l.contains("http::")
}

fn scan_cargo_dep_lines(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_deps = false;
    for line in s.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            // Workspace members live under [workspace.dependencies] — not real deps.
            if t.starts_with("[workspace") {
                in_deps = false;
                continue;
            }
            in_deps = t == "[dependencies]" || t.starts_with("[dependencies.");
        }
        if in_deps && !t.is_empty() && !t.starts_with('[') && !t.starts_with('#') && t.contains('=')
        {
            out.push(t.to_string());
        }
    }
    out
}

fn scan_npm_deps(s: &str) -> Vec<String> {
    // Best-effort: extract keys under "dependencies" / "devDependencies".
    let mut out = Vec::new();
    let mut in_deps = false;
    let mut depth: i32 = 0;
    for line in s.lines() {
        let t = line.trim();
        if t.contains("\"dependencies\"") {
            in_deps = true;
            depth = 0;
        }
        if in_deps {
            depth += t.matches('{').count() as i32;
            depth -= t.matches('}').count() as i32;
            // Keys look like "express": "4.x"
            if t.starts_with('"')
                && t.contains(':')
                && let Some(k) = t.split('"').nth(1)
                && !k.is_empty()
                && k != "dependencies"
                && k != "devDependencies"
            {
                out.push(k.to_string());
            }
            if depth <= 0 && in_deps && t.contains('}') {
                // Heuristic end; keep scanning for devDependencies.
                // Don't break — there may be a second block.
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn scan_pyproject_deps(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in s.lines() {
        let t = line.trim();
        if t.starts_with("dependencies") || t.starts_with('"') && t.contains("==") {
            // Very loose — enough for the advisor's "what exists" signal.
            let name = t
                .split(['"', '\'', '=', '>', '<', '[', ' '])
                .next()
                .unwrap_or("")
                .trim();
            if !name.is_empty() {
                out.push(name.to_string());
            }
        }
    }
    // Also scan PEP 621 [project] dependencies = ["requests==2.x", ...]
    for cap in s.split('"').collect::<Vec<_>>().chunks(2) {
        if cap.len() == 2 && cap[1].contains("==") {
            let name = cap[1].split("==").next().unwrap_or("").trim();
            if !name.is_empty() && !name.contains(' ') {
                out.push(name.to_string());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn scan_requirements_deps(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in s.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let name = t
            .split(['=', '>', '<', '[', ' ', ';'])
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if !name.is_empty() {
            out.push(name);
        }
    }
    out.sort();
    out.dedup();
    out
}

fn scan_go_deps(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in s.lines() {
        let t = line.trim();
        if t.starts_with("module ")
            || t.starts_with("require ")
            || t.starts_with("require(")
            || t.starts_with("github.com/")
            || t.starts_with("golang.org/")
        {
            out.push(t.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_line_detection() {
        assert!(is_import_line("import express from \"express\""));
        assert!(is_import_line("from .foo import bar"));
        assert!(is_import_line("use serde::Deserialize;"));
        assert!(is_import_line("require(\"axios\")"));
        assert!(!is_import_line("let x = 1;"));
    }

    #[test]
    fn sched_line_detection() {
        assert!(is_sched_line("cron.schedule(\"* * * * *\", fn)"));
        assert!(is_sched_line("setInterval(fn, 1000)"));
        assert!(is_sched_line("tokio::spawn(async { })"));
        assert!(!is_sched_line("let x = 1;"));
    }

    #[test]
    fn http_line_detection() {
        assert!(is_http_line("import axios from \"axios\""));
        assert!(is_http_line("fetch(\"https://example.com\")"));
        assert!(is_http_line("use reqwest;"));
        assert!(!is_http_line("let x = 1;"));
    }
}
