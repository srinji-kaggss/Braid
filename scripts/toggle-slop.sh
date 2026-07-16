#!/usr/bin/env bash
# Seeded-slop fixture toggle — apply/revert red-team fixtures (PB-01 W1).
#
# Each fixture introduces a specific class of slop into the Braid workspace.
# When applied, cargo test fails, which makes keel-floor.sh report the
# failing atom (specification_fidelity / testability_falsifiability).
#
# Usage:
#   scripts/toggle-slop.sh apply  <fixture>   # introduce slop
#   scripts/toggle-slop.sh revert <fixture>   # remove slop
#   scripts/toggle-slop.sh list               # list available fixtures
#   scripts/toggle-slop.sh status             # show applied state
#
# Fixtures are OFF by default. CI should never apply them — they are for
# demonstration and red-team verification only.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURES="$ROOT/fixtures/known-bad"

list_fixtures() {
    echo "Available fixtures:"
    for d in "$FIXTURES"/*/; do
        name=$(basename "$d")
        echo "  $name"
    done
}

show_status() {
    echo "Fixture status:"
    # re-derived-primitive: check if canon_dup.rs is in braid-ir/src
    if [ -f "$ROOT/crates/braid-ir/src/canon_dup.rs" ]; then
        echo "  re-derived-primitive: APPLIED"
    else
        echo "  re-derived-primitive: off"
    fi
    # ungrounded-claim: check if overclaim.rs is in braid-ir/src
    if [ -f "$ROOT/crates/braid-ir/src/overclaim.rs" ]; then
        echo "  ungrounded-claim: APPLIED"
    else
        echo "  ungrounded-claim: off"
    fi
    # vacuous-test: check if slop.rs is in braid-ir/tests
    if [ -f "$ROOT/crates/braid-ir/tests/slop.rs" ]; then
        echo "  vacuous-test: APPLIED"
    else
        echo "  vacuous-test: off"
    fi
}

apply_fixture() {
    local name="$1"
    local dir="$FIXTURES/$name"
    if [ ! -d "$dir" ]; then
        echo "Unknown fixture: $name" >&2
        list_fixtures >&2
        exit 1
    fi
    case "$name" in
        re-derived-primitive)
            cp "$dir/canon_dup.rs" "$ROOT/crates/braid-ir/src/canon_dup.rs"
            if ! grep -q 'pub mod canon_dup' "$ROOT/crates/braid-ir/src/lib.rs"; then
                sed -i.bak 's/^pub mod value;/pub mod value;\npub mod canon_dup;/' "$ROOT/crates/braid-ir/src/lib.rs"
                rm -f "$ROOT/crates/braid-ir/src/lib.rs.bak"
            fi
            echo "Applied: $name — canon_dup.rs added to braid-ir"
            echo "  Expected failure: cargo test -p braid-ir fails (encode divergence)"
            echo "  Keel atom: specification_fidelity → NotSlop RED"
            ;;
        ungrounded-claim)
            cp "$dir/overclaim.rs" "$ROOT/crates/braid-ir/src/overclaim.rs"
            if ! grep -q 'pub mod overclaim' "$ROOT/crates/braid-ir/src/lib.rs"; then
                sed -i.bak 's/^pub mod value;/pub mod value;\npub mod overclaim;/' "$ROOT/crates/braid-ir/src/lib.rs"
                rm -f "$ROOT/crates/braid-ir/src/lib.rs.bak"
            fi
            echo "Applied: $name — overclaim.rs added to braid-ir"
            echo "  Expected failure: cargo test -p braid-ir fails (ungrounded totality claim)"
            echo "  Keel atom: specification_fidelity → NotSlop RED"
            ;;
        vacuous-test)
            cp "$dir/slop.rs" "$ROOT/crates/braid-ir/tests/slop.rs"
            echo "Applied: $name — slop.rs added to braid-ir/tests"
            echo "  Expected failure: cargo test -p braid-ir fails (guard catches vacuous assertion)"
            echo "  Keel atom: testability_falsifiability → NotSlop RED"
            ;;
        *)
            echo "Unknown fixture: $name" >&2
            exit 1
            ;;
    esac
}

revert_fixture() {
    local name="$1"
    case "$name" in
        re-derived-primitive)
            rm -f "$ROOT/crates/braid-ir/src/canon_dup.rs"
            sed -i.bak '/^pub mod canon_dup;$/d' "$ROOT/crates/braid-ir/src/lib.rs"
            rm -f "$ROOT/crates/braid-ir/src/lib.rs.bak"
            echo "Reverted: $name"
            ;;
        ungrounded-claim)
            rm -f "$ROOT/crates/braid-ir/src/overclaim.rs"
            sed -i.bak '/^pub mod overclaim;$/d' "$ROOT/crates/braid-ir/src/lib.rs"
            rm -f "$ROOT/crates/braid-ir/src/lib.rs.bak"
            echo "Reverted: $name"
            ;;
        vacuous-test)
            rm -f "$ROOT/crates/braid-ir/tests/slop.rs"
            echo "Reverted: $name"
            ;;
        *)
            echo "Unknown fixture: $name" >&2
            exit 1
            ;;
    esac
}

case "${1:-}" in
    apply)  apply_fixture "${2:-}" ;;
    revert) revert_fixture "${2:-}" ;;
    list)   list_fixtures ;;
    status) show_status ;;
    *)      echo "Usage: $0 {apply|revert|list|status} [fixture]" >&2; exit 1 ;;
esac
