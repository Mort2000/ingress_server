//! Key delivery over the shared builder core. Only a closing-edge schedule
//! that advances field completion can displace direct linking; all feasible
//! sender/receiver pairs are compared against the same allocation preview.
//! Delivery is Key-only because Link consumes no Supply. Sender streams
//! settle first within the faction, so receivers can Move + Link immediately
//! after payment. Public-state races remain possible and are recorded as Drops.

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
use crate::r2::tasks::{Task, TaskRequest};

/// The recorded `logistics-builder-r2` parameters.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LogisticsBuilderParams {
    /// Legacy Supply reserve retained in parameter records. r2.2 only
    /// delivers the Key needed for Link, which consumes no Supply.
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

/// The `logistics-builder-r2` policy (version [`crate::r2::R2_VERSION`], family
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
    comparisons: Vec<DeliveryComparison>,
}

/// Read-only evidence for the last decision. Delays are zero-based future
/// decision offsets: zero means the current round, one means next round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryComparison {
    pub round: u32,
    pub source: PortalId,
    pub target: PortalId,
    pub runner: AgentId,
    pub station: AgentId,
    pub direct_delay: Option<u32>,
    pub delivery_delay: Option<u32>,
    pub meeting_delay: Option<u32>,
    pub station_move_rounds: u32,
    pub station_wait_rounds: u32,
    pub receiver_task: Option<Task>,
    pub net_value: i64,
    pub decision: DeliveryDecision,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryDecision {
    Accepted,
    OtherEdgesPending,
    Unreachable,
    DirectNoLater,
    ReceiverBusy,
    NonPositiveValue,
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
            comparisons: Vec::new(),
        }
    }

    pub fn delivery_comparisons(&self) -> &[DeliveryComparison] {
        &self.comparisons
    }

    fn compare_delivery(
        &self,
        obs: &Observation,
        source: PortalId,
        target: PortalId,
        runner: AgentId,
        station: AgentId,
        receiver_task: Option<Task>,
    ) -> DeliveryComparison {
        let cfg = self.core.cfg();
        let own = common::owners(self.core.faction()).0;
        let sender = obs
            .own_agents
            .iter()
            .find(|a| a.id == runner)
            .expect("selected runner");
        let receiver = obs
            .own_agents
            .iter()
            .find(|a| a.id == station)
            .expect("selected station");
        let mut comparison =
            DeliveryComparison {
                round: obs.round,
                source,
                target,
                runner,
                station,
                direct_delay: obs
                    .own_agents
                    .iter()
                    .flat_map(|agent| {
                        [(source, target), (target, source)].into_iter().filter_map(
                            move |(s, t)| common::link_completion_delay(obs, own, cfg, agent, s, t),
                        )
                    })
                    .min(),
                delivery_delay: None,
                meeting_delay: None,
                station_move_rounds: 0,
                station_wait_rounds: 0,
                receiver_task,
                net_value: 0,
                decision: DeliveryDecision::Unreachable,
            };
        let plan = self.core.active().expect("active plan");
        let closing_edge = plan
            .vertices
            .iter()
            .filter(|&&p| p != source && p != target)
            .all(|&third| {
                [source, target].iter().all(|&end| {
                    obs.links.iter().any(|l| {
                        l.faction == self.core.faction()
                            && l.id == arena_core::types::LinkId::new(third, end).unwrap()
                    })
                })
            });
        if !closing_edge {
            comparison.decision = DeliveryDecision::OtherEdgesPending;
            return comparison;
        }
        let mut sender_position = sender.position;
        let mut receiver_position = receiver.position;
        let source_position = common::portal(obs, source).position;
        let remaining = cfg.total_rounds.saturating_sub(obs.round).saturating_add(1);
        for delay in 0..remaining {
            let receiver_next = if in_range(
                receiver_position,
                source_position,
                cfg.portal_operation_range_cm,
            ) {
                receiver_position
            } else {
                common::step_toward(receiver_position, source_position, cfg.move_budget_cm)
            };
            if in_range(sender_position, receiver_position, cfg.transfer_range_cm) {
                comparison.meeting_delay = Some(delay);
                // The sender's stream precedes the receiver's. After payment
                // the receiver may spend its full Move budget and Link now.
                comparison.station_move_rounds += u32::from(receiver_next != receiver_position);
                let mut keyed_receiver = receiver.clone();
                keyed_receiver.position = receiver_position;
                keyed_receiver.keys.insert(target, 1);
                comparison.delivery_delay =
                    common::link_completion_delay(obs, own, cfg, &keyed_receiver, source, target)
                        .and_then(|walk| delay.checked_add(walk));
                break;
            }
            let sender_next =
                common::step_toward(sender_position, receiver_position, cfg.move_budget_cm);
            comparison.station_move_rounds += u32::from(receiver_next != receiver_position);
            comparison.station_wait_rounds += u32::from(receiver_next == receiver_position);
            if sender_next == sender_position && receiver_next == receiver_position {
                break;
            }
            sender_position = sender_next;
            receiver_position = receiver_next;
        }
        let (Some(delivery), Some(meeting)) = (comparison.delivery_delay, comparison.meeting_delay)
        else {
            return comparison;
        };
        if delivery >= remaining {
            return comparison;
        }
        let direct = comparison.direct_delay.unwrap_or(remaining);
        if direct <= delivery {
            comparison.decision = DeliveryDecision::DirectNoLater;
            return comparison;
        }
        // The receiver may keep a source-local auxiliary Hack, or wait at
        // this station. Committing an agent to other work is not treated as free.
        if receiver_task.is_some_and(|task| {
            !matches!(task,
            Task::Standby { portal } | Task::KeyFarm { portal } if portal == source)
        }) {
            comparison.decision = DeliveryDecision::ReceiverBusy;
            return comparison;
        }
        comparison.net_value = crate::r2::value::transfer_value(
            &self.core.economy(obs),
            plan.area2,
            direct - delivery,
            meeting,
        );
        comparison.decision = if comparison.net_value > 0 {
            DeliveryDecision::Accepted
        } else {
            DeliveryDecision::NonPositiveValue
        };
        comparison
    }

    /// A payable Key-only delivery. Link consumes no Supply, so no Supply
    /// is moved without a consumer in the selected completion plan.
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
        Some(0)
    }

    /// The runner-scheme request transformation (pre-allocation): for
    /// missing active-plan edge, propose a receiver and holder, then admit
    /// only a delivery that advances complete field creation without
    /// displacing the receiver's productive baseline grant. Both roles
    /// follow the compared source/rendezvous paths; positions and competing
    /// tasks are rechecked each round before keeping the commitment.
    fn apply_runner_scheme(
        &mut self,
        obs: &Observation,
        requests: &mut BTreeMap<AgentId, Vec<TaskRequest>>,
    ) {
        self.comparisons.clear();
        if self.params.max_transfers_per_round == 0 {
            self.in_flight.clear();
            return;
        }
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
        self.in_flight.retain(|edge, _| edges.contains(edge));
        let baseline = self.core.preview_allocation(obs, requests);
        let base_grants: BTreeMap<_, _> = baseline
            .agents
            .iter()
            .filter_map(|agent| agent.grant.map(|task| (agent.agent, task)))
            .collect();
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
            // Propose roles, then compare complete schedules below on every round.
            if !self.in_flight.contains_key(&(source, target)) {
                let stations: std::collections::BTreeSet<AgentId> = self
                    .in_flight
                    .values()
                    .map(|(station, _)| *station)
                    .collect();
                let mut best: Option<DeliveryComparison> = None;
                for station in obs.own_agents.iter().filter(|me| {
                    me.keys.get(&target).copied().unwrap_or(0) == 0 && !stations.contains(&me.id)
                }) {
                    for runner in obs.own_agents.iter().filter(|me| {
                        me.id != station.id
                            && me.keys.get(&target).copied().unwrap_or(0) > 0
                            && !stations.contains(&me.id)
                    }) {
                        let comparison = self.compare_delivery(
                            obs,
                            source,
                            target,
                            runner.id,
                            station.id,
                            base_grants.get(&station.id).copied(),
                        );
                        if comparison.decision == DeliveryDecision::Accepted {
                            if best
                                .as_ref()
                                .is_none_or(|previous| comparison.net_value > previous.net_value)
                            {
                                best = Some(comparison);
                            }
                        } else {
                            self.comparisons.push(comparison);
                        }
                    }
                }
                let Some(best) = best else {
                    continue;
                };
                self.in_flight
                    .insert((source, target), (best.station, Some(best.runner)));
            }
            // Re-evaluate commitments as positions and alternative work change.
            if let Some(&(station, Some(runner))) = self.in_flight.get(&(source, target)) {
                let comparison = self.compare_delivery(
                    obs,
                    source,
                    target,
                    runner,
                    station,
                    base_grants.get(&station).copied(),
                );
                let accepted = comparison.decision == DeliveryDecision::Accepted;
                self.comparisons.push(comparison);
                if !accepted {
                    self.in_flight.remove(&(source, target));
                    continue;
                }
            }
            // Accepted closing-edge delivery: station approaches the source,
            // runner meets it; completing stations keep their normal Link task.
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
                let per_round = self.core.economy(obs).opportunity_per_round;
                let view_s = common::portal(obs, source);
                if let Some(station_me) = obs.own_agents.iter().find(|me| me.id == station) {
                    if let Some(mine) = requests.get_mut(&station) {
                        // The baseline allocation admitted this receiver only
                        // when it had no competing productive grant. Releasing
                        // the runner's Link reservation must not divert it into
                        // a formerly blocked reverse Link: the accepted schedule
                        // requires approaching this source before payment.
                        mine.retain(|request| {
                            matches!(request.task,
                            Task::KeyFarm { portal } | Task::Standby { portal } if portal == source)
                        });
                        let travel = crate::r2::value::travel_rounds_between(
                            station_me.position,
                            view_s.position,
                            self.core.cfg().move_budget_cm,
                        );
                        let priority = value.saturating_sub(1).saturating_sub(
                            (travel as i128 * per_round as i128).min(i64::MAX as i128) as i64,
                        );
                        mine.push(TaskRequest::one_shot(
                            Task::Standby { portal: source },
                            priority,
                        ));
                    }
                }
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

    /// Pay accepted deliveries before receiver movement, preserve local
    /// auxiliary work, and consume the received Key in a same-round Link
    /// when the compared source is reachable with this Move budget.
    fn pay_deliveries(
        &self,
        obs: &Observation,
        grants: &BTreeMap<AgentId, Task>,
        commands: &mut BTreeMap<AgentId, CommandList>,
    ) {
        let mut paid = 0u32;
        let mut payments: Vec<(AgentId, AgentId, PortalId, PortalId, u32)> = Vec::new();
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
                let source = self
                    .in_flight
                    .iter()
                    .find(|((_, target), (receiver, runner))| {
                        *target == *key && *receiver == station.id && *runner == Some(sender.id)
                    })
                    .map(|((source, _), _)| *source)
                    .expect("granted delivery has a source");
                payments.push((sender.id, station.id, source, *key, supply));
                paid += 1;
            }
        }
        for (sender_id, station_id, source, key, supply) in payments {
            let mut list = CommandList::new();
            let mut keys = BTreeMap::new();
            keys.insert(key, 1u32);
            list.aux(Command::Transfer {
                target_agent: station_id,
                supply,
                keys,
            });
            // Accepted receivers hold source-local work, never another
            // Transfer. Submission ordering therefore pays the Key before
            // their Move or Link. Preserve any planned source-local Hack.
            let mut receiver = obs
                .own_agents
                .iter()
                .find(|agent| agent.id == station_id)
                .unwrap()
                .clone();
            receiver
                .keys
                .insert(key, receiver.keys.get(&key).copied().unwrap_or(0) + 1);
            if let Some(position) = common::link_position_this_round(
                obs,
                common::owners(self.core.faction()).0,
                self.core.cfg(),
                &receiver,
                source,
                key,
            ) {
                let mut station_commands = CommandList::new();
                for command in commands.remove(&station_id).unwrap_or_default().build() {
                    if !matches!(command, Command::Move { .. }) {
                        station_commands.aux(command);
                    }
                }
                if position != receiver.position {
                    station_commands.aux(Command::Move {
                        destination: position,
                    });
                }
                station_commands.aux(Command::Link {
                    source_portal: source,
                    target_portal: key,
                });
                commands.insert(station_id, station_commands);
            }
            commands.insert(sender_id, list);
        }
    }
}

impl R2Policy for LogisticsBuilder {
    fn decision_trace(&self) -> Option<&crate::r2::build::DecisionTrace> {
        self.core.decision_trace()
    }

    fn delivery_comparisons(&self) -> &[DeliveryComparison] {
        &self.comparisons
    }

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
        let mut requests = if self.core.active().is_some() {
            self.core.build_requests(obs)
        } else {
            self.core.fallback_requests(obs)
        };
        self.apply_runner_scheme(obs, &mut requests);
        let mut commands = self.core.allocate_and_execute(obs, &requests);
        let grants = self
            .core
            .decision_trace()
            .expect("just recorded")
            .agents
            .iter()
            .filter_map(|a| a.grant.map(|task| (a.agent, task)))
            .collect();
        self.pay_deliveries(obs, &grants, &mut commands);
        submissions(obs, commands)
    }
}
