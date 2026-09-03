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

use crate::common::{self, CommandList, HackLedger, in_range, link_is_legal, owners, step_toward};
use crate::r2::rng::{R2Rng, R2Stream};
use crate::r2::sample::pick_near_equal;
use crate::r2::tasks::{Task, TaskBoard, TaskRequest, allocate};
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
    /// The shared value model's thresholds (inlined into the record).
    pub value: ValueParams,
}

impl Default for CoreParams {
    fn default() -> Self {
        CoreParams {
            near_equal_permille: 100,
            replan_retry_rounds: 25,
            fallback_priority: 1,
            value: ValueParams::default(),
        }
    }
}

const CORE_PARAM_KEYS: [&str; 3] = ["nearequal", "retry", "fbpri"];

impl CoreParams {
    /// Canonical serialization: `nearequal=..;retry=..;fbpri=..|value=<ValueParams::tag>`.
    pub fn tag(&self) -> String {
        let values = [
            self.near_equal_permille.to_string(),
            self.replan_retry_rounds.to_string(),
            self.fallback_priority.to_string(),
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

    /// Parses [`CoreParams::tag`] back — strictly canonical.
    pub fn from_tag(tag: &str) -> Option<CoreParams> {
        let (own, value) = tag.split_once('|')?;
        let value = value.strip_prefix("value=")?;
        let parts: Vec<&str> = own.split(';').collect();
        if parts.len() != CORE_PARAM_KEYS.len() {
            return None;
        }
        let mut values = [0u64; 3];
        for (index, part) in parts.iter().enumerate() {
            let (key, raw) = part.split_once('=')?;
            if key != CORE_PARAM_KEYS[index] {
                return None;
            }
            values[index] = raw.parse().ok()?;
        }
        Some(CoreParams {
            near_equal_permille: u32::try_from(values[0]).ok()?,
            replan_retry_rounds: u32::try_from(values[1]).ok()?,
            fallback_priority: i64::try_from(values[2]).ok()?,
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
    /// The last spawn set (test/diagnostic echo).
    last_spawns: Vec<(AgentId, Coord)>,
    /// Best pending own candidate area2 of the last search (the economy's
    /// opportunity rate; 0 when the map offered nothing).
    best_pending_area2: u64,
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
            last_spawns: Vec::new(),
            best_pending_area2: 0,
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
        self.last_spawns = match picked {
            Some(candidate) => roster
                .iter()
                .enumerate()
                .map(|(index, &agent)| {
                    let portal = map
                        .portal(candidate.vertices[index % 3])
                        .expect("search only returns map portals");
                    (agent, portal.position)
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
    }

    /// The plan validated for this round (`None` = fallback mode).
    pub fn active(&self) -> Option<&FieldCandidate> {
        self.active.as_ref()
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
    /// same-portal crowding.
    pub fn build_requests(&self, obs: &Observation) -> BTreeMap<AgentId, Vec<TaskRequest>> {
        let mut requests = BTreeMap::new();
        let Some(plan) = &self.active else {
            return requests;
        };
        let base = field_value(obs, self.faction, &self.cfg, &self.params.value, plan);
        let per_round = plan.area2;
        let keyed = |portal: PortalId| {
            obs.own_agents
                .iter()
                .any(|agent| agent.keys.get(&portal).copied().unwrap_or(0) >= 1)
        };
        for me in obs.own_agents.iter() {
            let mut mine = Vec::new();
            for &vertex in &plan.vertices {
                let view = common::portal(obs, vertex);
                match view.owner {
                    arena_core::types::Owner::Neutral => {
                        let pri = self.station_priority(obs, base, per_round, me, vertex);
                        mine.push(TaskRequest::one_shot(Task::Capture { portal: vertex }, pri));
                    }
                    owner if owner == self.own && !keyed(vertex) => {
                        let pri = self.station_priority(obs, base, per_round, me, vertex);
                        mine.push(TaskRequest::one_shot(Task::KeyFarm { portal: vertex }, pri));
                    }
                    _ => {}
                }
            }
            for (source, target) in Self::edges(&plan.vertices) {
                let link_id =
                    arena_core::types::LinkId::new(source, target).expect("distinct plan vertices");
                let exists = obs
                    .links
                    .iter()
                    .any(|l| l.id == link_id && l.faction == self.faction);
                if exists {
                    continue;
                }
                let (view_s, view_t) = (common::portal(obs, source), common::portal(obs, target));
                if view_s.owner != self.own || view_t.owner != self.own {
                    continue;
                }
                if me.keys.get(&target).copied().unwrap_or(0) < 1 {
                    continue;
                }
                let pri = self.station_priority(obs, base, per_round, me, source);
                mine.push(TaskRequest::one_shot(
                    Task::Link {
                        source,
                        target: target,
                    },
                    pri,
                ));
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
            for (source, target) in self.own_pairs_in_range(obs, me) {
                mine.push(TaskRequest::one_shot(
                    Task::Link { source, target },
                    self.params.fallback_priority,
                ));
            }
            // Nearest incomplete portals, near-first.
            let mut incomplete: Vec<(i64, PortalId)> = obs
                .portals
                .iter()
                .filter(|p| p.owner != self.own || me.keys.get(&p.id).copied().unwrap_or(0) == 0)
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
            for (source, target) in self.own_pairs_in_range(obs, me) {
                out.push((source, target, me.id));
            }
        }
        out
    }

    /// Every legal LINK `(source, target)` `me` could emit this round:
    /// standing within operation range of an own `source`, holding a key of
    /// an own `target`, and passing the full pre-flight legality check.
    fn own_pairs_in_range(
        &self,
        obs: &Observation,
        me: &arena_core::observe::AgentPrivate,
    ) -> Vec<(PortalId, PortalId)> {
        let mut pairs = Vec::new();
        for source in obs.portals.iter().filter(|p| p.owner == self.own) {
            if !in_range(
                me.position,
                source.position,
                self.cfg.portal_operation_range_cm,
            ) {
                continue;
            }
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
                if link_is_legal(
                    obs,
                    self.own,
                    &self.cfg,
                    me.position,
                    &me.keys,
                    source.id,
                    target,
                ) {
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
                    if in_range(me.position, stand.position, cfg.portal_operation_range_cm)
                        && link_is_legal(obs, own, cfg, me.position, &me.keys, source, target)
                    {
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
                        commands.mv(arena_core::types::Command::Move {
                            destination: step_toward(
                                me.position,
                                target.position,
                                cfg.move_budget_cm,
                            ),
                        });
                    }
                }
                Task::Recharge { portal } => {
                    let target = common::portal(obs, portal);
                    if target.owner == own
                        && in_range(me.position, target.position, cfg.portal_operation_range_cm)
                        && me.supply >= cfg.recharge_cost_supply
                    {
                        commands.major(arena_core::types::Command::Recharge { portal });
                    } else if target.owner == own {
                        commands.mv(arena_core::types::Command::Move {
                            destination: step_toward(
                                me.position,
                                target.position,
                                cfg.move_budget_cm,
                            ),
                        });
                    }
                }
                // Transfer rendezvous commands are emitted by the policy
                // overlay that owns the payload (the core reserves the
                // one-shot task only).
                Task::Transfer { .. } => {}
            }
            if !commands.is_empty() {
                out.insert(me.id, commands);
            }
        }
        out
    }

    /// The pure-core round: refresh, build (or fallback) requests,
    /// allocate through a fresh per-round board, execute. This IS
    /// `greedy-coordinated-r2`'s decision function.
    pub fn core_round(&mut self, obs: &Observation) -> BTreeMap<AgentId, CommandList> {
        self.refresh(obs);
        let agents: Vec<AgentId> = obs.own_agents.iter().map(|me| me.id).collect();
        let requests = if self.active.is_some() {
            self.build_requests(obs)
        } else {
            self.fallback_requests(obs)
        };
        let mut board = TaskBoard::new(obs.round);
        let grants = allocate(&mut board, &agents, &requests);
        self.execute(obs, &grants)
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
