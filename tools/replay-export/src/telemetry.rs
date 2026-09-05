//! Private HUD telemetry, built from verified resolver results, never inferred
//! from requested destinations. Faction filtering is applied by the caller.
use crate::json::Json;
use arena_core::{
    observe::Observation,
    resolve::{CommandEffect, CommandOutcome, RoundReport, SlotStatus},
    types::{Command, Coord, Faction},
};

fn point(p: Coord) -> Json {
    Json::arr(vec![Json::int(p.x as i64), Json::int(p.y as i64)])
}

pub(crate) fn keys(observation: &Observation) -> Json {
    Json::Object(
        observation
            .own_agents
            .iter()
            .map(|a| {
                (
                    a.id.0.to_string(),
                    Json::Object(
                        a.keys
                            .iter()
                            .map(|(p, n)| (p.0.to_string(), Json::uint(*n as u64)))
                            .collect(),
                    ),
                )
            })
            .collect(),
    )
}

pub(crate) fn commands(
    observation: &Observation,
    faction: Faction,
    round: u32,
    report: &RoundReport,
) -> Json {
    Json::Object(
        observation
            .own_agents
            .iter()
            .map(|agent| {
                let status = report.slots.get(&agent.id);
                let executed = report
                    .execution_order
                    .iter()
                    .enumerate()
                    .find(|(_, s)| s.agent_id == agent.id);
                // Match the intake's FIRST locatable submission, not an ignored duplicate
                // or an envelope with a wrong round / claimed faction.
                // A round-close reinforcement had no intake slot. An earlier
                // unknown-agent envelope naming its future ID was unlocatable.
                let submission = status.and_then(|_| {
                    report.submissions.iter().find(|s| {
                        s.agent_id == agent.id && s.faction == faction && s.round == round
                    })
                });
                let (slot, reason) = match status {
                    Some(SlotStatus::Executed { .. }) => ("executed", String::new()),
                    Some(SlotStatus::Pass) => ("pass", "Explicit pass".into()),
                    Some(SlotStatus::Rejected(reason)) => ("rejected", format!("{reason:?}")),
                    Some(SlotStatus::NotSubmitted) => ("not_submitted", "No submission".into()),
                    None => ("spawned", "Reinforcement at round close".into()),
                };
                let cmds = submission
                    .map(|s| {
                        s.commands
                            .iter()
                            .enumerate()
                            .map(|(i, command)| {
                                let outcome =
                                    executed.and_then(|(_, stream)| stream.outcomes.get(i));
                                command_json(command, outcome)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                (
                    agent.id.0.to_string(),
                    Json::obj(vec![
                        ("slot", Json::str(slot)),
                        ("reason", Json::str(reason)),
                        (
                            "order",
                            executed
                                .map(|(i, _)| Json::uint((i + 1) as u64))
                                .unwrap_or(Json::Null),
                        ),
                        (
                            "bucket",
                            executed
                                .map(|(_, s)| Json::str(s.receive_bucket.to_string()))
                                .unwrap_or(Json::Null),
                        ),
                        (
                            "seq",
                            executed
                                .map(|(_, s)| Json::uint(s.receive_seq as u64))
                                .unwrap_or(Json::Null),
                        ),
                        ("cmds", Json::arr(cmds)),
                    ]),
                )
            })
            .collect(),
    )
}

fn command_json(command: &Command, outcome: Option<&CommandOutcome>) -> Json {
    let (name, params) = match command {
        Command::Move { destination } => (
            "MOVE",
            format!("→ ({}, {}) cm", destination.x, destination.y),
        ),
        Command::Attack => ("ATTACK", "Enemy portals in operation range".into()),
        Command::Capture { portal } => ("CAPTURE", format!("P{}", portal.0)),
        Command::Recharge { portal } => ("RECHARGE", format!("P{}", portal.0)),
        Command::Hack { portal } => ("HACK", format!("P{}", portal.0)),
        Command::Link {
            source_portal,
            target_portal,
        } => (
            "LINK",
            format!("P{} → P{}", source_portal.0, target_portal.0),
        ),
        Command::Transfer {
            target_agent,
            supply,
            keys,
        } => (
            "TRANSFER",
            format!(
                "A{} · Supply {} · Keys {}",
                target_agent.0,
                supply,
                keys.iter()
                    .map(|(p, n)| format!("P{} ×{}", p.0, n))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ),
    };
    let mut pairs = vec![
        ("name", Json::str(name)),
        ("params", Json::str(params)),
        ("major", Json::Bool(command.is_major())),
    ];
    let (status, detail) = match outcome {
        None => ("skipped", "Stream did not execute".into()),
        Some(CommandOutcome::Drop(reason)) => ("drop", format!("{reason:?}")),
        Some(CommandOutcome::Ok(effect)) => {
            let detail = match effect {
                CommandEffect::Moved { from, to, cost } => {
                    pairs.push(("from", point(*from)));
                    pairs.push(("to", point(*to)));
                    pairs.push(("cost_cm", Json::uint(*cost)));
                    format!("Moved · {} cm", cost)
                }
                CommandEffect::Hacked {
                    keys_gained,
                    supply_granted,
                    supply_after,
                    ..
                } => format!("Keys +{keys_gained} · Supply +{supply_granted} → {supply_after}"),
                CommandEffect::Recharged {
                    hp_before,
                    hp_after,
                    ..
                } => format!("HP {hp_before} → {hp_after}"),
                CommandEffect::Captured { .. } => "Portal captured".into(),
                CommandEffect::Attacked { hits } => format!(
                    "{} portals hit · {} neutralized",
                    hits.len(),
                    hits.iter().filter(|h| h.neutralized).count()
                ),
                CommandEffect::Transferred { supply, keys, .. } => format!(
                    "Supply {supply} · {} keys transferred",
                    keys.values().map(|n| *n as u64).sum::<u64>()
                ),
                CommandEffect::Linked { fields, .. } => {
                    format!("Link created · {} fields formed", fields.len())
                }
            };
            ("ok", detail)
        }
    };
    pairs.push(("status", Json::str(status)));
    pairs.push(("detail", Json::str(detail)));
    Json::obj(pairs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena_core::{observe::AgentPrivate, resolve::SubmittedCommandStream, types::AgentId};
    use std::collections::BTreeMap;

    #[test]
    fn reinforcement_does_not_inherit_an_unlocatable_submission() {
        let agent_id = AgentId(6);
        let observation = Observation {
            round: 101,
            portals: vec![],
            links: vec![],
            fields: vec![],
            events: vec![],
            scores: BTreeMap::new(),
            own_agents: vec![AgentPrivate {
                id: agent_id,
                position: Coord::new(0, 0),
                supply: 20,
                keys: BTreeMap::new(),
            }],
        };
        let report = RoundReport {
            submissions: vec![SubmittedCommandStream {
                round: 100,
                faction: Faction::Enl,
                agent_id,
                received_at_ms: 0,
                receive_seq: 1,
                commands: vec![Command::Attack],
            }],
            ..RoundReport::default()
        };
        let telemetry = commands(&observation, Faction::Enl, 100, &report);
        let stream = telemetry.get("6").unwrap();
        assert_eq!(stream.get("slot").and_then(Json::as_str), Some("spawned"));
        assert!(stream.get("cmds").unwrap().as_array().unwrap().is_empty());
    }
}
