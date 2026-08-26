# Braid

**Braid** is an application framework where AI authors code and humans own the
design. Programs are content-addressed graphs of typed terms drawn from a closed
capability vocabulary. A deterministic compiler/verifier owns the correctness and
confinement-safety floor — types, effects, capability attenuation, taint, bounds.
Humans own the architecture and intent above that floor.

AI and humans never share a representation. They meet at a shared verified
**anchor** through separate projections — the AI sees an IR it can produce
reliably; the human sees a rendered manifest they can audit.

## Published crates

| Crate | Version | What |
|-------|---------|------|
| [`lgwks_std`](https://crates.io/crates/lgwks_std) | 0.5.0 | Zero-config primitives that replace a dozen crates — hex, base64, timestamps, UUIDs, hashing, glob, regex, JSON, async. Zero external deps by default. |
| [`lgwks_bot`](https://crates.io/crates/lgwks_bot) | 0.1.0 | Capability-gated automation bots built on four verbs: Observe, Evaluate, Execute, Query. |

## Workspace

| Path | What |
|------|------|
| `crates/braid-ir` | Typed term-graph IR, canonical CBOR-subset encoding, BLAKE3 CIDs, bijection guard, capsule artifacts. |
| `crates/braid-flow-ir` | Canonical inter-capsule Flow IR, strict byte bijection, bounded identity, and justified-invocation declarations. |
| `crates/braid-flow-verify` | Independent strict decoder + fail-closed admission for Flow graphs. |
| `crates/braid-capability` | Capability token newtype — content-addressed dotted names. Vocabulary-agnostic attenuation. |
| `crates/braid-verify` | Independent strict decoder + fail-closed admission pipeline. Registry-parametric. |
| `crates/braid-render` | CID-bound manifest, deterministic text rendering, widening/narrowing diff, DOT export. |
| `crates/braid-sdk` | Typed authoring builder over `braid-ir`. |
| `crates/braid-cli` | The `braid` binary: encode, decode, verify, render, diff, catalog, store. |
| `crates/braid-manifest` | Repository-manifest sibling artifact (closed dimensions, canonical bytes, CID). |
| `crates/braid-run` | Deterministic DAG evaluation + capability-gated effect dispatch. |
| `crates/braid-governance` | Signed Keel change envelopes, budgets, allowlists, commitments, expiry. |
| `crates/braid-runtime` | Executable startup contract (validated args + one startup-failure path). |
| `crates/braid-elaborate-js` | Operator-precedence JS expression frontend that elaborates text into admitted capsules. |
| `crates/braid-project` | Multi-capsule project manifest and deterministic `braid-project build`. |
| `crates/braid-integrate` | Repo-graph advisor — proposes `lgwks_std` / `lgwks_bot` seams (`braid-integrate --json`). |
| `crates/braid-vocab-cms` | CMS vocabulary — the kernel term registry and capability verbs. |
| `crates/braid-vocab-js` | JavaScript vocabulary — JS capsules admitted via `braid-verify` with `js.*` capabilities. |
| `crates/braid-vocab-rust` | Rust vocabulary. |
| `crates/braid-vocab-web` | Web vocabulary. |
| `crates/lgwks-std` | Published: `lgwks_std` on crates.io. |
| `crates/lgwks-bot` | Published: `lgwks_bot` on crates.io. |
| `crates/lgwks-std-gate` | Build-time proof that dependencies match the human-approved contract. |
| `.claude/skills/braid-agent` | AI-agent skill — how an agent uses Braid's tools (see `docs/agent-guide.md`). |
| `spec/braid/` | PRD, decision register, threat model, KAT vectors. |

## Build & test

```bash
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets
```

## Boundary

The substrate crates (`braid-ir`, `braid-verify`, `braid-capability`) have a
machine-enforced dependency boundary — the build fails if an unapproved
dependency or import appears.
`crates/braid-ir/tests/boundary_conformance.rs` is the gate.

## License

BSD-3-Clause — Copyright 2026 Logical Works Incorporated
