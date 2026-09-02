//! hybrid-client: the frozen M6 `HybridBot` wrapped as an external Audited
//! Client through the official SDK (D69). Algorithm unchanged and
//! version-pinned; the SDK view is deep-converted to an arena-core
//! `Observation` each round. `#![forbid(unsafe_code)]` per P4-CAPABILITY.

#![forbid(unsafe_code)]

use std::str::FromStr;

use arena_bots::hybrid::HybridBot;
use arena_bots::{Bot, BotKind};
use arena_client_sdk::bot::ClientBot;
use arena_client_sdk::round::{RoundContext, SpawnContext};
use arena_client_sdk::{ConnectOptions, RunSummary, run};
use arena_core::map::MapDefinition;
use arena_core::types::{AgentId, Coord, Faction};

/// Wraps one frozen arena-bots strategy as an external SDK client.
struct SdkBot {
    inner: HybridBot,
}

impl ClientBot for SdkBot {
    fn choose_spawns(&mut self, ctx: &SpawnContext<'_>) -> Vec<(AgentId, Coord)> {
        let map: MapDefinition = ctx
            .map_view()
            .expect("the frozen map is readable in the spawn phase")
            .to_map_definition()
            .expect("the map view decodes");
        self.inner.choose_spawns(ctx.config(), &map, &ctx.roster())
    }

    fn on_round(&mut self, mut ctx: RoundContext<'_>) {
        let view = ctx
            .observation()
            .expect("the round observation is licensed");
        let obs = view
            .to_observation()
            .expect("the snapshots decode into an Observation");
        for submission in self.inner.submit_round(&obs) {
            let _ = ctx.submit(submission.agent_id, submission.commands);
        }
        let _ = ctx.commit();
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let socket = args
        .next()
        .unwrap_or_else(|| "/tmp/ingress-arena.sock".into());
    let seed = args
        .next()
        .and_then(|s| u64::from_str(&s).ok())
        .unwrap_or(0);
    let name = format!("hybrid-client-{seed}");

    let options = ConnectOptions::new(&socket, name);
    let result = run(
        |faction: Faction, cfg, map: &MapDefinition| SdkBot {
            inner: HybridBot::new(faction, seed, cfg, map),
        },
        options,
    );
    let _ = BotKind::Hybrid;
    match result {
        Ok(RunSummary::Normal {
            outcome,
            rounds_completed,
            score_enl,
            score_res,
        }) => {
            println!("outcome={outcome} rounds={rounds_completed} enl={score_enl} res={score_res}");
        }
        Ok(RunSummary::Aborted { outcome, detail }) => {
            println!("aborted outcome={outcome} detail={detail:?}");
            std::process::exit(3);
        }
        Err(error) => {
            eprintln!("client error: {error}");
            std::process::exit(2);
        }
    }
}
