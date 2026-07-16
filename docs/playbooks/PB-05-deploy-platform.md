# PB-05 — The deploy platform: productizing the Vercel niche

**Objective**: assemble the primitives (verified IR, manifests, widening gate,
runner, elaborator) into the actual product — a deployment platform for AI-authored
components where **admission replaces review** in the deploy pipeline. Working name
for the surface: `braid-deploy` (name provisional per D15 discipline).

**The product thesis (from the README/PRD analysis)**: Vercel's contract is "push
code, get an immutable URL, promote when happy." Its trust model assumes a human
wrote and reviewed the code. When the author is an AI at machine speed, that
assumption is the bottleneck and the risk. Braid's contract is stronger where it
matters: *the deployment IS a verified artifact* — its full authority (capabilities,
effects, egress, budgets) is a decidable read off the manifest, previews are
review-objects not vibes, promotion of anything irreversible is cryptographically
confirm-gated, and rollback is a CID pointer move. **Sell the delta: "your AI can
deploy to it unsupervised, and you can prove what it can't do."**

## What already exists vs what this playbook builds

Already built (do not rebuild — Prime Directive 3): content-addressed artifacts
(`Capsule::cid`), the review object (`braid render`), the mechanical deploy gate
(`braid diff` widening exit-1 + Keel `NotSlop` floor), the human-reconstructable CLI
loop (T13), evidence bundles (`spec/braid/vectors/demo-port/` shows the shape).

Missing (the platform layer proper):
1. **Artifact store + deployment index** — CID-keyed store; a *deployment* is
   (name, env, capsule CID, manifest CID, admission record, journal ref); an *alias*
   (prod/preview) is a signed pointer to a deployment.
2. **The deploy pipeline daemon** — accept capsule bytes → verify → render →
   gate (widening vs currently-aliased deployment, not just vs parent commit) →
   store → serve preview.
3. **Preview serving** — run the capsule via `braid-run` (PB-02) with a
   projection-read-only ambient grant (previews NEVER hold irreversible/egress
   grants — attenuation is the preview sandbox, D10 doing Vercel's sandbox job).
4. **Promote** — re-verify, then if the manifest contains Irreversible/Egress
   strands, demand the payload-hash-bound confirmation (T10) from a human; write the
   alias move as a journaled, signed event.
5. **Rollback** — alias repoint to a prior deployment; the old artifact re-verifies
   at load (T9 — rollback is safe *because* admission is re-run, not assumed).
6. **Tenancy/identity seam** — defer real multi-tenant auth to the kernel's
   capability system (D10: Braid adds no authority; the platform must not mint a
   parallel auth system). v0 = single-tenant, Director-operated.

## Deep-learning corpus (read → probe)

1. This repo: `PRD.md` §5 (architecture placement — the platform is the box between
   "admit" and "runtime admission"), D12/T4 (manifest binding), T12 (the gate you are
   generalizing from CI to deploys), D28/D29 (the sandwich — deployment config is a
   ratified-architecture anchor question), PB-02/PB-03 outputs.
2. `scripts/cli-loop.sh` + `scripts/demo-port.sh` — the pipeline you are daemonizing
   already exists as a shell loop; the platform v0 is *literally this loop with a
   store and an alias file*.
3. blackbox2 `RESEARCH-enterprise-codebase-protection-patterns.md` (protection
   patterns the enterprise pitch cites) and `FRAMEWORK-human-using-ai-2026.md`
   (the human-decision-points model — maps to where confirm gates go).
4. Kernel State Fabric / Causal Tape docs (ADR-068) — the journal's eventual home.
5. Competitive grounding: Vercel/Netlify deploy semantics (immutable deploys,
   aliases, checks API) — study the *UX contract*, import none of the trust model.

Probe: execute a full manual "deploy" today with nothing but the CLI: encode →
verify → render → diff vs previous → cp into a CID-named dir → update an alias
symlink. Where that hurts is exactly the platform backlog.

## Invariants

- **No admission bypass lane**: there is no "trusted uploader" path that skips
  verify — including the platform's own internal artifacts (laws-of-the-repo: the
  gate applies to the gatekeeper).
- **Preview ≠ prod authority**: preview ambient grants are structurally attenuated
  (projection reads only); promotion is the ONLY place wider grants attach, and only
  via ratified anchors + confirm policy.
- **Alias moves are journaled and signed**; the current-prod CID is always
  re-derivable from the journal (statefulness = files, not daemon memory — the
  blackbox2 lesson applied to the platform).
- **The widening gate compares against what is LIVE** (aliased), not the lineage
  parent — an authority-neutral refactor chain must not launder a widening in.
- **Human-reconstructable** (T13): every platform operation has a CLI equivalent;
  the daemon is a convenience over the loop, never the only path.

## Execution steps

1. **Spec first** (D13): `spec/braid/deploy/PRD.md` — deployment/alias/promotion
   data model as typed structs, the threat-model delta (multi-deployment confused
   deputy, alias race, preview-grant escalation, store poisoning = T9 at rest), and
   acceptance scenarios in the PRD-§7 style. Get the Director's veto window.
2. `braid-store` crate: CID-keyed content store + deployment/alias records (canonical
   encoding + own `lw.braid.deploy.*` domains, KATs first — D8 applies to platform
   records too).
3. `braid-deploy` daemon (or subcommand set first: `braid deploy|promote|rollback|
   aliases|log`): the pipeline above; serve previews through `braid-run`.
4. Demo-port goes production: the afternow-port landing surface (D16's own target)
   deployed, previewed, promoted with a confirm, rolled back — recorded as the
   evidence bundle + a screen-capturable walkthrough. This is the marketing demo AND
   acceptance test (T14: no toy capsules).
5. AI-lane demo (the niche proof): an executor lane (per blackbox2
   PLAYBOOK-orchestrator routing) authors a component change via the PB-03
   elaborator and deploys it unsupervised; show the widening attempt getting
   mechanically refused and the clean change reaching preview with zero human
   touches. Measure: touches-per-deploy, time-to-preview, refusal precision.
6. Adversarial pass on the platform layer (alias race, replayed promotion
   confirmation, store swap, preview-grant escalation) before any external demo.

## Verification

```bash
cargo test -p braid-store -p braid-deploy
# end-to-end: deploy→preview→promote(confirm)→rollback on demo-port, journals pinned
# red-team: widening deploy vs live alias ⇒ refused; promotion with stale/replayed
#           confirmation ⇒ refused; store byte-swap ⇒ load-time re-verify refuses
./scripts/keel-floor.sh   # the platform crates join the NotSlop floor
```

## Exit criteria

A named surface (landing port) is served from a promoted, verified deployment with
journaled promote/rollback history; an AI lane deploys to preview unsupervised with
mechanical refusal of authority creep; the platform spec + threat delta + adversarial
verdict are on record. At that point the Vercel-niche claim is demonstrable, not
aspirational — and pricing/packaging becomes a Director conversation grounded in a
working artifact.
