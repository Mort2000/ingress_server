//! Mutable match state, per-round trackers, and the mutation surface used
//! by the resolver (P1-LOOP). `GameState` owns every authoritative mutation:
//! [`crate::resolve::Resolver`] decides validity, this module applies it.

use std::collections::BTreeMap;

use crate::config::{ConfigError, GameConfig};
use crate::event::{Event, EventKind};
use crate::linkgraph::{LinkGraph, NeutralizeCascade};
use crate::map::MapDefinition;
use crate::spawn::{InitialSpawns, validate_initial_spawn_set};
use crate::types::{
    Agent, AgentId, Coord, Faction, FieldId, LinkId, MatchResult, Owner, Portal, PortalId, Score2,
};

/// Per-agent usage counters for the current round (R-ACTION-01, R-MOVE-01).
/// Reset ONLY by [`GameState::advance_round`] — never mid-round, even when a
/// command DROPs (R-RESOLVE-02).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct RoundTracker {
    /// Whether this agent's single Major Action allowance is spent. Set even
    /// when the major command DROPs (D28).
    pub major_used: bool,
    /// Sum of successful MOVE costs this round, in coordinate units.
    pub move_spent: u64,
}

/// Per-`(agent, portal)` HACK bookkeeping (R-HACK-02, D31/D32).
/// `hack_ordinal` is the LIFETIME count of successful hacks — the RNG stable
/// index of the next draw — and is never reset, not even by epoch
/// boundaries; `last_hack_round` and `hack_count_epoch` reset eagerly when a
/// new epoch starts.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct HackTracker {
    /// Round of the pair's last successful hack; anchors the cooldown
    /// `current_round - last_hack_round >= HACK_COOLDOWN`.
    pub last_hack_round: Option<u32>,
    /// Successful hacks by this pair within the current epoch.
    pub hack_count_epoch: u32,
    /// Lifetime count of successful hacks; the next draw's stable index.
    pub hack_ordinal: u32,
}

/// Construction failures for [`GameState::new`].
#[derive(Debug, PartialEq, Eq)]
pub enum StateError {
    /// The configuration failed [`GameConfig::validate`].
    InvalidConfig(ConfigError),
    /// The map definition failed `MapDefinition::validate` against the config.
    MapInvalid,
    /// A spawn set does not exactly cover that faction's initial roster (D17).
    SpawnSetMismatch(Faction),
}

/// Maps a faction onto the matching portal owner; `Owner::Neutral` has no
/// faction counterpart.
pub(crate) fn faction_as_owner(faction: Faction) -> Owner {
    match faction {
        Faction::Enl => Owner::Enl,
        Faction::Res => Owner::Res,
    }
}

/// Complete authoritative match state: portals, agents, per-round trackers,
/// the link graph with its derived field cache, the public event log, and
/// faction scores. Scores accumulate from surviving fields each round
/// (R-SCORE-01); the map definition is copied as the authoritative input.
/// `PartialEq` is whole-state equality — the M3/M4 determinism gates run the
/// same scenario twice and demand identical states, so the link graph and
/// the reinforcement id counter participate in the comparison.
#[derive(Debug, PartialEq, Eq)]
pub struct GameState {
    config: GameConfig,
    round: u32,
    portals: Vec<Portal>,
    agents: BTreeMap<AgentId, Agent>,
    trackers: BTreeMap<AgentId, RoundTracker>,
    hack_trackers: BTreeMap<(AgentId, PortalId), HackTracker>,
    link_graph: LinkGraph,
    next_agent_id: AgentId,
    events: Vec<Event>,
    event_seq: u64,
    scores: BTreeMap<Faction, Score2>,
    /// D47 same-round double-settle guard: `Some(r)` once
    /// [`crate::resolve::Resolver::run_round`] settled round `r` and the
    /// round has not advanced yet. Diagnostic bookkeeping — excluded from
    /// the M5 canonical state hash and cleared by `advance_round`.
    settled_round: Option<u32>,
}

impl GameState {
    /// Builds the initial state for round 1: validates the config and map,
    /// derives the roster per D17, and applies both spawn sets. A spawn set
    /// that does not exactly cover its faction's roster is rejected
    /// (`SpawnSetMismatch`) — there is no partial spawn (R-SPAWN-01).
    pub fn new(
        cfg: GameConfig,
        map: &MapDefinition,
        enl: &InitialSpawns,
        res: &InitialSpawns,
    ) -> Result<Self, StateError> {
        cfg.validate().map_err(StateError::InvalidConfig)?;
        map.validate(&cfg).map_err(|_| StateError::MapInvalid)?;

        let (enl_roster, res_roster) = Self::initial_roster(&cfg);
        let enl_entries: Vec<(AgentId, Coord)> = enl.iter().collect();
        validate_initial_spawn_set(&enl_roster, &enl_entries, &cfg)
            .map_err(|_| StateError::SpawnSetMismatch(Faction::Enl))?;
        let res_entries: Vec<(AgentId, Coord)> = res.iter().collect();
        validate_initial_spawn_set(&res_roster, &res_entries, &cfg)
            .map_err(|_| StateError::SpawnSetMismatch(Faction::Res))?;

        let mut agents = BTreeMap::new();
        for (id, position) in enl.iter() {
            agents.insert(
                id,
                Agent {
                    id,
                    faction: Faction::Enl,
                    position,
                    supply: cfg.initial_supply,
                    keys: BTreeMap::new(),
                },
            );
        }
        for (id, position) in res.iter() {
            agents.insert(
                id,
                Agent {
                    id,
                    faction: Faction::Res,
                    position,
                    supply: cfg.initial_supply,
                    keys: BTreeMap::new(),
                },
            );
        }

        let trackers = agents
            .keys()
            .map(|&id| (id, RoundTracker::default()))
            .collect();
        let mut scores = BTreeMap::new();
        scores.insert(Faction::Enl, 0);
        scores.insert(Faction::Res, 0);
        // D17 continuation: reinforcement ids continue past the initial
        // roster ENL 0..n-1, RES n..2n-1.
        let next_agent_id = AgentId(
            (cfg.initial_agents_per_faction as u64)
                .checked_mul(2)
                .expect("initial_agents_per_faction is bounded by config sanity")
                as u32,
        );

        Ok(GameState {
            config: cfg,
            round: 1,
            portals: map.portals().to_vec(),
            agents,
            trackers,
            hack_trackers: BTreeMap::new(),
            link_graph: LinkGraph::default(),
            next_agent_id,
            events: Vec::new(),
            event_seq: 0,
            scores,
            settled_round: None,
        })
    }

    /// Dense initial roster (D17): ENL `AgentId(0..n-1)`, then RES
    /// `AgentId(n..2n-1)`, with `n = initial_agents_per_faction`.
    pub fn initial_roster(cfg: &GameConfig) -> (Vec<AgentId>, Vec<AgentId>) {
        let n = cfg.initial_agents_per_faction as u64;
        let enl = (0..n).map(|i| AgentId(i as u32)).collect();
        let res = (n..2 * n).map(|i| AgentId(i as u32)).collect();
        (enl, res)
    }

    // ------------------------------------------------------------------
    // Read accessors
    // ------------------------------------------------------------------

    pub fn round(&self) -> u32 {
        self.round
    }

    pub fn config(&self) -> &GameConfig {
        &self.config
    }

    pub fn portals(&self) -> &[Portal] {
        &self.portals
    }

    /// Portal lookup by dense id; `None` for unknown ids.
    pub fn portal(&self, id: PortalId) -> Option<&Portal> {
        self.portals.get(id.0 as usize)
    }

    pub fn agent(&self, id: AgentId) -> Option<&Agent> {
        self.agents.get(&id)
    }

    pub fn agents(&self) -> &BTreeMap<AgentId, Agent> {
        &self.agents
    }

    pub fn tracker(&self, id: AgentId) -> Option<&RoundTracker> {
        self.trackers.get(&id)
    }

    /// Per-`(agent, portal)` HACK tracker (R-HACK-02); `None` until the
    /// pair's first successful hack.
    pub fn hack_tracker(&self, agent: AgentId, portal: PortalId) -> Option<&HackTracker> {
        self.hack_trackers.get(&(agent, portal))
    }

    /// The full HACK tracker table in ascending `(AgentId, PortalId)` order
    /// (D44 section 5): the canonical state hash iterates this map.
    pub fn hack_trackers(&self) -> &BTreeMap<(AgentId, PortalId), HackTracker> {
        &self.hack_trackers
    }

    /// 0-based epoch of a 1-based round (R-HACK-02, D32): rounds
    /// `1..=epoch_length` are epoch 0, `epoch_length+1..=2*epoch_length`
    /// epoch 1, and so on.
    ///
    /// Precondition: `round >= 1` (rounds are 1-based; a 0 round underflows
    /// and panics by design) and `epoch_length >= 1` (guaranteed by
    /// `GameConfig::validate`).
    pub fn epoch_index(round: u32, epoch_length: u32) -> u32 {
        round
            .checked_sub(1)
            .expect("rounds are 1-based")
            .checked_div(epoch_length)
            .expect("epoch_length >= 1 (GameConfig::validate)")
    }

    /// The epoch containing the current round.
    pub fn current_epoch(&self) -> u32 {
        Self::epoch_index(self.round, self.config.epoch_length)
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }

    pub fn scores(&self) -> &BTreeMap<Faction, Score2> {
        &self.scores
    }

    pub fn score(&self, faction: Faction) -> Score2 {
        self.scores.get(&faction).copied().unwrap_or(0)
    }

    /// The link graph with its derived field cache (R-LINK/R-FIELD).
    pub fn link_graph(&self) -> &LinkGraph {
        &self.link_graph
    }

    /// Next unassigned agent id; reinforcement rosters continue from here.
    pub fn next_agent_id(&self) -> AgentId {
        self.next_agent_id
    }

    /// Normal end-of-match result (R-SCORE-01, CONTEXT Draw): the plain
    /// `score2` comparison only — equal scores are a `Draw`, and no other
    /// state (portals, supply, scoring timing) is read as a tie-break.
    pub fn final_result(&self) -> MatchResult {
        let (enl, res) = (self.score(Faction::Enl), self.score(Faction::Res));
        if enl > res {
            MatchResult::EnlWin
        } else if res > enl {
            MatchResult::ResWin
        } else {
            MatchResult::Draw
        }
    }

    /// Diagnostic full-recompute cross-check over the live portal positions
    /// (P1-FIELD); settlement always reads the incrementally maintained
    /// cache instead.
    pub fn fields_full_recompute(
        &self,
    ) -> std::collections::BTreeMap<FieldId, crate::linkgraph::Field> {
        let GameState {
            portals,
            link_graph,
            ..
        } = self;
        link_graph.fields_full_recompute(|p| portals[p.0 as usize].position)
    }

    // ------------------------------------------------------------------
    // Mutators (pub(crate)): resolve.rs decides validity, state.rs mutates.
    // Every `expect` below is guarded by a documented resolver-side check;
    // a panic here is a server bug (D9 discipline).
    // ------------------------------------------------------------------

    /// Moves an agent and bills `cost` onto the round's move budget. The
    /// resolver has already verified bounds and budget headroom.
    pub(crate) fn apply_move(&mut self, agent: AgentId, to: Coord, cost: u64) {
        let tracker = self
            .trackers
            .get_mut(&agent)
            .expect("every agent has a tracker");
        tracker.move_spent = tracker
            .move_spent
            .checked_add(cost)
            .expect("bounded by MOVE_BUDGET (u64 accumulator)");
        self.agents.get_mut(&agent).expect("agent exists").position = to;
    }

    /// Marks the agent's single Major Action allowance as spent, even if the
    /// command later DROPs (D28 / R-ACTION-01).
    pub(crate) fn consume_major(&mut self, agent: AgentId) {
        self.trackers
            .get_mut(&agent)
            .expect("every agent has a tracker")
            .major_used = true;
    }

    /// Applies a successful CAPTURE: owner = agent's faction,
    /// hp = `PORTAL_MAX_HP`, and a `PORTAL_CAPTURED` event (R-CAPTURE-01).
    pub(crate) fn apply_capture(&mut self, agent: AgentId, portal: PortalId) {
        let faction = self.agents.get(&agent).expect("agent exists").faction;
        let max_hp = self.config.portal_max_hp;
        let target = self
            .portals
            .get_mut(portal.0 as usize)
            .expect("dense portal ids");
        target.owner = faction_as_owner(faction);
        target.hp = max_hp;
        self.push_event(EventKind::PortalCaptured {
            faction,
            agent_id: agent,
            portal,
        });
    }

    /// Applies fixed attack damage and returns `(hp_after, neutralized)`
    /// (R-ATTACK-01/02). HP floors at 0 — a game-rule clamp, not an overflow
    /// escape (D26). Event emission stays with the resolver (D25).
    pub(crate) fn apply_attack_damage(&mut self, portal: PortalId, damage: u32) -> (u32, bool) {
        let target = self
            .portals
            .get_mut(portal.0 as usize)
            .expect("dense portal ids");
        let hp_after = target.hp.checked_sub(damage).unwrap_or(0);
        target.hp = hp_after;
        (hp_after, hp_after == 0)
    }

    /// Neutralizes a portal: `owner = Neutral`, `hp = 0` (R-ATTACK-02 step 1)
    /// and the D40 link/field cascade — every incident link and every field
    /// on those links is removed. Returns the cascade inventory (links and
    /// fields, both ascending); event emission stays with the resolver (D25).
    pub(crate) fn neutralize_portal(&mut self, portal: PortalId) -> NeutralizeCascade {
        let cascade = self.link_graph.remove_links_incident(portal);
        let target = self
            .portals
            .get_mut(portal.0 as usize)
            .expect("dense portal ids");
        target.owner = Owner::Neutral;
        target.hp = 0;
        cascade
    }

    /// Applies a resolver-validated LINK (R-LINK-01): consumes EXACTLY one
    /// key of the target portal (the resolver checked the balance; a
    /// depleted entry is removed to keep the inventory canonical), inserts
    /// the undirected edge, and derives the fields it closes with their
    /// cached `area2`. Returns the canonical link id and the derived field
    /// ids ascending. Event emission stays with the resolver.
    pub(crate) fn apply_link(
        &mut self,
        agent: AgentId,
        source: PortalId,
        target: PortalId,
    ) -> (LinkId, Vec<FieldId>) {
        let link = LinkId::new(source, target).expect("resolver validated distinct portals");
        {
            let inventory = &mut self.agents.get_mut(&agent).expect("agent exists").keys;
            let entry = inventory
                .get_mut(&target)
                .expect("resolver validated the target key");
            *entry = entry
                .checked_sub(1)
                .expect("resolver validated the target key");
            if *entry == 0 {
                inventory.remove(&target);
            }
        }
        let faction = self.agents.get(&agent).expect("agent exists").faction;
        let fields = {
            let GameState {
                portals,
                link_graph,
                ..
            } = self;
            link_graph.insert_link(link, faction, |p| portals[p.0 as usize].position)
        };
        (link, fields)
    }

    /// Round-end score settlement (R-SCORE-01): the per-faction delta is the
    /// sum of `area2` over the fields alive at the end of the round (u64 →
    /// u128 checked adds — created-then-destroyed-same-round fields are
    /// absent and score 0), it accumulates into `scores`, and the delta map
    /// is returned for the round report.
    pub(crate) fn settle_scores(&mut self) -> BTreeMap<Faction, Score2> {
        let mut deltas: BTreeMap<Faction, Score2> = BTreeMap::new();
        deltas.insert(Faction::Enl, 0);
        deltas.insert(Faction::Res, 0);
        for field in self.link_graph.fields().values() {
            let entry = deltas
                .get_mut(&field.faction)
                .expect("a field faction is always Enl or Res");
            *entry = entry
                .checked_add(field.area2 as Score2)
                .expect("area2 sum bounded by config caps");
        }
        for (faction, delta) in &deltas {
            let score = self
                .scores
                .get_mut(faction)
                .expect("both factions are always present");
            *score = score
                .checked_add(*delta)
                .expect("score2 bounded by config caps");
        }
        deltas
    }

    /// Spawns one reinforcement block per faction at the shared `position`
    /// (R-SPAWN-02, D42): ENL takes `next..next+k`, then RES
    /// `next+k..next+2k`, ids continuing the match counter (D17
    /// continuation); the counter advances by `2k`. Every agent starts with
    /// `initial_supply`, no keys, and a fresh tracker (hack trackers stay
    /// lazy). Returns the first assigned id (the ENL block start).
    pub(crate) fn add_reinforcement_agents(&mut self, position: Coord, count: u32) -> AgentId {
        let k = count as u64;
        let start = self.next_agent_id.0 as u64;
        let enl_end = start
            .checked_add(k)
            .expect("agent ids bounded by roster growth");
        let res_end = enl_end
            .checked_add(k)
            .expect("agent ids bounded by roster growth");
        for (lo, hi, faction) in [
            (start, enl_end, Faction::Enl),
            (enl_end, res_end, Faction::Res),
        ] {
            for i in lo..hi {
                let id = AgentId(i as u32);
                self.agents.insert(
                    id,
                    Agent {
                        id,
                        faction,
                        position,
                        supply: self.config.initial_supply,
                        keys: BTreeMap::new(),
                    },
                );
                self.trackers.insert(id, RoundTracker::default());
            }
        }
        self.next_agent_id = AgentId(res_end as u32);
        AgentId(start as u32)
    }

    /// Applies a successful RECHARGE: `hp = min(PORTAL_MAX_HP, hp + amount)`
    /// computed in u64, then deducts `RECHARGE_COST` supply (R-RECHARGE-01).
    /// Never emits an event (R-EVENT-01).
    pub(crate) fn apply_recharge(
        &mut self,
        agent: AgentId,
        portal: PortalId,
        amount: u32,
    ) -> (u32, u32) {
        let cost = self.config.recharge_cost_supply;
        let max_hp = self.config.portal_max_hp as u64;
        let target = self
            .portals
            .get_mut(portal.0 as usize)
            .expect("dense portal ids");
        let hp_before = target.hp;
        let sum = (hp_before as u64)
            .checked_add(amount as u64)
            .expect("u64 sum of two u32 values");
        let hp_after = sum.min(max_hp) as u32;
        target.hp = hp_after;
        let owner = self.agents.get_mut(&agent).expect("agent exists");
        owner.supply = owner
            .supply
            .checked_sub(cost)
            .expect("resolver validated supply >= RECHARGE_COST");
        (hp_before, hp_after)
    }

    /// Appends a broadcast event with the next global sequence number and
    /// returns it (D26: `event_seq` is global, 0-based, u64).
    pub(crate) fn push_event(&mut self, kind: EventKind) -> Event {
        let event = Event {
            seq: self.event_seq,
            round: self.round,
            kind,
        };
        self.event_seq = self
            .event_seq
            .checked_add(1)
            .expect("event count bounded by rounds x commands");
        self.events.push(event);
        event
    }

    /// Applies a successful HACK (R-HACK-01, D34): grants `keys_gained`
    /// keys in full (keys are never clamped), adds `supply_yield` supply
    /// CLAMPED at `MAX_SUPPLY` — the clamp is a game rule, not a failure,
    /// and the tracker advances either way — and updates all three tracker
    /// fields (cooldown anchor = current round, epoch count +1, lifetime
    /// ordinal +1). Returns `(supply_granted, supply_after)`.
    pub(crate) fn apply_hack(
        &mut self,
        agent: AgentId,
        portal: PortalId,
        keys_gained: u32,
        supply_yield: u32,
    ) -> (u32, u32) {
        self.grant_keys(agent, portal, keys_gained);
        let cap = self.config.max_supply as u64;
        let owner = self.agents.get_mut(&agent).expect("agent exists");
        let supply_before = owner.supply as u64;
        let sum = supply_before
            .checked_add(supply_yield as u64)
            .expect("u64 sum of two u32 values");
        let supply_after = sum.min(cap) as u32;
        owner.supply = supply_after;
        let supply_granted = (supply_after as u64)
            .checked_sub(supply_before)
            .expect("the clamp never removes supply") as u32;

        let round = self.round;
        let tracker = self.hack_trackers.entry((agent, portal)).or_default();
        tracker.last_hack_round = Some(round);
        tracker.hack_count_epoch = tracker
            .hack_count_epoch
            .checked_add(1)
            .expect("hack count bounded by rounds x commands");
        tracker.hack_ordinal = tracker
            .hack_ordinal
            .checked_add(1)
            .expect("hack ordinal bounded by rounds x commands");
        (supply_granted, supply_after)
    }

    /// Applies a resolver-validated TRANSFER (R-TRANSFER-01, D35): subtracts
    /// the supply and keys from the sender — entries depleted to 0 are
    /// REMOVED so the inventory stays canonical (M5 hashes the map) — and
    /// adds them to the receiver, merging per portal. Zero-count entries are
    /// inert. A self-transfer nets out to no change. The resolver has
    /// already validated every balance and the receiver capacity.
    pub(crate) fn apply_transfer(
        &mut self,
        sender: AgentId,
        target: AgentId,
        supply: u32,
        keys: &BTreeMap<PortalId, u32>,
    ) {
        {
            let from = self.agents.get_mut(&sender).expect("agent exists");
            from.supply = from
                .supply
                .checked_sub(supply)
                .expect("resolver validated sender supply");
            for (&portal, &count) in keys {
                if count == 0 {
                    continue;
                }
                let entry = from
                    .keys
                    .get_mut(&portal)
                    .expect("resolver validated sender key balances");
                *entry = entry
                    .checked_sub(count)
                    .expect("resolver validated sender key balances");
                if *entry == 0 {
                    from.keys.remove(&portal);
                }
            }
        }
        let to = self.agents.get_mut(&target).expect("agent exists");
        to.supply = to
            .supply
            .checked_add(supply)
            .expect("resolver validated receiver capacity");
        for (&portal, &count) in keys {
            if count == 0 {
                continue;
            }
            let entry = to.keys.entry(portal).or_insert(0);
            *entry = entry
                .checked_add(count)
                .expect("key count bounded by portal population");
        }
    }

    /// Advances to the next round and resets every round tracker (D22
    /// close, then next round). When the step crosses an epoch boundary the
    /// hack trackers reset EAGERLY: `last_hack_round` and
    /// `hack_count_epoch` restart while `hack_ordinal` survives (R-HACK-02,
    /// D32) — a Round-100 hack must not block a Round-101 hack. Also clears
    /// the D47 double-settle guard, so the next round may settle.
    ///
    /// Public since M5: the replay engine re-executes recorded rounds and
    /// owns the same run_round → advance_round sequence as the harness.
    pub fn advance_round(&mut self) {
        self.settled_round = None;
        let old_epoch = self.current_epoch();
        self.round = self
            .round
            .checked_add(1)
            .expect("round bounded by total_rounds");
        if self.current_epoch() != old_epoch {
            for tracker in self.hack_trackers.values_mut() {
                tracker.last_hack_round = None;
                tracker.hack_count_epoch = 0;
            }
        }
        for tracker in self.trackers.values_mut() {
            *tracker = RoundTracker::default();
        }
    }

    /// D47 guard hook: marks round `self.round()` as settled. Panics when
    /// the same round would settle twice without an intervening
    /// [`GameState::advance_round`] — since M4 a double settle observably
    /// corrupts scores (`settle_scores` runs again), so it is a programmer
    /// error, not a game result.
    pub(crate) fn begin_settle_round(&mut self) {
        if self.settled_round == Some(self.round) {
            panic!(
                "run_round already settled round {}; call advance_round first",
                self.round
            );
        }
        self.settled_round = Some(self.round);
    }

    /// Hands `count` keys for `portal` to `agent` (key inventory primitive).
    /// HACK is the only rules-legal key source and routes through
    /// [`GameState::apply_hack`]; this stays `pub(crate)` as the shared
    /// mutation used by apply_hack and tests.
    pub(crate) fn grant_keys(&mut self, agent: AgentId, portal: PortalId, count: u32) {
        let inventory = &mut self.agents.get_mut(&agent).expect("agent exists").keys;
        let entry = inventory.entry(portal).or_insert(0);
        *entry = entry.checked_add(count).expect("key count bounded");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spawn::validate_initial_spawn_set;
    use crate::types::Coord;

    fn scenario_config(agents_per_faction: u32) -> GameConfig {
        let mut cfg = GameConfig::default();
        cfg.map_width_cm = 30_000;
        cfg.map_height_cm = 30_000;
        cfg.portal_count = 2;
        cfg.portal_cluster_count = 2;
        cfg.portal_min_distance_cm = 4_000;
        cfg.initial_agents_per_faction = agents_per_faction;
        cfg
    }

    fn two_portal_map() -> MapDefinition {
        let portals = vec![
            Portal {
                id: PortalId(0),
                position: Coord::new(10_000, 10_000),
                owner: Owner::Neutral,
                hp: 0,
            },
            Portal {
                id: PortalId(1),
                position: Coord::new(25_000, 25_000),
                owner: Owner::Neutral,
                hp: 0,
            },
        ];
        MapDefinition::new(portals)
    }

    fn spawns_for(cfg: &GameConfig, faction: Faction, coords: &[Coord]) -> InitialSpawns {
        let (enl, res) = GameState::initial_roster(cfg);
        let roster = if faction == Faction::Enl { &enl } else { &res };
        let entries: Vec<(AgentId, Coord)> =
            roster.iter().copied().zip(coords.iter().copied()).collect();
        validate_initial_spawn_set(roster, &entries, cfg).expect("fixture spawns valid")
    }

    // Checklist 1: constructor invariants.
    #[test]
    fn new_initializes_round_portals_agents_trackers_events_scores() {
        let cfg = scenario_config(1);
        let map = two_portal_map();
        let enl = spawns_for(&cfg, Faction::Enl, &[Coord::new(10_000, 5_000)]);
        let res = spawns_for(&cfg, Faction::Res, &[Coord::new(6_000, 10_000)]);
        let state = GameState::new(cfg.clone(), &map, &enl, &res).expect("state builds");

        assert_eq!(state.round(), 1);
        // Portals copied from the authoritative definition, untouched.
        assert_eq!(state.portals().len(), 2);
        for (copied, source) in state.portals().iter().zip(map.portals().iter()) {
            assert_eq!(copied, source);
            assert_eq!(copied.owner, Owner::Neutral);
            assert_eq!(copied.hp, 0);
        }
        // Both spawn sets applied with the faction split of D17.
        assert_eq!(
            state.agent(AgentId(0)).map(|a| a.position),
            Some(Coord::new(10_000, 5_000))
        );
        assert_eq!(
            state.agent(AgentId(1)).map(|a| a.position),
            Some(Coord::new(6_000, 10_000))
        );
        assert_eq!(
            state.agent(AgentId(0)).map(|a| a.faction),
            Some(Faction::Enl)
        );
        assert_eq!(
            state.agent(AgentId(1)).map(|a| a.faction),
            Some(Faction::Res)
        );
        assert_eq!(
            state.agent(AgentId(0)).map(|a| a.supply),
            Some(cfg.initial_supply)
        );
        assert!(
            state
                .agent(AgentId(0))
                .map(|a| a.keys.is_empty())
                .unwrap_or(false)
        );
        // Trackers zeroed, empty log, zero scores.
        for id in [AgentId(0), AgentId(1)] {
            assert_eq!(state.tracker(id), Some(&RoundTracker::default()));
        }
        assert!(state.events().is_empty());
        assert_eq!(state.score(Faction::Enl), 0);
        assert_eq!(state.score(Faction::Res), 0);
    }

    // Checklist 2: dense roster split ENL 0..n-1, RES n..2n-1.
    #[test]
    fn initial_roster_assigns_dense_ids_enl_then_res() {
        for n in [1u32, 2, 3, 5] {
            let cfg = scenario_config(n);
            let (enl, res) = GameState::initial_roster(&cfg);
            let expected_enl: Vec<AgentId> = (0..n).map(AgentId).collect();
            let expected_res: Vec<AgentId> = (n..2 * n).map(AgentId).collect();
            assert_eq!(enl, expected_enl, "ENL roster for n={n}");
            assert_eq!(res, expected_res, "RES roster for n={n}");
        }
    }

    // Checklist 3: spawn coverage and map validation failures.
    #[test]
    fn new_rejects_spawn_mismatch_and_invalid_map() {
        let cfg = scenario_config(2);
        let map = two_portal_map();

        // ENL set built for a one-agent roster: does not exactly cover the
        // two-agent roster of this config.
        let short_cfg = scenario_config(1);
        let short_enl = spawns_for(&short_cfg, Faction::Enl, &[Coord::new(10_000, 5_000)]);
        let ok_res = spawns_for(&cfg, Faction::Res, &[Coord::new(0, 0), Coord::new(1, 1)]);
        assert_eq!(
            GameState::new(cfg.clone(), &map, &short_enl, &ok_res).unwrap_err(),
            StateError::SpawnSetMismatch(Faction::Enl)
        );

        // RES set covering unknown ids relative to this config's roster.
        let ok_enl = spawns_for(&cfg, Faction::Enl, &[Coord::new(0, 0), Coord::new(1, 1)]);
        let bad_res = spawns_for(&short_cfg, Faction::Res, &[Coord::new(0, 0)]);
        assert_eq!(
            GameState::new(cfg.clone(), &map, &ok_enl, &bad_res).unwrap_err(),
            StateError::SpawnSetMismatch(Faction::Res)
        );

        // Map failing validate against the config (portal count mismatch).
        let one_portal = MapDefinition::new(vec![Portal {
            id: PortalId(0),
            position: Coord::new(10_000, 10_000),
            owner: Owner::Neutral,
            hp: 0,
        }]);
        let ok_enl = spawns_for(&cfg, Faction::Enl, &[Coord::new(0, 0), Coord::new(1, 1)]);
        let ok_res = spawns_for(&cfg, Faction::Res, &[Coord::new(0, 0), Coord::new(1, 1)]);
        assert_eq!(
            GameState::new(cfg.clone(), &one_portal, &ok_enl, &ok_res).unwrap_err(),
            StateError::MapInvalid
        );

        // Invalid config surfaces as InvalidConfig.
        let mut bad_cfg = scenario_config(1);
        bad_cfg.attack_damage = 0;
        let enl = spawns_for(&bad_cfg, Faction::Enl, &[Coord::new(0, 0)]);
        let res = spawns_for(&bad_cfg, Faction::Res, &[Coord::new(0, 0)]);
        assert_eq!(
            GameState::new(bad_cfg, &map, &enl, &res).unwrap_err(),
            StateError::InvalidConfig(ConfigError::HpBounds)
        );
    }

    // Checklist 4: round advance resets trackers after a full round.
    #[test]
    fn advance_round_increments_and_resets_trackers() {
        let cfg = scenario_config(1);
        let map = two_portal_map();
        let enl = spawns_for(&cfg, Faction::Enl, &[Coord::new(10_000, 5_000)]);
        let res = spawns_for(&cfg, Faction::Res, &[Coord::new(6_000, 10_000)]);
        let mut state = GameState::new(cfg, &map, &enl, &res).expect("state builds");

        state.apply_move(AgentId(0), Coord::new(12_000, 5_000), 2_000);
        state.consume_major(AgentId(0));
        state.apply_capture(AgentId(0), PortalId(0));
        assert_eq!(
            state.tracker(AgentId(0)),
            Some(&RoundTracker {
                major_used: true,
                move_spent: 2_000
            })
        );
        assert_eq!(state.events().len(), 1);
        assert_eq!(state.events()[0].round, 1);

        state.advance_round();
        assert_eq!(state.round(), 2);
        assert_eq!(state.tracker(AgentId(0)), Some(&RoundTracker::default()));
        assert_eq!(state.tracker(AgentId(1)), Some(&RoundTracker::default()));
        // Events and the world survive the round boundary.
        assert_eq!(state.events().len(), 1);
        assert_eq!(
            state.agent(AgentId(0)).map(|a| a.position),
            Some(Coord::new(12_000, 5_000))
        );
    }

    #[test]
    fn push_event_assigns_global_zero_based_sequences() {
        let cfg = scenario_config(1);
        let map = two_portal_map();
        let enl = spawns_for(&cfg, Faction::Enl, &[Coord::new(10_000, 5_000)]);
        let res = spawns_for(&cfg, Faction::Res, &[Coord::new(6_000, 10_000)]);
        let mut state = GameState::new(cfg, &map, &enl, &res).expect("state builds");

        let kinds = [
            EventKind::PortalCaptured {
                faction: Faction::Enl,
                agent_id: AgentId(0),
                portal: PortalId(0),
            },
            EventKind::PortalAttacked {
                faction: Faction::Res,
                agent_id: AgentId(1),
                portal: PortalId(0),
                damage: 20,
                hp_after: 80,
            },
            EventKind::PortalNeutralized {
                faction: Faction::Res,
                agent_id: AgentId(1),
                portal: PortalId(0),
            },
        ];
        for (expected_seq, kind) in kinds.iter().enumerate() {
            let event = state.push_event(*kind);
            assert_eq!(event.seq, expected_seq as u64);
            assert_eq!(event.round, state.round());
        }
        let seqs: Vec<u64> = state.events().iter().map(|e| e.seq).collect();
        assert_eq!(seqs, vec![0, 1, 2]);
    }

    #[test]
    fn attack_damage_floors_at_zero_and_reports_neutralization() {
        let cfg = scenario_config(1);
        let map = two_portal_map();
        let enl = spawns_for(&cfg, Faction::Enl, &[Coord::new(10_000, 5_000)]);
        let res = spawns_for(&cfg, Faction::Res, &[Coord::new(6_000, 10_000)]);
        let mut state = GameState::new(cfg, &map, &enl, &res).expect("state builds");

        state.apply_capture(AgentId(0), PortalId(0)); // hp = 100
        assert_eq!(state.apply_attack_damage(PortalId(0), 40), (60, false));
        assert_eq!(state.apply_attack_damage(PortalId(0), 60), (0, true));
        // Overkill clamps at 0 (game clamp, D26).
        assert_eq!(state.apply_attack_damage(PortalId(0), 5), (0, true));

        state.neutralize_portal(PortalId(0));
        assert_eq!(
            state.portal(PortalId(0)).map(|p| (p.owner, p.hp)),
            Some((Owner::Neutral, 0))
        );
    }

    #[test]
    fn recharge_clamps_at_max_and_consumes_cost() {
        let cfg = scenario_config(1);
        let map = two_portal_map();
        let enl = spawns_for(&cfg, Faction::Enl, &[Coord::new(10_000, 5_000)]);
        let res = spawns_for(&cfg, Faction::Res, &[Coord::new(6_000, 10_000)]);
        let mut state = GameState::new(cfg, &map, &enl, &res).expect("state builds");

        state.apply_capture(AgentId(0), PortalId(0)); // hp 100, supply 20
        // Full HP recharge clamps at max but still costs supply.
        assert_eq!(
            state.apply_recharge(AgentId(0), PortalId(0), 15),
            (100, 100)
        );
        assert_eq!(state.agent(AgentId(0)).map(|a| a.supply), Some(19));
        // Partial damage then recharge: exact arithmetic, u64 sum.
        state.apply_attack_damage(PortalId(0), 75); // hp 25
        assert_eq!(state.apply_recharge(AgentId(0), PortalId(0), 15), (25, 40));
        assert_eq!(state.agent(AgentId(0)).map(|a| a.supply), Some(18));
    }

    // ------------------------------------------------------------------
    // M3: epoch model and hack trackers (checklist 1-2)
    // ------------------------------------------------------------------

    // Checklist 1: epoch derivation boundaries (D32, R-HACK-02).
    #[test]
    fn epoch_index_boundaries() {
        // Default EPOCH_LENGTH 100: rounds 1-100 -> epoch 0, 101-200 -> 1,
        // 201 -> 2 (0-based epochs).
        for (round, epoch) in [
            (1u32, 0u32),
            (99, 0),
            (100, 0),
            (101, 1),
            (200, 1),
            (201, 2),
            (500, 4),
        ] {
            assert_eq!(GameState::epoch_index(round, 100), epoch, "round {round}");
        }
        // A non-default length shifts the boundaries the same way.
        for (round, epoch) in [(1u32, 0u32), (3, 0), (4, 1), (6, 1), (7, 2)] {
            assert_eq!(GameState::epoch_index(round, 3), epoch, "round {round}");
        }
        // Epoch length 1: every round is its own epoch.
        for round in 1..=3u32 {
            assert_eq!(GameState::epoch_index(round, 1), round - 1);
        }
    }

    // Checklist 1: current_epoch() derives from the live round.
    #[test]
    fn current_epoch_tracks_the_round() {
        let cfg = scenario_config(1);
        let map = two_portal_map();
        let enl = spawns_for(&cfg, Faction::Enl, &[Coord::new(10_000, 5_000)]);
        let res = spawns_for(&cfg, Faction::Res, &[Coord::new(6_000, 10_000)]);
        let mut state = GameState::new(cfg, &map, &enl, &res).expect("state builds");
        assert_eq!(state.current_epoch(), 0);
        for _ in 1..100 {
            state.advance_round();
        }
        assert_eq!(state.round(), 100);
        assert_eq!(state.current_epoch(), 0);
        state.advance_round();
        assert_eq!(state.round(), 101);
        assert_eq!(state.current_epoch(), 1);
    }

    // Checklist 2: the eager epoch sweep in advance_round (D32) resets the
    // cooldown anchor and the per-epoch count but PRESERVES hack_ordinal.
    #[test]
    fn advance_round_epoch_sweep_resets_cooldown_and_count_preserves_ordinal() {
        let mut cfg = scenario_config(1);
        cfg.epoch_length = 4;
        let map = two_portal_map();
        let enl = spawns_for(&cfg, Faction::Enl, &[Coord::new(10_000, 5_000)]);
        let res = spawns_for(&cfg, Faction::Res, &[Coord::new(6_000, 10_000)]);
        let mut state = GameState::new(cfg, &map, &enl, &res).expect("state builds");

        // Advance to round 4, the last round of epoch 0.
        for _ in 1..4 {
            state.advance_round();
        }
        assert_eq!(state.round(), 4);
        assert_eq!(state.current_epoch(), 0);
        state.apply_hack(AgentId(0), PortalId(0), 2, 1);
        assert_eq!(
            state.hack_tracker(AgentId(0), PortalId(0)),
            Some(&HackTracker {
                last_hack_round: Some(4),
                hack_count_epoch: 1,
                hack_ordinal: 1,
            })
        );

        // Crossing into epoch 1 eagerly resets cooldown and count.
        state.advance_round();
        assert_eq!(state.current_epoch(), 1);
        assert_eq!(
            state.hack_tracker(AgentId(0), PortalId(0)),
            Some(&HackTracker {
                last_hack_round: None,
                hack_count_epoch: 0,
                hack_ordinal: 1,
            })
        );

        // A same-epoch advance (5 -> 6) resets nothing.
        state.apply_hack(AgentId(0), PortalId(0), 1, 1);
        state.advance_round();
        assert_eq!(
            state.hack_tracker(AgentId(0), PortalId(0)),
            Some(&HackTracker {
                last_hack_round: Some(5),
                hack_count_epoch: 1,
                hack_ordinal: 2,
            })
        );
    }

    // State-level HACK contract (D34): keys in full, supply clamped in u64,
    // tracker fully advanced even when the clamp bites.
    #[test]
    fn apply_hack_grants_keys_clamps_supply_and_updates_tracker() {
        let mut cfg = scenario_config(1);
        cfg.initial_supply = 100; // at MAX_SUPPLY
        let map = two_portal_map();
        let enl = spawns_for(&cfg, Faction::Enl, &[Coord::new(10_000, 5_000)]);
        let res = spawns_for(&cfg, Faction::Res, &[Coord::new(6_000, 10_000)]);
        let mut state = GameState::new(cfg, &map, &enl, &res).expect("state builds");

        let (granted, after) = state.apply_hack(AgentId(0), PortalId(1), 2, 1);
        assert_eq!((granted, after), (0, 100), "clamped at MAX_SUPPLY");
        assert_eq!(
            state
                .agent(AgentId(0))
                .map(|a| a.keys.get(&PortalId(1)).copied()),
            Some(Some(2)),
            "keys are never clamped"
        );
        assert_eq!(
            state.hack_tracker(AgentId(0), PortalId(1)),
            Some(&HackTracker {
                last_hack_round: Some(1),
                hack_count_epoch: 1,
                hack_ordinal: 1,
            })
        );
    }

    // State-level TRANSFER contract (D35): resources move, sender entries
    // depleted to 0 vanish, receiver entries merge per portal.
    #[test]
    fn apply_transfer_moves_resources_and_keeps_inventory_canonical() {
        let cfg = scenario_config(2);
        let map = two_portal_map();
        let enl = spawns_for(
            &cfg,
            Faction::Enl,
            &[Coord::new(10_000, 5_000), Coord::new(10_000, 6_000)],
        );
        let res = spawns_for(
            &cfg,
            Faction::Res,
            &[Coord::new(6_000, 10_000), Coord::new(6_000, 11_000)],
        );
        let mut state = GameState::new(cfg, &map, &enl, &res).expect("state builds");

        state.grant_keys(AgentId(0), PortalId(0), 3);
        state.grant_keys(AgentId(0), PortalId(1), 2);
        state.grant_keys(AgentId(1), PortalId(0), 1); // existing entry to merge into
        let mut keys = BTreeMap::new();
        keys.insert(PortalId(0), 3u32);
        keys.insert(PortalId(1), 2u32);
        state.apply_transfer(AgentId(0), AgentId(1), 5, &keys);

        // Sender: supply 20-5, both key entries depleted to 0 are GONE.
        assert_eq!(state.agent(AgentId(0)).map(|a| a.supply), Some(15));
        assert!(
            state
                .agent(AgentId(0))
                .map(|a| a.keys.is_empty())
                .unwrap_or(false),
            "depleted key entries are removed from the inventory"
        );
        // Receiver: supply 20+5, P0 merged 1+3, P1 fresh 2.
        assert_eq!(state.agent(AgentId(1)).map(|a| a.supply), Some(25));
        assert_eq!(
            state
                .agent(AgentId(1))
                .map(|a| a.keys.get(&PortalId(0)).copied()),
            Some(Some(4))
        );
        assert_eq!(
            state
                .agent(AgentId(1))
                .map(|a| a.keys.get(&PortalId(1)).copied()),
            Some(Some(2))
        );
    }

    // ------------------------------------------------------------------
    // M4: link graph wiring, cascade, settlement, reinforcement (12-17)
    // ------------------------------------------------------------------

    /// Six portals, pairwise >= 4_000 cm apart, on a 40_000 cm map; one
    /// agent per faction (ENL AgentId(0), RES AgentId(1)) both at (0, 0).
    fn field_state() -> GameState {
        let coords = [
            (4_000, 4_000),
            (20_000, 4_000),
            (4_000, 20_000),
            (20_000, 20_000),
            (36_000, 4_000),
            (4_000, 30_000),
        ];
        let mut cfg = GameConfig::default();
        cfg.map_width_cm = 40_000;
        cfg.map_height_cm = 40_000;
        cfg.portal_min_distance_cm = 4_000;
        cfg.portal_count = coords.len() as u32;
        cfg.portal_cluster_count = coords.len() as u32;
        cfg.initial_agents_per_faction = 1;
        let portals: Vec<Portal> = coords
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| Portal {
                id: PortalId(i as u32),
                position: Coord::new(x, y),
                owner: Owner::Neutral,
                hp: 0,
            })
            .collect();
        let map = MapDefinition::new(portals);
        let enl = spawns_for(&cfg, Faction::Enl, &[Coord::new(0, 0)]);
        let res = spawns_for(&cfg, Faction::Res, &[Coord::new(0, 0)]);
        GameState::new(cfg, &map, &enl, &res).expect("state builds")
    }

    fn link_of(a: u32, b: u32) -> LinkId {
        LinkId::new(PortalId(a), PortalId(b)).expect("distinct portals")
    }

    fn field_of(a: u32, b: u32, c: u32) -> FieldId {
        FieldId::new(PortalId(a), PortalId(b), PortalId(c)).expect("distinct portals")
    }

    // Checklist 12: apply_link consumes EXACTLY one target key, leaves the
    // source keys alone, and inserts edge + adjacency.
    #[test]
    fn apply_link_consumes_exactly_one_target_key() {
        let mut state = field_state();
        state.grant_keys(AgentId(0), PortalId(1), 2);
        state.grant_keys(AgentId(0), PortalId(0), 3);
        let (link, fields) = state.apply_link(AgentId(0), PortalId(0), PortalId(1));
        assert_eq!(link, link_of(0, 1));
        assert!(fields.is_empty(), "no triangle closed yet");
        let agent = state.agent(AgentId(0)).expect("agent exists");
        assert_eq!(agent.keys.get(&PortalId(1)), Some(&1), "2 keys -> 1");
        assert_eq!(agent.keys.get(&PortalId(0)), Some(&3), "source untouched");
        assert_eq!(
            state.link_graph().link(link_of(0, 1)).map(|l| l.faction),
            Some(Faction::Enl)
        );
        assert_eq!(
            state.link_graph().neighbors(PortalId(0)),
            [PortalId(1)].into_iter().collect()
        );
        assert_eq!(
            state.link_graph().neighbors(PortalId(1)),
            [PortalId(0)].into_iter().collect()
        );
    }

    // Checklist 13: the cascade inventory is ascending and a layered field
    // spanning TWO removed edges is listed exactly once.
    #[test]
    fn neutralize_portal_returns_cascade_inventory_ascending() {
        let mut state = field_state();
        // Two triangles sharing only portal 2: (0,1,2) and (2,3,4). The field
        // (2,3,4) spans two of the removed edges (2-3 and 2-4).
        for &(a, b) in &[(0u32, 1u32), (0, 2), (1, 2), (2, 3), (2, 4), (3, 4)] {
            state.grant_keys(AgentId(0), PortalId(b), 1);
            state.apply_link(AgentId(0), PortalId(a), PortalId(b));
        }
        assert_eq!(
            state
                .link_graph()
                .fields()
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            vec![field_of(0, 1, 2), field_of(2, 3, 4)]
        );
        state.apply_capture(AgentId(0), PortalId(2));

        let cascade = state.neutralize_portal(PortalId(2));
        assert_eq!(
            cascade.links,
            vec![link_of(0, 2), link_of(1, 2), link_of(2, 3), link_of(2, 4)],
            "incident links ascending LinkId"
        );
        assert_eq!(
            cascade.fields,
            vec![field_of(0, 1, 2), field_of(2, 3, 4)],
            "dead fields ascending FieldId, each listed once"
        );
        assert_eq!(
            state.portal(PortalId(2)).map(|p| (p.owner, p.hp)),
            Some((Owner::Neutral, 0))
        );
        assert!(state.link_graph().fields().is_empty());
        // The edge (0,1) and (3,4) survive — no incident removal.
        assert!(state.link_graph().link(link_of(0, 1)).is_some());
        assert!(state.link_graph().link(link_of(3, 4)).is_some());
    }

    // Checklist 14: settlement sums cached area2 per faction into u128 and
    // returns the round delta; an empty cache yields zero deltas.
    #[test]
    fn settle_scores_accumulates_u128_and_returns_deltas() {
        let mut state = field_state();
        // ENL field (0,1,2): right triangle, legs 16_000 -> area2 256_000_000.
        for &(a, b) in &[(0u32, 1u32), (0, 2), (1, 2)] {
            state.grant_keys(AgentId(0), PortalId(b), 1);
            state.apply_link(AgentId(0), PortalId(a), PortalId(b));
        }
        // RES field (3,4,5): cross = 16_000*10_000 - 16_000*16_000 -> 96_000_000.
        for &(a, b) in &[(3u32, 4u32), (3, 5), (4, 5)] {
            state.grant_keys(AgentId(1), PortalId(b), 1);
            state.apply_link(AgentId(1), PortalId(a), PortalId(b));
        }

        let delta = state.settle_scores();
        assert_eq!(delta.get(&Faction::Enl), Some(&256_000_000u128));
        assert_eq!(delta.get(&Faction::Res), Some(&96_000_000u128));
        assert_eq!(state.score(Faction::Enl), 256_000_000u128);
        assert_eq!(state.score(Faction::Res), 96_000_000u128);

        // The next round settles the same surviving fields again.
        let delta = state.settle_scores();
        assert_eq!(delta.get(&Faction::Enl), Some(&256_000_000u128));
        assert_eq!(state.score(Faction::Enl), 512_000_000u128);
        assert_eq!(state.score(Faction::Res), 192_000_000u128);

        // An empty cache settles to zero deltas without touching scores.
        let mut fresh = field_state();
        let delta = fresh.settle_scores();
        assert_eq!(delta.get(&Faction::Enl), Some(&0));
        assert_eq!(delta.get(&Faction::Res), Some(&0));
    }

    // R-SCORE-01 counts EVERY surviving field separately: two layered
    // fields of ONE faction (both built on the shared link 1-3) each
    // contribute their own area2 — the geometric overlap never merges or
    // discounts them. Direct closure of the score2-overlap-counts-separately
    // checklist behavior.
    #[test]
    fn settle_scores_counts_layered_fields_of_one_faction_separately() {
        let mut state = field_state();
        // ENL closes two fields stacked on the shared link (1,3): field
        // (0,1,3) on the left, field (1,3,4) on the right (R-FIELD-02).
        for &(a, b) in &[(0u32, 1u32), (0, 3), (1, 3), (1, 4), (3, 4)] {
            state.grant_keys(AgentId(0), PortalId(b), 1);
            let (_, derived) = state.apply_link(AgentId(0), PortalId(a), PortalId(b));
            match (a, b) {
                (1, 3) => assert_eq!(derived, vec![field_of(0, 1, 3)]),
                (3, 4) => assert_eq!(derived, vec![field_of(1, 3, 4)]),
                _ => assert!(derived.is_empty(), "edge {a}-{b} closes nothing"),
            }
        }
        // Exactly the two layered fields live in the cache — distinct
        // FieldIds despite sharing the edge (1,3) — each with its own
        // hand-computed area2: (0,1,3) = |4k*(4k-20k)+20k*(20k-4k)| and
        // (1,3,4) = |20k*(20k-4k)+36k*(4k-20k)|, both 256_000_000.
        let fields = state.link_graph().fields();
        assert_eq!(
            fields.keys().copied().collect::<Vec<_>>(),
            vec![field_of(0, 1, 3), field_of(1, 3, 4)]
        );
        let layered_a = fields.get(&field_of(0, 1, 3)).expect("first layered field");
        let layered_b = fields
            .get(&field_of(1, 3, 4))
            .expect("second layered field");
        assert_eq!(layered_a.faction, Faction::Enl);
        assert_eq!(layered_b.faction, Faction::Enl);
        assert_eq!(layered_a.area2, 256_000_000u64);
        assert_eq!(layered_b.area2, 256_000_000u64);

        // Settlement sums BOTH overlapping areas into one faction delta.
        let expected = layered_a.area2 as Score2 + layered_b.area2 as Score2;
        let delta = state.settle_scores();
        assert_eq!(delta.get(&Faction::Enl), Some(&expected));
        assert_eq!(delta.get(&Faction::Enl), Some(&512_000_000u128));
        assert_eq!(delta.get(&Faction::Res), Some(&0));
        assert_eq!(state.score(Faction::Enl), 512_000_000u128);
        assert_eq!(state.score(Faction::Res), 0);

        // The layered pair keeps scoring every round it survives.
        let delta = state.settle_scores();
        assert_eq!(delta.get(&Faction::Enl), Some(&512_000_000u128));
        assert_eq!(state.score(Faction::Enl), 1_024_000_000u128);
    }

    // Checklist 15: final_result reads the score comparison only.
    #[test]
    fn final_result_compares_scores_only() {
        // Nothing scored: Draw.
        let mut state = field_state();
        assert_eq!(state.final_result(), MatchResult::Draw);

        // ENL field only: EnlWin.
        for &(a, b) in &[(0u32, 1u32), (0, 2), (1, 2)] {
            state.grant_keys(AgentId(0), PortalId(b), 1);
            state.apply_link(AgentId(0), PortalId(a), PortalId(b));
        }
        state.settle_scores();
        assert_eq!(state.final_result(), MatchResult::EnlWin);

        // RES field only — while ENL owns every portal: ownership is never a
        // tie-break, ResWin regardless.
        let mut state = field_state();
        for &(a, b) in &[(3u32, 4u32), (3, 5), (4, 5)] {
            state.grant_keys(AgentId(1), PortalId(b), 1);
            state.apply_link(AgentId(1), PortalId(a), PortalId(b));
        }
        state.settle_scores();
        assert_eq!(state.final_result(), MatchResult::ResWin);

        // Both score equally: Draw. RES takes the triangle (1,3,4) — the
        // same 16_000 x 16_000 leg shape as ENL's (0,1,2), so both area2
        // values are exactly 256_000_000.
        let mut state = field_state();
        for &(a, b) in &[(0u32, 1u32), (0, 2), (1, 2)] {
            state.grant_keys(AgentId(0), PortalId(b), 1);
            state.apply_link(AgentId(0), PortalId(a), PortalId(b));
        }
        for &(a, b) in &[(1u32, 3u32), (1, 4), (3, 4)] {
            state.grant_keys(AgentId(1), PortalId(b), 1);
            state.apply_link(AgentId(1), PortalId(a), PortalId(b));
        }
        state.settle_scores();
        assert_eq!(state.score(Faction::Enl), state.score(Faction::Res));
        assert_eq!(state.final_result(), MatchResult::Draw);
    }

    // Checklist 16: reinforcement blocks are consecutive, ENL first, with
    // fresh trackers, initial supply, no keys, and one shared position.
    #[test]
    fn add_reinforcement_agents_assigns_consecutive_blocks_enl_then_res() {
        let mut state = field_state();
        assert_eq!(state.next_agent_id(), AgentId(2), "one agent per faction");
        let position = Coord::new(7_000, 7_000);

        let first = state.add_reinforcement_agents(position, 2);
        assert_eq!(first, AgentId(2));
        assert_eq!(state.next_agent_id(), AgentId(6), "counter advanced by 2k");
        for (id, faction) in [
            (AgentId(2), Faction::Enl),
            (AgentId(3), Faction::Enl),
            (AgentId(4), Faction::Res),
            (AgentId(5), Faction::Res),
        ] {
            let agent = state.agent(id).expect("reinforcement agent exists");
            assert_eq!(agent.faction, faction, "{id:?}");
            assert_eq!(agent.position, position, "both factions share one position");
            assert_eq!(agent.supply, state.config().initial_supply);
            assert!(agent.keys.is_empty(), "reinforcements carry no keys");
            assert_eq!(state.tracker(id), Some(&RoundTracker::default()));
        }

        // The next block continues the counter.
        let first = state.add_reinforcement_agents(position, 1);
        assert_eq!(first, AgentId(6));
        assert_eq!(
            state.agent(AgentId(6)).map(|a| a.faction),
            Some(Faction::Enl)
        );
        assert_eq!(
            state.agent(AgentId(7)).map(|a| a.faction),
            Some(Faction::Res)
        );
        assert_eq!(state.next_agent_id(), AgentId(8));
    }

    // Checklist 17: link_graph and next_agent_id are part of the PartialEq
    // determinism surface — diverging runs compare unequal.
    #[test]
    fn new_state_fields_participate_in_partial_eq() {
        // A link on one side only.
        let mut linked = field_state();
        let plain = field_state();
        assert_eq!(linked, plain, "identical fixtures start equal");
        linked.grant_keys(AgentId(0), PortalId(1), 1);
        linked.apply_link(AgentId(0), PortalId(0), PortalId(1));
        assert_ne!(linked, plain, "the link graph participates in equality");

        // A reinforcement on one side only.
        let mut reinforced = field_state();
        let quiet = field_state();
        reinforced.add_reinforcement_agents(Coord::new(1_000, 1_000), 1);
        assert_ne!(reinforced, quiet, "next_agent_id participates in equality");
    }

    // ------------------------------------------------------------------
    // M5: replay support — hack tracker enumeration (D44 section 5) and the
    // run_round same-round double-call guard (D47, M4 review debt).
    // ------------------------------------------------------------------

    // The full tracker table enumerates every pair ascending, matching the
    // per-pair getter (the canonical hash iterates this map).
    #[test]
    fn hack_trackers_enumerates_every_pair_ascending() {
        let mut state = field_state();
        assert!(state.hack_trackers().is_empty(), "lazy until first hack");
        state.apply_hack(AgentId(0), PortalId(3), 1, 1);
        state.apply_hack(AgentId(1), PortalId(1), 2, 1);
        state.apply_hack(AgentId(0), PortalId(1), 1, 1);
        let keys: Vec<(AgentId, PortalId)> = state.hack_trackers().keys().copied().collect();
        assert_eq!(
            keys,
            vec![
                (AgentId(0), PortalId(1)),
                (AgentId(0), PortalId(3)),
                (AgentId(1), PortalId(1))
            ],
            "BTreeMap iteration is ascending"
        );
        for (key, tracker) in state.hack_trackers() {
            assert_eq!(state.hack_tracker(key.0, key.1), Some(tracker));
        }
    }

    // D47: a second run_round on the same round panics naming the round;
    // advance_round clears the guard so the next round settles fine.
    #[test]
    fn run_round_double_call_panics_and_advance_clears_guard() {
        use crate::resolve::{Resolver, SubmittedCommandStream};
        use crate::types::Command;

        let cfg = scenario_config(1);
        let map = two_portal_map();
        let enl = spawns_for(&cfg, Faction::Enl, &[Coord::new(10_000, 5_000)]);
        let res = spawns_for(&cfg, Faction::Res, &[Coord::new(6_000, 10_000)]);
        let mut state = GameState::new(cfg, &map, &enl, &res).expect("state builds");
        let resolver = Resolver::new();

        // Round 1 settles once...
        let _ = resolver.run_round(
            &mut state,
            &[SubmittedCommandStream {
                round: 1,
                faction: Faction::Enl,
                agent_id: AgentId(0),
                received_at_ms: 0,
                receive_seq: 0,
                commands: vec![Command::Capture {
                    portal: PortalId(0),
                }],
            }],
        );
        // ...and a same-round double call is a programmer error (M4 review:
        // a second settle_scores observably corrupts scores).
        let double = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = resolver.run_round(&mut state, &[]);
        }))
        .expect_err("the second same-round run_round must panic");
        let message = double
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| double.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default();
        assert!(
            message.contains("round 1"),
            "panic names the settled round: {message}"
        );

        // advance_round clears the guard: round 2 settles normally, and a
        // fresh double call names round 2.
        state.advance_round();
        let _ = resolver.run_round(&mut state, &[]);
        let double = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = resolver.run_round(&mut state, &[]);
        }))
        .expect_err("the second same-round run_round must panic");
        let message = double
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| double.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default();
        assert!(
            message.contains("round 2"),
            "panic names the settled round: {message}"
        );
    }

    // settle_scores stays a directly callable primitive (state tests call it
    // twice legitimately): the guard covers the run_round path only.
    #[test]
    fn guard_does_not_reach_direct_settle_scores() {
        let mut state = field_state();
        for &(a, b) in &[(0u32, 1u32), (0, 2), (1, 2)] {
            state.grant_keys(AgentId(0), PortalId(b), 1);
            state.apply_link(AgentId(0), PortalId(a), PortalId(b));
        }
        let _ = state.settle_scores();
        let _ = state.settle_scores();
        assert_eq!(state.score(Faction::Enl), 512_000_000u128);
    }
}
