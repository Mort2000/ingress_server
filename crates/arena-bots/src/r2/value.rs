//! The plan-6.2 shared value model: interpretable INTEGER heuristics over
//! the legal `Observation` and the public event history only — no black-box
//! learning, no enemy private state (unreachable by construction), no
//! floating point (ADR-0007 discipline: every score is a fixed-width
//! integer count of score2-equivalents, saturating on hostile magnitudes).
//!
//! One unit underlies every scorer: ONE ROUND OF ONE FIELD'S INCOME equals
//! its `area2` score2. From that:
//!
//! - FIELD value is the candidate's own expected area-time income with
//!   every deduction (travel rounds, key acquisition, link ordering, loss
//!   risk) expressed as rounds of that income: `area2 * (horizon - setup)`.
//! - ATTACK / RECHARGE / TRANSFER value compare gains against an
//!   [`Economy`] whose `opportunity_per_round` is the score2 one round of
//!   an agent's time is worth against its best forgone alternative.
//! - BLOCK value is the enemy candidate income a proposed own link's
//!   closed segment denies, minus the own candidate income the same
//!   segment precludes.
//!
//! Every threshold lives in the recorded [`ValueParams`] (plan 6.3: no
//! untracked scattered constants); rule constants come from `GameConfig`.
//! All functions are pure — deterministic given their inputs, no RNG — so
//! policy-level determinism reduces to the determinism of the caller's
//! [`crate::r2::rng::R2Rng`] stream (plan section 6.3).

use std::collections::BTreeSet;

use arena_core::config::GameConfig;
use arena_core::event::EventKind;
use arena_core::geometry::{area2, ceil_integer_sqrt, dist2, point_on_segment, segments_intersect};
use arena_core::observe::{Observation, PortalPublic};
use arena_core::types::{Coord, Faction, LinkId, Owner, PortalId};

use crate::common::owners;

/// The structured thresholds of the value model (plan 6.3). These are the
/// RECORDED r2.0 initial values — the policy layer carries them verbatim
/// into its `PolicySpec` parameters, and any change to them is a policy
/// version change, never an in-place edit of behavior (ADR-0015).
///
/// [`ValueParams::tag`] serializes every field canonically so records
/// round-trip (`policies.tsv` carries all strategy parameters).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ValueParams {
    /// Income horizon cap (rounds): no candidate is credited with more
    /// future income than this, even early in a long match.
    pub horizon_cap_rounds: u32,
    /// How recent (rounds, inclusive) an enemy attack must be to count as
    /// pressure on a portal.
    pub threat_window_rounds: u32,
    /// Income-rounds discounted per candidate vertex under recent enemy
    /// attack (loss risk).
    pub risk_rounds_per_threatened_vertex: u32,
    /// An own anchor is defense-urgent below this share (permille) of
    /// `PORTAL_MAX_HP` (or under attack inside the threat window).
    pub defend_hp_permille: u32,
    /// Share (permille) of a denied enemy candidate's income credited to a
    /// blocking link.
    pub deny_share_permille: u32,
    /// Share (permille) of own candidate income charged against a link
    /// that crosses it (own geometric loss).
    pub own_loss_share_permille: u32,
    /// Share (permille) of the best new own field through a portal
    /// credited as capture gain.
    pub capture_gain_share_permille: u32,
    /// Cost of one unit of Supply spent on RECHARGE, in permille of one
    /// round of opportunity income.
    pub supply_cost_permille: u32,
    /// Compute bound: how many ranked field candidates one decision keeps.
    pub max_field_candidates: usize,
}

impl Default for ValueParams {
    fn default() -> Self {
        ValueParams {
            horizon_cap_rounds: 100,
            threat_window_rounds: 8,
            risk_rounds_per_threatened_vertex: 10,
            defend_hp_permille: 500,
            deny_share_permille: 500,
            own_loss_share_permille: 500,
            capture_gain_share_permille: 500,
            supply_cost_permille: 500,
            max_field_candidates: 24,
        }
    }
}

/// Canonical key order of the serialized record — [`ValueParams::tag`]
/// emits exactly this sequence and [`ValueParams::from_tag`] accepts
/// exactly this sequence (a record has ONE canonical form).
const PARAM_KEYS: [&str; 9] = [
    "horizon", "window", "risk", "defend", "deny", "ownloss", "capture", "supply", "maxcand",
];

impl ValueParams {
    /// Canonical `key=value;...` serialization of EVERY parameter, in
    /// [`PARAM_KEYS`] order (machine-recorded, human-auditable).
    pub fn tag(&self) -> String {
        let values = [
            self.horizon_cap_rounds.to_string(),
            self.threat_window_rounds.to_string(),
            self.risk_rounds_per_threatened_vertex.to_string(),
            self.defend_hp_permille.to_string(),
            self.deny_share_permille.to_string(),
            self.own_loss_share_permille.to_string(),
            self.capture_gain_share_permille.to_string(),
            self.supply_cost_permille.to_string(),
            self.max_field_candidates.to_string(),
        ];
        let mut tag = String::new();
        for (key, value) in PARAM_KEYS.iter().zip(values) {
            if !tag.is_empty() {
                tag.push(';');
            }
            tag.push_str(key);
            tag.push('=');
            tag.push_str(&value);
        }
        tag
    }

    /// Parses [`ValueParams::tag`] back. Strictly canonical: exactly the
    /// nine known keys in order, decimal values, nothing else — any other
    /// string is `None` (a malformed parameter record never silently
    /// falls back to defaults).
    pub fn from_tag(tag: &str) -> Option<ValueParams> {
        let parts: Vec<&str> = tag.split(';').collect();
        if parts.len() != PARAM_KEYS.len() {
            return None;
        }
        let mut values = [0u64; 9];
        for (index, part) in parts.iter().enumerate() {
            let (key, raw) = part.split_once('=')?;
            if key != PARAM_KEYS[index] {
                return None;
            }
            values[index] = raw.parse().ok()?;
        }
        Some(ValueParams {
            horizon_cap_rounds: u32::try_from(values[0]).ok()?,
            threat_window_rounds: u32::try_from(values[1]).ok()?,
            risk_rounds_per_threatened_vertex: u32::try_from(values[2]).ok()?,
            defend_hp_permille: u32::try_from(values[3]).ok()?,
            deny_share_permille: u32::try_from(values[4]).ok()?,
            own_loss_share_permille: u32::try_from(values[5]).ok()?,
            capture_gain_share_permille: u32::try_from(values[6]).ok()?,
            supply_cost_permille: u32::try_from(values[7]).ok()?,
            max_field_candidates: usize::try_from(values[8]).ok()?,
        })
    }
}

/// The shared conversion terms every opportunity-cost deduction goes
/// through: how many rounds of income the model still credits, and how
/// much score2 one round of an agent's time is worth against its best
/// forgone alternative.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Economy {
    /// Remaining income horizon (rounds) the model credits.
    pub horizon_rounds: u32,
    /// Score2 forgone per round of agent time (the best pending build
    /// option's per-round income).
    pub opportunity_per_round: u64,
}

impl Economy {
    /// Derives the economy at `round`: the horizon is the remaining match
    /// length capped by `params.horizon_cap_rounds` (never zero before the
    /// final round settles); the opportunity rate is the caller's best
    /// pending own field area2 (score2 per round).
    pub fn for_round(
        cfg: &GameConfig,
        round: u32,
        params: &ValueParams,
        best_pending_own_area2: u64,
    ) -> Economy {
        Economy {
            horizon_rounds: horizon_rounds(cfg, round, params),
            opportunity_per_round: best_pending_own_area2,
        }
    }
}

/// A viable own-faction field candidate: three distinct portals whose
/// three link pairs are all buildable this round (the frozen
/// `search_triangle` edge definition) with the triangle's exact integer
/// `area2`. Vertices are canonical ascending.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct FieldCandidate {
    pub vertices: [PortalId; 3],
    pub area2: u64,
}

// ---------------------------------------------------------------------
// Small shared helpers (all integer, all saturating)
// ---------------------------------------------------------------------

/// Clamps an i128 intermediate into the i64 score domain.
fn sat_i64(value: i128) -> i64 {
    value.clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

/// Whole rounds an agent needs to cover `from -> to` at one round's
/// MOVE_BUDGET (ceil of the exact integer distance). Public: the builder
/// core and policy overlays reuse the same travel estimate.
pub fn travel_rounds_between(from: Coord, to: Coord, budget_cm: u32) -> u32 {
    if budget_cm == 0 {
        return u32::MAX; // GameConfig::validate rejects this; stay total
    }
    let distance = ceil_integer_sqrt(dist2(from, to).max(0) as u64);
    distance.div_ceil(budget_cm as u64).min(u32::MAX as u64) as u32
}

/// The position view of `portal` in `obs` (portals are dense by map
/// construction; `None` only for a caller-side id bug).
fn view<'a>(obs: &'a Observation, portal: PortalId) -> Option<&'a PortalPublic> {
    obs.portals.iter().find(|p| p.id == portal)
}

/// Minimum whole rounds any own agent needs to reach `portal` (u32::MAX
/// when the faction has no agents or the portal is unknown — a candidate
/// no agent can reach scores accordingly).
pub fn travel_rounds_to(obs: &Observation, cfg: &GameConfig, portal: PortalId) -> u32 {
    let Some(target) = view(obs, portal) else {
        return u32::MAX;
    };
    obs.own_agents
        .iter()
        .map(|agent| travel_rounds_between(agent.position, target.position, cfg.move_budget_cm))
        .min()
        .unwrap_or(u32::MAX)
}

/// The binding travel estimate of a candidate: the rounds until EVERY
/// station can be occupied — the max over vertices of the per-vertex
/// nearest-agent travel.
pub fn travel_rounds_to_triangle(
    obs: &Observation,
    cfg: &GameConfig,
    vertices: &[PortalId; 3],
) -> u32 {
    vertices
        .iter()
        .map(|&v| travel_rounds_to(obs, cfg, v))
        .max()
        .unwrap_or(u32::MAX)
}

/// Key-acquisition cost (rounds): the field plan consumes one Key of each
/// vertex as a link target; every vertex no own agent currently holds a
/// Key of costs one HACK cooldown period (the conservative yield-1
/// estimate over the public inventory only).
pub fn key_rounds(
    obs: &Observation,
    _faction: Faction,
    cfg: &GameConfig,
    candidate: &FieldCandidate,
) -> u32 {
    let mut rounds = 0u32;
    for &vertex in &candidate.vertices {
        let held = obs
            .own_agents
            .iter()
            .any(|agent| agent.keys.get(&vertex).copied().unwrap_or(0) >= 1);
        if !held {
            rounds = rounds.saturating_add(cfg.hack_cooldown_rounds);
        }
    }
    rounds
}

/// Link-ordering cost (rounds): every link pair with a vertex not yet
/// owned by `faction` has to wait for that Capture before its Link can be
/// emitted (a link requires both portals owned).
pub fn link_order_rounds(obs: &Observation, faction: Faction, candidate: &FieldCandidate) -> u32 {
    let (own, _) = owners(faction);
    let [a, b, c] = candidate.vertices;
    let mut waiting = 0u32;
    for (x, y) in [(a, b), (a, c), (b, c)] {
        let own_both = [x, y]
            .iter()
            .all(|&v| view(obs, v).is_some_and(|p| p.owner == own));
        if !own_both {
            waiting += 1;
        }
    }
    waiting
}

/// Whether `event` is an enemy-faction attack on `portal` inside the
/// threat window (public history only).
fn is_recent_enemy_attack(
    obs: &Observation,
    enemy: Faction,
    params: &ValueParams,
    event: &arena_core::event::Event,
    portal: PortalId,
) -> bool {
    event.round <= obs.round
        && obs.round - event.round <= params.threat_window_rounds
        && matches!(
            &event.kind,
            EventKind::PortalAttacked {
                faction, portal: hit, ..
            } if *faction == enemy && *hit == portal
        )
}

/// Loss-risk discount (rounds): every candidate vertex under recent enemy
/// attack discounts `risk_rounds_per_threatened_vertex` rounds of income.
pub fn risk_rounds(
    obs: &Observation,
    faction: Faction,
    params: &ValueParams,
    candidate: &FieldCandidate,
) -> u32 {
    let enemy = faction.opposite();
    candidate
        .vertices
        .iter()
        .filter(|&&vertex| {
            obs.events
                .iter()
                .any(|event| is_recent_enemy_attack(obs, enemy, params, event, vertex))
        })
        .count() as u32
        * params.risk_rounds_per_threatened_vertex
}

/// The income horizon at `round`: the remaining match length (this round
/// inclusive) capped by the recorded parameter.
pub fn horizon_rounds(cfg: &GameConfig, round: u32, params: &ValueParams) -> u32 {
    let remaining = cfg.total_rounds.saturating_sub(round) + 1;
    params.horizon_cap_rounds.min(remaining)
}

/// Whole attacks needed to bring a portal from `hp` to zero at the current
/// ATTACK_DAMAGE.
pub fn attacks_needed(hp: u32, cfg: &GameConfig) -> u32 {
    hp.div_ceil(cfg.attack_damage.max(1))
}

// ---------------------------------------------------------------------
// Field candidate generation (plan 5.1)
// ---------------------------------------------------------------------

/// Whether the canonical pair `(a, b)` is a buildable link for the faction
/// that may build on `buildable`-or-neutral vertices — the frozen
/// `search_triangle` edge definition: within the closed
/// `MAX_LINK_DISTANCE`, no link on the pair, no third portal on the closed
/// segment, and no crossing live link (shared endpoints exempt).
fn edge_viable(
    portals: &[PortalPublic],
    links: &[arena_core::observe::LinkPublic],
    link_ids: &BTreeSet<LinkId>,
    max_link_distance_cm: u32,
    a: PortalId,
    b: PortalId,
) -> bool {
    let (Some(pa), Some(pb)) = (
        portals.iter().find(|p| p.id == a),
        portals.iter().find(|p| p.id == b),
    ) else {
        return false;
    };
    if a == b {
        return false;
    }
    let max2 = (max_link_distance_cm as i64) * (max_link_distance_cm as i64);
    if dist2(pa.position, pb.position) > max2 {
        return false;
    }
    if link_ids.contains(&LinkId::new(a, b).expect("distinct pair")) {
        return false;
    }
    if portals
        .iter()
        .any(|p| p.id != a && p.id != b && point_on_segment(p.position, pa.position, pb.position))
    {
        return false;
    }
    !links.iter().any(|link| {
        let (Some(u), Some(v)) = (
            portals.iter().find(|p| p.id == link.id.min),
            portals.iter().find(|p| p.id == link.id.max),
        ) else {
            return false;
        };
        let shared = link.id.contains(a) || link.id.contains(b);
        !shared && segments_intersect(pa.position, pb.position, u.position, v.position)
    })
}

/// Every viable candidate triangle over vertices the `buildable` faction
/// may use (own-or-neutral from its perspective), with exact `area2`.
/// Deterministic: ascending vertex triples in enumeration order.
fn candidate_triples(
    portals: &[PortalPublic],
    links: &[arena_core::observe::LinkPublic],
    buildable: Owner,
    max_link_distance_cm: u32,
) -> Vec<([PortalId; 3], u64)> {
    let link_ids: BTreeSet<LinkId> = links.iter().map(|l| l.id).collect();
    let eligible: Vec<PortalId> = portals
        .iter()
        .filter(|p| p.owner == Owner::Neutral || p.owner == buildable)
        .map(|p| p.id)
        .collect();
    let eligible_set: BTreeSet<PortalId> = eligible.iter().copied().collect();

    // Adjacency over buildable pairs (each pair enumerated once, a < b).
    let mut adjacency: std::collections::BTreeMap<PortalId, BTreeSet<PortalId>> =
        std::collections::BTreeMap::new();
    for (index, &a) in eligible.iter().enumerate() {
        for &b in eligible.iter().skip(index + 1) {
            if edge_viable(portals, links, &link_ids, max_link_distance_cm, a, b) {
                adjacency.entry(a).or_default().insert(b);
                adjacency.entry(b).or_default().insert(a);
            }
        }
    }

    let position = |id: PortalId| -> Coord {
        portals
            .iter()
            .find(|p| p.id == id)
            .expect("dense portal ids")
            .position
    };
    let mut triples = Vec::new();
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
                let area = area2(position(a), position(b), position(c));
                if area > 0 {
                    triples.push(([a, b, c], area));
                }
            }
        }
    }
    triples
}

/// This round's viable own-faction field candidates for `faction`, ranked
/// by exact `area2` descending with ascending-triple tie-breaks and
/// truncated to `params.max_field_candidates` (the compute bound of plan
/// section 15; value ranking happens in [`rank_field_candidates`]).
pub fn field_candidates(
    obs: &Observation,
    faction: Faction,
    cfg: &GameConfig,
    params: &ValueParams,
) -> Vec<FieldCandidate> {
    let (own, _) = owners(faction);
    let mut candidates: Vec<FieldCandidate> =
        candidate_triples(&obs.portals, &obs.links, own, cfg.max_link_distance_cm)
            .into_iter()
            .map(|(vertices, area2)| FieldCandidate { vertices, area2 })
            .collect();
    candidates.sort_by(|a, b| {
        b.area2
            .cmp(&a.area2)
            .then_with(|| a.vertices.cmp(&b.vertices))
    });
    candidates.truncate(params.max_field_candidates);
    candidates
}

/// Full value ranking of [`field_candidates`] for `faction`: every
/// candidate scored by [`field_value`], sorted value-descending with
/// ascending-triple tie-breaks. A pure function of the observation —
/// equal observations rank identically.
pub fn rank_field_candidates(
    obs: &Observation,
    faction: Faction,
    cfg: &GameConfig,
    params: &ValueParams,
) -> Vec<(i64, FieldCandidate)> {
    let mut ranked: Vec<(i64, FieldCandidate)> = field_candidates(obs, faction, cfg, params)
        .into_iter()
        .map(|candidate| {
            (
                field_value(obs, faction, cfg, params, &candidate),
                candidate,
            )
        })
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.vertices.cmp(&b.1.vertices)));
    ranked
}

// ---------------------------------------------------------------------
// The five scorers (plan 6.2)
// ---------------------------------------------------------------------

/// FIELD value: the candidate's expected area-time income — `area2` times
/// the horizon net of every setup deduction, each deduction expressed as
/// rounds of the candidate's own income:
///
/// - travel ([`travel_rounds_to_triangle`]): rounds until every station
///   can be occupied;
/// - key acquisition ([`key_rounds`]): HACK cooldowns for missing target
///   keys;
/// - link ordering ([`link_order_rounds`]): link pairs waiting on
///   captures;
/// - loss risk ([`risk_rounds`]): income-rounds discounted per vertex
///   under recent enemy attack (public event history).
///
/// May be negative: a candidate whose setup outlasts its horizon destroys
/// income, and ranks below every candidate that does not.
pub fn field_value(
    obs: &Observation,
    faction: Faction,
    cfg: &GameConfig,
    params: &ValueParams,
    candidate: &FieldCandidate,
) -> i64 {
    let horizon = horizon_rounds(cfg, obs.round, params) as i64;
    let setup = [
        travel_rounds_to_triangle(obs, cfg, &candidate.vertices),
        key_rounds(obs, faction, cfg, candidate),
        link_order_rounds(obs, faction, candidate),
        risk_rounds(obs, faction, params, candidate),
    ]
    .iter()
    .fold(0i64, |sum, rounds| sum.saturating_add(*rounds as i64));
    sat_i64(candidate.area2 as i128 * horizon.saturating_sub(setup) as i128)
}

/// The enemy portal's contribution to enemy income that dies with it:
/// every enemy field anchored on the portal, times the horizon
/// (score2-equivalents; the denial term of [`attack_value`]).
fn preventable_income(
    obs: &Observation,
    enemy: Faction,
    horizon_rounds: u32,
    portal: PortalId,
) -> u128 {
    obs.fields
        .iter()
        .filter(|field| field.faction == enemy && field.id.contains(portal))
        .map(|field| field.area2 as u128 * horizon_rounds as u128)
        .sum()
}

/// The best own candidate triangle `faction` could build through `portal`
/// (its capture gain potential), over own-or-neutral other vertices with
/// viable pairs. Zero when the portal unlocks nothing.
pub fn best_own_triangle_through(
    obs: &Observation,
    faction: Faction,
    cfg: &GameConfig,
    portal: PortalId,
) -> u64 {
    let (own, _) = owners(faction);
    let Some(target) = view(obs, portal) else {
        return 0;
    };
    let _ = target;
    let link_ids: BTreeSet<LinkId> = obs.links.iter().map(|l| l.id).collect();
    let others: Vec<PortalId> = obs
        .portals
        .iter()
        .filter(|p| p.id != portal && (p.owner == Owner::Neutral || p.owner == own))
        .map(|p| p.id)
        .collect();
    let position = |id: PortalId| -> Coord {
        obs.portals
            .iter()
            .find(|p| p.id == id)
            .expect("dense portal ids")
            .position
    };
    let mut best = 0u64;
    for (index, &x) in others.iter().enumerate() {
        for &y in others.iter().skip(index + 1) {
            let (px, py, pp) = (position(x), position(y), position(portal));
            // Cheap necessary conditions first; full edge viability after.
            if dist2(pp, px) > (cfg.max_link_distance_cm as i64).pow(2)
                || dist2(pp, py) > (cfg.max_link_distance_cm as i64).pow(2)
                || dist2(px, py) > (cfg.max_link_distance_cm as i64).pow(2)
            {
                continue;
            }
            let viable = edge_viable(
                &obs.portals,
                &obs.links,
                &link_ids,
                cfg.max_link_distance_cm,
                portal,
                x,
            ) && edge_viable(
                &obs.portals,
                &obs.links,
                &link_ids,
                cfg.max_link_distance_cm,
                portal,
                y,
            ) && edge_viable(
                &obs.portals,
                &obs.links,
                &link_ids,
                cfg.max_link_distance_cm,
                x,
                y,
            );
            if viable {
                best = best.max(area2(pp, px, py));
            }
        }
    }
    best
}

/// ATTACK value of an (enemy-owned) portal for `faction` (plan 6.2): the
/// enemy income its destruction stops plus the capture gain of the best
/// new own field through it, minus the agent-rounds it costs — travel
/// plus the required attack count — each at the economy's opportunity
/// rate (the forgone-building cost). Unknown portals score `i64::MIN`
/// (a caller-side id bug ranks dead last, never silently zero).
pub fn attack_value(
    obs: &Observation,
    faction: Faction,
    cfg: &GameConfig,
    params: &ValueParams,
    economy: &Economy,
    portal: PortalId,
) -> i64 {
    let Some(target) = view(obs, portal) else {
        return i64::MIN;
    };
    let enemy = faction.opposite();
    let horizon = economy.horizon_rounds as u128;
    let preventable = preventable_income(obs, enemy, economy.horizon_rounds, portal);
    let capture_gain = best_own_triangle_through(obs, faction, cfg, portal) as u128
        * horizon
        * params.capture_gain_share_permille as u128
        / 1000;
    let agent_rounds =
        travel_rounds_to(obs, cfg, portal) as u128 + attacks_needed(target.hp, cfg) as u128;
    let cost = agent_rounds * economy.opportunity_per_round as u128;
    sat_i64(preventable as i128 + capture_gain as i128 - cost as i128)
}

/// RECHARGE / DEFENSE value of an own portal (plan 6.2): the future own
/// field income that dies with the anchor, minus the Supply spent, the
/// travel to reach it, and the Major Action's opportunity cost. Gated by
/// urgency — the anchor is below the recorded HP share or under recent
/// enemy attack — so healthy, unthreatened anchors score exactly 0 (a
/// Recharge that preserves nothing new is never valued). Portals carrying
/// no live own field score 0; unknown portals score `i64::MIN`.
pub fn defense_value(
    obs: &Observation,
    faction: Faction,
    cfg: &GameConfig,
    params: &ValueParams,
    economy: &Economy,
    portal: PortalId,
) -> i64 {
    let Some(target) = view(obs, portal) else {
        return i64::MIN;
    };
    let mut preserved_area: u128 = 0;
    for field in obs.fields.iter().filter(|f| f.faction == faction) {
        if field.id.contains(portal) {
            preserved_area += field.area2 as u128;
        }
    }
    if preserved_area == 0 {
        return 0;
    }
    let hp_low =
        (target.hp as u64) * 1000 < (cfg.portal_max_hp as u64) * params.defend_hp_permille as u64;
    let attacked = obs
        .events
        .iter()
        .any(|event| is_recent_enemy_attack(obs, faction.opposite(), params, event, portal));
    if !hp_low && !attacked {
        return 0;
    }
    let preserved = preserved_area * economy.horizon_rounds as u128;
    let agent_rounds = travel_rounds_to(obs, cfg, portal) as u128 + 1; // + the Major Action
    let movement_and_major = agent_rounds * economy.opportunity_per_round as u128;
    let supply = cfg.recharge_cost_supply as u128
        * params.supply_cost_permille as u128
        * economy.opportunity_per_round as u128
        / 1000;
    sat_i64(preserved as i128 - movement_and_major as i128 - supply as i128)
}

/// TRANSFER value (plan 6.2): the income a resource reallocation
/// accelerates — the completed structure's `area2` times the rounds
/// gained — minus the sender's walk into TRANSFER_RANGE at the
/// opportunity rate.
pub fn transfer_value(
    economy: &Economy,
    field_area2: u64,
    rounds_accelerated: u32,
    sender_travel_rounds: u32,
) -> i64 {
    let gain = field_area2 as i128 * rounds_accelerated as i128;
    let cost = sender_travel_rounds as i128 * economy.opportunity_per_round as i128;
    sat_i64(gain - cost)
}

/// Whether the closed segment of the proposed own link `(source, target)`
/// crosses the closed segment of the candidate pair `(x, y)` without
/// sharing an endpoint (a shared endpoint blocks nothing — the pair is
/// dead as a candidate anyway once the link exists).
fn link_crosses_pair(
    obs: &Observation,
    source: PortalId,
    target: PortalId,
    x: PortalId,
    y: PortalId,
) -> bool {
    if x == source || x == target || y == source || y == target {
        return false;
    }
    let (Some(ps), Some(pt), Some(px), Some(py)) = (
        view(obs, source),
        view(obs, target),
        view(obs, x),
        view(obs, y),
    ) else {
        return false;
    };
    segments_intersect(ps.position, pt.position, px.position, py.position)
}

/// BLOCK value of a proposed own link `(source, target)` (plan 6.2): the
/// enemy candidate-field income its closed segment denies — every enemy
/// candidate with a crossed pair dies while the link lives — minus the
/// own candidate income the same segment precludes, each at the recorded
/// share. Unknown portals score `i64::MIN`. The caller proposes a legal
/// own link; this scorer only ranks it.
pub fn block_value(
    obs: &Observation,
    faction: Faction,
    cfg: &GameConfig,
    params: &ValueParams,
    source: PortalId,
    target: PortalId,
) -> i64 {
    if view(obs, source).is_none() || view(obs, target).is_none() {
        return i64::MIN;
    }
    let (own, enemy) = owners(faction);
    let horizon = horizon_rounds(cfg, obs.round, params) as u128;
    let denied: u128 = candidate_triples(&obs.portals, &obs.links, enemy, cfg.max_link_distance_cm)
        .into_iter()
        .filter(|&(vertices, _)| {
            let [a, b, c] = vertices;
            [(a, b), (a, c), (b, c)]
                .iter()
                .any(|&(x, y)| link_crosses_pair(obs, source, target, x, y))
        })
        .map(|(_, area2)| area2 as u128)
        .sum();
    let own_loss: u128 = candidate_triples(&obs.portals, &obs.links, own, cfg.max_link_distance_cm)
        .into_iter()
        .filter(|&(vertices, _)| {
            let [a, b, c] = vertices;
            [(a, b), (a, c), (b, c)]
                .iter()
                .any(|&(x, y)| link_crosses_pair(obs, source, target, x, y))
        })
        .map(|(_, area2)| area2 as u128)
        .sum();
    let denial = denied * horizon * params.deny_share_permille as u128 / 1000;
    let loss = own_loss * horizon * params.own_loss_share_permille as u128 / 1000;
    sat_i64(denial as i128 - loss as i128)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Unit-level pins of the helpers the integration fixtures exercise
    // only through rankings.

    #[test]
    fn travel_rounds_use_exact_integer_ceil_sqrt() {
        // 30_000 cm at a 20_000 cm budget: ceil(sqrt(900_000_000)/20_000)
        // = ceil(30_000/20_000) = 2 rounds.
        assert_eq!(
            travel_rounds_between(Coord::new(0, 0), Coord::new(40_000, 0), 20_000),
            2
        );
        // The exact budget distance is one round (closed range).
        assert_eq!(
            travel_rounds_between(Coord::new(0, 0), Coord::new(20_000, 0), 20_000),
            1
        );
        // Diagonals use the exact integer root, not axis sums.
        assert_eq!(
            travel_rounds_between(Coord::new(0, 0), Coord::new(20_000, 20_000), 20_000),
            2 // sqrt(8)*10_000 = 28_284 -> 2 rounds
        );
        assert_eq!(
            travel_rounds_between(Coord::new(5, 7), Coord::new(5, 7), 20_000),
            0
        );
    }

    #[test]
    fn attacks_needed_is_the_ceil_of_hp_over_damage() {
        let mut cfg = GameConfig::default();
        cfg.attack_damage = 20;
        assert_eq!(attacks_needed(100, &cfg), 5);
        assert_eq!(attacks_needed(101, &cfg), 6);
        assert_eq!(attacks_needed(1, &cfg), 1);
        assert_eq!(attacks_needed(0, &cfg), 0);
        cfg.attack_damage = 0; // GameConfig::validate rejects this
        assert_eq!(attacks_needed(100, &cfg), 100, "defensive nonzero divisor");
    }

    #[test]
    fn horizon_never_reaches_zero_before_settlement() {
        let cfg = GameConfig::default();
        let mut params = ValueParams::default();
        params.horizon_cap_rounds = 1000; // above the remaining match
        assert_eq!(horizon_rounds(&cfg, 1, &params), 500);
        assert_eq!(horizon_rounds(&cfg, 500, &params), 1);
        assert_eq!(horizon_rounds(&cfg, 501, &params), 1, "saturates, never 0");
        params.horizon_cap_rounds = 3;
        assert_eq!(horizon_rounds(&cfg, 1, &params), 3);
    }
}
