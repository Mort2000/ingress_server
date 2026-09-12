//! Recomputable, descriptive paired comparison. No independence assumption or CI.
use super::{archive::Json, document::*, *};
use arena_core::{
    resolve::{CommandOutcome, Resolver, SlotStatus},
    spawn::validate_initial_spawn_set,
    state::GameState,
    types::Faction,
};
use arena_replay::format::ReplayFile;
use std::{collections::BTreeMap, fs, io, path::Path};

#[derive(Default)]
struct Diagnostics {
    missing: [u64; 2],
    rejected: [u64; 2],
    dropped: [u64; 2],
}
fn diagnostics(log: &ReplayFile) -> io::Result<Diagnostics> {
    let h = &log.header;
    let (a, b) = GameState::initial_roster(&h.config);
    let a = validate_initial_spawn_set(&a, &h.enl_spawns, &h.config)
        .map_err(|e| invalid(format!("{e:?}")))?;
    let b = validate_initial_spawn_set(&b, &h.res_spawns, &h.config)
        .map_err(|e| invalid(format!("{e:?}")))?;
    let mut state =
        GameState::new(h.config, &h.map, &a, &b).map_err(|e| invalid(format!("{e:?}")))?;
    let resolver = Resolver::with_seed(h.game_seed);
    let mut d = Diagnostics::default();
    for round in log.round_records() {
        let report = resolver.run_round(&mut state, &round.submissions);
        for (agent, status) in &report.slots {
            let side = if state
                .agent(*agent)
                .ok_or_else(|| invalid("slot agent missing"))?
                .faction
                == Faction::Enl
            {
                0
            } else {
                1
            };
            match status {
                SlotStatus::NotSubmitted => d.missing[side] += 1,
                SlotStatus::Rejected(_) => d.rejected[side] += 1,
                _ => {}
            }
        }
        for stream in report.execution_order {
            let side = if stream.faction == Faction::Enl { 0 } else { 1 };
            d.dropped[side] += stream
                .outcomes
                .iter()
                .filter(|o| matches!(o, CommandOutcome::Drop(_)))
                .count() as u64;
        }
        state.advance_round();
    }
    Ok(d)
}
fn counts(a: [u64; 2]) -> Json {
    Json::Array(a.into_iter().map(|n| Json::Int(n as i64)).collect())
}
fn first_differences(a: &ReplayFile, b: &ReplayFile) -> Json {
    let mut state = None;
    let mut command = None;
    if a.header.enl_spawns != b.header.enl_spawns || a.header.res_spawns != b.header.res_spawns {
        state = Some(0);
    }
    let ar = a.round_records().collect::<Vec<_>>();
    let br = b.round_records().collect::<Vec<_>>();
    for i in 0..ar.len().max(br.len()) {
        match (ar.get(i), br.get(i)) {
            (Some(a), Some(b)) => {
                if state.is_none() && a.state_hash != b.state_hash {
                    state = Some(a.round);
                }
                let commands = |r: &arena_replay::format::RoundRecord| {
                    let mut v = r
                        .submissions
                        .iter()
                        .map(|s| (s.agent_id, s.faction, s.commands.clone()))
                        .collect::<Vec<_>>();
                    v.sort_by_key(|s| s.0);
                    v
                };
                if command.is_none() && commands(a) != commands(b) {
                    command = Some(a.round);
                }
            }
            _ => {
                if state.is_none() {
                    state = Some(i as u32 + 1);
                }
                if command.is_none() {
                    command = Some(i as u32 + 1);
                }
            }
        }
    }
    object(vec![
        (
            "first_state_round",
            state.map(|n| Json::Int(n as i64)).unwrap_or(Json::Null),
        ),
        (
            "first_command_round",
            command.map(|n| Json::Int(n as i64)).unwrap_or(Json::Null),
        ),
    ])
}

/// Normal outcomes alone get comparison points (2/1/0). Technical results
/// remain final adopted results but are reported separately, as with M9 Elo.
fn points(outcome: &TournamentOutcome, item: &TournamentItem, candidate: u32) -> Option<i64> {
    if !outcome.elo_relevant() {
        return None;
    }
    Some(match outcome.normal_winner(item.a, item.b, item.a_is_enl) {
        Some(id) if id == candidate => 2,
        Some(_) => 0,
        None => 1,
    })
}
pub fn write(root: &Path) -> io::Result<Json> {
    let run = durable::execute(root, durable::Action::Rebuild)?;
    let plan = durable::load(root)?;
    let ext = plan
        .extension
        .as_ref()
        .ok_or_else(|| invalid("comparison requires batch-two metadata"))?;
    let samples = array(ext, "samples")?;
    let mut rows = Vec::new();
    let mut logs = Vec::new();
    let mut paired: BTreeMap<(String, u32), [Option<usize>; 2]> = BTreeMap::new();
    let mut matrix: BTreeMap<(String, String, String, u64), [u64; 5]> = BTreeMap::new();
    let mut faults = 0;
    let mut technical = 0;
    for (i, (item, outcome)) in plan.items.iter().zip(&run.outcomes).enumerate() {
        let sample = &samples[i];
        let candidate = text(sample, "candidate")?;
        let cid = plan
            .config
            .entrants
            .iter()
            .position(|e| e.name == candidate)
            .ok_or_else(|| invalid("sample candidate missing"))? as u32;
        let variant = text(sample, "variant")?;
        let baseline = text(sample, "baseline")?;
        let value = points(outcome, item, cid);
        let infra = matches!(
            outcome.stage(),
            Some(outcome::Stage::RunnerFault | outcome::Stage::SetupError)
        );
        if infra {
            faults += 1;
        } else if !outcome.elo_relevant() {
            technical += 1;
        }
        let tally = matrix
            .entry((
                candidate.into(),
                baseline.into(),
                item.map_hash.clone(),
                item.config_hash,
            ))
            .or_default();
        tally[match value {
            Some(2) => 0,
            Some(1) => 1,
            Some(0) => 2,
            _ if infra => 4,
            _ => 3,
        }] += 1;
        let replay = run.replay_rel[i]
            .as_ref()
            .map(|p| format!("matches/m{}/{p}", item.index));
        let log = replay
            .as_ref()
            .map(|p| arena_replay::replay::decode_log(&fs::read(root.join(p))?).map_err(invalid))
            .transpose()?;
        let d = log.as_ref().map(diagnostics).transpose()?;
        let logical = format!("matches/m{}", item.index);
        let attempt = if let Some(replay) = &replay {
            Path::new(replay)
                .parent()
                .and_then(Path::parent)
                .map(|p| p.to_string_lossy().to_string())
        } else {
            let dir = root.join(&logical).join("attempts");
            if dir.is_dir() {
                let mut names = fs::read_dir(dir)?
                    .map(|e| e.map(|e| e.file_name()))
                    .collect::<io::Result<Vec<_>>>()?;
                names.sort();
                names
                    .last()
                    .map(|n| format!("{logical}/attempts/{}", n.to_string_lossy()))
            } else {
                None
            }
        };
        let scores = if let TournamentOutcome::Normal {
            score_enl,
            score_res,
            ..
        } = outcome
        {
            Json::Array(vec![string(score_enl), string(score_res)])
        } else {
            Json::Null
        };
        rows.push(object(vec![
            ("item", Json::Int(item.index as i64)),
            ("pair", string(text(sample, "pair")?)),
            ("leg", Json::Int(number(sample, "leg")? as i64)),
            ("variant", string(variant)),
            ("candidate", string(candidate)),
            ("baseline", string(baseline)),
            ("map_hash", string(&item.map_hash)),
            ("config_hash", string(format!("{:016x}", item.config_hash))),
            ("outcome", string(outcome.outcome_tag())),
            (
                "stage",
                string(outcome.stage().map(|s| s.tag()).unwrap_or("normal")),
            ),
            ("reason", string(outcome.reason().unwrap_or(""))),
            ("points2", value.map(Json::Int).unwrap_or(Json::Null)),
            ("score2_enl_res", scores),
            (
                "candidate_faction",
                string(if item.enl_entrant() == cid {
                    "ENL"
                } else {
                    "RES"
                }),
            ),
            ("archive", string(attempt.as_deref().unwrap_or(&logical))),
            (
                "execution_id",
                attempt
                    .as_ref()
                    .and_then(|p| fs::read_to_string(root.join(p).join("execution-id")).ok())
                    .map(string)
                    .unwrap_or(Json::Null),
            ),
            (
                "candidate_binary_hash",
                string(
                    plan.config
                        .entrant(cid)
                        .provenance
                        .binary_blake3
                        .as_deref()
                        .unwrap_or("unknown"),
                ),
            ),
            ("replay", replay.map(string).unwrap_or(Json::Null)),
            (
                "missing_enl_res",
                d.as_ref().map(|d| counts(d.missing)).unwrap_or(Json::Null),
            ),
            (
                "rejected_enl_res",
                d.as_ref().map(|d| counts(d.rejected)).unwrap_or(Json::Null),
            ),
            (
                "dropped_enl_res",
                d.as_ref().map(|d| counts(d.dropped)).unwrap_or(Json::Null),
            ),
        ]));
        if matches!(variant, "before" | "after") {
            let slots = paired
                .entry((text(sample, "pair")?.into(), number(sample, "leg")?))
                .or_insert([None, None]);
            let slot = usize::from(variant == "after");
            if slots[slot].replace(i).is_some() {
                return Err(invalid("duplicate comparison sample"));
            }
        }
        logs.push(log);
    }
    let mut changes = Vec::new();
    let mut completed = 0;
    let mut incomplete = 0;
    let mut regressions = 0;
    for ((pair, leg), [before, after]) in paired {
        let (Some(a), Some(b)) = (before, after) else {
            incomplete += 1;
            continue;
        };
        let ai = &plan.items[a];
        let bi = &plan.items[b];
        if ai.game_seed != bi.game_seed
            || ai.client_seed != bi.client_seed
            || ai.map_hash != bi.map_hash
            || ai.config_hash != bi.config_hash
            || rows[a].get("candidate_faction") != rows[b].get("candidate_faction")
        {
            return Err(invalid("paired inputs differ"));
        }
        let old = rows[a].get("points2").and_then(Json::as_int);
        let new = rows[b].get("points2").and_then(Json::as_int);
        let delta = old.zip(new).map(|(a, b)| b - a);
        if delta.is_some() {
            completed += 1;
        } else {
            incomplete += 1;
        }
        if delta.is_some_and(|d| d < 0) {
            regressions += 1;
        }
        let differences = logs[a]
            .as_ref()
            .zip(logs[b].as_ref())
            .map(|(a, b)| first_differences(a, b))
            .unwrap_or(Json::Null);
        changes.push(object(vec![
            ("pair", string(pair)),
            ("leg", Json::Int(leg as i64)),
            ("before_item", Json::Int(a as i64)),
            ("after_item", Json::Int(b as i64)),
            ("delta_points2", delta.map(Json::Int).unwrap_or(Json::Null)),
            (
                "outcome_changed",
                Json::Bool(rows[a].get("outcome") != rows[b].get("outcome") || old != new),
            ),
            ("differences", differences),
        ]));
    }
    let matrix = matrix
        .into_iter()
        .map(|((candidate, baseline, map, config), n)| {
            object(vec![
                ("candidate", string(candidate)),
                ("baseline", string(baseline)),
                ("map_hash", string(map)),
                ("config_hash", string(format!("{config:016x}"))),
                ("wins", Json::Int(n[0] as i64)),
                ("draws", Json::Int(n[1] as i64)),
                ("losses", Json::Int(n[2] as i64)),
                ("technical", Json::Int(n[3] as i64)),
                ("infrastructure_or_pending", Json::Int(n[4] as i64)),
            ])
        })
        .collect();
    let report = object(vec![
        ("schema", string("arena-comparison-v1")),
        ("analysis_version", Json::Int(1)),
        ("mode", string(text(ext, "mode")?)),
        ("maps", Json::Array(array(ext, "maps")?.to_vec())),
        (
            "resources",
            object(vec![
                ("workers", Json::Int(plan.config.concurrency as i64)),
                (
                    "per_match_budget_ms",
                    string(plan.config.per_match_budget_ms),
                ),
                ("sandbox", string(plan.config.sandbox.tag())),
            ]),
        ),
        ("paired_samples", Json::Int(completed)),
        ("incomplete_or_technical_pairs", Json::Int(incomplete)),
        ("regressions", Json::Int(regressions)),
        ("infrastructure_or_pending", Json::Int(faults)),
        ("technical_results", Json::Int(technical)),
        ("rows", Json::Array(rows)),
        ("changes", Json::Array(changes)),
        ("opponent_map_matrix", Json::Array(matrix)),
    ]);
    let mut markdown = format!(
        "# 版本比较报告\n\n运行模式：{}。计划 {} 场；有效同输入对照 {} 组；退步 {} 组；不完整或技术终止对照 {} 组。\n\n正常结果计 2/1/0；技术判负单独报告，不计入 Elo 或此处正常胜负差。基础设施故障和待运行不计分。不估计置信区间；同地图样本有关联，不能视为独立样本。\n\n首次状态/命令差异用于定位，不证明后续胜负的唯一原因；实际接收时序可能不同。\n\n详细逐场整数数据、对手×地图矩阵见 [comparison.json](comparison.json)，采用记录见 [adoptions.tsv](adoptions.tsv)。\n\n## 差异样本\n\n",
        text(ext, "mode")?,
        plan.items.len(),
        completed,
        regressions,
        incomplete
    );
    for change in array(&report, "changes")? {
        let a = number(change, "before_item")?;
        let b = number(change, "after_item")?;
        let delta = change.get("delta_points2").and_then(Json::as_int);
        let row_a = &array(&report, "rows")?[a as usize];
        let row_b = &array(&report, "rows")?[b as usize];
        markdown.push_str(&format!(
            "- 样本 {a} → {b}：积分×2差 {}；[旧档案]({}/meta.json) / [新档案]({}/meta.json)",
            delta
                .map(|d| d.to_string())
                .unwrap_or_else(|| "不可比较".into()),
            text(row_a, "archive")?,
            text(row_b, "archive")?
        ));
        for i in [a, b] {
            if let Some(p) = array(&report, "rows")?[i as usize]
                .get("replay")
                .and_then(Json::as_str)
            {
                markdown.push_str(&format!(" / [回放 {i}]({p})"));
            }
        }
        markdown.push('\n');
    }
    markdown.push_str("\n## 异常与技术结果\n\n");
    for row in array(&report, "rows")? {
        if text(row, "stage")? != "normal" {
            let i = number(row, "item")?;
            markdown.push_str(&format!(
                "- [{i} 的档案]({}/meta.json)：{}\n",
                text(row, "archive")?,
                text(row, "stage")?
            ));
        }
    }
    markdown.push_str("\n## 对手与地图分层\n\n| 候选 | 对手 | 地图哈希 | 配置哈希 | 胜 / 平 / 负 | 技术终止 | 故障 / 待运行 |\n|---|---|---|---|---|---|---|\n");
    for row in array(&report, "opponent_map_matrix")? {
        markdown.push_str(&format!(
            "| {} | {} | {} | {} | {} / {} / {} | {} | {} |\n",
            text(row, "candidate")?,
            text(row, "baseline")?,
            text(row, "map_hash")?,
            text(row, "config_hash")?,
            number(row, "wins")?,
            number(row, "draws")?,
            number(row, "losses")?,
            number(row, "technical")?,
            number(row, "infrastructure_or_pending")?
        ));
    }
    durable::atomic_write(&root.join("comparison.json"), encode(&report).as_bytes())?;
    durable::atomic_write(&root.join("comparison.md"), markdown.as_bytes())?;
    Ok(report)
}
pub fn execute_cli(args: &[String]) -> io::Result<Option<String>> {
    if args.first().map(String::as_str) != Some("compare") {
        return Ok(None);
    }
    let opts = assets_cli::options(&args[1..], &["--out"])?;
    let root = Path::new(assets_cli::required(&opts, "--out")?);
    let report = write(root)?;
    Ok(Some(format!(
        "paired_samples={} regressions={} report={}\n",
        number(&report, "paired_samples")?,
        number(&report, "regressions")?,
        root.join("comparison.md").display()
    )))
}
