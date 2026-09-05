//! Bounded Development combat contrasts; every arm preserves policy identity
//! and initialization, reproduces ordered decisions, and verifies full IARP.
#![forbid(unsafe_code)]

use arena_bots::{
    policy::{PolicyId, PolicyParameters, PolicySpec, make_policy_player},
    r2::{R2_VERSION, tasks::Task},
};
use arena_core::{
    event::EventKind,
    observe::Observation,
    resolve::{CommandEffect, CommandOutcome, Resolver, SubmittedCommandStream},
    spawn::validate_initial_spawn_set,
    state::GameState,
    types::Faction,
};
use arena_replay::{
    canon,
    recorder::ReplayWriter,
    replay::{decode_log, replay},
};
use arena_runner::balance::record::parse_balance_tsv;
use std::{
    collections::BTreeMap,
    error::Error,
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Clone)]
struct Input {
    map_index: u32,
    leg: u8,
    source_index: u64,
    bot_seed: u64,
    replay: PathBuf,
    digest: String,
}

#[derive(Clone)]
struct Arm {
    tag: String,
    subject: PolicySpec,
    opponent: PolicySpec,
}

fn arms(group: &str) -> Result<Vec<Arm>> {
    let mut result = Vec::new();
    match group {
        "supply-defense" | "supply-adaptive" => {
            for reserve in [0, 5] {
                let mut subject = if group == "supply-defense" {
                    PolicyId::BuilderDefenderR2.r2_spec()
                } else {
                    PolicyId::AdaptiveHybridR2.r2_spec()
                };
                match &mut subject.parameters {
                    PolicyParameters::BuilderDefender(p) => p.core.supply_reserve_recharges = reserve,
                    PolicyParameters::AdaptiveHybridR2(p) => p.core.supply_reserve_recharges = reserve,
                    _ => unreachable!(),
                }
                let mut opponent = PolicyId::BuilderDisruptorR2.r2_spec();
                if let PolicyParameters::BuilderDisruptor(p) = &mut opponent.parameters {
                    p.core.supply_reserve_recharges = 0;
                }
                result.push(Arm { tag: format!("reserve{reserve}"), subject, opponent });
            }
        }
        "compound" => {
            for defense_aux in [false, true] {
                for attack_aux in [false, true] {
                    let mut subject = PolicyId::BuilderDefenderR2.r2_spec();
                    let mut opponent = PolicyId::BuilderDisruptorR2.r2_spec();
                    if let PolicyParameters::BuilderDefender(p) = &mut subject.parameters {
                        p.core.combat_auxiliaries = defense_aux;
                    }
                    if let PolicyParameters::BuilderDisruptor(p) = &mut opponent.parameters {
                        p.core.combat_auxiliaries = attack_aux;
                    }
                    result.push(Arm {
                        tag: format!("daux{}-aaux{}", u8::from(defense_aux), u8::from(attack_aux)),
                        subject,
                        opponent,
                    });
                }
            }
        }
        "adaptive" => {
            for (tag, opening, lead) in [
                ("legacy", 300, 550),
                ("mid-open", 300, 0),
                ("all-open", 0, 0),
            ] {
                let mut spec = PolicyId::AdaptiveHybridR2.r2_spec();
                if let PolicyParameters::AdaptiveHybridR2(p) = &mut spec.parameters {
                    p.opening_permille = opening;
                    p.attack_lead_share_permille = lead;
                }
                result.push(Arm {
                    tag: tag.into(),
                    subject: spec,
                    opponent: PolicyId::BuilderDisruptorR2.r2_spec(),
                });
            }
        }
        "factorial" => {
            for defense in [0, 1, 2] {
                for attack in [0, 1000] {
                    let mut subject = PolicyId::BuilderDefenderR2.r2_spec();
                    let mut opponent = PolicyId::BuilderDisruptorR2.r2_spec();
                    if let PolicyParameters::BuilderDefender(p) = &mut subject.parameters {
                        p.defense_task_cap = defense;
                    }
                    if let PolicyParameters::BuilderDisruptor(p) = &mut opponent.parameters {
                        p.attack_share_permille = attack;
                    }
                    result.push(Arm {
                        tag: format!("def{defense}-atk{}", u32::from(attack > 0)),
                        subject,
                        opponent,
                    });
                }
            }
        }
        "adaptive-defense" => {
            for defense in [0, 2] {
                let mut subject = PolicyId::AdaptiveHybridR2.r2_spec();
                if let PolicyParameters::AdaptiveHybridR2(p) = &mut subject.parameters {
                    p.defense_task_cap = defense;
                }
                result.push(Arm {
                    tag: format!("adaptive-def{defense}"),
                    subject,
                    opponent: PolicyId::BuilderDisruptorR2.r2_spec(),
                });
            }
        }
        _ => return Err("group must be adaptive, factorial, adaptive-defense, compound, supply-defense, or supply-adaptive".into()),
    }
    for arm in &result {
        arm.subject.validate()?;
        arm.opponent.validate()?;
    }
    Ok(result)
}

fn input_cases(archive: &Path, count: u32) -> Result<Vec<Input>> {
    let records = parse_balance_tsv(&fs::read_to_string(archive.join("matches.tsv"))?)?;
    let mapping = fs::read_to_string(archive.join("replays.tsv"))?;
    let mut lines = mapping.lines();
    let names: Vec<_> = lines
        .next()
        .ok_or("missing replay header")?
        .split('\t')
        .collect();
    let col = |name: &str| {
        names
            .iter()
            .position(|s| *s == name)
            .ok_or("missing replay column")
    };
    let (status, index, file, digest) = (
        col("status")?,
        col("cell_index")?,
        col("file")?,
        col("blake3")?,
    );
    let mut replays = BTreeMap::new();
    for line in lines {
        let cells: Vec<_> = line.split('\t').collect();
        if cells[status] == "captured" {
            replays.insert(
                cells[index].parse::<u64>()?,
                (cells[file].to_owned(), cells[digest].to_owned()),
            );
        }
    }
    let mut result = Vec::new();
    for row in records.iter().filter(|r| {
        r.map_index < count
            && r.variant_id == "default"
            && r.policy_a == PolicyId::BuilderDefenderR2
            && r.policy_b == PolicyId::BuilderDisruptorR2
    }) {
        assert!(row.map_id.starts_with("development-"), "Development only");
        assert_eq!(
            row.bucket_policy,
            arena_runner::policy::BucketPolicy::Flat(0)
        );
        assert_eq!(row.policy_a_is_enl, row.leg == 0);
        // Some archives capture leg 0 only. Both legs share map/config/game
        // seed; the new policies still choose and record their own spawns.
        let base = records
            .iter()
            .find(|r| {
                r.map_index == row.map_index
                    && r.leg == 0
                    && r.variant_id == row.variant_id
                    && r.policy_a == row.policy_a
                    && r.policy_b == row.policy_b
            })
            .ok_or("missing paired leg")?;
        assert_eq!(
            (row.game_seed, row.bot_seed),
            (base.game_seed, base.bot_seed)
        );
        let (file, digest) = replays.get(&base.index).ok_or("missing source replay")?;
        let input = Input {
            map_index: row.map_index,
            leg: row.leg,
            source_index: row.index,
            bot_seed: row.bot_seed,
            replay: archive.join(file),
            digest: digest.clone(),
        };
        let bytes = fs::read(&input.replay)?;
        assert_eq!(blake3::hash(&bytes).to_hex().as_str(), input.digest);
        let source = decode_log(&bytes)?;
        assert_eq!(source.header.game_seed, row.game_seed);
        assert_eq!(
            source.header.config,
            arena_core::config::GameConfig::default()
        );
        result.push(input);
    }
    result.sort_by_key(|i| (i.map_index, i.leg));
    assert_eq!(
        result.len(),
        count as usize * 2,
        "complete map/leg coverage"
    );
    for (i, input) in result.iter().enumerate() {
        assert_eq!(
            (input.map_index, input.leg),
            ((i / 2) as u32, (i % 2) as u8)
        );
    }
    Ok(result)
}

#[derive(Default)]
struct Stats {
    agent_rounds: u64,
    grants: u64,
    empty_grants: u64,
    submitted: u64,
    attack_tasks: u64,
    defense_tasks: u64,
    attacks: u64,
    damage: u64,
    neutralized: u64,
    recharges: u64,
    hp: u64,
    supply: u64,
    fields: u64,
    drops: u64,
    no_ops: u64,
    movement: u64,
    stationary: u64,
    build_tasks: u64,
    companions: u64,
    hacks: u64,
    supply_gained: u64,
    links: u64,
    recharge_hack: u64,
    recharge_link: u64,
    recharge_move: u64,
    attack_hack: u64,
    attack_link: u64,
}

impl Stats {
    fn json(&self) -> String {
        let base = format!(
            "{{\"agent_rounds\":{},\"grants\":{},\"empty_grants\":{},\"submitted\":{},\"attack_tasks\":{},\"defense_tasks\":{},\"attacks\":{},\"damage\":{},\"neutralized\":{},\"recharges\":{},\"hp\":{},\"supply\":{},\"fields\":{},\"drops\":{},\"no_ops\":{},\"movement\":{},\"stationary\":{},\"build_tasks\":{}}}",
            self.agent_rounds,
            self.grants,
            self.empty_grants,
            self.submitted,
            self.attack_tasks,
            self.defense_tasks,
            self.attacks,
            self.damage,
            self.neutralized,
            self.recharges,
            self.hp,
            self.supply,
            self.fields,
            self.drops,
            self.no_ops,
            self.movement,
            self.stationary,
            self.build_tasks
        );
        format!(
            "{},\"companions\":{},\"hacks\":{},\"supply_gained\":{},\"links\":{},\"recharge_hack\":{},\"recharge_link\":{},\"recharge_move\":{},\"attack_hack\":{},\"attack_link\":{}}}",
            base.trim_end_matches('}'),
            self.companions,
            self.hacks,
            self.supply_gained,
            self.links,
            self.recharge_hack,
            self.recharge_link,
            self.recharge_move,
            self.attack_hack,
            self.attack_link
        )
    }
}

fn side(faction: Faction) -> usize {
    if faction == Faction::Enl { 0 } else { 1 }
}

fn run(out: &Path, input: &Input, arm: &Arm) -> Result<String> {
    let name = format!("{}-m{}-leg{}", arm.tag, input.map_index, input.leg);
    let dir = out.join(&name);
    fs::create_dir(&dir)?;
    let source = decode_log(&fs::read(&input.replay)?)?;
    let h = &source.header;
    let specs = if input.leg == 0 {
        [arm.subject, arm.opponent]
    } else {
        [arm.opponent, arm.subject]
    };
    let mut players = [
        make_policy_player(&specs[0], Faction::Enl, input.bot_seed, &h.config, &h.map),
        make_policy_player(&specs[1], Faction::Res, input.bot_seed, &h.config, &h.map),
    ];
    let mut shadows = [
        make_policy_player(&specs[0], Faction::Enl, input.bot_seed, &h.config, &h.map),
        make_policy_player(&specs[1], Faction::Res, input.bot_seed, &h.config, &h.map),
    ];
    let roster = GameState::initial_roster(&h.config);
    let entries = [
        players[0].choose_spawns(&h.config, &h.map, &roster.0),
        players[1].choose_spawns(&h.config, &h.map, &roster.1),
    ];
    assert_eq!(
        entries[0],
        shadows[0].choose_spawns(&h.config, &h.map, &roster.0)
    );
    assert_eq!(
        entries[1],
        shadows[1].choose_spawns(&h.config, &h.map, &roster.1)
    );
    let enl = validate_initial_spawn_set(&roster.0, &entries[0], &h.config)
        .map_err(|e| format!("{e:?}"))?;
    let res = validate_initial_spawn_set(&roster.1, &entries[1], &h.config)
        .map_err(|e| format!("{e:?}"))?;
    let mut state = GameState::new(h.config, &h.map, &enl, &res).map_err(|e| format!("{e:?}"))?;
    let initial_hash = blake3::Hash::from(canon::state_hash(&state))
        .to_hex()
        .to_string();
    let resolver = Resolver::with_seed(h.game_seed);
    let mut writer = ReplayWriter::new(h.game_seed, &h.config, &h.map, &enl, &res);
    let mut trace = BufWriter::new(File::create(dir.join("trace.jsonl"))?);
    let mut stats = [Stats::default(), Stats::default()];
    for _ in 0..h.config.total_rounds {
        let round = state.round();
        let observations = [
            Observation::for_faction(&state, Faction::Enl),
            Observation::for_faction(&state, Faction::Res),
        ];
        let submissions = [
            players[0].submit_round(&observations[0]),
            players[1].submit_round(&observations[1]),
        ];
        for i in 0..2 {
            assert_eq!(
                submissions[i],
                shadows[i].submit_round(&observations[i]),
                "ordered decisions: {name}/{round}"
            );
            let s = &mut stats[i];
            s.agent_rounds += observations[i].own_agents.len() as u64;
            s.submitted += submissions[i].len() as u64;
            if let Some(decision) = players[i].decision_trace() {
                for agent in &decision.agents {
                    s.grants += u64::from(agent.grant.is_some());
                    let submitted = submissions[i]
                        .iter()
                        .any(|sub| sub.agent_id == agent.agent && !sub.commands.is_empty());
                    s.empty_grants += u64::from(agent.grant.is_some() && !submitted);
                    let portal = agent
                        .grant
                        .and_then(|t| t.target_portal())
                        .map_or("null".into(), |p| p.0.to_string());
                    let me = observations[i]
                        .own_agents
                        .iter()
                        .find(|a| a.id == agent.agent)
                        .unwrap();
                    let target = agent.grant.and_then(|t| t.target_portal());
                    let hp = target
                        .and_then(|p| observations[i].portals.iter().find(|v| v.id == p))
                        .map_or("null".into(), |p| p.hp.to_string());
                    let key = target.is_some_and(|p| me.keys.get(&p).copied().unwrap_or(0) > 0);
                    let commands = submissions[i]
                        .iter()
                        .find(|s| s.agent_id == agent.agent)
                        .map_or("[]".to_owned(), |s| format!("{:?}", s.commands));
                    writeln!(
                        trace,
                        "{{\"kind\":\"decision\",\"round\":{round},\"side\":{i},\"agent\":{},\"task\":\"{}\",\"portal\":{portal},\"candidates\":{},\"submitted\":{submitted},\"position\":[{},{}],\"supply\":{},\"target_hp\":{hp},\"target_key\":{key},\"active_plan\":{},\"commands\":\"{commands}\"}}",
                        agent.agent.0,
                        agent.grant.map_or("none", |t| t.kind_tag()),
                        agent.candidate_count,
                        me.position.x,
                        me.position.y,
                        me.supply,
                        decision.active.is_some()
                    )?;
                    if let Some(task) = agent.companion {
                        s.companions += 1;
                        let target = task
                            .target_portal()
                            .map_or("null".into(), |p| p.0.to_string());
                        writeln!(
                            trace,
                            "{{\"kind\":\"companion\",\"round\":{round},\"side\":{i},\"agent\":{},\"task\":\"{}\",\"portal\":{target}}}",
                            agent.agent.0,
                            task.kind_tag()
                        )?;
                    }
                    for task in [agent.grant, agent.companion].into_iter().flatten() {
                        match task {
                            Task::Attack { .. } => s.attack_tasks += 1,
                            Task::Recharge { .. } => s.defense_tasks += 1,
                            _ => s.build_tasks += 1,
                        }
                    }
                }
            }
        }
        let streams: Vec<_> = [Faction::Enl, Faction::Res]
            .into_iter()
            .zip(&submissions)
            .flat_map(|(faction, subs)| {
                subs.iter()
                    .enumerate()
                    .map(move |(seq, sub)| SubmittedCommandStream {
                        round,
                        faction,
                        agent_id: sub.agent_id,
                        received_at_ms: sub.bucket * h.config.priority_bucket_ms as u64,
                        receive_seq: seq as u32,
                        commands: sub.commands.clone(),
                    })
            })
            .collect();
        let events_before = state.events().len();
        let mut portal_hp: BTreeMap<_, _> = observations[0]
            .portals
            .iter()
            .map(|p| (p.id, p.hp))
            .collect();
        let report = resolver.run_round(&mut state, &streams);
        for executed in &report.execution_order {
            let i = side(state.agent(executed.agent_id).unwrap().faction);
            let s = &mut stats[i];
            let mut changed = false;
            let (mut attacked, mut recharged, mut hacked, mut linked, mut moved) =
                (false, false, false, false, false);
            for outcome in &executed.outcomes {
                match outcome {
                    CommandOutcome::Drop(reason) => {
                        s.drops += 1;
                        // Debug is retained only as an opaque diagnostic string;
                        // all mechanism metrics below use typed effects.
                        writeln!(
                            trace,
                            "{{\"kind\":\"drop\",\"round\":{round},\"side\":{i},\"agent\":{},\"reason\":\"{reason:?}\"}}",
                            executed.agent_id.0
                        )?;
                    }
                    CommandOutcome::Ok(effect) => match effect {
                        CommandEffect::Attacked { hits } => {
                            attacked |= !hits.is_empty();
                            s.attacks += u64::from(!hits.is_empty());
                            changed |= !hits.is_empty();
                            for hit in hits {
                                let before = portal_hp.insert(hit.portal, hit.hp_after).unwrap();
                                s.damage += (before - hit.hp_after) as u64;
                                s.neutralized += u64::from(hit.hp_after == 0);
                                writeln!(
                                    trace,
                                    "{{\"kind\":\"attack\",\"round\":{round},\"side\":{i},\"agent\":{},\"portal\":{},\"before\":{before},\"after\":{}}}",
                                    executed.agent_id.0, hit.portal.0, hit.hp_after
                                )?;
                            }
                        }
                        CommandEffect::Recharged {
                            portal,
                            hp_before,
                            hp_after,
                        } => {
                            portal_hp.insert(*portal, *hp_after);
                            let hp = hp_after - hp_before;
                            recharged |= hp > 0;
                            s.recharges += 1;
                            s.hp += hp as u64;
                            s.supply += h.config.recharge_cost_supply as u64;
                            changed |= hp > 0;
                            writeln!(
                                trace,
                                "{{\"kind\":\"recharge\",\"round\":{round},\"side\":{i},\"agent\":{},\"portal\":{},\"before\":{hp_before},\"after\":{hp_after}}}",
                                executed.agent_id.0, portal.0
                            )?;
                        }
                        CommandEffect::Moved { from, to, .. } => {
                            moved |= from != to;
                            s.movement += 1;
                            s.stationary += u64::from(from == to);
                            changed |= from != to;
                        }
                        CommandEffect::Hacked {
                            portal,
                            keys_gained,
                            supply_granted,
                            supply_after,
                        } => {
                            hacked = true;
                            s.hacks += 1;
                            s.supply_gained += *supply_granted as u64;
                            changed |= *keys_gained > 0 || *supply_granted > 0;
                            writeln!(
                                trace,
                                "{{\"kind\":\"hack\",\"round\":{round},\"side\":{i},\"agent\":{},\"portal\":{},\"keys\":{keys_gained},\"supply\":{supply_granted},\"supply_after\":{supply_after}}}",
                                executed.agent_id.0, portal.0
                            )?;
                        }
                        CommandEffect::Linked { link, fields } => {
                            linked = true;
                            s.links += 1;
                            s.fields += fields.len() as u64;
                            changed = true;
                            writeln!(
                                trace,
                                "{{\"kind\":\"link\",\"round\":{round},\"side\":{i},\"agent\":{},\"source\":{},\"target\":{},\"fields\":{}}}",
                                executed.agent_id.0,
                                link.min.0,
                                link.max.0,
                                fields.len()
                            )?;
                        }
                        CommandEffect::Captured { portal } => {
                            portal_hp.insert(*portal, h.config.portal_max_hp);
                            changed = true;
                        }
                        _ => changed = true,
                    },
                }
            }
            s.recharge_hack += u64::from(recharged && hacked);
            s.recharge_link += u64::from(recharged && linked);
            s.recharge_move += u64::from(recharged && moved);
            s.attack_hack += u64::from(attacked && hacked);
            s.attack_link += u64::from(attacked && linked);
            writeln!(
                trace,
                "{{\"kind\":\"actions\",\"round\":{round},\"side\":{i},\"agent\":{},\"attack\":{attacked},\"recharge\":{recharged},\"hack\":{hacked},\"link\":{linked},\"move\":{moved}}}",
                executed.agent_id.0
            )?;
            s.no_ops += u64::from(!changed);
        }
        for event in &state.events()[events_before..] {
            let (kind, field, faction) = match event.kind {
                EventKind::FieldCreated { field, faction, .. } => ("field-created", field, faction),
                EventKind::FieldDestroyed { field, faction, .. } => {
                    ("field-destroyed-by", field, faction)
                }
                _ => continue,
            };
            let position = |id| {
                h.map
                    .portals()
                    .iter()
                    .find(|p| p.id == id)
                    .unwrap()
                    .position
            };
            let area = arena_core::geometry::area2(
                position(field.a),
                position(field.b),
                position(field.c),
            );
            writeln!(
                trace,
                "{{\"kind\":\"{kind}\",\"round\":{round},\"side\":{},\"field\":[{},{},{}],\"area2\":{area}}}",
                side(faction),
                field.a.0,
                field.b.0,
                field.c.0
            )?;
        }
        writeln!(
            trace,
            "{{\"kind\":\"income\",\"round\":{round},\"enl\":{},\"res\":{}}}",
            report.score2_delta[&Faction::Enl],
            report.score2_delta[&Faction::Res]
        )?;
        state.advance_round();
        writer.record_round(&report, &state)?;
    }
    trace.flush()?;
    let bytes = writer.finish(&state)?;
    let verified = replay(&decode_log(&bytes)?)?;
    assert_eq!(verified.final_state_hash, canon::state_hash(&state));
    fs::write(dir.join("replay.iarp"), &bytes)?;
    let subject = input.leg as usize;
    let scores = [verified.final_scores.0, verified.final_scores.1];
    let record = format!(
        "{{\"match\":\"{name}\",\"arm\":\"{}\",\"map_index\":{},\"leg\":{},\"source_index\":{},\"version\":\"{R2_VERSION}\",\"subject\":\"{}\",\"opponent\":\"{}\",\"subject_side\":{subject},\"subject_parameters\":\"{}\",\"opponent_parameters\":\"{}\",\"game_seed\":{},\"bot_seed\":{},\"initial_hash\":\"{initial_hash}\",\"subject_score\":{},\"opponent_score\":{},\"subject_stats\":{},\"opponent_stats\":{},\"rules_verified\":true,\"decisions_verified\":true,\"replay_blake3\":\"{}\",\"final_hash\":\"{}\"}}",
        arm.tag,
        input.map_index,
        input.leg,
        input.source_index,
        arm.subject.policy_id.tag(),
        arm.opponent.policy_id.tag(),
        arm.subject.parameters.tag(),
        arm.opponent.parameters.tag(),
        h.game_seed,
        input.bot_seed,
        scores[subject],
        scores[1 - subject],
        stats[subject].json(),
        stats[1 - subject].json(),
        blake3::hash(&bytes).to_hex(),
        blake3::Hash::from(verified.final_state_hash).to_hex()
    );
    fs::write(dir.join("result.json"), &record)?;
    eprintln!(
        "{name}: verified, subject {} opponent {}",
        scores[subject],
        scores[1 - subject]
    );
    Ok(record)
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 6 {
        return Err("combat-validate <Development archive> <new output> <adaptive|factorial|adaptive-defense> <map count> <workers>".into());
    }
    let archive = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    let count: u32 = args[4].parse()?;
    let workers: usize = args[5].parse()?;
    if count == 0 || count > 20 || workers == 0 || workers > 32 {
        return Err("bounded run: 1..20 maps, 1..32 workers".into());
    }
    if out.exists() {
        return Err("output exists; use a new directory".into());
    }
    let inputs = input_cases(archive, count)?;
    let arms = arms(&args[3])?;
    fs::create_dir_all(out)?;
    fs::write(
        out.join("manifest.txt"),
        format!(
            "status=running\nversion={R2_VERSION}\nsource_archive={}\ngroup={}\nmaps={count}\nworkers={workers}\n",
            archive.display(),
            args[3]
        ),
    )?;
    let jobs: Vec<_> = arms
        .iter()
        .flat_map(|arm| inputs.iter().map(move |input| (input.clone(), arm.clone())))
        .collect();
    let jobs = Arc::new(jobs);
    let next = AtomicUsize::new(0);
    let mut results = std::thread::scope(|scope| -> Result<Vec<String>> {
        let mut threads = Vec::new();
        for _ in 0..workers.min(jobs.len()) {
            let jobs = &jobs;
            let next = &next;
            threads.push(scope.spawn(move || -> Result<Vec<String>> {
                let mut result = Vec::new();
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some((input, arm)) = jobs.get(i) else {
                        break;
                    };
                    result.push(run(out, input, arm)?);
                }
                Ok(result)
            }));
        }
        let mut result = Vec::new();
        for thread in threads {
            result.extend(thread.join().map_err(|_| "worker panicked")??);
        }
        Ok(result)
    })?;
    results.sort();
    fs::write(out.join("matches.jsonl"), results.join("\n") + "\n")?;
    let manifest = fs::read_to_string(out.join("manifest.txt"))?;
    fs::write(
        out.join("manifest.txt"),
        manifest.replace(
            "status=running",
            &format!(
                "status=complete\nmatches={}\nrules_verified={}\ndecisions_verified={}",
                results.len(),
                results.len(),
                results.len()
            ),
        ),
    )?;
    Ok(())
}
