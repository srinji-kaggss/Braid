//! CLI adapter for the admitted-JS-expression loop.

use std::process::ExitCode;

use crate::elaborate_and_admit;
use braid_verify::Verdict;

/// Run the executable loop: validate argv, admit source, then print its CID,
/// verdict, and manifest.
pub fn run(args: &[String]) -> ExitCode {
    let Some(src) = args.first() else {
        eprintln!("usage: braid-elaborate-js \"<js expression>\"");
        eprintln!("example: braid-elaborate-js '\"hello\" + \"world\"'");
        return ExitCode::from(2);
    };

    match elaborate_and_admit(src) {
        Ok(elaboration) => {
            println!("source : {src}");
            println!("cid    : {}", elaboration.capsule.cid().to_hex());
            match &elaboration.verdict {
                Verdict::Admit { capsule_cid } => {
                    println!("verdict: ADMIT ({})", capsule_cid.to_hex());
                }
                Verdict::Reject { stage, reason } => {
                    println!("verdict: REJECT [{stage:?}] {reason}");
                }
            }
            println!("\n--- manifest ---\n{}", elaboration.manifest_text);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("elaboration error: {error}");
            ExitCode::FAILURE
        }
    }
}
