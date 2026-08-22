//! `lgwks-gate` is the human-facing half of INV-DEP-REGISTERED: the same audit
//! the build script runs, runnable by hand, plus the two commands that make the
//! admission process a path rather than a folk practice.
//!
//! This binary is a doctor, not an authority. `check` diagnoses, `request`
//! prints a block for a human to fill in and commit, and `init` writes a
//! fail-closed starting register. None of them can approve anything — approval
//! is a diff with a name on it, which is the whole point of the contract.

use std::path::PathBuf;
use std::process::ExitCode;

use lgwks_std_gate::{check, check_against, contract::Contract, repository_root, CONTRACT_PATH};

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

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("");
    match command {
        "check" => {
            let positional: Vec<&String> =
                args[1..].iter().filter(|a| !a.starts_with("--")).collect();
            let override_path = args
                .iter()
                .position(|a| a == "--contract")
                .and_then(|i| args.get(i + 1))
                .map(PathBuf::from);
            run_check(positional.first().map(|p| PathBuf::from(p.as_str())), override_path)
        }
        "request" => run_request(args.get(1), args.get(2)),
        "init" => run_init(args.get(1).map(PathBuf::from)),
        "tiers" => {
            print!("{LADDER}");
            ExitCode::SUCCESS
        }
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("lgwks-gate: unknown command {other:?}\n");
            eprint!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

// ── check ───────────────────────────────────────────────────────────────────

fn run_check(path: Option<PathBuf>, contract_override: Option<PathBuf>) -> ExitCode {
    let start = path.unwrap_or_else(|| PathBuf::from("."));
    let root = match repository_root(&start) {
        Ok(root) => root,
        Err(e) => return refuse(&e.to_string()),
    };
    let outcome = match &contract_override {
        Some(path) => check_against(&root, path),
        None => check(&root),
    };
    let (register, refusals) = match outcome {
        Ok(outcome) => outcome,
        Err(e) => return refuse(&e.to_string()),
    };

    if refusals.is_empty() {
        println!(
            "OK  {} — {} approved, every other resolved crate is local or lgwks_std",
            root.display(),
            register.entries.len()
        );
        return ExitCode::SUCCESS;
    }

    eprintln!("REFUSED  {} — {} unapproved dependencies\n", root.display(), refusals.len());
    for refusal in &refusals {
        eprintln!("  {refusal}");
    }
    eprintln!(
        "\nEach one is a decision, not a paperwork step. Climb the ladder first \
         (`lgwks-gate tiers`);\nif the answer is still a dependency, \
         `lgwks-gate request <crate> <version>` prints the block."
    );
    if !register.enforce {
        eprintln!("\nNOTE  [policy] enforce = false, so builds still pass. This is adoption-only.");
        return ExitCode::SUCCESS;
    }
    ExitCode::from(2)
}

fn refuse(message: &str) -> ExitCode {
    eprintln!("REFUSED  {message}");
    ExitCode::from(2)
}

// ── request ─────────────────────────────────────────────────────────────────

fn run_request(krate: Option<&String>, version: Option<&String>) -> ExitCode {
    let (Some(krate), Some(version)) = (krate, version) else {
        eprint!("{USAGE}");
        return ExitCode::from(2);
    };
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

fn run_init(path: Option<PathBuf>) -> ExitCode {
    let start = path.unwrap_or_else(|| PathBuf::from("."));
    let root = match repository_root(&start) {
        Ok(root) => root,
        Err(e) => return refuse(&e.to_string()),
    };
    let target = root.join(CONTRACT_PATH);
    if target.exists() {
        return refuse(&format!("{} already exists; init will not overwrite it", target.display()));
    }
    if let Some(parent) = target.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return refuse(&format!("cannot create {}: {e}", parent.display()));
        }
    }
    if let Err(e) = std::fs::write(&target, STARTER) {
        return refuse(&format!("cannot write {}: {e}", target.display()));
    }
    // Parsing what was just written proves the starter is a valid contract
    // rather than a file that happens to exist.
    match Contract::parse(STARTER) {
        Ok(_) => {
            println!("wrote {}", target.display());
            println!("next: lgwks-gate check {}", root.display());
            ExitCode::SUCCESS
        }
        Err(e) => refuse(&format!("starter register does not parse: {e}")),
    }
}

// ── The ladder ──────────────────────────────────────────────────────────────

const LADDER: &str = "\
The admission ladder — climb it before writing an approval.
Rungs 0 to 5 need no approval at all, because they add no dependency edge.

  0  Drop the feature.            Is it worth having? Most aren't.
  1  Use std.                     Check first; std grew while you weren't looking.
  2  Use lgwks_std.               Already approved, already vendored, zero new edges.
  3  Add a module to lgwks_std.   ELIMINATE tier — small, well-understood algorithms
                                  where a reimplementation is less risk than a
                                  supply-chain edge. hex, base64, glob, uuid, time.
  4  Consolidate onto one.        CONSOLIDATE tier — the estate already has two
                                  crates doing this job. Pick one, wrap it once.
  5  Vendor the audited source.   VENDOR tier — cryptography and anything where a
                                  hand-rolled version is a security regression.
                                  Pinned source under vendor/, with PROVENANCE.md.
  6  Approve it as a boundary.    BOUNDARY tier — reimplementing it is a multi-year
                                  project of its own: tokio, serde, regex, syn,
                                  rusqlite, cap-std. Needs an entry in the register.

Rungs 5 and 6 are the only ones that produce a contract entry, which is why
`tier` admits only `vendor` and `boundary`.

An approval names what std cannot do — not what the crate is convenient for.
\"Show me the commit we need. Don't update for the sake of it.\"  — Mitchell Hashimoto
";
