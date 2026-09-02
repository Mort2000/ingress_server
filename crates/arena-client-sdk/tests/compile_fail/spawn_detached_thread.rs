// error-code: E0521
// Handing the view to a DETACHED thread (std::thread::spawn needs
// 'static) is impossible; scoped threads remain usable and join.
use arena_client_sdk::bot::ClientBot;
use arena_client_sdk::round::RoundContext;

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
        // The view borrows the round context; a detached thread needs
        // 'static. This must not compile.
        let handle = std::thread::spawn(move || {
            let _ = view.round();
        });
        let _ = handle;
        let _ = ctx;
    }
}

fn main() {}
