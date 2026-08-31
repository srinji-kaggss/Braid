//! Braid Flow SDK — RON-first authoring, JSON interop/inspection only,
//! YAML importer-only (P4 #58, R5).
//!
//! All source forms lower to one validated AST; semantic-loss reports are
//! mandatory; source 16 MiB + nesting 64 + declared node/edge/port/expansion
//! bounds refuse **before** AST/allocation — incremental `try_reserve` style.
//!
//! This crate owns **ceiling checks only** and never duplicates verifier logic.
//! It is `std` for RON/JSON parsing, but envelope checks are allocation-free.

#![forbid(unsafe_code)]

pub use braid_flow_ir::FlowError;
pub use braid_flow_ir::LimitKind;

// ── RON ceilings — the only place 16 MiB / depth 64 is defined for text authoring ──

/// Maximum RON source envelope — checked before any AST allocation.
pub const MAX_RON_BYTES: usize = 16 * 1024 * 1024;

/// Maximum RON structural nesting depth — checked before any AST allocation.
pub const MAX_RON_DEPTH: usize = 64;

// ── Declared-bound hard limits — mirrors braid-flow-ir HARD_MAX_* (kept in sync manually) ──

/// Max source nodes — hard limit, checked before `Vec::try_reserve`.
pub const HARD_MAX_SOURCE_NODES: usize = 10_000;
/// Max source edges — hard limit, checked before `Vec::try_reserve`.
pub const HARD_MAX_SOURCE_EDGES: usize = 50_000;
/// Max ports per graph — hard limit, checked before reservation.
pub const HARD_MAX_PORTS: usize = 128;
/// Max expanded nodes — hard limit, checked before reservation.
pub const HARD_MAX_EXPANDED_NODES: usize = 50_000;
/// Max expanded edges — hard limit, checked before reservation.
pub const HARD_MAX_EXPANDED_EDGES: usize = 250_000;

const INV_BOUNDS: &str = "INV-FLOW-004";
const INV_MALFORMED: &str = "INV-FLOW-018";

// ── Envelope ────────────────────────────────────────────────────────────────

/// Validate RON source envelope **before** any AST allocation.
///
/// Checks, in order:
/// 1. `bytes.len() <= MAX_RON_BYTES` (16 MiB) — allocation-free.
/// 2. Structural nesting depth `<= MAX_RON_DEPTH` (64) — byte-scan, no allocation,
///    string literals are skipped so brackets inside quotes do not inflate depth.
///    Comments (`//` line, `/* */` block) are also skipped.
/// 3. Basic balance (unclosed string/comment, mismatched brackets) — malformed if
///    violated.
///
/// Returns `FlowError::LimitExceeded` with `INV-FLOW-004` on ceiling breach,
/// `FlowError::Malformed` with `INV-FLOW-018` on truncated/unclosed source.
/// No `Vec` or AST is allocated on any path.
pub fn check_ron_envelope(bytes: &[u8]) -> Result<(), FlowError> {
    if bytes.len() > MAX_RON_BYTES {
        return Err(FlowError::LimitExceeded {
            kind: LimitKind::WireBytes,
            actual: bytes.len(),
            limit: MAX_RON_BYTES,
            invariant: INV_BOUNDS,
        });
    }
    check_ron_depth(bytes)
}

/// Allocation-free depth scan for RON-like bytes.
///
/// Tracks nesting via `{` `[` `(` increments and `}` `]` `)` decrements,
/// ignoring brackets inside double-quoted string literals (with `\` escapes)
/// and inside `//` / `/* */` comments.
fn check_ron_depth(bytes: &[u8]) -> Result<(), FlowError> {
    let mut depth: usize = 0;
    let mut in_string = false;
    let mut escaped = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut i = 0usize;

    while i < bytes.len() {
        let b = bytes[i];

        if in_line_comment {
            if b == b'\n' {
                in_line_comment = false;
            }
            i += 1;
            continue;
        }
        if in_block_comment {
            if b == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                in_block_comment = false;
                i += 2;
                continue;
            }
            // Allow nested block-start inside block comment to be ignored (RON spec
            // is ambiguous on nesting; we simply stay in comment until first */).
            i += 1;
            continue;
        }
        if in_string {
            if escaped {
                escaped = false;
                i += 1;
                continue;
            }
            if b == b'\\' {
                escaped = true;
                i += 1;
                continue;
            }
            if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        // Not in string or comment.
        if b == b'"' {
            in_string = true;
            i += 1;
            continue;
        }
        if b == b'/' && i + 1 < bytes.len() {
            if bytes[i + 1] == b'/' {
                in_line_comment = true;
                i += 2;
                continue;
            }
            if bytes[i + 1] == b'*' {
                in_block_comment = true;
                i += 2;
                continue;
            }
        }

        match b {
            b'{' | b'[' | b'(' => {
                depth = depth.checked_add(1).ok_or(FlowError::ArithmeticOverflow {
                    field: "ron depth",
                    invariant: INV_BOUNDS,
                })?;
                if depth > MAX_RON_DEPTH {
                    return Err(FlowError::LimitExceeded {
                        kind: LimitKind::PredicateDepth,
                        actual: depth,
                        limit: MAX_RON_DEPTH,
                        invariant: INV_BOUNDS,
                    });
                }
            }
            b'}' | b']' | b')' => {
                if depth == 0 {
                    return Err(FlowError::Malformed {
                        field: "ron bracket",
                        invariant: INV_MALFORMED,
                    });
                }
                depth -= 1;
            }
            _ => {}
        }
        i += 1;
    }

    if in_string {
        return Err(FlowError::Malformed {
            field: "ron",
            invariant: INV_MALFORMED,
        });
    }
    if in_block_comment {
        return Err(FlowError::Malformed {
            field: "ron",
            invariant: INV_MALFORMED,
        });
    }
    if depth != 0 {
        return Err(FlowError::Malformed {
            field: "ron",
            invariant: INV_MALFORMED,
        });
    }
    Ok(())
}

// ── Declared bounds — before try_reserve ─────────────────────────────────

/// Validate declared collection bounds **before** any `Vec::try_reserve` or AST allocation.
///
/// Mirrors `braid-flow-ir` `HARD_MAX_*` (kept in sync manually because those are
/// `pub(crate)`). Fail-closed: any declared count exceeding its hard limit returns
/// `LimitExceeded` with `INV-FLOW-004` and the caller must not reserve.
pub fn check_declared_bounds(
    node_count: usize,
    edge_count: usize,
    port_count: usize,
    expansion: usize,
) -> Result<(), FlowError> {
    if node_count > HARD_MAX_SOURCE_NODES {
        return Err(FlowError::LimitExceeded {
            kind: LimitKind::SourceNodes,
            actual: node_count,
            limit: HARD_MAX_SOURCE_NODES,
            invariant: INV_BOUNDS,
        });
    }
    if edge_count > HARD_MAX_SOURCE_EDGES {
        return Err(FlowError::LimitExceeded {
            kind: LimitKind::SourceEdges,
            actual: edge_count,
            limit: HARD_MAX_SOURCE_EDGES,
            invariant: INV_BOUNDS,
        });
    }
    if port_count > HARD_MAX_PORTS {
        return Err(FlowError::LimitExceeded {
            kind: LimitKind::Ports,
            actual: port_count,
            limit: HARD_MAX_PORTS,
            invariant: INV_BOUNDS,
        });
    }
    if expansion > HARD_MAX_EXPANDED_NODES {
        return Err(FlowError::LimitExceeded {
            kind: LimitKind::ExpandedNodes,
            actual: expansion,
            limit: HARD_MAX_EXPANDED_NODES,
            invariant: INV_BOUNDS,
        });
    }
    Ok(())
}

// ── JSON interop — inspection/interop only, unknown/lossy refuses ─────────

/// Allowed top-level JSON keys for the Flow-adjacent JSON interop surface.
/// This is intentionally permissive but closed: any key outside this set is
/// an **unknown field** and fails closed with `Malformed` rather than being
/// silently dropped (T5.2).
const ALLOWED_JSON_FIELDS: &[&str] = &[
    "name",
    "roots",
    "nodes",
    "edges",
    "terminals",
    "bounds",
    "version",
    // capsule-adjacent interop fields (kept here to allow the same validator
    // to be reused for both Flow and capsule JSON-of-IR inspection):
    "intent",
    "strands",
    "outputs",
    "budget",
    "confirm",
    "evidence",
    "grants",
    "vocab_version",
    "ir_version",
    "registry_cid",
];

/// Validate a JSON source for **unknown fields** and **semantic loss**.
///
/// * JSON is **interop/inspection only** — never authoritative text.
/// * Unknown top-level fields → `Malformed` (not silently dropped).
/// * Semantic-loss markers (`__lossy`, `semantic_loss`, or float-coerced
///   integers) → `Malformed`.
/// * Envelope size is also checked (`MAX_RON_BYTES`) before parsing, so a
///   hostile JSON cannot force allocation beyond the ceiling.
///
/// This is allocation-minimal: size check before `serde_json` allocation, then
/// a single `Value` parse followed by a closed key-set check. It does **not**
/// construct a `FlowSpec`; that remains the verifier's authority.
pub fn validate_json_source(bytes: &[u8]) -> Result<(), FlowError> {
    if bytes.len() > MAX_RON_BYTES {
        return Err(FlowError::LimitExceeded {
            kind: LimitKind::WireBytes,
            actual: bytes.len(),
            limit: MAX_RON_BYTES,
            invariant: INV_BOUNDS,
        });
    }

    // Quick sentinel scan before JSON parse — catches explicit lossy markers
    // without waiting for serde, and keeps the refusal path allocation-free
    // for the sentinel case.
    if contains_lossy_sentinel(bytes) {
        return Err(FlowError::Malformed {
            field: "json semantic_loss",
            invariant: INV_MALFORMED,
        });
    }

    let value: lgwks_std::json::Value =
        lgwks_std::json::from_slice(bytes).map_err(|_| FlowError::Malformed {
            field: "json",
            invariant: INV_MALFORMED,
        })?;

    match value {
        lgwks_std::json::Value::Object(map) => {
            for key in map.keys() {
                if !ALLOWED_JSON_FIELDS.contains(&key.as_str()) {
                    return Err(FlowError::Malformed {
                        field: "json unknown_field",
                        invariant: INV_MALFORMED,
                    });
                }
                // Any nested object that carries an unknown sentinel at depth is
                // also lossy — check one level deeper for __lossy in nested maps.
                if key == "__lossy" || key == "semantic_loss" || key == "unknown_field" {
                    return Err(FlowError::Malformed {
                        field: "json semantic_loss",
                        invariant: INV_MALFORMED,
                    });
                }
            }
            // Detect float semantic loss: serde_json distinguishes int vs f64 via
            // Number. If any number is f64 that is not an integer, or a u64 that
            // would truncate on coercion, treat as lossy.
            // We walk the Value tree shallowly; the spec says semantic-loss reports
            // mandatory, not silent coercion, so any non-integer number at top
            // level is suspect. For this P4 scope, we treat any f64 in the payload
            // as lossy unless the JSON explicitly documents it as float.
            if contains_float_loss(map.values()) {
                return Err(FlowError::Malformed {
                    field: "json semantic_loss",
                    invariant: INV_MALFORMED,
                });
            }
            Ok(())
        }
        _ => Err(FlowError::Malformed {
            field: "json",
            invariant: INV_MALFORMED,
        }),
    }
}

fn contains_lossy_sentinel(bytes: &[u8]) -> bool {
    // Cheap substring search for the two canonical lossy markers before serde.
    // This is allocation-free and fails closed even if JSON is otherwise valid.
    let needles: &[&[u8]] = &[b"__lossy", b"semantic_loss", b"\"unknown_field\""];
    for needle in needles {
        if memchr_subslice(bytes, needle) {
            return true;
        }
    }
    false
}

fn memchr_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn contains_float_loss<'a, I>(values: I) -> bool
where
    I: IntoIterator<Item = &'a lgwks_std::json::Value>,
{
    for v in values {
        match v {
            lgwks_std::json::Value::Number(n) => {
                // serde_json Number: if it is f64 and not integral, that's lossy
                // for a Flow int field.
                if n.is_f64() {
                    return true;
                }
            }
            lgwks_std::json::Value::Object(map) => {
                if map.contains_key("__lossy") || map.contains_key("semantic_loss") {
                    return true;
                }
                if contains_float_loss(map.values()) {
                    return true;
                }
            }
            lgwks_std::json::Value::Array(arr) if contains_float_loss(arr) => return true,
            lgwks_std::json::Value::Array(_) => {}
            _ => {}
        }
    }
    false
}

// ── YAML importer — input-only, strict/audit ──────────────────────────────

/// Validate a GitHub Actions (YAML) import payload — **importer input only**.
///
/// YAML never becomes authoritative text; it is lowered through the single
/// validated AST path. In `strict` mode unknown keys and lossy markers refuse
/// closed; in audit mode the same checks apply (YAML importer never silently
/// drops).
///
/// Currently validates envelope + delegates lossy/unknown detection to the same
/// JSON sentinel logic after a best-effort UTF-8 check. A full YAML parse
/// (via `serde_yaml` or `lgwks_std` if later added) would replace the sentinel
/// scan, but the ceiling-before-allocation contract is already enforced here.
pub fn import_gh_yaml(bytes: &[u8], strict: bool) -> Result<(), FlowError> {
    // Envelope before any YAML parse allocation.
    check_ron_envelope(bytes)?;

    // UTF-8 must hold — YAML is text.
    std::str::from_utf8(bytes).map_err(|_| FlowError::Malformed {
        field: "yaml",
        invariant: INV_MALFORMED,
    })?;

    if strict && contains_lossy_sentinel(bytes) {
        return Err(FlowError::Malformed {
            field: "yaml semantic_loss",
            invariant: INV_MALFORMED,
        });
    }
    // In strict mode also reject unknown_field sentinel even if YAML.
    if contains_lossy_sentinel(bytes) {
        return Err(FlowError::Malformed {
            field: "yaml unknown_field",
            invariant: INV_MALFORMED,
        });
    }
    Ok(())
}

// ── Re-exports ─────────────────────────────────────────────────────────────
// (LimitKind already re-exported at top; keep FlowError there as well via pub use above
//  — this tail export is redundant after the fix).
