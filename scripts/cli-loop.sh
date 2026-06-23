#!/usr/bin/env bash
# Braid scenario #12 — the human-reconstructable CLI loop, plus the T12
# manifest-widening gate self-test (ADR-088 U6 #2).
#
# This is the EXECUTABLE form of the acceptance scenario: a human (or CI) with
# only the `braid` binary drives author -> verify -> render -> diff and the
# script asserts every exit code. No Rust toolchain knowledge required beyond
# building the binary once.
#
# Usage:  scripts/cli-loop.sh [path-to-braid-binary]
#         (defaults to target/debug/braid; CI passes the release path)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BRAID="${1:-$ROOT/target/debug/braid}"
FIX="$ROOT/crates/braid-cli/tests/fixtures"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

[ -x "$BRAID" ] || { echo "braid binary not found/executable at $BRAID" >&2; exit 2; }

pass() { printf '  \033[32mok\033[0m   %s\n' "$1"; }
fail() { printf '  \033[31mFAIL\033[0m %s\n' "$1"; exit 1; }

# expect_exit <wanted> <label> -- <cmd...>
expect_exit() {
  local want="$1" label="$2"; shift 3
  set +e; "$@" >"$WORK/out" 2>"$WORK/err"; local got=$?; set -e
  [ "$got" = "$want" ] || { echo "--- stdout ---"; cat "$WORK/out"; echo "--- stderr ---"; cat "$WORK/err"; fail "$label (wanted exit $want, got $got)"; }
  pass "$label"
}

echo "Braid CLI loop  (binary: $BRAID)"

# 1. Scenario #12 — author -> verify -> render, the happy path.
expect_exit 0 "encode edit_section"          -- "$BRAID" encode "$FIX/edit_section.json"          -o "$WORK/edit.braid"
expect_exit 0 "verify  edit_section (ADMIT)" -- "$BRAID" verify "$WORK/edit.braid"
expect_exit 0 "render  edit_section"         -- "$BRAID" render "$WORK/edit.braid"
expect_exit 0 "decode  edit_section"         -- "$BRAID" decode "$WORK/edit.braid"

# CID parity (T13): CLI-authored edit_section == the pinned reference KAT.
CID="$("$BRAID" encode "$FIX/edit_section.json" -o "$WORK/k.braid" 2>&1 >/dev/null | sed -n 's/^cid //p')"
[ "$CID" = "ccedc469e6b0513720969ce1a4f169f53365eeadbc853042c411b44c1f15b71f" ] \
  && pass "CID parity with pinned KAT" || fail "CID drift: $CID"

# 2. The irreversible publish admits only with its confirm policy intact.
expect_exit 0 "encode publish (human-confirm)" -- "$BRAID" encode "$FIX/publish.json" -o "$WORK/pub.braid"
expect_exit 0 "verify publish (ADMIT)"         -- "$BRAID" verify "$WORK/pub.braid"

# 3. The laundering capsule is REJECTED at the taint stage (policy-negative=1).
expect_exit 0 "encode laundering"            -- "$BRAID" encode "$FIX/laundering.json" -o "$WORK/laundry.braid"
expect_exit 1 "verify laundering (REJECT)"   -- "$BRAID" verify "$WORK/laundry.braid"

# 4. T12 — the manifest-widening gate. Base vs a capsule that grows authority
#    must be flagged WIDENING and fail (exit 1); the reverse is a narrowing
#    (exit 0). This is the seeded-widening red-team evidence in CI.
expect_exit 0 "encode edit_section_widened"  -- "$BRAID" encode "$FIX/edit_section_widened.json" -o "$WORK/wide.braid"
expect_exit 1 "diff  base->widened (gate fires)"  -- "$BRAID" diff "$WORK/edit.braid" "$WORK/wide.braid"
expect_exit 0 "diff  widened->base (narrowing ok)" -- "$BRAID" diff "$WORK/wide.braid" "$WORK/edit.braid"
expect_exit 0 "diff  identical (no change)"   -- "$BRAID" diff "$WORK/edit.braid" "$WORK/edit.braid"

# 5. U8 — the Day-0 CMS reference actions (demo-port, D16). The real
#    landing-surface verbs admit/refuse correctly on the no-Rust CLI path.
DP="$ROOT/crates/braid-cli/tests/fixtures/demo-port"
expect_exit 0 "encode dp:edit-home-hero"        -- "$BRAID" encode "$DP/edit-home-hero.json"      -o "$WORK/dp_edit.braid"
expect_exit 0 "verify dp:edit-home-hero (ADMIT)" -- "$BRAID" verify "$WORK/dp_edit.braid"
expect_exit 0 "encode dp:publish-services"       -- "$BRAID" encode "$DP/publish-services.json"    -o "$WORK/dp_pub.braid"
expect_exit 0 "verify dp:publish-services (ADMIT)" -- "$BRAID" verify "$WORK/dp_pub.braid"
expect_exit 0 "encode dp:render-work-listing"    -- "$BRAID" encode "$DP/render-work-listing.json" -o "$WORK/dp_list.braid"
expect_exit 0 "verify dp:render-work-listing (ADMIT)" -- "$BRAID" verify "$WORK/dp_list.braid"
expect_exit 2 "encode dp:publish-noconfirm (author-refused)" -- "$BRAID" encode "$DP/publish-services-noconfirm.json" -o "$WORK/dp_nc.braid"

echo "all CLI-loop assertions passed"
