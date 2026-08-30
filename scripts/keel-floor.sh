#!/usr/bin/env bash
# Braid U-SA — run Keel safety-assurance floor against the workspace.
#
# Runs the current native Keel control plane against this checkout. The old
# profile adapter used `keel/src/run.mjs`; that entry point no longer exists.
# This command fails explicitly when the native binary is unavailable instead
# of falling back to a sibling source tree or advertising stale evidence.
#
# Usage:  scripts/keel-floor.sh
#
# Requires: a separately installed native `keel` binary (or KEEL_BIN).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KEEL_BIN="${KEEL_BIN:-}"

if [[ $# -ne 0 ]]; then
  echo "usage: $0" >&2
  exit 64
fi

if [[ -z "$KEEL_BIN" ]]; then
  if command -v keel >/dev/null 2>&1; then
    KEEL_BIN="$(command -v keel)"
  fi
fi
if [[ -z "$KEEL_BIN" || ! -x "$KEEL_BIN" ]]; then
  echo "native Keel is unavailable; install it or set KEEL_BIN" >&2
  echo "issue #78 tracks a hermetic Braid assurance distribution" >&2
  exit 127
fi

echo "Keel native assurance scan"
echo "  binary: $KEEL_BIN"
echo "  target: $ROOT"
exec "$KEEL_BIN" "$ROOT"
