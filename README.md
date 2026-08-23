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
| `crates/braid-ir` | The **substrate**: typed term-graph IR, canonical CBOR-subset encoding, BLAKE3 CIDs, bijection guard, closed `TermRegistry` shape, capsule artifact. No domain vocabulary lives here (D31). |
| `crates/braid-capability` | The capability token newtype — a content-addressed string (`Capability::new("js.eval")`). Each vocabulary owns its capability space; the verifier's attenuation check works on any token set (D31). |
| `crates/braid-vocab-cms` | **Vocabulary package** — the kernel/landing-port CMS term registry + the 10 kernel capability verbs as named consts. The first vocabulary (D7/D16). |
| `crates/braid-vocab-js` | **Vocabulary package** — the JavaScript elaboration target. The second vocabulary, proving the global-IR claim (D31): JS capsules admit via the one `braid-verify` with a `js.*` capability space. |
| `crates/braid-verify` | Independent strict decoder + fail-closed admission pipeline; zero shared serialization code with `braid-ir` (D9). Registry-parametric — admits against any vocabulary's `TermRegistry`. |
| `crates/braid-render` | CID-bound manifest, deterministic text rendering, widening/narrowing diff, DOT graph export. |
| `crates/braid-sdk` | Typed authoring builder over `braid-ir`; takes any vocabulary's registry. |
| `crates/braid-cli` | The `braid` binary: `encode`/`decode`/`verify`/`render`/`diff` — the human-reconstructable loop (no AI, no Rust). Pins the `braid-vocab-cms` registry for the CMS reference workflow. |
| `crates/braid-elaborate-js` | **Frontend** (U11–U12): an operator-precedence JS expression language (`+ - * < == && \|\| !`, literals, booleans, parens) that elaborates JS *text* into an admitted capsule via the one `braid-verify`. The first real frontend over the global IR — "renders JS useless" made operational (D31). |
| `crates/braid-project` | **Toolchain** (U13): a multi-capsule project manifest + `braid-project build` — elaborate + admit every capsule fail-closed, emit a deterministic project CID. The first step toward a `braid build` for projects (D-TOOLCHAIN). |
| `spec/braid/` | PRD, decision register (`DECISIONS.md`), threat model, unit plan, KAT vectors. **Start here: `spec/braid/README.md`.** |
| `docs/` | ADR-088 (ratified doctrine + locked invariants); `authoring-cli.md` (hand-author a capsule); `CRATE-OWNERSHIP.md` (crate invariant and boundary map). |

## Build & test

```bash
cargo check --workspace
cargo test --workspace      # 135 tests
cargo clippy --workspace --all-targets
./scripts/cli-loop.sh       # scenario #12 end-to-end (also a CI job)
```

## Boundary

Braid depends only on the declared kernel contract — the `Capability` token's
dotted names (vendored as named consts in `braid-vocab-cms`). `crates/braid-ir/tests/boundary_conformance.rs`
machine-enforces this: the build fails if a `braid-*` substrate crate grows a dependency
or `use` outside the allowlist. Vocabulary packages (`braid-vocab-cms`, `braid-vocab-js`)
depend on the substrate + `braid-capability`; they are consumer-side, not trust-base.
