//! Read-only version-matched replay review. JSON adapters reuse the review-bot-replay template.
//! Both production policies and a request-level shadow must match every archived stream.
//! Supply probes change one observed inventory only; they are not simulated game outcomes.
use arena_bots::{
    common,
    policy::{PolicyId, PolicyParameters, PolicySpec, make_policy_player},
    r2::{
        R2_VERSION,
        build::{BuilderCore, DecisionTrace},
        builder_defender::{defense_overlay, merge_requests},
        builder_disruptor::{BuilderDisruptorParams, attack_overlay},
        opponent::OpponentModel,
        rng::R2Stream,
        tasks::{Task, TaskRequest},
    },
};
use arena_core::{
    config::GameConfig,
    harness::AgentSubmission,
    observe::Observation,
    resolve::{CommandEffect, CommandOutcome, Resolver},
    spawn::validate_initial_spawn_set,
    state::GameState,
    types::{AgentId, Command, Faction, PortalId},
};
use arena_replay::{
    canon,
    replay::{decode_log, replay},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fmt, fs,
};

enum J {
    Nul,
    Bool(bool),
    Int(i128),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

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
                    if i > 0 {
                        write!(w, ",")?;
                    }
                    write!(w, "{item}")?;
                }
                write!(w, "]")
            }
            J::Obj(pairs) => {
                write!(w, "{{")?;
                for (i, (k, v)) in pairs.iter().enumerate() {
                    if i > 0 {
                        write!(w, ",")?;
                    }
                    write!(w, "{}:{v}", J::Str(k.clone()))?;
                }
                write!(w, "}}")
            }
        }
    }
}

fn s(v: impl Into<String>) -> J {
    J::Str(v.into())
}
fn i(v: impl Into<i128>) -> J {
    J::Int(v.into())
}
fn o(pairs: Vec<(&str, J)>) -> J {
    J::Obj(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}
fn a(items: Vec<J>) -> J {
    J::Arr(items)
}

fn config_j(cfg: &GameConfig) -> J {
    let mut bytes = Vec::new();
    arena_core::codec::push_config(cfg, &mut bytes);
    o(arena_runner::balance::plan::CONFIG_FIELD_NAMES.iter()
        .zip(bytes.chunks_exact(4))
        .map(|(&key, word)| (key, i(u32::from_le_bytes(word.try_into().unwrap()))))
        .collect())
}
fn keys_j(keys: &BTreeMap<PortalId, u32>) -> J {
    J::Obj(keys.iter().map(|(p, n)| (p.0.to_string(), i(*n))).collect())
}
fn emit(row: J) {
    println!("{row}");
}

// ---- 领域对象 → JSON ----

fn faction_s(f: Faction) -> &'static str {
    match f {
        Faction::Enl => "enl",
        Faction::Res => "res",
    }
}

fn command_j(c: &Command) -> J {
    use Command::*;
    match c {
        Move { destination } => o(vec![
            ("kind", s("move")),
            ("x", i(destination.x as i128)),
            ("y", i(destination.y as i128)),
        ]),
        Attack => o(vec![("kind", s("attack"))]),
        Capture { portal } => o(vec![("kind", s("capture")), ("portal", i(portal.0))]),
        Recharge { portal } => o(vec![("kind", s("recharge")), ("portal", i(portal.0))]),
        Hack { portal } => o(vec![("kind", s("hack")), ("portal", i(portal.0))]),
        Transfer {
            target_agent,
            supply,
            keys,
        } => o(vec![
            ("kind", s("transfer")),
            ("target", i(target_agent.0)),
            ("supply", i(*supply)),
            ("keys", keys_j(keys)),
        ]),
        Link {
            source_portal,
            target_portal,
        } => o(vec![
            ("kind", s("link")),
            ("source", i(source_portal.0)),
            ("target", i(target_portal.0)),
        ]),
    }
}

fn effect_j(e: &CommandEffect) -> J {
    use CommandEffect::*;
    match e {
        Moved { from, to, cost } => o(vec![
            ("effect", s("moved")),
            ("from", a(vec![i(from.x as i128), i(from.y as i128)])),
            ("to", a(vec![i(to.x as i128), i(to.y as i128)])),
            ("cost", i(*cost)),
        ]),
        Captured { portal } => o(vec![("effect", s("captured")), ("portal", i(portal.0))]),
        Attacked { hits } => o(vec![
            ("effect", s("attacked")),
            (
                "hits",
                a(hits
                    .iter()
                    .map(|h| {
                        o(vec![
                            ("portal", i(h.portal.0)),
                            ("hp_after", i(h.hp_after)),
                            ("neutralized", J::Bool(h.neutralized)),
                        ])
                    })
                    .collect()),
            ),
        ]),
        Recharged {
            portal,
            hp_before,
            hp_after,
        } => o(vec![
            ("effect", s("recharged")),
            ("portal", i(portal.0)),
            ("hp_before", i(*hp_before)),
            ("hp_after", i(*hp_after)),
        ]),
        Hacked {
            portal,
            keys_gained,
            supply_granted,
            supply_after,
        } => o(vec![
            ("effect", s("hacked")),
            ("portal", i(portal.0)),
            ("keys_gained", i(*keys_gained)),
            ("supply_granted", i(*supply_granted)),
            ("supply_after", i(*supply_after)),
        ]),
        Transferred {
            target,
            supply,
            keys,
        } => o(vec![
            ("effect", s("transferred")),
            ("target", i(target.0)),
            ("supply", i(*supply)),
            ("keys", keys_j(keys)),
        ]),
        Linked { link, fields } => o(vec![
            ("effect", s("linked")),
            ("link", a(vec![i(link.min.0), i(link.max.0)])),
            (
                "fields",
                a(fields
                    .iter()
                    .map(|f| a(vec![i(f.a.0), i(f.b.0), i(f.c.0)]))
                    .collect()),
            ),
        ]),
    }
}

fn task_j(t: &Task) -> J {
    let mut pairs = vec![("kind", s(t.kind_tag()))];
    match t {
        Task::Capture { portal }
        | Task::KeyFarm { portal }
        | Task::Attack { portal }
        | Task::Recharge { portal }
        | Task::Standby { portal } => pairs.push(("portal", i(portal.0))),
        Task::Link { source, target } => {
            pairs.push(("source", i(source.0)));
            pairs.push(("target", i(target.0)));
        }
        Task::Transfer { receiver, key } => {
            pairs.push(("receiver", i(receiver.0)));
            pairs.push(("key", i(key.0)));
        }
    }
    o(pairs)
}

fn task_opt(t: Option<&Task>) -> J {
    match t {
        Some(t) => task_j(t),
        None => J::Nul,
    }
}

struct Review {
    core: BuilderCore,
    params: PolicyParameters,
    opponent: Option<OpponentModel>,
}

impl Review {
    fn new(spec: &PolicySpec, side: Faction, seed: u64, cfg: &GameConfig) -> Self {
        let (params, stream, opponent) = match spec.parameters {
            PolicyParameters::BuilderDefender(p) => (p.core, R2Stream::BuilderDefender, None),
            PolicyParameters::AdaptiveHybridR2(p) => (
                p.core,
                R2Stream::AdaptiveHybrid,
                Some(OpponentModel::new(p.opponent, side.opposite())),
            ),
            _ => panic!("only Defender and Adaptive subjects are supported"),
        };
        Self {
            core: BuilderCore::new(side, seed, stream, cfg, params),
            params: spec.parameters,
            opponent,
        }
    }

    // Mirrors the archived public request path; full submission and trace equality
    // below guard against stale or incomplete reconstruction.
    fn requests(&self, obs: &Observation) -> BTreeMap<AgentId, Vec<TaskRequest>> {
        let mut requests = if self.core.active().is_some() {
            self.core.build_requests(obs)
        } else {
            self.core.fallback_requests(obs)
        };
        match self.params {
            PolicyParameters::BuilderDefender(p) => {
                merge_requests(
                    &mut requests,
                    defense_overlay(&self.core, obs, p.defense_task_cap),
                );
            }
            PolicyParameters::AdaptiveHybridR2(p) => {
                let share_round =
                    obs.round as u64 * 1000 / self.core.cfg().total_rounds.max(1) as u64;
                let opening = share_round < p.opening_permille as u64;
                let endgame = !opening && share_round >= p.endgame_permille as u64;
                if self.opponent.as_ref().unwrap().any_threat(obs) || endgame {
                    merge_requests(
                        &mut requests,
                        defense_overlay(&self.core, obs, p.defense_task_cap),
                    );
                }
                let own = obs.scores[&self.core.faction()];
                let enemy = obs.scores[&self.core.faction().opposite()];
                let share = if own + enemy == 0 {
                    500
                } else {
                    own * 1000 / (own + enemy)
                };
                if !opening
                    && (endgame
                        || share >= p.attack_lead_share_permille as u128
                        || self.core.active().is_none())
                {
                    merge_requests(
                        &mut requests,
                        attack_overlay(
                            &self.core,
                            obs,
                            &BuilderDisruptorParams {
                                attack_share_permille: p.attack_share_permille,
                                core: p.core,
                            },
                        ),
                    );
                }
            }
            _ => unreachable!(),
        }
        requests
    }

    fn trace(&mut self, obs: &Observation, expected: &[AgentSubmission], decision: &DecisionTrace) {
        self.core.refresh(obs);
        if let Some(opponent) = &mut self.opponent {
            opponent.update(obs);
        }
        let requests = self.requests(obs);
        let actual = self.core.preview_allocation(obs, &requests);
        assert_eq!(&actual, decision, "request shadow round {}", obs.round);
        let cfg = *self.core.cfg();
        for agent in &obs.own_agents {
            let hackable = obs
                .portals
                .iter()
                .filter(|p| {
                    common::in_range(agent.position, p.position, cfg.portal_operation_range_cm)
                        && self
                            .core
                            .ledger_mut()
                            .available(obs.round, &cfg, agent.id, p.id)
                })
                .map(|p| i(p.id.0))
                .collect();
            emit(o(vec![
                ("kind", s("agent")),
                ("round", i(obs.round)),
                ("agent", i(agent.id.0)),
                (
                    "position",
                    a(vec![i(agent.position.x), i(agent.position.y)]),
                ),
                ("supply", i(agent.supply)),
                ("keys", keys_j(&agent.keys)),
                ("hackable_here", a(hackable)),
            ]));
            for request in requests.get(&agent.id).into_iter().flatten() {
                emit(o(vec![
                    ("kind", s("candidate")),
                    ("round", i(obs.round)),
                    ("agent", i(agent.id.0)),
                    ("task", task_j(&request.task)),
                    ("priority", i(request.priority)),
                    ("capacity", i(request.assignee_capacity)),
                ]));
            }
        }
        for agent in &actual.agents {
            emit(o(vec![
                ("kind", s("grant")),
                ("round", i(obs.round)),
                ("agent", i(agent.agent.0)),
                ("primary", task_opt(agent.grant.as_ref())),
                ("companion", task_opt(agent.companion.as_ref())),
                ("candidates", i(agent.candidate_count as u64)),
                ("unavailable", i(agent.unavailable_count as u64)),
            ]));
        }
        for (name, plan) in [
            ("primary", self.core.active()),
            ("second", self.core.second()),
        ] {
            emit(o(vec![
                ("kind", s("plan")),
                ("round", i(obs.round)),
                ("slot", s(name)),
                (
                    "vertices",
                    plan.map_or(J::Nul, |p| a(p.vertices.iter().map(|p| i(p.0)).collect())),
                ),
                ("area2", plan.map_or(J::Nul, |p| i(p.area2))),
            ]));
        }
        for exhausted in obs
            .own_agents
            .iter()
            .filter(|a| a.supply < cfg.recharge_cost_supply)
        {
            let mut restored = obs.clone();
            restored
                .own_agents
                .iter_mut()
                .find(|a| a.id == exhausted.id)
                .unwrap()
                .supply = cfg.recharge_cost_supply;
            let alternative_requests = self.requests(&restored);
            let alternative = self
                .core
                .preview_allocation(&restored, &alternative_requests);
            let newly_offered: Vec<_> = alternative_requests
                .get(&exhausted.id)
                .into_iter()
                .flatten()
                .filter(|r| matches!(r.task, Task::Recharge { .. }))
                .map(|r| task_j(&r.task))
                .collect();
            let grants = alternative
                .agents
                .iter()
                .filter(|a| {
                    [a.grant, a.companion]
                        .into_iter()
                        .flatten()
                        .any(|t| matches!(t, Task::Recharge { .. }))
                })
                .map(|a| {
                    o(vec![
                        ("agent", i(a.agent.0)),
                        ("primary", task_opt(a.grant.as_ref())),
                        ("companion", task_opt(a.companion.as_ref())),
                    ])
                })
                .collect();
            emit(o(vec![
                ("kind", s("supply_probe")),
                ("round", i(obs.round)),
                ("agent", i(exhausted.id.0)),
                ("recharge_candidates", a(newly_offered)),
                ("alternative_recharge_grants", a(grants)),
                (
                    "grants_changed",
                    J::Bool(
                        actual
                            .agents
                            .iter()
                            .zip(&alternative.agents)
                            .any(|(a, b)| a.grant != b.grant || a.companion != b.companion),
                    ),
                ),
                (
                    "new_recharge_grants",
                    a(recharges(&alternative)
                        .difference(&recharges(&actual))
                        .map(|(agent, portal)| {
                            o(vec![("agent", i(agent.0)), ("portal", i(portal.0))])
                        })
                        .collect()),
                ),
            ]));
        }
        let exhausted: Vec<_> = obs
            .own_agents
            .iter()
            .filter(|a| a.supply < cfg.recharge_cost_supply)
            .map(|a| a.id)
            .collect();
        if exhausted.len() > 1 {
            let mut restored = obs.clone();
            for agent in &mut restored.own_agents {
                agent.supply = agent.supply.max(cfg.recharge_cost_supply);
            }
            let alternative = self
                .core
                .preview_allocation(&restored, &self.requests(&restored));
            emit(o(vec![
                ("kind", s("supply_team_probe")),
                ("round", i(obs.round)),
                ("agents", a(exhausted.iter().map(|id| i(id.0)).collect())),
                (
                    "new_recharge_grants",
                    a(recharges(&alternative)
                        .difference(&recharges(&actual))
                        .map(|(agent, portal)| {
                            o(vec![("agent", i(agent.0)), ("portal", i(portal.0))])
                        })
                        .collect()),
                ),
            ]));
        }
        let commands = self.core.allocate_and_execute(obs, &requests);
        let own: Vec<_> = arena_bots::r2::greedy_coordinated::submissions(obs, commands);
        assert_eq!(&own, expected, "shadow command order round {}", obs.round);
    }
}

fn spec(tag: &str) -> PolicySpec {
    let id = PolicyId::from_tag(tag.split_once('|').unwrap().0).unwrap();
    let result = PolicySpec {
        policy_id: id,
        family: id.family(),
        version: R2_VERSION,
        parameters: PolicyParameters::from_tag(tag).unwrap(),
    };
    result.validate().unwrap();
    result
}

fn recharges(decision: &DecisionTrace) -> BTreeSet<(AgentId, PortalId)> {
    decision
        .agents
        .iter()
        .flat_map(|a| {
            [a.grant, a.companion]
                .into_iter()
                .flatten()
                .filter_map(move |task| match task {
                    Task::Recharge { portal } => Some((a.agent, portal)),
                    _ => None,
                })
        })
        .collect()
}

/// One explicitly selected legal alternative, settled from the exact archived
/// state. Other commands remain fixed for THIS round; no claim about future play.
fn check_alternative(bytes: &[u8], round: u32, agent: AgentId, portal: PortalId, move_first: bool) {
    let log = decode_log(bytes).unwrap();
    let h = &log.header;
    let (left, right) = GameState::initial_roster(&h.config);
    let enl = validate_initial_spawn_set(&left, &h.enl_spawns, &h.config).unwrap();
    let res = validate_initial_spawn_set(&right, &h.res_spawns, &h.config).unwrap();
    let mut baseline = GameState::new(h.config, &h.map, &enl, &res).unwrap();
    let mut branch = GameState::new(h.config, &h.map, &enl, &res).unwrap();
    let resolver = Resolver::with_seed(h.game_seed);
    for record in log.round_records() {
        let mut streams = record.submissions.clone();
        if record.round == round {
            let stream = streams.iter_mut().find(|s| s.agent_id == agent).unwrap();
            if move_first {
                stream.commands.push(Command::Move {
                    destination: branch.portal(portal).unwrap().position,
                });
            }
            stream.commands.push(Command::Hack { portal });
        }
        let before = baseline.agent(agent).map(|a| a.supply);
        let original = resolver.run_round(&mut baseline, &record.submissions);
        let alternative = resolver.run_round(&mut branch, &streams);
        if record.round == round {
            assert_eq!(original.score2_delta, alternative.score2_delta);
            assert_eq!(
                original.execution_order.len(),
                alternative.execution_order.len()
            );
            for (old, new) in original
                .execution_order
                .iter()
                .zip(&alternative.execution_order)
            {
                assert_eq!(old.agent_id, new.agent_id);
                assert_eq!(old.outcomes, new.outcomes[..old.outcomes.len()]);
                assert!(
                    new.outcomes[old.outcomes.len()..]
                        .iter()
                        .all(|o| matches!(o, CommandOutcome::Ok(_)))
                );
            }
            assert_eq!(
                branch.agent(agent).unwrap().supply,
                baseline.agent(agent).unwrap().supply + h.config.hack_supply_yield
            );
            let commands = &streams
                .iter()
                .find(|s| s.agent_id == agent)
                .unwrap()
                .commands;
            emit(o(vec![
                ("kind", s("legal_alternative")),
                ("round", i(round)),
                ("agent", i(agent.0)),
                ("commands", a(commands.iter().map(command_j).collect())),
                ("supply_before", i(before.unwrap())),
                (
                    "baseline_supply_after",
                    i(baseline.agent(agent).unwrap().supply),
                ),
                (
                    "alternative_supply_after",
                    i(branch.agent(agent).unwrap().supply),
                ),
                ("original_effects_unchanged", J::Bool(true)),
                ("same_round_income_unchanged", J::Bool(true)),
                ("added_commands_succeeded", J::Bool(true)),
                ("future_rounds_simulated", J::Bool(false)),
            ]));
            return;
        }
        baseline.advance_round();
        branch.advance_round();
        assert_eq!(canon::state_hash(&baseline), record.state_hash);
        assert_eq!(canon::state_hash(&branch), record.state_hash);
    }
    panic!("alternative round not found");
}

fn main() {
    let args: Vec<_> = env::args().collect();
    assert!(
        args.len() == 7 || args.len() == 11,
        "REPLAY BOT_SEED SUBJECT_SIDE SUBJECT_PARAMETERS OPPONENT_PARAMETERS VERSION [ROUND AGENT HACK_PORTAL MOVE_FIRST]"
    );
    assert_eq!(
        args[6], R2_VERSION,
        "never substitute a current version for a historical policy"
    );
    let bytes = fs::read(&args[1]).unwrap();
    let log = decode_log(&bytes).unwrap();
    let verified = replay(&log).unwrap();
    let h = &log.header;
    let seed: u64 = args[2].parse().unwrap();
    let subject: usize = args[3].parse().unwrap();
    assert!(subject < 2);
    let mut specs = [spec(&args[4]), spec(&args[5])];
    if subject == 1 {
        specs.swap(0, 1);
    }
    let factions = [Faction::Enl, Faction::Res];
    let roster = GameState::initial_roster(&h.config);
    let rosters = [&roster.0, &roster.1];
    let spawns = [&h.enl_spawns, &h.res_spawns];
    let mut players = [
        make_policy_player(&specs[0], factions[0], seed, &h.config, &h.map),
        make_policy_player(&specs[1], factions[1], seed, &h.config, &h.map),
    ];
    for idx in 0..2 {
        assert_eq!(
            &players[idx].choose_spawns(&h.config, &h.map, rosters[idx]),
            spawns[idx]
        );
    }
    let mut review = Review::new(&specs[subject], factions[subject], seed, &h.config);
    assert_eq!(
        &review
            .core
            .choose_spawns(&h.config, &h.map, rosters[subject]),
        spawns[subject]
    );
    let enl = validate_initial_spawn_set(rosters[0], spawns[0], &h.config).unwrap();
    let res = validate_initial_spawn_set(rosters[1], spawns[1], &h.config).unwrap();
    let mut state = GameState::new(h.config, &h.map, &enl, &res).unwrap();
    let resolver = Resolver::with_seed(h.game_seed);
    emit(o(vec![
        ("kind", s("identity")),
        ("version", s(R2_VERSION)),
        ("config", config_j(&h.config)),
        ("subject_side", i(subject as u64)),
        ("game_seed", i(h.game_seed)),
        ("bot_seed", i(seed)),
        ("subject_parameters", s(&args[4])),
        ("opponent_parameters", s(&args[5])),
        (
            "replay_sha256_note",
            s("recorded by the Python input boundary"),
        ),
        ("initial_hash", s(canon::hex(&canon::state_hash(&state)))),
    ]));
    for portal in state.portals() {
        emit(o(vec![
            ("kind", s("portal_position")),
            ("portal", i(portal.id.0)),
            (
                "position",
                a(vec![i(portal.position.x), i(portal.position.y)]),
            ),
        ]));
    }
    for record in log.round_records() {
        let observations = [
            Observation::for_faction(&state, factions[0]),
            Observation::for_faction(&state, factions[1]),
        ];
        for idx in 0..2 {
            let actual = players[idx].submit_round(&observations[idx]);
            let expected: Vec<_> = record
                .submissions
                .iter()
                .filter(|s| s.faction == factions[idx])
                .enumerate()
                .map(|(seq, sub)| {
                    assert_eq!(sub.receive_seq, seq as u32);
                    assert_eq!(sub.received_at_ms, 0, "this reviewer accepts Flat(0) only");
                    AgentSubmission::new(sub.agent_id, sub.commands.clone())
                })
                .collect();
            assert_eq!(
                actual, expected,
                "production policy round {} side {}",
                record.round, idx
            );
            if idx == subject {
                review.trace(
                    &observations[idx],
                    &actual,
                    players[idx].decision_trace().unwrap(),
                );
            }
        }
        let obs = &observations[subject];
        for p in &obs.portals {
            emit(o(vec![
                ("kind", s("portal")),
                ("round", i(record.round)),
                ("portal", i(p.id.0)),
                ("hp", i(p.hp)),
                ("owner", s(format!("{:?}", p.owner).to_lowercase())),
            ]));
        }
        for f in &obs.fields {
            emit(o(vec![
                ("kind", s("field")),
                ("round", i(record.round)),
                ("side", s(faction_s(f.faction))),
                ("field", a(vec![i(f.id.a.0), i(f.id.b.0), i(f.id.c.0)])),
                ("area2", i(f.area2)),
            ]));
        }
        for sub in &record.submissions {
            emit(o(vec![
                ("kind", s("commands")),
                ("round", i(record.round)),
                ("side", s(faction_s(sub.faction))),
                ("agent", i(sub.agent_id.0)),
                ("commands", a(sub.commands.iter().map(command_j).collect())),
            ]));
        }
        let report = resolver.run_round(&mut state, &record.submissions);
        for stream in &report.execution_order {
            for (seq, outcome) in stream.outcomes.iter().enumerate() {
                let result = match outcome {
                    CommandOutcome::Ok(effect) => effect_j(effect),
                    CommandOutcome::Drop(reason) => o(vec![
                        ("effect", s("drop")),
                        ("reason", s(format!("{reason:?}"))),
                    ]),
                };
                emit(o(vec![
                    ("kind", s("effect")),
                    ("round", i(record.round)),
                    ("side", s(faction_s(stream.faction))),
                    ("agent", i(stream.agent_id.0)),
                    ("seq", i(seq as u64)),
                    ("result", result),
                ]));
            }
        }
        emit(o(vec![
            ("kind", s("income")),
            ("round", i(record.round)),
            ("enl", i(report.score2_delta[&Faction::Enl] as i128)),
            ("res", i(report.score2_delta[&Faction::Res] as i128)),
        ]));
        state.advance_round();
        assert_eq!(canon::state_hash(&state), record.state_hash);
    }
    if args.len() == 11 {
        check_alternative(
            &bytes,
            args[7].parse().unwrap(),
            AgentId(args[8].parse().unwrap()),
            PortalId(args[9].parse().unwrap()),
            args[10].parse::<bool>().unwrap(),
        );
    }
    emit(o(vec![
        ("kind", s("verified")),
        ("rounds", i(verified.rounds_completed)),
        ("rules", J::Bool(true)),
        ("production_policies", J::Bool(true)),
        ("request_shadow", J::Bool(true)),
        ("final_hash", s(canon::hex(&verified.final_state_hash))),
    ]));
}
