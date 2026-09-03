//! `logistics-builder-r2` (plan section 6.1, Key Runner + Builder): the
//! coordinated builder core plus the explicit Key/Supply runner scheme.
//! The command rule spends a Link target's key from the ACTING agent's
//! inventory — so when the plan's edge `(source, target)` has no
//! key-holder at the source, the core stations a keyless agent there
//! (the `Standby` task) while the key sits with a distant holder. This
//! policy commits the STATION as the edge's linker and dispatches the
//! holder as a runner: the holder's walking-Link request is replaced by
//! a [`Task::Transfer`] rendezvous — walk to the station, hand over the
//! key (plus Supply headroom), move on to the next delivery. The frozen
//! m6 baseline used Transfer 0 times in 10,000 matches; this policy
//! exists to measure the mechanism (plan 6.1).
//!
//! Every Transfer command passes observation-level pre-flight checks
//! (receiver is an own agent inside the closed `TRANSFER_RANGE`, sender
//! holds the key, Supply respects both the sender's recorded reserve and
//! the receiver's headroom to `MAX_SUPPLY`), so the emission is
//! drop-free by construction; execution races still record as normal
//! Drops (plan 6.3).

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
use crate::r2::tasks::{Task, TaskBoard, TaskRequest, allocate};

/// The recorded `logistics-builder-r2` parameters.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LogisticsBuilderParams {
    /// Supply the sender keeps after a Transfer (the runner's own reserve).
    pub supply_reserve: u32,
    /// At most this many Transfer deliveries per round (the transport
    /// headroom).
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
    /// Deliveries in flight: edge -> (station, runner?). STICKY: the
    /// assignment is remembered across rounds and only re-derived when
    /// it becomes invalid. `Some(runner)` = delivering; `None` =
    /// COMPLETING — the station holds the key and fires the edge through
    /// the core's Link path (a keyed station must never be re-derived as
    /// a runner, or station and runner just swap roles and ping-pong the
    /// key forever). Deterministic given the observation sequence.
    in_flight: BTreeMap<(PortalId, PortalId), (AgentId, Option<AgentId>)>,
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
            in_flight: BTreeMap::new(),
        }
    }

    /// Whether `sender` may hand a key of `target_key` (plus some Supply)
    /// to `receiver` RIGHT NOW: distinct own agents, receiver inside the
    /// closed TRANSFER_RANGE, sender holds the key, and the Supply
    /// payload respects the sender's reserve and the receiver's headroom
    /// (a zero Supply payload is legal — keys only). Returns the payable
    /// Supply amount.
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

    /// The runner-scheme request transformation (pre-allocation): for
    /// every plan edge still missing, both vertices ours, with NO
    /// key-holder in operation range of the source, the station is the
    /// committed linker — every holder's walking `Link` request for that
    /// edge is replaced by a `Transfer` rendezvous toward the station
    /// agent (the keyless agent nearest the source — exactly the agent
    /// the allocator grants the core's `Standby` task to). The rendezvous
    /// walks at the delivery value: the plan's per-round income minus the
    /// sender's travel to the station.
    fn apply_runner_scheme(
        &mut self,
        obs: &Observation,
        requests: &mut BTreeMap<AgentId, Vec<TaskRequest>>,
    ) {
        let Some(plan) = self.core.active().map(|p| p.vertices) else {
            self.in_flight.clear();
            return;
        };
        let own = common::owners(self.core.faction()).0;
        let agent = |id: AgentId| obs.own_agents.iter().find(|me| me.id == id);
        let [a, b, c] = plan;
        let mut edges = [
            (a.min(b), a.max(b)),
            (a.min(c), a.max(c)),
            (b.min(c), c.max(b)),
        ];
        edges.sort();
        let mut base: Option<i64> = None;
        for (source, target) in edges {
            let link_id =
                arena_core::types::LinkId::new(source, target).expect("distinct plan vertices");
            let fired = obs
                .links
                .iter()
                .any(|l| l.id == link_id && l.faction == self.core.faction());
            let view_s = common::portal(obs, source);
            let view_t = common::portal(obs, target);
            let buildable = !fired && view_s.owner == own && view_t.owner == own;
            if !buildable {
                self.in_flight.remove(&(source, target));
                continue;
            }
            // Advance the sticky state machine.
            if let Some(&(station, runner)) = self.in_flight.get(&(source, target)) {
                if agent(station).is_none() {
                    self.in_flight.remove(&(source, target));
                } else if agent(station)
                    .expect("just checked")
                    .keys
                    .get(&target)
                    .copied()
                    .unwrap_or(0)
                    >= 1
                {
                    // COMPLETING: the station holds the key — its own
                    // core Link request fires the edge. Never re-derive.
                    self.in_flight.insert((source, target), (station, None));
                } else if let Some(runner) = runner {
                    let runner_ready = agent(runner).is_some_and(|me| {
                        me.id != station && me.keys.get(&target).copied().unwrap_or(0) >= 1
                    });
                    if !runner_ready {
                        self.in_flight.remove(&(source, target));
                    }
                }
            }
            // A key-holder already in operation range of the source fires
            // the edge directly — UNLESS a delivery is completing (the
            // station is that holder) or its runner has arrived (the
            // payment beats the direct link by policy identity, and the
            // station fires next round).
            let holder_in_range = obs.own_agents.iter().any(|me| {
                me.keys.get(&target).copied().unwrap_or(0) >= 1
                    && in_range(
                        me.position,
                        view_s.position,
                        self.core.cfg().portal_operation_range_cm,
                    )
            });
            let completing = self
                .in_flight
                .get(&(source, target))
                .is_some_and(|&(_, runner)| runner.is_none());
            let delivery_armed =
                self.in_flight
                    .get(&(source, target))
                    .is_some_and(|&(station, runner)| {
                        runner.is_some_and(|runner| {
                            agent(station).is_some_and(|s| {
                                agent(runner).is_some_and(|r| {
                                    in_range(
                                        r.position,
                                        s.position,
                                        self.core.cfg().transfer_range_cm,
                                    )
                                })
                            })
                        })
                    });
            if holder_in_range && !completing && !delivery_armed {
                self.in_flight.remove(&(source, target));
                continue;
            }
            // (Re-)derive a fresh assignment when none is in flight.
            if !self.in_flight.contains_key(&(source, target)) {
                let stations: std::collections::BTreeSet<AgentId> = self
                    .in_flight
                    .values()
                    .map(|(station, _)| *station)
                    .collect();
                let Some(station) = obs
                    .own_agents
                    .iter()
                    .filter(|me| {
                        me.keys.get(&target).copied().unwrap_or(0) == 0
                            && !stations.contains(&me.id)
                    })
                    .min_by_key(|me| {
                        (
                            arena_core::geometry::dist2(me.position, view_s.position),
                            me.id,
                        )
                    })
                    .map(|me| me.id)
                else {
                    continue;
                };
                let Some(runner) = obs
                    .own_agents
                    .iter()
                    .filter(|me| {
                        me.id != station
                            && me.keys.get(&target).copied().unwrap_or(0) >= 1
                            && !stations.contains(&me.id)
                    })
                    .min_by_key(|me| {
                        (
                            arena_core::geometry::dist2(me.position, view_s.position),
                            me.id,
                        )
                    })
                    .map(|me| me.id)
                else {
                    continue;
                };
                self.in_flight
                    .insert((source, target), (station, Some(runner)));
            }
            // In the DELIVERING state, replace the runner's walking Link
            // request for this edge with the Transfer rendezvous at the
            // delivery value: the FULL edge value minus a single unit
            // (the Key Runner commits to the station — a delivery
            // outranks every walking task of the same plan and yields
            // only to work an agent is already standing on; that
            // commitment is the policy's identity, traded against the
            // one-round payment delay). The COMPLETING state strips
            // nothing: the station's core Link request fires the edge.
            if let Some(&(station, Some(runner))) = self.in_flight.get(&(source, target)) {
                let value = *base.get_or_insert_with(|| {
                    crate::r2::value::field_value(
                        obs,
                        self.core.faction(),
                        self.core.cfg(),
                        &self.core.params().value,
                        self.core.active().expect("active plan"),
                    )
                });
                if let Some(mine) = requests.get_mut(&runner) {
                    mine.retain(|request| request.task != Task::Link { source, target });
                    mine.push(TaskRequest::one_shot(
                        Task::Transfer {
                            receiver: station,
                            key: target,
                        },
                        value.saturating_sub(1),
                    ));
                }
            }
        }
    }

    /// The payment step (post-allocation): a holder granted a Transfer
    /// rendezvous pays the station the moment it is inside the closed
    /// TRANSFER_RANGE — the key plus Supply headroom, drop-free by
    /// construction. The station stands by (its `Standby` execution
    /// already emits nothing once in range) so the payload cannot be
    /// outrun, and once it holds the key the edge's Link request is its
    /// own — the scheme never runs twice for one edge.
    fn pay_deliveries(
        &self,
        obs: &Observation,
        grants: &BTreeMap<AgentId, Task>,
        commands: &mut BTreeMap<AgentId, CommandList>,
    ) {
        let mut paid = 0u32;
        let mut payments: Vec<(AgentId, AgentId, PortalId, u32)> = Vec::new();
        for (agent, task) in grants.iter() {
            let Task::Transfer { receiver, key } = task else {
                continue;
            };
            if paid >= self.params.max_transfers_per_round {
                break;
            }
            let Some(sender) = obs.own_agents.iter().find(|me| me.id == *agent) else {
                continue;
            };
            let Some(station) = obs.own_agents.iter().find(|me| me.id == *receiver) else {
                continue;
            };
            if let Some(supply) = self.transfer_payload(sender, station, *key) {
                payments.push((sender.id, station.id, *key, supply));
                paid += 1;
            }
        }
        for (sender_id, station_id, key, supply) in payments {
            let mut list = CommandList::new();
            let mut keys = BTreeMap::new();
            keys.insert(key, 1u32);
            list.aux(Command::Transfer {
                target_agent: station_id,
                supply,
                keys,
            });
            commands.remove(&station_id);
            commands.insert(sender_id, list);
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
        let mut requests = if self.core.active().is_some() {
            self.core.build_requests(obs)
        } else {
            self.core.fallback_requests(obs)
        };
        self.apply_runner_scheme(obs, &mut requests);
        let mut board = TaskBoard::new(obs.round);
        let grants = allocate(&mut board, &agents, &requests);
        let mut commands = self.core.execute(obs, &grants);
        self.pay_deliveries(obs, &grants, &mut commands);
        submissions(obs, commands)
    }
}
