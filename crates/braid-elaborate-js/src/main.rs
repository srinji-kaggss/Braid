//! `braid-elaborate-js "<js expression>"` — the human-reconstructable loop for
//! the JS frontend: prints the capsule CID, the verifier verdict, and the
//! CID-bound manifest. Mirrors `braid-cli`'s shape so the SDK/CLI parity
//! doctrine holds (no path that only the library can drive — T13).

use std::process::ExitCode;

use braid_elaborate_js::elaborate_and_admit;
use braid_verify::Verdict;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: braid-elaborate-js \"<js expression>\"");
        eprintln!("example: braid-elaborate-js '\"hello\" + \"world\"'");
        return ExitCode::from(2);
    }
    let src = args.join(" ");

    match elaborate_and_admit(&src) {
        Ok(e) => {
            println!("source : {src}");
            println!("cid    : {}", e.capsule.cid().to_hex());
            match &e.verdict {
                Verdict::Admit { capsule_cid } => {
                    println!("verdict: ADMIT ({})", capsule_cid.to_hex());
                }
                Verdict::Reject { stage, reason } => {
                    println!("verdict: REJECT [{stage:?}] {reason}");
                }
            }
            println!("\n--- manifest ---\n{}", e.manifest_text);
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("elaboration error: {err}");
            ExitCode::FAILURE
        }
    }
}
