//! sb-malloc-bomb: grows virtual allocations until the RLIMIT_AS cap
//! aborts the allocator. Exits 42 only if 8 GiB of address space was
//! granted without failure (i.e. NO limit was enforced).

#![forbid(unsafe_code)]

fn main() {
    let chunk = 1 << 20; // 1 MiB, never touched: address space only
    let mut held: Vec<Vec<u8>> = Vec::new();
    let mut total = 0usize;
    while total < 8 << 30 {
        held.push(Vec::with_capacity(chunk));
        total += chunk;
    }
    // Reaching this line means the allocation never failed: the
    // sandbox did not enforce an address-space limit.
    std::process::exit(42);
}
