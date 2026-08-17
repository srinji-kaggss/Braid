#!/bin/sh
# Seed the default braid store with the W5 fixture manifests + the declared
# inventory, then print the org map.
#
# These are STAND-INS for real per-repo data entry: correct a repo's fields
# with `braid store put <repo-manifest.json> --replace`, and edit the
# inventory when the declared org set changes.
#
# Run: sh spec/braid/vectors/w5/seed.sh
set -e
DIR=$(dirname "$0")
STORE="$HOME/.local/share/braid/store"
for f in "$DIR"/manifests/*.json; do
  braid store put "$f"
done
cp "$DIR"/inventory.json "$STORE/inventory.json"
braid catalog
