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
separate authority surface rather than part of the Braid substrate. Later
additions — `braid-runtime` (D5 process boundary), `braid-flow-ir` /
`braid-flow-verify` (P2), `braid-flow-plan` (P3), `braid-governance` (Keel envelopes), and now
`braid-integrate` (repo-graph advisor) — bring the present count to 23.

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
| `braid-governance` | Signed Keel change envelopes, session budgets, allowed actions, commitments, expiry, and generation-time denial. | Change envelope policy/schema, governance crypto inputs, budget rules, or denial semantics. | **Authority/security/failure mode:** Keel authoring governance is explicitly outside IR admission; isolation also confines its hash/signature dependency set (BLAKE3 via `lgwks_std::hash`, Ed25519). |
| `braid-integrate` | Repo-graph advisor: file inventory + import-line scan + manifest signals → `lgwks_std` / `lgwks_bot` seams (read-only unless `--apply`; never a second verifier). | Add a detector, adjust a seam heuristic, or change patch/proposal shape. | **Lifecycle/security:** advisor output is non-authoritative and reversible; isolation keeps heuristics from leaking into admission or encoding trust bases. |
| `lgwks-std` | The estate's one approved non-`std` primitive surface, including platform, encoding, automation, and serialization primitives. | Add a primitive, revise its platform contract, or change an approved dependency-backed implementation. | **Authority/security/cadence:** this is a governed alternative to `std`, reviewed by dependency and primitive rather than through Braid IR releases. |
| `braid-test-support` | The workspace's single property-testing dependency boundary. | Change the external property engine or shared falsification support. | **Dependency authority/cadence:** production crates keep only a local dev edge; one crate owns the external test engine. |
| `lgwks-std-gate` | Direct dependency-edge ownership from Cargo metadata against the human-approved contract. | Change the approval schema, metadata audit, refusal taxonomy, or CI entry point. | **Security/failure mode:** unowned, mis-sourced, wrong-consumer, and stale approvals fail before the workspace build rather than becoming review folklore. |

## Ratified Frontier Flow boundaries

ADR-099 ratifies four implementation crates. `braid-flow-ir` (P1), `braid-flow-verify` (P2), and `braid-flow-plan` (P3) are delivered present on main (PRs 68,85); `braid-flow-sdk` (P4) remains planned. They
land through Braid #57, #60, #59, and #58; after #59, the SDK work in #58 may
proceed in parallel with the separately owned Forge runtime adapter.

| Planned crate | Invariant owned | Legitimate reason to change | Split justification |
| --- | --- | --- | --- |
| `braid-flow-ir` *(P1 foundation present)* | The closed outer graph shape, deterministic encoding, domain-separated semantic Flow identity, bounded source normalization, and predicate AST. | Change identity-bearing Flow semantics, encoding, bounds, or the closed v0 graph vocabulary. | **Authority/security:** semantic bytes and CID remain a small `no_std + alloc` trust base independent of source parsers and runtimes. |
| `braid-flow-verify` *(P2 delivered — present on main since PR 68)* | Independent canonical decoding and fail-closed static admission of Flow structure, types, reachability, joins, bounded symbolic Choice disjointness, terminals, justification, and authority non-aggregation. | Add or tighten a Flow admission obligation, symbolic fragment, resource ceiling, or typed refusal. | **Authority/failure mode:** a builder or importer cannot make its own graph admissible. |
| `braid-flow-plan` *(P3 delivered — present on main since PR 85)* | Deterministic, immutable-snapshot-bound satiation, Choice-target evaluation, next-frontier derivation, and Plan identity. | Change trusted readiness, satiation, invalidation, selection, or planning-context semantics. | **Failure mode/cadence:** trusted planning is distinct from graph admission and from Forge's durable scheduling lifecycle. |
| `braid-flow-sdk` *(P4 planned)* | Rust builder state, first-class RON authoring, normalized JSON interoperability, source diagnostics, and lowering to the one Flow AST. | Improve authoring ergonomics, source schema versions, diagnostics, or importer-facing conversion. | **Lifecycle/security:** textual parsing and friendly recovery evolve without entering canonical IR or verifier trust bases. |

Existing integration ownership is also frozen: `braid-render` owns deterministic
Flow manifest and full DOT projection; `braid-project` may consume admitted
Flows after P2 but owns neither admission nor execution; `braid-run` executes
one admitted capsule selected by a plan step and never becomes a durable
scheduler. RON/JSON/YAML parsing stays out of `braid-flow-ir`,
`braid-flow-verify`, and `braid-flow-plan`.

## Outcome

All 23 boundaries satisfy all three columns. The earlier #28 behavioral slices
removed the web-vocabulary panic surface, bounded JS elaboration input, and made
the five-result swallow ceiling a CI gate; later slices added
`braid-governance` and `braid-integrate` without changing any existing crate's
boundary.
