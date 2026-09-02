//! nearest-client: the frozen M6 `NearestPortalBot` wrapped as an external
//! Audited Client through the official SDK (D69). The algorithm is
//! UNCHANGED and version-pinned: the SDK view is deep-converted to an
//! arena-core `Observation` each round and handed to the arena-bots
//! strategy. `#![forbid(unsafe_code)]` per P4-CAPABILITY.

#![forbid(unsafe_code)]

use std::str::FromStr;

use arena_bots::nearest::NearestPortalBot;
use arena_bots::{Bot, BotKind};
use arena_client_sdk::bot::ClientBot;
use arena_client_sdk::round::{RoundContext, SpawnContext};
use arena_client_sdk::{ConnectOptions, RunSummary, run};
use arena_core::types::{AgentId, Coord};

/// Wraps one frozen arena-bots strategy as an external SDK client.
struct SdkBot<Client> {
    inner: Client,
    rounds_played: u32,
}

impl<Client> SdkBot<Client> {
    fn new(inner: Client) -> Self {
        SdkBot {
            inner,
            rounds_played: 0,
        }
    }
}

impl<Client> ClientBot for SdkBot<Client>
where
    Client: Bot,
{
    fn choose_spawns(&mut self, ctx: &SpawnContext<'_>) -> Vec<(AgentId, Coord)> {
        let map = ctx
            .map_view()
            .expect("the frozen map is readable in the spawn phase")
            .to_map_definition()
            .expect("the map view decodes");
        self.inner.choose_spawns(ctx.config(), &map, &ctx.roster())
    }

    fn on_round(&mut self, mut ctx: RoundContext<'_>) {
        self.rounds_played += 1;
        let view = ctx
            .observation()
            .expect("the round observation is licensed");
        let obs = view
            .to_observation()
            .expect("the snapshots decode into an Observation");
        for submission in self.inner.submit_round(&obs) {
            let _ = ctx.submit(submission.agent_id, submission.commands);
        }
        // Early completion: the server advances as soon as both factions
        // finish (P3-TIMEOUT); exercising commit is part of the contract.
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
    let name = format!("nearest-client-{seed}");

    let options = ConnectOptions::new(&socket, name);
    let result = run(
        |faction, cfg, _map| SdkBot::new(NearestPortalBot::new(faction, seed, cfg)),
        options,
    );
    let _ = BotKind::NearestPortal; // strategy identity is logged via name
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
