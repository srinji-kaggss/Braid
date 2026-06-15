#!/usr/bin/env bash
# Braid U8 — Day-0 CMS reference workflow (D16 "landing page as first full
# port"). Regenerates the committed evidence bundle under
# spec/braid/vectors/demo-port/ by driving the REAL `braid` binary through
# the author -> verify -> render legs of three demo-port CMS actions (modeled
# on the kernel's blueprints/afternow-port/ landing surface).
#
# This is the human/CI-reproducible artifact for PRD §8 ("3+ CMS reference
# actions admitted ... with journaled evidence"), minus the EXECUTION leg,
# which is blocked on the kernel Day-0 WASM runtime (U7, tracked in #6). The
# deferred seam is documented in the bundle's README.
#
# Usage:  scripts/demo-port.sh [path-to-braid-binary]
#         (defaults to target/debug/braid)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BRAID="${1:-$ROOT/target/debug/braid}"
FIX="$ROOT/crates/braid-cli/tests/fixtures/demo-port"
OUT="$ROOT/spec/braid/vectors/demo-port"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

[ -x "$BRAID" ] || { echo "braid binary not found/executable at $BRAID" >&2; exit 2; }
mkdir -p "$OUT"

pass() { printf '  \033[32mok\033[0m   %s\n' "$1"; }
fail() { printf '  \033[31mFAIL\033[0m %s\n' "$1"; exit 1; }

echo "demo-port evidence bundle  (binary: $BRAID)"

# The three admitted reference actions. For each: encode -> verify (ADMIT) ->
# render, writing the manifest + CID + verdict into the committed bundle.
for name in edit-home-hero publish-services render-work-listing; do
  cid="$("$BRAID" encode "$FIX/$name.json" -o "$WORK/$name.braid" 2>&1 >/dev/null | sed -n 's/^cid //p')"
  [ -n "$cid" ] || fail "$name: encode produced no CID"

  v="$("$BRAID" verify "$WORK/$name.braid")" || fail "$name: verify did not ADMIT (exit $?)"
  case "$v" in *ADMIT*) ;; *) fail "$name: verdict not ADMIT" ;; esac

  "$BRAID" render "$WORK/$name.braid" > "$OUT/$name.manifest.txt" || fail "$name: render failed"
  printf '%s\n' "$cid" > "$OUT/$name.cid"
  printf '%s\n' "$v"   > "$OUT/$name.verdict"
  pass "$name  ADMIT  $cid"
done

# The escalation probe: publishing without a confirm policy is refused at author
# time (exit 2). Record the refusal as evidence; emit no capsule.
set +e
nc_err="$("$BRAID" encode "$FIX/publish-services-noconfirm.json" -o "$WORK/nc.braid" 2>&1 >/dev/null)"
nc_code=$?
set -e
[ "$nc_code" = "2" ] || fail "publish-services-noconfirm: expected author refusal (exit 2), got $nc_code"
case "$nc_err" in *ConfirmRequired*) ;; *) fail "publish-services-noconfirm: expected ConfirmRequired" ;; esac
printf 'author-refused (exit 2): %s\n' "$nc_err" > "$OUT/publish-services-noconfirm.refused"
pass "publish-services-noconfirm  AUTHOR-REFUSED (ConfirmRequired)"

echo "evidence bundle written to spec/braid/vectors/demo-port/"
