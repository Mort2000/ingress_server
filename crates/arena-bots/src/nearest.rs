//! `NearestPortalBot` (P2): measures whether basic spatial expansion holds.
//! Every agent targets the `(dist2, ascending id)`-nearest portal NOT owned
//! by us, steps toward it within `MOVE_BUDGET`, and once inside the closed
//! operation range CAPTURES it (neutral), ATTACKS (enemy), or HACKs it for
//! keys/supply (own, ledger-gated). Fully deterministic — no randomness
//! after construction.

use arena_core::config::GameConfig;
use arena_core::harness::AgentSubmission;
use arena_core::map::MapDefinition;
use arena_core::observe::Observation;
use arena_core::types::{AgentId, Coord, Faction};

use crate::bot::{Bot, BotKind};
use crate::common::{self, CommandList, HackLedger, engage_target, owners};
use crate::rng::BotRng;

pub struct NearestPortalBot {
    own: arena_core::types::Owner,
    cfg: GameConfig,
    ledger: HackLedger,
    /// Present for the frozen per-kind RNG key shape (D53); this bot draws
    /// no randomness — its algorithm is a pure function of the observation.
    _rng: BotRng,
}

impl NearestPortalBot {
    pub fn new(faction: Faction, bot_seed: u64, cfg: &GameConfig) -> Self {
        let (own, _enemy) = owners(faction);
        NearestPortalBot {
            own,
            cfg: *cfg,
            ledger: HackLedger::new(),
            _rng: BotRng::new(bot_seed, faction, BotKind::NearestPortal),
        }
    }
}

impl Bot for NearestPortalBot {
    fn kind(&self) -> BotKind {
        BotKind::NearestPortal
    }

    /// On chosen portals: roster agent `i` spawns on portal `i mod P`
    /// (portal ids are dense by map construction), immediately adjacent to
    /// expansion targets.
    fn choose_spawns(
        &mut self,
        cfg: &GameConfig,
        map: &MapDefinition,
        roster: &[AgentId],
    ) -> Vec<(AgentId, Coord)> {
        let count = map.len() as u32;
        roster
            .iter()
            .enumerate()
            .map(|(i, &agent)| {
                let portal = map
                    .portal(arena_core::types::PortalId((i as u32) % count))
                    .expect("dense portal ids");
                let _ = cfg;
                (agent, portal.position)
            })
            .collect()
    }

    fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission> {
        let round = obs.round;
        let own = self.own;
        let mut submissions = Vec::with_capacity(obs.own_agents.len());
        for me in obs.own_agents.iter() {
            // Nearest unfinished portal: not ours yet, or ours without a
            // key from this agent (the hack leg). Nothing to do when every
            // portal is captured and keyed.
            let Some(target) = common::nearest_incomplete(obs, me, own) else {
                continue;
            };
            let target = common::portal(obs, target);
            let commands: CommandList =
                engage_target(me, target, own, &mut self.ledger, &self.cfg, round);
            if commands.is_empty() {
                continue;
            }
            submissions.push(common::submission(me.id, commands.build()));
        }
        submissions
    }
}
