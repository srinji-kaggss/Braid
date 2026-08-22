//! `lgwks-gate` is the human-facing half of INV-DEP-REGISTERED: the same audit
//! the build script runs, runnable by hand, plus the two commands that make the
//! admission process a path rather than a folk practice.
//!
//! This binary is a doctor, not an authority. `check` diagnoses, `request`
//! prints a block for a human to fill in and commit, and `init` writes a
//! fail-closed starting register. None of them can approve anything — approval
//! is a diff with a name on it, which is the whole point of the contract.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use lgwks_std_gate::{
    check_dependencies, check_dependencies_against, contract::Contract, repository_root, Refusal,
    CONTRACT_PATH,
};

const USAGE: &str = "\
lgwks-gate — dependency admission for the std+ estate

USAGE
  lgwks-gate check [PATH]              audit the repo at PATH (default: cwd)
             [--contract FILE]         read the register from FILE instead of
                                       PATH/contract/APPROVED.toml. Diagnosis
                                       only — a build always reads the register
                                       committed beside the code it builds.
  lgwks-gate request <CRATE> <VERSION> print an approval block to fill in
  lgwks-gate init [PATH]               write a fail-closed starting register
  lgwks-gate tiers                     print the admission ladder

EXIT
  0  every resolved dependency is std, lgwks_std, local, or approved
  2  a refusal, a missing register, or an unparseable one
";

fn parse_check_args(args: &[String]) -> (Option<PathBuf>, Option<PathBuf>) {
    let positional: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let override_path = args
        .iter()
        .position(|a| a == "--contract")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    let target_path = positional.first().map(|p| PathBuf::from(p.as_str()));
    (target_path, override_path)
}

fn handle_check(args: &[String]) -> ExitCode {
    let (target_path, override_path) = parse_check_args(args);
    run_check(target_path, override_path)
}

fn handle_help() -> ExitCode {
    print!("{USAGE}");
    ExitCode::SUCCESS
}

fn handle_tiers() -> ExitCode {
    print!("{LADDER}");
    ExitCode::SUCCESS
}

fn handle_unknown(other: &str) -> ExitCode {
    eprintln!("lgwks-gate: unknown command {other:?}\n");
    eprint!("{USAGE}");
    ExitCode::from(2)
}

fn dispatch(command: &str, args: &[String]) -> ExitCode {
    match command {
        "check" => handle_check(&args[1..]),
        "request" => run_request(args.get(1), args.get(2)),
        "init" => run_init(args.get(1).map(PathBuf::from)),
        "tiers" => handle_tiers(),
        "-h" | "--help" | "help" => handle_help(),
        other => handle_unknown(other),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("");
    dispatch(command, &args)
}

// ── check ───────────────────────────────────────────────────────────────────

fn audit_root(
    root: &Path,
    contract_override: &Option<PathBuf>,
) -> Result<(Contract, Vec<Refusal>), String> {
    let outcome = match contract_override {
        Some(path) => check_dependencies_against(root, path),
        None => check_dependencies(root),
    };
    outcome.map_err(|e| e.to_string())
}

fn report_refusals(root: &Path, register: &Contract, refusals: &[Refusal]) -> ExitCode {
    eprintln!(
        "REFUSED  {} — {} unapproved dependencies\n",
        root.display(),
        refusals.len()
    );
    for refusal in refusals {
        eprintln!("  {refusal}");
    }
    eprintln!(
        "\nEach one is a decision, not a paperwork step. Climb the ladder first \
         (`lgwks-gate tiers`);\nif the answer is still a dependency, \
         `lgwks-gate request <crate> <version>` prints the block."
    );
    if !register.enforce {
        eprintln!("\nNOTE  [policy] enforce = false, so builds still pass. This is adoption-only.");
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}

fn report_ok(root: &Path, count: usize) -> ExitCode {
    println!(
        "OK  {} — {} approved, every other resolved crate is local or lgwks_std",
        root.display(),
        count
    );
    ExitCode::SUCCESS
}

fn run_check(path: Option<PathBuf>, contract_override: Option<PathBuf>) -> ExitCode {
    let start = path.unwrap_or_else(|| PathBuf::from("."));
    let root = match repository_root(&start) {
        Ok(root) => root,
        Err(e) => return refuse(&e.to_string()),
    };
    let (register, refusals) = match audit_root(&root, &contract_override) {
        Ok(outcome) => outcome,
        Err(err_msg) => return refuse(&err_msg),
    };

    if refusals.is_empty() {
        report_ok(&root, register.entries.len())
    } else {
        report_refusals(&root, &register, &refusals)
    }
}

fn refuse(message: &str) -> ExitCode {
    eprintln!("REFUSED  {message}");
    ExitCode::from(2)
}

// ── request ─────────────────────────────────────────────────────────────────

fn print_request_template(krate: &str, version: &str) {
    print!("{LADDER}");
    println!(
        "\n\
         If every rung above still leaves a dependency, append this to {CONTRACT_PATH},\n\
         fill in the four blanks, and commit it. The commit is the approval.\n\n\
         [[approved]]\n\
         crate = \"{krate}\"\n\
         tier = \"boundary\"          # boundary | vendor — see the ladder above\n\
         version = \"{version}\"\n\
         reason = \"\"                # one sentence naming what std cannot do\n\
         approved_by = \"\"           # the human who decided\n\
         approved_on = \"\"           # YYYY-MM-DD\n\
         review = \"\"                # path or URL to the evidence\n"
    );
}

fn run_request(krate: Option<&String>, version: Option<&String>) -> ExitCode {
    let (Some(krate), Some(version)) = (krate, version) else {
        eprint!("{USAGE}");
        return ExitCode::from(2);
    };
    print_request_template(krate, version);
    ExitCode::SUCCESS
}

// ── init ────────────────────────────────────────────────────────────────────

const STARTER: &str = "\
# Approved dependencies — the semantic contract for INV-DEP-REGISTERED.
#
# A crate reaches this file only after the ladder in `lgwks-gate tiers` has been
# climbed and every rung above a dependency was rejected for a stated reason.
# Adding a block here is an approval; the commit that adds it is the signature.
#
# ELIMINATE and CONSOLIDATE crates never appear here. They become a module in
# `lgwks_std` instead, which is why `tier` admits only `boundary` and `vendor`.

[policy]
# Fail-closed. Set to false only while a repo is being brought onto the gate,
# and only in a diff a human signed off — refusals then report as warnings.
enforce = true
";

fn check_target_exists(target: &Path) -> Result<(), String> {
    if target.exists() {
        Err(format!(
            "{} already exists; init will not overwrite it",
            target.display()
        ))
    } else {
        Ok(())
    }
}

fn create_parent_dirs(target: &Path) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))
    } else {
        Ok(())
    }
}

fn prepare_init_file(target: &Path) -> Result<(), String> {
    check_target_exists(target)?;
    create_parent_dirs(target)?;
    std::fs::write(target, STARTER).map_err(|e| format!("cannot write {}: {e}", target.display()))
}

fn run_init(path: Option<PathBuf>) -> ExitCode {
    let start = path.unwrap_or_else(|| PathBuf::from("."));
    let root = match repository_root(&start) {
        Ok(root) => root,
        Err(e) => return refuse(&e.to_string()),
    };
    let target = root.join(CONTRACT_PATH);
    if let Err(msg) = prepare_init_file(&target) {
        return refuse(&msg);
    }
    println!("OK  initialized {}", target.display());
    ExitCode::SUCCESS
}

// ── Ladder text ─────────────────────────────────────────────────────────────

const LADDER: &str = "\
The std+ admission ladder (INV-DEP-REGISTERED)

Every dependency in Cargo.lock must be accounted for at one of these rungs.
Lower rungs are preferred; each step up is an escalation that requires a reason.

  1. Rust standard library (std / core / alloc)
     Preferred unconditionally. Zero dependencies, zero supply-chain risk.

  2. Workspace stdlib+ (`lgwks_std`)
     The common substrate: id (uuid v4), hex, time (RFC 3339), glob, fs, leb128, task.
     Zero external dependencies of its own; uses standard library only.

  3. ELIMINATE
     Crates whose functionality belongs in `lgwks_std` or std.
     Target for removal: write the minimal zero-dependency implementation.

  4. CONSOLIDATE
     Multiple crates solving the same problem.
     Target for convergence: pick one, retire the rest.

  5. VENDOR
     A mature, audit-clean dependency whose code is reviewed and checked into the
     workspace rather than resolved through crates.io at build time.

  6. BOUNDARY
     A third-party dependency approved for use across an external boundary
     (e.g., protocol parsers, hardware drivers, cryptographic primitives).
     Must be declared in `contract/APPROVED.toml` with a human sign-off.
";
