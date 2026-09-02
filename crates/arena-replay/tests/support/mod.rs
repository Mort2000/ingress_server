//! Shared fixtures for the arena-replay integration tests (M5): the
//! hand-placed two-triangle map, the FieldBot ported from the M4 harness
//! tests, the AggroBot, the blake3-derived gate seeds, and the
//! triangle-pair search over generated maps.

// Each test binary compiles this module separately and uses only part of
// it, so cross-binary "dead" items are expected.
#![allow(dead_code)]

use arena_core::config::GameConfig;
use arena_core::geometry::{area2, ceil_integer_sqrt, dist2, point_on_segment, segments_intersect};
use arena_core::harness::{AgentSubmission, RoundStrategy};
use arena_core::map::{MapDefinition, generate_map};
use arena_core::observe::Observation;
use arena_core::spawn::{InitialSpawns, validate_initial_spawn_set};
use arena_core::state::GameState;
use arena_core::types::{AgentId, Command, Coord, Faction, LinkId, Owner, Portal, PortalId};

// ---------------------------------------------------------------------------
// Hand-placed fixture: two disjoint triangles inside one MOVE_BUDGET hop
// ---------------------------------------------------------------------------

/// Default config except a 40_000 cm map with six hand-placed portals and
/// three agents per faction (tour reachability: every triangle edge is
/// within the default 20_000 cm MOVE_BUDGET).
pub fn replay_cfg() -> GameConfig {
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
/// no edge crossings, no portal on a foreign segment, pairwise distances
/// within one MOVE_BUDGET hop per triangle.
pub fn replay_map() -> MapDefinition {
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

/// Spawns one faction's three initial agents exactly on its triangle's
/// portals (spawns are free coordinates).
pub fn replay_spawns(cfg: &GameConfig, faction: Faction) -> InitialSpawns {
    let (enl, res) = GameState::initial_roster(cfg);
    let roster: Vec<AgentId> = if faction == Faction::Enl { enl } else { res };
    let triangle: [u32; 3] = if faction == Faction::Enl {
        [0, 1, 2]
    } else {
        [3, 4, 5]
    };
    let map = replay_map();
    let entries: Vec<(AgentId, Coord)> = roster
        .iter()
        .zip(triangle.iter())
        .map(|(&agent, &portal)| {
            (
                agent,
                map.portal(PortalId(portal)).expect("dense portal").position,
            )
        })
        .collect();
    validate_initial_spawn_set(&roster, &entries, cfg).expect("fixture spawns valid")
}

// ---------------------------------------------------------------------------
// FieldBot (ported from the M4 harness tests; identical tour logic)
// ---------------------------------------------------------------------------

/// Test-local deterministic strategy (M6 builds the real baselines): a
/// pure function of the `Observation` plus a fixed per-agent triangle
/// plan, at most one command per agent per round. Tour: p0 (capture +
/// hack) -> p1 -> p2 -> LINK(p0,p1) -> LINK(p1,p2) -> LINK(p2,p0), so each
/// faction holds one persistent field by ~round 15. Reinforcement agents
/// are not planned and idle (no submission).
pub struct FieldBot {
    owner: Owner,
    plan: [PortalId; 3],
    agents: [AgentId; 3],
}

impl FieldBot {
    pub fn new(faction: Faction, plan: [u32; 3], roster_offset: u32) -> Self {
        FieldBot {
            owner: if faction == Faction::Enl {
                Owner::Enl
            } else {
                Owner::Res
            },
            plan: [PortalId(plan[0]), PortalId(plan[1]), PortalId(plan[2])],
            agents: [
                AgentId(roster_offset),
                AgentId(roster_offset + 1),
                AgentId(roster_offset + 2),
            ],
        }
    }
}

impl RoundStrategy for FieldBot {
    fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission> {
        let portal_pos = |id: PortalId| {
            obs.portals
                .iter()
                .find(|p| p.id == id)
                .expect("plan portal exists")
                .position
        };
        let owner_of = |id: PortalId| {
            obs.portals
                .iter()
                .find(|p| p.id == id)
                .expect("plan portal exists")
                .owner
        };

        let mut submissions = Vec::new();
        for &agent in &self.agents {
            let Some(me) = obs.own_agents.iter().find(|a| a.id == agent) else {
                continue;
            };
            // Six tour stations: three capture+hack stops, then the three
            // triangle edges in tour order (source, target).
            let stations: [(PortalId, Option<PortalId>); 6] = [
                (self.plan[0], None),
                (self.plan[1], None),
                (self.plan[2], None),
                (self.plan[0], Some(self.plan[1])),
                (self.plan[1], Some(self.plan[2])),
                (self.plan[2], Some(self.plan[0])),
            ];
            let complete = |(p, target): &(PortalId, Option<PortalId>)| match target {
                None => owner_of(*p) == self.owner && me.keys.get(p).copied().unwrap_or(0) >= 1,
                Some(t) => obs
                    .links
                    .iter()
                    .any(|l| l.id == LinkId::new(*p, *t).expect("plan portals are distinct")),
            };
            let Some((stand, target)) = stations.iter().find(|s| !complete(s)) else {
                continue; // tour done: explicit PASS
            };
            let command = if me.position != portal_pos(*stand) {
                Command::Move {
                    destination: portal_pos(*stand),
                }
            } else {
                match target {
                    None => {
                        if owner_of(*stand) == Owner::Neutral {
                            Command::Capture { portal: *stand }
                        } else if me.keys.get(stand).copied().unwrap_or(0) == 0 {
                            Command::Hack { portal: *stand }
                        } else {
                            continue; // captured and keyed: complete next round
                        }
                    }
                    Some(t) => {
                        if me.keys.get(t).copied().unwrap_or(0) >= 1 {
                            Command::Link {
                                source_portal: *stand,
                                target_portal: *t,
                            }
                        } else {
                            continue; // cannot link without the target key
                        }
                    }
                }
            };
            submissions.push(AgentSubmission::new(agent, vec![command]));
        }
        submissions
    }
}

// ---------------------------------------------------------------------------
// AggroBot: pure f(Observation), integer-only, deterministic
// ---------------------------------------------------------------------------

/// Aggressive baseline: every agent (reinforcements included) targets the
/// ascending-nearest enemy-owned portal (fallback: nearest neutral; ties
/// broken by ascending `PortalId`), steps toward it by at most
/// `MOVE_BUDGET` via integer `ceil_integer_sqrt` stepping, and in closed
/// `PORTAL_OPERATION_RANGE` ATTACKs when any enemy portal is in range,
/// CAPTURES a neutral target, or HACKs the target. Pure function of the
/// observation plus the two config constants (they are not part of the
/// observation).
pub struct AggroBot {
    enemy: Owner,
    move_budget_cm: u32,
    operation_range_cm: u32,
}

impl AggroBot {
    pub fn new(faction: Faction, cfg: &GameConfig) -> Self {
        AggroBot {
            enemy: if faction == Faction::Enl {
                Owner::Res
            } else {
                Owner::Enl
            },
            move_budget_cm: cfg.move_budget_cm,
            operation_range_cm: cfg.portal_operation_range_cm,
        }
    }

    fn target(&self, obs: &Observation, from: Coord) -> Option<PortalId> {
        // (dist2, portal id) ascending picks the nearest portal of the
        // preferred class with the ascending-id tie-break.
        let pick = |owner: Owner| {
            obs.portals
                .iter()
                .filter(|p| p.owner == owner)
                .map(|p| (dist2(from, p.position), p.id.0))
                .min()
                .map(|(_, id)| PortalId(id))
        };
        pick(self.enemy).or_else(|| pick(Owner::Neutral))
    }

    fn step_toward(&self, from: Coord, to: Coord) -> Coord {
        let dx = to.x as i64 - from.x as i64;
        let dy = to.y as i64 - from.y as i64;
        let dist = ceil_integer_sqrt((dx * dx + dy * dy) as u64);
        if dist <= self.move_budget_cm as u64 {
            return to;
        }
        let dist = dist as i64;
        let budget = self.move_budget_cm as i64;
        let step = |delta: i64| ((delta.abs() * budget / dist) * delta.signum()) as i32;
        let (mut sx, mut sy) = (step(dx), step(dy));
        if sx == 0 && sy == 0 {
            // Cannot happen when dist > budget (the dominant component
            // steps by at least budget/2), but stay total: nudge the
            // dominant axis by one centimetre.
            if dx.abs() >= dy.abs() {
                sx = dx.signum() as i32;
            } else {
                sy = dy.signum() as i32;
            }
        }
        Coord::new(from.x + sx, from.y + sy)
    }
}

impl RoundStrategy for AggroBot {
    fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission> {
        let range2 = (self.operation_range_cm as i64) * (self.operation_range_cm as i64);
        let mut submissions = Vec::new();
        for me in &obs.own_agents {
            let Some(target) = self.target(obs, me.position) else {
                continue; // every portal already ours: idle
            };
            let target_view = obs
                .portals
                .iter()
                .find(|p| p.id == target)
                .expect("target comes from the observation");
            let target_in_range = dist2(me.position, target_view.position) <= range2;
            let enemy_in_range = obs
                .portals
                .iter()
                .any(|p| p.owner == self.enemy && dist2(me.position, p.position) <= range2);
            let command = if target_in_range && enemy_in_range {
                Command::Attack
            } else if target_in_range && target_view.owner == Owner::Neutral {
                Command::Capture { portal: target }
            } else if target_in_range {
                Command::Hack { portal: target }
            } else {
                Command::Move {
                    destination: self.step_toward(me.position, target_view.position),
                }
            };
            submissions.push(AgentSubmission::new(me.id, vec![command]));
        }
        submissions
    }
}

// ---------------------------------------------------------------------------
// Generated-map triangle search (ported from the M4 harness tests)
// ---------------------------------------------------------------------------

/// Deterministic search over a generated map for two triangles the bots
/// can complete without geometric interference (see [`triangle_pair`]).
/// Panics when the map holds none — fixture maps must be chosen to work.
pub fn find_triangle_pair(map: &MapDefinition) -> ([u32; 3], [u32; 3]) {
    triangle_pair(map).expect("the map has two disjoint non-crossing triangles")
}

/// Spawns for a generated map: the faction's three initial agents on their
/// triangle's portals.
pub fn triangle_spawns(
    cfg: &GameConfig,
    faction: Faction,
    map: &MapDefinition,
    triangle: &[u32; 3],
) -> InitialSpawns {
    let (enl, res) = GameState::initial_roster(cfg);
    let roster: Vec<AgentId> = if faction == Faction::Enl { enl } else { res };
    let entries: Vec<(AgentId, Coord)> = roster
        .iter()
        .zip(triangle.iter())
        .map(|(&agent, &portal)| {
            (
                agent,
                map.portal(PortalId(portal)).expect("dense portal").position,
            )
        })
        .collect();
    validate_initial_spawn_set(&roster, &entries, cfg).expect("triangle spawns valid")
}

/// Non-panicking variant of [`find_triangle_pair`]'s pair search.
fn triangle_pair(map: &MapDefinition) -> Option<([u32; 3], [u32; 3])> {
    const HOP2: i64 = 20_000 * 20_000;
    let portals: Vec<(PortalId, Coord)> =
        map.portals().iter().map(|p| (p.id, p.position)).collect();
    let n = portals.len();
    if n < 6 {
        return None;
    }
    let segment_clear = |a: Coord, b: Coord, endpoints: &[u32]| {
        portals
            .iter()
            .filter(|(id, _)| !endpoints.contains(&id.0))
            .all(|&(_, p)| !point_on_segment(p, a, b))
    };
    let mut candidates: Vec<[u32; 3]> = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            if dist2(portals[i].1, portals[j].1) > HOP2 {
                continue;
            }
            for k in (j + 1)..n {
                if dist2(portals[i].1, portals[k].1) > HOP2
                    || dist2(portals[j].1, portals[k].1) > HOP2
                {
                    continue;
                }
                if area2(portals[i].1, portals[j].1, portals[k].1) == 0 {
                    continue;
                }
                let triple = [i as u32, j as u32, k as u32];
                if segment_clear(portals[i].1, portals[j].1, &triple)
                    && segment_clear(portals[i].1, portals[k].1, &triple)
                    && segment_clear(portals[j].1, portals[k].1, &triple)
                {
                    candidates.push(triple);
                }
            }
        }
    }
    for x in 0..candidates.len() {
        for y in (x + 1)..candidates.len() {
            let (a, b) = (&candidates[x], &candidates[y]);
            if a.iter().any(|p| b.contains(p)) {
                continue;
            }
            let pos = |id: u32| portals[id as usize].1;
            let mut crossing = false;
            for u in 0..3 {
                for v in (u + 1)..3 {
                    for s in 0..3 {
                        for t in (s + 1)..3 {
                            if segments_intersect(pos(a[u]), pos(a[v]), pos(b[s]), pos(b[t])) {
                                crossing = true;
                            }
                        }
                    }
                }
            }
            if !crossing {
                return Some((*a, *b));
            }
        }
    }
    None
}

/// Generates the map for gate match `m`, applying the deterministic skip
/// rule (D50): when the base seed's map holds no triangle pair, replace
/// the seed with `map_seed(m + 1000*k)`, `k = 1, 2, ...`. Returns the map,
/// the seed actually used, and the skip count.
pub fn gate_map(m: u32, cfg: &GameConfig, needs_search: bool) -> (MapDefinition, u64, u32) {
    let mut skips = 0u32;
    loop {
        let seed = gate_map_seed(m.wrapping_add(1000u32.wrapping_mul(skips)));
        let map = generate_map(seed, cfg).expect("generated map");
        if !needs_search || triangle_pair(&map).is_some() {
            return (map, seed, skips);
        }
        skips += 1;
    }
}

// ---------------------------------------------------------------------------
// Gate seeds (D50): blake3-derived, no ad-hoc arithmetic
// ---------------------------------------------------------------------------

/// `LE-u64(blake3(domain || m_le32))` — one distinct seed stream per
/// domain label.
pub fn derive_seed(domain: &[u8], m: u32) -> u64 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(&m.to_le_bytes());
    let hash = hasher.finalize();
    let mut words = [0u8; 8];
    words.copy_from_slice(&hash.as_bytes()[0..8]);
    u64::from_le_bytes(words)
}

/// Game seed of gate match `m` (D50).
pub fn gate_game_seed(m: u32) -> u64 {
    derive_seed(b"ingress-arena-m5-game", m)
}

/// Map seed of gate match `m` (D50).
pub fn gate_map_seed(m: u32) -> u64 {
    derive_seed(b"ingress-arena-m5-map", m)
}
