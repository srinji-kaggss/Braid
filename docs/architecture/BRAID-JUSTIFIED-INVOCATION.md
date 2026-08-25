# Braid Justified Invocation

**Status:** design note / research thesis, not yet a normative protocol  
**Date:** 2026-08-25  
**Scope:** machine-first representation and verification of *why an invocation should exist at all*.

## Thesis

A meaningful function call should be treated as a three-component product:

\[
\boxed{(\text{safe},\ \text{authorized},\ \text{justified})}
\]

In the idealized model used here:

1. **Safe** — the language/runtime proves the operation can execute without violating its structural safety rules. Rust ownership/borrowing is the motivating example.
2. **Authorized** — the caller possesses the externally-defined authority required to perform the effects. Braid does **not** own that authority contract; the constellation charter assigns the capability envelope to `logic-os-kernel`.
3. **Justified** — there is a presently unsatisfied condition, and this invocation has a deterministic causal reason to reduce or satisfy it.

The third component is the focus of this note.

The question is not:

> Can this function run?

Nor:

> Is this caller allowed to run it?

It is:

> **Why is this function running now? What presently unsatisfied condition gives this invocation a reason to exist?**

A safe, authorized, correctly implemented function may still be an invalid invocation because nothing needs it.

---

## The missing primitive: need-to-act

Most programming systems make invocation itself cheap semantically:

```text
caller requested f
        ↓
preconditions hold
        ↓
execute f
```

This note proposes a stronger admission boundary:

```text
caller requested f
        ↓
structurally safe?
        ↓
authorized?
        ↓
unsatisfied reason exists?
        ↓
f is proven relevant to that reason?
        ↓
execute f
```

The key rule is:

\[
\boxed{
Run(f,S,G)
\iff
Safe(f,S)
\land Authorized(f,S)
\land Justified(f,S,G)
}
\]

where `S` is the fixed state against which admission is evaluated and `G` is the declared satisfactory condition or invariant.

The third term must not be delegated to an LLM, embedding similarity, confidence model, heuristic score, or free-form natural-language rationale.

---

## Justification is not purpose metadata

A declaration such as:

```text
purpose: "rebuild the index"
```

is documentation. It does not prove that rebuilding the index is currently warranted.

Justification is invocation-specific:

```text
current state
    ↓
unsatisfied invariant
    ↓
required state change
    ↓
function proven to produce that change
    ↓
invocation admitted
```

Canonical causal chain:

\[
\boxed{
Invariant
\rightarrow Discrepancy
\rightarrow RequiredEffect
\rightarrow Action
}
\]

If the chain cannot be constructed, the action does not execute.

---

## Satiation: doing nothing must be a valid winner

The biological intuition motivating this note is homeostatic rather than maximization-oriented:

> act because a condition is unsatisfied; stop when it is satisfied.

A system should not indefinitely maximize a metric merely because an action can improve it. It should define a satisfactory region and refuse unnecessary work once the state is inside that region.

Example:

```text
healthy index:
    fragmentation <= 10%
    latency <= 10ms
```

Current state:

```text
fragmentation = 3%
latency        = 7ms
```

`rebuild_index()` may be safe, authorized, and correctly implemented. It is still unjustified because the relevant need is already satisfied.

```text
Safe       ✓
Authorized ✓
Justified  ✗

=> do not execute
```

This is a distinct class of correctness: **the implementation can be correct while the invocation is wrong**.

---

## Do not turn justification into a fuzzy scalar

A tempting implementation is:

\[
profit = w_1x_1 + w_2x_2 + \dots
\]

and then:

```text
if profit > 0.73:
    run
```

That reintroduces the exact ambiguity this mechanism is intended to remove. Weights, thresholds, model estimates, and confidence scores become a hidden policy language.

The trusted gate should instead operate over exact predicates, proof obligations, and deterministic ordering.

For example:

```text
1. hard invariants may never be violated
2. a declared need must currently be unsatisfied
3. the action must guarantee relevant progress or satisfaction
4. if multiple actions satisfy the same need, choose by a declared deterministic cost order
5. exact ties use a canonical tie-break
```

No weighted utility is required.

A useful formalization is:

\[
Justified(f,S,G)
=
\neg G(S)
\land Applicable_f(S)
\land Progress_f(S,G)
\]

For a strong version of `Progress`:

\[
\forall S' \in f(S): Progress_G(S',S)
\]

That is a proof obligation, not a prediction.

---

## "Vibes off" becomes `Unknown`

Humans often have a useful primitive that looks like:

> everything obvious checks out, but I do not have enough confidence in the situation to act.

The deterministic translation is not a probability. It is incomplete proof.

Use three-valued admission logic:

```rust
pub enum JustificationState {
    Proven,
    Disproven,
    Unknown,
}
```

Semantics:

```text
Proven     => may continue through admission
Disproven  => reject
Unknown    => do not execute; defer or surface missing proof
```

The critical asymmetry is:

\[
\boxed{\text{only known-good may execute}}
\]

not:

\[
\neg\text{known-bad} \Rightarrow \text{good}
\]

`Unknown` is therefore a first-class, fail-closed state.

In Rust specifically, ordinary admission failure should normally be represented as a typed error or deferral rather than `panic!`; panic remains appropriate for violated internal invariants, not routine control flow. A strict development mode may choose to panic on an impossible or supposedly-unreachable admission state to expose contract bugs early.

---

## Functions declare guarantees, not guesses

The declaration must state what makes the invocation relevant and what the implementation guarantees.

Illustrative syntax only:

```text
action compact_index(index) {
    needed_when:
        index.fragmentation > 10

    guarantees:
        next.index.fragmentation < current.index.fragmentation

    satisfied_when:
        index.fragmentation <= 10

    preserves:
        data_integrity
        index_readability

    cost_order:
        bounded_cpu
        bounded_io
}
```

This is deliberately different from:

```text
probably_reduces_fragmentation: 0.91
```

If a guarantee cannot be proven under the declared domain, it cannot be used as justification evidence.

Some guarantees may be compile-time provable; others may require runtime evidence, metering, checked transitions, or a verifier over a bounded state model. The trusted decision still remains deterministic over the evidence it receives.

---

## Proof-carrying invocation

The conceptual object is an invocation that cannot exist without its `why`.

Illustrative Rust-like shape:

```rust
pub struct JustificationProof<F, N> {
    need: N,
    state_version: u64,
    _function: core::marker::PhantomData<F>,
}
```

Then conceptually:

```rust
fn compact_index(
    index: &mut Index,
    why: JustificationProof<CompactIndex, FragmentationNeed>,
) -> Result<(), CompactError> {
    // ...
}
```

This note intentionally does not define or duplicate the kernel-owned capability envelope. The complete invocation product should compose external authority evidence with Braid-verifiable justification evidence without Braid becoming a second authority owner.

The design goal is:

> **You physically cannot represent an admitted meaningful invocation without a machine-checkable reason for that invocation to exist.**

---

## Freeze the relevant state during admission

A justification proven against state `S₀` is invalid if the relevant state becomes `S₁` before execution.

Otherwise the system has a classic check/use race:

```text
prove action needed
        ↓
world changes
        ↓
execute stale justification
```

The proof therefore binds to a version, snapshot, lease, transaction, or other explicit state identity.

```rust
let snapshot = state.version();
let why = prove::<CompactIndex>(&state)?;
execute_if_state_matches(snapshot, why)?;
```

If the relevant state changed, invalidate the proof and re-evaluate. Do not silently reuse it.

This is also how lossy upstream observations are prevented from becoming hidden mutable context inside the function: admission happens against a named, fixed representation.

---

## History may be lossy; the gate may not be fuzzy

The system may derive a compact state from a much larger history:

\[
H_{0..t} \rightarrow S_t
\]

That compression can intentionally lose information. The invocation gate then operates only on the declared state `S_t` and its provenance/version.

For example:

```text
raw execution history
        ↓
deterministic classifier / verified state reducer
        ↓
KnownStable | KnownEdge | Novel | PreviouslyFailed
        ↓
justification rules
```

The gate must not smuggle uncertainty back in through an undocumented "vibe" score. If an upstream component cannot deterministically establish a state fact, that fact is `Unknown` for admission purposes.

This separates two questions:

1. **How was the world summarized?**
2. **Given the admitted facts, is the action justified?**

Only the second belongs to this trusted primitive.

---

## Global profit without scalar utility

The motivating intuition is that work should only occur when its net effect improves the system relative to declared needs, and that the least expensive path to an equivalent satisfactory outcome should win.

Do not encode this as a universal scalar "profit" function. Local profit can create global loss, and arbitrary weights make incomparable concerns silently tradeable.

Instead use ordered constraints:

```text
GLOBAL HARD INVARIANTS
        ↓
SYSTEM SATISFACTION
        ↓
SUBSYSTEM SATISFACTION
        ↓
ACTION COST
```

An action that violates a higher-order invariant is not allowed to compensate by being cheaper elsewhere.

When two actions both preserve all higher invariants and reach the same declared satisfactory region, the cheaper action may deterministically win:

\[
f^*
=
\arg\min_f Cost(f,S)
\quad\text{subject to}\quad
f(S) \in S_{good}
\]

Example:

```text
state: fragmentation = 34%
healthy: fragmentation <= 10%

incremental_compact:
    guarantees <= 10%
    bounded cost = 300ms

rebuild_index:
    guarantees <= 10%
    bounded cost = 4000ms
```

If the cost model is comparable and both guarantees are proven, `incremental_compact` wins. If the state is already healthy, **doing nothing wins**.

---

## Proposed declaration dimensions

A first implementation experiment should try to keep the language small. A meaningful action needs at most these justification-facing declarations:

```text
needed_when      — exact predicate establishing an unsatisfied condition
satisfied_when   — exact predicate defining "enough"
guarantees       — proven state relation/effect of the action
preserves        — invariants that may not regress
cost_order       — optional deterministic choice rule among equivalent valid actions
```

The first three are the core. `preserves` provides defense in depth. `cost_order` is selection policy, not justification itself.

The essential relationship is:

\[
needed\_when(S)
\land guarantees(f,S,S')
\land satisfied\_when(S')
\]

or, for actions that make partial progress rather than finish the job, a declared well-founded progress relation.

---

## Determinism requirements

A compliant justification gate should obey all of the following:

1. **Explicit inputs only.** No hidden model context, wall-clock dependency, environment read, or mutable global may influence the result unless it is represented in the evaluated state.
2. **Canonical state identity.** The proof binds to the state/version it was created against.
3. **Exact predicates.** Trusted admission logic does not consume confidence scores.
4. **Three-valued result.** `Proven`, `Disproven`, `Unknown`.
5. **Fail closed.** `Unknown` does not execute.
6. **Canonical tie-breaking.** Equivalent valid actions cannot depend on nondeterministic iteration order.
7. **No untracked fallback.** A failed justification cannot silently route to "best effort" execution.
8. **Proof invalidation is explicit.** Relevant state change invalidates stale proof.
9. **Bounded decision procedure.** Admission itself must have declared resource bounds and termination behavior.
10. **Replayability.** Given the same canonical action declaration and state snapshot, the verifier must reach the same admission result.

This is how the mechanism remains a logic primitive instead of becoming an AI policy layer.

---

## What "zero loss" should mean here

Do not use ML loss terminology literally.

The target is not:

```text
model loss -> 0
```

The useful target is:

> **zero undeclared admission surface inside the supported domain.**

For every supported `(action, state)` pair, the system should deterministically produce one of:

```text
Proven
Disproven
Unknown(reason)
```

As declarations become more complete, the reachable `Unknown` region should shrink. But forcing an answer where proof is unavailable would destroy the safety property. A truthful `Unknown` is better than a guessed `Proven`.

---

## Relationship to existing work

This thesis is a synthesis, not a claim that the underlying mathematics is new.

Relevant intellectual ancestors include:

- **Hoare logic** — preconditions, programs, and postconditions.
- **Design by Contract** — executable preconditions/postconditions/invariants.
- **Typestate** (Strom/Yemini) — valid operations depend on the current abstract state.
- **Refinement/dependent types** — values carry predicates/proofs beyond ordinary nominal types.
- **Effect and capability systems** — constrain which effects a computation may perform; authority remains externally owned in the Braid constellation.
- **Proof-carrying code** — code/evidence is accepted only when a verifier can establish a policy.
- **Model checking / planning** — reason over explicit state transitions and goal states.
- **Cybernetics, homeostasis, and perceptual control** — action as reduction of discrepancy from a satisfactory/reference state, with cessation once satisfied.
- **Affordance competition / action selection** — biological action selection as competition among currently available actions biased by context and need, rather than requiring language-level deliberation.

The potentially distinctive synthesis is narrower:

> **make the invocation itself carry a deterministic, machine-verifiable causal justification that an unsatisfied condition exists and this action is relevant to satisfying it.**

That is stronger than documenting purpose and orthogonal to memory safety or authority.

---

## What this does *not* solve

This mechanism does not prove that arbitrary software computes the correct semantic answer.

A function may be:

```text
safe       ✓
authorized ✓
justified  ✓
implemented incorrectly ✗
```

Traditional correctness obligations still matter.

Likewise, arbitrary general-purpose semantic properties are undecidable. The tractable system must therefore operate over a deliberately constrained declaration language and bounded/provable state relations. The goal is not to solve the halting problem by syntax; it is to make a useful class of invocation rationale explicit enough that the machine can refuse unjustified work deterministically.

---

## Why this belongs near Braid

Braid's chartered role is canonical machine-first IR, encoding, term vocabulary, and verifier. That makes it a plausible home for the **representation of justification declarations and proofs**, provided the concept is ratified and does not duplicate authorities owned elsewhere.

A future implementation could let Braid encode:

```text
Need
SatisfactionPredicate
ActionEffect
PreservedInvariant
JustificationProof
```

and verify their internal/canonical relationships, while the kernel or another registered owner decides runtime admission policy and supplies authority evidence.

This note does **not** establish those names as public API or authority ownership. It records the design thesis so implementation can be falsified before the vocabulary is frozen.

---

## First falsification experiments

Before designing a DSL, test whether the idea survives small deterministic worlds.

### Experiment A — unnecessary work

Create five actions that are all safe and authorized but where only some address a currently unsatisfied invariant. The verifier must reject every irrelevant action with no model inference.

### Experiment B — satiation

Once the satisfactory predicate is true, repeated invocation of the satisfying action must be rejected as unjustified.

### Experiment C — two equivalent actions

Two actions satisfy the same need. A canonical cost rule must choose the same winner under repeated/reordered evaluation.

### Experiment D — stale proof

Construct a justification under state version `N`, mutate the relevant state to `N+1`, and prove that the old justification cannot be consumed.

### Experiment E — unknown

Remove one required fact. The result must be `Unknown`, never guessed `Proven`.

### Experiment F — local/global conflict

Create a locally useful action that violates a higher-order invariant. The global invariant must dominate regardless of local benefit or lower cost.

### Experiment G — adversarial declaration

Attempt to create circular justification:

```text
action is needed because action should run
```

or a satisfaction predicate that is trivially always true/false. The verifier or authoring layer must expose the vacuity rather than accept a meaningless proof shell.

---

## Open questions

1. What is the smallest declaration language that can express `needed_when`, `satisfied_when`, and `guarantees` without becoming a second programming language?
2. Which guarantees must be statically proven versus dynamically checked against a transaction/snapshot?
3. How are nested needs composed without creating circular justification?
4. What establishes the root invariant — where does the `why` chain intentionally terminate?
5. How do we represent partial progress while proving termination or a well-founded decrease?
6. What is the canonical ordering when multiple actions satisfy the same invariant with incomparable costs?
7. How do we prove that a declaration is non-vacuous and not merely written to game the gate?
8. Which part is Braid IR/verifier vocabulary versus kernel runtime policy?

The central constraint for all future work is simple:

> **Do not solve an inability to declare `why` by asking an intelligent model to guess `why`.**

The point of the mechanism is to flatten that decision into inspectable logic.
