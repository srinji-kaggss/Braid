# Braid contract release

`contract-v0.toml` is the machine-readable release boundary. The contract is
the smallest useful cross-repository surface: canonical identity and IR,
independent verification, and the CMS/web vocabulary packages. Flow, material
effects, and DSL crates remain buildable workspace members but are not promoted
as stable consumer APIs while issue #63 is unresolved.

## Reproduce the consumer proof

Run the probe against an immutable 40-character Git commit:

```bash
./scripts/braid-release-probe.sh \
  https://github.com/srinji-kaggss/Braid.git \
  <source-commit>
```

The probe creates an empty temporary Cargo project, instantiates a committed
lockfile, and builds with `--locked`. Its manifest contains exact Braid crate
versions and one Git source/revision; it contains no path dependencies. The
program regenerates the CMS registry, checks the pinned registry and capsule
CIDs, and admits the reference capsule through `braid-verify`. Registry crates,
transitive versions, registry checksums, and the Git source are fixed by
`consumer-probe/Cargo.lock.in`; the probe prints that resolved lockfile's
SHA-256.

The independent registry-export gate is:

```bash
./scripts/braid-registry-export-check.sh
```

It regenerates the canonical CMS v1 bytes twice, proves byte-for-byte
determinism, and checks their BLAKE3 domain-separated CID through the existing
known-answer tests. The exporter writes only to an explicit caller-selected
temporary directory in the gate; it does not update source fixtures.

## Promotion and rollback

Promotion is two-phase. First merge an ordinary PR and let main CI prove the
exact merge commit. Then run the clean consumer probe against the GitHub URL and
that merge commit. Only that commit may receive the signed annotated tag named
in `contract-v0.toml`. A tag is never moved or reused.

A failed probe creates no advertised release. If a defect is found after tag
promotion, mark the GitHub release withdrawn, preserve its evidence, fix on a
new PR, increment the contract version, and cut a new tag. Consumers remain
pinned to the prior immutable commit until they opt into the replacement.

Publishing these crates to crates.io is a later distribution channel, not a
different contract. It must preserve the same crate set, exact versions, MSRV,
known-answer CIDs, and dependency graph.
