# Crate ownership map

This map answers issue #28's O1 question: every crate states the invariant it
owns, the events that may legitimately change it, and the reason its boundary
exists. The rule is deliberately strict: **authority, lifecycle, failure mode,
scaling, security, or cadence** justify a boundary; symmetry or speculative
reuse does not.

## Audit basis

Measured at `af69f012a9298a817ee8c8863bf2d09cf1b046fe`: 17 workspace crates,
66 Rust source files, and 17,075 lines in `crates/*/src/**/*.rs`. The original
#28 census predated the `lgwks-*` crates, which are intentionally present as a
separate authority surface rather than part of the Braid substrate. The D5
process-boundary extraction adds `braid-runtime`, so the current audited count
is 18.

## Boundaries

| Crate | Invariant owned | Legitimate reason to change | Split justification |
| --- | --- | --- | --- |
| `braid-capability` | A capability is a verbatim dotted-name token; equality and canonical bytes are name-based, with no normalization or domain enum. | Change the protocol-stable capability representation or its serialized identity. | **Authority/security:** this tiny type feeds CIDs and admission for every vocabulary; isolating it prevents domain policy from leaking into the shared token. |
| `braid-ir` | The type universe, canonical byte form, BLAKE3 CID contract, DAG/capsule shapes, and closed registry *shape*. | Change canonical encoding, content addressing, typed graph structure, capsule framing, or registry-shape rules. | **Authority/security:** Braid's sole identity and encoding authority must remain a small, independently reviewable trust base. |
| `braid-verify` | Independent decoding plus fail-closed admission against a supplied registry and ambient grants. | Add an admission stage, tighten rejection behavior, or revise the independent decoder. | **Authority/failure mode:** admission must stay separate from construction, rendering, and execution so no producer can make its own accept decision. |
| `braid-render` | Deterministic human manifests, widening/narrowing deltas, and DOT projection derived from admitted material. | Change presentation, delta classification, or graph-projection output. | **Lifecycle/cadence:** display and audit projections can evolve without altering IR bytes or admission semantics. |
| `braid-sdk` | The typed builder's compile-adjacent authoring state and strand-construction checks. | Improve authoring ergonomics, builder diagnostics, or build-time validation. | **Lifecycle/failure mode:** consumer-facing API compatibility is managed separately from the wire form and the independent verifier. |
| `braid-vocab-cms` | The CMS domain alphabet: types, terms, capabilities, pinned registry identity, and reference capsules. | Add or retire CMS terms/capabilities, or pin a new registry version. | **Authority/cadence:** domain vocabulary changes on CMS product cadence while the substrate remains globally neutral. |
| `braid-vocab-js` | The JavaScript elaboration alphabet and its typed values, terms, and capability names. | Change the JavaScript subset's term signatures, types, or capability space. | **Authority/cadence:** JS language/frontend concerns stay out of both the universal IR and unrelated vocabularies. |
| `braid-vocab-web` | The browser engine's closed `web.*` action alphabet and pinned browser registry identity. | Revise the browser action contract or pin a new browser vocabulary. | **Authority/security:** browser side effects have their own capability envelope and consumer contract. |
| `braid-vocab-rust` | The admitted-capsule-to-Rust emission contract and generated dependency-free API shape. | Change Rust emission strategy, naming, module layout, or generated-API compatibility. | **Lifecycle/cadence:** generated-Rust compatibility is distinct from source parsing, admission, and runtime execution. |
| `braid-elaborate-js` | Untrusted JS text parsing, statement/expression elaboration, refusal typing, and depth bounds. | Expand accepted syntax, improve typed refusals, or harden resource limits. | **Security/failure mode:** hostile-source handling stays isolated from vocabulary authorities and cannot be bypassed by builder/runtime internals. |
| `braid-project` | Whole-project admission: unique capsule names, independent per-capsule admission, no partial success, and a deterministic project CID. | Change project-manifest schema, aggregation policy, or project-CID framing. | **Failure mode/lifecycle:** project builds are toolchain artifacts and fail as a whole; they do not create a second capsule-admission authority. |
| `braid-runtime` | The executable startup contract: validated OS arguments and one startup-failure path before any domain state exists. | Change process/platform input handling or the common startup diagnostic contract. | **Failure mode/lifecycle:** OS startup failures are handled once before partial domain work; each CLI retains its own policy/output semantics. |
| `braid-cli` | The human-reconstructable command loop across encode, decode, verify, render, diff, catalog, and store workflows. | Add commands, alter output contracts, or improve operator recovery. | **Cadence/lifecycle:** operator UX and CLI compatibility move independently from library APIs. |
| `braid-manifest` | The repository-manifest sibling artifact: closed dimensions, required fields, validation, canonical bytes, and CID domain. | Change the inventory schema, closed enums, validation rules, or manifest CID domain. | **Authority/lifecycle:** repository metadata uses Braid's encoding discipline but is deliberately not an admitted capsule. |
| `braid-run` | Deterministic DAG evaluation, capability-gated effect dispatch, confirmation/budget enforcement, and journal evidence. | Change execution order guarantees, host dispatch, resource accounting, evidence records, or effect handling. | **Security/failure/scaling:** execution is a distinct trust and resource-control surface from static admission and authoring. |
| `braid-governance` | Signed Keel change envelopes, session budgets, allowed actions, commitments, expiry, and generation-time denial. | Change envelope policy/schema, governance crypto inputs, budget rules, or denial semantics. | **Authority/security/failure mode:** Keel authoring governance is explicitly outside IR admission; isolation also confines its SHA-256/Ed25519 dependency set. |
| `lgwks-std` | The estate's one approved non-`std` primitive surface, including platform, encoding, automation, and serialization primitives. | Add a primitive, revise its platform contract, or change an approved dependency-backed implementation. | **Authority/security/cadence:** this is a governed alternative to `std`, reviewed by dependency and primitive rather than through Braid IR releases. |
| `lgwks-std-gate` | Build-time proof that `lgwks-std` dependencies match the human-approved contract. | Change the approval contract format, lockfile audit, refusal taxonomy, or gate entry point. | **Security/failure mode:** unauthorized dependency growth fails the build through a dedicated gate rather than relying on review memory. |

## Outcome

All 18 boundaries satisfy all three columns. The earlier #28 behavioral slices
removed the web-vocabulary panic surface, bounded JS elaboration input, and made
the five-result swallow ceiling a CI gate. This slice completes issue #28's
entrypoint consolidation without merging any existing crate.
