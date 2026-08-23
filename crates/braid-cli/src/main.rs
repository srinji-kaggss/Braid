//! Process entrypoint for the Braid operator CLI.

use std::process::ExitCode;

use braid_cli::run;
use braid_runtime::entrypoint;

fn main() -> ExitCode {
    entrypoint(run)
}
