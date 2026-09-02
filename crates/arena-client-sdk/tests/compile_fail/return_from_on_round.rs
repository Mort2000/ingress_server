// error: lifetime may not live long enough
// Returning the zero-copy view from on_round is impossible: it cannot
// outlive the round context that produced it.
use arena_client_sdk::bot::ClientBot;
use arena_client_sdk::round::RoundContext;
use arena_client_sdk::ObservationView;

struct BadBot;

impl ClientBot for BadBot {
    fn choose_spawns(
        &mut self,
        ctx: &arena_client_sdk::SpawnContext<'_>,
    ) -> Vec<(arena_core::types::AgentId, arena_core::types::Coord)> {
        let _ = ctx;
        Vec::new()
    }

    fn on_round(&mut self, ctx: RoundContext<'_>) {
        let view = ctx.observation().unwrap();
        let escaped: ObservationView<'static> = steal(view);
        let _ = escaped;
    }
}

fn steal<'a>(view: ObservationView<'a>) -> ObservationView<'static> {
    view
}

fn main() {}
