//! Offline diagnostics only: no runtime connection, no hidden wall clock.
use crate::strategy::{OfflineRound, Strategy, Trace};
use arena_core::{
    observe::Observation,
    resolve::{Resolver, RoundReport, SubmittedCommandStream},
    state::GameState,
    types::Faction,
};
use arena_replay::format::ReplayFile;
use std::{collections::BTreeMap, io};

pub struct HistoryRound {
    pub observation: Observation,
    pub submissions: Vec<arena_core::harness::AgentSubmission>,
    pub recorded: Vec<SubmittedCommandStream>,
    /// Official execution results for recorded inputs, NOT the new decisions.
    pub execution: RoundReport,
    pub budgets: Vec<u64>,
    pub traces: Vec<Trace>,
}
pub struct History {
    pub source_hash: String,
    pub budget_input_hash: String,
    pub faction: Faction,
    pub first_difference: Option<u32>,
    pub rounds: Vec<HistoryRound>,
}

/// Validates the whole replay, then restores legal faction observations from
/// opening. The caller supplies a freshly initialized strategy. No mid-game
/// snapshot is inferred. New output NEVER changes the recorded world.
/// Feedback-dependent strategies need their original transport callbacks;
/// IARP alone does not contain those, so this API does not synthesize them.
pub fn reconstruct(
    log: &ReplayFile,
    faction: Faction,
    budgets: &Budgets,
    strategy: &mut impl Strategy,
) -> io::Result<History> {
    arena_replay::replay::replay(log).map_err(invalid)?;
    let h = &log.header;
    let (a, b) = GameState::initial_roster(&h.config);
    let a = arena_core::spawn::validate_initial_spawn_set(&a, &h.enl_spawns, &h.config)
        .map_err(|e| invalid(format!("{e:?}")))?;
    let b = arena_core::spawn::validate_initial_spawn_set(&b, &h.res_spawns, &h.config)
        .map_err(|e| invalid(format!("{e:?}")))?;
    let mut state =
        GameState::new(h.config, &h.map, &a, &b).map_err(|e| invalid(format!("{e:?}")))?;
    let resolver = Resolver::with_seed(h.game_seed);
    let mut result = History {
        source_hash: blake3::hash(&log.encode()).to_hex().to_string(),
        budget_input_hash: budgets.content_hash(),
        faction,
        first_difference: None,
        rounds: vec![],
    };
    for record in log.round_records() {
        let observation = Observation::for_faction(&state, faction);
        let mut turn = OfflineRound::new(&observation, budgets.for_round(record.round)?, 0);
        strategy.decide(&observation, &mut turn).map_err(invalid)?;
        strategy.on_diagnostics(record.round, turn.budget_reads(), turn.traces());
        let recorded: Vec<_> = record
            .submissions
            .iter()
            .filter(|s| s.faction == faction)
            .cloned()
            .collect();
        let actual: Vec<_> = turn
            .submissions()
            .iter()
            .map(|s| (s.agent_id, &s.commands))
            .collect();
        let expected: Vec<_> = recorded.iter().map(|s| (s.agent_id, &s.commands)).collect();
        if actual != expected && result.first_difference.is_none() {
            result.first_difference = Some(record.round);
        }
        let execution = resolver.run_round(&mut state, &record.submissions);
        state.advance_round();
        result.rounds.push(HistoryRound {
            observation,
            recorded,
            execution,
            submissions: turn.submissions().to_vec(),
            budgets: turn.budget_reads().to_vec(),
            traces: turn.traces().to_vec(),
        });
    }
    Ok(result)
}

fn quote(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
fn command(
    round: u32,
    agent: arena_core::types::AgentId,
    commands: &[arena_core::types::Command],
) -> String {
    let bytes = arena_protocol::build::envelope_bytes(
        &arena_protocol::decode::Envelope::SubmitCommandStream {
            round,
            agent_id: agent,
            commands: commands.to_vec(),
        },
    );
    quote(&arena_replay::canon::hex(&bytes))
}
impl History {
    /// Machine-readable observations use official public/private snapshot
    /// encodings (hex). No enemy private state is exported. Commands use
    /// official SubmitCommandStream envelope bytes, without frame prefix.
    pub fn to_json(&self, recorded_output: bool) -> String {
        let rounds = self.rounds.iter().map(|r| {
            let n = r.observation.round;
            let outputs = if recorded_output {
                r.recorded.iter().map(|s| command(n, s.agent_id, &s.commands)).collect::<Vec<_>>()
            } else { r.submissions.iter().map(|s| command(n, s.agent_id, &s.commands)).collect() };
            let received = r.recorded.iter().map(|s| format!("{{\"agent\":{},\"received_at_ms\":{},\"receive_seq\":{}}}", s.agent_id.0, s.received_at_ms, s.receive_seq)).collect::<Vec<_>>().join(",");
            let traces = r.traces.iter().map(|t| format!("{{\"agent\":{},\"task\":{},\"target\":{},\"value\":{},\"elapsed_us\":{}}}",t.agent.0,quote(&t.task),quote(&t.target),t.value,t.elapsed_us)).collect::<Vec<_>>().join(",");
            let public = arena_protocol::build::public_snapshot_bytes(&r.observation, n as u64, 0, 0);
            let private = arena_protocol::build::private_snapshot_bytes(&r.observation, n as u64);
            let own_execution: Vec<_> = r.execution.execution_order.iter().filter(|s| s.faction == self.faction).collect();
            format!("{{\"round\":{n},\"public_snapshot_hex\":{},\"private_snapshot_hex\":{},\"commands\":[{}],\"recorded_receipts\":[{received}],\"recorded_execution\":{},\"budgets_ms\":{:?},\"trace\":[{traces}]}}", quote(&arena_replay::canon::hex(&public)),quote(&arena_replay::canon::hex(&private)),outputs.join(","),quote(&format!("{:?}", own_execution)),r.budgets)
        }).collect::<Vec<_>>().join(",");
        format!(
            "{{\"schema\":\"arena-decisions-v1\",\"source_hash\":{},\"budget_input_hash\":{},\"faction\":{},\"scope\":\"recorded-observations\",\"output\":{},\"rounds\":[{rounds}]}}\n",
            quote(&self.source_hash),
            quote(&self.budget_input_hash),
            quote(if self.faction == Faction::Enl {
                "enl"
            } else {
                "res"
            }),
            quote(if recorded_output {
                "recorded"
            } else {
                "strategy"
            })
        )
    }
}

/// Optional executable convention for independent development clients:
/// `--arena-offline SCENARIO_DIR NEW_OUTPUT.json`. Factory gets only public
/// match inputs and must use the same explicit initialization/seed as live.
pub fn run_cli<S: Strategy>(
    args: &[String],
    factory: impl FnOnce(Faction, &arena_core::config::GameConfig, &arena_core::map::MapDefinition) -> S,
) -> io::Result<bool> {
    if args.first().map(String::as_str) != Some("--arena-offline") {
        return Ok(false);
    }
    if args.len() != 3 {
        return Err(invalid("--arena-offline SCENARIO_DIR NEW_OUTPUT.json"));
    }
    let root = std::path::Path::new(&args[1]);
    let faction = match std::fs::read_to_string(root.join("faction"))?.as_str() {
        "enl" => Faction::Enl,
        "res" => Faction::Res,
        _ => return Err(invalid("invalid faction")),
    };
    let log =
        arena_replay::format::decode(&std::fs::read(root.join("source.iarp"))?).map_err(invalid)?;
    let budgets = Budgets::parse(&std::fs::read_to_string(root.join("budgets.tsv"))?)?;
    let mut strategy = factory(faction, &log.header.config, &log.header.map);
    let result = reconstruct(&log, faction, &budgets, &mut strategy)?;
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[2])?;
    file.write_all(result.to_json(false).as_bytes())?;
    file.sync_all()?;
    Ok(true)
}

fn invalid(message: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

/// Versioned text boundary: header, then `round<TAB>comma-separated u64 ms`.
/// An empty list declares that the strategy must not read a budget that round.
pub struct Budgets(BTreeMap<u32, Vec<u64>>);
impl Budgets {
    fn content_hash(&self) -> String {
        let mut hash = blake3::Hasher::new();
        hash.update(b"arena-budgets-v1\0");
        hash.update(&(self.0.len() as u64).to_le_bytes());
        for (round, values) in &self.0 {
            hash.update(&round.to_le_bytes());
            hash.update(&(values.len() as u64).to_le_bytes());
            for value in values {
                hash.update(&value.to_le_bytes());
            }
        }
        hash.finalize().to_hex().to_string()
    }
    pub fn parse(text: &str) -> io::Result<Self> {
        let mut lines = text.lines();
        if lines.next() != Some("arena-budgets-v1") {
            return Err(invalid("unsupported budget document"));
        }
        let mut rounds = BTreeMap::new();
        for line in lines {
            let (round, values) = line
                .split_once('\t')
                .ok_or_else(|| invalid("budget row requires a tab"))?;
            let round: u32 = round.parse().map_err(invalid)?;
            let values = if values.is_empty() {
                vec![]
            } else {
                values
                    .split(',')
                    .map(|v| v.parse::<u64>().map_err(invalid))
                    .collect::<io::Result<Vec<_>>>()?
            };
            if round == 0 || rounds.insert(round, values).is_some() {
                return Err(invalid("zero or duplicate budget round"));
            }
        }
        Ok(Self(rounds))
    }
    pub fn for_round(&self, round: u32) -> io::Result<Vec<u64>> {
        self.0
            .get(&round)
            .cloned()
            .ok_or_else(|| invalid(format!("missing budget round {round}")))
    }
}
