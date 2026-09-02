//! `RandomBot` (P2 smoke test): random but mostly-legal commands from a
//! legality-weighted menu built from the observation. Its job is to shake
//! out crashes and state-machine holes, not to win: every menu entry is
//! legal per the CURRENT observation, so drops come only from same-round
//! races (an enemy capturing the same portal, a foreign link appearing
//! between observation and execution).

use arena_core::config::GameConfig;
use arena_core::map::MapDefinition;
use arena_core::observe::{AgentPrivate, Observation, PortalPublic};
use arena_core::types::{AgentId, Command, Coord, Faction, Owner, PortalId};

use crate::bot::{Bot, BotKind};
use crate::common::{self, CommandList, HackLedger, in_range, owners};
use crate::rng::BotRng;

pub struct RandomBot {
    own: Owner,
    enemy: Owner,
    rng: BotRng,
    ledger: HackLedger,
    cfg: GameConfig,
    /// Closed draw ranges for spawn/destination coordinates: `[0, dim]` has
    /// `dim + 1` values per axis.
    width_n: u32,
    height_n: u32,
}

impl RandomBot {
    pub fn new(faction: Faction, bot_seed: u64, cfg: &GameConfig) -> Self {
        let (own, enemy) = owners(faction);
        RandomBot {
            own,
            enemy,
            rng: BotRng::new(bot_seed, faction, BotKind::Random),
            ledger: HackLedger::new(),
            cfg: *cfg,
            width_n: cfg.map_width_cm as u32 + 1,
            height_n: cfg.map_height_cm as u32 + 1,
        }
    }

    /// One agent's legality-weighted menu: CAPTURE of every neutral portal
    /// in operation range, RECHARGE of every own damaged-enough portal in
    /// range with supply available, HACK of every in-range ledger-eligible
    /// portal, one ATTACK entry when any enemy portal is in range, at most
    /// one fully verified LINK, and always one in-bounds budget-limited
    /// random MOVE (the guaranteed `>= 1` legal fallback entry).
    fn menu(&mut self, obs: &Observation, me: &AgentPrivate) -> Vec<Command> {
        let round = obs.round;
        let range = self.cfg.portal_operation_range_cm;
        let mut menu: Vec<Command> = Vec::new();
        let mut enemy_in_range = false;
        let mut hackable: Vec<PortalId> = Vec::new();
        for p in obs.portals.iter() {
            if !in_range(me.position, p.position, range) {
                continue;
            }
            match p.owner {
                Owner::Neutral => menu.push(Command::Capture { portal: p.id }),
                owner if owner == self.enemy => enemy_in_range = true,
                _ => {
                    if me.supply >= self.cfg.recharge_cost_supply {
                        menu.push(Command::Recharge { portal: p.id });
                    }
                }
            }
            if self.ledger.eligible(round, &self.cfg, me.id, p.id) {
                hackable.push(p.id);
            }
        }
        for portal in hackable {
            menu.push(Command::Hack { portal });
        }
        if enemy_in_range {
            menu.push(Command::Attack);
        }
        // At most one fully verified link (first found ascending s then t).
        for s in obs.portals.iter() {
            if s.owner != self.own {
                continue;
            }
            if !in_range(me.position, s.position, range) {
                continue;
            }
            let mut targets: Vec<&PortalPublic> = obs
                .portals
                .iter()
                .filter(|t| {
                    t.id != s.id
                        && t.owner == self.own
                        && me.keys.get(&t.id).copied().unwrap_or(0) >= 1
                })
                .collect();
            targets.sort_by_key(|t| t.id);
            let legal = targets.iter().find(|t| {
                common::link_is_legal(obs, self.own, &self.cfg, me.position, &me.keys, s.id, t.id)
            });
            if let Some(t) = legal {
                menu.push(Command::Link {
                    source_portal: s.id,
                    target_portal: t.id,
                });
                break;
            }
        }
        // The always-legal fallback entry: a random in-bounds destination,
        // budget-stepped (the step stays on the in-bounds segment).
        let x = self.rng.uniform(self.width_n) as i32;
        let y = self.rng.uniform(self.height_n) as i32;
        menu.push(Command::Move {
            destination: common::step_toward(
                me.position,
                Coord::new(x, y),
                self.cfg.move_budget_cm,
            ),
        });
        menu
    }
}

impl Bot for RandomBot {
    fn kind(&self) -> BotKind {
        BotKind::Random
    }

    /// Uniform coordinates in the closed map bounds, drawn in roster order
    /// (spawn draws come first on the bot's stream, D53).
    fn choose_spawns(
        &mut self,
        _cfg: &GameConfig,
        _map: &MapDefinition,
        roster: &[AgentId],
    ) -> Vec<(AgentId, Coord)> {
        roster
            .iter()
            .map(|&agent| {
                let x = self.rng.uniform(self.width_n) as i32;
                let y = self.rng.uniform(self.height_n) as i32;
                (agent, Coord::new(x, y))
            })
            .collect()
    }

    fn submit_round(&mut self, obs: &Observation) -> Vec<arena_core::harness::AgentSubmission> {
        let round = obs.round;
        let mut submissions = Vec::with_capacity(obs.own_agents.len());
        for me in obs.own_agents.iter() {
            // Draw order per agent (frozen): destination x, destination y,
            // command count k = 1 + u(3), then k menu draws without
            // replacement.
            let menu = self.menu(obs, me);
            let k = 1 + self.rng.uniform(3) as usize;
            let mut items = menu;
            let mut commands = CommandList::new();
            let mut drawn = 0usize;
            while drawn < k {
                // Once a major was drawn, later draws cannot pick another
                // (the single Move entry is unique, so it needs no rule).
                let has_major = commands.has_major();
                items.retain(|c| !(has_major && c.is_major()));
                if items.is_empty() {
                    break;
                }
                let index = self.rng.uniform(items.len() as u32) as usize;
                let command = items.swap_remove(index);
                match &command {
                    Command::Move { .. } => commands.mv(command),
                    c if c.is_major() => commands.major(command),
                    Command::Hack { portal } => {
                        let portal = *portal;
                        // Eligibility was checked at menu build; only the
                        // draw decides whether the hack is actually emitted.
                        self.ledger.record(round, &self.cfg, me.id, portal);
                        commands.aux(command);
                    }
                    _ => commands.aux(command),
                }
                drawn += 1;
            }
            let built = commands.build();
            debug_assert!(
                !built.is_empty(),
                "the fallback Move entry guarantees at least one command"
            );
            submissions.push(crate::common::submission(me.id, built));
        }
        submissions
    }
}
