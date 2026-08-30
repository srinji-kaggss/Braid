//! Deterministic CMS v1 registry exporter used by the release gate.

use std::env;
use std::io::{self, Write};
use std::process::ExitCode;

use braid_vocab_cms::registry_v0;

fn main() -> ExitCode {
    let format = match env::args().nth(1).as_deref() {
        None | Some("raw") => "raw",
        Some("hex") => "hex",
        Some(other) => {
            eprintln!("unsupported format `{other}` (expected `raw` or `hex`)");
            return ExitCode::from(2);
        }
    };

    let registry = registry_v0();
    let bytes = braid_ir::encode(&registry.to_canon());
    let mut stdout = io::stdout().lock();
    let result = if format == "hex" {
        for byte in &bytes {
            if write!(stdout, "{byte:02x}").is_err() {
                return ExitCode::FAILURE;
            }
        }
        writeln!(stdout)
    } else {
        stdout.write_all(&bytes)
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("registry export failed: {error}");
            ExitCode::FAILURE
        }
    }
}
