#![forbid(unsafe_code)]

// The crate root itself is clean; the audited tokens live in a module
// pulled in by #[path] from OUTSIDE src/.
#[path = "../hidden/evil.rs"]
mod evil;

fn main() {
    println!("{}", evil::dial());
}
