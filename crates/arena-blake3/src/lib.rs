//! arena-blake3: dependency-free BLAKE3 (hash, keyed hash, derive_key, XOF).
//! Portable reference implementation; M1 fills in the algorithm and vectors.

#![forbid(unsafe_code)]

/// Placeholder to keep the workspace compiling before M1 lands.
pub fn workspace_ready() -> bool {
    true
}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_compiles() {
        assert!(super::workspace_ready());
    }
}
