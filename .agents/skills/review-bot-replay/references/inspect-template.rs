// IARP 检查器模板（review-bot-replay 技能）。
//
// 用途：只读单场 .iarp —— 逐轮重执行规则并校验 state hash；可选对 BuilderCore
// 系策略做影子决策（逐轮 assert 生成的命令与档案提交一致）；全部证据以 JSONL
// 输出到 stdout，供 analyze.py 或 ad-hoc Python 分析。不要用 Debug 字符串 TSV
// 加 awk 数列号的方式消费输出。
//
// 建立临时 crate（放在 <repo>/.scratch/<分析目录>/ 下，path 依赖按此相对路径；
// 仓库无 serde_json 依赖，模板只用三个 path 依赖 + 手写 JSON）：
//
//   Cargo.toml:
//     [package]
//     name = "inspect"
//     version = "0.1.0"
//     edition = "2021"
//     [dependencies]
//     arena-core = { path = "../../crates/arena-core" }
//     arena-replay = { path = "../../crates/arena-replay" }
//     arena-bots = { path = "../../crates/arena-bots" }
//     [workspace]
//
// 运行（bot_seed 取自该批次 matches.tsv，不是 game seed）：
//   cargo run --release -- <replay.iarp> <bot_seed> [shadow] > trace.jsonl
//   shadow 形如 `enl=BuilderDefender,res=BuilderDisruptor`（省略或 none = 只做规则重放）。
//   可用 stream：BuilderDefender / BuilderDisruptor / BlockerBuilder（overlay 系）。
//   Logistics/Adaptive 有私有状态机，不能用本模板的 BuilderCore 影子还原。
//
// 参数说明：defender 的防守目标上限按该批次默认 cap=1 写死；disruptor/blocker 用
// 默认参数。影子 assert 与档案逐轮一致即证明参数匹配；assert 失败时先怀疑批次
// 参数不同（从 policies.tsv 用 PolicyParameters::from_tag 恢复后替换默认值）。
//
// 输出 JSONL 行的 kind：config / spawn / portal_pos / agent / portal / field /
// overlay / overlay_change / grant / plan / command / transfer_direct / effect /
// summary。时间语义：agent/portal/field/overlay/grant/plan 是**轮结算前**（该轮
// Bot 的决策输入）；command/transfer_direct/effect 属于该轮结算。未做影子还原的
// 阵营没有 overlay/grant/plan 行——任务分配是策略私有，仅影子可得。校验失败直接
// panic —— 不要在未通过校验的 trace 上做决策归因。

use std::{collections::BTreeMap, env, fs, fmt};

use arena_bots::common;
use arena_bots::r2::{
    build::{BuilderCore, CoreParams},
    blocker_builder::{blocking_requests, BlockerBuilderParams},
    builder_defender::{defense_overlay, merge_requests},
    builder_disruptor::{attack_overlay, BuilderDisruptorParams},
    rng::R2Stream,
    tasks::{allocate, Task, TaskBoard},
};
use arena_core::resolve::{CommandEffect, CommandOutcome};
use arena_core::types::{Command, Faction, PortalId};
use arena_core::{observe::Observation, resolve::Resolver, spawn::validate_initial_spawn_set, state::GameState};
use arena_replay::format::RoundRecord;
use arena_replay::{canon, replay::{decode_log, replay}};

// ---- 极简 JSON 值（仓库无 serde_json，模板不引入外部依赖） ----

enum J { Nul, Bool(bool), Int(i128), Str(String), Arr(Vec<J>), Obj(Vec<(String, J)>) }

impl fmt::Display for J {
    fn fmt(&self, w: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            J::Nul => write!(w, "null"),
            J::Bool(b) => write!(w, "{b}"),
            J::Int(n) => write!(w, "{n}"),
            J::Str(s) => {
                write!(w, "\"")?;
                for c in s.chars() {
                    match c {
                        '"' => write!(w, "\\\"")?,
                        '\\' => write!(w, "\\\\")?,
                        '\n' => write!(w, "\\n")?,
                        '\r' => write!(w, "\\r")?,
                        '\t' => write!(w, "\\t")?,
                        c if (c as u32) < 0x20 => write!(w, "\\u{:04x}", c as u32)?,
                        c => write!(w, "{c}")?,
                    }
                }
                write!(w, "\"")
            }
            J::Arr(items) => {
                write!(w, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 { write!(w, ",")?; }
                    write!(w, "{item}")?;
                }
                write!(w, "]")
            }
            J::Obj(pairs) => {
                write!(w, "{{")?;
                for (i, (k, v)) in pairs.iter().enumerate() {
                    if i > 0 { write!(w, ",")?; }
                    write!(w, "{}:{v}", J::Str(k.clone()))?;
                }
                write!(w, "}}")
            }
        }
    }
}

fn s(v: impl Into<String>) -> J { J::Str(v.into()) }
fn i(v: impl Into<i128>) -> J { J::Int(v.into()) }
fn o(pairs: Vec<(&str, J)>) -> J { J::Obj(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()) }
fn a(items: Vec<J>) -> J { J::Arr(items) }
fn keys_j(keys: &BTreeMap<PortalId, u32>) -> J {
    J::Obj(keys.iter().map(|(p, n)| (p.0.to_string(), i(*n))).collect())
}
fn emit(row: J) { println!("{row}"); }

// ---- 领域对象 → JSON ----

fn faction_s(f: Faction) -> &'static str {
    match f { Faction::Enl => "enl", Faction::Res => "res" }
}

fn command_j(c: &Command) -> J {
    use Command::*;
    match c {
        Move { destination } => o(vec![("kind", s("move")), ("x", i(destination.x as i128)), ("y", i(destination.y as i128))]),
        Attack => o(vec![("kind", s("attack"))]),
        Capture { portal } => o(vec![("kind", s("capture")), ("portal", i(portal.0))]),
        Recharge { portal } => o(vec![("kind", s("recharge")), ("portal", i(portal.0))]),
        Hack { portal } => o(vec![("kind", s("hack")), ("portal", i(portal.0))]),
        Transfer { target_agent, supply, keys } => o(vec![("kind", s("transfer")), ("target", i(target_agent.0)), ("supply", i(*supply)), ("keys", keys_j(keys))]),
        Link { source_portal, target_portal } => o(vec![("kind", s("link")), ("source", i(source_portal.0)), ("target", i(target_portal.0))]),
    }
}

fn effect_j(e: &CommandEffect) -> J {
    use CommandEffect::*;
    match e {
        Moved { from, to, cost } => o(vec![("effect", s("moved")),
            ("from", a(vec![i(from.x as i128), i(from.y as i128)])),
            ("to", a(vec![i(to.x as i128), i(to.y as i128)])), ("cost", i(*cost))]),
        Captured { portal } => o(vec![("effect", s("captured")), ("portal", i(portal.0))]),
        Attacked { hits } => o(vec![("effect", s("attacked")), ("hits", a(hits.iter()
            .map(|h| o(vec![("portal", i(h.portal.0)), ("hp_after", i(h.hp_after)), ("neutralized", J::Bool(h.neutralized))]))
            .collect()))]),
        Recharged { portal, hp_before, hp_after } => o(vec![("effect", s("recharged")),
            ("portal", i(portal.0)), ("hp_before", i(*hp_before)), ("hp_after", i(*hp_after))]),
        Hacked { portal, keys_gained, supply_granted, supply_after } => o(vec![("effect", s("hacked")),
            ("portal", i(portal.0)), ("keys_gained", i(*keys_gained)),
            ("supply_granted", i(*supply_granted)), ("supply_after", i(*supply_after))]),
        Transferred { target, supply, keys } => o(vec![("effect", s("transferred")),
            ("target", i(target.0)), ("supply", i(*supply)), ("keys", keys_j(keys))]),
        Linked { link, fields } => o(vec![("effect", s("linked")),
            ("link", a(vec![i(link.min.0), i(link.max.0)])),
            ("fields", a(fields.iter().map(|f| a(vec![i(f.a.0), i(f.b.0), i(f.c.0)])).collect()))]),
    }
}

fn task_j(t: &Task) -> J {
    let mut pairs = vec![("kind", s(t.kind_tag()))];
    match t {
        Task::Capture { portal } | Task::KeyFarm { portal } | Task::Attack { portal }
        | Task::Recharge { portal } | Task::Standby { portal } => pairs.push(("portal", i(portal.0))),
        Task::Link { source, target } => { pairs.push(("source", i(source.0))); pairs.push(("target", i(target.0))); }
        Task::Transfer { receiver, key } => { pairs.push(("receiver", i(receiver.0))); pairs.push(("key", i(key.0))); }
    }
    o(pairs)
}

fn task_opt(t: Option<&Task>) -> J { match t { Some(t) => task_j(t), None => J::Nul } }

// ---- 影子策略：逐轮还原并与档案对齐 ----

struct Shadow { core: BuilderCore, stream: R2Stream, faction: Faction }

impl Shadow {
    /// refresh → overlay → 两次 allocate 对比 → execute，
    /// assert 生成的命令与档案提交一致；打印 overlay/overlay_change/grant/plan 行。
    fn trace(&mut self, obs: &Observation, record: &RoundRecord) {
        let f = faction_s(self.faction);
        self.core.refresh(obs);
        let mut requests = if self.core.active().is_some() {
            self.core.build_requests(obs)
        } else {
            self.core.fallback_requests(obs)
        };
        let overlay = match self.stream {
            R2Stream::BuilderDefender => defense_overlay(&self.core, obs, 1),
            R2Stream::BuilderDisruptor => attack_overlay(&self.core, obs, &BuilderDisruptorParams::default()),
            R2Stream::BlockerBuilder => blocking_requests(&self.core, obs, &BlockerBuilderParams::default()),
            other => unreachable!("unsupported shadow stream {other:?}"),
        };
        for (agent, rows) in &overlay {
            for row in rows {
                emit(o(vec![("kind", s("overlay")), ("round", i(obs.round)), ("faction", s(f)),
                    ("agent", i(agent.0)), ("task", task_j(&row.task)),
                    ("priority", i(row.priority as i128)), ("capacity", i(row.assignee_capacity as i128))]));
            }
        }
        let agents: Vec<_> = obs.own_agents.iter().map(|me| me.id).collect();
        let base_grants = allocate(&mut TaskBoard::new(obs.round), &agents, &requests);
        merge_requests(&mut requests, overlay.clone());
        let grants = allocate(&mut TaskBoard::new(obs.round), &agents, &requests);
        for agent in &agents {
            if base_grants.get(agent) != grants.get(agent) {
                emit(o(vec![("kind", s("overlay_change")), ("round", i(obs.round)), ("faction", s(f)),
                    ("agent", i(agent.0)),
                    ("base", task_opt(base_grants.get(agent))), ("merged", task_opt(grants.get(agent)))]));
            }
        }
        for agent in &agents {
            let grant = grants.get(agent);
            let priority = requests.get(agent).into_iter().flatten()
                .filter(|r| Some(&r.task) == grant).map(|r| r.priority).max();
            let over = overlay.get(agent).is_some_and(|rows|
                rows.iter().any(|r| Some(&r.task) == grant && Some(r.priority) == priority));
            emit(o(vec![("kind", s("grant")), ("round", i(obs.round)), ("faction", s(f)), ("agent", i(agent.0)),
                ("task", task_opt(grant)), ("priority", priority.map(|p| i(p as i128)).unwrap_or(J::Nul)),
                ("overlay", J::Bool(over))]));
        }
        emit(o(vec![("kind", s("plan")), ("round", i(obs.round)), ("faction", s(f)),
            ("plan", s(format!("{:?}", self.core.active())))]));
        let commands = self.core.execute(obs, &grants);
        let actual: BTreeMap<_, _> = commands.into_iter()
            .map(|(id, list)| (id, list.build()))
            .filter(|(_, list)| !list.is_empty()).collect();
        let expected: BTreeMap<_, _> = record.submissions.iter()
            .filter(|sub| sub.faction == self.faction && !sub.commands.is_empty())
            .map(|sub| (sub.agent_id, sub.commands.clone())).collect();
        assert_eq!(actual, expected,
            "shadow decisions differ at round {} {:?}", obs.round, self.faction);
    }
}

fn parse_shadow(spec: &str) -> Vec<(Faction, R2Stream)> {
    spec.split(',').filter(|p| !p.is_empty()).map(|pair| {
        let (f, name) = pair.split_once('=').expect("shadow spec: enl=BuilderDefender,res=...");
        let faction = match f { "enl" => Faction::Enl, "res" => Faction::Res, _ => panic!("faction: enl|res") };
        let stream = match name {
            "BuilderDefender" => R2Stream::BuilderDefender,
            "BuilderDisruptor" => R2Stream::BuilderDisruptor,
            "BlockerBuilder" => R2Stream::BlockerBuilder,
            other => panic!("stream {other:?} 不支持影子还原（Logistics/Adaptive 有私有状态）"),
        };
        (faction, stream)
    }).collect()
}

fn main() {
    let args: Vec<_> = env::args().collect();
    let log = decode_log(&fs::read(&args[1]).unwrap()).unwrap();
    let outcome = replay(&log).unwrap();
    let seed: u64 = args[2].parse().unwrap();
    let shadow_spec = args.get(3).map(|x| x.as_str()).unwrap_or("none");
    let specs = if shadow_spec == "none" { Vec::new() } else { parse_shadow(shadow_spec) };
    let h = &log.header;
    let c = &h.config;
    emit(o(vec![("kind", s("config")), ("total_rounds", i(c.total_rounds as i128)),
        ("move_budget_cm", i(c.move_budget_cm as i128)),
        ("portal_operation_range_cm", i(c.portal_operation_range_cm as i128)),
        ("transfer_range_cm", i(c.transfer_range_cm as i128)),
        ("initial_supply", i(c.initial_supply as i128)), ("max_supply", i(c.max_supply as i128)),
        ("portal_max_hp", i(c.portal_max_hp as i128)), ("attack_damage", i(c.attack_damage as i128)),
        ("recharge_amount", i(c.recharge_amount as i128)),
        ("recharge_cost_supply", i(c.recharge_cost_supply as i128)),
        ("hack_supply_yield", i(c.hack_supply_yield as i128)),
        ("max_link_distance_cm", i(c.max_link_distance_cm as i128)),
        ("game_seed", i(h.game_seed as i128))]));
    let (enl_roster, res_roster) = GameState::initial_roster(&h.config);
    let enl_spawns = validate_initial_spawn_set(&enl_roster, &h.enl_spawns, &h.config).unwrap();
    let res_spawns = validate_initial_spawn_set(&res_roster, &h.res_spawns, &h.config).unwrap();
    for (f, spawns) in [(Faction::Enl, &h.enl_spawns), (Faction::Res, &h.res_spawns)] {
        for sp in spawns {
            emit(o(vec![("kind", s("spawn")), ("faction", s(faction_s(f))), ("agent", i(sp.0 .0)),
                ("x", i(sp.1.x as i128)), ("y", i(sp.1.y as i128))]));
        }
    }
    let mut state = GameState::new(h.config, &h.map, &enl_spawns, &res_spawns).unwrap();
    let resolver = Resolver::with_seed(h.game_seed);
    let mut shadows: Vec<Shadow> = Vec::new();
    for (faction, stream) in specs {
        let mut core = BuilderCore::new(faction, seed, stream, &h.config, CoreParams::default());
        let (roster, expected) = if faction == Faction::Enl { (&enl_roster, &h.enl_spawns) }
                                 else { (&res_roster, &h.res_spawns) };
        assert_eq!(&core.choose_spawns(&h.config, &h.map, roster), expected,
            "spawn mismatch —— 检查 bot seed / 策略身份 / 换边腿 / policy_a_is_enl 映射");
        shadows.push(Shadow { core, stream, faction });
    }
    let mut printed_portals = false;
    for record in log.round_records() {
        for faction in [Faction::Enl, Faction::Res] {
            let obs = Observation::for_faction(&state, faction);
            if faction == Faction::Enl {
                if !printed_portals {
                    for p in &obs.portals {
                        emit(o(vec![("kind", s("portal_pos")), ("portal", i(p.id.0)),
                            ("x", i(p.position.x as i128)), ("y", i(p.position.y as i128))]));
                    }
                    printed_portals = true;
                }
                // 公共视图双方一致（R-VIS-01）：portal/field 只走一遍 ENL 观察。
                for p in obs.portals.iter().filter(|p| p.owner != arena_core::types::Owner::Neutral) {
                    emit(o(vec![("kind", s("portal")), ("round", i(record.round)), ("portal", i(p.id.0)),
                        ("owner", s(format!("{:?}", p.owner).to_lowercase())), ("hp", i(p.hp as i128))]));
                }
                for f in &obs.fields {
                    emit(o(vec![("kind", s("field")), ("round", i(record.round)),
                        ("faction", s(faction_s(f.faction))),
                        ("a", i(f.id.a.0)), ("b", i(f.id.b.0)), ("c", i(f.id.c.0)),
                        ("area2", i(f.area2 as i128))]));
                }
            }
            let fs = faction_s(faction);
            for me in &obs.own_agents {
                emit(o(vec![("kind", s("agent")), ("round", i(record.round)), ("faction", s(fs)),
                    ("agent", i(me.id.0)), ("x", i(me.position.x as i128)), ("y", i(me.position.y as i128)),
                    ("supply", i(me.supply as i128)), ("keys", keys_j(&me.keys))]));
            }
            for shadow in shadows.iter_mut().filter(|sh| sh.faction == faction) {
                shadow.trace(&obs, record);
            }
        }
        for submission in &record.submissions {
            let fs = faction_s(submission.faction);
            for command in &submission.commands {
                emit(o(vec![("kind", s("command")), ("round", i(record.round)), ("faction", s(fs)),
                    ("agent", i(submission.agent_id.0)), ("cmd", command_j(command))]));
                if let Command::Transfer { keys, .. } = command {
                    let obs = Observation::for_faction(&state, submission.faction);
                    let me = obs.own_agents.iter().find(|ag| ag.id == submission.agent_id).unwrap();
                    let (own, _enemy) = common::owners(submission.faction);
                    for key in keys.keys() {
                        let legal: Vec<_> = obs.portals.iter()
                            .filter(|p| common::link_is_legal(&obs, own, &h.config,
                                me.position, &me.keys, p.id, *key))
                            .map(|p| i(p.id.0)).collect();
                        emit(o(vec![("kind", s("transfer_direct")), ("round", i(record.round)),
                            ("faction", s(fs)), ("agent", i(submission.agent_id.0)),
                            ("key", i(key.0)), ("sources", J::Arr(legal))]));
                    }
                }
            }
        }
        let report = resolver.run_round(&mut state, &record.submissions);
        for stream in &report.execution_order {
            let fs = faction_s(stream.faction);
            for (seq, result) in stream.outcomes.iter().enumerate() {
                let row = match result {
                    CommandOutcome::Ok(e) => {
                        let mut v = effect_j(e);
                        if let J::Obj(pairs) = &mut v { pairs.push(("status".to_string(), s("ok"))); }
                        v
                    }
                    CommandOutcome::Drop(reason) => {
                        let dbg = format!("{reason:?}");
                        let kind = dbg.split(['{', '(']).next().unwrap_or("").trim().to_string();
                        o(vec![("status", s("drop")), ("reason", s(dbg)), ("reason_kind", s(kind))])
                    }
                };
                // status 放进 result 之外，便于过滤：effect 行的 result.status。
                emit(o(vec![("kind", s("effect")), ("round", i(record.round)), ("faction", s(fs)),
                    ("agent", i(stream.agent_id.0)), ("seq", i(seq as i128)), ("result", row)]));
            }
        }
        state.advance_round();
        assert_eq!(canon::state_hash(&state), record.state_hash);
    }
    emit(o(vec![("kind", s("summary")), ("rounds", i(outcome.rounds_completed as i128)),
        ("result", s(format!("{:?}", outcome.result))),
        ("scores", s(format!("{:?}", outcome.final_scores))),
        ("final_hash", s(canon::hex(&outcome.final_state_hash)))]));
    eprintln!("{} rounds verified; {} shadow faction(s) matched archive; outcome {:?}; hash {}",
        outcome.rounds_completed, shadows.len(), outcome.result, canon::hex(&outcome.final_state_hash));
}
