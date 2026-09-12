//! Minimal deterministic, stateful development template, not a competitive bot.
#![forbid(unsafe_code)]
use arena_client_sdk::{
    ConnectOptions, SdkError,
    strategy::{Decision, LiveStrategy, Strategy, Trace},
};
use arena_core::{
    config::GameConfig,
    map::MapDefinition,
    observe::Observation,
    types::{Command, Faction},
};

struct Example {
    calls: u32,
    faction: Faction,
    config: GameConfig,
}
impl Example {
    fn new(faction: Faction, config: &GameConfig, _: &MapDefinition) -> Self {
        Self {
            calls: 0,
            faction,
            config: *config,
        }
    }
}
impl Strategy for Example {
    fn decide(&mut self, obs: &Observation, turn: &mut dyn Decision) -> Result<(), SdkError> {
        self.calls += 1;
        for (i, agent) in obs.own_agents.iter().enumerate() {
            // Submit a cheap action first. More expensive planning could follow
            // before the next Agent submission; the adapter does not buffer it.
            let commands = if i == 0
                && !obs.portals.is_empty()
                && self.config.max_commands_per_agent_round > 0
            {
                vec![if self.calls == 1 {
                    Command::Capture {
                        portal: obs.portals[0].id,
                    }
                } else {
                    Command::Hack {
                        portal: obs.portals[0].id,
                    }
                }]
            } else {
                vec![]
            };
            turn.trace(Trace {
                agent: agent.id,
                task: format!("{:?}-step-{}", self.faction, self.calls),
                target: "first-portal".into(),
                value: 0,
                elapsed_us: 0,
            })?;
            turn.submit(agent.id, commands)?;
        }
        turn.commit()
    }
    fn on_timing(&mut self, timing: &arena_client_sdk::RoundTiming) {
        eprintln!("ARENA_TIMING {}", timing.to_json());
    }
    fn on_diagnostics(&mut self, round: u32, budgets: &[u64], trace: &[Trace]) {
        eprintln!("ARENA_TRACE round={round} budgets_ms={budgets:?} trace={trace:?}");
    }
    fn on_feedback(&mut self, f: &arena_client_sdk::SubmissionFeedback) {
        eprintln!(
            "ARENA_RECEIPT round={} agent={} status={:?} detail={:?}",
            f.round, f.agent.0, f.status, f.detail
        );
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if arena_client_sdk::offline::run_cli(&args, Example::new)? {
        return Ok(());
    }
    if args.len() != 2 {
        return Err("SOCKET CLIENT_SEED or --arena-offline SCENARIO_DIR NEW_OUTPUT.json".into());
    }
    // This example is deliberately non-random; a random strategy must pass
    // and record this seed explicitly in both live and offline initialization.
    let _seed: u64 = args[1].parse()?;
    arena_client_sdk::run(
        |faction, cfg, map| {
            let n = cfg.initial_agents_per_faction;
            let offset = if faction == Faction::Enl { 0 } else { n };
            let position = map
                .portals()
                .first()
                .map(|p| p.position)
                .unwrap_or(arena_core::types::Coord::new(0, 0));
            let spawns = (offset..offset + n)
                .map(|id| (arena_core::types::AgentId(id), position))
                .collect();
            LiveStrategy::new(Example::new(faction, cfg, map), spawns)
        },
        ConnectOptions::new(&args[0], "iteration-example")
            .with_bot_version(env!("CARGO_PKG_VERSION")),
    )?;
    Ok(())
}
