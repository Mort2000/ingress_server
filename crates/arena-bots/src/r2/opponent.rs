//! Limited Opponent Model from the LEGAL public event history (plan
//! sections 5.1 and 6.2): recent enemy attack pressure per portal and
//! enemy link-building zones, maintained incrementally from `obs.events`
//! only. The [`Observation`] type carries no enemy private state
//! (R-VIS), so no omniscient or debug-view input is reachable by
//! construction — the model's entire input vocabulary is public.
//!
//! All counters are plain integers. The model is advisory: policies feed
//! its threat and zone estimates into the shared value model's
//! deductions and their role overlays; nothing here touches Game Result,
//! Replay Hash or settlement.

use std::collections::BTreeMap;

use arena_core::event::{Event, EventKind};
use arena_core::observe::Observation;
use arena_core::types::{Faction, PortalId};

/// The recorded thresholds of the opponent model (plan 6.3: thresholds
/// live in the policy's `PolicySpec` parameters, never as scattered
/// constants).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OpponentParams {
    /// How recent (rounds, inclusive) an enemy attack counts as pressure
    /// on a portal.
    pub threat_window_rounds: u32,
    /// How recent (rounds, inclusive) an enemy Link creation keeps a
    /// portal hot as an enemy build-zone portal.
    pub zone_window_rounds: u32,
    /// Saturation cap of per-portal pressure (bounds the estimate; more
    /// attacks inside the window still read as this cap).
    pub pressure_cap_per_portal: u32,
}

impl Default for OpponentParams {
    fn default() -> Self {
        OpponentParams {
            threat_window_rounds: 8,
            zone_window_rounds: 12,
            pressure_cap_per_portal: 5,
        }
    }
}

const OPPONENT_PARAM_KEYS: [&str; 3] = ["threat", "zone", "cap"];

impl OpponentParams {
    /// Canonical `key=value;...` serialization (record round-trips).
    pub fn tag(&self) -> String {
        let values = [
            self.threat_window_rounds.to_string(),
            self.zone_window_rounds.to_string(),
            self.pressure_cap_per_portal.to_string(),
        ];
        let mut tag = String::new();
        for (key, value) in OPPONENT_PARAM_KEYS.iter().zip(values) {
            if !tag.is_empty() {
                tag.push(';');
            }
            tag.push_str(key);
            tag.push('=');
            tag.push_str(&value);
        }
        tag
    }

    /// Parses [`OpponentParams::tag`] back — strictly canonical (exactly
    /// the three known keys in order, decimal values); anything else is
    /// `None`.
    pub fn from_tag(tag: &str) -> Option<OpponentParams> {
        let parts: Vec<&str> = tag.split(';').collect();
        if parts.len() != OPPONENT_PARAM_KEYS.len() {
            return None;
        }
        let mut values = [0u64; 3];
        for (index, part) in parts.iter().enumerate() {
            let (key, raw) = part.split_once('=')?;
            if key != OPPONENT_PARAM_KEYS[index] {
                return None;
            }
            values[index] = raw.parse().ok()?;
        }
        Some(OpponentParams {
            threat_window_rounds: u32::try_from(values[0]).ok()?,
            zone_window_rounds: u32::try_from(values[1]).ok()?,
            pressure_cap_per_portal: u32::try_from(values[2]).ok()?,
        })
    }
}

/// The model state over one faction's public history. Constructed per
/// policy instance (one enemy faction per match side); `update` consumes
/// only events the observation has not shown yet (by `seq`), so repeated
/// observations are idempotent and incremental.
#[derive(Debug)]
pub struct OpponentModel {
    params: OpponentParams,
    enemy: Faction,
    /// Highest event `seq` already consumed.
    last_seq_seen: u64,
    /// Raw `(round, seq)` of every retained enemy attack, per portal.
    attacks: BTreeMap<PortalId, Vec<(u32, u64)>>,
    /// Raw `(round, seq)` of every retained enemy link creation, per
    /// endpoint portal.
    enemy_links: BTreeMap<PortalId, Vec<(u32, u64)>>,
}

impl OpponentModel {
    pub fn new(params: OpponentParams, enemy: Faction) -> Self {
        OpponentModel {
            params,
            enemy,
            last_seq_seen: 0,
            attacks: BTreeMap::new(),
            enemy_links: BTreeMap::new(),
        }
    }

    /// Consumes the NEW public events of `obs` (by `seq`) and prunes raw
    /// entries outside both windows of `obs.round` (memory is bounded by
    /// the window sizes). A pure function of the event history prefix:
    /// deterministic and idempotent for repeated observations.
    pub fn update(&mut self, obs: &Observation) {
        let keep_from = obs.round.saturating_sub(
            self.params
                .threat_window_rounds
                .max(self.params.zone_window_rounds),
        );
        let mut fresh: Vec<&Event> = obs
            .events
            .iter()
            .filter(|event| event.seq >= self.last_seq_seen)
            .collect();
        fresh.sort_by_key(|event| event.seq);
        for event in fresh {
            self.last_seq_seen = self.last_seq_seen.max(event.seq + 1);
            match &event.kind {
                EventKind::PortalAttacked {
                    faction, portal, ..
                } if *faction == self.enemy => {
                    self.attacks
                        .entry(*portal)
                        .or_default()
                        .push((event.round, event.seq));
                }
                EventKind::LinkCreated { faction, link, .. } if *faction == self.enemy => {
                    self.enemy_links
                        .entry(link.min)
                        .or_default()
                        .push((event.round, event.seq));
                    self.enemy_links
                        .entry(link.max)
                        .or_default()
                        .push((event.round, event.seq));
                }
                _ => {}
            }
        }
        let prune = |map: &mut BTreeMap<PortalId, Vec<(u32, u64)>>| {
            map.retain(|_, rounds| {
                rounds.retain(|&(round, _)| round >= keep_from);
                !rounds.is_empty()
            });
        };
        prune(&mut self.attacks);
        prune(&mut self.enemy_links);
    }

    /// Enemy attack pressure on `portal` (count of in-window attacks,
    /// capped): the threat estimate defense and blocking decisions read.
    pub fn pressure(&self, obs: &Observation, portal: PortalId) -> u32 {
        self.attacks
            .get(&portal)
            .map(|rounds| {
                rounds
                    .iter()
                    .filter(|&&(round, _)| {
                        round <= obs.round && obs.round - round <= self.params.threat_window_rounds
                    })
                    .count() as u32
            })
            .unwrap_or(0)
            .min(self.params.pressure_cap_per_portal)
    }

    /// Whether any portal is currently under enemy pressure.
    pub fn any_threat(&self, obs: &Observation) -> bool {
        self.attacks
            .keys()
            .any(|portal| self.pressure(obs, *portal) > 0)
    }

    /// The hottest portal by pressure (ties by ascending portal id);
    /// `None` when the enemy is not attacking anything in-window.
    pub fn top_threat(&self, obs: &Observation) -> Option<(PortalId, u32)> {
        self.attacks
            .keys()
            .map(|portal| (*portal, self.pressure(obs, *portal)))
            .filter(|&(_, pressure)| pressure > 0)
            .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
    }

    /// Enemy link-building heat of `portal`: how many in-window enemy
    /// link creations used it as an endpoint (the enemy's active
    /// construction zone).
    pub fn build_zone_heat(&self, obs: &Observation, portal: PortalId) -> u32 {
        self.enemy_links
            .get(&portal)
            .map(|rounds| {
                rounds
                    .iter()
                    .filter(|&&(round, _)| {
                        round <= obs.round && obs.round - round <= self.params.zone_window_rounds
                    })
                    .count() as u32
            })
            .unwrap_or(0)
    }
}
