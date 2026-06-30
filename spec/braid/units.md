# Braid — issue-ready unit plan (U0–U15)

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
| U11 JS→Braid elaborator (thin slice) | U6 U10 | T13 (closes D-ELAB.1) |
| U12 JS vocabulary at scale | U11 | T2 T13 (closes D-VOCAB) |
| U13 multi-capsule project + toolchain | U6 | T13 (closes D-TOOLCHAIN) |
| U14 first live consumer collapse | U1 (cross-repo) | T15 (closes D-CONSUMER) |
| U15 Lean⇄verifier conformance | U3 U4 U5 | T2 T11 (closes D-SEMANTICS) |

> **U0–U10 = v0 (shipped).** U11–U15 are the **post-v0 frontier**: the
> Java-ecosystem gap catalogued in `DEBT_REGISTER.md`, chunked into issue-ready
> units. Sequencing still obeys the lowest-satisfied-blocked-by rule; D-RUN
> (U7) stays blocked on the kernel WASM runtime and is *not* re-chunked here.
> The discipline is **thin vertical slices** — each unit is the smallest end-to-
> end increment that moves a debt, not a big-bang.

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

**Status (2026-06-14, branch `u8-demo-port-reference`)** — the
**author→admit→render slice landed**. Three demo-port actions (modeled on the
kernel `blueprints/afternow-port/` surface): `edit-home-hero`,
`publish-services`, `render-work-listing` — authored via the JSON-of-IR→SDK
path, admitted, and rendered; `crates/braid-cli/tests/demo_port.rs` pins CIDs
and asserts verdicts, and `scripts/demo-port.sh` regenerates the committed
evidence bundle at `spec/braid/vectors/demo-port/`. Scenario #1 (admit, no egress/irreversible)
and the author-time fail-closed refusal of the no-confirm publish are covered;
scenario #2's verify-stage effect reject stays covered by
`braid-verify/tests/acceptance.rs`; #11 by the U6 widening gate. **Deferred to
U7** (kernel WASM runtime): the execution leg — on-tape fact journaling, real
execution, and scenario #3's *runtime* confirmation-hash-mismatch reject. Seam:
capsule CID → kernel runtime load → manifest re-derivation (T4) + fact journal.

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

---

# Post-v0 frontier (U11–U15) — closing the Java-ecosystem gap

Each unit names the `DEBT_REGISTER.md` debt it moves. The bar is unchanged: no
substrate/verifier rewrite (the v0 floor is locked), thin vertical slices,
evidence on the issue.

## U11 — JS→Braid elaborator: the first real frontend *(thin slice — LANDED this session)*
**Closes**: the first increment of **D-ELAB**. Makes "renders JS useless" (D31)
*operational*, not just architectural — JS *text* compiles into the verified
substrate via the one `braid-verify`, with zero AI in the path.
**Scope**: a `braid-elaborate-js` **consumer** crate (frontend, not trust-base;
outside the D3 boundary the substrate crates are fenced to). A JS *expression*
grammar — string literals, integer-number literals, the binary `+` (left-assoc,
parenthesizable) — lexed + parsed (hand-written recursive descent, no external
parser dep) and elaborated through `braid_sdk::Builder` over
`braid_vocab_js::registry_v0()`: `+` → `js.concat` for two strings, `js.add` for
two numbers. Library API (`elaborate_js`, `elaborate_and_admit`) + a
`braid-elaborate-js` binary that prints CID + verdict + manifest (SDK/CLI parity,
T13). **Explicitly out of scope (→ U12):** identifiers, calls, statements, the
`js.eval`/`js.fetch` escalation probes, and literal *values* (the seed
`js.lit.*` terms are valueless — IR/CID is a function of structure, not content).
**AC**: a JS expression elaborates to a capsule the verifier **Admits**; a mixed
`string + number` is **rejected at elaboration** (fail-closed, no coercion, no
malformed capsule reaches the verifier); a pinned-CID test fixes the
human-reconstructable-loop guarantee (same source ⇒ same CID); left-assoc and
parenthesized grouping are structurally distinct; malformed sources fail closed.
**Verification**: `cargo test -p braid-elaborate-js` (7 tests, all green);
`cargo run -p braid-elaborate-js -- '"hello" + "world"'` prints ADMIT;
`boundary_conformance.rs` still green (the consumer crate respects the boundary).

## U12 — JS vocabulary expansion (pure operators) *(LANDED this session)*
**Closes**: the first increment of **D-VOCAB** (8-term seed → a usable
expression language).
**Scope (revised — see the note below on literal payloads)**: grow
`braid-vocab-js` v1→**v2** with eight new **pure** terms — `js.sub`, `js.mul`,
`js.lt`, `js.eq.num`, `js.eq.str`, `js.and`, `js.or`, `js.not` — and extend the
`braid-elaborate-js` frontend to a full operator-precedence expression language
(`+ - * < == && || !`, boolean literals, parentheses; left-assoc; type-directed
overload resolution to distinct typed terms). A **vocabulary-extension
governance note** (module doc in `braid-vocab-js`): bump-and-re-pin, pure-by-
default, repurpose > extend > mint.
**Hardening (the three dredging classes, mutation-proven)**:
- *Composition/aggregation exfil (T1/T5)*: `dangerous_terms()` + the
  `expansion_added_no_escape_hatch` guard pin the authority surface to exactly
  `{js.dom.querySelector, js.eval, js.fetch}` — a capability cannot ride in on a
  "math" term; and `no_composition_yields_authority` proves no expression, however
  composed, yields a capsule with grants. The probes stay *unspellable* from text.
- *Context/spec drift*: vocab version bumped to 2; the registry CID and the
  frontend capsule CIDs re-pinned in the same change (no silent CID move).
- *Test-hollowing/Goodhart (T14/T7)*: every operator test asserts real strand
  structure + wiring + an ADMIT verdict, and a per-operator type-mismatch reject.
**AC**: the expression language round-trips source → admit; the three guards are
green and trip under mutation. **Out of scope, deferred honestly**: literal
*values* and identifiers/`let` (see note).
**Verification**: `cargo test -p braid-vocab-js -p braid-elaborate-js`.

> **Literal payloads are a substrate unit, not a vocabulary one.** `Strand` is
> `{ term, inputs }` — it carries no operand/constant, so `js.lit.string`
> records *that* a string literal occurs, not its bytes (the CMS `lit.text` is
> the same). Carrying values needs a `braid-ir` change (Strand + canonical
> encoding + verifier + render + a KAT re-pin) — locked-substrate work that does
> not belong in a vocabulary expansion. Filed as its own future unit; **not**
> silently folded into U12.

## U13 — multi-capsule project model + `braid` toolchain
**Closes**: **D-TOOLCHAIN** (no build/test model for projects of many capsules).
**Scope**: a typed project manifest (a set of capsules + their intents/anchors),
`braid build` (elaborate + admit every capsule, fail-closed on any reject) and
`braid test` (a harness for capsules, distinct from tests *of* the substrate).
No package *registry* (PRD §49 non-goal) — local project only.
**AC**: a multi-capsule sample project builds and tests through one command;
one rejecting capsule fails the whole build deterministically with the stage.
**Verification**: integration test over a fixture project; CI job.

## U14 — first live consumer collapse *(cross-repo coordination)*
**Closes**: **D-CONSUMER** ("become a dependency" has zero live dependents).
**Scope**: spec + execute the seam that deletes the browser engine's parallel
`BraidTerm` enum onto `braid-ir`/`braid-capability`/`braid-vocab-web` (the
`docs/BRAID_STEER.md` collapse), and live-wire the kernel's
`braid_vocab_binding.rs` snapshot to decode `braid_vocab_cms::registry_v0()`.
Braid ships only the substrate + the steer; the consumer owns its vocabulary.
**AC**: one real consumer compiles against the published Braid crates with its
parallel IR deleted; the kernel binding decodes the live registry with its
snapshot assertions still green (the dotted names are preserved verbatim).
**Verification**: the consumer repo's CI; the kernel binding test.

## U15 — Lean⇄verifier conformance check
**Closes**: **D-SEMANTICS** (the 8 verifier stages are not machine-checked
against the Lean predicates D22 names as the proof oracle).
**Scope**: a conformance harness mapping each `braid-verify` stage to its Lean
predicate (`excellent_not_hallucinated` skeleton), failing CI if a stage's
admit/reject behavior diverges from the proven rule. Part of the `U-SA` Tier-2
agenda; no new runtime, no AI in the verdict path (D32).
**AC**: every stage has a named Lean predicate it conforms to; a seeded stage
regression (mutation) trips the conformance gate red.
**Verification**: the conformance job; mutation evidence.
