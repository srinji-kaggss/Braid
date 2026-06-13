# Braid — issue-ready unit plan (U0–U10)

**Rule**: no issue, no work — each unit becomes a GitHub issue before
implementation, citing this file. Sequence rule: lowest unit whose blocked-by
are satisfied; the Braid track runs parallel to (never ahead of) the A-series
queue. Every unit ships with evidence per the build-state-ledger discipline
(command + SHA + output + independent re-run); mutation-verification is
expected wherever a check has teeth.

| Unit | Blocked by | Closes threats |
|---|---|---|
| U0 doc ratification | — | T16 |
| U1 `braid-ir` | U0 | T1 T3 T8 T15 |
| U2 `braid-render` manifests | U1 | T4 T12(partial) |
| U3 `braid-verify` stages 1–5 | U1 | T1 T2 T6 T11 |
| U4 effect + path-taint stages | U3 | T5 T10 |
| U5 bounds/budget stage | U3 | T7 |
| U6 `braid-cli` + CI widening gate | U2 U4 | T12 T13 |
| U7 WASM codegen + admission | U4 U5 + kernel WASM epic | T4 T9 |
| U8 Day-0 CMS reference | U6 (U7 for execution leg) | T14 |
| U9 adversarial hacker pass | U6 (re-run after U8) | all |
| U10 Rust SDK polish | U3 | T13 |

---

## U0 — Ratify ADR-088 + spec set
**Scope**: this PR. Director reviews D5's INTERPRETED entry (veto window),
merges, files U1–U6 issues from this file.
**AC**: ADR-088 status ACCEPTED; §16 status line corrected (#555); #556
commented with the D4 fresh-start resolution.
**Verification**: PR merged; issues exist; `node scripts/validate-docs.mjs`
passes if applicable.

## U1 — `braid-ir`: types, canonical encoding, CID, KATs
**Scope**: new kernel-workspace crate. Term/strand/braid/capsule types (PRD
§4); canonical CBOR-subset encode/decode with **bijection guard**; BLAKE3 CID
under `lw.braid.capsule.v0` (+ per-type domains); KAT vector file
(`spec/braid/vectors/` checked in, consumed by tests); fixed-point numeric
types only.
**AC**:
- KAT: known capsule bytes → pinned CID (the `bare_fact_content_hash_known_answer` pattern).
- Bijection: fuzz `decode(bytes) → re-encode == bytes ∨ reject` (proptest), plus hand-built malleability cases (reordered keys, indefinite lengths, junk padding — the A4.8 exploit set).
- Type universe rejects floats and interpretable-code types at construction.
- **Boundary conformance test**: `braid-*` crates import only the declared kernel contracts (lexer over `use` statements, `test_module_boundary_contract.rs` pattern).
**Verification**: `cargo test -p braid-ir`; mutation ×2 (disable bijection guard ⇒ malleability test RED; flip a domain string ⇒ KAT RED).

## U2 — `braid-render`: manifest model + deterministic renderer
**Scope**: manifest schema (PRD §4.4); `render(capsule) → Manifest` bound to
capsule CID; `manifest_diff(old,new) → Widenings | Narrowings | Neutral`;
golden manifest fixtures; **braid-graph export** (deterministic DOT/JSON of
the strand DAG — the D17 "translation/graph stuff": the IR→human direction is
both the manifest AND the visualizable graph).
**AC**: same capsule ⇒ byte-identical manifest across platforms; manifest
embeds capsule CID; diff classifies a capability addition as Widening; golden
fixtures pinned.
**Verification**: `cargo test -p braid-render`; mutation (drop CID binding ⇒
binding test RED).

## U3 — `braid-verify` stages: canonical-form, vocabulary/version, type, capability
**Scope**: verifier crate, independent of `braid-ir`'s encoder (decode-only +
own canonical re-encoder for the bijection check — D9). Stages: canonical-form
→ vocabulary membership + `VOCABULARY_VERSION` pin → type check → capability
attenuation (against the ADR-073 lattice semantics via the declared kernel
contract). Typed, machine-readable verdicts (`Reject { stage, reason }`).
**AC**: acceptance scenarios #4, #6, #7, #8, #13, #14 green; dep-allowlist
conformance test (T11); KAT parity with `braid-ir` as a build gate.
**Verification**: `cargo test -p braid-verify`; mutation (skip version pin ⇒
#7 RED).

## U4 — Effect calculus + path-level taint + confirm policy
**Scope**: effect-composition stage (postures from the registry, mirroring
`vocabulary.rs` `CompositionPosture`); **path-level** monotone exposure fold
(never per-hop only — T5); confirm-policy validation (payload-hash-bound,
one-shot shape — T10).
**AC**: scenarios #2, #3 (static half), #5 green; the laundering trip-wire
test exists and is mutation-proven (disable the path fold ⇒ trip-wire RED).
**Verification**: `cargo test -p braid-verify`; mutation evidence in PR.

## U5 — Bounds/budget stage + enforcement contract
**Scope**: capsule-level budget composition from strand cost bounds; the
*enforcement mapping spec* (declared budget → WASM fuel/page limits — OQ1
resolved or explicitly deferred with the enforcement seam typed); deterministic
exhaustion verdict shape.
**AC**: scenario #9's static half (budget composition + verdict); a bound
without an enforcement mapping is a verifier REJECT, not a warning (anti-T7).
**Verification**: `cargo test -p braid-verify`.

## U6 — `braid-cli` + CI manifest-widening gate
**Scope**: `braid encode|decode|verify|render|diff`; CI job rendering manifest
diffs on PRs touching capsules/vectors, flagging Widenings (observe-only
first, then required); human-authoring doc (hand-write a capsule with no AI).
**AC**: scenario #12 (CLI-only loop) runs in CI; a seeded widening PR is
flagged (red-team evidence — T12); docs sufficient for a specialist (reviewed
by the Director or delegate).
**Verification**: CI run links; the red-team PR.

## U7 — WASM codegen + runtime admission *(coordination unit)*
**Scope**: deterministic codegen capsule → WASM component whose import surface
is exactly the three-syscall contract; load-time re-verification + manifest
re-derivation (T4, T9). **Blocked by the kernel Day-0 WASM runtime epic — this
unit coordinates with it; it must not build a second runtime.**
**AC**: scenarios #3 (runtime half), #9 (runtime half), #10 green on the
kernel runtime; import-surface conformance (no host function outside the
syscall contract).
**Verification**: integration tests on the kernel runtime; import-surface
lexer test.

## U8 — Day-0 CMS reference workflow (= the landing-surface port, D16)
**Scope**: ≥3 real `blueprints/afternow-port/` CMS actions — afternow-port IS
the landing-port blueprint, so this is the Director's "landing page as first
full port" (D16) — e.g. edit section [reversible], publish page [irreversible
+ confirm], render listing [projection read]; authored as capsules via the
SDK, admitted, executed with journaled evidence. Render output is typed
`ViewDirective`/`MotionDirective` terms, never DOM (D16). Frontend-first =
the v0 vocabulary is pure render + projection reads; the irreversible publish
strand is the ONLY effectful term in v0 (the escalation probe). No mocks on
the kernel path (T14).
**AC**: scenarios #1, #2, #3, #11 demonstrated on the real actions; evidence
(facts on tape, journal entries, manifests) attached to the issue; PRD §8
success metrics measured.
**Verification**: end-to-end test + evidence bundle; independent re-run.

## U9 — Adversarial hacker pass *(blocking gate for "v0 done")*
**Scope**: independent adversarial review of U1–U8 against `threat-model.md` —
encoding malleability, parse differentials, laundering compositions, manifest
spoofing (R3), hollow tests, confirmation replay. FIX-THEN-SHIP protocol per
repo doctrine.
**AC**: written verdict per threat (exploitable / not, at `file:line`); all
confirmed-real findings closed and mutation-verified; verdict recorded in the
build-state ledger pattern.
**Verification**: the hacker report + closure evidence.

## U10 — Rust SDK polish ("rust day 1")
**Scope**: ergonomic typed builder over `braid-ir` (compile-time term
signatures where feasible); examples for each capsule pattern; doc parity with
the CLI path so the SDK never becomes the only path (T13).
**AC**: the U8 reference capsules re-authored via the SDK with identical CIDs;
`compile_fail` doctests for illegal compositions the type system can catch
statically.
**Verification**: `cargo test -p braid-sdk` + doctests.
