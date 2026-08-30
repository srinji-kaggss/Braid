#!/usr/bin/env bash

set -euo pipefail

SOURCE="${1:-}"
REVISION="${2:-}"
if [[ ! "$SOURCE" =~ ^(https://[^?#\|[:space:]]+|file:///[^?#\|[:space:]]+)$ ]]; then
  echo "usage: $0 <https-or-file-git-url> <40-hex-commit>" >&2
  exit 2
fi
if [[ ! "$REVISION" =~ ^[0-9a-f]{40}$ ]]; then
  echo "release probe: revision must be exactly 40 lowercase hex characters" >&2
  exit 2
fi

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FIXTURE="$REPO_ROOT/release/consumer-probe"
PROBE_DIR="$(mktemp -d "${TMPDIR:-/tmp}/braid-release-probe.XXXXXX")"
trap 'rm -rf "$PROBE_DIR"' EXIT
mkdir -p "$PROBE_DIR/src"

sed \
  -e "s|__BRAID_SOURCE__|$SOURCE|g" \
  -e "s|__BRAID_REV__|$REVISION|g" \
  "$FIXTURE/Cargo.toml.in" > "$PROBE_DIR/Cargo.toml"
sed \
  -e "s|__BRAID_SOURCE__|$SOURCE|g" \
  -e "s|__BRAID_REV__|$REVISION|g" \
  "$FIXTURE/Cargo.lock.in" > "$PROBE_DIR/Cargo.lock"
cp "$FIXTURE/src/main.rs" "$PROBE_DIR/src/main.rs"

if grep -Eq 'path[[:space:]]*=' "$PROBE_DIR/Cargo.toml"; then
  echo "release probe: consumer manifest contains a path dependency" >&2
  exit 1
fi
if grep -Rq '__BRAID_' "$PROBE_DIR"; then
  echo "release probe: unresolved template token" >&2
  exit 1
fi

CARGO_TARGET_DIR="$PROBE_DIR/target" \
  cargo run --locked --manifest-path "$PROBE_DIR/Cargo.toml"

if command -v sha256sum >/dev/null 2>&1; then
  LOCK_SHA256="$(sha256sum "$PROBE_DIR/Cargo.lock" | awk '{print $1}')"
else
  LOCK_SHA256="$(shasum -a 256 "$PROBE_DIR/Cargo.lock" | awk '{print $1}')"
fi

printf 'source=%s\nrevision=%s\nlock_sha256=%s\n' \
  "$SOURCE" "$REVISION" "$LOCK_SHA256"
