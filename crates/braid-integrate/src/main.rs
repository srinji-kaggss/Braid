//! Process entrypoint for the Braid integration advisor.

use std::process::ExitCode;

use braid_integrate::run;
use braid_runtime::entrypoint;

fn main() -> ExitCode {
    entrypoint(run)
}
