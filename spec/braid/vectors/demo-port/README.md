# demo-port — Day-0 CMS reference evidence (U8)

The Director's "landing page as first full port" (D16). Three demo-port CMS
actions — modeled on the kernel's `blueprints/afternow-port/` landing surface —
authored as Braid capsules and driven through the **author → admit → render**
legs of the `braid` CLI. This directory is the regenerable evidence bundle for
PRD §8.

| Action | Fixture | Verbs | Verdict |
|--------|---------|-------|---------|
| Edit the home hero section (reversible, local) | `edit-home-hero.json` | `cms.edit_section` + `view.section` | **ADMIT** — no egress, no irreversible (scenario #1) |
| Edit + publish the services page (irreversible, human-confirmed) | `publish-services.json` | `cms.edit_section` + `cms.publish` | **ADMIT** — irreversible strand + escalated grant, confirm declared |
| Read the work case-study listing (projection) | `render-work-listing.json` | `proj.listing` | **ADMIT** — read-only, no writes/egress |
| Publish without a confirm policy (escalation probe) | `publish-services-noconfirm.json` | `cms.publish` | **AUTHOR-REFUSED** — `ConfirmRequired`, fail-closed; no capsule emitted |

Each admitted action has a pinned `<name>.cid`, a `<name>.verdict`, and the
rendered `<name>.manifest.txt`. The same CIDs are pinned in
`crates/braid-cli/tests/demo_port.rs` (the CI gate) — drift turns the build
RED (T13 / scenario #13).

## Regenerate

```bash
cargo build -p braid-cli
./scripts/demo-port.sh            # rewrites this bundle, asserting every verdict
```

An independent re-run reproduces these files byte-for-byte (scenario #12).

## Deferred behind the U7 / kernel-WASM seam (NOT in this bundle — tracked in #6)

This slice covers authoring + admission + rendering only. The **execution leg**
is blocked on the kernel Day-0 WASM runtime epic (U7). When that lands, the seam
it plugs into is:

> **capsule CID → kernel runtime load → manifest re-derivation (refuse-on-mismatch, T4/scenario #10) + fact journal on tape**

Scenario #3's *runtime* confirmation-hash-mismatch reject (T10) and the runtime
halves of scenarios #9 (budget kill) and #10 (manifest spoof) ride this seam.
Naming it now is the build-the-basement discipline so U7 extends this slice
rather than refactoring it.
