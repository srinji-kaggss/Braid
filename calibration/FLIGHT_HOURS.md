# Braid — Flight Hours Ledger

> **D-FLIGHT**: Braid's correctness claims are validated against PRE-VALIDATED
> EXTERNAL standards, not self-tests. "Flight hours" = real cross-checks against
> the world, accumulated over time. Each entry is a run: what was checked, what
> the external standard says, what Braid produced, the verdict, the date.
> This is the build-state-ledger discipline (D13) applied to calibration.

## Why this exists

A self-test only proves Braid agrees with itself. The canonical encoder (T3/T8)
and the CID hash (D8) make byte-level claims that must match the *published
standards* — RFC 8949 (IETF/cbor-wg) and BLAKE3 (BLAKE3-team). These are someone
else's validation; Braid consumes the verdict, it doesn't mint it.

This ledger tracks every such cross-check as a flight hour. A regression that
breaks agreement with the external standard is a RED the self-tests would miss.

## The pre-validated corpora (not ours)

| Corpus | Source | Validated by | Vectors | In Braid's universe |
|--------|--------|--------------|---------|---------------------|
| RFC 8949 Appendix A | `cbor/test-vectors` (cbor-wg) | IETF RFC 8949 §4.2 deterministic encoding | 82 | 35 roundtrip-true in-scope |
| BLAKE3 KAT | `BLAKE3-team/BLAKE3/test_vectors` | BLAKE3 team (2019-12-27 reference vectors) | 35 | 1 checked (input_len=0) |

Fetched via lgwks (`lgwks fetch`) with crwl as the proven fallback, per the
dogfood directive. Vendored at `calibration/vectors/`. Not regenerated from
Braid — consumed verbatim from the standard.

## Run log

| Date | Run | Checked | Verdict | Evidence |
|------|-----|---------|---------|----------|
| 2026-06-23 | RFC 8949 deterministic CBOR — Braid encoder byte-match | 31 vectors | ✅ PASS — all 31 byte-identical; 4 skipped (nested floats/bytes outside Braid's universe) | `crates/braid-ir/tests/calibration.rs::canonical_encoder_matches_rfc8949_deterministic_vectors` |
| 2026-06-23 | BLAKE3 KAT input_len=0 — blake3 crate matches reference | 1 vector | ✅ PASS — `af1349b9f5f9a1a6a0404dea36dcc949` | `crates/braid-ir/tests/calibration.rs::blake3_cid_matches_blake3_team_kat_zero_input` |
| 2026-07-15 | RFC 8949 §4.2.1 map key ordering — length-first canonical order | 2 vector sets (top-level + nested) | ✅ PASS — Braid's encoder produces RFC 8949 deterministic bytes for multi-length keys; decoder rejects bytewise (non-length-first) order | `crates/braid-ir/tests/calibration.rs::map_ordering_matches_rfc8949_length_first_deterministic`, `nested_map_ordering_matches_rfc8949_length_first` |
| 2026-07-15 | D-SA5 Lean⇄verifier stage conformance — Rust stage verdicts match Lean predicates | 1 corpus (8 reject classes + 1 admit) | ✅ PASS — 9/9 capsule verdicts agree; Lean axiom-free | `keel/src/adapters/lean.mjs` conformance check; corpus at `crates/braid-verify/tests/lean_conformance.rs` |

## What this proves (and does NOT prove)

**Proves:**
- Braid's canonical CBOR subset encoder is byte-compatible with RFC 8949
  deterministic encoding for every in-scope value type (int/text/map/array/bool).
  Two independent encoders agree on the canonical form → the bijection guard
  (T3) is validated against the standard, not just against itself.
- Braid's hash dependency (`blake3`) matches the BLAKE3-team reference KAT →
  the CID computation (D8) rests on a dependency that behaves per spec.

**Does NOT prove (the honest gaps):**
- Only 31 of 35 in-scope RFC vectors exercised in CI (the floor guard catches
  a regression to <30). The map-ordering cases (length-first) are the highest-
  value remaining check; nested-map ordering should be added.
- The BLAKE3 check is a single input (empty); the domain-separated framing
  (`lw.braid.*` domains) is self-checked, not cross-checked against an external
  domain-separation standard. A future flight hour: derive Braid's CID framing
  from the BLAKE3 derive_key spec and cross-check.
- These are *encoding/hash* flight hours. The verifier's 8 admission stages
  have no external cross-check yet (D-SA5: the Lean conformance check is the
  planned flight hour for the verifier).

## Next flight hours (the queue)

1. ~~RFC 8949 map-ordering vectors (length-first canonical order — the A4.8 lesson
   axis).~~ ✅ **Done (2026-07-15).** Braid's `key_cmp` matches RFC's deterministic
   order on multi-key maps (top-level + nested), cross-checked.
2. ~~The verifier's stage semantics vs the Lean `ExcellentCode.Framework` predicates
   (D-SA5).~~ ✅ **Done (2026-07-15).** Lean conformance check wired; 9/9 corpus
   verdicts agree between Rust and Lean; Lean build is axiom-free.
3. A known-bad corpus: cross-check the verifier's rejects against a pre-validated
   vulnerability dataset (NIST Juliet / OWASP) once Braid has a real elaborator
   (D-ELAB). Today Braid rejects its own crafted bad capsules; external bad
   capsules don't exist yet because no elaborator produces them.