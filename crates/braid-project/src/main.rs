//! Process entrypoint for the project toolchain.

use std::process::ExitCode;

use braid_project::cli;
use braid_runtime::entrypoint;

fn main() -> ExitCode {
    entrypoint(cli::run)
}
