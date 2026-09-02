//! `GreedyFieldBot` (P2): searches for the largest-area field currently
//! obtainable and plans keys and route for it (Field Builder role,
//! R-BALANCE-01) — the measuring tool for "is the pure big-triangle
//! strategy too strong?".
//!
//! Plan shape (the M4 FieldBot tour pattern, shared by the whole faction):
//! a cached triangle plus six tour stations — three capture+key stops
//! (portal ours AND the agent holds at least one key of it), then the three
//! triangle edges in tour order. Completed stations are skipped via public
//! observation facts; every agent walks the same list, so portal ownership
//! and existing links make progress idempotent and self-healing (a
//! neutralized vertex simply re-enters the tour).
//!
//! Replanning: a plan is COMPLETE when all three own edges exist, and
//! INVALID when a vertex became enemy-owned, a foreign link sits on a
//! planned pair, or a foreign link crosses a planned edge. Both trigger a
//! fresh max-area search over own-or-neutral vertices. When no viable
//! triangle exists the bot degrades to NearestPortal expansion — never
//! idle, never skip a map — retrying the search on a fixed round cadence.

use std::collections::{BTreeMap, BTreeSet};

use arena_core::config::GameConfig;
use arena_core::geometry::{area2, dist2, point_on_segment, segments_intersect};
use arena_core::harness::AgentSubmission;
use arena_core::map::MapDefinition;
use arena_core::observe::{AgentPrivate, LinkPublic, Observation, PortalPublic};
use arena_core::types::{AgentId, Command, Coord, Faction, LinkId, Owner, PortalId};

use crate::bot::{Bot, BotKind};
use crate::common::{
    self, CommandList, HackLedger, engage_target, in_range, link_is_legal, owners,
};
use crate::rng::BotRng;

/// Rounds between triangle re-searches while no viable plan exists
/// (deterministic, round-based; a failed search is not retried every round).
const FALLBACK_RETRY_INTERVAL: u32 = 25;

/// A cached field plan: the target triangle (stations are derived).
#[derive(Clone, Debug)]
pub(crate) struct FieldPlan {
    pub triangle: [PortalId; 3],
}

/// The six tour stations of a triangle: three capture+key stops, then the
/// three edges in tour order `(source, link target)`.
fn stations(triangle: &[PortalId; 3]) -> [(PortalId, Option<PortalId>); 6] {
    let [a, b, c] = *triangle;
    [
        (a, None),
        (b, None),
        (c, None),
        (a, Some(b)),
        (b, Some(c)),
        (c, Some(a)),
    ]
}

/// Max-area triangle search (the frozen definition):
/// candidate edges are own-or-neutral portal pairs within the closed
/// `MAX_LINK_DISTANCE` whose canonical pair has no link, whose closed
/// segment is clear of other portals, and which no existing link crosses
/// without a shared endpoint; a candidate triple needs all three edges.
/// Ranking is `area2` descending, then all-own-vertices preferred, then
/// ascending ids — deterministic on every map.
pub(crate) fn search_triangle(
    portals: &[PortalPublic],
    links: &[LinkPublic],
    own: Owner,
    max_link_distance_cm: u32,
) -> Option<[PortalId; 3]> {
    let max2 = (max_link_distance_cm as i64) * (max_link_distance_cm as i64);
    let pos = |id: PortalId| -> Coord {
        portals
            .iter()
            .find(|p| p.id == id)
            .expect("dense portal ids")
            .position
    };
    let link_ids: BTreeSet<LinkId> = links.iter().map(|l| l.id).collect();

    // One candidate edge: distance, duplicate pair, portal-clear segment,
    // crossing-free against every live link.
    let edge_ok = |a: PortalId, b: PortalId| -> bool {
        let (pa, pb) = (pos(a), pos(b));
        if dist2(pa, pb) > max2 {
            return false;
        }
        if link_ids.contains(&LinkId::new(a, b).expect("a != b")) {
            return false;
        }
        if portals
            .iter()
            .any(|p| p.id != a && p.id != b && point_on_segment(p.position, pa, pb))
        {
            return false;
        }
        !links.iter().any(|l| {
            let (u, v) = (pos(l.id.min), pos(l.id.max));
            let shared = l.id.contains(a) || l.id.contains(b);
            !shared && segments_intersect(pa, pb, u, v)
        })
    };

    let eligible: Vec<PortalId> = portals
        .iter()
        .filter(|p| p.owner == Owner::Neutral || p.owner == own)
        .map(|p| p.id)
        .collect();
    let eligible_set: BTreeSet<PortalId> = eligible.iter().copied().collect();

    // Adjacency over candidate edges (each pair enumerated once, a < b).
    let mut adjacency: BTreeMap<PortalId, BTreeSet<PortalId>> = BTreeMap::new();
    for (index, &a) in eligible.iter().enumerate() {
        for &b in eligible.iter().skip(index + 1) {
            if edge_ok(a, b) {
                adjacency.entry(a).or_default().insert(b);
                adjacency.entry(b).or_default().insert(a);
            }
        }
    }

    let own_count = |triple: &[PortalId; 3]| {
        triple
            .iter()
            .filter(|p| portals.iter().find(|q| q.id == **p).expect("dense").owner == own)
            .count()
    };

    let mut best: Option<([PortalId; 3], u64, usize)> = None; // (triple, area2, own vertices)
    for (&a, neighbors) in adjacency.iter() {
        for &b in neighbors
            .iter()
            .filter(|b| eligible_set.contains(b) && **b > a)
        {
            let common: Vec<PortalId> = neighbors
                .iter()
                .filter(|c| **c > b && adjacency.get(c).is_some_and(|n| n.contains(&b)))
                .copied()
                .collect();
            for c in common {
                let triple = [a, b, c];
                let triple_area = area2(pos(a), pos(b), pos(c));
                if triple_area == 0 {
                    continue;
                }
                let owns = own_count(&triple);
                let better = match &best {
                    None => true,
                    Some((best_triple, best_area, best_owns)) => {
                        (triple_area > *best_area)
                            || (triple_area == *best_area && owns > *best_owns)
                            || (triple_area == *best_area
                                && owns == *best_owns
                                && &triple < best_triple)
                    }
                };
                if better {
                    best = Some((triple, triple_area, owns));
                }
            }
        }
    }
    best.map(|(triple, _, _)| triple)
}

/// This round's viability of a cached plan.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PlanStatus {
    /// Some edge still missing and nothing blocks the plan.
    Active,
    /// All three own edges exist — time for the next triangle.
    Complete,
    /// A vertex is enemy-owned, a foreign link sits on a planned pair, or a
    /// foreign link crosses a planned edge.
    Invalid,
}

/// Shared field-planning core (also the builder half of `HybridBot`).
pub(crate) struct FieldPlanner {
    faction: Faction,
    own: Owner,
    cfg: GameConfig,
    /// The cached plan (last searched triangle, possibly stale).
    plan: Option<FieldPlan>,
    /// The plan validated for THIS round (`None` = fallback expansion).
    active: Option<FieldPlan>,
    next_search_round: u32,
}

impl FieldPlanner {
    /// Runs the spawn-time search over the frozen map (map owners as
    /// defined, no links) — also used to place spawns on the seed triangle.
    pub fn new(faction: Faction, cfg: &GameConfig, map: &MapDefinition) -> Self {
        let (own, _) = owners(faction);
        let portals: Vec<PortalPublic> = map
            .portals()
            .iter()
            .map(|p| PortalPublic {
                id: p.id,
                position: p.position,
                owner: p.owner,
                hp: p.hp,
            })
            .collect();
        let plan = search_triangle(&portals, &[], own, cfg.max_link_distance_cm)
            .map(|triangle| FieldPlan { triangle });
        FieldPlanner {
            faction,
            own,
            cfg: *cfg,
            plan: plan.clone(),
            active: plan,
            next_search_round: 1,
        }
    }

    /// The spawn-time seed triangle, if the map has a viable one.
    pub fn seed_triangle(&self) -> Option<[PortalId; 3]> {
        self.plan.as_ref().map(|p| p.triangle)
    }

    fn status(&self, plan: &FieldPlan, obs: &Observation) -> PlanStatus {
        let mut complete = true;
        for (source, target) in stations(&plan.triangle).iter() {
            let Some(target) = target else { continue };
            let link_id = LinkId::new(*source, *target).expect("distinct plan vertices");
            match obs.links.iter().find(|l| l.id == link_id) {
                Some(link) if link.faction == self.faction => continue, // edge done
                Some(_) => return PlanStatus::Invalid,                  // foreign link on our pair
                None => {
                    complete = false;
                    let owner_of = |id: PortalId| common::portal(obs, id).owner;
                    let enemy = owners(self.faction).1;
                    if owner_of(*source) == enemy || owner_of(*target) == enemy {
                        return PlanStatus::Invalid;
                    }
                    // A foreign link crossing this planned edge would DROP
                    // our LINK forever while it lives: replan instead.
                    let (ps, pt) = (
                        common::portal(obs, *source).position,
                        common::portal(obs, *target).position,
                    );
                    let crossed = obs.links.iter().any(|l| {
                        let (u, v) = (
                            common::portal(obs, l.id.min).position,
                            common::portal(obs, l.id.max).position,
                        );
                        let shared = l.id.contains(*source) || l.id.contains(*target);
                        !shared && segments_intersect(ps, pt, u, v)
                    });
                    if crossed {
                        return PlanStatus::Invalid;
                    }
                }
            }
        }
        if complete {
            PlanStatus::Complete
        } else {
            PlanStatus::Active
        }
    }

    /// Validates the cached plan against this round's observation and
    /// replans when it is complete or invalid. A SUCCESSFUL search always
    /// runs immediately (a completed or invalidated plan must never idle
    /// the faction); only a FAILED search is rate-limited to one retry per
    /// [`FALLBACK_RETRY_INTERVAL`] rounds — hopeless re-searches would
    /// otherwise dominate fallback rounds on link-blocked maps.
    pub fn refresh(&mut self, obs: &Observation) {
        let status = match &self.plan {
            Some(plan) => self.status(plan, obs),
            None => PlanStatus::Invalid,
        };
        match status {
            PlanStatus::Active => self.active = self.plan.clone(),
            PlanStatus::Complete | PlanStatus::Invalid => {
                let due = status == PlanStatus::Complete || obs.round >= self.next_search_round;
                if due {
                    match search_triangle(
                        &obs.portals,
                        &obs.links,
                        self.own,
                        self.cfg.max_link_distance_cm,
                    ) {
                        Some(plan) => {
                            self.plan = Some(FieldPlan { triangle: plan });
                            // Success resets the throttle: the next
                            // invalidation replans immediately again.
                            self.next_search_round = obs.round;
                        }
                        None => {
                            self.plan = None;
                            self.next_search_round = obs.round + FALLBACK_RETRY_INTERVAL;
                        }
                    }
                }
                self.active = match &self.plan {
                    Some(plan) if self.status(plan, obs) == PlanStatus::Active => {
                        Some(plan.clone())
                    }
                    _ => None,
                };
            }
        }
    }

    /// Whether this round has a usable plan (after `refresh`).
    pub fn has_plan(&self) -> bool {
        self.active.is_some()
    }

    /// One agent's commands on the shared station tour, starting from
    /// station `offset` (stations rotated; completed ones skipped through
    /// public facts). Empty output = the agent idles this round (waiting on
    /// teammates or hack cooldown); `Err(())`-free by design — plan-level
    /// invalidations are handled by `refresh` and station-level surprises
    /// surface as idle rounds, never illegal commands.
    pub fn agent_commands(
        &self,
        obs: &Observation,
        me: &AgentPrivate,
        offset: usize,
        ledger: &mut HackLedger,
    ) -> CommandList {
        let Some(plan) = &self.active else {
            return CommandList::new();
        };
        let stations = stations(&plan.triangle);
        let round = obs.round;
        for index in 0..stations.len() {
            let (stand, target) = stations[(index + offset) % stations.len()];
            let stand_view = common::portal(obs, stand);
            match target {
                None => {
                    // Capture + key stop: complete when the portal is ours
                    // AND this agent holds a key of it.
                    let keyed = me.keys.get(&stand).copied().unwrap_or(0) >= 1;
                    if stand_view.owner == self.own && keyed {
                        continue;
                    }
                    let mut commands = CommandList::new();
                    if me.position != stand_view.position {
                        commands.mv(Command::Move {
                            destination: common::step_toward(
                                me.position,
                                stand_view.position,
                                self.cfg.move_budget_cm,
                            ),
                        });
                    } else if stand_view.owner == Owner::Neutral {
                        commands.major(Command::Capture { portal: stand });
                    } else if stand_view.owner != self.own {
                        // Lost to the enemy between rounds; `refresh` will
                        // invalidate the plan next round. Idle now.
                    } else if ledger.eligible(round, &self.cfg, me.id, stand) {
                        ledger.record(round, &self.cfg, me.id, stand);
                        commands.aux(Command::Hack { portal: stand });
                    }
                    return commands;
                }
                Some(target) => {
                    // Edge stop: complete when OUR link exists.
                    let link_id = LinkId::new(stand, target).expect("distinct plan vertices");
                    if obs
                        .links
                        .iter()
                        .any(|l| l.id == link_id && l.faction == self.faction)
                    {
                        continue;
                    }
                    if obs.links.iter().any(|l| l.id == link_id) {
                        // Foreign link on the pair: plan dies next refresh.
                        return CommandList::new();
                    }
                    let target_view = common::portal(obs, target);
                    if stand_view.owner != self.own || target_view.owner != self.own {
                        // Vertices not ours (yet): capture stops and
                        // teammates finish first. Idle.
                        return CommandList::new();
                    }
                    if !in_range(
                        me.position,
                        stand_view.position,
                        self.cfg.portal_operation_range_cm,
                    ) {
                        let mut commands = CommandList::new();
                        commands.mv(Command::Move {
                            destination: common::step_toward(
                                me.position,
                                stand_view.position,
                                self.cfg.move_budget_cm,
                            ),
                        });
                        return commands;
                    }
                    if me.keys.get(&target).copied().unwrap_or(0) < 1 {
                        // Key consumed by an earlier rebuild: the capture
                        // stop will re-farm it on the next pass. Idle.
                        return CommandList::new();
                    }
                    if link_is_legal(
                        obs,
                        self.own,
                        &self.cfg,
                        me.position,
                        &me.keys,
                        stand,
                        target,
                    ) {
                        let mut commands = CommandList::new();
                        commands.aux(Command::Link {
                            source_portal: stand,
                            target_portal: target,
                        });
                        return commands;
                    }
                    // Blocked at execution time (race): idle; `refresh`
                    // invalidates the plan next round.
                    return CommandList::new();
                }
            }
        }
        // Every station complete for this agent (a teammate still works on
        // one): explicit idle.
        CommandList::new()
    }
}

pub struct GreedyFieldBot {
    own: Owner,
    cfg: GameConfig,
    planner: FieldPlanner,
    ledger: HackLedger,
    /// Frozen per-kind RNG key shape (D53); the algorithm itself draws no
    /// randomness.
    _rng: BotRng,
}

impl GreedyFieldBot {
    pub fn new(faction: Faction, bot_seed: u64, cfg: &GameConfig, map: &MapDefinition) -> Self {
        let (own, _) = owners(faction);
        GreedyFieldBot {
            own,
            cfg: *cfg,
            planner: FieldPlanner::new(faction, cfg, map),
            ledger: HackLedger::new(),
            _rng: BotRng::new(bot_seed, faction, BotKind::GreedyField),
        }
    }
}

impl Bot for GreedyFieldBot {
    fn kind(&self) -> BotKind {
        BotKind::GreedyField
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
                    .portal(PortalId((i as u32) % count))
                    .expect("dense portal ids");
                (agent, portal.position)
            })
            .collect()
    }

    fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission> {
        self.planner.refresh(obs);
        let round = obs.round;
        let own = self.own;
        let mut submissions = Vec::with_capacity(obs.own_agents.len());
        for me in obs.own_agents.iter() {
            let commands = if self.planner.has_plan() {
                // All agents walk the shared list in the same order (offset
                // 0): the M4 FieldBot pattern.
                self.planner.agent_commands(obs, me, 0, &mut self.ledger)
            } else {
                // No viable triangle: NearestPortal expansion — never idle,
                // never skip a map.
                match common::nearest_incomplete(obs, me, own) {
                    Some(target) => {
                        let target = common::portal(obs, target);
                        engage_target(me, target, own, &mut self.ledger, &self.cfg, round)
                    }
                    None => CommandList::new(),
                }
            };
            if commands.is_empty() {
                continue;
            }
            submissions.push(common::submission(me.id, commands.build()));
        }
        submissions
    }
}
