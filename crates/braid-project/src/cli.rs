//! CLI adapter for `braid-project build`.

use crate::{build_from_json, build_rust, parse_project, Project, RustCrate};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn check_build_command(cmd: &str) -> Result<(), String> {
    if cmd != "build" {
        Err("usage: braid-project build <manifest.json> [--target rust]".into())
    } else {
        Ok(())
    }
}

fn extract_manifest_path(rest: &[String]) -> Result<&str, String> {
    rest.iter()
        .find(|a| !a.starts_with("--") && a.as_str() != "rust")
        .map(String::as_str)
        .ok_or_else(|| "usage: braid-project build <manifest.json> [--target rust]".into())
}

fn write_crate_files(dir: &Path, rc: &RustCrate) -> Result<(), String> {
    let cargo_path = dir.join("Cargo.toml");
    std::fs::write(&cargo_path, &rc.cargo_toml).map_err(|e| e.to_string())?;
    let build_path = dir.join("build.rs");
    std::fs::write(&build_path, &rc.build_rs).map_err(|e| e.to_string())?;
    let lib_path = dir.join("src/lib.rs");
    std::fs::write(&lib_path, &rc.lib_rs).map_err(|e| e.to_string())?;
    Ok(())
}

fn write_rust_crate(dir: &Path, rc: &RustCrate) -> Result<(), String> {
    let src_dir = dir.join("src");
    std::fs::create_dir_all(&src_dir).map_err(|e| e.to_string())?;
    write_crate_files(dir, rc)
}

fn emit_rust_target(project: &Project) -> Result<String, String> {
    let crates = build_rust(project).map_err(|e| e.to_string())?;
    let out_root = PathBuf::from(format!("{}-rust", project.name));
    let mut lines = vec![format!(
        "project: {} — {} rust crate(s) emitted into {}",
        project.name,
        crates.len(),
        out_root.display()
    )];
    for (name, rc) in &crates {
        let dir = out_root.join(name);
        write_rust_crate(&dir, rc)?;
        lines.push(format!("  ✓ {:<20} {}", name, dir.display()));
    }
    Ok(lines.join("\n"))
}

fn emit_plain_target(json: &str) -> Result<String, String> {
    let report = build_from_json(json).map_err(|e| e.to_string())?;
    let mut lines = vec![format!("project: {}", report.name)];
    for e in &report.entries {
        lines.push(format!("  ✓ {:<20} {}", e.name, e.cid.to_hex()));
    }
    lines.push(format!("project-cid: {}", report.project_cid.to_hex()));
    lines.push(format!("{} capsule(s) admitted", report.entries.len()));
    Ok(lines.join("\n"))
}

fn check_rust_flag(rest: &[String]) -> bool {
    rest.iter()
        .any(|a| a == "--target" || a == "rust" || a == "--target=rust")
}

fn read_manifest(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("reading {path}: {e}"))
}

fn dispatch_cli(cmd: &str, rest: &[String]) -> Result<String, String> {
    check_build_command(cmd)?;
    let want_rust = check_rust_flag(rest);
    let path = extract_manifest_path(rest)?;
    let json = read_manifest(path)?;

    if want_rust {
        let project = parse_project(&json).map_err(|e| e.to_string())?;
        emit_rust_target(&project)
    } else {
        emit_plain_target(&json)
    }
}

/// Run the project command and translate its result into process I/O and exit status.
pub fn run(args: &[String]) -> ExitCode {
    let outcome = args
        .split_first()
        .ok_or_else(|| "usage: braid-project build <manifest.json> [--target rust]".to_string())
        .and_then(|(cmd, rest)| dispatch_cli(cmd, rest));

    match outcome {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
