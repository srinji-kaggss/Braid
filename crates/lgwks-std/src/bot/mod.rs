//! `bot` owns the named automation primitive and enforces INV-BOT-FOUR-VERBS:
//! every bot is built from exactly four verb traits — Observe, Evaluate, Execute,
//! Query — wired together as `(condition, action)` tuples bound to observed sources.
//!
//! The domain axis is open: shipped domains cover GitHub, filesystem, network,
//! system, notification, data, chat, and flow composition. Custom domains pass
//! the same capability gate as shipped ones (INV-BOT-SAME-GATE).

pub mod cap;
pub mod domain;
pub mod error;
pub mod gate;
pub mod spec;
pub mod verb;

pub use cap::Cap;
pub use error::BotError;
pub use gate::GrantSet;
pub use spec::{Bot, BotSpec, Chain, ChainEntry};
pub use verb::{Evaluate, Execute, Observe, Query};
