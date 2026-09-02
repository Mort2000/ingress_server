//! Shared fixtures for the arena-bots integration tests: the hand-placed
//! two-triangle map (the M5 replay fixture), an idle dummy opponent, a
//! recording strategy wrapper, and the bot-driven match helpers.

// Each test binary compiles this module separately and uses only part of
// it, so cross-binary "dead" items are expected.
#![allow(dead_code)]

use std::collections::BTreeMap;

use arena_bots::bot::{Bot, BotKind, make_bot};
use arena_core::config::GameConfig;
use arena_core::harness::{AgentSubmission, MatchHarness, RoundStrategy};
use arena_core::map::{MapDefinition, generate_map};
use arena_core::observe::Observation;
use arena_core::spawn::{InitialSpawns, validate_initial_spawn_set};
use arena_core::state::GameState;
use arena_core::types::{
    AgentId, Command, Coord, Faction, FieldId, LinkId, Owner, Portal, PortalId, Score2,
};

/// Default config except a 40_000 cm map with six hand-placed portals and
/// three agents per faction; every triangle edge is within one
/// `MOVE_BUDGET` hop.
pub fn fixture_cfg() -> GameConfig {
    let mut cfg = GameConfig::default();
    cfg.map_width_cm = 40_000;
    cfg.map_height_cm = 40_000;
    cfg.portal_count = 6;
    cfg.portal_cluster_count = 6;
    cfg.portal_min_distance_cm = 4_000;
    cfg.initial_agents_per_faction = 3;
    cfg
}

/// Triangle A = (P0, P1, P2), triangle B = (P3, P4, P5): portal-disjoint,
/// no edge crossings, no portal on a foreign segment.
pub fn fixture_map() -> MapDefinition {
    let coords = [
        (4_000, 4_000),   // P0 (A)
        (16_000, 4_000),  // P1 (A)
        (4_000, 16_000),  // P2 (A)
        (36_000, 4_000),  // P3 (B)
        (36_000, 16_000), // P4 (B)
        (24_000, 16_000), // P5 (B)
    ];
    MapDefinition::new(
        coords
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| Portal {
                id: PortalId(i as u32),
                position: Coord::new(x, y),
                owner: Owner::Neutral,
                hp: 0,
            })
            .collect(),
    )
}

/// A generated default-config map (the M4/M5 pinned seed-0 map).
pub fn generated_map() -> MapDefinition {
    generate_map(0, &GameConfig::default()).expect("the pinned map generates")
}

/// The dummy opponent: every agent idles (auto-PASS) every round.
pub struct IdleStrategy;

impl RoundStrategy for IdleStrategy {
    fn submit_round(&mut self, _obs: &Observation) -> Vec<AgentSubmission> {
        Vec::new()
    }
}

/// Wraps a `RoundStrategy`, recording every submission it produces (the
/// determinism gate compares two recordings command-for-command).
pub struct Recording<S> {
    inner: S,
    pub rounds: Vec<Vec<AgentSubmission>>,
}

impl<S: RoundStrategy> Recording<S> {
    pub fn new(inner: S) -> Self {
        Recording {
            inner,
            rounds: Vec::new(),
        }
    }
}

impl<S: RoundStrategy> RoundStrategy for Recording<S> {
    fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission> {
        let submissions = self.inner.submit_round(obs);
        self.rounds.push(submissions.clone());
        submissions
    }
}

/// Validates a bot's `choose_spawns` output for `faction` (a rejected set
/// is a bot bug and fails the test loudly).
pub fn validate_bot_spawns(
    cfg: &GameConfig,
    faction: Faction,
    map: &MapDefinition,
    entries: &[(AgentId, Coord)],
) -> InitialSpawns {
    let (enl_roster, res_roster) = GameState::initial_roster(cfg);
    let roster = if faction == Faction::Enl {
        enl_roster
    } else {
        res_roster
    };
    validate_initial_spawn_set(&roster, entries, cfg).expect("bot spawn set validates")
}

/// The faction's initial roster ids.
pub fn roster(cfg: &GameConfig, faction: Faction) -> Vec<AgentId> {
    let (enl, res) = GameState::initial_roster(cfg);
    if faction == Faction::Enl { enl } else { res }
}

/// Runs one bot (ENL) against a scripted RES strategy for `rounds` rounds,
/// on a fixture map. The SAME bot instance chooses spawns and then plays
/// (D53: spawn draws precede round draws on one stream). Returns the
/// per-round reports plus the final state.
pub fn run_bot_match(
    cfg: &GameConfig,
    map: &MapDefinition,
    enl_kind: BotKind,
    res: Box<dyn RoundStrategy>,
    res_spawns: InitialSpawns,
    game_seed: u64,
    bot_seed: u64,
    rounds: u32,
) -> (Vec<arena_core::resolve::RoundReport>, GameState) {
    let mut enl = make_bot(enl_kind, Faction::Enl, bot_seed, cfg, map);
    let enl_roster = roster(cfg, Faction::Enl);
    let entries = enl.choose_spawns(cfg, map, &enl_roster);
    let enl_spawns = validate_bot_spawns(cfg, Faction::Enl, map, &entries);
    let mut harness = MatchHarness::new(
        game_seed,
        *cfg,
        map,
        &enl_spawns,
        &res_spawns,
        Box::new(arena_bots::bot::BotStrategy(enl)),
        res,
    )
    .expect("fixture match builds");
    let reports = harness.run_rounds(rounds);
    let state = harness.into_state();
    (reports, state)
}

/// Hand-builds an `Observation` (unit fixtures inspect single decisions).
pub struct ObsBuilder {
    round: u32,
    portals: Vec<Portal>,
    links: Vec<(u32, u32, Faction)>,
    fields: Vec<([u32; 3], Faction, u64)>,
    events: Vec<arena_core::event::Event>,
    own_agents: Vec<(u32, Coord, u32)>,
    keys: BTreeMap<u32, BTreeMap<PortalId, u32>>,
}

impl ObsBuilder {
    pub fn new(round: u32) -> Self {
        ObsBuilder {
            round,
            portals: Vec::new(),
            links: Vec::new(),
            fields: Vec::new(),
            events: Vec::new(),
            own_agents: Vec::new(),
            keys: BTreeMap::new(),
        }
    }

    pub fn portal(mut self, id: u32, position: Coord, owner: Owner, hp: u32) -> Self {
        self.portals.push(Portal {
            id: PortalId(id),
            position,
            owner,
            hp,
        });
        self
    }

    pub fn link(mut self, a: u32, b: u32, faction: Faction) -> Self {
        self.links.push((a, b, faction));
        self
    }

    pub fn field(mut self, triple: [u32; 3], faction: Faction, area2: u64) -> Self {
        self.fields.push((triple, faction, area2));
        self
    }

    pub fn event(mut self, event: arena_core::event::Event) -> Self {
        self.events.push(event);
        self
    }

    pub fn agent(mut self, id: u32, position: Coord, supply: u32) -> Self {
        self.own_agents.push((id, position, supply));
        self
    }

    pub fn agent_keys(mut self, id: u32, portal: u32, count: u32) -> Self {
        self.keys
            .entry(id)
            .or_default()
            .insert(PortalId(portal), count);
        self
    }

    pub fn build(self) -> Observation {
        Observation {
            round: self.round,
            portals: self
                .portals
                .iter()
                .map(|p| arena_core::observe::PortalPublic {
                    id: p.id,
                    position: p.position,
                    owner: p.owner,
                    hp: p.hp,
                })
                .collect(),
            links: self
                .links
                .iter()
                .map(|&(a, b, f)| arena_core::observe::LinkPublic {
                    id: LinkId::new(PortalId(a), PortalId(b)).expect("distinct endpoints"),
                    faction: f,
                })
                .collect(),
            fields: self
                .fields
                .iter()
                .map(
                    |&(triple, faction, area2)| arena_core::observe::FieldPublic {
                        id: FieldId::new(
                            PortalId(triple[0]),
                            PortalId(triple[1]),
                            PortalId(triple[2]),
                        )
                        .expect("distinct vertices"),
                        faction,
                        area2,
                    },
                )
                .collect(),
            events: self.events,
            scores: BTreeMap::from([(Faction::Enl, 0 as Score2), (Faction::Res, 0 as Score2)]),
            own_agents: self
                .own_agents
                .iter()
                .map(
                    |&(id, position, supply)| arena_core::observe::AgentPrivate {
                        id: AgentId(id),
                        position,
                        supply,
                        keys: self.keys.get(&id).cloned().unwrap_or_default(),
                    },
                )
                .collect(),
        }
    }
}

/// The attack-and-recapture opponent for the GreedyField replan test:
/// stands on `victim`, waits until the enemy faction holds any field, then
/// attacks the victim every round while it is enemy-owned and captures it
/// whenever it is neutral (retries after a raced recapture). Idles once the
/// victim is ours.
pub struct NeutralizeAndCapture {
    victim: PortalId,
    enemy: Faction,
    engaged: bool,
}

impl NeutralizeAndCapture {
    pub fn new(victim: PortalId, enemy: Faction) -> Self {
        NeutralizeAndCapture {
            victim,
            enemy,
            engaged: false,
        }
    }
}

impl RoundStrategy for NeutralizeAndCapture {
    fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission> {
        let victim = obs
            .portals
            .iter()
            .find(|p| p.id == self.victim)
            .expect("victim portal exists");
        let me = obs.own_agents.first().expect("one scripted agent");
        if !self.engaged {
            self.engaged = obs.fields.iter().any(|f| f.faction == self.enemy);
        }
        let enemy_owner = match self.enemy {
            Faction::Enl => Owner::Enl,
            Faction::Res => Owner::Res,
        };
        let command = match victim.owner {
            _ if victim.owner == Owner::Neutral && self.engaged => Command::Capture {
                portal: self.victim,
            },
            _ if victim.owner == enemy_owner && self.engaged => Command::Attack,
            _ => return Vec::new(),
        };
        vec![AgentSubmission::new(me.id, vec![command])]
    }
}
