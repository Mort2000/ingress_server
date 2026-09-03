//! `logistics-builder-r2` (plan section 6.1, Key Runner + Builder): the
//! coordinated builder core plus the Key/Supply transport overlay. The
//! command rule spends a Link target's key from the ACTING agent's
//! inventory — so when the only holder of a plan vertex's key stands far
//! from the link source while another agent is already in range, the
//! overlay routes the key (and spare Supply) there with an explicit
//! [`Command::Transfer`], pre-positioning resources for long links
//! instead of re-farming them locally (Transfer was 0/10,000 matches in
//! the frozen m6 baseline).
//!
//! Every Transfer passes observation-level pre-flight checks (receiver is
//! an own agent inside closed `TRANSFER_RANGE`, sender holds the keys,
//! Supply respects both the sender's recorded reserve and the receiver's
//! headroom to `MAX_SUPPLY`), so the emission is drop-free by
//! construction; execution races still record as normal Drops (plan 6.3).

use std::collections::BTreeMap;

use arena_core::config::GameConfig;
use arena_core::harness::AgentSubmission;
use arena_core::map::MapDefinition;
use arena_core::observe::Observation;
use arena_core::types::{AgentId, Command, Coord, Faction, PortalId};

use crate::common::{self, CommandList, in_range};
use crate::r2::R2Policy;
use crate::r2::build::{BuilderCore, CoreParams};
use crate::r2::greedy_coordinated::submissions;
use crate::r2::tasks::{Task, TaskBoard, allocate};

/// The recorded `logistics-builder-r2` parameters.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LogisticsBuilderParams {
    /// Supply the sender keeps after a Transfer (the runner's own reserve).
    pub supply_reserve: u32,
    /// At most this many Transfers per round (the transport headroom).
    pub max_transfers_per_round: u32,
    /// The shared builder-core + value-model parameters.
    pub core: CoreParams,
}

impl Default for LogisticsBuilderParams {
    fn default() -> Self {
        LogisticsBuilderParams {
            supply_reserve: 5,
            max_transfers_per_round: 2,
            core: CoreParams::default(),
        }
    }
}

impl LogisticsBuilderParams {
    /// Canonical serialization: `reserve=..;cap=..|core=<CoreParams::tag>`.
    pub fn tag(&self) -> String {
        format!(
            "reserve={};cap={}|core={}",
            self.supply_reserve,
            self.max_transfers_per_round,
            self.core.tag()
        )
    }

    /// Parses [`LogisticsBuilderParams::tag`] back — strictly canonical.
    pub fn from_tag(tag: &str) -> Option<LogisticsBuilderParams> {
        let (own, core) = tag.split_once('|')?;
        let core = core.strip_prefix("core=")?;
        let parts: Vec<&str> = own.split(';').collect();
        if parts.len() != 2 {
            return None;
        }
        let (key_reserve, raw_reserve) = parts[0].split_once('=')?;
        let (key_cap, raw_cap) = parts[1].split_once('=')?;
        if key_reserve != "reserve" || key_cap != "cap" {
            return None;
        }
        Some(LogisticsBuilderParams {
            supply_reserve: u32::try_from(raw_reserve.parse::<u64>().ok()?).ok()?,
            max_transfers_per_round: u32::try_from(raw_cap.parse::<u64>().ok()?).ok()?,
            core: CoreParams::from_tag(core)?,
        })
    }
}

/// The `logistics-builder-r2` policy (version `r2.0`, family
/// `key-runner`).
pub struct LogisticsBuilder {
    core: BuilderCore,
    params: LogisticsBuilderParams,
}

impl LogisticsBuilder {
    pub fn new(
        faction: Faction,
        bot_seed: u64,
        cfg: &GameConfig,
        params: LogisticsBuilderParams,
    ) -> Self {
        LogisticsBuilder {
            core: BuilderCore::new(
                faction,
                bot_seed,
                crate::r2::rng::R2Stream::LogisticsBuilder,
                cfg,
                params.core,
            ),
            params,
        }
    }

    /// Whether `sender` may Transfer `keys_needed` of `target_key` and
    /// some Supply to `receiver` RIGHT NOW: both own agents, receiver
    /// inside the closed TRANSFER_RANGE, sender holds the key, and the
    /// Supply payload respects the sender's reserve and the receiver's
    /// headroom (a zero Supply payload is legal — keys only).
    fn transfer_payload(
        &self,
        sender: &arena_core::observe::AgentPrivate,
        receiver: &arena_core::observe::AgentPrivate,
        target_key: PortalId,
    ) -> Option<u32> {
        if sender.id == receiver.id {
            return None;
        }
        if !in_range(
            sender.position,
            receiver.position,
            self.core.cfg().transfer_range_cm,
        ) {
            return None;
        }
        if sender.keys.get(&target_key).copied().unwrap_or(0) < 1 {
            return None;
        }
        let headroom = self.core.cfg().max_supply.saturating_sub(receiver.supply);
        let spare = sender.supply.saturating_sub(self.params.supply_reserve);
        Some(headroom.min(spare))
    }

    /// The transport overlay, run AFTER core allocation: for every plan
    /// edge whose link is still missing with both vertices ours, if an
    /// agent WITHOUT the target key already stands in range of the source
    /// while another agent holds a spare key (and nobody can link the
    /// edge directly this round), the holder transfers the key (plus
    /// Supply headroom) instead of the source agent walking home. The
    /// receiver stands by (no Move) while a Transfer is in flight so the
    /// payload stays in range; the holder's delivery replaces its own
    /// round (idle holders are preferred, so a granted task is diverted
    /// only when no idle holder exists).
    fn transport_overlay(
        &self,
        obs: &Observation,
        grants: &BTreeMap<AgentId, Task>,
        commands: &mut BTreeMap<AgentId, CommandList>,
    ) {
        let Some(plan) = self.core.active().map(|p| p.vertices) else {
            return;
        };
        let own = common::owners(self.core.faction()).0;
        let mut transfers = 0u32;
        // Ascending edge order for determinism.
        let [a, b, c] = plan;
        let mut edges = [
            (a.min(b), a.max(b)),
            (a.min(c), a.max(c)),
            (b.min(c), c.max(b)),
        ];
        edges.sort();
        for (source, target) in edges {
            if transfers >= self.params.max_transfers_per_round {
                break;
            }
            let link_id =
                arena_core::types::LinkId::new(source, target).expect("distinct plan vertices");
            if obs
                .links
                .iter()
                .any(|l| l.id == link_id && l.faction == self.core.faction())
            {
                continue; // edge already ours
            }
            let (view_s, view_t) = (common::portal(obs, source), common::portal(obs, target));
            if view_s.owner != own || view_t.owner != own {
                continue;
            }
            // A direct link by a present key-holder beats any transport.
            let direct_linker = obs.own_agents.iter().any(|me| {
                me.keys.get(&target).copied().unwrap_or(0) >= 1
                    && in_range(
                        me.position,
                        view_s.position,
                        self.core.cfg().portal_operation_range_cm,
                    )
                    && common::link_is_legal(
                        obs,
                        own,
                        self.core.cfg(),
                        me.position,
                        &me.keys,
                        source,
                        target,
                    )
            });
            if direct_linker {
                continue;
            }
            // A stationed agent without the key, ascending id.
            let Some(stationed) = obs
                .own_agents
                .iter()
                .find(|me| {
                    in_range(
                        me.position,
                        view_s.position,
                        self.core.cfg().portal_operation_range_cm,
                    ) && me.keys.get(&target).copied().unwrap_or(0) == 0
                })
                .cloned()
            else {
                continue;
            };
            // Spare-key holders that can pay right now: idle ones first
            // (a granted task is diverted only when necessary), then the
            // nearest, then ascending id.
            let holder = obs
                .own_agents
                .iter()
                .filter(|me| {
                    me.id != stationed.id
                        && me.keys.get(&target).copied().unwrap_or(0) >= 1
                        && self.transfer_payload(me, &stationed, target).is_some()
                })
                .min_by_key(|me| {
                    (
                        u8::from(grants.contains_key(&me.id)),
                        arena_core::geometry::dist2(me.position, stationed.position),
                        me.id,
                    )
                })
                .cloned();
            let Some(holder) = holder else {
                continue;
            };
            let Some(supply) = self.transfer_payload(&holder, &stationed, target) else {
                continue;
            };
            // The receiver stands by this round: a Move could leave
            // TRANSFER_RANGE before the payload executes.
            commands.remove(&stationed.id);
            let mut list = CommandList::new();
            let mut keys = BTreeMap::new();
            keys.insert(target, 1u32);
            list.aux(Command::Transfer {
                target_agent: stationed.id,
                supply,
                keys,
            });
            commands.insert(holder.id, list);
            transfers += 1;
        }
    }
}

impl R2Policy for LogisticsBuilder {
    fn choose_spawns(
        &mut self,
        cfg: &GameConfig,
        map: &MapDefinition,
        roster: &[AgentId],
    ) -> Vec<(AgentId, Coord)> {
        self.core.choose_spawns(cfg, map, roster)
    }

    fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission> {
        self.core.refresh(obs);
        let agents: Vec<AgentId> = obs.own_agents.iter().map(|me| me.id).collect();
        let requests = if self.core.active().is_some() {
            self.core.build_requests(obs)
        } else {
            self.core.fallback_requests(obs)
        };
        let mut board = TaskBoard::new(obs.round);
        let grants = allocate(&mut board, &agents, &requests);
        let mut commands = self.core.execute(obs, &grants);
        self.transport_overlay(obs, &grants, &mut commands);
        submissions(obs, commands)
    }
}
