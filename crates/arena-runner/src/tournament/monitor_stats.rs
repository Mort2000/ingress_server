//! Compact telemetry from the same verified replay used by the archive monitor.
use super::{
    archive::Json,
    document::{object, string},
};
use arena_core::{event::EventKind, resolve::SlotStatus, types::Faction};
use arena_replay::{format::ReplayFile, replay::ReplayOutcome};

#[derive(Clone, Debug, Default)]
pub struct PlayerStatistics {
    pub score2: u128,
    pub portals_captured: u64,
    pub portals_destroyed: u64,
    pub links_created: u64,
    pub links_destroyed: u64,
    pub fields_created: u64,
    pub fields_destroyed: u64,
    /// Every active agent contributes one slot per completed round.
    pub agent_rounds: u64,
    pub pass_slots: u64,
    pub not_submitted_slots: u64,
    pub rejected_slots: u64,
    /// All active agents explicitly passed or did not submit. Rejections and
    /// executed streams (even ones with dropped commands) are not passes.
    pub idle_rounds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena_core::{
        config::GameConfig,
        resolve::{Resolver, SubmittedCommandStream},
        spawn::validate_initial_spawn_set,
        state::GameState,
        types::{AgentId, Command, Coord, FieldId, LinkId, PortalId},
    };
    use arena_replay::{
        recorder::ReplayWriter,
        replay::{decode_log, replay},
    };

    #[test]
    fn verified_slots_distinguish_pass_missing_rejection_and_failed_execution_with_reinforcements()
    {
        let config = GameConfig {
            total_rounds: 3,
            initial_agents_per_faction: 1,
            reinforcement_interval_rounds: 2,
            reinforcement_agents_per_faction: 1,
            ..GameConfig::default()
        };
        let map = arena_core::map::generate_map(42, &config).unwrap();
        let (enl, res) = GameState::initial_roster(&config);
        let spawn = |roster: &[AgentId]| {
            validate_initial_spawn_set(roster, &[(roster[0], Coord::new(1000, 1000))], &config)
                .unwrap()
        };
        let es = spawn(&enl);
        let rs = spawn(&res);
        let mut state = GameState::new(config, &map, &es, &rs).unwrap();
        let mut writer = ReplayWriter::new(42, &config, &map, &es, &rs);
        let resolver = Resolver::with_seed(42);
        let submit = |round, faction, agent_id, commands| SubmittedCommandStream {
            round,
            faction,
            agent_id,
            received_at_ms: 0,
            receive_seq: 0,
            commands,
        };
        for round in 1..=3 {
            let inputs = match round {
                1 => vec![submit(round, Faction::Enl, enl[0], vec![])],
                2 => vec![
                    submit(
                        round,
                        Faction::Enl,
                        enl[0],
                        vec![Command::Attack, Command::Attack],
                    ),
                    submit(round, Faction::Res, res[0], vec![Command::Attack]),
                ],
                _ => vec![],
            };
            let report = resolver.run_round(&mut state, &inputs);
            state.advance_round();
            writer.record_round(&report, &state).unwrap();
        }
        let log = decode_log(&writer.finish(&state).unwrap()).unwrap();
        let verified = replay(&log).unwrap();
        let stats = MatchStatistics::from_verified(&log, &verified);
        assert_eq!(stats.rounds, 3);
        assert_eq!(stats.enl.agent_rounds, 4);
        assert_eq!(stats.res.agent_rounds, 4);
        assert_eq!(stats.enl.pass_slots, 1);
        assert_eq!(stats.enl.not_submitted_slots, 2);
        assert_eq!(stats.enl.rejected_slots, 1);
        assert_eq!(stats.res.not_submitted_slots, 3);
        assert_eq!(stats.res.rejected_slots, 0);
        assert_eq!(stats.enl.idle_rounds, 2);
        assert_eq!(
            stats.res.idle_rounds, 2,
            "failed ATTACK must not be counted as PASS"
        );
    }

    #[test]
    fn structures_count_events_and_credit_cascade_to_attacker() {
        let mut stats = MatchStatistics::default();
        let portal = PortalId(0);
        let link = LinkId::new(portal, PortalId(1)).unwrap();
        let field = FieldId::new(portal, PortalId(1), PortalId(2)).unwrap();
        for _ in 0..2 {
            stats.event(EventKind::PortalCaptured {
                faction: Faction::Enl,
                agent_id: AgentId(0),
                portal,
            });
            stats.event(EventKind::LinkCreated {
                faction: Faction::Enl,
                agent_id: AgentId(0),
                link,
            });
            stats.event(EventKind::FieldCreated {
                faction: Faction::Enl,
                agent_id: AgentId(0),
                field,
            });
        }
        stats.event(EventKind::PortalNeutralized {
            faction: Faction::Res,
            agent_id: AgentId(1),
            portal,
        });
        stats.event(EventKind::LinkDestroyed {
            faction: Faction::Res,
            agent_id: AgentId(1),
            link,
        });
        stats.event(EventKind::FieldDestroyed {
            faction: Faction::Res,
            agent_id: AgentId(1),
            field,
        });
        assert_eq!(stats.enl.portals_captured, 2);
        assert_eq!(stats.enl.links_created, 2);
        assert_eq!(stats.enl.fields_created, 2);
        assert_eq!(stats.enl.portals_destroyed, 0);
        assert_eq!(stats.res.portals_destroyed, 1);
        assert_eq!(stats.res.links_destroyed, 1);
        assert_eq!(stats.res.fields_destroyed, 1);
    }
}
impl PlayerStatistics {
    fn json(&self) -> Json {
        let mut fields = vec![("score2", string(self.score2))];
        for (key, value) in [
            ("portals_captured", self.portals_captured),
            ("portals_destroyed", self.portals_destroyed),
            ("links_created", self.links_created),
            ("links_destroyed", self.links_destroyed),
            ("fields_created", self.fields_created),
            ("fields_destroyed", self.fields_destroyed),
            ("agent_rounds", self.agent_rounds),
            ("pass_slots", self.pass_slots),
            ("not_submitted_slots", self.not_submitted_slots),
            ("rejected_slots", self.rejected_slots),
            ("idle_rounds", self.idle_rounds),
        ] {
            fields.push((key, Json::Int(value as i64)));
        }
        object(fields)
    }
}

#[derive(Clone, Debug, Default)]
pub struct MatchStatistics {
    pub rounds: u32,
    pub enl: PlayerStatistics,
    pub res: PlayerStatistics,
}
impl MatchStatistics {
    fn player(&mut self, faction: Faction) -> &mut PlayerStatistics {
        match faction {
            Faction::Enl => &mut self.enl,
            Faction::Res => &mut self.res,
        }
    }
    pub(crate) fn from_verified(log: &ReplayFile, verified: &ReplayOutcome) -> Self {
        let mut stats = Self {
            rounds: verified.rounds_completed,
            ..Self::default()
        };
        stats.enl.score2 = verified.final_scores.0;
        stats.res.score2 = verified.final_scores.1;
        for record in log.round_records() {
            for faction in [Faction::Enl, Faction::Res] {
                // Final authoritative agents include reinforcements; the round's
                // slots ensure agents only count after they actually spawned.
                let player = stats.player(faction);
                let mut active = 0;
                let mut idle = 0;
                for (id, slot) in &record.slots {
                    if verified.state.agents()[id].faction != faction {
                        continue;
                    }
                    active += 1;
                    player.agent_rounds += 1;
                    match slot {
                        SlotStatus::Pass => {
                            player.pass_slots += 1;
                            idle += 1;
                        }
                        SlotStatus::NotSubmitted => {
                            player.not_submitted_slots += 1;
                            idle += 1;
                        }
                        SlotStatus::Rejected(_) => player.rejected_slots += 1,
                        SlotStatus::Executed { .. } => {}
                    }
                }
                if active > 0 && idle == active {
                    player.idle_rounds += 1;
                }
            }
            for event in &record.events {
                stats.event(event.kind);
            }
        }
        stats
    }
    fn event(&mut self, event: EventKind) {
        // Destruction events name the attacking actor, not the former owner.
        match event {
            EventKind::PortalCaptured { faction, .. } => self.player(faction).portals_captured += 1,
            EventKind::PortalNeutralized { faction, .. } => {
                self.player(faction).portals_destroyed += 1
            }
            EventKind::LinkCreated { faction, .. } => self.player(faction).links_created += 1,
            EventKind::LinkDestroyed { faction, .. } => self.player(faction).links_destroyed += 1,
            EventKind::FieldCreated { faction, .. } => self.player(faction).fields_created += 1,
            EventKind::FieldDestroyed { faction, .. } => self.player(faction).fields_destroyed += 1,
            _ => {}
        }
    }
    pub(crate) fn json(&self) -> Json {
        object(vec![
            ("rounds", Json::Int(self.rounds as i64)),
            ("enl", self.enl.json()),
            ("res", self.res.json()),
        ])
    }
}
