//! The coordinated builder core (plan sections 5.1 and 6.1): the engine
//! `greedy-coordinated-r2` is and every other R2 policy builds on. It
//! fixes the GreedyField self-competition root cause (B0: 73.6% of
//! GreedyField's 856k drops were `CaptureTargetNotNeutral` because every
//! agent walked the SAME six-station tour in the SAME order): stations of
//! the chosen field plan become one-shot [`Task`]s in the shared
//! allocator, so each round assigns DISTINCT stations to DISTINCT agents
//! with per-round target reservation.
//!
//! Decision discipline:
//!
//! - the plan triangle is the near-equal seeded pick over the value-ranked
//!   candidates ([`crate::r2::value::rank_field_candidates`]) — mirror
//!   factions draw different triangles and spawn sets;
//! - a SECOND non-crossing candidate (r2.1,
//!   [`CoreParams::second_plan_discount_permille`]) feeds the same
//!   allocator at a discounted priority, so the agents one triangle cannot
//!   employ — everyone past its six stations, reinforcements included —
//!   build the next-best field instead of idling (the 2026-09-05 action
//!   audit's task-pool finding: 66–68% no-task agent-rounds);
//! - replanning mirrors the frozen `FieldPlanner` semantics: COMPLETE (all
//!   three own links exist) and INVALID (enemy-held vertex, foreign link
//!   on a planned pair, foreign link crossing a planned edge) replan
//!   immediately; only a FAILED search is throttled by
//!   [`CoreParams::replan_retry_rounds`];
//! - the fallback (no viable triangle anywhere) spreads claims over the
//!   nearest incomplete portals through the SAME allocator and keeps
//!   opportunistic linking alive, so a policy never idles a map;
//! - compute stays bounded (plan section 15) by decision structure rather
//!   than a geometry cache: the value model reads portal positions straight
//!   from the observation, and the expensive candidate re-ranking runs only
//!   on replan (immediate on completion/invalidation, throttled by
//!   [`CoreParams::replan_retry_rounds`] on failure) — never per round.
//!
//! Every threshold lives in the recorded [`CoreParams`] (plan 6.3).

use std::collections::BTreeMap;

use arena_core::config::GameConfig;
use arena_core::map::MapDefinition;
use arena_core::observe::Observation;
use arena_core::types::{AgentId, Coord, Faction, PortalId};

use crate::common::{self, CommandList, HackLedger, in_range, owners, step_toward};
use crate::r2::rng::{R2Rng, R2Stream};
use crate::r2::sample::pick_near_equal;
use crate::r2::tasks::{Task, TaskBoard, TaskRequest, allocate, allocate_compatible};
use crate::r2::value::{
    Economy, FieldCandidate, ValueParams, field_candidates, field_value, rank_field_candidates,
    travel_rounds_to,
};

/// The recorded core thresholds of every R2 build policy (plan 6.3).
///
/// [`CoreParams::tag`] serializes every field — the core's own keys, then
/// the full [`ValueParams`] record after a `|` separator — so
/// `policies.tsv` carries the complete parameter set and the record
/// round-trips canonically.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CoreParams {
    /// Near-equal band (permille of the best value) the seeded top-candidate
    /// draw samples from — the tie-break that keeps mirror matches from
    /// colliding on the identical plan (plan 10.1).
    pub near_equal_permille: u32,
    /// Rounds between re-searches while NO viable triangle exists (a failed
    /// search is not retried every round; successful replans are immediate).
    pub replan_retry_rounds: u32,
    /// Priority of an opportunistic fallback LINK (score2-equivalents): any
    /// positive value means "better than walking", while staying far below
    /// every real build value so fallback work never preempts a live plan.
    pub fallback_priority: i64,
    /// Seeded spawn jitter (cm): each roster agent spawns at its plan
    /// vertex plus a faction-keyed random offset in
    /// `[-jitter..=jitter] x [-jitter..=jitter]` (clamped in bounds).
    /// The station assignment is unchanged — every agent stays inside the
    /// portal operation range — but two mirror factions of one policy can
    /// never collide on the identical Initial Spawn Set when the map's
    /// best triangle strictly dominates the near-equal band (plan section
    /// 10.1 mirror gate; the seeded tie-break alone cannot diverge a
    /// singleton band).
    pub spawn_jitter_cm: i32,
    /// Priority discount (permille) of the SECOND plan's tasks (r2.1): the
    /// surplus agents a single triangle cannot employ — everyone past its
    /// six stations, reinforcements included — work the next-best
    /// non-crossing candidate at `field_value * discount / 1000`, so the
    /// primary plan still finishes first while idle agents build income
    /// (the 2026-09-05 action audit: 66–68% of defender/blocker agent-rounds
    /// carried no task against the disruptor's 10%). `0` disables the
    /// second plan (the r2.0 behavior, also how pre-r2.1 parameter records
    /// parse back).
    pub second_plan_discount_permille: u32,
    /// The shared value model's thresholds (inlined into the record).
    pub value: ValueParams,
}

impl Default for CoreParams {
    fn default() -> Self {
        CoreParams {
            near_equal_permille: 100,
            replan_retry_rounds: 25,
            fallback_priority: 1,
            spawn_jitter_cm: 1_000,
            second_plan_discount_permille: 750,
            value: ValueParams::default(),
        }
    }
}

const CORE_PARAM_KEYS: [&str; 5] = ["nearequal", "retry", "fbpri", "jit", "sec2p"];

impl CoreParams {
    /// Canonical serialization:
    /// `nearequal=..;retry=..;fbpri=..;jit=..;sec2p=..|value=<ValueParams::tag>`.
    pub fn tag(&self) -> String {
        let values = [
            self.near_equal_permille.to_string(),
            self.replan_retry_rounds.to_string(),
            self.fallback_priority.to_string(),
            self.spawn_jitter_cm.to_string(),
            self.second_plan_discount_permille.to_string(),
        ];
        let mut tag = String::new();
        for (key, value) in CORE_PARAM_KEYS.iter().zip(values) {
            if !tag.is_empty() {
                tag.push(';');
            }
            tag.push_str(key);
            tag.push('=');
            tag.push_str(&value);
        }
        tag.push_str("|value=");
        tag.push_str(&self.value.tag());
        tag
    }

    /// Parses [`CoreParams::tag`] back — strictly canonical for r2.1
    /// records (exactly the five keys). A pre-r2.1 record (the four keys
    /// without `sec2p`) parses with `second_plan_discount_permille = 0`:
    /// that IS the r2.0 behavior the record was produced with, so archived
    /// experiments keep re-executing byte-identically instead of failing
    /// to parse.
    pub fn from_tag(tag: &str) -> Option<CoreParams> {
        let (own, value) = tag.split_once('|')?;
        let value = value.strip_prefix("value=")?;
        let parts: Vec<&str> = own.split(';').collect();
        if parts.len() != CORE_PARAM_KEYS.len() && parts.len() != CORE_PARAM_KEYS.len() - 1 {
            return None;
        }
        let mut values = [0i64; 5];
        // A legacy record's missing key defaults to 0 (disabled) — the
        // value the r2.0 core effectively ran with.
        values[4] = 0;
        for (index, part) in parts.iter().enumerate() {
            let (key, raw) = part.split_once('=')?;
            if key != CORE_PARAM_KEYS[index] {
                return None;
            }
            values[index] = raw.parse().ok()?;
        }
        Some(CoreParams {
            near_equal_permille: u32::try_from(values[0].max(0) as u64).ok()?,
            replan_retry_rounds: u32::try_from(values[1].max(0) as u64).ok()?,
            fallback_priority: values[2],
            spawn_jitter_cm: i32::try_from(values[3].clamp(0, 100_000)).ok()?,
            second_plan_discount_permille: u32::try_from(values[4].max(0) as u64).ok()?,
            value: ValueParams::from_tag(value)?,
        })
    }
}

/// The viability of the cached plan this round (the frozen `FieldPlanner`
/// semantics, re-stated for the value-ranked candidate).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PlanStatus {
    Active,
    Complete,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentDecision {
    pub agent: AgentId,
    /// Executable requests after policy overlays, before allocation. With no grant,
    /// zero means no supplied feasible candidates; nonzero means reservation conflict.
    pub candidate_count: usize,
    /// Requests held back because the action is not available this round.
    pub unavailable_count: usize,
    pub grant: Option<Task>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionTrace {
    pub round: u32,
    pub active: Option<FieldCandidate>,
    pub second: Option<FieldCandidate>,
    pub agents: Vec<AgentDecision>,
}

pub struct BuilderCore {
    faction: Faction,
    own: arena_core::types::Owner,
    cfg: GameConfig,
    params: CoreParams,
    rng: R2Rng,
    ledger: HackLedger,
    /// The chosen plan (possibly stale).
    plan: Option<FieldCandidate>,
    /// The plan validated for THIS round (`None` = fallback mode).
    active: Option<FieldCandidate>,
    next_search_round: u32,
    /// The SECOND plan (r2.1): the best value-ranked candidate whose edges
    /// cross neither the active plan's pending edges nor any live link —
    /// the task source for agents the primary triangle cannot employ.
    /// `None` in fallback mode and while the discount is 0.
    second: Option<FieldCandidate>,
    next_second_search_round: u32,
    /// The last spawn set (test/diagnostic echo).
    last_spawns: Vec<(AgentId, Coord)>,
    /// Best pending own candidate area2 of the last search (the economy's
    /// opportunity rate; 0 when the map offered nothing).
    best_pending_area2: u64,
    last_decision: Option<DecisionTrace>,
}

impl BuilderCore {
    pub fn new(
        faction: Faction,
        bot_seed: u64,
        stream: R2Stream,
        cfg: &GameConfig,
        params: CoreParams,
    ) -> Self {
        let (own, _) = owners(faction);
        BuilderCore {
            faction,
            own,
            cfg: *cfg,
            params,
            rng: R2Rng::new(bot_seed, faction, stream),
            ledger: HackLedger::new(),
            plan: None,
            active: None,
            next_search_round: 1,
            second: None,
            next_second_search_round: 1,
            last_spawns: Vec::new(),
            best_pending_area2: 0,
            last_decision: None,
        }
    }

    /// The spawn-time candidate view of the frozen map: every portal as
    /// defined, no links, no fields, no agents — exactly the legal
    /// pre-Round-1 information.
    fn map_observation(map: &MapDefinition) -> Observation {
        use arena_core::event::Event;
        Observation {
            round: 1,
            portals: map
                .portals()
                .iter()
                .map(|p| arena_core::observe::PortalPublic {
                    id: p.id,
                    position: p.position,
                    owner: p.owner,
                    hp: p.hp,
                })
                .collect(),
            links: Vec::new(),
            fields: Vec::new(),
            events: Vec::<Event>::new(),
            scores: BTreeMap::new(),
            own_agents: Vec::new(),
        }
    }

    /// Full-map initial spawn strategy: the near-equal seeded pick over the
    /// map's field candidates, roster agent `i` on the chosen triangle's
    /// vertex `i % 3` (the plan seeds itself with the same pick); without a
    /// viable triangle, a seeded rotation over the map's portals.
    pub fn choose_spawns(
        &mut self,
        cfg: &GameConfig,
        map: &MapDefinition,
        roster: &[AgentId],
    ) -> Vec<(AgentId, Coord)> {
        let obs = Self::map_observation(map);
        let candidates = field_candidates(&obs, self.faction, cfg, &self.params.value);
        self.best_pending_area2 = candidates.first().map(|c| c.area2).unwrap_or(0);
        let picked = pick_near_equal(
            &candidates,
            |candidate| candidate.area2 as i64,
            self.params.near_equal_permille,
            &mut self.rng,
        )
        .copied();
        self.plan = picked;
        self.active = picked;
        let jitter = self.params.spawn_jitter_cm.max(0);
        let (width, height) = (cfg.map_width_cm, cfg.map_height_cm);
        let bounds = |position: Coord| -> Coord {
            Coord::new(position.x.clamp(0, width), position.y.clamp(0, height))
        };
        self.last_spawns = match picked {
            Some(candidate) => roster
                .iter()
                .enumerate()
                .map(|(index, &agent)| {
                    let portal = map
                        .portal(candidate.vertices[index % 3])
                        .expect("search only returns map portals");
                    // Seeded faction-keyed jitter: the station is the
                    // vertex; the coordinates differ per faction stream.
                    let (dx, dy) = if jitter == 0 {
                        (0, 0)
                    } else {
                        (
                            self.rng.uniform(2 * jitter as u32 + 1) as i32 - jitter,
                            self.rng.uniform(2 * jitter as u32 + 1) as i32 - jitter,
                        )
                    };
                    (
                        agent,
                        bounds(Coord::new(portal.position.x + dx, portal.position.y + dy)),
                    )
                })
                .collect(),
            None => {
                let count = map.len() as u32;
                let offset = self.rng.uniform(count.max(1));
                roster
                    .iter()
                    .enumerate()
                    .map(|(index, &agent)| {
                        let portal = map
                            .portal(PortalId((offset as usize + index) as u32 % count))
                            .expect("dense portal ids");
                        (agent, portal.position)
                    })
                    .collect()
            }
        };
        self.last_spawns.clone()
    }

    /// The last spawn set (ascending roster order).
    pub fn last_spawns(&self) -> &[(AgentId, Coord)] {
        &self.last_spawns
    }

    fn status(&self, plan: &FieldCandidate, obs: &Observation) -> PlanStatus {
        let [a, b, c] = plan.vertices;
        let enemy = owners(self.faction).1;
        let mut complete = true;
        for (source, target) in [(a, b), (a, c), (b, c)] {
            let link_id =
                arena_core::types::LinkId::new(source, target).expect("distinct plan vertices");
            match obs.links.iter().find(|l| l.id == link_id) {
                Some(link) if link.faction == self.faction => continue,
                Some(_) => return PlanStatus::Invalid,
                None => {
                    complete = false;
                    let owner_of = |id: PortalId| common::portal(obs, id).owner;
                    if owner_of(source) == enemy || owner_of(target) == enemy {
                        return PlanStatus::Invalid;
                    }
                    let (ps, pt) = (
                        common::portal(obs, source).position,
                        common::portal(obs, target).position,
                    );
                    let crossed = obs.links.iter().any(|l| {
                        let (u, v) = (
                            common::portal(obs, l.id.min).position,
                            common::portal(obs, l.id.max).position,
                        );
                        let shared = l.id.contains(source) || l.id.contains(target);
                        !shared && arena_core::geometry::segments_intersect(ps, pt, u, v)
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

    /// The replan discipline (frozen `FieldPlanner` semantics): COMPLETE and
    /// INVALID replan immediately; only a FAILED search is throttled.
    pub fn refresh(&mut self, obs: &Observation) {
        let status = match &self.plan {
            Some(plan) => self.status(plan, obs),
            None => PlanStatus::Invalid,
        };
        match status {
            PlanStatus::Active => self.active = self.plan,
            PlanStatus::Complete | PlanStatus::Invalid => {
                let due = status == PlanStatus::Complete || obs.round >= self.next_search_round;
                if due {
                    let ranked =
                        rank_field_candidates(obs, self.faction, &self.cfg, &self.params.value);
                    self.best_pending_area2 = ranked.first().map(|(_, c)| c.area2).unwrap_or(0);
                    match pick_near_equal(
                        &ranked,
                        |(value, _)| *value,
                        self.params.near_equal_permille,
                        &mut self.rng,
                    )
                    .map(|(_, candidate)| *candidate)
                    {
                        Some(candidate) => {
                            self.plan = Some(candidate);
                            self.next_search_round = obs.round;
                        }
                        None => {
                            self.plan = None;
                            self.next_search_round = obs.round + self.params.replan_retry_rounds;
                        }
                    }
                }
                self.active = match &self.plan {
                    Some(plan) if self.status(plan, obs) == PlanStatus::Active => Some(*plan),
                    _ => None,
                };
            }
        }
        self.refresh_second(obs);
    }

    /// The second-plan discipline (r2.1): in plan mode with the discount
    /// enabled, keep a SECOND active candidate for the agents the primary
    /// triangle cannot employ. COMPLETE (the surplus built it) and INVALID
    /// re-derive immediately; a failed search is throttled by the same
    /// retry as the primary. Fallback mode clears it (the fallback spread
    /// already serves every agent). Deterministic: the best non-crossing
    /// candidate by (value, vertices) — no RNG, so mirror factions stay
    /// seed-symmetric.
    fn refresh_second(&mut self, obs: &Observation) {
        if self.params.second_plan_discount_permille == 0 || self.active.is_none() {
            self.second = None;
            self.next_second_search_round = obs.round;
            return;
        }
        let second_active = self
            .second
            .is_some_and(|second| self.status(&second, obs) == PlanStatus::Active);
        if second_active && self.second != self.active {
            return;
        }
        if obs.round < self.next_second_search_round {
            return;
        }
        let ranked = rank_field_candidates(obs, self.faction, &self.cfg, &self.params.value);
        let picked = ranked
            .iter()
            .filter(|(_, candidate)| {
                Some(candidate) != self.active.as_ref() && !self.crosses_plan(obs, candidate)
            })
            .find(|(_, candidate)| self.status(candidate, obs) == PlanStatus::Active)
            .map(|(_, candidate)| *candidate);
        self.second = picked;
        if picked.is_none() {
            self.next_second_search_round = obs.round + self.params.replan_retry_rounds;
        }
    }

    /// Whether `candidate`'s pending edges cross any pending edge of the
    /// ACTIVE plan (shared endpoints exempt — a shared vertex anchors both
    /// triangles). Existing links cannot be crossed by legality, so only
    /// the two plans' pending edges interact.
    fn crosses_plan(&self, obs: &Observation, candidate: &FieldCandidate) -> bool {
        let Some(plan) = &self.active else {
            return false;
        };
        for (cs, ct) in Self::edges(&candidate.vertices) {
            let candidate_link = arena_core::types::LinkId::new(cs, ct);
            let fired = obs.links.iter().any(|l| Some(l.id) == candidate_link);
            if fired {
                continue;
            }
            let (ps, pt) = (
                common::portal(obs, cs).position,
                common::portal(obs, ct).position,
            );
            for (ss, st) in Self::edges(&plan.vertices) {
                let plan_link = arena_core::types::LinkId::new(ss, st);
                let plan_fired = obs.links.iter().any(|l| Some(l.id) == plan_link);
                if plan_fired || cs == ss || cs == st || ct == ss || ct == st {
                    continue;
                }
                let (qs, qt) = (
                    common::portal(obs, ss).position,
                    common::portal(obs, st).position,
                );
                if arena_core::geometry::segments_intersect(ps, pt, qs, qt) {
                    return true;
                }
            }
        }
        false
    }

    /// The plan validated for this round (`None` = fallback mode).
    pub fn active(&self) -> Option<&FieldCandidate> {
        self.active.as_ref()
    }

    /// The second plan validated for this round (`None` = no surplus-work
    /// candidate; test/diagnostic accessor).
    pub fn second(&self) -> Option<&FieldCandidate> {
        self.second.as_ref()
    }

    /// The faction's economy at `obs`: horizon from the value model,
    /// opportunity rate = the best pending own candidate's per-round income.
    pub fn economy(&self, obs: &Observation) -> Economy {
        Economy::for_round(
            &self.cfg,
            obs.round,
            &self.params.value,
            self.best_pending_area2,
        )
    }

    /// The hack ledger (policies pre-check HACK legality through it).
    pub fn ledger_mut(&mut self) -> &mut HackLedger {
        &mut self.ledger
    }

    /// Marginal priority of assigning `me` to `station`: the plan's net
    /// value minus the candidate's own income for every round the agent
    /// spends reaching the station (integer score2 units, saturating).
    fn station_priority(
        &self,
        obs: &Observation,
        base: i64,
        per_round: u64,
        me: &arena_core::observe::AgentPrivate,
        station: PortalId,
    ) -> i64 {
        let target = common::portal(obs, station);
        let rounds = crate::r2::value::travel_rounds_between(
            me.position,
            target.position,
            self.cfg.move_budget_cm,
        );
        base.saturating_sub((per_round as i128 * rounds as i128).min(i64::MAX as i128) as i64)
    }

    /// The build task requests of the ACTIVE plan for every own agent:
    /// vertex stations (capture the neutral, key-farm the keyless own) and
    /// edge stations (link when both vertices are ours and the agent holds
    /// the target key), each agent ranked by net marginal priority. An
    /// agent requests several tasks so the allocator can fall through when
    /// its preferred station is taken — the staggering that removes
    /// same-portal crowding. With the r2.1 second plan active, the same
    /// stations of the SECOND candidate are appended at the recorded
    /// discount — surplus agents (and reinforcements) fall through into
    /// them after the primary plan's work, instead of idling.
    pub fn build_requests(&self, obs: &Observation) -> BTreeMap<AgentId, Vec<TaskRequest>> {
        let mut requests = BTreeMap::new();
        let Some(plan) = &self.active else {
            return requests;
        };
        let base = field_value(obs, self.faction, &self.cfg, &self.params.value, plan);
        let per_round = plan.area2;
        // The second plan's discounted base: its stations rank strictly
        // below comparable primary stations but far above any fallback. A
        // non-positive discounted base requests nothing (surplus agents
        // idle rather than build a candidate whose setup outlasts its
        // value).
        let second_base = self.second.map_or(0, |candidate| {
            let raw = field_value(obs, self.faction, &self.cfg, &self.params.value, &candidate);
            (raw as i128 * self.params.second_plan_discount_permille as i128 / 1000)
                .clamp(i64::MIN as i128, i64::MAX as i128) as i64
        });
        let second = match (&self.second, second_base > 0) {
            (Some(second), true) => Some(*second),
            _ => None,
        };
        let second_per_round = second.map_or(0, |candidate| candidate.area2);
        let keyed = |portal: PortalId| {
            obs.own_agents
                .iter()
                .any(|agent| agent.keys.get(&portal).copied().unwrap_or(0) >= 1)
        };
        let mut missing_key_vertices = std::collections::BTreeSet::new();
        for candidate in [Some(plan), second.as_ref()].into_iter().flatten() {
            for (source, target) in Self::edges(&candidate.vertices) {
                if obs
                    .links
                    .iter()
                    .any(|link| link.id == arena_core::types::LinkId::new(source, target).unwrap())
                {
                    continue;
                }
                let owned = [source, target]
                    .into_iter()
                    .all(|portal| common::portal(obs, portal).owner == self.own);
                let no_usable_holder = owned
                    && !obs.own_agents.iter().any(|agent| {
                        [(source, target), (target, source)]
                            .into_iter()
                            .any(|(s, t)| {
                                common::link_completion_delay(obs, self.own, &self.cfg, agent, s, t)
                                    .is_some()
                            })
                    });
                if (!keyed(source) && !keyed(target)) || no_usable_holder {
                    missing_key_vertices.extend([source, target]);
                }
            }
        }
        for me in obs.own_agents.iter() {
            let mut mine = Vec::new();
            for (candidate, base, per_round) in [
                (Some(plan), base, per_round),
                (second.as_ref(), second_base, second_per_round),
            ] {
                let Some(candidate) = candidate else {
                    continue;
                };
                for &vertex in &candidate.vertices {
                    let view = common::portal(obs, vertex);
                    match view.owner {
                        arena_core::types::Owner::Neutral => {
                            let pri = self.station_priority(obs, base, per_round, me, vertex);
                            mine.push(TaskRequest::one_shot(Task::Capture { portal: vertex }, pri));
                        }
                        owner
                            if owner == self.own
                                && missing_key_vertices.contains(&vertex)
                                && me.keys.get(&vertex).copied().unwrap_or(0) == 0 =>
                        {
                            let pri = self.station_priority(obs, base, per_round, me, vertex);
                            mine.push(TaskRequest::one_shot(Task::KeyFarm { portal: vertex }, pri));
                        }
                        _ => {}
                    }
                }
                for (source, target) in Self::edges(&candidate.vertices) {
                    let link_id = arena_core::types::LinkId::new(source, target)
                        .expect("distinct plan vertices");
                    let exists = obs
                        .links
                        .iter()
                        .any(|l| l.id == link_id && l.faction == self.faction);
                    if exists {
                        continue;
                    }
                    let (view_s, view_t) =
                        (common::portal(obs, source), common::portal(obs, target));
                    if view_s.owner != self.own || view_t.owner != self.own {
                        continue;
                    }
                    for (source, target) in [(source, target), (target, source)] {
                        if let Some(delay) = common::link_completion_delay(
                            obs, self.own, &self.cfg, me, source, target,
                        ) {
                            let pri = base.saturating_sub(
                                (per_round as i128 * delay as i128).min(i64::MAX as i128) as i64,
                            );
                            mine.push(TaskRequest::one_shot(Task::Link { source, target }, pri));
                        }
                    }
                }
            }
            requests.insert(me.id, mine);
        }
        requests
    }

    /// The three canonical edges of a triangle (ascending pairs).
    fn edges(vertices: &[PortalId; 3]) -> [(PortalId, PortalId); 3] {
        let [a, b, c] = *vertices;
        [
            (a.min(b), a.max(b)),
            (a.min(c), a.max(c)),
            (b.min(c), b.max(c)),
        ]
    }

    /// Fallback task requests (no viable plan anywhere): every agent
    /// requests its nearest incomplete portals (positive priorities well
    /// below any live-plan value, so a replan preempts fallback instantly)
    /// plus every opportunistic LINK it could legally emit right now — the
    /// fallback still closes triangles, it just has no plan to walk.
    pub fn fallback_requests(&self, obs: &Observation) -> BTreeMap<AgentId, Vec<TaskRequest>> {
        let mut requests = BTreeMap::new();
        for me in obs.own_agents.iter() {
            let mut mine = Vec::new();
            // Opportunistic links first (the highest-value fallback work).
            for (source, target) in self.own_pairs_this_round(obs, me) {
                mine.push(TaskRequest::one_shot(
                    Task::Link { source, target },
                    self.params.fallback_priority,
                ));
            }
            // Nearest incomplete portals, near-first.
            let mut incomplete: Vec<(i64, PortalId)> = obs
                .portals
                .iter()
                .filter(|p| {
                    p.owner == arena_core::types::Owner::Neutral
                        || me.keys.get(&p.id).copied().unwrap_or(0) == 0
                })
                .map(|p| (arena_core::geometry::dist2(me.position, p.position), p.id))
                .collect();
            incomplete.sort();
            for (rank, (dist2, portal)) in incomplete.into_iter().take(3).enumerate() {
                let view = common::portal(obs, portal);
                let task = match view.owner {
                    arena_core::types::Owner::Neutral => Task::Capture { portal },
                    _ => Task::KeyFarm { portal },
                };
                let _ = dist2;
                mine.push(TaskRequest::one_shot(
                    task,
                    self.params
                        .fallback_priority
                        .saturating_sub(1 + rank as i64 + (rank as i64) * rank as i64),
                ));
            }
            requests.insert(me.id, mine);
        }
        requests
    }

    /// Every legal LINK `(source, target)` SOME own agent could emit this
    /// round (the blocker overlay's candidate set): for each agent in
    /// operation range of an own `source` holding a key of an own
    /// `target`, the full pre-flight legality check. Deterministic:
    /// ascending agent id, then ascending pairs.
    pub fn blockable_pairs(&self, obs: &Observation) -> Vec<(PortalId, PortalId, AgentId)> {
        let mut out = Vec::new();
        for me in obs.own_agents.iter() {
            for (source, target) in self.own_pairs_this_round(obs, me) {
                out.push((source, target, me.id));
            }
        }
        out
    }

    /// Every legal LINK `(source, target)` `me` could emit this round:
    /// standing or moving into operation range of an own `source`, holding a key of
    /// an own `target`, and passing the full pre-flight legality check.
    fn own_pairs_this_round(
        &self,
        obs: &Observation,
        me: &arena_core::observe::AgentPrivate,
    ) -> Vec<(PortalId, PortalId)> {
        let mut pairs = Vec::new();
        for source in obs.portals.iter().filter(|p| p.owner == self.own) {
            let mut targets: Vec<PortalId> = me
                .keys
                .iter()
                .filter(|(portal, count)| {
                    **count >= 1
                        && **portal != source.id
                        && common::portal(obs, **portal).owner == self.own
                })
                .map(|(&portal, _)| portal)
                .collect();
            targets.sort();
            for target in targets {
                if common::link_position_this_round(obs, self.own, &self.cfg, me, source.id, target)
                    .is_some()
                {
                    pairs.push((source.id, target));
                }
            }
        }
        pairs
    }

    /// Executes the granted tasks into command lists, with the
    /// observation-level pre-submit legality checks of plan 6.3 (execution
    /// races still record as normal Drops): approach stations with one
    /// Move, act when in range and the observation still supports the
    /// action. Agents with no grant idle (an explicit PASS stream).
    pub fn execute(
        &mut self,
        obs: &Observation,
        grants: &BTreeMap<AgentId, Task>,
    ) -> BTreeMap<AgentId, CommandList> {
        let round = obs.round;
        let cfg = &self.cfg;
        let own = self.own;
        let mut out = BTreeMap::new();
        for me in obs.own_agents.iter() {
            let Some(task) = grants.get(&me.id) else {
                continue;
            };
            let mut commands = CommandList::new();
            match *task {
                Task::Capture { portal } => {
                    let target = common::portal(obs, portal);
                    if target.owner == arena_core::types::Owner::Neutral
                        && in_range(me.position, target.position, cfg.portal_operation_range_cm)
                    {
                        commands.major(arena_core::types::Command::Capture { portal });
                    } else if target.owner != own {
                        commands.mv(arena_core::types::Command::Move {
                            destination: step_toward(
                                me.position,
                                target.position,
                                cfg.move_budget_cm,
                            ),
                        });
                    } else {
                        // Captured by a teammate in the same round's
                        // race or an earlier round: farm a key instead.
                        if me.keys.get(&portal).copied().unwrap_or(0) == 0
                            && in_range(me.position, target.position, cfg.portal_operation_range_cm)
                            && self.ledger.eligible(round, cfg, me.id, portal)
                        {
                            self.ledger.record(round, cfg, me.id, portal);
                            commands.aux(arena_core::types::Command::Hack { portal });
                        }
                    }
                }
                Task::KeyFarm { portal } => {
                    let target = common::portal(obs, portal);
                    if in_range(me.position, target.position, cfg.portal_operation_range_cm) {
                        if self.ledger.eligible(round, cfg, me.id, portal) {
                            self.ledger.record(round, cfg, me.id, portal);
                            commands.aux(arena_core::types::Command::Hack { portal });
                        }
                    } else {
                        commands.mv(arena_core::types::Command::Move {
                            destination: step_toward(
                                me.position,
                                target.position,
                                cfg.move_budget_cm,
                            ),
                        });
                    }
                }
                Task::Link { source, target } => {
                    let stand = common::portal(obs, source);
                    if let Some(position) =
                        common::link_position_this_round(obs, own, cfg, me, source, target)
                    {
                        if position != me.position {
                            commands.aux(arena_core::types::Command::Move {
                                destination: position,
                            });
                        }
                        commands.aux(arena_core::types::Command::Link {
                            source_portal: source,
                            target_portal: target,
                        });
                    } else if common::portal(obs, source).owner == own
                        || common::portal(obs, source).owner == arena_core::types::Owner::Neutral
                    {
                        commands.mv(arena_core::types::Command::Move {
                            destination: step_toward(
                                me.position,
                                stand.position,
                                cfg.move_budget_cm,
                            ),
                        });
                    }
                }
                Task::Attack { portal } => {
                    let target = common::portal(obs, portal);
                    let enemy = owners(self.faction).1;
                    if target.owner == enemy
                        && in_range(me.position, target.position, cfg.portal_operation_range_cm)
                    {
                        commands.major(arena_core::types::Command::Attack);
                    } else if target.owner == enemy {
                        let destination =
                            step_toward(me.position, target.position, cfg.move_budget_cm);
                        // Aux Move precedes the Major Action in the same stream.
                        if in_range(destination, target.position, cfg.portal_operation_range_cm) {
                            commands.aux(arena_core::types::Command::Move { destination });
                            commands.major(arena_core::types::Command::Attack);
                        } else {
                            commands.mv(arena_core::types::Command::Move { destination });
                        }
                    }
                }
                Task::Recharge { portal } => {
                    let target = common::portal(obs, portal);
                    // The overlay releases full-HP anchors. A stale grant
                    // also emits nothing; remote target Keys satisfy the
                    // position condition without being consumed.
                    let damaged = target.hp < cfg.portal_max_hp;
                    if target.owner == own
                        && damaged
                        && (in_range(me.position, target.position, cfg.portal_operation_range_cm)
                            || me.keys.get(&portal).copied().unwrap_or(0) > 0)
                        && me.supply >= cfg.recharge_cost_supply
                    {
                        commands.major(arena_core::types::Command::Recharge { portal });
                    } else if target.owner == own
                        && damaged
                        && me.supply >= cfg.recharge_cost_supply
                    {
                        let destination =
                            step_toward(me.position, target.position, cfg.move_budget_cm);
                        if in_range(destination, target.position, cfg.portal_operation_range_cm) {
                            commands.aux(arena_core::types::Command::Move { destination });
                            commands.major(arena_core::types::Command::Recharge { portal });
                        } else {
                            commands.mv(arena_core::types::Command::Move { destination });
                        }
                    }
                }
                // Hold station at the portal: approach, then wait for the
                // key delivery (the logistics overlay's rendezvous).
                Task::Standby { portal } => {
                    let target = common::portal(obs, portal);
                    if !in_range(me.position, target.position, cfg.portal_operation_range_cm) {
                        commands.mv(arena_core::types::Command::Move {
                            destination: step_toward(
                                me.position,
                                target.position,
                                cfg.move_budget_cm,
                            ),
                        });
                    }
                }
                // Transfer rendezvous: approach the receiver; the owning
                // policy's payment step emits the payload once the
                // receiver is inside TRANSFER_RANGE.
                Task::Transfer { receiver, .. } => {
                    let Some(target) = obs.own_agents.iter().find(|me| me.id == receiver) else {
                        continue;
                    };
                    if !in_range(me.position, target.position, cfg.transfer_range_cm) {
                        commands.mv(arena_core::types::Command::Move {
                            destination: step_toward(
                                me.position,
                                target.position,
                                cfg.move_budget_cm,
                            ),
                        });
                    }
                }
            }
            if !commands.is_empty() {
                out.insert(me.id, commands);
            }
        }
        out
    }

    /// Links emitted this round share a geometric reservation as well as
    /// an edge id. Opponent races remain unknown; our own crossing edges
    /// are known before submission and must fall through to alternate work.
    fn compatible_link_grant(
        &self,
        obs: &Observation,
        agent: AgentId,
        task: Task,
        granted: &BTreeMap<AgentId, Task>,
    ) -> bool {
        let Task::Link { source, target } = task else {
            return true;
        };
        let me = obs.own_agents.iter().find(|me| me.id == agent).unwrap();
        if common::link_position_this_round(obs, self.own, &self.cfg, me, source, target).is_none()
        {
            return true;
        }
        granted.iter().all(|(&other, task)| {
            let Task::Link {
                source: other_source,
                target: other_target,
            } = *task
            else {
                return true;
            };
            if [source, target]
                .iter()
                .any(|p| *p == other_source || *p == other_target)
            {
                return true;
            }
            let teammate = obs.own_agents.iter().find(|me| me.id == other).unwrap();
            common::link_position_this_round(
                obs,
                self.own,
                &self.cfg,
                teammate,
                other_source,
                other_target,
            )
            .is_none()
                || !arena_core::geometry::segments_intersect(
                    common::portal(obs, source).position,
                    common::portal(obs, target).position,
                    common::portal(obs, other_source).position,
                    common::portal(obs, other_target).position,
                )
        })
    }

    /// Read-only allocation preview shared by delivery comparison and execution.
    /// Availability and surplus work are evaluated on the same reservation board.
    pub fn preview_allocation(
        &self,
        obs: &Observation,
        requests: &BTreeMap<AgentId, Vec<TaskRequest>>,
    ) -> DecisionTrace {
        let agents: Vec<_> = obs.own_agents.iter().map(|a| a.id).collect();
        let mut ready: BTreeMap<AgentId, Vec<TaskRequest>> = obs
            .own_agents
            .iter()
            .map(|agent| {
                let mine = requests
                    .get(&agent.id)
                    .into_iter()
                    .flatten()
                    .filter(|request| match request.task {
                        Task::KeyFarm { portal } => {
                            !in_range(
                                agent.position,
                                common::portal(obs, portal).position,
                                self.cfg.portal_operation_range_cm,
                            ) || self
                                .ledger
                                .available(obs.round, &self.cfg, agent.id, portal)
                        }
                        _ => true,
                    })
                    .copied()
                    .collect();
                (agent.id, mine)
            })
            .collect();
        let unavailable: BTreeMap<AgentId, usize> = agents
            .iter()
            .map(|&agent| {
                (
                    agent,
                    requests.get(&agent).map_or(0, Vec::len)
                        - ready.get(&agent).map_or(0, Vec::len),
                )
            })
            .collect();
        let mut board = TaskBoard::new(obs.round);
        let compatible = |agent, task, granted: &BTreeMap<AgentId, Task>| {
            self.compatible_link_grant(obs, agent, task, granted)
        };
        let mut grants = allocate_compatible(&mut board, &agents, &ready, compatible);
        // Construction or another combat task may win a required teammate.
        // Recheck actual cohorts until removing a failed plan no longer
        // changes the others. Each iteration removes at least one request.
        loop {
            let mut rejected = std::collections::BTreeSet::new();
            let targets: std::collections::BTreeSet<_> = grants
                .values()
                .filter_map(|task| {
                    if let Task::Attack { portal } = task {
                        Some(*portal)
                    } else {
                        None
                    }
                })
                .collect();
            for portal in targets {
                let target = common::portal(obs, portal);
                let economy = self.economy(obs);
                let team: Vec<_> = obs
                    .own_agents
                    .iter()
                    .filter(|agent| grants.get(&agent.id) == Some(&Task::Attack { portal }))
                    .filter_map(|agent| {
                        crate::r2::value::AttackArrival::for_agent(
                            agent,
                            target.position,
                            &self.cfg,
                            economy.horizon_rounds,
                        )
                    })
                    .collect();
                let income = crate::r2::value::attack_income_per_round(
                    obs,
                    self.faction,
                    &self.cfg,
                    &self.params.value,
                    portal,
                );
                if crate::r2::value::strike_value(
                    &economy,
                    income,
                    crate::r2::value::attacks_needed(target.hp, &self.cfg),
                    &team,
                )
                .is_none_or(|value| value <= 0)
                {
                    rejected.insert(Task::Attack { portal });
                }
            }
            let rescue_targets: std::collections::BTreeSet<_> = grants
                .values()
                .filter_map(|task| match task {
                    Task::Recharge { portal } => Some(*portal),
                    _ => None,
                })
                .collect();
            if !rescue_targets.is_empty() {
                let economy = self.economy(obs);
                let defense = crate::r2::value::DefenseModel::new(
                    obs,
                    self.faction,
                    &self.cfg,
                    &self.params.value,
                    &economy,
                );
                for portal in rescue_targets {
                    let team: Vec<_> = grants
                        .iter()
                        .filter_map(|(&agent, task)| {
                            (*task == Task::Recharge { portal }).then_some(agent)
                        })
                        .collect();
                    if defense.coordinated_value(&team, portal) <= 0 {
                        rejected.insert(Task::Recharge { portal });
                    }
                }
            }
            if rejected.is_empty() {
                break;
            }
            for mine in ready.values_mut() {
                mine.retain(|request| !rejected.contains(&request.task));
            }
            board = TaskBoard::new(obs.round);
            grants = allocate_compatible(&mut board, &agents, &ready, compatible);
        }
        // Unused builders may expand without drawing a crossing Link through
        // either live plan. Reuse the same reservation board; planned work and
        // policy overlays keep their grants, and no capture is duplicated.
        {
            let free: Vec<_> = agents
                .iter()
                .copied()
                .filter(|agent| !grants.contains_key(agent))
                .collect();
            let mut expansion = BTreeMap::new();
            for &agent in &free {
                let me = obs.own_agents.iter().find(|me| me.id == agent).unwrap();
                let mut targets: Vec<_> = obs
                    .portals
                    .iter()
                    .filter(|p| p.owner == arena_core::types::Owner::Neutral)
                    .filter_map(|p| {
                        common::movement_rounds_to_within(
                            me.position,
                            p.position,
                            self.cfg.portal_operation_range_cm,
                            self.cfg.move_budget_cm,
                            self.cfg.total_rounds.saturating_sub(obs.round),
                        )
                        .map(|moves| (moves, p.id))
                    })
                    .collect();
                targets.sort();
                let mine: Vec<_> = targets
                    .into_iter()
                    .map(|(moves, portal)| {
                        TaskRequest::one_shot(Task::Capture { portal }, -(moves as i64))
                    })
                    .collect();
                for request in &mine {
                    if !ready[&agent].iter().any(|r| r.task == request.task) {
                        ready.get_mut(&agent).unwrap().push(*request);
                    }
                }
                expansion.insert(agent, mine);
            }
            grants.extend(allocate(&mut board, &free, &expansion));
        }
        DecisionTrace {
            round: obs.round,
            active: self.active,
            second: self.second,
            agents: agents
                .iter()
                .map(|&agent| AgentDecision {
                    agent,
                    candidate_count: ready.get(&agent).map_or(0, Vec::len),
                    unavailable_count: unavailable[&agent],
                    grant: grants.get(&agent).copied(),
                })
                .collect(),
        }
    }

    /// Allocate once, record the actual grants, then execute them.
    pub fn allocate_and_execute(
        &mut self,
        obs: &Observation,
        requests: &BTreeMap<AgentId, Vec<TaskRequest>>,
    ) -> BTreeMap<AgentId, CommandList> {
        let decision = self.preview_allocation(obs, requests);
        let grants = decision
            .agents
            .iter()
            .filter_map(|agent| agent.grant.map(|task| (agent.agent, task)))
            .collect();
        self.last_decision = Some(decision);
        self.execute(obs, &grants)
    }

    pub fn decision_trace(&self) -> Option<&DecisionTrace> {
        self.last_decision.as_ref()
    }

    /// The pure-core round: refresh, build (or fallback) requests,
    /// allocate through a fresh per-round board, execute. This IS
    /// `greedy-coordinated-r2`'s decision function.
    pub fn core_round(&mut self, obs: &Observation) -> BTreeMap<AgentId, CommandList> {
        self.refresh(obs);
        let requests = if self.active.is_some() {
            self.build_requests(obs)
        } else {
            self.fallback_requests(obs)
        };
        self.allocate_and_execute(obs, &requests)
    }

    /// The faction this core plays.
    pub fn faction(&self) -> Faction {
        self.faction
    }

    /// The match's GameConfig (the value-model helpers need it).
    pub fn cfg(&self) -> &GameConfig {
        &self.cfg
    }

    /// The core's parameters (policies re-serialize them into their spec).
    pub fn params(&self) -> &CoreParams {
        &self.params
    }

    /// Minimum whole rounds an agent needs to reach `portal` (the public
    /// helper over the observation).
    pub fn travel_rounds_to(&self, obs: &Observation, portal: PortalId) -> u32 {
        travel_rounds_to(obs, &self.cfg, portal)
    }
}
