# AI Evidence Contract

This repository treats AI/model output as **untrusted first-party evidence** until independently verified.

## Why this exists
A language model can generate code, tests, CI narratives, PR descriptions, and explanations that reinforce the same mistaken assumption. Internal consistency is not proof. Plausibility is not implementation. Confidence is not evidence.

## Mandatory claim states
Every material capability claim must be reported separately as:
1. **Planned** — described or intended only.
2. **Present in code** — identifiable implementation exists at an exact commit.
3. **Exercised** — the exact claimed path was run on that exact commit.
4. **Independently evidenced** — evidence is not authored solely by the same model/agent making the claim.
5. **Safe to rely on / merge** — failure modes, scope, compatibility, and relevant negative tests have been reviewed.

Never collapse these states into “done.”

## Evidence rules
- Self-authored tests, generated fixtures, PR-body claims, documentation, and model summaries are first-party evidence only.
- Green CI proves only the checks that actually ran. It does not prove unstated semantics.
- A content hash does not prove a persistent Merkle DAG.
- A single-host test does not prove distributed orchestration.
- A standards-inspired test does not prove certification or standards compliance.
- Architectural intent, type signatures, mocks, and compile success do not prove runtime behavior.
- Every capability statement must name the exact exercised path, commit, environment, and observed result.

## Falsification before celebration
Before declaring success, actively search for the cheapest counterexample that would make the claim false. Prefer adversarial, external, differential, integration, and end-to-end evidence over self-confirming unit tests.

## Scope discipline
Set a hard scope budget before editing. If a supposedly small fix expands into broad architecture churn, stop and re-evaluate. Large commit counts or unrelated rewrites are evidence of scope failure, not diligence.

## Merge gate
No AI-authored change is merge-safe merely because it is mergeable, reviewed by the same agent, or green in CI. Merge safety requires evidence proportionate to the claim and risk.

## Reporting rule
When evidence is missing, say **unknown**, **not exercised**, or **not independently verified**. Never substitute a plausible story.

This contract outranks convenience, velocity, and model confidence.