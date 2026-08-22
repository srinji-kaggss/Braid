//! `glob` owns shell-style path pattern matching and enforces
//! INV-GLOB-SEPARATOR: a single `*`, a `?`, and a character class never cross a
//! `/`, and only `**` does — so a pattern cannot silently reach into a
//! subdirectory the author did not name.
//!
//! Retires the `glob` crate, declared in 2 manifests and reached from 3 call
//! sites. The estate's use is pattern *matching*
//! against paths it already has, not filesystem traversal, so directory walking
//! is deliberately out of scope: matching is pure and testable, traversal is an
//! I/O concern that belongs to the caller.
//!
//! An unterminated `[` is treated as a literal bracket rather than an error.
//! That is the POSIX `fnmatch` behaviour and the upstream crate's, and matching
//! it keeps the migration a substitution instead of a semantic change.

// ── Matching ────────────────────────────────────────────────────────────────

/// Reports whether `path` matches `pattern`.
///
/// Supported syntax: `?` for one non-separator character, `*` for any run of
/// non-separator characters, `**` for any run including separators, and
/// `[abc]` / `[a-z]` / `[!a-z]` character classes.
pub fn matches(pattern: &str, path: &str) -> bool {
    match_bytes(pattern.as_bytes(), path.as_bytes())
}

fn match_bytes(pattern: &[u8], path: &[u8]) -> bool {
    let Some(&head) = pattern.first() else {
        return path.is_empty();
    };
    match head {
        b'*' if pattern.get(1) == Some(&b'*') => {
            // `**` spans separators. A trailing `/` after it is consumed so
            // `a/**/b` also matches `a/b` with nothing in between.
            let rest = if pattern.get(2) == Some(&b'/') { &pattern[3..] } else { &pattern[2..] };
            if match_bytes(rest, path) {
                return true;
            }
            (0..path.len()).any(|i| match_bytes(rest, &path[i + 1..]))
        }
        b'*' => {
            let rest = &pattern[1..];
            let mut consumed = 0;
            loop {
                if match_bytes(rest, &path[consumed..]) {
                    return true;
                }
                if consumed >= path.len() || path[consumed] == b'/' {
                    return false;
                }
                consumed += 1;
            }
        }
        b'?' => {
            matches!(path.first(), Some(&c) if c != b'/') && match_bytes(&pattern[1..], &path[1..])
        }
        b'[' => match class_end(pattern) {
            Some(end) => {
                matches!(path.first(), Some(&c) if c != b'/' && class_matches(&pattern[1..end], c))
                    && match_bytes(&pattern[end + 1..], &path[1..])
            }
            // Unterminated class: the bracket is a literal.
            None => path.first() == Some(&b'[') && match_bytes(&pattern[1..], &path[1..]),
        },
        literal => path.first() == Some(&literal) && match_bytes(&pattern[1..], &path[1..]),
    }
}

// ── Character classes ───────────────────────────────────────────────────────

/// Index of the `]` that closes the class opening at `pattern[0]`, or `None` if
/// the class is unterminated. A `]` in the first content position is a literal,
/// per POSIX.
fn class_end(pattern: &[u8]) -> Option<usize> {
    let mut i = 1;
    if matches!(pattern.get(i), Some(b'!' | b'^')) {
        i += 1;
    }
    if pattern.get(i) == Some(&b']') {
        i += 1;
    }
    while i < pattern.len() {
        if pattern[i] == b']' {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn class_matches(body: &[u8], candidate: u8) -> bool {
    let (negated, body) = match body.first() {
        Some(b'!' | b'^') => (true, &body[1..]),
        _ => (false, body),
    };
    let mut hit = false;
    let mut i = 0;
    while i < body.len() {
        // A `-` with members either side is a range; a leading or trailing `-`
        // is a literal.
        if i + 2 < body.len() && body[i + 1] == b'-' {
            if body[i] <= candidate && candidate <= body[i + 2] {
                hit = true;
            }
            i += 3;
        } else {
            if body[i] == candidate {
                hit = true;
            }
            i += 1;
        }
    }
    hit != negated
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_literal_pattern_matches_only_itself() {
        assert!(matches("src/lib.rs", "src/lib.rs"));
        assert!(!matches("src/lib.rs", "src/main.rs"));
    }

    #[test]
    fn star_does_not_cross_a_separator() {
        assert!(matches("src/*.rs", "src/lib.rs"));
        assert!(!matches("src/*.rs", "src/inner/lib.rs"));
        assert!(matches("*", "lib.rs"));
        assert!(!matches("*", "src/lib.rs"));
    }

    #[test]
    fn double_star_crosses_separators() {
        assert!(matches("src/**/*.rs", "src/a/b/c.rs"));
        assert!(matches("**/*.rs", "lib.rs"));
        assert!(matches("**", "a/b/c"));
    }

    #[test]
    fn double_star_matches_nothing_in_between() {
        assert!(matches("a/**/b", "a/b"));
        assert!(matches("a/**/b", "a/x/b"));
        assert!(matches("a/**/b", "a/x/y/b"));
        assert!(!matches("a/**/b", "a/x/y/c"));
    }

    #[test]
    fn question_mark_matches_one_non_separator() {
        assert!(matches("?.rs", "a.rs"));
        assert!(!matches("?.rs", "ab.rs"));
        assert!(!matches("a?b", "a/b"));
    }

    #[test]
    fn a_character_class_matches_one_member() {
        assert!(matches("[abc].rs", "b.rs"));
        assert!(!matches("[abc].rs", "d.rs"));
    }

    #[test]
    fn a_character_class_supports_ranges() {
        assert!(matches("[a-z][0-9].rs", "a1.rs"));
        assert!(!matches("[a-z][0-9].rs", "1a.rs"));
    }

    #[test]
    fn a_negated_class_excludes_its_members() {
        assert!(matches("[!abc].rs", "d.rs"));
        assert!(!matches("[!abc].rs", "a.rs"));
        assert!(matches("[^0-9].rs", "a.rs"));
    }

    #[test]
    fn a_class_never_matches_a_separator() {
        assert!(!matches("a[!x]b", "a/b"));
    }

    #[test]
    fn an_unterminated_class_is_a_literal_bracket() {
        assert!(matches("[abc", "[abc"));
        assert!(!matches("[abc", "a"));
    }

    #[test]
    fn a_trailing_dash_in_a_class_is_a_literal() {
        assert!(matches("[a-]", "-"));
        assert!(matches("[a-]", "a"));
    }

    #[test]
    fn an_empty_pattern_matches_only_an_empty_path() {
        assert!(matches("", ""));
        assert!(!matches("", "a"));
        assert!(matches("*", ""));
    }

    #[test]
    fn backtracking_terminates_on_a_pathological_pattern() {
        // Guards against the exponential blow-up shape `a*a*a*...b` on a
        // non-matching input; the separator bound keeps each `*` local.
        assert!(!matches("a*a*a*a*a*b", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
    }
}
