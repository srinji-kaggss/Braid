//! `braid-project build <manifest.json> [--target rust]` — elaborate + admit
//! every capsule in a project, fail-closed. Plain build prints each capsule's
//! CID + the project CID; `--target rust` additionally emits each admitted
//! capsule as a dependency-free Rust crate (`<name>-rust/<capsule>/`). Mirrors
//! the CLI shape so the human-reconstructable loop holds at the project level.

use std::process::ExitCode;

use braid_project::{build_from_json, build_rust, parse_project};

fn run() -> Result<String, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cmd, rest) = args
        .split_first()
        .ok_or("usage: braid-project build <manifest.json> [--target rust]")?;
    if cmd != "build" {
        return Err("usage: braid-project build <manifest.json> [--target rust]".into());
    }
    let want_rust = rest
        .iter()
        .any(|a| a == "--target" || a == "rust" || a == "--target=rust");
    let path = rest
        .iter()
        .find(|a| !a.starts_with("--") && a.as_str() != "rust")
        .ok_or("usage: braid-project build <manifest.json> [--target rust]")?;
    let json = std::fs::read_to_string(path).map_err(|e| format!("reading {path}: {e}"))?;
    let project = parse_project(&json).map_err(|e| e.to_string())?;

    if want_rust {
        let crates = build_rust(&project).map_err(|e| e.to_string())?;
        let out_root = std::path::PathBuf::from(format!("{}-rust", project.name));
        let mut lines = vec![format!(
            "project: {} — {} rust crate(s) emitted into {}",
            project.name,
            crates.len(),
            out_root.display()
        )];
        for (name, rc) in &crates {
            let dir = out_root.join(name);
            std::fs::create_dir_all(dir.join("src")).map_err(|e| e.to_string())?;
            std::fs::write(dir.join("Cargo.toml"), &rc.cargo_toml).map_err(|e| e.to_string())?;
            std::fs::write(dir.join("build.rs"), &rc.build_rs).map_err(|e| e.to_string())?;
            std::fs::write(dir.join("src/lib.rs"), &rc.lib_rs).map_err(|e| e.to_string())?;
            lines.push(format!("  ✓ {:<20} {}", name, dir.display()));
        }
        return Ok(lines.join("\n"));
    }

    let report = build_from_json(&json).map_err(|e| e.to_string())?;
    let mut lines = vec![format!("project: {}", report.name)];
    for e in &report.entries {
        lines.push(format!("  ✓ {:<20} {}", e.name, e.cid.to_hex()));
    }
    lines.push(format!("project-cid: {}", report.project_cid.to_hex()));
    lines.push(format!("{} capsule(s) admitted", report.entries.len()));
    Ok(lines.join("\n"))
}

fn main() -> ExitCode {
    match run() {
        Ok(msg) => {
            println!("{msg}");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("braid-project: {msg}");
            ExitCode::FAILURE
        }
    }
}
