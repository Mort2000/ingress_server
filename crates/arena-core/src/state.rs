//! Mutable match state, per-round trackers, and the mutation surface used
//! by the resolver (P1-LOOP). `GameState` owns every authoritative mutation:
//! [`crate::resolve::Resolver`] decides validity, this module applies it.

use std::collections::BTreeMap;

use crate::config::{ConfigError, GameConfig};
use crate::event::{Event, EventKind};
use crate::map::MapDefinition;
use crate::spawn::{InitialSpawns, validate_initial_spawn_set};
use crate::types::{Agent, AgentId, Coord, Faction, Owner, Portal, PortalId, Score2};

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
/// the public event log, and faction scores. Scores stay zero until fields
/// exist (M4); the map definition is copied as the authoritative input.
#[derive(Debug)]
pub struct GameState {
    config: GameConfig,
    round: u32,
    portals: Vec<Portal>,
    agents: BTreeMap<AgentId, Agent>,
    trackers: BTreeMap<AgentId, RoundTracker>,
    events: Vec<Event>,
    event_seq: u64,
    scores: BTreeMap<Faction, Score2>,
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

        Ok(GameState {
            config: cfg,
            round: 1,
            portals: map.portals().to_vec(),
            agents,
            trackers,
            events: Vec::new(),
            event_seq: 0,
            scores,
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

    pub fn events(&self) -> &[Event] {
        &self.events
    }

    pub fn scores(&self) -> &BTreeMap<Faction, Score2> {
        &self.scores
    }

    pub fn score(&self, faction: Faction) -> Score2 {
        self.scores.get(&faction).copied().unwrap_or(0)
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

    /// Neutralizes a portal: `owner = Neutral`, `hp = 0` (R-ATTACK-02 step 1).
    /// M4 extension point: link deletion and field cascades hook in here.
    pub(crate) fn neutralize_portal(&mut self, portal: PortalId) {
        let target = self
            .portals
            .get_mut(portal.0 as usize)
            .expect("dense portal ids");
        target.owner = Owner::Neutral;
        target.hp = 0;
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

    /// Advances to the next round and resets every tracker (D22 close, then
    /// next round). Called by the harness after the resolver finishes.
    pub(crate) fn advance_round(&mut self) {
        self.round = self
            .round
            .checked_add(1)
            .expect("round bounded by total_rounds");
        for tracker in self.trackers.values_mut() {
            *tracker = RoundTracker::default();
        }
    }

    /// Test/M3 bridge: hands `count` keys for `portal` to `agent`. HACK (M3)
    /// is the only rules-legal key source; the resolver never calls this.
    /// The `cfg(test)` gate comes off in M3 when HACK lands.
    #[cfg(test)]
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
}
