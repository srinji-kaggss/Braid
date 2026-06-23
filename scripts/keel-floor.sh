#!/usr/bin/env bash
# Braid U-SA — run Keel safety-assurance floor against the workspace.
#
# Produces the Tier-2 semantic verdict: reads Tier-1 evidence (cargo test,
# clippy, etc.) and evaluates the NotSlop concept via Keel's engine.
#
# Usage:  scripts/keel-floor.sh [--concept <id>]
#         (defaults to NotSlop gate concept)
#
# Requires: node (>=22), cargo, the keel repo at ~/keel (or KEEL_ROOT).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KEEL="${KEEL_ROOT:-$HOME/keel}"
PROFILE="$ROOT/braid.profile.json"

if [ ! -f "$KEEL/src/run.mjs" ]; then
  echo "Keel not found at $KEEL (set KEEL_ROOT to override)" >&2
  exit 2
fi

if [ ! -f "$PROFILE" ]; then
  echo "braid.profile.json not found at $PROFILE" >&2
  exit 2
fi

CONCEPT="${1:-}"
if [ "$CONCEPT" = "--concept" ] && [ -n "${2:-}" ]; then
  CONCEPT_FLAG=(--concept "$2")
  shift 2
else
  CONCEPT_FLAG=()
fi

echo "Keel safety-assurance floor (profile: $PROFILE)"
echo "  keel:    $KEEL"
echo "  concept: ${CONCEPT_FLAG[1]:-NotSlop (default)}"
echo ""

node "$KEEL/src/run.mjs" --profile "$PROFILE" "${CONCEPT_FLAG[@]+"${CONCEPT_FLAG[@]}"}"
