#!/usr/bin/env bash
# ── older-toolchain-test.sh — run the workspace suite under an explicit
# older toolchain. Owns MSRV verification isolation.
#
# INV-MSRV-ISOLATION — a run under an old toolchain must not mutate package
# manifests, rust-toolchain.toml, or any artifact of the default toolchain:
# builds land in a dedicated per-toolchain CARGO_TARGET_DIR.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [ $# -lt 1 ]; then
  echo "usage: $0 <toolchain> [extra cargo args…]" >&2
  echo "example: $0 1.93" >&2
  exit 64
fi

TOOLCHAIN="$1"
shift

if ! command -v rustup >/dev/null 2>&1; then
  echo "rustup is required to resolve toolchain '$TOOLCHAIN'" >&2
  exit 69
fi

TARGET_DIR="$ROOT/target/toolchain-${TOOLCHAIN//./-}"
export CARGO_TARGET_DIR="$TARGET_DIR"

cd "$ROOT"
exec rustup run "$TOOLCHAIN" cargo test --workspace "$@"
