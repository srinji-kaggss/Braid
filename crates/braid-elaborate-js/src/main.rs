//! Process entrypoint for the JavaScript elaborator CLI.

use std::process::ExitCode;

use braid_elaborate_js::cli;
use braid_runtime::entrypoint;

fn main() -> ExitCode {
    entrypoint(cli::run)
}
