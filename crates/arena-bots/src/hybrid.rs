//! `HybridBot` (P2, the intended default benchmark): fixed roles — the
//! initial trio builds the GreedyField plan (with per-agent station
//! offsets, so the builders stagger over the shared tour), every later
//! agent (all reinforcements included) runs the DestroyAnchor attack, any
//! agent below `HYBRID_HACK_SUPPLY` supply hacks an in-range portal first,
//! and when the faction's top own anchor dropped below 50% max hp last
//! round the nearest attacker recharges it instead of attacking this
//! round.

use arena_core::config::GameConfig;
use arena_core::event::EventKind;
use arena_core::harness::AgentSubmission;
use arena_core::map::MapDefinition;
use arena_core::observe::{AgentPrivate, Observation};
use arena_core::types::{AgentId, Command, Coord, Faction, Owner};

use crate::bot::{Bot, BotKind};
use crate::common::{
    self, CommandList, HackLedger, in_range, nearest_matching, owners, step_toward,
};
use crate::destroy_anchor::{attack_commands, attack_targets, nearest_agent_to, top_anchor};
use crate::greedy_field::FieldPlanner;
use crate::rng::BotRng;

/// Agents below this supply hack an in-range portal before anything else.
pub const HYBRID_HACK_SUPPLY: u32 = 3;

/// How many of the faction's lowest-id agents form the builder trio (with
/// the default `INITIAL_AGENTS_PER_FACTION` = 3 this is exactly the initial
/// trio; every later agent — reinforcements included — attacks).
pub const BUILDER_COUNT: usize = 3;

pub struct HybridBot {
    faction: Faction,
    own: Owner,
    cfg: GameConfig,
    planner: FieldPlanner,
    ledger: HackLedger,
    /// Frozen per-kind RNG key shape (D53); this bot draws no randomness.
    _rng: BotRng,
}

impl HybridBot {
    pub fn new(faction: Faction, bot_seed: u64, cfg: &GameConfig, map: &MapDefinition) -> Self {
        let (own, _) = owners(faction);
        HybridBot {
            faction,
            own,
            cfg: *cfg,
            planner: FieldPlanner::new(faction, cfg, map),
            ledger: HackLedger::new(),
            _rng: BotRng::new(bot_seed, faction, BotKind::Hybrid),
        }
    }

    /// The defense dispatch for this round: `(attacker, anchor)` when the
    /// top own anchor was attacked below 50% max hp last round — the
    /// `(dist2, id)`-nearest ATTACKER diverts to it. Builders never divert
    /// ("instead of attacking" selects among agents that would attack).
    fn defense_dispatch(
        &self,
        obs: &Observation,
        attackers: &[&AgentPrivate],
    ) -> Option<(AgentId, arena_core::types::PortalId)> {
        let anchor = top_anchor(obs, self.faction)?;
        let half = self.cfg.portal_max_hp as u64;
        let attacked_low = obs.events.iter().any(|event| {
            event.round + 1 == obs.round
                && matches!(
                    event.kind,
                    EventKind::PortalAttacked { portal, hp_after, .. }
                        if portal == anchor && (hp_after as u64) * 2 < half
                )
        });
        if !attacked_low || attackers.is_empty() {
            return None;
        }
        // The nearest ATTACKER diverts ("instead of attacking" selects
        // among agents that would attack; builders keep the field plan).
        nearest_agent_to(obs, &attackers, anchor).map(|agent| (agent, anchor))
    }
}

impl Bot for HybridBot {
    fn kind(&self) -> BotKind {
        BotKind::Hybrid
    }

    /// On the seed-triangle vertices (roster agent `i` on vertex `i % 3`);
    /// without a viable triangle, degrade to portal-id spawns.
    fn choose_spawns(
        &mut self,
        _cfg: &GameConfig,
        map: &MapDefinition,
        roster: &[AgentId],
    ) -> Vec<(AgentId, Coord)> {
        if let Some(triangle) = self.planner.seed_triangle() {
            return roster
                .iter()
                .enumerate()
                .map(|(i, &agent)| {
                    let portal = map
                        .portal(triangle[i % 3])
                        .expect("search only returns map portals");
                    (agent, portal.position)
                })
                .collect();
        }
        let count = map.len() as u32;
        roster
            .iter()
            .enumerate()
            .map(|(i, &agent)| {
                let portal = map
                    .portal(arena_core::types::PortalId((i as u32) % count))
                    .expect("dense portal ids");
                (agent, portal.position)
            })
            .collect()
    }

    fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission> {
        self.planner.refresh(obs);
        let round = obs.round;
        let enemy = self.faction.opposite();
        let builders: Vec<AgentId> = obs
            .own_agents
            .iter()
            .take(BUILDER_COUNT.min(obs.own_agents.len()))
            .map(|a| a.id)
            .collect();
        let attackers: Vec<&AgentPrivate> = obs
            .own_agents
            .iter()
            .filter(|a| !builders.contains(&a.id))
            .collect();
        let defense = self.defense_dispatch(obs, &attackers);
        let targets = attack_targets(obs, enemy, &attackers);

        let mut submissions = Vec::with_capacity(obs.own_agents.len());
        for (index, me) in obs.own_agents.iter().enumerate() {
            let mut commands = CommandList::new();

            // Any agent below the supply floor hacks an in-range portal
            // first (auxiliary; may ride along with the major below).
            if me.supply < HYBRID_HACK_SUPPLY {
                if let Some(portal) = nearest_matching(obs, me.position, |p| {
                    in_range(me.position, p.position, self.cfg.portal_operation_range_cm)
                }) {
                    if self.ledger.eligible(round, &self.cfg, me.id, portal) {
                        self.ledger.record(round, &self.cfg, me.id, portal);
                        commands.aux(Command::Hack { portal });
                    }
                }
            }

            if builders.contains(&me.id) {
                // Builder: the field plan with a per-agent station offset
                // (the trio staggers over the shared tour).
                let planned = self
                    .planner
                    .agent_commands(obs, me, index, &mut self.ledger);
                commands.merge(planned);
            } else if let Some((agent, anchor)) = defense
                && agent == me.id
            {
                // Defense dispatch: recharge the wounded anchor when the
                // position-or-key requirement and supply hold, else step
                // toward it.
                let view = common::portal(obs, anchor);
                let keyed = me.keys.get(&anchor).copied().unwrap_or(0) >= 1;
                if view.owner == self.own
                    && me.supply >= self.cfg.recharge_cost_supply
                    && (in_range(
                        me.position,
                        view.position,
                        self.cfg.portal_operation_range_cm,
                    ) || keyed)
                {
                    commands.major(Command::Recharge { portal: anchor });
                } else {
                    commands.mv(Command::Move {
                        destination: step_toward(
                            me.position,
                            view.position,
                            self.cfg.move_budget_cm,
                        ),
                    });
                }
            } else {
                // Attacker: the DestroyAnchor body (Hybrid's own supply
                // floor applies inside it).
                let attack = attack_commands(
                    obs,
                    me,
                    self.own,
                    enemy,
                    &self.cfg,
                    &mut self.ledger,
                    HYBRID_HACK_SUPPLY,
                    targets.get(&me.id).copied(),
                );
                commands.merge(attack);
            }

            if commands.is_empty() {
                continue;
            }
            submissions.push(common::submission(me.id, commands.build()));
        }
        submissions
    }
}
