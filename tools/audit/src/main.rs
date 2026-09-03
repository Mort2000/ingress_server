//! audit CLI: runs the white-box audit over the participants of a
//! repository and exits 0 (PASS) / 1 (FAIL).
//!
//! ```text
//! audit [--root <repo>] [--participants bots/*] [--operators crates/*,tools/*]
//! ```

use std::path::PathBuf;

fn main() {
    let mut root = std::env::current_dir().expect("cwd");
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--root" => root = PathBuf::from(args.next().unwrap_or_default()),
            other => {
                eprintln!("unknown flag {other:?}");
                std::process::exit(1);
            }
        }
    }
    let participants = audit::participant_dirs(&root);
    let operators = audit::operator_dirs(&root);
    if participants.is_empty() {
        eprintln!(
            "audit: no participant crates found under {}",
            root.join("bots").display()
        );
        std::process::exit(1);
    }
    // The frozen allowlist must equal the real external closure.
    if let Err(mismatch) = audit::allowlist_matches_lock(&root) {
        eprintln!("audit: {mismatch}");
        std::process::exit(1);
    }
    let report = audit::run(&root, &participants, &operators);
    print!("{}", report.render());
    if !report.pass() {
        std::process::exit(1);
    }
}
