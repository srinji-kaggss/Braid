# Keel

**A standalone, deterministic verification authority for code authored by AI.**

A keel is the structural spine along the bottom of a hull. Everything is built on it;
without it the vessel cannot hold a heading. Keel is that spine for a codebase: a
restrictive, content-addressed **floor** of correctness and confinement that an
AI author cannot argue past, over which the rest of the system grows naturally.

## What it is, in one paragraph

Keel treats the code generator — today an LLM — as an **untrusted, possibly
incoherent author**. It does not review code the way a senior engineer advises a
junior; it **gates** code with independent, deterministic, reconstructable evidence. The verdict
is produced by deterministic measurement and symbolic composition — **never by a
second AI**. Keel runs with no network and no CI service (it was built because
GitHub Actions was billing-blocked and we needed an authority that does not depend
on anyone's servers), and it is designed to be lifted whole into any codebase and
**tailored by an AI through schema, not code edits**.

## The three properties that define it

1. **Deterministic floor, not advice.** Keel owns a *floor* of correctness and
   confinement-safety. Humans own architecture and intent *above* the floor. The
   floor is enforced by measurement, so the prose rulebook above it can shrink
   toward two lines. (See `docs/00-thesis.md`.)
2. **Content-addressed and stale-proof by construction.** Every verifiable thing is
   a node in a Merkle DAG, named by the hash of its content and its inputs' hashes.
   When source moves — and AI moves thousands of lines an hour — only the nodes whose
   content actually changed recompute; stale verdicts are *unreadable by
   construction*, not chased after the fact. (See `docs/01-architecture.md`.)
3. **Restrictive schema, AI-tailorable.** The set of obligations is a closed,
   restrictive schema. An AI adapts Keel to a new codebase by *filling the schema*,
   not by rewriting the engine. Degrees of freedom are the enemy of verifiability and
   the source of staleness; the schema is deliberately tight even when that costs the
   authoring AI extra effort. (See `docs/03-schema-and-tailoring.md`.)

## Two-tier acceptance

- **Tier 1 — operational acceptance.** The traditional gate: it compiles, types,
  lints, tests, builds. These tools do not *judge*; they *produce evidence*.
- **Tier 2 — semantic acceptance.** Evidence is compiled into the twenty axioms of
  the *Excellent Code Framework* (`docs/02-axioms.md`) and a verdict is read off a
  deterministic theorem (`Excellent ⇒ ¬Hallucinated`). The semantic layer never
  judges inline — it *compiles evidence and packages it separately*; the verdict is a
  mechanical implication over instantiated predicates.

## Mission

Keel aims to be a portable verification spine for AI-authored software, owned as a public-good style
artifact rather than a project-local convention. This repository is its standalone origin.

## Status

First pass. See `docs/` for the complete scientific specification and the issue
ledger for what is deferred. Keel is a *specification with a running seed*, not a
finished product — by design, the basement is poured before the house.

## Reviewing Keel

If you are an independent AI or auditor, **`docs/08-review-walkthrough.md` is the entry
point.** It lets you verify every claim below from a cold checkout — each with the file that
implements it and the command that proves it. Trust nothing the prose says; run the command:

```bash
node src/selftest.mjs          # algebra + hashing + concurrency + lean-proof-node identity
node src/qualify.mjs           # DO-330 tool qualification vs fixtures/known-bad/ (+ lean machine-check)
node src/qualify.mjs --self-test                       # the auditor-of-the-auditor
node src/run.mjs --profile examples/keel.profile.json  # Keel verifies itself → GO
(cd lean && lake build)        # machine-check the framework skeleton (needs elan/lake; else qualify SKIPs)
```

## The finite-outcome crossing (docs/07)

A codebase's outcomes are **finite, given its structure** — the decision space, the mutation
space, the input space, the failure space, the platform matrix. Verification is **crossing
that space to completion**, not sampling it: you find failure by exercising the declared
envelope and by loading every structural joint. Keel crosses the activation
set **concurrently** (the verdict is a pure function of content, so parallelism changes
throughput, never the result), composes with three-valued ∧ (one red *or* unknown point
dominates), and reports crossing coverage honestly (an uncrossed point is reported, never
assumed green).

## Safety-critical CI contract (docs/11)

`docs/11-safety-critical-ci.md` maps Keel's deterministic gates to the public FAA/RTCA/ISO shape of
DO-178B, DO-178C, DO-331, DO-330, and ISO 26262. The matching `safety_case` profile block makes those
claims machine-checkable: DAL/ASIL source, hazard analysis, requirements traceability, independence,
coverage, tool qualification, configuration index, and model-based evidence all become restrictive
schema fields. This is not a certification claim; it is the CI discipline future agents must satisfy
before touching life-impact code.

## CI/CD operations (docs/12)

`scripts/ci/run.mjs` is the portable CI surface for Keel itself: JSON sanity, overclaim scanning,
self-test, known-bad qualification, qualification-harness self-test, and the self-hosting profile
run. `.github/workflows/ci.yml` runs that surface and uploads `.ci-runs/` evidence. Future agents
should start with `docs/guides/agent-navigation.md`, then use the local skills under `skills/` for
CI architecture, safety-profile authoring, and release gating.

For internal audit runs, use `scripts/ci/vm-run.mjs` (see `docs/13-vm-soc-audit-runner.md`). GitHub
Actions is an optional projection; the SOC-style authority is a VM-backed run that stages the checkout
into a writable guest workspace, runs CI there, pulls evidence back, records guest identity/toolchain
state/logs/manifest hashes, and writes an audit seal. On the current workstation:

```bash
node scripts/ci/vm-run.mjs --provider lima --instance keel-ci
```

Host runs are marked `host-dev-only` and are never release evidence.

## Layout

```
docs/    scientific specification (00 thesis … 06 productization; 07 concurrency+crossing; 08 review walkthrough; 11 safety-critical CI; 12 CI/CD; 13 VM audit)
skills/  local agent skills for CI architecture, safety profiles, and release gates
schema/  the restrictive ontology: atoms (evidence), concepts (formulas), the tailoring profile
src/     the running seed: anchor, concurrency pool, atom evaluators, concept algebra, projections, runners, qualify
fixtures/ known-bad corpus — one planted case per core guarantee (DO-330 tool qualification)
examples/ a worked tailoring profile
```
