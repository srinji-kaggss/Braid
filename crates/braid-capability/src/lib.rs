//! Vendored kernel capability contract (ADR-088 D3 boundary).
//!
//! SOURCE OF TRUTH: `canvas-protocol::Capability` on the Logic OS kernel
//! (`srinji-kaggss/logic-os-kernel`, `origin/main` —
//! `kernel/crates/canvas-protocol/src/lib.rs`). This is a VERBATIM mirror of
//! the closed permission-token enum, vendored so Braid stands alone without a
//! path/git dependency back into the kernel workspace.
//!
//! //why vendored, not depended-on: ADR-088 frames Braid as the extractable
//! machine-first framework. Only the `Capability` enum crosses the kernel
//! boundary (D3); pulling the whole `canvas-protocol` crate (and its serde_json
//! / uuid / chrono surface) into a standalone repo would import kernel types
//! Braid never uses. The mirror is kept byte-identical so re-sync is a trivial
//! diff against `origin/main`.
//!
//! INVARIANT: do not edit the variants, `#[serde(rename = …)]`, or
//! `#[strum(serialize = …)]` attributes here independently. The strum-`Display`
//! string is stored into the Braid IR (`term.rs`) and therefore feeds capsule
//! CIDs / pinned KAT vectors — drift here silently changes content addresses.
//! If the kernel bumps the enum, re-vendor verbatim from `origin/main`.

use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter, EnumString};

/// Permission token.
/// //why: ARCH-02 establishes this as the single source of truth for all kernel capability checks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Display, EnumString, EnumIter)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "kebab-case")]
pub enum Capability {
    #[serde(rename = "signal.emit")]
    #[strum(serialize = "signal.emit")]
    SignalEmit,
    #[serde(rename = "signal.subscribe")]
    #[strum(serialize = "signal.subscribe")]
    SignalSubscribe,
    #[serde(rename = "tape.read")]
    #[strum(serialize = "tape.read")]
    TapeRead,
    #[serde(rename = "view.inject")]
    #[strum(serialize = "view.inject")]
    ViewInject,
    #[serde(rename = "intent.emit")]
    #[strum(serialize = "intent.emit")]
    IntentEmit,
    #[serde(rename = "motion.schedule")]
    #[strum(serialize = "motion.schedule")]
    MotionSchedule,
    #[serde(rename = "motion.observe")]
    #[strum(serialize = "motion.observe")]
    MotionObserve,
    #[serde(rename = "motion.patch")]
    #[strum(serialize = "motion.patch")]
    MotionPatch,
    #[serde(rename = "motion.plugin.register")]
    #[strum(serialize = "motion.plugin.register")]
    MotionPluginRegister,
    #[serde(rename = "motion.replay")]
    #[strum(serialize = "motion.replay")]
    MotionReplay,
    #[serde(rename = "efface.shred")]
    #[strum(serialize = "efface.shred")]
    Shred,
    #[serde(rename = "efface.rtbf")]
    #[strum(serialize = "efface.rtbf")]
    Rtbf,
    /// Outbound remote computation request (#447).
    #[serde(rename = "compute.remote")]
    #[strum(serialize = "compute.remote")]
    RemoteCompute,
}
