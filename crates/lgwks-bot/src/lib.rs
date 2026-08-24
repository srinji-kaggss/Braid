//! `lgwks_bot` owns automation semantics and the bot ontology.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use lgwks_std as _;

pub mod cap;
pub mod domain {
    //! Shipped automation domains.
    pub mod chat;
    pub mod data;
    pub mod eval;
    pub mod flow;
    pub mod fs;
    pub mod gh;
    pub mod net;
    pub mod notify;
    pub mod sys;
}
pub mod error;
pub mod gate;
pub mod json;
pub mod spec;
pub mod verb;

pub use cap::Cap;
pub use error::BotError;
pub use gate::GrantSet;
pub use spec::{Bot, BotSpec, Chain, ChainEntry};
pub use verb::{Evaluate, Execute, Observe, Query};

pub use domain::chat;
pub use domain::data;
pub use domain::eval;
pub use domain::flow;
pub use domain::fs;
pub use domain::gh;
pub use domain::net;
pub use domain::notify;
pub use domain::sys;
