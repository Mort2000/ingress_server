//! sb-env-report: prints the current working directory and the sorted
//! environment (one `KEY=VALUE` per line). Lets tests verify the env
//! scrub + cwd WITHOUT shell forks (RLIMIT_NPROC is uid-wide, so fork
//! behavior is ambient-load-dependent; this fixture is deterministic).

#![forbid(unsafe_code)]

fn main() {
    match std::env::current_dir() {
        Ok(dir) => println!("{}", dir.display()),
        Err(error) => println!("CWD_ERROR={error}"),
    }
    let mut vars: Vec<String> = std::env::vars()
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    vars.sort();
    for line in vars {
        println!("{line}");
    }
}
