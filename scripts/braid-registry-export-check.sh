#!/usr/bin/env bash

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROBE_DIR="$(mktemp -d "${TMPDIR:-/tmp}/braid-registry-export.XXXXXX")"
trap 'rm -rf "$PROBE_DIR"' EXIT

cd "$REPO_ROOT"
cargo run --quiet --locked -p braid-vocab-cms --bin braid-registry-export -- raw \
  > "$PROBE_DIR/registry-a.cbor"
cargo run --quiet --locked -p braid-vocab-cms --bin braid-registry-export -- raw \
  > "$PROBE_DIR/registry-b.cbor"
cmp "$PROBE_DIR/registry-a.cbor" "$PROBE_DIR/registry-b.cbor"
cargo test --quiet --locked -p braid-ir --test kat registry_v0_cid_known_answer

if command -v sha256sum >/dev/null 2>&1; then
  EXPORT_SHA256="$(sha256sum "$PROBE_DIR/registry-a.cbor" | awk '{print $1}')"
else
  EXPORT_SHA256="$(shasum -a 256 "$PROBE_DIR/registry-a.cbor" | awk '{print $1}')"
fi

printf 'registry_bytes=%s\nregistry_export_sha256=%s\n' \
  "$(wc -c < "$PROBE_DIR/registry-a.cbor" | tr -d ' ')" "$EXPORT_SHA256"
