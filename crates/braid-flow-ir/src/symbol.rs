//! Validated identifiers. Invalid identity-bearing names are unrepresentable.

use crate::error::IdentifierError;
use alloc::string::{String, ToString};

pub const MAX_IDENTIFIER_BYTES: usize = 128;

fn is_valid_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_IDENTIFIER_BYTES {
        return false;
    }
    if !bytes[0].is_ascii_lowercase() {
        return false;
    }
    bytes[1..].iter().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'.' | b'-')
    })
}

macro_rules! identifier {
    ($name:ident, $kind:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: &str) -> Result<Self, IdentifierError> {
                if is_valid_identifier(value) {
                    Ok(Self(value.to_string()))
                } else {
                    Err(IdentifierError {
                        kind: $kind,
                        length: value.len(),
                    })
                }
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl core::str::FromStr for $name {
            type Err = IdentifierError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }
    };
}

identifier!(FlowName, "Flow name");
identifier!(NodeKey, "node key");
identifier!(InputKey, "input key");
identifier!(PortKey, "port key");
identifier!(FactRef, "snapshot fact reference");
identifier!(RelationRef, "relation reference");
identifier!(InvariantRef, "invariant reference");
identifier!(CostOrderRef, "cost-order reference");
