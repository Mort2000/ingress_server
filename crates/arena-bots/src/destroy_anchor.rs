//! `DestroyAnchorBot` (P2 / Disruptor role, R-BALANCE-01): computes, for
//! every enemy portal, the summed area of enemy fields containing it
//! (`sum(Area(field containing p))` — the Anchor value estimate) and
//! converges every agent on the argmax anchor (ties by ascending id). With
//! no enemy fields the bot degrades to expansion with deterministic
//! claim-spreading: agents (ascending id) each target the nearest enemy
//! portal not already targeted this round, else the nearest unclaimed
//! neutral portal — capture races inside one faction are a bug, not a
//! strategy. The lowest-id agent standing in range of a damaged own portal
//! recharges it in passing, and any agent low on supply hacks first. Also
//! the attacker half of `HybridBot`.

use std::collections::{BTreeMap, BTreeSet};

use arena_core::config::GameConfig;
use arena_core::geometry::dist2;
use arena_core::harness::AgentSubmission;
use arena_core::map::MapDefinition;
use arena_core::observe::{AgentPrivate, Observation, PortalPublic};
use arena_core::types::{AgentId, Command, Coord, Faction, Owner, PortalId};

use crate::bot::{Bot, BotKind};
use crate::common::{
    self, CommandList, HackLedger, in_range, nearest_matching, owners, step_toward,
};
use crate::rng::BotRng;

/// Supply threshold below which a DestroyAnchor attacker HACKs first
/// (auxiliary, may ride along with the round's major action).
pub const ATTACKER_HACK_SUPPLY: u32 = 10;

/// `anchor_value(p) = sum of area2 of the fields of `fields_of` containing
/// p` (portals carrying nothing are absent).
pub(crate) fn anchor_values(obs: &Observation, fields_of: Faction) -> BTreeMap<PortalId, u64> {
    let mut values: BTreeMap<PortalId, u64> = BTreeMap::new();
    for field in obs.fields.iter().filter(|f| f.faction == fields_of) {
        for portal in [field.id.a, field.id.b, field.id.c] {
            *values.entry(portal).or_insert(0) += field.area2;
        }
    }
    values
}

/// The argmax anchor portal of `fields_of`'s fields (value > 0, ties
/// ascending id).
pub(crate) fn top_anchor(obs: &Observation, fields_of: Faction) -> Option<PortalId> {
    anchor_values(obs, fields_of)
        .into_iter()
        // Value descending, then ascending id (the composed comparator is a
        // strict total order over the ascending BTreeMap iteration).
        .max_by(|a, b| (a.1, b.0).cmp(&(b.1, a.0)))
        .filter(|(_, value)| *value > 0)
        .map(|(portal, _)| portal)
}

/// The round's per-agent attack targets: with an enemy anchor every agent
/// converges on it; otherwise expansion with claim-spreading — agents in
/// ascending order each claim the `(dist2, id)`-nearest unclaimed enemy
/// portal, falling back to the nearest unclaimed neutral portal.
pub(crate) fn attack_targets(
    obs: &Observation,
    enemy: Faction,
    agents: &[&AgentPrivate],
) -> BTreeMap<AgentId, PortalId> {
    let enemy_owner = owners(enemy).0;
    if let Some(anchor) = top_anchor(obs, enemy) {
        return agents.iter().map(|a| (a.id, anchor)).collect();
    }
    let mut claimed: BTreeSet<PortalId> = BTreeSet::new();
    let mut targets = BTreeMap::new();
    for agent in agents {
        let target = nearest_matching(obs, agent.position, |p: &PortalPublic| {
            p.owner == enemy_owner && !claimed.contains(&p.id)
        })
        .or_else(|| {
            nearest_matching(obs, agent.position, |p: &PortalPublic| {
                p.owner == Owner::Neutral && !claimed.contains(&p.id)
            })
        });
        if let Some(target) = target {
            claimed.insert(target);
            targets.insert(agent.id, target);
        }
    }
    targets
}

/// The shared attacker body (DestroyAnchorBot and HybridBot's attackers):
/// optional supply hack first, then the defense-in-passing recharge, else
/// engage this agent's assigned target (see [`attack_targets`]).
pub(crate) fn attack_commands(
    obs: &Observation,
    me: &AgentPrivate,
    own: Owner,
    enemy: Faction,
    cfg: &GameConfig,
    ledger: &mut HackLedger,
    hack_below_supply: u32,
    target: Option<PortalId>,
) -> CommandList {
    let round = obs.round;
    let enemy_owner = owners(enemy).0;
    let mut commands = CommandList::new();

    // Supply first: hack the nearest in-range ledger-eligible portal.
    if me.supply < hack_below_supply {
        if let Some(portal) = nearest_matching(obs, me.position, |p| {
            in_range(me.position, p.position, cfg.portal_operation_range_cm)
        }) {
            if ledger.eligible(round, cfg, me.id, portal) {
                ledger.record(round, cfg, me.id, portal);
                commands.aux(Command::Hack { portal });
            }
        }
    }

    // Defense in passing: the lowest-id own agent standing in range of a
    // damaged own portal (and holding supply) recharges it instead of
    // attacking this round. Every agent computes the same roster decision
    // from public hp values plus the faction's own agent views.
    if let Some((agent, portal)) = lowest_id_recharge_candidate(obs, own, cfg) {
        if agent == me.id {
            if me.supply >= cfg.recharge_cost_supply {
                commands.major(Command::Recharge { portal });
            }
            // Out of supply: no major this round (the hack above may
            // recover some for later rounds).
            return commands;
        }
    }

    let Some(target) = target else {
        return commands; // no assignment: nothing to attack or capture
    };
    let target = common::portal(obs, target);
    if in_range(me.position, target.position, cfg.portal_operation_range_cm) {
        match target.owner {
            Owner::Neutral => commands.major(Command::Capture { portal: target.id }),
            owner if owner == enemy_owner => commands.major(Command::Attack),
            _ => {} // own target (a teammate captured it this round): idle
        }
    } else {
        commands.mv(Command::Move {
            destination: step_toward(me.position, target.position, cfg.move_budget_cm),
        });
    }
    commands
}

/// The lowest-id own agent that stands in range of at least one own portal
/// below max hp and holds enough supply, with the lowest-id such portal
/// (None when nobody qualifies).
fn lowest_id_recharge_candidate(
    obs: &Observation,
    own: Owner,
    cfg: &GameConfig,
) -> Option<(AgentId, PortalId)> {
    let max_hp = cfg.portal_max_hp;
    let damaged_in_range = |agent: &AgentPrivate| -> Option<PortalId> {
        obs.portals
            .iter()
            .filter(|p| p.owner == own && p.hp < max_hp)
            .find(|p| in_range(agent.position, p.position, cfg.portal_operation_range_cm))
            .map(|p| p.id)
    };
    obs.own_agents
        .iter()
        .filter(|agent| agent.supply >= cfg.recharge_cost_supply)
        .find_map(|agent| damaged_in_range(agent).map(|portal| (agent.id, portal)))
}

/// The `(dist2, ascending id)`-nearest agent of `agents` to `portal`.
pub(crate) fn nearest_agent_to(
    obs: &Observation,
    agents: &[&AgentPrivate],
    portal: PortalId,
) -> Option<AgentId> {
    let position = common::portal(obs, portal).position;
    agents
        .iter()
        .map(|a| (dist2(a.position, position), a.id.0))
        .min()
        .map(|(_, id)| AgentId(id))
}

pub struct DestroyAnchorBot {
    faction: Faction,
    own: Owner,
    cfg: GameConfig,
    ledger: HackLedger,
    /// Frozen per-kind RNG key shape (D53); this bot draws no randomness.
    _rng: BotRng,
}

impl DestroyAnchorBot {
    pub fn new(faction: Faction, bot_seed: u64, cfg: &GameConfig) -> Self {
        let (own, _) = owners(faction);
        DestroyAnchorBot {
            faction,
            own,
            cfg: *cfg,
            ledger: HackLedger::new(),
            _rng: BotRng::new(bot_seed, faction, BotKind::DestroyAnchor),
        }
    }
}

impl Bot for DestroyAnchorBot {
    fn kind(&self) -> BotKind {
        BotKind::DestroyAnchor
    }

    /// Integer portal centroid, clamped into the closed map bounds (all
    /// agents share it — overlapping coordinates are legal, R-AGENT-01).
    fn choose_spawns(
        &mut self,
        cfg: &GameConfig,
        map: &MapDefinition,
        roster: &[AgentId],
    ) -> Vec<(AgentId, Coord)> {
        let portals = map.portals();
        let count = portals.len() as i64;
        let sum_x: i64 = portals.iter().map(|p| p.position.x as i64).sum();
        let sum_y: i64 = portals.iter().map(|p| p.position.y as i64).sum();
        let clamp = |value: i64, dim: i32| value.clamp(0, dim as i64) as i32;
        let centroid = Coord::new(
            clamp(sum_x / count, cfg.map_width_cm),
            clamp(sum_y / count, cfg.map_height_cm),
        );
        roster.iter().map(|&agent| (agent, centroid)).collect()
    }

    fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission> {
        let (own, enemy) = (self.own, self.faction.opposite());
        let roster: Vec<&AgentPrivate> = obs.own_agents.iter().collect();
        let targets = attack_targets(obs, enemy, &roster);
        let mut submissions = Vec::with_capacity(obs.own_agents.len());
        for me in obs.own_agents.iter() {
            let commands = attack_commands(
                obs,
                me,
                own,
                enemy,
                &self.cfg,
                &mut self.ledger,
                ATTACKER_HACK_SUPPLY,
                targets.get(&me.id).copied(),
            );
            if commands.is_empty() {
                continue;
            }
            submissions.push(common::submission(me.id, commands.build()));
        }
        submissions
    }
}
