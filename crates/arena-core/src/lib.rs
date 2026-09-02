//! arena-core: authoritative game model and deterministic settlement for
//! Ingress Arena v0.1. Rule semantics come from `.scratch/ingress_arena_v0.1/rules/`;
//! this crate must not contain a second copy of the rules.

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
