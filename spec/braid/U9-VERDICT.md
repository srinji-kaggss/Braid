# U9 — Adversarial hacker pass verdict

**Unit**: U9 (`spec/braid/units.md`) — the blocking gate before any "v0 done"
claim (D13). **Scope**: independent adversarial review of U1–U8 against
`threat-model.md`. **AC**: a written verdict per threat (exploitable / not,
at `file:line`); all confirmed-real findings closed and mutation-verified;
verdict recorded in the build-state ledger pattern.

**Method**: read every crate's source and tests; for each threat, locate the
mitigation in code, probe it live (encode/verify/render against the real
`braid` binary), and attempt the bypass the threat predicts. Findings that
reproduce are FIXED-THEN-SHIPPED with a mutation-red regression; threats whose
mitigation holds on the probe are verdicted *not exploitable* with the
pinning evidence.

**Status**: 4 findings closed this pass (T3 sub-map [prior], T4 review-path
[prior], T12 neutral-collapse [prior], **R3 line-injection [new this pass]**);
all other threats verdicted not-exploitable at the cited pin. Full workspace
green: `cargo test --workspace` (92 tests), `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo fmt --all --check`,
`./scripts/cli-loop.sh`.

---

## The register — T1–T16 + R1–R3

| Threat | Verdict | Pinning evidence (file:line) |
|---|---|---|
| **T1** vocabulary escape hatches | **not exploitable** | `TypeTag` enum has no interpretable-code variant (`crates/braid-ir/src/term.rs:16-29`); registry insertion enforces pure⇔no-capability (`term.rs:100-117`); `lit.text`/`bytes.id` carry opaque data, no term interprets its input as code. Outbound destinations are kernel-typed `EgressDestination` (deferred to U7 runtime). Verified: `eval` term rejected (`acceptance.rs:142`, `scenario_14`). |
| **T2** verifier/generator collusion | **not exploitable** | `braid-verify/src/decode.rs` restates the CBOR codec independently of `braid-ir/src/canon.rs` (D9); parity suite `braid-verify/tests/parity.rs` holds both decoders byte-equal on all examples + the pinned KAT; malleability set rejected by both (`parity.rs:53`). |
| **T3** canonical-encoding malleability | **closed (was High)** | bijection guard `canon.rs:281-290`; sub-map smuggling closed by `Value::require_only_keys` at every map level (`capsule.rs:177`, `braid.rs:90,97`, `term.rs:151,281`); regressions `malleability.rs:178-232` + `acceptance.rs:226`. Accepted-bytes round-trip identity `malleability.rs:238`. |
| **T4** manifest ≠ behavior gap | **closed (Medium)** | static half: manifest binds capsule CID (`render/src/lib.rs:75`, `render/tests/render.rs:9`); review-path fail-closed: `render`/`diff` call `verify` first (`cli/src/main.rs:316,331-332`, `require_admit_for_review` `main.rs:400`); regressions `cli_loop.rs:190,215`. Runtime re-derivation (T4 runtime half) deferred to U7. |
| **T5** capability/effect laundering | **not exploitable** | path-level monotone fold `verify/src/lib.rs:154-173`; trip-wire `acceptance.rs:84` (vault→pure→pure→egress REJECTs at Taint); the fold uses `incoming.max(exposure[input_idx])` so taint carries through pure hops. |
| **T6** version skew | **not exploitable** | `verify/src/lib.rs:59-67` pins ir_version, vocab_version, AND registry_cid (content-addressed, not just a number); `acceptance.rs:104` (vocab bump + zeroed registry_cid both REJECT at VersionPin). |
| **T7** narrated-not-enforced bounds | **not exploitable (static half)** | `verify/src/lib.rs:175-189` checked-sum (overflow ⇒ Reject, not wrap) + budget comparison; `acceptance.rs:132` (budget 3 < cost 12 ⇒ Bounds Reject). Runtime enforcement mapping deferred to U7 (OQ1 seam named in `units.md` U5). |
| **T8** float/nondeterminism poisoning | **not exploitable** | no `Value::Float` variant exists (`value.rs:13-24`); float heads rejected at canonical form (`canon.rs:201-206`, `decode.rs:96-102`); `malleability.rs:24`; `acceptance.rs:121`. Internal ordering is `BTreeMap` (`canon.rs:246`). |
| **T9** replay/provenance gaps | **not exploitable (static half)** | content-addressed artifact (CID is the lookup key, `cid.rs`); runtime re-verify + one-shot confirm deferred to U7. No static-path replay vector exists in v0 (no confirmation token binding yet — that's the U7 seam). |
| **T10** human-confirmation spoofing | **not exploitable (static half)** | confirm policy is a typed enum, payload-hash binding is the U7 runtime half; static stage 6 refuses irreversible/egress without `HumanConfirm` (`verify/src/lib.rs:134-146`); SDK refuses at author time (`sdk/src/lib.rs:204-211`); `acceptance.rs:38`, `cli_loop.rs:268`. |
| **T11** toolchain/dependency compromise | **not exploitable** | dep-allowlist conformance test `boundary_conformance.rs:62` + use-statement scan `boundary_conformance.rs:96`; verifier deps = `blake3`, `braid-ir`, `braid-capability` only (`allowed_deps` `boundary_conformance.rs:20`). No new deps without Director approval (CLAUDE.md directive 4). |
| **T12** authority creep through review fatigue | **closed (Medium)** | mechanical widening classifier `render/src/lib.rs:193-286`; CI gate `has_widening` `render/src/lib.rs:289`; red-team `cli_loop.rs:117` (seeded widening ⇒ exit 1); neutral-collapse closed (`render/tests/render.rs:90`, `cli_loop.rs:166`). |
| **T13** AI-only path dependence | **not exploitable** | CLI loop is the reference path (`cli/src/main.rs`); scenario #12 runs in CI (`scripts/cli-loop.sh`, `cli_loop.rs:63`); `encode` reproduces pinned reference CID `cli_loop.rs:52`; authoring doc `docs/authoring-cli.md`. |
| **T14** abstract-acceptance slop | **not exploitable (static half)** | demo-port uses real registry terms, no mocks (`crates/braid-cli/tests/demo_port.rs`); execution leg deferred to U7/#6 (no mock runtime in v0 — the seam is named, not stubbed). |
| **T15** boundary erosion | **not exploitable** | `boundary_conformance.rs` machine-enforces dep + use allowlist; `braid-capability` is the single vendored crossing type; re-sync covenant in README. |
| **T16** scope stampede | **not exploitable (process)** | D6/D13 lock non-goals; `DECISIONS.md` gates grammar; no issue no work; this verdict stays in scope. |
| **R1** vocab churn | **accepted** | capsules pin versions (D11); churn is breaking-but-visible. |
| **R2** runtime dependence | **accepted** | P1–P2 valuable standalone; U7 blocked on kernel epic. |
| **R3** manifest renderer bugs | **closed (Medium, new this pass)** | `render_text` now escapes `\` / `\n` / `\r` / `\t` in all user-controlled string fields via `escape_field` (`render/src/lib.rs:93-113`); one logical field ⇒ one physical line. Regressions: `render/tests/render.rs` (4 tests) + `cli_loop.rs::render_escapes_newlines_so_manifest_cannot_be_spoofed`. See threat-model.md R3 for the exploit description. |

---

## The new finding — R3 manifest line-injection (reproduction + closure)

**Hypothesis**: the manifest is line-oriented `key: value` and is the human
review object (D12). `intent` and `evidence` are user-controlled strings
rendered verbatim. A `\n` in either would inject forged lines.

**Reproduced** against the real binary (2026-06-23, pre-fix):

```
$ braid encode spoof_intent.json -o spoof.braid   # intent contains "\ncapsule: <zeros>\ncapabilities: (none)\n"
$ braid verify spoof.braid                         # ADMIT  (intent content is advisory, not blocking — D30)
$ braid render spoof.braid
capsule: 561a2b8f...                                          # real binding
intent: edit section
capsule: 0000000000000000000000000000000000000000000000000000000000000000   # FORGED
capabilities: (none)                                          # FORGED
intent: benign                                               # FORGED
ir_version: 0
...
capabilities: signal.emit                                     # real authority, buried
```

A reviewer scanning the diff sees the forged `capsule: <zeros>` and
`capabilities: (none)` lines *before* the real ones. Same vector via
`evidence`.

**Root cause**: `render_text` (`braid-render/src/lib.rs`) emitted string
fields with `out.push_str(&v)` and no control-char escaping. The invariant
"one field → one line" was not enforced at the emission boundary.

**Fix**: `escape_field` escapes `\` (first, for unambiguity), `\n`, `\r`,
`\t`. Applied to `intent`, `capabilities`, `effects`, `evidence` in
`render_text`. Numeric/CID fields are hex-only and safe. The diff `detail`
strings were audited: capability/effect names are registry-defined enum
Display names (no newlines possible); intent detail is the static string
"changed"; so the diff path needs no change.

**Mutation check**: reverting `escape_field` to identity (or removing the
`\n` arm) makes `newline_in_intent_cannot_inject_manifest_lines` RED — the
forged `capsule:` line count rises to 2. Confirmed the regression has teeth.

**Why this is Medium not High**: the admission gate is unaffected (the
capsule genuinely admits — its real capabilities ARE declared and verified);
the exploit is against the *review object's integrity*, bounded by D12's
runtime re-derivation check (which catches a swapped artifact). Within v0's
static scope, the manifest IS the review object and CI gates on it, so
line-injection materially weakens the review gate — hence Medium, not Low.

---

## What U9 does NOT close (deferred, on record)

- **T7/T9/T10 runtime halves + T14 execution leg**: blocked on U7 (kernel
  Day-0 WASM runtime). The seams are named in `units.md` and issue #6; no
  static-path bypass exists in v0.
- **D25/D27 advisory judgments** (intent-coherence, grain, restraint-as-
  blocking): per D30 these are advisory/research in v0, NOT blocking. Empty
  intent admits (`/tmp/empty_intent.json` probe) — this is the documented
  deflation, not a finding. The advisory→blocking frontier is an explore-next
  agenda item (D30 A), not a v0 bug.
- **R1/R2**: accepted residual risks, unchanged.

## Verdict

**No confirmed-real bypass of the admission path remains open.** 4 findings
closed and mutation-verified (T3, T4, T12, R3). The v0 static verifier +
manifest + CLI loop hold against the threat catalogue. The remaining gaps are
runtime halves explicitly deferred behind U7, not static-path holes.