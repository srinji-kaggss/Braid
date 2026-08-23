//! The shared process boundary for Braid's executable shells.
//!
//! Runtime owns the only concerns common to every entrypoint: reading the OS
//! argument vector without panicking on non-UTF-8 input, exposing validated
//! command arguments, and giving startup failures one diagnostic shape. Domain
//! parsing, admission policy, output contracts, and exit semantics stay with
//! each CLI; this crate deliberately owns no domain behavior.
//!
//! The boundary is justified by **failure mode/lifecycle**: process-startup
//! failures must be handled before a frontend can construct partial state, and
//! the OS interface changes on toolchain/platform cadence rather than with any
//! Braid vocabulary or artifact format.

#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::process::ExitCode;

/// Every way shared process setup can fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupError {
    /// The operating system supplied no program-name argument.
    MissingProgram,
    /// A command argument was not valid UTF-8.
    InvalidArgument {
        /// Zero-based index among arguments after the program name.
        position: usize,
        /// Lossy rendering for diagnostics only.
        value: String,
    },
}

impl core::fmt::Display for StartupError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingProgram => f.write_str("process is missing its program-name argument"),
            Self::InvalidArgument { position, value } => {
                write!(f, "command argument {position} is not valid UTF-8: {value}")
            }
        }
    }
}

impl std::error::Error for StartupError {}

/// Validated process inputs handed to an executable shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Runtime {
    program_name: String,
    arguments: Vec<String>,
}

impl Runtime {
    fn from_os_args<I>(args: I) -> Result<Self, StartupError>
    where
        I: IntoIterator<Item = OsString>,
    {
        let mut args = args.into_iter();
        let program_name = args
            .next()
            .ok_or(StartupError::MissingProgram)?
            .to_string_lossy()
            .into_owned();

        let mut arguments = Vec::new();
        for (position, arg) in args.enumerate() {
            match arg.into_string() {
                Ok(arg) => arguments.push(arg),
                Err(arg) => {
                    return Err(StartupError::InvalidArgument {
                        position,
                        value: arg.to_string_lossy().into_owned(),
                    })
                }
            }
        }

        Ok(Self {
            program_name,
            arguments,
        })
    }

    /// The operating system's name for this executable.
    #[must_use]
    pub fn program_name(&self) -> &str {
        &self.program_name
    }

    /// Command arguments after the program name.
    #[must_use]
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }
}

/// Read and validate the real process arguments.
pub fn bootstrap() -> Result<Runtime, StartupError> {
    Runtime::from_os_args(std::env::args_os())
}

/// Start a Braid executable with the common validated argument vector.
///
/// `shell` receives command arguments after the program name and retains full
/// ownership of domain parsing, output contracts, policy outcomes, and exit
/// codes. Startup failures use exit 2 (operator error) consistently.
pub fn entrypoint(shell: impl FnOnce(&[String]) -> ExitCode) -> ExitCode {
    match bootstrap() {
        Ok(runtime) => shell(runtime.arguments()),
        Err(error) => {
            eprintln!("startup error: {error}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(args: &[&str]) -> Result<Runtime, StartupError> {
        Runtime::from_os_args(args.iter().map(OsString::from))
    }

    #[test]
    fn bootstrap_separates_program_from_command_arguments() {
        let runtime = os(&["braid", "verify", "capsule.braid"]).unwrap();
        assert_eq!(runtime.program_name(), "braid");
        assert_eq!(
            runtime.arguments(),
            ["verify".to_string(), "capsule.braid".to_string()]
        );
    }

    #[test]
    fn missing_program_fails_closed() {
        assert_eq!(Runtime::from_os_args([]), Err(StartupError::MissingProgram));
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_argument_is_rejected_without_panicking() {
        use std::os::unix::ffi::OsStringExt;

        let invalid = OsString::from_vec(vec![0xff]);
        let error = Runtime::from_os_args([OsString::from("braid"), invalid]).unwrap_err();
        assert_eq!(
            error,
            StartupError::InvalidArgument {
                position: 0,
                value: "\u{fffd}".to_string(),
            }
        );
    }
}
