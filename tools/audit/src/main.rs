//! Audit independently checked-out participants against the official SDK workspace.
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str =
    "Usage: audit [--root <server-repo>] --participant <bot-repo> [--participant <another-repo>]";

fn execute() -> Result<bool, String> {
    let mut root = std::env::current_dir().map_err(|error| error.to_string())?;
    let mut participants = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        if flag == "--help" || flag == "-h" {
            println!("{USAGE}");
            return Ok(true);
        }
        let value = args
            .next()
            .ok_or_else(|| format!("{flag} requires a path"))?;
        match flag.as_str() {
            "--root" => root = PathBuf::from(value),
            "--participant" => participants.push(PathBuf::from(value)),
            _ => return Err(format!("unknown option {flag:?}\n{USAGE}")),
        }
    }
    if participants.is_empty() {
        return Err(USAGE.into());
    }
    let root = root.canonicalize().map_err(|error| error.to_string())?;
    audit::allowlist_matches_lock(&root)?;
    for participant in &mut participants {
        *participant = participant
            .canonicalize()
            .map_err(|error| error.to_string())?;
        audit::participant_lock_is_allowed(participant)?;
    }
    let report = audit::run(&root, &participants, &audit::operator_dirs(&root));
    print!("{}", report.render());
    Ok(report.pass())
}

fn main() -> ExitCode {
    match execute() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("audit: {error}");
            ExitCode::FAILURE
        }
    }
}
