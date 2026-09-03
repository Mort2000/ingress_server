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
        // M4 update: the round-100 boundary (interval 100 < total 500) now
        // spawns one reinforcement agent per faction and broadcasts
        // REINFORCEMENT — the ONLY event of this scenario
        // (HACK/TRANSFER/RECHARGE still never broadcast).
        assert_eq!(state.events().len(), 1, "exactly the boundary event");
        assert_eq!(state.events()[0].round, 100);
        assert_eq!(
            state.events()[0].kind,
            EventKind::Reinforcement {
                index: 1,
                position: crate::rng::RandomOracle::new(game_seed)
                    .reinforcement_position(1, 30_001, 30_001),
                count_per_faction: 1,
                first_agent_id: AgentId(4),
            }
        );
        // The roster grew 2 -> 3 per faction; the newcomers idle (no script).
        assert_eq!(
            state.agent(AgentId(4)).map(|a| a.faction),
            Some(Faction::Enl)
        );
        assert_eq!(
            state.agent(AgentId(5)).map(|a| a.faction),
            Some(Faction::Res)
        );
        // Empty rounds 98/99 are recorded as auto-PASS slots.
        assert_eq!(
            reports[97].slots.get(&AgentId(0)),
            Some(&crate::resolve::SlotStatus::NotSubmitted)
        );
        // The boundary round itself saw only the two initial agents.
        assert_eq!(reports[99].slots.len(), 4);
        assert!(!reports[99].slots.contains_key(&AgentId(4)));
        // ...and round 101 records the reinforcement agent's idle slot.
        assert_eq!(
            reports[100].slots.get(&AgentId(4)),
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

    // ------------------------------------------------------------------
    // M4 acceptance (checklist 42-47): the 500-round Field Game match.
    // ------------------------------------------------------------------

    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use crate::geometry::{area2, dist2, point_on_segment, segments_intersect};
    use crate::map::generate_map;
    use crate::rng::RandomOracle;
    use crate::types::{LinkId, MatchResult, PortalId};

    /// Pinned deterministic inputs: the map seed is the one whose generated
    /// default-config map holds two disjoint, non-crossing portal triangles
    /// (verified by the search below); the game seed feeds the oracle.
    const M4_MAP_SEED: u64 = 0;
    const M4_GAME_SEED: u64 = 0xBEEF_0004;

    /// Deterministic search over the pinned map for two triangles the bots
    /// can complete without geometric interference. A candidate triple has
    /// pairwise distance <= `MOVE_BUDGET` (a single hop: ceil_sqrt(dist2)
    /// <= 20_000 exactly when dist2 <= 20_000^2), non-zero area2, and no
    /// other portal on any of its three closed segments; the returned pair
    /// is additionally portal-disjoint with no edge crossings between the
    /// two triangles.
    fn find_triangle_pair(map: &MapDefinition) -> ([u32; 3], [u32; 3]) {
        find_triangle_pair_within(map, usize::MAX)
    }

    /// The bounded form of [`find_triangle_pair`]: only the first
    /// `scan_portals` portals participate in the triple enumeration (the
    /// 300-portal A-CORE stress map would make the full O(n^3) search
    /// minutes-slow for no extra coverage; two clear triangles exist among
    /// the densely placed leading portals).
    fn find_triangle_pair_within(map: &MapDefinition, scan_portals: usize) -> ([u32; 3], [u32; 3]) {
        const HOP2: i64 = 20_000 * 20_000;
        let portals: Vec<(PortalId, Coord)> =
            map.portals().iter().map(|p| (p.id, p.position)).collect();
        let n = portals.len().min(scan_portals);
        assert!(
            portals.len() >= 6,
            "the map must hold two disjoint triangles"
        );
        assert!(
            n >= 6,
            "the scanned prefix must hold two disjoint triangles"
        );
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
        assert!(
            !candidates.is_empty(),
            "the scanned portals hold at least one clear triangle"
        );

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
                    return (*a, *b);
                }
            }
        }
        panic!("the pinned map has no two disjoint non-crossing triangles");
    }

    /// Test-local deterministic strategy (M6 builds the real baselines): a
    /// pure function of the `Observation` plus a fixed per-agent triangle
    /// plan, at most one command per agent per round. Tour: p0 (capture +
    /// hack) -> p1 -> p2 -> LINK(p0,p1) -> LINK(p1,p2) -> LINK(p2,p0), so
    /// each faction holds one persistent field by ~round 15. Reinforcement
    /// agents are not planned and idle (no submission). Records the own
    /// agent count per round for the reinforcement gate.
    struct FieldBot {
        owner: Owner,
        plan: [PortalId; 3],
        agents: [AgentId; 3],
        counts: Arc<Mutex<Vec<(u32, usize)>>>,
    }

    impl FieldBot {
        fn new(
            faction: Faction,
            plan: [u32; 3],
            roster_offset: u32,
            counts: Arc<Mutex<Vec<(u32, usize)>>>,
        ) -> Self {
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
                counts,
            }
        }
    }

    impl RoundStrategy for FieldBot {
        fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission> {
            self.counts
                .lock()
                .expect("uncontended")
                .push((obs.round, obs.own_agents.len()));

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
                // Six tour stations: three capture+hack stops, then the
                // three triangle edges in tour order (source, target).
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

    /// Match fixture: default config (500 rounds, 120 portals, 600_000 cm
    /// map, 3 agents per faction, reinforcement every 100 rounds), the
    /// pinned generated map, and both factions' three initial agents spawned
    /// exactly ON their triangle portals (spawns are free coordinates).
    fn field_match_fixture() -> (GameConfig, MapDefinition, InitialSpawns, InitialSpawns) {
        let cfg = GameConfig::default();
        let map = generate_map(M4_MAP_SEED, &cfg).expect("the pinned map generates");
        let (enl_tri, res_tri) = find_triangle_pair(&map);
        let tri_coords = |tri: &[u32; 3]| -> Vec<Coord> {
            tri.iter()
                .map(|&p| map.portal(PortalId(p)).expect("dense portal").position)
                .collect()
        };
        let enl_spawns = spawns(&cfg, Faction::Enl, &tri_coords(&enl_tri));
        let res_spawns = spawns(&cfg, Faction::Res, &tri_coords(&res_tri));
        (cfg, map, enl_spawns, res_spawns)
    }

    fn run_field_match(
        game_seed: u64,
    ) -> (
        Vec<RoundReport>,
        GameState,
        Arc<Mutex<Vec<(u32, usize)>>>,
        Arc<Mutex<Vec<(u32, usize)>>>,
    ) {
        let (cfg, map, enl_spawns, res_spawns) = field_match_fixture();
        let enl_counts = Arc::new(Mutex::new(Vec::new()));
        let res_counts = Arc::new(Mutex::new(Vec::new()));
        let enl = FieldBot::new(
            Faction::Enl,
            find_triangle_pair(&map).0,
            0,
            Arc::clone(&enl_counts),
        );
        let res = FieldBot::new(
            Faction::Res,
            find_triangle_pair(&map).1,
            3,
            Arc::clone(&res_counts),
        );
        let mut harness = MatchHarness::new(
            game_seed,
            cfg,
            &map,
            &enl_spawns,
            &res_spawns,
            Box::new(enl),
            Box::new(res),
        )
        .expect("harness builds");
        let reports = harness.run_rounds(500);
        let state = harness.into_state();
        (reports, state, enl_counts, res_counts)
    }

    // Checklist 42: the full default match runs to completion.
    #[test]
    fn gate_m4_500_round_match_completes() {
        let (reports, state, _, _) = run_field_match(M4_GAME_SEED);
        assert_eq!(reports.len(), 500);
        assert_eq!(state.round(), 501, "500 rounds ran; the counter advanced");
        // The bots really played the field game.
        assert!(
            state.link_graph().links().len() >= 6,
            "both triangles built"
        );
        assert!(
            state.link_graph().fields().len() >= 2,
            "both factions hold a field"
        );
    }

    // Checklist 43: both factions scored and the result is the plain score
    // comparison (R-SCORE-01).
    #[test]
    fn gate_m4_field_scores_positive_and_result_consistent() {
        let (reports, state, _, _) = run_field_match(M4_GAME_SEED);
        let (enl, res) = (state.score(Faction::Enl), state.score(Faction::Res));
        // The tour closes one persistent field per faction by ~round 15
        // (observed: round 14 for both) — the first non-zero delta lands
        // well before the match's midpoint.
        for faction in [Faction::Enl, Faction::Res] {
            let first = reports
                .iter()
                .position(|r| r.score2_delta[&faction] > 0)
                .expect("a delta landed");
            assert!(
                first + 1 <= 20,
                "{faction:?} first scored at round {}",
                first + 1
            );
        }
        assert!(enl > 0, "ENL scored a persistent field");
        assert!(res > 0, "RES scored a persistent field");

        let expected = if enl > res {
            MatchResult::EnlWin
        } else if res > enl {
            MatchResult::ResWin
        } else {
            MatchResult::Draw
        };
        assert_eq!(state.final_result(), expected);
        assert!(matches!(
            state.final_result(),
            MatchResult::EnlWin | MatchResult::ResWin | MatchResult::Draw
        ));
        // R-SCORE-01 telescoping: the per-round deltas sum to the totals.
        let mut sum_enl = 0u128;
        let mut sum_res = 0u128;
        for report in &reports {
            sum_enl += report.score2_delta[&Faction::Enl];
            sum_res += report.score2_delta[&Faction::Res];
        }
        assert_eq!(sum_enl, enl);
        assert_eq!(sum_res, res);
    }

    // Checklist 44: reinforcements land exactly on the default schedule
    // (rounds 100/200/300/400, one per faction at one oracle position) and
    // are first visible in the following rounds' observations.
    #[test]
    fn gate_m4_reinforcements_at_expected_boundaries() {
        let (_reports, state, enl_counts, res_counts) = run_field_match(M4_GAME_SEED);
        let reinforcements: Vec<&crate::event::Event> = state
            .events()
            .iter()
            .filter(|e| matches!(e.kind, EventKind::Reinforcement { .. }))
            .collect();
        assert_eq!(
            reinforcements.iter().map(|e| e.round).collect::<Vec<_>>(),
            vec![100, 200, 300, 400],
            "exactly the four default boundaries"
        );

        let oracle = RandomOracle::new(M4_GAME_SEED);
        for (i, event) in reinforcements.iter().enumerate() {
            match event.kind {
                EventKind::Reinforcement {
                    index,
                    position,
                    count_per_faction,
                    first_agent_id,
                } => {
                    assert_eq!(index as usize, i + 1, "interval index 1..=4");
                    assert_eq!(count_per_faction, 1);
                    let expected = oracle.reinforcement_position(index, 600_001, 600_001);
                    assert_eq!(position, expected, "oracle-derived shared position");
                    assert_eq!(
                        state.agent(first_agent_id).map(|a| (a.faction, a.position)),
                        Some((Faction::Enl, expected)),
                        "ENL block first"
                    );
                    assert_eq!(
                        state
                            .agent(AgentId(first_agent_id.0 + 1))
                            .map(|a| (a.faction, a.position)),
                        Some((Faction::Res, expected)),
                        "RES block shares the position"
                    );
                }
                other => panic!("unexpected kind {other:?}"),
            }
        }

        // Observations at rounds 101/201/301/401 show 4/5/6/7 own agents.
        for (counts, faction) in [(enl_counts, "ENL"), (res_counts, "RES")] {
            let counts = counts.lock().expect("uncontended");
            for (round, expected) in [(101u32, 4usize), (201, 5), (301, 6), (401, 7)] {
                let seen = counts
                    .iter()
                    .find(|(r, _)| *r == round)
                    .expect("every round was observed")
                    .1;
                assert_eq!(seen, expected, "{faction} own agents in round {round}");
            }
        }
    }

    // Checklist 45: the whole match is deterministic — same game_seed, same
    // map, fresh bots, identical reports and identical final state (the M3
    // gate extended to the full 500-round field match).
    #[test]
    fn gate_m4_full_match_determinism() {
        let (reports_a, state_a, _, _) = run_field_match(M4_GAME_SEED);
        let (reports_b, state_b, _, _) = run_field_match(M4_GAME_SEED);
        assert_eq!(reports_a, reports_b, "Vec<RoundReport> equal");
        assert_eq!(state_a, state_b, "GameState equal (PartialEq)");
        // The gate is not vacuous.
        assert_eq!(state_a.round(), 501);
        assert!(state_a.score(Faction::Enl) > 0);
        assert!(state_a.score(Faction::Res) > 0);
        assert!(state_a.link_graph().fields().len() >= 2);
    }

    // Checklist 46: runtime bound — catches accidental per-round complexity
    // blowups (A-CORE). `Instant` is test scaffolding only; no float enters
    // an authoritative path.
    #[test]
    fn gate_m4_runtime_under_60_seconds() {
        let start = Instant::now();
        let (_reports, state, _, _) = run_field_match(M4_GAME_SEED);
        let elapsed = start.elapsed();
        assert_eq!(state.round(), 501);
        assert!(
            elapsed < Duration::from_secs(60),
            "the full match took {elapsed:?}"
        );
    }

    // Checklist 47 (P1-FIELD): the incrementally maintained field cache
    // equals the from-scratch recompute over the real 500-round final state.
    #[test]
    fn gate_m4_field_cache_equals_full_recompute_at_match_end() {
        let (_reports, state, _, _) = run_field_match(M4_GAME_SEED);
        assert_eq!(
            *state.link_graph().fields(),
            state.fields_full_recompute(),
            "cache == full recompute on real match data"
        );
    }

    // ------------------------------------------------------------------
    // A-CORE stress (M-final): the MVP acceptance row demands "处理至少
    // 200 Portal" — a 300-portal map from the FROZEN generator running a
    // full default-length (500-round) match in-process with the same
    // scripted FieldBot used by the M4 gates. Measured 20 ms debug on the
    // 12-core dev box, map generation and triangle search included (the
    // 120-portal default match runs ~0.08 s; the assert stays a generous
    // 60 s so slow CI cannot flake — it exists to catch accidental
    // complexity blowups, not to pin hardware).
    // ------------------------------------------------------------------

    /// Map seed whose generated 300-portal stress map exists (verified by
    /// the test itself; the generator retries whole maps on infeasibility,
    /// so the first seed working is the common case).
    const STRESS_MAP_SEED: u64 = 0x0055_0000_0300;

    fn stress_config() -> GameConfig {
        let mut cfg = GameConfig::default(); // 500 rounds: the default length
        cfg.map_width_cm = 1_000_000;
        cfg.map_height_cm = 1_000_000;
        cfg.portal_count = 300;
        // 90 clustered + 210 uniform: the default 70% ratio wants 210
        // clustered portals, more than six ~48 km cluster regions can hold
        // at the default 8 km minimum spacing — the generator would spend
        // its whole restart budget and go Infeasible.
        cfg.clustered_portal_ratio_permille = 300;
        cfg
    }

    #[test]
    fn gate_final_300_portal_500_round_match_completes() {
        let start = Instant::now();
        let cfg = stress_config();
        let map = generate_map(STRESS_MAP_SEED, &cfg).expect("the 300-portal stress map");
        assert_eq!(map.len(), 300, "the frozen generator built 300 portals");
        assert_eq!(map.validate(&cfg), Ok(()));

        // Two clear triangles among the leading (clustered, dense) portals;
        // the bounded scan keeps the O(n^3) search out of minutes territory.
        let (enl_tri, res_tri) = find_triangle_pair_within(&map, 64);
        let tri_coords = |tri: &[u32; 3]| -> Vec<Coord> {
            tri.iter()
                .map(|&p| map.portal(PortalId(p)).expect("dense portal").position)
                .collect()
        };
        let enl_spawns = spawns(&cfg, Faction::Enl, &tri_coords(&enl_tri));
        let res_spawns = spawns(&cfg, Faction::Res, &tri_coords(&res_tri));
        let mut harness = MatchHarness::new(
            M4_GAME_SEED,
            cfg,
            &map,
            &enl_spawns,
            &res_spawns,
            Box::new(FieldBot::new(
                Faction::Enl,
                enl_tri,
                0,
                Arc::new(Mutex::new(Vec::new())),
            )),
            Box::new(FieldBot::new(
                Faction::Res,
                res_tri,
                3,
                Arc::new(Mutex::new(Vec::new())),
            )),
        )
        .expect("stress harness builds");
        let reports = harness.run_rounds(500);
        let state = harness.into_state();

        // Completion: every round settled exactly once, the counter
        // advanced past the last round, and the map really carried 300
        // portals through the whole match.
        assert_eq!(reports.len(), 500);
        assert_eq!(state.round(), 501);
        assert_eq!(state.portals().len(), 300);

        // Correct final state/result: the field game actually played, the
        // result is the plain score comparison, and the per-round deltas
        // telescope to the final scores (R-SCORE-01).
        assert!(
            state.link_graph().links().len() >= 6,
            "both triangles built"
        );
        assert!(
            state.link_graph().fields().len() >= 2,
            "both factions hold a field"
        );
        let (enl, res) = (state.score(Faction::Enl), state.score(Faction::Res));
        assert!(enl > 0, "ENL scored a persistent field");
        assert!(res > 0, "RES scored a persistent field");
        let expected = if enl > res {
            MatchResult::EnlWin
        } else if res > enl {
            MatchResult::ResWin
        } else {
            MatchResult::Draw
        };
        assert_eq!(state.final_result(), expected);
        let mut sum_enl = 0u128;
        let mut sum_res = 0u128;
        for report in &reports {
            sum_enl += report.score2_delta[&Faction::Enl];
            sum_res += report.score2_delta[&Faction::Res];
        }
        assert_eq!(sum_enl, enl);
        assert_eq!(sum_res, res);

        // Reinforcements landed on the default schedule as the roster grew.
        let reinforcements = state
            .events()
            .iter()
            .filter(|e| matches!(e.kind, EventKind::Reinforcement { .. }))
            .count();
        assert_eq!(reinforcements, 4, "rounds 100/200/300/400");

        // Runtime bound (A-CORE): generous, hardware-independent.
        let elapsed = start.elapsed();
        eprintln!("300-portal 500-round stress match: {elapsed:?} (debug)");
        assert!(
            elapsed < Duration::from_secs(60),
            "the 300-portal match took {elapsed:?}"
        );
    }
}
