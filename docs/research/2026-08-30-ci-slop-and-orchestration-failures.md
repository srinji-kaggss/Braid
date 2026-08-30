# CI slop markers and orchestration failures

Date: 2026-08-30  
Tracking issue: #78  
Scope: Braid's GitHub Actions workflow, local receipt gate, and current Keel seam

## Result

"Slop" is not a score and a marker is not a verdict. The useful automated
boundary is a small set of failure shapes whose semantics are unambiguous:

| Class | Blocking marker | Why it is load-bearing |
|---|---|---|
| Failure closure | `continue-on-error: true` or an executable `|| true` | A failing required check can be reported as success. |
| Provenance | An external action referenced by a tag or branch | The executed action can change without a Braid source change. |
| Coverage | A source-classifier skips required jobs | A green run may not have tested the changed gate or data. |
| Liveness | A job has no positive timeout | A lost dependency or runner can occupy the orchestration graph indefinitely. |
| Lifecycle | Cleanup does not wait for every producer or does not run after failure | Persistent self-hosted runners leak run-owned state into later evidence. |
| Falsifiability | A policy has no known-bad fixture | Exit zero may mean the checker did not observe its target. |
| Authority drift | Current docs describe a gate or dependency path that no longer exists | Reviewers reason about an imagined control plane rather than executed bytes. |

`TODO`, function length, clone counts, and complexity are review signals, not
automatic correctness failures. Treating every textual smell as a gate creates
false positives and teaches developers to silence the scanner. A blocking
marker must name the invariant it violates and have a negative fixture.

## Braid findings

The 2026-08-30 audit found five concrete orchestration defects:

1. The stack-position lookup redirected diagnostics and appended `|| true`.
   GitHub/API failure therefore looked identical to "no blocking PR".
2. Every external action used a mutable `@v4` or `@stable` reference.
3. The scope classifier omitted several control/data surfaces and allowed
   required tests and lint to be skipped. The classifier itself had already
   needed a repair in #94 after a shell/workflow change skipped its own lane.
4. One job had no timeout.
5. Cleanup waited for only `tests` and `clippy` and removed only the unsuffixed
   target directory. Five run-owned suffixed target directories could remain on
   the persistent runner.

The assurance seam had separate authority drift. Current files repeatedly said
that the Keel `NotSlop` floor ran in CI, but neither `.github/workflows/ci.yml`
nor `.wwfd/local-ci.sh` invoked it. `scripts/keel-floor.sh` expected the removed
Node entry point `keel/src/run.mjs`, while Keel is not present in a clean Braid
checkout. The current Keel repository is private, so a clean unauthenticated
consumer cannot fetch it as an implicit tool dependency. The installed native
Keel binary was therefore used only as a diagnostic: on Braid main
`3a89e608f50c232cbf8ef26600d662b9c3a76375` it returned `NO-GO` with 454
findings over 210 files. That is debt evidence, not a release gate result.

## Implemented policy

`scripts/ci-policy-check.sh` now rejects the five workflow failure shapes above
and proves the checker with six negative fixtures: mutable action, continued
error, swallowed shell failure, missing timeout, early cleanup, and scope skip.
The workflow runs this policy before build work.

The workflow also:

- pins `actions/checkout` and `dtolnay/rust-toolchain` to full 40-character
  commit identifiers;
- runs the full Braid gate for every change instead of maintaining an
  incomplete source classifier;
- makes stack lookup failure block the graph;
- gives every job a timeout; and
- makes cleanup wait for every job and delete the six exact run-owned target
  directories.

This does not close #78. Current Keel distribution, the finding baseline,
offline evidence export, and release-time mutation/U9 receipts remain open.
The obsolete adapter is made explicit rather than advertised as green.

## Primary sources

- GitHub documents that a failed or skipped dependency skips downstream jobs,
  that `continue-on-error` permits a step failure without failing the job, and
  that job/step timeouts are explicit workflow controls:
  <https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax>
- GitHub documents full commit SHAs as the immutable action reference and
  provides a repository setting that can require them:
  <https://docs.github.com/en/actions/how-tos/create-and-publish-actions/manage-custom-actions>
  and
  <https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/enabling-features-for-your-repository/managing-github-actions-settings-for-a-repository>
- GitHub recommends ephemeral self-hosted runners because they provide a clean
  environment per job and reduce leakage from previous jobs:
  <https://docs.github.com/en/actions/reference/runners/self-hosted-runners>
- NIST SP 800-204D requires automated checks over all artifacts in a change and
  confidence in the source of CI tools:
  <https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-204D.pdf>
- NIST's DevSecOps reference model requires pipeline evidence and self-contained
  test environments that are decommissioned after each iteration:
  <https://pages.nist.gov/nccoe-devsecops/notational-reference-model.html>
- Google's testing guidance identifies improper cleanup, prior-run state,
  missing timeouts, and race/order assumptions as sources of flaky evidence:
  <https://testing.googleblog.com/2021/03/test-flakiness-one-of-main-challenges.html>
- Van Deursen et al.'s original test-smell work establishes that test code has
  its own failure patterns; later empirical work also warns that static smell
  detectors have false positives, supporting the marker-versus-verdict split:
  <https://www.researchgate.net/publication/2534882_Refactoring_Test_Code>
  and <https://pure.tudelft.nl/ws/portalfiles/portal/82718732/main.pdf>

## Falsifiers

This policy is wrong if a negative fixture above is accepted, if a legitimate
immutable action form is rejected, if cleanup can start before a target producer
finishes, or if a change can still produce a successful run without executing
the full workspace tests and lint. GitHub's terminal job graph remains the
independent integration proof; the source policy cannot prove runner behavior.
