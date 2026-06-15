# Braid

> ⚠️ **Name provisional (ADR-088 D15).** "Braid" is a working codename until the
> Director finalizes naming. Identity is the spec + decision register, never the
> word.
>
> ⚠️ **"Machine-first" is legacy framing (superseded by D20).** The thesis is
> *predictable-surface + confinement + amortized human judgment* — not stripping
> human affordances. The verifier owns a correctness/safety **floor**, not "good
> code." The phrase persists in the ADR filename/title for provenance only; the
> decision register (D20, D28–D30) is authoritative.

**Braid** is Logic OS's application framework for the era where **AI authors and
humans design**. Programs are content-addressed graphs of typed terms drawn from a
closed capability vocabulary. A deterministic compiler/verifier — not an AI, not a
reviewer — owns a **floor** of correctness and confinement-safety (types, effects,
capability attenuation, taint, bounds); **humans own the architecture and intent**
above that floor and audit rendered manifests. AI and humans never share a
representation — they meet at a shared verified **anchor** through separate
projections (register D20, D28–D30).

This repository was extracted from
[`logic-os-kernel`](https://github.com/srinji-kaggss/logic-os-kernel) (PR #564)
on 2026-06-13, executing the ADR-088 D5 extraction covenant. See the
**extraction addendum** at the top of
[`docs/adr-088-…`](docs/adr-088-braid-machine-first-framework-foundations.md)
for exactly what changed in the move.

## Layout

| Path | What |
|------|------|
| `crates/braid-ir` | Typed term-graph IR, canonical CBOR-subset encoding, BLAKE3 CIDs, bijection guard, KAT vectors (U1 #558). |
| `crates/braid-verify` | Independent strict decoder + fail-closed admission pipeline; zero shared serialization code with `braid-ir` (D9 anti-trusting-trust) (U3–U5 #560). |
| `crates/braid-render` | CID-bound manifest, deterministic text rendering, widening/narrowing diff, DOT graph export (U2 #559). |
| `crates/braid-sdk` | Typed authoring builder over `braid-ir`; reproduces reference CIDs byte-for-byte (U10). |
| `crates/braid-cli` | The `braid` binary: `encode`/`decode`/`verify`/`render`/`diff` — the human-reconstructable loop (no AI, no Rust). `encode` reads JSON-of-IR (D19) through the SDK (U6 #2). |
| `crates/braid-capability` | **Vendored** kernel capability contract — a verbatim mirror of `canvas-protocol::Capability` on the kernel `origin/main` (the single type that crosses the ADR-088 D3 boundary). |
| `spec/braid/` | PRD, decision register (`DECISIONS.md`), threat model, unit plan, KAT vectors. **Start here: `spec/braid/README.md`.** |
| `docs/` | ADR-088 (ratified doctrine + locked invariants); `authoring-cli.md` (hand-author a capsule). |

## Build & test

```bash
cargo check --workspace
cargo test --workspace      # 79 tests
cargo clippy --workspace --all-targets
./scripts/cli-loop.sh       # scenario #12 end-to-end (also a CI job)
```

## Boundary

Braid depends only on the declared kernel contract — the `Capability` enum,
vendored as `braid-capability`. `crates/braid-ir/tests/boundary_conformance.rs`
machine-enforces this: the build fails if a `braid-*` crate grows a dependency
or `use` outside the allowlist. To re-sync the capability contract after a kernel
change, diff `crates/braid-capability/src/lib.rs` against
`canvas-protocol::Capability` on the kernel's `origin/main` and re-vendor verbatim.
