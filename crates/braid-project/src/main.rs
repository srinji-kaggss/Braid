//! `braid-project build <manifest.json>` — elaborate + admit every capsule in a
//! project, fail-closed, and print each capsule's CID + the project CID. Mirrors
//! the CLI shape so the human-reconstructable loop holds at the project level.

use std::process::ExitCode;

use braid_project::{build_from_json, BuildReport};

fn run() -> Result<BuildReport, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.split_first() {
        Some((cmd, rest)) if cmd == "build" => {
            let path = rest
                .first()
                .ok_or_else(|| "usage: braid-project build <manifest.json>".to_string())?;
            let json = std::fs::read_to_string(path).map_err(|e| format!("reading {path}: {e}"))?;
            build_from_json(&json).map_err(|e| e.to_string())
        }
        _ => Err("usage: braid-project build <manifest.json>".to_string()),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(report) => {
            println!("project: {}", report.name);
            for e in &report.entries {
                println!("  ✓ {:<20} {}", e.name, e.cid.to_hex());
            }
            println!("project-cid: {}", report.project_cid.to_hex());
            println!("{} capsule(s) admitted", report.entries.len());
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("braid-project: {msg}");
            ExitCode::FAILURE
        }
    }
}
