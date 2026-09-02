// error-code: E0106
// Storing an ObservationView in long-lived bot state is impossible: the
// view borrows the round context and has no lifetime of its own (P3-RPC
// SDK contract; ADR-0006).
use arena_client_sdk::ObservationView;

struct BotState {
    saved: ObservationView<'_>,
}

fn main() {
    let _ = std::mem::size_of::<BotState>();
}
