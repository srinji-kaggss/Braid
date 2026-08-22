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
    walk_recursive(root_path, &canonical_root, 0, options, &mut out, &mut visited)?;
    Ok(out)
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

    if let Ok(canonical) = dir.canonicalize() {
        if visited_canonical.contains(&canonical) {
            // Symlink loop detected — prevent infinite recursion.
            return Ok(());
        }
        visited_canonical.push(canonical);
    }

    let mut entries: Vec<DirEntry> = fs::read_dir(dir)?.filter_map(Result::ok).collect();
    if options.sort_alphabetically {
        entries.sort_by_key(|e| e.file_name());
    }

    for entry in entries {
        let path = entry.path();
        if let Ok(file_type) = entry.file_type() {
            out.push(path.clone());

            if file_type.is_dir() {
                walk_recursive(&path, canonical_root, current_depth + 1, options, out, visited_canonical)?;
            } else if file_type.is_symlink() && options.follow_symlinks {
                if let Ok(target) = fs::read_link(&path) {
                    let resolved = if target.is_relative() {
                        dir.join(target)
                    } else {
                        target
                    };
                    if let Ok(resolved_canon) = resolved.canonicalize() {
                        // Sandbox check: symlink must not escape the walk root.
                        if let Some(ref canon_root) = canonical_root {
                            if !resolved_canon.starts_with(canon_root) {
                                continue;
                            }
                        }
                        if resolved_canon.is_dir() {
                            walk_recursive(&resolved, canonical_root, current_depth + 1, options, out, visited_canonical)?;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_directory_deterministically() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let options = WalkOptions {
            max_depth: 2,
            follow_symlinks: false,
            sort_alphabetically: true,
        };

        let paths = walk_dir(&root, &options).expect("walk successful");
        assert!(!paths.is_empty());
        assert!(paths.iter().any(|p| p.ends_with("Cargo.toml")));
        assert!(paths.iter().any(|p| p.ends_with("src")));
    }
}
