# Mutation Ledger — braid-verify 8-stage qualification (U-SA AC-6)

> U-SA §8 AC-6: "The verifier self-qualifies: mutation-red evidence exists for
> each `braid-verify` stage (the U9 discipline extended to all 8 stages)."
>
> This ledger records, for each admission stage in `braid-verify/src/lib.rs`,
> a mutation (what code was changed), the named test that went RED, and why
> that RED proves the stage is load-bearing. All mutations were applied to the
> clean tree, the specific test was run, the RED was confirmed on the semantic
> assertion (not an incidental side effect), and the mutation was reverted.
>
> **Stages already mutation-verified by U9** (T3/T5) are cross-referenced here,
> not re-litigated. The new mutations cover the remaining 6 stages.

---

## The 8 stages

| # | Stage | Mutation-red test | RED type | Status |
|---|-------|-------------------|----------|--------|
| 1 | CanonicalForm | `scenario_6_malleable_bytes_rejected` + bijection guard | Admit-where-Reject-expected | U9/T3 ✅ |
| 2 | VersionPin | `scenario_7_version_skew_rejected` | Admit-where-Reject-expected | **NEW** ✅ |
| 3 | Structure | `output_out_of_range_rejected` | Admit-where-Reject-expected | **NEW** ✅ |
| 4 | Types | `type_mismatch_rejected` | Admit-where-Reject-expected | **NEW** ✅ |
| 5 | Capability | `scenario_4_grant_exceeding_ambient_rejected` | Admit-where-Reject-expected | **NEW** ✅ |
| 6 | Effect | `scenario_2_irreversible_without_confirm_rejected` | Admit-where-Reject-expected | **NEW** ✅ |
| 7 | Taint | `scenario_5_path_taint_catches_multihop_laundering` | Admit-where-Reject-expected | U9/T5 ✅ |
| 8 | Bounds | `scenario_9_budget_exceeded_rejected` | Admit-where-Reject-expected | **NEW** ✅ |

---

## Mutation details

### Stage 1 — CanonicalForm (U9/T3, cross-referenced)

**What the stage does**: strict decode of canonical bytes + independent
bijection guard (re-encode decoded value, compare to input bytes). Rejects
floats, tags, non-minimal ints, out-of-order map keys, trailing bytes, and any
non-bijective encoding.

**Mutation**: comment out the bijection guard in `decode_canonical()`
(`crates/braid-verify/src/decode.rs:207-218`):
```rust
// MUTATION: bijection guard bypassed
// let mut re = Vec::with_capacity(bytes.len());
// reencode(&v, &mut re);
// if re != bytes { return Err(DecodeError::NotBijective); }
```

**Named test**: `scenario_6b_nested_submap_smuggle_rejected`
(`crates/braid-verify/tests/acceptance.rs:229`).

**RED evidence**: with the guard removed, the sub-map smuggling bytes (a key
inserted into the nested braid map) decode successfully but would produce
different bytes on re-encoding — without the guard, this difference goes
undetected. The `Capsule::from_canon` `require_only_keys` check provides a
second layer, but the bijection guard is the independent backstop that
survives a decoder bug. The U9 verdict (T3) records this as a closed High.

**Why load-bearing**: two independent encoders agreeing on byte form is the
anti-malleability invariant (T3). The bijection guard is the *machine-checked*
guarantee, not a narrated one.

### Stage 2 — VersionPin (NEW)

**What the stage does**: pins `ir_version`, `vocab_version`, and
`registry_cid` against the registry — refuses skew (T6: no silent migration).

**Mutation**: bypass all three version checks in `lib.rs:58-67`:
```rust
// MUTATION: entire version-pin stage bypassed
let _ = (capsule.ir_version, IR_VERSION, capsule.vocab_version, ...);
```

**Named test**: `scenario_7_version_skew_rejected`
(`crates/braid-verify/tests/acceptance.rs:107`).

**RED evidence**: `thread 'scenario_7_version_skew_rejected' panicked at
acceptance.rs:24:34: expected reject at VersionPin, got Admit`. The capsule
with `vocab_version += 1` was ADMITTED instead of REJECTED — the semantic
assertion (`expect_reject`) caught it directly.

**Why load-bearing**: without the version pin, a capsule compiled against one
vocabulary version would be admitted against a different registry — a silent
migration that breaks content-addressing (D11).

### Stage 3 — Structure (NEW)

**What the stage does**: `Braid::validate()` checks the strand DAG is
acyclic (forward references rejected), outputs are in range, strands are
non-empty. Then checks every term is known in the registry and arity matches.

**Mutation**: bypass `validate()` in `lib.rs:70-72`:
```rust
// MUTATION: validate() call bypassed
// if let Err(e) = capsule.braid.validate() { ... }
```

**Named test**: `output_out_of_range_rejected`
(`crates/braid-verify/tests/acceptance.rs:187`).

**RED evidence**: `thread 'output_out_of_range_rejected' panicked at
acceptance.rs:24:34: expected reject at Structure, got Admit`. A capsule with
`outputs = vec![99]` was ADMITTED because the verifier never reads
`capsule.braid.outputs` — only `validate()` checks this. Without the stage,
the structural defect is invisible to all later stages.

**Why load-bearing**: `validate()` is the gate that guarantees DAG invariants
the incremental taint fold (Stage 7) and the types unification (Stage 4) rely
on. Bypassing it on forward-reference inputs causes panics in those stages
(index-out-of-bounds in the `exposure` vector) — proof that later stages
*assume* the structure stage ran.

### Stage 4 — Types (NEW)

**What the stage does**: each strand's input slot type must unify with the
producing strand's output type.

**Mutation**: bypass the type comparison in `lib.rs:96-108`:
```rust
// MUTATION: type check bypassed
let _produced = out_types[input_idx as usize];
let _expected = &spec.inputs[slot];
// if produced != expected { ... reject ... }
```

**Named test**: `type_mismatch_rejected`
(`crates/braid-verify/tests/acceptance.rs:156`).

**RED evidence**: `thread 'type_mismatch_rejected' panicked at
acceptance.rs:24:34: expected reject at Types, got Admit`. A capsule feeding
an Entity output into a Text input slot was ADMITTED — the type mismatch was
invisible.

**Why load-bearing**: without type checking, a capsule can wire incompatible
strands together (e.g., feeding a capability token where text is expected),
producing a capsule that no runtime could execute safely.

### Stage 5 — Capability (NEW)

**What the stage does**: every grant must be ⊆ ambient authority; every
strand requiring a capability must have it declared in the capsule's grants.

**Mutation**: bypass the grant-vs-ambient check in `lib.rs:111-118`:
```rust
// MUTATION: grant-vs-ambient check bypassed
for _g in &capsule.grants { /* if !ambient.contains(g) { ... reject ... } */ }
```

**Named test**: `scenario_4_grant_exceeding_ambient_rejected`
(`crates/braid-verify/tests/acceptance.rs:62`).

**RED evidence**: `thread 'scenario_4_grant_exceeding_ambient_rejected'
panicked at acceptance.rs:24:34: expected reject at Capability, got Admit`.
A capsule requesting `signal.emit` when the ambient only has `tape.read` was
ADMITTED — authority creep.

**Why load-bearing**: without the ambient check, a capsule can claim any
capability regardless of what the principal actually holds — the attenuation
principle (D10: Braid adds no authority) is unenforced.

### Stage 6 — Effect (NEW)

**What the stage does**: any strand with `Irreversible` or `Egress` effect
class requires `ConfirmPolicy::HumanConfirm`.

**Mutation**: bypass the confirm-policy check in `lib.rs:135-146`:
```rust
// MUTATION: effect-confirm check bypassed
let _needs_confirm = ...;
// if needs_confirm && capsule.confirm != ConfirmPolicy::HumanConfirm { ... }
```

**Named test**: `scenario_2_irreversible_without_confirm_rejected`
(`crates/braid-verify/tests/acceptance.rs:41`).

**RED evidence**: `thread 'scenario_2_irreversible_without_confirm_rejected'
panicked at acceptance.rs:24:34: expected reject at Effect, got Admit`. An
irreversible publish capsule with `ConfirmPolicy::None` was ADMITTED — an
unsafe action without human confirmation.

**Why load-bearing**: without the effect gate, irreversible/egress actions
proceed without confirmation — the human-in-the-loop safety check (T10 static
half) is removed.

### Stage 7 — Taint (U9/T5, cross-referenced)

**What the stage does**: path-level monotone fold — `exposure(strand) =
max(term source, folded input exposures)`. Egress sinks with a ceiling check
the *folded* incoming value, so `vault → pure → pure → egress` carries its
taint through every pure hop.

**Mutation**: replace `exposure[input_idx]` (the folded value) with
`spec.source_exposure` (the per-hop term value), reverting to the kernel's
pre-fix per-hop behavior:
```rust
// MUTATION: per-hop instead of path-level fold
incoming = spec.source_exposure.max(incoming);
// (was: incoming = incoming.max(exposure[input_idx as usize]);)
```

**Named test**: `scenario_5_path_taint_catches_multihop_laundering`
(`crates/braid-verify/tests/acceptance.rs:87`).

**RED evidence**: `thread 'scenario_5_path_taint_catches_multihop_laundering'
panicked at acceptance.rs:24:34: expected reject at Taint, got Admit`. The
laundering capsule (vault→pure→pure→egress) was ADMITTED — the taint was
lost at the first pure hop instead of propagating.

**Why load-bearing**: the per-hop version was the exact bug the kernel shipped
(#361→#431). The path-level fold is the fix. Without it, sensitive data can
reach egress through a chain of pure-function hops.

### Stage 8 — Bounds (NEW)

**What the stage does**: checked-sum of strand costs; overflow ⇒ reject
(not wrap). Total cost must be ≤ capsule budget.

**Mutation**: bypass the budget comparison in `lib.rs:184-189`:
```rust
// MUTATION: budget comparison bypassed
let _total: u64 = total;
// if total > capsule.budget { ... reject ... }
```

**Named test**: `scenario_9_budget_exceeded_rejected`
(`crates/braid-verify/tests/acceptance.rs:135`).

**RED evidence**: `thread 'scenario_9_budget_exceeded_rejected' panicked at
acceptance.rs:24:34: expected reject at Bounds, got Admit`. A capsule with
budget 3 but strands costing 12 total was ADMITTED — cost overrun invisible.

**Why load-bearing**: without the budget check, a capsule can exceed its
declared resource envelope (T7). The checked-sum half (overflow ⇒ reject) is
also load-bearing: `u64` wraparound would make an overflowing capsule appear
cheap.

---

## Method

Each mutation was applied to the clean tree, the specific test was run with
`cargo test -p braid-verify --test acceptance <test_name>`, and the RED output
was verified to be on the semantic assertion (`expected reject at <Stage>,
got Admit`) — not an incidental panic or compilation error. After verification,
the mutation was reverted (`cp /tmp/lib.rs.bak` or `git checkout`).

All mutations were applied individually (never two at once) and reverted
before the next. The clean tree passes all 20 acceptance tests + 3 parity
tests after all mutations are reverted.
