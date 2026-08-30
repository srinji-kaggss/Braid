#!/usr/bin/env bash
#
# local-ci.sh — Braid's local CI, at parity with .github/workflows/ci.yml.
#
# Mirrors every code-changed lane of the GitHub workflow step for step:
# swallow budget, fmt, build, tests, doc tests, clippy, lgwks-std feature
# matrix, MSRV checks, contract drift, package smoke, consumption contract,
# deterministic registry export, and the locked tagged-Git consumer probe.
# Stack-position and Scope are pull-request-runner concerns and are skipped;
# a local receipt run is always full-scope.
#
# On a full pass it writes /Users/srinji/wwfd/state/local-ci-receipt.json,
# which wwfd-guard's spend gate reads to allow `git push`. The receipt is
# written ONLY after every step exits 0. There is no --skip, no || true,
# and no set +e: a gate that can be talked out of a failure is not a gate.
#
# Subject resolution (lessons L202/L206): the receipt certifies the exact
# Jujutsu working-copy commit that the feature bookmark will publish. A working
# copy without a local bookmark is refused; certifying colocated Git HEAD would
# attest @- while omitting the feature bytes in @.
#
# Usage:  bash .wwfd/local-ci.sh
#
# Exit 0 = green + receipt written. Non-zero = the failing step's status,
# and no receipt is written or refreshed.

set -euo pipefail

REPO_ROOT="$(jj root)"
RECEIPT="/Users/srinji/wwfd/state/local-ci-receipt.json"
cd "$REPO_ROOT"

step=0
run() {
  step=$((step + 1))
  printf '\n── [%02d] %s\n' "$step" "$1"
  shift
  "$@"
}

locked_metadata() {
  cargo metadata --locked --no-deps --format-version 1 >/dev/null
}

# ── Subject reachability ──────────────────────────────────────────────────────

SUBJECT_SHA="$(jj --no-pager log --no-graph -r @ -T 'commit_id')"
BOOKMARKS="$(jj --no-pager log --no-graph -r @ -T 'bookmarks')"
if [[ -z "$BOOKMARKS" ]]; then
  echo "local-ci: subject $SUBJECT_SHA has no local bookmark." >&2
  echo "  Put a bookmark on it first:" >&2
  echo "    jj bookmark create local-ci-subject -r @" >&2
  exit 128
fi
echo "subject: $SUBJECT_SHA (bookmarks: $BOOKMARKS)"

# ── Lane 1: swallow budget (ci.yml · swallow-budget) ─────────────────────────

count="$(grep -rc 'let _ = \|\.ok();' crates/ | awk -F: '{s+=$2} END{print s}')"
echo "swallowed-results=$count ceiling=5"
run "swallow budget ≤ 5" test "$count" -le 5

# ── Lane 2: formatting (ci.yml · fmt) ────────────────────────────────────────

run "cargo fmt --check" cargo fmt --all -- --check

# ── Lane 3–5: build, tests, doc tests (ci.yml · build/tests) ─────────────────

run "locked metadata"        locked_metadata
run "owned dependency edges" cargo run --locked -p lgwks_std_gate --bin lgwks-gate -- check .
run "build all targets"      cargo test --workspace --all-targets --locked --no-run
run "workspace tests"        cargo test --workspace --all-targets --locked
run "doc tests"              cargo test --workspace --doc --locked

# ── Lane 6: clippy (ci.yml · clippy) ─────────────────────────────────────────

run "clippy -D warnings" cargo clippy --workspace --all-targets --locked -- -D warnings

# ── Lane 7: lgwks-std feature matrix (ci.yml · lgwks-std-feature-matrix) ─────

run "std no-default-features"  cargo test -p lgwks_std --no-default-features --all-targets
run "std default features"     cargo test -p lgwks_std --all-targets
run "std hash-only"            cargo test -p lgwks_std --features hash --no-default-features --all-targets
run "std pattern-only"         cargo test -p lgwks_std --features pattern --no-default-features --all-targets
run "std json-only"            cargo test -p lgwks_std --features json --no-default-features --all-targets
run "std wire-only"            cargo test -p lgwks_std --features wire --no-default-features --all-targets
run "std all-features"         cargo test -p lgwks_std --all-features --all-targets

# ── Lane 8: MSRV feature checks (ci.yml · lgwks-std-msrv) ────────────────────

STD_MANIFEST="crates/lgwks-std/Cargo.toml"
run "msrv check no-default"    cargo check --manifest-path "$STD_MANIFEST" --all-targets --no-default-features
run "msrv check default"       cargo check --manifest-path "$STD_MANIFEST" --all-targets
run "msrv check hash"          cargo check --manifest-path "$STD_MANIFEST" --all-targets --features hash --no-default-features
run "msrv check pattern"       cargo check --manifest-path "$STD_MANIFEST" --all-targets --features pattern --no-default-features
run "msrv check json"          cargo check --manifest-path "$STD_MANIFEST" --all-targets --features json --no-default-features
run "msrv check wire"          cargo check --manifest-path "$STD_MANIFEST" --all-targets --features wire --no-default-features
run "msrv check all"           cargo check --manifest-path "$STD_MANIFEST" --all-targets --all-features
run "msrv check std-gate"      cargo check --manifest-path crates/lgwks-std-gate/Cargo.toml --all-targets

# ── Lane 9: contract drift (ci.yml · lgwks-std-contract-drift) ───────────────

run "contract drift" python3 - <<'PY'
import re
import tomllib
from pathlib import Path

manifest = Path("crates/lgwks-std/Cargo.toml")
readme = Path("crates/lgwks-std/README.md")

parsed = tomllib.loads(manifest.read_text(encoding="utf-8"))
manifest_deps = set(parsed["dependencies"].keys())

text = readme.read_text(encoding="utf-8")
marker = "## Dependency philosophy"
start = text.find(marker)
if start == -1:
    raise SystemExit("README dependency philosophy section missing")

section = text[start:start + 1800]
readme_deps = set(re.findall(r"- \*\*([a-zA-Z0-9_]+)\*\*", section))

missing_from_readme = sorted(manifest_deps - readme_deps)
missing_from_manifest = sorted(readme_deps - manifest_deps)
if missing_from_readme or missing_from_manifest:
    raise SystemExit(
        f"Dependency contract drift detected. "
        f"missing_from_readme={missing_from_readme}, "
        f"missing_from_manifest={missing_from_manifest}"
    )

package = parsed["package"]
if package["repository"] != "https://github.com/srinji-kaggss/Braid":
    raise SystemExit(f"Unexpected package repository URL: {package['repository']}")
PY

# ── Lane 10: package smoke (ci.yml · lgwks-std-package-smoke) ────────────────

run "package smoke" ./scripts/lgwks-std-package-smoke.sh

# ── Lane 11: consumption contract (ci.yml · lgwks-std-consumption-contract) ──

command -v rg >/dev/null 2>&1 || {
  echo "local-ci: rg required for the consumption-contract lane" >&2
  exit 127
}
no_path_fallback() {
  if rg -q 'lgwks_std\s*=\s*\{\s*path\s*=\s*"\.\./lgwks-std"\s*\}' -g '*.toml' crates; then
    echo "Contract breach: path-based lgwks_std dependency remains." >&2
    return 1
  fi
}
run "no path fallback" no_path_fallback
run "version pin present" rg -q \
  '^lgwks_std\s*=\s*\{\s*version\s*=\s*"0\.5\.1"\s*\}' -g '*.toml' Cargo.toml crates
run "local patch present" rg -q \
  '^lgwks_std\s*=\s*\{\s*path\s*=\s*"crates/lgwks-std"\s*\}' -g '*.toml' Cargo.toml

# ── Lane 12: Braid contract release boundary ────────────────────────────────

release_probe_rejects_bad_revision() {
  if ./scripts/braid-release-probe.sh "file://$REPO_ROOT" not-a-commit; then
    echo "release probe accepted a malformed revision" >&2
    return 1
  fi
}
run "registry export is deterministic" ./scripts/braid-registry-export-check.sh
run "release probe rejects bad revision" release_probe_rejects_bad_revision
run "locked tagged-Git consumer" \
  ./scripts/braid-release-probe.sh "file://$REPO_ROOT" "$SUBJECT_SHA"

# ── Receipt ──────────────────────────────────────────────────────────────────

BRANCH="$BOOKMARKS"
HEAD_SHA="${SUBJECT_SHA:0:12}"
printf '{"repo_root":"%s","branch":"%s","head_sha":"%s","attested_at":%s,"ci_script":"bash .wwfd/local-ci.sh"}\n' \
  "$REPO_ROOT" "$BRANCH" "$HEAD_SHA" "$(date +%s)" > "$RECEIPT"
echo
echo "GREEN — receipt written: $RECEIPT"
echo "  $(cat "$RECEIPT")"
