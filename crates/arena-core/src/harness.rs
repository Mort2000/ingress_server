//! In-process match harness (P1-LOOP): owns the `GameState`, a `Resolver`,
//! and both factions' strategies, and runs rounds without any transport or
//! wall clock. Local bots see the same `Observation` visibility rules as
//! external clients; the harness injects a virtual receive time derived
//! from the strategy's stated bucket (D18, ADR-0002) and never reads a
//! real clock.

use crate::map::MapDefinition;
use crate::observe::Observation;
use crate::resolve::{Resolver, RoundReport, SubmittedCommandStream};
use crate::spawn::InitialSpawns;
use crate::state::{GameState, StateError};
use crate::types::{AgentId, Command, Faction};

/// One agent's planned submission for a round, as returned by a strategy.
/// `bucket` is the virtual Priority Bucket (D18): the harness derives
/// `received_at_ms = bucket * priority_bucket_ms` (checked; panics with a
/// diagnostic rather than wrapping — see [`virtual_receive_time`]);
/// strategies that do not care about ordering use bucket 0 (the ADR-0002
/// default).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AgentSubmission {
    pub agent_id: AgentId,
    pub bucket: u64,
    pub commands: Vec<Command>,
}

impl AgentSubmission {
    /// Convenience constructor for the common default-bucket case.
    pub fn new(agent_id: AgentId, commands: Vec<Command>) -> Self {
        AgentSubmission {
            agent_id,
            bucket: 0,
            commands,
        }
    }
}

/// Derives the virtual receive time from a strategy's stated Priority
/// Bucket (D18): `bucket * priority_bucket_ms`. Checked per D9 — a bucket
/// whose receive time would overflow u64 panics with a diagnostic instead
/// of silently wrapping and mis-ordering streams (a wrapped
/// `received_at_ms` is the authoritative ordering input of R-RESOLVE-03,
/// so quiet wrapping would corrupt ordering in release builds, where
/// plain `*` wraps instead of panicking).
pub fn virtual_receive_time(bucket: u64, bucket_ms: u64) -> u64 {
    bucket
        .checked_mul(bucket_ms)
        .expect("virtual receive time = bucket * priority_bucket_ms must not overflow u64")
}

/// A faction's round planner: receives the faction's round-start
/// `Observation` and returns that faction's submissions in collection
/// order (the harness assigns `receive_seq` by position in the Vec).
pub trait RoundStrategy {
    fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission>;
}

/// Deterministic strategy playing a pre-written script, one Vec of
/// submissions per round. Panics once the script runs out — a scripted
/// match must know its length up front.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ScriptedStrategy {
    pub per_round: Vec<Vec<AgentSubmission>>,
}

impl ScriptedStrategy {
    pub fn new(per_round: Vec<Vec<AgentSubmission>>) -> Self {
        ScriptedStrategy { per_round }
    }
}

impl RoundStrategy for ScriptedStrategy {
    fn submit_round(&mut self, _obs: &Observation) -> Vec<AgentSubmission> {
        if self.per_round.is_empty() {
            panic!("script exhausted");
        }
        self.per_round.remove(0)
    }
}

/// One scripted match: state + resolver + both strategies.
pub struct MatchHarness {
    state: GameState,
    resolver: Resolver,
    enl: Box<dyn RoundStrategy>,
    res: Box<dyn RoundStrategy>,
}

impl MatchHarness {
    /// Builds a match from its deterministic setup inputs: the `game_seed`
    /// feeds the resolver's random oracle (D33, D15 — the seed is Replay
    /// Header material and never lives in the state), then config, frozen
    /// map, both spawn sets, and both strategies.
    pub fn new(
        game_seed: u64,
        cfg: crate::config::GameConfig,
        map: &MapDefinition,
        enl_spawns: &InitialSpawns,
        res_spawns: &InitialSpawns,
        enl: Box<dyn RoundStrategy>,
        res: Box<dyn RoundStrategy>,
    ) -> Result<Self, StateError> {
        Ok(MatchHarness {
            state: GameState::new(cfg, map, enl_spawns, res_spawns)?,
            resolver: Resolver::with_seed(game_seed),
            enl,
            res,
        })
    }

    /// Runs one full round: build both observations (ENL first —
    /// deterministic), poll both strategies, tag submissions with faction,
    /// virtual receive time, and per-faction collection order, settle, then
    /// advance the round (which resets every tracker).
    pub fn run_round(&mut self) -> RoundReport {
        let round = self.state.round();
        let bucket_ms = self.state.config().priority_bucket_ms as u64;

        let obs = Observation::for_faction(&self.state, Faction::Enl);
        let enl_submissions = self.enl.submit_round(&obs);
        let obs = Observation::for_faction(&self.state, Faction::Res);
        let res_submissions = self.res.submit_round(&obs);

        let mut submissions = Vec::with_capacity(enl_submissions.len() + res_submissions.len());
        for (seq, submission) in enl_submissions.into_iter().enumerate() {
            submissions.push(SubmittedCommandStream {
                round,
                faction: Faction::Enl,
                agent_id: submission.agent_id,
                received_at_ms: virtual_receive_time(submission.bucket, bucket_ms),
                receive_seq: seq as u32,
                commands: submission.commands,
            });
        }
        for (seq, submission) in res_submissions.into_iter().enumerate() {
            submissions.push(SubmittedCommandStream {
                round,
                faction: Faction::Res,
                agent_id: submission.agent_id,
                received_at_ms: virtual_receive_time(submission.bucket, bucket_ms),
                receive_seq: seq as u32,
                commands: submission.commands,
            });
        }

        let report = self.resolver.run_round(&mut self.state, &submissions);
        self.state.advance_round();
        report
    }

    /// Runs `rounds` consecutive rounds, returning the reports in order.
    pub fn run_rounds(&mut self, rounds: u32) -> Vec<RoundReport> {
        (0..rounds).map(|_| self.run_round()).collect()
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }

    pub fn into_state(self) -> GameState {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GameConfig;
    use crate::event::EventKind;
    use crate::map::MapDefinition;
    use crate::spawn::validate_initial_spawn_set;
    use crate::types::{Coord, Owner, Portal};

    fn gate_config(agents_per_faction: u32) -> GameConfig {
        let mut cfg = GameConfig::default();
        cfg.map_width_cm = 30_000;
        cfg.map_height_cm = 30_000;
        cfg.portal_count = 2;
        cfg.portal_cluster_count = 2;
        cfg.portal_min_distance_cm = 4_000;
        cfg.initial_agents_per_faction = agents_per_faction;
        cfg
    }

    fn contested_map() -> MapDefinition {
        MapDefinition::new(vec![
            Portal {
                id: crate::types::PortalId(0),
                position: Coord::new(10_000, 10_000),
                owner: Owner::Neutral,
                hp: 0,
            },
            Portal {
                id: crate::types::PortalId(1),
                position: Coord::new(25_000, 25_000),
                owner: Owner::Neutral,
                hp: 0,
            },
        ])
    }

    fn spawns(cfg: &GameConfig, faction: Faction, coords: &[Coord]) -> InitialSpawns {
        let (enl_roster, res_roster) = GameState::initial_roster(cfg);
        let roster = if faction == Faction::Enl {
            &enl_roster
        } else {
            &res_roster
        };
        let entries: Vec<(crate::types::AgentId, Coord)> =
            roster.iter().copied().zip(coords.iter().copied()).collect();
        validate_initial_spawn_set(roster, &entries, cfg).expect("fixture spawns")
    }

    // Checklist 43: virtual receive bookkeeping (D18).
    #[test]
    fn receive_seq_follows_collection_order_and_bucket_derives_receive_time() {
        let cfg = gate_config(2);
        let map = contested_map();
        let enl_spawns = spawns(
            &cfg,
            Faction::Enl,
            &[Coord::new(10_000, 5_000), Coord::new(10_000, 6_000)],
        );
        let res_spawns = spawns(
            &cfg,
            Faction::Res,
            &[Coord::new(6_000, 10_000), Coord::new(6_000, 11_000)],
        );
        // Round 1: agent 1 is returned first (receive_seq 0) but states a
        // later bucket; agent 0 is second (receive_seq 1) in bucket 0.
        let enl = ScriptedStrategy::new(vec![
            vec![
                AgentSubmission {
                    agent_id: AgentId(1),
                    bucket: 1,
                    commands: vec![Command::Move {
                        destination: Coord::new(10_000, 6_000),
                    }],
                },
                AgentSubmission {
                    agent_id: AgentId(0),
                    bucket: 0,
                    commands: vec![Command::Move {
                        destination: Coord::new(10_000, 5_000),
                    }],
                },
            ],
            // Round 2: default bucket 0 for both; order follows the Vec.
            vec![
                AgentSubmission::new(
                    AgentId(0),
                    vec![Command::Move {
                        destination: Coord::new(10_000, 5_000),
                    }],
                ),
                AgentSubmission::new(
                    AgentId(1),
                    vec![Command::Move {
                        destination: Coord::new(10_000, 6_000),
                    }],
                ),
            ],
        ]);
        let res = ScriptedStrategy::new(vec![vec![], vec![]]);
        let mut harness = MatchHarness::new(
            0,
            cfg,
            &map,
            &enl_spawns,
            &res_spawns,
            Box::new(enl),
            Box::new(res),
        )
        .expect("harness builds");

        let first = harness.run_round();
        // Bucket 0 settles before bucket 1 regardless of collection order.
        assert_eq!(first.execution_order[0].agent_id, AgentId(0));
        assert_eq!(first.execution_order[1].agent_id, AgentId(1));
        // Both bookkeeping facts are recorded per stream.
        assert_eq!(
            first.slots.get(&AgentId(0)),
            Some(&crate::resolve::SlotStatus::Executed {
                bucket: 0,
                receive_seq: 1
            })
        );
        assert_eq!(
            first.slots.get(&AgentId(1)),
            Some(&crate::resolve::SlotStatus::Executed {
                bucket: 1,
                receive_seq: 0
            })
        );
        assert_eq!(first.execution_order[1].receive_bucket, 1);
        assert_eq!(first.execution_order[1].receive_seq, 0);

        let second = harness.run_round();
        // All bucket 0: per-faction arrival order decides (default ADR-0002).
        assert_eq!(second.execution_order[0].agent_id, AgentId(0));
        assert_eq!(second.execution_order[1].agent_id, AgentId(1));
        assert_eq!(
            second.slots.get(&AgentId(2)),
            Some(&crate::resolve::SlotStatus::NotSubmitted)
        );
    }

    // Checklist 44: each strategy sees only its own faction's observation.
    #[test]
    fn strategies_receive_only_their_own_faction_observation() {
        use std::sync::{Arc, Mutex};

        struct RecordingStrategy {
            expected_agent: AgentId,
            captured: Arc<Mutex<Vec<Observation>>>,
        }
        impl RoundStrategy for RecordingStrategy {
            fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission> {
                // The view carries exactly one agent and it is ours.
                assert_eq!(obs.own_agents.len(), 1);
                assert_eq!(obs.own_agents[0].id, self.expected_agent);
                self.captured.lock().expect("uncontended").push(obs.clone());
                vec![]
            }
        }

        let cfg = gate_config(1);
        let map = contested_map();
        let enl_spawns = spawns(&cfg, Faction::Enl, &[Coord::new(10_000, 5_000)]);
        let res_spawns = spawns(&cfg, Faction::Res, &[Coord::new(6_000, 10_000)]);
        let enl_captured = Arc::new(Mutex::new(Vec::new()));
        let res_captured = Arc::new(Mutex::new(Vec::new()));
        let enl = RecordingStrategy {
            expected_agent: AgentId(0),
            captured: Arc::clone(&enl_captured),
        };
        let res = RecordingStrategy {
            expected_agent: AgentId(1),
            captured: Arc::clone(&res_captured),
        };
        let mut harness = MatchHarness::new(
            0,
            cfg,
            &map,
            &enl_spawns,
            &res_spawns,
            Box::new(enl),
            Box::new(res),
        )
        .expect("harness builds");
        let _ = harness.run_rounds(2);

        // Each recording saw two rounds of its own single agent; the other
        // faction's agent id never appears in the opposite view.
        let enl_views = enl_captured.lock().expect("uncontended");
        let res_views = res_captured.lock().expect("uncontended");
        assert_eq!(enl_views.len(), 2);
        assert_eq!(res_views.len(), 2);
        for obs in enl_views.iter() {
            let ids: Vec<AgentId> = obs.own_agents.iter().map(|a| a.id).collect();
            assert_eq!(ids, vec![AgentId(0)]);
        }
        for obs in res_views.iter() {
            let ids: Vec<AgentId> = obs.own_agents.iter().map(|a| a.id).collect();
            assert_eq!(ids, vec![AgentId(1)]);
        }
    }

    // Checklist 45 — the M2 acceptance gate.
    #[test]
    fn gate_two_agents_contest_one_portal_across_five_rounds() {
        let mut cfg = gate_config(1);
        cfg.attack_damage = 60;
        let map = contested_map();
        let enl_spawns = spawns(&cfg, Faction::Enl, &[Coord::new(10_000, 5_000)]);
        let res_spawns = spawns(&cfg, Faction::Res, &[Coord::new(6_000, 10_000)]);
        let p0 = crate::types::PortalId(0);

        let enl = ScriptedStrategy::new(vec![
            vec![AgentSubmission::new(
                AgentId(0),
                vec![Command::Capture { portal: p0 }],
            )],
            vec![AgentSubmission::new(
                AgentId(0),
                vec![Command::Recharge { portal: p0 }],
            )],
            vec![],
            vec![AgentSubmission::new(
                AgentId(0),
                vec![Command::Capture { portal: p0 }],
            )],
            vec![AgentSubmission::new(
                AgentId(0),
                vec![Command::Recharge { portal: p0 }],
            )],
        ]);
        let res = ScriptedStrategy::new(vec![
            vec![AgentSubmission::new(
                AgentId(1),
                vec![Command::Capture { portal: p0 }],
            )],
            vec![AgentSubmission::new(AgentId(1), vec![Command::Attack])],
            vec![AgentSubmission::new(AgentId(1), vec![Command::Attack])],
            vec![],
            vec![],
        ]);
        let mut harness = MatchHarness::new(
            0,
            cfg,
            &map,
            &enl_spawns,
            &res_spawns,
            Box::new(enl),
            Box::new(res),
        )
        .expect("harness builds");

        // Round 1: odd round, both bucket 0 => ENL settles first and wins
        // the double-CAPTURE conflict (R-RESOLVE-04).
        let first = harness.run_round();
        assert_eq!(first.execution_order.len(), 2);
        assert_eq!(first.execution_order[0].faction, Faction::Enl);
        assert_eq!(
            first.execution_order[0].outcomes[0],
            crate::resolve::CommandOutcome::Ok(crate::resolve::CommandEffect::Captured {
                portal: p0
            })
        );
        assert_eq!(
            first.execution_order[1].outcomes[0],
            crate::resolve::CommandOutcome::Drop(
                crate::resolve::DropReason::CaptureTargetNotNeutral {
                    portal: p0,
                    owner: Owner::Enl
                }
            )
        );
        // Trackers reset between rounds: R1's capture spent ENL's major.
        assert_eq!(
            harness.state().tracker(AgentId(0)),
            Some(&Default::default())
        );
        assert_eq!(harness.state().round(), 2);

        let rest = harness.run_rounds(4);
        let reports: Vec<&RoundReport> = std::iter::once(&first).chain(rest.iter()).collect();

        // Round 2 (even => RES first): attack 100 -> 40, then ENL's
        // defensive recharge heals 40 -> 55.
        assert_eq!(
            reports[1].execution_order[0].outcomes[0],
            crate::resolve::CommandOutcome::Ok(crate::resolve::CommandEffect::Attacked {
                hits: vec![crate::resolve::AttackHit {
                    portal: p0,
                    hp_after: 40,
                    neutralized: false
                }]
            })
        );
        assert_eq!(
            reports[1].execution_order[1].outcomes[0],
            crate::resolve::CommandOutcome::Ok(crate::resolve::CommandEffect::Recharged {
                portal: p0,
                hp_before: 40,
                hp_after: 55
            })
        );

        // Round 3: 55 - 60 clamps to 0 => neutralized; ENL auto-PASSes.
        assert_eq!(
            reports[2].execution_order[0].outcomes[0],
            crate::resolve::CommandOutcome::Ok(crate::resolve::CommandEffect::Attacked {
                hits: vec![crate::resolve::AttackHit {
                    portal: p0,
                    hp_after: 0,
                    neutralized: true
                }]
            })
        );
        assert_eq!(
            reports[2].slots.get(&AgentId(0)),
            Some(&crate::resolve::SlotStatus::NotSubmitted)
        );

        // Round 4: ENL recaptures the neutral portal.
        assert_eq!(
            reports[3].execution_order[0].outcomes[0],
            crate::resolve::CommandOutcome::Ok(crate::resolve::CommandEffect::Captured {
                portal: p0
            })
        );

        // Round 5: full-HP recharge clamps at max but still spends supply.
        assert_eq!(
            reports[4].execution_order[0].outcomes[0],
            crate::resolve::CommandOutcome::Ok(crate::resolve::CommandEffect::Recharged {
                portal: p0,
                hp_before: 100,
                hp_after: 100
            })
        );

        // Ordered public event log across the whole match.
        let state = harness.state();
        let events = state.events();
        assert_eq!(events.len(), 5);
        let expected: Vec<EventKind> = vec![
            EventKind::PortalCaptured {
                faction: Faction::Enl,
                agent_id: AgentId(0),
                portal: p0,
            },
            EventKind::PortalAttacked {
                faction: Faction::Res,
                agent_id: AgentId(1),
                portal: p0,
                damage: 60,
                hp_after: 40,
            },
            EventKind::PortalAttacked {
                faction: Faction::Res,
                agent_id: AgentId(1),
                portal: p0,
                damage: 60,
                hp_after: 0,
            },
            EventKind::PortalNeutralized {
                faction: Faction::Res,
                agent_id: AgentId(1),
                portal: p0,
            },
            EventKind::PortalCaptured {
                faction: Faction::Enl,
                agent_id: AgentId(0),
                portal: p0,
            },
        ];
        let kinds: Vec<EventKind> = events.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, expected);
        for (index, event) in events.iter().enumerate() {
            assert_eq!(event.seq, index as u64);
        }
        assert_eq!(
            events.iter().map(|e| e.round).collect::<Vec<_>>(),
            vec![1, 2, 3, 3, 4]
        );

        // Final world: ENL owns a full-HP portal; supply was spent twice.
        let portal = state.portal(p0).expect("portal exists");
        assert_eq!((portal.owner, portal.hp), (Owner::Enl, 100));
        assert_eq!(state.agent(AgentId(0)).map(|a| a.supply), Some(18));
        assert_eq!(state.agent(AgentId(1)).map(|a| a.supply), Some(20));
        // Five rounds ran; round counter advanced; trackers are fresh.
        assert_eq!(state.round(), 6);
        assert_eq!(state.tracker(AgentId(0)), Some(&Default::default()));
        assert_eq!(state.tracker(AgentId(1)), Some(&Default::default()));
    }

    #[test]
    fn scripted_strategy_panics_when_exhausted() {
        let mut scripted = ScriptedStrategy::new(vec![vec![]]);
        let obs = Observation::for_faction(&exhaustion_state(), Faction::Enl);
        assert!(scripted.submit_round(&obs).is_empty());
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| scripted.submit_round(&obs)));
        assert!(result.is_err(), "second poll must panic");
    }

    fn exhaustion_state() -> GameState {
        let cfg = gate_config(1);
        let (enl_roster, res_roster) = GameState::initial_roster(&cfg);
        let enl_entries: Vec<(AgentId, Coord)> = enl_roster
            .iter()
            .copied()
            .zip([Coord::new(10_000, 5_000)])
            .collect();
        let res_entries: Vec<(AgentId, Coord)> = res_roster
            .iter()
            .copied()
            .zip([Coord::new(6_000, 10_000)])
            .collect();
        let enl = validate_initial_spawn_set(&enl_roster, &enl_entries, &cfg).expect("spawns");
        let res = validate_initial_spawn_set(&res_roster, &res_entries, &cfg).expect("spawns");
        GameState::new(cfg, &contested_map(), &enl, &res).expect("state")
    }

    // Regression (M2 review): the virtual receive time must be derived with
    // checked math (D9) — a bucket whose receive time would overflow u64
    // must panic with a diagnostic, never silently wrap. Plain `*` wraps in
    // release builds, where a wrapped `received_at_ms` would mis-bucket and
    // reorder streams (the authoritative ordering input of R-RESOLVE-03).
    #[test]
    fn virtual_receive_time_checked_at_the_boundary() {
        // u64::MAX is divisible by 5: the exact fit still succeeds.
        assert_eq!(virtual_receive_time(0, 5), 0);
        assert_eq!(virtual_receive_time(1, 5), 5);
        assert_eq!(virtual_receive_time(u64::MAX / 5, 5), u64::MAX);
    }

    #[test]
    #[should_panic(expected = "must not overflow u64")]
    fn virtual_receive_time_panics_instead_of_wrapping() {
        let _ = virtual_receive_time(u64::MAX / 5 + 1, 5);
    }

    // Same contract end-to-end: a strategy supplying an overflowing bucket
    // makes run_round fail loudly (this is the shape a buggy M6 self-play
    // script would produce). Without the checked op this wraps silently in
    // release builds and panics with the wrong message in debug builds.
    #[test]
    #[should_panic(expected = "must not overflow u64")]
    fn run_round_panics_on_overflowing_strategy_bucket() {
        let cfg = gate_config(1);
        let map = contested_map();
        let enl_spawns = spawns(&cfg, Faction::Enl, &[Coord::new(10_000, 5_000)]);
        let res_spawns = spawns(&cfg, Faction::Res, &[Coord::new(6_000, 10_000)]);
        let enl = ScriptedStrategy::new(vec![vec![AgentSubmission {
            agent_id: AgentId(0),
            // With the default priority_bucket_ms = 5 this overflows u64.
            bucket: u64::MAX / 2 + 1,
            commands: vec![Command::Move {
                destination: Coord::new(10_000, 5_000),
            }],
        }]]);
        let res = ScriptedStrategy::new(vec![vec![], vec![]]);
        let mut harness = MatchHarness::new(
            0,
            cfg,
            &map,
            &enl_spawns,
            &res_spawns,
            Box::new(enl),
            Box::new(res),
        )
        .expect("harness builds");
        let _ = harness.run_round();
    }

    // ------------------------------------------------------------------
    // M3 acceptance gates (checklist 17-18)
    // ------------------------------------------------------------------

    /// The full M3 economy scenario (checklist 17). World: P0 is ENL-owned
    /// hp 40 at (10_000, 10_000); A0 spawns at (10_000, 6_000) — inside
    /// P0's operation range — and A1 at (10_000, 4_900): within
    /// TRANSFER_RANGE of A0 (1_100 cm) but BEYOND P0's operation range
    /// (5_100 cm), so A1's Round-102 recharge must use the key path.
    /// Rounds 97-102 under the default EPOCH_LENGTH 100.
    fn economy_config() -> crate::config::GameConfig {
        gate_config(2)
    }

    fn economy_map() -> MapDefinition {
        MapDefinition::new(vec![
            Portal {
                id: crate::types::PortalId(0),
                position: Coord::new(10_000, 10_000),
                owner: Owner::Enl,
                hp: 40,
            },
            Portal {
                id: crate::types::PortalId(1),
                position: Coord::new(25_000, 25_000),
                owner: Owner::Neutral,
                hp: 0,
            },
        ])
    }

    fn economy_spawns(cfg: &crate::config::GameConfig) -> (InitialSpawns, InitialSpawns) {
        let enl = spawns(
            cfg,
            Faction::Enl,
            &[Coord::new(10_000, 6_000), Coord::new(10_000, 4_900)],
        );
        let res = spawns(
            cfg,
            Faction::Res,
            &[Coord::new(6_000, 10_000), Coord::new(6_000, 11_000)],
        );
        (enl, res)
    }

    fn economy_enl_script() -> ScriptedStrategy {
        let p0 = crate::types::PortalId(0);
        let mut per_round = vec![Vec::<AgentSubmission>::new(); 96]; // rounds 1-96
        // Round 97: first successful hack (ordinal 0).
        per_round.push(vec![AgentSubmission::new(
            AgentId(0),
            vec![Command::Hack { portal: p0 }],
        )]);
        per_round.push(Vec::new()); // 98
        per_round.push(Vec::new()); // 99
        // Round 100: last round of epoch 0; cooldown clean (100 - 97 == 3).
        per_round.push(vec![AgentSubmission::new(
            AgentId(0),
            vec![Command::Hack { portal: p0 }],
        )]);
        // Round 101: first round of epoch 1 — the sweep wiped the cooldown,
        // so this hack is legal — then transfer 2 supply + 1 key to A1.
        let mut keys = std::collections::BTreeMap::new();
        keys.insert(p0, 1u32);
        per_round.push(vec![AgentSubmission::new(
            AgentId(0),
            vec![
                Command::Hack { portal: p0 },
                Command::Transfer {
                    target_agent: AgentId(1),
                    supply: 2,
                    keys,
                },
            ],
        )]);
        // Round 102: A0 still on cooldown (101 -> 102); A1 remote-recharges
        // P0 through the transferred key.
        per_round.push(vec![
            AgentSubmission::new(AgentId(0), vec![Command::Hack { portal: p0 }]),
            AgentSubmission::new(AgentId(1), vec![Command::Recharge { portal: p0 }]),
        ]);
        assert_eq!(per_round.len(), 102);
        ScriptedStrategy::new(per_round)
    }

    fn run_economy_scenario(game_seed: u64) -> (Vec<RoundReport>, GameState) {
        let cfg = economy_config();
        let map = economy_map();
        let (enl_spawns, res_spawns) = economy_spawns(&cfg);
        let mut harness = MatchHarness::new(
            game_seed,
            cfg,
            &map,
            &enl_spawns,
            &res_spawns,
            Box::new(economy_enl_script()),
            Box::new(ScriptedStrategy::new(vec![Vec::new(); 102])),
        )
        .expect("harness builds");
        let reports = harness.run_rounds(102);
        let state = harness.into_state();
        (reports, state)
    }

    /// D3 key yield derived independently from the oracle at a named address.
    fn m3_keys(game_seed: u64, round: u32, ordinal: u32) -> u32 {
        let oracle = crate::rng::RandomOracle::new(game_seed);
        let address = oracle.hack_address(round, AgentId(0), crate::types::PortalId(0), ordinal);
        1 + oracle.uniform(&address, 3)
    }

    // Checklist 17 — the M3 acceptance economy loop.
    #[test]
    fn gate_m3_economy_loop_rounds_ninety_nine_to_one_hundred_two() {
        let game_seed = 0xC0FF_EE;
        let (reports, state) = run_economy_scenario(game_seed);
        assert_eq!(reports.len(), 102);
        assert_eq!(state.round(), 103);

        let p0 = crate::types::PortalId(0);
        let k97 = m3_keys(game_seed, 97, 0);
        let k100 = m3_keys(game_seed, 100, 1);
        let k101 = m3_keys(game_seed, 101, 2);

        // Round 97: hack, ordinal 0.
        let r97 = &reports[96];
        assert_eq!(r97.execution_order.len(), 1);
        assert_eq!(
            r97.execution_order[0].outcomes[0],
            crate::resolve::CommandOutcome::Ok(crate::resolve::CommandEffect::Hacked {
                portal: p0,
                keys_gained: k97,
                supply_granted: 1,
                supply_after: 21,
            })
        );

        // Round 100: still epoch 0, cooldown clean — success.
        let r100 = &reports[99];
        assert_eq!(
            r100.execution_order[0].outcomes[0],
            crate::resolve::CommandOutcome::Ok(crate::resolve::CommandEffect::Hacked {
                portal: p0,
                keys_gained: k100,
                supply_granted: 1,
                supply_after: 22,
            })
        );

        // Round 101: new epoch — the Round-100 hack does NOT block it — then
        // the atomic transfer of 2 supply + 1 key to A1.
        let r101 = &reports[100];
        assert_eq!(r101.execution_order.len(), 1);
        assert_eq!(
            r101.execution_order[0].outcomes[0],
            crate::resolve::CommandOutcome::Ok(crate::resolve::CommandEffect::Hacked {
                portal: p0,
                keys_gained: k101,
                supply_granted: 1,
                supply_after: 23,
            })
        );
        let mut moved = std::collections::BTreeMap::new();
        moved.insert(p0, 1u32);
        assert_eq!(
            r101.execution_order[0].outcomes[1],
            crate::resolve::CommandOutcome::Ok(crate::resolve::CommandEffect::Transferred {
                target: AgentId(1),
                supply: 2,
                keys: moved,
            })
        );

        // Round 102: A0 on cooldown (101 -> 102 leaves 2 rounds); A1 remote-
        // recharges P0 via the key path: hp 40 -> 55, key NOT consumed.
        let r102 = &reports[101];
        assert_eq!(r102.execution_order.len(), 2);
        assert_eq!(
            r102.execution_order[0].outcomes[0],
            crate::resolve::CommandOutcome::Drop(crate::resolve::DropReason::HackCooldownActive {
                portal: p0,
                last_hack_round: 101,
                rounds_remaining: 2,
            })
        );
        assert_eq!(
            r102.execution_order[1].outcomes[0],
            crate::resolve::CommandOutcome::Ok(crate::resolve::CommandEffect::Recharged {
                portal: p0,
                hp_before: 40,
                hp_after: 55,
            })
        );

        // Exact final world.
        assert_eq!(
            state.portal(p0).map(|p| (p.owner, p.hp)),
            Some((Owner::Enl, 55))
        );
        assert_eq!(state.agent(AgentId(0)).map(|a| a.supply), Some(21));
        assert_eq!(state.agent(AgentId(1)).map(|a| a.supply), Some(21));
        assert_eq!(state.agent(AgentId(2)).map(|a| a.supply), Some(20));
        assert_eq!(
            state.agent(AgentId(0)).map(|a| a.keys.get(&p0).copied()),
            Some(Some(k97 + k100 + k101 - 1)),
            "three draws minus the transferred key"
        );
        assert_eq!(
            state.agent(AgentId(1)).map(|a| a.keys.get(&p0).copied()),
            Some(Some(1)),
            "R-RECHARGE-01: remote recharge holds the key, never consumes it"
        );
        assert_eq!(
            state.hack_tracker(AgentId(0), p0),
            Some(&crate::state::HackTracker {
                last_hack_round: Some(101),
                hack_count_epoch: 1,
                hack_ordinal: 3,
            })
        );
        assert!(
            state.events().is_empty(),
            "HACK/TRANSFER/RECHARGE never broadcast; nothing else happened"
        );
        // Empty rounds 98/99 are recorded as auto-PASS slots.
        assert_eq!(
            reports[97].slots.get(&AgentId(0)),
            Some(&crate::resolve::SlotStatus::NotSubmitted)
        );
    }

    // Checklist 18 — the M3 acceptance determinism gate: the same game_seed
    // and fresh scripts reproduce identical reports and identical state.
    #[test]
    fn gate_m3_economy_determinism_same_seed_same_everything() {
        let game_seed = 0xC0FF_EE;
        let (reports_a, state_a) = run_economy_scenario(game_seed);
        let (reports_b, state_b) = run_economy_scenario(game_seed);

        assert_eq!(reports_a, reports_b, "Vec<RoundReport> equal");
        assert_eq!(state_a, state_b, "GameState equal (PartialEq)");
        assert_eq!(
            format!("{:?}", reports_a),
            format!("{:?}", reports_b),
            "report debug encodings equal"
        );
        assert_eq!(
            format!("{:?}", state_a),
            format!("{:?}", state_b),
            "state debug encodings equal"
        );
        // And the gate is not vacuous: 102 rounds ran, five streams executed
        // (r97, r100, r101, and two on r102).
        assert_eq!(reports_a.len(), 102);
        assert_eq!(
            reports_a
                .iter()
                .map(|r| r.execution_order.len())
                .sum::<usize>(),
            5
        );
    }
}
