//! Rebuildable timing evidence: server receipt times, client-declared wall
//! times and host conditions remain separate, including missing diagnostics.
use super::{archive::Json, document::*, durable};
use std::{fs, io, path::Path};
pub fn execute_cli(args: &[String]) -> io::Result<Option<String>> {
    if args.first().map(String::as_str) != Some("timing-report") {
        return Ok(None);
    }
    if args.len() != 3 || args[1] != "--out" {
        return Err(invalid("timing-report --out PLAN_DIR"));
    }
    let root = Path::new(&args[2]);
    let run = durable::execute(root, durable::Action::Rebuild)?;
    let mut matches = Vec::new();
    for (i, item) in run.items.iter().enumerate() {
        let mut receipts = vec![];
        let mut clients = vec![];
        let mut conditions = Json::Null;
        let mut logs = vec![];
        let replay = run.replay_rel[i]
            .as_ref()
            .map(|r| format!("matches/m{}/{r}", item.index));
        if let Some(relative) = &replay {
            let replay_path = root.join(relative);
            let log = arena_replay::format::decode(&fs::read(&replay_path)?).map_err(invalid)?;
            for round in log.round_records() {
                for s in &round.submissions {
                    receipts.push(object(vec![
                        ("round", Json::Int(round.round as i64)),
                        ("agent", Json::Int(s.agent_id.0 as i64)),
                        (
                            "faction",
                            string(if s.faction == arena_core::types::Faction::Enl {
                                "enl"
                            } else {
                                "res"
                            }),
                        ),
                        ("received_at_ms", Json::Int(s.received_at_ms as i64)),
                        (
                            "bucket",
                            Json::Int(
                                (s.received_at_ms / log.header.config.priority_bucket_ms as u64)
                                    as i64,
                            ),
                        ),
                        ("receive_seq", Json::Int(s.receive_seq as i64)),
                    ]));
                }
            }
        }
        // Rebuild has verified committed attempts, including technical endings
        // without IARP. Keep their logs and conditions as first-class evidence.
        let attempts = durable::attempt_dirs(root, item.index)?;
        let attempt = attempts.last();
        if let Some(attempt) = attempt {
            let resources = attempt.join("resources.json");
            if resources.exists() {
                conditions = read(&resources)?;
            }
            for label in ["a", "b"] {
                let side = if !run.connect_order_verified[i] {
                    "unverified"
                } else if (label == "a") == item.a_is_enl {
                    "enl"
                } else {
                    "res"
                };
                let direct = attempt.join(format!("client_{label}.stderr"));
                let sandbox = attempt.join(format!("sandbox_{label}.json"));
                let (source, content, invalid_utf8) = if direct.exists() {
                    let bytes = fs::read(&direct)?;
                    let invalid_utf8 = std::str::from_utf8(&bytes).is_err();
                    (
                        direct.clone(),
                        String::from_utf8_lossy(&bytes).into_owned(),
                        invalid_utf8,
                    )
                } else if sandbox.exists() {
                    let r = read(&sandbox)?;
                    (
                        sandbox.clone(),
                        r.get("stderr_tail")
                            .and_then(Json::as_str)
                            .unwrap_or("")
                            .to_string(),
                        false,
                    )
                } else {
                    continue;
                };
                logs.push(object(vec![
                    (
                        "path",
                        string(
                            source
                                .strip_prefix(root)
                                .unwrap_or(&source)
                                .to_string_lossy(),
                        ),
                    ),
                    ("invalid_utf8", Json::Bool(invalid_utf8)),
                ]));
                for line in content.lines() {
                    if let Some(payload) = line.strip_prefix("ARENA_TIMING ") {
                        match super::archive::parse_json(payload) {
                            Ok(t)
                                if t.get("schema").and_then(Json::as_str)
                                    == Some("arena-client-timing-v1") =>
                            {
                                clients.push(object(vec![
                                    ("faction", string(side)),
                                    ("client_declared", Json::Bool(true)),
                                    ("timing", t),
                                ]))
                            }
                            _ => clients.push(object(vec![
                                ("faction", string(side)),
                                ("malformed_or_truncated", Json::Bool(true)),
                            ])),
                        }
                    }
                }
            }
        }
        matches.push(object(vec![
            ("item", Json::Int(item.index as i64)),
            ("outcome", string(run.outcomes[i].outcome_tag())),
            (
                "stage",
                run.outcomes[i]
                    .stage()
                    .map(|s| string(s.tag()))
                    .unwrap_or(Json::Null),
            ),
            (
                "reason",
                run.outcomes[i].reason().map(string).unwrap_or(Json::Null),
            ),
            (
                "attempt",
                attempt
                    .map(|p| string(p.strip_prefix(root).unwrap_or(p).to_string_lossy()))
                    .unwrap_or(Json::Null),
            ),
            ("replay", replay.map(string).unwrap_or(Json::Null)),
            ("conditions", conditions),
            ("logs", Json::Array(logs)),
            ("client_timings", Json::Array(clients)),
            ("server_receipts", Json::Array(receipts)),
        ]));
    }
    let report = object(vec![
        ("schema", string("arena-timing-report-v1")),
        (
            "interpretation",
            string(
                "client elapsed wall time is not CPU time; server arrival includes scheduling and transport; missing/truncated client diagnostics are not zero; fixed buckets are not timing measurements",
            ),
        ),
        ("matches", Json::Array(matches)),
    ]);
    let output = encode(&report);
    durable::atomic_write(&root.join("timing.json"), output.as_bytes())?;
    Ok(Some(output))
}
