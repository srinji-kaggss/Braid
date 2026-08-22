//! `fs` owns recursive filesystem walking and directory traversal, enforcing
//! INV-FS-SAFE-WALK: directory walking respects depth bounds, handles symlink
//! loops defensively, stays within root sandbox bounds, and requires zero
//! external dependencies like `walkdir`.

use std::fs::{self, DirEntry};
use std::io;
use std::path::{Path, PathBuf};

/// Options for configuring a recursive filesystem walk.
#[derive(Debug, Clone)]
pub struct WalkOptions {
    /// Maximum directory depth to traverse (0 = only the root directory entries).
    pub max_depth: usize,
    /// Whether to follow symlinks to directories.
    pub follow_symlinks: bool,
    /// Whether to sort entries by filename for deterministic ordering.
    pub sort_alphabetically: bool,
}

impl Default for WalkOptions {
    fn default() -> Self {
        Self {
            max_depth: 32,
            follow_symlinks: false,
            sort_alphabetically: true,
        }
    }
}

/// Recursively walks `root` according to `options`, returning all matching entries.
pub fn walk_dir(root: impl AsRef<Path>, options: &WalkOptions) -> io::Result<Vec<PathBuf>> {
    let root_path = root.as_ref();
    let canonical_root = root_path.canonicalize().ok();
    let mut out = Vec::new();
    let mut visited = Vec::new();
    walk_recursive(
        root_path,
        &canonical_root,
        0,
        options,
        &mut out,
        &mut visited,
    )?;
    Ok(out)
}

fn track_canonical_visit(dir: &Path, visited_canonical: &mut Vec<PathBuf>) -> bool {
    if let Ok(canonical) = dir.canonicalize() {
        if visited_canonical.contains(&canonical) {
            return false;
        }
        visited_canonical.push(canonical);
    }
    true
}

fn read_sorted_entries(dir: &Path, sort_alphabetically: bool) -> io::Result<Vec<DirEntry>> {
    let mut entries: Vec<DirEntry> = fs::read_dir(dir)?.filter_map(Result::ok).collect();
    if sort_alphabetically {
        entries.sort_by_key(|entry| entry.file_name());
    }
    Ok(entries)
}

fn resolve_symlink_path(dir: &Path, path: &Path) -> Option<PathBuf> {
    let target = fs::read_link(path).ok()?;
    if target.is_relative() {
        Some(dir.join(target))
    } else {
        Some(target)
    }
}

fn check_sandbox(resolved: &Path, canonical_root: &Option<PathBuf>) -> Option<PathBuf> {
    let canon = resolved.canonicalize().ok()?;
    if let Some(canon_root) = canonical_root {
        if !canon.starts_with(canon_root) {
            return None;
        }
    }
    if canon.is_dir() {
        Some(canon)
    } else {
        None
    }
}

fn resolve_symlink_target(
    dir: &Path,
    path: &Path,
    canonical_root: &Option<PathBuf>,
) -> Option<PathBuf> {
    let resolved = resolve_symlink_path(dir, path)?;
    check_sandbox(&resolved, canonical_root)
}

fn handle_directory_entry(
    path: &Path,
    canonical_root: &Option<PathBuf>,
    depth: usize,
    options: &WalkOptions,
    out: &mut Vec<PathBuf>,
    visited: &mut Vec<PathBuf>,
) -> io::Result<()> {
    walk_recursive(path, canonical_root, depth, options, out, visited)
}

fn handle_symlink_entry(
    dir: &Path,
    path: &Path,
    canonical_root: &Option<PathBuf>,
    depth: usize,
    options: &WalkOptions,
    out: &mut Vec<PathBuf>,
    visited: &mut Vec<PathBuf>,
) -> io::Result<()> {
    if options.follow_symlinks {
        if let Some(target_dir) = resolve_symlink_target(dir, path, canonical_root) {
            walk_recursive(&target_dir, canonical_root, depth, options, out, visited)?;
        }
    }
    Ok(())
}

fn process_entry(
    entry: DirEntry,
    dir: &Path,
    canonical_root: &Option<PathBuf>,
    current_depth: usize,
    options: &WalkOptions,
    out: &mut Vec<PathBuf>,
    visited_canonical: &mut Vec<PathBuf>,
) -> io::Result<()> {
    let path = entry.path();
    let Ok(file_type) = entry.file_type() else {
        return Ok(());
    };
    out.push(path.clone());

    let next_depth = current_depth + 1;
    if file_type.is_dir() {
        handle_directory_entry(
            &path,
            canonical_root,
            next_depth,
            options,
            out,
            visited_canonical,
        )?;
    } else if file_type.is_symlink() {
        handle_symlink_entry(
            dir,
            &path,
            canonical_root,
            next_depth,
            options,
            out,
            visited_canonical,
        )?;
    }
    Ok(())
}

fn walk_recursive(
    dir: &Path,
    canonical_root: &Option<PathBuf>,
    current_depth: usize,
    options: &WalkOptions,
    out: &mut Vec<PathBuf>,
    visited_canonical: &mut Vec<PathBuf>,
) -> io::Result<()> {
    if current_depth > options.max_depth {
        return Ok(());
    }
    if !track_canonical_visit(dir, visited_canonical) {
        return Ok(());
    }
    let entries = read_sorted_entries(dir, options.sort_alphabetically)?;
    for entry in entries {
        process_entry(
            entry,
            dir,
            canonical_root,
            current_depth,
            options,
            out,
            visited_canonical,
        )?;
    }
    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_directory_deterministically() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let src_dir = manifest_dir.join("src");
        let entries1 = walk_dir(&src_dir, &WalkOptions::default()).expect("walks src");
        let entries2 = walk_dir(&src_dir, &WalkOptions::default()).expect("walks src");
        assert!(!entries1.is_empty());
        assert_eq!(entries1, entries2);
    }
}
